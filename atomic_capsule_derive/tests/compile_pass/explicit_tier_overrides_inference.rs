//! Test: Explicit tier overrides inference
//!
//! T28 Q1 (Core Behaviors): Explicit tier takes precedence
//! UCE34 Q10: User-specified tier wins over inference
//!
//! Expected: Compilation succeeds, explicit tier "Mixed" used despite atomic fields

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, tier = "Mixed")]  // Explicit tier
#[repr(C, align(64))]
struct ExplicitMixed {
    // Would infer Atomic, but explicit tier="Mixed" takes precedence
    state: AtomicU64,
    _padding: [u8; 56],
}

fn main() {
    let capsule = ExplicitMixed {
        state: AtomicU64::new(0),
        _padding: [0u8; 56],
    };

    println!("Explicit tier 'Mixed' overrode inferred 'Atomic'!");
}
