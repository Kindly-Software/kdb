//! Error types for syndrome extraction

use core::fmt;

/// Result type for syndrome extraction operations
pub type SyndromeResult<T> = Result<T, SyndromeError>;

/// Errors that can occur during syndrome extraction
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyndromeError {
    /// Invalid state vector (length must be power of 2)
    InvalidStateVector {
        length: usize,
        expected: usize,
    },

    /// Parity violation detected (even parity required)
    ParityViolation {
        syndrome_len: usize,
        ones_count: usize,
    },

    /// Unsupported code distance
    UnsupportedDistance(usize),

    /// SIMD not available on this platform
    SimdUnavailable,

    /// Stabilizer generation failed
    StabilizerGenerationFailed {
        distance: usize,
        reason: &'static str,
    },

    /// Invalid qubit count
    InvalidQubitCount {
        got: usize,
        expected: usize,
    },
}

impl fmt::Display for SyndromeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStateVector { length, expected } => {
                write!(
                    f,
                    "Invalid state vector: length {} (expected {})",
                    length, expected
                )
            }
            Self::ParityViolation {
                syndrome_len,
                ones_count,
            } => {
                write!(
                    f,
                    "Parity violation: syndrome length {}, ones count {} (must be even)",
                    syndrome_len, ones_count
                )
            }
            Self::UnsupportedDistance(d) => {
                write!(f, "Unsupported code distance: {} (supported: 3, 5, 7)", d)
            }
            Self::SimdUnavailable => {
                write!(f, "SIMD not available on this platform")
            }
            Self::StabilizerGenerationFailed { distance, reason } => {
                write!(
                    f,
                    "Stabilizer generation failed for distance {}: {}",
                    distance, reason
                )
            }
            Self::InvalidQubitCount { got, expected } => {
                write!(
                    f,
                    "Invalid qubit count: got {}, expected {}",
                    got, expected
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SyndromeError {}

// ASSUM Safety Tags
//
// #ASSUME_ERROR_DISPLAY_NO_PANIC
// Assumption: Display implementation never panics
// Verification: All branches return Result, no unwrap/panic
// Status: ✅ Verified

// #ASSUME_ERROR_CLONE_CHEAP
// Assumption: Clone is cheap (no heap allocation in error variants)
// Verification: All fields are Copy or static references
// Status: ✅ Verified
