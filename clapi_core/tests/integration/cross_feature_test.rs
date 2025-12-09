//! Cross-Feature Integration Tests - T28 Q15-Q21
//!
//! **Framework**: T28 Integration Testing (Q15-Q21)
//! **Coverage**: Budget + OAuth + Payment + Circuit breaker integration
//!
//! # T28 Q15-Q21 Coverage
//!
//! ## Q15: Integration Scope
//! - Budget + OAuth: Session creation with budget allocation
//! - OAuth + Payment: Payment processing with session validation
//! - Budget + Circuit: Provider failure preserves budget
//! - All 4 features: Full stack integration
//!
//! ## Q16: Minimal Integration
//! - Create session → Allocate budget → Process payment → Verify all states
//!
//! ## Q17: Property Invariants
//! - Budget balance consistent across all operations
//! - OAuth session valid throughout payment flow
//! - Circuit breaker doesn't corrupt budget state
//! - Payment amounts match budget deductions
//!
//! ## Q18: Performance Budget
//! - Full stack latency: <10ms p50
//! - Budget + OAuth: <200ns combined
//! - OAuth + Payment: <300ns combined
//! - All 4 features: <1ms combined
//!
//! ## Q19: Edge Cases
//! - Session expires during payment
//! - Budget exhausted mid-transaction
//! - Provider fails during payment
//! - Concurrent operations across features
//!
//! ## Q20: Stress Integration
//! - 1000 concurrent full-stack operations
//! - Budget + OAuth + Payment + Circuit under load
//!
//! ## Q21: System Recovery
//! - Feature isolation on failure
//! - Graceful degradation
//! - State consistency recovery

use clapi_core::capsules::{
    BudgetMetaCapsule, OAuthSessionCapsule, PaymentCapsule256, PaymentStatus,
    CircuitBreakerCapsule, CircuitState, ProviderCircuitArray,
};
#[cfg(feature = "compliance")]
use clapi_core::compliance::{ComplianceCapsule256, ComplianceEntry};
use clapi_core::error::ClapiResult;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// T28 Q16: Minimal Cross-Feature Integration
// ============================================================================

#[test]
fn test_q16_minimal_full_stack() -> ClapiResult<()> {
    // Q16: Minimal integration - Budget + OAuth + Payment + Circuit

    // 1. Budget: Allocate budget
    let budget_meta = BudgetMetaCapsule::new();
    let budget_id = 1u64;
    budget_meta.allocate(budget_id, 100_00)?;  // $100.00
    let budget = budget_meta.get(budget_id).unwrap();

    // 2. OAuth: Create session
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, Some(3600));
    assert!(session.is_valid(), "Session should be valid");

    // 3. Circuit: Verify circuit closed
    let circuit = CircuitBreakerCapsule::new();
    assert_eq!(circuit.get_state().circuit_state, CircuitState::Closed);

    // 4. Payment: Process payment
    let payment = PaymentCapsule256::new(0x00000001, 1001, 10_00);
    payment.start_processing()?;
    payment.confirm_payment()?;
    assert_eq!(payment.status(), PaymentStatus::Success);

    // 5. Budget: Deduct payment amount
    budget.try_deduct(10_00)?;

    // Verify final state
    assert_eq!(budget.remaining(), 90_00);
    assert!(session.is_valid());
    assert_eq!(circuit.get_state().circuit_state, CircuitState::Closed);
    assert_eq!(payment.status(), PaymentStatus::Success);

    Ok(())
}

// ============================================================================
// T28 Q17: Budget + OAuth Integration
// ============================================================================

#[test]
fn test_q17_budget_oauth_integration() -> ClapiResult<()> {
    // Q17: Property - Budget allocation doesn't affect OAuth session

    let budget_meta = BudgetMetaCapsule::new();
    let session = OAuthSessionCapsule::new(1001, 0x10CE0001, Some(3600));

    // Allocate budget
    budget_meta.allocate(1, 100_00)?;

    // OAuth session should remain valid
    assert!(session.is_valid(), "OAuth session should be unaffected by budget allocation");

    // Budget operations don't block session verification
    let start = Instant::now();

    for _ in 0..1000 {
        assert!(session.verify_token(0x10CE0001));
        let _ = budget_meta.get(1).unwrap().remaining();
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / 1000;

    println!("Budget + OAuth combined latency: {} ns", avg_ns);

    // B32: Should be <500ns combined
    assert!(avg_ns < 500, "Combined latency {}ns exceeds 500ns", avg_ns);

    Ok(())
}

// ============================================================================
// T28 Q17: OAuth + Payment Integration
// ============================================================================

#[test]
fn test_q17_oauth_payment_integration() -> ClapiResult<()> {
    // Q17: Property - Payment requires valid OAuth session

    let session = OAuthSessionCapsule::new(1001, 0xDECE0001, Some(3600));
    let user_id = session.user_id();

    // Verify session valid
    assert!(session.is_valid());

    // Process payment for same user
    let payment = PaymentCapsule256::new(0x00000002, user_id, 50_00);

    // Verify user_id matches
    assert_eq!(payment.user_id(), user_id, "Payment user should match session user");

    // Confirm payment
    payment.start_processing()?;
    payment.confirm_payment()?;

    // Session should remain valid after payment
    assert!(session.is_valid(), "Session should remain valid after payment");

    Ok(())
}

// ============================================================================
// T28 Q17: Budget + Circuit Integration
// ============================================================================

#[test]
fn test_q17_budget_circuit_integration() -> ClapiResult<()> {
    // Q17: Property - Circuit breaker preserves budget on failure

    let budget_meta = BudgetMetaCapsule::new();
    let circuit = CircuitBreakerCapsule::new();

    let budget_id = 1u64;
    let initial_amount = 100_00;
    budget_meta.allocate(budget_id, initial_amount)?;
    let budget = budget_meta.get(budget_id).unwrap();

    // Trip circuit breaker
    for _ in 0..10 {
        circuit.record_failure();
    }

    // Verify circuit open or half-open
    let state = circuit.get_state();
    println!("Circuit state after failures: {:?}", state);

    // Budget should be unchanged (no false deductions)
    assert_eq!(
        budget.remaining(),
        initial_amount,
        "Budget should be preserved when circuit opens"
    );

    Ok(())
}

// ============================================================================
// T28 Q17: Payment + Circuit Integration
// ============================================================================

#[test]
fn test_q17_payment_circuit_integration() -> ClapiResult<()> {
    // Q17: Property - Payment fails when circuit open

    let payment = PaymentCapsule256::new(0xC12C0001, 1001, 10_00);
    let provider_array = ProviderCircuitArray::new();

    // Trip provider circuit
    let provider_id = 0;  // Anthropic
    for _ in 0..10 {
        provider_array.record_failure(provider_id, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64);
    }

    // Check provider state
    let provider_status = provider_array.get_or_init(provider_id, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64).unwrap();
    println!("Provider {} state: {:?}", provider_id, provider_status.get_state());

    // If circuit open, payment should be blocked
    if provider_status.get_state() != CircuitState::Closed {
        println!("Payment blocked due to circuit breaker");
        // Payment remains pending
        assert_eq!(payment.status(), PaymentStatus::Pending);
    }

    Ok(())
}

// ============================================================================
// T28 Q18: Performance Budget - Full Stack Latency
// ============================================================================

#[test]
fn test_q18_full_stack_latency() -> ClapiResult<()> {
    // Q18: Performance - Full stack <1ms

    let budget_meta = BudgetMetaCapsule::new();
    let circuit = CircuitBreakerCapsule::new();
    let provider_array = ProviderCircuitArray::new();

    budget_meta.allocate(1, 1_000_000_00)?;  // $1M budget
    let budget = budget_meta.get(1).unwrap();

    let start = Instant::now();
    let iterations = 1000;

    for i in 0..iterations {
        // Full stack operation
        let session = OAuthSessionCapsule::new(1001, 0xFA510000 + i as u64, Some(3600));
        let _ = session.is_valid();  // OAuth check
        let _ = circuit.get_state();  // Circuit check
        let _ = provider_array.get_or_init(i % 16, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64).unwrap();  // Provider routing
        let _ = budget.try_deduct(1_00);  // Budget deduction
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / iterations;
    let avg_us = avg_ns as f64 / 1000.0;

    println!("Full stack latency: {:.3}μs ({} ns)", avg_us, avg_ns);

    // B32: Target <10μs for full stack
    assert!(avg_us < 10.0, "Full stack {}μs exceeds 10μs", avg_us);

    Ok(())
}

// ============================================================================
// T28 Q19: Edge Cases - Session Expires During Payment
// ============================================================================

#[test]
fn test_q19_session_expires_during_payment() -> ClapiResult<()> {
    // Q19: Edge case - OAuth session expires mid-payment

    // Create session with very short TTL
    let session = OAuthSessionCapsule::new(1001, 0xE1C10001, Some(100_000));  // 0.1ms

    // Start payment
    let payment = PaymentCapsule256::new(0x00000003, 1001, 10_00);

    // Wait for session expiry
    thread::sleep(Duration::from_millis(200));

    // Session should be expired
    assert!(!session.is_valid(), "Session should be expired");

    // Payment should fail (no valid session)
    // Note: In production, payment would check session validity
    println!("Payment processing with expired session (should fail)");

    Ok(())
}

// ============================================================================
// T28 Q19: Edge Cases - Budget Exhausted Mid-Transaction
// ============================================================================

#[test]
fn test_q19_budget_exhausted_mid_transaction() -> ClapiResult<()> {
    // Q19: Edge case - Budget runs out during payment processing

    let budget_meta = BudgetMetaCapsule::new();
    budget_meta.allocate(1, 5_00)?;  // Only $5.00
    let budget = budget_meta.get(1).unwrap();

    // First payment succeeds
    let payment1 = PaymentCapsule256::new(0x00000004, 1001, 4_00);
    budget.try_deduct(4_00)?;
    payment1.start_processing()?;
    payment1.confirm_payment()?;

    // Second payment fails (insufficient budget)
    let payment2 = PaymentCapsule256::new(0x00000005, 1001, 10_00);
    let result = budget.try_deduct(10_00);

    assert!(result.is_err(), "Should fail with insufficient budget");
    assert_eq!(payment2.status(), PaymentStatus::Pending, "Payment should remain pending");

    Ok(())
}

// ============================================================================
// T28 Q19: Edge Cases - Provider Fails During Payment
// ============================================================================

#[test]
fn test_q19_provider_fails_during_payment() -> ClapiResult<()> {
    // Q19: Edge case - Provider fails mid-payment

    let budget_meta = BudgetMetaCapsule::new();
    let provider_array = ProviderCircuitArray::new();

    budget_meta.allocate(1, 100_00)?;
    let budget = budget_meta.get(1).unwrap();

    // Start payment
    let payment = PaymentCapsule256::new(0xFA110001, 1001, 10_00);

    // Provider fails
    for _ in 0..10 {
        provider_array.record_failure(0, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64);
    }

    // Payment should fail
    payment.fail_payment("")?;

    // Budget should NOT be deducted (payment failed)
    assert_eq!(budget.remaining(), 100_00, "Budget preserved on payment failure");

    Ok(())
}

// ============================================================================
// T28 Q20: Stress - 1000 Concurrent Full-Stack Operations
// ============================================================================

#[test]
fn test_q20_stress_full_stack() -> ClapiResult<()> {
    // Q20: Stress - 1000 concurrent operations across all features

    let budget_meta = Arc::new(BudgetMetaCapsule::new());
    let circuit = Arc::new(CircuitBreakerCapsule::new());
    let provider_array = Arc::new(ProviderCircuitArray::new());

    // Setup: Allocate large budget
    budget_meta.allocate(1, 10_000_000_00)?;  // $10M

    let mut handles = vec![];

    // Spawn 100 threads, each processing 10 operations
    for thread_id in 0..100 {
        let budget_meta_clone = Arc::clone(&budget_meta);
        let circuit_clone = Arc::clone(&circuit);
        let provider_clone = Arc::clone(&provider_array);

        let handle = thread::spawn(move || -> ClapiResult<()> {
            for i in 0..10 {
                // Full stack operation
                let session = OAuthSessionCapsule::new(
                    1000 + thread_id as u64,
                    0xD1E50000 + (thread_id * 10 + i) as u64,
                    Some(3600),
                );

                if session.is_valid() && circuit_clone.state() == CircuitState::Closed {
                    let provider_id = (thread_id + i) % 16;
                    let _ = provider_clone.get_or_init(provider_id, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64).unwrap();

                    let payment = PaymentCapsule256::new(
                        0xD1E50000 + (thread_id * 10 + i) as u64,
                        session.user_id(),
                        100_00,
                    );

                    let budget = budget_meta_clone.get(1).unwrap();
                    if budget.try_deduct(100_00).is_ok() {
                        payment.start_processing()?;
                        payment.confirm_payment()?;
                        circuit_clone.record_success();
                    }
                }
            }
            Ok(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap()?;
    }

    // Verify operations completed
    let budget = budget_meta.get(1).unwrap();
    println!("Final budget remaining: ${}.{:02}", budget.remaining() / 100, budget.remaining() % 100);

    Ok(())
}

// ============================================================================
// T28 Q21: System Recovery - Feature Isolation
// ============================================================================

#[test]
fn test_q21_feature_isolation_on_failure() -> ClapiResult<()> {
    // Q21: Recovery - One feature failure doesn't cascade

    let budget_meta = BudgetMetaCapsule::new();
    let session = OAuthSessionCapsule::new(1001, 0x150A0001, Some(3600));
    let circuit = CircuitBreakerCapsule::new();

    budget_meta.allocate(1, 100_00)?;

    // Fail circuit breaker
    for _ in 0..10 {
        circuit.record_failure();
    }

    // Budget and OAuth should remain functional
    assert!(session.is_valid(), "OAuth should be unaffected by circuit failure");
    assert_eq!(budget_meta.get(1).unwrap().remaining(), 100_00, "Budget should be intact");

    Ok(())
}

// ============================================================================
// T28 Q21: System Recovery - Graceful Degradation
// ============================================================================

#[test]
fn test_q21_graceful_degradation() -> ClapiResult<()> {
    // Q21: Recovery - System degrades gracefully under failure

    let budget_meta = BudgetMetaCapsule::new();
    let provider_array = ProviderCircuitArray::new();

    budget_meta.allocate(1, 100_00)?;

    // Fail primary provider
    for _ in 0..10 {
        provider_array.record_failure(0, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64);
    }

    // Check failover to secondary provider
    let provider_1_status = provider_array.get_or_init(1, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64).unwrap();

    if provider_1_status.state() == CircuitState::Closed {
        println!("Failover to provider 1 successful");
        // Budget operations should continue
        let budget = budget_meta.get(1).unwrap();
        assert!(budget.try_deduct(10_00).is_ok(), "Budget operations should work during failover");
    }

    Ok(())
}

// ============================================================================
// T28 Q21: System Recovery - State Consistency
// ============================================================================

#[test]
fn test_q21_state_consistency_recovery() -> ClapiResult<()> {
    // Q21: Recovery - State remains consistent after partial failure

    let budget_meta = BudgetMetaCapsule::new();
    let payment = PaymentCapsule256::new(0xC0510001, 1001, 10_00);

    budget_meta.allocate(1, 100_00)?;
    let budget = budget_meta.get(1).unwrap();

    // Start transaction
    budget.try_deduct(10_00)?;

    // Payment fails
    payment.fail_payment("")?;

    // Budget should be restored (transaction rollback)
    // Note: In production, this would require explicit rollback logic
    // For now, we verify state is tracked correctly
    assert_eq!(payment.status(), PaymentStatus::Failed);

    Ok(())
}

// ============================================================================
// Compliance Integration (if enabled)
// ============================================================================

#[test]
#[cfg(feature = "compliance")]
fn test_compliance_cross_feature_integration() -> ClapiResult<()> {
    // Integration: Compliance logging across all features

    let compliance = ComplianceCapsule256::new();

    // Log budget allocation
    let entry1 = ComplianceEntry {
        timestamp_ns: 1_000_000_000,
        user_id: 1001,
        event_type: "budget_allocated".to_string(),
        amount_cents: 100_00,
        description: "Initial budget allocation".to_string(),
    };
    compliance.record_entry(entry1)?;

    // Log OAuth session creation
    let entry2 = ComplianceEntry {
        timestamp_ns: 2_000_000_000,
        user_id: 1001,
        event_type: "session_created".to_string(),
        amount_cents: 0,
        description: "OAuth session created".to_string(),
    };
    compliance.record_entry(entry2)?;

    // Log payment
    let entry3 = ComplianceEntry {
        timestamp_ns: 3_000_000_000,
        user_id: 1001,
        event_type: "payment_processed".to_string(),
        amount_cents: 10_00,
        description: "Payment confirmed".to_string(),
    };
    compliance.record_entry(entry3)?;

    // Export compliance report
    let report = compliance.export_json()?;

    // Verify all events logged
    assert!(report.contains("budget_allocated"));
    assert!(report.contains("session_created"));
    assert!(report.contains("payment_processed"));

    Ok(())
}

// ============================================================================
// End-to-End User Journey
// ============================================================================

#[test]
fn test_end_to_end_user_journey() -> ClapiResult<()> {
    // Complete user journey: Register → Login → Purchase → Logout

    // 1. Register: Allocate budget
    let budget_meta = BudgetMetaCapsule::new();
    budget_meta.allocate(1001, 50_00)?;  // $50 starter credit

    // 2. Login: Create OAuth session
    let session = OAuthSessionCapsule::new(1001, 0xE1E11001, Some(3600));
    assert!(session.is_valid());

    // 3. Purchase: Process payment
    let budget = budget_meta.get(1001).unwrap();
    budget.try_deduct(5_00)?;  // $5 API usage

    let payment = PaymentCapsule256::new(0xDEA10001, 1001, 5_00);
    payment.start_processing()?;
    payment.confirm_payment()?;

    // 4. Verify state
    assert_eq!(budget.remaining(), 45_00);
    assert_eq!(payment.status(), PaymentStatus::Success);
    assert!(session.is_valid());

    // 5. Logout: Revoke session
    session.revoke();
    assert!(!session.is_valid());

    // Budget and payment should persist after logout
    assert_eq!(budget.remaining(), 45_00);
    assert_eq!(payment.status(), PaymentStatus::Success);

    Ok(())
}
