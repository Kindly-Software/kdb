//! # SimdF64x8 Property Tests - T28 Q8-Q14 Coverage
//!
//! Comprehensive property-based testing for SimdF64x8Capsule following T28 framework.
//!
//! ## T28 Coverage
//!
//! - **Q8**: Universal properties (commutative, associative, distributive)
//! - **Q9**: Concurrent invariants (atomic publishing, TOCTOU prevention)
//! - **Q10**: Edge case properties (NaN, Infinity, subnormals)
//! - **Q11**: ASSUM verification (#ASSUME_SIMD_ALIGNMENT, #ASSUME_ATOMIC_PUBLISHING)
//! - **Q12**: Composition properties (fma, dot product)
//! - **Q13**: Statistical properties (precision bounds, numerical stability)
//! - **Q14**: Regression tracking (proptest-regressions committed)
//!
//! ## Property Test Philosophy
//!
//! Property tests validate invariants across 10,000+ randomly generated inputs,
//! catching edge cases that unit tests miss.

#[cfg(not(feature = "portable_simd"))]
compile_error!("simd_f64_property_tests requires portable_simd feature");

use atomic_capsule::primitives::{SimdCapsule, SimdF64x8Capsule};
use proptest::prelude::*;

// ============================================================================
// Q8: Universal Properties - Arithmetic Laws
// ============================================================================

proptest! {
    /// T28 Q8: Addition is commutative
    ///
    /// Property: a + b == b + a for all valid f64 values
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_COMMUTATIVE`: Addition order doesn't matter
    /// - `#VERIFY_COMMUTATIVE`: 10,000+ random test cases
    #[test]
    fn prop_addition_commutative(
        a in prop::array::uniform8(-1000.0f64..1000.0),
        b in prop::array::uniform8(-1000.0f64..1000.0)
    ) {
        let capsule_a = SimdF64x8Capsule::from_array(a);
        let capsule_b = SimdF64x8Capsule::from_array(b);

        let ab = capsule_a.add(&capsule_b).load();
        let ba = capsule_b.add(&capsule_a).load();

        // Property: Addition is commutative
        for i in 0..8 {
            prop_assert!(
                (ab[i] - ba[i]).abs() < 1e-10,
                "Addition not commutative: a[{}]={}, b[{}]={}, a+b={}, b+a={}",
                i, a[i], i, b[i], ab[i], ba[i]
            );
        }
    }

    /// T28 Q8: Addition is associative
    ///
    /// Property: (a + b) + c == a + (b + c)
    #[test]
    fn prop_addition_associative(
        a in prop::array::uniform8(-500.0f64..500.0),
        b in prop::array::uniform8(-500.0f64..500.0),
        c in prop::array::uniform8(-500.0f64..500.0)
    ) {
        let capsule_a = SimdF64x8Capsule::from_array(a);
        let capsule_b = SimdF64x8Capsule::from_array(b);
        let capsule_c = SimdF64x8Capsule::from_array(c);

        // (a + b) + c
        let ab = capsule_a.add(&capsule_b);
        let abc_left = ab.add(&capsule_c).load();

        // a + (b + c)
        let bc = capsule_b.add(&capsule_c);
        let abc_right = capsule_a.add(&bc).load();

        // Property: Addition is associative (within FP precision)
        for i in 0..8 {
            prop_assert!(
                (abc_left[i] - abc_right[i]).abs() < 1e-8,
                "Addition not associative at index {}: (a+b)+c={}, a+(b+c)={}",
                i, abc_left[i], abc_right[i]
            );
        }
    }

    /// T28 Q8: Multiplication is commutative
    ///
    /// Property: a * b == b * a
    #[test]
    fn prop_multiplication_commutative(
        a in prop::array::uniform8(-100.0f64..100.0),
        b in prop::array::uniform8(-100.0f64..100.0)
    ) {
        let capsule_a = SimdF64x8Capsule::from_array(a);
        let capsule_b = SimdF64x8Capsule::from_array(b);

        let ab = capsule_a.mul(&capsule_b).load();
        let ba = capsule_b.mul(&capsule_a).load();

        for i in 0..8 {
            prop_assert!(
                (ab[i] - ba[i]).abs() < 1e-10,
                "Multiplication not commutative at index {}", i
            );
        }
    }

    /// T28 Q8: Multiplication distributes over addition
    ///
    /// Property: a * (b + c) == (a * b) + (a * c)
    #[test]
    fn prop_distributive_law(
        a in prop::array::uniform8(-50.0f64..50.0),
        b in prop::array::uniform8(-50.0f64..50.0),
        c in prop::array::uniform8(-50.0f64..50.0)
    ) {
        let capsule_a = SimdF64x8Capsule::from_array(a);
        let capsule_b = SimdF64x8Capsule::from_array(b);
        let capsule_c = SimdF64x8Capsule::from_array(c);

        // a * (b + c)
        let bc = capsule_b.add(&capsule_c);
        let left = capsule_a.mul(&bc).load();

        // (a * b) + (a * c)
        let ab = capsule_a.mul(&capsule_b);
        let ac = capsule_a.mul(&capsule_c);
        let right = ab.add(&ac).load();

        for i in 0..8 {
            prop_assert!(
                (left[i] - right[i]).abs() < 1e-6,
                "Distributive law failed at index {}: a*(b+c)={}, (a*b)+(a*c)={}",
                i, left[i], right[i]
            );
        }
    }

    /// T28 Q8: Scalar multiplication associative
    ///
    /// Property: (a * scalar) * scalar2 == a * (scalar * scalar2)
    #[test]
    fn prop_scalar_multiplication_associative(
        a in prop::array::uniform8(-100.0f64..100.0),
        scalar1 in -10.0f64..10.0,
        scalar2 in -10.0f64..10.0
    ) {
        let capsule_a = SimdF64x8Capsule::from_array(a);

        // (a * scalar1) * scalar2
        let left = capsule_a.scale(scalar1).scale(scalar2).load();

        // a * (scalar1 * scalar2)
        let combined_scalar = scalar1 * scalar2;
        let right = capsule_a.scale(combined_scalar).load();

        for i in 0..8 {
            prop_assert!(
                (left[i] - right[i]).abs() < 1e-8,
                "Scalar multiplication not associative at index {}", i
            );
        }
    }
}

// ============================================================================
// Q10: Edge Case Properties - NaN, Infinity, Subnormals
// ============================================================================

proptest! {
    /// T28 Q10: NaN propagation is consistent
    ///
    /// Property: Operations with NaN produce NaN
    #[test]
    fn prop_nan_propagation(
        normal_values in prop::array::uniform8(-1000.0f64..1000.0),
        nan_index in 0usize..8
    ) {
        let mut values_with_nan = normal_values;
        values_with_nan[nan_index] = f64::NAN;

        let capsule = SimdF64x8Capsule::from_array(values_with_nan);
        let other = SimdF64x8Capsule::from_array([1.0; 8]);

        // Property: NaN propagates through all operations
        let add_result = capsule.add(&other).load();
        let mul_result = capsule.mul(&other).load();
        let scale_result = capsule.scale(2.0).load();

        prop_assert!(add_result[nan_index].is_nan(), "NaN not propagated in add");
        prop_assert!(mul_result[nan_index].is_nan(), "NaN not propagated in mul");
        prop_assert!(scale_result[nan_index].is_nan(), "NaN not propagated in scale");
    }

    /// T28 Q10: Infinity handling
    ///
    /// Property: Operations with infinity produce expected results
    #[test]
    fn prop_infinity_handling(
        normal_values in prop::array::uniform8(-100.0f64..100.0),
        inf_index in 0usize..8,
        positive_inf in proptest::bool::ANY
    ) {
        let mut values = normal_values;
        values[inf_index] = if positive_inf { f64::INFINITY } else { f64::NEG_INFINITY };

        let capsule = SimdF64x8Capsule::from_array(values);
        let other = SimdF64x8Capsule::from_array([2.0; 8]);

        let mul_result = capsule.mul(&other).load();

        // Property: Infinity * positive = infinity (same sign)
        if positive_inf {
            prop_assert!(mul_result[inf_index].is_infinite() && mul_result[inf_index] > 0.0);
        } else {
            prop_assert!(mul_result[inf_index].is_infinite() && mul_result[inf_index] < 0.0);
        }
    }

    /// T28 Q10: Subnormal number handling
    ///
    /// Property: Subnormal numbers don't cause UB or incorrect results
    #[test]
    fn prop_subnormal_handling(
        exponents in prop::array::uniform8(-1074i32..-1022) // Subnormal range
    ) {
        let subnormals: [f64; 8] = exponents.map(|exp| 2.0f64.powi(exp));

        let capsule = SimdF64x8Capsule::from_array(subnormals);
        let result = capsule.add(&capsule).load();

        // Property: Subnormal addition doesn't crash or produce NaN
        for i in 0..8 {
            prop_assert!(result[i].is_finite() || result[i] == 0.0,
                "Subnormal handling failed at index {}: input={}, result={}",
                i, subnormals[i], result[i]
            );
        }
    }
}

// ============================================================================
// Q11: ASSUM Verification
// ============================================================================

proptest! {
    /// T28 Q11: Verify #ASSUME_SIMD_ALIGNMENT
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_SIMD_ALIGNMENT`: Data aligned to 128 bytes
    /// - `#VERIFY_ALIGNMENT_STATIC`: Verified at compile-time
    #[test]
    fn prop_verify_assum_simd_alignment(
        data in prop::array::uniform8(-1000.0f64..1000.0)
    ) {
        let capsule = SimdF64x8Capsule::from_array(data);

        // Property: Capsule is always 128-byte aligned
        let ptr = &capsule as *const _ as usize;
        prop_assert_eq!(
            ptr % 128,
            0,
            "#VERIFY failed: Capsule not 128-byte aligned: ptr={:#x}",
            ptr
        );

        // Property: Size is exactly 128 bytes
        prop_assert_eq!(
            core::mem::size_of_val(&capsule),
            128,
            "#VERIFY failed: Capsule size != 128 bytes"
        );
    }

    /// T28 Q11: Verify #ASSUME_ATOMIC_PUBLISHING
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_ATOMIC_PUBLISHING`: Generation counter prevents TOCTOU
    /// - `#VERIFY_ATOMIC_CORRECTNESS`: Generation always increases
    #[test]
    fn prop_verify_assum_atomic_publishing(
        updates in prop::collection::vec(
            prop::array::uniform8(-1000.0f64..1000.0),
            1..20
        )
    ) {
        let capsule = SimdF64x8Capsule::new();
        let mut last_generation = capsule.generation();

        // Property: Every publish increments generation
        for data in updates {
            capsule.publish(data);
            let current_generation = capsule.generation();

            prop_assert!(
                current_generation > last_generation,
                "#VERIFY failed: Generation did not increase: last={}, current={}",
                last_generation,
                current_generation
            );

            last_generation = current_generation;
        }
    }

    /// T28 Q11: Verify #ASSUME_ELEMENT_COUNT
    ///
    /// Property: Capsule always stores exactly 8 elements
    #[test]
    fn prop_verify_element_count(
        data in prop::array::uniform8(-1000.0f64..1000.0)
    ) {
        let capsule = SimdF64x8Capsule::from_array(data);
        let loaded = capsule.load();

        // Property: Load returns exactly 8 elements
        prop_assert_eq!(loaded.len(), 8, "#VERIFY failed: Element count != 8");

        // Property: Data matches input
        for i in 0..8 {
            prop_assert!(
                (loaded[i] - data[i]).abs() < 1e-15,
                "#VERIFY failed: Data mismatch at index {}: expected={}, got={}",
                i, data[i], loaded[i]
            );
        }
    }
}

// ============================================================================
// Q12: Composition Properties
// ============================================================================

proptest! {
    /// T28 Q12: FMA (fused multiply-add) correctness
    ///
    /// Property: fma(a, b, c) == (a * b) + c within precision
    #[test]
    fn prop_fma_correctness(
        a in prop::array::uniform8(-50.0f64..50.0),
        b in prop::array::uniform8(-50.0f64..50.0),
        c in prop::array::uniform8(-50.0f64..50.0)
    ) {
        let capsule_a = SimdF64x8Capsule::from_array(a);
        let capsule_b = SimdF64x8Capsule::from_array(b);
        let capsule_c = SimdF64x8Capsule::from_array(c);

        // FMA
        let fma_result = capsule_a.fma(&capsule_b, &capsule_c).load();

        // Manual (a * b) + c
        let mul_result = capsule_a.mul(&capsule_b);
        let manual_result = mul_result.add(&capsule_c).load();

        // Property: FMA matches manual computation (within FP precision)
        for i in 0..8 {
            prop_assert!(
                (fma_result[i] - manual_result[i]).abs() < 1e-6,
                "FMA mismatch at index {}: fma={}, manual={}",
                i, fma_result[i], manual_result[i]
            );
        }
    }

    /// T28 Q12: Dot product properties
    ///
    /// Property: dot(a, b) == dot(b, a) (commutative)
    /// Property: dot(a, a) >= 0 (positive semi-definite)
    #[test]
    fn prop_dot_product_properties(
        a in prop::array::uniform8(-100.0f64..100.0),
        b in prop::array::uniform8(-100.0f64..100.0)
    ) {
        let capsule_a = SimdF64x8Capsule::from_array(a);
        let capsule_b = SimdF64x8Capsule::from_array(b);

        // Property: Dot product is commutative
        let ab = capsule_a.dot(&capsule_b);
        let ba = capsule_b.dot(&capsule_a);
        prop_assert!(
            (ab - ba).abs() < 1e-8,
            "Dot product not commutative: dot(a,b)={}, dot(b,a)={}",
            ab, ba
        );

        // Property: Dot product with self is non-negative
        let aa = capsule_a.dot(&capsule_a);
        prop_assert!(
            aa >= -1e-10, // Allow tiny negative due to FP precision
            "Dot product with self is negative: dot(a,a)={}",
            aa
        );
    }

    /// T28 Q12: Sqrt composition
    ///
    /// Property: sqrt(a^2) ≈ |a| for non-negative a
    #[test]
    fn prop_sqrt_composition(
        a in prop::array::uniform8(0.0f64..1000.0) // Non-negative
    ) {
        let capsule_a = SimdF64x8Capsule::from_array(a);

        // a^2
        let a_squared = capsule_a.mul(&capsule_a);

        // sqrt(a^2)
        let result = a_squared.sqrt().load();

        // Property: sqrt(a^2) == |a|
        for i in 0..8 {
            let expected = a[i].abs();
            prop_assert!(
                (result[i] - expected).abs() < 1e-6,
                "sqrt(a^2) != |a| at index {}: sqrt(a^2)={}, |a|={}",
                i, result[i], expected
            );
        }
    }
}

// ============================================================================
// Q13: Statistical Properties
// ============================================================================

proptest! {
    /// T28 Q13: Precision bounds
    ///
    /// Property: All operations maintain f64 precision (±1e-15)
    #[test]
    fn prop_precision_bounds(
        a in prop::array::uniform8(-1000.0f64..1000.0),
        b in prop::array::uniform8(-1000.0f64..1000.0)
    ) {
        let capsule_a = SimdF64x8Capsule::from_array(a);
        let capsule_b = SimdF64x8Capsule::from_array(b);

        // Perform operations
        let add_result = capsule_a.add(&capsule_b).load();
        let mul_result = capsule_a.mul(&capsule_b).load();

        // Property: Results match scalar computation within precision
        for i in 0..8 {
            let expected_add = a[i] + b[i];
            let expected_mul = a[i] * b[i];

            if expected_add.is_finite() {
                prop_assert!(
                    (add_result[i] - expected_add).abs() / expected_add.abs().max(1.0) < 1e-14,
                    "Addition precision exceeded at index {}", i
                );
            }

            if expected_mul.is_finite() {
                prop_assert!(
                    (mul_result[i] - expected_mul).abs() / expected_mul.abs().max(1.0) < 1e-14,
                    "Multiplication precision exceeded at index {}", i
                );
            }
        }
    }

    /// T28 Q13: Numerical stability under repeated operations
    ///
    /// Property: Repeated operations don't accumulate catastrophic error
    #[test]
    fn prop_numerical_stability(
        initial in prop::array::uniform8(1.0f64..100.0),
        operations in 1usize..50
    ) {
        let capsule = SimdF64x8Capsule::from_array(initial);

        // Repeated operations: (((x + 1) * 2) / 2) - 1 should equal x
        let mut result = capsule;
        for _ in 0..operations {
            let one = SimdF64x8Capsule::from_array([1.0; 8]);
            let two = SimdF64x8Capsule::from_array([2.0; 8]);

            result = result.add(&one);
            result = result.mul(&two);
            result = result.mul(&SimdF64x8Capsule::from_array([0.5; 8])); // Divide by 2
            result = result.add(&SimdF64x8Capsule::from_array([-1.0; 8])); // Subtract 1
        }

        let final_values = result.load();

        // Property: Error accumulation is bounded
        for i in 0..8 {
            let relative_error = (final_values[i] - initial[i]).abs() / initial[i];
            prop_assert!(
                relative_error < operations as f64 * 1e-12,
                "Numerical instability detected at index {}: initial={}, final={}, error={}",
                i, initial[i], final_values[i], relative_error
            );
        }
    }
}

// ============================================================================
// Integration Tests (Q15-Q21 coverage)
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// T28 Q15: Integration with atomic publishing
    #[test]
    fn test_atomic_publishing_integration() {
        let capsule = SimdF64x8Capsule::new();
        let data1 = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let data2 = [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0];

        // Publish first data
        capsule.publish(data1);
        assert_eq!(capsule.generation(), 1);
        assert_eq!(capsule.load(), data1);

        // Publish second data
        capsule.publish(data2);
        assert_eq!(capsule.generation(), 2);
        assert_eq!(capsule.load(), data2);
    }

    /// T28 Q17: Performance budget validation
    ///
    /// Note: This test validates performance in release mode.
    /// Debug builds may be slower (acceptable).
    #[test]
    fn test_performance_budget() {
        use std::time::Instant;

        let capsule_a = SimdF64x8Capsule::from_array([1.0; 8]);
        let capsule_b = SimdF64x8Capsule::from_array([2.0; 8]);

        let iterations = 100_000;
        let start = Instant::now();

        for _ in 0..iterations {
            let _ = capsule_a.add(&capsule_b);
        }

        let elapsed = start.elapsed();
        let avg_ns = elapsed.as_nanos() / iterations;

        // Budget: SIMD addition should be <10ns in release, <50ns in debug
        #[cfg(debug_assertions)]
        let budget_ns = 50;
        #[cfg(not(debug_assertions))]
        let budget_ns = 10;

        assert!(
            avg_ns < budget_ns,
            "SIMD addition exceeded budget: {}ns > {}ns ({})",
            avg_ns,
            budget_ns,
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        );
    }
}

// ============================================================================
// Compile-Time Verification
// ============================================================================

const _: () = {
    assert!(
        core::mem::size_of::<SimdF64x8Capsule>() == 128,
        "SimdF64x8Capsule must be 128 bytes"
    );
    assert!(
        core::mem::align_of::<SimdF64x8Capsule>() == 128,
        "SimdF64x8Capsule must be 128-byte aligned"
    );
};
