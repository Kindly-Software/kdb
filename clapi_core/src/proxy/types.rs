//! OpenAI-compatible request/response types
//!
//! # UCE33 Q14: Data Structures
//! - OpenAI Chat Completion API compatibility
//! - Serde JSON serialization
//! - Optional Clapi-specific extensions

use serde::{Deserialize, Serialize};

/// Budget ID (numeric u64 identifier for lockfree budget tracking)
///
/// # Migration from String IDs
/// Clients must maintain a mapping from user identifiers to numeric BudgetIds:
/// - Store mapping in database/Redis/memory
/// - Assign sequential u64 IDs starting from 1
/// - Send numeric ID in API requests
///
/// # Example
/// ```text
/// User mapping:
/// "user_alice" → 1
/// "user_bob"   → 2
/// "org_acme"   → 3
/// ```
pub type BudgetId = u64;

/// Default budget ID for requests without explicit budget
pub const DEFAULT_BUDGET_ID: BudgetId = 0;

/// Chat completion request (OpenAI-compatible)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatCompletionRequest {
    /// Model to use (e.g., "gpt-4", "claude-3-opus")
    pub model: String,

    /// List of messages in conversation
    pub messages: Vec<Message>,

    /// Sampling temperature (0.0-2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Top-p sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Frequency penalty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Presence penalty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// Streaming mode
    #[serde(default)]
    pub stream: bool,

    /// Clapi-specific: Budget ID for tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_id: Option<BudgetId>,
}

/// Message in conversation
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    /// Role: "system", "user", or "assistant"
    pub role: String,

    /// Message content
    pub content: String,

    /// Optional name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Chat completion response (OpenAI-compatible)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatCompletionResponse {
    /// Response ID
    pub id: String,

    /// Object type ("chat.completion")
    pub object: String,

    /// Unix timestamp (seconds)
    pub created: u64,

    /// Model used
    pub model: String,

    /// List of completion choices
    pub choices: Vec<Choice>,

    /// Token usage statistics
    pub usage: Usage,

    /// Clapi-specific: Cost in cents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_cents: Option<f64>,

    /// Clapi-specific: Provider ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

/// Completion choice
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Choice {
    /// Choice index
    pub index: u32,

    /// Generated message
    pub message: Message,

    /// Finish reason ("stop", "length", "content_filter")
    pub finish_reason: String,
}

/// Token usage statistics
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Usage {
    /// Tokens in prompt
    pub prompt_tokens: u32,

    /// Tokens in completion
    pub completion_tokens: u32,

    /// Total tokens (prompt + completion)
    pub total_tokens: u32,
}

impl ChatCompletionRequest {
    /// Estimate cost in cents (simplified model)
    ///
    /// # Assumptions
    /// - GPT-4: $0.03/1K prompt tokens, $0.06/1K completion tokens
    /// - GPT-3.5-turbo: $0.0015/1K prompt tokens, $0.002/1K completion tokens
    /// - Claude: $0.015/1K prompt tokens, $0.075/1K completion tokens
    ///
    /// This is a rough estimate for budget checking purposes.
    pub fn estimate_cost_cents(&self) -> i64 {
        // Estimate prompt tokens (rough: 4 chars per token)
        let prompt_tokens: usize = self.messages.iter().map(|m| m.content.len() / 4).sum();

        // Estimate completion tokens
        let completion_tokens = self.max_tokens.unwrap_or(1000) as usize;

        // Use conservative pricing (GPT-4 rates)
        let prompt_cost = (prompt_tokens as f64 * 0.03 / 1000.0) * 100.0; // Convert to cents
        let completion_cost = (completion_tokens as f64 * 0.06 / 1000.0) * 100.0;

        ((prompt_cost + completion_cost) * 1.5).ceil() as i64 // Add 50% safety margin
    }

    /// Get budget ID or default
    ///
    /// # Returns
    /// - Numeric BudgetId from request
    /// - DEFAULT_BUDGET_ID (0) if not specified
    pub fn budget_id(&self) -> BudgetId {
        self.budget_id.unwrap_or(DEFAULT_BUDGET_ID)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_cost() {
        let req = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "a".repeat(4000), // ~1000 tokens
                name: None,
            }],
            temperature: None,
            max_tokens: Some(1000),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: false,
            budget_id: None,
        };

        let cost = req.estimate_cost_cents();
        assert!(cost > 0);
        assert!(cost < 1000_00); // Less than $1000
    }

    #[test]
    fn test_budget_id_default() {
        let req = ChatCompletionRequest {
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

        assert_eq!(req.budget_id(), DEFAULT_BUDGET_ID);
    }

    #[test]
    fn test_budget_id_custom() {
        let req = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: false,
            budget_id: Some(12345),
        };

        assert_eq!(req.budget_id(), 12345);
    }

    #[test]
    fn test_budget_id_numeric() {
        let req = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: false,
            budget_id: Some(999),
        };

        assert_eq!(req.budget_id(), 999);
    }
}
