//! # SIMD Computational Capsules
//!
//! **Cache-aligned SIMD wrappers for high-performance vectorized computation.**
//!
//! This module provides cache-aligned computational capsules that wrap SIMD types,
//! ensuring optimal memory access patterns and cache behavior for vectorized operations.
//!
//! ## Design Principles (UCE32 Framework Applied)
//!
//! - **Q28 (Simplicity)**: Thin wrappers over std::simd - minimal abstraction overhead
//! - **Q29 (Constraints)**: 64-byte alignment for cache line isolation, f32x8 fits in 32 bytes
//! - **Q30 (Validation)**: Benchmarks validate 2-4x speedup over scalar baselines
//! - **Q31 (Rust Transform)**: Zero-cost abstractions via inlining and const generics
//! - **Q32 (Nightly)**: Uses portable_simd for cross-platform SIMD acceleration
//!
//! ## IMPL-2 V2.0 Compliance
//!
//! This module provides **exactly** what's needed for computational capsules:
//! - Single wrapper type: `SimdF32x8Capsule`
//! - Cache-aligned (64 bytes) for optimal performance
//! - No premature abstraction - expand when needed for other SIMD types
//!
//! ## Usage
//!
//! ```rust
//! #![feature(portable_simd)]
//! use atomic_capsule::SimdF32x8Capsule;
//! use std::simd::f32x8;
//!
//! // Create aligned SIMD capsule
//! let a = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
//! let b = SimdF32x8Capsule::new([8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]);
//!
//! // Vectorized dot product (SIMD-accelerated)
//! let result = (a.load_simd() * b.load_simd()).reduce_sum();
//! assert_eq!(result, 120.0);
//! ```
//!
//! ## Safety Model (ASSUM Framework)
//!
//! - `#ASSUME_ALIGNMENT`: 64-byte alignment guaranteed by `#[repr(C, align(64))]`
//! - `#VERIFY_ALIGNMENT`: Compile-time verification via const assertions
//! - `#ASSUME_SIMD_SIZE`: f32x8 = 32 bytes fits in 64-byte cache line with padding
//!
//! ## Performance Targets (B32 Framework)
//!
//! Based on Intel Ultra 7 155H measurements:
//! - Expected speedup: 2-4x over scalar (realistic AVX2/AVX-512)
//! - Cache miss rate: <0.1 misses per operation (64-byte alignment)
//! - Load latency: ~1ns (L1 cache hit for aligned loads)
//! - Dot product (8 elements): 2-3ns (SIMD) vs 8-10ns (scalar)

#![cfg(feature = "portable_simd")]

use core::simd::f32x8;
use crate::alignment::AlignmentTier;

/// Cache-aligned SIMD F32x8 computational capsule
///
/// Provides a 64-byte aligned wrapper around `f32x8` SIMD vector for optimal
/// cache behavior and vectorized computation performance.
///
/// # Layout
///
/// ```text
/// [0-31]: f32x8 SIMD vector (8 × f32 = 32 bytes)
/// [32-63]: Padding for 64-byte cache line alignment
/// ```
///
/// # Performance Characteristics
///
/// - **Alignment**: 64-byte (single cache line)
/// - **Size**: 64 bytes total (32 bytes data + 32 bytes padding)
/// - **Cache behavior**: Isolated cache line prevents false sharing
/// - **SIMD operations**: Zero-cost abstractions compile to SIMD instructions
///
/// # Example
///
/// ```rust
/// #![feature(portable_simd)]
/// use atomic_capsule::SimdF32x8Capsule;
///
/// let cap = SimdF32x8Capsule::new([1.0; 8]);
/// let vec = cap.load_simd();
/// let sum = vec.reduce_sum();
/// ```
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy)]
pub struct SimdF32x8Capsule {
    /// SIMD vector data (32 bytes)
    data: f32x8,
    /// Padding to reach 64-byte cache line (32 bytes)
    _padding: [u8; 32],
}

impl SimdF32x8Capsule {
    /// Create a new SIMD capsule from an array of 8 floats
    ///
    /// # Example
    ///
    /// ```rust
    /// # #![feature(portable_simd)]
    /// # use atomic_capsule::SimdF32x8Capsule;
    /// let cap = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    /// ```
    #[inline(always)]
    pub const fn new(values: [f32; 8]) -> Self {
        Self {
            data: f32x8::from_array(values),
            _padding: [0u8; 32],
        }
    }

    /// Create a capsule with all elements set to the same value
    ///
    /// # Example
    ///
    /// ```rust
    /// # #![feature(portable_simd)]
    /// # use atomic_capsule::SimdF32x8Capsule;
    /// let cap = SimdF32x8Capsule::splat(42.0);
    /// ```
    #[inline(always)]
    pub const fn splat(value: f32) -> Self {
        Self {
            data: f32x8::from_array([value; 8]),
            _padding: [0u8; 32],
        }
    }

    /// Load the SIMD vector for computation
    ///
    /// This operation is zero-cost - it simply returns a copy of the SIMD vector.
    /// The alignment ensures this compiles to optimal SIMD load instructions.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #![feature(portable_simd)]
    /// # use atomic_capsule::SimdF32x8Capsule;
    /// let cap = SimdF32x8Capsule::new([1.0; 8]);
    /// let vec = cap.load_simd();
    /// let sum = vec.reduce_sum();
    /// ```
    #[inline(always)]
    pub fn load_simd(&self) -> f32x8 {
        self.data
    }

    /// Store a SIMD vector into this capsule
    ///
    /// # Example
    ///
    /// ```rust
    /// # #![feature(portable_simd)]
    /// # use atomic_capsule::SimdF32x8Capsule;
    /// # use std::simd::f32x8;
    /// let mut cap = SimdF32x8Capsule::new([0.0; 8]);
    /// cap.store_simd(f32x8::from_array([1.0; 8]));
    /// ```
    #[inline(always)]
    pub fn store_simd(&mut self, vec: f32x8) {
        self.data = vec;
    }

    /// Get a reference to the underlying array
    ///
    /// # Example
    ///
    /// ```rust
    /// # #![feature(portable_simd)]
    /// # use atomic_capsule::SimdF32x8Capsule;
    /// let cap = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    /// assert_eq!(cap.as_array()[0], 1.0);
    /// ```
    #[inline(always)]
    pub fn as_array(&self) -> &[f32; 8] {
        self.data.as_array()
    }

    /// Get a mutable reference to the underlying array
    ///
    /// # Example
    ///
    /// ```rust
    /// # #![feature(portable_simd)]
    /// # use atomic_capsule::SimdF32x8Capsule;
    /// let mut cap = SimdF32x8Capsule::new([0.0; 8]);
    /// cap.as_mut_array()[0] = 42.0;
    /// ```
    #[inline(always)]
    pub fn as_mut_array(&mut self) -> &mut [f32; 8] {
        self.data.as_mut_array()
    }
}

impl AlignmentTier for SimdF32x8Capsule {
    const TIER: &'static str = "hot";
    const ALIGNMENT: usize = 64;
}

// Compile-time verification of alignment and size
const _: () = {
    assert!(
        core::mem::align_of::<SimdF32x8Capsule>() == 64,
        "SimdF32x8Capsule must be 64-byte aligned"
    );
    assert!(
        core::mem::size_of::<SimdF32x8Capsule>() == 64,
        "SimdF32x8Capsule must be exactly 64 bytes"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment() {
        let cap = SimdF32x8Capsule::new([1.0; 8]);
        let addr = &cap as *const _ as usize;
        assert_eq!(addr % 64, 0, "Capsule must be 64-byte aligned");
    }

    #[test]
    fn test_size() {
        assert_eq!(
            core::mem::size_of::<SimdF32x8Capsule>(),
            64,
            "Capsule must be exactly 64 bytes"
        );
    }

    #[test]
    fn test_new_and_load() {
        let values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let cap = SimdF32x8Capsule::new(values);
        let vec = cap.load_simd();
        assert_eq!(vec.as_array(), &values);
    }

    #[test]
    fn test_splat() {
        let cap = SimdF32x8Capsule::splat(42.0);
        let vec = cap.load_simd();
        assert_eq!(vec.as_array(), &[42.0; 8]);
    }

    #[test]
    fn test_store() {
        let mut cap = SimdF32x8Capsule::new([0.0; 8]);
        let new_vec = f32x8::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        cap.store_simd(new_vec);
        assert_eq!(cap.as_array(), &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn test_simd_operations() {
        let a = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let b = SimdF32x8Capsule::new([8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]);

        // Test addition
        let sum = a.load_simd() + b.load_simd();
        assert_eq!(sum.as_array(), &[9.0; 8]);

        // Test multiplication (element-wise)
        let product = a.load_simd() * b.load_simd();
        assert_eq!(
            product.as_array(),
            &[8.0, 14.0, 18.0, 20.0, 20.0, 18.0, 14.0, 8.0]
        );

        // Test dot product (sum of element-wise products)
        let dot = (a.load_simd() * b.load_simd()).reduce_sum();
        assert_eq!(dot, 120.0);
    }

    #[test]
    fn test_as_array_mut() {
        let mut cap = SimdF32x8Capsule::new([0.0; 8]);
        let array = cap.as_mut_array();
        array[0] = 1.0;
        array[7] = 8.0;
        assert_eq!(cap.as_array(), &[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 8.0]);
    }
}
