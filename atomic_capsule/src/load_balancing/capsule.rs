//! Main HealthCheckCapsule (256B, T1+T8)

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use core::mem::size_of;

#[cfg(feature = "std")]
use std::collections::HashMap;

use super::check_types::{HealthCheckError, HealthCheckResult, HealthCheckType, HealthStatus};
use super::backend_state::BackendHealthState;
use super::passive_monitoring::PassiveHealthMonitor;
use super::circuit_breaker_integration::{CircuitBreakerIntegration, CircuitBreakerState};

/// Health check capsule for load balancer backends (256B, T1+T8)
///
/// Coordinates active and passive health checks across multiple backends.
/// All operations are lockfree atomic.
///
/// # Layout
///
/// ```ignore
/// [check_state: u64] (coordination flags)
/// [backends_ptr: u64] (pointer to backend array - optional)
/// [check_interval_ms: u32][timeout_ms: u32]
/// [healthy_threshold: u8][unhealthy_threshold: u8]
/// [total_checks: u64][successful_checks: u64][failed_checks: u64][timeout_checks: u64]
/// [healthy_backends: u32][unhealthy_backends: u32][draining_backends: u32]
/// [http_probes: u64][tcp_probes: u64][icmp_probes: u64]
/// [passive_successes: u64][passive_failures: u64]
/// [circuit_breaker_triggers: u32]
/// [last_check_cycle_ns: u64]
/// [padding: to 256 bytes]
/// = 256 bytes total
/// ```
#[repr(C, align(256))]
pub struct HealthCheckCapsule {
    /// State coordination flags
    check_state: AtomicU64,

    /// Pointer to backend array (0 if not using external storage)
    backends_ptr: AtomicU64,

    /// Health check interval (milliseconds, default 5000)
    check_interval_ms: AtomicU32,

    /// Health check timeout (milliseconds, default 3000)
    timeout_ms: AtomicU32,

    /// Consecutive successes needed to mark healthy
    healthy_threshold: AtomicU32,

    /// Consecutive failures needed to mark unhealthy
    unhealthy_threshold: AtomicU32,

    /// Total health checks performed
    total_checks: AtomicU64,

    /// Successful health checks
    successful_checks: AtomicU64,

    /// Failed health checks
    failed_checks: AtomicU64,

    /// Timed-out health checks
    timeout_checks: AtomicU64,

    /// Count of healthy backends
    healthy_backends: AtomicU32,

    /// Count of unhealthy backends
    unhealthy_backends: AtomicU32,

    /// Count of draining backends
    draining_backends: AtomicU32,

    /// Number of HTTP probes executed
    http_probes: AtomicU64,

    /// Number of TCP probes executed
    tcp_probes: AtomicU64,

    /// Number of ICMP probes executed
    icmp_probes: AtomicU64,

    /// Passive monitoring successes
    passive_successes: AtomicU64,

    /// Passive monitoring failures
    passive_failures: AtomicU64,

    /// Circuit breaker triggers
    circuit_breaker_triggers: AtomicU32,

    /// Last health check cycle timestamp (ns)
    last_check_cycle_ns: AtomicU64,

    /// Padding to reach 256 bytes
    _padding: [u8; 24],
}

impl HealthCheckCapsule {
    /// Create a new health check capsule
    pub fn new() -> Self {
        HealthCheckCapsule {
            check_state: AtomicU64::new(0),
            backends_ptr: AtomicU64::new(0),
            check_interval_ms: AtomicU32::new(5000),
            timeout_ms: AtomicU32::new(3000),
            healthy_threshold: AtomicU32::new(2),
            unhealthy_threshold: AtomicU32::new(3),
            total_checks: AtomicU64::new(0),
            successful_checks: AtomicU64::new(0),
            failed_checks: AtomicU64::new(0),
            timeout_checks: AtomicU64::new(0),
            healthy_backends: AtomicU32::new(0),
            unhealthy_backends: AtomicU32::new(0),
            draining_backends: AtomicU32::new(0),
            http_probes: AtomicU64::new(0),
            tcp_probes: AtomicU64::new(0),
            icmp_probes: AtomicU64::new(0),
            passive_successes: AtomicU64::new(0),
            passive_failures: AtomicU64::new(0),
            circuit_breaker_triggers: AtomicU32::new(0),
            last_check_cycle_ns: AtomicU64::new(0),
            _padding: [0; 24],
        }
    }

    /// Set health check interval (milliseconds)
    pub fn set_check_interval_ms(&self, interval_ms: u32) {
        self.check_interval_ms.store(interval_ms, Ordering::Release);
    }

    /// Get health check interval (milliseconds)
    pub fn get_check_interval_ms(&self) -> u32 {
        self.check_interval_ms.load(Ordering::Acquire)
    }

    /// Set health check timeout (milliseconds)
    pub fn set_timeout_ms(&self, timeout_ms: u32) {
        self.timeout_ms.store(timeout_ms, Ordering::Release);
    }

    /// Get health check timeout (milliseconds)
    pub fn get_timeout_ms(&self) -> u32 {
        self.timeout_ms.load(Ordering::Acquire)
    }

    /// Set consecutive successes threshold
    pub fn set_healthy_threshold(&self, threshold: u8) {
        self.healthy_threshold
            .store(threshold as u32, Ordering::Release);
    }

    /// Get consecutive successes threshold
    pub fn get_healthy_threshold(&self) -> u8 {
        self.healthy_threshold.load(Ordering::Acquire) as u8
    }

    /// Set consecutive failures threshold
    pub fn set_unhealthy_threshold(&self, threshold: u8) {
        self.unhealthy_threshold
            .store(threshold as u32, Ordering::Release);
    }

    /// Get consecutive failures threshold
    pub fn get_unhealthy_threshold(&self) -> u8 {
        self.unhealthy_threshold.load(Ordering::Acquire) as u8
    }

    /// Record a health check result
    pub fn record_check_result(
        &self,
        result: HealthCheckResult,
    ) {
        self.total_checks.fetch_add(1, Ordering::Release);

        if result.success {
            self.successful_checks.fetch_add(1, Ordering::Release);
        } else {
            self.failed_checks.fetch_add(1, Ordering::Release);
            if result.error == Some(crate::load_balancing::check_types::ErrorType::RequestTimeout) {
                self.timeout_checks.fetch_add(1, Ordering::Release);
            }
        }
    }

    /// Record a passive monitoring success
    pub fn record_passive_success(&self, backend: &BackendHealthState, latency_ns: u64) {
        let healthy_threshold = self.get_healthy_threshold();
        PassiveHealthMonitor::record_success(backend, latency_ns, healthy_threshold);
        self.passive_successes.fetch_add(1, Ordering::Release);
    }

    /// Record a passive monitoring failure
    pub fn record_passive_failure(
        &self,
        backend: &BackendHealthState,
        error: crate::load_balancing::check_types::ErrorType,
    ) {
        let unhealthy_threshold = self.get_unhealthy_threshold();
        PassiveHealthMonitor::record_failure(backend, error, unhealthy_threshold);
        self.passive_failures.fetch_add(1, Ordering::Release);
    }

    /// Get total checks performed
    pub fn total_checks(&self) -> u64 {
        self.total_checks.load(Ordering::Acquire)
    }

    /// Get successful checks
    pub fn successful_checks(&self) -> u64 {
        self.successful_checks.load(Ordering::Acquire)
    }

    /// Get failed checks
    pub fn failed_checks(&self) -> u64 {
        self.failed_checks.load(Ordering::Acquire)
    }

    /// Get timeout checks
    pub fn timeout_checks(&self) -> u64 {
        self.timeout_checks.load(Ordering::Acquire)
    }

    /// Get overall success rate (0-100%)
    pub fn overall_success_rate(&self) -> u32 {
        let total = self.total_checks.load(Ordering::Acquire);
        if total == 0 {
            return 0;
        }
        let success = self.successful_checks.load(Ordering::Acquire);
        ((success * 100) / total) as u32
    }

    /// Get count of healthy backends
    pub fn healthy_backends_count(&self) -> u32 {
        self.healthy_backends.load(Ordering::Acquire)
    }

    /// Increment healthy backends count
    pub fn increment_healthy_backends(&self) {
        self.healthy_backends.fetch_add(1, Ordering::Release);
    }

    /// Decrement healthy backends count
    pub fn decrement_healthy_backends(&self) {
        self.healthy_backends.fetch_sub(1, Ordering::Release);
    }

    /// Get count of unhealthy backends
    pub fn unhealthy_backends_count(&self) -> u32 {
        self.unhealthy_backends.load(Ordering::Acquire)
    }

    /// Increment unhealthy backends count
    pub fn increment_unhealthy_backends(&self) {
        self.unhealthy_backends.fetch_add(1, Ordering::Release);
    }

    /// Decrement unhealthy backends count
    pub fn decrement_unhealthy_backends(&self) {
        self.unhealthy_backends.fetch_sub(1, Ordering::Release);
    }

    /// Get count of draining backends
    pub fn draining_backends_count(&self) -> u32 {
        self.draining_backends.load(Ordering::Acquire)
    }

    /// Increment draining backends count
    pub fn increment_draining_backends(&self) {
        self.draining_backends.fetch_add(1, Ordering::Release);
    }

    /// Decrement draining backends count
    pub fn decrement_draining_backends(&self) {
        self.draining_backends.fetch_sub(1, Ordering::Release);
    }

    /// Record HTTP probe execution
    pub fn record_http_probe(&self) {
        self.http_probes.fetch_add(1, Ordering::Release);
    }

    /// Get HTTP probes count
    pub fn http_probes_count(&self) -> u64 {
        self.http_probes.load(Ordering::Acquire)
    }

    /// Record TCP probe execution
    pub fn record_tcp_probe(&self) {
        self.tcp_probes.fetch_add(1, Ordering::Release);
    }

    /// Get TCP probes count
    pub fn tcp_probes_count(&self) -> u64 {
        self.tcp_probes.load(Ordering::Acquire)
    }

    /// Record ICMP probe execution
    pub fn record_icmp_probe(&self) {
        self.icmp_probes.fetch_add(1, Ordering::Release);
    }

    /// Get ICMP probes count
    pub fn icmp_probes_count(&self) -> u64 {
        self.icmp_probes.load(Ordering::Acquire)
    }

    /// Trigger circuit breaker
    pub fn trigger_circuit_breaker(&self) {
        self.circuit_breaker_triggers
            .fetch_add(1, Ordering::Release);
    }

    /// Get circuit breaker triggers count
    pub fn circuit_breaker_triggers_count(&self) -> u32 {
        self.circuit_breaker_triggers.load(Ordering::Acquire)
    }

    /// Set last check cycle timestamp
    pub fn set_last_check_cycle_ns(&self, ts_ns: u64) {
        self.last_check_cycle_ns.store(ts_ns, Ordering::Release);
    }

    /// Get last check cycle timestamp
    pub fn get_last_check_cycle_ns(&self) -> u64 {
        self.last_check_cycle_ns.load(Ordering::Acquire)
    }

    /// Perform active HTTP health check
    ///
    /// This is a stub implementation that returns a successful result.
    /// A full implementation would make actual HTTP requests.
    pub fn check_http_health(
        &self,
        _backend_id: u32,
        _path: &str,
        _expected_status: u16,
    ) -> Result<HealthCheckResult, HealthCheckError> {
        // Record probe
        self.record_http_probe();

        // Simulated result: success with 500ns latency
        Ok(HealthCheckResult::http_success(500, 200))
    }

    /// Perform active TCP health check
    ///
    /// This is a stub implementation that returns a successful result.
    /// A full implementation would attempt TCP connection.
    pub fn check_tcp_health(
        &self,
        _backend_id: u32,
        _port: u16,
    ) -> Result<HealthCheckResult, HealthCheckError> {
        // Record probe
        self.record_tcp_probe();

        // Simulated result: success with 100ns latency
        Ok(HealthCheckResult::success(100))
    }

    /// Get backend health status
    pub fn get_backend_health(
        &self,
        backend: &BackendHealthState,
    ) -> Result<HealthStatus, HealthCheckError> {
        Ok(backend.health_status())
    }

    /// Manually set backend health status
    pub fn set_backend_health(
        &self,
        backend: &BackendHealthState,
        status: HealthStatus,
    ) -> Result<(), HealthCheckError> {
        backend.set_health_status(status);
        Ok(())
    }

    /// Drain a backend (finish existing connections, don't accept new ones)
    pub fn drain_backend(&self, backend: &BackendHealthState) -> Result<(), HealthCheckError> {
        backend.set_draining(true);
        backend.set_health_status(HealthStatus::Draining);
        self.increment_draining_backends();
        Ok(())
    }

    /// Resume a backend from draining
    pub fn resume_backend(&self, backend: &BackendHealthState) -> Result<(), HealthCheckError> {
        backend.set_draining(false);
        if backend.health_status() == HealthStatus::Draining {
            backend.set_health_status(HealthStatus::Healthy);
        }
        self.decrement_draining_backends();
        Ok(())
    }

    /// Check if should trigger circuit breaker transition
    pub fn evaluate_circuit_breaker(
        &self,
        backend: &BackendHealthState,
        error_threshold_percent: u32,
        min_samples: u32,
    ) -> bool {
        let should_open =
            CircuitBreakerIntegration::should_open_circuit(backend, error_threshold_percent, min_samples);

        if should_open {
            self.trigger_circuit_breaker();
            backend.set_health_status(HealthStatus::Unhealthy);
        }

        should_open
    }

    /// Verify struct size is exactly 256 bytes
    pub const fn verify_size() {
        // Compile-time size check via transmute
        let _ = core::mem::transmute::<HealthCheckCapsule, [u8; 256]>;
    }
}

// Compile-time size verification
const _: () = {
    const fn assert_size() {
        let _ = core::mem::transmute::<HealthCheckCapsule, [u8; 256]>;
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size() {
        assert_eq!(size_of::<HealthCheckCapsule>(), 256);
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(core::mem::align_of::<HealthCheckCapsule>(), 256);
    }

    #[test]
    fn test_capsule_creation() {
        let capsule = HealthCheckCapsule::new();
        assert_eq!(capsule.get_check_interval_ms(), 5000);
        assert_eq!(capsule.get_timeout_ms(), 3000);
        assert_eq!(capsule.get_healthy_threshold(), 2);
        assert_eq!(capsule.get_unhealthy_threshold(), 3);
    }

    #[test]
    fn test_interval_configuration() {
        let capsule = HealthCheckCapsule::new();
        capsule.set_check_interval_ms(10000);
        assert_eq!(capsule.get_check_interval_ms(), 10000);
    }

    #[test]
    fn test_threshold_configuration() {
        let capsule = HealthCheckCapsule::new();
        capsule.set_healthy_threshold(3);
        capsule.set_unhealthy_threshold(5);
        assert_eq!(capsule.get_healthy_threshold(), 3);
        assert_eq!(capsule.get_unhealthy_threshold(), 5);
    }

    #[test]
    fn test_check_statistics() {
        let capsule = HealthCheckCapsule::new();

        let result = HealthCheckResult::success(500);
        capsule.record_check_result(result);
        capsule.record_check_result(result);

        let fail_result = HealthCheckResult::failure(
            1000,
            crate::load_balancing::check_types::ErrorType::ConnectionRefused,
        );
        capsule.record_check_result(fail_result);

        assert_eq!(capsule.total_checks(), 3);
        assert_eq!(capsule.successful_checks(), 2);
        assert_eq!(capsule.failed_checks(), 1);
        assert_eq!(capsule.overall_success_rate(), 66);
    }

    #[test]
    fn test_backend_counters() {
        let capsule = HealthCheckCapsule::new();

        capsule.increment_healthy_backends();
        capsule.increment_healthy_backends();
        assert_eq!(capsule.healthy_backends_count(), 2);

        capsule.increment_unhealthy_backends();
        assert_eq!(capsule.unhealthy_backends_count(), 1);

        capsule.decrement_healthy_backends();
        assert_eq!(capsule.healthy_backends_count(), 1);
    }

    #[test]
    fn test_probe_tracking() {
        let capsule = HealthCheckCapsule::new();

        capsule.record_http_probe();
        capsule.record_http_probe();
        assert_eq!(capsule.http_probes_count(), 2);

        capsule.record_tcp_probe();
        assert_eq!(capsule.tcp_probes_count(), 1);

        capsule.record_icmp_probe();
        assert_eq!(capsule.icmp_probes_count(), 1);
    }

    #[test]
    fn test_drain_backend() {
        let capsule = HealthCheckCapsule::new();
        let backend = BackendHealthState::new(1);

        backend.set_health_status(HealthStatus::Healthy);
        assert!(!backend.is_draining());

        capsule.drain_backend(&backend).unwrap();
        assert!(backend.is_draining());
        assert_eq!(backend.health_status(), HealthStatus::Draining);
        assert_eq!(capsule.draining_backends_count(), 1);
    }

    #[test]
    fn test_resume_backend() {
        let capsule = HealthCheckCapsule::new();
        let backend = BackendHealthState::new(1);

        backend.set_draining(true);
        capsule.increment_draining_backends();

        capsule.resume_backend(&backend).unwrap();
        assert!(!backend.is_draining());
        assert_eq!(capsule.draining_backends_count(), 0);
    }

    #[test]
    fn test_circuit_breaker_trigger() {
        let capsule = HealthCheckCapsule::new();

        capsule.trigger_circuit_breaker();
        capsule.trigger_circuit_breaker();
        assert_eq!(capsule.circuit_breaker_triggers_count(), 2);
    }

    #[test]
    fn test_passive_monitoring() {
        let capsule = HealthCheckCapsule::new();
        let backend = BackendHealthState::new(1);

        capsule.record_passive_success(&backend, 500);
        capsule.record_passive_success(&backend, 600);

        assert_eq!(capsule.passive_successes.load(Ordering::Acquire), 2);
    }
}
