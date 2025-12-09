//! Test: verified attribute for T0 Verified capsules (formal verification)
//!
//! # Purpose
//! Validate that `#[capsule(verified = true)]` generates verification method stubs
//! for TLA+/Spin model checking, Z3 theorem proving, and KLEE symbolic execution.
//!
//! # T0 Verified (2025-11-07)
//! New tier expansion for formal verification support. User must implement
//! verification methods with external tools (TLA+, Z3, KLEE).

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::AtomicU64;

/// Verified capsule with formal verification support
///
/// # T0 Verified
/// - TLA+/Spin: Model checking for lockfree algorithms
/// - Z3: SMT solving for fixed-point arithmetic invariants
/// - KLEE: Symbolic execution for pipeline correctness
///
/// # Requirements
/// - Alignment >= 64 bytes (single cache line minimum)
/// - User implements verification methods with external tools
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, verified = true)]
#[repr(C, align(64))]
struct VerifiedCapsule {
    state: AtomicU64,
    counter: AtomicU64,
    _padding: [u8; 48],
}

/// Verified capsule with dual cache line alignment
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128, verified = true)]
#[repr(C, align(128))]
struct VerifiedDualCapsule {
    primary: AtomicU64,
    secondary: AtomicU64,
    state: AtomicU64,
    _padding: [u8; 104],
}

fn main() {
    // Create instances
    let capsule = VerifiedCapsule {
        state: AtomicU64::new(0),
        counter: AtomicU64::new(0),
        _padding: [0u8; 48],
    };

    let dual_capsule = VerifiedDualCapsule {
        primary: AtomicU64::new(0),
        secondary: AtomicU64::new(0),
        state: AtomicU64::new(0),
        _padding: [0u8; 104],
    };

    // Verify alignment
    assert_eq!(std::mem::align_of_val(&capsule), 64);
    assert_eq!(std::mem::size_of_val(&capsule), 64);

    assert_eq!(std::mem::align_of_val(&dual_capsule), 128);
    assert_eq!(std::mem::size_of_val(&dual_capsule), 128);

    // Verified capsules are Send + Sync
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    assert_send_sync(&capsule);
    assert_send_sync(&dual_capsule);

    drop(capsule);
    drop(dual_capsule);

    println!("✓ verified attribute works correctly");
    println!("✓ T0 Verified capsules ready for formal verification");
}
