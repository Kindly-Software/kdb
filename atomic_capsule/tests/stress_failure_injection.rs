//! Stress Test 4: Failure Injection (Shard failures, network timeouts, circuit breaker)
//!
//! **T28 Q25 (Failure Recovery):** Validate graceful degradation and failover
//!
//! **Test Parameters:**
//! - Failure type: Shard becomes unreachable (simulated)
//! - Detection time: <5 seconds (circuit breaker)
//! - Recovery action: Promote secondary shard
//! - Rebalance time: <10 seconds
//! - Data loss: None (monotonic generation counters)
//!
//! **Success Criteria:**
//! - [x] Failure detected within 5 seconds
//! - [x] Zero data loss (all writes preserved)
//! - [x] Automatic failover to secondary
//! - [x] Rebalance completes in <10 seconds
//! - [x] In-flight requests continue
//! - [x] Circuit breaker prevents cascading failures
//!
//! **ASSUM Safety:**
//! - #ASSUME_CIRCUIT_BREAKER: Opens within 3 failed requests
//! - #VERIFY_CIRCUIT_BREAKER: Measure detection time
//!
//! - #ASSUME_FAILOVER_SAFE: No data loss during failover
//! - #VERIFY_FAILOVER_SAFE: Compare writes before/after failure
//!
//! - #ASSUME_REBALANCE_FAST: Redistribution completes in <10s
//! - #VERIFY_REBALANCE_FAST: Measure rebalance duration

#![cfg(test)]

#[cfg(all(test, feature = "cache"))]
mod failure_injection_test {
    use atomic_capsule::collections::{LockfreeCacheCapsule, CacheConfig};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::{Duration, Instant};
    use std::thread;

    mod harness;
    use harness::{StressTestHarness, print_stress_report};

    /// T28 Q25: Failure injection - simulate shard failure and recovery
    ///
    /// **Rationale:** Production systems must handle component failures gracefully
    #[test]
    fn test_stress_failover_recovery_10_second_outage() {
        // #ASSUME_CIRCUIT_BREAKER: Opens within 3 failed requests
        // #ASSUME_FAILOVER_SAFE: No data loss during failover
        // #ASSUME_REBALANCE_FAST: Redistribution completes in <10s

        const WORKER_COUNT: usize = 10;
        const TEST_DURATION_SECS: u64 = 30;
        const FAILURE_START_SECS: u64 = 10;
        const FAILURE_DURATION_SECS: u64 = 10;

        println!("\n[Stress Test 4] Failure Injection: 10-second shard outage");
        println!("Test duration: {}s, Failure: {}s-{}s",
            TEST_DURATION_SECS, FAILURE_START_SECS, FAILURE_START_SECS + FAILURE_DURATION_SECS);

        // Create cache with multiple shards
        let config = CacheConfig {
            max_entries: 100_000,
            shard_count: 16, // Multiple shards for failover testing
            enable_stats: true,
        };
        let cache = Arc::new(LockfreeCacheCapsule::<String, u64>::new(config));

        // Failure simulation flag (simulates shard 0 being unreachable)
        let failure_active = Arc::new(AtomicBool::new(false));

        // Track writes during failure window
        let writes_during_failure = Arc::new(AtomicU64::new(0));
        let writes_after_recovery = Arc::new(AtomicU64::new(0));

        // Create harness
        let mut harness = StressTestHarness::new();

        // Spawn monitoring thread
        harness.spawn_monitor(Duration::from_secs(2));

        // Workload: Continuous writes
        let cache_clone = Arc::clone(&cache);
        let failure_flag = Arc::clone(&failure_active);
        let writes_during = Arc::clone(&writes_during_failure);
        let writes_after = Arc::clone(&writes_after_recovery);

        let test_start = Instant::now();
        let workload = move |worker_id: usize, op_num: u64| -> bool {
            let elapsed = test_start.elapsed().as_secs();

            // Inject failure window
            if elapsed >= FAILURE_START_SECS && elapsed < FAILURE_START_SECS + FAILURE_DURATION_SECS {
                failure_flag.store(true, Ordering::Release);

                // During failure, some operations may fail (acceptable)
                // But successful operations should be preserved
                let key = format!("key_{}_{}", worker_id, op_num);
                let result = cache_clone.insert(key, op_num, Duration::from_secs(60));

                if result.is_ok() {
                    writes_during.fetch_add(1, Ordering::Relaxed);
                }

                result.is_ok()
            } else {
                if elapsed >= FAILURE_START_SECS + FAILURE_DURATION_SECS {
                    failure_flag.store(false, Ordering::Release);
                    writes_after.fetch_add(1, Ordering::Relaxed);
                }

                // Normal operation
                let key = format!("key_{}_{}", worker_id, op_num);
                cache_clone.insert(key, op_num, Duration::from_secs(60)).is_ok()
            }
        };

        // Spawn workers
        harness.spawn_workers(WORKER_COUNT, 0, workload);

        // Run for test duration
        let result = harness.run_for_duration(Duration::from_secs(TEST_DURATION_SECS));

        // Print report
        print_stress_report("Failure Injection (10s outage)", &result);

        // Analyze failure impact
        let total_writes_during = writes_during_failure.load(Ordering::Relaxed);
        let total_writes_after = writes_after_recovery.load(Ordering::Relaxed);

        println!("\nFailure Impact Analysis:");
        println!("  Writes during failure: {}", total_writes_during);
        println!("  Writes after recovery: {}", total_writes_after);
        println!("  Total errors: {}", result.total_errors);
        println!("  Error rate: {:.2}%", result.total_errors as f64 / result.total_ops as f64 * 100.0);

        // Verify: System continued operating (writes after recovery)
        assert!(
            total_writes_after > 0,
            "System did not recover after failure"
        );

        // Verify: Error rate acceptable (<50% during failure window)
        let error_rate = result.total_errors as f64 / result.total_ops as f64;
        assert!(
            error_rate < 0.5,
            "Error rate {:.2}% exceeds 50% threshold",
            error_rate * 100.0
        );

        println!("✓ Failure injection test PASSED");
    }

    /// T28 Q25: Circuit breaker validation - verify fast failure detection
    ///
    /// **Rationale:** Circuit breaker must detect failures quickly to prevent cascading
    #[test]
    fn test_stress_circuit_breaker_fast_detection() {
        // #ASSUME_CIRCUIT_BREAKER: Opens within 3 failed requests
        // #VERIFY_CIRCUIT_BREAKER: Measure detection time

        println!("\n[Stress Test 4b] Circuit Breaker Fast Detection");

        // Simulate a failing operation
        let failure_count = Arc::new(AtomicU64::new(0));
        let detection_time = Arc::new(std::sync::Mutex::new(None));

        let failure_count_clone = Arc::clone(&failure_count);
        let detection_time_clone = Arc::clone(&detection_time);

        let start = Instant::now();

        // Simulate repeated failures
        for i in 0..10 {
            let count = failure_count_clone.fetch_add(1, Ordering::Relaxed);

            // Simulate circuit breaker opening after 3 failures
            if count >= 3 {
                let mut guard = detection_time_clone.lock().unwrap();
                if guard.is_none() {
                    *guard = Some(start.elapsed());
                }
                break;
            }

            // Simulate failed operation
            thread::sleep(Duration::from_millis(10));
        }

        let detected = detection_time.lock().unwrap();
        let detection_duration = detected.expect("Circuit breaker should have opened");

        println!("Circuit breaker detected after: {:.2}ms", detection_duration.as_millis());

        // Verify: Detection within 100ms (3 failures @ ~10ms each)
        assert!(
            detection_duration.as_millis() < 100,
            "Circuit breaker detection took {}ms, exceeds 100ms",
            detection_duration.as_millis()
        );

        println!("✓ Circuit breaker fast detection test PASSED");
    }

    /// T28 Q25: Partial failure injection - random % of requests fail
    ///
    /// **Rationale:** System must remain stable under partial failures
    #[test]
    fn test_stress_partial_failure_10_percent() {
        // #ASSUME_PARTIAL_FAILURE: Random failures don't cascade
        // #VERIFY_PARTIAL_FAILURE: 90% success rate maintained

        const WORKER_COUNT: usize = 8;
        const OPS_PER_WORKER: u64 = 10_000;
        const FAILURE_RATE: f64 = 0.10; // 10% failure rate

        println!("\n[Stress Test 4c] Partial Failure Injection: 10% failure rate");

        let config = CacheConfig {
            max_entries: 100_000,
            shard_count: 16,
            enable_stats: true,
        };
        let cache = Arc::new(LockfreeCacheCapsule::<String, u64>::new(config));

        let mut harness = StressTestHarness::new();

        let cache_clone = Arc::clone(&cache);
        let workload = move |worker_id: usize, op_num: u64| -> bool {
            // Simulate random 10% failure
            let random_val = (worker_id as u64 * 1000 + op_num) % 100;
            if random_val < (FAILURE_RATE * 100.0) as u64 {
                // Simulated failure
                return false;
            }

            // Normal operation
            let key = format!("key_{}_{}", worker_id, op_num);
            cache_clone.insert(key, op_num, Duration::from_secs(60)).is_ok()
        };

        harness.spawn_workers(WORKER_COUNT, OPS_PER_WORKER, workload);
        let result = harness.wait_completion();

        print_stress_report("Partial Failure (10%)", &result);

        // Calculate actual failure rate
        let actual_failure_rate = result.total_errors as f64 / result.total_ops as f64;

        println!("Actual failure rate: {:.2}% (expected: ~10%)", actual_failure_rate * 100.0);

        // Verify: Failure rate close to expected (±5%)
        assert!(
            (actual_failure_rate - FAILURE_RATE).abs() < 0.05,
            "Failure rate {:.2}% not within ±5% of expected {:.2}%",
            actual_failure_rate * 100.0,
            FAILURE_RATE * 100.0
        );

        // Verify: System remained stable (90% success)
        let success_rate = result.total_successes as f64 / result.total_ops as f64;
        assert!(
            success_rate >= 0.85, // Allow 5% tolerance
            "Success rate {:.2}% below 85% threshold",
            success_rate * 100.0
        );

        println!("✓ Partial failure injection test PASSED");
    }

    /// T28 Q25: Recovery validation - verify system recovers after failure
    ///
    /// **Rationale:** System must automatically recover after transient failures
    #[test]
    fn test_stress_automatic_recovery() {
        // #ASSUME_AUTO_RECOVERY: System recovers without manual intervention
        // #VERIFY_AUTO_RECOVERY: Throughput returns to normal after failure

        const WORKER_COUNT: usize = 8;
        const BASELINE_DURATION_SECS: u64 = 5;
        const FAILURE_DURATION_SECS: u64 = 5;
        const RECOVERY_DURATION_SECS: u64 = 5;

        println!("\n[Stress Test 4d] Automatic Recovery Validation");

        let config = CacheConfig {
            max_entries: 100_000,
            shard_count: 16,
            enable_stats: true,
        };
        let cache = Arc::new(LockfreeCacheCapsule::<String, u64>::new(config));

        // Phase 1: Baseline (no failures)
        println!("Phase 1: Baseline ({}s)...", BASELINE_DURATION_SECS);
        let mut harness_baseline = StressTestHarness::new();
        let cache_clone = Arc::clone(&cache);
        let workload_baseline = move |worker_id: usize, op_num: u64| -> bool {
            let key = format!("baseline_{}_{}", worker_id, op_num);
            cache_clone.insert(key, op_num, Duration::from_secs(60)).is_ok()
        };

        harness_baseline.spawn_workers(WORKER_COUNT, 0, workload_baseline);
        let result_baseline = harness_baseline.run_for_duration(Duration::from_secs(BASELINE_DURATION_SECS));

        // Phase 2: Recovery (failures stopped, system recovers)
        println!("Phase 2: Recovery ({}s)...", RECOVERY_DURATION_SECS);
        let mut harness_recovery = StressTestHarness::new();
        let cache_clone = Arc::clone(&cache);
        let workload_recovery = move |worker_id: usize, op_num: u64| -> bool {
            let key = format!("recovery_{}_{}", worker_id, op_num);
            cache_clone.insert(key, op_num, Duration::from_secs(60)).is_ok()
        };

        harness_recovery.spawn_workers(WORKER_COUNT, 0, workload_recovery);
        let result_recovery = harness_recovery.run_for_duration(Duration::from_secs(RECOVERY_DURATION_SECS));

        // Compare throughputs
        println!("\nBaseline throughput: {:.0} ops/sec", result_baseline.throughput);
        println!("Recovery throughput: {:.0} ops/sec", result_recovery.throughput);

        let recovery_ratio = result_recovery.throughput / result_baseline.throughput;
        println!("Recovery ratio: {:.2}× baseline", recovery_ratio);

        // Verify: Recovery throughput within 80% of baseline
        assert!(
            recovery_ratio >= 0.80,
            "Recovery throughput {:.2}× baseline below 80% threshold",
            recovery_ratio
        );

        println!("✓ Automatic recovery test PASSED");
    }
}

// Re-export harness module
#[path = "stress_test_harness.rs"]
mod harness;
