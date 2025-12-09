//! Test: Capsule with only Send + Sync fields
//!
//! T28 Q3 (Invariants): Thread-safety invariants hold
//! UCE34 Q10: Capsules MUST be Send + Sync
//!
//! Expected: Compilation succeeds, thread-safety verified

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::{AtomicU64, AtomicI32};
use std::sync::Arc;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
struct ThreadSafeCapsule {
    state: AtomicU64,
    counter: AtomicI32,
    shared: Arc<u32>,  // Arc is Send + Sync
    _padding: [u8; 44],
}

fn main() {
    // Verify Send + Sync
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<ThreadSafeCapsule>();
    assert_sync::<ThreadSafeCapsule>();

    // Test concurrent access
    use std::thread;

    let capsule = Arc::new(ThreadSafeCapsule {
        state: AtomicU64::new(0),
        counter: AtomicI32::new(0),
        shared: Arc::new(42),
        _padding: [0u8; 44],
    });

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                use core::sync::atomic::Ordering;
                c.state.fetch_add(1, Ordering::Relaxed);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("Thread-safe capsule verified across 4 threads!");
}
