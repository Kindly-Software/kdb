//! # SimdI32x8Capsule - 8 × i32 SIMD Capsule (Hot Tier)
//!
//! **256-byte aligned SIMD capsule for integer operations (indexing, bit-packing, counters).**
//!
//! ## UCE33 Analysis
//!
//! - **Q28 (Simplicity)**: Integer load/store/compare operations
//! - **Q29 (Constraints)**: 32-byte SIMD (i32x8), 256-byte capsule
//! - **Q33 (Tier 2 SIMD)**: Parallel integer operations for indexing/bitwise ops

use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "portable_simd")]
use core::simd::{
    i32x8,
    cmp::SimdOrd,  // Provides simd_min, simd_max for integers
    num::SimdInt,  // Provides reduce_sum, reduce_min, reduce_max
};

use crate::SimdCapsule;

/// SIMD I32x8 capsule for vectorized integer operations
///
/// # Use Cases
/// - Parallel indexing (8 indices in parallel)
/// - Bit-packing operations
/// - Counter arrays
/// - Integer comparisons
#[repr(C, align(256))]
pub struct SimdI32x8Capsule {
    #[cfg(feature = "portable_simd")]
    data: i32x8,

    #[cfg(not(feature = "portable_simd"))]
    data: [i32; 8],

    generation: AtomicU64,
    _padding: [u8; 216], // 32 + 8 + 216 = 256
}

impl SimdI32x8Capsule {
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "portable_simd")]
            data: i32x8::from_array([0; 8]),
            #[cfg(not(feature = "portable_simd"))]
            data: [0; 8],
            generation: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    pub const fn from_array(data: [i32; 8]) -> Self {
        Self {
            #[cfg(feature = "portable_simd")]
            data: i32x8::from_array(data),
            #[cfg(not(feature = "portable_simd"))]
            data,
            generation: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    #[cfg(feature = "portable_simd")]
    pub fn splat(value: i32) -> Self {
        Self {
            data: i32x8::splat(value),
            generation: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn splat(value: i32) -> Self {
        Self {
            data: [value; 8],
            generation: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    // Arithmetic operations
    #[cfg(feature = "portable_simd")]
    pub fn add(&self, other: &Self) -> Self {
        Self {
            data: self.data + other.data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn add(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = self.data[i].wrapping_add(other.data[i]);
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(feature = "portable_simd")]
    pub fn simd_min(&self, other: &Self) -> Self {
        Self {
            data: self.data.simd_min(other.data),
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_min(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = self.data[i].min(other.data[i]);
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(feature = "portable_simd")]
    pub fn simd_max(&self, other: &Self) -> Self {
        Self {
            data: self.data.simd_max(other.data),
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_max(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = self.data[i].max(other.data[i]);
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    // Reduction operations
    #[cfg(feature = "portable_simd")]
    pub fn reduce_sum(&self) -> i32 {
        self.data.reduce_sum()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn reduce_sum(&self) -> i32 {
        self.data.iter().sum()
    }

    #[cfg(feature = "portable_simd")]
    pub fn reduce_min(&self) -> i32 {
        self.data.reduce_min()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn reduce_min(&self) -> i32 {
        *self.data.iter().min().unwrap_or(&i32::MAX)
    }

    #[cfg(feature = "portable_simd")]
    pub fn reduce_max(&self) -> i32 {
        self.data.reduce_max()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn reduce_max(&self) -> i32 {
        *self.data.iter().max().unwrap_or(&i32::MIN)
    }

    // Utility
    #[cfg(feature = "portable_simd")]
    pub fn to_array(&self) -> [i32; 8] {
        self.data.to_array()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn to_array(&self) -> [i32; 8] {
        self.data
    }

    /// Load data (convenience method for testing)
    pub fn load(&self) -> [i32; 8] {
        self.to_array()
    }
}

impl SimdCapsule for SimdI32x8Capsule {
    type Element = i32;
    const LANES: usize = 8;
    const ALIGNMENT: usize = 256;

    fn load_boxed(&self) -> alloc::boxed::Box<[Self::Element]> {
        alloc::boxed::Box::new(self.to_array())
    }

    fn store_slice(&mut self, data: &[Self::Element]) {
        let arr: [i32; 8] = [
            data.get(0).copied().unwrap_or(0),
            data.get(1).copied().unwrap_or(0),
            data.get(2).copied().unwrap_or(0),
            data.get(3).copied().unwrap_or(0),
            data.get(4).copied().unwrap_or(0),
            data.get(5).copied().unwrap_or(0),
            data.get(6).copied().unwrap_or(0),
            data.get(7).copied().unwrap_or(0),
        ];
        #[cfg(feature = "portable_simd")]
        {
            self.data = i32x8::from_array(arr);
        }
        #[cfg(not(feature = "portable_simd"))]
        {
            self.data = arr;
        }
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}

impl Default for SimdI32x8Capsule {
    fn default() -> Self {
        Self::new()
    }
}

const _: () = {
    assert!(core::mem::size_of::<SimdI32x8Capsule>() == 256);
    assert!(core::mem::align_of::<SimdI32x8Capsule>() == 256);
};
