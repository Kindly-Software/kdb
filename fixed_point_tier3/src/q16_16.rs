//! Q16.16 Fixed-Point Arithmetic
//!
//! 16-bit integer part, 16-bit fractional part
//! - Range: ±32767.999985
//! - Precision: 0.000015259 (1/65536)
//! - Use case: Most financial applications, regulatory compliance

/// Q16.16 Fixed-Point Type
///
/// Format: 16 integer bits, 16 fractional bits
/// Scale: 65536 (2^16)
/// Range: [-32768.0, 32767.999985]
/// Precision: 0.000015259 (1/65536)
///
/// **Recommended for most financial applications** - best balance of range and precision
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Q16_16 {
    raw: i32,
}

/// Scale factor for Q16.16 format
const SCALE: i64 = 65536;

impl Q16_16 {
    /// Maximum representable value: 32767.999985
    pub const MAX: Self = Self { raw: i32::MAX };

    /// Minimum representable value: -32768.0
    pub const MIN: Self = Self { raw: i32::MIN };

    /// Zero value
    pub const ZERO: Self = Self { raw: 0 };

    /// One (1.0)
    pub const ONE: Self = Self { raw: SCALE as i32 };

    /// Create from raw fixed-point value
    ///
    /// # Safety
    /// Raw value is in Q16.16 format (scaled by 65536)
    #[inline]
    pub const fn from_raw(raw: i32) -> Self {
        Self { raw }
    }

    /// Create from pre-scaled fixed-point value
    ///
    /// Example: `Q16_16::from_fixed(100_0000)` represents 100.0000
    #[inline]
    pub const fn from_fixed(value: i64) -> Self {
        // Convert from base-10000 to Q16.16 (base-65536)
        // value is in format: 100_0000 = 100.0000
        // Q16.16 needs: value * 65536 / 10000
        let raw = ((value * SCALE) / 10000) as i32;
        Self { raw }
    }

    /// Convert to f64
    #[inline]
    pub fn to_f64(self) -> f64 {
        self.raw as f64 / SCALE as f64
    }

    /// Create from f64
    ///
    /// Returns None if value exceeds Q16.16 range
    #[inline]
    pub fn from_f64(value: f64) -> Option<Self> {
        let raw = (value * SCALE as f64).round();
        if raw >= i32::MIN as f64 && raw <= i32::MAX as f64 {
            Some(Self { raw: raw as i32 })
        } else {
            None
        }
    }

    /// Get raw value (Q16.16 format)
    #[inline]
    pub const fn raw(self) -> i32 {
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
    /// use fixed_point_tier3::Q16_16;
    /// let a = Q16_16::from_fixed(100_0000); // 100.0000
    /// let b = Q16_16::from_fixed(50_0000);  // 50.0000
    /// let result = a.checked_add(b).unwrap();
    /// assert!((result.to_f64() - 150.0).abs() < 0.0001);
    /// ```
    #[inline]
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        // #ASSUME_OVERFLOW_DETECTION: i32 addition detects overflow
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
    /// use fixed_point_tier3::Q16_16;
    /// let a = Q16_16::from_fixed(100_0000);
    /// let b = Q16_16::from_fixed(50_0000);
    /// let result = a.checked_sub(b).unwrap();
    /// assert!((result.to_f64() - 50.0).abs() < 0.0001);
    /// ```
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        // #ASSUME_OVERFLOW_DETECTION: i32 subtraction detects underflow
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
    /// use fixed_point_tier3::Q16_16;
    /// let a = Q16_16::from_fixed(2_0000); // 2.0000
    /// let b = Q16_16::from_fixed(3_0000); // 3.0000
    /// let result = a.checked_mul(b).unwrap();
    /// assert!((result.to_f64() - 6.0).abs() < 0.0001);
    /// ```
    #[inline]
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        // #ASSUME_OVERFLOW_DETECTION: i64 multiplication detects overflow
        // #VERIFY_PRECISION_LOSS: Division by SCALE maintains Q16.16 format

        // Multiply in i64 to prevent intermediate overflow
        let result = (self.raw as i64)
            .checked_mul(rhs.raw as i64)?
            .checked_div(SCALE)?;

        if result >= i32::MIN as i64 && result <= i32::MAX as i64 {
            Some(Self { raw: result as i32 })
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
    /// use fixed_point_tier3::Q16_16;
    /// let a = Q16_16::from_fixed(6_0000); // 6.0000
    /// let b = Q16_16::from_fixed(2_0000); // 2.0000
    /// let result = a.checked_div(b).unwrap();
    /// assert!((result.to_f64() - 3.0).abs() < 0.0001);
    /// ```
    #[inline]
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        // #ASSUME_OVERFLOW_DETECTION: Division by zero detected
        // #VERIFY_PRECISION_LOSS: Multiply by SCALE before division maintains precision

        if rhs.raw == 0 {
            return None;
        }

        // Scale numerator to maintain precision
        let numerator = (self.raw as i64).checked_mul(SCALE)?;
        let result = numerator.checked_div(rhs.raw as i64)?;

        if result >= i32::MIN as i64 && result <= i32::MAX as i64 {
            Some(Self { raw: result as i32 })
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
    /// use fixed_point_tier3::Q16_16;
    /// let max = Q16_16::MAX;
    /// let one = Q16_16::from_fixed(1_0000);
    /// assert_eq!(max.saturating_add(one), Q16_16::MAX);
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
    /// use fixed_point_tier3::Q16_16;
    /// let min = Q16_16::MIN;
    /// let one = Q16_16::from_fixed(1_0000);
    /// assert_eq!(min.saturating_sub(one), Q16_16::MIN);
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
    /// use fixed_point_tier3::Q16_16;
    /// let a = Q16_16::from_fixed(100000_0000);
    /// let b = Q16_16::from_fixed(100000_0000);
    /// // Would overflow, saturates to MAX
    /// assert_eq!(a.saturating_mul(b), Q16_16::MAX);
    /// ```
    #[inline]
    pub fn saturating_mul(self, rhs: Self) -> Self {
        // #ASSUME_SATURATION_CORRECTNESS: Saturation is better than wrapping
        // #VERIFY_NO_PANICS: saturating_mul never panics

        let result = (self.raw as i64).saturating_mul(rhs.raw as i64) / SCALE;

        if result > i32::MAX as i64 {
            Self::MAX
        } else if result < i32::MIN as i64 {
            Self::MIN
        } else {
            Self { raw: result as i32 }
        }
    }

    /// Saturating division - clamps result on overflow (division by zero returns MIN)
    ///
    /// # Example
    /// ```
    /// use fixed_point_tier3::Q16_16;
    /// let a = Q16_16::from_fixed(6_0000);
    /// let zero = Q16_16::ZERO;
    /// // Division by zero saturates to MIN
    /// assert_eq!(a.saturating_div(zero), Q16_16::MIN);
    /// ```
    #[inline]
    pub fn saturating_div(self, rhs: Self) -> Self {
        // #ASSUME_SATURATION_CORRECTNESS: Saturation on divide-by-zero
        // #VERIFY_NO_PANICS: saturating_div never panics

        if rhs.raw == 0 {
            return Self::MIN;
        }

        let numerator = (self.raw as i64).saturating_mul(SCALE);
        let result = numerator.saturating_div(rhs.raw as i64);

        if result > i32::MAX as i64 {
            Self::MAX
        } else if result < i32::MIN as i64 {
            Self::MIN
        } else {
            Self { raw: result as i32 }
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
    /// use fixed_point_tier3::Q16_16;
    /// let max = Q16_16::MAX;
    /// let one = Q16_16::from_fixed(1_0000);
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

        let result = (self.raw as i64).wrapping_mul(rhs.raw as i64) / SCALE;
        Self { raw: result as i32 }
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

        let numerator = (self.raw as i64).wrapping_mul(SCALE);
        let result = numerator.wrapping_div(rhs.raw as i64);
        Self { raw: result as i32 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q16_16_basic_conversions() {
        let a = Q16_16::from_fixed(100_0000);
        assert!((a.to_f64() - 100.0).abs() < 0.0001);

        let b = Q16_16::from_f64(50.0).unwrap();
        assert!((b.to_f64() - 50.0).abs() < 0.0001);
    }

    #[test]
    fn q16_16_checked_add_normal() {
        let a = Q16_16::from_fixed(100_0000);
        let b = Q16_16::from_fixed(50_0000);
        let result = a.checked_add(b).unwrap();
        assert!((result.to_f64() - 150.0).abs() < 0.0001);
    }

    #[test]
    fn q16_16_checked_add_overflow() {
        let max = Q16_16::MAX;
        let one = Q16_16::from_fixed(1_0000);
        assert_eq!(max.checked_add(one), None);
    }

    #[test]
    fn q16_16_saturating_add_max() {
        let max = Q16_16::MAX;
        let one = Q16_16::from_fixed(1_0000);
        assert_eq!(max.saturating_add(one), Q16_16::MAX);
    }

    #[test]
    fn q16_16_checked_sub_underflow() {
        let min = Q16_16::MIN;
        let one = Q16_16::from_fixed(1_0000);
        assert_eq!(min.checked_sub(one), None);
    }

    #[test]
    fn q16_16_saturating_sub_min() {
        let min = Q16_16::MIN;
        let one = Q16_16::from_fixed(1_0000);
        assert_eq!(min.saturating_sub(one), Q16_16::MIN);
    }

    #[test]
    fn q16_16_checked_mul_precision() {
        let a = Q16_16::from_fixed(2_0000);
        let b = Q16_16::from_fixed(3_0000);
        let result = a.checked_mul(b).unwrap();
        assert!((result.to_f64() - 6.0).abs() < 0.0001);
    }

    #[test]
    fn q16_16_checked_div_precision() {
        let a = Q16_16::from_fixed(6_0000);
        let b = Q16_16::from_fixed(2_0000);
        let result = a.checked_div(b).unwrap();
        assert!((result.to_f64() - 3.0).abs() < 0.0001);
    }

    #[test]
    fn q16_16_wrapping_overflow() {
        let max = Q16_16::MAX;
        let one = Q16_16::from_fixed(1_0000);
        // Wraps to negative
        assert!(max.wrapping_add(one).raw() < 0);
    }

    #[test]
    fn q16_16_financial_precision() {
        // Test typical financial calculation: price * quantity - fee
        // Note: Q16.16 range is ±32767, so use smaller values
        let price = Q16_16::from_f64(123.4567).unwrap();
        let quantity = Q16_16::from_fixed(100_0000); // 100 shares (not 1000, exceeds range)
        let fee = Q16_16::from_f64(0.10).unwrap();

        let trade_value = price.checked_mul(quantity).unwrap();
        let net = trade_value.checked_sub(fee).unwrap();

        // Expected: 123.4567 * 100 - 0.10 = 12345.57
        assert!((net.to_f64() - 12345.57).abs() < 0.01);
    }
}
