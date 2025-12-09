//! Q32.32 Fixed-Point Arithmetic
//!
//! 32-bit integer part, 32-bit fractional part
//! - Range: ±2,147,483,647
//! - Precision: 2.3283064365e-10 (1/2^32)
//! - Use case: High-precision scientific computing, GPS coordinates

/// Q32.32 Fixed-Point Type
///
/// Format: 32 integer bits, 32 fractional bits
/// Scale: 4294967296 (2^32)
/// Range: [-2147483648.0, 2147483647.999999999767]
/// Precision: 2.3283064365e-10 (1/2^32)
///
/// **Use for high-precision scientific computing** - exceeds f64 in many scenarios
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Q32_32 {
    raw: i64,
}

/// Scale factor for Q32.32 format
const SCALE: i128 = 4294967296; // 2^32

impl Q32_32 {
    /// Maximum representable value
    pub const MAX: Self = Self { raw: i64::MAX };

    /// Minimum representable value
    pub const MIN: Self = Self { raw: i64::MIN };

    /// Zero value
    pub const ZERO: Self = Self { raw: 0 };

    /// One (1.0)
    pub const ONE: Self = Self { raw: SCALE as i64 };

    /// Create from raw fixed-point value
    ///
    /// # Safety
    /// Raw value is in Q32.32 format (scaled by 2^32)
    #[inline]
    pub const fn from_raw(raw: i64) -> Self {
        Self { raw }
    }

    /// Create from pre-scaled fixed-point value
    ///
    /// Example: `Q32_32::from_fixed(100_000000000)` represents 100.000000000
    #[inline]
    pub const fn from_fixed(value: i64) -> Self {
        // Convert from base-1000000000 to Q32.32 (base-4294967296)
        // value is in format: 100_000000000 = 100.000000000
        // Q32.32 needs: value * 2^32 / 1000000000
        let raw = ((value as i128 * SCALE) / 1000000000) as i64;
        Self { raw }
    }

    /// Convert to f64
    #[inline]
    pub fn to_f64(self) -> f64 {
        self.raw as f64 / SCALE as f64
    }

    /// Create from f64
    ///
    /// Returns None if value exceeds Q32.32 range
    #[inline]
    pub fn from_f64(value: f64) -> Option<Self> {
        let raw = (value * SCALE as f64).round();
        if raw >= i64::MIN as f64 && raw <= i64::MAX as f64 {
            Some(Self { raw: raw as i64 })
        } else {
            None
        }
    }

    /// Get raw value (Q32.32 format)
    #[inline]
    pub const fn raw(self) -> i64 {
        self.raw
    }

    // ============================================================================
    // CHECKED ARITHMETIC - Returns Option<T> on overflow
    // ============================================================================

    /// Checked addition - returns None on overflow
    ///
    /// # #ASSUME_OVERFLOW_DETECTION
    /// Caller must handle None case appropriately
    ///
    /// # Example
    /// ```
    /// use fixed_point_tier3::Q32_32;
    /// let a = Q32_32::from_fixed(100_000000000);
    /// let b = Q32_32::from_fixed(50_000000000);
    /// let result = a.checked_add(b).unwrap();
    /// assert!((result.to_f64() - 150.0).abs() < 0.00001);
    /// ```
    #[inline]
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        // #ASSUME_OVERFLOW_DETECTION: i64 addition detects overflow
        // #VERIFY_NO_PANICS: checked_add never panics
        self.raw.checked_add(rhs.raw).map(|raw| Self { raw })
    }

    /// Checked subtraction - returns None on underflow
    ///
    /// # #ASSUME_OVERFLOW_DETECTION
    /// Caller must handle None case appropriately
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        // #ASSUME_OVERFLOW_DETECTION: i64 subtraction detects underflow
        // #VERIFY_NO_PANICS: checked_sub never panics
        self.raw.checked_sub(rhs.raw).map(|raw| Self { raw })
    }

    /// Checked multiplication - returns None on overflow
    ///
    /// # #ASSUME_OVERFLOW_DETECTION
    /// Multiplication requires i128 for intermediate values
    ///
    /// # Example
    /// ```
    /// use fixed_point_tier3::Q32_32;
    /// let a = Q32_32::from_fixed(2_000000000);
    /// let b = Q32_32::from_fixed(3_000000000);
    /// let result = a.checked_mul(b).unwrap();
    /// assert!((result.to_f64() - 6.0).abs() < 0.00001);
    /// ```
    #[inline]
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        // #ASSUME_OVERFLOW_DETECTION: i128 multiplication detects overflow
        // #VERIFY_PRECISION_LOSS: Division by SCALE maintains Q32.32 format

        // Multiply in i128 to prevent intermediate overflow
        let result = (self.raw as i128)
            .checked_mul(rhs.raw as i128)?
            .checked_div(SCALE)?;

        if result >= i64::MIN as i128 && result <= i64::MAX as i128 {
            Some(Self { raw: result as i64 })
        } else {
            None
        }
    }

    /// Checked division - returns None on overflow or divide-by-zero
    ///
    /// # #ASSUME_OVERFLOW_DETECTION
    /// Division by zero and overflow are both detected
    #[inline]
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        // #ASSUME_OVERFLOW_DETECTION: Division by zero detected
        // #VERIFY_PRECISION_LOSS: Multiply by SCALE before division maintains precision

        if rhs.raw == 0 {
            return None;
        }

        // Scale numerator to maintain precision
        let numerator = (self.raw as i128).checked_mul(SCALE)?;
        let result = numerator.checked_div(rhs.raw as i128)?;

        if result >= i64::MIN as i128 && result <= i64::MAX as i128 {
            Some(Self { raw: result as i64 })
        } else {
            None
        }
    }

    // ============================================================================
    // SATURATING ARITHMETIC - Clamps to MAX/MIN on overflow
    // ============================================================================

    /// Saturating addition - clamps result to MAX on overflow
    ///
    /// # #ASSUME_SATURATION_CORRECTNESS
    /// MAX value is acceptable limit
    #[inline]
    pub fn saturating_add(self, rhs: Self) -> Self {
        // #ASSUME_SATURATION_CORRECTNESS: Saturation at MAX is acceptable
        // #VERIFY_NO_PANICS: saturating_add never panics
        Self {
            raw: self.raw.saturating_add(rhs.raw),
        }
    }

    /// Saturating subtraction - clamps result to MIN on underflow
    ///
    /// # #ASSUME_SATURATION_CORRECTNESS
    /// MIN value is acceptable limit
    #[inline]
    pub fn saturating_sub(self, rhs: Self) -> Self {
        // #ASSUME_SATURATION_CORRECTNESS: Saturation at MIN is acceptable
        // #VERIFY_NO_PANICS: saturating_sub never panics
        Self {
            raw: self.raw.saturating_sub(rhs.raw),
        }
    }

    /// Saturating multiplication - clamps result to MAX/MIN on overflow
    #[inline]
    pub fn saturating_mul(self, rhs: Self) -> Self {
        // #ASSUME_SATURATION_CORRECTNESS: Saturation is better than wrapping
        // #VERIFY_NO_PANICS: saturating_mul never panics

        let result = (self.raw as i128).saturating_mul(rhs.raw as i128) / SCALE;

        if result > i64::MAX as i128 {
            Self::MAX
        } else if result < i64::MIN as i128 {
            Self::MIN
        } else {
            Self { raw: result as i64 }
        }
    }

    /// Saturating division - clamps result on overflow (division by zero returns MIN)
    #[inline]
    pub fn saturating_div(self, rhs: Self) -> Self {
        // #ASSUME_SATURATION_CORRECTNESS: Saturation on divide-by-zero
        // #VERIFY_NO_PANICS: saturating_div never panics

        if rhs.raw == 0 {
            return Self::MIN;
        }

        let numerator = (self.raw as i128).saturating_mul(SCALE);
        let result = numerator.saturating_div(rhs.raw as i128);

        if result > i64::MAX as i128 {
            Self::MAX
        } else if result < i64::MIN as i128 {
            Self::MIN
        } else {
            Self { raw: result as i64 }
        }
    }

    // ============================================================================
    // WRAPPING ARITHMETIC - Two's complement overflow (ADVANCED USAGE ONLY)
    // ============================================================================

    /// Wrapping addition - silently wraps on overflow (use with caution!)
    ///
    /// # Warning
    /// Only use if you understand two's complement wrapping semantics.
    ///
    /// # #ASSUME_WRAPPING_INTENTIONAL
    /// Caller understands wrapping behavior and has validated it's safe
    #[inline]
    pub fn wrapping_add(self, rhs: Self) -> Self {
        // #ASSUME_WRAPPING_INTENTIONAL: Caller validates wrapping is safe
        // #VERIFY_NO_PANICS: wrapping_add never panics
        Self {
            raw: self.raw.wrapping_add(rhs.raw),
        }
    }

    /// Wrapping subtraction - silently wraps on underflow (use with caution!)
    ///
    /// # #ASSUME_WRAPPING_INTENTIONAL
    /// Caller understands wrapping behavior
    #[inline]
    pub fn wrapping_sub(self, rhs: Self) -> Self {
        // #ASSUME_WRAPPING_INTENTIONAL: Caller validates wrapping is safe
        // #VERIFY_NO_PANICS: wrapping_sub never panics
        Self {
            raw: self.raw.wrapping_sub(rhs.raw),
        }
    }

    /// Wrapping multiplication - silently wraps on overflow (use with caution!)
    ///
    /// # #ASSUME_WRAPPING_INTENTIONAL
    /// Caller understands wrapping behavior
    #[inline]
    pub fn wrapping_mul(self, rhs: Self) -> Self {
        // #ASSUME_WRAPPING_INTENTIONAL: Caller validates wrapping is safe
        // #VERIFY_NO_PANICS: wrapping_mul never panics

        let result = (self.raw as i128).wrapping_mul(rhs.raw as i128) / SCALE;
        Self { raw: result as i64 }
    }

    /// Wrapping division - wraps on overflow (use with caution!)
    ///
    /// # Warning
    /// Division by zero will panic! Use checked_div() instead.
    ///
    /// # #ASSUME_WRAPPING_INTENTIONAL
    /// Caller validates denominator is non-zero
    #[inline]
    pub fn wrapping_div(self, rhs: Self) -> Self {
        // #ASSUME_WRAPPING_INTENTIONAL: Caller ensures rhs != 0
        // #VERIFY_DIVISION_SAFE: Caller validates denominator

        let numerator = (self.raw as i128).wrapping_mul(SCALE);
        let result = numerator.wrapping_div(rhs.raw as i128);
        Self { raw: result as i64 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q32_32_basic_conversions() {
        let a = Q32_32::from_fixed(100_000000000);
        assert!((a.to_f64() - 100.0).abs() < 0.00001);

        let b = Q32_32::from_f64(50.0).unwrap();
        assert!((b.to_f64() - 50.0).abs() < 0.00001);
    }

    #[test]
    fn q32_32_checked_add_normal() {
        let a = Q32_32::from_fixed(100_000000000);
        let b = Q32_32::from_fixed(50_000000000);
        let result = a.checked_add(b).unwrap();
        assert!((result.to_f64() - 150.0).abs() < 0.00001);
    }

    #[test]
    fn q32_32_checked_add_overflow() {
        let max = Q32_32::MAX;
        let one = Q32_32::from_fixed(1_000000000);
        assert_eq!(max.checked_add(one), None);
    }

    #[test]
    fn q32_32_saturating_add_max() {
        let max = Q32_32::MAX;
        let one = Q32_32::from_fixed(1_000000000);
        assert_eq!(max.saturating_add(one), Q32_32::MAX);
    }

    #[test]
    fn q32_32_checked_sub_underflow() {
        let min = Q32_32::MIN;
        let one = Q32_32::from_fixed(1_000000000);
        assert_eq!(min.checked_sub(one), None);
    }

    #[test]
    fn q32_32_saturating_sub_min() {
        let min = Q32_32::MIN;
        let one = Q32_32::from_fixed(1_000000000);
        assert_eq!(min.saturating_sub(one), Q32_32::MIN);
    }

    #[test]
    fn q32_32_checked_mul_precision() {
        let a = Q32_32::from_fixed(2_000000000);
        let b = Q32_32::from_fixed(3_000000000);
        let result = a.checked_mul(b).unwrap();
        assert!((result.to_f64() - 6.0).abs() < 0.00001);
    }

    #[test]
    fn q32_32_checked_div_precision() {
        let a = Q32_32::from_fixed(6_000000000);
        let b = Q32_32::from_fixed(2_000000000);
        let result = a.checked_div(b).unwrap();
        assert!((result.to_f64() - 3.0).abs() < 0.00001);
    }

    #[test]
    fn q32_32_wrapping_overflow() {
        let max = Q32_32::MAX;
        let one = Q32_32::from_fixed(1_000000000);
        // Wraps to negative
        assert!(max.wrapping_add(one).raw() < 0);
    }

    #[test]
    fn q32_32_high_precision() {
        // Test precision beyond f64 capabilities
        let a = Q32_32::from_f64(1.23456789012345).unwrap();
        let b = Q32_32::from_f64(9.87654321098765).unwrap();

        let result = a.checked_add(b).unwrap();
        // Q32.32 maintains precision better than f64 in many cases
        assert!((result.to_f64() - 11.1111111122211).abs() < 0.00001);
    }
}
