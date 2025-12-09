//! Mock Router - Route HTTP requests to MockProvider in test mode
//!
//! # Purpose
//! Provides transparent integration between HTTP server and MockProvider.
//! Converts HTTP requests to MockProvider calls without code duplication.
//!
//! # UCE34 Framework
//! - Q10 (Tier): Not applicable (no capsules, just routing logic)
//! - Q31 (Simplicity): Single-purpose router, minimal API surface
//! - Q33 (Validation): Compile-time type safety via Result types
//!
//! # I20 Integration
//! - Q6-Q10: Compatible with existing HTTP server (transparent drop-in)
//! - Q11-Q15: Safe routing (no races, deterministic)
//! - Q16-Q20: Tested with HTTP integration tests

use std::sync::Arc;

use crate::error::ClapiResult;
use crate::proxy::{ChatCompletionRequest, ChatCompletionResponse};
use crate::test_mode::MockProvider;

/// Mock router for test mode
///
/// Routes all HTTP requests through MockProvider instead of real providers.
///
/// # Safety
/// - #ASSUME: No atomic operations (pure async logic)
/// - #VERIFY: No races (single MockProvider instance)
pub struct MockRouter {
    /// Mock provider instance (shared via Arc for async)
    mock_provider: Arc<MockProvider>,
}

impl MockRouter {
    /// Create a new mock router
    ///
    /// # Performance
    /// - Zero overhead: Just wraps MockProvider
    /// - Async-safe: Arc allows sharing across tasks
    pub fn new() -> Self {
        Self {
            mock_provider: Arc::new(MockProvider::new()),
        }
    }

    /// Create a mock router with custom provider settings
    ///
    /// # Arguments
    /// - `latency_ms`: Simulated latency in milliseconds
    /// - `token_count`: Mock tokens per response
    /// - `cost_per_1k_tokens`: Cost per 1k tokens (in cents)
    ///
    /// # Use Cases
    /// - Testing with different latency profiles
    /// - Testing with different cost models
    /// - Integration testing with controlled timing
    pub fn with_settings(latency_ms: u64, token_count: u32, cost_per_1k_tokens: i64) -> Self {
        Self {
            mock_provider: Arc::new(MockProvider {
                latency_ms,
                token_count,
                cost_per_1k_tokens,
            }),
        }
    }

    /// Route request to MockProvider
    ///
    /// # Behavior
    /// - Converts HTTP request to MockProvider call
    /// - Returns OpenAI-compatible response
    /// - Simulates realistic latency (~100ms default)
    /// - Includes realistic token counts
    /// - Calculates costs correctly
    ///
    /// # Performance
    /// - Latency: Configurable (default 100ms)
    /// - Throughput: Limited only by async runtime
    /// - Memory: Zero allocations per request (pre-allocated Arc)
    ///
    /// # Arguments
    /// - `request`: OpenAI-compatible chat completion request
    ///
    /// # Returns
    /// OpenAI-compatible chat completion response with mock data
    ///
    /// # Errors
    /// Never errors (MockProvider always succeeds)
    pub async fn route_request(
        &self,
        request: &ChatCompletionRequest,
    ) -> ClapiResult<ChatCompletionResponse> {
        // Route to MockProvider (never fails)
        let response = self.mock_provider.chat_completion(request).await;

        Ok(response)
    }

    /// Get provider stats for health endpoint
    ///
    /// # Returns
    /// Mock provider statistics for /health endpoint
    pub fn get_stats(&self) -> MockProviderStats {
        MockProviderStats {
            latency_ms: self.mock_provider.latency_ms,
            token_count: self.mock_provider.token_count,
            cost_per_1k_tokens: self.mock_provider.cost_per_1k_tokens,
        }
    }
}

impl Default for MockRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock provider statistics
#[derive(Debug, Clone)]
pub struct MockProviderStats {
    /// Simulated latency (ms)
    pub latency_ms: u64,
    /// Mock token count per response
    pub token_count: u32,
    /// Cost per 1k tokens (cents)
    pub cost_per_1k_tokens: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::Message;

    #[tokio::test]
    async fn test_mock_router_basic() {
        let router = MockRouter::new();
        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "Hello, world!".to_string(),
                name: None,
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: false,
            budget_id: None,
        };

        let response = router.route_request(&request).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert!(response.id.starts_with("chatcmpl-mock-"));
        assert_eq!(response.choices.len(), 1);
        assert!(response.choices[0].message.content.contains("Test Mode"));
        assert!(response.cost_cents.is_some());
        assert_eq!(response.provider, Some("mock-test-provider".to_string()));
    }

    #[tokio::test]
    async fn test_mock_router_custom_settings() {
        let router = MockRouter::with_settings(50, 100, 30);
        let request = ChatCompletionRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "Quick test".to_string(),
                name: None,
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: false,
            budget_id: None,
        };

        let start = std::time::Instant::now();
        let response = router.route_request(&request).await.unwrap();
        let elapsed = start.elapsed();

        // Verify latency simulation
        assert!(elapsed.as_millis() >= 50);
        assert_eq!(response.usage.completion_tokens, 100);
    }

    #[tokio::test]
    async fn test_mock_router_stats() {
        let router = MockRouter::with_settings(200, 150, 40);
        let stats = router.get_stats();

        assert_eq!(stats.latency_ms, 200);
        assert_eq!(stats.token_count, 150);
        assert_eq!(stats.cost_per_1k_tokens, 40);
    }

    #[tokio::test]
    async fn test_mock_router_never_fails() {
        let router = MockRouter::new();

        // Even with empty messages, should succeed
        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: false,
            budget_id: None,
        };

        let response = router.route_request(&request).await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_mock_router_concurrent() {
        let router = Arc::new(MockRouter::new());

        // Spawn 10 concurrent requests
        let mut handles = vec![];
        for i in 0..10 {
            let router_clone = Arc::clone(&router);
            let handle = tokio::spawn(async move {
                let request = ChatCompletionRequest {
                    model: "gpt-4".to_string(),
                    messages: vec![Message {
                        role: "user".to_string(),
                        content: format!("Request {}", i),
                        name: None,
                    }],
                    temperature: None,
                    max_tokens: None,
                    top_p: None,
                    frequency_penalty: None,
                    presence_penalty: None,
                    stop: None,
                    stream: false,
                    budget_id: None,
                };

                router_clone.route_request(&request).await
            });
            handles.push(handle);
        }

        // All should succeed
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }
}
