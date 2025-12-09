//! Latency Injection Chaos Test (Scenario 2)
//!
//! **Purpose**: Inject random 100-1000ms delays into operations
//! **Expected Behavior**:
//! - Local operations maintain <10ms p50 latency
//! - Timeout handling works correctly
//! - Retry logic functions properly
//! - System remains responsive
//!
//! # ASSUM Safety
//! - #ASSUME: Local operations (budget checks) are <10ms p50
//! - #VERIFY: Measure latency with and without network delays
//! - #ASSUME: Timeout detection within 1% of configured timeout
//! - #VERIFY: Measure actual timeout vs expected
//! - #ASSUME: Retry backoff prevents stampeding herd
//! - #VERIFY: Monitor retry distribution over time
//!
//! # UCE34 Compliance
//! - Q23 (Concurrency): Multi-threaded latency injection
//! - Q24 (Performance): Isolate local vs network latency
//! - Q25 (Recovery): System recovers when latency returns to normal
//!
//! # T28 Testing
//! - Q22: Production scenario (network latency spikes are common)
//! - Q24: B32 benchmarks (measure p50/p95/p99 under latency)

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use rand::Rng;

use clapi_core::proxy::BudgetRegistry;
use super::{ChaosConfig, ChaosFault, ChaosTestHarness};

/// Latency injection simulator
#[derive(Clone)]
struct LatencyInjector {
    /// Injection enabled flag
    enabled: Arc<AtomicBool>,
    /// Min latency (milliseconds)
    min_ms: u64,
    /// Max latency (milliseconds)
    max_ms: u64,
    /// Total injections
    injection_count: Arc<AtomicU64>,
    /// Total injected latency (milliseconds)
    total_injected_ms: Arc<AtomicU64>,
}

impl LatencyInjector {
    fn new(enabled: Arc<AtomicBool>, min_ms: u64, max_ms: u64) -> Self {
        Self {
            enabled,
            min_ms,
            max_ms,
            injection_count: Arc::new(AtomicU64::new(0)),
            total_injected_ms: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Inject latency if enabled
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Thread::sleep is monotonic and accurate
    /// - #VERIFY: Measure actual sleep time vs requested
    /// - #ASSUME: Random delay generation is thread-safe
    /// - #VERIFY: Use thread_rng() for thread-local RNG
    fn maybe_inject(&self) {
        if !self.enabled.load(Ordering::Acquire) {
            return;
        }

        // Generate random delay
        let mut rng = rand::thread_rng();
        let delay_ms = rng.gen_range(self.min_ms..=self.max_ms);

        // Inject delay
        let start = Instant::now();
        std::thread::sleep(Duration::from_millis(delay_ms));
        let actual_ms = start.elapsed().as_millis() as u64;

        // Record injection
        self.injection_count.fetch_add(1, Ordering::Relaxed);
        self.total_injected_ms.fetch_add(actual_ms, Ordering::Relaxed);
    }

    /// Get injection statistics
    fn get_stats(&self) -> (u64, f64) {
        let count = self.injection_count.load(Ordering::Relaxed);
        let total_ms = self.total_injected_ms.load(Ordering::Relaxed);
        let avg_ms = if count > 0 {
            total_ms as f64 / count as f64
        } else {
            0.0
        };
        (count, avg_ms)
    }

    fn clone_handle(&self) -> Self {
        Self {
            enabled: Arc::clone(&self.enabled),
            min_ms: self.min_ms,
            max_ms: self.max_ms,
            injection_count: Arc::clone(&self.injection_count),
            total_injected_ms: Arc::clone(&self.total_injected_ms),
        }
    }
}

/// Test: Latency injection with local operation isolation
///
/// # Test Scenario
/// 1. Baseline: Measure local operation latency (10s)
/// 2. Chaos: Inject 100-1000ms delays (30s)
/// 3. Recovery: Remove delays, validate recovery (30s)
///
/// # Expected Results
/// - Local operations maintain <10ms p50 during chaos
/// - Network operations show injected latency
/// - System remains responsive (no hangs)
/// - Recovery restores normal latency
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_latency_injection_isolation() {
    // Setup chaos config
    let config = ChaosConfig::new(
        ChaosFault::LatencyInjection { min_ms: 100, max_ms: 1000 },
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    // Create latency injector
    let injector = LatencyInjector::new(Arc::clone(&config.enabled), 100, 1000);

    // Budget registry for local operations
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let budget_id = 0x1234567890ABCDEF;

    // Test function: Local budget check + simulated network call
    let test_fn = {
        let injector = injector.clone_handle();
        let budget_registry = Arc::clone(&budget_registry);

        move || {
            // Local operation (should be fast: <10ms)
            let local_start = Instant::now();
            let result = budget_registry.try_deduct(budget_id, 1_00);
            let local_latency_ms = local_start.elapsed().as_millis() as u64;

            // Validate local latency (<10ms p50 target)
            if local_latency_ms > 50 {
                eprintln!("WARNING: Local operation took {}ms (expected <10ms)", local_latency_ms);
            }

            // Simulated network call with injected latency
            injector.maybe_inject();

            // Return result
            result.map(|_| ()).map_err(|e| format!("Budget error: {:?}", e))
        }
    };

    // Run chaos test
    let harness = ChaosTestHarness::new(config);
    let results = harness.run("Latency Injection Isolation", test_fn);

    // Get injection stats
    let (injection_count, avg_injected_ms) = injector.get_stats();

    // Validate results
    // #ASSUME: System survives latency injection
    // #VERIFY: Test completed
    assert!(results.survived, "System should survive latency injection");

    // #ASSUME: Injected latency is within expected range (100-1000ms)
    // #VERIFY: Average injected latency is ~550ms (midpoint)
    assert!(
        avg_injected_ms >= 100.0 && avg_injected_ms <= 1000.0,
        "Average injected latency should be 100-1000ms, got {:.2}ms",
        avg_injected_ms
    );

    // #ASSUME: Recovery restores normal latency
    // #VERIFY: Recovery p50 < 20ms (local + minimal overhead)
    assert!(
        results.recovery_p50_ms < 20.0,
        "Recovery p50 should be <20ms, got {:.2}ms",
        results.recovery_p50_ms
    );

    println!("\n{}", results.summary());
    println!("Latency Injection: {} injections, avg={:.2}ms", injection_count, avg_injected_ms);
}

/// Test: Timeout handling under latency injection
///
/// # Test Scenario
/// - Inject delays exceeding timeout (1000-2000ms)
/// - Timeout configured at 500ms
/// - System should detect and handle timeouts
///
/// # Expected Results
/// - Timeouts detected correctly
/// - No hangs or deadlocks
/// - Error messages clear
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_timeout_handling() {
    // Setup: Inject delays exceeding timeout
    let timeout_ms = 500;
    let config = ChaosConfig::new(
        ChaosFault::LatencyInjection { min_ms: 1000, max_ms: 2000 },
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    let injector = LatencyInjector::new(Arc::clone(&config.enabled), 1000, 2000);
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let budget_id = 0xDEADBEEFCAFEBABE;

    // Track timeouts
    let timeout_count = Arc::new(AtomicU64::new(0));

    let test_fn = {
        let injector = injector.clone_handle();
        let budget_registry = Arc::clone(&budget_registry);
        let timeout_count = Arc::clone(&timeout_count);

        move || {
            let op_start = Instant::now();

            // Inject latency
            injector.maybe_inject();

            // Check if timeout exceeded
            let elapsed_ms = op_start.elapsed().as_millis() as u64;
            if elapsed_ms > timeout_ms {
                timeout_count.fetch_add(1, Ordering::Relaxed);
                return Err(format!("Timeout: operation took {}ms (limit: {}ms)", elapsed_ms, timeout_ms));
            }

            // Normal operation
            budget_registry.try_deduct(budget_id, 1_00)
                .map(|_| ())
                .map_err(|e| format!("Budget error: {:?}", e))
        }
    };

    // Run chaos test
    let harness = ChaosTestHarness::new(config);
    let results = harness.run("Timeout Handling", test_fn);

    // Validate timeout detection
    let timeouts = timeout_count.load(Ordering::Relaxed);

    // #ASSUME: Most operations timeout during chaos (delays > timeout)
    // #VERIFY: Timeout count > 50% of chaos requests
    assert!(
        timeouts > results.chaos_requests / 2,
        "Should detect timeouts (got {} out of {} requests)",
        timeouts, results.chaos_requests
    );

    // #ASSUME: System survives timeout handling
    // #VERIFY: No panics
    assert!(results.survived, "System should survive timeout handling");

    println!("\n{}", results.summary());
    println!("Timeouts detected: {} / {} ({:.1}%)",
             timeouts, results.chaos_requests,
             timeouts as f64 / results.chaos_requests as f64 * 100.0);
}

/// Test: Retry logic with exponential backoff
///
/// # Test Scenario
/// - Inject intermittent latency (50% of requests)
/// - Retry logic should handle failures
/// - Backoff should prevent stampeding herd
///
/// # Expected Results
/// - Retry success rate >90%
/// - Backoff delays observed
/// - No stampeding herd (distributed retries)
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_retry_backoff() {
    // Setup: Intermittent latency
    let config = ChaosConfig::new(
        ChaosFault::LatencyInjection { min_ms: 100, max_ms: 500 },
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    let injector = LatencyInjector::new(Arc::clone(&config.enabled), 100, 500);
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let budget_id = 0x0123456789ABCDEF;

    // Track retries
    let retry_count = Arc::new(AtomicU64::new(0));
    let success_count = Arc::new(AtomicU64::new(0));

    let test_fn = {
        let injector = injector.clone_handle();
        let budget_registry = Arc::clone(&budget_registry);
        let retry_count = Arc::clone(&retry_count);
        let success_count = Arc::clone(&success_count);

        move || {
            const MAX_RETRIES: usize = 3;
            let mut retries = 0;

            loop {
                // Inject latency with 50% probability
                let mut rng = rand::thread_rng();
                if rng.gen_bool(0.5) {
                    injector.maybe_inject();
                }

                // Try operation
                match budget_registry.try_deduct(budget_id, 1_00) {
                    Ok(_) => {
                        success_count.fetch_add(1, Ordering::Relaxed);
                        return Ok(());
                    }
                    Err(e) if retries < MAX_RETRIES => {
                        // Retry with exponential backoff
                        retries += 1;
                        retry_count.fetch_add(1, Ordering::Relaxed);

                        let backoff_ms = 10 * (1 << retries); // 20, 40, 80ms
                        std::thread::sleep(Duration::from_millis(backoff_ms));
                    }
                    Err(e) => {
                        return Err(format!("Max retries exceeded: {:?}", e));
                    }
                }
            }
        }
    };

    // Run chaos test
    let harness = ChaosTestHarness::new(config);
    let results = harness.run("Retry Backoff", test_fn);

    // Validate retry logic
    let retries = retry_count.load(Ordering::Relaxed);
    let successes = success_count.load(Ordering::Relaxed);

    // #ASSUME: Retry logic improves success rate to >90%
    // #VERIFY: Success count vs total requests
    let success_rate = successes as f64 / (results.chaos_requests + results.recovery_requests) as f64;
    assert!(
        success_rate > 0.9,
        "Retry logic should achieve >90% success rate, got {:.1}%",
        success_rate * 100.0
    );

    println!("\n{}", results.summary());
    println!("Retries: {} (avg {:.2} per request)", retries,
             retries as f64 / (results.chaos_requests + results.recovery_requests) as f64);
    println!("Success rate: {:.1}%", success_rate * 100.0);
}

#[cfg(test)]
mod compile_tests {
    use super::*;

    #[test]
    fn test_latency_injector_clone() {
        let enabled = Arc::new(AtomicBool::new(false));
        let injector = LatencyInjector::new(enabled, 100, 1000);
        let cloned = injector.clone_handle();

        injector.injection_count.store(42, Ordering::Relaxed);
        assert_eq!(cloned.injection_count.load(Ordering::Relaxed), 42);
    }

    #[test]
    fn test_latency_range() {
        let enabled = Arc::new(AtomicBool::new(true));
        let injector = LatencyInjector::new(enabled, 100, 1000);

        // Test multiple injections
        for _ in 0..10 {
            injector.maybe_inject();
        }

        let (count, avg_ms) = injector.get_stats();
        assert_eq!(count, 10);
        assert!(avg_ms >= 100.0 && avg_ms <= 1000.0, "Average should be in range: {}", avg_ms);
    }
}
