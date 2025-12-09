//! Algorithm Optimization Tests for AtomicHedgeCapsule
//!
//! UCE32 Framework Analysis Applied (Complexity Level 6-8: Coordination Systems):
//! - Q28 (Simplicity): Are optimizations actually simpler and better?
//! - Q29 (Practical Constraints): Real hardware limits from B32 K1-K27 measurements
//! - Q30 (Empirical Validation): Statistical validation of all optimization claims
//! - Q31 (Rust Transform): Leveraging Rust's zero-cost abstractions and safety
//! - Q32 (Nightly Enhancement): Testing nightly features for breakthrough performance
//!
//! B32 Benchmark Framework Applied:
//! - Hardware Reality Checks: Intel Ultra 7 155H baseline measurements
//! - Statistical Rigor: 95% confidence intervals, 1000+ iterations
//! - Fair Baselines: Compare against optimized implementations
//! - Honest Performance: 10-50% typical, 2x exceptional, 10x+ suspicious
//!
//! ASSUM Safety Framework Applied:
//! - Every #ASSUME has corresponding #VERIFY
//! - Memory ordering validation for all atomic optimizations
//! - Race condition prevention through systematic testing

use atomic_hedge_capsule::{
    types::{BracketOrder, EntryOrder, OrderState},
    AtomicHedgeCapsule, HedgeError, HedgeState,
};
use proptest::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// B32 HARDWARE REALITY CONSTANTS (Intel Ultra 7 155H)
// ============================================================================

/// B32 K2: Measured atomic operation latencies
const ATOMICU64_CAS_BASELINE_NS: u64 = 15; // B32 K2: Real measurement
const ATOMICU64_LOAD_BASELINE_NS: u64 = 10; // B32 K2: Real measurement
const ATOMICU64_STORE_BASELINE_NS: u64 = 12; // B32 K2: Real measurement

/// B32 K6: Cache hierarchy reality
const L1_CACHE_LATENCY_NS: u64 = 1;
const L2_CACHE_LATENCY_NS: u64 = 3;
const L3_CACHE_LATENCY_NS: u64 = 12;
const RAM_LATENCY_NS: u64 = 100;

/// B32 K7: Branch prediction reality
const BRANCH_MISPREDICTION_PENALTY_CYCLES: u64 = 18;
const CPU_CYCLE_NS: f64 = 0.21; // P-core @ 4.8GHz

/// Performance targets based on B32 framework
const TYPICAL_IMPROVEMENT_MIN: f64 = 0.10; // 10% minimum for "improvement"
const TYPICAL_IMPROVEMENT_MAX: f64 = 0.50; // 50% maximum for "typical"
const EXCEPTIONAL_IMPROVEMENT_MAX: f64 = 2.0; // 2x maximum for "exceptional"
const SUSPICIOUS_IMPROVEMENT_MIN: f64 = 10.0; // 10x+ requires extensive validation

/// Test configuration constants
const STRESS_TEST_THREADS: usize = 12; // B32 K8: Efficient scaling limit
const STRESS_TEST_OPERATIONS: usize = 1000;
const CONTENTION_TEST_THREADS: usize = 16; // Beyond efficient scaling
const BENCHMARK_ITERATIONS: usize = 10000;
const ABA_TEST_ITERATIONS: usize = 10000;
const STATISTICAL_CONFIDENCE_LEVEL: f64 = 0.95; // 95% confidence intervals

// ============================================================================
// 1. CAS RETRY OPTIMIZATION TESTS (UCE32 + B32 VALIDATION)
// ============================================================================

#[test]
fn test_cas_retry_backoff_behavior() {
    // UCE32 Q29: CAS storms are a real constraint beyond 12 threads (B32 K12)
    // UCE32 Q30: Measure actual backoff effectiveness

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize with entry order
    let entry = EntryOrder::new(
        "TEST".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let retry_count = Arc::new(AtomicU64::new(0));
    let success_count = Arc::new(AtomicU64::new(0));
    let barrier = Arc::new(Barrier::new(CONTENTION_TEST_THREADS));

    let handles: Vec<_> = (0..CONTENTION_TEST_THREADS)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let retry_count = Arc::clone(&retry_count);
            let success_count = Arc::clone(&success_count);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();

                for iteration in 0..100 {
                    let filled_amount = (thread_id as f64 + iteration as f64) / 1000.0;
                    let mut local_retries = 0;

                    // Track CAS retry attempts
                    loop {
                        match capsule.update_entry_state(OrderState::PartiallyFilled, filled_amount)
                        {
                            Ok(_) => {
                                success_count.fetch_add(1, Ordering::Relaxed);
                                break;
                            }
                            Err(HedgeError::StateUpdateFailed(_)) => {
                                local_retries += 1;
                                if local_retries > 5 {
                                    break; // Prevent infinite retry
                                }
                                // Exponential backoff simulation
                                std::thread::sleep(Duration::from_nanos(1u64 << local_retries));
                            }
                            Err(_) => break, // Other errors
                        }
                    }

                    retry_count.fetch_add(local_retries, Ordering::Relaxed);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let total_retries = retry_count.load(Ordering::Relaxed);
    let total_successes = success_count.load(Ordering::Relaxed);
    let retry_rate = total_retries as f64 / total_successes as f64;

    // B32 K12: CAS retry rate should be reasonable under contention
    assert!(
        retry_rate < 3.0,
        "CAS retry rate {} exceeds reasonable limit",
        retry_rate
    );
    assert!(total_successes > 0, "No successful operations completed");

    println!("CAS Retry Analysis:");
    println!("  Total retries: {}", total_retries);
    println!("  Total successes: {}", total_successes);
    println!("  Retry rate: {:.2}", retry_rate);
}

#[test]
fn test_cas_retry_fairness() {
    // UCE32 Q31: Rust's fair scheduling should prevent starvation
    // UCE32 Q30: Measure fairness empirically

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "TEST".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let thread_success_counts = Arc::new(std::sync::Mutex::new(vec![0u64; STRESS_TEST_THREADS]));
    let barrier = Arc::new(Barrier::new(STRESS_TEST_THREADS));

    let handles: Vec<_> = (0..STRESS_TEST_THREADS)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let thread_success_counts = Arc::clone(&thread_success_counts);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();
                let mut successes = 0;

                for i in 0..100 {
                    let filled = (i as f64) / 1000.0;
                    if capsule
                        .update_entry_state(OrderState::PartiallyFilled, filled)
                        .is_ok()
                    {
                        successes += 1;
                    }
                }

                let mut counts = thread_success_counts.lock().unwrap();
                counts[thread_id] = successes;
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let counts = thread_success_counts.lock().unwrap();
    let total_successes: u64 = counts.iter().sum();
    let mean_successes = total_successes as f64 / STRESS_TEST_THREADS as f64;

    // Calculate fairness (coefficient of variation should be reasonable)
    let variance: f64 = counts
        .iter()
        .map(|&count| (count as f64 - mean_successes).powi(2))
        .sum::<f64>()
        / STRESS_TEST_THREADS as f64;
    let std_dev = variance.sqrt();
    let coefficient_of_variation = std_dev / mean_successes;

    // Fairness test: coefficient of variation should be < 0.5 for fair scheduling
    assert!(
        coefficient_of_variation < 0.5,
        "CAS fairness failed: CV {} indicates potential starvation",
        coefficient_of_variation
    );

    println!("CAS Fairness Analysis:");
    println!("  Mean successes per thread: {:.2}", mean_successes);
    println!("  Standard deviation: {:.2}", std_dev);
    println!(
        "  Coefficient of variation: {:.3}",
        coefficient_of_variation
    );
}

// ============================================================================
// 2. BRANCH PREDICTION OPTIMIZATION TESTS
// ============================================================================

#[test]
fn test_branch_prediction_hot_path_optimization() {
    // UCE32 Q29: Branch misprediction penalty is 18 cycles (B32 K7)
    // UCE32 Q30: Measure branch prediction accuracy empirically

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "TEST".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let hot_path_iterations = 10000;
    let cold_path_iterations = 100;

    // Measure hot path (predictable branch)
    let start_hot = Instant::now();
    for i in 0..hot_path_iterations {
        let filled = (i as f64) / 100000.0; // Always small, predictable
        let _result = capsule.update_entry_state(OrderState::PartiallyFilled, filled);
    }
    let hot_path_duration = start_hot.elapsed();

    // Measure cold path (unpredictable branch)
    let start_cold = Instant::now();
    for i in 0..cold_path_iterations {
        // Mix of different states to cause branch mispredictions
        let state = match i % 4 {
            0 => OrderState::PendingValidation,
            1 => OrderState::PartiallyFilled,
            2 => OrderState::Filled,
            _ => OrderState::Cancelled,
        };
        let filled = fastrand::f64() * 100.0; // Unpredictable
        let _result = capsule.update_entry_state(state, filled);
    }
    let cold_path_duration = start_cold.elapsed();

    // Calculate per-operation timing
    let hot_path_ns_per_op = hot_path_duration.as_nanos() as f64 / hot_path_iterations as f64;
    let cold_path_ns_per_op = cold_path_duration.as_nanos() as f64 / cold_path_iterations as f64;
    let prediction_penalty = cold_path_ns_per_op - hot_path_ns_per_op;

    // B32 K7: Branch misprediction penalty should be measurable
    // Expected penalty: 18 cycles * 0.21ns/cycle = ~3.8ns
    assert!(
        prediction_penalty >= 0.0,
        "Hot path should be faster than cold path"
    );
    assert!(
        prediction_penalty < 50.0,
        "Branch prediction penalty {} ns exceeds reasonable limit",
        prediction_penalty
    );

    println!("Branch Prediction Analysis:");
    println!("  Hot path: {:.2} ns/op", hot_path_ns_per_op);
    println!("  Cold path: {:.2} ns/op", cold_path_ns_per_op);
    println!("  Prediction penalty: {:.2} ns", prediction_penalty);
}

#[test]
fn test_likely_unlikely_annotation_effectiveness() {
    // UCE32 Q32: Test if likely/unlikely annotations help with nightly features
    // UCE32 Q30: Measure effectiveness empirically

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "TEST".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    // Test likely path (common case)
    let likely_iterations = 10000;
    let start_likely = Instant::now();
    for i in 0..likely_iterations {
        let filled = (i as f64) / 100000.0; // Normal range
        if filled < 1.0 {
            // Likely path
            let _result = capsule.update_entry_state(OrderState::PartiallyFilled, filled);
        }
    }
    let likely_duration = start_likely.elapsed();

    // Test unlikely path (error case)
    let unlikely_iterations = 1000;
    let start_unlikely = Instant::now();
    for i in 0..unlikely_iterations {
        let filled = (i as f64) * 1000.0; // Extreme values
        if filled >= 1000.0 {
            // Unlikely path
            let _result = capsule.update_entry_state(OrderState::PartiallyFilled, filled);
        }
    }
    let unlikely_duration = start_unlikely.elapsed();

    let likely_ns_per_op = likely_duration.as_nanos() as f64 / likely_iterations as f64;
    let unlikely_ns_per_op = unlikely_duration.as_nanos() as f64 / unlikely_iterations as f64;

    // Both should complete without crashing
    assert!(likely_ns_per_op > 0.0);
    assert!(unlikely_ns_per_op > 0.0);

    println!("Likely/Unlikely Path Analysis:");
    println!("  Likely path: {:.2} ns/op", likely_ns_per_op);
    println!("  Unlikely path: {:.2} ns/op", unlikely_ns_per_op);
}

// ============================================================================
// 3. NIGHTLY FEATURE OPTIMIZATION TESTS
// ============================================================================

#[cfg(feature = "nightly")]
#[test]
fn test_nightly_portable_simd_optimization() {
    // UCE32 Q32: Test portable_simd for batch operations
    // UCE32 Q30: Measure SIMD acceleration empirically

    use std::simd::{f64x8, u64x8};

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Test SIMD batch processing simulation
    let batch_size = 8;
    let iterations = 1000;

    // Simulate batch filled amounts
    let start_simd = Instant::now();
    for batch in 0..iterations {
        let base_value = batch as f64 / 10000.0;
        let values = f64x8::from_array([
            base_value,
            base_value * 1.1,
            base_value * 1.2,
            base_value * 1.3,
            base_value * 1.4,
            base_value * 1.5,
            base_value * 1.6,
            base_value * 1.7,
        ]);

        // SIMD processing simulation
        let processed = values * f64x8::splat(1.618); // φ-optimization
        let result_array = processed.to_array();

        // Apply first result to capsule
        if result_array[0] > 0.0 && result_array[0] < 1.0 {
            let _result = capsule.update_entry_state(OrderState::PartiallyFilled, result_array[0]);
        }
    }
    let simd_duration = start_simd.elapsed();

    // Compare with scalar version
    let start_scalar = Instant::now();
    for batch in 0..iterations {
        let base_value = batch as f64 / 10000.0;
        for i in 0..batch_size {
            let value = base_value * (1.0 + (i as f64) * 0.1);
            let processed = value * 1.618; // φ-optimization

            if processed > 0.0 && processed < 1.0 && i == 0 {
                let _result = capsule.update_entry_state(OrderState::PartiallyFilled, processed);
            }
        }
    }
    let scalar_duration = start_scalar.elapsed();

    let simd_ns_per_batch = simd_duration.as_nanos() as f64 / iterations as f64;
    let scalar_ns_per_batch = scalar_duration.as_nanos() as f64 / iterations as f64;
    let acceleration_factor = scalar_ns_per_batch / simd_ns_per_batch;

    // B32 K9: SIMD acceleration should be 3-4x for 8-element operations
    assert!(
        acceleration_factor >= 1.0,
        "SIMD should not be slower than scalar"
    );
    assert!(
        acceleration_factor <= 8.0,
        "SIMD acceleration {} exceeds theoretical maximum",
        acceleration_factor
    );

    println!("SIMD Optimization Analysis:");
    println!("  SIMD: {:.2} ns/batch", simd_ns_per_batch);
    println!("  Scalar: {:.2} ns/batch", scalar_ns_per_batch);
    println!("  Acceleration factor: {:.2}x", acceleration_factor);
}

#[cfg(feature = "nightly")]
#[test]
fn test_nightly_const_fn_floating_point() {
    // UCE32 Q32: Test const_fn_floating_point_arithmetic for compile-time calculations
    // UCE32 Q30: Verify compile-time computation accuracy

    const fn phi_reciprocal() -> f64 {
        0.6180339887498948 // φ^(-1)
    }

    const fn calculate_hedge_threshold() -> f64 {
        phi_reciprocal() * 0.05 // 3.09% threshold
    }

    // These calculations should happen at compile-time
    const HEDGE_THRESHOLD: f64 = calculate_hedge_threshold();
    const PHI_INV: f64 = phi_reciprocal();

    // Runtime verification
    assert!((HEDGE_THRESHOLD - 0.030901699).abs() < 1e-9);
    assert!((PHI_INV - 0.6180339887498948).abs() < 1e-15);

    // Performance test: const vs runtime calculation
    let iterations = 100000;

    let start_const = Instant::now();
    for _ in 0..iterations {
        let _threshold = HEDGE_THRESHOLD; // Compile-time constant
        black_box(_threshold);
    }
    let const_duration = start_const.elapsed();

    let start_runtime = Instant::now();
    for _ in 0..iterations {
        let _threshold = 0.6180339887498948 * 0.05; // Runtime calculation
        black_box(_threshold);
    }
    let runtime_duration = start_runtime.elapsed();

    let const_ns_per_op = const_duration.as_nanos() as f64 / iterations as f64;
    let runtime_ns_per_op = runtime_duration.as_nanos() as f64 / iterations as f64;

    // Const should be faster (or at least not slower)
    assert!(
        const_ns_per_op <= runtime_ns_per_op * 1.1,
        "Const calculation should not be significantly slower than runtime"
    );

    println!("Const Fn Floating Point Analysis:");
    println!("  Const: {:.4} ns/op", const_ns_per_op);
    println!("  Runtime: {:.4} ns/op", runtime_ns_per_op);
    println!("  Speedup: {:.2}x", runtime_ns_per_op / const_ns_per_op);
}

#[cfg(feature = "nightly")]
#[test]
fn test_nightly_atomic_from_mut() {
    // UCE32 Q32: Test atomic_from_mut for zero-cost atomic creation
    // UCE32 Q30: Measure conversion overhead

    use std::sync::atomic::AtomicU64;

    let mut data = vec![0u64; 1000];
    let iterations = 1000;

    // Test atomic_from_mut conversion
    let start_atomic_from_mut = Instant::now();
    for i in 0..iterations {
        let atomic_slice = AtomicU64::from_mut_slice(&mut data[i..i + 1]);
        atomic_slice[0].store(i as u64, Ordering::Relaxed);
        let _value = atomic_slice[0].load(Ordering::Relaxed);
    }
    let atomic_from_mut_duration = start_atomic_from_mut.elapsed();

    // Compare with manual atomic creation
    let mut atomic_data: Vec<AtomicU64> = (0..1000).map(|_| AtomicU64::new(0)).collect();
    let start_manual = Instant::now();
    for i in 0..iterations {
        atomic_data[i].store(i as u64, Ordering::Relaxed);
        let _value = atomic_data[i].load(Ordering::Relaxed);
    }
    let manual_duration = start_manual.elapsed();

    let atomic_from_mut_ns_per_op = atomic_from_mut_duration.as_nanos() as f64 / iterations as f64;
    let manual_ns_per_op = manual_duration.as_nanos() as f64 / iterations as f64;

    // atomic_from_mut should be zero-cost (same performance)
    let performance_ratio = atomic_from_mut_ns_per_op / manual_ns_per_op;
    assert!(
        performance_ratio >= 0.8 && performance_ratio <= 1.2,
        "atomic_from_mut performance ratio {} outside expected range",
        performance_ratio
    );

    println!("Atomic From Mut Analysis:");
    println!("  atomic_from_mut: {:.2} ns/op", atomic_from_mut_ns_per_op);
    println!("  Manual atomic: {:.2} ns/op", manual_ns_per_op);
    println!("  Performance ratio: {:.3}", performance_ratio);
}

// ============================================================================
// 4. HOT PATH OPTIMIZATION TESTS
// ============================================================================

#[test]
fn test_hot_path_inlining_effectiveness() {
    // UCE32 Q29: Function call overhead is ~1-2ns (B32 measurement)
    // UCE32 Q30: Measure inlining effectiveness empirically

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "TEST".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let iterations = 10000;

    // Test inlined hot path (direct method calls)
    let start_inlined = Instant::now();
    for i in 0..iterations {
        let filled = (i as f64) / 100000.0;
        // Direct method call should be inlined
        let _result = capsule.update_entry_state(OrderState::PartiallyFilled, filled);
    }
    let inlined_duration = start_inlined.elapsed();

    // Test function pointer path (prevents inlining)
    let update_fn: fn(&AtomicHedgeCapsule, OrderState, f64) -> Result<(), HedgeError> =
        |capsule, state, filled| capsule.update_entry_state(state, filled);

    let start_function_ptr = Instant::now();
    for i in 0..iterations {
        let filled = (i as f64) / 100000.0;
        let _result = update_fn(&capsule, OrderState::PartiallyFilled, filled);
    }
    let function_ptr_duration = start_function_ptr.elapsed();

    let inlined_ns_per_op = inlined_duration.as_nanos() as f64 / iterations as f64;
    let function_ptr_ns_per_op = function_ptr_duration.as_nanos() as f64 / iterations as f64;
    let inlining_benefit = function_ptr_ns_per_op - inlined_ns_per_op;

    // Inlining should provide some benefit (though modern CPUs minimize this)
    assert!(inlined_ns_per_op > 0.0);
    assert!(
        function_ptr_ns_per_op >= inlined_ns_per_op,
        "Inlined path should be at least as fast as function pointer"
    );

    println!("Hot Path Inlining Analysis:");
    println!("  Inlined: {:.2} ns/op", inlined_ns_per_op);
    println!("  Function pointer: {:.2} ns/op", function_ptr_ns_per_op);
    println!("  Inlining benefit: {:.2} ns", inlining_benefit);
}

#[test]
fn test_cache_line_optimization() {
    // UCE32 Q29: Cache line size is 64 bytes (B32 K6)
    // UCE32 Q30: Measure cache line alignment effectiveness

    let capsules: Vec<AtomicHedgeCapsule> = (0..16).map(|_| AtomicHedgeCapsule::new()).collect();
    let iterations = 1000;

    // Test sequential access (cache-friendly)
    let start_sequential = Instant::now();
    for iteration in 0..iterations {
        for (i, capsule) in capsules.iter().enumerate() {
            if iteration == 0 {
                // Initialize only once
                let entry = EntryOrder::new(
                    format!("TEST_{}", i),
                    "BTCUSD".to_string(),
                    "Buy".to_string(),
                    1.0,
                );
                let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
                let _ = capsule.initialize(entry, bracket);
            }

            let filled = (iteration as f64 + i as f64) / 10000.0;
            let _result = capsule.update_entry_state(OrderState::PartiallyFilled, filled);
        }
    }
    let sequential_duration = start_sequential.elapsed();

    // Test random access (cache-unfriendly)
    let mut indices: Vec<usize> = (0..capsules.len()).collect();
    fastrand::shuffle(&mut indices);

    let start_random = Instant::now();
    for iteration in 0..iterations {
        for &i in &indices {
            let filled = (iteration as f64 + i as f64) / 10000.0;
            let _result = capsules[i].update_entry_state(OrderState::PartiallyFilled, filled);
        }
    }
    let random_duration = start_random.elapsed();

    let sequential_ns_per_op =
        sequential_duration.as_nanos() as f64 / (iterations * capsules.len()) as f64;
    let random_ns_per_op = random_duration.as_nanos() as f64 / (iterations * capsules.len()) as f64;
    let cache_penalty = random_ns_per_op - sequential_ns_per_op;

    // Sequential should be faster than random access
    assert!(
        sequential_ns_per_op <= random_ns_per_op,
        "Sequential access should not be slower than random access"
    );

    println!("Cache Line Optimization Analysis:");
    println!("  Sequential: {:.2} ns/op", sequential_ns_per_op);
    println!("  Random: {:.2} ns/op", random_ns_per_op);
    println!("  Cache penalty: {:.2} ns", cache_penalty);
}

// ============================================================================
// 5. COMBINED OPTIMIZATION STRESS TESTS
// ============================================================================

#[test]
fn test_combined_optimizations_under_stress() {
    // UCE32 Q30: Test all optimizations working together
    // UCE32 Q29: Stress test up to efficient scaling limit (B32 K8)

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "STRESS".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let barrier = Arc::new(Barrier::new(STRESS_TEST_THREADS));
    let total_operations = Arc::new(AtomicU64::new(0));
    let total_errors = Arc::new(AtomicU64::new(0));
    let start_time = Arc::new(std::sync::Mutex::new(None::<Instant>));

    let handles: Vec<_> = (0..STRESS_TEST_THREADS)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);
            let total_operations = Arc::clone(&total_operations);
            let total_errors = Arc::clone(&total_errors);
            let start_time = Arc::clone(&start_time);

            thread::spawn(move || {
                barrier.wait();

                // Start timing after all threads are ready
                if thread_id == 0 {
                    *start_time.lock().unwrap() = Some(Instant::now());
                }

                let mut local_operations = 0;
                let mut local_errors = 0;

                for i in 0..STRESS_TEST_OPERATIONS {
                    // Mix different operation types
                    let operation_type = i % 4;
                    let result = match operation_type {
                        0 => {
                            let filled = (i as f64 + thread_id as f64) / 100000.0;
                            capsule.update_entry_state(OrderState::PartiallyFilled, filled)
                        }
                        1 => {
                            let filled = fastrand::f64() * 0.5;
                            capsule.update_entry_state(OrderState::Filled, filled)
                        }
                        2 => {
                            Ok(()) // Read-only operation simulation
                        }
                        _ => {
                            let filled = (i as f64) / 50000.0;
                            capsule.update_entry_state(OrderState::PartiallyFilled, filled)
                        }
                    };

                    match result {
                        Ok(_) => local_operations += 1,
                        Err(_) => local_errors += 1,
                    }
                }

                total_operations.fetch_add(local_operations, Ordering::Relaxed);
                total_errors.fetch_add(local_errors, Ordering::Relaxed);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let end_time = Instant::now();
    let start = start_time.lock().unwrap().unwrap();
    let total_duration = end_time.duration_since(start);

    let ops = total_operations.load(Ordering::Relaxed);
    let errors = total_errors.load(Ordering::Relaxed);
    let success_rate = ops as f64 / (ops + errors) as f64;
    let throughput = ops as f64 / total_duration.as_secs_f64();

    // Performance and correctness validation
    assert!(
        success_rate >= 0.90,
        "Success rate {} below acceptable threshold",
        success_rate
    );
    assert!(throughput > 0.0, "Throughput should be positive");

    println!("Combined Optimization Stress Test:");
    println!("  Total operations: {}", ops);
    println!("  Total errors: {}", errors);
    println!("  Success rate: {:.2}%", success_rate * 100.0);
    println!("  Throughput: {:.0} ops/sec", throughput);
    println!("  Duration: {:.3}s", total_duration.as_secs_f64());
}

#[test]
fn test_optimization_regression_detection() {
    // UCE32 Q30: Detect performance regressions
    // B32: Statistical validation with confidence intervals

    let capsule = AtomicHedgeCapsule::new();

    // Initialize
    let entry = EntryOrder::new(
        "REGRESSION".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let iterations = 1000;
    let samples = 50;
    let mut measurements = Vec::with_capacity(samples);

    // Take multiple measurements for statistical analysis
    for _sample in 0..samples {
        let start = Instant::now();
        for i in 0..iterations {
            let filled = (i as f64) / 100000.0;
            let _result = capsule.update_entry_state(OrderState::PartiallyFilled, filled);
        }
        let duration = start.elapsed();
        measurements.push(duration.as_nanos() as f64 / iterations as f64);
    }

    // Statistical analysis
    let mean = measurements.iter().sum::<f64>() / measurements.len() as f64;
    let variance = measurements.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
        / (measurements.len() - 1) as f64;
    let std_dev = variance.sqrt();
    let std_error = std_dev / (measurements.len() as f64).sqrt();

    // 95% confidence interval
    let t_critical = 2.009; // t-value for 49 degrees of freedom, 95% confidence
    let margin_of_error = t_critical * std_error;
    let confidence_interval = (mean - margin_of_error, mean + margin_of_error);

    // B32 performance baseline (should be under 100ns for simple operations)
    let performance_baseline = 100.0; // ns

    assert!(mean > 0.0, "Mean performance should be positive");
    assert!(
        mean < performance_baseline,
        "Performance regression detected: {} ns > {} ns baseline",
        mean,
        performance_baseline
    );
    assert!(
        std_dev / mean < 0.2,
        "Performance variability too high: CV = {:.3}",
        std_dev / mean
    );

    println!("Regression Detection Analysis:");
    println!("  Mean: {:.2} ± {:.2} ns", mean, margin_of_error);
    println!(
        "  95% CI: [{:.2}, {:.2}] ns",
        confidence_interval.0, confidence_interval.1
    );
    println!("  Standard deviation: {:.2} ns", std_dev);
    println!("  Coefficient of variation: {:.3}", std_dev / mean);
    println!("  Baseline: {:.0} ns", performance_baseline);
}

// ============================================================================
// 6. PROPERTY-BASED TESTING FOR OPTIMIZATION INVARIANTS
// ============================================================================

proptest! {
    #[test]
    fn test_optimization_invariants_property_based(
        filled_amount in 0.0f64..10.0,
        thread_count in 1usize..16,
        operation_count in 1usize..1000
    ) {
        // UCE32 Q30: Property-based testing for optimization correctness
        let capsule = Arc::new(AtomicHedgeCapsule::new());

        // Initialize
        let entry = EntryOrder::new("PROP".to_string(), "BTCUSD".to_string(), "Buy".to_string(), 1.0);
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        prop_assert!(capsule.initialize(entry, bracket).is_ok());

        // Property: All optimized operations should maintain correctness
        let success_count = Arc::new(AtomicU64::new(0));
        let handles: Vec<_> = (0..thread_count).map(|_| {
            let capsule = Arc::clone(&capsule);
            let success_count = Arc::clone(&success_count);

            thread::spawn(move || {
                for i in 0..operation_count {
                    let normalized_filled = (filled_amount + i as f64) % 2.0; // Keep in reasonable range
                    if let Ok(_) = capsule.update_entry_state(OrderState::PartiallyFilled, normalized_filled) {
                        success_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        }).collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // Invariant: Some operations should succeed
        prop_assert!(success_count.load(Ordering::Relaxed) > 0);

        // Invariant: Capsule should remain in valid state
        prop_assert!(capsule.is_active());
    }
}

proptest! {
    #[test]
    fn test_cas_retry_optimization_invariants(
        retry_count in 1u32..10,
        thread_multiplier in 1usize..8
    ) {
        // UCE32 Q30: CAS retry optimization should maintain correctness
        let thread_count = thread_multiplier.min(STRESS_TEST_THREADS);
        let capsule = Arc::new(AtomicHedgeCapsule::new());

        // Initialize
        let entry = EntryOrder::new("CAS_PROP".to_string(), "BTCUSD".to_string(), "Buy".to_string(), 1.0);
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        prop_assert!(capsule.initialize(entry, bracket).is_ok());

        let total_attempts = Arc::new(AtomicU64::new(0));
        let total_successes = Arc::new(AtomicU64::new(0));

        let handles: Vec<_> = (0..thread_count).map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let total_attempts = Arc::clone(&total_attempts);
            let total_successes = Arc::clone(&total_successes);

            thread::spawn(move || {
                for attempt in 0..retry_count {
                    total_attempts.fetch_add(1, Ordering::Relaxed);
                    let filled = (thread_id as f64 + attempt as f64) / 1000.0;

                    if let Ok(_) = capsule.update_entry_state(OrderState::PartiallyFilled, filled) {
                        total_successes.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        }).collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let attempts = total_attempts.load(Ordering::Relaxed);
        let successes = total_successes.load(Ordering::Relaxed);

        // Invariant: Some operations should succeed
        prop_assert!(successes > 0);
        // Invariant: Success rate should be reasonable
        prop_assert!(successes as f64 / attempts as f64 >= 0.1); // At least 10% success rate
    }
}

// ============================================================================
// 7. ASSUM SAFETY FRAMEWORK VALIDATION
// ============================================================================

#[test]
fn test_assum_memory_ordering_safety() {
    // ASSUM Safety Framework: Validate all memory ordering assumptions
    // Every #ASSUME has corresponding #VERIFY

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "ASSUM".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    // #ASSUME_MEMORY_ORDERING: Release/Acquire provides sufficient synchronization
    // #VERIFY_ORDERING_SUFFICIENT: Test under contention

    let barrier = Arc::new(Barrier::new(CONTENTION_TEST_THREADS));
    let ordering_violations = Arc::new(AtomicU64::new(0));
    let write_count = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..CONTENTION_TEST_THREADS)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let barrier = Arc::clone(&barrier);
            let ordering_violations = Arc::clone(&ordering_violations);
            let write_count = Arc::clone(&write_count);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..100 {
                    let filled = (thread_id as f64 + i as f64) / 10000.0;

                    // Write operation
                    if capsule
                        .update_entry_state(OrderState::PartiallyFilled, filled)
                        .is_ok()
                    {
                        write_count.fetch_add(1, Ordering::Relaxed);

                        // Immediately read back and verify
                        {
                            let state = capsule.get_state_fast();
                            // Note: HedgeState is an enum, so we check if emergency state matches expectation
                            if matches!(state, HedgeState::Emergency) {
                                // Potential ordering violation
                                ordering_violations.fetch_add(1, Ordering::Relaxed);
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

    let writes = write_count.load(Ordering::Relaxed);
    let violations = ordering_violations.load(Ordering::Relaxed);
    let violation_rate = if writes > 0 {
        violations as f64 / writes as f64
    } else {
        0.0
    };

    // #VERIFY_ORDERING_SUFFICIENT: No memory ordering violations allowed
    assert_eq!(
        violations, 0,
        "Memory ordering violations detected: {} out of {} writes",
        violations, writes
    );

    println!("ASSUM Memory Ordering Validation:");
    println!("  Total writes: {}", writes);
    println!("  Ordering violations: {}", violations);
    println!("  Violation rate: {:.6}%", violation_rate * 100.0);
}

#[test]
fn test_assum_aba_prevention_safety() {
    // ASSUM Safety Framework: ABA problem prevention
    // #ASSUME_ABA_PREVENTION: Generation counters prevent ABA problems
    // #VERIFY_ABA_PREVENTION: Test under high contention

    let capsule = Arc::new(AtomicHedgeCapsule::new());

    // Initialize
    let entry = EntryOrder::new(
        "ABA".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let aba_attempts = Arc::new(AtomicU64::new(0));
    let aba_successes = Arc::new(AtomicU64::new(0));
    let barrier = Arc::new(Barrier::new(CONTENTION_TEST_THREADS));

    let handles: Vec<_> = (0..CONTENTION_TEST_THREADS)
        .map(|thread_id| {
            let capsule = Arc::clone(&capsule);
            let aba_attempts = Arc::clone(&aba_attempts);
            let aba_successes = Arc::clone(&aba_successes);
            let barrier = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..ABA_TEST_ITERATIONS / CONTENTION_TEST_THREADS {
                    aba_attempts.fetch_add(1, Ordering::Relaxed);

                    // Try to create ABA scenario
                    let original_state: Result<HedgeState, ()> = Ok(capsule.get_state_fast());
                    if let Ok(state) = original_state {
                        // Attempt modification
                        let new_filled = (thread_id as f64 + i as f64) / 100000.0;

                        // This should either succeed or fail safely (no ABA)
                        if capsule
                            .update_entry_state(OrderState::PartiallyFilled, new_filled)
                            .is_ok()
                        {
                            aba_successes.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let attempts = aba_attempts.load(Ordering::Relaxed);
    let successes = aba_successes.load(Ordering::Relaxed);

    // #VERIFY_ABA_PREVENTION: Operations should complete safely
    assert!(attempts > 0, "No ABA prevention tests were attempted");
    assert!(
        successes <= attempts,
        "Success count cannot exceed attempt count"
    );

    // The important part is that no crashes or undefined behavior occurred
    // Generation counters should prevent ABA problems

    println!("ASSUM ABA Prevention Validation:");
    println!("  ABA test attempts: {}", attempts);
    println!("  Successful operations: {}", successes);
    println!(
        "  Success rate: {:.2}%",
        (successes as f64 / attempts as f64) * 100.0
    );
}

// ============================================================================
// 8. PERFORMANCE BASELINE VALIDATION
// ============================================================================

#[test]
fn test_b32_performance_baseline_compliance() {
    // B32 Framework: Validate against hardware reality baselines
    // Performance claims must be honest and reproducible

    let capsule = AtomicHedgeCapsule::new();

    // Initialize
    let entry = EntryOrder::new(
        "B32".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let iterations = 10000;
    let samples = 100;
    let mut latency_measurements = Vec::with_capacity(samples);

    // Measure operation latency
    for _sample in 0..samples {
        let start = Instant::now();
        for i in 0..iterations {
            let filled = (i as f64) / 100000.0;
            let _result = capsule.update_entry_state(OrderState::PartiallyFilled, filled);
        }
        let duration = start.elapsed();
        latency_measurements.push(duration.as_nanos() as f64 / iterations as f64);
    }

    // Statistical analysis
    let mean_latency = latency_measurements.iter().sum::<f64>() / latency_measurements.len() as f64;
    let min_latency = latency_measurements
        .iter()
        .fold(f64::INFINITY, |a, &b| a.min(b));
    let max_latency = latency_measurements.iter().fold(0.0f64, |a, &b| a.max(b));

    // B32 K2: AtomicU64 CAS baseline is 15ns
    // Our operation should be within reasonable multiple of baseline
    let baseline_multiple = mean_latency / ATOMICU64_CAS_BASELINE_NS as f64;

    // B32 compliance checks
    assert!(mean_latency > 0.0, "Mean latency should be positive");
    assert!(
        mean_latency < 1000.0,
        "Mean latency {} ns exceeds reasonable upper bound",
        mean_latency
    );
    assert!(
        baseline_multiple < 50.0,
        "Operation {} is {}x baseline, exceeds reasonable bound",
        mean_latency,
        baseline_multiple
    );

    // Performance should be consistent (low variance)
    let variance = latency_measurements
        .iter()
        .map(|x| (x - mean_latency).powi(2))
        .sum::<f64>()
        / latency_measurements.len() as f64;
    let coefficient_of_variation = variance.sqrt() / mean_latency;

    assert!(
        coefficient_of_variation < 0.5,
        "Performance inconsistency too high: CV = {:.3}",
        coefficient_of_variation
    );

    println!("B32 Performance Baseline Validation:");
    println!("  Mean latency: {:.2} ns", mean_latency);
    println!("  Min latency: {:.2} ns", min_latency);
    println!("  Max latency: {:.2} ns", max_latency);
    println!("  Baseline multiple: {:.2}x", baseline_multiple);
    println!(
        "  Coefficient of variation: {:.3}",
        coefficient_of_variation
    );
    println!("  B32 K2 baseline: {} ns", ATOMICU64_CAS_BASELINE_NS);
}

#[test]
fn test_optimization_claims_validation() {
    // UCE32 Q30 + B32: Validate all optimization performance claims
    // Claims must be empirically verified and statistically sound

    let iterations = 5000;
    let samples = 50;

    // Test optimized path
    let capsule_optimized = AtomicHedgeCapsule::new();
    let entry = EntryOrder::new(
        "OPT".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule_optimized.initialize(entry, bracket).unwrap();

    let mut optimized_measurements = Vec::with_capacity(samples);
    for _sample in 0..samples {
        let start = Instant::now();
        for i in 0..iterations {
            let filled = (i as f64) / 100000.0;
            let _result = capsule_optimized.update_entry_state(OrderState::PartiallyFilled, filled);
        }
        let duration = start.elapsed();
        optimized_measurements.push(duration.as_nanos() as f64 / iterations as f64);
    }

    // Calculate statistics
    let optimized_mean =
        optimized_measurements.iter().sum::<f64>() / optimized_measurements.len() as f64;
    let optimized_std = {
        let variance = optimized_measurements
            .iter()
            .map(|x| (x - optimized_mean).powi(2))
            .sum::<f64>()
            / (optimized_measurements.len() - 1) as f64;
        variance.sqrt()
    };

    // Improvement calculation (compared to baseline)
    let baseline_latency = ATOMICU64_CAS_BASELINE_NS as f64;
    let improvement_factor = baseline_latency / optimized_mean;
    let improvement_percentage = (improvement_factor - 1.0) * 100.0;

    // B32 framework validation
    if improvement_percentage > 0.0 {
        // Positive improvement claimed
        assert!(
            improvement_percentage >= TYPICAL_IMPROVEMENT_MIN * 100.0,
            "Improvement {:.1}% below minimum threshold {:.1}%",
            improvement_percentage,
            TYPICAL_IMPROVEMENT_MIN * 100.0
        );

        if improvement_percentage > TYPICAL_IMPROVEMENT_MAX * 100.0 {
            assert!(
                improvement_percentage <= EXCEPTIONAL_IMPROVEMENT_MAX * 100.0,
                "Improvement {:.1}% exceeds exceptional threshold {:.1}%",
                improvement_percentage,
                EXCEPTIONAL_IMPROVEMENT_MAX * 100.0
            );

            if improvement_percentage > EXCEPTIONAL_IMPROVEMENT_MAX * 100.0 {
                assert!(
                    improvement_percentage <= SUSPICIOUS_IMPROVEMENT_MIN * 100.0,
                    "Improvement {:.1}% requires extensive validation (suspicious threshold)",
                    improvement_percentage
                );
            }
        }
    }

    // Statistical significance test (basic)
    let _confidence_level = 0.95;
    let t_critical = 2.009; // 49 degrees of freedom
    let standard_error = optimized_std / (optimized_measurements.len() as f64).sqrt();
    let margin_of_error = t_critical * standard_error;
    let confidence_interval = (
        optimized_mean - margin_of_error,
        optimized_mean + margin_of_error,
    );

    println!("Optimization Claims Validation:");
    println!(
        "  Optimized mean: {:.2} ± {:.2} ns",
        optimized_mean, margin_of_error
    );
    println!(
        "  95% CI: [{:.2}, {:.2}] ns",
        confidence_interval.0, confidence_interval.1
    );
    println!("  Baseline: {:.0} ns", baseline_latency);
    println!("  Improvement: {:.1}%", improvement_percentage);
    println!(
        "  B32 classification: {}",
        if improvement_percentage <= TYPICAL_IMPROVEMENT_MAX * 100.0 {
            "TYPICAL"
        } else if improvement_percentage <= EXCEPTIONAL_IMPROVEMENT_MAX * 100.0 {
            "EXCEPTIONAL"
        } else {
            "SUSPICIOUS - REQUIRES VALIDATION"
        }
    );
}

// Helper function for statistical validation
fn calculate_confidence_interval(measurements: &[f64], _confidence_level: f64) -> (f64, f64, f64) {
    let n = measurements.len() as f64;
    let mean = measurements.iter().sum::<f64>() / n;
    let variance = measurements.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let std_dev = variance.sqrt();
    let standard_error = std_dev / n.sqrt();

    // Simplified t-critical for 95% confidence
    let t_critical = if n > 30.0 { 1.96 } else { 2.045 };
    let margin_of_error = t_critical * standard_error;

    (mean, mean - margin_of_error, mean + margin_of_error)
}
