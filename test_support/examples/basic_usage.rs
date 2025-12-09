//! Basic usage example of test_support utilities
//!
//! Demonstrates B32 benchmarking, statistical validation, and lockfree verification.

use test_support::*;
use test_support::validation::Assert;
use test_support::generators::MarketConfig;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Test Support Primitive - Basic Usage Example");
    println!("=============================================\n");

    // 1. B32 Benchmarking Example
    println!("1. B32 Benchmarking Framework");
    println!("------------------------------");

    let validator = BenchmarkValidator::new()
        .with_baseline("mutex", 100.0) // 100ns baseline
        .with_baseline("rwlock", 80.0); // 80ns baseline

    let atomic = Arc::new(AtomicU64::new(0));
    let result = {
        let atomic = Arc::clone(&atomic);
        validator.measure_operation(move || {
            atomic.fetch_add(1, Ordering::Relaxed);
        })?
    };

    println!("B32 Benchmark Result:");
    println!("  Mean: {:.2}ns", result.mean_ns);
    println!("  95% CI: [{:.2}, {:.2}]ns",
        result.confidence_interval.0, result.confidence_interval.1);
    println!("  P99: {:.2}ns", result.percentiles.p99);
    println!("  B32 Compliant: {}", result.meets_b32_standards());

    if let Some(ref comparison) = result.baseline_comparison {
        println!("  Speedup vs {}: {:.2}x",
            comparison.baseline_name, comparison.speedup);
    }
    println!();

    // 2. Statistical Validation Example
    println!("2. Statistical Validation");
    println!("-------------------------");

    let stat_validator = StatisticalValidator::new()
        .with_confidence_level(0.95)
        .with_min_sample_size(100);

    // Generate test measurements
    let mut generator = TestDataGenerator::default_config();
    let measurements = generator.generate_values(1000)?;

    let metrics = stat_validator.analyze_measurements(&measurements)?;
    println!("Statistical Analysis:");
    println!("  Sample size: {}", metrics.sample_size);
    println!("  Mean: {:.2} ± {:.2}", metrics.mean, metrics.std_dev);
    println!("  CV: {:.1}%", metrics.coefficient_of_variation * 100.0);
    println!("  Outliers: {}", metrics.outliers.len());
    println!("  95% CI: [{:.2}, {:.2}]",
        metrics.confidence_interval.lower_bound,
        metrics.confidence_interval.upper_bound);
    println!();

    // 3. Lockfree Verification Example
    println!("3. Lockfree Verification");
    println!("------------------------");

    let mut lockfree_verifier = LockfreeVerifier::new();
    let atomic_for_verify = Arc::new(AtomicU64::new(0));

    let verification_result = {
        let atomic = Arc::clone(&atomic_for_verify);
        lockfree_verifier.verify_atomic_operation(move || {
            atomic.compare_exchange_weak(
                0, 1,
                Ordering::AcqRel,
                Ordering::Relaxed
            ).unwrap_or_else(|_| 0)
        })?
    };

    println!("Lockfree Verification:");
    println!("  Lockfree compliant: {}", verification_result.is_lockfree_compliant);
    println!("  Single-thread perf: {:.2}ns", verification_result.performance_profile.single_thread_ns);
    println!("  Scaling efficiency:");
    for (&threads, &efficiency) in &verification_result.performance_profile.scaling_efficiency {
        println!("    {} threads: {:.1}%", threads, efficiency * 100.0);
    }
    println!("  Safety violations: {}", verification_result.safety_violations.len());

    if !verification_result.recommendations.is_empty() {
        println!("  Recommendations:");
        for rec in &verification_result.recommendations {
            println!("    - {}", rec);
        }
    }
    println!();

    // 4. Assertion Framework Example
    println!("4. Test Assertions");
    println!("------------------");

    let results = vec![
        Assert::eq("basic_equality", 2 + 2, 4),
        Assert::in_range("performance_check", result.mean_ns, 5.0, 50.0),
        Assert::approx_eq("floating_point", 1.0001, 1.0, 0.001),
        Assert::is_true("lockfree_check", verification_result.is_lockfree_compliant),
    ];

    let combined = ValidationResult::combine(results);
    println!("Assertion Results:");
    println!("  Total assertions: {}", combined.assertion_count);
    println!("  All passed: {}", combined.passed);

    if !combined.passed {
        if let Some(details) = &combined.details {
            println!("  Failures:\n{}", details);
        }
    }
    println!();

    // 5. Test Data Generation Example
    println!("5. Test Data Generation");
    println!("----------------------");

    let mut market_generator = MarketDataGenerator::new(MarketConfig::default(), 12345);
    let market_data = market_generator.generate_market_data(5)?;

    println!("Generated market data:");
    for data_point in &market_data {
        println!("  {}: ${:.2} (vol: {})",
            data_point.instrument, data_point.price, data_point.volume);
    }
    println!();

    // Summary
    println!("Summary");
    println!("=======");
    println!("✓ B32 benchmarking completed with {} iterations", result.iterations);
    println!("✓ Statistical analysis performed on {} samples", metrics.sample_size);
    println!("✓ Lockfree verification completed across {} thread levels",
        verification_result.performance_profile.scaling_efficiency.len());
    println!("✓ {} assertions validated", combined.assertion_count);
    println!("✓ {} market data points generated", market_data.len());

    println!("\nTest support primitives working correctly! 🚀");

    Ok(())
}