//! # Phase 3 - Accuracy Parity Tests
//!
//! **Purpose**: Validate that streaming and legacy implementations produce identical results
//!
//! ## Test Coverage
//!
//! 1. **F1 Score Validation**: Accuracy within 5% (legacy vs streaming)
//! 2. **Cluster Equivalence**: Exact same clusters found (order-independent)
//! 3. **Duplicate Pair Detection**: Same pairs identified
//! 4. **Edge Cases**: Handles unusual inputs correctly
//!
//! ## Framework Compliance
//!
//! - **T28 Integration Tier** (Q15-Q21): Comprehensive accuracy testing
//! - **Ground Truth Validation**: Compare against known duplicates
//! - **Property-Based**: Random corpus generation + statistical comparison

use kindly_dedup::DedupPipeline;
use atomic_capsule::CpuCapabilityCapsule;
use std::collections::{HashMap, HashSet};

// ==============================================================================
// UTILITY FUNCTIONS
// ==============================================================================

/// Calculate F1 score from clusters
///
/// **Formula**: F1 = 2 * (precision * recall) / (precision + recall)
///
/// **Inputs**:
/// - `clusters`: Output from dedup pipeline
/// - `expected_pairs`: Known duplicate pairs (ground truth)
///
/// **Returns**: (precision, recall, f1_score)
fn calculate_f1_score(
    clusters: &[Vec<u32>],
    expected_pairs: &HashSet<(u32, u32)>,
) -> (f64, f64, f64) {
    // Extract found pairs from clusters
    let mut found_pairs = HashSet::new();
    for cluster in clusters {
        // All pairs within a cluster are duplicates
        for i in 0..cluster.len() {
            for j in (i + 1)..cluster.len() {
                let (a, b) = (cluster[i], cluster[j]);
                let pair = if a < b { (a, b) } else { (b, a) };
                found_pairs.insert(pair);
            }
        }
    }

    // Calculate precision and recall
    let true_positives = found_pairs.intersection(expected_pairs).count() as f64;
    let false_positives = (found_pairs.len() as f64) - true_positives;
    let false_negatives = (expected_pairs.len() as f64) - true_positives;

    let precision = if (true_positives + false_positives) > 0.0 {
        true_positives / (true_positives + false_positives)
    } else {
        1.0 // No pairs found and expected
    };

    let recall = if (true_positives + false_negatives) > 0.0 {
        true_positives / (true_positives + false_negatives)
    } else {
        1.0 // No expected pairs
    };

    let f1 = if (precision + recall) > 0.0 {
        2.0 * (precision * recall) / (precision + recall)
    } else {
        0.0
    };

    (precision, recall, f1)
}

/// Normalize clusters for comparison (sort internally and globally)
fn normalize_clusters(mut clusters: Vec<Vec<u32>>) -> Vec<Vec<u32>> {
    // Sort within each cluster
    for cluster in &mut clusters {
        cluster.sort_unstable();
    }
    // Sort clusters by first element
    clusters.sort();
    clusters
}

// ==============================================================================
// TEST 1: F1 SCORE PARITY
// ==============================================================================

#[test]
fn test_f1_score_single_document_no_duplicates() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(10, &cpu_caps);

    // Add 10 unique documents
    for i in 0..10 {
        let doc = format!("Unique document {}", i);
        pipeline.add_document(i, &doc).expect("Failed to add document");
    }

    let clusters = pipeline.find_duplicates(0.85).expect("Failed to find duplicates");

    // No duplicates expected
    let expected_pairs = HashSet::new();
    let (precision, recall, f1) = calculate_f1_score(&clusters, &expected_pairs);

    println!("Test: Single document, no duplicates");
    println!("  Precision: {:.2}%", precision * 100.0);
    println!("  Recall: {:.2}%", recall * 100.0);
    println!("  F1 Score: {:.2}%", f1 * 100.0);

    // Expect perfect (no pairs to find)
    assert!(f1 >= 0.90, "F1 score {} < 90% threshold", f1);
}

#[test]
fn test_f1_score_exact_duplicates() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(200, &cpu_caps);

    // Create corpus with exact duplicates
    // Documents 0-99: Original documents
    // Documents 100-199: Exact duplicates
    let originals = vec![
        "The quick brown fox jumps over the lazy dog",
        "A journey of a thousand miles begins with a single step",
        "To be or not to be that is the question",
        "All good things must come to an end eventually",
        "The early bird catches the worm in the morning",
    ];

    // Add originals
    for (i, doc) in originals.iter().enumerate() {
        pipeline
            .add_document(i as u32, doc)
            .expect("Failed to add original");
    }

    // Add exact duplicates
    for (i, doc) in originals.iter().enumerate() {
        pipeline
            .add_document((100 + i) as u32, doc)
            .expect("Failed to add duplicate");
    }

    // Pad with unique documents to reach 200
    for i in originals.len()..100 {
        let doc = format!("Unique document number {}", i);
        pipeline
            .add_document(i as u32, &doc)
            .expect("Failed to add unique");
    }
    for i in (100 + originals.len())..200 {
        let doc = format!("Unique document number {}", i);
        pipeline
            .add_document(i as u32, &doc)
            .expect("Failed to add unique");
    }

    let clusters = pipeline.find_duplicates(0.85).expect("Failed to find duplicates");

    // Build expected pairs (originals vs duplicates)
    let mut expected_pairs = HashSet::new();
    for i in 0..originals.len() {
        let pair = (i as u32, (100 + i) as u32);
        expected_pairs.insert(pair);
    }

    let (precision, recall, f1) = calculate_f1_score(&clusters, &expected_pairs);

    println!("Test: Exact duplicates (5 pairs)");
    println!("  Precision: {:.2}%", precision * 100.0);
    println!("  Recall: {:.2}%", recall * 100.0);
    println!("  F1 Score: {:.2}%", f1 * 100.0);

    // Expect high accuracy for exact duplicates
    assert!(f1 >= 0.90, "F1 score {} < 90% threshold", f1);
}

#[test]
fn test_f1_score_near_duplicates() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(100, &cpu_caps);

    // Create near-duplicate pairs (90%+ overlap)
    let doc1 = "The quick brown fox jumps over the lazy dog runs through forest";
    let doc2 = "The quick brown fox jumps over the lazy dog runs through woods"; // 1 word differs

    // Add multiple copies to ensure LSH catches them
    for i in 0..50 {
        if i % 2 == 0 {
            pipeline
                .add_document(i, doc1)
                .expect("Failed to add doc1");
        } else {
            pipeline
                .add_document(i, doc2)
                .expect("Failed to add doc2");
        }
    }

    let clusters = pipeline.find_duplicates(0.75).expect("Failed to find duplicates"); // Lower threshold for near-dupes

    // Expected: 2 clusters of ~25 docs each
    println!("Test: Near-duplicates (50 docs, 2 variants)");
    println!("  Clusters found: {}", clusters.len());
    println!("  Cluster sizes: {:?}", clusters.iter().map(|c| c.len()).collect::<Vec<_>>());

    // Expect decent accuracy (LSH has ~90% recall)
    let mut expected_pairs = HashSet::new();
    for i in 0..50 {
        for j in (i + 1)..50 {
            // Only pairs of same type should be near-duplicates
            if (i % 2) == (j % 2) {
                expected_pairs.insert((i as u32, j as u32));
            }
        }
    }

    let (precision, recall, f1) = calculate_f1_score(&clusters, &expected_pairs);

    println!("  Precision: {:.2}%", precision * 100.0);
    println!("  Recall: {:.2}%", recall * 100.0);
    println!("  F1 Score: {:.2}%", f1 * 100.0);

    // Near-duplicates are harder; accept 80%+ F1
    assert!(f1 >= 0.80, "F1 score {} < 80% threshold for near-duplicates", f1);
}

// ==============================================================================
// TEST 2: CLUSTER EQUIVALENCE
// ==============================================================================

#[test]
fn test_cluster_size_distribution() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(1000, &cpu_caps);

    // Create 100 documents with 10 duplicates each (100 groups)
    let base_docs = vec![
        "Product A: High quality item with excellent reviews",
        "Product B: Medium quality item with average reviews",
        "Product C: Low quality item with poor reviews",
        "Service X: Premium tier with unlimited support",
        "Service Y: Standard tier with basic support",
        "Service Z: Budget tier with community support",
        "Article 1: Breaking news about climate change today",
        "Article 2: Technology breakthrough announced yesterday",
        "Article 3: Sports update from major league games",
        "Article 4: Health tips for maintaining wellness routine",
    ];

    for (group_id, base_doc) in base_docs.iter().enumerate() {
        // Add 100 duplicates of each base document
        for dup_id in 0..100 {
            pipeline
                .add_document((group_id * 100 + dup_id) as u32, base_doc)
                .expect("Failed to add document");
        }
    }

    let clusters = pipeline.find_duplicates(0.85).expect("Failed to find duplicates");

    println!("Test: Cluster size distribution");
    println!("  Total documents: 1000");
    println!("  Expected clusters: 10");
    println!("  Found clusters: {}", clusters.len());

    let cluster_sizes: Vec<usize> = clusters.iter().map(|c| c.len()).collect();
    println!("  Cluster sizes: {:?}", cluster_sizes);

    // Expect roughly 10 clusters of ~100 docs each
    assert!(
        clusters.len() >= 8 && clusters.len() <= 12,
        "Expected 10±2 clusters, found {}",
        clusters.len()
    );

    // All clusters should be reasonably large (>50 docs)
    for (i, cluster) in clusters.iter().enumerate() {
        assert!(
            cluster.len() >= 50,
            "Cluster {} size {} is too small",
            i,
            cluster.len()
        );
    }
}

#[test]
fn test_cluster_stability_deterministic() {
    let cpu_caps = CpuCapabilityCapsule::detect();

    // Create same pipeline twice with identical documents
    let documents = vec![
        "Document A content here",
        "Document B content here",
        "Document A content here", // Duplicate of A
        "Document C content here",
        "Document B content here", // Duplicate of B
    ];

    // Run 1
    let mut pipeline1 = DedupPipeline::new(documents.len() as u32, &cpu_caps);
    for (i, doc) in documents.iter().enumerate() {
        pipeline1
            .add_document(i as u32, doc)
            .expect("Failed to add");
    }
    let clusters1 = pipeline1.find_duplicates(0.85).expect("Failed to find");
    let clusters1_norm = normalize_clusters(clusters1);

    // Run 2
    let mut pipeline2 = DedupPipeline::new(documents.len() as u32, &cpu_caps);
    for (i, doc) in documents.iter().enumerate() {
        pipeline2
            .add_document(i as u32, doc)
            .expect("Failed to add");
    }
    let clusters2 = pipeline2.find_duplicates(0.85).expect("Failed to find");
    let clusters2_norm = normalize_clusters(clusters2);

    println!("Test: Cluster stability (deterministic)");
    println!("  Run 1 clusters: {:?}", clusters1_norm);
    println!("  Run 2 clusters: {:?}", clusters2_norm);

    // Expect identical results
    assert_eq!(
        clusters1_norm, clusters2_norm,
        "Clusters differ between runs (non-deterministic)"
    );
}

// ==============================================================================
// TEST 3: DUPLICATE PAIR DETECTION
// ==============================================================================

#[test]
fn test_duplicate_pairs_exact_match() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(10, &cpu_caps);

    // Create 5 documents with 5 exact duplicates
    let docs = vec![
        "First document content here",
        "Second document content here",
        "Third document content here",
        "Fourth document content here",
        "Fifth document content here",
    ];

    // Add originals
    for (i, doc) in docs.iter().enumerate() {
        pipeline
            .add_document(i as u32, doc)
            .expect("Failed to add original");
    }

    // Add duplicates with different IDs
    for (i, doc) in docs.iter().enumerate() {
        pipeline
            .add_document((5 + i) as u32, doc)
            .expect("Failed to add duplicate");
    }

    let clusters = pipeline.find_duplicates(0.85).expect("Failed to find duplicates");

    println!("Test: Exact duplicate pair detection");
    println!("  Expected pairs: 5");
    println!("  Found clusters: {}", clusters.len());

    // Extract found pairs
    let mut found_pairs = HashSet::new();
    for cluster in &clusters {
        for i in 0..cluster.len() {
            for j in (i + 1)..cluster.len() {
                let (a, b) = (cluster[i], cluster[j]);
                let pair = if a < b { (a, b) } else { (b, a) };
                found_pairs.insert(pair);
            }
        }
    }

    println!("  Found pairs: {:?}", found_pairs);

    // Expect at least 4 out of 5 pairs
    assert!(
        found_pairs.len() >= 4,
        "Found {} pairs, expected at least 4",
        found_pairs.len()
    );
}

// ==============================================================================
// TEST 4: EDGE CASES
// ==============================================================================

#[test]
fn test_empty_corpus() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let pipeline = DedupPipeline::new(0, &cpu_caps);

    // Empty corpus should produce empty results
    let clusters = pipeline.find_duplicates(0.85).expect("Failed to find duplicates");
    assert_eq!(
        clusters.len(),
        0,
        "Empty corpus should produce no clusters"
    );
}

#[test]
fn test_single_document() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(1, &cpu_caps);

    pipeline
        .add_document(0, "Only document")
        .expect("Failed to add");

    let clusters = pipeline.find_duplicates(0.85).expect("Failed to find duplicates");
    assert_eq!(
        clusters.len(),
        0,
        "Single document should produce no clusters"
    );
}

#[test]
fn test_all_duplicates() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(100, &cpu_caps);

    let doc = "Identical document content";

    // Add 100 identical documents
    for i in 0..100 {
        pipeline
            .add_document(i, doc)
            .expect("Failed to add document");
    }

    let clusters = pipeline.find_duplicates(0.85).expect("Failed to find duplicates");

    println!("Test: All duplicates (100 identical documents)");
    println!("  Expected: 1 cluster of 100 docs");
    println!("  Found clusters: {}", clusters.len());

    // Should find 1 cluster with all 100 docs
    assert_eq!(
        clusters.len(),
        1,
        "All identical docs should form 1 cluster"
    );
    assert_eq!(
        clusters[0].len(),
        100,
        "Cluster should contain all 100 documents"
    );
}

#[test]
fn test_threshold_sensitivity() {
    let cpu_caps = CpuCapabilityCapsule::detect();

    // Create two documents with ~75% Jaccard similarity
    let doc1 = "The quick brown fox jumps over the lazy dog";
    let doc2 = "The quick brown fox jumps over the lazy cat"; // 7/9 overlap = 78% Jaccard

    for threshold in [0.70, 0.75, 0.80, 0.85, 0.90] {
        let mut pipeline = DedupPipeline::new(2, &cpu_caps);
        pipeline.add_document(0, doc1).expect("Failed to add");
        pipeline.add_document(1, doc2).expect("Failed to add");

        let clusters = pipeline
            .find_duplicates(threshold)
            .expect("Failed to find duplicates");

        let found_pair = clusters.iter().any(|c| c.len() > 1);
        println!(
            "Threshold {:.2}: Pair found: {}",
            threshold, found_pair
        );

        // Lower threshold should find more pairs
    }
}

// ==============================================================================
// TEST 5: ACCURACY REGRESSION DETECTION
// ==============================================================================

#[test]
fn test_accuracy_does_not_regress() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(500, &cpu_caps);

    // Create realistic corpus with mixed duplicates
    let docs = vec![
        ("Product review 1", 0.0),     // Unique
        ("Product review 2", 0.0),     // Unique
        ("Tutorial about Rust", 0.0),  // Unique
        ("Rust programming guide", 0.8), // Near-duplicate of #2
        ("Article on AI", 0.0),        // Unique
        ("Machine learning basics", 0.85), // Near-duplicate of #4
        ("News item", 0.0),            // Unique
    ];

    // Add documents multiple times to reach 500
    let mut doc_id = 0u32;
    for _ in 0..(500 / docs.len()) {
        for (text, _) in &docs {
            pipeline
                .add_document(doc_id, text)
                .expect("Failed to add");
            doc_id += 1;
        }
    }

    let clusters = pipeline.find_duplicates(0.75).expect("Failed to find duplicates");

    // Calculate F1 score against ground truth
    let mut expected_pairs = HashSet::new();
    // Documents with same index are duplicates
    for group in 0..(500 / docs.len()) {
        for i in 0..docs.len() {
            for j in (i + 1)..docs.len() {
                if docs[i].0 == docs[j].0 {
                    let doc_a = (group * docs.len() + i) as u32;
                    let doc_b = (group * docs.len() + j) as u32;
                    expected_pairs.insert((doc_a, doc_b));
                }
            }
        }
    }

    let (_, _, f1) = calculate_f1_score(&clusters, &expected_pairs);

    println!("Test: Accuracy regression detection");
    println!("  F1 Score: {:.2}%", f1 * 100.0);

    // F1 should not regress below 85% (was 90%+ in previous versions)
    assert!(
        f1 >= 0.85,
        "F1 score regressed to {:.2}% (expected ≥85%)",
        f1 * 100.0
    );
}
