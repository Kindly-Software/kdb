//! Comprehensive Safety Validation Tests for AtomicHedgeCapsule
//!
//! This test suite validates all safety fixes using UCE32 framework principles:
//! - Thread safety validation (Q31 Rust Transform)
//! - ABA problem prevention (Q30 Empirical Validation)
//! - Overflow protection (Q29 Practical Constraints)
//! - Memory ordering correctness (ASSUM Safety Framework)
//! - Performance regression tests (B32 Benchmark Framework)
//!
//! Based on CLAUDE.md framework: UCE32 Task Classification = coordination-systems (complexity 6-8)
//! Applying: lockfree-testing, contention-testing, race-detection

use atomic_hedge_capsule::{
    types::{BracketOrder, EntryOrder, HedgeStateSnapshot, OrderState},
    AtomicHedgeCapsule, HedgeError, HedgeState,
};
use proptest::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// CONSTANTS FOR TESTING
// ============================================================================

const STRESS_TEST_THREADS: usize = 16;
const STRESS_TEST_OPERATIONS: usize = 1000;
const CONTENTION_TEST_THREADS: usize = 32;
const CONTENTION_TEST_DURATION_MS: u64 = 100;
const ABA_TEST_ITERATIONS: usize = 10000;
const PERFORMANCE_BASELINE_NS: u64 = 1000; // 1μs baseline

// ============================================================================
// 1. THREAD SAFETY VALIDATION TESTS
// ============================================================================

#[test]
fn test_concurrent_initialization() {
    // Test multiple threads attempting to initialize simultaneously
    let capsule = Arc::new(AtomicHedgeCapsule::new());
    let barrier = Arc::new(Barrier::new(STRESS_TEST_THREADS));
    let success_count = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..STRESS_TEST_THREADS)
        .map(|i| {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);
            let success_count = Arc::clone(&success_count);

            thread::spawn(move || {
                barrier.wait();

                let entry = EntryOrder::new(
                    format!("EXCHANGE_{}", i),
                    "BTCUSD".to_string(),
                    "Buy".to_string(),
                    1.0,
                );
                let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);

                if capsule.initialize(entry, bracket).is_ok() {
                    success_count.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Only one initialization should succeed due to race prevention
    assert_eq!(success_count.load(Ordering::Relaxed), 1);
    assert!(capsule.is_active());
}

#[test]
fn test_concurrent_state_updates() {
    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize once
    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let barrier = Arc::new(Barrier::new(STRESS_TEST_THREADS));
    let operation_count = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..STRESS_TEST_THREADS)
        .map(|i| {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);
            let operation_count = Arc::clone(&operation_count);

            thread::spawn(move || {
                barrier.wait();

                for j in 0..STRESS_TEST_OPERATIONS {
                    let filled = (i as f64 + j as f64) / 1000.0;
                    let state = if j % 2 == 0 {
                        OrderState::PartiallyFilled
                    } else {
                        OrderState::Filled
                    };

                    if capsule.update_entry_state(state, filled).is_ok() {
                        operation_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify state consistency
    let final_state = capsule.get_hedge_state();
    assert!(final_state.operation_count > 0);
    assert!(!capsule.is_emergency_stopped());

    println!(
        "Successful operations: {}",
        operation_count.load(Ordering::Relaxed)
    );
    println!("Final generation: {}", final_state.operation_count);
}

#[test]
fn test_emergency_stop_thread_safety() {
    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let barrier = Arc::new(Barrier::new(STRESS_TEST_THREADS));
    let emergency_calls = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..STRESS_TEST_THREADS)
        .map(|i| {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);
            let emergency_calls = Arc::clone(&emergency_calls);

            thread::spawn(move || {
                barrier.wait();

                if capsule
                    .emergency_stop(&format!("Emergency from thread {}", i))
                    .is_ok()
                {
                    emergency_calls.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // All emergency calls should succeed (idempotent operation)
    assert!(capsule.is_emergency_stopped());
    assert_eq!(
        emergency_calls.load(Ordering::Relaxed),
        STRESS_TEST_THREADS as u64
    );
}

// ============================================================================
// 2. ABA PROBLEM PREVENTION TESTS
// ============================================================================

#[test]
fn test_generation_counter_aba_prevention() {
    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let aba_detected = Arc::new(AtomicBool::new(false));

    let capsule_1 = Arc::clone(&capsule);
    let capsule_2 = Arc::clone(&capsule);
    let barrier_1 = Arc::clone(&barrier);
    let barrier_2 = Arc::clone(&barrier);
    let aba_detected_1 = Arc::clone(&aba_detected);

    // Thread 1: Prepare, then wait, then commit with stale generation
    let handle1 = thread::spawn(move || {
        let gen = capsule_1.prepare_update().unwrap();
        barrier_1.wait();

        // Attempt to commit with potentially stale generation
        match capsule_1.commit_update(gen, OrderState::Filled, 1.0) {
            Err(HedgeError::StateUpdateFailed(_)) => {
                aba_detected_1.store(true, Ordering::Relaxed);
            }
            _ => {}
        }
    });

    // Thread 2: Make intervening updates to advance generation
    let handle2 = thread::spawn(move || {
        barrier_2.wait();

        // Multiple rapid updates to advance generation counter
        for _ in 0..10 {
            let _ = capsule_2.update_entry_state(OrderState::PartiallyFilled, 0.5);
        }
    });

    handle1.join().unwrap();
    handle2.join().unwrap();

    // ABA should be prevented by generation counter mismatch
    // Note: This test is probabilistic - may not always trigger ABA
    println!(
        "ABA prevention test completed. ABA detected: {}",
        aba_detected.load(Ordering::Relaxed)
    );
}

#[test]
fn test_compare_exchange_aba_protection() {
    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let retry_count = Arc::new(AtomicU64::new(0));
    let success_count = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..ABA_TEST_ITERATIONS)
        .map(|_| {
            let capsule = Arc::clone(&capsule);
            let retry_count = Arc::clone(&retry_count);
            let success_count = Arc::clone(&success_count);

            thread::spawn(move || {
                let mut retries = 0;
                loop {
                    match capsule.update_entry_state(OrderState::PartiallyFilled, 0.1) {
                        Ok(_) => {
                            success_count.fetch_add(1, Ordering::Relaxed);
                            break;
                        }
                        Err(_) => {
                            retries += 1;
                            if retries > 100 {
                                break; // Prevent infinite loops
                            }
                        }
                    }
                }
                retry_count.fetch_add(retries, Ordering::Relaxed);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let total_retries = retry_count.load(Ordering::Relaxed);
    let total_successes = success_count.load(Ordering::Relaxed);

    println!(
        "ABA Protection Test - Total retries: {}, Successes: {}",
        total_retries, total_successes
    );

    // Expect high success rate with reasonable retry counts
    assert!(total_successes > ABA_TEST_ITERATIONS as u64 / 2);
    assert!(total_retries < ABA_TEST_ITERATIONS as u64 * 10); // Reasonable retry bound
}

// ============================================================================
// 3. OVERFLOW PROTECTION TESTS
// ============================================================================

#[test]
fn test_u128_position_packing_overflow() {
    let capsule = AtomicHedgeCapsule::new();

    // Test maximum values that should not overflow
    let max_state = HedgeState::Emergency;
    let max_generation = 0xFFFF_u16; // 16 bits
    let max_size = 0xFFFF_FFFF_u32; // 32 bits
    let max_profit = 0xFFFF_u16; // 16 bits

    // This should work without panic or overflow
    let packed = AtomicHedgeCapsule::pack_position(max_state, max_generation, max_size, max_profit);

    // Verify extraction works correctly
    let extracted_state = AtomicHedgeCapsule::extract_state(packed);
    let (extracted_gen, extracted_size, extracted_profit) =
        AtomicHedgeCapsule::extract_position_data(packed);

    assert_eq!(extracted_state, max_state);
    assert_eq!(extracted_gen, max_generation);
    assert_eq!(extracted_size, max_size);
    assert_eq!(extracted_profit, max_profit);
}

#[test]
fn test_fixed_point_conversion_overflow() {
    let capsule = AtomicHedgeCapsule::new();

    // Initialize
    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    // Test various edge cases for filled amounts
    let test_cases = vec![
        0.0,         // Minimum
        0.000001,    // Very small
        1.0,         // Normal
        1000.0,      // Large
        4294.967295, // Near u32 max when multiplied by 1M
        f64::MAX,    // Would overflow - should be handled gracefully
    ];

    for &filled in &test_cases {
        let result = capsule.update_entry_state(OrderState::PartiallyFilled, filled);

        if filled <= 4294.967295 {
            // Should succeed for reasonable values
            assert!(result.is_ok(), "Failed for filled amount: {}", filled);
        } else {
            // May fail for extreme values - should not panic
            // Just ensure no panic occurs
            let _ = result;
        }
    }
}

#[test]
fn test_generation_counter_overflow() {
    let capsule = AtomicHedgeCapsule::new();

    // Initialize
    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    // Rapidly increment generation counter to test wrap-around behavior
    let start_gen = capsule.increment_generation().unwrap();

    // Increment many times to test overflow behavior
    for _ in 0..1000 {
        let _ = capsule.increment_generation();
    }

    let end_gen = capsule.increment_generation().unwrap();

    // Should handle wraparound gracefully
    assert!(end_gen > start_gen);
    assert!(end_gen - start_gen >= 1000);
}

// ============================================================================
// 4. MEMORY ORDERING CORRECTNESS TESTS
// ============================================================================

#[test]
fn test_acquire_release_ordering() {
    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let ordering_verified = Arc::new(AtomicBool::new(false));

    let capsule_writer = Arc::clone(&capsule);
    let capsule_reader = Arc::clone(&capsule);
    let barrier_writer = Arc::clone(&barrier);
    let barrier_reader = Arc::clone(&barrier);
    let ordering_verified_reader = Arc::clone(&ordering_verified);

    // Writer thread: Update state with Release ordering
    let writer = thread::spawn(move || {
        barrier_writer.wait();

        // This update uses Release ordering in implementation
        capsule_writer
            .update_entry_state(OrderState::Filled, 1.0)
            .unwrap();
    });

    // Reader thread: Read state with Acquire ordering
    let reader = thread::spawn(move || {
        barrier_reader.wait();

        // Small delay to ensure writer goes first
        thread::sleep(Duration::from_micros(10));

        // This read uses Acquire ordering in implementation
        let state = capsule_reader.get_hedge_state();

        // If Acquire/Release ordering works correctly, we should see the update
        if state.filled_size > 0.0 {
            ordering_verified_reader.store(true, Ordering::Relaxed);
        }
    });

    writer.join().unwrap();
    reader.join().unwrap();

    // Memory ordering should ensure visibility
    assert!(ordering_verified.load(Ordering::Relaxed));
}

#[test]
fn test_sequential_consistency_emergency() {
    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let barrier = Arc::new(Barrier::new(CONTENTION_TEST_THREADS));
    let emergency_observed = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..CONTENTION_TEST_THREADS)
        .map(|i| {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);
            let emergency_observed = Arc::clone(&emergency_observed);

            thread::spawn(move || {
                barrier.wait();

                if i == 0 {
                    // First thread triggers emergency with SeqCst ordering
                    capsule.emergency_stop("Test emergency").unwrap();
                } else {
                    // Other threads should observe emergency state
                    thread::sleep(Duration::from_micros(1));
                    if capsule.is_emergency_stopped() {
                        emergency_observed.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // SeqCst ordering should ensure all threads observe emergency state
    let observed_count = emergency_observed.load(Ordering::Relaxed);
    println!("Emergency state observed by {} threads", observed_count);

    // Most threads should observe the emergency state due to SeqCst ordering
    assert!(observed_count >= (CONTENTION_TEST_THREADS - 1) as u64 / 2);
    assert!(capsule.is_emergency_stopped());
}

// ============================================================================
// 5. PERFORMANCE REGRESSION TESTS
// ============================================================================

#[test]
fn test_single_thread_performance_baseline() {
    let capsule = AtomicHedgeCapsule::new();

    // Initialize
    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    // Warm up
    for _ in 0..100 {
        capsule
            .update_entry_state(OrderState::PartiallyFilled, 0.5)
            .unwrap();
    }

    // Measure performance
    let iterations = 10000;
    let start = Instant::now();

    for i in 0..iterations {
        let filled = (i as f64) / (iterations as f64);
        capsule
            .update_entry_state(OrderState::PartiallyFilled, filled)
            .unwrap();
    }

    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() / iterations as u128;

    println!("Single-thread performance: {} ns/operation", ns_per_op);

    // Performance should be well under 1μs per operation for lockfree implementation
    assert!(ns_per_op < PERFORMANCE_BASELINE_NS as u128);
}

#[test]
fn test_contention_performance_scaling() {
    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let operations_per_thread = 1000;
    let total_operations = Arc::new(AtomicU64::new(0));

    let start = Instant::now();

    let handles: Vec<_> = (0..CONTENTION_TEST_THREADS)
        .map(|_| {
            let capsule = Arc::clone(&capsule);
            let total_operations = Arc::clone(&total_operations);

            thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let filled = (i as f64) / (operations_per_thread as f64);
                    if capsule
                        .update_entry_state(OrderState::PartiallyFilled, filled)
                        .is_ok()
                    {
                        total_operations.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = total_operations.load(Ordering::Relaxed);
    let ns_per_op = elapsed.as_nanos() / total_ops as u128;

    println!(
        "Contention performance: {} ns/operation ({} threads, {} total ops)",
        ns_per_op, CONTENTION_TEST_THREADS, total_ops
    );

    // Should maintain reasonable performance under contention
    // Allow 10x degradation under high contention as acceptable
    assert!(ns_per_op < PERFORMANCE_BASELINE_NS as u128 * 10);

    // Should achieve reasonable success rate under high contention
    let expected_ops = CONTENTION_TEST_THREADS * operations_per_thread;
    assert!(total_ops > (expected_ops as u64) / 4); // At least 25% success rate under high contention
}

// ============================================================================
// 6. PROPERTY-BASED TESTING
// ============================================================================

proptest! {
    #[test]
    fn prop_test_state_transitions(
        states in prop::collection::vec(
            prop::sample::select(vec![
                OrderState::PendingValidation,
                OrderState::Validated,
                OrderState::PartiallyFilled,
                OrderState::Filled,
            ]),
            1..100
        ),
        filled_values in prop::collection::vec(0.0f64..10.0, 1..100)
    ) {
        let capsule = AtomicHedgeCapsule::new();

        // Initialize
        let entry = EntryOrder::new("NDAX".to_string(), "BTCUSD".to_string(), "Buy".to_string(), 1.0);
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        capsule.initialize(entry, bracket).unwrap();

        // Apply random state transitions
        for (state, filled) in states.iter().zip(filled_values.iter()) {
            let _ = capsule.update_entry_state(*state, *filled);

            // Invariants that should always hold
            let snapshot = capsule.get_hedge_state();
            assert!(snapshot.operation_count > 0);
            assert!(snapshot.filled_size >= 0.0);
        }

        // Final consistency check
        assert!(capsule.is_active());
    }

    #[test]
    fn prop_test_concurrent_operations(
        thread_counts in 2..17usize,
        operation_counts in 10..101usize
    ) {
        let capsule = Arc::new(AtomicHedgeCapsule::new());

        // Initialize
        let entry = EntryOrder::new("NDAX".to_string(), "BTCUSD".to_string(), "Buy".to_string(), 1.0);
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        capsule.initialize(entry, bracket).unwrap();

        let barrier = Arc::new(Barrier::new(thread_counts));
        let success_count = Arc::new(AtomicU64::new(0));

        let handles: Vec<_> = (0..thread_counts).map(|_| {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);
            let success_count = Arc::clone(&success_count);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..operation_counts {
                    let filled = (i as f64) / (operation_counts as f64);
                    if capsule.update_entry_state(OrderState::PartiallyFilled, filled).is_ok() {
                        success_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        }).collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // Should achieve reasonable success rate under any concurrency level
        let total_expected = thread_counts * operation_counts;
        let actual_success = success_count.load(Ordering::Relaxed);

        // At least 10% success rate (very conservative for property testing)
        assert!(actual_success >= (total_expected as u64) / 10);
    }
}

// ============================================================================
// 7. STRESS TEST SUITE
// ============================================================================

#[test]
#[ignore] // Run with --ignored for full stress testing
fn stress_test_extended_duration() {
    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let duration = Duration::from_secs(30); // 30-second stress test
    let start_time = Instant::now();
    let operation_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..STRESS_TEST_THREADS)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let operation_count = Arc::clone(&operation_count);
            let error_count = Arc::clone(&error_count);

            thread::spawn(move || {
                let mut local_ops = 0u64;
                let mut local_errors = 0u64;

                while start_time.elapsed() < duration {
                    let filled = (local_ops as f64) / 1000.0;
                    let state = if local_ops % 3 == 0 {
                        OrderState::PartiallyFilled
                    } else {
                        OrderState::Filled
                    };

                    match capsule.update_entry_state(state, filled) {
                        Ok(_) => local_ops += 1,
                        Err(_) => local_errors += 1,
                    }

                    // Very rare emergency stop test to avoid disrupting the stress test
                    if local_ops == 50000 && thread_id == 0 {
                        let _ = capsule.emergency_stop("Rare test emergency");
                    }

                    // Brief pause to allow other threads
                    if local_ops % 100 == 0 {
                        thread::sleep(Duration::from_nanos(1));
                    }
                }

                operation_count.fetch_add(local_ops, Ordering::Relaxed);
                error_count.fetch_add(local_errors, Ordering::Relaxed);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let total_ops = operation_count.load(Ordering::Relaxed);
    let total_errors = error_count.load(Ordering::Relaxed);
    let elapsed = start_time.elapsed();

    println!("Stress test results:");
    println!("  Duration: {:?}", elapsed);
    println!("  Total operations: {}", total_ops);
    println!("  Total errors: {}", total_errors);
    println!(
        "  Operations/second: {:.2}",
        total_ops as f64 / elapsed.as_secs_f64()
    );
    println!(
        "  Error rate: {:.2}%",
        (total_errors as f64 / (total_ops + total_errors) as f64) * 100.0
    );

    // Should maintain high throughput and low error rate
    assert!(total_ops > 100_000); // Reasonable minimum throughput
    assert!((total_errors as f64 / (total_ops + total_errors) as f64) < 0.5); // Less than 50% error rate
}

// ============================================================================
// 8. INTEGRATION TESTS
// ============================================================================

#[test]
fn test_full_lifecycle_thread_safety() {
    let capsule = Arc::new(AtomicHedgeCapsule::new());
    let barrier = Arc::new(Barrier::new(4));

    // Test full lifecycle across multiple threads
    let handles = vec![
        // Thread 1: Initialize
        {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let entry = EntryOrder::new(
                    "NDAX".to_string(),
                    "BTCUSD".to_string(),
                    "Buy".to_string(),
                    1.0,
                );
                let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
                capsule.initialize(entry, bracket).unwrap();
            })
        },
        // Thread 2: Update states
        {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                thread::sleep(Duration::from_millis(1)); // Ensure initialization happens first

                for i in 0..100 {
                    let filled = (i as f64) / 100.0;
                    let _ = capsule.update_entry_state(OrderState::PartiallyFilled, filled);
                }
            })
        },
        // Thread 3: Read states
        {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                thread::sleep(Duration::from_millis(2));

                for _ in 0..50 {
                    let _ = capsule.get_hedge_state();
                    thread::sleep(Duration::from_micros(100));
                }
            })
        },
        // Thread 4: Emergency operations
        {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                thread::sleep(Duration::from_millis(5));

                let _ = capsule.emergency_stop("Integration test emergency");
            })
        },
    ];

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state consistency
    assert!(capsule.is_active());
    assert!(capsule.is_emergency_stopped());

    let final_state = capsule.get_hedge_state();
    assert!(final_state.operation_count > 0);
    assert!(final_state.emergency_stopped);
}
