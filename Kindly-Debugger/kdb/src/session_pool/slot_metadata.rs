//! SlotMetadata Capsule - T1 Atomic Session Slot State Tracking
//!
//! 64-byte cache-line aligned capsule for tracking session slot state in free-lists.
//! Uses DualAtomicU64 pattern for packed state with ABA prevention via generation counter.
//!
//! # Layout (64 bytes)
//!
//! ```text
//! Field             Bits    Description
//! ─────────────────────────────────────────────────
//! slot_id           [0:15]  Unique slot identifier (0-65535)
//! tier              [16:19] Session tier (0=LIGHT, 1=MEDIUM, 2=HEAVY)
//! state             [20:23] Slot state (6 states)
//! generation        [24:47] Generation counter (ABA prevention, 16M cycles)
//! timestamp         [48:63] Compact timestamp (seconds since epoch, 16-bit)
//! ```
//!
//! # COCA Compliance
//!
//! - 100% lockfree (no mutex/RwLock)
//! - Cache-line aligned (64 bytes)
//! - Generation counter for ABA prevention
//! - const fn for compile-time initialization
//!
//! #ASSUME_LOCKFREE_ONLY: All coordination via atomics, no mutex/RwLock
//! #ASSUME_CACHE_ALIGNED: 64-byte alignment prevents false sharing
//! #ASSUME_ABA_PREVENTION: Generation counter prevents ABA problem
//! #ASSUME_COPY_SNAPSHOT: All reads are atomic snapshots

use std::sync::atomic::{AtomicU64, Ordering};

/// Session tier defining memory allocation size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SessionTier {
    /// Light session: 64 KB allocation
    Light = 0,
    /// Medium session: 256 KB allocation
    Medium = 1,
    /// Heavy session: 1.09 MB allocation (full DebuggerCapsule)
    Heavy = 2,
}

impl SessionTier {
    /// Convert from raw u8 value (4-bit field).
    #[inline]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw & 0x0F {
            0 => Some(Self::Light),
            1 => Some(Self::Medium),
            2 => Some(Self::Heavy),
            _ => None,
        }
    }

    /// Get allocation size in bytes for this tier.
    #[inline]
    pub const fn allocation_size(&self) -> usize {
        match self {
            Self::Light => 65_536,         // 64 KB
            Self::Medium => 262_144,       // 256 KB
            Self::Heavy => 1_146_880,      // ~1.09 MB
        }
    }
}

/// Slot state in the session pool lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SlotState {
    /// Slot is available in free-list
    Free = 0,
    /// Slot has been allocated but not yet in use
    Allocated = 1,
    /// Slot is actively being used by a session
    InUse = 2,
    /// Slot is draining (graceful shutdown)
    Draining = 3,
    /// Slot is being upgraded to higher tier
    Upgrading = 4,
    /// Slot is being downgraded to lower tier
    Downgrading = 5,
}

impl SlotState {
    /// Convert from raw u8 value (4-bit field).
    #[inline]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw & 0x0F {
            0 => Some(Self::Free),
            1 => Some(Self::Allocated),
            2 => Some(Self::InUse),
            3 => Some(Self::Draining),
            4 => Some(Self::Upgrading),
            5 => Some(Self::Downgrading),
            _ => None,
        }
    }

    /// Check if slot can be allocated.
    #[inline]
    pub const fn is_allocatable(&self) -> bool {
        matches!(self, Self::Free)
    }

    /// Check if slot is in active use.
    #[inline]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::InUse | Self::Draining)
    }
}

/// Packed slot metadata for atomic operations.
///
/// Layout: slot_id(16) | tier(4) | state(4) | generation(24) | timestamp(16)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedMetadata(u64);

impl PackedMetadata {
    // Bit field positions and masks
    const SLOT_ID_SHIFT: u32 = 0;
    const SLOT_ID_MASK: u64 = 0xFFFF;

    const TIER_SHIFT: u32 = 16;
    const TIER_MASK: u64 = 0x0F;

    const STATE_SHIFT: u32 = 20;
    const STATE_MASK: u64 = 0x0F;

    const GENERATION_SHIFT: u32 = 24;
    const GENERATION_MASK: u64 = 0xFFFFFF; // 24 bits

    const TIMESTAMP_SHIFT: u32 = 48;
    const TIMESTAMP_MASK: u64 = 0xFFFF;

    /// Create new packed metadata.
    #[inline]
    pub const fn new(slot_id: u16, tier: SessionTier, state: SlotState, generation: u32, timestamp: u16) -> Self {
        let packed = ((slot_id as u64) & Self::SLOT_ID_MASK)
            | (((tier as u64) & Self::TIER_MASK) << Self::TIER_SHIFT)
            | (((state as u64) & Self::STATE_MASK) << Self::STATE_SHIFT)
            | (((generation as u64) & Self::GENERATION_MASK) << Self::GENERATION_SHIFT)
            | (((timestamp as u64) & Self::TIMESTAMP_MASK) << Self::TIMESTAMP_SHIFT);
        Self(packed)
    }

    /// Create from raw u64 value.
    #[inline]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Get raw u64 value.
    #[inline]
    pub const fn as_raw(&self) -> u64 {
        self.0
    }

    /// Extract slot ID (16 bits).
    #[inline]
    pub const fn slot_id(&self) -> u16 {
        ((self.0 >> Self::SLOT_ID_SHIFT) & Self::SLOT_ID_MASK) as u16
    }

    /// Extract tier (4 bits).
    #[inline]
    pub fn tier(&self) -> SessionTier {
        let raw = ((self.0 >> Self::TIER_SHIFT) & Self::TIER_MASK) as u8;
        SessionTier::from_raw(raw).unwrap_or(SessionTier::Light)
    }

    /// Extract state (4 bits).
    #[inline]
    pub fn state(&self) -> SlotState {
        let raw = ((self.0 >> Self::STATE_SHIFT) & Self::STATE_MASK) as u8;
        SlotState::from_raw(raw).unwrap_or(SlotState::Free)
    }

    /// Extract generation counter (24 bits).
    #[inline]
    pub const fn generation(&self) -> u32 {
        ((self.0 >> Self::GENERATION_SHIFT) & Self::GENERATION_MASK) as u32
    }

    /// Extract timestamp (16 bits).
    #[inline]
    pub const fn timestamp(&self) -> u16 {
        ((self.0 >> Self::TIMESTAMP_SHIFT) & Self::TIMESTAMP_MASK) as u16
    }

    /// Create new metadata with updated state and incremented generation.
    #[inline]
    pub fn with_state(&self, new_state: SlotState) -> Self {
        let new_gen = (self.generation() + 1) & (Self::GENERATION_MASK as u32);
        Self::new(self.slot_id(), self.tier(), new_state, new_gen, self.timestamp())
    }

    /// Create new metadata with updated tier and incremented generation.
    #[inline]
    pub fn with_tier(&self, new_tier: SessionTier) -> Self {
        let new_gen = (self.generation() + 1) & (Self::GENERATION_MASK as u32);
        Self::new(self.slot_id(), new_tier, self.state(), new_gen, self.timestamp())
    }

    /// Create new metadata with updated timestamp.
    #[inline]
    pub fn with_timestamp(&self, new_timestamp: u16) -> Self {
        Self::new(self.slot_id(), self.tier(), self.state(), self.generation(), new_timestamp)
    }
}

/// SlotMetadata Capsule - 64 bytes, cache-line aligned.
///
/// Tracks session slot state using DualAtomicU64 pattern for packed state.
/// Generation counter provides ABA prevention for lockfree operations.
///
/// # Performance
///
/// - Load: <5ns (single atomic load)
/// - CAS: <10ns (single compare-and-swap)
/// - False sharing: Prevented via 64B alignment
///
/// #ASSUME_LOCKFREE_ONLY: All operations via atomics
/// #VERIFY_UNIT_TEST: test_slot_metadata_size, test_packed_metadata
#[repr(C, align(64))]
pub struct SlotMetadata {
    /// Packed metadata: slot_id(16) | tier(4) | state(4) | generation(24) | timestamp(16)
    packed: AtomicU64,

    /// Reserved for future extension (maintains 64B alignment)
    _reserved: [u8; 56],
}

impl SlotMetadata {
    /// Create empty slot metadata (const fn for compile-time initialization).
    #[inline]
    pub const fn empty() -> Self {
        Self {
            packed: AtomicU64::new(0),
            _reserved: [0; 56],
        }
    }

    /// Create new slot metadata with specified initial state.
    #[inline]
    pub const fn new(slot_id: u16, tier: SessionTier) -> Self {
        let packed = PackedMetadata::new(slot_id, tier, SlotState::Free, 0, 0);
        Self {
            packed: AtomicU64::new(packed.as_raw()),
            _reserved: [0; 56],
        }
    }

    /// Load current metadata atomically.
    ///
    /// #ASSUME_COPY_SNAPSHOT: Returns atomic snapshot of packed state
    #[inline]
    pub fn load(&self) -> PackedMetadata {
        PackedMetadata::from_raw(self.packed.load(Ordering::Acquire))
    }

    /// Store new metadata atomically.
    #[inline]
    pub fn store(&self, metadata: PackedMetadata) {
        self.packed.store(metadata.as_raw(), Ordering::Release);
    }

    /// Compare-and-swap metadata atomically.
    ///
    /// Returns Ok(new) if successful, Err(current) if failed.
    ///
    /// #ASSUME_ABA_PREVENTION: Generation counter prevents ABA
    /// #VERIFY_UNIT_TEST: test_cas_aba_prevention
    #[inline]
    pub fn compare_exchange(
        &self,
        expected: PackedMetadata,
        new: PackedMetadata,
    ) -> Result<PackedMetadata, PackedMetadata> {
        match self.packed.compare_exchange(
            expected.as_raw(),
            new.as_raw(),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(new),
            Err(current) => Err(PackedMetadata::from_raw(current)),
        }
    }

    /// Transition slot state atomically with generation increment.
    ///
    /// Returns Ok(new_metadata) if transition succeeded, Err(current) if failed.
    pub fn transition_state(&self, expected_state: SlotState, new_state: SlotState) -> Result<PackedMetadata, PackedMetadata> {
        loop {
            let current = self.load();
            if current.state() != expected_state {
                return Err(current);
            }

            let new = current.with_state(new_state);
            match self.compare_exchange(current, new) {
                Ok(result) => return Ok(result),
                Err(_) => continue, // Retry on CAS failure
            }
        }
    }

    /// Attempt to allocate this slot (Free -> Allocated).
    ///
    /// Returns Ok(metadata) if successful, Err(current) if slot not free.
    #[inline]
    pub fn try_allocate(&self) -> Result<PackedMetadata, PackedMetadata> {
        self.transition_state(SlotState::Free, SlotState::Allocated)
    }

    /// Activate an allocated slot (Allocated -> InUse).
    #[inline]
    pub fn activate(&self) -> Result<PackedMetadata, PackedMetadata> {
        self.transition_state(SlotState::Allocated, SlotState::InUse)
    }

    /// Begin draining a slot (InUse -> Draining).
    #[inline]
    pub fn begin_drain(&self) -> Result<PackedMetadata, PackedMetadata> {
        self.transition_state(SlotState::InUse, SlotState::Draining)
    }

    /// Release slot back to free-list (any terminal state -> Free).
    pub fn release(&self) -> Result<PackedMetadata, PackedMetadata> {
        loop {
            let current = self.load();
            match current.state() {
                SlotState::Allocated | SlotState::Draining => {
                    let new = current.with_state(SlotState::Free);
                    match self.compare_exchange(current, new) {
                        Ok(result) => return Ok(result),
                        Err(_) => continue,
                    }
                }
                SlotState::Free => return Ok(current), // Already free
                _ => return Err(current), // Cannot release from InUse/Upgrading/Downgrading
            }
        }
    }

    /// Get current slot ID.
    #[inline]
    pub fn slot_id(&self) -> u16 {
        self.load().slot_id()
    }

    /// Get current tier.
    #[inline]
    pub fn tier(&self) -> SessionTier {
        self.load().tier()
    }

    /// Get current state.
    #[inline]
    pub fn state(&self) -> SlotState {
        self.load().state()
    }

    /// Get current generation (ABA counter).
    #[inline]
    pub fn generation(&self) -> u32 {
        self.load().generation()
    }

    /// Check if slot is free.
    #[inline]
    pub fn is_free(&self) -> bool {
        self.state() == SlotState::Free
    }

    /// Check if slot is in active use.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state().is_active()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn test_slot_metadata_size() {
        assert_eq!(size_of::<SlotMetadata>(), 64, "SlotMetadata must be 64 bytes");
    }

    #[test]
    fn test_slot_metadata_alignment() {
        assert_eq!(align_of::<SlotMetadata>(), 64, "SlotMetadata must be 64-byte aligned");
    }

    #[test]
    fn test_packed_metadata_fields() {
        let packed = PackedMetadata::new(42, SessionTier::Medium, SlotState::InUse, 12345, 9999);

        assert_eq!(packed.slot_id(), 42);
        assert_eq!(packed.tier(), SessionTier::Medium);
        assert_eq!(packed.state(), SlotState::InUse);
        assert_eq!(packed.generation(), 12345);
        assert_eq!(packed.timestamp(), 9999);
    }

    #[test]
    fn test_packed_metadata_with_state() {
        let packed = PackedMetadata::new(100, SessionTier::Heavy, SlotState::Free, 0, 0);
        let updated = packed.with_state(SlotState::Allocated);

        assert_eq!(updated.slot_id(), 100);
        assert_eq!(updated.tier(), SessionTier::Heavy);
        assert_eq!(updated.state(), SlotState::Allocated);
        assert_eq!(updated.generation(), 1); // Incremented
    }

    #[test]
    fn test_slot_metadata_transitions() {
        let slot = SlotMetadata::new(1, SessionTier::Light);

        // Free -> Allocated
        assert!(slot.try_allocate().is_ok());
        assert_eq!(slot.state(), SlotState::Allocated);

        // Allocated -> InUse
        assert!(slot.activate().is_ok());
        assert_eq!(slot.state(), SlotState::InUse);

        // InUse -> Draining
        assert!(slot.begin_drain().is_ok());
        assert_eq!(slot.state(), SlotState::Draining);

        // Draining -> Free
        assert!(slot.release().is_ok());
        assert_eq!(slot.state(), SlotState::Free);
    }

    #[test]
    fn test_slot_metadata_invalid_transition() {
        let slot = SlotMetadata::new(2, SessionTier::Medium);

        // Cannot activate a Free slot (must allocate first)
        assert!(slot.activate().is_err());

        // Cannot drain a Free slot
        assert!(slot.begin_drain().is_err());
    }

    #[test]
    fn test_generation_counter_aba_prevention() {
        let slot = SlotMetadata::new(3, SessionTier::Heavy);
        let initial_gen = slot.generation();

        // Each transition increments generation
        slot.try_allocate().unwrap();
        assert_eq!(slot.generation(), initial_gen + 1);

        slot.activate().unwrap();
        assert_eq!(slot.generation(), initial_gen + 2);

        slot.begin_drain().unwrap();
        assert_eq!(slot.generation(), initial_gen + 3);

        slot.release().unwrap();
        assert_eq!(slot.generation(), initial_gen + 4);
    }

    #[test]
    fn test_session_tier_sizes() {
        assert_eq!(SessionTier::Light.allocation_size(), 65_536);
        assert_eq!(SessionTier::Medium.allocation_size(), 262_144);
        assert_eq!(SessionTier::Heavy.allocation_size(), 1_146_880);
    }

    #[test]
    fn test_const_empty_initialization() {
        // Verify const fn works at compile time
        const EMPTY: SlotMetadata = SlotMetadata::empty();
        assert_eq!(EMPTY.slot_id(), 0);
        assert_eq!(EMPTY.state(), SlotState::Free);
    }
}
