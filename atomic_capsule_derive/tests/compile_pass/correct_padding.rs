//! Test: Capsule with correct padding to reach target size
//!
//! T28 Q3 (Invariants): Size and alignment invariants hold
//! UCE34 Q33: Padding ensures exact size match
//!
//! Expected: Compilation succeeds, size assertions pass

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct WellPaddedCapsule {
    primary: AtomicU64,    // 8 bytes
    secondary: AtomicU64,  // 8 bytes
    generation: AtomicU64, // 8 bytes
    _padding: [u8; 104],   // 104 bytes → total 128 bytes
}

fn main() {
    use core::mem::{size_of, align_of};

    // Verify size and alignment at runtime
    assert_eq!(size_of::<WellPaddedCapsule>(), 128);
    assert_eq!(align_of::<WellPaddedCapsule>(), 128);

    let capsule = WellPaddedCapsule {
        primary: AtomicU64::new(0),
        secondary: AtomicU64::new(0),
        generation: AtomicU64::new(0),
        _padding: [0u8; 104],
    };

    println!("Well-padded capsule verified: {}B @ {}B alignment",
             size_of::<WellPaddedCapsule>(),
             align_of::<WellPaddedCapsule>());
}
