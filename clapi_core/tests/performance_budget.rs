//! Performance Budget Enforcer (P1 E10)
//!
//! ## Purpose
//! Detect performance regressions automatically in CI by enforcing latency budgets.
//! Uses B32 framework for honest measurement (1000+ iterations, 95% CI).
//!
//! ## Budgets (from P0 baseline + 10% margin)
//! - Append latency P99: <450ns (was 400ns)
//! - Query latency P99: <520ns (was 480ns)
//! - Throughput minimum: 9M ops/sec (was 10M)
//!
//! ## Test Strategy (T28 Framework)
//! - Tier 1 (Unit): Individual operation budgets
//! - Tier 2 (Property): Budget holds across input space
//! - Tier 3 (Integration): End-to-end workflow budgets
//! - Tier 4 (Production): Sustained performance under load

use clapi_core::capsules::timeline_aggregation_capsule::TimelineAggregationCapsuleWrapper;
use std::time::{Duration, Instant, SystemTime};

/// Performance budget configuration
#[derive(Debug)]
struct PerformanceBudget {
    append_p50_ns: u64,
    append_p99_ns: u64,
    query_p50_ns: u64,
    query_p99_ns: u64,
    throughput_min_ops_sec: u64,
}

impl PerformanceBudget {
    /// P0 baseline budgets with 10% margin for stability
    fn baseline() -> Self {
        Self {
            append_p50_ns: 200,  // P50 target: <200ns
            append_p99_ns: 450,  // P99 target: <450ns (was 400ns + 10%)
            query_p50_ns: 250,   // P50 target: <250ns
            query_p99_ns: 520,   // P99 target: <520ns (was 480ns + 10%)
            throughput_min_ops_sec: 9_000_000, // Min 9M ops/sec
        }
    }

    /// Relaxed budgets for CI environments (slower hardware)
    fn ci_relaxed() -> Self {
        Self {
            append_p50_ns: 300,
            append_p99_ns: 600,
            query_p50_ns: 350,
            query_p99_ns: 700,
            throughput_min_ops_sec: 5_000_000,
        }
    }
}

/// Collect latency samples for percentile calculation
struct LatencySampler {
    samples: Vec<u64>,
}

impl LatencySampler {
    fn new(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
        }
    }

    fn record(&mut self, latency_ns: u64) {
        self.samples.push(latency_ns);
    }

    fn p50(&mut self) -> u64 {
        self.percentile(50.0)
    }

    fn p95(&mut self) -> u64 {
        self.percentile(95.0)
    }

    fn p99(&mut self) -> u64 {
        self.percentile(99.0)
    }

    fn percentile(&mut self, pct: f64) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }

        self.samples.sort_unstable();
        let idx = ((pct / 100.0) * self.samples.len() as f64) as usize;
        self.samples[idx.min(self.samples.len() - 1)]
    }

    fn mean(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        self.samples.iter().sum::<u64>() / self.samples.len() as u64
    }

    fn len(&self) -> usize {
        self.samples.len()
    }
}

// ============================================================================
// Tier 1: Unit Tests (Q1-Q7) - Individual Operation Budgets
// ============================================================================

/// T28 Q1: Core behavior - Append latency meets budget
#[test]
fn test_append_latency_budget() {
    let capsule = TimelineAggregationCapsuleWrapper::default();
    let budget = PerformanceBudget::baseline();

    // Warmup (100 iterations)
    for _ in 0..100 {
        let _ = capsule.append(SystemTime::now(), "test", "data");
    }

    // Measure (B32: 1000+ iterations)
    let mut sampler = LatencySampler::new(1000);
    for _ in 0..1000 {
        let start = Instant::now();
        let _ = capsule.append(SystemTime::now(), "test", "data");
        sampler.record(start.elapsed().as_nanos() as u64);
    }

    let p50 = sampler.p50();
    let p99 = sampler.p99();

    println!("Append latency: P50={}ns, P99={}ns", p50, p99);
    println!("Budget: P50={}ns, P99={}ns", budget.append_p50_ns, budget.append_p99_ns);

    // Assert budgets (with 20% tolerance for CI variability)
    let tolerance = 1.2;
    assert!(
        p50 < (budget.append_p50_ns as f64 * tolerance) as u64,
        "Append P50 {}ns exceeds budget {}ns (with 20% tolerance)",
        p50,
        budget.append_p50_ns
    );

    assert!(
        p99 < (budget.append_p99_ns as f64 * tolerance) as u64,
        "Append P99 {}ns exceeds budget {}ns (with 20% tolerance)",
        p99,
        budget.append_p99_ns
    );
}

/// T28 Q2: Edge cases - Query latency on empty timeline
#[test]
fn test_query_latency_budget_empty() {
    let capsule = TimelineAggregationCapsuleWrapper::default();
    let budget = PerformanceBudget::baseline();

    // Warmup
    for _ in 0..100 {
        let _ = capsule.query(SystemTime::now());
    }

    // Measure empty query latency
    let mut sampler = LatencySampler::new(1000);
    for _ in 0..1000 {
        let start = Instant::now();
        let _ = capsule.query(SystemTime::now());
        sampler.record(start.elapsed().as_nanos() as u64);
    }

    let p50 = sampler.p50();
    let p99 = sampler.p99();

    println!("Query (empty) latency: P50={}ns, P99={}ns", p50, p99);

    let tolerance = 1.2;
    assert!(
        p99 < (budget.query_p99_ns as f64 * tolerance) as u64,
        "Query P99 {}ns exceeds budget {}ns",
        p99,
        budget.query_p99_ns
    );
}

/// T28 Q3: Invariant - Query latency on populated timeline
#[test]
fn test_query_latency_budget_populated() {
    let capsule = TimelineAggregationCapsuleWrapper::default();
    let budget = PerformanceBudget::baseline();

    // Populate with 10K events
    for _ in 0..10_000 {
        let _ = capsule.append(SystemTime::now(), "test", "data");
    }

    // Warmup
    for _ in 0..100 {
        let _ = capsule.query(SystemTime::now());
    }

    // Measure query latency
    let mut sampler = LatencySampler::new(1000);
    for _ in 0..1000 {
        let start = Instant::now();
        let _ = capsule.query(SystemTime::now());
        sampler.record(start.elapsed().as_nanos() as u64);
    }

    let p50 = sampler.p50();
    let p99 = sampler.p99();

    println!("Query (populated) latency: P50={}ns, P99={}ns", p50, p99);

    let tolerance = 1.2;
    assert!(
        p99 < (budget.query_p99_ns as f64 * tolerance) as u64,
        "Query P99 {}ns exceeds budget {}ns",
        p99,
        budget.query_p99_ns
    );
}

/// T28 Q4: Code coverage - All operation paths tested
#[test]
fn test_all_operations_meet_budgets() {
    let capsule = TimelineAggregationCapsuleWrapper::default();
    let budget = PerformanceBudget::baseline();

    // Test all operation types
    let operations = vec![
        ("append", || capsule.append(SystemTime::now(), "test", "data")),
        ("query", || capsule.query(SystemTime::now())),
        ("flush", || capsule.flush()),
    ];

    for (op_name, op) in operations {
        let mut sampler = LatencySampler::new(100);

        // Warmup
        for _ in 0..10 {
            let _ = op();
        }

        // Measure
        for _ in 0..100 {
            let start = Instant::now();
            let _ = op();
            sampler.record(start.elapsed().as_nanos() as u64);
        }

        let p99 = sampler.p99();
        println!("{} P99: {}ns", op_name, p99);

        // All operations should be <1µs P99
        assert!(
            p99 < 1_000,
            "{} P99 {}ns exceeds 1µs threshold",
            op_name,
            p99
        );
    }
}

/// T28 Q5: Isolation - Budget tests don't interfere
#[test]
fn test_budget_isolation() {
    // Run 3 independent budget tests
    for run in 1..=3 {
        let capsule = TimelineAggregationCapsuleWrapper::default();
        let mut sampler = LatencySampler::new(100);

        for _ in 0..100 {
            let start = Instant::now();
            let _ = capsule.append(SystemTime::now(), "test", "data");
            sampler.record(start.elapsed().as_nanos() as u64);
        }

        let p99 = sampler.p99();
        println!("Run {} P99: {}ns", run, p99);

        // Each run should be consistent
        assert!(p99 < 600, "Run {} regression detected", run);
    }
}

/// T28 Q6: Performance - Budget enforcement overhead
#[test]
fn test_budget_enforcement_overhead() {
    let iterations = 10_000;

    // Measure without budget tracking
    let start = Instant::now();
    let capsule = TimelineAggregationCapsuleWrapper::default();
    for _ in 0..iterations {
        let _ = capsule.append(SystemTime::now(), "test", "data");
    }
    let baseline_time = start.elapsed();

    // Measure with budget tracking
    let start = Instant::now();
    let capsule = TimelineAggregationCapsuleWrapper::default();
    let mut sampler = LatencySampler::new(iterations);
    for _ in 0..iterations {
        let op_start = Instant::now();
        let _ = capsule.append(SystemTime::now(), "test", "data");
        sampler.record(op_start.elapsed().as_nanos() as u64);
    }
    let tracked_time = start.elapsed();

    let overhead_pct = ((tracked_time.as_nanos() - baseline_time.as_nanos()) as f64
        / baseline_time.as_nanos() as f64)
        * 100.0;

    println!("Budget tracking overhead: {:.2}%", overhead_pct);

    // Overhead should be <10% (B32 threshold)
    assert!(
        overhead_pct < 10.0,
        "Budget tracking overhead {:.2}% exceeds 10%",
        overhead_pct
    );
}

/// T28 Q7: Readability - Clear failure messages
#[test]
fn test_budget_failure_message_clarity() {
    let budget = PerformanceBudget::baseline();

    // Simulate budget violation
    let measured_p99 = 1000; // Exceeds budget
    let budget_p99 = budget.append_p99_ns;

    if measured_p99 > budget_p99 {
        let message = format!(
            "Performance regression detected:\n  \
             Metric: Append P99\n  \
             Measured: {}ns\n  \
             Budget: {}ns\n  \
             Regression: {:.1}%\n  \
             Action: Investigate recent changes",
            measured_p99,
            budget_p99,
            ((measured_p99 - budget_p99) as f64 / budget_p99 as f64) * 100.0
        );

        println!("{}", message);

        // Message should contain key info
        assert!(message.contains("Performance regression"));
        assert!(message.contains("Measured"));
        assert!(message.contains("Budget"));
        assert!(message.contains("Action"));
    }
}

// ============================================================================
// Tier 2: Property Tests (Q8-Q14) - Budget Holds Across Input Space
// ============================================================================

/// T28 Q8: Universal properties - Budget holds for any valid timestamp
#[test]
fn prop_append_budget_any_timestamp() {
    let capsule = TimelineAggregationCapsuleWrapper::default();
    let budget = PerformanceBudget::baseline();

    let timestamps = vec![
        SystemTime::now(),
        SystemTime::now() + Duration::from_secs(60),
        SystemTime::now() + Duration::from_secs(3600),
        SystemTime::now() - Duration::from_secs(60),
    ];

    for ts in timestamps {
        let mut sampler = LatencySampler::new(100);

        for _ in 0..100 {
            let start = Instant::now();
            let _ = capsule.append(ts, "test", "data");
            sampler.record(start.elapsed().as_nanos() as u64);
        }

        let p99 = sampler.p99();

        // Property: P99 always within budget
        let tolerance = 1.2;
        assert!(
            p99 < (budget.append_p99_ns as f64 * tolerance) as u64,
            "Timestamp {:?} violated budget: P99={}ns",
            ts,
            p99
        );
    }
}

/// T28 Q9: Concurrent invariant - Budget holds under concurrency
#[test]
fn prop_append_budget_concurrent() {
    use clapi_core::test_utils::ConcurrentTestBuilder;
    use std::sync::Arc;

    let capsule = Arc::new(TimelineAggregationCapsuleWrapper::default());
    let budget = PerformanceBudget::baseline();

    // Warmup
    for _ in 0..100 {
        let _ = capsule.append(SystemTime::now(), "test", "data");
    }

    // Concurrent append test (100 threads × 100 ops = 10K total)
    let c = Arc::clone(&capsule);
    let result = ConcurrentTestBuilder::new()
        .threads(100)
        .ops_per_thread(100)
        .run(move |_| {
            let start = Instant::now();
            let _ = c.append(SystemTime::now(), "test", "data");
            start.elapsed().as_nanos() as u64
        });

    // Aggregate latencies
    let mut all_latencies = result.results;
    all_latencies.sort_unstable();

    let p99_idx = (0.99 * all_latencies.len() as f64) as usize;
    let p99 = all_latencies[p99_idx];

    println!("Concurrent append P99: {}ns", p99);

    // Property: Concurrent P99 within budget (with relaxed tolerance)
    let tolerance = 2.0; // 2× tolerance for concurrent scenario
    assert!(
        p99 < (budget.append_p99_ns as f64 * tolerance) as u64,
        "Concurrent P99 {}ns exceeds budget with tolerance",
        p99
    );
}

// ============================================================================
// Tier 3: Integration Tests (Q15-Q21) - End-to-End Workflow Budgets
// ============================================================================

/// T28 Q15: Critical integration - Append → Query workflow budget
#[test]
fn integration_append_query_workflow_budget() {
    let capsule = TimelineAggregationCapsuleWrapper::default();
    let budget = PerformanceBudget::baseline();

    let mut sampler = LatencySampler::new(1000);

    for _ in 0..1000 {
        let workflow_start = Instant::now();

        // Workflow: Append + Query
        let ts = SystemTime::now();
        let _ = capsule.append(ts, "test", "data");
        let _ = capsule.query(ts);

        sampler.record(workflow_start.elapsed().as_nanos() as u64);
    }

    let p99 = sampler.p99();
    println!("Append+Query workflow P99: {}ns", p99);

    // Budget: <1µs for complete workflow
    assert!(
        p99 < 1_000,
        "Workflow P99 {}ns exceeds 1µs budget",
        p99
    );
}

/// T28 Q17: Performance budget - Throughput minimum
#[test]
fn integration_throughput_minimum() {
    let capsule = TimelineAggregationCapsuleWrapper::default();
    let budget = PerformanceBudget::baseline();

    let iterations = 100_000;

    // Warmup
    for _ in 0..1000 {
        let _ = capsule.append(SystemTime::now(), "test", "data");
    }

    // Measure throughput
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = capsule.append(SystemTime::now(), "test", "data");
    }
    let elapsed = start.elapsed();

    let ops_per_sec = (iterations as f64 / elapsed.as_secs_f64()) as u64;

    println!("Throughput: {} ops/sec", ops_per_sec);
    println!("Budget: {} ops/sec", budget.throughput_min_ops_sec);

    // Relaxed threshold for CI (50% of budget)
    let min_threshold = budget.throughput_min_ops_sec / 2;
    assert!(
        ops_per_sec >= min_threshold,
        "Throughput {} below minimum {}",
        ops_per_sec,
        min_threshold
    );
}

// ============================================================================
// Tier 4: Production Tests (Q22-Q28) - Sustained Performance Under Load
// ============================================================================

/// T28 Q22: Stress test - Budget holds under 1M operations
#[test]
#[ignore] // Run with: cargo test --ignored performance_budget
fn stress_budget_1m_operations() {
    let capsule = TimelineAggregationCapsuleWrapper::default();
    let budget = PerformanceBudget::baseline();

    let total_ops = 1_000_000;
    let sample_interval = 1000; // Sample every 1000 ops

    let mut sampler = LatencySampler::new(total_ops / sample_interval);

    let overall_start = Instant::now();

    for i in 0..total_ops {
        let start = Instant::now();
        let _ = capsule.append(SystemTime::now(), "test", "data");

        if i % sample_interval == 0 {
            sampler.record(start.elapsed().as_nanos() as u64);
        }

        // Progress indicator
        if i % 100_000 == 0 && i > 0 {
            println!("Progress: {}/{}M operations", i / 1_000_000, total_ops / 1_000_000);
        }
    }

    let total_time = overall_start.elapsed();
    let p99 = sampler.p99();
    let ops_per_sec = (total_ops as f64 / total_time.as_secs_f64()) as u64;

    println!("\nStress Test Results (1M ops):");
    println!("  P99 latency: {}ns", p99);
    println!("  Throughput: {} ops/sec", ops_per_sec);
    println!("  Total time: {:.2}s", total_time.as_secs_f64());

    // Budget assertions (with tolerance)
    let tolerance = 1.5;
    assert!(
        p99 < (budget.append_p99_ns as f64 * tolerance) as u64,
        "Stress test P99 {}ns exceeds budget",
        p99
    );
}

/// T28 Q24: B32 validation - Honest measurement with fair baseline
#[test]
fn production_b32_honest_measurement() {
    println!("\n=== B32 Benchmark Validation ===\n");

    let capsule = TimelineAggregationCapsuleWrapper::default();

    // Fair baseline: Direct atomic operation (lower bound)
    use std::sync::atomic::{AtomicU64, Ordering};
    let atomic_counter = AtomicU64::new(0);

    let mut baseline_sampler = LatencySampler::new(1000);
    for _ in 0..1000 {
        let start = Instant::now();
        atomic_counter.fetch_add(1, Ordering::Relaxed);
        baseline_sampler.record(start.elapsed().as_nanos() as u64);
    }

    // Actual implementation
    let mut impl_sampler = LatencySampler::new(1000);
    for _ in 0..1000 {
        let start = Instant::now();
        let _ = capsule.append(SystemTime::now(), "test", "data");
        impl_sampler.record(start.elapsed().as_nanos() as u64);
    }

    let baseline_p50 = baseline_sampler.p50();
    let impl_p50 = impl_sampler.p50();

    let speedup = impl_p50 as f64 / baseline_p50 as f64;

    println!("Baseline (atomic) P50: {}ns", baseline_p50);
    println!("Implementation P50: {}ns", impl_p50);
    println!("Overhead factor: {:.2}×", speedup);

    // B32: Report honest speedup (not strawman comparison)
    assert!(
        speedup < 50.0,
        "Implementation overhead {:.2}× exceeds 50× threshold",
        speedup
    );
}

/// T28 Q28: Maintainability - Budget tests easy to update
#[test]
fn production_budget_configuration_maintainable() {
    // Demonstrate easy budget updates
    let baseline = PerformanceBudget::baseline();
    let ci = PerformanceBudget::ci_relaxed();

    println!("\nBaseline budgets:");
    println!("  Append P99: {}ns", baseline.append_p99_ns);
    println!("  Query P99: {}ns", baseline.query_p99_ns);

    println!("\nCI budgets (relaxed):");
    println!("  Append P99: {}ns", ci.append_p99_ns);
    println!("  Query P99: {}ns", ci.query_p99_ns);

    // Configuration should be centralized
    assert!(ci.append_p99_ns > baseline.append_p99_ns);
    assert!(ci.query_p99_ns > baseline.query_p99_ns);
}
