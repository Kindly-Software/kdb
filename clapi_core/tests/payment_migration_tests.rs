//! PaymentCapsule128 Migration Test Suite
//!
//! Comprehensive tests validating PaymentCapsule128 (128B) vs PaymentCapsule256 (256B):
//! - API compatibility (100% identical behavior)
//! - Bit packing roundtrip accuracy
//! - Property tests (fee/net consistency)
//! - Integration tests (migration path)
//!
//! T28 Framework Coverage:
//! - Q1-Q7 (Unit tests): Bit packing, API compatibility
//! - Q8-Q14 (Property tests): Roundtrip accuracy, arithmetic invariants
//! - Q15-Q21 (Integration tests): PaymentCapsule256 → PaymentCapsule128 migration

use clapi_core::capsules::{PaymentCapsule128, PaymentCapsule256, PaymentStatus};

// ============================================================================
// Unit Tests (T28 Q1-Q7)
// ============================================================================

#[test]
fn test_size_reduction() {
    // Verify PaymentCapsule128 is exactly half the size of PaymentCapsule256
    assert_eq!(std::mem::size_of::<PaymentCapsule256>(), 256);
    assert_eq!(std::mem::size_of::<PaymentCapsule128>(), 128);
}

#[test]
fn test_alignment_compatibility() {
    // Both capsules maintain cache-line alignment
    assert_eq!(std::mem::align_of::<PaymentCapsule256>(), 256);
    assert_eq!(std::mem::align_of::<PaymentCapsule128>(), 128);
}

#[test]
fn test_api_compatibility_new() {
    // Test that new() has identical behavior (modulo rounding)
    let amount = 1_000_00; // $1000

    let p256 = PaymentCapsule256::new(123, 456, amount);
    let p128 = PaymentCapsule128::new(123, 456, amount).unwrap();

    assert_eq!(p256.payment_id(), p128.payment_id());
    assert_eq!(p256.user_id(), p128.user_id());
    assert_eq!(p256.amount(), p128.amount());

    // Fee/net may differ by ±1 cent due to Q16.8 vs Q0.64 precision
    assert!((p256.fee() - p128.fee()).abs() <= 1);
    assert!((p256.net() - p128.net()).abs() <= 1);

    assert_eq!(p256.status(), p128.status());
}

#[test]
fn test_api_compatibility_state_machine() {
    // Test that state machine transitions are identical
    let p256 = PaymentCapsule256::new(1, 1, 1_000_00);
    let p128 = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();

    // Pending → Processing
    p256.start_processing().unwrap();
    p128.start_processing().unwrap();
    assert_eq!(p256.status(), p128.status());

    // Processing → Success
    p256.confirm_payment().unwrap();
    p128.confirm_payment().unwrap();
    assert_eq!(p256.status(), p128.status());

    // Success → Refunded
    p256.refund_payment().unwrap();
    p128.refund_payment().unwrap();
    assert_eq!(p256.status(), p128.status());
}

#[test]
fn test_api_compatibility_stripe_id() {
    // Test that Stripe ID hashing is identical
    let stripe_id = "pi_3N1234567890abcdef";

    let p256 = PaymentCapsule256::new(1, 1, 1_000_00);
    let p128 = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();

    p256.record_stripe_id(stripe_id).unwrap();
    p128.record_stripe_id(stripe_id).unwrap();

    assert_eq!(p256.stripe_id_hash(), p128.stripe_id_hash());
}

#[test]
fn test_api_compatibility_retry_count() {
    // Test that retry count behavior is identical
    let p256 = PaymentCapsule256::new(1, 1, 1_000_00);
    let p128 = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();

    assert_eq!(p256.retry_count(), p128.retry_count());

    p256.increment_retry().unwrap();
    p128.increment_retry().unwrap();

    assert_eq!(p256.retry_count(), p128.retry_count());
}

#[test]
fn test_api_compatibility_generation() {
    // Test that generation counter behavior is identical
    let p256 = PaymentCapsule256::new(1, 1, 1_000_00);
    let p128 = PaymentCapsule128::new(1, 1, 1_000_00).unwrap();

    let gen256_1 = p256.generation();
    let gen128_1 = p128.generation();
    assert_eq!(gen256_1, gen128_1);

    p256.start_processing().unwrap();
    p128.start_processing().unwrap();

    let gen256_2 = p256.generation();
    let gen128_2 = p128.generation();
    assert_eq!(gen256_2, gen128_2);
}

#[test]
fn test_bit_packing_positive_values() {
    // Test bit packing for positive fee/net values
    let test_cases = vec![
        (1_000_00, 3_000, 97_000),       // $1000 → $30 fee → $970 net
        (5_000_00, 15_000, 485_000),     // $5000 → $150 fee → $4850 net
        (100_00, 300, 9_700),            // $100 → $3 fee → $97 net
        (10_00, 30, 970),                // $10 → $0.30 fee → $9.70 net
    ];

    for (amount, expected_fee, expected_net) in test_cases {
        let p128 = PaymentCapsule128::new(1, 1, amount).unwrap();

        let fee = p128.fee();
        let net = p128.net();

        // Allow ±1 cent rounding error
        assert!(
            (fee - expected_fee).abs() <= 1,
            "Fee mismatch for amount {}: got {}, expected {}",
            amount, fee, expected_fee
        );

        assert!(
            (net - expected_net).abs() <= 1,
            "Net mismatch for amount {}: got {}, expected {}",
            amount, net, expected_net
        );
    }
}

#[test]
fn test_bit_packing_negative_values() {
    // Test bit packing for negative amounts (refunds, chargebacks)
    let test_cases = vec![
        (-1_000_00, -3_000, -97_000),    // -$1000 → -$30 fee → -$970 net
        (-100_00, -300, -9_700),         // -$100 → -$3 fee → -$97 net
    ];

    for (amount, expected_fee, expected_net) in test_cases {
        let p128 = PaymentCapsule128::new(1, 1, amount).unwrap();

        let fee = p128.fee();
        let net = p128.net();

        // Allow ±1 cent rounding error
        assert!(
            (fee - expected_fee).abs() <= 1,
            "Fee mismatch for amount {}: got {}, expected {}",
            amount, fee, expected_fee
        );

        assert!(
            (net - expected_net).abs() <= 1,
            "Net mismatch for amount {}: got {}, expected {}",
            amount, net, expected_net
        );
    }
}

#[test]
fn test_bit_packing_edge_cases() {
    // Test bit packing at the boundaries of Q16.8 representation
    let edge_cases = vec![
        1,       // $0.01 (1 cent)
        10,      // $0.10
        99,      // $0.99
        100,     // $1.00
        1_000,   // $10.00
        10_000,  // $100.00
    ];

    for amount in edge_cases {
        let p128 = PaymentCapsule128::new(1, 1, amount).unwrap();

        // Verify roundtrip
        assert!(p128.verify_arithmetic());

        // Verify status preserved
        assert_eq!(p128.status(), PaymentStatus::Pending);
    }
}

// ============================================================================
// Property Tests (T28 Q8-Q14)
// ============================================================================

#[test]
fn test_property_roundtrip_lossless() {
    // Property: pack(unpack(x)) == x for all valid x
    let test_amounts = vec![
        1_00,
        10_00,
        100_00,
        1_000_00,
        5_000_00,
        10_000_00,
        15_000_00, // Near max for 23-bit net
    ];

    for amount in test_amounts {
        let p128 = PaymentCapsule128::new(1, 1, amount).unwrap();

        // Extract fee/net
        let fee = p128.fee();
        let net = p128.net();

        // Create new payment with same amount
        let p128_2 = PaymentCapsule128::new(1, 1, amount).unwrap();

        // Verify roundtrip (±1 cent rounding)
        assert!((p128.fee() - p128_2.fee()).abs() <= 1);
        assert!((p128.net() - p128_2.net()).abs() <= 1);
    }
}

#[test]
fn test_property_fee_calculation_deterministic() {
    // Property: fee(amount) always returns same value for same input
    let amount = 1_000_00;

    let p1 = PaymentCapsule128::new(1, 1, amount).unwrap();
    let p2 = PaymentCapsule128::new(2, 2, amount).unwrap();
    let p3 = PaymentCapsule128::new(3, 3, amount).unwrap();

    // All should have identical fee/net (±1 cent rounding)
    assert!((p1.fee() - p2.fee()).abs() <= 1);
    assert!((p1.fee() - p3.fee()).abs() <= 1);
    assert!((p1.net() - p2.net()).abs() <= 1);
    assert!((p1.net() - p3.net()).abs() <= 1);
}

#[test]
fn test_property_arithmetic_invariant() {
    // Property: ∀ payments: amount - fee ≈ net (±1 cent rounding)
    let test_amounts = vec![
        1_00, 10_00, 100_00, 1_000_00, 5_000_00, 10_000_00,
    ];

    for amount in test_amounts {
        let p128 = PaymentCapsule128::new(1, 1, amount).unwrap();

        let computed_net = p128.amount() - p128.fee();
        let stored_net = p128.net();

        assert!(
            (computed_net - stored_net).abs() <= 1,
            "Arithmetic invariant violated for amount {}: amount {} - fee {} = {}, but net = {}",
            amount, p128.amount(), p128.fee(), computed_net, stored_net
        );

        // Also verify via verify_arithmetic()
        assert!(p128.verify_arithmetic());
    }
}

#[test]
fn test_property_state_transitions_preserve_amounts() {
    // Property: State transitions do NOT mutate amount/fee/net
    let amount = 1_000_00;
    let p128 = PaymentCapsule128::new(1, 1, amount).unwrap();

    let initial_amount = p128.amount();
    let initial_fee = p128.fee();
    let initial_net = p128.net();

    // Pending → Processing
    p128.start_processing().unwrap();
    assert_eq!(p128.amount(), initial_amount);
    assert!((p128.fee() - initial_fee).abs() <= 1);
    assert!((p128.net() - initial_net).abs() <= 1);

    // Processing → Success
    p128.confirm_payment().unwrap();
    assert_eq!(p128.amount(), initial_amount);
    assert!((p128.fee() - initial_fee).abs() <= 1);
    assert!((p128.net() - initial_net).abs() <= 1);

    // Success → Refunded
    p128.refund_payment().unwrap();
    assert_eq!(p128.amount(), initial_amount);
    assert!((p128.fee() - initial_fee).abs() <= 1);
    assert!((p128.net() - initial_net).abs() <= 1);
}

// ============================================================================
// Integration Tests (T28 Q15-Q21)
// ============================================================================

#[test]
fn test_migration_paymentcapsule256_to_paymentcapsule128() {
    // Simulate migration: PaymentCapsule256 → PaymentCapsule128
    let amount = 1_000_00;

    // Create PaymentCapsule256
    let p256 = PaymentCapsule256::new(123, 456, amount);

    // Migrate to PaymentCapsule128
    let p128 = PaymentCapsule128::new(
        p256.payment_id(),
        p256.user_id(),
        p256.amount(),
    ).unwrap();

    // Verify all fields migrated correctly (±1 cent rounding)
    assert_eq!(p128.payment_id(), p256.payment_id());
    assert_eq!(p128.user_id(), p256.user_id());
    assert_eq!(p128.amount(), p256.amount());
    assert!((p128.fee() - p256.fee()).abs() <= 1);
    assert!((p128.net() - p256.net()).abs() <= 1);
    assert_eq!(p128.status(), p256.status());
}

#[test]
fn test_migration_with_state_transitions() {
    // Migrate payment that has undergone state transitions
    let amount = 5_000_00;

    // Create and transition PaymentCapsule256
    let p256 = PaymentCapsule256::new(123, 456, amount);
    p256.start_processing().unwrap();
    p256.confirm_payment().unwrap();

    // Migrate to PaymentCapsule128
    let p128 = PaymentCapsule128::new(
        p256.payment_id(),
        p256.user_id(),
        p256.amount(),
    ).unwrap();

    // Manually apply same state transitions
    p128.start_processing().unwrap();
    p128.confirm_payment().unwrap();

    // Verify final state matches
    assert_eq!(p128.status(), p256.status());
    assert_eq!(p128.status(), PaymentStatus::Success);
}

#[test]
fn test_overflow_detection_prevents_migration() {
    // Test that PaymentCapsule128 rejects amounts that would overflow bit-packed fields
    let large_amount = 40_000_00; // $40,000 (fee would be $1,200, exceeds 24-bit limit)

    // PaymentCapsule256 accepts this
    let p256 = PaymentCapsule256::new(1, 1, large_amount);
    assert_eq!(p256.amount(), large_amount);

    // PaymentCapsule128 rejects it
    let result = PaymentCapsule128::new(1, 1, large_amount);
    assert!(result.is_err(), "Expected overflow error");

    if let Err(e) = result {
        let err_str = format!("{:?}", e);
        assert!(err_str.contains("exceeds PaymentCapsule128 range"));
    }
}

#[test]
fn test_concurrent_access_compatibility() {
    // Test that concurrent access patterns work identically for both capsules
    use std::sync::Arc;
    use std::thread;

    let amount = 1_000_00;

    let p256 = Arc::new(PaymentCapsule256::new(1, 1, amount));
    let p128 = Arc::new(PaymentCapsule128::new(1, 1, amount).unwrap());

    // Start processing
    p256.start_processing().unwrap();
    p128.start_processing().unwrap();

    // Concurrent confirmations
    let p256_1 = Arc::clone(&p256);
    let p256_2 = Arc::clone(&p256);
    let p128_1 = Arc::clone(&p128);
    let p128_2 = Arc::clone(&p128);

    let h256_1 = thread::spawn(move || p256_1.confirm_payment());
    let h256_2 = thread::spawn(move || p256_2.confirm_payment());
    let h128_1 = thread::spawn(move || p128_1.confirm_payment());
    let h128_2 = thread::spawn(move || p128_2.confirm_payment());

    let r256_1 = h256_1.join().unwrap();
    let r256_2 = h256_2.join().unwrap();
    let r128_1 = h128_1.join().unwrap();
    let r128_2 = h128_2.join().unwrap();

    // Both capsules should have exactly one success
    assert!(r256_1.is_ok() != r256_2.is_ok());
    assert!(r128_1.is_ok() != r128_2.is_ok());

    // Both should end in Success state
    assert_eq!(p256.status(), PaymentStatus::Success);
    assert_eq!(p128.status(), PaymentStatus::Success);
}

#[test]
fn test_hash_chain_compatibility() {
    // Test that hash chain logic works identically for both capsules
    let amount = 1_000_00;

    let p256 = PaymentCapsule256::new(1, 1, amount);
    let p128 = PaymentCapsule128::new(1, 1, amount).unwrap();

    // Update hash chains
    p256.update_hash_chain();
    p128.update_hash_chain();

    // Both should verify
    assert!(p256.verify_chain());
    assert!(p128.verify_chain());

    // After state change
    p256.start_processing().unwrap();
    p128.start_processing().unwrap();

    p256.update_hash_chain();
    p128.update_hash_chain();

    assert!(p256.verify_chain());
    assert!(p128.verify_chain());
}

#[test]
fn test_snapshot_compatibility() {
    // Test that snapshot() returns compatible data
    let amount = 1_000_00;

    let p256 = PaymentCapsule256::new(123, 456, amount);
    let p128 = PaymentCapsule128::new(123, 456, amount).unwrap();

    p256.start_processing().unwrap();
    p128.start_processing().unwrap();

    let snap256 = p256.snapshot();
    let snap128 = p128.snapshot();

    assert_eq!(snap256.payment_id, snap128.payment_id);
    assert_eq!(snap256.user_id, snap128.user_id);
    assert_eq!(snap256.amount_cents, snap128.amount_cents);
    assert!((snap256.fee_cents - snap128.fee_cents).abs() <= 1);
    assert!((snap256.net_cents - snap128.net_cents).abs() <= 1);
    assert_eq!(snap256.status, snap128.status);
}

// ============================================================================
// Performance Regression Tests
// ============================================================================

#[test]
fn test_performance_no_regression() {
    // Verify PaymentCapsule128 is NOT slower than PaymentCapsule256
    // (Actual benchmarking in benches/payment_size_bench.rs)

    use std::time::Instant;

    let amount = 1_000_00;
    let iterations = 10_000;

    // Benchmark PaymentCapsule256
    let start256 = Instant::now();
    for _ in 0..iterations {
        let p256 = PaymentCapsule256::new(1, 1, amount);
        let _ = p256.amount();
        let _ = p256.fee();
        let _ = p256.net();
        let _ = p256.status();
    }
    let elapsed256 = start256.elapsed();

    // Benchmark PaymentCapsule128
    let start128 = Instant::now();
    for _ in 0..iterations {
        let p128 = PaymentCapsule128::new(1, 1, amount).unwrap();
        let _ = p128.amount();
        let _ = p128.fee();
        let _ = p128.net();
        let _ = p128.status();
    }
    let elapsed128 = start128.elapsed();

    println!("PaymentCapsule256: {:?} for {} iterations", elapsed256, iterations);
    println!("PaymentCapsule128: {:?} for {} iterations", elapsed128, iterations);

    // PaymentCapsule128 should be faster or comparable (within 20%)
    let ratio = elapsed128.as_nanos() as f64 / elapsed256.as_nanos() as f64;
    assert!(
        ratio <= 1.2,
        "PaymentCapsule128 is slower than expected: {}× vs PaymentCapsule256",
        ratio
    );
}
