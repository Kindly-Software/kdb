//! Test: 64-byte cache-aligned capsule
//!
//! T28 Q3 (Invariants): Cache alignment prevents false sharing
//! UCE34 Q10: 64B alignment fits single cache line
//!
//! Expected: Compilation succeeds

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct CacheAligned64B {
    state: AtomicU64,
    _padding: [u8; 56],
}

fn main() {
    use core::mem::{size_of, align_of};

    // Verify cache alignment
    assert_eq!(align_of::<CacheAligned64B>(), 64);
    assert_eq!(size_of::<CacheAligned64B>(), 64);

    // Check address alignment
    let capsule = CacheAligned64B {
        state: AtomicU64::new(0),
        _padding: [0u8; 56],
    };

    let addr = &capsule as *const _ as usize;
    assert_eq!(addr % 64, 0, "Capsule must be 64-byte aligned");

    println!("64-byte cache-aligned capsule verified!");
}
