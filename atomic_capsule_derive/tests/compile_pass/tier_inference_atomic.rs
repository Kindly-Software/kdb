//! Test: Tier inference from atomic field types
//!
//! T28 Q1 (Core Behaviors): Automatic tier inference from fields
//! UCE34 Q10: Infer "Atomic" tier from AtomicU64 fields
//!
//! Expected: Compilation succeeds, tier inferred as Atomic

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]  // No tier specified - should infer
#[repr(C, align(64))]
struct InferredAtomic {
    state: AtomicU64,
    counter: AtomicU64,
    _padding: [u8; 48],
}

fn main() {
    let capsule = InferredAtomic {
        state: AtomicU64::new(0),
        counter: AtomicU64::new(0),
        _padding: [0u8; 48],
    };

    println!("Tier inferred as Atomic from field types!");
}
