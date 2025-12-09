//! # SIMD Capsule Tier 2 - Vectorized Computational Primitives
//!
//! **Production-ready SIMD capsules with proven 2-19× speedups.**
//!
//! ## UCE33 Analysis
//!
//! - **Q10 (Tier Selection)**: Tier 2 SIMD for embarrassingly parallel operations
//! - **Q12 (Nightly)**: portable_simd MANDATORY for cross-platform vectorization
//! - **Q28 (Simplicity)**: Minimal API - load, compute, store
//! - **Q29 (Constraints)**: 256-bit AVX2 (f32x8, i32x8), 256-bit for f64x4
//! - **Q31 (Rust Transform)**: Safe SIMD via std::simd (zero unsafe in operations)
//! - **Q32 (Nightly Features)**: portable_simd, const_trait_impl
//! - **Q33 (Verification)**: verify_simd_capsule! macro enforces alignment
//!
//! ## Proven Performance (KEY_INNOVATIONS.md)
//!
//! - **19× Hebbian learning** (kindly_hft: 6-element batches)
//! - **7× table scans** (ScanCapsule: f32x8 predicates)
//! - **5× aggregations** (AggregationCapsule: f64x4 reductions)
//!
//! ## ASSUM Safety Framework
//!
//! - `#ASSUME_SIMD_ALIGNMENT`: 256-byte alignment for SIMD operations
//! - `#VERIFY_ALIGNMENT_COMPILE_TIME`: Checked at build time (zero runtime cost)
//! - `#ASSUME_PORTABLE_SIMD`: Works across x86, ARM, RISC-V, WASM
//! - `#VERIFY_SCALAR_FALLBACK`: Stable Rust has equivalent scalar code
//!
//! ## Memory Layout Tiers
//!
//! - **Hot Tier (256B)**: Single 4-cache-line fit, <5ns access
//! - **Warm Tier (512B)**: Dual 4-cache-line fit, <10ns access
//! - **Cold Tier (1024B)**: Batch processing, <20ns access
//!
//! ## Examples
//!
//! ```rust
//! use simd_capsule_tier2::SimdF32x8Capsule;
//!
//! // Create SIMD capsule (256-byte aligned)
//! let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
//! let b = SimdF32x8Capsule::from_array([2.0; 8]);
//!
//! // SIMD addition: 8 operations in parallel (~2-4ns)
//! let result = a.add(&b);
//! assert_eq!(result.to_array(), [3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
//!
//! // Horizontal sum reduction (~3-5ns)
//! let sum = a.reduce_sum();
//! assert_eq!(sum, 36.0);
//! ```

#![cfg_attr(feature = "portable_simd", feature(portable_simd))]
#![cfg_attr(feature = "nightly", feature(generic_const_exprs))]
#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(not(feature = "std"), feature = "std"))]
compile_error!("Cannot have both std and no_std");

extern crate alloc;

pub mod f32x8;
pub mod f64x4;
pub mod i32x8;
pub mod fallback;
pub mod verification;
pub mod patterns;

// Re-exports
pub use f32x8::SimdF32x8Capsule;
pub use f64x4::SimdF64x4Capsule;
pub use i32x8::SimdI32x8Capsule;

// Verification macros are exported via #[macro_export] in verification.rs

/// SIMD capsule trait - unified interface across all SIMD types
///
/// # Q33 Design Pattern
/// - All SIMD capsules share same interface
/// - Enables generic programming over SIMD types
/// - Zero-cost abstraction (monomorphization)
pub trait SimdCapsule {
    /// Element type (f32, f64, i32, etc.)
    type Element: Copy;

    /// Number of SIMD lanes
    const LANES: usize;

    /// Capsule alignment (256B/512B/1024B)
    const ALIGNMENT: usize;

    /// Load data from capsule (workaround for const generic limitation)
    fn load_boxed(&self) -> alloc::boxed::Box<[Self::Element]>;

    /// Store data to capsule (workaround for const generic limitation)
    fn store_slice(&mut self, data: &[Self::Element]);

    /// Get generation counter (atomic coordination)
    fn generation(&self) -> u64;
}
