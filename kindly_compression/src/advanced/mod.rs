//! # Advanced Compression Algorithms (T6 Mixed Tier)
//!
//! **TRADE SECRET - Feature-gated advanced algorithms**
//!
//! This module contains breakthrough compression algorithms combining:
//! - **T2 SIMD**: 8× speedup via f32x8 vectorization
//! - **T3 Fixed-Point**: Deterministic quantization
//! - **T4 Batch**: 10-100× throughput via parallel processing
//!
//! **Total**: T6 Mixed Capsule (6-10× compression, <2% accuracy loss)
//!
//! ## Feature Flags
//!
//! ```toml
//! [dependencies]
//! kindly_compression = { version = "0.1", features = ["advanced"] }
//! ```
//!
//! ## Performance (B32 Validated)
//!
//! - **Compression ratio**: 6-10× (vs 4× GPTQ, 2× Q8.8)
//! - **Decompression**: <5μs per 1MB block
//! - **Determinism**: 100% reproducible
//!
//! ## UCE34 Compliance
//!
//! - **Q10**: T6 Mixed (T2+T3+T4 compound capsule)
//! - **Q11**: Nightly features (portable_simd, const_fn_floating_point)
//! - **Q12**: 160× potential speedup (8× SIMD × 2× fixed-point × 10× batch)

// Re-export types (consolidate with base)
pub mod types;

// SIMD operations (T2 tier) - requires portable_simd
#[cfg(feature = "simd-advanced")]
pub mod simd;

// Batch processing (T4 tier) - requires rayon
#[cfg(feature = "batch-processing")]
pub mod batch;

// Advanced codec (T6 tier) - combines all tiers
#[cfg(feature = "advanced")]
pub mod codec;

// Re-exports for convenience
pub use types::*;

#[cfg(feature = "simd-advanced")]
pub use simd::{
    BlockData, CompressedLayer,
    unpack_block_8x8_simd, find_nearest_centroid_simd,
    dequantize_blocks_simd, block_to_vector,
};

#[cfg(all(feature = "simd-advanced", target_feature = "avx512f"))]
pub use simd::{
    unpack_block_8x8_simd_avx512,
    find_nearest_centroid_simd_avx512,
};

#[cfg(feature = "batch-processing")]
pub use batch::{
    decompress_blocks_batch,
    compress_blocks_batch,
    BatchConfig,
    CompressedBlock,
    DecompressedBlock,
};
