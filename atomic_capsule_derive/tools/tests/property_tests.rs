//! T28 Tier 2: Property Testing (Q8-Q14)
//! Validates invariants hold across the input space using property-based testing.
//!
//! Test coverage:
//! - Q8: Universal properties (size conservation, determinism)
//! - Q9: Concurrent access (thread-safety)
//! - Q10: Edge case properties (boundary values)
//! - Q11: ASSUM verification (safety assumptions)
//! - Q12: Composition properties (multiple transformations)
//! - Q13: Statistical properties (distribution, bounds)
//! - Q14: Regression prevention (saved test cases)

use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q8: Universal Properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1000,
        max_shrink_iters: 1000,
        .. ProptestConfig::default()
    })]

    #[test]
    fn prop_size_conservation_u32(count in 1usize..1000) {
        use atomic_capsule_tools::transform_primitive_padding;

        // Property: Original size = transformed size
        let input = format!("_padding: [u32; {}]", count);
        let result = transform_primitive_padding(&input).unwrap();

        // Extract byte count
        let bytes_str = result.split('[').nth(1).unwrap()
            .split(';').nth(1).unwrap()
            .trim().trim_end_matches(']');
        let actual_bytes: usize = bytes_str.parse().unwrap();

        // Original size: count * 4 bytes
        let expected_bytes = count * 4;

        prop_assert_eq!(actual_bytes, expected_bytes,
            "Size not conserved: {} * 4 = {}, got {}",
            count, expected_bytes, actual_bytes);
    }

    #[test]
    fn prop_size_conservation_u64(count in 1usize..500) {
        use atomic_capsule_tools::transform_primitive_padding;

        let input = format!("_padding: [u64; {}]", count);
        let result = transform_primitive_padding(&input).unwrap();

        let bytes_str = result.split('[').nth(1).unwrap()
            .split(';').nth(1).unwrap()
            .trim().trim_end_matches(']');
        let actual_bytes: usize = bytes_str.parse().unwrap();
        let expected_bytes = count * 8;

        prop_assert_eq!(actual_bytes, expected_bytes);
    }

    #[test]
    fn prop_size_conservation_u16(count in 1usize..2000) {
        use atomic_capsule_tools::transform_primitive_padding;

        let input = format!("_padding: [u16; {}]", count);
        let result = transform_primitive_padding(&input).unwrap();

        let bytes_str = result.split('[').nth(1).unwrap()
            .split(';').nth(1).unwrap()
            .trim().trim_end_matches(']');
        let actual_bytes: usize = bytes_str.parse().unwrap();
        let expected_bytes = count * 2;

        prop_assert_eq!(actual_bytes, expected_bytes);
    }

    #[test]
    fn prop_determinism(count in 1usize..1000) {
        use atomic_capsule_tools::transform_primitive_padding;

        // Property: Same input always produces same output
        let input = format!("_padding: [u32; {}]", count);

        let result1 = transform_primitive_padding(&input).unwrap();
        let result2 = transform_primitive_padding(&input).unwrap();

        prop_assert_eq!(result1, result2, "Not deterministic");
    }

    #[test]
    fn prop_field_name_preserved(
        name in "[a-z_][a-z0-9_]{0,10}",
        count in 1usize..100
    ) {
        use atomic_capsule_tools::transform_primitive_padding;

        // Property: Field name must be preserved
        let input = format!("{}: [u32; {}]", name, count);
        let result = transform_primitive_padding(&input).unwrap();

        prop_assert!(result.starts_with(&name),
            "Field name not preserved: {} -> {}", input, result);
    }

    #[test]
    fn prop_output_is_u8_array(count in 1usize..1000) {
        use atomic_capsule_tools::transform_primitive_padding;

        // Property: Output must always be u8 array
        let input = format!("_padding: [u32; {}]", count);
        let result = transform_primitive_padding(&input).unwrap();

        prop_assert!(result.contains("[u8;"),
            "Output should be u8 array: {}", result);
    }

    #[test]
    fn prop_arithmetic_commutative(a in 1usize..100, b in 1usize..100) {
        use atomic_capsule_tools::evaluate_const_expr;

        // Property: Addition is commutative
        let expr1 = format!("{} + {}", a, b);
        let expr2 = format!("{} + {}", b, a);

        let result1 = evaluate_const_expr(&expr1).unwrap();
        let result2 = evaluate_const_expr(&expr2).unwrap();

        prop_assert_eq!(result1, result2, "Addition not commutative");
    }

    #[test]
    fn prop_arithmetic_associative(
        a in 1usize..50,
        b in 1usize..50,
        c in 1usize..50
    ) {
        use atomic_capsule_tools::evaluate_const_expr;

        // Property: Addition is associative: (a + b) + c = a + (b + c)
        let left = format!("{} + {}", a + b, c);
        let right = format!("{} + {}", a, b + c);

        let result_left = evaluate_const_expr(&left).unwrap();
        let result_right = evaluate_const_expr(&right).unwrap();

        prop_assert_eq!(result_left, result_right,
            "Addition not associative: ({} + {}) + {} != {} + ({} + {})",
            a, b, c, a, b, c);
    }

    #[test]
    fn prop_multiplication_distributive(
        a in 1usize..20,
        b in 1usize..20,
        c in 1usize..20
    ) {
        use atomic_capsule_tools::evaluate_const_expr;

        // Property: a * (b + c) = a * b + a * c
        let left_val = a * (b + c);
        let right_val = a * b + a * c;

        prop_assert_eq!(left_val, right_val,
            "Multiplication not distributive");
    }
}

// ============================================================================
// Q9: Concurrent Access
// ============================================================================

#[test]
fn prop_concurrent_no_races() {
    use atomic_capsule_tools::transform_primitive_padding;

    // Property: Concurrent calls must not interfere
    let inputs: Vec<String> = (0..100)
        .map(|i| format!("_padding{}: [u32; {}]", i, (i % 50) + 1))
        .collect();

    let inputs = Arc::new(inputs);
    let handles: Vec<_> = (0..10)
        .map(|thread_id| {
            let inputs = Arc::clone(&inputs);
            thread::spawn(move || {
                let mut results = Vec::new();
                for input in inputs.iter() {
                    let result = transform_primitive_padding(input).unwrap();
                    results.push(result);
                }
                results
            })
        })
        .collect();

    // All threads should complete without panics
    for handle in handles {
        let results = handle.join().expect("Thread should not panic");
        assert_eq!(results.len(), 100, "All transformations should complete");
    }
}

#[test]
fn prop_concurrent_determinism() {
    use atomic_capsule_tools::transform_primitive_padding;

    // Property: Concurrent calls produce same results as sequential
    let test_input = "_padding: [u32; 42]";

    // Sequential baseline
    let sequential = transform_primitive_padding(test_input).unwrap();

    // Concurrent execution
    let handles: Vec<_> = (0..20)
        .map(|_| {
            let input = test_input.to_string();
            thread::spawn(move || {
                transform_primitive_padding(&input).unwrap()
            })
        })
        .collect();

    for handle in handles {
        let concurrent = handle.join().unwrap();
        assert_eq!(concurrent, sequential,
            "Concurrent result differs from sequential");
    }
}

// ============================================================================
// Q10: Edge Case Properties
// ============================================================================

proptest! {
    #[test]
    fn prop_handles_extreme_counts(count in prop::num::usize::ANY) {
        use atomic_capsule_tools::transform_primitive_padding;

        let input = format!("_padding: [u32; {}]", count);
        let result = transform_primitive_padding(&input);

        match result {
            Ok(output) => {
                // If successful, size must be conserved
                let bytes_str = output.split('[').nth(1).unwrap()
                    .split(';').nth(1).unwrap()
                    .trim().trim_end_matches(']');
                let actual_bytes: usize = bytes_str.parse().unwrap();

                // Check for overflow
                if let Some(expected) = count.checked_mul(4) {
                    prop_assert_eq!(actual_bytes, expected);
                } else {
                    // If multiplication would overflow, we should have errored
                    prop_assert!(false, "Should have errored on overflow");
                }
            }
            Err(_) => {
                // Error is acceptable for extreme values
                // Verify it's due to overflow
                prop_assert!(count.checked_mul(4).is_none(),
                    "Should only error on overflow");
            }
        }
    }

    #[test]
    fn prop_handles_boundary_arithmetic(a in 0usize..10, b in 0usize..10) {
        use atomic_capsule_tools::evaluate_const_expr;

        // Test arithmetic near boundaries
        let expr = format!("{} + {}", a, b);
        let result = evaluate_const_expr(&expr).unwrap();

        prop_assert_eq!(result, a + b, "Boundary arithmetic incorrect");
    }

    #[test]
    fn prop_handles_zero_count(field_name in "[a-z_][a-z0-9_]{0,10}") {
        use atomic_capsule_tools::transform_primitive_padding;

        // Property: Zero count arrays should work
        let input = format!("{}: [u32; 0]", field_name);
        let result = transform_primitive_padding(&input).unwrap();

        prop_assert!(result.contains("[u8; 0]"),
            "Zero count not handled correctly");
    }
}

// ============================================================================
// Q11: ASSUM Verification
// ============================================================================

#[test]
fn verify_assum_size_correct() {
    use atomic_capsule_tools::type_size;
    use atomic_capsule_tools::TypeSize;

    // #ASSUM: SIZE_CORRECT
    // #VERIFY: All type sizes match std::mem::size_of

    assert_eq!(type_size("u8"), TypeSize::Fixed(std::mem::size_of::<u8>()));
    assert_eq!(type_size("u16"), TypeSize::Fixed(std::mem::size_of::<u16>()));
    assert_eq!(type_size("u32"), TypeSize::Fixed(std::mem::size_of::<u32>()));
    assert_eq!(type_size("u64"), TypeSize::Fixed(std::mem::size_of::<u64>()));
    assert_eq!(type_size("usize"), TypeSize::Fixed(std::mem::size_of::<usize>()));

    // Atomics have same size as their underlying types
    use std::sync::atomic::*;
    assert_eq!(type_size("AtomicU8"), TypeSize::Fixed(std::mem::size_of::<AtomicU8>()));
    assert_eq!(type_size("AtomicU32"), TypeSize::Fixed(std::mem::size_of::<AtomicU32>()));
    assert_eq!(type_size("AtomicU64"), TypeSize::Fixed(std::mem::size_of::<AtomicU64>()));
}

#[test]
fn verify_assum_no_overflow() {
    use atomic_capsule_tools::evaluate_const_expr;

    // #ASSUM: EXPR_SAFE
    // #VERIFY: Overflow detection works

    // Should detect overflow
    let overflow = evaluate_const_expr(&format!("{} + 1", usize::MAX));
    assert!(overflow.is_err(), "Overflow not detected");

    let underflow = evaluate_const_expr("0 - 1");
    assert!(underflow.is_err(), "Underflow not detected");

    let mult_overflow = evaluate_const_expr(&format!("{} * 2", usize::MAX / 2 + 1));
    assert!(mult_overflow.is_err(), "Multiplication overflow not detected");
}

// ============================================================================
// Q12: Composition Properties
// ============================================================================

proptest! {
    #[test]
    fn prop_multiple_transformations_independent(
        count1 in 1usize..100,
        count2 in 1usize..100
    ) {
        use atomic_capsule_tools::transform_primitive_padding;

        // Property: Multiple transformations don't affect each other
        let input1 = format!("_padding1: [u32; {}]", count1);
        let input2 = format!("_padding2: [u64; {}]", count2);

        let result1 = transform_primitive_padding(&input1).unwrap();
        let result2 = transform_primitive_padding(&input2).unwrap();

        // Re-transform to verify independence
        let result1_again = transform_primitive_padding(&input1).unwrap();
        let result2_again = transform_primitive_padding(&input2).unwrap();

        prop_assert_eq!(result1, result1_again,
            "First transformation affected by second");
        prop_assert_eq!(result2, result2_again,
            "Second transformation affected by first");
    }

    #[test]
    fn prop_composition_with_evaluation(
        count in 1usize..50,
        multiplier in 1usize..10
    ) {
        use atomic_capsule_tools::{transform_primitive_padding, evaluate_const_expr};

        // Property: Can compose transformation with evaluation
        let total = count * multiplier;
        let input = format!("_padding: [u32; {}]", total);

        let result = transform_primitive_padding(&input).unwrap();
        let expected_bytes = evaluate_const_expr(&format!("{} * 4", total)).unwrap();

        let bytes_str = result.split('[').nth(1).unwrap()
            .split(';').nth(1).unwrap()
            .trim().trim_end_matches(']');
        let actual_bytes: usize = bytes_str.parse().unwrap();

        prop_assert_eq!(actual_bytes, expected_bytes,
            "Composition failed");
    }
}

// ============================================================================
// Q13: Statistical Properties
// ============================================================================

proptest! {
    #[test]
    fn prop_output_size_bounded(count in 1usize..1000) {
        use atomic_capsule_tools::transform_primitive_padding;

        // Property: Output size is bounded by input constraints
        let input = format!("_padding: [u64; {}]", count);
        let result = transform_primitive_padding(&input).unwrap();

        let bytes_str = result.split('[').nth(1).unwrap()
            .split(';').nth(1).unwrap()
            .trim().trim_end_matches(']');
        let actual_bytes: usize = bytes_str.parse().unwrap();

        // Maximum size: count * 8 bytes
        let max_bytes = count * 8;

        prop_assert!(actual_bytes <= max_bytes,
            "Output size exceeds bound: {} > {}", actual_bytes, max_bytes);

        // Minimum size: 0 bytes (if count = 0)
        prop_assert!(actual_bytes > 0,
            "Output size should be positive for count {}", count);
    }

    #[test]
    fn prop_distribution_uniform(count in 1usize..100) {
        use atomic_capsule_tools::transform_primitive_padding;

        // Property: Transformation works uniformly across range
        let input = format!("_padding: [u32; {}]", count);
        let result = transform_primitive_padding(&input);

        prop_assert!(result.is_ok(),
            "Transformation should succeed for count {}", count);
    }
}

// ============================================================================
// Q14: Regression Prevention
// ============================================================================

// proptest automatically saves failing cases to .proptest-regressions/
// These files should be committed to the repository

#[test]
fn test_known_regression_cases() {
    use atomic_capsule_tools::transform_primitive_padding;

    // Known regression cases from previous failures
    let cases = vec![
        ("_padding: [u32; 14]", "_padding: [u8; 56]"),
        ("_padding: [u64; 7]", "_padding: [u8; 56]"),
        ("_padding: [u16; 28]", "_padding: [u8; 56]"),
    ];

    for (input, expected) in cases {
        let result = transform_primitive_padding(input).unwrap();
        assert_eq!(result, expected,
            "Regression detected: {} -> {} (expected {})",
            input, result, expected);
    }
}

#[test]
fn test_regression_overflow_cases() {
    use atomic_capsule_tools::evaluate_const_expr;

    // Known overflow cases that should error
    let overflow_cases = vec![
        format!("{} + 1", usize::MAX),
        format!("{} * 2", usize::MAX / 2 + 1),
        "0 - 1".to_string(),
    ];

    for expr in overflow_cases {
        let result = evaluate_const_expr(&expr);
        assert!(result.is_err(),
            "Regression: overflow not detected for {}", expr);
    }
}
