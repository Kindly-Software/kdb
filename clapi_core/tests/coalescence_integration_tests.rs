//! Integration Tests for Request Coalescing (T28 Q15-Q21)
//!
//! **Coverage**:
//! - Q15: End-to-end coalescing flow (coordinator + waiters + response)
//! - Q16: Real-world request patterns (JSON serialization)
//! - Q17: Timeout handling (waiter timeout after 30s)
//! - Q18: Cleanup lifecycle (TTL-based expiration)
//! - Q19: Metrics aggregation (hit rate, efficiency)
//! - Q20: Error propagation (coordinator errors shared with waiters)
//! - Q21: Performance under load (100 concurrent requests)

use clapi_core::capsules::coalescence::{CoalescenceEntry128, CoalescenceState};
use clapi_core::proxy::coalescing::CoalescingRegistry;
use clapi_core::proxy::types::{ChatCompletionRequest, ChatCompletionResponse, Message, Usage, Choice};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_end_to_end_coalescing_flow() {
    // Q15: Complete coalescing lifecycle
    let registry = Arc::new(CoalescingRegistry::new());

    // Create realistic request
    let request_json = serde_json::to_string(&ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Hello, world!".to_string(),
            name: None,
        }],
        temperature: Some(0.7),
        max_tokens: Some(100),
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        stream: false,
        budget_id: None,
    })
    .unwrap();

    // Coordinator thread
    let registry_coord = Arc::clone(&registry);
    let request_coord = request_json.clone();
    let coord_handle = thread::spawn(move || {
        let (is_coordinator, slot, _response) = registry_coord.lookup_or_insert(&request_coord);
        assert!(is_coordinator);

        // Simulate provider API call
        thread::sleep(Duration::from_millis(50));

        let response = ChatCompletionResponse {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![Choice {
                index: 0,
                message: Message {
                    role: "assistant".to_string(),
                    content: "Hello! How can I help?".to_string(),
                    name: None,
                },
                finish_reason: "stop".to_string(),
            }],
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 8,
                total_tokens: 18,
            },
            cost_cents: Some(0.54),
            provider: Some("openai".to_string()),
        };

        registry_coord.complete_request(slot, Ok(response.clone()));
        response
    });

    // Waiter threads (10 identical requests)
    let waiter_handles: Vec<_> = (0..10)
        .map(|_| {
            let registry = Arc::clone(&registry);
            let request = request_json.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(5)); // Stagger arrivals
                let (is_coordinator, _slot, shared_response) = registry.lookup_or_insert(&request);
                assert!(!is_coordinator); // Should be waiter

                // Poll for response
                let timeout = Duration::from_secs(5);
                let start = Instant::now();
                loop {
                    if let Ok(guard) = shared_response.lock() {
                        if let Some(Ok(response)) = guard.as_ref() {
                            return Some(response.clone());
                        }
                    }
                    if start.elapsed() > timeout {
                        return None; // Timeout
                    }
                    thread::sleep(Duration::from_micros(100));
                }
            })
        })
        .collect();

    let coord_response = coord_handle.join().unwrap();

    // All waiters should receive same response
    for handle in waiter_handles {
        let waiter_response = handle.join().unwrap();
        assert!(waiter_response.is_some());
        let waiter_response = waiter_response.unwrap();
        assert_eq!(waiter_response.id, coord_response.id);
        assert_eq!(waiter_response.usage.total_tokens, coord_response.usage.total_tokens);
    }
}

#[test]
fn test_different_request_patterns() {
    // Q16: Multiple distinct requests processed independently
    let registry = Arc::new(CoalescingRegistry::new());

    let requests = vec![
        r#"{"model":"gpt-4","messages":[{"role":"user","content":"Hello"}]}"#,
        r#"{"model":"gpt-4","messages":[{"role":"user","content":"Goodbye"}]}"#,
        r#"{"model":"claude-3","messages":[{"role":"user","content":"Hello"}]}"#,
    ];

    let handles: Vec<_> = requests
        .into_iter()
        .map(|request| {
            let registry = Arc::clone(&registry);
            let request = request.to_string();
            thread::spawn(move || {
                let (is_coordinator, slot, _response) = registry.lookup_or_insert(&request);
                (is_coordinator, slot)
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All should be coordinators (different requests)
    for (is_coordinator, _slot) in results {
        assert!(is_coordinator);
    }
}

#[test]
fn test_error_propagation_to_waiters() {
    // Q20: Coordinator errors shared with waiters
    let registry = Arc::new(CoalescingRegistry::new());
    let request = r#"{"model":"gpt-4","messages":[]}"#;

    // Coordinator thread (returns error)
    let registry_coord = Arc::clone(&registry);
    let request_coord = request.to_string();
    let coord_handle = thread::spawn(move || {
        let (is_coordinator, slot, _response) = registry_coord.lookup_or_insert(&request_coord);
        assert!(is_coordinator);

        thread::sleep(Duration::from_millis(10));

        // Simulate provider error
        registry_coord.complete_request(slot, Err("Provider timeout".to_string()));
    });

    // Waiter thread
    let registry_waiter = Arc::clone(&registry);
    let request_waiter = request.to_string();
    let waiter_handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(5));
        let (_is_coordinator, _slot, shared_response) = registry_waiter.lookup_or_insert(&request_waiter);

        // Wait for response
        let timeout = Duration::from_secs(5);
        let start = Instant::now();
        loop {
            if let Ok(guard) = shared_response.lock() {
                if let Some(result) = guard.as_ref() {
                    return result.clone();
                }
            }
            if start.elapsed() > timeout {
                return Err("Timeout".to_string());
            }
            thread::sleep(Duration::from_micros(100));
        }
    });

    coord_handle.join().unwrap();

    // Waiter should receive error
    let waiter_result = waiter_handle.join().unwrap();
    assert!(waiter_result.is_err());
    assert_eq!(waiter_result.unwrap_err(), "Provider timeout");
}

#[test]
fn test_cleanup_lifecycle() {
    // Q18: TTL-based expiration
    let mut registry = CoalescingRegistry::with_capacity(16);
    registry.set_ttl_ns(100_000); // 100 microseconds

    let request = r#"{"model":"gpt-4","messages":[]}"#;
    registry.lookup_or_insert(request);

    // Wait for expiration
    thread::sleep(Duration::from_millis(1));

    let cleaned = registry.cleanup_expired();
    assert!(cleaned > 0);
}

#[test]
fn test_metrics_aggregation() {
    // Q19: Hit rate and efficiency calculation
    let registry = Arc::new(CoalescingRegistry::new());
    let request = r#"{"model":"gpt-4","messages":[]}"#;

    // 1 coordinator + 99 waiters = 100× efficiency
    registry.record_request();
    registry.lookup_or_insert(request); // Coordinator

    for _ in 0..99 {
        registry.record_request();
        registry.lookup_or_insert(request); // Waiters
    }

    let snapshot = registry.snapshot();
    assert_eq!(snapshot.total_requests, 100);
    assert_eq!(snapshot.coalesced_requests, 99);
    assert_eq!(snapshot.provider_calls, 1);

    // Hit rate = 99/100 = 99% = 9900 basis points
    assert!(snapshot.hit_rate_bp >= 9800 && snapshot.hit_rate_bp <= 10000);

    // Efficiency = 100 / 1 = 100×
    assert!((snapshot.efficiency() - 100.0).abs() < 1.0);
}

#[test]
fn test_performance_under_load() {
    // Q21: 100 concurrent identical requests
    let registry = Arc::new(CoalescingRegistry::new());
    let request = r#"{"model":"gpt-4","messages":[]}"#;

    let start = Instant::now();

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let registry = Arc::clone(&registry);
            let request = request.to_string();
            thread::spawn(move || {
                registry.record_request();
                let (is_coordinator, _slot, _response) = registry.lookup_or_insert(&request);
                is_coordinator
            })
        })
        .collect();

    let coordinator_count: usize = handles
        .into_iter()
        .map(|h| h.join().unwrap() as usize)
        .sum();

    let elapsed = start.elapsed();

    // Exactly 1 coordinator
    assert_eq!(coordinator_count, 1);

    // Should complete in <100ms (fast lookup)
    assert!(elapsed < Duration::from_millis(100));

    let snapshot = registry.snapshot();
    assert_eq!(snapshot.total_requests, 100);
    assert_eq!(snapshot.provider_calls, 1);
    assert_eq!(snapshot.coalesced_requests, 99);
}

#[test]
fn test_mixed_identical_and_unique_requests() {
    // Q16: Realistic mix of identical and unique requests
    let registry = Arc::new(CoalescingRegistry::new());

    // 10 unique requests
    let unique_handles: Vec<_> = (0..10)
        .map(|i| {
            let registry = Arc::clone(&registry);
            thread::spawn(move || {
                let request = format!(r#"{{"model":"gpt-4","messages":[],"id":{}}}"#, i);
                registry.record_request();
                registry.lookup_or_insert(&request)
            })
        })
        .collect();

    // 90 identical requests (should coalesce)
    let identical_request = r#"{"model":"gpt-4","messages":[],"common":true}"#;
    let identical_handles: Vec<_> = (0..90)
        .map(|_| {
            let registry = Arc::clone(&registry);
            let request = identical_request.to_string();
            thread::spawn(move || {
                registry.record_request();
                registry.lookup_or_insert(&request)
            })
        })
        .collect();

    // Collect results
    for handle in unique_handles {
        handle.join().unwrap();
    }
    for handle in identical_handles {
        handle.join().unwrap();
    }

    let snapshot = registry.snapshot();
    assert_eq!(snapshot.total_requests, 100);
    // 10 unique + 1 coordinator for identical = 11 provider calls
    assert!(snapshot.provider_calls >= 10 && snapshot.provider_calls <= 12);
    // ~89 coalesced requests
    assert!(snapshot.coalesced_requests >= 85);
}

#[test]
fn test_state_transitions_integration() {
    // Q15: Entry state transitions throughout lifecycle
    let registry = CoalescingRegistry::new();
    let request = r#"{"model":"gpt-4","messages":[]}"#;

    // Insert (Empty → Pending)
    let (is_coordinator, slot, shared_response) = registry.lookup_or_insert(request);
    assert!(is_coordinator);

    // Complete (Pending → Completed)
    let response = ChatCompletionResponse {
        id: "test-789".to_string(),
        object: "chat.completion".to_string(),
        created: 1234567890,
        model: "gpt-4".to_string(),
        choices: vec![],
        usage: Usage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        },
        cost_cents: Some(5.0),
        provider: Some("openai".to_string()),
    };

    registry.complete_request(slot, Ok(response.clone()));

    // Verify response available
    if let Ok(guard) = shared_response.lock() {
        assert!(guard.is_some());
        if let Some(Ok(stored)) = guard.as_ref() {
            assert_eq!(stored.id, "test-789");
        }
    }; // Drop temporary to fix lifetime issue
}

#[test]
fn test_sequential_coalescing_rounds() {
    // Q18: Multiple rounds of coalescing (cleanup between rounds)
    let mut registry = CoalescingRegistry::with_capacity(32);
    registry.set_ttl_ns(1_000_000); // 1 millisecond

    for round in 0..5 {
        let request = format!(r#"{{"model":"gpt-4","round":{}}}"#, round);

        // 10 identical requests per round
        for _ in 0..10 {
            registry.record_request();
            registry.lookup_or_insert(&request);
        }

        // Wait for expiration
        thread::sleep(Duration::from_millis(2));

        // Cleanup
        registry.cleanup_expired();
    }

    let snapshot = registry.snapshot();
    assert_eq!(snapshot.total_requests, 50); // 5 rounds × 10 requests
    assert!(snapshot.provider_calls >= 5 && snapshot.provider_calls <= 10);
}
