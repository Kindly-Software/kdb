//! Test: Capsule with Box field (Send + Sync if T is)
//!
//! T28 Q1 (Core Behaviors): Testing Box as valid capsule field
//! UCE34 Q10: Box<T> is Send + Sync if T is Send + Sync
//!
//! Expected: Compilation succeeds, Box field is thread-safe

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct CapsuleWithBox {
    state: AtomicU64,
    heap_data: Box<[u8; 16]>,
    _padding: [u8; 48],
}

fn main() {
    let capsule = CapsuleWithBox {
        state: AtomicU64::new(0),
        heap_data: Box::new([42u8; 16]),
        _padding: [0u8; 48],
    };

    // Verify Send + Sync (Box<T> where T: Send + Sync)
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<CapsuleWithBox>();
    assert_sync::<CapsuleWithBox>();

    assert_eq!(capsule.heap_data[0], 42);
    println!("Capsule with Box field verified!");
}
