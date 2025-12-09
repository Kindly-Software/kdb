//! HybridDedupPipeline - T7 Heterogeneous Tier (CPU-GPU Hybrid)
//!
//! Combines CPU tokenization with GPU MinHash/LSH for maximum throughput.
//!
//! # Architecture
//!
//! ```text
//! CPU Stage 1: Tokenization (sequential, <10µs/doc)
//!     |
//!     v [Double Buffer]
//! GPU Stage: MinHash + LSH (parallel, 500K-2M docs/sec)
//!     |
//!     v [Candidate Pairs]
//! CPU Stage 2: Union-Find (sequential, O(α(n)))
//! ```
//!
//! # Pipeline Modes
//!
//! - **Auto**: Detect GPU availability, use GPU if ≥2× speedup expected
//! - **GpuAccelerated**: Force GPU (fail if unavailable)
//! - **CpuOnly**: Use CPU SIMD (for testing, CI, or weak GPUs)
//!
//! # Performance Targets (B32 Framework)
//!
//! | Hardware | CPU Baseline | GPU Target | Speedup |
//! |----------|--------------|------------|---------|
//! | iGPU (Ryzen) | 73.4K docs/sec | 150K docs/sec | 2× |
//! | GTX 1650 | 73.4K docs/sec | 300K docs/sec | 4× |
//! | RTX 3060 | 73.4K docs/sec | 500K docs/sec | 7× |
//! | RTX 4090 | 73.4K docs/sec | 1M docs/sec | 14× |
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier (CPU+GPU coordination)
//! - **Chaos**: 100% lockfree via AtomicU64 state
//! - **ASSUM**: GPU availability runtime-checked, graceful fallback
//! - **B32**: Fair benchmarking (vs CPU SIMD baseline)
//! - **T28**: GPU correctness tests (GPU == CPU within tolerance)
//! - **I20**: Same API as DedupPipeline (drop-in replacement)

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;

use atomic_capsule::CpuCapabilityCapsule;
use atomic_capsule::probabilistic::{tokenize, MinHashSignatureCapsule, UnionFind};

#[cfg(feature = "gpu")]
use crate::gpu::{
    GpuContextCapsule, GpuContextState, GpuCapabilities,
    MinHashGpuCapsule, MinHashGpuInput, MinHashGpuOutput,
    LshBandGpuCapsule, LshBandGpuInput, LshBandGpuOutput,
    DoubleBuffer, GpuBatch, BatchCoordinator,
    GpuError, GpuResult,
    NUM_BANDS,
    mmap_bucket_storage::MmapBucketStorage,
    mmap_signature_storage::MmapSignatureStorage,
    // Phase 2-3 GPU safety capsules (Wave 1.1 integration)
    GpuPipelineMetacapsule, GpuPipelineSnapshot,
    MemoryPressureLevel,
};

#[cfg(feature = "gpu-async")]
use crate::gpu::{
    AsyncPipelineCoordinator, AsyncGpuRunner, GpuBatchResult,
    PipelinePhase as AsyncPhase,
};

use crate::PipelineError;

/// Document ID type
pub type DocId = u32;

/// Pipeline execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineMode {
    /// Automatically detect GPU and use if beneficial
    Auto,
    /// Force GPU acceleration (error if unavailable)
    GpuAccelerated,
    /// Force CPU-only processing (for testing/CI)
    CpuOnly,
}

impl Default for PipelineMode {
    fn default() -> Self {
        PipelineMode::Auto
    }
}

/// Pipeline state (packed in AtomicU64)
///
/// Bit layout:
/// - Bits 0-7: Phase (0=Idle, 1=Tokenizing, 2=Computing, 3=Clustering, 4=Complete)
/// - Bits 8-15: Error code (0=None)
/// - Bits 16-31: Documents processed (count)
/// - Bits 32-63: Generation counter (Q34 audit)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelinePhase {
    /// Pipeline idle, ready for documents
    Idle = 0,
    /// Tokenizing documents (CPU)
    Tokenizing = 1,
    /// Computing MinHash/LSH (GPU or CPU)
    Computing = 2,
    /// Clustering with Union-Find (CPU)
    Clustering = 3,
    /// Processing complete
    Complete = 4,
    /// Error state
    Error = 5,
}

impl From<u64> for PipelinePhase {
    fn from(v: u64) -> Self {
        match v & 0xFF {
            0 => PipelinePhase::Idle,
            1 => PipelinePhase::Tokenizing,
            2 => PipelinePhase::Computing,
            3 => PipelinePhase::Clustering,
            4 => PipelinePhase::Complete,
            _ => PipelinePhase::Error,
        }
    }
}

/// Pack pipeline state into u64
fn pack_state(phase: PipelinePhase, error_code: u8, docs_processed: u16, generation: u32) -> u64 {
    (phase as u64)
        | ((error_code as u64) << 8)
        | ((docs_processed as u64) << 16)
        | ((generation as u64) << 32)
}

/// Unpack pipeline state from u64
fn unpack_state(packed: u64) -> (PipelinePhase, u8, u16, u32) {
    let phase = PipelinePhase::from(packed);
    let error_code = ((packed >> 8) & 0xFF) as u8;
    let docs_processed = ((packed >> 16) & 0xFFFF) as u16;
    let generation = ((packed >> 32) & 0xFFFFFFFF) as u32;
    (phase, error_code, docs_processed, generation)
}

/// Hybrid pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct HybridPipelineStats {
    /// Documents processed
    pub docs_processed: u64,
    /// Documents processed via GPU
    pub gpu_docs: u64,
    /// Documents processed via CPU
    pub cpu_docs: u64,
    /// Batches submitted to GPU
    pub gpu_batches: u64,
    /// Total tokenization time (us)
    pub tokenization_us: u64,
    /// Total GPU compute time (us)
    pub gpu_compute_us: u64,
    /// Total LSH band hashing time (us)
    pub lsh_band_us: u64,
    /// Total clustering time (us)
    pub clustering_us: u64,
    /// Duplicate pairs found
    pub duplicate_pairs: u64,
    /// Candidate pairs generated by LSH
    pub lsh_candidates: u64,
    /// Clusters formed
    pub clusters: u64,
}

/// HybridDedupPipeline - GPU-accelerated deduplication
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
/// use atomic_capsule::CpuCapabilityCapsule;
///
/// let cpu_caps = CpuCapabilityCapsule::detect();
/// let mut pipeline = HybridDedupPipeline::new(10_000, PipelineMode::Auto, &cpu_caps)?;
///
/// // Add documents
/// for (id, text) in documents {
///     pipeline.add_document(id, text)?;
/// }
///
/// // Find duplicates
/// let clusters = pipeline.find_duplicates(0.85)?;
/// println!("Found {} duplicate clusters", clusters.len());
/// ```
#[repr(C, align(128))]
pub struct HybridDedupPipeline {
    /// Atomic state (phase, error, docs_processed, generation)
    state: AtomicU64,

    /// Pipeline mode
    mode: PipelineMode,

    /// Using GPU acceleration
    using_gpu: bool,

    /// GPU context (if available)
    #[cfg(feature = "gpu")]
    gpu_context: Option<Arc<GpuContextCapsule>>,

    /// GPU MinHash capsule (if using GPU)
    #[cfg(feature = "gpu")]
    minhash_gpu: Option<MinHashGpuCapsule>,

    /// GPU LSH band capsule (if using GPU)
    #[cfg(feature = "gpu")]
    lsh_band_gpu: Option<LshBandGpuCapsule>,

    /// Batch coordinator (GPU double buffering)
    #[cfg(feature = "gpu")]
    batch_coordinator: Option<BatchCoordinator>,

    /// Signature storage: Mmap-backed O(1) memory (GPU mode)
    #[cfg(feature = "gpu")]
    signature_storage: Option<MmapSignatureStorage>,

    /// In-memory fallback for tests (when mmap is disabled) or CPU-only mode
    #[cfg(feature = "gpu")]
    signatures_fallback: Vec<Option<MinHashSignatureCapsule>>,

    /// CPU fallback: In-memory signatures for non-GPU mode
    #[cfg(not(feature = "gpu"))]
    signatures: Vec<Option<MinHashSignatureCapsule>>,

    /// LSH bucket storage: mmap-backed O(1) memory
    /// Used for efficient candidate pair generation (GPU path only)
    #[cfg(feature = "gpu")]
    lsh_bucket_storage: Option<MmapBucketStorage>,

    /// GPU Pipeline Metacapsule - T6 Mixed tier orchestrator (Wave 1.1)
    ///
    /// Coordinates GPU safety capsules for robust lifecycle management:
    /// - GpuStateMachineCapsule: 6-state lifecycle (Uninitialized -> Ready -> Processing)
    /// - GpuHealthCapsule: 6 capability flags for health monitoring
    /// - MemoryPressureCapsule: VMA-style memory budget enforcement
    /// - GpuFallbackManager: Circuit breaker pattern for CPU/GPU switching
    ///
    /// # ASSUM: Metacapsule provides unified GPU decision-making
    /// #ASSUME_METACAPSULE_LOCKFREE: GpuPipelineMetacapsule is 100% lockfree (512B, 8 cache lines)
    /// #VERIFY_METACAPSULE_LOCKFREE: All sub-capsules use AtomicU64, no mutex
    /// #ASSUME_METACAPSULE_THREADSAFE: Metacapsule is Send+Sync via atomic operations
    /// #VERIFY_METACAPSULE_THREADSAFE: Verified in gpu/pipeline_metacapsule.rs tests
    #[cfg(feature = "gpu")]
    gpu_metacapsule: GpuPipelineMetacapsule,

    /// Union-Find for clustering
    union_find: UnionFind,

    /// Target batch size (documents per GPU batch)
    batch_size: usize,

    /// Maximum tokens per batch
    max_tokens_per_batch: usize,

    /// Total document capacity
    capacity: usize,

    /// Statistics
    stats: HybridPipelineStats,

    /// Memory budget for O(1) enforcement (optional)
    memory_budget: Option<crate::adaptive::MemoryBudgetCapsule>,

    /// Async mode enabled
    #[cfg(feature = "gpu-async")]
    async_enabled: AtomicBool,

    /// Async pipeline coordinator
    #[cfg(feature = "gpu-async")]
    async_coordinator: Option<Arc<AsyncPipelineCoordinator>>,

    /// Async GPU runner
    #[cfg(feature = "gpu-async")]
    async_runner: Option<AsyncGpuRunner>,

    /// Pending async results to process
    #[cfg(feature = "gpu-async")]
    pending_results: Vec<GpuBatchResult>,

    /// Padding for cache alignment
    _padding: [u8; 96],
}

impl HybridDedupPipeline {
    /// Create new hybrid pipeline
    ///
    /// # Arguments
    ///
    /// - `capacity`: Expected document count
    /// - `mode`: Pipeline execution mode
    /// - `cpu_caps`: CPU capabilities for SIMD dispatch
    ///
    /// # Returns
    ///
    /// - `Ok(HybridDedupPipeline)`: Pipeline ready for use
    /// - `Err(PipelineError)`: GPU required but unavailable (GpuAccelerated mode)
    pub fn new(
        capacity: usize,
        mode: PipelineMode,
        cpu_caps: &CpuCapabilityCapsule,
    ) -> Result<Self, PipelineError> {
        let mut pipeline = Self {
            state: AtomicU64::new(pack_state(PipelinePhase::Idle, 0, 0, 0)),
            mode,
            using_gpu: false,
            #[cfg(feature = "gpu")]
            gpu_context: None,
            #[cfg(feature = "gpu")]
            minhash_gpu: None,
            #[cfg(feature = "gpu")]
            lsh_band_gpu: None,
            #[cfg(feature = "gpu")]
            batch_coordinator: None,
            #[cfg(feature = "gpu")]
            signature_storage: {
                // Skip mmap in tests to avoid SIGBUS from temp directory restrictions
                if cfg!(test) {
                    None
                } else {
                    let path = std::env::temp_dir().join(format!("kindly_dedup_sig_{}.mmap", std::process::id()));
                    // capacity signatures × 260 bytes = O(capacity) file, O(1) resident memory
                    // Example: 16M × 260 = 4.16 GB file, ~200 MB resident (OS mmap paging)
                    MmapSignatureStorage::create(&path, capacity as u32).ok()
                }
            },
            #[cfg(feature = "gpu")]
            signatures_fallback: vec![None; capacity],
            #[cfg(not(feature = "gpu"))]
            signatures: vec![None; capacity],
            // LSH bucket storage: deferred until GPU is initialized (O(1) memory fix)
            // Only GPU path needs mmap LSH buckets, CPU path uses different algorithm
            #[cfg(feature = "gpu")]
            lsh_bucket_storage: None,
            // GPU Pipeline Metacapsule - T6 Mixed tier orchestrator (Wave 1.1)
            // Initialized with default 8 GB VRAM assumption (updated when GPU detected)
            //
            // #ASSUME_VRAM_DEFAULT: 8 GB default is safe for most discrete GPUs
            // #VERIFY_VRAM_DEFAULT: Updated via update_memory_usage() after GPU init
            #[cfg(feature = "gpu")]
            gpu_metacapsule: GpuPipelineMetacapsule::new(),
            union_find: UnionFind::new(capacity),
            batch_size: 10_000,
            max_tokens_per_batch: 1_000_000,
            capacity,
            stats: HybridPipelineStats::default(),
            memory_budget: None,
            #[cfg(feature = "gpu-async")]
            async_enabled: AtomicBool::new(false),
            #[cfg(feature = "gpu-async")]
            async_coordinator: None,
            #[cfg(feature = "gpu-async")]
            async_runner: None,
            #[cfg(feature = "gpu-async")]
            pending_results: Vec::new(),
            _padding: [0; 96],
        };

        // Initialize GPU based on mode
        #[cfg(feature = "gpu")]
        match mode {
            PipelineMode::Auto => {
                pipeline.try_init_gpu();
            }
            PipelineMode::GpuAccelerated => {
                if !pipeline.try_init_gpu() {
                    return Err(PipelineError::ResourceLimitExceeded {
                        reason: "GPU acceleration required but no GPU available".to_string(),
                    });
                }
            }
            PipelineMode::CpuOnly => {
                // Don't initialize GPU
            }
        }

        #[cfg(not(feature = "gpu"))]
        if mode == PipelineMode::GpuAccelerated {
            return Err(PipelineError::ResourceLimitExceeded {
                reason: "GPU feature not compiled".to_string(),
            });
        }

        Ok(pipeline)
    }

    /// Create hybrid pipeline with memory budget enforcement
    ///
    /// # Arguments
    ///
    /// - `capacity`: Expected document count
    /// - `mode`: Pipeline execution mode
    /// - `cpu_caps`: CPU capabilities for SIMD dispatch
    /// - `memory_budget`: Memory budget capsule for O(1) enforcement
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
    /// use kindly_dedup::adaptive::MemoryBudgetCapsule;
    /// use atomic_capsule::CpuCapabilityCapsule;
    ///
    /// let cpu_caps = CpuCapabilityCapsule::detect();
    /// let budget = MemoryBudgetCapsule::new_gb(1); // 1 GB limit
    /// let pipeline = HybridDedupPipeline::with_memory_budget(
    ///     10_000_000,
    ///     PipelineMode::Auto,
    ///     &cpu_caps,
    ///     budget,
    /// )?;
    /// ```
    pub fn with_memory_budget(
        capacity: usize,
        mode: PipelineMode,
        cpu_caps: &CpuCapabilityCapsule,
        memory_budget: crate::adaptive::MemoryBudgetCapsule,
    ) -> Result<Self, PipelineError> {
        // Use new() but store memory_budget for later validation
        let mut pipeline = Self::new(capacity, mode, cpu_caps)?;
        pipeline.memory_budget = Some(memory_budget);
        Ok(pipeline)
    }

    /// Try to initialize GPU context
    ///
    /// Returns true if GPU is available and worth using.
    #[cfg(feature = "gpu")]
    fn try_init_gpu(&mut self) -> bool {
        // Try to create GPU context
        let mut ctx = match GpuContextCapsule::new_blocking() {
            Ok(ctx) => ctx,
            Err(_) => return false,
        };

        // Check if GPU is worth using
        if !ctx.capabilities().worth_using() {
            return false;
        }

        // Get recommended batch size
        self.batch_size = ctx.capabilities().recommended_batch_size().min(100_000);

        // Initialize FED hash parameters for 6-24× speedup
        // Use PID + timestamp for high-entropy seed
        //
        // #ASSUME_CLOCK_RELIABILITY: System clock is typically reliable (>99.99% of systems).
        // Fallback: If clock is before UNIX epoch (virtualized/misconfigured environments),
        // use Duration::ZERO. Seed degrades to (PID << 32), still unique per-process.
        // This is acceptable for non-cryptographic seeding (MinHash parameters).
        // #VERIFY_FALLBACK: Tested in CI with clock edge cases. See T28 test coverage.
        let seed = (std::process::id() as u64) << 32
            | std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::ZERO)
                .as_nanos() as u64;

        if let Err(e) = ctx.init_fed_params(seed) {
            eprintln!("Warning: Failed to initialize FED params: {}. Falling back to legacy MinHash.", e);
        }

        // Create MinHash GPU capsule with FED optimization
        // Fallback to legacy if FED fails
        let minhash = match MinHashGpuCapsule::new_fed(&ctx) {
            Ok(m) => {
                println!("GPU MinHash: FED optimization enabled (6-24× speedup expected)");
                m
            }
            Err(e) => {
                eprintln!("Warning: FED MinHash failed: {}. Using legacy MinHash.", e);
                match MinHashGpuCapsule::new(&ctx) {
                    Ok(m) => m,
                    Err(_) => return false,
                }
            }
        };

        // Create LSH Band GPU capsule
        let lsh_band = match LshBandGpuCapsule::new(&ctx) {
            Ok(l) => l,
            Err(_) => return false,
        };

        // Create batch coordinator
        let coordinator = BatchCoordinator::new(self.batch_size, self.max_tokens_per_batch);

        // Store GPU resources
        self.gpu_context = Some(Arc::new(ctx));
        self.minhash_gpu = Some(minhash);
        self.lsh_band_gpu = Some(lsh_band);
        self.batch_coordinator = Some(coordinator);
        self.using_gpu = true;

        // Initialize GPU Pipeline Metacapsule (Wave 1.1)
        //
        // #ASSUME_METACAPSULE_INIT: Initialization succeeds if GPU context is valid
        // #VERIFY_METACAPSULE_INIT: GpuPipelineMetacapsule::initialize() validates state transitions
        if let Err(e) = self.gpu_metacapsule.initialize() {
            eprintln!("Warning: GPU metacapsule initialization failed: {}. GPU safety monitoring degraded.", e);
            // Continue anyway - the metacapsule will report unhealthy state
            // but we can still use GPU for compute (graceful degradation)
        }

        // Update batch size from metacapsule recommendation (respects memory pressure)
        //
        // #ASSUME_BATCH_SIZE_REASONABLE: Metacapsule returns sensible batch size (1K-100K)
        // #VERIFY_BATCH_SIZE_REASONABLE: Bounded by MIN_BATCH_SIZE and hardware caps in metacapsule
        let metacapsule_batch_size = self.gpu_metacapsule.get_recommended_batch_size();
        if metacapsule_batch_size > 0 && metacapsule_batch_size <= 100_000 {
            self.batch_size = self.batch_size.min(metacapsule_batch_size);
        }

        true
    }

    #[cfg(not(feature = "gpu"))]
    fn try_init_gpu(&mut self) -> bool {
        false
    }

    /// Check if pipeline is using GPU
    pub fn is_using_gpu(&self) -> bool {
        self.using_gpu
    }

    /// Get current pipeline phase
    pub fn phase(&self) -> PipelinePhase {
        let packed = self.state.load(Ordering::Acquire);
        PipelinePhase::from(packed)
    }

    /// Get pipeline statistics
    pub fn stats(&self) -> &HybridPipelineStats {
        &self.stats
    }

    /// Get GPU capabilities (if using GPU)
    #[cfg(feature = "gpu")]
    pub fn gpu_capabilities(&self) -> Option<&GpuCapabilities> {
        self.gpu_context.as_ref().map(|ctx| ctx.capabilities())
    }

    /// Get GPU pipeline metacapsule snapshot (Wave 1.1)
    ///
    /// Returns an atomic snapshot of the GPU pipeline orchestrator state including:
    /// - GPU lifecycle state (Uninitialized/Ready/Processing/etc.)
    /// - Health flags (6 capability flags)
    /// - Memory pressure level
    /// - Circuit breaker state
    /// - Recommended batch size
    ///
    /// # Performance
    ///
    /// - Latency: <100ns (6 atomic loads + packing)
    /// - Throughput: 10M+ snapshots/sec
    ///
    /// # Q34 Audit Trail
    ///
    /// Includes generation counter for tamper-evident audit logging.
    #[cfg(feature = "gpu")]
    pub fn gpu_pipeline_snapshot(&self) -> GpuPipelineSnapshot {
        self.gpu_metacapsule.snapshot()
    }

    /// Check if GPU pipeline is fully healthy (Wave 1.1)
    ///
    /// Returns true if ALL of:
    /// - State machine is Ready
    /// - All 6 health flags are OK
    /// - Memory pressure below Critical
    /// - Circuit breaker is Closed
    #[cfg(feature = "gpu")]
    pub fn is_gpu_pipeline_healthy(&self) -> bool {
        self.gpu_metacapsule.is_gpu_healthy()
    }

    /// Update GPU memory usage for pressure tracking (Wave 1.1)
    ///
    /// Call this periodically during GPU operations to update memory pressure.
    /// The metacapsule will automatically adjust batch sizes and may trigger
    /// CPU fallback if pressure becomes Critical.
    ///
    /// # Arguments
    ///
    /// - `used_bytes`: Current GPU VRAM usage in bytes
    ///
    /// # Returns
    ///
    /// The new memory pressure level.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // After GPU buffer allocation
    /// let level = pipeline.update_gpu_memory_usage(1 * 1024 * 1024 * 1024); // 1 GB
    /// if level >= MemoryPressureLevel::High {
    ///     println!("Warning: GPU memory pressure is high");
    /// }
    /// ```
    #[cfg(feature = "gpu")]
    pub fn update_gpu_memory_usage(&self, used_bytes: u64) -> MemoryPressureLevel {
        self.gpu_metacapsule.update_memory_usage(used_bytes)
    }

    /// Force CPU-only mode via metacapsule (Wave 1.1)
    ///
    /// Disables GPU even if available. Useful for testing CPU fallback
    /// or when GPU is causing issues.
    #[cfg(feature = "gpu")]
    pub fn force_cpu_mode(&self) {
        self.gpu_metacapsule.force_cpu_mode();
    }

    /// Clear forced CPU mode (Wave 1.1)
    ///
    /// Re-enables GPU if available and healthy.
    #[cfg(feature = "gpu")]
    pub fn clear_force_cpu(&self) {
        self.gpu_metacapsule.clear_force_cpu();
    }

    /// Add document to pipeline
    ///
    /// # Arguments
    ///
    /// - `doc_id`: Document identifier
    /// - `text`: Document text
    ///
    /// # Returns
    ///
    /// - `Ok(())`: Document added
    /// - `Err(PipelineError)`: Document ID out of bounds or GPU error
    pub fn add_document(&mut self, doc_id: DocId, text: &str) -> Result<(), PipelineError> {
        let doc_idx = doc_id as usize;

        // Bounds check
        if doc_idx >= self.capacity {
            return Err(PipelineError::DocumentIdOutOfBounds {
                doc_id: doc_idx,
                capacity: self.capacity,
            });
        }

        // Update state to Tokenizing
        self.set_phase(PipelinePhase::Tokenizing);

        // Tokenize document
        let tokens = tokenize(text);
        // Convert Vec<String> to Vec<&str> for API compatibility
        let tokens_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();

        // Choose path based on mode and metacapsule health (Wave 1.1)
        //
        // Decision logic:
        // 1. Check metacapsule.should_use_gpu() which combines:
        //    - GpuStateMachine: Ready/Processing state
        //    - GpuHealth: DEVICE_AVAILABLE | COMPUTE_OK flags
        //    - MemoryPressure: Below Critical level
        //    - FallbackManager: Circuit breaker allows GPU
        // 2. Fall back to CPU if any safety check fails
        //
        // #ASSUME_METACAPSULE_FAST: should_use_gpu() < 100ns (4 atomic loads)
        // #VERIFY_METACAPSULE_FAST: Verified via B32 benchmarks in pipeline_metacapsule.rs
        #[cfg(feature = "gpu-async")]
        if self.async_enabled.load(Ordering::Acquire) {
            self.add_document_async(doc_id, &tokens_refs)?;
            // Poll for results periodically (every 100 docs)
            if self.stats.docs_processed % 100 == 0 {
                self.poll_async_results();
            }
        } else if self.using_gpu && self.gpu_metacapsule.should_use_gpu() {
            #[cfg(feature = "gpu")]
            {
                self.add_document_gpu(doc_id, &tokens_refs)?;
            }
        } else {
            self.add_document_cpu(doc_idx, &tokens_refs);
        }

        #[cfg(not(feature = "gpu-async"))]
        {
            // Check both using_gpu flag AND metacapsule safety (Wave 1.1)
            #[cfg(feature = "gpu")]
            let use_gpu_path = self.using_gpu && self.gpu_metacapsule.should_use_gpu();
            #[cfg(not(feature = "gpu"))]
            let use_gpu_path = false;

            if use_gpu_path {
                #[cfg(feature = "gpu")]
                {
                    self.add_document_gpu(doc_id, &tokens_refs)?;
                }
            } else {
                self.add_document_cpu(doc_idx, &tokens_refs);
            }
        }

        // Update stats (based on actual path taken, not just using_gpu flag)
        self.stats.docs_processed += 1;
        #[cfg(feature = "gpu")]
        {
            // Track actual GPU usage based on metacapsule decision (Wave 1.1)
            if self.using_gpu && self.gpu_metacapsule.should_use_gpu() {
                self.stats.gpu_docs += 1;
            } else {
                self.stats.cpu_docs += 1;
            }
        }
        #[cfg(not(feature = "gpu"))]
        {
            self.stats.cpu_docs += 1;
        }

        // Update state
        let packed = self.state.load(Ordering::Acquire);
        let (_, err, count, gen) = unpack_state(packed);
        let new_state = pack_state(PipelinePhase::Idle, err, count.saturating_add(1), gen.wrapping_add(1));
        self.state.store(new_state, Ordering::Release);

        // Check memory budget if set
        if let Some(ref budget) = self.memory_budget {
            if !budget.can_allocate(0) {
                return Err(PipelineError::MemoryBudgetExceeded);
            }
        }

        Ok(())
    }

    /// Add document using GPU path
    #[cfg(feature = "gpu")]
    fn add_document_gpu(&mut self, doc_id: DocId, tokens: &[&str]) -> Result<(), PipelineError> {
        let coordinator = self.batch_coordinator.as_mut().unwrap();

        // Pre-hash tokens to u32
        let token_hashes: Vec<u32> = tokens
            .iter()
            .map(|t| {
                // Simple FNV-1a hash
                let mut h = 2166136261u32;
                for b in t.bytes() {
                    h ^= b as u32;
                    h = h.wrapping_mul(16777619);
                }
                h
            })
            .collect();

        // Add to batch
        let batch_full = coordinator.add_document(doc_id, token_hashes);

        // If batch is full, submit to GPU
        if batch_full {
            coordinator.submit_batch();

            // Process GPU batch (simplified - in production would be async)
            self.process_gpu_batch()?;
        }

        Ok(())
    }

    /// Process GPU batch
    ///
    /// Executes MinHash + LSH computation on GPU with metacapsule tracking (Wave 1.1).
    /// Records success/failure for circuit breaker pattern.
    #[cfg(feature = "gpu")]
    fn process_gpu_batch(&mut self) -> Result<(), PipelineError> {
        // Set phase first before taking mutable borrows
        self.set_phase(PipelinePhase::Computing);

        // Check metacapsule health before GPU dispatch (Wave 1.1)
        //
        // #ASSUME_HEALTH_CHECK_FAST: is_gpu_healthy() < 50ns
        // #VERIFY_HEALTH_CHECK_FAST: Verified in pipeline_metacapsule.rs B32 benchmarks
        if !self.gpu_metacapsule.is_gpu_healthy() {
            // Record failure and return error - circuit breaker may trip
            self.gpu_metacapsule.record_failure();
            return Err(PipelineError::ResourceLimitExceeded {
                reason: "GPU health check failed (metacapsule reports unhealthy)".to_string(),
            });
        }

        let ctx = self.gpu_context.as_ref().unwrap();
        let minhash = self.minhash_gpu.as_ref().unwrap();
        let lsh_band = self.lsh_band_gpu.as_ref().unwrap();
        let coordinator = self.batch_coordinator.as_mut().unwrap();

        // Get batches from processing buffer
        let batches = coordinator.processing_batches();
        let mut total_docs_in_batch: u64 = 0;

        for batch in batches.iter() {
            if batch.is_empty() {
                continue;
            }

            let batch_doc_count = batch.len() as u64;

            // Prepare GPU input for MinHash
            let minhash_input = MinHashGpuInput {
                tokens: &batch.tokens,
                offsets: &batch.offsets,
                num_docs: batch.len() as u32,
            };

            // Compute MinHash on GPU
            let minhash_start = std::time::Instant::now();
            let minhash_output = match minhash.compute(ctx, minhash_input) {
                Ok(output) => output,
                Err(e) => {
                    // Record failure to metacapsule (Wave 1.1)
                    // This updates circuit breaker and may cause GPU->CPU fallback
                    self.gpu_metacapsule.record_failure();
                    return Err(PipelineError::ResourceLimitExceeded {
                        reason: format!("GPU MinHash compute failed: {}", e),
                    });
                }
            };
            self.stats.gpu_compute_us += minhash_start.elapsed().as_micros() as u64;

            // Compute LSH band hashes on GPU (directly from MinHash output)
            let lsh_start = std::time::Instant::now();
            let lsh_input = LshBandGpuInput {
                signatures: minhash_output.signatures(),
                num_docs: minhash_output.num_docs(),
            };
            let lsh_output = match lsh_band.compute(ctx, lsh_input) {
                Ok(output) => output,
                Err(e) => {
                    // Record failure to metacapsule (Wave 1.1)
                    self.gpu_metacapsule.record_failure();
                    return Err(PipelineError::ResourceLimitExceeded {
                        reason: format!("GPU LSH band compute failed: {}", e),
                    });
                }
            };
            self.stats.lsh_band_us += lsh_start.elapsed().as_micros() as u64;

            // Store signatures and populate LSH buckets
            for (i, &doc_id) in batch.doc_ids.iter().enumerate() {
                // Store CPU signature for later Jaccard verification
                let sig_array = minhash_output.get_signature(i);

                // Store in mmap-backed signature storage
                if let Some(ref storage) = self.signature_storage {
                    let _ = storage.store(doc_id, &sig_array);
                }

                // Insert into LSH buckets for candidate pair generation
                let band_hashes = lsh_output.get_band_hashes(i);
                if let Some(storage) = &self.lsh_bucket_storage {
                    for (band_idx, band_hash) in band_hashes.iter().enumerate() {
                        // Ignore insertion errors (bucket full is acceptable for LSH approximate matching)
                        let _ = storage.insert(band_idx as u32, *band_hash, doc_id);
                    }
                }
            }

            total_docs_in_batch += batch_doc_count;
        }

        // Swap buffers for next batch
        coordinator.swap_buffers();

        // Record success to metacapsule (Wave 1.1)
        //
        // #ASSUME_RECORD_SUCCESS_FAST: record_success() < 100ns
        // #VERIFY_RECORD_SUCCESS_FAST: Verified in pipeline_metacapsule.rs B32 benchmarks
        self.gpu_metacapsule.record_success(total_docs_in_batch);

        self.stats.gpu_batches += 1;
        self.set_phase(PipelinePhase::Idle);

        Ok(())
    }

    /// Add document using CPU path (SIMD MinHash)
    fn add_document_cpu(&mut self, doc_idx: usize, tokens: &[&str]) {
        // Create CPU MinHash signature
        let signature = MinHashSignatureCapsule::compute_signature(tokens);

        // Store based on feature flags
        #[cfg(feature = "gpu")]
        {
            // Try mmap storage first (production path)
            if let Some(ref storage) = self.signature_storage {
                let sig_array = signature.signature();
                let _ = storage.store(doc_idx as u32, sig_array);
            } else {
                // Fallback to in-memory Vec (test path or when mmap disabled)
                self.signatures_fallback[doc_idx] = Some(signature);
            }
        }

        #[cfg(not(feature = "gpu"))]
        {
            self.signatures[doc_idx] = Some(signature);
        }
    }

    /// Find duplicates using Jaccard threshold
    ///
    /// # Arguments
    ///
    /// - `threshold`: Jaccard similarity threshold (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// Vector of duplicate clusters (each cluster is a Vec of DocIds)
    pub fn find_duplicates(&mut self, threshold: f64) -> Result<Vec<Vec<usize>>, PipelineError> {
        self.set_phase(PipelinePhase::Computing);

        // Flush any remaining GPU batch
        #[cfg(feature = "gpu")]
        if self.using_gpu {
            if let Some(coordinator) = &mut self.batch_coordinator {
                if coordinator.flush() {
                    self.process_gpu_batch()?;
                }
            }
        }

        self.set_phase(PipelinePhase::Clustering);

        let threshold_f32 = threshold as f32;
        let mut candidate_pairs = Vec::new();

        // Use LSH bucket-based candidate generation when GPU is enabled
        #[cfg(feature = "gpu")]
        if self.using_gpu && self.lsh_bucket_storage.is_some() {
            // Generate candidate pairs from LSH buckets (much more efficient than brute force)
            let mut seen_pairs: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
            let storage = self.lsh_bucket_storage.as_ref().unwrap();

            // Iterate over all bands and buckets
            for band in 0..storage.num_bands() {
                // Iterate over all possible bucket hashes (0..num_buckets_per_band)
                for bucket_hash in 0..storage.num_buckets_per_band() as u64 {
                    let doc_ids = storage.get_bucket(band, bucket_hash);

                    // Documents in the same bucket are candidate pairs
                    if doc_ids.len() < 2 {
                        continue;
                    }

                    for (idx_i, &doc_i) in doc_ids.iter().enumerate() {
                        for &doc_j in &doc_ids[idx_i + 1..] {
                            let pair = if doc_i < doc_j {
                                (doc_i as usize, doc_j as usize)
                            } else {
                                (doc_j as usize, doc_i as usize)
                            };

                            // Skip if we've already seen this pair
                            if seen_pairs.contains(&pair) {
                                continue;
                            }
                            seen_pairs.insert(pair);

                            // Verify with actual Jaccard similarity
                            // Get signatures from mmap storage
                            let sig_i_opt = self.signature_storage.as_ref()
                                .and_then(|s| s.get(pair.0 as u32))
                                .map(|arr| MinHashSignatureCapsule::from_signature(arr));
                            let sig_j_opt = self.signature_storage.as_ref()
                                .and_then(|s| s.get(pair.1 as u32))
                                .map(|arr| MinHashSignatureCapsule::from_signature(arr));

                            if let (Some(sig_i), Some(sig_j)) = (sig_i_opt, sig_j_opt) {
                                let jaccard = sig_i.jaccard_similarity(&sig_j);
                                if jaccard >= threshold_f32 {
                                    candidate_pairs.push(pair);
                                }
                            }
                        }
                    }
                }
            }

            self.stats.lsh_candidates = seen_pairs.len() as u64;
        }

        // Fall back to brute-force comparison when not using GPU LSH
        #[cfg(feature = "gpu")]
        let use_brute_force = !self.using_gpu || self.lsh_bucket_storage.is_none();
        #[cfg(not(feature = "gpu"))]
        let use_brute_force = true;

        if use_brute_force {
            // Use feature-specific signature storage
            #[cfg(feature = "gpu")]
            {
                // Check mmap storage first, then fallback to in-memory Vec
                let use_mmap = self.signature_storage.is_some();

                for i in 0..self.capacity {
                    // Check if signature exists (either in mmap or fallback)
                    let has_sig_i = if use_mmap {
                        self.signature_storage.as_ref().unwrap().contains(i as u32)
                    } else {
                        self.signatures_fallback[i].is_some()
                    };

                    if !has_sig_i {
                        continue;
                    }

                    for j in (i + 1)..self.capacity {
                        let has_sig_j = if use_mmap {
                            self.signature_storage.as_ref().unwrap().contains(j as u32)
                        } else {
                            self.signatures_fallback[j].is_some()
                        };

                        if !has_sig_j {
                            continue;
                        }

                        // Get signatures from appropriate storage
                        let (sig_i_opt, sig_j_opt) = if use_mmap {
                            let storage = self.signature_storage.as_ref().unwrap();
                            let sig_i = storage.get(i as u32)
                                .map(|arr| MinHashSignatureCapsule::from_signature(arr));
                            let sig_j = storage.get(j as u32)
                                .map(|arr| MinHashSignatureCapsule::from_signature(arr));
                            (sig_i, sig_j)
                        } else {
                            (self.signatures_fallback[i].as_ref().cloned(),
                             self.signatures_fallback[j].as_ref().cloned())
                        };

                        if let (Some(sig_i), Some(sig_j)) = (sig_i_opt, sig_j_opt) {
                            // Compute Jaccard similarity from MinHash signatures
                            let jaccard = sig_i.jaccard_similarity(&sig_j);
                            if jaccard >= threshold_f32 {
                                candidate_pairs.push((i, j));
                            }
                        }
                    }
                }
            }

            #[cfg(not(feature = "gpu"))]
            {
                for i in 0..self.capacity {
                    if self.signatures[i].is_none() {
                        continue;
                    }

                    for j in (i + 1)..self.capacity {
                        if self.signatures[j].is_none() {
                            continue;
                        }

                        let sig_i = self.signatures[i].as_ref().unwrap();
                        let sig_j = self.signatures[j].as_ref().unwrap();

                        // Compute Jaccard similarity from MinHash signatures
                        let jaccard = sig_i.jaccard_similarity(sig_j);
                        if jaccard >= threshold_f32 {
                            candidate_pairs.push((i, j));
                        }
                    }
                }
            }
        }

        self.stats.duplicate_pairs = candidate_pairs.len() as u64;

        // Union-Find clustering
        for (i, j) in candidate_pairs {
            self.union_find.union(i, j);
        }

        // Extract clusters
        let mut cluster_map: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();

        // Check which documents have signatures based on feature flags
        #[cfg(feature = "gpu")]
        {
            let use_mmap = self.signature_storage.is_some();
            for i in 0..self.capacity {
                let has_sig = if use_mmap {
                    self.signature_storage.as_ref().unwrap().contains(i as u32)
                } else {
                    self.signatures_fallback[i].is_some()
                };

                if has_sig {
                    let root = self.union_find.find(i);
                    cluster_map.entry(root).or_default().push(i);
                }
            }
        }

        #[cfg(not(feature = "gpu"))]
        {
            for i in 0..self.capacity {
                if self.signatures[i].is_some() {
                    let root = self.union_find.find(i);
                    cluster_map.entry(root).or_default().push(i);
                }
            }
        }

        // Filter to clusters with >1 member
        let clusters: Vec<Vec<usize>> = cluster_map
            .into_values()
            .filter(|c| c.len() > 1)
            .collect();

        self.stats.clusters = clusters.len() as u64;
        self.set_phase(PipelinePhase::Complete);

        Ok(clusters)
    }

    /// Set pipeline phase (internal)
    fn set_phase(&self, phase: PipelinePhase) {
        let packed = self.state.load(Ordering::Acquire);
        let (_, err, count, gen) = unpack_state(packed);
        let new_state = pack_state(phase, err, count, gen.wrapping_add(1));
        self.state.store(new_state, Ordering::Release);
    }

    /// Get generation counter (Q34 audit)
    pub fn generation(&self) -> u32 {
        let packed = self.state.load(Ordering::Acquire);
        let (_, _, _, gen) = unpack_state(packed);
        gen
    }

    // ==================== ASYNC MODE API (gpu-async feature) ====================

    /// Enable async GPU processing mode
    ///
    /// When enabled, CPU fills batches while GPU processes previous batch.
    /// This achieves >80% overlap efficiency, hiding GPU transfer latency.
    ///
    /// # Requirements
    ///
    /// - GPU must be initialized (is_using_gpu() == true)
    /// - Feature: `gpu-async`
    ///
    /// # Returns
    ///
    /// - `true`: Async mode enabled successfully
    /// - `false`: GPU not available or already in async mode
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut pipeline = HybridDedupPipeline::new(10_000, PipelineMode::Auto, &cpu_caps)?;
    /// if pipeline.enable_async() {
    ///     println!("Async mode enabled, overlap efficiency: {:.1}%", pipeline.async_overlap_efficiency() * 100.0);
    /// }
    /// ```
    #[cfg(feature = "gpu-async")]
    pub fn enable_async(&mut self) -> bool {
        // Require GPU to be initialized
        if !self.using_gpu {
            return false;
        }

        // Already enabled?
        if self.async_enabled.load(Ordering::Acquire) {
            return true;
        }

        // Get GPU context and create MinHash capsule for async runner
        let ctx = match &self.gpu_context {
            Some(ctx) => ctx.clone(),
            None => return false,
        };

        // Create MinHash capsule for async runner (separate from sync path)
        let minhash = match MinHashGpuCapsule::new(ctx.as_ref()) {
            Ok(m) => Arc::new(m),
            Err(_) => return false,
        };

        // Create async pipeline coordinator
        let coordinator = Arc::new(AsyncPipelineCoordinator::new(self.batch_size));

        // Create async GPU runner
        let mut runner = AsyncGpuRunner::new(ctx, minhash, coordinator.clone());
        runner.start();

        self.async_coordinator = Some(coordinator);
        self.async_runner = Some(runner);
        self.async_enabled.store(true, Ordering::Release);

        true
    }

    /// Disable async GPU processing mode
    ///
    /// Drains pending batches and stops background thread.
    #[cfg(feature = "gpu-async")]
    pub fn disable_async(&mut self) {
        if !self.async_enabled.load(Ordering::Acquire) {
            return;
        }

        // Drain and stop runner
        if let Some(runner) = &mut self.async_runner {
            runner.drain();
            runner.stop();
        }

        // Process any remaining results
        self.process_pending_async_results();

        self.async_runner = None;
        self.async_coordinator = None;
        self.async_enabled.store(false, Ordering::Release);
    }

    /// Check if async mode is enabled
    #[cfg(feature = "gpu-async")]
    pub fn is_async_enabled(&self) -> bool {
        self.async_enabled.load(Ordering::Acquire)
    }

    /// Get async overlap efficiency (0.0 to 1.0)
    ///
    /// Returns the ratio of time GPU was busy vs idle.
    /// Target: >0.8 (80% overlap)
    #[cfg(feature = "gpu-async")]
    pub fn async_overlap_efficiency(&self) -> f64 {
        match &self.async_runner {
            Some(runner) => runner.overlap_efficiency(),
            None => 0.0,
        }
    }

    /// Poll for async GPU results (non-blocking)
    ///
    /// Call this periodically during document addition to process
    /// completed GPU batches and populate LSH buckets.
    ///
    /// # Returns
    ///
    /// Number of results processed
    #[cfg(feature = "gpu-async")]
    pub fn poll_async_results(&mut self) -> usize {
        if !self.async_enabled.load(Ordering::Acquire) {
            return 0;
        }

        let mut count = 0;

        if let Some(runner) = &self.async_runner {
            while let Some(result) = runner.poll_result() {
                self.pending_results.push(result);
                count += 1;
            }
        }

        // Process pending results
        count += self.process_pending_async_results();
        count
    }

    /// Process pending async results (internal)
    ///
    /// Converts GPU results to signatures and populates LSH buckets.
    #[cfg(feature = "gpu-async")]
    fn process_pending_async_results(&mut self) -> usize {
        if self.pending_results.is_empty() {
            return 0;
        }

        let count = self.pending_results.len();

        for result in self.pending_results.drain(..) {
            for (i, &doc_id) in result.doc_ids.iter().enumerate() {
                let doc_idx = doc_id as usize;
                if doc_idx >= self.capacity {
                    continue;
                }

                // Extract signature and store
                let sig_slice = result.get_signature(i);
                let mut sig_array = [0u16; 128];
                sig_array.copy_from_slice(sig_slice);

                // Store in mmap-backed signature storage
                if let Some(ref storage) = self.signature_storage {
                    let _ = storage.store(doc_id, &sig_array);
                }

                // Note: LSH buckets are populated separately via band hashes
                // For now, we rely on brute-force similarity when using async mode
            }
        }

        count
    }

    /// Add document using async GPU path
    ///
    /// Submits document to async GPU runner without blocking.
    /// Call poll_async_results() periodically to process results.
    #[cfg(feature = "gpu-async")]
    fn add_document_async(&mut self, doc_id: DocId, tokens: &[&str]) -> Result<(), PipelineError> {
        let runner = match &self.async_runner {
            Some(r) => r,
            None => return Err(PipelineError::ResourceLimitExceeded {
                reason: "Async runner not initialized".to_string(),
            }),
        };

        // Pre-hash tokens to u32
        let token_hashes: Vec<u32> = tokens
            .iter()
            .map(|t| {
                let mut h = 2166136261u32;
                for b in t.bytes() {
                    h ^= b as u32;
                    h = h.wrapping_mul(16777619);
                }
                h
            })
            .collect();

        // Add to current batch via coordinator
        let coordinator = self.async_coordinator.as_ref().unwrap();

        // Fill the coordinator's buffer
        {
            let buffer = coordinator.filling_buffer();
            buffer.add_document(doc_id, token_hashes);
        }

        // Check if batch is ready to submit
        if !coordinator.is_filling_empty() {
            let buffer = coordinator.filling_buffer();
            if buffer.len() >= self.batch_size {
                // Submit batch to GPU
                let batch = std::mem::replace(
                    coordinator.filling_buffer(),
                    GpuBatch::with_capacity(self.batch_size, self.max_tokens_per_batch),
                );

                if !runner.submit_batch(batch) {
                    // Failed to submit, put batch back
                    // This shouldn't happen in normal operation
                    return Err(PipelineError::ResourceLimitExceeded {
                        reason: "GPU async queue full".to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Get async runner statistics
    #[cfg(feature = "gpu-async")]
    pub fn async_stats(&self) -> Option<(u64, u64, usize)> {
        self.async_runner.as_ref().map(|runner| {
            (
                runner.batches_submitted(),
                runner.batches_completed(),
                runner.pending_results(),
            )
        })
    }

    /// Clear pipeline state (for reuse)
    pub fn clear(&mut self) {
        // Disable async mode first (drains pending batches)
        #[cfg(feature = "gpu-async")]
        self.disable_async();

        // Clear signatures based on feature flags
        #[cfg(feature = "gpu")]
        {
            // Clear signature storage
            if let Some(storage) = &mut self.signature_storage {
                storage.clear();
            }

            // Clear in-memory fallback
            self.signatures_fallback.fill(None);

            // Clear LSH bucket storage
            if let Some(storage) = &mut self.lsh_bucket_storage {
                storage.clear_all();
            }

            // Clear batch coordinator
            if let Some(coordinator) = &mut self.batch_coordinator {
                coordinator.clear();
            }

            // Reset GPU metacapsule (Wave 1.1)
            // Note: This does NOT re-initialize the metacapsule, just clears statistics
            // and resets circuit breaker. GPU resources remain available.
            self.gpu_metacapsule.reset();

            // Re-initialize if GPU was in use (restore Ready state)
            if self.using_gpu {
                let _ = self.gpu_metacapsule.initialize();
            }
        }

        #[cfg(not(feature = "gpu"))]
        {
            self.signatures.fill(None);
        }

        self.union_find = UnionFind::new(self.capacity);
        self.stats = HybridPipelineStats::default();

        #[cfg(feature = "gpu-async")]
        {
            self.pending_results.clear();
        }

        self.state.store(pack_state(PipelinePhase::Idle, 0, 0, 0), Ordering::Release);
    }
}

impl std::fmt::Debug for HybridDedupPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HybridDedupPipeline")
            .field("mode", &self.mode)
            .field("using_gpu", &self.using_gpu)
            .field("phase", &self.phase())
            .field("capacity", &self.capacity)
            .field("batch_size", &self.batch_size)
            .field("stats", &self.stats)
            .finish()
    }
}

// SAFETY: HybridDedupPipeline is Send because all fields are Send
// - AtomicU64 is Send + Sync
// - GPU resources (wgpu) are Send (thread-safe)
// - Vec and primitives are Send
unsafe impl Send for HybridDedupPipeline {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_mode_default() {
        assert_eq!(PipelineMode::default(), PipelineMode::Auto);
    }

    #[test]
    fn test_state_packing() {
        let packed = pack_state(PipelinePhase::Computing, 5, 1000, 12345);
        let (phase, err, count, gen) = unpack_state(packed);

        assert_eq!(phase, PipelinePhase::Computing);
        assert_eq!(err, 5);
        assert_eq!(count, 1000);
        assert_eq!(gen, 12345);
    }

    #[test]
    fn test_pipeline_creation_cpu_only() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let pipeline = HybridDedupPipeline::new(100, PipelineMode::CpuOnly, &cpu_caps).unwrap();

        assert!(!pipeline.is_using_gpu());
        assert_eq!(pipeline.phase(), PipelinePhase::Idle);
        assert_eq!(pipeline.capacity, 100);
    }

    #[test]
    fn test_pipeline_add_document_cpu() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let mut pipeline = HybridDedupPipeline::new(10, PipelineMode::CpuOnly, &cpu_caps).unwrap();

        pipeline.add_document(0, "The quick brown fox").unwrap();
        pipeline.add_document(1, "The quick brown fox jumps").unwrap();
        pipeline.add_document(2, "Completely different text").unwrap();

        assert_eq!(pipeline.stats().docs_processed, 3);
        assert_eq!(pipeline.stats().cpu_docs, 3);
    }

    #[test]
    fn test_pipeline_find_duplicates_cpu() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let mut pipeline = HybridDedupPipeline::new(5, PipelineMode::CpuOnly, &cpu_caps).unwrap();

        // Add near-duplicate documents
        pipeline.add_document(0, "The quick brown fox jumps over the lazy dog").unwrap();
        pipeline.add_document(1, "The quick brown fox jumps over the lazy cat").unwrap();
        pipeline.add_document(2, "A completely different document with other words").unwrap();

        let clusters = pipeline.find_duplicates(0.5).unwrap();

        // Documents 0 and 1 should be clustered (high similarity)
        assert!(!clusters.is_empty());
        assert_eq!(pipeline.phase(), PipelinePhase::Complete);
    }

    #[test]
    fn test_pipeline_bounds_check() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let mut pipeline = HybridDedupPipeline::new(5, PipelineMode::CpuOnly, &cpu_caps).unwrap();

        let result = pipeline.add_document(10, "Out of bounds");
        assert!(result.is_err());
    }

    #[test]
    fn test_pipeline_clear() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let mut pipeline = HybridDedupPipeline::new(10, PipelineMode::CpuOnly, &cpu_caps).unwrap();

        pipeline.add_document(0, "Test document").unwrap();
        assert_eq!(pipeline.stats().docs_processed, 1);

        pipeline.clear();
        assert_eq!(pipeline.stats().docs_processed, 0);
        assert_eq!(pipeline.phase(), PipelinePhase::Idle);
    }

    #[test]
    fn test_pipeline_generation_counter() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let mut pipeline = HybridDedupPipeline::new(10, PipelineMode::CpuOnly, &cpu_caps).unwrap();

        let gen1 = pipeline.generation();
        pipeline.add_document(0, "Test").unwrap();
        let gen2 = pipeline.generation();

        assert!(gen2 > gen1);
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn test_pipeline_gpu_auto_detection() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let pipeline = HybridDedupPipeline::new(100, PipelineMode::Auto, &cpu_caps).unwrap();

        // Pipeline should work regardless of GPU availability
        println!("Using GPU: {}", pipeline.is_using_gpu());
        if pipeline.is_using_gpu() {
            println!("GPU: {:?}", pipeline.gpu_capabilities());
        }
    }
}
