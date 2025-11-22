//! Error Types for Auditable Capsules
//!
//! Rich error context for debugging and forensic analysis.

use core::fmt;

#[cfg(feature = "std")]
use std::error::Error;

#[cfg(feature = "std")]
use std::io;

/// Errors that can occur during audit trail operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditError {
    /// Hash chain verification failed
    ///
    /// # Fields
    /// - `pos`: Position in chain where mismatch occurred
    /// - `expected`: Expected hash value
    /// - `actual`: Actual hash value found
    ChainMismatch {
        /// Position in chain (0-indexed)
        pos: usize,
        /// Expected hash value (hex-formatted)
        expected: String,
        /// Actual hash value (hex-formatted)
        actual: String,
    },

    /// Integrity check failed (hash doesn't match recomputed value)
    ///
    /// # Use Case
    /// Detect unauthorized state modifications
    IntegrityFailed {
        /// Expected hash (from current state)
        expected: u64,
        /// Actual hash (stored)
        actual: u64,
    },

    /// Keyed HMAC verification failed
    ///
    /// # Use Case
    /// Cryptographic authentication of audit trail
    #[cfg(feature = "audit-trail")]
    KeyedHmacFailed {
        /// HMAC value that failed verification
        hmac: [u8; 32],
    },

    /// Generation counter anomaly detected
    ///
    /// # Use Case
    /// Detect missing or out-of-order updates
    GenerationAnomaly {
        /// Expected generation counter
        expected: u64,
        /// Actual generation counter
        actual: u64,
    },

    /// Timestamp anomaly (non-monotonic or zero)
    ///
    /// # Use Case
    /// Detect time-travel attacks or clock manipulation
    TimestampAnomaly {
        /// Previous timestamp
        prev_timestamp_ns: u64,
        /// Current timestamp (invalid)
        curr_timestamp_ns: u64,
    },

    /// I/O error during audit operations
    ///
    /// # Use Case
    /// File system operations for persistent audit trail
    #[cfg(feature = "std")]
    Io(String),
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditError::ChainMismatch {
                pos,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Chain verification failed at position {}: expected {}, got {}",
                    pos, expected, actual
                )
            }
            AuditError::IntegrityFailed { expected, actual } => {
                write!(
                    f,
                    "Integrity check failed: hash mismatch (expected {:016x}, got {:016x})",
                    expected, actual
                )
            }
            #[cfg(feature = "audit-trail")]
            AuditError::KeyedHmacFailed { hmac } => {
                write!(f, "Keyed HMAC verification failed: {:02x?}", &hmac[..8])
            }
            AuditError::GenerationAnomaly { expected, actual } => {
                write!(
                    f,
                    "Generation counter anomaly: expected {}, got {}",
                    expected, actual
                )
            }
            AuditError::TimestampAnomaly {
                prev_timestamp_ns,
                curr_timestamp_ns,
            } => {
                write!(
                    f,
                    "Timestamp anomaly: prev {} > curr {}",
                    prev_timestamp_ns, curr_timestamp_ns
                )
            }
            #[cfg(feature = "std")]
            AuditError::Io(msg) => {
                write!(f, "I/O error: {}", msg)
            }
        }
    }
}

#[cfg(feature = "std")]
impl Error for AuditError {}

/// Errors that can occur during encrypted state operations
#[cfg(feature = "encrypted-state")]
#[derive(Debug)]
pub enum StateError {
    /// I/O error during file operations
    Io(io::Error),

    /// Invalid file size
    InvalidFileSize { expected: usize, actual: usize },

    /// Invalid file magic
    InvalidMagic { expected: u64, actual: u64 },

    /// Encryption failed
    EncryptionFailed,

    /// Decryption failed (authentication tag mismatch)
    DecryptionFailed,

    /// Key derivation failed
    KeyDerivationFailed,

    /// Mmap not initialized
    MmapNotInitialized,

    /// Insufficient space in mmap region
    InsufficientSpace { required: usize, available: usize },
}

#[cfg(feature = "encrypted-state")]
impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::Io(e) => write!(f, "I/O error: {}", e),
            StateError::InvalidFileSize { expected, actual } => {
                write!(
                    f,
                    "Invalid file size: expected at least {} bytes, got {}",
                    expected, actual
                )
            }
            StateError::InvalidMagic { expected, actual } => {
                write!(
                    f,
                    "Invalid file magic: expected 0x{:016x}, got 0x{:016x}",
                    expected, actual
                )
            }
            StateError::EncryptionFailed => write!(f, "Encryption failed"),
            StateError::DecryptionFailed => write!(f, "Decryption failed (authentication tag mismatch)"),
            StateError::KeyDerivationFailed => write!(f, "Key derivation failed"),
            StateError::MmapNotInitialized => write!(f, "Mmap region not initialized"),
            StateError::InsufficientSpace { required, available } => {
                write!(
                    f,
                    "Insufficient space: required {} bytes, available {}",
                    required, available
                )
            }
        }
    }
}

/// Errors that can occur during protection orchestration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectionError {
    /// Multiple layers failed (≥3 layers)
    LayersFailed {
        /// Number of layers that failed
        count: usize,
    },

    /// Critical layer (P0: layers 0-2) failed
    CriticalLayerFailed {
        /// Layer index that failed (0=BuildHardening, 1=CryptoLicense, 2=EncryptedState)
        layer: usize,
    },

    /// Layer check timeout
    LayerTimeout {
        /// Layer index that timed out
        layer: usize,
        /// Timeout duration in milliseconds
        timeout_ms: u64,
    },

    /// Invalid layer index
    InvalidLayer {
        /// Layer index provided
        layer: usize,
        /// Maximum valid layer index
        max: usize,
    },
}

impl fmt::Display for ProtectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtectionError::LayersFailed { count } => {
                write!(
                    f,
                    "Protection compromised: {} layers failed (threshold: 3)",
                    count
                )
            }
            ProtectionError::CriticalLayerFailed { layer } => {
                let layer_name = match layer {
                    0 => "BuildHardening",
                    1 => "CryptoLicense",
                    2 => "EncryptedState",
                    _ => "Unknown",
                };
                write!(
                    f,
                    "Critical layer {} ({}) failed - immediate block",
                    layer, layer_name
                )
            }
            ProtectionError::LayerTimeout { layer, timeout_ms } => {
                write!(
                    f,
                    "Layer {} check timed out after {}ms",
                    layer, timeout_ms
                )
            }
            ProtectionError::InvalidLayer { layer, max } => {
                write!(f, "Invalid layer index {} (max: {})", layer, max)
            }
        }
    }
}

#[cfg(feature = "std")]
impl Error for ProtectionError {}

#[cfg(feature = "encrypted-state")]
impl Error for StateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            StateError::Io(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_mismatch_display() {
        let err = AuditError::ChainMismatch {
            pos: 42,
            expected: "0xDEADBEEF".to_string(),
            actual: "0xBADC0FFE".to_string(),
        };

        let display = format!("{}", err);
        assert!(display.contains("position 42"));
        assert!(display.contains("0xDEADBEEF"));
        assert!(display.contains("0xBADC0FFE"));
    }

    #[test]
    fn test_integrity_failed_display() {
        let err = AuditError::IntegrityFailed {
            expected: 0xDEADBEEF,
            actual: 0xBADC0FFE,
        };

        let display = format!("{}", err);
        assert!(display.contains("deadbeef"));
        assert!(display.contains("badc0ffe"));
    }

    #[test]
    fn test_generation_anomaly_display() {
        let err = AuditError::GenerationAnomaly {
            expected: 100,
            actual: 50,
        };

        let display = format!("{}", err);
        assert!(display.contains("expected 100"));
        assert!(display.contains("got 50"));
    }

    #[test]
    fn test_timestamp_anomaly_display() {
        let err = AuditError::TimestampAnomaly {
            prev_timestamp_ns: 1000,
            curr_timestamp_ns: 500,
        };

        let display = format!("{}", err);
        assert!(display.contains("1000"));
        assert!(display.contains("500"));
    }

    #[test]
    fn test_error_equality() {
        let err1 = AuditError::IntegrityFailed {
            expected: 100,
            actual: 200,
        };
        let err2 = AuditError::IntegrityFailed {
            expected: 100,
            actual: 200,
        };
        assert_eq!(err1, err2);
    }
}

/// Errors that can occur during memory encryption operations
#[cfg(feature = "memory-encryption")]
#[derive(Debug)]
pub enum MemoryError {
    /// Platform not available (SGX/SEV-SNP/Secure Enclave not detected)
    PlatformNotAvailable {
        /// Platform name
        platform: &'static str,
        /// Reason for unavailability
        reason: &'static str,
    },

    /// Invalid memory region size
    InvalidSize {
        /// Size provided
        size: usize,
        /// Reason for invalidity
        reason: &'static str,
    },

    /// Memory allocation failed
    AllocationFailed {
        /// Size that failed to allocate
        size: usize,
    },

    /// Memory lock failed (mlock)
    LockFailed {
        /// Reason for failure
        reason: &'static str,
    },

    /// Memory protection failed (mprotect)
    ProtectionFailed {
        /// Operation that failed
        operation: &'static str,
    },

    /// Wrong platform for operation
    WrongPlatform {
        /// Expected platform
        expected: &'static str,
        /// Actual platform
        actual: &'static str,
    },

    /// Memory region not initialized
    RegionNotInitialized,

    /// Insufficient space in memory region
    InsufficientSpace {
        /// Required space
        required: usize,
        /// Available space
        available: usize,
    },

    /// SGX sealing failed
    #[cfg(all(target_feature = "sgx", feature = "sgx-enclave"))]
    SgxSealFailed {
        /// Error code from SGX SDK
        code: u32,
    },

    /// SGX unsealing failed
    #[cfg(all(target_feature = "sgx", feature = "sgx-enclave"))]
    SgxUnsealFailed {
        /// Error code from SGX SDK
        code: u32,
    },

    /// SEV-SNP attestation failed
    #[cfg(all(target_arch = "x86_64", target_feature = "sev", feature = "sev-snp"))]
    SevAttestationFailed {
        /// Error message
        message: String,
    },

    /// Secure Enclave operation failed
    #[cfg(all(target_os = "macos", feature = "secure-enclave"))]
    SecureEnclaveFailed {
        /// Error message
        message: String,
    },
}

#[cfg(feature = "memory-encryption")]
impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryError::PlatformNotAvailable { platform, reason } => {
                write!(f, "Platform {} not available: {}", platform, reason)
            }
            MemoryError::InvalidSize { size, reason } => {
                write!(f, "Invalid size {} bytes: {}", size, reason)
            }
            MemoryError::AllocationFailed { size } => {
                write!(f, "Memory allocation failed for {} bytes", size)
            }
            MemoryError::LockFailed { reason } => {
                write!(f, "Memory lock failed: {}", reason)
            }
            MemoryError::ProtectionFailed { operation } => {
                write!(f, "Memory protection failed: {}", operation)
            }
            MemoryError::WrongPlatform { expected, actual } => {
                write!(
                    f,
                    "Wrong platform: expected {}, got {}",
                    expected, actual
                )
            }
            MemoryError::RegionNotInitialized => {
                write!(f, "Memory region not initialized")
            }
            MemoryError::InsufficientSpace { required, available } => {
                write!(
                    f,
                    "Insufficient space: required {} bytes, available {}",
                    required, available
                )
            }
            #[cfg(all(target_feature = "sgx", feature = "sgx-enclave"))]
            MemoryError::SgxSealFailed { code } => {
                write!(f, "SGX seal failed with code 0x{:08x}", code)
            }
            #[cfg(all(target_feature = "sgx", feature = "sgx-enclave"))]
            MemoryError::SgxUnsealFailed { code } => {
                write!(f, "SGX unseal failed with code 0x{:08x}", code)
            }
            #[cfg(all(target_arch = "x86_64", target_feature = "sev", feature = "sev-snp"))]
            MemoryError::SevAttestationFailed { message } => {
                write!(f, "SEV-SNP attestation failed: {}", message)
            }
            #[cfg(all(target_os = "macos", feature = "secure-enclave"))]
            MemoryError::SecureEnclaveFailed { message } => {
                write!(f, "Secure Enclave failed: {}", message)
            }
        }
    }
}

#[cfg(all(feature = "std", feature = "memory-encryption"))]
impl Error for MemoryError {}
