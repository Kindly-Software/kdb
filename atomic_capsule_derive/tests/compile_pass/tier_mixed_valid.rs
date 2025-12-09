//! Test: Valid Mixed tier documentation
//!
//! T28 Q1 (Core Behaviors): Testing Mixed tier label
//! UCE34 Q10: Tier 6 Mixed capsules for compound optimizations
//!
//! Expected: Compilation succeeds

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128, tier = "Mixed")]
#[repr(C, align(128))]
struct MixedTierCapsule {
    // Combines multiple tier patterns
    atomic_state: AtomicU64,      // T1: Atomic
    fixed_price: AtomicU64,        // T3: FixedPoint (Q16.16)
    batch_counter: AtomicU64,      // T4: Batch
    _padding: [u8; 104],
}

fn main() {
    let capsule = MixedTierCapsule {
        atomic_state: AtomicU64::new(0),
        fixed_price: AtomicU64::new(0),
        batch_counter: AtomicU64::new(0),
        _padding: [0u8; 104],
    };

    println!("Mixed tier capsule (T6) label verified!");
}
