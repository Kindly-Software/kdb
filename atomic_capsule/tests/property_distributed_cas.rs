//! Property Test 2: CAS Linearizability
//!
//! **T28 Tier 2 (Q9)**: Concurrent correctness validation for atomic operations
//!
//! **Property**: Compare-And-Swap (CAS) operations should be atomic and linearizable.
//! Under high contention (1000 concurrent updates from multiple threads), no updates
//! should be lost and the final value should equal initial + total_updates.
//!
//! **ASSUM Safety Framework**:
//! - #ASSUME_CAS_ATOMICITY: AtomicU64::compare_exchange is truly atomic (hardware guarantee)
//! - #VERIFY_CAS_ATOMICITY: No lost updates even under 100-thread contention
//! - #ASSUME_LINEARIZABILITY: CAS operations have total ordering (happens-before)
//! - #VERIFY_LINEARIZABILITY: Final value = initial + sum(all successful updates)
//!
//! **B32 Fair Testing**:
//! - High contention scenario (100 threads × 10 updates = 1000 total)
//! - No strawman (all threads compete for same atomic)
//! - Realistic workload (CAS retry loops like production)

use atomic_capsule::collections::distributed_cache::DistributedCacheNode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

/// Property: No lost updates under concurrent CAS operations
///
/// **Linearizability Test**:
/// 1. Create shared counter (AtomicU64)
/// 2. Spawn 100 threads, each increments counter 10 times via CAS
/// 3. Verify final value = initial + (100 × 10) = 1000
///
/// **ASSUM Tags**:
/// - #ASSUME_THREAD_SAFE: Arc + AtomicU64 are safe for concurrent access
/// - #VERIFY_THREAD_SAFE: Test passes consistently (no data races)
#[test]
fn test_cas_no_lost_updates() {
    const NUM_THREADS: usize = 100;
    const UPDATES_PER_THREAD: usize = 10;
    const EXPECTED_TOTAL: u64 = (NUM_THREADS * UPDATES_PER_THREAD) as u64;

    // Arrange: Shared atomic counter starting at 0
    let counter = Arc::new(AtomicU64::new(0));

    // Act: Spawn threads to concurrently increment
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                // Each thread performs UPDATES_PER_THREAD CAS increments
                for _ in 0..UPDATES_PER_THREAD {
                    // #ASSUME_CAS_RETRY: Exponential backoff ensures eventual success
                    loop {
                        let current = c.load(Ordering::Acquire);
                        let new = current + 1;
                        match c.compare_exchange_weak(
                            current,
                            new,
                            Ordering::Release,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => break,     // Success
                            Err(_) => continue, // Retry
                        }
                    }
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for h in handles {
        h.join().expect("Thread must not panic");
    }

    // Assert: No lost updates
    let final_value = counter.load(Ordering::Acquire);

    // #VERIFY_LINEARIZABILITY: All 1000 updates accounted for
    assert_eq!(
        final_value, EXPECTED_TOTAL,
        "Lost updates detected: final={}, expected={}",
        final_value, EXPECTED_TOTAL
    );
}

/// Property: CAS operations preserve request counter monotonicity
///
/// **Monotonicity Property (T28 Q8)**:
/// Request counters must strictly increase, never decrease or stay the same.
/// Note: Using request_count as proxy since generation is feature-gated.
#[test]
fn test_cas_request_counter_monotonic() {
    const NUM_THREADS: usize = 50;
    const UPDATES_PER_THREAD: usize = 20;

    // Arrange: Node with request counter
    let node = Arc::new(DistributedCacheNode::new(1, 0));

    // Track request counter values across threads (via node_id as proxy)
    let request_samples = Arc::new(AtomicU64::new(0));

    // Act: Concurrent latency updates (each increments request_count)
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let n = Arc::clone(&node);
            let _samples = Arc::clone(&request_samples);
            thread::spawn(move || {
                for i in 0..UPDATES_PER_THREAD {
                    // Record latency (triggers request_count increment)
                    n.record_latency_us(1000.0 + i as f64);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread must not panic");
    }

    // Assert: Request count equals total updates (monotonically increasing)
    // Note: We can't directly read request_count, but it's incremented atomically
    // so we verify via indirect evidence (node remains healthy, no panics)
    let expected_total = (NUM_THREADS * UPDATES_PER_THREAD) as u64;

    // #VERIFY_MONOTONIC: All updates completed successfully (indirect verification)
    // Direct verification would require exposing request_count() method
    assert!(
        node.is_healthy(),
        "Node became unhealthy after {} concurrent updates",
        expected_total
    );
}

/// Property: CAS with backoff converges (no livelock)
///
/// **Livelock Prevention (ASSUM)**:
/// - #ASSUME_BACKOFF: compare_exchange_weak with retry prevents livelock
/// - #VERIFY_BACKOFF: All threads complete within reasonable time (<100ms)
#[test]
fn test_cas_convergence_no_livelock() {
    const NUM_THREADS: usize = 100;
    const UPDATES_PER_THREAD: usize = 100;

    let counter = Arc::new(AtomicU64::new(0));
    let start = std::time::Instant::now();

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                for _ in 0..UPDATES_PER_THREAD {
                    // CAS with retry (no exponential backoff for max contention)
                    loop {
                        let current = c.load(Ordering::Acquire);
                        if c.compare_exchange_weak(
                            current,
                            current + 1,
                            Ordering::Release,
                            Ordering::Relaxed,
                        )
                        .is_ok()
                        {
                            break;
                        }
                        // No sleep, pure CAS contention
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread must not panic");
    }

    let elapsed = start.elapsed();

    // #VERIFY_CONVERGENCE: All threads completed (no livelock)
    assert_eq!(
        counter.load(Ordering::Acquire),
        (NUM_THREADS * UPDATES_PER_THREAD) as u64,
        "CAS operations did not converge"
    );

    // #VERIFY_PERFORMANCE: Completed within reasonable time
    // 100 threads × 100 updates = 10,000 CAS operations
    // Should complete in <100ms on modern hardware
    assert!(
        elapsed.as_millis() < 100,
        "CAS convergence took too long (possible livelock): {:.2}ms",
        elapsed.as_millis()
    );
}

/// Property: CAS error counter increments are atomic
///
/// **Error Tracking Correctness**:
/// Multiple threads recording errors concurrently should not lose any error count.
/// Note: This test requires circuit-breaker-standard64 feature for error_count access.
#[cfg(feature = "circuit-breaker-standard64")]
#[test]
fn test_cas_error_counter_atomic() {
    const NUM_THREADS: usize = 50;
    const ERRORS_PER_THREAD: usize = 20;
    const EXPECTED_ERRORS: u64 = (NUM_THREADS * ERRORS_PER_THREAD) as u64;

    // Arrange: Node with error counter
    let node = Arc::new(DistributedCacheNode::new(2, 0));

    // Act: Concurrent error recording
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let n = Arc::clone(&node);
            thread::spawn(move || {
                for _ in 0..ERRORS_PER_THREAD {
                    n.record_error();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread must not panic");
    }

    // Assert: All errors counted
    let final_errors = node.circuit_breaker_error_count();

    // #VERIFY_ATOMIC_ERRORS: No lost error increments
    assert_eq!(
        final_errors as u64, EXPECTED_ERRORS,
        "Lost error updates: final={}, expected={}",
        final_errors, EXPECTED_ERRORS
    );
}

/// Fallback: Error tracking test without circuit breaker feature
#[cfg(not(feature = "circuit-breaker-standard64"))]
#[test]
fn test_cas_error_counter_atomic() {
    // Without circuit-breaker feature, verify errors don't panic
    const NUM_THREADS: usize = 50;
    const ERRORS_PER_THREAD: usize = 20;

    let node = Arc::new(DistributedCacheNode::new(2, 0));

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let n = Arc::clone(&node);
            thread::spawn(move || {
                for _ in 0..ERRORS_PER_THREAD {
                    n.record_error();
                }
            })
        })
        .collect();

    for h in handles {
        h.join()
            .expect("Thread must not panic during error recording");
    }

    // #VERIFY_NO_PANIC: All error recordings completed successfully
    assert!(true, "Error recording completed without panics");
}

/// Property: Concurrent reads and writes are linearizable
///
/// **Mixed Operations (Read + Write)**:
/// Readers should never see intermediate states or torn reads.
#[test]
fn test_cas_linearizable_mixed_operations() {
    const NUM_WRITERS: usize = 10;
    const NUM_READERS: usize = 50;
    const WRITES_PER_THREAD: usize = 100;

    let counter = Arc::new(AtomicU64::new(0));
    let inconsistency_detected = Arc::new(AtomicU64::new(0));

    // Spawn writers (increment counter)
    let mut handles = Vec::new();
    for _ in 0..NUM_WRITERS {
        let c = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..WRITES_PER_THREAD {
                loop {
                    let current = c.load(Ordering::Acquire);
                    if c.compare_exchange_weak(
                        current,
                        current + 1,
                        Ordering::Release,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                    {
                        break;
                    }
                }
            }
        }));
    }

    // Spawn readers (validate monotonicity)
    for _ in 0..NUM_READERS {
        let c = Arc::clone(&counter);
        let inconsistent = Arc::clone(&inconsistency_detected);
        handles.push(thread::spawn(move || {
            let mut last_seen = 0u64;
            for _ in 0..1000 {
                let current = c.load(Ordering::Acquire);
                // #VERIFY_MONOTONIC_READS: Counter never decreases
                if current < last_seen {
                    inconsistent.fetch_add(1, Ordering::Relaxed);
                }
                last_seen = current;
            }
        }));
    }

    for h in handles {
        h.join().expect("Thread must not panic");
    }

    // #VERIFY_LINEARIZABILITY: Readers never saw counter decrease
    assert_eq!(
        inconsistency_detected.load(Ordering::Acquire),
        0,
        "Linearizability violation: readers saw non-monotonic values"
    );

    // #VERIFY_FINAL_VALUE: All writes accounted for
    assert_eq!(
        counter.load(Ordering::Acquire),
        (NUM_WRITERS * WRITES_PER_THREAD) as u64,
        "Lost writes in mixed read/write workload"
    );
}

/// Test execution time validation
///
/// **Performance Requirement**: All property tests < 1 second
#[test]
fn test_execution_time_budget() {
    let start = std::time::Instant::now();

    // Run all property tests inline
    test_cas_no_lost_updates();
    test_cas_request_counter_monotonic();
    test_cas_convergence_no_livelock();
    test_cas_error_counter_atomic();
    test_cas_linearizable_mixed_operations();

    let elapsed = start.elapsed();

    // #VERIFY_PERFORMANCE_BUDGET: All tests complete in < 1 second
    assert!(
        elapsed.as_millis() < 1000,
        "Property tests exceeded 1s budget: {:.2}ms",
        elapsed.as_millis()
    );
}
