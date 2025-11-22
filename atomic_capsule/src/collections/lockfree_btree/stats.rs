//! # BTreeStatsCapsule - Lockfree B-tree Statistics
//!
//! Observability capsule for B-tree performance monitoring.

use core::sync::atomic::{AtomicU64, Ordering};

/// BTreeStatsCapsule - Lockfree statistics tracking (128B cache-aligned)
///
/// # Memory Layout
/// ```text
/// Offset 0-7:   inserts (AtomicU64) - Total insert operations
/// Offset 8-15:  gets (AtomicU64) - Total get operations
/// Offset 16-23: removes (AtomicU64) - Total remove operations
/// Offset 24-31: splits (AtomicU64) - Total node splits
/// Offset 32-39: merges (AtomicU64) - Total node merges
/// Offset 40-47: height (AtomicU64) - Current tree height
/// Offset 48-127: _padding - Complete 128B alignment
/// ```
///
/// # Ordering
/// All operations use Relaxed ordering (observability only, no synchronization required)
///
/// # Performance
/// - Increment: <5ns (Relaxed atomic fetch_add)
/// - Read: <5ns (Relaxed atomic load)
/// - Zero contention: Each metric is separate cache line word
///
/// # Safety
/// - `#[repr(C, align(128))]` guarantees layout and alignment
/// - Relaxed ordering safe for statistics (eventual consistency acceptable)
///
/// NOTE: Cannot use derive(ComputationalCapsule) on generic-free struct
/// Using manual verification below
#[repr(C, align(128))]
pub struct BTreeStatsCapsule {
    /// Total insert operations
    ///
    /// # Ordering
    /// Relaxed (observability only)
    inserts: AtomicU64,

    /// Total get operations
    ///
    /// # Ordering
    /// Relaxed (observability only)
    gets: AtomicU64,

    /// Total remove operations
    ///
    /// # Ordering
    /// Relaxed (observability only)
    removes: AtomicU64,

    /// Total node splits
    ///
    /// # Ordering
    /// Relaxed (observability only)
    splits: AtomicU64,

    /// Total node merges
    ///
    /// # Ordering
    /// Relaxed (observability only)
    merges: AtomicU64,

    /// Current tree height
    ///
    /// # Ordering
    /// Relaxed (observability only)
    height: AtomicU64,

    /// Padding to complete 128-byte alignment
    /// 6 × 8 = 48 bytes, need 80 bytes padding
    _padding: [u8; 80],
}

// Compile-time verification
#[cfg(not(feature = "derive"))]
crate::verify_capsule_properties!(BTreeStatsCapsule, 128, 128);

impl BTreeStatsCapsule {
    /// Create new statistics capsule
    pub const fn new() -> Self {
        Self {
            inserts: AtomicU64::new(0),
            gets: AtomicU64::new(0),
            removes: AtomicU64::new(0),
            splits: AtomicU64::new(0),
            merges: AtomicU64::new(0),
            height: AtomicU64::new(0),
            _padding: [0u8; 80],
        }
    }

    /// Increment insert counter
    #[inline(always)]
    pub fn inc_inserts(&self) {
        self.inserts.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment get counter
    #[inline(always)]
    pub fn inc_gets(&self) {
        self.gets.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment remove counter
    #[inline(always)]
    pub fn inc_removes(&self) {
        self.removes.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment split counter
    #[inline(always)]
    pub fn inc_splits(&self) {
        self.splits.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment merge counter
    #[inline(always)]
    pub fn inc_merges(&self) {
        self.merges.fetch_add(1, Ordering::Relaxed);
    }

    /// Update tree height
    #[inline(always)]
    pub fn set_height(&self, height: u64) {
        self.height.store(height, Ordering::Relaxed);
    }

    /// Load all statistics
    pub fn snapshot(&self) -> BTreeStats {
        BTreeStats {
            inserts: self.inserts.load(Ordering::Relaxed),
            gets: self.gets.load(Ordering::Relaxed),
            removes: self.removes.load(Ordering::Relaxed),
            splits: self.splits.load(Ordering::Relaxed),
            merges: self.merges.load(Ordering::Relaxed),
            height: self.height.load(Ordering::Relaxed),
        }
    }

    /// Reset all counters
    pub fn reset(&self) {
        self.inserts.store(0, Ordering::Relaxed);
        self.gets.store(0, Ordering::Relaxed);
        self.removes.store(0, Ordering::Relaxed);
        self.splits.store(0, Ordering::Relaxed);
        self.merges.store(0, Ordering::Relaxed);
        // Don't reset height (not a counter)
    }
}

impl Default for BTreeStatsCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of B-tree statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BTreeStats {
    /// Total insert operations
    pub inserts: u64,
    /// Total get operations
    pub gets: u64,
    /// Total remove operations
    pub removes: u64,
    /// Total node splits
    pub splits: u64,
    /// Total node merges
    pub merges: u64,
    /// Current tree height
    pub height: u64,
}

impl BTreeStats {
    /// Total operations
    pub fn total_ops(&self) -> u64 {
        self.inserts + self.gets + self.removes
    }

    /// Read/write ratio
    pub fn read_write_ratio(&self) -> f64 {
        let writes = self.inserts + self.removes;
        if writes == 0 {
            f64::INFINITY
        } else {
            self.gets as f64 / writes as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_new() {
        let stats = BTreeStatsCapsule::new();
        let snapshot = stats.snapshot();

        assert_eq!(snapshot.inserts, 0);
        assert_eq!(snapshot.gets, 0);
        assert_eq!(snapshot.removes, 0);
        assert_eq!(snapshot.splits, 0);
        assert_eq!(snapshot.merges, 0);
        assert_eq!(snapshot.height, 0);
    }

    #[test]
    fn test_stats_increment() {
        let stats = BTreeStatsCapsule::new();

        stats.inc_inserts();
        stats.inc_inserts();
        stats.inc_gets();
        stats.inc_removes();

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.inserts, 2);
        assert_eq!(snapshot.gets, 1);
        assert_eq!(snapshot.removes, 1);
        assert_eq!(snapshot.total_ops(), 4);
    }

    #[test]
    fn test_stats_reset() {
        let stats = BTreeStatsCapsule::new();

        stats.inc_inserts();
        stats.inc_gets();
        stats.set_height(5);

        stats.reset();

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.inserts, 0);
        assert_eq!(snapshot.gets, 0);
        assert_eq!(snapshot.height, 5); // Height not reset
    }

    #[test]
    fn test_alignment() {
        use core::mem::{align_of, size_of};

        assert_eq!(align_of::<BTreeStatsCapsule>(), 128);
        assert_eq!(size_of::<BTreeStatsCapsule>(), 128);
    }
}
