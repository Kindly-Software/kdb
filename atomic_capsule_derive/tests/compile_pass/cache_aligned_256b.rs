//! Test: 256-byte cache-aligned capsule (quad cache line)
//!
//! T28 Q2 (Edge Cases): Maximum typical cache alignment
//! UCE34 Q10: 256B alignment for large complex capsules
//!
//! Expected: Compilation succeeds

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
struct CacheAligned256B {
    state: [AtomicU64; 8],  // 64 bytes
    _padding: [u8; 192],     // 192 bytes
}

fn main() {
    use core::mem::{size_of, align_of};

    // Verify quad cache line alignment
    assert_eq!(align_of::<CacheAligned256B>(), 256);
    assert_eq!(size_of::<CacheAligned256B>(), 256);

    let capsule = CacheAligned256B {
        state: [
            AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
        ],
        _padding: [0u8; 192],
    };

    let addr = &capsule as *const _ as usize;
    assert_eq!(addr % 256, 0, "Capsule must be 256-byte aligned");

    println!("256-byte cache-aligned capsule (quad line) verified!");
}
