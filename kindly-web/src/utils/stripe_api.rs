// [TRADE SECRET] Stripe API client for frontend
// Calls to webhook handler endpoints

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

/// Early adopter count response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarlyAdopterCount {
    pub sold: u64,
    pub limit: u64,
    pub remaining: u64,
    pub sold_out: bool,
}

/// Create checkout session request/response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCheckoutSessionRequest {
    pub price_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCheckoutSessionResponse {
    pub session_id: String,
    pub success: bool,
    pub message: String,
}

/// Get early adopter remaining count
pub async fn get_early_adopter_remaining() -> Result<EarlyAdopterCount, String> {
    let base_url = get_api_base_url();
    let url = format!("{}/api/early-adopter-remaining", base_url);

    Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?
        .json::<EarlyAdopterCount>()
        .await
        .map_err(|e| format!("Parse error: {}", e))
}

/// Create Stripe checkout session
pub async fn create_checkout_session(price_id: &str) -> Result<String, String> {
    let base_url = get_api_base_url();
    let url = format!("{}/api/create-checkout-session", base_url);

    let request = CreateCheckoutSessionRequest {
        price_id: price_id.to_string(),
    };

    let response = Request::post(&url)
        .json(&request)
        .map_err(|e| format!("Serialization error: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if !response.ok() {
        return Err(format!("Server error: {}", response.status()));
    }

    let data = response
        .json::<CreateCheckoutSessionResponse>()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    if data.success {
        Ok(data.session_id)
    } else {
        Err(data.message)
    }
}

/// Get API base URL (from environment or current host)
fn get_api_base_url() -> String {
    // In production, this would use the actual webhook handler URL
    // For development, it could be localhost:3000
    std::env::var("STRIPE_API_BASE_URL")
        .unwrap_or_else(|_| {
            // Default to current host + /api
            if cfg!(debug_assertions) {
                "http://localhost:3000".to_string()
            } else {
                // Production: webhook handler on Fly.io
                "https://kindly-dedup-webhook.fly.dev".to_string()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_base_url() {
        let url = get_api_base_url();
        assert!(!url.is_empty());
    }
}
