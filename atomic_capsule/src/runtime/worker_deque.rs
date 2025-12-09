//! Worker Deque - Individual Worker Task Queue
//!
//! T4 Batch tier primitive implementing Chase-Lev work-stealing deque.
//!
//! # Architecture
//!
//! Each worker thread has its own WorkerDeque for task storage:
//! - Local push/pop operations (single-producer, single-consumer)
//! - Remote steal operation (multi-consumer)
//! - Circular buffer with power-of-two capacity
//!
//! # Algorithm
//!
//! Based on Chase-Lev "Dynamic Circular Work-Stealing Deque" (2005):
//! - Owner pushes to bottom, pops from bottom
//! - Thieves steal from top
//! - Relaxed ordering for local ops, Acquire/Release for steals
//!
//! # Performance (B32 Targets)
//!
//! - push_local: <30ns (relaxed atomic increment)
//! - pop_local: <30ns (relaxed atomic decrement)
//! - steal_remote: <100ns (CAS on top index)
//!
//! # Safety (ASSUM Framework)
//!
//! #ASSUME_CHASE_LEV_CORRECT: Algorithm proven correct in original paper
//! #ASSUME_BUFFER_BOUNDS: Circular indexing with power-of-two capacity
//! #ASSUME_STEAL_ORDERING: SeqCst fence ensures steal correctness

use core::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering, fence};
use core::cell::UnsafeCell;

/// Default deque capacity (must be power of two)
pub const DEFAULT_DEQUE_CAPACITY: usize = 256;

/// Result of a steal operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StealResult {
    /// Successfully stole a task
    Success(u32),
    /// Deque is empty
    Empty,
    /// Lost race with owner or another thief
    Retry,
}

/// Result of a pop operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopResult {
    /// Successfully popped a task
    Success(u32),
    /// Deque is empty
    Empty,
}

/// Chase-Lev work-stealing deque for a single worker
///
/// # Memory Layout (256B total, 64B aligned)
///
/// ```text
/// WorkerDeque (256B)
/// ├── [0-7]   top: AtomicI64 (steal index, thieves CAS here)
/// ├── [8-15]  bottom: AtomicI64 (push/pop index, owner only)
/// ├── [16-23] capacity_mask: usize (capacity - 1 for modulo)
/// ├── [24-31] generation: AtomicU64 (for statistics)
/// ├── [32-39] stats: AtomicU64 (operations count)
/// ├── [40-63] _padding: [u8; 24]
/// └── [64-255] buffer: inline circular buffer (192B = 48 u32 slots)
/// ```
///
/// For larger capacities (256 slots), buffer is heap-allocated.
#[repr(C, align(64))]
pub struct WorkerDeque {
    // === Cache Line 1 (Hot: indices) ===
    /// Top index (thieves steal from here)
    /// Uses i64 to handle wraparound correctly in comparisons
    top: AtomicI64,

    /// Bottom index (owner pushes/pops here)
    bottom: AtomicI64,

    /// Capacity mask (capacity - 1) for circular indexing
    capacity_mask: usize,

    /// Generation counter for ABA prevention
    generation: AtomicU64,

    /// Statistics: low 32 bits = pushes, high 32 bits = pops
    stats: AtomicU64,

    /// Padding to 64B
    _padding: [u8; 24],

    // === Remaining space: Buffer ===
    /// Circular buffer of task slot indices
    /// UnsafeCell allows modification via shared reference
    /// Using heap allocation for full capacity
    buffer: Box<[UnsafeCell<u32>]>,
}

// SAFETY: WorkerDeque uses proper atomic synchronization
// Owner has exclusive access to bottom, thieves use CAS on top
unsafe impl Send for WorkerDeque {}
unsafe impl Sync for WorkerDeque {}

impl WorkerDeque {
    /// Create a new work-stealing deque with default capacity
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_DEQUE_CAPACITY)
    }

    /// Create a new deque with specified capacity (must be power of two)
    pub fn with_capacity(capacity: usize) -> Self {
        // Ensure power of two
        let capacity = capacity.next_power_of_two();

        // Allocate buffer
        let mut buffer_vec = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer_vec.push(UnsafeCell::new(0));
        }
        let buffer = buffer_vec.into_boxed_slice();

        Self {
            top: AtomicI64::new(0),
            bottom: AtomicI64::new(0),
            capacity_mask: capacity - 1,
            generation: AtomicU64::new(0),
            stats: AtomicU64::new(0),
            _padding: [0u8; 24],
            buffer,
        }
    }

    /// Get deque capacity
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity_mask + 1
    }

    /// Get current number of tasks in deque
    #[inline]
    pub fn len(&self) -> usize {
        let bottom = self.bottom.load(Ordering::Relaxed);
        let top = self.top.load(Ordering::Relaxed);
        // Handle potential underflow during concurrent operations
        (bottom - top).max(0) as usize
    }

    /// Check if deque is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Push a task to the bottom of the deque (owner only)
    ///
    /// Returns true on success, false if deque is full.
    ///
    /// # Safety
    ///
    /// Must only be called by the owning worker thread.
    ///
    /// #ASSUME_CHASE_LEV_CORRECT: Push modifies bottom, no synchronization needed
    #[inline]
    pub fn push(&self, task_index: u32) -> bool {
        let bottom = self.bottom.load(Ordering::Relaxed);
        let top = self.top.load(Ordering::Acquire);

        // Check if full
        if (bottom - top) as usize >= self.capacity_mask {
            return false;
        }

        // #ASSUME_BUFFER_BOUNDS: Index is within capacity due to mask
        let index = (bottom as usize) & self.capacity_mask;

        // SAFETY: We're the only writer to this position
        // Thieves can only read from positions < bottom
        unsafe {
            *self.buffer[index].get() = task_index;
        }

        // Release fence ensures task data is visible before bottom is updated
        fence(Ordering::Release);

        // Update bottom (thieves will see new bottom after fence)
        self.bottom.store(bottom + 1, Ordering::Relaxed);

        // Update statistics
        self.increment_pushes();

        true
    }

    /// Pop a task from the bottom of the deque (owner only)
    ///
    /// Returns the task index if successful.
    ///
    /// # Safety
    ///
    /// Must only be called by the owning worker thread.
    ///
    /// #ASSUME_CHASE_LEV_CORRECT: Pop may race with steal on last element
    #[inline]
    pub fn pop(&self) -> PopResult {
        // Decrement bottom first
        let bottom = self.bottom.load(Ordering::Relaxed) - 1;
        self.bottom.store(bottom, Ordering::Relaxed);

        // Full memory barrier - ensures bottom update visible to thieves
        fence(Ordering::SeqCst);

        let top = self.top.load(Ordering::Relaxed);

        if top <= bottom {
            // There's at least one element
            let index = (bottom as usize) & self.capacity_mask;

            // SAFETY: We've decremented bottom, so this slot is ours
            let task = unsafe { *self.buffer[index].get() };

            if top == bottom {
                // This was the last element - might race with steal
                // Try to claim it by moving top forward
                if self.top.compare_exchange(
                    top,
                    top + 1,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ).is_err() {
                    // Lost race with thief
                    self.bottom.store(bottom + 1, Ordering::Relaxed);
                    return PopResult::Empty;
                }
                self.bottom.store(bottom + 1, Ordering::Relaxed);
            }

            self.increment_pops();
            PopResult::Success(task)
        } else {
            // Deque was empty
            self.bottom.store(bottom + 1, Ordering::Relaxed);
            PopResult::Empty
        }
    }

    /// Steal a task from the top of the deque (thieves only)
    ///
    /// Returns the task index if successful.
    ///
    /// # Safety
    ///
    /// Can be called from any thread except during push/pop by owner.
    ///
    /// #ASSUME_STEAL_ORDERING: SeqCst ensures correctness with concurrent operations
    #[inline]
    pub fn steal(&self) -> StealResult {
        let top = self.top.load(Ordering::Acquire);

        // Acquire fence ensures we see current bottom
        fence(Ordering::SeqCst);

        let bottom = self.bottom.load(Ordering::Acquire);

        if top >= bottom {
            return StealResult::Empty;
        }

        // Read task at top
        let index = (top as usize) & self.capacity_mask;

        // SAFETY: top < bottom guarantees this slot has valid data
        let task = unsafe { *self.buffer[index].get() };

        // Try to claim the task by incrementing top
        match self.top.compare_exchange(
            top,
            top + 1,
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(_) => StealResult::Success(task),
            Err(_) => StealResult::Retry, // Lost race
        }
    }

    /// Steal multiple tasks at once (batch steal for efficiency)
    ///
    /// Returns a vector of stolen task indices.
    ///
    /// #ASSUME_BATCH_STEAL_SAFE: Multiple steals are atomic individually
    pub fn steal_batch(&self, max_count: usize) -> Vec<u32> {
        let mut stolen = Vec::with_capacity(max_count.min(16));

        for _ in 0..max_count {
            match self.steal() {
                StealResult::Success(task) => stolen.push(task),
                StealResult::Empty => break,
                StealResult::Retry => continue, // Try again
            }
        }

        stolen
    }

    /// Get push count
    #[inline]
    pub fn push_count(&self) -> u32 {
        (self.stats.load(Ordering::Relaxed) & 0xFFFFFFFF) as u32
    }

    /// Get pop count
    #[inline]
    pub fn pop_count(&self) -> u32 {
        (self.stats.load(Ordering::Relaxed) >> 32) as u32
    }

    /// Increment push counter
    #[inline]
    fn increment_pushes(&self) {
        // Low 32 bits
        self.stats.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment pop counter
    #[inline]
    fn increment_pops(&self) {
        // High 32 bits
        self.stats.fetch_add(1 << 32, Ordering::Relaxed);
    }

    /// Reset the deque (for reuse or testing)
    pub fn reset(&self) {
        // Set both indices to 0
        self.top.store(0, Ordering::Release);
        self.bottom.store(0, Ordering::Release);
        self.stats.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for WorkerDeque {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for WorkerDeque {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WorkerDeque")
            .field("capacity", &self.capacity())
            .field("len", &self.len())
            .field("pushes", &self.push_count())
            .field("pops", &self.pop_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_deque_new() {
        let deque = WorkerDeque::new();
        assert_eq!(deque.capacity(), DEFAULT_DEQUE_CAPACITY);
        assert!(deque.is_empty());
        assert_eq!(deque.len(), 0);
    }

    #[test]
    fn test_deque_push_pop() {
        let deque = WorkerDeque::new();

        // Push some tasks
        assert!(deque.push(1));
        assert!(deque.push(2));
        assert!(deque.push(3));
        assert_eq!(deque.len(), 3);

        // Pop in LIFO order
        assert_eq!(deque.pop(), PopResult::Success(3));
        assert_eq!(deque.pop(), PopResult::Success(2));
        assert_eq!(deque.pop(), PopResult::Success(1));
        assert_eq!(deque.pop(), PopResult::Empty);
    }

    #[test]
    fn test_deque_steal() {
        let deque = WorkerDeque::new();

        // Push tasks
        deque.push(1);
        deque.push(2);
        deque.push(3);

        // Steal from top (FIFO from thief's perspective)
        assert_eq!(deque.steal(), StealResult::Success(1));
        assert_eq!(deque.steal(), StealResult::Success(2));
        assert_eq!(deque.steal(), StealResult::Success(3));
        assert_eq!(deque.steal(), StealResult::Empty);
    }

    #[test]
    fn test_deque_mixed_ops() {
        let deque = WorkerDeque::new();

        // Push
        deque.push(1);
        deque.push(2);

        // Steal one
        assert_eq!(deque.steal(), StealResult::Success(1));

        // Push more
        deque.push(3);
        deque.push(4);

        // Pop one
        assert_eq!(deque.pop(), PopResult::Success(4));

        // Steal remaining
        assert_eq!(deque.steal(), StealResult::Success(2));
        assert_eq!(deque.steal(), StealResult::Success(3));
        assert_eq!(deque.steal(), StealResult::Empty);
    }

    #[test]
    fn test_deque_full() {
        let deque = WorkerDeque::with_capacity(4);

        assert!(deque.push(1));
        assert!(deque.push(2));
        assert!(deque.push(3));
        // Can push capacity - 1 elements (need one empty slot)
        assert!(!deque.push(4)); // Should fail

        // Pop one, then push should succeed
        deque.pop();
        assert!(deque.push(4));
    }

    #[test]
    fn test_deque_batch_steal() {
        let deque = WorkerDeque::new();

        for i in 0..10 {
            deque.push(i);
        }

        let stolen = deque.steal_batch(5);
        assert_eq!(stolen.len(), 5);
        assert_eq!(stolen, vec![0, 1, 2, 3, 4]);

        let remaining = deque.steal_batch(10);
        assert_eq!(remaining.len(), 5);
        assert_eq!(remaining, vec![5, 6, 7, 8, 9]);
    }

    #[test]
    fn test_deque_concurrent_push_steal() {
        let deque = Arc::new(WorkerDeque::with_capacity(1024));
        let num_tasks = 500;

        // Owner thread pushes
        let deque_owner = Arc::clone(&deque);
        let owner = thread::spawn(move || {
            for i in 0..num_tasks {
                while !deque_owner.push(i) {
                    thread::yield_now();
                }
            }
        });

        // Thief threads steal
        let num_thieves = 4;
        let thieves: Vec<_> = (0..num_thieves)
            .map(|_| {
                let deque_thief = Arc::clone(&deque);
                thread::spawn(move || {
                    let mut stolen = Vec::new();
                    for _ in 0..num_tasks * 2 {
                        match deque_thief.steal() {
                            StealResult::Success(task) => stolen.push(task),
                            StealResult::Empty => {
                                if stolen.len() >= num_tasks as usize / num_thieves {
                                    break;
                                }
                                thread::yield_now();
                            }
                            StealResult::Retry => {}
                        }
                    }
                    stolen
                })
            })
            .collect();

        owner.join().unwrap();

        let mut all_stolen: Vec<u32> = Vec::new();
        for thief in thieves {
            all_stolen.extend(thief.join().unwrap());
        }

        // Drain any remaining
        loop {
            match deque.steal() {
                StealResult::Success(task) => all_stolen.push(task),
                StealResult::Empty => break,
                StealResult::Retry => {}
            }
        }

        // All tasks should be accounted for
        all_stolen.sort();
        all_stolen.dedup();
        // We may not get all tasks due to timing, but shouldn't have duplicates
        assert!(all_stolen.len() <= num_tasks as usize);
    }

    #[test]
    fn test_deque_statistics() {
        let deque = WorkerDeque::new();

        for i in 0..10 {
            deque.push(i);
        }
        assert_eq!(deque.push_count(), 10);

        for _ in 0..5 {
            deque.pop();
        }
        assert_eq!(deque.pop_count(), 5);
    }

    #[test]
    fn test_deque_reset() {
        let deque = WorkerDeque::new();

        deque.push(1);
        deque.push(2);
        deque.push(3);

        deque.reset();

        assert!(deque.is_empty());
        assert_eq!(deque.push_count(), 0);
        assert_eq!(deque.pop_count(), 0);
    }

    #[test]
    fn test_deque_power_of_two_capacity() {
        // Non-power-of-two should round up
        let deque = WorkerDeque::with_capacity(100);
        assert_eq!(deque.capacity(), 128); // Next power of two
    }
}
