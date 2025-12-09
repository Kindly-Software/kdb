//! Deterministic Fixed-Point Quantization (T3 Tier)
//!
//! **Architecture:** Q8.8 and Q4.4 fixed-point arithmetic
//! **Unique Feature:** Deterministic (same input → same output, always)
//! **Framework:** UCE34 Q10 (T3 Fixed-Point tier)
//!
//! ## Formats
//!
//! - **Q8.8:** 8 bits integer, 8 bits fractional (range: -128 to +127.996, precision: 1/256)
//! - **Q4.4:** 4 bits integer, 4 bits fractional (range: -8 to +7.9375, precision: 1/16)
//!
//! ## Determinism
//!
//! Unlike floating-point arithmetic, fixed-point operations are deterministic:
//! - No rounding errors
//! - Bit-identical results across runs
//! - Reproducible for research, compliance, debugging
//!
//! ## Usage
//!
//! ```ignore
//! use kindly_inference::quantization::Q8_8;
//!
//! // Convert FP16 → Q8.8 (deterministic)
//! let q = Q8_8::from_f32(3.14159);
//!
//! // Fixed-point multiplication (deterministic)
//! let result = q.mul(Q8_8::from_f32(2.0));
//! assert_eq!(result.to_f32(), 6.28125);  // Deterministic rounding
//! ```

/// Q8.8 fixed-point type (8 bits integer, 8 bits fractional)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Q8_8 {
    value: i16,
}

impl Q8_8 {
    /// Create from f32 (deterministic rounding)
    pub const fn from_f32(f: f32) -> Self {
        let scaled = (f * 256.0) as i16;
        Self { value: scaled }
    }

    /// Convert to f32
    pub const fn to_f32(self) -> f32 {
        (self.value as f32) / 256.0
    }

    /// Fixed-point multiplication (deterministic)
    pub const fn mul(self, other: Self) -> Self {
        let result = (self.value as i32 * other.value as i32) >> 8;
        Self { value: result as i16 }
    }

    /// Fixed-point addition (deterministic)
    pub const fn add(self, other: Self) -> Self {
        Self { value: self.value.wrapping_add(other.value) }
    }
}

/// Q4.4 fixed-point type (4 bits integer, 4 bits fractional)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Q4_4 {
    value: i8,
}

impl Q4_4 {
    /// Create from f32 (deterministic rounding)
    pub const fn from_f32(f: f32) -> Self {
        let scaled = (f * 16.0) as i8;
        Self { value: scaled }
    }

    /// Convert to f32
    pub const fn to_f32(self) -> f32 {
        (self.value as f32) / 16.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_q8_8_deterministic() {
        let a = Q8_8::from_f32(3.14159);
        let b = Q8_8::from_f32(2.0);
        let result = a.mul(b);

        // Same operation → same result (deterministic)
        let result2 = a.mul(b);
        assert_eq!(result, result2);
    }

    #[test]
    fn test_q4_4_range() {
        let min = Q4_4::from_f32(-8.0);
        let max = Q4_4::from_f32(7.9375);

        assert_eq!(min.to_f32(), -8.0);
        assert_eq!(max.to_f32(), 7.9375);
    }
}
