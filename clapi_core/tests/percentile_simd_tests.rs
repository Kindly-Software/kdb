//! T28 Framework Tests for SIMD Percentile Implementation
//!
//! # T28 Test Coverage
//!
//! ## Q1-Q7: Unit Tests (8 tests)
//! - SIMD/scalar equivalence for standard distributions
//! - Edge cases (empty histogram, single sample, boundary percentiles)
//! - Bucket assignment correctness
//! - Transparent API behavior
//!
//! ## Q8-Q14: Property Tests (4 tests)
//! - Percentile monotonicity (p50 <= p99 <= p999)
//! - SIMD/scalar equivalence across all percentiles
//! - Batch percentile correctness
//! - Large histogram stress test (10K+ samples)
//!
//! ## Q15-Q21: Integration Tests (2 tests)
//! - End-to-end profiling with SIMD
//! - Multi-component profiling
//!
//! ## Total: 14+ tests
//!
//! # Build Instructions
//!
//! Stable Rust (scalar only):
//! ```bash
//! cargo test --test percentile_simd_tests
//! ```
//!
//! Nightly Rust (SIMD + scalar):
//! ```bash
//! cargo +nightly test --test percentile_simd_tests --features portable_simd
//! ```

use clapi_core::profiling::capsule::LatencyHistogramCapsule;

// ============================================================================
// T28 Q1-Q7: UNIT TESTS
// ============================================================================

#[test]
fn test_scalar_percentile_basic() {
    let histogram = LatencyHistogramCapsule::new();

    // Record 100 samples: 10ns, 20ns, 30ns, ..., 1000ns
    for i in 1..=100 {
        histogram.record(i * 10);
    }

    let p50 = histogram.percentile_scalar(50.0);
    let p99 = histogram.percentile_scalar(99.0);

    // Verify logarithmic bucketing behavior
    assert!(p50 >= 256 && p50 <= 512, "p50={} (expected [256, 512])", p50);
    assert!(p99 >= 512 && p99 <= 1024, "p99={} (expected [512, 1024])", p99);
}

#[cfg(feature = "portable_simd")]
#[test]
fn test_simd_scalar_equivalence_uniform() {
    let histogram = LatencyHistogramCapsule::new();

    // Uniform distribution: 0ns to 9999ns
    for i in 0..10000 {
        histogram.record(i);
    }

    // Test multiple percentiles for equivalence
    for p in [0.0, 10.0, 25.0, 50.0, 75.0, 90.0, 95.0, 99.0, 99.9, 100.0] {
        let simd_result = histogram.percentile_simd(p);
        let scalar_result = histogram.percentile_scalar(p);

        assert_eq!(
            simd_result, scalar_result,
            "SIMD/scalar mismatch at p{}: SIMD={}, scalar={}",
            p, simd_result, scalar_result
        );
    }
}

#[cfg(feature = "portable_simd")]
#[test]
fn test_simd_scalar_equivalence_skewed() {
    let histogram = LatencyHistogramCapsule::new();

    // Skewed distribution: mostly low latencies with some outliers
    for _ in 0..900 {
        histogram.record(100); // 90% at 100ns
    }
    for _ in 0..90 {
        histogram.record(1000); // 9% at 1μs
    }
    for _ in 0..10 {
        histogram.record(10000); // 1% at 10μs
    }

    let p50_simd = histogram.percentile_simd(50.0);
    let p50_scalar = histogram.percentile_scalar(50.0);
    let p99_simd = histogram.percentile_simd(99.0);
    let p99_scalar = histogram.percentile_scalar(99.0);

    assert_eq!(p50_simd, p50_scalar, "p50 mismatch");
    assert_eq!(p99_simd, p99_scalar, "p99 mismatch");
}

#[test]
fn test_edge_case_empty_histogram() {
    let histogram = LatencyHistogramCapsule::new();

    assert_eq!(histogram.percentile_scalar(50.0), 0);
    assert_eq!(histogram.percentile_scalar(99.0), 0);
    assert_eq!(histogram.percentile_optimized(50.0), 0);
}

#[test]
fn test_edge_case_single_sample() {
    let histogram = LatencyHistogramCapsule::new();
    histogram.record(256);

    let p0 = histogram.percentile_scalar(0.0);
    let p50 = histogram.percentile_scalar(50.0);
    let p100 = histogram.percentile_scalar(100.0);

    // All percentiles should return same bucket for single sample
    assert_eq!(p0, p50, "p0 != p50 for single sample");
    assert_eq!(p50, p100, "p50 != p100 for single sample");
    assert_eq!(p0, 256, "Expected bucket midpoint 256 (2^8)");
}

#[test]
fn test_edge_case_all_same_value() {
    let histogram = LatencyHistogramCapsule::new();

    // All 1000 samples at same latency
    for _ in 0..1000 {
        histogram.record(512);
    }

    let p0 = histogram.percentile_scalar(0.0);
    let p50 = histogram.percentile_scalar(50.0);
    let p99 = histogram.percentile_scalar(99.0);
    let p100 = histogram.percentile_scalar(100.0);

    // All percentiles should return same bucket
    assert_eq!(p0, 512, "p0 bucket mismatch");
    assert_eq!(p50, 512, "p50 bucket mismatch");
    assert_eq!(p99, 512, "p99 bucket mismatch");
    assert_eq!(p100, 512, "p100 bucket mismatch");
}

#[test]
fn test_transparent_api_scalar() {
    let histogram = LatencyHistogramCapsule::new();

    for i in 1..=1000 {
        histogram.record(i);
    }

    let p50_optimized = histogram.percentile_optimized(50.0);
    let p50_scalar = histogram.percentile_scalar(50.0);

    // Without portable_simd, optimized should use scalar
    #[cfg(not(feature = "portable_simd"))]
    assert_eq!(p50_optimized, p50_scalar, "Optimized API should use scalar");

    // With portable_simd, optimized should use SIMD (same result as scalar)
    #[cfg(feature = "portable_simd")]
    {
        let p50_simd = histogram.percentile_simd(50.0);
        assert_eq!(p50_optimized, p50_simd, "Optimized API should use SIMD");
        assert_eq!(p50_simd, p50_scalar, "SIMD/scalar equivalence");
    }
}

#[test]
fn test_percentile_boundary_values() {
    let histogram = LatencyHistogramCapsule::new();

    for i in 0..1000 {
        histogram.record(i);
    }

    // Test boundary percentiles
    let p0 = histogram.percentile_scalar(0.0);
    let p100 = histogram.percentile_scalar(100.0);

    assert!(p0 > 0, "p0 should be > 0 for non-empty histogram");
    assert!(p100 > p0, "p100 should be > p0");
}

// ============================================================================
// T28 Q8-Q14: PROPERTY TESTS
// ============================================================================

#[test]
fn test_property_monotonicity() {
    let histogram = LatencyHistogramCapsule::new();

    // Random latencies
    for i in 0..10000 {
        histogram.record((i * 137) % 10000); // Pseudo-random sequence
    }

    // Monotonicity property: p_i <= p_j for i < j
    let percentiles = [0.0, 10.0, 25.0, 50.0, 75.0, 90.0, 95.0, 99.0, 99.9, 100.0];
    let mut results = Vec::new();

    for &p in &percentiles {
        results.push(histogram.percentile_scalar(p));
    }

    // Verify monotonic property
    for i in 0..results.len() - 1 {
        assert!(
            results[i] <= results[i + 1],
            "Monotonicity violation: p{}={} > p{}={}",
            percentiles[i], results[i], percentiles[i + 1], results[i + 1]
        );
    }
}

#[cfg(feature = "portable_simd")]
#[test]
fn test_property_simd_scalar_equivalence_comprehensive() {
    let histogram = LatencyHistogramCapsule::new();

    // Complex distribution
    for i in 0..5000 {
        histogram.record(i);
    }
    for i in 0..3000 {
        histogram.record(i * 10);
    }
    for i in 0..2000 {
        histogram.record(i * 100);
    }

    // Test all percentiles from 0 to 100 in steps of 5
    for p in (0..=100).step_by(5) {
        let p_f64 = p as f64;
        let simd_result = histogram.percentile_simd(p_f64);
        let scalar_result = histogram.percentile_scalar(p_f64);

        assert_eq!(
            simd_result, scalar_result,
            "Equivalence violation at p{}: SIMD={}, scalar={}",
            p, simd_result, scalar_result
        );
    }
}

#[cfg(feature = "portable_simd")]
#[test]
fn test_property_batch_percentiles_correctness() {
    let histogram = LatencyHistogramCapsule::new();

    for i in 0..10000 {
        histogram.record(i);
    }

    let percentiles = vec![10.0, 25.0, 50.0, 75.0, 90.0, 95.0, 99.0, 99.9];
    let batch_results = histogram.batch_percentiles(&percentiles);

    // Verify each batch result matches individual SIMD call
    for (i, &p) in percentiles.iter().enumerate() {
        let individual_result = histogram.percentile_simd(p);
        assert_eq!(
            batch_results[i], individual_result,
            "Batch result mismatch at p{}: batch={}, individual={}",
            p, batch_results[i], individual_result
        );
    }

    // Verify monotonicity in batch results
    for i in 0..batch_results.len() - 1 {
        assert!(
            batch_results[i] <= batch_results[i + 1],
            "Batch monotonicity violation: p{}={} > p{}={}",
            percentiles[i], batch_results[i], percentiles[i + 1], batch_results[i + 1]
        );
    }
}

#[test]
fn test_property_large_histogram_stress() {
    let histogram = LatencyHistogramCapsule::new();

    // Stress test: 100K samples
    for i in 0..100_000 {
        histogram.record((i * 73) % 100_000); // Pseudo-random
    }

    let p50 = histogram.percentile_scalar(50.0);
    let p99 = histogram.percentile_scalar(99.0);
    let p999 = histogram.percentile_scalar(99.9);

    // Verify reasonable results
    assert!(p50 > 0, "p50 should be > 0");
    assert!(p99 > p50, "p99 should be > p50");
    assert!(p999 > p99, "p999 should be > p99");

    // Verify count
    assert_eq!(histogram.count(), 100_000, "Sample count mismatch");
}

// ============================================================================
// T28 Q15-Q21: INTEGRATION TESTS
// ============================================================================

#[test]
fn test_integration_end_to_end_profiling() {
    use clapi_core::profiling::{LatencyProfiler, ComponentType};

    let profiler = LatencyProfiler::new();

    // Simulate HTTP request profiling
    for i in 0..1000 {
        profiler.record(ComponentType::HttpRequest, i * 100);
    }

    // Simulate budget validation profiling
    for i in 0..500 {
        profiler.record(ComponentType::BudgetValidation, i * 50);
    }

    // Query stats
    let http_stats = profiler.stats(ComponentType::HttpRequest);
    let budget_stats = profiler.stats(ComponentType::BudgetValidation);

    assert_eq!(http_stats.count, 1000, "HTTP request count mismatch");
    assert_eq!(budget_stats.count, 500, "Budget validation count mismatch");

    assert!(http_stats.p99 > 0, "HTTP p99 should be > 0");
    assert!(budget_stats.p99 > 0, "Budget p99 should be > 0");
}

#[test]
fn test_integration_multi_component_profiling() {
    use clapi_core::profiling::{LatencyProfiler, ComponentType};

    let profiler = LatencyProfiler::new();

    // Profile multiple components concurrently
    let components = [
        ComponentType::HttpRequest,
        ComponentType::BudgetValidation,
        ComponentType::ProviderRouting,
        ComponentType::CircuitBreaker,
        ComponentType::AuditLog,
    ];

    for component in &components {
        for i in 0..100 {
            profiler.record(*component, i * 10);
        }
    }

    // Verify all components have stats
    let all_stats = profiler.all_stats();
    assert_eq!(all_stats.len(), 8, "Should have 8 component types");

    // Verify specific components
    for component in &components {
        let stats = profiler.stats(*component);
        assert_eq!(stats.count, 100, "Component {:?} count mismatch", component);
        assert!(stats.p50 > 0, "Component {:?} p50 should be > 0", component);
        assert!(stats.p99 > 0, "Component {:?} p99 should be > 0", component);
    }
}

// ============================================================================
// ADDITIONAL SIMD-SPECIFIC TESTS
// ============================================================================

#[cfg(feature = "portable_simd")]
#[test]
fn test_simd_batch_percentiles_empty() {
    let histogram = LatencyHistogramCapsule::new();
    let percentiles = vec![50.0, 99.0, 99.9];
    let results = histogram.batch_percentiles(&percentiles);

    assert_eq!(results.len(), 3);
    assert_eq!(results[0], 0, "Empty histogram should return 0");
    assert_eq!(results[1], 0, "Empty histogram should return 0");
    assert_eq!(results[2], 0, "Empty histogram should return 0");
}

#[cfg(feature = "portable_simd")]
#[test]
fn test_simd_batch_percentiles_single_percentile() {
    let histogram = LatencyHistogramCapsule::new();

    for i in 0..1000 {
        histogram.record(i * 10);
    }

    // Single percentile batch should match individual call
    let batch = histogram.batch_percentiles(&[99.0]);
    let individual = histogram.percentile_simd(99.0);

    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0], individual, "Single batch percentile mismatch");
}

#[cfg(feature = "portable_simd")]
#[test]
fn test_simd_stats_snapshot() {
    let histogram = LatencyHistogramCapsule::new();

    for i in 1..=1000 {
        histogram.record(i);
    }

    let stats_simd = histogram.stats_simd();
    let stats_original = histogram.stats();

    // SIMD stats should match original stats
    assert_eq!(stats_simd.count, stats_original.count, "Count mismatch");
    assert_eq!(stats_simd.min, stats_original.min, "Min mismatch");
    assert_eq!(stats_simd.max, stats_original.max, "Max mismatch");
    assert_eq!(stats_simd.mean, stats_original.mean, "Mean mismatch");
    assert_eq!(stats_simd.p50, stats_original.p50, "p50 mismatch");
    assert_eq!(stats_simd.p99, stats_original.p99, "p99 mismatch");
    assert_eq!(stats_simd.p999, stats_original.p999, "p999 mismatch");
}

#[cfg(feature = "portable_simd")]
#[test]
fn test_simd_percentile_chunk_boundaries() {
    let histogram = LatencyHistogramCapsule::new();

    // Test SIMD chunk boundary behavior (8-bucket chunks)
    // Buckets 0-7: first chunk
    // Buckets 8-15: second chunk
    // etc.

    // Fill buckets strategically to test chunk boundaries
    for bucket_idx in [0, 7, 8, 15, 16, 23, 24, 31, 32, 39, 40, 47, 48, 49] {
        let latency = if bucket_idx == 0 { 1 } else { 1u64 << bucket_idx };
        histogram.record(latency);
    }

    // Query percentiles that fall on chunk boundaries
    let p50 = histogram.percentile_simd(50.0);
    let p99 = histogram.percentile_simd(99.0);

    assert!(p50 > 0, "p50 should be > 0");
    assert!(p99 > p50, "p99 should be > p50");
}
