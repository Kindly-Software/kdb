//! Type definitions for weight compression
//!
//! All types designed for computational capsule architecture.

use std::vec::Vec;

/// Quantization format for fixed-point representation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum QuantFormat {
    /// 4 bits integer, 4 bits fractional (±8.0, 0.0625 precision)
    Q4_4 = 0,
    /// 6 bits integer, 6 bits fractional (±32.0, 0.015625 precision)
    Q6_6 = 1,
    /// 8 bits integer, 8 bits fractional (±128.0, 0.00390625 precision)
    Q8_8 = 2,
}

impl QuantFormat {
    /// Get the scale factor for this format
    pub const fn scale(self) -> f32 {
        match self {
            QuantFormat::Q4_4 => 16.0,   // 2^4
            QuantFormat::Q6_6 => 64.0,   // 2^6
            QuantFormat::Q8_8 => 256.0,  // 2^8
        }
    }

    /// Get the min/max range for this format
    pub const fn range(self) -> (f32, f32) {
        match self {
            QuantFormat::Q4_4 => (-8.0, 7.9375),
            QuantFormat::Q6_6 => (-32.0, 31.984375),
            QuantFormat::Q8_8 => (-128.0, 127.99609375),
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

/// Error types for compression operations
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionError {
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

impl std::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionError::InvalidBlockSize => write!(f, "Invalid block size"),
            CompressionError::UnsupportedFormat => write!(f, "Unsupported quantization format"),
            CompressionError::DictionaryLookupFailed => write!(f, "Dictionary lookup failed"),
            CompressionError::DecompressionFailed => write!(f, "Decompression failed"),
            CompressionError::InvalidSparsity => write!(f, "Invalid sparsity ratio"),
        }
    }
}

impl std::error::Error for CompressionError {}

pub type Result<T> = core::result::Result<T, CompressionError>;
