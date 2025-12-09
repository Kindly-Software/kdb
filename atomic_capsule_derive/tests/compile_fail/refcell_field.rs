//! Test: RefCell field in capsule should generate deprecation warning
//!
//! T28 Q2 (Edge Cases): Testing non-thread-safe field types
//! UCE34 Q10: Capsules MUST be Send + Sync
//!
//! Expected: Compilation should succeed but generate deprecation warning
//! about RefCell not being thread-safe

use atomic_capsule_derive::ComputationalCapsule;
use std::cell::RefCell;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct BadCapsuleWithRefCell {
    state: RefCell<u64>,  // ❌ Should generate deprecation warning (not Send/Sync)
    _padding: [u8; 40],
}

fn main() {}
