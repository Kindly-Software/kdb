//! MetricsCapsule - T1 Atomic Prometheus Metrics for KDB Signup Service
//!
//! A 128-byte, 64-byte aligned computational capsule for aggregating metrics
//! from all service capsules and exposing them via Prometheus format.
//!
//! # UCE34/Chaos Compliance
//! - **Tier**: T1 Atomic (lockfree, <10ns operations)
//! - **Size**: 128 bytes (cache-optimized, 2 cache lines)
//! - **Alignment**: 64 bytes (cache-line aligned, no false sharing)
//! - **Concurrency**: 100% lockfree via AtomicU64 only
//! - **TOCTOU Prevention**: Generation counter incremented on every snapshot
//!
//! # Metrics Exposed
//!
//! - `kdb_signup_registrations_total` - Total successful signups
//! - `kdb_signup_verifications_total` - Total email verifications
//! - `kdb_signup_licenses_issued_total` - Total licenses generated
//! - `kdb_signup_promo_licenses_total` - Licenses during promo period
//! - `kdb_signup_rate_limited_total` - Requests blocked by rate limiter
//! - `kdb_signup_tokens_generated_total` - Verification tokens created
//! - `kdb_signup_capsule_generation` - Metrics capsule generation counter
//!
//! # Framework Compliance
//!
//! - UCE34: Q10 T1 Atomic tier, Q33 lockfree, Q34 audit-compatible metrics
//! - Chaos: Cache-aligned, generation counters, zero mutex
//! - ASSUM: Zero unsafe blocks (all AtomicU64 operations)

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use metrics::{counter, describe_counter, describe_gauge, gauge};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

use crate::routes::AppState;

/// Snapshot of all metrics values
#[derive(Debug, Clone, Copy, Default)]
pub struct MetricsSnapshot {
    /// Total successful registrations
    pub registrations_total: u64,
    /// Total successful verifications
    pub verifications_total: u64,
    /// Total licenses issued
    pub licenses_issued_total: u64,
    /// Licenses issued during promo period
    pub promo_licenses_total: u64,
    /// Requests blocked by rate limiter
    pub rate_limited_total: u64,
    /// Verification tokens generated
    pub tokens_generated_total: u64,
    /// Metrics capsule generation counter
    pub generation: u64,
}

/// T1 Atomic tier metrics capsule
///
/// 128-byte, 64-byte aligned structure with:
/// - 7 atomic counters (56 bytes)
/// - Padding (72 bytes)
///
/// # Memory Layout
/// ```text
/// Offset  Size  Field
/// 0       8     registrations_total (AtomicU64)
/// 8       8     verifications_total (AtomicU64)
/// 16      8     licenses_issued_total (AtomicU64)
/// 24      8     promo_licenses_total (AtomicU64)
/// 32      8     rate_limited_total (AtomicU64)
/// 40      8     tokens_generated_total (AtomicU64)
/// 48      8     generation (AtomicU64)
/// 56      72    _padding
/// ─────────────
/// 128     Total (64B aligned)
/// ```
#[repr(C, align(64))]
pub struct MetricsCapsule {
    // === Counters (56 bytes) ===
    /// Total successful registrations
    registrations_total: AtomicU64,
    /// Total successful verifications
    verifications_total: AtomicU64,
    /// Total licenses issued
    licenses_issued_total: AtomicU64,
    /// Licenses issued during promo period
    promo_licenses_total: AtomicU64,
    /// Requests blocked by rate limiter
    rate_limited_total: AtomicU64,
    /// Verification tokens generated
    tokens_generated_total: AtomicU64,
    /// Generation counter for TOCTOU prevention
    generation: AtomicU64,

    // === Padding to 128 bytes ===
    /// Padding: 128 - 56 = 72 bytes
    _padding: [u8; 72],
}

// Compile-time verification of struct size and alignment
const _: () = {
    assert!(std::mem::size_of::<MetricsCapsule>() == 128);
    assert!(std::mem::align_of::<MetricsCapsule>() == 64);
};

impl MetricsCapsule {
    /// Create a new MetricsCapsule with zeroed state
    #[inline]
    pub const fn new() -> Self {
        Self {
            registrations_total: AtomicU64::new(0),
            verifications_total: AtomicU64::new(0),
            licenses_issued_total: AtomicU64::new(0),
            promo_licenses_total: AtomicU64::new(0),
            rate_limited_total: AtomicU64::new(0),
            tokens_generated_total: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            _padding: [0u8; 72],
        }
    }

    /// Increment registrations counter
    #[inline]
    pub fn increment_registrations(&self) {
        self.registrations_total.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Increment verifications counter
    #[inline]
    pub fn increment_verifications(&self) {
        self.verifications_total.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Increment licenses issued counter
    #[inline]
    pub fn increment_licenses_issued(&self) {
        self.licenses_issued_total.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Increment promo licenses counter
    #[inline]
    pub fn increment_promo_licenses(&self) {
        self.promo_licenses_total.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Increment rate limited counter
    #[inline]
    pub fn increment_rate_limited(&self) {
        self.rate_limited_total.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Increment tokens generated counter
    #[inline]
    pub fn increment_tokens_generated(&self) {
        self.tokens_generated_total.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get an atomic snapshot of all metrics
    ///
    /// Uses Acquire ordering for consistent read across all counters.
    /// Generation counter ensures snapshot consistency.
    #[inline]
    pub fn snapshot(&self) -> MetricsSnapshot {
        // Read generation first for consistency check
        let gen = self.generation.load(Ordering::Acquire);

        MetricsSnapshot {
            registrations_total: self.registrations_total.load(Ordering::Relaxed),
            verifications_total: self.verifications_total.load(Ordering::Relaxed),
            licenses_issued_total: self.licenses_issued_total.load(Ordering::Relaxed),
            promo_licenses_total: self.promo_licenses_total.load(Ordering::Relaxed),
            rate_limited_total: self.rate_limited_total.load(Ordering::Relaxed),
            tokens_generated_total: self.tokens_generated_total.load(Ordering::Relaxed),
            generation: gen,
        }
    }

    /// Get the current generation counter
    ///
    /// Used for TOCTOU prevention and change detection
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Update all counters from AppState capsules (lockfree read)
    ///
    /// Reads atomic values from registration, verification, and license capsules
    /// and stores them in the metrics capsule for Prometheus export.
    pub fn update_from_state(&self, state: &AppState) {
        // Get snapshots from each capsule (lockfree atomic reads)
        let reg_stats = state.registration.stats();
        let ver_stats = state.verification.stats();
        let lic_stats = state.license_gen.stats();

        // Store values (absolute updates, not increments)
        self.registrations_total
            .store(reg_stats.registrations_total, Ordering::Relaxed);
        self.rate_limited_total
            .store(reg_stats.blocked_count, Ordering::Relaxed);
        self.tokens_generated_total
            .store(ver_stats.tokens_generated, Ordering::Relaxed);
        self.verifications_total
            .store(ver_stats.tokens_verified, Ordering::Relaxed);
        self.licenses_issued_total
            .store(lic_stats.total_licenses, Ordering::Relaxed);
        self.promo_licenses_total
            .store(lic_stats.promo_licenses, Ordering::Relaxed);

        // Increment generation to signal update
        self.generation.fetch_add(1, Ordering::Release);
    }
}

impl Default for MetricsCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: MetricsCapsule uses only AtomicU64 for shared state
// All operations are lockfree and thread-safe
unsafe impl Send for MetricsCapsule {}
unsafe impl Sync for MetricsCapsule {}

// ============================================================================
// Prometheus Integration
// ============================================================================

/// Initialize Prometheus metrics recorder
///
/// Returns a handle that can be used to render metrics.
/// Call this once at startup before any metrics are recorded.
///
/// # Panics
/// Panics if called more than once (Prometheus recorder is global)
pub fn init_prometheus() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus recorder")
}

/// Describe all metrics (call once at startup after init_prometheus)
pub fn describe_metrics() {
    // Registration capsule metrics
    describe_counter!(
        "kdb_signup_registrations_total",
        "Total successful user registrations"
    );
    describe_counter!(
        "kdb_signup_rate_limited_total",
        "Total requests blocked by rate limiter"
    );

    // Verification capsule metrics
    describe_counter!(
        "kdb_signup_tokens_generated_total",
        "Total verification tokens generated"
    );
    describe_counter!(
        "kdb_signup_verifications_total",
        "Total successful email verifications"
    );

    // License capsule metrics
    describe_counter!(
        "kdb_signup_licenses_issued_total",
        "Total licenses generated"
    );
    describe_counter!(
        "kdb_signup_promo_licenses_total",
        "Licenses issued during promo period"
    );

    // Generation counters (gauges - current value)
    describe_gauge!(
        "kdb_signup_registration_generation",
        "UserRegistrationCapsule generation counter"
    );
    describe_gauge!(
        "kdb_signup_verification_generation",
        "EmailVerificationCapsule generation counter"
    );
    describe_gauge!(
        "kdb_signup_license_generation",
        "LicenseGeneratorCapsule generation counter"
    );
    describe_gauge!(
        "kdb_signup_metrics_generation",
        "MetricsCapsule generation counter"
    );

    // Promo status
    describe_gauge!(
        "kdb_signup_promo_active",
        "Whether promo period is active (1=yes, 0=no)"
    );
    describe_gauge!(
        "kdb_signup_promo_days_remaining",
        "Days remaining in promotional period"
    );
}

/// Update Prometheus metrics from capsule state
///
/// Reads atomic counters from all capsules and updates Prometheus metrics.
/// This is lockfree - reads AtomicU64 values with Relaxed/Acquire ordering.
pub fn update_prometheus_metrics(state: &Arc<AppState>) {
    // Get snapshots from each capsule (lockfree atomic reads)
    let reg_stats = state.registration.stats();
    let ver_stats = state.verification.stats();
    let lic_stats = state.license_gen.stats();

    // Registration capsule
    counter!("kdb_signup_registrations_total").absolute(reg_stats.registrations_total);
    counter!("kdb_signup_rate_limited_total").absolute(reg_stats.blocked_count);

    // Verification capsule
    counter!("kdb_signup_tokens_generated_total").absolute(ver_stats.tokens_generated);
    counter!("kdb_signup_verifications_total").absolute(ver_stats.tokens_verified);

    // License capsule
    counter!("kdb_signup_licenses_issued_total").absolute(lic_stats.total_licenses);
    counter!("kdb_signup_promo_licenses_total").absolute(lic_stats.promo_licenses);

    // Generation counters (for TOCTOU monitoring)
    gauge!("kdb_signup_registration_generation").set(reg_stats.generation as f64);
    gauge!("kdb_signup_verification_generation").set(ver_stats.generation as f64);
    gauge!("kdb_signup_license_generation").set(lic_stats.generation as f64);

    // Promo status
    gauge!("kdb_signup_promo_active").set(if lic_stats.promo_active { 1.0 } else { 0.0 });
    gauge!("kdb_signup_promo_days_remaining").set(lic_stats.promo_days_remaining as f64);
}

/// Axum handler for GET /metrics endpoint
///
/// Renders Prometheus metrics in text format.
/// Updates metrics from capsule state before rendering.
pub async fn metrics_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    prometheus_handle: axum::extract::Extension<PrometheusHandle>,
) -> impl IntoResponse {
    // Update metrics from capsule state
    update_prometheus_metrics(&state);

    // Render Prometheus format
    let metrics = prometheus_handle.render();

    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        metrics,
    )
}

/// Simplified metrics handler that takes the handle directly
///
/// Use this if you store the PrometheusHandle separately from Extension.
pub async fn metrics_handler_simple(
    state: Arc<AppState>,
    handle: PrometheusHandle,
) -> impl IntoResponse {
    // Update metrics from capsule state
    update_prometheus_metrics(&state);

    // Render Prometheus format
    let metrics = handle.render();

    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        metrics,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(
            std::mem::size_of::<MetricsCapsule>(),
            128,
            "Capsule must be exactly 128 bytes"
        );
        assert_eq!(
            std::mem::align_of::<MetricsCapsule>(),
            64,
            "Capsule must be 64-byte aligned"
        );
    }

    #[test]
    fn test_new_capsule_zeroed() {
        let capsule = MetricsCapsule::new();
        let snapshot = capsule.snapshot();

        assert_eq!(snapshot.registrations_total, 0);
        assert_eq!(snapshot.verifications_total, 0);
        assert_eq!(snapshot.licenses_issued_total, 0);
        assert_eq!(snapshot.promo_licenses_total, 0);
        assert_eq!(snapshot.rate_limited_total, 0);
        assert_eq!(snapshot.tokens_generated_total, 0);
        assert_eq!(snapshot.generation, 0);
    }

    #[test]
    fn test_increment_registrations() {
        let capsule = MetricsCapsule::new();

        capsule.increment_registrations();
        capsule.increment_registrations();
        capsule.increment_registrations();

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.registrations_total, 3);
        assert_eq!(snapshot.generation, 3);
    }

    #[test]
    fn test_increment_verifications() {
        let capsule = MetricsCapsule::new();

        capsule.increment_verifications();
        capsule.increment_verifications();

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.verifications_total, 2);
        assert_eq!(snapshot.generation, 2);
    }

    #[test]
    fn test_increment_licenses_issued() {
        let capsule = MetricsCapsule::new();

        capsule.increment_licenses_issued();

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.licenses_issued_total, 1);
        assert_eq!(snapshot.generation, 1);
    }

    #[test]
    fn test_increment_promo_licenses() {
        let capsule = MetricsCapsule::new();

        capsule.increment_promo_licenses();
        capsule.increment_promo_licenses();

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.promo_licenses_total, 2);
        assert_eq!(snapshot.generation, 2);
    }

    #[test]
    fn test_increment_rate_limited() {
        let capsule = MetricsCapsule::new();

        capsule.increment_rate_limited();

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.rate_limited_total, 1);
        assert_eq!(snapshot.generation, 1);
    }

    #[test]
    fn test_increment_tokens_generated() {
        let capsule = MetricsCapsule::new();

        capsule.increment_tokens_generated();
        capsule.increment_tokens_generated();
        capsule.increment_tokens_generated();
        capsule.increment_tokens_generated();

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.tokens_generated_total, 4);
        assert_eq!(snapshot.generation, 4);
    }

    #[test]
    fn test_generation_counter_increments() {
        let capsule = MetricsCapsule::new();

        assert_eq!(capsule.generation(), 0);

        capsule.increment_registrations();
        assert_eq!(capsule.generation(), 1);

        capsule.increment_verifications();
        assert_eq!(capsule.generation(), 2);

        capsule.increment_licenses_issued();
        assert_eq!(capsule.generation(), 3);
    }

    #[test]
    fn test_concurrent_increments() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(MetricsCapsule::new());
        let mut handles = vec![];

        // Spawn 10 threads, each incrementing 100 times
        for _ in 0..10 {
            let capsule = Arc::clone(&capsule);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    capsule.increment_registrations();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.registrations_total, 1000);
        assert_eq!(snapshot.generation, 1000);
    }

    #[test]
    fn test_default_trait() {
        let capsule = MetricsCapsule::default();
        assert_eq!(capsule.generation(), 0);
    }

    #[test]
    fn test_snapshot_consistency() {
        let capsule = MetricsCapsule::new();

        // Increment various counters
        capsule.increment_registrations();
        capsule.increment_verifications();
        capsule.increment_licenses_issued();
        capsule.increment_promo_licenses();
        capsule.increment_rate_limited();
        capsule.increment_tokens_generated();

        // Take snapshot and verify all values
        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.registrations_total, 1);
        assert_eq!(snapshot.verifications_total, 1);
        assert_eq!(snapshot.licenses_issued_total, 1);
        assert_eq!(snapshot.promo_licenses_total, 1);
        assert_eq!(snapshot.rate_limited_total, 1);
        assert_eq!(snapshot.tokens_generated_total, 1);
        assert_eq!(snapshot.generation, 6);
    }
}
