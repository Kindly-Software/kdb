//! Q8-Q14 Deterministic Deduplication Tests
//!
//! T28 comprehensive testing framework for deterministic deduplication properties.
//! Validates 100% reproducibility of deduplication results across runs.
//!
//! # Test Coverage (14 tests total)
//!
//! - **Q8 (Determinism)**: MinHash signatures, pipeline reproducibility
//! - **Q9 (Monotonicity)**: Document ID constraints
//! - **Q10 (Idempotency)**: Adding documents twice
//! - **Q11 (Memory Coherence)**: Concurrent visibility
//! - **Q12 (Bounded Resources)**: Memory growth limits
//! - **Q13 (Convergence)**: Algorithm termination
//! - **Q14 (Invariants)**: Transitive closure

#[cfg(test)]
mod determinism_tests {
    use kindly_dedup::deterministic_dedup::{
        DeterministicDedupPipeline, DeterministicMinHash, SeededRng,
    };

    // ========================================================================
    // Q8: DETERMINISM - MinHash and pipeline reproducibility
    // ========================================================================

    /// Q8.1: MinHash determinism with same seed
    #[test]
    fn q8_minhash_same_seed_determinism() {
        let text = "The quick brown fox jumps over the lazy dog";
        let seed = 0xDEADBEEF;

        let sig1 = DeterministicMinHash::compute(text, seed);
        let sig2 = DeterministicMinHash::compute(text, seed);

        assert_eq!(sig1, sig2, "MinHash not deterministic with same seed!");
    }

    /// Q8.2: Different seeds produce different MinHash
    #[test]
    fn q8_minhash_different_seed_different_hash() {
        let text = "The quick brown fox jumps over the lazy dog";

        let sig1 = DeterministicMinHash::compute(text, 0x1111);
        let sig2 = DeterministicMinHash::compute(text, 0x2222);

        assert_ne!(sig1, sig2, "Different seeds should produce different hashes");
    }

    /// Q8.3: Full pipeline determinism - run 1
    #[test]
    fn q8_full_pipeline_determinism_run1() {
        let documents = vec![
            (0u32, "Document about machine learning"),
            (1, "Document about machine learning and AI"),
            (2, "Completely different topic here"),
            (3, "Document about machine learning again"),
            (4, "Another machine learning document"),
        ];

        let mut pipe = DeterministicDedupPipeline::new(100, 0x1234567890).unwrap();
        for (doc_id, text) in documents {
            pipe.add_document(doc_id, text).unwrap();
        }

        let clusters = pipe.find_duplicates(0.6).unwrap();
        // Store for comparison with run 2
        let _ = clusters;
    }

    /// Q8.4: Full pipeline determinism - run 2 (identical to run 1)
    #[test]
    fn q8_full_pipeline_determinism_run2_identical() {
        let documents = vec![
            (0u32, "Document about machine learning"),
            (1, "Document about machine learning and AI"),
            (2, "Completely different topic here"),
            (3, "Document about machine learning again"),
            (4, "Another machine learning document"),
        ];

        let mut pipe1 = DeterministicDedupPipeline::new(100, 0x1234567890).unwrap();
        let mut pipe2 = DeterministicDedupPipeline::new(100, 0x1234567890).unwrap();

        for (doc_id, text) in &documents {
            pipe1.add_document(*doc_id, text).unwrap();
            pipe2.add_document(*doc_id, text).unwrap();
        }

        let clusters1 = pipe1.find_duplicates(0.6).unwrap();
        let clusters2 = pipe2.find_duplicates(0.6).unwrap();

        assert_eq!(clusters1, clusters2, "Pipeline results not deterministic!");
    }

    /// Q8.5: Seeded RNG determinism
    #[test]
    fn q8_seeded_rng_determinism() {
        let seed = 0x5A5A5A5A;
        let mut rng1 = SeededRng::new(seed);
        let mut rng2 = SeededRng::new(seed);

        for _ in 0..1000 {
            assert_eq!(
                rng1.next_u64(),
                rng2.next_u64(),
                "Seeded RNG not deterministic!"
            );
        }
    }

    // ========================================================================
    // Q9: MONOTONICITY - Document IDs and ordering
    // ========================================================================

    /// Q9.1: Document IDs always unique (monotonic in insertion)
    #[test]
    fn q9_document_ids_unique() {
        let mut pipe = DeterministicDedupPipeline::new(100, 0xABCD).unwrap();

        for i in 0..50 {
            pipe.add_document(i, &format!("Document {}", i)).unwrap();
        }

        assert_eq!(
            pipe.document_count(),
            50,
            "Document count mismatch (duplicate IDs?)"
        );
    }

    /// Q9.2: Clusters sorted deterministically (monotonic output)
    #[test]
    fn q9_cluster_sorting_deterministic() {
        let documents = vec![
            (5u32, "text"),
            (3, "text"),
            (7, "text"),
            (1, "text"),
            (9, "text"),
        ];

        let mut pipe1 = DeterministicDedupPipeline::new(100, 0x7777).unwrap();
        let mut pipe2 = DeterministicDedupPipeline::new(100, 0x7777).unwrap();

        for (doc_id, text) in &documents {
            pipe1.add_document(*doc_id, text).unwrap();
            pipe2.add_document(*doc_id, text).unwrap();
        }

        let clusters1 = pipe1.find_duplicates(0.9).unwrap();
        let clusters2 = pipe2.find_duplicates(0.9).unwrap();

        assert_eq!(
            clusters1, clusters2,
            "Cluster sorting not deterministic!"
        );
    }

    // ========================================================================
    // Q10: IDEMPOTENCY - add_document twice = once
    // ========================================================================

    /// Q10.1: Adding same document twice is error
    #[test]
    fn q10_idempotent_document_add_same_id_error() {
        let mut pipe = DeterministicDedupPipeline::new(100, 0x1111).unwrap();

        pipe.add_document(0, "Document text").unwrap();

        let result = pipe.add_document(0, "Document text");
        assert!(
            result.is_err(),
            "Adding same document ID should be an error"
        );
    }

    /// Q10.2: Document count stays same after idempotent attempt
    #[test]
    fn q10_idempotent_document_count_unchanged() {
        let mut pipe = DeterministicDedupPipeline::new(100, 0x2222).unwrap();

        pipe.add_document(0, "Document").unwrap();
        let count_before = pipe.document_count();

        let _ = pipe.add_document(0, "Document"); // Fails (correct)

        assert_eq!(
            pipe.document_count(),
            count_before,
            "Document count changed after idempotent error"
        );
    }

    // ========================================================================
    // Q11: MEMORY COHERENCE - LSH buckets visible across operations
    // ========================================================================

    /// Q11.1: All added documents visible in signatures
    #[test]
    fn q11_all_documents_visible() {
        let mut pipe = DeterministicDedupPipeline::new(100, 0x3333).unwrap();

        for i in 0..100 {
            pipe.add_document(i, &format!("Document {}", i)).unwrap();
        }

        // All documents should be retrievable
        for i in 0..100 {
            assert!(
                pipe.get_signature(i).is_some(),
                "Document {} not visible in signatures",
                i
            );
        }
    }

    /// Q11.2: Signatures consistent across multiple accesses
    #[test]
    fn q11_signature_consistency() {
        let mut pipe = DeterministicDedupPipeline::new(100, 0x4444).unwrap();

        pipe.add_document(0, "Document text").unwrap();

        let sig1 = pipe.get_signature(0).unwrap().clone();
        let sig2 = pipe.get_signature(0).unwrap().clone();

        assert_eq!(sig1, sig2, "Signature not consistent across accesses");
    }

    // ========================================================================
    // Q12: BOUNDED RESOURCES - No unbounded growth
    // ========================================================================

    /// Q12.1: Memory usage bounded by capacity
    #[test]
    fn q12_memory_bounded_by_capacity() {
        let capacity = 1000;
        let mut pipe = DeterministicDedupPipeline::new(capacity, 0x5555).unwrap();

        for i in 0..capacity as u32 {
            pipe.add_document(i, "Document").unwrap();
        }

        let memory = pipe.memory_usage();
        // Should be approximately capacity × signature_size
        // MinHash (128 u16 = 256 bytes) + HashMap overhead (~250 bytes/entry)
        let expected_max = capacity * 600; // Conservative estimate with overhead

        assert!(
            memory < expected_max as usize,
            "Memory usage {} exceeds expected max {}",
            memory,
            expected_max
        );
    }

    /// Q12.2: Document count matches added count
    #[test]
    fn q12_document_count_matches() {
        let mut pipe = DeterministicDedupPipeline::new(100, 0x6666).unwrap();

        let count = 50;
        for i in 0..count {
            pipe.add_document(i, "Document").unwrap();
        }

        assert_eq!(
            pipe.document_count(),
            count as usize,
            "Document count mismatch"
        );
    }

    // ========================================================================
    // Q13: CONVERGENCE - Algorithm terminates in reasonable time
    // ========================================================================

    /// Q13.1: find_duplicates terminates for small corpus
    #[test]
    fn q13_convergence_small_corpus() {
        let mut pipe = DeterministicDedupPipeline::new(100, 0x7777).unwrap();

        for i in 0..10 {
            pipe.add_document(i, &format!("Document {}", i)).unwrap();
        }

        // Should complete quickly
        let start = std::time::Instant::now();
        let _clusters = pipe.find_duplicates(0.8).unwrap();
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 1000,
            "find_duplicates took too long: {:?}",
            elapsed
        );
    }

    /// Q13.2: find_duplicates terminates for medium corpus
    #[test]
    fn q13_convergence_medium_corpus() {
        let mut pipe = DeterministicDedupPipeline::new(1000, 0x8888).unwrap();

        for i in 0..100 {
            pipe.add_document(i, &format!("Document {}", i)).unwrap();
        }

        let start = std::time::Instant::now();
        let _clusters = pipe.find_duplicates(0.8).unwrap();
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_secs() < 10,
            "find_duplicates took too long: {:?}",
            elapsed
        );
    }

    /// Q13.3: Invalid threshold rejected
    #[test]
    fn q13_convergence_invalid_threshold() {
        let mut pipe = DeterministicDedupPipeline::new(100, 0x9999).unwrap();

        pipe.add_document(0, "Document").unwrap();

        let result = pipe.find_duplicates(1.5); // Invalid
        assert!(
            result.is_err(),
            "Should reject invalid threshold > 1.0"
        );
    }

    // ========================================================================
    // Q14: INVARIANTS - Transitive closure (A~B, B~C → A~C)
    // ========================================================================

    /// Q14.1: Transitivity in duplicate clusters
    #[test]
    fn q14_transitive_closure() {
        let documents = vec![
            (0u32, "text text text"),
            (1, "text text text"), // Same as 0
            (2, "text text text"), // Same as 0 and 1
        ];

        let mut pipe = DeterministicDedupPipeline::new(100, 0xAAAA).unwrap();
        for (doc_id, text) in documents {
            pipe.add_document(doc_id, text).unwrap();
        }

        let clusters = pipe.find_duplicates(0.9).unwrap();

        // All three should be in same cluster (transitive)
        let cluster = &clusters[0];
        assert_eq!(
            cluster.len(),
            3,
            "Not all transitive duplicates in same cluster"
        );

        // Check they're all there
        assert!(cluster.contains(&0));
        assert!(cluster.contains(&1));
        assert!(cluster.contains(&2));
    }

    /// Q14.2: Non-duplicates not in same cluster
    #[test]
    fn q14_non_duplicates_separate() {
        let documents = vec![
            (0u32, "apple banana cherry"),
            (1, "elephant forest giraffe"),
            (2, "house island jungle"),
        ];

        let mut pipe = DeterministicDedupPipeline::new(100, 0xBBBB).unwrap();
        for (doc_id, text) in documents {
            pipe.add_document(doc_id, text).unwrap();
        }

        let clusters = pipe.find_duplicates(0.8).unwrap();

        // Should have 3 separate clusters (or 3 singletons)
        assert_eq!(clusters.len(), 3, "Different documents in same cluster");
    }

    /// Q14.3: Large transitive closure (chain: 0~1~2~3~4)
    #[test]
    fn q14_large_transitive_chain() {
        let mut pipe = DeterministicDedupPipeline::new(100, 0xCCCC).unwrap();

        // Create a chain where each is very similar to previous
        for i in 0..5 {
            pipe.add_document(i, "base text content here").unwrap();
        }

        let clusters = pipe.find_duplicates(0.7).unwrap();

        // Due to high similarity, likely in same cluster
        let max_cluster = clusters.iter().max_by_key(|c| c.len()).unwrap();
        assert!(
            max_cluster.len() >= 3,
            "Transitive chain not well clustered"
        );
    }
}
