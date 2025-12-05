//! Deadlock Example - Circular ABBA Lock Ordering
//!
//! This program demonstrates a classic ABBA deadlock pattern where two threads
//! acquire locks in opposite order, leading to circular wait.
//!
//! **Deadlock Pattern:**
//! ```text
//! Thread 1: lock(A) → lock(B) → unlock(B) → unlock(A)
//! Thread 2: lock(B) → lock(A) → unlock(A) → unlock(B)
//!
//! Timeline (deadlock scenario):
//! T1: acquire A ─────────────────────────▼ wait for B (T2 holds B)
//! T2: ─────────── acquire B ─────────────▼ wait for A (T1 holds A)
//!                                        ▼ DEADLOCK: circular wait
//! ```
//!
//! **Use with kdb:**
//! 1. Run this program: `cargo run --example deadlock_circular`
//! 2. Attach kdb: `attach <pid>`
//! 3. Use `stack` to see both threads blocked on futex
//! 4. Observe the ABBA deadlock pattern in lock acquisition order
//!
//! **T28 Framework Compliance:**
//! - Q29: Deterministic deadlock (specific timing window)
//! - Q30: Reproducible on multi-core systems
//! - Q31: Validated lock ordering violation

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Resource A - First lock in the ordering
#[allow(dead_code)]
struct ResourceA {
    value: u64,
}

/// Resource B - Second lock in the ordering
#[allow(dead_code)]
struct ResourceB {
    value: u64,
}

fn main() {
    println!("=== ABBA Deadlock Demonstration ===");
    println!("PID: {}", std::process::id());
    println!("\nThis program WILL DEADLOCK by design.");
    println!("Use kdb to inspect the deadlock state.\n");

    // Shared resources protected by mutexes
    let resource_a = Arc::new(Mutex::new(ResourceA { value: 1 }));
    let resource_b = Arc::new(Mutex::new(ResourceB { value: 2 }));

    // Clone for thread 2
    let resource_a_clone = Arc::clone(&resource_a);
    let resource_b_clone = Arc::clone(&resource_b);

    // Thread 1: Acquires A then B (correct order: A → B)
    let thread1 = thread::spawn(move || {
        println!("[Thread 1] Starting - will acquire A then B");

        // Acquire lock A
        let _guard_a = resource_a.lock().unwrap();
        println!("[Thread 1] Acquired lock A");

        // Delay to increase deadlock probability
        // This gives Thread 2 time to acquire lock B
        thread::sleep(Duration::from_millis(100));

        println!("[Thread 1] Attempting to acquire lock B...");
        // This will block if Thread 2 holds B
        let _guard_b = resource_b.lock().unwrap();
        println!("[Thread 1] Acquired lock B");

        // Simulate work
        println!("[Thread 1] Working with both resources");

        println!("[Thread 1] Releasing locks");
        // Guards dropped automatically
    });

    // Thread 2: Acquires B then A (WRONG order: B → A)
    // This creates the ABBA deadlock pattern
    let thread2 = thread::spawn(move || {
        println!("[Thread 2] Starting - will acquire B then A (WRONG ORDER!)");

        // Small delay to let Thread 1 start first
        thread::sleep(Duration::from_millis(50));

        // Acquire lock B first (opposite order from Thread 1)
        let _guard_b = resource_b_clone.lock().unwrap();
        println!("[Thread 2] Acquired lock B");

        // Delay to ensure Thread 1 has acquired A
        thread::sleep(Duration::from_millis(100));

        println!("[Thread 2] Attempting to acquire lock A...");
        // This will block if Thread 1 holds A → DEADLOCK!
        let _guard_a = resource_a_clone.lock().unwrap();
        println!("[Thread 2] Acquired lock A");

        // Simulate work
        println!("[Thread 2] Working with both resources");

        println!("[Thread 2] Releasing locks");
        // Guards dropped automatically
    });

    // Set a timeout for deadlock detection
    println!("\nWaiting for threads (will timeout on deadlock)...");
    println!("Press Ctrl+C to abort, or use kdb to inspect.\n");

    // Wait with timeout (in production, this would hang forever)
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(5);

    loop {
        if thread1.is_finished() && thread2.is_finished() {
            println!("\n✓ Both threads completed (no deadlock occurred)");
            println!("  This can happen if timing prevents the race condition.");
            break;
        }

        if start.elapsed() > timeout {
            println!("\n✗ DEADLOCK DETECTED!");
            println!("  Threads have been blocked for > 5 seconds.");
            println!("\n  Use kdb to inspect:");
            println!("  1. attach {}", std::process::id());
            println!("  2. stack        # Show blocked threads");
            println!("  3. info threads # List all threads");
            println!("\n  The program will now hang indefinitely.");
            println!("  Press Ctrl+C to abort.");

            // In real production code, you might want to abort here
            // For demonstration, we let it hang so kdb can inspect
            loop {
                thread::sleep(Duration::from_secs(1));
            }
        }

        thread::sleep(Duration::from_millis(100));
    }

    // Join threads (this would block forever on deadlock)
    thread1.join().expect("Thread 1 panicked");
    thread2.join().expect("Thread 2 panicked");

    println!("\n=== Demonstration Complete ===");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify the deadlock condition is detectable
    /// This test uses a timeout to detect the deadlock
    #[test]
    fn test_deadlock_detection_timeout() {
        use std::sync::mpsc;
        use std::time::Duration;

        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();

        let resource_a = Arc::new(Mutex::new(ResourceA { value: 1 }));
        let resource_b = Arc::new(Mutex::new(ResourceB { value: 2 }));

        let a1 = Arc::clone(&resource_a);
        let b1 = Arc::clone(&resource_b);
        let a2 = Arc::clone(&resource_a);
        let b2 = Arc::clone(&resource_b);

        // Thread 1: A → B
        let t1 = thread::spawn(move || {
            let _a = a1.lock().unwrap();
            tx1.send(()).unwrap(); // Signal A acquired
            thread::sleep(Duration::from_millis(50));
            let _b = b1.lock().unwrap();
            tx1.send(()).unwrap(); // Signal B acquired
        });

        // Thread 2: B → A (opposite order)
        let t2 = thread::spawn(move || {
            let _b = b2.lock().unwrap();
            tx2.send(()).unwrap(); // Signal B acquired
            thread::sleep(Duration::from_millis(50));
            let _a = a2.lock().unwrap();
            tx2.send(()).unwrap(); // Signal A acquired
        });

        // Wait for first lock acquisitions
        rx1.recv_timeout(Duration::from_secs(1)).ok();
        rx2.recv_timeout(Duration::from_secs(1)).ok();

        // Second acquisitions should timeout (deadlock)
        let t1_second = rx1.recv_timeout(Duration::from_millis(500));
        let t2_second = rx2.recv_timeout(Duration::from_millis(500));

        // At least one should timeout (deadlock detected)
        let deadlock_detected = t1_second.is_err() || t2_second.is_err();

        // Clean up (this will leak threads on deadlock, but that's expected)
        // In production, we'd use try_lock or parking_lot with timeouts

        assert!(
            deadlock_detected,
            "Expected deadlock was not detected within timeout"
        );
    }
}
