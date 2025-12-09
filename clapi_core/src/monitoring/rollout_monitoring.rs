//! Rollout Monitoring - Track phased deployment metrics
//!
//! This module provides monitoring and alerting for the 4-week phased rollout
//! strategy. It tracks performance metrics, error rates, and rollback triggers
//! for each rollout phase (Week 1-4).
//!
//! # Architecture
//!
//! - **Week 1**: Baseline proxy metrics (budget, circuit breaker, throughput)
//! - **Week 2**: OAuth metrics (session creation, token verification, errors)
//! - **Week 3**: Payment metrics (payment creation, webhook processing, idempotency)
//! - **Week 4**: Compliance metrics (export latency, hash integrity, forensics)
//!
//! # Framework Compliance
//!
//! - **I20 Q19**: Integration strategy monitoring
//! - **I20 Q20**: Rollback trigger detection
//! - **B32**: Performance budget enforcement
//! - **T28**: Production monitoring (Q22-Q28)

use std::sync::atomic::{AtomicU64, Ordering};

/// Rollout week identifier (1-4)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolloutWeek {
    /// Week 1: Proxy-only baseline
    Week1ProxyOnly = 1,
    /// Week 2: OAuth 2.0 integration
    Week2OAuth = 2,
    /// Week 3: Stripe payment integration
    Week3Payments = 3,
    /// Week 4: Full compliance + KindlyDB
    Week4Full = 4,
}

impl RolloutWeek {
    /// Parse from integer (1-4)
    pub fn from_u8(week: u8) -> Option<Self> {
        match week {
            1 => Some(Self::Week1ProxyOnly),
            2 => Some(Self::Week2OAuth),
            3 => Some(Self::Week3Payments),
            4 => Some(Self::Week4Full),
            _ => None,
        }
    }

    /// Get feature list for this week
    pub fn features(&self) -> &'static [&'static str] {
        match self {
            Self::Week1ProxyOnly => &["proxy-only"],
            Self::Week2OAuth => &["oauth"],
            Self::Week3Payments => &["payments"],
            Self::Week4Full => &["full"],
        }
    }

    /// Get traffic percentage for this week (initial)
    pub fn initial_traffic_percentage(&self) -> u8 {
        match self {
            Self::Week1ProxyOnly => 100,
            Self::Week2OAuth => 1,    // 1% canary
            Self::Week3Payments => 10, // 10% canary
            Self::Week4Full => 100,   // Big bang (capsule-based)
        }
    }
}

/// Week 1: Proxy-only baseline metrics
#[repr(C, align(256))]
pub struct Week1ProxyMetrics {
    // Budget operations
    pub budget_check_count: AtomicU64,
    pub budget_check_total_ns: AtomicU64,
    pub budget_check_max_ns: AtomicU64,

    // Slot allocation
    pub slot_allocation_count: AtomicU64,
    pub slot_allocation_total_ns: AtomicU64,
    pub slot_allocation_max_ns: AtomicU64,

    // Circuit breaker
    pub circuit_breaker_check_count: AtomicU64,
    pub circuit_breaker_total_ns: AtomicU64,
    pub circuit_breaker_trip_count: AtomicU64,

    // Error tracking
    pub panic_count: AtomicU64,
    pub crash_count: AtomicU64,

    _padding: [u8; 128], // Ensure 256-byte alignment
}

impl Week1ProxyMetrics {
    pub const fn new() -> Self {
        Self {
            budget_check_count: AtomicU64::new(0),
            budget_check_total_ns: AtomicU64::new(0),
            budget_check_max_ns: AtomicU64::new(0),
            slot_allocation_count: AtomicU64::new(0),
            slot_allocation_total_ns: AtomicU64::new(0),
            slot_allocation_max_ns: AtomicU64::new(0),
            circuit_breaker_check_count: AtomicU64::new(0),
            circuit_breaker_total_ns: AtomicU64::new(0),
            circuit_breaker_trip_count: AtomicU64::new(0),
            panic_count: AtomicU64::new(0),
            crash_count: AtomicU64::new(0),
            _padding: [0; 128],
        }
    }

    /// Record budget check operation
    pub fn record_budget_check(&self, duration_ns: u64) {
        self.budget_check_count.fetch_add(1, Ordering::Relaxed);
        self.budget_check_total_ns.fetch_add(duration_ns, Ordering::Relaxed);

        // Update max (CAS loop)
        let mut current_max = self.budget_check_max_ns.load(Ordering::Relaxed);
        while duration_ns > current_max {
            match self.budget_check_max_ns.compare_exchange_weak(
                current_max,
                duration_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(new_max) => current_max = new_max,
            }
        }
    }

    /// Record slot allocation operation
    pub fn record_slot_allocation(&self, duration_ns: u64) {
        self.slot_allocation_count.fetch_add(1, Ordering::Relaxed);
        self.slot_allocation_total_ns.fetch_add(duration_ns, Ordering::Relaxed);

        let mut current_max = self.slot_allocation_max_ns.load(Ordering::Relaxed);
        while duration_ns > current_max {
            match self.slot_allocation_max_ns.compare_exchange_weak(
                current_max,
                duration_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(new_max) => current_max = new_max,
            }
        }
    }

    /// Record circuit breaker check
    pub fn record_circuit_breaker_check(&self, duration_ns: u64) {
        self.circuit_breaker_check_count.fetch_add(1, Ordering::Relaxed);
        self.circuit_breaker_total_ns.fetch_add(duration_ns, Ordering::Relaxed);
    }

    /// Record circuit breaker trip
    pub fn record_circuit_breaker_trip(&self) {
        self.circuit_breaker_trip_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get average budget check latency (ns)
    pub fn avg_budget_check_ns(&self) -> u64 {
        let count = self.budget_check_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0;
        }
        self.budget_check_total_ns.load(Ordering::Relaxed) / count
    }

    /// Get average slot allocation latency (ns)
    pub fn avg_slot_allocation_ns(&self) -> u64 {
        let count = self.slot_allocation_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0;
        }
        self.slot_allocation_total_ns.load(Ordering::Relaxed) / count
    }

    /// Check if baseline meets success criteria
    pub fn meets_success_criteria(&self) -> bool {
        // Budget check: avg <60ns, max <120ns
        let avg_budget = self.avg_budget_check_ns();
        let max_budget = self.budget_check_max_ns.load(Ordering::Relaxed);

        // Slot allocation: avg <80ns, max <140ns
        let avg_slot = self.avg_slot_allocation_ns();
        let max_slot = self.slot_allocation_max_ns.load(Ordering::Relaxed);

        // Zero panics/crashes
        let zero_errors = self.panic_count.load(Ordering::Relaxed) == 0
            && self.crash_count.load(Ordering::Relaxed) == 0;

        avg_budget < 60 && max_budget < 120 && avg_slot < 80 && max_slot < 140 && zero_errors
    }
}

impl Default for Week1ProxyMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Week 2: OAuth metrics
#[repr(C, align(256))]
pub struct Week2OAuthMetrics {
    // Session operations
    pub session_create_count: AtomicU64,
    pub session_create_total_ns: AtomicU64,
    pub session_create_errors: AtomicU64,

    // Token verification
    pub token_verify_count: AtomicU64,
    pub token_verify_total_ns: AtomicU64,
    pub token_verify_errors: AtomicU64,

    // KindlyDB persistence
    pub kindlydb_write_count: AtomicU64,
    pub kindlydb_write_total_ns: AtomicU64,
    pub kindlydb_errors: AtomicU64,

    // OAuth provider health
    pub oauth_provider_unavailable_count: AtomicU64,

    _padding: [u8; 144], // Ensure 256-byte alignment
}

impl Week2OAuthMetrics {
    pub const fn new() -> Self {
        Self {
            session_create_count: AtomicU64::new(0),
            session_create_total_ns: AtomicU64::new(0),
            session_create_errors: AtomicU64::new(0),
            token_verify_count: AtomicU64::new(0),
            token_verify_total_ns: AtomicU64::new(0),
            token_verify_errors: AtomicU64::new(0),
            kindlydb_write_count: AtomicU64::new(0),
            kindlydb_write_total_ns: AtomicU64::new(0),
            kindlydb_errors: AtomicU64::new(0),
            oauth_provider_unavailable_count: AtomicU64::new(0),
            _padding: [0; 144],
        }
    }

    /// Record session creation
    pub fn record_session_create(&self, duration_ns: u64, success: bool) {
        self.session_create_count.fetch_add(1, Ordering::Relaxed);
        self.session_create_total_ns.fetch_add(duration_ns, Ordering::Relaxed);
        if !success {
            self.session_create_errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record token verification
    pub fn record_token_verify(&self, duration_ns: u64, success: bool) {
        self.token_verify_count.fetch_add(1, Ordering::Relaxed);
        self.token_verify_total_ns.fetch_add(duration_ns, Ordering::Relaxed);
        if !success {
            self.token_verify_errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record KindlyDB write
    pub fn record_kindlydb_write(&self, duration_ns: u64, success: bool) {
        self.kindlydb_write_count.fetch_add(1, Ordering::Relaxed);
        self.kindlydb_write_total_ns.fetch_add(duration_ns, Ordering::Relaxed);
        if !success {
            self.kindlydb_errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record OAuth provider unavailable
    pub fn record_oauth_provider_unavailable(&self) {
        self.oauth_provider_unavailable_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get session creation error rate (basis points)
    pub fn session_create_error_rate_bp(&self) -> u64 {
        let count = self.session_create_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0;
        }
        let errors = self.session_create_errors.load(Ordering::Relaxed);
        (errors * 10_000) / count
    }

    /// Check if rollback should trigger (>1% error rate)
    pub fn should_rollback(&self) -> bool {
        self.session_create_error_rate_bp() > 100 // >1%
            || self.kindlydb_errors.load(Ordering::Relaxed) > 0
    }

    /// Get average session creation latency (ns)
    pub fn avg_session_create_ns(&self) -> u64 {
        let count = self.session_create_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0;
        }
        self.session_create_total_ns.load(Ordering::Relaxed) / count
    }
}

impl Default for Week2OAuthMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Week 3: Payment metrics
#[repr(C, align(256))]
pub struct Week3PaymentMetrics {
    // Payment creation
    pub payment_create_count: AtomicU64,
    pub payment_create_total_ns: AtomicU64,
    pub payment_create_errors: AtomicU64,

    // Webhook processing
    pub webhook_process_count: AtomicU64,
    pub webhook_process_total_ms: AtomicU64,
    pub webhook_process_errors: AtomicU64,

    // Idempotency
    pub idempotency_check_count: AtomicU64,
    pub idempotency_check_total_ns: AtomicU64,
    pub idempotency_failures: AtomicU64,

    // Stripe API
    pub stripe_api_calls: AtomicU64,
    pub stripe_rate_limit_warnings: AtomicU64,

    _padding: [u8; 144], // Ensure 256-byte alignment
}

impl Week3PaymentMetrics {
    pub const fn new() -> Self {
        Self {
            payment_create_count: AtomicU64::new(0),
            payment_create_total_ns: AtomicU64::new(0),
            payment_create_errors: AtomicU64::new(0),
            webhook_process_count: AtomicU64::new(0),
            webhook_process_total_ms: AtomicU64::new(0),
            webhook_process_errors: AtomicU64::new(0),
            idempotency_check_count: AtomicU64::new(0),
            idempotency_check_total_ns: AtomicU64::new(0),
            idempotency_failures: AtomicU64::new(0),
            stripe_api_calls: AtomicU64::new(0),
            stripe_rate_limit_warnings: AtomicU64::new(0),
            _padding: [0; 144],
        }
    }

    /// Record payment creation
    pub fn record_payment_create(&self, duration_ns: u64, success: bool) {
        self.payment_create_count.fetch_add(1, Ordering::Relaxed);
        self.payment_create_total_ns.fetch_add(duration_ns, Ordering::Relaxed);
        if !success {
            self.payment_create_errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record webhook processing
    pub fn record_webhook_process(&self, duration_ms: u64, success: bool) {
        self.webhook_process_count.fetch_add(1, Ordering::Relaxed);
        self.webhook_process_total_ms.fetch_add(duration_ms, Ordering::Relaxed);
        if !success {
            self.webhook_process_errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record idempotency check
    pub fn record_idempotency_check(&self, duration_ns: u64, success: bool) {
        self.idempotency_check_count.fetch_add(1, Ordering::Relaxed);
        self.idempotency_check_total_ns.fetch_add(duration_ns, Ordering::Relaxed);
        if !success {
            self.idempotency_failures.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record Stripe API call
    pub fn record_stripe_api_call(&self) {
        self.stripe_api_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record Stripe rate limit warning
    pub fn record_stripe_rate_limit_warning(&self) {
        self.stripe_rate_limit_warnings.fetch_add(1, Ordering::Relaxed);
    }

    /// Get payment creation error rate (basis points)
    pub fn payment_create_error_rate_bp(&self) -> u64 {
        let count = self.payment_create_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0;
        }
        let errors = self.payment_create_errors.load(Ordering::Relaxed);
        (errors * 10_000) / count
    }

    /// Get webhook processing error rate (basis points)
    pub fn webhook_error_rate_bp(&self) -> u64 {
        let count = self.webhook_process_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0;
        }
        let errors = self.webhook_process_errors.load(Ordering::Relaxed);
        (errors * 10_000) / count
    }

    /// Get idempotency failure rate (basis points)
    pub fn idempotency_failure_rate_bp(&self) -> u64 {
        let count = self.idempotency_check_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0;
        }
        let failures = self.idempotency_failures.load(Ordering::Relaxed);
        (failures * 10_000) / count
    }

    /// Check if rollback should trigger
    pub fn should_rollback(&self) -> bool {
        self.payment_create_error_rate_bp() > 100 // >1% payment errors
            || self.webhook_error_rate_bp() > 100 // >1% webhook errors
            || self.idempotency_failure_rate_bp() > 10 // >0.1% idempotency failures
    }
}

impl Default for Week3PaymentMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Week 4: Compliance metrics
#[repr(C, align(256))]
pub struct Week4ComplianceMetrics {
    // Export operations
    pub export_json_count: AtomicU64,
    pub export_json_total_ms: AtomicU64,
    pub export_csv_count: AtomicU64,
    pub export_csv_total_ms: AtomicU64,
    pub export_errors: AtomicU64,

    // Hash chain integrity
    pub hash_chain_verifications: AtomicU64,
    pub hash_chain_integrity_failures: AtomicU64,

    // Forensics
    pub timeline_reconstruction_count: AtomicU64,
    pub timeline_reconstruction_total_ms: AtomicU64,
    pub anomaly_detection_count: AtomicU64,
    pub anomaly_detection_total_ms: AtomicU64,

    _padding: [u8; 128], // Ensure 256-byte alignment
}

impl Week4ComplianceMetrics {
    pub const fn new() -> Self {
        Self {
            export_json_count: AtomicU64::new(0),
            export_json_total_ms: AtomicU64::new(0),
            export_csv_count: AtomicU64::new(0),
            export_csv_total_ms: AtomicU64::new(0),
            export_errors: AtomicU64::new(0),
            hash_chain_verifications: AtomicU64::new(0),
            hash_chain_integrity_failures: AtomicU64::new(0),
            timeline_reconstruction_count: AtomicU64::new(0),
            timeline_reconstruction_total_ms: AtomicU64::new(0),
            anomaly_detection_count: AtomicU64::new(0),
            anomaly_detection_total_ms: AtomicU64::new(0),
            _padding: [0; 128],
        }
    }

    /// Record JSON export
    pub fn record_export_json(&self, duration_ms: u64, success: bool) {
        self.export_json_count.fetch_add(1, Ordering::Relaxed);
        self.export_json_total_ms.fetch_add(duration_ms, Ordering::Relaxed);
        if !success {
            self.export_errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record CSV export
    pub fn record_export_csv(&self, duration_ms: u64, success: bool) {
        self.export_csv_count.fetch_add(1, Ordering::Relaxed);
        self.export_csv_total_ms.fetch_add(duration_ms, Ordering::Relaxed);
        if !success {
            self.export_errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record hash chain verification
    pub fn record_hash_chain_verification(&self, success: bool) {
        self.hash_chain_verifications.fetch_add(1, Ordering::Relaxed);
        if !success {
            self.hash_chain_integrity_failures.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record timeline reconstruction
    pub fn record_timeline_reconstruction(&self, duration_ms: u64) {
        self.timeline_reconstruction_count.fetch_add(1, Ordering::Relaxed);
        self.timeline_reconstruction_total_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
    }

    /// Record anomaly detection
    pub fn record_anomaly_detection(&self, duration_ms: u64) {
        self.anomaly_detection_count.fetch_add(1, Ordering::Relaxed);
        self.anomaly_detection_total_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
    }

    /// Get export error rate (basis points)
    pub fn export_error_rate_bp(&self) -> u64 {
        let total = self.export_json_count.load(Ordering::Relaxed)
            + self.export_csv_count.load(Ordering::Relaxed);
        if total == 0 {
            return 0;
        }
        let errors = self.export_errors.load(Ordering::Relaxed);
        (errors * 10_000) / total
    }

    /// Check if rollback should trigger (CRITICAL: Any hash integrity failure)
    pub fn should_rollback(&self) -> bool {
        self.hash_chain_integrity_failures.load(Ordering::Relaxed) > 0
            || self.export_error_rate_bp() > 100 // >1% export errors
    }

    /// Get hash chain integrity percentage
    pub fn hash_chain_integrity_percentage(&self) -> f64 {
        let verifications = self.hash_chain_verifications.load(Ordering::Relaxed);
        if verifications == 0 {
            return 100.0; // No verifications yet = 100% (optimistic)
        }
        let failures = self.hash_chain_integrity_failures.load(Ordering::Relaxed);
        let successes = verifications.saturating_sub(failures);
        (successes as f64 / verifications as f64) * 100.0
    }
}

impl Default for Week4ComplianceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Global rollout monitoring state
pub struct RolloutMonitoring {
    pub current_week: RolloutWeek,
    pub week1: Week1ProxyMetrics,
    pub week2: Week2OAuthMetrics,
    pub week3: Week3PaymentMetrics,
    pub week4: Week4ComplianceMetrics,
}

impl RolloutMonitoring {
    pub const fn new(current_week: RolloutWeek) -> Self {
        Self {
            current_week,
            week1: Week1ProxyMetrics::new(),
            week2: Week2OAuthMetrics::new(),
            week3: Week3PaymentMetrics::new(),
            week4: Week4ComplianceMetrics::new(),
        }
    }

    /// Check if current week should trigger rollback
    pub fn should_rollback(&self) -> bool {
        match self.current_week {
            RolloutWeek::Week1ProxyOnly => false, // Baseline, no rollback
            RolloutWeek::Week2OAuth => self.week2.should_rollback(),
            RolloutWeek::Week3Payments => self.week3.should_rollback(),
            RolloutWeek::Week4Full => self.week4.should_rollback(),
        }
    }

    /// Get rollback reason (if should_rollback() is true)
    pub fn rollback_reason(&self) -> Option<String> {
        if !self.should_rollback() {
            return None;
        }

        match self.current_week {
            RolloutWeek::Week1ProxyOnly => None,
            RolloutWeek::Week2OAuth => {
                let error_rate = self.week2.session_create_error_rate_bp();
                if error_rate > 100 {
                    Some(format!("OAuth session creation error rate {}bp > 100bp (1%)", error_rate))
                } else if self.week2.kindlydb_errors.load(Ordering::Relaxed) > 0 {
                    Some("KindlyDB connection errors detected".to_string())
                } else {
                    Some("OAuth provider unavailable".to_string())
                }
            }
            RolloutWeek::Week3Payments => {
                let payment_errors = self.week3.payment_create_error_rate_bp();
                let webhook_errors = self.week3.webhook_error_rate_bp();
                let idempotency_failures = self.week3.idempotency_failure_rate_bp();

                if payment_errors > 100 {
                    Some(format!("Payment creation error rate {}bp > 100bp (1%)", payment_errors))
                } else if webhook_errors > 100 {
                    Some(format!("Webhook processing error rate {}bp > 100bp (1%)", webhook_errors))
                } else if idempotency_failures > 10 {
                    Some(format!("Idempotency failure rate {}bp > 10bp (0.1%)", idempotency_failures))
                } else {
                    Some("Stripe API errors detected".to_string())
                }
            }
            RolloutWeek::Week4Full => {
                if self.week4.hash_chain_integrity_failures.load(Ordering::Relaxed) > 0 {
                    Some("CRITICAL: Hash chain integrity failures detected".to_string())
                } else {
                    Some(format!("Compliance export error rate {}bp > 100bp (1%)", self.week4.export_error_rate_bp()))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rollout_week_from_u8() {
        assert_eq!(RolloutWeek::from_u8(1), Some(RolloutWeek::Week1ProxyOnly));
        assert_eq!(RolloutWeek::from_u8(2), Some(RolloutWeek::Week2OAuth));
        assert_eq!(RolloutWeek::from_u8(3), Some(RolloutWeek::Week3Payments));
        assert_eq!(RolloutWeek::from_u8(4), Some(RolloutWeek::Week4Full));
        assert_eq!(RolloutWeek::from_u8(5), None);
    }

    #[test]
    fn test_week1_success_criteria() {
        let metrics = Week1ProxyMetrics::new();

        // Record baseline operations
        metrics.record_budget_check(50); // <60ns ✓
        metrics.record_slot_allocation(70); // <80ns ✓

        assert!(metrics.meets_success_criteria());
    }

    #[test]
    fn test_week1_failure_criteria() {
        let metrics = Week1ProxyMetrics::new();

        // Record operations exceeding budget
        metrics.record_budget_check(200); // >120ns ✗

        assert!(!metrics.meets_success_criteria());
    }

    #[test]
    fn test_week2_rollback_trigger() {
        let metrics = Week2OAuthMetrics::new();

        // Record 100 sessions, 2 errors = 2% error rate > 1% threshold
        for _ in 0..98 {
            metrics.record_session_create(30, true);
        }
        for _ in 0..2 {
            metrics.record_session_create(30, false);
        }

        assert!(metrics.should_rollback());
    }

    #[test]
    fn test_week3_rollback_trigger_idempotency() {
        let metrics = Week3PaymentMetrics::new();

        // Record 1000 idempotency checks, 2 failures = 0.2% > 0.1% threshold
        for _ in 0..998 {
            metrics.record_idempotency_check(60, true);
        }
        for _ in 0..2 {
            metrics.record_idempotency_check(60, false);
        }

        assert!(metrics.should_rollback());
    }

    #[test]
    fn test_week4_hash_integrity_failure() {
        let metrics = Week4ComplianceMetrics::new();

        // Any hash integrity failure triggers rollback
        metrics.record_hash_chain_verification(false);

        assert!(metrics.should_rollback());
    }

    #[test]
    fn test_week4_hash_integrity_percentage() {
        let metrics = Week4ComplianceMetrics::new();

        // 100 verifications, 0 failures = 100%
        for _ in 0..100 {
            metrics.record_hash_chain_verification(true);
        }

        assert_eq!(metrics.hash_chain_integrity_percentage(), 100.0);

        // 1 failure = 99%
        metrics.record_hash_chain_verification(false);
        assert_eq!(
            metrics.hash_chain_integrity_percentage(),
            99.00990099009901 // 100/101
        );
    }
}
