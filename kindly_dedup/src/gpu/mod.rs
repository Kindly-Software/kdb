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
//! - **COCA**: 100% lockfree state management
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
pub use pipeline_coordinator::{DoubleBuffer, GpuBatch, BatchCoordinator};

// Phase 3: Async overlap exports
pub use pipeline_coordinator::{AsyncPipelineCoordinator, AsyncPipelineState, PipelinePhase};
pub use async_runner::{AsyncGpuRunner, GpuBatchResult};

// Phase 4: Validation exports
pub use validation::{
    CpuMinHashReference, GpuValidationReport,
    validate_gpu_vs_cpu, validate_gpu_determinism, run_comprehensive_validation,
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
    pollster::block_on(async {
        GpuContextCapsule::new().await.is_ok()
    })
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
