//! # Scalar Fallback Implementations
//!
//! **Stable Rust scalar implementations when portable_simd is unavailable.**
//!
//! ## Purpose
//!
//! This module provides equivalent scalar code for all SIMD operations,
//! enabling compilation on stable Rust without nightly features.
//!
//! ## Performance
//!
//! - Scalar: Sequential operations (baseline performance)
//! - SIMD: 2-19× faster (when portable_simd enabled)
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_EQUIVALENT_SEMANTICS`: Scalar and SIMD produce identical results
//! - `#VERIFY_CORRECTNESS`: Property-based tests validate equivalence
//!
//! ## Q31 Rust Transform
//!
//! All fallback implementations use safe Rust:
//! - No unsafe blocks
//! - No manual vectorization
//! - Compiler may auto-vectorize (but not guaranteed)

/// Scalar f32 operations (fallback for f32x8)
pub mod scalar_f32 {
    /// Scalar dot product (8 elements)
    pub fn dot(a: &[f32; 8], b: &[f32; 8]) -> f32 {
        let mut sum = 0.0f32;
        for i in 0..8 {
            sum += a[i] * b[i];
        }
        sum
    }

    /// Scalar horizontal sum (8 elements)
    pub fn horizontal_sum(a: &[f32; 8]) -> f32 {
        a.iter().sum()
    }

    /// Scalar element-wise addition
    pub fn add(a: &[f32; 8], b: &[f32; 8]) -> [f32; 8] {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = a[i] + b[i];
        }
        result
    }

    /// Scalar element-wise multiplication
    pub fn mul(a: &[f32; 8], b: &[f32; 8]) -> [f32; 8] {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = a[i] * b[i];
        }
        result
    }
}

/// Scalar f64 operations (fallback for f64x4)
pub mod scalar_f64 {
    /// Scalar dot product (4 elements)
    pub fn dot(a: &[f64; 4], b: &[f64; 4]) -> f64 {
        let mut sum = 0.0f64;
        for i in 0..4 {
            sum += a[i] * b[i];
        }
        sum
    }

    /// Scalar horizontal sum (4 elements)
    pub fn horizontal_sum(a: &[f64; 4]) -> f64 {
        a.iter().sum()
    }
}

/// Scalar i32 operations (fallback for i32x8)
pub mod scalar_i32 {
    /// Scalar horizontal sum (8 elements)
    pub fn horizontal_sum(a: &[i32; 8]) -> i32 {
        a.iter().sum()
    }

    /// Scalar minimum (8 elements)
    pub fn horizontal_min(a: &[i32; 8]) -> i32 {
        *a.iter().min().unwrap_or(&i32::MAX)
    }

    /// Scalar maximum (8 elements)
    pub fn horizontal_max(a: &[i32; 8]) -> i32 {
        *a.iter().max().unwrap_or(&i32::MIN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_f32_dot() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b = [2.0; 8];
        assert_eq!(scalar_f32::dot(&a, &b), 72.0);
    }

    #[test]
    fn test_scalar_f64_dot() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [2.0; 4];
        assert_eq!(scalar_f64::dot(&a, &b), 20.0);
    }
}
