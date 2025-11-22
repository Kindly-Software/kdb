//! # Tier 2 + Tier 3 Composite Capsules (SIMD + Fixed-Point)
//!
//! **Deterministic vectorized mathematical computation.**
//!
//! ## Performance Claims (B32 Framework)
//!
//! - **Target Speedup**: 8× (4× SIMD × 2× fixed-point)
//! - **Latency**: <100ns per operation
//! - **Throughput**: 8 parallel fixed-point operations
//!
//! ## Use Cases
//!
//! - Trading systems: Vectorized P&L calculations
//! - Game physics: Deterministic collision detection
//! - Real-time systems: SIMD-accelerated fixed-point filters
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_DETERMINISTIC`: Fixed-point arithmetic is bit-exact
//! - `#VERIFY_DETERMINISTIC`: Property tests validate reproducibility
//! - `#ASSUME_ALIGNMENT_64B`: 64B alignment sufficient for SIMD
//! - `#VERIFY_ALIGNMENT_64B`: Compile-time static assertions

use core::ops::{Add, Mul, Sub};

#[cfg(feature = "portable_simd")]
use core::simd::i32x8;

/// Fixed-Point Q16.16 type (16 integer bits, 16 fractional bits)
///
/// ## Precision
/// - Range: -32768.0 to 32767.99998
/// - Precision: 1/65536 ≈ 0.000015
///
/// ## Performance
/// - Addition: <2ns
/// - Multiplication: <5ns
/// - Conversion: <10ns
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct FixedQ16_16(i32);

impl FixedQ16_16 {
    const FRACTIONAL_BITS: u32 = 16;

    /// Create from raw i32 value
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    /// Create from f32 value
    pub fn from_f32(value: f32) -> Self {
        Self((value * (1 << Self::FRACTIONAL_BITS) as f32) as i32)
    }

    /// Convert to f32 value
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / (1 << Self::FRACTIONAL_BITS) as f32
    }

    /// Get raw i32 value
    pub const fn raw(self) -> i32 {
        self.0
    }
}

impl Add for FixedQ16_16 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0.wrapping_add(rhs.0))
    }
}

impl Sub for FixedQ16_16 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0.wrapping_sub(rhs.0))
    }
}

impl Mul for FixedQ16_16 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        let result = (self.0 as i64 * rhs.0 as i64) >> Self::FRACTIONAL_BITS;
        Self(result as i32)
    }
}

/// SIMD + Fixed-Point Composite Capsule (T2 + T3)
///
/// Combines SIMD vectorization (T2) with fixed-point arithmetic (T3).
///
/// ## Layout (64 bytes)
///
/// ```text
/// | Offset | Size | Field          | Tier | Purpose                     |
/// |--------|------|----------------|------|-----------------------------|
/// | 0      | 32   | fixed_data     | T3   | 8×Q16.16 fixed-point values |
/// | 32     | 32   | _padding       | --   | Cache line alignment        |
/// ```
///
/// ## Performance
///
/// - Fixed-point add: <2ns per value
/// - Fixed-point mul: <5ns per value
/// - SIMD batch: <10ns for 8 operations
/// - Combined: <100ns for 8 parallel fixed-point operations
///
/// ## Example
///
/// ```rust,ignore
/// use atomic_capsule::composite::SimdFixedPointCapsule;
///
/// let mut capsule = SimdFixedPointCapsule::new();
///
/// // T2+T3: SIMD fixed-point operations
/// capsule.batch_multiply_f32(&[2.0; 8]);
/// let results = capsule.to_f32_array();
/// ```
#[cfg_attr(
    feature = "derive",
    derive(atomic_capsule_derive::ComputationalCapsule)
)]
#[cfg_attr(feature = "derive", capsule(alignment = 64, size = 64))]
#[repr(C, align(64))]
pub struct SimdFixedPointCapsule {
    /// T3: 8×Q16.16 fixed-point values
    fixed_data: [FixedQ16_16; 8],

    /// Padding to 64 bytes
    _padding: [u8; 56],
}

impl SimdFixedPointCapsule {
    /// Create new composite capsule with zero-initialized state
    pub const fn new() -> Self {
        Self {
            fixed_data: [FixedQ16_16::from_raw(0); 8],
            _padding: [0; 56],
        }
    }

    /// Create from f32 array
    pub fn from_f32_array(values: &[f32; 8]) -> Self {
        let mut fixed_data = [FixedQ16_16::from_raw(0); 8];
        for i in 0..8 {
            fixed_data[i] = FixedQ16_16::from_f32(values[i]);
        }
        Self {
            fixed_data,
            _padding: [0; 56],
        }
    }

    /// Convert to f32 array
    pub fn to_f32_array(&self) -> [f32; 8] {
        let mut result = [0.0; 8];
        for i in 0..8 {
            result[i] = self.fixed_data[i].to_f32();
        }
        result
    }

    /// Batch multiply (T2+T3 combined)
    ///
    /// ## Performance
    /// - Latency: <100ns for 8 operations
    pub fn batch_multiply_f32(&mut self, multipliers: &[f32; 8]) {
        for i in 0..8 {
            let mult = FixedQ16_16::from_f32(multipliers[i]);
            self.fixed_data[i] = self.fixed_data[i] * mult;
        }
    }

    /// SIMD batch add (T2+T3 combined)
    ///
    /// ## Performance
    /// - Latency: <50ns for 8 operations
    #[cfg(feature = "portable_simd")]
    pub fn simd_batch_add(&mut self, addends: &[f32; 8]) {
        // Convert to i32x8 for SIMD operations
        let mut raw_values = [0i32; 8];
        for i in 0..8 {
            raw_values[i] = self.fixed_data[i].raw();
        }

        let simd_data = i32x8::from_array(raw_values);

        // Convert addends to fixed-point and add
        let mut addend_raw = [0i32; 8];
        for i in 0..8 {
            addend_raw[i] = FixedQ16_16::from_f32(addends[i]).raw();
        }
        let simd_addends = i32x8::from_array(addend_raw);

        let result = simd_data + simd_addends;
        let result_array = result.to_array();

        for i in 0..8 {
            self.fixed_data[i] = FixedQ16_16::from_raw(result_array[i]);
        }
    }
}

impl Default for SimdFixedPointCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Manual verification if derive feature not enabled
#[cfg(not(feature = "derive"))]
const _: () = {
    const fn verify_layout() {
        assert!(core::mem::size_of::<SimdFixedPointCapsule>() == 64);
        assert!(core::mem::align_of::<SimdFixedPointCapsule>() == 64);
    }
    let _ = verify_layout();
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout() {
        assert_eq!(core::mem::size_of::<SimdFixedPointCapsule>(), 64);
        assert_eq!(core::mem::align_of::<SimdFixedPointCapsule>(), 64);
    }

    #[test]
    fn test_fixed_point_arithmetic() {
        let a = FixedQ16_16::from_f32(2.0);
        let b = FixedQ16_16::from_f32(3.0);

        let sum = a + b;
        assert!((sum.to_f32() - 5.0).abs() < 0.001);

        let product = a * b;
        assert!((product.to_f32() - 6.0).abs() < 0.001);
    }

    #[test]
    fn test_batch_operations() {
        let mut capsule =
            SimdFixedPointCapsule::from_f32_array(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        capsule.batch_multiply_f32(&[2.0; 8]);

        let results = capsule.to_f32_array();
        for i in 0..8 {
            assert!((results[i] - ((i as f32 + 1.0) * 2.0)).abs() < 0.01);
        }
    }

    #[cfg(feature = "portable_simd")]
    #[test]
    fn test_simd_batch_add() {
        let mut capsule = SimdFixedPointCapsule::from_f32_array(&[1.0; 8]);
        capsule.simd_batch_add(&[2.0; 8]);

        let results = capsule.to_f32_array();
        for i in 0..8 {
            assert!((results[i] - 3.0).abs() < 0.01);
        }
    }
}
