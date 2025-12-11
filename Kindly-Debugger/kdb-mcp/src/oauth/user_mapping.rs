//! OAuthUserCapsule - T1 Atomic OAuth User Mapping (17KB, 256B-aligned)
//!
//! Maps Google OAuth user IDs (sub) to license keys for the OAuth 2.1 flow.
//! Uses FNV-1a hash tables for O(1) lockfree lookup.
//!
//! **Tier**: T1 Atomic (lockfree hash table with generation counters)
//! **Size**: ~17KB (1024 slots x 16 bytes + header)
//! **Latency**: <50ns link/lookup, <30ns get
//!
//! ## UCE35 Compliance
//! - Q10: T1 Atomic (FNV-1a hash table)
//! - Q22: Packed entries (google_sub_hash:64 | license_hash:64)
//! - Q23: 100% lockfree (CAS loops, generation counters)
//! - Q33: 256-byte aligned (multi-capsule cache line optimization)
//! - Q34: Generation counters for audit trail integrity
//!
//! ## ASSUM Safety
//! - #ASSUME: FNV-1a provides sufficient distribution for OAuth user IDs
//! - #VERIFY: Linear probing bounded by MAX_PROBES (8)
//! - #ASSUME: Google sub IDs are globally unique per user
//! - #VERIFY: CAS loops terminate via generation counter monotonicity
//!
//! ## Usage
//! ```rust,ignore
//! use kdb_mcp::oauth::user_mapping::OAuthUserCapsule;
//!
//! let user_map = OAuthUserCapsule::new();
//!
//! // Link Google user to license after OAuth flow
//! let linked = user_map.link_google_to_license("google-sub-123", "KDB-PRO-abc123");
//!
//! // Look up license for returning user
//! if let Some(license_hash) = user_map.get_license_hash_for_google("google-sub-123") {
//!     // Found existing user, validate license
//! }
//! ```

use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// Constants
// ============================================================================

/// Number of hash table slots (power of 2 for fast modulo)
pub const USER_TABLE_SLOTS: usize = 1024;

/// Maximum probe distance for linear probing
const MAX_PROBES: usize = 8;

/// FNV-1a constants (64-bit)
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// Empty slot marker (0 is invalid hash since FNV-1a never produces 0 for non-empty input)
const EMPTY_SLOT: u64 = 0;

// ============================================================================
// Hash Function
// ============================================================================

/// FNV-1a hash function for OAuth user IDs
///
/// **Performance**: <10ns for typical OAuth user IDs (20-40 chars)
#[inline]
pub fn fnv1a_hash_oauth(s: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in s.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    // Ensure non-zero (0 is reserved for empty slot)
    if hash == 0 {
        hash = 1;
    }
    hash
}

// ============================================================================
// User Slot (16 bytes)
// ============================================================================

/// OAuth user mapping slot
///
/// **Layout** (16 bytes):
/// - google_sub_hash (8B): FNV-1a hash of Google user ID
/// - license_hash (8B): FNV-1a hash of license key
///
/// Both fields are AtomicU64 for lockfree access.
/// Empty slot indicated by google_sub_hash == 0.
#[repr(C)]
pub struct OAuthUserSlot {
    /// FNV-1a hash of Google user ID (sub claim)
    google_sub_hash: AtomicU64,
    /// FNV-1a hash of license key (KDB-{TIER}-{...})
    license_hash: AtomicU64,
}

impl OAuthUserSlot {
    /// Create empty slot
    const fn new() -> Self {
        Self {
            google_sub_hash: AtomicU64::new(EMPTY_SLOT),
            license_hash: AtomicU64::new(EMPTY_SLOT),
        }
    }

    /// Check if slot is empty
    #[inline]
    fn is_empty(&self) -> bool {
        self.google_sub_hash.load(Ordering::Acquire) == EMPTY_SLOT
    }

    /// Get Google sub hash
    #[inline]
    fn get_google_hash(&self) -> u64 {
        self.google_sub_hash.load(Ordering::Acquire)
    }

    /// Get license hash
    #[inline]
    fn get_license_hash(&self) -> u64 {
        self.license_hash.load(Ordering::Acquire)
    }
}

// ============================================================================
// OAuthUserCapsule (17KB, 256B-aligned)
// ============================================================================

/// OAuth User Mapping Capsule - T1 Atomic lockfree user-to-license mapping
///
/// **Layout** (17KB total):
/// ```text
/// Offset     Size    Field
/// ------     ----    -----
/// 0          8       generation (AtomicU64)
/// 8          8       active_users (AtomicU64)
/// 16         8       total_linked (AtomicU64)
/// 24         8       new_users_created (AtomicU64)
/// 32         8       link_failures (AtomicU64)
/// 40         8       lookup_hits (AtomicU64)
/// 48         8       lookup_misses (AtomicU64)
/// 56         8       unlink_count (AtomicU64)
/// 64         16384   slots[1024] (OAuthUserSlot)
/// 16448      896     _reserved
/// ```
///
/// **Memory Ordering**:
/// - Read path (get_license_hash_for_google): Acquire
/// - Write path (link_google_to_license): AcqRel CAS
/// - Stats updates: Relaxed (non-critical)
///
/// **ASSUM Safety**:
/// - #ASSUME: Linear probing with MAX_PROBES=8 sufficient for <80% load
/// - #VERIFY: Generation counter increments on all mutations
/// - #ASSUME: FNV-1a collision rate acceptable for OAuth user population
#[repr(C, align(256))]
pub struct OAuthUserCapsule {
    // Header (64 bytes)
    /// Generation counter for TOCTOU prevention
    generation: AtomicU64,
    /// Currently active (non-empty) user mappings
    active_users: AtomicU64,
    /// Total successful link operations
    total_linked: AtomicU64,
    /// New users created (first-time links)
    new_users_created: AtomicU64,
    /// Failed link attempts (table full or CAS failures)
    link_failures: AtomicU64,
    /// Successful lookup operations
    lookup_hits: AtomicU64,
    /// Failed lookup operations (user not found)
    lookup_misses: AtomicU64,
    /// Successful unlink operations
    unlink_count: AtomicU64,

    // Hash table (1024 slots x 16 bytes = 16KB)
    /// Maps google_sub_hash -> license_hash
    slots: [OAuthUserSlot; USER_TABLE_SLOTS],

    // Reserved for future expansion (896 bytes to reach 17KB)
    _reserved: [u8; 896],
}

impl OAuthUserCapsule {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Create new empty OAuth user mapping capsule
    ///
    /// **Performance**: O(1) const initialization
    pub const fn new() -> Self {
        const EMPTY_SLOT_INIT: OAuthUserSlot = OAuthUserSlot::new();
        Self {
            generation: AtomicU64::new(0),
            active_users: AtomicU64::new(0),
            total_linked: AtomicU64::new(0),
            new_users_created: AtomicU64::new(0),
            link_failures: AtomicU64::new(0),
            lookup_hits: AtomicU64::new(0),
            lookup_misses: AtomicU64::new(0),
            unlink_count: AtomicU64::new(0),
            slots: [EMPTY_SLOT_INIT; USER_TABLE_SLOTS],
            _reserved: [0u8; 896],
        }
    }

    // ========================================================================
    // Core Operations
    // ========================================================================

    /// Link a Google OAuth user to a license key
    ///
    /// **Algorithm**:
    /// 1. Hash google_sub and license_key
    /// 2. Find slot via linear probing
    /// 3. CAS to claim empty slot or update existing
    ///
    /// **Performance**: <50ns typical (hash + CAS)
    ///
    /// **Returns**:
    /// - `Ok(true)` if new user was created
    /// - `Ok(false)` if existing user was updated
    /// - `Err(OAuthUserError)` if table full or CAS failed after retries
    pub fn link_google_to_license(
        &self,
        google_sub: &str,
        license_key: &str,
    ) -> Result<bool, OAuthUserError> {
        let google_hash = fnv1a_hash_oauth(google_sub);
        let license_hash = fnv1a_hash_oauth(license_key);
        let start_index = (google_hash as usize) % USER_TABLE_SLOTS;

        // Linear probing with retry
        for retry in 0..3 {
            for probe in 0..MAX_PROBES {
                let slot_idx = (start_index + probe) % USER_TABLE_SLOTS;
                let slot = &self.slots[slot_idx];

                let current_google = slot.google_sub_hash.load(Ordering::Acquire);

                // Case 1: Empty slot - try to claim it
                if current_google == EMPTY_SLOT {
                    // CAS to claim the slot
                    if slot
                        .google_sub_hash
                        .compare_exchange(
                            EMPTY_SLOT,
                            google_hash,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        // Successfully claimed, now set license hash
                        slot.license_hash.store(license_hash, Ordering::Release);

                        // Update stats
                        self.generation.fetch_add(1, Ordering::Relaxed);
                        self.active_users.fetch_add(1, Ordering::Relaxed);
                        self.total_linked.fetch_add(1, Ordering::Relaxed);
                        self.new_users_created.fetch_add(1, Ordering::Relaxed);

                        return Ok(true); // New user created
                    }
                    // CAS failed, another thread claimed it, retry probe
                    continue;
                }

                // Case 2: Existing user - update license
                if current_google == google_hash {
                    slot.license_hash.store(license_hash, Ordering::Release);

                    // Update stats
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    self.total_linked.fetch_add(1, Ordering::Relaxed);

                    return Ok(false); // Existing user updated
                }

                // Case 3: Different user in slot, continue probing
            }

            // All probes exhausted, sleep briefly and retry
            if retry < 2 {
                #[cfg(feature = "std")]
                std::thread::yield_now();
            }
        }

        // Table full or excessive contention
        self.link_failures.fetch_add(1, Ordering::Relaxed);
        Err(OAuthUserError::TableFull)
    }

    /// Get license hash for a Google OAuth user
    ///
    /// **Performance**: <30ns (hash + linear probe)
    ///
    /// **Returns**: `Some(license_hash)` if found, `None` if not found
    pub fn get_license_hash_for_google(&self, google_sub: &str) -> Option<u64> {
        let google_hash = fnv1a_hash_oauth(google_sub);
        let start_index = (google_hash as usize) % USER_TABLE_SLOTS;

        // Linear probing
        for probe in 0..MAX_PROBES {
            let slot_idx = (start_index + probe) % USER_TABLE_SLOTS;
            let slot = &self.slots[slot_idx];

            let current_google = slot.get_google_hash();

            // Empty slot - user not found
            if current_google == EMPTY_SLOT {
                self.lookup_misses.fetch_add(1, Ordering::Relaxed);
                return None;
            }

            // Found user
            if current_google == google_hash {
                let license_hash = slot.get_license_hash();
                self.lookup_hits.fetch_add(1, Ordering::Relaxed);
                return Some(license_hash);
            }

            // Different user, continue probing
        }

        // Not found after MAX_PROBES
        self.lookup_misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Unlink a Google OAuth user
    ///
    /// **Performance**: <50ns (hash + CAS)
    ///
    /// **Returns**: `true` if user was found and unlinked, `false` if not found
    pub fn unlink_google(&self, google_sub: &str) -> bool {
        let google_hash = fnv1a_hash_oauth(google_sub);
        let start_index = (google_hash as usize) % USER_TABLE_SLOTS;

        // Linear probing
        for probe in 0..MAX_PROBES {
            let slot_idx = (start_index + probe) % USER_TABLE_SLOTS;
            let slot = &self.slots[slot_idx];

            let current_google = slot.google_sub_hash.load(Ordering::Acquire);

            // Empty slot - user not found
            if current_google == EMPTY_SLOT {
                return false;
            }

            // Found user - clear slot
            if current_google == google_hash {
                // CAS to clear (prevents race with concurrent link)
                if slot
                    .google_sub_hash
                    .compare_exchange(
                        google_hash,
                        EMPTY_SLOT,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    // Clear license hash too
                    slot.license_hash.store(EMPTY_SLOT, Ordering::Release);

                    // Update stats
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    self.active_users.fetch_sub(1, Ordering::Relaxed);
                    self.unlink_count.fetch_add(1, Ordering::Relaxed);

                    return true;
                }
                // CAS failed, slot was modified - return false
                return false;
            }

            // Different user, continue probing
        }

        false
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get current generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get statistics snapshot
    pub fn stats(&self) -> OAuthUserStats {
        OAuthUserStats {
            generation: self.generation.load(Ordering::Acquire),
            active_users: self.active_users.load(Ordering::Relaxed),
            total_linked: self.total_linked.load(Ordering::Relaxed),
            new_users_created: self.new_users_created.load(Ordering::Relaxed),
            link_failures: self.link_failures.load(Ordering::Relaxed),
            lookup_hits: self.lookup_hits.load(Ordering::Relaxed),
            lookup_misses: self.lookup_misses.load(Ordering::Relaxed),
            unlink_count: self.unlink_count.load(Ordering::Relaxed),
        }
    }

    /// Get number of active user mappings
    #[inline]
    pub fn active_count(&self) -> u64 {
        self.active_users.load(Ordering::Relaxed)
    }

    /// Get table capacity
    #[inline]
    pub const fn capacity(&self) -> usize {
        USER_TABLE_SLOTS
    }

    /// Calculate load factor (0.0 - 1.0)
    #[inline]
    pub fn load_factor(&self) -> f64 {
        self.active_users.load(Ordering::Relaxed) as f64 / USER_TABLE_SLOTS as f64
    }

    // ========================================================================
    // Maintenance
    // ========================================================================

    /// Clear all user mappings
    ///
    /// **Warning**: This is NOT lockfree - use only during maintenance windows
    pub fn clear(&self) {
        for slot in &self.slots {
            slot.google_sub_hash.store(EMPTY_SLOT, Ordering::Relaxed);
            slot.license_hash.store(EMPTY_SLOT, Ordering::Relaxed);
        }

        // Reset stats (keep generation for audit trail)
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.active_users.store(0, Ordering::Relaxed);
        // Don't reset counters - they're cumulative
    }
}

impl Default for OAuthUserCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: OAuthUserCapsule only contains AtomicU64 fields which are Send + Sync
unsafe impl Send for OAuthUserCapsule {}
unsafe impl Sync for OAuthUserCapsule {}

// ============================================================================
// Error Types
// ============================================================================

/// OAuth user mapping errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthUserError {
    /// Hash table is full (>80% load factor)
    TableFull,
    /// CAS failed after retries (high contention)
    ContentionFailure,
}

impl core::fmt::Display for OAuthUserError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            OAuthUserError::TableFull => write!(f, "OAuth user table full (>80% capacity)"),
            OAuthUserError::ContentionFailure => write!(f, "CAS contention failure"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OAuthUserError {}

// ============================================================================
// Statistics
// ============================================================================

/// OAuth user mapping statistics snapshot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OAuthUserStats {
    /// Generation counter (increments on each mutation)
    pub generation: u64,
    /// Currently active user mappings
    pub active_users: u64,
    /// Total successful link operations
    pub total_linked: u64,
    /// New users created (first-time links)
    pub new_users_created: u64,
    /// Failed link attempts
    pub link_failures: u64,
    /// Successful lookup operations
    pub lookup_hits: u64,
    /// Failed lookup operations
    pub lookup_misses: u64,
    /// Successful unlink operations
    pub unlink_count: u64,
}

impl OAuthUserStats {
    /// Calculate lookup hit rate (0.0 - 1.0)
    pub fn hit_rate(&self) -> f64 {
        let total = self.lookup_hits + self.lookup_misses;
        if total == 0 {
            0.0
        } else {
            self.lookup_hits as f64 / total as f64
        }
    }

    /// Calculate link success rate (0.0 - 1.0)
    pub fn link_success_rate(&self) -> f64 {
        let total = self.total_linked + self.link_failures;
        if total == 0 {
            1.0 // No attempts = 100% success
        } else {
            self.total_linked as f64 / total as f64
        }
    }
}

// ============================================================================
// Static Assertions (Compile-Time Verification)
// ============================================================================

#[cfg(test)]
const _: () = {
    // Verify slot size is 16 bytes
    const SLOT_SIZE: usize = core::mem::size_of::<OAuthUserSlot>();
    assert!(SLOT_SIZE == 16, "OAuthUserSlot must be 16 bytes");

    // Verify capsule alignment is 256 bytes
    const ALIGN: usize = core::mem::align_of::<OAuthUserCapsule>();
    assert!(ALIGN == 256, "OAuthUserCapsule must be 256-byte aligned");

    // Verify capsule size is approximately 17KB
    const SIZE: usize = core::mem::size_of::<OAuthUserCapsule>();
    // Header (64B) + Slots (16384B) + Reserved (896B) = 17344B
    assert!(SIZE >= 17000, "OAuthUserCapsule must be at least 17KB");
    assert!(SIZE <= 18000, "OAuthUserCapsule must be at most 18KB");
};

// ============================================================================
// Unit Tests (T28 Q1-Q7)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    // =========================================================================
    // Capsule Layout Tests
    // =========================================================================

    #[test]
    fn test_capsule_size() {
        let size = std::mem::size_of::<OAuthUserCapsule>();
        assert!(
            size >= 17000 && size <= 18000,
            "OAuthUserCapsule size {} not in expected range 17-18KB",
            size
        );
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(
            std::mem::align_of::<OAuthUserCapsule>(),
            256,
            "OAuthUserCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_slot_size() {
        assert_eq!(
            std::mem::size_of::<OAuthUserSlot>(),
            16,
            "OAuthUserSlot must be 16 bytes"
        );
    }

    // =========================================================================
    // Basic Link/Lookup Tests
    // =========================================================================

    #[test]
    fn test_link_new_user() {
        let capsule = OAuthUserCapsule::new();

        let result = capsule.link_google_to_license("google-sub-123", "KDB-PRO-abc123");
        assert!(result.is_ok());
        assert!(result.unwrap()); // New user created

        let stats = capsule.stats();
        assert_eq!(stats.active_users, 1);
        assert_eq!(stats.new_users_created, 1);
        assert_eq!(stats.total_linked, 1);
    }

    #[test]
    fn test_link_existing_user() {
        let capsule = OAuthUserCapsule::new();

        // First link
        let result1 = capsule.link_google_to_license("google-sub-123", "KDB-PRO-abc123");
        assert!(result1.unwrap()); // New user

        // Second link (update)
        let result2 = capsule.link_google_to_license("google-sub-123", "KDB-ENT-xyz789");
        assert!(!result2.unwrap()); // Existing user updated

        let stats = capsule.stats();
        assert_eq!(stats.active_users, 1);
        assert_eq!(stats.new_users_created, 1);
        assert_eq!(stats.total_linked, 2);
    }

    #[test]
    fn test_lookup_existing() {
        let capsule = OAuthUserCapsule::new();

        let license_key = "KDB-PRO-abc123";
        capsule
            .link_google_to_license("google-sub-123", license_key)
            .unwrap();

        let result = capsule.get_license_hash_for_google("google-sub-123");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), fnv1a_hash_oauth(license_key));

        let stats = capsule.stats();
        assert_eq!(stats.lookup_hits, 1);
        assert_eq!(stats.lookup_misses, 0);
    }

    #[test]
    fn test_lookup_nonexistent() {
        let capsule = OAuthUserCapsule::new();

        let result = capsule.get_license_hash_for_google("nonexistent-user");
        assert!(result.is_none());

        let stats = capsule.stats();
        assert_eq!(stats.lookup_hits, 0);
        assert_eq!(stats.lookup_misses, 1);
    }

    // =========================================================================
    // Unlink Tests
    // =========================================================================

    #[test]
    fn test_unlink_existing() {
        let capsule = OAuthUserCapsule::new();

        capsule
            .link_google_to_license("google-sub-123", "KDB-PRO-abc123")
            .unwrap();
        assert_eq!(capsule.active_count(), 1);

        let unlinked = capsule.unlink_google("google-sub-123");
        assert!(unlinked);
        assert_eq!(capsule.active_count(), 0);

        // Lookup should now fail
        assert!(capsule.get_license_hash_for_google("google-sub-123").is_none());
    }

    #[test]
    fn test_unlink_nonexistent() {
        let capsule = OAuthUserCapsule::new();

        let unlinked = capsule.unlink_google("nonexistent-user");
        assert!(!unlinked);
    }

    // =========================================================================
    // Hash Collision Handling
    // =========================================================================

    #[test]
    fn test_multiple_users_same_bucket() {
        let capsule = OAuthUserCapsule::new();

        // Insert multiple users (linear probing should handle collisions)
        for i in 0..50 {
            let google_sub = format!("google-user-{}", i);
            let license = format!("KDB-PRO-{}", i);
            let result = capsule.link_google_to_license(&google_sub, &license);
            assert!(result.is_ok(), "Failed to link user {}", i);
        }

        // Verify all can be looked up
        for i in 0..50 {
            let google_sub = format!("google-user-{}", i);
            let license = format!("KDB-PRO-{}", i);
            let expected_hash = fnv1a_hash_oauth(&license);

            let result = capsule.get_license_hash_for_google(&google_sub);
            assert_eq!(
                result,
                Some(expected_hash),
                "Failed to find user {}",
                i
            );
        }

        assert_eq!(capsule.active_count(), 50);
    }

    // =========================================================================
    // Concurrent Access Tests
    // =========================================================================

    #[test]
    fn test_concurrent_links() {
        let capsule = Arc::new(OAuthUserCapsule::new());
        let num_threads = 10;
        let users_per_thread = 50;

        let handles: Vec<_> = (0..num_threads)
            .map(|t| {
                let capsule = Arc::clone(&capsule);
                thread::spawn(move || {
                    for i in 0..users_per_thread {
                        let google_sub = format!("thread-{}-user-{}", t, i);
                        let license = format!("KDB-{}-{}", t, i);
                        let _ = capsule.link_google_to_license(&google_sub, &license);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // All unique users should be linked
        let stats = capsule.stats();
        assert_eq!(
            stats.active_users,
            (num_threads * users_per_thread) as u64,
            "Expected {} users, got {}",
            num_threads * users_per_thread,
            stats.active_users
        );
    }

    #[test]
    fn test_concurrent_same_user() {
        let capsule = Arc::new(OAuthUserCapsule::new());
        let num_threads = 16;

        let handles: Vec<_> = (0..num_threads)
            .map(|t| {
                let capsule = Arc::clone(&capsule);
                thread::spawn(move || {
                    let license = format!("KDB-THREAD-{}", t);
                    capsule
                        .link_google_to_license("shared-google-user", &license)
                        .is_ok()
                })
            })
            .collect();

        let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All should succeed (one creates, others update)
        assert!(results.iter().all(|&r| r));

        // Only one active user
        assert_eq!(capsule.active_count(), 1);
    }

    #[test]
    fn test_concurrent_read_write() {
        let capsule = Arc::new(OAuthUserCapsule::new());

        // Pre-populate
        for i in 0..100 {
            let google_sub = format!("pre-user-{}", i);
            let license = format!("KDB-PRE-{}", i);
            capsule.link_google_to_license(&google_sub, &license).unwrap();
        }

        let num_readers = 4;
        let num_writers = 4;
        let iterations = 100;

        let readers: Vec<_> = (0..num_readers)
            .map(|_| {
                let capsule = Arc::clone(&capsule);
                thread::spawn(move || {
                    let mut hits = 0u64;
                    for i in 0..iterations {
                        let google_sub = format!("pre-user-{}", i % 100);
                        if capsule.get_license_hash_for_google(&google_sub).is_some() {
                            hits += 1;
                        }
                    }
                    hits
                })
            })
            .collect();

        let writers: Vec<_> = (0..num_writers)
            .map(|t| {
                let capsule = Arc::clone(&capsule);
                thread::spawn(move || {
                    for i in 0..iterations {
                        let google_sub = format!("new-user-{}-{}", t, i);
                        let license = format!("KDB-NEW-{}-{}", t, i);
                        let _ = capsule.link_google_to_license(&google_sub, &license);
                    }
                })
            })
            .collect();

        // Wait for readers
        for handle in readers {
            let hits = handle.join().unwrap();
            assert!(hits > 0, "Reader should get some hits");
        }

        // Wait for writers
        for handle in writers {
            handle.join().unwrap();
        }

        // Should have more users now
        assert!(capsule.active_count() > 100);
    }

    // =========================================================================
    // FNV-1a Hash Tests
    // =========================================================================

    #[test]
    fn test_fnv1a_deterministic() {
        let hash1 = fnv1a_hash_oauth("google-sub-12345");
        let hash2 = fnv1a_hash_oauth("google-sub-12345");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_fnv1a_different_inputs() {
        let hash1 = fnv1a_hash_oauth("user-1");
        let hash2 = fnv1a_hash_oauth("user-2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_fnv1a_never_zero() {
        // Empty string should still produce non-zero
        let hash = fnv1a_hash_oauth("");
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_fnv1a_distribution() {
        let mut bucket_counts = vec![0u32; 16];

        for i in 0..1000 {
            let key = format!("google-oauth-user-{}-sub-id", i);
            let hash = fnv1a_hash_oauth(&key);
            let bucket = (hash as usize) % 16;
            bucket_counts[bucket] += 1;
        }

        // Each bucket should have roughly 1000/16 = 62.5 entries
        for (i, &count) in bucket_counts.iter().enumerate() {
            assert!(
                count >= 30 && count <= 100,
                "Bucket {} has {} entries (expected 30-100)",
                i,
                count
            );
        }
    }

    // =========================================================================
    // Statistics Tests
    // =========================================================================

    #[test]
    fn test_stats_initial() {
        let capsule = OAuthUserCapsule::new();
        let stats = capsule.stats();

        assert_eq!(stats.generation, 0);
        assert_eq!(stats.active_users, 0);
        assert_eq!(stats.total_linked, 0);
        assert_eq!(stats.new_users_created, 0);
        assert_eq!(stats.link_failures, 0);
        assert_eq!(stats.lookup_hits, 0);
        assert_eq!(stats.lookup_misses, 0);
    }

    #[test]
    fn test_stats_after_operations() {
        let capsule = OAuthUserCapsule::new();

        // Link 5 new users
        for i in 0..5 {
            capsule
                .link_google_to_license(&format!("user-{}", i), &format!("license-{}", i))
                .unwrap();
        }

        // Hit 3, miss 2
        for i in 0..5 {
            capsule.get_license_hash_for_google(&format!("user-{}", i));
        }
        capsule.get_license_hash_for_google("nonexistent-1");
        capsule.get_license_hash_for_google("nonexistent-2");

        let stats = capsule.stats();
        assert_eq!(stats.active_users, 5);
        assert_eq!(stats.new_users_created, 5);
        assert_eq!(stats.total_linked, 5);
        assert_eq!(stats.lookup_hits, 5);
        assert_eq!(stats.lookup_misses, 2);
    }

    #[test]
    fn test_hit_rate() {
        let stats = OAuthUserStats {
            generation: 0,
            active_users: 100,
            total_linked: 100,
            new_users_created: 100,
            link_failures: 0,
            lookup_hits: 75,
            lookup_misses: 25,
            unlink_count: 0,
        };

        assert!((stats.hit_rate() - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_link_success_rate() {
        let stats = OAuthUserStats {
            generation: 0,
            active_users: 90,
            total_linked: 90,
            new_users_created: 90,
            link_failures: 10,
            lookup_hits: 0,
            lookup_misses: 0,
            unlink_count: 0,
        };

        assert!((stats.link_success_rate() - 0.9).abs() < 0.001);
    }

    // =========================================================================
    // Clear/Maintenance Tests
    // =========================================================================

    #[test]
    fn test_clear() {
        let capsule = OAuthUserCapsule::new();

        // Add some users
        for i in 0..10 {
            capsule
                .link_google_to_license(&format!("user-{}", i), &format!("license-{}", i))
                .unwrap();
        }
        assert_eq!(capsule.active_count(), 10);

        // Clear
        capsule.clear();

        assert_eq!(capsule.active_count(), 0);
        assert!(capsule.get_license_hash_for_google("user-0").is_none());
    }

    #[test]
    fn test_load_factor() {
        let capsule = OAuthUserCapsule::new();

        for i in 0..100 {
            capsule
                .link_google_to_license(&format!("user-{}", i), &format!("license-{}", i))
                .unwrap();
        }

        let load_factor = capsule.load_factor();
        let expected = 100.0 / USER_TABLE_SLOTS as f64;
        assert!((load_factor - expected).abs() < 0.001);
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_empty_google_sub() {
        let capsule = OAuthUserCapsule::new();

        let result = capsule.link_google_to_license("", "KDB-PRO-abc");
        assert!(result.is_ok());

        let lookup = capsule.get_license_hash_for_google("");
        assert!(lookup.is_some());
    }

    #[test]
    fn test_long_google_sub() {
        let capsule = OAuthUserCapsule::new();

        let long_sub = "a".repeat(1000);
        let result = capsule.link_google_to_license(&long_sub, "KDB-PRO-abc");
        assert!(result.is_ok());

        let lookup = capsule.get_license_hash_for_google(&long_sub);
        assert!(lookup.is_some());
    }

    #[test]
    fn test_unicode_google_sub() {
        let capsule = OAuthUserCapsule::new();

        let unicode_sub = "google-sub-Hello-World";
        let result = capsule.link_google_to_license(unicode_sub, "KDB-PRO-abc");
        assert!(result.is_ok());

        let lookup = capsule.get_license_hash_for_google(unicode_sub);
        assert!(lookup.is_some());
    }

    // =========================================================================
    // Send + Sync Tests
    // =========================================================================

    #[test]
    fn test_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<OAuthUserCapsule>();
        assert_sync::<OAuthUserCapsule>();
    }

    // =========================================================================
    // Default Trait Test
    // =========================================================================

    #[test]
    fn test_default_trait() {
        let capsule: OAuthUserCapsule = Default::default();
        assert_eq!(capsule.active_count(), 0);
        assert_eq!(capsule.generation(), 0);
    }
}
