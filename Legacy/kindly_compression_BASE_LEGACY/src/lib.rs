//! # Kindly Compression
//!
//! **TRADE SECRET - Proprietary compression algorithms for the Kindly ecosystem.**
//!
//! ## Features
//!
//! - **Fixed-Point Quantization**: Q4.4/Q6.6/Q8.8 deterministic quantization (T3 tier)
//! - **Weight Compression**: 1.5-2.5× compression for neural network weights
//! - **Deterministic**: 100% reproducible (same input → same output, bit-exact)
//! - **Fast Decompression**: ~40µs for 1KB (production-ready)
//! - **Zero Dependencies**: Pure Rust implementation
//!
//! ## Usage
//!
//! ```rust
//! use kindly_compression::weight_compression::{quantize_q4_4, dequantize_q4_4};
//!
//! let weight: f32 = 3.14159;
//! let quantized = quantize_q4_4(weight);
//! let reconstructed = dequantize_q4_4(quantized);
//! assert!((weight - reconstructed).abs() < 0.02);  // <2% error
//! ```
//!
//! ## License
//!
//! **PROPRIETARY** - All algorithms and code are trade secret protected.
//! NEVER commit to public repositories, NEVER share publicly.

pub mod error;
pub mod token_clustering;
pub mod weight_compression;

pub use error::CompressionError;
pub use token_clustering::TokenClusteringCodec;

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
