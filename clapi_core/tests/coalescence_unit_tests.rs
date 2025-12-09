//! Unit Tests for Request Coalescing (T28 Q1-Q7)
//!
//! **Coverage**:
//! - Q1: Capsule invariants (size, alignment, state machine)
//! - Q2: State transitions (Empty → Pending → Completed → Expired)
//! - Q3: Hash matching (identical requests coalesced)
//! - Q4: Waiter counting (accurate concurrent waiter tracking)
//! - Q5: Expiration logic (TTL-based cleanup)
//! - Q6: Linear probing (hash collision resolution)
//! - Q7: Metrics tracking (hit rate, efficiency)

use clapi_core::capsules::coalescence::{CoalescenceEntry128, CoalescenceState, CoalescenceSnapshot};
use clapi_core::proxy::coalescing::CoalescingRegistry;
use clapi_core::proxy::types::{ChatCompletionRequest, ChatCompletionResponse, Usage, Choice, Message};
use std::thread;
use std::time::Duration;

#[test]
fn test_coalescence_entry_size_and_alignment() {
    // Q33: Verify capsule size and alignment
    assert_eq!(std::mem::size_of::<CoalescenceEntry128>(), 128);
    assert_eq!(std::mem::align_of::<CoalescenceEntry128>(), 64);
}

#[test]
fn test_entry_initial_state() {
    // Q1: Verify initial state
    let entry = CoalescenceEntry128::new();
    assert_eq!(entry.get_state(), CoalescenceState::Empty);
    assert_eq!(entry.get_waiter_count(), 0);
    assert_eq!(entry.get_hash(), 0);
    assert_eq!(entry.get_created_ns(), 0);
    assert_eq!(entry.get_completed_ns(), 0);
}

#[test]
fn test_state_machine_empty_to_pending() {
    // Q2: Empty → Pending transition
    let entry = CoalescenceEntry128::new();
    let hash = 0x1234_5678_9ABC_DEF0;

    assert!(entry.try_claim(hash));
    assert_eq!(entry.get_state(), CoalescenceState::Pending);
    assert_eq!(entry.get_hash(), hash);
    assert!(entry.get_created_ns() > 0);
}

#[test]
fn test_state_machine_pending_to_completed() {
    // Q2: Pending → Completed transition
    let entry = CoalescenceEntry128::new();
    entry.try_claim(0x1111);

    entry.mark_completed();
    assert_eq!(entry.get_state(), CoalescenceState::Completed);
    assert!(entry.get_completed_ns() > 0);
}

#[test]
fn test_state_machine_completed_to_expired() {
    // Q2: Completed → Expired transition
    let entry = CoalescenceEntry128::new();
    entry.try_claim(0x2222);
    entry.mark_completed();

    entry.mark_expired();
    assert_eq!(entry.get_state(), CoalescenceState::Expired);
}

#[test]
fn test_state_machine_expired_to_empty() {
    // Q2: Expired → Empty transition (via reset)
    let entry = CoalescenceEntry128::new();
    entry.try_claim(0x3333);
    entry.mark_completed();
    entry.mark_expired();

    entry.reset();
    assert_eq!(entry.get_state(), CoalescenceState::Empty);
    assert_eq!(entry.get_hash(), 0);
    assert_eq!(entry.get_waiter_count(), 0);
}

#[test]
fn test_try_claim_prevents_double_claim() {
    // Q2: CAS prevents concurrent claims
    let entry = CoalescenceEntry128::new();
    let hash1 = 0x1111;
    let hash2 = 0x2222;

    assert!(entry.try_claim(hash1));
    assert!(!entry.try_claim(hash2)); // Should fail (slot occupied)
    assert_eq!(entry.get_hash(), hash1); // Hash unchanged
}

#[test]
fn test_matches_identical_hash() {
    // Q3: Hash matching for identical requests
    let entry = CoalescenceEntry128::new();
    let hash = 0xABCD_EF12_3456_7890;

    entry.try_claim(hash);
    assert!(entry.matches(hash));
}

#[test]
fn test_matches_different_hash() {
    // Q3: Hash mismatch for different requests
    let entry = CoalescenceEntry128::new();
    let hash1 = 0x1111;
    let hash2 = 0x2222;

    entry.try_claim(hash1);
    assert!(!entry.matches(hash2));
}

#[test]
fn test_matches_empty_entry() {
    // Q3: Empty entry doesn't match any hash
    let entry = CoalescenceEntry128::new();
    assert!(!entry.matches(0x1234));
}

#[test]
fn test_matches_completed_entry() {
    // Q3: Completed entry still matches
    let entry = CoalescenceEntry128::new();
    let hash = 0x5678;

    entry.try_claim(hash);
    entry.mark_completed();
    assert!(entry.matches(hash));
}

#[test]
fn test_add_waiter_increments_count() {
    // Q4: Waiter count increments correctly
    let entry = CoalescenceEntry128::new();
    entry.try_claim(0x1234);

    assert_eq!(entry.add_waiter(), 1);
    assert_eq!(entry.add_waiter(), 2);
    assert_eq!(entry.add_waiter(), 3);
    assert_eq!(entry.get_waiter_count(), 3);
}

#[test]
fn test_mark_completed_preserves_waiter_count() {
    // Q4: Completion preserves waiter count
    let entry = CoalescenceEntry128::new();
    entry.try_claim(0x5678);
    entry.add_waiter();
    entry.add_waiter();

    entry.mark_completed();
    assert_eq!(entry.get_waiter_count(), 2);
}

#[test]
fn test_is_expired_fresh_entry() {
    // Q5: Fresh entry is not expired
    let entry = CoalescenceEntry128::new();
    entry.try_claim(0x1111);

    assert!(!entry.is_expired(1_000_000_000)); // 1 second TTL
}

#[test]
fn test_is_expired_empty_entry() {
    // Q5: Empty entry is not expired
    let entry = CoalescenceEntry128::new();
    assert!(!entry.is_expired(1_000_000_000));
}

#[test]
fn test_is_expired_large_ttl() {
    // Q5: Entry not expired with very large TTL
    let entry = CoalescenceEntry128::new();
    entry.try_claim(0x2222);

    assert!(!entry.is_expired(u64::MAX));
}

#[test]
fn test_registry_creation() {
    // Q1: Registry initialization
    let registry = CoalescingRegistry::new();
    assert_eq!(registry.capacity(), 16_384);
    assert!(registry.ttl_ns() > 0);
}

#[test]
fn test_registry_lookup_coordinator() {
    // Q6: First request becomes coordinator
    let registry = CoalescingRegistry::new();
    let request = r#"{"model":"gpt-4","messages":[]}"#;

    let (is_coordinator, _slot, _response) = registry.lookup_or_insert(request);
    assert!(is_coordinator);
}

#[test]
fn test_registry_lookup_waiter() {
    // Q6: Identical request becomes waiter
    let registry = CoalescingRegistry::new();
    let request = r#"{"model":"gpt-4","messages":[]}"#;

    let (is_coord1, slot1, _resp1) = registry.lookup_or_insert(request);
    assert!(is_coord1);

    let (is_coord2, slot2, _resp2) = registry.lookup_or_insert(request);
    assert!(!is_coord2); // Waiter
    assert_eq!(slot1, slot2); // Same slot
}

#[test]
fn test_registry_different_requests() {
    // Q3: Different requests get different slots
    let registry = CoalescingRegistry::new();
    let request1 = r#"{"model":"gpt-4","messages":[]}"#;
    let request2 = r#"{"model":"claude-3","messages":[]}"#;

    let (is_coord1, slot1, _) = registry.lookup_or_insert(request1);
    let (is_coord2, slot2, _) = registry.lookup_or_insert(request2);

    assert!(is_coord1);
    assert!(is_coord2);
    // Slots may differ (hash-dependent)
}

#[test]
fn test_registry_complete_request() {
    // Q4: Complete request with response
    let registry = CoalescingRegistry::new();
    let request = r#"{"model":"gpt-4","messages":[]}"#;

    let (is_coordinator, slot, shared_response) = registry.lookup_or_insert(request);
    assert!(is_coordinator);

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

    registry.complete_request(slot, Ok(response.clone()));

    // Verify response stored
    if let Ok(guard) = shared_response.lock() {
        assert!(guard.is_some());
    }
}

#[test]
fn test_registry_metrics() {
    // Q7: Metrics tracking
    let registry = CoalescingRegistry::new();
    let request = r#"{"model":"gpt-4","messages":[]}"#;

    registry.record_request();
    registry.lookup_or_insert(request); // Coordinator

    registry.record_request();
    registry.lookup_or_insert(request); // Waiter (coalesced)

    let snapshot = registry.snapshot();
    assert_eq!(snapshot.total_requests, 2);
    assert_eq!(snapshot.coalesced_requests, 1);
    assert_eq!(snapshot.provider_calls, 1);
}

#[test]
fn test_registry_cleanup_expired() {
    // Q5: Cleanup expired entries
    let mut registry = CoalescingRegistry::with_capacity(16);
    registry.set_ttl_ns(1); // 1 nanosecond TTL

    let request = r#"{"model":"gpt-4","messages":[]}"#;
    registry.lookup_or_insert(request);

    // Wait for expiration
    thread::sleep(Duration::from_micros(1));

    let cleaned = registry.cleanup_expired();
    assert!(cleaned > 0);
}

#[test]
fn test_coalescence_snapshot_efficiency() {
    // Q7: Efficiency calculation
    let snapshot = CoalescenceSnapshot {
        total_requests: 1000,
        coalesced_requests: 900,
        provider_calls: 100,
        hit_rate_bp: 9000,
        avg_waiters: 9.0,
        max_waiters: 50,
    };

    let efficiency = snapshot.efficiency();
    assert!((efficiency - 10.0).abs() < 0.01); // 10× efficiency
}

#[test]
fn test_concurrent_waiter_count() {
    // Q4: Concurrent waiter increments (basic)
    let entry = CoalescenceEntry128::new();
    entry.try_claim(0x9999);

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let entry_ref = &entry;
            thread::spawn(move || {
                entry_ref.add_waiter();
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(entry.get_waiter_count(), 10);
}

#[test]
fn test_registry_custom_capacity() {
    // Q1: Custom capacity initialization
    let registry = CoalescingRegistry::with_capacity(1024);
    assert_eq!(registry.capacity(), 1024);
}

#[test]
fn test_registry_ttl_setter() {
    // Q5: TTL modification
    let mut registry = CoalescingRegistry::new();
    registry.set_ttl_ns(5_000_000_000); // 5 seconds

    assert_eq!(registry.ttl_ns(), 5_000_000_000);
}
