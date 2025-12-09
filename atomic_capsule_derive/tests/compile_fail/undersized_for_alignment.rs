//! Test: Size smaller than alignment should compile but may cause issues
//!
//! T28 Q2 (Edge Cases): Testing size constraints
//! UCE34 Q33: Size should typically equal or exceed alignment
//!
//! Expected: Compilation succeeds (technically valid)
//! Note: This is a compile-pass edge case, but demonstrates unusual sizing

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 64)]  // Size < alignment (unusual but valid)
#[repr(C, align(128))]
struct UndersizedCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}

fn main() {
    // This compiles but is unusual
    let _capsule = UndersizedCapsule {
        state: AtomicU64::new(0),
        _padding: [0u8; 56],
    };
}
