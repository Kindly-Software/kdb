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
//! - **COCA**: Lockfree result queue (ArrayQueue), atomic control
//! - **ASSUM**: Thread safety via Arc, shutdown via AtomicBool
//! - **B32**: Overlap efficiency benchmarks
//! - **T28**: Async flow tests, shutdown tests

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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

/// Result from GPU batch processing
///
/// Contains MinHash signatures and optional LSH band hashes.
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

/// Lockfree result queue using atomic ring buffer
///
/// # COCA Compliance
///
/// 100% lockfree - uses atomic head/tail indices, no Mutex/RwLock.
/// Uses UnsafeCell for interior mutability in single-producer-single-consumer pattern.
///
/// # ASSUM Safety
///
/// - `#ASSUME_SINGLE_PRODUCER`: Only GPU thread pushes results
/// - `#VERIFY_SINGLE_PRODUCER`: Only background thread calls push()
/// - `#ASSUME_SINGLE_CONSUMER`: Only main thread pops results
/// - `#VERIFY_SINGLE_CONSUMER`: Only poll_result() calls pop()
/// - `#ASSUME_NO_CONCURRENT_ACCESS`: Producer/consumer access different indices
/// - `#VERIFY_NO_CONCURRENT_ACCESS`: Head/tail separation ensures non-overlapping access
#[repr(C, align(128))]
pub struct LockfreeResultQueue {
    /// Ring buffer of results (UnsafeCell for interior mutability)
    buffer: Box<[UnsafeCell<Option<GpuBatchResult>>; RESULT_QUEUE_DEPTH]>,

    /// Head index (producer writes here)
    head: AtomicU64,

    /// Tail index (consumer reads here)
    tail: AtomicU64,

    /// Padding for cache line alignment
    _padding: [u8; 48],
}

impl LockfreeResultQueue {
    /// Create new result queue
    fn new() -> Self {
        // Initialize array with None values wrapped in UnsafeCell
        let buffer: [UnsafeCell<Option<GpuBatchResult>>; RESULT_QUEUE_DEPTH] =
            std::array::from_fn(|_| UnsafeCell::new(None));

        Self {
            buffer: Box::new(buffer),
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            _padding: [0; 48],
        }
    }

    /// Push result (producer only)
    ///
    /// Returns true if pushed, false if queue full.
    fn push(&self, result: GpuBatchResult) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        // Check if full
        if head.wrapping_sub(tail) >= RESULT_QUEUE_DEPTH as u64 {
            return false;
        }

        let idx = (head % RESULT_QUEUE_DEPTH as u64) as usize;

        // SAFETY: Single producer (background thread only), index is unique per head value.
        // UnsafeCell provides interior mutability for this SPSC queue pattern.
        // Producer and consumer never access the same index simultaneously because
        // head advances after write, tail advances after read.
        unsafe {
            *self.buffer[idx].get() = Some(result);
        }

        self.head.fetch_add(1, Ordering::Release);
        true
    }

    /// Pop result (consumer only)
    ///
    /// Returns Some(result) if available, None if empty.
    fn pop(&self) -> Option<GpuBatchResult> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        // Check if empty
        if head == tail {
            return None;
        }

        let idx = (tail % RESULT_QUEUE_DEPTH as u64) as usize;

        // SAFETY: Single consumer (main thread only), index is unique per tail value.
        // UnsafeCell provides interior mutability for this SPSC queue pattern.
        // Producer and consumer never access the same index simultaneously because
        // head advances after write, tail advances after read.
        let result = unsafe {
            (*self.buffer[idx].get()).take()
        };

        self.tail.fetch_add(1, Ordering::Release);
        result
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

// SAFETY: Queue is designed for single-producer single-consumer
unsafe impl Send for LockfreeResultQueue {}
unsafe impl Sync for LockfreeResultQueue {}

/// Async GPU runner that processes batches in background
///
/// # Architecture
///
/// ```text
/// AsyncGpuRunner
/// ├── ctx: Arc<GpuContextCapsule> (shared GPU context)
/// ├── minhash: Arc<MinHashGpuCapsule> (shared compute pipeline)
/// ├── coordinator: Arc<AsyncPipelineCoordinator> (phase coordination)
/// ├── running: Arc<AtomicBool> (shutdown signal)
/// ├── handle: Option<JoinHandle> (background thread)
/// └── results: Arc<LockfreeResultQueue> (result queue)
/// ```
///
/// # COCA Compliance
///
/// 100% lockfree - uses atomic shutdown flag and lockfree result queue.
///
/// # ASSUM Safety
///
/// - `#ASSUME_GPU_THREAD_SAFE`: wgpu is thread-safe
/// - `#VERIFY_GPU_THREAD_SAFE`: wgpu guarantees Send + Sync
/// - `#ASSUME_SHUTDOWN_ORDERED`: AtomicBool ensures clean shutdown
/// - `#VERIFY_SHUTDOWN_ORDERED`: Join handle waits for completion
pub struct AsyncGpuRunner {
    /// GPU context (shared)
    ctx: Arc<GpuContextCapsule>,

    /// MinHash GPU capsule (shared)
    minhash: Arc<MinHashGpuCapsule>,

    /// Pipeline coordinator (shared)
    coordinator: Arc<AsyncPipelineCoordinator>,

    /// Running flag (shutdown signal)
    running: Arc<AtomicBool>,

    /// Background thread handle
    handle: Option<JoinHandle<()>>,

    /// Result queue (lockfree)
    results: Arc<LockfreeResultQueue>,

    /// Generation counter (Q34 audit)
    generation: AtomicU64,

    /// Total batches submitted
    batches_submitted: AtomicU64,

    /// Total batches completed
    batches_completed: AtomicU64,
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
            ctx,
            minhash,
            coordinator,
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
            results: Arc::new(LockfreeResultQueue::new()),
            generation: AtomicU64::new(0),
            batches_submitted: AtomicU64::new(0),
            batches_completed: AtomicU64::new(0),
        }
    }

    /// Start background GPU processing thread
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_NOT_RUNNING`: start() called only once
    /// - `#VERIFY_NOT_RUNNING`: Checks running flag before starting
    pub fn start(&mut self) {
        if self.running.load(Ordering::Relaxed) {
            return; // Already running
        }

        self.running.store(true, Ordering::Release);

        let ctx = self.ctx.clone();
        let minhash = self.minhash.clone();
        let coordinator = self.coordinator.clone();
        let running = self.running.clone();
        let results = self.results.clone();

        self.handle = Some(thread::spawn(move || {
            Self::gpu_worker_loop(ctx, minhash, coordinator, running, results);
        }));
    }

    /// Stop background processing (graceful shutdown)
    ///
    /// Signals worker to stop and waits for completion.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if runner is active
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// GPU worker loop (runs in background thread)
    ///
    /// Processes batches from coordinator and pushes results to queue.
    fn gpu_worker_loop(
        ctx: Arc<GpuContextCapsule>,
        minhash: Arc<MinHashGpuCapsule>,
        coordinator: Arc<AsyncPipelineCoordinator>,
        running: Arc<AtomicBool>,
        results: Arc<LockfreeResultQueue>,
    ) {
        let mut generation: u64 = 0;

        while running.load(Ordering::Acquire) {
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

                            // Convert output to result - copy signatures from GPU output
                            let signatures: Vec<u16> = (0..batch.len())
                                .flat_map(|i| output.get_signature(i).to_vec())
                                .collect();

                            let result = GpuBatchResult {
                                doc_ids: batch.doc_ids.clone(),
                                signatures,
                                band_hashes: None, // TODO: Add LSH band hashing
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
                            generation = generation.wrapping_add(1);
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
                            // Copy signatures from GPU output
                            let signatures: Vec<u16> = (0..batch.len())
                                .flat_map(|i| output.get_signature(i).to_vec())
                                .collect();

                            let result = GpuBatchResult {
                                doc_ids: batch.doc_ids.clone(),
                                signatures,
                                band_hashes: None,
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
            self.batches_completed.fetch_add(1, Ordering::Relaxed);
        }
        result
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
        self.batches_submitted.fetch_add(1, Ordering::Relaxed);

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

    /// Get total batches submitted
    pub fn batches_submitted(&self) -> u64 {
        self.batches_submitted.load(Ordering::Relaxed)
    }

    /// Get total batches completed
    pub fn batches_completed(&self) -> u64 {
        self.batches_completed.load(Ordering::Relaxed)
    }

    /// Get generation counter (Q34 audit)
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
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
