//! Computational capsules for AI call protection (v0.3.0 - Phase 2 Complete)
//!
//! Twelve specialized capsules following UCE33 framework:
//! - RequestCapsule128 (T1 Atomic): Budget validation, 3-5× speedup
//! - RoutingCapsule128 (T1 Atomic): Provider selection, 3-8× speedup
//! - ResponseCapsule256 (T2+T3 SIMD+Fixed-Point): Metrics tracking, 4-12× speedup
//! - AuditLogEntry128 (T5 Streaming): Audit trail, 10-100× speedup
//! - EpochTile1024 (T4+T3 Batch+Fixed-Point): Cost aggregation, 10-20× speedup
//! - BudgetMetaCapsule (T4+T1 Batch+Atomic): 1M budget management, <100ns operations
//! - BudgetSlotCapsule (T1 Atomic): Lockfree slot allocation, <50ns operations
//! - CircuitBreakerCapsule (T1 Atomic): Graceful degradation, <5ns checks
//! - CircuitBreakerMetrics (T1 Atomic): Metrics tracking, <20ns operations
//! - ProviderCircuitStatus (T1 Atomic): Per-provider circuit state, <20ns operations
//! - ProviderCircuitArray (T4 Batch): 16-provider independent tracking, O(16) bounded
//!
//! # New in v0.3.0 (Phase 2)
//! - CircuitBreakerMetrics: Atomic metrics tracking for circuit breakers
//! - ProviderCircuitStatus: Per-provider circuit breaker state (64B)
//! - ProviderCircuitArray: Array of 16 independent circuit breakers (1KB)
//!
//! # New in v0.2.0
//! - BudgetSlotCapsule: Lockfree slot management via AtomicPtr
//! - CircuitBreakerCapsule: Failure protection (opens at >10% failure rate)
//! - BudgetMetaCapsule: Pure atomic architecture (no Vec, no Arc, no locks)

pub mod req_128;
pub mod request_capsule128_enhanced;
pub mod rte_128;
pub mod res_256;
pub mod ale_128;
pub mod et_1kb;
pub mod budget_metacapsule;
pub mod budget_slot_capsule;
pub mod circuit_breaker_capsule;
pub mod circuit_breaker_metrics;
pub mod provider_circuit_status;
pub mod provider_circuit_array;
pub mod capsule_hash64;
pub mod metrics_snapshot;
pub mod provider_metrics;
pub mod budget_metrics;
pub mod rate_limit;
pub mod advanced_rate_limiter;
pub mod oauth_session;
pub mod request;
pub mod compression_state;
pub mod payment;
pub mod payment128;
pub mod metrics_stream;
pub mod cost_forecast;
pub mod coalescence;
pub mod pattern_learner;
pub mod audit_event_capsule;
pub mod timeline_aggregation_capsule;
pub mod multi_tenant_timeline;
pub mod timeline_metrics;
pub mod checkpoint;
pub mod stress_test_harness;
pub mod flush_audit;
pub mod async_flush_audit;
pub mod outlier_audit;
pub mod checkpoint_hash;
pub mod simd_aggregation;
pub mod sharded_multi_tenant;
pub mod config_reload;
pub mod capacity_planner;
pub mod response_cache;
pub mod deduplication;
pub mod anomaly_detector;
pub mod metrics_registry;

// Phase 2 Loop Armor: Advanced Protection Capsules
pub mod burst_detector;
pub mod cost_velocity;
pub mod pattern_signature;
pub mod tracing_capsule64;
pub mod async_flush_capsule;
pub mod batch_append_capsule;

// Phase 3 Loop Armor: Per-Client Circuit Breaker
pub mod client_circuit_breaker;

// P0 Error Handling & Logging (Phase 6)
pub mod error_context_capsule;
pub mod structured_log_capsule;
pub mod worker_recovery;

// P0 Critical Enhancements 5,6,9,10: Operations & Reliability
pub mod alerting_capsule;
pub mod recovery_capsule;

pub use req_128::RequestCapsule128;
pub use request_capsule128_enhanced::{
    RequestCapsule128Enhanced,
    EnhancedMetrics,
    AuditEntry as RequestAuditEntry,
    ChainValidationResult,
};
pub use capsule_hash64::CapsuleHash64;
pub use rte_128::{RoutingCapsule128, ProviderState};
pub use res_256::ResponseCapsule256;
pub use ale_128::{AuditLogEntry128, AuditEntry, EventType};
pub use et_1kb::{EpochTile1024, EpochSnapshot, ProviderSnapshot};
pub use budget_metacapsule::{BudgetMetaCapsule, BudgetMetaCapsuleHeader, MetaCapsuleStats, MAX_BUDGET_SLOTS};
pub use budget_slot_capsule::{BudgetSlotCapsule, SlotStatus};
pub use circuit_breaker_capsule::{CircuitBreakerCapsule, CircuitBreakerState, CircuitState};
pub use circuit_breaker_metrics::{CircuitBreakerMetrics, CircuitBreakerMetricsSnapshot};
pub use provider_circuit_array::{ProviderCircuitArray, ProviderCircuitStatus, CircuitState as ProviderCircuitState, thresholds};
pub use metrics_snapshot::{MetricsSnapshot, MetricsSnapshotData};
pub use provider_metrics::{ProviderMetrics, ProviderSnapshot as ProviderMetricsSnapshot, CircuitState as ProviderCircuitStateMetrics};
pub use budget_metrics::{BudgetMetrics, BudgetSnapshot};
pub use rate_limit::{RateLimitCapsule, RateLimitStats};
pub use advanced_rate_limiter::{AdvancedRateLimiter64, RateLimiterStats};
pub use oauth_session::{OAuthSessionCapsule, SessionState, SessionSnapshot};
pub use request::{RequestCapsule, RequestCoordinator, RequestSnapshot};
pub use compression_state::{CompressionStateCapsule, CompressionStats, compute_histogram, compute_histogram_scalar};
pub use payment::{PaymentCapsule256, PaymentStatus, PaymentSnapshot};
pub use payment128::{PaymentCapsule128, PaymentSnapshot128};
pub use metrics_stream::{MetricsStreamCapsule, MetricsSnapshot as MetricsStreamSnapshot};
pub use cost_forecast::{CostForecast256, ForecastSnapshot};
pub use coalescence::{CoalescenceEntry128, CoalescenceState, CoalescenceSnapshot};
pub use pattern_learner::{PatternLearner256, PatternStats, PATTERN_WINDOW_SIZE, MAX_CORRELATION_PAIRS, PREFETCH_CONFIDENCE_THRESHOLD_BP};
pub use audit_event_capsule::{AuditEventCapsule, AuditEventType};
pub use timeline_aggregation_capsule::{
    TimelineAggregationCapsule, TimelineAggregationCapsuleCore, TimelineBucket, BucketGranularity,
    BucketStatus, BucketSnapshot, TimelineBuilder, Trend,
};
pub use multi_tenant_timeline::MultiTenantTimelineCapsule;
pub use timeline_metrics::TimelineMetrics;
pub use checkpoint::Checkpoint;
pub use stress_test_harness::{
    StressTestHarness, StressTestSummary, LatencyHistogram, HistogramSummary,
    get_current_rss_bytes, DeterministicRng,
};
pub use flush_audit::{FlushAuditEntry, FlushAuditTrail};
pub use async_flush_audit::{AsyncFlushAuditEntry, AsyncFlushAuditTrail, FlushTaskState};
pub use outlier_audit::{OutlierAuditEntry, OutlierAuditTrail, OutlierRootCause};
pub use checkpoint_hash::{CheckpointHashCapsule, CheckpointHashChain, VerificationStatus};
pub use config_reload::ConfigReloadCapsule64;
pub use capacity_planner::{CapacityPlannerCapsule128, TimeTillExhaustion};
pub use response_cache::{ResponseCache, CacheKeyCapsule, CacheEntry, CacheStats};
pub use deduplication::{DeduplicationCapsule, InFlightRequestCapsule, DeduplicationStats};

// P2 E15: SIMD Aggregation Helpers (T2 Tier)
#[cfg(feature = "portable_simd")]
pub use simd_aggregation::{
    simd_sum_u64x4, simd_sum_u64x8, simd_min_u64x4, simd_max_u64x4,
    simd_avg_u64x4, simd_percentile_u64x4, simd_moving_avg_u64x8, adaptive_sum,
};

#[cfg(feature = "simd")]
pub use compression_state::compute_histogram_simd;
pub mod health_check;
pub use health_check::{HealthCheckCapsule64, Component, HealthStatus};

// P3-E2: Anomaly Detection + Metrics Registry
pub use anomaly_detector::{AnomalyDetectorCapsule128, AnomalySeverity, Anomaly};
pub use metrics_registry::{MetricsRegistry, MetricId, MetricType};
pub use tracing_capsule64::TracingCapsule64;

// P2 Enhancements: Async Flush + Batch Append (Phase 6)
pub use async_flush_capsule::{AsyncFlushPipeline, FlushTask, FlushResult, FlushMetrics, FlushMetricsSnapshot};
pub use batch_append_capsule::{BatchAppendRequest, BatchAppendStats, BatchAppendConfig, BatchAppendProcessor};

// P0 Error Handling & Logging exports (Phase 6)
pub use error_context_capsule::{
    ErrorContextCapsule, ErrorCode, ErrorSeverity, ErrorState, ErrorMetrics,
};
pub use structured_log_capsule::{
    StructuredLogCapsule, LogEntry, LogLevel,
};
pub use worker_recovery::{
    WorkerRecovery, WorkerHealth, RecoveryConfig, WorkerThread,
};

// P0 Critical Enhancements 5,6,9,10: Operations & Reliability
pub use alerting_capsule::{AlertingCapsule, Alert, AlertType, AlertSeverity};
pub use recovery_capsule::{RecoveryManager, RecoveryStrategy, RecoveryAttempt, RecoveryStats, ErrorCode as RecoveryErrorCode};

// Phase 2 Loop Armor: Advanced Protection Capsules
pub use burst_detector::BurstDetectorCapsule64;
pub use cost_velocity::CostVelocityCapsule128;
pub use pattern_signature::PatternSignatureCapsule256;

// Phase 3 Loop Armor: Per-Client Circuit Breaker
pub use client_circuit_breaker::{ClientCircuitBreakerCapsule128, CircuitBreakerDecision, CircuitBreakerState as ClientCircuitBreakerState};
