//! Fixed-point helpers for Waterloo engine math (Q8.8/Q16.16), delegating to atomic_capsule.

pub use atomic_capsule::primitives::fixed_point::{Q16_16, Q8_8};

/// Convert f64 to Q8.8 with saturation.
#[inline(always)]
pub fn q8_from_f64(val: f64) -> Q8_8 {
    Q8_8::from_f64(val)
}

/// Convert Q8.8 to f64.
#[inline(always)]
pub fn q8_to_f64(val: Q8_8) -> f64 {
    val.to_f64()
}

/// Multiply two Q8.8 values, saturating.
#[inline(always)]
pub fn q8_mul(a: Q8_8, b: Q8_8) -> Q8_8 {
    a.saturating_mul(b)
}

/// Add two Q8.8 values, saturating.
#[inline(always)]
pub fn q8_add(a: Q8_8, b: Q8_8) -> Q8_8 {
    a.saturating_add(b)
}

/// Convert meters (f64) to Q16.16 for world positions.
#[inline(always)]
pub fn q16_from_meters(m: f64) -> Q16_16 {
    Q16_16::from_f64(m)
}

/// Convert Q16.16 back to meters.
#[inline(always)]
pub fn q16_to_meters(q: Q16_16) -> f64 {
    q.to_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q8_roundtrip() {
        let v = q8_from_f64(2.25);
        assert!((q8_to_f64(v) - 2.25).abs() < 0.01);
    }

    #[test]
    fn q8_mul_add() {
        let a = q8_from_f64(1.5);
        let b = q8_from_f64(0.25);
        let prod = q8_mul(a, b);
        assert!((q8_to_f64(prod) - 0.375).abs() < 0.01);
        let sum = q8_add(a, b);
        assert!((q8_to_f64(sum) - 1.75).abs() < 0.01);
    }

    #[test]
    fn q16_roundtrip() {
        let m = q16_from_meters(123.5);
        assert!((q16_to_meters(m) - 123.5).abs() < 0.001);
    }
}
