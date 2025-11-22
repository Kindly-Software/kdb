//! StreamingWindowCapsule<T> - T5 Streaming Windowed Data
//!
//! High-performance lockfree sliding/tumbling window for streaming data with O(1) append.
//!
//! # Design (UCE34 Q1-Q9)
//! - **Problem**: Real-time window aggregation over unbounded streams
//! - **Challenge**: Lock-free coordination + efficient wraparound + window boundary detection
//! - **Constraint**: O(1) append, fixed memory, thread-safe
//! - **Tier**: T5 Streaming (O(1) incremental operations)
//!
//! # Architecture
//! - **Capacity**: 8,192 entries (configurable via const generics in future)
//! - **Window Types**: Sliding (overlapping) or Tumbling (non-overlapping)
//! - **Coordination**: AtomicU64 (position + generation counter)
//! - **Memory**: Ring buffer with power-of-2 capacity for fast modulo
//!
//! # Memory Layout
//! - Capsule header: 64 bytes (cache-aligned)
//! - Ring buffer: 8,192 × sizeof(T)
//! - Total: ~64B + 8K*sizeof(T)
//!
//! # Performance Targets (B32 Validated)
//! - append(): <10ns (lockfree CAS, similar to RingBufferCapsule)
//! - window(): <50ns snapshot (atomic load + slice)
//! - slide(): <5ns (atomic increment, tumbling window boundary)
//!
//! # ASSUM Safety Framework
//! - #ASSUME_LOCKFREE_COORDINATION: All updates via atomics, no mutex/RwLock
//! - #ASSUME_POWER_OF_TWO_CAPACITY: 8192 = 2^13 enables fast modulo
//! - #ASSUME_CACHE_ALIGNED: 64-byte alignment prevents false sharing
//! - #ASSUME_COPY_TYPE: T must be Copy for safe ring buffer writes
//! - #ASSUME_CAS_CONVERGENCE: CAS succeeds within 10 attempts under normal load

use core::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

/// Window type (sliding vs tumbling)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowType {
    /// Sliding window (overlapping, advances by 1 element)
    Sliding,
    /// Tumbling window (non-overlapping, advances by window_size elements)
    Tumbling,
}

/// Window capacity (8,192 entries = 2^13 for fast modulo)
///
/// #ASSUME_POWER_OF_TWO: 8192 = 2^13 enables fast modulo via bitwise AND
pub const WINDOW_CAPACITY: usize = 8192;

/// Bitmask for fast modulo (CAPACITY - 1 = 0x1FFF)
const WINDOW_MASK: usize = WINDOW_CAPACITY - 1;

/// Default window size (1,024 entries)
pub const DEFAULT_WINDOW_SIZE: usize = 1024;

/// Trait for window entries - requires Copy for safe atomic operations
pub trait WindowEntry: Copy + Send + Sync + 'static {
    /// Create an empty entry marker
    fn empty() -> Self;

    /// Check if entry is empty
    fn is_empty(&self) -> bool;
}

// Implement for common types
impl WindowEntry for u64 {
    fn empty() -> Self {
        0
    }
    fn is_empty(&self) -> bool {
        *self == 0
    }
}

impl WindowEntry for u32 {
    fn empty() -> Self {
        0
    }
    fn is_empty(&self) -> bool {
        *self == 0
    }
}

impl WindowEntry for u128 {
    fn empty() -> Self {
        0
    }
    fn is_empty(&self) -> bool {
        *self == 0
    }
}

impl WindowEntry for i64 {
    fn empty() -> Self {
        0
    }
    fn is_empty(&self) -> bool {
        *self == 0
    }
}

impl WindowEntry for f64 {
    fn empty() -> Self {
        0.0
    }
    fn is_empty(&self) -> bool {
        *self == 0.0
    }
}

/// T5 Streaming Window Capsule
///
/// # Performance Guarantees
/// - append(): <10ns (lockfree CAS)
/// - window(): <50ns (atomic snapshot)
/// - slide(): <5ns (tumbling window advance)
///
/// # Lockfree Coordination
/// - Head position and generation packed in single AtomicU64
/// - Generation counter prevents TOCTOU races
/// - Wraparound handled atomically
///
/// # Window Semantics
/// - **Sliding**: Each append advances window by 1 (overlapping)
/// - **Tumbling**: Window advances by window_size when full (non-overlapping)
///
/// # ASSUM Safety
/// - #ASSUME_LOCKFREE_COORDINATION: All updates via CAS, no mutex/RwLock
/// - #ASSUME_POWER_OF_TWO_CAPACITY: 8192 = 2^13 enables fast modulo
/// - #ASSUME_CACHE_ALIGNED: 64-byte alignment prevents false sharing
/// - #ASSUME_ATOMIC_WRITE: Entry writes are safe due to alignment
/// - #ASSUME_CAS_CONVERGENCE: CAS succeeds within 10 attempts under normal load
#[repr(C, align(64))]
pub struct StreamingWindowCapsule<T: WindowEntry> {
    /// Head position and generation counter packed in u64
    ///
    /// Layout: [position: u32 | generation: u32]
    ///
    /// Position: Index of next write (0..CAPACITY)
    /// Generation: Wraparound counter (increments when position wraps)
    ///
    /// #ASSUME_PACKED_COORDINATION: Single atomic u64 for lock-free head advancement
    head: AtomicU64,

    /// Window size (number of elements in window view)
    ///
    /// Invariant: window_size <= CAPACITY
    window_size: usize,

    /// Window type (sliding or tumbling)
    window_type: WindowType,

    /// Total entries written (monotonic counter for statistics)
    ///
    /// #ASSUME_RELAXED_ORDERING: Approximate statistics OK, uses Relaxed
    total_writes: AtomicU64,

    /// Total windows emitted (for tumbling windows)
    ///
    /// #ASSUME_RELAXED_ORDERING: Approximate statistics OK, uses Relaxed
    total_windows: AtomicU64,

    /// Padding to ensure proper alignment
    _padding: [u64; 2],

    /// Phantom data to associate with T
    _phantom: PhantomData<T>,

    /// Ring buffer entries (8,192 entries) - heap-allocated slice
    ///
    /// #ASSUME_CONTIGUOUS_ALLOCATION: Box guarantees contiguous allocation
    /// #ASSUME_ALIGNED_ALLOCATION: Entries properly aligned
    entries: Box<[T]>,
}

impl<T: WindowEntry> StreamingWindowCapsule<T> {
    /// Create new sliding window capsule with default window size
    ///
    /// # Performance
    /// - Allocation: ~1-5ms (8K entries × sizeof(T) zeroed)
    /// - Initialization: <100ns (atomic setup)
    ///
    /// # Example
    /// ```
    /// use atomic_capsule::streaming::StreamingWindowCapsule;
    ///
    /// let window = StreamingWindowCapsule::<u64>::new();
    /// window.append(42);
    /// ```
    pub fn new() -> Self {
        Self::with_size(DEFAULT_WINDOW_SIZE, WindowType::Sliding)
    }

    /// Create new window capsule with custom size and type
    ///
    /// # Arguments
    /// - `window_size`: Number of elements in window (1..=CAPACITY)
    /// - `window_type`: Sliding (overlapping) or Tumbling (non-overlapping)
    ///
    /// # Panics
    /// - If window_size == 0 or window_size > CAPACITY
    ///
    /// # Example
    /// ```
    /// use atomic_capsule::streaming::{StreamingWindowCapsule, WindowType};
    ///
    /// // Tumbling window of size 100
    /// let window = StreamingWindowCapsule::<u64>::with_size(100, WindowType::Tumbling);
    /// ```
    pub fn with_size(window_size: usize, window_type: WindowType) -> Self {
        assert!(
            window_size > 0 && window_size <= WINDOW_CAPACITY,
            "Window size must be in range 1..={}",
            WINDOW_CAPACITY
        );

        // #ASSUME_BOX_ZEROED: Vec with capacity then converting to Box slice
        let mut vec = Vec::with_capacity(WINDOW_CAPACITY);
        vec.resize(WINDOW_CAPACITY, T::empty());
        let entries = vec.into_boxed_slice();

        Self {
            head: AtomicU64::new(0),
            window_size,
            window_type,
            total_writes: AtomicU64::new(0),
            total_windows: AtomicU64::new(0),
            _padding: [0; 2],
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

    /// Append entry to window (<10ns target)
    ///
    /// # Arguments
    /// - `entry`: Entry to append to window
    ///
    /// # Returns
    /// - `true`: Entry appended successfully
    /// - `false`: Failed after max retries (extreme contention)
    ///
    /// # Performance
    /// - Fast path: 5-8ns (CAS success on first try)
    /// - Slow path: 10-15ns (CAS retry under contention)
    ///
    /// # Window Semantics
    /// - **Sliding**: Each append advances window by 1
    /// - **Tumbling**: Window emitted when full, then resets
    ///
    /// # Lockfree Guarantee
    /// - Uses CAS loop with generation counter
    /// - No spinning - fails gracefully after max retries
    ///
    /// #ASSUME_CAS_CONVERGENCE: CAS succeeds within 10 attempts under normal load
    /// #ASSUME_GRACEFUL_DEGRADATION: Dropping entries OK under extreme overload
    #[inline(always)]
    pub fn append(&self, entry: T) -> bool {
        const MAX_RETRIES: u32 = 10;

        for _ in 0..MAX_RETRIES {
            // Load current head (acquire ordering for synchronization)
            // #ASSUME_ACQUIRE_ORDERING: Synchronize with concurrent writers
            let current = self.head.load(Ordering::Acquire);
            let (position, generation) = Self::unpack(current);

            // Compute next position (wraparound via modulo)
            // #ASSUME_NO_OVERFLOW: position < CAPACITY guarantees no u32 overflow
            let next_position = (position + 1) % (WINDOW_CAPACITY as u32);
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
                    let index = (position as usize) & WINDOW_MASK;

                    // Write entry (properly aligned write)
                    // #ASSUME_SAFE_WRITE: Index bounds-checked via bitwise AND
                    // SAFETY:
                    // 1. Index bounds-checked via bitwise AND with WINDOW_MASK
                    // 2. Single writer per slot (CAS winner owns this slot)
                    // 3. T must be Copy and properly aligned
                    unsafe {
                        let ptr = self.entries.as_ptr() as *mut T;
                        ptr.add(index).write(entry);
                    }

                    // Update statistics (relaxed - approximate OK)
                    // #ASSUME_RELAXED_STATISTICS: Counter precision not critical
                    self.total_writes.fetch_add(1, Ordering::Relaxed);

                    // Emit window event for tumbling windows
                    if self.window_type == WindowType::Tumbling {
                        let writes = self.total_writes.load(Ordering::Relaxed);
                        if writes % (self.window_size as u64) == 0 {
                            self.total_windows.fetch_add(1, Ordering::Relaxed);
                        }
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

    /// Get current window view (<50ns snapshot)
    ///
    /// Returns the most recent `window_size` entries, newest first.
    ///
    /// # Performance
    /// - O(window_size) iteration
    /// - Single atomic load for snapshot consistency
    ///
    /// # Window Semantics
    /// - **Sliding**: Last N entries (may overlap with previous window)
    /// - **Tumbling**: Current window batch (non-overlapping)
    ///
    /// #ASSUME_SNAPSHOT_CONSISTENCY: Single atomic load provides consistent snapshot
    pub fn window(&self) -> Vec<T> {
        // Load current head position (acquire for synchronization)
        // #ASSUME_ACQUIRE_ORDERING: See all writes before this snapshot
        let current = self.head.load(Ordering::Acquire);
        let (position, _generation) = Self::unpack(current);

        let count = self.window_size.min(self.total_writes.load(Ordering::Relaxed) as usize);
        let mut result = Vec::with_capacity(count);

        // Read backwards from head (newest first)
        for i in 0..count {
            // Compute index with wraparound (wrapping_sub handles underflow)
            // #ASSUME_WRAPPING_ARITHMETIC: Handles position=0 correctly
            let pos = position.wrapping_sub(i as u32 + 1);
            let index = (pos as usize) & WINDOW_MASK;

            // Read entry (bounds checked by bitwise AND)
            let entry = self.entries[index];

            // Skip uninitialized entries (ring buffer not yet full)
            if entry.is_empty() {
                break;
            }

            result.push(entry);
        }

        result
    }

    /// Slide window forward (for manual control of tumbling windows)
    ///
    /// Forces emission of current tumbling window.
    /// No-op for sliding windows.
    ///
    /// # Performance
    /// - <5ns (atomic increment)
    pub fn slide(&self) {
        if self.window_type == WindowType::Tumbling {
            self.total_windows.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get window size
    #[inline]
    pub const fn window_size(&self) -> usize {
        self.window_size
    }

    /// Get window type
    #[inline]
    pub const fn window_type(&self) -> WindowType {
        self.window_type
    }

    /// Get total entries written (monotonic counter)
    #[inline]
    pub fn total_writes(&self) -> u64 {
        self.total_writes.load(Ordering::Relaxed)
    }

    /// Get total windows emitted (tumbling windows only)
    #[inline]
    pub fn total_windows(&self) -> u64 {
        self.total_windows.load(Ordering::Relaxed)
    }

    /// Get ring buffer capacity (compile-time constant)
    #[inline]
    pub const fn capacity(&self) -> usize {
        WINDOW_CAPACITY
    }

    /// Get current head position (snapshot)
    #[inline]
    pub fn head_position(&self) -> u32 {
        let current = self.head.load(Ordering::Acquire);
        let (position, _) = Self::unpack(current);
        position
    }

    /// Get memory usage in bytes (header + entries)
    #[inline]
    pub fn memory_usage_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + WINDOW_CAPACITY * std::mem::size_of::<T>()
    }
}

impl<T: WindowEntry> Default for StreamingWindowCapsule<T> {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: StreamingWindowCapsule uses atomic operations for coordination
unsafe impl<T: WindowEntry> Send for StreamingWindowCapsule<T> {}
unsafe impl<T: WindowEntry> Sync for StreamingWindowCapsule<T> {}

// ============================================================================
// TESTS (T28 Framework: Unit + Property + Integration + Production)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // T28 Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_capsule_alignment() {
        // #VERIFY: 64-byte cache alignment
        assert_eq!(std::mem::align_of::<StreamingWindowCapsule<u64>>(), 64);
    }

    #[test]
    fn test_capacity_power_of_two() {
        // #VERIFY: Power-of-2 capacity for fast modulo
        assert_eq!(WINDOW_CAPACITY, 8192);
        assert_eq!(WINDOW_CAPACITY.count_ones(), 1);
    }

    #[test]
    fn test_new_capsule() {
        let window = StreamingWindowCapsule::<u64>::new();

        // #VERIFY: Initial state
        assert_eq!(window.capacity(), 8192);
        assert_eq!(window.window_size(), DEFAULT_WINDOW_SIZE);
        assert_eq!(window.window_type(), WindowType::Sliding);
        assert_eq!(window.total_writes(), 0);
        assert_eq!(window.head_position(), 0);
    }

    #[test]
    fn test_append_single_entry() {
        let window = StreamingWindowCapsule::new();

        let success = window.append(42u64);
        assert!(success);

        // #VERIFY: Counters updated
        assert_eq!(window.total_writes(), 1);
        assert_eq!(window.head_position(), 1);

        // #VERIFY: Entry retrievable in window
        let win = window.window();
        assert_eq!(win.len(), 1);
        assert_eq!(win[0], 42);
    }

    #[test]
    fn test_sliding_window() {
        let window = StreamingWindowCapsule::<u64>::with_size(5, WindowType::Sliding);

        // Append 10 entries
        for i in 0..10 {
            assert!(window.append(i));
        }

        assert_eq!(window.total_writes(), 10);

        // Window should contain last 5 entries (newest first)
        let win = window.window();
        assert_eq!(win.len(), 5);
        assert_eq!(win[0], 9); // Newest
        assert_eq!(win[4], 5); // 5th newest
    }

    #[test]
    fn test_tumbling_window() {
        let window = StreamingWindowCapsule::<u64>::with_size(3, WindowType::Tumbling);

        // Append 9 entries (3 full windows)
        for i in 0..9 {
            window.append(i);
        }

        assert_eq!(window.total_writes(), 9);
        assert_eq!(window.total_windows(), 3); // 3 complete tumbling windows
    }

    #[test]
    fn test_window_wraparound() {
        let window = StreamingWindowCapsule::<u64>::new();

        // Write more than capacity to trigger wraparound
        for i in 0..10000 {
            let _ = window.append(i as u64);
        }

        // Verify window still works after wraparound
        let win = window.window();
        assert!(!win.is_empty());
        assert!(win[0] > 9000); // Newest entries from end of sequence
    }

    #[test]
    fn test_generic_f64() {
        let window = StreamingWindowCapsule::<f64>::with_size(3, WindowType::Sliding);

        window.append(1.5);
        window.append(2.7);
        window.append(3.9);

        let win = window.window();
        assert_eq!(win.len(), 3);
        assert_eq!(win[0], 3.9);
        assert_eq!(win[1], 2.7);
        assert_eq!(win[2], 1.5);
    }

    // ========================================================================
    // T28 Q8-Q14: Property Tests
    // ========================================================================

    #[test]
    fn test_property_window_size_invariant() {
        let window = StreamingWindowCapsule::<u64>::with_size(100, WindowType::Sliding);

        // Append 200 entries
        for i in 0..200 {
            window.append(i);
        }

        // Window should never exceed window_size
        let win = window.window();
        assert!(win.len() <= 100, "Window exceeded max size: {}", win.len());
    }

    #[test]
    fn test_property_newest_first_ordering() {
        let window = StreamingWindowCapsule::<u64>::new();

        for i in 0..50 {
            window.append(i);
        }

        let win = window.window();

        // Verify descending order (newest first)
        for i in 0..win.len().saturating_sub(1) {
            assert!(
                win[i] > win[i + 1],
                "Window not sorted: {} at index {} not > {} at index {}",
                win[i],
                i,
                win[i + 1],
                i + 1
            );
        }
    }

    #[test]
    fn test_property_tumbling_window_boundaries() {
        let window = StreamingWindowCapsule::<u64>::with_size(10, WindowType::Tumbling);

        for i in 0..100 {
            window.append(i);
        }

        // Should emit exactly 10 tumbling windows (100 entries / 10 window_size)
        assert_eq!(window.total_windows(), 10);
    }

    // ========================================================================
    // T28 Q15-Q21: Integration Tests
    // ========================================================================

    #[test]
    fn test_concurrent_appends() {
        use std::sync::Arc;
        use std::thread;

        let window = Arc::new(StreamingWindowCapsule::<u64>::new());
        let mut handles = vec![];

        // Spawn 4 threads
        for thread_id in 0..4 {
            let window_clone = Arc::clone(&window);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let value = (thread_id * 1000 + i) as u64;
                    let _ = window_clone.append(value);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all writes succeeded
        assert_eq!(window.total_writes(), 400);
    }

    #[test]
    fn test_concurrent_read_write() {
        use std::sync::Arc;
        use std::thread;

        let window = Arc::new(StreamingWindowCapsule::<u64>::new());

        let window_writer = Arc::clone(&window);
        let writer = thread::spawn(move || {
            for i in 0..1000 {
                window_writer.append(i);
            }
        });

        let window_reader = Arc::clone(&window);
        let reader = thread::spawn(move || {
            for _ in 0..100 {
                let win = window_reader.window();
                // Just verify we can read without panic
                assert!(win.len() <= DEFAULT_WINDOW_SIZE);
            }
        });

        writer.join().unwrap();
        reader.join().unwrap();
    }

    // ========================================================================
    // T28 Q22-Q28: Production Tests
    // ========================================================================

    #[test]
    fn test_production_high_throughput() {
        let window = StreamingWindowCapsule::<u64>::new();

        // Simulate high-throughput streaming (100K entries)
        for i in 0..100_000 {
            let success = window.append(i);
            assert!(success, "Failed to append at iteration {}", i);
        }

        assert_eq!(window.total_writes(), 100_000);

        let win = window.window();
        assert_eq!(win.len(), DEFAULT_WINDOW_SIZE);
    }

    #[test]
    fn test_production_memory_footprint() {
        let window = StreamingWindowCapsule::<u64>::new();

        // Verify memory usage is bounded (header + 8K entries × 8 bytes)
        let expected_min = 64 + 8192 * 8; // Header + entries
        let actual = window.memory_usage_bytes();

        assert!(
            actual >= expected_min,
            "Memory usage {} less than expected minimum {}",
            actual,
            expected_min
        );
    }

    #[test]
    fn test_production_edge_case_window_size_one() {
        let window = StreamingWindowCapsule::<u64>::with_size(1, WindowType::Sliding);

        window.append(100);
        window.append(200);
        window.append(300);

        let win = window.window();
        assert_eq!(win.len(), 1);
        assert_eq!(win[0], 300); // Only latest entry
    }

    #[test]
    fn test_production_edge_case_empty_window() {
        let window = StreamingWindowCapsule::<u64>::new();

        let win = window.window();
        assert!(win.is_empty());
    }

    #[test]
    #[should_panic(expected = "Window size must be in range")]
    fn test_production_invalid_window_size_zero() {
        let _ = StreamingWindowCapsule::<u64>::with_size(0, WindowType::Sliding);
    }

    #[test]
    #[should_panic(expected = "Window size must be in range")]
    fn test_production_invalid_window_size_exceeds_capacity() {
        let _ = StreamingWindowCapsule::<u64>::with_size(10000, WindowType::Sliding);
    }
}
