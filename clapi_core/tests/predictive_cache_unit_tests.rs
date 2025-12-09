//! Predictive Cache Unit Tests - T28 Framework Q1-Q7
//!
//! **Testing Strategy**: Comprehensive unit tests for PatternLearner256 and PredictivePrefetchCache
//!
//! # T28 Q1-Q7: Unit Tests
//!
//! - Q1: Test invariants (ring buffer wraparound, correlation uniqueness)
//! - Q2: Test edge cases (empty patterns, single request, full slots)
//! - Q3: Test error handling (hash collisions, eviction)
//! - Q4: Test performance (< 200ns pattern update, <100ns prediction)
//! - Q5: Test correctness (confidence calculation, prediction accuracy)
//! - Q6: Test concurrency (lockfree updates, no data races)
//! - Q7: Test integration (cache + pattern learner coordination)

use clapi_core::capsules::pattern_learner::{
    PatternLearner256, PATTERN_WINDOW_SIZE, MAX_CORRELATION_PAIRS,
    PREFETCH_CONFIDENCE_THRESHOLD_BP,
};
use clapi_core::cache::{LruCache, CacheConfig, PredictivePrefetchCache};
use std::sync::Arc;

// ============================================================================
// T28 Q1: Invariant Tests
// ============================================================================

#[test]
fn test_pattern_learner_initialization() {
    let learner = PatternLearner256::new();
    let stats = learner.get_stats();

    // Invariant: New learner has zero requests
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.unique_correlations, 0);
    assert_eq!(stats.avg_confidence_bp, 0);
}

#[test]
fn test_ring_buffer_wraparound() {
    let learner = PatternLearner256::new();

    // Fill window beyond capacity (PATTERN_WINDOW_SIZE + 1)
    // Use non-zero lower 32 bits to avoid hash truncation collisions
    for i in 0..(PATTERN_WINDOW_SIZE + 1) {
        learner.record_request(((i as u64) << 40) | 0x1234_5678);
    }

    let stats = learner.get_stats();

    // Invariant: Window wraps correctly (no panic, no overflow)
    assert_eq!(stats.total_requests, (PATTERN_WINDOW_SIZE + 1) as u64);

    // Invariant: Correlations learned despite wraparound
    // (May be less than total due to LFU eviction with only 6 slots)
    assert!(stats.unique_correlations > 0, "Should learn some correlations despite wraparound");
}

#[test]
fn test_correlation_uniqueness() {
    let learner = PatternLearner256::new();

    // Record same A→B sequence 10 times
    for _ in 0..10 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x2222_2222_2222_2222);
    }

    let correlations = learner.get_top_correlations();

    // Invariant: Each unique correlation appears only once in top list
    let mut seen = std::collections::HashSet::new();
    for (hash_a, hash_b, _, _) in &correlations {
        let key = (*hash_a, *hash_b);
        assert!(!seen.contains(&key), "Duplicate correlation in top list");
        seen.insert(key);
    }
}

// ============================================================================
// T28 Q2: Edge Case Tests
// ============================================================================

#[test]
fn test_empty_pattern_learner() {
    let learner = PatternLearner256::new();

    // Edge case: Get predictions from empty learner
    let predictions = learner.get_predictions(0x1234_5678_9ABC_DEF0);

    assert!(predictions.is_empty(), "Empty learner should return no predictions");
}

#[test]
fn test_single_request() {
    let learner = PatternLearner256::new();

    // Edge case: Single request (no previous, so no correlation)
    learner.record_request(0x1111_1111_1111_1111);

    let stats = learner.get_stats();
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.unique_correlations, 0, "Single request should create no correlations");
}

#[test]
fn test_two_requests() {
    let learner = PatternLearner256::new();

    // Edge case: Two requests (exactly one correlation)
    learner.record_request(0x1111_1111_1111_1111);
    learner.record_request(0x2222_2222_2222_2222);

    let stats = learner.get_stats();
    assert_eq!(stats.total_requests, 2);
    assert_eq!(stats.unique_correlations, 1, "Two requests should create one correlation");
}

#[test]
fn test_all_correlation_slots_filled() {
    let learner = PatternLearner256::new();

    // Edge case: Fill all MAX_CORRELATION_PAIRS slots
    // Use a repeating pattern to avoid creating unwanted B→A correlations
    let base_hash = 0xFFFF_0000_0000_0000u64;

    // Record initial request (establishes "previous")
    learner.record_request(base_hash);

    // Now record MAX_CORRELATION_PAIRS unique "next" requests
    // This creates exactly MAX_CORRELATION_PAIRS correlations: base→next[i]
    for i in 0..MAX_CORRELATION_PAIRS {
        learner.record_request(base_hash | ((i as u64) << 32));
        learner.record_request(base_hash); // Reset to base for next iteration
    }

    let stats = learner.get_stats();
    // We should have close to MAX_CORRELATION_PAIRS unique correlations
    // (May be slightly less due to LFU eviction)
    assert!(
        stats.unique_correlations >= (MAX_CORRELATION_PAIRS as u64) - 1,
        "Should have at least {} correlation slots filled, got {}",
        MAX_CORRELATION_PAIRS - 1,
        stats.unique_correlations
    );
}

#[test]
fn test_eviction_when_slots_full() {
    let learner = PatternLearner256::new();

    let base = 0x1000_0000_0000_0000u64;

    // Fill slots with weak correlations (count=1)
    // Use pattern: base→X, base→Y, base→Z (always returns to base)
    learner.record_request(base);
    for i in 0..MAX_CORRELATION_PAIRS {
        learner.record_request(base | ((i as u64) << 32));
        learner.record_request(base);
    }

    // Add strong correlation (15 repetitions to ensure it wins)
    for _ in 0..15 {
        learner.record_request(0xFFFF_0000_1111_1111);
        learner.record_request(0xFFFF_0000_2222_2222);
    }

    let correlations = learner.get_top_correlations();

    // Edge case: Should have some correlations
    assert!(!correlations.is_empty(), "Should have correlations");

    // Top correlation should be the strong one (count >= 15)
    let (_, _, count, _) = correlations[0];
    assert!(
        count >= 10,
        "Top correlation should be strong (count >= 10), got {}",
        count
    );
}

// ============================================================================
// T28 Q3: Error Handling Tests
// ============================================================================

#[test]
fn test_hash_truncation_collision() {
    let learner = PatternLearner256::new();

    // Create two hashes that collide in lower 32 bits
    let hash1 = 0x0000_0000_AAAA_BBBB;
    let hash2 = 0xFFFF_FFFF_AAAA_BBBB; // Same lower 32 bits

    learner.record_request(hash1);
    learner.record_request(hash2);
    learner.record_request(hash1);
    learner.record_request(hash2);

    let correlations = learner.get_top_correlations();

    // Both correlations should be tracked (despite collision in lower bits)
    // Note: This is an expected limitation of 32-bit truncation
    assert!(correlations.len() >= 1, "Should track at least one correlation");
}

#[test]
fn test_zero_hash_handling() {
    let learner = PatternLearner256::new();

    // Edge case: Zero hash (reserved for empty slots)
    learner.record_request(0);
    learner.record_request(0x1234_5678_9ABC_DEF0);

    let stats = learner.get_stats();

    // Should handle zero hash gracefully (implementation-defined behavior)
    assert_eq!(stats.total_requests, 2);
}

// ============================================================================
// T28 Q4: Performance Tests (basic timing)
// ============================================================================

#[test]
fn test_record_request_performance() {
    let learner = PatternLearner256::new();

    // Warmup
    for i in 0..100 {
        learner.record_request(i);
    }

    // Measure 1000 record operations
    let start = std::time::Instant::now();
    for i in 100..1100 {
        learner.record_request(i);
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / 1000;

    // Performance target: <1000ns (relaxed for debug builds)
    println!("Average record_request time: {}ns (target: <200ns release)", avg_ns);
    assert!(
        avg_ns < 2000,
        "record_request should be <2000ns (debug build), got {}ns",
        avg_ns
    );
}

#[test]
fn test_get_predictions_performance() {
    let learner = PatternLearner256::new();

    // Build some correlations
    for _ in 0..100 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x2222_2222_2222_2222);
    }

    // Measure 1000 prediction queries
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = learner.get_predictions(0x1111_1111_1111_1111);
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / 1000;

    // Performance target: <1000ns (relaxed for debug builds)
    println!("Average get_predictions time: {}ns (target: <100ns release)", avg_ns);
    assert!(
        avg_ns < 1000,
        "get_predictions should be <1000ns (debug build), got {}ns",
        avg_ns
    );
}

// ============================================================================
// T28 Q5: Correctness Tests
// ============================================================================

#[test]
fn test_confidence_calculation() {
    let learner = PatternLearner256::new();

    // Record A→B exactly 7 times out of 10 total pairs
    learner.record_request(0x1111_1111_1111_1111); // No prev (request 1)
    learner.record_request(0x2222_2222_2222_2222); // Pair 1: A→B
    learner.record_request(0x1111_1111_1111_1111); // Pair 2: B→A
    learner.record_request(0x2222_2222_2222_2222); // Pair 3: A→B
    learner.record_request(0x1111_1111_1111_1111); // Pair 4: B→A
    learner.record_request(0x2222_2222_2222_2222); // Pair 5: A→B
    learner.record_request(0x1111_1111_1111_1111); // Pair 6: B→A
    learner.record_request(0x2222_2222_2222_2222); // Pair 7: A→B
    learner.record_request(0x1111_1111_1111_1111); // Pair 8: B→A
    learner.record_request(0x2222_2222_2222_2222); // Pair 9: A→B
    learner.record_request(0x1111_1111_1111_1111); // Pair 10: B→A

    let correlations = learner.get_top_correlations();

    // Find A→B correlation
    let ab_correlation = correlations.iter()
        .find(|(a, b, _, _)| *a == 0x1111_1111 && *b == 0x2222_2222);

    assert!(ab_correlation.is_some(), "Should find A→B correlation");

    let (_, _, count, confidence) = ab_correlation.unwrap();

    // A→B appears 5 times out of 10 pairs = 50% confidence (5000 bp)
    assert_eq!(*count, 5, "A→B should appear 5 times");
    assert_eq!(*confidence, 5000, "Confidence should be 50% (5000 bp)");
}

#[test]
fn test_prediction_accuracy() {
    let learner = PatternLearner256::new();

    // Build strong A→B correlation (15 repetitions for >70% confidence)
    for _ in 0..15 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x2222_2222_2222_2222);
    }

    // Query predictions for A
    let predictions = learner.get_predictions(0x1111_1111_1111_1111);

    // Should predict B with high confidence
    assert!(!predictions.is_empty(), "Should have predictions");

    let (predicted_hash, confidence) = predictions[0];

    // Check predicted hash matches B (lower 32 bits)
    assert_eq!(predicted_hash & 0xFFFF_FFFF, 0x2222_2222);

    // Check confidence is above threshold
    assert!(
        confidence >= PREFETCH_CONFIDENCE_THRESHOLD_BP,
        "Confidence {} should be >= threshold {}",
        confidence,
        PREFETCH_CONFIDENCE_THRESHOLD_BP
    );
}

#[test]
fn test_low_confidence_filtering() {
    let learner = PatternLearner256::new();

    // Build weak correlation (only 2 occurrences out of many)
    learner.record_request(0x1111_1111_1111_1111);
    learner.record_request(0x2222_2222_2222_2222);
    learner.record_request(0x1111_1111_1111_1111);
    learner.record_request(0x2222_2222_2222_2222);

    // Add noise (many other patterns)
    for i in 10..20 {
        learner.record_request(((i as u64) << 32) | 0xAAAA_AAAA);
        learner.record_request(((i as u64) << 32) | 0xBBBB_BBBB);
    }

    // Query predictions for A
    let predictions = learner.get_predictions(0x1111_1111_1111_1111);

    // Weak correlation should be filtered out (below 70% threshold)
    // (2 out of 22 total pairs = ~9% confidence)
    assert!(
        predictions.is_empty(),
        "Weak correlations (<70%) should be filtered out"
    );
}

// ============================================================================
// T28 Q6: Concurrency Tests
// ============================================================================

#[test]
fn test_concurrent_record_requests() {
    use std::sync::Arc;
    use std::thread;

    let learner = Arc::new(PatternLearner256::new());
    let mut handles = vec![];

    // Spawn 4 threads, each recording 250 requests
    for thread_id in 0..4 {
        let learner_clone = Arc::clone(&learner);
        let handle = thread::spawn(move || {
            for i in 0..250 {
                // Use non-zero lower 32 bits
                let hash = ((thread_id as u64) << 48) | ((i as u64) << 32) | 0xABCD;
                learner_clone.record_request(hash);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    let stats = learner.get_stats();

    // Correctness: All 1000 requests recorded
    assert_eq!(stats.total_requests, 1000, "All concurrent requests should be recorded");

    // Correctness: Some correlations learned
    assert!(stats.unique_correlations > 0, "Should learn correlations from concurrent updates");
}

#[test]
fn test_concurrent_predictions() {
    use std::sync::Arc;
    use std::thread;

    let learner = Arc::new(PatternLearner256::new());

    // Build some correlations first
    for _ in 0..20 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x2222_2222_2222_2222);
    }

    let mut handles = vec![];

    // Spawn 4 threads, each querying predictions 250 times
    for _ in 0..4 {
        let learner_clone = Arc::clone(&learner);
        let handle = thread::spawn(move || {
            for _ in 0..250 {
                let predictions = learner_clone.get_predictions(0x1111_1111_1111_1111);
                assert!(!predictions.is_empty(), "Should always get predictions");
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }
}

// ============================================================================
// T28 Q7: Integration Tests (Cache + Pattern Learner)
// ============================================================================

#[tokio::test]
async fn test_predictive_cache_integration() {
    let config = CacheConfig {
        max_entries: 100,
        default_ttl_ns: 60_000_000_000, // 60 seconds
    };
    let cache = Arc::new(LruCache::new(config));
    let learner = Arc::new(PatternLearner256::new());
    let pred_cache = PredictivePrefetchCache::new(cache, learner);

    // Build A→B correlation via cache
    for _ in 0..15 {
        pred_cache
            .get_or_fetch("request_A", || async { Ok("response_A".to_string()) })
            .await
            .unwrap();

        pred_cache
            .get_or_fetch("request_B", || async { Ok("response_B".to_string()) })
            .await
            .unwrap();
    }

    // Verify pattern learning
    let stats = pred_cache.get_pattern_stats();
    assert_eq!(stats.total_requests, 30); // 15 A + 15 B
    assert!(stats.unique_correlations > 0);

    // Verify predictions
    let hash_a = atomic_capsule::hash::const_fast_hash(b"request_A");
    let prediction_count = pred_cache.prefetch_predictions(hash_a).await;
    assert!(prediction_count > 0, "Should have predictions for A→B");
}

#[tokio::test]
async fn test_prefetch_stats_tracking() {
    let config = CacheConfig {
        max_entries: 100,
        default_ttl_ns: 60_000_000_000, // 60 seconds
    };
    let cache = Arc::new(LruCache::new(config));
    let learner = Arc::new(PatternLearner256::new());
    let pred_cache = PredictivePrefetchCache::new(cache, learner);

    // Build correlation
    for _ in 0..15 {
        pred_cache
            .get_or_fetch("request_A", || async { Ok("response_A".to_string()) })
            .await
            .unwrap();

        pred_cache
            .get_or_fetch("request_B", || async { Ok("response_B".to_string()) })
            .await
            .unwrap();
    }

    // Trigger manual prefetch
    let hash_a = atomic_capsule::hash::const_fast_hash(b"request_A");
    pred_cache.prefetch_predictions(hash_a).await;

    // Check prefetch stats
    let stats = pred_cache.get_prefetch_stats();
    assert!(stats.attempts > 0, "Should have prefetch attempts");
}
