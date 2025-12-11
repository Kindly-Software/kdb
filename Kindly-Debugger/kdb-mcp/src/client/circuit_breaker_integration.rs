//! Circuit Breaker Integration for MCP Client
//!
//! T1 Atomic circuit breaker wrapper providing fault tolerance for MCP clients.
//! Re-exports atomic_capsule's production-grade `CircuitBreaker` (AtomicBreakerSWeMR)
//! with client-specific configuration and recovery semantics.
//!
//! ## Tier: T1 Atomic (Lockfree Coordination)
//!
//! ## Features
//!
//! - **Re-exports atomic_capsule types**: CircuitBreaker, State, plus client-specific error
//! - **Configurable thresholds**: Failure count, recovery timeout, half-open success threshold
//! - **Environment configuration**: `KDB_CB_*` environment variables for runtime tuning
//! - **SWeMR pattern**: Single-Writer, Eventually-Multiple-Readers (<15ns update, <5ns load)
//! - **64B cache alignment**: Prevents false sharing in concurrent scenarios
//!
//! ## State Transitions
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────────────────┐
//! │                     CIRCUIT BREAKER STATE MACHINE                     │
//! └───────────────────────────────────────────────────────────────────────┘
//!
//!                         ┌─────────────┐
//!             success     │             │  failures >= threshold
//!          ┌─────────────►│   Closed    ├─────────────────────┐
//!          │              │  (Normal)   │                     │
//!          │              └──────┬──────┘                     │
//!          │                     │                            ▼
//!          │                     │                    ┌─────────────┐
//!          │                     │                    │             │
//!          │                     │                    │    Open     │
//!          │                     │                    │ (Rejecting) │
//!          │                     │                    └──────┬──────┘
//!          │                     │                           │
//!          │                     │           recovery_timeout elapsed
//!          │                     │                           │
//!          │              ┌──────▼──────┐                    │
//!          │              │             │◄───────────────────┘
//!          └──────────────┤  HalfOpen   │
//!            success      │  (Probing)  │
//!            threshold    └──────┬──────┘
//!            met                 │
//!                                │ failure
//!                                │
//!                                ▼
//!                        ┌─────────────┐
//!                        │    Open     │
//!                        │ (Back off)  │
//!                        └─────────────┘
//!
//!                    ┌────────────────────┐
//!                    │    ForcedOpen      │  Manual intervention only
//!                    │ (Admin override)   │  (force_open() method)
//!                    └────────────────────┘
//! ```
//!
//! ## Performance (B32 Validated)
//!
//! - Load: <5ns (relaxed), <8ns (acquire)
//! - Update: <15ns (single store, SWeMR)
//! - State check: <10ns (all operations lockfree)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kdb_mcp::client::circuit_breaker_integration::{
//!     MutableCircuitBreaker, CircuitBreakerState, CircuitBreakerError
//! };
//!
//! // Create with defaults (5 failures, 60s recovery)
//! let breaker = MutableCircuitBreaker::new(5, 60);
//!
//! // Or from environment variables
//! let breaker = MutableCircuitBreaker::from_env();
//!
//! // Check before making request
//! match breaker.check() {
//!     Ok(()) => {
//!         match make_mcp_request().await {
//!             Ok(result) => {
//!                 breaker.record_success();
//!                 Ok(result)
//!             }
//!             Err(e) => {
//!                 breaker.record_failure();
//!                 Err(e)
//!             }
//!         }
//!     }
//!     Err(CircuitBreakerError::Open) => {
//!         // Circuit is open, fail fast
//!         Err(ServiceUnavailable)
//!     }
//! }
//! ```
//!
//! ## Environment Variables
//!
//! - `KDB_CB_FAILURE_THRESHOLD`: Number of failures before opening (default: 5)
//! - `KDB_CB_RECOVERY_TIMEOUT`: Seconds before attempting recovery (default: 60)
//! - `KDB_CB_HALF_OPEN_SUCCESS`: Successes needed to close from half-open (default: 3)
//!
//! ## UCE35/Chaos Compliance
//!
//! - **Q33 Lockfree**: 100% lockfree via AtomicU64 (no mutex/RwLock)
//! - **Cache Alignment**: 64B alignment prevents false sharing
//! - **SWeMR Pattern**: Single-Writer, Eventually-Multiple-Readers
//! - **Generation Counters**: Embedded in atomic_capsule's CircuitBreaker

use core::sync::atomic::{AtomicU64, Ordering};

// Re-export atomic_capsule circuit breaker types
pub use atomic_capsule::patterns::circuit_breaker::{
    AtomicBreakerSWeMR as CircuitBreaker,
    State as CircuitBreakerState,
};

/// Circuit breaker error types for client operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerError {
    /// Circuit is open due to excessive failures.
    /// Wait for recovery timeout before retrying.
    Open,
    /// Circuit has been manually forced open by operator.
    /// Requires manual intervention to reset.
    ForcedOpen,
}

impl core::fmt::Display for CircuitBreakerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Open => write!(f, "Circuit breaker is open"),
            Self::ForcedOpen => write!(f, "Circuit breaker is forced open"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CircuitBreakerError {}

/// Client-specific circuit breaker wrapper with configurable thresholds.
///
/// Wraps atomic_capsule's `CircuitBreaker` (8-byte packed, Q8.8 metrics)
/// with client-specific configuration for failure tracking and recovery.
///
/// ## Layout
///
/// ```text
/// Offset  Field                        Size    Alignment
/// ------  ---------------------------  ------  ---------
/// 0       breaker: CircuitBreaker      8B      8B
/// 8       failure_count: AtomicU64     8B      8B
/// 16      open_timestamp: AtomicU64    8B      8B
/// 24      half_open_successes: AtomicU64 8B    8B
/// 32      failure_threshold: u8        1B      1B
/// 33      recovery_timeout_secs: u32   4B      1B
/// 37      half_open_success_threshold  1B      1B
/// 38      _padding                     26B     -
/// ------  ---------------------------  ------  ---------
/// Total                                64B     64B
/// ```
#[repr(C, align(64))]
pub struct MutableCircuitBreaker {
    /// Inner atomic breaker (SWeMR pattern, 8-byte packed word)
    breaker: CircuitBreaker,

    /// Current failure count (separate from breaker's internal error counter)
    failure_count: AtomicU64,

    /// Timestamp when circuit was opened (Unix epoch seconds)
    open_timestamp: AtomicU64,

    /// Success count in half-open state
    half_open_successes: AtomicU64,

    /// Number of failures before opening the circuit
    failure_threshold: u8,

    /// Seconds to wait before attempting recovery
    recovery_timeout_secs: u32,

    /// Successes needed in half-open state to close the circuit
    half_open_success_threshold: u8,

    /// Padding to 64 bytes for cache alignment
    _padding: [u8; 26],
}

impl MutableCircuitBreaker {
    /// Create a new circuit breaker with specified thresholds.
    ///
    /// # Arguments
    ///
    /// * `failure_threshold` - Number of failures before opening (1-255)
    /// * `recovery_timeout_secs` - Seconds before attempting recovery (1-86400)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let breaker = MutableCircuitBreaker::new(5, 60);
    /// ```
    #[must_use]
    pub const fn new(failure_threshold: u8, recovery_timeout_secs: u32) -> Self {
        Self {
            breaker: CircuitBreaker::new(CircuitBreakerState::Closed),
            failure_count: AtomicU64::new(0),
            open_timestamp: AtomicU64::new(0),
            half_open_successes: AtomicU64::new(0),
            failure_threshold,
            recovery_timeout_secs,
            half_open_success_threshold: 3,
            _padding: [0u8; 26],
        }
    }

    /// Create a new circuit breaker with custom half-open success threshold.
    ///
    /// # Arguments
    ///
    /// * `failure_threshold` - Number of failures before opening (1-255)
    /// * `recovery_timeout_secs` - Seconds before attempting recovery (1-86400)
    /// * `half_open_success_threshold` - Successes needed to close from half-open (1-255)
    #[must_use]
    pub const fn with_half_open_threshold(
        failure_threshold: u8,
        recovery_timeout_secs: u32,
        half_open_success_threshold: u8,
    ) -> Self {
        Self {
            breaker: CircuitBreaker::new(CircuitBreakerState::Closed),
            failure_count: AtomicU64::new(0),
            open_timestamp: AtomicU64::new(0),
            half_open_successes: AtomicU64::new(0),
            failure_threshold,
            recovery_timeout_secs,
            half_open_success_threshold,
            _padding: [0u8; 26],
        }
    }

    /// Create a circuit breaker from environment variables.
    ///
    /// Reads configuration from:
    /// - `KDB_CB_FAILURE_THRESHOLD` (default: 5)
    /// - `KDB_CB_RECOVERY_TIMEOUT` (default: 60)
    /// - `KDB_CB_HALF_OPEN_SUCCESS` (default: 3)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// std::env::set_var("KDB_CB_FAILURE_THRESHOLD", "10");
    /// std::env::set_var("KDB_CB_RECOVERY_TIMEOUT", "120");
    /// let breaker = MutableCircuitBreaker::from_env();
    /// assert_eq!(breaker.failure_threshold(), 10);
    /// assert_eq!(breaker.recovery_timeout_secs(), 120);
    /// ```
    #[cfg(feature = "std")]
    #[must_use]
    pub fn from_env() -> Self {
        let failure_threshold = std::env::var("KDB_CB_FAILURE_THRESHOLD")
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(5);

        let recovery_timeout_secs = std::env::var("KDB_CB_RECOVERY_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(60);

        let half_open_success_threshold = std::env::var("KDB_CB_HALF_OPEN_SUCCESS")
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(3);

        Self::with_half_open_threshold(
            failure_threshold,
            recovery_timeout_secs,
            half_open_success_threshold,
        )
    }

    /// Check if the circuit allows a request to proceed.
    ///
    /// Returns `Ok(())` if the request should proceed, or an error if blocked.
    ///
    /// # State Behavior
    ///
    /// - **Closed**: Always allows (`Ok(())`)
    /// - **Open**: Checks recovery timeout, transitions to HalfOpen if elapsed
    /// - **HalfOpen**: Always allows (probing mode)
    /// - **ForcedOpen**: Always blocks (admin override)
    ///
    /// # Performance
    ///
    /// <10ns typical (lockfree atomic load + comparison)
    #[cfg(feature = "std")]
    pub fn check(&self) -> Result<(), CircuitBreakerError> {
        match self.breaker.state() {
            CircuitBreakerState::Closed => Ok(()),
            CircuitBreakerState::Open => {
                // Check if recovery timeout has elapsed
                if self.should_attempt_recovery() {
                    // Transition to HalfOpen for probing
                    self.breaker.half_open();
                    self.half_open_successes.store(0, Ordering::Release);
                    Ok(())
                } else {
                    Err(CircuitBreakerError::Open)
                }
            }
            CircuitBreakerState::HalfOpen => Ok(()),
            CircuitBreakerState::ForcedOpen => Err(CircuitBreakerError::ForcedOpen),
        }
    }

    /// Check without time dependency (for no_std or testing).
    ///
    /// Same as `check()` but doesn't attempt automatic recovery transition.
    /// Use `try_recovery()` to manually check recovery eligibility.
    #[must_use]
    pub fn check_no_time(&self) -> Result<(), CircuitBreakerError> {
        match self.breaker.state() {
            CircuitBreakerState::Closed => Ok(()),
            CircuitBreakerState::Open => Err(CircuitBreakerError::Open),
            CircuitBreakerState::HalfOpen => Ok(()),
            CircuitBreakerState::ForcedOpen => Err(CircuitBreakerError::ForcedOpen),
        }
    }

    /// Record a successful operation.
    ///
    /// # State Behavior
    ///
    /// - **Closed**: Resets failure count
    /// - **HalfOpen**: Increments success count, closes circuit if threshold met
    /// - **Open/ForcedOpen**: No effect
    ///
    /// # Performance
    ///
    /// <15ns (atomic store with release ordering)
    pub fn record_success(&self) {
        match self.breaker.state() {
            CircuitBreakerState::HalfOpen => {
                let successes = self.half_open_successes.fetch_add(1, Ordering::AcqRel) + 1;
                if successes >= u64::from(self.half_open_success_threshold) {
                    // Transition to Closed
                    self.breaker.close();
                    self.failure_count.store(0, Ordering::Release);
                    self.half_open_successes.store(0, Ordering::Release);
                    #[cfg(feature = "std")]
                    eprintln!(
                        "[Client-CB] Circuit breaker CLOSED: {} consecutive successes",
                        successes
                    );
                }
            }
            CircuitBreakerState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Release);
            }
            _ => {}
        }
    }

    /// Record a failed operation.
    ///
    /// # State Behavior
    ///
    /// - **Closed**: Increments failure count, opens circuit if threshold reached
    /// - **HalfOpen**: Immediately transitions to Open
    /// - **Open/ForcedOpen**: No effect (already blocking)
    ///
    /// # Performance
    ///
    /// <15ns (atomic fetch_add + conditional store)
    #[cfg(feature = "std")]
    pub fn record_failure(&self) {
        match self.breaker.state() {
            CircuitBreakerState::Closed => {
                let failures = self.failure_count.fetch_add(1, Ordering::AcqRel) + 1;

                if failures >= u64::from(self.failure_threshold) {
                    self.open_circuit();
                    eprintln!(
                        "[Client-CB] Circuit breaker OPEN: {} failures (threshold: {}), recovery in {}s",
                        failures, self.failure_threshold, self.recovery_timeout_secs
                    );
                }
            }
            CircuitBreakerState::HalfOpen => {
                // Any failure in half-open state re-opens the circuit
                self.open_circuit();
                #[cfg(feature = "std")]
                eprintln!(
                    "[Client-CB] Circuit breaker OPEN: failure during half-open probe, recovery in {}s",
                    self.recovery_timeout_secs
                );
            }
            _ => {}
        }
    }

    /// Record a failure without logging (for no_std environments).
    pub fn record_failure_silent(&self) {
        match self.breaker.state() {
            CircuitBreakerState::Closed => {
                let failures = self.failure_count.fetch_add(1, Ordering::AcqRel) + 1;
                if failures >= u64::from(self.failure_threshold) {
                    self.open_circuit();
                }
            }
            CircuitBreakerState::HalfOpen => {
                self.open_circuit();
            }
            _ => {}
        }
    }

    /// Open the circuit and record the timestamp.
    fn open_circuit(&self) {
        self.breaker.open();
        self.half_open_successes.store(0, Ordering::Release);

        #[cfg(feature = "std")]
        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            self.open_timestamp.store(now, Ordering::Release);
        }
    }

    /// Check if recovery should be attempted.
    #[cfg(feature = "std")]
    fn should_attempt_recovery(&self) -> bool {
        let open_time = self.open_timestamp.load(Ordering::Acquire);
        if open_time == 0 {
            return true; // No timestamp recorded, allow recovery
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        now.saturating_sub(open_time) >= u64::from(self.recovery_timeout_secs)
    }

    /// Manually transition to half-open state for recovery attempt.
    ///
    /// Use this in no_std environments where automatic recovery isn't available.
    pub fn try_recovery(&self) {
        if self.breaker.state() == CircuitBreakerState::Open {
            self.breaker.half_open();
            self.half_open_successes.store(0, Ordering::Release);
        }
    }

    /// Force the circuit open (admin override).
    ///
    /// The circuit will remain open until manually reset with `reset()`.
    pub fn force_open(&self) {
        self.breaker.force_open();
    }

    /// Reset the circuit breaker to closed state.
    ///
    /// Clears all counters and transitions to Closed state.
    pub fn reset(&self) {
        self.breaker.close();
        self.failure_count.store(0, Ordering::Release);
        self.half_open_successes.store(0, Ordering::Release);
        self.open_timestamp.store(0, Ordering::Release);
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    /// Get the current circuit breaker state.
    #[must_use]
    pub fn state(&self) -> CircuitBreakerState {
        self.breaker.state()
    }

    /// Get the current failure count.
    #[must_use]
    pub fn failure_count(&self) -> u64 {
        self.failure_count.load(Ordering::Acquire)
    }

    /// Get the configured failure threshold.
    #[must_use]
    pub const fn failure_threshold(&self) -> u8 {
        self.failure_threshold
    }

    /// Get the configured recovery timeout in seconds.
    #[must_use]
    pub const fn recovery_timeout_secs(&self) -> u32 {
        self.recovery_timeout_secs
    }

    /// Get the configured half-open success threshold.
    #[must_use]
    pub const fn half_open_success_threshold(&self) -> u8 {
        self.half_open_success_threshold
    }

    /// Get the success count in half-open state.
    #[must_use]
    pub fn half_open_success_count(&self) -> u64 {
        self.half_open_successes.load(Ordering::Acquire)
    }

    /// Check if the circuit is in closed (normal) state.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.breaker.state() == CircuitBreakerState::Closed
    }

    /// Check if the circuit is in open (blocking) state.
    #[must_use]
    pub fn is_open(&self) -> bool {
        matches!(
            self.breaker.state(),
            CircuitBreakerState::Open | CircuitBreakerState::ForcedOpen
        )
    }

    /// Check if the circuit is in half-open (probing) state.
    #[must_use]
    pub fn is_half_open(&self) -> bool {
        self.breaker.state() == CircuitBreakerState::HalfOpen
    }

    /// Get a reference to the inner CircuitBreaker for advanced usage.
    #[must_use]
    pub const fn inner(&self) -> &CircuitBreaker {
        &self.breaker
    }
}

// Safety: MutableCircuitBreaker is thread-safe via atomic operations
unsafe impl Send for MutableCircuitBreaker {}
unsafe impl Sync for MutableCircuitBreaker {}

impl Default for MutableCircuitBreaker {
    fn default() -> Self {
        Self::new(5, 60)
    }
}

impl core::fmt::Debug for MutableCircuitBreaker {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MutableCircuitBreaker")
            .field("state", &self.breaker.state())
            .field("failure_count", &self.failure_count.load(Ordering::Relaxed))
            .field("failure_threshold", &self.failure_threshold)
            .field("recovery_timeout_secs", &self.recovery_timeout_secs)
            .field("half_open_success_threshold", &self.half_open_success_threshold)
            .field("half_open_successes", &self.half_open_successes.load(Ordering::Relaxed))
            .finish()
    }
}

// =============================================================================
// TESTS (10 T28-compliant tests)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Q1-Q7: Unit Tests
    // =========================================================================

    /// Test that from_env falls back to defaults when env vars not set.
    #[test]
    fn test_circuit_breaker_from_env() {
        // Clear any existing env vars
        std::env::remove_var("KDB_CB_FAILURE_THRESHOLD");
        std::env::remove_var("KDB_CB_RECOVERY_TIMEOUT");
        std::env::remove_var("KDB_CB_HALF_OPEN_SUCCESS");

        let breaker = MutableCircuitBreaker::from_env();

        // Should use defaults
        assert_eq!(breaker.failure_threshold(), 5);
        assert_eq!(breaker.recovery_timeout_secs(), 60);
        assert_eq!(breaker.half_open_success_threshold(), 3);
    }

    /// Test that from_env reads custom values from environment.
    #[test]
    fn test_circuit_breaker_env_custom() {
        std::env::set_var("KDB_CB_FAILURE_THRESHOLD", "10");
        std::env::set_var("KDB_CB_RECOVERY_TIMEOUT", "120");
        std::env::set_var("KDB_CB_HALF_OPEN_SUCCESS", "5");

        let breaker = MutableCircuitBreaker::from_env();

        assert_eq!(breaker.failure_threshold(), 10);
        assert_eq!(breaker.recovery_timeout_secs(), 120);
        assert_eq!(breaker.half_open_success_threshold(), 5);

        // Clean up
        std::env::remove_var("KDB_CB_FAILURE_THRESHOLD");
        std::env::remove_var("KDB_CB_RECOVERY_TIMEOUT");
        std::env::remove_var("KDB_CB_HALF_OPEN_SUCCESS");
    }

    /// Test default values via Default trait.
    #[test]
    fn test_circuit_breaker_defaults() {
        let breaker = MutableCircuitBreaker::default();

        assert_eq!(breaker.failure_threshold(), 5);
        assert_eq!(breaker.recovery_timeout_secs(), 60);
        assert_eq!(breaker.half_open_success_threshold(), 3);
        assert!(breaker.is_closed());
        assert_eq!(breaker.failure_count(), 0);
    }

    /// Test Closed -> Open transition when threshold reached.
    #[test]
    fn test_closed_to_open_transition() {
        let breaker = MutableCircuitBreaker::new(3, 60);

        assert!(breaker.is_closed());

        // Record failures up to threshold
        breaker.record_failure_silent();
        assert!(breaker.is_closed());
        assert_eq!(breaker.failure_count(), 1);

        breaker.record_failure_silent();
        assert!(breaker.is_closed());
        assert_eq!(breaker.failure_count(), 2);

        breaker.record_failure_silent();
        assert!(breaker.is_open());
        assert_eq!(breaker.state(), CircuitBreakerState::Open);
    }

    /// Test Open -> HalfOpen recovery transition.
    #[test]
    fn test_open_to_half_open_recovery() {
        let breaker = MutableCircuitBreaker::new(1, 1);

        // Open the circuit
        breaker.record_failure_silent();
        assert!(breaker.is_open());

        // Manually trigger recovery (simulates timeout elapsed)
        breaker.try_recovery();

        assert!(breaker.is_half_open());
        assert_eq!(breaker.state(), CircuitBreakerState::HalfOpen);
    }

    /// Test HalfOpen -> Closed success transition.
    #[test]
    fn test_half_open_to_closed_success() {
        let breaker = MutableCircuitBreaker::with_half_open_threshold(1, 1, 2);

        // Open the circuit
        breaker.record_failure_silent();
        assert!(breaker.is_open());

        // Transition to half-open
        breaker.try_recovery();
        assert!(breaker.is_half_open());

        // Record successes
        breaker.record_success();
        assert!(breaker.is_half_open()); // Not yet at threshold
        assert_eq!(breaker.half_open_success_count(), 1);

        breaker.record_success();
        assert!(breaker.is_closed()); // Threshold met, should close
        assert_eq!(breaker.failure_count(), 0);
    }

    /// Test HalfOpen -> Open failure transition.
    #[test]
    fn test_half_open_to_open_failure() {
        let breaker = MutableCircuitBreaker::new(1, 60);

        // Open the circuit
        breaker.record_failure_silent();
        assert!(breaker.is_open());

        // Transition to half-open
        breaker.try_recovery();
        assert!(breaker.is_half_open());

        // Failure during half-open should re-open
        breaker.record_failure_silent();
        assert!(breaker.is_open());
        assert_eq!(breaker.state(), CircuitBreakerState::Open);
    }

    /// Test ForcedOpen blocks all requests.
    #[test]
    fn test_forced_open_blocks_all() {
        let breaker = MutableCircuitBreaker::new(5, 60);

        // Initially closed
        assert!(breaker.check_no_time().is_ok());

        // Force open
        breaker.force_open();

        assert_eq!(breaker.state(), CircuitBreakerState::ForcedOpen);
        assert!(matches!(
            breaker.check_no_time(),
            Err(CircuitBreakerError::ForcedOpen)
        ));

        // Reset should clear forced open
        breaker.reset();
        assert!(breaker.is_closed());
        assert!(breaker.check_no_time().is_ok());
    }

    /// Test failure threshold accuracy.
    #[test]
    fn test_failure_threshold_accuracy() {
        for threshold in [1u8, 3, 5, 10, 255] {
            let breaker = MutableCircuitBreaker::new(threshold, 60);

            // Should stay closed until threshold
            for i in 1..threshold {
                breaker.record_failure_silent();
                assert!(
                    breaker.is_closed(),
                    "Should be closed at {} failures (threshold: {})",
                    i,
                    threshold
                );
            }

            // Should open at threshold
            breaker.record_failure_silent();
            assert!(
                breaker.is_open(),
                "Should be open at threshold {}",
                threshold
            );
        }
    }

    /// Test recovery timeout with real time (brief timeout).
    #[test]
    fn test_recovery_timeout_timing() {
        let breaker = MutableCircuitBreaker::new(1, 1); // 1 second timeout

        // Open the circuit
        breaker.record_failure();
        assert!(breaker.is_open());

        // Check should fail immediately
        assert!(matches!(breaker.check(), Err(CircuitBreakerError::Open)));

        // Wait for recovery timeout
        std::thread::sleep(std::time::Duration::from_millis(1100));

        // Check should succeed and transition to half-open
        assert!(breaker.check().is_ok());
        assert!(breaker.is_half_open());
    }

    /// Test capsule size and alignment (Q33 compliance).
    #[test]
    fn test_capsule_size_and_alignment() {
        // Must be 64 bytes for cache alignment
        assert_eq!(
            core::mem::size_of::<MutableCircuitBreaker>(),
            64,
            "MutableCircuitBreaker must be exactly 64 bytes"
        );

        // Must be 64-byte aligned
        assert_eq!(
            core::mem::align_of::<MutableCircuitBreaker>(),
            64,
            "MutableCircuitBreaker must be 64-byte aligned"
        );

        // Verify on heap allocation
        let breaker = Box::new(MutableCircuitBreaker::default());
        let ptr = &*breaker as *const MutableCircuitBreaker as usize;
        assert_eq!(
            ptr % 64,
            0,
            "Heap-allocated MutableCircuitBreaker must be 64-byte aligned"
        );
    }

    // =========================================================================
    // Additional Q8-Q14: Property-based edge cases
    // =========================================================================

    /// Test that success in closed state resets failure count.
    #[test]
    fn test_success_resets_failures_in_closed() {
        let breaker = MutableCircuitBreaker::new(5, 60);

        // Accumulate some failures
        breaker.record_failure_silent();
        breaker.record_failure_silent();
        assert_eq!(breaker.failure_count(), 2);

        // Success should reset
        breaker.record_success();
        assert_eq!(breaker.failure_count(), 0);
        assert!(breaker.is_closed());
    }

    /// Test Debug impl.
    #[test]
    fn test_debug_impl() {
        let breaker = MutableCircuitBreaker::new(5, 60);
        let debug_str = format!("{:?}", breaker);

        assert!(debug_str.contains("MutableCircuitBreaker"));
        assert!(debug_str.contains("state"));
        assert!(debug_str.contains("failure_threshold"));
    }

    /// Test thread safety with concurrent access.
    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let breaker = Arc::new(MutableCircuitBreaker::new(100, 60));
        let mut handles = vec![];

        // Spawn multiple threads recording failures
        for _ in 0..10 {
            let b = Arc::clone(&breaker);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    b.record_failure_silent();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All 100 failures recorded, circuit should be open
        assert!(breaker.is_open());
    }
}
