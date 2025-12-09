//! Test: Cell field in capsule should generate deprecation warning
//!
//! T28 Q2 (Edge Cases): Testing non-thread-safe field types
//! UCE34 Q10: Capsules MUST be Send + Sync
//!
//! Expected: Compilation should succeed but generate deprecation warning
//! about Cell not being thread-safe

use atomic_capsule_derive::ComputationalCapsule;
use std::cell::Cell;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct BadCapsuleWithCell {
    state: Cell<u64>,  // ❌ Should generate deprecation warning (not Send/Sync)
    _padding: [u8; 56],
}

fn main() {}
