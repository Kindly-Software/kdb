//! Test: Mutex field in capsule should generate deprecation warning
//!
//! T28 Q2 (Edge Cases): Testing non-atomic field types
//! UCE34 Q10: Capsules MUST use lockfree atomics, not Mutex
//!
//! Expected: Compilation should succeed but generate deprecation warning
//! about Mutex being incompatible with capsule architecture

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::Mutex;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct BadCapsuleWithMutex {
    state: Mutex<u64>,  // ❌ Should generate deprecation warning
    _padding: [u8; 48],
}

fn main() {}
