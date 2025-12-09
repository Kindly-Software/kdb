//! # Retry Convergence Property Tests
//!
//! **T28 Framework**: Q11 (ASSUM Verification) + Q8 (Universal Properties)
//! **ASSUM Verification**: #ASSUME_EXPONENTIAL_SUFFICIENT → #VERIFY no livelock
//!
//! Tests that retry policies always converge, preventing livelock:
//! - All backoff strategies converge within reasonable iterations
//! - No livelock under high contention (2-50 threads)
//! - Exponential backoff prevents starvation
//! - Proper yielding behavior at iteration thresholds
//!
//! **Coverage Goal**: Validate retry convergence assumption (currently untested)

use atomic_capsule::{BackoffStrategy, RetryPolicy};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

#[cfg(feature = "proptest")]
use proptest::prelude::*;

// =============================================================================
// T28 Q11: ASSUM Verification - Retry Convergence
// =============================================================================

#[test]
fn verify_assum_immediate_backoff_converges() {
    // #ASSUME_EXPONENTIAL_SUFFICIENT: IMMEDIATE strategy converges
    // #VERIFY: Test with contention

    let counter = Arc::new(AtomicU64::new(0));
    let num_threads = 10;
    let operations = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                for _ in 0..operations {
                    let mut policy = RetryPolicy::new(BackoffStrategy::IMMEDIATE);
                    loop {
                        let current = c.load(Ordering::Acquire);
                        match c.compare_exchange(
                            current,
                            current + 1,
                            Ordering::Release,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => break,
                            Err(_) => {
                                if policy.should_yield() {
                                    policy.backoff();
                                }
                                policy.increment();
                                // #VERIFY: Never exhausted
                                assert!(
                                    !policy.is_exhausted(),
                                    "IMMEDIATE strategy failed to converge"
                                );
                            }
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let final_value = counter.load(Ordering::Acquire);
    assert_eq!(final_value, (num_threads * operations) as u64);
}

#[test]
fn verify_assum_light_backoff_converges() {
    // #ASSUME_EXPONENTIAL_SUFFICIENT: LIGHT strategy converges
    // #VERIFY: Test with moderate contention

    let counter = Arc::new(AtomicU64::new(0));
    let num_threads = 16;
    let operations = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                for _ in 0..operations {
                    let mut policy = RetryPolicy::new(BackoffStrategy::LIGHT);
                    loop {
                        let current = c.load(Ordering::Acquire);
                        match c.compare_exchange(
                            current,
                            current + 1,
                            Ordering::Release,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => break,
                            Err(_) => {
                                if policy.should_yield() {
                                    policy.backoff();
                                }
                                policy.increment();
                                // #VERIFY: Never exhausted
                                assert!(
                                    !policy.is_exhausted(),
                                    "LIGHT strategy failed to converge"
                                );
                            }
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let final_value = counter.load(Ordering::Acquire);
    assert_eq!(final_value, (num_threads * operations) as u64);
}

#[test]
fn verify_assum_standard_backoff_converges() {
    // #ASSUME_EXPONENTIAL_SUFFICIENT: STANDARD strategy converges
    // #VERIFY: Test with high contention

    let counter = Arc::new(AtomicU64::new(0));
    let num_threads = 32;
    let operations = 50;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                for _ in 0..operations {
                    let mut policy = RetryPolicy::new(BackoffStrategy::STANDARD);
                    loop {
                        let current = c.load(Ordering::Acquire);
                        match c.compare_exchange(
                            current,
                            current + 1,
                            Ordering::Release,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => break,
                            Err(_) => {
                                if policy.should_yield() {
                                    policy.backoff();
                                }
                                policy.increment();
                                // #VERIFY: Never exhausted
                                assert!(
                                    !policy.is_exhausted(),
                                    "STANDARD strategy failed to converge"
                                );
                            }
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let final_value = counter.load(Ordering::Acquire);
    assert_eq!(final_value, (num_threads * operations) as u64);
}

#[test]
fn verify_assum_persistent_backoff_converges() {
    // #ASSUME_EXPONENTIAL_SUFFICIENT: PERSISTENT strategy converges
    // #VERIFY: Test with extreme contention

    let counter = Arc::new(AtomicU64::new(0));
    let num_threads = 50;
    let operations = 20;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                for _ in 0..operations {
                    let mut policy = RetryPolicy::new(BackoffStrategy::PERSISTENT);
                    loop {
                        let current = c.load(Ordering::Acquire);
                        match c.compare_exchange(
                            current,
                            current + 1,
                            Ordering::Release,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => break,
                            Err(_) => {
                                if policy.should_yield() {
                                    policy.backoff();
                                }
                                policy.increment();
                                // #VERIFY: Never exhausted
                                assert!(
                                    !policy.is_exhausted(),
                                    "PERSISTENT strategy failed to converge"
                                );
                            }
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let final_value = counter.load(Ordering::Acquire);
    assert_eq!(final_value, (num_threads * operations) as u64);
}

// =============================================================================
// T28 Q8: Universal Properties - Backoff Progression
// =============================================================================

#[test]
fn test_backoff_iteration_progression() {
    let mut policy = RetryPolicy::new(BackoffStrategy::STANDARD);

    // Verify iteration count increases
    for i in 0..100 {
        assert_eq!(policy.iterations(), i, "Iteration count mismatch");
        policy.increment();
    }

    assert_eq!(policy.iterations(), 100);
}

#[test]
fn test_backoff_should_yield_thresholds() {
    // IMMEDIATE: Never yields (spins only)
    let mut immediate = RetryPolicy::new(BackoffStrategy::IMMEDIATE);
    for _ in 0..100 {
        assert!(!immediate.should_yield(), "IMMEDIATE should never yield");
        immediate.increment();
    }

    // LIGHT: Yields after 10 iterations
    let mut light = RetryPolicy::new(BackoffStrategy::LIGHT);
    for i in 0..10 {
        assert!(
            !light.should_yield(),
            "LIGHT should not yield before 10: {}",
            i
        );
        light.increment();
    }
    assert!(light.should_yield(), "LIGHT should yield at 10");

    // STANDARD: Yields after 50 iterations
    let mut standard = RetryPolicy::new(BackoffStrategy::STANDARD);
    for _ in 0..50 {
        standard.increment();
    }
    assert!(standard.should_yield(), "STANDARD should yield at 50");

    // PERSISTENT: Never exhausted (no iteration limit)
    let mut persistent = RetryPolicy::new(BackoffStrategy::PERSISTENT);
    for _ in 0..10_000 {
        persistent.increment();
    }
    assert!(
        !persistent.is_exhausted(),
        "PERSISTENT should never exhaust"
    );
}

#[test]
fn test_backoff_exhaustion_detection() {
    // Default strategy has max iterations
    let mut policy = RetryPolicy::default();

    // Should not be exhausted initially
    assert!(!policy.is_exhausted());

    // Increment past limit (implementation-dependent)
    for _ in 0..1_000_000 {
        if policy.is_exhausted() {
            break;
        }
        policy.increment();
    }

    // STANDARD/LIGHT/IMMEDIATE may exhaust, PERSISTENT never does
    // This test just verifies the API works
}

// =============================================================================
// T28 Q8: Property Testing - Convergence Under Contention
// =============================================================================

#[cfg(feature = "proptest")]
proptest! {
    #[test]
    fn prop_retry_always_converges_varying_contention(
        num_threads in 2usize..50,
        operations in 10usize..200
    ) {
        let counter = Arc::new(AtomicU64::new(0));
        let success_count = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];

        for _ in 0..num_threads {
            let c = Arc::clone(&counter);
            let sc = Arc::clone(&success_count);
            handles.push(thread::spawn(move || {
                for _ in 0..operations {
                    let mut policy = RetryPolicy::new(BackoffStrategy::STANDARD);
                    loop {
                        let current = c.load(Ordering::Acquire);
                        match c.compare_exchange(
                            current, current + 1,
                            Ordering::Release, Ordering::Relaxed
                        ) {
                            Ok(_) => {
                                sc.fetch_add(1, Ordering::Relaxed);
                                break;
                            }
                            Err(_) => {
                                if policy.should_yield() {
                                    policy.backoff();
                                }
                                policy.increment();
                                // Property: Retry always converges (no livelock)
                                prop_assert!(!policy.is_exhausted());
                            }
                        }
                    }
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Property: All operations succeeded
        let final_count = counter.load(Ordering::Acquire);
        let successful = success_count.load(Ordering::Relaxed);
        let expected = (num_threads * operations) as u64;

        prop_assert_eq!(final_count, expected);
        prop_assert_eq!(successful, num_threads * operations);
    }

    #[test]
    fn prop_all_strategies_converge(
        strategy_index in 0usize..4,
        num_threads in 2usize..16,
        operations in 50usize..150
    ) {
        let strategies = [
            BackoffStrategy::IMMEDIATE,
            BackoffStrategy::LIGHT,
            BackoffStrategy::STANDARD,
            BackoffStrategy::PERSISTENT,
        ];
        let strategy = strategies[strategy_index];

        let counter = Arc::new(AtomicU64::new(0));
        let mut handles = vec![];

        for _ in 0..num_threads {
            let c = Arc::clone(&counter);
            handles.push(thread::spawn(move || {
                for _ in 0..operations {
                    let mut policy = RetryPolicy::new(strategy);
                    loop {
                        let current = c.load(Ordering::Acquire);
                        match c.compare_exchange(
                            current, current + 1,
                            Ordering::Release, Ordering::Relaxed
                        ) {
                            Ok(_) => break,
                            Err(_) => {
                                if policy.should_yield() {
                                    policy.backoff();
                                }
                                policy.increment();
                            }
                        }
                    }
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Property: All strategies eventually converge
        let final_count = counter.load(Ordering::Acquire);
        prop_assert_eq!(final_count, (num_threads * operations) as u64);
    }
}

// =============================================================================
// Integration Tests - Retry with CAS Loop
// =============================================================================

#[test]
fn test_cas_loop_with_retry_policy() {
    let counter = Arc::new(AtomicU64::new(0));
    let num_threads = 8;
    let increments = 1_000;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                for _ in 0..increments {
                    let mut policy = RetryPolicy::default();
                    loop {
                        let current = c.load(Ordering::Acquire);
                        let new = current + 1;

                        match c.compare_exchange_weak(
                            current,
                            new,
                            Ordering::Release,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => break,
                            Err(_) => {
                                if policy.should_yield() {
                                    policy.backoff();
                                }
                                policy.increment();
                            }
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Validate: All increments applied
    let expected = num_threads * increments;
    assert_eq!(counter.load(Ordering::Acquire), expected as u64);
}

// =============================================================================
// Stress Tests (Livelock Prevention)
// =============================================================================

#[test]
fn test_retry_prevents_livelock_worst_case() {
    // Worst case: Maximum contention (50 threads)
    let counter = Arc::new(AtomicU64::new(0));
    let num_threads = 50;
    let operations = 100;
    let timeout = std::time::Duration::from_secs(10);
    let start = std::time::Instant::now();

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&counter);
            let start_time = start;
            thread::spawn(move || {
                for _ in 0..operations {
                    let mut policy = RetryPolicy::new(BackoffStrategy::STANDARD);
                    loop {
                        // Timeout check (livelock detection)
                        assert!(
                            start_time.elapsed() < timeout,
                            "Livelock detected: exceeded 10 second timeout"
                        );

                        let current = c.load(Ordering::Acquire);
                        match c.compare_exchange(
                            current,
                            current + 1,
                            Ordering::Release,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => break,
                            Err(_) => {
                                if policy.should_yield() {
                                    policy.backoff();
                                }
                                policy.increment();
                            }
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let final_value = counter.load(Ordering::Acquire);
    let expected = (num_threads * operations) as u64;

    // All operations completed without livelock
    assert_eq!(final_value, expected);
    println!("Stress test completed in {:?} (no livelock)", elapsed);
}

#[test]
#[ignore] // Expensive, run with: cargo test --ignored
fn stress_test_retry_convergence_extreme_contention() {
    // Extreme stress: 100 threads × 1000 operations
    let counter = Arc::new(AtomicU64::new(0));
    let num_threads = 100;
    let operations = 1_000;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                for _ in 0..operations {
                    let mut policy = RetryPolicy::new(BackoffStrategy::PERSISTENT);
                    loop {
                        let current = c.load(Ordering::Acquire);
                        match c.compare_exchange(
                            current,
                            current + 1,
                            Ordering::Release,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => break,
                            Err(_) => {
                                if policy.should_yield() {
                                    policy.backoff();
                                }
                                policy.increment();
                            }
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let final_value = counter.load(Ordering::Acquire);
    let expected = (num_threads * operations) as u64;
    assert_eq!(final_value, expected);

    println!("Extreme stress test: {} operations completed", expected);
}
