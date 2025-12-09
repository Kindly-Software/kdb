//! Common utilities for kdb E2E test target binaries.
//!
//! This crate provides test binaries designed to exercise specific debugger
//! capabilities:
//!
//! - `simple_loop`: Basic attach/detach testing
//! - `breakpoint_target`: Breakpoint set/hit validation
//! - `time_travel_target`: Time-travel debugging verification
//! - `stack_deep`: Stack unwinding with 10+ nested frames
//! - `multi_thread`: Multi-threaded debugging scenarios
//!
//! All binaries print "PID: <pid>" immediately for harness detection.

/// Marker function to create a recognizable symbol in binaries.
/// The E2E harness can set breakpoints on this function.
#[inline(never)]
pub fn e2e_marker() {
    // Prevent optimization
    std::hint::black_box(());
}

/// Sleep helper that cannot be optimized away.
#[inline(never)]
pub fn sleep_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

/// Print PID in the expected format for harness detection.
#[inline(never)]
pub fn print_pid() {
    println!("PID: {}", std::process::id());
    // Flush to ensure harness sees output immediately
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_e2e_marker() {
        e2e_marker();
    }

    #[test]
    fn test_print_pid() {
        // Just verify it doesn't panic
        print_pid();
    }
}
