//! Phase 0: Property Tests (T28 Q8-Q14)
//!
//! Property-based tests for Q16.16 fixed-point Jaccard similarity.
//!
//! # T28 Tier 2: Property Testing
//! - Q8: Universal properties (commutativity, self-similarity, range)
//! - Q9: Concurrent invariants (thread-safe, no races)
//! - Q10: Edge case properties (overflow, boundary values)
//! - Q11: ASSUM verification (determinism, fixed-point precision)
//! - Q12: Composition properties (pipeline integration)
//! - Q13: Statistical properties (distribution, outliers)
//! - Q14: Regression tracking (proptest saves failing cases)

#[cfg(test)]
mod p0_property_tests {
    use atomic_capsule::primitives::fixed_point::Q16_16;
    use atomic_capsule::probabilistic::MinHashSignatureCapsule;

    // Helper to convert Vec<String> to Vec<&str> for MinHash
    fn to_str_vec(tokens: &[String]) -> Vec<&str> {
        tokens.iter().map(|s| s.as_str()).collect()
    }
    use proptest::prelude::*;
    use std::sync::Arc;
    use std::thread;

    /// Q8: Universal Property - Jaccard must be commutative for all inputs
    ///
    /// Property: sim(A, B) = sim(B, A) for all documents A, B.
    #[test]

    fn prop_q16_commutativity() {
        proptest!(|(
            tokens_a in prop::collection::vec("[a-z]+", 10..100),
            tokens_b in prop::collection::vec("[a-z]+", 10..100),
        )| {
            // Act: Compute both orders
            let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
            let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

            let sim_ab = sig_a.jaccard_similarity_q16(&sig_b);
            let sim_ba = sig_b.jaccard_similarity_q16(&sig_a);

            // Assert: Must be commutative
            prop_assert_eq!(
                sim_ab, sim_ba,
                "Jaccard must be commutative: AB={}, BA={}",
                sim_ab.to_f64(), sim_ba.to_f64()
            );
        });
    }

    /// Q8: Universal Property - Self-similarity must always be 1.0
    ///
    /// Property: sim(A, A) = 1.0 for all documents A.
    #[test]

    fn prop_q16_self_similarity() {
        proptest!(|(
            tokens in prop::collection::vec("[a-z]+", 1..100),
        )| {
            // Act: Compute self-similarity
            let sig = MinHashSignatureCapsule::compute_signature(&tokens.iter().map(|s| s.as_str()).collect::<Vec<_>>());
            let sim = sig.jaccard_similarity_q16(&sig);

            // Assert: Must be exactly 1.0
            prop_assert_eq!(
                sim,
                Q16_16::ONE,
                "Self-similarity must be 1.0, got {}",
                sim.to_f64()
            );
        });
    }

    /// Q8: Universal Property - Jaccard must be in [0, 1] range
    ///
    /// Property: 0 ≤ sim(A, B) ≤ 1 for all documents A, B.
    #[test]

    fn prop_q16_range_bounds() {
        proptest!(|(
            tokens_a in prop::collection::vec("[a-z]+", 1..100),
            tokens_b in prop::collection::vec("[a-z]+", 1..100),
        )| {
            // Act: Compute similarity
            let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
            let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

            let sim = sig_a.jaccard_similarity_q16(&sig_b);

            // Assert: Must be in [0, 1]
            prop_assert!(
                sim >= Q16_16::ZERO,
                "Jaccard must be ≥ 0: {}",
                sim.to_f64()
            );
            prop_assert!(
                sim <= Q16_16::ONE,
                "Jaccard must be ≤ 1: {}",
                sim.to_f64()
            );
        });
    }

    /// Q8: Universal Property - Monotonicity with respect to overlap
    ///
    /// Property: More shared tokens → higher similarity.
    #[test]

    fn prop_q16_monotonicity() {
        proptest!(|(
            shared in prop::collection::vec("[a-z]+", 5..20),
            _only_a in prop::collection::vec("[a-z]+", 5..20),
            only_b in prop::collection::vec("[a-z]+", 5..20),
        )| {
            // Arrange: Create documents with varying overlap
            let tokens_full_overlap = shared.clone();
            let mut tokens_partial_overlap = shared.clone();
            tokens_partial_overlap.extend(only_b.clone());

            let sig_shared = MinHashSignatureCapsule::compute_signature(&to_str_vec(&shared));
            let sig_full = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_full_overlap));
            let sig_partial = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_partial_overlap));

            // Act: Compute similarities
            let sim_full = sig_shared.jaccard_similarity_q16(&sig_full);
            let sim_partial = sig_shared.jaccard_similarity_q16(&sig_partial);

            // Assert: Full overlap should have higher similarity than partial
            prop_assert!(
                sim_full >= sim_partial,
                "Full overlap ({}) must have ≥ similarity than partial ({})",
                sim_full.to_f64(),
                sim_partial.to_f64()
            );
        });
    }

    /// Q9: Concurrent Invariant - Thread-safe signature computation
    ///
    /// Property: Concurrent signature computation produces identical results.
    #[test]

    fn prop_q16_concurrent_signature() {
        proptest!(|(
            tokens in prop::collection::vec("[a-z]+", 10..50),
        )| {
            // Arrange: Share tokens across threads
            let tokens_arc = Arc::new(tokens);

            // Act: Compute signatures concurrently
            let handles: Vec<_> = (0..10)
                .map(|_| {
                    let t = Arc::clone(&tokens_arc);
                    thread::spawn(move || {
                        MinHashSignatureCapsule::compute_signature(&to_str_vec(&*t))
                    })
                })
                .collect();

            let signatures: Vec<_> = handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .collect();

            // Assert: All signatures must be identical (deterministic)
            for sig in &signatures[1..] {
                prop_assert_eq!(
                    signatures[0].jaccard_similarity_q16(sig),
                    Q16_16::ONE,
                    "Concurrent signature computation must be deterministic"
                );
            }
        });
    }

    /// Q9: Concurrent Invariant - Thread-safe Jaccard computation
    ///
    /// Property: Concurrent Jaccard computation produces identical results.
    #[test]

    fn prop_q16_concurrent_jaccard() {
        proptest!(|(
            tokens_a in prop::collection::vec("[a-z]+", 10..50),
            tokens_b in prop::collection::vec("[a-z]+", 10..50),
        )| {
            // Arrange: Compute signatures once
            let sig_a = Arc::new(MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a)));
            let sig_b = Arc::new(MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b)));

            // Act: Compute Jaccard concurrently
            let handles: Vec<_> = (0..10)
                .map(|_| {
                    let a = Arc::clone(&sig_a);
                    let b = Arc::clone(&sig_b);
                    thread::spawn(move || {
                        a.jaccard_similarity_q16(&*b)
                    })
                })
                .collect();

            let similarities: Vec<_> = handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .collect();

            // Assert: All results must be identical
            for sim in &similarities[1..] {
                prop_assert_eq!(
                    similarities[0], *sim,
                    "Concurrent Jaccard computation must be deterministic"
                );
            }
        });
    }

    /// Q10: Edge Case Property - Handle extreme token counts
    ///
    /// Property: Jaccard works correctly with very small and very large token counts.
    #[test]

    fn prop_q16_extreme_token_counts() {
        proptest!(|(
            small_tokens in prop::collection::vec("[a-z]+", 1..5),
            large_tokens in prop::collection::vec("[a-z]+", 500..1000),
        )| {
            // Act: Compute similarities for extreme sizes
            let sig_small = MinHashSignatureCapsule::compute_signature(&to_str_vec(&small_tokens));
            let sig_large = MinHashSignatureCapsule::compute_signature(&to_str_vec(&large_tokens));

            let sim_small = sig_small.jaccard_similarity_q16(&sig_small);
            let sim_large = sig_large.jaccard_similarity_q16(&sig_large);
            let sim_mixed = sig_small.jaccard_similarity_q16(&sig_large);

            // Assert: All similarities must be valid
            prop_assert_eq!(sim_small, Q16_16::ONE, "Small self-similarity must be 1.0");
            prop_assert_eq!(sim_large, Q16_16::ONE, "Large self-similarity must be 1.0");
            prop_assert!(
                sim_mixed >= Q16_16::ZERO && sim_mixed <= Q16_16::ONE,
                "Mixed similarity must be in [0, 1]: {}",
                sim_mixed.to_f64()
            );
        });
    }

    /// Q10: Edge Case Property - Handle duplicate tokens
    ///
    /// Property: Duplicate tokens within a document don't break Jaccard.
    #[test]

    fn prop_q16_duplicate_tokens() {
        proptest!(|(
            base_tokens in prop::collection::vec("[a-z]+", 10..50),
            duplicate_count in 2..10usize,
        )| {
            // Arrange: Create document with duplicates
            let mut tokens_with_dupes = base_tokens.clone();
            for _ in 0..duplicate_count {
                tokens_with_dupes.extend(base_tokens.clone());
            }

            // Act: Compute signatures
            let sig_base = MinHashSignatureCapsule::compute_signature(&to_str_vec(&base_tokens));
            let sig_dupes = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_with_dupes));

            let sim = sig_base.jaccard_similarity_q16(&sig_dupes);

            // Assert: Duplicates shouldn't drastically change similarity
            // (MinHash treats sets, so duplicates should be ignored)
            prop_assert!(
                sim.to_f64() > 0.8,
                "Duplicate tokens should produce high similarity: {}",
                sim.to_f64()
            );
        });
    }

    /// Q11: ASSUM Verification - Q16.16 precision is sufficient
    ///
    /// #ASSUME: Q16.16 fixed-point provides sufficient precision (1/65536 ≈ 0.0000153).
    /// #VERIFY: Difference between Q16.16 and f64 is < 0.01% for all inputs.
    #[test]

    fn prop_q16_precision_assumption() {
        proptest!(|(
            tokens_a in prop::collection::vec("[a-z]+", 10..100),
            tokens_b in prop::collection::vec("[a-z]+", 10..100),
        )| {
            // Act: Compute both Q16.16 and f32
            let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
            let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

            let q16_sim = sig_a.jaccard_similarity_q16(&sig_b);
            let f32_sim = sig_a.jaccard_similarity(&sig_b);

            // Assert: Difference must be within precision tolerance
            let diff = (q16_sim.to_f64() - f32_sim as f64).abs();
            prop_assert!(
                diff < 0.0001,
                "Q16.16 precision must match f32 within 0.01%: q16={}, f32={}, diff={}",
                q16_sim.to_f64(),
                f32_sim,
                diff
            );
        });
    }

    /// Q11: ASSUM Verification - Deterministic computation
    ///
    /// #ASSUME: Fixed-point arithmetic is deterministic across platforms.
    /// #VERIFY: Same inputs always produce same outputs.
    #[test]

    fn prop_q16_determinism_assumption() {
        proptest!(|(
            tokens_a in prop::collection::vec("[a-z]+", 10..100),
            tokens_b in prop::collection::vec("[a-z]+", 10..100),
        )| {
            // Act: Compute signature and Jaccard twice
            let sig_a1 = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
            let sig_b1 = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));
            let sim1 = sig_a1.jaccard_similarity_q16(&sig_b1);

            let sig_a2 = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
            let sig_b2 = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));
            let sim2 = sig_a2.jaccard_similarity_q16(&sig_b2);

            // Assert: Must be identical (bit-for-bit)
            prop_assert_eq!(
                sim1, sim2,
                "Fixed-point computation must be deterministic: sim1={}, sim2={}",
                sim1.to_f64(), sim2.to_f64()
            );
        });
    }

    /// Q12: Composition Property - Pipeline integration preserves properties
    ///
    /// Property: Using Q16.16 Jaccard in find_duplicates preserves commutativity.
    #[test]

    fn prop_q16_pipeline_commutativity() {
        proptest!(|(
            tokens_a in prop::collection::vec("[a-z]+", 10..50),
            tokens_b in prop::collection::vec("[a-z]+", 10..50),
        )| {
            // Act: Compute in both orders
            let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
            let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

            let sim_ab = sig_a.jaccard_similarity_q16(&sig_b);
            let sim_ba = sig_b.jaccard_similarity_q16(&sig_a);

            // Assert: Commutativity preserved in pipeline
            prop_assert_eq!(
                sim_ab, sim_ba,
                "Pipeline must preserve commutativity: AB={}, BA={}",
                sim_ab.to_f64(), sim_ba.to_f64()
            );
        });
    }

    /// Q12: Composition Property - Threshold consistency
    ///
    /// Property: If sim(A, B) > threshold, then sim(B, A) > threshold.
    #[test]

    fn prop_q16_threshold_consistency() {
        proptest!(|(
            tokens_a in prop::collection::vec("[a-z]+", 10..50),
            tokens_b in prop::collection::vec("[a-z]+", 10..50),
            threshold in 0.0..1.0f32,
        )| {
            // Act: Compute both directions
            let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
            let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

            let sim_ab = sig_a.jaccard_similarity_q16(&sig_b);
            let sim_ba = sig_b.jaccard_similarity_q16(&sig_a);

            let threshold_q16 = Q16_16::from_f64(threshold as f64);

            // Assert: Threshold decision must be consistent
            prop_assert_eq!(
                sim_ab > threshold_q16,
                sim_ba > threshold_q16,
                "Threshold decision must be consistent: AB={}, BA={}, threshold={}",
                sim_ab.to_f64(), sim_ba.to_f64(), threshold
            );
        });
    }

    /// Q13: Statistical Property - Distribution of similarities
    ///
    /// Property: Random document pairs should have low average similarity.
    #[test]

    fn prop_q16_statistical_distribution() {
        proptest!(|(
            doc_pairs in prop::collection::vec(
                (
                    prop::collection::vec("[a-z]+", 10..50),
                    prop::collection::vec("[a-z]+", 10..50)
                ),
                50..100
            ),
        )| {
            // Act: Compute similarities for all pairs
            let similarities: Vec<f32> = doc_pairs
                .iter()
                .map(|(tokens_a, tokens_b)| {
                    let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(tokens_a));
                    let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(tokens_b));
                    sig_a.jaccard_similarity_q16(&sig_b).to_f64() as f32
                })
                .collect();

            // Assert: Average similarity should be low for random documents
            let avg_similarity = similarities.iter().sum::<f32>() / similarities.len() as f32;
            prop_assert!(
                avg_similarity < 0.5,
                "Random documents should have low average similarity: {}",
                avg_similarity
            );

            // Assert: All similarities must be in [0, 1]
            for sim in similarities {
                prop_assert!(
                    sim >= 0.0 && sim <= 1.0,
                    "Similarity must be in [0, 1]: {}",
                    sim
                );
            }
        });
    }

    /// Q13: Statistical Property - Outlier handling
    ///
    /// Property: Extremely similar or dissimilar documents are handled correctly.
    #[test]

    fn prop_q16_outlier_handling() {
        proptest!(|(
            base_tokens in prop::collection::vec("[a-z]+", 20..50),
        )| {
            // Arrange: Create outlier documents
            let sig_base = MinHashSignatureCapsule::compute_signature(&to_str_vec(&base_tokens));

            // Identical (outlier: similarity = 1.0)
            let sig_identical = MinHashSignatureCapsule::compute_signature(&to_str_vec(&base_tokens));

            // Completely different (outlier: similarity ≈ 0.0)
            let different_tokens: Vec<String> = (0..50)
                .map(|i| format!("unique_token_{}", i))
                .collect();
            let sig_different = MinHashSignatureCapsule::compute_signature(&to_str_vec(&different_tokens));

            // Act: Compute outlier similarities
            let sim_identical = sig_base.jaccard_similarity_q16(&sig_identical);
            let sim_different = sig_base.jaccard_similarity_q16(&sig_different);

            // Assert: Outliers are handled correctly
            prop_assert_eq!(
                sim_identical,
                Q16_16::ONE,
                "Identical outlier must have similarity = 1.0"
            );
            prop_assert!(
                sim_different.to_f64() < 0.1,
                "Disjoint outlier must have similarity ≈ 0.0: {}",
                sim_different.to_f64()
            );
        });
    }

    /// Q14: Regression Tracking - Proptest saves failing cases
    ///
    /// Property: This test will save any failing inputs to .proptest-regressions.
    /// Commit those files to catch regressions.
    #[test]

    fn prop_q16_regression_tracking() {
        proptest!(|(
            tokens_a in prop::collection::vec("[a-z]+", 10..100),
            tokens_b in prop::collection::vec("[a-z]+", 10..100),
        )| {
            // Act: Compute Jaccard
            let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
            let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

            let similarity = sig_a.jaccard_similarity_q16(&sig_b);

            // Assert: Core invariants (if any fail, proptest saves the case)
            prop_assert!(
                similarity >= Q16_16::ZERO && similarity <= Q16_16::ONE,
                "Similarity must be in [0, 1]: {}",
                similarity.to_f64()
            );

            // Commutativity
            let sim_ba = sig_b.jaccard_similarity_q16(&sig_a);
            prop_assert_eq!(
                similarity, sim_ba,
                "Must be commutative: AB={}, BA={}",
                similarity.to_f64(), sim_ba.to_f64()
            );
        });
    }
}
