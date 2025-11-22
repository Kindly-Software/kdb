//! Minimal reproduction test for panic-induced livelock
//!
//! **Root Cause**: Panic in worker task corrupts queue state, blocking subsequent tasks
//!
//! **Expected Behavior**: Test completes in <1s with 2/3 tasks executed (panic isolated)
//! **Actual Behavior**: Test hangs >60s (task 2 never completes)
//!
//! **Test Design** (T28 Framework):
//! - Unit tier: Single scenario (3 tasks: normal, panic, normal)
//! - Property: Panic isolation (queue remains functional after panic)
//! - Integration: ThreadPool + panic handling
//! - Production: Real-world pattern (1 failing task among many)

use crate::parallel::{ParallelError, ThreadPool};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// T1: Unit test - Panic isolation minimal reproducer
///
/// **Hypothesis**: Panic in task 1 corrupts queue state, preventing task 2 from being stolen
///
/// **Expected**: 2 tasks completed (panic is isolated)
/// **Actual**: Hangs waiting for task 2 (queue corruption confirmed)
///
/// **ASSUM Framework**:
/// #ASSUME_PANIC_ISOLATION: Panic in one task should not affect queue
/// #VERIFY_PANIC_ISOLATION: Task 2 should complete despite task 1 panic
///
/// **Instrumentation**: Run with queue_debug module to see steal attempts
#[test]
#[ignore] // Hangs for >60s, only run manually
fn test_panic_queue_corruption() {
    let pool = ThreadPool::new(2).expect("Failed to create thread pool");
    let tasks_completed = Arc::new(AtomicUsize::new(0));

    let start = Instant::now();
    let completed = tasks_completed.clone();

    // Use catch_unwind to prevent test runner from aborting
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pool.scope(|s| {
            // Task 0: Normal execution
            let c0 = completed.clone();
            s.spawn(move || {
                c0.fetch_add(1, SeqCst);
                eprintln!("[Task 0] Completed normally");
            });

            // Task 1: Panics (simulates error in worker)
            s.spawn(|| {
                eprintln!("[Task 1] About to panic...");
                panic!("Expected panic - testing queue corruption");
            });

            // Task 2: Normal execution (will hang if queue corrupted)
            let c2 = completed.clone();
            s.spawn(move || {
                c2.fetch_add(1, SeqCst);
                eprintln!("[Task 2] Completed normally");
            });

            eprintln!("[Main] All 3 tasks spawned, waiting for scope exit...");
        });
    }));

    let elapsed = start.elapsed();
    let tasks_done = tasks_completed.load(SeqCst);

    eprintln!(
        "[Result] Elapsed: {:?}, Tasks completed: {}/3",
        elapsed, tasks_done
    );

    // Expected: 2 tasks completed (task 1 panicked, tasks 0 and 2 succeeded)
    // Actual: Likely hangs here or completes with <2 tasks
    if elapsed > Duration::from_secs(60) {
        panic!(
            "Test hung for >60s - queue corruption confirmed (only {}/2 tasks completed)",
            tasks_done
        );
    }

    assert_eq!(
        tasks_done, 2,
        "Expected 2 tasks to complete (task 1 panicked)"
    );
    assert!(result.is_err(), "Expected scope to propagate panic");
}

/// T2: Property test - Multiple panics don't deadlock
///
/// **Hypothesis**: Multiple concurrent panics completely corrupt queue state
///
/// **Expected**: Some tasks complete, scope propagates panic
/// **Actual**: Likely deadlocks with zero completions
#[test]
#[ignore] // Hangs for >60s
fn test_multiple_panics() {
    let pool = ThreadPool::new(4).expect("Failed to create thread pool");
    let tasks_completed = Arc::new(AtomicUsize::new(0));

    let completed = tasks_completed.clone();
    let start = Instant::now();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pool.scope(|s| {
            // Spawn 10 tasks: 3 panic, 7 succeed
            for i in 0..10 {
                let c = completed.clone();
                s.spawn(move || {
                    if i % 3 == 0 {
                        panic!("Panic in task {}", i);
                    } else {
                        c.fetch_add(1, SeqCst);
                        eprintln!("[Task {}] Completed", i);
                    }
                });
            }
        });
    }));

    let elapsed = start.elapsed();
    let tasks_done = tasks_completed.load(SeqCst);

    eprintln!(
        "[Result] Elapsed: {:?}, Tasks completed: {}/7",
        elapsed, tasks_done
    );

    if elapsed > Duration::from_secs(60) {
        panic!("Test hung for >60s with multiple panics");
    }

    // At least some non-panicking tasks should complete
    assert!(
        tasks_done >= 3,
        "Expected at least 3 tasks to complete, got {}",
        tasks_done
    );
}

/// T3: Integration test - Panic recovery pattern
///
/// **Hypothesis**: Queue state corruption is permanent (not transient)
///
/// **Expected**: Subsequent tasks after panic should execute normally
/// **Actual**: Queue remains corrupted, subsequent tasks never execute
#[test]
#[ignore] // Hangs for >60s
fn test_panic_recovery() {
    let pool = ThreadPool::new(2).expect("Failed to create thread pool");
    let completed = Arc::new(AtomicUsize::new(0));

    // Phase 1: Induce panic
    eprintln!("[Phase 1] Inducing panic...");
    let c1 = completed.clone();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pool.scope(|s| {
            s.spawn(|| {
                panic!("Intentional panic");
            });
        });
    }));

    // Phase 2: Try to use pool normally (will hang if queue corrupted)
    eprintln!("[Phase 2] Attempting normal execution after panic...");
    let start = Instant::now();

    pool.scope(|s| {
        for i in 0..5 {
            let c = c1.clone();
            s.spawn(move || {
                c.fetch_add(1, SeqCst);
                eprintln!("[Task {}] Completed in phase 2", i);
            });
        }
    });

    let elapsed = start.elapsed();
    let tasks_done = completed.load(SeqCst);

    eprintln!(
        "[Result] Elapsed: {:?}, Tasks completed: {}/5",
        elapsed, tasks_done
    );

    if elapsed > Duration::from_secs(60) {
        panic!("Test hung for >60s - queue state permanently corrupted");
    }

    assert_eq!(
        tasks_done, 5,
        "Expected all 5 tasks to complete after panic recovery"
    );
}
