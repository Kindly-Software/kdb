//! Provider HTTP client with connection pooling
//!
//! # UCE33 Q14: Concurrency
//! - Reqwest client (connection pooling built-in)
//! - Async/await throughout
//! - Timeout handling

use std::time::Duration;

use crate::error::ClapiResult;
use crate::proxy::{ChatCompletionRequest, ChatCompletionResponse, ProviderConfig};

/// Provider client with connection pooling
///
/// # Safety
/// - #ASSUME: Reqwest client internally uses Arc for connection pool
/// - #VERIFY: No manual synchronization needed (reqwest handles it)
pub struct ProviderClient {
    /// HTTP client (connection pooling built-in)
    client: reqwest::Client,

    /// Base URL for API
    base_url: String,

    /// API key for authentication
    api_key: String,

    /// Provider ID (index in config)
    provider_id: u16,

    /// Provider name
    name: String,
}

impl ProviderClient {
    /// Create new provider client
    ///
    /// # Arguments
    /// - `config`: Provider configuration
    /// - `provider_id`: Provider index
    /// - `timeout`: Request timeout
    pub fn new(config: &ProviderConfig, provider_id: u16, timeout: Duration) -> ClapiResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .pool_max_idle_per_host(10) // Connection pooling
            .pool_idle_timeout(Duration::from_secs(90))
            .build()?;

        Ok(Self {
            client,
            base_url: config.base_url.clone(),
            api_key: config.api_key.clone(),
            provider_id,
            name: config.name.clone(),
        })
    }

    /// Send chat completion request
    ///
    /// # Performance
    /// - Connection pooling: Reuse TCP connections
    /// - Timeout: Configurable per-request timeout
    /// - Async: Non-blocking I/O
    pub async fn chat_completion(
        &self,
        req: &ChatCompletionRequest,
    ) -> ClapiResult<ChatCompletionResponse> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(req)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::ClapiError::ProviderError(format!(
                "Provider {} returned {}: {}",
                self.name, status, body
            )));
        }

        let mut completion: ChatCompletionResponse = response.json().await?;

        // Add Clapi metadata
        completion.provider = Some(self.name.clone());

        Ok(completion)
    }

    /// Get provider ID
    #[inline]
    pub fn provider_id(&self) -> u16 {
        self.provider_id
    }

    /// Get provider name
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let config = ProviderConfig {
            name: "test".to_string(),
            base_url: "https://api.test.com".to_string(),
            api_key: "test_key".to_string(),
            priority: 0,
            models: vec![],
        };

        let client = ProviderClient::new(&config, 0, Duration::from_secs(30));
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.provider_id(), 0);
        assert_eq!(client.name(), "test");
    }
}
