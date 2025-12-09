//! WASM Property Tests - T28 Tier 2 (Q8-Q14)
//!
//! Property-based tests for atomic capsule primitives on wasm32-unknown-unknown.
//! Following T28 framework: Q8 (Universal properties), Q9 (Concurrent invariants - N/A for WASM),
//! Q10 (Edge case properties), Q11 (ASSUM verification), Q12 (Composition), Q13 (Statistical),
//! Q14 (Regression tracking).
//!
//! WASM Note: Q9 (concurrent invariants) skipped because WASM is single-threaded.
//! Focus on properties that hold regardless of execution context.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use atomic_capsule::primitives::DualAtomicU64;
use core::sync::atomic::Ordering;

// ============================================================================
// T28 Q8: Universal Properties
// ============================================================================

/// Property: Position updates conserved (what goes in, comes out)
#[wasm_bindgen_test]
#[timeout(10000)]
fn prop_position_conservation() {
    // Test across multiple values
    for initial in [0, 100, 1000, u64::MAX / 2] {
        for delta in [1, 10, 100, 1000] {
            // Arrange
            let dual = DualAtomicU64::new(initial, 0);

            // Act: Add delta
            dual.fetch_add_primary(delta, Ordering::Relaxed);

            // Assert: Position change equals delta (conservation)
            let final_val = dual.load_primary(Ordering::Relaxed);
            let expected = initial.wrapping_add(delta);
            assert_eq!(
                final_val, expected,
                "Conservation failed: {} + {} != {}",
                initial, delta, final_val
            );
        }
    }
}

/// Property: Idempotent reads (multiple reads return same value)
#[wasm_bindgen_test]
#[timeout(10000)]
fn prop_idempotent_reads() {
    // Test across different stored values
    for value in [0, 42, 1000, u64::MAX / 2, u64::MAX] {
        // Arrange
        let dual = DualAtomicU64::new(value, 0);

        // Act: Multiple reads
        let read1 = dual.load_primary(Ordering::Relaxed);
        let read2 = dual.load_primary(Ordering::Relaxed);
        let read3 = dual.load_primary(Ordering::Relaxed);

        // Assert: All reads return same value (idempotent)
        assert_eq!(read1, value);
        assert_eq!(read2, value);
        assert_eq!(read3, value);
    }
}

/// Property: Generation monotonicity (always increases)
#[wasm_bindgen_test]
#[timeout(10000)]
fn prop_generation_monotonic() {
    // Test across multiple modification sequences
    for num_ops in [10, 50, 100] {
        // Arrange
        let dual = DualAtomicU64::new(0, 0);
        let mut last_gen = dual.generation();

        // Act: Perform operations
        for i in 0..num_ops {
            dual.store_primary(i, Ordering::Relaxed);
            let current_gen = dual.generation();

            // Assert: Generation always increases (monotonic)
            assert!(
                current_gen > last_gen,
                "Generation not monotonic: {} <= {} at iteration {}",
                current_gen,
                last_gen,
                i
            );

            last_gen = current_gen;
        }
    }
}

/// Property: Commutative fetch-add (order doesn't matter for same operations)
#[wasm_bindgen_test]
#[timeout(10000)]
fn prop_fetch_add_commutative() {
    // Test: (a + b + c) should equal (c + b + a)
    let values = [10, 20, 30];

    // Path 1: Add in order
    let dual1 = DualAtomicU64::new(0, 0);
    for &val in &values {
        dual1.fetch_add_primary(val, Ordering::Relaxed);
    }
    let result1 = dual1.load_primary(Ordering::Relaxed);

    // Path 2: Add in reverse order
    let dual2 = DualAtomicU64::new(0, 0);
    for &val in values.iter().rev() {
        dual2.fetch_add_primary(val, Ordering::Relaxed);
    }
    let result2 = dual2.load_primary(Ordering::Relaxed);

    // Assert: Results equal (commutative)
    assert_eq!(result1, result2, "Fetch-add not commutative");
    assert_eq!(result1, 60); // 10 + 20 + 30
}

// ============================================================================
// T28 Q10: Edge Case Properties
// ============================================================================

/// Property: Handles extreme values correctly
#[wasm_bindgen_test]
#[timeout(10000)]
fn prop_extreme_values() {
    let test_values = [0, 1, u64::MAX / 2, u64::MAX - 1, u64::MAX];

    for &value in &test_values {
        // Arrange
        let dual = DualAtomicU64::new(value, 0);

        // Act: Store and load
        dual.store_primary(value, Ordering::Relaxed);
        let loaded = dual.load_primary(Ordering::Relaxed);

        // Assert: Value preserved
        assert_eq!(loaded, value, "Extreme value not preserved: {}", value);
    }
}

/// Property: Overflow wrapping is consistent
#[wasm_bindgen_test]
#[timeout(10000)]
fn prop_overflow_wrapping() {
    // Test wrapping addition
    let test_cases = [
        (u64::MAX, 1, 0),                    // MAX + 1 = 0
        (u64::MAX - 10, 20, 9),              // (MAX-10) + 20 = 9
        (u64::MAX / 2, u64::MAX / 2 + 2, 1), // Overflow case
    ];

    for &(initial, delta, expected) in &test_cases {
        // Arrange
        let dual = DualAtomicU64::new(initial, 0);

        // Act: Fetch-add
        dual.fetch_add_primary(delta, Ordering::Relaxed);
        let result = dual.load_primary(Ordering::Relaxed);

        // Assert: Wrapping behavior consistent
        assert_eq!(
            result, expected,
            "Overflow wrapping inconsistent: {} + {} != {}",
            initial, delta, result
        );
    }
}

/// Property: Zero is identity for addition
#[wasm_bindgen_test]
#[timeout(10000)]
fn prop_zero_identity() {
    let test_values = [0, 42, 1000, u64::MAX];

    for &value in &test_values {
        // Arrange
        let dual = DualAtomicU64::new(value, 0);

        // Act: Add zero
        dual.fetch_add_primary(0, Ordering::Relaxed);
        let result = dual.load_primary(Ordering::Relaxed);

        // Assert: Value unchanged (zero is identity)
        assert_eq!(
            result, value,
            "Zero identity violated: {} + 0 != {}",
            value, result
        );
    }
}

// ============================================================================
// T28 Q11: ASSUM Verification (WASM-specific)
// ============================================================================

/// ASSUM: AtomicU64 operations are deterministic on WASM
/// VERIFY: Sequence of operations produces consistent results
#[wasm_bindgen_test]
#[timeout(10000)]
fn verify_assum_deterministic_atomics() {
    // Run same sequence twice, verify same results
    let operations = [(1, 10), (2, 20), (3, 30), (4, 40)];

    // Run 1
    let dual1 = DualAtomicU64::new(0, 0);
    for &(key, val) in &operations {
        if key % 2 == 0 {
            dual1.store_primary(val, Ordering::Relaxed);
        } else {
            dual1.fetch_add_primary(val, Ordering::Relaxed);
        }
    }
    let result1 = dual1.load_primary(Ordering::Relaxed);

    // Run 2 (identical sequence)
    let dual2 = DualAtomicU64::new(0, 0);
    for &(key, val) in &operations {
        if key % 2 == 0 {
            dual2.store_primary(val, Ordering::Relaxed);
        } else {
            dual2.fetch_add_primary(val, Ordering::Relaxed);
        }
    }
    let result2 = dual2.load_primary(Ordering::Relaxed);

    // Assert: Results identical (deterministic)
    assert_eq!(
        result1, result2,
        "Atomic operations not deterministic on WASM"
    );
}

/// ASSUM: Memory alignment works correctly on WASM
/// VERIFY: Aligned access doesn't fault
#[wasm_bindgen_test]
#[timeout(10000)]
fn verify_assum_alignment_safe() {
    use core::mem::align_of;

    // Create aligned instance
    let dual = DualAtomicU64::new(0, 0);

    // Verify alignment (WASM should handle this)
    let alignment = align_of::<DualAtomicU64>();
    assert!(alignment >= 8, "DualAtomicU64 not properly aligned on WASM");

    // Perform operations (should not fault)
    dual.store_primary(42, Ordering::Relaxed);
    let _ = dual.load_primary(Ordering::Relaxed);
    dual.fetch_add_primary(10, Ordering::Relaxed);

    // If we reach here, alignment is safe
}

// ============================================================================
// T28 Q12: Composition Properties
// ============================================================================

/// Property: Dual channels compose independently
#[wasm_bindgen_test]
#[timeout(10000)]
fn prop_dual_channel_composition() {
    // Test multiple interleaved operations on both channels
    let dual = DualAtomicU64::new(0, 0);

    // Interleave primary and secondary operations
    for i in 0..20 {
        if i % 2 == 0 {
            dual.fetch_add_primary(i, Ordering::Relaxed);
        } else {
            dual.fetch_add_secondary(i, Ordering::Relaxed);
        }
    }

    // Calculate expected values
    let expected_primary: u64 = (0..20).step_by(2).sum(); // 0+2+4+...+18
    let expected_secondary: u64 = (1..20).step_by(2).sum(); // 1+3+5+...+19

    // Assert: Both channels accumulated correctly
    assert_eq!(dual.load_primary(Ordering::Relaxed), expected_primary);
    assert_eq!(dual.load_secondary(Ordering::Relaxed), expected_secondary);
}

// ============================================================================
// T28 Q13: Statistical Properties
// ============================================================================

/// Property: Distribution of values is uniform (no bias)
#[wasm_bindgen_test]
#[timeout(10000)]
fn prop_no_value_bias() {
    // Store and retrieve many different values
    let mut stored_values = Vec::new();

    for value in 0..100u64 {
        let dual = DualAtomicU64::new(value, 0);
        stored_values.push(dual.load_primary(Ordering::Relaxed));
    }

    // Assert: All values stored correctly (no bias)
    for (i, &stored) in stored_values.iter().enumerate() {
        assert_eq!(stored, i as u64, "Value bias detected at index {}", i);
    }
}

/// Property: Generation counter has reasonable growth rate
#[wasm_bindgen_test]
#[timeout(10000)]
fn prop_generation_growth_rate() {
    // Perform operations and track generation growth
    let dual = DualAtomicU64::new(0, 0);
    let gen_start = dual.generation();

    // Perform 10 operations
    for i in 0..10 {
        dual.store_primary(i, Ordering::Relaxed);
    }

    let gen_end = dual.generation();
    let growth = gen_end - gen_start;

    // Assert: Generation grew (exact amount may vary by implementation)
    assert!(growth >= 10, "Generation growth rate too low: {}", growth);
    assert!(growth <= 100, "Generation growth rate too high: {}", growth);
}

// ============================================================================
// T28 Q14: Regression Tracking
// ============================================================================

/// Regression test: DualAtomicU64 CAS behavior
/// (Based on actual behavior observed in development)
#[wasm_bindgen_test]
#[timeout(10000)]
fn regression_cas_behavior() {
    // Known working case from development
    let dual = DualAtomicU64::new(100, 0);

    // CAS should succeed with correct current value
    let result = dual.compare_exchange_primary(100, 200, Ordering::Relaxed, Ordering::Relaxed);

    assert_eq!(result, Ok(100), "Regression: CAS behavior changed");
    assert_eq!(dual.load_primary(Ordering::Relaxed), 200);
}

/// Regression test: Generation counter increments on modification
/// (Ensure generation counter always increments, not decrements)
#[wasm_bindgen_test]
#[timeout(10000)]
fn regression_generation_increments() {
    let dual = DualAtomicU64::new(0, 0);

    let gen1 = dual.generation();
    dual.store_primary(1, Ordering::Relaxed);
    let gen2 = dual.generation();

    // Regression check: Generation must increase, not decrease
    assert!(
        gen2 > gen1,
        "Regression: Generation counter not incrementing"
    );
}

// ============================================================================
// Fixed-Point Property Tests (Tier 3)
// ============================================================================

#[cfg(feature = "tier3")]
mod fixed_point_properties {
    use super::*;
    use atomic_capsule::primitives::fixed_point::{Q16_16, Q8_8};

    /// Property: Addition is associative
    #[wasm_bindgen_test]
    #[timeout(10000)]
    fn prop_q16_16_addition_associative() {
        let a = Q16_16::from_f64(1.5);
        let b = Q16_16::from_f64(2.5);
        let c = Q16_16::from_f64(3.5);

        // (a + b) + c
        let ab = a.add(&b);
        let ab_c = ab.add(&c);

        // a + (b + c)
        let bc = b.add(&c);
        let a_bc = a.add(&bc);

        // Assert: Associative property holds
        let result1 = ab_c.to_f64();
        let result2 = a_bc.to_f64();
        assert!(
            (result1 - result2).abs() < 0.001,
            "Addition not associative"
        );
    }

    /// Property: Multiplication is commutative
    #[wasm_bindgen_test]
    #[timeout(10000)]
    fn prop_q16_16_multiplication_commutative() {
        let a = Q16_16::from_f64(2.5);
        let b = Q16_16::from_f64(4.0);

        // a × b
        let ab = a.mul(&b);

        // b × a
        let ba = b.mul(&a);

        // Assert: Commutative property holds
        let result1 = ab.to_f64();
        let result2 = ba.to_f64();
        assert!(
            (result1 - result2).abs() < 0.001,
            "Multiplication not commutative"
        );
    }

    /// Property: Q8.8 maintains precision within range
    #[wasm_bindgen_test]
    #[timeout(10000)]
    fn prop_q8_8_precision() {
        // Test values within Q8.8 range (-128 to 127)
        let test_values = [0.0, 1.5, -1.5, 10.25, -10.25, 100.0];

        for &value in &test_values {
            let q = Q8_8::from_f32(value);
            let recovered = q.to_f32();

            // Assert: Precision within 0.01 (Q8.8 precision)
            assert!(
                (recovered - value).abs() < 0.01,
                "Q8.8 precision lost: {} != {}",
                value,
                recovered
            );
        }
    }
}

// ============================================================================
// T28 Summary Checklist for WASM Property Tests
// ============================================================================
//
// ✅ Q8: Universal properties validated (conservation, idempotence, monotonicity)
// ⏭️ Q9: Concurrent invariants (N/A - WASM is single-threaded)
// ✅ Q10: Edge case properties tested (extreme values, overflow, zero identity)
// ✅ Q11: ASSUM verified (deterministic atomics, alignment safety)
// ✅ Q12: Composition properties validated (dual channel independence)
// ✅ Q13: Statistical properties checked (no bias, reasonable growth rates)
// ✅ Q14: Regression tracking (CAS behavior, generation increments)
//
// Test Count: 15+ property tests
// Platform: wasm32-unknown-unknown (single-threaded)
// Timeout: 10 seconds per test (property tests may need more iterations)
// Framework: T28 Tier 2 (Property Testing, Q9 skipped for WASM)
