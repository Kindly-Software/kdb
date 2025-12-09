//! Type definitions for advanced weight compression
//!
//! All types designed for computational capsule architecture (T6 Mixed Tier).

use std::vec::Vec;

// Import QuantFormat from base module (T3 tier)
pub use crate::weight_compression::QuantFormat;

/// 8×8 weight block (256 bytes, cache-aligned)
///
/// Used as fallback when SIMD is disabled
#[repr(C, align(32))]
#[derive(Clone, Copy)]
pub struct BlockData {
    /// 8×8 weight matrix (row-major layout)
    pub weights: [[f32; 8]; 8],
}

impl BlockData {
    /// Create new zero-initialized block
    pub const fn new() -> Self {
        Self {
            weights: [[0.0; 8]; 8],
        }
    }
}

/// Sparse block containing non-zero weights from an 8×8 block
#[derive(Clone, Debug)]
pub struct SparseBlock {
    /// Flattened weights (64 elements for 8×8 block)
    pub weights: [f32; 64],
    /// Block index in the original weight matrix
    pub block_index: u32,
    /// L2 magnitude (used for pruning decisions)
    pub magnitude: f32,
}

impl SparseBlock {
    /// Create a new sparse block from an 8×8 array
    pub fn from_block_8x8(block: &[[f32; 8]; 8], block_index: u32) -> Self {
        let mut weights = [0.0f32; 64];
        let mut magnitude = 0.0f32;

        for (i, row) in block.iter().enumerate() {
            for (j, &weight) in row.iter().enumerate() {
                let idx = i * 8 + j;
                weights[idx] = weight;
                magnitude += weight * weight;
            }
        }

        magnitude = magnitude.sqrt();

        Self {
            weights,
            block_index,
            magnitude,
        }
    }

    /// Reconstruct as 8×8 array
    pub fn to_block_8x8(&self) -> [[f32; 8]; 8] {
        let mut block = [[0.0f32; 8]; 8];

        for i in 0..8 {
            for j in 0..8 {
                block[i][j] = self.weights[i * 8 + j];
            }
        }

        block
    }
}

/// Quantized block (compressed representation)
#[derive(Clone, Debug)]
pub struct QuantizedBlock {
    /// Quantized weights (variable size depending on format)
    pub data: Vec<u8>,
    /// Quantization format used
    pub format: QuantFormat,
    /// Block index
    pub block_index: u32,
}

/// Compressed layer (final output after all 3 stages)
#[derive(Clone, Debug)]
pub struct CompressedLayer {
    /// Dictionary centroid IDs (8 bits per block)
    pub centroid_ids: Vec<u8>,
    /// Sparse block indices (which blocks are non-zero)
    pub sparse_indices: Vec<u32>,
    /// Layer quantization format
    pub format: QuantFormat,
    /// Number of original blocks
    pub total_blocks: usize,
}

/// Advanced compression error types (extends base CompressionError)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvancedCompressionError {
    /// Invalid block dimensions
    InvalidBlockSize,
    /// Unsupported quantization format
    UnsupportedFormat,
    /// Dictionary lookup failed
    DictionaryLookupFailed,
    /// Decompression error
    DecompressionFailed,
    /// Invalid sparsity ratio
    InvalidSparsity,
}

impl std::fmt::Display for AdvancedCompressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdvancedCompressionError::InvalidBlockSize => write!(f, "Invalid block size"),
            AdvancedCompressionError::UnsupportedFormat => write!(f, "Unsupported quantization format"),
            AdvancedCompressionError::DictionaryLookupFailed => write!(f, "Dictionary lookup failed"),
            AdvancedCompressionError::DecompressionFailed => write!(f, "Decompression failed"),
            AdvancedCompressionError::InvalidSparsity => write!(f, "Invalid sparsity ratio"),
        }
    }
}

impl std::error::Error for AdvancedCompressionError {}

/// Result type for advanced compression operations
pub type Result<T> = core::result::Result<T, AdvancedCompressionError>;
