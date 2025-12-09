//! LockCapsule - T1 Atomic coordination for git repository access.
//!
//! Uses DualAtomicU64 pattern with generation counters for TOCTOU prevention
//! and heartbeat tracking for stale lock detection.
//!
//! # Layout (64 bytes, cache-aligned)
//! ```text
//! [0-7]   Primary: owner_id (u32) | generation (u32)
//! [8-15]  Secondary: last_heartbeat (u64)
//! [16-23] Sequence number (u64)
//! [24-63] Padding (40 bytes)
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::Path;
use crate::error::{LockError, Result};

/// Lock capsule for git coordination
///
/// # T1 Atomic Properties
/// - Cache-aligned (64B)
/// - Generation counters (TOCTOU prevention)
/// - Heartbeat tracking (stale detection)
/// - 100% lockfree (CAS-only)
#[repr(C, align(64))]
pub struct LockCapsule {
    /// Primary atomic: owner_id (upper 32) | generation (lower 32)
    primary: AtomicU64,

    /// Secondary atomic: last heartbeat timestamp (seconds since epoch)
    heartbeat: AtomicU64,

    /// Sequence number for deterministic ordering (Q34)
    sequence: AtomicU64,

    /// Padding to 64 bytes
    _padding: [u8; 40],
}

const HEARTBEAT_TIMEOUT_SECS: u64 = 30;

impl LockCapsule {
    /// Load or create lock capsule from mmap
    pub fn load_or_create(path: &Path) -> Result<Self> {
        // TODO: Implement mmap persistence with atomic_capsule::mmap::CapsuleMmapRegion
        // For now, create in-memory
        Ok(Self::new())
    }

    /// Create new lock capsule (unlocked state)
    pub fn new() -> Self {
        Self {
            primary: AtomicU64::new(0),
            heartbeat: AtomicU64::new(0),
            sequence: AtomicU64::new(0),
            _padding: [0; 40],
        }
    }

    /// Try to acquire lock
    ///
    /// Returns sequence number on success
    pub fn try_acquire(&self, instance_id: u32, generation: u32) -> std::result::Result<u64, LockError> {
        let current = self.primary.load(Ordering::Acquire);
        let current_owner = (current >> 32) as u32;
        let current_gen = current as u32;

        // Check if already held
        if current_owner != 0 {
            // Check if stale
            if self.is_stale() {
                return Err(LockError::Stale(current_owner));
            }
            return Err(LockError::Held(current_owner, current_gen));
        }

        // Try to acquire
        let new_value = ((instance_id as u64) << 32) | (generation as u64);

        match self.primary.compare_exchange(
            0,
            new_value,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Update heartbeat
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                self.heartbeat.store(now, Ordering::Release);

                // Increment sequence
                let seq = self.sequence.fetch_add(1, Ordering::AcqRel);
                Ok(seq)
            }
            Err(actual) => {
                let actual_owner = (actual >> 32) as u32;
                let actual_gen = actual as u32;
                Err(LockError::Held(actual_owner, actual_gen))
            }
        }
    }

    /// Release lock
    pub fn release(&self) {
        self.primary.store(0, Ordering::Release);
    }

    /// Force release (for stale lock recovery)
    pub fn force_release(&self) {
        self.primary.store(0, Ordering::Release);
        self.heartbeat.store(0, Ordering::Release);
    }

    /// Update heartbeat
    pub fn update_heartbeat(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.heartbeat.store(now, Ordering::Release);
    }

    /// Check if lock is stale
    pub fn is_stale(&self) -> bool {
        let last_heartbeat = self.heartbeat.load(Ordering::Acquire);
        if last_heartbeat == 0 {
            return false;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        now - last_heartbeat > HEARTBEAT_TIMEOUT_SECS
    }

    /// Get current owner (if any)
    ///
    /// Returns (instance_id, generation) or None if unlocked
    pub fn owner(&self) -> Option<(u32, u32)> {
        let current = self.primary.load(Ordering::Acquire);
        let owner_id = (current >> 32) as u32;
        let generation = current as u32;

        if owner_id == 0 {
            None
        } else {
            Some((owner_id, generation))
        }
    }

    /// Get lock age in seconds
    pub fn age_seconds(&self) -> u64 {
        let last_heartbeat = self.heartbeat.load(Ordering::Acquire);
        if last_heartbeat == 0 {
            return u64::MAX;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        now - last_heartbeat
    }

    /// Get last sequence number
    pub fn last_sequence(&self) -> Option<u64> {
        let seq = self.sequence.load(Ordering::Acquire);
        if seq == 0 {
            None
        } else {
            Some(seq - 1)
        }
    }
}

impl Default for LockCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Verification (compile-time safety)
const _: () = {
    assert!(std::mem::size_of::<LockCapsule>() == 64);
    assert!(std::mem::align_of::<LockCapsule>() == 64);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_new() {
        let lock = LockCapsule::new();
        assert_eq!(lock.owner(), None);
    }

    #[test]
    fn test_lock_acquire_release() {
        let lock = LockCapsule::new();

        // Acquire
        let seq = lock.try_acquire(1, 1).unwrap();
        assert_eq!(seq, 0);
        assert_eq!(lock.owner(), Some((1, 1)));

        // Release
        lock.release();
        assert_eq!(lock.owner(), None);
    }

    #[test]
    fn test_lock_contention() {
        let lock = LockCapsule::new();

        // First acquire
        lock.try_acquire(1, 1).unwrap();

        // Second acquire fails
        let err = lock.try_acquire(2, 1).unwrap_err();
        assert!(matches!(err, LockError::Held(1, 1)));
    }

    #[test]
    fn test_heartbeat() {
        let lock = LockCapsule::new();

        lock.try_acquire(1, 1).unwrap();
        lock.update_heartbeat();

        assert!(!lock.is_stale());
        assert!(lock.age_seconds() < 5);
    }
}
