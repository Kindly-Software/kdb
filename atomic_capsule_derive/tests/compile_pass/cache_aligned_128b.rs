//! Test: 128-byte cache-aligned capsule (dual cache line)
//!
//! T28 Q3 (Invariants): Dual cache line alignment for complex capsules
//! UCE34 Q10: 128B alignment for DualAtomicU64 pattern
//!
//! Expected: Compilation succeeds

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct CacheAligned128B {
    generation: AtomicU64,
    primary: AtomicU64,
    secondary: AtomicU64,
    _padding: [u8; 104],
}

fn main() {
    use core::mem::{size_of, align_of};

    // Verify dual cache line alignment
    assert_eq!(align_of::<CacheAligned128B>(), 128);
    assert_eq!(size_of::<CacheAligned128B>(), 128);

    let capsule = CacheAligned128B {
        generation: AtomicU64::new(0),
        primary: AtomicU64::new(0),
        secondary: AtomicU64::new(0),
        _padding: [0u8; 104],
    };

    let addr = &capsule as *const _ as usize;
    assert_eq!(addr % 128, 0, "Capsule must be 128-byte aligned");

    println!("128-byte cache-aligned capsule (dual line) verified!");
}
