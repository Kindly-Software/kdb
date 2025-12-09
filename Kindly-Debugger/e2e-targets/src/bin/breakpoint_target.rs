//! Breakpoint testing target with predictable function calls.
//!
//! This binary repeatedly calls a known function, allowing E2E tests to:
//! - Set breakpoints on `target_function`
//! - Verify breakpoint hits
//! - Test breakpoint enable/disable
//! - Validate hit counting
//!
//! # Function Symbol
//! The `target_function` is marked `#[inline(never)]` to ensure it has a
//! stable address for breakpoint testing.

use std::io::Write;

/// Target function for breakpoint testing.
///
/// This function performs a simple computation that cannot be optimized away.
/// The E2E harness sets breakpoints on this function's address.
#[inline(never)]
#[no_mangle]
pub fn target_function(x: u64) -> u64 {
    // Use wrapping_mul to prevent overflow panics
    let result = x.wrapping_mul(31);
    // Prevent dead code elimination
    std::hint::black_box(result)
}

/// Secondary target function for multi-breakpoint testing.
#[inline(never)]
#[no_mangle]
pub fn secondary_target(x: u64) -> u64 {
    let result = x.wrapping_add(17);
    std::hint::black_box(result)
}

/// Entry point marker - useful for testing entry breakpoints.
#[inline(never)]
#[no_mangle]
pub fn entry_marker() {
    std::hint::black_box(());
}

fn main() {
    // Print PID for harness detection
    println!("PID: {}", std::process::id());
    let _ = std::io::stdout().flush();

    // Mark entry point
    entry_marker();

    let mut val = 1u64;
    let mut secondary_val = 1u64;

    for iteration in 0..10000 {
        // Call target function (main breakpoint target)
        val = target_function(val);

        // Call secondary target every 10 iterations
        if iteration % 10 == 0 {
            secondary_val = secondary_target(secondary_val);
        }

        // Sleep to allow breakpoint testing
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Periodic status
        if iteration % 100 == 0 {
            eprintln!(
                "breakpoint_target: iteration {}, val={}, secondary={}",
                iteration, val, secondary_val
            );
        }
    }

    println!("Final: val={}, secondary={}", val, secondary_val);
}
