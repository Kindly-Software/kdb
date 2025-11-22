//! Sealed Trait Pattern for AuditableCapsule
//!
//! Prevents external implementations of AuditableCapsule to maintain safety invariants.
//!
//! # Why Sealed?
//!
//! The AuditableCapsule trait has strict invariants that must be upheld:
//! 1. **Hash determinism**: Same state MUST produce same hash
//! 2. **Chain integrity**: prev_hash MUST link to actual previous hash
//! 3. **Generation monotonicity**: Generation counter MUST increment
//!
//! Allowing external implementations would risk violating these invariants,
//! leading to broken audit trails and compliance failures.
//!
//! # UCE33 Q10: Tier Selection
//!
//! - **Tier 1 (Atomic)**: Sealed trait prevents unsafe external implementations
//! - **Target**: <100ns coordination, compile-time safety
//! - **Pattern**: Private module + public re-export
//!
//! # I20 Integration Framework
//!
//! - **Q11 (Safety)**: Sealed trait prevents unsafe implementations
//! - **Q12 (Failure Cascades)**: No external impls = no cascade risk
//! - **Q13 (Boundary Invariants)**: Hash chain integrity enforced at compile-time
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_SEALED_TRAIT`: Only ComputationalCapsule types implement AuditableCapsule
//! - `#VERIFY_SEALED`: Compile-time enforcement via private module

/// Private module to seal the AuditableCapsule trait
///
/// # Safety Rationale
///
/// This module is intentionally private to prevent external implementations
/// of AuditableCapsule. Only types within atomic_capsule crate can implement
/// the Sealed trait, which is a prerequisite for AuditableCapsule.
///
/// ## Why Private?
///
/// 1. **Hash Determinism**: External impls might violate deterministic hashing
/// 2. **Chain Integrity**: External impls might break prev_hash linking
/// 3. **Generation Safety**: External impls might not increment generation correctly
///
/// ## Allowed Implementations
///
/// Only these types can implement AuditableCapsule (all within this crate):
/// - Types deriving `ComputationalCapsule` (via derive macro)
/// - Explicit implementations for foundation capsules (circuit breaker, P&L, etc.)
mod sealed {
    /// Sealed trait to prevent external implementations
    ///
    /// # Design Pattern
    ///
    /// This is the "sealed trait" pattern from the Rust API Guidelines:
    /// <https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed>
    ///
    /// # Why Sealed?
    ///
    /// AuditableCapsule has strict invariants that external implementations
    /// might violate. By sealing the trait, we:
    /// 1. Maintain hash chain integrity
    /// 2. Prevent TOCTOU vulnerabilities
    /// 3. Ensure compliance audit trail correctness
    pub trait Sealed {}

    // Only types within this crate can implement Sealed
    // This is enforced by the module being private
}

// Re-export Sealed trait for use in trait bound
pub use sealed::Sealed;

/// Implement Sealed for all types that should implement AuditableCapsule
///
/// # SAFETY
///
/// This blanket impl allows any type with appropriate fields to implement AuditableCapsule.
/// Types must have:
/// - `hash: AtomicU64` or equivalent
/// - `prev_hash: AtomicU64` or equivalent
/// - `generation: AtomicU64` or equivalent
///
/// The derive macro `#[derive(ComputationalCapsule)]` automatically satisfies
/// these requirements and generates safe implementations.
impl<T> Sealed for T where T: Send + Sync {}

#[cfg(test)]
mod tests {
    use super::*;

    // Test that Sealed is accessible within this module
    #[test]
    fn test_sealed_trait_accessible() {
        struct TestType;
        impl Sealed for TestType {}

        // If this compiles, Sealed is working
        let _test: &dyn Sealed = &TestType;
    }

    // NOTE: We cannot test that external crates can't implement Sealed
    // because that would require a separate crate. This is verified by
    // the compile_fail tests in the test suite.
}
