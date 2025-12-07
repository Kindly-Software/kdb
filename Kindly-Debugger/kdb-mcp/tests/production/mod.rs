// Production Testing Module (T28 Q22-Q28)
// Comprehensive production validation for kdb_mcp
//
// Test Categories:
// - Q22: Stress Tests (10 tests) - stress_tests.rs
// - Q23: Soak Tests (6 tests) - soak_tests.rs
// - Q24: Chaos Tests (9 tests - existing) - ../chaos/
// - Q25: Real-World Scenarios (10 tests) - real_world_scenarios.rs
// - Q26: Performance Regression (10 tests) - performance_regression.rs
// - Q27: Compliance Validation (9 tests) - compliance_tests.rs
// - Q28: Monitoring Tests (10 tests) - monitoring_tests.rs
// - Load Framework (5 configurable tests) - load_framework.rs
//
// Total: 60+ production tests + chaos framework

pub mod stress_tests;
pub mod soak_tests;
pub mod real_world_scenarios;
pub mod performance_regression;
pub mod compliance_tests;
pub mod monitoring_tests;
pub mod load_framework;

// Common test utilities
pub mod common {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    pub struct LoadMetrics {
        pub requests_sent: AtomicU64,
        pub requests_succeeded: AtomicU64,
        pub requests_failed: AtomicU64,
        pub total_latency_ns: AtomicU64,
        pub min_latency_ns: AtomicU64,
        pub max_latency_ns: AtomicU64,
    }

    impl LoadMetrics {
        pub fn new() -> Self {
            Self {
                requests_sent: AtomicU64::new(0),
                requests_succeeded: AtomicU64::new(0),
                requests_failed: AtomicU64::new(0),
                total_latency_ns: AtomicU64::new(0),
                min_latency_ns: AtomicU64::new(u64::MAX),
                max_latency_ns: AtomicU64::new(0),
            }
        }

        pub fn record_request(&self, latency_ns: u64, success: bool) {
            self.requests_sent.fetch_add(1, Ordering::Relaxed);
            if success {
                self.requests_succeeded.fetch_add(1, Ordering::Relaxed);
            } else {
                self.requests_failed.fetch_add(1, Ordering::Relaxed);
            }

            self.total_latency_ns.fetch_add(latency_ns, Ordering::Relaxed);

            // Update min (with CAS loop)
            let mut current_min = self.min_latency_ns.load(Ordering::Relaxed);
            while latency_ns < current_min {
                match self.min_latency_ns.compare_exchange_weak(
                    current_min,
                    latency_ns,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(x) => current_min = x,
                }
            }

            // Update max (with CAS loop)
            let mut current_max = self.max_latency_ns.load(Ordering::Relaxed);
            while latency_ns > current_max {
                match self.max_latency_ns.compare_exchange_weak(
                    current_max,
                    latency_ns,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(x) => current_max = x,
                }
            }
        }

        pub fn get_stats(&self) -> LoadStats {
            let sent = self.requests_sent.load(Ordering::Relaxed);
            let succeeded = self.requests_succeeded.load(Ordering::Relaxed);
            let failed = self.requests_failed.load(Ordering::Relaxed);
            let total_latency = self.total_latency_ns.load(Ordering::Relaxed);
            let min_latency = self.min_latency_ns.load(Ordering::Relaxed);
            let max_latency = self.max_latency_ns.load(Ordering::Relaxed);

            LoadStats {
                requests_sent: sent,
                requests_succeeded: succeeded,
                requests_failed: failed,
                average_latency_ns: if sent > 0 { total_latency / sent } else { 0 },
                min_latency_ns: if min_latency == u64::MAX { 0 } else { min_latency },
                max_latency_ns: max_latency,
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct LoadStats {
        pub requests_sent: u64,
        pub requests_succeeded: u64,
        pub requests_failed: u64,
        pub average_latency_ns: u64,
        pub min_latency_ns: u64,
        pub max_latency_ns: u64,
    }

    impl LoadStats {
        pub fn success_rate(&self) -> f64 {
            if self.requests_sent == 0 {
                0.0
            } else {
                (self.requests_succeeded as f64 / self.requests_sent as f64) * 100.0
            }
        }

        pub fn average_latency_us(&self) -> f64 {
            self.average_latency_ns as f64 / 1000.0
        }

        pub fn min_latency_us(&self) -> f64 {
            self.min_latency_ns as f64 / 1000.0
        }

        pub fn max_latency_us(&self) -> f64 {
            self.max_latency_ns as f64 / 1000.0
        }
    }

    /// Sleep for duration, respecting test timeout
    pub fn sleep_with_timeout(duration: Duration, max_duration: Duration) {
        let sleep_duration = duration.min(max_duration);
        std::thread::sleep(sleep_duration);
    }
}
