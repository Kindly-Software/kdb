//! Right to be Forgotten - GDPR Article 17 Compliance
//!
//! # Compliance Requirements
//!
//! GDPR Article 17 (Right to Erasure) requires:
//! - **Request tracking**: Log all erasure requests immutably
//! - **Reason documentation**: Record legal basis for erasure
//! - **Status tracking**: Track processing status (pending/partial/complete)
//! - **Proof of compliance**: Provide evidence of data deletion
//!
//! # Implementation
//!
//! - **Immutable log**: Forget requests stored in audit trail (cannot be deleted)
//! - **Status tracking**: Track progress of data deletion
//! - **Subject privacy**: Hash user IDs to protect privacy in logs
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_HASH_IRREVERSIBLE`: SHA-256 hashing prevents reverse lookup of subject IDs
//! - `#VERIFY_HASH_IRREVERSIBLE`: Property tests validate hash uniqueness

use std::string::String;

use core::fmt;

use super::timestamp_verification::Timestamp;

/// GDPR Right to be Forgotten request
///
/// # Privacy
///
/// - **subject_id**: Hashed user ID (not plaintext) for privacy
/// - **reason**: Legal basis for erasure request
/// - **status**: Current processing status
///
/// # Example
///
/// ```
/// use atomic_capsule::forensics::{ForgetRequest, ForgetReason, ForgetStatus};
///
/// let request = ForgetRequest::new(
///     "user_12345_hashed",
///     ForgetReason::UserRequest,
/// );
///
/// assert_eq!(request.status(), &ForgetStatus::Pending);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgetRequest {
    /// Hashed subject ID (for privacy)
    subject_id: String,

    /// Timestamp when request was made
    requested_at: Timestamp,

    /// Legal reason for erasure
    reason: ForgetReason,

    /// Current processing status
    status: ForgetStatus,
}

impl ForgetRequest {
    /// Create new forget request
    ///
    /// # Arguments
    ///
    /// - `subject_id`: Hashed user ID (use SHA-256 or similar)
    /// - `reason`: Legal basis for erasure
    ///
    /// # Performance
    ///
    /// - Target: <100ns (struct creation)
    pub fn new(subject_id: impl Into<String>, reason: ForgetReason) -> Self {
        Self {
            subject_id: subject_id.into(),
            requested_at: Timestamp::now(),
            reason,
            status: ForgetStatus::Pending,
        }
    }

    /// Get subject ID (hashed)
    pub fn subject_id(&self) -> &str {
        &self.subject_id
    }

    /// Get request timestamp
    pub fn requested_at(&self) -> Timestamp {
        self.requested_at
    }

    /// Get erasure reason
    pub fn reason(&self) -> &ForgetReason {
        &self.reason
    }

    /// Get current status
    pub fn status(&self) -> &ForgetStatus {
        &self.status
    }

    /// Update status (for processing workflow)
    pub fn set_status(&mut self, status: ForgetStatus) {
        self.status = status;
    }

    /// Mark as acknowledged
    pub fn acknowledge(&mut self) {
        self.status = ForgetStatus::Acknowledged;
    }

    /// Mark as partially processed
    pub fn mark_partial(&mut self, count: usize) {
        self.status = ForgetStatus::ProcessedPartially { count };
    }

    /// Mark as fully processed
    pub fn mark_complete(&mut self, count: usize) {
        self.status = ForgetStatus::ProcessedFully { count };
    }
}

/// Legal reason for erasure (GDPR Article 17)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForgetReason {
    /// User explicitly requested erasure (Article 17.1.a)
    UserRequest,

    /// User withdrew consent (Article 17.1.b)
    ConsentWithdrawn,

    /// Data no longer necessary (Article 17.1.a)
    NoLongerNecessary,

    /// Data processed illegally (Article 17.1.d)
    IllegalProcessing,

    /// Legal obligation to erase (Article 17.1.e)
    LegalObligation,
}

impl fmt::Display for ForgetReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForgetReason::UserRequest => write!(f, "User request (Article 17.1.a)"),
            ForgetReason::ConsentWithdrawn => write!(f, "Consent withdrawn (Article 17.1.b)"),
            ForgetReason::NoLongerNecessary => write!(f, "No longer necessary (Article 17.1.a)"),
            ForgetReason::IllegalProcessing => write!(f, "Illegal processing (Article 17.1.d)"),
            ForgetReason::LegalObligation => write!(f, "Legal obligation (Article 17.1.e)"),
        }
    }
}

/// Processing status of forget request
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgetStatus {
    /// Request received, not yet processed
    Pending,

    /// Request acknowledged, processing started
    Acknowledged,

    /// Partially processed
    ProcessedPartially {
        /// Number of records deleted so far
        count: usize,
    },

    /// Fully processed
    ProcessedFully {
        /// Total number of records deleted
        count: usize,
    },
}

impl fmt::Display for ForgetStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForgetStatus::Pending => write!(f, "Pending"),
            ForgetStatus::Acknowledged => write!(f, "Acknowledged"),
            ForgetStatus::ProcessedPartially { count } => {
                write!(f, "Partially processed ({} records)", count)
            }
            ForgetStatus::ProcessedFully { count } => {
                write!(f, "Fully processed ({} records)", count)
            }
        }
    }
}

/// GDPR-specific errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GdprError {
    /// PII detection failed
    PiiDetectionFailed(&'static str),

    /// Redaction failed
    RedactionFailed(&'static str),

    /// Forget request processing failed
    ForgetRequestFailed(&'static str),
}

impl fmt::Display for GdprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GdprError::PiiDetectionFailed(msg) => write!(f, "PII detection failed: {}", msg),
            GdprError::RedactionFailed(msg) => write!(f, "Redaction failed: {}", msg),
            GdprError::ForgetRequestFailed(msg) => write!(f, "Forget request failed: {}", msg),
        }
    }
}

impl core::error::Error for GdprError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forget_request_new() {
        let request = ForgetRequest::new("user_hash_12345", ForgetReason::UserRequest);

        assert_eq!(request.subject_id(), "user_hash_12345");
        assert_eq!(request.reason(), &ForgetReason::UserRequest);
        assert_eq!(request.status(), &ForgetStatus::Pending);
    }

    #[test]
    fn test_forget_request_acknowledge() {
        let mut request = ForgetRequest::new("user_hash", ForgetReason::ConsentWithdrawn);
        request.acknowledge();

        assert_eq!(request.status(), &ForgetStatus::Acknowledged);
    }

    #[test]
    fn test_forget_request_partial() {
        let mut request = ForgetRequest::new("user_hash", ForgetReason::NoLongerNecessary);
        request.mark_partial(42);

        assert_eq!(
            request.status(),
            &ForgetStatus::ProcessedPartially { count: 42 }
        );
    }

    #[test]
    fn test_forget_request_complete() {
        let mut request = ForgetRequest::new("user_hash", ForgetReason::LegalObligation);
        request.mark_complete(100);

        assert_eq!(
            request.status(),
            &ForgetStatus::ProcessedFully { count: 100 }
        );
    }

    #[test]
    fn test_forget_reason_display() {
        assert_eq!(
            format!("{}", ForgetReason::UserRequest),
            "User request (Article 17.1.a)"
        );
        assert_eq!(
            format!("{}", ForgetReason::IllegalProcessing),
            "Illegal processing (Article 17.1.d)"
        );
    }

    #[test]
    fn test_forget_status_display() {
        assert_eq!(format!("{}", ForgetStatus::Pending), "Pending");
        assert_eq!(
            format!("{}", ForgetStatus::ProcessedPartially { count: 42 }),
            "Partially processed (42 records)"
        );
        assert_eq!(
            format!("{}", ForgetStatus::ProcessedFully { count: 100 }),
            "Fully processed (100 records)"
        );
    }
}
