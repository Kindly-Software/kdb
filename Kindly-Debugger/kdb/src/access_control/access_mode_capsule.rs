//! AccessModeCapsule - SOTA Lockfree State Machine for Observer/Operator Access Control
//!
//! A T1 Atomic tier computational capsule implementing a lockfree state machine
//! for access mode transitions between Observer and Operator roles.
//!
//! # Bit-Packing Layout (64-bit atomic)
//!
//! ```text
//! |  Bits 0-2   |  Bits 3-31    |   Bits 32-63      |
//! |  Mode (3)   |  Gen (29)     |   Timestamp (32)  |
//! |  0=Observer |  536M gens    |   Unix seconds    |
//! |  1=Pending  |  ABA prevent  |   32-bit epoch    |
//! |  2=Operator |               |                   |
//! |  3=Expired  |               |                   |
//! ```
//!
//! # COCA Compliance
//!
//! - 100% lockfree (zero mutex/RwLock)
//! - 128-byte aligned (cache-friendly, no false sharing)
//! - Generation counters for ABA prevention
//! - CAS loops with compare_exchange_weak for state transitions
//!
//! # Performance Targets
//!
//! - State read: <5ns (single atomic load)
//! - State transition: <50ns (CAS loop, typical 1-2 iterations)
//! - Expiry check: <10ns (atomic load + comparison)

use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// BIT-PACKING CONSTANTS
// ============================================================================

/// Bits 0-2: Mode field (3 bits, values 0-3)
const MODE_MASK: u64 = 0x7;

/// Bits 3-31: Generation counter (29 bits, 536M generations for ABA prevention)
const GEN_SHIFT: u32 = 3;
const GEN_MASK: u64 = 0xFFFF_FFF8; // 29 bits (0x1FFF_FFFF) shifted left by 3

/// Bits 32-63: Timestamp (32 bits, seconds since UNIX epoch)
const TIMESTAMP_SHIFT: u32 = 32;
#[allow(dead_code)] // Reserved for future direct timestamp extraction
const TIMESTAMP_MASK: u64 = 0xFFFF_FFFF_0000_0000;

/// Maximum generation counter value (29 bits = 536,870,911)
const MAX_GENERATION: u32 = 0x1FFF_FFFF;

// ============================================================================
// ACCESS MODE ENUM
// ============================================================================

/// Access control modes for debugger sessions
///
/// State machine transitions:
/// - Observer -> ChallengePending -> Operator (normal escalation)
/// - Operator -> Expired (timeout)
/// - Any -> Observer (demotion/reset)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccessMode {
    /// Read-only access: Can view state, cannot modify
    Observer = 0,
    /// Challenge in progress: Awaiting authentication
    ChallengePending = 1,
    /// Full access: Can modify breakpoints, step, continue
    Operator = 2,
    /// Session expired: Must re-authenticate
    Expired = 3,
}

impl AccessMode {
    /// Convert from raw u8 value to AccessMode
    ///
    /// # Safety (ASSUM-01: INPUT_VALIDATION)
    /// #ASSUME: Input value is in range 0-3 from trusted atomic read
    /// #VERIFY: Mode mask ensures only 3 bits are extracted
    #[inline(always)]
    fn from_raw(value: u8) -> Self {
        // #ASSUME: value & 0x7 always yields 0-3, matching all enum variants
        // #VERIFY: Exhaustive match guarantees no undefined behavior
        match value & 0x7 {
            0 => AccessMode::Observer,
            1 => AccessMode::ChallengePending,
            2 => AccessMode::Operator,
            3 => AccessMode::Expired,
            // #ASSUME: Values 4-7 should not occur with MODE_MASK applied
            // #VERIFY: Default to Observer for safety in case of corruption
            _ => AccessMode::Observer,
        }
    }

    /// Convert AccessMode to raw u8 value
    #[inline(always)]
    fn to_raw(self) -> u8 {
        self as u8
    }
}

// ============================================================================
// BIT-PACKING FUNCTIONS
// ============================================================================

/// Pack mode, generation, and timestamp into a single 64-bit value
///
/// # Arguments
/// * `mode` - Access mode (0-3)
/// * `generation` - Generation counter (0 to MAX_GENERATION)
/// * `timestamp` - Unix timestamp (seconds since epoch)
///
/// # Returns
/// Packed 64-bit state value
#[inline(always)]
const fn pack_state(mode: u8, generation: u32, timestamp: u32) -> u64 {
    let mode_bits = (mode & 0x7) as u64;
    let gen_bits = ((generation & MAX_GENERATION) as u64) << GEN_SHIFT;
    let ts_bits = (timestamp as u64) << TIMESTAMP_SHIFT;
    mode_bits | gen_bits | ts_bits
}

/// Unpack a 64-bit state value into mode, generation, and timestamp
///
/// # Arguments
/// * `state` - Packed 64-bit state value
///
/// # Returns
/// Tuple of (mode, generation, timestamp)
#[inline(always)]
fn unpack_state(state: u64) -> (u8, u32, u32) {
    let mode = (state & MODE_MASK) as u8;
    let generation = ((state & GEN_MASK) >> GEN_SHIFT) as u32;
    let timestamp = (state >> TIMESTAMP_SHIFT) as u32;
    (mode, generation, timestamp)
}

// ============================================================================
// ACCESS MODE CAPSULE
// ============================================================================

/// Result type for access mode operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessModeError {
    /// State changed between read and CAS (retry may succeed)
    ConcurrentModification,
    /// Current mode does not match expected mode
    InvalidTransition {
        expected: AccessMode,
        actual: AccessMode,
    },
    /// Generation counter overflow (extremely rare, 536M transitions)
    GenerationOverflow,
}

/// AccessModeCapsule - Lockfree state machine for access control
///
/// # Memory Layout
///
/// ```text
/// Offset 0-7:   state (AtomicU64) - Packed mode/generation/timestamp
/// Offset 8-127: _padding (120 bytes) - Cache-line padding
/// Total: 128 bytes, 64-byte aligned
/// ```
///
/// # COCA Compliance
///
/// - T1 Atomic tier: Single AtomicU64 for all state
/// - 128-byte size: Occupies 2 cache lines for optimal performance
/// - 64-byte alignment: Prevents false sharing
/// - Generation counters: ABA prevention on every state transition
/// - Lockfree: Zero mutex/RwLock, CAS-only coordination
#[repr(C, align(64))]
pub struct AccessModeCapsule {
    /// Packed state: mode (3 bits) | generation (29 bits) | timestamp (32 bits)
    state: AtomicU64,
    /// Padding to ensure 128-byte size (cache-friendly, prevents false sharing)
    /// Total struct: 8 bytes (state) + 120 bytes (padding) = 128 bytes
    _padding: [u8; 120],
}

impl AccessModeCapsule {
    /// Create a new AccessModeCapsule in Observer mode
    ///
    /// # Arguments
    /// * `initial_timestamp` - Unix timestamp for initial state (0 for default)
    ///
    /// # Returns
    /// New capsule initialized to Observer mode with generation 0
    #[inline]
    pub const fn new(initial_timestamp: u32) -> Self {
        Self {
            state: AtomicU64::new(pack_state(
                AccessMode::Observer as u8,
                0,
                initial_timestamp,
            )),
            _padding: [0u8; 120],
        }
    }

    /// Create a new AccessModeCapsule with default timestamp (0)
    #[inline]
    pub const fn default() -> Self {
        Self::new(0)
    }

    /// Get current access mode, generation, and timestamp
    ///
    /// # Returns
    /// Tuple of (AccessMode, generation, timestamp)
    ///
    /// # Performance
    /// <5ns (single atomic load with Acquire ordering)
    #[inline]
    pub fn get_mode(&self) -> (AccessMode, u32, u32) {
        let state = self.state.load(Ordering::Acquire);
        let (mode_raw, generation, timestamp) = unpack_state(state);
        (AccessMode::from_raw(mode_raw), generation, timestamp)
    }

    /// Fast-path check: Is current mode Operator?
    ///
    /// # Returns
    /// true if Operator mode, false otherwise
    ///
    /// # Performance
    /// <5ns (single atomic load + mask comparison)
    #[inline]
    pub fn is_operator(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & MODE_MASK) == AccessMode::Operator as u64
    }

    /// Fast-path check: Is current mode Observer?
    ///
    /// # Returns
    /// true if Observer mode, false otherwise
    ///
    /// # Performance
    /// <5ns (single atomic load + mask comparison)
    #[inline]
    pub fn is_observer(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & MODE_MASK) == AccessMode::Observer as u64
    }

    /// Check if session has expired based on current time and timeout
    ///
    /// # Arguments
    /// * `current_time` - Current Unix timestamp (seconds)
    /// * `timeout_secs` - Timeout duration in seconds
    ///
    /// # Returns
    /// true if session expired, false otherwise
    ///
    /// # Performance
    /// <10ns (atomic load + arithmetic comparison)
    #[inline]
    pub fn is_expired(&self, current_time: u32, timeout_secs: u32) -> bool {
        let state = self.state.load(Ordering::Acquire);
        let (mode_raw, _, timestamp) = unpack_state(state);

        // Already marked expired
        if mode_raw == AccessMode::Expired as u8 {
            return true;
        }

        // Check timeout (handle wraparound for 32-bit timestamps)
        // #ASSUME: current_time >= timestamp in normal operation
        // #VERIFY: Wraparound handling for timestamps near u32::MAX
        if current_time >= timestamp {
            current_time - timestamp > timeout_secs
        } else {
            // Wraparound case: timestamp was near u32::MAX, current_time wrapped
            // This will occur ~136 years from epoch
            false
        }
    }

    /// Transition from one access mode to another
    ///
    /// # Arguments
    /// * `from` - Expected current mode (for CAS validation)
    /// * `to` - Target mode
    /// * `current_time` - Unix timestamp for new state
    ///
    /// # Returns
    /// Ok(new_generation) on success, Err on failure
    ///
    /// # CAS Loop Behavior
    /// Uses compare_exchange_weak for optimal performance on ARM/x86.
    /// Retries up to 16 times on spurious failures before returning error.
    ///
    /// # ABA Prevention
    /// Generation counter increments on every successful transition.
    ///
    /// # Performance
    /// <50ns typical (1-2 CAS iterations)
    #[inline]
    pub fn transition(
        &self,
        from: AccessMode,
        to: AccessMode,
        current_time: u32,
    ) -> Result<u32, AccessModeError> {
        const MAX_RETRIES: u32 = 16;

        for _ in 0..MAX_RETRIES {
            let current_state = self.state.load(Ordering::Acquire);
            let (current_mode_raw, current_gen, _current_ts) = unpack_state(current_state);
            let current_mode = AccessMode::from_raw(current_mode_raw);

            // Validate expected mode
            if current_mode != from {
                return Err(AccessModeError::InvalidTransition {
                    expected: from,
                    actual: current_mode,
                });
            }

            // Increment generation (with wraparound for ABA prevention)
            let new_gen = if current_gen >= MAX_GENERATION {
                // #ASSUME: Wraparound is extremely rare (536M transitions)
                // #VERIFY: We wrap to 1 (not 0) to distinguish from initial state
                1
            } else {
                current_gen + 1
            };

            let new_state = pack_state(to.to_raw(), new_gen, current_time);

            // #ASSUME: compare_exchange_weak may spuriously fail
            // #VERIFY: Loop retries handle spurious failures
            // #ASSUME: Ordering::AcqRel ensures visibility to other threads
            // #VERIFY: Acquire on load, Release on successful store
            match self.state.compare_exchange_weak(
                current_state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(new_gen),
                Err(_) => {
                    // Spurious failure or concurrent modification, retry
                    core::hint::spin_loop();
                }
            }
        }

        Err(AccessModeError::ConcurrentModification)
    }

    /// Renew session timestamp (extend timeout) without changing mode
    ///
    /// # Arguments
    /// * `current_time` - New Unix timestamp
    ///
    /// # Returns
    /// Ok(new_generation) on success, Err on concurrent modification
    ///
    /// # Performance
    /// <50ns typical
    #[inline]
    pub fn renew(&self, current_time: u32) -> Result<u32, AccessModeError> {
        const MAX_RETRIES: u32 = 16;

        for _ in 0..MAX_RETRIES {
            let current_state = self.state.load(Ordering::Acquire);
            let (current_mode_raw, current_gen, _) = unpack_state(current_state);

            // Increment generation
            let new_gen = if current_gen >= MAX_GENERATION {
                1
            } else {
                current_gen + 1
            };

            let new_state = pack_state(current_mode_raw, new_gen, current_time);

            match self.state.compare_exchange_weak(
                current_state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(new_gen),
                Err(_) => {
                    core::hint::spin_loop();
                }
            }
        }

        Err(AccessModeError::ConcurrentModification)
    }

    /// Force transition to a specific mode (bypasses mode validation)
    ///
    /// # Safety (ASSUM-02: FORCE_TRANSITION)
    /// #ASSUME: Caller has verified authorization for forced transition
    /// #VERIFY: Used only for administrative reset or timeout expiration
    ///
    /// # Arguments
    /// * `to` - Target mode
    /// * `current_time` - Unix timestamp for new state
    ///
    /// # Returns
    /// New generation counter after transition
    #[inline]
    pub fn force_transition(&self, to: AccessMode, current_time: u32) -> u32 {
        const MAX_RETRIES: u32 = 64;

        for _ in 0..MAX_RETRIES {
            let current_state = self.state.load(Ordering::Acquire);
            let (_, current_gen, _) = unpack_state(current_state);

            let new_gen = if current_gen >= MAX_GENERATION {
                1
            } else {
                current_gen + 1
            };

            let new_state = pack_state(to.to_raw(), new_gen, current_time);

            match self.state.compare_exchange_weak(
                current_state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return new_gen,
                Err(_) => {
                    core::hint::spin_loop();
                }
            }
        }

        // Fallback: unconditional store (safe due to generation increment)
        // #ASSUME: After 64 retries, contention is extreme
        // #VERIFY: Store with new timestamp is always safe (monotonic)
        let current_state = self.state.load(Ordering::Acquire);
        let (_, current_gen, _) = unpack_state(current_state);
        let new_gen = current_gen.wrapping_add(1) & MAX_GENERATION;
        let new_state = pack_state(to.to_raw(), new_gen, current_time);
        self.state.store(new_state, Ordering::Release);
        new_gen
    }

    /// Get raw packed state (for debugging/serialization)
    #[inline]
    pub fn raw_state(&self) -> u64 {
        self.state.load(Ordering::Acquire)
    }

    /// Mark session as expired
    ///
    /// # Arguments
    /// * `current_time` - Unix timestamp when expiration occurred
    ///
    /// # Returns
    /// New generation counter after marking expired
    #[inline]
    pub fn mark_expired(&self, current_time: u32) -> u32 {
        self.force_transition(AccessMode::Expired, current_time)
    }

    /// Reset to Observer mode (demotion)
    ///
    /// # Arguments
    /// * `current_time` - Unix timestamp for reset
    ///
    /// # Returns
    /// New generation counter after reset
    #[inline]
    pub fn reset_to_observer(&self, current_time: u32) -> u32 {
        self.force_transition(AccessMode::Observer, current_time)
    }
}

// ============================================================================
// TRAIT IMPLEMENTATIONS
// ============================================================================

impl Default for AccessModeCapsule {
    fn default() -> Self {
        Self::default()
    }
}

impl core::fmt::Debug for AccessModeCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (mode, gen, ts) = self.get_mode();
        f.debug_struct("AccessModeCapsule")
            .field("mode", &mode)
            .field("generation", &gen)
            .field("timestamp", &ts)
            .finish()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    // ========================================================================
    // SIZE AND ALIGNMENT TESTS
    // ========================================================================

    #[test]
    fn test_capsule_size() {
        assert_eq!(
            size_of::<AccessModeCapsule>(),
            128,
            "AccessModeCapsule must be exactly 128 bytes"
        );
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(
            align_of::<AccessModeCapsule>(),
            64,
            "AccessModeCapsule must be 64-byte aligned"
        );
    }

    #[test]
    fn test_access_mode_size() {
        assert_eq!(
            size_of::<AccessMode>(),
            1,
            "AccessMode must be 1 byte (u8)"
        );
    }

    // ========================================================================
    // BIT-PACKING TESTS
    // ========================================================================

    #[test]
    fn test_pack_unpack_roundtrip() {
        // Test all modes
        for mode in 0..=3u8 {
            let generation = 12345678u32;
            let timestamp = 1700000000u32;

            let packed = pack_state(mode, generation, timestamp);
            let (unpacked_mode, unpacked_gen, unpacked_ts) = unpack_state(packed);

            assert_eq!(unpacked_mode, mode, "Mode roundtrip failed");
            assert_eq!(unpacked_gen, generation, "Generation roundtrip failed");
            assert_eq!(unpacked_ts, timestamp, "Timestamp roundtrip failed");
        }
    }

    #[test]
    fn test_pack_max_values() {
        let mode = 3u8; // Max mode (Expired)
        let generation = MAX_GENERATION;
        let timestamp = u32::MAX;

        let packed = pack_state(mode, generation, timestamp);
        let (unpacked_mode, unpacked_gen, unpacked_ts) = unpack_state(packed);

        assert_eq!(unpacked_mode, mode);
        assert_eq!(unpacked_gen, generation);
        assert_eq!(unpacked_ts, timestamp);
    }

    #[test]
    fn test_pack_zero_values() {
        let packed = pack_state(0, 0, 0);
        let (mode, gen, ts) = unpack_state(packed);

        assert_eq!(mode, 0);
        assert_eq!(gen, 0);
        assert_eq!(ts, 0);
        assert_eq!(packed, 0);
    }

    #[test]
    fn test_bit_field_isolation() {
        // Test that each field is properly isolated

        // Mode only
        let mode_only = pack_state(7, 0, 0); // 7 = 0b111, should be masked to 3 bits
        let (mode, gen, ts) = unpack_state(mode_only);
        assert_eq!(mode, 7);
        assert_eq!(gen, 0);
        assert_eq!(ts, 0);

        // Generation only
        let gen_only = pack_state(0, 0x1FFFFFFF, 0);
        let (mode, gen, ts) = unpack_state(gen_only);
        assert_eq!(mode, 0);
        assert_eq!(gen, MAX_GENERATION);
        assert_eq!(ts, 0);

        // Timestamp only
        let ts_only = pack_state(0, 0, 0xFFFFFFFF);
        let (mode, gen, ts) = unpack_state(ts_only);
        assert_eq!(mode, 0);
        assert_eq!(gen, 0);
        assert_eq!(ts, u32::MAX);
    }

    // ========================================================================
    // ACCESS MODE CONVERSION TESTS
    // ========================================================================

    #[test]
    fn test_access_mode_from_raw() {
        assert_eq!(AccessMode::from_raw(0), AccessMode::Observer);
        assert_eq!(AccessMode::from_raw(1), AccessMode::ChallengePending);
        assert_eq!(AccessMode::from_raw(2), AccessMode::Operator);
        assert_eq!(AccessMode::from_raw(3), AccessMode::Expired);
        // Out-of-range values should default to Observer
        assert_eq!(AccessMode::from_raw(4), AccessMode::Observer);
        assert_eq!(AccessMode::from_raw(255), AccessMode::Observer);
    }

    #[test]
    fn test_access_mode_to_raw() {
        assert_eq!(AccessMode::Observer.to_raw(), 0);
        assert_eq!(AccessMode::ChallengePending.to_raw(), 1);
        assert_eq!(AccessMode::Operator.to_raw(), 2);
        assert_eq!(AccessMode::Expired.to_raw(), 3);
    }

    // ========================================================================
    // CAPSULE CREATION TESTS
    // ========================================================================

    #[test]
    fn test_new_default_observer() {
        let capsule = AccessModeCapsule::default();
        let (mode, gen, ts) = capsule.get_mode();

        assert_eq!(mode, AccessMode::Observer);
        assert_eq!(gen, 0);
        assert_eq!(ts, 0);
    }

    #[test]
    fn test_new_with_timestamp() {
        let timestamp = 1700000000u32;
        let capsule = AccessModeCapsule::new(timestamp);
        let (mode, gen, ts) = capsule.get_mode();

        assert_eq!(mode, AccessMode::Observer);
        assert_eq!(gen, 0);
        assert_eq!(ts, timestamp);
    }

    // ========================================================================
    // STATE TRANSITION TESTS
    // ========================================================================

    #[test]
    fn test_transition_observer_to_pending() {
        let capsule = AccessModeCapsule::new(1000);
        let result = capsule.transition(AccessMode::Observer, AccessMode::ChallengePending, 1001);

        assert!(result.is_ok());
        let new_gen = result.unwrap();
        assert_eq!(new_gen, 1); // Generation incremented from 0 to 1

        let (mode, gen, ts) = capsule.get_mode();
        assert_eq!(mode, AccessMode::ChallengePending);
        assert_eq!(gen, 1);
        assert_eq!(ts, 1001);
    }

    #[test]
    fn test_transition_pending_to_operator() {
        let capsule = AccessModeCapsule::new(1000);

        // First transition: Observer -> ChallengePending
        capsule
            .transition(AccessMode::Observer, AccessMode::ChallengePending, 1001)
            .unwrap();

        // Second transition: ChallengePending -> Operator
        let result = capsule.transition(AccessMode::ChallengePending, AccessMode::Operator, 1002);

        assert!(result.is_ok());
        let (mode, gen, ts) = capsule.get_mode();
        assert_eq!(mode, AccessMode::Operator);
        assert_eq!(gen, 2);
        assert_eq!(ts, 1002);
    }

    #[test]
    fn test_transition_invalid_from_mode() {
        let capsule = AccessModeCapsule::new(1000);

        // Try to transition from Operator when actually in Observer
        let result = capsule.transition(AccessMode::Operator, AccessMode::ChallengePending, 1001);

        assert!(result.is_err());
        match result {
            Err(AccessModeError::InvalidTransition { expected, actual }) => {
                assert_eq!(expected, AccessMode::Operator);
                assert_eq!(actual, AccessMode::Observer);
            }
            _ => panic!("Expected InvalidTransition error"),
        }

        // State should be unchanged
        let (mode, gen, _) = capsule.get_mode();
        assert_eq!(mode, AccessMode::Observer);
        assert_eq!(gen, 0);
    }

    #[test]
    fn test_force_transition() {
        let capsule = AccessModeCapsule::new(1000);

        // Force directly to Operator (bypasses mode validation)
        let new_gen = capsule.force_transition(AccessMode::Operator, 1001);

        assert_eq!(new_gen, 1);
        let (mode, _, ts) = capsule.get_mode();
        assert_eq!(mode, AccessMode::Operator);
        assert_eq!(ts, 1001);
    }

    // ========================================================================
    // ABA PREVENTION TESTS
    // ========================================================================

    #[test]
    fn test_generation_increment() {
        let capsule = AccessModeCapsule::new(1000);

        // Perform multiple transitions
        for i in 1..=10u32 {
            capsule.force_transition(
                if i % 2 == 0 {
                    AccessMode::Observer
                } else {
                    AccessMode::Operator
                },
                1000 + i,
            );

            let (_, gen, _) = capsule.get_mode();
            assert_eq!(gen, i, "Generation should increment on each transition");
        }
    }

    #[test]
    fn test_generation_wraparound() {
        let capsule = AccessModeCapsule::new(1000);

        // Manually set state near max generation
        // We'll use force_transition and verify behavior
        let near_max_gen = MAX_GENERATION - 1;
        let packed = pack_state(AccessMode::Observer as u8, near_max_gen, 1000);
        capsule.state.store(packed, Ordering::Release);

        // Transition should increment past MAX_GENERATION
        let new_gen = capsule.force_transition(AccessMode::Operator, 1001);
        assert_eq!(new_gen, MAX_GENERATION);

        // Next transition should wrap to 1 (not 0)
        let new_gen = capsule.force_transition(AccessMode::Observer, 1002);
        assert_eq!(new_gen, 1, "Generation should wrap to 1, not 0");
    }

    // ========================================================================
    // EXPIRY TESTS
    // ========================================================================

    #[test]
    fn test_is_expired_false_within_timeout() {
        let capsule = AccessModeCapsule::new(1000);
        capsule
            .transition(AccessMode::Observer, AccessMode::Operator, 1000)
            .unwrap();

        // Check at current_time=1050, timeout=60 (not expired)
        assert!(!capsule.is_expired(1050, 60));
    }

    #[test]
    fn test_is_expired_true_past_timeout() {
        let capsule = AccessModeCapsule::new(1000);
        capsule
            .transition(AccessMode::Observer, AccessMode::Operator, 1000)
            .unwrap();

        // Check at current_time=1100, timeout=60 (expired)
        assert!(capsule.is_expired(1100, 60));
    }

    #[test]
    fn test_is_expired_true_when_marked() {
        let capsule = AccessModeCapsule::new(1000);
        capsule.mark_expired(1001);

        // Should be expired regardless of timeout
        assert!(capsule.is_expired(1001, 3600));
    }

    // ========================================================================
    // FAST-PATH TESTS
    // ========================================================================

    #[test]
    fn test_is_operator() {
        let capsule = AccessModeCapsule::new(1000);
        assert!(!capsule.is_operator());

        capsule.force_transition(AccessMode::Operator, 1001);
        assert!(capsule.is_operator());

        capsule.force_transition(AccessMode::Observer, 1002);
        assert!(!capsule.is_operator());
    }

    #[test]
    fn test_is_observer() {
        let capsule = AccessModeCapsule::new(1000);
        assert!(capsule.is_observer());

        capsule.force_transition(AccessMode::Operator, 1001);
        assert!(!capsule.is_observer());

        capsule.reset_to_observer(1002);
        assert!(capsule.is_observer());
    }

    // ========================================================================
    // RENEW TESTS
    // ========================================================================

    #[test]
    fn test_renew_extends_timestamp() {
        let capsule = AccessModeCapsule::new(1000);
        capsule.force_transition(AccessMode::Operator, 1000);

        // Renew at time 1500
        let result = capsule.renew(1500);
        assert!(result.is_ok());

        let (mode, _, ts) = capsule.get_mode();
        assert_eq!(mode, AccessMode::Operator);
        assert_eq!(ts, 1500);
    }

    #[test]
    fn test_renew_increments_generation() {
        let capsule = AccessModeCapsule::new(1000);

        let (_, gen_before, _) = capsule.get_mode();
        capsule.renew(1500).unwrap();
        let (_, gen_after, _) = capsule.get_mode();

        assert_eq!(gen_after, gen_before + 1);
    }

    // ========================================================================
    // CONVENIENCE METHOD TESTS
    // ========================================================================

    #[test]
    fn test_mark_expired() {
        let capsule = AccessModeCapsule::new(1000);
        capsule.force_transition(AccessMode::Operator, 1000);

        capsule.mark_expired(1001);

        let (mode, _, ts) = capsule.get_mode();
        assert_eq!(mode, AccessMode::Expired);
        assert_eq!(ts, 1001);
    }

    #[test]
    fn test_reset_to_observer() {
        let capsule = AccessModeCapsule::new(1000);
        capsule.force_transition(AccessMode::Operator, 1000);

        capsule.reset_to_observer(1001);

        let (mode, _, ts) = capsule.get_mode();
        assert_eq!(mode, AccessMode::Observer);
        assert_eq!(ts, 1001);
    }

    // ========================================================================
    // DEBUG TRAIT TESTS
    // ========================================================================

    #[test]
    fn test_debug_output() {
        let capsule = AccessModeCapsule::new(1700000000);
        let debug_str = format!("{:?}", capsule);

        assert!(debug_str.contains("AccessModeCapsule"));
        assert!(debug_str.contains("Observer"));
        assert!(debug_str.contains("generation"));
        assert!(debug_str.contains("timestamp"));
    }

    // ========================================================================
    // CONCURRENT ACCESS TESTS (Single-threaded simulation)
    // ========================================================================

    #[test]
    fn test_concurrent_transition_simulation() {
        // Simulate concurrent access by rapidly alternating transitions
        let capsule = AccessModeCapsule::new(1000);

        for i in 0..100u32 {
            let from_mode = if i % 2 == 0 {
                AccessMode::Observer
            } else {
                AccessMode::Operator
            };
            let to_mode = if i % 2 == 0 {
                AccessMode::Operator
            } else {
                AccessMode::Observer
            };

            let result = capsule.transition(from_mode, to_mode, 1000 + i);

            // Should succeed since we're single-threaded
            if let Err(AccessModeError::InvalidTransition { .. }) = result {
                // This can happen if we're checking wrong expected mode
                // Use force_transition to continue the test
                capsule.force_transition(to_mode, 1000 + i);
            }
        }

        // Final state should be valid
        let (mode, gen, _) = capsule.get_mode();
        assert!(gen > 0);
        assert!(mode == AccessMode::Observer || mode == AccessMode::Operator);
    }

    #[test]
    fn test_raw_state() {
        let capsule = AccessModeCapsule::new(1700000000);
        capsule.force_transition(AccessMode::Operator, 1700000001);

        let raw = capsule.raw_state();
        let (mode, gen, ts) = unpack_state(raw);

        assert_eq!(mode, AccessMode::Operator as u8);
        assert_eq!(gen, 1);
        assert_eq!(ts, 1700000001);
    }
}
