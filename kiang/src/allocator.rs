//! Lockfree Bump Allocator
//!
//! **Design Philosophy**: Single writer, lockfree readers via MemoryCapsule
//! **Avoids AMD mistake**: No concurrent resource allocation
//!
//! # Architecture
//!
//! - **SINGLE WRITER**: Only one allocation thread (enforced by &mut self)
//! - **LOCKFREE READS**: MemoryCapsule provides lockfree availability checks
//! - **Why safe**: Readers check availability lockfree, writer performs allocation serially
//!
//! # Performance Targets
//!
//! - `can_allocate()`: <5ns (lockfree hot path via MemoryCapsule)
//! - `allocate()`: <1μs (single writer, no contention)
//! - Alignment: 4K pages (GPU requirement)

use crate::memory::{MemoryCapsule, MemoryState};
use std::sync::atomic::{AtomicU64, Ordering};

/// Lockfree bump allocator for VRAM
///
/// #ASSUME_SINGLE_WRITER: Only one allocation thread
/// #VERIFY_SINGLE_WRITER: API requires &mut self for allocations
///
/// The type system enforces single writer at compile-time (Q31 Rust Transform).
/// Readers use lockfree `can_allocate()` for hot path decisions.
#[repr(C, align(64))]
pub struct BumpAllocator {
    /// Memory tracking capsule (lockfree reads)
    memory: MemoryCapsule,

    /// Current allocation offset (only writer modifies)
    offset: AtomicU64,

    /// Total capacity in bytes
    capacity: u64,

    /// Allocation generation counter (prevents ABA)
    generation: AtomicU64,
}

impl BumpAllocator {
    /// Create new bump allocator
    ///
    /// # Arguments
    ///
    /// * `capacity_mb` - Total VRAM capacity in megabytes
    pub fn new(capacity_mb: u16) -> Self {
        let capacity = (capacity_mb as u64) * 1024 * 1024;

        let allocator = Self {
            memory: MemoryCapsule::new(capacity_mb),
            offset: AtomicU64::new(0),
            capacity,
            generation: AtomicU64::new(1),
        };

        // Publish initial memory state
        let state = MemoryState {
            total_vram_mb: capacity_mb,
            used_vram_mb: 0,
            free_vram_mb: capacity_mb,
            allocation_count: 0,
            fragment_count: 0,
            largest_free_mb: capacity_mb,
            allocation_gen: 0,
            pressure_pct: 0,
        };
        allocator.memory.publish(state);

        allocator
    }

    /// Check if allocation possible (lockfree hot path)
    ///
    /// Uses MemoryCapsule for <5ns decision.
    /// This is the critical path for allocation decisions.
    ///
    /// # Arguments
    ///
    /// * `size` - Requested allocation size in bytes
    ///
    /// # Returns
    ///
    /// `true` if allocation is possible, `false` if OOM
    #[inline(always)]
    pub fn can_allocate(&self, size: u64) -> bool {
        let size_mb = size.div_ceil((1024 * 1024)) as u16;
        self.memory.can_allocate(size_mb)
    }

    /// Allocate aligned memory (single writer only!)
    ///
    /// #ASSUME_SINGLE_WRITER: &mut self ensures exclusive access
    /// #VERIFY_NO_CONCURRENT: Type system enforces
    ///
    /// # Arguments
    ///
    /// * `size` - Allocation size in bytes
    /// * `align` - Alignment requirement (must be power of 2)
    ///
    /// # Returns
    ///
    /// `Some(Allocation)` on success, `None` on OOM
    ///
    /// # Performance
    ///
    /// Target: <1μs (no locks, sequential bump)
    pub fn allocate(&mut self, size: u64, align: u64) -> Option<Allocation> {
        // Validate alignment is power of 2
        debug_assert!(align.is_power_of_two(), "Alignment must be power of 2");

        // Get current offset and align it
        let current = self.offset.load(Ordering::Relaxed);
        let aligned_offset = align_up(current, align);

        // Check if allocation fits
        let new_offset = aligned_offset.checked_add(size)?;
        if new_offset > self.capacity {
            return None; // OOM
        }

        // Perform allocation (single writer, no CAS needed)
        self.offset.store(new_offset, Ordering::Release);

        // Increment generation counter
        let generation = self.generation.fetch_add(1, Ordering::Relaxed);

        // Update memory capsule state
        let used_mb = new_offset.div_ceil((1024 * 1024)) as u16;
        let total_mb = self.capacity.div_ceil((1024 * 1024)) as u16;
        let free_mb = total_mb.saturating_sub(used_mb);
        let state = MemoryState {
            total_vram_mb: total_mb,
            used_vram_mb: used_mb,
            free_vram_mb: free_mb,
            allocation_count: (generation + 1) as u32,
            fragment_count: 0,
            largest_free_mb: free_mb,
            allocation_gen: (generation & 0xFFFF) as u16,
            pressure_pct: if total_mb > 0 {
                ((used_mb as u32 * 100) / total_mb as u32) as u8
            } else {
                0
            },
        };
        self.memory.publish(state);

        Some(Allocation {
            offset: aligned_offset,
            size,
            generation,
        })
    }

    /// Free memory (single writer only!)
    ///
    /// Note: Bump allocator doesn't support individual frees.
    /// This is a no-op for compatibility. Use `reset()` to reclaim all memory.
    pub fn free(&mut self, _alloc: Allocation) {
        // Bump allocator doesn't support individual frees
        // Use reset() to reclaim all memory
    }

    /// Reset allocator (reclaim all memory)
    ///
    /// # Safety
    ///
    /// Caller must ensure no outstanding allocations are in use.
    pub fn reset(&mut self) {
        self.offset.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Relaxed);

        // Update memory capsule
        let total_mb = self.capacity.div_ceil((1024 * 1024)) as u16;
        let state = MemoryState {
            total_vram_mb: total_mb,
            used_vram_mb: 0,
            free_vram_mb: total_mb,
            allocation_count: 0,
            fragment_count: 0,
            largest_free_mb: total_mb,
            allocation_gen: 0,
            pressure_pct: 0,
        };
        self.memory.publish(state);
    }

    /// Get current allocation statistics
    pub fn stats(&self) -> AllocatorStats {
        let snapshot = self.memory.read();
        let (used, available) = if let Some(s) = snapshot {
            (
                s.state.used_vram_mb as u64 * 1024 * 1024,
                s.state.free_vram_mb as u64 * 1024 * 1024,
            )
        } else {
            (0, self.capacity)
        };

        AllocatorStats {
            capacity: self.capacity,
            used,
            available,
            utilization_pct: if self.capacity > 0 {
                ((used * 100) / self.capacity) as u8
            } else {
                0
            },
        }
    }
}

/// Memory allocation handle
///
/// Contains offset, size, and generation counter for ABA prevention.
#[derive(Debug, Clone, Copy)]
pub struct Allocation {
    /// Offset in VRAM (aligned)
    offset: u64,

    /// Size in bytes
    size: u64,

    /// Generation counter (for ABA prevention)
    generation: u64,
}

impl Allocation {
    /// Get allocation offset
    #[inline(always)]
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Get allocation size
    #[inline(always)]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get generation counter
    #[inline(always)]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Allocator statistics
#[derive(Debug, Clone, Copy)]
pub struct AllocatorStats {
    /// Total capacity in bytes
    pub capacity: u64,

    /// Used bytes
    pub used: u64,

    /// Available bytes
    pub available: u64,

    /// Utilization percentage (0-100)
    pub utilization_pct: u8,
}

/// Align value up to nearest multiple of alignment
#[inline(always)]
fn align_up(value: u64, align: u64) -> u64 {
    (value + align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocator_creation() {
        let allocator = BumpAllocator::new(256); // 256MB
        let stats = allocator.stats();

        assert_eq!(stats.capacity, 256 * 1024 * 1024);
        assert_eq!(stats.used, 0);
        assert_eq!(stats.available, 256 * 1024 * 1024);
        assert_eq!(stats.utilization_pct, 0);
    }

    #[test]
    fn test_basic_allocation() {
        let mut allocator = BumpAllocator::new(256); // 256MB

        // Check can allocate
        assert!(allocator.can_allocate(1024 * 1024)); // 1MB

        // Allocate 1MB with 4K alignment
        let alloc = allocator.allocate(1024 * 1024, 4096);
        assert!(alloc.is_some());

        let alloc = alloc.unwrap();
        assert_eq!(alloc.offset(), 0); // First allocation at offset 0
        assert_eq!(alloc.size(), 1024 * 1024);

        // Check stats
        let stats = allocator.stats();
        assert_eq!(stats.used, 1024 * 1024);
    }

    #[test]
    fn test_aligned_allocation() {
        let mut allocator = BumpAllocator::new(256); // 256MB

        // Allocate 100 bytes with 4K alignment
        let alloc1 = allocator.allocate(100, 4096).unwrap();
        assert_eq!(alloc1.offset(), 0);

        // Next allocation should be aligned to 4K
        let alloc2 = allocator.allocate(100, 4096).unwrap();
        assert_eq!(alloc2.offset(), 4096); // Aligned to next 4K boundary
    }

    #[test]
    fn test_sequential_allocations() {
        let mut allocator = BumpAllocator::new(256); // 256MB

        // Allocate multiple 1MB blocks
        for i in 0..10 {
            let alloc = allocator.allocate(1024 * 1024, 4096);
            assert!(alloc.is_some());

            let alloc = alloc.unwrap();
            assert_eq!(alloc.offset(), i * 1024 * 1024);
        }

        let stats = allocator.stats();
        assert_eq!(stats.used, 10 * 1024 * 1024);
        assert_eq!(stats.utilization_pct, 3); // ~3.9%
    }

    #[test]
    fn test_oom_handling() {
        let mut allocator = BumpAllocator::new(1); // 1MB

        // Try to allocate 2MB (should fail)
        assert!(!allocator.can_allocate(2 * 1024 * 1024));

        let alloc = allocator.allocate(2 * 1024 * 1024, 4096);
        assert!(alloc.is_none());
    }

    #[test]
    fn test_exact_capacity() {
        let mut allocator = BumpAllocator::new(1); // 1MB

        // Allocate exactly 1MB
        let alloc = allocator.allocate(1024 * 1024, 1);
        assert!(alloc.is_some());

        // Next allocation should fail
        let alloc2 = allocator.allocate(1, 1);
        assert!(alloc2.is_none());

        let stats = allocator.stats();
        assert_eq!(stats.utilization_pct, 100);
    }

    #[test]
    fn test_reset() {
        let mut allocator = BumpAllocator::new(256); // 256MB

        // Allocate some memory
        allocator.allocate(10 * 1024 * 1024, 4096).unwrap();
        assert_eq!(allocator.stats().used, 10 * 1024 * 1024);

        // Reset allocator
        allocator.reset();

        // Should be able to allocate again
        let stats = allocator.stats();
        assert_eq!(stats.used, 0);
        assert_eq!(stats.available, 256 * 1024 * 1024);
    }

    #[test]
    fn test_alignment_power_of_two() {
        let mut allocator = BumpAllocator::new(256); // 256MB

        // Test various power-of-2 alignments
        let alignments = vec![1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

        for align in alignments {
            allocator.reset();
            let alloc = allocator.allocate(1024, align).unwrap();
            assert_eq!(alloc.offset() % align, 0);
        }
    }

    #[test]
    fn test_fragmentation_pattern() {
        let mut allocator = BumpAllocator::new(256); // 256MB

        // Allocate with varying sizes and alignments
        let alloc1 = allocator.allocate(1000, 4096).unwrap();
        assert_eq!(alloc1.offset(), 0);

        let alloc2 = allocator.allocate(2000, 4096).unwrap();
        assert_eq!(alloc2.offset(), 4096); // Next 4K boundary

        let alloc3 = allocator.allocate(500, 8192).unwrap();
        assert_eq!(alloc3.offset(), 8192); // Next 8K boundary
    }

    #[test]
    fn test_generation_counter() {
        let mut allocator = BumpAllocator::new(256); // 256MB

        let alloc1 = allocator.allocate(1024, 1).unwrap();
        let gen1 = alloc1.generation();

        let alloc2 = allocator.allocate(1024, 1).unwrap();
        let gen2 = alloc2.generation();

        // Generation should increment
        assert_eq!(gen2, gen1 + 1);

        // Reset should increment generation
        allocator.reset();

        let alloc3 = allocator.allocate(1024, 1).unwrap();
        let gen3 = alloc3.generation();
        assert!(gen3 > gen2);
    }

    #[test]
    fn test_align_up_helper() {
        assert_eq!(align_up(0, 4096), 0);
        assert_eq!(align_up(1, 4096), 4096);
        assert_eq!(align_up(4095, 4096), 4096);
        assert_eq!(align_up(4096, 4096), 4096);
        assert_eq!(align_up(4097, 4096), 8192);

        assert_eq!(align_up(100, 64), 128);
        assert_eq!(align_up(64, 64), 64);
        assert_eq!(align_up(0, 1), 0);
    }

    #[test]
    fn test_lockfree_can_allocate() {
        let allocator = BumpAllocator::new(256); // 256MB

        // Test lockfree availability check
        assert!(allocator.can_allocate(1024 * 1024)); // 1MB
        assert!(allocator.can_allocate(256 * 1024 * 1024)); // 256MB (exact)
        assert!(!allocator.can_allocate(257 * 1024 * 1024)); // 257MB (over)
    }
}
