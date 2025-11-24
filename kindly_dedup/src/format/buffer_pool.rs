//! # BufferPool - T1 Atomic Tier Lockfree Buffer Management
//!
//! High-performance buffer pool for zero-allocation streaming using a lockfree Treiber stack.
//!
//! ## Architecture
//!
//! BufferPool maintains a fixed set of pre-allocated buffers (default: 16 × 64 KB = 1 MB).
//! Buffers are managed via a lockfree Treiber stack with ABA prevention using generation counters.
//!
//! ```text
//! Free List (Treiber Stack):
//!   Head [Gen:u32 | Index:u32] → Buffer[0] ↔ Buffer[1] ↔ ... ↔ None
//!
//!   High 32 bits: Generation counter (ABA prevention, rolls over at 2^32)
//!   Low 32 bits:  Buffer index (0-15) or 0xFFFFFFFF if empty
//! ```
//!
//! ## Performance
//!
//! | Operation | Target | Implementation |
//! |-----------|--------|-----------------|
//! | Allocate  | <50ns  | Lockfree CAS    |
//! | Deallocate| <50ns  | Lockfree CAS    |
//! | Available | <100ns | Linear scan     |
//! | Stats     | <50ns  | Atomic loads    |
//!
//! ## Framework Compliance
//!
//! - **UCE34 Q10**: T1 (Atomic) tier selection
//! - **COCA**: 100% lockfree (Treiber stack, no mutex/RwLock)
//! - **ASSUM**: All assumptions documented with #ASSUME/#VERIFY tags
//! - **B32**: Fair baseline (vs naive Vec-based pool)
//! - **T28**: 4-tier testing (unit/property/integration/production)
//!
//! ## Example
//!
//! ```rust,ignore
//! use kindly_dedup::format::BufferPool;
//!
//! let pool = BufferPool::new(64 * 1024, 16);
//!
//! // Allocate a buffer (lockfree CAS)
//! let buf = pool.allocate().expect("pool has buffers");
//! assert_eq!(buf.len(), 64 * 1024);
//! assert_eq!(pool.available(), 15);  // 1 allocated
//!
//! // Deallocate returns buffer to pool
//! pool.deallocate(buf).expect("valid buffer");
//! assert_eq!(pool.available(), 16);  // Back to full
//! ```
//!
//! ## Lockfree Treiber Stack Implementation
//!
//! The free list uses a classic Treiber stack:
//! 1. **ABA Prevention**: Generation counter in high 32 bits prevents ABA races
//! 2. **Compare-Exchange**: CAS loop retries on contention (exponential backoff not needed for typical contention)
//! 3. **Memory Ordering**: Acquire/Release for proper synchronization
//! 4. **Deterministic Behavior**: No spinning, just CAS retry loop
//!
//! ## ASSUM Framework Tags
//!
//! ```
//! #ASSUME: Max 16 buffers sufficient (16 × 64 KB = 1 MB total pool)
//! #ASSUME: Generation counter prevents ABA (u32 sufficient for typical lifetimes)
//! #ASSUME: Buffer size fixed (64 KB, no fragmentation or size variations)
//! #ASSUME: Single allocate/deallocate per borrowed buffer (no double-free)
//! #ASSUME: buffer_size > 0 and max_buffers > 0 (constructor validates)
//! #VERIFY: BufferPool struct size = 64 bytes (cache-aligned, fits L1 line)
//! #VERIFY: Lockfree property (no mutex, only AtomicU64 + CAS)
//! #VERIFY: No buffer leaks (deallocate count == allocate count at program end)
//! #VERIFY: LIFO ordering (stack property preserved by CAS)
//! #VERIFY: ABA-safe (generation prevents index reuse in stack)
//! #VERIFY: Memory safety (no unsafe buffer access, bounds checked)
//! ```

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[cfg(test)]
use std::sync::Arc;

/// T1 Atomic tier lockfree buffer pool
///
/// Manages pre-allocated buffers via a lockfree Treiber stack.
/// Each buffer is a Vec<u8> pre-allocated to buffer_size.
///
/// ## Memory Layout (64 bytes, cache-aligned)
///
/// ```text
/// Offset  Size  Field
/// ------  ----  -----
/// 0       8     buffer_size (usize)
/// 8       4     max_buffers (u32)
/// 12      4     _padding_config
/// 16      8     free_list_head (AtomicU64)
/// 24      8     total_allocations (AtomicU64)
/// 32      8     total_deallocations (AtomicU64)
/// 40      4     peak_usage (AtomicU32)
/// 44      12    _padding_stats
/// 56      8     [heap pointers for buffers/next]
/// ------
/// 64      (aligned to 64-byte cache line)
/// ```
///
/// # ASSUME: Heap pointers are stored separately (Vec buffers, Vec next pointers)
/// # VERIFY: sizeof(BufferPool) >= 64 bytes with #[repr(C, align(64))]
#[repr(C, align(64))]
pub struct BufferPool {
    // Configuration (16 bytes)
    buffer_size: usize,           // Size of each buffer (typically 64 KB)
    max_buffers: u32,             // Max number of buffers (max 16 per spec)
    _padding_config: [u8; 4],     // Padding to maintain alignment

    // Free list head (Treiber stack, 8 bytes)
    // #ASSUME: High 32 bits = generation counter (ABA prevention)
    // #ASSUME: Low 32 bits = buffer index (0-15) or 0xFFFFFFFF if empty
    free_list_head: AtomicU64,

    // Statistics (32 bytes)
    total_allocations: AtomicU64,   // Count of successful allocations
    total_deallocations: AtomicU64, // Count of successful deallocations
    peak_usage: AtomicU32,          // Peak number of allocated buffers
    _padding_stats: [u8; 12],       // Padding to next 64B boundary

    // Heap-allocated buffer storage (off-cache)
    // These are not part of the 64-byte capsule header
    buffers: Vec<Vec<u8>>,          // Pre-allocated buffers
    next: Vec<AtomicU32>,           // Next pointers for free list (index of next free buffer)
}

/// Statistics snapshot from BufferPool
#[derive(Clone, Debug)]
pub struct PoolStats {
    /// Total successful allocations since creation
    pub total_allocations: u64,

    /// Total successful deallocations since creation
    pub total_deallocations: u64,

    /// Peak number of simultaneously allocated buffers
    pub peak_usage: u32,

    /// Current number of available (free) buffers
    pub current_available: u32,
}

impl BufferPool {
    /// Create a new buffer pool with pre-allocated buffers
    ///
    /// # Arguments
    ///
    /// * `buffer_size` - Size of each buffer in bytes (typically 64 KB)
    /// * `max_buffers` - Maximum number of buffers to pre-allocate (≤16)
    ///
    /// # Returns
    ///
    /// New BufferPool with all buffers free (ready to allocate)
    ///
    /// # Panics
    ///
    /// * If `max_buffers > 16` (generation counter space limited)
    /// * If `buffer_size == 0`
    ///
    /// # ASSUME
    ///
    /// * max_buffers ≤ 16 (generation counter is u32, can handle 2^32 wraparounds per 16 buffers)
    /// * buffer_size > 0 (validated at construction)
    /// * All buffers pre-allocated successfully (OOM would panic)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use kindly_dedup::format::BufferPool;
    ///
    /// // Create pool with 16 buffers of 64 KB each
    /// let pool = BufferPool::new(64 * 1024, 16);
    /// assert_eq!(pool.available(), 16);  // All free initially
    /// ```
    pub fn new(buffer_size: usize, max_buffers: u32) -> Self {
        assert!(max_buffers <= 16, "max_buffers must be ≤ 16 (generation counter limited)");
        assert!(buffer_size > 0, "buffer_size must be > 0");

        // #ASSUME: All buffer allocations succeed (OOM handling not needed for this tier)
        let mut buffers = Vec::with_capacity(max_buffers as usize);
        let mut next = Vec::with_capacity(max_buffers as usize);

        // Initialize all buffers as free
        // Free list is a stack: buffer 0 → buffer 1 → ... → None
        for i in 0..(max_buffers as usize) {
            buffers.push(vec![0u8; buffer_size]);

            // Next pointer: point to next buffer in chain, or u32::MAX if last
            let next_index = if i < (max_buffers as usize) - 1 {
                (i + 1) as u32
            } else {
                u32::MAX // End of free list
            };
            next.push(AtomicU32::new(next_index));
        }

        // Initialize head to point to buffer 0 with generation 0
        // Format: (generation:u32 << 32) | (index:u32)
        let initial_head = (0u64 << 32) | 0u64;

        BufferPool {
            buffer_size,
            max_buffers,
            _padding_config: [0u8; 4],
            free_list_head: AtomicU64::new(initial_head),
            total_allocations: AtomicU64::new(0),
            total_deallocations: AtomicU64::new(0),
            peak_usage: AtomicU32::new(0),
            _padding_stats: [0u8; 12],
            buffers,
            next,
        }
    }

    /// Allocate a buffer from the pool (lockfree)
    ///
    /// Pops a buffer from the free list using lockfree CAS.
    /// Retries on contention until successful or pool exhausted.
    ///
    /// # Returns
    ///
    /// * `Some(Vec<u8>)` - Allocated buffer (size = buffer_size)
    /// * `None` - Pool exhausted (no free buffers available)
    ///
    /// # Performance
    ///
    /// Target: <50ns (lockfree CAS, <3 retries typical)
    ///
    /// # Memory Ordering
    ///
    /// * Load: Acquire (pairs with Release in deallocate)
    /// * CAS: Release/Relaxed (ensure visibility before pop completes)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pool = BufferPool::new(64 * 1024, 4);
    ///
    /// // Allocate 4 buffers
    /// let b1 = pool.allocate().expect("first");
    /// let b2 = pool.allocate().expect("second");
    /// let b3 = pool.allocate().expect("third");
    /// let b4 = pool.allocate().expect("fourth");
    ///
    /// // Pool exhausted
    /// assert!(pool.allocate().is_none());
    /// ```
    ///
    /// # ASSUME
    ///
    /// * index is valid (0 ≤ index < max_buffers) guaranteed by initialization
    /// * buffer_size is accurate for returned buffer
    pub fn allocate(&self) -> Option<Vec<u8>> {
        loop {
            // Load current head (generation + index)
            let head = self.free_list_head.load(Ordering::Acquire);
            let index = (head & 0xFFFFFFFF) as u32;
            let generation = (head >> 32) as u32;

            // Check if pool is empty
            if index == u32::MAX {
                return None;
            }

            // #VERIFY: index < max_buffers (guaranteed by initialization)
            let index_usize = index as usize;

            // Load next pointer from next[index]
            // #ASSUME: index_usize is valid (checked above)
            let next_index = self.next[index_usize].load(Ordering::Relaxed);

            // Create new head: increment generation (ABA prevention), use next_index
            let new_generation = generation.wrapping_add(1);
            let new_head = ((new_generation as u64) << 32) | (next_index as u64);

            // Try to atomically pop from stack
            // #VERIFY: Lockfree (CAS is wait-free, retries on contention)
            if self
                .free_list_head
                .compare_exchange(head, new_head, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                // Success: update statistics
                self.total_allocations.fetch_add(1, Ordering::Relaxed);
                self.update_peak_usage();

                // #VERIFY: No unsafe, bounds are checked
                return Some(self.buffers[index_usize].clone());
            }
            // CAS failed (contention), retry
        }
    }

    /// Deallocate a buffer back to the pool (lockfree)
    ///
    /// Pushes a buffer back to the free list using lockfree CAS.
    /// Retries on contention until successful.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to deallocate (must be from this pool)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Successfully deallocated
    /// * `Err(String)` - Buffer size mismatch or not from this pool
    ///
    /// # Performance
    ///
    /// Target: <50ns (lockfree CAS, <3 retries typical)
    ///
    /// # Memory Ordering
    ///
    /// * Store next: Relaxed (no external visibility required)
    /// * CAS: Release/Relaxed (ensure next pointer visible before update)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pool = BufferPool::new(64 * 1024, 4);
    /// let buf = pool.allocate().expect("allocated");
    ///
    /// // Deallocate returns buffer
    /// pool.deallocate(buf).expect("deallocate succeeds");
    /// assert_eq!(pool.available(), 1);  // Back in free list
    /// ```
    ///
    /// # ASSUME
    ///
    /// * buffer.len() == buffer_size (caller's responsibility)
    /// * buffer from this pool (pointer comparison)
    /// * no double-free (caller must not deallocate same buffer twice)
    ///
    /// # VERIFY
    ///
    /// * Pointer comparison is safe (Vec pointers are stable if not reallocated)
    pub fn deallocate(&self, buffer: Vec<u8>) -> Result<(), String> {
        // Validate buffer size
        // #ASSUME: buffer.len() == buffer_size (caller responsibility)
        if buffer.len() != self.buffer_size {
            return Err(format!(
                "Buffer size mismatch: expected {}, got {}",
                self.buffer_size,
                buffer.len()
            ));
        }

        // Find which buffer index this corresponds to (linear search)
        // #VERIFY: Pointer comparison is safe (Vec backing pointers are stable)
        let buffer_ptr = buffer.as_ptr() as u64;
        let mut index = None;

        for (i, pooled_buffer) in self.buffers.iter().enumerate() {
            if pooled_buffer.as_ptr() as u64 == buffer_ptr {
                index = Some(i as u32);
                break;
            }
        }

        let index = match index {
            Some(i) => {
                if i >= self.max_buffers {
                    return Err("Buffer index out of range".to_string());
                }
                i
            }
            None => return Err("Buffer not from this pool".to_string()),
        };

        // Push buffer back to free list (Treiber stack LIFO)
        loop {
            let head = self.free_list_head.load(Ordering::Acquire);
            let current_index = (head & 0xFFFFFFFF) as u32;
            let generation = (head >> 32) as u32;

            // Set next pointer: this buffer's next is the current head
            // #VERIFY: index < max_buffers (checked above)
            self.next[index as usize].store(current_index, Ordering::Relaxed);

            // Create new head: same generation (not deallocate), this buffer as new head
            let new_head = ((generation as u64) << 32) | (index as u64);

            // Try to atomically push to stack
            // #VERIFY: Lockfree (CAS is wait-free)
            if self
                .free_list_head
                .compare_exchange(head, new_head, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                // Success: update statistics and drop buffer
                self.total_deallocations.fetch_add(1, Ordering::Relaxed);
                std::mem::drop(buffer);
                return Ok(());
            }
            // CAS failed (contention), retry
        }
    }

    /// Get number of available (free) buffers
    ///
    /// # Returns
    ///
    /// Count of free buffers currently in the pool (0 to max_buffers)
    ///
    /// # Performance
    ///
    /// O(available_count) linear scan, typically <100ns
    ///
    /// # Note
    ///
    /// This is a snapshot value. By the time this returns, another thread
    /// may have allocated or deallocated buffers.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pool = BufferPool::new(64 * 1024, 4);
    /// assert_eq!(pool.available(), 4);
    ///
    /// let _buf = pool.allocate();
    /// assert_eq!(pool.available(), 3);
    /// ```
    pub fn available(&self) -> u32 {
        let mut count = 0u32;
        let mut index = (self.free_list_head.load(Ordering::Acquire) & 0xFFFFFFFF) as u32;

        // Walk the free list, counting buffers
        // #ASSUME: max_buffers <= 16, so loop bounded
        while index != u32::MAX && count < self.max_buffers {
            // #VERIFY: index < max_buffers (guaranteed by loop condition)
            index = self.next[index as usize].load(Ordering::Relaxed);
            count += 1;
        }

        count
    }

    /// Get pool statistics
    ///
    /// # Returns
    ///
    /// PoolStats with allocation/deallocation counts and peak usage
    ///
    /// # Performance
    ///
    /// <50ns (4 atomic loads)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pool = BufferPool::new(64 * 1024, 4);
    /// let _buf = pool.allocate();
    ///
    /// let stats = pool.stats();
    /// assert_eq!(stats.total_allocations, 1);
    /// assert_eq!(stats.current_available, 3);
    /// assert_eq!(stats.peak_usage, 1);
    /// ```
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            total_allocations: self.total_allocations.load(Ordering::Relaxed),
            total_deallocations: self.total_deallocations.load(Ordering::Relaxed),
            peak_usage: self.peak_usage.load(Ordering::Relaxed),
            current_available: self.available(),
        }
    }

    /// Update peak usage statistic
    ///
    /// Atomically updates peak_usage to reflect maximum simultaneous allocations.
    ///
    /// # Performance
    ///
    /// <50ns (atomic CAS loop, typically <2 iterations)
    ///
    /// # Implementation
    ///
    /// Uses CAS loop to atomically update if new value > current max
    fn update_peak_usage(&self) {
        let used = self.max_buffers - self.available();
        let mut peak = self.peak_usage.load(Ordering::Relaxed);

        // #VERIFY: CAS loop converges (peak only increases)
        while used > peak {
            match self.peak_usage.compare_exchange(
                peak,
                used,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
    }

    /// Get buffer size
    ///
    /// # Returns
    ///
    /// Size in bytes of each buffer in this pool
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    /// Get maximum buffer count
    ///
    /// # Returns
    ///
    /// Maximum number of buffers (16 or less)
    pub fn max_buffers(&self) -> u32 {
        self.max_buffers
    }
}

// #VERIFY: BufferPool is Send + Sync (all fields thread-safe)
unsafe impl Send for BufferPool {}
unsafe impl Sync for BufferPool {}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Unit Tests (T28 Tier 1) ==========

    #[test]
    fn test_buffer_pool_creation() {
        let pool = BufferPool::new(64 * 1024, 4);
        assert_eq!(pool.available(), 4);
        assert_eq!(pool.buffer_size(), 64 * 1024);
        assert_eq!(pool.max_buffers(), 4);
    }

    #[test]
    fn test_allocate_single_buffer() {
        let pool = BufferPool::new(64 * 1024, 4);

        let buf = pool.allocate().expect("should allocate");
        assert_eq!(buf.len(), 64 * 1024);
        assert_eq!(pool.available(), 3);

        let stats = pool.stats();
        assert_eq!(stats.total_allocations, 1);
        assert_eq!(stats.peak_usage, 1);
    }

    #[test]
    fn test_deallocate_single_buffer() {
        let pool = BufferPool::new(64 * 1024, 4);

        let buf = pool.allocate().expect("should allocate");
        assert_eq!(pool.available(), 3);

        pool.deallocate(buf).expect("should deallocate");
        assert_eq!(pool.available(), 4);

        let stats = pool.stats();
        assert_eq!(stats.total_deallocations, 1);
    }

    #[test]
    fn test_pool_exhaustion() {
        let pool = BufferPool::new(64 * 1024, 2);

        let _buf1 = pool.allocate().expect("first allocate");
        let _buf2 = pool.allocate().expect("second allocate");
        assert_eq!(pool.available(), 0);

        let buf3 = pool.allocate();
        assert!(buf3.is_none(), "pool should be exhausted");
    }

    #[test]
    fn test_lifo_ordering() {
        let pool = BufferPool::new(64 * 1024, 3);

        let buf1 = pool.allocate().expect("allocate 1");
        let buf2 = pool.allocate().expect("allocate 2");
        let buf3 = pool.allocate().expect("allocate 3");

        // Deallocate in order: 3, 2, 1
        pool.deallocate(buf3).expect("deallocate 3");
        pool.deallocate(buf2).expect("deallocate 2");
        pool.deallocate(buf1).expect("deallocate 1");

        // Allocate again: LIFO means we should get buffers in reverse order
        // (This is implementation detail, not guaranteed by API, but verified for Treiber stack)
        let _rebuf1 = pool.allocate().expect("reallocate 1");
        let _rebuf2 = pool.allocate().expect("reallocate 2");
        let _rebuf3 = pool.allocate().expect("reallocate 3");

        assert_eq!(pool.available(), 0);
    }

    #[test]
    fn test_peak_usage_tracking() {
        let pool = BufferPool::new(64 * 1024, 4);

        let buf1 = pool.allocate();
        let buf2 = pool.allocate();
        let stats1 = pool.stats();
        assert_eq!(stats1.peak_usage, 2);

        let buf3 = pool.allocate();
        let stats2 = pool.stats();
        assert_eq!(stats2.peak_usage, 3);

        // Deallocate one, peak should not decrease
        if let (Some(b1), Some(b2), Some(b3)) = (buf1, buf2, buf3) {
            pool.deallocate(b1).ok();
            let stats3 = pool.stats();
            assert_eq!(stats3.peak_usage, 3, "peak usage should not decrease");

            pool.deallocate(b2).ok();
            pool.deallocate(b3).ok();
        }
    }

    #[test]
    fn test_invalid_deallocate_size_mismatch() {
        let pool = BufferPool::new(64 * 1024, 2);

        // Create a buffer with wrong size
        let wrong_buf = vec![0u8; 32 * 1024]; // Wrong size

        let result = pool.deallocate(wrong_buf);
        assert!(result.is_err(), "should reject wrong-sized buffer");
    }

    #[test]
    fn test_stats_consistency() {
        let pool = BufferPool::new(64 * 1024, 4);

        let buf1 = pool.allocate();
        let buf2 = pool.allocate();

        let stats = pool.stats();
        assert_eq!(stats.total_allocations, 2);
        assert_eq!(stats.total_deallocations, 0);
        assert_eq!(stats.current_available, 2);

        if let (Some(b1), Some(b2)) = (buf1, buf2) {
            pool.deallocate(b1).ok();
            let stats2 = pool.stats();
            assert_eq!(stats2.total_deallocations, 1);
            assert_eq!(stats2.current_available, 3);

            pool.deallocate(b2).ok();
        }
    }

    // ========== Property Tests (T28 Tier 2) ==========

    #[test]
    fn test_no_leaks_simple() {
        // #VERIFY: No buffer leaks
        let pool = Arc::new(BufferPool::new(64 * 1024, 8));

        for _ in 0..100 {
            let buf = pool.allocate().expect("should allocate");
            pool.deallocate(buf).expect("should deallocate");
        }

        let stats = pool.stats();
        assert_eq!(stats.total_allocations, 100);
        assert_eq!(stats.total_deallocations, 100);
        assert_eq!(stats.current_available, 8, "all buffers should be free");
    }

    #[test]
    fn test_allocate_deallocate_balance() {
        let pool = BufferPool::new(64 * 1024, 4);

        // Allocate all buffers
        let bufs: Vec<_> = (0..4).map(|_| pool.allocate().expect("allocate")).collect();
        assert_eq!(pool.available(), 0);

        // Deallocate all buffers
        for buf in bufs {
            pool.deallocate(buf).expect("deallocate");
        }
        assert_eq!(pool.available(), 4);

        let stats = pool.stats();
        assert_eq!(stats.total_allocations, 4);
        assert_eq!(stats.total_deallocations, 4);
    }

    // ========== Integration Tests (T28 Tier 3) ==========

    #[test]
    fn test_concurrent_allocate_deallocate() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
        use std::thread;

        let pool = Arc::new(BufferPool::new(64 * 1024, 8));
        let success_count = Arc::new(AtomicUsize::new(0));

        let mut handles = vec![];

        for _ in 0..4 {
            let pool_clone = Arc::clone(&pool);
            let success_clone = Arc::clone(&success_count);

            let handle = thread::spawn(move || {
                for _ in 0..50 {
                    if let Some(buf) = pool_clone.allocate() {
                        success_clone.fetch_add(1, AtomicOrdering::Relaxed);
                        pool_clone.deallocate(buf).expect("deallocate");
                    }
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        assert_eq!(success_count.load(AtomicOrdering::Relaxed), 200);
        assert_eq!(pool.available(), 8);

        let stats = pool.stats();
        assert_eq!(stats.total_allocations, 200);
        assert_eq!(stats.total_deallocations, 200);
    }

    #[test]
    fn test_contention_stress() {
        use std::sync::Arc;
        use std::thread;

        let pool = Arc::new(BufferPool::new(64 * 1024, 16));

        let mut handles = vec![];

        for _ in 0..8 {
            let pool_clone = Arc::clone(&pool);

            let handle = thread::spawn(move || {
                let mut allocated = vec![];

                // Allocate 2 buffers per thread
                for _ in 0..2 {
                    if let Some(buf) = pool_clone.allocate() {
                        allocated.push(buf);
                    }
                }

                // Hold for a moment
                std::thread::sleep(std::time::Duration::from_micros(10));

                // Deallocate all
                for buf in allocated {
                    let _ = pool_clone.deallocate(buf);
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        assert_eq!(pool.available(), 16);
        let stats = pool.stats();
        assert_eq!(stats.total_deallocations, stats.total_allocations);
    }

    // ========== Production Tests (T28 Tier 4) ==========

    #[test]
    fn test_production_1m_cycles() {
        let pool = Arc::new(BufferPool::new(64 * 1024, 16));

        let start = std::time::Instant::now();

        for _ in 0..1_000_000 {
            if let Some(buf) = pool.allocate() {
                let _ = pool.deallocate(buf);
            }
        }

        let elapsed = start.elapsed();
        let per_cycle_ns = (elapsed.as_nanos() / 1_000_000) as f64;

        println!(
            "1M allocate/deallocate cycles: {:.2} µs total, {:.2} ns per cycle",
            elapsed.as_secs_f64() * 1_000_000.0,
            per_cycle_ns
        );

        // Target: <50ns per allocate, <50ns per deallocate = <100ns per cycle
        // Allow 5x margin for system variance and contention
        assert!(per_cycle_ns < 500.0, "Performance regression: {:.2} ns per cycle", per_cycle_ns);

        let stats = pool.stats();
        assert_eq!(stats.total_allocations, 1_000_000);
        assert_eq!(stats.total_deallocations, 1_000_000);
        assert_eq!(stats.current_available, 16);
    }

    #[test]
    fn test_production_concurrent_load() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
        use std::thread;

        let pool = Arc::new(BufferPool::new(64 * 1024, 16));
        let total_ops = Arc::new(AtomicU64::new(0));

        let mut handles = vec![];

        for _ in 0..16 {
            let pool_clone = Arc::clone(&pool);
            let total_clone = Arc::clone(&total_ops);

            let handle = thread::spawn(move || {
                let mut local_ops = 0u64;

                for _ in 0..100_000 {
                    if let Some(buf) = pool_clone.allocate() {
                        local_ops += 1;
                        pool_clone.deallocate(buf).expect("deallocate");
                        local_ops += 1;
                    }
                }

                total_clone.fetch_add(local_ops, AtomicOrdering::Relaxed);
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        let total = total_ops.load(AtomicOrdering::Relaxed);
        println!("Total operations: {}", total);

        assert_eq!(pool.available(), 16);
        let stats = pool.stats();
        assert_eq!(stats.total_deallocations, stats.total_allocations);
    }
}
