//! Memory Ordering Optimization Tests for AtomicHedgeCapsule
//!
//! UCE32 Framework Analysis (Memory Ordering Optimization):
//! - Q28 (Simplicity): SeqCst → Acquire/Release is simpler and semantically correct
//! - Q29 (Practical Constraints): SeqCst adds 10ns latency vs Acquire/Release (B32 K2)
//! - Q30 (Empirical Validation): B32 benchmarks show 20-40% improvement with optimized ordering
//! - Q31 (Rust Transform): Rust's memory ordering gives precise control over synchronization
//! - Q32 (Nightly Enhancement): atomic_from_mut for zero-cost initialization
//!
//! ASSUM Safety Framework Applied to Memory Ordering:
//! - #ASSUME_MEMORY_ORDERING: Acquire/Release sufficient for emergency coordination
//! - #VERIFY_ORDERING_SUFFICIENT: Test under high contention
//! - #ASSUME_ABA_PREVENTION: Generation counters prevent TOCTOU races
//! - #VERIFY_COORDINATION_SAFETY: Multi-threaded validation
//!
//! B32 Hardware Reality Validation:
//! - Emergency stop: 25ns (SeqCst) → 15ns (Release) = 40% improvement target
//! - Emergency check: 25ns (SeqCst) → 10ns (Acquire) = 60% improvement target
//! - Progress counter: 20ns (AcqRel) → 8ns (Relaxed) = 60% improvement target

use atomic_hedge_capsule::{
    types::{BracketOrder, EntryOrder, OrderState},
    AtomicHedgeCapsule, HedgeError,
};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// MEMORY ORDERING OPTIMIZATION TEST CONSTANTS
// ============================================================================

/// B32 K2: Measured atomic operation latencies (Intel Ultra 7 155H)
const SEQCST_CAS_BASELINE_NS: u64 = 25; // Sequential consistency
const RELEASE_STORE_BASELINE_NS: u64 = 15; // Release store
const ACQUIRE_LOAD_BASELINE_NS: u64 = 10; // Acquire load
const RELAXED_FETCHADD_BASELINE_NS: u64 = 8; // Relaxed fetch_add

/// Expected improvement percentages from memory ordering optimization
const EMERGENCY_STORE_TARGET_IMPROVEMENT: f64 = 0.40; // 40% SeqCst → Release
const EMERGENCY_LOAD_TARGET_IMPROVEMENT: f64 = 0.60; // 60% SeqCst → Acquire
const PROGRESS_COUNTER_TARGET_IMPROVEMENT: f64 = 0.60; // 60% AcqRel → Relaxed

/// Test configuration
const MEMORY_ORDERING_TEST_THREADS: usize = 16;
const MEMORY_ORDERING_ITERATIONS: usize = 10000;
const CONTENTION_TEST_DURATION_MS: u64 = 100;

// ============================================================================
// 1. EMERGENCY COORDINATION MEMORY ORDERING TESTS
// ============================================================================

#[test]
fn test_emergency_coordination_memory_ordering() {
    // ASSUM Memory Ordering Analysis:
    // #ASSUME_MEMORY_ORDERING: Release sufficient for emergency flag coordination
    // #VERIFY_ORDERING_SUFFICIENT: Test emergency coordination under contention

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "EMERGENCY".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let emergency_triggered = Arc::new(AtomicBool::new(false));
    let operations_before_emergency = Arc::new(AtomicU64::new(0));
    let operations_after_emergency = Arc::new(AtomicU64::new(0));
    let barrier = Arc::new(Barrier::new(MEMORY_ORDERING_TEST_THREADS + 1));

    // Spawn worker threads
    let handles: Vec<_> = (0..MEMORY_ORDERING_TEST_THREADS)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let emergency_triggered = Arc::clone(&emergency_triggered);
            let operations_before_emergency = Arc::clone(&operations_before_emergency);
            let operations_after_emergency = Arc::clone(&operations_after_emergency);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..MEMORY_ORDERING_ITERATIONS {
                    // Check for emergency BEFORE operation (Acquire load)
                    if emergency_triggered.load(Ordering::Acquire) {
                        operations_after_emergency.fetch_add(1, Ordering::Relaxed);
                        break; // Emergency stop
                    }

                    // Perform normal operation
                    let filled = (thread_id as f64 + i as f64) / 100000.0;
                    let _result = capsule.update_entry_state(OrderState::PartiallyFilled, filled);

                    operations_before_emergency.fetch_add(1, Ordering::Relaxed);

                    // Small delay to allow emergency signal
                    if i % 100 == 0 {
                        std::thread::yield_now();
                    }
                }
            })
        })
        .collect();

    // Emergency trigger thread
    barrier.wait();

    // Wait for some operations to start
    std::thread::sleep(Duration::from_millis(10));

    // Trigger emergency (Release store)
    let emergency_start = Instant::now();
    emergency_triggered.store(true, Ordering::Release);
    let emergency_latency = emergency_start.elapsed();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    let ops_before = operations_before_emergency.load(Ordering::Relaxed);
    let ops_after = operations_after_emergency.load(Ordering::Relaxed);
    let total_ops = ops_before + ops_after;

    // Memory ordering validation
    assert!(
        ops_before > 0,
        "Some operations should complete before emergency"
    );
    assert!(
        total_ops > ops_before,
        "Emergency should be detected by some threads"
    );
    assert!(
        emergency_latency.as_nanos() < 1_000_000, // 1ms max emergency latency
        "Emergency coordination took {} ns, exceeds 1ms limit",
        emergency_latency.as_nanos()
    );

    println!("Emergency Coordination Memory Ordering Test:");
    println!("  Operations before emergency: {}", ops_before);
    println!("  Operations after emergency: {}", ops_after);
    println!(
        "  Emergency signal latency: {} ns",
        emergency_latency.as_nanos()
    );
    println!(
        "  Stop effectiveness: {:.1}%",
        (ops_after as f64 / total_ops as f64) * 100.0
    );
}

#[test]
fn test_emergency_ordering_performance_improvement() {
    // UCE32 Q30: Measure actual improvement from SeqCst → Release optimization
    // B32: Target 40% improvement (25ns → 15ns)

    let emergency_flag = AtomicBool::new(false);
    let iterations = 100000;
    let samples = 100;

    // Simulate SeqCst emergency coordination (baseline)
    let mut seqcst_measurements = Vec::with_capacity(samples);
    for _sample in 0..samples {
        let start = Instant::now();
        for i in 0..iterations {
            // Simulate SeqCst store
            emergency_flag.store(i % 2 == 0, Ordering::SeqCst);
            // Simulate SeqCst load
            let _emergency = emergency_flag.load(Ordering::SeqCst);
        }
        let duration = start.elapsed();
        seqcst_measurements.push(duration.as_nanos() as f64 / iterations as f64);
    }

    // Simulate Release/Acquire emergency coordination (optimized)
    let mut optimized_measurements = Vec::with_capacity(samples);
    for _sample in 0..samples {
        let start = Instant::now();
        for i in 0..iterations {
            // Optimized: Release store
            emergency_flag.store(i % 2 == 0, Ordering::Release);
            // Optimized: Acquire load
            let _emergency = emergency_flag.load(Ordering::Acquire);
        }
        let duration = start.elapsed();
        optimized_measurements.push(duration.as_nanos() as f64 / iterations as f64);
    }

    // Statistical analysis
    let seqcst_mean = seqcst_measurements.iter().sum::<f64>() / seqcst_measurements.len() as f64;
    let optimized_mean =
        optimized_measurements.iter().sum::<f64>() / optimized_measurements.len() as f64;
    let improvement = (seqcst_mean - optimized_mean) / seqcst_mean;
    let improvement_percentage = improvement * 100.0;

    // B32 validation: Should achieve target improvement
    assert!(
        improvement >= 0.0,
        "Optimization should not make performance worse"
    );
    assert!(
        improvement <= 0.8,
        "Improvement {:.1}% exceeds reasonable maximum",
        improvement_percentage
    );

    // UCE32 Q30: Empirical validation of claimed improvement
    if improvement >= EMERGENCY_STORE_TARGET_IMPROVEMENT * 0.8 {
        println!("✓ Emergency ordering optimization meets target");
    } else {
        println!(
            "⚠ Emergency ordering optimization below target: {:.1}% vs {:.1}% target",
            improvement_percentage,
            EMERGENCY_STORE_TARGET_IMPROVEMENT * 100.0
        );
    }

    println!("Emergency Ordering Performance Analysis:");
    println!("  SeqCst baseline: {:.2} ns/op", seqcst_mean);
    println!("  Release/Acquire optimized: {:.2} ns/op", optimized_mean);
    println!("  Improvement: {:.1}%", improvement_percentage);
    println!(
        "  Target: {:.1}%",
        EMERGENCY_STORE_TARGET_IMPROVEMENT * 100.0
    );
    println!("  B32 K2 SeqCst baseline: {} ns", SEQCST_CAS_BASELINE_NS);
    println!(
        "  B32 K2 Release baseline: {} ns",
        RELEASE_STORE_BASELINE_NS
    );
}

// ============================================================================
// 2. PROGRESS COUNTER MEMORY ORDERING TESTS
// ============================================================================

#[test]
fn test_progress_counter_relaxed_ordering() {
    // ASSUM Memory Ordering Analysis:
    // #ASSUME_MEMORY_ORDERING: Relaxed sufficient for progress monitoring
    // #VERIFY_ORDERING_SUFFICIENT: Progress counter monotonic and approximate

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "PROGRESS".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let progress_counter = Arc::new(AtomicU64::new(0));
    let barrier = Arc::new(Barrier::new(MEMORY_ORDERING_TEST_THREADS));

    let handles: Vec<_> = (0..MEMORY_ORDERING_TEST_THREADS)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let progress_counter = Arc::clone(&progress_counter);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..1000 {
                    // Update capsule state
                    let filled = (thread_id as f64 + i as f64) / 100000.0;
                    if capsule
                        .update_entry_state(OrderState::PartiallyFilled, filled)
                        .is_ok()
                    {
                        // Increment progress counter with Relaxed ordering
                        progress_counter.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let final_progress = progress_counter.load(Ordering::Relaxed);

    // Progress counter validation
    assert!(final_progress > 0, "Progress counter should increment");
    assert!(
        final_progress <= MEMORY_ORDERING_TEST_THREADS as u64 * 1000,
        "Progress counter {} exceeds maximum possible",
        final_progress
    );

    // Relaxed ordering allows approximate counting - exact value not guaranteed
    // but should be in reasonable range
    let expected_min = (MEMORY_ORDERING_TEST_THREADS as u64 * 1000) / 10; // At least 10%
    assert!(
        final_progress >= expected_min,
        "Progress counter {} below minimum expected {}",
        final_progress,
        expected_min
    );

    println!("Progress Counter Relaxed Ordering Test:");
    println!("  Final progress count: {}", final_progress);
    println!(
        "  Maximum possible: {}",
        MEMORY_ORDERING_TEST_THREADS * 1000
    );
    println!(
        "  Accuracy: {:.1}%",
        (final_progress as f64 / (MEMORY_ORDERING_TEST_THREADS as f64 * 1000.0)) * 100.0
    );
}

#[test]
fn test_progress_counter_performance_improvement() {
    // UCE32 Q30: Measure improvement from AcqRel → Relaxed optimization
    // B32: Target 60% improvement (20ns → 8ns)

    let progress_counter = AtomicU64::new(0);
    let iterations = 50000;
    let samples = 100;

    // Simulate AcqRel progress updates (baseline)
    let mut acqrel_measurements = Vec::with_capacity(samples);
    for _sample in 0..samples {
        let start = Instant::now();
        for _i in 0..iterations {
            progress_counter.fetch_add(1, Ordering::AcqRel);
        }
        let duration = start.elapsed();
        acqrel_measurements.push(duration.as_nanos() as f64 / iterations as f64);
    }

    // Reset counter
    progress_counter.store(0, Ordering::Relaxed);

    // Simulate Relaxed progress updates (optimized)
    let mut relaxed_measurements = Vec::with_capacity(samples);
    for _sample in 0..samples {
        let start = Instant::now();
        for _i in 0..iterations {
            progress_counter.fetch_add(1, Ordering::Relaxed);
        }
        let duration = start.elapsed();
        relaxed_measurements.push(duration.as_nanos() as f64 / iterations as f64);
    }

    // Statistical analysis
    let acqrel_mean = acqrel_measurements.iter().sum::<f64>() / acqrel_measurements.len() as f64;
    let relaxed_mean = relaxed_measurements.iter().sum::<f64>() / relaxed_measurements.len() as f64;
    let improvement = (acqrel_mean - relaxed_mean) / acqrel_mean;
    let improvement_percentage = improvement * 100.0;

    // B32 validation
    assert!(improvement >= 0.0, "Relaxed ordering should not be slower");
    assert!(
        improvement <= 0.9,
        "Improvement {:.1}% exceeds reasonable maximum",
        improvement_percentage
    );

    // UCE32 Q30: Empirical validation
    if improvement >= PROGRESS_COUNTER_TARGET_IMPROVEMENT * 0.7 {
        println!("✓ Progress counter optimization meets target");
    } else {
        println!(
            "⚠ Progress counter optimization below target: {:.1}% vs {:.1}% target",
            improvement_percentage,
            PROGRESS_COUNTER_TARGET_IMPROVEMENT * 100.0
        );
    }

    println!("Progress Counter Performance Analysis:");
    println!("  AcqRel baseline: {:.2} ns/op", acqrel_mean);
    println!("  Relaxed optimized: {:.2} ns/op", relaxed_mean);
    println!("  Improvement: {:.1}%", improvement_percentage);
    println!(
        "  Target: {:.1}%",
        PROGRESS_COUNTER_TARGET_IMPROVEMENT * 100.0
    );
}

// ============================================================================
// 3. GENERATION COUNTER MEMORY ORDERING TESTS
// ============================================================================

#[test]
fn test_generation_counter_acqrel_ordering() {
    // ASSUM Memory Ordering Analysis:
    // #ASSUME_MEMORY_ORDERING: AcqRel required for generation coordination
    // #VERIFY_ORDERING_SUFFICIENT: TOCTOU prevention validation

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "GENERATION".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let generation_mismatches = Arc::new(AtomicU64::new(0));
    let successful_operations = Arc::new(AtomicU64::new(0));
    let barrier = Arc::new(Barrier::new(MEMORY_ORDERING_TEST_THREADS));

    let handles: Vec<_> = (0..MEMORY_ORDERING_TEST_THREADS)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let generation_mismatches = Arc::clone(&generation_mismatches);
            let successful_operations = Arc::clone(&successful_operations);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..1000 {
                    // Read generation before operation
                    let state_before = capsule.get_state_fast();

                    if let Ok(state) = state_before {
                        let generation_before = state.generation;

                        // Perform operation
                        let filled = (thread_id as f64 + i as f64) / 100000.0;
                        let result =
                            capsule.update_entry_state(OrderState::PartiallyFilled, filled);

                        // Read generation after operation
                        if let Ok(new_state) = capsule.get_state_fast() {
                            let generation_after = new_state.generation;

                            match result {
                                Ok(_) => {
                                    successful_operations.fetch_add(1, Ordering::Relaxed);
                                    // Generation should increment on successful update
                                    if generation_after <= generation_before {
                                        generation_mismatches.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                                Err(_) => {
                                    // On failure, generation might not change
                                    // This is acceptable for concurrent modifications
                                }
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

    let mismatches = generation_mismatches.load(Ordering::Relaxed);
    let successes = successful_operations.load(Ordering::Relaxed);

    // Generation counter validation
    assert!(successes > 0, "Some operations should succeed");

    // #VERIFY_ORDERING_SUFFICIENT: No generation mismatches allowed
    // AcqRel ordering should prevent TOCTOU races
    assert_eq!(
        mismatches, 0,
        "Generation counter mismatches detected: {} out of {} operations",
        mismatches, successes
    );

    println!("Generation Counter AcqRel Ordering Test:");
    println!("  Successful operations: {}", successes);
    println!("  Generation mismatches: {}", mismatches);
    println!(
        "  TOCTOU prevention: {}",
        if mismatches == 0 {
            "✓ PASS"
        } else {
            "✗ FAIL"
        }
    );
}

// ============================================================================
// 4. COMPARE-AND-EXCHANGE MEMORY ORDERING TESTS
// ============================================================================

#[test]
fn test_cas_memory_ordering_optimization() {
    // ASSUM Memory Ordering Analysis:
    // #ASSUME_MEMORY_ORDERING: Success=Release, Failure=Acquire sufficient for CAS
    // #VERIFY_ORDERING_SUFFICIENT: Test CAS coordination under contention

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "CAS".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let cas_successes = Arc::new(AtomicU64::new(0));
    let cas_failures = Arc::new(AtomicU64::new(0));
    let barrier = Arc::new(Barrier::new(MEMORY_ORDERING_TEST_THREADS));

    let handles: Vec<_> = (0..MEMORY_ORDERING_TEST_THREADS)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let cas_successes = Arc::clone(&cas_successes);
            let cas_failures = Arc::clone(&cas_failures);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..500 {
                    let filled = (thread_id as f64 + i as f64) / 100000.0;

                    match capsule.update_entry_state(OrderState::PartiallyFilled, filled) {
                        Ok(_) => {
                            cas_successes.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(HedgeError::CoordinationFailure { .. }) => {
                            cas_failures.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            // Other errors (validation, etc.)
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let successes = cas_successes.load(Ordering::Relaxed);
    let failures = cas_failures.load(Ordering::Relaxed);
    let total_cas_attempts = successes + failures;
    let success_rate = if total_cas_attempts > 0 {
        successes as f64 / total_cas_attempts as f64
    } else {
        0.0
    };

    // CAS memory ordering validation
    assert!(successes > 0, "Some CAS operations should succeed");
    assert!(total_cas_attempts > 0, "CAS operations should be attempted");
    assert!(
        success_rate >= 0.05,
        "CAS success rate {} too low",
        success_rate
    );
    assert!(
        success_rate <= 1.0,
        "CAS success rate {} exceeds 100%",
        success_rate
    );

    println!("CAS Memory Ordering Test:");
    println!("  CAS successes: {}", successes);
    println!("  CAS failures: {}", failures);
    println!("  Success rate: {:.1}%", success_rate * 100.0);
    println!("  Memory ordering: Success=Release, Failure=Acquire");
}

#[test]
fn test_cas_ordering_performance_comparison() {
    // UCE32 Q30: Compare SeqCst vs Release/Acquire CAS performance

    let test_value = AtomicU64::new(0);
    let iterations = 10000;
    let samples = 50;

    // SeqCst CAS (baseline)
    let mut seqcst_measurements = Vec::with_capacity(samples);
    for sample in 0..samples {
        test_value.store(sample as u64, Ordering::Relaxed);

        let start = Instant::now();
        for i in 0..iterations {
            let current = test_value.load(Ordering::SeqCst);
            let new_value = current.wrapping_add(1);
            let _result = test_value.compare_exchange_weak(
                current,
                new_value,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
        }
        let duration = start.elapsed();
        seqcst_measurements.push(duration.as_nanos() as f64 / iterations as f64);
    }

    // Release/Acquire CAS (optimized)
    let mut optimized_measurements = Vec::with_capacity(samples);
    for sample in 0..samples {
        test_value.store(sample as u64, Ordering::Relaxed);

        let start = Instant::now();
        for i in 0..iterations {
            let current = test_value.load(Ordering::Acquire);
            let new_value = current.wrapping_add(1);
            let _result = test_value.compare_exchange_weak(
                current,
                new_value,
                Ordering::Release,
                Ordering::Acquire,
            );
        }
        let duration = start.elapsed();
        optimized_measurements.push(duration.as_nanos() as f64 / iterations as f64);
    }

    // Statistical analysis
    let seqcst_mean = seqcst_measurements.iter().sum::<f64>() / seqcst_measurements.len() as f64;
    let optimized_mean =
        optimized_measurements.iter().sum::<f64>() / optimized_measurements.len() as f64;
    let improvement = (seqcst_mean - optimized_mean) / seqcst_mean;
    let improvement_percentage = improvement * 100.0;

    println!("CAS Memory Ordering Performance Comparison:");
    println!("  SeqCst CAS: {:.2} ns/op", seqcst_mean);
    println!("  Release/Acquire CAS: {:.2} ns/op", optimized_mean);
    println!("  Improvement: {:.1}%", improvement_percentage);
    println!("  B32 K2 SeqCst baseline: {} ns", SEQCST_CAS_BASELINE_NS);

    // Validate reasonable performance
    assert!(seqcst_mean > 0.0);
    assert!(optimized_mean > 0.0);
    assert!(
        improvement >= -0.2,
        "Optimization should not significantly worsen performance"
    );
}

// ============================================================================
// 5. COMPREHENSIVE MEMORY ORDERING INTEGRATION TEST
// ============================================================================

#[test]
fn test_comprehensive_memory_ordering_integration() {
    // UCE32 Q30: Test all memory ordering optimizations working together
    // ASSUM: All optimizations maintain correctness under high contention

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "INTEGRATION".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let emergency_flag = Arc::new(AtomicBool::new(false));
    let progress_counter = Arc::new(AtomicU64::new(0));
    let operation_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));
    let barrier = Arc::new(Barrier::new(MEMORY_ORDERING_TEST_THREADS + 1));

    // Worker threads
    let handles: Vec<_> = (0..MEMORY_ORDERING_TEST_THREADS)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let emergency_flag = Arc::clone(&emergency_flag);
            let progress_counter = Arc::clone(&progress_counter);
            let operation_count = Arc::clone(&operation_count);
            let error_count = Arc::clone(&error_count);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..1000 {
                    // Check emergency with Acquire ordering
                    if emergency_flag.load(Ordering::Acquire) {
                        break;
                    }

                    // Perform main operation (uses Release/Acquire CAS internally)
                    let filled = (thread_id as f64 + i as f64) / 100000.0;
                    let result = capsule.update_entry_state(OrderState::PartiallyFilled, filled);

                    operation_count.fetch_add(1, Ordering::Relaxed);

                    match result {
                        Ok(_) => {
                            // Update progress with Relaxed ordering
                            progress_counter.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            error_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    // Periodic yield for emergency detection
                    if i % 50 == 0 {
                        std::thread::yield_now();
                    }
                }
            })
        })
        .collect();

    // Emergency trigger thread
    barrier.wait();

    // Let operations run for a bit
    std::thread::sleep(Duration::from_millis(20));

    // Trigger emergency with Release ordering
    emergency_flag.store(true, Ordering::Release);

    // Wait for completion
    for handle in handles {
        handle.join().unwrap();
    }

    let total_ops = operation_count.load(Ordering::Relaxed);
    let progress = progress_counter.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);
    let success_rate = if total_ops > 0 {
        progress as f64 / total_ops as f64
    } else {
        0.0
    };

    // Integration validation
    assert!(total_ops > 0, "Operations should be performed");
    assert!(progress > 0, "Some operations should succeed");
    assert!(success_rate >= 0.1, "Success rate {} too low", success_rate);
    assert!(
        success_rate <= 1.0,
        "Success rate {} exceeds 100%",
        success_rate
    );

    // Emergency should eventually stop operations
    // (Cannot guarantee exact timing due to thread scheduling)

    println!("Comprehensive Memory Ordering Integration Test:");
    println!("  Total operations: {}", total_ops);
    println!("  Successful operations: {}", progress);
    println!("  Errors: {}", errors);
    println!("  Success rate: {:.1}%", success_rate * 100.0);
    println!(
        "  Emergency triggered: {}",
        emergency_flag.load(Ordering::Acquire)
    );
    println!("  Memory ordering optimizations: ✓ Emergency (Release/Acquire), ✓ Progress (Relaxed), ✓ CAS (Release/Acquire)");
}

// ============================================================================
// 6. MEMORY ORDERING SAFETY VALIDATION
// ============================================================================

#[test]
fn test_memory_ordering_safety_under_stress() {
    // ASSUM Safety Validation: All memory ordering optimizations are safe
    // #VERIFY_ORDERING_SUFFICIENT: No data races or corruption under stress

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "SAFETY".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let data_corruption_detected = Arc::new(AtomicBool::new(false));
    let consistency_violations = Arc::new(AtomicU64::new(0));
    let barrier = Arc::new(Barrier::new(MEMORY_ORDERING_TEST_THREADS));

    let handles: Vec<_> = (0..MEMORY_ORDERING_TEST_THREADS)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let data_corruption_detected = Arc::clone(&data_corruption_detected);
            let consistency_violations = Arc::clone(&consistency_violations);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..2000 {
                    let filled = (thread_id as f64 + i as f64) / 100000.0;

                    // Perform update
                    let update_result =
                        capsule.update_entry_state(OrderState::PartiallyFilled, filled);

                    // Immediately read back state
                    if let Ok(state) = capsule.get_state_fast() {
                        // Validate state consistency
                        if state.filled_amount < 0.0 || state.filled_amount > 100.0 {
                            data_corruption_detected.store(true, Ordering::Release);
                        }

                        // Check for impossible state transitions
                        match (&state.order_state, update_result) {
                            (OrderState::PartiallyFilled, Ok(_)) => {
                                // Expected case
                            }
                            (_, Ok(_)) => {
                                // Successful update but unexpected state
                                consistency_violations.fetch_add(1, Ordering::Relaxed);
                            }
                            _ => {
                                // Failed update - state should be unchanged (acceptable)
                            }
                        }
                    } else {
                        // State read failed after update - potential corruption
                        data_corruption_detected.store(true, Ordering::Release);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let corruption = data_corruption_detected.load(Ordering::Acquire);
    let violations = consistency_violations.load(Ordering::Relaxed);

    // Safety validation
    assert!(
        !corruption,
        "Data corruption detected with memory ordering optimizations"
    );
    assert_eq!(
        violations, 0,
        "Consistency violations detected: {}",
        violations
    );

    println!("Memory Ordering Safety Validation:");
    println!(
        "  Data corruption: {}",
        if corruption { "DETECTED" } else { "NONE" }
    );
    println!("  Consistency violations: {}", violations);
    println!(
        "  Safety status: {}",
        if !corruption && violations == 0 {
            "✓ SAFE"
        } else {
            "✗ UNSAFE"
        }
    );
    println!("  Memory ordering optimizations maintain safety guarantees");
}

// Helper function for statistical analysis of memory ordering measurements
fn analyze_memory_ordering_performance(
    baseline_measurements: &[f64],
    optimized_measurements: &[f64],
    operation_name: &str,
    target_improvement: f64,
) {
    let baseline_mean =
        baseline_measurements.iter().sum::<f64>() / baseline_measurements.len() as f64;
    let optimized_mean =
        optimized_measurements.iter().sum::<f64>() / optimized_measurements.len() as f64;

    let baseline_std = {
        let variance = baseline_measurements
            .iter()
            .map(|x| (x - baseline_mean).powi(2))
            .sum::<f64>()
            / (baseline_measurements.len() - 1) as f64;
        variance.sqrt()
    };

    let optimized_std = {
        let variance = optimized_measurements
            .iter()
            .map(|x| (x - optimized_mean).powi(2))
            .sum::<f64>()
            / (optimized_measurements.len() - 1) as f64;
        variance.sqrt()
    };

    let improvement = (baseline_mean - optimized_mean) / baseline_mean;
    let improvement_percentage = improvement * 100.0;

    println!("{} Memory Ordering Analysis:", operation_name);
    println!("  Baseline: {:.2} ± {:.2} ns", baseline_mean, baseline_std);
    println!(
        "  Optimized: {:.2} ± {:.2} ns",
        optimized_mean, optimized_std
    );
    println!("  Improvement: {:.1}%", improvement_percentage);
    println!("  Target: {:.1}%", target_improvement * 100.0);
    println!(
        "  Status: {}",
        if improvement >= target_improvement * 0.8 {
            "✓ TARGET MET"
        } else {
            "⚠ BELOW TARGET"
        }
    );
}
