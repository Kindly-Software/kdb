//! Anti-Debug Target - Ptrace detection test program
//!
//! This program demonstrates common anti-debugging techniques:
//! 1. PTRACE_TRACEME check (Linux-specific)
//! 2. /proc/self/status TracerPid detection
//! 3. Timing-based detection
//!
//! Use this to test kdb's stealth capabilities and debugger evasion.
//!
//! Usage:
//!   cargo run --example anti_debug_target
//!   # Then attach kdb and observe detection behavior

use std::fs;
use std::time::{Duration, Instant};

/// Check if being traced via PTRACE_TRACEME
/// Returns true if debugger detected
fn check_ptrace_traceme() -> bool {
    #[cfg(target_os = "linux")]
    {
        // PTRACE_TRACEME returns -1 with EPERM if already traced
        let result = unsafe { libc::ptrace(libc::PTRACE_TRACEME, 0, 0, 0) };
        if result == -1 {
            return true; // Already being traced
        }
        // Detach self if we successfully attached
        unsafe {
            libc::ptrace(libc::PTRACE_DETACH, 0, 0, 0);
        }
        false
    }
    #[cfg(not(target_os = "linux"))]
    {
        false // Not supported on non-Linux
    }
}

/// Check TracerPid in /proc/self/status
/// Returns Some(pid) if being traced, None otherwise
fn check_tracer_pid() -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        let status = fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            if line.starts_with("TracerPid:") {
                let pid_str = line.split_whitespace().nth(1)?;
                let pid: u32 = pid_str.parse().ok()?;
                if pid != 0 {
                    return Some(pid);
                }
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Timing-based detection
/// Single-step debugging causes significant slowdown
fn check_timing_anomaly() -> bool {
    let iterations = 1000;
    let start = Instant::now();

    // Simple loop that should complete in microseconds
    let mut sum: u64 = 0;
    for i in 0..iterations {
        sum = sum.wrapping_add(i);
        std::hint::black_box(sum);
    }

    let elapsed = start.elapsed();

    // Under debugging with single-stepping, this takes milliseconds
    // Normal execution: < 100 microseconds
    elapsed > Duration::from_millis(10)
}

/// Combined detection with multiple techniques
fn detect_debugger() -> Vec<&'static str> {
    let mut detections = Vec::new();

    if check_ptrace_traceme() {
        detections.push("PTRACE_TRACEME");
    }

    if check_tracer_pid().is_some() {
        detections.push("TracerPid");
    }

    if check_timing_anomaly() {
        detections.push("Timing");
    }

    detections
}

fn main() {
    println!("=== Anti-Debug Target ===");
    println!("PID: {}", std::process::id());
    println!("Testing debugger detection techniques...\n");

    // Initial detection
    println!("[Initial Check]");
    let initial = detect_debugger();
    if initial.is_empty() {
        println!("  No debugger detected");
    } else {
        println!("  Debugger detected via: {:?}", initial);
    }

    // Continuous monitoring loop
    println!("\n[Monitoring Mode - 30 seconds]");
    println!("Attach kdb now to test detection...\n");

    let start = Instant::now();
    let mut last_state: Vec<&str> = Vec::new();
    let mut check_count = 0;

    while start.elapsed() < Duration::from_secs(30) {
        check_count += 1;

        // Run detection every 500ms
        std::thread::sleep(Duration::from_millis(500));

        let current = detect_debugger();

        // Report state changes
        if current != last_state {
            let elapsed = start.elapsed().as_secs_f64();
            if current.is_empty() {
                println!("[{:.1}s] Debugger DETACHED", elapsed);
            } else {
                println!("[{:.1}s] Debugger DETECTED via: {:?}", elapsed, current);
            }
            last_state = current;
        }

        // Periodic status
        if check_count % 10 == 0 {
            let elapsed = start.elapsed().as_secs_f64();
            if let Some(tracer) = check_tracer_pid() {
                println!("[{:.1}s] TracerPid: {}", elapsed, tracer);
            } else {
                println!("[{:.1}s] No tracer", elapsed);
            }
        }
    }

    // Final report
    println!("\n=== Final Report ===");
    let final_state = detect_debugger();
    if final_state.is_empty() {
        println!("Status: NOT debugged");
    } else {
        println!("Status: DEBUGGED via {:?}", final_state);
    }

    println!("\nkdb Test Instructions:");
    println!("  1. Run: ./target/release/examples/anti_debug_target");
    println!("  2. Attach kdb: attach <PID>");
    println!("  3. Observe detection messages");
    println!("  4. Detach and observe state change");
}
