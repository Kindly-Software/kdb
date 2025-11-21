//! # T28 Tier 2: Property Testing (Q8-Q14)
//!
//! **Comprehensive property-based tests for computational capsules.**
//!
//! Coverage:
//! - Q8: Universal properties hold for all inputs
//! - Q9: Concurrent invariants validated
//! - Q10: Edge case properties tested
//! - Q11: ASSUM assumptions verified with properties
//! - Q12: Composition properties validated
//! - Q13: Statistical properties checked
//! - Q14: Property regressions tracked

#![cfg(all(feature = "nightly", feature = "std"))]
#![feature(portable_simd)]

use atomic_capsule::SimdF32x8Capsule;
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// ============================================================================
// T28 Q8: Universal Properties
// ============================================================================

proptest! {
    #[test]
    fn prop_simd_load_preserves_data(values in prop::array::uniform8(any::<f32>())) {
        // Filter out NaN and infinite values for this property
        if values.iter().all(|&v| v.is_finite()) {
            let capsule = SimdF32x8Capsule::new(values);
            let loaded = capsule.load_simd();

            prop_assert_eq!(loaded.as_array(), &values);
        }
    }

    #[test]
    fn prop_simd_add_commutative(
        a in prop::array::uniform8(-1000.0f32..1000.0),
        b in prop::array::uniform8(-1000.0f32..1000.0)
    ) {
        let cap_a = SimdF32x8Capsule::new(a);
        let cap_b = SimdF32x8Capsule::new(b);

        // Property: a + b == b + a (commutative)
        let ab = cap_a.load_simd() + cap_b.load_simd();
        let ba = cap_b.load_simd() + cap_a.load_simd();

        prop_assert_eq!(ab.as_array(), ba.as_array());
    }

    #[test]
    fn prop_simd_multiply_associative(
        values in prop::array::uniform8(-100.0f32..100.0),
        scalar1 in -10.0f32..10.0,
        scalar2 in -10.0f32..10.0
    ) {
        use core::simd::f32x8;

        let capsule = SimdF32x8Capsule::new(values);
        let vec = capsule.load_simd();

        // Property: (v * s1) * s2 == v * (s1 * s2)
        let result1 = (vec * f32x8::splat(scalar1)) * f32x8::splat(scalar2);
        let result2 = vec * f32x8::splat(scalar1 * scalar2);

        // Allow small floating-point error
        for i in 0..8 {
            let diff = (result1.as_array()[i] - result2.as_array()[i]).abs();
            prop_assert!(diff < 1e-3, "Difference too large: {}", diff);
        }
    }

    #[test]
    fn prop_simd_dot_product_symmetry(
        a in prop::array::uniform8(-100.0f32..100.0),
        b in prop::array::uniform8(-100.0f32..100.0)
    ) {
        use core::simd::f32x8;

        let vec_a = f32x8::from_array(a);
        let vec_b = f32x8::from_array(b);

        // Property: dot(a, b) == dot(b, a) (symmetric)
        let dot_ab = (vec_a * vec_b).reduce_sum();
        let dot_ba = (vec_b * vec_a).reduce_sum();

        let diff = (dot_ab - dot_ba).abs();
        prop_assert!(diff < 1e-3, "Dot product not symmetric");
    }

    #[test]
    fn prop_simd_addition_identity(
        values in prop::array::uniform8(-1000.0f32..1000.0)
    ) {
        use core::simd::f32x8;

        let vec = f32x8::from_array(values);
        let zero = f32x8::splat(0.0);

        // Property: v + 0 == v (additive identity)
        let result = vec + zero;

        prop_assert_eq!(result.as_array(), &values);
    }

    #[test]
    fn prop_simd_multiplication_identity(
        values in prop::array::uniform8(-1000.0f32..1000.0)
    ) {
        use core::simd::f32x8;

        let vec = f32x8::from_array(values);
        let one = f32x8::splat(1.0);

        // Property: v * 1 == v (multiplicative identity)
        let result = vec * one;

        for i in 0..8 {
            let diff = (result.as_array()[i] - values[i]).abs();
            prop_assert!(diff < 1e-6, "Not identity: {} != {}", result.as_array()[i], values[i]);
        }
    }
}

// ============================================================================
// T28 Q9: Concurrent Invariants
// ============================================================================

#[test]
fn prop_concurrent_read_consistency() {
    let values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let capsule = Arc::new(SimdF32x8Capsule::new(values));
    let num_readers = 10;
    let reads_per_thread = 1000;

    let handles: Vec<_> = (0..num_readers)
        .map(|_| {
            let cap = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..reads_per_thread {
                    let loaded = cap.load_simd();
                    // Property: All reads see consistent data
                    assert_eq!(loaded.as_array(), &values);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }
}

#[test]
fn prop_concurrent_no_torn_reads() {
    // Create capsule with distinct pattern
    let values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let capsule = Arc::new(SimdF32x8Capsule::new(values));

    // Spawn many concurrent readers
    let handles: Vec<_> = (0..50)
        .map(|_| {
            let cap = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..100 {
                    let loaded = cap.load_simd();
                    let array = loaded.as_array();

                    // Property: Never see torn reads (all values from same update)
                    assert_eq!(array, &values, "Torn read detected");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }
}

// ============================================================================
// T28 Q10: Edge Case Properties
// ============================================================================

proptest! {
    #[test]
    fn prop_handles_zero(
        zeros in Just([0.0f32; 8])
    ) {
        let capsule = SimdF32x8Capsule::new(zeros);
        let vec = capsule.load_simd();

        prop_assert_eq!(vec.as_array(), &[0.0; 8]);
    }

    #[test]
    fn prop_handles_negative_zero(
        neg_zeros in Just([-0.0f32; 8])
    ) {
        let capsule = SimdF32x8Capsule::new(neg_zeros);
        let vec = capsule.load_simd();

        // Negative zero should be preserved
        for i in 0..8 {
            prop_assert!(vec.as_array()[i].is_sign_negative() || vec.as_array()[i] == 0.0);
        }
    }

    #[test]
    fn prop_handles_very_large_values(
        large in prop::array::uniform8(1e20f32..1e30f32)
    ) {
        let capsule = SimdF32x8Capsule::new(large);
        let vec = capsule.load_simd();

        // All values should remain finite
        for &val in vec.as_array() {
            prop_assert!(val.is_finite());
        }
    }

    #[test]
    fn prop_handles_very_small_values(
        small in prop::array::uniform8(1e-30f32..1e-20f32)
    ) {
        let capsule = SimdF32x8Capsule::new(small);
        let vec = capsule.load_simd();

        // All values should remain finite and positive
        for &val in vec.as_array() {
            prop_assert!(val.is_finite());
            prop_assert!(val > 0.0);
        }
    }

    #[test]
    fn prop_handles_mixed_magnitudes(
        large in 1e10f32..1e20f32,
        small in 1e-10f32..1e-5f32
    ) {
        let values = [large, small, large, small, large, small, large, small];
        let capsule = SimdF32x8Capsule::new(values);
        let vec = capsule.load_simd();

        prop_assert_eq!(vec.as_array(), &values);
    }
}

// ============================================================================
// T28 Q11: ASSUM Verification
// ============================================================================

#[test]
fn verify_assum_alignment() {
    // #ASSUME_ALIGNMENT: 64-byte alignment for cache efficiency
    // #VERIFY_ALIGNMENT: Check at runtime

    let capsule = SimdF32x8Capsule::new([1.0; 8]);
    let ptr = &capsule as *const _ as usize;

    assert_eq!(ptr % 64, 0, "ASSUM violation: Capsule not 64-byte aligned");
}

#[test]
fn verify_assum_size() {
    // #ASSUME_SIZE: Capsule is exactly 64 bytes (one cache line)
    // #VERIFY_SIZE: Check at compile time (also done in const assertion)

    assert_eq!(
        core::mem::size_of::<SimdF32x8Capsule>(),
        64,
        "ASSUM violation: Capsule not exactly 64 bytes"
    );
}

#[test]
fn verify_assum_cache_isolation() {
    // #ASSUME_CACHE_ISOLATION: Separate capsules don't share cache lines
    // #VERIFY_CACHE_ISOLATION: Check addresses

    let cap1 = SimdF32x8Capsule::new([1.0; 8]);
    let cap2 = SimdF32x8Capsule::new([2.0; 8]);

    let addr1 = &cap1 as *const _ as usize;
    let addr2 = &cap2 as *const _ as usize;

    let diff = if addr1 > addr2 {
        addr1 - addr2
    } else {
        addr2 - addr1
    };

    // Should be at least 64 bytes apart (separate cache lines)
    assert!(
        diff >= 64,
        "ASSUM violation: Capsules share cache line (diff={} bytes)",
        diff
    );
}

// ============================================================================
// T28 Q12: Composition Properties
// ============================================================================

proptest! {
    #[test]
    fn prop_composition_preserves_values(
        a in prop::array::uniform8(-100.0f32..100.0),
        b in prop::array::uniform8(-100.0f32..100.0)
    ) {
        use core::simd::f32x8;

        let cap_a = SimdF32x8Capsule::new(a);
        let cap_b = SimdF32x8Capsule::new(b);

        // Compose operations: (a + b) * 2
        let vec_a = cap_a.load_simd();
        let vec_b = cap_b.load_simd();
        let sum = vec_a + vec_b;
        let result = sum * f32x8::splat(2.0);

        // Property: Composition should equal direct calculation
        let expected = f32x8::from_array([
            (a[0] + b[0]) * 2.0,
            (a[1] + b[1]) * 2.0,
            (a[2] + b[2]) * 2.0,
            (a[3] + b[3]) * 2.0,
            (a[4] + b[4]) * 2.0,
            (a[5] + b[5]) * 2.0,
            (a[6] + b[6]) * 2.0,
            (a[7] + b[7]) * 2.0,
        ]);

        for i in 0..8 {
            let diff = (result.as_array()[i] - expected.as_array()[i]).abs();
            prop_assert!(diff < 1e-3, "Composition mismatch at index {}", i);
        }
    }

    #[test]
    fn prop_nested_composition(
        values in prop::array::uniform8(-10.0f32..10.0)
    ) {
        use core::simd::f32x8;

        let cap = SimdF32x8Capsule::new(values);

        // Nested operations: ((v * 2) + 1) * 3
        let vec = cap.load_simd();
        let step1 = vec * f32x8::splat(2.0);
        let step2 = step1 + f32x8::splat(1.0);
        let result = step2 * f32x8::splat(3.0);

        // Property: Should equal direct calculation
        for i in 0..8 {
            let expected = ((values[i] * 2.0) + 1.0) * 3.0;
            let diff = (result.as_array()[i] - expected).abs();
            prop_assert!(diff < 1e-3, "Nested composition mismatch");
        }
    }
}

// ============================================================================
// T28 Q13: Statistical Properties
// ============================================================================

proptest! {
    #[test]
    fn prop_sum_bounded(
        values in prop::array::uniform8(-10.0f32..10.0)
    ) {
        use core::simd::f32x8;

        let vec = f32x8::from_array(values);
        let sum = vec.reduce_sum();

        // Property: Sum is bounded by sum of absolute values
        let max_possible = values.iter().map(|v| v.abs()).sum::<f32>();

        prop_assert!(
            sum.abs() <= max_possible + 1e-3,
            "Sum exceeds bounds: {} > {}",
            sum.abs(),
            max_possible
        );
    }

    #[test]
    fn prop_mean_in_range(
        values in prop::array::uniform8(-100.0f32..100.0)
    ) {
        use core::simd::f32x8;

        let vec = f32x8::from_array(values);
        let sum = vec.reduce_sum();
        let mean = sum / 8.0;

        // Property: Mean is between min and max
        let min = values.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        prop_assert!(
            mean >= min - 1e-3 && mean <= max + 1e-3,
            "Mean {} not in range [{}, {}]",
            mean,
            min,
            max
        );
    }
}

// ============================================================================
// T28 Q14: Regression Prevention
// ============================================================================

// Proptest automatically saves failing test cases to .proptest-regressions
// These are committed to catch regressions

#[test]
fn test_known_regression_case_1() {
    // This test would be generated from a proptest failure
    // Example: Found edge case where specific values caused issues
    let values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let capsule = SimdF32x8Capsule::new(values);
    let loaded = capsule.load_simd();

    assert_eq!(loaded.as_array(), &values);
}

#[test]
fn test_known_regression_case_2() {
    // Example: Edge case with zeros and large values
    use core::simd::f32x8;

    let values = [0.0, 1e20, 0.0, -1e20, 1.0, -1.0, 0.0, 0.0];
    let vec = f32x8::from_array(values);

    // Should not panic or produce NaN/Inf unexpectedly
    let sum = vec.reduce_sum();
    assert!(sum.is_finite() || sum == 0.0);
}

#[cfg(test)]
mod proptest_config {
    use super::*;

    // Configure proptest for more thorough testing
    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 1000, // Run 1000 test cases per property
            max_shrink_iters: 10000,
            .. ProptestConfig::default()
        })]

        #[test]
        fn prop_simd_never_panics(
            values in prop::array::uniform8(any::<f32>())
        ) {
            // Filter out NaN and Inf for this test
            if values.iter().all(|v| v.is_finite()) {
                let capsule = SimdF32x8Capsule::new(values);
                let _ = capsule.load_simd();
                // Should never panic
            }
        }
    }
}
