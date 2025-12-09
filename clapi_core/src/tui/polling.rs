//! Metrics Polling - Background HTTP Polling with Exponential Backoff
//!
//! # Purpose
//! Computational capsule for background HTTP polling with:
//! - **100% lockfree** - Atomic fields for polling state
//! - **Real-time updates** - 5s polling interval (configurable)
//! - **Exponential backoff** - Retry with backoff on HTTP errors
//! - **Graceful degradation** - Display "Server Offline" on connection refused
//!
//! # UCE34 Framework
//! - **Q10**: Tier 1 (Atomic) + Tier 5 (Streaming) - Incremental metrics updates
//! - **Q11**: Rust atomic primitives (AtomicU64, AtomicU32, AtomicBool)
//! - **Q12**: Stable Rust (no nightly features required)
//! - **Q13**: Resources - Background thread, 5s polling interval, minimal memory
//! - **Q15**: Scale - Single polling thread, no contention
//! - **Q20**: Error Handling - Graceful degradation on HTTP failures, retry with exponential backoff
//! - **Q33**: Validation - #[derive(ComputationalCapsule)]
//!
//! # Performance
//! - **HTTP GET**: <50ms (local endpoint)
//! - **Atomic update**: <10ns per field (Relaxed ordering)
//! - **Backoff delay**: 100ms → 200ms → 400ms → 800ms → max 5s
//! - **Memory**: <1MB (reqwest client + minimal state)
//!
//! # ASSUM Safety
//! - `#ASSUME_HTTP_LOCALHOST`: HTTP endpoint is localhost (no TLS required)
//! - `#VERIFY_TIMEOUT`: reqwest client with default timeout (10s)
//! - `#ASSUME_MIN_INTERVAL`: Polling interval ≥100ms (no flooding)
//! - `#VERIFY_INTERVAL`: Interval validation enforces 100ms minimum
//! - `#ASSUME_ATOMIC_SAFE`: DashboardContentCapsule updates are atomic-safe
//! - `#VERIFY_RELAXED_OK`: All update methods use Relaxed ordering (no synchronization needed)

#![warn(clippy::missing_capsule_verification)]

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::task::JoinHandle;

use crate::tui::DashboardContentCapsule;
use crate::cli::dashboard::SystemMetrics;

/// Metrics polling capsule (Tier 1 Atomic + Tier 5 Streaming)
///
/// # Layout
/// ```text
/// [0]       enabled            AtomicBool (1B)
/// [1-3]     _padding0          [u8; 3] (alignment padding)
/// [4-7]     interval_ms        AtomicU32 (4B)
/// [8-15]    last_poll_ns       AtomicU64 (8B)
/// [16-23]   poll_count         AtomicU64 (8B)
/// [24-27]   error_count        AtomicU32 (4B)
/// [28-31]   last_error_code    AtomicU32 (4B)
/// [32-35]   http_latency_us    AtomicU32 (4B)
/// [36-38]   _padding1          [u8; 3] (alignment padding)
/// [39]      last_success       AtomicBool (1B)
/// [40-255]  _padding2          [u8; 216] (pad to 256B)
/// ```
///
/// **Alignment**: 256B (cache-aligned for hot polling state)
/// **Size**: 256B (first 40B hot, rest reserved for future expansion)
///
/// # Performance
/// - **Atomic update**: <10ns (Relaxed ordering)
/// - **Full snapshot**: <100ns (read all fields)
/// - **HTTP poll**: <50ms (local endpoint)
///
/// # Chaos Principles
/// - **Cache-aligned**: 256B alignment prevents false sharing
/// - **Tiered layout**: Hot polling state (first 40B), cold metrics (rest)
/// - **One-read decision**: All state fits in single cache line (64B)
/// - **Lockfree**: 100% atomic operations (no Mutex/RwLock)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256, tier = "Atomic")]
#[repr(C, align(256))]
pub struct MetricsPollingCapsule {
    /// Polling active (true = polling, false = stopped)
    enabled: AtomicBool,

    /// Alignment padding (3B)
    _padding0: [u8; 3],

    /// Polling interval in milliseconds (default: 5000ms)
    interval_ms: AtomicU32,

    /// Last successful poll timestamp (nanoseconds since epoch)
    last_poll_ns: AtomicU64,

    /// Total polls attempted (includes failures)
    poll_count: AtomicU64,

    /// Failed polls count
    error_count: AtomicU32,

    /// Last HTTP status code (0 = no error, 404/500/etc)
    last_error_code: AtomicU32,

    /// Last poll HTTP latency (microseconds)
    http_latency_us: AtomicU32,

    /// Alignment padding (3B)
    _padding1: [u8; 3],

    /// Last poll success state (true = success, false = error)
    last_success: AtomicBool,

    /// Padding to 256B (216B reserved for future expansion)
    _padding2: [u8; 216],
}

impl MetricsPollingCapsule {
    /// Create new metrics polling capsule
    ///
    /// # Performance
    /// - <50ns (zero-cost atomic initialization)
    ///
    /// # Arguments
    /// - `interval_ms`: Polling interval in milliseconds (minimum 100ms)
    ///
    /// # Examples
    /// ```ignore
    /// use clapi_core::tui::MetricsPollingCapsule;
    ///
    /// let capsule = MetricsPollingCapsule::new(5000); // 5s refresh
    /// ```
    pub fn new(interval_ms: u32) -> Self {
        // Enforce minimum 100ms interval (prevents flooding)
        let safe_interval = interval_ms.max(100);

        Self {
            enabled: AtomicBool::new(false),
            _padding0: [0; 3],
            interval_ms: AtomicU32::new(safe_interval),
            last_poll_ns: AtomicU64::new(0),
            poll_count: AtomicU64::new(0),
            error_count: AtomicU32::new(0),
            last_error_code: AtomicU32::new(0),
            http_latency_us: AtomicU32::new(0),
            _padding1: [0; 3],
            last_success: AtomicBool::new(false),
            _padding2: [0; 216],
        }
    }

    /// Enable polling
    #[inline]
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
    }

    /// Disable polling
    #[inline]
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }

    /// Check if polling is enabled
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Get polling interval in milliseconds
    #[inline]
    pub fn interval_ms(&self) -> u32 {
        self.interval_ms.load(Ordering::Relaxed)
    }

    /// Set polling interval (minimum 100ms)
    #[inline]
    pub fn set_interval_ms(&self, interval_ms: u32) {
        let safe_interval = interval_ms.max(100);
        self.interval_ms.store(safe_interval, Ordering::Relaxed);
    }

    /// Record successful poll
    ///
    /// # Performance
    /// - <50ns (4 atomic stores with Relaxed ordering)
    ///
    /// # Arguments
    /// - `latency_us`: HTTP latency in microseconds
    #[inline]
    pub fn record_success(&self, latency_us: u32) {
        let now_ns = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        self.last_poll_ns.store(now_ns, Ordering::Relaxed);
        self.poll_count.fetch_add(1, Ordering::Relaxed);
        self.http_latency_us.store(latency_us, Ordering::Relaxed);
        self.last_error_code.store(0, Ordering::Relaxed);
        self.last_success.store(true, Ordering::Relaxed);
    }

    /// Record failed poll
    ///
    /// # Performance
    /// - <30ns (3 atomic stores with Relaxed ordering)
    ///
    /// # Arguments
    /// - `error_code`: HTTP status code (0 for network errors)
    #[inline]
    pub fn record_failure(&self, error_code: u32) {
        self.poll_count.fetch_add(1, Ordering::Relaxed);
        self.error_count.fetch_add(1, Ordering::Relaxed);
        self.last_error_code.store(error_code, Ordering::Relaxed);
        self.last_success.store(false, Ordering::Relaxed);
    }

    /// Get polling statistics snapshot
    ///
    /// # Performance
    /// - <100ns (7 atomic loads with Relaxed ordering)
    pub fn stats(&self) -> PollingStats {
        PollingStats {
            enabled: self.enabled.load(Ordering::Relaxed),
            interval_ms: self.interval_ms.load(Ordering::Relaxed),
            last_poll_ns: self.last_poll_ns.load(Ordering::Relaxed),
            poll_count: self.poll_count.load(Ordering::Relaxed),
            error_count: self.error_count.load(Ordering::Relaxed),
            last_error_code: self.last_error_code.load(Ordering::Relaxed),
            http_latency_us: self.http_latency_us.load(Ordering::Relaxed),
            last_success: self.last_success.load(Ordering::Relaxed),
        }
    }
}

/// Polling statistics snapshot (lockfree)
#[derive(Debug, Clone, Copy)]
pub struct PollingStats {
    pub enabled: bool,
    pub interval_ms: u32,
    pub last_poll_ns: u64,
    pub poll_count: u64,
    pub error_count: u32,
    pub last_error_code: u32,
    pub http_latency_us: u32,
    pub last_success: bool,
}

impl PollingStats {
    /// Calculate success rate (0.0 - 1.0)
    pub fn success_rate(&self) -> f64 {
        if self.poll_count == 0 {
            return 1.0;
        }
        let success_count = self.poll_count - self.error_count as u64;
        success_count as f64 / self.poll_count as f64
    }

    /// Time since last poll (seconds)
    pub fn time_since_last_poll_secs(&self) -> u64 {
        if self.last_poll_ns == 0 {
            return u64::MAX; // Never polled
        }

        let now_ns = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        (now_ns.saturating_sub(self.last_poll_ns)) / 1_000_000_000
    }
}

/// Metrics poller with background HTTP polling
///
/// # Example
/// ```ignore
/// use clapi_core::tui::{MetricsPoller, DashboardContentCapsule};
/// use std::sync::Arc;
///
/// let content = Arc::new(DashboardContentCapsule::new(5000));
/// let poller = MetricsPoller::new("http://localhost:8080/metrics".to_string());
///
/// // Start background polling
/// let handle = poller.start(content.clone());
///
/// // ... TUI event loop ...
///
/// // Stop polling
/// poller.stop();
/// handle.await;
/// ```
pub struct MetricsPoller {
    /// Polling capsule (atomic state)
    capsule: Arc<MetricsPollingCapsule>,

    /// HTTP client (connection pooling)
    client: reqwest::Client,

    /// Base URL for metrics endpoint
    base_url: String,
}

/// Metrics API response (matches /metrics endpoint schema)
#[derive(Debug, serde::Deserialize)]
struct MetricsResponse {
    #[serde(default)]
    budgets: Vec<BudgetMetricRaw>,

    #[serde(default)]
    providers: Vec<ProviderMetricRaw>,

    #[serde(default)]
    system: SystemMetricsRaw,

    #[serde(default)]
    cache: Option<CacheMetricsRaw>,

    #[serde(default)]
    compression: Option<CompressionMetricsRaw>,

    #[serde(default)]
    load_balancer: Option<LoadBalancerMetricsRaw>,

    #[serde(default)]
    performance: Option<PerformanceMetricsRaw>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct BudgetMetricRaw {
    budget_id: u64,
    available_cents: i64,
    spent_cents: i64,
    utilization_bp: u32, // Basis points (0-10000)
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct ProviderMetricRaw {
    provider_name: String,
    circuit_state: u8, // 0=Closed, 1=HalfOpen, 2=Open
    failures_count: u64,
    latency_p99_ns: u64,
    success_rate_bp: u32, // Basis points (0-10000)
    total_requests: u64,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct SystemMetricsRaw {
    uptime_secs: u64,
    total_requests: u64,
    avg_latency_ns: u64,
    memory_bytes: u64,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct CacheMetricsRaw {
    hit_rate: f64,
    #[allow(dead_code)]
    hits: u64,
    #[allow(dead_code)]
    misses: u64,
    memory_bytes: u64,
    entry_count: u64,
    eviction_rate: f64,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct CompressionMetricsRaw {
    compression_ratio: f64,
    throughput_bytes_per_sec: u64,
    bandwidth_saved_bytes: u64,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct LoadBalancerMetricsRaw {
    cost_per_1k_tokens_cents: f64,
    provider_latencies_ms: Vec<(String, f64)>,
    failover_rate: f64,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct PerformanceMetricsRaw {
    p50_ms: f64,
    p99_ms: f64,
    p999_ms: f64,
}

impl MetricsPoller {
    /// Create new metrics poller
    ///
    /// # Arguments
    /// - `base_url`: Metrics endpoint URL (e.g., "http://localhost:8080/metrics")
    ///
    /// # Examples
    /// ```
    /// use clapi_core::tui::MetricsPoller;
    ///
    /// let poller = MetricsPoller::new("http://localhost:8080/metrics".to_string());
    /// ```
    pub fn new(base_url: String) -> Self {
        let capsule = Arc::new(MetricsPollingCapsule::new(5000)); // 5s default

        // Create HTTP client with 10s timeout
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            capsule,
            client,
            base_url,
        }
    }

    /// Start background polling thread
    ///
    /// # Arguments
    /// - `content`: Dashboard content capsule to update
    ///
    /// # Returns
    /// JoinHandle for background task
    ///
    /// # Performance
    /// - Polling overhead: <50ms per refresh (local HTTP)
    /// - Atomic updates: <10ns per field
    /// - Memory: <1MB (reqwest client + minimal state)
    ///
    /// # Examples
    /// ```ignore
    /// use clapi_core::tui::{MetricsPoller, DashboardContentCapsule};
    /// use std::sync::Arc;
    ///
    /// let content = Arc::new(DashboardContentCapsule::new(5000));
    /// let poller = MetricsPoller::new("http://localhost:8080/metrics".to_string());
    ///
    /// let handle = poller.start(content.clone());
    /// ```
    pub fn start(&self, content: Arc<DashboardContentCapsule>) -> JoinHandle<()> {
        self.capsule.enable();

        let capsule = self.capsule.clone();
        let client = self.client.clone();
        let base_url = self.base_url.clone();

        tokio::spawn(async move {
            let mut backoff_ms = 100u64; // Start at 100ms

            loop {
                // Check if polling is still enabled
                if !capsule.is_enabled() {
                    break;
                }

                // Get polling interval
                let interval_ms = capsule.interval_ms();

                // Attempt HTTP fetch
                let start = SystemTime::now();
                match Self::fetch_metrics_impl(&client, &base_url).await {
                    Ok(metrics) => {
                        // Record success
                        let latency_us = start.elapsed().unwrap().as_micros() as u32;
                        capsule.record_success(latency_us);

                        // Update dashboard content
                        Self::update_content(&content, metrics);

                        // Reset backoff on success
                        backoff_ms = 100;

                        // Sleep for polling interval
                        tokio::time::sleep(Duration::from_millis(interval_ms as u64)).await;
                    }
                    Err((error_code, _error_msg)) => {
                        // Record failure
                        capsule.record_failure(error_code);

                        // Set error state in dashboard
                        content.set_error(true);

                        // Note: Errors are stored in capsule.stats() for monitoring,
                        // but NOT printed to avoid flooding TUI output
                        // (see capsule.last_error_code for error details)

                        // Exponential backoff (100ms → 200ms → 400ms → 800ms → max 5s)
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        backoff_ms = (backoff_ms * 2).min(5000);
                    }
                }
            }
        })
    }

    /// Stop polling
    ///
    /// # Examples
    /// ```ignore
    /// let poller = MetricsPoller::new("http://localhost:8080/metrics".to_string());
    /// // ... start polling ...
    /// poller.stop();
    /// ```
    pub fn stop(&self) {
        self.capsule.disable();
    }

    /// Get polling statistics
    ///
    /// # Examples
    /// ```ignore
    /// let poller = MetricsPoller::new("http://localhost:8080/metrics".to_string());
    /// let stats = poller.stats();
    /// println!("Success rate: {:.1}%", stats.success_rate() * 100.0);
    /// ```
    pub fn stats(&self) -> PollingStats {
        self.capsule.stats()
    }

    /// Fetch metrics from HTTP endpoint (internal implementation)
    ///
    /// # Performance
    /// - HTTP GET: <50ms (local endpoint)
    /// - JSON parsing: <10ms (serde_json)
    ///
    /// # Returns
    /// - Ok(MetricsResponse) on success
    /// - Err((error_code, error_msg)) on failure
    ///
    /// # Safety
    /// - #ASSUME_HTTP_LOCALHOST: HTTP endpoint is localhost (no TLS required)
    /// - #VERIFY_TIMEOUT: reqwest client with 10s timeout
    async fn fetch_metrics_impl(
        client: &reqwest::Client,
        base_url: &str,
    ) -> Result<MetricsResponse, (u32, String)> {
        let response = client
            .get(base_url)
            .send()
            .await
            .map_err(|e| {
                // Connection errors (server offline, network unreachable)
                (0, format!("Connection failed: {}", e))
            })?;

        // Check HTTP status
        let status = response.status();
        if !status.is_success() {
            return Err((status.as_u16() as u32, format!("HTTP error: {}", status)));
        }

        // Parse JSON response
        let metrics: MetricsResponse = response
            .json()
            .await
            .map_err(|e| (0, format!("JSON parsing failed: {}", e)))?;

        Ok(metrics)
    }

    /// Update dashboard content with fetched metrics
    ///
    /// # Performance
    /// - <100ns (atomic stores with Relaxed ordering)
    ///
    /// # Arguments
    /// - `content`: Dashboard content capsule
    /// - `metrics`: Fetched metrics response
    fn update_content(content: &DashboardContentCapsule, metrics: MetricsResponse) {
        // Update system metrics
        let system = SystemMetrics {
            uptime: format_duration(metrics.system.uptime_secs),
            total_requests: metrics.system.total_requests,
            avg_latency_ms: metrics.system.avg_latency_ns / 1_000_000,
            memory_mb: metrics.system.memory_bytes / 1_048_576,
            uptime_secs: metrics.system.uptime_secs,
        };
        content.update_system_metrics(&system);

        // Parse provider metrics (circuit breaker states)
        for (idx, provider) in metrics.providers.iter().enumerate().take(8) {
            let provider_idx = idx as u8;
            content.set_circuit_state(provider_idx, provider.circuit_state);

            // Convert basis points to percentage (0-10000 -> 0-100)
            let success_pct = (provider.success_rate_bp / 100).min(100) as u8;
            content.set_provider_success_rate(provider_idx, success_pct);

            let failures = provider.failures_count.min(255) as u8;
            content.set_provider_failures(provider_idx, failures);
        }

        // Parse budget metrics
        for (idx, budget) in metrics.budgets.iter().enumerate().take(8) {
            let budget_idx = idx as u8;

            // Convert basis points to percentage
            let utilization_pct = (budget.utilization_bp / 100).min(100) as u8;
            content.set_budget_utilization(budget_idx, utilization_pct);

            // Store spent/available for cost tracking
            let spent_cents = budget.spent_cents.max(0) as u64;
            content.add_spent_cents(spent_cents);
        }

        // Parse performance metrics (if available)
        if let Some(perf) = metrics.performance {
            content.set_p50_latency(perf.p50_ms as u32);
            content.set_p99_latency(perf.p99_ms as u32);
            content.set_p999_latency(perf.p999_ms as u32);
        }

        // Parse load balancer cost metrics (if available)
        if let Some(lb) = metrics.load_balancer {
            let cost_cents = (lb.cost_per_1k_tokens_cents * 100.0) as u32;
            content.set_cost_per_1k_tokens(cost_cents);
        }

        // Update budget count
        content.set_budgets_count(metrics.budgets.len() as u32);

        // Update provider count
        content.set_providers_count(metrics.providers.len() as u32);

        // Clear error state on success
        content.set_error(false);
    }
}

/// Format duration as human-readable string
///
/// # Examples
/// ```
/// # use clapi_core::tui::polling::format_duration;
/// assert_eq!(format_duration(65), "1m 5s");
/// assert_eq!(format_duration(3661), "1h 1m 1s");
/// ```
pub fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_creation() {
        let capsule = MetricsPollingCapsule::new(5000);
        assert_eq!(capsule.interval_ms(), 5000);
        assert!(!capsule.is_enabled());
    }

    #[test]
    fn test_min_interval_enforcement() {
        // Test that intervals below 100ms are clamped
        let capsule = MetricsPollingCapsule::new(50);
        assert_eq!(capsule.interval_ms(), 100);

        // Test setting interval below 100ms
        capsule.set_interval_ms(25);
        assert_eq!(capsule.interval_ms(), 100);
    }

    #[test]
    fn test_enable_disable() {
        let capsule = MetricsPollingCapsule::new(1000);
        assert!(!capsule.is_enabled());

        capsule.enable();
        assert!(capsule.is_enabled());

        capsule.disable();
        assert!(!capsule.is_enabled());
    }

    #[test]
    fn test_record_success() {
        let capsule = MetricsPollingCapsule::new(1000);

        capsule.record_success(50_000); // 50ms latency

        let stats = capsule.stats();
        assert_eq!(stats.poll_count, 1);
        assert_eq!(stats.error_count, 0);
        assert_eq!(stats.http_latency_us, 50_000);
        assert_eq!(stats.last_error_code, 0);
        assert!(stats.last_success);
        assert!(stats.last_poll_ns > 0);
    }

    #[test]
    fn test_record_failure() {
        let capsule = MetricsPollingCapsule::new(1000);

        capsule.record_failure(500); // HTTP 500

        let stats = capsule.stats();
        assert_eq!(stats.poll_count, 1);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.last_error_code, 500);
        assert!(!stats.last_success);
    }

    #[test]
    fn test_success_rate() {
        let capsule = MetricsPollingCapsule::new(1000);

        // 3 successes, 1 failure
        capsule.record_success(100);
        capsule.record_success(150);
        capsule.record_success(120);
        capsule.record_failure(0);

        let stats = capsule.stats();
        assert_eq!(stats.poll_count, 4);
        assert_eq!(stats.error_count, 1);
        assert!((stats.success_rate() - 0.75).abs() < 0.01); // 75% success rate
    }

    #[test]
    fn test_success_rate_no_polls() {
        let capsule = MetricsPollingCapsule::new(1000);
        let stats = capsule.stats();
        assert_eq!(stats.success_rate(), 1.0); // 100% by default
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(65), "1m 5s");
        assert_eq!(format_duration(3661), "1h 1m 1s");
        assert_eq!(format_duration(7200), "2h 0m 0s");
    }

    #[test]
    fn test_polling_stats_snapshot() {
        let capsule = MetricsPollingCapsule::new(5000);

        capsule.enable();
        capsule.record_success(1000);
        capsule.record_failure(500);

        let stats = capsule.stats();
        assert!(stats.enabled);
        assert_eq!(stats.interval_ms, 5000);
        assert_eq!(stats.poll_count, 2);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.last_error_code, 500);
        assert!(!stats.last_success); // Last poll failed
    }

    #[test]
    fn test_time_since_last_poll() {
        let capsule = MetricsPollingCapsule::new(1000);

        // No poll yet
        let stats = capsule.stats();
        assert_eq!(stats.time_since_last_poll_secs(), u64::MAX);

        // Record a poll
        capsule.record_success(100);
        let stats = capsule.stats();
        assert!(stats.time_since_last_poll_secs() < 5); // Should be very recent
    }

    #[test]
    fn test_poller_creation() {
        let poller = MetricsPoller::new("http://localhost:8080/metrics".to_string());
        assert_eq!(poller.base_url, "http://localhost:8080/metrics");

        let stats = poller.stats();
        assert!(!stats.enabled);
        assert_eq!(stats.interval_ms, 5000); // Default 5s
    }

    #[test]
    fn test_poller_stop() {
        let poller = MetricsPoller::new("http://localhost:8080/metrics".to_string());

        poller.capsule.enable();
        assert!(poller.capsule.is_enabled());

        poller.stop();
        assert!(!poller.capsule.is_enabled());
    }
}
