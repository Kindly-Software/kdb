//! Test: False sharing prevention via cache alignment
//!
//! T28 Q9 (Concurrent Invariants): Cache alignment prevents false sharing
//! UCE34 Q10: Each capsule gets its own cache line
//!
//! Expected: Compilation succeeds, concurrent access without contention

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct NoCacheContention {
    counter: AtomicU64,
    _padding: [u8; 56],
}

fn main() {
    // Create two capsules - each should be on separate cache lines
    let capsule1 = Arc::new(NoCacheContention {
        counter: AtomicU64::new(0),
        _padding: [0u8; 56],
    });

    let capsule2 = Arc::new(NoCacheContention {
        counter: AtomicU64::new(0),
        _padding: [0u8; 56],
    });

    // Verify they're on different cache lines
    let addr1 = &*capsule1 as *const _ as usize;
    let addr2 = &*capsule2 as *const _ as usize;
    assert_ne!(addr1 / 64, addr2 / 64, "Capsules should be on different cache lines");

    // Hammer both capsules concurrently (no false sharing)
    let threads: Vec<_> = (0..4)
        .map(|i| {
            let c1 = Arc::clone(&capsule1);
            let c2 = Arc::clone(&capsule2);
            thread::spawn(move || {
                for _ in 0..10_000 {
                    if i % 2 == 0 {
                        c1.counter.fetch_add(1, Ordering::Relaxed);
                    } else {
                        c2.counter.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for t in threads {
        t.join().unwrap();
    }

    println!("False sharing prevention verified!");
    println!("Capsule1: {}, Capsule2: {}",
             capsule1.counter.load(Ordering::Relaxed),
             capsule2.counter.load(Ordering::Relaxed));
}
