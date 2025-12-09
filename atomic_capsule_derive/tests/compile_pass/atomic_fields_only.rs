//! Test: Capsule with only atomic fields should compile successfully
//!
//! T28 Q1 (Core Behaviors): Testing valid atomic-only capsule
//! UCE34 Q10: Capsules with atomic fields are ideal pattern
//!
//! Expected: Compilation succeeds, no warnings

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::{AtomicU64, AtomicU32, AtomicBool};

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct AllAtomicCapsule {
    state: AtomicU64,
    counter: AtomicU32,
    flag: AtomicBool,
    _padding: [u8; 51],
}

fn main() {
    let capsule = AllAtomicCapsule {
        state: AtomicU64::new(0),
        counter: AtomicU32::new(0),
        flag: AtomicBool::new(false),
        _padding: [0u8; 51],
    };

    // Verify traits
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<AllAtomicCapsule>();
    assert_sync::<AllAtomicCapsule>();

    println!("All-atomic capsule compiled successfully!");
}
