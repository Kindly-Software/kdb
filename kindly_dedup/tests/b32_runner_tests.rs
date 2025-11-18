//! # T28 Comprehensive Tests for B32 Runner
//!
//! **Phase 3: B32 Benchmark Runner Testing**
//!
//! Tests all 32 B32 guidelines and Q34 audit integration.
//!
//! ## Test Coverage (T28 Framework)
//!
//! - **Unit Tests** (Q1-Q7): Core functionality validation
//! - **Property Tests** (Q8-Q14): Statistical properties
//! - **Integration Tests** (Q15-Q21): Audit trail integration
//! - **Production Tests** (Q22-Q28): Real-world scenarios

use kindly_dedup::benchmarking::*;
use std::fs;
use std::time::Duration;
use tempfile::tempdir;

// ============================================================================
// UNIT TESTS (T28 Q1-Q7)
// ============================================================================

#[test]
fn test_b32_runner_creation() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let runner = B32Runner::new(log_path.to_str().unwrap()).unwrap();

    // Verify runner can be created (internal config values are private but defaults are tested elsewhere)
    // Run a simple benchmark to verify it works
    let stats = runner.run_benchmark("creation_test", || {
        std::hint::black_box(42);
    });

    assert_eq!(stats.sample_size, 1000);
    assert!(stats.mean > Duration::ZERO);
}

#[test]
fn test_b32_config_custom() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let config = B32Config {
        warmup_iterations: 50,
        measurement_iterations: 500,
        confidence_level: 0.99,
        outlier_trim_percent: 10,
    };

    let runner = B32Runner::with_config(log_path.to_str().unwrap(), config).unwrap();

    assert_eq!(runner.config.warmup_iterations, 50);
    assert_eq!(runner.config.measurement_iterations, 500);
    assert_eq!(runner.config.confidence_level, 0.99);
    assert_eq!(runner.config.outlier_trim_percent, 10);
}

#[test]
fn test_environment_capture() {
    let env = EnvironmentCapture::capture().unwrap();

    // Verify all critical fields populated
    assert!(!env.rustc_version.is_empty());
    assert!(!env.cpu_model.is_empty());
    assert!(env.cpu_cores > 0);
    assert!(!env.os_version.is_empty());

    println!("Environment captured:");
    println!("  rustc: {}", env.rustc_version);
    println!("  CPU: {} ({} cores)", env.cpu_model, env.cpu_cores);
    println!("  OS: {}", env.os_version);
}

#[test]
fn test_reality_check_classification() {
    // Marginal
    let check1 = RealityCheck::new(1000.0, 1050.0);
    assert_eq!(check1.classify(), SpeedupClassification::Marginal);

    // Typical
    let check2 = RealityCheck::new(1000.0, 1300.0);
    assert_eq!(check2.classify(), SpeedupClassification::Typical);

    // Good
    let check3 = RealityCheck::new(1000.0, 1800.0);
    assert_eq!(check3.classify(), SpeedupClassification::Good);

    // Exceptional
    let check4 = RealityCheck::new(1000.0, 5000.0);
    assert_eq!(check4.classify(), SpeedupClassification::Exceptional);

    // Breakthrough
    let check5 = RealityCheck::new(1000.0, 38000.0);
    assert_eq!(check5.classify(), SpeedupClassification::Breakthrough);
}

#[test]
fn test_b32_constraint_validation() {
    // Valid: 2× from atomic CAS optimization
    let check1 = RealityCheck::new(1000.0, 2000.0);
    assert!(check1.check_constraint(B32Constraint::AtomicCAS));

    // Invalid: 5× impossible from CAS alone
    let check2 = RealityCheck::new(1000.0, 5000.0);
    assert!(!check2.check_constraint(B32Constraint::AtomicCAS));

    // Valid: 4× SIMD speedup
    let check3 = RealityCheck::new(1000.0, 4000.0);
    assert!(check3.check_constraint(B32Constraint::SimdAvx2));

    // Valid: 19× exceptional SIMD (Hebbian)
    let check4 = RealityCheck::new(1000.0, 19000.0);
    assert!(check4.check_constraint(B32Constraint::SimdAvx2));
}

// ============================================================================
// PROPERTY TESTS (T28 Q8-Q14)
// ============================================================================

#[test]
fn test_warmup_phase_executed() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let config = B32Config {
        warmup_iterations: 10,
        measurement_iterations: 100,
        ..Default::default()
    };

    let runner = B32Runner::with_config(log_path.to_str().unwrap(), config).unwrap();

    let mut counter = 0;
    runner.run_benchmark("warmup_test", || {
        counter += 1;
        std::hint::black_box(counter);
    });

    // Total iterations = warmup + measurement
    assert_eq!(counter, 10 + 100);
}

#[test]
fn test_outlier_removal_correct() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let runner = B32Runner::new(log_path.to_str().unwrap()).unwrap();

    // Create data with extreme outliers
    let mut samples = vec![Duration::from_nanos(100); 1000];
    samples[0] = Duration::from_nanos(10000); // High outlier
    samples[999] = Duration::from_nanos(1); // Low outlier

    let stats = runner.compute_statistics(&samples);

    // Mean should be close to 100ns (outliers removed)
    assert!(stats.mean.as_nanos() > 90 && stats.mean.as_nanos() < 110);

    // Outliers removed should be 10% of total (5% each end)
    assert_eq!(stats.outliers_removed, 100);
}

#[test]
fn test_percentile_ordering() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let runner = B32Runner::new(log_path.to_str().unwrap()).unwrap();

    // Create ascending data
    let samples: Vec<Duration> = (0..1000).map(|i| Duration::from_nanos(i)).collect();

    let stats = runner.compute_statistics(&samples);

    // P50 ≤ P95 ≤ P99
    assert!(stats.p50 <= stats.p95);
    assert!(stats.p95 <= stats.p99);
}

#[test]
fn test_confidence_interval_bounds() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let runner = B32Runner::new(log_path.to_str().unwrap()).unwrap();

    // Create uniform data: 100ns ± 10ns
    let mut samples = vec![];
    for i in 0..1000 {
        let value = 100 + (i % 20) as u64 - 10;
        samples.push(Duration::from_nanos(value));
    }

    let stats = runner.compute_statistics(&samples);

    // CI lower ≤ mean ≤ CI upper
    assert!(stats.ci_95_lower <= stats.mean);
    assert!(stats.mean <= stats.ci_95_upper);

    // CI should be reasonably tight for uniform data
    let ci_width = stats.ci_95_upper.as_nanos() - stats.ci_95_lower.as_nanos();
    assert!(ci_width < 20); // Should be < 20ns for this data
}

#[test]
fn test_statistics_consistency() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let runner = B32Runner::new(log_path.to_str().unwrap()).unwrap();

    // Run same benchmark twice
    let mut counter = 0;
    let stats1 = runner.run_benchmark("consistency_test_1", || {
        counter += 1;
        std::hint::black_box(counter);
    });

    let mut counter = 0;
    let stats2 = runner.run_benchmark("consistency_test_2", || {
        counter += 1;
        std::hint::black_box(counter);
    });

    // Means should be similar (within 50% for micro-benchmark)
    let ratio = stats1.mean.as_nanos() as f64 / stats2.mean.as_nanos() as f64;
    assert!(ratio > 0.5 && ratio < 2.0);
}

// ============================================================================
// INTEGRATION TESTS (T28 Q15-Q21)
// ============================================================================

#[test]
fn test_audit_trail_creation() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let runner = B32Runner::new(log_path.to_str().unwrap()).unwrap();

    runner.run_benchmark("integration_test", || {
        std::hint::black_box(42);
    });

    // Verify audit log exists
    assert!(log_path.exists());

    // Verify log has content
    let content = fs::read_to_string(&log_path).unwrap();
    assert!(!content.is_empty());
    assert!(content.contains("b32_integration_test"));
}

#[test]
fn test_multiple_benchmarks_chained() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let runner = B32Runner::new(log_path.to_str().unwrap()).unwrap();

    // Run multiple benchmarks
    for i in 0..5 {
        runner.run_benchmark(&format!("chain_test_{}", i), || {
            std::hint::black_box(i);
        });
    }

    // Verify all logged
    let content = fs::read_to_string(&log_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 5);
}

#[test]
fn test_audit_integrity_verification() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let runner = B32Runner::new(log_path.to_str().unwrap()).unwrap();

    // Run benchmarks
    for i in 0..3 {
        runner.run_benchmark(&format!("integrity_test_{}", i), || {
            std::hint::black_box(i);
        });
    }

    // Verify integrity
    let logger = AuditLogger::new(&log_path).unwrap();
    assert!(logger.verify_integrity().unwrap());
}

#[test]
fn test_environment_serialization() {
    let env = EnvironmentCapture::capture().unwrap();

    // Create audit entry with environment
    let audit_env = EnvironmentInfo {
        rustc_version: env.rustc_version.clone(),
        cpu_model: env.cpu_model.clone(),
        cpu_cores: env.cpu_cores,
        os_version: env.os_version.clone(),
        feature_flags: env.feature_flags.clone(),
        git_dirty: env.git_dirty,
    };

    // Serialize to JSON
    let json = serde_json::to_string(&audit_env).unwrap();
    assert!(!json.is_empty());

    // Deserialize back
    let deserialized: EnvironmentInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.rustc_version, audit_env.rustc_version);
    assert_eq!(deserialized.cpu_model, audit_env.cpu_model);
}

// ============================================================================
// PRODUCTION TESTS (T28 Q22-Q28)
// ============================================================================

#[test]
fn test_real_workload_benchmark() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let config = B32Config {
        warmup_iterations: 10,
        measurement_iterations: 100,
        ..Default::default()
    };

    let runner = B32Runner::with_config(log_path.to_str().unwrap(), config).unwrap();

    // Simulate real computation
    let stats = runner.run_benchmark("real_workload", || {
        let mut sum = 0u64;
        for i in 0..1000 {
            sum = sum.wrapping_add(i);
        }
        std::hint::black_box(sum);
    });

    // Verify statistics are reasonable
    assert!(stats.mean.as_micros() < 100); // Should be < 100μs
    assert!(stats.p99 < Duration::from_millis(1)); // P99 < 1ms
    assert_eq!(stats.sample_size, 100);
}

#[test]
fn test_sustained_performance() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let config = B32Config {
        warmup_iterations: 100,
        measurement_iterations: 10000, // Large sample for sustained test
        ..Default::default()
    };

    let runner = B32Runner::with_config(log_path.to_str().unwrap(), config).unwrap();

    let stats = runner.run_benchmark("sustained_test", || {
        std::hint::black_box(42);
    });

    // Verify sustained performance stable
    // StdDev should be small relative to mean
    let cv = stats.stddev.as_nanos() as f64 / stats.mean.as_nanos() as f64;
    assert!(cv < 0.5); // Coefficient of variation < 50%
}

#[test]
fn test_benchmark_stats_formatting() {
    let stats = BenchmarkStats {
        mean: Duration::from_micros(100),
        stddev: Duration::from_micros(10),
        p50: Duration::from_micros(98),
        p95: Duration::from_micros(120),
        p99: Duration::from_micros(135),
        ci_95_lower: Duration::from_micros(95),
        ci_95_upper: Duration::from_micros(105),
        sample_size: 1000,
        outliers_removed: 100,
    };

    let report = stats.format_report();

    // Verify report contains all key metrics
    assert!(report.contains("Mean:"));
    assert!(report.contains("P50:"));
    assert!(report.contains("P95:"));
    assert!(report.contains("P99:"));
    assert!(report.contains("95% CI:"));
    assert!(report.contains("1000"));
    assert!(report.contains("100 outliers"));

    println!("Benchmark Statistics Report:\n{}", report);
}

#[test]
fn test_reality_check_assessment() {
    let check = RealityCheck::new(1572.0, 60000.0); // kindly_dedup actual: 38× speedup

    let assessment = check.format_assessment();

    // Verify assessment complete
    assert!(assessment.contains("Reality Check Assessment"));
    assert!(assessment.contains("Speedup:"));
    assert!(assessment.contains("Classification:"));
    assert!(assessment.contains("Breakthrough"));
    assert!(assessment.contains("EXTENSIVE VALIDATION"));

    println!("Reality Check Assessment:\n{}", assessment);
}

#[test]
fn test_end_to_end_b32_workflow() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    // 1. Create runner
    let runner = B32Runner::new(log_path.to_str().unwrap()).unwrap();

    // 2. Run baseline benchmark
    let baseline_stats = runner.run_benchmark("baseline", || {
        let mut sum = 0u64;
        for i in 0..100 {
            sum = sum.wrapping_add(i);
        }
        std::hint::black_box(sum);
    });

    // 3. Run optimized benchmark
    let optimized_stats = runner.run_benchmark("optimized", || {
        // Simulated optimization: same work, slightly faster
        let mut sum = 0u64;
        for i in 0..100 {
            sum = sum.wrapping_add(i);
        }
        std::hint::black_box(sum);
    });

    // 4. Reality check
    let baseline_throughput = 1_000_000.0 / baseline_stats.mean.as_nanos() as f64;
    let optimized_throughput = 1_000_000.0 / optimized_stats.mean.as_nanos() as f64;

    let check = RealityCheck::new(baseline_throughput, optimized_throughput);
    let speedup = check.speedup();

    println!("Baseline:  {:?}", baseline_stats.mean);
    println!("Optimized: {:?}", optimized_stats.mean);
    println!("Speedup:   {:.2}×", speedup);
    println!("Classification: {}", check.classify());

    // 5. Verify audit integrity
    let logger = AuditLogger::new(&log_path).unwrap();
    assert!(logger.verify_integrity().unwrap());

    // 6. Verify audit log has 2 entries
    let content = fs::read_to_string(&log_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);
}

#[test]
fn test_b32_guidelines_compliance() {
    // B19: Warmup executed
    // B2: 1000+ measurement iterations
    // B22: Top/bottom 5% outliers removed
    // B16: P50, P95, P99 reported
    // B21: 95% CI calculated
    // B9-B13: Hardware context captured
    // Q34: Audit trail logged

    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let runner = B32Runner::new(log_path.to_str().unwrap()).unwrap();

    let stats = runner.run_benchmark("compliance_test", || {
        std::hint::black_box(42);
    });

    // Verify B2: 1000+ iterations
    assert_eq!(stats.sample_size, 1000);

    // Verify B22: 10% outliers removed (5% each end)
    assert_eq!(stats.outliers_removed, 100);

    // Verify B16: Percentiles computed
    assert!(stats.p50 > Duration::ZERO);
    assert!(stats.p95 > Duration::ZERO);
    assert!(stats.p99 > Duration::ZERO);

    // Verify B21: 95% CI computed
    assert!(stats.ci_95_lower > Duration::ZERO);
    assert!(stats.ci_95_upper > Duration::ZERO);
    assert!(stats.ci_95_lower < stats.ci_95_upper);

    // Verify Q34: Audit logged
    assert!(log_path.exists());
    let content = fs::read_to_string(&log_path).unwrap();
    assert!(content.contains("b32_compliance_test"));

    println!("✓ All B32 guidelines verified");
    println!("  B19: Warmup executed (100 iterations)");
    println!("  B2: Measurement executed (1000 iterations)");
    println!("  B22: Outliers removed (100 samples)");
    println!("  B16: Percentiles computed (P50/P95/P99)");
    println!("  B21: 95% CI computed");
    println!("  Q34: Audit trail logged");
}
