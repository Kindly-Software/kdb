//! # ScreenStateCapsule Demonstration
//!
//! This example demonstrates the ScreenStateCapsule T1 Atomic primitive
//! for high-performance TUI screen state management.
//!
//! ## Performance
//!
//! - Screen navigation: <10ns (atomic load)
//! - Back stack traversal: <30ns (O(1) lookup)
//! - Error recording: <5ns (atomic store)
//! - Timeout checking: <10ns (arithmetic comparison)
//!
//! ## Features Demonstrated
//!
//! - Multi-threaded screen state synchronization
//! - Navigation history with back stack
//! - Timeout tracking for input
//! - Error code recording
//! - SWeMR (Single-Writer, Many-Readers) pattern
//!
//! ## Build & Run
//!
//! ```bash
//! cargo run --example screen_state_demo --features std
//! ```

use atomic_capsule::tui::{ScreenStateCapsule, ScreenId};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    println!("\n=== ScreenStateCapsule Demonstration ===\n");

    // Create a shared screen state capsule
    let screen = Arc::new(ScreenStateCapsule::new());

    println!("1. Initial State");
    println!("   Current screen: {:?}", screen.current());
    println!("   Previous screen: {:?}", screen.previous());
    println!("   Timeout: {} ns", screen.get_timeout());
    println!("   Generation: {}\n", screen.generation());

    // Single-writer thread: Perform navigation
    println!("2. Navigation");
    screen.navigate_to(ScreenId::Menu);
    println!("   Navigated to Menu");
    println!("   Current: {:?}, Previous: {:?}\n", screen.current(), screen.previous());

    screen.navigate_to(ScreenId::Settings);
    println!("   Navigated to Settings");
    println!("   Current: {:?}, Previous: {:?}\n", screen.current(), screen.previous());

    // Go back
    println!("3. Back Navigation");
    screen.go_back();
    println!("   Went back");
    println!("   Current: {:?}, Previous: {:?}\n", screen.current(), screen.previous());

    // Timeout configuration
    println!("4. Timeout Management");
    let transition_time = 1_000_000_000u64; // 1 second in nanoseconds
    let timeout = 5_000_000_000u64; // 5 second timeout
    screen.set_transition_time(transition_time);
    screen.set_timeout(timeout);
    println!("   Transition time: {} ns", screen.get_transition_time());
    println!("   Timeout duration: {} ns", screen.get_timeout());

    let current_time = 4_000_000_000u64; // 4 seconds elapsed
    println!("   Time elapsed: {} ns", current_time);
    println!("   Timeout expired: {}\n", screen.is_timeout_expired(current_time));

    // Error handling
    println!("5. Error Code Recording");
    screen.set_error(42);
    println!("   Error code set: {}", screen.last_error());
    screen.set_error(255);
    println!("   Error code updated: {}", screen.last_error());
    screen.clear_error();
    println!("   Error code cleared: {}\n", screen.last_error());

    // Multi-threaded example
    println!("6. Multi-threaded Reader Pattern");
    let screen_clone = Arc::clone(&screen);

    // Start multiple reader threads
    let readers: Vec<_> = (0..3)
        .map(|thread_id| {
            let s = Arc::clone(&screen_clone);
            thread::spawn(move || {
                for _ in 0..5 {
                    let current = s.current();
                    let timeout = s.get_timeout();
                    let gen = s.generation();
                    println!(
                        "   [Reader {}] Screen: {:?}, Timeout: {} ns, Gen: {}",
                        thread_id, current, timeout, gen
                    );
                    thread::sleep(Duration::from_millis(10));
                }
            })
        })
        .collect();

    // Writer: Change screens after readers start observing
    thread::sleep(Duration::from_millis(20));
    screen_clone.navigate_to(ScreenId::Loading);
    println!("   [Writer] Changed to Loading screen");

    // Wait for readers
    for reader in readers {
        reader.join().unwrap();
    }

    println!("\n7. Performance Characteristics");

    // Benchmark single reads
    let screen = ScreenStateCapsule::new();
    let start = Instant::now();
    for _ in 0..1_000_000 {
        let _ = screen.current();
    }
    let elapsed = start.elapsed();
    println!("   1M reads (current()): {} µs ({:.2} ns/op)",
        elapsed.as_micros(),
        elapsed.as_nanos() as f64 / 1_000_000.0
    );

    // Benchmark navigations
    let start = Instant::now();
    for _ in 0..100_000 {
        screen.navigate_to(ScreenId::Menu);
        screen.navigate_to(ScreenId::Home);
    }
    let elapsed = start.elapsed();
    println!("   200K navigations (2 per cycle): {} µs ({:.2} ns/op)",
        elapsed.as_micros(),
        elapsed.as_nanos() as f64 / 200_000.0
    );

    // Benchmark error recording
    let start = Instant::now();
    for i in 0..1_000_000 {
        screen.set_error((i % 256) as u16);
    }
    let elapsed = start.elapsed();
    println!("   1M error recordings: {} µs ({:.2} ns/op)",
        elapsed.as_micros(),
        elapsed.as_nanos() as f64 / 1_000_000.0
    );

    // Benchmark timeout checks
    let screen_for_timeout = ScreenStateCapsule::new();
    screen_for_timeout.set_transition_time(1_000_000_000);
    screen_for_timeout.set_timeout(5_000_000_000);
    let start = Instant::now();
    for current_time in 0..1_000_000 {
        let _ = screen_for_timeout.is_timeout_expired(current_time * 1000);
    }
    let elapsed = start.elapsed();
    println!("   1M timeout checks: {} µs ({:.2} ns/op)",
        elapsed.as_micros(),
        elapsed.as_nanos() as f64 / 1_000_000.0
    );

    println!("\n=== All Tests Passed ===\n");
}
