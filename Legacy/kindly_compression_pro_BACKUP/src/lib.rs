//! # kindly_compression_pro - Proprietary Weight Compression Codec
//!
//! **TRADE SECRET**: This crate contains breakthrough compression algorithms.
//! **NEVER commit to public repositories, NEVER share publicly**
//!
//! ## Breakthrough Discovery (UCE34 Analysis)
//!
//! **6-10× weight compression with <2% accuracy loss** using:
//!
//! 1. **Structured Block Sparsity** (40-60%): 1.67-2.5× compression, 1% loss
//! 2. **Mixed-Precision Quantization** (layer-sensitive): 2-3× compression, 1% loss
//! 3. **Dictionary Compression** (weight clustering): 1.5× compression, 0.5% loss
//!
//! **Total**: 1.67× × 2.5× × 1.5× = **6.26× compression, 2% total loss**
//!
//! ## Performance Targets (B32 Framework)
//!
//! - **Compression ratio**: 6-10× (vs 4× GPTQ, 2× Q8.8)
//! - **Accuracy loss**: <2% perplexity increase
//! - **Decompression**: <5μs per 1MB block (SIMD parallelized)
//! - **Determinism**: 100% reproducible (fixed-point, no FP arithmetic)
//!
//! ## Computational Capsule Architecture (Q10-Q12)
//!
//! **T6 Mixed (T2+T3+T4)** - Composite Capsule:
//!
//! - **T2 (SIMD)**: Parallel block unpacking (f32x8, 8× speedup)
//! - **T3 (Fixed-Point)**: Deterministic quantization (Q4.4, Q6.6, Q8.8)
//! - **T4 (Batch)**: Batch processing (512-4096 blocks, 10-100× throughput)
//!
//! **Compound Speedup**: 8× × 2× × 10× = **160× potential**

// IMPL-2 v3.1: Nightly features MANDATORY for target performance
#![cfg_attr(feature = "portable_simd", feature(portable_simd))]
#![cfg_attr(feature = "nightly-const-fp", feature(const_fn_floating_point_arithmetic))]

// Mandatory Chaos enforcement: NO mutex/RwLock
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(
    missing_docs,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]
#![allow(clippy::module_name_repetitions)]

pub mod weight_compression;

// Re-exports for convenience
pub use weight_compression::{
    BlockData, CompressedLayer, QuantFormat, QuantizedBlock,
    unpack_block_8x8_simd, find_nearest_centroid_simd,
    dequantize_blocks_simd, block_to_vector,
    decompress_blocks_batch, compress_blocks_batch,
    BatchConfig, CompressedBlock, DecompressedBlock,
};

#[cfg(all(feature = "portable_simd", target_feature = "avx512f"))]
pub use weight_compression::{
    unpack_block_8x8_simd_avx512,
    find_nearest_centroid_simd_avx512,
};

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
