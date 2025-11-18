//! Error types for bounded DocumentId validation
//!
//! Framework: UCE34 Q11 (Rust Transform), ASSUM 99.99% safe
//! Tier: T0 Auditable (compile-time type safety)

use thiserror::Error;

/// Errors that can occur during DocumentId validation
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum BoundsError {
    /// Document ID exceeds allocator capacity
    ///
    /// # Safety Invariant
    /// This error proves that DocumentId construction failed, preventing
    /// out-of-bounds indexing at compile time.
    ///
    /// # Example
    /// ```
    /// use kindly_dedup::bounded_docid::{DocumentIdAllocator, BoundsError};
    ///
    /// let allocator = DocumentIdAllocator::new(100);
    /// let result = allocator.validate(150);
    ///
    /// assert_eq!(
    ///     result,
    ///     Err(BoundsError::DocumentIdOutOfBounds { id: 150, capacity: 100 })
    /// );
    /// ```
    #[error("Document ID {id} out of bounds (capacity: {capacity})")]
    DocumentIdOutOfBounds {
        /// The invalid document ID that was rejected
        id: usize,
        /// The allocator's capacity (maximum valid ID is capacity - 1)
        capacity: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_error_display() {
        let error = BoundsError::DocumentIdOutOfBounds { id: 150, capacity: 100 };
        let display = format!("{}", error);
        assert_eq!(display, "Document ID 150 out of bounds (capacity: 100)");
    }

    #[test]
    fn test_bounds_error_equality() {
        let error1 = BoundsError::DocumentIdOutOfBounds { id: 150, capacity: 100 };
        let error2 = BoundsError::DocumentIdOutOfBounds { id: 150, capacity: 100 };
        let error3 = BoundsError::DocumentIdOutOfBounds { id: 151, capacity: 100 };

        assert_eq!(error1, error2);
        assert_ne!(error1, error3);
    }

    #[test]
    fn test_bounds_error_clone() {
        let error = BoundsError::DocumentIdOutOfBounds { id: 150, capacity: 100 };
        let cloned = error.clone();
        assert_eq!(error, cloned);
    }
}
