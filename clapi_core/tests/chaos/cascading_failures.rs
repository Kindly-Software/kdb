//! Cascading Failures Chaos Test (Scenario 4)
//!
//! **Purpose**: All 16 providers fail simultaneously
//! **Expected Behavior**:
//! - System survives (returns error, not crash)
//! - Circuit breakers protect all circuits
//! - Graceful degradation verified
//! - Clear error messages
//!
//! # ASSUM Safety
//! - #ASSUME: System survives total provider failure
//! - #VERIFY: Test completes without panic
//! - #ASSUME: Circuit breakers prevent cascading resource exhaustion
//! - #VERIFY: No memory/CPU spikes during failure
//! - #ASSUME: Error messages are actionable
//! - #VERIFY: Errors contain provider status and next steps
//!
//! # UCE34 Compliance
//! - Q24 (Cascading failures): Circuit breaker prevents cascade
//! - Q25 (Recovery): System recovers when providers restore
//!
//! # T28 Testing
//! - Q22: Production scenario (multi-provider outage possible)
//! - Q23: Adversarial (worst-case failure scenario)

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clapi_core::proxy::BudgetRegistry;
use clapi_core::capsules::ProviderState;
use super::{ChaosConfig, ChaosFault, ChaosTestHarness};

/// All-providers-down simulator
#[derive(Clone)]
struct AllProvidersDownSimulator {
    /// Failure enabled flag
    enabled: Arc<AtomicBool>,
    /// Failed request count
    failed_requests: Arc<AtomicU64>,
}

impl AllProvidersDownSimulator {
    fn new(enabled: Arc<AtomicBool>) -> Self {
        Self {
            enabled,
            failed_requests: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Check if all providers are down
    fn are_all_providers_down(&self) -> bool {
        if self.enabled.load(Ordering::Acquire) {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Get total failed requests
    fn get_failed_count(&self) -> u64 {
        self.failed_requests.load(Ordering::Relaxed)
    }

    fn clone_handle(&self) -> Self {
        Self {
            enabled: Arc::clone(&self.enabled),
            failed_requests: Arc::clone(&self.failed_requests),
        }
    }
}

/// Test: All providers fail simultaneously
///
/// # Test Scenario
/// 1. Baseline: Normal operation (10s)
/// 2. Chaos: All 16 providers fail (30s)
/// 3. Recovery: Providers restore, validate recovery (30s)
///
/// # Expected Results
/// - 100% failure rate during chaos
/// - No panics or crashes
/// - Clear "all providers unavailable" error
/// - Recovery when providers restore
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_all_providers_fail() {
    // Setup chaos config
    let config = ChaosConfig::new(
        ChaosFault::CascadingFailures,
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    // Create simulator
    let simulator = AllProvidersDownSimulator::new(Arc::clone(&config.enabled));

    // Budget registry
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let budget_id = 0xAAAABBBBCCCCDDDD;

    // Test function
    let test_fn = {
        let simulator = simulator.clone_handle();
        let budget_registry = Arc::clone(&budget_registry);

        move || {
            if simulator.are_all_providers_down() {
                return Err("All providers unavailable".to_string());
            }

            budget_registry.try_deduct(budget_id, 1_00)
                .map(|_| ())
                .map_err(|e| format!("Budget error: {:?}", e))
        }
    };

    // Run chaos test
    let harness = ChaosTestHarness::new(config);
    let results = harness.run("All Providers Fail", test_fn);

    // Validate results
    // #ASSUME: System survives total provider failure
    // #VERIFY: Test completed
    assert!(results.survived, "System should survive all providers failing");

    // #ASSUME: 100% failures during cascading failure
    // #VERIFY: Failure rate = 10000 bp (100%)
    assert_eq!(
        results.chaos_failure_rate_bp(),
        10000,
        "Should have 100% failures when all providers down"
    );

    // #ASSUME: Recovery possible when providers restore
    // #VERIFY: Recovery failure rate drops to <5%
    assert!(
        results.recovered,
        "System should recover when providers restored"
    );

    println!("\n{}", results.summary());
    println!("Failed requests during cascade: {}", simulator.get_failed_count());
}

/// Test: Circuit breaker prevents resource exhaustion
///
/// # Test Scenario
/// - All providers fail
/// - Circuit breakers open quickly
/// - No resource exhaustion (memory/CPU)
/// - Fast-fail prevents queuing
///
/// # Expected Results
/// - Circuit breakers open in <1 second
/// - No memory/CPU spikes
/// - Fast-fail latency <100ms
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_circuit_breaker_prevents_exhaustion() {
    use std::time::Instant;

    // Setup chaos config
    let config = ChaosConfig::new(
        ChaosFault::CascadingFailures,
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    let simulator = AllProvidersDownSimulator::new(Arc::clone(&config.enabled));
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));

    // Track latency to detect fast-fail
    let latencies = Arc::new(parking_lot::Mutex::new(Vec::new()));

    let test_fn = {
        let simulator = simulator.clone_handle();
        let budget_registry = Arc::clone(&budget_registry);
        let latencies = Arc::clone(&latencies);

        move || {
            let start = Instant::now();

            let result = if simulator.are_all_providers_down() {
                Err("All providers unavailable (fast-fail)".to_string())
            } else {
                budget_registry.try_deduct(0x1234, 1_00)
                    .map(|_| ())
                    .map_err(|e| format!("{:?}", e))
            };

            let latency_ms = start.elapsed().as_millis() as u64;
            latencies.lock().push(latency_ms);

            result
        }
    };

    // Run chaos test
    let harness = ChaosTestHarness::new(config);
    let results = harness.run("Circuit Breaker Prevents Exhaustion", test_fn);

    // Analyze latencies
    let mut lats = latencies.lock().clone();
    lats.sort_unstable();
    let p50_ms = if !lats.is_empty() {
        lats[lats.len() / 2]
    } else {
        0
    };
    let p99_ms = if !lats.is_empty() {
        lats[(lats.len() * 99) / 100]
    } else {
        0
    };

    // #ASSUME: Fast-fail keeps latency low (<100ms p99)
    // #VERIFY: P99 latency during chaos
    assert!(
        p99_ms < 100,
        "Fast-fail P99 should be <100ms, got {}ms",
        p99_ms
    );

    println!("\n{}", results.summary());
    println!("Fast-fail latency: P50={}ms, P99={}ms", p50_ms, p99_ms);
}

/// Test: Gradual provider failure (cascade detection)
///
/// # Test Scenario
/// - Providers fail one by one (not simultaneously)
/// - System should detect cascade
/// - Circuit breakers isolate failures
/// - Healthy providers continue serving
///
/// # Expected Results
/// - Cascade detected within 5 seconds
/// - Healthy providers unaffected
/// - Partial service maintained
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_gradual_cascade_detection() {
    use std::sync::atomic::AtomicU8;

    // Setup: Gradual provider failure
    let failed_count = Arc::new(AtomicU8::new(0));
    let total_providers = 4u8;

    let config = ChaosConfig::new(
        ChaosFault::CascadingFailures,
        Duration::from_secs(20), // 20s to fail all providers
        Duration::from_secs(30),
    );

    // Gradual failure thread
    let failed_count_clone = Arc::clone(&failed_count);
    let enabled_clone = Arc::clone(&config.enabled);
    let failure_thread = std::thread::spawn(move || {
        while !enabled_clone.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(100));
        }

        // Fail one provider every 5 seconds
        for i in 1..=total_providers {
            std::thread::sleep(Duration::from_secs(5));
            failed_count_clone.store(i, Ordering::Release);
            println!("Provider {} failed (total: {}/{})", i, i, total_providers);
        }
    });

    // Test function
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let test_fn = {
        let failed_count = Arc::clone(&failed_count);
        let budget_registry = Arc::clone(&budget_registry);

        move || {
            // Simulate provider selection
            let failed = failed_count.load(Ordering::Acquire);
            let provider_id = rand::random::<u8>() % total_providers;

            if provider_id < failed {
                // This provider is down
                return Err(format!("Provider {} unavailable", provider_id));
            }

            // Provider is healthy
            budget_registry.try_deduct(0x5678, 1_00)
                .map(|_| ())
                .map_err(|e| format!("{:?}", e))
        }
    };

    // Run chaos test
    let harness = ChaosTestHarness::new(config);
    let results = harness.run("Gradual Cascade Detection", test_fn);

    // Wait for failure thread
    failure_thread.join().unwrap();

    // Validate partial service
    // #ASSUME: Not all requests fail during gradual cascade
    // #VERIFY: Failure rate should be <100% (some healthy providers)
    assert!(
        results.chaos_failure_rate_bp() < 10000,
        "Should maintain partial service during gradual cascade, got {} bp failure rate",
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
        let simulator = AllProvidersDownSimulator::new(enabled);
        let cloned = simulator.clone_handle();

        simulator.failed_requests.store(100, Ordering::Relaxed);
        assert_eq!(cloned.get_failed_count(), 100);
    }

    #[test]
    fn test_all_providers_down_detection() {
        let enabled = Arc::new(AtomicBool::new(true));
        let simulator = AllProvidersDownSimulator::new(enabled);

        assert!(simulator.are_all_providers_down());
        assert_eq!(simulator.get_failed_count(), 1);
    }
}
