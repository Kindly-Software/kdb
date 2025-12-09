//! Signal Handler Capsule Demo
//!
//! Demonstrates production-grade Unix signal handling with self-pipe trick.
//!
//! ## Features
//!
//! - **SIGWINCH**: Terminal resize detection with TIOCGWINSZ
//! - **SIGINT**: Graceful shutdown (Ctrl+C)
//! - **SIGTSTP**: Suspend handling (Ctrl+Z)
//! - **SIGCONT**: Resume handling
//!
//! ## Architecture
//!
//! Uses the self-pipe trick for async-signal-safe notification:
//! 1. Signal handlers write to pipe (async-signal-safe)
//! 2. Main loop polls pipe with timeout
//! 3. When readable, check flags and handle signals
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example signal_handler_demo --features tui-terminal
//!
//! # Try these:
//! # - Resize terminal window → See SIGWINCH
//! # - Press Ctrl+C → See SIGINT
//! # - Press Ctrl+Z → See SIGTSTP (suspend)
//! # - Type 'fg' → See SIGCONT (resume)
//! ```

#![cfg(unix)]

use atomic_capsule::terminal::signal::{SignalHandlerCapsule, SignalError};
use std::io::{self, Write};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Signal Handler Demo ===\n");
    println!("This demo shows Unix signal handling with the self-pipe trick.\n");
    println!("Try these actions:");
    println!("  - Resize terminal window → SIGWINCH");
    println!("  - Press Ctrl+C → SIGINT (graceful exit)");
    println!("  - Press Ctrl+Z → SIGTSTP (suspend)");
    println!("  - Type 'fg' after suspend → SIGCONT (resume)");
    println!("\nPress Ctrl+C to exit.\n");

    // Create and register signal handler
    let handler = SignalHandlerCapsule::new()?;
    handler.register()?;

    println!("✓ Signal handler registered (pipe FD: {})\n", handler.pipe_fd());

    // Get initial terminal size
    let (cols, rows) = get_terminal_size()?;
    println!("Initial terminal size: {}×{} (cols×rows)\n", cols, rows);

    let mut iteration = 0u64;

    // Main event loop
    loop {
        iteration += 1;

        // Poll pipe with timeout (simulate application work)
        if poll_pipe_readable(handler.pipe_fd(), Duration::from_millis(500))? {
            // Drain pipe first
            handler.drain_pipe()?;

            // Check which signals were received
            if handler.check_winch() {
                handle_resize()?;
            }

            if handler.check_int() {
                println!("\n✓ SIGINT received (Ctrl+C)");
                println!("Shutting down gracefully...");
                break;
            }

            if handler.check_tstp() {
                handle_suspend(&handler)?;
            }

            if handler.check_cont() {
                handle_resume()?;
            }
        } else {
            // Timeout - no signals, show heartbeat
            if iteration % 10 == 0 {
                print!(".");
                io::stdout().flush()?;
            }
        }
    }

    // Cleanup
    handler.unregister()?;
    println!("✓ Signal handler unregistered");
    println!("\nDemo complete!");

    Ok(())
}

/// Handle SIGWINCH (terminal resize)
fn handle_resize() -> io::Result<()> {
    let (cols, rows) = get_terminal_size()?;
    println!("\n✓ SIGWINCH: Terminal resized to {}×{}", cols, rows);
    Ok(())
}

/// Handle SIGTSTP (suspend - Ctrl+Z)
fn handle_suspend(handler: &SignalHandlerCapsule) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n✓ SIGTSTP: Suspending...");
    println!("  (Restore terminal state here if in raw mode)");

    // In a real TUI application, you would:
    // 1. Restore terminal to normal mode
    // 2. Save application state
    // 3. Re-raise SIGTSTP to actually suspend

    // For demo, we just acknowledge
    println!("  Type 'fg' to resume");

    // Re-raise SIGTSTP to actually suspend the process
    unsafe { libc::raise(libc::SIGTSTP) };

    Ok(())
}

/// Handle SIGCONT (resume after suspend)
fn handle_resume() -> io::Result<()> {
    println!("\n✓ SIGCONT: Resuming...");
    println!("  (Restore raw mode here if needed)");

    // In a real TUI application, you would:
    // 1. Re-enable raw mode
    // 2. Restore screen state
    // 3. Continue rendering

    Ok(())
}

/// Get terminal size using TIOCGWINSZ ioctl
fn get_terminal_size() -> io::Result<(u16, u16)> {
    use libc::{ioctl, winsize, STDOUT_FILENO, TIOCGWINSZ};

    let mut ws: winsize = unsafe { std::mem::zeroed() };

    let ret = unsafe { ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut ws) };

    if ret == -1 {
        return Err(io::Error::last_os_error());
    }

    Ok((ws.ws_col, ws.ws_row))
}

/// Poll pipe for readability with timeout
///
/// Uses poll(2) to wait for pipe to become readable.
/// Returns true if readable, false if timeout.
fn poll_pipe_readable(fd: i32, timeout: Duration) -> io::Result<bool> {
    use libc::{poll, pollfd, POLLIN};

    let mut fds = [pollfd {
        fd,
        events: POLLIN,
        revents: 0,
    }];

    let timeout_ms = timeout.as_millis() as libc::c_int;

    let ret = unsafe { poll(fds.as_mut_ptr(), 1, timeout_ms) };

    if ret == -1 {
        let err = io::Error::last_os_error();
        // Ignore EINTR (interrupted by signal, which is expected)
        if err.raw_os_error() == Some(libc::EINTR) {
            return Ok(false);
        }
        return Err(err);
    }

    if ret == 0 {
        // Timeout
        return Ok(false);
    }

    // Check if POLLIN is set
    Ok((fds[0].revents & POLLIN) != 0)
}
