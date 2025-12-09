//! # Retry Optimization Behavior Validation Tests
//!
//! Ensures that Phase 1 retry backoff optimizations maintain correct behavior.
//!
//! ## Test Goals
//!
//! 1. **Correctness**: Optimization doesn't change retry semantics
//! 2. **Convergence**: CAS loops still terminate under contention
//! 3. **Strategy Behavior**: Each strategy behaves as documented
//! 4. **Backward Compatibility**: Existing code continues to work

use atomic_capsule::{BackoffStrategy, RetryPolicy};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// STRATEGY CONSTANT TESTS
// ============================================================================

#[test]
fn test_strategy_constants_defined() {
    // Verify all strategy constants are accessible
    let _ = BackoffStrategy::IMMEDIATE;
    let _ = BackoffStrategy::LIGHT;
    let _ = BackoffStrategy::STANDARD;
    let _ = BackoffStrategy::PERSISTENT;
}

#[test]
fn test_strategy_immediate_is_none() {
    assert_eq!(BackoffStrategy::IMMEDIATE, BackoffStrategy::None);
}

#[test]
fn test_strategy_light_config() {
    match BackoffStrategy::LIGHT {
        BackoffStrategy::Exponential { initial, max } => {
            assert_eq!(initial, 1);
            assert_eq!(max, 8);
        }
        _ => panic!("LIGHT should be Exponential"),
    }
}

#[test]
fn test_strategy_standard_config() {
    match BackoffStrategy::STANDARD {
        BackoffStrategy::Exponential { initial, max } => {
            assert_eq!(initial, 1);
            assert_eq!(max, 256);
        }
        _ => panic!("STANDARD should be Exponential"),
    }
}

#[test]
fn test_strategy_persistent_config() {
    match BackoffStrategy::PERSISTENT {
        BackoffStrategy::Exponential { initial, max } => {
            assert_eq!(initial, 2);
            assert_eq!(max, 128);
        }
        _ => panic!("PERSISTENT should be Exponential"),
    }
}

// ============================================================================
// YIELD THRESHOLD TESTS
// ============================================================================

#[test]
fn test_yield_threshold_immediate() {
    assert_eq!(BackoffStrategy::IMMEDIATE.yield_threshold(), u32::MAX);
}

#[test]
fn test_yield_threshold_light() {
    // LIGHT yields after ~3-5 iterations (standard threshold)
    assert_eq!(BackoffStrategy::LIGHT.yield_threshold(), 5);
}

#[test]
fn test_yield_threshold_standard() {
    assert_eq!(BackoffStrategy::STANDARD.yield_threshold(), 5);
}

#[test]
fn test_yield_threshold_persistent() {
    // PERSISTENT yields after 2 iterations (aggressive)
    assert_eq!(BackoffStrategy::PERSISTENT.yield_threshold(), 2);
}

// ============================================================================
// BACKOFF BEHAVIOR TESTS
// ============================================================================

#[test]
fn test_backoff_increments_iteration() {
    let mut policy = RetryPolicy::new(BackoffStrategy::LIGHT);

    assert_eq!(policy.iteration(), 0);

    policy.backoff();
    assert_eq!(policy.iteration(), 1);

    policy.backoff();
    assert_eq!(policy.iteration(), 2);
}

#[test]
fn test_backoff_immediate_never_yields() {
    let mut policy = RetryPolicy::new(BackoffStrategy::IMMEDIATE);

    // Even after many iterations, should never yield
    for _ in 0..20 {
        assert!(!policy.should_yield());
        policy.backoff();
    }
}

#[test]
fn test_backoff_light_yields_after_threshold() {
    let mut policy = RetryPolicy::new(BackoffStrategy::LIGHT);

    // First few iterations don't yield
    for i in 0..5 {
        assert!(!policy.should_yield(), "Iteration {}", i);
        policy.backoff();
    }

    // After threshold, should yield
    assert!(policy.should_yield());
}

#[test]
fn test_backoff_persistent_yields_quickly() {
    let mut policy = RetryPolicy::new(BackoffStrategy::PERSISTENT);

    // First 2 iterations don't yield
    assert!(!policy.should_yield());
    policy.backoff();

    assert!(!policy.should_yield());
    policy.backoff();

    // After 2 iterations, should yield
    assert!(policy.should_yield());
}

// ============================================================================
// CAS LOOP CONVERGENCE TESTS
// ============================================================================

#[test]
fn test_cas_loop_converges_uncontended() {
    let atomic = AtomicU64::new(0);
    let mut policy = RetryPolicy::new(BackoffStrategy::STANDARD);

    // Single-threaded increment should succeed quickly
    loop {
        let current = atomic.load(Ordering::Acquire);
        let new = current + 1;

        match atomic.compare_exchange_weak(current, new, Ordering::Release, Ordering::Relaxed) {
            Ok(_) => break,
            Err(_) => {
                policy.backoff();
                assert!(
                    !policy.is_exhausted(),
                    "Should not exhaust in uncontended case"
                );
            }
        }
    }

    assert_eq!(atomic.load(Ordering::Acquire), 1);
    assert!(policy.iteration() < 5, "Should succeed in few iterations");
}

#[test]
fn test_cas_loop_converges_light_contention() {
    let atomic = Arc::new(AtomicU64::new(0));
    let atomic_clone = atomic.clone();

    // Spawn thread to create contention
    let handle = std::thread::spawn(move || {
        let mut policy = RetryPolicy::new(BackoffStrategy::LIGHT);

        for _ in 0..100 {
            loop {
                let current = atomic_clone.load(Ordering::Acquire);
                let new = current.wrapping_add(1);

                match atomic_clone.compare_exchange_weak(
                    current,
                    new,
                    Ordering::Release,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(_) => {
                        policy.backoff();
                        if policy.is_exhausted() {
                            panic!("Policy exhausted under light contention");
                        }
                    }
                }
            }
            policy.reset();
        }
    });

    // Main thread increments
    let mut policy = RetryPolicy::new(BackoffStrategy::LIGHT);
    for _ in 0..100 {
        loop {
            let current = atomic.load(Ordering::Acquire);
            let new = current.wrapping_add(1);

            match atomic.compare_exchange_weak(current, new, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => break,
                Err(_) => {
                    policy.backoff();
                    if policy.is_exhausted() {
                        panic!("Policy exhausted under light contention");
                    }
                }
            }
        }
        policy.reset();
    }

    handle.join().unwrap();

    // Both threads incremented 100 times
    assert_eq!(atomic.load(Ordering::Acquire), 200);
}

#[test]
fn test_cas_loop_terminates_with_max_iterations() {
    let atomic = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicBool::new(false));

    let atomic_clone = atomic.clone();
    let done_clone = done.clone();

    // Spawn high-contention threads
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let atomic = atomic_clone.clone();
            let done = done_clone.clone();

            std::thread::spawn(move || {
                let mut policy = RetryPolicy::new(BackoffStrategy::STANDARD);

                while !done.load(Ordering::Relaxed) {
                    loop {
                        let current = atomic.load(Ordering::Acquire);
                        let new = current.wrapping_add(1);

                        match atomic.compare_exchange_weak(
                            current,
                            new,
                            Ordering::Release,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => break,
                            Err(_) => {
                                policy.backoff();
                                if policy.is_exhausted() {
                                    // Safe termination after max iterations
                                    break;
                                }
                            }
                        }
                    }
                    policy.reset();
                }
            })
        })
        .collect();

    // Let threads contend for a bit
    std::thread::sleep(Duration::from_millis(100));

    // Signal threads to stop
    done.store(true, Ordering::Relaxed);

    // All threads should terminate cleanly
    for handle in handles {
        handle.join().unwrap();
    }

    // Should have many successful increments (no deadlock)
    let final_count = atomic.load(Ordering::Acquire);
    assert!(final_count > 0, "Should have successful increments");
}

// ============================================================================
// RESET BEHAVIOR TESTS
// ============================================================================

#[test]
fn test_reset_clears_iteration() {
    let mut policy = RetryPolicy::new(BackoffStrategy::STANDARD);

    policy.backoff();
    policy.backoff();
    policy.backoff();

    assert_eq!(policy.iteration(), 3);

    policy.reset();

    assert_eq!(policy.iteration(), 0);
}

#[test]
fn test_reset_restores_initial_delay() {
    let mut policy = RetryPolicy::new(BackoffStrategy::Exponential {
        initial: 2,
        max: 128,
    });

    assert_eq!(policy.current_delay, 2);

    // Trigger exponential growth
    policy.increment();
    policy.increment();
    assert_eq!(policy.current_delay, 8);

    policy.reset();

    assert_eq!(policy.current_delay, 2);
}

// ============================================================================
// BACKWARD COMPATIBILITY TESTS
// ============================================================================

#[test]
fn test_default_strategy_unchanged() {
    let policy = RetryPolicy::default();

    // Default should still be exponential backoff
    match policy.strategy {
        BackoffStrategy::Exponential { initial, max } => {
            assert_eq!(initial, 1);
            assert_eq!(max, 256);
        }
        _ => panic!("Default strategy changed"),
    }
}

#[test]
fn test_max_iterations_unchanged() {
    let policy = RetryPolicy::default();
    assert_eq!(policy.max_iterations, 16);
}

#[test]
fn test_is_exhausted_behavior_unchanged() {
    let mut policy = RetryPolicy::default().with_max_iterations(3);

    assert!(!policy.is_exhausted());

    policy.backoff();
    assert!(!policy.is_exhausted());

    policy.backoff();
    assert!(!policy.is_exhausted());

    policy.backoff();
    assert!(policy.is_exhausted());
}

// ============================================================================
// PROPERTY-BASED TESTS
// ============================================================================

#[test]
fn test_iteration_always_increases() {
    for strategy in [
        BackoffStrategy::IMMEDIATE,
        BackoffStrategy::LIGHT,
        BackoffStrategy::STANDARD,
        BackoffStrategy::PERSISTENT,
    ] {
        let mut policy = RetryPolicy::new(strategy);

        for expected in 0..20 {
            assert_eq!(policy.iteration(), expected);
            policy.backoff();
        }
    }
}

#[test]
fn test_should_yield_monotonic() {
    // should_yield() should never go from true to false without reset
    for strategy in [
        BackoffStrategy::LIGHT,
        BackoffStrategy::STANDARD,
        BackoffStrategy::PERSISTENT,
    ] {
        let mut policy = RetryPolicy::new(strategy);
        let mut has_yielded = false;

        for _ in 0..20 {
            let should_yield = policy.should_yield();

            if has_yielded {
                assert!(should_yield, "should_yield regressed for {:?}", strategy);
            }

            if should_yield {
                has_yielded = true;
            }

            policy.backoff();
        }

        assert!(has_yielded, "Strategy {:?} never yielded", strategy);
    }
}

#[test]
fn test_backoff_never_panics() {
    // Ensure backoff is robust under all strategies
    for strategy in [
        BackoffStrategy::IMMEDIATE,
        BackoffStrategy::LIGHT,
        BackoffStrategy::STANDARD,
        BackoffStrategy::PERSISTENT,
        BackoffStrategy::None,
        BackoffStrategy::Exponential {
            initial: 1,
            max: 1024,
        },
        BackoffStrategy::Fixed { delay: 100 },
    ] {
        let mut policy = RetryPolicy::new(strategy);

        // Should never panic, even after many iterations
        for _ in 0..100 {
            policy.backoff();
        }
    }
}
