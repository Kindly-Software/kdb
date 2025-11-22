//! Minimal reproduction of parallel module segfault
//!
//! This example reproduces the SIGSEGV from the pilot test to isolate the bug.

use atomic_capsule::parallel::{ParallelError, ThreadPool};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing ThreadPool with Arc clone pattern from pilot...");

    let pool = ThreadPool::new(4)?;
    let counter = Arc::new(AtomicU64::new(0));

    let task_count = 100_000; // 10 epochs × 10K neurons
    println!("Pushing {} tasks...", task_count);

    for _i in 0..task_count {
        // EXACT pilot pattern: two-level Arc clone with retry loop
        let counter_clone = Arc::clone(&counter);

        let mut retries = 0;
        loop {
            let c = Arc::clone(&counter_clone); // Clone AGAIN for retry

            match pool.push(Box::new(move || {
                // Simulate work (like pilot)
                let _ = (0..100).fold(0u64, |acc, x| acc.wrapping_add(x));
                c.fetch_add(1, Ordering::Relaxed);
            })) {
                Ok(()) => break,
                Err(atomic_capsule::parallel::ParallelError::QueueFull) => {
                    retries += 1;
                    if retries > 100 {
                        panic!("Queue full after 100 retries");
                    }
                    std::thread::yield_now();
                }
                Err(e) => {
                    panic!("ThreadPool error: {}", e);
                }
            }
        }
    }

    println!("Waiting for tasks to complete...");
    pool.wait();

    let final_count = counter.load(Ordering::Acquire);
    println!("Final count: {}", final_count);

    assert_eq!(final_count, task_count as u64, "All tasks should execute");

    println!("✅ Test passed!");

    Ok(())
}
