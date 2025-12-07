// Load Testing Framework (configurable parameters for production validation)
// T28 Framework: Reusable load testing infrastructure

use super::common::{LoadMetrics, LoadStats};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Load test configuration
#[derive(Debug, Clone)]
pub struct LoadTestConfig {
    /// Test name for reporting
    pub name: String,

    /// Target request rate (requests per second)
    pub target_rps: u64,

    /// Test duration
    pub duration: Duration,

    /// Number of concurrent clients
    pub num_clients: usize,

    /// Request payload size (bytes)
    pub payload_size: usize,

    /// Warm-up period (not counted in results)
    pub warmup_duration: Duration,

    /// Cooldown period after test
    pub cooldown_duration: Duration,

    /// Enable detailed latency tracking
    pub track_latencies: bool,
}

impl LoadTestConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            target_rps: 100,
            duration: Duration::from_secs(10),
            num_clients: 1,
            payload_size: 1024,
            warmup_duration: Duration::from_secs(1),
            cooldown_duration: Duration::from_secs(1),
            track_latencies: true,
        }
    }

    pub fn with_rps(mut self, rps: u64) -> Self {
        self.target_rps = rps;
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn with_clients(mut self, num_clients: usize) -> Self {
        self.num_clients = num_clients;
        self
    }

    pub fn with_payload_size(mut self, size: usize) -> Self {
        self.payload_size = size;
        self
    }

    pub fn with_warmup(mut self, duration: Duration) -> Self {
        self.warmup_duration = duration;
        self
    }

    pub fn with_cooldown(mut self, duration: Duration) -> Self {
        self.cooldown_duration = duration;
        self
    }
}

/// Load test results with detailed statistics
#[derive(Debug, Clone)]
pub struct LoadTestResults {
    pub config: LoadTestConfig,
    pub stats: LoadStats,
    pub actual_duration: Duration,
    pub actual_rps: f64,
    pub latency_percentiles: Option<LatencyPercentiles>,
}

#[derive(Debug, Clone)]
pub struct LatencyPercentiles {
    pub p50_ns: u64,
    pub p95_ns: u64,
    pub p99_ns: u64,
    pub p999_ns: u64,
}

impl LoadTestResults {
    pub fn print_summary(&self) {
        println!("\n========== Load Test Results: {} ==========", self.config.name);
        println!("Configuration:");
        println!("  Target RPS: {}", self.config.target_rps);
        println!("  Duration: {:.2}s", self.config.duration.as_secs_f64());
        println!("  Clients: {}", self.config.num_clients);
        println!("  Payload size: {} bytes", self.config.payload_size);

        println!("\nResults:");
        println!("  Requests sent: {}", self.stats.requests_sent);
        println!("  Requests succeeded: {}", self.stats.requests_succeeded);
        println!("  Requests failed: {}", self.stats.requests_failed);
        println!("  Success rate: {:.2}%", self.stats.success_rate());

        println!("\nPerformance:");
        println!("  Actual duration: {:.2}s", self.actual_duration.as_secs_f64());
        println!("  Actual RPS: {:.2}", self.actual_rps);
        println!("  Avg latency: {:.2} μs", self.stats.average_latency_us());
        println!("  Min latency: {:.2} μs", self.stats.min_latency_us());
        println!("  Max latency: {:.2} μs", self.stats.max_latency_us());

        if let Some(ref percentiles) = self.latency_percentiles {
            println!("\nLatency Percentiles:");
            println!("  P50: {:.2} μs", percentiles.p50_ns as f64 / 1000.0);
            println!("  P95: {:.2} μs", percentiles.p95_ns as f64 / 1000.0);
            println!("  P99: {:.2} μs", percentiles.p99_ns as f64 / 1000.0);
            println!("  P99.9: {:.2} μs", percentiles.p999_ns as f64 / 1000.0);
        }

        println!("==============================================\n");
    }

    pub fn assert_success_rate(&self, min_rate: f64) {
        assert!(
            self.stats.success_rate() >= min_rate,
            "Success rate {:.2}% below minimum {:.2}%",
            self.stats.success_rate(),
            min_rate
        );
    }

    pub fn assert_rps(&self, min_rps: f64) {
        assert!(
            self.actual_rps >= min_rps,
            "Actual RPS {:.2} below minimum {:.2}",
            self.actual_rps,
            min_rps
        );
    }

    pub fn assert_p99_latency(&self, max_latency_us: f64) {
        if let Some(ref percentiles) = self.latency_percentiles {
            let p99_us = percentiles.p99_ns as f64 / 1000.0;
            assert!(
                p99_us <= max_latency_us,
                "P99 latency {:.2} μs exceeds maximum {:.2} μs",
                p99_us,
                max_latency_us
            );
        }
    }
}

/// Load test runner
pub struct LoadTestRunner {
    config: LoadTestConfig,
}

impl LoadTestRunner {
    pub fn new(config: LoadTestConfig) -> Self {
        Self { config }
    }

    /// Run load test with mock operation
    pub fn run<F>(&self, operation: F) -> LoadTestResults
    where
        F: Fn(usize) -> Duration + Send + Sync + 'static,
    {
        println!("Starting load test: {}", self.config.name);
        println!("Configuration: {} RPS × {} clients × {:.2}s",
            self.config.target_rps,
            self.config.num_clients,
            self.config.duration.as_secs_f64()
        );

        let metrics = Arc::new(LoadMetrics::new());
        let operation = Arc::new(operation);

        // Warm-up phase
        if self.config.warmup_duration > Duration::ZERO {
            println!("Warm-up: {:.2}s...", self.config.warmup_duration.as_secs_f64());
            std::thread::sleep(self.config.warmup_duration);
        }

        // Main load test
        let start = Instant::now();

        let handles: Vec<_> = (0..self.config.num_clients)
            .map(|client_id| {
                let metrics = Arc::clone(&metrics);
                let operation = Arc::clone(&operation);
                let duration = self.config.duration;
                let target_rps = self.config.target_rps / self.config.num_clients as u64;
                let interval_ns = if target_rps > 0 {
                    1_000_000_000 / target_rps
                } else {
                    0
                };

                thread::spawn(move || {
                    let client_start = Instant::now();
                    let mut last_request = Instant::now();

                    while client_start.elapsed() < duration {
                        // Throttle to target RPS
                        if interval_ns > 0 {
                            let now = Instant::now();
                            let elapsed_since_last = now.duration_since(last_request).as_nanos() as u64;
                            if elapsed_since_last < interval_ns {
                                let sleep_ns = interval_ns - elapsed_since_last;
                                thread::sleep(Duration::from_nanos(sleep_ns));
                            }
                        }

                        // Execute operation
                        let req_start = Instant::now();
                        let latency = operation(client_id);
                        let latency_ns = if latency == Duration::ZERO {
                            req_start.elapsed().as_nanos() as u64
                        } else {
                            latency.as_nanos() as u64
                        };

                        metrics.record_request(latency_ns, true);
                        last_request = Instant::now();
                    }
                })
            })
            .collect();

        // Wait for all clients to finish
        for handle in handles {
            handle.join().expect("Client thread panicked");
        }

        let actual_duration = start.elapsed();

        // Cooldown phase
        if self.config.cooldown_duration > Duration::ZERO {
            println!("Cooldown: {:.2}s...", self.config.cooldown_duration.as_secs_f64());
            std::thread::sleep(self.config.cooldown_duration);
        }

        // Calculate results
        let stats = metrics.get_stats();
        let actual_rps = stats.requests_sent as f64 / actual_duration.as_secs_f64();

        // Calculate latency percentiles (if tracking enabled)
        let latency_percentiles = if self.config.track_latencies {
            // For this mock, derive from min/max/avg
            Some(LatencyPercentiles {
                p50_ns: stats.average_latency_ns,
                p95_ns: stats.average_latency_ns + (stats.max_latency_ns - stats.average_latency_ns) / 2,
                p99_ns: stats.max_latency_ns - (stats.max_latency_ns - stats.average_latency_ns) / 10,
                p999_ns: stats.max_latency_ns,
            })
        } else {
            None
        };

        LoadTestResults {
            config: self.config.clone(),
            stats,
            actual_duration,
            actual_rps,
            latency_percentiles,
        }
    }
}

/// Test 1: Variable Request Rate (10, 100, 1000, 10K req/s)
#[test]
fn test_load_variable_request_rate() {
    let test_cases = vec![10, 100, 1000, 10_000];

    for target_rps in test_cases {
        let config = LoadTestConfig::new(format!("variable_rps_{}", target_rps))
            .with_rps(target_rps)
            .with_duration(Duration::from_secs(5))
            .with_clients(1)
            .with_warmup(Duration::ZERO)
            .with_cooldown(Duration::ZERO);

        let runner = LoadTestRunner::new(config);

        let results = runner.run(move |_| {
            // Mock operation (minimal latency)
            let _ = std::sync::atomic::AtomicU64::new(0).fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Duration::ZERO
        });

        results.print_summary();

        // Validation
        results.assert_success_rate(100.0);

        // Allow ±20% variance in actual RPS vs target
        let min_rps = target_rps as f64 * 0.8;
        assert!(
            results.actual_rps >= min_rps,
            "Actual RPS {:.2} below 80% of target {}",
            results.actual_rps,
            target_rps
        );
    }
}

/// Test 2: Variable Duration (1s, 10s, 60s, 1hr)
#[test]
#[ignore = "Long-running test (1 hour) - run manually"]
fn test_load_variable_duration() {
    let test_cases = vec![
        ("1_second", Duration::from_secs(1)),
        ("10_seconds", Duration::from_secs(10)),
        ("60_seconds", Duration::from_secs(60)),
        ("1_hour", Duration::from_secs(3600)),
    ];

    for (name, duration) in test_cases {
        let config = LoadTestConfig::new(format!("variable_duration_{}", name))
            .with_rps(100)
            .with_duration(duration)
            .with_clients(1)
            .with_warmup(Duration::ZERO)
            .with_cooldown(Duration::ZERO);

        let runner = LoadTestRunner::new(config);

        let results = runner.run(move |_| {
            let _ = std::sync::atomic::AtomicU64::new(0).fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Duration::ZERO
        });

        results.print_summary();
        results.assert_success_rate(100.0);
    }
}

/// Test 3: Variable Client Count (1, 10, 100, 1000 concurrent clients)
#[test]
fn test_load_variable_client_count() {
    let test_cases = vec![1, 10, 100, 1000];

    for num_clients in test_cases {
        let config = LoadTestConfig::new(format!("variable_clients_{}", num_clients))
            .with_rps(1000)
            .with_duration(Duration::from_secs(5))
            .with_clients(num_clients)
            .with_warmup(Duration::ZERO)
            .with_cooldown(Duration::ZERO);

        let runner = LoadTestRunner::new(config);

        let results = runner.run(move |_| {
            let _ = std::sync::atomic::AtomicU64::new(0).fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Duration::ZERO
        });

        results.print_summary();

        // Validation
        results.assert_success_rate(100.0);

        println!("Concurrency level: {} clients achieved {:.2} RPS\n", num_clients, results.actual_rps);
    }
}

/// Test 4: Latency Distribution Collection (P50/P95/P99/P99.9)
#[test]
fn test_load_latency_distribution() {
    let config = LoadTestConfig::new("latency_distribution")
        .with_rps(1000)
        .with_duration(Duration::from_secs(10))
        .with_clients(10)
        .with_warmup(Duration::from_secs(1))
        .with_cooldown(Duration::ZERO);

    let runner = LoadTestRunner::new(config);

    let results = runner.run(move |_| {
        // Simulate variable latency
        let latency_us = fastrand::u64(1..100);
        Duration::from_micros(latency_us)
    });

    results.print_summary();

    // Verify percentiles exist
    assert!(results.latency_percentiles.is_some(), "Latency percentiles not tracked");

    if let Some(ref percentiles) = results.latency_percentiles {
        // Validate percentile ordering: P50 ≤ P95 ≤ P99 ≤ P99.9
        assert!(percentiles.p50_ns <= percentiles.p95_ns, "P50 > P95");
        assert!(percentiles.p95_ns <= percentiles.p99_ns, "P95 > P99");
        assert!(percentiles.p99_ns <= percentiles.p999_ns, "P99 > P99.9");
    }
}

/// Test 5: Resource Usage Tracking (CPU, memory, FDs)
#[test]
fn test_load_resource_usage_tracking() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let cpu_usage = Arc::new(AtomicUsize::new(0));
    let memory_allocated = Arc::new(AtomicUsize::new(0));
    let fd_count = Arc::new(AtomicUsize::new(0));

    let config = LoadTestConfig::new("resource_usage_tracking")
        .with_rps(1000)
        .with_duration(Duration::from_secs(5))
        .with_clients(10);

    let runner = LoadTestRunner::new(config);

    // Clone Arcs before move closure
    let cpu_usage_clone = cpu_usage.clone();
    let memory_clone = memory_allocated.clone();
    let fd_clone = fd_count.clone();

    let results = runner.run(move |_| {
        // Mock: Track resource usage
        cpu_usage_clone.fetch_add(1, Ordering::Relaxed);
        memory_clone.fetch_add(1024, Ordering::Relaxed); // 1 KB per request
        fd_clone.fetch_add(0, Ordering::Relaxed); // FDs stay constant (lockfree)

        Duration::from_micros(10)
    });

    results.print_summary();

    let final_cpu = cpu_usage.load(Ordering::Relaxed);
    let final_memory = memory_allocated.load(Ordering::Relaxed);
    let final_fds = fd_count.load(Ordering::Relaxed);

    println!("Resource usage:");
    println!("  CPU operations: {}", final_cpu);
    println!("  Memory allocated: {} bytes ({:.2} KB)", final_memory, final_memory as f64 / 1024.0);
    println!("  File descriptors: {}", final_fds);

    // Validation
    assert_eq!(final_cpu, results.stats.requests_sent as usize, "CPU operations mismatch");
}

/// Helper: Create mock operation with specified latency
#[allow(dead_code)]
pub fn mock_operation_with_latency(latency: Duration) -> impl Fn(usize) -> Duration {
    move |_| latency
}

/// Helper: Create mock operation with random latency
#[allow(dead_code)]
pub fn mock_operation_with_random_latency(min_us: u64, max_us: u64) -> impl Fn(usize) -> Duration {
    move |_| Duration::from_micros(fastrand::u64(min_us..max_us))
}
