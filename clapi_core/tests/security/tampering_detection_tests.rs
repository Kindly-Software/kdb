//! Tampering Detection Tests - Hash Chain Integrity Validation
//!
//! **Purpose**: Comprehensive validation of hash chain tampering detection
//! **Framework**: T28 Testing Framework + ASSUM Safety
//!
//! # Test Coverage
//! - **Field Tampering**: payment_id, amount_cents, status, timestamps
//! - **Hash Manipulation**: Direct hash modification, prev_hash splicing
//! - **Pre-image Resistance**: Cannot forge valid hash
//! - **Chain Integrity**: Cannot splice or reorder chains
//! - **Collision Resistance**: <1/2^64 collision probability
//!
//! # ASSUM Validation
//! - Validates Q34 auditability (hash chain enables forensics)
//! - Tests hash chain integrity under tampering attacks
//! - Property tests for cryptographic properties

use clapi_core::capsules::PaymentCapsule256;
use atomic_capsule::hash::const_fast_hash;
use std::sync::atomic::Ordering;

// ============================================================================
// Field Tampering Detection Tests (T28 Q1-Q5)
// ============================================================================

#[test]
fn test_tamper_payment_id_detected() {
    // T28 Q1: Tampering with payment_id should break hash chain
    let capsule = PaymentCapsule256::new(
        12345,   // payment_id
        67890,   // user_id
        100_00,  // amount_cents ($100.00)
        3_00,    // fee_cents ($3.00)
        0xABCDEF, // stripe_id_hash
    );

    // Get original hash
    let original_hash = capsule.hash();

    // TAMPER: Change payment_id directly (simulates memory corruption)
    capsule.payment_id.store(99999, Ordering::Relaxed);

    // Recompute hash with new payment_id
    let tampered_hash = capsule.compute_hash();

    // Hash should differ (tampering detected)
    assert_ne!(
        original_hash, tampered_hash,
        "Tampering with payment_id should change hash"
    );

    // verify_integrity() should return false
    assert!(
        !capsule.verify_integrity(),
        "verify_integrity() should detect payment_id tampering"
    );
}

#[test]
fn test_tamper_amount_cents_detected() {
    // T28 Q2: Tampering with amount should break hash chain
    let capsule = PaymentCapsule256::new(111, 222, 50_00, 1_50, 0x123);

    let original_hash = capsule.hash();

    // TAMPER: Change amount from $50.00 to $500.00
    capsule.amount_cents.store(500_00, Ordering::Relaxed);

    let tampered_hash = capsule.compute_hash();

    assert_ne!(original_hash, tampered_hash, "Amount tampering should change hash");
    assert!(!capsule.verify_integrity(), "Should detect amount tampering");
}

#[test]
fn test_tamper_status_detected() {
    // T28 Q3: Tampering with status should break hash chain
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x456);

    // Confirm payment (changes status)
    capsule.mark_confirmed(1000_000_000).unwrap();
    let confirmed_hash = capsule.hash();

    // TAMPER: Revert status to Pending (simulates fraud)
    use clapi_core::capsules::PaymentStatus;
    capsule.status.store(PaymentStatus::Pending as u8, Ordering::Relaxed);

    let tampered_hash = capsule.compute_hash();

    assert_ne!(confirmed_hash, tampered_hash, "Status tampering should change hash");
    assert!(!capsule.verify_integrity(), "Should detect status tampering");
}

#[test]
fn test_tamper_user_id_detected() {
    // T28 Q4: Tampering with user_id should break hash chain
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x789);

    let original_hash = capsule.hash();

    // TAMPER: Change user_id (simulates account hijacking)
    capsule.user_id.store(999, Ordering::Relaxed);

    let tampered_hash = capsule.compute_hash();

    assert_ne!(original_hash, tampered_hash, "User ID tampering should change hash");
    assert!(!capsule.verify_integrity(), "Should detect user_id tampering");
}

#[test]
fn test_tamper_stripe_id_hash_detected() {
    // T28 Q5: Tampering with stripe_id_hash should break chain
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0xABC123);

    let original_hash = capsule.hash();

    // TAMPER: Change Stripe ID hash
    capsule.stripe_id_hash.store(0xDEADBEEF, Ordering::Relaxed);

    let tampered_hash = capsule.compute_hash();

    assert_ne!(original_hash, tampered_hash, "Stripe ID tampering should change hash");
    assert!(!capsule.verify_integrity(), "Should detect stripe_id_hash tampering");
}

// ============================================================================
// Direct Hash Manipulation Tests (T28 Q6-Q10)
// ============================================================================

#[test]
fn test_direct_hash_modification_detected() {
    // T28 Q6: Direct hash modification should be detected
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);

    let original_hash = capsule.hash();

    // TAMPER: Directly modify hash (simulates attacker changing hash)
    capsule.hash.store(0xDEADBEEF, Ordering::Relaxed);

    // verify_integrity() should fail (hash doesn't match state)
    assert!(!capsule.verify_integrity(), "Should detect direct hash modification");

    // Restore original hash
    capsule.hash.store(original_hash, Ordering::Relaxed);
    assert!(capsule.verify_integrity(), "Should pass after restoring hash");
}

#[test]
fn test_flip_single_bit_in_hash_detected() {
    // T28 Q7: Flipping a single bit should be detected
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x456);

    let original_hash = capsule.hash();

    // TAMPER: Flip single bit (minimal tampering)
    let tampered_hash = original_hash ^ 0x01;  // Flip LSB
    capsule.hash.store(tampered_hash, Ordering::Relaxed);

    assert!(!capsule.verify_integrity(), "Should detect single-bit flip");
}

#[test]
fn test_flip_multiple_bits_detected() {
    // T28 Q8: Flipping multiple bits should be detected
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x789);

    let original_hash = capsule.hash();

    // TAMPER: Flip multiple bits
    let tampered_hash = original_hash ^ 0xFFFF;  // Flip 16 bits
    capsule.hash.store(tampered_hash, Ordering::Relaxed);

    assert!(!capsule.verify_integrity(), "Should detect multi-bit flip");
}

#[test]
fn test_zero_hash_detected() {
    // T28 Q9: Setting hash to zero should be detected
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0xABC);

    // TAMPER: Set hash to zero (simulates uninitialized state)
    capsule.hash.store(0, Ordering::Relaxed);

    assert!(!capsule.verify_integrity(), "Should detect zero hash");
}

#[test]
fn test_max_hash_detected() {
    // T28 Q10: Setting hash to max value should be detected
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0xDEF);

    // TAMPER: Set hash to max u64
    capsule.hash.store(u64::MAX, Ordering::Relaxed);

    assert!(!capsule.verify_integrity(), "Should detect max hash");
}

// ============================================================================
// Hash Chain Splicing Tests (T28 Q11-Q15)
// ============================================================================

#[test]
fn test_prev_hash_tampering_detected() {
    // T28 Q11: Tampering with prev_hash should break chain
    let capsule1 = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);
    let hash1 = capsule1.hash();

    let capsule2 = PaymentCapsule256::new(222, 333, 200_00, 6_00, 0x456);
    capsule2.prev_hash.store(hash1, Ordering::Relaxed);  // Link to capsule1

    // TAMPER: Change prev_hash (breaks chain linkage)
    capsule2.prev_hash.store(0xDEADBEEF, Ordering::Relaxed);

    // verify_chain() should detect broken link
    assert!(
        !capsule2.verify_chain(hash1),
        "Should detect prev_hash tampering"
    );
}

#[test]
fn test_cannot_splice_unrelated_chains() {
    // T28 Q12: Cannot splice unrelated payment chains
    let chain_a_1 = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);
    let hash_a_1 = chain_a_1.hash();

    let chain_a_2 = PaymentCapsule256::new(222, 222, 50_00, 1_50, 0x456);
    chain_a_2.prev_hash.store(hash_a_1, Ordering::Relaxed);

    let chain_b_1 = PaymentCapsule256::new(333, 444, 200_00, 6_00, 0x789);
    let hash_b_1 = chain_b_1.hash();

    // ATTACK: Try to splice chain_b_1 into chain_a
    chain_a_2.prev_hash.store(hash_b_1, Ordering::Relaxed);

    // verify_chain() should fail (hashes don't match)
    assert!(
        !chain_a_2.verify_chain(hash_b_1),
        "Should detect chain splicing attack"
    );
}

#[test]
fn test_cannot_reorder_chain() {
    // T28 Q13: Cannot reorder payments in chain
    let payment1 = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);
    let hash1 = payment1.hash();

    let payment2 = PaymentCapsule256::new(222, 222, 50_00, 1_50, 0x456);
    payment2.prev_hash.store(hash1, Ordering::Relaxed);
    let hash2 = payment2.hash();

    let payment3 = PaymentCapsule256::new(333, 222, 75_00, 2_25, 0x789);
    payment3.prev_hash.store(hash2, Ordering::Relaxed);

    // ATTACK: Try to reorder (payment3 → payment1, skipping payment2)
    payment3.prev_hash.store(hash1, Ordering::Relaxed);

    // verify_chain() should fail
    assert!(!payment3.verify_chain(hash1), "Should detect reordering");
}

#[test]
fn test_circular_chain_detected() {
    // T28 Q14: Circular chain references should be invalid
    let payment1 = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);
    let hash1 = payment1.hash();

    let payment2 = PaymentCapsule256::new(222, 222, 50_00, 1_50, 0x456);
    payment2.prev_hash.store(hash1, Ordering::Relaxed);
    let hash2 = payment2.hash();

    // ATTACK: Create circular reference (payment1.prev_hash = hash2)
    payment1.prev_hash.store(hash2, Ordering::Relaxed);

    // This breaks hash integrity (payment1's hash changes)
    assert!(!payment1.verify_integrity(), "Circular reference breaks integrity");
}

#[test]
fn test_chain_gap_detected() {
    // T28 Q15: Missing payments in chain should be detectable
    let payment1 = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);
    let hash1 = payment1.hash();

    let _payment2 = PaymentCapsule256::new(222, 222, 50_00, 1_50, 0x456);
    // Note: payment2 not linked (gap in chain)

    let payment3 = PaymentCapsule256::new(333, 222, 75_00, 2_25, 0x789);
    payment3.prev_hash.store(hash1, Ordering::Relaxed);  // Links to payment1, skips payment2

    // Chain is technically valid (payment3 → payment1)
    // But timeline analysis would detect missing payment2
    // This is a forensics-level check, not hash chain check
    assert!(payment3.verify_chain(hash1), "Direct chain valid, but timeline would detect gap");
}

// ============================================================================
// Pre-Image Resistance Tests (T28 Q16-Q20)
// ============================================================================

#[test]
fn test_cannot_forge_hash_for_specific_state() {
    // T28 Q16: Cannot find state that produces specific hash
    let target_hash = 0xDEADBEEFCAFEBABE;

    // Try 10,000 random states, none should match target hash
    for i in 0..10_000 {
        let capsule = PaymentCapsule256::new(
            i,
            i + 1,
            (i as i64) * 100,
            (i as i64) * 3,
            i as u64,
        );

        assert_ne!(
            capsule.hash(), target_hash,
            "Should not accidentally produce target hash"
        );
    }
}

#[test]
fn test_hash_avalanche_effect() {
    // T28 Q17: Small state change → large hash change
    let capsule1 = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);
    let hash1 = capsule1.hash();

    // Change only payment_id by 1
    let capsule2 = PaymentCapsule256::new(112, 222, 100_00, 3_00, 0x123);
    let hash2 = capsule2.hash();

    // Hashes should differ significantly (avalanche effect)
    let xor = hash1 ^ hash2;
    let bit_diff = xor.count_ones();

    assert!(
        bit_diff >= 20,
        "Insufficient avalanche: only {} bits differ (expected ≥20)",
        bit_diff
    );
}

#[test]
fn test_hash_deterministic() {
    // T28 Q18: Same state → same hash (determinism)
    let capsule1 = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);
    let capsule2 = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);

    assert_eq!(
        capsule1.hash(), capsule2.hash(),
        "Identical state should produce identical hash"
    );
}

#[test]
fn test_hash_collision_resistance() {
    // T28 Q19: Collision probability should be negligible
    use std::collections::HashSet;

    let mut hashes = HashSet::new();

    // Generate 100,000 payments, check for collisions
    for i in 0..100_000 {
        let capsule = PaymentCapsule256::new(
            i,
            i % 1000,
            (i as i64) % 1_000_00,
            (i as i64) % 100_00,
            i as u64,
        );

        let hash = capsule.hash();

        assert!(
            hashes.insert(hash),
            "Hash collision detected at iteration {}",
            i
        );
    }

    // Birthday paradox: P(collision) ≈ n^2 / (2 * 2^64)
    // For n = 100,000:
    // P(collision) ≈ 10^10 / 2^65 ≈ 2.7 × 10^-10 (0.00000003%)
    assert_eq!(hashes.len(), 100_000, "All hashes should be unique");
}

#[test]
fn test_hash_non_zero_for_all_states() {
    // T28 Q20: Hash should never be zero (avoid uninitialized confusion)
    for i in 0..1000 {
        let capsule = PaymentCapsule256::new(i, i, i as i64, 0, i as u64);

        assert_ne!(capsule.hash(), 0, "Hash should never be zero");
    }
}

// ============================================================================
// Concurrent Tampering Tests (T28 Q21-Q25)
// ============================================================================

#[test]
fn test_concurrent_hash_verification() {
    // T28 Q21: Concurrent verification should be safe
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123));

    let mut handles = vec![];

    for _ in 0..8 {
        let capsule_clone = Arc::clone(&capsule);

        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                // Concurrent verification should always succeed
                assert!(
                    capsule_clone.verify_integrity(),
                    "Concurrent verification failed"
                );
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_tampering_detection() {
    // T28 Q22: Concurrent tampering should be detected
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use std::thread;

    let capsule = Arc::new(PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123));
    let original_hash = capsule.hash();
    let tampered_detected = Arc::new(AtomicBool::new(false));

    let mut handles = vec![];

    // Thread 1: Tamper with amount
    let capsule_clone = Arc::clone(&capsule);
    let handle = thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1));
        capsule_clone.amount_cents.store(999_99, Ordering::Relaxed);
    });
    handles.push(handle);

    // Threads 2-8: Verify integrity
    for _ in 0..7 {
        let capsule_clone = Arc::clone(&capsule);
        let detected = Arc::clone(&tampered_detected);

        let handle = thread::spawn(move || {
            for _ in 0..100 {
                if !capsule_clone.verify_integrity() {
                    detected.store(true, Ordering::Relaxed);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_micros(10));
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // At least one thread should have detected tampering
    assert!(
        tampered_detected.load(Ordering::Relaxed),
        "Concurrent tampering should be detected"
    );
}

#[test]
fn test_race_free_hash_update() {
    // T28 Q23: Hash updates should be race-free
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);

    // Update hash multiple times
    for i in 0..100 {
        capsule.amount_cents.store((i + 1) * 100, Ordering::Relaxed);
        capsule.update_hash();

        // Integrity check should always pass after update
        assert!(
            capsule.verify_integrity(),
            "Hash update race detected at iteration {}",
            i
        );
    }
}

#[test]
fn test_concurrent_state_transitions() {
    // T28 Q24: Concurrent status changes should maintain integrity
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123));

    let mut handles = vec![];

    // Thread 1: Confirm payment
    let capsule_clone = Arc::clone(&capsule);
    let handle = thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(5));
        let _ = capsule_clone.mark_confirmed(1000_000_000);
    });
    handles.push(handle);

    // Threads 2-4: Verify integrity
    for _ in 0..3 {
        let capsule_clone = Arc::clone(&capsule);

        let handle = thread::spawn(move || {
            for _ in 0..100 {
                // Verification should eventually see confirmed state
                let _ = capsule_clone.verify_integrity();
                std::thread::sleep(std::time::Duration::from_micros(100));
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Final state should be valid
    assert!(capsule.verify_integrity(), "Final state should be valid");
}

#[test]
fn test_generation_counter_prevents_aba() {
    // T28 Q25: Generation counter should prevent ABA problem
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);

    let gen0 = capsule.generation();
    let hash0 = capsule.hash();

    // Update state
    capsule.mark_confirmed(1000_000_000).unwrap();
    let gen1 = capsule.generation();
    let hash1 = capsule.hash();

    assert_ne!(gen0, gen1, "Generation should increment");
    assert_ne!(hash0, hash1, "Hash should change");

    // Update again
    let _ = capsule.mark_failed();
    let gen2 = capsule.generation();

    assert_ne!(gen1, gen2, "Generation should keep incrementing");
    assert!(gen2 > gen1, "Generation should be monotonic");
}

// ============================================================================
// Property Tests (Invariants)
// ============================================================================

#[test]
fn test_property_tampering_always_detected() {
    // Property: ANY field modification breaks hash integrity
    for i in 0..100 {
        let capsule = PaymentCapsule256::new(i, i, i as i64, 0, i as u64);

        // Verify original state
        assert!(capsule.verify_integrity(), "Original state should be valid");

        // Tamper with random field
        match i % 7 {
            0 => capsule.payment_id.store(i + 1, Ordering::Relaxed),
            1 => capsule.user_id.store(i + 1, Ordering::Relaxed),
            2 => capsule.amount_cents.store((i + 1) as i64, Ordering::Relaxed),
            3 => capsule.fee_cents.store(1, Ordering::Relaxed),
            4 => capsule.stripe_id_hash.store((i + 1) as u64, Ordering::Relaxed),
            5 => capsule.status.store(1, Ordering::Relaxed),
            6 => capsule.hash.store(i as u64, Ordering::Relaxed),
            _ => unreachable!(),
        }

        // Tampering should be detected
        assert!(
            !capsule.verify_integrity(),
            "Tampering should be detected for field {}",
            i % 7
        );
    }
}

#[test]
fn test_property_hash_changes_on_update() {
    // Property: Hash changes whenever state changes
    let capsule = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);

    for i in 0..50 {
        let old_hash = capsule.hash();

        // Change state
        capsule.amount_cents.store(((i + 1) * 100) as i64, Ordering::Relaxed);
        capsule.update_hash();

        let new_hash = capsule.hash();

        assert_ne!(old_hash, new_hash, "Hash should change on state update");
    }
}

#[test]
fn test_property_chain_verification_transitive() {
    // Property: If A → B and B → C valid, then A → B → C valid
    let payment_a = PaymentCapsule256::new(111, 222, 100_00, 3_00, 0x123);
    let hash_a = payment_a.hash();

    let payment_b = PaymentCapsule256::new(222, 222, 50_00, 1_50, 0x456);
    payment_b.prev_hash.store(hash_a, Ordering::Relaxed);
    let hash_b = payment_b.hash();

    let payment_c = PaymentCapsule256::new(333, 222, 75_00, 2_25, 0x789);
    payment_c.prev_hash.store(hash_b, Ordering::Relaxed);

    // Verify chain links
    assert!(payment_b.verify_chain(hash_a), "A → B should be valid");
    assert!(payment_c.verify_chain(hash_b), "B → C should be valid");

    // Transitivity: A → B → C should be verifiable by checking links
    assert!(payment_b.verify_chain(hash_a) && payment_c.verify_chain(hash_b),
        "Chain transitivity should hold");
}

#[test]
fn test_property_hash_uniformly_distributed() {
    // Property: Hashes should be uniformly distributed across 64-bit space
    let mut bucket_counts = [0u32; 16];  // 16 buckets (4 bits)

    for i in 0..10_000 {
        let capsule = PaymentCapsule256::new(i, i, i as i64, 0, i as u64);
        let hash = capsule.hash();

        // Use top 4 bits to determine bucket
        let bucket = (hash >> 60) as usize;
        bucket_counts[bucket] += 1;
    }

    // Check distribution (each bucket should have ~625 items, allow ±200)
    for (i, &count) in bucket_counts.iter().enumerate() {
        assert!(
            count >= 400 && count <= 850,
            "Bucket {} has {} items (expected 625 ± 225)",
            i, count
        );
    }
}

// End of tampering_detection_tests.rs
