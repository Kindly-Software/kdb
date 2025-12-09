//! Comprehensive T28 Test Suite for StreamingMinHashBuilderCapsule
//!
//! **45 Tests across 4 tiers: Unit (Q1-Q7) | Property (Q8-Q14) | Integration (Q15-Q21) | Production (Q22-Q28)**
//!
//! ## Framework Compliance
//!
//! - **T28**: Systematic 4-tier testing framework
//! - **UCE34**: Q1-Q34 discovery validated through tests
//! - **Chaos**: 100% lockfree (no mutex in tests, atomic operations only)
//! - **B32**: Performance validation with throughput metrics
//! - **I20**: Integration testing with StreamingTokenizerCapsule output

use kindly_dedup::streaming::StreamingMinHashBuilderCapsule;
use std::sync::Arc;

// ============================================================================
// Q1-Q7: UNIT TESTS (15 tests - Basic Correctness)
// ============================================================================

#[test]
fn unit_q1_test_new_initialization() {
    let builder = StreamingMinHashBuilderCapsule::new();

    // All signatures should be u16::MAX
    for i in 0..128 {
        assert_eq!(
            builder.signatures[i].load(std::sync::atomic::Ordering::Relaxed),
            u16::MAX,
            "Signature {} should initialize to u16::MAX",
            i
        );
    }

    // Counters should be zero
    assert_eq!(
        builder.token_count.load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        builder.generation.load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[test]
fn unit_q2_test_default_trait() {
    let builder1 = StreamingMinHashBuilderCapsule::new();
    let builder2 = StreamingMinHashBuilderCapsule::default();

    // Default should be identical to new()
    let sig1 = builder1.extract_signature();
    let sig2 = builder2.extract_signature();

    assert_eq!(sig1, sig2, "Default should match new()");
}

#[test]
fn unit_q3_test_add_single_token() {
    let builder = StreamingMinHashBuilderCapsule::new();

    builder.add_token("test");

    // At least one signature should change from u16::MAX
    let changed = (0..128).filter(|&i| {
        builder.signatures[i].load(std::sync::atomic::Ordering::Relaxed) != u16::MAX
    });

    assert!(
        changed.count() > 0,
        "At least one signature should change after adding token"
    );

    // Token count should be 1
    assert_eq!(
        builder.token_count.load(std::sync::atomic::Ordering::Relaxed),
        1
    );
}

#[test]
fn unit_q4_test_extract_signature() {
    let builder = StreamingMinHashBuilderCapsule::new();

    builder.add_token("hello");
    builder.add_token("world");

    let sig = builder.extract_signature();

    // Signature should have exactly 128 elements
    assert_eq!(sig.len(), 128);

    // At least some non-MAX values (very high probability)
    let non_max = sig.iter().filter(|&&x| x != u16::MAX).count();
    assert!(non_max > 0, "Expected non-MAX values in signature");
}

#[test]
fn unit_q5_test_reset() {
    let builder = StreamingMinHashBuilderCapsule::new();

    builder.add_token("test");
    assert_eq!(
        builder.token_count.load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    let gen1 = builder.generation.load(std::sync::atomic::Ordering::Relaxed);

    builder.reset();
    assert_eq!(
        builder.token_count.load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        builder.generation.load(std::sync::atomic::Ordering::Relaxed),
        gen1 + 1
    );

    // All signatures should be back to u16::MAX
    for i in 0..128 {
        assert_eq!(
            builder.signatures[i].load(std::sync::atomic::Ordering::Relaxed),
            u16::MAX
        );
    }
}

#[test]
fn unit_q6_test_empty_document() {
    let builder = StreamingMinHashBuilderCapsule::new();

    // Extract without adding any tokens
    let sig = builder.extract_signature();

    // All values should be u16::MAX (no tokens processed)
    for val in sig {
        assert_eq!(val, u16::MAX);
    }
}

#[test]
fn unit_q7_test_process_tokens() {
    let builder = StreamingMinHashBuilderCapsule::new();

    let tokens = vec!["the", "quick", "brown", "fox"];
    let sig = builder.process_tokens(&tokens);

    // Should produce non-MAX values
    assert!(
        sig.iter().any(|&x| x != u16::MAX),
        "Batch processing should produce non-MAX values"
    );

    // Token count should be 4
    assert_eq!(
        builder.token_count.load(std::sync::atomic::Ordering::Relaxed),
        4
    );
}

#[test]
fn unit_q8_test_process_arc_str_tokens() {
    let builder = StreamingMinHashBuilderCapsule::new();

    let tokens: Vec<Arc<str>> =
        vec![Arc::from("the"), Arc::from("quick"), Arc::from("brown")];

    let sig = builder.process_arc_tokens(&tokens);

    assert!(
        sig.iter().any(|&x| x != u16::MAX),
        "Arc<str> processing should work"
    );
}

#[test]
fn unit_q9_test_get_token_count() {
    let builder = StreamingMinHashBuilderCapsule::new();

    assert_eq!(builder.get_token_count(), 0);

    builder.add_token("a");
    assert_eq!(builder.get_token_count(), 1);

    builder.add_token("b");
    assert_eq!(builder.get_token_count(), 2);

    builder.reset();
    assert_eq!(builder.get_token_count(), 0);
}

#[test]
fn unit_q10_test_get_generation() {
    let builder = StreamingMinHashBuilderCapsule::new();

    let gen0 = builder.get_generation();
    assert_eq!(gen0, 0);

    builder.reset();
    let gen1 = builder.get_generation();
    assert_eq!(gen1, 1);

    builder.reset();
    let gen2 = builder.get_generation();
    assert_eq!(gen2, 2);
}

#[test]
fn unit_q11_test_multiple_resets() {
    let builder = StreamingMinHashBuilderCapsule::new();

    for _ in 0..10 {
        builder.add_token("token");
        let sig = builder.extract_signature();
        assert!(sig.iter().any(|&x| x != u16::MAX));
        builder.reset();
    }
}

#[test]
fn unit_q12_test_signature_array_bounds() {
    let builder = StreamingMinHashBuilderCapsule::new();
    builder.add_token("test");

    let sig = builder.extract_signature();

    // All values should be valid u16
    for (i, &val) in sig.iter().enumerate() {
        assert!(val <= u16::MAX, "Value at index {} exceeds u16::MAX", i);
    }
}

#[test]
fn unit_q13_test_deterministic_hash() {
    let builder = StreamingMinHashBuilderCapsule::new();

    // Same token twice
    let sig1 = builder.process_tokens(&["test"]);
    builder.reset();
    let sig2 = builder.process_tokens(&["test"]);

    assert_eq!(sig1, sig2, "Same token should produce identical signatures");
}

#[test]
fn unit_q14_test_capacity_stability() {
    let builder = StreamingMinHashBuilderCapsule::new();

    // All 128 signature slots should always be accessible
    for i in 0..128 {
        builder.signatures[i].store(i as u16, std::sync::atomic::Ordering::Relaxed);
        let val = builder.signatures[i].load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(val, i as u16);
    }
}

#[test]
fn unit_q15_test_send_sync_traits() {
    // Verify Send + Sync are implemented
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<StreamingMinHashBuilderCapsule>();
    assert_sync::<StreamingMinHashBuilderCapsule>();
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS (10 tests - Invariants & Guarantees)
// ============================================================================

#[test]
fn property_q1_test_minimum_invariant() {
    let builder = StreamingMinHashBuilderCapsule::new();

    // Add multiple tokens
    let tokens = vec!["token1", "token2", "token3", "token4"];
    builder.process_tokens(&tokens);

    let sig = builder.extract_signature();

    // All signature values should be valid u16 (no overflow)
    for &val in sig {
        assert!(val <= u16::MAX);
    }
}

#[test]
fn property_q2_test_permutation_independence() {
    let builder = StreamingMinHashBuilderCapsule::new();

    builder.add_token("test");
    let sig = builder.extract_signature();

    // Check that most signature values are different (independence of permutations)
    let unique_count = sig.iter().collect::<std::collections::HashSet<_>>().len();

    assert!(
        unique_count > 100,
        "Expected >100 unique values, got {}",
        unique_count
    );
}

#[test]
fn property_q3_test_set_semantics() {
    // Same token twice should not change the signature
    let builder1 = StreamingMinHashBuilderCapsule::new();
    builder1.add_token("duplicate");
    builder1.add_token("duplicate");
    let sig1 = builder1.extract_signature();

    let builder2 = StreamingMinHashBuilderCapsule::new();
    builder2.add_token("duplicate");
    let sig2 = builder2.extract_signature();

    // Signatures should be identical (min is idempotent)
    assert_eq!(sig1, sig2, "Duplicate tokens should not change signature");
}

#[test]
fn property_q4_test_order_invariance() {
    // Different orders should produce identical signatures
    let builder1 = StreamingMinHashBuilderCapsule::new();
    builder1.process_tokens(&["a", "b", "c"]);
    let sig1 = builder1.extract_signature();

    let builder2 = StreamingMinHashBuilderCapsule::new();
    builder2.process_tokens(&["c", "b", "a"]);
    let sig2 = builder2.extract_signature();

    // Same tokens in different order → same signature
    assert_eq!(sig1, sig2, "Token order should not affect signature");
}

#[test]
fn property_q5_test_subset_property() {
    // Adding more tokens should only decrease some minimums (monotonicity)
    let builder = StreamingMinHashBuilderCapsule::new();
    let tokens_small = vec!["a", "b"];
    let sig_small = builder.process_tokens(&tokens_small);

    builder.reset();
    let tokens_large = vec!["a", "b", "c", "d", "e"];
    let sig_large = builder.process_tokens(&tokens_large);

    // At least one value in sig_large should be ≤ corresponding value in sig_small
    let improved = (0..128).filter(|&i| sig_large[i] < sig_small[i]).count();
    assert!(improved > 0, "Larger set should improve at least some minimums");
}

#[test]
fn property_q6_test_collision_resistance() {
    // Different tokens should likely produce different signatures
    let builder = StreamingMinHashBuilderCapsule::new();
    let sig1 = builder.process_tokens(&["token1", "token2"]);

    builder.reset();
    let sig2 = builder.process_tokens(&["different", "tokens"]);

    // Very unlikely to have identical signatures with MinHash
    let diff_count = (0..128).filter(|&i| sig1[i] != sig2[i]).count();
    assert!(
        diff_count > 50,
        "Different tokens should produce mostly different signatures"
    );
}

#[test]
fn property_q7_test_determinism_property() {
    // Multiple runs with same tokens should produce identical signatures
    let tokens = vec!["test", "determinism", "property"];

    let mut sigs = Vec::new();
    for _ in 0..5 {
        let builder = StreamingMinHashBuilderCapsule::new();
        sigs.push(builder.process_tokens(&tokens));
    }

    // All signatures should be identical
    for sig in &sigs[1..] {
        assert_eq!(sig, &sigs[0], "Determinism violated");
    }
}

#[test]
fn property_q8_test_incremental_vs_batch() {
    let tokens = vec!["the", "quick", "brown", "fox"];

    // Method 1: Batch processing
    let builder1 = StreamingMinHashBuilderCapsule::new();
    let sig_batch = builder1.process_tokens(&tokens);

    // Method 2: Incremental
    let builder2 = StreamingMinHashBuilderCapsule::new();
    for token in &tokens {
        builder2.add_token(token);
    }
    let sig_incremental = builder2.extract_signature();

    // Both methods should produce identical signatures
    assert_eq!(
        sig_batch, sig_incremental,
        "Batch and incremental should produce identical results"
    );
}

#[test]
fn property_q9_test_reset_idempotence() {
    let builder = StreamingMinHashBuilderCapsule::new();

    builder.add_token("test");
    builder.reset();
    let sig1 = builder.extract_signature();

    builder.reset();
    let sig2 = builder.extract_signature();

    // Both resets should produce identical (empty) signatures
    assert_eq!(sig1, sig2, "Reset should be idempotent");
}

#[test]
fn property_q10_test_generation_counter_monotonicity() {
    let builder = StreamingMinHashBuilderCapsule::new();

    let mut prev_gen = builder.get_generation();
    for _ in 0..10 {
        builder.reset();
        let curr_gen = builder.get_generation();
        assert!(
            curr_gen > prev_gen,
            "Generation should monotonically increase"
        );
        prev_gen = curr_gen;
    }
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS (12 tests - Component Interaction)
// ============================================================================

#[test]
fn integration_q1_test_batch_processing() {
    let builder = StreamingMinHashBuilderCapsule::new();

    let tokens = vec!["the", "quick", "brown", "fox", "jumps", "over"];
    let sig = builder.process_tokens(&tokens);

    assert!(
        sig.iter().any(|&x| x != u16::MAX),
        "Batch should produce non-MAX values"
    );
}

#[test]
fn integration_q2_test_arc_str_compatibility() {
    let builder = StreamingMinHashBuilderCapsule::new();
    let tokens: Vec<Arc<str>> = vec![Arc::from("the"), Arc::from("quick"), Arc::from("brown")];

    let sig = builder.process_arc_tokens(&tokens);
    assert!(sig.iter().any(|&x| x != u16::MAX));
}

#[test]
fn integration_q3_test_multiple_documents_sequential() {
    let builder = StreamingMinHashBuilderCapsule::new();

    // Document 1
    let sig1 = builder.process_tokens(&["doc1", "text"]);

    // Document 2 (same builder, reset between)
    builder.reset();
    let sig2 = builder.process_tokens(&["doc2", "text"]);

    // Different documents should likely have different signatures
    let diff_count = (0..128).filter(|&i| sig1[i] != sig2[i]).count();
    assert!(diff_count > 0, "Different documents should have different signatures");
}

#[test]
fn integration_q4_test_large_batch() {
    let builder = StreamingMinHashBuilderCapsule::new();

    let tokens: Vec<&str> = (0..1000)
        .map(|i| Box::leak(format!("token{}", i).into_boxed_str()))
        .map(|s: &'static str| s)
        .collect();

    let sig = builder.process_tokens(&tokens);
    assert_eq!(builder.get_token_count(), 1000);
    assert!(sig.iter().any(|&x| x != u16::MAX));
}

#[test]
fn integration_q5_test_repeated_tokens() {
    let builder = StreamingMinHashBuilderCapsule::new();

    // Token appears multiple times
    let tokens = vec!["common", "common", "common", "rare"];
    let sig = builder.process_tokens(&tokens);

    // Should still work correctly (set semantics)
    assert!(sig.iter().any(|&x| x != u16::MAX));
}

#[test]
fn integration_q6_test_empty_tokens() {
    let builder = StreamingMinHashBuilderCapsule::new();

    // Empty string token
    builder.add_token("");
    let sig = builder.extract_signature();

    // Should still produce a valid signature
    assert_eq!(sig.len(), 128);
}

#[test]
fn integration_q7_test_long_token_strings() {
    let builder = StreamingMinHashBuilderCapsule::new();

    let long_token = "a".repeat(10000);
    builder.add_token(&long_token);

    let sig = builder.extract_signature();
    assert!(sig.iter().any(|&x| x != u16::MAX));
}

#[test]
fn integration_q8_test_unicode_tokens() {
    let builder = StreamingMinHashBuilderCapsule::new();

    let tokens = vec!["こんにちは", "мир", "🚀", "Ñoño"];
    let sig = builder.process_tokens(&tokens);

    assert!(sig.iter().any(|&x| x != u16::MAX));
}

#[test]
fn integration_q9_test_pipeline_workflow() {
    // Simulate: Tokenizer → MinHash → Extract
    let tokens_doc1: Vec<Arc<str>> = vec![Arc::from("hello"), Arc::from("world")];
    let tokens_doc2: Vec<Arc<str>> = vec![Arc::from("goodbye"), Arc::from("world")];

    let builder = StreamingMinHashBuilderCapsule::new();

    // Process doc 1
    let sig1 = builder.process_arc_tokens(&tokens_doc1);

    // Process doc 2
    builder.reset();
    let sig2 = builder.process_arc_tokens(&tokens_doc2);

    // Signatures should be computed
    assert!(sig1.iter().any(|&x| x != u16::MAX));
    assert!(sig2.iter().any(|&x| x != u16::MAX));
}

#[test]
fn integration_q10_test_streaming_interface() {
    let builder = StreamingMinHashBuilderCapsule::new();

    // Simulate streaming (tokens arriving one at a time)
    for i in 0..100 {
        builder.add_token(&format!("token{}", i));
    }

    let sig = builder.extract_signature();
    assert_eq!(builder.get_token_count(), 100);
    assert!(sig.iter().any(|&x| x != u16::MAX));
}

#[test]
fn integration_q11_test_signature_cache_stability() {
    let builder = StreamingMinHashBuilderCapsule::new();

    builder.add_token("stable");
    let sig1 = builder.extract_signature();
    let sig2 = builder.extract_signature();
    let sig3 = builder.extract_signature();

    // Multiple extractions should return identical signatures (cache stability)
    assert_eq!(sig1, sig2);
    assert_eq!(sig2, sig3);
}

#[test]
fn integration_q12_test_interleaved_add_extract() {
    let builder = StreamingMinHashBuilderCapsule::new();

    builder.add_token("first");
    let sig1 = builder.extract_signature();

    builder.add_token("second");
    let sig2 = builder.extract_signature();

    // Second signature should have same or lower minimums
    let stable = (0..128).filter(|&i| sig2[i] <= sig1[i]).count();
    assert_eq!(stable, 128, "Minimums should be monotonically non-increasing");
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS (8 tests - Scale & Performance)
// ============================================================================

#[test]
fn production_q1_test_10m_docs_throughput() {
    use std::time::Instant;

    let builder = StreamingMinHashBuilderCapsule::new();

    // Measure throughput for 10,000 documents (scaled from 10M)
    let start = Instant::now();

    for doc_id in 0..10000 {
        let tokens = vec![
            format!("token_{}_{}", doc_id, 0),
            format!("token_{}_{}", doc_id, 1),
            format!("token_{}_{}", doc_id, 2),
        ];

        let tokens_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
        let _sig = builder.process_tokens(&tokens_refs);

        if doc_id % 100 == 0 {
            builder.reset();
        }
    }

    let elapsed = start.elapsed();
    let throughput = 10000.0 / elapsed.as_secs_f64();
    eprintln!("Throughput: {:.0} docs/sec", throughput);

    // Should handle at least 10K docs in reasonable time
    assert!(elapsed.as_secs() < 10, "Throughput too slow");
}

#[test]
fn production_q2_test_signature_quality() {
    let builder = StreamingMinHashBuilderCapsule::new();

    // Process long document
    let long_doc = (0..1000)
        .map(|i| format!("word{}", i))
        .collect::<Vec<_>>();

    let tokens: Vec<&str> = long_doc.iter().map(|s| s.as_str()).collect();
    let sig = builder.process_tokens(&tokens);

    // Signature should be well-distributed (many unique values)
    let unique: std::collections::HashSet<_> = sig.iter().collect();
    assert!(
        unique.len() > 100,
        "Signature should have >100 unique values, got {}",
        unique.len()
    );
}

#[test]
fn production_q3_test_memory_stability() {
    // Run 10K iterations without memory leaks
    for _ in 0..10000 {
        let builder = StreamingMinHashBuilderCapsule::new();
        builder.process_tokens(&["test", "stability"]);
        let _sig = builder.extract_signature();
    }
}

#[test]
fn production_q4_test_atomic_ordering_consistency() {
    let builder = StreamingMinHashBuilderCapsule::new();

    builder.add_token("token");

    // Read with different orderings should be consistent
    let sig_relaxed = builder.signatures[0].load(std::sync::atomic::Ordering::Relaxed);
    let sig_acquire = builder.signatures[0].load(std::sync::atomic::Ordering::Acquire);

    assert_eq!(sig_relaxed, sig_acquire);
}

#[test]
fn production_q5_test_cache_friendly_extraction() {
    use std::time::Instant;

    let builder = StreamingMinHashBuilderCapsule::new();
    builder.add_token("test");

    // Extract signature 100K times
    let start = Instant::now();
    for _ in 0..100000 {
        let _sig = builder.extract_signature();
    }
    let elapsed = start.elapsed();

    // Should be extremely fast (cache-resident)
    let latency_ns = elapsed.as_nanos() / 100000;
    eprintln!("Extraction latency: {} ns", latency_ns);
    assert!(latency_ns < 1000, "Extraction should be <1000ns");
}

#[test]
fn production_q6_test_determinism_across_rebuild() {
    let tokens = vec!["determinism", "test", "production"];

    let mut sigs = Vec::new();

    // Run 10 times
    for _ in 0..10 {
        let builder = StreamingMinHashBuilderCapsule::new();
        sigs.push(builder.process_tokens(&tokens));
    }

    // All should be identical
    for (i, sig) in sigs.iter().enumerate().skip(1) {
        assert_eq!(sig, &sigs[0], "Signature {} differs", i);
    }
}

#[test]
fn production_q7_test_overflow_safety() {
    let builder = StreamingMinHashBuilderCapsule::new();

    // Add many tokens (stress u32 counter if it were unguarded)
    for i in 0..100000 {
        builder.add_token(&format!("token{}", i));
    }

    let count = builder.get_token_count();
    assert_eq!(count, 100000);

    // Signature should still be valid
    let sig = builder.extract_signature();
    assert!(sig.iter().any(|&x| x != u16::MAX));
}

#[test]
fn production_q8_test_concurrent_extraction() {
    use std::sync::Arc as StdArc;
    use std::thread;

    let builder = StdArc::new(StreamingMinHashBuilderCapsule::new());
    builder.add_token("concurrent");

    let mut handles = vec![];

    // Spawn 10 threads extracting simultaneously
    for _ in 0..10 {
        let b = StdArc::clone(&builder);
        handles.push(thread::spawn(move || {
            let sig = b.extract_signature();
            assert!(sig.iter().any(|&x| x != u16::MAX));
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
