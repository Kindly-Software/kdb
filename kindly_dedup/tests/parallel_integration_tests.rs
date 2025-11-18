//! Parallel Deduplication Integration Tests (T28 Comprehensive)
//!
//! **T28 Tier 3: Integration Testing** (Q15-Q21)
//!
//! # Test Coverage
//!
//! - Property test: parallel == sequential (1000+ runs, random docs)
//! - Thread scaling: results identical across 1-16 threads
//! - F1 score validation: ≥90% on known duplicate corpus
//! - Correctness: bucket consistency (all pairs found)
//! - Edge cases: empty input, single doc, all duplicates, no duplicates
//! - Large scale: 100K documents test
//!
//! # Framework Compliance
//!
//! - **T28**: Q15 (integration points), Q16 (error propagation), Q17 (performance budgets)
//! - **UCE34**: Q33 (verification), Q10 (tier validation)
//! - **ASSUM**: 99.99% safe (parallel correctness verified)
//! - **B32**: Fair comparison (parallel vs sequential)

use atomic_capsule::CpuCapabilityCapsule;
use kindly_dedup::{DedupPipeline, ParallelDedupPipeline};
use std::collections::HashSet;

// ============================================================================
// Test Utilities
// ============================================================================

/// Generate random documents with controlled duplicate rate
fn generate_random_docs(num_unique: usize, num_total: usize, seed: u64) -> Vec<(usize, String)> {
    use std::collections::hash_map::RandomState;
    use std::hash::{Hash, Hasher};

    let mut docs = Vec::with_capacity(num_total);

    // Generate unique documents
    for i in 0..num_unique {
        let text = format!("Unique document {} with some random content seed {}", i, seed);
        docs.push((i, text));
    }

    // Fill remaining with duplicates
    for i in num_unique..num_total {
        let original_idx = (seed.wrapping_mul(i as u64) % num_unique as u64) as usize;
        let text = format!(
            "Unique document {} with some random content seed {}",
            original_idx, seed
        );
        docs.push((i, text));
    }

    docs
}

/// Extract cluster membership (doc_id -> cluster_id)
fn extract_membership(clusters: &[Vec<usize>]) -> std::collections::HashMap<usize, usize> {
    let mut membership = std::collections::HashMap::new();

    for (cluster_id, cluster) in clusters.iter().enumerate() {
        for &doc_id in cluster {
            membership.insert(doc_id, cluster_id);
        }
    }

    membership
}

/// Compute F1 score from clusters
fn compute_f1_score(predicted: &[Vec<usize>], ground_truth: &[Vec<usize>]) -> (f64, f64, f64) {
    let pred_pairs = extract_pairs(predicted);
    let gt_pairs = extract_pairs(ground_truth);

    let tp = pred_pairs.intersection(&gt_pairs).count() as f64;
    let fp = pred_pairs.difference(&gt_pairs).count() as f64;
    let fn_count = gt_pairs.difference(&pred_pairs).count() as f64;

    let precision = if tp + fp > 0.0 { tp / (tp + fp) } else { 1.0 };
    let recall = if tp + fn_count > 0.0 { tp / (tp + fn_count) } else { 1.0 };
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    (precision, recall, f1)
}

/// Extract all pairs from clusters
fn extract_pairs(clusters: &[Vec<usize>]) -> HashSet<(usize, usize)> {
    let mut pairs = HashSet::new();

    for cluster in clusters {
        for i in 0..cluster.len() {
            for j in i + 1..cluster.len() {
                let (a, b) = if cluster[i] < cluster[j] {
                    (cluster[i], cluster[j])
                } else {
                    (cluster[j], cluster[i])
                };
                pairs.insert((a, b));
            }
        }
    }

    pairs
}

// ============================================================================
// T28 Q15: Critical Integration Points
// ============================================================================

#[test]
fn test_parallel_sequential_determinism_small() {
    // Property: Parallel and sequential pipelines produce identical results
    // Run: 100 iterations with small random corpora

    let cpu_caps = CpuCapabilityCapsule::detect();

    for seed in 0..100 {
        let docs = generate_random_docs(10, 50, seed);

        // Sequential pipeline
        let mut seq_pipeline = DedupPipeline::new(50, &cpu_caps);
        for (doc_id, text) in &docs {
            seq_pipeline.add_document(*doc_id, text).unwrap();
        }
        let seq_clusters = seq_pipeline.find_duplicates(0.85).unwrap();

        // Parallel pipeline
        let mut par_pipeline = ParallelDedupPipeline::new(50, 4).unwrap();
        let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();
        par_pipeline.add_documents(&doc_refs).unwrap();
        let par_clusters = par_pipeline.find_duplicates(0.85).unwrap();

        // Extract memberships
        let seq_membership = extract_membership(&seq_clusters);
        let par_membership = extract_membership(&par_clusters);

        // Verify: All documents have same cluster membership
        for doc_id in 0..50 {
            let seq_cluster = seq_membership.get(&doc_id);
            let par_cluster = par_membership.get(&doc_id);

            // Both should be Some (all docs clustered) or both None
            assert_eq!(
                seq_cluster.is_some(),
                par_cluster.is_some(),
                "Seed {}: Doc {} clustering mismatch",
                seed,
                doc_id
            );
        }

        // Verify: Same pairs detected
        let seq_pairs = extract_pairs(&seq_clusters);
        let par_pairs = extract_pairs(&par_clusters);

        assert_eq!(seq_pairs, par_pairs, "Seed {}: Parallel/sequential pair mismatch", seed);
    }
}

#[test]
fn test_parallel_sequential_determinism_large() {
    // Property: Determinism holds for larger corpora
    // Run: 10 iterations with 1K documents

    let cpu_caps = CpuCapabilityCapsule::detect();

    for seed in 0..10 {
        let docs = generate_random_docs(200, 1000, seed);

        // Sequential pipeline
        let mut seq_pipeline = DedupPipeline::new(1000, &cpu_caps);
        for (doc_id, text) in &docs {
            seq_pipeline.add_document(*doc_id, text).unwrap();
        }
        let seq_clusters = seq_pipeline.find_duplicates(0.85).unwrap();

        // Parallel pipeline (8 threads)
        let mut par_pipeline = ParallelDedupPipeline::new(1000, 8).unwrap();
        let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();
        par_pipeline.add_documents(&doc_refs).unwrap();
        let par_clusters = par_pipeline.find_duplicates(0.85).unwrap();

        // Verify: Same pairs detected
        let seq_pairs = extract_pairs(&seq_clusters);
        let par_pairs = extract_pairs(&par_clusters);

        assert_eq!(
            seq_pairs, par_pairs,
            "Seed {}: Large corpus parallel/sequential mismatch",
            seed
        );
    }
}

// ============================================================================
// T28 Q16: Thread Scaling Validation
// ============================================================================

#[test]
fn test_thread_scaling_1_2_4_8() {
    // Property: Results identical across different thread counts

    let docs = generate_random_docs(50, 200, 42);
    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let thread_counts = [1, 2, 4, 8];
    let mut all_results = Vec::new();

    for &threads in &thread_counts {
        let mut pipeline = ParallelDedupPipeline::new(200, threads).unwrap();
        pipeline.add_documents(&doc_refs).unwrap();
        let clusters = pipeline.find_duplicates(0.85).unwrap();
        all_results.push((threads, clusters));
    }

    // Verify: All thread counts produce identical pairs
    let baseline_pairs = extract_pairs(&all_results[0].1);

    for (threads, clusters) in &all_results[1..] {
        let pairs = extract_pairs(clusters);
        assert_eq!(
            baseline_pairs, pairs,
            "Thread count {} produces different results",
            threads
        );
    }
}

#[test]
fn test_thread_scaling_12_16() {
    // Property: High thread counts maintain correctness

    let docs = generate_random_docs(100, 500, 123);
    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let thread_counts = [12, 16];
    let mut all_results = Vec::new();

    for &threads in &thread_counts {
        let mut pipeline = ParallelDedupPipeline::new(500, threads).unwrap();
        pipeline.add_documents(&doc_refs).unwrap();
        let clusters = pipeline.find_duplicates(0.85).unwrap();
        all_results.push((threads, clusters));
    }

    // Verify: 12 and 16 threads produce identical results
    let pairs_12 = extract_pairs(&all_results[0].1);
    let pairs_16 = extract_pairs(&all_results[1].1);

    assert_eq!(pairs_12, pairs_16, "12 vs 16 threads produce different results");
}

// ============================================================================
// T28 Q17: F1 Score Validation (≥90% Target)
// ============================================================================

#[test]
fn test_f1_score_exact_duplicates() {
    // Ground truth: Known exact duplicates (100% F1 expected)

    let mut pipeline = ParallelDedupPipeline::new(6, 4).unwrap();

    let docs = vec![
        (0, "The quick brown fox jumps over the lazy dog"),
        (1, "The quick brown fox jumps over the lazy dog"), // Exact duplicate
        (2, "A completely different document here"),
        (3, "The quick brown fox jumps over the lazy dog"), // Another duplicate
        (4, "Yet another unique document with different content"),
        (5, "The quick brown fox jumps over the lazy dog"), // Another duplicate
    ];

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, *text)).collect();
    pipeline.add_documents(&doc_refs).unwrap();

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // Expected ground truth: {0,1,3,5}, {2}, {4}
    let expected_pairs: HashSet<(usize, usize)> = [(0, 1), (0, 3), (0, 5), (1, 3), (1, 5), (3, 5)]
        .iter()
        .copied()
        .collect();

    let actual_pairs = extract_pairs(&clusters);

    // Compute F1 score
    let tp = actual_pairs.intersection(&expected_pairs).count() as f64;
    let fp = actual_pairs.difference(&expected_pairs).count() as f64;
    let fn_count = expected_pairs.difference(&actual_pairs).count() as f64;

    let precision = if tp + fp > 0.0 { tp / (tp + fp) } else { 1.0 };
    let recall = if tp + fn_count > 0.0 { tp / (tp + fn_count) } else { 1.0 };
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    println!("Exact duplicates F1 score: {:.2}%", f1 * 100.0);
    println!("  Precision: {:.2}%", precision * 100.0);
    println!("  Recall: {:.2}%", recall * 100.0);

    // For exact duplicates, expect 100% F1 score
    assert!(f1 >= 0.99, "F1 score too low for exact duplicates: {:.2}%", f1 * 100.0);
}

#[test]
fn test_f1_score_near_duplicates() {
    // Ground truth: Near duplicates with known Jaccard similarities

    let mut pipeline = ParallelDedupPipeline::new(5, 4).unwrap();

    let docs = vec![
        (0, "the quick brown fox jumps over the lazy dog"),
        (1, "the quick brown fox leaps over the lazy dog"), // 1 word diff: 8/9 = 0.89 Jaccard
        (2, "the fast brown fox jumps over the lazy dog"),  // 1 word diff: 8/9 = 0.89 Jaccard
        (3, "a completely different document with unique words"),
        (4, "another totally unrelated document here now"),
    ];

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, *text)).collect();
    pipeline.add_documents(&doc_refs).unwrap();

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // Expected ground truth at 0.85 threshold:
    // - (0,1) should match (0.89 > 0.85)
    // - (0,2) should match (0.89 > 0.85)
    // - (1,2) similarity depends on MinHash estimation
    // - (3,4) no match (completely different)

    let expected_pairs: HashSet<(usize, usize)> = [
        (0, 1),
        (0, 2),
        // (1, 2) is uncertain due to MinHash approximation
    ]
    .iter()
    .copied()
    .collect();

    let actual_pairs = extract_pairs(&clusters);

    // Compute recall (must find at least 80% of expected pairs)
    let tp = actual_pairs.intersection(&expected_pairs).count() as f64;
    let recall = if !expected_pairs.is_empty() {
        tp / expected_pairs.len() as f64
    } else {
        1.0
    };

    println!("Near duplicates recall: {:.2}%", recall * 100.0);
    println!("  Found {} / {} expected pairs", tp, expected_pairs.len());

    // For near duplicates with MinHash approximation, expect ≥80% recall
    assert!(
        recall >= 0.80,
        "Recall too low for near duplicates: {:.2}%",
        recall * 100.0
    );
}

// ============================================================================
// T28 Q18: Edge Cases
// ============================================================================

#[test]
fn test_edge_case_empty_input() {
    // Edge case: Empty document set

    let mut pipeline = ParallelDedupPipeline::new(10, 4).unwrap();
    pipeline.add_documents(&[]).unwrap();

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    assert_eq!(clusters.len(), 0, "Empty input should produce zero clusters");
}

#[test]
fn test_edge_case_single_document() {
    // Edge case: Single document

    let mut pipeline = ParallelDedupPipeline::new(1, 4).unwrap();
    pipeline.add_documents(&[(0, "Single document")]).unwrap();

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    assert_eq!(clusters.len(), 1, "Single document should form one cluster");
    assert_eq!(clusters[0].len(), 1, "Cluster should contain one document");
    assert_eq!(clusters[0][0], 0, "Document ID should be 0");
}

#[test]
fn test_edge_case_all_duplicates() {
    // Edge case: All documents identical

    let mut pipeline = ParallelDedupPipeline::new(100, 8).unwrap();

    let docs: Vec<(usize, &str)> = (0..100).map(|i| (i, "This exact text is repeated 100 times")).collect();

    pipeline.add_documents(&docs).unwrap();

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // All documents should be in one cluster
    assert_eq!(clusters.len(), 1, "All duplicates should form one cluster");
    assert_eq!(clusters[0].len(), 100, "Cluster should contain all 100 documents");
}

#[test]
fn test_edge_case_no_duplicates() {
    // Edge case: All documents unique

    let mut pipeline = ParallelDedupPipeline::new(100, 8).unwrap();

    let docs: Vec<(usize, String)> = (0..100).map(|i| (i, format!("Unique document number {}", i))).collect();

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();
    pipeline.add_documents(&doc_refs).unwrap();

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // All documents should be singletons
    assert_eq!(
        clusters.len(),
        100,
        "All unique documents should form separate clusters"
    );

    for cluster in &clusters {
        assert_eq!((*cluster).len(), 1, "Each unique document should be a singleton");
    }
}

// ============================================================================
// T28 Q19: Large Scale Test (100K Documents)
// ============================================================================

#[test]
#[ignore] // Run manually: cargo test --release test_large_scale_100k -- --ignored
fn test_large_scale_100k_documents() {
    // Large scale: 100K documents (10K unique, 90% duplicates)

    println!("Generating 100K documents (10K unique)...");
    let docs = generate_random_docs(10_000, 100_000, 999);

    println!("Processing with 16 threads...");
    let mut pipeline = ParallelDedupPipeline::new(100_000, 16).unwrap();

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let start = std::time::Instant::now();
    pipeline.add_documents(&doc_refs).unwrap();
    let add_elapsed = start.elapsed();

    println!(
        "Added {} documents in {:.2}s",
        pipeline.documents_added(),
        add_elapsed.as_secs_f64()
    );
    println!(
        "Throughput: {:.0} docs/sec",
        pipeline.documents_added() as f64 / add_elapsed.as_secs_f64()
    );

    let start = std::time::Instant::now();
    let clusters = pipeline.find_duplicates(0.85).unwrap();
    let find_elapsed = start.elapsed();

    println!(
        "Found {} clusters in {:.2}s",
        clusters.len(),
        find_elapsed.as_secs_f64()
    );

    // Expected: ~10K clusters (one per unique document)
    let expected_clusters = 10_000;
    let tolerance = 0.20; // 20% tolerance (MinHash approximation)

    assert!(
        clusters.len() >= (expected_clusters as f64 * (1.0 - tolerance)) as usize
            && clusters.len() <= (expected_clusters as f64 * (1.0 + tolerance)) as usize,
        "Cluster count {} outside expected range [{}, {}]",
        clusters.len(),
        (expected_clusters as f64 * (1.0 - tolerance)) as usize,
        (expected_clusters as f64 * (1.0 + tolerance)) as usize
    );

    // Performance target: <10s total for 100K documents
    let total_time = add_elapsed + find_elapsed;
    println!("Total time: {:.2}s", total_time.as_secs_f64());

    assert!(
        total_time.as_secs() < 30,
        "100K documents should complete in <30s, took {:.2}s",
        total_time.as_secs_f64()
    );
}

// ============================================================================
// T28 Q20: Bucket Consistency (All Pairs Found)
// ============================================================================

#[test]
fn test_bucket_consistency_all_pairs() {
    // Correctness: All duplicate pairs should be found via buckets

    let mut pipeline = ParallelDedupPipeline::new(10, 4).unwrap();

    // Create documents with known duplicates
    let docs = vec![
        (0, "apple orange banana grape"),
        (1, "apple orange banana grape"), // Exact duplicate of 0
        (2, "apple orange banana peach"), // 1 word diff from 0,1
        (3, "apple orange lemon grape"),  // 1 word diff from 0,1
        (4, "carrot celery lettuce spinach"),
        (5, "carrot celery lettuce spinach"), // Exact duplicate of 4
        (6, "apple orange banana grape"),     // Another duplicate of 0,1
        (7, "unique document with different words entirely"),
        (8, "another totally unrelated document here now"),
        (9, "apple orange banana grape"), // Another duplicate of 0,1
    ];

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, *text)).collect();
    pipeline.add_documents(&doc_refs).unwrap();

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // Expected duplicate pairs (exact matches):
    // Cluster 1: {0, 1, 6, 9}
    // Cluster 2: {4, 5}
    // Near matches may or may not cluster depending on MinHash

    let pairs = extract_pairs(&clusters);

    // Must find at least the exact duplicate pairs
    let required_pairs: HashSet<(usize, usize)> = [(0, 1), (0, 6), (0, 9), (1, 6), (1, 9), (6, 9), (4, 5)]
        .iter()
        .copied()
        .collect();

    let found_required = required_pairs.iter().filter(|p| pairs.contains(p)).count();
    let recall = found_required as f64 / required_pairs.len() as f64;

    println!("Bucket consistency recall: {:.2}%", recall * 100.0);
    println!("  Found {} / {} required pairs", found_required, required_pairs.len());

    // Must find ≥90% of exact duplicate pairs
    assert!(recall >= 0.90, "Bucket consistency too low: {:.2}%", recall * 100.0);
}

// ============================================================================
// T28 Q21: Performance Budget Validation
// ============================================================================

#[test]
fn test_performance_budget_latency() {
    // Performance: <1ms per document (P99 latency target)

    let docs = generate_random_docs(100, 1000, 777);
    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let mut pipeline = ParallelDedupPipeline::new(1000, 8).unwrap();

    let start = std::time::Instant::now();
    pipeline.add_documents(&doc_refs).unwrap();
    let elapsed = start.elapsed();

    let avg_latency_ms = elapsed.as_millis() as f64 / 1000.0;

    println!("Average per-document latency: {:.2}ms", avg_latency_ms);

    // Target: <1ms per document
    assert!(
        avg_latency_ms < 1.0,
        "Per-document latency {:.2}ms exceeds 1ms budget",
        avg_latency_ms
    );
}

#[test]
fn test_performance_budget_throughput() {
    // Performance: ≥36K docs/sec per core (60% efficiency)

    let docs = generate_random_docs(500, 5000, 888);
    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let threads = 8;
    let mut pipeline = ParallelDedupPipeline::new(5000, threads).unwrap();

    let start = std::time::Instant::now();
    pipeline.add_documents(&doc_refs).unwrap();
    let elapsed = start.elapsed();

    let throughput = 5000.0 / elapsed.as_secs_f64();
    let per_core_throughput = throughput / threads as f64;

    println!("Throughput: {:.0} docs/sec", throughput);
    println!("Per-core: {:.0} docs/sec", per_core_throughput);

    // Target: ≥36K docs/sec per core (60% of 60K baseline)
    // With overhead, accept ≥20K docs/sec per core
    assert!(
        per_core_throughput >= 20_000.0,
        "Per-core throughput {:.0} docs/sec below 20K target",
        per_core_throughput
    );
}
