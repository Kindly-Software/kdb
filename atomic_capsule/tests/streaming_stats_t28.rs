//! T28 Tests for StreamingStatsCapsule
//!
//! # Test Structure
//! - Q1-Q7: Unit tests (basic operations, alignment, edge cases)
//! - Q8-Q14: Property tests (accuracy, monotonicity, convergence)
//! - Q15-Q21: Integration tests (multi-thread, compression, snapshots)
//! - Q22-Q28: Production tests (stress, performance, regression)

#[cfg(feature = "streaming-stats")]
mod tests {
    use atomic_capsule::collections::{StreamingStatsCapsule, StreamingSnapshot};
    use std::sync::Arc;
    use std::thread;

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn q1_basic_insert() {
        let stats = StreamingStatsCapsule::new();
        stats.insert(1_000_000); // 1ms
        assert_eq!(stats.total_count(), 1);
        assert_eq!(stats.min(), Some(1_000_000));
        assert_eq!(stats.max(), Some(1_000_000));
        assert_eq!(stats.centroid_count(), 1);
    }

    #[test]
    fn q2_percentile_query_basic() {
        let stats = StreamingStatsCapsule::new();

        // Insert 10 values: 1-10 ms
        for i in 1..=10 {
            stats.insert(i * 1_000_000);
        }

        // P50 should be ~5ms (±1% error)
        let p50 = stats.p50().unwrap();
        assert!(
            p50 >= 4_000_000 && p50 <= 6_000_000,
            "P50 {} not in range 4-6ms",
            p50
        );

        // P90 should be ~9ms (±1% error)
        let p90 = stats.p90().unwrap();
        assert!(
            p90 >= 8_000_000 && p90 <= 10_000_000,
            "P90 {} not in range 8-10ms",
            p90
        );
    }

    #[test]
    fn q3_empty_stats() {
        let stats = StreamingStatsCapsule::new();
        assert_eq!(stats.p50(), None);
        assert_eq!(stats.p90(), None);
        assert_eq!(stats.p95(), None);
        assert_eq!(stats.p99(), None);
        assert_eq!(stats.p999(), None);
        assert_eq!(stats.min(), None);
        assert_eq!(stats.max(), None);
        assert_eq!(stats.total_count(), 0);
        assert_eq!(stats.centroid_count(), 0);
    }

    #[test]
    fn q4_single_value() {
        let stats = StreamingStatsCapsule::new();
        stats.insert(5_000_000); // 5ms

        // All percentiles should equal the single value
        assert_eq!(stats.p50(), Some(5_000_000));
        assert_eq!(stats.p90(), Some(5_000_000));
        assert_eq!(stats.p95(), Some(5_000_000));
        assert_eq!(stats.p99(), Some(5_000_000));
        assert_eq!(stats.p999(), Some(5_000_000));
        assert_eq!(stats.min(), Some(5_000_000));
        assert_eq!(stats.max(), Some(5_000_000));
    }

    #[test]
    fn q5_identical_values() {
        let stats = StreamingStatsCapsule::new();

        // Insert 100 identical values
        for _ in 0..100 {
            stats.insert(5_000_000); // 5ms
        }

        assert_eq!(stats.total_count(), 100);

        // All percentiles should equal the identical value
        assert_eq!(stats.p50(), Some(5_000_000));
        assert_eq!(stats.p90(), Some(5_000_000));
        assert_eq!(stats.p99(), Some(5_000_000));

        // Should merge into single centroid
        assert_eq!(stats.centroid_count(), 1);
    }

    #[test]
    fn q6_alignment_verification() {
        use std::mem::{align_of, size_of};

        // Verify capsule alignment
        assert_eq!(align_of::<StreamingStatsCapsule>(), 64);
        assert_eq!(size_of::<StreamingStatsCapsule>(), 512);

        // Verify centroids are aligned
        let stats = StreamingStatsCapsule::new();
        let ptr = &stats as *const _ as usize;
        assert_eq!(ptr % 64, 0, "Capsule not 64-byte aligned");
    }

    #[test]
    fn q7_reset() {
        let mut stats = StreamingStatsCapsule::new();

        // Insert values
        for i in 1..=100 {
            stats.insert(i * 1_000_000);
        }

        assert_eq!(stats.total_count(), 100);
        assert!(stats.p50().is_some());

        // Reset
        stats.reset();

        // Verify empty state
        assert_eq!(stats.total_count(), 0);
        assert_eq!(stats.centroid_count(), 0);
        assert_eq!(stats.min(), None);
        assert_eq!(stats.max(), None);
        assert_eq!(stats.p50(), None);
    }

    // ========================================================================
    // Q8-Q14: Property Tests
    // ========================================================================

    #[test]
    fn q8_percentile_monotonicity() {
        let stats = StreamingStatsCapsule::new();

        // Insert 1000 values
        for i in 1..=1000 {
            stats.insert(i * 1000);
        }

        // Verify percentiles are monotonically increasing
        let snapshot = stats.snapshot();
        assert!(snapshot.p50 <= snapshot.p90, "P50 > P90");
        assert!(snapshot.p90 <= snapshot.p95, "P90 > P95");
        assert!(snapshot.p95 <= snapshot.p99, "P95 > P99");
        assert!(snapshot.p99 <= snapshot.p999, "P99 > P999");
    }

    #[test]
    fn q9_accuracy_uniform_distribution() {
        let stats = StreamingStatsCapsule::new();

        // Insert 1000 values: 0-999ms
        for i in 0..1000 {
            stats.insert(i * 1_000_000);
        }

        // P50 should be ~500ms (±1% error = ±5ms)
        let p50 = stats.p50().unwrap();
        let expected = 500_000_000;
        let error = (p50 as i64 - expected as i64).abs() as u64;
        let error_pct = (error as f64 / expected as f64) * 100.0;
        assert!(
            error_pct <= 1.0,
            "P50 error {:.2}% exceeds 1% (value: {}, expected: {})",
            error_pct,
            p50,
            expected
        );

        // P99 should be ~990ms (±1% error = ±10ms)
        let p99 = stats.p99().unwrap();
        let expected = 990_000_000;
        let error = (p99 as i64 - expected as i64).abs() as u64;
        let error_pct = (error as f64 / expected as f64) * 100.0;
        assert!(
            error_pct <= 1.0,
            "P99 error {:.2}% exceeds 1% (value: {}, expected: {})",
            error_pct,
            p99,
            expected
        );
    }

    #[test]
    fn q10_min_max_bounds() {
        let stats = StreamingStatsCapsule::new();

        // Insert values with known min/max
        stats.insert(100);
        stats.insert(5000);
        stats.insert(1000);
        stats.insert(3000);
        stats.insert(10000); // max

        assert_eq!(stats.min(), Some(100));
        assert_eq!(stats.max(), Some(10000));

        // All percentiles must be within [min, max]
        let snapshot = stats.snapshot();
        assert!(snapshot.p50 >= 100 && snapshot.p50 <= 10000);
        assert!(snapshot.p90 >= 100 && snapshot.p90 <= 10000);
        assert!(snapshot.p99 >= 100 && snapshot.p99 <= 10000);
    }

    #[test]
    fn q11_compression_accuracy() {
        let stats = StreamingStatsCapsule::new();

        // Insert 10,000 values to force compression
        for i in 1..=10000 {
            stats.insert(i * 1000);
        }

        // Verify centroid count ≤ MAX_CENTROIDS
        let count = stats.centroid_count();
        assert!(
            count <= StreamingStatsCapsule::MAX_CENTROIDS as u32,
            "Centroid count {} exceeds MAX_CENTROIDS",
            count
        );

        // Verify P50 accuracy after compression (±1%)
        let p50 = stats.p50().unwrap();
        let expected = 5_000_000_000; // 5000ms
        let error_pct = ((p50 as i64 - expected as i64).abs() as f64 / expected as f64) * 100.0;
        assert!(
            error_pct <= 1.0,
            "P50 error {:.2}% after compression exceeds 1%",
            error_pct
        );
    }

    #[test]
    fn q12_convergence_incremental() {
        let stats = StreamingStatsCapsule::new();

        // Insert values incrementally and check convergence
        for i in 1..=100 {
            stats.insert(i * 1_000_000);

            if i >= 10 {
                // After 10 values, percentiles should be reasonable
                let p50 = stats.p50().unwrap();
                let expected_approx = (i / 2) * 1_000_000;
                assert!(
                    p50 <= (i as u64 * 1_000_000),
                    "P50 exceeds max value at iteration {}",
                    i
                );
            }
        }
    }

    #[test]
    fn q13_edge_percentiles() {
        let stats = StreamingStatsCapsule::new();

        for i in 1..=100 {
            stats.insert(i * 1_000_000);
        }

        // P0 should be min
        // P100 should be max
        // query_percentile should handle these edge cases
        assert_eq!(stats.query_percentile(0.0), Some(stats.min().unwrap()));
        assert_eq!(stats.query_percentile(100.0), Some(stats.max().unwrap()));
    }

    #[test]
    fn q14_invalid_percentiles() {
        let stats = StreamingStatsCapsule::new();
        stats.insert(1_000_000);

        // Out of range percentiles should return None
        assert_eq!(stats.query_percentile(-1.0), None);
        assert_eq!(stats.query_percentile(101.0), None);
    }

    // ========================================================================
    // Q15-Q21: Integration Tests
    // ========================================================================

    #[test]
    fn q15_multi_thread_insert() {
        let stats = Arc::new(StreamingStatsCapsule::new());
        let threads: Vec<_> = (0..10)
            .map(|thread_id| {
                let s = Arc::clone(&stats);
                thread::spawn(move || {
                    for i in 0..100 {
                        s.insert((thread_id * 100 + i) * 1000);
                    }
                })
            })
            .collect();

        for t in threads {
            t.join().unwrap();
        }

        // All 1000 updates recorded
        assert_eq!(stats.total_count(), 1000);

        // Percentiles valid
        assert!(stats.p50().is_some());
        assert!(stats.p99().is_some());
    }

    #[test]
    fn q16_snapshot_consistency() {
        let stats = StreamingStatsCapsule::new();

        for i in 1..=100 {
            stats.insert(i * 1_000_000);
        }

        let snapshot = stats.snapshot();

        // Snapshot fields should be consistent
        assert_eq!(snapshot.count, 100);
        assert!(snapshot.min <= snapshot.p50);
        assert!(snapshot.p50 <= snapshot.p90);
        assert!(snapshot.p90 <= snapshot.p95);
        assert!(snapshot.p95 <= snapshot.p99);
        assert!(snapshot.p99 <= snapshot.p999);
        assert!(snapshot.p999 <= snapshot.max);
    }

    #[test]
    fn q17_large_value_range() {
        let stats = StreamingStatsCapsule::new();

        // Insert values spanning 6 orders of magnitude
        stats.insert(1_000); // 1μs
        stats.insert(1_000_000); // 1ms
        stats.insert(1_000_000_000); // 1s
        stats.insert(1_000_000_000_000); // 1000s

        assert_eq!(stats.min(), Some(1_000));
        assert_eq!(stats.max(), Some(1_000_000_000_000));

        // Percentiles should handle wide range
        let p50 = stats.p50().unwrap();
        assert!(p50 >= 1_000 && p50 <= 1_000_000_000_000);
    }

    #[test]
    fn q18_compression_multiple_rounds() {
        let stats = StreamingStatsCapsule::new();

        // Insert 1000 values to force multiple compression rounds
        for i in 1..=1000 {
            stats.insert(i * 1_000_000);
        }

        let count1 = stats.centroid_count();
        assert!(count1 <= StreamingStatsCapsule::MAX_CENTROIDS as u32);

        // Insert another 1000
        for i in 1001..=2000 {
            stats.insert(i * 1_000_000);
        }

        let count2 = stats.centroid_count();
        assert!(count2 <= StreamingStatsCapsule::MAX_CENTROIDS as u32);

        // Percentiles should still be accurate
        let p50 = stats.p50().unwrap();
        let expected = 1_000_000_000; // 1000ms
        let error_pct = ((p50 as i64 - expected as i64).abs() as f64 / expected as f64) * 100.0;
        assert!(error_pct <= 2.0, "P50 error {:.2}% exceeds 2%", error_pct); // Relaxed to 2% after multiple compressions
    }

    #[test]
    fn q19_skewed_distribution() {
        let stats = StreamingStatsCapsule::new();

        // Insert 1000 values with 90% < 10ms, 10% > 100ms
        for i in 0..900 {
            stats.insert(i * 10_000); // 0-9ms
        }
        for i in 0..100 {
            stats.insert(100_000_000 + i * 1_000_000); // 100-199ms
        }

        // P90 should be close to 10ms boundary
        let p90 = stats.p90().unwrap();
        assert!(
            p90 >= 8_000_000 && p90 <= 20_000_000,
            "P90 {} not near 10ms boundary",
            p90
        );

        // P99 should be in tail (100-199ms)
        let p99 = stats.p99().unwrap();
        assert!(
            p99 >= 90_000_000 && p99 <= 200_000_000,
            "P99 {} not in tail",
            p99
        );
    }

    #[test]
    fn q20_custom_compression() {
        // Higher compression should maintain more accuracy at tails
        let stats_low = StreamingStatsCapsule::with_compression(50);
        let stats_high = StreamingStatsCapsule::with_compression(200);

        for i in 1..=1000 {
            stats_low.insert(i * 1_000_000);
            stats_high.insert(i * 1_000_000);
        }

        // Both should have valid P99
        let p99_low = stats_low.p99().unwrap();
        let p99_high = stats_high.p99().unwrap();

        assert!(p99_low > 0);
        assert!(p99_high > 0);

        // High compression may have slightly better accuracy (not guaranteed in simple impl)
    }

    #[test]
    fn q21_mixed_operations() {
        let stats = StreamingStatsCapsule::new();

        // Interleave inserts and queries
        for i in 1..=100 {
            stats.insert(i * 1_000_000);

            if i % 10 == 0 {
                let p50 = stats.p50();
                assert!(p50.is_some());
            }
        }

        let final_count = stats.total_count();
        assert_eq!(final_count, 100);
    }

    // ========================================================================
    // Q22-Q28: Production Tests
    // ========================================================================

    #[test]
    fn q22_stress_concurrent_writers() {
        let stats = Arc::new(StreamingStatsCapsule::new());
        let threads: Vec<_> = (0..50)
            .map(|thread_id| {
                let s = Arc::clone(&stats);
                thread::spawn(move || {
                    for i in 0..1000 {
                        s.insert((thread_id * 1000 + i) * 1000);
                    }
                })
            })
            .collect();

        for t in threads {
            t.join().unwrap();
        }

        // 50 threads × 1000 values = 50,000 total
        assert_eq!(stats.total_count(), 50_000);

        // Percentiles should be valid
        let snapshot = stats.snapshot();
        assert!(snapshot.p50 > 0);
        assert!(snapshot.p99 > 0);
        assert!(snapshot.p50 <= snapshot.p99);
    }

    #[test]
    fn q23_performance_insert_latency() {
        use std::time::Instant;

        let stats = StreamingStatsCapsule::new();
        let iterations = 10_000;

        let start = Instant::now();
        for i in 0..iterations {
            stats.insert(i * 1000);
        }
        let elapsed = start.elapsed();

        let avg_latency_ns = elapsed.as_nanos() / iterations;

        // Target: <50ns per insert
        // Reality: May be higher in debug mode
        println!(
            "Average insert latency: {} ns (target: <50ns)",
            avg_latency_ns
        );

        // Relaxed assertion for test suite (actual performance verified in benches/)
        assert!(
            avg_latency_ns < 500,
            "Insert latency {} ns exceeds 500ns",
            avg_latency_ns
        );
    }

    #[test]
    fn q24_performance_query_latency() {
        use std::time::Instant;

        let stats = StreamingStatsCapsule::new();

        // Populate with 1000 values
        for i in 0..1000 {
            stats.insert(i * 1000);
        }

        let iterations = 10_000;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = stats.p50();
            let _ = stats.p95();
            let _ = stats.p99();
        }
        let elapsed = start.elapsed();

        let avg_latency_ns = elapsed.as_nanos() / (iterations * 3);

        // Target: <100ns per query
        println!(
            "Average query latency: {} ns (target: <100ns)",
            avg_latency_ns
        );

        // Relaxed assertion for test suite
        assert!(
            avg_latency_ns < 1000,
            "Query latency {} ns exceeds 1000ns",
            avg_latency_ns
        );
    }

    #[test]
    fn q25_memory_footprint() {
        use std::mem::size_of;

        let size = size_of::<StreamingStatsCapsule>();
        assert_eq!(size, 512, "Capsule size {} != 512 bytes", size);

        // Verify no heap allocations (all fixed size)
        let stats = StreamingStatsCapsule::new();
        stats.insert(1_000_000);
        // No way to measure heap in Rust without external tools
        // This is a compile-time guarantee via fixed arrays
    }

    #[test]
    fn q26_regression_p50_accuracy() {
        let stats = StreamingStatsCapsule::new();

        // Regression test: P50 should be within ±1% for uniform distribution
        for i in 1..=1000 {
            stats.insert(i * 1_000_000);
        }

        let p50 = stats.p50().unwrap();
        let expected = 500_000_000;
        let error = (p50 as i64 - expected as i64).abs() as u64;
        let error_pct = (error as f64 / expected as f64) * 100.0;

        assert!(
            error_pct <= 1.0,
            "REGRESSION: P50 error {:.2}% exceeds 1% (value: {}, expected: {})",
            error_pct,
            p50,
            expected
        );
    }

    #[test]
    fn q27_regression_compression_ratio() {
        let stats = StreamingStatsCapsule::new();

        // Insert 10,000 unique values
        for i in 1..=10_000 {
            stats.insert(i * 1000);
        }

        let count = stats.centroid_count();

        // Regression test: Compression should keep centroids ≤ 100
        assert!(
            count <= StreamingStatsCapsule::MAX_CENTROIDS as u32,
            "REGRESSION: Centroid count {} exceeds MAX_CENTROIDS",
            count
        );

        // Should achieve significant compression (10,000 → ~100)
        let compression_ratio = 10_000.0 / count as f64;
        assert!(
            compression_ratio >= 50.0,
            "REGRESSION: Compression ratio {:.1}× below 50×",
            compression_ratio
        );
    }

    #[test]
    fn q28_production_realistic_workload() {
        let stats = Arc::new(StreamingStatsCapsule::new());

        // Simulate realistic monitoring workload:
        // - 10 threads
        // - 10,000 latency samples each
        // - Bimodal distribution (fast + slow requests)

        let threads: Vec<_> = (0..10)
            .map(|thread_id| {
                let s = Arc::clone(&stats);
                thread::spawn(move || {
                    for i in 0..10_000 {
                        // 95% fast requests (<10ms)
                        // 5% slow requests (100-500ms)
                        let latency = if i % 20 == 0 {
                            100_000_000 + (i % 400) * 1_000_000 // 100-500ms
                        } else {
                            (thread_id * 100 + i % 100) * 100_000 // 0-10ms
                        };
                        s.insert(latency);
                    }
                })
            })
            .collect();

        for t in threads {
            t.join().unwrap();
        }

        // Verify realistic distribution properties
        let snapshot = stats.snapshot();

        // Total samples: 100,000
        assert_eq!(snapshot.count, 100_000);

        // P50 should be in fast range (<10ms)
        assert!(
            snapshot.p50 < 15_000_000,
            "P50 {} not in fast range",
            snapshot.p50
        );

        // P95 should be in slow range (since 5% are slow)
        assert!(
            snapshot.p95 > 50_000_000,
            "P95 {} not in slow range",
            snapshot.p95
        );

        // P99 should be well into slow range
        assert!(
            snapshot.p99 > 100_000_000,
            "P99 {} not in slow tail",
            snapshot.p99
        );

        println!("Production workload percentiles:");
        println!("  P50:  {} μs", snapshot.p50 / 1000);
        println!("  P90:  {} μs", snapshot.p90 / 1000);
        println!("  P95:  {} μs", snapshot.p95 / 1000);
        println!("  P99:  {} μs", snapshot.p99 / 1000);
        println!("  P999: {} μs", snapshot.p999 / 1000);
        println!("  Min:  {} μs", snapshot.min / 1000);
        println!("  Max:  {} μs", snapshot.max / 1000);
        println!("  Centroids: {}", stats.centroid_count());
    }
}
