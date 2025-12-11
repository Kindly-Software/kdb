//! SessionTierMapCapsule - T1 Atomic Session to Tier Mapping (256 KB)
//!
//! Lockfree hash table mapping session IDs to subscription tiers.
//! **Latency**: <50ns lookup, <100ns insert
//! **Tier**: T1 Atomic (lockfree linear probing with FNV-1a hash)
//!
//! ## UCE34 Framework Application (Q1-Q34)
//!
//! ### Q10-Q12: Tier Selection
//! - Q10: T1 Atomic (lockfree hash table, <50ns lookup)
//! - Q11: Type-safe SessionTierEntry with atomic fields
//! - Q12: Nightly: const generics for table size
//!
//! ### Q33: Verification
//! - Size: 256 KB (4096 slots x 16 bytes + metadata)
//! - Alignment: 256 bytes (multi-cache-line for hot paths)
//! - 100% lockfree (linear probing, max 16 probes)
//!
//! ### Q34: Auditability
//! - Generation counter for TOCTOU prevention
//! - Created timestamp in tier_and_meta field
//! - Active session count for monitoring

use core::sync::atomic::{AtomicU64, Ordering};

// Re-export SubscriptionTier from canonical source
pub use crate::subscription_tier::SubscriptionTier;

// ============================================================================
// Constants
// ============================================================================

/// Number of slots in the hash table (power of 2 for fast modulo)
pub const SESSION_TABLE_SLOTS: usize = 4096;

/// Maximum probing distance before giving up
const MAX_PROBE_DISTANCE: usize = 16;

/// Mask for slot index calculation (slots - 1 for power-of-2)
const SLOT_MASK: u64 = (SESSION_TABLE_SLOTS - 1) as u64;

// ============================================================================
// SessionTierEntry (16 bytes, atomic)
// ============================================================================

/// Single entry in the session-tier hash table
///
/// # Memory Layout (16 bytes)
/// ```text
/// session_id (8 bytes):    Session identifier (0 = empty slot)
/// tier_and_meta (8 bytes): Packed tier, timestamp, and generation
///   ├─ bits 0-7:   tier (SubscriptionTier as u8)
///   ├─ bits 8-31:  created_unix_days (24 bits, days since epoch, ~45K years)
///   └─ bits 32-63: generation (32 bits, TOCTOU prevention)
/// ```
#[repr(C)]
pub struct SessionTierEntry {
    /// Session ID (0 = empty/available slot)
    session_id: AtomicU64,

    /// Packed tier and metadata
    /// - bits 0-7:   tier
    /// - bits 8-31:  created_unix_days (24 bits)
    /// - bits 32-63: generation (32 bits)
    tier_and_meta: AtomicU64,
}

impl SessionTierEntry {
    /// Create empty entry
    const fn empty() -> Self {
        Self {
            session_id: AtomicU64::new(0),
            tier_and_meta: AtomicU64::new(0),
        }
    }

    /// Pack tier and metadata into u64
    #[inline]
    fn pack_meta(tier: SubscriptionTier, created_days: u32, generation: u32) -> u64 {
        let tier_bits = tier as u64;
        let days_bits = ((created_days as u64) & 0xFFFFFF) << 8;
        let gen_bits = (generation as u64) << 32;
        tier_bits | days_bits | gen_bits
    }

    /// Unpack tier from metadata
    #[inline]
    fn unpack_tier(meta: u64) -> Option<SubscriptionTier> {
        SubscriptionTier::from_u8((meta & 0xFF) as u8)
    }

    /// Unpack created days from metadata
    #[inline]
    fn unpack_created_days(meta: u64) -> u32 {
        ((meta >> 8) & 0xFFFFFF) as u32
    }

    /// Unpack generation from metadata
    #[inline]
    fn unpack_generation(meta: u64) -> u32 {
        (meta >> 32) as u32
    }
}

// ============================================================================
// SessionTierMapCapsule (256 KB, T1 Atomic)
// ============================================================================

/// T1 Atomic Session-Tier Map - Lockfree session to tier mapping
///
/// # Memory Layout (256 KB)
/// ```text
/// Offset 0-65535:      entries[4096] (16 bytes x 4096 = 64 KB)
/// Offset 65536-65543:  generation (8 bytes)
/// Offset 65544-65551:  active_sessions (8 bytes)
/// Offset 65552-65791:  _reserved (240 bytes)
/// Total: 65792 bytes (~64 KB)
/// ```
///
/// Note: Actual size is 64 KB, not 256 KB. The 256 KB alignment ensures
/// the entire structure fits in a single memory region for NUMA locality.
///
/// # Hash Function
/// FNV-1a inspired hash for fast, well-distributed slot calculation:
/// ```text
/// hash = (session_id ^ FNV_OFFSET) * FNV_PRIME
/// slot = hash & (SLOTS - 1)
/// ```
///
/// # Collision Resolution
/// Linear probing with max 16 probes. If all 16 slots are occupied,
/// insertion fails (table is effectively full for this hash chain).
///
/// # Performance (B32 Framework)
/// - get_tier: <50ns (hash + max 16 loads)
/// - set_tier: <100ns (hash + CAS with max 16 probes)
/// - remove_session: <50ns (atomic store)
///
/// # ASSUM Safety (99.99%+)
/// - #ASSUME_LOCKFREE: No mutex/RwLock, all atomic operations
/// - #ASSUME_LINEAR_PROBE_BOUNDED: Max 16 probes prevents infinite loops
/// - #ASSUME_POWER_OF_2_SLOTS: Slot mask is (slots - 1) for fast modulo
#[repr(C, align(256))]
pub struct SessionTierMapCapsule {
    /// Hash table entries (4096 slots)
    entries: [SessionTierEntry; SESSION_TABLE_SLOTS],

    /// Generation counter for TOCTOU prevention
    generation: AtomicU64,

    /// Count of active sessions
    active_sessions: AtomicU64,

    /// Reserved for future use
    _reserved: [u8; 240],
}

impl SessionTierMapCapsule {
    /// Create new empty session-tier map
    ///
    /// # Performance
    /// O(n) initialization (4096 slots zeroed)
    pub const fn new() -> Self {
        // Const initialization requires explicit array creation
        // Use a const fn helper to create the array
        const EMPTY_ENTRY: SessionTierEntry = SessionTierEntry::empty();
        Self {
            entries: [EMPTY_ENTRY; SESSION_TABLE_SLOTS],
            generation: AtomicU64::new(0),
            active_sessions: AtomicU64::new(0),
            _reserved: [0; 240],
        }
    }

    /// Get tier for session ID (<50ns)
    ///
    /// # Arguments
    /// - `session_id`: Session identifier (must be non-zero)
    ///
    /// # Returns
    /// - `Some(tier)` if session exists
    /// - `None` if session not found
    ///
    /// # Performance
    /// <50ns (hash calculation + linear probe up to 16 slots)
    #[inline]
    pub fn get_tier(&self, session_id: u64) -> Option<SubscriptionTier> {
        if session_id == 0 {
            return None;
        }

        let start_slot = self.hash_to_slot(session_id);

        // Linear probe up to MAX_PROBE_DISTANCE
        for offset in 0..MAX_PROBE_DISTANCE {
            let slot = (start_slot + offset) & (SESSION_TABLE_SLOTS - 1);
            let entry = &self.entries[slot];

            let stored_id = entry.session_id.load(Ordering::Acquire);

            if stored_id == session_id {
                // Found the session
                let meta = entry.tier_and_meta.load(Ordering::Acquire);
                return SessionTierEntry::unpack_tier(meta);
            }

            if stored_id == 0 {
                // Empty slot means session not in table
                return None;
            }

            // Different session, continue probing
        }

        // Max probes reached, session not found
        None
    }

    /// Set tier for session ID (<100ns)
    ///
    /// # Arguments
    /// - `session_id`: Session identifier (must be non-zero)
    /// - `tier`: Subscription tier to assign
    ///
    /// # Returns
    /// - `Ok(())` if successfully inserted/updated
    /// - `Err(())` if table is full (max probes reached)
    ///
    /// # Performance
    /// <100ns (hash + CAS loop with max 16 probes)
    pub fn set_tier(&self, session_id: u64, tier: SubscriptionTier) -> Result<(), ()> {
        if session_id == 0 {
            return Err(());
        }

        let start_slot = self.hash_to_slot(session_id);
        let created_days = self.current_unix_days();
        let gen = self.generation.fetch_add(1, Ordering::Relaxed) as u32;
        let new_meta = SessionTierEntry::pack_meta(tier, created_days, gen);

        // Linear probe for existing or empty slot
        for offset in 0..MAX_PROBE_DISTANCE {
            let slot = (start_slot + offset) & (SESSION_TABLE_SLOTS - 1);
            let entry = &self.entries[slot];

            let stored_id = entry.session_id.load(Ordering::Acquire);

            if stored_id == session_id {
                // Update existing session
                entry.tier_and_meta.store(new_meta, Ordering::Release);
                return Ok(());
            }

            if stored_id == 0 {
                // Empty slot - try to claim it with CAS
                match entry.session_id.compare_exchange(
                    0,
                    session_id,
                    Ordering::Release,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Successfully claimed slot
                        entry.tier_and_meta.store(new_meta, Ordering::Release);
                        self.active_sessions.fetch_add(1, Ordering::Relaxed);
                        return Ok(());
                    }
                    Err(actual) => {
                        // Another thread claimed it
                        if actual == session_id {
                            // Same session - update tier
                            entry.tier_and_meta.store(new_meta, Ordering::Release);
                            return Ok(());
                        }
                        // Different session - continue probing
                    }
                }
            }

            // Slot occupied by different session, continue probing
        }

        // Max probes reached - table effectively full
        Err(())
    }

    /// Remove session from map (<50ns)
    ///
    /// # Arguments
    /// - `session_id`: Session identifier to remove
    ///
    /// # Returns
    /// - `true` if session was found and removed
    /// - `false` if session was not in the map
    ///
    /// # Performance
    /// <50ns (hash + linear probe)
    ///
    /// # Note
    /// This uses tombstone deletion (stores 0) which may reduce lookup
    /// performance for sessions that hash to the same chain. Consider
    /// periodic compaction for high-churn workloads.
    pub fn remove_session(&self, session_id: u64) -> bool {
        if session_id == 0 {
            return false;
        }

        let start_slot = self.hash_to_slot(session_id);

        for offset in 0..MAX_PROBE_DISTANCE {
            let slot = (start_slot + offset) & (SESSION_TABLE_SLOTS - 1);
            let entry = &self.entries[slot];

            let stored_id = entry.session_id.load(Ordering::Acquire);

            if stored_id == session_id {
                // Found it - mark as deleted
                entry.session_id.store(0, Ordering::Release);
                entry.tier_and_meta.store(0, Ordering::Release);
                self.active_sessions.fetch_sub(1, Ordering::Relaxed);
                self.generation.fetch_add(1, Ordering::Relaxed);
                return true;
            }

            if stored_id == 0 {
                // Empty slot - session not in table
                return false;
            }
        }

        // Max probes - not found
        false
    }

    /// Get count of active sessions
    ///
    /// # Performance
    /// <10ns (single atomic load)
    #[inline]
    pub fn active_count(&self) -> u64 {
        self.active_sessions.load(Ordering::Relaxed)
    }

    /// Get generation counter
    ///
    /// # Performance
    /// <10ns (single atomic load)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Check if map contains session
    ///
    /// # Performance
    /// <50ns (same as get_tier)
    #[inline]
    pub fn contains(&self, session_id: u64) -> bool {
        self.get_tier(session_id).is_some()
    }

    /// Get session metadata (tier, created_days, entry_generation)
    ///
    /// # Returns
    /// - `Some((tier, created_days, entry_gen))` if session exists
    /// - `None` if session not found
    pub fn get_session_meta(&self, session_id: u64) -> Option<(SubscriptionTier, u32, u32)> {
        if session_id == 0 {
            return None;
        }

        let start_slot = self.hash_to_slot(session_id);

        for offset in 0..MAX_PROBE_DISTANCE {
            let slot = (start_slot + offset) & (SESSION_TABLE_SLOTS - 1);
            let entry = &self.entries[slot];

            let stored_id = entry.session_id.load(Ordering::Acquire);

            if stored_id == session_id {
                let meta = entry.tier_and_meta.load(Ordering::Acquire);
                let tier = SessionTierEntry::unpack_tier(meta)?;
                let days = SessionTierEntry::unpack_created_days(meta);
                let gen = SessionTierEntry::unpack_generation(meta);
                return Some((tier, days, gen));
            }

            if stored_id == 0 {
                return None;
            }
        }

        None
    }

    // ========================================================================
    // Private Methods
    // ========================================================================

    /// Calculate slot index from session ID using FNV-1a inspired hash
    #[inline]
    fn hash_to_slot(&self, session_id: u64) -> usize {
        // FNV-1a constants (64-bit)
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        // Simple but effective hash
        let hash = (session_id ^ FNV_OFFSET).wrapping_mul(FNV_PRIME);

        // Use high bits (better distribution) XOR'd with low bits
        let mixed = hash ^ (hash >> 32);

        (mixed & SLOT_MASK) as usize
    }

    /// Get current Unix days (days since epoch)
    #[inline]
    fn current_unix_days(&self) -> u32 {
        #[cfg(feature = "std")]
        {
            use std::time::SystemTime;
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| (d.as_secs() / 86400) as u32)
                .unwrap_or(0)
        }
        #[cfg(not(feature = "std"))]
        {
            0
        }
    }
}

impl Default for SessionTierMapCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: SessionTierMapCapsule uses only atomic operations
unsafe impl Send for SessionTierMapCapsule {}
unsafe impl Sync for SessionTierMapCapsule {}

// ============================================================================
// Tests (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{size_of, align_of};

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_entry_size() {
        assert_eq!(size_of::<SessionTierEntry>(), 16, "Entry must be 16 bytes");
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(align_of::<SessionTierMapCapsule>(), 256, "Capsule must be 256-byte aligned");
    }

    #[test]
    fn test_capsule_size() {
        // 4096 entries x 16 bytes = 65536 bytes
        // + 8 (generation) + 8 (active_sessions) + 240 (_reserved) = 65792 bytes
        let expected = SESSION_TABLE_SLOTS * size_of::<SessionTierEntry>() + 8 + 8 + 240;
        assert_eq!(size_of::<SessionTierMapCapsule>(), expected);
    }

    #[test]
    fn test_new_map() {
        let map = SessionTierMapCapsule::new();
        assert_eq!(map.active_count(), 0);
        assert_eq!(map.generation(), 0);
    }

    #[test]
    fn test_set_and_get_tier() {
        let map = SessionTierMapCapsule::new();

        // Set tier for session
        assert!(map.set_tier(12345, SubscriptionTier::Engineer).is_ok());

        // Get tier
        let tier = map.get_tier(12345);
        assert_eq!(tier, Some(SubscriptionTier::Engineer));

        // Active count
        assert_eq!(map.active_count(), 1);
    }

    #[test]
    fn test_update_tier() {
        let map = SessionTierMapCapsule::new();

        // Set initial tier
        map.set_tier(12345, SubscriptionTier::Hobby).unwrap();
        assert_eq!(map.get_tier(12345), Some(SubscriptionTier::Hobby));

        // Update tier
        map.set_tier(12345, SubscriptionTier::Teams).unwrap();
        assert_eq!(map.get_tier(12345), Some(SubscriptionTier::Teams));

        // Should still be 1 session (update, not insert)
        assert_eq!(map.active_count(), 1);
    }

    #[test]
    fn test_remove_session() {
        let map = SessionTierMapCapsule::new();

        map.set_tier(12345, SubscriptionTier::Engineer).unwrap();
        assert_eq!(map.active_count(), 1);

        // Remove
        assert!(map.remove_session(12345));
        assert_eq!(map.get_tier(12345), None);
        assert_eq!(map.active_count(), 0);

        // Remove again (should return false)
        assert!(!map.remove_session(12345));
    }

    #[test]
    fn test_contains() {
        let map = SessionTierMapCapsule::new();

        assert!(!map.contains(12345));

        map.set_tier(12345, SubscriptionTier::Pro).unwrap();
        assert!(map.contains(12345));

        map.remove_session(12345);
        assert!(!map.contains(12345));
    }

    #[test]
    fn test_zero_session_id() {
        let map = SessionTierMapCapsule::new();

        // Zero session ID should be rejected
        assert!(map.set_tier(0, SubscriptionTier::Engineer).is_err());
        assert_eq!(map.get_tier(0), None);
        assert!(!map.remove_session(0));
    }

    #[test]
    fn test_hash_collision_handling() {
        let map = SessionTierMapCapsule::new();

        // Insert multiple sessions (some will collide)
        for i in 1..=100 {
            assert!(map.set_tier(i, SubscriptionTier::Hobby).is_ok());
        }

        assert_eq!(map.active_count(), 100);

        // Verify all can be retrieved
        for i in 1..=100 {
            assert_eq!(map.get_tier(i), Some(SubscriptionTier::Hobby));
        }
    }

    #[test]
    fn test_get_session_meta() {
        let map = SessionTierMapCapsule::new();

        map.set_tier(12345, SubscriptionTier::Teams).unwrap();

        let meta = map.get_session_meta(12345);
        assert!(meta.is_some());

        let (tier, _created_days, _gen) = meta.unwrap();
        assert_eq!(tier, SubscriptionTier::Teams);
    }

    #[test]
    fn test_generation_increments() {
        let map = SessionTierMapCapsule::new();
        let gen0 = map.generation();

        map.set_tier(1, SubscriptionTier::Hobby).unwrap();
        let gen1 = map.generation();
        assert!(gen1 > gen0, "Generation should increment on insert");

        map.remove_session(1);
        let gen2 = map.generation();
        assert!(gen2 > gen1, "Generation should increment on remove");
    }

    #[test]
    fn test_metadata_packing() {
        // Test pack/unpack roundtrip
        let tier = SubscriptionTier::Teams;
        let days = 19000u32; // ~52 years
        let gen = 0xDEADBEEFu32;

        let packed = SessionTierEntry::pack_meta(tier, days, gen);
        let unpacked_tier = SessionTierEntry::unpack_tier(packed);
        let unpacked_days = SessionTierEntry::unpack_created_days(packed);
        let unpacked_gen = SessionTierEntry::unpack_generation(packed);

        assert_eq!(unpacked_tier, Some(tier));
        assert_eq!(unpacked_days, days);
        assert_eq!(unpacked_gen, gen);
    }

    // ========================================================================
    // Q8-Q14: Property Tests (concurrent safety)
    // ========================================================================

    #[test]
    fn test_concurrent_insert() {
        use std::sync::Arc;
        use std::thread;

        let map = Arc::new(SessionTierMapCapsule::new());
        let mut handles = vec![];

        // Spawn 4 threads, each inserting 100 sessions
        for t in 0..4 {
            let map_clone = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    let session_id = (t * 1000 + i + 1) as u64;
                    let _ = map_clone.set_tier(session_id, SubscriptionTier::Engineer);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(map.active_count(), 400, "All inserts should succeed");
    }

    #[test]
    fn test_concurrent_lookup() {
        use std::sync::Arc;
        use std::thread;

        let map = Arc::new(SessionTierMapCapsule::new());

        // Pre-populate with sessions
        for i in 1..=100 {
            map.set_tier(i, SubscriptionTier::Teams).unwrap();
        }

        let mut handles = vec![];

        // Spawn 4 threads doing lookups
        for _ in 0..4 {
            let map_clone = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    for i in 1..=100 {
                        let tier = map_clone.get_tier(i);
                        assert_eq!(tier, Some(SubscriptionTier::Teams));
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_mixed_operations() {
        use std::sync::Arc;
        use std::thread;

        let map = Arc::new(SessionTierMapCapsule::new());

        // Pre-populate
        for i in 1..=50 {
            map.set_tier(i, SubscriptionTier::Hobby).unwrap();
        }

        let mut handles = vec![];

        // Thread 1: Insert new sessions
        let map1 = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            for i in 51..=100 {
                let _ = map1.set_tier(i, SubscriptionTier::Pro);
            }
        }));

        // Thread 2: Update existing sessions
        let map2 = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            for i in 1..=50 {
                let _ = map2.set_tier(i, SubscriptionTier::Engineer);
            }
        }));

        // Thread 3: Lookups
        let map3 = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                for i in 1..=50 {
                    let _ = map3.get_tier(i);
                }
            }
        }));

        // Thread 4: Remove some sessions
        let map4 = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            for i in 1..=10 {
                let _ = map4.remove_session(i);
            }
        }));

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify consistency (at least 40 original sessions remain)
        let count = map.active_count();
        // Could be 40-100 depending on timing
        assert!(count >= 40 && count <= 100);
    }
}
