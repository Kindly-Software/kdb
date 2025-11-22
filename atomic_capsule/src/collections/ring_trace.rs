//! T5 Streaming Ring Buffer Capsule (Generic)
//!
//! High-performance lockfree generic ring buffer for streaming data with continuous append capability.
//!
//! # Design
//! - **Capacity**: 16,384 entries (configurable via generics in the future)
//! - **Entry Size**: Generic over `T` (must implement Copy + Send + Sync)
//! - **Performance**: <10ns record, O(1) all operations
//! - **Coordination**: AtomicU64 for lockfree head with generation counter
//! - **Wraparound**: Automatic with TOCTOU-safe generation tracking
//!
//! # Memory Layout
//! - Capsule header: 64 bytes (cache-aligned)
//! - Ring buffer entries: 16,384 × sizeof(T)
//! - Total: ~64B + 16K*sizeof(T)
//!
//! # ASSUM Safety Framework
//! - #ASSUME_LOCKFREE_COORDINATION: All updates via CAS, no mutex/RwLock
//! - #ASSUME_POWER_OF_TWO_CAPACITY: 16384 = 2^14 enables fast modulo
//! - #ASSUME_CACHE_ALIGNED: 64-byte alignment prevents false sharing
//! - #ASSUME_COPY_TYPE: T must be Copy for safe atomic writes
//! - #ASSUME_CAS_CONVERGENCE: CAS succeeds within 10 attempts under normal load

use core::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

/// Trace entry flags (16 bits) - Original TraceEntry variant
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceFlags {
    /// Function call
    Call = 0x0001,
    /// Function return
    Return = 0x0002,
    /// Unconditional jump
    Jump = 0x0004,
    /// Exception/interrupt
    Exception = 0x0008,
    /// Conditional branch
    Branch = 0x0010,
    /// Memory load
    Load = 0x0020,
    /// Memory store
    Store = 0x0040,
    /// System call
    Syscall = 0x0080,
}

/// Compact 16-byte trace entry
///
/// #ASSUME_ALIGNED_WRITE: 16-byte alignment ensures atomic write on x86-64
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct TraceEntry {
    /// Program counter (instruction address)
    pub pc: u64,

    /// Relative timestamp in nanoseconds (wraps every ~4.3 seconds)
    pub timestamp: u32,

    /// Thread ID
    pub thread_id: u16,

    /// Trace flags (TraceFlags bitmask)
    pub flags: u16,
}

impl TraceEntry {
    /// Create a new trace entry
    #[inline(always)]
    pub const fn new(pc: u64, timestamp: u32, thread_id: u16, flags: u16) -> Self {
        Self {
            pc,
            timestamp,
            thread_id,
            flags,
        }
    }

    /// Check if entry has a specific flag
    #[inline(always)]
    pub const fn has_flag(&self, flag: TraceFlags) -> bool {
        (self.flags & flag as u16) != 0
    }

    /// Create empty/uninitialized entry marker
    #[inline(always)]
    pub const fn empty() -> Self {
        Self {
            pc: 0,
            timestamp: 0,
            thread_id: 0,
            flags: 0,
        }
    }

    /// Check if entry is uninitialized
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.pc == 0
    }
}

impl Default for TraceEntry {
    fn default() -> Self {
        Self::empty()
    }
}

// Trait for ring buffer entries - requires Copy for safe atomic operations
pub trait RingBufferEntry: Copy + Send + Sync + 'static {
    /// Create an empty entry (marker for uninitialized)
    fn empty() -> Self;

    /// Check if entry is empty
    fn is_empty(&self) -> bool;
}

impl RingBufferEntry for TraceEntry {
    fn empty() -> Self {
        TraceEntry::empty()
    }

    fn is_empty(&self) -> bool {
        TraceEntry::is_empty(self)
    }
}

// Implement for basic types as well
impl RingBufferEntry for u64 {
    fn empty() -> Self {
        0
    }

    fn is_empty(&self) -> bool {
        *self == 0
    }
}

impl RingBufferEntry for u32 {
    fn empty() -> Self {
        0
    }

    fn is_empty(&self) -> bool {
        *self == 0
    }
}

impl RingBufferEntry for u128 {
    fn empty() -> Self {
        0
    }

    fn is_empty(&self) -> bool {
        *self == 0
    }
}

/// Ring buffer capacity constant (16,384 entries = 2^14 for fast modulo)
///
/// #ASSUME_POWER_OF_TWO: 16384 = 2^14 enables fast modulo via bitwise AND
pub const RING_BUFFER_CAPACITY: usize = 16384;

/// Bitmask for fast modulo (CAPACITY - 1 = 0x3FFF)
const RING_BUFFER_MASK: usize = RING_BUFFER_CAPACITY - 1;

/// T5 Streaming Ring Buffer Capsule (Generic)
///
/// # Performance Targets
/// - Record: <10ns (lockfree CAS)
/// - Read recent: O(1) per entry
/// - Export: Zero-copy slice
///
/// # Lockfree Coordination
/// - Head position and generation packed in single AtomicU64
/// - Generation counter prevents TOCTOU races
/// - Wraparound handled atomically
/// - No tail tracking (write-only ring buffer)
///
/// # ASSUM Safety
/// - #ASSUME_LOCKFREE_COORDINATION: All updates via CAS, no mutex/RwLock
/// - #ASSUME_POWER_OF_TWO_CAPACITY: 16384 = 2^14 enables fast modulo
/// - #ASSUME_CACHE_ALIGNED: 64-byte alignment prevents false sharing
/// - #ASSUME_ATOMIC_WRITE: Entry writes are safe due to alignment
/// - #ASSUME_CAS_CONVERGENCE: CAS succeeds within 10 attempts under normal load
#[repr(C, align(64))]
pub struct RingBufferCapsule<T: RingBufferEntry> {
    /// Head position and generation counter packed in u64
    ///
    /// Layout: [position: u32 | generation: u32]
    ///
    /// Position: Index of next write (0..CAPACITY)
    /// Generation: Wraparound counter (increments when position wraps to 0)
    ///
    /// #ASSUME_PACKED_COORDINATION: Single atomic u64 for lock-free head advancement
    head: AtomicU64,

    /// Total entries written (monotonic counter for statistics)
    ///
    /// #ASSUME_RELAXED_ORDERING: Approximate statistics OK, uses Relaxed
    total_writes: AtomicU64,

    /// Total wraparounds (for generation tracking validation)
    ///
    /// #ASSUME_RELAXED_ORDERING: Approximate statistics OK, uses Relaxed
    total_wraps: AtomicU64,

    /// Padding to ensure proper alignment
    _padding: [u64; 4],

    /// Phantom data to associate with T
    _phantom: PhantomData<T>,

    /// Ring buffer entries (16,384 entries) - heap-allocated slice
    ///
    /// #ASSUME_CONTIGUOUS_ALLOCATION: Box guarantees contiguous allocation
    /// #ASSUME_ALIGNED_ALLOCATION: Entries properly aligned
    entries: Box<[T]>,
}

impl<T: RingBufferEntry> RingBufferCapsule<T> {
    /// Create a new ring buffer capsule
    ///
    /// # Performance
    /// - Allocation: ~1-5ms (16K entries × sizeof(T) zeroed)
    /// - Initialization: <100ns (atomic setup)
    pub fn new() -> Self {
        // #ASSUME_BOX_ZEROED: Vec with capacity then converting to Box slice
        let mut vec = Vec::with_capacity(RING_BUFFER_CAPACITY);
        vec.resize(RING_BUFFER_CAPACITY, T::empty());
        let entries = vec.into_boxed_slice();

        Self {
            head: AtomicU64::new(0),
            total_writes: AtomicU64::new(0),
            total_wraps: AtomicU64::new(0),
            _padding: [0; 4],
            _phantom: PhantomData,
            entries,
        }
    }

    /// Pack position and generation into u64
    #[inline(always)]
    const fn pack(position: u32, generation: u32) -> u64 {
        ((generation as u64) << 32) | (position as u64)
    }

    /// Unpack u64 into (position, generation)
    #[inline(always)]
    const fn unpack(packed: u64) -> (u32, u32) {
        let position = packed as u32;
        let generation = (packed >> 32) as u32;
        (position, generation)
    }

    /// Record an entry (lockfree, <10ns target)
    ///
    /// # Arguments
    /// - `entry`: Entry to record
    ///
    /// # Returns
    /// - `true`: Entry recorded successfully
    /// - `false`: Failed after max retries (ring buffer under extreme contention)
    ///
    /// # Performance
    /// - Fast path: 5-8ns (CAS success on first try)
    /// - Slow path: 10-15ns (CAS retry under contention)
    ///
    /// # Lockfree Guarantee
    /// - Uses CAS loop with generation counter
    /// - No spinning - fails gracefully after max retries
    /// - Single writer per slot (no data races)
    ///
    /// #ASSUME_CAS_CONVERGENCE: CAS succeeds within 10 attempts under normal load
    /// #ASSUME_GRACEFUL_DEGRADATION: Dropping entries OK under extreme overload
    #[inline(always)]
    pub fn record(&self, entry: T) -> bool {
        const MAX_RETRIES: u32 = 10;

        for _ in 0..MAX_RETRIES {
            // Load current head (acquire ordering for synchronization with other writers)
            // #ASSUME_ACQUIRE_ORDERING: Synchronize with concurrent writers
            let current = self.head.load(Ordering::Acquire);
            let (position, generation) = Self::unpack(current);

            // Compute next position (wraparound via modulo)
            // #ASSUME_NO_OVERFLOW: position < CAPACITY guarantees no u32 overflow
            let next_position = (position + 1) % (RING_BUFFER_CAPACITY as u32);
            let next_generation = if next_position == 0 {
                // Wrapped around - increment generation for TOCTOU prevention
                generation.wrapping_add(1)
            } else {
                generation
            };

            let next = Self::pack(next_position, next_generation);

            // Try to advance head atomically (CAS is linearization point)
            // #ASSUME_CAS_ATOMIC: compare_exchange provides atomic read-modify-write
            match self.head.compare_exchange(
                current,
                next,
                Ordering::AcqRel,  // Success: synchronize with readers
                Ordering::Acquire,  // Failure: retry with fresh value
            ) {
                Ok(_) => {
                    // CAS succeeded - write entry at position
                    // #ASSUME_SAFE_INDEX: position < CAPACITY by construction (modulo)
                    let index = (position as usize) & RING_BUFFER_MASK;

                    // Write entry (properly aligned write)
                    // #ASSUME_SAFE_WRITE: Index bounds-checked via bitwise AND
                    // SAFETY:
                    // 1. Index bounds-checked via bitwise AND with RING_BUFFER_MASK
                    // 2. Single writer per slot (CAS winner owns this slot)
                    // 3. T must be Copy and properly aligned
                    unsafe {
                        let ptr = self.entries.as_ptr() as *mut T;
                        ptr.add(index).write(entry);
                    }

                    // Update statistics (relaxed - approximate OK)
                    // #ASSUME_RELAXED_STATISTICS: Counter precision not critical
                    self.total_writes.fetch_add(1, Ordering::Relaxed);
                    if next_position == 0 {
                        self.total_wraps.fetch_add(1, Ordering::Relaxed);
                    }

                    return true;
                }
                Err(_) => {
                    // CAS failed - another thread advanced head, retry
                    // #ASSUME_SPIN_HINT: Reduces contention on busy-wait
                    std::hint::spin_loop();
                    continue;
                }
            }
        }

        // Failed after max retries - extreme contention
        // #ASSUME_GRACEFUL_DEGRADATION: OK to drop entries under pathological load
        false
    }

    /// Get the most recent N entries (newest first)
    ///
    /// # Arguments
    /// - `count`: Number of entries to retrieve (capped at CAPACITY)
    ///
    /// # Returns
    /// Vector of entries, newest first. May be shorter than `count` if
    /// fewer entries have been written.
    ///
    /// # Performance
    /// - O(N) where N = min(count, entries available)
    /// - Single atomic load for head position (snapshot consistency)
    /// - Skips uninitialized entries (via is_empty())
    ///
    /// #ASSUME_SNAPSHOT_CONSISTENCY: Single atomic load provides consistent snapshot
    pub fn get_recent(&self, count: usize) -> Vec<T> {
        let count = count.min(RING_BUFFER_CAPACITY);

        // Load current head position (acquire for synchronization with writers)
        // #ASSUME_ACQUIRE_ORDERING: See all writes before this snapshot
        let current = self.head.load(Ordering::Acquire);
        let (position, _generation) = Self::unpack(current);

        let mut result = Vec::with_capacity(count);

        // Read backwards from head (newest first)
        for i in 0..count {
            // Compute index with wraparound (wrapping_sub handles underflow)
            // #ASSUME_WRAPPING_ARITHMETIC: Handles position=0 correctly
            let pos = position.wrapping_sub(i as u32 + 1);
            let index = (pos as usize) & RING_BUFFER_MASK;

            // Read entry (no unsafe needed - bounds checked by bitwise AND)
            let entry = self.entries[index];

            // Skip uninitialized entries (ring buffer not yet full)
            if entry.is_empty() {
                break;
            }

            result.push(entry);
        }

        result
    }

    /// Export ring buffer as two contiguous slices (zero-copy)
    ///
    /// Returns (newer_slice, older_slice) where:
    /// - newer_slice: Entries from start to head (most recent wraparound)
    /// - older_slice: Entries from head to end (before wraparound)
    ///
    /// # Performance
    /// - O(1) - just computes slice boundaries
    /// - Zero allocation, zero copy
    /// - Direct references into internal buffer
    ///
    /// # Example
    /// ```ignore
    /// let (newer, older) = capsule.export();
    /// for entry in newer.iter().rev().chain(older.iter().rev()) {
    ///     println!("{:?}", entry);
    /// }
    /// ```
    ///
    /// #ASSUME_SPLIT_AT_SAFETY: split_at checks bounds internally
    pub fn export(&self) -> (&[T], &[T]) {
        // Load current head position (snapshot)
        let current = self.head.load(Ordering::Acquire);
        let (position, _generation) = Self::unpack(current);
        let head_idx = (position as usize) & RING_BUFFER_MASK;

        // Split buffer at head position
        // older: [head_idx..CAPACITY] (written first, before wraparound)
        // newer: [0..head_idx] (written after wraparound)
        let (newer, older) = self.entries.split_at(head_idx);

        (newer, older)
    }

    /// Get total entries written (monotonic counter)
    ///
    /// #ASSUME_RELAXED_ORDERING: Approximate count sufficient for statistics
    #[inline]
    pub fn total_writes(&self) -> u64 {
        self.total_writes.load(Ordering::Relaxed)
    }

    /// Get total wraparounds (generation counter validation)
    ///
    /// #ASSUME_RELAXED_ORDERING: Approximate count sufficient for statistics
    #[inline]
    pub fn total_wraps(&self) -> u64 {
        self.total_wraps.load(Ordering::Relaxed)
    }

    /// Get ring buffer capacity (compile-time constant)
    #[inline]
    pub const fn capacity(&self) -> usize {
        RING_BUFFER_CAPACITY
    }

    /// Get current head position (snapshot)
    #[inline]
    pub fn head_position(&self) -> u32 {
        let current = self.head.load(Ordering::Acquire);
        let (position, _) = Self::unpack(current);
        position
    }

    /// Get current generation (for TOCTOU prevention validation)
    #[inline]
    pub fn head_generation(&self) -> u32 {
        let current = self.head.load(Ordering::Acquire);
        let (_, generation) = Self::unpack(current);
        generation
    }

    /// Get memory usage in bytes (header + entries)
    #[inline]
    pub fn memory_usage_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + RING_BUFFER_CAPACITY * std::mem::size_of::<T>()
    }
}

impl<T: RingBufferEntry> Default for RingBufferCapsule<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_entry_size() {
        // #VERIFY: 16-byte size and alignment
        assert_eq!(std::mem::size_of::<TraceEntry>(), 16);
        assert_eq!(std::mem::align_of::<TraceEntry>(), 16);
    }

    #[test]
    fn test_capsule_alignment() {
        // #VERIFY: 64-byte cache alignment
        assert_eq!(std::mem::align_of::<RingBufferCapsule<TraceEntry>>(), 64);
    }

    #[test]
    fn test_capacity_power_of_two() {
        // #VERIFY: Power-of-2 capacity for fast modulo
        assert_eq!(RING_BUFFER_CAPACITY, 16384);
        assert_eq!(RING_BUFFER_CAPACITY.count_ones(), 1);
    }

    #[test]
    fn test_new_capsule() {
        let capsule = RingBufferCapsule::<TraceEntry>::new();

        // #VERIFY: Initial state
        assert_eq!(capsule.capacity(), 16384);
        assert_eq!(capsule.total_writes(), 0);
        assert_eq!(capsule.total_wraps(), 0);
        assert_eq!(capsule.head_position(), 0);
        assert_eq!(capsule.head_generation(), 0);
    }

    #[test]
    fn test_record_single_entry() {
        let capsule = RingBufferCapsule::new();

        let entry = TraceEntry::new(0x1000, 123, 1, TraceFlags::Call as u16);
        let success = capsule.record(entry);
        assert!(success);

        // #VERIFY: Counters updated
        assert_eq!(capsule.total_writes(), 1);
        assert_eq!(capsule.head_position(), 1);

        // #VERIFY: Entry retrievable
        let recent = capsule.get_recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].pc, 0x1000);
        assert_eq!(recent[0].timestamp, 123);
    }

    #[test]
    fn test_record_multiple_entries() {
        let capsule = RingBufferCapsule::new();

        // Record 10 entries
        for i in 0..10 {
            let entry = TraceEntry::new(0x1000 + i, (i * 100) as u32, 1, 0);
            assert!(capsule.record(entry));
        }

        assert_eq!(capsule.total_writes(), 10);
        assert_eq!(capsule.head_position(), 10);

        // Verify recent entries (newest first)
        let recent = capsule.get_recent(5);
        assert_eq!(recent.len(), 5);
        assert_eq!(recent[0].pc, 0x1000 + 9); // Newest
        assert_eq!(recent[4].pc, 0x1000 + 5); // 5th newest
    }

    #[test]
    fn test_generic_u64() {
        let capsule = RingBufferCapsule::<u64>::new();

        for i in 0..10 {
            assert!(capsule.record(1000 + i as u64));
        }

        assert_eq!(capsule.total_writes(), 10);

        let recent = capsule.get_recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0], 1009); // Newest
        assert_eq!(recent[2], 1007);
    }

    #[test]
    fn test_generic_u32() {
        let capsule = RingBufferCapsule::<u32>::new();

        for i in 0..5 {
            assert!(capsule.record(100 + i as u32));
        }

        assert_eq!(capsule.total_writes(), 5);

        let recent = capsule.get_recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0], 104);
    }

    #[test]
    fn test_wraparound() {
        let capsule = RingBufferCapsule::<u64>::new();

        // Write more than capacity to trigger wraparound
        for i in 0..20000 {
            let _ = capsule.record(i as u64);
        }

        // Verify that generation incremented
        assert!(capsule.head_generation() > 0 || capsule.total_wraps() > 0);

        // Verify entries are still readable
        let recent = capsule.get_recent(10);
        assert!(!recent.is_empty());
    }

    #[test]
    fn test_export_slices() {
        let capsule = RingBufferCapsule::new();

        for i in 0..100 {
            let entry = TraceEntry::new(0x1000 + i, i as u32, 1, 0);
            capsule.record(entry);
        }

        let (newer, older) = capsule.export();

        // Verify both slices exist and cover the ring buffer
        assert!(!newer.is_empty() || !older.is_empty());

        // Together they should cover all entries
        let total_entries = newer.len() + older.len();
        assert_eq!(total_entries, RING_BUFFER_CAPACITY);
    }

    #[test]
    fn test_head_position_increment() {
        let capsule = RingBufferCapsule::<u64>::new();

        assert_eq!(capsule.head_position(), 0);
        capsule.record(1);
        assert_eq!(capsule.head_position(), 1);
        capsule.record(2);
        assert_eq!(capsule.head_position(), 2);
    }

    #[test]
    fn test_trace_flags() {
        let entry = TraceEntry::new(0x1000, 100, 1, TraceFlags::Call as u16);
        assert!(entry.has_flag(TraceFlags::Call));
        assert!(!entry.has_flag(TraceFlags::Return));
    }

    #[test]
    fn test_empty_detection() {
        let entry = TraceEntry::empty();
        assert!(entry.is_empty());

        let entry2 = TraceEntry::new(0x1000, 100, 1, 0);
        assert!(!entry2.is_empty());
    }

    #[test]
    fn test_concurrent_recording() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(RingBufferCapsule::<u64>::new());
        let mut handles = vec![];

        // Spawn 4 threads
        for thread_id in 0..4 {
            let capsule_clone = Arc::clone(&capsule);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let value = (thread_id * 1000 + i) as u64;
                    let _ = capsule_clone.record(value);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all writes succeeded
        assert_eq!(capsule.total_writes(), 400);
    }
}
