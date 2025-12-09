//! Resend Email Client
//!
//! Wraps the resend-rs crate for sending verification and license emails.
//!
//! # Features
//!
//! - Async email sending via Resend API
//! - Two email templates: verification and license delivery
//! - Environment variable configuration (RESEND_API_KEY, FROM_EMAIL)
//! - Error handling with proper types
//!
//! # Framework Compliance
//!
//! - Async-first design (no blocking)
//! - Error handling via thiserror
//! - Environment-based configuration

use resend_rs::{types::CreateEmailBaseOptions, Resend};
use thiserror::Error;

/// Email-specific errors for the Resend client
#[derive(Debug, Error)]
pub enum EmailError {
    /// Missing RESEND_API_KEY environment variable
    #[error("Missing API key (RESEND_API_KEY)")]
    MissingApiKey,

    /// Missing FROM_EMAIL environment variable
    #[error("Missing FROM_EMAIL")]
    MissingFromEmail,

    /// Resend API error
    #[error("Resend API error: {0}")]
    ResendError(String),

    /// Invalid email address
    #[error("Invalid email address: {0}")]
    InvalidAddress(String),
}

/// Resend email client for sending verification and license emails
///
/// # Example
///
/// ```no_run
/// use kdb_signup::email::resend_client::ResendClient;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = ResendClient::new()?;
///
/// // Send verification email
/// client.send_verification_email(
///     "user@example.com",
///     "abc123token",
///     "Acme Corp"
/// ).await?;
///
/// // Send license email
/// client.send_license_email(
///     "user@example.com",
///     "KDB-HOBBY-XXXXX",
///     "Hobby",
///     10,
///     false
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub struct ResendClient {
    /// Resend API key
    api_key: String,
    /// From email address
    from_email: String,
    /// Base URL for verification links
    verification_base_url: String,
}

impl ResendClient {
    /// Create a new ResendClient from environment variables
    ///
    /// Required environment variables:
    /// - `RESEND_API_KEY`: Resend API key
    /// - `FROM_EMAIL`: Sender email address (optional, defaults to "noreply@kindly.software")
    /// - `VERIFICATION_BASE_URL`: Base URL for verification (optional, defaults to "https://api.kindly.software/v1/verify")
    ///
    /// # Errors
    ///
    /// Returns `EmailError::MissingApiKey` if RESEND_API_KEY is not set.
    pub fn new() -> Result<Self, EmailError> {
        let api_key = std::env::var("RESEND_API_KEY").map_err(|_| EmailError::MissingApiKey)?;

        let from_email = std::env::var("FROM_EMAIL")
            .unwrap_or_else(|_| "noreply@kindly.software".to_string());

        let verification_base_url = std::env::var("VERIFICATION_BASE_URL")
            .unwrap_or_else(|_| "https://api.kindly.software/api/v1/verify".to_string());

        Ok(Self {
            api_key,
            from_email,
            verification_base_url,
        })
    }

    /// Create a new ResendClient with explicit configuration
    ///
    /// Use this for testing or when environment variables are not available.
    pub fn new_with_config(api_key: &str, from_email: &str, verification_url: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            from_email: from_email.to_string(),
            verification_base_url: verification_url.to_string(),
        }
    }

    /// Send a verification email to confirm the user's email address
    ///
    /// # Arguments
    ///
    /// * `to` - Recipient email address
    /// * `token` - Verification token (will be appended to verification_base_url)
    /// * `org_name` - Organization or user name for personalization
    ///
    /// # Errors
    ///
    /// Returns `EmailError::ResendError` if the Resend API call fails.
    pub async fn send_verification_email(
        &self,
        to: &str,
        token: &str,
        org_name: &str,
    ) -> Result<(), EmailError> {
        let resend = Resend::new(&self.api_key);

        let verification_link = format!("{}/{}", self.verification_base_url, token);

        let html_content = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; }}
        .container {{ max-width: 600px; margin: 0 auto; padding: 20px; }}
        .header {{ color: #6B21A8; font-size: 24px; font-weight: bold; margin-bottom: 20px; }}
        .button {{ display: inline-block; background-color: #6B21A8; color: #FFD700; padding: 12px 24px; text-decoration: none; border-radius: 6px; margin: 20px 0; font-weight: bold; }}
        .footer {{ margin-top: 30px; color: #666; font-size: 14px; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">Welcome to KDB - The Kindly Debugger!</div>

        <p>Hi {org_name},</p>

        <p>Thank you for signing up for KDB! Please verify your email address to complete your registration.</p>

        <a href="{verification_link}" class="button" style="display: inline-block; background-color: #6B21A8; color: #FFD700 !important; padding: 12px 24px; text-decoration: none; border-radius: 6px; margin: 20px 0; font-weight: bold;">Verify Email Address</a>

        <p>Or copy and paste this link into your browser:</p>
        <p><a href="{verification_link}">{verification_link}</a></p>

        <p><strong>This link expires in 24 hours.</strong></p>

        <p>If you didn't sign up for KDB, you can safely ignore this email.</p>

        <div class="footer">
            <p>- The Kindly Team</p>
        </div>
    </div>
</body>
</html>"#,
            org_name = html_escape(org_name),
            verification_link = verification_link,
        );

        let text_content = format!(
            r#"Welcome to KDB - The Kindly Debugger!

Hi {org_name},

Click to verify your email: {verification_link}

This link expires in 24 hours.

If you didn't sign up for KDB, ignore this email.

- The Kindly Team"#,
            org_name = org_name,
            verification_link = verification_link,
        );

        let email = CreateEmailBaseOptions::new(
            &self.from_email,
            [to.to_string()],
            "Verify your KDB account",
        )
        .with_html(&html_content)
        .with_text(&text_content);

        resend
            .emails
            .send(email)
            .await
            .map_err(|e| EmailError::ResendError(e.to_string()))?;

        tracing::info!(to = %to, "Verification email sent successfully");

        Ok(())
    }

    /// Send a license delivery email with the user's license key
    ///
    /// # Arguments
    ///
    /// * `to` - Recipient email address
    /// * `license_key` - The generated license key
    /// * `tier_name` - Name of the tier (e.g., "Hobby", "Pro")
    /// * `sessions_per_month` - Number of sessions allowed per month
    /// * `is_promo` - Whether this is a promotional unlimited period
    ///
    /// # Errors
    ///
    /// Returns `EmailError::ResendError` if the Resend API call fails.
    pub async fn send_license_email(
        &self,
        to: &str,
        license_key: &str,
        tier_name: &str,
        sessions_per_month: u64,
        is_promo: bool,
    ) -> Result<(), EmailError> {
        let resend = Resend::new(&self.api_key);

        let promo_text = if is_promo {
            " (PROMO: Unlimited this week!)"
        } else {
            ""
        };

        let promo_html = if is_promo {
            r#" <span style="color: #16A34A; font-weight: bold;">(PROMO: Unlimited this week!)</span>"#
        } else {
            ""
        };

        let html_content = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; }}
        .container {{ max-width: 600px; margin: 0 auto; padding: 20px; }}
        .header {{ color: #6B21A8; font-size: 24px; font-weight: bold; margin-bottom: 20px; }}
        .license-box {{ background-color: #F3F4F6; border: 2px solid #6B21A8; border-radius: 8px; padding: 20px; margin: 20px 0; text-align: center; }}
        .license-key {{ font-family: 'Courier New', monospace; font-size: 18px; font-weight: bold; color: #6B21A8; letter-spacing: 1px; }}
        .info-table {{ width: 100%; border-collapse: collapse; margin: 20px 0; }}
        .info-table td {{ padding: 8px 0; border-bottom: 1px solid #E5E7EB; }}
        .info-table td:first-child {{ font-weight: bold; width: 40%; }}
        .steps {{ background-color: #F9FAFB; border-radius: 8px; padding: 20px; margin: 20px 0; }}
        .steps h3 {{ margin-top: 0; color: #6B21A8; }}
        .steps ol {{ margin: 0; padding-left: 20px; }}
        .steps li {{ margin: 10px 0; }}
        .footer {{ margin-top: 30px; color: #666; font-size: 14px; }}
        a {{ color: #6B21A8; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">Welcome to KDB!</div>

        <p>Your email has been verified and your license is ready!</p>

        <div class="license-box">
            <p style="margin: 0 0 10px 0; color: #666;">Your License Key</p>
            <div class="license-key">{license_key}</div>
        </div>

        <table class="info-table">
            <tr>
                <td>Tier:</td>
                <td>{tier_name}</td>
            </tr>
            <tr>
                <td>Sessions:</td>
                <td>{sessions_per_month}/month{promo_html}</td>
            </tr>
        </table>

        <div class="steps">
            <h3>Getting Started</h3>
            <ol>
                <li>Install the Claude Code extension</li>
                <li>Add your license key in settings</li>
                <li>Start debugging with AI assistance</li>
            </ol>
        </div>

        <p>
            <strong>Documentation:</strong> <a href="https://kindly.software/#docs">https://kindly.software/#docs</a><br>
            <strong>Support:</strong> <a href="mailto:support@kindly.software">support@kindly.software</a>
        </p>

        <div class="footer">
            <p>Happy debugging!</p>
            <p>- The Kindly Team</p>
        </div>
    </div>
</body>
</html>"#,
            license_key = html_escape(license_key),
            tier_name = html_escape(tier_name),
            sessions_per_month = sessions_per_month,
            promo_html = promo_html,
        );

        let text_content = format!(
            r#"Welcome to KDB!

Your license key: {license_key}

Tier: {tier_name}
Sessions: {sessions_per_month}/month{promo_text}

Getting Started:
1. Install the Claude Code extension
2. Add your license key in settings
3. Start debugging with AI assistance

Documentation: https://kindly.software/#docs
Support: support@kindly.software

Happy debugging!
- The Kindly Team"#,
            license_key = license_key,
            tier_name = tier_name,
            sessions_per_month = sessions_per_month,
            promo_text = promo_text,
        );

        let email = CreateEmailBaseOptions::new(
            &self.from_email,
            [to.to_string()],
            "Your KDB Hobby License Key",
        )
        .with_html(&html_content)
        .with_text(&text_content);

        resend
            .emails
            .send(email)
            .await
            .map_err(|e| EmailError::ResendError(e.to_string()))?;

        tracing::info!(to = %to, tier = %tier_name, "License email sent successfully");

        Ok(())
    }
}

/// Simple HTML escape for user-provided content
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_config() {
        let client = ResendClient::new_with_config(
            "test_api_key",
            "test@example.com",
            "https://test.example.com/verify",
        );

        assert_eq!(client.api_key, "test_api_key");
        assert_eq!(client.from_email, "test@example.com");
        assert_eq!(
            client.verification_base_url,
            "https://test.example.com/verify"
        );
    }

    #[test]
    fn test_new_missing_api_key() {
        // Ensure RESEND_API_KEY is not set for this test
        std::env::remove_var("RESEND_API_KEY");

        let result = ResendClient::new();
        assert!(result.is_err());

        match result {
            Err(EmailError::MissingApiKey) => {}
            _ => panic!("Expected MissingApiKey error"),
        }
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("A & B"), "A &amp; B");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(html_escape("it's"), "it&#x27;s");
    }

    #[test]
    fn test_email_error_display() {
        assert_eq!(
            EmailError::MissingApiKey.to_string(),
            "Missing API key (RESEND_API_KEY)"
        );
        assert_eq!(
            EmailError::MissingFromEmail.to_string(),
            "Missing FROM_EMAIL"
        );
        assert_eq!(
            EmailError::ResendError("test error".to_string()).to_string(),
            "Resend API error: test error"
        );
        assert_eq!(
            EmailError::InvalidAddress("bad@".to_string()).to_string(),
            "Invalid email address: bad@"
        );
    }
}
