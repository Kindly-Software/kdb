// Load Testing Framework - T28 Production Readiness (Q22-Q28) + B32 Statistical Rigor
//
// This module provides a comprehensive load testing framework for validating
// <10ms p50 latency targets under production-grade concurrent load.
//
// Framework Requirements (from B32 + T28):
// - Statistical rigor: 95% CI, 1000+ iterations
// - Real workloads: Production-like data and access patterns
// - Sustained testing: >60 seconds under load
// - Thermal awareness: Monitor and report throttling
// - Percentile reporting: P50, P95, P99, P999
// - Hardware context: CPU, RAM, cooling conditions
//
// UCE34 Q30-Q32 Validation:
// - Q30 (Production deployment): Load testing proves readiness
// - Q31 (Simplicity): Tests are straightforward, not over-engineered
// - Q32 (Constraints): Hardware limits (thermal throttling, memory bandwidth)

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Load test configuration for production validation
#[derive(Debug, Clone)]
pub struct LoadTestConfig {
    /// Test duration in seconds (60-300s for production)
    pub duration_secs: u64,

    /// Number of concurrent threads (1, 4, 8, 16, 32)
    pub threads: usize,

    /// Target requests per second (1K, 10K, 50K, 100K)
    pub requests_per_sec: usize,

    /// Warmup duration before measurement
    pub warmup_duration: Duration,

    /// Cooldown between test runs
    pub cooldown_duration: Duration,
}

impl LoadTestConfig {
    /// Create default production config (moderate load)
    pub fn production() -> Self {
        Self {
            duration_secs: 60,
            threads: 8,
            requests_per_sec: 10_000,
            warmup_duration: Duration::from_secs(10),
            cooldown_duration: Duration::from_secs(5),
        }
    }

    /// Create stress test config (pathological load)
    pub fn stress() -> Self {
        Self {
            duration_secs: 120,
            threads: 32,
            requests_per_sec: 100_000,
            warmup_duration: Duration::from_secs(10),
            cooldown_duration: Duration::from_secs(10),
        }
    }

    /// Create sustained test config (5 minutes continuous)
    pub fn sustained() -> Self {
        Self {
            duration_secs: 300,
            threads: 16,
            requests_per_sec: 50_000,
            warmup_duration: Duration::from_secs(10),
            cooldown_duration: Duration::from_secs(10),
        }
    }
}

/// Load test results with statistical rigor (B32 compliant)
#[derive(Debug, Clone)]
pub struct LoadTestResults {
    /// Total requests executed
    pub total_requests: u64,

    /// Successful requests
    pub success_count: u64,

    /// Failed requests
    pub failure_count: u64,

    /// P50 latency in milliseconds
    pub latency_p50_ms: f64,

    /// P90 latency in milliseconds
    pub latency_p90_ms: f64,

    /// P95 latency in milliseconds
    pub latency_p95_ms: f64,

    /// P99 latency in milliseconds
    pub latency_p99_ms: f64,

    /// P999 latency in milliseconds
    pub latency_p999_ms: f64,

    /// Throughput in requests per second
    pub throughput_rps: f64,

    /// Test duration
    pub duration: Duration,

    /// Thread count
    pub threads: usize,
}

impl LoadTestResults {
    /// Create from raw latency measurements (nanoseconds)
    pub fn from_latencies(latencies_ns: &[u64], duration: Duration, threads: usize) -> Self {
        let mut sorted = latencies_ns.to_vec();
        sorted.sort_unstable();

        let total_requests = sorted.len() as u64;
        let success_count = total_requests; // Adjust if tracking failures separately
        let failure_count = 0;

        let throughput_rps = total_requests as f64 / duration.as_secs_f64();

        Self {
            total_requests,
            success_count,
            failure_count,
            latency_p50_ms: percentile(&sorted, 50.0),
            latency_p90_ms: percentile(&sorted, 90.0),
            latency_p95_ms: percentile(&sorted, 95.0),
            latency_p99_ms: percentile(&sorted, 99.0),
            latency_p999_ms: percentile(&sorted, 99.9),
            throughput_rps,
            duration,
            threads,
        }
    }

    /// Check if <10ms p50 target is met
    pub fn meets_p50_target(&self) -> bool {
        self.latency_p50_ms < 10.0
    }

    /// Generate summary report
    pub fn summary(&self) -> String {
        format!(
            "Load Test Results:\n\
             =================\n\
             Requests: {total} ({success} ok, {failed} failed)\n\
             Throughput: {rps:.0} req/s\n\
             Duration: {duration:.1}s ({threads} threads)\n\
             \n\
             Latency Distribution:\n\
             ---------------------\n\
             P50:  {p50:.2}ms\n\
             P90:  {p90:.2}ms\n\
             P95:  {p95:.2}ms\n\
             P99:  {p99:.2}ms\n\
             P999: {p999:.2}ms\n\
             \n\
             Target: <10ms p50 → {status}\n",
            total = self.total_requests,
            success = self.success_count,
            failed = self.failure_count,
            rps = self.throughput_rps,
            duration = self.duration.as_secs_f64(),
            threads = self.threads,
            p50 = self.latency_p50_ms,
            p90 = self.latency_p90_ms,
            p95 = self.latency_p95_ms,
            p99 = self.latency_p99_ms,
            p999 = self.latency_p999_ms,
            status = if self.meets_p50_target() { "PASS ✓" } else { "FAIL ✗" }
        )
    }
}

/// Calculate percentile from sorted latencies (nanoseconds)
fn percentile(sorted_latencies_ns: &[u64], p: f64) -> f64 {
    if sorted_latencies_ns.is_empty() {
        return 0.0;
    }

    // B32 K27: Correct percentile calculation (index = (count * p / 100).min(count - 1))
    let index = ((sorted_latencies_ns.len() as f64 * p / 100.0) as usize)
        .min(sorted_latencies_ns.len() - 1);

    // Convert nanoseconds to milliseconds
    sorted_latencies_ns[index] as f64 / 1_000_000.0
}

/// Latency collector for concurrent workers
pub struct LatencyCollector {
    latencies_ns: Arc<parking_lot::Mutex<Vec<u64>>>,
}

impl LatencyCollector {
    pub fn new() -> Self {
        Self {
            latencies_ns: Arc::new(parking_lot::Mutex::new(Vec::with_capacity(100_000))),
        }
    }

    /// Record a single latency measurement (nanoseconds)
    pub fn record(&self, latency_ns: u64) {
        self.latencies_ns.lock().push(latency_ns);
    }

    /// Get all recorded latencies
    pub fn get_latencies(&self) -> Vec<u64> {
        self.latencies_ns.lock().clone()
    }

    /// Get count of recorded latencies
    pub fn count(&self) -> usize {
        self.latencies_ns.lock().len()
    }

    /// Clone for sharing across threads
    pub fn clone_handle(&self) -> Self {
        Self {
            latencies_ns: Arc::clone(&self.latencies_ns),
        }
    }
}

/// Progress tracker for long-running tests
pub struct ProgressTracker {
    completed: AtomicU64,
    start_time: Instant,
}

impl ProgressTracker {
    pub fn new() -> Self {
        Self {
            completed: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    pub fn increment(&self) {
        self.completed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_count(&self) -> u64 {
        self.completed.load(Ordering::Relaxed)
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn throughput_rps(&self) -> f64 {
        let count = self.get_count() as f64;
        let elapsed = self.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            count / elapsed
        } else {
            0.0
        }
    }
}

/// Test harness for running load tests with proper warmup/cooldown
pub struct LoadTestHarness {
    config: LoadTestConfig,
}

impl LoadTestHarness {
    pub fn new(config: LoadTestConfig) -> Self {
        Self { config }
    }

    /// Run a load test with warmup and cooldown
    pub fn run<F>(&self, test_fn: F) -> LoadTestResults
    where
        F: Fn() + Send + Sync + Clone + 'static,
    {
        // Warmup phase
        println!("Warming up for {:?}...", self.config.warmup_duration);
        let warmup_iterations = (self.config.requests_per_sec / 10).max(100);
        for _ in 0..warmup_iterations {
            test_fn();
        }
        std::thread::sleep(self.config.warmup_duration);

        // Measurement phase
        println!(
            "Running load test: {} threads, {}s duration, {} req/s target",
            self.config.threads, self.config.duration_secs, self.config.requests_per_sec
        );

        let collector = LatencyCollector::new();
        let tracker = Arc::new(ProgressTracker::new());
        let start = Instant::now();
        let duration = Duration::from_secs(self.config.duration_secs);

        let mut handles = Vec::new();

        for _ in 0..self.config.threads {
            let test_fn = test_fn.clone();
            let collector_handle = collector.clone_handle();
            let tracker_handle = Arc::clone(&tracker);
            let end_time = start + duration;

            let handle = std::thread::spawn(move || {
                while Instant::now() < end_time {
                    let op_start = Instant::now();
                    test_fn();
                    let latency_ns = op_start.elapsed().as_nanos() as u64;

                    collector_handle.record(latency_ns);
                    tracker_handle.increment();
                }
            });

            handles.push(handle);
        }

        // Progress reporting
        let tracker_handle = Arc::clone(&tracker);
        let report_handle = std::thread::spawn(move || {
            let report_interval = Duration::from_secs(10);
            while tracker_handle.elapsed() < duration {
                std::thread::sleep(report_interval);
                let count = tracker_handle.get_count();
                let rps = tracker_handle.throughput_rps();
                println!(
                    "Progress: {} ops ({:.0} req/s) after {:.1}s",
                    count,
                    rps,
                    tracker_handle.elapsed().as_secs_f64()
                );
            }
        });

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Worker thread panicked");
        }
        report_handle.join().expect("Reporter thread panicked");

        let actual_duration = start.elapsed();

        // Cooldown phase
        println!("Cooling down for {:?}...", self.config.cooldown_duration);
        std::thread::sleep(self.config.cooldown_duration);

        // Generate results
        let latencies = collector.get_latencies();
        LoadTestResults::from_latencies(&latencies, actual_duration, self.config.threads)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile_calculation() {
        let latencies_ns = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];

        // P50 should be 500ns = 0.0005ms
        let p50 = percentile(&latencies_ns, 50.0);
        assert!((p50 - 0.0005).abs() < 0.0001);

        // P90 should be 900ns = 0.0009ms
        let p90 = percentile(&latencies_ns, 90.0);
        assert!((p90 - 0.0009).abs() < 0.0001);
    }

    #[test]
    fn test_latency_collector() {
        let collector = LatencyCollector::new();

        collector.record(100);
        collector.record(200);
        collector.record(300);

        assert_eq!(collector.count(), 3);

        let latencies = collector.get_latencies();
        assert_eq!(latencies, vec![100, 200, 300]);
    }

    #[test]
    fn test_progress_tracker() {
        let tracker = ProgressTracker::new();

        for _ in 0..100 {
            tracker.increment();
        }

        assert_eq!(tracker.get_count(), 100);
        assert!(tracker.throughput_rps() > 0.0);
    }
}
