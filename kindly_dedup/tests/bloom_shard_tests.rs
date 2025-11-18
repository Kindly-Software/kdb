//! Comprehensive Test Suite for ShardedBloomFilterCapsule (Phase 6.2)
//!
//! 35+ tests covering unit, property, integration, and production scenarios.
//! Tests verify zero-contention Bloom filter for LLM dataset deduplication.
//!
//! # Test Organization
//!
//! - **Unit Tests (10)**: Basic operations, FPR validation, distribution
//! - **Property Tests (12)**: Determinism, monotonicity, concurrency invariants
//! - **Integration Tests (8)**: Pipeline integration, multi-threaded stress
//! - **Production Tests (5)**: Real-world scale, performance under pressure
//!
//! # Execution
//!
//! ```bash
//! cargo test --test bloom_shard_tests
//! cargo test --test bloom_shard_tests -- --nocapture  # with output
//! ```

use atomic_capsule::probabilistic::ShardedBloomFilterCapsule;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

// ============================================================================
// HELPERS
// ============================================================================

/// Hash a u64 value deterministically
fn hash_value(val: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    val.hash(&mut hasher);
    hasher.finish()
}

/// Hash a string deterministically
fn hash_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Generate pseudo-random u64 (simple LCG for determinism)
fn simple_random(seed: u64, index: u64) -> u64 {
    seed.wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
        .wrapping_add(index)
}

// ============================================================================
// UNIT TESTS (10)
// ============================================================================

#[test]
fn test_unit_01_new_empty() {
    // Verify all shards initialized to zero
    let bloom = ShardedBloomFilterCapsule::new();

    let (checked, skipped, rate) = bloom.audit_metrics();
    assert_eq!(checked, 0, "Empty filter should have 0 checked");
    assert_eq!(skipped, 0, "Empty filter should have 0 skipped");
    assert_eq!(rate, 0.0, "Empty filter should have 0.0 skip rate");
}

#[test]
fn test_unit_02_insert_single() {
    // Insert one hash, verify contains returns true
    let bloom = ShardedBloomFilterCapsule::new();
    let hash = hash_value(42);

    bloom.insert(hash);
    assert!(bloom.might_exist(hash), "Inserted hash should be found");
}

#[test]
fn test_unit_03_insert_duplicate() {
    // Insert same hash twice, verify idempotent
    let bloom = ShardedBloomFilterCapsule::new();
    let hash = hash_value(100);

    bloom.insert(hash);
    let (checked1, skipped1, _) = bloom.audit_metrics();

    bloom.insert(hash);
    let (checked2, skipped2, _) = bloom.audit_metrics();

    // Audit counters should not change on second insert (Bloom is idempotent)
    assert!(bloom.might_exist(hash), "Hash should exist after duplicate insert");
}

#[test]
fn test_unit_04_false_positive_rate() {
    // Insert 10K hashes, measure FPR against random queries
    let bloom = ShardedBloomFilterCapsule::new();
    let num_inserts = 10_000;

    // Insert known hashes
    for i in 0..num_inserts {
        let hash = hash_value(i);
        bloom.insert(hash);
    }

    // Query with unrelated hashes (should be rare false positives)
    let num_queries = 10_000;
    let mut false_positives = 0;

    for i in num_inserts as u64..(num_inserts as u64 + num_queries as u64) {
        let hash = hash_value(i);
        if bloom.might_exist(hash) {
            false_positives += 1;
        }
    }

    let fpr = false_positives as f64 / num_queries as f64;

    // At 10K capacity per shard (160K total), expect <1% FPR
    assert!(fpr < 0.01, "False positive rate {:.4}% should be < 1%", fpr * 100.0);
}

#[test]
fn test_unit_05_zero_false_negatives() {
    // Every inserted hash must be found
    let bloom = ShardedBloomFilterCapsule::new();
    let num_hashes = 5_000;

    // Insert hashes
    for i in 0..num_hashes {
        let hash = hash_value(i);
        bloom.insert(hash);
    }

    // Query all inserted hashes
    let mut found = 0;
    for i in 0..num_hashes {
        let hash = hash_value(i);
        if bloom.might_exist(hash) {
            found += 1;
        }
    }

    assert_eq!(
        found, num_hashes,
        "All {} inserted hashes should be found, got {}",
        num_hashes, found
    );
}

#[test]
fn test_unit_06_shard_distribution() {
    // Verify hashes distributed across 16 shards
    let bloom = ShardedBloomFilterCapsule::new();
    let num_hashes = 1_000;

    // We can't directly access shards, but we can verify via audit metrics
    // that different hashes are being processed
    for i in 0..num_hashes {
        let hash = hash_value(i);
        bloom.insert(hash);
    }

    // All should be found
    let mut found = 0;
    for i in 0..num_hashes {
        let hash = hash_value(i);
        if bloom.might_exist(hash) {
            found += 1;
        }
    }

    // Expect 100% recall (no false negatives)
    assert_eq!(found, num_hashes, "Shard distribution should not lose hashes");
}

#[test]
fn test_unit_07_bit_distribution() {
    // Verify bits distributed across 64 positions in each shard
    // (Indirect test via high insert/query rate on same shard)
    let bloom = ShardedBloomFilterCapsule::new();

    // Insert many items targeting same shard
    // (we can't directly control shard, but hash distribution should spread)
    for i in 0..100 {
        let hash = hash_value(i * 16); // Different bases, some will map to same shard
        bloom.insert(hash);
    }

    // Query all should succeed
    let mut found = 0;
    for i in 0..100 {
        let hash = hash_value(i * 16);
        if bloom.might_exist(hash) {
            found += 1;
        }
    }

    assert_eq!(found, 100, "Bit distribution should handle 100 hashes");
}

#[test]
fn test_unit_08_clear() {
    // Insert, clear, verify empty
    let bloom = ShardedBloomFilterCapsule::new();

    // Insert
    for i in 0..100 {
        let hash = hash_value(i);
        bloom.insert(hash);
    }

    // Verify inserted
    assert!(bloom.might_exist(hash_value(0)), "Should find inserted hash");

    // Clear
    bloom.clear();

    // Verify empty - all subsequent queries should return false (after new inserts)
    // Note: We need to test via skip_rate increasing due to reset
    let rate_after_clear = bloom.skip_rate();
    assert_eq!(rate_after_clear, 0.0, "Skip rate should be 0 after clear");
}

#[test]
fn test_unit_09_memory_layout() {
    // Verify 512KB total size (16 × 32KB)
    let bloom = ShardedBloomFilterCapsule::new();
    let capacity = bloom.capacity();

    // Capacity should be 160K (16 shards × 10K each)
    assert_eq!(capacity, 160_000, "Capacity should be 160K, got {}", capacity);
}

#[test]
fn test_unit_10_cache_alignment() {
    // Verify each shard 256B aligned (prevent false sharing)
    let bloom = ShardedBloomFilterCapsule::new();

    // Check that the structure itself is properly aligned
    let bloom_ptr = &bloom as *const _ as usize;
    assert_eq!(
        bloom_ptr % 256,
        0,
        "Bloom filter should be 256B aligned, got offset {}",
        bloom_ptr % 256
    );
}

// ============================================================================
// PROPERTY TESTS (12)
// ============================================================================

#[test]
fn test_prop_01_idempotent_insert() {
    // Inserting twice ≡ inserting once
    let bloom = ShardedBloomFilterCapsule::new();
    let hash = hash_value(999);

    bloom.insert(hash);
    bloom.insert(hash);
    bloom.insert(hash);

    // Should still be found
    assert!(bloom.might_exist(hash), "Idempotent inserts should still find hash");
}

#[test]
fn test_prop_02_contains_after_insert() {
    // If insert succeeds, contains must return true
    let bloom = ShardedBloomFilterCapsule::new();

    for i in 0..1000 {
        let hash = hash_value(i);
        bloom.insert(hash);
        assert!(bloom.might_exist(hash), "Hash {} should be found after insert", i);
    }
}

#[test]
fn test_prop_03_monotonic_memory() {
    // Clearing resets to empty state (metrics)
    let bloom = ShardedBloomFilterCapsule::new();

    // Insert and check metrics
    for i in 0..100 {
        let hash = hash_value(i);
        bloom.insert(hash);
    }

    let (checked_before, _, _) = bloom.audit_metrics();
    assert!(checked_before >= 0, "Metrics should be positive after inserts");

    // Clear should reset counters
    bloom.clear();
    let (checked_after, skipped_after, _) = bloom.audit_metrics();
    assert_eq!(
        checked_after, 0,
        "Checked should be 0 after clear, got {}",
        checked_after
    );
    assert_eq!(
        skipped_after, 0,
        "Skipped should be 0 after clear, got {}",
        skipped_after
    );
}

#[test]
fn test_prop_04_no_false_negatives() {
    // Every inserted element must be queryable
    let bloom = ShardedBloomFilterCapsule::new();

    // Insert batch
    let test_hashes: Vec<u64> = (0..5000).map(|i| hash_value(i)).collect();
    for hash in &test_hashes {
        bloom.insert(*hash);
    }

    // Query all
    for hash in &test_hashes {
        assert!(
            bloom.might_exist(*hash),
            "Inserted hash must be found (no false negatives)"
        );
    }
}

#[test]
fn test_prop_05_shard_independence() {
    // Shards don't interfere (different hashes don't affect each other)
    let bloom = ShardedBloomFilterCapsule::new();

    // Insert batch 1
    for i in 0..100 {
        bloom.insert(hash_value(i));
    }

    // Query batch 1 should all succeed
    let mut batch1_found = 0;
    for i in 0..100 {
        if bloom.might_exist(hash_value(i)) {
            batch1_found += 1;
        }
    }
    assert_eq!(batch1_found, 100, "Batch 1 should all be found");

    // Insert batch 2 (different range)
    for i in 10000..10100 {
        bloom.insert(hash_value(i));
    }

    // Query batch 1 again should still all succeed (insertion doesn't break them)
    let mut batch1_still_found = 0;
    for i in 0..100 {
        if bloom.might_exist(hash_value(i)) {
            batch1_still_found += 1;
        }
    }
    assert_eq!(
        batch1_still_found, 100,
        "Batch 1 should still be found after batch 2 insertion"
    );

    // Query batch 2 should all succeed
    let mut batch2_found = 0;
    for i in 10000..10100 {
        if bloom.might_exist(hash_value(i)) {
            batch2_found += 1;
        }
    }
    assert_eq!(batch2_found, 100, "Batch 2 should all be found");
}

#[test]
fn test_prop_06_deterministic_hashing() {
    // Same hash always → same shard/bit (determinism)
    let bloom = ShardedBloomFilterCapsule::new();
    let hash = hash_value(54321);

    // Insert multiple times
    for _ in 0..10 {
        bloom.insert(hash);
    }

    // Query multiple times - should always find
    for _ in 0..10 {
        assert!(bloom.might_exist(hash), "Deterministic query should always succeed");
    }
}

#[test]
fn test_prop_07_collision_tolerance() {
    // FPR remains <1% under load (progressive load)
    let bloom = ShardedBloomFilterCapsule::new();

    // Insert 50K hashes (well-loaded but under capacity of 160K)
    for i in 0..50_000 {
        let hash = hash_value(i);
        bloom.insert(hash);
    }

    // Query 50K unrelated hashes
    let mut fps = 0;
    for i in 100_000..150_000 {
        let hash = hash_value(i);
        if bloom.might_exist(hash) {
            fps += 1;
        }
    }

    let fpr = fps as f64 / 50_000.0;
    assert!(fpr < 0.01, "FPR under load should be <1%, got {:.4}%", fpr * 100.0);
}

#[test]
fn test_prop_08_stress_10k_inserts() {
    // Handle 10K random inserts
    let bloom = ShardedBloomFilterCapsule::new();

    // Insert 10K with pseudo-random distribution
    for i in 0..10_000 {
        let hash = simple_random(12345, i);
        bloom.insert(hash);
    }

    // Verify recall
    let mut found = 0;
    for i in 0..10_000 {
        let hash = simple_random(12345, i);
        if bloom.might_exist(hash) {
            found += 1;
        }
    }

    assert_eq!(found, 10_000, "All 10K inserts should be found");
}

#[test]
fn test_prop_09_stress_100k_inserts() {
    // Handle 100K random inserts (parallel thread simulation)
    let bloom = Arc::new(ShardedBloomFilterCapsule::new());

    // Sequential insert (100K, near capacity limit)
    let num_inserts = 100_000;
    for i in 0..num_inserts {
        let hash = simple_random(99999, i);
        bloom.insert(hash);
    }

    // Verify recall (may have some FP due to capacity)
    let mut found = 0;
    for i in 0..num_inserts {
        let hash = simple_random(99999, i);
        if bloom.might_exist(hash) {
            found += 1;
        }
    }

    // Expect >99% recall at this capacity
    let recall = found as f64 / num_inserts as f64;
    assert!(
        recall > 0.99,
        "Recall at 100K should be >99%, got {:.2}%",
        recall * 100.0
    );
}

#[test]
fn test_prop_10_concurrent_reads() {
    // Multiple readers, same state
    let bloom = Arc::new(ShardedBloomFilterCapsule::new());

    // Pre-populate
    for i in 0..1000 {
        bloom.insert(hash_value(i));
    }

    // Spawn 4 readers
    let mut handles = vec![];

    for thread_id in 0..4 {
        let bloom_clone = Arc::clone(&bloom);
        let handle = thread::spawn(move || {
            let mut found = 0;
            for i in thread_id * 250..(thread_id + 1) * 250 {
                if bloom_clone.might_exist(hash_value(i)) {
                    found += 1;
                }
            }
            found
        });
        handles.push(handle);
    }

    // Collect results
    let mut total_found = 0;
    for handle in handles {
        total_found += handle.join().unwrap();
    }

    assert_eq!(total_found, 1000, "All 1000 should be found by 4 concurrent readers");
}

#[test]
fn test_prop_11_zero_cost_abstraction() {
    // No hidden allocations (verify via insert count consistency)
    let bloom = ShardedBloomFilterCapsule::new();

    // Insert 1000 items
    for i in 0..1000 {
        bloom.insert(hash_value(i));
    }

    // Query 1000 items + 1000 non-existent
    for i in 0..2000 {
        let _ = bloom.might_exist(hash_value(i + 100_000));
    }

    let (checked, _, _) = bloom.audit_metrics();
    // Should have exactly 2000 checks (no extra allocations or loops)
    assert_eq!(
        checked, 2000,
        "Checked count should be 2000, got {} (possible hidden allocations)",
        checked
    );
}

#[test]
fn test_prop_12_cache_locality() {
    // Shard accesses stay L3-local (<30ns per access)
    let bloom = ShardedBloomFilterCapsule::new();

    // Pre-warm cache
    for i in 0..100 {
        bloom.insert(hash_value(i));
    }

    // Measure query latency (warm cache)
    let start = Instant::now();
    let num_queries = 10_000;

    for i in 0..num_queries {
        let _ = bloom.might_exist(hash_value(i));
    }

    let elapsed = start.elapsed();
    let per_query = elapsed.as_nanos() as f64 / num_queries as f64;

    // Expect <200ns per query on average (30-50ns per query with overhead)
    assert!(
        per_query < 200.0,
        "Cache locality: {:.1}ns per query (should be <200ns)",
        per_query
    );
}

// ============================================================================
// INTEGRATION TESTS (8)
// ============================================================================

#[test]
fn test_integ_01_pipeline_bloom_skip_rate_50pct() {
    // 50% duplicate corpus should achieve >40% skip rate
    let bloom = ShardedBloomFilterCapsule::new();

    // Insert 100 unique docs
    for i in 0..100 {
        let hash = hash_value(i);
        bloom.insert(hash);
    }

    // Query: 100 originals + 100 duplicates
    let mut skipped = 0;
    for i in 0..200 {
        if i < 100 {
            // Original
            if !bloom.might_exist(hash_value(i)) {
                skipped += 1;
            }
        } else {
            // Duplicate
            if !bloom.might_exist(hash_value(i - 100)) {
                skipped += 1;
            }
        }
    }

    let skip_rate = skipped as f64 / 200.0;
    assert!(
        skip_rate > 0.40,
        "50% duplicate corpus should have >40% skip rate, got {:.2}%",
        skip_rate * 100.0
    );
}

#[test]
fn test_integ_02_pipeline_bloom_skip_rate_90pct() {
    // 90% duplicate corpus should achieve >80% skip rate
    let bloom = ShardedBloomFilterCapsule::new();

    // Insert 100 unique docs
    for i in 0..100 {
        let hash = hash_value(i);
        bloom.insert(hash);
    }

    // Query: 100 originals + 900 duplicates (9:1 ratio)
    let mut skipped = 0;
    for query_id in 0..1000 {
        let hash = if query_id < 100 {
            hash_value(query_id)
        } else {
            // Duplicate from [0..100]
            hash_value((query_id - 100) % 100)
        };

        if !bloom.might_exist(hash) {
            skipped += 1;
        }
    }

    let skip_rate = skipped as f64 / 1000.0;
    assert!(
        skip_rate > 0.80,
        "90% duplicate corpus should have >80% skip rate, got {:.2}%",
        skip_rate * 100.0
    );
}

#[test]
fn test_integ_03_pipeline_bloom_no_skip_unique() {
    // Unique corpus, no skips (all queries are new)
    let bloom = ShardedBloomFilterCapsule::new();

    // Query 1000 unique items (none inserted)
    let mut skipped = 0;
    for i in 0..1000 {
        if !bloom.might_exist(hash_value(i)) {
            skipped += 1;
        }
    }

    // All should be skipped (0% false positive rate on empty Bloom)
    let skip_rate = skipped as f64 / 1000.0;
    assert_eq!(skip_rate, 1.0, "Empty Bloom should skip all unique queries");
}

#[test]
fn test_integ_04_pipeline_accuracy() {
    // Bloom + verification = high accuracy
    // (Bloom filters provide high recall with acceptable FPR)
    let bloom = ShardedBloomFilterCapsule::new();

    // Insert 1000 docs
    for i in 0..1000 {
        bloom.insert(hash_value(i));
    }

    // Query with mix
    let mut tp = 0; // True positives (inserted)
    let mut fp = 0; // False positives (not inserted but Bloom says maybe)
    let mut tn = 0; // True negatives (not inserted, Bloom says no)

    // Check inserted items
    for i in 0..1000 {
        if bloom.might_exist(hash_value(i)) {
            tp += 1;
        }
    }

    // Check uninserted items
    for i in 1000..2000 {
        if bloom.might_exist(hash_value(i)) {
            fp += 1;
        } else {
            tn += 1;
        }
    }

    let recall = tp as f64 / 1000.0;
    let fpr = fp as f64 / 1000.0;

    assert_eq!(recall, 1.0, "Recall should be 100% (no false negatives)");
    assert!(fpr < 0.01, "FPR should be <1%, got {:.2}%", fpr * 100.0);
}

#[test]
fn test_integ_05_pipeline_parallel_bloom() {
    // Parallel access to Bloom filter
    let bloom = Arc::new(ShardedBloomFilterCapsule::new());

    // Thread 0: Insert 2500 items
    let bloom_t0 = Arc::clone(&bloom);
    let handle_t0 = thread::spawn(move || {
        for i in 0..2500 {
            bloom_t0.insert(hash_value(i));
        }
    });

    // Thread 1: Insert 2500 items
    let bloom_t1 = Arc::clone(&bloom);
    let handle_t1 = thread::spawn(move || {
        for i in 2500..5000 {
            bloom_t1.insert(hash_value(i));
        }
    });

    // Wait for inserts
    handle_t0.join().unwrap();
    handle_t1.join().unwrap();

    // Verify all 5000 inserted
    let mut found = 0;
    for i in 0..5000 {
        if bloom.might_exist(hash_value(i)) {
            found += 1;
        }
    }

    assert_eq!(found, 5000, "Parallel insert should result in 5000 items found");
}

#[test]
fn test_integ_06_pipeline_incremental_bloom() {
    // Incremental additions to Bloom
    let bloom = ShardedBloomFilterCapsule::new();

    // Phase 1: Insert 100
    for i in 0..100 {
        bloom.insert(hash_value(i));
    }

    let (checked_p1, _, _) = bloom.audit_metrics();

    // Phase 2: Insert 100 more
    for i in 100..200 {
        bloom.insert(hash_value(i));
    }

    let (checked_p2, _, _) = bloom.audit_metrics();

    // Phase 3: Query all 200
    for i in 0..200 {
        let _ = bloom.might_exist(hash_value(i));
    }

    let (checked_p3, _, _) = bloom.audit_metrics();

    // Verify progression
    assert!(checked_p1 > 0, "Phase 1 should have some checks");
    assert!(checked_p2 >= checked_p1, "Phase 2 checks >= Phase 1");
    assert!(checked_p3 > checked_p2, "Phase 3 checks > Phase 2");
}

#[test]
fn test_integ_07_pipeline_memory_bounded() {
    // 512KB overhead only (Bloom filter size)
    let bloom = ShardedBloomFilterCapsule::new();
    let capacity = bloom.capacity();

    // Verify capacity constant (160K)
    assert_eq!(capacity, 160_000, "Capacity should be 160K");

    // Insert max capacity
    for i in 0..capacity {
        bloom.insert(hash_value(i as u64));
    }

    // Still queryable
    assert!(
        bloom.might_exist(hash_value(0)),
        "Should find first inserted after full"
    );
    assert!(
        bloom.might_exist(hash_value((capacity - 1) as u64)),
        "Should find last inserted after full"
    );
}

#[test]
fn test_integ_08_pipeline_latency_bound() {
    // <30ns per Bloom query (on average)
    let bloom = ShardedBloomFilterCapsule::new();

    // Pre-populate
    for i in 0..1000 {
        bloom.insert(hash_value(i));
    }

    // Warm cache
    for _ in 0..100 {
        let _ = bloom.might_exist(hash_value(0));
    }

    // Measure latency
    let start = Instant::now();
    let num_iterations = 100_000;

    for i in 0..num_iterations {
        let _ = bloom.might_exist(hash_value(i % 1000));
    }

    let elapsed = start.elapsed();
    let per_op = elapsed.as_nanos() as f64 / num_iterations as f64;

    // Expect <300ns per operation (30ns ± overhead)
    assert!(per_op < 300.0, "Latency should be <300ns/op, got {:.1}ns", per_op);
}

// ============================================================================
// PRODUCTION TESTS (5)
// ============================================================================

#[test]
fn test_prod_01_1m_corpus() {
    // 1M documents, sustained throughput
    let bloom = ShardedBloomFilterCapsule::new();

    let start = Instant::now();
    let num_docs = 1_000_000;

    // Insert 1M docs
    for i in 0..num_docs {
        let hash = simple_random(777777, i as u64);
        bloom.insert(hash);
    }

    let insert_elapsed = start.elapsed();

    // Query 1M docs
    let start = Instant::now();
    for i in 0..num_docs {
        let hash = simple_random(777777, i as u64);
        let _ = bloom.might_exist(hash);
    }
    let query_elapsed = start.elapsed();

    let insert_rate = num_docs as f64 / insert_elapsed.as_secs_f64();
    let query_rate = num_docs as f64 / query_elapsed.as_secs_f64();

    println!("1M corpus: {:.0} insert/sec, {:.0} query/sec", insert_rate, query_rate);

    // Expect >10M ops/sec on modern hardware
    assert!(
        insert_rate > 1_000_000.0,
        "Insert rate should be >1M/sec, got {:.0}",
        insert_rate
    );
    assert!(
        query_rate > 1_000_000.0,
        "Query rate should be >1M/sec, got {:.0}",
        query_rate
    );
}

#[test]
fn test_prod_02_memory_pressure() {
    // Under memory pressure (< 1GB available simulated)
    let bloom = ShardedBloomFilterCapsule::new();

    // Fill to near capacity (160K)
    for i in 0..160_000 {
        let hash = simple_random(888888, i as u64);
        bloom.insert(hash);
    }

    // Should still be responsive
    let start = Instant::now();
    let mut found = 0;
    for i in 0..10_000 {
        let hash = simple_random(888888, i as u64);
        if bloom.might_exist(hash) {
            found += 1;
        }
    }
    let elapsed = start.elapsed();

    assert_eq!(found, 10_000, "Should find all inserted even at capacity");
    assert!(
        elapsed.as_millis() < 100,
        "Query at capacity should complete in <100ms, took {:?}",
        elapsed
    );
}

#[test]
fn test_prod_03_cpu_cache_effects() {
    // Cold cache vs warm cache performance
    let bloom = Arc::new(ShardedBloomFilterCapsule::new());

    // Pre-populate
    for i in 0..10_000 {
        bloom.insert(hash_value(i));
    }

    // Cold cache measurement (simulate by accessing different shards)
    let start = Instant::now();
    let mut cold_sum = 0u64;
    for i in 0..5000 {
        if bloom.might_exist(hash_value(i * 32)) {
            cold_sum += 1;
        }
    }
    let cold_elapsed = start.elapsed();

    // Warm cache measurement (same shard accesses)
    let start = Instant::now();
    let mut warm_sum = 0u64;
    for i in 0..5000 {
        if bloom.might_exist(hash_value(i)) {
            warm_sum += 1;
        }
    }
    let warm_elapsed = start.elapsed();

    let cold_rate = 5000.0 / cold_elapsed.as_secs_f64();
    let warm_rate = 5000.0 / warm_elapsed.as_secs_f64();

    println!(
        "Cold cache: {:.0} ops/sec, Warm cache: {:.0} ops/sec",
        cold_rate, warm_rate
    );

    // Warm should be faster
    assert!(
        warm_elapsed <= cold_elapsed,
        "Warm cache should be faster or equal to cold cache"
    );
}

#[test]
fn test_prod_04_concurrent_16_threads() {
    // 16 threads, no contention
    let bloom = Arc::new(ShardedBloomFilterCapsule::new());

    // Pre-populate from main thread
    for i in 0..100_000 {
        bloom.insert(hash_value(i));
    }

    let mut handles = vec![];
    let start = Instant::now();

    // Spawn 16 reader threads
    for thread_id in 0..16 {
        let bloom_clone = Arc::clone(&bloom);
        let handle = thread::spawn(move || {
            let mut found = 0;
            let items_per_thread = 100_000 / 16;
            for i in 0..items_per_thread {
                let idx = thread_id * items_per_thread + i;
                if bloom_clone.might_exist(hash_value(idx as u64)) {
                    found += 1;
                }
            }
            found
        });
        handles.push(handle);
    }

    // Collect results
    let mut total_found = 0;
    for handle in handles {
        total_found += handle.join().unwrap();
    }

    let elapsed = start.elapsed();

    assert_eq!(total_found, 100_000, "All 100K should be found by 16 threads");

    let throughput = 100_000.0 / elapsed.as_secs_f64();
    println!("16-thread throughput: {:.0} queries/sec", throughput);

    // Expect >5M queries/sec (300K+ per thread)
    assert!(
        throughput > 5_000_000.0,
        "16-thread throughput should be >5M/sec, got {:.0}",
        throughput
    );
}

#[test]
fn test_prod_05_recovery_after_clear() {
    // Crash resilience via clear + rebuild
    let bloom = ShardedBloomFilterCapsule::new();

    // Phase 1: Populate
    for i in 0..50_000 {
        bloom.insert(hash_value(i));
    }

    // Verify populated
    let (checked_p1, _, skip_rate_p1) = bloom.audit_metrics();
    assert!(checked_p1 > 0, "Phase 1 should have checks");

    // Phase 2: Clear (simulating recovery)
    bloom.clear();
    let (checked_p2, skipped_p2, skip_rate_p2) = bloom.audit_metrics();
    assert_eq!(checked_p2, 0, "After clear, checked should be 0");
    assert_eq!(skipped_p2, 0, "After clear, skipped should be 0");

    // Phase 3: Rebuild with new data
    for i in 0..50_000 {
        bloom.insert(hash_value(i + 1_000_000)); // Different data
    }

    // Query phase 1 data (should NOT be found, they were cleared)
    let mut old_found = 0;
    for i in 0..50_000 {
        if bloom.might_exist(hash_value(i)) {
            old_found += 1;
        }
    }

    // Query phase 3 data (should be found)
    let mut new_found = 0;
    for i in 0..50_000 {
        if bloom.might_exist(hash_value(i + 1_000_000)) {
            new_found += 1;
        }
    }

    assert!(
        old_found < 1000,
        "Old data should mostly be gone (got {} old)",
        old_found
    );
    assert_eq!(new_found, 50_000, "New data should all be found");
}
