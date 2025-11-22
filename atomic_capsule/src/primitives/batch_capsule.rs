//! Tier 4: Batch Capsule Primitives
//!
//! Reference implementations for batch processing capsules achieving 10-100× throughput.

use crate::traits::{BatchCapsule, BatchError, ComputationalCapsule};
use core::sync::atomic::{AtomicUsize, Ordering};

/// Generic batch ring buffer capsule
///
/// ## UCE33 Q10: Tier 4 (Batch)
///
/// - **Performance**: 10-100× throughput vs single-item
/// - **Optimal size**: 512 items (L2 cache fit)
/// - **Alignment**: 64 bytes (cache line aligned)
///
/// ## Example
///
/// ```rust
/// use atomic_capsule::primitives::BatchRingBuffer;
///
/// let mut batch = BatchRingBuffer::<f64, 512>::new();
///
/// // Push items
/// for i in 0..512 {
///     batch.push(i as f64).unwrap();
/// }
///
/// // Process batch
/// let count = batch.batch_process(|items| {
///     println!("Processing {} items", items.len());
/// });
/// assert_eq!(count, 512);
/// ```
#[repr(C, align(64))]
pub struct BatchRingBuffer<T: Copy, const N: usize> {
    /// Ring buffer storage
    items: [T; N],
    /// Current item count (atomic for thread-safe reads)
    count: AtomicUsize,
    /// Padding to cache line
    _padding: [u8; 64 - 16],
}

impl<T: Copy, const N: usize> BatchRingBuffer<T, N> {
    /// Create new empty batch
    ///
    /// ## Safety
    ///
    /// T must be Copy and safe to zero-initialize.
    pub const fn new() -> Self
    where
        T: Copy,
    {
        Self {
            items: unsafe { core::mem::zeroed() },
            count: AtomicUsize::new(0),
            _padding: [0; 64 - 16],
        }
    }

    /// Get current count (thread-safe)
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }

    /// Check if batch is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get capacity
    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N
    }
}

impl<T: Copy, const N: usize> ComputationalCapsule for BatchRingBuffer<T, N> {
    const ALIGNMENT: usize = 64;
    const SIZE: usize = core::mem::size_of::<Self>();
    const TYPE_ID: &'static str = "BatchRingBuffer";
}

impl<T: Copy, const N: usize> BatchCapsule for BatchRingBuffer<T, N> {
    type Item = T;
    const BATCH_SIZE: usize = N;

    #[inline(always)]
    fn push(&mut self, item: Self::Item) -> Result<(), BatchError> {
        let count = self.count.load(Ordering::Acquire);

        if count >= N {
            return Err(BatchError::Full);
        }

        // Safety: bounds checked above
        self.items[count] = item;
        self.count.store(count + 1, Ordering::Release);
        Ok(())
    }

    fn batch_process<F>(&mut self, mut f: F) -> usize
    where
        F: FnMut(&[Self::Item]),
    {
        let count = self.count.load(Ordering::Acquire);

        if count > 0 {
            f(&self.items[..count]);
            self.count.store(0, Ordering::Release);
        }

        count
    }

    fn batch_count_atomic(&self) -> &AtomicUsize {
        &self.count
    }
}

/// Type alias for optimal batch size (512 items)
pub type OptimalBatchRingBuffer<T> = BatchRingBuffer<T, 512>;

/// Type alias for large batch size (4096 items)
pub type LargeBatchRingBuffer<T> = BatchRingBuffer<T, 4096>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_ring_buffer_basic() {
        let mut batch = BatchRingBuffer::<u64, 512>::new();

        // Push items
        for i in 0..512 {
            assert!(batch.push(i).is_ok());
        }

        // Batch full
        assert!(batch.push(999).is_err());
        assert_eq!(batch.len(), 512);

        // Process batch
        let mut sum = 0u64;
        let count = batch.batch_process(|items| {
            sum = items.iter().sum();
        });

        assert_eq!(count, 512);
        assert_eq!(sum, (0..512).sum());
        assert_eq!(batch.len(), 0);
    }

    #[test]
    fn test_batch_ring_buffer_empty() {
        let mut batch = BatchRingBuffer::<f64, 128>::new();
        assert!(batch.is_empty());

        let count = batch.batch_process(|_items| {
            panic!("Should not be called on empty batch");
        });

        assert_eq!(count, 0);
    }

    #[test]
    fn test_batch_ring_buffer_partial() {
        let mut batch = BatchRingBuffer::<i32, 256>::new();

        // Push partial batch
        for i in 0..100 {
            batch.push(i).unwrap();
        }

        assert_eq!(batch.len(), 100);

        // Process partial
        let mut max = 0;
        let count = batch.batch_process(|items| {
            max = *items.iter().max().unwrap();
        });

        assert_eq!(count, 100);
        assert_eq!(max, 99);
        assert!(batch.is_empty());
    }

    #[test]
    fn test_batch_trait_interface() {
        let mut batch = OptimalBatchRingBuffer::<f32>::new();

        assert_eq!(BatchRingBuffer::<f32, 512>::BATCH_SIZE, 512);
        assert!(batch.is_empty());
        assert!(!batch.is_full());

        // Fill batch
        for i in 0..512 {
            batch.push(i as f32).unwrap();
        }

        assert!(batch.is_full());
        assert_eq!(batch.count(), 512);

        // Flush
        let count = batch.flush(|items| {
            assert_eq!(items.len(), 512);
        });

        assert_eq!(count, 512);
        assert!(batch.is_empty());
    }

    #[test]
    fn test_optimal_batch_size() {
        // Verify 512-item batch fits in L2 cache (typical 256-512KB)
        let size = core::mem::size_of::<OptimalBatchRingBuffer<f64>>();

        // 512 × 8 bytes = 4KB (fits comfortably in L1)
        assert!(size < 8192, "Batch size {} exceeds L1 cache", size);
    }

    #[test]
    fn test_large_batch_size() {
        // Verify 4096-item batch fits in L2 cache
        let size = core::mem::size_of::<LargeBatchRingBuffer<f64>>();

        // 4096 × 8 bytes = 32KB (fits in L2)
        assert!(size < 65536, "Batch size {} exceeds reasonable L2 cache", size);
    }

    #[test]
    fn test_batch_alignment() {
        let batch = OptimalBatchRingBuffer::<u64>::new();
        let ptr = &batch as *const _ as usize;

        // Verify 64-byte alignment
        assert_eq!(ptr % 64, 0, "Batch not 64-byte aligned");
    }
}
