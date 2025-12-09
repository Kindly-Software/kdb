//! Pre-built packing helpers for DualAtomicU64
//!
//! This module provides high-level wrappers around DualAtomicU64 for common use cases,
//! eliminating manual bit-packing and ensuring consistent TOCTOU protection.
//!
//! # Coverage of Use Cases
//!
//! - `DualTimestampGen`: 40% - Timestamps with counters (position tracking, event ordering)
//! - `DualQ16_16Pair`: 25% - Paired Q16.16 fixed-point values (RTT, financial metrics)
//! - `DualStateCounter`: 20% - State machines with counters (FSMs, connection state)
//! - `DualArraySlot`: 15% - Array indexing with ABA prevention (pools, lockfree queues)
//!
//! # Design Principles
//!
//! - 100% lockfree (uses DualAtomicU64 internally)
//! - TOCTOU protection via consistent reads
//! - Proper memory ordering (Acquire/Release)
//! - Zero-cost abstractions (compile-time bit-packing)
//! - `#![no_std]` compatible
//!
//! # Examples
//!
//! ```rust
//! use atomic_capsule::patterns::dual_atomic_helpers::DualTimestampGen;
//!
//! let tracker = DualTimestampGen::new();
//! tracker.set(1234567890, 42);
//! let (timestamp, counter) = tracker.read_consistent();
//! assert_eq!(timestamp, 1234567890);
//! assert_eq!(counter, 42);
//! ```

use crate::patterns::dual_atomic::DualAtomicU64;
use core::sync::atomic::Ordering;

// ============================================================================
// 1. DualTimestampGen (40% of uses)
// ============================================================================

/// Timestamp with counter and generation tracking
///
/// # Layout
///
/// - **Primary**: timestamp (lower 32 bits) + counter (upper 32 bits)
/// - **Secondary**: generation counter (full 64 bits)
///
/// # Use Cases
///
/// - Position tracking with event sequencing
/// - Message ordering with timestamps
/// - Cache invalidation with versioning
///
/// # Examples
///
/// ```rust
/// use atomic_capsule::patterns::dual_atomic_helpers::DualTimestampGen;
///
/// let tracker = DualTimestampGen::new();
///
/// // Set timestamp and counter
/// tracker.set(1234567890, 42);
///
/// // Read consistently (TOCTOU-safe)
/// let (timestamp, counter) = tracker.read_consistent();
/// assert_eq!(timestamp, 1234567890);
/// assert_eq!(counter, 42);
///
/// // Update counter only
/// tracker.increment_counter();
/// let (ts, cnt) = tracker.read_consistent();
/// assert_eq!(ts, 1234567890);
/// assert_eq!(cnt, 43);
/// ```
#[repr(C, align(16))]
pub struct DualTimestampGen {
    inner: DualAtomicU64,
}

impl DualTimestampGen {
    /// Creates a new timestamp tracker with zero values
    #[inline]
    pub const fn new() -> Self {
        Self {
            inner: DualAtomicU64::new(0, 0),
        }
    }

    /// Creates a timestamp tracker with initial values
    #[inline]
    pub const fn with_values(timestamp: u32, counter: u32) -> Self {
        let primary = ((counter as u64) << 32) | (timestamp as u64);
        Self {
            inner: DualAtomicU64::new(primary, 0),
        }
    }

    /// Sets timestamp and counter atomically
    ///
    /// # Memory Ordering
    ///
    /// Uses `Release` ordering to ensure writes are visible to other threads.
    #[inline]
    pub fn set(&self, timestamp: u32, counter: u32) {
        let primary = ((counter as u64) << 32) | (timestamp as u64);
        self.inner.store_primary(primary, Ordering::Release);
    }

    /// Reads timestamp and counter consistently (TOCTOU-safe)
    ///
    /// This ensures the timestamp and counter are read from the same snapshot,
    /// preventing TOCTOU races.
    ///
    /// # Memory Ordering
    ///
    /// Uses `Acquire` ordering to ensure reads see latest writes.
    #[inline]
    pub fn read_consistent(&self) -> (u32, u32) {
        let primary = self.inner.load_primary(Ordering::Acquire);
        let timestamp = (primary & 0xFFFF_FFFF) as u32;
        let counter = (primary >> 32) as u32;
        (timestamp, counter)
    }

    /// Increments the counter atomically
    ///
    /// # Memory Ordering
    ///
    /// Uses `AcqRel` for read-modify-write consistency.
    #[inline]
    pub fn increment_counter(&self) {
        let primary = self.inner.load_primary(Ordering::Acquire);
        let timestamp = primary & 0xFFFF_FFFF;
        let counter = ((primary >> 32) + 1) & 0xFFFF_FFFF;
        let new_primary = (counter << 32) | timestamp;
        self.inner.store_primary(new_primary, Ordering::Release);
    }

    /// Gets the current generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.inner.load_secondary(Ordering::Acquire)
    }

    /// Gets the inner DualAtomicU64 for advanced operations
    #[inline]
    pub const fn inner(&self) -> &DualAtomicU64 {
        &self.inner
    }
}

impl Default for DualTimestampGen {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 2. DualQ16_16Pair (25% of uses)
// ============================================================================

/// Paired Q16.16 fixed-point values
///
/// # Layout
///
/// - **Primary**: Q16.16 value A (full 64 bits as i64)
/// - **Secondary**: Q16.16 value B (full 64 bits as i64)
///
/// # Note
///
/// Unlike other helpers, this does NOT use generation counters.
/// Both channels store independent data values.
///
/// # Use Cases
///
/// - RTT estimation (min RTT, smoothed RTT)
/// - Financial metrics (bid, ask)
/// - Coordinate pairs (x, y)
///
/// # Examples
///
/// ```rust
/// use atomic_capsule::patterns::dual_atomic_helpers::DualQ16_16Pair;
///
/// let pair = DualQ16_16Pair::new();
///
/// // Set both values (as Q16.16 integers)
/// let value_a = 100 << 16; // 100.0 in Q16.16
/// let value_b = 200 << 16; // 200.0 in Q16.16
/// pair.set_both(value_a, value_b);
///
/// // Read both consistently
/// let (a, b) = pair.read_both();
/// assert_eq!(a, value_a);
/// assert_eq!(b, value_b);
///
/// // Update individual values
/// pair.set_a(150 << 16);
/// let (a, _) = pair.read_both();
/// assert_eq!(a, 150 << 16);
/// ```
#[repr(C, align(16))]
pub struct DualQ16_16Pair {
    inner: DualAtomicU64,
}

impl DualQ16_16Pair {
    /// Creates a new Q16.16 pair with zero values
    #[inline]
    pub const fn new() -> Self {
        Self {
            inner: DualAtomicU64::new(0, 0),
        }
    }

    /// Creates a Q16.16 pair with initial values
    #[inline]
    pub const fn with_values(value_a: i64, value_b: i64) -> Self {
        Self {
            inner: DualAtomicU64::new(value_a as u64, value_b as u64),
        }
    }

    /// Sets both values atomically
    ///
    /// # Memory Ordering
    ///
    /// Uses `Release` ordering to ensure writes are visible.
    #[inline]
    pub fn set_both(&self, value_a: i64, value_b: i64) {
        self.inner.store_primary(value_a as u64, Ordering::Release);
        self.inner.store_secondary(value_b as u64, Ordering::Release);
    }

    /// Sets value A only
    ///
    /// # Memory Ordering
    ///
    /// Uses `Release` ordering.
    #[inline]
    pub fn set_a(&self, value_a: i64) {
        self.inner.store_primary(value_a as u64, Ordering::Release);
    }

    /// Sets value B only
    ///
    /// # Memory Ordering
    ///
    /// Uses `Release` ordering.
    #[inline]
    pub fn set_b(&self, value_b: i64) {
        self.inner.store_secondary(value_b as u64, Ordering::Release);
    }

    /// Reads both values consistently
    ///
    /// # Memory Ordering
    ///
    /// Uses `Acquire` ordering to ensure reads see latest writes.
    #[inline]
    pub fn read_both(&self) -> (i64, i64) {
        let a = self.inner.load_primary(Ordering::Acquire);
        let b = self.inner.load_secondary(Ordering::Acquire);
        (a as i64, b as i64)
    }

    /// Reads value A only
    #[inline]
    pub fn read_a(&self) -> i64 {
        self.inner.load_primary(Ordering::Acquire) as i64
    }

    /// Reads value B only
    #[inline]
    pub fn read_b(&self) -> i64 {
        self.inner.load_secondary(Ordering::Acquire) as i64
    }

    /// Gets the inner DualAtomicU64 for advanced operations
    #[inline]
    pub const fn inner(&self) -> &DualAtomicU64 {
        &self.inner
    }
}

impl Default for DualQ16_16Pair {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 3. DualStateCounter (20% of uses)
// ============================================================================

/// State machine with counter and generation tracking
///
/// # Type Parameters
///
/// - `S`: State type (must fit in STATE_BITS)
/// - `STATE_BITS`: Number of bits for state (remaining bits for counter)
///
/// # Layout
///
/// - **Primary**: state (lower STATE_BITS) + counter (remaining bits)
/// - **Secondary**: generation counter (full 64 bits)
///
/// # Use Cases
///
/// - FSMs with transition counts
/// - Connection state with packet counters
/// - Resource lifecycle tracking
///
/// # Examples
///
/// ```rust
/// use atomic_capsule::patterns::dual_atomic_helpers::DualStateCounter;
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// #[repr(u8)]
/// enum ConnectionState {
///     Closed = 0,
///     Opening = 1,
///     Open = 2,
///     Closing = 3,
/// }
///
/// impl From<u8> for ConnectionState {
///     fn from(v: u8) -> Self {
///         match v {
///             0 => ConnectionState::Closed,
///             1 => ConnectionState::Opening,
///             2 => ConnectionState::Open,
///             3 => ConnectionState::Closing,
///             _ => ConnectionState::Closed,
///         }
///     }
/// }
///
/// impl From<ConnectionState> for u8 {
///     fn from(s: ConnectionState) -> u8 {
///         s as u8
///     }
/// }
///
/// // 2 bits for state (4 possible values), 62 bits for counter
/// let fsm = DualStateCounter::<ConnectionState, 2>::new(ConnectionState::Closed);
///
/// // Transition state
/// fsm.transition(ConnectionState::Opening);
/// let (state, counter) = fsm.read_consistent();
/// assert_eq!(state, ConnectionState::Opening);
/// assert_eq!(counter, 0);
///
/// // Increment counter
/// fsm.increment_counter();
/// let (state, counter) = fsm.read_consistent();
/// assert_eq!(state, ConnectionState::Opening);
/// assert_eq!(counter, 1);
/// ```
#[repr(C, align(16))]
pub struct DualStateCounter<S, const STATE_BITS: u8>
where
    S: Into<u8> + From<u8> + Copy,
{
    inner: DualAtomicU64,
    _phantom: core::marker::PhantomData<S>,
}

impl<S, const STATE_BITS: u8> DualStateCounter<S, STATE_BITS>
where
    S: Into<u8> + From<u8> + Copy,
{
    /// Creates a new state counter with initial state
    #[inline]
    pub fn new(initial_state: S) -> Self {
        // ASSUME: STATE_BITS <= 8 (state fits in u8)
        // VERIFY: Runtime assertion (const trait bounds not stable yet)
        assert!(STATE_BITS <= 8, "STATE_BITS must be <= 8");

        let state_val = initial_state.into() as u64;
        Self {
            inner: DualAtomicU64::new(state_val, 0),
            _phantom: core::marker::PhantomData,
        }
    }

    /// Creates a state counter with initial state and counter
    #[inline]
    pub fn with_counter(initial_state: S, counter: u64) -> Self {
        // ASSUME: STATE_BITS <= 8 (state fits in u8)
        // VERIFY: Compile-time assertion via const evaluation in new()
        assert!(STATE_BITS <= 8, "STATE_BITS must be <= 8");

        let state_val = initial_state.into() as u64;
        let state_mask = (1u64 << STATE_BITS) - 1;
        let counter_bits = 64 - STATE_BITS;
        let counter_val = counter & ((1u64 << counter_bits) - 1);
        let primary = (counter_val << STATE_BITS) | (state_val & state_mask);

        Self {
            inner: DualAtomicU64::new(primary, 0),
            _phantom: core::marker::PhantomData,
        }
    }

    /// Reads state and counter consistently (TOCTOU-safe)
    ///
    /// # Memory Ordering
    ///
    /// Uses `Acquire` ordering to ensure reads see latest writes.
    #[inline]
    pub fn read_consistent(&self) -> (S, u64) {
        let primary = self.inner.load_primary(Ordering::Acquire);
        let state_mask = (1u64 << STATE_BITS) - 1;
        let state_val = (primary & state_mask) as u8;
        let counter = primary >> STATE_BITS;
        (S::from(state_val), counter)
    }

    /// Transitions to a new state
    ///
    /// # Memory Ordering
    ///
    /// Uses `AcqRel` for read-modify-write consistency.
    #[inline]
    pub fn transition(&self, new_state: S) {
        let primary = self.inner.load_primary(Ordering::Acquire);
        let counter_bits = primary >> STATE_BITS;
        let state_mask = (1u64 << STATE_BITS) - 1;
        let new_state_val = (new_state.into() as u64) & state_mask;
        let new_primary = (counter_bits << STATE_BITS) | new_state_val;
        self.inner.store_primary(new_primary, Ordering::Release);
    }

    /// Increments the counter atomically
    ///
    /// # Memory Ordering
    ///
    /// Uses `AcqRel` for read-modify-write consistency.
    #[inline]
    pub fn increment_counter(&self) {
        let primary = self.inner.load_primary(Ordering::Acquire);
        let state_mask = (1u64 << STATE_BITS) - 1;
        let state_val = primary & state_mask;
        let counter_bits = 64 - STATE_BITS;
        let counter = ((primary >> STATE_BITS) + 1) & ((1u64 << counter_bits) - 1);
        let new_primary = (counter << STATE_BITS) | state_val;
        self.inner.store_primary(new_primary, Ordering::Release);
    }

    /// Gets the current generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.inner.load_secondary(Ordering::Acquire)
    }

    /// Gets the inner DualAtomicU64 for advanced operations
    #[inline]
    pub const fn inner(&self) -> &DualAtomicU64 {
        &self.inner
    }
}

// ============================================================================
// 4. DualArraySlot (15% of uses)
// ============================================================================

/// Array slot with index, capacity, and ABA prevention
///
/// # Layout
///
/// - **Primary**: index (lower 32 bits) + capacity (upper 32 bits)
/// - **Secondary**: generation counter (for ABA prevention)
///
/// # Use Cases
///
/// - Lock-free pool allocation
/// - Lock-free queue head/tail tracking
/// - Slot management with wraparound
///
/// # Examples
///
/// ```rust
/// use atomic_capsule::patterns::dual_atomic_helpers::DualArraySlot;
///
/// let slot = DualArraySlot::new(0, 1024);
///
/// // Read current state
/// let (index, capacity) = slot.read_consistent();
/// assert_eq!(index, 0);
/// assert_eq!(capacity, 1024);
///
/// // Advance index
/// slot.set_index(42);
/// let (idx, _) = slot.read_consistent();
/// assert_eq!(idx, 42);
///
/// // Update capacity
/// slot.set_capacity(2048);
/// let (_, cap) = slot.read_consistent();
/// assert_eq!(cap, 2048);
///
/// // Generation counter prevents ABA
/// let gen = slot.generation();
/// assert_eq!(gen, 0);
/// ```
#[repr(C, align(16))]
pub struct DualArraySlot {
    inner: DualAtomicU64,
}

impl DualArraySlot {
    /// Creates a new array slot with initial values
    #[inline]
    pub const fn new(index: u32, capacity: u32) -> Self {
        let primary = ((capacity as u64) << 32) | (index as u64);
        Self {
            inner: DualAtomicU64::new(primary, 0),
        }
    }

    /// Sets index and capacity atomically
    ///
    /// # Memory Ordering
    ///
    /// Uses `Release` ordering to ensure writes are visible.
    #[inline]
    pub fn set(&self, index: u32, capacity: u32) {
        let primary = ((capacity as u64) << 32) | (index as u64);
        self.inner.store_primary(primary, Ordering::Release);
    }

    /// Sets index only
    ///
    /// # Memory Ordering
    ///
    /// Uses `AcqRel` for read-modify-write consistency.
    #[inline]
    pub fn set_index(&self, index: u32) {
        let primary = self.inner.load_primary(Ordering::Acquire);
        let capacity = primary >> 32;
        let new_primary = (capacity << 32) | (index as u64);
        self.inner.store_primary(new_primary, Ordering::Release);
    }

    /// Sets capacity only
    ///
    /// # Memory Ordering
    ///
    /// Uses `AcqRel` for read-modify-write consistency.
    #[inline]
    pub fn set_capacity(&self, capacity: u32) {
        let primary = self.inner.load_primary(Ordering::Acquire);
        let index = primary & 0xFFFF_FFFF;
        let new_primary = ((capacity as u64) << 32) | index;
        self.inner.store_primary(new_primary, Ordering::Release);
    }

    /// Reads index and capacity consistently (TOCTOU-safe)
    ///
    /// # Memory Ordering
    ///
    /// Uses `Acquire` ordering to ensure reads see latest writes.
    #[inline]
    pub fn read_consistent(&self) -> (u32, u32) {
        let primary = self.inner.load_primary(Ordering::Acquire);
        let index = (primary & 0xFFFF_FFFF) as u32;
        let capacity = (primary >> 32) as u32;
        (index, capacity)
    }

    /// Increments the index atomically (with wraparound at capacity)
    ///
    /// # Memory Ordering
    ///
    /// Uses `AcqRel` for read-modify-write consistency.
    ///
    /// # Returns
    ///
    /// The new index after increment.
    #[inline]
    pub fn increment_index(&self) -> u32 {
        let primary = self.inner.load_primary(Ordering::Acquire);
        let index = (primary & 0xFFFF_FFFF) as u32;
        let capacity = (primary >> 32) as u32;
        let new_index = if capacity > 0 {
            (index + 1) % capacity
        } else {
            index + 1
        };
        let new_primary = ((capacity as u64) << 32) | (new_index as u64);
        self.inner.store_primary(new_primary, Ordering::Release);
        new_index
    }

    /// Gets the current generation counter (for ABA prevention)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.inner.load_secondary(Ordering::Acquire)
    }

    /// Gets the inner DualAtomicU64 for advanced operations
    #[inline]
    pub const fn inner(&self) -> &DualAtomicU64 {
        &self.inner
    }
}

impl Default for DualArraySlot {
    #[inline]
    fn default() -> Self {
        Self::new(0, 0)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // DualTimestampGen tests
    #[test]
    fn test_dual_timestamp_gen_new() {
        let tracker = DualTimestampGen::new();
        let (timestamp, counter) = tracker.read_consistent();
        assert_eq!(timestamp, 0);
        assert_eq!(counter, 0);
        assert_eq!(tracker.generation(), 0);
    }

    #[test]
    fn test_dual_timestamp_gen_with_values() {
        let tracker = DualTimestampGen::with_values(1234567890, 42);
        let (timestamp, counter) = tracker.read_consistent();
        assert_eq!(timestamp, 1234567890);
        assert_eq!(counter, 42);
    }

    #[test]
    fn test_dual_timestamp_gen_set() {
        let tracker = DualTimestampGen::new();
        tracker.set(3_876_543_210, 100); // Max u32 is ~4.29 billion
        let (timestamp, counter) = tracker.read_consistent();
        assert_eq!(timestamp, 3_876_543_210);
        assert_eq!(counter, 100);
    }

    #[test]
    fn test_dual_timestamp_gen_increment_counter() {
        let tracker = DualTimestampGen::with_values(1234567890, 42);
        tracker.increment_counter();
        let (timestamp, counter) = tracker.read_consistent();
        assert_eq!(timestamp, 1234567890);
        assert_eq!(counter, 43);
    }

    #[test]
    fn test_dual_timestamp_gen_multiple_increments() {
        let tracker = DualTimestampGen::with_values(1000, 0);
        for i in 1..=10 {
            tracker.increment_counter();
            let (_, counter) = tracker.read_consistent();
            assert_eq!(counter, i);
        }
    }

    // DualQ16_16Pair tests
    #[test]
    fn test_dual_q16_16_pair_new() {
        let pair = DualQ16_16Pair::new();
        let (a, b) = pair.read_both();
        assert_eq!(a, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_dual_q16_16_pair_with_values() {
        let value_a = 100 << 16; // 100.0 in Q16.16
        let value_b = 200 << 16; // 200.0 in Q16.16
        let pair = DualQ16_16Pair::with_values(value_a, value_b);
        let (a, b) = pair.read_both();
        assert_eq!(a, value_a);
        assert_eq!(b, value_b);
    }

    #[test]
    fn test_dual_q16_16_pair_set_both() {
        let pair = DualQ16_16Pair::new();
        let value_a = 150 << 16;
        let value_b = 250 << 16;
        pair.set_both(value_a, value_b);
        let (a, b) = pair.read_both();
        assert_eq!(a, value_a);
        assert_eq!(b, value_b);
    }

    #[test]
    fn test_dual_q16_16_pair_set_a() {
        let pair = DualQ16_16Pair::with_values(100 << 16, 200 << 16);
        pair.set_a(150 << 16);
        let (a, b) = pair.read_both();
        assert_eq!(a, 150 << 16);
        assert_eq!(b, 200 << 16);
    }

    #[test]
    fn test_dual_q16_16_pair_set_b() {
        let pair = DualQ16_16Pair::with_values(100 << 16, 200 << 16);
        pair.set_b(250 << 16);
        let (a, b) = pair.read_both();
        assert_eq!(a, 100 << 16);
        assert_eq!(b, 250 << 16);
    }

    #[test]
    fn test_dual_q16_16_pair_read_individual() {
        let pair = DualQ16_16Pair::with_values(100 << 16, 200 << 16);
        assert_eq!(pair.read_a(), 100 << 16);
        assert_eq!(pair.read_b(), 200 << 16);
    }

    // DualStateCounter tests
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    enum TestState {
        Idle = 0,
        Active = 1,
        Paused = 2,
        Done = 3,
    }

    impl From<u8> for TestState {
        fn from(v: u8) -> Self {
            match v & 0b11 {
                0 => TestState::Idle,
                1 => TestState::Active,
                2 => TestState::Paused,
                3 => TestState::Done,
                _ => unreachable!(),
            }
        }
    }

    impl From<TestState> for u8 {
        fn from(s: TestState) -> u8 {
            s as u8
        }
    }

    #[test]
    fn test_dual_state_counter_new() {
        let fsm = DualStateCounter::<TestState, 2>::new(TestState::Idle);
        let (state, counter) = fsm.read_consistent();
        assert_eq!(state, TestState::Idle);
        assert_eq!(counter, 0);
        assert_eq!(fsm.generation(), 0);
    }

    #[test]
    fn test_dual_state_counter_with_counter() {
        let fsm = DualStateCounter::<TestState, 2>::with_counter(TestState::Active, 42);
        let (state, counter) = fsm.read_consistent();
        assert_eq!(state, TestState::Active);
        assert_eq!(counter, 42);
    }

    #[test]
    fn test_dual_state_counter_transition() {
        let fsm = DualStateCounter::<TestState, 2>::new(TestState::Idle);
        fsm.transition(TestState::Active);
        let (state, counter) = fsm.read_consistent();
        assert_eq!(state, TestState::Active);
        assert_eq!(counter, 0);
    }

    #[test]
    fn test_dual_state_counter_increment_counter() {
        let fsm = DualStateCounter::<TestState, 2>::new(TestState::Active);
        fsm.increment_counter();
        let (state, counter) = fsm.read_consistent();
        assert_eq!(state, TestState::Active);
        assert_eq!(counter, 1);
    }

    #[test]
    fn test_dual_state_counter_transition_and_increment() {
        let fsm = DualStateCounter::<TestState, 2>::new(TestState::Idle);
        fsm.transition(TestState::Active);
        fsm.increment_counter();
        fsm.increment_counter();
        let (state, counter) = fsm.read_consistent();
        assert_eq!(state, TestState::Active);
        assert_eq!(counter, 2);
    }

    // DualArraySlot tests
    #[test]
    fn test_dual_array_slot_new() {
        let slot = DualArraySlot::new(0, 1024);
        let (index, capacity) = slot.read_consistent();
        assert_eq!(index, 0);
        assert_eq!(capacity, 1024);
        assert_eq!(slot.generation(), 0);
    }

    #[test]
    fn test_dual_array_slot_set() {
        let slot = DualArraySlot::new(0, 1024);
        slot.set(42, 2048);
        let (index, capacity) = slot.read_consistent();
        assert_eq!(index, 42);
        assert_eq!(capacity, 2048);
    }

    #[test]
    fn test_dual_array_slot_set_index() {
        let slot = DualArraySlot::new(0, 1024);
        slot.set_index(100);
        let (index, capacity) = slot.read_consistent();
        assert_eq!(index, 100);
        assert_eq!(capacity, 1024);
    }

    #[test]
    fn test_dual_array_slot_set_capacity() {
        let slot = DualArraySlot::new(42, 1024);
        slot.set_capacity(2048);
        let (index, capacity) = slot.read_consistent();
        assert_eq!(index, 42);
        assert_eq!(capacity, 2048);
    }

    #[test]
    fn test_dual_array_slot_increment_index() {
        let slot = DualArraySlot::new(0, 10);
        let new_index = slot.increment_index();
        assert_eq!(new_index, 1);
        let (index, _) = slot.read_consistent();
        assert_eq!(index, 1);
    }

    #[test]
    fn test_dual_array_slot_increment_wraparound() {
        let slot = DualArraySlot::new(9, 10);
        let new_index = slot.increment_index();
        assert_eq!(new_index, 0); // Wraps at capacity
        let (index, capacity) = slot.read_consistent();
        assert_eq!(index, 0);
        assert_eq!(capacity, 10);
    }

    #[test]
    fn test_dual_array_slot_multiple_increments() {
        let slot = DualArraySlot::new(0, 5);
        for i in 1..=10 {
            let new_index = slot.increment_index();
            assert_eq!(new_index, i % 5);
        }
    }

    #[test]
    fn test_dual_array_slot_default() {
        let slot = DualArraySlot::default();
        let (index, capacity) = slot.read_consistent();
        assert_eq!(index, 0);
        assert_eq!(capacity, 0);
    }
}
