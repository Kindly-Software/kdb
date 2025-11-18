//! DocumentId allocator factory with validation
//!
//! Framework: UCE34 Q10 (T0 Auditable), Q11 (Factory Pattern), Q33 (Validation)
//! Tier: T0 Auditable - Compile-time correctness via type system
//!
//! # Design Principles
//!
//! 1. **Single Source of Truth**: Only this allocator can create DocumentId instances
//! 2. **Validation Once**: Check bounds at load time, never at use time
//! 3. **Iterator Pattern**: Provide sequential ID generation for common case
//!
//! # Safety Invariants
//!
//! - #ASSUME_CAPACITY_VALID: Allocator capacity represents actual array size
//! - #VERIFY_BOUNDS_CHECK: validate() enforces id < capacity before construction
//! - #ASSUME_ITERATOR_EXHAUSTION: sequential() never produces id >= capacity

use super::document_id::DocumentId;
use super::error::BoundsError;

/// Factory for creating bounded DocumentIds with validation
///
/// # Invariant
/// All DocumentId instances created by this allocator satisfy `id < capacity`.
/// This invariant is enforced at construction and maintained throughout lifetime.
///
/// # Example
/// ```
/// use kindly_dedup::bounded_docid::DocumentIdAllocator;
///
/// // Create allocator for 100 documents
/// let allocator = DocumentIdAllocator::new(100);
///
/// // Validate individual IDs
/// let id0 = allocator.validate(0).unwrap();
/// let id99 = allocator.validate(99).unwrap();
/// let id100 = allocator.validate(100); // Error: out of bounds
/// assert!(id100.is_err());
///
/// // Generate sequential IDs
/// let ids: Vec<_> = allocator.sequential().take(10).collect();
/// assert_eq!(ids.len(), 10);
/// assert_eq!(ids[0].get(), 0);
/// assert_eq!(ids[9].get(), 9);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentIdAllocator {
    /// Maximum valid DocumentId is capacity - 1
    capacity: usize,
}

impl DocumentIdAllocator {
    /// Create new allocator with specified capacity
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of documents (valid IDs are 0..capacity)
    ///
    /// # Example
    /// ```
    /// use kindly_dedup::bounded_docid::DocumentIdAllocator;
    ///
    /// let allocator = DocumentIdAllocator::new(1000);
    /// assert_eq!(allocator.capacity(), 1000);
    /// ```
    #[inline]
    pub const fn new(capacity: usize) -> Self {
        Self { capacity }
    }

    /// Get the allocator's capacity
    ///
    /// # Returns
    /// Maximum valid DocumentId.get() is capacity - 1
    ///
    /// # Example
    /// ```
    /// use kindly_dedup::bounded_docid::DocumentIdAllocator;
    ///
    /// let allocator = DocumentIdAllocator::new(100);
    /// assert_eq!(allocator.capacity(), 100);
    /// ```
    #[inline]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Validate usize and construct DocumentId if within bounds
    ///
    /// # Arguments
    /// * `id` - Raw document ID to validate
    ///
    /// # Returns
    /// * `Ok(DocumentId)` if id < capacity
    /// * `Err(BoundsError)` if id >= capacity
    ///
    /// # Performance
    /// Single comparison (<1ns), amortized to zero after batch validation
    ///
    /// # Example
    /// ```
    /// use kindly_dedup::bounded_docid::{DocumentIdAllocator, BoundsError};
    ///
    /// let allocator = DocumentIdAllocator::new(100);
    ///
    /// // Valid IDs
    /// assert!(allocator.validate(0).is_ok());
    /// assert!(allocator.validate(99).is_ok());
    ///
    /// // Invalid IDs
    /// assert_eq!(
    ///     allocator.validate(100),
    ///     Err(BoundsError::DocumentIdOutOfBounds { id: 100, capacity: 100 })
    /// );
    /// assert!(allocator.validate(150).is_err());
    /// ```
    #[inline]
    pub const fn validate(&self, id: usize) -> Result<DocumentId, BoundsError> {
        if id >= self.capacity {
            return Err(BoundsError::DocumentIdOutOfBounds {
                id,
                capacity: self.capacity,
            });
        }
        Ok(DocumentId::new_unchecked(id))
    }

    /// Create sequential iterator over all valid DocumentIds (0..capacity)
    ///
    /// # Performance
    /// Zero-cost iterator (inlined, no allocations)
    ///
    /// # Example
    /// ```
    /// use kindly_dedup::bounded_docid::DocumentIdAllocator;
    ///
    /// let allocator = DocumentIdAllocator::new(5);
    ///
    /// let ids: Vec<_> = allocator.sequential().collect();
    /// assert_eq!(ids.len(), 5);
    /// assert_eq!(ids[0].get(), 0);
    /// assert_eq!(ids[4].get(), 4);
    /// ```
    #[inline]
    pub fn sequential(&self) -> impl Iterator<Item = DocumentId> + '_ {
        (0..self.capacity).map(DocumentId::new_unchecked)
    }

    /// Validate batch of IDs (faster than individual validate() calls)
    ///
    /// # Arguments
    /// * `ids` - Slice of raw document IDs to validate
    ///
    /// # Returns
    /// * `Ok(Vec<DocumentId>)` if ALL ids are valid
    /// * `Err(BoundsError)` if ANY id is invalid (short-circuits on first error)
    ///
    /// # Performance
    /// Same as sequential validate() calls (~1ns per ID), but more ergonomic
    ///
    /// # Example
    /// ```
    /// use kindly_dedup::bounded_docid::DocumentIdAllocator;
    ///
    /// let allocator = DocumentIdAllocator::new(100);
    ///
    /// // All valid
    /// let ids = allocator.validate_batch(&[0, 10, 50, 99]).unwrap();
    /// assert_eq!(ids.len(), 4);
    ///
    /// // One invalid (short-circuits)
    /// let result = allocator.validate_batch(&[0, 10, 150, 99]);
    /// assert!(result.is_err());
    /// ```
    #[inline]
    pub fn validate_batch(&self, ids: &[usize]) -> Result<Vec<DocumentId>, BoundsError> {
        ids.iter().map(|&id| self.validate(id)).collect()
    }

    /// Validate batch of IDs, returning which succeeded and which failed
    ///
    /// # Arguments
    /// * `ids` - Slice of raw document IDs to validate
    ///
    /// # Returns
    /// Tuple of (valid_ids, invalid_ids_with_errors)
    ///
    /// # Performance
    /// Same as validate_batch() but doesn't short-circuit (~1ns per ID)
    ///
    /// # Example
    /// ```
    /// use kindly_dedup::bounded_docid::DocumentIdAllocator;
    ///
    /// let allocator = DocumentIdAllocator::new(100);
    ///
    /// let ids = vec![0, 10, 150, 99, 200];
    /// let (valid, invalid) = allocator.validate_batch_partial(&ids);
    ///
    /// assert_eq!(valid.len(), 3);  // 0, 10, 99
    /// assert_eq!(invalid.len(), 2); // 150, 200
    /// assert_eq!(invalid[0].0, 150);
    /// assert_eq!(invalid[1].0, 200);
    /// ```
    pub fn validate_batch_partial(&self, ids: &[usize]) -> (Vec<DocumentId>, Vec<(usize, BoundsError)>) {
        let mut valid = Vec::new();
        let mut invalid = Vec::new();

        for &id in ids {
            match self.validate(id) {
                Ok(doc_id) => valid.push(doc_id),
                Err(e) => invalid.push((id, e)),
            }
        }

        (valid, invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocator_new() {
        let allocator = DocumentIdAllocator::new(100);
        assert_eq!(allocator.capacity(), 100);
    }

    #[test]
    fn test_allocator_validation_valid() {
        let allocator = DocumentIdAllocator::new(100);

        // Boundary cases
        assert!(allocator.validate(0).is_ok());
        assert!(allocator.validate(99).is_ok());

        // Middle values
        assert!(allocator.validate(50).is_ok());
    }

    #[test]
    fn test_allocator_validation_invalid() {
        let allocator = DocumentIdAllocator::new(100);

        // Exactly at capacity
        let result = allocator.validate(100);
        assert_eq!(
            result,
            Err(BoundsError::DocumentIdOutOfBounds { id: 100, capacity: 100 })
        );

        // Above capacity
        assert!(allocator.validate(150).is_err());
        assert!(allocator.validate(usize::MAX).is_err());
    }

    #[test]
    fn test_allocator_boundary_conditions() {
        let allocator = DocumentIdAllocator::new(100);

        // Zero (valid)
        assert_eq!(allocator.validate(0).unwrap().get(), 0);

        // capacity - 1 (valid)
        assert_eq!(allocator.validate(99).unwrap().get(), 99);

        // capacity (invalid)
        assert!(allocator.validate(100).is_err());

        // usize::MAX (invalid)
        assert!(allocator.validate(usize::MAX).is_err());
    }

    #[test]
    fn test_sequential_iterator() {
        let allocator = DocumentIdAllocator::new(5);

        let ids: Vec<_> = allocator.sequential().collect();
        assert_eq!(ids.len(), 5);

        for (i, id) in ids.iter().enumerate() {
            assert_eq!(id.get(), i);
        }
    }

    #[test]
    fn test_sequential_iterator_empty() {
        let allocator = DocumentIdAllocator::new(0);
        let ids: Vec<_> = allocator.sequential().collect();
        assert_eq!(ids.len(), 0);
    }

    #[test]
    fn test_sequential_iterator_large() {
        let allocator = DocumentIdAllocator::new(10_000);
        let count = allocator.sequential().count();
        assert_eq!(count, 10_000);
    }

    #[test]
    fn test_batch_validation_all_valid() {
        let allocator = DocumentIdAllocator::new(100);

        let ids = allocator.validate_batch(&[0, 10, 50, 99]).unwrap();
        assert_eq!(ids.len(), 4);
        assert_eq!(ids[0].get(), 0);
        assert_eq!(ids[1].get(), 10);
        assert_eq!(ids[2].get(), 50);
        assert_eq!(ids[3].get(), 99);
    }

    #[test]
    fn test_batch_validation_one_invalid() {
        let allocator = DocumentIdAllocator::new(100);

        // Should short-circuit on first error (150)
        let result = allocator.validate_batch(&[0, 10, 150, 99]);
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_validation_empty() {
        let allocator = DocumentIdAllocator::new(100);

        let ids = allocator.validate_batch(&[]).unwrap();
        assert_eq!(ids.len(), 0);
    }

    #[test]
    fn test_batch_partial_validation() {
        let allocator = DocumentIdAllocator::new(100);

        let ids = vec![0, 10, 150, 99, 200];
        let (valid, invalid) = allocator.validate_batch_partial(&ids);

        assert_eq!(valid.len(), 3);
        assert_eq!(valid[0].get(), 0);
        assert_eq!(valid[1].get(), 10);
        assert_eq!(valid[2].get(), 99);

        assert_eq!(invalid.len(), 2);
        assert_eq!(invalid[0].0, 150);
        assert_eq!(invalid[1].0, 200);
    }

    #[test]
    fn test_allocator_copy() {
        let allocator1 = DocumentIdAllocator::new(100);
        let allocator2 = allocator1; // Copy

        assert_eq!(allocator1.capacity(), allocator2.capacity());
        assert!(allocator1.validate(50).is_ok());
        assert!(allocator2.validate(50).is_ok());
    }

    #[test]
    fn test_allocator_equality() {
        let allocator1 = DocumentIdAllocator::new(100);
        let allocator2 = DocumentIdAllocator::new(100);
        let allocator3 = DocumentIdAllocator::new(200);

        assert_eq!(allocator1, allocator2);
        assert_ne!(allocator1, allocator3);
    }
}
