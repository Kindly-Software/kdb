//! Test: Valid Atomic tier with atomic fields
//!
//! T28 Q1 (Core Behaviors): Testing correct Atomic tier usage
//! UCE34 Q10: Tier 1 Atomic capsules for lockfree coordination
//!
//! Expected: Compilation succeeds

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, tier = "Atomic")]
#[repr(C, align(64))]
struct AtomicTierCapsule {
    state: AtomicU64,
    counter: AtomicU64,
    _padding: [u8; 48],
}

fn main() {
    let capsule = AtomicTierCapsule {
        state: AtomicU64::new(0),
        counter: AtomicU64::new(0),
        _padding: [0u8; 48],
    };

    println!("Atomic tier capsule (T1) verified!");
}
