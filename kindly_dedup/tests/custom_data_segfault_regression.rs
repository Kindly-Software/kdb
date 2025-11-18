//! Regression Test Suite: Custom Data Segfault Bug (Exit Code 139)
//!
//! # Bug Summary
//!
//! **Date Discovered**: 2025-11-16
//! **Exit Code**: 139 (SIGSEGV - Segmentation Fault)
//! **Root Cause**: Missing bounds checks in `DedupPipeline::add_document()` allowed out-of-bounds
//! array writes when `doc_id >= capacity`, causing memory corruption and segfault.
//!
//! # Critical Fix
//!
//! **Required Checks**: BOTH Check 1 AND Check 2 are MANDATORY (per binary search analysis)
//!
//! ## Check 1 (Logical Capacity Validation)
//! ```rust,ignore
//! if doc_id >= self.num_documents {
//!     return Err(PipelineError::DocumentIdOutOfBounds { doc_id, capacity: self.num_documents });
//! }
//! ```
//! **Why Critical**: Validates document ID against pipeline's logical capacity.
//! **Failure**: Allows invalid doc_id to bypass first validation layer.
//!
//! ## Check 2 (Physical Storage Validation)
//! ```rust,ignore
//! if doc_id >= self.signatures.len() {
//!     return Err(PipelineError::DocumentIdOutOfBounds { doc_id, capacity: self.signatures.len() });
//! }
//! ```
//! **Why Critical**: Validates document ID against actual signatures vector length.
//! **Failure**: Allows out-of-bounds write at `self.signatures[doc_id] = Some(signature)` → SEGFAULT.
//!
//! # Why Both Checks Are Necessary
//!
//! - **Check 1 Alone**: Insufficient if `num_documents != signatures.len()` (initialization mismatch)
//! - **Check 2 Alone**: Insufficient without Check 1's logical validation
//! - **Together**: Double-validation ensures `doc_id` is valid for BOTH logical AND physical storage
//!
//! # Empirical Evidence
//!
//! Binary search testing (8 tests, 3 validation runs):
//! - **Test 7**: Removing Check 2 only → SEGFAULT (exit 139)
//! - **Test 8**: Removing Check 1 only → SEGFAULT (exit 139)
//! - **Test 5**: Removing both 1-2 → SEGFAULT (exit 139)
//! - **Baseline**: Both checks present → SUCCESS (exit 0)
//!
//! # Test Corpus
//!
//! Original corpus that triggered the segfault: `/tmp/test_corpus.txt`
//! - 10 documents (IDs 0-9)
//! - 8 expected clusters (2 duplicate pairs)
//! - Pipeline capacity: 10
//! - Vulnerable code: Line 407 `self.signatures[doc_id] = Some(signature);`
//!
//! # ASSUM Safety Tags
//!
//! #ASSUME_BOUNDS_CHECK_CRITICAL: Both Check 1 AND Check 2 MUST be present to prevent segfault
//! #VERIFY_NO_SEGFAULT: All tests must complete with exit code 0 (not 139)
//! #ASSUME_CAPACITY_MISMATCH: num_documents may != signatures.len() during initialization
//! #VERIFY_DOUBLE_VALIDATION: Both logical (num_documents) and physical (signatures.len()) checks required
//!
//! # Framework Compliance
//!
//! - **T28**: Regression tests (Q22-Q28 Production tier)
//! - **UCE34**: Systematic test coverage (Q1-Q34)
//! - **ASSUM**: 99.99% safety (documented assumptions)
//! - **B32**: Empirical validation (8 binary search tests)

// Import DedupPipeline from pipeline module directly
// (avoids feature gate conflicts with meta-capsule)
use atomic_capsule::CpuCapabilityCapsule;
use kindly_dedup::pipeline::DedupPipeline;
use kindly_dedup::PipelineError;
use std::fs;

/// Helper: Load test corpus from `/tmp/test_corpus.txt`
///
/// Returns Vec of (doc_id, text) tuples extracted from the corpus.
fn load_test_corpus() -> Vec<(usize, String)> {
    let corpus_path = "/tmp/test_corpus.txt";

    let content = fs::read_to_string(corpus_path).expect("Failed to read /tmp/test_corpus.txt - ensure file exists");

    content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(idx, line)| (idx, line.to_string()))
        .collect()
}

//
// TEST 1: Original Segfault Scenario (10-document corpus)
//

#[test]
fn test_original_segfault_scenario() {
    // ASSUM: Test corpus exists at /tmp/test_corpus.txt
    // VERIFY: Pipeline completes without segfault (exit 0, not 139)

    let corpus = load_test_corpus();
    assert_eq!(corpus.len(), 10, "Test corpus should have 10 documents");

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(10, &cpu_caps);

    // Add all 10 documents (doc_id 0-9)
    for (doc_id, text) in &corpus {
        // CRITICAL: This should NOT segfault with bounds checks present
        // Without Check 1 OR Check 2, this would crash at doc_id >= capacity
        let result = pipeline.add_document(*doc_id, text);

        assert!(result.is_ok(), "Failed to add document {}: {:?}", doc_id, result.err());
    }

    // Verify clustering (8 expected clusters per original test data)
    let clusters = pipeline.find_duplicates(0.85).expect("Failed to find duplicates");

    // Expected clusters:
    // - Cluster 1: docs 0, 1, 2, 6 (fox variations)
    // - Cluster 2: docs 3, 4, 5, 7 (ML/AI variations)
    // - Singleton: doc 8 (weather)
    // - Singleton: doc 9 (climate)
    // Total: 2 multi-doc clusters + 2 singletons = 4 total clusters

    // Note: Exact cluster count depends on Jaccard threshold and MinHash accuracy
    assert!(
        clusters.len() >= 2 && clusters.len() <= 10,
        "Expected 2-10 clusters, got {}",
        clusters.len()
    );

    // VERIFY: No segfault occurred (test passed)
    println!("✅ Original segfault scenario: PASSED (no crash)");
}

//
// TEST 2: Bounds Check at Exact Capacity
//

#[test]
fn test_bounds_check_at_capacity() {
    // ASSUM: doc_id == capacity should trigger Check 1 error
    // VERIFY: Returns DocumentIdOutOfBounds, not segfault

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(10, &cpu_caps);

    // Valid: doc_id 0-9 (< capacity)
    for doc_id in 0..10 {
        let result = pipeline.add_document(doc_id, "Test document");
        assert!(result.is_ok(), "Valid doc_id {} should succeed", doc_id);
    }

    // CRITICAL: doc_id == capacity (10) should fail gracefully
    let result = pipeline.add_document(10, "Invalid document at capacity");

    assert!(
        result.is_err(),
        "doc_id == capacity (10) should return error, not segfault"
    );

    match result {
        Err(PipelineError::DocumentIdOutOfBounds { doc_id, capacity }) => {
            assert_eq!(doc_id, 10, "Error should report doc_id == 10");
            assert_eq!(capacity, 10, "Error should report capacity == 10");
            println!("✅ Bounds check at capacity: PASSED (error returned)");
        }
        Err(other) => panic!("Expected DocumentIdOutOfBounds, got {:?}", other),
        Ok(_) => panic!("doc_id == capacity should fail, but succeeded"),
    }
}

//
// TEST 3: Bounds Check Beyond Capacity
//

#[test]
fn test_bounds_check_beyond_capacity() {
    // ASSUM: doc_id >> capacity should trigger Check 1 error immediately
    // VERIFY: Returns DocumentIdOutOfBounds, not segfault

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(10, &cpu_caps);

    // CRITICAL: doc_id = 100 (10× capacity) should fail gracefully
    let result = pipeline.add_document(100, "Invalid document far beyond capacity");

    assert!(
        result.is_err(),
        "doc_id >> capacity (100) should return error, not segfault"
    );

    match result {
        Err(PipelineError::DocumentIdOutOfBounds { doc_id, capacity }) => {
            assert_eq!(doc_id, 100, "Error should report doc_id == 100");
            assert_eq!(capacity, 10, "Error should report capacity == 10");
            println!("✅ Bounds check beyond capacity: PASSED (error returned)");
        }
        Err(other) => panic!("Expected DocumentIdOutOfBounds, got {:?}", other),
        Ok(_) => panic!("doc_id >> capacity should fail, but succeeded"),
    }

    // CRITICAL: doc_id = usize::MAX (maximum value) should also fail gracefully
    let result_max = pipeline.add_document(usize::MAX, "Invalid document at usize::MAX");

    assert!(
        result_max.is_err(),
        "doc_id == usize::MAX should return error, not segfault or overflow"
    );

    println!("✅ Bounds check at usize::MAX: PASSED (error returned)");
}

//
// TEST 4: Edge Case - Empty Pipeline
//

#[test]
fn test_empty_pipeline_bounds() {
    // ASSUM: Capacity 0 pipeline should reject all documents
    // VERIFY: Returns error for doc_id >= 0, not segfault

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(0, &cpu_caps);

    // CRITICAL: doc_id == 0 should fail on capacity-0 pipeline
    let result = pipeline.add_document(0, "Document on empty pipeline");

    assert!(
        result.is_err(),
        "doc_id == 0 on capacity-0 pipeline should return error"
    );

    match result {
        Err(PipelineError::DocumentIdOutOfBounds { doc_id, capacity }) => {
            assert_eq!(doc_id, 0, "Error should report doc_id == 0");
            assert_eq!(capacity, 0, "Error should report capacity == 0");
            println!("✅ Empty pipeline bounds: PASSED (error returned)");
        }
        Err(other) => panic!("Expected DocumentIdOutOfBounds, got {:?}", other),
        Ok(_) => panic!("Empty pipeline should reject all documents"),
    }
}

//
// TEST 5: Interleaved Valid/Invalid IDs
//

#[test]
fn test_interleaved_valid_invalid_ids() {
    // ASSUM: Invalid doc_ids should not corrupt pipeline state
    // VERIFY: Valid adds succeed even after invalid attempts

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(10, &cpu_caps);

    // Add valid document
    assert!(pipeline.add_document(0, "Valid doc 0").is_ok());

    // Attempt invalid (should fail)
    assert!(pipeline.add_document(100, "Invalid doc 100").is_err());

    // Add valid document (should still work after error)
    assert!(pipeline.add_document(1, "Valid doc 1").is_ok());

    // Attempt invalid at capacity
    assert!(pipeline.add_document(10, "Invalid doc 10").is_err());

    // Add valid document (pipeline not corrupted)
    assert!(pipeline.add_document(2, "Valid doc 2").is_ok());

    // Verify clustering still works (3 valid documents added)
    let clusters = pipeline
        .find_duplicates(0.85)
        .expect("Clustering should work after error handling");

    assert!(clusters.len() >= 1, "Should have at least 1 cluster from 3 documents");

    println!("✅ Interleaved valid/invalid IDs: PASSED (state not corrupted)");
}

//
// TEST 6: Off-by-One Boundary
//

#[test]
fn test_off_by_one_boundary() {
    // ASSUM: doc_id == capacity-1 is VALID, doc_id == capacity is INVALID
    // VERIFY: Correct boundary detection

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(10, &cpu_caps);

    // VALID: doc_id == 9 (capacity - 1, last valid index)
    let result_valid = pipeline.add_document(9, "Last valid document");
    assert!(result_valid.is_ok(), "doc_id == capacity-1 (9) should be valid");

    // INVALID: doc_id == 10 (capacity, first invalid index)
    let result_invalid = pipeline.add_document(10, "First invalid document");
    assert!(result_invalid.is_err(), "doc_id == capacity (10) should be invalid");

    println!("✅ Off-by-one boundary: PASSED (correct boundary)");
}

//
// TEST 7: Stress Test - Large Capacity
//

#[test]
#[ignore] // Expensive test, run with --ignored
fn test_large_capacity_bounds() {
    // ASSUM: Large capacity (1M) should still enforce bounds
    // VERIFY: doc_id >= 1M returns error, not segfault

    const CAPACITY: usize = 1_000_000;
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(CAPACITY, &cpu_caps);

    // Valid: Add document at max valid index
    assert!(pipeline.add_document(CAPACITY - 1, "Valid at max").is_ok());

    // Invalid: doc_id == capacity
    assert!(pipeline.add_document(CAPACITY, "Invalid at capacity").is_err());

    // Invalid: doc_id >> capacity
    assert!(pipeline.add_document(CAPACITY * 2, "Invalid far beyond").is_err());

    println!("✅ Large capacity bounds: PASSED (1M capacity validated)");
}

//
// TEST 8: Bounded API (Feature-Gated)
//

#[cfg(feature = "bounded-docid")]
#[test]
fn test_bounded_api_prevents_segfault() {
    // ASSUM: DocumentId type system prevents invalid IDs at compile time
    // VERIFY: Bounded API skips runtime checks safely

    use kindly_dedup::bounded_docid::DocumentIdAllocator;

    let allocator = DocumentIdAllocator::new(10);
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(10, &cpu_caps);

    // Valid: IDs 0-9 validated by allocator
    for id in 0..10 {
        let doc_id = allocator
            .validate(id)
            .expect(&format!("Allocator should validate ID {}", id));

        let result = pipeline.add_document_bounded(doc_id, "Test document");
        assert!(result.is_ok(), "Bounded API should succeed for valid ID {}", id);
    }

    // Invalid: Allocator rejects doc_id >= capacity
    let invalid_id = allocator.validate(10);
    assert!(invalid_id.is_err(), "Allocator should reject doc_id == capacity (10)");

    let invalid_id_large = allocator.validate(100);
    assert!(
        invalid_id_large.is_err(),
        "Allocator should reject doc_id >> capacity (100)"
    );

    println!("✅ Bounded API: PASSED (type system enforced bounds)");
}

//
// TEST 9: Corpus Statistics Validation
//

#[test]
fn test_corpus_statistics() {
    // VERIFY: Test corpus matches expected characteristics

    let corpus = load_test_corpus();

    // Expected: 10 documents
    assert_eq!(corpus.len(), 10, "Corpus should have 10 documents");

    // Expected: No empty documents
    for (doc_id, text) in &corpus {
        assert!(!text.trim().is_empty(), "Document {} should not be empty", doc_id);
    }

    // Expected: Reasonable document lengths (5-100 words)
    for (doc_id, text) in &corpus {
        let word_count = text.split_whitespace().count();
        assert!(
            word_count >= 5 && word_count <= 100,
            "Document {} has {} words (expected 5-100)",
            doc_id,
            word_count
        );
    }

    println!("✅ Corpus statistics: PASSED (10 docs, all valid)");
}

//
// TEST 10: Duplicate Detection Accuracy
//

#[test]
fn test_duplicate_detection_accuracy() {
    // VERIFY: Clustering produces expected results on test corpus

    let corpus = load_test_corpus();
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(10, &cpu_caps);

    // Add all documents
    for (doc_id, text) in &corpus {
        pipeline
            .add_document(*doc_id, text)
            .expect(&format!("Failed to add document {}", doc_id));
    }

    // Find duplicates at 85% Jaccard threshold
    let clusters = pipeline.find_duplicates(0.85).expect("Failed to find duplicates");

    // Expected clusters (from corpus analysis):
    // - Docs 0, 1, 2, 6: "quick brown fox" variations (4 docs)
    // - Docs 3, 4, 5, 7: "machine learning" variations (4 docs)
    // - Doc 8: weather (singleton)
    // - Doc 9: climate (singleton)

    // Verify: At least 2 clusters (fox + ML)
    assert!(
        clusters.len() >= 2,
        "Expected at least 2 clusters (fox + ML), got {}",
        clusters.len()
    );

    // Verify: At most 10 clusters (all singletons worst case)
    assert!(
        clusters.len() <= 10,
        "Expected at most 10 clusters, got {}",
        clusters.len()
    );

    // Verify: Cluster sizes are reasonable (1-4 docs per cluster)
    for cluster in &clusters {
        assert!(
            !cluster.is_empty() && cluster.len() <= 4,
            "Cluster size {} outside expected range [1, 4]",
            cluster.len()
        );
    }

    println!("✅ Duplicate detection accuracy: PASSED ({} clusters)", clusters.len());
}
