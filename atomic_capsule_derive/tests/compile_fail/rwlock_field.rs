//! Test: RwLock field in capsule should generate deprecation warning
//!
//! T28 Q2 (Edge Cases): Testing non-atomic field types
//! UCE34 Q10: Capsules MUST use lockfree atomics, not RwLock
//!
//! Expected: Compilation should succeed but generate deprecation warning
//! about RwLock being incompatible with capsule architecture

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::RwLock;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct BadCapsuleWithRwLock {
    state: RwLock<u64>,  // ❌ Should generate deprecation warning
    _padding: [u8; 48],
}

fn main() {}
