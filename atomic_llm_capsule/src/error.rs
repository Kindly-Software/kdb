//! Error types for quantization operations

/// Quantization error type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantError {
    /// Invalid input size
    InvalidSize,
    /// Invalid range
    InvalidRange,
    /// Buffer size mismatch
    BufferSizeMismatch {
        /// Expected size
        expected: usize,
        /// Actual size
        actual: usize,
    },
    /// Scale overflow (division by zero or extreme values)
    ScaleOverflow,
    /// Capsule not resident in RAM (must load from SSD first)
    NotResident,
    /// Generation counter mismatch (torn read detected)
    GenerationMismatch,
    /// I/O error during SSD eviction or load
    #[cfg(feature = "std")]
    IoError,
}

#[cfg(feature = "std")]
impl From<std::io::Error> for QuantError {
    fn from(_: std::io::Error) -> Self {
        QuantError::IoError
    }
}

/// Quantization result type
pub type QuantResult<T> = Result<T, QuantError>;
