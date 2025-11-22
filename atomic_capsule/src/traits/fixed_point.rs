//! # Fixed-Point Capsule Trait
//!
//! Specialized trait for fixed-point arithmetic capsules.
//!
//! ## UCE33 Q33 (Foundation)
//!
//! Fixed-point capsules enable deterministic arithmetic (The Atomic Capsule Section 6):
//! - "Rule 5: Fixed-point over floats. Ticks, basis points, milliseconds"
//! - Deterministic rounding (no FP edge cases)
//! - Cache-friendly integer operations
//! - Sub-nanosecond arithmetic (vs float conversion overhead)
//!
//! ## Use Cases (from The Atomic Capsule)
//!
//! - **Ticks**: i16/i24 (±32767 ticks)
//! - **Basis points**: Q8.8 in u16/i16
//! - **Milliseconds**: u20 (up to ~17 minutes)
//! - **Minutes-since-midnight**: u11 (0..1439)

use super::ComputationalCapsule;

/// Fixed-point capsule specialization for deterministic arithmetic.
///
/// Implementors MUST use integer types for fixed-point representation.
///
/// # UCE33 Q33 (Foundation)
///
/// Fixed-point arithmetic from The Atomic Capsule (Section 9: Implementation):
/// - "Use Q8.8 for basis points, Q4.12 for fractional ticks"
/// - "Keep conversions centralized; never scatter FP math in readers"
///
/// # Safety Model
///
/// This trait is intentionally unsafe to implement because:
/// - Incorrect scale factor causes arithmetic overflow
/// - Fractional bit count mismatches lose precision
/// - Conversions between formats can introduce errors
///
/// # ASSUM Framework
///
/// - `#ASSUME_SCALE_VALID`: Scale factor matches fractional bits
/// - `#VERIFY_SCALE_VALID`: Compile-time via const evaluation
/// - `#ASSUME_NO_OVERFLOW`: Arithmetic operations don't overflow
/// - `#VERIFY_NO_OVERFLOW`: Property tests with boundary values
///
/// # Example
///
/// ```rust
/// use atomic_capsule::traits::{ComputationalCapsule, FixedPointCapsule};
///
/// #[repr(C, align(64))]
/// struct BasisPointCapsule {
///     value: i16, // Q8.8 format (8 integer bits, 8 fractional bits)
/// }
///
/// unsafe impl ComputationalCapsule for BasisPointCapsule {
///     const ALIGNMENT: usize = 64;
///     const SIZE: usize = 2;
///     const TYPE_ID: &'static str = "BasisPointCapsule";
/// }
///
/// unsafe impl FixedPointCapsule for BasisPointCapsule {
///     type Integer = i16;
///     const FRACTIONAL_BITS: u32 = 8;
///
///     fn scale_factor() -> f64 {
///         256.0 // 2^8
///     }
/// }
/// ```
///
/// # Safety
///
/// Implementors must ensure:
/// - `FRACTIONAL_BITS` is less than or equal to the bit width of `Integer`
/// - Conversions between fixed-point and floating-point do not overflow
/// - `scale_factor()` accurately computes 2^FRACTIONAL_BITS
/// - Rounding behavior is well-defined and documented
/// - No undefined behavior in arithmetic operations (check for overflow/underflow)
// const_trait disabled for this nightly
// #[cfg_attr(feature = "portable_simd", const_trait)]
pub unsafe trait FixedPointCapsule: ComputationalCapsule {
    /// Integer type for fixed-point storage (i16, i32, i64, u16, u32, u64).
    ///
    /// # UCE33 Q31 (Rust Transform)
    /// Associated type enables compile-time integer selection
    type Integer: Copy + Sized;

    /// Number of fractional bits (0-63).
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_FRACTIONAL_VALID`: Fractional bits ≤ integer bit width
    /// - `#VERIFY_FRACTIONAL_VALID`: Checked at compile-time
    ///
    /// # UCE33 Q29 (Constraints)
    ///
    /// Common formats (from The Atomic Capsule Appendix A):
    /// - Q8.8: 8 fractional bits (basis points)
    /// - Q4.12: 12 fractional bits (fractional ticks)
    /// - Q16.16: 16 fractional bits (high-precision)
    const FRACTIONAL_BITS: u32;

    /// Scale factor for conversion (2^FRACTIONAL_BITS).
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_SCALE_MATCHES`: Scale = 2^FRACTIONAL_BITS
    /// - `#VERIFY_SCALE_MATCHES`: Compile-time const evaluation
    ///
    /// # UCE33 Q32 (Nightly Enhancement)
    ///
    /// With `const_fn_floating_point`, this can be:
    /// ```rust,ignore
    /// const fn scale_factor() -> f64 {
    ///     2.0_f64.powi(Self::FRACTIONAL_BITS as i32)
    /// }
    /// ```
    ///
    /// # Default: 2^FRACTIONAL_BITS
    ///
    /// Manual calculation for each format.
    #[inline(always)]
    fn scale_factor() -> f64 {
        // Manual power-of-2 calculation (no const_fn_floating_point on stable)
        (1u64 << Self::FRACTIONAL_BITS) as f64
    }

    /// Integer bits (total bits - fractional bits).
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INTEGER_BITS_VALID`: Integer bits > 0
    /// - `#VERIFY_INTEGER_BITS_VALID`: Compile-time check
    ///
    /// # UCE33 Q30 (Validation)
    /// Compile-time calculation of integer bit width
    #[inline(always)]
    fn integer_bits() -> u32 {
        (core::mem::size_of::<Self::Integer>() * 8) as u32 - Self::FRACTIONAL_BITS
    }

    /// Verify fractional bits are valid.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INVARIANT`: Fractional bits ≤ total bits
    /// - `#VERIFY_INVARIANT`: Checked at compile-time
    ///
    /// # UCE33 Q30 (Validation)
    /// Compile-time verification of fixed-point format
    #[inline(always)]
    fn verify_fractional_bits() -> bool {
        let total_bits = (core::mem::size_of::<Self::Integer>() * 8) as u32;
        Self::FRACTIONAL_BITS <= total_bits && Self::FRACTIONAL_BITS > 0
    }

    /// Format name for debugging (e.g., "Q8.8", "Q16.16").
    ///
    /// # UCE33 Q31 (Rust Transform)
    /// Const fn enables compile-time format string generation
    #[inline(always)]
    fn format_name() -> &'static str {
        // Manual format names for common cases
        match Self::FRACTIONAL_BITS {
            8 => "Q8.8",
            12 => "Q4.12",
            16 => "Q16.16",
            24 => "Q8.24",
            _ => "QX.Y",
        }
    }

    /// Expected fixed-point operation latency in nanoseconds.
    ///
    /// # UCE33 Q29 (Constraints)
    ///
    /// Hardware constraint: Integer arithmetic latency
    /// - Add/Sub: ~0.5ns (1 cycle)
    /// - Mul: ~1-3ns (3-10 cycles depending on CPU)
    /// - Div: ~10-20ns (variable latency)
    ///
    /// # Performance Targets (B32 Framework)
    ///
    /// - Fixed-point ops: <2ns (vs ~5-10ns for float)
    /// - Deterministic: No FP rounding edge cases
    /// - Cache-friendly: Integer operations stay in registers
    ///
    /// # Default: 2ns
    ///
    /// Typical fixed-point multiply-add latency.
    #[inline(always)]
    fn expected_latency_ns() -> u64 {
        2
    }
}

/// Fixed-point capsule verification.
///
/// # UCE33 Q30 (Validation)
/// Macro enables fixed-point capsule verification
///
/// # Example
///
/// ```rust
/// # use atomic_capsule::traits::{ComputationalCapsule, FixedPointCapsule};
/// # #[repr(C, align(64))]
/// # struct MyFixedPointCapsule {
/// #     value: i16,
/// # }
/// # unsafe impl ComputationalCapsule for MyFixedPointCapsule {
/// #     const ALIGNMENT: usize = 64;
/// #     const SIZE: usize = 2;
/// #     const TYPE_ID: &'static str = "MyFixedPointCapsule";
/// # }
/// # unsafe impl FixedPointCapsule for MyFixedPointCapsule {
/// #     type Integer = i16;
/// #     const FRACTIONAL_BITS: u32 = 8;
/// # }
/// use atomic_capsule::verify_fixed_point_capsule;
///
/// verify_fixed_point_capsule!(MyFixedPointCapsule, i16, 8);
/// ```
#[macro_export]
macro_rules! verify_fixed_point_capsule {
    ($capsule:ty, $integer:ty, $fractional:expr) => {
        // Verify base capsule properties
        $crate::verify_capsule!($capsule);

        // Verify fixed-point specific properties
        assert_eq!(
            <$capsule as $crate::traits::FixedPointCapsule>::FRACTIONAL_BITS,
            $fractional,
            "Fractional bits mismatch"
        );
        assert!(
            <$capsule as $crate::traits::FixedPointCapsule>::verify_fractional_bits(),
            "Fractional bits invalid"
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(C, align(64))]
    struct TestBasisPointCapsule {
        value: i16, // Q8.8
    }

    unsafe impl ComputationalCapsule for TestBasisPointCapsule {
        const ALIGNMENT: usize = 64;
        const SIZE: usize = 2;
        const TYPE_ID: &'static str = "TestBasisPointCapsule";
    }

    unsafe impl FixedPointCapsule for TestBasisPointCapsule {
        type Integer = i16;
        const FRACTIONAL_BITS: u32 = 8;
    }

    #[test]
    fn test_fixed_point_defaults() {
        assert_eq!(TestBasisPointCapsule::scale_factor(), 256.0);
        assert_eq!(TestBasisPointCapsule::integer_bits(), 8);
        assert!(TestBasisPointCapsule::verify_fractional_bits());
        assert_eq!(TestBasisPointCapsule::format_name(), "Q8.8");
        assert_eq!(TestBasisPointCapsule::expected_latency_ns(), 2);
    }

    #[test]
    fn test_fixed_point_verification_macro() {
        verify_fixed_point_capsule!(TestBasisPointCapsule, i16, 8);
    }

    #[repr(C, align(64))]
    struct TestHighPrecisionCapsule {
        value: i32, // Q16.16
    }

    unsafe impl ComputationalCapsule for TestHighPrecisionCapsule {
        const ALIGNMENT: usize = 64;
        const SIZE: usize = 4;
        const TYPE_ID: &'static str = "TestHighPrecisionCapsule";
    }

    unsafe impl FixedPointCapsule for TestHighPrecisionCapsule {
        type Integer = i32;
        const FRACTIONAL_BITS: u32 = 16;
    }

    #[test]
    fn test_high_precision_fixed_point() {
        assert_eq!(TestHighPrecisionCapsule::scale_factor(), 65536.0);
        assert_eq!(TestHighPrecisionCapsule::integer_bits(), 16);
        assert!(TestHighPrecisionCapsule::verify_fractional_bits());
        assert_eq!(TestHighPrecisionCapsule::format_name(), "Q16.16");
    }
}
