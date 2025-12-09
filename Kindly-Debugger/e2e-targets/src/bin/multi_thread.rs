//! Multi-threaded debugging test target.
//!
//! This binary spawns 4+ threads with distinct work patterns to test:
//! - Thread enumeration
//! - Per-thread register inspection
//! - Thread-specific breakpoints
//! - Concurrent state inspection
//!
//! # Thread Model
//! - Main thread: Coordinates and waits
//! - Worker threads 0-3: Perform distinct computations
//!
//! Each thread has a recognizable work function for breakpoint targeting.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Shared shutdown flag for clean termination.
static RUNNING: AtomicBool = AtomicBool::new(true);

/// Per-thread counters for state verification.
static THREAD_COUNTERS: [AtomicU64; 4] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];

/// Thread 0 work function - arithmetic operations.
#[inline(never)]
#[no_mangle]
pub fn thread_work_0(iteration: u64) -> u64 {
    let result = iteration.wrapping_mul(31).wrapping_add(17);
    std::hint::black_box(result)
}

/// Thread 1 work function - bitwise operations.
#[inline(never)]
#[no_mangle]
pub fn thread_work_1(iteration: u64) -> u64 {
    let result = iteration.rotate_left(13) ^ 0xDEADBEEF;
    std::hint::black_box(result)
}

/// Thread 2 work function - hash-like computation.
#[inline(never)]
#[no_mangle]
pub fn thread_work_2(iteration: u64) -> u64 {
    let mut h = iteration;
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    std::hint::black_box(h)
}

/// Thread 3 work function - Fibonacci-like computation.
#[inline(never)]
#[no_mangle]
pub fn thread_work_3(iteration: u64) -> u64 {
    let a = iteration;
    let b = iteration.wrapping_add(1);
    let result = a.wrapping_add(b);
    std::hint::black_box(result)
}

/// Generic thread work dispatcher.
#[inline(never)]
pub fn thread_work(thread_id: usize, iteration: u64) -> u64 {
    match thread_id {
        0 => thread_work_0(iteration),
        1 => thread_work_1(iteration),
        2 => thread_work_2(iteration),
        3 => thread_work_3(iteration),
        _ => std::hint::black_box(iteration),
    }
}

/// Thread entry point - distinct symbol for each thread.
#[inline(never)]
fn thread_main(thread_id: usize) {
    eprintln!("multi_thread: thread {} started", thread_id);

    let mut iteration = 0u64;

    while RUNNING.load(Ordering::Relaxed) {
        // Perform thread-specific work
        let result = thread_work(thread_id, iteration);

        // Update per-thread counter
        THREAD_COUNTERS[thread_id].store(result, Ordering::Relaxed);

        iteration = iteration.wrapping_add(1);

        // Sleep to allow debugging
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Periodic status
        if iteration % 100 == 0 {
            eprintln!(
                "multi_thread: thread {} iteration {}, result={}",
                thread_id, iteration, result
            );
        }
    }

    eprintln!("multi_thread: thread {} exiting", thread_id);
}

fn main() {
    // Print PID for harness detection
    println!("PID: {}", std::process::id());
    let _ = std::io::stdout().flush();

    eprintln!("multi_thread: spawning 4 worker threads");

    // Spawn 4 worker threads
    let _handles: Vec<_> = (0..4)
        .map(|thread_id| {
            std::thread::Builder::new()
                .name(format!("worker-{}", thread_id))
                .spawn(move || {
                    thread_main(thread_id);
                })
                .expect("Failed to spawn thread")
        })
        .collect();

    eprintln!("multi_thread: all threads spawned, main thread waiting");

    // Main thread waits (can be interrupted by debugger)
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Print summary of thread counters
        let counters: Vec<_> = THREAD_COUNTERS
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect();
        eprintln!("multi_thread: counters = {:?}", counters);
    }

    // Unreachable in normal operation, but included for completeness
    #[allow(unreachable_code)]
    {
        RUNNING.store(false, Ordering::Relaxed);
        for h in _handles {
            h.join().expect("Thread panicked");
        }
        println!("multi_thread: clean shutdown");
    }
}
