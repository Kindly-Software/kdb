//! GPU Acceleration Module - T7 Heterogeneous Tier
//!
//! # Architecture
//!
//! Cross-platform GPU compute using wgpu (WebGPU).
//! Provides 5-50x speedup for MinHash/LSH operations on GPU-enabled systems.
//!
//! # Module Structure
//!
//! - `context` - GpuContextCapsule (T7): Device/queue management
//! - `capabilities` - GpuCapabilities (T0): GPU feature detection
//! - `buffer_pool` - GpuBufferPoolCapsule (T1): Lockfree buffer management
//! - `pipeline_coordinator` - DoubleBuffer/BatchCoordinator (T7): CPU-GPU overlap
//! - `error` - GPU-specific error types
//! - `kernels/` - WGSL compute shaders
//!
//! # Pipeline Design
//!
//! ```text
//! CPU Stage 1: Tokenization (not GPU-friendly)
//!      |
//!      v
//! GPU Stage: MinHash + LSH (embarrassingly parallel)
//!      |
//!      v
//! CPU Stage 2: Union-Find clustering (sequential)
//! ```
//!
//! # Performance Targets (B32 Framework)
//!
//! | Hardware | Current (CPU) | Target (GPU) | Speedup |
//! |----------|---------------|--------------|---------|
//! | iGPU | 73.4K docs/sec | 150K docs/sec | 2x |
//! | GTX 1650 | 73.4K docs/sec | 300K docs/sec | 4x |
//! | RTX 3060 | 73.4K docs/sec | 500K docs/sec | 7x |
//! | RTX 4090 | 73.4K docs/sec | 1M docs/sec | 14x |
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier (CPU+GPU coordination)
//! - **Chaos**: 100% lockfree state management
//! - **ASSUM**: GPU availability is runtime-checked, graceful fallback
//! - **B32**: Performance targets based on hardware tier
//! - **T28**: Unit tests for all components
//! - **I20**: Backward compatible (same API as DedupPipeline)
//!
//! # Example
//!
//! ```rust,ignore
//! use kindly_dedup::gpu::{GpuContextCapsule, GpuBufferPoolCapsule};
//!
//! // Initialize GPU context (async)
//! let ctx = GpuContextCapsule::new().await?;
//!
//! // Check if GPU is available and worth using
//! if ctx.is_ready() {
//!     println!("GPU: {}", ctx.capabilities().device_name);
//!     println!("Max batch: {} docs", ctx.max_minhash_batch_size());
//!
//!     // Create buffer pool for compute operations
//!     let mut pool = GpuBufferPoolCapsule::new(16);
//!     // Use GPU for MinHash computation...
//! } else {
//!     println!("Falling back to CPU SIMD");
//! }
//! ```
//!
//! # Backend Priority
//!
//! 1. Vulkan (Linux, Windows, Android) - widest compatibility
//! 2. Metal (macOS, iOS) - best for Apple Silicon
//! 3. DirectX 12 (Windows) - Windows optimization
//! 4. DirectX 11 (Windows legacy)
//! 5. OpenGL (fallback)
//! 6. WebGPU (browser)
//!
//! # Feature Gate
//!
//! This module requires the `gpu` feature:
//!
//! ```toml
//! [dependencies]
//! kindly_dedup = { version = "2.4", features = ["gpu"] }
//! ```

pub mod error;
pub mod capabilities;
pub mod context;
pub mod buffer_pool;
pub mod kernels;
pub mod pipeline_coordinator;
pub mod async_runner;
pub mod validation;
pub mod mmap_bucket_storage;
pub mod mmap_signature_storage;
pub mod crossover_detector;
pub mod fed_params;
pub mod state_machine;
pub mod health;
pub mod memory_pressure;
pub mod fallback_manager;
pub mod timeline_semaphore;
pub mod dependency_graph;
pub mod pipeline_metacapsule;
pub mod multi_gpu;

// Wave 3.1: GpuDriverMetacapsule v4.0 - T6 Mixed tier GPU orchestrator
// 2048B 32-capsule orchestrator following atomic_capsule blueprint
pub mod driver_metacapsule;

// Wave 3.3: KGPU Integration (atomic_capsule GPU capsules)
#[cfg(feature = "kgpu-integration")]
pub mod kgpu_integration;

// Re-exports for convenience
pub use error::{GpuError, GpuResult};
pub use capabilities::{GpuCapabilities, Backend, GpuClass, PerformanceTier};
pub use context::{GpuContextCapsule, GpuContextState};
pub use buffer_pool::{GpuBufferPoolCapsule, PoolStats, presets};
pub use kernels::{MinHashGpuCapsule, MinHashGpuInput, MinHashGpuOutput};
pub use kernels::{
    LshBandGpuCapsule, LshBandGpuInput, LshBandGpuOutput,
    NUM_BANDS, ROWS_PER_BAND, SIGNATURE_SIZE,
    cpu_hash_band, cpu_compute_all_bands, unpack_signature,
};
pub use kernels::{
    GpuLshCapsule, GpuLshConfig, GpuLshPhase,
    SignatureOutput, BandHashOutput, DocId,
    NUM_PERMUTATIONS, LSH_NUM_BANDS, LSH_ROWS_PER_BAND, MAX_BATCH_SIZE,
};
pub use pipeline_coordinator::{DoubleBuffer, GpuBatch, BatchCoordinator};

// Phase 3: Async overlap exports
pub use pipeline_coordinator::{AsyncPipelineCoordinator, AsyncPipelineState, PipelinePhase};
pub use async_runner::{AsyncGpuRunner, GpuBatchResult};

// Phase 4: Validation exports
pub use validation::{
    CpuMinHashReference, GpuValidationReport,
    validate_gpu_vs_cpu, validate_gpu_determinism, run_comprehensive_validation,
};

// Mmap-backed LSH bucket storage (T9 Persistent tier for O(1) memory guarantee)
pub use mmap_bucket_storage::{
    MmapBucketStorage, MmapBucketError, DocId as MmapDocId,
};

// Mmap-backed MinHash signature storage (T9 Persistent tier for O(1) memory guarantee)
pub use mmap_signature_storage::{
    MmapSignatureStorage, MmapError, SlotState, SLOT_SIZE,
};

// Crossover detector (T1 Atomic + T3 Fixed-Point tier for CPU/GPU mode selection)
pub use crossover_detector::{CrossoverDetectorCapsule, ExecutionMode};

// FED hash parameters (T7 Heterogeneous tier for CPU precompute + GPU constant memory)
pub use fed_params::{FedHashParamsCapsule, NUM_PERMUTATIONS as FED_NUM_PERMUTATIONS, HASH_PRIME};

// GPU safety capsules (T1 Atomic tier for robust GPU lifecycle management)
pub use state_machine::{GpuState, GpuStateMachineCapsule, GpuStateSnapshot, GpuStateMachineError};
pub use health::{GpuHealthCapsule, GpuHealthFlags};
pub use memory_pressure::{MemoryPressureCapsule, MemoryPressureLevel};
pub use fallback_manager::{GpuFallbackManager, CircuitState, FallbackStatus, FallbackMetrics};

// Timeline semaphore (T1 Atomic tier for GPU-CPU synchronization)
pub use timeline_semaphore::{TimelineSemaphoreCapsule, WaitResult, SemaphoreStats};

// Dependency graph (T8 Network tier for lock-free stage coordination)
pub use dependency_graph::{DependencyGraphCapsule, DependencySnapshot, PipelineStage, MAX_STAGES};

// Pipeline metacapsule (T6 Mixed tier for unified GPU orchestration)
pub use pipeline_metacapsule::{GpuPipelineMetacapsule, GpuPipelineSnapshot};

// Driver metacapsule v4.0 (T6 Mixed tier for 32-capsule GPU orchestration)
// Wave 3.1: 2048B orchestrator following atomic_capsule blueprint
pub use driver_metacapsule::{
    GpuDriverMetacapsule, GpuDriverSnapshot, GpuHealthStatus, GpuDriverTelemetry,
    GpuDriverError, DriverState, EngineMask,
    Phase1CapsuleStub, Phase2CapsuleStub, Phase3CapsuleStub, Phase4CapsuleStub,
};

// Multi-GPU coordination (T8 Network tier for distributed GPU orchestration)
pub use multi_gpu::{
    MultiGpuCoordinator, GpuDeviceCapsule, GpuDeviceState, GpuStats,
    LoadBalancingStrategy, GpuId, MAX_GPUS,
};

// Wave 3.3: KGPU Integration re-exports (kindly_dedup adapters implementing KGPU patterns)
// NOTE: These are local implementations following atomic_capsule KGPU patterns.
// When atomic_capsule's kgpu module is fully exported, these can be refactored
// to re-export the upstream implementations directly.
#[cfg(feature = "kgpu-integration")]
pub use kgpu_integration::{
    // Adapters (kindly_dedup implementations of KGPU patterns)
    KgpuCommandEncoderAdapter,
    KgpuShaderCacheAdapter,
    KgpuMemoryPoolAdapter,
    KgpuFenceAdapter,
    KgpuComputePassAdapter,
    KgpuPipelineCacheAdapter,
    KgpuIntegrationSnapshot,
    SizeClass,
};

/// GPU acceleration is available (feature compiled)
pub const GPU_AVAILABLE: bool = true;

/// Check if GPU acceleration is compiled in
#[inline]
pub const fn is_gpu_feature_enabled() -> bool {
    GPU_AVAILABLE
}

/// Check if GPU acceleration is available on this system
///
/// # Returns
///
/// - `true`: GPU with compute shaders available
/// - `false`: No suitable GPU found, use CPU fallback
///
/// # Note
///
/// This function catches panics from wgpu initialization to prevent
/// application crashes on GPUs with driver issues (e.g., Intel Arc on Linux).
///
/// # Example
///
/// ```rust,ignore
/// if kindly_dedup::gpu::is_gpu_available() {
///     println!("GPU acceleration available");
/// } else {
///     println!("Using CPU fallback");
/// }
/// ```
pub fn is_gpu_available() -> bool {
    // Wrap GPU check in catch_unwind to prevent crashes from wgpu driver issues
    // Intel Arc, some AMD APUs, and other GPUs may cause wgpu to panic
    std::panic::catch_unwind(|| {
        GpuContextCapsule::new_blocking().is_ok()
    }).unwrap_or(false)
}

/// Try to detect and initialize GPU (convenience function)
///
/// Returns Some(context) if GPU is available and initialized,
/// None otherwise. This is the recommended entry point.
///
/// # Example
///
/// ```rust,ignore
/// if let Some(gpu) = kindly_dedup::gpu::try_init_gpu() {
///     println!("Using GPU: {}", gpu.capabilities().device_name);
/// } else {
///     println!("No GPU, using CPU fallback");
/// }
/// ```
pub fn try_init_gpu() -> Option<GpuContextCapsule> {
    GpuContextCapsule::new_blocking().ok()
}

/// Try to detect and initialize GPU (async version)
pub async fn try_init_gpu_async() -> Option<GpuContextCapsule> {
    GpuContextCapsule::new().await.ok()
}

/// Get GPU device information (if available)
///
/// # Returns
///
/// - `Some(capabilities)`: GPU capabilities
/// - `None`: No GPU available
pub fn get_gpu_info() -> Option<GpuCapabilities> {
    try_init_gpu().map(|ctx| ctx.capabilities().clone())
}

/// WGSL shader source for MinHash kernel
///
/// Embedded at compile time for convenience.
/// Can also be loaded from `src/gpu/kernels/minhash.wgsl`.
pub const MINHASH_SHADER: &str = include_str!("kernels/minhash.wgsl");

/// WGSL shader source for LSH Band kernel
///
/// Embedded at compile time for convenience.
/// Can also be loaded from `src/gpu/kernels/lsh_band.wgsl`.
pub const LSH_BAND_SHADER: &str = include_str!("kernels/lsh_band.wgsl");

/// WGSL shader source for FED MinHash kernel (Fast Exact Deduplication)
///
/// FED optimization: Precomputed hash parameters in uniform buffer (constant memory).
/// Expected speedup: 6-24× vs current GPU MinHash (arXiv:2501.01046).
///
/// Key benefits:
/// - Zero parameter computation on GPU (done on CPU once)
/// - Constant memory broadcast (1 read per workgroup vs N per thread)
/// - Simpler hash function (3 ops vs 12+ ops for FNV-1a)
/// - Better GPU occupancy (lower register pressure)
///
/// Embedded at compile time for convenience.
/// Can also be loaded from `src/gpu/kernels/minhash_fed.wgsl`.
pub const MINHASH_FED_SHADER: &str = include_str!("kernels/minhash_fed.wgsl");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_enabled() {
        assert!(is_gpu_feature_enabled());
        assert!(GPU_AVAILABLE);
    }

    #[test]
    fn test_gpu_availability_check() {
        // This should not panic regardless of GPU presence
        let _available = is_gpu_available();
    }

    #[test]
    fn test_gpu_info_retrieval() {
        // This should not panic regardless of GPU presence
        let info = get_gpu_info();
        if let Some(caps) = info {
            assert!(!caps.device_name.is_empty());
            println!("GPU: {} ({:?})", caps.device_name, caps.backend);
        }
    }

    #[test]
    fn test_try_init_gpu() {
        // This test passes regardless of GPU availability
        match try_init_gpu() {
            Some(ctx) => {
                println!("GPU initialized successfully");
                assert!(ctx.is_ready());

                let caps = ctx.capabilities();
                println!("Device: {}", caps.device_name);
                println!("Backend: {:?}", caps.backend);
                println!("Worth using: {}", caps.worth_using());
            }
            None => {
                println!("No GPU available - fallback to CPU");
                // This is expected on CI or headless systems
            }
        }
    }

    #[test]
    fn test_buffer_pool_integration() {
        let pool = GpuBufferPoolCapsule::new(8);
        let stats = pool.stats();

        assert_eq!(stats.available_buffers, 0);
        assert_eq!(stats.max_pool_size, 8);
    }

    #[test]
    fn test_shader_embedded() {
        // Verify shader is embedded and non-empty
        assert!(!MINHASH_SHADER.is_empty());
        assert!(MINHASH_SHADER.contains("minhash_kernel"));
        assert!(MINHASH_SHADER.contains("@compute"));
    }

    #[test]
    fn test_reexports() {
        // Verify all public types are accessible
        let _: GpuError = GpuError::NoAdapterFound;
        let _: Backend = Backend::Vulkan;
        let _: GpuClass = GpuClass::Discrete;
        let _: PerformanceTier = PerformanceTier::HighEnd;
        let _: GpuContextState = GpuContextState::Uninitialized;
    }
}
