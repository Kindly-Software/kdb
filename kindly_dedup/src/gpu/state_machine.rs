//! GPU State Machine Capsule - T1 Atomic Tier
//!
//! Robust state machine for GPU lifecycle management with 6 states and 10 transitions.
//! Enables graceful degradation and recovery from GPU errors.
//!
//! # Architecture (T1 Atomic)
//!
//! 64-byte cache-aligned capsule with AtomicU64 bit-packed state:
//! - Bits 0-7: GpuState enum (6 states)
//! - Bits 8-31: Error code (24 bits)
//! - Bits 32-63: Generation counter (32 bits, Q34 audit trail)
//!
//! # State Diagram
//!
//! ```text
//! Uninitialized -> Initializing -> Ready -> Processing
//!                       |            |          |
//!                       v            v          v
//!                    Failed <-- Recovering <----+
//! ```
//!
//! # Framework Compliance
//!
//! - UCE34: T1 Atomic tier (lockfree state coordination)
//! - Chaos: 100% lockfree, 64B cache-aligned, generation counter
//! - ASSUM: All state transition assumptions documented
//! - B32: <50ns state transition target
//! - T28: Comprehensive test coverage

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// GPU lifecycle states (6 states)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuState {
    /// Initial state - GPU not yet probed
    Uninitialized = 0,
    /// GPU context being created
    Initializing = 1,
    /// GPU available for compute
    Ready = 2,
    /// GPU currently executing
    Processing = 3,
    /// Error recovery in progress
    Recovering = 4,
    /// Unrecoverable failure (requires restart)
    Failed = 5,
}

impl GpuState {
    /// Convert from u8 to GpuState
    #[inline]
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => GpuState::Uninitialized,
            1 => GpuState::Initializing,
            2 => GpuState::Ready,
            3 => GpuState::Processing,
            4 => GpuState::Recovering,
            5 => GpuState::Failed,
            _ => GpuState::Failed, // Invalid state -> Failed
        }
    }

    /// Human-readable state name
    pub fn as_str(&self) -> &'static str {
        match self {
            GpuState::Uninitialized => "Uninitialized",
            GpuState::Initializing => "Initializing",
            GpuState::Ready => "Ready",
            GpuState::Processing => "Processing",
            GpuState::Recovering => "Recovering",
            GpuState::Failed => "Failed",
        }
    }
}

/// State machine errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuStateMachineError {
    /// Invalid state transition attempted
    InvalidTransition {
        from: GpuState,
        to: GpuState,
        reason: &'static str,
    },
    /// Concurrent modification detected
    ConcurrentModification,
    /// Recovery attempts exceeded
    RecoveryExhausted { attempts: u32 },
}

impl std::fmt::Display for GpuStateMachineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTransition { from, to, reason } => {
                write!(
                    f,
                    "Invalid GPU state transition from {} to {}: {}",
                    from.as_str(),
                    to.as_str(),
                    reason
                )
            }
            Self::ConcurrentModification => {
                write!(f, "Concurrent state modification detected")
            }
            Self::RecoveryExhausted { attempts } => {
                write!(f, "GPU recovery exhausted after {} attempts", attempts)
            }
        }
    }
}

impl std::error::Error for GpuStateMachineError {}

/// Atomic snapshot of state machine
#[derive(Debug, Clone, Copy)]
pub struct GpuStateSnapshot {
    pub state: GpuState,
    pub error_code: u32,
    pub generation: u32,
}

/// GPU State Machine Capsule - T1 Atomic Tier
///
/// 64-byte cache-aligned state machine for GPU lifecycle management.
/// Uses AtomicU64 bit-packing for lockfree state + generation counter.
///
/// # Layout (64 bytes)
///
/// - Bytes 0-7: state_and_gen (AtomicU64) - packed state, error, generation
/// - Bytes 8-15: last_transition_ns (AtomicU64) - timestamp
/// - Bytes 16-23: recovery_config (AtomicU64) - attempts, max, delay
/// - Bytes 24-31: statistics (AtomicU64) - counters
/// - Bytes 32-63: _padding for cache line alignment
///
/// # ASSUM Safety
///
/// - `#ASSUME_STATE_ATOMIC`: AtomicU64 provides lockfree state updates
/// - `#VERIFY_STATE_ATOMIC`: CAS operations ensure consistency
/// - `#ASSUME_GEN_MONOTONIC`: Generation counter never wraps (2^32 transitions)
/// - `#VERIFY_GEN_MONOTONIC`: Increment on every transition
#[repr(C, align(64))]
pub struct GpuStateMachineCapsule {
    /// State + error code + generation counter (bit-packed)
    /// Bits 0-7: state, Bits 8-31: error_code, Bits 32-63: generation
    state_and_gen: AtomicU64,

    /// Timestamp of last state transition (nanoseconds since epoch)
    last_transition_ns: AtomicU64,

    /// Recovery configuration (packed)
    /// Bits 0-31: attempts, Bits 32-47: max_attempts, Bits 48-63: delay_ms
    recovery_config: AtomicU64,

    /// Statistics counters (packed)
    /// Bits 0-15: init_count, Bits 16-31: error_count, Bits 32-47: recovery_count, Bits 48-63: compute_count
    statistics: AtomicU64,

    /// Padding to 64-byte cache line
    _padding: [u8; 32],
}

// Bit-packing constants
const STATE_MASK: u64 = 0xFF;
const ERROR_MASK: u64 = 0xFFFFFF00;
const ERROR_SHIFT: u64 = 8;
const GEN_SHIFT: u64 = 32;

// Recovery config constants
const RECOVERY_ATTEMPTS_MASK: u64 = 0xFFFFFFFF;
const RECOVERY_MAX_SHIFT: u64 = 32;
const RECOVERY_MAX_MASK: u64 = 0xFFFF;

// Statistics constants
const STAT_INIT_MASK: u64 = 0xFFFF;
const STAT_ERROR_SHIFT: u64 = 16;
const STAT_RECOVERY_SHIFT: u64 = 32;
const STAT_COMPUTE_SHIFT: u64 = 48;

impl GpuStateMachineCapsule {
    /// Create new state machine in Uninitialized state
    pub const fn new() -> Self {
        Self {
            state_and_gen: AtomicU64::new(0), // Uninitialized, gen=0
            last_transition_ns: AtomicU64::new(0),
            recovery_config: AtomicU64::new(3 << RECOVERY_MAX_SHIFT), // max_attempts=3
            statistics: AtomicU64::new(0),
            _padding: [0; 32],
        }
    }

    /// Pack state, error code, and generation into u64
    #[inline]
    fn pack_state(state: GpuState, error_code: u32, generation: u32) -> u64 {
        (state as u64) | ((error_code as u64) << ERROR_SHIFT) | ((generation as u64) << GEN_SHIFT)
    }

    /// Unpack state from u64
    #[inline]
    fn unpack_state(packed: u64) -> GpuStateSnapshot {
        GpuStateSnapshot {
            state: GpuState::from_u8((packed & STATE_MASK) as u8),
            error_code: ((packed & ERROR_MASK) >> ERROR_SHIFT) as u32,
            generation: (packed >> GEN_SHIFT) as u32,
        }
    }

    /// Get current timestamp in nanoseconds
    #[inline]
    fn now_ns() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::ZERO)
            .as_nanos() as u64
    }

    /// Attempt state transition with CAS
    fn try_transition(
        &self,
        from: GpuState,
        to: GpuState,
        error_code: u32,
    ) -> Result<(), GpuStateMachineError> {
        let current = self.state_and_gen.load(Ordering::Acquire);
        let snapshot = Self::unpack_state(current);

        if snapshot.state != from {
            return Err(GpuStateMachineError::InvalidTransition {
                from: snapshot.state,
                to,
                reason: "current state does not match expected",
            });
        }

        let new_gen = snapshot.generation.wrapping_add(1);
        let new_packed = Self::pack_state(to, error_code, new_gen);

        match self.state_and_gen.compare_exchange(
            current,
            new_packed,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                self.last_transition_ns.store(Self::now_ns(), Ordering::Release);
                Ok(())
            }
            Err(_) => Err(GpuStateMachineError::ConcurrentModification),
        }
    }

    /// Increment a statistics counter
    fn increment_stat(&self, shift: u64) {
        self.statistics.fetch_add(1 << shift, Ordering::Relaxed);
    }

    // ========== Transition Methods (10 transitions) ==========

    /// Transition: Uninitialized -> Initializing
    pub fn init(&self) -> Result<(), GpuStateMachineError> {
        self.try_transition(GpuState::Uninitialized, GpuState::Initializing, 0)?;
        self.increment_stat(0); // init_count
        Ok(())
    }

    /// Transition: Initializing -> Ready
    pub fn init_complete(&self) -> Result<(), GpuStateMachineError> {
        self.try_transition(GpuState::Initializing, GpuState::Ready, 0)
    }

    /// Transition: Initializing -> Failed
    pub fn init_failed(&self, error_code: u32) -> Result<(), GpuStateMachineError> {
        self.try_transition(GpuState::Initializing, GpuState::Failed, error_code)?;
        self.increment_stat(STAT_ERROR_SHIFT);
        Ok(())
    }

    /// Transition: Ready -> Processing
    pub fn begin_compute(&self) -> Result<(), GpuStateMachineError> {
        self.try_transition(GpuState::Ready, GpuState::Processing, 0)?;
        self.increment_stat(STAT_COMPUTE_SHIFT);
        Ok(())
    }

    /// Transition: Processing -> Ready
    pub fn compute_complete(&self) -> Result<(), GpuStateMachineError> {
        self.try_transition(GpuState::Processing, GpuState::Ready, 0)
    }

    /// Transition: Processing -> Recovering
    pub fn compute_error(&self, error_code: u32) -> Result<(), GpuStateMachineError> {
        self.try_transition(GpuState::Processing, GpuState::Recovering, error_code)?;
        self.increment_stat(STAT_ERROR_SHIFT);

        // Increment recovery attempts
        self.recovery_config.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Transition: Recovering -> Ready
    pub fn recovery_success(&self) -> Result<(), GpuStateMachineError> {
        self.try_transition(GpuState::Recovering, GpuState::Ready, 0)?;
        self.increment_stat(STAT_RECOVERY_SHIFT);

        // Reset recovery attempts
        let config = self.recovery_config.load(Ordering::Acquire);
        let max_and_delay = config & !RECOVERY_ATTEMPTS_MASK;
        self.recovery_config.store(max_and_delay, Ordering::Release);
        Ok(())
    }

    /// Transition: Recovering -> Failed
    pub fn recovery_failed(&self, error_code: u32) -> Result<(), GpuStateMachineError> {
        self.try_transition(GpuState::Recovering, GpuState::Failed, error_code)?;
        self.increment_stat(STAT_ERROR_SHIFT);
        Ok(())
    }

    /// Transition: Ready -> Failed
    pub fn critical_error(&self, error_code: u32) -> Result<(), GpuStateMachineError> {
        self.try_transition(GpuState::Ready, GpuState::Failed, error_code)?;
        self.increment_stat(STAT_ERROR_SHIFT);
        Ok(())
    }

    /// Transition: Any -> Uninitialized (always succeeds)
    pub fn reset(&self) {
        let current = self.state_and_gen.load(Ordering::Acquire);
        let snapshot = Self::unpack_state(current);
        let new_gen = snapshot.generation.wrapping_add(1);
        let new_packed = Self::pack_state(GpuState::Uninitialized, 0, new_gen);
        self.state_and_gen.store(new_packed, Ordering::Release);
        self.last_transition_ns.store(Self::now_ns(), Ordering::Release);

        // Reset recovery attempts
        let config = self.recovery_config.load(Ordering::Acquire);
        let max_and_delay = config & !RECOVERY_ATTEMPTS_MASK;
        self.recovery_config.store(max_and_delay, Ordering::Release);
    }

    // ========== Query Methods ==========

    /// Get current state
    #[inline]
    pub fn state(&self) -> GpuState {
        let packed = self.state_and_gen.load(Ordering::Acquire);
        GpuState::from_u8((packed & STATE_MASK) as u8)
    }

    /// Get generation counter (for Q34 audit trail)
    #[inline]
    pub fn generation(&self) -> u32 {
        let packed = self.state_and_gen.load(Ordering::Acquire);
        (packed >> GEN_SHIFT) as u32
    }

    /// Get last error code
    #[inline]
    pub fn error_code(&self) -> u32 {
        let packed = self.state_and_gen.load(Ordering::Acquire);
        ((packed & ERROR_MASK) >> ERROR_SHIFT) as u32
    }

    /// Get atomic snapshot of state machine
    #[inline]
    pub fn snapshot(&self) -> GpuStateSnapshot {
        let packed = self.state_and_gen.load(Ordering::Acquire);
        Self::unpack_state(packed)
    }

    /// Check if GPU is ready for compute
    #[inline]
    pub fn is_ready(&self) -> bool {
        self.state() == GpuState::Ready
    }

    /// Check if GPU has failed
    #[inline]
    pub fn is_failed(&self) -> bool {
        self.state() == GpuState::Failed
    }

    /// Check if GPU can accept compute requests
    #[inline]
    pub fn can_compute(&self) -> bool {
        self.state() == GpuState::Ready
    }

    // ========== Recovery Configuration ==========

    /// Set maximum recovery attempts before transitioning to Failed
    pub fn set_max_recovery_attempts(&self, max: u16) {
        let current = self.recovery_config.load(Ordering::Acquire);
        let attempts = current & RECOVERY_ATTEMPTS_MASK;
        let new_config = attempts | ((max as u64) << RECOVERY_MAX_SHIFT);
        self.recovery_config.store(new_config, Ordering::Release);
    }

    /// Get current recovery attempt count
    pub fn recovery_attempts(&self) -> u32 {
        let config = self.recovery_config.load(Ordering::Acquire);
        (config & RECOVERY_ATTEMPTS_MASK) as u32
    }

    /// Check if recovery should be attempted
    pub fn should_attempt_recovery(&self) -> bool {
        let config = self.recovery_config.load(Ordering::Acquire);
        let attempts = (config & RECOVERY_ATTEMPTS_MASK) as u32;
        let max = ((config >> RECOVERY_MAX_SHIFT) & RECOVERY_MAX_MASK) as u32;
        attempts < max
    }

    // ========== Statistics ==========

    /// Get initialization count
    pub fn init_count(&self) -> u16 {
        let stats = self.statistics.load(Ordering::Relaxed);
        (stats & STAT_INIT_MASK) as u16
    }

    /// Get error count
    pub fn error_count(&self) -> u16 {
        let stats = self.statistics.load(Ordering::Relaxed);
        ((stats >> STAT_ERROR_SHIFT) & 0xFFFF) as u16
    }

    /// Get recovery count
    pub fn recovery_count(&self) -> u16 {
        let stats = self.statistics.load(Ordering::Relaxed);
        ((stats >> STAT_RECOVERY_SHIFT) & 0xFFFF) as u16
    }

    /// Get compute count
    pub fn compute_count(&self) -> u16 {
        let stats = self.statistics.load(Ordering::Relaxed);
        ((stats >> STAT_COMPUTE_SHIFT) & 0xFFFF) as u16
    }
}

impl Default for GpuStateMachineCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Verify 64-byte size at compile time
const _: () = assert!(std::mem::size_of::<GpuStateMachineCapsule>() == 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let sm = GpuStateMachineCapsule::new();
        assert_eq!(sm.state(), GpuState::Uninitialized);
        assert_eq!(sm.generation(), 0);
        assert_eq!(sm.error_code(), 0);
    }

    #[test]
    fn test_init_transition() {
        let sm = GpuStateMachineCapsule::new();
        assert!(sm.init().is_ok());
        assert_eq!(sm.state(), GpuState::Initializing);
        assert_eq!(sm.generation(), 1);
    }

    #[test]
    fn test_full_lifecycle() {
        let sm = GpuStateMachineCapsule::new();

        // Init
        assert!(sm.init().is_ok());
        assert_eq!(sm.state(), GpuState::Initializing);

        // Init complete
        assert!(sm.init_complete().is_ok());
        assert_eq!(sm.state(), GpuState::Ready);

        // Begin compute
        assert!(sm.begin_compute().is_ok());
        assert_eq!(sm.state(), GpuState::Processing);

        // Compute complete
        assert!(sm.compute_complete().is_ok());
        assert_eq!(sm.state(), GpuState::Ready);
    }

    #[test]
    fn test_recovery_cycle() {
        let sm = GpuStateMachineCapsule::new();

        // Setup: get to Processing
        sm.init().unwrap();
        sm.init_complete().unwrap();
        sm.begin_compute().unwrap();

        // Trigger error
        assert!(sm.compute_error(42).is_ok());
        assert_eq!(sm.state(), GpuState::Recovering);
        assert_eq!(sm.error_code(), 42);

        // Recovery success
        assert!(sm.recovery_success().is_ok());
        assert_eq!(sm.state(), GpuState::Ready);
    }

    #[test]
    fn test_invalid_transition() {
        let sm = GpuStateMachineCapsule::new();

        // Can't go directly to Ready from Uninitialized
        let result = sm.init_complete();
        assert!(result.is_err());
        assert!(matches!(result, Err(GpuStateMachineError::InvalidTransition { .. })));
    }

    #[test]
    fn test_reset_from_any_state() {
        let sm = GpuStateMachineCapsule::new();

        // Get to Failed state
        sm.init().unwrap();
        sm.init_failed(100).unwrap();
        assert_eq!(sm.state(), GpuState::Failed);

        // Reset should work
        sm.reset();
        assert_eq!(sm.state(), GpuState::Uninitialized);
    }

    #[test]
    fn test_statistics() {
        let sm = GpuStateMachineCapsule::new();

        sm.init().unwrap();
        assert_eq!(sm.init_count(), 1);

        sm.init_complete().unwrap();
        sm.begin_compute().unwrap();
        assert_eq!(sm.compute_count(), 1);

        sm.compute_error(1).unwrap();
        assert_eq!(sm.error_count(), 1);

        sm.recovery_success().unwrap();
        assert_eq!(sm.recovery_count(), 1);
    }

    #[test]
    fn test_recovery_attempts() {
        let sm = GpuStateMachineCapsule::new();
        sm.set_max_recovery_attempts(3);

        sm.init().unwrap();
        sm.init_complete().unwrap();

        // Test single recovery cycle
        sm.begin_compute().unwrap();
        sm.compute_error(1).unwrap();
        assert_eq!(sm.recovery_attempts(), 1);
        assert!(sm.should_attempt_recovery());
        sm.recovery_success().unwrap();

        // After recovery success, attempts should reset to 0
        assert_eq!(sm.recovery_attempts(), 0);

        // Test multiple failures without recovery (should accumulate)
        sm.begin_compute().unwrap();
        sm.compute_error(2).unwrap();
        assert_eq!(sm.recovery_attempts(), 1);
        assert!(sm.should_attempt_recovery());
        // Don't recover - stay in Recovering state by resetting to Ready manually
        // Actually we need to transition back to Ready to continue the cycle
        sm.recovery_success().unwrap();

        // Verify max attempts behavior
        sm.set_max_recovery_attempts(1);
        sm.begin_compute().unwrap();
        sm.compute_error(3).unwrap();
        assert_eq!(sm.recovery_attempts(), 1);
        assert!(!sm.should_attempt_recovery()); // 1 >= 1, should not attempt
    }

    #[test]
    fn test_snapshot() {
        let sm = GpuStateMachineCapsule::new();
        sm.init().unwrap();
        sm.init_complete().unwrap();

        let snap = sm.snapshot();
        assert_eq!(snap.state, GpuState::Ready);
        assert_eq!(snap.generation, 2);
        assert_eq!(snap.error_code, 0);
    }

    #[test]
    fn test_cache_alignment() {
        assert_eq!(std::mem::size_of::<GpuStateMachineCapsule>(), 64);
        assert_eq!(std::mem::align_of::<GpuStateMachineCapsule>(), 64);
    }

    #[test]
    fn test_state_from_u8() {
        assert_eq!(GpuState::from_u8(0), GpuState::Uninitialized);
        assert_eq!(GpuState::from_u8(1), GpuState::Initializing);
        assert_eq!(GpuState::from_u8(2), GpuState::Ready);
        assert_eq!(GpuState::from_u8(3), GpuState::Processing);
        assert_eq!(GpuState::from_u8(4), GpuState::Recovering);
        assert_eq!(GpuState::from_u8(5), GpuState::Failed);
        assert_eq!(GpuState::from_u8(255), GpuState::Failed); // Invalid -> Failed
    }

    #[test]
    fn test_generation_increments() {
        let sm = GpuStateMachineCapsule::new();
        assert_eq!(sm.generation(), 0);

        sm.init().unwrap();
        assert_eq!(sm.generation(), 1);

        sm.init_complete().unwrap();
        assert_eq!(sm.generation(), 2);

        sm.reset();
        assert_eq!(sm.generation(), 3);
    }
}
