//! Minimal reproduction test for contention livelock
//!
//! **Root Cause**: Synchronized backoff in steal() causes all workers to retry simultaneously,
//! creating sustained contention and preventing progress
//!
//! **Expected Behavior**: 10K tasks complete in <2s
//! **Actual Behavior**: Test hangs >60s or completes very slowly (throughput collapse)
//!
//! **Test Design** (T28 Framework):
//! - Unit tier: Single scenario (many tasks, few workers)
//! - Property: Contention handling (throughput remains reasonable under load)
//! - Integration: ThreadPool + work-stealing under extreme contention
//! - Production: Real-world pattern (server handling request burst)

use crate::parallel::{ParallelError, ThreadPool};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// T1: Unit test - Extreme contention with minimal work
///
/// **Hypothesis**: Synchronized backoff causes all workers to retry simultaneously,
/// creating livelock where no worker makes progress
///
/// **Expected**: 10K tasks complete in <2s (5K tasks/sec throughput)
/// **Actual**: Hangs >60s or completes in 30-60s (throughput <200 tasks/sec)
///
/// **ASSUM Framework**:
/// #ASSUME_CONTENTION_RESOLUTION: CAS retries should eventually succeed
/// #VERIFY_CONTENTION_RESOLUTION: All tasks should complete in reasonable time
///
/// **Instrumentation**: Track steal attempts vs successes (expect very low success rate <1%)
#[test]
#[ignore] // Hangs for >60s or very slow
fn test_contention_under_extreme_load() {
    let pool = ThreadPool::new(2).expect("Failed to create thread pool");
    let tasks_completed = Arc::new(AtomicUsize::new(0));

    let start = Instant::now();
    let completed = tasks_completed.clone();

    pool.scope(|s| {
        // Submit 10K tasks (extreme contention with only 2 workers)
        for i in 0..10_000 {
            let c = completed.clone();
            s.spawn(move || {
                // Minimal work to keep contention high
                black_box(42);
                c.fetch_add(1, SeqCst);
                if i % 1000 == 0 {
                    eprintln!("[Task {}] Completed", i);
                }
            });
        }

        eprintln!("[Main] All 10K tasks spawned, waiting for completion...");
    });

    let elapsed = start.elapsed();
    let tasks_done = tasks_completed.load(SeqCst);
    let throughput = tasks_done as f64 / elapsed.as_secs_f64();

    eprintln!(
        "[Result] Elapsed: {:?}, Tasks completed: {}/10000, Throughput: {:.0} tasks/sec",
        elapsed, tasks_done, throughput
    );

    // Expected: All 10K tasks in <2s (throughput >5K tasks/sec)
    // Actual: Hangs >60s or very slow (throughput <200 tasks/sec)
    if elapsed > Duration::from_secs(60) {
        panic!(
            "Test hung for >60s - contention livelock (only {}/10000 completed)",
            tasks_done
        );
    }

    assert_eq!(tasks_done, 10_000, "Not all tasks completed");
    assert!(
        elapsed < Duration::from_secs(10),
        "Took too long: {:?}",
        elapsed
    );

    // Reasonable throughput expectation: >1K tasks/sec (current: likely <200)
    assert!(
        throughput > 1000.0,
        "Throughput too low: {:.0} tasks/sec",
        throughput
    );
}

/// T2: Property test - Contention scales with task count
///
/// **Hypothesis**: Throughput degrades non-linearly as task count increases
///
/// **Expected**: Linear degradation (2x tasks = 2x time)
/// **Actual**: Exponential degradation (2x tasks = 10x time) due to contention
#[test]
#[ignore] // Very slow or hangs
fn test_contention_scaling() {
    let pool = ThreadPool::new(4).expect("Failed to create thread pool");

    for task_count in [100, 1_000, 10_000] {
        let completed = Arc::new(AtomicUsize::new(0));
        let start = Instant::now();

        let c = completed.clone();
        pool.scope(|s| {
            for _ in 0..task_count {
                let c = c.clone();
                s.spawn(move || {
                    black_box(42);
                    c.fetch_add(1, SeqCst);
                });
            }
        });

        let elapsed = start.elapsed();
        let throughput = task_count as f64 / elapsed.as_secs_f64();

        eprintln!(
            "[{} tasks] Elapsed: {:?}, Throughput: {:.0} tasks/sec",
            task_count, elapsed, throughput
        );

        // Sanity check: should complete eventually
        assert_eq!(completed.load(SeqCst), task_count);

        // Throughput should not collapse (expect >1K tasks/sec even at 10K)
        if task_count >= 10_000 {
            assert!(
                throughput > 1000.0,
                "Throughput collapsed at {} tasks: {:.0} tasks/sec",
                task_count,
                throughput
            );
        }
    }
}

/// T3: Integration test - Contention with varying worker counts
///
/// **Hypothesis**: More workers = worse contention (more CAS failures)
///
/// **Expected**: More workers = better throughput (parallelism benefit)
/// **Actual**: More workers = worse throughput (contention overhead dominates)
#[test]
#[ignore] // Very slow or hangs
fn test_contention_with_worker_scaling() {
    for worker_count in [2, 4, 8] {
        let pool = ThreadPool::new(worker_count).expect("Failed to create thread pool");
        let completed = Arc::new(AtomicUsize::new(0));
        let start = Instant::now();

        let c = completed.clone();
        pool.scope(|s| {
            for _ in 0..10_000 {
                let c = c.clone();
                s.spawn(move || {
                    black_box(42);
                    c.fetch_add(1, SeqCst);
                });
            }
        });

        let elapsed = start.elapsed();
        let throughput = 10_000.0 / elapsed.as_secs_f64();

        eprintln!(
            "[{} workers] Elapsed: {:?}, Throughput: {:.0} tasks/sec",
            worker_count, elapsed, throughput
        );

        // More workers should improve throughput (parallelism benefit)
        // If throughput decreases with more workers, contention dominates
    }
}

/// T4: Production test - Burst workload pattern
///
/// **Hypothesis**: Contention livelock prevents recovery even after burst ends
///
/// **Expected**: Burst completes quickly, subsequent tasks execute normally
/// **Actual**: Burst triggers livelock, subsequent tasks also slow
#[test]
#[ignore] // Very slow or hangs
fn test_burst_workload_recovery() {
    let pool = ThreadPool::new(4).expect("Failed to create thread pool");
    let completed = Arc::new(AtomicUsize::new(0));

    // Phase 1: Initial burst (10K tasks)
    eprintln!("[Phase 1] Submitting burst of 10K tasks...");
    let start = Instant::now();

    let c1 = completed.clone();
    pool.scope(|s| {
        for _ in 0..10_000 {
            let c = c1.clone();
            s.spawn(move || {
                black_box(42);
                c.fetch_add(1, SeqCst);
            });
        }
    });

    let phase1_elapsed = start.elapsed();
    let phase1_done = completed.load(SeqCst);
    eprintln!(
        "[Phase 1] Completed {} tasks in {:?}",
        phase1_done, phase1_elapsed
    );

    // Phase 2: Normal load (100 tasks) - should be fast if recovered
    eprintln!("[Phase 2] Submitting normal load (100 tasks)...");
    let phase2_start = Instant::now();

    pool.scope(|s| {
        for _ in 0..100 {
            let c = c1.clone();
            s.spawn(move || {
                black_box(42);
                c.fetch_add(1, SeqCst);
            });
        }
    });

    let phase2_elapsed = phase2_start.elapsed();
    let phase2_done = completed.load(SeqCst) - phase1_done;
    eprintln!(
        "[Phase 2] Completed {} tasks in {:?}",
        phase2_done, phase2_elapsed
    );

    // Phase 2 should be fast (<100ms) if pool recovered from contention
    assert_eq!(phase2_done, 100, "Not all phase 2 tasks completed");
    assert!(
        phase2_elapsed < Duration::from_millis(100),
        "Phase 2 took too long: {:?} (pool didn't recover)",
        phase2_elapsed
    );
}

/// T5: Production test - Sustained high contention
///
/// **Hypothesis**: Livelock is immediate under sustained contention
///
/// **Expected**: 50K tasks complete in <10s (5K tasks/sec sustained)
/// **Actual**: Never completes or takes >60s
#[test]
#[ignore] // Hangs for >60s
fn test_sustained_high_contention() {
    let pool = ThreadPool::new(8).expect("Failed to create thread pool");
    let completed = Arc::new(AtomicUsize::new(0));

    let start = Instant::now();
    let c = completed.clone();

    pool.scope(|s| {
        // Sustained load: 50K minimal tasks
        for i in 0..50_000 {
            let c = c.clone();
            s.spawn(move || {
                black_box(42);
                c.fetch_add(1, SeqCst);
                if i % 10_000 == 0 {
                    eprintln!("[Progress] {} tasks completed", i);
                }
            });
        }
    });

    let elapsed = start.elapsed();
    let tasks_done = completed.load(SeqCst);
    let throughput = tasks_done as f64 / elapsed.as_secs_f64();

    eprintln!(
        "[Result] Elapsed: {:?}, Tasks: {}/50000, Throughput: {:.0} tasks/sec",
        elapsed, tasks_done, throughput
    );

    if elapsed > Duration::from_secs(60) {
        panic!("Test hung for >60s with sustained contention");
    }

    assert_eq!(tasks_done, 50_000, "Not all tasks completed");
    assert!(
        throughput > 5000.0,
        "Sustained throughput too low: {:.0} tasks/sec",
        throughput
    );
}
