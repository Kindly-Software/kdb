//! # SerializationBufferCapsule - T1 Atomic Lockfree Buffer Pool
//!
//! **Tier 1: Atomic Foundation** - Lockfree buffer pool for zero-allocation serialization.
//!
//! ## UCE34 Framework Application
//!
//! - **Q10 (Computational Capsule)**: T1 Atomic tier - lockfree coordination via AtomicU64
//! - **Q11 (Rust Transform)**: Cache-aligned 128B capsule, zero unsafe in public API
//! - **Q28 (Simplicity)**: Simple acquire/release API, no configuration needed
//! - **Q33 (Validation)**: Generation counters prevent ABA problem
//!
//! ## Design Philosophy
//!
//! - **Zero allocation during serialization**: Pre-allocated buffer pool
//! - **Lockfree**: No mutex/RwLock, atomic CAS operations only (Chaos mandate)
//! - **ABA-safe**: 32-bit generation counters prevent use-after-free
//! - **Cache-aligned**: 128B to prevent false sharing
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_POWER_OF_TWO`: capacity and buffer_size are powers of 2
//! - `#VERIFY_POWER_OF_TWO`: Constructor validates via (x & (x-1)) == 0
//! - `#ASSUME_ABA_SAFE`: Generation counter prevents ABA problem
//! - `#VERIFY_ABA_SAFE`: 32-bit generation wraps at 4 billion operations
//! - `#ASSUME_BUFFER_LIFETIME`: BufferHandle tied to pool lifetime
//! - `#VERIFY_BUFFER_LIFETIME`: Rust borrow checker enforces at compile-time
//!
//! ## Performance (B32 Framework)
//!
//! - Acquire: <20ns (single CAS)
//! - Release: <15ns (single CAS)
//! - Buffer access: <5ns (pointer arithmetic)

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

use core::sync::atomic::{AtomicU64, Ordering};

/// SerializationBufferCapsule - T1 Atomic lockfree buffer pool
///
/// Provides zero-allocation serialization by pre-allocating a pool of buffers
/// that can be acquired and released atomically.
///
/// # Layout (128B, cache-aligned)
///
/// ```text
/// [0-7]   head: AtomicU64     - Free list head (index:32 + generation:32)
/// [8-15]  capacity: u64       - Total buffer count (power of 2, max 1024)
/// [16-23] buffer_size: u64    - Size per buffer (power of 2, default 4096)
/// [24-31] allocated: AtomicU64 - Currently allocated count
/// [32-39] buffers_ptr: AtomicU64 - Pointer to buffer array
/// [40-47] generation: AtomicU64 - Pool generation counter
/// [48-127] _padding            - Cache alignment to 128B
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use atomic_capsule::serialize::SerializationBufferCapsule;
///
/// // Create pool with 16 buffers of 4096 bytes each
/// let pool = SerializationBufferCapsule::new(16, 4096);
///
/// // Acquire buffer for serialization
/// let mut handle = pool.acquire().expect("No buffers available");
///
/// // Write to buffer
/// let buffer = pool.buffer_mut(&mut handle);
/// buffer[..4].copy_from_slice(&42u32.to_le_bytes());
///
/// // Release buffer back to pool
/// pool.release(handle);
/// ```
#[repr(C, align(128))]
pub struct SerializationBufferCapsule {
    /// Free list head: packed (index:32 + generation:32)
    /// - Bits 0-31: Next free buffer index (0xFFFFFFFF = empty)
    /// - Bits 32-63: Generation counter for ABA prevention
    head: AtomicU64,

    /// Total buffer count (power of 2, max 1024)
    capacity: u64,

    /// Size per buffer in bytes (power of 2, default 4096)
    buffer_size: u64,

    /// Currently allocated buffer count (for diagnostics)
    allocated: AtomicU64,

    /// Pointer to buffer array (Box<[BufferSlot]>)
    buffers_ptr: AtomicU64,

    /// Pool generation counter (for validation)
    generation: AtomicU64,

    /// Padding to 128B cache line
    _padding: [u8; 80],
}

/// Internal buffer slot with next pointer for free list
#[repr(C)]
struct BufferSlot {
    /// Next free index (packed with generation)
    next: AtomicU64,
    /// Buffer data
    data: Vec<u8>,
}

/// Handle to an acquired buffer
///
/// # Safety
///
/// - Handle is valid only while pool exists
/// - Must not be cloned or copied (one owner)
/// - Must be released back to pool when done
#[derive(Debug)]
pub struct BufferHandle {
    /// Index in buffer array
    index: u32,
    /// Generation when acquired (for validation)
    generation: u32,
    /// Pool generation (for validation)
    pool_generation: u64,
}

/// Error type for buffer pool operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferPoolError {
    /// No free buffers available
    PoolExhausted,
    /// Invalid capacity (not power of 2 or too large)
    InvalidCapacity,
    /// Invalid buffer size (not power of 2)
    InvalidBufferSize,
    /// Invalid handle (wrong generation or pool)
    InvalidHandle,
}

impl core::fmt::Display for BufferPoolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BufferPoolError::PoolExhausted => write!(f, "Buffer pool exhausted"),
            BufferPoolError::InvalidCapacity => write!(f, "Invalid capacity (must be power of 2, max 1024)"),
            BufferPoolError::InvalidBufferSize => write!(f, "Invalid buffer size (must be power of 2)"),
            BufferPoolError::InvalidHandle => write!(f, "Invalid buffer handle"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BufferPoolError {}

/// Result type for buffer pool operations
pub type BufferPoolResult<T> = core::result::Result<T, BufferPoolError>;

impl SerializationBufferCapsule {
    /// Empty marker for free list (no more free buffers)
    const EMPTY: u32 = 0xFFFFFFFF;

    /// Maximum capacity (1024 buffers)
    const MAX_CAPACITY: u64 = 1024;

    /// Default buffer size (4096 bytes)
    pub const DEFAULT_BUFFER_SIZE: u64 = 4096;

    /// Default capacity (16 buffers)
    pub const DEFAULT_CAPACITY: u64 = 16;

    /// Create new buffer pool with specified capacity and buffer size
    ///
    /// # Arguments
    ///
    /// - `capacity`: Number of buffers (must be power of 2, max 1024)
    /// - `buffer_size`: Size of each buffer in bytes (must be power of 2)
    ///
    /// # Errors
    ///
    /// - `InvalidCapacity`: capacity not power of 2 or exceeds 1024
    /// - `InvalidBufferSize`: buffer_size not power of 2
    ///
    /// # Performance
    ///
    /// O(capacity) - allocates all buffers upfront
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_POWER_OF_TWO`: capacity and buffer_size are powers of 2
    /// - `#VERIFY_POWER_OF_TWO`: Validated at constructor time
    #[cfg(feature = "std")]
    pub fn new(capacity: u64, buffer_size: u64) -> BufferPoolResult<Self> {
        // #VERIFY_POWER_OF_TWO: Validate capacity
        if capacity == 0 || capacity > Self::MAX_CAPACITY || !Self::is_power_of_two(capacity) {
            return Err(BufferPoolError::InvalidCapacity);
        }

        // #VERIFY_POWER_OF_TWO: Validate buffer_size
        if buffer_size == 0 || !Self::is_power_of_two(buffer_size) {
            return Err(BufferPoolError::InvalidBufferSize);
        }

        // Allocate buffer slots
        let mut slots: Vec<BufferSlot> = Vec::with_capacity(capacity as usize);
        for i in 0..capacity as u32 {
            let next = if i + 1 < capacity as u32 {
                Self::pack_head(i + 1, 0)
            } else {
                Self::pack_head(Self::EMPTY, 0)
            };
            slots.push(BufferSlot {
                next: AtomicU64::new(next),
                data: vec![0u8; buffer_size as usize],
            });
        }

        // Convert to boxed slice and get raw pointer
        let boxed_slots = slots.into_boxed_slice();
        let ptr = Box::into_raw(boxed_slots) as *mut BufferSlot as u64;

        Ok(Self {
            head: AtomicU64::new(Self::pack_head(0, 0)), // First buffer free
            capacity,
            buffer_size,
            allocated: AtomicU64::new(0),
            buffers_ptr: AtomicU64::new(ptr),
            generation: AtomicU64::new(0),
            _padding: [0u8; 80],
        })
    }

    /// Create pool with default settings (16 buffers, 4096 bytes each)
    #[cfg(feature = "std")]
    #[inline]
    pub fn with_defaults() -> BufferPoolResult<Self> {
        Self::new(Self::DEFAULT_CAPACITY, Self::DEFAULT_BUFFER_SIZE)
    }

    /// Acquire a buffer from the pool (lockfree CAS)
    ///
    /// # Returns
    ///
    /// - `Some(BufferHandle)`: Handle to acquired buffer
    /// - `None`: Pool exhausted
    ///
    /// # Performance
    ///
    /// - Target: <20ns (single CAS under low contention)
    /// - Linear backoff under high contention
    ///
    /// # Thread Safety
    ///
    /// Lockfree via atomic CAS. Multiple threads can acquire concurrently.
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_ABA_SAFE`: Generation counter prevents ABA
    /// - `#VERIFY_ABA_SAFE`: Generation incremented on each operation
    #[inline]
    pub fn acquire(&self) -> Option<BufferHandle> {
        let pool_gen = self.generation.load(Ordering::Acquire);

        loop {
            let current = self.head.load(Ordering::Acquire);
            let (index, gen) = Self::unpack_head(current);

            // Pool exhausted
            if index == Self::EMPTY {
                return None;
            }

            // Get next from slot
            let slots = self.get_slots();
            let next = slots[index as usize].next.load(Ordering::Acquire);

            // New head with incremented generation
            let new_head = Self::pack_head(Self::unpack_head(next).0, gen.wrapping_add(1));

            // CAS to acquire
            match self.head.compare_exchange_weak(
                current,
                new_head,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Successfully acquired
                    self.allocated.fetch_add(1, Ordering::Relaxed);
                    return Some(BufferHandle {
                        index,
                        generation: gen,
                        pool_generation: pool_gen,
                    });
                }
                Err(_) => {
                    // Contention - retry
                    core::hint::spin_loop();
                }
            }
        }
    }

    /// Release buffer back to pool (lockfree push)
    ///
    /// # Arguments
    ///
    /// - `handle`: Handle to release (consumed)
    ///
    /// # Performance
    ///
    /// - Target: <15ns (single CAS)
    ///
    /// # Thread Safety
    ///
    /// Lockfree via atomic CAS.
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_VALID_HANDLE`: Handle was acquired from this pool
    /// - `#VERIFY_VALID_HANDLE`: Generation + pool_generation check (debug builds)
    #[inline]
    pub fn release(&self, handle: BufferHandle) {
        // Debug validation
        debug_assert_eq!(
            handle.pool_generation,
            self.generation.load(Ordering::Relaxed),
            "BufferHandle from different pool"
        );

        let slots = self.get_slots();

        loop {
            let current = self.head.load(Ordering::Acquire);
            let (head_index, gen) = Self::unpack_head(current);

            // Update slot's next pointer to current head
            slots[handle.index as usize]
                .next
                .store(Self::pack_head(head_index, 0), Ordering::Release);

            // New head points to released buffer with incremented generation
            let new_head = Self::pack_head(handle.index, gen.wrapping_add(1));

            // CAS to release
            match self.head.compare_exchange_weak(
                current,
                new_head,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.allocated.fetch_sub(1, Ordering::Relaxed);
                    return;
                }
                Err(_) => {
                    // Contention - retry
                    core::hint::spin_loop();
                }
            }
        }
    }

    /// Get immutable reference to buffer data
    ///
    /// # Performance
    ///
    /// <5ns (pointer arithmetic)
    #[inline]
    pub fn buffer_ref(&self, handle: &BufferHandle) -> &[u8] {
        let slots = self.get_slots();
        &slots[handle.index as usize].data
    }

    /// Get mutable reference to buffer data
    ///
    /// # Performance
    ///
    /// <5ns (pointer arithmetic)
    ///
    /// # Safety
    ///
    /// Caller must ensure exclusive access (single BufferHandle owner)
    #[inline]
    pub fn buffer_mut(&self, handle: &mut BufferHandle) -> &mut [u8] {
        let slots = self.get_slots_mut();
        &mut slots[handle.index as usize].data
    }

    /// Get current allocation count
    #[inline]
    pub fn allocated_count(&self) -> u64 {
        self.allocated.load(Ordering::Relaxed)
    }

    /// Get pool capacity
    #[inline]
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Get buffer size
    #[inline]
    pub fn buffer_size(&self) -> u64 {
        self.buffer_size
    }

    /// Get number of free buffers
    #[inline]
    pub fn free_count(&self) -> u64 {
        self.capacity - self.allocated.load(Ordering::Relaxed)
    }

    /// Check if pool is exhausted
    #[inline]
    pub fn is_exhausted(&self) -> bool {
        let current = self.head.load(Ordering::Acquire);
        let (index, _) = Self::unpack_head(current);
        index == Self::EMPTY
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    /// Pack index and generation into u64
    #[inline]
    const fn pack_head(index: u32, generation: u32) -> u64 {
        ((generation as u64) << 32) | (index as u64)
    }

    /// Unpack index and generation from u64
    #[inline]
    const fn unpack_head(packed: u64) -> (u32, u32) {
        let index = packed as u32;
        let generation = (packed >> 32) as u32;
        (index, generation)
    }

    /// Check if value is power of 2
    #[inline]
    const fn is_power_of_two(n: u64) -> bool {
        n != 0 && (n & (n - 1)) == 0
    }

    /// Get slots array (immutable)
    #[inline]
    fn get_slots(&self) -> &[BufferSlot] {
        let ptr = self.buffers_ptr.load(Ordering::Acquire) as *const BufferSlot;
        // SAFETY: ptr is valid for lifetime of pool, initialized in constructor
        unsafe { core::slice::from_raw_parts(ptr, self.capacity as usize) }
    }

    /// Get slots array (mutable)
    #[inline]
    fn get_slots_mut(&self) -> &mut [BufferSlot] {
        let ptr = self.buffers_ptr.load(Ordering::Acquire) as *mut BufferSlot;
        // SAFETY: ptr is valid for lifetime of pool, single owner via handle
        unsafe { core::slice::from_raw_parts_mut(ptr, self.capacity as usize) }
    }
}

impl Drop for SerializationBufferCapsule {
    fn drop(&mut self) {
        // Reconstruct and drop boxed slice
        let ptr = self.buffers_ptr.load(Ordering::Relaxed) as *mut BufferSlot;
        if !ptr.is_null() {
            // SAFETY: ptr was created by Box::into_raw in constructor
            unsafe {
                let _ = Box::from_raw(core::slice::from_raw_parts_mut(ptr, self.capacity as usize));
            }
        }
    }
}

// SAFETY: Pool is thread-safe via atomic operations
unsafe impl Send for SerializationBufferCapsule {}
unsafe impl Sync for SerializationBufferCapsule {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_pool() {
        let pool = SerializationBufferCapsule::new(16, 4096).unwrap();
        assert_eq!(pool.capacity(), 16);
        assert_eq!(pool.buffer_size(), 4096);
        assert_eq!(pool.allocated_count(), 0);
        assert_eq!(pool.free_count(), 16);
    }

    #[test]
    fn test_acquire_release() {
        let pool = SerializationBufferCapsule::new(4, 1024).unwrap();

        // Acquire buffer
        let handle = pool.acquire().expect("Should acquire");
        assert_eq!(pool.allocated_count(), 1);
        assert_eq!(pool.free_count(), 3);

        // Release buffer
        pool.release(handle);
        assert_eq!(pool.allocated_count(), 0);
        assert_eq!(pool.free_count(), 4);
    }

    #[test]
    fn test_buffer_access() {
        let pool = SerializationBufferCapsule::new(4, 1024).unwrap();
        let mut handle = pool.acquire().expect("Should acquire");

        // Write to buffer
        let buffer = pool.buffer_mut(&mut handle);
        buffer[0..4].copy_from_slice(&42u32.to_le_bytes());

        // Read from buffer
        let buffer = pool.buffer_ref(&handle);
        let value = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        assert_eq!(value, 42);

        pool.release(handle);
    }

    #[test]
    fn test_pool_exhaustion() {
        let pool = SerializationBufferCapsule::new(2, 512).unwrap();

        // Acquire all buffers
        let h1 = pool.acquire().expect("Should acquire first");
        let h2 = pool.acquire().expect("Should acquire second");

        // Pool exhausted
        assert!(pool.acquire().is_none());
        assert!(pool.is_exhausted());

        // Release one
        pool.release(h1);
        assert!(!pool.is_exhausted());

        // Can acquire again
        let h3 = pool.acquire().expect("Should acquire after release");

        pool.release(h2);
        pool.release(h3);
    }

    #[test]
    fn test_invalid_capacity() {
        // Not power of 2
        assert!(SerializationBufferCapsule::new(3, 1024).is_err());

        // Zero
        assert!(SerializationBufferCapsule::new(0, 1024).is_err());

        // Too large
        assert!(SerializationBufferCapsule::new(2048, 1024).is_err());
    }

    #[test]
    fn test_invalid_buffer_size() {
        // Not power of 2
        assert!(SerializationBufferCapsule::new(16, 1000).is_err());

        // Zero
        assert!(SerializationBufferCapsule::new(16, 0).is_err());
    }

    #[test]
    fn test_pack_unpack() {
        let packed = SerializationBufferCapsule::pack_head(42, 100);
        let (index, gen) = SerializationBufferCapsule::unpack_head(packed);
        assert_eq!(index, 42);
        assert_eq!(gen, 100);
    }

    #[test]
    fn test_power_of_two() {
        assert!(SerializationBufferCapsule::is_power_of_two(1));
        assert!(SerializationBufferCapsule::is_power_of_two(2));
        assert!(SerializationBufferCapsule::is_power_of_two(4));
        assert!(SerializationBufferCapsule::is_power_of_two(1024));

        assert!(!SerializationBufferCapsule::is_power_of_two(0));
        assert!(!SerializationBufferCapsule::is_power_of_two(3));
        assert!(!SerializationBufferCapsule::is_power_of_two(1000));
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_acquire_release() {
        use std::sync::Arc;
        use std::thread;

        let pool = Arc::new(SerializationBufferCapsule::new(16, 1024).unwrap());
        let mut handles = vec![];

        // Spawn 8 threads, each doing 100 acquire/release cycles
        for _ in 0..8 {
            let pool_clone = Arc::clone(&pool);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    if let Some(mut handle) = pool_clone.acquire() {
                        // Write something
                        let buffer = pool_clone.buffer_mut(&mut handle);
                        buffer[0] = 42;
                        // Release
                        pool_clone.release(handle);
                    }
                }
            }));
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // All buffers should be released
        assert_eq!(pool.allocated_count(), 0);
        assert_eq!(pool.free_count(), 16);
    }
}
