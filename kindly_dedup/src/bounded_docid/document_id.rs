//! Type-safe bounded DocumentId newtype
//!
//! Framework: UCE34 Q10 (T0 Auditable), Q11 (Rust Transform), Q33 (Zero-cost abstraction)
//! Tier: T0 Auditable - Compile-time verification via type system
//!
//! # Design Principles
//!
//! 1. **Make Invalid States Unrepresentable**: Private field prevents external construction
//! 2. **Zero-Cost Abstraction**: repr(transparent) ensures sizeof(DocumentId) == sizeof(usize)
//! 3. **Module Privacy**: Only DocumentIdAllocator can construct DocumentId instances
//!
//! # Safety Invariants
//!
//! - #ASSUME_MODULE_PRIVACY: Only allocator module can construct DocumentId (enforced by private field)
//! - #VERIFY_ZERO_COST: repr(transparent) ensures zero runtime overhead (validated in tests)
//! - #ASSUME_CAPACITY_VALID: Allocator capacity >= all created DocumentId values (factory enforced)

use std::fmt;

/// Type-safe document identifier with compile-time bounds guarantee
///
/// # Invariant
/// A DocumentId can ONLY be constructed by DocumentIdAllocator::validate(),
/// which enforces id < capacity. Therefore, any DocumentId in scope is
/// guaranteed to be valid for indexing.
///
/// # Zero-Cost Abstraction
/// - Size: sizeof(DocumentId) == sizeof(usize) (verified in tests)
/// - Alignment: align_of(DocumentId) == align_of(usize) (verified in tests)
/// - Assembly: Identical to raw usize after optimization (see bounded-docid.xml)
///
/// # Example
/// ```
/// use kindly_dedup::bounded_docid::{DocumentIdAllocator, DocumentId};
///
/// let allocator = DocumentIdAllocator::new(100);
///
/// // Valid ID (< capacity)
/// let id = allocator.validate(42).expect("42 < 100");
/// assert_eq!(id.get(), 42);
///
/// // Invalid ID (>= capacity)
/// let result = allocator.validate(150);
/// assert!(result.is_err());
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct DocumentId(usize);

impl DocumentId {
    /// Construct DocumentId without validation (INTERNAL USE ONLY)
    ///
    /// # Safety
    /// This function is pub(super) to restrict access to the bounded_docid module.
    /// Only DocumentIdAllocator should call this after validation.
    ///
    /// # ASSUM Safety
    /// - #ASSUME_VALIDATED: Caller guarantees id < allocator.capacity
    pub(super) const fn new_unchecked(id: usize) -> Self {
        Self(id)
    }

    /// Get the raw usize value (guaranteed valid for indexing)
    ///
    /// # Safety Guarantee
    /// Because DocumentId can only be constructed via DocumentIdAllocator::validate(),
    /// this value is ALWAYS < the allocator's capacity. No runtime check needed.
    ///
    /// # Example
    /// ```
    /// use kindly_dedup::bounded_docid::DocumentIdAllocator;
    ///
    /// let allocator = DocumentIdAllocator::new(100);
    /// let id = allocator.validate(42).unwrap();
    ///
    /// // Safe indexing: id.get() < 100 guaranteed by type system
    /// let mut array = vec![0; 100];
    /// array[id.get()] = 99;
    /// ```
    #[inline(always)]
    pub const fn get(self) -> usize {
        self.0
    }

    /// Alias for get() (more ergonomic for array indexing)
    ///
    /// # Example
    /// ```
    /// use kindly_dedup::bounded_docid::DocumentIdAllocator;
    ///
    /// let allocator = DocumentIdAllocator::new(100);
    /// let id = allocator.validate(42).unwrap();
    ///
    /// let array = vec![0; 100];
    /// let value = array[id.as_usize()];  // More readable than array[id.get()]
    /// ```
    #[inline(always)]
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_document_id_size() {
        // Zero-cost abstraction: DocumentId must be same size as usize
        assert_eq!(mem::size_of::<DocumentId>(), mem::size_of::<usize>());
    }

    #[test]
    fn test_document_id_alignment() {
        // Zero-cost abstraction: DocumentId must have same alignment as usize
        assert_eq!(mem::align_of::<DocumentId>(), mem::align_of::<usize>());
    }

    #[test]
    fn test_document_id_get() {
        let id = DocumentId::new_unchecked(42);
        assert_eq!(id.get(), 42);
        assert_eq!(id.as_usize(), 42);
    }

    #[test]
    fn test_document_id_display() {
        let id = DocumentId::new_unchecked(42);
        assert_eq!(format!("{}", id), "42");
    }

    #[test]
    fn test_document_id_equality() {
        let id1 = DocumentId::new_unchecked(42);
        let id2 = DocumentId::new_unchecked(42);
        let id3 = DocumentId::new_unchecked(43);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_document_id_ordering() {
        let id1 = DocumentId::new_unchecked(10);
        let id2 = DocumentId::new_unchecked(20);
        let id3 = DocumentId::new_unchecked(30);

        assert!(id1 < id2);
        assert!(id2 < id3);
        assert!(id1 < id3);
    }

    #[test]
    fn test_document_id_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(DocumentId::new_unchecked(42));
        set.insert(DocumentId::new_unchecked(42)); // Duplicate
        set.insert(DocumentId::new_unchecked(43));

        assert_eq!(set.len(), 2); // Only 2 unique IDs
    }

    #[test]
    fn test_document_id_copy() {
        let id1 = DocumentId::new_unchecked(42);
        let id2 = id1; // Copy, not move
        assert_eq!(id1, id2);
        assert_eq!(id1.get(), 42); // id1 still valid (Copy trait worked)
    }

    #[test]
    fn test_document_id_boundary_values() {
        let zero = DocumentId::new_unchecked(0);
        let max = DocumentId::new_unchecked(usize::MAX);

        assert_eq!(zero.get(), 0);
        assert_eq!(max.get(), usize::MAX);
    }
}
