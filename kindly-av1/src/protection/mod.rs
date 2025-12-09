//! 11-Layer IP Protection System for kindly-av1
//!
//! ## Architecture Overview
//!
//! The protection system consists of 11 layers divided into 3 priority tiers:
//!
//! ### P0 Foundation (Layers 0-2) - MANDATORY
//! - Layer 0: Build Hardening (symbol stripping, LTO, PGO)
//! - Layer 1: Crypto License (AES-256-GCM hardware binding)
//! - Layer 2: Encrypted State (runtime parameter encryption)
//!
//! ### P1 Advanced (Layers 3-6) - RECOMMENDED
//! - Layer 3: Remote Attestation (server-side validation)
//! - Layer 4: TPM Binding (hardware root of trust)
//! - Layer 5: Obfuscation (control flow flattening, nightly)
//! - Layer 6: PUF/Fuzzy Extractor (hardware entropy)
//!
//! ### P2 Enterprise (Layers 7-10) - OPTIONAL
//! - Layer 7: Anomaly Detector (behavioral analysis)
//! - Layer 8: Memory Encryption (runtime data protection)
//! - Layer 9: Kernel Protection (syscall filtering)
//! - Layer 10: Observability (Q34 audit trails)
//!
//! ## UCE34 Framework
//! - Q10: Tier = T0 Foundation (build-time) + T1 Atomic (runtime checks)
//! - Q11: Rust = 100% safe (zero unsafe in protection layer wrappers)
//! - Q12: Nightly = Optional (Layer 5 obfuscation only)
//! - Q28: Simplicity = Layered architecture, progressive activation
//! - Q33: Validation = Hardware ID consistency, license signature verification
//! - Q34: Auditability = Hash-chained audit trail (Layer 10)
//!
//! ## Chaos Compliance
//! - 100% lockfree (AtomicU64, DualAtomicU64, no mutex/RwLock)
//! - Cache-aligned capsules (64B/128B/256B)
//! - Generation counters for state versioning
//! - Acquire/Release memory ordering
//!
//! ## Example
//! ```rust
//! use kindly_av1::protection::{HardwareIdCapsule, ProtectionError};
//!
//! // Layer 1: Hardware binding
//! let hw_id = HardwareIdCapsule::derive()?;
//! println!("Hardware ID: {:?}", &hw_id.fingerprint()[0..8]);
//!
//! // Validate on subsequent runs
//! hw_id.validate()?;
//! ```

#![allow(dead_code)]

// ============================================================================
// MODULE DECLARATIONS
// ============================================================================

pub mod hardware_id;
pub mod audit;
pub mod tamper_detection;
pub mod hardware_ban;

// ============================================================================
// KINDLY BAN MESSAGE
// ============================================================================

/// Kindly ban message displayed when hardware is banned
/// Shows appeal contact with purple heart emoji
pub const BAN_MESSAGE: &str = r#"
💜 kindly-av1 has detected tampering and output may be corrupted.

If you believe this is a mistake, please contact:
  samuel@kindly.software

Include your hardware ID and we'll investigate.
Thank you for supporting kindly-av1! 💜
"#;

/// Support appeal email
pub const SUPPORT_EMAIL: &str = "samuel@kindly.software";

// P0 Protection Layers (MANDATORY, stable)
#[cfg(feature = "protection-build-hardening")]
pub mod build_hardening;

#[cfg(feature = "protection-crypto-license")]
pub mod crypto_license;

#[cfg(feature = "protection-encrypted-state")]
pub mod encrypted_state;

// P1 Protection Layers (RECOMMENDED, stable)
#[cfg(feature = "protection-remote-attestation")]
pub mod remote_attestation;

#[cfg(feature = "protection-tpm")]
pub mod tpm_binding;

#[cfg(feature = "protection-obfuscation")]
pub mod obfuscation;

#[cfg(feature = "protection-puf")]
pub mod puf;

// P2 Protection Layers (OPTIONAL, stable)
#[cfg(feature = "protection-anomaly")]
pub mod anomaly_detector;

#[cfg(feature = "protection-memory-encryption")]
pub mod memory_encryption;

#[cfg(feature = "protection-kernel")]
pub mod kernel_protection;

#[cfg(feature = "protection-observability")]
pub mod observability;

// ============================================================================
// LAYER CONSTANTS
// ============================================================================

/// Layer 0: Build Hardening (symbol stripping, LTO, PGO)
pub const LAYER_BUILD_HARDENING: u8 = 0;

/// Layer 1: Crypto License (AES-256-GCM hardware binding)
pub const LAYER_CRYPTO_LICENSE: u8 = 1;

/// Layer 2: Encrypted State (runtime parameter encryption)
pub const LAYER_ENCRYPTED_STATE: u8 = 2;

/// Layer 3: Remote Attestation (server-side validation)
pub const LAYER_REMOTE_ATTESTATION: u8 = 3;

/// Layer 4: TPM Binding (hardware root of trust)
pub const LAYER_TPM_BINDING: u8 = 4;

/// Layer 5: Obfuscation (control flow flattening, nightly)
pub const LAYER_OBFUSCATION: u8 = 5;

/// Layer 6: PUF/Fuzzy Extractor (hardware entropy)
pub const LAYER_FUZZY_EXTRACTOR: u8 = 6;

/// Layer 7: Anomaly Detector (behavioral analysis)
pub const LAYER_ANOMALY_DETECTOR: u8 = 7;

/// Layer 8: Memory Encryption (runtime data protection)
pub const LAYER_MEMORY_ENCRYPTION: u8 = 8;

/// Layer 9: Kernel Protection (syscall filtering)
pub const LAYER_KERNEL_PROTECTION: u8 = 9;

/// Layer 10: Observability (Q34 audit trails)
pub const LAYER_OBSERVABILITY: u8 = 10;

/// Total number of protection layers
pub const NUM_LAYERS: usize = 11;

// ============================================================================
// PUBLIC RE-EXPORTS
// ============================================================================

pub use hardware_id::{HardwareIdCapsule, HardwareIdError};

// Audit trail exports (Q34 compliance)
pub use audit::{
    log_security_event, SecurityEventType, TamperType, AuditError,
    verify_audit_trail, audit_event_count, current_audit_hash,
};

// Tamper detection exports
pub use tamper_detection::{
    TamperDetectionCapsule, run_tamper_detection, get_escalation_tier,
    get_corruption_mask, init_tamper_detection, method_name,
    method_explanation, method_dev_instructions,
};

// Hardware ban exports
pub use hardware_ban::{
    HardwareBanCapsule, BanError, is_banned, ban_hardware,
    generate_support_code, apply_reset_code, load_ban_list, save_ban_list,
};

#[cfg(feature = "protection-build-hardening")]
pub use build_hardening::BuildHardeningCapsule;

#[cfg(feature = "protection-crypto-license")]
pub use crypto_license::CryptoLicenseCapsule;

#[cfg(feature = "protection-encrypted-state")]
pub use encrypted_state::EncryptedStateCapsule;

#[cfg(feature = "protection-remote-attestation")]
pub use remote_attestation::RemoteAttestationCapsule;

#[cfg(feature = "protection-tpm")]
pub use tpm_binding::TpmBindingCapsule;

#[cfg(feature = "protection-obfuscation")]
pub use obfuscation::ObfuscationCapsule;

#[cfg(feature = "protection-puf")]
pub use puf::PufCapsule;

#[cfg(feature = "protection-anomaly")]
pub use anomaly_detector::AnomalyDetectorCapsule;

#[cfg(feature = "protection-memory-encryption")]
pub use memory_encryption::MemoryEncryptionCapsule;

#[cfg(feature = "protection-kernel")]
pub use kernel_protection::KernelProtectionCapsule;

#[cfg(feature = "protection-observability")]
pub use observability::ObservabilityCapsule;

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Protection error variants
#[derive(Debug)]
pub enum ProtectionError {
    /// Hardware ID mismatch (binary copied to different machine)
    HardwareIdMismatch { expected: [u8; 32], actual: [u8; 32] },

    /// License validation failed
    LicenseInvalid,

    /// License expired
    LicenseExpired,

    /// Remote attestation failed
    AttestationFailed,

    /// TPM binding failed
    TpmBindingFailed,

    /// PUF extraction failed
    PufExtractionFailed,

    /// Anomaly detected (behavioral analysis)
    AnomalyDetected,

    /// Memory encryption initialization failed
    MemoryEncryptionFailed,

    /// Kernel protection initialization failed
    KernelProtectionFailed,

    /// Audit trail corruption detected
    AuditTrailCorrupted,

    /// Hardware is permanently banned (Tier 4 self-destruct)
    HardwareBanned {
        hardware_id: [u8; 32],
        reason: u8,
        banned_at: u64,
    },

    /// Support reset code already used
    ResetCodeUsed,

    /// Invalid support reset code
    InvalidResetCode,

    /// Ban file I/O error
    BanFileError(String),

    /// Generic protection error
    Generic(String),
}

impl std::fmt::Display for ProtectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtectionError::HardwareIdMismatch { expected, actual } => {
                write!(
                    f,
                    "Hardware ID mismatch (expected: {:?}, actual: {:?})",
                    &expected[0..8],
                    &actual[0..8]
                )
            }
            ProtectionError::LicenseInvalid => write!(f, "License validation failed"),
            ProtectionError::LicenseExpired => write!(f, "License expired"),
            ProtectionError::AttestationFailed => write!(f, "Remote attestation failed"),
            ProtectionError::TpmBindingFailed => write!(f, "TPM binding failed"),
            ProtectionError::PufExtractionFailed => write!(f, "PUF extraction failed"),
            ProtectionError::AnomalyDetected => write!(f, "Anomaly detected"),
            ProtectionError::MemoryEncryptionFailed => {
                write!(f, "Memory encryption initialization failed")
            }
            ProtectionError::KernelProtectionFailed => {
                write!(f, "Kernel protection initialization failed")
            }
            ProtectionError::AuditTrailCorrupted => write!(f, "Audit trail corruption detected"),
            ProtectionError::HardwareBanned { hardware_id, reason, banned_at } => {
                let reason_name = method_name(*reason);
                let explanation = method_explanation(*reason);
                let instructions = method_dev_instructions(*reason);

                write!(
                    f,
                    "Hardware banned due to {} (ID: {}, banned at: {})\n\
                     \n\
                     What triggered this ban:\n\
                     {}\n\
                     \n\
                     What you can do:\n\
                     {}\n\
                     \n\
                     To appeal this ban:\n\
                     - Email {} with your hardware ID\n\
                     - Include details about what you were doing when banned\n\
                     - We'll investigate and may provide a reset code if legitimate\n\
                     \n\
                     {}",
                    reason_name,
                    hex::encode(&hardware_id[..8]),
                    banned_at,
                    explanation,
                    instructions,
                    SUPPORT_EMAIL,
                    BAN_MESSAGE
                )
            }
            ProtectionError::ResetCodeUsed => write!(f, "Support reset code already used"),
            ProtectionError::InvalidResetCode => write!(f, "Invalid support reset code"),
            ProtectionError::BanFileError(msg) => write!(f, "Ban file error: {}", msg),
            ProtectionError::Generic(msg) => write!(f, "Protection error: {}", msg),
        }
    }
}

impl std::error::Error for ProtectionError {}

impl From<HardwareIdError> for ProtectionError {
    fn from(err: HardwareIdError) -> Self {
        match err {
            HardwareIdError::Mismatch { expected, actual } => {
                ProtectionError::HardwareIdMismatch { expected, actual }
            }
            HardwareIdError::CpuSerialFailed => {
                ProtectionError::Generic("CPU serial extraction failed".to_string())
            }
        }
    }
}

impl From<hardware_ban::BanError> for ProtectionError {
    fn from(err: hardware_ban::BanError) -> Self {
        match err {
            hardware_ban::BanError::IoError(e) => ProtectionError::BanFileError(e.to_string()),
            hardware_ban::BanError::CryptoError => ProtectionError::BanFileError("Encryption error".to_string()),
            hardware_ban::BanError::InvalidFormat => ProtectionError::BanFileError("Invalid format".to_string()),
            hardware_ban::BanError::AlreadyBanned => ProtectionError::Generic("Already banned".to_string()),
            hardware_ban::BanError::ResetAlreadyUsed => ProtectionError::ResetCodeUsed,
            hardware_ban::BanError::InvalidResetCode => ProtectionError::InvalidResetCode,
            hardware_ban::BanError::IntegrityCheckFailed => ProtectionError::BanFileError("Integrity check failed".to_string()),
        }
    }
}
