//! Binary protection infrastructure for kindly_dedup
//!
//! ## Layer 1: Build-Time Protection (Current)
//! - Customer-specific compilation (unique CUSTOMER_ID)
//! - Binary signing (SHA-256 signature)
//! - Symbol stripping (IP protection)
//! - Build audit trail (Q34 compliance)
//!
//! ## Layer 2: Weaponized Circuit Breaker + Encryption (Implemented - Phase 2.4.1)
//! - Continuous tamper detection (<12ns overhead)
//! - Escalating response (WARNING → DEGRADE → CORRUPT → NUKE)
//! - 5 tamper checks (debugger, timing, state, injection, canary)
//! - AES-256-GCM encryption (algorithm parameter protection, <1µs overhead)
//! - RDRAND nonce generation (hardware RNG, cryptographically secure)
//!
//! ## Future Layers (Planned)
//! - Layer 3: License Enforcement (online validation, hardware binding)
//! - Layer 4: Security Audit Trail (hash-chained logging)
//!
//! ## UCE34 Framework
//! - Q10: Tier = T0 Foundation (encryption) + T1 Atomic (build-time constants)
//! - Q28: Simplicity = AES-256-GCM only (NIST-approved standard)
//! - Q29: Dependencies = aes-gcm crate (RustCrypto, well-audited)
//! - Q33: Validation = NIST SP 800-38D test vectors
//! - Q34: Auditability = Build audit trail (build_audit.log)
//!
//! ## Example
//! ```rust
//! use kindly_dedup::protection::{BuildVerification, check_protection, init_protection};
//!
//! // Layer 1: Build-time verification
//! let build_info = BuildVerification::get();
//! println!("Customer ID: {}", build_info.customer_id());
//! println!("Build Signature: {}", build_info.build_signature());
//! assert!(build_info.verify_integrity());
//!
//! // Layer 2: Runtime protection (feature-gated)
//! #[cfg(feature = "binary-protection")]
//! {
//!     init_protection();
//!     check_protection().expect("Tamper detected");
//! }
//! ```

#![allow(dead_code)]

pub mod audit;
pub mod background_monitor;
pub mod build_verification;
pub mod commercial_limiter;
pub mod dedup_audit;
pub mod demo_limiter;
pub mod encryption;
pub mod hardware_id;
pub mod license;
pub mod meta_capsule;
pub mod puf;
pub mod status_capsule;
pub mod tamper_detection;

// P0 Protection Capsule Wrappers (Phase P0 Integration)
#[cfg(feature = "protection-build-hardening")]
pub mod build_hardening_wrapper;
#[cfg(feature = "protection-crypto-license")]
pub mod crypto_license_wrapper;
#[cfg(feature = "protection-encrypted-state")]
pub mod encrypted_state_wrapper;

// P1 Protection Wrappers (Phase P1 Integration)
#[cfg(feature = "nightly")]
pub mod obfuscation_wrapper;
#[cfg(feature = "remote-attestation")]
pub mod remote_attestation_wrapper;

// P2 Protection System + Wrappers (Phase P2 Integration)
#[cfg(feature = "anomaly-detector")]
pub mod anomaly_detector_wrapper;
#[cfg(feature = "kernel-protection")]
pub mod kernel_protection_wrapper;
#[cfg(feature = "memory-encryption")]
pub mod memory_encryption_wrapper;
pub mod protection_system;

pub use audit::{AuditError, SecurityAuditEvent, SecurityEventType};
pub use background_monitor::{is_running, shutdown_monitor, spawn_monitor};
pub use build_verification::BuildVerification;
pub use commercial_limiter::{CommercialLimitError, CommercialLimiterCapsule, LicenseTier};
pub use dedup_audit::{
    log_add_document, log_bloom_skip, log_cluster_formed, log_find_duplicate, DedupAuditEvent, DedupEventType,
};
pub use demo_limiter::{DemoLimitError, DemoLimiter};
pub use encryption::{AlgorithmConfig, EncryptedConfig, EncryptionError};
pub use hardware_id::{HardwareId, HardwareIdError};
pub use license::{LicenseError, LicenseStatus, LicenseValidator};
pub use meta_capsule::{DedupMetaCapsule, MetaCapsuleError};
pub use puf::{PufEntropy, PufError};
pub use status_capsule::{
    ProtectionStatusCapsule, PROTECTION_BLOCKED, PROTECTION_DEGRADED, PROTECTION_FAILED, PROTECTION_OK,
    PROTECTION_STATUS, PROTECTION_WARNING,
};
pub use tamper_detection::{check_protection, get_corruption_mask, init_protection, ProtectionError, TamperType};

// P0 Protection Capsule Wrapper Exports
#[cfg(feature = "protection-build-hardening")]
pub use build_hardening_wrapper::BuildHardeningWrapper;
#[cfg(feature = "protection-crypto-license")]
pub use crypto_license_wrapper::{CryptoLicenseWrapper, LicenseData};
#[cfg(feature = "protection-encrypted-state")]
pub use encrypted_state_wrapper::{EncryptedStateWrapper, StateError};

// P1 Protection Wrapper Exports
#[cfg(feature = "nightly")]
pub use obfuscation_wrapper::ObfuscationWrapper;
#[cfg(feature = "remote-attestation")]
pub use remote_attestation_wrapper::RemoteAttestationWrapper;

// P2 Protection System + Wrapper Exports
#[cfg(feature = "anomaly-detector")]
pub use anomaly_detector_wrapper::AnomalyDetectorWrapper;
#[cfg(feature = "kernel-protection")]
pub use kernel_protection_wrapper::KernelProtectionWrapper;
#[cfg(feature = "memory-encryption")]
pub use memory_encryption_wrapper::MemoryEncryptionWrapper;
pub use protection_system::{
    ProtectionSystem, LAYER_ANOMALY_DETECTOR, LAYER_BUILD_HARDENING, LAYER_CRYPTO_LICENSE, LAYER_ENCRYPTED_STATE,
    LAYER_FUZZY_EXTRACTOR, LAYER_KERNEL_PROTECTION, LAYER_MEMORY_ENCRYPTION, LAYER_OBFUSCATION, LAYER_OBSERVABILITY,
    LAYER_REMOTE_ATTESTATION, LAYER_TPM_BINDING, NUM_LAYERS,
};
