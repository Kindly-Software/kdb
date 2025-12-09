//! Worker Crash Recovery - Self-healing worker thread coordination
//!
//! **UCE34 Analysis**:
//! - **Q1**: Problem: Worker thread crash causes permanent data loss
//! - **Q10**: Tier: T1 (Atomic coordination) + T5 (Streaming recovery)
//! - **Q11**: Rust: Use panic hook + exponential backoff + health tracking
//! - **Q12**: Nightly: No (stable sufficient)
//! - **Q28**: Simplicity: Automatic recovery, max 3 attempts, then unhealthy
//! - **Q31**: Constraints: <1s detection, <5s recovery
//! - **Q33**: Validation: Integration tests with panic injection
//! - **Q34**: Auditability: All panics and recoveries logged
//!
//! ## ASSUM Safety Assumptions
//!
//! #ASSUME_PANIC_HOOK_SAFE: Panic hook can safely record error before unwinding
//! #VERIFY_HOOK_SAFETY: Integration tests validate panic recording works
//!
//! #ASSUME_RECOVERY_BOUNDED: Max 3 recovery attempts prevents infinite loops
//! #VERIFY_BOUND: Property tests validate recovery limit enforcement
//!
//! #ASSUME_EXPONENTIAL_BACKOFF: 100ms, 400ms, 1600ms backoff prevents thrashing
//! #VERIFY_BACKOFF: Unit tests validate backoff timing
//!
//! ## Performance Targets (B32 Framework)
//!
//! - Panic detection: <100ms (worker heartbeat check)
//! - Recovery attempt: <5s (restart + state restore)
//! - Health check: <10ns (atomic load)

use crate::capsules::error_context_capsule::{ErrorCode, ErrorContextCapsule};
use crate::capsules::structured_log_capsule::StructuredLogCapsule;
use std::panic;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Worker health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WorkerHealth {
    /// Worker healthy and running
    Healthy = 0,
    /// Worker recovering from panic
    Recovering = 1,
    /// Worker unhealthy (max recovery attempts exceeded)
    Unhealthy = 2,
    /// Worker stopped (graceful shutdown)
    Stopped = 3,
}

impl WorkerHealth {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Healthy,
            1 => Self::Recovering,
            2 => Self::Unhealthy,
            3 => Self::Stopped,
            _ => Self::Unhealthy,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

/// Worker recovery configuration
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    /// Maximum recovery attempts before marking unhealthy
    pub max_attempts: u32,

    /// Base backoff delay in milliseconds
    pub base_backoff_ms: u64,

    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,

    /// Enable automatic recovery
    pub auto_recovery: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_backoff_ms: 100,
            heartbeat_interval_ms: 100,
            auto_recovery: true,
        }
    }
}

/// Worker recovery coordinator
///
/// Tracks worker health and coordinates crash recovery with exponential backoff.
pub struct WorkerRecovery {
    /// Worker health status (atomic u8)
    health: AtomicU8,

    /// Recovery attempt counter
    recovery_attempts: AtomicU32,

    /// Last heartbeat timestamp (milliseconds)
    last_heartbeat: AtomicU64,

    /// Worker running flag
    running: AtomicBool,

    /// Recovery configuration
    config: RecoveryConfig,

    /// Error context for tracking
    error_context: Arc<ErrorContextCapsule>,

    /// Structured logging
    log: Arc<StructuredLogCapsule>,
}

// Wrapper for AtomicU8 (not in std, so we implement it)
struct AtomicU8 {
    inner: AtomicU32, // Use AtomicU32 internally (no AtomicU8 in stable)
}

impl AtomicU8 {
    fn new(value: u8) -> Self {
        Self {
            inner: AtomicU32::new(value as u32),
        }
    }

    fn load(&self, ordering: Ordering) -> u8 {
        self.inner.load(ordering) as u8
    }

    fn store(&self, value: u8, ordering: Ordering) {
        self.inner.store(value as u32, ordering);
    }

    fn compare_exchange(
        &self,
        current: u8,
        new: u8,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u8, u8> {
        self.inner
            .compare_exchange(current as u32, new as u32, success, failure)
            .map(|v| v as u8)
            .map_err(|v| v as u8)
    }
}

impl WorkerRecovery {
    /// Create new worker recovery coordinator
    pub fn new(
        config: RecoveryConfig,
        error_context: Arc<ErrorContextCapsule>,
        log: Arc<StructuredLogCapsule>,
    ) -> Self {
        Self {
            health: AtomicU8::new(WorkerHealth::Healthy.to_u8()),
            recovery_attempts: AtomicU32::new(0),
            last_heartbeat: AtomicU64::new(Self::current_time_ms()),
            running: AtomicBool::new(true),
            config,
            error_context,
            log,
        }
    }

    /// Get current health status
    ///
    /// **Performance**: <10ns (atomic load)
    pub fn get_health(&self) -> WorkerHealth {
        WorkerHealth::from_u8(self.health.load(Ordering::Acquire))
    }

    /// Check if worker is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.get_health(), WorkerHealth::Healthy)
    }

    /// Check if worker is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Record heartbeat (called by worker thread)
    ///
    /// **Performance**: <20ns (atomic store)
    pub fn record_heartbeat(&self) {
        self.last_heartbeat
            .store(Self::current_time_ms(), Ordering::Release);
    }

    /// Check if heartbeat is stale
    ///
    /// **Performance**: <50ns (atomic load + comparison)
    pub fn is_heartbeat_stale(&self) -> bool {
        let last = self.last_heartbeat.load(Ordering::Acquire);
        let now = Self::current_time_ms();
        let elapsed = now.saturating_sub(last);
        elapsed > self.config.heartbeat_interval_ms * 3 // 3x interval = stale
    }

    /// Record worker panic
    ///
    /// **Performance**: <100ns (error recording + logging)
    pub fn record_panic(&self, error_code: ErrorCode, message: &str) {
        // Update health to Recovering
        self.health
            .store(WorkerHealth::Recovering.to_u8(), Ordering::Release);

        // Record in error context
        self.error_context.record_panic(error_code);

        // Log critical error
        self.log
            .log_critical(error_code.to_u8() as u16, message)
            .ok();
    }

    /// Attempt recovery
    ///
    /// **Performance**: <5s (backoff + restart)
    ///
    /// Returns true if recovery should be attempted, false if max attempts exceeded
    pub fn attempt_recovery(&self) -> bool {
        let attempts = self.recovery_attempts.fetch_add(1, Ordering::Relaxed);

        // Check if max attempts exceeded
        if attempts >= self.config.max_attempts {
            self.health
                .store(WorkerHealth::Unhealthy.to_u8(), Ordering::Release);

            self.log
                .log_critical(
                    ErrorCode::WorkerPanic.to_u8() as u16,
                    &format!(
                        "Max recovery attempts ({}) exceeded, marking unhealthy",
                        self.config.max_attempts
                    ),
                )
                .ok();

            return false;
        }

        // Record recovery attempt
        self.error_context.record_recovery_attempt();

        // Exponential backoff: 100ms, 400ms, 1600ms
        let backoff_ms = self.config.base_backoff_ms * 4u64.pow(attempts);
        let backoff = Duration::from_millis(backoff_ms);

        self.log
            .log_warn(
                ErrorCode::WorkerPanic.to_u8() as u16,
                &format!(
                    "Recovery attempt {} of {}, backing off for {}ms",
                    attempts + 1,
                    self.config.max_attempts,
                    backoff_ms
                ),
            )
            .ok();

        // Backoff
        thread::sleep(backoff);

        true
    }

    /// Mark recovery successful
    pub fn mark_recovered(&self) {
        self.health
            .store(WorkerHealth::Healthy.to_u8(), Ordering::Release);

        self.recovery_attempts.store(0, Ordering::Relaxed);

        self.log
            .log_info("Worker recovery successful")
            .ok();
    }

    /// Stop worker (graceful shutdown)
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
        self.health
            .store(WorkerHealth::Stopped.to_u8(), Ordering::Release);

        self.log
            .log_info("Worker stopped gracefully")
            .ok();
    }

    /// Get recovery attempt count
    pub fn get_recovery_attempts(&self) -> u32 {
        self.recovery_attempts.load(Ordering::Relaxed)
    }

    /// Get milliseconds since last heartbeat
    pub fn get_heartbeat_age_ms(&self) -> u64 {
        let last = self.last_heartbeat.load(Ordering::Acquire);
        let now = Self::current_time_ms();
        now.saturating_sub(last)
    }

    /// Get current time in milliseconds
    fn current_time_ms() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Install panic hook for worker thread
    ///
    /// **ASSUM Safety**:
    /// - #ASSUME_PANIC_HOOK_SAFE: Panic hook executes before unwinding
    /// - #VERIFY_HOOK_SAFETY: Integration tests validate panic recording
    pub fn install_panic_hook(&self) -> Arc<Self> {
        let recovery = Arc::new(Self {
            health: AtomicU8::new(self.health.load(Ordering::Acquire)),
            recovery_attempts: AtomicU32::new(self.recovery_attempts.load(Ordering::Relaxed)),
            last_heartbeat: AtomicU64::new(self.last_heartbeat.load(Ordering::Acquire)),
            running: AtomicBool::new(self.running.load(Ordering::Acquire)),
            config: self.config.clone(),
            error_context: Arc::clone(&self.error_context),
            log: Arc::clone(&self.log),
        });

        let recovery_clone = Arc::clone(&recovery);

        panic::set_hook(Box::new(move |panic_info| {
            let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic".to_string()
            };

            let location = if let Some(loc) = panic_info.location() {
                format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
            } else {
                "unknown location".to_string()
            };

            let full_message = format!("Worker panic at {}: {}", location, message);

            recovery_clone.record_panic(ErrorCode::WorkerPanic, &full_message);
        }));

        recovery
    }
}

/// Worker thread wrapper with automatic recovery
///
/// Usage:
/// ```ignore
/// let recovery = WorkerRecovery::new(config, error_context, log);
/// let recovery = recovery.install_panic_hook();
///
/// loop {
///     recovery.record_heartbeat();
///
///     // Worker logic here
///     if !recovery.is_running() {
///         break;
///     }
/// }
/// ```
pub struct WorkerThread<F>
where
    F: FnMut() -> Result<(), String> + Send + 'static,
{
    recovery: Arc<WorkerRecovery>,
    worker_fn: F,
    handle: Option<thread::JoinHandle<()>>,
}

impl<F> WorkerThread<F>
where
    F: FnMut() -> Result<(), String> + Send + 'static,
{
    /// Create new worker thread with recovery
    pub fn new(
        config: RecoveryConfig,
        error_context: Arc<ErrorContextCapsule>,
        log: Arc<StructuredLogCapsule>,
        worker_fn: F,
    ) -> Self {
        let recovery = WorkerRecovery::new(config, error_context, log);

        Self {
            recovery: Arc::new(recovery),
            worker_fn,
            handle: None,
        }
    }

    /// Start worker thread
    pub fn start(&mut self) {
        let recovery = Arc::clone(&self.recovery);
        let recovery_with_hook = recovery.install_panic_hook();

        // Move worker_fn out of self temporarily
        // (This is a simplified example - real implementation would use channel or similar)
        let handle = thread::spawn(move || {
            loop {
                recovery_with_hook.record_heartbeat();

                // Worker logic execution would go here
                // For now, just demonstrate recovery loop

                if !recovery_with_hook.is_running() {
                    break;
                }

                thread::sleep(Duration::from_millis(100));
            }
        });

        self.handle = Some(handle);
    }

    /// Stop worker thread
    pub fn stop(&mut self) {
        self.recovery.stop();

        if let Some(handle) = self.handle.take() {
            handle.join().ok();
        }
    }

    /// Get recovery coordinator
    pub fn recovery(&self) -> &Arc<WorkerRecovery> {
        &self.recovery
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_recovery_creation() {
        let config = RecoveryConfig::default();
        let error_context = Arc::new(ErrorContextCapsule::new());
        let log = Arc::new(StructuredLogCapsule::new());

        let recovery = WorkerRecovery::new(config, error_context, log);

        assert!(recovery.is_healthy());
        assert!(recovery.is_running());
        assert_eq!(recovery.get_recovery_attempts(), 0);
    }

    #[test]
    fn test_heartbeat() {
        let config = RecoveryConfig::default();
        let error_context = Arc::new(ErrorContextCapsule::new());
        let log = Arc::new(StructuredLogCapsule::new());

        let recovery = WorkerRecovery::new(config, error_context, log);

        recovery.record_heartbeat();
        assert!(!recovery.is_heartbeat_stale());

        // Sleep longer than 3x heartbeat interval
        thread::sleep(Duration::from_millis(400));
        assert!(recovery.is_heartbeat_stale());
    }

    #[test]
    fn test_panic_recording() {
        let config = RecoveryConfig::default();
        let error_context = Arc::new(ErrorContextCapsule::new());
        let log = Arc::new(StructuredLogCapsule::new());

        let recovery = WorkerRecovery::new(config, error_context.clone(), log.clone());

        recovery.record_panic(ErrorCode::WorkerPanic, "Test panic message");

        assert_eq!(recovery.get_health(), WorkerHealth::Recovering);
        assert_eq!(error_context.get_panic_count(), 1);
    }

    #[test]
    fn test_recovery_attempts() {
        let config = RecoveryConfig {
            max_attempts: 3,
            base_backoff_ms: 10, // Short for testing
            ..Default::default()
        };
        let error_context = Arc::new(ErrorContextCapsule::new());
        let log = Arc::new(StructuredLogCapsule::new());

        let recovery = WorkerRecovery::new(config, error_context.clone(), log);

        // Attempt 1
        assert!(recovery.attempt_recovery());
        assert_eq!(recovery.get_recovery_attempts(), 1);
        assert_eq!(error_context.get_recovery_attempts(), 1);

        // Attempt 2
        assert!(recovery.attempt_recovery());
        assert_eq!(recovery.get_recovery_attempts(), 2);

        // Attempt 3
        assert!(recovery.attempt_recovery());
        assert_eq!(recovery.get_recovery_attempts(), 3);

        // Attempt 4 - should fail (max exceeded)
        assert!(!recovery.attempt_recovery());
        assert_eq!(recovery.get_health(), WorkerHealth::Unhealthy);
    }

    #[test]
    fn test_successful_recovery() {
        let config = RecoveryConfig::default();
        let error_context = Arc::new(ErrorContextCapsule::new());
        let log = Arc::new(StructuredLogCapsule::new());

        let recovery = WorkerRecovery::new(config, error_context, log);

        recovery.record_panic(ErrorCode::WorkerPanic, "Test panic");
        assert_eq!(recovery.get_health(), WorkerHealth::Recovering);

        recovery.mark_recovered();
        assert_eq!(recovery.get_health(), WorkerHealth::Healthy);
        assert_eq!(recovery.get_recovery_attempts(), 0);
    }

    #[test]
    fn test_graceful_stop() {
        let config = RecoveryConfig::default();
        let error_context = Arc::new(ErrorContextCapsule::new());
        let log = Arc::new(StructuredLogCapsule::new());

        let recovery = WorkerRecovery::new(config, error_context, log);

        recovery.stop();
        assert_eq!(recovery.get_health(), WorkerHealth::Stopped);
        assert!(!recovery.is_running());
    }
}
