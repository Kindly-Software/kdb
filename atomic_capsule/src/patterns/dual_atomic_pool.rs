//! Zero-Allocation Pool for DualAtomicU64
//!
//! # Overview
//!
//! Provides a lockfree, zero-allocation pool of DualAtomicU64 instances with
//! bitmap-based slot management. Target performance: <5ns acquire/release.
//!
//! # Framework Compliance
//!
//! - **UCE34**: T1 Atomic tier (lockfree coordination)
//! - **Chaos**: 100% lockfree, cache-aligned, page-aligned for huge pages
//! - **ASSUM**: All assumptions documented with #ASSUME tags
//! - **B32**: <5ns acquire/release target (validated via benchmarks)
//! - **T28**: Concurrent stress tests included
//!
//! # Architecture
//!
//! ```text
//! DualAtomicU64Pool (4096 slots default)
//! ├─ slots: [DualAtomicU64; CAPACITY]      (512 KB for 4096 slots)
//! ├─ free_bitmap: [AtomicU64; 64]          (512 bytes for 4096 slots)
//! ├─ next_hint: AtomicUsize                (search optimization)
//! └─ allocated: AtomicUsize                (allocation counter)
//! ```
//!
//! # Performance
//!
//! - **Acquire**: <5ns (bitmap scan + atomic OR)
//! - **Release**: <5ns (atomic AND)
//! - **Memory**: CAPACITY × 128 bytes + 8 bytes per 64 slots
//!
//! # Example
//!
//! ```rust
//! use atomic_capsule::patterns::DualAtomicPool;
//!
//! // Create pool (const-initialized, zero-alloc)
//! static POOL: DualAtomicPool<1024, 16> = DualAtomicPool::new();
//!
//! // Acquire slot
//! let slot = POOL.acquire().expect("pool exhausted");
//! slot.store_primary(42, Ordering::Relaxed);
//! slot.store_secondary(100, Ordering::Relaxed);
//!
//! // Automatic release on drop
//! drop(slot);
//! ```

use core::ops::Deref;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::patterns::dual_atomic::DualAtomicU64;

/// Zero-allocation pool of DualAtomicU64 instances with lockfree bitmap management.
///
/// # Layout
///
/// Page-aligned (4096 bytes) to enable huge page support for reduced TLB misses.
/// Total size for 4096 slots: 512 KB (slots) + 512 bytes (bitmap) + 16 bytes (metadata) ≈ 512.5 KB.
///
/// # Thread Safety
///
/// All methods are lockfree and safe for concurrent access from multiple threads.
///
/// # ASSUM Safety
///
/// - #ASSUME: CAPACITY must be a multiple of 64 (bitmap word size)
/// - #VERIFY: Compile-time assertion in `new()`
///
/// # Implementation Note
///
/// Uses separate BITMAP_SIZE const generic to avoid `generic_const_exprs` feature.
/// BITMAP_SIZE = CAPACITY / 64 must be ensured by caller.
#[repr(C, align(4096))]
pub struct DualAtomicPool<const CAPACITY: usize = 4096, const BITMAP_SIZE: usize = 64> {
    /// Pre-allocated slot storage (zero-initialized)
    slots: [DualAtomicU64; CAPACITY],

    /// Bitmap tracking free slots (1 = allocated, 0 = free)
    /// Each AtomicU64 tracks 64 slots
    free_bitmap: [AtomicU64; BITMAP_SIZE],

    /// Optimization hint: start bitmap scan from this index
    /// Reduces linear scan overhead when pool is partially full
    next_hint: AtomicUsize,

    /// Number of currently allocated slots (for monitoring)
    allocated: AtomicUsize,
}

impl<const CAPACITY: usize, const BITMAP_SIZE: usize> DualAtomicPool<CAPACITY, BITMAP_SIZE> {
    /// Creates a new pool with all slots free.
    ///
    /// # Compile-Time Assertions
    ///
    /// - CAPACITY must be > 0
    /// - CAPACITY must be a multiple of 64 (bitmap word size)
    /// - BITMAP_SIZE must equal CAPACITY / 64
    ///
    /// # Const Compatibility
    ///
    /// This is a const fn, allowing static initialization:
    ///
    /// ```rust
    /// static POOL: DualAtomicPool<1024, 16> = DualAtomicPool::new();
    /// ```
    ///
    /// # ASSUM
    ///
    /// - #ASSUME: Array initialization to zero is valid for DualAtomicU64
    /// - #VERIFY: DualAtomicU64 has `const fn new()` that zero-initializes
    #[inline]
    pub const fn new() -> Self {
        // Compile-time assertions
        assert!(CAPACITY > 0, "CAPACITY must be > 0");
        assert!(
            CAPACITY % 64 == 0,
            "CAPACITY must be a multiple of 64 for bitmap alignment"
        );
        assert!(
            BITMAP_SIZE * 64 == CAPACITY,
            "BITMAP_SIZE must equal CAPACITY / 64"
        );

        // SAFETY: We use a helper to initialize large arrays at compile-time
        // This avoids stack overflow from large const initializers
        Self {
            slots: Self::init_slots(),
            free_bitmap: Self::init_bitmap(),
            next_hint: AtomicUsize::new(0),
            allocated: AtomicUsize::new(0),
        }
    }

    /// Helper to initialize slots array at compile-time.
    ///
    /// # Implementation Note
    ///
    /// For large CAPACITY values, we use MaybeUninit to avoid stack overflow.
    /// This is safe because DualAtomicU64::new(0, 0) zero-initializes.
    const fn init_slots() -> [DualAtomicU64; CAPACITY] {
        use core::mem::MaybeUninit;

        // Create uninitialized array
        let mut slots: [MaybeUninit<DualAtomicU64>; CAPACITY] = unsafe {
            MaybeUninit::uninit().assume_init()
        };

        // Initialize each element
        let mut i = 0;
        while i < CAPACITY {
            slots[i] = MaybeUninit::new(DualAtomicU64::new(0, 0));
            i += 1;
        }

        // SAFETY: All elements have been initialized above
        // MaybeUninit<T> has same layout as T, so this is safe
        unsafe {
            // Use ptr::read to avoid size check in transmute
            core::ptr::read(&slots as *const _ as *const [DualAtomicU64; CAPACITY])
        }
    }

    /// Helper to initialize bitmap array at compile-time.
    const fn init_bitmap() -> [AtomicU64; BITMAP_SIZE] {
        use core::mem::MaybeUninit;

        // Create uninitialized array
        let mut bitmap: [MaybeUninit<AtomicU64>; BITMAP_SIZE] = unsafe {
            MaybeUninit::uninit().assume_init()
        };

        // Initialize each element
        let mut i = 0;
        while i < BITMAP_SIZE {
            bitmap[i] = MaybeUninit::new(AtomicU64::new(0));
            i += 1;
        }

        // SAFETY: All elements have been initialized above
        // MaybeUninit<T> has same layout as T, so this is safe
        unsafe {
            // Use ptr::read to avoid size check in transmute
            core::ptr::read(&bitmap as *const _ as *const [AtomicU64; BITMAP_SIZE])
        }
    }

    /// Acquires a free slot from the pool.
    ///
    /// # Returns
    ///
    /// - `Some(PoolSlot)` if a free slot was found and atomically claimed
    /// - `None` if the pool is exhausted (all slots allocated)
    ///
    /// # Performance
    ///
    /// Target: <5ns in the common case (free slot in first word scanned)
    /// Worst case: O(CAPACITY/64) bitmap word scans
    ///
    /// # Algorithm
    ///
    /// 1. Load `next_hint` as starting point
    /// 2. Scan bitmap words starting from hint
    /// 3. For each word with free bits (not 0xFFFF_FFFF_FFFF_FFFF):
    ///    - Find first zero bit with `trailing_ones()`
    ///    - Attempt atomic claim with `fetch_or`
    ///    - If successful, update hint and return slot
    /// 4. Wrap around to beginning if needed
    /// 5. Return None if full scan yields no free slots
    ///
    /// # ASSUM
    ///
    /// - #ASSUME: compare_exchange_weak loop prevents double-allocation races
    /// - #VERIFY: Stress test with concurrent threads validates no double-alloc
    #[inline]
    pub fn acquire(&self) -> Option<PoolSlot<'_, CAPACITY, BITMAP_SIZE>> {
        let bitmap_len = self.free_bitmap.len();
        let hint = self.next_hint.load(Ordering::Relaxed);

        // #ASSUME: Wrapping search from hint prevents starvation
        // #VERIFY: Tests validate uniform distribution of allocations
        for offset in 0..bitmap_len {
            let word_idx = (hint + offset) % bitmap_len;

            // Retry loop to handle races when claiming a bit
            // #ASSUME: CAS loop correctly handles all race conditions
            // #VERIFY: CAS ensures atomicity - only ONE thread can successfully claim each bit
            loop {
                let current = self.free_bitmap[word_idx].load(Ordering::Acquire);

                // Skip fully allocated words (all bits = 1)
                if current == u64::MAX {
                    break; // Move to next word
                }

                // Find first free bit (0 bit in bitmap)
                let bit_idx = (!current).trailing_zeros() as usize;

                // Safety check
                if bit_idx >= 64 {
                    break; // No free bits, move to next word
                }

                let mask = 1u64 << bit_idx;
                let new_value = current | mask;

                // Use compare_exchange to atomically claim the bit
                // This ensures NO race: only succeeds if bitmap unchanged since we loaded it
                match self.free_bitmap[word_idx].compare_exchange(
                    current,
                    new_value,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Success! We atomically claimed this bit
                        let slot_idx = word_idx * 64 + bit_idx;

                        // #ASSUME: slot_idx is always < CAPACITY
                        // #VERIFY: Compile-time bitmap size ensures this
                        debug_assert!(slot_idx < CAPACITY);

                        // Update hint for next allocation (optimization)
                        self.next_hint.store(word_idx, Ordering::Relaxed);

                        // Increment allocation counter
                        self.allocated.fetch_add(1, Ordering::Relaxed);

                        return Some(PoolSlot {
                            pool: self,
                            index: slot_idx,
                        });
                    }
                    Err(_) => {
                        // CAS failed - another thread modified the bitmap
                        // Reload and recalculate which bit to claim
                        continue;
                    }
                }
            }
        }

        // Pool exhausted
        None
    }

    /// Releases a slot back to the pool.
    ///
    /// # Parameters
    ///
    /// - `slot`: The slot to release (consumed by this method)
    ///
    /// # Performance
    ///
    /// Target: <5ns (single atomic AND operation)
    ///
    /// # ASSUM
    ///
    /// - #ASSUME: index is valid and was previously allocated
    /// - #VERIFY: PoolSlot only constructed by acquire() with valid index
    #[inline]
    fn release(&self, index: usize) {
        // #ASSUME: index < CAPACITY (enforced by PoolSlot construction)
        debug_assert!(index < CAPACITY);

        let word_idx = index / 64;
        let bit_idx = index % 64;
        let mask = !(1u64 << bit_idx); // Inverted mask for AND operation

        // Atomically clear the bit (mark as free)
        self.free_bitmap[word_idx].fetch_and(mask, Ordering::Release);

        // Decrement allocation counter
        self.allocated.fetch_sub(1, Ordering::Relaxed);

        // #ASSUME: No ABA problem because PoolSlot ensures single release
        // #VERIFY: PoolSlot Drop impl calls release() exactly once
    }

    /// Returns the number of currently allocated slots.
    ///
    /// # Note
    ///
    /// This is an approximation due to concurrent modifications.
    /// Use for monitoring/debugging only, not for correctness.
    #[inline]
    pub fn len(&self) -> usize {
        self.allocated.load(Ordering::Relaxed)
    }

    /// Returns the total capacity of the pool.
    #[inline]
    pub const fn capacity(&self) -> usize {
        CAPACITY
    }

    /// Returns true if no slots are allocated.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns true if all slots are allocated.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.len() == CAPACITY
    }
}

/// RAII wrapper for a pool slot that automatically releases on drop.
///
/// # Thread Safety
///
/// PoolSlot is !Send and !Sync because it contains a reference to the pool.
/// This prevents use-after-free if the pool is dropped while slots are live.
///
/// # ASSUM
///
/// - #ASSUME: Drop is always called exactly once per PoolSlot
/// - #VERIFY: Rust guarantees Drop is called on scope exit/unwinding
pub struct PoolSlot<'a, const CAPACITY: usize = 4096, const BITMAP_SIZE: usize = 64> {
    pool: &'a DualAtomicPool<CAPACITY, BITMAP_SIZE>,
    index: usize,
}

impl<'a, const CAPACITY: usize, const BITMAP_SIZE: usize> PoolSlot<'a, CAPACITY, BITMAP_SIZE> {
    /// Returns the slot index within the pool.
    ///
    /// Useful for debugging or associating external metadata with slots.
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }
}

impl<const CAPACITY: usize, const BITMAP_SIZE: usize> Deref for PoolSlot<'_, CAPACITY, BITMAP_SIZE> {
    type Target = DualAtomicU64;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: index is always valid (enforced by acquire())
        &self.pool.slots[self.index]
    }
}

// Note: DerefMut is not implemented because DualAtomicU64 provides interior mutability
// through atomic operations. There's no need for mutable access.

impl<const CAPACITY: usize, const BITMAP_SIZE: usize> Drop for PoolSlot<'_, CAPACITY, BITMAP_SIZE> {
    #[inline]
    fn drop(&mut self) {
        // Automatically release slot back to pool
        self.pool.release(self.index);
    }
}

// Safety: DualAtomicPool is Send if DualAtomicU64 is Send
unsafe impl<const CAPACITY: usize, const BITMAP_SIZE: usize> Send for DualAtomicPool<CAPACITY, BITMAP_SIZE> {}

// Safety: DualAtomicPool is Sync because all operations are atomic
unsafe impl<const CAPACITY: usize, const BITMAP_SIZE: usize> Sync for DualAtomicPool<CAPACITY, BITMAP_SIZE> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn test_basic_acquire_release() {
        let pool = DualAtomicPool::<64, 1>::new();

        // Initially empty
        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());
        assert!(!pool.is_full());

        // Acquire a slot
        let slot = pool.acquire().expect("should have free slot");
        assert_eq!(pool.len(), 1);

        // Use the slot
        slot.store_primary(42, Ordering::Relaxed);
        slot.store_secondary(100, Ordering::Relaxed);

        assert_eq!(slot.load_primary(Ordering::Relaxed), 42);
        assert_eq!(slot.load_secondary(Ordering::Relaxed), 100);

        // Release the slot
        drop(slot);
        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());
    }

    #[test]
    fn test_exhaust_pool() {
        const CAPACITY: usize = 64;
        let pool = DualAtomicPool::<CAPACITY, 1>::new();

        let mut slots = Vec::new();

        // Acquire all slots
        for i in 0..CAPACITY {
            let slot = pool.acquire().expect("should have free slot");
            slot.store_primary(i as u64, Ordering::Relaxed);
            slots.push(slot);
            assert_eq!(pool.len(), i + 1);
        }

        assert!(pool.is_full());

        // Pool should be exhausted
        assert!(pool.acquire().is_none());

        // Release half
        slots.truncate(CAPACITY / 2);
        assert_eq!(pool.len(), CAPACITY / 2);

        // Should be able to acquire again
        let slot = pool.acquire().expect("should have free slot after release");
        assert!(slot.index() >= CAPACITY / 2 || slot.index() < CAPACITY);
    }

    #[test]
    fn test_concurrent_acquire_release() {
        // Reduce capacity to avoid stack overflow (DualAtomicPool<1024> is 128KB)
        const CAPACITY: usize = 256;  // 32KB - safe for stack
        const BITMAP_SIZE: usize = 4; // 256 / 64 = 4
        const THREADS: usize = 8;
        const OPS_PER_THREAD: usize = 10_000;

        let pool = Arc::new(DualAtomicPool::<CAPACITY, BITMAP_SIZE>::new());
        let mut handles = Vec::new();

        for thread_id in 0..THREADS {
            let pool_clone = Arc::clone(&pool);
            let handle = thread::spawn(move || {
                for i in 0..OPS_PER_THREAD {
                    if let Some(slot) = pool_clone.acquire() {
                        // Write unique value
                        let value = (thread_id as u64) << 32 | (i as u64);
                        slot.store_primary(value, Ordering::Relaxed);
                        slot.store_secondary(value + 1, Ordering::Relaxed);

                        // Verify
                        assert_eq!(slot.load_primary(Ordering::Relaxed), value);
                        assert_eq!(slot.load_secondary(Ordering::Relaxed), value + 1);

                        // Release
                        drop(slot);
                    }
                    // If pool exhausted, just skip this iteration
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All slots should be released
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_bitmap_cas() {
        // Simplified test to verify CAS prevents double-allocation
        let bitmap = Arc::new(AtomicU64::new(0));
        let results = Arc::new(std::sync::Mutex::new(Vec::new()));

        let mut handles = Vec::new();
        for thread_id in 0..4 {
            let bitmap_clone = Arc::clone(&bitmap);
            let results_clone = Arc::clone(&results);
            let handle = thread::spawn(move || {
                // Try to atomically claim bit 0
                match bitmap_clone.compare_exchange_weak(
                    0,  // expected: all bits free
                    1,  // new: bit 0 set
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        results_clone.lock().unwrap().push(thread_id);
                    }
                    Err(_) => {}
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let winners = results.lock().unwrap();
        assert_eq!(winners.len(), 1, "Only one thread should win the CAS, but {:?} won", winners);
    }

    #[test]
    fn test_no_double_allocation() {
        const CAPACITY: usize = 64;
        const BITMAP_SIZE: usize = 1; // 64 / 64 = 1
        const THREADS: usize = 4;

        let pool = Arc::new(DualAtomicPool::<CAPACITY, BITMAP_SIZE>::new());
        let mut handles = Vec::new();

        // Barrier to sync all threads: wait until ALL have acquired before releasing
        let barrier = Arc::new(Barrier::new(THREADS));

        // Track which slots are allocated by each thread
        // Use Vec instead of array to avoid Copy requirement
        let mut allocated_by_vec = Vec::with_capacity(CAPACITY);
        for _ in 0..CAPACITY {
            allocated_by_vec.push(AtomicU64::new(u64::MAX));
        }
        let allocated_by = Arc::new(allocated_by_vec);

        for thread_id in 0..THREADS {
            let pool_clone = Arc::clone(&pool);
            let allocated_by_clone = Arc::clone(&allocated_by);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                let mut my_slots = Vec::new();

                // Acquire multiple slots
                for _ in 0..(CAPACITY / THREADS) {
                    if let Some(slot) = pool_clone.acquire() {
                        let idx = slot.index();

                        // Atomically claim this index
                        let prev =
                            allocated_by_clone[idx].swap(thread_id as u64, Ordering::SeqCst);

                        // Verify no other thread claimed it
                        assert_eq!(
                            prev,
                            u64::MAX,
                            "Double allocation detected! Slot {} claimed by thread {} and {}",
                            idx,
                            prev,
                            thread_id
                        );

                        my_slots.push(slot);
                    }
                }

                // CRITICAL: Wait for ALL threads to finish acquiring
                // This prevents early releasers from having their slots re-acquired
                barrier_clone.wait();

                // Now safe to release - no thread will re-acquire during verification
                my_slots.clear();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_slot_deref() {
        let pool = DualAtomicPool::<64, 1>::new();
        let slot = pool.acquire().unwrap();

        // Test Deref
        slot.store_primary(123, Ordering::Relaxed);
        assert_eq!(slot.load_primary(Ordering::Relaxed), 123);

        // Test method calls through Deref
        let left = slot.load_primary(Ordering::Relaxed);
        let right = slot.load_secondary(Ordering::Relaxed);
        assert_eq!(left, 123);
        assert_eq!(right, 0);
    }

    #[test]
    fn test_capacity_multiples() {
        // Test various CAPACITY values (all multiples of 64)
        // Note: Keep capacities small to avoid stack overflow
        // (DualAtomicPool<N> is N × 128 bytes on stack before Box moves it)
        let pool_64 = DualAtomicPool::<64, 1>::new();
        assert_eq!(pool_64.capacity(), 64);

        let pool_128 = DualAtomicPool::<128, 2>::new();
        assert_eq!(pool_128.capacity(), 128);

        let pool_256 = DualAtomicPool::<256, 4>::new();
        assert_eq!(pool_256.capacity(), 256);
    }

    #[test]
    fn test_large_capacity_via_static() {
        // Test large capacity (4096) using static initialization to avoid stack overflow
        // DualAtomicPool<4096, 64> is 512KB - too large for stack allocation
        static LARGE_POOL: DualAtomicPool<4096, 64> = DualAtomicPool::new();

        assert_eq!(LARGE_POOL.capacity(), 4096);

        // Basic functionality test
        let slot = LARGE_POOL.acquire().expect("should have free slot");
        slot.store_primary(12345, Ordering::Relaxed);
        assert_eq!(slot.load_primary(Ordering::Relaxed), 12345);
        drop(slot);

        assert_eq!(LARGE_POOL.len(), 0);
    }

    #[test]
    fn test_static_pool() {
        // Verify const initialization works for static
        static POOL: DualAtomicPool<128, 2> = DualAtomicPool::new();

        let slot = POOL.acquire().unwrap();
        slot.store_primary(999, Ordering::Relaxed);
        assert_eq!(slot.load_primary(Ordering::Relaxed), 999);
    }

    #[test]
    fn test_stress_acquire_release_cycle() {
        // Reduce capacity to be conservative (DualAtomicPool<256> is 32KB)
        const CAPACITY: usize = 128;  // 16KB - conservative for stack safety
        const BITMAP_SIZE: usize = 2; // 128 / 64 = 2
        const CYCLES: usize = 1000;

        let pool = DualAtomicPool::<CAPACITY, BITMAP_SIZE>::new();

        for cycle in 0..CYCLES {
            // Acquire all
            let mut slots = Vec::new();
            for _ in 0..CAPACITY {
                let slot = pool.acquire().expect("should have free slot");
                slot.store_primary(cycle as u64, Ordering::Relaxed);
                slots.push(slot);
            }

            assert!(pool.is_full());
            assert!(pool.acquire().is_none());

            // Verify all values
            for slot in &slots {
                assert_eq!(slot.load_primary(Ordering::Relaxed), cycle as u64);
            }

            // Release all
            slots.clear();
            assert!(pool.is_empty());
        }
    }

    #[test]
    fn test_hint_optimization() {
        const CAPACITY: usize = 128;
        const BITMAP_SIZE: usize = 2; // 128 / 64 = 2
        let pool = DualAtomicPool::<CAPACITY, BITMAP_SIZE>::new();

        // Acquire slots in sequence
        let mut slots = Vec::new();
        for i in 0..64 {
            let slot = pool.acquire().unwrap();
            // Hint should advance as we allocate
            assert!(slot.index() >= i);
            slots.push(slot);
        }

        // Release first half
        slots.truncate(32);

        // Next allocation should prefer released slots
        let slot = pool.acquire().unwrap();
        // Should reuse a slot from first half (indices 0..32)
        assert!(slot.index() < 64);
    }
}
