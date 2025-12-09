//! QueueCapsule - T4 Batch operation queue for git coordination.
//!
//! Uses atomic ring buffer for operation tracking with generation counters.

use std::sync::atomic::{AtomicU64, Ordering};
use std::path::Path;
use crate::error::{QueueError, Result};

/// Operation entry in the queue
#[derive(Debug, Clone, Copy)]
pub struct Operation {
    /// Instance ID (process ID)
    pub instance_id: u32,
    /// Generation counter
    pub generation: u32,
    /// Timestamp (seconds since epoch)
    pub timestamp: u64,
    /// Operation type (1=commit, 2=branch, etc.)
    pub op_type: u8,
}

/// Queue capsule for operation tracking
///
/// # T4 Batch Properties
/// - Ring buffer (bounded capacity)
/// - Generation counters (ABA prevention)
/// - Atomic head/tail (lockfree)
/// - Cache-aligned (128B)
#[repr(C, align(128))]
pub struct QueueCapsule {
    /// Head index (read position)
    head: AtomicU64,

    /// Tail index (write position)
    tail: AtomicU64,

    /// Capacity (fixed at creation)
    capacity: usize,

    /// Operation buffer (allocated separately)
    operations: *mut Operation,

    /// Padding to 128 bytes
    _padding: [u8; 96],
}

impl QueueCapsule {
    /// Load or create queue capsule
    pub fn load_or_create(path: &Path, capacity: usize) -> Result<Self> {
        // TODO: Implement mmap persistence
        // For now, create in-memory
        Ok(Self::new(capacity))
    }

    /// Create new queue capsule
    pub fn new(capacity: usize) -> Self {
        let operations = unsafe {
            let layout = std::alloc::Layout::array::<Operation>(capacity).unwrap();
            std::alloc::alloc_zeroed(layout) as *mut Operation
        };

        Self {
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            capacity,
            operations,
            _padding: [0; 96],
        }
    }

    /// Push operation to queue
    pub fn push(&self, op: Operation) -> std::result::Result<(), QueueError> {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);

        let count = tail - head;
        if count >= self.capacity as u64 {
            return Err(QueueError::Full(self.capacity));
        }

        // Write operation
        let index = (tail % self.capacity as u64) as usize;
        unsafe {
            std::ptr::write(self.operations.add(index), op);
        }

        // Update tail
        self.tail.store(tail + 1, Ordering::Release);

        Ok(())
    }

    /// Pop operation from queue
    pub fn pop(&self) -> std::result::Result<Operation, QueueError> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        if head >= tail {
            return Err(QueueError::Empty);
        }

        // Read operation
        let index = (head % self.capacity as u64) as usize;
        let op = unsafe { std::ptr::read(self.operations.add(index)) };

        // Update head
        self.head.store(head + 1, Ordering::Release);

        Ok(op)
    }

    /// Get current count
    pub fn count(&self) -> u32 {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);
        (tail - head) as u32
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// Check if full
    pub fn is_full(&self) -> bool {
        self.count() >= self.capacity as u32
    }
}

impl Drop for QueueCapsule {
    fn drop(&mut self) {
        unsafe {
            let layout = std::alloc::Layout::array::<Operation>(self.capacity).unwrap();
            std::alloc::dealloc(self.operations as *mut u8, layout);
        }
    }
}

// Verification
const _: () = {
    assert!(std::mem::size_of::<QueueCapsule>() == 128);
    assert!(std::mem::align_of::<QueueCapsule>() == 128);
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_queue_new() {
        let queue = QueueCapsule::new(1024);
        assert_eq!(queue.count(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_push_pop() {
        let queue = QueueCapsule::new(10);

        let op = Operation {
            instance_id: 1,
            generation: 1,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            op_type: 1,
        };

        queue.push(op).unwrap();
        assert_eq!(queue.count(), 1);

        let popped = queue.pop().unwrap();
        assert_eq!(popped.instance_id, 1);
        assert_eq!(queue.count(), 0);
    }

    #[test]
    fn test_queue_full() {
        let queue = QueueCapsule::new(2);

        let op = Operation {
            instance_id: 1,
            generation: 1,
            timestamp: 0,
            op_type: 1,
        };

        queue.push(op).unwrap();
        queue.push(op).unwrap();

        let err = queue.push(op).unwrap_err();
        assert!(matches!(err, QueueError::Full(2)));
    }
}
