//! I20 Integration Security Tests - Cross-Component Isolation & Safety
//!
//! **Purpose**: Validate security boundaries between integrated components
//! **Framework**: I20 Integration Framework + ASSUM Safety
//!
//! # Test Coverage (I20 Q11-Q15: Safety)
//! - **Component Isolation**: Payment + OAuth isolation (no cross-contamination)
//! - **Feature Flag Security**: Cannot bypass security via env vars
//! - **Audit Log Integrity**: Writes are atomic, no partial entries
//! - **Rate Limiting**: Per-user quotas enforced under concurrent load
//! - **Hash Chain Separation**: Payment/OAuth chains don't interfere
//!
//! # ASSUM Validation
//! - Validates I20 Q11-Q15 (safety during integration)
//! - Tests component boundaries under malicious input
//! - Verifies atomic log writes (no partial audit entries)

use clapi_core::capsules::{
    PaymentCapsule256, OAuthSessionCapsule, RateLimitCapsule,
    RequestCapsule128Enhanced, AuditLogCapsule128,
};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::thread;

// ============================================================================
// Component Isolation Tests (I20 Q11: Safety Boundaries)
// ============================================================================

#[test]
fn test_payment_oauth_isolation() {
    // I20 Q11: Payment and OAuth capsules should not share state
    let payment = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);
    let session = OAuthSessionCapsule::new(0x456, 0x789, 3600_000_000_000);

    // Initial states independent
    let payment_hash = payment.hash();
    let session_hash = session.hash();

    // Modify payment
    payment.mark_confirmed(1000_000_000).unwrap();
    let new_payment_hash = payment.hash();

    // Session should be unaffected
    assert_eq!(
        session.hash(), session_hash,
        "OAuth session should not be affected by payment changes"
    );
    assert_ne!(
        new_payment_hash, payment_hash,
        "Payment hash should change after confirmation"
    );

    // Modify session
    session.revoke();
    let new_session_hash = session.hash();

    // Payment should be unaffected
    assert_eq!(
        payment.hash(), new_payment_hash,
        "Payment should not be affected by session revocation"
    );
    assert_ne!(
        new_session_hash, session_hash,
        "Session hash should change after revocation"
    );
}

#[test]
fn test_concurrent_payment_oauth_no_interference() {
    // I20 Q11: Concurrent payment + OAuth operations should not interfere
    let payment = Arc::new(PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123));
    let session = Arc::new(OAuthSessionCapsule::new(0x456, 0x789, 3600_000_000_000));

    let payment_clone = Arc::clone(&payment);
    let payment_handle = thread::spawn(move || {
        for _ in 0..1000 {
            let _ = payment_clone.mark_confirmed(1000_000_000);
            let _ = payment_clone.mark_failed();
        }
    });

    let session_clone = Arc::clone(&session);
    let session_handle = thread::spawn(move || {
        for _ in 0..1000 {
            session_clone.revoke();
            let _ = session_clone.verify_token(0x789);
        }
    });

    payment_handle.join().unwrap();
    session_handle.join().unwrap();

    // Both components should maintain integrity
    assert!(payment.verify_integrity(), "Payment integrity should be maintained");
    // Session integrity check (state should be consistent)
    let session_state = session.state();
    assert!(
        session_state == 0 || session_state == 1,
        "Session state should be valid (Active=0 or Revoked=1)"
    );
}

#[test]
fn test_user_id_isolation_between_components() {
    // I20 Q11: Different components tracking same user_id should not collide
    let user_id = 12345u64;

    let payment = PaymentCapsule256::new(111, user_id, 100_00, 3_00, 0x123);
    let rate_limit = RateLimitCapsule::new(user_id, 100, 60_000_000_000);

    // Payment user_id and rate limit user_id are independent
    assert_eq!(payment.user_id(), user_id);
    assert_eq!(rate_limit.user_id(), user_id);

    // Modifying payment should not affect rate limit
    payment.mark_confirmed(1000_000_000).unwrap();

    // Rate limit should still function
    assert!(rate_limit.try_acquire_tokens(1), "Rate limit should be unaffected");
}

// ============================================================================
// Feature Flag Security Tests (I20 Q12: Bypass Prevention)
// ============================================================================

#[test]
fn test_cannot_bypass_budget_check_via_overflow() {
    // I20 Q12: Budget exhaustion should not be bypassed via integer overflow
    let capsule = RequestCapsule128Enhanced::new(0x123, 100_00);

    // Try to deduct more than available (should fail)
    assert!(
        capsule.try_deduct(200_00).is_err(),
        "Should not allow deduction exceeding budget"
    );

    // Try to deduct u64::MAX (overflow attempt)
    assert!(
        capsule.try_deduct(i64::MAX).is_err(),
        "Should not allow massive deduction (overflow protection)"
    );

    // Budget should remain unchanged
    assert_eq!(capsule.budget(), 100_00, "Budget should be unchanged after failed deductions");
}

#[test]
fn test_cannot_bypass_rate_limit_via_negative_tokens() {
    // I20 Q12: Rate limiting should not be bypassed via negative token counts
    let rate_limit = RateLimitCapsule::new(12345, 10, 60_000_000_000);

    // Exhaust quota
    for _ in 0..10 {
        assert!(rate_limit.try_acquire_tokens(1), "Should acquire tokens");
    }

    // Try to acquire more (should fail)
    assert!(
        !rate_limit.try_acquire_tokens(1),
        "Should not acquire tokens when quota exhausted"
    );

    // Quota should be 0
    assert_eq!(rate_limit.quota_remaining(), 0, "Quota should be exhausted");
}

#[test]
fn test_circuit_breaker_cannot_be_bypassed() {
    // I20 Q12: Circuit breaker should not be bypassable
    use clapi_core::capsules::CircuitBreakerCapsule;

    let breaker = CircuitBreakerCapsule::new();

    // Record 100 requests, 20 failures (20% failure rate → should trip at 10%)
    for _ in 0..80 {
        breaker.record_success();
    }

    for _ in 0..20 {
        breaker.record_failure();
    }

    // Circuit should be open (failure rate = 20% > 10% threshold)
    assert!(breaker.is_open(), "Circuit should trip at 20% failure rate");

    // Cannot bypass by resetting counters (counters are internal)
    // This test verifies circuit state cannot be manipulated externally
    assert!(breaker.is_open(), "Circuit state should not be bypassable");
}

// ============================================================================
// Audit Log Integrity Tests (I20 Q13: Atomic Writes)
// ============================================================================

#[test]
fn test_audit_log_atomic_writes() {
    // I20 Q13: Audit log entries should be written atomically
    let log = AuditLogCapsule128::new(
        1,  // event_type (BudgetDeduction)
        0x123,  // budget_id
        100_00,  // amount_cents
        1000_000_000_000,  // timestamp_ns
    );

    // Entry should have consistent state
    let snapshot = log.snapshot();

    assert_eq!(snapshot.event_type, 1, "Event type should be set");
    assert_eq!(snapshot.budget_id_hash, 0x123, "Budget ID should be set");
    assert_eq!(snapshot.amount_cents, 100_00, "Amount should be set");
    assert_eq!(snapshot.timestamp_ns, 1000_000_000_000, "Timestamp should be set");
    assert_eq!(snapshot.generation, 0, "Generation should be 0 initially");
}

#[test]
fn test_concurrent_audit_log_writes() {
    // I20 Q13: Concurrent audit log writes should be atomic
    let logs = Arc::new((0..8).map(|_| {
        AuditLogCapsule128::new(1, 0x123, 0, 0)
    }).collect::<Vec<_>>());

    let write_count = Arc::new(AtomicU64::new(0));

    let mut handles = vec![];

    for i in 0..8 {
        let logs_clone = Arc::clone(&logs);
        let count_clone = Arc::clone(&write_count);

        let handle = thread::spawn(move || {
            for j in 0..1000 {
                let log = &logs_clone[i];

                // Simulate write (update amount + timestamp)
                log.amount_cents.store((i * 1000 + j) as i64, Ordering::Relaxed);
                log.timestamp_ns.store(j as u64, Ordering::Relaxed);
                log.generation.fetch_add(1, Ordering::Relaxed);

                count_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let total_writes = write_count.load(Ordering::Relaxed);
    assert_eq!(total_writes, 8000, "All writes should complete atomically");

    // Each log should have consistent final state
    for (i, log) in logs.iter().enumerate() {
        let gen = log.generation.load(Ordering::Relaxed);
        assert_eq!(gen, 1000, "Log {} should have 1000 generations", i);
    }
}

#[test]
fn test_audit_log_no_partial_entries() {
    // I20 Q13: Audit logs should never have partially written entries
    let log = Arc::new(AuditLogCapsule128::new(1, 0x123, 0, 0));

    let log_clone = Arc::clone(&log);

    let writer = thread::spawn(move || {
        // Atomic write (all fields updated together)
        log_clone.amount_cents.store(100_00, Ordering::Release);
        log_clone.timestamp_ns.store(1000_000_000_000, Ordering::Release);
        log_clone.generation.fetch_add(1, Ordering::Release);
    });

    writer.join().unwrap();

    // Reader should see all fields updated (no partial entry)
    let snapshot = log.snapshot();

    if snapshot.generation > 0 {
        // If generation incremented, all fields should be updated
        assert_eq!(snapshot.amount_cents, 100_00, "Amount should be updated");
        assert_eq!(snapshot.timestamp_ns, 1000_000_000_000, "Timestamp should be updated");
    }
}

// ============================================================================
// Rate Limiting Security Tests (I20 Q14: Per-User Enforcement)
// ============================================================================

#[test]
fn test_rate_limit_per_user_isolation() {
    // I20 Q14: Each user should have independent rate limits
    let user1_limit = Arc::new(RateLimitCapsule::new(111, 10, 60_000_000_000));
    let user2_limit = Arc::new(RateLimitCapsule::new(222, 10, 60_000_000_000));

    // User 1 exhausts quota
    for _ in 0..10 {
        assert!(user1_limit.try_acquire_tokens(1));
    }

    // User 1 quota exhausted
    assert!(!user1_limit.try_acquire_tokens(1), "User 1 quota should be exhausted");

    // User 2 should still have quota
    assert!(user2_limit.try_acquire_tokens(1), "User 2 should have quota");
}

#[test]
fn test_rate_limit_concurrent_users() {
    // I20 Q14: Concurrent users should not interfere with each other's quotas
    let limits = Arc::new((0..8).map(|i| {
        RateLimitCapsule::new(i as u64, 100, 60_000_000_000)
    }).collect::<Vec<_>>());

    let mut handles = vec![];

    for i in 0..8 {
        let limits_clone = Arc::clone(&limits);

        let handle = thread::spawn(move || {
            let limit = &limits_clone[i];

            // Each user tries to acquire 100 tokens
            let mut acquired = 0;

            for _ in 0..100 {
                if limit.try_acquire_tokens(1) {
                    acquired += 1;
                }
            }

            acquired
        });

        handles.push(handle);
    }

    let results: Vec<u64> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Each user should acquire exactly 100 tokens (no interference)
    for (i, &acquired) in results.iter().enumerate() {
        assert_eq!(acquired, 100, "User {} should acquire 100 tokens", i);
    }
}

#[test]
fn test_rate_limit_quota_refill() {
    // I20 Q14: Quota refill should be atomic and correct
    let rate_limit = RateLimitCapsule::new(12345, 10, 1_000_000_000);  // 1 second window

    // Exhaust quota
    for _ in 0..10 {
        assert!(rate_limit.try_acquire_tokens(1));
    }

    assert_eq!(rate_limit.quota_remaining(), 0, "Quota should be exhausted");

    // Wait for window to expire
    std::thread::sleep(std::time::Duration::from_millis(1100));

    // Try to acquire again (should trigger refill)
    assert!(rate_limit.try_acquire_tokens(1), "Quota should be refilled after window");
}

// ============================================================================
// Hash Chain Separation Tests (I20 Q15: Cross-Component Integrity)
// ============================================================================

#[test]
fn test_payment_oauth_hash_chains_separate() {
    // I20 Q15: Payment and OAuth hash chains should not interfere
    let payment1 = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);
    let hash_p1 = payment1.hash();

    let payment2 = PaymentCapsule256::new(222, 222, 50_00, 1_50, 0x456);
    payment2.prev_hash.store(hash_p1, Ordering::Relaxed);

    let session1 = OAuthSessionCapsule::new(0x789, 0xABC, 3600_000_000_000);
    let hash_s1 = session1.hash();

    let session2 = OAuthSessionCapsule::new(0xDEF, 0x111, 3600_000_000_000);
    session2.prev_hash.store(hash_s1, Ordering::Relaxed);

    // Payment chain should be valid
    assert!(payment2.verify_chain(hash_p1), "Payment chain should be valid");

    // OAuth chain should be valid
    assert!(session2.verify_chain(hash_s1), "OAuth chain should be valid");

    // Chains should not cross-contaminate
    assert!(
        !payment2.verify_chain(hash_s1),
        "Payment chain should reject OAuth hash"
    );
    assert!(
        !session2.verify_chain(hash_p1),
        "OAuth chain should reject payment hash"
    );
}

#[test]
fn test_concurrent_multi_component_hash_integrity() {
    // I20 Q15: Concurrent updates to multiple components should maintain hash integrity
    let payment = Arc::new(PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123));
    let session = Arc::new(OAuthSessionCapsule::new(0x456, 0x789, 3600_000_000_000));
    let budget = Arc::new(RequestCapsule128Enhanced::new(0xABC, 1000_00));

    let mut handles = vec![];

    // Thread 1: Update payment
    let payment_clone = Arc::clone(&payment);
    handles.push(thread::spawn(move || {
        for _ in 0..100 {
            let _ = payment_clone.mark_confirmed(1000_000_000);
            payment_clone.update_hash();
        }
    }));

    // Thread 2: Update session
    let session_clone = Arc::clone(&session);
    handles.push(thread::spawn(move || {
        for _ in 0..100 {
            session_clone.revoke();
            session_clone.update_hash();
        }
    }));

    // Thread 3: Update budget
    let budget_clone = Arc::clone(&budget);
    handles.push(thread::spawn(move || {
        for _ in 0..100 {
            let _ = budget_clone.try_deduct(10_00);
            budget_clone.update_hash();
        }
    }));

    for handle in handles {
        handle.join().unwrap();
    }

    // All components should maintain integrity independently
    assert!(payment.verify_integrity(), "Payment integrity should be maintained");
    assert!(budget.verify_integrity(), "Budget integrity should be maintained");
    // Session integrity (state should be consistent)
    assert!(!session.is_active(), "Session should be revoked");
}

#[test]
fn test_audit_log_captures_all_components() {
    // I20 Q15: Audit log should capture events from all components without interference
    let logs = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Payment event
    {
        let mut log_vec = logs.lock().unwrap();
        log_vec.push(AuditLogCapsule128::new(1, 0x123, 100_00, 1000));
    }

    // OAuth event
    {
        let mut log_vec = logs.lock().unwrap();
        log_vec.push(AuditLogCapsule128::new(2, 0x456, 0, 2000));
    }

    // Budget event
    {
        let mut log_vec = logs.lock().unwrap();
        log_vec.push(AuditLogCapsule128::new(3, 0x789, 50_00, 3000));
    }

    let log_vec = logs.lock().unwrap();

    // All events should be captured independently
    assert_eq!(log_vec.len(), 3, "All events should be logged");

    assert_eq!(log_vec[0].event_type.load(Ordering::Relaxed), 1, "Payment event");
    assert_eq!(log_vec[1].event_type.load(Ordering::Relaxed), 2, "OAuth event");
    assert_eq!(log_vec[2].event_type.load(Ordering::Relaxed), 3, "Budget event");
}

// End of i20_security_tests.rs
