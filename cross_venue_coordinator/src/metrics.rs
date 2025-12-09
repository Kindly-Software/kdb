//! Performance Metrics and Monitoring
//!
//! Comprehensive metrics collection and performance monitoring
//! for cross-venue coordination operations.

use core::sync::atomic::{AtomicU64, Ordering};
use serde::{Deserialize, Serialize};
use crate::types::{VenueId, CoordinationStats};

/// Coordination metrics for performance monitoring
#[derive(Debug)]
#[repr(C, align(64))]
pub struct CoordinationMetrics {
    /// Total coordination operations
    operations: AtomicU64,
    /// Successful operations
    successful_operations: AtomicU64,
    /// Failed operations
    failed_operations: AtomicU64,
    /// Total latency sum for average calculation
    total_latency_ns: AtomicU64,
    /// Maximum observed latency
    max_latency_ns: AtomicU64,
    /// Minimum observed latency
    min_latency_ns: AtomicU64,
    /// Last operation timestamp
    last_operation_ns: AtomicU64,
    /// Cache line padding
    _padding: [u8; 8],
}

impl CoordinationMetrics {
    /// Create new metrics instance
    pub const fn new() -> Self {
        Self {
            operations: AtomicU64::new(0),
            successful_operations: AtomicU64::new(0),
            failed_operations: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
            max_latency_ns: AtomicU64::new(0),
            min_latency_ns: AtomicU64::new(u64::MAX),
            last_operation_ns: AtomicU64::new(0),
            _padding: [0; 8],
        }
    }

    /// Record successful operation
    pub fn record_operation_success(&self) {
        self.operations.fetch_add(1, Ordering::Relaxed);
        self.successful_operations.fetch_add(1, Ordering::Relaxed);
        self.update_timestamp();
    }

    /// Record failed operation
    pub fn record_operation_failure(&self) {
        self.operations.fetch_add(1, Ordering::Relaxed);
        self.failed_operations.fetch_add(1, Ordering::Relaxed);
        self.update_timestamp();
    }

    /// Record operation latency
    pub fn record_latency(&self, latency_ns: u64) {
        self.total_latency_ns.fetch_add(latency_ns, Ordering::Relaxed);

        // Update max latency
        let mut current_max = self.max_latency_ns.load(Ordering::Relaxed);
        while latency_ns > current_max {
            match self.max_latency_ns.compare_exchange_weak(
                current_max,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_max = actual,
            }
        }

        // Update min latency
        let mut current_min = self.min_latency_ns.load(Ordering::Relaxed);
        while latency_ns < current_min {
            match self.min_latency_ns.compare_exchange_weak(
                current_min,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_min = actual,
            }
        }
    }

    /// Get performance counters snapshot
    pub fn snapshot(&self) -> PerformanceCounters {
        let operations = self.operations.load(Ordering::Relaxed);
        let successful = self.successful_operations.load(Ordering::Relaxed);
        let failed = self.failed_operations.load(Ordering::Relaxed);
        let total_latency = self.total_latency_ns.load(Ordering::Relaxed);
        let max_latency = self.max_latency_ns.load(Ordering::Relaxed);
        let min_latency = self.min_latency_ns.load(Ordering::Relaxed);
        let last_operation = self.last_operation_ns.load(Ordering::Relaxed);

        let avg_latency = if operations > 0 {
            total_latency / operations
        } else {
            0
        };

        let success_rate = if operations > 0 {
            (successful as f64 / operations as f64) * 100.0
        } else {
            0.0
        };

        PerformanceCounters {
            total_operations: operations,
            successful_operations: successful,
            failed_operations: failed,
            success_rate,
            avg_latency_ns: avg_latency,
            min_latency_ns: if min_latency == u64::MAX { 0 } else { min_latency },
            max_latency_ns: max_latency,
            last_operation_ns: last_operation,
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.operations.store(0, Ordering::Relaxed);
        self.successful_operations.store(0, Ordering::Relaxed);
        self.failed_operations.store(0, Ordering::Relaxed);
        self.total_latency_ns.store(0, Ordering::Relaxed);
        self.max_latency_ns.store(0, Ordering::Relaxed);
        self.min_latency_ns.store(u64::MAX, Ordering::Relaxed);
        self.last_operation_ns.store(0, Ordering::Relaxed);
    }

    /// Update timestamp
    fn update_timestamp(&self) {
        #[cfg(feature = "std")]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            if let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
                self.last_operation_ns.store(now.as_nanos() as u64, Ordering::Relaxed);
            }
        }
    }
}

/// Performance counters snapshot
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceCounters {
    /// Total operations performed
    pub total_operations: u64,
    /// Successful operations
    pub successful_operations: u64,
    /// Failed operations
    pub failed_operations: u64,
    /// Success rate percentage
    pub success_rate: f64,
    /// Average latency in nanoseconds
    pub avg_latency_ns: u64,
    /// Minimum latency in nanoseconds
    pub min_latency_ns: u64,
    /// Maximum latency in nanoseconds
    pub max_latency_ns: u64,
    /// Last operation timestamp
    pub last_operation_ns: u64,
}

impl PerformanceCounters {
    /// Calculate operations per second
    pub fn operations_per_second(&self, window_ns: u64) -> f64 {
        if window_ns == 0 {
            0.0
        } else {
            (self.total_operations as f64) * 1_000_000_000.0 / (window_ns as f64)
        }
    }

    /// Check if performance meets targets
    pub fn meets_targets(&self, min_success_rate: f64, max_avg_latency_ns: u64) -> bool {
        self.success_rate >= min_success_rate && self.avg_latency_ns <= max_avg_latency_ns
    }

    /// Calculate failure rate
    pub fn failure_rate(&self) -> f64 {
        100.0 - self.success_rate
    }
}

/// Per-venue metrics tracking
#[derive(Debug)]
#[repr(C, align(64))]
pub struct VenueMetrics {
    /// Venue ID
    venue_id: VenueId,
    /// Venue-specific counters
    counters: CoordinationMetrics,
    /// Last health check result
    last_health_score: AtomicU64, // f64 as u64 bits
    /// Venue availability percentage
    availability: AtomicU64, // f64 as u64 bits
    /// Circuit breaker activation count
    circuit_breaker_activations: AtomicU64,
}

impl VenueMetrics {
    /// Create new venue metrics
    pub fn new(venue_id: VenueId) -> Self {
        Self {
            venue_id,
            counters: CoordinationMetrics::new(),
            last_health_score: AtomicU64::new(0),
            availability: AtomicU64::new(f64::to_bits(100.0)),
            circuit_breaker_activations: AtomicU64::new(0),
        }
    }

    /// Record venue operation
    pub fn record_operation(&self, success: bool, latency_ns: u64) {
        if success {
            self.counters.record_operation_success();
        } else {
            self.counters.record_operation_failure();
        }
        self.counters.record_latency(latency_ns);
    }

    /// Update health score
    pub fn update_health_score(&self, score: f64) {
        self.last_health_score.store(score.to_bits(), Ordering::Relaxed);
    }

    /// Update availability percentage
    pub fn update_availability(&self, availability: f64) {
        self.availability.store(availability.to_bits(), Ordering::Relaxed);
    }

    /// Record circuit breaker activation
    pub fn record_circuit_breaker_activation(&self) {
        self.circuit_breaker_activations.fetch_add(1, Ordering::Relaxed);
    }

    /// Get venue metrics snapshot
    pub fn snapshot(&self) -> VenueMetricsSnapshot {
        let counters = self.counters.snapshot();
        let health_score = f64::from_bits(self.last_health_score.load(Ordering::Relaxed));
        let availability = f64::from_bits(self.availability.load(Ordering::Relaxed));
        let cb_activations = self.circuit_breaker_activations.load(Ordering::Relaxed);

        VenueMetricsSnapshot {
            venue_id: self.venue_id,
            performance: counters,
            health_score,
            availability,
            circuit_breaker_activations: cb_activations,
        }
    }

    /// Get venue ID
    pub fn venue_id(&self) -> VenueId {
        self.venue_id
    }
}

/// Venue metrics snapshot
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VenueMetricsSnapshot {
    /// Venue ID
    pub venue_id: VenueId,
    /// Performance counters
    pub performance: PerformanceCounters,
    /// Health score (0.0 to 1.0)
    pub health_score: f64,
    /// Availability percentage
    pub availability: f64,
    /// Circuit breaker activations
    pub circuit_breaker_activations: u64,
}

impl VenueMetricsSnapshot {
    /// Check if venue is performing well
    pub fn is_performing_well(&self) -> bool {
        self.health_score >= 0.8 &&
        self.availability >= 95.0 &&
        self.performance.success_rate >= 95.0
    }

    /// Calculate overall venue score
    pub fn overall_score(&self) -> f64 {
        let performance_score = self.performance.success_rate / 100.0;
        let availability_score = self.availability / 100.0;
        let cb_penalty = if self.circuit_breaker_activations > 0 {
            0.9f64.powi(self.circuit_breaker_activations as i32)
        } else {
            1.0
        };

        (self.health_score + performance_score + availability_score) / 3.0 * cb_penalty
    }
}

/// System-wide metrics aggregator
#[derive(Debug)]
pub struct SystemMetrics {
    /// Global coordination metrics
    global_metrics: CoordinationMetrics,
    /// Per-venue metrics
    venue_metrics: Vec<VenueMetrics>,
    /// System start time
    start_time_ns: u64,
}

impl SystemMetrics {
    /// Create new system metrics
    pub fn new(num_venues: usize) -> Self {
        let venue_metrics = (0..num_venues)
            .map(VenueMetrics::new)
            .collect();

        Self {
            global_metrics: CoordinationMetrics::new(),
            venue_metrics,
            start_time_ns: Self::current_timestamp_ns(),
        }
    }

    /// Record global operation
    pub fn record_global_operation(&self, success: bool, latency_ns: u64) {
        if success {
            self.global_metrics.record_operation_success();
        } else {
            self.global_metrics.record_operation_failure();
        }
        self.global_metrics.record_latency(latency_ns);
    }

    /// Record venue operation
    pub fn record_venue_operation(&self, venue_id: VenueId, success: bool, latency_ns: u64) {
        if venue_id < self.venue_metrics.len() {
            self.venue_metrics[venue_id].record_operation(success, latency_ns);
        }
    }

    /// Get global metrics
    pub fn global_snapshot(&self) -> PerformanceCounters {
        self.global_metrics.snapshot()
    }

    /// Get venue metrics
    pub fn venue_snapshot(&self, venue_id: VenueId) -> Option<VenueMetricsSnapshot> {
        self.venue_metrics.get(venue_id).map(|vm| vm.snapshot())
    }

    /// Get all venue metrics
    pub fn all_venue_snapshots(&self) -> Vec<VenueMetricsSnapshot> {
        self.venue_metrics.iter().map(|vm| vm.snapshot()).collect()
    }

    /// Get system uptime in nanoseconds
    pub fn uptime_ns(&self) -> u64 {
        Self::current_timestamp_ns().saturating_sub(self.start_time_ns)
    }

    /// Calculate system-wide coordination stats
    pub fn coordination_stats(&self) -> CoordinationStats {
        let global = self.global_snapshot();
        let uptime_seconds = self.uptime_ns() as f64 / 1_000_000_000.0;

        CoordinationStats {
            total_operations: global.total_operations,
            successful_operations: global.successful_operations,
            failed_operations: global.failed_operations,
            avg_latency_ns: global.avg_latency_ns,
            p95_latency_ns: global.max_latency_ns, // Simplified - would need histogram for true P95
            p99_latency_ns: global.max_latency_ns, // Simplified - would need histogram for true P99
            operations_per_second: if uptime_seconds > 0.0 {
                global.total_operations as f64 / uptime_seconds
            } else {
                0.0
            },
        }
    }

    /// Reset all metrics
    pub fn reset_all(&self) {
        self.global_metrics.reset();
        for venue_metric in &self.venue_metrics {
            venue_metric.counters.reset();
        }
    }

    /// Get current timestamp
    #[cfg(feature = "std")]
    fn current_timestamp_ns() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    #[cfg(not(feature = "std"))]
    fn current_timestamp_ns() -> u64 {
        0
    }
}

/// Metrics collection configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable detailed venue metrics
    pub enable_venue_metrics: bool,
    /// Enable latency histogram
    pub enable_latency_histogram: bool,
    /// Metrics collection interval in nanoseconds
    pub collection_interval_ns: u64,
    /// Maximum metrics retention time in nanoseconds
    pub retention_time_ns: u64,
    /// Enable real-time metrics export
    pub enable_real_time_export: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enable_venue_metrics: true,
            enable_latency_histogram: false, // Disabled by default for performance
            collection_interval_ns: 1_000_000_000, // 1 second
            retention_time_ns: 3600_000_000_000, // 1 hour
            enable_real_time_export: false,
        }
    }
}

// Compile-time validation
const _: () = {
    assert!(core::mem::size_of::<CoordinationMetrics>() == 64);
    assert!(core::mem::align_of::<CoordinationMetrics>() == 64);
    assert!(core::mem::size_of::<VenueMetrics>() <= 256);
    assert!(core::mem::align_of::<VenueMetrics>() == 64);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordination_metrics() {
        let metrics = CoordinationMetrics::new();

        metrics.record_operation_success();
        metrics.record_latency(1000);
        metrics.record_operation_failure();
        metrics.record_latency(2000);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total_operations, 2);
        assert_eq!(snapshot.successful_operations, 1);
        assert_eq!(snapshot.failed_operations, 1);
        assert_eq!(snapshot.success_rate, 50.0);
        assert_eq!(snapshot.avg_latency_ns, 1500);
        assert_eq!(snapshot.min_latency_ns, 1000);
        assert_eq!(snapshot.max_latency_ns, 2000);
    }

    #[test]
    fn test_venue_metrics() {
        let venue_metrics = VenueMetrics::new(0);

        venue_metrics.record_operation(true, 500);
        venue_metrics.record_operation(false, 1500);
        venue_metrics.update_health_score(0.85);
        venue_metrics.update_availability(98.5);

        let snapshot = venue_metrics.snapshot();
        assert_eq!(snapshot.venue_id, 0);
        assert_eq!(snapshot.performance.total_operations, 2);
        assert_eq!(snapshot.health_score, 0.85);
        assert_eq!(snapshot.availability, 98.5);
        assert!(snapshot.is_performing_well());
    }

    #[test]
    fn test_system_metrics() {
        let system_metrics = SystemMetrics::new(4);

        system_metrics.record_global_operation(true, 1000);
        system_metrics.record_venue_operation(0, true, 800);
        system_metrics.record_venue_operation(1, false, 1200);

        let global = system_metrics.global_snapshot();
        assert_eq!(global.total_operations, 1);
        assert_eq!(global.success_rate, 100.0);

        let venue0 = system_metrics.venue_snapshot(0).unwrap();
        assert_eq!(venue0.venue_id, 0);
        assert_eq!(venue0.performance.total_operations, 1);

        let stats = system_metrics.coordination_stats();
        assert_eq!(stats.total_operations, 1);
    }

    #[test]
    fn test_performance_counters() {
        let counters = PerformanceCounters {
            total_operations: 1000,
            successful_operations: 950,
            failed_operations: 50,
            success_rate: 95.0,
            avg_latency_ns: 500_000,
            min_latency_ns: 100_000,
            max_latency_ns: 2_000_000,
            last_operation_ns: 0,
        };

        assert_eq!(counters.failure_rate(), 5.0);
        assert!(counters.meets_targets(90.0, 1_000_000));
        assert!(!counters.meets_targets(98.0, 1_000_000));

        let ops_per_sec = counters.operations_per_second(1_000_000_000); // 1 second
        assert_eq!(ops_per_sec, 1000.0);
    }
}