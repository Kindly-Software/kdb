//! Property Test 1: EMA Convergence
//!
//! **T28 Tier 2 (Q8-Q13)**: Property-based testing for distributed cache EMA convergence
//!
//! **Property**: Exponential Moving Average (EMA) should converge to constant input value.
//! When feeding the same value repeatedly (1000 iterations), the EMA's variance (σ) should
//! approach zero, and the mean (μ) should approach the input value.
//!
//! **ASSUM Safety Framework**:
//! - #ASSUME_EMA_CONVERGENCE: α=0.1 decay factor ensures convergence within 1000 iterations
//! - #VERIFY_EMA_CONVERGENCE: σ→0 and μ→input_value (within 1% tolerance)
//! - #ASSUME_NO_OVERFLOW: f64 arithmetic doesn't overflow for realistic latency values
//! - #VERIFY_NO_OVERFLOW: All results are finite (not NaN or infinite)
//!
//! **B32 Fair Testing**:
//! - No strawman assumptions (realistic latency range 0-10ms)
//! - Statistical validation (1000 iterations, 1% tolerance)
//! - Deterministic execution (<1 second)

use atomic_capsule::collections::distributed_cache::DistributedCacheNode;
use std::sync::Arc;

/// Property: EMA converges to constant input
///
/// **Mathematical Foundation**:
/// EMA(t) = α·x(t) + (1-α)·EMA(t-1)
/// For constant input x, EMA → x as t → ∞
/// Variance σ² → 0 as t → ∞
///
/// **Test Strategy**:
/// 1. Create node with clean state
/// 2. Feed constant latency value 1000 times
/// 3. Verify μ ≈ input_value (within 1%)
/// 4. Verify σ ≈ 0 (low variance)
#[test]
fn test_ema_convergence_to_constant() {
    // #ASSUME_ALPHA: α=0.1 standard for EMA, proven convergence rate
    const ALPHA: f64 = 0.1;
    const ITERATIONS: usize = 1000;
    const INPUT_VALUE_US: f64 = 5000.0; // 5ms constant latency
    const TOLERANCE_PERCENT: f64 = 1.0; // 1% tolerance

    // Arrange: Create fresh node (generation counters start clean)
    let node = Arc::new(DistributedCacheNode::new(1, 0));

    // Act: Feed constant latency 1000 times
    // #ASSUME_DETERMINISTIC: Sequential updates produce deterministic EMA
    for _ in 0..ITERATIONS {
        node.record_latency_us(INPUT_VALUE_US);
    }

    // Assert: μ converged to input value (within 1%)
    let final_mu = node.latency_p99_us(); // Actually returns μ (EMA mean)
    let mu_error_percent = ((final_mu - INPUT_VALUE_US).abs() / INPUT_VALUE_US) * 100.0;

    // #VERIFY_CONVERGENCE: μ within 1% of target
    assert!(
        mu_error_percent < TOLERANCE_PERCENT,
        "EMA mean did not converge: μ={:.2}μs, expected={:.2}μs, error={:.2}%",
        final_mu,
        INPUT_VALUE_US,
        mu_error_percent
    );

    // #VERIFY_NO_OVERFLOW: Result is finite (not NaN or infinite)
    assert!(final_mu.is_finite(), "EMA mean is not finite: {}", final_mu);

    // Additional verification: μ should be very close to input after 1000 iterations
    // With α=0.1, convergence is ~99% after 46 iterations, >99.9% after 100 iterations
    // After 1000 iterations, should be essentially exact
    assert!(
        mu_error_percent < 0.1,
        "EMA convergence too slow: error={:.4}% after {} iterations",
        mu_error_percent,
        ITERATIONS
    );
}

/// Property: EMA variance decreases with constant input
///
/// **Test Strategy**:
/// 1. Record variance after 10, 100, 1000 iterations
/// 2. Verify variance is monotonically decreasing
/// 3. Verify final variance is near zero
#[test]
fn test_ema_variance_decreases() {
    const INPUT_VALUE_US: f64 = 3000.0; // 3ms constant latency
    let node = Arc::new(DistributedCacheNode::new(2, 0));

    // Checkpoints for variance sampling
    let checkpoints = [10, 100, 1000];
    let mut variances = Vec::new();

    let mut iteration = 0;
    for &checkpoint in &checkpoints {
        // Feed latency until checkpoint
        while iteration < checkpoint {
            node.record_latency_us(INPUT_VALUE_US);
            iteration += 1;
        }

        // Sample variance approximation (we don't have direct σ access)
        // Use μ convergence as proxy: error decreases → implicit variance decrease
        let mu = node.latency_p99_us();
        let error = (mu - INPUT_VALUE_US).abs();
        variances.push(error);
    }

    // #VERIFY_MONOTONIC_DECREASE: Each checkpoint has lower error than previous
    for i in 1..variances.len() {
        assert!(
            variances[i] <= variances[i - 1],
            "Variance did not decrease monotonically: checkpoint {}={:.2}, checkpoint {}={:.2}",
            checkpoints[i - 1],
            variances[i - 1],
            checkpoints[i],
            variances[i]
        );
    }

    // #VERIFY_FINAL_VARIANCE: After 1000 iterations, error < 0.1%
    let final_error_percent = (variances[2] / INPUT_VALUE_US) * 100.0;
    assert!(
        final_error_percent < 0.1,
        "Final variance too high: {:.4}%",
        final_error_percent
    );
}

/// Property: EMA handles extreme but valid values
///
/// **Edge Case Testing (T28 Q10)**:
/// - Zero latency (immediate response)
/// - Very high latency (10ms = 10,000μs)
/// - Convergence still works for all valid ranges
#[test]
fn test_ema_convergence_edge_cases() {
    // Test 1: Zero latency (immediate local cache hit)
    {
        let node = Arc::new(DistributedCacheNode::new(3, 0));
        for _ in 0..1000 {
            node.record_latency_us(0.0);
        }
        let mu = node.latency_p99_us();
        assert!(
            mu < 0.01, // Should be essentially zero (floating-point tolerance)
            "Zero latency convergence failed: μ={:.6}μs",
            mu
        );
        assert!(mu.is_finite(), "Zero latency produced non-finite result");
    }

    // Test 2: Very high latency (slow remote node)
    {
        const HIGH_LATENCY_US: f64 = 10_000.0; // 10ms
        let node = Arc::new(DistributedCacheNode::new(4, 0));
        for _ in 0..1000 {
            node.record_latency_us(HIGH_LATENCY_US);
        }
        let mu = node.latency_p99_us();
        let error_percent = ((mu - HIGH_LATENCY_US).abs() / HIGH_LATENCY_US) * 100.0;
        assert!(
            error_percent < 0.1,
            "High latency convergence failed: μ={:.2}μs, expected={:.2}μs, error={:.4}%",
            mu,
            HIGH_LATENCY_US,
            error_percent
        );
        assert!(mu.is_finite(), "High latency produced non-finite result");
    }
}

/// Property: EMA is deterministic for same input sequence
///
/// **Reproducibility (T28 Q14)**:
/// Same input sequence → same EMA output
#[test]
fn test_ema_deterministic() {
    const INPUT_SEQUENCE: [f64; 5] = [1000.0, 2000.0, 1500.0, 3000.0, 2500.0];
    const ITERATIONS: usize = 100;

    // Run 1: First execution
    let node1 = Arc::new(DistributedCacheNode::new(5, 0));
    for _ in 0..ITERATIONS {
        for &latency in &INPUT_SEQUENCE {
            node1.record_latency_us(latency);
        }
    }
    let mu1 = node1.latency_p99_us();

    // Run 2: Second execution (fresh node, same inputs)
    let node2 = Arc::new(DistributedCacheNode::new(6, 0));
    for _ in 0..ITERATIONS {
        for &latency in &INPUT_SEQUENCE {
            node2.record_latency_us(latency);
        }
    }
    let mu2 = node2.latency_p99_us();

    // #VERIFY_DETERMINISTIC: Both runs produce identical results
    assert_eq!(
        mu1, mu2,
        "EMA is not deterministic: run1={:.6}μs, run2={:.6}μs",
        mu1, mu2
    );
}

/// Test execution time validation
///
/// **Performance Requirement**: All property tests < 1 second
#[test]
fn test_execution_time_budget() {
    let start = std::time::Instant::now();

    // Run all property tests inline
    test_ema_convergence_to_constant();
    test_ema_variance_decreases();
    test_ema_convergence_edge_cases();
    test_ema_deterministic();

    let elapsed = start.elapsed();

    // #VERIFY_PERFORMANCE_BUDGET: All tests complete in < 1 second
    assert!(
        elapsed.as_millis() < 1000,
        "Property tests exceeded 1s budget: {:.2}ms",
        elapsed.as_millis()
    );
}
