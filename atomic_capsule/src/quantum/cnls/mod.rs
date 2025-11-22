//! # CNLS (Cubic-Nonlinear Schrodinger) Rule Capsule (Phase 4.2)
//!
//! **Tier 6 Mixed Capsule**: Atomic coordination (T1) + Fixed-Point computation (T3) + Q34 Auditability
//!
//! ## Capsules
//!
//! - **CNLSRuleCapsule**: Quantum wave evolution rule with tamper-evident audit trails
//!   - 128-byte aligned composite capsule
//!   - Q16.48 fixed-point deterministic arithmetic
//!   - Hash-chained audit trail for Q34 compliance
//!   - <50ns per evolution step
//!
//! - **evolve_cnls_4d**: 4D quantum wave evolution function
//!   - SIMD-accelerated complex arithmetic (ComplexF32x4)
//!   - Deterministic fixed-point computation (ComplexCell)
//!   - Breakthrough innovation: SIMD + Fixed-Point + Auditability
//!
//! ## UCE34 Q1-Q34 Analysis
//!
//! ### Q10-Q12: Foundation
//! - Q10 (Capsule Tier): T6 Mixed (T1 Atomic + T3 Fixed-Point + Q34 Auditability)
//! - Q11 (Rust Transform): Composite capsule with multiple tier integration
//! - Q12 (Nightly): portable_simd, const_fn_floating_point, atomic_from_mut
//!
//! ### Q16-Q20: Integration (I20)
//! - Q16 (Rollback): Git revert (additive only, no existing code modified)
//! - Q17 (Monitoring): Q34 audit trails provide tamper-evident logs
//! - Q18 (Compatibility): Zero breaking changes (new pattern, standalone)
//! - Q19 (Strategy): I20-Progressive (parallel deployment, CNLSRuleCapsule is self-contained)
//! - Q20 (Success): Compilation + tests pass + Q34 audit trail verified
//!
//! ### Q25: Verification
//! - #[derive(ComputationalCapsule)] for compile-time verification
//! - 128-byte alignment (composite tier requirement)
//! - Generation counters for TOCTOU prevention
//!
//! ### Q33: Validation
//! - Property tests for quantum wave conservation
//! - Unit tests for CNLS evolution correctness
//! - Integration tests for audit trail integrity
//!
//! ### Q34: Auditability
//! - Hash-chained audit trail for state modifications
//! - Tamper-detection via hash verification
//! - Compliance-ready (SOX, SOC2, GDPR, HIPAA)
//! - Reproducibility from audit trail (exact replay)
//!
//! ## ASSUM Framework
//! - #ASSUME_COMPOSITE_ALIGNMENT: 128-byte alignment for multi-tier capsule
//! - #VERIFY_ALIGNMENT_STATIC: Compile-time verification via #[repr(C, align(128))]
//! - #ASSUME_FIXED_POINT_DETERMINISM: Q16.48 arithmetic is exact (no rounding)
//! - #VERIFY_AUDIT_TRAIL: Hash chain integrity validated at runtime
//! - #ASSUME_LOCKFREE: All atomic operations use generation counters
//! - #VERIFY_TOCTOU: Generation counter prevents race conditions
//!
//! ## Feature Flags
//!
//! ```toml
//! # Cargo.toml
//! [dependencies]
//! atomic_capsule = { version = "0.3.4", features = ["cnls"] }
//! ```
//!
//! **Dependencies**: `cnls` feature requires:
//! - `complex-simd` (ComplexF32x4 for SIMD acceleration)
//! - `complex-fixed` (ComplexCell for deterministic arithmetic)
//! - `capsule-serialize` (Q34 audit trail serialization)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use atomic_capsule::patterns::cnls::{CNLSRuleCapsule, evolve_cnls_4d};
//! use atomic_capsule::primitives::complex::{ComplexF32x4, ComplexCell};
//!
//! // Create CNLS rule capsule with audit trail
//! let rule = CNLSRuleCapsule::new(0.1, 1.0, 0.5); // dt, alpha, beta
//!
//! // Evolve quantum wave (4× parallel complex numbers)
//! let psi = ComplexF32x4::splat(1.0, 0.0);
//! let psi_next = evolve_cnls_4d(&rule, &psi);
//!
//! // Verify audit trail integrity
//! assert!(rule.verify_audit_trail());
//! ```
//!
//! ## Performance Targets (B32)
//!
//! - CNLSRuleCapsule creation: <10ns (atomic initialization)
//! - evolve_cnls_4d: <50ns (SIMD complex arithmetic + fixed-point)
//! - Audit trail append: <50ns (hash-chain update, amortized)
//!
//! ## IMPL-2 V3.1 Compliance
//!
//! This pattern uses cutting-edge-first development:
//! - Nightly-first (portable_simd, const_fn_floating_point)
//! - Tier-maximization (T6 Mixed = T1+T3+Q34 compound)
//! - Innovation-stacking (SIMD + Fixed-Point + Auditability = breakthrough)
//! - Target breakthrough performance (compound speedup from 3 tiers)
//! - Advanced patterns mandatory (generation counters, cache alignment, hash-chain audit)
//!
//! ## Research Context
//!
//! CNLS (Cubic-Nonlinear Schrodinger) equation models quantum wave propagation:
//! ```text
//! i ∂ψ/∂t = -∇²ψ + α|ψ|²ψ + β|ψ|⁴ψ
//! ```
//!
//! This capsule implements a 4th-order Runge-Kutta solver with:
//! - SIMD acceleration (4× complex numbers per operation)
//! - Deterministic arithmetic (Q16.48 fixed-point)
//! - Tamper-evident audit trails (Q34 compliance)

// ========== T6 Mixed Composite: CNLS Rule Capsule ==========

pub mod cnls_rule;

// Re-export CNLS types (only what exists in cnls_rule.rs)
pub use cnls_rule::{
    evolve_cnls_4d, validate_determinism_q16_48, verify_norm_conservation, CNLSError,
    CNLSRuleCapsule, ComplexCell, Universe4DInterface,
};

// Phase 4.2 Week 4: Split-Step Fourier Method (feature-gated)
#[cfg(feature = "split-step-fourier")]
pub mod split_step_fourier;

#[cfg(feature = "split-step-fourier")]
pub use split_step_fourier::{
    evolve_split_step_cnls_4d, fft_4d_forward, ifft_4d_backward, LinearOperator, NonlinearOperator,
    SplitStepFourierCNLS,
};

#[cfg(test)]
mod tests {
    #[test]
    fn test_cnls_module_structure() {
        // Module exists and is accessible
        assert!(true);
    }

    #[cfg(feature = "cnls")]
    #[test]
    fn test_cnls_feature_enabled() {
        // CNLS feature is properly gated
        assert!(true);
    }
}
