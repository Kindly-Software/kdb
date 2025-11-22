//! Compliance Framework - Unified SOX/GDPR/SOC2 Compliance
//!
//! # Supported Frameworks
//!
//! - **SOX (Sarbanes-Oxley)**: Financial audit trails, transaction IDs, 7-year retention
//! - **GDPR (General Data Protection)**: PII redaction, right to be forgotten
//! - **SOC2 Type II**: Change control, timestamp verification, observation period
//!
//! # Architecture
//!
//! ComplianceFramework provides unified API for all compliance requirements:
//! - Transaction ID generation (SOX)
//! - Retention policy enforcement (SOX)
//! - PII detection and redaction (GDPR)
//! - Forget request tracking (GDPR)
//! - Timestamp verification (SOC2)
//!
//! # Example
//!
//! ```
//! use atomic_capsule::forensics::ComplianceFramework;
//!
//! let framework = ComplianceFramework::new();
//!
//! // SOX: Generate transaction ID
//! let tx_id = framework.new_transaction_id();
//!
//! // GDPR: Redact PII
//! let safe_text = framework.redact_pii("Email: john@example.com");
//! assert!(!safe_text.contains("john@example.com"));
//!
//! // SOC2: Verify timestamp
//! let ts = framework.current_timestamp();
//! assert!(framework.verify_timestamp_soc2(&ts).is_ok());
//! ```

use super::pii_redaction::{PiiDetector, PiiMatch, PiiRedacter};
use super::retention_policy::RetentionPolicy;
#[cfg(test)]
use super::right_to_forget::ForgetStatus;
use super::right_to_forget::{ForgetReason, ForgetRequest};
use super::sox_transaction_id::{SoxError, SoxTransactionId};
use super::timestamp_verification::{SocError, Timestamp};
use std::string::String;
use std::vec::Vec;

use core::fmt;

/// Unified compliance framework
///
/// # Features
///
/// - **SOX Compliance**: Transaction IDs, retention policies, audit trails
/// - **GDPR Compliance**: PII redaction, forget requests
/// - **SOC2 Compliance**: Timestamp verification, change control
///
/// # Thread Safety
///
/// All operations are thread-safe (atomic transaction ID generation)
#[derive(Debug)]
pub struct ComplianceFramework {
    /// PII redacter for GDPR compliance
    pii_redacter: PiiRedacter,

    /// Default retention policy (7 years for SOX)
    default_retention: RetentionPolicy,
}

impl ComplianceFramework {
    /// Create new compliance framework with SOX defaults
    ///
    /// # Defaults
    ///
    /// - **Retention**: 7 years (SOX requirement)
    /// - **PII Redaction**: Enabled with `***REDACTED***` mask
    pub fn new() -> Self {
        Self {
            pii_redacter: PiiRedacter::new(),
            default_retention: RetentionPolicy::sox_compliant(),
        }
    }

    // ============================================================================
    // SOX (Sarbanes-Oxley) Compliance
    // ============================================================================

    /// Generate new SOX-compliant transaction ID
    ///
    /// # Performance
    ///
    /// - Target: <100ns (atomic counter)
    ///
    /// # Guarantees
    ///
    /// - **Monotonic**: IDs always increase
    /// - **Unique**: No duplicates
    /// - **Thread-safe**: Safe from multiple threads
    #[inline]
    pub fn new_transaction_id(&self) -> SoxTransactionId {
        SoxTransactionId::next()
    }

    /// Verify SOX transaction ID is valid
    ///
    /// # Checks
    ///
    /// - ID is not zero
    /// - ID is not in the future (beyond current counter)
    pub fn verify_transaction_id(&self, id: &SoxTransactionId) -> Result<(), SoxError> {
        id.verify()
    }

    /// Get default retention policy (7 years)
    pub fn default_retention_policy(&self) -> RetentionPolicy {
        self.default_retention
    }

    /// Check if data should be retained (within retention window)
    ///
    /// # Arguments
    ///
    /// - `policy`: Retention policy to check
    ///
    /// # Returns
    ///
    /// - `true`: Data must be retained (within 7-year window)
    /// - `false`: Data can be deleted (past retention period)
    pub fn should_retain(&self, policy: &RetentionPolicy) -> bool {
        policy.should_retain()
    }

    // ============================================================================
    // GDPR (General Data Protection) Compliance
    // ============================================================================

    /// Detect PII in text
    ///
    /// # Performance
    ///
    /// - Target: <1μs for typical audit trail entry (100 chars)
    ///
    /// # Returns
    ///
    /// Vector of all PII matches found
    pub fn detect_pii(&self, text: &str) -> Vec<PiiMatch> {
        self.pii_redacter.detect_pii(text)
    }

    /// Check if text contains PII
    ///
    /// # Returns
    ///
    /// - `true`: PII detected
    /// - `false`: No PII found
    pub fn contains_pii(&self, text: &str) -> bool {
        self.pii_redacter.contains_pii(text)
    }

    /// Redact all PII in text (GDPR-safe)
    ///
    /// # Performance
    ///
    /// - Target: <1μs for typical audit trail entry (100 chars)
    ///
    /// # Returns
    ///
    /// Redacted text with all PII replaced by `***REDACTED***`
    pub fn redact_pii(&self, text: &str) -> String {
        self.pii_redacter.redact(text)
    }

    /// Create GDPR forget request
    ///
    /// # Arguments
    ///
    /// - `subject_id`: Hashed user ID (use SHA-256 for privacy)
    /// - `reason`: Legal basis for erasure
    ///
    /// # Returns
    ///
    /// ForgetRequest with status tracking
    pub fn create_forget_request(
        &self,
        subject_id: impl Into<String>,
        reason: ForgetReason,
    ) -> ForgetRequest {
        ForgetRequest::new(subject_id, reason)
    }

    // ============================================================================
    // SOC2 Type II Compliance
    // ============================================================================

    /// Get current timestamp
    ///
    /// # Performance
    ///
    /// - Target: <50ns (syscall)
    pub fn current_timestamp(&self) -> Timestamp {
        Timestamp::now()
    }

    /// Verify timestamp is SOC2 compliant
    ///
    /// # Checks
    ///
    /// - Not in the future (> now + 60 seconds)
    /// - Not too old (> 7 years retention period)
    ///
    /// # Errors
    ///
    /// Returns error if timestamp fails verification
    pub fn verify_timestamp_soc2(&self, timestamp: &Timestamp) -> Result<(), SocError> {
        timestamp.verify_soc2_compliance()
    }

    // ============================================================================
    // Unified Compliance Reporting
    // ============================================================================

    /// Get compliance status summary
    ///
    /// # Returns
    ///
    /// ComplianceStatus with all framework statuses
    pub fn compliance_status(&self) -> ComplianceStatus {
        ComplianceStatus {
            sox_enabled: true,
            gdpr_enabled: true,
            soc2_enabled: true,
            retention_years: self.default_retention.retention_years(),
            pii_redaction_enabled: true,
        }
    }
}

impl Default for ComplianceFramework {
    fn default() -> Self {
        Self::new()
    }
}

/// Compliance status summary
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplianceStatus {
    /// SOX (Sarbanes-Oxley) enabled
    pub sox_enabled: bool,

    /// GDPR (General Data Protection) enabled
    pub gdpr_enabled: bool,

    /// SOC2 Type II enabled
    pub soc2_enabled: bool,

    /// Retention period in years
    pub retention_years: u32,

    /// PII redaction enabled
    pub pii_redaction_enabled: bool,
}

impl fmt::Display for ComplianceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Compliance Status:")?;
        writeln!(
            f,
            "  SOX (Sarbanes-Oxley): {}",
            if self.sox_enabled { "✓" } else { "✗" }
        )?;
        writeln!(
            f,
            "  GDPR (Data Protection): {}",
            if self.gdpr_enabled { "✓" } else { "✗" }
        )?;
        writeln!(
            f,
            "  SOC2 Type II: {}",
            if self.soc2_enabled { "✓" } else { "✗" }
        )?;
        writeln!(f, "  Retention Period: {} years", self.retention_years)?;
        writeln!(
            f,
            "  PII Redaction: {}",
            if self.pii_redaction_enabled {
                "✓"
            } else {
                "✗"
            }
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_framework_new() {
        let framework = ComplianceFramework::new();
        let status = framework.compliance_status();

        assert!(status.sox_enabled);
        assert!(status.gdpr_enabled);
        assert!(status.soc2_enabled);
        assert_eq!(status.retention_years, 7);
    }

    #[test]
    fn test_sox_transaction_id() {
        let framework = ComplianceFramework::new();

        let tx1 = framework.new_transaction_id();
        let tx2 = framework.new_transaction_id();

        assert!(tx2.value() > tx1.value());
        assert!(framework.verify_transaction_id(&tx1).is_ok());
    }

    #[test]
    fn test_sox_retention_policy() {
        let framework = ComplianceFramework::new();
        let policy = framework.default_retention_policy();

        assert_eq!(policy.retention_years(), 7);
        assert!(framework.should_retain(&policy));
    }

    #[test]
    fn test_gdpr_pii_detection() {
        let framework = ComplianceFramework::new();

        let text = "Contact john@example.com or 555-1234";
        assert!(framework.contains_pii(text));

        let matches = framework.detect_pii(text);
        assert!(!matches.is_empty());
    }

    #[test]
    fn test_gdpr_pii_redaction() {
        let framework = ComplianceFramework::new();

        let text = "Email: john@example.com";
        let redacted = framework.redact_pii(text);

        assert!(!redacted.contains("john@example.com"));
        assert!(redacted.contains("***REDACTED***"));
    }

    #[test]
    fn test_gdpr_forget_request() {
        let framework = ComplianceFramework::new();

        let request = framework.create_forget_request("user_hash_123", ForgetReason::UserRequest);

        assert_eq!(request.subject_id(), "user_hash_123");
        assert_eq!(request.status(), &ForgetStatus::Pending);
    }

    #[test]
    fn test_soc2_timestamp() {
        let framework = ComplianceFramework::new();

        let ts = framework.current_timestamp();
        assert!(framework.verify_timestamp_soc2(&ts).is_ok());
    }

    #[test]
    fn test_compliance_status_display() {
        let framework = ComplianceFramework::new();
        let status = framework.compliance_status();

        let display = format!("{}", status);
        assert!(display.contains("SOX"));
        assert!(display.contains("GDPR"));
        assert!(display.contains("SOC2"));
    }
}
