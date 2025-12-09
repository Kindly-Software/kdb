//! Test: Capsule with Arc field (Send + Sync)
//!
//! T28 Q1 (Core Behaviors): Testing Arc as valid capsule field
//! UCE34 Q10: Arc is Send + Sync, valid for shared ownership
//!
//! Expected: Compilation succeeds, Arc field is thread-safe

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;
use std::sync::Arc;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct CapsuleWithArc {
    state: AtomicU64,
    shared_data: Arc<Vec<u8>>,
    _padding: [u8; 48],
}

fn main() {
    let data = Arc::new(vec![1, 2, 3, 4, 5]);

    let capsule = CapsuleWithArc {
        state: AtomicU64::new(0),
        shared_data: Arc::clone(&data),
        _padding: [0u8; 48],
    };

    // Verify Send + Sync
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<CapsuleWithArc>();
    assert_sync::<CapsuleWithArc>();

    // Test concurrent access
    use std::thread;

    let capsule_arc = Arc::new(capsule);
    let handle = {
        let c = Arc::clone(&capsule_arc);
        thread::spawn(move || {
            assert_eq!(c.shared_data.len(), 5);
        })
    };

    handle.join().unwrap();
    println!("Capsule with Arc field verified across threads!");
}
