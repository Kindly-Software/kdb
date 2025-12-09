//! Time-travel debugging validation target.
//!
//! This binary modifies state predictably, allowing E2E tests to:
//! - Capture snapshots at known states
//! - Step forward and verify state changes
//! - Step backward and verify state restoration
//! - Validate snapshot capacity and replay accuracy
//!
//! # State Model
//! The program maintains a counter that increments predictably, making it
//! easy to verify that time-travel correctly restores previous states.

use std::io::Write;

/// Global counter for time-travel validation.
/// The E2E harness can read this to verify state changes.
static mut GLOBAL_COUNTER: u64 = 0;

/// State modification function for time-travel testing.
///
/// This function modifies the counter in a predictable way that can be
/// verified during backward stepping.
#[inline(never)]
#[no_mangle]
pub fn modify_state(counter: &mut u64) {
    *counter = counter.wrapping_add(1);
    // Volatile read to prevent optimization
    std::hint::black_box(*counter);
}

/// Checkpoint function - good place to set breakpoints for snapshot capture.
#[inline(never)]
#[no_mangle]
pub fn checkpoint(step: u64) {
    std::hint::black_box(step);
}

/// State snapshot helper - modifies global state for verification.
#[inline(never)]
#[no_mangle]
pub fn update_global() {
    unsafe {
        GLOBAL_COUNTER = GLOBAL_COUNTER.wrapping_add(1);
        std::hint::black_box(GLOBAL_COUNTER);
    }
}

fn main() {
    // Print PID for harness detection
    println!("PID: {}", std::process::id());
    let _ = std::io::stdout().flush();

    let mut counter = 0u64;

    for step in 0..100000 {
        // Mark checkpoint (useful for breakpoint-based snapshot triggers)
        checkpoint(step);

        // Modify local state
        modify_state(&mut counter);

        // Modify global state (verifiable via memory inspection)
        update_global();

        // Very short sleep to allow high-frequency stepping
        std::thread::sleep(std::time::Duration::from_micros(100));

        // Periodic status
        if step % 1000 == 0 {
            let global = unsafe { GLOBAL_COUNTER };
            eprintln!(
                "time_travel_target: step={}, counter={}, global={}",
                step, counter, global
            );
        }
    }

    println!("Final counter: {}", counter);
    println!("Final global: {}", unsafe { GLOBAL_COUNTER });
}
