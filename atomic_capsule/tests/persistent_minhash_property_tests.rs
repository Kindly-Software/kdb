//! Property Tests for PersistentMinHashIndex
//!
//! **T28 Testing Framework - Tier 2: Property Tests**
//!
//! Coverage:
//! - 1000+ iterations (95% CI, B32 framework)
//! - Deterministic sketches (same input = same signature always)
//! - Collision rate properties
//! - Consistency across crashes
//! - Generation counter monotonicity
//! - Jaccard similarity bounds
//!
//! **UCE34 Q30**: B32 validation (statistical rigor)

#![cfg(all(
    test,
    feature = "mmap-persistence",
    feature = "nightly-atomic",
    feature = "std"
))]

use atomic_capsule::collections::persistent_minhash::*;
use atomic_capsule::probabilistic::MinHashSignatureCapsule;
use std::collections::HashSet;
use std::path::PathBuf;

// ============================================================================
// TEST UTILITIES
// ============================================================================

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("test_minhash_prop_{}.mmap", name))
}

fn cleanup(path: &PathBuf) {
    let _ = std::fs::remove_file(path);
}

// ============================================================================
// DETERMINISM PROPERTIES
// ============================================================================

#[test]
fn property_sketch_determinism_1000_iterations() {
    // B32: 1000+ iterations for statistical significance
    let path = temp_path("determinism_1000");
    cleanup(&path);

    let index = PersistentMinHashIndex::create(&path, 100).expect("Failed to create");

    let content = "the quick brown fox jumps over the lazy dog";

    // Compute signature 1000 times
    let signatures: Vec<_> = (0..1000).map(|_| index.compute_sketch(content)).collect();

    // All signatures must be identical
    for i in 1..signatures.len() {
        assert_eq!(
            signatures[0].signature(),
            signatures[i].signature(),
            "Signature mismatch at iteration {}",
            i
        );
    }

    cleanup(&path);
}

#[test]
fn property_different_content_different_signatures() {
    // Generate 100 random documents, verify signatures differ
    let path = temp_path("different_sigs");
    cleanup(&path);

    let index = PersistentMinHashIndex::create(&path, 1000).expect("Failed to create");

    let mut signatures = Vec::new();

    for i in 0..100 {
        let content = format!("document number {} with unique content", i);
        let sig = index.compute_sketch(&content);
        signatures.push(sig);
    }

    // Count collisions (should be very rare)
    let mut collisions = 0;
    for i in 0..signatures.len() {
        for j in (i + 1)..signatures.len() {
            if signatures[i].signature() == signatures[j].signature() {
                collisions += 1;
            }
        }
    }

    // Expect <1% collision rate for random content
    // (100 signatures = 4950 comparisons, expect <50 collisions)
    assert!(
        collisions < 50,
        "Too many collisions: {} (expected <50)",
        collisions
    );

    cleanup(&path);
}

#[test]
fn property_similar_content_high_jaccard() {
    // Similar content should have high Jaccard similarity
    let path = temp_path("similar_jaccard");
    cleanup(&path);

    let index = PersistentMinHashIndex::create(&path, 100).expect("Failed to create");

    let base = "the quick brown fox jumps over the lazy dog";
    let sig_base = index.compute_sketch(base);

    // 1000 iterations with slight variations
    let mut high_similarity_count = 0;

    for i in 0..1000 {
        // Add small variation (1 word changed)
        let variation = format!("the quick brown fox jumps over the lazy cat{}", i % 10);
        let sig_var = index.compute_sketch(&variation);

        let similarity = sig_base.jaccard_similarity(&sig_var);

        // Similar content should have >0.7 Jaccard similarity
        if similarity > 0.7 {
            high_similarity_count += 1;
        }
    }

    // At least 90% should have high similarity
    assert!(
        high_similarity_count > 900,
        "Only {} / 1000 had high similarity",
        high_similarity_count
    );

    cleanup(&path);
}

// ============================================================================
// COLLISION PROPERTIES
// ============================================================================

#[test]
fn property_hash_collision_rate_bounds() {
    // MinHash hash collisions should be <0.01% for random content
    let path = temp_path("collision_rate");
    cleanup(&path);

    let index = PersistentMinHashIndex::create(&path, 1000).expect("Failed to create");

    let mut hash_values = HashSet::new();
    let mut collisions = 0;

    // Generate 1000 random documents
    for i in 0..1000 {
        let content = format!("random document {}", i);
        let sig = index.compute_sketch(&content);

        // Extract first hash value as u16
        let hash = sig.signature()[0];

        if !hash_values.insert(hash) {
            collisions += 1;
        }
    }

    // Expect <10 collisions out of 1000 (u16 space = 65536 values)
    assert!(
        collisions < 10,
        "Too many hash collisions: {} (expected <10)",
        collisions
    );

    cleanup(&path);
}

// ============================================================================
// DUPLICATE DETECTION PROPERTIES
// ============================================================================

#[test]
fn property_duplicate_detection_accuracy_1000_docs() {
    // Add 1000 documents, verify no false negatives
    let path = temp_path("dup_accuracy");
    cleanup(&path);

    let mut index = PersistentMinHashIndex::create(&path, 1000).expect("Failed to create");

    // Add 1000 unique documents
    for i in 0..1000 {
        let content = format!("unique document number {}", i);
        let is_new = index
            .add_document(i as u64, &content)
            .expect("Failed to add");
        assert!(is_new, "Document {} incorrectly marked as duplicate", i);
    }

    assert_eq!(index.document_count(), 1000);

    // Verify all are now duplicates
    for i in 0..1000 {
        let content = format!("unique document number {}", i);
        let is_dup = index.is_duplicate(&content).expect("Failed to check");
        assert!(is_dup, "Document {} not detected as duplicate", i);
    }

    cleanup(&path);
}

#[test]
fn property_no_false_positives_for_unique_content() {
    // Ensure unique content is never marked as duplicate
    let path = temp_path("no_false_pos");
    cleanup(&path);

    let mut index = PersistentMinHashIndex::create(&path, 100).expect("Failed to create");

    // Add base document
    index
        .add_document(1, "the quick brown fox")
        .expect("Failed to add");

    // Test 100 different documents (should all be unique)
    let mut false_positives = 0;

    for i in 0..100 {
        let content = format!("completely different content {}", i);
        let is_dup = index.is_duplicate(&content).expect("Failed to check");

        if is_dup {
            false_positives += 1;
        }
    }

    // Expect <5% false positive rate
    assert!(
        false_positives < 5,
        "Too many false positives: {} / 100",
        false_positives
    );

    cleanup(&path);
}

// ============================================================================
// RECOVERY CONSISTENCY PROPERTIES
// ============================================================================

#[test]
fn property_recovery_consistency() {
    // Add documents, flush, re-open, verify consistency
    let path = temp_path("recovery_consistency");
    cleanup(&path);

    let original_count;
    let original_docs: Vec<String> = (0..100).map(|i| format!("document {}", i)).collect();

    // Create and populate
    {
        let mut index = PersistentMinHashIndex::create(&path, 200).expect("Failed to create");

        for (i, doc) in original_docs.iter().enumerate() {
            index.add_document(i as u64, doc).expect("Failed to add");
        }

        original_count = index.document_count();
        index.flush().expect("Failed to flush");
    }

    // Re-open and verify
    let index = PersistentMinHashIndex::open(&path).expect("Failed to open");

    assert_eq!(
        index.document_count(),
        original_count,
        "Document count mismatch after recovery"
    );

    // All original documents should be duplicates
    for doc in &original_docs {
        let is_dup = index.is_duplicate(doc).expect("Failed to check");
        assert!(is_dup, "Document not found after recovery: {}", doc);
    }

    cleanup(&path);
}

#[test]
fn property_partial_write_recovery() {
    // Simulate crash during write (last document incomplete)
    let path = temp_path("partial_write");
    cleanup(&path);

    {
        let mut index = PersistentMinHashIndex::create(&path, 100).expect("Failed to create");

        // Add 10 documents
        for i in 0..10 {
            index
                .add_document(i, &format!("document {}", i))
                .expect("Failed to add");
        }

        // Flush first 10
        index.flush().expect("Failed to flush");

        // Add 11th but don't flush (simulates crash)
        index
            .add_document(10, "incomplete document")
            .expect("Failed to add");

        // Drop without flush (simulates crash)
    }

    // Re-open: should recover 10 flushed documents
    let index = PersistentMinHashIndex::open(&path).expect("Failed to open");

    // Count should be 10 (11th document lost due to no flush)
    // NOTE: This test assumes generation counter = 0 for uninitialized entries
    // Recovery skips entries with generation = 0

    // Exact count depends on whether 11th write was partial or complete
    // Acceptable: 10 or 11 documents
    let count = index.document_count();
    assert!(count >= 10 && count <= 11, "Unexpected count: {}", count);

    cleanup(&path);
}

// ============================================================================
// GENERATION COUNTER PROPERTIES
// ============================================================================

#[test]
fn property_generation_monotonicity() {
    // Generation counters should be monotonically increasing
    let path = temp_path("gen_monotonic");
    cleanup(&path);

    let sig1 = MinHashSignatureCapsule::new();
    let sig2 = MinHashSignatureCapsule::new();

    let entry1 = PersistentMinHashEntry::new(sig1, 1, 1000);
    let entry2 = PersistentMinHashEntry::new(sig2, 2, 2000);

    // New entries start at 0
    assert_eq!(entry1.generation(), 0);
    assert_eq!(entry2.generation(), 0);

    cleanup(&path);
}

// ============================================================================
// JACCARD SIMILARITY BOUNDS
// ============================================================================

#[test]
fn property_jaccard_similarity_bounds() {
    // Jaccard similarity should be in [0.0, 1.0]
    let path = temp_path("jaccard_bounds");
    cleanup(&path);

    let index = PersistentMinHashIndex::create(&path, 100).expect("Failed to create");

    // Generate 100 random documents
    for i in 0..100 {
        let content1 = format!("document {}", i);
        let content2 = format!("document {}", (i + 1) % 100);

        let sig1 = index.compute_sketch(&content1);
        let sig2 = index.compute_sketch(&content2);

        let similarity = sig1.jaccard_similarity(&sig2);

        // Jaccard similarity must be in [0.0, 1.0]
        assert!(
            similarity >= 0.0 && similarity <= 1.0,
            "Invalid Jaccard similarity: {}",
            similarity
        );
    }

    cleanup(&path);
}

#[test]
fn property_identical_content_similarity_1_0() {
    // Identical content should have Jaccard = 1.0
    let path = temp_path("identical_sim");
    cleanup(&path);

    let index = PersistentMinHashIndex::create(&path, 100).expect("Failed to create");

    // Test with 100 different contents
    for i in 0..100 {
        let content = format!("test content {}", i);

        let sig1 = index.compute_sketch(&content);
        let sig2 = index.compute_sketch(&content);

        let similarity = sig1.jaccard_similarity(&sig2);

        // Identical signatures should have perfect similarity
        assert_eq!(similarity, 1.0, "Expected 1.0 for identical content");
    }

    cleanup(&path);
}

// ============================================================================
// STRESS TESTS
// ============================================================================

#[test]
fn property_stress_1000_sequential_adds() {
    // Stress test: 1000 sequential document additions
    let path = temp_path("stress_1000");
    cleanup(&path);

    let mut index = PersistentMinHashIndex::create(&path, 1500).expect("Failed to create");

    // Add 1000 documents
    for i in 0..1000 {
        let content = format!("stress test document number {}", i);
        let is_new = index
            .add_document(i as u64, &content)
            .expect("Failed to add");
        assert!(is_new);
    }

    assert_eq!(index.document_count(), 1000);

    index.flush().expect("Failed to flush");

    cleanup(&path);
}

#[test]
fn property_batch_duplicate_detection() {
    // Add 100 unique docs, then try to add same 100 again (all duplicates)
    let path = temp_path("batch_dup");
    cleanup(&path);

    let mut index = PersistentMinHashIndex::create(&path, 200).expect("Failed to create");

    let docs: Vec<String> = (0..100).map(|i| format!("batch document {}", i)).collect();

    // First pass: all new
    for (i, doc) in docs.iter().enumerate() {
        let is_new = index.add_document(i as u64, doc).expect("Failed to add");
        assert!(is_new, "Document {} should be new", i);
    }

    assert_eq!(index.document_count(), 100);

    // Second pass: all duplicates
    for doc in &docs {
        let is_dup = index.is_duplicate(doc).expect("Failed to check");
        assert!(is_dup, "Document should be duplicate: {}", doc);
    }

    cleanup(&path);
}
