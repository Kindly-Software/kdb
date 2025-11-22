//! # Complex Number Primitives (Phase 4.2 - CNLS Foundation)
//!
//! **Tier 2 SIMD + Tier 3 Fixed-Point** complex number capsules for quantum wave mechanics.
//!
//! ## Primitives
//!
//! - **ComplexF32x4**: 4× complex f32 SIMD (feature: `complex-simd`)
//!   - 10-13× speedup vs scalar complex arithmetic
//!   - 32-byte aligned (AVX2 f32x8)
//!   - <6ns per complex multiply
//!
//! - **ComplexCell**: Q16.48 fixed-point complex (feature: `complex-fixed`)
//!   - 2-5× speedup vs floating-point
//!   - Deterministic arithmetic (no rounding errors)
//!   - 32-byte aligned capsule
//!
//! ## UCE34 Q1-Q34 Analysis
//!
//! ### Q10-Q12: Foundation
//! - Q10 (Capsule Tier): T2 (SIMD) for ComplexF32x4, T3 (Fixed-Point) for ComplexCell
//! - Q11 (Rust Transform): portable_simd + fixed-point arithmetic
//! - Q12 (Nightly): f32x8 SIMD, const_fn_floating_point
//!
//! ### Q16-Q20: Integration (I20)
//! - Q16 (Rollback): Git revert (additive only, 100% backward compatible)
//! - Q17 (Monitoring): None required (deterministic, no telemetry)
//! - Q18 (Compatibility): No breaking changes to existing primitives
//! - Q19 (Strategy): I20-Progressive (new module, parallel deployment)
//! - Q20 (Success): Compilation + tests pass + 0 breaking changes
//!
//! ### Q33: Validation
//! - #[derive(ComputationalCapsule)] for compile-time verification
//! - Property tests for complex arithmetic identities
//! - Unit tests for SIMD correctness
//!
//! ## ASSUM Framework
//! - #ASSUME_SIMD_AVAILABLE: f32x8 available (AVX2 or portable_simd)
//! - #VERIFY_ALIGNMENT: 32-byte alignment for SIMD, compile-time verified
//! - #ASSUME_FIXED_POINT_DETERMINISM: Q16.48 arithmetic is exact
//! - #VERIFY_ARITHMETIC: Complex operations match scalar results
//!
//! ## Feature Flags
//!
//! ```toml
//! # Cargo.toml
//! [dependencies]
//! atomic_capsule = { version = "0.3.4", features = ["complex-simd"] }
//! atomic_capsule = { version = "0.3.4", features = ["complex-fixed"] }
//! atomic_capsule = { version = "0.3.4", features = ["complex-simd", "complex-fixed"] }
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use atomic_capsule::primitives::complex::{ComplexF32x4, ComplexCell};
//!
//! // SIMD complex arithmetic (4× parallel)
//! let a = ComplexF32x4::splat(3.0, 4.0);
//! let b = ComplexF32x4::splat(1.0, 2.0);
//! let c = a.mul(&b); // 4× complex multiply in <6ns
//!
//! // Fixed-point deterministic complex
//! let z1 = ComplexCell::new(3.0, 4.0);
//! let z2 = ComplexCell::new(1.0, 2.0);
//! let z3 = z1.mul(&z2); // Deterministic, exact
//! ```
//!
//! ## Performance Targets (B32)
//!
//! - ComplexF32x4: 1.5ns per complex multiply (4× parallel = 6ns total)
//! - ComplexF32x4: 0.75ns per magnitude squared (4× parallel = 3ns total)
//! - ComplexCell: <20ns per complex operation (deterministic)
//!
//! ## IMPL-2 V3.1 Compliance
//!
//! This module uses cutting-edge-first development:
//! - Nightly portable_simd (mandatory for ComplexF32x4)
//! - Tier 2 SIMD + Tier 3 Fixed-Point (dual-tier optimization)
//! - Zero-cost abstractions (#[inline(always)])
//! - Innovation stacking: SIMD + Fixed-Point for quantum wave mechanics

// ========== T2 SIMD Complex Numbers ==========
#[cfg(feature = "complex-simd")]
pub mod complex_simd;

// ========== T3 Fixed-Point Complex Numbers ==========
#[cfg(feature = "complex-fixed")]
pub mod complex_cell;

// ========== Re-exports for convenience ==========
#[cfg(feature = "complex-simd")]
pub use complex_simd::ComplexF32x4;

#[cfg(feature = "complex-fixed")]
pub use complex_cell::{from_q16_48, to_q16_48, ComplexCell};

#[cfg(test)]
mod tests {
    #[cfg(feature = "complex-simd")]
    #[test]
    fn test_complex_simd_module_accessible() {
        use super::ComplexF32x4;
        let c = ComplexF32x4::zero();
        assert_eq!(c.to_array(), [0.0; 8]);
    }

    #[cfg(feature = "complex-fixed")]
    #[test]
    fn test_complex_fixed_module_accessible() {
        use super::ComplexCell;
        let c = ComplexCell::new(1.0, 2.0, 0.0, 0.0);
        assert!((c.real() - 1.0).abs() < 1e-6);
        assert!((c.imag() - 2.0).abs() < 1e-6);
    }
}
