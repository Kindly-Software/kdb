//! Test: Large valid capsule (256 bytes, maximum typical size)
//!
//! T28 Q2 (Edge Cases): Testing upper bound of typical capsule size
//! UCE34 Q29: 256B is maximum for multi-line capsules
//!
//! Expected: Compilation succeeds for 256-byte capsule

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
struct LargeCapsule {
    state: [AtomicU64; 8],  // 64 bytes
    _padding: [u8; 192],     // 192 bytes → total 256 bytes
}

fn main() {
    use core::mem::{size_of, align_of};
    use core::sync::atomic::AtomicU64;

    assert_eq!(size_of::<LargeCapsule>(), 256);
    assert_eq!(align_of::<LargeCapsule>(), 256);

    let capsule = LargeCapsule {
        state: [
            AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
        ],
        _padding: [0u8; 192],
    };

    println!("Large capsule (256 bytes) verified!");
}
