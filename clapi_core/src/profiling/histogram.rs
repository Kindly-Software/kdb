//! Histogram aggregation and profiler interface
//!
//! High-level profiler interface for managing multiple latency histograms.

use super::capsule::LatencyHistogramCapsule;
use std::sync::Arc;

// Re-export HistogramStats publicly
pub use super::capsule::HistogramStats;

/// Component type for profiling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentType {
    /// HTTP request handling
    HttpRequest,
    /// Budget validation
    BudgetValidation,
    /// Provider routing
    ProviderRouting,
    /// Circuit breaker check
    CircuitBreaker,
    /// Audit logging
    AuditLog,
    /// Database query
    DatabaseQuery,
    /// Cache operation
    CacheOperation,
    /// Compression operation
    Compression,
    /// Custom component
    Custom(&'static str),
}

/// Real-time latency profiler with multiple histograms
///
/// # Architecture
///
/// Manages independent histograms for different system components,
/// enabling granular performance analysis.
///
/// # Example
///
/// ```rust
/// use clapi_core::profiling::{LatencyProfiler, ComponentType};
/// use std::time::Instant;
///
/// let profiler = LatencyProfiler::new();
///
/// // Profile HTTP request
/// let start = Instant::now();
/// handle_request();
/// profiler.record(ComponentType::HttpRequest, start.elapsed().as_nanos() as u64);
///
/// // Profile budget validation
/// let start = Instant::now();
/// validate_budget();
/// profiler.record(ComponentType::BudgetValidation, start.elapsed().as_nanos() as u64);
///
/// // Get statistics
/// let http_stats = profiler.stats(ComponentType::HttpRequest);
/// println!("HTTP p99: {}ns", http_stats.p99);
/// ```
pub struct LatencyProfiler {
    http_request: Arc<LatencyHistogramCapsule>,
    budget_validation: Arc<LatencyHistogramCapsule>,
    provider_routing: Arc<LatencyHistogramCapsule>,
    circuit_breaker: Arc<LatencyHistogramCapsule>,
    audit_log: Arc<LatencyHistogramCapsule>,
    database_query: Arc<LatencyHistogramCapsule>,
    cache_operation: Arc<LatencyHistogramCapsule>,
    compression: Arc<LatencyHistogramCapsule>,
}

impl LatencyProfiler {
    /// Create a new latency profiler
    pub fn new() -> Self {
        Self {
            http_request: Arc::new(LatencyHistogramCapsule::new()),
            budget_validation: Arc::new(LatencyHistogramCapsule::new()),
            provider_routing: Arc::new(LatencyHistogramCapsule::new()),
            circuit_breaker: Arc::new(LatencyHistogramCapsule::new()),
            audit_log: Arc::new(LatencyHistogramCapsule::new()),
            database_query: Arc::new(LatencyHistogramCapsule::new()),
            cache_operation: Arc::new(LatencyHistogramCapsule::new()),
            compression: Arc::new(LatencyHistogramCapsule::new()),
        }
    }

    /// Record a latency sample for a component
    ///
    /// # Performance
    ///
    /// <10ns (delegates to LatencyHistogramCapsule::record)
    pub fn record(&self, component: ComponentType, latency_ns: u64) {
        self.histogram(component).record(latency_ns);
    }

    /// Get statistics for a component
    ///
    /// # Performance
    ///
    /// <100ns (delegates to LatencyHistogramCapsule::stats)
    pub fn stats(&self, component: ComponentType) -> HistogramStats {
        self.histogram(component).stats()
    }

    /// Get percentile for a component
    ///
    /// # Performance
    ///
    /// <50ns (delegates to LatencyHistogramCapsule::percentile)
    pub fn percentile(&self, component: ComponentType, p: f64) -> u64 {
        self.histogram(component).percentile(p)
    }

    /// Reset statistics for a component
    pub fn reset(&self, component: ComponentType) {
        self.histogram(component).reset();
    }

    /// Reset all statistics
    pub fn reset_all(&self) {
        self.http_request.reset();
        self.budget_validation.reset();
        self.provider_routing.reset();
        self.circuit_breaker.reset();
        self.audit_log.reset();
        self.database_query.reset();
        self.cache_operation.reset();
        self.compression.reset();
    }

    /// Get all component statistics
    ///
    /// # Performance
    ///
    /// <1μs (8 histogram snapshots × <100ns each)
    pub fn all_stats(&self) -> Vec<(ComponentType, HistogramStats)> {
        vec![
            (ComponentType::HttpRequest, self.http_request.stats()),
            (ComponentType::BudgetValidation, self.budget_validation.stats()),
            (ComponentType::ProviderRouting, self.provider_routing.stats()),
            (ComponentType::CircuitBreaker, self.circuit_breaker.stats()),
            (ComponentType::AuditLog, self.audit_log.stats()),
            (ComponentType::DatabaseQuery, self.database_query.stats()),
            (ComponentType::CacheOperation, self.cache_operation.stats()),
            (ComponentType::Compression, self.compression.stats()),
        ]
    }

    /// Get histogram for a component
    fn histogram(&self, component: ComponentType) -> &Arc<LatencyHistogramCapsule> {
        match component {
            ComponentType::HttpRequest => &self.http_request,
            ComponentType::BudgetValidation => &self.budget_validation,
            ComponentType::ProviderRouting => &self.provider_routing,
            ComponentType::CircuitBreaker => &self.circuit_breaker,
            ComponentType::AuditLog => &self.audit_log,
            ComponentType::DatabaseQuery => &self.database_query,
            ComponentType::CacheOperation => &self.cache_operation,
            ComponentType::Compression => &self.compression,
            ComponentType::Custom(_) => &self.http_request, // Fallback to HTTP
        }
    }
}

impl Default for LatencyProfiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Format latency in human-readable form
pub fn format_latency(ns: u64) -> String {
    if ns < 1_000 {
        format!("{}ns", ns)
    } else if ns < 1_000_000 {
        format!("{:.2}μs", ns as f64 / 1_000.0)
    } else if ns < 1_000_000_000 {
        format!("{:.2}ms", ns as f64 / 1_000_000.0)
    } else {
        format!("{:.2}s", ns as f64 / 1_000_000_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_creation() {
        let profiler = LatencyProfiler::new();
        let stats = profiler.stats(ComponentType::HttpRequest);
        assert_eq!(stats.count, 0);
    }

    #[test]
    fn test_record_and_query() {
        let profiler = LatencyProfiler::new();

        // Record 100 samples for HTTP requests
        for i in 1..=100 {
            profiler.record(ComponentType::HttpRequest, i * 1000);
        }

        let stats = profiler.stats(ComponentType::HttpRequest);
        assert_eq!(stats.count, 100);
        assert!(stats.p99 > 0);
    }

    #[test]
    fn test_multiple_components() {
        let profiler = LatencyProfiler::new();

        profiler.record(ComponentType::HttpRequest, 100);
        profiler.record(ComponentType::BudgetValidation, 50);
        profiler.record(ComponentType::CircuitBreaker, 10);

        assert_eq!(profiler.stats(ComponentType::HttpRequest).count, 1);
        assert_eq!(profiler.stats(ComponentType::BudgetValidation).count, 1);
        assert_eq!(profiler.stats(ComponentType::CircuitBreaker).count, 1);
    }

    #[test]
    fn test_reset() {
        let profiler = LatencyProfiler::new();

        profiler.record(ComponentType::HttpRequest, 100);
        assert_eq!(profiler.stats(ComponentType::HttpRequest).count, 1);

        profiler.reset(ComponentType::HttpRequest);
        assert_eq!(profiler.stats(ComponentType::HttpRequest).count, 0);
    }

    #[test]
    fn test_reset_all() {
        let profiler = LatencyProfiler::new();

        profiler.record(ComponentType::HttpRequest, 100);
        profiler.record(ComponentType::BudgetValidation, 50);

        profiler.reset_all();

        assert_eq!(profiler.stats(ComponentType::HttpRequest).count, 0);
        assert_eq!(profiler.stats(ComponentType::BudgetValidation).count, 0);
    }

    #[test]
    fn test_all_stats() {
        let profiler = LatencyProfiler::new();

        profiler.record(ComponentType::HttpRequest, 100);
        profiler.record(ComponentType::BudgetValidation, 50);

        let all = profiler.all_stats();
        assert_eq!(all.len(), 8); // 8 tracked components
    }

    #[test]
    fn test_format_latency() {
        assert_eq!(format_latency(100), "100ns");
        assert_eq!(format_latency(1_500), "1.50μs");
        assert_eq!(format_latency(2_500_000), "2.50ms");
        assert_eq!(format_latency(3_500_000_000), "3.50s");
    }
}
