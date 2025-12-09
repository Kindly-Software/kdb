//! Q8.8 Fixed-Point Arithmetic
//!
//! 8-bit integer part, 8-bit fractional part
//! - Range: ±127.99609375
//! - Precision: 0.00390625 (1/256)
//! - Use case: Sub-dollar calculations, percentage tracking

/// Q8.8 Fixed-Point Type
///
/// Format: 8 integer bits, 8 fractional bits
/// Scale: 256 (2^8)
/// Range: [-128.0, 127.99609375]
/// Precision: 0.00390625 (1/256)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Q8_8 {
    raw: i16,
}

/// Scale factor for Q8.8 format
const SCALE: i32 = 256;

impl Q8_8 {
    /// Maximum representable value: 127.99609375
    pub const MAX: Self = Self { raw: i16::MAX };

    /// Minimum representable value: -128.0
    pub const MIN: Self = Self { raw: i16::MIN };

    /// Zero value
    pub const ZERO: Self = Self { raw: 0 };

    /// One (1.0)
    pub const ONE: Self = Self { raw: SCALE as i16 };

    /// Create from raw fixed-point value
    ///
    /// # Safety
    /// Raw value is in Q8.8 format (scaled by 256)
    #[inline]
    pub const fn from_raw(raw: i16) -> Self {
        Self { raw }
    }

    /// Create from pre-scaled fixed-point value
    ///
    /// Example: `Q8_8::from_fixed(100_00)` represents 100.00 (scaled by 100)
    /// This is NOT Q8.8 format - it will be converted.
    #[inline]
    pub const fn from_fixed(value: i32) -> Self {
        // Convert from base-100 to Q8.8 (base-256)
        // value is in format: 100_00 = 100.00
        // Q8.8 needs: value * 256 / 100
        let raw = ((value as i64 * SCALE as i64) / 100) as i16;
        Self { raw }
    }

    /// Convert to f64
    #[inline]
    pub fn to_f64(self) -> f64 {
        self.raw as f64 / SCALE as f64
    }

    /// Create from f64
    ///
    /// Returns None if value exceeds Q8.8 range
    #[inline]
    pub fn from_f64(value: f64) -> Option<Self> {
        let raw = (value * SCALE as f64).round();
        if raw >= i16::MIN as f64 && raw <= i16::MAX as f64 {
            Some(Self { raw: raw as i16 })
        } else {
            None
        }
    }

    /// Get raw value (Q8.8 format)
    #[inline]
    pub const fn raw(self) -> i16 {
        self.raw
    }

    // ============================================================================
    // CHECKED ARITHMETIC - Returns Option<T> on overflow
    // ============================================================================

    /// Checked addition - returns None on overflow
    ///
    /// # #ASSUME_OVERFLOW_DETECTION
    /// Caller must handle None case appropriately (e.g., reject trade, log alert)
    ///
    /// # Example
    /// ```
    /// use fixed_point_tier3::Q8_8;
    /// let a = Q8_8::from_fixed(50_00); // 50.00
    /// let b = Q8_8::from_fixed(30_00); // 30.00
    /// assert!(a.checked_add(b).is_some());
    /// ```
    #[inline]
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        // #ASSUME_OVERFLOW_DETECTION: i16 addition detects overflow
        // #VERIFY_NO_PANICS: checked_add never panics
        self.raw.checked_add(rhs.raw).map(|raw| Self { raw })
    }

    /// Checked subtraction - returns None on underflow
    ///
    /// # #ASSUME_OVERFLOW_DETECTION
    /// Caller must handle None case appropriately
    ///
    /// # Example
    /// ```
    /// use fixed_point_tier3::Q8_8;
    /// let a = Q8_8::from_fixed(50_00); // 50.00
    /// let b = Q8_8::from_fixed(30_00); // 30.00
    /// assert!(a.checked_sub(b).is_some());
    /// ```
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        // #ASSUME_OVERFLOW_DETECTION: i16 subtraction detects underflow
        // #VERIFY_NO_PANICS: checked_sub never panics
        self.raw.checked_sub(rhs.raw).map(|raw| Self { raw })
    }

    /// Checked multiplication - returns None on overflow
    ///
    /// # #ASSUME_OVERFLOW_DETECTION
    /// Multiplication requires scaling adjustment
    ///
    /// # Example
    /// ```
    /// use fixed_point_tier3::Q8_8;
    /// let a = Q8_8::from_fixed(2_00); // 2.00
    /// let b = Q8_8::from_fixed(3_00); // 3.00
    /// let result = a.checked_mul(b).unwrap();
    /// assert!((result.to_f64() - 6.0).abs() < 0.01);
    /// ```
    #[inline]
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        // #ASSUME_OVERFLOW_DETECTION: i32 multiplication detects overflow
        // #VERIFY_PRECISION_LOSS: Division by SCALE maintains Q8.8 format

        // Multiply in i32 to prevent intermediate overflow
        let result = (self.raw as i32)
            .checked_mul(rhs.raw as i32)?
            .checked_div(SCALE)?;

        if result >= i16::MIN as i32 && result <= i16::MAX as i32 {
            Some(Self { raw: result as i16 })
        } else {
            None
        }
    }

    /// Checked division - returns None on overflow or divide-by-zero
    ///
    /// # #ASSUME_OVERFLOW_DETECTION
    /// Division by zero and overflow are both detected
    ///
    /// # Example
    /// ```
    /// use fixed_point_tier3::Q8_8;
    /// let a = Q8_8::from_fixed(6_00); // 6.00
    /// let b = Q8_8::from_fixed(2_00); // 2.00
    /// let result = a.checked_div(b).unwrap();
    /// assert!((result.to_f64() - 3.0).abs() < 0.01);
    /// ```
    #[inline]
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        // #ASSUME_OVERFLOW_DETECTION: Division by zero detected
        // #VERIFY_PRECISION_LOSS: Multiply by SCALE before division maintains precision

        if rhs.raw == 0 {
            return None;
        }

        // Scale numerator to maintain precision
        let numerator = (self.raw as i32).checked_mul(SCALE)?;
        let result = numerator.checked_div(rhs.raw as i32)?;

        if result >= i16::MIN as i32 && result <= i16::MAX as i32 {
            Some(Self { raw: result as i16 })
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
    /// MAX value is acceptable limit for financial calculations
    ///
    /// # Example
    /// ```
    /// use fixed_point_tier3::Q8_8;
    /// let max = Q8_8::MAX;
    /// let one = Q8_8::from_fixed(1_00);
    /// assert_eq!(max.saturating_add(one), Q8_8::MAX);
    /// ```
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
    /// MIN value is acceptable limit for financial calculations
    ///
    /// # Example
    /// ```
    /// use fixed_point_tier3::Q8_8;
    /// let min = Q8_8::MIN;
    /// let one = Q8_8::from_fixed(1_00);
    /// assert_eq!(min.saturating_sub(one), Q8_8::MIN);
    /// ```
    #[inline]
    pub fn saturating_sub(self, rhs: Self) -> Self {
        // #ASSUME_SATURATION_CORRECTNESS: Saturation at MIN is acceptable
        // #VERIFY_NO_PANICS: saturating_sub never panics
        Self {
            raw: self.raw.saturating_sub(rhs.raw),
        }
    }

    /// Saturating multiplication - clamps result to MAX/MIN on overflow
    ///
    /// # Example
    /// ```
    /// use fixed_point_tier3::Q8_8;
    /// let a = Q8_8::from_fixed(100_00);
    /// let b = Q8_8::from_fixed(100_00);
    /// // Would overflow, saturates to MAX
    /// assert_eq!(a.saturating_mul(b), Q8_8::MAX);
    /// ```
    #[inline]
    pub fn saturating_mul(self, rhs: Self) -> Self {
        // #ASSUME_SATURATION_CORRECTNESS: Saturation is better than wrapping
        // #VERIFY_NO_PANICS: saturating_mul never panics

        let result = (self.raw as i32).saturating_mul(rhs.raw as i32) / SCALE;

        if result > i16::MAX as i32 {
            Self::MAX
        } else if result < i16::MIN as i32 {
            Self::MIN
        } else {
            Self { raw: result as i16 }
        }
    }

    /// Saturating division - clamps result on overflow (division by zero returns MIN)
    ///
    /// # Example
    /// ```
    /// use fixed_point_tier3::Q8_8;
    /// let a = Q8_8::from_fixed(6_00);
    /// let zero = Q8_8::ZERO;
    /// // Division by zero saturates to MIN
    /// assert_eq!(a.saturating_div(zero), Q8_8::MIN);
    /// ```
    #[inline]
    pub fn saturating_div(self, rhs: Self) -> Self {
        // #ASSUME_SATURATION_CORRECTNESS: Saturation on divide-by-zero
        // #VERIFY_NO_PANICS: saturating_div never panics

        if rhs.raw == 0 {
            return Self::MIN;
        }

        let numerator = (self.raw as i32).saturating_mul(SCALE);
        let result = numerator.saturating_div(rhs.raw as i32);

        if result > i16::MAX as i32 {
            Self::MAX
        } else if result < i16::MIN as i32 {
            Self::MIN
        } else {
            Self { raw: result as i16 }
        }
    }

    // ============================================================================
    // WRAPPING ARITHMETIC - Two's complement overflow (ADVANCED USAGE ONLY)
    // ============================================================================

    /// Wrapping addition - silently wraps on overflow (use with caution!)
    ///
    /// # Warning
    /// Only use if you understand two's complement wrapping semantics.
    /// For financial systems, prefer checked_add() or saturating_add().
    ///
    /// # #ASSUME_WRAPPING_INTENTIONAL
    /// Caller understands wrapping behavior and has validated it's safe
    ///
    /// # Example
    /// ```
    /// use fixed_point_tier3::Q8_8;
    /// let max = Q8_8::MAX;
    /// let one = Q8_8::from_fixed(1_00);
    /// // Wraps to negative value!
    /// assert!(max.wrapping_add(one).raw() < 0);
    /// ```
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
    /// # Warning
    /// Only use if you understand two's complement wrapping semantics.
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
    /// # Warning
    /// Only use if you understand two's complement wrapping semantics.
    ///
    /// # #ASSUME_WRAPPING_INTENTIONAL
    /// Caller understands wrapping behavior
    #[inline]
    pub fn wrapping_mul(self, rhs: Self) -> Self {
        // #ASSUME_WRAPPING_INTENTIONAL: Caller validates wrapping is safe
        // #VERIFY_NO_PANICS: wrapping_mul never panics

        let result = (self.raw as i32).wrapping_mul(rhs.raw as i32) / SCALE;
        Self { raw: result as i16 }
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

        let numerator = (self.raw as i32).wrapping_mul(SCALE);
        let result = numerator.wrapping_div(rhs.raw as i32);
        Self { raw: result as i16 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q8_8_basic_conversions() {
        let a = Q8_8::from_fixed(100_00);
        assert!((a.to_f64() - 100.0).abs() < 0.01);

        let b = Q8_8::from_f64(50.0).unwrap();
        assert!((b.to_f64() - 50.0).abs() < 0.01);
    }

    #[test]
    fn q8_8_checked_add_normal() {
        let a = Q8_8::from_fixed(50_00);
        let b = Q8_8::from_fixed(30_00);
        let result = a.checked_add(b).unwrap();
        assert!((result.to_f64() - 80.0).abs() < 0.01);
    }

    #[test]
    fn q8_8_checked_add_overflow() {
        let max = Q8_8::MAX;
        let one = Q8_8::from_fixed(1_00);
        assert_eq!(max.checked_add(one), None);
    }

    #[test]
    fn q8_8_saturating_add_max() {
        let max = Q8_8::MAX;
        let one = Q8_8::from_fixed(1_00);
        assert_eq!(max.saturating_add(one), Q8_8::MAX);
    }

    #[test]
    fn q8_8_checked_sub_underflow() {
        let min = Q8_8::MIN;
        let one = Q8_8::from_fixed(1_00);
        assert_eq!(min.checked_sub(one), None);
    }

    #[test]
    fn q8_8_saturating_sub_min() {
        let min = Q8_8::MIN;
        let one = Q8_8::from_fixed(1_00);
        assert_eq!(min.saturating_sub(one), Q8_8::MIN);
    }

    #[test]
    fn q8_8_checked_mul_precision() {
        let a = Q8_8::from_fixed(2_00);
        let b = Q8_8::from_fixed(3_00);
        let result = a.checked_mul(b).unwrap();
        assert!((result.to_f64() - 6.0).abs() < 0.01);
    }

    #[test]
    fn q8_8_checked_div_precision() {
        let a = Q8_8::from_fixed(6_00);
        let b = Q8_8::from_fixed(2_00);
        let result = a.checked_div(b).unwrap();
        assert!((result.to_f64() - 3.0).abs() < 0.01);
    }

    #[test]
    fn q8_8_wrapping_overflow() {
        let max = Q8_8::MAX;
        let one = Q8_8::from_fixed(1_00);
        // Wraps to negative
        assert!(max.wrapping_add(one).raw() < 0);
    }
}
