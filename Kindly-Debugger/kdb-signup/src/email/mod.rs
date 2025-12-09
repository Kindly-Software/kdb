//! Email Validation and Sending
//!
//! Uses mailchecker for disposable email detection and Resend API for sending.
//!
//! # Features
//!
//! - Disposable email detection (mailchecker + custom FNV-1a hash set)
//! - Email validation (format, MX records)
//! - Verification email sending (Resend API)
//! - License delivery email sending (Resend API)
//!
//! # Modules
//!
//! - [`disposable`]: DisposableEmailBlocker with FNV-1a hash lookup (<50ns)
//! - [`resend_client`]: Resend API wrapper for sending verification and license emails
//!
//! # Framework Compliance
//!
//! - Async-first design (no blocking)
//! - Error handling via thiserror
//! - T1 Atomic tier: Lockfree hash lookup

pub mod disposable;
pub mod resend_client;

pub use resend_client::{EmailError as ResendEmailError, ResendClient};

use thiserror::Error;

/// Email validation and sending errors
#[derive(Error, Debug)]
pub enum EmailError {
    /// Invalid email format
    #[error("Invalid email format: {0}")]
    InvalidFormat(String),

    /// Disposable email detected
    #[error("Disposable email not allowed: {0}")]
    DisposableEmail(String),

    /// Failed to send email
    #[error("Failed to send email: {0}")]
    SendFailed(String),

    /// Resend API error
    #[error("Resend API error: {0}")]
    ResendError(String),
}

/// Validate email address
///
/// Checks format and disposable email status using mailchecker.
pub fn validate_email(email: &str) -> Result<(), EmailError> {
    // Basic format validation
    if !email.contains('@') || !email.contains('.') {
        return Err(EmailError::InvalidFormat(email.to_string()));
    }

    // Check for disposable email using mailchecker
    if !mailchecker::is_valid(email) {
        return Err(EmailError::DisposableEmail(email.to_string()));
    }

    Ok(())
}

// NOTE: For email sending, use `ResendClient` from the `resend_client` module.
// ResendClient provides:
// - `ResendClient::new()` - creates from env vars (RESEND_API_KEY, FROM_EMAIL, VERIFICATION_BASE_URL)
// - `send_verification_email(to, token, org_name)` - sends verification email with link
// - `send_license_email(to, license_key, tier_name, sessions_per_month, is_promo)` - sends license key

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_email() {
        assert!(validate_email("user@example.com").is_ok());
    }

    #[test]
    fn test_invalid_format() {
        assert!(validate_email("not-an-email").is_err());
    }

    #[test]
    fn test_disposable_email() {
        // mailchecker should catch common disposable domains
        assert!(validate_email("test@mailinator.com").is_err());
    }
}
