//! Circuit breaker integration for health check state management

use super::check_types::{ErrorType, HealthStatus};
use super::backend_state::BackendHealthState;

/// Circuit breaker state for backend coordination
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CircuitBreakerState {
    /// Accepting all requests (normal operation)
    Closed = 0,
    /// Rejecting all requests (failure detected)
    Open = 1,
    /// Testing if backend recovered (gradual recovery)
    HalfOpen = 2,
}

impl CircuitBreakerState {
    /// Convert from u8
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(CircuitBreakerState::Closed),
            1 => Some(CircuitBreakerState::Open),
            2 => Some(CircuitBreakerState::HalfOpen),
            _ => None,
        }
    }
}

/// Circuit breaker integration for health checking
pub struct CircuitBreakerIntegration;

impl CircuitBreakerIntegration {
    /// Determine circuit breaker state based on error type
    pub fn breaker_state_for_error(error: ErrorType) -> CircuitBreakerState {
        match error {
            // Permanent errors trigger Open state immediately
            ErrorType::ConnectionRefused | ErrorType::BackendNotFound => {
                CircuitBreakerState::Open
            }
            // Server errors trigger Open state
            ErrorType::HttpServerError => CircuitBreakerState::Open,
            // Transient errors keep Closed (retry later)
            ErrorType::ConnectionTimeout
            | ErrorType::RequestTimeout
            | ErrorType::NetworkError => CircuitBreakerState::Closed,
            // Config/internal errors don't trip circuit breaker
            ErrorType::InvalidConfig | ErrorType::InternalError => CircuitBreakerState::Closed,
        }
    }

    /// Update backend health based on circuit breaker state
    pub fn apply_breaker_state(
        backend: &BackendHealthState,
        breaker_state: CircuitBreakerState,
    ) -> HealthStatus {
        match breaker_state {
            CircuitBreakerState::Closed => {
                // Normal operation - keep current health if healthy
                let current = backend.health_status();
                if current == HealthStatus::Healthy {
                    current
                } else {
                    // Check if we can transition from Unhealthy back to Healthy
                    HealthStatus::Healthy
                }
            }
            CircuitBreakerState::Open => {
                // Circuit is open - mark unhealthy
                backend.set_health_status(HealthStatus::Unhealthy)
            }
            CircuitBreakerState::HalfOpen => {
                // Half-open - allow limited traffic to test recovery
                // Don't change health status, let passive monitoring handle updates
                backend.health_status()
            }
        }
    }

    /// Check if backend should be open-circuited based on error rate
    pub fn should_open_circuit(
        backend: &BackendHealthState,
        error_threshold_percent: u32,
        min_samples: u32,
    ) -> bool {
        let total = backend.check_count();

        // Need sufficient samples
        if total < min_samples {
            return false;
        }

        let failed = backend.failure_count();
        let error_rate = ((failed as u64 * 100) / total as u64) as u32;

        error_rate >= error_threshold_percent
    }

    /// Check if backend should transition from Open to HalfOpen
    pub fn should_half_open(
        backend: &BackendHealthState,
        open_duration_ns: u64,
        current_time_ns: u64,
    ) -> bool {
        let last_failure = backend.last_failure_time_ns();

        // Only transition if opened long enough to attempt recovery
        if last_failure == 0 {
            return false;
        }

        current_time_ns - last_failure >= open_duration_ns
    }

    /// Check if backend should transition from HalfOpen back to Closed (healthy)
    pub fn should_close_circuit(
        backend: &BackendHealthState,
        half_open_successes_threshold: u8,
    ) -> bool {
        backend.consecutive_successes() >= half_open_successes_threshold
    }

    /// Check if backend should stay Open (not attempt recovery yet)
    pub fn should_remain_open(
        backend: &BackendHealthState,
        min_open_duration_ns: u64,
        current_time_ns: u64,
    ) -> bool {
        let last_failure = backend.last_failure_time_ns();

        if last_failure == 0 {
            return false;
        }

        current_time_ns - last_failure < min_open_duration_ns
    }

    /// Get recommended retry delay based on error type
    pub fn retry_delay_ms(error: ErrorType) -> u32 {
        match error {
            // Transient errors: short retry delay
            ErrorType::ConnectionTimeout => 100,
            ErrorType::RequestTimeout => 100,
            ErrorType::NetworkError => 200,
            // Permanent errors: long delay or no retry
            ErrorType::ConnectionRefused => 5000,
            ErrorType::HttpServerError => 1000,
            ErrorType::BackendNotFound => 60000,
            // Configuration/internal errors: no immediate retry
            ErrorType::InvalidConfig | ErrorType::InternalError => 0,
        }
    }

    /// Get circuit breaker failure threshold based on error rate
    pub fn failure_threshold_percent(backend_type: &str) -> u32 {
        match backend_type {
            "critical" => 10, // 10% error rate triggers for critical backends
            "important" => 20, // 20% for important
            "normal" => 30,    // 30% for normal
            "best_effort" => 50, // 50% for best-effort
            _ => 30,           // Default
        }
    }

    /// Suggested duration to keep circuit open (exponential backoff)
    pub fn open_duration_ms(consecutive_opens: u32) -> u64 {
        // Exponential backoff: 1s, 2s, 4s, 8s, ..., up to 60s max
        let base_ms: u64 = 1000;
        let backoff = base_ms << consecutive_opens.min(6);
        backoff.min(60_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breaker_state_for_error() {
        // Permanent errors trigger Open
        assert_eq!(
            CircuitBreakerIntegration::breaker_state_for_error(
                ErrorType::ConnectionRefused
            ),
            CircuitBreakerState::Open
        );
        assert_eq!(
            CircuitBreakerIntegration::breaker_state_for_error(
                ErrorType::HttpServerError
            ),
            CircuitBreakerState::Open
        );

        // Transient errors stay Closed
        assert_eq!(
            CircuitBreakerIntegration::breaker_state_for_error(
                ErrorType::ConnectionTimeout
            ),
            CircuitBreakerState::Closed
        );
        assert_eq!(
            CircuitBreakerIntegration::breaker_state_for_error(
                ErrorType::RequestTimeout
            ),
            CircuitBreakerState::Closed
        );
    }

    #[test]
    fn test_should_open_circuit() {
        let backend = BackendHealthState::new(1);

        // Record 30% failure rate
        for _ in 0..7 {
            backend.increment_check_count();
            backend.increment_success_count();
        }
        for _ in 0..3 {
            backend.increment_check_count();
            backend.increment_failure_count();
        }

        // With 20% threshold, should NOT open (30% > 20%, but check implementation)
        assert!(CircuitBreakerIntegration::should_open_circuit(
            &backend,
            20,
            5
        ));
    }

    #[test]
    fn test_should_half_open() {
        let backend = BackendHealthState::new(1);
        let current_time_ns = 5_000_000_000; // 5 seconds - enough room for subtraction

        // Set last failure 2 seconds ago
        backend.set_last_failure_time_ns(current_time_ns - 2_000_000_000);

        // Should transition to HalfOpen after 1 second
        assert!(CircuitBreakerIntegration::should_half_open(
            &backend,
            1_000_000_000,
            current_time_ns
        ));

        // Should NOT transition if still within minimum time
        assert!(!CircuitBreakerIntegration::should_half_open(
            &backend,
            3_000_000_000,
            current_time_ns
        ));
    }

    #[test]
    fn test_should_close_circuit() {
        let backend = BackendHealthState::new(1);

        // Need 2 consecutive successes to close
        backend.increment_successes();
        assert!(!CircuitBreakerIntegration::should_close_circuit(&backend, 2));

        backend.increment_successes();
        assert!(CircuitBreakerIntegration::should_close_circuit(&backend, 2));
    }

    #[test]
    fn test_retry_delay() {
        // Transient errors get short delays
        assert_eq!(
            CircuitBreakerIntegration::retry_delay_ms(ErrorType::ConnectionTimeout),
            100
        );
        assert_eq!(
            CircuitBreakerIntegration::retry_delay_ms(ErrorType::RequestTimeout),
            100
        );

        // Permanent errors get longer delays
        assert_eq!(
            CircuitBreakerIntegration::retry_delay_ms(ErrorType::ConnectionRefused),
            5000
        );
        assert_eq!(
            CircuitBreakerIntegration::retry_delay_ms(ErrorType::HttpServerError),
            1000
        );
    }

    #[test]
    fn test_failure_threshold_percent() {
        assert_eq!(
            CircuitBreakerIntegration::failure_threshold_percent("critical"),
            10
        );
        assert_eq!(
            CircuitBreakerIntegration::failure_threshold_percent("important"),
            20
        );
        assert_eq!(
            CircuitBreakerIntegration::failure_threshold_percent("normal"),
            30
        );
        assert_eq!(
            CircuitBreakerIntegration::failure_threshold_percent("best_effort"),
            50
        );
    }

    #[test]
    fn test_open_duration_exponential_backoff() {
        // First open: 1s
        assert_eq!(CircuitBreakerIntegration::open_duration_ms(0), 1000);

        // Exponential backoff
        assert_eq!(CircuitBreakerIntegration::open_duration_ms(1), 2000);
        assert_eq!(CircuitBreakerIntegration::open_duration_ms(2), 4000);
        assert_eq!(CircuitBreakerIntegration::open_duration_ms(3), 8000);

        // Capped at 60s
        assert_eq!(CircuitBreakerIntegration::open_duration_ms(10), 60000);
    }
}
