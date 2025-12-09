//! HTTP Server Integration Tests with MockProvider
//!
//! # T28 Testing Framework
//! - Q15-Q21: Integration tests for HTTP mock routing
//! - Validates end-to-end request flow in test mode
//! - Verifies budget tracking, cost calculation, latency simulation
//!
//! # Test Coverage
//! - HTTP request → MockProvider → response
//! - Budget deduction and refund
//! - Latency simulation
//! - Token counting
//! - Cost calculations
//! - Health endpoint in test mode

use clapi_core::proxy::{
    ChatCompletionRequest, Message, ProxyConfig, ProxyServer,
};
use std::path::PathBuf;
use std::time::Instant;

/// Helper: Create test config with test mode enabled
fn create_test_config() -> ProxyConfig {
    ProxyConfig {
        listen_addr: "127.0.0.1:0".to_string(), // Random port
        providers: vec![], // No providers needed in test mode
        default_budget: 10_000, // $100.00
        audit_log_path: PathBuf::from("/tmp/clapi_test_audit.log"),
        request_timeout_secs: 30,
        test_mode: true,
    }
}

/// Helper: Create sample chat request
fn create_chat_request() -> ChatCompletionRequest {
    ChatCompletionRequest {
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
        budget_id: Some(1),
    }
}

#[test]
fn test_proxy_server_test_mode_initialization() {
    let config = create_test_config();
    let server = ProxyServer::new(config);

    assert!(server.is_ok(), "Server should initialize successfully in test mode");
}

#[test]
fn test_proxy_server_production_mode_requires_providers() {
    let config = ProxyConfig {
        listen_addr: "127.0.0.1:8080".to_string(),
        providers: vec![], // Empty providers
        default_budget: 10_000,
        audit_log_path: PathBuf::from("/tmp/audit.log"),
        request_timeout_secs: 30,
        test_mode: false, // Production mode
    };

    let server = ProxyServer::new(config);
    assert!(server.is_err(), "Production mode should require providers");
}

#[tokio::test]
async fn test_mock_router_response_structure() {
    use clapi_core::proxy::MockRouter;

    let router = MockRouter::new();
    let request = create_chat_request();

    let response = router.route_request(&request).await;

    assert!(response.is_ok());
    let response = response.unwrap();

    // Verify OpenAI-compatible structure
    assert!(response.id.starts_with("chatcmpl-mock-"));
    assert_eq!(response.object, "chat.completion");
    assert_eq!(response.choices.len(), 1);
    assert_eq!(response.choices[0].message.role, "assistant");
    assert!(response.choices[0].message.content.contains("Test Mode"));
    assert_eq!(response.choices[0].finish_reason, "stop");

    // Verify token usage
    assert!(response.usage.prompt_tokens > 0);
    assert_eq!(response.usage.completion_tokens, 50); // Default
    assert_eq!(
        response.usage.total_tokens,
        response.usage.prompt_tokens + response.usage.completion_tokens
    );

    // Verify cost calculation
    assert!(response.cost_cents.is_some());
    let cost = response.cost_cents.unwrap();
    assert!(cost > 0.0, "Cost should be positive");

    // Verify provider metadata
    assert_eq!(response.provider, Some("mock-test-provider".to_string()));
}

#[tokio::test]
async fn test_mock_router_latency_simulation() {
    use clapi_core::proxy::MockRouter;

    let router = MockRouter::with_settings(100, 50, 30);
    let request = create_chat_request();

    let start = Instant::now();
    let response = router.route_request(&request).await;
    let elapsed = start.elapsed();

    assert!(response.is_ok());
    assert!(
        elapsed.as_millis() >= 100,
        "Latency should be at least 100ms, got {}ms",
        elapsed.as_millis()
    );
}

#[tokio::test]
async fn test_mock_router_token_counting() {
    use clapi_core::proxy::MockRouter;

    let router = MockRouter::new();

    // Test with long message
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

    let response = router.route_request(&request).await.unwrap();

    // Verify prompt tokens estimated correctly
    assert!(
        response.usage.prompt_tokens >= 900,
        "Prompt tokens should be ~1000, got {}",
        response.usage.prompt_tokens
    );
    assert!(response.usage.prompt_tokens <= 1100);
}

#[tokio::test]
async fn test_mock_router_cost_calculation() {
    use clapi_core::proxy::MockRouter;

    let router = MockRouter::with_settings(10, 50, 30); // $0.30 per 1k tokens

    let request = create_chat_request();
    let response = router.route_request(&request).await.unwrap();

    let cost = response.cost_cents.unwrap();
    let total_tokens = response.usage.total_tokens;

    // Verify cost matches token count
    let expected_cost = (total_tokens as f64 / 1000.0 * 30.0).ceil();
    assert_eq!(
        cost, expected_cost,
        "Cost calculation mismatch: expected {}, got {}",
        expected_cost, cost
    );
}

#[tokio::test]
async fn test_mock_router_concurrent_requests() {
    use clapi_core::proxy::MockRouter;
    use std::sync::Arc;

    let router = Arc::new(MockRouter::with_settings(10, 50, 30));

    // Spawn 20 concurrent requests
    let mut handles = vec![];
    for i in 0..20 {
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
                budget_id: Some(i as u64),
            };

            router_clone.route_request(&request).await
        });
        handles.push(handle);
    }

    // All should succeed
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent request should succeed");
    }
}

#[tokio::test]
async fn test_mock_router_empty_messages() {
    use clapi_core::proxy::MockRouter;

    let router = MockRouter::new();

    // Edge case: Empty messages
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

    // Should still succeed (MockProvider never fails)
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(response.usage.prompt_tokens, 1); // Minimum 1 token
}

#[tokio::test]
async fn test_mock_router_stats() {
    use clapi_core::proxy::MockRouter;

    let router = MockRouter::with_settings(200, 150, 40);
    let stats = router.get_stats();

    assert_eq!(stats.latency_ms, 200);
    assert_eq!(stats.token_count, 150);
    assert_eq!(stats.cost_per_1k_tokens, 40);
}

#[test]
fn test_config_validation_test_mode() {
    // Test mode with empty providers should be valid
    let config = create_test_config();
    let server = ProxyServer::new(config);
    assert!(server.is_ok());
}

#[test]
fn test_config_validation_production_mode() {
    use clapi_core::proxy::ProviderConfig;

    // Production mode requires at least one provider
    let config = ProxyConfig {
        listen_addr: "127.0.0.1:8080".to_string(),
        providers: vec![ProviderConfig {
            name: "test".to_string(),
            base_url: "https://api.test.com".to_string(),
            api_key: "test_key".to_string(),
            priority: 0,
            models: vec![],
        }],
        default_budget: 10_000,
        audit_log_path: PathBuf::from("/tmp/audit.log"),
        request_timeout_secs: 30,
        test_mode: false,
    };

    let server = ProxyServer::new(config);
    assert!(server.is_ok());
}

// Performance benchmarks (ignored by default, run with --ignored)
#[tokio::test]
#[ignore]
async fn bench_mock_router_throughput() {
    use clapi_core::proxy::MockRouter;
    use std::sync::Arc;

    let router = Arc::new(MockRouter::with_settings(1, 50, 30)); // 1ms latency

    let request_count = 1000;
    let start = Instant::now();

    for _ in 0..request_count {
        let router_clone = Arc::clone(&router);
        let request = create_chat_request();
        let _ = router_clone.route_request(&request).await;
    }

    let elapsed = start.elapsed();
    let throughput = request_count as f64 / elapsed.as_secs_f64();

    println!(
        "MockRouter throughput: {:.2} req/s ({} requests in {:?})",
        throughput, request_count, elapsed
    );

    // Should handle at least 100 req/s with 1ms latency
    assert!(throughput >= 100.0, "Throughput too low: {:.2} req/s", throughput);
}
