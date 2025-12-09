//! Test Mode - Mock AI Provider for Zero-Config Testing
//!
//! # Purpose
//! Provides a mock AI provider that returns friendly test responses without requiring
//! API keys or external services. Perfect for:
//! - First-time setup and exploration
//! - Integration testing
//! - Development without API costs
//!
//! # Usage
//! ```bash
//! clapi start --test
//! ```
//!
//! # UCE34 Framework
//! - Q10 (Tier): T1 Atomic (lockfree capsules for production-grade mock provider)
//! - Q31 (Simplicity): Zero dependencies, minimal configuration
//! - Q33 (Validation): Returns valid OpenAI-compatible responses
//!
//! # Architecture (v2.0)
//! - `MockProvider`: Legacy simple mock (backward compatible)
//! - `MockLLMProvider`: Production-grade with 100+ response patterns (recommended)

use crate::proxy::types::{
    ChatCompletionRequest, ChatCompletionResponse, Choice, Message, Usage,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// Production-grade mock provider (100+ patterns, T1 atomic capsules)
pub mod mock_llm_provider;
pub use mock_llm_provider::{MockLLMProvider, ProviderType};

/// Mock AI provider for test mode
///
/// Returns friendly test responses with realistic latency and token counts.
pub struct MockProvider {
    /// Simulated latency in milliseconds
    pub latency_ms: u64,
    /// Mock tokens per response
    pub token_count: u32,
    /// Cost per 1k tokens (in cents)
    pub cost_per_1k_tokens: i64,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MockProvider {
    /// Create a new mock provider with default settings
    ///
    /// # Defaults
    /// - Latency: 100ms (realistic AI response time)
    /// - Token count: 50 tokens per response
    /// - Cost: $0.30 per 1k tokens (~GPT-3.5-turbo pricing)
    pub fn new() -> Self {
        Self {
            latency_ms: 100,
            token_count: 50,
            cost_per_1k_tokens: 30, // $0.30 per 1k tokens
        }
    }

    /// Generate a chat completion response (async)
    ///
    /// # Behavior
    /// - Sleeps for `latency_ms` to simulate network/processing time
    /// - Returns a friendly test message with emojis
    /// - Includes realistic token counts and cost calculations
    ///
    /// # Arguments
    /// - `request`: OpenAI-compatible chat completion request
    ///
    /// # Returns
    /// OpenAI-compatible chat completion response with mock data
    pub async fn chat_completion(&self, request: &ChatCompletionRequest) -> ChatCompletionResponse {
        // Simulate AI processing latency
        tokio::time::sleep(Duration::from_millis(self.latency_ms)).await;

        // Generate mock response message
        let response_content = self.generate_test_message(request);

        // Calculate mock cost (in cents)
        let prompt_tokens = self.estimate_prompt_tokens(request);
        let completion_tokens = self.token_count;
        let total_tokens = prompt_tokens + completion_tokens;
        let cost_cents = (total_tokens as f64 / 1000.0 * self.cost_per_1k_tokens as f64).ceil();

        ChatCompletionResponse {
            id: format!("chatcmpl-mock-{}", Uuid::new_v4()),
            object: "chat.completion".to_string(),
            created: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs(),
            model: format!("mock-{}", request.model),
            choices: vec![Choice {
                index: 0,
                message: Message {
                    role: "assistant".to_string(),
                    content: response_content,
                    name: None,
                },
                finish_reason: "stop".to_string(),
            }],
            usage: Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
            },
            cost_cents: Some(cost_cents),
            provider: Some("mock-test-provider".to_string()),
        }
    }

    /// Generate a friendly test message with helpful guidance
    ///
    /// # Message Contents
    /// - Emoji-enhanced greeting
    /// - Explanation that this is test mode
    /// - Instructions for configuring real providers
    /// - Context-aware suggestions based on request
    fn generate_test_message(&self, request: &ChatCompletionRequest) -> String {
        let user_message = request
            .messages
            .iter()
            .filter(|m| m.role == "user")
            .next_back()
            .map(|m| m.content.as_str())
            .unwrap_or("");

        format!(
            "🧪 **Test Mode Response**\n\n\
            Hello! You're running clapi in test mode with mock AI responses.\n\n\
            **Your message:** \"{}\"\n\n\
            **What's happening:**\n\
            • This is a simulated AI response (not a real AI model)\n\
            • No API keys or external services required\n\
            • Perfect for testing clapi's budget protection and routing\n\n\
            **To use real AI providers:**\n\
            ```bash\n\
            clapi config  # Interactive configuration wizard\n\
            ```\n\n\
            **Or edit your config file:**\n\
            ```bash\n\
            # Add your API keys to clapi.toml\n\
            [[providers]]\n\
            id = \"anthropic\"\n\
            api_key = \"sk-ant-...\"\n\
            endpoint = \"https://api.anthropic.com/v1/messages\"\n\
            ```\n\n\
            📚 Documentation: https://docs.clapi.dev/setup\n\
            💬 Need help? https://kindly.feedback",
            user_message.chars().take(100).collect::<String>()
        )
    }

    /// Estimate prompt tokens from request
    ///
    /// # Algorithm
    /// - Rough estimate: 4 characters per token (English average)
    /// - Counts all message content
    ///
    /// This is a simplified version of the real token counting logic.
    fn estimate_prompt_tokens(&self, request: &ChatCompletionRequest) -> u32 {
        let total_chars: usize = request.messages.iter().map(|m| m.content.len()).sum();
        (total_chars / 4).max(1) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_provider_basic() {
        let provider = MockProvider::new();
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

        let response = provider.chat_completion(&request).await;

        // Verify response structure
        assert!(response.id.starts_with("chatcmpl-mock-"));
        assert_eq!(response.object, "chat.completion");
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].message.role, "assistant");
        assert!(response.choices[0].message.content.contains("Test Mode"));
        assert_eq!(response.usage.completion_tokens, 50);
        assert!(response.usage.total_tokens > 0);
        assert!(response.cost_cents.is_some());
    }

    #[tokio::test]
    async fn test_mock_provider_latency() {
        let provider = MockProvider {
            latency_ms: 50,
            token_count: 100,
            cost_per_1k_tokens: 30,
        };

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
        let response = provider.chat_completion(&request).await;
        let elapsed = start.elapsed();

        // Verify latency simulation (should take at least 50ms)
        assert!(elapsed >= Duration::from_millis(50));
        assert_eq!(response.usage.completion_tokens, 100);
    }

    #[tokio::test]
    async fn test_mock_provider_cost_calculation() {
        let provider = MockProvider::new();
        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "a".repeat(4000), // ~1000 tokens
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

        let response = provider.chat_completion(&request).await;

        // Verify cost calculation
        // ~1000 prompt + 50 completion = 1050 tokens
        // At $0.30 per 1k tokens = $0.315 ≈ 32 cents
        let cost = response.cost_cents.unwrap() as f64;
        assert!(cost >= 30.0 && cost <= 35.0, "Cost {} cents out of range", cost);
    }

    #[test]
    fn test_token_estimation() {
        let provider = MockProvider::new();
        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: "You are a helpful assistant.".to_string(),
                    name: None,
                },
                Message {
                    role: "user".to_string(),
                    content: "Hello!".to_string(), // ~1-2 tokens
                    name: None,
                },
            ],
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: false,
            budget_id: None,
        };

        let tokens = provider.estimate_prompt_tokens(&request);
        assert!(tokens > 0);
        // "You are a helpful assistant." (28 chars) + "Hello!" (6 chars) = 34 chars
        // 34 / 4 = 8-9 tokens (rough estimate)
        assert!(tokens >= 8 && tokens <= 10);
    }
}
