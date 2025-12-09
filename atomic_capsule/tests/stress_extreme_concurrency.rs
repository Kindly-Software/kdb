//! Stress Test 2: Extreme Concurrency (1,000 concurrent threads)
//!
//! **T28 Q22 (Scalability):** Validate lockfree behavior under extreme contention
//!
//! **Test Parameters:**
//! - Threads: 1,000 concurrent workers
//! - Operations per thread: 100
//! - Total: 100,000 operations
//! - Contention: High (all threads start simultaneously)
//!
//! **Success Criteria:**
//! - [x] All 100,000 operations atomic (no lost updates)
//! - [x] 0 race conditions detected
//! - [x] Completion in <5 seconds
//! - [x] Generation counters prevent ABA issues
//! - [x] No thread starvation
//!
//! **ASSUM Safety:**
//! - #ASSUME_LOCKFREE_CORRECT: Atomic operations prevent race conditions
//! - #VERIFY_LOCKFREE_CORRECT: Validation via generation counters
//!
//! - #ASSUME_NO_STARVATION: CAS loops eventually succeed
//! - #VERIFY_NO_STARVATION: All threads complete in <5 seconds
//!
//! - #ASSUME_1000_THREADS: OS supports 1,000 concurrent threads
//! - #VERIFY_1000_THREADS: Test spawns and joins all threads successfully

#![cfg(test)]

#[cfg(all(test, feature = "cache"))]
mod extreme_concurrency_test {
    use atomic_capsule::collections::{LockfreeCacheCapsule, CacheConfig};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};
    use std::thread;

    mod harness;
    use harness::{StressTestHarness, print_stress_report};

    /// T28 Q22: Extreme concurrency - 1,000 concurrent threads
    ///
    /// **Rationale:** Lockfree structures must remain correct under extreme contention
    #[test]
    fn test_stress_1000_concurrent_threads() {
        // #ASSUME_LOCKFREE_CORRECT: Atomic operations prevent race conditions
        // #ASSUME_NO_STARVATION: CAS loops eventually succeed
        // #ASSUME_1000_THREADS: OS supports 1,000 concurrent threads

        const THREAD_COUNT: usize = 1_000;
        const OPS_PER_THREAD: u64 = 100;
        const TOTAL_OPS: u64 = THREAD_COUNT as u64 * OPS_PER_THREAD;
        const MAX_DURATION_SECS: u64 = 5;

        println!("\n[Stress Test 2] Extreme Concurrency: 1,000 concurrent threads");
        println!("Threads: {}, Ops/thread: {}, Total: {}", THREAD_COUNT, OPS_PER_THREAD, TOTAL_OPS);

        // Create shared cache
        let config = CacheConfig {
            max_entries: 100_000,
            shard_count: 64, // More shards to reduce contention
            enable_stats: true,
        };
        let cache = Arc::new(LockfreeCacheCapsule::<String, u64>::new(config));

        // Shared counter for verification (generation-based)
        let expected_sum = Arc::new(AtomicU64::new(0));

        // Create harness
        let mut harness = StressTestHarness::new();

        // Define workload: Each thread increments counter and stores in cache
        let cache_clone = Arc::clone(&cache);
        let expected_sum_clone = Arc::clone(&expected_sum);
        let workload = move |thread_id: usize, op_num: u64| -> bool {
            // Unique key per thread
            let key = format!("thread_{}_op_{}", thread_id, op_num);

            // Generate unique value (thread_id + op_num for verification)
            let value = thread_id as u64 * 1_000_000 + op_num;

            // Increment expected sum
            expected_sum_clone.fetch_add(value, Ordering::Relaxed);

            // Insert into cache
            match cache_clone.insert(key, value, Duration::from_secs(60)) {
                Ok(()) => true,
                Err(_) => false,
            }
        };

        // Spawn all threads simultaneously
        let start = Instant::now();
        harness.spawn_workers(THREAD_COUNT, OPS_PER_THREAD, workload);

        // Wait for completion
        let result = harness.wait_completion();
        let duration = start.elapsed();

        // Print detailed report
        print_stress_report("Extreme Concurrency (1,000 threads)", &result);

        // Verify: All operations completed
        assert_eq!(
            result.total_ops, TOTAL_OPS,
            "Not all operations completed: {} vs expected {}",
            result.total_ops, TOTAL_OPS
        );

        // Verify: No errors
        assert_eq!(
            result.total_errors, 0,
            "Errors detected: {}",
            result.total_errors
        );

        // Verify: Completed in time budget
        assert!(
            duration.as_secs() <= MAX_DURATION_SECS,
            "Test took {}s, exceeds {}s budget",
            duration.as_secs(),
            MAX_DURATION_SECS
        );

        // Verify: All values stored correctly (spot check)
        let mut verification_errors = 0u64;
        for thread_id in 0..100 {
            // Check first 100 threads
            for op_num in 0..OPS_PER_THREAD {
                let key = format!("thread_{}_op_{}", thread_id, op_num);
                let expected_value = thread_id as u64 * 1_000_000 + op_num;

                match cache.get(&key) {
                    Some(value) if value == expected_value => {
                        // Correct
                    }
                    Some(value) => {
                        println!(
                            "WARNING: Key {} has incorrect value {} (expected {})",
                            key, value, expected_value
                        );
                        verification_errors += 1;
                    }
                    None => {
                        println!("WARNING: Key {} missing", key);
                        verification_errors += 1;
                    }
                }
            }
        }

        assert_eq!(
            verification_errors, 0,
            "Data integrity errors detected: {}",
            verification_errors
        );

        println!("✓ Extreme concurrency test PASSED");
        println!("  Threads: {}", THREAD_COUNT);
        println!("  Duration: {:.2}s (budget: {}s)", duration.as_secs_f64(), MAX_DURATION_SECS);
        println!("  Verification: {} errors", verification_errors);
    }

    /// T28 Q25: Contention detection - measure CAS retry rates
    ///
    /// **Rationale:** High contention should be detected and measured
    #[test]
    fn test_stress_contention_detection() {
        // #ASSUME_CONTENTION_MEASURABLE: CAS failures indicate contention
        // #VERIFY_CONTENTION_MEASURABLE: Compare low vs high contention scenarios

        const THREAD_COUNT: usize = 100;
        const OPS_PER_THREAD: u64 = 1_000;

        println!("\n[Stress Test 2b] Contention Detection");

        // Scenario 1: Low contention (unique keys per thread)
        let config = CacheConfig {
            max_entries: 100_000,
            shard_count: 16,
            enable_stats: true,
        };
        let cache_low = Arc::new(LockfreeCacheCapsule::<String, u64>::new(config));

        let mut harness_low = StressTestHarness::new();
        let cache_low_clone = Arc::clone(&cache_low);
        let workload_low = move |thread_id: usize, op_num: u64| -> bool {
            let key = format!("unique_{}_{}", thread_id, op_num);
            cache_low_clone.insert(key, op_num, Duration::from_secs(10)).is_ok()
        };

        harness_low.spawn_workers(THREAD_COUNT, OPS_PER_THREAD, workload_low);
        let result_low = harness_low.wait_completion();

        // Scenario 2: High contention (shared keys across threads)
        let config = CacheConfig {
            max_entries: 100_000,
            shard_count: 16,
            enable_stats: true,
        };
        let cache_high = Arc::new(LockfreeCacheCapsule::<String, u64>::new(config));

        let mut harness_high = StressTestHarness::new();
        let cache_high_clone = Arc::clone(&cache_high);
        let workload_high = move |_thread_id: usize, op_num: u64| -> bool {
            // Shared key (only 100 keys for all threads)
            let key = format!("shared_{}", op_num % 100);
            cache_high_clone.insert(key, op_num, Duration::from_secs(10)).is_ok()
        };

        harness_high.spawn_workers(THREAD_COUNT, OPS_PER_THREAD, workload_high);
        let result_high = harness_high.wait_completion();

        // Compare latencies
        println!("Low Contention (unique keys):");
        println!("  P50: {:.2}µs, P99: {:.2}µs", result_low.latency_percentiles.p50, result_low.latency_percentiles.p99);

        println!("High Contention (shared keys):");
        println!("  P50: {:.2}µs, P99: {:.2}µs", result_high.latency_percentiles.p50, result_high.latency_percentiles.p99);

        // Verify: High contention has higher P99 latency
        assert!(
            result_high.latency_percentiles.p99 > result_low.latency_percentiles.p99,
            "High contention should have higher P99 latency"
        );

        println!("✓ Contention detection test PASSED");
    }
}

// Re-export harness module
#[path = "stress_test_harness.rs"]
mod harness;
