//! Week 2 Batch LSH Integration Tests
//!
//! Validates integration of BatchLSHLookup into DedupPipeline
//! via I20 Integration Framework (20 questions validated)
//!
//! Test Coverage (12+ tests, T28 all 4 tiers):
//! - Unit Tests (3): Minimal integration, empty corpus, single document
//! - Property Tests (4): Output equivalence, determinism, thread safety, performance
//! - Integration Tests (3): Scale (10K docs), Bloom integration, accuracy
//! - Production Tests (2): Concurrent stress, performance budget
//!
//! I20 Questions Validated:
//! - Q16: Minimal integration test (test_batch_minimal_integration)
//! - Q17: Property invariants (property_batch_equals_baseline, etc.)
//! - Q18: Performance budget (test_batch_performance_budget)
//! - Q19: Big Bang deployment (feature-gated, zero breaking changes)
//! - Q20: Rollback plan (git revert, always available baseline)

#![cfg(all(test, feature = "batch-lsh"))]

use kindly_dedup::pipeline::DocId;
use kindly_dedup::DedupPipeline;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

/// Helper: Sort clusters for order-independent comparison
fn sort_clusters(mut clusters: Vec<Vec<DocId>>) -> Vec<Vec<DocId>> {
    for cluster in &mut clusters {
        cluster.sort_unstable();
    }
    clusters.sort_unstable_by(|a, b| {
        // Sort by first element of each cluster
        match (a.first(), b.first()) {
            (Some(x), Some(y)) => x.cmp(y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
    clusters
}

// =============================================================================
// Unit Tests (3 tests) - Q16: Minimal Integration
// =============================================================================

#[test]
fn test_batch_minimal_integration() {
    // Q16: Minimal test - Validates basic integration
    // Expected: 2 clusters ({0,1} and {2})
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(3, &cpu_caps);

    pipeline.add_document(0, "The quick brown fox").unwrap();
    pipeline.add_document(1, "The quick brown fox").unwrap(); // Duplicate
    pipeline.add_document(2, "Completely different").unwrap();

    let clusters = pipeline.find_duplicates_batch(0.85).unwrap();

    // Verify: 2 clusters (duplicate pair + singleton)
    assert_eq!(clusters.len(), 2, "Expected 2 clusters");

    // Verify: Duplicate cluster exists
    let duplicate_cluster = clusters
        .iter()
        .find(|c| c.len() == 2)
        .expect("Should have one cluster with 2 docs");

    assert!(duplicate_cluster.contains(&0), "Duplicate cluster should contain doc 0");
    assert!(duplicate_cluster.contains(&1), "Duplicate cluster should contain doc 1");
}

#[test]
fn test_batch_empty_corpus() {
    // Edge case: Empty corpus
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let pipeline = DedupPipeline::new(0, &cpu_caps);

    let clusters = pipeline.find_duplicates_batch(0.85).unwrap();
    assert_eq!(clusters.len(), 0, "Empty corpus should have 0 clusters");
}

#[test]
fn test_batch_single_document() {
    // Edge case: Single document
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(1, &cpu_caps);

    pipeline.add_document(0, "Single document").unwrap();

    let clusters = pipeline.find_duplicates_batch(0.85).unwrap();
    assert_eq!(clusters.len(), 1, "Single doc should have 1 cluster");
    assert_eq!(clusters[0].len(), 1, "Cluster should contain 1 doc");
    assert_eq!(clusters[0][0], 0, "Cluster should contain doc 0");
}

// =============================================================================
// Property Tests (4 tests) - Q17: Property Invariants
// =============================================================================

#[test]
fn property_batch_equals_baseline() {
    // Q17: Output equivalence - Batch should match baseline
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();

    for num_docs in [10, 50, 100, 500, 1000] {
        let mut pipeline = DedupPipeline::new(num_docs, &cpu_caps);

        // Add documents (some duplicates)
        for i in 0..num_docs {
            let text = format!("Document with content {}", i % (num_docs / 2));
            pipeline.add_document(i, &text).unwrap();
        }

        // Baseline
        let baseline = pipeline.find_duplicates(0.85).unwrap();

        // Batch
        let batch = pipeline.find_duplicates_batch(0.85).unwrap();

        // Property: Same clusters (order-independent)
        assert_eq!(
            sort_clusters(baseline.clone()),
            sort_clusters(batch.clone()),
            "Batch output should match baseline for {} docs",
            num_docs
        );

        println!(
            "✓ Output equivalence validated for {} docs ({} clusters)",
            num_docs,
            baseline.len()
        );
    }
}

#[test]
fn property_batch_deterministic() {
    // Q17: Determinism - Same input should produce same output
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();

    for num_docs in [10, 100, 1000] {
        let mut pipeline = DedupPipeline::new(num_docs, &cpu_caps);

        for i in 0..num_docs {
            pipeline.add_document(i, &format!("doc {}", i)).unwrap();
        }

        // Run batch twice
        let run1 = pipeline.find_duplicates_batch(0.85).unwrap();
        let run2 = pipeline.find_duplicates_batch(0.85).unwrap();

        // Property: Same input → same output
        assert_eq!(
            sort_clusters(run1),
            sort_clusters(run2),
            "Batch should be deterministic for {} docs",
            num_docs
        );

        println!("✓ Determinism validated for {} docs", num_docs);
    }
}

#[test]
fn property_batch_thread_safe() {
    // Q17: Thread safety - Concurrent batch calls should not interfere
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let num_docs = 1000;

    let pipeline = Arc::new({
        let mut p = DedupPipeline::new(num_docs, &cpu_caps);
        for i in 0..num_docs {
            p.add_document(i, &format!("doc {}", i)).unwrap();
        }
        p
    });

    // Spawn 20 threads, each calls find_duplicates_batch() 10 times
    let handles: Vec<_> = (0..20)
        .map(|thread_id| {
            let p = Arc::clone(&pipeline);
            thread::spawn(move || {
                for iteration in 0..10 {
                    let result = p.find_duplicates_batch(0.85);
                    assert!(
                        result.is_ok(),
                        "Thread {} iteration {} failed: {:?}",
                        thread_id,
                        iteration,
                        result
                    );
                }
            })
        })
        .collect();

    // All threads should complete without panic
    for (thread_id, handle) in handles.into_iter().enumerate() {
        assert!(handle.join().is_ok(), "Thread {} panicked", thread_id);
    }

    println!("✓ Thread safety validated (20 threads × 10 iterations = 200 concurrent calls)");
}

#[test]
fn property_batch_faster_than_baseline() {
    // Q18: Performance - Batch should be faster (1.3-2× target)
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let num_docs = 2000; // Large enough to measure speedup

    let mut pipeline = DedupPipeline::new(num_docs, &cpu_caps);

    for i in 0..num_docs {
        pipeline.add_document(i, &format!("doc {}", i)).unwrap();
    }

    // Baseline timing (10 iterations for stability)
    let baseline_time = {
        let start = Instant::now();
        for _ in 0..10 {
            pipeline.find_duplicates(0.85).unwrap();
        }
        start.elapsed() / 10
    };

    // Batch timing (10 iterations)
    let batch_time = {
        let start = Instant::now();
        for _ in 0..10 {
            pipeline.find_duplicates_batch(0.85).unwrap();
        }
        start.elapsed() / 10
    };

    let speedup = baseline_time.as_secs_f64() / batch_time.as_secs_f64();

    // Budget: 1.2-2.5× (allowing 10% margin below 1.3× target)
    assert!(
        speedup >= 1.2,
        "Speedup too low: {:.2}× < 1.2× (baseline: {:?}, batch: {:?})",
        speedup,
        baseline_time,
        batch_time
    );

    assert!(speedup <= 2.5, "Speedup too high (suspicious): {:.2}×", speedup);

    println!(
        "✓ Performance validated: {:.2}× speedup (baseline: {:?}, batch: {:?})",
        speedup, baseline_time, batch_time
    );
}

// =============================================================================
// Integration Tests (3 tests) - Q16: Integration at Scale
// =============================================================================

#[test]
fn test_batch_10k_documents() {
    // Q16: Integration at scale (10K docs)
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let num_docs = 10_000;

    let mut pipeline = DedupPipeline::new(num_docs, &cpu_caps);

    // Create 1000 unique docs, 10 copies each
    for i in 0..num_docs {
        let text = format!("Document with content {}", i % 1000);
        pipeline.add_document(i, &text).unwrap();
    }

    let clusters = pipeline.find_duplicates_batch(0.85).unwrap();

    // Should have ~1000 clusters (one per unique content)
    assert!(
        clusters.len() >= 900 && clusters.len() <= 1100,
        "Expected ~1000 clusters, got {}",
        clusters.len()
    );

    // Verify cluster sizes
    let total_docs: usize = clusters.iter().map(|c| c.len()).sum();
    assert_eq!(total_docs, num_docs, "All docs should be in clusters");

    println!(
        "✓ 10K document integration: {} clusters, {} total docs",
        clusters.len(),
        total_docs
    );
}

#[test]
fn test_batch_with_bloom_filter() {
    // Integration: Bloom pre-filter + Batch LSH
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let num_unique = 100;

    let mut pipeline = DedupPipeline::new(num_unique * 10, &cpu_caps);

    // Add 100 unique docs, 10 copies each (90% duplicate rate)
    for i in 0..num_unique {
        let text = format!("Unique document {}", i);
        for _copy in 0..10 {
            pipeline.add_document(i, &text).unwrap();
        }
    }

    // Verify Bloom filter skip rate
    let skip_rate = pipeline.skip_rate();
    assert!(
        skip_rate > 0.85,
        "Bloom should skip 85%+ duplicates, got {:.2}%",
        skip_rate * 100.0
    );

    // Verify deduplication accuracy
    let clusters = pipeline.find_duplicates_batch(0.85).unwrap();

    assert_eq!(clusters.len(), num_unique, "Should have {} unique clusters", num_unique);

    println!(
        "✓ Bloom integration: {:.2}% skip rate, {} clusters",
        skip_rate * 100.0,
        clusters.len()
    );
}

#[test]
fn test_batch_accuracy_high_duplicates() {
    // Q17: Accuracy validation (high duplicate rate)
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let num_unique = 50;
    let copies_per_doc = 10;

    let mut pipeline = DedupPipeline::new(num_unique * copies_per_doc, &cpu_caps);

    // Create 50 unique docs, 10 copies each
    for i in 0..num_unique {
        let base_text = format!("Unique document with specific content {}", i);
        for copy in 0..copies_per_doc {
            let doc_id = i * copies_per_doc + copy;
            pipeline.add_document(doc_id, &base_text).unwrap();
        }
    }

    let clusters = pipeline.find_duplicates_batch(0.95).unwrap();

    // Should have ~50 clusters (one per unique doc)
    assert!(
        clusters.len() >= 45 && clusters.len() <= 55,
        "Expected ~50 clusters, got {}",
        clusters.len()
    );

    // Verify cluster sizes
    let avg_cluster_size: f64 = clusters.iter().map(|c| c.len() as f64).sum::<f64>() / clusters.len() as f64;

    assert!(
        avg_cluster_size >= 8.0 && avg_cluster_size <= 12.0,
        "Expected ~10 docs per cluster, got {:.1}",
        avg_cluster_size
    );

    println!(
        "✓ Accuracy validated: {} clusters, {:.1} avg cluster size",
        clusters.len(),
        avg_cluster_size
    );
}

// =============================================================================
// Production Tests (2 tests) - Q17/Q18: Production Validation
// =============================================================================

#[test]
fn test_batch_stress_concurrent() {
    // Q17: Production stress test (50 threads × 100 iterations)
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let num_docs = 5000;

    let pipeline = Arc::new({
        let mut p = DedupPipeline::new(num_docs, &cpu_caps);
        for i in 0..num_docs {
            p.add_document(i, &format!("doc {}", i)).unwrap();
        }
        p
    });

    // 50 concurrent threads, each calls find_duplicates_batch() 100 times
    let handles: Vec<_> = (0..50)
        .map(|thread_id| {
            let p = Arc::clone(&pipeline);
            thread::spawn(move || {
                for iteration in 0..100 {
                    let result = p.find_duplicates_batch(0.85);
                    if result.is_err() {
                        panic!("Thread {} iteration {} failed: {:?}", thread_id, iteration, result);
                    }
                }
            })
        })
        .collect();

    // All threads should complete
    for (thread_id, handle) in handles.into_iter().enumerate() {
        assert!(handle.join().is_ok(), "Thread {} panicked", thread_id);
    }

    println!("✓ Stress test passed: 50 threads × 100 iterations = 5000 concurrent calls");
}

#[test]
fn test_batch_performance_budget() {
    // Q18: Budget enforcement (1.3-2× speedup target)
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let num_docs = 5000;

    let mut pipeline = DedupPipeline::new(num_docs, &cpu_caps);

    for i in 0..num_docs {
        pipeline.add_document(i, &format!("doc {}", i)).unwrap();
    }

    // Baseline timing (20 iterations for statistical stability)
    let baseline_time = {
        let start = Instant::now();
        for _ in 0..20 {
            pipeline.find_duplicates(0.85).unwrap();
        }
        start.elapsed() / 20
    };

    // Batch timing (20 iterations)
    let batch_time = {
        let start = Instant::now();
        for _ in 0..20 {
            pipeline.find_duplicates_batch(0.85).unwrap();
        }
        start.elapsed() / 20
    };

    let speedup = baseline_time.as_secs_f64() / batch_time.as_secs_f64();

    // Budget: 1.3-2.5× (strict enforcement)
    assert!(
        speedup >= 1.3,
        "Budget VIOLATED: {:.2}× < 1.3× minimum\nBaseline: {:?}\nBatch: {:?}",
        speedup,
        baseline_time,
        batch_time
    );

    assert!(speedup <= 2.5, "Speedup EXCESSIVE (suspicious): {:.2}×", speedup);

    println!(
        "✅ Budget PASSED: {:.2}× speedup (baseline: {:?}, batch: {:?})",
        speedup, baseline_time, batch_time
    );
}

// =============================================================================
// Backward Compatibility Tests (2 tests) - Q19/Q20: Rollback Validation
// =============================================================================

#[test]
fn test_baseline_always_available() {
    // Q20: Rollback validation - Baseline should always work
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(100, &cpu_caps);

    for i in 0..100 {
        pipeline.add_document(i, &format!("doc {}", i)).unwrap();
    }

    // Baseline should always be available (no feature gate)
    let clusters = pipeline.find_duplicates(0.85).unwrap();
    assert!(clusters.len() > 0, "Baseline should work");

    println!("✓ Baseline always available (rollback safety)");
}

#[test]
fn test_batch_zero_breaking_changes() {
    // Q19: Big Bang deployment - No API changes
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(100, &cpu_caps);

    for i in 0..100 {
        pipeline.add_document(i, &format!("doc {}", i)).unwrap();
    }

    // Both methods have identical signatures
    let _: Result<Vec<Vec<DocId>>, _> = pipeline.find_duplicates(0.85);
    let _: Result<Vec<Vec<DocId>>, _> = pipeline.find_duplicates_batch(0.85);

    // Both return same type (Result<Vec<Vec<DocId>>, PipelineError>)
    // Both accept same input (threshold: f64)
    // No breaking changes

    println!("✓ Zero breaking changes (backward compatible)");
}
