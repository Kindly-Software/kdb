//! T28 Tier 1: Unit Testing (Q1-Q7)
//! Validates individual components of fix_padding_fields in isolation.
//!
//! Test coverage:
//! - Q1: Core behaviors (type_size, evaluate_const_expr, transform_primitive_padding)
//! - Q2: Edge cases (overflow, empty, boundary values)
//! - Q3: Invariants (size conservation, idempotency)
//! - Q4: All code paths (error paths, match arms)
//! - Q5: Isolation (no shared state, deterministic)
//! - Q6: Performance (<10ms per test)
//! - Q7: Readability (descriptive names, clear structure)

use std::time::Duration;

// Timeout macro for all tests
macro_rules! test_with_timeout {
    ($name:ident, $timeout_ms:expr, $body:block) => {
        #[test]
        #[timeout(Duration::from_millis($timeout_ms))]
        fn $name() {
            $body
        }
    };
}

/// Helper to measure test execution time
fn measure_time<F: FnOnce() -> R, R>(f: F) -> (R, Duration) {
    let start = std::time::Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    (result, elapsed)
}

// ============================================================================
// Q1: Core Behaviors
// ============================================================================

mod core_behaviors {
    use super::*;
    use atomic_capsule_tools::{type_size, TypeSize, evaluate_const_expr, transform_primitive_padding};

    #[test]
    fn test_type_size_u8() {
        // Arrange: u8 type string
        let ty = "u8";

        // Act: Get size
        let result = type_size(ty);

        // Assert: Returns 1 byte
        assert_eq!(result, TypeSize::Fixed(1));
    }

    #[test]
    fn test_type_size_u16() {
        assert_eq!(type_size("u16"), TypeSize::Fixed(2));
    }

    #[test]
    fn test_type_size_u32() {
        assert_eq!(type_size("u32"), TypeSize::Fixed(4));
    }

    #[test]
    fn test_type_size_u64() {
        assert_eq!(type_size("u64"), TypeSize::Fixed(8));
    }

    #[test]
    fn test_type_size_atomics() {
        assert_eq!(type_size("AtomicU8"), TypeSize::Fixed(1));
        assert_eq!(type_size("AtomicU16"), TypeSize::Fixed(2));
        assert_eq!(type_size("AtomicU32"), TypeSize::Fixed(4));
        assert_eq!(type_size("AtomicU64"), TypeSize::Fixed(8));
        assert_eq!(type_size("AtomicBool"), TypeSize::Fixed(1));
        assert_eq!(type_size("AtomicUsize"), TypeSize::Fixed(8));
    }

    #[test]
    fn test_evaluate_const_expr_literal() {
        // Arrange: Simple literal
        let expr = "42";

        // Act: Evaluate
        let result = evaluate_const_expr(expr).unwrap();

        // Assert: Returns correct value
        assert_eq!(result, 42);
    }

    #[test]
    fn test_evaluate_const_expr_addition() {
        assert_eq!(evaluate_const_expr("10 + 5").unwrap(), 15);
        assert_eq!(evaluate_const_expr("1 + 2 + 3").unwrap(), 6);
    }

    #[test]
    fn test_evaluate_const_expr_subtraction() {
        assert_eq!(evaluate_const_expr("20 - 5").unwrap(), 15);
        assert_eq!(evaluate_const_expr("100 - 50").unwrap(), 50);
    }

    #[test]
    fn test_evaluate_const_expr_multiplication() {
        assert_eq!(evaluate_const_expr("3 * 4").unwrap(), 12);
        assert_eq!(evaluate_const_expr("8 * 7").unwrap(), 56);
    }

    #[test]
    fn test_evaluate_const_expr_division() {
        assert_eq!(evaluate_const_expr("20 / 4").unwrap(), 5);
        assert_eq!(evaluate_const_expr("100 / 10").unwrap(), 10);
    }

    #[test]
    fn test_transform_primitive_padding_u32_to_u8() {
        // Arrange: u32 array padding
        let input = "_padding: [u32; 14]";

        // Act: Transform to u8 array
        let result = transform_primitive_padding(input).unwrap();

        // Assert: Correct byte count (14 * 4 = 56)
        assert_eq!(result, "_padding: [u8; 56]");
    }

    #[test]
    fn test_transform_primitive_padding_u64_to_u8() {
        let input = "_padding: [u64; 7]";
        let result = transform_primitive_padding(input).unwrap();
        assert_eq!(result, "_padding: [u8; 56]");
    }

    #[test]
    fn test_transform_primitive_padding_u16_to_u8() {
        let input = "_padding: [u16; 28]";
        let result = transform_primitive_padding(input).unwrap();
        assert_eq!(result, "_padding: [u8; 56]");
    }
}

// ============================================================================
// Q2: Edge Cases
// ============================================================================

mod edge_cases {
    use super::*;
    use atomic_capsule_tools::{evaluate_const_expr, transform_primitive_padding, PaddingError};

    #[test]
    fn test_evaluate_overflow_addition() {
        // Arrange: Expression that overflows
        let expr = format!("{} + 1", usize::MAX);

        // Act: Try to evaluate
        let result = evaluate_const_expr(&expr);

        // Assert: Returns overflow error
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PaddingError::Overflow(_)));
    }

    #[test]
    fn test_evaluate_overflow_multiplication() {
        let expr = format!("{} * 2", usize::MAX / 2 + 1);
        let result = evaluate_const_expr(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluate_division_by_zero() {
        let expr = "100 / 0";
        let result = evaluate_const_expr(expr);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PaddingError::Overflow(_)));
    }

    #[test]
    fn test_evaluate_underflow_subtraction() {
        let expr = "5 - 10";
        let result = evaluate_const_expr(expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluate_zero() {
        assert_eq!(evaluate_const_expr("0").unwrap(), 0);
    }

    #[test]
    fn test_evaluate_max_usize() {
        let expr = format!("{}", usize::MAX);
        assert_eq!(evaluate_const_expr(&expr).unwrap(), usize::MAX);
    }

    #[test]
    fn test_transform_empty_field_name() {
        // Edge case: Empty or malformed input
        let result = transform_primitive_padding("");
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_missing_colon() {
        let result = transform_primitive_padding("_padding [u32; 14]");
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_missing_brackets() {
        let result = transform_primitive_padding("_padding: u32; 14");
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_missing_semicolon() {
        let result = transform_primitive_padding("_padding: [u32 14]");
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_zero_count() {
        let input = "_padding: [u32; 0]";
        let result = transform_primitive_padding(input).unwrap();
        assert_eq!(result, "_padding: [u8; 0]");
    }

    #[test]
    fn test_transform_one_element() {
        let input = "_padding: [u32; 1]";
        let result = transform_primitive_padding(input).unwrap();
        assert_eq!(result, "_padding: [u8; 4]");
    }

    #[test]
    fn test_transform_large_count() {
        let input = "_padding: [u32; 1000]";
        let result = transform_primitive_padding(input).unwrap();
        assert_eq!(result, "_padding: [u8; 4000]");
    }
}

// ============================================================================
// Q3: Invariants
// ============================================================================

mod invariants {
    use super::*;
    use atomic_capsule_tools::{transform_primitive_padding, type_size, TypeSize};

    #[test]
    fn test_invariant_size_conservation() {
        // Invariant: Total byte size must be preserved
        let test_cases = vec![
            ("_padding: [u32; 14]", 14 * 4), // 56 bytes
            ("_padding: [u64; 7]", 7 * 8),   // 56 bytes
            ("_padding: [u16; 28]", 28 * 2), // 56 bytes
        ];

        for (input, expected_bytes) in test_cases {
            let result = transform_primitive_padding(input).unwrap();

            // Extract byte count from result: "_padding: [u8; N]"
            let bytes_str = result
                .split('[')
                .nth(1)
                .unwrap()
                .split(';')
                .nth(1)
                .unwrap()
                .trim()
                .trim_end_matches(']');
            let actual_bytes: usize = bytes_str.parse().unwrap();

            assert_eq!(
                actual_bytes, expected_bytes,
                "Size conservation violated: {} -> {} (expected {})",
                input, actual_bytes, expected_bytes
            );
        }
    }

    #[test]
    fn test_invariant_field_name_preserved() {
        // Invariant: Field name must not change
        let inputs = vec![
            "_padding",
            "_padding1",
            "_pad",
            "_reserve",
        ];

        for field_name in inputs {
            let input = format!("{}: [u32; 10]", field_name);
            let result = transform_primitive_padding(&input).unwrap();

            assert!(
                result.starts_with(field_name),
                "Field name not preserved: {} -> {}",
                input, result
            );
        }
    }

    #[test]
    fn test_invariant_idempotency() {
        // Invariant: Transforming u8 array should be no-op or error
        // (Current implementation transforms all array types, so we test that behavior)
        let input = "_padding: [u8; 56]";
        let result = transform_primitive_padding(input).unwrap();

        // Should preserve the byte array (or transform to same size)
        assert!(result.contains("[u8; 56]"));
    }

    #[test]
    fn test_invariant_type_size_positive() {
        // Invariant: All type sizes must be positive
        let types = vec!["u8", "u16", "u32", "u64", "AtomicU32", "AtomicU64"];

        for ty in types {
            match type_size(ty) {
                TypeSize::Fixed(size) => {
                    assert!(size > 0, "Type size must be positive: {}", ty);
                }
                TypeSize::Variable => {
                    // Variable types don't have fixed size, skip
                }
            }
        }
    }
}

// ============================================================================
// Q4: Code Path Coverage
// ============================================================================

mod code_coverage {
    use super::*;
    use atomic_capsule_tools::{type_size, TypeSize, evaluate_const_expr, PaddingError};

    #[test]
    fn test_all_primitive_types() {
        // Cover all primitive branches
        assert_eq!(type_size("u8"), TypeSize::Fixed(1));
        assert_eq!(type_size("i8"), TypeSize::Fixed(1));
        assert_eq!(type_size("bool"), TypeSize::Fixed(1));
        assert_eq!(type_size("u16"), TypeSize::Fixed(2));
        assert_eq!(type_size("i16"), TypeSize::Fixed(2));
        assert_eq!(type_size("u32"), TypeSize::Fixed(4));
        assert_eq!(type_size("i32"), TypeSize::Fixed(4));
        assert_eq!(type_size("f32"), TypeSize::Fixed(4));
        assert_eq!(type_size("u64"), TypeSize::Fixed(8));
        assert_eq!(type_size("i64"), TypeSize::Fixed(8));
        assert_eq!(type_size("f64"), TypeSize::Fixed(8));
        assert_eq!(type_size("usize"), TypeSize::Fixed(8));
        assert_eq!(type_size("isize"), TypeSize::Fixed(8));
    }

    #[test]
    fn test_all_atomic_types() {
        assert_eq!(type_size("AtomicU8"), TypeSize::Fixed(1));
        assert_eq!(type_size("AtomicI8"), TypeSize::Fixed(1));
        assert_eq!(type_size("AtomicBool"), TypeSize::Fixed(1));
        assert_eq!(type_size("AtomicU16"), TypeSize::Fixed(2));
        assert_eq!(type_size("AtomicI16"), TypeSize::Fixed(2));
        assert_eq!(type_size("AtomicU32"), TypeSize::Fixed(4));
        assert_eq!(type_size("AtomicI32"), TypeSize::Fixed(4));
        assert_eq!(type_size("AtomicU64"), TypeSize::Fixed(8));
        assert_eq!(type_size("AtomicI64"), TypeSize::Fixed(8));
        assert_eq!(type_size("AtomicUsize"), TypeSize::Fixed(8));
        assert_eq!(type_size("AtomicIsize"), TypeSize::Fixed(8));
    }

    #[test]
    fn test_all_arithmetic_operators() {
        // Addition
        assert!(evaluate_const_expr("10 + 5").is_ok());

        // Subtraction
        assert!(evaluate_const_expr("20 - 5").is_ok());

        // Multiplication
        assert!(evaluate_const_expr("3 * 4").is_ok());

        // Division
        assert!(evaluate_const_expr("20 / 4").is_ok());
    }

    #[test]
    fn test_all_error_types() {
        // Overflow error
        let overflow = evaluate_const_expr(&format!("{} + 1", usize::MAX));
        assert!(matches!(overflow.unwrap_err(), PaddingError::Overflow(_)));

        // Parse error
        let parse = evaluate_const_expr("invalid expression");
        assert!(matches!(parse.unwrap_err(), PaddingError::Parse(_)));

        // Invalid syntax error (from transform)
        use atomic_capsule_tools::transform_primitive_padding;
        let syntax = transform_primitive_padding("invalid");
        assert!(matches!(syntax.unwrap_err(), PaddingError::InvalidSyntax(_)));
    }
}

// ============================================================================
// Q5: Isolation and Determinism
// ============================================================================

mod isolation {
    use super::*;
    use atomic_capsule_tools::{transform_primitive_padding, evaluate_const_expr};

    #[test]
    fn test_no_shared_state() {
        // Each call should be independent
        let result1 = transform_primitive_padding("_padding: [u32; 10]").unwrap();
        let result2 = transform_primitive_padding("_padding: [u32; 10]").unwrap();

        assert_eq!(result1, result2, "Results must be identical (no shared state)");
    }

    #[test]
    fn test_deterministic_same_input() {
        // Same input must produce same output
        let input = "_padding: [u64; 7]";

        for _ in 0..100 {
            let result = transform_primitive_padding(input).unwrap();
            assert_eq!(result, "_padding: [u8; 56]");
        }
    }

    #[test]
    fn test_deterministic_evaluation() {
        // Arithmetic must be deterministic
        let expr = "14 * 4";

        for _ in 0..100 {
            assert_eq!(evaluate_const_expr(expr).unwrap(), 56);
        }
    }

    #[test]
    fn test_parallel_safety() {
        use std::thread;
        use std::sync::Arc;

        // Test that multiple threads can call functions without issues
        let handles: Vec<_> = (0..10)
            .map(|i| {
                thread::spawn(move || {
                    let input = format!("_padding{}: [u32; 10]", i);
                    transform_primitive_padding(&input).unwrap()
                })
            })
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result.contains("[u8; 40]"));
        }
    }
}

// ============================================================================
// Q6: Performance
// ============================================================================

mod performance {
    use super::*;
    use atomic_capsule_tools::{transform_primitive_padding, evaluate_const_expr, type_size};

    #[test]
    fn test_type_size_fast() {
        let (_, elapsed) = measure_time(|| {
            for _ in 0..1000 {
                let _ = type_size("u64");
            }
        });

        // Should be <1ms for 1000 calls
        assert!(elapsed < Duration::from_millis(1), "type_size too slow: {:?}", elapsed);
    }

    #[test]
    fn test_evaluate_const_expr_fast() {
        let (_, elapsed) = measure_time(|| {
            for _ in 0..1000 {
                let _ = evaluate_const_expr("14 * 4").unwrap();
            }
        });

        // Should be <10ms for 1000 calls
        assert!(elapsed < Duration::from_millis(10), "evaluate_const_expr too slow: {:?}", elapsed);
    }

    #[test]
    fn test_transform_primitive_padding_fast() {
        let (_, elapsed) = measure_time(|| {
            for _ in 0..100 {
                let _ = transform_primitive_padding("_padding: [u32; 14]").unwrap();
            }
        });

        // Should be <10ms for 100 calls
        assert!(elapsed < Duration::from_millis(10), "transform_primitive_padding too slow: {:?}", elapsed);
    }
}

// ============================================================================
// Q7: Readability
// ============================================================================

// Test names are already descriptive (test_X_Y pattern)
// Arrange-Act-Assert structure used throughout
// Clear failure messages with context

#[test]
fn test_error_messages_are_clear() {
    use atomic_capsule_tools::{transform_primitive_padding, evaluate_const_expr};

    // Test that error messages contain useful context
    let result = transform_primitive_padding("invalid");
    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(msg.len() > 10, "Error message should be descriptive");
        }
        Ok(_) => panic!("Expected error"),
    }

    let result = evaluate_const_expr("100 / 0");
    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(msg.contains("zero") || msg.contains("overflow"),
                   "Division by zero should be mentioned: {}", msg);
        }
        Ok(_) => panic!("Expected error"),
    }
}
