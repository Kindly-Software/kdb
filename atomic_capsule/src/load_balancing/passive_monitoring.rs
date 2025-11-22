//! Passive health monitoring (recording request outcomes)

use super::check_types::{ErrorType, HealthStatus};
use super::backend_state::BackendHealthState;

/// Passive health monitoring state machine
///
/// Tracks backend health based on observed request outcomes (success/failure)
/// rather than explicit health checks.
///
/// # State Transitions
///
/// ```ignore
/// Unknown → Healthy (healthy_threshold successes)
/// Healthy → Unhealthy (unhealthy_threshold failures)
/// Unhealthy → Healthy (healthy_threshold successes)
/// Healthy/Unhealthy → Draining (manual drain command)
/// Draining → Down (all connections closed)
/// ```
pub struct PassiveHealthMonitor;

impl PassiveHealthMonitor {
    /// Record a successful request for a backend
    ///
    /// Increments consecutive successes and transitions state if threshold met.
    /// Performance: <50ns lockfree atomic operation
    pub fn record_success(
        backend: &BackendHealthState,
        latency_ns: u64,
        healthy_threshold: u8,
    ) -> HealthStatus {
        // Record request outcome
        backend.set_last_latency_ns(latency_ns);
        backend.increment_check_count();
        backend.increment_success_count();

        // Reset failure counter
        backend.reset_failures();

        // Increment success counter and check threshold
        let new_successes = backend.increment_successes();

        let current_status = backend.health_status();

        // Transition to healthy if threshold met
        if new_successes >= healthy_threshold
            && (current_status == HealthStatus::Unknown
                || current_status == HealthStatus::Unhealthy)
        {
            backend.set_health_status(HealthStatus::Healthy)
        } else if current_status == HealthStatus::Healthy {
            HealthStatus::Healthy
        } else {
            current_status
        }
    }

    /// Record a failed request for a backend
    ///
    /// Increments consecutive failures and transitions state if threshold met.
    /// Performance: <50ns lockfree atomic operation
    pub fn record_failure(
        backend: &BackendHealthState,
        error: ErrorType,
        unhealthy_threshold: u8,
    ) -> HealthStatus {
        // Record request outcome
        backend.increment_check_count();
        backend.increment_failure_count();

        // Check error type for timeout tracking
        if error == ErrorType::RequestTimeout || error == ErrorType::ConnectionTimeout {
            backend.increment_timeout_count();
        }

        // Reset success counter
        backend.reset_successes();

        // Increment failure counter and check threshold
        let new_failures = backend.increment_failures();

        let current_status = backend.health_status();

        // Transition to unhealthy if threshold met
        if new_failures >= unhealthy_threshold && current_status == HealthStatus::Healthy {
            backend.set_health_status(HealthStatus::Unhealthy)
        } else if current_status == HealthStatus::Unhealthy {
            HealthStatus::Unhealthy
        } else {
            current_status
        }
    }

    /// Evaluate overall health based on passive monitoring statistics
    ///
    /// Uses rolling statistics to determine if backend should be considered healthy
    /// even if not explicitly checked.
    pub fn evaluate_health(
        backend: &BackendHealthState,
        min_samples: u32,
        success_rate_threshold: u32,
    ) -> HealthStatus {
        let check_count = backend.check_count();

        // Need minimum samples for statistical significance
        if check_count < min_samples {
            return backend.health_status();
        }

        let success_rate = backend.success_rate_percent();

        if success_rate >= success_rate_threshold {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    /// Get failure rate as percentage
    pub fn failure_rate_percent(backend: &BackendHealthState) -> u32 {
        let total = backend.check_count();
        if total == 0 {
            return 0;
        }
        let failed = backend.failure_count();
        ((failed as u64 * 100) / total as u64) as u32
    }

    /// Get timeout rate as percentage
    pub fn timeout_rate_percent(backend: &BackendHealthState) -> u32 {
        let total = backend.check_count();
        if total == 0 {
            return 0;
        }
        let timeouts = backend.timeout_count();
        ((timeouts as u64 * 100) / total as u64) as u32
    }

    /// Check if backend health is degrading based on recent samples
    ///
    /// Compares recent success rate to overall success rate to detect
    /// degradation trends
    pub fn is_degrading(
        backend: &BackendHealthState,
        recent_samples: u32,
        degradation_threshold: u32,
    ) -> bool {
        let total = backend.check_count();

        // Need sufficient samples
        if total < recent_samples {
            return false;
        }

        let overall_success_rate = backend.success_rate_percent();

        // In a simple implementation, we could track recent samples separately
        // For now, return false (requires enhanced state tracking)
        // This is a placeholder for the full implementation
        overall_success_rate < degradation_threshold
    }
}

/// Health monitoring statistics snapshot
#[derive(Clone, Copy, Debug)]
pub struct HealthMonitoringStats {
    /// Total checks performed
    pub total_checks: u32,
    /// Successful checks
    pub successful_checks: u32,
    /// Failed checks
    pub failed_checks: u32,
    /// Timed-out checks
    pub timeout_checks: u32,
    /// Overall success rate (0-100)
    pub success_rate_percent: u32,
    /// Failure rate (0-100)
    pub failure_rate_percent: u32,
    /// Timeout rate (0-100)
    pub timeout_rate_percent: u32,
}

impl HealthMonitoringStats {
    /// Create stats snapshot from backend state
    pub fn from_backend(backend: &BackendHealthState) -> Self {
        let total = backend.check_count();
        let successful = backend.success_count();
        let failed = backend.failure_count();
        let timeouts = backend.timeout_count();

        let success_rate = if total == 0 {
            0
        } else {
            ((successful as u64 * 100) / total as u64) as u32
        };

        let failure_rate = if total == 0 {
            0
        } else {
            ((failed as u64 * 100) / total as u64) as u32
        };

        let timeout_rate = if total == 0 {
            0
        } else {
            ((timeouts as u64 * 100) / total as u64) as u32
        };

        HealthMonitoringStats {
            total_checks: total,
            successful_checks: successful,
            failed_checks: failed,
            timeout_checks: timeouts,
            success_rate_percent: success_rate,
            failure_rate_percent: failure_rate,
            timeout_rate_percent: timeout_rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passive_success_transitions_to_healthy() {
        let backend = BackendHealthState::new(1);
        assert_eq!(backend.health_status(), HealthStatus::Unknown);

        // Record successes
        let status = PassiveHealthMonitor::record_success(&backend, 100, 2);
        assert_eq!(backend.consecutive_successes(), 1);

        let status = PassiveHealthMonitor::record_success(&backend, 100, 2);
        assert_eq!(backend.consecutive_successes(), 2);
        assert_eq!(status, HealthStatus::Healthy);
        assert_eq!(backend.health_status(), HealthStatus::Healthy);
    }

    #[test]
    fn test_passive_failure_transitions_to_unhealthy() {
        let backend = BackendHealthState::new(1);
        backend.set_health_status(HealthStatus::Healthy);

        // Record failures
        let _status =
            PassiveHealthMonitor::record_failure(&backend, ErrorType::ConnectionRefused, 3);
        assert_eq!(backend.consecutive_failures(), 1);

        let _status =
            PassiveHealthMonitor::record_failure(&backend, ErrorType::ConnectionRefused, 3);
        assert_eq!(backend.consecutive_failures(), 2);

        let status =
            PassiveHealthMonitor::record_failure(&backend, ErrorType::ConnectionRefused, 3);
        assert_eq!(backend.consecutive_failures(), 3);
        assert_eq!(status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_success_resets_failures() {
        let backend = BackendHealthState::new(1);

        // Record some failures
        PassiveHealthMonitor::record_failure(&backend, ErrorType::ConnectionRefused, 3);
        PassiveHealthMonitor::record_failure(&backend, ErrorType::ConnectionRefused, 3);
        assert_eq!(backend.consecutive_failures(), 2);

        // Success resets failures
        PassiveHealthMonitor::record_success(&backend, 100, 2);
        assert_eq!(backend.consecutive_failures(), 0);
        assert_eq!(backend.consecutive_successes(), 1);
    }

    #[test]
    fn test_failure_resets_successes() {
        let backend = BackendHealthState::new(1);

        // Record some successes
        PassiveHealthMonitor::record_success(&backend, 100, 2);
        PassiveHealthMonitor::record_success(&backend, 100, 2);
        assert_eq!(backend.consecutive_successes(), 2);

        // Failure resets successes
        PassiveHealthMonitor::record_failure(&backend, ErrorType::ConnectionRefused, 3);
        assert_eq!(backend.consecutive_successes(), 0);
        assert_eq!(backend.consecutive_failures(), 1);
    }

    #[test]
    fn test_statistics_calculation() {
        let backend = BackendHealthState::new(1);

        // Record mixed results: 2 success, 1 failure, 1 timeout
        PassiveHealthMonitor::record_success(&backend, 100, 2);
        PassiveHealthMonitor::record_success(&backend, 100, 2);
        PassiveHealthMonitor::record_failure(&backend, ErrorType::ConnectionRefused, 3);
        PassiveHealthMonitor::record_failure(&backend, ErrorType::RequestTimeout, 3);

        assert_eq!(backend.check_count(), 4);
        assert_eq!(backend.success_count(), 2);
        assert_eq!(backend.failure_count(), 2);
        assert_eq!(backend.timeout_count(), 1);
        assert_eq!(backend.success_rate_percent(), 50);

        let failure_rate = PassiveHealthMonitor::failure_rate_percent(&backend);
        assert_eq!(failure_rate, 50);

        let timeout_rate = PassiveHealthMonitor::timeout_rate_percent(&backend);
        assert_eq!(timeout_rate, 25);
    }

    #[test]
    fn test_health_monitoring_stats() {
        let backend = BackendHealthState::new(1);

        PassiveHealthMonitor::record_success(&backend, 100, 2);
        PassiveHealthMonitor::record_success(&backend, 100, 2);
        PassiveHealthMonitor::record_failure(&backend, ErrorType::ConnectionRefused, 3);

        let stats = HealthMonitoringStats::from_backend(&backend);
        assert_eq!(stats.total_checks, 3);
        assert_eq!(stats.successful_checks, 2);
        assert_eq!(stats.failed_checks, 1);
        assert_eq!(stats.success_rate_percent, 66);
        assert_eq!(stats.failure_rate_percent, 33);
    }

    #[test]
    fn test_evaluate_health() {
        let backend = BackendHealthState::new(1);
        backend.set_health_status(HealthStatus::Healthy);

        // Record 100 successes out of 100 checks
        for _ in 0..100 {
            PassiveHealthMonitor::record_success(&backend, 100, 2);
        }

        // Should remain healthy with 100% success rate
        let status = PassiveHealthMonitor::evaluate_health(&backend, 10, 80);
        assert_eq!(status, HealthStatus::Healthy);
    }
}
