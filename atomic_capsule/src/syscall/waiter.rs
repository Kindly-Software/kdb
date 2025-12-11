//! # Waiter Capsule - Individual Thread Wait State
//!
//! **UCE34 T1 Atomic: Per-thread wait state tracking for futex operations**
//!
//! ## Design
//!
//! Each waiter represents a single thread blocked on a futex. The waiter
//! capsule tracks:
//! - Thread identifier for wakeup targeting
//! - Wait state (Waiting, Woken, Interrupted, TimedOut)
//! - Bitset mask for selective wake (FUTEX_WAIT_BITSET)
//! - Generation counter for ABA prevention
//!
//! ## Layout (64 bytes, cache-line aligned)
//!
//! ```text
//! +--------+--------+--------+--------+--------+--------+--------+--------+
//! |  state (8B)     | thread_id (8B)  | bitset (4B)     | gen (4B)       |
//! +--------+--------+--------+--------+--------+--------+--------+--------+
//! | futex_addr (8B) | enqueue_ns (8B) | wake_token (8B) | next (8B)      |
//! +--------+--------+--------+--------+--------+--------+--------+--------+
//! ```
//!
//! ## State Machine
//!
//! ```text
//! [Created] --enqueue--> [Waiting] --wake--> [Woken]
//!                            |
//!                            +--signal--> [Interrupted]
//!                            |
//!                            +--timeout--> [TimedOut]
//!                            |
//!                            +--requeue--> [Requeued] --wake--> [Woken]
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_WAITER_UNIQUE`: Each waiter has unique thread_id
//! - `#VERIFY_WAITER_UNIQUE`: Enforced by scheduler thread creation
//! - `#ASSUME_STATE_ATOMIC`: State transitions are atomic
//! - `#VERIFY_STATE_ATOMIC`: Single AtomicU64 for packed state

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Unique waiter identifier
///
/// # Layout
/// - High 32 bits: Generation counter (ABA prevention)
/// - Low 32 bits: Index in waiter pool
///
/// # ASSUM Framework
/// - `#ASSUME_WAITER_ID_UNIQUE`: High bits prevent ABA
/// - `#VERIFY_WAITER_ID_UNIQUE`: Generation increments on each allocation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct WaiterId(pub u64);

impl WaiterId {
    /// Create new waiter ID
    ///
    /// # Arguments
    /// - `generation`: ABA prevention counter
    /// - `index`: Pool index
    #[inline]
    pub const fn new(generation: u32, index: u32) -> Self {
        Self(((generation as u64) << 32) | (index as u64))
    }

    /// Get generation counter
    #[inline]
    pub const fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Get pool index
    #[inline]
    pub const fn index(self) -> u32 {
        self.0 as u32
    }

    /// Invalid waiter ID (sentinel)
    pub const INVALID: Self = Self(u64::MAX);

    /// Check if ID is valid
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u64::MAX
    }
}

/// Waiter state enumeration
///
/// # ASSUM Framework
/// - `#ASSUME_STATE_FITS_8BITS`: State values fit in 8 bits
/// - `#VERIFY_STATE_FITS_8BITS`: Enum is repr(u8)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WaiterState {
    /// Initial state, not yet enqueued
    Created = 0,

    /// Enqueued and waiting for wake
    Waiting = 1,

    /// Successfully woken by FUTEX_WAKE
    Woken = 2,

    /// Interrupted by signal (EINTR)
    Interrupted = 3,

    /// Wait timed out (ETIMEDOUT)
    TimedOut = 4,

    /// Requeued to different futex
    Requeued = 5,

    /// Removed from queue (cancelled)
    Cancelled = 6,
}

impl WaiterState {
    /// Check if waiter should stop waiting
    #[inline]
    pub const fn should_wake(self) -> bool {
        !matches!(self, Self::Created | Self::Waiting | Self::Requeued)
    }

    /// Check if waiter is still active
    #[inline]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Waiting | Self::Requeued)
    }

    /// Convert to u8 for packing
    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Convert from u8
    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Created,
            1 => Self::Waiting,
            2 => Self::Woken,
            3 => Self::Interrupted,
            4 => Self::TimedOut,
            5 => Self::Requeued,
            6 => Self::Cancelled,
            _ => Self::Created, // Default to Created for invalid values
        }
    }
}

/// Packed waiter state for atomic operations
///
/// # Layout (8 bytes)
/// - Bits 0-7: WaiterState (8 states)
/// - Bits 8-15: Reserved
/// - Bits 16-31: Wake reason/flags
/// - Bits 32-63: Generation counter
///
/// # ASSUM Framework
/// - `#ASSUME_PACKED_ATOMIC`: 8-byte value for single atomic load/store
/// - `#VERIFY_PACKED_ATOMIC`: AtomicU64 operations are lock-free on x86_64/aarch64
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PackedWaiterState(pub u64);

impl PackedWaiterState {
    /// Create packed state
    #[inline]
    pub const fn new(state: WaiterState, flags: u16, generation: u32) -> Self {
        let v = (state.to_u8() as u64)
            | ((flags as u64) << 16)
            | ((generation as u64) << 32);
        Self(v)
    }

    /// Extract state
    #[inline]
    pub const fn state(self) -> WaiterState {
        WaiterState::from_u8((self.0 & 0xFF) as u8)
    }

    /// Extract flags
    #[inline]
    pub const fn flags(self) -> u16 {
        ((self.0 >> 16) & 0xFFFF) as u16
    }

    /// Extract generation
    #[inline]
    pub const fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Update state preserving generation
    #[inline]
    pub const fn with_state(self, state: WaiterState) -> Self {
        let v = (self.0 & !0xFF) | (state.to_u8() as u64);
        Self(v)
    }

    /// Increment generation
    #[inline]
    pub const fn next_generation(self) -> Self {
        let gen = self.generation().wrapping_add(1);
        Self::new(self.state(), self.flags(), gen)
    }
}

/// Waiter capsule - represents a single waiting thread
///
/// # Layout (64 bytes, cache-line aligned)
///
/// # Performance Targets (B32)
/// - State load: <5ns (Acquire)
/// - State CAS: <15ns (AcqRel)
/// - Full snapshot: <10ns (3 loads)
///
/// # ASSUM Framework
/// - `#ASSUME_CACHE_ALIGNED`: 64-byte alignment prevents false sharing
/// - `#VERIFY_CACHE_ALIGNED`: repr(C, align(64)) enforced at compile time
/// - `#ASSUME_NO_TEARNING`: AtomicU64 prevents partial reads on state
/// - `#VERIFY_NO_TEARNING`: x86_64/aarch64 guarantee atomic 8-byte access
#[repr(C, align(64))]
pub struct WaiterCapsule {
    /// Packed state (state + flags + generation)
    ///
    /// # Memory Ordering
    /// - Load: Acquire (synchronize with state transitions)
    /// - Store: Release (publish state changes)
    /// - CAS: AcqRel (atomic state machine transitions)
    ///
    /// # ASSUM_STATE_ORDERING
    /// - Acquire on load synchronizes with Release from wake
    /// - AcqRel on CAS provides total order on transitions
    state: AtomicU64,

    /// Thread/task identifier for scheduler wakeup
    ///
    /// # ASSUM_THREAD_ID_IMMUTABLE
    /// - Set once during creation, never modified
    /// - Safe to read with Relaxed ordering after initial setup
    pub thread_id: AtomicU64,

    /// Bitset mask for FUTEX_WAIT_BITSET/FUTEX_WAKE_BITSET
    ///
    /// Default: FUTEX_BITSET_MATCH_ANY (0xFFFFFFFF)
    ///
    /// # ASSUM_BITSET_IMMUTABLE
    /// - Set during FUTEX_WAIT_BITSET, not modified during wait
    pub bitset: u32,

    /// Generation counter for this waiter slot
    ///
    /// # ASSUM_GENERATION_MONOTONIC
    /// - Increments on each reuse of waiter slot
    /// - Prevents ABA problem in lockfree queue
    pub slot_generation: u32,

    /// Futex address being waited on
    ///
    /// # ASSUM_ADDRESS_STABLE
    /// - Set during enqueue, not modified until dequeue
    pub futex_addr: AtomicU64,

    /// Enqueue timestamp (nanoseconds since epoch)
    ///
    /// Used for timeout checking and statistics
    pub enqueue_ns: AtomicU64,

    /// Wake token (for scheduler integration)
    ///
    /// # Purpose
    /// - Passed to scheduler for thread wakeup
    /// - Opaque value, meaning depends on scheduler
    pub wake_token: AtomicU64,

    /// Next pointer for intrusive queue
    ///
    /// # Layout
    /// - High 32 bits: Generation
    /// - Low 32 bits: Next waiter index (or INVALID)
    ///
    /// # ASSUM_NEXT_LOCKFREE
    /// - Packed generation prevents ABA in CAS operations
    /// - AtomicUsize for platform-native pointer size
    pub next: AtomicUsize,
}

// ASSUM_LAYOUT_VERIFIED: Compile-time size check
const _: () = {
    assert!(core::mem::size_of::<WaiterCapsule>() == 64);
    assert!(core::mem::align_of::<WaiterCapsule>() == 64);
};

impl WaiterCapsule {
    /// Create new waiter in Created state
    ///
    /// # Arguments
    /// - `thread_id`: Thread identifier for wakeup
    /// - `slot_generation`: Generation for ABA prevention
    #[inline]
    pub const fn new(thread_id: u64, slot_generation: u32) -> Self {
        Self {
            state: AtomicU64::new(PackedWaiterState::new(WaiterState::Created, 0, 0).0),
            thread_id: AtomicU64::new(thread_id),
            bitset: 0xFFFF_FFFF, // FUTEX_BITSET_MATCH_ANY
            slot_generation,
            futex_addr: AtomicU64::new(0),
            enqueue_ns: AtomicU64::new(0),
            wake_token: AtomicU64::new(0),
            next: AtomicUsize::new(usize::MAX),
        }
    }

    /// Initialize waiter for a new wait operation
    ///
    /// # Arguments
    /// - `futex_addr`: Address of futex word
    /// - `bitset`: Bitset mask (default: 0xFFFFFFFF for all)
    /// - `enqueue_ns`: Current timestamp in nanoseconds
    /// - `wake_token`: Scheduler wakeup token
    ///
    /// # Safety
    /// Must be called before enqueueing to futex queue
    ///
    /// # ASSUM_INIT_ORDER
    /// - All fields must be set before state transition to Waiting
    /// - Release barrier in transition_to_waiting publishes all fields
    pub fn initialize(&self, futex_addr: u64, bitset: u32, enqueue_ns: u64, wake_token: u64) {
        // Store fields with Relaxed - they will be visible after state transition
        self.futex_addr.store(futex_addr, Ordering::Relaxed);
        self.enqueue_ns.store(enqueue_ns, Ordering::Relaxed);
        self.wake_token.store(wake_token, Ordering::Relaxed);
        self.next.store(usize::MAX, Ordering::Relaxed);

        // Note: bitset is not atomic, must be set before any concurrent access
        // This is safe because waiter is not visible to other threads until enqueued
    }

    /// Load current state with Acquire ordering
    ///
    /// # Returns
    /// Current packed state
    #[inline]
    pub fn load_state(&self) -> PackedWaiterState {
        PackedWaiterState(self.state.load(Ordering::Acquire))
    }

    /// Load current state enum only
    #[inline]
    pub fn state(&self) -> WaiterState {
        self.load_state().state()
    }

    /// Transition from Created to Waiting
    ///
    /// # Returns
    /// true if transition succeeded, false if already in different state
    ///
    /// # Memory Ordering
    /// Release: Publishes all initialized fields
    ///
    /// # ASSUM_CREATED_TO_WAITING
    /// - Only valid transition from Created state
    /// - Single producer (the waiting thread itself)
    pub fn transition_to_waiting(&self) -> bool {
        let current = self.load_state();
        if current.state() != WaiterState::Created {
            return false;
        }

        let next = current.with_state(WaiterState::Waiting);
        self.state
            .compare_exchange(current.0, next.0, Ordering::Release, Ordering::Relaxed)
            .is_ok()
    }

    /// Attempt to wake this waiter
    ///
    /// # Arguments
    /// - `wake_bitset`: Bitset to match against waiter's bitset
    ///
    /// # Returns
    /// true if waiter was woken, false if already woken or bitset doesn't match
    ///
    /// # Memory Ordering
    /// AcqRel: Synchronizes with waiter's subsequent state check
    ///
    /// # ASSUM_WAKE_ATOMIC
    /// - CAS ensures exactly one waker succeeds
    /// - Prevents double-wake race condition
    pub fn try_wake(&self, wake_bitset: u32) -> bool {
        // Check bitset match (non-atomic read is safe - bitset is immutable after init)
        if self.bitset & wake_bitset == 0 {
            return false;
        }

        let current = self.load_state();
        if !current.state().is_active() {
            return false;
        }

        let next = current.with_state(WaiterState::Woken);
        self.state
            .compare_exchange(current.0, next.0, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    /// Attempt to interrupt waiter (for signal delivery)
    ///
    /// # Returns
    /// true if waiter was interrupted, false if already woken
    pub fn try_interrupt(&self) -> bool {
        let current = self.load_state();
        if !current.state().is_active() {
            return false;
        }

        let next = current.with_state(WaiterState::Interrupted);
        self.state
            .compare_exchange(current.0, next.0, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    /// Attempt to timeout waiter
    ///
    /// # Returns
    /// true if waiter was timed out, false if already woken
    pub fn try_timeout(&self) -> bool {
        let current = self.load_state();
        if !current.state().is_active() {
            return false;
        }

        let next = current.with_state(WaiterState::TimedOut);
        self.state
            .compare_exchange(current.0, next.0, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    /// Attempt to requeue waiter to different futex
    ///
    /// # Arguments
    /// - `new_futex_addr`: New futex address to wait on
    ///
    /// # Returns
    /// true if waiter was requeued, false if not in Waiting state
    pub fn try_requeue(&self, new_futex_addr: u64) -> bool {
        let current = self.load_state();
        if current.state() != WaiterState::Waiting {
            return false;
        }

        let next = current.with_state(WaiterState::Requeued);
        if self
            .state
            .compare_exchange(current.0, next.0, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            // Update futex address after successful state transition
            self.futex_addr.store(new_futex_addr, Ordering::Release);

            // Transition back to Waiting on new futex
            let requeued = self.load_state();
            let waiting = requeued.with_state(WaiterState::Waiting);
            let _ = self.state.compare_exchange(
                requeued.0,
                waiting.0,
                Ordering::Release,
                Ordering::Relaxed,
            );
            true
        } else {
            false
        }
    }

    /// Cancel waiter (remove from queue without waking)
    ///
    /// # Returns
    /// true if waiter was cancelled, false if already in terminal state
    pub fn try_cancel(&self) -> bool {
        let current = self.load_state();
        if current.state().should_wake() {
            return false;
        }

        let next = current.with_state(WaiterState::Cancelled);
        self.state
            .compare_exchange(current.0, next.0, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    /// Reset waiter for pool reuse
    ///
    /// # Safety
    /// Must only be called when waiter is not in any queue
    ///
    /// # ASSUM_RESET_EXCLUSIVE
    /// - Caller must ensure exclusive access (no concurrent readers)
    /// - Called only by pool allocator after waiter is dequeued
    pub fn reset(&self, new_generation: u32) {
        self.state.store(
            PackedWaiterState::new(WaiterState::Created, 0, new_generation).0,
            Ordering::Relaxed,
        );
        self.futex_addr.store(0, Ordering::Relaxed);
        self.enqueue_ns.store(0, Ordering::Relaxed);
        self.wake_token.store(0, Ordering::Relaxed);
        self.next.store(usize::MAX, Ordering::Relaxed);
    }

    /// Get thread ID
    #[inline]
    pub fn thread_id(&self) -> u64 {
        self.thread_id.load(Ordering::Relaxed)
    }

    /// Get futex address
    #[inline]
    pub fn futex_addr(&self) -> u64 {
        self.futex_addr.load(Ordering::Acquire)
    }

    /// Get enqueue timestamp
    #[inline]
    pub fn enqueue_ns(&self) -> u64 {
        self.enqueue_ns.load(Ordering::Relaxed)
    }

    /// Get wake token
    #[inline]
    pub fn wake_token(&self) -> u64 {
        self.wake_token.load(Ordering::Relaxed)
    }

    /// Check if bitset matches
    #[inline]
    pub fn matches_bitset(&self, wake_bitset: u32) -> bool {
        self.bitset & wake_bitset != 0
    }
}

// Safety: WaiterCapsule is Send + Sync (all fields are atomic or immutable after init)
unsafe impl Send for WaiterCapsule {}
unsafe impl Sync for WaiterCapsule {}

impl core::fmt::Debug for WaiterCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WaiterCapsule")
            .field("state", &self.state())
            .field("thread_id", &self.thread_id())
            .field("bitset", &format_args!("{:#010x}", self.bitset))
            .field("slot_generation", &self.slot_generation)
            .field("futex_addr", &format_args!("{:#x}", self.futex_addr()))
            .finish()
    }
}

/// Default bitset for FUTEX_WAIT (matches all)
pub const FUTEX_BITSET_MATCH_ANY: u32 = 0xFFFF_FFFF;
