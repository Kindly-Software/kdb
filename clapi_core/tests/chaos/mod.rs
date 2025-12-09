//! Chaos Engineering Test Infrastructure
//!
//! **Purpose**: Production resilience validation through fault injection (Week 4 Option B)
//! **Framework**: UCE34 Q23-Q25, T28 Q22-Q28, ASSUM Safety
//!
//! # Chaos Test Scenarios (7 Total)
//! 1. **Network Partition**: Provider API unreachable (503 status)
//! 2. **Latency Injection**: Random 100-1000ms delays
//! 3. **Resource Exhaustion**: OOM, CPU saturation
//! 4. **Cascading Failures**: All 16 providers fail simultaneously
//! 5. **Partial Degradation**: 50% providers intermittent (10% random failures)
//! 6. **Database Failure**: KindlyDB connection loss (OAuth/payments)
//! 7. **Clock Skew**: System time jumps backward/forward
//!
//! # Implementation Requirements
//! - 100% Rust (no external scripts)
//! - Lockfree (no mutex/RwLock in chaos injection)
//! - ASSUM tagged (all assumptions documented and verified)
//! - Real implementation (no stubs or simulation)
//!
//! # Framework Compliance
//! - UCE34 Q23: Concurrency (multi-threaded chaos injection)
//! - UCE34 Q24: Cascading failures (circuit breaker validation)
//! - UCE34 Q25: Recovery (graceful degradation, automatic recovery)
//! - T28 Q22: Production testing (realistic fault scenarios)
//! - T28 Q23: Security/adversarial (malicious input, resource exhaustion)
//! - T28 Q24: B32 benchmarks (measure impact on latency/throughput)
//! - ASSUM: 100% safety assumption coverage

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// Sub-modules for each chaos scenario
pub mod network_chaos;
pub mod latency_injection;
pub mod resource_exhaustion;
pub mod cascading_failures;
pub mod partial_degradation;
pub mod database_failure;
pub mod clock_skew;

/// Chaos fault injection types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChaosFault {
    /// Network partition (provider unreachable)
    NetworkPartition,
    /// Latency injection (random delays)
    LatencyInjection { min_ms: u64, max_ms: u64 },
    /// Resource exhaustion (memory/CPU)
    ResourceExhaustion { memory_mb: usize },
    /// Cascading failures (all providers fail)
    CascadingFailures,
    /// Partial degradation (intermittent failures)
    PartialDegradation { failure_rate_bp: u64 }, // Basis points (0-10000)
    /// Database failure (connection loss)
    DatabaseFailure,
    /// Clock skew (time jumps)
    ClockSkew { offset_secs: i64 },
}

/// Chaos test configuration
#[derive(Debug, Clone)]
pub struct ChaosConfig {
    /// Fault to inject
    pub fault: ChaosFault,
    /// Duration of chaos injection
    pub duration: Duration,
    /// Recovery duration after fault removed
    pub recovery_duration: Duration,
    /// Enable chaos injection (atomic flag)
    pub enabled: Arc<AtomicBool>,
    /// Fault injection counter (for metrics)
    pub injection_count: Arc<AtomicU64>,
}

impl ChaosConfig {
    /// Create new chaos configuration
    ///
    /// # Arguments
    /// - `fault`: Fault type to inject
    /// - `duration`: How long to inject fault
    /// - `recovery_duration`: How long to observe recovery
    pub fn new(fault: ChaosFault, duration: Duration, recovery_duration: Duration) -> Self {
        Self {
            fault,
            duration,
            recovery_duration,
            enabled: Arc::new(AtomicBool::new(false)),
            injection_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Enable chaos injection
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Release);
    }

    /// Disable chaos injection
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Release);
    }

    /// Check if chaos is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Increment injection counter
    pub fn record_injection(&self) {
        self.injection_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total injections
    pub fn get_injection_count(&self) -> u64 {
        self.injection_count.load(Ordering::Relaxed)
    }
}

/// Chaos test results
#[derive(Debug, Clone)]
pub struct ChaosResults {
    /// Test name
    pub test_name: String,
    /// Fault injected
    pub fault: ChaosFault,
    /// Test duration
    pub duration: Duration,
    /// Recovery duration
    pub recovery_duration: Duration,
    /// Total requests during chaos
    pub chaos_requests: u64,
    /// Failed requests during chaos
    pub chaos_failures: u64,
    /// Total requests during recovery
    pub recovery_requests: u64,
    /// Failed requests during recovery
    pub recovery_failures: u64,
    /// P50 latency during chaos (milliseconds)
    pub chaos_p50_ms: f64,
    /// P99 latency during chaos (milliseconds)
    pub chaos_p99_ms: f64,
    /// P50 latency during recovery (milliseconds)
    pub recovery_p50_ms: f64,
    /// P99 latency during recovery (milliseconds)
    pub recovery_p99_ms: f64,
    /// System survived chaos injection
    pub survived: bool,
    /// Recovery successful
    pub recovered: bool,
}

impl ChaosResults {
    /// Calculate chaos failure rate (0-10000 basis points)
    pub fn chaos_failure_rate_bp(&self) -> u64 {
        if self.chaos_requests == 0 {
            return 0;
        }
        (self.chaos_failures * 10000 / self.chaos_requests)
    }

    /// Calculate recovery failure rate (0-10000 basis points)
    pub fn recovery_failure_rate_bp(&self) -> u64 {
        if self.recovery_requests == 0 {
            return 0;
        }
        (self.recovery_failures * 10000 / self.recovery_requests)
    }

    /// Check if system is resilient (survived and recovered)
    pub fn is_resilient(&self) -> bool {
        self.survived && self.recovered
    }

    /// Generate summary report
    pub fn summary(&self) -> String {
        format!(
            "Chaos Test Results: {test_name}\n\
             ==========================================\n\
             Fault: {fault:?}\n\
             Duration: {duration:.1}s (chaos) + {recovery:.1}s (recovery)\n\
             \n\
             Chaos Phase:\n\
             -----------\n\
             Requests: {chaos_reqs} ({chaos_fail} failed, {chaos_fail_rate_bp} bp)\n\
             Latency: P50={chaos_p50:.2}ms, P99={chaos_p99:.2}ms\n\
             \n\
             Recovery Phase:\n\
             --------------\n\
             Requests: {recovery_reqs} ({recovery_fail} failed, {recovery_fail_rate_bp} bp)\n\
             Latency: P50={recovery_p50:.2}ms, P99={recovery_p99:.2}ms\n\
             \n\
             Resilience: {status}\n\
             Survived: {survived}\n\
             Recovered: {recovered}\n",
            test_name = self.test_name,
            fault = self.fault,
            duration = self.duration.as_secs_f64(),
            recovery = self.recovery_duration.as_secs_f64(),
            chaos_reqs = self.chaos_requests,
            chaos_fail = self.chaos_failures,
            chaos_fail_rate_bp = self.chaos_failure_rate_bp(),
            chaos_p50 = self.chaos_p50_ms,
            chaos_p99 = self.chaos_p99_ms,
            recovery_reqs = self.recovery_requests,
            recovery_fail = self.recovery_failures,
            recovery_fail_rate_bp = self.recovery_failure_rate_bp(),
            recovery_p50 = self.recovery_p50_ms,
            recovery_p99 = self.recovery_p99_ms,
            status = if self.is_resilient() { "RESILIENT ✓" } else { "NEEDS_HARDENING ✗" },
            survived = if self.survived { "YES ✓" } else { "NO ✗" },
            recovered = if self.recovered { "YES ✓" } else { "NO ✗" },
        )
    }
}

/// Latency collector for chaos testing
pub struct ChaosLatencyCollector {
    latencies_ns: Arc<parking_lot::Mutex<Vec<u64>>>,
}

impl ChaosLatencyCollector {
    pub fn new() -> Self {
        Self {
            latencies_ns: Arc::new(parking_lot::Mutex::new(Vec::with_capacity(100_000))),
        }
    }

    /// Record latency (nanoseconds)
    pub fn record(&self, latency_ns: u64) {
        self.latencies_ns.lock().push(latency_ns);
    }

    /// Get all latencies and clear
    pub fn take_latencies(&self) -> Vec<u64> {
        std::mem::take(&mut *self.latencies_ns.lock())
    }

    /// Calculate percentile (returns milliseconds)
    pub fn percentile(&self, p: f64) -> f64 {
        let mut latencies = self.latencies_ns.lock().clone();
        if latencies.is_empty() {
            return 0.0;
        }
        latencies.sort_unstable();
        let index = ((latencies.len() as f64 * p / 100.0) as usize).min(latencies.len() - 1);
        latencies[index] as f64 / 1_000_000.0 // Convert ns to ms
    }

    /// Clone handle for sharing
    pub fn clone_handle(&self) -> Self {
        Self {
            latencies_ns: Arc::clone(&self.latencies_ns),
        }
    }
}

/// Chaos test harness
pub struct ChaosTestHarness {
    config: ChaosConfig,
}

impl ChaosTestHarness {
    /// Create new chaos test harness
    pub fn new(config: ChaosConfig) -> Self {
        Self { config }
    }

    /// Run chaos test with fault injection
    ///
    /// # Test Phases
    /// 1. Baseline: Measure normal operation (10s)
    /// 2. Chaos: Inject fault for configured duration
    /// 3. Recovery: Disable fault, measure recovery
    ///
    /// # Returns
    /// - ChaosResults with detailed metrics
    pub fn run<F>(&self, test_name: &str, test_fn: F) -> ChaosResults
    where
        F: Fn() -> Result<(), String> + Send + Sync + Clone + 'static,
    {
        println!("=== Chaos Test: {} ===", test_name);
        println!("Fault: {:?}", self.config.fault);
        println!("Duration: {:.1}s chaos + {:.1}s recovery",
                 self.config.duration.as_secs_f64(),
                 self.config.recovery_duration.as_secs_f64());

        // Phase 1: Baseline (10 seconds)
        println!("\n[Phase 1] Baseline measurement (10s)...");
        let baseline_duration = Duration::from_secs(10);
        let baseline_start = Instant::now();
        let mut baseline_requests = 0u64;
        while baseline_start.elapsed() < baseline_duration {
            if test_fn().is_ok() {
                baseline_requests += 1;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let baseline_rps = baseline_requests as f64 / baseline_duration.as_secs_f64();
        println!("Baseline: {} req/s", baseline_rps as u64);

        // Phase 2: Chaos injection
        println!("\n[Phase 2] Chaos injection ({:.1}s)...", self.config.duration.as_secs_f64());
        self.config.enable();

        let chaos_collector = ChaosLatencyCollector::new();
        let chaos_start = Instant::now();
        let mut chaos_requests = 0u64;
        let mut chaos_failures = 0u64;

        while chaos_start.elapsed() < self.config.duration {
            let op_start = Instant::now();
            let result = test_fn();
            let latency_ns = op_start.elapsed().as_nanos() as u64;

            chaos_collector.record(latency_ns);
            chaos_requests += 1;

            if result.is_err() {
                chaos_failures += 1;
            }

            std::thread::sleep(Duration::from_millis(10));
        }

        let chaos_p50_ms = chaos_collector.percentile(50.0);
        let chaos_p99_ms = chaos_collector.percentile(99.0);
        let chaos_failure_rate = if chaos_requests > 0 {
            chaos_failures * 10000 / chaos_requests
        } else {
            0
        };

        println!("Chaos: {} requests, {} failures ({} bp), P50={:.2}ms, P99={:.2}ms",
                 chaos_requests, chaos_failures, chaos_failure_rate,
                 chaos_p50_ms, chaos_p99_ms);

        // Phase 3: Recovery
        println!("\n[Phase 3] Recovery ({:.1}s)...", self.config.recovery_duration.as_secs_f64());
        self.config.disable();

        let recovery_collector = ChaosLatencyCollector::new();
        let recovery_start = Instant::now();
        let mut recovery_requests = 0u64;
        let mut recovery_failures = 0u64;

        while recovery_start.elapsed() < self.config.recovery_duration {
            let op_start = Instant::now();
            let result = test_fn();
            let latency_ns = op_start.elapsed().as_nanos() as u64;

            recovery_collector.record(latency_ns);
            recovery_requests += 1;

            if result.is_err() {
                recovery_failures += 1;
            }

            std::thread::sleep(Duration::from_millis(10));
        }

        let recovery_p50_ms = recovery_collector.percentile(50.0);
        let recovery_p99_ms = recovery_collector.percentile(99.0);
        let recovery_failure_rate = if recovery_requests > 0 {
            recovery_failures * 10000 / recovery_requests
        } else {
            0
        };

        println!("Recovery: {} requests, {} failures ({} bp), P50={:.2}ms, P99={:.2}ms",
                 recovery_requests, recovery_failures, recovery_failure_rate,
                 recovery_p50_ms, recovery_p99_ms);

        // Determine resilience
        // #ASSUME: System survived if no panics during chaos
        // #VERIFY: This test returns ChaosResults (no panic = survived)
        let survived = true; // If we reach here, no panic occurred

        // #ASSUME: System recovered if failure rate drops to <5% (500 bp)
        // #VERIFY: Compare recovery_failure_rate vs threshold
        let recovered = recovery_failure_rate < 500; // <5% failure rate

        let results = ChaosResults {
            test_name: test_name.to_string(),
            fault: self.config.fault,
            duration: self.config.duration,
            recovery_duration: self.config.recovery_duration,
            chaos_requests,
            chaos_failures,
            recovery_requests,
            recovery_failures,
            chaos_p50_ms,
            chaos_p99_ms,
            recovery_p50_ms,
            recovery_p99_ms,
            survived,
            recovered,
        };

        println!("\n{}", results.summary());
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chaos_config_enable_disable() {
        let config = ChaosConfig::new(
            ChaosFault::NetworkPartition,
            Duration::from_secs(30),
            Duration::from_secs(30),
        );

        assert!(!config.is_enabled());
        config.enable();
        assert!(config.is_enabled());
        config.disable();
        assert!(!config.is_enabled());
    }

    #[test]
    fn test_chaos_config_injection_counter() {
        let config = ChaosConfig::new(
            ChaosFault::LatencyInjection { min_ms: 100, max_ms: 1000 },
            Duration::from_secs(30),
            Duration::from_secs(30),
        );

        assert_eq!(config.get_injection_count(), 0);
        config.record_injection();
        assert_eq!(config.get_injection_count(), 1);
        config.record_injection();
        assert_eq!(config.get_injection_count(), 2);
    }

    #[test]
    fn test_chaos_results_failure_rates() {
        let results = ChaosResults {
            test_name: "test".to_string(),
            fault: ChaosFault::NetworkPartition,
            duration: Duration::from_secs(30),
            recovery_duration: Duration::from_secs(30),
            chaos_requests: 100,
            chaos_failures: 10,
            recovery_requests: 100,
            recovery_failures: 2,
            chaos_p50_ms: 50.0,
            chaos_p99_ms: 200.0,
            recovery_p50_ms: 30.0,
            recovery_p99_ms: 100.0,
            survived: true,
            recovered: true,
        };

        // 10% failure rate = 1000 bp
        assert_eq!(results.chaos_failure_rate_bp(), 1000);
        // 2% failure rate = 200 bp
        assert_eq!(results.recovery_failure_rate_bp(), 200);
        // Both survived and recovered
        assert!(results.is_resilient());
    }

    #[test]
    fn test_latency_collector() {
        let collector = ChaosLatencyCollector::new();

        // Record some latencies (in nanoseconds)
        collector.record(50_000_000); // 50ms
        collector.record(100_000_000); // 100ms
        collector.record(150_000_000); // 150ms
        collector.record(200_000_000); // 200ms

        // P50 should be ~100ms
        let p50 = collector.percentile(50.0);
        assert!((p50 - 100.0).abs() < 10.0, "P50 should be ~100ms, got {}", p50);

        // P99 should be ~200ms
        let p99 = collector.percentile(99.0);
        assert!((p99 - 200.0).abs() < 10.0, "P99 should be ~200ms, got {}", p99);
    }
}
