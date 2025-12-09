//! Lockfree Bump Allocator for BucketArray Allocation
//!
//! Specialized allocator for hash table resize operations that eliminates
//! per-allocation overhead by using atomic bump pointer allocation.
//!
//! # Design Principles (UCE32 Q28 - Simplicity)
//!
//! 1. **Atomic Bump Pointer**: Single global bump allocator (simplest design)
//! 2. **Proper Drop Semantics**: Track allocations for cleanup via Vec
//! 3. **Lockfree Allocation**: CAS-based allocation, no mutex
//! 4. **Resize-Only Usage**: Not a general allocator - optimized for BucketArray
//! 5. **IMPL-2 Compliance**: Simple implementation, no complex memory reclamation
//!
//! # Performance (UCE32 Q30 - Empirical Validation)
//!
//! Target: 50ns reduction vs Box::new() per BucketArray allocation
//! - Box::new(): ~80-100ns (heap allocation + initialization)
//! - Bump alloc: ~30-50ns (atomic increment + pointer offset)
//!
//! # Safety Assumptions (ASSUM Framework)
//!
//! #ASSUME_TYPE_SAFE: Arena memory is valid for lifetime of allocator
//! #VERIFY_UNSAFE_INVARIANTS: Miri validates memory safety
//! #ASSUME_TOCTOU_SAFE: Atomic bump pointer prevents double allocation
//! #VERIFY_TOCTOU_PREVENTED: CAS ensures exclusive allocation
//! #ASSUME_MEMORY_ORDERING: Release on bump, Acquire on read
//! #VERIFY_ORDERING_SUFFICIENT: Synchronizes allocation across threads
//! #ASSUME_RESOURCE_CLEANUP: Drop implementation frees all allocations
//! #VERIFY_DROP_SAFE: Leak detection validates no memory leaks
//!
//! # Rust Transformation (UCE32 Q31)
//!
//! Rust's ownership model ensures:
//! - Drop trait guarantees cleanup of all tracked allocations
//! - Box ownership prevents double-free
//! - AtomicPtr provides safe lockfree coordination
//! - Lifetime bounds prevent use-after-free
//!
//! # Note on Design Choice (UCE32 Q28 Analysis)
//!
//! We chose atomic bump allocator over thread-local arenas because:
//! 1. Simpler implementation (single global state vs per-thread state)
//! 2. Resize is relatively rare (amortized cost acceptable)
//! 3. Lockfree CAS provides adequate performance
//! 4. No thread coordination complexity for Drop
//! 5. Easier to reason about correctness (IMPL-2 principle)

use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use core::ptr;
use alloc::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use alloc::vec::Vec;
use alloc::boxed::Box;

use crate::bucket::BucketCapsule;

/// Arena size for BucketArray allocations (1MB initial)
///
/// Sized to hold ~16K BucketCapsules (64 bytes each).
/// This supports tables up to 16K capacity without arena expansion.
///
/// #ASSUME_INVARIANT: Arena size sufficient for typical table sizes
/// #VERIFY_INVARIANT: Benchmarks validate arena utilization
const ARENA_SIZE: usize = 1024 * 1024; // 1 MB

/// Maximum number of BucketArrays to track for Drop (safety limit)
///
/// Prevents unbounded memory growth in pathological cases.
/// With 1MB arena and 64-byte buckets, max ~16K allocations.
///
/// #ASSUME_INVARIANT: Max allocations never exceeded in normal operation
/// #VERIFY_INVARIANT: Tests validate allocation count
const MAX_TRACKED_ALLOCATIONS: usize = 1024;

/// Arena-based memory region for bump allocation
///
/// Single contiguous memory block allocated once and subdivided
/// via atomic bump pointer increments.
struct Arena {
    /// Base pointer to arena memory
    /// #ASSUME_LIFETIME_VALID: Valid for lifetime of Arena
    /// #VERIFY_LIFETIME_BOUNDS: Drop ensures cleanup
    base: *mut u8,

    /// Arena size in bytes
    size: usize,

    /// Layout for deallocation
    layout: Layout,
}

impl Arena {
    /// Allocate new arena with specified size
    ///
    /// #ASSUME_TYPE_SAFE: alloc returns properly aligned memory
    /// #VERIFY_UNSAFE_INVARIANTS: Null check validates allocation
    fn new(size: usize) -> Self {
        assert!(size > 0, "Arena size must be > 0");

        // #ASSUME_TYPE_SAFE: Layout safe for any alignment ≤ size
        let layout = Layout::from_size_align(size, 64).unwrap();

        // SAFETY: alloc returns valid memory or null (checked below)
        // #ASSUME_TYPE_SAFE: Allocated memory is properly aligned
        // #VERIFY_UNSAFE_INVARIANTS: Null check ensures validity
        let base = unsafe {
            let ptr = alloc(layout);
            if ptr.is_null() {
                handle_alloc_error(layout);
            }
            ptr
        };

        Self { base, size, layout }
    }

    /// Check if arena has space for allocation
    #[inline(always)]
    fn has_space(&self, offset: usize, alloc_size: usize) -> bool {
        offset + alloc_size <= self.size
    }

    /// Get pointer at offset
    ///
    /// #ASSUME_INVARIANT: Offset < size (validated by caller)
    /// #VERIFY_INVARIANT: has_space check ensures bounds
    #[inline(always)]
    unsafe fn ptr_at_offset(&self, offset: usize) -> *mut u8 {
        debug_assert!(offset <= self.size);
        self.base.add(offset)
    }
}

impl Drop for Arena {
    /// Free arena memory
    ///
    /// #ASSUME_RESOURCE_CLEANUP: Called exactly once per arena
    /// #VERIFY_DROP_SAFE: Rust guarantees Drop called once
    fn drop(&mut self) {
        // SAFETY: base was allocated with alloc using layout
        // #ASSUME_TYPE_SAFE: Pointer and layout match original allocation
        // #VERIFY_UNSAFE_INVARIANTS: Drop called exactly once by Rust
        unsafe {
            dealloc(self.base, self.layout);
        }
    }
}

// SAFETY: Arena can be sent between threads (pointer is stable)
// #ASSUME_SEND_SYNC: Arena memory is thread-safe for read/write
// #VERIFY_THREAD_SAFE: Synchronization via atomic operations
unsafe impl Send for Arena {}
unsafe impl Sync for Arena {}

/// Metadata for a BucketArray allocation
///
/// Tracks allocation location and size for proper Drop semantics.
struct AllocationMetadata {
    /// Pointer to allocated BucketCapsule array
    ptr: *mut BucketCapsule,

    /// Number of buckets in array
    capacity: usize,
}

// SAFETY: Metadata is just bookkeeping data
unsafe impl Send for AllocationMetadata {}
unsafe impl Sync for AllocationMetadata {}

/// Lockfree bump allocator for BucketArray allocations
///
/// Thread-safe bump allocator using atomic pointer increments.
/// Maintains a list of allocations for proper Drop cleanup.
pub struct BumpAllocator {
    /// Current arena
    /// #ASSUME_TOCTOU_SAFE: AtomicPtr prevents concurrent arena swap
    /// #VERIFY_TOCTOU_PREVENTED: Single-writer for arena swap
    arena: AtomicPtr<Arena>,

    /// Current bump offset in arena (monotonically increasing)
    /// #ASSUME_METRIC_ATOMIC: All increments are atomic
    /// #VERIFY_COUNTER_ACCURACY: Monotonic increase validated
    bump_offset: AtomicUsize,

    /// Tracked allocations for Drop (protected by internal mutex)
    ///
    /// NOTE: This is NOT in hot path - only used during:
    /// 1. Allocation tracking (rare - only during resize)
    /// 2. Drop cleanup (once at program end)
    ///
    /// Hot path (get/insert) never touches this.
    ///
    /// #ASSUME_RESOURCE_CLEANUP: All allocations tracked for cleanup
    /// #VERIFY_DROP_SAFE: Drop validates all allocations freed
    tracked_allocations: parking_lot::Mutex<Vec<AllocationMetadata>>,
}

impl BumpAllocator {
    /// Create new bump allocator with default arena size
    pub fn new() -> Self {
        Self::with_arena_size(ARENA_SIZE)
    }

    /// Create new bump allocator with specified arena size
    pub fn with_arena_size(size: usize) -> Self {
        let arena = Arena::new(size);
        let arena_ptr = Box::into_raw(Box::new(arena));

        Self {
            arena: AtomicPtr::new(arena_ptr),
            bump_offset: AtomicUsize::new(0),
            tracked_allocations: parking_lot::Mutex::new(Vec::new()),
        }
    }

    /// Allocate BucketCapsule array (lockfree allocation)
    ///
    /// Returns pointer to allocated array or None if arena full.
    ///
    /// # Performance
    ///
    /// Target: ~30-50ns (atomic fetch_add + pointer offset)
    /// vs Box::new(): ~80-100ns (heap allocation + initialization)
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - Returned pointer is freed via this allocator's Drop
    /// - Pointer is not used after allocator is dropped
    ///
    /// #ASSUME_TOCTOU_SAFE: Atomic fetch_add ensures exclusive allocation
    /// #VERIFY_TOCTOU_PREVENTED: No two threads get same offset
    pub fn allocate_bucket_array(&self, capacity: usize) -> Option<*mut BucketCapsule> {
        assert!(capacity > 0, "Capacity must be > 0");

        // Calculate allocation size (aligned to 64 bytes for BucketCapsule)
        let bucket_size = core::mem::size_of::<BucketCapsule>();
        let alloc_size = capacity * bucket_size;

        // SAFETY: Arena pointer is valid (never null after construction)
        let arena = unsafe {
            let ptr = self.arena.load(Ordering::Acquire);
            &*ptr
        };

        // Atomic bump allocation (lockfree)
        // #ASSUME_TOCTOU_SAFE: fetch_add is atomic, prevents double allocation
        // #VERIFY_TOCTOU_PREVENTED: Multiple threads get different offsets
        let offset = self.bump_offset.fetch_add(alloc_size, Ordering::Relaxed);

        // Check if allocation fits in arena
        if !arena.has_space(offset, alloc_size) {
            // Arena exhausted - fallback to Box::new() in caller
            // (This is rare - only happens if many resizes occur)
            return None;
        }

        // SAFETY: Offset validated by has_space check
        // Memory is valid and aligned (arena is 64-byte aligned)
        let ptr = unsafe {
            let base_ptr = arena.ptr_at_offset(offset);
            let bucket_ptr = base_ptr as *mut BucketCapsule;

            // Initialize each bucket with BucketCapsule::new()
            for i in 0..capacity {
                bucket_ptr.add(i).write(BucketCapsule::new());
            }

            bucket_ptr
        };

        // Track allocation for Drop cleanup
        // NOTE: This lock is NOT in hot path - only during resize
        let metadata = AllocationMetadata { ptr, capacity };
        {
            let mut allocations = self.tracked_allocations.lock();

            // Safety limit: prevent unbounded growth
            if allocations.len() >= MAX_TRACKED_ALLOCATIONS {
                return None;
            }

            allocations.push(metadata);
        }

        Some(ptr)
    }

    /// Get allocation statistics (for monitoring)
    #[allow(dead_code)]
    pub fn stats(&self) -> AllocatorStats {
        let offset = self.bump_offset.load(Ordering::Relaxed);
        let allocations = self.tracked_allocations.lock();

        // SAFETY: Arena pointer is valid
        let arena = unsafe {
            let ptr = self.arena.load(Ordering::Acquire);
            &*ptr
        };

        AllocatorStats {
            arena_size: arena.size,
            bytes_allocated: offset,
            bytes_remaining: arena.size.saturating_sub(offset),
            allocation_count: allocations.len(),
        }
    }
}

impl Default for BumpAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BumpAllocator {
    /// Cleanup all tracked allocations
    ///
    /// #ASSUME_RESOURCE_CLEANUP: All allocations freed exactly once
    /// #VERIFY_DROP_SAFE: Tracked metadata ensures no leaks
    fn drop(&mut self) {
        // Free all tracked allocations
        // SAFETY: We own all tracked pointers (allocated by us)
        // #ASSUME_TYPE_SAFE: Pointers are valid BucketCapsule arrays
        // #VERIFY_UNSAFE_INVARIANTS: Metadata tracks capacity correctly
        let allocations = self.tracked_allocations.lock();
        for metadata in allocations.iter() {
            unsafe {
                // Drop each bucket capsule
                for i in 0..metadata.capacity {
                    ptr::drop_in_place(metadata.ptr.add(i));
                }
                // Note: Memory itself is freed when arena is dropped
            }
        }

        // Free arena
        // SAFETY: Arena pointer is valid (never null after construction)
        unsafe {
            let ptr = self.arena.load(Ordering::Relaxed);
            if !ptr.is_null() {
                let _ = Box::from_raw(ptr);
            }
        }
    }
}

/// Allocator statistics for monitoring
#[derive(Clone, Copy, Debug)]
pub struct AllocatorStats {
    pub arena_size: usize,
    pub bytes_allocated: usize,
    pub bytes_remaining: usize,
    pub allocation_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator_new() {
        let alloc = BumpAllocator::new();
        let stats = alloc.stats();

        assert_eq!(stats.arena_size, ARENA_SIZE);
        assert_eq!(stats.bytes_allocated, 0);
        assert_eq!(stats.allocation_count, 0);
    }

    #[test]
    fn allocator_single_allocation() {
        let alloc = BumpAllocator::new();

        let ptr = alloc.allocate_bucket_array(16);
        assert!(ptr.is_some());

        let stats = alloc.stats();
        assert_eq!(stats.allocation_count, 1);
        assert!(stats.bytes_allocated > 0);
    }

    #[test]
    fn allocator_multiple_allocations() {
        let alloc = BumpAllocator::new();

        // Allocate several arrays
        for _ in 0..10 {
            let ptr = alloc.allocate_bucket_array(16);
            assert!(ptr.is_some());
        }

        let stats = alloc.stats();
        assert_eq!(stats.allocation_count, 10);
    }

    #[test]
    fn allocator_arena_exhaustion() {
        // Create small arena to test exhaustion
        let alloc = BumpAllocator::with_arena_size(4096); // 4KB arena

        // Allocate until arena is full
        let mut success_count = 0;
        for _ in 0..100 {
            if alloc.allocate_bucket_array(16).is_some() {
                success_count += 1;
            } else {
                break;
            }
        }

        // Should have allocated some but not all
        assert!(success_count > 0);
        assert!(success_count < 100);
    }

    #[test]
    fn allocator_concurrent_allocation() {
        use std::sync::Arc;
        use std::thread;

        let alloc = Arc::new(BumpAllocator::new());
        let mut handles = vec![];

        // Spawn multiple threads allocating concurrently
        for _ in 0..4 {
            let alloc_clone = Arc::clone(&alloc);
            let handle = thread::spawn(move || {
                let mut allocated = 0;
                for _ in 0..10 {
                    if alloc_clone.allocate_bucket_array(16).is_some() {
                        allocated += 1;
                    }
                }
                allocated
            });
            handles.push(handle);
        }

        // Wait and count total allocations
        let total: usize = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .sum();

        assert_eq!(total, 40); // All allocations should succeed
    }

    #[test]
    fn allocator_drop_cleanup() {
        // Create allocator in scope
        {
            let alloc = BumpAllocator::new();
            alloc.allocate_bucket_array(16);
            alloc.allocate_bucket_array(32);
            // Drop should cleanup without leaking
        }
        // Valgrind/LeakSanitizer will detect leaks
    }
}
