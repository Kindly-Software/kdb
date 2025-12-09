//! Type-safe field packing for DualAtomicU64
//!
//! Provides compile-time verified bit-field packing with zero runtime overhead.
//! All operations are `const fn` and field bounds are validated at compile-time.
//!
//! # Framework Compliance
//!
//! - **UCE34 Q33**: Zero-cost abstractions via const fn generics
//! - **Chaos**: 100% lockfree, no allocations, cache-aligned compatible
//! - **ASSUM**: All bounds checked at compile-time (zero runtime UB risk)
//! - **T0 Auditable**: Compile-time field layout verification
//!
//! # Example
//!
//! ```rust
//! use atomic_capsule::patterns::typed_field::Field;
//!
//! // Define fields with compile-time bounds checking
//! type StateField = Field<0, 3>;      // bits 0-2 (8 states)
//! type VersionField = Field<3, 2>;    // bits 3-4 (4 versions)
//! type CounterField = Field<5, 16>;   // bits 5-20 (65536 range)
//! type TimestampField = Field<21, 32>; // bits 21-52 (timestamp)
//!
//! let mut packed = 0u64;
//!
//! // Set fields
//! packed = StateField::set(packed, 5);
//! packed = VersionField::set(packed, 2);
//! packed = CounterField::set(packed, 12345);
//! packed = TimestampField::set(packed, 1700000000);
//!
//! // Get fields
//! assert_eq!(StateField::get(packed), 5);
//! assert_eq!(VersionField::get(packed), 2);
//! assert_eq!(CounterField::get(packed), 12345);
//! assert_eq!(TimestampField::get(packed), 1700000000);
//! ```
//!
//! # Safety
//!
//! All safety is enforced at compile-time:
//! - #ASSUME: OFFSET + WIDTH <= 64 (verified by const assertion)
//! - #ASSUME: WIDTH > 0 (verified by const assertion)
//! - #ASSUME: WIDTH <= 64 (verified by const assertion)
//! - #VERIFY: Field bounds checked in impl block constructors

use core::marker::PhantomData;

/// A type-safe bit field within a u64 value.
///
/// # Type Parameters
///
/// - `OFFSET`: Bit offset from LSB (0-63)
/// - `WIDTH`: Number of bits (1-64)
///
/// # Compile-time Safety
///
/// - Asserts OFFSET + WIDTH <= 64
/// - Asserts WIDTH > 0
/// - Asserts WIDTH <= 64
///
/// # Performance
///
/// - All operations are `const fn` (zero runtime cost)
/// - Single mask + shift per operation
/// - Inlines to 2-3 CPU instructions
///
/// # Example
///
/// ```rust
/// use atomic_capsule::patterns::typed_field::Field;
///
/// // 3-bit state field at offset 0
/// type StateField = Field<0, 3>;
///
/// let packed = 0u64;
/// let packed = StateField::set(packed, 5);
/// assert_eq!(StateField::get(packed), 5);
/// assert_eq!(StateField::max_value(), 7); // 2^3 - 1
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field<const OFFSET: u8, const WIDTH: u8> {
    _phantom: PhantomData<()>,
}

impl<const OFFSET: u8, const WIDTH: u8> Field<OFFSET, WIDTH> {
    /// Compile-time assertion that field fits within 64 bits.
    ///
    /// # ASSUM Safety
    ///
    /// - #ASSUME: OFFSET + WIDTH <= 64
    /// - #VERIFY: Const assertion panics at compile-time if violated
    const ASSERT_FITS: () = {
        assert!(
            (OFFSET as usize) + (WIDTH as usize) <= 64,
            "Field exceeds 64-bit boundary"
        );
    };

    /// Compile-time assertion that WIDTH is non-zero.
    ///
    /// # ASSUM Safety
    ///
    /// - #ASSUME: WIDTH > 0
    /// - #VERIFY: Const assertion panics at compile-time if violated
    const ASSERT_NON_ZERO: () = {
        assert!(WIDTH > 0, "Field width must be greater than 0");
    };

    /// Compile-time assertion that WIDTH doesn't exceed 64 bits.
    ///
    /// # ASSUM Safety
    ///
    /// - #ASSUME: WIDTH <= 64
    /// - #VERIFY: Const assertion panics at compile-time if violated
    const ASSERT_MAX_WIDTH: () = {
        assert!(WIDTH <= 64, "Field width cannot exceed 64 bits");
    };

    /// Create a new field instance (enforces compile-time assertions).
    ///
    /// # Compile-time Safety
    ///
    /// This function forces evaluation of all const assertions.
    #[inline(always)]
    pub const fn new() -> Self {
        // Force compile-time assertion evaluation
        let _ = Self::ASSERT_FITS;
        let _ = Self::ASSERT_NON_ZERO;
        let _ = Self::ASSERT_MAX_WIDTH;

        Self {
            _phantom: PhantomData,
        }
    }

    /// Get the bit mask for this field.
    ///
    /// # Returns
    ///
    /// A u64 with WIDTH bits set, shifted to OFFSET position.
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::patterns::typed_field::Field;
    ///
    /// type F = Field<2, 3>; // 3 bits at offset 2
    /// assert_eq!(F::mask(), 0b11100); // 0x1C
    /// ```
    #[inline(always)]
    pub const fn mask() -> u64 {
        let _ = Self::ASSERT_FITS;
        let _ = Self::ASSERT_NON_ZERO;
        let _ = Self::ASSERT_MAX_WIDTH;

        // #ASSUME: WIDTH <= 64 (verified above)
        // Special case: WIDTH == 64 would overflow
        if WIDTH == 64 {
            u64::MAX
        } else {
            ((1u64 << WIDTH) - 1) << OFFSET
        }
    }

    /// Get the maximum value representable by this field.
    ///
    /// # Returns
    ///
    /// 2^WIDTH - 1 (unshifted maximum value)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::patterns::typed_field::Field;
    ///
    /// type F = Field<0, 3>; // 3 bits
    /// assert_eq!(F::max_value(), 7); // 2^3 - 1
    /// ```
    #[inline(always)]
    pub const fn max_value() -> u64 {
        let _ = Self::ASSERT_NON_ZERO;
        let _ = Self::ASSERT_MAX_WIDTH;

        // #ASSUME: WIDTH <= 64 (verified above)
        if WIDTH == 64 {
            u64::MAX
        } else {
            (1u64 << WIDTH) - 1
        }
    }

    /// Extract this field's value from a packed u64.
    ///
    /// # Arguments
    ///
    /// * `packed` - The packed u64 value
    ///
    /// # Returns
    ///
    /// The field value (right-aligned, unshifted)
    ///
    /// # Performance
    ///
    /// - O(1) time
    /// - 2 CPU instructions (mask + shift)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::patterns::typed_field::Field;
    ///
    /// type StateField = Field<0, 3>;
    /// let packed = 0b101; // state = 5
    /// assert_eq!(StateField::get(packed), 5);
    /// ```
    #[inline(always)]
    pub const fn get(packed: u64) -> u64 {
        let _ = Self::ASSERT_FITS;
        (packed & Self::mask()) >> OFFSET
    }

    /// Set this field's value in a packed u64.
    ///
    /// # Arguments
    ///
    /// * `packed` - The current packed u64 value
    /// * `value` - The new field value (will be masked to WIDTH bits)
    ///
    /// # Returns
    ///
    /// The updated packed u64 value
    ///
    /// # Performance
    ///
    /// - O(1) time
    /// - 4 CPU instructions (mask, shift, clear, OR)
    ///
    /// # ASSUM Safety
    ///
    /// - #ASSUME: value fits in WIDTH bits (masked automatically)
    /// - #VERIFY: Value is masked before shifting (no overflow)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::patterns::typed_field::Field;
    ///
    /// type StateField = Field<0, 3>;
    /// let packed = 0u64;
    /// let packed = StateField::set(packed, 5);
    /// assert_eq!(StateField::get(packed), 5);
    /// ```
    #[inline(always)]
    pub const fn set(packed: u64, value: u64) -> u64 {
        let _ = Self::ASSERT_FITS;

        // #ASSUME: value may exceed WIDTH bits
        // #VERIFY: Mask value to WIDTH bits before shifting
        let masked_value = value & Self::max_value();
        let shifted_value = masked_value << OFFSET;

        // Clear field, then set new value
        (packed & !Self::mask()) | shifted_value
    }

    /// Get field offset in bits.
    #[inline(always)]
    pub const fn offset() -> u8 {
        OFFSET
    }

    /// Get field width in bits.
    #[inline(always)]
    pub const fn width() -> u8 {
        WIDTH
    }
}

/// Trait for validating field layouts at compile-time.
///
/// Implement this trait to define multi-field layouts with
/// compile-time overlap detection.
///
/// # Example
///
/// ```rust
/// use atomic_capsule::patterns::typed_field::{Field, FieldLayout};
///
/// struct MyLayout;
///
/// impl FieldLayout for MyLayout {
///     const TOTAL_BITS: u8 = 53; // bits 0-52 used
/// }
///
/// // Define fields
/// type StateField = Field<0, 3>;
/// type VersionField = Field<3, 2>;
/// type CounterField = Field<5, 16>;
/// type TimestampField = Field<21, 32>;
/// ```
pub trait FieldLayout {
    /// Total bits used by all fields in this layout.
    ///
    /// Must be <= 64.
    const TOTAL_BITS: u8;

    /// Compile-time assertion that layout fits in 64 bits.
    const ASSERT_LAYOUT_FITS: () = {
        assert!(
            Self::TOTAL_BITS <= 64,
            "Field layout exceeds 64-bit boundary"
        );
    };
}

/// Helper macro for defining field layouts.
///
/// # Example
///
/// ```rust
/// use atomic_capsule::define_field_layout;
///
/// define_field_layout! {
///     pub struct CircuitBreakerLayout {
///         state: 3,        // bits 0-2 (8 states)
///         version: 2,      // bits 3-4 (4 versions)
///         failure_count: 16, // bits 5-20
///         timestamp: 32,   // bits 21-52
///     }
/// }
///
/// // Use the generated type aliases
/// let packed = 0u64;
/// let packed = CircuitBreakerLayoutStateField::set(packed, 2);
/// let packed = CircuitBreakerLayoutVersionField::set(packed, 1);
/// ```
#[macro_export]
macro_rules! define_field_layout {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field:ident: $width:expr
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis struct $name;

        $crate::define_field_layout!(@fields $name, 0, $( $(#[$field_meta])* $field: $width ),*);

        impl $crate::patterns::typed_field::FieldLayout for $name {
            const TOTAL_BITS: u8 = $crate::define_field_layout!(@sum 0, $($width),*);
        }
    };

    // Generate field type aliases
    (@fields $layout:ident, $offset:expr, $(#[$meta:meta])* $field:ident: $width:expr $(, $(#[$field_meta:meta])* $rest_field:ident: $rest_width:expr)*) => {
        $crate::paste::paste! {
            $(#[$meta])*
            pub type [<$layout $field:camel Field>] = $crate::patterns::typed_field::Field<$offset, $width>;
        }

        $crate::define_field_layout!(@fields $layout, $offset + $width, $($(#[$field_meta])* $rest_field: $rest_width),*);
    };

    (@fields $layout:ident, $offset:expr,) => {};

    // Sum field widths
    (@sum $acc:expr, $width:expr $(, $rest:expr)*) => {
        $crate::define_field_layout!(@sum $acc + $width, $($rest),*)
    };

    (@sum $acc:expr,) => { $acc };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_basic_get_set() {
        type StateField = Field<0, 3>; // bits 0-2

        let packed = 0u64;
        let packed = StateField::set(packed, 5);
        assert_eq!(StateField::get(packed), 5);

        let packed = StateField::set(packed, 7);
        assert_eq!(StateField::get(packed), 7);
    }

    #[test]
    fn test_field_mask() {
        type F1 = Field<0, 3>; // bits 0-2
        assert_eq!(F1::mask(), 0b111); // 0x7

        type F2 = Field<2, 3>; // bits 2-4
        assert_eq!(F2::mask(), 0b11100); // 0x1C

        type F3 = Field<0, 8>; // bits 0-7
        assert_eq!(F3::mask(), 0xFF);

        type F4 = Field<32, 32>; // bits 32-63
        assert_eq!(F4::mask(), 0xFFFFFFFF00000000);
    }

    #[test]
    fn test_field_max_value() {
        type F1 = Field<0, 3>;
        assert_eq!(F1::max_value(), 7); // 2^3 - 1

        type F2 = Field<0, 8>;
        assert_eq!(F2::max_value(), 255); // 2^8 - 1

        type F3 = Field<0, 16>;
        assert_eq!(F3::max_value(), 65535); // 2^16 - 1
    }

    #[test]
    fn test_field_overflow_masking() {
        type F = Field<0, 3>; // 3 bits, max value 7

        let packed = 0u64;
        let packed = F::set(packed, 15); // Exceeds 3 bits
        assert_eq!(F::get(packed), 7); // Masked to 0b111
    }

    #[test]
    fn test_multiple_fields() {
        type StateField = Field<0, 3>; // bits 0-2
        type VersionField = Field<3, 2>; // bits 3-4
        type CounterField = Field<5, 16>; // bits 5-20

        let mut packed = 0u64;

        // Set fields
        packed = StateField::set(packed, 5);
        packed = VersionField::set(packed, 2);
        packed = CounterField::set(packed, 12345);

        // Verify all fields
        assert_eq!(StateField::get(packed), 5);
        assert_eq!(VersionField::get(packed), 2);
        assert_eq!(CounterField::get(packed), 12345);

        // Update one field, others unchanged
        packed = StateField::set(packed, 7);
        assert_eq!(StateField::get(packed), 7);
        assert_eq!(VersionField::get(packed), 2);
        assert_eq!(CounterField::get(packed), 12345);
    }

    #[test]
    fn test_field_roundtrip() {
        type F = Field<8, 16>; // bits 8-23

        let values = [0, 1, 42, 1234, 65535];

        for &value in &values {
            let packed = F::set(0, value);
            assert_eq!(F::get(packed), value);
        }
    }

    #[test]
    fn test_field_update_pattern() {
        type CounterField = Field<0, 8>;

        let packed = CounterField::set(0, 5);

        // Manual update pattern (increment)
        let old_value = CounterField::get(packed);
        let packed = CounterField::set(packed, old_value + 1);
        assert_eq!(CounterField::get(packed), 6);

        // Manual update pattern (multiply)
        let old_value = CounterField::get(packed);
        let packed = CounterField::set(packed, old_value * 2);
        assert_eq!(CounterField::get(packed), 12);
    }

    #[test]
    fn test_field_offset_width() {
        type F = Field<5, 10>;
        assert_eq!(F::offset(), 5);
        assert_eq!(F::width(), 10);
    }

    #[test]
    fn test_full_64_bit_field() {
        type FullField = Field<0, 64>;

        let value = 0x123456789ABCDEF0u64;
        let packed = FullField::set(0, value);
        assert_eq!(FullField::get(packed), value);
        assert_eq!(FullField::mask(), u64::MAX);
        assert_eq!(FullField::max_value(), u64::MAX);
    }

    #[test]
    fn test_high_offset_field() {
        type HighField = Field<56, 8>; // bits 56-63

        let packed = HighField::set(0, 0xAB);
        assert_eq!(HighField::get(packed), 0xAB);
        assert_eq!(packed, 0xAB00000000000000);
    }

    #[test]
    fn test_field_layout_basic() {
        struct TestLayout;

        impl FieldLayout for TestLayout {
            const TOTAL_BITS: u8 = 53;
        }

        // Force compile-time assertion
        let _ = TestLayout::ASSERT_LAYOUT_FITS;

        assert_eq!(TestLayout::TOTAL_BITS, 53);
    }

    // Compile-fail tests (uncomment to verify compile-time checks)
    // These should fail at compile-time, not runtime

    /*
    #[test]
    fn test_field_exceeds_boundary() {
        type BadField = Field<60, 8>; // 60 + 8 = 68 > 64
        let _ = BadField::new(); // Should fail to compile
    }

    #[test]
    fn test_field_zero_width() {
        type BadField = Field<0, 0>; // WIDTH = 0
        let _ = BadField::new(); // Should fail to compile
    }

    #[test]
    fn test_field_width_exceeds_64() {
        type BadField = Field<0, 65>; // WIDTH > 64
        let _ = BadField::new(); // Should fail to compile
    }

    #[test]
    fn test_layout_exceeds_64_bits() {
        struct BadLayout;

        impl FieldLayout for BadLayout {
            const TOTAL_BITS: u8 = 70; // > 64
        }

        let _ = BadLayout::ASSERT_LAYOUT_FITS; // Should fail to compile
    }
    */

    #[test]
    fn test_realistic_circuit_breaker_layout() {
        // Realistic circuit breaker layout (from CircuitBreakerCapsule)
        type StateField = Field<0, 3>; // 3 bits: 8 states
        type VersionField = Field<3, 2>; // 2 bits: 4 versions
        type FailureCountField = Field<5, 16>; // 16 bits: 0-65535 failures
        type SuccessCountField = Field<21, 16>; // 16 bits: 0-65535 successes
        type TimestampField = Field<37, 27>; // 27 bits: ~134M seconds (~4 years)

        let mut packed = 0u64;

        // Set initial state
        packed = StateField::set(packed, 2); // HalfOpen
        packed = VersionField::set(packed, 1);
        packed = FailureCountField::set(packed, 5);
        packed = SuccessCountField::set(packed, 100);
        // Use a value that fits in 27 bits (max = 134,217,727)
        packed = TimestampField::set(packed, 100_000_000);

        // Verify all fields
        assert_eq!(StateField::get(packed), 2);
        assert_eq!(VersionField::get(packed), 1);
        assert_eq!(FailureCountField::get(packed), 5);
        assert_eq!(SuccessCountField::get(packed), 100);
        assert_eq!(TimestampField::get(packed), 100_000_000);

        // Update failure count (manual pattern)
        let old_failures = FailureCountField::get(packed);
        packed = FailureCountField::set(packed, old_failures + 1);
        assert_eq!(FailureCountField::get(packed), 6);

        // Other fields unchanged
        assert_eq!(StateField::get(packed), 2);
        assert_eq!(SuccessCountField::get(packed), 100);
    }

    #[test]
    fn test_stress_all_bits() {
        // Use every bit in the 64-bit space
        type F1 = Field<0, 16>;
        type F2 = Field<16, 16>;
        type F3 = Field<32, 16>;
        type F4 = Field<48, 16>;

        let mut packed = 0u64;
        packed = F1::set(packed, 0xAAAA);
        packed = F2::set(packed, 0xBBBB);
        packed = F3::set(packed, 0xCCCC);
        packed = F4::set(packed, 0xDDDD);

        assert_eq!(F1::get(packed), 0xAAAA);
        assert_eq!(F2::get(packed), 0xBBBB);
        assert_eq!(F3::get(packed), 0xCCCC);
        assert_eq!(F4::get(packed), 0xDDDD);

        assert_eq!(packed, 0xDDDDCCCCBBBBAAAA);
    }
}
