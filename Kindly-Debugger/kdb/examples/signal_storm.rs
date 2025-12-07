//! Signal Storm Test - High-frequency SIGUSR1 bombardment
//!
//! This program sends SIGUSR1 signals at 1000 Hz to stress-test
//! debugger signal handling and demonstrate kdb's lockfree signal
//! coordination (T1 Atomic tier).
//!
//! Usage:
//!   cargo run --example signal_storm
//!   # Then attach kdb to observe signal handling performance

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[allow(unused_imports)]
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Signal handler state - atomics for lockfree coordination
static SIGNAL_COUNT: AtomicU64 = AtomicU64::new(0);

/// Signal handler for SIGUSR1
extern "C" fn signal_handler(_sig: libc::c_int) {
    // Lockfree increment - T1 Atomic tier pattern
    SIGNAL_COUNT.fetch_add(1, Ordering::Relaxed);
}

fn main() {
    println!("=== Signal Storm Test ===");
    println!("PID: {}", std::process::id());
    println!("Sending SIGUSR1 at 1000 Hz for 5 seconds\n");

    // Install signal handler
    unsafe {
        libc::signal(libc::SIGUSR1, signal_handler as libc::sighandler_t);
    }

    let pid = std::process::id() as libc::pid_t;
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = Arc::clone(&running);

    // Sender thread - 1000 signals per second
    let sender = thread::spawn(move || {
        let start = Instant::now();
        let mut sent: u64 = 0;

        while running_clone.load(Ordering::Relaxed) && start.elapsed() < Duration::from_secs(5) {
            unsafe {
                libc::kill(pid, libc::SIGUSR1);
            }
            sent += 1;

            // 1ms sleep = 1000 Hz rate
            thread::sleep(Duration::from_micros(1000));

            // Progress report every 500 signals
            if sent % 500 == 0 {
                let received = SIGNAL_COUNT.load(Ordering::Relaxed);
                println!(
                    "[{:.1}s] Sent: {}, Received: {}, Loss: {:.2}%",
                    start.elapsed().as_secs_f64(),
                    sent,
                    received,
                    if sent > 0 {
                        (1.0 - (received as f64 / sent as f64)) * 100.0
                    } else {
                        0.0
                    }
                );
            }
        }
        sent
    });

    // Wait for sender to complete
    let total_sent = sender.join().unwrap();
    running.store(false, Ordering::Relaxed);

    // Final statistics
    let total_received = SIGNAL_COUNT.load(Ordering::Relaxed);
    println!("\n=== Results ===");
    println!("Signals sent:     {}", total_sent);
    println!("Signals received: {}", total_received);
    println!(
        "Signal loss:      {:.2}%",
        if total_sent > 0 {
            (1.0 - (total_received as f64 / total_sent as f64)) * 100.0
        } else {
            0.0
        }
    );

    if total_received >= total_sent {
        println!("\nAll signals handled successfully!");
    } else {
        println!(
            "\nNote: Signal coalescing is expected under high load."
        );
    }

    println!("\nDebug with kdb:");
    println!("  1. Attach: attach {}", std::process::id());
    println!("  2. Set breakpoint on signal_handler");
    println!("  3. Observe lockfree signal coordination");
}
