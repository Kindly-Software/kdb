//! MCP Client Metrics Capsule (T1+T3)
//!
//! Phase 1 resilience metrics tracking for MCP client operations.
//!
//! ## UCE35 Analysis
//!
//! - **Q10 (Tier Selection)**: T1+T3 (Atomic counters + Fixed-Point latency)
//! - **Q28 (Simplicity)**: Simple API hiding Q16.16 fixed-point complexity
//! - **Q33 (Atomic Capsule)**: 64B-aligned, 100% lockfree
//! - **Q34 (Audit Trail)**: Metrics exportable for compliance reporting
//!
//! ## Memory Layout
//!
//! ```text
//! [Request Counters: 48 bytes]
//!   total_requests:           AtomicU64 = 8 bytes
//!   successful_requests:      AtomicU64 = 8 bytes
//!   failed_requests:          AtomicU64 = 8 bytes
//!   cached_hits:              AtomicU64 = 8 bytes
//!   retried_requests:         AtomicU64 = 8 bytes
//!   circuit_breaker_rejects:  AtomicU64 = 8 bytes
//!
//! [Latency Tracking Q16.16: 32 bytes]
//!   latency_sum_q16:          AtomicU64 = 8 bytes
//!   latency_count:            AtomicU64 = 8 bytes
//!   latency_max_q16:          AtomicU64 = 8 bytes
//!   latency_p99_q16:          AtomicU64 = 8 bytes
//!
//! [Timestamps: 16 bytes]
//!   started_at_unix:          AtomicU64 = 8 bytes
//!   last_request_unix:        AtomicU64 = 8 bytes
//!
//! [Padding: 32 bytes]
//!   _padding:                 [u8; 32] = 32 bytes
//!
//! Total: 128 bytes (64B-aligned, 2 cache lines)
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_CACHE_ALIGNMENT`: 64-byte alignment prevents false sharing
//! - `#VERIFY_ALIGNMENT_STATIC`: Verified at compile-time via repr(align(64))
//! - `#ASSUME_MONOTONIC_COUNTERS`: Counters only increment, Relaxed ordering safe
//! - `#VERIFY_MONOTONIC`: record_request() only uses fetch_add
//! - `#ASSUME_LATENCY_COORDINATION`: Latency updates use AcqRel for consistency
//! - `#VERIFY_LATENCY_ORDERING`: fetch_add/compare_exchange use AcqRel

use core::sync::atomic::{AtomicU64, Ordering};

/// Q16.16 fixed-point scale factor (2^16 = 65536)
const Q16_16_SCALE: u64 = 65536;

/// Metrics snapshot for external consumption
#[derive(Debug, Clone, Copy)]
pub struct MetricsStats {
    /// Total requests processed
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Cache hits
    pub cached_hits: u64,
    /// Retried requests
    pub retried_requests: u64,
    /// Circuit breaker rejections
    pub circuit_breaker_rejects: u64,
    /// Average latency in microseconds
    pub average_latency_us: f64,
    /// Maximum latency in microseconds
    pub max_latency_us: f64,
    /// P99 latency estimate in microseconds
    pub p99_latency_us: f64,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
    /// Unix timestamp when metrics started
    pub started_at_unix: u64,
    /// Unix timestamp of last request
    pub last_request_unix: u64,
    /// Uptime in seconds
    pub uptime_seconds: u64,
}

/// MCP Client Metrics Capsule (T1+T3)
///
/// Tracks request counts, latency, success rates using Q16.16 fixed-point
/// for deterministic arithmetic.
///
/// # Layout
/// - Size: 128 bytes (2 cache lines)
/// - Alignment: 64 bytes (cache line)
/// - Tiers: T1 (Atomic counters) + T3 (Fixed-Point latency)
///
/// # Performance
/// - record_request(): <50ns (atomic fetch_add + compare_exchange)
/// - get_stats(): <100ns (atomic snapshot)
/// - success_rate(): <10ns (atomic loads)
///
/// # ASSUM Safety
/// - `#ASSUME_LOCKFREE`: 100% lockfree, no mutex/RwLock
/// - `#VERIFY_LOCKFREE`: Only AtomicU64 operations used
#[repr(C, align(64))]
pub struct McpMetricsCapsule {
    // Request counters (T1 Atomic) - 48 bytes
    /// Total requests processed (monotonic counter)
    total_requests: AtomicU64,
    /// Successful requests (monotonic counter)
    successful_requests: AtomicU64,
    /// Failed requests (monotonic counter)
    failed_requests: AtomicU64,
    /// Cache hits (monotonic counter)
    cached_hits: AtomicU64,
    /// Retried requests (monotonic counter)
    retried_requests: AtomicU64,
    /// Circuit breaker rejections (monotonic counter)
    circuit_breaker_rejects: AtomicU64,

    // Latency tracking (T3 Fixed-Point Q16.16) - 32 bytes
    /// Sum of latencies in Q16.16 microseconds (for average calculation)
    latency_sum_q16: AtomicU64,
    /// Count for average latency calculation
    latency_count: AtomicU64,
    /// Maximum latency observed in Q16.16 microseconds
    latency_max_q16: AtomicU64,
    /// Approximate P99 latency via EMA in Q16.16 microseconds
    latency_p99_q16: AtomicU64,

    // Timestamps - 16 bytes
    /// Unix timestamp when capsule was created
    started_at_unix: AtomicU64,
    /// Unix timestamp of last request
    last_request_unix: AtomicU64,

    // Padding to 128B - 32 bytes
    _padding: [u8; 32],
}

impl McpMetricsCapsule {
    /// Create new metrics capsule initialized with current timestamp
    ///
    /// # Examples
    /// ```
    /// use kdb_mcp::client::McpMetricsCapsule;
    ///
    /// let metrics = McpMetricsCapsule::new();
    /// assert_eq!(metrics.get_stats().total_requests, 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::with_timestamp(Self::current_unix_timestamp())
    }

    /// Create new metrics capsule with explicit start timestamp
    ///
    /// Useful for testing with deterministic timestamps.
    #[must_use]
    pub const fn with_timestamp(started_at: u64) -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            successful_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            cached_hits: AtomicU64::new(0),
            retried_requests: AtomicU64::new(0),
            circuit_breaker_rejects: AtomicU64::new(0),
            latency_sum_q16: AtomicU64::new(0),
            latency_count: AtomicU64::new(0),
            latency_max_q16: AtomicU64::new(0),
            latency_p99_q16: AtomicU64::new(0),
            started_at_unix: AtomicU64::new(started_at),
            last_request_unix: AtomicU64::new(started_at),
            _padding: [0u8; 32],
        }
    }

    /// Record a request with its outcome and timing
    ///
    /// # Arguments
    /// - `success`: Whether the request succeeded
    /// - `latency_us`: Request latency in microseconds
    /// - `from_cache`: Whether the response came from cache
    /// - `retry_count`: Number of retries (0 if first attempt succeeded)
    ///
    /// # Performance
    /// - <50ns (atomic fetch_add + compare_exchange)
    ///
    /// # Examples
    /// ```
    /// use kdb_mcp::client::McpMetricsCapsule;
    ///
    /// let metrics = McpMetricsCapsule::new();
    /// metrics.record_request(true, 1500, false, 0);  // 1.5ms success
    /// metrics.record_request(false, 5000, false, 2); // 5ms failure after 2 retries
    /// ```
    pub fn record_request(&self, success: bool, latency_us: u32, from_cache: bool, retry_count: u8) {
        // #ASSUME_MONOTONIC_COUNTERS: fetch_add guarantees monotonic increment
        // #VERIFY_MONOTONIC: Only increment operations used

        // Update request counters (Relaxed - monotonic counters)
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        if success {
            self.successful_requests.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
        }

        if from_cache {
            self.cached_hits.fetch_add(1, Ordering::Relaxed);
        }

        if retry_count > 0 {
            self.retried_requests
                .fetch_add(retry_count as u64, Ordering::Relaxed);
        }

        // Update latency tracking (AcqRel - coordination required)
        // Convert to Q16.16: latency_us * 65536
        let latency_q16 = (latency_us as u64) * Q16_16_SCALE;

        // #ASSUME_LATENCY_COORDINATION: AcqRel ensures consistent reads
        // #VERIFY_LATENCY_ORDERING: fetch_add uses AcqRel for sum/count coordination
        self.latency_sum_q16.fetch_add(latency_q16, Ordering::AcqRel);
        self.latency_count.fetch_add(1, Ordering::AcqRel);

        // Update max latency (compare-exchange loop)
        self.update_max_latency(latency_q16);

        // Update P99 estimate via EMA (alpha = 0.01 for slow decay)
        // P99_new = P99_old * 0.99 + latency * 0.01 (if latency > P99)
        // Q16.16: 0.99 = 64881, 0.01 = 655
        self.update_p99_estimate(latency_q16);

        // Update last request timestamp
        let now = Self::current_unix_timestamp();
        self.last_request_unix.store(now, Ordering::Relaxed);
    }

    /// Record a circuit breaker rejection
    ///
    /// Called when a request is rejected due to circuit breaker open state.
    ///
    /// # Performance
    /// - <10ns (single atomic increment)
    pub fn record_circuit_breaker_reject(&self) {
        self.circuit_breaker_rejects.fetch_add(1, Ordering::Relaxed);
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.failed_requests.fetch_add(1, Ordering::Relaxed);

        let now = Self::current_unix_timestamp();
        self.last_request_unix.store(now, Ordering::Relaxed);
    }

    /// Get snapshot of all metrics
    ///
    /// # Performance
    /// - <100ns (atomic snapshot)
    ///
    /// # Examples
    /// ```
    /// use kdb_mcp::client::McpMetricsCapsule;
    ///
    /// let metrics = McpMetricsCapsule::new();
    /// metrics.record_request(true, 1000, false, 0);
    ///
    /// let stats = metrics.get_stats();
    /// assert_eq!(stats.total_requests, 1);
    /// assert_eq!(stats.successful_requests, 1);
    /// ```
    #[must_use]
    pub fn get_stats(&self) -> MetricsStats {
        let total = self.total_requests.load(Ordering::Acquire);
        let successful = self.successful_requests.load(Ordering::Acquire);
        let failed = self.failed_requests.load(Ordering::Acquire);
        let cached = self.cached_hits.load(Ordering::Acquire);
        let retried = self.retried_requests.load(Ordering::Acquire);
        let cb_rejects = self.circuit_breaker_rejects.load(Ordering::Acquire);

        let latency_sum = self.latency_sum_q16.load(Ordering::Acquire);
        let latency_count = self.latency_count.load(Ordering::Acquire);
        let latency_max = self.latency_max_q16.load(Ordering::Acquire);
        let latency_p99 = self.latency_p99_q16.load(Ordering::Acquire);

        let started = self.started_at_unix.load(Ordering::Acquire);
        let last = self.last_request_unix.load(Ordering::Acquire);
        let now = Self::current_unix_timestamp();

        // Calculate average latency (Q16.16 to f64)
        let avg_latency = if latency_count > 0 {
            Self::q16_16_to_f64(latency_sum / latency_count)
        } else {
            0.0
        };

        // Calculate success rate
        let success_rate = if total > 0 {
            successful as f64 / total as f64
        } else {
            1.0
        };

        MetricsStats {
            total_requests: total,
            successful_requests: successful,
            failed_requests: failed,
            cached_hits: cached,
            retried_requests: retried,
            circuit_breaker_rejects: cb_rejects,
            average_latency_us: avg_latency,
            max_latency_us: Self::q16_16_to_f64(latency_max),
            p99_latency_us: Self::q16_16_to_f64(latency_p99),
            success_rate,
            started_at_unix: started,
            last_request_unix: last,
            uptime_seconds: now.saturating_sub(started),
        }
    }

    /// Get average latency in microseconds
    ///
    /// # Performance
    /// - <20ns (2 atomic loads + division)
    ///
    /// # Examples
    /// ```
    /// use kdb_mcp::client::McpMetricsCapsule;
    ///
    /// let metrics = McpMetricsCapsule::new();
    /// metrics.record_request(true, 1000, false, 0);
    /// metrics.record_request(true, 2000, false, 0);
    ///
    /// assert!((metrics.average_latency_us() - 1500.0).abs() < 0.1);
    /// ```
    #[must_use]
    pub fn average_latency_us(&self) -> f64 {
        let sum = self.latency_sum_q16.load(Ordering::Acquire);
        let count = self.latency_count.load(Ordering::Acquire);

        if count > 0 {
            Self::q16_16_to_f64(sum / count)
        } else {
            0.0
        }
    }

    /// Get success rate (successful / total)
    ///
    /// # Returns
    /// - 0.0 to 1.0 representing success percentage
    /// - 1.0 if no requests recorded
    ///
    /// # Performance
    /// - <10ns (2 atomic loads)
    ///
    /// # Examples
    /// ```
    /// use kdb_mcp::client::McpMetricsCapsule;
    ///
    /// let metrics = McpMetricsCapsule::new();
    /// metrics.record_request(true, 100, false, 0);
    /// metrics.record_request(true, 100, false, 0);
    /// metrics.record_request(false, 100, false, 0);
    ///
    /// assert!((metrics.success_rate() - 0.666).abs() < 0.01);
    /// ```
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Acquire);
        let successful = self.successful_requests.load(Ordering::Acquire);

        if total > 0 {
            successful as f64 / total as f64
        } else {
            1.0
        }
    }

    /// Print metrics summary to stderr (for shutdown logging)
    ///
    /// Outputs a human-readable summary of all metrics.
    ///
    /// # Examples
    /// ```
    /// use kdb_mcp::client::McpMetricsCapsule;
    ///
    /// let metrics = McpMetricsCapsule::new();
    /// metrics.record_request(true, 1500, false, 0);
    /// metrics.print_summary(); // Prints to stderr
    /// ```
    #[cfg(feature = "std")]
    pub fn print_summary(&self) {
        let stats = self.get_stats();

        eprintln!("=== MCP Client Metrics Summary ===");
        eprintln!("Total Requests:     {}", stats.total_requests);
        eprintln!("Successful:         {}", stats.successful_requests);
        eprintln!("Failed:             {}", stats.failed_requests);
        eprintln!("Cache Hits:         {}", stats.cached_hits);
        eprintln!("Retried:            {}", stats.retried_requests);
        eprintln!("CB Rejects:         {}", stats.circuit_breaker_rejects);
        eprintln!(
            "Success Rate:       {:.2}%",
            stats.success_rate * 100.0
        );
        eprintln!("Avg Latency:        {:.2}us", stats.average_latency_us);
        eprintln!("Max Latency:        {:.2}us", stats.max_latency_us);
        eprintln!("P99 Latency:        {:.2}us", stats.p99_latency_us);
        eprintln!("Uptime:             {}s", stats.uptime_seconds);
        eprintln!("==================================");
    }

    /// Print metrics summary (no_std version - no-op)
    #[cfg(not(feature = "std"))]
    pub fn print_summary(&self) {
        // No-op in no_std
    }

    // --- Private Helper Methods ---

    /// Update max latency using compare-exchange loop
    #[inline(always)]
    fn update_max_latency(&self, latency_q16: u64) {
        let mut current = self.latency_max_q16.load(Ordering::Acquire);
        loop {
            if latency_q16 <= current {
                break;
            }
            match self.latency_max_q16.compare_exchange_weak(
                current,
                latency_q16,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(c) => current = c,
            }
        }
    }

    /// Update P99 estimate using EMA (exponential moving average)
    ///
    /// Only updates when latency exceeds current P99 estimate.
    /// Uses alpha=0.01 for slow decay (99th percentile estimation).
    #[inline(always)]
    fn update_p99_estimate(&self, latency_q16: u64) {
        let current_p99 = self.latency_p99_q16.load(Ordering::Acquire);

        // Only update if latency exceeds current P99 (contributes to high percentile)
        if latency_q16 > current_p99 {
            // EMA: new_p99 = old_p99 * 0.99 + latency * 0.01
            // Q16.16 fixed-point: 0.99 * 65536 = 64881, 0.01 * 65536 = 655
            const ALPHA_COMPLEMENT_Q16: u64 = 64881; // 0.99
            const ALPHA_Q16: u64 = 655; // 0.01

            // Calculate new P99 with fixed-point arithmetic
            // new_p99 = (current_p99 * 64881 + latency * 655) / 65536
            let weighted_current = (current_p99 as u128 * ALPHA_COMPLEMENT_Q16 as u128) >> 16;
            let weighted_latency = (latency_q16 as u128 * ALPHA_Q16 as u128) >> 16;
            let new_p99 = (weighted_current + weighted_latency) as u64;

            // Atomic update (best-effort, races are acceptable for EMA)
            let _ = self.latency_p99_q16.compare_exchange_weak(
                current_p99,
                new_p99,
                Ordering::AcqRel,
                Ordering::Relaxed,
            );
        }
    }

    /// Convert Q16.16 fixed-point to f64
    #[inline(always)]
    fn q16_16_to_f64(value: u64) -> f64 {
        value as f64 / Q16_16_SCALE as f64
    }

    /// Get current Unix timestamp
    #[inline(always)]
    #[cfg(feature = "std")]
    fn current_unix_timestamp() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Get current Unix timestamp (no_std fallback - returns 0)
    #[inline(always)]
    #[cfg(not(feature = "std"))]
    fn current_unix_timestamp() -> u64 {
        0
    }
}

impl Default for McpMetricsCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time verification
const _: () = {
    assert!(
        core::mem::size_of::<McpMetricsCapsule>() == 128,
        "McpMetricsCapsule must be 128 bytes"
    );
    assert!(
        core::mem::align_of::<McpMetricsCapsule>() == 64,
        "McpMetricsCapsule must be 64-byte aligned"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);
        let stats = metrics.get_stats();

        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.successful_requests, 0);
        assert_eq!(stats.failed_requests, 0);
        assert_eq!(stats.cached_hits, 0);
        assert_eq!(stats.retried_requests, 0);
        assert_eq!(stats.circuit_breaker_rejects, 0);
        assert_eq!(stats.average_latency_us, 0.0);
        assert_eq!(stats.max_latency_us, 0.0);
        assert_eq!(stats.success_rate, 1.0);
        assert_eq!(stats.started_at_unix, 1000);
    }

    #[test]
    fn test_record_successful_request() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(true, 1500, false, 0);

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 0);
        assert!((stats.average_latency_us - 1500.0).abs() < 0.1);
        assert_eq!(stats.success_rate, 1.0);
    }

    #[test]
    fn test_record_failed_request() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(false, 5000, false, 0);

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 0);
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(stats.success_rate, 0.0);
    }

    #[test]
    fn test_cache_hit_tracking() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(true, 100, true, 0);
        metrics.record_request(true, 1000, false, 0);
        metrics.record_request(true, 50, true, 0);

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.cached_hits, 2);
    }

    #[test]
    fn test_retry_tracking() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(true, 1000, false, 0); // No retries
        metrics.record_request(true, 2000, false, 2); // 2 retries
        metrics.record_request(false, 3000, false, 5); // 5 retries

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.retried_requests, 7); // 0 + 2 + 5
    }

    #[test]
    fn test_circuit_breaker_tracking() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(true, 1000, false, 0);
        metrics.record_circuit_breaker_reject();
        metrics.record_circuit_breaker_reject();

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 2);
        assert_eq!(stats.circuit_breaker_rejects, 2);
    }

    #[test]
    fn test_latency_q16_16_conversion() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        // Record various latencies
        metrics.record_request(true, 1000, false, 0); // 1ms
        metrics.record_request(true, 2000, false, 0); // 2ms
        metrics.record_request(true, 3000, false, 0); // 3ms

        let stats = metrics.get_stats();

        // Average should be 2000us
        assert!(
            (stats.average_latency_us - 2000.0).abs() < 1.0,
            "Average latency: {} (expected ~2000)",
            stats.average_latency_us
        );

        // Max should be 3000us
        assert!(
            (stats.max_latency_us - 3000.0).abs() < 1.0,
            "Max latency: {} (expected ~3000)",
            stats.max_latency_us
        );
    }

    #[test]
    fn test_success_rate_calculation() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        // 7 successful, 3 failed = 70% success rate
        for _ in 0..7 {
            metrics.record_request(true, 100, false, 0);
        }
        for _ in 0..3 {
            metrics.record_request(false, 100, false, 0);
        }

        let rate = metrics.success_rate();
        assert!(
            (rate - 0.7).abs() < 0.01,
            "Success rate: {} (expected 0.7)",
            rate
        );
    }

    #[test]
    fn test_average_latency_us() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(true, 1000, false, 0);
        metrics.record_request(true, 2000, false, 0);
        metrics.record_request(true, 3000, false, 0);

        let avg = metrics.average_latency_us();
        assert!(
            (avg - 2000.0).abs() < 1.0,
            "Average: {} (expected 2000)",
            avg
        );
    }

    #[test]
    fn test_max_latency_tracking() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        metrics.record_request(true, 1000, false, 0);
        metrics.record_request(true, 5000, false, 0);
        metrics.record_request(true, 3000, false, 0);
        metrics.record_request(true, 2000, false, 0);

        let stats = metrics.get_stats();
        assert!(
            (stats.max_latency_us - 5000.0).abs() < 1.0,
            "Max: {} (expected 5000)",
            stats.max_latency_us
        );
    }

    #[test]
    fn test_p99_latency_estimate() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);

        // Record mostly low latencies
        for _ in 0..99 {
            metrics.record_request(true, 100, false, 0);
        }
        // Record one high latency (outlier)
        metrics.record_request(true, 10000, false, 0);

        let stats = metrics.get_stats();
        // P99 should be elevated due to the outlier
        assert!(
            stats.p99_latency_us > 100.0,
            "P99: {} (expected > 100)",
            stats.p99_latency_us
        );
    }

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<McpMetricsCapsule>(), 128);
        assert_eq!(core::mem::align_of::<McpMetricsCapsule>(), 64);
    }

    #[test]
    fn test_empty_metrics_success_rate() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);
        // Empty metrics should return 1.0 (100%) success rate
        assert_eq!(metrics.success_rate(), 1.0);
    }

    #[test]
    fn test_empty_metrics_average_latency() {
        let metrics = McpMetricsCapsule::with_timestamp(1000);
        // Empty metrics should return 0.0 average latency
        assert_eq!(metrics.average_latency_us(), 0.0);
    }

    #[test]
    fn test_concurrent_access_safety() {
        use std::sync::Arc;
        use std::thread;

        let metrics = Arc::new(McpMetricsCapsule::with_timestamp(1000));
        let mut handles = vec![];

        // Spawn 10 threads, each recording 100 requests
        for _ in 0..10 {
            let m = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    m.record_request(i % 2 == 0, (i * 10) as u32, i % 3 == 0, (i % 4) as u8);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 1000);
        // 500 even + 500 odd = 500 successful
        assert_eq!(stats.successful_requests, 500);
        assert_eq!(stats.failed_requests, 500);
    }

    #[test]
    fn test_default_impl() {
        let metrics = McpMetricsCapsule::default();
        let stats = metrics.get_stats();
        assert_eq!(stats.total_requests, 0);
    }
}
