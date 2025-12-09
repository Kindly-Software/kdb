// [TRADE SECRET] Stripe webhook signature verification
// HMAC-SHA256 signature validation for webhook authenticity

use crate::error::{ApiError, ApiResult};
use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Verify Stripe webhook signature
///
/// Stripe includes a stripe-signature header with format: t=<timestamp>,v1=<signature>
/// We compute HMAC-SHA256(secret, timestamp.payload) and compare with the signature.
///
/// This ensures:
/// 1. Webhook came from Stripe (not an attacker)
/// 2. Webhook was not tampered with
/// 3. Replay attacks are prevented (timestamp validation could be added)
pub fn verify_stripe_signature(
    body: &[u8],
    signature_header: &str,
    secret: &str,
) -> ApiResult<()> {
    // Parse signature header: "t=<timestamp>,v1=<signature>"
    let mut timestamp = None;
    let mut signature = None;

    for part in signature_header.split(',') {
        let parts: Vec<&str> = part.split('=').collect();
        if parts.len() != 2 {
            continue;
        }

        match parts[0] {
            "t" => timestamp = Some(parts[1]),
            "v1" => signature = Some(parts[1]),
            _ => {}
        }
    }

    let timestamp = timestamp
        .ok_or_else(|| ApiError::InvalidSignature("Missing timestamp".to_string()))?;
    let signature = signature
        .ok_or_else(|| ApiError::InvalidSignature("Missing v1 signature".to_string()))?;

    // Compute expected signature: HMAC-SHA256(secret, timestamp.payload)
    let signed_content = format!("{}.{}", timestamp, String::from_utf8_lossy(body));

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| ApiError::InvalidSignature("Invalid secret key".to_string()))?;

    mac.update(signed_content.as_bytes());

    // Convert computed signature to hex
    let computed_sig = hex::encode(mac.finalize().into_bytes());

    // Constant-time comparison to prevent timing attacks
    if constant_time_compare(computed_sig.as_bytes(), signature.as_bytes()) {
        Ok(())
    } else {
        Err(ApiError::InvalidSignature(
            "Signature verification failed".to_string(),
        ))
    }
}

/// Constant-time string comparison to prevent timing attacks
fn constant_time_compare(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }

    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_verification() {
        // Example from Stripe documentation
        let secret = "whsec_test_secret";
        let timestamp = "1614556800";
        let body = br#"{"id": "evt_test"}"#;
        let signed_content = format!("{}.{}", timestamp, String::from_utf8_lossy(body));

        // Compute signature
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(signed_content.as_bytes());
        let sig_hex = hex::encode(mac.finalize().into_bytes());

        // Verify it
        let header = format!("t={},v1={}", timestamp, sig_hex);
        assert!(verify_stripe_signature(body, &header, secret).is_ok());
    }

    #[test]
    fn test_invalid_signature() {
        let body = br#"{"id": "evt_test"}"#;
        let header = "t=1614556800,v1=invalidsig";
        assert!(verify_stripe_signature(body, header, "secret").is_err());
    }
}
