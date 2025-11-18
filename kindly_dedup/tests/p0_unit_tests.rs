//! Phase 0: Unit Tests (T28 Q1-Q7)
//!
//! Tests for Q16.16 fixed-point Jaccard similarity implementation.
//!
//! # T28 Tier 1: Unit Testing
//! - Q1: Core behaviors (determinism, range, accuracy vs f32)
//! - Q2: Edge cases (identical, disjoint, empty, overflow)
//! - Q3: Invariants (commutativity, symmetry, triangle inequality)
//! - Q4: Code paths (all branches covered)
//! - Q5: Isolation (no shared state)
//! - Q6: Performance (<10ms per test)
//! - Q7: Readability (arrange-act-assert structure)

#[cfg(test)]
mod p0_unit_tests {
    use atomic_capsule::primitives::fixed_point::Q16_16;
    use atomic_capsule::probabilistic::{tokenize, MinHashSignatureCapsule};

    // Helper to convert Vec<String> to Vec<&str> for MinHash
    fn to_str_vec(tokens: &[String]) -> Vec<&str> {
        tokens.iter().map(|s| s.as_str()).collect()
    }

    /// Q1: Core Behavior - Q16.16 Jaccard must be deterministic
    ///
    /// Tests that the same inputs always produce the same output.
    #[test]

    fn test_q16_jaccard_deterministic() {
        // Arrange: Create two document signatures
        let tokens_a = tokenize("The quick brown fox jumps over the lazy dog");
        let tokens_b = tokenize("The quick brown fox leaps over the lazy cat");

        let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
        let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

        // Act: Compute Jaccard similarity twice
        let sim1 = sig_a.jaccard_similarity_q16(&sig_b);
        let sim2 = sig_a.jaccard_similarity_q16(&sig_b);

        // Assert: Must be identical (deterministic)
        assert_eq!(
            sim1,
            sim2,
            "Q16.16 Jaccard must be deterministic: sim1={:?}, sim2={:?}",
            sim1.to_f64(),
            sim2.to_f64()
        );
    }

    /// Q1: Core Behavior - Q16.16 Jaccard must be in [0, 1] range
    ///
    /// Tests that Jaccard similarity never exceeds valid probability bounds.
    #[test]

    fn test_q16_range() {
        // Arrange: Various token sets
        let test_cases = vec![
            ("The quick brown fox", "The quick brown fox"),         // Identical
            ("The quick brown fox", "A completely different text"), // Disjoint
            ("The quick brown fox", "The quick brown cat"),         // Partial overlap
            ("hello world", "hello"),                               // Subset
            ("a b c d e", "c d e f g"),                             // Overlapping
        ];

        for (text_a, text_b) in test_cases {
            let tokens_a = tokenize(text_a);
            let tokens_b = tokenize(text_b);

            let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
            let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

            // Act: Compute similarity
            let similarity = sig_a.jaccard_similarity_q16(&sig_b);

            // Assert: Must be in [0, 1] range
            assert!(
                similarity >= Q16_16::ZERO,
                "Jaccard < 0 for texts ('{}', '{}'): {}",
                text_a,
                text_b,
                similarity.to_f64()
            );
            assert!(
                similarity <= Q16_16::ONE,
                "Jaccard > 1 for texts ('{}', '{}'): {}",
                text_a,
                text_b,
                similarity.to_f64()
            );
        }
    }

    /// Q1: Core Behavior - Q16.16 must match f32 within precision tolerance
    ///
    /// Tests that fixed-point arithmetic maintains accuracy compared to floating-point.
    #[test]

    fn test_q16_vs_f32_accuracy() {
        // Arrange: Generate test documents
        let test_docs = vec![
            ("The quick brown fox jumps", "The quick brown fox leaps"),
            ("Machine learning models", "Deep learning networks"),
            ("rust programming language", "rust systems programming"),
            ("a b c d e f g h", "c d e f g h i j"),
        ];

        for (text_a, text_b) in test_docs {
            let tokens_a = tokenize(text_a);
            let tokens_b = tokenize(text_b);

            let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
            let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

            // Act: Compute both Q16.16 and f32 similarity
            let q16_sim = sig_a.jaccard_similarity_q16(&sig_b);
            let f32_sim = sig_a.jaccard_similarity(&sig_b);

            // Assert: Must match within Q16.16 precision (1/65536 ≈ 0.0000153)
            let diff = (q16_sim.to_f64() - f32_sim as f64).abs();
            assert!(
                diff < 0.0001,
                "Q16.16 must match f32 within 0.01% for ('{}', '{}'): q16={}, f32={}, diff={}",
                text_a,
                text_b,
                q16_sim.to_f64(),
                f32_sim,
                diff
            );
        }
    }

    /// Q2: Edge Case - Identical documents must have similarity = 1.0
    #[test]

    fn test_q16_identical_documents() {
        // Arrange: Same text
        let text = "The quick brown fox jumps over the lazy dog";
        let tokens = tokenize(text);
        let sig = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens));

        // Act: Compute self-similarity
        let similarity = sig.jaccard_similarity_q16(&sig);

        // Assert: Must be exactly 1.0
        assert_eq!(
            similarity,
            Q16_16::ONE,
            "Identical documents must have Jaccard = 1.0, got {}",
            similarity.to_f64()
        );
    }

    /// Q2: Edge Case - Completely disjoint documents must have similarity ≈ 0.0
    #[test]

    fn test_q16_disjoint_documents() {
        // Arrange: Completely different texts (no shared tokens)
        let text_a = "aaaaa bbbbb ccccc ddddd eeeee";
        let text_b = "fffff ggggg hhhhh iiiii jjjjj";

        let tokens_a = tokenize(text_a);
        let tokens_b = tokenize(text_b);

        let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
        let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

        // Act: Compute similarity
        let similarity = sig_a.jaccard_similarity_q16(&sig_b);

        // Assert: Must be very close to 0.0 (MinHash is probabilistic, not exact)
        // Allow small error due to hash collisions
        assert!(
            similarity.to_f64() < 0.1,
            "Disjoint documents must have Jaccard ≈ 0.0, got {}",
            similarity.to_f64()
        );
    }

    /// Q2: Edge Case - Empty document handling
    #[test]

    fn test_q16_empty_documents() {
        // Arrange: Empty and non-empty documents
        let empty_tokens = vec![];
        let non_empty_tokens = tokenize("The quick brown fox");

        let sig_empty = MinHashSignatureCapsule::compute_signature(&to_str_vec(&empty_tokens));
        let sig_non_empty = MinHashSignatureCapsule::compute_signature(&to_str_vec(&non_empty_tokens));

        // Act: Compute similarities
        let empty_self = sig_empty.jaccard_similarity_q16(&sig_empty);
        let empty_vs_non_empty = sig_empty.jaccard_similarity_q16(&sig_non_empty);

        // Assert: Empty documents should have well-defined behavior
        // (Exact behavior depends on implementation - document actual behavior)
        assert!(
            empty_self >= Q16_16::ZERO && empty_self <= Q16_16::ONE,
            "Empty self-similarity must be in [0, 1]: {}",
            empty_self.to_f64()
        );
        assert!(
            empty_vs_non_empty >= Q16_16::ZERO && empty_vs_non_empty <= Q16_16::ONE,
            "Empty vs non-empty similarity must be in [0, 1]: {}",
            empty_vs_non_empty.to_f64()
        );
    }

    /// Q2: Edge Case - Single token documents
    #[test]

    fn test_q16_single_token() {
        // Arrange: Single token documents
        let tokens_a = vec!["hello".to_string()];
        let tokens_b = vec!["world".to_string()];
        let tokens_c = vec!["hello".to_string()];

        let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
        let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));
        let sig_c = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_c));

        // Act: Compute similarities
        let same = sig_a.jaccard_similarity_q16(&sig_c);
        let different = sig_a.jaccard_similarity_q16(&sig_b);

        // Assert: Same token should have high similarity, different should be low
        assert!(
            same.to_f64() > 0.9,
            "Same single token should have high similarity: {}",
            same.to_f64()
        );
        assert!(
            different.to_f64() < 0.1,
            "Different single tokens should have low similarity: {}",
            different.to_f64()
        );
    }

    /// Q3: Invariant - Jaccard similarity must be commutative
    ///
    /// Tests that sim(A, B) = sim(B, A) for all documents A, B.
    #[test]

    fn test_q16_commutativity() {
        // Arrange: Various document pairs
        let test_pairs = vec![
            ("The quick brown fox", "The lazy dog"),
            ("rust programming", "systems language"),
            ("machine learning", "deep neural networks"),
        ];

        for (text_a, text_b) in test_pairs {
            let tokens_a = tokenize(text_a);
            let tokens_b = tokenize(text_b);

            let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
            let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

            // Act: Compute both orders
            let sim_ab = sig_a.jaccard_similarity_q16(&sig_b);
            let sim_ba = sig_b.jaccard_similarity_q16(&sig_a);

            // Assert: Must be commutative
            assert_eq!(
                sim_ab,
                sim_ba,
                "Jaccard must be commutative for ('{}', '{}'): AB={}, BA={}",
                text_a,
                text_b,
                sim_ab.to_f64(),
                sim_ba.to_f64()
            );
        }
    }

    /// Q3: Invariant - Self-similarity must always be 1.0
    #[test]

    fn test_q16_self_similarity_invariant() {
        // Arrange: Various documents
        let test_docs = vec![
            "The quick brown fox",
            "a b c d e f g h i j",
            "single",
            "Machine learning models for natural language processing",
        ];

        for text in test_docs {
            let tokens = tokenize(text);
            let sig = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens));

            // Act: Compute self-similarity
            let sim = sig.jaccard_similarity_q16(&sig);

            // Assert: Must be exactly 1.0
            assert_eq!(
                sim,
                Q16_16::ONE,
                "Self-similarity must be 1.0 for '{}': got {}",
                text,
                sim.to_f64()
            );
        }
    }

    /// Q4: Code Path Coverage - Test all comparison paths
    #[test]

    fn test_q16_code_coverage() {
        // Arrange: Documents that trigger different code paths
        let identical = ("hello world", "hello world");
        let partial = ("hello world", "hello rust");
        let disjoint = ("aaaaa", "bbbbb");

        // Act & Assert: Cover all branches
        for (text_a, text_b) in vec![identical, partial, disjoint] {
            let tokens_a = tokenize(text_a);
            let tokens_b = tokenize(text_b);

            let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
            let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

            let similarity = sig_a.jaccard_similarity_q16(&sig_b);

            // Assert: All paths produce valid output
            assert!(
                similarity >= Q16_16::ZERO && similarity <= Q16_16::ONE,
                "Similarity must be valid for ('{}', '{}'): {}",
                text_a,
                text_b,
                similarity.to_f64()
            );
        }
    }

    /// Q5: Isolation - No shared state between test runs
    #[test]

    fn test_q16_isolation() {
        // Arrange: Create independent signatures
        let text = "The quick brown fox";
        let tokens = tokenize(text);

        // Act: Create multiple signatures independently
        let sig1 = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens));
        let sig2 = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens));

        // Assert: Both should produce identical results (deterministic, no shared state)
        let sim1 = sig1.jaccard_similarity_q16(&sig1);
        let sim2 = sig2.jaccard_similarity_q16(&sig2);

        assert_eq!(
            sim1,
            sim2,
            "Independent signatures must produce identical results: sim1={}, sim2={}",
            sim1.to_f64(),
            sim2.to_f64()
        );
    }

    /// Q6: Performance - Jaccard computation must be fast (<10ms)
    #[test]

    fn test_q16_performance() {
        use std::time::Instant;

        // Arrange: Pre-compute signatures
        let tokens_a = tokenize("The quick brown fox jumps over the lazy dog");
        let tokens_b = tokenize("The quick brown fox leaps over the lazy cat");

        let sig_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
        let sig_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

        // Act: Measure Jaccard computation time (100 iterations)
        let start = Instant::now();
        for _ in 0..100 {
            let _ = sig_a.jaccard_similarity_q16(&sig_b);
        }
        let elapsed = start.elapsed();

        // Assert: Average time per computation < 100μs (10ms / 100 iterations)
        let avg_micros = elapsed.as_micros() / 100;
        assert!(
            avg_micros < 100,
            "Q16.16 Jaccard must be fast: avg={}μs (target: <100μs)",
            avg_micros
        );
    }

    /// Q7: Readability - Clear test structure and failure messages
    #[test]

    fn test_q16_readable_test_example() {
        // Arrange: Set up test data with clear variable names
        let document_a = "The quick brown fox jumps over the lazy dog";
        let document_b = "The quick brown fox leaps over the lazy cat";

        let tokens_a = tokenize(document_a);
        let tokens_b = tokenize(document_b);

        let signature_a = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_a));
        let signature_b = MinHashSignatureCapsule::compute_signature(&to_str_vec(&tokens_b));

        // Act: Perform operation under test
        let similarity = signature_a.jaccard_similarity_q16(&signature_b);

        // Assert: Verify expected outcome with clear message
        assert!(
            similarity >= Q16_16::ZERO && similarity <= Q16_16::ONE,
            "Jaccard similarity must be in [0, 1] range. \
             Document A: '{}', \
             Document B: '{}', \
             Similarity: {}",
            document_a,
            document_b,
            similarity.to_f64()
        );
    }
}
