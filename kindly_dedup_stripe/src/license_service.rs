// [TRADE SECRET] License key generation and email delivery service

use crate::error::{ApiError, ApiResult};
use uuid::Uuid;

/// Generate a license key for the given tier
///
/// Format: `KINDLY-<TIER>-<UUID>`
/// Example: `KINDLY-PRO-550e8400-e29b-41d4-a716-446655440000`
///
/// **Tier codes**:
/// - `PRO`: Pro license (unlimited dedup)
/// - `STARTER`: Starter license (500 GB limit)
/// - `ENTERPRISE`: Enterprise license (custom)
pub async fn generate_license_key(tier: &str) -> ApiResult<String> {
    let tier_code = match tier.to_lowercase().as_str() {
        "pro" => "PRO",
        "starter" => "STARTER",
        "enterprise" => "ENTERPRISE",
        other => return Err(ApiError::InvalidRequest(format!("Unknown tier: {}", other))),
    };

    let uuid = Uuid::new_v4();
    Ok(format!("KINDLY-{}-{}", tier_code, uuid))
}

/// Send license key email to customer
///
/// Currently a stub - Stripe's built-in email notifications handle customer delivery.
/// Future enhancement: Integrate with SendGrid or Mailgun for custom email templates.
pub async fn send_license_email(_customer_email: &str, _license_key: &str, _tier: &str) -> ApiResult<()> {
    // License key is included in Stripe's email receipt. Custom email delivery can be added later.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_license_key_generation_pro() {
        let key = generate_license_key("pro").await.unwrap();
        assert!(key.starts_with("KINDLY-PRO-"));
        assert_eq!(key.len(), "KINDLY-PRO-00000000-0000-0000-0000-000000000000".len());
    }

    #[tokio::test]
    async fn test_license_key_generation_starter() {
        let key = generate_license_key("starter").await.unwrap();
        assert!(key.starts_with("KINDLY-STARTER-"));
    }

    #[tokio::test]
    async fn test_license_key_generation_invalid() {
        let result = generate_license_key("invalid").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_license_keys_are_unique() {
        let key1 = generate_license_key("pro").await.unwrap();
        let key2 = generate_license_key("pro").await.unwrap();
        assert_ne!(key1, key2);
    }
}
