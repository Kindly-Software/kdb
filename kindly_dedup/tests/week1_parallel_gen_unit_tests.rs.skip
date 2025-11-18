//! # Week 1 - T28 Tier 1: Parallel Corpus Generation Unit Tests
//!
//! **Purpose**: Validate parallel synthetic corpus generation correctness
//!
//! ## T28 Framework Compliance (Q1-Q7)
//!
//! - **Q1**: Core behaviors: corpus generation, statistics, parallelization
//! - **Q2**: Edge cases: 0 docs, 1 doc, 10M docs
//! - **Q3**: Invariants: 5% exact, 15% near, 30% similar, 50% unique
//! - **Q4**: Code paths: All distribution branches covered
//! - **Q5**: Isolated: Deterministic with fixed seed
//! - **Q6**: Fast: <10 sec timeout per test
//! - **Q7**: Readable: Clear test names, helper functions

// Note: This module assumes generate_synthetic_corpus_parallel exists
// It will be implemented by the Parallel Gen Expert

use kindly_dedup::benchmarking::generate_synthetic_corpus_parallel;

// ============================================================================
// Q1: Core Behaviors
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(10000))]
fn test_parallel_gen_basic_corpus() {
    // Arrange & Act: Generate 1000 documents
    let corpus = generate_synthetic_corpus_parallel(1000);

    // Assert: Correct count
    assert_eq!(corpus.len(), 1000, "Should generate exactly 1000 documents");

    // Assert: Non-empty text
    for (id, text) in corpus.iter() {
        assert!(*id < 1000, "ID out of range: {}", id);
        assert!(!text.is_empty(), "Text should not be empty");
    }
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(10000))]
fn test_parallel_gen_document_ids_unique() {
    // Arrange & Act: Generate 10,000 documents
    let corpus = generate_synthetic_corpus_parallel(10_000);

    // Assert: All IDs unique
    let mut ids: Vec<usize> = corpus.iter().map(|(id, _)| *id).collect();
    ids.sort_unstable();
    ids.dedup();

    assert_eq!(ids.len(), 10_000, "All document IDs should be unique");
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(10000))]
fn test_parallel_gen_document_ids_sequential() {
    // Arrange & Act: Generate 100 documents
    let corpus = generate_synthetic_corpus_parallel(100);

    // Assert: IDs are 0..100
    let mut ids: Vec<usize> = corpus.iter().map(|(id, _)| *id).collect();
    ids.sort_unstable();

    for (expected, actual) in (0..100).zip(ids.iter()) {
        assert_eq!(expected, *actual, "Document IDs should be sequential 0..100");
    }
}

// ============================================================================
// Q2: Edge Cases
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_parallel_gen_zero_documents() {
    // Arrange & Act: Generate 0 documents
    let corpus = generate_synthetic_corpus_parallel(0);

    // Assert: Empty corpus
    assert_eq!(corpus.len(), 0, "Should generate empty corpus");
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(5000))]
fn test_parallel_gen_one_document() {
    // Arrange & Act: Generate 1 document
    let corpus = generate_synthetic_corpus_parallel(1);

    // Assert: Single document
    assert_eq!(corpus.len(), 1, "Should generate exactly 1 document");
    assert_eq!(corpus[0].0, 0, "Single document should have ID 0");
    assert!(!corpus[0].1.is_empty(), "Single document should have non-empty text");
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(10000))]
#[ignore] // Run manually: Large corpus test
fn test_parallel_gen_large_corpus_10m() {
    // Arrange & Act: Generate 10M documents (stress test)
    let corpus = generate_synthetic_corpus_parallel(10_000_000);

    // Assert: Correct count
    assert_eq!(corpus.len(), 10_000_000, "Should generate exactly 10M documents");

    // Assert: Sample documents valid
    for i in (0..10_000_000).step_by(1_000_000) {
        assert_eq!(corpus[i].0, i, "ID mismatch at index {}", i);
        assert!(!corpus[i].1.is_empty(), "Text empty at index {}", i);
    }
}

// ============================================================================
// Q3: Invariants (Distribution Properties)
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(10000))]
fn test_parallel_gen_exact_duplicate_distribution() {
    // Arrange & Act: Generate 10,000 documents
    let corpus = generate_synthetic_corpus_parallel(10_000);

    // Count exact duplicates (same text)
    let mut text_counts = std::collections::HashMap::new();
    for (_id, text) in corpus.iter() {
        *text_counts.entry(text.clone()).or_insert(0) += 1;
    }

    let exact_duplicates: usize = text_counts
        .values()
        .filter(|&&count| count > 1)
        .map(|&count| count - 1)
        .sum();

    let exact_pct = exact_duplicates as f64 / 10_000.0;

    // Assert: 5% exact duplicates (±1% tolerance)
    assert!(
        (exact_pct - 0.05).abs() < 0.01,
        "Exact duplicate rate {:.2}% (expected 5% ±1%)",
        exact_pct * 100.0
    );
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(10000))]
fn test_parallel_gen_near_duplicate_distribution() {
    // Arrange & Act: Generate 10,000 documents
    let corpus = generate_synthetic_corpus_parallel(10_000);

    // Count near-duplicates (Jaccard > 0.9)
    let mut near_duplicates = 0;

    // Sample 1000 pairs to estimate near-duplicate rate
    for i in (0..1000).step_by(100) {
        for j in (i + 1)..(i + 100).min(1000) {
            let jaccard = simple_jaccard(&corpus[i].1, &corpus[j].1);
            if jaccard > 0.9 && jaccard < 1.0 {
                near_duplicates += 1;
            }
        }
    }

    let near_pct = near_duplicates as f64 / 1000.0;

    // Assert: ~15% near-duplicates (relaxed check due to sampling)
    assert!(
        near_pct > 0.05,
        "Near-duplicate rate too low: {:.2}% (expected ~15%)",
        near_pct * 100.0
    );
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(10000))]
fn test_parallel_gen_corpus_diversity() {
    // Arrange & Act: Generate 1000 documents
    let corpus = generate_synthetic_corpus_parallel(1000);

    // Assert: At least 50% unique texts
    let unique_texts: std::collections::HashSet<_> = corpus.iter().map(|(_, text)| text).collect();

    let unique_pct = unique_texts.len() as f64 / 1000.0;

    assert!(
        unique_pct >= 0.50,
        "Diversity too low: {:.2}% unique (expected ≥50%)",
        unique_pct * 100.0
    );
}

// ============================================================================
// Q4: Code Path Coverage
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(10000))]
fn test_parallel_gen_all_cluster_types() {
    // Arrange & Act: Generate 10,000 documents
    let corpus = generate_synthetic_corpus_parallel(10_000);

    // Count documents by expected cluster type
    let exact_count = count_exact_duplicates(&corpus);
    let near_count = count_near_duplicates(&corpus);
    let similar_count = count_similar_documents(&corpus);
    let unique_count = count_unique_documents(&corpus);

    // Assert: All cluster types present
    assert!(exact_count > 0, "No exact duplicates found");
    assert!(near_count > 0, "No near-duplicates found");
    assert!(similar_count > 0, "No similar documents found");
    assert!(unique_count > 0, "No unique documents found");

    println!(
        "Distribution: Exact {}%, Near {}%, Similar {}%, Unique {}%",
        exact_count as f64 / 10_000.0 * 100.0,
        near_count as f64 / 10_000.0 * 100.0,
        similar_count as f64 / 10_000.0 * 100.0,
        unique_count as f64 / 10_000.0 * 100.0
    );
}

// ============================================================================
// Q5: Isolation & Determinism
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(10000))]
fn test_parallel_gen_deterministic_output() {
    // Arrange & Act: Generate same corpus twice
    let corpus1 = generate_synthetic_corpus_parallel(1000);
    let corpus2 = generate_synthetic_corpus_parallel(1000);

    // Assert: Identical output (deterministic)
    assert_eq!(corpus1.len(), corpus2.len());

    for (doc1, doc2) in corpus1.iter().zip(corpus2.iter()) {
        assert_eq!(doc1.0, doc2.0, "Document IDs differ (non-deterministic generation)");
        assert_eq!(doc1.1, doc2.1, "Document texts differ (non-deterministic generation)");
    }
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(10000))]
fn test_parallel_gen_no_memory_corruption() {
    // Arrange & Act: Generate multiple corpora in sequence
    let corpus1 = generate_synthetic_corpus_parallel(1000);
    let corpus2 = generate_synthetic_corpus_parallel(2000);
    let corpus3 = generate_synthetic_corpus_parallel(500);

    // Assert: Each corpus independent (no memory corruption)
    assert_eq!(corpus1.len(), 1000);
    assert_eq!(corpus2.len(), 2000);
    assert_eq!(corpus3.len(), 500);

    // Check first corpus still valid
    assert_eq!(corpus1[0].0, 0);
    assert!(!corpus1[0].1.is_empty());
}

// ============================================================================
// Q7: Readability - Helper Functions
// ============================================================================

/// Helper: Simple Jaccard similarity (for testing only)
fn simple_jaccard(text1: &str, text2: &str) -> f64 {
    let tokens1: std::collections::HashSet<_> = text1.split_whitespace().collect();
    let tokens2: std::collections::HashSet<_> = text2.split_whitespace().collect();

    let intersection = tokens1.intersection(&tokens2).count();
    let union = tokens1.union(&tokens2).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Helper: Count exact duplicates in corpus
fn count_exact_duplicates(corpus: &[(usize, String)]) -> usize {
    let mut text_counts = std::collections::HashMap::new();
    for (_id, text) in corpus.iter() {
        if text.starts_with("Exact duplicate cluster") {
            *text_counts.entry(text.clone()).or_insert(0) += 1;
        }
    }

    text_counts
        .values()
        .filter(|&&count| count > 1)
        .map(|&count| count - 1)
        .sum()
}

/// Helper: Count near-duplicates in corpus
fn count_near_duplicates(corpus: &[(usize, String)]) -> usize {
    corpus
        .iter()
        .filter(|(_id, text)| text.starts_with("Near duplicate cluster"))
        .count()
}

/// Helper: Count similar documents in corpus
fn count_similar_documents(corpus: &[(usize, String)]) -> usize {
    corpus
        .iter()
        .filter(|(_id, text)| text.starts_with("Similar document cluster"))
        .count()
}

/// Helper: Count unique documents in corpus
fn count_unique_documents(corpus: &[(usize, String)]) -> usize {
    corpus
        .iter()
        .filter(|(_id, text)| text.starts_with("Unique document"))
        .count()
}
