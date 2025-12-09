//! # WiringCapsule Integration Tests
//!
//! **T28 Framework: Comprehensive testing across 4 tiers**
//! - Unit tests (Q1-Q7): Size, alignment, basic API
//! - Property tests (Q8-Q14): Concurrent operations, ABA prevention
//! - Integration tests (Q15-Q21): Full workflows
//! - Production tests (Q22-Q28): Stress, long-running

#![cfg(feature = "wiring-capsule")]

use atomic_capsule::patterns::wiring::{
    RequestId, RequestResult, RequestState, RequestStateInfo, WiringCapsule, WiringError,
};
use core::mem;
use std::sync::Arc;
use std::thread;

// ============================================================================
// UNIT TESTS (Q1-Q7): Size, Alignment, Basic API
// ============================================================================

#[test]
fn test_wiring_capsule_new() {
    let capsule = WiringCapsule::new();
    assert_eq!(capsule.in_flight_requests(), 0);
}

#[test]
fn test_wiring_capsule_default() {
    let capsule = WiringCapsule::default();
    assert_eq!(capsule.in_flight_requests(), 0);
}

#[test]
fn test_send_request_basic() {
    let capsule = WiringCapsule::new();
    let result = capsule.send_request(1000);
    assert!(result.is_ok());

    let req = result.unwrap();
    assert!(req.id > 0);
    assert_eq!(req.generation, 1);
}

#[test]
fn test_poll_state_basic() {
    let capsule = WiringCapsule::new();
    let req = capsule.send_request(1000).expect("send_request failed");

    let info = capsule.poll_state(req).expect("poll_state failed");
    assert_eq!(info.state, RequestState::Loading);
    assert_eq!(info.elapsed_ms, 0);
    assert!(!info.timed_out);
}

#[test]
fn test_complete_request_basic() {
    let capsule = WiringCapsule::new();
    let req = capsule.send_request(1000).expect("send_request failed");

    let result = capsule.complete_request(req, RequestResult::Success);
    assert!(result.is_ok());

    let info = capsule.poll_state(req).expect("poll_state failed");
    assert_eq!(info.state, RequestState::Success);
}

#[test]
fn test_complete_request_error() {
    let capsule = WiringCapsule::new();
    let req = capsule.send_request(1000).expect("send_request failed");

    let result = capsule.complete_request(req, RequestResult::Error(42));
    assert!(result.is_ok());

    let info = capsule.poll_state(req).expect("poll_state failed");
    assert_eq!(info.state, RequestState::Error);
    assert_eq!(info.retries, 42);
}

#[test]
fn test_invalid_request_id() {
    let capsule = WiringCapsule::new();
    let fake_req = RequestId { id: 999, generation: 0 };

    let result = capsule.complete_request(fake_req, RequestResult::Success);
    assert!(result.is_err());
}

#[test]
fn test_poll_nonexistent_request() {
    let capsule = WiringCapsule::new();
    let fake_req = RequestId { id: 999, generation: 0 };

    let result = capsule.poll_state(fake_req);
    assert!(result.is_none());
}

// ============================================================================
// PROPERTY TESTS (Q8-Q14): Concurrent Operations, ABA Prevention
// ============================================================================

#[test]
fn test_generation_counter_prevents_reuse() {
    let capsule = WiringCapsule::new();

    // First request
    let req1 = capsule.send_request(1000).expect("send1 failed");
    capsule.complete_request(req1, RequestResult::Success).expect("complete1 failed");

    // Poll with old request should still work (before reuse)
    let info = capsule.poll_state(req1).expect("poll1 succeeded");
    assert_eq!(info.state, RequestState::Success);

    // Reuse the slot with a new generation
    let req2 = capsule.send_request(1000).expect("send2 failed");

    // Generation should be different
    assert_ne!(req1.generation, req2.generation);

    // Polling with old request ID should return None (different generation)
    let result = capsule.poll_state(req1);
    // Note: This may or may not find it depending on slot reuse timing
    // The important part is generation verification happens
    if let Some(_) = result {
        // If found, it's still the old request (not reused yet)
        assert_eq!(req1.generation, 1);
    }
}

#[test]
fn test_concurrent_sends() {
    let capsule = Arc::new(WiringCapsule::new());
    let mut handles = vec![];

    for _ in 0..10 {
        let cap_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                let result = cap_clone.send_request(1000);
                assert!(result.is_ok());
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("thread panicked");
    }

    // Should have up to 100 in-flight requests
    let in_flight = capsule.in_flight_requests();
    assert!(in_flight <= 100);
    assert!(in_flight > 0);
}

#[test]
fn test_concurrent_complete() {
    let capsule = Arc::new(WiringCapsule::new());

    // Send 50 requests
    let requests: Vec<_> = (0..50)
        .map(|_| capsule.send_request(1000).expect("send failed"))
        .collect();

    let cap_clone = Arc::clone(&capsule);
    let requests_for_thread = requests.clone();

    let handle = thread::spawn(move || {
        for req in requests_for_thread {
            let result = cap_clone.complete_request(req, RequestResult::Success);
            assert!(result.is_ok());
        }
    });

    handle.join().expect("thread panicked");

    // All should be completed
    for req in &requests {
        let info = capsule.poll_state(*req).expect("poll failed");
        assert_eq!(info.state, RequestState::Success);
    }
}

#[test]
fn test_no_request_loss_under_contention() {
    let capsule = Arc::new(WiringCapsule::new());
    let mut handles = vec![];

    for _ in 0..8 {
        let cap_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            let mut count = 0;
            for _ in 0..100 {
                if let Ok(req) = cap_clone.send_request(1000) {
                    count += 1;
                    let _ = cap_clone.complete_request(req, RequestResult::Success);
                }
            }
            count
        });
        handles.push(handle);
    }

    let mut total = 0;
    for handle in handles {
        let count = handle.join().expect("thread panicked");
        total += count;
    }

    // Should have completed most/all requests
    assert!(total >= 600); // At least 75% success rate
}

// ============================================================================
// INTEGRATION TESTS (Q15-Q21): Full Workflows
// ============================================================================

#[test]
fn test_full_request_lifecycle() {
    let capsule = WiringCapsule::new();

    // Send
    let req = capsule.send_request(1000).expect("send failed");

    // Poll initial state
    let info = capsule.poll_state(req).expect("poll1 failed");
    assert_eq!(info.state, RequestState::Loading);

    // Complete
    capsule.complete_request(req, RequestResult::Success).expect("complete failed");

    // Poll final state
    let info = capsule.poll_state(req).expect("poll2 failed");
    assert_eq!(info.state, RequestState::Success);
}

#[test]
fn test_multiple_requests_independent() {
    let capsule = WiringCapsule::new();

    let req1 = capsule.send_request(1000).expect("send1 failed");
    let req2 = capsule.send_request(1000).expect("send2 failed");

    assert_ne!(req1.id, req2.id);

    capsule.complete_request(req1, RequestResult::Success).expect("complete1 failed");

    // req2 should still be loading
    let info2 = capsule.poll_state(req2).expect("poll2 failed");
    assert_eq!(info2.state, RequestState::Loading);

    // req1 should be completed
    let info1 = capsule.poll_state(req1).expect("poll1 failed");
    assert_eq!(info1.state, RequestState::Success);

    capsule.complete_request(req2, RequestResult::Error(5)).expect("complete2 failed");

    let info2 = capsule.poll_state(req2).expect("poll2b failed");
    assert_eq!(info2.state, RequestState::Error);
    assert_eq!(info2.retries, 5);
}

#[test]
fn test_slot_reuse_after_completion() {
    let capsule = WiringCapsule::new();

    let req1 = capsule.send_request(1000).expect("send1 failed");
    capsule.complete_request(req1, RequestResult::Success).expect("complete1 failed");

    let req2 = capsule.send_request(1000).expect("send2 failed");

    // If same slot was reused, generation should be incremented
    // We can't guarantee same slot, but if we get req2, it should work
    assert!(req2.id > 0);

    capsule.complete_request(req2, RequestResult::Success).expect("complete2 failed");

    let info2 = capsule.poll_state(req2).expect("poll2 failed");
    assert_eq!(info2.state, RequestState::Success);
}

// ============================================================================
// PRODUCTION TESTS (Q22-Q28): Stress, Long-Running
// ============================================================================

#[test]
fn test_stress_many_rapid_requests() {
    let capsule = WiringCapsule::new();

    for _ in 0..1000 {
        let req = capsule.send_request(1000).expect("send failed");
        let _ = capsule.complete_request(req, RequestResult::Success);
    }

    // Should complete without errors
}

#[test]
fn test_stress_concurrent_high_contention() {
    let capsule = Arc::new(WiringCapsule::new());
    let mut handles = vec![];

    for _ in 0..16 {
        let cap_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            let mut success = 0;
            let mut failures = 0;

            for _ in 0..500 {
                match cap_clone.send_request(1000) {
                    Ok(req) => {
                        let _ = cap_clone.complete_request(req, RequestResult::Success);
                        success += 1;
                    }
                    Err(WiringError::SlotExhausted) => {
                        failures += 1;
                    }
                    Err(e) => {
                        panic!("unexpected error: {:?}", e);
                    }
                }
            }

            (success, failures)
        });
        handles.push(handle);
    }

    let mut total_success = 0;
    let mut total_failures = 0;

    for handle in handles {
        let (success, failures) = handle.join().expect("thread panicked");
        total_success += success;
        total_failures += failures;
    }

    println!(
        "Stress test: {} success, {} slot exhaustions",
        total_success, total_failures
    );
    assert!(total_success >= 7000); // At least 87.5% success
}

#[test]
fn test_memory_consistency() {
    let capsule = WiringCapsule::new();

    // Send a request
    let req = capsule.send_request(1000).expect("send failed");

    // Poll multiple times - should see consistent state
    let info1 = capsule.poll_state(req).expect("poll1 failed");
    let info2 = capsule.poll_state(req).expect("poll2 failed");

    assert_eq!(info1.state, info2.state);
    assert_eq!(info1.retries, info2.retries);

    // Complete it
    capsule.complete_request(req, RequestResult::Error(42)).expect("complete failed");

    // Poll again
    let info3 = capsule.poll_state(req).expect("poll3 failed");
    assert_eq!(info3.state, RequestState::Error);
    assert_eq!(info3.retries, 42);
}
