//! MetricsSource trait - Generic interface for any metrics provider
//!
//! Implement this trait to provide metrics data to kindly_dash from your project.
//! Works with clapi_core, kindly_hft, fqbit, or custom implementations.

use crate::types::{Alert, BudgetMetrics, DashboardSnapshot, Forecast, ProviderMetrics};

/// Generic trait for any project to provide metrics to kindly_dash
///
/// # Design Philosophy
///
/// Instead of tight coupling to clapi_core, kindly_dash uses a trait-based
/// architecture. Any project can implement `MetricsSource` to provide data:
///
/// - **clapi_core**: Implements from BudgetRegistry + Phase 4.5 metrics
/// - **kindly_hft**: Implements from brain metrics + performance stats
/// - **fqbit**: Implements from mining metrics + hash rates
/// - **Custom**: Implement for any application
///
/// # Performance
///
/// All methods should be O(1) or O(n) where n is the number of items:
/// - `snapshot()`: <100ns (atomic loads only)
/// - `budget_metrics()`: <1μs (hash map lookup)
/// - `provider_metrics()`: <10μs (iterate 16 providers)
/// - `alert_history()`: <100μs (last 100 alerts)
/// - `forecast()`: <1ms (polynomial regression)
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync`. The dashboard may call
/// these methods from multiple threads concurrently.
///
/// # Consistency
///
/// Related calls within a short time window should return consistent data.
/// Example: `snapshot()` and `budget_metrics()` calls 100ms apart should
/// show coherent state (no wild jumps).
pub trait MetricsSource: Send + Sync {
    /// Complete snapshot of all metrics at current moment
    ///
    /// Called every 100ms for WebSocket updates. Must be <100ns.
    ///
    /// # Returns
    ///
    /// A complete snapshot including global totals, circuit state, and alert counts.
    fn snapshot(&self) -> DashboardSnapshot;

    /// Budget-specific metrics and forecast
    ///
    /// Called when user clicks on a specific budget. May involve computation
    /// (e.g., forecasting), so <1μs target (not <100ns).
    ///
    /// # Returns
    ///
    /// Detailed metrics for the requested budget, or None if not found.
    fn budget_metrics(&self, budget_id: u64) -> Option<BudgetMetrics>;

    /// All provider metrics (typically 1-16)
    ///
    /// Called for provider grid display. Iterate all providers.
    /// Target: <10μs (16 providers × <1μs per provider)
    ///
    /// # Returns
    ///
    /// Vector of provider metrics. Should be sorted by provider_id for consistency.
    fn provider_metrics(&self) -> Vec<ProviderMetrics>;

    /// Recent alerts in chronological order (newest first)
    ///
    /// Called for alert list display. Target: <100μs for last 100 alerts.
    ///
    /// # Returns
    ///
    /// Vector of alerts, sorted by triggered_at_ns descending (newest first).
    /// Should limit to last 100-1000 for performance.
    fn alert_history(&self) -> Vec<Alert>;

    /// Budget forecast for N days
    ///
    /// Called when user requests forecast. May involve:
    /// - Time-series data collection
    /// - Polynomial regression
    /// - Confidence interval calculation
    ///
    /// Target: <1ms (most of budget's request count spent on regression)
    ///
    /// # Returns
    ///
    /// Forecast with projections and recommended actions, or None if insufficient data.
    fn forecast(&self, budget_id: u64, days: u32) -> Option<Forecast>;

    // Optional trait methods (with default implementations)

    /// Maximum budgets this implementation supports
    ///
    /// Used for UI sizing and alerts. Default: 1,000,000 (clapi_core typical).
    fn max_budgets(&self) -> u64 {
        1_000_000
    }

    /// Maximum providers this implementation supports
    ///
    /// Used for provider grid. Default: 16 (clapi_core fixed).
    fn max_providers(&self) -> u64 {
        16
    }

    /// Implementation name (for diagnostics)
    ///
    /// Example: "clapi_core v0.4.5", "kindly_hft v1.0", "custom_metrics"
    fn implementation_name(&self) -> &str {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Minimal mock implementation for testing
    struct MockMetrics;

    impl MetricsSource for MockMetrics {
        fn snapshot(&self) -> DashboardSnapshot {
            DashboardSnapshot::default()
        }

        fn budget_metrics(&self, _id: u64) -> Option<BudgetMetrics> {
            None
        }

        fn provider_metrics(&self) -> Vec<ProviderMetrics> {
            Vec::new()
        }

        fn alert_history(&self) -> Vec<Alert> {
            Vec::new()
        }

        fn forecast(&self, _id: u64, _days: u32) -> Option<Forecast> {
            None
        }

        fn implementation_name(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn test_mock_metrics_trait() {
        let metrics = MockMetrics;
        let snap = metrics.snapshot();
        assert_eq!(snap.total_requests, 0);
        assert_eq!(metrics.implementation_name(), "mock");
    }

    #[test]
    fn test_send_sync_bounds() {
        // Verify trait is Send + Sync
        fn require_send_sync<T: Send + Sync>() {}
        require_send_sync::<Arc<MockMetrics>>();
    }
}
