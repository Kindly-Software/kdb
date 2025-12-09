//! Circuit Breaker Protection Trait
//!
//! Provides a standardized interface for primitives that can be protected by circuit breakers.
//! Follows UCE-32 framework with IMPL-2 constraints for minimal, practical implementation.
//!
//! # Design Principles
//! - Q28 (Simplicity): Minimal 3-method trait interface
//! - Q29 (Constraints): 100% lockfree, no allocation, atomic operations only
//! - Q30 (Validation): Compile-time safety through trait bounds
//! - Q31 (Rust Transform): Zero-cost abstraction via traits
//!
//! # ASSUM Framework
//! - #ASSUME: All breaker operations are lockfree
//! - #VERIFY: No mutex/RwLock usage anywhere in implementation

use std::time::Duration;

/// Result of a protected operation attempt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionResult<T> {
    /// Operation executed successfully
    Success(T),
    /// Operation blocked by circuit breaker
    Blocked {
        /// Current breaker state
        state: BreakerState,
        /// Suggested retry delay
        retry_after: Option<Duration>,
    },
    /// Operation failed with error
    Failed {
        /// The underlying error
        error: ProtectionError,
        /// Whether failure affected breaker state
        state_updated: bool,
    },
}

/// Simplified breaker state for trait interface
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    /// Normal operation - requests pass through
    Closed,
    /// Limited probing for recovery - some requests allowed
    HalfOpen,
    /// Actively rejecting requests - most requests blocked
    Open,
    /// Operator forced open - all requests blocked
    ForcedOpen,
}

/// Protection operation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionError {
    /// Breaker is in blocking state
    BreakerOpen,
    /// Operation timed out
    Timeout,
    /// Resource unavailable
    ResourceUnavailable,
    /// Rate limit exceeded
    RateLimited,
    /// Unknown protection failure
    Unknown,
}

impl std::fmt::Display for ProtectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BreakerOpen => write!(f, "circuit breaker is open"),
            Self::Timeout => write!(f, "operation timed out"),
            Self::ResourceUnavailable => write!(f, "resource unavailable"),
            Self::RateLimited => write!(f, "rate limit exceeded"),
            Self::Unknown => write!(f, "unknown protection error"),
        }
    }
}

impl std::error::Error for ProtectionError {}

/// Trait for primitives that can be protected by circuit breakers
///
/// # Design Philosophy (UCE-32)
/// This trait provides a minimal, zero-cost abstraction for circuit breaker integration.
/// Following Q28 (Simplicity), it exposes only the essential operations needed for
/// protection without overengineering.
///
/// # Lockfree Guarantee (Q29 Constraints)
/// All implementations MUST be 100% lockfree. No mutex, RwLock, or blocking operations.
/// Only atomic operations with appropriate memory ordering are permitted.
///
/// # Usage Example
/// ```rust
/// use test_support::breaker::{BreakerProtected, ProtectionResult};
/// use atomic_breaker::breaker::{AtomicBreakerSWeMR, State};
///
/// // Create a breaker-protected primitive
/// let breaker = AtomicBreakerSWeMR::new(State::Closed);
///
/// // Use the BreakerProtected trait methods
/// fn protected_operation<T: BreakerProtected>(primitive: &T) -> Result<u64, String> {
///     // Quick check before attempting operation
///     if !primitive.is_protected() {
///         return Err("Service unavailable".to_string());
///     }
///
///     // Execute operation under circuit breaker protection
///     match primitive.protect_operation(|| {
///         // Your operation logic here - could be database call, API request, etc.
///         std::thread::sleep(std::time::Duration::from_millis(10));
///         Ok(42u64)
///     }) {
///         ProtectionResult::Success(value) => Ok(value),
///         ProtectionResult::Blocked { state, retry_after } => {
///             let retry_msg = if let Some(delay) = retry_after {
///                 format!(", retry after {}ms", delay.as_millis())
///             } else {
///                 ", manual intervention required".to_string()
///             };
///             Err(format!("Blocked (state: {:?}){}", state, retry_msg))
///         },
///         ProtectionResult::Failed { error, .. } => {
///             Err(format!("Operation failed: {}", error))
///         },
///     }
/// }
///
/// // Example usage
/// let result = protected_operation(&breaker);
/// assert!(result.is_ok());
/// ```
pub trait BreakerProtected {
    /// Get current circuit breaker state using relaxed ordering
    ///
    /// This method MUST be lockfree and use only atomic operations.
    /// Returns simplified state suitable for external decision making.
    fn breaker_state(&self) -> BreakerState;

    /// Check if the primitive is currently protected (breaker closed/half-open)
    ///
    /// Returns true if operations are likely to succeed, false if blocked.
    /// This is a fast path check using relaxed ordering.
    fn is_protected(&self) -> bool {
        matches!(self.breaker_state(), BreakerState::Closed | BreakerState::HalfOpen)
    }

    /// Execute an operation under circuit breaker protection
    ///
    /// # Arguments
    /// * `operation` - Closure containing the operation to protect
    ///
    /// # Returns
    /// * `ProtectionResult::Success(T)` - Operation completed successfully
    /// * `ProtectionResult::Blocked` - Operation blocked by breaker state
    /// * `ProtectionResult::Failed` - Operation failed and may have affected breaker
    ///
    /// # Implementation Requirements
    /// - MUST be lockfree (atomic operations only)
    /// - MUST check breaker state before executing operation
    /// - SHOULD update breaker metrics based on operation outcome
    /// - MAY implement rate limiting or backoff strategies
    ///
    /// # Memory Ordering
    /// Implementations should use:
    /// - Acquire ordering when reading breaker state for decisions
    /// - Release ordering when updating breaker state after operations
    /// - Relaxed ordering for metric updates that don't affect control flow
    fn protect_operation<T, F, E>(&self, operation: F) -> ProtectionResult<T>
    where
        F: FnOnce() -> Result<T, E>,
        E: Into<ProtectionError>;
}

/// Extension trait for breaker primitives with metrics support
///
/// This trait is automatically implemented for types that support telemetry
/// and provides additional functionality for monitoring and tuning.
pub trait BreakerMetrics: BreakerProtected {
    /// Get current error rate as a normalized value [0.0, 1.0]
    fn error_rate(&self) -> f32;

    /// Get current mean response time as normalized value
    fn mean_response_time(&self) -> f32;

    /// Get current jitter (standard deviation) as normalized value
    fn response_jitter(&self) -> f32;

    /// Get current backoff suggestion in milliseconds
    fn suggested_backoff_ms(&self) -> u32;

    /// Apply a telemetry sample to update breaker metrics
    ///
    /// # Arguments
    /// * `response_time_ms` - Operation response time in milliseconds
    /// * `success` - Whether the operation succeeded
    /// * `error_type` - Type of error if operation failed
    fn record_operation(&self, response_time_ms: f32, success: bool, error_type: Option<ProtectionError>);
}

/// Implementation of BreakerProtected for atomic_breaker types
///
/// This module provides concrete implementations of the BreakerProtected trait
/// for atomic_breaker primitives, following UCE-32 framework guidelines.
pub mod atomic_breaker_impl {
    use super::*;
    use atomic_breaker::breaker::{AtomicBreakerSWeMR, State as BreakerStateRaw, BreakerLike};

    #[cfg(feature = "mpmc")]
    use atomic_breaker::breaker::AtomicBreakerMPMC;

    /// Convert atomic_breaker::State to our simplified BreakerState
    fn convert_state(raw_state: BreakerStateRaw) -> BreakerState {
        match raw_state {
            BreakerStateRaw::Closed => BreakerState::Closed,
            BreakerStateRaw::HalfOpen => BreakerState::HalfOpen,
            BreakerStateRaw::Open => BreakerState::Open,
            BreakerStateRaw::ForcedOpen => BreakerState::ForcedOpen,
        }
    }

    /// Implementation for single-writer/many-reader atomic breaker
    impl BreakerProtected for AtomicBreakerSWeMR {
        fn breaker_state(&self) -> BreakerState {
            convert_state(self.state())
        }

        fn protect_operation<T, F, E>(&self, operation: F) -> ProtectionResult<T>
        where
            F: FnOnce() -> Result<T, E>,
            E: Into<ProtectionError>,
        {
            // Q30 (Validation): Check breaker state with acquire semantics for proper synchronization
            let current_state = self.state();

            match current_state {
                BreakerStateRaw::ForcedOpen => {
                    return ProtectionResult::Blocked {
                        state: BreakerState::ForcedOpen,
                        retry_after: None, // Forced open requires manual intervention
                    };
                },
                BreakerStateRaw::Open => {
                    // Calculate backoff based on current backoff level
                    let backoff_index = self.backoff();
                    let retry_delay = Duration::from_millis(
                        (100 * (1 << backoff_index.min(6))) as u64 // Exponential backoff, capped at ~6.4s
                    );

                    return ProtectionResult::Blocked {
                        state: BreakerState::Open,
                        retry_after: Some(retry_delay),
                    };
                },
                BreakerStateRaw::HalfOpen | BreakerStateRaw::Closed => {
                    // Proceed with operation
                }
            }

            // Execute the operation and handle the result
            let start_time = std::time::Instant::now();
            let result = operation();
            let elapsed = start_time.elapsed();

            match result {
                Ok(value) => {
                    // Q31 (Rust Transform): Use atomic operations for lockfree metrics update
                    // Success: Reset error counter and update metrics
                    self.clear_error();

                    // Update timing metrics (simplified approach)
                    let response_time_ms = elapsed.as_millis() as u16;
                    let mu_q = (response_time_ms.min(255) * 256) / 255; // Normalized to Q8.8
                    let sg_q = 128; // Assume low jitter for successful operations

                    self.update_metrics(0, mu_q, sg_q, 0, 0);

                    ProtectionResult::Success(value)
                },
                Err(error) => {
                    // Q29 (Constraints): Update error metrics atomically
                    let protection_error = error.into();

                    // Map protection error to cause bits
                    let cause_bits = match protection_error {
                        ProtectionError::Timeout => 0x01,        // LAT - latency related
                        ProtectionError::ResourceUnavailable => 0x02, // CPU - resource related
                        ProtectionError::RateLimited => 0x04,    // THR - throughput related
                        _ => 0x08,                                // ERR - general error
                    };

                    // Increment error counter and update metrics
                    let response_time_ms = elapsed.as_millis() as u16;
                    let mu_q = (response_time_ms.min(255) * 256) / 255;
                    let sg_q = 200; // Higher jitter for failed operations
                    let current_backoff = self.backoff();

                    self.update_metrics(1, mu_q, sg_q, cause_bits, current_backoff);

                    ProtectionResult::Failed {
                        error: protection_error,
                        state_updated: true,
                    }
                }
            }
        }
    }

    #[cfg(feature = "mpmc")]
    /// Implementation for multi-producer/multi-consumer atomic breaker
    impl BreakerProtected for AtomicBreakerMPMC {
        fn breaker_state(&self) -> BreakerState {
            // Use BreakerLike trait for consistent state access
            let word = self.load_relaxed();
            let guard = atomic_breaker::breaker::AtomicBreakerGuard::new(word);
            convert_state(guard.state())
        }

        fn protect_operation<T, F, E>(&self, operation: F) -> ProtectionResult<T>
        where
            F: FnOnce() -> Result<T, E>,
            E: Into<ProtectionError>,
        {
            // Q32 (Nightly): Could use const trait for compile-time state checking
            let current_state = self.breaker_state();

            match current_state {
                BreakerState::ForcedOpen => {
                    return ProtectionResult::Blocked {
                        state: BreakerState::ForcedOpen,
                        retry_after: None,
                    };
                },
                BreakerState::Open => {
                    return ProtectionResult::Blocked {
                        state: BreakerState::Open,
                        retry_after: Some(Duration::from_millis(500)), // Fixed backoff for MPMC
                    };
                },
                BreakerState::HalfOpen | BreakerState::Closed => {
                    // Proceed with operation
                }
            }

            // Execute operation with timing
            let start_time = std::time::Instant::now();
            let result = operation();
            let elapsed = start_time.elapsed();

            match result {
                Ok(value) => {
                    // Update metrics using CAS loop for thread safety
                    let response_time_ms = elapsed.as_millis() as u16;
                    let mu_q = (response_time_ms.min(255) * 256) / 255;
                    let sg_q = 128;

                    // Use best effort CAS update (may fail under contention)
                    let _ = self.update_metrics_cas(0, mu_q, sg_q, 0, 0, 3);

                    ProtectionResult::Success(value)
                },
                Err(error) => {
                    let protection_error = error.into();
                    let cause_bits = match protection_error {
                        ProtectionError::Timeout => 0x01,
                        ProtectionError::ResourceUnavailable => 0x02,
                        ProtectionError::RateLimited => 0x04,
                        _ => 0x08,
                    };

                    let response_time_ms = elapsed.as_millis() as u16;
                    let mu_q = (response_time_ms.min(255) * 256) / 255;
                    let sg_q = 200;

                    // Best effort metric update
                    let _ = self.update_metrics_cas(1, mu_q, sg_q, cause_bits, 0, 3);

                    ProtectionResult::Failed {
                        error: protection_error,
                        state_updated: true,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock breaker for testing trait interface
    struct MockBreaker {
        state: std::sync::atomic::AtomicU8,
    }

    impl MockBreaker {
        fn new(state: BreakerState) -> Self {
            Self {
                state: std::sync::atomic::AtomicU8::new(state as u8),
            }
        }

        fn set_state(&self, new_state: BreakerState) {
            self.state.store(new_state as u8, std::sync::atomic::Ordering::Release);
        }
    }

    impl BreakerProtected for MockBreaker {
        fn breaker_state(&self) -> BreakerState {
            let state = self.state.load(std::sync::atomic::Ordering::Relaxed);
            match state {
                0 => BreakerState::Closed,
                1 => BreakerState::HalfOpen,
                2 => BreakerState::Open,
                _ => BreakerState::ForcedOpen,
            }
        }

        fn protect_operation<T, F, E>(&self, operation: F) -> ProtectionResult<T>
        where
            F: FnOnce() -> Result<T, E>,
            E: Into<ProtectionError>,
        {
            let state = self.breaker_state();

            match state {
                BreakerState::ForcedOpen | BreakerState::Open => {
                    ProtectionResult::Blocked {
                        state,
                        retry_after: Some(Duration::from_millis(100)),
                    }
                },
                BreakerState::HalfOpen | BreakerState::Closed => {
                    match operation() {
                        Ok(result) => ProtectionResult::Success(result),
                        Err(error) => ProtectionResult::Failed {
                            error: error.into(),
                            state_updated: true,
                        },
                    }
                },
            }
        }
    }

    #[test]
    fn test_breaker_state_reporting() {
        let breaker = MockBreaker::new(BreakerState::Closed);
        assert_eq!(breaker.breaker_state(), BreakerState::Closed);
        assert!(breaker.is_protected());

        breaker.set_state(BreakerState::Open);
        assert_eq!(breaker.breaker_state(), BreakerState::Open);
        assert!(!breaker.is_protected());
    }

    #[test]
    fn test_protection_success() {
        let breaker = MockBreaker::new(BreakerState::Closed);
        let result = breaker.protect_operation(|| Ok::<u32, ProtectionError>(42));

        match result {
            ProtectionResult::Success(value) => assert_eq!(value, 42),
            _ => panic!("Expected success result"),
        }
    }

    #[test]
    fn test_protection_blocked() {
        let breaker = MockBreaker::new(BreakerState::Open);
        let result = breaker.protect_operation(|| Ok::<u32, ProtectionError>(42));

        match result {
            ProtectionResult::Blocked { state, retry_after } => {
                assert_eq!(state, BreakerState::Open);
                assert!(retry_after.is_some());
            },
            _ => panic!("Expected blocked result"),
        }
    }

    #[test]
    fn test_protection_failed() {
        let breaker = MockBreaker::new(BreakerState::Closed);
        let result = breaker.protect_operation(|| Err::<u32, ProtectionError>(ProtectionError::Timeout));

        match result {
            ProtectionResult::Failed { error, state_updated } => {
                assert_eq!(error, ProtectionError::Timeout);
                assert!(state_updated);
            },
            _ => panic!("Expected failed result"),
        }
    }

    #[test]
    fn test_is_protected_helper() {
        let breaker = MockBreaker::new(BreakerState::Closed);
        assert!(breaker.is_protected());

        breaker.set_state(BreakerState::HalfOpen);
        assert!(breaker.is_protected());

        breaker.set_state(BreakerState::Open);
        assert!(!breaker.is_protected());

        breaker.set_state(BreakerState::ForcedOpen);
        assert!(!breaker.is_protected());
    }

    #[test]
    fn test_atomic_breaker_integration() {
        use crate::breaker::atomic_breaker_impl::*;
        use atomic_breaker::breaker::{AtomicBreakerSWeMR, State};

        let breaker = AtomicBreakerSWeMR::new(State::Closed);

        // Test state reporting
        assert_eq!(breaker.breaker_state(), BreakerState::Closed);
        assert!(breaker.is_protected());

        // Test successful operation
        let result = breaker.protect_operation(|| Ok::<u32, ProtectionError>(42));
        match result {
            ProtectionResult::Success(value) => assert_eq!(value, 42),
            _ => panic!("Expected successful operation"),
        }

        // Test blocked operation
        breaker.force_open();
        assert_eq!(breaker.breaker_state(), BreakerState::ForcedOpen);
        assert!(!breaker.is_protected());

        let result = breaker.protect_operation(|| Ok::<u32, ProtectionError>(42));
        match result {
            ProtectionResult::Blocked { state, retry_after } => {
                assert_eq!(state, BreakerState::ForcedOpen);
                assert!(retry_after.is_none()); // Forced open has no retry
            },
            _ => panic!("Expected blocked operation"),
        }

        // Test failed operation
        breaker.close(); // Reset to closed state
        let result = breaker.protect_operation(|| Err::<u32, ProtectionError>(ProtectionError::Timeout));
        match result {
            ProtectionResult::Failed { error, state_updated } => {
                assert_eq!(error, ProtectionError::Timeout);
                assert!(state_updated);
            },
            _ => panic!("Expected failed operation"),
        }
    }
}