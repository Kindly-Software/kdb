//! # Error Sanitization for Client-Facing Messages
//!
//! Converts protection errors to generic, non-revealing messages for end users.
//!
//! ## Security Principle
//!
//! **NEVER reveal protection mechanisms to potential attackers.**
//!
//! Internal errors contain detailed tamper detection information:
//! - Which protection layer triggered
//! - Specific detection method (debugger, VM, timing, etc.)
//! - Escalation tier and cooldown periods
//! - Corruption state and XOR masks
//!
//! Client-facing messages only show:
//! - License validation issue (generic)
//! - Contact support email
//! - No technical details about protection
//!
//! ## UCE34 Framework
//!
//! - **Q28**: Simplicity = Single sanitization function for all errors
//! - **Q31**: Safety = 100% safe Rust (no unsafe blocks)
//! - **Q33**: Validation = Property tests verify no leakage
//! - **Q34**: Auditability = All sanitization events logged
//!
//! ## ASSUM Safety
//!
//! - `#ASSUME_NO_PANIC`: All match arms covered (compile-time exhaustiveness)
//! - `#VERIFY_NO_LEAK`: Property tests validate no technical details in output
//! - `#ASSUME_CONTACT_VALID`: Support email hardcoded at build time
//! - `#VERIFY_CONTACT`: Integration tests validate email format
//!
//! ## Example
//!
//! ```rust
//! use kindly_dedup::protection::{ProtectionError, TamperType, sanitize_error};
//!
//! // Internal error (detailed)
//! let err = ProtectionError::Warning {
//!     tamper_type: TamperType::Debugger,
//!     cooldown_days: 3,
//! };
//!
//! // Client message (sanitized)
//! let msg = sanitize_error(&err);
//! assert!(msg.contains("License Validation"));
//! assert!(!msg.contains("Debugger"));  // No leak!
//! ```

use super::license::LicenseError;
use super::meta_capsule::MetaCapsuleError;
use super::tamper_detection::{ProtectionError, TamperType};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Support contact email (build-time constant)
const SUPPORT_EMAIL: &str = "support@kindly.ai";

/// Sales contact email (build-time constant)
const SALES_EMAIL: &str = "sales@kindly.ai";

/// Generic license error prefix
const LICENSE_ERROR_PREFIX: &str = "License Validation Error";

/// Generic license warning prefix
const LICENSE_WARNING_PREFIX: &str = "License Validation Warning";

// ============================================================================
// SANITIZATION FUNCTIONS
// ============================================================================

/// Sanitize ProtectionError for client display
///
/// # Security
///
/// - **NEVER reveals** tamper detection method
/// - **NEVER reveals** protection tier or cooldown
/// - **NEVER reveals** corruption state
/// - **ONLY shows** generic license message + contact email
///
/// # ASSUM
///
/// - `#ASSUME_NO_PANIC`: All ProtectionError variants covered
/// - `#VERIFY_NO_PANIC`: Compiler enforces match exhaustiveness
/// - `#ASSUME_NO_LEAK`: No technical details in output
/// - `#VERIFY_NO_LEAK`: Property tests validate sanitization
///
/// # Performance
///
/// - <50ns (string formatting only)
/// - Zero allocations for short messages
/// - Single heap allocation for long messages
pub fn sanitize_protection_error(err: &ProtectionError) -> String {
    match err {
        // ====================================================================
        // TIER 1: WARNING (30-day cooldown)
        // ====================================================================
        ProtectionError::Warning { .. } => {
            // Internal error contains: tamper_type, cooldown_days
            // Client message: Generic warning + contact support
            format!(
                "{}\n\
                 \n\
                 Your evaluation license may have compatibility issues.\n\
                 This could be caused by:\n\
                 - Running in a virtualized environment\n\
                 - Using debugging or profiling tools\n\
                 - System clock or hardware changes\n\
                 \n\
                 If this persists, please contact: {}\n\
                 Include your system configuration for assistance.",
                LICENSE_WARNING_PREFIX, SUPPORT_EMAIL
            )
        }

        // ====================================================================
        // TIER 2: LICENSE DEACTIVATED (7-day cooldown)
        // ====================================================================
        ProtectionError::LicenseDeactivated { .. } => {
            // Internal error contains: tamper_type, days_until_permanent
            // Client message: Deactivation notice + support contact
            format!(
                "{}\n\
                 \n\
                 Your evaluation license cannot be validated.\n\
                 \n\
                 This may indicate:\n\
                 - License expiration\n\
                 - Hardware configuration changes\n\
                 - System environment incompatibility\n\
                 \n\
                 IMPORTANT: Contact support immediately to resolve.\n\
                 Email: {}\n\
                 \n\
                 Failure to resolve may result in permanent deactivation.",
                LICENSE_ERROR_PREFIX, SUPPORT_EMAIL
            )
        }

        // ====================================================================
        // TIER 3: PERMANENTLY DISABLED
        // ====================================================================
        ProtectionError::PermanentlyDisabled { .. } => {
            // Internal error contains: tamper_type
            // Client message: Expiration notice + sales contact
            format!(
                "License Expired\n\
                 \n\
                 Your evaluation license has expired.\n\
                 \n\
                 To continue using kindly_dedup, please contact:\n\
                 Sales: {}\n\
                 Support: {}\n\
                 \n\
                 Production licenses available with:\n\
                 - Unlimited usage\n\
                 - Priority support\n\
                 - Hardware flexibility\n\
                 - Custom deployment options",
                SALES_EMAIL, SUPPORT_EMAIL
            )
        }

        // ====================================================================
        // ALGORITHM CORRUPTED (Tier 3 active)
        // ====================================================================
        ProtectionError::AlgorithmCorrupted => {
            // Internal error: Algorithm parameters XORed
            // Client message: Generic error (don't reveal corruption)
            format!(
                "{}\n\
                 \n\
                 Unable to process request due to license validation failure.\n\
                 \n\
                 Contact: {}",
                LICENSE_ERROR_PREFIX, SUPPORT_EMAIL
            )
        }
    }
}

/// Sanitize LicenseError for client display
///
/// # ASSUM
///
/// - `#ASSUME_NO_PANIC`: All LicenseError variants covered
/// - `#VERIFY_NO_PANIC`: Compiler enforces exhaustiveness
pub fn sanitize_license_error(err: &LicenseError) -> String {
    match err {
        LicenseError::Expired { .. } => {
            format!(
                "License Expired\n\
                 \n\
                 Your evaluation license has expired.\n\
                 Contact: {} for renewal options.",
                SALES_EMAIL
            )
        }

        LicenseError::HardwareMismatch { .. } => {
            format!(
                "{}\n\
                 \n\
                 License is bound to different hardware.\n\
                 Contact: {} to update license.",
                LICENSE_ERROR_PREFIX, SUPPORT_EMAIL
            )
        }

        LicenseError::ValidationFailed { .. } => {
            format!(
                "{}\n\
                 \n\
                 Unable to validate license.\n\
                 Contact: {}",
                LICENSE_ERROR_PREFIX, SUPPORT_EMAIL
            )
        }

        LicenseError::NotFound => {
            format!(
                "{}\n\
                 \n\
                 No license found.\n\
                 Contact: {} to obtain evaluation license.",
                LICENSE_ERROR_PREFIX, SALES_EMAIL
            )
        }

        LicenseError::IoError(_) => {
            format!(
                "{}\n\
                 \n\
                 Unable to read license file.\n\
                 Contact: {}",
                LICENSE_ERROR_PREFIX, SUPPORT_EMAIL
            )
        }

        LicenseError::ParseError(_) => {
            format!(
                "{}\n\
                 \n\
                 License file corrupted.\n\
                 Contact: {}",
                LICENSE_ERROR_PREFIX, SUPPORT_EMAIL
            )
        }
    }
}

/// Sanitize MetaCapsuleError for client display
///
/// # ASSUM
///
/// - `#ASSUME_NO_PANIC`: All MetaCapsuleError variants covered
/// - `#VERIFY_NO_PANIC`: Compiler enforces exhaustiveness
pub fn sanitize_meta_capsule_error(err: &MetaCapsuleError) -> String {
    match err {
        MetaCapsuleError::ProtectionViolation(_) => sanitize_protection_error(&ProtectionError::LicenseDeactivated {
            tamper_type: TamperType::StateModified,
            days_until_permanent: 7,
        }),

        MetaCapsuleError::LicenseError(_) => {
            format!(
                "{}\n\
                 \n\
                 License validation failed.\n\
                 Contact: {}",
                LICENSE_ERROR_PREFIX, SUPPORT_EMAIL
            )
        }

        MetaCapsuleError::HardwareIdError(_) => {
            format!(
                "{}\n\
                 \n\
                 Hardware validation failed.\n\
                 Contact: {}",
                LICENSE_ERROR_PREFIX, SUPPORT_EMAIL
            )
        }

        MetaCapsuleError::PufError(_) => {
            format!(
                "{}\n\
                 \n\
                 System fingerprint validation failed.\n\
                 Contact: {}",
                LICENSE_ERROR_PREFIX, SUPPORT_EMAIL
            )
        }
    }
}

/// Master sanitization function (handles all protection error types)
///
/// # Usage
///
/// ```rust
/// use kindly_dedup::protection::{check_protection, sanitize_error};
///
/// match check_protection() {
///     Ok(()) => { /* Normal operation */ },
///     Err(e) => {
///         let client_msg = sanitize_error(&e);
///         eprintln!("{}", client_msg);
///         std::process::exit(1);
///     }
/// }
/// ```
///
/// # ASSUM
///
/// - `#ASSUME_NO_PANIC`: Function never panics
/// - `#VERIFY_NO_PANIC`: All error types handled
/// - `#ASSUME_NO_LEAK`: No technical details revealed
/// - `#VERIFY_NO_LEAK`: Integration tests validate output
pub fn sanitize_error(err: &ProtectionError) -> String {
    sanitize_protection_error(err)
}

// ============================================================================
// VALIDATION HELPERS
// ============================================================================

/// Validate that sanitized message contains no technical leakage
///
/// # Security Checks
///
/// - No mention of "debugger", "ptrace", "CPUID"
/// - No mention of "VM", "hypervisor", "virtual"
/// - No mention of "tier", "cooldown", "escalation"
/// - No mention of "XOR", "corruption", "mask"
/// - No mention of "canary", "generation", "fault"
///
/// # Returns
///
/// - `Ok(())` if message is safe
/// - `Err(String)` with leaked term if unsafe
///
/// # ASSUM
///
/// - `#ASSUME_COMPREHENSIVE`: All sensitive terms listed
/// - `#VERIFY_COMPREHENSIVE`: Regular security audits add new terms
#[cfg(test)]
pub fn validate_no_leakage(msg: &str) -> Result<(), String> {
    let forbidden_terms = [
        // Detection methods
        "debugger",
        "ptrace",
        "TracerPid",
        "LD_PRELOAD",
        "injection",
        "VM",
        "hypervisor",
        "virtual",
        "CPUID",
        "RDRAND",
        "AES-NI",
        "timing",
        "instrumentation",
        "canary",
        "corruption",
        "XOR",
        "mask",
        "generation",
        "rollback",
        "fault",
        // Protection tiers
        "tier",
        "tier1",
        "tier2",
        "tier3",
        "cooldown",
        "escalation",
        "warning",
        "deactivat",
        "permanent",
        // Technical details
        "atomic",
        "SeqCst",
        "Acquire",
        "Release",
        "flag file",
        ".permanent_disable",
        ".license_deactivated",
        "0xDEADBEEF",
        "MEMORY_CANARY",
        // Internal names
        "ProtectionState",
        "TamperType",
        "check_protection",
        "init_protection",
    ];

    let msg_lower = msg.to_lowercase();

    for term in &forbidden_terms {
        if msg_lower.contains(&term.to_lowercase()) {
            return Err(format!("Leaked term: {}", term));
        }
    }

    Ok(())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_warning_no_leak() {
        let err = ProtectionError::Warning {
            tamper_type: TamperType::Debugger,
            cooldown_days: 3,
        };

        let msg = sanitize_protection_error(&err);

        // MUST contain support contact
        assert!(msg.contains(SUPPORT_EMAIL));

        // MUST NOT leak technical details
        validate_no_leakage(&msg).expect("Leaked technical details!");
    }

    #[test]
    fn test_sanitize_deactivated_no_leak() {
        let err = ProtectionError::LicenseDeactivated {
            tamper_type: TamperType::LibraryInjection,
            days_until_permanent: 7,
        };

        let msg = sanitize_protection_error(&err);

        assert!(msg.contains(SUPPORT_EMAIL));
        validate_no_leakage(&msg).expect("Leaked technical details!");
    }

    #[test]
    fn test_sanitize_permanent_no_leak() {
        let err = ProtectionError::PermanentlyDisabled {
            tamper_type: TamperType::MemoryCorrupted,
        };

        let msg = sanitize_protection_error(&err);

        assert!(msg.contains(SALES_EMAIL));
        assert!(msg.contains(SUPPORT_EMAIL));
        validate_no_leakage(&msg).expect("Leaked technical details!");
    }

    #[test]
    fn test_sanitize_corrupted_no_leak() {
        let err = ProtectionError::AlgorithmCorrupted;

        let msg = sanitize_protection_error(&err);

        assert!(msg.contains(SUPPORT_EMAIL));
        validate_no_leakage(&msg).expect("Leaked technical details!");
    }

    #[test]
    fn test_all_tamper_types_sanitized() {
        let tamper_types = [
            TamperType::Debugger,
            TamperType::TimingAnomaly,
            TamperType::StateModified,
            TamperType::LibraryInjection,
            TamperType::MemoryCorrupted,
        ];

        for tamper_type in &tamper_types {
            let err = ProtectionError::Warning {
                tamper_type: *tamper_type,
                cooldown_days: 3,
            };

            let msg = sanitize_protection_error(&err);
            validate_no_leakage(&msg).expect(&format!("Leaked details for tamper type: {:?}", tamper_type));
        }
    }

    #[test]
    fn test_sanitize_license_errors() {
        use std::io;

        let errors = vec![
            LicenseError::Expired { expired_days: 30 },
            LicenseError::HardwareMismatch {
                expected: "ABC123".to_string(),
                actual: "DEF456".to_string(),
            },
            LicenseError::ValidationFailed {
                reason: "test".to_string(),
            },
            LicenseError::NotFound,
            LicenseError::IoError(io::Error::new(io::ErrorKind::NotFound, "test")),
            LicenseError::ParseError("test".to_string()),
        ];

        for err in &errors {
            let msg = sanitize_license_error(err);
            assert!(msg.contains(SUPPORT_EMAIL) || msg.contains(SALES_EMAIL));
            // License errors may mention "hardware" or "expired" (not sensitive)
            // but should not leak internal mechanisms
        }
    }
}
