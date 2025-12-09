//! Property Tests for Request Coalescing (T28 Q8-Q14)
//!
//! **Coverage**:
//! - Q8: Concurrent correctness (100 threads)
//! - Q9: State machine invariants (no invalid transitions)
//! - Q10: Hash collision handling (linear probing)
//! - Q11: Waiter count accuracy (concurrent increments)
//! - Q12: Response sharing safety (Arc<Mutex>)
//! - Q13: Cleanup safety (no data races)
//! - Q14: Metrics consistency (counters never negative)

use clapi_core::capsules::coalescence::{CoalescenceEntry128, CoalescenceState};
use clapi_core::proxy::coalescing::CoalescingRegistry;
use clapi_core::proxy::types::{ChatCompletionResponse, Usage};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_concurrent_coordinators_different_hashes() {
    // Q8: 100 concurrent requests with different hashes
    let registry = Arc::new(CoalescingRegistry::new());
    let handles: Vec<_> = (0..100)
        .map(|i| {
            let registry = Arc::clone(&registry);
            thread::spawn(move || {
                let request = format!(r#"{{"model":"gpt-4","messages":[],"id":{}}}"#, i);
                let (is_coordinator, _slot, _response) = registry.lookup_or_insert(&request);
                is_coordinator
            })
        })
        .collect();

    let coordinator_count: usize = handles
        .into_iter()
        .map(|h| h.join().unwrap() as usize)
        .sum();

    // All requests should be coordinators (different hashes)
    assert_eq!(coordinator_count, 100);
}

#[test]
fn test_concurrent_waiters_same_hash() {
    // Q8: 100 concurrent identical requests
    let registry = Arc::new(CoalescingRegistry::new());
    let request = r#"{"model":"gpt-4","messages":[]}"#;

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let registry = Arc::clone(&registry);
            let request = request.to_string();
            thread::spawn(move || {
                let (is_coordinator, _slot, _response) = registry.lookup_or_insert(&request);
                is_coordinator
            })
        })
        .collect();

    let coordinator_count: usize = handles
        .into_iter()
        .map(|h| h.join().unwrap() as usize)
        .sum();

    // Exactly 1 coordinator, rest are waiters
    assert_eq!(coordinator_count, 1);
}

#[test]
fn test_state_machine_no_invalid_transitions() {
    // Q9: State machine invariants
    let entry = CoalescenceEntry128::new();

    // Empty → Pending (valid)
    assert_eq!(entry.get_state(), CoalescenceState::Empty);
    assert!(entry.try_claim(0x1234));
    assert_eq!(entry.get_state(), CoalescenceState::Pending);

    // Pending → Pending (invalid, should fail)
    assert!(!entry.try_claim(0x5678));
    assert_eq!(entry.get_state(), CoalescenceState::Pending);

    // Pending → Completed (valid)
    entry.mark_completed();
    assert_eq!(entry.get_state(), CoalescenceState::Completed);

    // Completed → Expired (valid)
    entry.mark_expired();
    assert_eq!(entry.get_state(), CoalescenceState::Expired);

    // Expired → Empty (via reset, valid)
    entry.reset();
    assert_eq!(entry.get_state(), CoalescenceState::Empty);
}

#[test]
fn test_concurrent_waiter_increments() {
    // Q11: Waiter count accuracy under concurrency
    let entry = Arc::new(CoalescenceEntry128::new());
    entry.try_claim(0x9999);

    let handles: Vec<_> = (0..1000)
        .map(|_| {
            let entry = Arc::clone(&entry);
            thread::spawn(move || {
                entry.add_waiter();
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(entry.get_waiter_count(), 1000);
}

#[test]
fn test_response_sharing_safety() {
    // Q12: Arc<Mutex> ensures safe cross-thread access
    let registry = Arc::new(CoalescingRegistry::new());
    let request = r#"{"model":"gpt-4","messages":[]}"#;

    // Coordinator thread
    let registry_coord = Arc::clone(&registry);
    let request_coord = request.to_string();
    let coord_handle = thread::spawn(move || {
        let (is_coordinator, slot, _response) = registry_coord.lookup_or_insert(&request_coord);
        assert!(is_coordinator);

        // Simulate API call
        thread::sleep(Duration::from_millis(10));

        let response = ChatCompletionResponse {
            id: "test-123".to_string(),
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

        registry_coord.complete_request(slot, Ok(response));
    });

    // Waiter threads
    let waiter_handles: Vec<_> = (0..10)
        .map(|_| {
            let registry = Arc::clone(&registry);
            let request = request.to_string();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(1)); // Ensure coordinator starts first
                let (is_coordinator, _slot, shared_response) = registry.lookup_or_insert(&request);
                assert!(!is_coordinator); // Should be waiter

                // Wait for response
                let mut attempts = 0;
                loop {
                    if let Ok(guard) = shared_response.lock() {
                        if guard.is_some() {
                            return true; // Response received
                        }
                    }
                    thread::sleep(Duration::from_micros(100));
                    attempts += 1;
                    if attempts > 1000 {
                        return false; // Timeout
                    }
                }
            })
        })
        .collect();

    coord_handle.join().unwrap();

    // All waiters should receive response
    for handle in waiter_handles {
        assert!(handle.join().unwrap());
    }
}

#[test]
fn test_cleanup_concurrent_safety() {
    // Q13: Cleanup doesn't cause data races
    let registry = Arc::new(CoalescingRegistry::new());

    // Writer threads
    let writer_handles: Vec<_> = (0..50)
        .map(|i| {
            let registry = Arc::clone(&registry);
            thread::spawn(move || {
                let request = format!(r#"{{"model":"gpt-4","messages":[],"id":{}}}"#, i);
                registry.lookup_or_insert(&request);
            })
        })
        .collect();

    // Cleanup thread
    let registry_cleanup = Arc::clone(&registry);
    let cleanup_handle = thread::spawn(move || {
        for _ in 0..10 {
            registry_cleanup.cleanup_expired();
            thread::sleep(Duration::from_micros(100));
        }
    });

    for handle in writer_handles {
        handle.join().unwrap();
    }
    cleanup_handle.join().unwrap();

    // No panics = success
}

#[test]
fn test_metrics_consistency() {
    // Q14: Metrics counters never negative
    let registry = Arc::new(CoalescingRegistry::new());

    let handles: Vec<_> = (0..100)
        .map(|i| {
            let registry = Arc::clone(&registry);
            thread::spawn(move || {
                let request = format!(r#"{{"model":"gpt-4","messages":[],"id":{}}}"#, i % 10);
                registry.record_request();
                registry.lookup_or_insert(&request);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let snapshot = registry.snapshot();
    assert!(snapshot.total_requests >= snapshot.coalesced_requests);
    assert!(snapshot.total_requests >= snapshot.provider_calls);
    assert!(snapshot.provider_calls > 0);
    assert!(snapshot.coalesced_requests >= 0);
}

#[test]
fn test_linear_probing_collision_resolution() {
    // Q10: Linear probing handles hash collisions
    let registry = CoalescingRegistry::with_capacity(16);

    // Fill slots to force collisions
    let handles: Vec<_> = (0..32)
        .map(|i| {
            let request = format!(r#"{{"model":"gpt-4","messages":[],"id":{}}}"#, i);
            registry.lookup_or_insert(&request)
        })
        .collect();

    // All should get a slot (coordinator or via probing)
    for (is_coordinator, slot, _) in handles {
        assert!(slot < registry.capacity() || is_coordinator);
    }
}

#[test]
fn test_concurrent_complete_and_wait() {
    // Q12: Concurrent completion and waiting
    let registry = Arc::new(CoalescingRegistry::new());
    let request = r#"{"model":"gpt-4","messages":[]}"#;

    // Coordinator
    let registry_coord = Arc::clone(&registry);
    let request_coord = request.to_string();
    let coord_handle = thread::spawn(move || {
        let (is_coordinator, slot, _response) = registry_coord.lookup_or_insert(&request_coord);
        assert!(is_coordinator);

        thread::sleep(Duration::from_millis(5));

        let response = ChatCompletionResponse {
            id: "test-456".to_string(),
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

        registry_coord.complete_request(slot, Ok(response));
        slot
    });

    // Waiters
    let waiter_handles: Vec<_> = (0..20)
        .map(|_| {
            let registry = Arc::clone(&registry);
            let request = request.to_string();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(1));
                let (_is_coordinator, _slot, _shared_response) = registry.lookup_or_insert(&request);
                // Just verify no panics
            })
        })
        .collect();

    let coord_slot = coord_handle.join().unwrap();
    assert!(coord_slot < registry.capacity());

    for handle in waiter_handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_waiter_count_under_stress() {
    // Q11: Stress test waiter count (1000 threads)
    let entry = Arc::new(CoalescenceEntry128::new());
    entry.try_claim(0xDEAD_BEEF);

    let handles: Vec<_> = (0..1000)
        .map(|_| {
            let entry = Arc::clone(&entry);
            thread::spawn(move || {
                entry.add_waiter();
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(entry.get_waiter_count(), 1000);
}

#[test]
fn test_registry_capacity_limits() {
    // Q10: Registry respects capacity limits
    let registry = CoalescingRegistry::with_capacity(8);

    // Fill all slots
    for i in 0..8 {
        let request = format!(r#"{{"model":"gpt-4","messages":[],"unique":{}}}"#, i);
        registry.lookup_or_insert(&request);
    }

    // Additional request should still work (fallback mode)
    let request = r#"{"model":"gpt-4","messages":[],"overflow":true}"#;
    let (is_coordinator, _slot, _response) = registry.lookup_or_insert(request);
    assert!(is_coordinator); // Fallback to coordinator
}
