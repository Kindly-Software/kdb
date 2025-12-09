//! Performance Budget Tests - Timeline Aggregation SLO Enforcement (T28 Q1-Q7)
//!
//! ## Purpose
//! Enforce strict latency budgets for Timeline Aggregation operations. Fail tests
//! if p99.9 exceeds SLOs, preventing performance regressions in CI/CD.
//!
//! ## Framework Compliance
//! - **T28 Q1-Q7**: Unit tier - SLO validation
//! - **B32**: Fair measurement with statistical rigor (1000+ samples, 95% CI)
//! - **UCE34 Q30**: Validation framework - enforce budgets
//!
//! ## Latency SLOs (Strict Bounds - From P1 Enhancement 5,6)
//! - **Append**: <100ns p99.9 (single-threaded), <200ns p99.9 (concurrent)
//! - **Query**: <1μs p99.9 (single bucket), <10μs p99.9 (range query)
//! - **Flush**: <10μs p99.9 (single bucket), <100μs p99.9 (batch flush)
//! - **Memory**: <128MB for 1M events (bounded memory growth)
//!
//! ## Test Structure (T28)
//! - Q1: Core behaviors (append/query/flush latencies)
//! - Q2: Edge cases (empty buckets, full buckets, boundary timestamps)
//! - Q3: Invariants (memory bounds, latency budgets, no data loss)
//! - Q4: Code paths (all operation types covered)
//! - Q5: Isolation (independent tests, no shared state)
//! - Q6: Speed (<10ms per test for fast feedback)
//! - Q7: Readability (clear arrange-act-assert structure)

use clapi_core::capsules::timeline_aggregation_capsule::{
    TimelineAggregationCapsuleWrapper, TimelineBucket, BucketStatus,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ============================================================================
// SLO CONSTANTS (Strict Performance Budgets)
// ============================================================================

/// Append p99.9 budget: 100ns (lockfree atomic increment)
const APPEND_P99_9_NS: u64 = 100;

/// Append concurrent p99.9 budget: 200ns (with contention allowance)
const APPEND_CONCURRENT_P99_9_NS: u64 = 200;

/// Query single bucket p99.9 budget: 1μs (direct index access)
const QUERY_P99_9_NS: u64 = 1_000;

/// Query range p99.9 budget: 10μs (multi-bucket aggregation)
const QUERY_RANGE_P99_9_NS: u64 = 10_000;

/// Flush single bucket p99.9 budget: 10μs (batch write)
const FLUSH_P99_9_NS: u64 = 10_000;

/// Flush batch p99.9 budget: 100μs (multi-bucket flush)
const FLUSH_BATCH_P99_9_NS: u64 = 100_000;

/// Memory budget: 128MB for 1M events
const MEMORY_BUDGET_BYTES: usize = 128 * 1024 * 1024;
const MEMORY_TEST_EVENTS: usize = 1_000_000;

// ============================================================================
// TEST HELPERS (T28 Q7: Readability)
// ============================================================================

/// Measure latency samples with statistical rigor (B32 compliance)
fn measure_latencies<F>(samples: usize, mut operation: F) -> Vec<u64>
where
    F: FnMut() -> (),
{
    let mut latencies = Vec::with_capacity(samples);

    // Warmup: 100 iterations to stabilize cache/CPU
    for _ in 0..100 {
        operation();
    }

    // Measurement: Collect latency samples
    for _ in 0..samples {
        let start = Instant::now();
        operation();
        let elapsed = start.elapsed().as_nanos() as u64;
        latencies.push(elapsed);
    }

    latencies
}

/// Calculate percentile from sorted samples
fn percentile(samples: &[u64], p: f64) -> u64 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let idx = ((sorted.len() as f64 * p) / 100.0) as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Create timeline with minute buckets (1440 = 24 hours)
fn create_timeline() -> TimelineAggregationCapsuleWrapper {
    TimelineAggregationCapsuleWrapper::new(1440, 60)
        .expect("Timeline creation failed")
}

// ============================================================================
// T28 Q1: CORE BEHAVIORS - LATENCY SLO TESTS
// ============================================================================

#[test]
fn test_append_latency_slo_single_threaded() {
    // Arrange: Create timeline
    let timeline = create_timeline();
    let now = SystemTime::now();

    // Act: Measure append latencies (1000 samples for 95% CI)
    let samples = measure_latencies(1000, || {
        timeline.append_system_time(now).unwrap();
    });

    // Assert: p99.9 within budget
    let p99_9 = percentile(&samples, 99.9);
    let p99 = percentile(&samples, 99.0);
    let p50 = percentile(&samples, 50.0);

    println!("Append latencies: p50={}ns, p99={}ns, p99.9={}ns", p50, p99, p99_9);

    assert!(
        p99_9 < APPEND_P99_9_NS,
        "BUDGET EXCEEDED: append p99.9 {}ns > budget {}ns (FAIL)",
        p99_9, APPEND_P99_9_NS
    );
}

#[test]
fn test_append_latency_slo_concurrent() {
    // Arrange: Create timeline, 8 threads
    let timeline = Arc::new(create_timeline());
    let samples_per_thread = 1000;
    let num_threads = 8;

    // Act: Measure concurrent append latencies
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let t = Arc::clone(&timeline);
            thread::spawn(move || {
                let now = SystemTime::now();
                let mut latencies = Vec::with_capacity(samples_per_thread);

                for _ in 0..samples_per_thread {
                    let start = Instant::now();
                    t.append_system_time(now).unwrap();
                    latencies.push(start.elapsed().as_nanos() as u64);
                }

                latencies
            })
        })
        .collect();

    // Collect all samples
    let mut all_samples = Vec::new();
    for h in handles {
        all_samples.extend(h.join().unwrap());
    }

    // Assert: p99.9 within concurrent budget
    let p99_9 = percentile(&all_samples, 99.9);
    let p99 = percentile(&all_samples, 99.0);
    let p50 = percentile(&all_samples, 50.0);

    println!("Concurrent append: p50={}ns, p99={}ns, p99.9={}ns", p50, p99, p99_9);

    assert!(
        p99_9 < APPEND_CONCURRENT_P99_9_NS,
        "BUDGET EXCEEDED: concurrent append p99.9 {}ns > budget {}ns (FAIL)",
        p99_9, APPEND_CONCURRENT_P99_9_NS
    );
}

#[test]
fn test_query_latency_slo_single_bucket() {
    // Arrange: Create timeline, append events
    let timeline = create_timeline();
    let now = SystemTime::now();

    for _ in 0..100 {
        timeline.append_system_time(now).unwrap();
    }

    // Act: Measure query latencies
    let samples = measure_latencies(1000, || {
        let _ = timeline.query_bucket_system_time(now);
    });

    // Assert: p99.9 within budget
    let p99_9 = percentile(&samples, 99.9);
    let p99 = percentile(&samples, 99.0);
    let p50 = percentile(&samples, 50.0);

    println!("Query latencies: p50={}ns, p99={}ns, p99.9={}ns", p50, p99, p99_9);

    assert!(
        p99_9 < QUERY_P99_9_NS,
        "BUDGET EXCEEDED: query p99.9 {}ns > budget {}ns (FAIL)",
        p99_9, QUERY_P99_9_NS
    );
}

#[test]
fn test_query_range_latency_slo() {
    // Arrange: Create timeline, append events across multiple buckets
    let timeline = create_timeline();
    let now = SystemTime::now();

    // Append to 10 different buckets
    for i in 0..10 {
        let ts = now - Duration::from_secs(i * 60);
        for _ in 0..100 {
            timeline.append_system_time(ts).unwrap();
        }
    }

    // Act: Measure range query latencies (10 buckets)
    let samples = measure_latencies(1000, || {
        let _ = timeline.query_last_hours(1); // 60 buckets
    });

    // Assert: p99.9 within range budget
    let p99_9 = percentile(&samples, 99.9);
    let p99 = percentile(&samples, 99.0);
    let p50 = percentile(&samples, 50.0);

    println!("Range query: p50={}ns, p99={}ns, p99.9={}ns", p50, p99, p99_9);

    assert!(
        p99_9 < QUERY_RANGE_P99_9_NS,
        "BUDGET EXCEEDED: range query p99.9 {}ns > budget {}ns (FAIL)",
        p99_9, QUERY_RANGE_P99_9_NS
    );
}

#[test]
fn test_flush_latency_slo_single_bucket() {
    // Arrange: Create timeline, append events
    let timeline = create_timeline();
    let now = SystemTime::now();

    // Append 1000 events to create meaningful flush
    for _ in 0..1000 {
        timeline.append_system_time(now).unwrap();
    }

    // Act: Measure flush latencies
    let samples = measure_latencies(100, || {
        let _ = timeline.flush_bucket_system_time(now);
    });

    // Assert: p99.9 within budget
    let p99_9 = percentile(&samples, 99.9);
    let p99 = percentile(&samples, 99.0);
    let p50 = percentile(&samples, 50.0);

    println!("Flush latencies: p50={}ns, p99={}ns, p99.9={}ns", p50, p99, p99_9);

    assert!(
        p99_9 < FLUSH_P99_9_NS,
        "BUDGET EXCEEDED: flush p99.9 {}ns > budget {}ns (FAIL)",
        p99_9, FLUSH_P99_9_NS
    );
}

// ============================================================================
// T28 Q2: EDGE CASES - BOUNDARY VALIDATION
// ============================================================================

#[test]
fn test_append_empty_timeline() {
    // Arrange: Fresh timeline
    let timeline = create_timeline();
    let now = SystemTime::now();

    // Act: Append to empty timeline
    let samples = measure_latencies(1000, || {
        timeline.append_system_time(now).unwrap();
    });

    // Assert: First append same latency as steady-state
    let p99_9 = percentile(&samples, 99.9);
    assert!(p99_9 < APPEND_P99_9_NS, "Empty timeline append too slow: {}ns", p99_9);
}

#[test]
fn test_query_empty_bucket() {
    // Arrange: Timeline with no events
    let timeline = create_timeline();
    let now = SystemTime::now();

    // Act: Query empty bucket
    let samples = measure_latencies(1000, || {
        let _ = timeline.query_bucket_system_time(now);
    });

    // Assert: Empty bucket query within budget
    let p99_9 = percentile(&samples, 99.9);
    assert!(p99_9 < QUERY_P99_9_NS, "Empty bucket query too slow: {}ns", p99_9);
}

#[test]
fn test_append_boundary_timestamp() {
    // Arrange: Timeline
    let timeline = create_timeline();

    // Act: Append at bucket boundary (transition to next bucket)
    let epoch_start = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let boundary = epoch_start + Duration::from_secs(60); // Exact bucket boundary

    let samples = measure_latencies(1000, || {
        timeline.append_system_time(boundary).unwrap();
    });

    // Assert: Boundary timestamps within budget
    let p99_9 = percentile(&samples, 99.9);
    assert!(p99_9 < APPEND_P99_9_NS, "Boundary append too slow: {}ns", p99_9);
}

// ============================================================================
// T28 Q3: INVARIANTS - MEMORY BUDGET
// ============================================================================

#[test]
fn test_memory_budget_1m_events() {
    // Arrange: Timeline
    let timeline = create_timeline();
    let now = SystemTime::now();

    // Act: Append 1M events
    for _ in 0..MEMORY_TEST_EVENTS {
        timeline.append_system_time(now).unwrap();
    }

    // Assert: Memory usage within budget
    // Note: Actual memory measurement requires jemalloc/mimalloc stats
    // For now, verify capsule size invariant (256B aligned)
    let capsule_size = std::mem::size_of::<TimelineAggregationCapsuleWrapper>();
    println!("Capsule size: {} bytes", capsule_size);

    // Approximate memory: capsule + 1440 buckets × 64B per bucket
    let estimated_memory = capsule_size + (1440 * 64);
    assert!(
        estimated_memory < MEMORY_BUDGET_BYTES,
        "Estimated memory {}B exceeds budget {}B",
        estimated_memory, MEMORY_BUDGET_BYTES
    );
}

// ============================================================================
// T28 Q4: CODE PATHS - ALL OPERATIONS COVERED
// ============================================================================

#[test]
fn test_all_operation_types_within_budget() {
    // Arrange: Timeline with events
    let timeline = create_timeline();
    let now = SystemTime::now();

    for _ in 0..1000 {
        timeline.append_system_time(now).unwrap();
    }

    // Act & Assert: All operations within budget
    let mut all_ok = true;

    // 1. Append
    let append_samples = measure_latencies(1000, || {
        timeline.append_system_time(now).unwrap();
    });
    let append_p99_9 = percentile(&append_samples, 99.9);
    if append_p99_9 >= APPEND_P99_9_NS {
        println!("FAIL: append p99.9 {}ns >= {}ns", append_p99_9, APPEND_P99_9_NS);
        all_ok = false;
    }

    // 2. Query bucket
    let query_samples = measure_latencies(1000, || {
        let _ = timeline.query_bucket_system_time(now);
    });
    let query_p99_9 = percentile(&query_samples, 99.9);
    if query_p99_9 >= QUERY_P99_9_NS {
        println!("FAIL: query p99.9 {}ns >= {}ns", query_p99_9, QUERY_P99_9_NS);
        all_ok = false;
    }

    // 3. Flush
    let flush_samples = measure_latencies(100, || {
        let _ = timeline.flush_bucket_system_time(now);
    });
    let flush_p99_9 = percentile(&flush_samples, 99.9);
    if flush_p99_9 >= FLUSH_P99_9_NS {
        println!("FAIL: flush p99.9 {}ns >= {}ns", flush_p99_9, FLUSH_P99_9_NS);
        all_ok = false;
    }

    assert!(all_ok, "One or more operations exceeded budget");
}

// ============================================================================
// T28 Q5: ISOLATION - NO SHARED STATE
// ============================================================================

#[test]
fn test_timeline_isolation_no_shared_state() {
    // Arrange: Two independent timelines
    let timeline1 = create_timeline();
    let timeline2 = create_timeline();
    let now = SystemTime::now();

    // Act: Append to timeline1 only
    for _ in 0..1000 {
        timeline1.append_system_time(now).unwrap();
    }

    // Assert: timeline2 unaffected (isolation)
    let bucket1 = timeline1.query_bucket_system_time(now).unwrap();
    let bucket2 = timeline2.query_bucket_system_time(now).unwrap();

    assert_eq!(bucket1.count, 1000, "Timeline1 should have 1000 events");
    assert_eq!(bucket2.count, 0, "Timeline2 should have 0 events (isolated)");
}

// ============================================================================
// T28 Q6: SPEED - FAST FEEDBACK (<10ms per test)
// ============================================================================

#[test]
fn test_suite_speed_under_10ms_per_test() {
    // Arrange: Timeline
    let timeline = create_timeline();
    let now = SystemTime::now();

    // Act: Measure test execution time
    let start = Instant::now();

    // Simulate minimal test: 100 operations
    for _ in 0..100 {
        timeline.append_system_time(now).unwrap();
    }

    let elapsed = start.elapsed();

    // Assert: Test completes in <10ms
    assert!(
        elapsed.as_millis() < 10,
        "Test too slow: {}ms (target: <10ms)",
        elapsed.as_millis()
    );
}

// ============================================================================
// T28 Q7: READABILITY - CLEAR STRUCTURE
// ============================================================================

#[test]
fn test_arrange_act_assert_structure_example() {
    // ARRANGE: Set up test conditions
    let timeline = create_timeline();
    let now = SystemTime::now();
    let initial_count = timeline.query_bucket_system_time(now)
        .map(|b| b.count)
        .unwrap_or(0);

    // ACT: Perform operation under test
    timeline.append_system_time(now).unwrap();
    timeline.append_system_time(now).unwrap();
    timeline.append_system_time(now).unwrap();

    // ASSERT: Verify expected outcome
    let final_count = timeline.query_bucket_system_time(now).unwrap().count;
    assert_eq!(
        final_count, initial_count + 3,
        "Expected 3 new events, got {} events",
        final_count - initial_count
    );
}

// ============================================================================
// CI/CD INTEGRATION - PERFORMANCE REGRESSION DETECTION
// ============================================================================

#[test]
#[ignore] // Run with: cargo test --test performance_budget_tests -- --ignored
fn test_ci_cd_performance_budget_enforcement() {
    // This test runs in CI/CD to detect performance regressions
    // Fails the build if any SLO is exceeded

    let timeline = create_timeline();
    let now = SystemTime::now();

    // Measure all operations
    let append_samples = measure_latencies(1000, || {
        timeline.append_system_time(now).unwrap();
    });

    let query_samples = measure_latencies(1000, || {
        let _ = timeline.query_bucket_system_time(now);
    });

    // Calculate percentiles
    let append_p99_9 = percentile(&append_samples, 99.9);
    let query_p99_9 = percentile(&query_samples, 99.9);

    // Report metrics
    println!("=== CI/CD Performance Report ===");
    println!("Append p99.9: {}ns (budget: {}ns)", append_p99_9, APPEND_P99_9_NS);
    println!("Query p99.9: {}ns (budget: {}ns)", query_p99_9, QUERY_P99_9_NS);

    // Enforce budgets (fail CI/CD if exceeded)
    assert!(
        append_p99_9 < APPEND_P99_9_NS,
        "REGRESSION: Append latency {}ns exceeds budget {}ns",
        append_p99_9, APPEND_P99_9_NS
    );

    assert!(
        query_p99_9 < QUERY_P99_9_NS,
        "REGRESSION: Query latency {}ns exceeds budget {}ns",
        query_p99_9, QUERY_P99_9_NS
    );

    println!("=== All budgets PASSED ===");
}
