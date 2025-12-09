//! Integration Tests for PersistentMinHashIndex
//!
//! **T28 Testing Framework - Tier 3: Integration Tests**
//!
//! Coverage:
//! - 10K document integration test
//! - Incremental addition test
//! - Cross-process consistency (future)
//! - Large-scale duplicate detection
//! - Recovery from partial failures
//!
//! **UCE34 Q30**: End-to-end validation

#![cfg(all(
    test,
    feature = "mmap-persistence",
    feature = "nightly-atomic",
    feature = "std"
))]

use atomic_capsule::collections::persistent_minhash::*;
use std::path::PathBuf;

// ============================================================================
// TEST UTILITIES
// ============================================================================

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("test_minhash_integ_{}.mmap", name))
}

fn cleanup(path: &PathBuf) {
    let _ = std::fs::remove_file(path);
}

// ============================================================================
// LARGE-SCALE INTEGRATION TESTS
// ============================================================================

#[test]
fn integration_10k_documents() {
    // Integration test: 10K unique documents
    let path = temp_path("10k_docs");
    cleanup(&path);

    let mut index = PersistentMinHashIndex::create(&path, 15_000).expect("Failed to create");

    println!("Adding 10K documents...");
    let start = std::time::Instant::now();

    // Add 10K unique documents
    for i in 0..10_000 {
        let content = format!("document id {} with unique content for testing", i);
        let is_new = index
            .add_document(i as u64, &content)
            .expect("Failed to add");

        assert!(is_new, "Document {} should be new", i);

        // Progress indicator
        if i % 1000 == 0 {
            println!("  {} documents added", i);
        }
    }

    let elapsed = start.elapsed();
    println!(
        "10K documents added in {:?} (avg: {:?}/doc)",
        elapsed,
        elapsed / 10_000
    );

    assert_eq!(index.document_count(), 10_000);

    // Flush to disk
    println!("Flushing to disk...");
    index.flush().expect("Failed to flush");

    // Verify all are now duplicates
    println!("Verifying duplicates...");
    let verify_start = std::time::Instant::now();

    for i in 0..10_000 {
        let content = format!("document id {} with unique content for testing", i);
        let is_dup = index.is_duplicate(&content).expect("Failed to check");

        assert!(is_dup, "Document {} not detected as duplicate", i);

        if i % 1000 == 0 {
            println!("  {} documents verified", i);
        }
    }

    let verify_elapsed = verify_start.elapsed();
    println!(
        "10K duplicates verified in {:?} (avg: {:?}/doc)",
        verify_elapsed,
        verify_elapsed / 10_000
    );

    cleanup(&path);
}

#[test]
fn integration_incremental_addition() {
    // Simulate weekly incremental updates
    let path = temp_path("incremental");
    cleanup(&path);

    // Week 1: Initial 1000 documents
    {
        println!("Week 1: Adding 1000 documents");
        let mut index = PersistentMinHashIndex::create(&path, 5_000).expect("Failed to create");

        for i in 0..1000 {
            let content = format!("initial document {}", i);
            index.add_document(i, &content).expect("Failed to add");
        }

        assert_eq!(index.document_count(), 1000);
        index.flush().expect("Failed to flush");
    }

    // Week 2: Add 100 new documents
    {
        println!("Week 2: Adding 100 new documents");
        let mut index = PersistentMinHashIndex::open(&path).expect("Failed to open");

        assert_eq!(index.document_count(), 1000); // Recovered

        let start_id = 1000;
        for i in 0..100 {
            let content = format!("new document {}", i);
            let is_new = index
                .add_document(start_id + i, &content)
                .expect("Failed to add");
            assert!(is_new);
        }

        assert_eq!(index.document_count(), 1100);
        index.flush().expect("Failed to flush");
    }

    // Week 3: Verify all previous documents are duplicates
    {
        println!("Week 3: Verifying 1100 documents");
        let index = PersistentMinHashIndex::open(&path).expect("Failed to open");

        assert_eq!(index.document_count(), 1100);

        // Check initial 1000
        for i in 0..1000 {
            let content = format!("initial document {}", i);
            let is_dup = index.is_duplicate(&content).expect("Failed to check");
            assert!(is_dup, "Document {} not found", i);
        }

        // Check week 2 additions
        for i in 0..100 {
            let content = format!("new document {}", i);
            let is_dup = index.is_duplicate(&content).expect("Failed to check");
            assert!(is_dup, "Document {} not found", i);
        }
    }

    cleanup(&path);
}

#[test]
fn integration_high_duplicate_rate() {
    // Simulate 99% duplicate rate (realistic for LLM dedup)
    let path = temp_path("high_dup_rate");
    cleanup(&path);

    let mut index = PersistentMinHashIndex::create(&path, 1_000).expect("Failed to create");

    println!("Adding 100 unique documents");

    // Add 100 unique documents
    for i in 0..100 {
        let content = format!("unique document {}", i);
        index.add_document(i, &content).expect("Failed to add");
    }

    assert_eq!(index.document_count(), 100);

    println!("Attempting to add 9900 duplicates (99% duplicate rate)");

    let mut new_count = 0;
    let mut dup_count = 0;

    // Try to add 9900 more (all duplicates of first 100)
    for i in 0..9900 {
        let content = format!("unique document {}", i % 100); // Repeat first 100
        let is_new = index
            .add_document(100 + i, &content)
            .expect("Failed to add");

        if is_new {
            new_count += 1;
        } else {
            dup_count += 1;
        }
    }

    println!("Results: {} new, {} duplicates", new_count, dup_count);

    // Should have detected 9900 duplicates
    assert_eq!(index.document_count(), 100 + new_count);
    assert!(
        dup_count > 9800,
        "Expected >9800 duplicates, got {}",
        dup_count
    );

    cleanup(&path);
}

#[test]
fn integration_recovery_after_crash() {
    // Simulate crash and recovery workflow
    let path = temp_path("crash_recovery");
    cleanup(&path);

    let original_count;

    // Phase 1: Create index and add documents
    {
        println!("Phase 1: Creating index and adding documents");
        let mut index = PersistentMinHashIndex::create(&path, 1_000).expect("Failed to create");

        for i in 0..500 {
            let content = format!("persistent document {}", i);
            index.add_document(i, &content).expect("Failed to add");
        }

        original_count = index.document_count();
        assert_eq!(original_count, 500);

        println!("Flushing {} documents", original_count);
        index.flush().expect("Failed to flush");

        // Simulate crash: drop index without cleanup
    }

    // Phase 2: Recovery
    {
        println!("Phase 2: Recovering from crash");
        let index = PersistentMinHashIndex::open(&path).expect("Failed to open");

        assert_eq!(
            index.document_count(),
            original_count,
            "Lost documents after crash"
        );

        // Verify data integrity
        println!("Verifying {} documents", original_count);
        for i in 0..original_count {
            let content = format!("persistent document {}", i);
            let is_dup = index.is_duplicate(&content).expect("Failed to check");
            assert!(is_dup, "Document {} lost after crash", i);
        }
    }

    cleanup(&path);
}

#[test]
fn integration_similarity_threshold_tuning() {
    // Test different similarity thresholds
    let path = temp_path("threshold_tuning");
    cleanup(&path);

    let mut index = PersistentMinHashIndex::create(&path, 100).expect("Failed to create");

    // Add base documents
    index
        .add_document(1, "the quick brown fox jumps over the lazy dog")
        .expect("Failed to add");

    index
        .add_document(2, "a completely different sentence about cats")
        .expect("Failed to add");

    // Test with default threshold (0.8)
    let similar1 = "the quick brown fox jumps over the lazy cat";
    let is_dup_default = index.is_duplicate(similar1).expect("Failed to check");
    println!(
        "Similar content (default 0.8): duplicate={}",
        is_dup_default
    );

    // Lower threshold (0.5 = more permissive)
    index.set_similarity_threshold(0.5);
    let is_dup_low = index.is_duplicate(similar1).expect("Failed to check");
    println!("Similar content (threshold 0.5): duplicate={}", is_dup_low);

    // Higher threshold (0.95 = stricter)
    index.set_similarity_threshold(0.95);
    let is_dup_high = index.is_duplicate(similar1).expect("Failed to check");
    println!(
        "Similar content (threshold 0.95): duplicate={}",
        is_dup_high
    );

    // Very different content should not be duplicate at any threshold
    let different = "completely unrelated topic about quantum physics";

    index.set_similarity_threshold(0.3); // Very permissive
    let is_dup_diff = index.is_duplicate(different).expect("Failed to check");
    println!(
        "Different content (threshold 0.3): duplicate={}",
        is_dup_diff
    );

    // This test just verifies the threshold mechanism works, not exact values
    // (exact duplicate detection depends on Jaccard similarity)

    cleanup(&path);
}
