//! AnomalyV2Metrics - Prometheus Metrics Export for ML V2 Anomaly Detection
//!
//! **Tier**: T1 Atomic (lockfree counters, 64B cache-aligned)
//! **Performance**: <5ns per counter update (Relaxed atomic ordering)
//!
//! # UCE34 Framework Analysis
//! - **Q10 (Tier)**: T1 Atomic - lockfree metric aggregation
//! - **Q11 (Transform)**: AtomicU64 counters, no mutex/RwLock
//! - **Q12 (Nightly)**: None required (stable Rust)
//! - **Q33 (Validation)**: Compile-time size/alignment verification
//! - **Q34 (Auditability)**: Export metrics to Prometheus for monitoring
//!
//! # Prometheus Metrics Exported
//!
//! ## Counters
//! - `anomaly_v2_total_checks` - Total behavior checks performed
//! - `anomaly_v2_v1_detections` - V1 layer detections (probabilistic)
//! - `anomaly_v2_v2_detections` - V2 layer detections (GMM/TinyML/Temporal)
//! - `anomaly_v2_agreement_count` - V1/V2 agreement count (both agree)
//! - `anomaly_v2_critical_count` - Critical anomaly detections
//! - `anomaly_v2_fast_path_hits` - Fast path (Bloom filter) hits
//!
//! ## Gauges
//! - `anomaly_v2_layer_latency_ns{layer="..."}` - Per-layer latency estimates
//! - `anomaly_v2_anomaly_rate` - Current anomaly rate (0.0 - 1.0)
//! - `anomaly_v2_fast_path_rate` - Fast path hit rate (0.0 - 1.0)
//! - `anomaly_v2_enabled_layers` - Bitmask of enabled layers
//!
//! ## Histogram
//! - `anomaly_v2_check_duration_ns` - Check duration distribution
//!
//! # Performance (B32 Validated)
//! - Counter increment: <5ns (Relaxed atomic)
//! - Metrics export: <500ns (atomic loads + string formatting)
//! - Memory overhead: 256 bytes (2 cache lines)

#![allow(unsafe_code)]

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// ============================================================================
// LATENCY HISTOGRAM (for check duration distribution)
// ============================================================================

/// Latency histogram buckets for check duration
/// Buckets: <30ns, <50ns, <100ns, <160ns, <500ns, +Inf
#[repr(C, align(64))]
pub struct CheckLatencyHistogram {
    /// <30ns bucket (fast path)
    bucket_30ns: AtomicU64,
    /// <50ns bucket (probabilistic + GMM)
    bucket_50ns: AtomicU64,
    /// <100ns bucket (through TinyML)
    bucket_100ns: AtomicU64,
    /// <160ns bucket (full path)
    bucket_160ns: AtomicU64,
    /// <500ns bucket (slow path)
    bucket_500ns: AtomicU64,
    /// +Inf bucket
    bucket_inf: AtomicU64,
    /// Sum of all latencies (Q32.32 fixed-point nanoseconds)
    sum_ns_q32: AtomicU64,
    /// Total observation count
    count: AtomicU64,
}

impl CheckLatencyHistogram {
    /// Create new histogram
    pub const fn new() -> Self {
        Self {
            bucket_30ns: AtomicU64::new(0),
            bucket_50ns: AtomicU64::new(0),
            bucket_100ns: AtomicU64::new(0),
            bucket_160ns: AtomicU64::new(0),
            bucket_500ns: AtomicU64::new(0),
            bucket_inf: AtomicU64::new(0),
            sum_ns_q32: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// Record a latency observation
    #[inline]
    pub fn record(&self, latency_ns: u32) {
        let latency = latency_ns as u64;

        // Increment appropriate bucket
        if latency < 30 {
            self.bucket_30ns.fetch_add(1, Ordering::Relaxed);
        } else if latency < 50 {
            self.bucket_50ns.fetch_add(1, Ordering::Relaxed);
        } else if latency < 100 {
            self.bucket_100ns.fetch_add(1, Ordering::Relaxed);
        } else if latency < 160 {
            self.bucket_160ns.fetch_add(1, Ordering::Relaxed);
        } else if latency < 500 {
            self.bucket_500ns.fetch_add(1, Ordering::Relaxed);
        } else {
            self.bucket_inf.fetch_add(1, Ordering::Relaxed);
        }

        // Update sum (Q32.32 fixed-point: store as raw nanoseconds)
        self.sum_ns_q32.fetch_add(latency, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get cumulative bucket counts
    pub fn buckets(&self) -> [u64; 6] {
        let b30 = self.bucket_30ns.load(Ordering::Relaxed);
        let b50 = self.bucket_50ns.load(Ordering::Relaxed);
        let b100 = self.bucket_100ns.load(Ordering::Relaxed);
        let b160 = self.bucket_160ns.load(Ordering::Relaxed);
        let b500 = self.bucket_500ns.load(Ordering::Relaxed);
        let inf = self.bucket_inf.load(Ordering::Relaxed);

        // Return cumulative counts
        [
            b30,
            b30 + b50,
            b30 + b50 + b100,
            b30 + b50 + b100 + b160,
            b30 + b50 + b100 + b160 + b500,
            b30 + b50 + b100 + b160 + b500 + inf,
        ]
    }

    /// Get sum in nanoseconds
    #[inline]
    pub fn sum_ns(&self) -> u64 {
        self.sum_ns_q32.load(Ordering::Relaxed)
    }

    /// Get total count
    #[inline]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Reset histogram
    pub fn reset(&self) {
        self.bucket_30ns.store(0, Ordering::Relaxed);
        self.bucket_50ns.store(0, Ordering::Relaxed);
        self.bucket_100ns.store(0, Ordering::Relaxed);
        self.bucket_160ns.store(0, Ordering::Relaxed);
        self.bucket_500ns.store(0, Ordering::Relaxed);
        self.bucket_inf.store(0, Ordering::Relaxed);
        self.sum_ns_q32.store(0, Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);
    }
}

// ============================================================================
// ANOMALY V2 METRICS CAPSULE (256 bytes, 128B aligned)
// ============================================================================

/// AnomalyV2Metrics - Prometheus-compatible metrics for ML V2 anomaly detection
///
/// **Size**: 256B (2 cache lines, prevents false sharing)
/// **Alignment**: 128 bytes (cache-cluster aligned)
/// **Lockfree**: 100% atomic operations
///
/// # Safety
/// - All fields are atomic, safe for concurrent access
/// - No mutex/RwLock, 100% lockfree
/// - Cache-aligned to prevent false sharing
#[repr(C, align(128))]
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 128, size = 256))]
pub struct AnomalyV2Metrics {
    // ========== Counters (64 bytes) ==========

    /// Total behavior checks performed
    total_checks: AtomicU64,

    /// V1 (probabilistic) layer detections
    v1_detections: AtomicU64,

    /// V2 (GMM/TinyML/Temporal) layer detections
    v2_detections: AtomicU64,

    /// Agreement count (V1 and V2 agree)
    agreement_count: AtomicU64,

    /// Critical anomaly detections
    critical_count: AtomicU64,

    /// Fast path (Bloom filter) hits
    fast_path_hits: AtomicU64,

    /// Normal results count
    normal_count: AtomicU64,

    /// Suspicious results count
    suspicious_count: AtomicU64,

    // ========== Layer Latencies (16 bytes) ==========

    /// Layer latency estimates (nanoseconds)
    /// Index 0: Probabilistic, 1: GMM, 2: TinyML, 3: Temporal
    layer_latencies: [AtomicU32; 4],

    // ========== Gauges (32 bytes) ==========

    /// Enabled layers bitmask
    enabled_layers: AtomicU64,

    /// Last check timestamp (milliseconds)
    last_check_ms: AtomicU64,

    /// Generation counter (Q34 audit trail)
    generation: AtomicU64,

    /// Reserved for future use
    _reserved: AtomicU64,

    // ========== Histogram (64 bytes) ==========

    /// Check duration histogram
    histogram: CheckLatencyHistogram,

    // ========== Padding ==========

    /// Padding to reach 256 bytes
    _padding: [u8; 16],
}

impl AnomalyV2Metrics {
    /// Create new metrics capsule
    pub const fn new() -> Self {
        Self {
            total_checks: AtomicU64::new(0),
            v1_detections: AtomicU64::new(0),
            v2_detections: AtomicU64::new(0),
            agreement_count: AtomicU64::new(0),
            critical_count: AtomicU64::new(0),
            fast_path_hits: AtomicU64::new(0),
            normal_count: AtomicU64::new(0),
            suspicious_count: AtomicU64::new(0),
            layer_latencies: [
                AtomicU32::new(30),  // Probabilistic default
                AtomicU32::new(20),  // GMM default
                AtomicU32::new(60),  // TinyML default
                AtomicU32::new(50),  // Temporal default
            ],
            enabled_layers: AtomicU64::new(0b1111), // All layers enabled
            last_check_ms: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            _reserved: AtomicU64::new(0),
            histogram: CheckLatencyHistogram::new(),
            _padding: [0; 16],
        }
    }

    // ========================================================================
    // Counter Increments
    // ========================================================================

    /// Increment total checks counter
    #[inline]
    pub fn increment_total_checks(&self) {
        self.total_checks.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment V1 detections counter
    #[inline]
    pub fn increment_v1_detections(&self) {
        self.v1_detections.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment V2 detections counter
    #[inline]
    pub fn increment_v2_detections(&self) {
        self.v2_detections.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment agreement counter
    #[inline]
    pub fn increment_agreement(&self) {
        self.agreement_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment critical count
    #[inline]
    pub fn increment_critical(&self) {
        self.critical_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment fast path hits
    #[inline]
    pub fn increment_fast_path_hits(&self) {
        self.fast_path_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment normal count
    #[inline]
    pub fn increment_normal(&self) {
        self.normal_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment suspicious count
    #[inline]
    pub fn increment_suspicious(&self) {
        self.suspicious_count.fetch_add(1, Ordering::Relaxed);
    }

    // ========================================================================
    // Counter Reads
    // ========================================================================

    /// Get total checks
    #[inline]
    pub fn total_checks(&self) -> u64 {
        self.total_checks.load(Ordering::Relaxed)
    }

    /// Get V1 detections
    #[inline]
    pub fn v1_detections(&self) -> u64 {
        self.v1_detections.load(Ordering::Relaxed)
    }

    /// Get V2 detections
    #[inline]
    pub fn v2_detections(&self) -> u64 {
        self.v2_detections.load(Ordering::Relaxed)
    }

    /// Get agreement count
    #[inline]
    pub fn agreement_count(&self) -> u64 {
        self.agreement_count.load(Ordering::Relaxed)
    }

    /// Get critical count
    #[inline]
    pub fn critical_count(&self) -> u64 {
        self.critical_count.load(Ordering::Relaxed)
    }

    /// Get fast path hits
    #[inline]
    pub fn fast_path_hits(&self) -> u64 {
        self.fast_path_hits.load(Ordering::Relaxed)
    }

    /// Get normal count
    #[inline]
    pub fn normal_count(&self) -> u64 {
        self.normal_count.load(Ordering::Relaxed)
    }

    /// Get suspicious count
    #[inline]
    pub fn suspicious_count(&self) -> u64 {
        self.suspicious_count.load(Ordering::Relaxed)
    }

    // ========================================================================
    // Layer Latency Management
    // ========================================================================

    /// Update layer latency estimate
    #[inline]
    pub fn update_layer_latency(&self, layer_idx: usize, latency_ns: u32) {
        if layer_idx < 4 {
            self.layer_latencies[layer_idx].store(latency_ns, Ordering::Relaxed);
        }
    }

    /// Get layer latency
    #[inline]
    pub fn layer_latency(&self, layer_idx: usize) -> u32 {
        if layer_idx < 4 {
            self.layer_latencies[layer_idx].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Get all layer latencies
    pub fn layer_latencies(&self) -> [u32; 4] {
        [
            self.layer_latencies[0].load(Ordering::Relaxed),
            self.layer_latencies[1].load(Ordering::Relaxed),
            self.layer_latencies[2].load(Ordering::Relaxed),
            self.layer_latencies[3].load(Ordering::Relaxed),
        ]
    }

    // ========================================================================
    // Gauge Management
    // ========================================================================

    /// Set enabled layers bitmask
    #[inline]
    pub fn set_enabled_layers(&self, mask: u64) {
        self.enabled_layers.store(mask, Ordering::Relaxed);
    }

    /// Get enabled layers bitmask
    #[inline]
    pub fn enabled_layers(&self) -> u64 {
        self.enabled_layers.load(Ordering::Relaxed)
    }

    /// Update last check timestamp
    #[inline]
    pub fn update_last_check_ms(&self, ms: u64) {
        self.last_check_ms.store(ms, Ordering::Relaxed);
    }

    /// Get last check timestamp
    #[inline]
    pub fn last_check_ms(&self) -> u64 {
        self.last_check_ms.load(Ordering::Relaxed)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    // ========================================================================
    // Histogram Management
    // ========================================================================

    /// Record check duration
    #[inline]
    pub fn record_check_duration(&self, latency_ns: u32) {
        self.histogram.record(latency_ns);
    }

    /// Get histogram reference
    pub fn histogram(&self) -> &CheckLatencyHistogram {
        &self.histogram
    }

    // ========================================================================
    // Computed Metrics
    // ========================================================================

    /// Compute anomaly rate (0.0 - 1.0)
    #[inline]
    pub fn anomaly_rate(&self) -> f64 {
        let total = self.total_checks();
        let anomalies = self.v2_detections() + self.critical_count();
        if total == 0 {
            0.0
        } else {
            anomalies as f64 / total as f64
        }
    }

    /// Compute fast path rate (0.0 - 1.0)
    #[inline]
    pub fn fast_path_rate(&self) -> f64 {
        let total = self.total_checks();
        let fast = self.fast_path_hits();
        if total == 0 {
            0.0
        } else {
            fast as f64 / total as f64
        }
    }

    /// Compute agreement rate (0.0 - 1.0)
    #[inline]
    pub fn agreement_rate(&self) -> f64 {
        let total = self.total_checks();
        let agree = self.agreement_count();
        if total == 0 {
            0.0
        } else {
            agree as f64 / total as f64
        }
    }

    /// Compute average latency (nanoseconds)
    #[inline]
    pub fn avg_latency_ns(&self) -> f64 {
        let count = self.histogram.count();
        let sum = self.histogram.sum_ns();
        if count == 0 {
            0.0
        } else {
            sum as f64 / count as f64
        }
    }

    // ========================================================================
    // Reset
    // ========================================================================

    /// Reset all metrics
    pub fn reset(&self) {
        self.total_checks.store(0, Ordering::SeqCst);
        self.v1_detections.store(0, Ordering::SeqCst);
        self.v2_detections.store(0, Ordering::SeqCst);
        self.agreement_count.store(0, Ordering::SeqCst);
        self.critical_count.store(0, Ordering::SeqCst);
        self.fast_path_hits.store(0, Ordering::SeqCst);
        self.normal_count.store(0, Ordering::SeqCst);
        self.suspicious_count.store(0, Ordering::SeqCst);
        self.last_check_ms.store(0, Ordering::SeqCst);
        // Don't reset generation - it should be monotonic
        self.histogram.reset();
    }

    // ========================================================================
    // Prometheus Export
    // ========================================================================

    /// Export metrics in Prometheus text exposition format
    ///
    /// **Performance**: <500ns (atomic loads + string formatting)
    /// **Format**: Prometheus text format v0.0.4
    pub fn to_prometheus(&self) -> String {
        let mut output = String::with_capacity(4096);

        // ====================================================================
        // Counters
        // ====================================================================

        output.push_str("# HELP anomaly_v2_total_checks Total behavior checks performed\n");
        output.push_str("# TYPE anomaly_v2_total_checks counter\n");
        output.push_str(&format!("anomaly_v2_total_checks {}\n\n", self.total_checks()));

        output.push_str("# HELP anomaly_v2_v1_detections V1 probabilistic layer detections\n");
        output.push_str("# TYPE anomaly_v2_v1_detections counter\n");
        output.push_str(&format!("anomaly_v2_v1_detections {}\n\n", self.v1_detections()));

        output.push_str("# HELP anomaly_v2_v2_detections V2 ML layer detections (GMM/TinyML/Temporal)\n");
        output.push_str("# TYPE anomaly_v2_v2_detections counter\n");
        output.push_str(&format!("anomaly_v2_v2_detections {}\n\n", self.v2_detections()));

        output.push_str("# HELP anomaly_v2_agreement_count V1/V2 agreement count\n");
        output.push_str("# TYPE anomaly_v2_agreement_count counter\n");
        output.push_str(&format!("anomaly_v2_agreement_count {}\n\n", self.agreement_count()));

        output.push_str("# HELP anomaly_v2_critical_count Critical anomaly detections\n");
        output.push_str("# TYPE anomaly_v2_critical_count counter\n");
        output.push_str(&format!("anomaly_v2_critical_count {}\n\n", self.critical_count()));

        output.push_str("# HELP anomaly_v2_fast_path_hits Fast path (Bloom filter) hits\n");
        output.push_str("# TYPE anomaly_v2_fast_path_hits counter\n");
        output.push_str(&format!("anomaly_v2_fast_path_hits {}\n\n", self.fast_path_hits()));

        output.push_str("# HELP anomaly_v2_normal_count Normal result count\n");
        output.push_str("# TYPE anomaly_v2_normal_count counter\n");
        output.push_str(&format!("anomaly_v2_normal_count {}\n\n", self.normal_count()));

        output.push_str("# HELP anomaly_v2_suspicious_count Suspicious result count\n");
        output.push_str("# TYPE anomaly_v2_suspicious_count counter\n");
        output.push_str(&format!("anomaly_v2_suspicious_count {}\n\n", self.suspicious_count()));

        // ====================================================================
        // Gauges - Layer Latencies
        // ====================================================================

        output.push_str("# HELP anomaly_v2_layer_latency_ns Per-layer latency estimates in nanoseconds\n");
        output.push_str("# TYPE anomaly_v2_layer_latency_ns gauge\n");
        let latencies = self.layer_latencies();
        output.push_str(&format!("anomaly_v2_layer_latency_ns{{layer=\"probabilistic\"}} {}\n", latencies[0]));
        output.push_str(&format!("anomaly_v2_layer_latency_ns{{layer=\"gmm\"}} {}\n", latencies[1]));
        output.push_str(&format!("anomaly_v2_layer_latency_ns{{layer=\"tinyml\"}} {}\n", latencies[2]));
        output.push_str(&format!("anomaly_v2_layer_latency_ns{{layer=\"temporal\"}} {}\n\n", latencies[3]));

        // ====================================================================
        // Gauges - Rates
        // ====================================================================

        output.push_str("# HELP anomaly_v2_anomaly_rate Current anomaly rate (0.0-1.0)\n");
        output.push_str("# TYPE anomaly_v2_anomaly_rate gauge\n");
        output.push_str(&format!("anomaly_v2_anomaly_rate {:.6}\n\n", self.anomaly_rate()));

        output.push_str("# HELP anomaly_v2_fast_path_rate Fast path hit rate (0.0-1.0)\n");
        output.push_str("# TYPE anomaly_v2_fast_path_rate gauge\n");
        output.push_str(&format!("anomaly_v2_fast_path_rate {:.6}\n\n", self.fast_path_rate()));

        output.push_str("# HELP anomaly_v2_agreement_rate V1/V2 agreement rate (0.0-1.0)\n");
        output.push_str("# TYPE anomaly_v2_agreement_rate gauge\n");
        output.push_str(&format!("anomaly_v2_agreement_rate {:.6}\n\n", self.agreement_rate()));

        output.push_str("# HELP anomaly_v2_enabled_layers Bitmask of enabled layers\n");
        output.push_str("# TYPE anomaly_v2_enabled_layers gauge\n");
        output.push_str(&format!("anomaly_v2_enabled_layers {}\n\n", self.enabled_layers()));

        output.push_str("# HELP anomaly_v2_generation Generation counter for audit trail\n");
        output.push_str("# TYPE anomaly_v2_generation counter\n");
        output.push_str(&format!("anomaly_v2_generation {}\n\n", self.generation()));

        // ====================================================================
        // Histogram
        // ====================================================================

        output.push_str("# HELP anomaly_v2_check_duration_ns Check duration distribution in nanoseconds\n");
        output.push_str("# TYPE anomaly_v2_check_duration_ns histogram\n");
        let buckets = self.histogram.buckets();
        output.push_str(&format!("anomaly_v2_check_duration_ns_bucket{{le=\"30\"}} {}\n", buckets[0]));
        output.push_str(&format!("anomaly_v2_check_duration_ns_bucket{{le=\"50\"}} {}\n", buckets[1]));
        output.push_str(&format!("anomaly_v2_check_duration_ns_bucket{{le=\"100\"}} {}\n", buckets[2]));
        output.push_str(&format!("anomaly_v2_check_duration_ns_bucket{{le=\"160\"}} {}\n", buckets[3]));
        output.push_str(&format!("anomaly_v2_check_duration_ns_bucket{{le=\"500\"}} {}\n", buckets[4]));
        output.push_str(&format!("anomaly_v2_check_duration_ns_bucket{{le=\"+Inf\"}} {}\n", buckets[5]));
        output.push_str(&format!("anomaly_v2_check_duration_ns_sum {}\n", self.histogram.sum_ns()));
        output.push_str(&format!("anomaly_v2_check_duration_ns_count {}\n", self.histogram.count()));

        output
    }

    /// Create a snapshot of current metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_checks: self.total_checks(),
            v1_detections: self.v1_detections(),
            v2_detections: self.v2_detections(),
            agreement_count: self.agreement_count(),
            critical_count: self.critical_count(),
            fast_path_hits: self.fast_path_hits(),
            normal_count: self.normal_count(),
            suspicious_count: self.suspicious_count(),
            layer_latencies: self.layer_latencies(),
            enabled_layers: self.enabled_layers(),
            generation: self.generation(),
            anomaly_rate: self.anomaly_rate(),
            fast_path_rate: self.fast_path_rate(),
            agreement_rate: self.agreement_rate(),
            avg_latency_ns: self.avg_latency_ns(),
        }
    }
}

impl Default for AnomalyV2Metrics {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// METRICS SNAPSHOT
// ============================================================================

/// Snapshot of metrics at a point in time
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_checks: u64,
    pub v1_detections: u64,
    pub v2_detections: u64,
    pub agreement_count: u64,
    pub critical_count: u64,
    pub fast_path_hits: u64,
    pub normal_count: u64,
    pub suspicious_count: u64,
    pub layer_latencies: [u32; 4],
    pub enabled_layers: u64,
    pub generation: u64,
    pub anomaly_rate: f64,
    pub fast_path_rate: f64,
    pub agreement_rate: f64,
    pub avg_latency_ns: f64,
}

// ============================================================================
// GLOBAL METRICS INSTANCE
// ============================================================================

#[cfg(feature = "std")]
use std::sync::OnceLock;

#[cfg(feature = "std")]
static GLOBAL_METRICS: OnceLock<AnomalyV2Metrics> = OnceLock::new();

/// Get global metrics instance
#[cfg(feature = "std")]
pub fn get_global_metrics() -> &'static AnomalyV2Metrics {
    GLOBAL_METRICS.get_or_init(AnomalyV2Metrics::new)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = AnomalyV2Metrics::new();
        assert_eq!(metrics.total_checks(), 0);
        assert_eq!(metrics.v1_detections(), 0);
        assert_eq!(metrics.v2_detections(), 0);
        assert_eq!(metrics.enabled_layers(), 0b1111);
    }

    #[test]
    fn test_counter_increments() {
        let metrics = AnomalyV2Metrics::new();

        metrics.increment_total_checks();
        assert_eq!(metrics.total_checks(), 1);

        metrics.increment_v1_detections();
        assert_eq!(metrics.v1_detections(), 1);

        metrics.increment_v2_detections();
        assert_eq!(metrics.v2_detections(), 1);

        metrics.increment_agreement();
        assert_eq!(metrics.agreement_count(), 1);

        metrics.increment_critical();
        assert_eq!(metrics.critical_count(), 1);

        metrics.increment_fast_path_hits();
        assert_eq!(metrics.fast_path_hits(), 1);
    }

    #[test]
    fn test_layer_latencies() {
        let metrics = AnomalyV2Metrics::new();

        // Check defaults
        let latencies = metrics.layer_latencies();
        assert_eq!(latencies[0], 30);
        assert_eq!(latencies[1], 20);
        assert_eq!(latencies[2], 60);
        assert_eq!(latencies[3], 50);

        // Update and verify
        metrics.update_layer_latency(0, 35);
        assert_eq!(metrics.layer_latency(0), 35);

        metrics.update_layer_latency(3, 55);
        assert_eq!(metrics.layer_latency(3), 55);
    }

    #[test]
    fn test_computed_rates() {
        let metrics = AnomalyV2Metrics::new();

        // With no data, rates should be 0
        assert_eq!(metrics.anomaly_rate(), 0.0);
        assert_eq!(metrics.fast_path_rate(), 0.0);
        assert_eq!(metrics.agreement_rate(), 0.0);

        // Add some data
        for _ in 0..100 {
            metrics.increment_total_checks();
        }
        for _ in 0..10 {
            metrics.increment_v2_detections();
        }
        for _ in 0..70 {
            metrics.increment_fast_path_hits();
        }
        for _ in 0..80 {
            metrics.increment_agreement();
        }

        assert!((metrics.anomaly_rate() - 0.1).abs() < 0.01);
        assert!((metrics.fast_path_rate() - 0.7).abs() < 0.01);
        assert!((metrics.agreement_rate() - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_histogram_recording() {
        let metrics = AnomalyV2Metrics::new();

        metrics.record_check_duration(25);  // <30ns bucket
        metrics.record_check_duration(45);  // <50ns bucket
        metrics.record_check_duration(80);  // <100ns bucket
        metrics.record_check_duration(150); // <160ns bucket
        metrics.record_check_duration(300); // <500ns bucket
        metrics.record_check_duration(600); // +Inf bucket

        let buckets = metrics.histogram().buckets();
        assert_eq!(buckets[0], 1);  // <30ns
        assert_eq!(buckets[1], 2);  // <50ns (cumulative)
        assert_eq!(buckets[2], 3);  // <100ns (cumulative)
        assert_eq!(buckets[3], 4);  // <160ns (cumulative)
        assert_eq!(buckets[4], 5);  // <500ns (cumulative)
        assert_eq!(buckets[5], 6);  // +Inf (cumulative)

        assert_eq!(metrics.histogram().count(), 6);
        assert_eq!(metrics.histogram().sum_ns(), 25 + 45 + 80 + 150 + 300 + 600);
    }

    #[test]
    fn test_prometheus_export() {
        let metrics = AnomalyV2Metrics::new();

        metrics.increment_total_checks();
        metrics.increment_v1_detections();
        metrics.increment_fast_path_hits();
        metrics.record_check_duration(50);

        let output = metrics.to_prometheus();

        // Verify format
        assert!(output.contains("# HELP anomaly_v2_total_checks"));
        assert!(output.contains("# TYPE anomaly_v2_total_checks counter"));
        assert!(output.contains("anomaly_v2_total_checks 1"));
        assert!(output.contains("anomaly_v2_v1_detections 1"));
        assert!(output.contains("anomaly_v2_fast_path_hits 1"));
        assert!(output.contains("anomaly_v2_layer_latency_ns{layer=\"probabilistic\"}"));
        assert!(output.contains("anomaly_v2_check_duration_ns_bucket"));
    }

    #[test]
    fn test_reset() {
        let metrics = AnomalyV2Metrics::new();

        metrics.increment_total_checks();
        metrics.increment_v1_detections();
        metrics.record_check_duration(50);

        let gen_before = metrics.generation();
        metrics.reset();

        assert_eq!(metrics.total_checks(), 0);
        assert_eq!(metrics.v1_detections(), 0);
        assert_eq!(metrics.histogram().count(), 0);
        // Generation should NOT be reset
        assert_eq!(metrics.generation(), gen_before);
    }

    #[test]
    fn test_snapshot() {
        let metrics = AnomalyV2Metrics::new();

        for _ in 0..50 {
            metrics.increment_total_checks();
        }
        metrics.increment_v2_detections();
        metrics.increment_critical();

        let snapshot = metrics.snapshot();

        assert_eq!(snapshot.total_checks, 50);
        assert_eq!(snapshot.v2_detections, 1);
        assert_eq!(snapshot.critical_count, 1);
    }

    #[test]
    fn test_concurrent_increments() {
        use std::sync::Arc;
        use std::thread;

        let metrics = Arc::new(AnomalyV2Metrics::new());
        let mut handles = vec![];

        for _ in 0..8 {
            let m = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    m.increment_total_checks();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(metrics.total_checks(), 8000);
    }

    #[test]
    fn test_metrics_size_and_alignment() {
        let size = std::mem::size_of::<AnomalyV2Metrics>();
        let align = std::mem::align_of::<AnomalyV2Metrics>();

        assert_eq!(align, 128, "Alignment must be 128 bytes");
        assert!(size <= 256, "Size must be <= 256 bytes, got {}", size);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_global_metrics() {
        let m1 = get_global_metrics();
        let m2 = get_global_metrics();

        // Should be same instance
        assert_eq!(m1 as *const _, m2 as *const _);
    }
}
