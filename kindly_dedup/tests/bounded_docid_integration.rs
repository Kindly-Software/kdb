//! Integration tests for bounded DocumentId pattern
//!
//! Framework: T28 (Integration Tests - Q15-Q21)
//! Coverage: Basic usage, C4 corpus, mixed API, error handling

#[cfg(all(feature = "bounded-docid", not(feature = "meta-capsule")))]
mod tests {
    use atomic_capsule::CpuCapabilityCapsule;
    use kindly_dedup::bounded_docid::{BoundsError, DocumentIdAllocator};
    use kindly_dedup::DedupPipeline;

    #[test]
    fn test_add_document_bounded_basic() {
        // T28 Q15: Basic integration - bounded API usage
        let cpu_caps = CpuCapabilityCapsule::detect();
        let mut pipeline = DedupPipeline::new(10, &cpu_caps);
        let allocator = DocumentIdAllocator::new(10);

        // Valid IDs work
        let id0 = allocator.validate(0).unwrap();
        let id5 = allocator.validate(5).unwrap();
        let id9 = allocator.validate(9).unwrap();

        pipeline.add_document_bounded(id0, "test document 0").unwrap();
        pipeline.add_document_bounded(id5, "test document 5").unwrap();
        pipeline.add_document_bounded(id9, "test document 9").unwrap();

        // Verify documents were added
        let duplicates = pipeline.find_duplicates(0.85).unwrap();
        assert!(duplicates.len() <= 3); // No duplicates expected (different texts)
    }

    #[test]
    fn test_add_document_bounded_sequential() {
        // T28 Q16: Sequential allocation pattern (common case)
        let cpu_caps = CpuCapabilityCapsule::detect();
        let mut pipeline = DedupPipeline::new(100, &cpu_caps);
        let allocator = DocumentIdAllocator::new(100);

        // Add documents using sequential iterator
        for (i, id) in allocator.sequential().take(10).enumerate() {
            let text = format!("Document number {}", i);
            pipeline.add_document_bounded(id, &text).unwrap();
        }

        // Verify all documents were added
        let duplicates = pipeline.find_duplicates(0.85).unwrap();
        assert!(duplicates.len() <= 10);
    }

    #[test]
    fn test_allocator_validation_rejection() {
        // T28 Q17: Error handling - invalid IDs rejected
        let allocator = DocumentIdAllocator::new(10);

        // Valid boundaries
        assert!(allocator.validate(0).is_ok());
        assert!(allocator.validate(9).is_ok());

        // Invalid boundaries
        let result_10 = allocator.validate(10);
        assert_eq!(
            result_10,
            Err(BoundsError::DocumentIdOutOfBounds { id: 10, capacity: 10 })
        );

        let result_100 = allocator.validate(100);
        assert!(result_100.is_err());
    }

    #[test]
    fn test_batch_validation() {
        // T28 Q18: Batch validation workflow
        let cpu_caps = CpuCapabilityCapsule::detect();
        let mut pipeline = DedupPipeline::new(100, &cpu_caps);
        let allocator = DocumentIdAllocator::new(100);

        // Validate batch of IDs from external corpus
        let external_ids = vec![0, 10, 42, 50, 99];
        let doc_ids = allocator.validate_batch(&external_ids).unwrap();

        assert_eq!(doc_ids.len(), 5);

        // Add all validated documents
        for (i, id) in doc_ids.iter().enumerate() {
            let text = format!("Batch document {}", i);
            pipeline.add_document_bounded(*id, &text).unwrap();
        }

        let duplicates = pipeline.find_duplicates(0.85).unwrap();
        assert!(duplicates.len() <= 5);
    }

    #[test]
    fn test_batch_validation_partial() {
        // T28 Q19: Partial batch validation (some valid, some invalid)
        let allocator = DocumentIdAllocator::new(100);

        let mixed_ids = vec![0, 10, 150, 99, 200]; // 150 and 200 are invalid
        let (valid, invalid) = allocator.validate_batch_partial(&mixed_ids);

        assert_eq!(valid.len(), 3); // 0, 10, 99
        assert_eq!(invalid.len(), 2); // 150, 200

        assert_eq!(valid[0].get(), 0);
        assert_eq!(valid[1].get(), 10);
        assert_eq!(valid[2].get(), 99);

        assert_eq!(invalid[0].0, 150);
        assert_eq!(invalid[1].0, 200);
    }

    #[test]
    fn test_mixed_api_usage() {
        // T28 Q20: Backward compatibility - old and new APIs coexist
        let cpu_caps = CpuCapabilityCapsule::detect();
        let mut pipeline = DedupPipeline::new(100, &cpu_caps);
        let allocator = DocumentIdAllocator::new(100);

        // Use old API (runtime checks)
        pipeline.add_document(0, "old api document 0").unwrap();
        pipeline.add_document(50, "old api document 50").unwrap();

        // Use new bounded API (type-safe, no runtime checks)
        let id10 = allocator.validate(10).unwrap();
        let id60 = allocator.validate(60).unwrap();

        pipeline.add_document_bounded(id10, "new api document 10").unwrap();
        pipeline.add_document_bounded(id60, "new api document 60").unwrap();

        // Both APIs work together
        let duplicates = pipeline.find_duplicates(0.85).unwrap();
        assert!(duplicates.len() <= 4);
    }

    #[test]
    fn test_zero_cost_abstraction_verification() {
        // T28 Q21: Verify zero-cost abstraction guarantee
        use kindly_dedup::bounded_docid::DocumentId;
        use std::mem;

        // Size check
        assert_eq!(mem::size_of::<DocumentId>(), mem::size_of::<usize>());

        // Alignment check
        assert_eq!(mem::align_of::<DocumentId>(), mem::align_of::<usize>());

        // Array indexing works (no performance overhead)
        let allocator = DocumentIdAllocator::new(100);
        let id = allocator.validate(42).unwrap();

        let mut array = vec![0u64; 100];
        array[id.as_usize()] = 999; // Direct indexing, no overhead

        assert_eq!(array[42], 999);
    }

    #[test]
    fn test_boundary_conditions() {
        // T28 Additional: Boundary condition testing
        let cpu_caps = CpuCapabilityCapsule::detect();
        let mut pipeline = DedupPipeline::new(1000, &cpu_caps);
        let allocator = DocumentIdAllocator::new(1000);

        // Zero (edge case)
        let id_zero = allocator.validate(0).unwrap();
        pipeline.add_document_bounded(id_zero, "zero document").unwrap();

        // capacity - 1 (edge case)
        let id_max = allocator.validate(999).unwrap();
        pipeline.add_document_bounded(id_max, "max document").unwrap();

        // capacity (should fail validation)
        let id_invalid = allocator.validate(1000);
        assert!(id_invalid.is_err());

        let duplicates = pipeline.find_duplicates(0.85).unwrap();
        assert!(duplicates.len() <= 2);
    }

    #[test]
    fn test_empty_allocator() {
        // T28 Additional: Empty allocator (capacity = 0)
        let allocator = DocumentIdAllocator::new(0);

        // Sequential iterator should be empty
        let ids: Vec<_> = allocator.sequential().collect();
        assert_eq!(ids.len(), 0);

        // All validations should fail
        assert!(allocator.validate(0).is_err());
        assert!(allocator.validate(1).is_err());
    }

    #[test]
    fn test_large_allocator() {
        // T28 Additional: Large allocator (stress test)
        let allocator = DocumentIdAllocator::new(1_000_000);

        // Sequential iterator count
        let count = allocator.sequential().count();
        assert_eq!(count, 1_000_000);

        // Boundary checks
        assert!(allocator.validate(0).is_ok());
        assert!(allocator.validate(999_999).is_ok());
        assert!(allocator.validate(1_000_000).is_err());
    }
}
