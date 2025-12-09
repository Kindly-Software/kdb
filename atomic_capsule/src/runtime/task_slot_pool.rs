//! Task Slot Pool - Pre-allocated Lockfree Task Storage
//!
//! T1 Atomic tier capsule for async runtime task management.
//!
//! # Architecture
//!
//! TaskSlotPoolCapsule provides a fixed-capacity pool of pre-allocated TaskSlots
//! with O(1) lockfree acquire/release operations:
//!
//! - Capacity: 1024 slots (configurable via const generics)
//! - Slot size: 64 bytes each (cache-aligned)
//! - Total memory: 64KB for default configuration
//!
//! # Allocation Strategy
//!
//! Uses a lockfree free-list with generation counters:
//! 1. Free slots are tracked via atomic bitfield (32 × u32 = 1024 bits)
//! 2. Allocation scans for first free bit using trailing_zeros()
//! 3. Release sets bit and increments slot generation
//!
//! # Performance (B32 Targets)
//!
//! - acquire_slot: <50ns (atomic bitfield scan + CAS)
//! - release_slot: <30ns (atomic OR + generation bump)
//! - is_available: <10ns (atomic load)
//!
//! # Safety (ASSUM Framework)
//!
//! #ASSUME_SLOT_ARRAY_BOUNDS: Index validation before array access
//! #ASSUME_BITFIELD_CONSISTENCY: Bitfield accurately reflects slot state
//! #ASSUME_GENERATION_MONOTONIC: Generation counters only increase

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::task_slot::{TaskSlot, TaskSlotState};

/// Default pool capacity (1024 slots)
pub const DEFAULT_POOL_CAPACITY: usize = 1024;

/// Number of u32 words needed to track 1024 slots (32 slots per word)
const BITFIELD_WORDS: usize = DEFAULT_POOL_CAPACITY / 32;

/// Error types for TaskSlotPool operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSlotPoolError {
    /// No free slots available
    PoolExhausted,
    /// Invalid slot index
    InvalidIndex,
    /// Slot is not in expected state
    InvalidState,
    /// Generation mismatch (ABA detection)
    GenerationMismatch,
    /// Pool is shutting down
    ShuttingDown,
}

/// Handle to an allocated task slot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskSlotHandle {
    /// Index into the slot array
    pub index: u32,
    /// Generation at time of allocation (ABA prevention)
    pub generation: u32,
}

impl TaskSlotHandle {
    /// Create a new handle
    #[inline]
    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Pack handle into a single u64 for atomic storage
    #[inline]
    pub const fn pack(&self) -> u64 {
        ((self.generation as u64) << 32) | (self.index as u64)
    }

    /// Unpack handle from u64
    #[inline]
    pub const fn unpack(packed: u64) -> Self {
        Self {
            index: packed as u32,
            generation: (packed >> 32) as u32,
        }
    }
}

/// Pre-allocated lockfree task slot pool
///
/// # Memory Layout
///
/// ```text
/// TaskSlotPoolCapsule (256B total, 64B aligned for cache efficiency)
/// ├── [0-7]   stats: AtomicU64 (allocated count + peak count packed)
/// ├── [8-11]  state: AtomicU32 (pool state: Active/Draining/Shutdown)
/// ├── [12-15] hint_word: AtomicU32 (last successful allocation word)
/// ├── [16-23] slots_ptr: Box pointer (8B on 64-bit)
/// ├── [24-63] _padding1: 40B padding for cache line
/// ├── [64-191] free_bitfield: [AtomicU32; 32] (128B, 1024 bits tracking free slots)
/// └── [192-255] _padding2: 64B cold stats padding
/// ```
#[repr(C, align(64))]
pub struct TaskSlotPoolCapsule {
    // === Cache Line 1 (Hot path: stats + state) ===
    /// Statistics: low 32 bits = currently allocated, high 32 bits = peak allocated
    stats: AtomicU64,

    /// Pool state: 0=Active, 1=Draining, 2=Shutdown
    state: AtomicU32,

    /// Hint for allocation: last word that had a free slot
    hint_word: AtomicU32,

    /// Pre-allocated slot array (64KB, heap allocated)
    /// Placed here to keep hot fields in first cache line
    slots: Box<[TaskSlot; DEFAULT_POOL_CAPACITY]>,

    /// Padding to complete first cache line (64B - 8 - 4 - 4 - 8 = 40B)
    _padding1: [u8; 40],

    // === Cache Lines 2-3 (Bitfield: 128B) ===
    /// Bitfield tracking free slots (1 = free, 0 = allocated)
    /// Using inverted logic: free slots have bit SET for efficient trailing_zeros scan
    free_bitfield: [AtomicU32; BITFIELD_WORDS],

    // === Cache Line 4 (Cold: padding) ===
    /// Padding for alignment
    _padding2: [u8; 64],
}

/// Pool state values
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolState {
    /// Pool is active and accepting allocations
    Active = 0,
    /// Pool is draining (no new allocations, existing tasks complete)
    Draining = 1,
    /// Pool is shut down
    Shutdown = 2,
}

impl TaskSlotPoolCapsule {
    /// Create a new task slot pool with all slots initially free
    pub fn new() -> Self {
        // Initialize all slots
        let slots: Box<[TaskSlot; DEFAULT_POOL_CAPACITY]> = {
            let mut vec = Vec::with_capacity(DEFAULT_POOL_CAPACITY);
            for i in 0..DEFAULT_POOL_CAPACITY {
                vec.push(TaskSlot::with_id(i as u32));
            }
            vec.into_boxed_slice()
                .try_into()
                .expect("Vec has correct length")
        };

        // Initialize bitfield with all bits set (all slots free)
        let mut free_bitfield: [AtomicU32; BITFIELD_WORDS] = unsafe {
            // SAFETY: AtomicU32 is a valid zero-initialized value
            core::mem::zeroed()
        };
        for word in &mut free_bitfield {
            *word = AtomicU32::new(0xFFFF_FFFF); // All 32 bits set = all free
        }

        Self {
            stats: AtomicU64::new(0),
            state: AtomicU32::new(PoolState::Active as u32),
            hint_word: AtomicU32::new(0),
            slots,
            _padding1: [0u8; 40],
            free_bitfield,
            _padding2: [0u8; 64],
        }
    }

    /// Get pool capacity
    #[inline]
    pub const fn capacity(&self) -> usize {
        DEFAULT_POOL_CAPACITY
    }

    /// Get number of currently allocated slots
    #[inline]
    pub fn allocated_count(&self) -> u32 {
        (self.stats.load(Ordering::Acquire) & 0xFFFF_FFFF) as u32
    }

    /// Get peak allocation count
    #[inline]
    pub fn peak_count(&self) -> u32 {
        (self.stats.load(Ordering::Acquire) >> 32) as u32
    }

    /// Get number of available slots
    #[inline]
    pub fn available_count(&self) -> u32 {
        DEFAULT_POOL_CAPACITY as u32 - self.allocated_count()
    }

    /// Check if pool is active
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state.load(Ordering::Acquire) == PoolState::Active as u32
    }

    /// Check if pool is draining
    #[inline]
    pub fn is_draining(&self) -> bool {
        self.state.load(Ordering::Acquire) == PoolState::Draining as u32
    }

    /// Check if pool is shut down
    #[inline]
    pub fn is_shutdown(&self) -> bool {
        self.state.load(Ordering::Acquire) == PoolState::Shutdown as u32
    }

    /// Acquire a free slot from the pool
    ///
    /// Returns a handle containing the slot index and generation.
    ///
    /// # Performance
    ///
    /// - Best case: <30ns (hint word has free slot)
    /// - Typical: <50ns (scan 1-2 words)
    /// - Worst case: O(capacity/32) words scanned
    ///
    /// # Safety
    ///
    /// #ASSUME_BITFIELD_CONSISTENCY: Bitfield accurately tracks slot availability
    pub fn acquire(&self) -> Result<TaskSlotHandle, TaskSlotPoolError> {
        // Check pool state
        if self.state.load(Ordering::Acquire) != PoolState::Active as u32 {
            return Err(TaskSlotPoolError::ShuttingDown);
        }

        // Start from hint word (last successful allocation location)
        let hint = self.hint_word.load(Ordering::Relaxed) as usize;

        // Scan bitfield for a free slot
        for offset in 0..BITFIELD_WORDS {
            let word_idx = (hint + offset) % BITFIELD_WORDS;
            let word = &self.free_bitfield[word_idx];

            // Load current word value
            let mut current = word.load(Ordering::Acquire);

            while current != 0 {
                // Find first set bit (free slot)
                let bit_idx = current.trailing_zeros();
                if bit_idx >= 32 {
                    break; // No free bits in this word
                }

                let bit_mask = 1u32 << bit_idx;
                let slot_idx = (word_idx * 32 + bit_idx as usize) as u32;

                // #ASSUME_SLOT_ARRAY_BOUNDS: Validate index
                if slot_idx >= DEFAULT_POOL_CAPACITY as u32 {
                    break;
                }

                // Try to clear the bit (mark as allocated)
                match word.compare_exchange_weak(
                    current,
                    current & !bit_mask,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Successfully claimed the bit, now allocate the slot
                        let slot = &self.slots[slot_idx as usize];
                        let gen = slot.generation();

                        match slot.try_allocate(gen) {
                            Some(new_gen) => {
                                // Update hint for next allocation
                                self.hint_word.store(word_idx as u32, Ordering::Relaxed);

                                // Update statistics
                                self.increment_allocated();

                                return Ok(TaskSlotHandle::new(slot_idx, new_gen));
                            }
                            None => {
                                // Slot allocation failed (race condition), restore bit
                                word.fetch_or(bit_mask, Ordering::Release);
                                // Retry with updated current value
                                current = word.load(Ordering::Acquire);
                                continue;
                            }
                        }
                    }
                    Err(new_current) => {
                        // CAS failed, retry with new value
                        current = new_current;
                    }
                }
            }
        }

        Err(TaskSlotPoolError::PoolExhausted)
    }

    /// Release a slot back to the pool
    ///
    /// # Safety
    ///
    /// #ASSUME_GENERATION_MONOTONIC: Generation is incremented on release
    pub fn release(&self, handle: TaskSlotHandle) -> Result<(), TaskSlotPoolError> {
        // #ASSUME_SLOT_ARRAY_BOUNDS: Validate index
        if handle.index >= DEFAULT_POOL_CAPACITY as u32 {
            return Err(TaskSlotPoolError::InvalidIndex);
        }

        let slot = &self.slots[handle.index as usize];

        // Verify generation matches
        if slot.generation() != handle.generation {
            return Err(TaskSlotPoolError::GenerationMismatch);
        }

        // Release the slot (clears data, increments generation)
        let _new_gen = slot.release();

        // Set the free bit
        let word_idx = handle.index as usize / 32;
        let bit_idx = handle.index % 32;
        let bit_mask = 1u32 << bit_idx;

        self.free_bitfield[word_idx].fetch_or(bit_mask, Ordering::Release);

        // Update statistics
        self.decrement_allocated();

        Ok(())
    }

    /// Get a reference to a slot by handle
    ///
    /// Returns None if handle is invalid or generation mismatches.
    pub fn get(&self, handle: TaskSlotHandle) -> Option<&TaskSlot> {
        if handle.index >= DEFAULT_POOL_CAPACITY as u32 {
            return None;
        }

        let slot = &self.slots[handle.index as usize];

        if slot.generation() != handle.generation {
            return None;
        }

        Some(slot)
    }

    /// Get a reference to a slot by index (no generation check)
    ///
    /// # Safety
    ///
    /// Caller must ensure index is valid and handle generation checking.
    #[inline]
    pub fn get_unchecked(&self, index: u32) -> Option<&TaskSlot> {
        if index >= DEFAULT_POOL_CAPACITY as u32 {
            return None;
        }
        Some(&self.slots[index as usize])
    }

    /// Iterate over all allocated slots
    ///
    /// Returns an iterator yielding (handle, slot) pairs for non-free slots.
    pub fn iter_allocated(&self) -> impl Iterator<Item = (TaskSlotHandle, &TaskSlot)> {
        self.slots.iter().enumerate().filter_map(|(idx, slot)| {
            if !slot.is_free() {
                Some((
                    TaskSlotHandle::new(idx as u32, slot.generation()),
                    slot,
                ))
            } else {
                None
            }
        })
    }

    /// Start draining the pool (no new allocations)
    pub fn start_drain(&self) -> bool {
        self.state
            .compare_exchange(
                PoolState::Active as u32,
                PoolState::Draining as u32,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// Shut down the pool
    pub fn shutdown(&self) {
        self.state.store(PoolState::Shutdown as u32, Ordering::Release);
    }

    /// Reset the pool to initial state (all slots free)
    ///
    /// # Safety
    ///
    /// Caller must ensure no handles are in use.
    pub fn reset(&self) {
        // Reset all slots
        for slot in self.slots.iter() {
            slot.reset();
        }

        // Reset bitfield (all free)
        for word in &self.free_bitfield {
            word.store(0xFFFF_FFFF, Ordering::Release);
        }

        // Reset statistics
        self.stats.store(0, Ordering::Release);

        // Reset state to active
        self.state.store(PoolState::Active as u32, Ordering::Release);

        // Reset hint
        self.hint_word.store(0, Ordering::Release);
    }

    /// Increment allocated count and update peak
    #[inline]
    fn increment_allocated(&self) {
        loop {
            let current = self.stats.load(Ordering::Acquire);
            let allocated = (current & 0xFFFF_FFFF) as u32;
            let peak = (current >> 32) as u32;

            let new_allocated = allocated.saturating_add(1);
            let new_peak = peak.max(new_allocated);
            let new_stats = ((new_peak as u64) << 32) | (new_allocated as u64);

            if self
                .stats
                .compare_exchange_weak(current, new_stats, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Decrement allocated count
    #[inline]
    fn decrement_allocated(&self) {
        loop {
            let current = self.stats.load(Ordering::Acquire);
            let allocated = (current & 0xFFFF_FFFF) as u32;
            let peak = (current >> 32) as u32;

            let new_allocated = allocated.saturating_sub(1);
            let new_stats = ((peak as u64) << 32) | (new_allocated as u64);

            if self
                .stats
                .compare_exchange_weak(current, new_stats, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }
}

impl Default for TaskSlotPoolCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for TaskSlotPoolCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TaskSlotPoolCapsule")
            .field("capacity", &self.capacity())
            .field("allocated", &self.allocated_count())
            .field("available", &self.available_count())
            .field("peak", &self.peak_count())
            .field("state", &self.state.load(Ordering::Relaxed))
            .finish()
    }
}

// SAFETY: TaskSlotPoolCapsule only contains atomic types and a Box
unsafe impl Send for TaskSlotPoolCapsule {}
unsafe impl Sync for TaskSlotPoolCapsule {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_pool_new() {
        let pool = TaskSlotPoolCapsule::new();
        assert_eq!(pool.capacity(), DEFAULT_POOL_CAPACITY);
        assert_eq!(pool.allocated_count(), 0);
        assert_eq!(pool.available_count(), DEFAULT_POOL_CAPACITY as u32);
        assert!(pool.is_active());
    }

    #[test]
    fn test_pool_acquire_release() {
        let pool = TaskSlotPoolCapsule::new();

        // Acquire a slot
        let handle = pool.acquire().expect("Should acquire slot");
        assert_eq!(pool.allocated_count(), 1);
        assert_eq!(pool.available_count(), DEFAULT_POOL_CAPACITY as u32 - 1);

        // Get the slot
        let slot = pool.get(handle).expect("Should get slot");
        assert_eq!(slot.state(), TaskSlotState::Allocated);

        // Release the slot
        pool.release(handle).expect("Should release slot");
        assert_eq!(pool.allocated_count(), 0);
        assert_eq!(pool.available_count(), DEFAULT_POOL_CAPACITY as u32);
    }

    #[test]
    fn test_pool_handle_pack_unpack() {
        let handle = TaskSlotHandle::new(42, 123);
        let packed = handle.pack();
        let unpacked = TaskSlotHandle::unpack(packed);
        assert_eq!(unpacked.index, 42);
        assert_eq!(unpacked.generation, 123);
    }

    #[test]
    fn test_pool_generation_check() {
        let pool = TaskSlotPoolCapsule::new();

        let handle = pool.acquire().expect("Should acquire");
        pool.release(handle).expect("Should release");

        // Old handle should now fail generation check
        assert!(pool.get(handle).is_none());

        // But slot should be available for new allocation
        let new_handle = pool.acquire().expect("Should acquire again");
        // Could be same index, but different generation
        if new_handle.index == handle.index {
            assert_ne!(new_handle.generation, handle.generation);
        }
    }

    #[test]
    fn test_pool_acquire_all() {
        let pool = TaskSlotPoolCapsule::new();
        let mut handles = Vec::new();

        // Acquire all slots
        for _ in 0..DEFAULT_POOL_CAPACITY {
            let handle = pool.acquire().expect("Should acquire");
            handles.push(handle);
        }

        assert_eq!(pool.allocated_count(), DEFAULT_POOL_CAPACITY as u32);
        assert_eq!(pool.available_count(), 0);

        // Next acquire should fail
        assert_eq!(pool.acquire(), Err(TaskSlotPoolError::PoolExhausted));

        // Release one
        pool.release(handles.pop().unwrap()).expect("Should release");
        assert_eq!(pool.allocated_count(), DEFAULT_POOL_CAPACITY as u32 - 1);

        // Now acquire should succeed
        let handle = pool.acquire().expect("Should acquire after release");
        handles.push(handle);

        // Release all
        for handle in handles {
            pool.release(handle).expect("Should release");
        }
        assert_eq!(pool.allocated_count(), 0);
    }

    #[test]
    fn test_pool_peak_tracking() {
        let pool = TaskSlotPoolCapsule::new();

        // Acquire 10 slots
        let mut handles: Vec<_> = (0..10).map(|_| pool.acquire().unwrap()).collect();
        assert_eq!(pool.peak_count(), 10);

        // Release 5
        for _ in 0..5 {
            pool.release(handles.pop().unwrap()).unwrap();
        }
        assert_eq!(pool.peak_count(), 10); // Peak unchanged

        // Acquire 8 more
        for _ in 0..8 {
            handles.push(pool.acquire().unwrap());
        }
        assert_eq!(pool.peak_count(), 13); // New peak
    }

    #[test]
    fn test_pool_drain_shutdown() {
        let pool = TaskSlotPoolCapsule::new();

        let handle = pool.acquire().expect("Should acquire");
        assert!(pool.is_active());

        // Start draining
        assert!(pool.start_drain());
        assert!(pool.is_draining());

        // Cannot acquire while draining
        assert_eq!(pool.acquire(), Err(TaskSlotPoolError::ShuttingDown));

        // Can still release
        pool.release(handle).expect("Should release");

        // Shutdown
        pool.shutdown();
        assert!(pool.is_shutdown());
    }

    #[test]
    fn test_pool_reset() {
        let pool = TaskSlotPoolCapsule::new();

        // Allocate some slots
        let _handles: Vec<_> = (0..50).map(|_| pool.acquire().unwrap()).collect();
        assert_eq!(pool.allocated_count(), 50);

        // Reset
        pool.reset();
        assert_eq!(pool.allocated_count(), 0);
        assert_eq!(pool.peak_count(), 0);
        assert!(pool.is_active());

        // Should be able to acquire again
        let handle = pool.acquire().expect("Should acquire after reset");
        assert_eq!(handle.generation, 0); // Generation reset
    }

    #[test]
    fn test_pool_iter_allocated() {
        let pool = TaskSlotPoolCapsule::new();

        // Acquire 5 slots
        let handles: Vec<_> = (0..5).map(|_| pool.acquire().unwrap()).collect();

        // Count allocated via iterator
        let count = pool.iter_allocated().count();
        assert_eq!(count, 5);

        // Release 2
        pool.release(handles[0]).unwrap();
        pool.release(handles[2]).unwrap();

        let count = pool.iter_allocated().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_pool_invalid_handle() {
        let pool = TaskSlotPoolCapsule::new();

        // Invalid index
        let bad_handle = TaskSlotHandle::new(DEFAULT_POOL_CAPACITY as u32 + 1, 0);
        assert_eq!(pool.release(bad_handle), Err(TaskSlotPoolError::InvalidIndex));
        assert!(pool.get(bad_handle).is_none());
    }

    #[test]
    fn test_pool_concurrent_acquire_release() {
        let pool = Arc::new(TaskSlotPoolCapsule::new());
        let num_threads = 8;
        let ops_per_thread = 100;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let pool = Arc::clone(&pool);
                thread::spawn(move || {
                    let mut local_handles = Vec::new();

                    for _ in 0..ops_per_thread {
                        // Acquire
                        if let Ok(handle) = pool.acquire() {
                            local_handles.push(handle);
                        }

                        // Release half
                        if local_handles.len() > 5 {
                            for _ in 0..local_handles.len() / 2 {
                                if let Some(h) = local_handles.pop() {
                                    let _ = pool.release(h);
                                }
                            }
                        }
                    }

                    // Return remaining handles for cleanup
                    local_handles
                })
            })
            .collect();

        // Collect all remaining handles
        let mut all_handles = Vec::new();
        for handle in handles {
            all_handles.extend(handle.join().unwrap());
        }

        // Verify consistency
        let allocated = pool.allocated_count();
        assert_eq!(allocated as usize, all_handles.len());

        // Clean up
        for handle in all_handles {
            pool.release(handle).expect("Should release");
        }
        assert_eq!(pool.allocated_count(), 0);
    }

    #[test]
    fn test_pool_slot_state_transitions() {
        let pool = TaskSlotPoolCapsule::new();

        let handle = pool.acquire().unwrap();
        let slot = pool.get(handle).unwrap();

        // Initial state: Allocated
        assert_eq!(slot.state(), TaskSlotState::Allocated);

        // Transition to Running
        assert!(slot.start_poll(handle.generation));
        assert_eq!(slot.state(), TaskSlotState::Running);

        // Transition to Suspended
        assert!(slot.suspend(handle.generation));
        assert_eq!(slot.state(), TaskSlotState::Suspended);

        // Back to Running
        assert!(slot.start_poll(handle.generation));
        assert_eq!(slot.state(), TaskSlotState::Running);

        // Complete
        assert!(slot.complete(handle.generation));
        assert_eq!(slot.state(), TaskSlotState::Completed);

        // Release
        pool.release(handle).unwrap();
    }
}
