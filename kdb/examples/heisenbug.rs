//! Heisenbug Example - Race Condition that Changes Under Observation
//!
//! This program demonstrates a classic heisenbug: a race condition that
//! behaves differently when debugged due to timing changes introduced by
//! the debugger itself.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== Heisenbug Demonstration ===");
    println!("PID: {}", std::process::id());
    println!("This program has a race condition that may disappear under debugging.\n");

    // Shared counter with intentional race condition
    let counter = Arc::new(AtomicU64::new(0));
    let mut handles = vec![];

    // Spawn 4 racing threads
    for thread_id in 0..4 {
        let counter_clone = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            for i in 0..10000 {
                // THE BUG: Read-modify-write race condition
                // Multiple threads can read the same value before any writes
                let current = counter_clone.load(Ordering::Relaxed);

                // Delay to make race window larger and allow debugger attachment
                if i % 100 == 0 {
                    thread::sleep(Duration::from_micros(100));
                }
                std::hint::spin_loop();

                // Non-atomic increment (lost updates possible)
                counter_clone.store(current + 1, Ordering::Relaxed);

                // Occasional reporting
                if i % 2500 == 0 {
                    println!("Thread {} at iteration {}, counter = {}",
                             thread_id, i, counter_clone.load(Ordering::Relaxed));
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    let final_value = counter.load(Ordering::Relaxed);
    println!("\n=== Results ===");
    println!("Expected: 40000 (4 threads × 10000 increments)");
    println!("Actual:   {}", final_value);

    if final_value == 40000 {
        println!("✓ No race detected (heisenbug hiding!)");
    } else {
        println!("✗ Race condition detected! Lost {} updates", 40000 - final_value);
    }

    println!("\nTry debugging this with kdb:");
    println!("1. Set breakpoint on line with load(Ordering::Relaxed)");
    println!("2. Watch the counter value as threads race");
    println!("3. Notice how debugger timing affects the outcome!");
}
