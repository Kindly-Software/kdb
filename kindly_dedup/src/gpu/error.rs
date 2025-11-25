//! GPU Error Types - T7 Heterogeneous Tier
//!
//! Error types for GPU operations in kindly_dedup.
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier (CPU-GPU coordination)
//! - **COCA**: 100% lockfree (error types are immutable)
//! - **ASSUM**: Zero unsafe code
//! - **B32**: N/A (error types don't have performance targets)
//! - **T28**: Comprehensive error coverage

use std::fmt;

/// GPU operation errors
#[derive(Debug, Clone)]
pub enum GpuError {
    /// No suitable GPU adapter found
    NoAdapterFound,
    /// GPU device request failed
    DeviceRequestFailed(String),
    /// GPU not initialized
    NotInitialized,
    /// Shader compilation failed
    ShaderCompilationFailed(String),
    /// Buffer creation failed
    BufferCreationFailed(String),
    /// Compute operation failed
    ComputeFailed(String),
    /// Buffer mapping failed
    BufferMappingFailed(String),
    /// Invalid input data
    InvalidInput(String),
    /// GPU memory exhausted
    OutOfMemory,
    /// Feature not supported on this GPU
    FeatureNotSupported(String),
    /// Pipeline creation failed
    PipelineCreationFailed(String),
    /// Bind group creation failed
    BindGroupCreationFailed(String),
    /// Buffer too large for GPU memory
    BufferTooLarge {
        /// Requested buffer size in bytes
        requested: u64,
        /// Maximum allowed size in bytes
        max_size: u64,
    },
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuError::NoAdapterFound => write!(f, "No suitable GPU adapter found"),
            GpuError::DeviceRequestFailed(msg) => write!(f, "GPU device request failed: {}", msg),
            GpuError::NotInitialized => write!(f, "GPU not initialized"),
            GpuError::ShaderCompilationFailed(msg) => {
                write!(f, "Shader compilation failed: {}", msg)
            }
            GpuError::BufferCreationFailed(msg) => write!(f, "Buffer creation failed: {}", msg),
            GpuError::ComputeFailed(msg) => write!(f, "Compute operation failed: {}", msg),
            GpuError::BufferMappingFailed(msg) => write!(f, "Buffer mapping failed: {}", msg),
            GpuError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            GpuError::OutOfMemory => write!(f, "GPU memory exhausted"),
            GpuError::FeatureNotSupported(msg) => {
                write!(f, "Feature not supported on this GPU: {}", msg)
            }
            GpuError::PipelineCreationFailed(msg) => {
                write!(f, "Pipeline creation failed: {}", msg)
            }
            GpuError::BindGroupCreationFailed(msg) => {
                write!(f, "Bind group creation failed: {}", msg)
            }
            GpuError::BufferTooLarge { requested, max_size } => {
                write!(f, "Buffer too large: requested {} bytes, max {} bytes", requested, max_size)
            }
        }
    }
}

impl std::error::Error for GpuError {}

/// Result type for GPU operations
pub type GpuResult<T> = Result<T, GpuError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = GpuError::NoAdapterFound;
        assert_eq!(err.to_string(), "No suitable GPU adapter found");

        let err = GpuError::ComputeFailed("test error".to_string());
        assert!(err.to_string().contains("test error"));
    }

    #[test]
    fn test_error_debug() {
        let err = GpuError::OutOfMemory;
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("OutOfMemory"));
    }

    #[test]
    fn test_error_clone() {
        let err1 = GpuError::NotInitialized;
        let err2 = err1.clone();
        assert_eq!(err1.to_string(), err2.to_string());
    }
}
