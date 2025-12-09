//! # Kindly Compression - Unified Compression Framework
//!
//! **TRADE SECRET - Proprietary compression algorithms for the Kindly ecosystem.**
//!
//! ## Architecture
//!
//! This crate provides a unified compression framework with two tiers:
//!
//! ### Base Algorithms (Always Available)
//! - **Fixed-Point Quantization** (T3 tier): Q4.4/Q6.6/Q8.8 deterministic quantization
//! - **Token Clustering** (T3 tier): 4-6× compression for LLM token sequences
//! - **Zero Dependencies**: Pure Rust, no external dependencies
//! - **100% Deterministic**: Same input → same output, bit-exact
//!
//! ### Advanced Algorithms (Feature-Gated)
//! - **SIMD Operations** (T2 tier): 8× speedup via f32x8 vectorization
//! - **Batch Processing** (T4 tier): 10-100× throughput via parallel processing
//! - **Structured Sparsity** (T6 tier): 6-10× compression with <2% accuracy loss
//!
//! ## Feature Flags
//!
//! ```toml
//! [dependencies]
//! # Base algorithms only (default)
//! kindly_compression = "0.1"
//!
//! # With advanced algorithms
//! kindly_compression = { version = "0.1", features = ["advanced", "simd-advanced", "batch-processing"] }
//! ```
//!
//! ## Usage
//!
//! ### Base Quantization
//! ```rust
//! use kindly_compression::weight_compression::{quantize_q4_4, dequantize_q4_4};
//!
//! let weight: f32 = 3.14159;
//! let quantized = quantize_q4_4(weight);
//! let reconstructed = dequantize_q4_4(quantized);
//! assert!((weight - reconstructed).abs() < 0.02);  // <2% error
//! ```
//!
//! ### Advanced SIMD Operations (feature = "simd-advanced")
//! ```rust,ignore
//! #[cfg(feature = "simd-advanced")]
//! use kindly_compression::advanced::{BlockData, unpack_block_8x8_simd};
//! ```
//!
//! ## License
//!
//! **PROPRIETARY** - All algorithms and code are trade secret protected.
//! NEVER commit to public repositories, NEVER share publicly.

// ============================================================================
// Nightly Features
// ============================================================================

#![cfg_attr(feature = "portable_simd", feature(portable_simd))]
#![cfg_attr(feature = "nightly-const-fp", feature(const_fn_floating_point_arithmetic))]

// ============================================================================
// Base Modules (Always Available)
// ============================================================================

pub mod dictionary;
pub mod error;
pub mod multi_stage;
pub mod token_clustering;
pub mod token_clustering_capsule;
pub mod weight_compression;

// ============================================================================
// Advanced Modules (Feature-Gated)
// ============================================================================

#[cfg(feature = "advanced")]
pub mod advanced;

// ============================================================================
// Re-exports
// ============================================================================

// Base exports
pub use dictionary::{DictionaryCodec, Provider};
pub use error::CompressionError;
pub use multi_stage::TokenClusteringCapsule;
pub use token_clustering::TokenClusteringCodec;

// Advanced exports (feature-gated)
#[cfg(feature = "advanced")]
pub use advanced::{
    AdvancedCompressionError,
    SparseBlock,
    QuantizedBlock,
    CompressedLayer,
};

#[cfg(feature = "simd-advanced")]
pub use advanced::{
    BlockData,
    unpack_block_8x8_simd,
    find_nearest_centroid_simd,
    dequantize_blocks_simd,
    block_to_vector,
};

#[cfg(feature = "batch-processing")]
pub use advanced::{
    BatchConfig,
    decompress_blocks_batch,
    compress_blocks_batch,
};

// ============================================================================
// Universal Compression Trait
// ============================================================================

/// Universal compression interface for all compression algorithms.
///
/// This trait defines a common interface for compression and decompression
/// operations across different algorithms (token clustering, delta encoding, etc.).
pub trait Compress {
    /// The compressed output type (typically `Vec<u8>` for byte sequences).
    type Compressed;

    /// Error type for compression/decompression failures.
    type Error;

    /// Compress input data.
    ///
    /// # Arguments
    ///
    /// * `data` - Input bytes to compress
    ///
    /// # Returns
    ///
    /// Compressed representation or error.
    fn compress(&self, data: &[u8]) -> Result<Self::Compressed, Self::Error>;

    /// Decompress previously compressed data.
    ///
    /// # Arguments
    ///
    /// * `compressed` - Compressed data (output from `compress()`)
    ///
    /// # Returns
    ///
    /// Original data or error.
    fn decompress(&self, compressed: &Self::Compressed) -> Result<Vec<u8>, Self::Error>;

    /// Compression ratio achieved by last operation.
    ///
    /// Returns the ratio of original size to compressed size.
    /// Example: 6.0 means 6:1 compression (6× reduction).
    fn ratio(&self) -> f32;
}
