//! # AnomalyRouter - Shadow Mode Deployment Orchestration (T6 Mixed)
//!
//! **Purpose**: Orchestrate V1 and V2 anomaly detectors for shadow mode deployment,
//! enabling A/B comparison, gradual rollout, and performance validation.
//!
//! ## UCE34 Framework Analysis (Q1-Q34)
//!
//! ### Q1-Q9: Meta-Cognitive Analysis
//! - **Q1 (Scope)**: Safe V2 rollout with V1 fallback and A/B comparison
//! - **Q2 (Assumptions)**: V1 is production-proven, V2 is experimental
//! - **Q3 (Constraints)**: <100ns overhead for routing, atomic mode switching
//! - **Q4 (Context)**: Protection system migration, zero-downtime upgrade
//! - **Q5 (Success)**: 99%+ V1/V2 agreement before full V2 rollout
//! - **Q6 (Failure)**: V2 regression detected, automatic fallback to V1
//! - **Q7 (Patterns)**: Shadow mode, A/B testing, feature flags
//! - **Q8 (Alternatives)**: Big-bang migration (risky), parallel APIs (complex)
//! - **Q9 (Trade-offs)**: 2x CPU in shadow mode, but safe validation
//!
//! ### Q10-Q12: Foundation (Capsule Architecture)
//! - **Q10 (Tier Selection)**: T6 Mixed (T1 orchestration + T10 detectors)
//! - **Q11 (Rust Transform)**: AnomalyRouter (wrapper), ShadowMetrics (atomic counters)
//! - **Q12 (Nightly)**: No nightly features required
//!
//! ## Deployment Modes
//!
//! ```text
//! V1Only Mode:     [Request] → [V1 Detector] → [Decision]
//!                           ↑ Production-proven, default
//!
//! Shadow Mode:     [Request] → [V1 Detector] → [Decision]
//!                           ↓
//!                          [V2 Detector] → [Log discrepancy]
//!                           ↑ V2 runs, V1 decides, log differences
//!
//! Hybrid Mode:     [Request] → [V1: weight%] ─┐
//!                           ↓                 ├→ [Weighted Decision]
//!                          [V2: (100-weight)%]┘
//!                           ↑ Gradual rollout, configurable weight
//!
//! V2Only Mode:     [Request] → [V2 Detector] → [Decision]
//!                           ↑ Full migration after validation
//! ```
//!
//! ## Performance Targets (B32 Framework)
//!
//! | Mode | Latency | Description |
//! |------|---------|-------------|
//! | V1Only | <100ns | Single detector path |
//! | Shadow | <200ns | Both detectors, V1 decides |
//! | Hybrid | <150ns | Weighted combination |
//! | V2Only | <100ns | Single detector path |
//!
//! ## ASSUM Safety
//! - #ASSUME_ATOMIC_MODE_SWITCH: Mode changes are atomic (AtomicU8)
//! - #ASSUME_COUNTER_OVERFLOW_SAFE: u64 counters won't overflow in practice
//! - #ASSUME_V1_STABLE: V1 detector is production-proven baseline
//! - #ASSUME_LOCKFREE_METRICS: All metrics use Relaxed ordering (informational)

use core::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use super::anomaly_detector::{AnomalyDetectorCapsule, AnomalyResult as V1Result};
#[cfg(all(feature = "anomaly-v2", feature = "anomaly-detection"))]
use super::anomaly_detector_v2::{AnomalyDetectorV2, AnomalyResultV2 as V2Result};

// ============================================================================
// ROUTER MODE
// ============================================================================

/// Deployment mode for anomaly detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RouterMode {
    /// V1 only - production-proven detector
    V1Only = 0,

    /// Shadow mode - V2 runs in parallel, V1 decides, log discrepancies
    Shadow = 1,

    /// Hybrid mode - weighted combination of V1 and V2
    Hybrid = 2,

    /// V2 only - fully migrated to new detector
    V2Only = 3,
}

impl RouterMode {
    /// Convert from u8
    #[inline]
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => RouterMode::V1Only,
            1 => RouterMode::Shadow,
            2 => RouterMode::Hybrid,
            3 => RouterMode::V2Only,
            _ => RouterMode::V1Only, // Safe default
        }
    }
}

// ============================================================================
// DISCREPANCY TYPE
// ============================================================================

/// Type of discrepancy between V1 and V2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscrepancyType {
    /// Both agree: normal
    AgreeNormal,

    /// Both agree: anomalous (V1 Suspicious/Anomalous, V2 Suspicious/Anomalous/Critical)
    AgreeAnomalous,

    /// V1 says normal, V2 says anomalous (potential false negative in V1)
    V1NormalV2Anomalous,

    /// V1 says anomalous, V2 says normal (potential false positive in V1)
    V1AnomalousV2Normal,
}

// ============================================================================
// SHADOW METRICS (256 bytes)
// ============================================================================

/// Metrics tracking V1/V2 comparison in shadow mode
///
/// **Memory Layout**: 256 bytes, 64-byte aligned
/// - Counters use AtomicU64 with Relaxed ordering (informational)
/// - Latency tracking uses moving average (Q8.8 fixed-point)
///
/// **Thread Safety**: 100% lockfree via atomic operations
#[repr(C, align(64))]
pub struct ShadowMetrics {
    // ========== Agreement Counters (64 bytes) ==========

    /// Both V1 and V2 classify as normal
    pub agree_normal: AtomicU64,

    /// Both V1 and V2 classify as anomalous
    pub agree_anomalous: AtomicU64,

    /// V1 normal, V2 anomalous (V1 potential false negative)
    pub v1_normal_v2_anomalous: AtomicU64,

    /// V1 anomalous, V2 normal (V1 potential false positive)
    pub v1_anomalous_v2_normal: AtomicU64,

    /// Padding to 64 bytes
    _padding1: [u8; 32],

    // ========== Latency Tracking (64 bytes) ==========

    /// V1 total latency (nanoseconds, cumulative)
    pub v1_latency_total_ns: AtomicU64,

    /// V2 total latency (nanoseconds, cumulative)
    pub v2_latency_total_ns: AtomicU64,

    /// Total checks performed
    pub total_checks: AtomicU64,

    /// Total discrepancies (v1_normal_v2_anomalous + v1_anomalous_v2_normal)
    pub total_discrepancies: AtomicU64,

    /// Padding to 64 bytes
    _padding2: [u8; 32],

    // ========== Severity Tracking (64 bytes) ==========

    /// V2 critical anomalies detected
    pub v2_critical_count: AtomicU64,

    /// V2 anomalous count (not critical)
    pub v2_anomalous_count: AtomicU64,

    /// V2 suspicious count
    pub v2_suspicious_count: AtomicU64,

    /// Generation counter (for snapshots)
    pub generation: AtomicU64,

    /// Padding to 64 bytes
    _padding3: [u8; 32],

    // ========== Configuration (64 bytes) ==========

    /// Hybrid mode V1 weight (0-100, percentage)
    pub hybrid_v1_weight: AtomicU8,

    /// Current mode
    mode: AtomicU8,

    /// Padding to 64 bytes
    _padding4: [u8; 62],
}

impl ShadowMetrics {
    /// Create new metrics tracker
    #[inline]
    pub const fn new() -> Self {
        Self {
            agree_normal: AtomicU64::new(0),
            agree_anomalous: AtomicU64::new(0),
            v1_normal_v2_anomalous: AtomicU64::new(0),
            v1_anomalous_v2_normal: AtomicU64::new(0),
            _padding1: [0; 32],

            v1_latency_total_ns: AtomicU64::new(0),
            v2_latency_total_ns: AtomicU64::new(0),
            total_checks: AtomicU64::new(0),
            total_discrepancies: AtomicU64::new(0),
            _padding2: [0; 32],

            v2_critical_count: AtomicU64::new(0),
            v2_anomalous_count: AtomicU64::new(0),
            v2_suspicious_count: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            _padding3: [0; 32],

            hybrid_v1_weight: AtomicU8::new(100), // 100% V1 by default
            mode: AtomicU8::new(0), // V1Only
            _padding4: [0; 62],
        }
    }

    /// Record a comparison result
    #[inline]
    pub fn record_comparison(
        &self,
        discrepancy: DiscrepancyType,
        v1_latency_ns: u64,
        v2_latency_ns: u64,
    ) {
        // Update agreement counters
        match discrepancy {
            DiscrepancyType::AgreeNormal => {
                self.agree_normal.fetch_add(1, Ordering::Relaxed);
            }
            DiscrepancyType::AgreeAnomalous => {
                self.agree_anomalous.fetch_add(1, Ordering::Relaxed);
            }
            DiscrepancyType::V1NormalV2Anomalous => {
                self.v1_normal_v2_anomalous.fetch_add(1, Ordering::Relaxed);
                self.total_discrepancies.fetch_add(1, Ordering::Relaxed);
            }
            DiscrepancyType::V1AnomalousV2Normal => {
                self.v1_anomalous_v2_normal.fetch_add(1, Ordering::Relaxed);
                self.total_discrepancies.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Update latency
        self.v1_latency_total_ns.fetch_add(v1_latency_ns, Ordering::Relaxed);
        self.v2_latency_total_ns.fetch_add(v2_latency_ns, Ordering::Relaxed);
        self.total_checks.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Record V2 severity
    #[cfg(all(feature = "anomaly-v2", feature = "anomaly-detection"))]
    #[inline]
    pub fn record_v2_severity(&self, result: V2Result) {
        match result {
            V2Result::Normal => {}
            V2Result::Suspicious => {
                self.v2_suspicious_count.fetch_add(1, Ordering::Relaxed);
            }
            V2Result::Anomalous => {
                self.v2_anomalous_count.fetch_add(1, Ordering::Relaxed);
            }
            V2Result::Critical => {
                self.v2_critical_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Get agreement rate (0.0 - 1.0)
    #[inline]
    pub fn agreement_rate(&self) -> f64 {
        let total = self.total_checks.load(Ordering::Relaxed);
        if total == 0 {
            return 1.0; // No data = perfect agreement
        }

        let agree = self.agree_normal.load(Ordering::Relaxed)
            + self.agree_anomalous.load(Ordering::Relaxed);
        agree as f64 / total as f64
    }

    /// Get discrepancy rate (0.0 - 1.0)
    #[inline]
    pub fn discrepancy_rate(&self) -> f64 {
        let total = self.total_checks.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }

        let discrepancies = self.total_discrepancies.load(Ordering::Relaxed);
        discrepancies as f64 / total as f64
    }

    /// Get average V1 latency (nanoseconds)
    #[inline]
    pub fn avg_v1_latency_ns(&self) -> u64 {
        let total = self.total_checks.load(Ordering::Relaxed);
        if total == 0 {
            return 0;
        }
        self.v1_latency_total_ns.load(Ordering::Relaxed) / total
    }

    /// Get average V2 latency (nanoseconds)
    #[inline]
    pub fn avg_v2_latency_ns(&self) -> u64 {
        let total = self.total_checks.load(Ordering::Relaxed);
        if total == 0 {
            return 0;
        }
        self.v2_latency_total_ns.load(Ordering::Relaxed) / total
    }

    /// Export metrics as JSON string
    #[cfg(feature = "std")]
    pub fn to_json(&self) -> String {
        let total = self.total_checks.load(Ordering::Relaxed);
        let agree_normal = self.agree_normal.load(Ordering::Relaxed);
        let agree_anomalous = self.agree_anomalous.load(Ordering::Relaxed);
        let v1_normal_v2_anomalous = self.v1_normal_v2_anomalous.load(Ordering::Relaxed);
        let v1_anomalous_v2_normal = self.v1_anomalous_v2_normal.load(Ordering::Relaxed);
        let v2_critical = self.v2_critical_count.load(Ordering::Relaxed);
        let v2_anomalous = self.v2_anomalous_count.load(Ordering::Relaxed);
        let v2_suspicious = self.v2_suspicious_count.load(Ordering::Relaxed);
        let generation = self.generation.load(Ordering::Relaxed);
        let mode = RouterMode::from_u8(self.mode.load(Ordering::Relaxed));
        let weight = self.hybrid_v1_weight.load(Ordering::Relaxed);

        format!(
            r#"{{"total_checks":{},"agreement":{{"normal":{},"anomalous":{}}},"discrepancies":{{"v1_normal_v2_anomalous":{},"v1_anomalous_v2_normal":{}}},"v2_severity":{{"critical":{},"anomalous":{},"suspicious":{}}},"rates":{{"agreement":{:.4},"discrepancy":{:.4}}},"latency_ns":{{"v1_avg":{},"v2_avg":{}}},"config":{{"mode":"{}","hybrid_v1_weight":{}}},"generation":{}}}"#,
            total,
            agree_normal, agree_anomalous,
            v1_normal_v2_anomalous, v1_anomalous_v2_normal,
            v2_critical, v2_anomalous, v2_suspicious,
            self.agreement_rate(), self.discrepancy_rate(),
            self.avg_v1_latency_ns(), self.avg_v2_latency_ns(),
            match mode {
                RouterMode::V1Only => "v1_only",
                RouterMode::Shadow => "shadow",
                RouterMode::Hybrid => "hybrid",
                RouterMode::V2Only => "v2_only",
            },
            weight,
            generation
        )
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.agree_normal.store(0, Ordering::Relaxed);
        self.agree_anomalous.store(0, Ordering::Relaxed);
        self.v1_normal_v2_anomalous.store(0, Ordering::Relaxed);
        self.v1_anomalous_v2_normal.store(0, Ordering::Relaxed);
        self.v1_latency_total_ns.store(0, Ordering::Relaxed);
        self.v2_latency_total_ns.store(0, Ordering::Relaxed);
        self.total_checks.store(0, Ordering::Relaxed);
        self.total_discrepancies.store(0, Ordering::Relaxed);
        self.v2_critical_count.store(0, Ordering::Relaxed);
        self.v2_anomalous_count.store(0, Ordering::Relaxed);
        self.v2_suspicious_count.store(0, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Create a snapshot of current metrics
    pub fn snapshot(&self) -> ShadowMetricsSnapshot {
        ShadowMetricsSnapshot {
            total_checks: self.total_checks.load(Ordering::Relaxed),
            agree_normal: self.agree_normal.load(Ordering::Relaxed),
            agree_anomalous: self.agree_anomalous.load(Ordering::Relaxed),
            v1_normal_v2_anomalous: self.v1_normal_v2_anomalous.load(Ordering::Relaxed),
            v1_anomalous_v2_normal: self.v1_anomalous_v2_normal.load(Ordering::Relaxed),
            v2_critical_count: self.v2_critical_count.load(Ordering::Relaxed),
            v2_anomalous_count: self.v2_anomalous_count.load(Ordering::Relaxed),
            v2_suspicious_count: self.v2_suspicious_count.load(Ordering::Relaxed),
            avg_v1_latency_ns: self.avg_v1_latency_ns(),
            avg_v2_latency_ns: self.avg_v2_latency_ns(),
            agreement_rate: self.agreement_rate(),
            discrepancy_rate: self.discrepancy_rate(),
            generation: self.generation.load(Ordering::Relaxed),
        }
    }
}

impl Default for ShadowMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of shadow metrics (non-atomic, for analysis)
#[derive(Debug, Clone, Copy)]
pub struct ShadowMetricsSnapshot {
    pub total_checks: u64,
    pub agree_normal: u64,
    pub agree_anomalous: u64,
    pub v1_normal_v2_anomalous: u64,
    pub v1_anomalous_v2_normal: u64,
    pub v2_critical_count: u64,
    pub v2_anomalous_count: u64,
    pub v2_suspicious_count: u64,
    pub avg_v1_latency_ns: u64,
    pub avg_v2_latency_ns: u64,
    pub agreement_rate: f64,
    pub discrepancy_rate: f64,
    pub generation: u64,
}

// ============================================================================
// ANOMALY ROUTER (512 bytes)
// ============================================================================

/// AnomalyRouter - Orchestrates V1 and V2 anomaly detectors
///
/// **Memory Layout**: 512 bytes, 512-byte aligned
/// - V1 detector: 2560 bytes (external reference)
/// - V2 detector: 8192 bytes (external reference, feature-gated)
/// - Metrics: 256 bytes (embedded)
///
/// **Thread Safety**: 100% lockfree
#[repr(C, align(512))]
pub struct AnomalyRouter {
    /// Shadow metrics for V1/V2 comparison
    metrics: ShadowMetrics,

    /// Padding to 512 bytes
    _padding: [u8; 256],
}

impl AnomalyRouter {
    /// Create new router with V1Only mode (default)
    #[inline]
    pub const fn new() -> Self {
        Self {
            metrics: ShadowMetrics::new(),
            _padding: [0; 256],
        }
    }

    /// Get current mode
    #[inline]
    pub fn mode(&self) -> RouterMode {
        RouterMode::from_u8(self.metrics.mode.load(Ordering::Acquire))
    }

    /// Set routing mode
    #[inline]
    pub fn set_mode(&self, mode: RouterMode) {
        self.metrics.mode.store(mode as u8, Ordering::Release);
        self.metrics.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Set hybrid mode V1 weight (0-100)
    #[inline]
    pub fn set_hybrid_weight(&self, weight: u8) {
        let clamped = weight.min(100);
        self.metrics.hybrid_v1_weight.store(clamped, Ordering::Release);
    }

    /// Get hybrid mode V1 weight
    #[inline]
    pub fn hybrid_weight(&self) -> u8 {
        self.metrics.hybrid_v1_weight.load(Ordering::Acquire)
    }

    /// Get reference to metrics
    #[inline]
    pub fn metrics(&self) -> &ShadowMetrics {
        &self.metrics
    }

    /// Check behavior with V1 only (fast path)
    #[inline]
    pub fn check_v1_only(&self, v1: &AnomalyDetectorCapsule, behavior: u64) -> V1Result {
        let start = std::time::Instant::now();
        let result = v1.check_behavior(behavior);
        let elapsed_ns = start.elapsed().as_nanos() as u64;

        // Record V1-only latency
        self.metrics.v1_latency_total_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
        self.metrics.total_checks.fetch_add(1, Ordering::Relaxed);
        self.metrics.generation.fetch_add(1, Ordering::SeqCst);

        result
    }

    /// Check behavior with shadow mode (V2 runs, V1 decides, log discrepancies)
    #[cfg(all(feature = "anomaly-v2", feature = "anomaly-detection"))]
    pub fn check_shadow(
        &self,
        v1: &AnomalyDetectorCapsule,
        v2: &AnomalyDetectorV2,
        behavior: u64,
        timestamp_ms: u32,
    ) -> V1Result {
        // Run V1
        let v1_start = std::time::Instant::now();
        let v1_result = v1.check_behavior(behavior);
        let v1_latency = v1_start.elapsed().as_nanos() as u64;

        // Run V2 (shadow - doesn't affect decision)
        let v2_start = std::time::Instant::now();
        let v2_report = v2.check_behavior_v2(behavior, timestamp_ms);
        let v2_latency = v2_start.elapsed().as_nanos() as u64;

        // Classify discrepancy
        let discrepancy = classify_discrepancy(v1_result, v2_report.result);

        // Record metrics
        self.metrics.record_comparison(discrepancy, v1_latency, v2_latency);
        self.metrics.record_v2_severity(v2_report.result);

        // V1 decides in shadow mode
        v1_result
    }

    /// Check behavior with hybrid mode (weighted combination)
    #[cfg(all(feature = "anomaly-v2", feature = "anomaly-detection"))]
    pub fn check_hybrid(
        &self,
        v1: &AnomalyDetectorCapsule,
        v2: &AnomalyDetectorV2,
        behavior: u64,
        timestamp_ms: u32,
    ) -> V1Result {
        let weight = self.hybrid_weight() as u64;

        // Run V1
        let v1_start = std::time::Instant::now();
        let v1_result = v1.check_behavior(behavior);
        let v1_latency = v1_start.elapsed().as_nanos() as u64;

        // Run V2
        let v2_start = std::time::Instant::now();
        let v2_report = v2.check_behavior_v2(behavior, timestamp_ms);
        let v2_latency = v2_start.elapsed().as_nanos() as u64;

        // Record metrics
        let discrepancy = classify_discrepancy(v1_result, v2_report.result);
        self.metrics.record_comparison(discrepancy, v1_latency, v2_latency);
        self.metrics.record_v2_severity(v2_report.result);

        // Weighted decision
        // Use behavior hash as pseudo-random selector for fair distribution
        let selector = (behavior ^ self.metrics.generation.load(Ordering::Relaxed)) % 100;

        if selector < weight {
            // Use V1 decision
            v1_result
        } else {
            // Use V2 decision (converted to V1 result)
            v2_report.result.to_v1()
        }
    }

    /// Check behavior with V2 only
    #[cfg(all(feature = "anomaly-v2", feature = "anomaly-detection"))]
    #[inline]
    pub fn check_v2_only(&self, v2: &AnomalyDetectorV2, behavior: u64, timestamp_ms: u32) -> V1Result {
        let start = std::time::Instant::now();
        let report = v2.check_behavior_v2(behavior, timestamp_ms);
        let elapsed_ns = start.elapsed().as_nanos() as u64;

        // Record V2-only latency
        self.metrics.v2_latency_total_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
        self.metrics.total_checks.fetch_add(1, Ordering::Relaxed);
        self.metrics.record_v2_severity(report.result);
        self.metrics.generation.fetch_add(1, Ordering::SeqCst);

        report.result.to_v1()
    }

    /// Main routing function - automatically selects based on mode
    #[cfg(all(feature = "anomaly-v2", feature = "anomaly-detection"))]
    pub fn check_behavior(
        &self,
        v1: &AnomalyDetectorCapsule,
        v2: &AnomalyDetectorV2,
        behavior: u64,
        timestamp_ms: u32,
    ) -> V1Result {
        match self.mode() {
            RouterMode::V1Only => self.check_v1_only(v1, behavior),
            RouterMode::Shadow => self.check_shadow(v1, v2, behavior, timestamp_ms),
            RouterMode::Hybrid => self.check_hybrid(v1, v2, behavior, timestamp_ms),
            RouterMode::V2Only => self.check_v2_only(v2, behavior, timestamp_ms),
        }
    }

    /// Check if V2 is ready for full deployment (>99% agreement)
    #[inline]
    pub fn is_v2_ready(&self) -> bool {
        let rate = self.metrics.agreement_rate();
        let total = self.metrics.total_checks.load(Ordering::Relaxed);

        // Need at least 10,000 samples and >99% agreement
        rate >= 0.99 && total >= 10_000
    }

    /// Get deployment recommendation
    pub fn deployment_recommendation(&self) -> DeploymentRecommendation {
        let agreement = self.metrics.agreement_rate();
        let discrepancy = self.metrics.discrepancy_rate();
        let total = self.metrics.total_checks.load(Ordering::Relaxed);
        let v1_latency = self.metrics.avg_v1_latency_ns();
        let v2_latency = self.metrics.avg_v2_latency_ns();

        if total < 1000 {
            return DeploymentRecommendation::NeedMoreData { current: total, required: 1000 };
        }

        if agreement >= 0.99 {
            // >99% agreement - safe to migrate
            if v2_latency <= v1_latency * 2 {
                DeploymentRecommendation::ReadyForV2Only
            } else {
                DeploymentRecommendation::V2SlowerButReady {
                    latency_ratio: v2_latency as f64 / v1_latency as f64
                }
            }
        } else if agreement >= 0.95 {
            // 95-99% agreement - consider hybrid
            DeploymentRecommendation::HybridRecommended { agreement_rate: agreement }
        } else if discrepancy > 0.10 {
            // >10% discrepancy - investigate
            DeploymentRecommendation::InvestigateDiscrepancies {
                discrepancy_rate: discrepancy
            }
        } else {
            DeploymentRecommendation::StayShadow { agreement_rate: agreement }
        }
    }
}

impl Default for AnomalyRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Deployment recommendation based on shadow metrics
#[derive(Debug, Clone)]
pub enum DeploymentRecommendation {
    /// Need more data before making recommendation
    NeedMoreData { current: u64, required: u64 },

    /// Stay in shadow mode, gather more data
    StayShadow { agreement_rate: f64 },

    /// Investigate high discrepancy rate
    InvestigateDiscrepancies { discrepancy_rate: f64 },

    /// Recommend hybrid mode deployment
    HybridRecommended { agreement_rate: f64 },

    /// Ready for full V2 deployment
    ReadyForV2Only,

    /// V2 ready but slower than V1
    V2SlowerButReady { latency_ratio: f64 },
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Classify discrepancy between V1 and V2 results
#[cfg(all(feature = "anomaly-v2", feature = "anomaly-detection"))]
#[inline]
fn classify_discrepancy(v1: V1Result, v2: V2Result) -> DiscrepancyType {
    let v1_is_normal = matches!(v1, V1Result::Normal);
    let v2_is_normal = matches!(v2, V2Result::Normal);

    match (v1_is_normal, v2_is_normal) {
        (true, true) => DiscrepancyType::AgreeNormal,
        (false, false) => DiscrepancyType::AgreeAnomalous,
        (true, false) => DiscrepancyType::V1NormalV2Anomalous,
        (false, true) => DiscrepancyType::V1AnomalousV2Normal,
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== UNIT TESTS (10) ====================

    #[test]
    fn test_router_mode_conversion() {
        assert_eq!(RouterMode::from_u8(0), RouterMode::V1Only);
        assert_eq!(RouterMode::from_u8(1), RouterMode::Shadow);
        assert_eq!(RouterMode::from_u8(2), RouterMode::Hybrid);
        assert_eq!(RouterMode::from_u8(3), RouterMode::V2Only);
        assert_eq!(RouterMode::from_u8(255), RouterMode::V1Only); // Safe default
    }

    #[test]
    fn test_shadow_metrics_creation() {
        let metrics = ShadowMetrics::new();
        assert_eq!(metrics.total_checks.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.agreement_rate(), 1.0); // No data = perfect agreement
    }

    #[test]
    fn test_shadow_metrics_record() {
        let metrics = ShadowMetrics::new();

        metrics.record_comparison(DiscrepancyType::AgreeNormal, 50, 100);
        assert_eq!(metrics.agree_normal.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.total_checks.load(Ordering::Relaxed), 1);

        metrics.record_comparison(DiscrepancyType::V1NormalV2Anomalous, 60, 120);
        assert_eq!(metrics.v1_normal_v2_anomalous.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.total_discrepancies.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_agreement_rate() {
        let metrics = ShadowMetrics::new();

        // 8 agreements, 2 discrepancies = 80% agreement
        for _ in 0..8 {
            metrics.record_comparison(DiscrepancyType::AgreeNormal, 50, 100);
        }
        for _ in 0..2 {
            metrics.record_comparison(DiscrepancyType::V1NormalV2Anomalous, 50, 100);
        }

        let rate = metrics.agreement_rate();
        assert!((rate - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_discrepancy_rate() {
        let metrics = ShadowMetrics::new();

        for _ in 0..90 {
            metrics.record_comparison(DiscrepancyType::AgreeNormal, 50, 100);
        }
        for _ in 0..10 {
            metrics.record_comparison(DiscrepancyType::V1AnomalousV2Normal, 50, 100);
        }

        let rate = metrics.discrepancy_rate();
        assert!((rate - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_latency_tracking() {
        let metrics = ShadowMetrics::new();

        metrics.record_comparison(DiscrepancyType::AgreeNormal, 100, 200);
        metrics.record_comparison(DiscrepancyType::AgreeNormal, 100, 200);

        assert_eq!(metrics.avg_v1_latency_ns(), 100);
        assert_eq!(metrics.avg_v2_latency_ns(), 200);
    }

    #[test]
    fn test_router_creation() {
        let router = AnomalyRouter::new();
        assert_eq!(router.mode(), RouterMode::V1Only);
        assert_eq!(router.hybrid_weight(), 100);
    }

    #[test]
    fn test_router_mode_switch() {
        let router = AnomalyRouter::new();

        router.set_mode(RouterMode::Shadow);
        assert_eq!(router.mode(), RouterMode::Shadow);

        router.set_mode(RouterMode::Hybrid);
        assert_eq!(router.mode(), RouterMode::Hybrid);

        router.set_mode(RouterMode::V2Only);
        assert_eq!(router.mode(), RouterMode::V2Only);
    }

    #[test]
    fn test_hybrid_weight() {
        let router = AnomalyRouter::new();

        router.set_hybrid_weight(50);
        assert_eq!(router.hybrid_weight(), 50);

        // Clamp to 100
        router.set_hybrid_weight(150);
        assert_eq!(router.hybrid_weight(), 100);
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = ShadowMetrics::new();

        for _ in 0..100 {
            metrics.record_comparison(DiscrepancyType::AgreeNormal, 50, 100);
        }

        assert_eq!(metrics.total_checks.load(Ordering::Relaxed), 100);

        metrics.reset();
        assert_eq!(metrics.total_checks.load(Ordering::Relaxed), 0);
    }

    // ==================== PROPERTY TESTS (5) ====================

    #[test]
    fn proptest_agreement_rate_bounded() {
        let metrics = ShadowMetrics::new();

        for i in 0..100 {
            let discrepancy = if i % 10 == 0 {
                DiscrepancyType::V1NormalV2Anomalous
            } else {
                DiscrepancyType::AgreeNormal
            };
            metrics.record_comparison(discrepancy, 50, 100);
        }

        let rate = metrics.agreement_rate();
        assert!(rate >= 0.0 && rate <= 1.0);
    }

    #[test]
    fn proptest_discrepancy_rate_bounded() {
        let metrics = ShadowMetrics::new();

        for i in 0..100 {
            let discrepancy = if i % 5 == 0 {
                DiscrepancyType::V1AnomalousV2Normal
            } else {
                DiscrepancyType::AgreeAnomalous
            };
            metrics.record_comparison(discrepancy, 50, 100);
        }

        let rate = metrics.discrepancy_rate();
        assert!(rate >= 0.0 && rate <= 1.0);
    }

    #[test]
    fn proptest_generation_monotonic() {
        let metrics = ShadowMetrics::new();
        let mut prev = metrics.generation.load(Ordering::Relaxed);

        for _ in 0..100 {
            metrics.record_comparison(DiscrepancyType::AgreeNormal, 50, 100);
            let current = metrics.generation.load(Ordering::Relaxed);
            assert!(current > prev);
            prev = current;
        }
    }

    #[test]
    fn proptest_total_checks_monotonic() {
        let metrics = ShadowMetrics::new();
        let mut prev = metrics.total_checks.load(Ordering::Relaxed);

        for _ in 0..100 {
            metrics.record_comparison(DiscrepancyType::AgreeNormal, 50, 100);
            let current = metrics.total_checks.load(Ordering::Relaxed);
            assert!(current > prev);
            prev = current;
        }
    }

    #[test]
    fn proptest_counters_consistent() {
        let metrics = ShadowMetrics::new();

        for i in 0..100 {
            let discrepancy = match i % 4 {
                0 => DiscrepancyType::AgreeNormal,
                1 => DiscrepancyType::AgreeAnomalous,
                2 => DiscrepancyType::V1NormalV2Anomalous,
                _ => DiscrepancyType::V1AnomalousV2Normal,
            };
            metrics.record_comparison(discrepancy, 50, 100);
        }

        let total = metrics.total_checks.load(Ordering::Relaxed);
        let sum = metrics.agree_normal.load(Ordering::Relaxed)
            + metrics.agree_anomalous.load(Ordering::Relaxed)
            + metrics.v1_normal_v2_anomalous.load(Ordering::Relaxed)
            + metrics.v1_anomalous_v2_normal.load(Ordering::Relaxed);

        assert_eq!(total, sum);
    }

    // ==================== INTEGRATION TESTS (5) ====================

    #[test]
    fn integration_shadow_metrics_snapshot() {
        let metrics = ShadowMetrics::new();

        for _ in 0..50 {
            metrics.record_comparison(DiscrepancyType::AgreeNormal, 100, 150);
        }

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total_checks, 50);
        assert_eq!(snapshot.agree_normal, 50);
        assert_eq!(snapshot.avg_v1_latency_ns, 100);
        assert_eq!(snapshot.avg_v2_latency_ns, 150);
    }

    #[cfg(feature = "std")]
    #[test]
    fn integration_json_export() {
        let metrics = ShadowMetrics::new();

        for _ in 0..10 {
            metrics.record_comparison(DiscrepancyType::AgreeNormal, 50, 100);
        }

        let json = metrics.to_json();
        assert!(json.contains("\"total_checks\":10"));
        assert!(json.contains("\"agreement\":"));
        assert!(json.contains("\"mode\":\"v1_only\""));
    }

    #[test]
    fn integration_deployment_recommendation_need_data() {
        let router = AnomalyRouter::new();

        for _ in 0..100 {
            router.metrics.record_comparison(DiscrepancyType::AgreeNormal, 50, 100);
        }

        match router.deployment_recommendation() {
            DeploymentRecommendation::NeedMoreData { current, required } => {
                assert_eq!(current, 100);
                assert_eq!(required, 1000);
            }
            _ => panic!("Expected NeedMoreData"),
        }
    }

    #[test]
    fn integration_deployment_recommendation_ready() {
        let router = AnomalyRouter::new();

        // Simulate 10,000 samples with >99% agreement
        for i in 0..10000 {
            let discrepancy = if i % 200 == 0 {
                DiscrepancyType::V1NormalV2Anomalous
            } else {
                DiscrepancyType::AgreeNormal
            };
            router.metrics.record_comparison(discrepancy, 50, 80);
        }

        match router.deployment_recommendation() {
            DeploymentRecommendation::ReadyForV2Only => {}
            other => panic!("Expected ReadyForV2Only, got {:?}", other),
        }
    }

    #[test]
    fn integration_concurrent_metrics() {
        use std::sync::Arc;
        use std::thread;

        let metrics = Arc::new(ShadowMetrics::new());
        let mut handles = vec![];

        for t in 0..4 {
            let m = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                for i in 0..250 {
                    let discrepancy = if (t + i) % 10 == 0 {
                        DiscrepancyType::V1NormalV2Anomalous
                    } else {
                        DiscrepancyType::AgreeNormal
                    };
                    m.record_comparison(discrepancy, 50, 100);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(metrics.total_checks.load(Ordering::Relaxed), 1000);
    }
}
