//! Minimal reproduction test for shutdown livelock
//!
//! **Root Cause**: Workers stuck in steal() loop, don't check shutdown flag frequently enough
//!
//! **Expected Behavior**: Scope exits cleanly in <2s (partial task completion acceptable)
//! **Actual Behavior**: Scope hangs >60s (workers can't exit steal loop)
//!
//! **Test Design** (T28 Framework):
//! - Unit tier: Single scenario (submit many tasks, shutdown mid-execution)
//! - Property: Shutdown responsiveness (workers exit steal loop promptly)
//! - Integration: ThreadPool shutdown + scoped execution
//! - Production: Real-world pattern (server shutdown with pending requests)

use crate::parallel::{ParallelError, ThreadPool};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// T1: Unit test - Shutdown exits steal loop
///
/// **Hypothesis**: Workers stuck in steal() loop don't check shutdown flag
///
/// **Expected**: Scope exits cleanly in <2s (some tasks may not complete)
/// **Actual**: Hangs >60s (workers can't exit)
///
/// **ASSUM Framework**:
/// #ASSUME_SHUTDOWN_CHECK: Workers check shutdown flag in steal loop
/// #VERIFY_SHUTDOWN_CHECK: Scope should exit within 2s of shutdown signal
///
/// **Instrumentation**: Count steal attempts vs successes (expect high attempts, low success rate)
#[test]
#[ignore] // Hangs for >60s
fn test_shutdown_exits_steal_loop() {
    let pool = ThreadPool::new(2).expect("Failed to create thread pool");
    let tasks_executed = Arc::new(AtomicUsize::new(0));

    let executed = tasks_executed.clone();
    let start = Instant::now();

    // Spawn scope that submits many tasks
    pool.scope(|s| {
        // Submit 1000 tasks (will overwhelm 2-worker pool)
        for i in 0..1000 {
            let e = executed.clone();
            s.spawn(move || {
                // Simulate 1ms work
                thread::sleep(Duration::from_millis(1));
                e.fetch_add(1, SeqCst);
                if i % 100 == 0 {
                    eprintln!("[Task {}] Executed", i);
                }
            });
        }

        // Give workers time to start processing
        thread::sleep(Duration::from_millis(100));

        eprintln!("[Main] {} tasks spawned, scope will drop soon...", 1000);
        // Scope drops here → shutdown signal sent
        // Workers should detect shutdown and exit steal loop
    });

    let elapsed = start.elapsed();
    let tasks_done = tasks_executed.load(SeqCst);

    eprintln!(
        "[Result] Elapsed: {:?}, Tasks executed: {}/1000",
        elapsed, tasks_done
    );

    // Expected: Scope exits within 2s (some tasks incomplete is acceptable)
    // Actual: Likely hangs >60s (workers stuck in steal loop)
    if elapsed > Duration::from_secs(60) {
        panic!(
            "Test hung for >60s - workers can't exit steal loop (executed {}/1000 tasks)",
            tasks_done
        );
    }

    // We expect some tasks to execute (but not all, since shutdown requested)
    assert!(tasks_done > 0, "No tasks executed before shutdown");
    assert!(
        elapsed < Duration::from_secs(2),
        "Shutdown took too long: {:?}",
        elapsed
    );
}

/// T2: Property test - Shutdown during high contention
///
/// **Hypothesis**: High steal contention prevents shutdown detection
///
/// **Expected**: Scope exits within 2s despite contention
/// **Actual**: Hangs indefinitely (contention blocks shutdown check)
#[test]
#[ignore] // Hangs for >60s
fn test_shutdown_during_contention() {
    let pool = ThreadPool::new(4).expect("Failed to create thread pool");
    let tasks_executed = Arc::new(AtomicUsize::new(0));

    let executed = tasks_executed.clone();
    let start = Instant::now();

    pool.scope(|s| {
        // Submit 10K tiny tasks (maximize contention)
        for i in 0..10_000 {
            let e = executed.clone();
            s.spawn(move || {
                // Minimal work (maximize steal attempts)
                e.fetch_add(1, SeqCst);
                if i % 1000 == 0 {
                    eprintln!("[Task {}] Executed", i);
                }
            });
        }

        // Brief delay to let contention build
        thread::sleep(Duration::from_millis(50));

        eprintln!("[Main] High contention scenario, scope dropping...");
    });

    let elapsed = start.elapsed();
    let tasks_done = tasks_executed.load(SeqCst);

    eprintln!(
        "[Result] Elapsed: {:?}, Tasks executed: {}/10000",
        elapsed, tasks_done
    );

    if elapsed > Duration::from_secs(60) {
        panic!("Test hung for >60s during high contention");
    }

    assert!(
        elapsed < Duration::from_secs(2),
        "Shutdown took too long: {:?}",
        elapsed
    );
}

/// T3: Integration test - Repeated shutdown cycles
///
/// **Hypothesis**: Shutdown detection failure is consistent, not intermittent
///
/// **Expected**: All 5 cycles complete within 10s total
/// **Actual**: First cycle hangs indefinitely
#[test]
#[ignore] // Hangs on first iteration
fn test_repeated_shutdown_cycles() {
    let start = Instant::now();

    for cycle in 0..5 {
        eprintln!("[Cycle {}] Starting...", cycle);
        let cycle_start = Instant::now();

        let pool = ThreadPool::new(2).expect("Failed to create thread pool");
        let executed = Arc::new(AtomicUsize::new(0));

        let e = executed.clone();
        pool.scope(|s| {
            // Submit 100 tasks per cycle
            for _ in 0..100 {
                let e = e.clone();
                s.spawn(move || {
                    thread::sleep(Duration::from_micros(100));
                    e.fetch_add(1, SeqCst);
                });
            }

            thread::sleep(Duration::from_millis(10));
        });

        let cycle_elapsed = cycle_start.elapsed();
        eprintln!(
            "[Cycle {}] Completed in {:?}, executed: {}/100",
            cycle,
            cycle_elapsed,
            executed.load(SeqCst)
        );

        if cycle_elapsed > Duration::from_secs(10) {
            panic!("Cycle {} hung for >10s", cycle);
        }
    }

    let total_elapsed = start.elapsed();
    eprintln!("[Result] All 5 cycles completed in {:?}", total_elapsed);

    assert!(
        total_elapsed < Duration::from_secs(10),
        "Total time too long: {:?}",
        total_elapsed
    );
}

/// T4: Production test - Graceful shutdown under load
///
/// **Hypothesis**: Workers prioritize task execution over shutdown detection
///
/// **Expected**: Shutdown detected within 1s regardless of queue load
/// **Actual**: Shutdown detection delayed until queue empty (never happens)
#[test]
#[ignore] // Hangs for >60s
fn test_graceful_shutdown_under_load() {
    let pool = Arc::new(ThreadPool::new(8).expect("Failed to create thread pool"));
    let executed = Arc::new(AtomicUsize::new(0));

    let e = executed.clone();
    let start = Instant::now();

    // Continuously submit tasks in background
    let pool_clone = Arc::clone(&pool);
    let submit_handle = thread::spawn(move || {
        for i in 0..5000 {
            let _ = pool_clone.push(Box::new(move || {
                thread::sleep(Duration::from_micros(500));
            }));
            if i % 1000 == 0 {
                eprintln!("[Submitter] Submitted {} tasks", i);
            }
        }
    });

    // Wait briefly for queue to fill
    thread::sleep(Duration::from_millis(100));

    // Request shutdown
    eprintln!("[Main] Requesting shutdown with full queue...");
    pool.shutdown();

    // Wait for workers to exit (should be <1s)
    drop(pool);

    submit_handle.join();

    let elapsed = start.elapsed();
    eprintln!("[Result] Shutdown completed in {:?}", elapsed);

    if elapsed > Duration::from_secs(60) {
        panic!("Shutdown hung for >60s");
    }

    assert!(
        elapsed < Duration::from_secs(1),
        "Shutdown took too long: {:?}",
        elapsed
    );
}
