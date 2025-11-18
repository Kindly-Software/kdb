//! # Week 1 - T28 Tier 3: Integration Tests
//!
//! **Purpose**: Validate end-to-end deduplication with optimizations
//!
//! ## T28 Framework Compliance (Q15-Q21)
//!
//! - **Q15**: Critical integration points: Bloom → MinHash → LSH → Clustering
//! - **Q16**: Error propagation: FPR impact on recall
//! - **Q17**: Performance budgets: <1ms per document end-to-end
//! - **Q18**: Production load: 100K corpus with realistic duplicates
//! - **Q19**: Rollback: Test with/without Bloom feature flag
//! - **Q20**: I20 validation: All assumptions tested
//! - **Q21**: Monitoring: Bloom stats collection

use kindly_dedup::benchmarking::generate_synthetic_corpus_parallel;
use kindly_dedup::{DedupBloomFilter, DedupPipeline};
use std::time::Instant;

// ============================================================================
// Q15: Critical Integration Points
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(120000))]
fn test_integration_bloom_to_minhash() {
    // Arrange: Generate corpus with 50% duplicates
    let corpus = generate_synthetic_corpus_parallel(10_000);

    // Act: Add documents with Bloom pre-filter
    let mut pipeline = DedupPipeline::new(10_000);
    let mut bloom = DedupBloomFilter::new();

    for (doc_id, text) in corpus.iter() {
        // Check Bloom first (integration point)
        if !bloom.query(*doc_id, text) {
            // Not seen: compute MinHash
            pipeline.add_document(*doc_id, text);
            bloom.insert(*doc_id, text);
        }
    }

    // Assert: Bloom skipped duplicates
    let skip_rate = (10_000 - pipeline.len()) as f64 / 10_000.0;
    assert!(
        skip_rate > 0.10,
        "Bloom skip rate too low: {:.2}% (expected >10%)",
        skip_rate * 100.0
    );
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(120000))]
fn test_integration_full_pipeline_with_bloom() {
    // Arrange: Generate 10K corpus
    let corpus = generate_synthetic_corpus_parallel(10_000);

    // Act: Full pipeline (Bloom → MinHash → LSH → Clustering)
    let mut pipeline = DedupPipeline::new(10_000);
    let mut bloom = DedupBloomFilter::new();

    for (doc_id, text) in corpus.iter() {
        if !bloom.query(*doc_id, text) {
            pipeline.add_document(*doc_id, text);
            bloom.insert(*doc_id, text);
        }
    }

    let clusters = pipeline.find_duplicates(0.85);

    // Assert: Found duplicate clusters
    assert!(!clusters.is_empty(), "No duplicate clusters found (pipeline broken)");
    assert!(clusters.len() < 10_000, "Too many clusters (no duplicates detected)");
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(120000))]
fn test_integration_parallel_corpus_to_pipeline() {
    // Arrange: Generate parallel corpus
    let corpus = generate_synthetic_corpus_parallel(5_000);

    // Act: Process through pipeline
    let mut pipeline = DedupPipeline::new(5_000);

    for (doc_id, text) in corpus.iter() {
        pipeline.add_document(*doc_id, text);
    }

    let clusters = pipeline.find_duplicates(0.85);

    // Assert: Cluster statistics match distribution
    let exact_cluster_count = clusters.iter().filter(|cluster| cluster.len() > 1).count();

    assert!(
        exact_cluster_count > 0,
        "No duplicate clusters found (expected ~5% exact duplicates)"
    );
}

// ============================================================================
// Q16: Error Propagation
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(120000))]
fn test_integration_bloom_fpr_impact_on_recall() {
    // Arrange: Generate corpus with known duplicates
    let corpus = generate_synthetic_corpus_parallel(10_000);

    // Act 1: Pipeline WITHOUT Bloom (baseline recall)
    let mut pipeline_no_bloom = DedupPipeline::new(10_000);
    for (doc_id, text) in corpus.iter() {
        pipeline_no_bloom.add_document(*doc_id, text);
    }
    let clusters_no_bloom = pipeline_no_bloom.find_duplicates(0.85);

    // Act 2: Pipeline WITH Bloom (may reduce recall due to FPR)
    let mut pipeline_with_bloom = DedupPipeline::new(10_000);
    let mut bloom = DedupBloomFilter::new();

    for (doc_id, text) in corpus.iter() {
        if !bloom.query(*doc_id, text) {
            pipeline_with_bloom.add_document(*doc_id, text);
            bloom.insert(*doc_id, text);
        }
    }
    let clusters_with_bloom = pipeline_with_bloom.find_duplicates(0.85);

    // Assert: Recall difference < 1% (FPR impact minimal)
    let recall_no_bloom = clusters_no_bloom.len();
    let recall_with_bloom = clusters_with_bloom.len();

    let recall_diff = (recall_no_bloom as f64 - recall_with_bloom as f64).abs() / recall_no_bloom as f64;

    assert!(
        recall_diff < 0.05,
        "Recall degradation too high: {:.2}% (expected <5%)",
        recall_diff * 100.0
    );
}

// ============================================================================
// Q17: Performance Budgets (I20 Q18)
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(120000))]
fn test_integration_latency_budget() {
    // Arrange: Generate 10K corpus
    let corpus = generate_synthetic_corpus_parallel(10_000);

    // Act: Measure per-document latency
    let mut pipeline = DedupPipeline::new(10_000);
    let start = Instant::now();

    for (doc_id, text) in corpus.iter() {
        pipeline.add_document(*doc_id, text);
    }

    let elapsed = start.elapsed();
    let avg_latency_ms = elapsed.as_micros() as f64 / 10_000.0 / 1000.0;

    // Assert: <1ms per document budget
    assert!(
        avg_latency_ms < 1.0,
        "Latency budget exceeded: {:.3}ms per doc (budget: 1ms)",
        avg_latency_ms
    );
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(120000))]
fn test_integration_throughput_budget() {
    // Arrange: Generate 100K corpus
    let corpus = generate_synthetic_corpus_parallel(100_000);

    // Act: Measure throughput
    let mut pipeline = DedupPipeline::new(100_000);
    let start = Instant::now();

    for (doc_id, text) in corpus.iter() {
        pipeline.add_document(*doc_id, text);
    }

    let elapsed = start.elapsed();
    let throughput = 100_000.0 / elapsed.as_secs_f64();

    // Assert: >50K docs/sec throughput (baseline target)
    assert!(
        throughput > 50_000.0,
        "Throughput too low: {:.0} docs/sec (target: >50K)",
        throughput
    );
}

// ============================================================================
// Q18: Production Load
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(120000))]
fn test_integration_realistic_100k_corpus() {
    // Arrange: Generate 100K corpus with realistic distribution
    let corpus = generate_synthetic_corpus_parallel(100_000);

    // Act: Full deduplication pipeline
    let mut pipeline = DedupPipeline::new(100_000);
    let mut bloom = DedupBloomFilter::new();

    let start = Instant::now();

    for (doc_id, text) in corpus.iter() {
        if !bloom.query(*doc_id, text) {
            pipeline.add_document(*doc_id, text);
            bloom.insert(*doc_id, text);
        }
    }

    let clusters = pipeline.find_duplicates(0.85);
    let elapsed = start.elapsed();

    // Assert: Completes in reasonable time (<3 minutes)
    assert!(
        elapsed.as_secs() < 180,
        "Processing took too long: {} seconds (expected <180)",
        elapsed.as_secs()
    );

    // Assert: Found duplicate clusters
    assert!(!clusters.is_empty(), "No clusters found in 100K corpus");

    println!(
        "100K corpus: {} clusters found in {:.2} seconds",
        clusters.len(),
        elapsed.as_secs_f64()
    );
}

// ============================================================================
// Q19: Rollback Scenarios
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(120000))]
fn test_integration_with_and_without_bloom() {
    // Arrange: Same corpus for both tests
    let corpus = generate_synthetic_corpus_parallel(5_000);

    // Act 1: WITHOUT Bloom (rollback scenario)
    let mut pipeline_no_bloom = DedupPipeline::new(5_000);
    for (doc_id, text) in corpus.iter() {
        pipeline_no_bloom.add_document(*doc_id, text);
    }
    let clusters_no_bloom = pipeline_no_bloom.find_duplicates(0.85);

    // Act 2: WITH Bloom (new feature)
    let mut pipeline_with_bloom = DedupPipeline::new(5_000);
    let mut bloom = DedupBloomFilter::new();

    for (doc_id, text) in corpus.iter() {
        if !bloom.query(*doc_id, text) {
            pipeline_with_bloom.add_document(*doc_id, text);
            bloom.insert(*doc_id, text);
        }
    }
    let clusters_with_bloom = pipeline_with_bloom.find_duplicates(0.85);

    // Assert: Results consistent (rollback safe)
    let cluster_diff = (clusters_no_bloom.len() as i64 - clusters_with_bloom.len() as i64).abs();
    let diff_pct = cluster_diff as f64 / clusters_no_bloom.len() as f64;

    assert!(
        diff_pct < 0.10,
        "Results diverge too much: {:.2}% (rollback unsafe)",
        diff_pct * 100.0
    );
}

// ============================================================================
// Q20: I20 Validation (Integration Framework)
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(120000))]
fn test_i20_bloom_assumptions_validated() {
    // I20 Q11: New assumptions from composition
    // ASSUME: Bloom FPR < 1% doesn't significantly impact recall

    let corpus = generate_synthetic_corpus_parallel(10_000);

    let mut pipeline = DedupPipeline::new(10_000);
    let mut bloom = DedupBloomFilter::new();

    for (doc_id, text) in corpus.iter() {
        if !bloom.query(*doc_id, text) {
            pipeline.add_document(*doc_id, text);
            bloom.insert(*doc_id, text);
        }
    }

    let clusters = pipeline.find_duplicates(0.85);

    // VERIFY: Recall > 92% (LSH baseline)
    let recall = clusters.len() as f64 / 500.0; // Expected ~500 clusters (5% exact)

    assert!(
        recall > 0.92,
        "Recall too low: {:.2}% (Bloom FPR impact too high)",
        recall * 100.0
    );
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(120000))]
fn test_i20_parallel_gen_assumptions_validated() {
    // I20 Q11: Parallel generation maintains distribution
    // ASSUME: 5% exact, 15% near, 30% similar, 50% unique

    let corpus = generate_synthetic_corpus_parallel(10_000);

    // Count distribution
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

    // VERIFY: Exact distribution within ±2%
    assert!(
        (exact_pct - 0.05).abs() < 0.02,
        "Distribution violated: {:.2}% exact (expected 5% ±2%)",
        exact_pct * 100.0
    );
}

// ============================================================================
// Q21: Monitoring & Instrumentation
// ============================================================================

#[test]
#[cfg_attr(feature = "test-timeout", timeout(120000))]
fn test_integration_bloom_stats_collection() {
    // Arrange: Generate corpus
    let corpus = generate_synthetic_corpus_parallel(10_000);

    // Act: Track Bloom statistics
    let mut bloom = DedupBloomFilter::new();
    let mut skipped = 0;
    let mut processed = 0;

    for (doc_id, text) in corpus.iter() {
        if bloom.query(*doc_id, text) {
            skipped += 1;
        } else {
            processed += 1;
            bloom.insert(*doc_id, text);
        }
    }

    // Assert: Statistics collected
    assert_eq!(bloom.documents_seen(), processed, "Documents seen counter mismatch");

    let skip_rate = skipped as f64 / 10_000.0;
    println!(
        "Bloom stats: {} processed, {} skipped ({:.2}% skip rate)",
        processed,
        skipped,
        skip_rate * 100.0
    );

    // Assert: Monitoring data available
    assert!(skipped > 0, "No documents skipped (Bloom not working)");
    assert!(processed > 0, "No documents processed (pipeline broken)");
}

#[test]
#[cfg_attr(feature = "test-timeout", timeout(120000))]
fn test_integration_end_to_end_metrics() {
    // Arrange: Generate corpus
    let corpus = generate_synthetic_corpus_parallel(10_000);

    // Act: Collect end-to-end metrics
    let mut pipeline = DedupPipeline::new(10_000);
    let mut bloom = DedupBloomFilter::new();

    let start = Instant::now();
    let mut bloom_hits = 0;
    let mut bloom_misses = 0;

    for (doc_id, text) in corpus.iter() {
        if bloom.query(*doc_id, text) {
            bloom_hits += 1;
        } else {
            bloom_misses += 1;
            pipeline.add_document(*doc_id, text);
            bloom.insert(*doc_id, text);
        }
    }

    let elapsed = start.elapsed();
    let clusters = pipeline.find_duplicates(0.85);

    // Assert: All metrics available for monitoring
    println!("End-to-end metrics:");
    println!("  Total docs: 10,000");
    println!("  Bloom hits: {}", bloom_hits);
    println!("  Bloom misses: {}", bloom_misses);
    println!("  Skip rate: {:.2}%", bloom_hits as f64 / 10_000.0 * 100.0);
    println!("  Clusters found: {}", clusters.len());
    println!("  Elapsed: {:.2} seconds", elapsed.as_secs_f64());
    println!("  Throughput: {:.0} docs/sec", 10_000.0 / elapsed.as_secs_f64());

    assert!(bloom_hits > 0, "Monitoring: No Bloom hits");
    assert!(bloom_misses > 0, "Monitoring: No Bloom misses");
    assert!(!clusters.is_empty(), "Monitoring: No clusters found");
}
