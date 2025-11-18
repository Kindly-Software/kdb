//! # Week 1 - T28 Tier 1: Bloom Prefilter Unit Tests
//!
//! **Purpose**: Validate BloomFilterCapsule insert/query correctness
//!
//! ## T28 Framework Compliance (Q1-Q7)
//!
//! - **Q1**: Core behaviors: insert, query, FPR measurement
//! - **Q2**: Edge cases: 0 docs, max capacity, duplicate inserts
//! - **Q3**: Invariants: FPR < 1%, skip rate > 85% on duplicate-heavy
//! - **Q4**: Code paths: All insert/query branches covered
//! - **Q5**: Isolated: No shared state, deterministic
//! - **Q6**: Fast: <5 sec timeout per test
//! - **Q7**: Readable: Arrange-Act-Assert structure, clear names

use kindly_dedup::DedupBloomFilter;

// ============================================================================
// Q1: Core Behaviors
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_insert_and_query() {
    // Arrange: Create empty Bloom filter
    let mut bloom = DedupBloomFilter::new();

    // Act: Insert document
    bloom.insert(0, "test document");

    // Assert: Query returns true for inserted document
    assert!(
        bloom.query(0, "test document"),
        "Bloom filter should find inserted document"
    );
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_query_unseen_document() {
    // Arrange: Create empty Bloom filter
    let bloom = DedupBloomFilter::new();

    // Act & Assert: Query unseen document returns false
    assert!(
        !bloom.query(0, "unseen document"),
        "Bloom filter should not find unseen document"
    );
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_multiple_inserts() {
    // Arrange: Create Bloom filter
    let mut bloom = DedupBloomFilter::new();

    // Act: Insert multiple documents
    for i in 0..100 {
        bloom.insert(i, &format!("document {}", i));
    }

    // Assert: All documents queryable
    for i in 0..100 {
        assert!(
            bloom.query(i, &format!("document {}", i)),
            "Document {} should be found after insert",
            i
        );
    }
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_documents_seen_counter() {
    // Arrange: Create Bloom filter
    let mut bloom = DedupBloomFilter::new();

    // Act: Insert 50 documents
    for i in 0..50 {
        bloom.insert(i, &format!("document {}", i));
    }

    // Assert: Counter matches inserts
    assert_eq!(
        bloom.documents_seen(),
        50,
        "Documents seen counter should match inserts"
    );
}

// ============================================================================
// Q2: Edge Cases
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_empty_filter_query() {
    // Arrange: Empty Bloom filter
    let bloom = DedupBloomFilter::new();

    // Act & Assert: Query any document returns false
    assert!(!bloom.query(0, "any document"));
    assert!(!bloom.query(999, "another document"));
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_duplicate_insert() {
    // Arrange: Bloom filter with one document
    let mut bloom = DedupBloomFilter::new();
    bloom.insert(0, "test document");

    // Act: Insert same document again
    bloom.insert(0, "test document");

    // Assert: Still queryable, counter incremented
    assert!(bloom.query(0, "test document"));
    assert_eq!(
        bloom.documents_seen(),
        2,
        "Counter should increment on duplicate insert"
    );
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_empty_text() {
    // Arrange: Bloom filter
    let mut bloom = DedupBloomFilter::new();

    // Act: Insert document with empty text
    bloom.insert(0, "");

    // Assert: Empty text queryable
    assert!(bloom.query(0, ""), "Empty text should be queryable");
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_very_long_text() {
    // Arrange: Bloom filter with very long document
    let mut bloom = DedupBloomFilter::new();
    let long_text = "a".repeat(10_000);

    // Act: Insert long document
    bloom.insert(0, &long_text);

    // Assert: Long document queryable
    assert!(bloom.query(0, &long_text), "Very long document should be queryable");
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_unicode_text() {
    // Arrange: Bloom filter
    let mut bloom = DedupBloomFilter::new();

    // Act: Insert Unicode document
    let unicode_text = "Hello 世界 🌍 Привет";
    bloom.insert(0, unicode_text);

    // Assert: Unicode text queryable
    assert!(bloom.query(0, unicode_text), "Unicode text should be queryable");
}

// ============================================================================
// Q3: Invariants (Core Properties)
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_fpr_under_1_percent() {
    // Arrange: Bloom filter with 1000 documents
    let mut bloom = DedupBloomFilter::new();

    for i in 0..1000 {
        bloom.insert(i, &format!("document {}", i));
    }

    // Act: Query 10,000 unseen documents
    let mut false_positives = 0;
    for i in 1000..11000 {
        if bloom.query(i, &format!("unseen {}", i)) {
            false_positives += 1;
        }
    }

    // Assert: FPR < 1% (100 in 10,000)
    let fpr = false_positives as f64 / 10_000.0;
    assert!(fpr < 0.01, "FPR too high: {:.4}% (expected <1%)", fpr * 100.0);
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_skip_rate_duplicate_heavy() {
    // Arrange: Bloom filter with 100 unique documents
    let mut bloom = DedupBloomFilter::new();

    for i in 0..100 {
        bloom.insert(i, &format!("unique document {}", i));
    }

    // Act: Query 900 duplicates (90% duplicate corpus)
    let mut duplicates_found = 0;
    for i in 0..100 {
        for _copy in 0..9 {
            if bloom.query(i, &format!("unique document {}", i)) {
                duplicates_found += 1;
            }
        }
    }

    // Assert: Skip rate > 85% (accounting for FPR)
    let skip_rate = duplicates_found as f64 / 900.0;
    assert!(
        skip_rate > 0.85,
        "Skip rate too low: {:.2}% (expected >85%)",
        skip_rate * 100.0
    );
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_100_percent_recall_on_inserts() {
    // Arrange: Bloom filter
    let mut bloom = DedupBloomFilter::new();

    // Act: Insert 500 documents
    for i in 0..500 {
        bloom.insert(i, &format!("document {}", i));
    }

    // Assert: 100% recall (all inserts queryable)
    for i in 0..500 {
        assert!(
            bloom.query(i, &format!("document {}", i)),
            "Document {} not found (recall < 100%)",
            i
        );
    }
}

// ============================================================================
// Q4: Code Path Coverage
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_default_constructor() {
    // Test Default trait implementation
    let bloom = DedupBloomFilter::default();
    assert_eq!(bloom.documents_seen(), 0);
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_clone() {
    // Arrange: Bloom filter with data
    let mut bloom1 = DedupBloomFilter::new();
    bloom1.insert(0, "test");

    // Act: Clone
    let bloom2 = bloom1.clone();

    // Assert: Clone has same data
    assert!(bloom2.query(0, "test"));
    assert_eq!(bloom2.documents_seen(), bloom1.documents_seen());
}

// ============================================================================
// Q5: Isolation & Determinism
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_deterministic_hashing() {
    // Arrange: Two filters with same inserts
    let mut bloom1 = DedupBloomFilter::new();
    let mut bloom2 = DedupBloomFilter::new();

    // Act: Insert same documents
    for i in 0..100 {
        let text = format!("document {}", i);
        bloom1.insert(i, &text);
        bloom2.insert(i, &text);
    }

    // Assert: Same queries return same results
    for i in 0..100 {
        let text = format!("document {}", i);
        assert_eq!(
            bloom1.query(i, &text),
            bloom2.query(i, &text),
            "Determinism violated: different results for same input"
        );
    }
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_no_cross_contamination() {
    // Arrange: Two independent filters
    let mut bloom1 = DedupBloomFilter::new();
    let mut bloom2 = DedupBloomFilter::new();

    // Act: Insert different documents
    bloom1.insert(0, "filter1 document");
    bloom2.insert(1, "filter2 document");

    // Assert: No cross-contamination
    assert!(bloom1.query(0, "filter1 document"));
    assert!(!bloom1.query(1, "filter2 document"));

    assert!(bloom2.query(1, "filter2 document"));
    assert!(!bloom2.query(0, "filter1 document"));
}

// ============================================================================
// Q7: Readability - Helper Functions
// ============================================================================

/// Helper: Create filter with N documents
fn create_filter_with_documents(num_docs: usize) -> DedupBloomFilter {
    let mut bloom = DedupBloomFilter::new();
    for i in 0..num_docs {
        bloom.insert(i, &format!("document {}", i));
    }
    bloom
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_bloom_helper_usage() {
    // Using helper for clarity
    let bloom = create_filter_with_documents(100);
    assert_eq!(bloom.documents_seen(), 100);
    assert!(bloom.query(50, "document 50"));
}
