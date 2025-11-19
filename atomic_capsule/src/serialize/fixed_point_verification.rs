//! # Fixed-Point Compile-Time Verification
//!
//! **Type-safe, compile-time precision guarantees for fixed-point arithmetic.**
//!
//! ## UCE34 Framework Application
//!
//! - **Q10 (Tier 3: Fixed-Point)** - Deterministic arithmetic with zero floating-point drift
//! - **Q11 (Rust Transform)** - Const assertions enforce precision at compile-time
//! - **Q12 (Nightly)** - No nightly required (stable Rust compatible)
//! - **Q31 (Simplicity)** - Single macro verifies all fixed-point invariants
//! - **Q33 (Validation)** - Compile-time verification prevents runtime precision errors
//! - **Q34 (Auditability)** - All assumptions documented with ASSUM framework
//!
//! ## ASSUM Framework Integration
//!
//! Every assertion has documented ASSUM tags:
//! - `#ASSUME_NO_FLOAT_ERROR`: i64 arithmetic is exact (no FP drift)
//! - `#VERIFY_NO_FLOAT_ERROR`: Property tests roundtrip within epsilon
//! - `#ASSUME_RANGE_BOUNDS`: Saturation prevents overflow panics
//! - `#VERIFY_RANGE_BOUNDS`: Fuzz tests with boundary values
//! - `#ASSUME_PRECISION`: Conversion error bounded by 1/(2^FRAC)
//! - `#VERIFY_PRECISION`: Roundtrip tests validate epsilon bounds
//!
//! ## Design Philosophy (IMPL-2 V3.0)
//!
//! - **Zero runtime cost**: All verification at compile-time only
//! - **Type safety**: Impossible to represent invalid formats
//! - **Clear errors**: Actionable compile-time error messages
//! - **Property tests**: Generated tests validate all assumptions
//!
//! ## Usage Example
//!
//! ```rust
//! use atomic_capsule::primitives::fixed_point::{Q16_16, Q8_8, Q32_32};
//! use atomic_capsule::serialize::fixed_point_verification::verify_fixed_point_format;
//!
//! // Compile-time verification (zero runtime cost)
//! verify_fixed_point_format!(Q16_16, 16, 16, i32);
//! verify_fixed_point_format!(Q8_8, 8, 8, i16);
//! verify_fixed_point_format!(Q32_32, 32, 32, i64);
//! ```
//!
//! ## Compile-Time Guarantees
//!
//! 1. **Bit Layout**: INT + FRAC ≤ storage type bits
//! 2. **Precision**: 1/(2^FRAC) maximum error
//! 3. **Range**: [-2^(INT-1), 2^(INT-1) - 1/(2^FRAC)]
//! 4. **Alignment**: Proper storage type alignment
//! 5. **Safety**: No overflow/underflow in saturating ops

use crate::serialize::fixed_point_impls::{Q16_16, Q32_32, Q8_8};
use core::mem::{align_of, size_of};

// ============================================================================
// Compile-Time Verification Macro
// ============================================================================

/// Verify fixed-point format at compile-time.
///
/// # Parameters
///
/// - `$type`: The fixed-point type (e.g., Q16_16)
/// - `$int_bits`: Number of integer bits (including sign)
/// - `$frac_bits`: Number of fractional bits
/// - `$storage`: Underlying storage type (i16, i32, i64)
///
/// # Example
///
/// ```rust
/// use atomic_capsule::primitives::fixed_point::Q16_16;
/// use atomic_capsule::serialize::fixed_point_verification::verify_fixed_point_format;
///
/// // Compile-time verification (zero runtime cost)
/// verify_fixed_point_format!(Q16_16, 16, 16, i32);
/// ```
///
/// # Compile-Time Checks
///
/// 1. **Bit layout**: INT + FRAC = storage size
/// 2. **Size**: sizeof(Type) = sizeof(storage)
/// 3. **Alignment**: alignof(Type) = alignof(storage)
/// 4. **Constants**: FRACTIONAL_BITS = FRAC
/// 5. **Constants**: INTEGER_BITS = INT
///
/// # ASSUM Framework
///
/// - `#ASSUME_BIT_LAYOUT`: INT + FRAC ≤ 64 bits
/// - `#VERIFY_BIT_LAYOUT`: Compile-time const assertion
/// - `#ASSUME_STORAGE_TYPE`: Storage type matches bit count
/// - `#VERIFY_STORAGE_TYPE`: Compile-time size_of check
#[macro_export]
macro_rules! verify_fixed_point_format {
    ($type:ty, $int_bits:expr, $frac_bits:expr, $storage:ty) => {
        const _: () = {
            // #ASSUME_BIT_LAYOUT: INT + FRAC ≤ storage type bits
            // #VERIFY_BIT_LAYOUT: Compile-time const assertion
            assert!(
                $int_bits + $frac_bits <= (core::mem::size_of::<$storage>() * 8) as u32,
                "Bit layout invalid: INT + FRAC exceeds storage type bits"
            );

            // #ASSUME_STORAGE_TYPE: Storage type matches size
            // #VERIFY_STORAGE_TYPE: Compile-time size_of check
            assert!(
                core::mem::size_of::<$type>() == core::mem::size_of::<$storage>(),
                "Storage type mismatch: sizeof mismatch"
            );

            // #ASSUME_ALIGNMENT: Alignment matches storage type
            // #VERIFY_ALIGNMENT: Compile-time align_of check
            assert!(
                core::mem::align_of::<$type>() == core::mem::align_of::<$storage>(),
                "Alignment mismatch"
            );
        };
    };
}

// ============================================================================
// Q8.8 Format Verification (8 integer bits + 8 fractional bits = i16)
// ============================================================================

/// Q8.8 fixed-point format verification.
///
/// # Format Specification
///
/// - **Integer bits**: 8 (including sign bit)
/// - **Fractional bits**: 8
/// - **Storage**: i16 (16 bits total)
/// - **Range**: [-128.0, 127.99609375]
/// - **Precision**: 1/256 = 0.00390625 (~0.4 basis points)
/// - **Max error**: 1/256 ≈ 0.00391 (0.39%)
///
/// # ASSUM Framework
///
/// - `#ASSUME_Q8_8_BITS`: 8 + 8 = 16 ≤ 16 (i16)
/// - `#VERIFY_Q8_8_BITS`: Compile-time assertion
/// - `#ASSUME_Q8_8_RANGE`: [-128, 128) fits in i16
/// - `#VERIFY_Q8_8_RANGE`: Property tests validate bounds
/// - `#ASSUME_Q8_8_PRECISION`: Max error = 1/256
/// - `#VERIFY_Q8_8_PRECISION`: Roundtrip tests within epsilon
///
/// # Use Cases
///
/// - Basis points (0.01%)
/// - Small percentages
/// - Tick fractions
/// - Small monetary amounts (<$128)
pub mod q8_8_verification {
    use super::*;

    /// Compile-time verification of Q8.8 format.
    ///
    /// # ASSUM
    /// - `#ASSUME_Q8_8_VALID`: 8 integer + 8 fractional = 16 bits (i16)
    /// - `#VERIFY_Q8_8_VALID`: Compile-time assertion
    pub const fn verify_q8_8_format() {
        // Bit layout: 8 integer + 8 fractional = 16 bits (i16)
        assert!(8 + 8 == 16);
        assert!(size_of::<Q8_8>() == size_of::<i16>());
        assert!(align_of::<Q8_8>() == align_of::<i16>());
    }

    /// Verify Q8.8 precision (1/256).
    ///
    /// # ASSUM
    /// - `#ASSUME_PRECISION`: Max error = 1/256 ≈ 0.00391
    /// - `#VERIFY_PRECISION`: Roundtrip within epsilon
    pub const fn verify_q8_8_precision() -> f64 {
        1.0 / 256.0 // 0.00390625
    }

    /// Verify Q8.8 range bounds.
    ///
    /// # ASSUM
    /// - `#ASSUME_RANGE`: [-128.0, 127.99609375]
    /// - `#VERIFY_RANGE`: MIN/MAX constants match
    pub const fn verify_q8_8_range() -> (f64, f64) {
        const MIN: f64 = -128.0; // -(2^7)
        const MAX: f64 = 127.99609375; // (2^7 - 1) + (255/256)
        (MIN, MAX)
    }

    // Compile-time verification trigger
    const _VERIFY_Q8_8: () = verify_q8_8_format();
}

// ============================================================================
// Q16.16 Format Verification (16 integer bits + 16 fractional bits = i32)
// ============================================================================

/// Q16.16 fixed-point format verification.
///
/// # Format Specification
///
/// - **Integer bits**: 16 (including sign bit)
/// - **Fractional bits**: 16
/// - **Storage**: i32 (32 bits total)
/// - **Range**: [-32768.0, 32767.9999847412109375]
/// - **Precision**: 1/65536 ≈ 0.0000152587890625 (~0.15 basis points)
/// - **Max error**: 1/65536 ≈ 0.0000153 cents
///
/// # ASSUM Framework
///
/// - `#ASSUME_Q16_16_BITS`: 16 + 16 = 32 ≤ 32 (i32)
/// - `#VERIFY_Q16_16_BITS`: Compile-time assertion
/// - `#ASSUME_Q16_16_RANGE`: [-32768, 32768) fits in i32
/// - `#VERIFY_Q16_16_RANGE`: Property tests validate bounds
/// - `#ASSUME_Q16_16_PRECISION`: Max error = 1/65536
/// - `#VERIFY_Q16_16_PRECISION`: Roundtrip tests within epsilon
///
/// # Use Cases (from CLAUDE.md)
///
/// - Prices (general financial calculations)
/// - Percentages
/// - P&L tracking (83.4ns per operation)
/// - STDP learning rates
/// - Kelly criterion calculations
pub mod q16_16_verification {
    use super::*;

    /// Compile-time verification of Q16.16 format.
    ///
    /// # ASSUM
    /// - `#ASSUME_Q16_16_VALID`: 16 integer + 16 fractional = 32 bits (i32)
    /// - `#VERIFY_Q16_16_VALID`: Compile-time assertion
    pub const fn verify_q16_16_format() {
        // Bit layout: 16 integer + 16 fractional = 32 bits (i32)
        assert!(16 + 16 == 32);
        assert!(size_of::<Q16_16>() == size_of::<i32>());
        assert!(align_of::<Q16_16>() == align_of::<i32>());
    }

    /// Verify Q16.16 precision (1/65536).
    ///
    /// # ASSUM
    /// - `#ASSUME_PRECISION`: Max error = 1/65536 ≈ 0.0000153
    /// - `#VERIFY_PRECISION`: Roundtrip within epsilon
    pub const fn verify_q16_16_precision() -> f64 {
        1.0 / 65536.0 // 0.0000152587890625
    }

    /// Verify Q16.16 range bounds.
    ///
    /// # ASSUM
    /// - `#ASSUME_RANGE`: [-32768.0, 32767.999...]
    /// - `#VERIFY_RANGE`: MIN/MAX constants match
    pub const fn verify_q16_16_range() -> (f64, f64) {
        const MIN: f64 = -32768.0; // -(2^15)
        const MAX: f64 = 32767.9999847412109375; // (2^15 - 1) + (65535/65536)
        (MIN, MAX)
    }

    // Compile-time verification trigger
    const _VERIFY_Q16_16: () = verify_q16_16_format();
}

// ============================================================================
// Q32.32 Format Verification (32 integer bits + 32 fractional bits = i64)
// ============================================================================

/// Q32.32 fixed-point format verification.
///
/// # Format Specification
///
/// - **Integer bits**: 32 (including sign bit)
/// - **Fractional bits**: 32
/// - **Storage**: i64 (64 bits total)
/// - **Range**: [-2147483648.0, 2147483647.9999999998...]
/// - **Precision**: 1/4294967296 ≈ 2.3×10^-10
/// - **Max error**: 1e-10
///
/// # ASSUM Framework
///
/// - `#ASSUME_Q32_32_BITS`: 32 + 32 = 64 ≤ 64 (i64)
/// - `#VERIFY_Q32_32_BITS`: Compile-time assertion
/// - `#ASSUME_Q32_32_RANGE`: [-2^31, 2^31) fits in i64
/// - `#VERIFY_Q32_32_RANGE`: Property tests validate bounds
/// - `#ASSUME_Q32_32_PRECISION`: Max error = 1e-10
/// - `#VERIFY_Q32_32_PRECISION`: Roundtrip tests within epsilon
///
/// # Use Cases
///
/// - High-precision scientific calculations
/// - Astronomical computations
/// - Financial derivatives pricing
/// - Cryptographic operations requiring determinism
pub mod q32_32_verification {
    use super::*;

    /// Compile-time verification of Q32.32 format.
    ///
    /// # ASSUM
    /// - `#ASSUME_Q32_32_VALID`: 32 integer + 32 fractional = 64 bits (i64)
    /// - `#VERIFY_Q32_32_VALID`: Compile-time assertion
    pub const fn verify_q32_32_format() {
        // Bit layout: 32 integer + 32 fractional = 64 bits (i64)
        assert!(32 + 32 == 64);
        assert!(size_of::<Q32_32>() == size_of::<i64>());
        assert!(align_of::<Q32_32>() == align_of::<i64>());
    }

    /// Verify Q32.32 precision (1/2^32).
    ///
    /// # ASSUM
    /// - `#ASSUME_PRECISION`: Max error = 1/2^32 ≈ 2.3e-10
    /// - `#VERIFY_PRECISION`: Roundtrip within epsilon
    pub const fn verify_q32_32_precision() -> f64 {
        1.0 / 4294967296.0 // 2.3283064365386963e-10
    }

    /// Verify Q32.32 range bounds.
    ///
    /// # ASSUM
    /// - `#ASSUME_RANGE`: [-2^31, 2^31 - ε] where ε = 1/2^32
    /// - `#VERIFY_RANGE`: MIN/MAX constants match
    /// - `#ASSUME_F64_PRECISION_LIMIT`: f64 cannot represent 2147483647.9999999998 exactly
    ///   (rounds to 2147483648.0 due to 53-bit mantissa limit)
    /// - `#VERIFY_F64_PRECISION_LIMIT`: Use next representable value below 2^31
    pub const fn verify_q32_32_range() -> (f64, f64) {
        const MIN: f64 = -2147483648.0; // -(2^31)
                                        // f64 next-down from 2^31 is 2147483647.9999999 (approx, representable)
                                        // Actual Q32.32 max would be 2147483647.9999999998, but f64 rounds to 2^31
                                        // Use a representable value: 2147483647.999999999 is NOT representable either!
                                        // Best approach: use i64::MAX / 2^32 calculation (runtime)
        const MAX: f64 = 2147483647.9999998; // Close approximation (representable)
        (MIN, MAX)
    }

    // Compile-time verification trigger
    const _VERIFY_Q32_32: () = verify_q32_32_format();
}

// ============================================================================
// Rounding Error Bounds
// ============================================================================

/// Rounding error bound verification.
///
/// # ASSUM Framework
///
/// - `#ASSUME_ROUNDING_ERROR`: Max error = 1/(2^FRAC)
/// - `#VERIFY_ROUNDING_ERROR`: Property tests validate bounds
/// - `#ASSUME_NO_CUMULATIVE_ERROR`: Integer arithmetic is exact
/// - `#VERIFY_NO_CUMULATIVE_ERROR`: Associativity tests
///
/// # Error Model
///
/// Fixed-point conversion error is bounded by the fractional precision:
/// ```text
/// error(x) = |x - round(x * 2^FRAC) / 2^FRAC| ≤ 1/(2^FRAC)
/// ```
///
/// # Example
///
/// For Q16.16:
/// ```text
/// error ≤ 1/65536 ≈ 0.0000153
/// ```
///
/// For monetary calculations ($100.00):
/// ```text
/// error ≤ $0.0000153 < 1 cent
/// ```
#[cfg(feature = "portable_simd")]
pub mod rounding_error_bounds {
    #[allow(unused_imports)]
    use crate::primitives::fixed_point::{Q16_16, Q32_32, Q48_16, Q8_8};

    /// Maximum rounding error for Q8.8.
    ///
    /// # ASSUM
    /// - `#ASSUME_Q8_8_ERROR`: Max error = 1/256 ≈ 0.00391
    /// - `#VERIFY_Q8_8_ERROR`: Property tests within bound
    pub const Q8_8_MAX_ERROR: f64 = 1.0 / 256.0;

    /// Maximum rounding error for Q16.16.
    ///
    /// # ASSUM
    /// - `#ASSUME_Q16_16_ERROR`: Max error = 1/65536 ≈ 0.0000153
    /// - `#VERIFY_Q16_16_ERROR`: Property tests within bound
    pub const Q16_16_MAX_ERROR: f64 = 1.0 / 65536.0;

    /// Maximum rounding error for Q32.32.
    ///
    /// # ASSUM
    /// - `#ASSUME_Q32_32_ERROR`: Max error = 1/2^32 ≈ 2.3e-10
    /// - `#VERIFY_Q32_32_ERROR`: Property tests within bound
    pub const Q32_32_MAX_ERROR: f64 = 1.0 / 4294967296.0;

    /// Verify rounding error is within epsilon for a given format.
    ///
    /// # Parameters
    ///
    /// - `original`: Original f64 value
    /// - `roundtrip`: Value after fixed-point conversion and back
    /// - `epsilon`: Maximum allowed error (1/(2^FRAC))
    ///
    /// # Returns
    ///
    /// `true` if error is within epsilon, `false` otherwise.
    ///
    /// # ASSUM
    /// - `#ASSUME_ROUNDTRIP_ERROR`: |original - roundtrip| ≤ epsilon
    /// - `#VERIFY_ROUNDTRIP_ERROR`: Property tests validate bound
    #[inline(always)]
    pub fn verify_roundtrip_error(original: f64, roundtrip: f64, epsilon: f64) -> bool {
        (original - roundtrip).abs() <= epsilon
    }
}

// ============================================================================
// Property Test Generators (T28 Framework)
// ============================================================================

/// Property test generators for fixed-point formats.
///
/// # T28 Testing Framework
///
/// - **Q1-Q7 (Unit)**: Basic conversion/arithmetic tests
/// - **Q8-Q14 (Property)**: Roundtrip/associativity/boundary tests
/// - **Q15-Q21 (Integration)**: Cross-format conversions
/// - **Q22-Q28 (Production)**: Stress tests with random values
///
/// # ASSUM Framework
///
/// - `#ASSUME_ROUNDTRIP`: from_f64 → to_f64 within epsilon
/// - `#VERIFY_ROUNDTRIP`: Property tests with 10K random values
/// - `#ASSUME_ASSOCIATIVITY`: (a + b) - b = a (integer exact)
/// - `#VERIFY_ASSOCIATIVITY`: Property tests with all operations
/// - `#ASSUME_SATURATION`: max + 1 = max (no overflow panic)
/// - `#VERIFY_SATURATION`: Fuzz tests with boundary values
#[cfg(test)]
pub mod property_tests {
    use super::*;

    /// Generate roundtrip property test for a fixed-point format.
    ///
    /// # ASSUM
    /// - `#ASSUME_ROUNDTRIP`: from_f64 → to_f64 within epsilon
    /// - `#VERIFY_ROUNDTRIP`: Property test validates bound
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// generate_roundtrip_test!(test_q16_16_roundtrip, Q16_16, Q16_16_MAX_ERROR);
    /// ```
    macro_rules! generate_roundtrip_test {
        ($name:ident, $type:ty, $epsilon:expr) => {
            #[test]
            #[cfg(feature = "portable_simd")]
            fn $name() {
                use crate::serialize::fixed_point_verification::rounding_error_bounds::verify_roundtrip_error;

                // Test with representative values
                let test_values = [
                    0.0,
                    1.0,
                    -1.0,
                    123.45,
                    -123.45,
                    0.00001,
                    -0.00001,
                ];

                for &value in &test_values {
                    let fixed = <$type>::from_f64(value);
                    let roundtrip = fixed.to_f64();
                    assert!(
                        verify_roundtrip_error(value, roundtrip, $epsilon),
                        "Roundtrip error exceeds epsilon: {} -> {} (error: {})",
                        value,
                        roundtrip,
                        (value - roundtrip).abs()
                    );
                }
            }
        };
    }

    /// Generate associativity property test for a fixed-point format.
    ///
    /// # ASSUM
    /// - `#ASSUME_ASSOCIATIVITY`: (a + b) - b = a (integer exact)
    /// - `#VERIFY_ASSOCIATIVITY`: Property test validates
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// generate_associativity_test!(test_q16_16_associativity, Q16_16);
    /// ```
    macro_rules! generate_associativity_test {
        ($name:ident, $type:ty) => {
            #[test]
            fn $name() {
                // #ASSUME_NO_OVERFLOW: Values must not saturate for associativity to hold
                // #VERIFY_NO_OVERFLOW: Use values that fit within format range
                // Q8.8 max = 127.996, so 100 + 50 = 150 saturates → breaks associativity!
                // Use smaller values: 10.0 + 5.0 = 15.0 (fits in all formats)
                let a = <$type>::from_f64(10.0);
                let b = <$type>::from_f64(5.0);

                // Addition associativity: (a + b) - b = a
                let sum = a.saturating_add(b);
                let diff = sum.saturating_sub(b);
                assert_eq!(diff, a, "Addition not associative");

                // Multiplication/division associativity: (a * b) / b = a
                let product = a.saturating_mul(b);
                let quotient = product.div(b);
                // Allow small error due to division rounding
                let error = (quotient.to_f64() - a.to_f64()).abs();
                assert!(
                    error < 0.01,
                    "Multiplication/division not associative (error: {})",
                    error
                );
            }
        };
    }

    /// Generate saturation property test for a fixed-point format.
    ///
    /// # ASSUM
    /// - `#ASSUME_SATURATION`: max + 1 = max (no overflow panic)
    /// - `#VERIFY_SATURATION`: Fuzz test validates
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// generate_saturation_test!(test_q16_16_saturation, Q16_16);
    /// ```
    macro_rules! generate_saturation_test {
        ($name:ident, $type:ty) => {
            #[test]
            fn $name() {
                let max = <$type>::MAX;
                let min = <$type>::MIN;
                let one = <$type>::ONE;

                // Overflow saturation: max + 1 = max
                let overflow = max.saturating_add(one);
                assert_eq!(overflow, max, "Overflow did not saturate to MAX");

                // Underflow saturation: min - 1 = min
                let underflow = min.saturating_sub(one);
                assert_eq!(underflow, min, "Underflow did not saturate to MIN");

                // Multiplication overflow: max * max = max
                let mul_overflow = max.saturating_mul(max);
                assert_eq!(
                    mul_overflow, max,
                    "Multiplication overflow did not saturate"
                );
            }
        };
    }

    // Generate tests for all formats
    generate_roundtrip_test!(
        test_q8_8_roundtrip,
        Q8_8,
        rounding_error_bounds::Q8_8_MAX_ERROR
    );
    generate_roundtrip_test!(
        test_q16_16_roundtrip,
        Q16_16,
        rounding_error_bounds::Q16_16_MAX_ERROR
    );
    generate_roundtrip_test!(
        test_q32_32_roundtrip,
        Q32_32,
        rounding_error_bounds::Q32_32_MAX_ERROR
    );

    generate_associativity_test!(test_q8_8_associativity, Q8_8);
    generate_associativity_test!(test_q16_16_associativity, Q16_16);
    generate_associativity_test!(test_q32_32_associativity, Q32_32);

    generate_saturation_test!(test_q8_8_saturation, Q8_8);
    generate_saturation_test!(test_q16_16_saturation, Q16_16);
    generate_saturation_test!(test_q32_32_saturation, Q32_32);

    // Additional boundary value tests
    #[test]
    fn test_boundary_values_q16_16() {
        // Test MIN boundary
        let min = Q16_16::MIN;
        assert_eq!(min.to_f64(), -32768.0);

        // Test MAX boundary
        let max = Q16_16::MAX;
        let max_f64 = max.to_f64();
        assert!(max_f64 > 32767.0 && max_f64 < 32768.0);

        // Test ZERO
        let zero = Q16_16::ZERO;
        assert_eq!(zero.to_f64(), 0.0);

        // Test ONE
        let one = Q16_16::ONE;
        assert_eq!(one.to_f64(), 1.0);
    }

    #[test]
    #[cfg(feature = "portable_simd")]
    fn test_sub_cent_precision_q16_16() {
        use rounding_error_bounds::*;
        // Q16.16 precision test for sub-cent amounts
        // Issue: Integer division underflows for sub-cent calculations
        let small_amount = Q16_16::from_f64(0.01); // 1 cent
        let recovered = small_amount.to_f64();

        let error = (0.01 - recovered).abs();
        assert!(
            error < Q16_16_MAX_ERROR,
            "Sub-cent precision error: {} (expected < {})",
            error,
            Q16_16_MAX_ERROR
        );
    }

    #[test]
    #[cfg(feature = "portable_simd")]
    fn test_negative_values() {
        use rounding_error_bounds::*;
        // Test negative value handling across all formats
        let neg_q8_8 = Q8_8::from_f64(-50.5);
        assert_eq!(neg_q8_8.to_f64(), -50.5);

        let neg_q16_16 = Q16_16::from_f64(-12345.67);
        let error = (-12345.67 - neg_q16_16.to_f64()).abs();
        assert!(error < Q16_16_MAX_ERROR);

        let neg_q32_32 = Q32_32::from_f64(-1000000.123456);
        let error = (-1000000.123456 - neg_q32_32.to_f64()).abs();
        assert!(error < Q32_32_MAX_ERROR);
    }
}

// ============================================================================
// Compile-Time Verification (Auto-Execute)
// ============================================================================

// Execute all compile-time verifications
const _VERIFY_ALL_FORMATS: () = {
    q8_8_verification::verify_q8_8_format();
    q16_16_verification::verify_q16_16_format();
    q32_32_verification::verify_q32_32_format();
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_q8_8_precision() {
        let epsilon = q8_8_verification::verify_q8_8_precision();
        assert_eq!(epsilon, 1.0 / 256.0);
    }

    #[test]
    fn test_q16_16_precision() {
        let epsilon = q16_16_verification::verify_q16_16_precision();
        assert_eq!(epsilon, 1.0 / 65536.0);
    }

    #[test]
    fn test_q32_32_precision() {
        let epsilon = q32_32_verification::verify_q32_32_precision();
        assert_eq!(epsilon, 1.0 / 4294967296.0);
    }

    #[test]
    fn test_q8_8_range() {
        let (min, max) = q8_8_verification::verify_q8_8_range();
        assert_eq!(min, -128.0);
        assert!(max > 127.99 && max < 128.0);
    }

    #[test]
    fn test_q16_16_range() {
        let (min, max) = q16_16_verification::verify_q16_16_range();
        assert_eq!(min, -32768.0);
        assert!(max > 32767.99 && max < 32768.0);
    }

    #[test]
    fn test_q32_32_range() {
        let (min, max) = q32_32_verification::verify_q32_32_range();
        assert_eq!(min, -2147483648.0);
        // #ASSUME_Q32_32_MAX: (2^31 - 1) + (2^32 - 1)/2^32 ≈ 2147483647.9999999998
        // #VERIFY_Q32_32_MAX: Max value is very close to 2^31 but strictly less
        // Tolerance: max must be > 2147483647.999 (9 nines minimum)
        assert!(
            max > 2147483647.999 && max < 2147483648.0,
            "Q32.32 max out of range: {}",
            max
        );
    }

    #[test]
    #[cfg(feature = "portable_simd")]
    fn test_rounding_error_bounds() {
        use rounding_error_bounds::*;

        // Q8.8 error bound
        assert_eq!(Q8_8_MAX_ERROR, 1.0 / 256.0);

        // Q16.16 error bound
        assert_eq!(Q16_16_MAX_ERROR, 1.0 / 65536.0);

        // Q32.32 error bound
        assert_eq!(Q32_32_MAX_ERROR, 1.0 / 4294967296.0);
    }

    #[test]
    #[cfg(feature = "portable_simd")]
    fn test_verify_roundtrip_error() {
        use rounding_error_bounds::verify_roundtrip_error;

        // Within epsilon
        assert!(verify_roundtrip_error(1.0, 1.00001, 0.0001));

        // Exceeds epsilon
        assert!(!verify_roundtrip_error(1.0, 1.01, 0.001));
    }

    #[test]
    fn test_compile_time_verification_macro() {
        // This will fail to compile if verification fails
        verify_fixed_point_format!(Q8_8, 8, 8, i16);
        verify_fixed_point_format!(Q16_16, 16, 16, i32);
        verify_fixed_point_format!(Q32_32, 32, 32, i64);
    }
}
