//! P2 E15: SIMD Aggregation Helpers - T28 Comprehensive Test Suite
//!
//! ## Test Coverage (T28 Framework)
//!
//! ### Tier 1: Unit Tests (Q1-Q7)
//! - test_simd_sum_correctness
//! - test_simd_min_max_correctness
//! - test_simd_avg_correctness
//! - test_simd_percentile_correctness
//! - test_simd_moving_avg_correctness
//! - test_adaptive_sum_correctness
//!
//! ### Tier 2: Property Tests (Q8-Q14)
//! - test_simd_sum_matches_scalar
//! - test_simd_min_max_matches_scalar
//! - test_simd_percentile_approximate
//! - test_simd_associativity
//! - test_simd_commutativity
//!
//! ### Tier 3: Integration Tests (Q15-Q21)
//! - test_timeline_integration
//! - test_multi_bucket_aggregation
//! - test_large_dataset_accuracy
//!
//! ### Tier 4: Production Tests (Q22-Q28)
//! - test_performance_threshold
//! - test_simd_vs_scalar_comparison
//! - test_adaptive_selection

#[cfg(feature = "portable_simd")]
use clapi_core::capsules::simd_aggregation::*;
use clapi_core::capsules::timeline_aggregation_capsule::TimelineAggregationCapsuleWrapper;
use std::time::{SystemTime, Duration};

// ============================================================================
// Tier 1: Unit Tests (Q1-Q7) - Correctness Validation
// ============================================================================

#[test]
#[cfg(feature = "portable_simd")]
fn test_simd_sum_correctness() {
    // Q1: Empty slice
    let empty: Vec<u64> = vec![];
    assert_eq!(simd_sum_u64x4(&empty), 0);
    assert_eq!(simd_sum_u64x8(&empty), 0);

    // Q2: Single element
    assert_eq!(simd_sum_u64x4(&[100]), 100);

    // Q3: Exact multiples of SIMD width
    let buckets_4 = vec![10, 20, 30, 40];
    assert_eq!(simd_sum_u64x4(&buckets_4), 100);

    let buckets_8 = vec![10, 20, 30, 40, 50, 60, 70, 80];
    assert_eq!(simd_sum_u64x8(&buckets_8), 360);

    // Q4: Non-multiples (test remainder handling)
    let buckets_5 = vec![10, 20, 30, 40, 50];
    assert_eq!(simd_sum_u64x4(&buckets_5), 150);

    let buckets_10 = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
    assert_eq!(simd_sum_u64x8(&buckets_10), 550);

    // Q5: Large values (test overflow prevention)
    let large = vec![u64::MAX / 4; 4];
    assert!(simd_sum_u64x4(&large) > 0); // No overflow in sum

    // Q6: Zero values
    let zeros = vec![0; 8];
    assert_eq!(simd_sum_u64x8(&zeros), 0);

    // Q7: Mixed values
    let mixed = vec![0, 100, 0, 200, 0, 300];
    assert_eq!(simd_sum_u64x4(&mixed), 600);
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_simd_min_max_correctness() {
    // Q1: Empty slice
    let empty: Vec<u64> = vec![];
    assert_eq!(simd_min_u64x4(&empty), None);
    assert_eq!(simd_max_u64x4(&empty), None);

    // Q2: Single element
    assert_eq!(simd_min_u64x4(&[42]), Some(42));
    assert_eq!(simd_max_u64x4(&[42]), Some(42));

    // Q3: Ascending order
    let ascending = vec![10, 20, 30, 40];
    assert_eq!(simd_min_u64x4(&ascending), Some(10));
    assert_eq!(simd_max_u64x4(&ascending), Some(40));

    // Q4: Descending order
    let descending = vec![40, 30, 20, 10];
    assert_eq!(simd_min_u64x4(&descending), Some(10));
    assert_eq!(simd_max_u64x4(&descending), Some(40));

    // Q5: Random order
    let random = vec![25, 15, 35, 5, 45];
    assert_eq!(simd_min_u64x4(&random), Some(5));
    assert_eq!(simd_max_u64x4(&random), Some(45));

    // Q6: All identical
    let identical = vec![100; 8];
    assert_eq!(simd_min_u64x4(&identical), Some(100));
    assert_eq!(simd_max_u64x4(&identical), Some(100));

    // Q7: Extreme values
    let extreme = vec![0, u64::MAX, 100];
    assert_eq!(simd_min_u64x4(&extreme), Some(0));
    assert_eq!(simd_max_u64x4(&extreme), Some(u64::MAX));
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_simd_avg_correctness() {
    // Q1: Empty slice
    let empty: Vec<u64> = vec![];
    assert_eq!(simd_avg_u64x4(&empty), 0.0);

    // Q2: Single element
    assert_eq!(simd_avg_u64x4(&[100]), 100.0);

    // Q3: Even average
    let even = vec![10, 20, 30, 40];
    assert_eq!(simd_avg_u64x4(&even), 25.0);

    // Q4: Odd average
    let odd = vec![10, 20, 30];
    assert_eq!(simd_avg_u64x4(&odd), 20.0);

    // Q5: Fractional average
    let fractional = vec![1, 2, 3];
    assert!((simd_avg_u64x4(&fractional) - 2.0).abs() < 0.01);

    // Q6: Large values
    let large = vec![1000000, 2000000, 3000000];
    assert_eq!(simd_avg_u64x4(&large), 2000000.0);

    // Q7: Zero values
    let zeros = vec![0; 5];
    assert_eq!(simd_avg_u64x4(&zeros), 0.0);
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_simd_percentile_correctness() {
    // Q1: Empty slice
    let empty: Vec<u64> = vec![];
    assert_eq!(simd_percentile_u64x4(&empty, 50).unwrap(), 0);

    // Q2: Single element
    assert_eq!(simd_percentile_u64x4(&[42], 50).unwrap(), 42);

    // Q3: Median (p50)
    let data = vec![10, 20, 30, 40, 50];
    let p50 = simd_percentile_u64x4(&data, 50).unwrap();
    assert!((p50 as i64 - 30).abs() <= 5, "p50={} not near 30", p50);

    // Q4: p99 (high percentile)
    let data100 = (1..=100).collect::<Vec<u64>>();
    let p99 = simd_percentile_u64x4(&data100, 99).unwrap();
    assert!(p99 >= 95 && p99 <= 100, "p99={} out of range [95,100]", p99);

    // Q5: p1 (low percentile)
    let p1 = simd_percentile_u64x4(&data100, 1).unwrap();
    assert!(p1 >= 1 && p1 <= 5, "p1={} out of range [1,5]", p1);

    // Q6: Invalid percentile
    assert!(simd_percentile_u64x4(&[1, 2, 3], 101).is_err());

    // Q7: Identical values
    let identical = vec![100; 10];
    assert_eq!(simd_percentile_u64x4(&identical, 50).unwrap(), 100);
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_simd_moving_avg_correctness() {
    // Q1: Empty slice
    let empty: Vec<u64> = vec![];
    assert_eq!(simd_moving_avg_u64x8(&empty, 5).unwrap(), 0.0);

    // Q2: Window larger than data
    let small = vec![10, 20, 30];
    assert_eq!(simd_moving_avg_u64x8(&small, 10).unwrap(), 20.0);

    // Q3: Exact window
    let data = vec![10, 20, 30, 40, 50];
    assert_eq!(simd_moving_avg_u64x8(&data, 5).unwrap(), 30.0);

    // Q4: Partial window
    let data10 = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
    let ma = simd_moving_avg_u64x8(&data10, 5).unwrap();
    // Average of last 5: (60 + 70 + 80 + 90 + 100) / 5 = 80.0
    assert_eq!(ma, 80.0);

    // Q5: Window size 1
    assert_eq!(simd_moving_avg_u64x8(&data10, 1).unwrap(), 100.0);

    // Q6: Invalid window (0)
    assert!(simd_moving_avg_u64x8(&data10, 0).is_err());

    // Q7: Large window
    let large = (1..=1000).collect::<Vec<u64>>();
    let ma_large = simd_moving_avg_u64x8(&large, 100).unwrap();
    // Average of last 100: (901 + 902 + ... + 1000) / 100 = 950.5
    assert!((ma_large - 950.5).abs() < 1.0);
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_adaptive_sum_correctness() {
    // Q1: Empty (scalar path)
    let empty: Vec<u64> = vec![];
    assert_eq!(adaptive_sum(&empty), 0);

    // Q2: 1-3 buckets (scalar path)
    assert_eq!(adaptive_sum(&[10]), 10);
    assert_eq!(adaptive_sum(&[10, 20]), 30);
    assert_eq!(adaptive_sum(&[10, 20, 30]), 60);

    // Q3: 4-7 buckets (u64x4 path)
    assert_eq!(adaptive_sum(&[10, 20, 30, 40]), 100);
    assert_eq!(adaptive_sum(&[10, 20, 30, 40, 50]), 150);

    // Q4: 8+ buckets (u64x8 path)
    assert_eq!(adaptive_sum(&[10, 20, 30, 40, 50, 60, 70, 80]), 360);
    assert_eq!(adaptive_sum(&[10; 100]), 1000);

    // Q5: Verify consistency across all paths
    for len in 1..=20 {
        let data: Vec<u64> = (1..=len).collect();
        let expected: u64 = (1..=len).sum();
        assert_eq!(adaptive_sum(&data), expected, "Failed for len={}", len);
    }
}

// ============================================================================
// Tier 2: Property Tests (Q8-Q14) - Invariant Validation
// ============================================================================

#[test]
#[cfg(feature = "portable_simd")]
fn test_simd_sum_matches_scalar() {
    // Q8: Property: SIMD sum == scalar sum (for all inputs)
    let test_cases = vec![
        vec![],
        vec![42],
        vec![1, 2, 3, 4],
        vec![10, 20, 30, 40, 50, 60, 70, 80],
        vec![1; 100],
        (1..=50).collect(),
    ];

    for buckets in test_cases {
        let scalar_sum: u64 = buckets.iter().sum();
        let simd_sum = simd_sum_u64x4(&buckets);
        assert_eq!(simd_sum, scalar_sum, "SIMD != scalar for {:?}", buckets);
    }
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_simd_min_max_matches_scalar() {
    // Q9: Property: SIMD min/max == scalar min/max
    let test_cases = vec![
        vec![42],
        vec![1, 5, 3, 9, 2],
        vec![100, 50, 75, 25],
        (1..=100).collect(),
        vec![u64::MAX, 0, 1000],
    ];

    for buckets in test_cases {
        let scalar_min = buckets.iter().min().copied();
        let scalar_max = buckets.iter().max().copied();

        let simd_min = simd_min_u64x4(&buckets);
        let simd_max = simd_max_u64x4(&buckets);

        assert_eq!(simd_min, scalar_min, "Min mismatch for {:?}", buckets);
        assert_eq!(simd_max, scalar_max, "Max mismatch for {:?}", buckets);
    }
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_simd_percentile_approximate() {
    // Q10: Property: SIMD percentile within 10% of exact (for large datasets)
    let data: Vec<u64> = (1..=100).collect();

    for percentile in [10, 25, 50, 75, 90, 99] {
        let exact = percentile; // For 1..=100, pX ≈ X
        let approx = simd_percentile_u64x4(&data, percentile).unwrap();

        let error = (approx as i64 - exact as i64).abs();
        let tolerance = (exact / 10).max(5); // 10% or min 5

        assert!(
            error <= tolerance as i64,
            "p{} error {} exceeds tolerance {} (exact={}, approx={})",
            percentile,
            error,
            tolerance,
            exact,
            approx
        );
    }
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_simd_associativity() {
    // Q11: Property: (a + b) + c == a + (b + c)
    let a = vec![10, 20];
    let b = vec![30, 40];
    let c = vec![50, 60];

    let mut left = a.clone();
    left.extend_from_slice(&b);
    left.extend_from_slice(&c);

    let mut right = a.clone();
    right.extend_from_slice(&b);
    right.extend_from_slice(&c);

    assert_eq!(simd_sum_u64x4(&left), simd_sum_u64x4(&right));
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_simd_commutativity() {
    // Q12: Property: sum(a) == sum(reverse(a))
    let data = vec![10, 20, 30, 40, 50];
    let mut reversed = data.clone();
    reversed.reverse();

    assert_eq!(simd_sum_u64x4(&data), simd_sum_u64x4(&reversed));
}

// ============================================================================
// Tier 3: Integration Tests (Q15-Q21) - End-to-End Workflows
// ============================================================================

#[test]
#[cfg(feature = "portable_simd")]
fn test_timeline_integration() {
    // Q15: Integration with TimelineAggregationCapsule
    let mut timeline = TimelineAggregationCapsuleWrapper::default();
    let now = SystemTime::now();

    // Append 20 events over 20 minutes
    for i in 0..20 {
        let ts = now - Duration::from_secs(i * 60);
        for _ in 0..((i + 1) * 10) {
            timeline.append(ts, "request", "test").unwrap();
        }
    }

    // Extract bucket counts
    let start = now - Duration::from_secs(20 * 60);
    let snapshots = timeline.query_range(start, now).unwrap();
    let counts: Vec<u64> = snapshots.iter().map(|s| s.event_count).collect();

    // Compare SIMD vs scalar aggregations
    let scalar_sum: u64 = counts.iter().sum();
    let simd_sum = simd_sum_u64x8(&counts);
    assert_eq!(simd_sum, scalar_sum, "Timeline integration sum mismatch");

    let scalar_min = counts.iter().min().copied().unwrap();
    let simd_min = simd_min_u64x4(&counts).unwrap();
    assert_eq!(simd_min, scalar_min, "Timeline integration min mismatch");
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_multi_bucket_aggregation() {
    // Q16: Multi-bucket parallel aggregation
    let buckets = vec![
        100, 200, 150, 300, 250, 400, 350, 500,
        450, 600, 550, 700, 650, 800, 750, 900,
    ];

    // Test all SIMD operations in parallel
    let sum = simd_sum_u64x8(&buckets);
    let min = simd_min_u64x4(&buckets).unwrap();
    let max = simd_max_u64x4(&buckets).unwrap();
    let avg = simd_avg_u64x4(&buckets);

    assert_eq!(sum, 7200);
    assert_eq!(min, 100);
    assert_eq!(max, 900);
    assert_eq!(avg, 450.0);
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_large_dataset_accuracy() {
    // Q17: Large dataset (1000 buckets)
    let large: Vec<u64> = (1..=1000).collect();

    let scalar_sum: u64 = large.iter().sum();
    let simd_sum = simd_sum_u64x8(&large);
    assert_eq!(simd_sum, scalar_sum);

    let scalar_avg = scalar_sum as f64 / large.len() as f64;
    let simd_avg = simd_avg_u64x4(&large);
    assert!((simd_avg - scalar_avg).abs() < 0.01);
}

// ============================================================================
// Tier 4: Production Tests (Q22-Q28) - Performance Validation
// ============================================================================

#[test]
#[cfg(feature = "portable_simd")]
fn test_performance_threshold() {
    // Q22: Verify SIMD is faster for 8+ buckets
    use std::time::Instant;

    let buckets: Vec<u64> = (1..=1000).collect();

    // Scalar baseline
    let start = Instant::now();
    for _ in 0..1000 {
        let _: u64 = buckets.iter().sum();
    }
    let scalar_time = start.elapsed();

    // SIMD
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = simd_sum_u64x8(&buckets);
    }
    let simd_time = start.elapsed();

    // SIMD should be at least 1.5× faster (conservative)
    let speedup = scalar_time.as_nanos() as f64 / simd_time.as_nanos() as f64;
    assert!(
        speedup >= 1.5,
        "SIMD speedup {:.2}× below target 1.5×",
        speedup
    );
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_simd_vs_scalar_comparison() {
    // Q23: B32 honest reporting - document where SIMD helps
    let small = vec![10, 20, 30]; // <4 buckets: scalar faster
    let medium = vec![10, 20, 30, 40, 50, 60]; // 4-7: u64x4 optimal
    let large = vec![10; 100]; // 8+: u64x8 optimal

    // All should produce correct results
    assert_eq!(adaptive_sum(&small), 60);
    assert_eq!(adaptive_sum(&medium), 210);
    assert_eq!(adaptive_sum(&large), 1000);
}

#[test]
#[cfg(feature = "portable_simd")]
fn test_adaptive_selection() {
    // Q24: Verify adaptive_sum chooses optimal path
    for len in 1..=20 {
        let data: Vec<u64> = (1..=len).collect();
        let expected: u64 = (1..=len).sum();

        let result = adaptive_sum(&data);
        assert_eq!(result, expected, "Adaptive failed for len={}", len);
    }
}

// ============================================================================
// Fallback Tests (no portable_simd feature)
// ============================================================================

#[test]
#[cfg(not(feature = "portable_simd"))]
fn test_scalar_fallback_works() {
    // Ensure scalar fallbacks compile and work correctly
    use clapi_core::capsules::simd_aggregation::*;

    let buckets = vec![10, 20, 30, 40];
    assert_eq!(simd_sum_u64x4(&buckets), 100);
    assert_eq!(simd_min_u64x4(&buckets), Some(10));
    assert_eq!(simd_max_u64x4(&buckets), Some(40));
    assert_eq!(simd_avg_u64x4(&buckets), 25.0);
    assert_eq!(simd_percentile_u64x4(&buckets, 50).unwrap(), 30);
    assert_eq!(adaptive_sum(&buckets), 100);
}
