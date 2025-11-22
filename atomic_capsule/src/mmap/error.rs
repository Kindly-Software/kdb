//! Error types for capsule-native memory-mapped file operations
//!
//! **UCE34 Framework**: Q29 Error Handling for T9 Persistent + T1 Atomic mmap
//!
//! # Design Philosophy
//!
//! - **Simple enum**: No external dependencies (no thiserror)
//! - **Platform-agnostic**: Unix/Windows/Capsule OS error mapping
//! - **Zero unsafe**: 100% safe error types (ASSUM 99.99%)
//! - **Clone + Copy**: Error values are cheap to propagate
//! - **Rich context**: Clear messages for debugging mmap failures
//!
//! # Error Variants
//!
//! 1. **IOError**: Filesystem operations (open, set_len, msync, etc.)
//! 2. **InvalidAlignment**: Page alignment violations (4KB boundary)
//! 3. **CapacityExceeded**: Region allocation full
//! 4. **PageFault**: Memory access violation (SIGSEGV/SEH)
//! 5. **InvalidRegionIndex**: Bad region access (out of bounds)
//! 6. **GenerationMismatch**: TOCTOU detection via generation counters
//! 7. **PlatformUnsupported**: Non-Unix/Windows/Capsule OS platforms
//! 8. **FeatureNotEnabled**: Missing feature flag (e.g., nightly-atomic)
//!
//! # ASSUM Safety
//!
//! - No unsafe code in error types
//! - All error variants are 100% safe to construct and propagate
//! - Platform-specific error mapping is safe (no raw pointers)

use std::fmt;
use std::io;

/// Result type for mmap operations
pub type MmapResult<T> = Result<T, MmapError>;

/// Error types for capsule-native memory-mapped file operations
///
/// # Size
///
/// `size_of::<MmapError>() = 32 bytes` (enum discriminant + largest variant)
///
/// # Platform Support
///
/// - Unix: Maps `errno` codes to specific variants
/// - Windows: Maps Win32 error codes to specific variants
/// - Capsule OS: Native error codes (future)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmapError {
    /// Filesystem I/O error (open, set_len, msync, etc.)
    ///
    /// # Examples
    ///
    /// - File not found (ENOENT)
    /// - Permission denied (EACCES)
    /// - Disk full (ENOSPC)
    /// - Invalid file descriptor (EBADF)
    IOError {
        /// OS error code (errno on Unix, GetLastError on Windows)
        code: i32,
        /// Operation that failed
        operation: &'static str,
    },

    /// Page alignment violation (4KB boundary requirement)
    ///
    /// # OS Requirement
    ///
    /// mmap requires page-aligned offsets (4KB on x86-64, 16KB on ARM64)
    InvalidAlignment {
        /// Requested offset (not aligned)
        offset: u64,
        /// Required alignment (4KB = 4096, 16KB = 16384)
        required: u64,
    },

    /// Region allocation capacity exceeded
    ///
    /// # Example
    ///
    /// - Region has 1MB capacity, request 2MB allocation
    CapacityExceeded {
        /// Requested allocation size (bytes)
        requested: usize,
        /// Available capacity in region (bytes)
        available: usize,
    },

    /// Memory access violation (SIGSEGV/SEH exception)
    ///
    /// # Causes
    ///
    /// - Access beyond mapped region
    /// - Write to read-only mapping
    /// - Access to unmapped page
    PageFault,

    /// Invalid region index (out of bounds)
    ///
    /// # Example
    ///
    /// - Manager has 8 regions, request region 10
    InvalidRegionIndex {
        /// Requested region index
        index: usize,
        /// Maximum valid index (num_regions - 1)
        max: usize,
    },

    /// Generation counter mismatch (TOCTOU detection)
    ///
    /// # Purpose
    ///
    /// Detects concurrent modifications via generation counters
    /// (Time-of-Check, Time-of-Use race prevention)
    GenerationMismatch {
        /// Expected generation (from previous read)
        expected: u64,
        /// Actual generation (concurrent modification detected)
        actual: u64,
    },

    /// Platform not supported (not Unix/Windows/Capsule OS)
    ///
    /// # Example
    ///
    /// - Trying to compile on WASM or embedded target without std
    PlatformUnsupported,

    /// Required feature flag not enabled
    ///
    /// # Example
    ///
    /// - atomic_from_mut API requires `nightly-atomic` feature
    FeatureNotEnabled {
        /// Feature name (e.g., "nightly-atomic")
        feature: &'static str,
    },
}

impl MmapError {
    /// Create IOError from OS error code
    ///
    /// # Platform-Specific
    ///
    /// - Unix: errno from libc
    /// - Windows: GetLastError code
    #[inline]
    pub fn from_os_error(code: i32, operation: &'static str) -> Self {
        Self::IOError { code, operation }
    }

    /// Create InvalidAlignment error
    #[inline]
    pub fn invalid_alignment(offset: u64, required: u64) -> Self {
        Self::InvalidAlignment { offset, required }
    }

    /// Create CapacityExceeded error
    #[inline]
    pub fn capacity_exceeded(requested: usize, available: usize) -> Self {
        Self::CapacityExceeded {
            requested,
            available,
        }
    }

    /// Create InvalidRegionIndex error
    #[inline]
    pub fn invalid_region_index(index: usize, max: usize) -> Self {
        Self::InvalidRegionIndex { index, max }
    }

    /// Create GenerationMismatch error
    #[inline]
    pub fn generation_mismatch(expected: u64, actual: u64) -> Self {
        Self::GenerationMismatch { expected, actual }
    }

    /// Create FeatureNotEnabled error
    #[inline]
    pub fn feature_not_enabled(feature: &'static str) -> Self {
        Self::FeatureNotEnabled { feature }
    }

    /// Check if error is retryable (transient failure)
    ///
    /// # Retryable Errors
    ///
    /// - EAGAIN (Unix): Resource temporarily unavailable
    /// - EINTR (Unix): Interrupted system call
    /// - ERROR_BUSY (Windows): Device busy
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::IOError { code, .. } => {
                #[cfg(unix)]
                {
                    *code == libc::EAGAIN || *code == libc::EINTR
                }
                #[cfg(windows)]
                {
                    // ERROR_BUSY = 170
                    *code == 170
                }
                #[cfg(not(any(unix, windows)))]
                {
                    false
                }
            }
            Self::GenerationMismatch { .. } => true, // CAS retry expected
            _ => false,
        }
    }

    /// Check if error is fatal (non-recoverable)
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::PageFault | Self::PlatformUnsupported | Self::InvalidAlignment { .. }
        )
    }
}

impl fmt::Display for MmapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IOError { code, operation } => {
                write!(f, "I/O error during {}: OS error code {}", operation, code)
            }
            Self::InvalidAlignment { offset, required } => {
                write!(
                    f,
                    "Invalid alignment: offset {} must be aligned to {} bytes",
                    offset, required
                )
            }
            Self::CapacityExceeded {
                requested,
                available,
            } => {
                write!(
                    f,
                    "Capacity exceeded: requested {} bytes, available {} bytes",
                    requested, available
                )
            }
            Self::PageFault => {
                write!(f, "Memory access violation (page fault)")
            }
            Self::InvalidRegionIndex { index, max } => {
                write!(
                    f,
                    "Invalid region index: {} (max valid index: {})",
                    index, max
                )
            }
            Self::GenerationMismatch { expected, actual } => {
                write!(
                    f,
                    "Generation mismatch: expected {}, found {} (concurrent modification detected)",
                    expected, actual
                )
            }
            Self::PlatformUnsupported => {
                write!(
                    f,
                    "Platform not supported (requires Unix, Windows, or Capsule OS)"
                )
            }
            Self::FeatureNotEnabled { feature } => {
                write!(f, "Required feature not enabled: {}", feature)
            }
        }
    }
}

impl std::error::Error for MmapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // No nested errors (all errors are leaf nodes)
        None
    }
}

impl From<io::Error> for MmapError {
    fn from(err: io::Error) -> Self {
        let code = err.raw_os_error().unwrap_or(-1);
        Self::IOError {
            code,
            operation: "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_sizes() {
        // Verify error type is reasonably sized
        assert!(
            std::mem::size_of::<MmapError>() <= 32,
            "MmapError should be ≤32 bytes"
        );
    }

    #[test]
    fn test_io_error_display() {
        let err = MmapError::from_os_error(2, "open");
        let display = format!("{}", err);
        assert!(display.contains("I/O error"));
        assert!(display.contains("open"));
        assert!(display.contains("2"));
    }

    #[test]
    fn test_invalid_alignment_display() {
        let err = MmapError::invalid_alignment(1000, 4096);
        let display = format!("{}", err);
        assert!(display.contains("Invalid alignment"));
        assert!(display.contains("1000"));
        assert!(display.contains("4096"));
    }

    #[test]
    fn test_capacity_exceeded_display() {
        let err = MmapError::capacity_exceeded(2048, 1024);
        let display = format!("{}", err);
        assert!(display.contains("Capacity exceeded"));
        assert!(display.contains("2048"));
        assert!(display.contains("1024"));
    }

    #[test]
    fn test_page_fault_display() {
        let err = MmapError::PageFault;
        let display = format!("{}", err);
        assert!(display.contains("page fault"));
    }

    #[test]
    fn test_invalid_region_index_display() {
        let err = MmapError::invalid_region_index(10, 7);
        let display = format!("{}", err);
        assert!(display.contains("Invalid region index"));
        assert!(display.contains("10"));
        assert!(display.contains("7"));
    }

    #[test]
    fn test_generation_mismatch_display() {
        let err = MmapError::generation_mismatch(100, 102);
        let display = format!("{}", err);
        assert!(display.contains("Generation mismatch"));
        assert!(display.contains("100"));
        assert!(display.contains("102"));
        assert!(display.contains("concurrent modification"));
    }

    #[test]
    fn test_platform_unsupported_display() {
        let err = MmapError::PlatformUnsupported;
        let display = format!("{}", err);
        assert!(display.contains("Platform not supported"));
    }

    #[test]
    fn test_feature_not_enabled_display() {
        let err = MmapError::feature_not_enabled("nightly-atomic");
        let display = format!("{}", err);
        assert!(display.contains("feature not enabled"));
        assert!(display.contains("nightly-atomic"));
    }

    #[test]
    fn test_error_clone_copy() {
        let err1 = MmapError::PageFault;
        let err2 = err1; // Copy
        let err3 = err1.clone(); // Clone
        assert_eq!(err1, err2);
        assert_eq!(err1, err3);
    }

    #[test]
    fn test_is_retryable() {
        // EAGAIN is retryable on Unix
        #[cfg(unix)]
        {
            let err = MmapError::from_os_error(libc::EAGAIN, "read");
            assert!(err.is_retryable());
        }

        // Generation mismatch is retryable (CAS retry)
        let err = MmapError::generation_mismatch(10, 11);
        assert!(err.is_retryable());

        // Page fault is NOT retryable
        let err = MmapError::PageFault;
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_is_fatal() {
        // Page fault is fatal
        let err = MmapError::PageFault;
        assert!(err.is_fatal());

        // Platform unsupported is fatal
        let err = MmapError::PlatformUnsupported;
        assert!(err.is_fatal());

        // Invalid alignment is fatal
        let err = MmapError::invalid_alignment(100, 4096);
        assert!(err.is_fatal());

        // Capacity exceeded is NOT fatal (may retry with smaller size)
        let err = MmapError::capacity_exceeded(2048, 1024);
        assert!(!err.is_fatal());
    }

    #[test]
    fn test_from_io_error() {
        let io_err = io::Error::from_raw_os_error(2);
        let mmap_err: MmapError = io_err.into();
        match mmap_err {
            MmapError::IOError { code, operation } => {
                assert_eq!(code, 2);
                assert_eq!(operation, "unknown");
            }
            _ => panic!("Expected IOError variant"),
        }
    }
}
