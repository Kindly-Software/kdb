//! Compression error types.

use std::fmt;

/// Errors that can occur during compression or decompression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompressionError {
    /// Input data is empty (cannot compress zero bytes).
    EmptyInput,

    /// Input data exceeds maximum size for this algorithm.
    InputTooLarge { size: usize, max: usize },

    /// Compressed data is corrupted or invalid.
    CorruptedData { reason: String },

    /// Decompression failed due to invalid format.
    InvalidFormat { expected: String, found: String },

    /// Invalid data encountered during processing.
    InvalidData(String),

    /// Internal error during compression/decompression.
    Internal { message: String },
}

impl fmt::Display for CompressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "Cannot compress empty input"),
            Self::InputTooLarge { size, max } => {
                write!(f, "Input size {} exceeds maximum {}", size, max)
            }
            Self::CorruptedData { reason } => write!(f, "Corrupted data: {}", reason),
            Self::InvalidFormat { expected, found } => {
                write!(f, "Invalid format: expected {}, found {}", expected, found)
            }
            Self::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
            Self::Internal { message } => write!(f, "Internal error: {}", message),
        }
    }
}

impl std::error::Error for CompressionError {}
