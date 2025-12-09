//! Test: Stress test with high concurrency
//!
//! T28 Q22 (Stress Testing): 100 threads × 10K operations
//! UCE34 Q10: Capsules must handle extreme concurrency
//!
//! Expected: Compilation and execution succeed, no data loss

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct StressCapsule {
    counter: AtomicU64,
    _padding: [u8; 56],
}

fn main() {
    let capsule = Arc::new(StressCapsule {
        counter: AtomicU64::new(0),
        _padding: [0u8; 56],
    });

    const THREADS: usize = 100;
    const OPS_PER_THREAD: usize = 10_000;

    let start = std::time::Instant::now();

    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..OPS_PER_THREAD {
                    c.counter.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();
    let final_count = capsule.counter.load(Ordering::Relaxed);
    let expected = (THREADS * OPS_PER_THREAD) as u64;

    assert_eq!(final_count, expected, "No lost updates!");

    println!("Stress test PASSED!");
    println!("Threads: {}, Ops/thread: {}, Total ops: {}",
             THREADS, OPS_PER_THREAD, final_count);
    println!("Time: {:?}, Throughput: {:.0} ops/sec",
             elapsed,
             expected as f64 / elapsed.as_secs_f64());
}
