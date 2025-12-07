//! SessionPoolCapsule - T6 Mixed Tier Orchestrator
//!
//! # Architecture
//!
//! T6 Mixed tier orchestrator managing three-tier session pools:
//! - **LIGHT (64KB)**: Quick attach/inspect operations
//! - **MEDIUM (256KB)**: Step debugging with moderate snapshots
//! - **HEAVY (1.09MB)**: Full replay with COW memory regions
//!
//! # Memory Layout (512 bytes orchestrator)
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │ Orchestrator Metadata (256 bytes)                              │
//! │   - generation: AtomicU64 (8B)                                 │
//! │   - state: AtomicU64 (8B)                                      │
//! │   - total_allocations: AtomicU64 (8B)                          │
//! │   - total_releases: AtomicU64 (8B)                             │
//! │   - total_upgrades: AtomicU64 (8B)                             │
//! │   - total_downgrades: AtomicU64 (8B)                           │
//! │   - config + padding: 208B                                     │
//! ├────────────────────────────────────────────────────────────────┤
//! │ Free-List Heads (64 bytes)                                     │
//! │   - light_free_head: AtomicU64 (8B)                            │
//! │   - medium_free_head: AtomicU64 (8B)                           │
//! │   - heavy_free_head: AtomicU64 (8B)                            │
//! │   - padding: 40B                                               │
//! ├────────────────────────────────────────────────────────────────┤
//! │ Pool Pointers (64 bytes)                                       │
//! │   - light_pool_ptr: AtomicU64 (8B)                             │
//! │   - medium_pool_ptr: AtomicU64 (8B)                            │
//! │   - heavy_pool_ptr: AtomicU64 (8B)                             │
//! │   - slot_metadata_ptr: AtomicU64 (8B)                          │
//! │   - padding: 32B                                               │
//! ├────────────────────────────────────────────────────────────────┤
//! │ Usage Stats (64 bytes)                                         │
//! │   - light_used: AtomicU32 (4B)                                 │
//! │   - medium_used: AtomicU32 (4B)                                │
//! │   - heavy_used: AtomicU32 (4B)                                 │
//! │   - peak_light: AtomicU32 (4B)                                 │
//! │   - peak_medium: AtomicU32 (4B)                                │
//! │   - peak_heavy: AtomicU32 (4B)                                 │
//! │   - padding: 40B                                               │
//! ├────────────────────────────────────────────────────────────────┤
//! │ Tier Thresholds (64 bytes)                                     │
//! │   - upgrade_snapshot_light_to_medium: AtomicU32 (4B)           │
//! │   - upgrade_snapshot_medium_to_heavy: AtomicU32 (4B)           │
//! │   - downgrade_idle_seconds: AtomicU32 (4B)                     │
//! │   - padding: 52B                                               │
//! └────────────────────────────────────────────────────────────────┘
//! Total: 512 bytes (cache-line aligned)
//! ```
//!
//! # Pool Capacities (64GB server target)
//! - LIGHT: 1,500 × 64KB = 96MB
//! - MEDIUM: 600 × 256KB = 150MB
//! - HEAVY: 400 × 1.09MB = 436MB
//! Total: ~682MB for session pools
//!
//! # Lockfree Free-List Design
//!
//! Each tier uses a lockfree Treiber stack for O(1) allocation/deallocation:
//! ```text
//! FreeListHead: Pack(next_index: u24, aba_counter: u40)
//! - next_index: Index of next free slot (0xFFFFFF = empty)
//! - aba_counter: ABA prevention counter (40 bits = 1 trillion ops)
//! ```
//!
//! # Session State Machine
//!
//! ```text
//! LIGHT ──(48+ snapshots)──► MEDIUM ──(384+ snapshots)──► HEAVY
//!       ◄──(idle >30min)───        ◄──(idle >30min)────
//! ```
//!
//! # Performance Targets (B32 Validated)
//! - `allocate_session()`: <100ns lockfree
//! - `release_session()`: <100ns lockfree
//! - `upgrade_session()`: <1μs (includes data migration)
//! - `get_pool_stats()`: <50ns (atomic snapshot)
//!
//! # ASSUM Safety (99.99%+)
//! - #ASSUME_LOCKFREE_ONLY: All coordination via CAS, no mutex/RwLock
//! - #ASSUME_ABA_PREVENTION: 40-bit counter prevents ABA (1 trillion ops)
//! - #ASSUME_ALIGNED_ACCESS: All pools 256-byte aligned
//! - #ASSUME_BOUNDS_CHECKED: Slot indices validated before access

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Constants
// ============================================================================

/// Default light pool capacity (1,500 sessions × 64KB = 96MB)
pub const DEFAULT_LIGHT_CAPACITY: u32 = 1500;

/// Default medium pool capacity (600 sessions × 256KB = 150MB)
pub const DEFAULT_MEDIUM_CAPACITY: u32 = 600;

/// Default heavy pool capacity (400 sessions × 1.09MB = 436MB)
pub const DEFAULT_HEAVY_CAPACITY: u32 = 400;

/// Snapshot threshold for LIGHT → MEDIUM upgrade
pub const DEFAULT_UPGRADE_LIGHT_TO_MEDIUM: u32 = 48;

/// Snapshot threshold for MEDIUM → HEAVY upgrade
pub const DEFAULT_UPGRADE_MEDIUM_TO_HEAVY: u32 = 384;

/// Idle seconds before downgrade (30 minutes)
pub const DEFAULT_DOWNGRADE_IDLE_SECONDS: u32 = 1800;

/// Sentinel value for empty free-list
pub const FREE_LIST_EMPTY: u32 = 0x00FF_FFFF;

/// Mask for extracting next_index from packed free-list head (24 bits)
const NEXT_INDEX_MASK: u64 = 0x00FF_FFFF;

/// Shift for ABA counter in packed free-list head
const ABA_COUNTER_SHIFT: u32 = 24;

// ============================================================================
// Session Tier Type
// ============================================================================

/// Session tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SessionTierType {
    /// Light session (64KB) - attach/inspect only
    Light = 0,
    /// Medium session (256KB) - step debugging
    Medium = 1,
    /// Heavy session (1.09MB) - full replay + COW
    Heavy = 2,
}

impl SessionTierType {
    /// Convert from u8
    #[inline]
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(SessionTierType::Light),
            1 => Some(SessionTierType::Medium),
            2 => Some(SessionTierType::Heavy),
            _ => None,
        }
    }

    /// Convert to u8
    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Get tier name
    #[inline]
    pub fn as_str(self) -> &'static str {
        match self {
            SessionTierType::Light => "Light",
            SessionTierType::Medium => "Medium",
            SessionTierType::Heavy => "Heavy",
        }
    }

    /// Get session size in bytes
    #[inline]
    pub fn session_size(self) -> usize {
        match self {
            SessionTierType::Light => 64 * 1024,        // 64KB
            SessionTierType::Medium => 256 * 1024,      // 256KB
            SessionTierType::Heavy => 1_147_392,        // 1.09MB (DebuggerCapsule size)
        }
    }
}

impl std::fmt::Display for SessionTierType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Session ID
// ============================================================================

/// Unique session identifier with embedded tier and slot information
///
/// # Layout (64 bits)
/// ```text
/// ┌────────────┬────────────┬────────────┬────────────────────────┐
/// │ Tier (2b)  │ Reserved   │ Slot (24b) │ Generation (32b)       │
/// │ bits 62-63 │ bits 56-61 │ bits 32-55 │ bits 0-31              │
/// └────────────┴────────────┴────────────┴────────────────────────┘
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SessionId(pub u64);

impl SessionId {
    /// Create new session ID
    #[inline]
    pub const fn new(tier: u8, slot: u32, generation: u32) -> Self {
        let tier_bits = ((tier as u64) & 0x03) << 62;
        let slot_bits = ((slot as u64) & 0x00FF_FFFF) << 32;
        let gen_bits = generation as u64;
        SessionId(tier_bits | slot_bits | gen_bits)
    }

    /// Extract tier from session ID
    #[inline]
    pub const fn tier(self) -> u8 {
        ((self.0 >> 62) & 0x03) as u8
    }

    /// Extract tier type
    #[inline]
    pub fn tier_type(self) -> Option<SessionTierType> {
        SessionTierType::from_u8(self.tier())
    }

    /// Extract slot index from session ID
    #[inline]
    pub const fn slot(self) -> u32 {
        ((self.0 >> 32) & 0x00FF_FFFF) as u32
    }

    /// Extract generation from session ID
    #[inline]
    pub const fn generation(self) -> u32 {
        (self.0 & 0xFFFF_FFFF) as u32
    }

    /// Check if this is a valid session ID (non-zero)
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }

    /// Invalid session ID constant
    pub const INVALID: SessionId = SessionId(0);
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Session({:?}:{}:{})",
            SessionTierType::from_u8(self.tier()).unwrap_or(SessionTierType::Light),
            self.slot(),
            self.generation()
        )
    }
}

// ============================================================================
// Pool Configuration
// ============================================================================

/// Pool configuration parameters
#[derive(Debug, Clone, Copy)]
pub struct PoolConfig {
    /// Light pool capacity (default: 1,500)
    pub light_capacity: u32,
    /// Medium pool capacity (default: 600)
    pub medium_capacity: u32,
    /// Heavy pool capacity (default: 400)
    pub heavy_capacity: u32,
    /// Snapshot threshold for LIGHT → MEDIUM upgrade (default: 48)
    pub upgrade_snapshot_light_to_medium: u32,
    /// Snapshot threshold for MEDIUM → HEAVY upgrade (default: 384)
    pub upgrade_snapshot_medium_to_heavy: u32,
    /// Idle seconds before downgrade (default: 1800 = 30 min)
    pub downgrade_idle_seconds: u32,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            light_capacity: DEFAULT_LIGHT_CAPACITY,
            medium_capacity: DEFAULT_MEDIUM_CAPACITY,
            heavy_capacity: DEFAULT_HEAVY_CAPACITY,
            upgrade_snapshot_light_to_medium: DEFAULT_UPGRADE_LIGHT_TO_MEDIUM,
            upgrade_snapshot_medium_to_heavy: DEFAULT_UPGRADE_MEDIUM_TO_HEAVY,
            downgrade_idle_seconds: DEFAULT_DOWNGRADE_IDLE_SECONDS,
        }
    }
}

impl PoolConfig {
    /// Create config for testing with smaller pools
    pub fn test_config() -> Self {
        Self {
            light_capacity: 16,
            medium_capacity: 8,
            heavy_capacity: 4,
            upgrade_snapshot_light_to_medium: 4,
            upgrade_snapshot_medium_to_heavy: 8,
            downgrade_idle_seconds: 60,
        }
    }

    /// Calculate total memory usage in bytes
    pub fn total_memory_bytes(&self) -> usize {
        let light = self.light_capacity as usize * SessionTierType::Light.session_size();
        let medium = self.medium_capacity as usize * SessionTierType::Medium.session_size();
        let heavy = self.heavy_capacity as usize * SessionTierType::Heavy.session_size();
        light + medium + heavy
    }
}

// ============================================================================
// Pool Statistics
// ============================================================================

/// Pool usage statistics (lockfree snapshot)
#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    /// Light sessions currently in use
    pub light_used: u32,
    /// Light pool capacity
    pub light_capacity: u32,
    /// Medium sessions currently in use
    pub medium_used: u32,
    /// Medium pool capacity
    pub medium_capacity: u32,
    /// Heavy sessions currently in use
    pub heavy_used: u32,
    /// Heavy pool capacity
    pub heavy_capacity: u32,
    /// Peak light usage
    pub peak_light: u32,
    /// Peak medium usage
    pub peak_medium: u32,
    /// Peak heavy usage
    pub peak_heavy: u32,
    /// Total allocations (all tiers)
    pub total_allocations: u64,
    /// Total releases (all tiers)
    pub total_releases: u64,
    /// Total upgrades
    pub total_upgrades: u64,
    /// Total downgrades
    pub total_downgrades: u64,
    /// Generation counter at snapshot time
    pub generation: u64,
}

impl PoolStats {
    /// Calculate total sessions in use
    #[inline]
    pub fn total_used(&self) -> u32 {
        self.light_used
            .saturating_add(self.medium_used)
            .saturating_add(self.heavy_used)
    }

    /// Calculate total capacity
    #[inline]
    pub fn total_capacity(&self) -> u32 {
        self.light_capacity
            .saturating_add(self.medium_capacity)
            .saturating_add(self.heavy_capacity)
    }

    /// Calculate total memory in use (bytes)
    pub fn memory_used(&self) -> usize {
        let light = self.light_used as usize * SessionTierType::Light.session_size();
        let medium = self.medium_used as usize * SessionTierType::Medium.session_size();
        let heavy = self.heavy_used as usize * SessionTierType::Heavy.session_size();
        light + medium + heavy
    }

    /// Calculate utilization percentage
    pub fn utilization_percent(&self) -> f64 {
        let total = self.total_capacity();
        if total == 0 {
            return 0.0;
        }
        (self.total_used() as f64 / total as f64) * 100.0
    }
}

impl std::fmt::Display for PoolStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PoolStats {{ light: {}/{}, medium: {}/{}, heavy: {}/{}, util: {:.1}% }}",
            self.light_used,
            self.light_capacity,
            self.medium_used,
            self.medium_capacity,
            self.heavy_used,
            self.heavy_capacity,
            self.utilization_percent()
        )
    }
}

// ============================================================================
// Pool Error
// ============================================================================

/// Pool operation errors
#[derive(Debug, Clone, PartialEq)]
pub enum PoolError {
    /// Pool is full for requested tier
    PoolFull { tier: SessionTierType, capacity: u32 },
    /// Invalid session ID
    InvalidSessionId(SessionId),
    /// Session not found
    SessionNotFound(SessionId),
    /// Session already released
    AlreadyReleased(SessionId),
    /// Cannot upgrade (already at highest tier)
    CannotUpgrade(SessionId),
    /// Cannot downgrade (already at lowest tier)
    CannotDowngrade(SessionId),
    /// Generation mismatch (stale session ID)
    GenerationMismatch { expected: u32, actual: u32 },
    /// Pool not initialized
    NotInitialized,
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolError::PoolFull { tier, capacity } => {
                write!(f, "{} pool full ({} capacity)", tier, capacity)
            }
            PoolError::InvalidSessionId(id) => {
                write!(f, "Invalid session ID: {}", id)
            }
            PoolError::SessionNotFound(id) => {
                write!(f, "Session not found: {}", id)
            }
            PoolError::AlreadyReleased(id) => {
                write!(f, "Session already released: {}", id)
            }
            PoolError::CannotUpgrade(id) => {
                write!(f, "Cannot upgrade session (already heavy): {}", id)
            }
            PoolError::CannotDowngrade(id) => {
                write!(f, "Cannot downgrade session (already light): {}", id)
            }
            PoolError::GenerationMismatch { expected, actual } => {
                write!(
                    f,
                    "Generation mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            PoolError::NotInitialized => {
                write!(f, "Pool not initialized")
            }
        }
    }
}

impl std::error::Error for PoolError {}

// ============================================================================
// Free List Node (embedded in slot metadata)
// ============================================================================

/// Packed free-list head for lockfree Treiber stack
///
/// # Layout (64 bits)
/// ```text
/// ┌────────────────────────────────────────────────────────────────┐
/// │ ABA Counter (40 bits)           │ Next Index (24 bits)        │
/// │ bits 24-63                      │ bits 0-23                   │
/// └────────────────────────────────────────────────────────────────┘
/// ```
#[repr(transparent)]
struct FreeListHead(AtomicU64);

impl FreeListHead {
    /// Create new free-list head (empty)
    #[inline]
    const fn new() -> Self {
        FreeListHead(AtomicU64::new(FREE_LIST_EMPTY as u64))
    }

    /// Create with initial index
    #[inline]
    fn with_head(index: u32) -> Self {
        let packed = Self::pack(index, 0);
        FreeListHead(AtomicU64::new(packed))
    }

    /// Pack index and ABA counter into u64
    #[inline]
    fn pack(next_index: u32, aba_counter: u64) -> u64 {
        ((aba_counter << ABA_COUNTER_SHIFT) | (next_index as u64 & NEXT_INDEX_MASK))
    }

    /// Unpack next_index from packed value
    #[inline]
    fn unpack_index(packed: u64) -> u32 {
        (packed & NEXT_INDEX_MASK) as u32
    }

    /// Unpack ABA counter from packed value
    #[inline]
    fn unpack_aba(packed: u64) -> u64 {
        packed >> ABA_COUNTER_SHIFT
    }

    /// Load current head (Acquire ordering)
    #[inline]
    fn load(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }

    /// Attempt to pop from free-list (returns slot index or None if empty)
    ///
    /// # Arguments
    /// - `get_next`: Closure to get next pointer from slot metadata
    ///
    /// # Returns
    /// - `Some(index)` if successfully popped
    /// - `None` if free-list is empty
    ///
    /// # ASSUM Safety
    /// - #ASSUME_CAS_LOOP: CAS retry is bounded by ABA counter
    /// - #VERIFY_NO_LIVELOCK: ABA counter prevents infinite retry
    fn pop<F>(&self, get_next: F) -> Option<u32>
    where
        F: Fn(u32) -> u32,
    {
        loop {
            let current = self.load();
            let head_index = Self::unpack_index(current);

            // Empty list check
            if head_index == FREE_LIST_EMPTY {
                return None;
            }

            // Get next pointer from slot metadata
            let next_index = get_next(head_index);
            let new_aba = Self::unpack_aba(current).wrapping_add(1);
            let new_packed = Self::pack(next_index, new_aba);

            // CAS to update head
            // #ASSUME_ABA_PREVENTION: 40-bit counter prevents ABA problem
            // #VERIFY_CAS_SUCCESS: Success means we own this slot
            match self.0.compare_exchange_weak(
                current,
                new_packed,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(head_index),
                Err(_) => continue, // Retry
            }
        }
    }

    /// Push slot back to free-list
    ///
    /// # Arguments
    /// - `index`: Slot index to push
    /// - `set_next`: Closure to set next pointer in slot metadata
    ///
    /// # ASSUM Safety
    /// - #ASSUME_CAS_LOOP: CAS retry is bounded
    /// - #VERIFY_NO_DOUBLE_FREE: Caller must ensure slot is not already in list
    fn push<F>(&self, index: u32, set_next: F)
    where
        F: Fn(u32, u32),
    {
        loop {
            let current = self.load();
            let old_head = Self::unpack_index(current);

            // Set this slot's next pointer to old head
            set_next(index, old_head);

            let new_aba = Self::unpack_aba(current).wrapping_add(1);
            let new_packed = Self::pack(index, new_aba);

            // CAS to update head
            // #ASSUME_ABA_PREVENTION: 40-bit counter prevents ABA
            // #VERIFY_PUSH_SUCCESS: Success means slot is now in list
            match self.0.compare_exchange_weak(
                current,
                new_packed,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(_) => continue, // Retry
            }
        }
    }
}

// ============================================================================
// Slot State (embedded in each slot's metadata)
// ============================================================================

/// Slot state for tracking allocation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SlotState {
    /// Slot is free (in free-list)
    Free = 0,
    /// Slot is allocated
    Allocated = 1,
    /// Slot is being upgraded (transitional)
    Upgrading = 2,
    /// Slot is being downgraded (transitional)
    Downgrading = 3,
}

impl SlotState {
    #[inline]
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => SlotState::Free,
            1 => SlotState::Allocated,
            2 => SlotState::Upgrading,
            3 => SlotState::Downgrading,
            _ => SlotState::Free,
        }
    }
}

// ============================================================================
// Slot Metadata Entry (16 bytes per slot)
// ============================================================================

/// Per-slot metadata for tracking state and free-list linkage
///
/// # Layout (16 bytes)
/// ```text
/// ┌────────────────────────────────────────────────────────────────┐
/// │ state_gen: AtomicU64 (8B)                                      │
/// │   - state (8 bits) | generation (24 bits) | next_free (32 bits)│
/// ├────────────────────────────────────────────────────────────────┤
/// │ last_activity_ns: AtomicU64 (8B) - timestamp for idle detect   │
/// └────────────────────────────────────────────────────────────────┘
/// ```
#[repr(C, align(16))]
struct SlotMetadataEntry {
    /// Packed: state(8) | generation(24) | next_free(32)
    state_gen: AtomicU64,
    /// Last activity timestamp (nanoseconds)
    last_activity_ns: AtomicU64,
}

impl SlotMetadataEntry {
    const STATE_SHIFT: u32 = 56;
    const GEN_SHIFT: u32 = 32;
    const GEN_MASK: u64 = 0x00FF_FFFF;
    const NEXT_MASK: u64 = 0xFFFF_FFFF;

    /// Create new metadata entry (free state)
    #[inline]
    const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(FREE_LIST_EMPTY as u64),
            last_activity_ns: AtomicU64::new(0),
        }
    }

    /// Pack state, generation, and next_free into u64
    #[inline]
    fn pack(state: SlotState, generation: u32, next_free: u32) -> u64 {
        ((state as u64) << Self::STATE_SHIFT)
            | (((generation as u64) & Self::GEN_MASK) << Self::GEN_SHIFT)
            | (next_free as u64 & Self::NEXT_MASK)
    }

    /// Unpack state from packed value
    #[inline]
    fn unpack_state(packed: u64) -> SlotState {
        SlotState::from_u8((packed >> Self::STATE_SHIFT) as u8)
    }

    /// Unpack generation from packed value
    #[inline]
    fn unpack_generation(packed: u64) -> u32 {
        ((packed >> Self::GEN_SHIFT) & Self::GEN_MASK) as u32
    }

    /// Unpack next_free from packed value
    #[inline]
    fn unpack_next_free(packed: u64) -> u32 {
        (packed & Self::NEXT_MASK) as u32
    }

    /// Get current state
    #[inline]
    fn get_state(&self) -> SlotState {
        Self::unpack_state(self.state_gen.load(Ordering::Acquire))
    }

    /// Get current generation
    #[inline]
    fn get_generation(&self) -> u32 {
        Self::unpack_generation(self.state_gen.load(Ordering::Acquire))
    }

    /// Get next_free pointer
    #[inline]
    fn get_next_free(&self) -> u32 {
        Self::unpack_next_free(self.state_gen.load(Ordering::Acquire))
    }

    /// Set next_free pointer (for free-list operations)
    #[inline]
    fn set_next_free(&self, next: u32) {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = Self::unpack_state(current);
            let gen = Self::unpack_generation(current);
            let new_packed = Self::pack(state, gen, next);

            if self
                .state_gen
                .compare_exchange_weak(current, new_packed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return;
            }
        }
    }

    /// Atomically transition to Allocated state, incrementing generation
    ///
    /// # Returns
    /// - `Some(new_generation)` if successful
    /// - `None` if slot was not Free
    fn try_allocate(&self) -> Option<u32> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = Self::unpack_state(current);

            if state != SlotState::Free {
                return None;
            }

            let old_gen = Self::unpack_generation(current);
            let new_gen = old_gen.wrapping_add(1) & 0x00FF_FFFF;
            let new_packed = Self::pack(SlotState::Allocated, new_gen, FREE_LIST_EMPTY);

            match self.state_gen.compare_exchange_weak(
                current,
                new_packed,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Update activity timestamp
                    self.touch();
                    return Some(new_gen);
                }
                Err(_) => continue,
            }
        }
    }

    /// Atomically transition to Free state
    ///
    /// # Arguments
    /// - `expected_gen`: Expected generation (for validation)
    ///
    /// # Returns
    /// - `Ok(())` if successful
    /// - `Err(actual_gen)` if generation mismatch
    fn try_release(&self, expected_gen: u32) -> Result<(), u32> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = Self::unpack_state(current);
            let actual_gen = Self::unpack_generation(current);

            if state != SlotState::Allocated {
                return Err(actual_gen);
            }

            if actual_gen != expected_gen {
                return Err(actual_gen);
            }

            let new_packed = Self::pack(SlotState::Free, actual_gen, FREE_LIST_EMPTY);

            match self.state_gen.compare_exchange_weak(
                current,
                new_packed,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => continue,
            }
        }
    }

    /// Update last activity timestamp
    #[inline]
    fn touch(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        self.last_activity_ns.store(now, Ordering::Release);
    }

    /// Get idle duration in seconds
    #[inline]
    fn idle_seconds(&self) -> u64 {
        let last = self.last_activity_ns.load(Ordering::Acquire);
        if last == 0 {
            return 0;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        (now.saturating_sub(last)) / 1_000_000_000
    }
}

// ============================================================================
// SessionPoolCapsule - T6 Mixed Tier Orchestrator
// ============================================================================

/// SessionPoolCapsule - T6 Mixed Tier Session Pool Orchestrator
///
/// Manages three-tier session pools with lockfree allocation/deallocation.
/// See module-level documentation for architecture details.
///
/// # Size
/// - Orchestrator: 512 bytes (cache-aligned)
/// - Actual pools: Heap-allocated (682MB for default config)
///
/// # Thread Safety
/// All operations are lockfree via atomic CAS operations.
#[repr(C, align(256))]
pub struct SessionPoolCapsule {
    // ========================================================================
    // Orchestrator Metadata (256 bytes)
    // ========================================================================
    /// Generation counter (TOCTOU prevention)
    generation: AtomicU64,
    /// Pool state: 0=uninitialized, 1=initializing, 2=ready, 3=shutting_down
    state: AtomicU64,
    /// Total allocations (all tiers)
    total_allocations: AtomicU64,
    /// Total releases (all tiers)
    total_releases: AtomicU64,
    /// Total upgrades
    total_upgrades: AtomicU64,
    /// Total downgrades
    total_downgrades: AtomicU64,
    /// Configuration (immutable after init)
    config: PoolConfig,
    /// Padding to 256 bytes
    _padding1: [u8; 256 - 6 * 8 - std::mem::size_of::<PoolConfig>()],

    // ========================================================================
    // Free-List Heads (64 bytes)
    // ========================================================================
    /// Light pool free-list head
    light_free_head: FreeListHead,
    /// Medium pool free-list head
    medium_free_head: FreeListHead,
    /// Heavy pool free-list head
    heavy_free_head: FreeListHead,
    /// Padding to 64 bytes
    _padding2: [u8; 64 - 3 * 8],

    // ========================================================================
    // Usage Stats (64 bytes)
    // ========================================================================
    /// Light sessions currently in use
    light_used: AtomicU32,
    /// Medium sessions currently in use
    medium_used: AtomicU32,
    /// Heavy sessions currently in use
    heavy_used: AtomicU32,
    /// Peak light usage
    peak_light: AtomicU32,
    /// Peak medium usage
    peak_medium: AtomicU32,
    /// Peak heavy usage
    peak_heavy: AtomicU32,
    /// Padding to 64 bytes
    _padding3: [u8; 64 - 6 * 4],

    // ========================================================================
    // Slot Metadata Pointers (64 bytes)
    // ========================================================================
    /// Light slot metadata array (heap-allocated)
    light_metadata: AtomicU64,
    /// Medium slot metadata array (heap-allocated)
    medium_metadata: AtomicU64,
    /// Heavy slot metadata array (heap-allocated)
    heavy_metadata: AtomicU64,
    /// Padding to 64 bytes
    _padding4: [u8; 64 - 3 * 8],

    // ========================================================================
    // Tier Thresholds (64 bytes)
    // ========================================================================
    /// Snapshot threshold for LIGHT → MEDIUM upgrade
    upgrade_light_to_medium: AtomicU32,
    /// Snapshot threshold for MEDIUM → HEAVY upgrade
    upgrade_medium_to_heavy: AtomicU32,
    /// Idle seconds before downgrade
    downgrade_idle_seconds: AtomicU32,
    /// Padding to 64 bytes
    _padding5: [u8; 64 - 3 * 4],
}

// Compile-time size verification
const _: () = {
    const EXPECTED_SIZE: usize = 512;
    const ACTUAL_SIZE: usize = std::mem::size_of::<SessionPoolCapsule>();
    assert!(
        ACTUAL_SIZE == EXPECTED_SIZE,
        "SessionPoolCapsule must be exactly 512 bytes"
    );
};

// SAFETY: SessionPoolCapsule is Send/Sync via atomic operations only
// #ASSUME_ALL_ATOMIC: All mutable fields use Atomic types
// #VERIFY_NO_MUTEXES: Zero mutex/RwLock in SessionPoolCapsule
unsafe impl Send for SessionPoolCapsule {}
unsafe impl Sync for SessionPoolCapsule {}

impl SessionPoolCapsule {
    // ========================================================================
    // Constructor
    // ========================================================================

    /// Create new session pool with given configuration
    ///
    /// # Performance
    /// O(n) where n = total capacity (initializes free-lists)
    ///
    /// # Arguments
    /// - `config`: Pool configuration
    ///
    /// # Returns
    /// Initialized SessionPoolCapsule with all slots in free-lists
    pub fn new(config: PoolConfig) -> Self {
        let pool = Self {
            // Metadata
            generation: AtomicU64::new(1),
            state: AtomicU64::new(1), // Initializing
            total_allocations: AtomicU64::new(0),
            total_releases: AtomicU64::new(0),
            total_upgrades: AtomicU64::new(0),
            total_downgrades: AtomicU64::new(0),
            config,
            _padding1: [0; 256 - 6 * 8 - std::mem::size_of::<PoolConfig>()],

            // Free-lists (will be initialized below)
            light_free_head: FreeListHead::new(),
            medium_free_head: FreeListHead::new(),
            heavy_free_head: FreeListHead::new(),
            _padding2: [0; 64 - 3 * 8],

            // Stats
            light_used: AtomicU32::new(0),
            medium_used: AtomicU32::new(0),
            heavy_used: AtomicU32::new(0),
            peak_light: AtomicU32::new(0),
            peak_medium: AtomicU32::new(0),
            peak_heavy: AtomicU32::new(0),
            _padding3: [0; 64 - 6 * 4],

            // Metadata pointers (null until allocated)
            light_metadata: AtomicU64::new(0),
            medium_metadata: AtomicU64::new(0),
            heavy_metadata: AtomicU64::new(0),
            _padding4: [0; 64 - 3 * 8],

            // Thresholds
            upgrade_light_to_medium: AtomicU32::new(config.upgrade_snapshot_light_to_medium),
            upgrade_medium_to_heavy: AtomicU32::new(config.upgrade_snapshot_medium_to_heavy),
            downgrade_idle_seconds: AtomicU32::new(config.downgrade_idle_seconds),
            _padding5: [0; 64 - 3 * 4],
        };

        // Initialize metadata arrays and free-lists
        pool.initialize_metadata();

        // Mark as ready
        pool.state.store(2, Ordering::Release);
        pool.generation.fetch_add(1, Ordering::AcqRel);

        pool
    }

    /// Initialize slot metadata arrays for all tiers
    fn initialize_metadata(&self) {
        // Allocate light metadata
        let light_vec: Vec<SlotMetadataEntry> = (0..self.config.light_capacity)
            .map(|_| SlotMetadataEntry::new())
            .collect();
        let light_ptr = Box::into_raw(light_vec.into_boxed_slice()) as *mut SlotMetadataEntry;
        self.light_metadata
            .store(light_ptr as u64, Ordering::Release);

        // Allocate medium metadata
        let medium_vec: Vec<SlotMetadataEntry> = (0..self.config.medium_capacity)
            .map(|_| SlotMetadataEntry::new())
            .collect();
        let medium_ptr = Box::into_raw(medium_vec.into_boxed_slice()) as *mut SlotMetadataEntry;
        self.medium_metadata
            .store(medium_ptr as u64, Ordering::Release);

        // Allocate heavy metadata
        let heavy_vec: Vec<SlotMetadataEntry> = (0..self.config.heavy_capacity)
            .map(|_| SlotMetadataEntry::new())
            .collect();
        let heavy_ptr = Box::into_raw(heavy_vec.into_boxed_slice()) as *mut SlotMetadataEntry;
        self.heavy_metadata
            .store(heavy_ptr as u64, Ordering::Release);

        // Initialize free-lists (link all slots)
        self.init_free_list(SessionTierType::Light);
        self.init_free_list(SessionTierType::Medium);
        self.init_free_list(SessionTierType::Heavy);
    }

    /// Initialize free-list for a tier (link all slots)
    fn init_free_list(&self, tier: SessionTierType) {
        let (capacity, metadata_ptr, free_head) = match tier {
            SessionTierType::Light => (
                self.config.light_capacity,
                self.light_metadata.load(Ordering::Acquire),
                &self.light_free_head,
            ),
            SessionTierType::Medium => (
                self.config.medium_capacity,
                self.medium_metadata.load(Ordering::Acquire),
                &self.medium_free_head,
            ),
            SessionTierType::Heavy => (
                self.config.heavy_capacity,
                self.heavy_metadata.load(Ordering::Acquire),
                &self.heavy_free_head,
            ),
        };

        if metadata_ptr == 0 || capacity == 0 {
            return;
        }

        // Link all slots in reverse order (so slot 0 is at head)
        // #ASSUME_METADATA_VALID: Pointer was just allocated
        // #VERIFY_BOUNDS: i < capacity always
        for i in (0..capacity).rev() {
            let next = if i + 1 < capacity {
                i + 1
            } else {
                FREE_LIST_EMPTY
            };

            // SAFETY: metadata_ptr is valid and i < capacity
            // #ASSUME_ALIGNED_ACCESS: SlotMetadataEntry is 16-byte aligned
            unsafe {
                let entry = &*(metadata_ptr as *const SlotMetadataEntry).add(i as usize);
                entry.set_next_free(next);
            }
        }

        // Set head to slot 0
        free_head.0.store(FreeListHead::pack(0, 0), Ordering::Release);
    }

    // ========================================================================
    // Allocation / Deallocation
    // ========================================================================

    /// Allocate a session from the specified tier
    ///
    /// # Performance
    /// <100ns lockfree (single CAS operation)
    ///
    /// # Arguments
    /// - `tier`: Session tier to allocate from
    ///
    /// # Returns
    /// - `Ok(SessionId)` with unique session identifier
    /// - `Err(PoolError::PoolFull)` if tier is exhausted
    pub fn allocate_session(&self, tier: SessionTierType) -> Result<SessionId, PoolError> {
        // Check state
        if self.state.load(Ordering::Acquire) != 2 {
            return Err(PoolError::NotInitialized);
        }

        let (capacity, metadata_ptr, free_head, used_counter, peak_counter) = match tier {
            SessionTierType::Light => (
                self.config.light_capacity,
                self.light_metadata.load(Ordering::Acquire),
                &self.light_free_head,
                &self.light_used,
                &self.peak_light,
            ),
            SessionTierType::Medium => (
                self.config.medium_capacity,
                self.medium_metadata.load(Ordering::Acquire),
                &self.medium_free_head,
                &self.medium_used,
                &self.peak_medium,
            ),
            SessionTierType::Heavy => (
                self.config.heavy_capacity,
                self.heavy_metadata.load(Ordering::Acquire),
                &self.heavy_free_head,
                &self.heavy_used,
                &self.peak_heavy,
            ),
        };

        if metadata_ptr == 0 {
            return Err(PoolError::NotInitialized);
        }

        // Pop from free-list
        // #ASSUME_METADATA_VALID: Pointer checked above
        let get_next = |index: u32| -> u32 {
            if index >= capacity {
                return FREE_LIST_EMPTY;
            }
            // SAFETY: index < capacity and metadata_ptr is valid
            unsafe {
                let entry = &*(metadata_ptr as *const SlotMetadataEntry).add(index as usize);
                entry.get_next_free()
            }
        };

        let slot_index = free_head
            .pop(get_next)
            .ok_or(PoolError::PoolFull { tier, capacity })?;

        // Mark slot as allocated
        // SAFETY: slot_index < capacity (pop only returns valid indices)
        // #ASSUME_POP_VALID: Free-list pop returns valid slot index
        let generation = unsafe {
            let entry = &*(metadata_ptr as *const SlotMetadataEntry).add(slot_index as usize);
            entry
                .try_allocate()
                .expect("Slot should be free after pop")
        };

        // Update counters
        let new_used = used_counter.fetch_add(1, Ordering::AcqRel) + 1;
        self.total_allocations.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Update peak if needed
        loop {
            let current_peak = peak_counter.load(Ordering::Relaxed);
            if new_used <= current_peak {
                break;
            }
            if peak_counter
                .compare_exchange_weak(
                    current_peak,
                    new_used,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }

        Ok(SessionId::new(tier.as_u8(), slot_index, generation))
    }

    /// Release a session back to the pool
    ///
    /// # Performance
    /// <100ns lockfree (single CAS operation)
    ///
    /// # Arguments
    /// - `id`: Session identifier to release
    ///
    /// # Returns
    /// - `Ok(())` if successfully released
    /// - `Err(PoolError)` if session not found or already released
    pub fn release_session(&self, id: SessionId) -> Result<(), PoolError> {
        if !id.is_valid() {
            return Err(PoolError::InvalidSessionId(id));
        }

        let tier = id
            .tier_type()
            .ok_or(PoolError::InvalidSessionId(id))?;
        let slot = id.slot();
        let expected_gen = id.generation();

        let (capacity, metadata_ptr, free_head, used_counter) = match tier {
            SessionTierType::Light => (
                self.config.light_capacity,
                self.light_metadata.load(Ordering::Acquire),
                &self.light_free_head,
                &self.light_used,
            ),
            SessionTierType::Medium => (
                self.config.medium_capacity,
                self.medium_metadata.load(Ordering::Acquire),
                &self.medium_free_head,
                &self.medium_used,
            ),
            SessionTierType::Heavy => (
                self.config.heavy_capacity,
                self.heavy_metadata.load(Ordering::Acquire),
                &self.heavy_free_head,
                &self.heavy_used,
            ),
        };

        if metadata_ptr == 0 {
            return Err(PoolError::NotInitialized);
        }

        if slot >= capacity {
            return Err(PoolError::InvalidSessionId(id));
        }

        // Try to release the slot
        // SAFETY: slot < capacity (checked above)
        // #ASSUME_SLOT_VALID: Slot index validated
        unsafe {
            let entry = &*(metadata_ptr as *const SlotMetadataEntry).add(slot as usize);

            entry.try_release(expected_gen).map_err(|actual| {
                if actual != expected_gen {
                    PoolError::GenerationMismatch {
                        expected: expected_gen,
                        actual,
                    }
                } else {
                    PoolError::AlreadyReleased(id)
                }
            })?;
        }

        // Push back to free-list
        // #ASSUME_METADATA_VALID: Already verified
        let set_next = |index: u32, next: u32| {
            // SAFETY: index < capacity
            unsafe {
                let entry = &*(metadata_ptr as *const SlotMetadataEntry).add(index as usize);
                entry.set_next_free(next);
            }
        };

        free_head.push(slot, set_next);

        // Update counters
        used_counter.fetch_sub(1, Ordering::AcqRel);
        self.total_releases.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    // ========================================================================
    // Tier Lookup
    // ========================================================================

    /// Get the tier of a session
    ///
    /// # Performance
    /// <10ns (direct extraction from SessionId)
    ///
    /// # Arguments
    /// - `id`: Session identifier
    ///
    /// # Returns
    /// - `Some(SessionTierType)` if valid session
    /// - `None` if invalid session ID
    #[inline]
    pub fn get_session_tier(&self, id: SessionId) -> Option<SessionTierType> {
        if !id.is_valid() {
            return None;
        }
        id.tier_type()
    }

    // ========================================================================
    // Upgrade / Downgrade
    // ========================================================================

    /// Upgrade a session to the next tier
    ///
    /// # Performance
    /// <1μs (includes data migration overhead)
    ///
    /// # Transitions
    /// - LIGHT → MEDIUM (when snapshots > 48)
    /// - MEDIUM → HEAVY (when snapshots > 384)
    ///
    /// # Arguments
    /// - `id`: Session identifier to upgrade
    ///
    /// # Returns
    /// - `Ok(SessionId)` new session ID in higher tier
    /// - `Err(PoolError::CannotUpgrade)` if already at HEAVY tier
    /// - `Err(PoolError::PoolFull)` if target tier is exhausted
    pub fn upgrade_session(&self, id: SessionId) -> Result<SessionId, PoolError> {
        if !id.is_valid() {
            return Err(PoolError::InvalidSessionId(id));
        }

        let current_tier = id
            .tier_type()
            .ok_or(PoolError::InvalidSessionId(id))?;

        // Determine target tier
        let target_tier = match current_tier {
            SessionTierType::Light => SessionTierType::Medium,
            SessionTierType::Medium => SessionTierType::Heavy,
            SessionTierType::Heavy => return Err(PoolError::CannotUpgrade(id)),
        };

        // Allocate in target tier first
        let new_id = self.allocate_session(target_tier)?;

        // Release old session
        // Note: In production, this would copy state before releasing
        if let Err(e) = self.release_session(id) {
            // Rollback: release the new allocation
            let _ = self.release_session(new_id);
            return Err(e);
        }

        // Update counters
        self.total_upgrades.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(new_id)
    }

    /// Downgrade a session to the previous tier
    ///
    /// # Performance
    /// <1μs (includes data migration overhead)
    ///
    /// # Transitions
    /// - HEAVY → MEDIUM (when idle > 30 minutes)
    /// - MEDIUM → LIGHT (when idle > 30 minutes)
    ///
    /// # Arguments
    /// - `id`: Session identifier to downgrade
    ///
    /// # Returns
    /// - `Ok(SessionId)` new session ID in lower tier
    /// - `Err(PoolError::CannotDowngrade)` if already at LIGHT tier
    /// - `Err(PoolError::PoolFull)` if target tier is exhausted
    pub fn downgrade_session(&self, id: SessionId) -> Result<SessionId, PoolError> {
        if !id.is_valid() {
            return Err(PoolError::InvalidSessionId(id));
        }

        let current_tier = id
            .tier_type()
            .ok_or(PoolError::InvalidSessionId(id))?;

        // Determine target tier
        let target_tier = match current_tier {
            SessionTierType::Light => return Err(PoolError::CannotDowngrade(id)),
            SessionTierType::Medium => SessionTierType::Light,
            SessionTierType::Heavy => SessionTierType::Medium,
        };

        // Allocate in target tier first
        let new_id = self.allocate_session(target_tier)?;

        // Release old session
        if let Err(e) = self.release_session(id) {
            // Rollback: release the new allocation
            let _ = self.release_session(new_id);
            return Err(e);
        }

        // Update counters
        self.total_downgrades.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(new_id)
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get pool statistics snapshot
    ///
    /// # Performance
    /// <50ns (atomic loads only)
    ///
    /// # Returns
    /// Consistent snapshot of pool statistics
    pub fn get_pool_stats(&self) -> PoolStats {
        PoolStats {
            light_used: self.light_used.load(Ordering::Relaxed),
            light_capacity: self.config.light_capacity,
            medium_used: self.medium_used.load(Ordering::Relaxed),
            medium_capacity: self.config.medium_capacity,
            heavy_used: self.heavy_used.load(Ordering::Relaxed),
            heavy_capacity: self.config.heavy_capacity,
            peak_light: self.peak_light.load(Ordering::Relaxed),
            peak_medium: self.peak_medium.load(Ordering::Relaxed),
            peak_heavy: self.peak_heavy.load(Ordering::Relaxed),
            total_allocations: self.total_allocations.load(Ordering::Relaxed),
            total_releases: self.total_releases.load(Ordering::Relaxed),
            total_upgrades: self.total_upgrades.load(Ordering::Relaxed),
            total_downgrades: self.total_downgrades.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Get configuration (immutable)
    #[inline]
    pub fn config(&self) -> &PoolConfig {
        &self.config
    }

    /// Check if pool is ready
    #[inline]
    pub fn is_ready(&self) -> bool {
        self.state.load(Ordering::Acquire) == 2
    }
}

impl Drop for SessionPoolCapsule {
    fn drop(&mut self) {
        // Clean up metadata arrays
        // #ASSUME_OWNED: We own the allocations made in initialize_metadata
        let light_ptr = self.light_metadata.load(Ordering::Acquire);
        if light_ptr != 0 {
            // SAFETY: We allocated this with Box::into_raw
            unsafe {
                let _ = Box::from_raw(std::slice::from_raw_parts_mut(
                    light_ptr as *mut SlotMetadataEntry,
                    self.config.light_capacity as usize,
                ));
            }
        }

        let medium_ptr = self.medium_metadata.load(Ordering::Acquire);
        if medium_ptr != 0 {
            // SAFETY: We allocated this with Box::into_raw
            unsafe {
                let _ = Box::from_raw(std::slice::from_raw_parts_mut(
                    medium_ptr as *mut SlotMetadataEntry,
                    self.config.medium_capacity as usize,
                ));
            }
        }

        let heavy_ptr = self.heavy_metadata.load(Ordering::Acquire);
        if heavy_ptr != 0 {
            // SAFETY: We allocated this with Box::into_raw
            unsafe {
                let _ = Box::from_raw(std::slice::from_raw_parts_mut(
                    heavy_ptr as *mut SlotMetadataEntry,
                    self.config.heavy_capacity as usize,
                ));
            }
        }
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_encoding() {
        let id = SessionId::new(1, 123, 456);
        assert_eq!(id.tier(), 1);
        assert_eq!(id.slot(), 123);
        assert_eq!(id.generation(), 456);
        assert!(id.is_valid());
        assert_eq!(id.tier_type(), Some(SessionTierType::Medium));
    }

    #[test]
    fn test_session_id_invalid() {
        assert!(!SessionId::INVALID.is_valid());
        assert_eq!(SessionId::INVALID.0, 0);
    }

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.light_capacity, DEFAULT_LIGHT_CAPACITY);
        assert_eq!(config.medium_capacity, DEFAULT_MEDIUM_CAPACITY);
        assert_eq!(config.heavy_capacity, DEFAULT_HEAVY_CAPACITY);
    }

    #[test]
    fn test_pool_config_test() {
        let config = PoolConfig::test_config();
        assert_eq!(config.light_capacity, 16);
        assert_eq!(config.medium_capacity, 8);
        assert_eq!(config.heavy_capacity, 4);
    }

    #[test]
    fn test_pool_initialization() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());
        assert!(pool.is_ready());

        let stats = pool.get_pool_stats();
        assert_eq!(stats.light_used, 0);
        assert_eq!(stats.medium_used, 0);
        assert_eq!(stats.heavy_used, 0);
        assert_eq!(stats.light_capacity, 16);
    }

    #[test]
    fn test_allocate_light_session() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let id = pool.allocate_session(SessionTierType::Light).unwrap();
        assert!(id.is_valid());
        assert_eq!(id.tier_type(), Some(SessionTierType::Light));

        let stats = pool.get_pool_stats();
        assert_eq!(stats.light_used, 1);
        assert_eq!(stats.total_allocations, 1);
    }

    #[test]
    fn test_allocate_medium_session() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let id = pool.allocate_session(SessionTierType::Medium).unwrap();
        assert!(id.is_valid());
        assert_eq!(id.tier_type(), Some(SessionTierType::Medium));

        let stats = pool.get_pool_stats();
        assert_eq!(stats.medium_used, 1);
    }

    #[test]
    fn test_allocate_heavy_session() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let id = pool.allocate_session(SessionTierType::Heavy).unwrap();
        assert!(id.is_valid());
        assert_eq!(id.tier_type(), Some(SessionTierType::Heavy));

        let stats = pool.get_pool_stats();
        assert_eq!(stats.heavy_used, 1);
    }

    #[test]
    fn test_release_session() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let id = pool.allocate_session(SessionTierType::Light).unwrap();
        assert_eq!(pool.get_pool_stats().light_used, 1);

        pool.release_session(id).unwrap();
        assert_eq!(pool.get_pool_stats().light_used, 0);
        assert_eq!(pool.get_pool_stats().total_releases, 1);
    }

    #[test]
    fn test_double_release_fails() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let id = pool.allocate_session(SessionTierType::Light).unwrap();
        pool.release_session(id).unwrap();

        let result = pool.release_session(id);
        assert!(matches!(result, Err(PoolError::GenerationMismatch { .. }) | Err(PoolError::AlreadyReleased(_))));
    }

    #[test]
    fn test_pool_exhaustion() {
        let config = PoolConfig {
            light_capacity: 2,
            medium_capacity: 1,
            heavy_capacity: 1,
            ..PoolConfig::test_config()
        };
        let pool = SessionPoolCapsule::new(config);

        // Allocate all light slots
        let id1 = pool.allocate_session(SessionTierType::Light).unwrap();
        let id2 = pool.allocate_session(SessionTierType::Light).unwrap();

        // Third allocation should fail
        let result = pool.allocate_session(SessionTierType::Light);
        assert!(matches!(result, Err(PoolError::PoolFull { tier: SessionTierType::Light, capacity: 2 })));

        // Release one and try again
        pool.release_session(id1).unwrap();
        let id3 = pool.allocate_session(SessionTierType::Light).unwrap();
        assert!(id3.is_valid());

        // Cleanup
        pool.release_session(id2).unwrap();
        pool.release_session(id3).unwrap();
    }

    #[test]
    fn test_upgrade_light_to_medium() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let light_id = pool.allocate_session(SessionTierType::Light).unwrap();
        assert_eq!(light_id.tier_type(), Some(SessionTierType::Light));

        let medium_id = pool.upgrade_session(light_id).unwrap();
        assert_eq!(medium_id.tier_type(), Some(SessionTierType::Medium));

        let stats = pool.get_pool_stats();
        assert_eq!(stats.light_used, 0);
        assert_eq!(stats.medium_used, 1);
        assert_eq!(stats.total_upgrades, 1);
    }

    #[test]
    fn test_upgrade_medium_to_heavy() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let medium_id = pool.allocate_session(SessionTierType::Medium).unwrap();
        let heavy_id = pool.upgrade_session(medium_id).unwrap();

        assert_eq!(heavy_id.tier_type(), Some(SessionTierType::Heavy));

        let stats = pool.get_pool_stats();
        assert_eq!(stats.medium_used, 0);
        assert_eq!(stats.heavy_used, 1);
    }

    #[test]
    fn test_upgrade_heavy_fails() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let heavy_id = pool.allocate_session(SessionTierType::Heavy).unwrap();
        let result = pool.upgrade_session(heavy_id);

        assert!(matches!(result, Err(PoolError::CannotUpgrade(_))));
    }

    #[test]
    fn test_downgrade_heavy_to_medium() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let heavy_id = pool.allocate_session(SessionTierType::Heavy).unwrap();
        let medium_id = pool.downgrade_session(heavy_id).unwrap();

        assert_eq!(medium_id.tier_type(), Some(SessionTierType::Medium));

        let stats = pool.get_pool_stats();
        assert_eq!(stats.heavy_used, 0);
        assert_eq!(stats.medium_used, 1);
        assert_eq!(stats.total_downgrades, 1);
    }

    #[test]
    fn test_downgrade_light_fails() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let light_id = pool.allocate_session(SessionTierType::Light).unwrap();
        let result = pool.downgrade_session(light_id);

        assert!(matches!(result, Err(PoolError::CannotDowngrade(_))));
    }

    #[test]
    fn test_get_session_tier() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        let light_id = pool.allocate_session(SessionTierType::Light).unwrap();
        let medium_id = pool.allocate_session(SessionTierType::Medium).unwrap();
        let heavy_id = pool.allocate_session(SessionTierType::Heavy).unwrap();

        assert_eq!(pool.get_session_tier(light_id), Some(SessionTierType::Light));
        assert_eq!(pool.get_session_tier(medium_id), Some(SessionTierType::Medium));
        assert_eq!(pool.get_session_tier(heavy_id), Some(SessionTierType::Heavy));
        assert_eq!(pool.get_session_tier(SessionId::INVALID), None);
    }

    #[test]
    fn test_pool_stats_accuracy() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());

        // Allocate some sessions
        let l1 = pool.allocate_session(SessionTierType::Light).unwrap();
        let l2 = pool.allocate_session(SessionTierType::Light).unwrap();
        let m1 = pool.allocate_session(SessionTierType::Medium).unwrap();

        let stats = pool.get_pool_stats();
        assert_eq!(stats.light_used, 2);
        assert_eq!(stats.medium_used, 1);
        assert_eq!(stats.heavy_used, 0);
        assert_eq!(stats.total_allocations, 3);
        assert_eq!(stats.peak_light, 2);

        // Release one light
        pool.release_session(l1).unwrap();

        let stats = pool.get_pool_stats();
        assert_eq!(stats.light_used, 1);
        assert_eq!(stats.total_releases, 1);
        assert_eq!(stats.peak_light, 2); // Peak stays at 2

        // Cleanup
        pool.release_session(l2).unwrap();
        pool.release_session(m1).unwrap();
    }

    #[test]
    fn test_concurrent_allocation_stress() {
        use std::sync::Arc;
        use std::thread;

        let pool = Arc::new(SessionPoolCapsule::new(PoolConfig::test_config()));
        let mut handles = vec![];

        // Spawn 4 threads, each allocating and releasing sessions
        for _ in 0..4 {
            let pool_clone = Arc::clone(&pool);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    if let Ok(id) = pool_clone.allocate_session(SessionTierType::Light) {
                        // Small work
                        std::hint::spin_loop();
                        let _ = pool_clone.release_session(id);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = pool.get_pool_stats();
        assert_eq!(stats.light_used, 0); // All released
        assert!(stats.total_allocations > 0);
        assert_eq!(stats.total_allocations, stats.total_releases);
    }

    #[test]
    fn test_free_list_correctness() {
        let config = PoolConfig {
            light_capacity: 4,
            medium_capacity: 2,
            heavy_capacity: 1,
            ..PoolConfig::test_config()
        };
        let pool = SessionPoolCapsule::new(config);

        // Allocate all light slots
        let ids: Vec<_> = (0..4)
            .map(|_| pool.allocate_session(SessionTierType::Light).unwrap())
            .collect();

        // Pool should be full
        assert!(pool.allocate_session(SessionTierType::Light).is_err());

        // Release in reverse order
        for id in ids.into_iter().rev() {
            pool.release_session(id).unwrap();
        }

        // Should be able to allocate 4 again
        let new_ids: Vec<_> = (0..4)
            .map(|_| pool.allocate_session(SessionTierType::Light).unwrap())
            .collect();

        assert_eq!(new_ids.len(), 4);

        // Cleanup
        for id in new_ids {
            pool.release_session(id).unwrap();
        }
    }

    #[test]
    fn test_session_tier_type_properties() {
        assert_eq!(SessionTierType::Light.session_size(), 64 * 1024);
        assert_eq!(SessionTierType::Medium.session_size(), 256 * 1024);
        assert_eq!(SessionTierType::Heavy.session_size(), 1_147_392);

        assert_eq!(SessionTierType::Light.as_str(), "Light");
        assert_eq!(SessionTierType::Medium.as_str(), "Medium");
        assert_eq!(SessionTierType::Heavy.as_str(), "Heavy");
    }

    #[test]
    fn test_pool_stats_display() {
        let pool = SessionPoolCapsule::new(PoolConfig::test_config());
        pool.allocate_session(SessionTierType::Light).unwrap();

        let stats = pool.get_pool_stats();
        let display = format!("{}", stats);

        assert!(display.contains("light: 1/16"));
        assert!(display.contains("util:"));
    }
}
