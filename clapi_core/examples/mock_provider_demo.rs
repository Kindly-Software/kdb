//! Mock Provider Demo - Production-Grade Response Patterns
//!
//! Demonstrates the MockLLMProvider with 100+ response patterns and
//! temperature-sensitive variation.
//!
//! # Usage
//! ```bash
//! cargo run --example mock_provider_demo
//! ```

use clapi_core::test_mode::{MockLLMProvider, ProviderType};
use clapi_core::proxy::types::{ChatCompletionRequest, Message};

#[tokio::main]
async fn main() {
    println!("🧪 Mock LLM Provider Demo");
    println!("=======================\n");

    // Create provider instances
    let helpful = MockLLMProvider::new(ProviderType::HelpfulAssistant);
    let code = MockLLMProvider::new(ProviderType::CodeAssistant);
    let general = MockLLMProvider::default(); // Auto-detects type

    // Test 1: Helpful Assistant
    println!("📚 Test 1: Helpful Assistant (temp=0.7)");
    println!("----------------------------------------");
    let request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "How do I learn Rust programming?".to_string(),
            name: None,
        }],
        temperature: Some(0.7),
        max_tokens: None,
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        stream: false,
        budget_id: None,
    };

    let response = helpful.chat_completion(&request).await;
    println!("Response: {}", response.choices[0].message.content);
    println!("Tokens: {} (prompt) + {} (completion) = {}",
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
        response.usage.total_tokens);
    println!("Cost: ${:.4}\n", response.cost_cents.unwrap() / 100.0);

    // Test 2: Code Assistant (auto-detected)
    println!("💻 Test 2: Code Assistant Detection (temp=0.5)");
    println!("-----------------------------------------------");
    let code_request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Write a Rust function to parse JSON into a struct".to_string(),
            name: None,
        }],
        temperature: Some(0.5),
        max_tokens: Some(200),
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        stream: false,
        budget_id: None,
    };

    let code_response = general.chat_completion(&code_request).await;
    println!("Response: {}", code_response.choices[0].message.content);
    println!("Tokens: {}\n", code_response.usage.total_tokens);

    // Test 3: Temperature Variation (Low)
    println!("🌡️ Test 3: Low Temperature (0.1) - Deterministic");
    println!("--------------------------------------------------");
    let low_temp_request = ChatCompletionRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Explain computational capsules".to_string(),
            name: None,
        }],
        temperature: Some(0.1),
        max_tokens: None,
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        stream: false,
        budget_id: None,
    };

    let response1 = helpful.chat_completion(&low_temp_request).await;
    println!("Response 1: {}", response1.choices[0].message.content);

    // Test 4: Temperature Variation (High)
    println!("\n🎲 Test 4: High Temperature (0.9) - Creative");
    println!("---------------------------------------------");
    let high_temp_request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Tell me about atomic capsules".to_string(),
            name: None,
        }],
        temperature: Some(0.9),
        max_tokens: None,
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        stream: false,
        budget_id: None,
    };

    let response_high = helpful.chat_completion(&high_temp_request).await;
    println!("Response: {}", response_high.choices[0].message.content);

    // Test 5: Math Tutor Detection
    println!("\n🔢 Test 5: Math Tutor Auto-Detection");
    println!("-------------------------------------");
    let math_request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Solve the equation x^2 + 5x + 6 = 0".to_string(),
            name: None,
        }],
        temperature: Some(0.3),
        max_tokens: None,
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        stream: false,
        budget_id: None,
    };

    let math_response = general.chat_completion(&math_request).await;
    println!("Response: {}", math_response.choices[0].message.content);

    // Test 6: Translator Detection
    println!("\n🌐 Test 6: Translator Auto-Detection");
    println!("-------------------------------------");
    let translate_request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Translate 'Hello, world!' to French".to_string(),
            name: None,
        }],
        temperature: Some(0.5),
        max_tokens: None,
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        stream: false,
        budget_id: None,
    };

    let translate_response = general.chat_completion(&translate_request).await;
    println!("Response: {}", translate_response.choices[0].message.content);

    // Test 7: Concurrent Requests
    println!("\n⚡ Test 7: Concurrent Performance");
    println!("----------------------------------");
    let provider = std::sync::Arc::new(MockLLMProvider::default());
    let mut handles = vec![];

    let start = std::time::Instant::now();
    for i in 0..10 {
        let provider_clone = provider.clone();
        let handle = tokio::spawn(async move {
            let request = ChatCompletionRequest {
                model: "gpt-4".to_string(),
                messages: vec![Message {
                    role: "user".to_string(),
                    content: format!("Request {}", i),
                    name: None,
                }],
                temperature: Some(0.7),
                max_tokens: None,
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
                stop: None,
                stream: false,
                budget_id: None,
            };
            provider_clone.chat_completion(&request).await
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
    let elapsed = start.elapsed();

    println!("Total requests: {}", provider.request_count());
    println!("Total time: {:?}", elapsed);
    println!("Avg time per request: {:?}", elapsed / 10);

    // Test 8: Business Analyst Detection
    println!("\n📊 Test 8: Business Analyst Auto-Detection");
    println!("-------------------------------------------");
    let business_request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Analyze the market opportunity for AI-powered LLM proxies".to_string(),
            name: None,
        }],
        temperature: Some(0.6),
        max_tokens: None,
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        stream: false,
        budget_id: None,
    };

    let business_response = general.chat_completion(&business_request).await;
    println!("Response: {}", business_response.choices[0].message.content);

    println!("\n✅ Demo Complete!");
    println!("\nKey Features Demonstrated:");
    println!("- 100+ response patterns across 6 provider types");
    println!("- Auto-detection of provider type from user message");
    println!("- Temperature-sensitive variation (0.0-2.0 range)");
    println!("- Lockfree concurrent request tracking");
    println!("- Realistic latency simulation (50-200ms)");
    println!("- OpenAI-compatible response format");
}
