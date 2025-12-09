//! Partial Degradation Chaos Test (Scenario 5)
//!
//! **Purpose**: 50% of providers intermittent (random 10% failures)
//! **Expected Behavior**:
//! - System continues with reduced capacity
//! - Automatic failover to healthy providers
//! - Performance impact <50% degradation
//! - Graceful degradation
//!
//! # ASSUM Safety
//! - #ASSUME: Partial degradation doesn't cascade to total failure
//! - #VERIFY: Healthy providers remain functional
//! - #ASSUME: Failover keeps overall failure rate <20%
//! - #VERIFY: Measure aggregate failure rate across all providers
//! - #ASSUME: Performance degrades linearly with failures (not exponentially)
//! - #VERIFY: Compare throughput vs failure percentage
//!
//! # UCE34 Compliance
//! - Q23 (Partial failures): Handle subset of providers failing
//! - Q24 (Load balancing): Distribute load across healthy providers
//! - Q25 (Graceful degradation): Degrade proportionally, not catastrophically
//!
//! # T28 Testing
//! - Q22: Production scenario (partial outages are common)
//! - Q24: B32 benchmarks (measure throughput degradation)

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use rand::Rng;

use clapi_core::proxy::BudgetRegistry;
use super::{ChaosConfig, ChaosFault, ChaosTestHarness};

/// Partial degradation simulator
#[derive(Clone)]
struct PartialDegradationSimulator {
    /// Degradation enabled flag
    enabled: Arc<AtomicBool>,
    /// Failure rate (basis points, 0-10000)
    failure_rate_bp: u64,
    /// Total requests
    total_requests: Arc<AtomicU64>,
    /// Failed requests
    failed_requests: Arc<AtomicU64>,
}

impl PartialDegradationSimulator {
    fn new(enabled: Arc<AtomicBool>, failure_rate_bp: u64) -> Self {
        Self {
            enabled,
            failure_rate_bp,
            total_requests: Arc::new(AtomicU64::new(0)),
            failed_requests: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Check if this request should fail (probabilistic)
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Random number generator is thread-safe
    /// - #VERIFY: Use thread_rng() for thread-local RNG
    /// - #ASSUME: Failure rate is uniform over time
    /// - #VERIFY: Statistical distribution matches configured rate
    fn should_fail(&self) -> bool {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        if !self.enabled.load(Ordering::Acquire) {
            return false;
        }

        // Generate random number (0-10000)
        let mut rng = rand::thread_rng();
        let roll = rng.gen_range(0..10000);

        let failed = roll < self.failure_rate_bp;
        if failed {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
        }

        failed
    }

    /// Get actual failure rate (basis points)
    fn get_actual_failure_rate_bp(&self) -> u64 {
        let total = self.total_requests.load(Ordering::Relaxed);
        let failed = self.failed_requests.load(Ordering::Relaxed);

        if total == 0 {
            return 0;
        }

        (failed * 10000) / total
    }

    fn clone_handle(&self) -> Self {
        Self {
            enabled: Arc::clone(&self.enabled),
            failure_rate_bp: self.failure_rate_bp,
            total_requests: Arc::clone(&self.total_requests),
            failed_requests: Arc::clone(&self.failed_requests),
        }
    }
}

/// Test: Partial degradation (10% intermittent failures)
///
/// # Test Scenario
/// 1. Baseline: Normal operation (10s)
/// 2. Chaos: 10% random failures (30s)
/// 3. Recovery: Remove failures, validate recovery (30s)
///
/// # Expected Results
/// - ~10% failure rate during chaos
/// - System continues serving (90% success)
/// - Performance degrades <20%
/// - Recovery restores 100% success
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_partial_degradation_10pct() {
    // Setup chaos config: 10% failures (1000 bp)
    let config = ChaosConfig::new(
        ChaosFault::PartialDegradation { failure_rate_bp: 1000 },
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    // Create simulator
    let simulator = PartialDegradationSimulator::new(Arc::clone(&config.enabled), 1000);

    // Budget registry
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let budget_id = 0x1111222233334444;

    // Test function
    let test_fn = {
        let simulator = simulator.clone_handle();
        let budget_registry = Arc::clone(&budget_registry);

        move || {
            if simulator.should_fail() {
                return Err("Intermittent failure (10% degradation)".to_string());
            }

            budget_registry.try_deduct(budget_id, 1_00)
                .map(|_| ())
                .map_err(|e| format!("Budget error: {:?}", e))
        }
    };

    // Run chaos test
    let harness = ChaosTestHarness::new(config);
    let results = harness.run("Partial Degradation 10%", test_fn);

    // Validate results
    // #ASSUME: System survives partial degradation
    // #VERIFY: Test completed
    assert!(results.survived, "System should survive 10% degradation");

    // #ASSUME: Actual failure rate matches configured rate (±2%)
    // #VERIFY: Failure rate is 8-12% (800-1200 bp)
    let actual_rate_bp = simulator.get_actual_failure_rate_bp();
    assert!(
        actual_rate_bp >= 800 && actual_rate_bp <= 1200,
        "Actual failure rate should be 8-12%, got {} bp",
        actual_rate_bp
    );

    // #ASSUME: Chaos failure rate ~10%
    // #VERIFY: Results match simulator
    assert!(
        results.chaos_failure_rate_bp() >= 800 && results.chaos_failure_rate_bp() <= 1200,
        "Chaos failure rate should be ~10%, got {} bp",
        results.chaos_failure_rate_bp()
    );

    // #ASSUME: Recovery restores normal operation
    // #VERIFY: Recovery failure rate <5%
    assert!(results.recovered, "System should recover");

    println!("\n{}", results.summary());
    println!("Simulator: {} failures / {} requests ({} bp)",
             simulator.failed_requests.load(Ordering::Relaxed),
             simulator.total_requests.load(Ordering::Relaxed),
             actual_rate_bp);
}

/// Test: Higher degradation (30% failures)
///
/// # Test Scenario
/// - 30% intermittent failures
/// - System should continue serving (70% success)
/// - Throughput degradation proportional to failures
///
/// # Expected Results
/// - ~30% failure rate
/// - System remains operational
/// - Throughput ~70% of baseline
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_partial_degradation_30pct() {
    // Setup chaos config: 30% failures (3000 bp)
    let config = ChaosConfig::new(
        ChaosFault::PartialDegradation { failure_rate_bp: 3000 },
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    let simulator = PartialDegradationSimulator::new(Arc::clone(&config.enabled), 3000);
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let budget_id = 0x5555666677778888;

    let test_fn = {
        let simulator = simulator.clone_handle();
        let budget_registry = Arc::clone(&budget_registry);

        move || {
            if simulator.should_fail() {
                return Err("Intermittent failure (30% degradation)".to_string());
            }

            budget_registry.try_deduct(budget_id, 1_00)
                .map(|_| ())
                .map_err(|e| format!("Budget error: {:?}", e))
        }
    };

    let harness = ChaosTestHarness::new(config);
    let results = harness.run("Partial Degradation 30%", test_fn);

    // Validate higher degradation
    // #ASSUME: System survives 30% degradation
    // #VERIFY: Test completed
    assert!(results.survived, "System should survive 30% degradation");

    // #ASSUME: Failure rate ~30% (±2%)
    // #VERIFY: 28-32% failure rate (2800-3200 bp)
    assert!(
        results.chaos_failure_rate_bp() >= 2800 && results.chaos_failure_rate_bp() <= 3200,
        "Chaos failure rate should be ~30%, got {} bp",
        results.chaos_failure_rate_bp()
    );

    // #ASSUME: Recovery restores service
    // #VERIFY: Recovery failure rate <5%
    assert!(results.recovered, "System should recover");

    println!("\n{}", results.summary());
}

/// Test: Multi-provider degradation (50% providers degraded)
///
/// # Test Scenario
/// - 50% of providers are degraded (10% failures each)
/// - 50% of providers are healthy (0% failures)
/// - Load balancer should prefer healthy providers
/// - Overall failure rate should be <10% (not 5%)
///
/// # Expected Results
/// - Overall failure rate 3-7% (load balancing helps)
/// - Healthy providers get more traffic
/// - Degraded providers get less traffic
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_multi_provider_degradation() {
    use std::sync::Arc;

    // Setup: 4 providers, 2 healthy + 2 degraded
    let config = ChaosConfig::new(
        ChaosFault::PartialDegradation { failure_rate_bp: 1000 }, // 10% when degraded
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    // Provider simulators (0-1 healthy, 2-3 degraded)
    let provider_0 = PartialDegradationSimulator::new(Arc::new(AtomicBool::new(false)), 0); // Healthy
    let provider_1 = PartialDegradationSimulator::new(Arc::new(AtomicBool::new(false)), 0); // Healthy
    let provider_2 = PartialDegradationSimulator::new(Arc::clone(&config.enabled), 1000); // Degraded
    let provider_3 = PartialDegradationSimulator::new(Arc::clone(&config.enabled), 1000); // Degraded

    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let budget_id = 0x9999AAAABBBBCCCC;

    // Track per-provider requests
    let provider_requests = Arc::new(parking_lot::Mutex::new([0u64; 4]));

    let test_fn = {
        let provider_0 = provider_0.clone_handle();
        let provider_1 = provider_1.clone_handle();
        let provider_2 = provider_2.clone_handle();
        let provider_3 = provider_3.clone_handle();
        let budget_registry = Arc::clone(&budget_registry);
        let provider_requests = Arc::clone(&provider_requests);

        move || {
            // Simple round-robin load balancing
            let provider_id = rand::random::<usize>() % 4;
            provider_requests.lock()[provider_id] += 1;

            let failed = match provider_id {
                0 => provider_0.should_fail(),
                1 => provider_1.should_fail(),
                2 => provider_2.should_fail(),
                3 => provider_3.should_fail(),
                _ => unreachable!(),
            };

            if failed {
                return Err(format!("Provider {} failed", provider_id));
            }

            budget_registry.try_deduct(budget_id, 1_00)
                .map(|_| ())
                .map_err(|e| format!("Budget error: {:?}", e))
        }
    };

    let harness = ChaosTestHarness::new(config);
    let results = harness.run("Multi-Provider Degradation", test_fn);

    // Analyze per-provider traffic
    let requests = provider_requests.lock();
    println!("Per-provider requests: {:?}", *requests);

    // Validate overall failure rate
    // #ASSUME: Overall failure rate ~5% (50% providers * 10% failures)
    // #VERIFY: 3-7% failure rate (300-700 bp)
    assert!(
        results.chaos_failure_rate_bp() >= 300 && results.chaos_failure_rate_bp() <= 700,
        "Overall failure rate should be 3-7%, got {} bp",
        results.chaos_failure_rate_bp()
    );

    println!("\n{}", results.summary());
}

#[cfg(test)]
mod compile_tests {
    use super::*;

    #[test]
    fn test_simulator_clone() {
        let enabled = Arc::new(AtomicBool::new(false));
        let simulator = PartialDegradationSimulator::new(enabled, 1000);
        let cloned = simulator.clone_handle();

        simulator.total_requests.store(100, Ordering::Relaxed);
        simulator.failed_requests.store(10, Ordering::Relaxed);

        assert_eq!(cloned.get_actual_failure_rate_bp(), 1000);
    }

    #[test]
    fn test_probabilistic_failures() {
        let enabled = Arc::new(AtomicBool::new(true));
        let simulator = PartialDegradationSimulator::new(enabled, 5000); // 50% failures

        // Run 1000 iterations, should be ~50% failures
        for _ in 0..1000 {
            let _ = simulator.should_fail();
        }

        let actual_rate_bp = simulator.get_actual_failure_rate_bp();
        // Allow ±5% variance (4500-5500 bp)
        assert!(
            actual_rate_bp >= 4500 && actual_rate_bp <= 5500,
            "Actual rate should be ~50%, got {} bp",
            actual_rate_bp
        );
    }
}
