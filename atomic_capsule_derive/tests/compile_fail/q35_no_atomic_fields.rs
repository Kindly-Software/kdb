//! Test: Q35 Self-Destruct Violation - No atomic fields
//!
//! T28 Q21 (Integration): Testing Q35 compile error for capsules without atomic fields
//! UCE34 Q35: Self-destruct requires at least one atomic field for poison tracking
//!
//! Expected: Compilation FAILS with Q35 violation error

use atomic_capsule_derive::ComputationalCapsule;

/// Capsule without atomic fields - should FAIL Q35 validation
/// (skip_self_destruct not set, so self-destruct is mandatory)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct NoAtomicFieldsCapsule {
    /// Plain data - not atomic!
    data: [u8; 64],
}

fn main() {}
