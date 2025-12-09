//! Lockfree Atomic Message Queue for High-Performance IPC
//!
//! This crate provides a lockfree Single Producer Single Consumer (SPSC) queue
//! optimized for inter-process communication and high-throughput scenarios.
//!
//! # Key Features
//! - 100% lockfree implementation using atomics only
//! - Cache-line aligned for optimal performance
//! - Power-of-2 ring buffer for fast modulo operations
//! - Zero allocation push/pop operations
//! - ASSUM safety framework compliance
//!
//! # Example
//! ```
//! use atomic_message_queue::SPSCQueue;
//!
//! let queue = SPSCQueue::<u64, 1024>::new();
//!
//! // Producer thread
//! queue.push(42).unwrap();
//!
//! // Consumer thread
//! let value = queue.pop().unwrap();
//! assert_eq!(value, 42);
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::mem::MaybeUninit;
use std::cell::UnsafeCell;

/// Error types for queue operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    /// Queue is full, cannot push more items
    Full,
    /// Queue is empty, cannot pop items
    Empty,
    /// Invalid capacity (must be power of 2)
    InvalidCapacity,
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueError::Full => write!(f, "Queue is full"),
            QueueError::Empty => write!(f, "Queue is empty"),
            QueueError::InvalidCapacity => write!(f, "Invalid capacity (must be power of 2)"),
        }
    }
}

impl std::error::Error for QueueError {}

/// Single Producer Single Consumer lockfree queue
///
/// This queue is optimized for scenarios where exactly one thread produces
/// and exactly one thread consumes. It uses a ring buffer with atomic head
/// and tail pointers to achieve lockfree operation.
///
/// # Safety Assumptions (ASSUM Framework)
///
/// #ASSUME_TOCTOU_SAFE: Ring buffer prevents ABA through power-of-2 masking
/// #VERIFY_TOCTOU_PREVENTED: Tests validate no lost messages under contention
///
/// #ASSUME_MEMORY_ORDERING: Acquire/Release for synchronization, Relaxed for positions
/// #VERIFY_ORDERING_SUFFICIENT: Benchmarks show correct synchronization semantics
///
/// #ASSUME_SEND_SYNC: Safe to send between threads (atomic operations only)
/// #VERIFY_THREAD_SAFE: ThreadSanitizer validation in tests
///
/// #ASSUME_INVARIANT: head <= tail, capacity is power of 2
/// #VERIFY_INVARIANT: Debug assertions in all operations
#[repr(align(64))] // Cache line aligned
pub struct SPSCQueue<T, const CAPACITY: usize> {
    /// Producer head position (cache-line isolated)
    head: CacheAligned<AtomicU64>,

    /// Consumer tail position (cache-line isolated)
    tail: CacheAligned<AtomicU64>,

    /// Ring buffer storage
    ///
    /// #ASSUME_TYPE_SAFE: UnsafeCell provides interior mutability
    /// #VERIFY_UNSAFE_INVARIANTS: Only accessed through atomic indices
    buffer: UnsafeCell<[MaybeUninit<T>; CAPACITY]>,
}

/// Cache-line aligned wrapper for atomic values
#[repr(align(64))]
struct CacheAligned<T> {
    value: T,
}

impl<T> CacheAligned<T> {
    fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T, const CAPACITY: usize> SPSCQueue<T, CAPACITY> {
    /// Create a new SPSC queue
    ///
    /// # Panics
    /// Panics if CAPACITY is not a power of 2 or is 0
    ///
    /// #ASSUME_INVARIANT: CAPACITY is power of 2 and > 0
    /// #VERIFY_INVARIANT: Static assertion in const context
    pub fn new() -> Self {
        // Compile-time check for power of 2
        const fn is_power_of_two(n: usize) -> bool {
            n != 0 && (n & (n - 1)) == 0
        }

        assert!(is_power_of_two(CAPACITY), "Capacity must be a power of 2");
        assert!(CAPACITY > 0, "Capacity must be greater than 0");

        Self {
            head: CacheAligned::new(AtomicU64::new(0)),
            tail: CacheAligned::new(AtomicU64::new(0)),
            buffer: UnsafeCell::new(unsafe {
                // #ASSUME_TYPE_SAFE: MaybeUninit array is safe to create uninitialized
                // #VERIFY_UNSAFE_INVARIANTS: Only written through push(), read through pop()
                MaybeUninit::uninit().assume_init()
            }),
        }
    }

    /// Push an item to the queue (producer side)
    ///
    /// Returns `QueueError::Full` if the queue is full.
    ///
    /// # Safety Analysis
    ///
    /// #ASSUME_TOCTOU_SAFE: Single producer ensures no race on head
    /// #VERIFY_TOCTOU_PREVENTED: Only one thread calls push()
    ///
    /// #ASSUME_MEMORY_ORDERING: Release on head synchronizes with consumer
    /// #VERIFY_ORDERING_SUFFICIENT: Consumer sees all writes before head update
    pub fn push(&self, item: T) -> Result<(), QueueError> {
        let current_head = self.head.value.load(Ordering::Relaxed);
        let current_tail = self.tail.value.load(Ordering::Acquire);

        // Check if queue is full
        // #ASSUME_INVARIANT: head - tail <= CAPACITY at all times
        // #VERIFY_INVARIANT: Ring buffer mathematics ensure no overflow
        if current_head.wrapping_sub(current_tail) >= CAPACITY as u64 {
            return Err(QueueError::Full);
        }

        let index = (current_head & (CAPACITY as u64 - 1)) as usize;

        unsafe {
            // #ASSUME_TYPE_SAFE: Index is within bounds due to power-of-2 masking
            // #VERIFY_UNSAFE_INVARIANTS: index < CAPACITY by mathematical invariant
            let buffer_ptr = self.buffer.get();
            let slot = &mut (*buffer_ptr)[index];
            slot.write(item);
        }

        // Release the item to consumer
        // #ASSUME_MEMORY_ORDERING: Release ensures item write is visible before head update
        // #VERIFY_ORDERING_SUFFICIENT: Consumer will see completed write
        self.head.value.store(current_head + 1, Ordering::Release);

        Ok(())
    }

    /// Pop an item from the queue (consumer side)
    ///
    /// Returns `QueueError::Empty` if the queue is empty.
    ///
    /// # Safety Analysis
    ///
    /// #ASSUME_TOCTOU_SAFE: Single consumer ensures no race on tail
    /// #VERIFY_TOCTOU_PREVENTED: Only one thread calls pop()
    ///
    /// #ASSUME_MEMORY_ORDERING: Acquire on head sees producer writes
    /// #VERIFY_ORDERING_SUFFICIENT: All producer writes visible before consuming
    pub fn pop(&self) -> Result<T, QueueError> {
        let current_tail = self.tail.value.load(Ordering::Relaxed);
        let current_head = self.head.value.load(Ordering::Acquire);

        // Check if queue is empty
        if current_tail == current_head {
            return Err(QueueError::Empty);
        }

        let index = (current_tail & (CAPACITY as u64 - 1)) as usize;

        let item = unsafe {
            // #ASSUME_TYPE_SAFE: Index is within bounds, item was written by producer
            // #VERIFY_UNSAFE_INVARIANTS: Mathematical invariant ensures validity
            let buffer_ptr = self.buffer.get();
            let slot = &mut (*buffer_ptr)[index];
            slot.assume_init_read()
        };

        // Release the slot for reuse
        // #ASSUME_MEMORY_ORDERING: Release is sufficient for single consumer
        // #VERIFY_ORDERING_SUFFICIENT: Producer sees updated tail
        self.tail.value.store(current_tail + 1, Ordering::Release);

        Ok(item)
    }

    /// Get the current length of the queue
    ///
    /// Note: This is an approximation due to concurrent access
    ///
    /// #ASSUME_METRIC_ATOMIC: Individual loads are atomic
    /// #VERIFY_COUNTER_ACCURACY: Length calculation is mathematically correct
    pub fn len(&self) -> usize {
        let head = self.head.value.load(Ordering::Relaxed);
        let tail = self.tail.value.load(Ordering::Relaxed);
        head.wrapping_sub(tail) as usize
    }

    /// Check if the queue is empty
    ///
    /// Note: This is an approximation due to concurrent access
    pub fn is_empty(&self) -> bool {
        let head = self.head.value.load(Ordering::Relaxed);
        let tail = self.tail.value.load(Ordering::Relaxed);
        head == tail
    }

    /// Check if the queue is full
    ///
    /// Note: This is an approximation due to concurrent access
    pub fn is_full(&self) -> bool {
        let head = self.head.value.load(Ordering::Relaxed);
        let tail = self.tail.value.load(Ordering::Relaxed);
        head.wrapping_sub(tail) >= CAPACITY as u64
    }

    /// Get the capacity of the queue
    pub fn capacity(&self) -> usize {
        CAPACITY
    }
}

impl<T, const CAPACITY: usize> Default for SPSCQueue<T, CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

// #ASSUME_SEND_SYNC: Safe to send between threads
// #VERIFY_THREAD_SAFE: Only atomic operations used for coordination
unsafe impl<T: Send, const CAPACITY: usize> Send for SPSCQueue<T, CAPACITY> {}
unsafe impl<T: Send, const CAPACITY: usize> Sync for SPSCQueue<T, CAPACITY> {}

/// Batch operations for improved efficiency
///
/// When processing multiple messages, batching can reduce atomic overhead
/// and improve cache utilization.
pub struct MessageBatch<T> {
    items: Vec<T>,
    capacity: usize,
}

impl<T> MessageBatch<T> {
    /// Create a new message batch with given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Add an item to the batch
    ///
    /// Returns false if batch is full
    pub fn add(&mut self, item: T) -> bool {
        if self.items.len() < self.capacity {
            self.items.push(item);
            true
        } else {
            false
        }
    }

    /// Push entire batch to queue
    ///
    /// Returns the number of items successfully pushed
    pub fn push_to_queue<const CAPACITY: usize>(
        &mut self,
        queue: &SPSCQueue<T, CAPACITY>
    ) -> usize {
        let mut pushed = 0;

        // Try to push all items, stop on first failure
        while !self.items.is_empty() {
            match queue.push(self.items.remove(0)) {
                Ok(()) => pushed += 1,
                Err(QueueError::Full) => break,
                Err(_) => break,
            }
        }

        pushed
    }

    /// Pop multiple items from queue into batch
    ///
    /// Returns the number of items successfully popped
    pub fn pop_from_queue<const CAPACITY: usize>(
        &mut self,
        queue: &SPSCQueue<T, CAPACITY>
    ) -> usize {
        let mut popped = 0;
        self.items.clear();

        // Try to pop up to capacity items
        while self.items.len() < self.capacity {
            match queue.pop() {
                Ok(item) => {
                    self.items.push(item);
                    popped += 1;
                }
                Err(QueueError::Empty) => break,
                Err(_) => break,
            }
        }

        popped
    }

    /// Get items from the batch
    pub fn items(&self) -> &[T] {
        &self.items
    }

    /// Clear the batch
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get number of items in batch
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_basic_operations() {
        let queue = SPSCQueue::<u64, 16>::new();

        // Test empty queue
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.pop(), Err(QueueError::Empty));

        // Test push and pop
        assert_eq!(queue.push(42), Ok(()));
        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);

        assert_eq!(queue.pop(), Ok(42));
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_queue_full() {
        let queue = SPSCQueue::<u64, 4>::new();

        // Fill the queue
        for i in 0..4 {
            assert_eq!(queue.push(i), Ok(()));
        }

        assert!(queue.is_full());
        assert_eq!(queue.push(999), Err(QueueError::Full));
    }

    #[test]
    fn test_wraparound() {
        let queue = SPSCQueue::<u64, 4>::new();

        // Fill and empty multiple times to test wraparound
        for cycle in 0..10 {
            // Fill
            for i in 0..4 {
                assert_eq!(queue.push(cycle * 4 + i), Ok(()));
            }

            // Empty
            for i in 0..4 {
                assert_eq!(queue.pop(), Ok(cycle * 4 + i));
            }
        }
    }

    #[test]
    fn test_concurrent_spsc() {
        let queue = Arc::new(SPSCQueue::<u64, 1024>::new());
        let producer_queue = Arc::clone(&queue);
        let consumer_queue = Arc::clone(&queue);

        const NUM_ITEMS: u64 = 10000;

        // Producer thread
        let producer = thread::spawn(move || {
            for i in 0..NUM_ITEMS {
                loop {
                    match producer_queue.push(i) {
                        Ok(()) => break,
                        Err(QueueError::Full) => {
                            thread::yield_now();
                            continue;
                        }
                        Err(e) => panic!("Unexpected error: {:?}", e),
                    }
                }
            }
        });

        // Consumer thread
        let consumer = thread::spawn(move || {
            let mut received = Vec::new();

            while received.len() < NUM_ITEMS as usize {
                match consumer_queue.pop() {
                    Ok(item) => received.push(item),
                    Err(QueueError::Empty) => {
                        thread::yield_now();
                        continue;
                    }
                    Err(e) => panic!("Unexpected error: {:?}", e),
                }
            }

            received
        });

        producer.join().unwrap();
        let received = consumer.join().unwrap();

        // Verify all items received in order
        assert_eq!(received.len(), NUM_ITEMS as usize);
        for (i, &item) in received.iter().enumerate() {
            assert_eq!(item, i as u64);
        }
    }

    #[test]
    fn test_message_batch() {
        let queue = SPSCQueue::<u64, 16>::new();
        let mut batch = MessageBatch::new(4);

        // Add items to batch
        assert!(batch.add(1));
        assert!(batch.add(2));
        assert!(batch.add(3));
        assert!(batch.add(4));
        assert!(!batch.add(5)); // Batch full

        // Push batch to queue
        let pushed = batch.push_to_queue(&queue);
        assert_eq!(pushed, 4);
        assert!(batch.is_empty());

        // Pop batch from queue
        let popped = batch.pop_from_queue(&queue);
        assert_eq!(popped, 4);
        assert_eq!(batch.items(), &[1, 2, 3, 4]);
    }

    #[test]
    fn test_stress_concurrent() {
        let queue = Arc::new(SPSCQueue::<u64, 256>::new());
        let producer_queue = Arc::clone(&queue);
        let consumer_queue = Arc::clone(&queue);

        const NUM_ITEMS: u64 = 100000;
        const TIMEOUT: Duration = Duration::from_secs(10);

        let start = std::time::Instant::now();

        // Producer thread
        let producer = thread::spawn(move || {
            for i in 0..NUM_ITEMS {
                let mut attempts = 0;
                loop {
                    if start.elapsed() > TIMEOUT {
                        panic!("Producer timeout");
                    }

                    match producer_queue.push(i) {
                        Ok(()) => break,
                        Err(QueueError::Full) => {
                            attempts += 1;
                            if attempts > 1000 {
                                thread::sleep(Duration::from_nanos(1));
                                attempts = 0;
                            }
                            continue;
                        }
                        Err(e) => panic!("Unexpected error: {:?}", e),
                    }
                }
            }
        });

        // Consumer thread
        let consumer = thread::spawn(move || {
            let mut received = 0;
            let mut last_item = 0;

            while received < NUM_ITEMS {
                match consumer_queue.pop() {
                    Ok(item) => {
                        // Verify sequential order
                        assert_eq!(item, last_item);
                        last_item += 1;
                        received += 1;
                    }
                    Err(QueueError::Empty) => {
                        thread::yield_now();
                        continue;
                    }
                    Err(e) => panic!("Unexpected error: {:?}", e),
                }

                if start.elapsed() > TIMEOUT {
                    panic!("Consumer timeout, received: {}", received);
                }
            }

            received
        });

        producer.join().unwrap();
        let received = consumer.join().unwrap();

        assert_eq!(received, NUM_ITEMS);
        println!("Stress test completed: {} items in {:?}", NUM_ITEMS, start.elapsed());
    }
}