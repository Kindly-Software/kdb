//! Metrics and alerting infrastructure for Clapi Core
//!
//! ## Architecture
//!
//! Built on lockfree computational capsules:
//! - **AlertingEngine**: Lockfree rule evaluation (<1μs per rule)
//! - **AlertPersistence**: Async KindlyDB storage (non-blocking)
//! - **MetricsSnapshot**: Atomic metrics aggregation
//! - **QueryEngine**: Lockfree query execution with fixed-point arithmetic
//! - **Forecasting**: Budget exhaustion prediction (polynomial regression)
//! - **Anomalies**: Statistical outlier detection (Welford's algorithm)
//! - **CostComparison**: Provider efficiency ranking
//!
//! ## Performance
//! - Rule evaluation: <1μs (atomic reads only)
//! - Alert creation: <100ns (no allocation in hot path)
//! - Persistence: Async (non-blocking, eventually consistent)
//! - Callbacks: Concurrent execution (DashMap for subscriptions)
//! - Query execution: O(n) or O(n log n) (bounded complexity)
//! - All cost calculations: Q16.16 fixed-point (deterministic)

pub mod alerting;
pub mod alert_persistence;
pub mod query;
pub mod forecasting;
pub mod anomalies;
pub mod cost_comparison;
pub mod rolling_bucket;

pub use alerting::{
    AlertingEngine,
    Alert,
    AlertRule,
    AlertSeverity,
    AlertContext,
    MetricsSnapshot,
};

pub use alert_persistence::{
    AlertPersistence,
    AlertQuery,
    AlertQueryResult,
};

pub use query::{
    MetricsQuery,
    QueryResult,
    QueryEngine,
    EpochStorage,
    AggOp,
    MetricPoint,
    AggregateResult,
    BudgetForecast,
    ProviderComparison,
    Anomaly,
    AnomalySeverity,
};

pub use anomalies::{
    detect_anomalies,
    detect_correlated_anomalies,
    CorrelatedAnomaly,
    AnomalyCorrelation,
};

pub use cost_comparison::{
    compare_provider_costs,
    compare_with_latency_adjustment,
    LatencyAdjustedComparison,
};

pub use forecasting::forecast_budget_exhaustion;

pub use rolling_bucket::RollingBucket;

// Re-export capsule-based metrics from capsules module
pub use crate::capsules::{
    MetricsSnapshot as CapsuleMetricsSnapshot,
    MetricsSnapshotData,
    ProviderMetrics,
    ProviderMetricsSnapshot,
    BudgetMetrics,
    BudgetSnapshot,
};
