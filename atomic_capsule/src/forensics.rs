//! Forensics Module - Compliance and Audit Trail Infrastructure
//!
//! # Compliance Frameworks Supported
//!
//! - **SOX (Sarbanes-Oxley)**: Transaction audit trails, 7-year retention, tampering detection
//! - **GDPR (General Data Protection)**: PII detection/redaction, right to be forgotten
//! - **SOC2 Type II**: Change control evidence, timestamp verification, non-repudiation
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │   Forensics Module (Tier 1: Atomic)         │
//! ├─────────────────────────────────────────────┤
//! │ • SOX Transaction ID (monotonic, <100ns)    │
//! │ • 7-year retention policy enforcement       │
//! │ • PII detection & redaction (GDPR)          │
//! │ • Right-to-be-forgotten tracking            │
//! │ • SOC2 timestamp verification               │
//! │ • Tampering detection & forensic analysis   │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! # Q10-Q12 UCE34 Analysis
//!
//! **Q10 (Capsule Tier)**: Tier 1 (Atomic) - Transaction ID generation requires lockfree,
//! monotonic counter with <100ns latency. All compliance fields (timestamps, retention dates)
//! are read-only after creation, no complex coordination needed.
//!
//! **Q11 (Rust Transform)**: Atomic capsule design with:
//! - AtomicU64 counter for SOX transaction IDs (SeqCst ordering for global monotonicity)
//! - Typed compliance structures (SoxTransactionId, RetentionPolicy, PiiRedacter)
//! - Zero-copy snapshot creation for audit trails
//!
//! **Q12 (Nightly Enhancement)**: Could leverage const_trait_impl for compile-time
//! validation of retention policies, but not critical for MVP. Future optimization.
//!
//! # ASSUM Safety Tags
//!
//! - `#ASSUME_MONOTONIC_SOX_ID`: SeqCst ordering guarantees monotonic transaction IDs
//! - `#VERIFY_MONOTONIC_SOX_ID`: ThreadSanitizer + stress tests validate no duplicates
//! - `#ASSUME_TIMESTAMP_ACCURACY`: SystemTime::now() is monotonic and accurate
//! - `#VERIFY_TIMESTAMP_ACCURACY`: Property tests validate timestamp ordering
//! - `#ASSUME_PII_PATTERNS_COMPLETE`: Regex patterns cover standard PII types
//! - `#VERIFY_PII_PATTERNS_COMPLETE`: Manual audit + test suite validate detection

pub mod compliance;
pub mod pii_redaction;
pub mod retention_policy;
pub mod right_to_forget;
pub mod sox_transaction_id;
pub mod timestamp_verification;

// Re-exports for convenience
pub use compliance::ComplianceFramework;
pub use pii_redaction::{PiiDetector, PiiRedacter, PiiType};
pub use retention_policy::RetentionPolicy;
pub use right_to_forget::{ForgetReason, ForgetRequest, ForgetStatus};
pub use sox_transaction_id::SoxTransactionId;
pub use timestamp_verification::Timestamp;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forensics_module_structure() {
        // Structural test - ensure all modules compile
        let _ = SoxTransactionId::next();
        let _ = RetentionPolicy::sox_compliant();
        let _ = Timestamp::now();
    }
}
