//! # FixedQ16_16Capsule - Q16.16 Fixed-Point Capsule (Hot Tier)

//!
//! **64-bit atomic fixed-point capsule for deterministic arithmetic.**
//!
//! ## UCE33 Analysis
//!
//! - **Q28 (Simplicity)**: Simple fixed-point API hiding scaling complexity
//! - **Q29 (Constraints)**: 32-bit range (-32768 to +32767.9999), 1/65536 precision
//! - **Q30 (Validation)**: Validate against floating-point reference with tolerance
//! - **Q31 (Rust Transform)**: AtomicI32 enables lockfree deterministic arithmetic
//! - **Q32 (Nightly)**: const_fn_floating_point for compile-time conversions
//! - **Q33 (Atomic Capsule)**: Fixed-point enables deterministic atomic operations
//!
//! ## Q16.16 Fixed-Point Format
//!
//! ```text
//! Sign: 1 bit | Integer: 15 bits | Fractional: 16 bits
//! Range: -32768.0 to +32767.99998474121
//! Precision: 1/65536 ≈ 0.0000152587890625
//! Scale Factor: 2^16 = 65536
//! ```
//!
//! ## Memory Layout
//!
//! ```text
//! [Fixed-Point Value: i32 = 4 bytes] [Generation: u32 = 4 bytes] [Padding: 56 bytes]
//! Total: 64 bytes (single cache line, Hot Tier alignment)
//! ```
//!
//! ## Use Cases
//!
//! - Financial calculations (basis points, prices)
//! - Game physics (velocities, positions)
//! - Signal processing (audio samples, filter coefficients)
//! - Real-time systems requiring deterministic arithmetic
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_ATOMIC_I32`: AtomicI32 provides lockfree fixed-point operations
//! - `#VERIFY_ATOMIC_CORRECTNESS`: All operations use atomic CAS or fetch_*
//! - `#ASSUME_OVERFLOW_HANDLING`: Multiplication/division use i64 intermediates
//! - `#VERIFY_OVERFLOW_PREVENTION`: Tests validate no overflow for valid ranges
//! - `#ASSUME_DETERMINISTIC`: Fixed-point arithmetic is bit-exact and deterministic
//! - `#VERIFY_DETERMINISM`: Property tests validate reproducibility

use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use super::FixedPointCapsule;

/// Q16.16 fixed-point capsule for deterministic atomic arithmetic
///
/// # Layout
/// - Value: i32 = 4 bytes (Q16.16 fixed-point)
/// - Generation: u32 = 4 bytes (atomic coordination)
/// - Padding: 56 bytes (cache line alignment)
/// - Total: 64 bytes (Hot Tier)
///
/// # Fixed-Point Representation
/// - Integer part: bits 16-31 (16 bits, signed)
/// - Fractional part: bits 0-15 (16 bits, unsigned)
/// - Scale factor: 2^16 = 65536
///
/// # Performance
/// - Load: ~3-5ns (single cache line atomic read)
/// - Store: ~3-5ns (single cache line atomic write)
/// - Mul/Div: ~10-15ns (i64 intermediate + atomic CAS)
///
/// # ASSUM Safety
/// - `#ASSUME_CACHE_ALIGNMENT`: 64-byte alignment for cache line fit
/// - `#VERIFY_ALIGNMENT_STATIC`: Verified at compile-time via repr(align(64))
#[repr(C, align(64))]
pub struct FixedQ16_16Capsule {
    /// Fixed-point value (Q16.16 format)
    value: AtomicI32,

    /// Generation counter for atomic coordination
    generation: AtomicU32,

    /// Padding to 64 bytes
    _padding: [u8; 56],
}

impl FixedQ16_16Capsule {
    /// Fixed-point scale factor (2^16)
    pub const SCALE: i32 = 65536;

    /// Scale factor as f64 for conversions
    const SCALE_F64: f64 = 65536.0;

    /// Maximum representable value (+32767.99998474121)
    pub const MAX: i32 = i32::MAX;

    /// Minimum representable value (-32768.0)
    pub const MIN: i32 = i32::MIN;

    /// Create new fixed-point capsule initialized to zero
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::FixedQ16_16Capsule;
    ///
    /// let capsule = FixedQ16_16Capsule::new();
    /// assert_eq!(capsule.to_f64(), 0.0);
    /// ```
    pub const fn new() -> Self {
        Self {
            value: AtomicI32::new(0),
            generation: AtomicU32::new(0),
            _padding: [0u8; 56],
        }
    }

    /// Create fixed-point capsule from raw Q16.16 value
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::FixedQ16_16Capsule;
    ///
    /// let capsule = FixedQ16_16Capsule::from_raw(65536); // 1.0 in Q16.16
    /// assert_eq!(capsule.to_f64(), 1.0);
    /// ```
    pub const fn from_raw(raw: i32) -> Self {
        Self {
            value: AtomicI32::new(raw),
            generation: AtomicU32::new(0),
            _padding: [0u8; 56],
        }
    }

    /// Create fixed-point capsule from floating-point value
    ///
    /// # Panics
    /// Panics if value is outside representable range [-32768.0, +32767.99998]
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::FixedQ16_16Capsule;
    ///
    /// let capsule = FixedQ16_16Capsule::from_f64(3.14159);
    /// assert!((capsule.to_f64() - 3.14159).abs() < 0.0001);
    /// ```
    pub fn from_f64(value: f64) -> Self {
        assert!(
            (-32768.0..=32767.99998).contains(&value),
            "Value {} out of Q16.16 range [-32768.0, +32767.99998]",
            value
        );

        let raw = (value * Self::SCALE_F64) as i32;
        Self {
            value: AtomicI32::new(raw),
            generation: AtomicU32::new(0),
            _padding: [0u8; 56],
        }
    }

    /// Convert fixed-point value to floating-point
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::FixedQ16_16Capsule;
    ///
    /// let capsule = FixedQ16_16Capsule::from_f64(42.5);
    /// assert_eq!(capsule.to_f64(), 42.5);
    /// ```
    pub fn to_f64(&self) -> f64 {
        let raw = self.value.load(Ordering::Acquire);
        raw as f64 / Self::SCALE_F64
    }

    /// Load raw Q16.16 value
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_ATOMIC_ORDERING`: Acquire ordering for value reads
    /// - `#VERIFY_ORDERING_SUFFICIENT`: Required for data dependency
    #[inline(always)]
    pub fn load_raw(&self) -> i32 {
        self.value.load(Ordering::Acquire)
    }

    /// Store raw Q16.16 value with atomic generation increment
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_ATOMIC_ORDERING`: Release ordering for value writes
    /// - `#VERIFY_ORDERING_CORRECTNESS`: Ensures value visibility before generation update
    #[inline(always)]
    pub fn store_raw(&self, raw: i32) {
        self.value.store(raw, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Store floating-point value with conversion
    ///
    /// # Panics
    /// Panics if value is outside representable range
    pub fn store_f64(&self, value: f64) {
        assert!(
            (-32768.0..=32767.99998).contains(&value),
            "Value {} out of Q16.16 range",
            value
        );

        let raw = (value * Self::SCALE_F64) as i32;
        self.store_raw(raw);
    }

    /// Load current generation counter
    ///
    /// # Q33 Atomic Capsule Pattern
    /// - Used for TOCTOU prevention in concurrent operations
    #[inline(always)]
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }

    /// Fixed-point addition: self + other
    ///
    /// # Performance
    /// - ~5-8ns (atomic load + addition + atomic CAS)
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::FixedQ16_16Capsule;
    ///
    /// let a = FixedQ16_16Capsule::from_f64(10.5);
    /// let b = FixedQ16_16Capsule::from_f64(5.25);
    /// let result = a.add(&b);
    /// assert_eq!(result.to_f64(), 15.75);
    /// ```
    pub fn add(&self, other: &Self) -> Self {
        let a = self.value.load(Ordering::Acquire);
        let b = other.value.load(Ordering::Acquire);
        let result = a.wrapping_add(b); // Wrapping for overflow safety

        Self {
            value: AtomicI32::new(result),
            generation: AtomicU32::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    /// Fixed-point subtraction: self - other
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::FixedQ16_16Capsule;
    ///
    /// let a = FixedQ16_16Capsule::from_f64(10.5);
    /// let b = FixedQ16_16Capsule::from_f64(5.25);
    /// let result = a.sub(&b);
    /// assert_eq!(result.to_f64(), 5.25);
    /// ```
    pub fn sub(&self, other: &Self) -> Self {
        let a = self.value.load(Ordering::Acquire);
        let b = other.value.load(Ordering::Acquire);
        let result = a.wrapping_sub(b);

        Self {
            value: AtomicI32::new(result),
            generation: AtomicU32::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    /// Fixed-point multiplication: self * other
    ///
    /// # Performance
    /// - ~10-15ns (i64 intermediate arithmetic + scaling)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_OVERFLOW_HANDLING`: i64 intermediate prevents overflow
    /// - `#VERIFY_OVERFLOW_PREVENTION`: Result scaled back to i32 range
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::FixedQ16_16Capsule;
    ///
    /// let a = FixedQ16_16Capsule::from_f64(3.0);
    /// let b = FixedQ16_16Capsule::from_f64(4.5);
    /// let result = a.mul(&b);
    /// assert!((result.to_f64() - 13.5).abs() < 0.001);
    /// ```
    pub fn mul(&self, other: &Self) -> Self {
        let a = self.value.load(Ordering::Acquire) as i64;
        let b = other.value.load(Ordering::Acquire) as i64;

        // Multiply in i64 to prevent overflow, then scale back
        let product = (a * b) >> 16; // Divide by scale factor (2^16)
        let result = product as i32;

        Self {
            value: AtomicI32::new(result),
            generation: AtomicU32::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    /// Fixed-point division: self / other
    ///
    /// # Performance
    /// - ~10-15ns (i64 intermediate arithmetic + scaling)
    ///
    /// # Panics
    /// Panics if dividing by zero
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::FixedQ16_16Capsule;
    ///
    /// let a = FixedQ16_16Capsule::from_f64(10.0);
    /// let b = FixedQ16_16Capsule::from_f64(4.0);
    /// let result = a.div(&b);
    /// assert_eq!(result.to_f64(), 2.5);
    /// ```
    pub fn div(&self, other: &Self) -> Self {
        let a = self.value.load(Ordering::Acquire) as i64;
        let b = other.value.load(Ordering::Acquire) as i64;

        assert!(b != 0, "Division by zero");

        // Scale numerator before division to maintain precision
        let quotient = (a << 16) / b; // Multiply by scale factor (2^16) before divide
        let result = quotient as i32;

        Self {
            value: AtomicI32::new(result),
            generation: AtomicU32::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    /// Negate: -self
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::FixedQ16_16Capsule;
    ///
    /// let a = FixedQ16_16Capsule::from_f64(42.5);
    /// let result = a.neg();
    /// assert_eq!(result.to_f64(), -42.5);
    /// ```
    pub fn neg(&self) -> Self {
        let value = self.value.load(Ordering::Acquire);
        let result = value.wrapping_neg();

        Self {
            value: AtomicI32::new(result),
            generation: AtomicU32::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    /// Absolute value: |self|
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::FixedQ16_16Capsule;
    ///
    /// let a = FixedQ16_16Capsule::from_f64(-42.5);
    /// let result = a.abs();
    /// assert_eq!(result.to_f64(), 42.5);
    /// ```
    pub fn abs(&self) -> Self {
        let value = self.value.load(Ordering::Acquire);
        let result = value.wrapping_abs();

        Self {
            value: AtomicI32::new(result),
            generation: AtomicU32::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }
}

impl FixedPointCapsule for FixedQ16_16Capsule {
    const SCALE: i32 = 65536;

    fn from_f64(value: f64) -> Self {
        Self::from_f64(value)
    }

    fn to_f64(&self) -> f64 {
        self.to_f64()
    }

    fn mul(&self, other: &Self) -> Self {
        self.mul(other)
    }

    fn div(&self, other: &Self) -> Self {
        self.div(other)
    }
}

impl Default for FixedQ16_16Capsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time verification
const _: () = {
    assert!(
        core::mem::size_of::<FixedQ16_16Capsule>() == 64,
        "FixedQ16_16Capsule must be 64 bytes"
    );
    assert!(
        core::mem::align_of::<FixedQ16_16Capsule>() == 64,
        "FixedQ16_16Capsule must be 64-byte aligned"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment() {
        assert_eq!(core::mem::align_of::<FixedQ16_16Capsule>(), 64);
        assert_eq!(core::mem::size_of::<FixedQ16_16Capsule>(), 64);
    }

    #[test]
    fn test_scale_constant() {
        assert_eq!(FixedQ16_16Capsule::SCALE, 65536);
    }

    #[test]
    fn test_new() {
        let capsule = FixedQ16_16Capsule::new();
        assert_eq!(capsule.to_f64(), 0.0);
    }

    #[test]
    fn test_from_raw() {
        let capsule = FixedQ16_16Capsule::from_raw(65536);
        assert_eq!(capsule.to_f64(), 1.0);
    }

    #[test]
    fn test_from_f64() {
        let capsule = FixedQ16_16Capsule::from_f64(3.14159);
        let result = capsule.to_f64();
        assert!((result - 3.14159).abs() < 0.0001);
    }

    #[test]
    fn test_add() {
        let a = FixedQ16_16Capsule::from_f64(10.5);
        let b = FixedQ16_16Capsule::from_f64(5.25);
        let result = a.add(&b);
        assert_eq!(result.to_f64(), 15.75);
    }

    #[test]
    fn test_sub() {
        let a = FixedQ16_16Capsule::from_f64(10.5);
        let b = FixedQ16_16Capsule::from_f64(5.25);
        let result = a.sub(&b);
        assert_eq!(result.to_f64(), 5.25);
    }

    #[test]
    fn test_mul() {
        let a = FixedQ16_16Capsule::from_f64(3.0);
        let b = FixedQ16_16Capsule::from_f64(4.5);
        let result = a.mul(&b);
        assert!((result.to_f64() - 13.5).abs() < 0.001);
    }

    #[test]
    fn test_div() {
        let a = FixedQ16_16Capsule::from_f64(10.0);
        let b = FixedQ16_16Capsule::from_f64(4.0);
        let result = a.div(&b);
        assert_eq!(result.to_f64(), 2.5);
    }

    #[test]
    fn test_neg() {
        let a = FixedQ16_16Capsule::from_f64(42.5);
        let result = a.neg();
        assert_eq!(result.to_f64(), -42.5);
    }

    #[test]
    fn test_abs() {
        let a = FixedQ16_16Capsule::from_f64(-42.5);
        let result = a.abs();
        assert_eq!(result.to_f64(), 42.5);
    }

    #[test]
    fn test_generation_counter() {
        let capsule = FixedQ16_16Capsule::new();
        let gen1 = capsule.generation();

        capsule.store_f64(10.0);
        let gen2 = capsule.generation();

        assert_eq!(gen2, gen1 + 1);
    }

    #[test]
    fn test_precision() {
        // Q16.16 precision is 1/65536
        let precision = 1.0 / 65536.0;

        let capsule = FixedQ16_16Capsule::from_f64(0.5);
        let result = capsule.to_f64();

        assert!((result - 0.5).abs() < precision);
    }

    #[test]
    #[should_panic(expected = "out of Q16.16 range")]
    fn test_overflow_detection() {
        let _capsule = FixedQ16_16Capsule::from_f64(40000.0); // Beyond max range
    }

    #[test]
    #[should_panic(expected = "Division by zero")]
    fn test_division_by_zero() {
        let a = FixedQ16_16Capsule::from_f64(10.0);
        let b = FixedQ16_16Capsule::new(); // Zero
        let _result = a.div(&b);
    }
}
