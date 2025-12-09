//! GPU Compute Kernels - T7 Heterogeneous Tier
//!
//! High-performance GPU compute kernels for deduplication operations.
//!
//! # Available Kernels
//!
//! | Kernel | Purpose | Expected Speedup | Status |
//! |--------|---------|------------------|--------|
//! | MinHash | Signature computation | 33-167x | Production |
//! | LSH Band | Band hashing | 5-25x | Production |
//! | GpuLsh | Unified MinHash + LSH + Buckets | 8x+ | Production |
//! | Jaccard | Pairwise similarity (Phase 3) | 3-12x | Planned |
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
//! - **Chaos**: 100% lockfree (GPU kernels are inherently parallel)
//! - **ASSUM**: All GPU assumptions documented
//! - **B32**: Fair benchmarks vs CPU SIMD baselines
//! - **T28**: Property tests (GPU == CPU within tolerance)

mod minhash;
mod lsh_band;
mod lsh;

pub use minhash::{MinHashGpuCapsule, MinHashGpuInput, MinHashGpuOutput};
pub use lsh_band::{
    LshBandGpuCapsule, LshBandGpuInput, LshBandGpuOutput,
    NUM_BANDS, ROWS_PER_BAND, SIGNATURE_SIZE,
    // CPU reference implementations for testing
    cpu_hash_band, cpu_compute_all_bands, unpack_signature,
};
pub use lsh::{
    GpuLshCapsule, GpuLshConfig, GpuLshPhase,
    SignatureOutput, BandHashOutput, DocId,
    NUM_PERMUTATIONS, NUM_BANDS as LSH_NUM_BANDS,
    ROWS_PER_BAND as LSH_ROWS_PER_BAND, MAX_BATCH_SIZE,
};

// Future kernel exports (Phase GPU-3):
// pub use jaccard::{JaccardGpuCapsule, JaccardGpuInput, JaccardGpuOutput};
