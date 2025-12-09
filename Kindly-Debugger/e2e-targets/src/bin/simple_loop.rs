//! Simple infinite loop for basic attach/detach testing.
//!
//! This binary runs an infinite loop with periodic sleeps, providing a stable
//! target for testing:
//! - Process attach (PTRACE_ATTACH)
//! - Process detach (PTRACE_DETACH)
//! - Basic process state inspection
//!
//! # Usage
//! ```bash
//! ./simple_loop &
//! kdb attach <pid>
//! ```

use std::io::Write;

fn main() {
    // Print PID for harness detection
    println!("PID: {}", std::process::id());
    let _ = std::io::stdout().flush();

    // Counter to show the process is alive
    let mut iteration: u64 = 0;

    loop {
        // Increment counter (visible in memory inspection)
        iteration = iteration.wrapping_add(1);

        // Prevent optimization of the counter
        std::hint::black_box(iteration);

        // Sleep to reduce CPU usage while remaining debuggable
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Periodic status output (every 10 seconds)
        if iteration % 100 == 0 {
            eprintln!("simple_loop: iteration {}", iteration);
        }
    }
}
