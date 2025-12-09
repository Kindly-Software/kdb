//! Test: Missing #[repr(C)] should fail compilation
//!
//! T28 Q2 (Edge Cases): Testing required attributes
//! UCE34 Q11: Capsules MUST have deterministic layout via #[repr(C)]
//!
//! Expected: Compilation error - missing #[repr(C)]

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(align(64))]  // ❌ Missing repr(C) - only has align
struct MissingReprC {
    state: AtomicU64,
    _padding: [u8; 56],
}

fn main() {}
