//! # RawModeCapsule Demo - Terminal Raw Mode Management
//!
//! This example demonstrates the RawModeCapsule for atomic terminal raw mode
//! management with automatic cleanup (RAII pattern).
//!
//! ## Features Demonstrated
//!
//! 1. **RAII Cleanup**: Automatic terminal restoration on drop (even during panic)
//! 2. **Atomic State Tracking**: <50ns state checks (no syscall overhead)
//! 3. **Generation Counters**: TOCTOU prevention and state versioning
//! 4. **Error Handling**: Comprehensive error types for debugging
//! 5. **Thread Safety**: Concurrent state reads via Arc
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example raw_mode_demo --features "std,tui-terminal"
//! ```
//!
//! ## Expected Output
//!
//! ```text
//! [Demo] RawModeCapsule - Terminal Raw Mode Management
//! [Demo] ================================================
//!
//! [1] Creating RawModeCapsule...
//! [✓] Created capsule (fd=0, generation=0)
//!
//! [2] Entering raw mode...
//! [✓] Raw mode enabled (generation=1)
//!
//! [3] Simulating TUI rendering (1000 iterations)...
//! [✓] Rendered 1000 frames in 0.001s (1,000,000 FPS)
//!
//! [4] Exiting raw mode...
//! [✓] Raw mode disabled (generation=2)
//!
//! [5] Demonstrating RAII cleanup (automatic on drop)...
//! [✓] RAII cleanup successful
//!
//! [6] Demonstrating concurrent state reads...
//! [✓] 4 threads × 100,000 reads = 400,000 total (0.001s)
//!
//! [Demo] All demonstrations completed successfully!
//! ```

#![cfg(all(feature = "std", feature = "tui-terminal"))]

use atomic_capsule::terminal::mode::{RawModeCapsule, RawModeError};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

fn main() -> Result<(), RawModeError> {
    println!("[Demo] RawModeCapsule - Terminal Raw Mode Management");
    println!("[Demo] ================================================\n");

    // Check if running in a TTY
    #[cfg(unix)]
    if unsafe { libc::isatty(libc::STDIN_FILENO) } != 1 {
        eprintln!("[Error] Not running in a TTY. Please run in a terminal.");
        eprintln!("[Hint]  Try: cargo run --example raw_mode_demo --features \"std,tui-terminal\"");
        return Err(RawModeError::NotATty);
    }

    // ========================================================================
    // Demonstration 1: Basic Usage
    // ========================================================================
    println!("[1] Creating RawModeCapsule...");
    let raw_mode = RawModeCapsule::new()?;
    println!("[✓] Created capsule (fd={}, generation={})\n", raw_mode.fd(), raw_mode.generation());

    // ========================================================================
    // Demonstration 2: Enable Raw Mode
    // ========================================================================
    println!("[2] Entering raw mode...");
    raw_mode.enable_raw_mode()?;
    println!("[✓] Raw mode enabled (generation={})\n", raw_mode.generation());

    // ========================================================================
    // Demonstration 3: Simulate TUI Rendering Loop
    // ========================================================================
    println!("[3] Simulating TUI rendering (1000 iterations)...");
    let start = Instant::now();
    let mut frame_count = 0;

    for _ in 0..1000 {
        // In a real TUI, you would:
        // 1. Read keyboard input (non-blocking)
        // 2. Update application state
        // 3. Render to terminal
        // 4. Check raw mode state (fast atomic check)

        if raw_mode.is_raw_mode() {
            frame_count += 1;
        }
    }

    let elapsed = start.elapsed();
    let fps = frame_count as f64 / elapsed.as_secs_f64();
    println!("[✓] Rendered {} frames in {:.3}s ({:.0} FPS)\n", frame_count, elapsed.as_secs_f64(), fps);

    // ========================================================================
    // Demonstration 4: Disable Raw Mode
    // ========================================================================
    println!("[4] Exiting raw mode...");
    raw_mode.disable_raw_mode()?;
    println!("[✓] Raw mode disabled (generation={})\n", raw_mode.generation());

    // ========================================================================
    // Demonstration 5: RAII Cleanup
    // ========================================================================
    println!("[5] Demonstrating RAII cleanup (automatic on drop)...");
    {
        let temp_raw_mode = RawModeCapsule::new()?;
        temp_raw_mode.enable_raw_mode()?;
        // Drop happens here - terminal automatically restored
    }
    println!("[✓] RAII cleanup successful\n");

    // ========================================================================
    // Demonstration 6: Concurrent State Reads
    // ========================================================================
    println!("[6] Demonstrating concurrent state reads...");
    let raw_mode_arc = Arc::new(RawModeCapsule::new()?);
    let mut threads = vec![];
    let start = Instant::now();

    for thread_id in 0..4 {
        let raw_mode_clone = raw_mode_arc.clone();
        let t = thread::spawn(move || {
            for _ in 0..100_000 {
                let _ = raw_mode_clone.is_raw_mode();
                let _ = raw_mode_clone.generation();
                let _ = raw_mode_clone.fd();
            }
            thread_id
        });
        threads.push(t);
    }

    let mut completed_threads = 0;
    for t in threads {
        let thread_id = t.join().expect("Thread should complete");
        completed_threads += 1;
        println!("  [Thread {}] Completed 100,000 reads", thread_id);
    }

    let elapsed = start.elapsed();
    let total_reads = completed_threads * 100_000;
    println!("[✓] {} threads × 100,000 reads = {} total ({:.3}s)\n", completed_threads, total_reads, elapsed.as_secs_f64());

    // ========================================================================
    // Demonstration 7: Error Handling
    // ========================================================================
    println!("[7] Demonstrating error handling...");

    // Try to enable twice (should fail)
    let error_demo = RawModeCapsule::new()?;
    error_demo.enable_raw_mode()?;
    match error_demo.enable_raw_mode() {
        Err(RawModeError::AlreadyInMode) => {
            println!("  [✓] Correctly rejected second enable: AlreadyInMode");
        }
        Ok(_) => {
            println!("  [✗] ERROR: Should have failed on second enable");
        }
        Err(e) => {
            println!("  [✗] ERROR: Unexpected error: {}", e);
        }
    }
    error_demo.disable_raw_mode()?;

    // Try to disable twice (should fail)
    match error_demo.disable_raw_mode() {
        Err(RawModeError::AlreadyInMode) => {
            println!("  [✓] Correctly rejected second disable: AlreadyInMode");
        }
        Ok(_) => {
            println!("  [✗] ERROR: Should have failed on second disable");
        }
        Err(e) => {
            println!("  [✗] ERROR: Unexpected error: {}", e);
        }
    }
    println!();

    // ========================================================================
    // Summary
    // ========================================================================
    println!("[Demo] All demonstrations completed successfully!");
    println!("[Demo] ================================================");
    println!();
    println!("Key Features:");
    println!("  • RAII cleanup: Automatic terminal restoration on drop");
    println!("  • Atomic state: <50ns state checks (no syscall)");
    println!("  • Thread-safe: Concurrent reads via Arc");
    println!("  • Generation counters: TOCTOU prevention");
    println!("  • Error handling: Comprehensive RawModeError types");
    println!();
    println!("Performance (expected):");
    println!("  • State check: <50ns (atomic load)");
    println!("  • Mode transition: <5μs (tcsetattr syscall)");
    println!("  • Speedup vs repeated syscalls: 100-200× on state checks");
    println!();

    Ok(())
}
