//! Integration tests for retry policies.
//!
//! Validates exponential backoff behavior under simulated contention.

use atomic_capsule::{BackoffStrategy, RetryPolicy};
use core::sync::atomic::{AtomicU64, Ordering};

#[test]
fn test_exponential_backoff_progression() {
    let mut policy = RetryPolicy::new(BackoffStrategy::Exponential {
        initial: 1,
        max: 16,
    });

    // Track delay progression
    let delays: Vec<u32> = (0..6)
        .map(|_| {
            let delay = policy.current_delay;
            policy.increment();
            delay
        })
        .collect();

    // Should double each time: 1, 2, 4, 8, 16, 16 (capped)
    assert_eq!(delays, vec![1, 2, 4, 8, 16, 16]);
}

#[test]
fn test_fixed_backoff_stays_constant() {
    let mut policy = RetryPolicy::new(BackoffStrategy::Fixed { delay: 10 });

    for _ in 0..10 {
        assert_eq!(policy.current_delay, 10);
        policy.increment();
    }
}

#[test]
fn test_no_backoff_stays_zero() {
    let mut policy = RetryPolicy::new(BackoffStrategy::None);

    for _ in 0..10 {
        assert_eq!(policy.current_delay, 0);
        assert!(!policy.should_yield());
        policy.increment();
    }
}

#[test]
fn test_retry_exhaustion() {
    let mut policy = RetryPolicy::new(BackoffStrategy::default()).with_max_iterations(5);

    for i in 0..5 {
        assert!(
            !policy.is_exhausted(),
            "Should not be exhausted at iteration {}",
            i
        );
        policy.increment();
    }

    assert!(
        policy.is_exhausted(),
        "Should be exhausted after 5 iterations"
    );
}

#[test]
fn test_reset_clears_state() {
    let mut policy = RetryPolicy::new(BackoffStrategy::Exponential {
        initial: 2,
        max: 64,
    });

    // Progress through several iterations
    for _ in 0..4 {
        policy.increment();
    }

    assert_eq!(policy.iteration(), 4);
    assert_eq!(policy.current_delay, 32); // 2 * 2^4

    // Reset should clear everything
    policy.reset();

    assert_eq!(policy.iteration(), 0);
    assert_eq!(policy.current_delay, 2);
    assert!(!policy.is_exhausted());
}

#[test]
fn test_default_max_iterations() {
    let mut policy = RetryPolicy::default();
    assert_eq!(policy.max_iterations, 16);
}

#[test]
fn test_should_yield_logic() {
    let mut policy = RetryPolicy::new(BackoffStrategy::default());

    // First attempt - should not yield
    assert!(!policy.should_yield());

    // After first failure - should yield
    policy.increment();
    assert!(policy.should_yield());

    // Subsequent failures - continue yielding
    policy.increment();
    assert!(policy.should_yield());
}

/// Simulate a contended CAS loop with retry policy
#[test]
fn test_cas_loop_with_retry() {
    let atomic = AtomicU64::new(0);
    let mut policy = RetryPolicy::default();
    let mut attempts = 0;

    loop {
        let current = atomic.load(Ordering::Acquire);
        let new = current + 1;

        match atomic.compare_exchange_weak(current, new, Ordering::Release, Ordering::Relaxed) {
            Ok(_) => break,
            Err(_) => {
                attempts += 1;
                if policy.should_yield() {
                    policy.backoff();
                }
                policy.increment();

                // Safety: prevent infinite loop in test
                if attempts > 100 {
                    panic!("CAS loop exceeded max attempts");
                }
            }
        }
    }

    assert_eq!(atomic.load(Ordering::Acquire), 1);
    // Should succeed quickly in single-threaded test
    assert!(
        attempts < 10,
        "CAS should succeed quickly without contention"
    );
}

/// Test backoff completes without panic
#[test]
fn test_backoff_completes() {
    let mut policy = RetryPolicy::new(BackoffStrategy::Exponential {
        initial: 100,
        max: 1000,
    });

    // Should complete without panic
    policy.backoff();

    // Test with higher delay
    let mut policy2 = RetryPolicy::new(BackoffStrategy::Fixed { delay: 500 });
    policy2.backoff();
}

/// Test multiple concurrent retry policies don't interfere
#[test]
fn test_independent_policies() {
    let mut policy1 = RetryPolicy::new(BackoffStrategy::Exponential {
        initial: 1,
        max: 16,
    });

    let mut policy2 = RetryPolicy::new(BackoffStrategy::Exponential {
        initial: 1,
        max: 16,
    });

    // Advance policy1
    policy1.increment();
    policy1.increment();

    // Policy2 should be independent
    assert_eq!(policy1.iteration(), 2);
    assert_eq!(policy2.iteration(), 0);

    assert_eq!(policy1.current_delay, 4);
    assert_eq!(policy2.current_delay, 1);
}

/// Property: Exponential backoff should never exceed max
#[test]
fn test_exponential_never_exceeds_max() {
    let max = 128;
    let mut policy = RetryPolicy::new(BackoffStrategy::Exponential { initial: 1, max });

    // Increment many times
    for _ in 0..100 {
        policy.increment();
        assert!(
            policy.current_delay <= max,
            "Delay should never exceed max: {} > {}",
            policy.current_delay,
            max
        );
    }
}

/// Property: Fixed backoff should stay constant
#[test]
fn test_fixed_always_constant() {
    let delay = 42;
    let mut policy = RetryPolicy::new(BackoffStrategy::Fixed { delay });

    for i in 0..100 {
        assert_eq!(
            policy.current_delay, delay,
            "Fixed delay should be constant at iteration {}",
            i
        );
        policy.increment();
    }
}
