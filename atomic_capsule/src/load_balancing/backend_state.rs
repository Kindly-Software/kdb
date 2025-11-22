//! Per-backend health state tracking (64B, cache-aligned)

use core::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use super::check_types::HealthStatus;

/// Per-backend health state (64B cache-aligned)
///
/// This structure tracks the health of a single backend, including:
/// - Current health status and thresholds
/// - Timing information for health checks
/// - Statistics about check results
/// - Latency measurements
///
/// All fields are atomic for lock-free concurrent updates.
///
/// # Layout
///
/// ```ignore
/// [backend_id: u32]
/// [health_status: u8][consecutive_successes: u8][consecutive_failures: u8][flags: u8]
/// [last_check_time_ns: u64]
/// [last_success_time_ns: u64]
/// [last_failure_time_ns: u64]
/// [check_count: u32][success_count: u32][failure_count: u32][timeout_count: u32]
/// [last_latency_ns: u64]
/// [padding: 8 bytes]
/// = 64 bytes total
/// ```
#[repr(C, align(64))]
pub struct BackendHealthState {
    /// Backend identifier
    backend_id: AtomicU32,

    /// Current health status (HealthStatus as u8)
    health_status: AtomicU8,

    /// Consecutive successful checks
    consecutive_successes: AtomicU8,

    /// Consecutive failed checks
    consecutive_failures: AtomicU8,

    /// Flags: bit 0 = draining, bit 1 = manual_override, bits 2-7 = reserved
    flags: AtomicU8,

    /// Timestamp (ns) of last health check
    last_check_time_ns: AtomicU64,

    /// Timestamp (ns) of last successful check
    last_success_time_ns: AtomicU64,

    /// Timestamp (ns) of last failed check
    last_failure_time_ns: AtomicU64,

    /// Total number of health checks performed
    check_count: AtomicU32,

    /// Number of successful checks
    success_count: AtomicU32,

    /// Number of failed checks
    failure_count: AtomicU32,

    /// Number of timed-out checks
    timeout_count: AtomicU32,

    /// Last measured latency (ns)
    last_latency_ns: AtomicU64,

    /// Padding to reach 64 bytes
    _padding: [u8; 8],
}

impl BackendHealthState {
    /// Create a new backend health state
    pub fn new(backend_id: u32) -> Self {
        BackendHealthState {
            backend_id: AtomicU32::new(backend_id),
            health_status: AtomicU8::new(HealthStatus::Unknown as u8),
            consecutive_successes: AtomicU8::new(0),
            consecutive_failures: AtomicU8::new(0),
            flags: AtomicU8::new(0),
            last_check_time_ns: AtomicU64::new(0),
            last_success_time_ns: AtomicU64::new(0),
            last_failure_time_ns: AtomicU64::new(0),
            check_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            failure_count: AtomicU32::new(0),
            timeout_count: AtomicU32::new(0),
            last_latency_ns: AtomicU64::new(0),
            _padding: [0; 8],
        }
    }

    /// Get backend ID
    pub fn backend_id(&self) -> u32 {
        self.backend_id.load(Ordering::Acquire)
    }

    /// Get current health status
    pub fn health_status(&self) -> HealthStatus {
        let status = self.health_status.load(Ordering::Acquire);
        HealthStatus::from_u8(status).unwrap_or(HealthStatus::Unknown)
    }

    /// Set health status atomically
    pub fn set_health_status(&self, status: HealthStatus) -> HealthStatus {
        let old = self.health_status.swap(status as u8, Ordering::Release);
        HealthStatus::from_u8(old).unwrap_or(HealthStatus::Unknown)
    }

    /// Get consecutive successes
    pub fn consecutive_successes(&self) -> u8 {
        self.consecutive_successes.load(Ordering::Acquire)
    }

    /// Increment consecutive successes, return new count
    pub fn increment_successes(&self) -> u8 {
        let new_val = self
            .consecutive_successes
            .fetch_add(1, Ordering::Release)
            + 1;
        new_val
    }

    /// Reset consecutive successes to 0
    pub fn reset_successes(&self) {
        self.consecutive_successes.store(0, Ordering::Release);
    }

    /// Get consecutive failures
    pub fn consecutive_failures(&self) -> u8 {
        self.consecutive_failures.load(Ordering::Acquire)
    }

    /// Increment consecutive failures, return new count
    pub fn increment_failures(&self) -> u8 {
        let new_val = self
            .consecutive_failures
            .fetch_add(1, Ordering::Release)
            + 1;
        new_val
    }

    /// Reset consecutive failures to 0
    pub fn reset_failures(&self) {
        self.consecutive_failures.store(0, Ordering::Release);
    }

    /// Check if backend is draining
    pub fn is_draining(&self) -> bool {
        (self.flags.load(Ordering::Acquire) & 0x01) != 0
    }

    /// Set draining flag
    pub fn set_draining(&self, draining: bool) {
        let mut flags = self.flags.load(Ordering::Acquire);
        if draining {
            flags |= 0x01;
        } else {
            flags &= !0x01;
        }
        self.flags.store(flags, Ordering::Release);
    }

    /// Check if backend has manual override
    pub fn has_manual_override(&self) -> bool {
        (self.flags.load(Ordering::Acquire) & 0x02) != 0
    }

    /// Set manual override flag
    pub fn set_manual_override(&self, override_flag: bool) {
        let mut flags = self.flags.load(Ordering::Acquire);
        if override_flag {
            flags |= 0x02;
        } else {
            flags &= !0x02;
        }
        self.flags.store(flags, Ordering::Release);
    }

    /// Get last check timestamp
    pub fn last_check_time_ns(&self) -> u64 {
        self.last_check_time_ns.load(Ordering::Acquire)
    }

    /// Set last check timestamp
    pub fn set_last_check_time_ns(&self, ts_ns: u64) {
        self.last_check_time_ns.store(ts_ns, Ordering::Release);
    }

    /// Get last success timestamp
    pub fn last_success_time_ns(&self) -> u64 {
        self.last_success_time_ns.load(Ordering::Acquire)
    }

    /// Set last success timestamp
    pub fn set_last_success_time_ns(&self, ts_ns: u64) {
        self.last_success_time_ns.store(ts_ns, Ordering::Release);
    }

    /// Get last failure timestamp
    pub fn last_failure_time_ns(&self) -> u64 {
        self.last_failure_time_ns.load(Ordering::Acquire)
    }

    /// Set last failure timestamp
    pub fn set_last_failure_time_ns(&self, ts_ns: u64) {
        self.last_failure_time_ns.store(ts_ns, Ordering::Release);
    }

    /// Get total check count
    pub fn check_count(&self) -> u32 {
        self.check_count.load(Ordering::Acquire)
    }

    /// Increment check count
    pub fn increment_check_count(&self) {
        self.check_count.fetch_add(1, Ordering::Release);
    }

    /// Get successful check count
    pub fn success_count(&self) -> u32 {
        self.success_count.load(Ordering::Acquire)
    }

    /// Increment success count
    pub fn increment_success_count(&self) {
        self.success_count.fetch_add(1, Ordering::Release);
    }

    /// Get failed check count
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::Acquire)
    }

    /// Increment failure count
    pub fn increment_failure_count(&self) {
        self.failure_count.fetch_add(1, Ordering::Release);
    }

    /// Get timeout check count
    pub fn timeout_count(&self) -> u32 {
        self.timeout_count.load(Ordering::Acquire)
    }

    /// Increment timeout count
    pub fn increment_timeout_count(&self) {
        self.timeout_count.fetch_add(1, Ordering::Release);
    }

    /// Get last latency measurement (ns)
    pub fn last_latency_ns(&self) -> u64 {
        self.last_latency_ns.load(Ordering::Acquire)
    }

    /// Set last latency measurement
    pub fn set_last_latency_ns(&self, latency_ns: u64) {
        self.last_latency_ns.store(latency_ns, Ordering::Release);
    }

    /// Calculate success rate as percentage (0-100)
    pub fn success_rate_percent(&self) -> u32 {
        let total = self.check_count.load(Ordering::Acquire);
        if total == 0 {
            return 0;
        }
        let success = self.success_count.load(Ordering::Acquire);
        ((success as u64 * 100) / total as u64) as u32
    }

    /// Verify struct size is exactly 64 bytes
    #[allow(dead_code)]
    const fn verify_size() {
        // Compile-time size check via transmute
        let _ = core::mem::transmute::<BackendHealthState, [u8; 64]>;
    }
}

// Compile-time size verification
const _: () = {
    const fn assert_size() {
        let _ = core::mem::transmute::<BackendHealthState, [u8; 64]>;
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_state_size() {
        assert_eq!(core::mem::size_of::<BackendHealthState>(), 64);
    }

    #[test]
    fn test_backend_state_alignment() {
        assert_eq!(core::mem::align_of::<BackendHealthState>(), 64);
    }

    #[test]
    fn test_backend_creation() {
        let state = BackendHealthState::new(42);
        assert_eq!(state.backend_id(), 42);
        assert_eq!(state.health_status(), HealthStatus::Unknown);
        assert_eq!(state.consecutive_successes(), 0);
        assert_eq!(state.consecutive_failures(), 0);
        assert_eq!(state.check_count(), 0);
    }

    #[test]
    fn test_consecutive_tracking() {
        let state = BackendHealthState::new(1);

        // Increment successes
        assert_eq!(state.increment_successes(), 1);
        assert_eq!(state.increment_successes(), 2);
        assert_eq!(state.consecutive_successes(), 2);

        // Reset and check failures
        state.reset_successes();
        assert_eq!(state.consecutive_successes(), 0);

        assert_eq!(state.increment_failures(), 1);
        assert_eq!(state.increment_failures(), 2);
        assert_eq!(state.consecutive_failures(), 2);
    }

    #[test]
    fn test_flags() {
        let state = BackendHealthState::new(1);

        // Test draining flag
        assert!(!state.is_draining());
        state.set_draining(true);
        assert!(state.is_draining());
        state.set_draining(false);
        assert!(!state.is_draining());

        // Test manual override flag
        assert!(!state.has_manual_override());
        state.set_manual_override(true);
        assert!(state.has_manual_override());
        state.set_manual_override(false);
        assert!(!state.has_manual_override());
    }

    #[test]
    fn test_statistics() {
        let state = BackendHealthState::new(1);

        state.increment_check_count();
        state.increment_success_count();
        state.increment_check_count();
        state.increment_failure_count();
        state.increment_check_count();
        state.increment_timeout_count();

        assert_eq!(state.check_count(), 3);
        assert_eq!(state.success_count(), 1);
        assert_eq!(state.failure_count(), 1);
        assert_eq!(state.timeout_count(), 1);
        assert_eq!(state.success_rate_percent(), 33);
    }

    #[test]
    fn test_latency_tracking() {
        let state = BackendHealthState::new(1);

        assert_eq!(state.last_latency_ns(), 0);
        state.set_last_latency_ns(1500);
        assert_eq!(state.last_latency_ns(), 1500);
    }
}
