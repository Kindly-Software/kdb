//! Prometheus Metrics - /metrics Endpoint
//!
//! **Purpose**: Expose Prometheus-compatible metrics for monitoring.
//!
//! **Response** (text/plain):
//! ```text
//! # TYPE kdb_requests_total counter
//! kdb_requests_total 1234
//!
//! # TYPE kdb_deletion_proofs_issued_total counter
//! kdb_deletion_proofs_issued_total 56
//!
//! # TYPE kdb_quota_exceeded_total counter
//! kdb_quota_exceeded_total 2
//!
//! # TYPE kdb_attach_errors_total counter
//! kdb_attach_errors_total 1
//! ```
//!
//! **Metrics**:
//! - `kdb_requests_total`: Total number of MCP tool requests
//! - `kdb_deletion_proofs_issued_total`: Total deletion proof certificates issued
//! - `kdb_quota_exceeded_total`: Snapshot quota exceeded events
//! - `kdb_attach_errors_total`: Process attach errors (permission denied, etc.)
//!
//! **Tier**: T1 Atomic (lockfree counters, 64B cache-aligned)
//! **Performance**: <5ns per counter update (Relaxed atomic ordering)
//!
//! **Integration**:
//! - Prometheus scrapes /metrics every 15s (configurable)
//! - Grafana dashboards visualize metrics
//! - AlertManager triggers alerts on thresholds

use std::sync::atomic::{AtomicU64, Ordering};

/// Metrics capsule (T1 Atomic, lockfree)
///
/// **Size**: 64B (cache-line aligned, prevents false sharing)
/// **Alignment**: 64 bytes (CPU cache line)
///
/// # Safety
/// All fields are AtomicU64, safe for concurrent access from multiple threads.
#[repr(C, align(64))]
pub struct MetricsCapsule {
    /// Total number of MCP tool requests
    /// Incremented on: debugger.attach, debugger.set_breakpoint, etc.
    requests_total: AtomicU64,

    /// Total deletion proof certificates issued
    /// Incremented on: deletion_certificate creation
    deletion_proofs_issued: AtomicU64,

    /// Snapshot quota exceeded events
    /// Incremented when: max_snapshots exceeded
    quota_exceeded_total: AtomicU64,

    /// Process attach errors (permission_denied, already_attached, etc.)
    /// Incremented when: PTRACE_ATTACH fails
    attach_errors_total: AtomicU64,

    // Padding to align to 64 bytes (8 fields × 8 bytes = 64 bytes exactly)
    _padding0: u64,
    _padding1: u64,
    _padding2: u64,
    _padding3: u64,
}

// Verify size is exactly 64 bytes (one cache line)
#[cfg(test)]
mod size_checks {
    use super::*;

    const _: () = {
        const fn assert_size() {
            const EXPECTED: usize = 64;
            const ACTUAL: usize = std::mem::size_of::<MetricsCapsule>();
            const _: () = assert!(ACTUAL == EXPECTED, "MetricsCapsule must be 64 bytes");
        }
    };
}

impl MetricsCapsule {
    /// Create new metrics capsule
    ///
    /// **Performance**: ~10ns (allocation + atomic initialization)
    pub fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            deletion_proofs_issued: AtomicU64::new(0),
            quota_exceeded_total: AtomicU64::new(0),
            attach_errors_total: AtomicU64::new(0),
            _padding0: 0,
            _padding1: 0,
            _padding2: 0,
            _padding3: 0,
        }
    }

    /// Increment request counter
    ///
    /// **Performance**: <5ns (Relaxed atomic fetch_add)
    /// **Ordering**: Relaxed (no synchronization needed)
    pub fn increment_requests(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment deletion proofs counter
    ///
    /// **Performance**: <5ns (Relaxed atomic fetch_add)
    pub fn increment_deletion_proofs(&self) {
        self.deletion_proofs_issued.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment quota exceeded counter
    ///
    /// **Performance**: <5ns (Relaxed atomic fetch_add)
    pub fn increment_quota_exceeded(&self) {
        self.quota_exceeded_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment attach errors counter
    ///
    /// **Performance**: <5ns (Relaxed atomic fetch_add)
    pub fn increment_attach_errors(&self) {
        self.attach_errors_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current value of requests counter
    pub fn get_requests_total(&self) -> u64 {
        self.requests_total.load(Ordering::Relaxed)
    }

    /// Get current value of deletion proofs counter
    pub fn get_deletion_proofs_total(&self) -> u64 {
        self.deletion_proofs_issued.load(Ordering::Relaxed)
    }

    /// Get current value of quota exceeded counter
    pub fn get_quota_exceeded_total(&self) -> u64 {
        self.quota_exceeded_total.load(Ordering::Relaxed)
    }

    /// Get current value of attach errors counter
    pub fn get_attach_errors_total(&self) -> u64 {
        self.attach_errors_total.load(Ordering::Relaxed)
    }

    /// Export Prometheus format (text/plain)
    ///
    /// **Performance**: ~100ns (4 atomic loads + string formatting)
    /// **Format**: Prometheus exposition format v0.0.4
    ///
    /// # Example Output
    /// ```text
    /// # TYPE kdb_requests_total counter
    /// kdb_requests_total 1234
    ///
    /// # TYPE kdb_deletion_proofs_issued_total counter
    /// kdb_deletion_proofs_issued_total 56
    ///
    /// # TYPE kdb_quota_exceeded_total counter
    /// kdb_quota_exceeded_total 2
    ///
    /// # TYPE kdb_attach_errors_total counter
    /// kdb_attach_errors_total 1
    /// ```
    pub fn export_prometheus(&self) -> String {
        format!(
            "# TYPE kdb_requests_total counter\nkdb_requests_total {}\n\n\
             # TYPE kdb_deletion_proofs_issued_total counter\nkdb_deletion_proofs_issued_total {}\n\n\
             # TYPE kdb_quota_exceeded_total counter\nkdb_quota_exceeded_total {}\n\n\
             # TYPE kdb_attach_errors_total counter\nkdb_attach_errors_total {}\n",
            self.get_requests_total(),
            self.get_deletion_proofs_total(),
            self.get_quota_exceeded_total(),
            self.get_attach_errors_total(),
        )
    }

    /// Reset all counters (for testing)
    ///
    /// **Performance**: ~20ns (4 atomic stores)
    #[cfg(test)]
    pub fn reset(&self) {
        self.requests_total.store(0, Ordering::Relaxed);
        self.deletion_proofs_issued.store(0, Ordering::Relaxed);
        self.quota_exceeded_total.store(0, Ordering::Relaxed);
        self.attach_errors_total.store(0, Ordering::Relaxed);
    }
}

impl Default for MetricsCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Global metrics instance (lazy-initialized)
use std::sync::OnceLock;

static METRICS_INSTANCE: OnceLock<MetricsCapsule> = OnceLock::new();

/// Get global metrics instance
///
/// **Performance**: ~5ns (atomic get from OnceLock)
///
/// # Example
/// ```ignore
/// let metrics = get_metrics_instance();
/// metrics.increment_requests();
/// ```
pub fn get_metrics_instance() -> &'static MetricsCapsule {
    METRICS_INSTANCE.get_or_init(MetricsCapsule::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_metrics_creation() {
        let metrics = MetricsCapsule::new();

        assert_eq!(metrics.get_requests_total(), 0);
        assert_eq!(metrics.get_deletion_proofs_total(), 0);
        assert_eq!(metrics.get_quota_exceeded_total(), 0);
        assert_eq!(metrics.get_attach_errors_total(), 0);
    }

    #[test]
    fn test_request_counter() {
        let metrics = MetricsCapsule::new();

        metrics.increment_requests();
        assert_eq!(metrics.get_requests_total(), 1);

        metrics.increment_requests();
        metrics.increment_requests();
        assert_eq!(metrics.get_requests_total(), 3);
    }

    #[test]
    fn test_deletion_proofs_counter() {
        let metrics = MetricsCapsule::new();

        metrics.increment_deletion_proofs();
        metrics.increment_deletion_proofs();
        assert_eq!(metrics.get_deletion_proofs_total(), 2);
    }

    #[test]
    fn test_quota_exceeded_counter() {
        let metrics = MetricsCapsule::new();

        metrics.increment_quota_exceeded();
        assert_eq!(metrics.get_quota_exceeded_total(), 1);
    }

    #[test]
    fn test_attach_errors_counter() {
        let metrics = MetricsCapsule::new();

        metrics.increment_attach_errors();
        metrics.increment_attach_errors();
        metrics.increment_attach_errors();
        assert_eq!(metrics.get_attach_errors_total(), 3);
    }

    #[test]
    fn test_prometheus_export() {
        let metrics = MetricsCapsule::new();
        metrics.increment_requests();
        metrics.increment_deletion_proofs();
        metrics.increment_quota_exceeded();

        let output = metrics.export_prometheus();

        assert!(output.contains("# TYPE kdb_requests_total counter"));
        assert!(output.contains("kdb_requests_total 1"));
        assert!(output.contains("# TYPE kdb_deletion_proofs_issued_total counter"));
        assert!(output.contains("kdb_deletion_proofs_issued_total 1"));
        assert!(output.contains("# TYPE kdb_quota_exceeded_total counter"));
        assert!(output.contains("kdb_quota_exceeded_total 1"));
        assert!(output.contains("# TYPE kdb_attach_errors_total counter"));
        assert!(output.contains("kdb_attach_errors_total 0"));
    }

    #[test]
    fn test_concurrent_increments() {
        let metrics = std::sync::Arc::new(MetricsCapsule::new());
        let mut handles = vec![];

        // Spawn 10 threads, each incrementing 100 times
        for _ in 0..10 {
            let metrics_clone = std::sync::Arc::clone(&metrics);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    metrics_clone.increment_requests();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have 1000 total increments
        assert_eq!(metrics.get_requests_total(), 1000);
    }

    #[test]
    fn test_global_instance() {
        let metrics1 = get_metrics_instance();
        let metrics2 = get_metrics_instance();

        // Should be the same instance (OnceLock singleton)
        assert_eq!(
            metrics1 as *const _,
            metrics2 as *const _
        );
    }

    #[test]
    fn test_metrics_size() {
        let size = std::mem::size_of::<MetricsCapsule>();
        assert_eq!(size, 64, "MetricsCapsule must be 64 bytes (one cache line)");
    }

    #[test]
    fn test_metrics_alignment() {
        let metrics = MetricsCapsule::new();
        let addr = &metrics as *const _ as usize;
        assert_eq!(
            addr % 64,
            0,
            "MetricsCapsule must be 64-byte aligned"
        );
    }
}
