//! Test: Recursive type with Box<Self>
//!
//! T28 Q2 (Edge Cases): Testing unusual type patterns
//! UCE34 Q10: Capsules should be self-contained, not recursive
//!
//! Expected: Compilation succeeds (Box breaks recursion)
//! Note: This is technically valid Rust, demonstrates edge case handling

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct RecursiveCapsule {
    state: AtomicU64,
    next: Option<Box<RecursiveCapsule>>,  // Recursive but Box breaks cycle
    _padding: [u8; 32],
}

fn main() {
    // This compiles - Box breaks recursion
    let _capsule = RecursiveCapsule {
        state: AtomicU64::new(0),
        next: None,
        _padding: [0u8; 32],
    };
}
