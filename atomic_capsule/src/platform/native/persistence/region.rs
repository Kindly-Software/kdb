//! MmapRegion - T1 Atomic Capsule for Lockfree Region Management
//!
//! **UCE34 Framework**: T1 Atomic tier with generation counters
//!
//! # Architecture
//!
//! Fixed-size region within memory-mapped file:
//! - **base_offset**: Base offset in mmap file (immutable)
//! - **capacity**: Region capacity in bytes (immutable)
//! - **allocated**: Current allocation in bytes (atomically incremented)
//! - **generation**: TOCTOU prevention for concurrent access
//!
//! # Performance Targets (B32)
//!
//! - **allocate()**: <20ns lockfree CAS (vs ~50ns memmap2 mutex)
//! - **capacity()/allocated()**: <5ns atomic load
//! - **generation()**: <5ns atomic load
//!
//! # UCE34 Q10-Q34 Validation
//!
//! **Q10**: T1 Atomic - lockfree CAS coordination
//! **Q11**: AtomicU32/AtomicU64 for capacity/allocated/generation
//! **Q12**: Nightly atomic_from_mut integration (future)
//! **Q33**: #[derive(ComputationalCapsule)] - automatic verification
//! **Q34**: Generation counter for audit trail (TOCTOU prevention)
//!
//! # ASSUM Safety
//!
//! #ASSUME_ACQUIRE_RELEASE: Acquire for CAS success ensures visibility
//! #ASSUME_RELAXED_GENERATION: Generation counter uses Release for visibility
//! #ASSUME_CAPACITY_IMMUTABLE: Capacity never changes after initialization

use super::MmapError;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

/// T1 Atomic capsule for lockfree region allocation
///
/// **Alignment**: 64B (cache line aligned)
/// **Tier**: T1 (Atomic)
/// **Speedup**: 3-10× vs memmap2 mutex (20ns vs 50ns target)
/// **Size**: 64B (single cache line)
#[repr(C, align(64))]
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 64))]
#[cfg_attr(
    feature = "derive",
    capsule(alignment = 64, size = 64, tier = "Atomic")
)]
pub struct MmapRegion {
    /// Base offset in mmap file (immutable after init, atomic for Chaos compliance)
    base_offset: AtomicU64,

    /// Region capacity in bytes (immutable)
    capacity: AtomicU32,

    /// Currently allocated bytes (atomically incremented)
    allocated: AtomicU32,

    /// Generation counter (TOCTOU prevention, Q34 audit trail)
    generation: AtomicU64,

    /// Padding to 64B cache line
    _padding: [u8; 40],
}

// SAFETY: MmapRegion is Send/Sync via atomic operations
// All atomic fields use proper memory ordering (Acquire/Release)
// Note: When derive feature is enabled, ComputationalCapsule provides Send/Sync
#[cfg(not(feature = "derive"))]
unsafe impl Send for MmapRegion {}
#[cfg(not(feature = "derive"))]
unsafe impl Sync for MmapRegion {}

impl MmapRegion {
    /// Create new region with base offset and capacity
    ///
    /// **Performance**: <5ns (initialization)
    ///
    /// # Arguments
    ///
    /// * `base_offset` - Base offset in mmap file
    /// * `capacity` - Region capacity in bytes
    ///
    /// #ASSUME_OFFSET_VALID: base_offset must be valid within mmap file
    /// #ASSUME_CAPACITY_VALID: capacity must be ≤ mmap file size - base_offset
    pub fn new(base_offset: u64, capacity: u32) -> Self {
        Self {
            base_offset: AtomicU64::new(base_offset),
            capacity: AtomicU32::new(capacity),
            allocated: AtomicU32::new(0),
            generation: AtomicU64::new(0),
            _padding: [0u8; 40],
        }
    }

    /// Allocate `size` bytes from this region
    ///
    /// **Performance**: <20ns lockfree CAS loop (target)
    ///
    /// Returns absolute offset in mmap file, or error if insufficient space
    ///
    /// # IMPL-2 V3.1: 100% Lockfree
    ///
    /// Uses CAS loop with bounded retries (max 1000 attempts before timeout)
    ///
    /// #ASSUME_ACQUIRE_RELEASE: Acquire on CAS success ensures allocated visibility
    pub fn allocate(&self, size: u32) -> Result<u64, MmapError> {
        const MAX_RETRIES: u32 = 1000;
        let mut retries = 0;

        loop {
            let current = self.allocated.load(Ordering::Acquire);
            let capacity = self.capacity.load(Ordering::Relaxed);

            // Check if allocation fits
            let new_allocated = current.checked_add(size).ok_or_else(|| {
                let available = capacity.saturating_sub(current);
                MmapError::capacity_exceeded(size as usize, available as usize)
            })?;

            if new_allocated > capacity {
                let available = capacity.saturating_sub(current);
                return Err(MmapError::capacity_exceeded(
                    size as usize,
                    available as usize,
                ));
            }

            // Attempt CAS
            match self.allocated.compare_exchange_weak(
                current,
                new_allocated,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Bump generation on successful allocation (Q34 audit trail)
                    self.generation.fetch_add(1, Ordering::Release);

                    // Return absolute offset in mmap file
                    return Ok(self.base_offset.load(Ordering::Relaxed) + current as u64);
                }
                Err(_) => {
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        // Too many retries - treat as capacity exceeded
                        let available = capacity.saturating_sub(current);
                        return Err(MmapError::capacity_exceeded(
                            size as usize,
                            available as usize,
                        ));
                    }
                    // CAS failed, retry
                    continue;
                }
            }
        }
    }

    /// Get region capacity in bytes
    ///
    /// **Performance**: <5ns (atomic load)
    #[inline]
    pub fn capacity(&self) -> u32 {
        self.capacity.load(Ordering::Relaxed)
    }

    /// Get currently allocated bytes
    ///
    /// **Performance**: <5ns (atomic load)
    #[inline]
    pub fn allocated(&self) -> u32 {
        self.allocated.load(Ordering::Acquire)
    }

    /// Get current generation counter
    ///
    /// **Performance**: <5ns (atomic load)
    ///
    /// Used for TOCTOU prevention and Q34 audit trail
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get remaining capacity in bytes
    ///
    /// **Performance**: <10ns (two atomic loads + subtraction)
    #[inline]
    pub fn available(&self) -> u32 {
        let allocated = self.allocated.load(Ordering::Relaxed);
        let capacity = self.capacity.load(Ordering::Relaxed);
        capacity.saturating_sub(allocated)
    }

    /// Reset allocation (TEST ONLY - not safe for concurrent use)
    ///
    /// **Performance**: <10ns (atomic stores)
    ///
    /// # Safety
    ///
    /// Only safe to call when no concurrent allocations are happening.
    /// Intended for test cleanup only.
    ///
    /// #ASSUME_NO_CONCURRENT_ACCESS: Caller ensures single-threaded access
    #[cfg(test)]
    pub fn reset(&self) {
        self.allocated.store(0, Ordering::Release);
        self.generation.store(0, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_creation() {
        let region = MmapRegion::new(0, 1024);

        assert_eq!(region.capacity(), 1024);
        assert_eq!(region.allocated(), 0);
        assert_eq!(region.generation(), 0);
        assert_eq!(region.available(), 1024);
    }

    #[test]
    fn test_region_allocation() {
        let region = MmapRegion::new(1000, 1024);

        // Allocate 256 bytes
        let offset1 = region.allocate(256).unwrap();
        assert_eq!(offset1, 1000); // Base offset
        assert_eq!(region.allocated(), 256);
        assert_eq!(region.generation(), 1);
        assert_eq!(region.available(), 768);

        // Allocate another 256 bytes
        let offset2 = region.allocate(256).unwrap();
        assert_eq!(offset2, 1256); // 1000 + 256
        assert_eq!(region.allocated(), 512);
        assert_eq!(region.generation(), 2);
        assert_eq!(region.available(), 512);
    }

    #[test]
    fn test_region_exhaustion() {
        let region = MmapRegion::new(0, 1024);

        // Fill region
        region.allocate(1024).unwrap();
        assert_eq!(region.available(), 0);

        // Next allocation should fail
        assert!(region.allocate(1).is_err());
    }

    #[test]
    fn test_region_overflow_protection() {
        let region = MmapRegion::new(0, 1024);

        // Attempt to allocate more than region capacity
        assert!(region.allocate(2048).is_err());
    }

    #[test]
    fn test_region_concurrent_allocation() {
        use std::sync::Arc;
        use std::thread;

        let region = Arc::new(MmapRegion::new(0, 10000));
        let mut handles = vec![];

        // Spawn 10 threads, each allocating 100 bytes 10 times
        for _ in 0..10 {
            let region = Arc::clone(&region);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    region.allocate(100).unwrap();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Total allocated should be 10 * 10 * 100 = 10000
        assert_eq!(region.allocated(), 10000);
        assert_eq!(region.generation(), 100);
        assert_eq!(region.available(), 0);
    }

    #[test]
    fn test_region_reset() {
        let region = MmapRegion::new(0, 1024);

        // Allocate some bytes
        region.allocate(256).unwrap();
        assert_eq!(region.allocated(), 256);
        assert_eq!(region.generation(), 1);

        // Reset
        region.reset();
        assert_eq!(region.allocated(), 0);
        assert_eq!(region.generation(), 0);
        assert_eq!(region.available(), 1024);
    }

    #[test]
    fn test_size_alignment() {
        // Verify struct is exactly 64 bytes
        assert_eq!(std::mem::size_of::<MmapRegion>(), 64);
        assert_eq!(std::mem::align_of::<MmapRegion>(), 64);
    }
}
