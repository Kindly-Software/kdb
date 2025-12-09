//! Test: Minimal valid capsule (smallest possible)
//!
//! T28 Q2 (Edge Cases): Testing minimal valid configuration
//! UCE34 Q31: Simplest capsule that satisfies all requirements
//!
//! Expected: Compilation succeeds with minimal 32-byte capsule

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 32, size = 32)]
#[repr(C, align(32))]
struct MinimalCapsule {
    state: AtomicU64,
    _padding: [u8; 24],
}

fn main() {
    use core::mem::{size_of, align_of};

    assert_eq!(size_of::<MinimalCapsule>(), 32);
    assert_eq!(align_of::<MinimalCapsule>(), 32);

    let capsule = MinimalCapsule {
        state: AtomicU64::new(42),
        _padding: [0u8; 24],
    };

    println!("Minimal capsule (32 bytes) verified!");
}
