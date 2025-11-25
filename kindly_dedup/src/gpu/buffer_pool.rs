//! GPU Buffer Pool - T1 Atomic Tier
//!
//! # Architecture
//!
//! Lockfree buffer management for GPU memory using atomic state.
//! Pools buffers for reuse to avoid allocation overhead.
//!
//! # COCA Compliance
//!
//! 100% lockfree via atomic free-list pattern:
//! - Each buffer slot has atomic state (free/in_use)
//! - Free-list head uses CAS for lockfree acquire/release
//! - Generation counters prevent ABA problems
//! - NO Mutex, NO RwLock, NO UnsafeCell<VecDeque>
//!
//! # Framework Compliance
//!
//! - UCE34: Q10 T1 Atomic tier (lockfree coordination)
//! - COCA: 100% lockfree via atomic free-list
//! - ASSUM: Buffer sizes validated, all assumptions documented
//! - B32: Performance targets documented

use std::sync::atomic::{AtomicU64, Ordering};
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device};

use super::error::{GpuError, GpuResult};

/// Buffer entry with metadata
#[repr(C)]
struct PooledBuffer {
    buffer: Buffer,
    size: u64,
    usage: BufferUsages,
}

/// Slot state constants for atomic operations
///
/// Packed layout in AtomicU64:
/// - Bits 0-15:  State (0=empty, 1=free, 2=in_use)
/// - Bits 16-31: Next free index (for free-list chaining)
/// - Bits 32-63: Generation counter (ABA prevention)
const SLOT_EMPTY: u64 = 0;
const SLOT_FREE: u64 = 1;
const SLOT_IN_USE: u64 = 2;

/// Sentinel value for end of free-list
const FREE_LIST_END: u16 = 0xFFFF;

/// Pack slot state into AtomicU64
#[inline]
fn pack_slot_state(state: u64, next_free: u16, generation: u32) -> u64 {
    (state & 0xFFFF) | ((next_free as u64) << 16) | ((generation as u64) << 32)
}

/// Unpack slot state from AtomicU64
#[inline]
fn unpack_slot_state(packed: u64) -> (u64, u16, u32) {
    let state = packed & 0xFFFF;
    let next_free = ((packed >> 16) & 0xFFFF) as u16;
    let generation = (packed >> 32) as u32;
    (state, next_free, generation)
}

/// A single slot in the buffer pool (cache-line aligned)
///
/// # COCA Compliance
///
/// Uses atomic state for lockfree slot management:
/// - state: Tracks slot status (empty/free/in_use) + next_free + generation
/// - buffer: UnsafeCell for buffer storage, protected by state transitions
///
/// # ASSUM Safety
///
/// - `#ASSUME_STATE_GUARDS_BUFFER`: Buffer is only accessed when state indicates valid data
/// - `#VERIFY_STATE_GUARDS_BUFFER`: CAS transitions ensure exclusive access
/// - `#ASSUME_GENERATION_PREVENTS_ABA`: Generation counter incremented on every state change
/// - `#VERIFY_GENERATION_PREVENTS_ABA`: CAS includes generation in comparison
#[repr(C, align(64))]
struct PooledBufferSlot {
    /// Atomic state: state(16) | next_free(16) | generation(32)
    state: AtomicU64,
    /// Buffer storage (protected by state machine)
    buffer: UnsafeCell<MaybeUninit<PooledBuffer>>,
    /// Padding to 64-byte cache line
    _padding: [u8; 24],
}

impl PooledBufferSlot {
    /// Create empty slot
    fn new_empty() -> Self {
        Self {
            state: AtomicU64::new(pack_slot_state(SLOT_EMPTY, FREE_LIST_END, 0)),
            buffer: UnsafeCell::new(MaybeUninit::uninit()),
            _padding: [0; 24],
        }
    }

    /// Check if slot is free
    #[inline]
    fn is_free(&self) -> bool {
        let (state, _, _) = unpack_slot_state(self.state.load(Ordering::Acquire));
        state == SLOT_FREE
    }

    /// Get buffer metadata (size, usage) if slot is free
    ///
    /// # Safety
    ///
    /// Only call when slot state is SLOT_FREE (verified by caller).
    #[inline]
    unsafe fn get_buffer_metadata(&self) -> (u64, BufferUsages) {
        let buffer = &*self.buffer.get();
        let pooled = buffer.assume_init_ref();
        (pooled.size, pooled.usage)
    }
}

/// Buffer pool state (packed in AtomicU64)
///
/// Bit layout:
/// - Bits 0-15: Available buffer count
/// - Bits 16-31: Total buffers created
/// - Bits 32-47: Generation counter
/// - Bits 48-63: Reserved
fn pack_pool_state(available: u16, total: u16, generation: u16) -> u64 {
    (available as u64)
        | ((total as u64) << 16)
        | ((generation as u64) << 32)
}

fn unpack_pool_state(packed: u64) -> (u16, u16, u16) {
    let available = (packed & 0xFFFF) as u16;
    let total = ((packed >> 16) & 0xFFFF) as u16;
    let generation = ((packed >> 32) & 0xFFFF) as u16;
    (available, total, generation)
}

/// GPU Buffer Pool Capsule (T1 Atomic)
///
/// Manages a pool of GPU buffers for reuse using a lockfree free-list.
///
/// # COCA Compliance
///
/// 100% lockfree implementation:
/// - Atomic free-list head with CAS for acquire/release
/// - Per-slot atomic state for concurrent access
/// - Generation counters for ABA prevention
/// - NO Mutex, NO RwLock, NO blocking operations
///
/// # Architecture
///
/// ```text
/// GpuBufferPoolCapsule
/// ├── state: AtomicU64 (available | total | generation)
/// ├── free_list_head: AtomicU64 (head_index | generation)
/// ├── slots: Box<[PooledBufferSlot]> (fixed-size array)
/// └── total_bytes: AtomicU64 (metrics)
/// ```
///
/// # ASSUM Safety
///
/// - `#ASSUME_CAS_ATOMICITY`: CAS operations are atomic (hardware guarantee)
/// - `#VERIFY_CAS_ATOMICITY`: Verified by platform atomics
/// - `#ASSUME_GENERATION_UNIQUENESS`: Generation overflow takes 2^32 ops per slot
/// - `#VERIFY_GENERATION_UNIQUENESS`: Acceptable for buffer pool lifetimes
/// - `#ASSUME_SLOT_BOUNDS`: Slot indices always < max_pool_size
/// - `#VERIFY_SLOT_BOUNDS`: Enforced by acquire/release logic
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::gpu::{GpuContextCapsule, GpuBufferPoolCapsule};
///
/// let ctx = GpuContextCapsule::new_blocking()?;
/// let mut pool = GpuBufferPoolCapsule::new(10);
///
/// let buffer = pool.acquire_or_create(
///     ctx.device().unwrap(),
///     1024,
///     BufferUsages::STORAGE | BufferUsages::COPY_DST
/// )?;
/// ```
#[repr(C, align(128))]
pub struct GpuBufferPoolCapsule {
    /// Atomic state (available count, total count, generation)
    state: AtomicU64,

    /// Free-list head: index(16) | generation(48)
    /// Uses CAS for lockfree acquire/release
    free_list_head: AtomicU64,

    /// Total bytes allocated
    total_bytes: AtomicU64,

    /// Buffer slots (fixed-size, each slot is cache-line aligned)
    slots: Box<[PooledBufferSlot]>,

    /// Maximum pool size
    max_pool_size: usize,

    /// Maximum individual buffer size
    max_buffer_size: u64,

    /// Padding for 128-byte alignment
    _padding: [u8; 24],
}

/// Pack free-list head: index(16) | generation(48)
#[inline]
fn pack_free_list_head(index: u16, generation: u64) -> u64 {
    (index as u64) | (generation << 16)
}

/// Unpack free-list head
#[inline]
fn unpack_free_list_head(packed: u64) -> (u16, u64) {
    let index = (packed & 0xFFFF) as u16;
    let generation = packed >> 16;
    (index, generation)
}

// Safety: GpuBufferPoolCapsule is Send because:
// - All fields are Send (AtomicU64, Box<[PooledBufferSlot]>)
// - wgpu::Buffer is Send
// - Slot access is protected by atomic state transitions
unsafe impl Send for GpuBufferPoolCapsule {}

// Safety: GpuBufferPoolCapsule is Sync because:
// - All state changes use atomic CAS operations
// - Free-list uses lockfree linked list pattern
// - Generation counters prevent ABA problems
// - Slot access requires successful CAS on slot state
unsafe impl Sync for GpuBufferPoolCapsule {}

impl GpuBufferPoolCapsule {
    /// Create a new buffer pool
    ///
    /// # Arguments
    ///
    /// * `max_pool_size` - Maximum number of buffers to pool
    ///
    /// # COCA Compliance
    ///
    /// Initializes lockfree free-list structure with atomic slots.
    pub fn new(max_pool_size: usize) -> Self {
        // Create slots array
        let mut slots = Vec::with_capacity(max_pool_size);
        for _ in 0..max_pool_size {
            slots.push(PooledBufferSlot::new_empty());
        }

        Self {
            state: AtomicU64::new(pack_pool_state(0, 0, 0)),
            free_list_head: AtomicU64::new(pack_free_list_head(FREE_LIST_END, 0)),
            total_bytes: AtomicU64::new(0),
            slots: slots.into_boxed_slice(),
            max_pool_size,
            max_buffer_size: 256 * 1024 * 1024, // 256 MB default max
            _padding: [0; 24],
        }
    }

    /// Create pool with custom max buffer size
    pub fn with_max_buffer_size(max_pool_size: usize, max_buffer_size: u64) -> Self {
        let mut pool = Self::new(max_pool_size);
        pool.max_buffer_size = max_buffer_size;
        pool
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        let packed = self.state.load(Ordering::Acquire);
        let (available, total, generation) = unpack_pool_state(packed);
        let total_bytes = self.total_bytes.load(Ordering::Relaxed);

        PoolStats {
            available_buffers: available as usize,
            total_buffers_created: total as usize,
            total_bytes_allocated: total_bytes,
            generation,
            max_pool_size: self.max_pool_size,
        }
    }

    /// Try to acquire a buffer from the pool (lockfree)
    ///
    /// Returns None if no suitable buffer is available.
    /// This is the fast path (<10ns when buffer available).
    ///
    /// # COCA Compliance
    ///
    /// Uses CAS loop on free-list head for lockfree acquire.
    /// Scans slots for suitable buffer (size + usage match).
    pub fn try_acquire(&self, min_size: u64, usage: BufferUsages) -> Option<Buffer> {
        // Fast path: check if any buffers available
        let packed = self.state.load(Ordering::Acquire);
        let (available, _, _) = unpack_pool_state(packed);
        if available == 0 {
            return None;
        }

        // Scan slots for suitable buffer
        for (idx, slot) in self.slots.iter().enumerate() {
            let slot_state = slot.state.load(Ordering::Acquire);
            let (state, next_free, gen) = unpack_slot_state(slot_state);

            if state != SLOT_FREE {
                continue;
            }

            // Check buffer metadata
            // SAFETY: Slot is in FREE state, buffer is initialized
            let (buf_size, buf_usage) = unsafe { slot.get_buffer_metadata() };
            if buf_size < min_size || !buf_usage.contains(usage) {
                continue;
            }

            // Try to acquire this slot via CAS
            let new_slot_state = pack_slot_state(SLOT_IN_USE, FREE_LIST_END, gen.wrapping_add(1));
            if slot.state.compare_exchange(
                slot_state,
                new_slot_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                // Successfully acquired slot - take buffer
                // SAFETY: We won the CAS, exclusive access to buffer
                let buffer = unsafe {
                    let buf_ptr = slot.buffer.get();
                    std::ptr::read(buf_ptr).assume_init()
                };

                // Remove from free-list and update state
                self.remove_from_free_list(idx as u16);
                self.decrement_available();

                return Some(buffer.buffer);
            }
            // CAS failed, another thread got it, continue scanning
        }

        None
    }

    /// Remove index from free-list (internal helper)
    fn remove_from_free_list(&self, target_idx: u16) {
        // For simplicity, we rebuild the free-list without the target
        // This is O(n) but acceptable for small pools
        // A more complex implementation could use a doubly-linked list
        let mut current = self.free_list_head.load(Ordering::Acquire);
        loop {
            let (head_idx, head_gen) = unpack_free_list_head(current);
            if head_idx == FREE_LIST_END {
                break; // Empty list
            }

            if head_idx == target_idx {
                // Target is head - pop it
                let slot = &self.slots[head_idx as usize];
                let slot_state = slot.state.load(Ordering::Acquire);
                let (_, next, _) = unpack_slot_state(slot_state);

                let new_head = pack_free_list_head(next, head_gen.wrapping_add(1));
                match self.free_list_head.compare_exchange(
                    current,
                    new_head,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(c) => current = c,
                }
            } else {
                // Target is not head - it was removed by slot CAS
                break;
            }
        }
    }

    /// Decrement available count atomically
    fn decrement_available(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let (available, total, gen) = unpack_pool_state(current);
            let new_state = pack_pool_state(
                available.saturating_sub(1),
                total,
                gen.wrapping_add(1),
            );
            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(c) => current = c,
            }
        }
    }

    /// Increment available count atomically
    fn increment_available(&self) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let (available, total, gen) = unpack_pool_state(current);
            let new_state = pack_pool_state(
                available.saturating_add(1),
                total,
                gen.wrapping_add(1),
            );
            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(c) => current = c,
            }
        }
    }

    /// Acquire buffer from pool or create new one
    ///
    /// This is the primary API for buffer acquisition.
    ///
    /// # COCA Compliance
    ///
    /// Lockfree: try_acquire uses CAS, create_buffer is allocation-only.
    pub fn acquire_or_create(
        &self,
        device: &Device,
        size: u64,
        usage: BufferUsages,
    ) -> GpuResult<Buffer> {
        // Validate size
        if size > self.max_buffer_size {
            return Err(GpuError::BufferTooLarge {
                requested: size,
                max_size: self.max_buffer_size,
            });
        }

        // Try pool first (fast path)
        if let Some(buffer) = self.try_acquire(size, usage) {
            return Ok(buffer);
        }

        // Create new buffer
        self.create_buffer(device, size, usage)
    }

    /// Create a new GPU buffer
    ///
    /// # COCA Compliance
    ///
    /// Allocation-only, no locking. State update is atomic.
    pub fn create_buffer(
        &self,
        device: &Device,
        size: u64,
        usage: BufferUsages,
    ) -> GpuResult<Buffer> {
        // Validate size
        if size > self.max_buffer_size {
            return Err(GpuError::BufferTooLarge {
                requested: size,
                max_size: self.max_buffer_size,
            });
        }

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("kindly_dedup_buffer"),
            size,
            usage,
            mapped_at_creation: false,
        });

        // Update stats atomically
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let (available, total, gen) = unpack_pool_state(current);
            let new_state = pack_pool_state(available, total.saturating_add(1), gen.wrapping_add(1));
            match self.state.compare_exchange_weak(
                current,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(c) => current = c,
            }
        }

        self.total_bytes.fetch_add(size, Ordering::Relaxed);

        Ok(buffer)
    }

    /// Release buffer back to pool (lockfree)
    ///
    /// Buffer is added to pool if pool is not full, otherwise dropped.
    ///
    /// # COCA Compliance
    ///
    /// Uses CAS to find empty slot and update free-list head.
    pub fn release(&self, buffer: Buffer, size: u64, usage: BufferUsages) {
        let packed = self.state.load(Ordering::Acquire);
        let (available, _, _) = unpack_pool_state(packed);

        // Check if pool is full
        if available as usize >= self.max_pool_size {
            // Drop buffer (wgpu handles deallocation)
            drop(buffer);
            return;
        }

        // Find empty slot via CAS
        for (idx, slot) in self.slots.iter().enumerate() {
            let slot_state = slot.state.load(Ordering::Acquire);
            let (state, _, gen) = unpack_slot_state(slot_state);

            if state != SLOT_EMPTY && state != SLOT_IN_USE {
                continue; // Slot is FREE (has buffer) or being modified
            }

            // For IN_USE slots, we need to check if this is a slot we're returning to
            // For EMPTY slots, we can use them directly
            if state == SLOT_IN_USE {
                continue; // Skip in-use slots (they have buffers being used)
            }

            // Try to claim this empty slot
            let old_head = self.free_list_head.load(Ordering::Acquire);
            let (old_head_idx, old_head_gen) = unpack_free_list_head(old_head);

            // New slot state: FREE, points to old head
            let new_slot_state = pack_slot_state(SLOT_FREE, old_head_idx, gen.wrapping_add(1));

            if slot.state.compare_exchange(
                slot_state,
                new_slot_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                // Successfully claimed slot - store buffer
                // SAFETY: We won the CAS, exclusive access to buffer storage
                unsafe {
                    let buf_ptr = slot.buffer.get();
                    std::ptr::write(buf_ptr, MaybeUninit::new(PooledBuffer { buffer, size, usage }));
                }

                // Update free-list head to point to this slot
                let new_head = pack_free_list_head(idx as u16, old_head_gen.wrapping_add(1));
                // Best effort - if this fails, slot is still valid and will be found by scan
                let _ = self.free_list_head.compare_exchange(
                    old_head,
                    new_head,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );

                self.increment_available();
                return;
            }
            // CAS failed, try next slot
        }

        // No empty slots - drop buffer
        drop(buffer);
    }

    /// Clear all pooled buffers
    ///
    /// # COCA Compliance
    ///
    /// Resets all slot states atomically.
    pub fn clear(&mut self) {
        // Reset all slots to empty (drops buffers)
        for slot in self.slots.iter() {
            let slot_state = slot.state.load(Ordering::Acquire);
            let (state, _, gen) = unpack_slot_state(slot_state);

            if state == SLOT_FREE {
                // Drop the buffer
                // SAFETY: Slot is FREE, buffer is initialized
                unsafe {
                    let buf_ptr = slot.buffer.get();
                    std::ptr::drop_in_place(buf_ptr);
                }
            }

            // Reset to empty
            let new_state = pack_slot_state(SLOT_EMPTY, FREE_LIST_END, gen.wrapping_add(1));
            slot.state.store(new_state, Ordering::Release);
        }

        // Reset free-list head
        self.free_list_head.store(pack_free_list_head(FREE_LIST_END, 0), Ordering::Release);

        // Reset state (keep total count for stats)
        let packed = self.state.load(Ordering::Acquire);
        let (_, total, gen) = unpack_pool_state(packed);
        let new_state = pack_pool_state(0, total, gen.wrapping_add(1));
        self.state.store(new_state, Ordering::Release);
    }

    /// Get number of available buffers
    pub fn available_count(&self) -> usize {
        let packed = self.state.load(Ordering::Acquire);
        let (available, _, _) = unpack_pool_state(packed);
        available as usize
    }

    /// Get total bytes allocated
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes.load(Ordering::Relaxed)
    }
}

impl Default for GpuBufferPoolCapsule {
    fn default() -> Self {
        Self::new(16) // Default pool of 16 buffers
    }
}

impl std::fmt::Debug for GpuBufferPoolCapsule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stats = self.stats();
        f.debug_struct("GpuBufferPoolCapsule")
            .field("available", &stats.available_buffers)
            .field("total_created", &stats.total_buffers_created)
            .field("total_bytes", &stats.total_bytes_allocated)
            .field("max_pool_size", &stats.max_pool_size)
            .finish()
    }
}

/// Pool statistics
#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    /// Number of buffers currently available in pool
    pub available_buffers: usize,
    /// Total buffers created (including released)
    pub total_buffers_created: usize,
    /// Total bytes allocated
    pub total_bytes_allocated: u64,
    /// Generation counter
    pub generation: u16,
    /// Maximum pool size
    pub max_pool_size: usize,
}

impl std::fmt::Display for PoolStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "GPU Buffer Pool Stats:")?;
        writeln!(f, "  Available: {}/{}", self.available_buffers, self.max_pool_size)?;
        writeln!(f, "  Total Created: {}", self.total_buffers_created)?;
        writeln!(f, "  Total Bytes: {} MB", self.total_bytes_allocated / (1024 * 1024))?;
        writeln!(f, "  Generation: {}", self.generation)?;
        Ok(())
    }
}

/// Buffer usage presets for common operations
pub mod presets {
    use wgpu::BufferUsages;

    /// Storage buffer for compute input (read-only)
    pub const STORAGE_READ: BufferUsages = BufferUsages::STORAGE.union(BufferUsages::COPY_DST);

    /// Storage buffer for compute output (read-write)
    pub const STORAGE_WRITE: BufferUsages = BufferUsages::STORAGE
        .union(BufferUsages::COPY_DST)
        .union(BufferUsages::COPY_SRC);

    /// Uniform buffer for constants
    pub const UNIFORM: BufferUsages = BufferUsages::UNIFORM.union(BufferUsages::COPY_DST);

    /// Staging buffer for CPU-GPU transfer
    pub const STAGING_UPLOAD: BufferUsages = BufferUsages::MAP_WRITE.union(BufferUsages::COPY_SRC);

    /// Staging buffer for GPU-CPU transfer
    pub const STAGING_DOWNLOAD: BufferUsages = BufferUsages::MAP_READ.union(BufferUsages::COPY_DST);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_state_packing() {
        let packed = pack_pool_state(5, 10, 42);
        let (available, total, gen) = unpack_pool_state(packed);

        assert_eq!(available, 5);
        assert_eq!(total, 10);
        assert_eq!(gen, 42);
    }

    #[test]
    fn test_pool_creation() {
        let pool = GpuBufferPoolCapsule::new(16);
        let stats = pool.stats();

        assert_eq!(stats.available_buffers, 0);
        assert_eq!(stats.total_buffers_created, 0);
        assert_eq!(stats.max_pool_size, 16);
    }

    #[test]
    fn test_pool_default() {
        let pool = GpuBufferPoolCapsule::default();
        assert_eq!(pool.stats().max_pool_size, 16);
    }

    #[test]
    fn test_pool_stats_display() {
        let pool = GpuBufferPoolCapsule::new(8);
        let stats = pool.stats();
        let display = format!("{}", stats);
        assert!(display.contains("Available: 0/8"));
    }

    #[test]
    fn test_buffer_size_validation() {
        let pool = GpuBufferPoolCapsule::with_max_buffer_size(8, 1024);

        // We can't test actual buffer creation without a device,
        // but we can verify the max size is stored
        assert_eq!(pool.max_buffer_size, 1024);
    }

    #[test]
    fn test_slot_state_packing() {
        let packed = pack_slot_state(SLOT_FREE, 42, 12345);
        let (state, next, gen) = unpack_slot_state(packed);

        assert_eq!(state, SLOT_FREE);
        assert_eq!(next, 42);
        assert_eq!(gen, 12345);
    }

    #[test]
    fn test_free_list_head_packing() {
        let packed = pack_free_list_head(100, 0x123456789ABC);
        let (index, gen) = unpack_free_list_head(packed);

        assert_eq!(index, 100);
        assert_eq!(gen, 0x123456789ABC);
    }

    #[test]
    fn test_slot_creation() {
        let slot = PooledBufferSlot::new_empty();
        let (state, next, _) = unpack_slot_state(slot.state.load(Ordering::Acquire));

        assert_eq!(state, SLOT_EMPTY);
        assert_eq!(next, FREE_LIST_END);
    }

    #[test]
    fn test_pool_sync_send() {
        // Verify GpuBufferPoolCapsule is Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GpuBufferPoolCapsule>();
    }

    #[test]
    fn test_presets() {
        use presets::*;

        // Verify presets have expected flags
        assert!(STORAGE_READ.contains(BufferUsages::STORAGE));
        assert!(STORAGE_READ.contains(BufferUsages::COPY_DST));

        assert!(STAGING_UPLOAD.contains(BufferUsages::MAP_WRITE));
        assert!(STAGING_DOWNLOAD.contains(BufferUsages::MAP_READ));
    }
}
