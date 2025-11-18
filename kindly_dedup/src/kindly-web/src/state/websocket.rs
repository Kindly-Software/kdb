//! WebSocketStateCapsule - Tier 1 Atomic (128B)
//!
//! Purpose: WebSocket connection state tracking
//! Memory Layout:
//!   [0]     state: AtomicU8 (0=Disconnected, 1=Connecting, 2=Connected)
//!   [1-7]   _pad1: [u8; 7]
//!   [8-15]  last_ping_ns: AtomicU64 (timestamp of last ping in nanoseconds)
//!   [16-23] packed: AtomicU64 (message_count:32b + generation:32b)
//!   [24-127] _padding: [u8; 104] (cache alignment)

use super::error::{CapsuleError, CapsuleResult};
use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

/// WebSocket connection states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WebSocketState {
    Disconnected = 0,
    Connecting = 1,
    Connected = 2,
}

impl WebSocketState {
    fn from_u8(value: u8) -> CapsuleResult<Self> {
        match value {
            0 => Ok(Self::Disconnected),
            1 => Ok(Self::Connecting),
            2 => Ok(Self::Connected),
            _ => Err(CapsuleError::InvalidValue {
                message: format!("Invalid WebSocket state: {}", value),
            }),
        }
    }
}

/// Tier 1 Atomic: WebSocket state capsule (128B cache-aligned)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct WebSocketStateCapsule {
    /// Connection state (0=Disconnected, 1=Connecting, 2=Connected)
    state: AtomicU8,
    /// Padding for alignment
    _pad1: [u8; 7],
    /// Last ping timestamp (nanoseconds)
    last_ping_ns: AtomicU64,
    /// Packed: message_count(32b) + generation(32b)
    packed: AtomicU64,
    /// Padding to 128 bytes
    _padding: [u8; 104],
}

const MESSAGE_COUNT_MASK: u64 = 0xFFFF_FFFF;
const GENERATION_MASK: u64 = 0xFFFF_FFFF_0000_0000;
const GENERATION_SHIFT: u32 = 32;

impl WebSocketStateCapsule {
    /// Create new WebSocket state capsule
    ///
    /// # Returns
    /// WebSocketStateCapsule in Disconnected state
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(WebSocketState::Disconnected as u8),
            _pad1: [0u8; 7],
            last_ping_ns: AtomicU64::new(0),
            packed: AtomicU64::new(0),
            _padding: [0u8; 104],
        }
    }

    /// Update connection state
    ///
    /// #ASSUME: Atomic CAS prevents invalid state transitions
    /// #VERIFY: State transitions validated (Disconnected <-> Connecting <-> Connected)
    ///
    /// # Arguments
    /// * `new_state` - New WebSocket state
    ///
    /// # Returns
    /// Previous state or error if invalid transition
    pub fn update_state(&self, new_state: WebSocketState) -> CapsuleResult<WebSocketState> {
        // #ASSUME: Acquire ordering ensures state read before transition
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let old_state = WebSocketState::from_u8(current)?;

            // Validate transition
            let valid_transition = match (old_state, new_state) {
                (WebSocketState::Disconnected, WebSocketState::Connecting) => true,
                (WebSocketState::Connecting, WebSocketState::Connected) => true,
                (WebSocketState::Connecting, WebSocketState::Disconnected) => true, // Connection failed
                (WebSocketState::Connected, WebSocketState::Disconnected) => true,  // Disconnection
                (a, b) if a == b => true,                                           // Same state (idempotent)
                _ => false,
            };

            if !valid_transition {
                return Err(CapsuleError::InvalidStateTransition {
                    from: format!("{:?}", old_state),
                    to: format!("{:?}", new_state),
                });
            }

            // #ASSUME: CAS with Release ensures state visible to other threads
            match self
                .state
                .compare_exchange_weak(current, new_state as u8, Ordering::Release, Ordering::Acquire)
            {
                Ok(_) => {
                    // Increment generation on state change
                    if old_state != new_state {
                        self._increment_generation();
                    }
                    return Ok(old_state);
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Get current state
    ///
    /// #ASSUME: Acquire load ensures latest state visible
    pub fn get_state(&self) -> CapsuleResult<WebSocketState> {
        let state_u8 = self.state.load(Ordering::Acquire);
        WebSocketState::from_u8(state_u8)
    }

    /// Check if connected
    ///
    /// #ASSUME: Acquire load ensures latest state visible
    pub fn is_connected(&self) -> bool {
        self.state.load(Ordering::Acquire) == WebSocketState::Connected as u8
    }

    /// Update ping timestamp
    ///
    /// #ASSUME: Atomic store prevents race conditions
    ///
    /// # Arguments
    /// * `timestamp_ns` - Timestamp in nanoseconds
    pub fn ping(&self, timestamp_ns: u64) {
        // #ASSUME: Release ordering ensures ping timestamp visible
        self.last_ping_ns.store(timestamp_ns, Ordering::Release);
    }

    /// Get last ping timestamp
    ///
    /// #ASSUME: Acquire load ensures latest ping timestamp visible
    pub fn get_last_ping_ns(&self) -> u64 {
        self.last_ping_ns.load(Ordering::Acquire)
    }

    /// Record incoming/outgoing message
    ///
    /// #ASSUME: Fetch-add with Relaxed safe (message_count is monotonic)
    ///
    /// # Returns
    /// New message count after increment
    pub fn record_message(&self) -> u32 {
        let mut current = self.packed.load(Ordering::Relaxed);
        loop {
            let count = (current & MESSAGE_COUNT_MASK) as u32;
            let gen = ((current & GENERATION_MASK) >> GENERATION_SHIFT) as u32;

            let new_count = count.wrapping_add(1);
            let new_packed = (new_count as u64) | ((gen as u64) << GENERATION_SHIFT);

            match self
                .packed
                .compare_exchange_weak(current, new_packed, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return new_count,
                Err(actual) => current = actual,
            }
        }
    }

    /// Get message count
    ///
    /// #ASSUME: Relaxed load safe (message_count is audit counter)
    pub fn get_message_count(&self) -> u32 {
        let packed = self.packed.load(Ordering::Relaxed);
        (packed & MESSAGE_COUNT_MASK) as u32
    }

    /// Get generation counter
    ///
    /// #ASSUME: Relaxed load safe (generation for TOCTOU only)
    pub fn generation(&self) -> u32 {
        let packed = self.packed.load(Ordering::Relaxed);
        ((packed & GENERATION_MASK) >> GENERATION_SHIFT) as u32
    }

    /// Increment generation counter (internal)
    ///
    /// #ASSUME: Fetch-add with Relaxed safe (generation is monotonic)
    fn _increment_generation(&self) {
        let mut current = self.packed.load(Ordering::Relaxed);
        loop {
            let count = (current & MESSAGE_COUNT_MASK) as u32;
            let gen = ((current & GENERATION_MASK) >> GENERATION_SHIFT) as u32;

            let new_gen = gen.wrapping_add(1);
            let new_packed = (count as u64) | ((new_gen as u64) << GENERATION_SHIFT);

            match self
                .packed
                .compare_exchange_weak(current, new_packed, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }

    /// Get snapshot of all values
    ///
    /// #ASSUME: Acquire loads ensure consistent snapshot
    ///
    /// # Returns
    /// (state, last_ping_ns, message_count, generation)
    pub fn snapshot(&self) -> CapsuleResult<(WebSocketState, u64, u32, u32)> {
        let state_u8 = self.state.load(Ordering::Acquire);
        let state = WebSocketState::from_u8(state_u8)?;
        let ping = self.last_ping_ns.load(Ordering::Acquire);
        let packed = self.packed.load(Ordering::Relaxed);
        let count = (packed & MESSAGE_COUNT_MASK) as u32;
        let gen = ((packed & GENERATION_MASK) >> GENERATION_SHIFT) as u32;
        Ok((state, ping, count, gen))
    }
}

impl Default for WebSocketStateCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_alignment() {
        assert_eq!(std::mem::align_of::<WebSocketStateCapsule>(), 128);
        assert_eq!(std::mem::size_of::<WebSocketStateCapsule>(), 128);
    }

    #[test]
    fn test_initial_state() {
        let ws = WebSocketStateCapsule::new();
        assert_eq!(ws.get_state().unwrap(), WebSocketState::Disconnected);
        assert_eq!(ws.is_connected(), false);
    }

    #[test]
    fn test_valid_state_transitions() {
        let ws = WebSocketStateCapsule::new();

        // Disconnected -> Connecting
        let old = ws.update_state(WebSocketState::Connecting).unwrap();
        assert_eq!(old, WebSocketState::Disconnected);
        assert_eq!(ws.get_state().unwrap(), WebSocketState::Connecting);

        // Connecting -> Connected
        let old = ws.update_state(WebSocketState::Connected).unwrap();
        assert_eq!(old, WebSocketState::Connecting);
        assert_eq!(ws.get_state().unwrap(), WebSocketState::Connected);
        assert_eq!(ws.is_connected(), true);

        // Connected -> Disconnected
        let old = ws.update_state(WebSocketState::Disconnected).unwrap();
        assert_eq!(old, WebSocketState::Connected);
        assert_eq!(ws.get_state().unwrap(), WebSocketState::Disconnected);
    }

    #[test]
    fn test_invalid_state_transition() {
        let ws = WebSocketStateCapsule::new();

        // Disconnected -> Connected (invalid)
        let result = ws.update_state(WebSocketState::Connected);
        assert!(result.is_err());
    }

    #[test]
    fn test_ping() {
        let ws = WebSocketStateCapsule::new();

        ws.ping(1234567890);
        assert_eq!(ws.get_last_ping_ns(), 1234567890);

        ws.ping(9876543210);
        assert_eq!(ws.get_last_ping_ns(), 9876543210);
    }

    #[test]
    fn test_record_message() {
        let ws = WebSocketStateCapsule::new();

        assert_eq!(ws.get_message_count(), 0);

        let count1 = ws.record_message();
        assert_eq!(count1, 1);
        assert_eq!(ws.get_message_count(), 1);

        let count2 = ws.record_message();
        assert_eq!(count2, 2);
        assert_eq!(ws.get_message_count(), 2);
    }

    #[test]
    fn test_snapshot() {
        let ws = WebSocketStateCapsule::new();
        ws.update_state(WebSocketState::Connecting).unwrap();
        ws.ping(123456);
        ws.record_message();
        ws.record_message();

        let (state, ping, count, _gen) = ws.snapshot().unwrap();
        assert_eq!(state, WebSocketState::Connecting);
        assert_eq!(ping, 123456);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_generation_on_state_change() {
        let ws = WebSocketStateCapsule::new();
        let gen0 = ws.generation();

        ws.update_state(WebSocketState::Connecting).unwrap();
        let gen1 = ws.generation();
        assert!(gen1 > gen0);

        ws.update_state(WebSocketState::Connected).unwrap();
        let gen2 = ws.generation();
        assert!(gen2 > gen1);
    }
}
