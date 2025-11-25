//! GPU Compute Kernels - T7 Heterogeneous Tier
//!
//! High-performance GPU compute kernels for deduplication operations.
//!
//! # Available Kernels
//!
//! | Kernel | Purpose | Expected Speedup | Status |
//! |--------|---------|------------------|--------|
//! | MinHash | Signature computation | 33-167× | Production |
//! | LSH Band | Band hashing | 5-25× | Production |
//! | Jaccard | Pairwise similarity (Phase 3) | 3-12× | Planned |
//!
//! # Architecture
//!
//! All kernels use WGSL compute shaders for cross-platform compatibility:
//! - Vulkan (Linux, Windows, Android)
//! - Metal (macOS, iOS)
//! - DX12 (Windows)
//! - WebGPU (Browser)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier (multi-accelerator)
//! - **COCA**: 100% lockfree (GPU kernels are inherently parallel)
//! - **ASSUM**: All GPU assumptions documented
//! - **B32**: Fair benchmarks vs CPU SIMD baselines
//! - **T28**: Property tests (GPU == CPU within tolerance)

mod minhash;
mod lsh_band;

pub use minhash::{MinHashGpuCapsule, MinHashGpuInput, MinHashGpuOutput};
pub use lsh_band::{
    LshBandGpuCapsule, LshBandGpuInput, LshBandGpuOutput,
    NUM_BANDS, ROWS_PER_BAND, SIGNATURE_SIZE,
    // CPU reference implementations for testing
    cpu_hash_band, cpu_compute_all_bands, unpack_signature,
};

// Future kernel exports (Phase GPU-3):
// pub use jaccard::{JaccardGpuCapsule, JaccardGpuInput, JaccardGpuOutput};
