//! Predictive Cache Property Tests - T28 Framework Q8-Q14
//!
//! **Testing Strategy**: Property-based testing for pattern learning correctness
//!
//! # T28 Q8-Q14: Property Tests
//!
//! - Q8: Pattern learning converges (repeated sequences → high confidence)
//! - Q9: Confidence correlates with frequency (more repetitions → higher confidence)
//! - Q10: Prediction accuracy improves with training data
//! - Q11: No false negatives (strong correlations always predicted)
//! - Q12: Bounded false positives (<10% of predictions)
//! - Q13: Eviction preserves strong correlations (LFU eviction)
//! - Q14: Concurrent updates preserve correctness

use clapi_core::capsules::pattern_learner::{
    PatternLearner256, PREFETCH_CONFIDENCE_THRESHOLD_BP,
};
use std::collections::HashMap;

// ============================================================================
// T28 Q8: Convergence Property
// ============================================================================

#[test]
fn property_pattern_learning_converges() {
    let learner = PatternLearner256::new();

    // Property: Repeated A→B sequence eventually reaches high confidence (>90%)
    let mut confidence_over_time = vec![];

    for i in 0..50 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x2222_2222_2222_2222);

        if i % 5 == 4 {
            // Sample confidence every 5 iterations
            let correlations = learner.get_top_correlations();
            if let Some((_, _, _, confidence)) = correlations
                .iter()
                .find(|(a, b, _, _)| *a == 0x1111_1111 && *b == 0x2222_2222)
            {
                confidence_over_time.push(*confidence);
            }
        }
    }

    // Property: Confidence monotonically increases (or stays constant)
    for i in 1..confidence_over_time.len() {
        assert!(
            confidence_over_time[i] >= confidence_over_time[i - 1],
            "Confidence should increase or stay constant: {} -> {}",
            confidence_over_time[i - 1],
            confidence_over_time[i]
        );
    }

    // Property: Final confidence is very high (>90% = 9000 bp)
    let final_confidence = *confidence_over_time.last().unwrap();
    assert!(
        final_confidence >= 9000,
        "After 50 repetitions, confidence should be >90%, got {}bp",
        final_confidence
    );
}

// ============================================================================
// T28 Q9: Frequency-Confidence Correlation
// ============================================================================

#[test]
fn property_confidence_correlates_with_frequency() {
    let learner = PatternLearner256::new();

    // Create patterns with different frequencies
    // Pattern A→B: 20 times (high frequency)
    // Pattern C→D: 5 times (low frequency)

    for _ in 0..20 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x2222_2222_2222_2222);
    }

    for _ in 0..5 {
        learner.record_request(0x3333_3333_3333_3333);
        learner.record_request(0x4444_4444_4444_4444);
    }

    let correlations = learner.get_top_correlations();

    // Find both correlations
    let ab_conf = correlations
        .iter()
        .find(|(a, b, _, _)| *a == 0x1111_1111 && *b == 0x2222_2222)
        .map(|(_, _, _, conf)| *conf)
        .expect("Should find A→B correlation");

    let cd_conf = correlations
        .iter()
        .find(|(a, b, _, _)| *a == 0x3333_3333 && *b == 0x4444_4444)
        .map(|(_, _, _, conf)| *conf)
        .expect("Should find C→D correlation");

    // Property: Higher frequency → higher confidence
    assert!(
        ab_conf > cd_conf,
        "A→B (20 reps) should have higher confidence than C→D (5 reps): {} vs {}",
        ab_conf,
        cd_conf
    );
}

// ============================================================================
// T28 Q10: Prediction Accuracy Improves with Training
// ============================================================================

#[test]
fn property_accuracy_improves_with_training() {
    let learner = PatternLearner256::new();

    // Measure prediction accuracy at different training sizes
    let training_sizes = [5, 10, 20, 40];
    let mut accuracies = vec![];

    for &size in &training_sizes {
        let test_learner = PatternLearner256::new();

        // Train with 'size' repetitions of A→B
        for _ in 0..size {
            test_learner.record_request(0x1111_1111_1111_1111);
            test_learner.record_request(0x2222_2222_2222_2222);
        }

        // Test prediction
        let predictions = test_learner.get_predictions(0x1111_1111_1111_1111);

        // Accuracy = 1.0 if predicted correctly, 0.0 otherwise
        let accuracy = if !predictions.is_empty()
            && (predictions[0].0 & 0xFFFF_FFFF) == 0x2222_2222
        {
            predictions[0].1 as f64 / 10000.0 // Confidence as accuracy
        } else {
            0.0
        };

        accuracies.push(accuracy);
    }

    // Property: Accuracy improves (or stays constant) with more training
    for i in 1..accuracies.len() {
        assert!(
            accuracies[i] >= accuracies[i - 1],
            "Accuracy should improve with training: {} -> {}",
            accuracies[i - 1],
            accuracies[i]
        );
    }

    println!("Accuracies: {:?}", accuracies);
}

// ============================================================================
// T28 Q11: No False Negatives (Strong Correlations Always Predicted)
// ============================================================================

#[test]
fn property_no_false_negatives() {
    let learner = PatternLearner256::new();

    // Build very strong correlation (50 repetitions → confidence ~100%)
    for _ in 0..50 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x2222_2222_2222_2222);
    }

    // Property: Strong correlation must be predicted
    let predictions = learner.get_predictions(0x1111_1111_1111_1111);

    assert!(
        !predictions.is_empty(),
        "Strong correlation (50 reps) should always be predicted (no false negatives)"
    );

    let (predicted_hash, confidence) = predictions[0];

    assert_eq!(
        predicted_hash & 0xFFFF_FFFF,
        0x2222_2222,
        "Predicted hash should match B"
    );

    assert!(
        confidence >= PREFETCH_CONFIDENCE_THRESHOLD_BP,
        "Confidence {} should be above threshold {}",
        confidence,
        PREFETCH_CONFIDENCE_THRESHOLD_BP
    );
}

// ============================================================================
// T28 Q12: Bounded False Positives (<10%)
// ============================================================================

#[test]
fn property_bounded_false_positives() {
    let learner = PatternLearner256::new();

    // Build mixed patterns with noise
    // Strong pattern: A→B (30 times)
    // Weak patterns: A→C, A→D, A→E (2 times each)

    for _ in 0..30 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x2222_2222_2222_2222);
    }

    for i in 3..6 {
        for _ in 0..2 {
            learner.record_request(0x1111_1111_1111_1111);
            learner.record_request(((i as u64) << 32) | 0x1111_1111);
        }
    }

    // Query predictions
    let predictions = learner.get_predictions(0x1111_1111_1111_1111);

    // Property: Only strong correlations predicted (weak ones filtered out)
    // False positive rate = weak predictions / total predictions

    let strong_predictions = predictions
        .iter()
        .filter(|(hash, _)| (*hash & 0xFFFF_FFFF) == 0x2222_2222)
        .count();

    let total_predictions = predictions.len();

    if total_predictions > 0 {
        let false_positive_rate = (total_predictions - strong_predictions) as f64
            / total_predictions as f64;

        println!(
            "False positive rate: {:.1}% ({}/{})",
            false_positive_rate * 100.0,
            total_predictions - strong_predictions,
            total_predictions
        );

        // Property: False positive rate <10%
        assert!(
            false_positive_rate < 0.10,
            "False positive rate {:.1}% should be <10%",
            false_positive_rate * 100.0
        );
    }
}

// ============================================================================
// T28 Q13: Eviction Preserves Strong Correlations
// ============================================================================

#[test]
fn property_lfu_eviction_preserves_strong_correlations() {
    let learner = PatternLearner256::new();

    // Build one very strong correlation (count=20)
    for _ in 0..20 {
        learner.record_request(0xFFFF_0000_1111_1111);
        learner.record_request(0xFFFF_0000_2222_2222);
    }

    // Fill all remaining slots with weak correlations (count=1)
    for i in 0..20 {
        let hash_a = ((i as u64) << 40) | 0xAAAA_AAAA;
        let hash_b = ((i as u64) << 40) | 0xBBBB_BBBB;
        learner.record_request(hash_a);
        learner.record_request(hash_b);
    }

    // Property: Strong correlation should still be present (not evicted)
    let correlations = learner.get_top_correlations();

    let strong_correlation_present = correlations
        .iter()
        .any(|(a, b, count, _)| {
            *a == 0xFFFF_0000 && *b == 0xFFFF_0000 && *count == 20
        });

    assert!(
        strong_correlation_present,
        "Strong correlation (count=20) should not be evicted in favor of weak ones (count=1)"
    );
}

// ============================================================================
// T28 Q14: Concurrent Updates Preserve Correctness
// ============================================================================

#[test]
fn property_concurrent_updates_correctness() {
    use std::sync::Arc;
    use std::thread;

    let learner = Arc::new(PatternLearner256::new());
    let mut handles = vec![];

    // Spawn 8 threads, each building same A→B correlation
    for _ in 0..8 {
        let learner_clone = Arc::clone(&learner);
        let handle = thread::spawn(move || {
            for _ in 0..50 {
                learner_clone.record_request(0x1111_1111_1111_1111);
                learner_clone.record_request(0x2222_2222_2222_2222);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    let stats = learner.get_stats();

    // Property: Total requests = 8 threads × 100 requests = 800
    assert_eq!(
        stats.total_requests, 800,
        "All concurrent requests should be counted"
    );

    // Property: A→B correlation should exist with high confidence
    let correlations = learner.get_top_correlations();
    let ab_correlation = correlations
        .iter()
        .find(|(a, b, _, _)| *a == 0x1111_1111 && *b == 0x2222_2222);

    assert!(
        ab_correlation.is_some(),
        "A→B correlation should be learned despite concurrent updates"
    );

    let (_, _, count, confidence) = ab_correlation.unwrap();

    // Property: Count should be ~400 (A→B appears in ~50% of pairs)
    // (8 threads × 50 reps = 400 A→B pairs)
    assert!(
        *count >= 390 && *count <= 410,
        "Count should be ~400, got {}",
        count
    );

    // Property: Confidence should be high (~50% since pattern alternates)
    assert!(
        *confidence >= 4500 && *confidence <= 5500,
        "Confidence should be ~50%, got {}bp",
        confidence
    );
}

// ============================================================================
// Additional Property Tests
// ============================================================================

#[test]
fn property_predictions_sorted_by_confidence() {
    let learner = PatternLearner256::new();

    // Build multiple correlations with different strengths
    // A→B: 30 times (strong)
    // A→C: 15 times (medium)
    // A→D: 5 times (weak)

    for _ in 0..30 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x2222_2222_2222_2222);
    }

    for _ in 0..15 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x3333_3333_3333_3333);
    }

    for _ in 0..5 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x4444_4444_4444_4444);
    }

    // Query predictions
    let predictions = learner.get_predictions(0x1111_1111_1111_1111);

    // Property: Predictions sorted by confidence (descending)
    for i in 1..predictions.len() {
        assert!(
            predictions[i - 1].1 >= predictions[i].1,
            "Predictions should be sorted by confidence: {} >= {}",
            predictions[i - 1].1,
            predictions[i].1
        );
    }

    // Property: Top prediction is strongest correlation (B)
    if !predictions.is_empty() {
        let (top_hash, _) = predictions[0];
        assert_eq!(
            top_hash & 0xFFFF_FFFF,
            0x2222_2222,
            "Top prediction should be B (strongest correlation)"
        );
    }
}

#[test]
fn property_reset_clears_all_state() {
    let learner = PatternLearner256::new();

    // Build correlations
    for _ in 0..50 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x2222_2222_2222_2222);
    }

    // Verify state exists
    let stats_before = learner.get_stats();
    assert!(stats_before.total_requests > 0);
    assert!(stats_before.unique_correlations > 0);

    // Reset
    learner.reset();

    // Property: Reset clears all state (equivalent to new learner)
    let stats_after = learner.get_stats();
    assert_eq!(stats_after.total_requests, 0);
    assert_eq!(stats_after.unique_correlations, 0);

    // Property: Predictions empty after reset
    let predictions = learner.get_predictions(0x1111_1111_1111_1111);
    assert!(predictions.is_empty(), "Predictions should be empty after reset");
}
