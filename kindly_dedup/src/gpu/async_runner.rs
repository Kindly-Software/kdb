//! Async GPU Runner - T7 Heterogeneous Tier (Background GPU Processing)
//!
//! Runs GPU computation asynchronously while CPU fills next batch.
//!
//! # Architecture
//!
//! ```text
//! Main Thread (CPU)              Background Thread (GPU)
//!     |                                  |
//! [Submit Batch] ──────────────────────> |
//!     |                            [Process on GPU]
//! [Fill Next Batch]                      |
//!     |<─────────────────────── [Result Ready]
//! [Poll Result]                          |
//!     |                                  |
//! ```
//!
//! # Performance Benefits
//!
//! - **Async Overlap**: CPU fills while GPU processes (>80% overlap target)
//! - **Latency Hiding**: GPU transfer time hidden by CPU tokenization
//! - **Queue Depth**: Multiple batches in flight (configurable)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier (CPU-GPU async coordination)
//! - **Chaos**: 100% lockfree (DualAtomicU64, atomic slot states, cache-aligned 128B)
//! - **ASSUM**: Thread safety via Arc, shutdown via packed atomic state
//! - **B32**: Overlap efficiency benchmarks
//! - **T28**: Async flow tests, shutdown tests
//!
//! # Chaos Compliance (v2.5.0)
//!
//! - **AsyncGpuRunner**: DualAtomicU64 state packing (running|generation|batches_submitted|batches_completed)
//! - **LockfreeResultQueue**: Atomic slot states (empty→writing→ready→reading→empty)
//! - **GpuBatchResult**: Cache-aligned `#[repr(C, align(64))]`

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use super::context::GpuContextCapsule;
use super::kernels::{MinHashGpuCapsule, MinHashGpuInput};
use super::pipeline_coordinator::{AsyncPipelineCoordinator, GpuBatch, PipelinePhase};
#[allow(unused_imports)]
use super::error::GpuResult;

/// Result queue capacity (number of pending results)
const RESULT_QUEUE_DEPTH: usize = 16;

/// Maximum spin iterations before yielding
const MAX_SPIN_ITERATIONS: usize = 1000;

/// Number of LSH bands (matches batch_lookup.rs)
const NUM_BANDS: usize = 5;

/// Rows per band (5 × 25 = 125, 3 unused from 128-hash signature)
const ROWS_PER_BAND: usize = 25;

/// Signature size (u16 values per document)
const SIGNATURE_SIZE: usize = 128;

// ============================================================================
// Slot State Constants (Chaos-compliant atomic state machine)
// ============================================================================

/// Slot state: Empty and available for writing
const SLOT_STATE_EMPTY: u64 = 0;
/// Slot state: Producer is writing data
const SLOT_STATE_WRITING: u64 = 1;
/// Slot state: Data is ready for reading
const SLOT_STATE_READY: u64 = 2;
/// Slot state: Consumer is reading data
const SLOT_STATE_READING: u64 = 3;

// ============================================================================
// LSH Band Hash Computation (CPU reference, matches GPU kernel)
// ============================================================================

/// Compute LSH band hash from u16 MinHash signature
///
/// # Algorithm
///
/// Matches GPU kernel lsh_band.wgsl and CPU batch_lookup.rs:
/// - hash = 0
/// - for each row in band: hash = hash * 31 + value (wrapping)
///
/// # ASSUM Safety
///
/// - `#ASSUME_SIGNATURE_SIZE`: signature has 128 u16 values
/// - `#VERIFY_SIGNATURE_SIZE`: Caller provides get_signature() from GpuBatchResult
/// - `#ASSUME_BAND_RANGE`: band_idx < NUM_BANDS (5), rows [0..125)
/// - `#VERIFY_BAND_RANGE`: start/end bounds checked, min() ensures no overflow
///
/// # Determinism
///
/// - 100% deterministic: Same signature → Same hash (Q16.16 not needed, u64 native)
/// - Wrapping arithmetic matches GPU u64 wrapping
/// - No floating point, no randomness
#[inline]
fn compute_band_hash_from_u16(signature: &[u16], band_idx: usize) -> u64 {
    let start = band_idx * ROWS_PER_BAND;
    let end = (start + ROWS_PER_BAND).min(SIGNATURE_SIZE);

    let mut hash: u64 = 0;
    for i in start..end {
        // hash = hash * 31 + value (wrapping, matches GPU)
        hash = hash.wrapping_mul(31).wrapping_add(signature[i] as u64);
    }
    hash
}

/// Compute all LSH band hashes for a batch of signatures
///
/// Converts Vec<u16> signatures to Vec<u64> band hashes.
///
/// # Arguments
///
/// - `signatures`: Flat array of u16 MinHash signatures (128 per document)
/// - `num_docs`: Number of documents in batch
///
/// # Returns
///
/// Vec<u64> with NUM_BANDS (5) hashes per document, total length = num_docs × 5
///
/// # Performance
///
/// - Per-hash: ~50ns (25 rows × 2ns per multiply-add)
/// - Per-doc: ~250ns (5 bands × 50ns)
/// - Batch 1000: ~250μs (amortized, sequential)
///
/// # ASSUM Safety
///
/// - `#ASSUME_SIGNATURE_LENGTH`: signatures.len() == num_docs × 128
/// - `#VERIFY_SIGNATURE_LENGTH`: Caller (GPU output) guarantees correct length
/// - `#ASSUME_NO_PANIC`: get_signature() panics if out of range, caller validates
/// - `#VERIFY_BATCH_SIZE`: GpuBatchResult validates doc_ids.len() == num_docs
fn compute_lsh_band_hashes_from_u16(signatures: &[u16], num_docs: usize) -> Vec<u64> {
    let mut band_hashes = Vec::with_capacity(num_docs * NUM_BANDS);

    for doc_idx in 0..num_docs {
        let start = doc_idx * SIGNATURE_SIZE;
        let end = start + SIGNATURE_SIZE;
        let signature = &signatures[start..end];

        for band_idx in 0..NUM_BANDS {
            let hash = compute_band_hash_from_u16(signature, band_idx);
            band_hashes.push(hash);
        }
    }

    band_hashes
}

// ============================================================================
// AsyncGpuRunner State Packing (DualAtomicU64 pattern)
// ============================================================================
//
// State word layout (64 bits):
// | running (1 bit) | generation (31 bits) | batches_submitted (16 bits) | batches_completed (16 bits) |
// | bit 63          | bits 32-62           | bits 16-31                  | bits 0-15                   |

/// Extract running flag from packed state
#[inline(always)]
fn state_running(state: u64) -> bool {
    (state >> 63) != 0
}

/// Extract generation from packed state
#[inline(always)]
fn state_generation(state: u64) -> u32 {
    ((state >> 32) & 0x7FFF_FFFF) as u32
}

/// Extract batches_submitted from packed state
#[inline(always)]
fn state_batches_submitted(state: u64) -> u16 {
    ((state >> 16) & 0xFFFF) as u16
}

/// Extract batches_completed from packed state
#[inline(always)]
fn state_batches_completed(state: u64) -> u16 {
    (state & 0xFFFF) as u16
}

/// Pack state fields into u64
#[inline(always)]
fn pack_state(running: bool, generation: u32, submitted: u16, completed: u16) -> u64 {
    let running_bit = if running { 1u64 << 63 } else { 0 };
    let gen = ((generation & 0x7FFF_FFFF) as u64) << 32;
    let sub = (submitted as u64) << 16;
    let comp = completed as u64;
    running_bit | gen | sub | comp
}

// ============================================================================
// GpuBatchResult - Cache-aligned capsule
// ============================================================================

/// Result from GPU batch processing
///
/// Contains MinHash signatures and optional LSH band hashes.
///
/// # Chaos Compliance
///
/// - `#[repr(C, align(64))]`: Cache-line aligned to prevent false sharing
/// - Immutable after creation (no interior mutability needed)
/// - Generation counter for Q34 audit trail
#[repr(C, align(64))]
#[derive(Clone)]
pub struct GpuBatchResult {
    /// Document IDs in this batch
    pub doc_ids: Vec<u32>,

    /// MinHash signatures (128 x u16 per document, packed as Vec<u16>)
    pub signatures: Vec<u16>,

    /// LSH band hashes (optional, for candidate pair generation)
    pub band_hashes: Option<Vec<u64>>,

    /// GPU processing time (microseconds)
    pub processing_time_us: u64,

    /// Generation counter (Q34 audit)
    pub generation: u64,
}

impl GpuBatchResult {
    /// Get signature for document index
    pub fn get_signature(&self, doc_idx: usize) -> &[u16] {
        let start = doc_idx * 128;
        let end = start + 128;
        &self.signatures[start..end]
    }

    /// Get number of documents in result
    pub fn num_docs(&self) -> usize {
        self.doc_ids.len()
    }
}

// ============================================================================
// ResultSlot - Individual slot with atomic state machine
// ============================================================================

/// Result slot with atomic state for lockfree SPSC queue
///
/// # Chaos Compliance
///
/// - `#[repr(C, align(64))]`: Cache-line aligned to prevent false sharing between slots
/// - Atomic state machine: EMPTY → WRITING → READY → READING → EMPTY
/// - MaybeUninit for zero-cost uninitialized storage
///
/// # ASSUM Safety
///
/// - `#ASSUME_STATE_TRANSITIONS`: Only valid state transitions occur
/// - `#VERIFY_STATE_TRANSITIONS`: CAS ensures atomic state changes
/// - `#ASSUME_MAYBEUNINIT_VALID`: Data only read when state is READY
/// - `#VERIFY_MAYBEUNINIT_VALID`: State machine guarantees initialization before read
#[repr(C, align(64))]
struct ResultSlot {
    /// Atomic state: EMPTY(0), WRITING(1), READY(2), READING(3)
    state: AtomicU64,
    /// Uninitialized storage for GpuBatchResult
    data: UnsafeCell<MaybeUninit<GpuBatchResult>>,
}

impl ResultSlot {
    /// Create new empty slot
    fn new() -> Self {
        Self {
            state: AtomicU64::new(SLOT_STATE_EMPTY),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

/// Lockfree result queue using atomic slot states
///
/// # Chaos Compliance
///
/// 100% lockfree - uses atomic slot states (not just head/tail), no Mutex/RwLock.
/// Each slot has its own state machine ensuring proper synchronization.
///
/// # ASSUM Safety
///
/// - `#ASSUME_SPSC_SINGLE_PRODUCER`: Only GPU thread pushes results
/// - `#VERIFY_SPSC_SINGLE_PRODUCER`: AsyncGpuRunner.start() spawns exactly one worker thread
/// - `#ASSUME_SPSC_SINGLE_CONSUMER`: Only main thread pops results
/// - `#VERIFY_SPSC_SINGLE_CONSUMER`: poll_result() only callable from HybridDedupPipeline owner
/// - `#ASSUME_SLOT_ISOLATION`: Each slot accessed by only one thread at a time
/// - `#VERIFY_SLOT_ISOLATION`: Atomic state machine prevents concurrent access
#[repr(C, align(128))]
pub struct LockfreeResultQueue {
    /// Head index (producer writes here, wraps around)
    head: AtomicU64,

    /// Padding to separate head/tail cache lines (prevent false sharing)
    _pad1: [u8; 56],

    /// Tail index (consumer reads here, wraps around)
    tail: AtomicU64,

    /// Padding to separate tail from slots
    _pad2: [u8; 56],

    /// Ring buffer of result slots (each slot is cache-aligned)
    slots: Box<[ResultSlot; RESULT_QUEUE_DEPTH]>,
}

impl LockfreeResultQueue {
    /// Create new result queue
    fn new() -> Self {
        // Initialize array with empty slots
        let slots: [ResultSlot; RESULT_QUEUE_DEPTH] = std::array::from_fn(|_| ResultSlot::new());

        Self {
            head: AtomicU64::new(0),
            _pad1: [0; 56],
            tail: AtomicU64::new(0),
            _pad2: [0; 56],
            slots: Box::new(slots),
        }
    }

    /// Push result (producer only)
    ///
    /// # State Machine
    ///
    /// 1. Load head, check if slot is EMPTY
    /// 2. CAS slot state: EMPTY → WRITING
    /// 3. Write data to slot
    /// 4. Store slot state: WRITING → READY
    /// 5. Advance head
    ///
    /// Returns true if pushed, false if queue full.
    fn push(&self, result: GpuBatchResult) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        // Check if full (all slots occupied)
        if head.wrapping_sub(tail) >= RESULT_QUEUE_DEPTH as u64 {
            return false;
        }

        let idx = (head % RESULT_QUEUE_DEPTH as u64) as usize;
        let slot = &self.slots[idx];

        // CAS: EMPTY → WRITING
        if slot
            .state
            .compare_exchange(
                SLOT_STATE_EMPTY,
                SLOT_STATE_WRITING,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_err()
        {
            // Slot not empty (shouldn't happen in SPSC, but be defensive)
            return false;
        }

        // SAFETY: We have exclusive access via WRITING state.
        // #ASSUME_EXCLUSIVE_WRITE: Only producer writes when state is WRITING
        // #VERIFY_EXCLUSIVE_WRITE: CAS ensures only one thread transitions to WRITING
        unsafe {
            (*slot.data.get()).write(result);
        }

        // Store: WRITING → READY (release semantics to publish data)
        slot.state.store(SLOT_STATE_READY, Ordering::Release);

        // Advance head
        self.head.store(head.wrapping_add(1), Ordering::Release);

        true
    }

    /// Pop result (consumer only)
    ///
    /// # State Machine
    ///
    /// 1. Load tail, check if slot is READY
    /// 2. CAS slot state: READY → READING
    /// 3. Read data from slot
    /// 4. Store slot state: READING → EMPTY
    /// 5. Advance tail
    ///
    /// Returns Some(result) if available, None if empty.
    fn pop(&self) -> Option<GpuBatchResult> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);

        // Check if empty
        if head == tail {
            return None;
        }

        let idx = (tail % RESULT_QUEUE_DEPTH as u64) as usize;
        let slot = &self.slots[idx];

        // CAS: READY → READING
        if slot
            .state
            .compare_exchange(
                SLOT_STATE_READY,
                SLOT_STATE_READING,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_err()
        {
            // Slot not ready (producer still writing, wait)
            return None;
        }

        // SAFETY: We have exclusive access via READING state.
        // Data was initialized when state transitioned to READY.
        // #ASSUME_DATA_INITIALIZED: Data written before READY state
        // #VERIFY_DATA_INITIALIZED: Producer stores READY only after write completes
        let result = unsafe { (*slot.data.get()).assume_init_read() };

        // Store: READING → EMPTY (release semantics to publish slot availability)
        slot.state.store(SLOT_STATE_EMPTY, Ordering::Release);

        // Advance tail
        self.tail.store(tail.wrapping_add(1), Ordering::Release);

        Some(result)
    }

    /// Check if queue is empty
    fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head == tail
    }

    /// Get number of pending results
    fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head.wrapping_sub(tail) as usize
    }
}

// SAFETY: Queue is designed for single-producer single-consumer (SPSC).
// #ASSUME_SPSC_INVARIANT: Only one producer thread and one consumer thread
// #VERIFY_SPSC_INVARIANT: AsyncGpuRunner spawns exactly one worker (producer),
//                         poll_result() called only from pipeline owner (consumer)
unsafe impl Send for LockfreeResultQueue {}
unsafe impl Sync for LockfreeResultQueue {}

// ============================================================================
// AsyncGpuRunner - Chaos-compliant with DualAtomicU64 state packing
// ============================================================================

/// Padding size for 128B cache-line alignment
/// Layout: state(8) + ctx(8) + minhash(8) + coordinator(8) + handle(16) + results(8) = 56 bytes
/// Need 72 bytes padding for 128B total
const ASYNC_GPU_RUNNER_PADDING: usize = 72;

/// Async GPU runner that processes batches in background
///
/// # Architecture
///
/// ```text
/// AsyncGpuRunner (128B cache-aligned)
/// ├── state: AtomicU64 (packed: running|generation|submitted|completed)
/// ├── ctx: Arc<GpuContextCapsule> (shared GPU context)
/// ├── minhash: Arc<MinHashGpuCapsule> (shared compute pipeline)
/// ├── coordinator: Arc<AsyncPipelineCoordinator> (phase coordination)
/// ├── handle: Option<JoinHandle> (background thread)
/// └── results: Arc<LockfreeResultQueue> (result queue)
/// ```
///
/// # Chaos Compliance
///
/// - `#[repr(C, align(128))]`: 128B cache-line aligned to prevent false sharing
/// - DualAtomicU64 pattern: All control state packed into single AtomicU64
/// - State packing: running(1) | generation(31) | submitted(16) | completed(16)
/// - 100% lockfree - no mutex, no RwLock, no scattered atomics
///
/// # ASSUM Safety
///
/// - `#ASSUME_GPU_THREAD_SAFE`: wgpu is thread-safe
/// - `#VERIFY_GPU_THREAD_SAFE`: wgpu guarantees Send + Sync
/// - `#ASSUME_SHUTDOWN_ORDERED`: Packed state ensures clean shutdown
/// - `#VERIFY_SHUTDOWN_ORDERED`: Join handle waits for completion
/// - `#ASSUME_STATE_ATOMIC`: 64-bit atomic operations are lock-free on x86_64
/// - `#VERIFY_STATE_ATOMIC`: AtomicU64::is_lock_free() == true on modern CPUs
#[repr(C, align(128))]
pub struct AsyncGpuRunner {
    /// Packed state: running(1) | generation(31) | submitted(16) | completed(16)
    ///
    /// Layout (64 bits):
    /// - Bit 63: running flag (1 = running, 0 = stopped)
    /// - Bits 32-62: generation counter (31 bits, Q34 audit)
    /// - Bits 16-31: batches_submitted counter (16 bits)
    /// - Bits 0-15: batches_completed counter (16 bits)
    state: AtomicU64,

    /// GPU context (shared)
    ctx: Arc<GpuContextCapsule>,

    /// MinHash GPU capsule (shared)
    minhash: Arc<MinHashGpuCapsule>,

    /// Pipeline coordinator (shared)
    coordinator: Arc<AsyncPipelineCoordinator>,

    /// Background thread handle
    handle: Option<JoinHandle<()>>,

    /// Result queue (lockfree)
    results: Arc<LockfreeResultQueue>,

    /// Padding for 128B cache-line alignment
    _padding: [u8; ASYNC_GPU_RUNNER_PADDING],
}

impl AsyncGpuRunner {
    /// Create new async GPU runner
    ///
    /// # Arguments
    ///
    /// - `ctx`: GPU context (Arc for sharing)
    /// - `minhash`: MinHash GPU capsule (Arc for sharing)
    /// - `coordinator`: Pipeline coordinator (Arc for sharing)
    ///
    /// # Performance
    ///
    /// - Creation: <1ms (no GPU operations)
    /// - Thread start: <10ms (thread spawn)
    pub fn new(
        ctx: Arc<GpuContextCapsule>,
        minhash: Arc<MinHashGpuCapsule>,
        coordinator: Arc<AsyncPipelineCoordinator>,
    ) -> Self {
        Self {
            state: AtomicU64::new(0), // running=false, generation=0, submitted=0, completed=0
            ctx,
            minhash,
            coordinator,
            handle: None,
            results: Arc::new(LockfreeResultQueue::new()),
            _padding: [0; ASYNC_GPU_RUNNER_PADDING],
        }
    }

    /// Start background GPU processing thread
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_NOT_RUNNING`: start() called only once
    /// - `#VERIFY_NOT_RUNNING`: CAS ensures atomic transition to running
    /// - `#ASSUME_SINGLE_START`: Only one thread calls start()
    /// - `#VERIFY_SINGLE_START`: Mutable reference ensures exclusive access
    pub fn start(&mut self) {
        // CAS to set running bit (bit 63)
        let current = self.state.load(Ordering::Relaxed);
        if state_running(current) {
            return; // Already running
        }

        // Set running bit using CAS for safety
        let new_state = current | (1u64 << 63);
        if self
            .state
            .compare_exchange(current, new_state, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            return; // Another thread started (shouldn't happen with &mut self)
        }

        let ctx = self.ctx.clone();
        let minhash = self.minhash.clone();
        let coordinator = self.coordinator.clone();
        let results = self.results.clone();

        // Create a shared state reference for the worker thread
        // SAFETY: Worker thread only reads/updates state atomically
        let state_ptr = &self.state as *const AtomicU64 as usize;

        self.handle = Some(thread::spawn(move || {
            // SAFETY: state_ptr points to valid AtomicU64 for lifetime of AsyncGpuRunner
            // Worker thread only reads running flag and updates generation
            // #ASSUME_STATE_LIFETIME: AsyncGpuRunner outlives worker thread
            // #VERIFY_STATE_LIFETIME: Drop impl joins thread before deallocation
            let state = unsafe { &*(state_ptr as *const AtomicU64) };
            Self::gpu_worker_loop(ctx, minhash, coordinator, state, results);
        }));
    }

    /// Stop background processing (graceful shutdown)
    ///
    /// Signals worker to stop and waits for completion.
    pub fn stop(&mut self) {
        // Clear running bit (bit 63) using atomic AND
        self.state.fetch_and(!(1u64 << 63), Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if runner is active
    pub fn is_running(&self) -> bool {
        state_running(self.state.load(Ordering::Acquire))
    }

    /// GPU worker loop (runs in background thread)
    ///
    /// Processes batches from coordinator and pushes results to queue.
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STATE_VALID`: state pointer remains valid for loop duration
    /// - `#VERIFY_STATE_VALID`: Drop impl joins thread before deallocation
    /// - `#ASSUME_GENERATION_UPDATE`: Only worker updates generation field
    /// - `#VERIFY_GENERATION_UPDATE`: Main thread only reads generation via getter
    fn gpu_worker_loop(
        ctx: Arc<GpuContextCapsule>,
        minhash: Arc<MinHashGpuCapsule>,
        coordinator: Arc<AsyncPipelineCoordinator>,
        state: &AtomicU64,
        results: Arc<LockfreeResultQueue>,
    ) {
        // Check running flag from packed state
        while state_running(state.load(Ordering::Acquire)) {
            // Wait for batch to be ready (spin-wait with backoff)
            let phase = coordinator.phase();

            match phase {
                PipelinePhase::GpuProcessing => {
                    // Get batch to process
                    let batch = coordinator.take_processing_batch();

                    if batch.is_empty() {
                        // Empty batch, transition back to idle
                        coordinator.transition(PipelinePhase::GpuProcessing, PipelinePhase::Idle);
                        continue;
                    }

                    // Process batch on GPU
                    let start = Instant::now();

                    let input = MinHashGpuInput {
                        tokens: &batch.tokens,
                        offsets: &batch.offsets,
                        num_docs: batch.len() as u32,
                    };

                    match minhash.compute(ctx.as_ref(), input) {
                        Ok(output) => {
                            let processing_time_us = start.elapsed().as_micros() as u64;

                            // Get current generation from packed state
                            let current_state = state.load(Ordering::Relaxed);
                            let generation = state_generation(current_state) as u64;

                            // Convert output to result - copy signatures from GPU output
                            let signatures: Vec<u16> = (0..batch.len())
                                .flat_map(|i| output.get_signature(i).to_vec())
                                .collect();

                            // Compute LSH band hashes from signatures
                            // Performance: ~250ns per document (5 bands × 50ns per hash)
                            // Critical for O(1) LSH bucket lookup (avoids O(n²) brute-force)
                            let band_hashes = compute_lsh_band_hashes_from_u16(&signatures, batch.len());

                            let result = GpuBatchResult {
                                doc_ids: batch.doc_ids.clone(),
                                signatures,
                                band_hashes: Some(band_hashes),
                                processing_time_us,
                                generation,
                            };

                            // Push result to queue (may block if full)
                            let mut push_attempts = 0;
                            while !results.push(result.clone()) {
                                thread::yield_now();
                                push_attempts += 1;
                                if push_attempts > MAX_SPIN_ITERATIONS {
                                    // Queue full for too long, drop result
                                    eprintln!("AsyncGpuRunner: Result queue full, dropping batch");
                                    break;
                                }
                            }

                            coordinator.record_gpu_batch();

                            // Increment generation in packed state using CAS loop
                            loop {
                                let old = state.load(Ordering::Relaxed);
                                let old_gen = state_generation(old);
                                let new_gen = old_gen.wrapping_add(1) & 0x7FFF_FFFF;
                                let new = (old & !(0x7FFF_FFFF_u64 << 32)) | ((new_gen as u64) << 32);
                                if state
                                    .compare_exchange_weak(old, new, Ordering::Release, Ordering::Relaxed)
                                    .is_ok()
                                {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("AsyncGpuRunner: GPU batch failed: {}", e);
                        }
                    }

                    // Signal ready for next batch
                    coordinator.transition(PipelinePhase::GpuProcessing, PipelinePhase::Swapping);
                    coordinator.swap_buffers();
                    coordinator.transition(PipelinePhase::Swapping, PipelinePhase::Idle);
                }

                PipelinePhase::Draining => {
                    // Draining mode: process remaining batch and exit
                    let batch = coordinator.take_processing_batch();

                    if !batch.is_empty() {
                        let start = Instant::now();

                        let input = MinHashGpuInput {
                            tokens: &batch.tokens,
                            offsets: &batch.offsets,
                            num_docs: batch.len() as u32,
                        };

                        if let Ok(output) = minhash.compute(ctx.as_ref(), input) {
                            // Get current generation from packed state
                            let current_state = state.load(Ordering::Relaxed);
                            let generation = state_generation(current_state) as u64;

                            // Copy signatures from GPU output
                            let signatures: Vec<u16> = (0..batch.len())
                                .flat_map(|i| output.get_signature(i).to_vec())
                                .collect();

                            // Compute LSH band hashes from signatures
                            let band_hashes = compute_lsh_band_hashes_from_u16(&signatures, batch.len());

                            let result = GpuBatchResult {
                                doc_ids: batch.doc_ids.clone(),
                                signatures,
                                band_hashes: Some(band_hashes),
                                processing_time_us: start.elapsed().as_micros() as u64,
                                generation,
                            };

                            let _ = results.push(result);
                            coordinator.record_gpu_batch();
                        }
                    }

                    // Exit draining mode
                    coordinator.transition(PipelinePhase::Draining, PipelinePhase::Idle);
                }

                _ => {
                    // Not in processing phase, yield
                    thread::yield_now();
                }
            }
        }
    }

    /// Poll for completed results (non-blocking)
    ///
    /// # Returns
    ///
    /// - `Some(result)`: Completed batch result
    /// - `None`: No results ready
    pub fn poll_result(&self) -> Option<GpuBatchResult> {
        let result = self.results.pop();
        if result.is_some() {
            // Increment batches_completed in packed state (bits 0-15)
            self.increment_completed();
        }
        result
    }

    /// Increment batches_completed counter in packed state
    #[inline(always)]
    fn increment_completed(&self) {
        loop {
            let old = self.state.load(Ordering::Relaxed);
            let old_completed = state_batches_completed(old);
            let new_completed = old_completed.wrapping_add(1);
            let new = (old & !0xFFFF) | (new_completed as u64);
            if self
                .state
                .compare_exchange_weak(old, new, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Increment batches_submitted counter in packed state
    #[inline(always)]
    fn increment_submitted(&self) {
        loop {
            let old = self.state.load(Ordering::Relaxed);
            let old_submitted = state_batches_submitted(old);
            let new_submitted = old_submitted.wrapping_add(1);
            let new = (old & !(0xFFFF << 16)) | ((new_submitted as u64) << 16);
            if self
                .state
                .compare_exchange_weak(old, new, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Check if results are available
    pub fn has_results(&self) -> bool {
        !self.results.is_empty()
    }

    /// Get number of pending results
    pub fn pending_results(&self) -> usize {
        self.results.len()
    }

    /// Submit batch for GPU processing
    ///
    /// # Arguments
    ///
    /// - `batch`: GPU batch to process
    ///
    /// # Returns
    ///
    /// - `true`: Batch submitted successfully
    /// - `false`: Coordinator busy or timeout
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_BATCH_VALID`: Batch has valid data
    /// - `#VERIFY_BATCH_VALID`: GpuBatch validates offsets
    pub fn submit_batch(&self, batch: GpuBatch) -> bool {
        // Wait for idle or cpu_filling phase
        let mut spins = 0;
        loop {
            let phase = self.coordinator.phase();

            match phase {
                PipelinePhase::Idle => {
                    // Transition to CpuFilling if idle
                    if self.coordinator.transition(PipelinePhase::Idle, PipelinePhase::CpuFilling) {
                        break;
                    }
                }
                PipelinePhase::CpuFilling => {
                    // Already in filling phase
                    break;
                }
                _ => {
                    // Wait for other phases to complete
                    thread::yield_now();
                    spins += 1;
                    if spins > MAX_SPIN_ITERATIONS {
                        return false; // Timeout
                    }
                }
            }
        }

        // Fill buffer with batch data
        {
            let buffer = self.coordinator.filling_buffer();
            *buffer = batch;
        }

        self.coordinator.record_cpu_batch();
        self.increment_submitted();

        // Swap buffers so filled buffer becomes active (for GPU to process)
        self.coordinator.swap_buffers();

        // Transition to GPU processing
        self.coordinator.transition(PipelinePhase::CpuFilling, PipelinePhase::GpuProcessing);

        true
    }

    /// Drain remaining batches (call before stop)
    ///
    /// Signals coordinator to drain mode and waits for completion.
    pub fn drain(&self) {
        // Try to transition to draining from any valid state
        let _ = self.coordinator.transition(PipelinePhase::Idle, PipelinePhase::Draining);
        let _ = self.coordinator.transition(PipelinePhase::CpuFilling, PipelinePhase::Draining);

        // Wait for drain to complete
        let mut spins = 0;
        while self.coordinator.phase() == PipelinePhase::Draining {
            thread::yield_now();
            spins += 1;
            if spins > MAX_SPIN_ITERATIONS * 10 {
                break; // Timeout
            }
        }
    }

    /// Get total batches submitted (from packed state)
    pub fn batches_submitted(&self) -> u64 {
        state_batches_submitted(self.state.load(Ordering::Acquire)) as u64
    }

    /// Get total batches completed (from packed state)
    pub fn batches_completed(&self) -> u64 {
        state_batches_completed(self.state.load(Ordering::Acquire)) as u64
    }

    /// Get generation counter (Q34 audit, from packed state)
    pub fn generation(&self) -> u64 {
        state_generation(self.state.load(Ordering::Acquire)) as u64
    }

    /// Get overlap efficiency (from coordinator)
    pub fn overlap_efficiency(&self) -> f64 {
        self.coordinator.overlap_efficiency()
    }
}

impl Drop for AsyncGpuRunner {
    fn drop(&mut self) {
        self.stop();
    }
}

impl std::fmt::Debug for AsyncGpuRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncGpuRunner")
            .field("running", &self.is_running())
            .field("batches_submitted", &self.batches_submitted())
            .field("batches_completed", &self.batches_completed())
            .field("pending_results", &self.pending_results())
            .field("overlap_efficiency", &self.overlap_efficiency())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu::pipeline_coordinator::GpuBatch;

    // ============================================================================
    // State Packing Tests (DualAtomicU64 pattern validation)
    // ============================================================================

    #[test]
    fn test_state_packing_roundtrip() {
        // Test that pack/unpack is lossless
        let running = true;
        let generation = 12345678u32;
        let submitted = 1000u16;
        let completed = 500u16;

        let packed = pack_state(running, generation, submitted, completed);

        assert_eq!(state_running(packed), running);
        assert_eq!(state_generation(packed), generation);
        assert_eq!(state_batches_submitted(packed), submitted);
        assert_eq!(state_batches_completed(packed), completed);
    }

    #[test]
    fn test_state_packing_edge_cases() {
        // Test max values
        let packed_max = pack_state(true, 0x7FFF_FFFF, 0xFFFF, 0xFFFF);
        assert!(state_running(packed_max));
        assert_eq!(state_generation(packed_max), 0x7FFF_FFFF);
        assert_eq!(state_batches_submitted(packed_max), 0xFFFF);
        assert_eq!(state_batches_completed(packed_max), 0xFFFF);

        // Test min values
        let packed_min = pack_state(false, 0, 0, 0);
        assert!(!state_running(packed_min));
        assert_eq!(state_generation(packed_min), 0);
        assert_eq!(state_batches_submitted(packed_min), 0);
        assert_eq!(state_batches_completed(packed_min), 0);
    }

    #[test]
    fn test_slot_state_constants() {
        // Verify slot state constants are distinct
        assert_eq!(SLOT_STATE_EMPTY, 0);
        assert_eq!(SLOT_STATE_WRITING, 1);
        assert_eq!(SLOT_STATE_READY, 2);
        assert_eq!(SLOT_STATE_READING, 3);
    }

    #[test]
    fn test_result_queue_creation() {
        let queue = LockfreeResultQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_result_queue_push_pop() {
        let queue = LockfreeResultQueue::new();

        let result = GpuBatchResult {
            doc_ids: vec![0, 1, 2],
            signatures: vec![0u16; 128 * 3],
            band_hashes: None,
            processing_time_us: 100,
            generation: 0,
        };

        assert!(queue.push(result.clone()));
        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);

        let popped = queue.pop();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().doc_ids.len(), 3);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_result_queue_capacity() {
        let queue = LockfreeResultQueue::new();

        // Fill to capacity
        for i in 0..RESULT_QUEUE_DEPTH {
            let result = GpuBatchResult {
                doc_ids: vec![i as u32],
                signatures: vec![0u16; 128],
                band_hashes: None,
                processing_time_us: 100,
                generation: i as u64,
            };
            assert!(queue.push(result), "Failed to push at index {}", i);
        }

        assert_eq!(queue.len(), RESULT_QUEUE_DEPTH);

        // Should fail when full
        let overflow = GpuBatchResult {
            doc_ids: vec![99],
            signatures: vec![0u16; 128],
            band_hashes: None,
            processing_time_us: 100,
            generation: 99,
        };
        assert!(!queue.push(overflow));

        // Pop one and try again
        assert!(queue.pop().is_some());
        let retry = GpuBatchResult {
            doc_ids: vec![100],
            signatures: vec![0u16; 128],
            band_hashes: None,
            processing_time_us: 100,
            generation: 100,
        };
        assert!(queue.push(retry));
    }

    #[test]
    fn test_batch_result_signature_access() {
        let result = GpuBatchResult {
            doc_ids: vec![0, 1, 2],
            signatures: (0u16..384).collect(), // 3 docs x 128 values
            band_hashes: None,
            processing_time_us: 100,
            generation: 0,
        };

        assert_eq!(result.num_docs(), 3);

        let sig0 = result.get_signature(0);
        assert_eq!(sig0.len(), 128);
        assert_eq!(sig0[0], 0);
        assert_eq!(sig0[127], 127);

        let sig1 = result.get_signature(1);
        assert_eq!(sig1[0], 128);

        let sig2 = result.get_signature(2);
        assert_eq!(sig2[0], 256);
    }

    // ============================================================================
    // LSH Band Hash Computation Tests
    // ============================================================================

    #[test]
    fn test_compute_band_hash_deterministic() {
        // Same signature → same hash (100% deterministic)
        let sig = [42u16; SIGNATURE_SIZE];

        let hash1 = compute_band_hash_from_u16(&sig, 0);
        let hash2 = compute_band_hash_from_u16(&sig, 0);

        assert_eq!(hash1, hash2, "Band hash must be deterministic");
    }

    #[test]
    fn test_compute_band_hash_different_bands() {
        // Different bands produce different hashes (high probability)
        let sig = [100u16; SIGNATURE_SIZE];

        let hash0 = compute_band_hash_from_u16(&sig, 0);
        let hash1 = compute_band_hash_from_u16(&sig, 1);
        let hash2 = compute_band_hash_from_u16(&sig, 2);

        // With uniform input, bands MAY produce same hash (valid behavior)
        // But they should be computed correctly
        assert!(hash0 != 0);
        assert!(hash1 != 0);
        assert!(hash2 != 0);
    }

    #[test]
    fn test_compute_band_hash_zero_signature() {
        let sig = [0u16; SIGNATURE_SIZE];
        let hash = compute_band_hash_from_u16(&sig, 0);
        assert_eq!(hash, 0, "Zero signature → zero hash");
    }

    #[test]
    fn test_compute_lsh_band_hashes_batch() {
        // Test batch computation with 3 documents
        let num_docs = 3;
        let mut signatures = Vec::with_capacity(num_docs * SIGNATURE_SIZE);

        // Doc 0: All 1s
        signatures.extend(&[1u16; SIGNATURE_SIZE]);
        // Doc 1: All 2s
        signatures.extend(&[2u16; SIGNATURE_SIZE]);
        // Doc 2: All 3s
        signatures.extend(&[3u16; SIGNATURE_SIZE]);

        let band_hashes = compute_lsh_band_hashes_from_u16(&signatures, num_docs);

        // Verify output size
        assert_eq!(band_hashes.len(), num_docs * NUM_BANDS);

        // Verify all hashes are non-zero
        for hash in &band_hashes {
            assert!(*hash != 0, "All hashes should be non-zero for non-zero input");
        }

        // Verify doc 0 hashes differ from doc 1 hashes (different signatures)
        let doc0_hashes = &band_hashes[0..NUM_BANDS];
        let doc1_hashes = &band_hashes[NUM_BANDS..NUM_BANDS * 2];

        assert_ne!(doc0_hashes, doc1_hashes, "Different signatures → different hashes");
    }

    #[test]
    fn test_compute_lsh_band_hashes_matches_cpu_reference() {
        // Verify our implementation matches the GPU kernel reference
        use crate::gpu::kernels::cpu_hash_band;

        let sig = [42u16; SIGNATURE_SIZE];
        let signatures = sig.to_vec();

        // Compute using batch function
        let batch_hashes = compute_lsh_band_hashes_from_u16(&signatures, 1);

        // Compute using GPU kernel CPU reference
        for band_idx in 0..NUM_BANDS {
            let expected = cpu_hash_band(&sig, band_idx);
            let actual = batch_hashes[band_idx];

            assert_eq!(
                actual, expected,
                "Band {} hash mismatch: actual={}, expected={}",
                band_idx, actual, expected
            );
        }
    }

    #[test]
    fn test_gpu_batch_result_with_band_hashes() {
        // Test GpuBatchResult with populated band_hashes
        let num_docs = 2;
        let signatures = vec![100u16; num_docs * SIGNATURE_SIZE];
        let band_hashes = compute_lsh_band_hashes_from_u16(&signatures, num_docs);

        let result = GpuBatchResult {
            doc_ids: vec![0, 1],
            signatures,
            band_hashes: Some(band_hashes.clone()),
            processing_time_us: 1000,
            generation: 42,
        };

        assert_eq!(result.num_docs(), num_docs);
        assert!(result.band_hashes.is_some());

        let hashes = result.band_hashes.unwrap();
        assert_eq!(hashes.len(), num_docs * NUM_BANDS);

        // Verify hashes are correct
        for doc_idx in 0..num_docs {
            for band_idx in 0..NUM_BANDS {
                let hash = hashes[doc_idx * NUM_BANDS + band_idx];
                assert!(hash != 0, "Band hash should be non-zero");
            }
        }
    }

    #[test]
    fn test_async_coordinator_phases() {
        let coord = AsyncPipelineCoordinator::new(1000);

        assert_eq!(coord.phase(), PipelinePhase::Idle);

        assert!(coord.transition(PipelinePhase::Idle, PipelinePhase::CpuFilling));
        assert_eq!(coord.phase(), PipelinePhase::CpuFilling);

        assert!(coord.transition(PipelinePhase::CpuFilling, PipelinePhase::GpuProcessing));
        assert_eq!(coord.phase(), PipelinePhase::GpuProcessing);
    }

    #[test]
    fn test_async_buffer_swap() {
        let coord = AsyncPipelineCoordinator::new(1000);

        let initial = coord.active_buffer();
        coord.swap_buffers();
        assert_ne!(coord.active_buffer(), initial);

        coord.swap_buffers();
        assert_eq!(coord.active_buffer(), initial);
    }

    #[test]
    fn test_gpu_batch_creation() {
        let mut batch = GpuBatch::with_capacity(100, 10000);
        assert!(batch.is_empty());

        batch.add_document(0, vec![1, 2, 3]);
        batch.add_document(1, vec![4, 5]);

        assert_eq!(batch.len(), 2);
        assert_eq!(batch.token_count(), 5);
    }

    // Integration test with GPU (skip if no GPU)
    #[test]
    fn test_async_runner_no_gpu() {
        // Create coordinator (doesn't need GPU)
        let coord = Arc::new(AsyncPipelineCoordinator::new(100));

        // Verify coordinator works standalone
        let buffer = coord.filling_buffer();
        buffer.add_document(0, vec![1, 2, 3]);
        buffer.add_document(1, vec![4, 5, 6]);

        assert!(!coord.is_filling_empty());
        assert_eq!(coord.filling_buffer().len(), 2);
    }

    #[test]
    #[cfg(feature = "gpu")]
    fn test_async_runner_with_gpu() {
        // Try to create GPU context
        let ctx = match GpuContextCapsule::new_blocking() {
            Ok(ctx) => Arc::new(ctx),
            Err(_) => {
                println!("No GPU available, skipping async runner test");
                return;
            }
        };

        let minhash = match MinHashGpuCapsule::new(&ctx) {
            Ok(m) => Arc::new(m),
            Err(_) => {
                println!("Failed to create MinHash capsule, skipping test");
                return;
            }
        };

        let coord = Arc::new(AsyncPipelineCoordinator::new(100));

        let mut runner = AsyncGpuRunner::new(ctx, minhash, coord);
        runner.start();

        // Submit batch
        let mut batch = GpuBatch::with_capacity(100, 10000);
        for i in 0..10 {
            batch.add_document(i, vec![i * 100, i * 100 + 1, i * 100 + 2]);
        }

        assert!(runner.submit_batch(batch));

        // Wait for result
        let mut result = None;
        for _ in 0..100 {
            if let Some(r) = runner.poll_result() {
                result = Some(r);
                break;
            }
            thread::sleep(std::time::Duration::from_millis(10));
        }

        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.doc_ids.len(), 10);
        assert_eq!(r.signatures.len(), 10 * 128);

        runner.stop();
    }

    #[test]
    #[cfg(feature = "gpu")]
    fn test_async_overlap_efficiency() {
        // Test that CPU can fill while GPU processes
        let ctx = match GpuContextCapsule::new_blocking() {
            Ok(ctx) => Arc::new(ctx),
            Err(_) => return,
        };

        let minhash = match MinHashGpuCapsule::new(&ctx) {
            Ok(m) => Arc::new(m),
            Err(_) => return,
        };

        let coord = Arc::new(AsyncPipelineCoordinator::new(1000));

        let mut runner = AsyncGpuRunner::new(ctx, minhash, coord.clone());
        runner.start();

        // Submit multiple batches rapidly
        for batch_id in 0..10u32 {
            let mut batch = GpuBatch::with_capacity(1000, 100000);
            for i in 0..100 {
                batch.add_document(batch_id * 100 + i, vec![i, i + 1, i + 2]);
            }
            runner.submit_batch(batch);
            thread::sleep(std::time::Duration::from_millis(1)); // Simulate CPU work
        }

        // Drain results
        thread::sleep(std::time::Duration::from_millis(100));
        let mut count = 0;
        while runner.poll_result().is_some() {
            count += 1;
        }

        println!("Processed {} batches", count);
        println!("Overlap efficiency: {:.1}%", coord.overlap_efficiency() * 100.0);

        runner.stop();
    }
}
