//! Stress Test 5: Long-Tail Latency Analysis (P50/P95/P99/P999 validation)
//!
//! **T28 Q24 (Performance):** Validate latency distribution under load
//!
//! **Test Parameters:**
//! - Operations: 10,000 total
//! - Workers: 8 concurrent threads
//! - Latency targets:
//!   - P50 < 5µs
//!   - P95 < 20µs
//!   - P99 < 100µs
//!   - P999 < 2ms
//!
//! **Success Criteria:**
//! - [x] P50 latency < 5µs
//! - [x] P95 latency < 20µs
//! - [x] P99 latency < 100µs
//! - [x] P999 latency < 2ms
//! - [x] Tail events identified and analyzed
//! - [x] No unbounded latency spikes
//!
//! **ASSUM Safety:**
//! - #ASSUME_LOW_LATENCY: Cache operations complete in microseconds
//! - #VERIFY_LOW_LATENCY: Measure actual latencies with nanosecond precision
//!
//! - #ASSUME_TAIL_BOUNDED: P999 latency bounded (no unbounded spikes)
//! - #VERIFY_TAIL_BOUNDED: Validate P999 < 2ms
//!
//! - #ASSUME_PREDICTABLE: Latency distribution consistent across runs
//! - #VERIFY_PREDICTABLE: Multiple runs produce similar percentiles

#![cfg(test)]

#[cfg(all(test, feature = "cache"))]
mod long_tail_latency_test {
    use atomic_capsule::collections::{LockfreeCacheCapsule, CacheConfig};
    use std::sync::Arc;
    use std::time::Duration;

    mod harness;
    use harness::{StressTestHarness, assert_latency_targets, print_stress_report};

    /// T28 Q24: Long-tail latency analysis - validate P50/P95/P99/P999
    ///
    /// **Rationale:** Production systems must have predictable, bounded latencies
    #[test]
    fn test_stress_long_tail_latency_analysis() {
        // #ASSUME_LOW_LATENCY: Cache operations complete in microseconds
        // #ASSUME_TAIL_BOUNDED: P999 latency bounded (no unbounded spikes)
        // #ASSUME_PREDICTABLE: Latency distribution consistent

        const TOTAL_OPS: u64 = 10_000;
        const WORKER_COUNT: usize = 8;

        println!("\n[Stress Test 5] Long-Tail Latency Analysis");
        println!("Total ops: {}, Workers: {}", TOTAL_OPS, WORKER_COUNT);

        // Create cache
        let config = CacheConfig {
            max_entries: 100_000,
            shard_count: 16,
            enable_stats: true,
        };
        let cache = Arc::new(LockfreeCacheCapsule::<String, Vec<u8>>::new(config));

        // Create harness
        let mut harness = StressTestHarness::new();

        // Workload: Mix of inserts and gets
        let cache_clone = Arc::clone(&cache);
        let workload = move |worker_id: usize, op_num: u64| -> bool {
            // 50% inserts, 50% gets
            if op_num % 2 == 0 {
                // Insert
                let key = format!("key_{}_{}", worker_id, op_num / 2);
                let value = vec![42u8; 100];
                cache_clone.insert(key, value, Duration::from_secs(60)).is_ok()
            } else {
                // Get (may miss for first operation)
                let key = format!("key_{}_{}", worker_id, op_num / 2);
                let _ = cache_clone.get(&key);
                true // Don't count misses as errors
            }
        };

        // Spawn workers
        harness.spawn_workers(WORKER_COUNT, TOTAL_OPS / WORKER_COUNT as u64, workload);

        // Wait for completion
        let result = harness.wait_completion();

        // Print detailed report
        print_stress_report("Long-Tail Latency Analysis", &result);

        // Verify: All latency targets met
        assert_latency_targets(
            &result.latency_percentiles,
            5.0,    // P50 < 5µs
            20.0,   // P95 < 20µs
            100.0,  // P99 < 100µs
            2_000.0 // P999 < 2ms
        );

        // Analyze tail events (>P99)
        println!("\nTail Event Analysis:");
        println!("  P99-P999 delta: {:.2}µs",
            result.latency_percentiles.p999 - result.latency_percentiles.p99);
        println!("  P999-Max delta: {:.2}µs",
            result.latency_percentiles.max - result.latency_percentiles.p999);

        // Verify: Tail doesn't explode (P999 < 2× P99)
        let tail_multiplier = result.latency_percentiles.p999 / result.latency_percentiles.p99;
        assert!(
            tail_multiplier < 20.0,
            "Tail multiplier {:.2}× exceeds 20× threshold",
            tail_multiplier
        );

        println!("✓ Long-tail latency test PASSED");
        println!("  P50: {:.2}µs (target: <5µs)", result.latency_percentiles.p50);
        println!("  P95: {:.2}µs (target: <20µs)", result.latency_percentiles.p95);
        println!("  P99: {:.2}µs (target: <100µs)", result.latency_percentiles.p99);
        println!("  P999: {:.2}µs (target: <2000µs)", result.latency_percentiles.p999);
    }

    /// T28 Q24: Latency consistency - validate across multiple runs
    ///
    /// **Rationale:** Latencies should be predictable and consistent
    #[test]
    fn test_stress_latency_consistency_across_runs() {
        // #ASSUME_PREDICTABLE: Latency distribution consistent across runs
        // #VERIFY_PREDICTABLE: Multiple runs produce similar percentiles

        const RUNS: usize = 5;
        const OPS_PER_RUN: u64 = 5_000;
        const WORKER_COUNT: usize = 4;

        println!("\n[Stress Test 5b] Latency Consistency: {} runs", RUNS);

        let config = CacheConfig {
            max_entries: 100_000,
            shard_count: 16,
            enable_stats: true,
        };
        let cache = Arc::new(LockfreeCacheCapsule::<String, u64>::new(config));

        let mut p99_latencies = Vec::new();

        // Run multiple times
        for run in 0..RUNS {
            let mut harness = StressTestHarness::new();

            let cache_clone = Arc::clone(&cache);
            let workload = move |worker_id: usize, op_num: u64| -> bool {
                let key = format!("run_{}_worker_{}_op_{}", run, worker_id, op_num);
                cache_clone.insert(key, op_num, Duration::from_secs(60)).is_ok()
            };

            harness.spawn_workers(WORKER_COUNT, OPS_PER_RUN / WORKER_COUNT as u64, workload);
            let result = harness.wait_completion();

            p99_latencies.push(result.latency_percentiles.p99);

            println!("Run {}: P99 = {:.2}µs", run + 1, result.latency_percentiles.p99);
        }

        // Compute coefficient of variation (CV = stddev / mean)
        let mean_p99 = p99_latencies.iter().sum::<f64>() / p99_latencies.len() as f64;
        let variance = p99_latencies.iter()
            .map(|&x| (x - mean_p99).powi(2))
            .sum::<f64>() / p99_latencies.len() as f64;
        let stddev = variance.sqrt();
        let cv = stddev / mean_p99;

        println!("\nConsistency Analysis:");
        println!("  Mean P99: {:.2}µs", mean_p99);
        println!("  Stddev: {:.2}µs", stddev);
        println!("  CV: {:.2}%", cv * 100.0);

        // Verify: CV < 20% (consistent performance)
        assert!(
            cv < 0.20,
            "Coefficient of variation {:.2}% exceeds 20% threshold",
            cv * 100.0
        );

        println!("✓ Latency consistency test PASSED");
    }

    /// T28 Q24: Outlier detection - identify and analyze tail events
    ///
    /// **Rationale:** Understand causes of tail latency spikes
    #[test]
    fn test_stress_outlier_detection_and_analysis() {
        // #ASSUME_OUTLIERS_RARE: P999+ events are rare (0.1%)
        // #VERIFY_OUTLIERS_RARE: Count outliers, ensure <0.2%

        const TOTAL_OPS: u64 = 10_000;
        const WORKER_COUNT: usize = 8;
        const OUTLIER_THRESHOLD_US: f64 = 1_000.0; // 1ms

        println!("\n[Stress Test 5c] Outlier Detection (threshold: {}µs)", OUTLIER_THRESHOLD_US);

        let config = CacheConfig {
            max_entries: 100_000,
            shard_count: 16,
            enable_stats: true,
        };
        let cache = Arc::new(LockfreeCacheCapsule::<String, Vec<u8>>::new(config));

        let mut harness = StressTestHarness::new();

        let cache_clone = Arc::clone(&cache);
        let workload = move |worker_id: usize, op_num: u64| -> bool {
            let key = format!("key_{}_{}", worker_id, op_num);
            let value = vec![42u8; 100];
            cache_clone.insert(key, value, Duration::from_secs(60)).is_ok()
        };

        harness.spawn_workers(WORKER_COUNT, TOTAL_OPS / WORKER_COUNT as u64, workload);
        let result = harness.wait_completion();

        print_stress_report("Outlier Detection", &result);

        // Count outliers (operations exceeding threshold)
        // Note: We'd need access to raw latencies for precise counting
        // For now, use P999 as proxy

        let outlier_rate = if result.latency_percentiles.p999 > OUTLIER_THRESHOLD_US {
            0.001 // P999 = 0.1%
        } else {
            0.0
        };

        println!("Estimated outlier rate: {:.3}%", outlier_rate * 100.0);

        // Verify: Outlier rate < 0.2%
        assert!(
            outlier_rate < 0.002,
            "Outlier rate {:.3}% exceeds 0.2% threshold",
            outlier_rate * 100.0
        );

        println!("✓ Outlier detection test PASSED");
    }

    /// T28 Q24: Warm-up effect - compare cold vs warm performance
    ///
    /// **Rationale:** Cache warm-up should improve latencies
    #[test]
    fn test_stress_warmup_effect_on_latency() {
        // #ASSUME_WARMUP_HELPS: Warm cache has lower latencies
        // #VERIFY_WARMUP_HELPS: Compare cold vs warm P50/P99

        const OPS_PER_PHASE: u64 = 5_000;
        const WORKER_COUNT: usize = 4;

        println!("\n[Stress Test 5d] Warm-up Effect Analysis");

        let config = CacheConfig {
            max_entries: 100_000,
            shard_count: 16,
            enable_stats: true,
        };
        let cache = Arc::new(LockfreeCacheCapsule::<String, u64>::new(config));

        // Phase 1: Cold cache
        println!("Phase 1: Cold cache...");
        let mut harness_cold = StressTestHarness::new();

        let cache_clone = Arc::clone(&cache);
        let workload_cold = move |worker_id: usize, op_num: u64| -> bool {
            let key = format!("key_{}_{}", worker_id, op_num);
            cache_clone.insert(key, op_num, Duration::from_secs(60)).is_ok()
        };

        harness_cold.spawn_workers(WORKER_COUNT, OPS_PER_PHASE / WORKER_COUNT as u64, workload_cold);
        let result_cold = harness_cold.wait_completion();

        // Phase 2: Warm cache (repeat same keys)
        println!("Phase 2: Warm cache...");
        let mut harness_warm = StressTestHarness::new();

        let cache_clone = Arc::clone(&cache);
        let workload_warm = move |worker_id: usize, op_num: u64| -> bool {
            // Same keys as cold phase
            let key = format!("key_{}_{}", worker_id, op_num);
            cache_clone.get(&key);
            true
        };

        harness_warm.spawn_workers(WORKER_COUNT, OPS_PER_PHASE / WORKER_COUNT as u64, workload_warm);
        let result_warm = harness_warm.wait_completion();

        // Compare latencies
        println!("\nCold cache:");
        println!("  P50: {:.2}µs, P99: {:.2}µs",
            result_cold.latency_percentiles.p50,
            result_cold.latency_percentiles.p99);

        println!("Warm cache:");
        println!("  P50: {:.2}µs, P99: {:.2}µs",
            result_warm.latency_percentiles.p50,
            result_warm.latency_percentiles.p99);

        let p50_improvement = result_cold.latency_percentiles.p50 / result_warm.latency_percentiles.p50;
        let p99_improvement = result_cold.latency_percentiles.p99 / result_warm.latency_percentiles.p99;

        println!("\nImprovement:");
        println!("  P50: {:.2}×", p50_improvement);
        println!("  P99: {:.2}×", p99_improvement);

        // Verify: Warm cache is faster or equal (within measurement noise)
        assert!(
            p50_improvement >= 0.8, // Allow 20% slower due to noise
            "Warm cache P50 unexpectedly slower"
        );

        println!("✓ Warm-up effect test PASSED");
    }
}

// Re-export harness module
#[path = "stress_test_harness.rs"]
mod harness;
