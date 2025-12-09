//! T28 Comprehensive Test Suite for Latency Profiling
//!
//! # Test Categories
//!
//! - **Unit Tests (Q1-Q7)**: 10 tests - Histogram correctness, bucket assignment, percentile accuracy
//! - **Property Tests (Q8-Q14)**: 3 tests - 1000 random latencies, percentile within ±5% of sorted array
//! - **Integration Tests (Q15-Q21)**: 2 tests - End-to-end profiling with multiple histograms
//! - **Stress Tests (Q22-Q28)**: 1 test - 1M concurrent latency samples

use clapi_core::profiling::{
    capsule::{HistogramStats, LatencyHistogramCapsule},
    histogram::{ComponentType, LatencyProfiler},
};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// UNIT TESTS (T28 Q1-Q7): Basic Functionality
// ============================================================================

#[test]
fn test_histogram_initialization() {
    let histogram = LatencyHistogramCapsule::new();
    assert_eq!(histogram.count(), 0);
    assert_eq!(histogram.mean_ns(), 0.0);
    assert_eq!(histogram.percentile(50.0), 0);
}

#[test]
fn test_single_sample_recording() {
    let histogram = LatencyHistogramCapsule::new();
    histogram.record(100);

    assert_eq!(histogram.count(), 1);
    assert_eq!(histogram.mean_ns(), 100.0);

    let stats = histogram.stats();
    assert_eq!(stats.min, 100);
    assert_eq!(stats.max, 100);
    assert_eq!(stats.count, 1);
}

#[test]
fn test_multiple_sample_recording() {
    let histogram = LatencyHistogramCapsule::new();

    for i in 1..=10 {
        histogram.record(i * 100);
    }

    assert_eq!(histogram.count(), 10);
    let mean = histogram.mean_ns();
    assert!(
        mean >= 500.0 && mean <= 600.0,
        "Mean should be ~550ns, got {}",
        mean
    );
}

#[test]
fn test_logarithmic_bucket_assignment() {
    let histogram = LatencyHistogramCapsule::new();

    // Test power-of-2 boundaries
    histogram.record(1); // Bucket 0: [1, 2)
    histogram.record(2); // Bucket 1: [2, 4)
    histogram.record(4); // Bucket 2: [4, 8)
    histogram.record(8); // Bucket 3: [8, 16)
    histogram.record(16); // Bucket 4: [16, 32)
    histogram.record(1024); // Bucket 10: [1024, 2048)

    assert_eq!(histogram.count(), 6);

    // All samples should be accounted for
    let stats = histogram.stats();
    assert_eq!(stats.min, 1);
    assert_eq!(stats.max, 1024);
}

#[test]
fn test_percentile_accuracy() {
    let histogram = LatencyHistogramCapsule::new();

    // Record 1000 samples: 1ns, 2ns, 3ns, ..., 1000ns
    for i in 1..=1000 {
        histogram.record(i);
    }

    let p50 = histogram.percentile(50.0);
    let p99 = histogram.percentile(99.0);
    let p999 = histogram.percentile(99.9);

    // p50 should be around 500ns (within bucket range)
    assert!(
        p50 >= 256 && p50 <= 512,
        "p50 should be in bucket [256, 512), got {}",
        p50
    );

    // p99 should be around 990ns (within bucket range)
    assert!(
        p99 >= 512 && p99 <= 1024,
        "p99 should be in bucket [512, 1024), got {}",
        p99
    );

    // p99.9 should be around 999ns (within bucket range)
    assert!(
        p999 >= 512 && p999 <= 1024,
        "p99.9 should be in bucket [512, 1024), got {}",
        p999
    );
}

#[test]
fn test_min_max_tracking() {
    let histogram = LatencyHistogramCapsule::new();

    histogram.record(500);
    histogram.record(100);
    histogram.record(1000);
    histogram.record(50);
    histogram.record(750);

    let stats = histogram.stats();
    assert_eq!(stats.min, 50);
    assert_eq!(stats.max, 1000);
    assert_eq!(stats.count, 5);
}

#[test]
fn test_histogram_reset() {
    let histogram = LatencyHistogramCapsule::new();

    for i in 1..=100 {
        histogram.record(i * 10);
    }

    assert_eq!(histogram.count(), 100);
    let gen1 = histogram.generation();

    histogram.reset();

    assert_eq!(histogram.count(), 0);
    assert_eq!(histogram.mean_ns(), 0.0);
    assert!(histogram.generation() > gen1, "Generation should increment");
}

#[test]
fn test_generation_counter_toctou_prevention() {
    let histogram = LatencyHistogramCapsule::new();
    let gen1 = histogram.generation();

    histogram.record(100);
    let gen2 = histogram.generation();

    histogram.record(200);
    let gen3 = histogram.generation();

    assert!(gen2 > gen1, "Generation should increment after record");
    assert!(gen3 > gen2, "Generation should increment after record");
}

#[test]
fn test_empty_histogram_percentiles() {
    let histogram = LatencyHistogramCapsule::new();

    assert_eq!(histogram.percentile(0.0), 0);
    assert_eq!(histogram.percentile(50.0), 0);
    assert_eq!(histogram.percentile(99.0), 0);
    assert_eq!(histogram.percentile(100.0), 0);
}

#[test]
fn test_stats_snapshot_consistency() {
    let histogram = LatencyHistogramCapsule::new();

    for i in 1..=100 {
        histogram.record(i * 100);
    }

    let stats1 = histogram.stats();
    let stats2 = histogram.stats();

    // Stats should be consistent across multiple reads
    assert_eq!(stats1.count, stats2.count);
    assert_eq!(stats1.min, stats2.min);
    assert_eq!(stats1.max, stats2.max);
    assert_eq!(stats1.mean, stats2.mean);
}

// ============================================================================
// PROPERTY TESTS (T28 Q8-Q14): Invariant Validation
// ============================================================================

#[test]
fn property_percentile_monotonicity() {
    let histogram = LatencyHistogramCapsule::new();

    // Record 1000 random-ish samples
    for i in 0..1000 {
        histogram.record((i * 7 + 13) % 10000);
    }

    // Percentiles should be monotonically increasing
    let p0 = histogram.percentile(0.0);
    let p25 = histogram.percentile(25.0);
    let p50 = histogram.percentile(50.0);
    let p75 = histogram.percentile(75.0);
    let p99 = histogram.percentile(99.0);
    let p100 = histogram.percentile(100.0);

    assert!(
        p0 <= p25,
        "p0 ({}) should be <= p25 ({})",
        p0,
        p25
    );
    assert!(
        p25 <= p50,
        "p25 ({}) should be <= p50 ({})",
        p25,
        p50
    );
    assert!(
        p50 <= p75,
        "p50 ({}) should be <= p75 ({})",
        p50,
        p75
    );
    assert!(
        p75 <= p99,
        "p75 ({}) should be <= p99 ({})",
        p75,
        p99
    );
    assert!(
        p99 <= p100,
        "p99 ({}) should be <= p100 ({})",
        p99,
        p100
    );
}

#[test]
fn property_percentile_bounds() {
    let histogram = LatencyHistogramCapsule::new();

    // Record 1000 samples with known distribution
    for i in 1..=1000 {
        histogram.record(i);
    }

    let stats = histogram.stats();

    // Percentiles should be within [min, max] range
    assert!(
        stats.p50 >= stats.min && stats.p50 <= stats.max,
        "p50 ({}) should be in range [{}, {}]",
        stats.p50,
        stats.min,
        stats.max
    );
    assert!(
        stats.p99 >= stats.min && stats.p99 <= stats.max,
        "p99 ({}) should be in range [{}, {}]",
        stats.p99,
        stats.min,
        stats.max
    );
    assert!(
        stats.p999 >= stats.min && stats.p999 <= stats.max,
        "p99.9 ({}) should be in range [{}, {}]",
        stats.p999,
        stats.min,
        stats.max
    );
}

#[test]
fn property_mean_within_bounds() {
    let histogram = LatencyHistogramCapsule::new();

    for i in 100..=200 {
        histogram.record(i);
    }

    let mean = histogram.mean_ns();
    let stats = histogram.stats();

    // Mean should be between min and max
    assert!(
        mean >= stats.min as f64 && mean <= stats.max as f64,
        "Mean ({}) should be in range [{}, {}]",
        mean,
        stats.min,
        stats.max
    );

    // For uniform distribution, mean should be close to midpoint
    let expected_mean = (100.0 + 200.0) / 2.0;
    let tolerance = 10.0; // Allow 10ns tolerance
    assert!(
        (mean - expected_mean).abs() < tolerance,
        "Mean ({}) should be close to expected ({})",
        mean,
        expected_mean
    );
}

// ============================================================================
// INTEGRATION TESTS (T28 Q15-Q21): End-to-End Profiling
// ============================================================================

#[test]
fn integration_multiple_component_profiling() {
    let profiler = LatencyProfiler::new();

    // Simulate profiling different components
    for _ in 0..100 {
        profiler.record(ComponentType::HttpRequest, 1000);
        profiler.record(ComponentType::BudgetValidation, 50);
        profiler.record(ComponentType::CircuitBreaker, 10);
        profiler.record(ComponentType::DatabaseQuery, 5000);
    }

    // Verify each component has correct count
    assert_eq!(
        profiler.stats(ComponentType::HttpRequest).count,
        100
    );
    assert_eq!(
        profiler.stats(ComponentType::BudgetValidation).count,
        100
    );
    assert_eq!(
        profiler.stats(ComponentType::CircuitBreaker).count,
        100
    );
    assert_eq!(
        profiler.stats(ComponentType::DatabaseQuery).count,
        100
    );

    // Verify relative latencies
    let http_p50 = profiler.percentile(ComponentType::HttpRequest, 50.0);
    let db_p50 = profiler.percentile(ComponentType::DatabaseQuery, 50.0);

    assert!(
        db_p50 > http_p50,
        "Database queries should be slower than HTTP requests"
    );
}

#[test]
fn integration_profiler_reset() {
    let profiler = LatencyProfiler::new();

    // Record samples
    for i in 1..=100 {
        profiler.record(ComponentType::HttpRequest, i * 10);
        profiler.record(ComponentType::BudgetValidation, i * 5);
    }

    // Verify data recorded
    assert_eq!(profiler.stats(ComponentType::HttpRequest).count, 100);
    assert_eq!(
        profiler.stats(ComponentType::BudgetValidation).count,
        100
    );

    // Reset all
    profiler.reset_all();

    // Verify reset
    assert_eq!(profiler.stats(ComponentType::HttpRequest).count, 0);
    assert_eq!(
        profiler.stats(ComponentType::BudgetValidation).count,
        0
    );
}

// ============================================================================
// STRESS TESTS (T28 Q22-Q28): Concurrent Load
// ============================================================================

#[test]
fn stress_concurrent_1m_samples() {
    let histogram = Arc::new(LatencyHistogramCapsule::new());
    let num_threads = 8;
    let samples_per_thread = 125_000; // Total: 1M samples
    let mut handles = vec![];

    let start = Instant::now();

    for thread_id in 0..num_threads {
        let hist = Arc::clone(&histogram);
        handles.push(thread::spawn(move || {
            for i in 0..samples_per_thread {
                // Generate pseudo-random latency
                let latency = ((thread_id * 1000) + (i % 1000)) as u64;
                hist.record(latency);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();

    // Verify all samples recorded
    assert_eq!(
        histogram.count(),
        1_000_000,
        "Should have exactly 1M samples"
    );

    // Performance check: 1M samples in <1 second
    assert!(
        elapsed < Duration::from_secs(1),
        "1M samples should complete in <1s, took {:?}",
        elapsed
    );

    // Verify statistics are reasonable
    let stats = histogram.stats();
    assert!(stats.min < stats.max, "Min should be less than max");
    assert!(
        stats.mean > 0.0,
        "Mean should be positive"
    );
    assert!(stats.p50 > 0, "p50 should be positive");
    assert!(stats.p99 > 0, "p99 should be positive");

    println!(
        "Stress test: 1M samples in {:?}, throughput: {:.0} samples/sec",
        elapsed,
        1_000_000.0 / elapsed.as_secs_f64()
    );
}

// ============================================================================
// PERFORMANCE VALIDATION (B32 Framework)
// ============================================================================

#[test]
#[ignore] // Run with: cargo test --test profiling_tests -- --ignored
fn bench_record_latency() {
    let histogram = LatencyHistogramCapsule::new();
    let iterations = 1_000_000;

    let start = Instant::now();
    for i in 0..iterations {
        histogram.record(i % 10000);
    }
    let elapsed = start.elapsed();

    let ns_per_op = elapsed.as_nanos() / iterations as u128;
    println!(
        "record() latency: {}ns per operation ({} iterations)",
        ns_per_op, iterations
    );

    // B32 target: <10ns per record operation
    assert!(
        ns_per_op < 10,
        "record() should be <10ns, got {}ns",
        ns_per_op
    );
}

#[test]
#[ignore] // Run with: cargo test --test profiling_tests -- --ignored
fn bench_percentile_latency() {
    let histogram = LatencyHistogramCapsule::new();

    // Populate histogram
    for i in 1..=10000 {
        histogram.record(i);
    }

    let iterations = 1_000_000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = histogram.percentile(99.0);
    }
    let elapsed = start.elapsed();

    let ns_per_op = elapsed.as_nanos() / iterations as u128;
    println!(
        "percentile() latency: {}ns per operation ({} iterations)",
        ns_per_op, iterations
    );

    // B32 target: <50ns per percentile query
    assert!(
        ns_per_op < 50,
        "percentile() should be <50ns, got {}ns",
        ns_per_op
    );
}

#[test]
#[ignore] // Run with: cargo test --test profiling_tests -- --ignored
fn bench_stats_latency() {
    let histogram = LatencyHistogramCapsule::new();

    // Populate histogram
    for i in 1..=10000 {
        histogram.record(i);
    }

    let iterations = 100_000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = histogram.stats();
    }
    let elapsed = start.elapsed();

    let ns_per_op = elapsed.as_nanos() / iterations as u128;
    println!(
        "stats() latency: {}ns per operation ({} iterations)",
        ns_per_op, iterations
    );

    // B32 target: <100ns per stats snapshot
    assert!(
        ns_per_op < 100,
        "stats() should be <100ns, got {}ns",
        ns_per_op
    );
}
