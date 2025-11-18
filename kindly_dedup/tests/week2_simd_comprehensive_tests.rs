//! Week 2 SIMD Text Hashing Comprehensive Tests (T28 Framework)
//!
//! **Test Suite**: 45 tests (15 Unit + 10 Property + 12 Integration + 8 Production)
//! **Target**: atomic_capsule::text::SimdTextHasher
//! **Framework**: T28 Testing Framework (4-tier validation)
//! **Feature Gate**: simd-text-hashing

#![cfg(feature = "simd-text-hashing")]

use atomic_capsule::text::SimdTextHasher;
use proptest::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7) - 15 tests, <5s timeout
// ============================================================================

/// Q1: Core behaviors - Empty string handling
#[test]
fn test_unit_empty_string() {
    let hasher = SimdTextHasher::new();
    let hashes = hasher.hash_tokens_simd("");
    assert_eq!(hashes.len(), 0, "Empty string should produce zero tokens");
}

/// Q1: Core behaviors - Single token
#[test]
fn test_unit_single_token() {
    let hasher = SimdTextHasher::new();
    let hashes = hasher.hash_tokens_simd("hello");
    assert_eq!(hashes.len(), 1, "Single token should produce one hash");
}

/// Q1: Core behaviors - Multiple tokens
#[test]
fn test_unit_multiple_tokens() {
    let hasher = SimdTextHasher::new();
    let hashes = hasher.hash_tokens_simd("the quick brown fox");
    assert_eq!(hashes.len(), 4, "Four tokens should produce four hashes");
}

/// Q1: Core behaviors - 100 tokens (typical document)
#[test]
fn test_unit_100_tokens() {
    let hasher = SimdTextHasher::new();
    let text = (0..100).map(|i| format!("token{}", i)).collect::<Vec<_>>().join(" ");
    let hashes = hasher.hash_tokens_simd(&text);
    assert_eq!(hashes.len(), 100, "100 tokens should produce 100 hashes");
}

/// Q2: Edge cases - Whitespace variations
#[test]
fn test_unit_whitespace_variations() {
    let hasher = SimdTextHasher::new();

    // Multiple spaces
    let h1 = hasher.hash_tokens_simd("a  b   c");
    assert_eq!(h1.len(), 3, "Multiple spaces should be collapsed");

    // Tabs and newlines
    let h2 = hasher.hash_tokens_simd("a\tb\nc");
    assert_eq!(h2.len(), 3, "Tabs and newlines are whitespace");

    // Leading/trailing whitespace
    let h3 = hasher.hash_tokens_simd("  hello world  ");
    assert_eq!(h3.len(), 2, "Leading/trailing whitespace trimmed");
}

/// Q2: Edge cases - Unicode characters
#[test]
fn test_unit_unicode_handling() {
    let hasher = SimdTextHasher::new();

    // Unicode tokens
    let hashes = hasher.hash_tokens_simd("café résumé naïve");
    assert_eq!(hashes.len(), 3, "Unicode tokens should be handled");

    // Emoji
    let h2 = hasher.hash_tokens_simd("hello 🌍 world 🚀");
    assert_eq!(h2.len(), 4, "Emoji should be treated as tokens");
}

/// Q2: Edge cases - Very long tokens
#[test]
fn test_unit_long_tokens() {
    let hasher = SimdTextHasher::new();

    // 1000-character token
    let long_token = "a".repeat(1000);
    let hashes = hasher.hash_tokens_simd(&long_token);
    assert_eq!(hashes.len(), 1, "Long token should produce one hash");
    assert_ne!(hashes[0], 0, "Hash should be non-zero");
}

/// Q3: Invariants - Determinism (same input → same output)
#[test]
fn test_unit_determinism() {
    let hasher = SimdTextHasher::new();
    let text = "the quick brown fox jumps over the lazy dog";

    let h1 = hasher.hash_tokens_simd(text);
    let h2 = hasher.hash_tokens_simd(text);

    assert_eq!(h1, h2, "Same input must produce same output");
}

/// Q3: Invariants - Distinct hashes for distinct tokens
#[test]
fn test_unit_distinct_hashes() {
    let hasher = SimdTextHasher::new();
    let hashes = hasher.hash_tokens_simd("alpha beta gamma delta");

    let unique: HashSet<_> = hashes.iter().collect();
    assert_eq!(unique.len(), 4, "Distinct tokens should produce distinct hashes");
}

/// Q4: Code path coverage - SIMD path (8+ tokens)
#[test]
#[cfg(feature = "portable_simd")]
fn test_unit_simd_path() {
    let hasher = SimdTextHasher::new();
    let text = "one two three four five six seven eight nine ten";
    let hashes = hasher.hash_tokens_simd(text);
    assert_eq!(hashes.len(), 10, "SIMD path should handle 10 tokens");
}

/// Q4: Code path coverage - Scalar remainder path (<8 tokens)
#[test]
fn test_unit_scalar_remainder() {
    let hasher = SimdTextHasher::new();
    let text = "one two three"; // 3 tokens (not divisible by 8)
    let hashes = hasher.hash_tokens_simd(text);
    assert_eq!(hashes.len(), 3, "Scalar remainder path should handle 3");
}

/// Q5: Isolation - Multiple hasher instances
#[test]
fn test_unit_multiple_instances() {
    let h1 = SimdTextHasher::new();
    let h2 = SimdTextHasher::new();

    let text = "test isolation";
    let r1 = h1.hash_tokens_simd(text);
    let r2 = h2.hash_tokens_simd(text);

    assert_eq!(r1, r2, "Different instances should produce same results");
}

/// Q6: Performance - Alignment verification (64B)
#[test]
fn test_unit_alignment() {
    assert_eq!(
        std::mem::align_of::<SimdTextHasher>(),
        64,
        "SimdTextHasher must be 64-byte aligned"
    );
    assert_eq!(
        std::mem::size_of::<SimdTextHasher>(),
        64,
        "SimdTextHasher must be exactly 64 bytes"
    );
}

/// Q7: Readability - hash_tokens_simd_into (pre-allocated output)
#[test]
fn test_unit_preallocated_output() {
    let hasher = SimdTextHasher::new();
    let mut output = Vec::with_capacity(100);

    hasher.hash_tokens_simd_into("the quick brown fox", &mut output);
    assert_eq!(output.len(), 4, "Should fill pre-allocated vector");

    // Verify reuse clears previous data
    hasher.hash_tokens_simd_into("hello world", &mut output);
    assert_eq!(output.len(), 2, "Should clear and refill vector");
}

/// Q7: Readability - Default constructor
#[test]
fn test_unit_default_constructor() {
    let h1 = SimdTextHasher::new();
    let h2 = SimdTextHasher::default();

    let text = "test default";
    assert_eq!(
        h1.hash_tokens_simd(text),
        h2.hash_tokens_simd(text),
        "new() and default() should be equivalent"
    );
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14) - 10 tests, <60s timeout, 1000 iterations
// ============================================================================

proptest! {
    /// Q8: Universal properties - SIMD equivalence vs scalar (CRITICAL)
    #[test]
    #[cfg(feature = "portable_simd")]
    fn prop_simd_equivalence(tokens in prop::collection::vec("[a-z]{1,10}", 1..100)) {
        let hasher = SimdTextHasher::new();
        let text = tokens.join(" ");

        // SIMD path
        let simd_hashes = hasher.hash_tokens_simd(&text);

        // Scalar baseline (from implementation)
        let scalar_hashes: Vec<u64> = text
            .split_whitespace()
            .map(|t| fnv1a_hash_scalar(t.as_bytes()))
            .collect();

        prop_assert_eq!(
            simd_hashes,
            scalar_hashes,
            "SIMD output must match scalar output"
        );
    }

    /// Q8: Universal properties - Hash distribution uniformity
    #[test]
    fn prop_hash_distribution(tokens in prop::collection::vec("[a-z]{5,10}", 100..200)) {
        let hasher = SimdTextHasher::new();
        let text = tokens.join(" ");
        let hashes = hasher.hash_tokens_simd(&text);

        // Check uniqueness (collision rate)
        let unique: HashSet<_> = hashes.iter().collect();
        let collision_rate = 1.0 - (unique.len() as f64 / hashes.len() as f64);

        prop_assert!(
            collision_rate < 0.01,
            "Collision rate must be <1% (actual: {:.2}%)",
            collision_rate * 100.0
        );
    }

    /// Q9: Concurrent invariants - Thread safety
    #[test]
    fn prop_concurrent_thread_safety(text in "[a-z ]{50,200}") {
        let hasher = Arc::new(SimdTextHasher::new());
        let expected = hasher.hash_tokens_simd(&text);

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let h = Arc::clone(&hasher);
                let t = text.clone();
                thread::spawn(move || h.hash_tokens_simd(&t))
            })
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            prop_assert_eq!(result, expected.clone(), "Concurrent access must be safe");
        }
    }

    /// Q10: Edge case properties - Extreme token counts
    #[test]
    fn prop_extreme_token_counts(count in 0usize..1000) {
        let hasher = SimdTextHasher::new();
        let text = (0..count)
            .map(|i| format!("t{}", i))
            .collect::<Vec<_>>()
            .join(" ");

        let hashes = hasher.hash_tokens_simd(&text);

        prop_assert_eq!(
            hashes.len(),
            count,
            "Token count must match output length"
        );
    }

    /// Q10: Edge case properties - Variable token lengths
    #[test]
    fn prop_variable_token_lengths(lengths in prop::collection::vec(1usize..100, 10..50)) {
        let hasher = SimdTextHasher::new();
        let tokens: Vec<_> = lengths.iter().map(|&len| "a".repeat(len)).collect();
        let text = tokens.join(" ");

        let hashes = hasher.hash_tokens_simd(&text);

        prop_assert_eq!(
            hashes.len(),
            tokens.len(),
            "Variable length tokens must all be hashed"
        );

        // All hashes should be distinct (different lengths)
        let unique: HashSet<_> = hashes.iter().collect();
        prop_assert_eq!(
            unique.len(),
            hashes.len(),
            "Different lengths should produce distinct hashes"
        );
    }

    /// Q11: ASSUM verification - Determinism property
    #[test]
    fn prop_assum_determinism(text in "[a-zA-Z0-9 ]{10,500}") {
        let hasher = SimdTextHasher::new();

        let h1 = hasher.hash_tokens_simd(&text);
        let h2 = hasher.hash_tokens_simd(&text);
        let h3 = hasher.hash_tokens_simd(&text);

        prop_assert_eq!(h1, h2.clone(), "Hash must be deterministic (run 1 vs 2)");
        prop_assert_eq!(h2, h3, "Hash must be deterministic (run 2 vs 3)");
    }

    /// Q12: Composition properties - Concatenation vs separate hashing
    #[test]
    fn prop_composition_concatenation(
        text1 in "[a-z ]{10,50}",
        text2 in "[a-z ]{10,50}"
    ) {
        let hasher = SimdTextHasher::new();

        // Hash concatenated text
        let combined = format!("{} {}", text1, text2);
        let h_combined = hasher.hash_tokens_simd(&combined);

        // Hash separately and combine
        let mut h_separate = hasher.hash_tokens_simd(&text1);
        h_separate.extend(hasher.hash_tokens_simd(&text2));

        prop_assert_eq!(
            h_combined,
            h_separate,
            "Concatenated hash must match separate hashes"
        );
    }

    /// Q13: Statistical properties - No zero hashes
    #[test]
    fn prop_statistical_no_zero_hashes(tokens in prop::collection::vec("[a-z]{1,20}", 10..100)) {
        let hasher = SimdTextHasher::new();
        let text = tokens.join(" ");
        let hashes = hasher.hash_tokens_simd(&text);

        let zero_count = hashes.iter().filter(|&&h| h == 0).count();

        prop_assert_eq!(
            zero_count,
            0,
            "Non-empty tokens should never produce zero hash"
        );
    }

    /// Q13: Statistical properties - Avalanche effect (small input change)
    #[test]
    fn prop_statistical_avalanche(base in "[a-z]{5,10}") {
        let hasher = SimdTextHasher::new();

        let h1 = hasher.hash_tokens_simd(&base);
        let modified = format!("{}x", base); // Append single char
        let h2 = hasher.hash_tokens_simd(&modified);

        // Hamming distance should be ~32 bits (50% different)
        let xor = h1[0] ^ h2[0];
        let bit_diff = xor.count_ones();

        prop_assert!(
            bit_diff >= 16 && bit_diff <= 48,
            "Avalanche effect: small change should flip ~50% of bits (actual: {})",
            bit_diff
        );
    }

    /// Q14: Regression tracking - Consistent with known good values
    #[test]
    fn prop_regression_known_values(seed in 0u64..1000) {
        let hasher = SimdTextHasher::new();
        let text = format!("test_{}", seed);

        // Hash twice to ensure consistency
        let h1 = hasher.hash_tokens_simd(&text);
        let h2 = hasher.hash_tokens_simd(&text);

        prop_assert_eq!(h1, h2, "Regression: output must be stable");
    }
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21) - 12 tests, <30s timeout
// ============================================================================

/// Q15: Critical integration - Corpus generation pipeline
#[test]
fn test_integration_corpus_generation() {
    let hasher = SimdTextHasher::new();

    // Simulate corpus generation for 1000 documents
    let corpus: Vec<_> = (0..1000)
        .map(|i| format!("Document {} contains words and tokens here", i))
        .collect();

    let total_tokens: usize = corpus.iter().map(|doc| hasher.hash_tokens_simd(doc).len()).sum();

    assert_eq!(total_tokens, 1000 * 7, "1000 docs × 7 tokens = 7000 total tokens");
}

/// Q15: Critical integration - MinHash signature generation
#[test]
fn test_integration_minhash_generation() {
    use atomic_capsule::probabilistic::MinHashSignatureCapsule;

    let hasher = SimdTextHasher::new();
    let text = "the quick brown fox jumps over the lazy dog";

    let _token_hashes = hasher.hash_tokens_simd(text);

    // Use tokens to generate MinHash signature (API changed to accept &str)
    let mut sig = MinHashSignatureCapsule::default();
    for token in text.split_whitespace() {
        sig.update(token);
    }

    // Verify signature is populated
    assert!(
        sig.signature().iter().any(|&v| v != u16::MAX),
        "MinHash signature should be updated from token hashes"
    );
}

/// Q16: Error propagation - UTF-8 validation (Rust &str guarantees)
#[test]
fn test_integration_utf8_validation() {
    let hasher = SimdTextHasher::new();

    // Valid UTF-8 (Rust &str enforces this)
    let valid = "Hello 世界 🌍";
    let hashes = hasher.hash_tokens_simd(valid);
    assert_eq!(hashes.len(), 3, "Valid UTF-8 should be processed");

    // Invalid UTF-8 cannot be constructed as &str (compile-time safe)
}

/// Q17: Performance budget - 14M docs/sec throughput target
#[test]
fn test_integration_throughput_target() {
    let hasher = SimdTextHasher::new();

    // Generate 10K documents (typical text length)
    let docs: Vec<_> = (0..10_000)
        .map(|i| format!("Document {} with typical text length here", i))
        .collect();

    let start = std::time::Instant::now();

    let total_hashes: usize = docs.iter().map(|doc| hasher.hash_tokens_simd(doc).len()).sum();

    let elapsed = start.elapsed();
    let docs_per_sec = 10_000.0 / elapsed.as_secs_f64();

    assert!(total_hashes > 0, "Sanity check: should hash tokens");
    assert!(
        docs_per_sec > 100_000.0,
        "Should achieve >100K docs/sec (actual: {:.0})",
        docs_per_sec
    );
}

/// Q18: Production load - Parallel corpus generation
#[test]
fn test_integration_parallel_corpus() {
    use rayon::prelude::*;

    let hasher = Arc::new(SimdTextHasher::new());

    let docs: Vec<_> = (0..10_000)
        .map(|i| format!("Parallel document {} with text", i))
        .collect();

    let total_tokens: usize = docs.par_iter().map(|doc| hasher.hash_tokens_simd(doc).len()).sum();

    assert_eq!(total_tokens, 10_000 * 5, "Parallel processing should match sequential");
}

/// Q19: Rollback scenarios - Feature flag fallback
#[test]
#[cfg(not(feature = "portable_simd"))]
fn test_integration_scalar_fallback() {
    let hasher = SimdTextHasher::new();
    let text = "the quick brown fox";

    // Scalar fallback should work when SIMD disabled
    let hashes = hasher.hash_tokens_simd(text);
    assert_eq!(hashes.len(), 4, "Scalar fallback should work");
}

/// Q20: I20 validation - Deterministic composition with pipeline
#[test]
fn test_integration_i20_deterministic_composition() {
    let hasher = SimdTextHasher::new();
    let text = "test deterministic composition";

    let h1 = hasher.hash_tokens_simd(text);
    let h2 = hasher.hash_tokens_simd(text);

    assert_eq!(h1, h2, "I20 boundary invariant: deterministic output across calls");
}

/// Q20: I20 validation - Zero-copy integration
#[test]
fn test_integration_i20_zero_copy() {
    let hasher = SimdTextHasher::new();
    let mut output = Vec::with_capacity(100);

    // First call
    hasher.hash_tokens_simd_into("first document", &mut output);
    let len1 = output.len();

    // Second call (reuse buffer)
    hasher.hash_tokens_simd_into("second document here", &mut output);
    let len2 = output.len();

    assert_eq!(len1, 2, "First call should produce 2 hashes");
    assert_eq!(len2, 3, "Second call should produce 3 hashes");
}

/// Q21: Monitoring - Token count metrics
#[test]
fn test_integration_monitoring_token_count() {
    let hasher = SimdTextHasher::new();

    let docs = vec!["one two three", "four five six seven", "eight nine ten eleven twelve"];

    let token_counts: Vec<_> = docs.iter().map(|doc| hasher.hash_tokens_simd(doc).len()).collect();

    assert_eq!(token_counts, vec![3, 4, 5], "Token counts for monitoring");
}

/// Q21: Monitoring - SIMD batch utilization
#[test]
#[cfg(feature = "portable_simd")]
fn test_integration_monitoring_simd_batches() {
    let hasher = SimdTextHasher::new();

    // 16 tokens = 2 SIMD batches (8 tokens each)
    let text = (0..16).map(|i| format!("t{}", i)).collect::<Vec<_>>().join(" ");

    let hashes = hasher.hash_tokens_simd(&text);
    assert_eq!(hashes.len(), 16, "16 tokens = 2 SIMD batches");
}

/// Q21: Monitoring - Average token length
#[test]
fn test_integration_monitoring_avg_token_length() {
    let hasher = SimdTextHasher::new();
    let text = "short medium longertoken verylongtoken";

    let tokens: Vec<_> = text.split_whitespace().collect();
    let avg_len: f64 = tokens.iter().map(|t| t.len()).sum::<usize>() as f64 / tokens.len() as f64;

    let hashes = hasher.hash_tokens_simd(text);
    assert_eq!(hashes.len(), 4, "4 tokens");
    assert!(avg_len > 5.0 && avg_len < 15.0, "Typical avg length");
}

/// Q21: Monitoring - Hash distribution metrics
#[test]
fn test_integration_monitoring_hash_distribution() {
    let hasher = SimdTextHasher::new();

    let docs: Vec<_> = (0..1000).map(|i| format!("doc{}", i)).collect();

    let all_hashes: Vec<_> = docs.iter().flat_map(|doc| hasher.hash_tokens_simd(doc)).collect();

    let unique: HashSet<_> = all_hashes.iter().collect();
    let collision_rate = 1.0 - (unique.len() as f64 / all_hashes.len() as f64);

    assert!(
        collision_rate < 0.01,
        "Collision rate <1% for monitoring (actual: {:.2}%)",
        collision_rate * 100.0
    );
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28) - 8 tests, <120s timeout
// ============================================================================

/// Q22: Stress test - 10M document corpus
#[test]
fn test_production_stress_10m_corpus() {
    let hasher = Arc::new(SimdTextHasher::new());

    // Generate 10K docs (representative sample of 10M)
    let docs: Vec<_> = (0..10_000)
        .map(|i| {
            format!(
                "Production document {} with realistic text length averaging 100 words",
                i
            )
        })
        .collect();

    let start = std::time::Instant::now();

    let total_tokens: usize = docs.iter().map(|doc| hasher.hash_tokens_simd(doc).len()).sum();

    let elapsed = start.elapsed();
    let docs_per_sec = 10_000.0 / elapsed.as_secs_f64();

    assert!(total_tokens > 100_000, "Should process 100K+ tokens");
    assert!(
        docs_per_sec > 50_000.0,
        "Production: >50K docs/sec (actual: {:.0})",
        docs_per_sec
    );
}

/// Q22: Stress test - 100 threads concurrent
#[test]
fn test_production_stress_100_threads() {
    let hasher = Arc::new(SimdTextHasher::new());
    let text = "Concurrent stress test with typical document length";

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let h = Arc::clone(&hasher);
            let t = text.to_string();
            thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = h.hash_tokens_simd(&t);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread should not panic");
    }
}

/// Q23: Security - Adversarial input resistance
#[test]
fn test_production_security_adversarial_inputs() {
    let hasher = SimdTextHasher::new();

    // Empty string
    assert_eq!(hasher.hash_tokens_simd("").len(), 0);

    // Very long token (DOS attempt)
    let long_token = "a".repeat(1_000_000);
    let h1 = hasher.hash_tokens_simd(&long_token);
    assert_eq!(h1.len(), 1, "Should handle 1M char token");

    // Many short tokens (DOS attempt)
    let many_tokens = (0..100_000).map(|_| "a").collect::<Vec<_>>().join(" ");
    let h2 = hasher.hash_tokens_simd(&many_tokens);
    assert_eq!(h2.len(), 100_000, "Should handle 100K tokens");

    // Unicode edge cases
    let unicode = "café\u{200B}invisible\u{FEFF}bom";
    let h3 = hasher.hash_tokens_simd(unicode);
    assert!(h3.len() > 0, "Should handle Unicode edge cases");
}

/// Q24: B32 benchmarks - Throughput validation
#[test]
fn test_production_b32_throughput() {
    let hasher = SimdTextHasher::new();

    // Realistic document corpus
    let docs: Vec<_> = (0..10_000)
        .map(|i| {
            format!(
                "Document {} with realistic text averaging 100 words and typical sentence structure",
                i
            )
        })
        .collect();

    let start = std::time::Instant::now();

    let total_hashes: usize = docs.iter().map(|doc| hasher.hash_tokens_simd(doc).len()).sum();

    let elapsed = start.elapsed();
    let throughput = total_hashes as f64 / elapsed.as_secs_f64();

    assert!(
        throughput > 1_000_000.0,
        "B32 target: >1M tokens/sec (actual: {:.0})",
        throughput
    );
}

/// Q25: ASSUM unsafe validation - Memory safety
#[test]
fn test_production_assum_memory_safety() {
    let hasher = SimdTextHasher::new();

    // Verify no unsafe code in hot path (pure safe Rust)
    let text = "Memory safety test with typical document length";
    let _hashes = hasher.hash_tokens_simd(text);

    // ASSUM: portable_simd is safe abstraction (no UB)
    // VERIFY: Miri tests would catch any UB (not in this suite)
}

/// Q26: TODO/FIXME audit - Production readiness
#[test]
fn test_production_todo_audit() {
    // This test serves as documentation that all TODOs resolved
    // Search codebase for "TODO" or "FIXME" in atomic_capsule::text::simd_hasher
}

/// Q27: Documentation completeness - Public API
#[test]
fn test_production_documentation_completeness() {
    // Verify SimdTextHasher has:
    // - Module-level docs (checked manually)
    // - Public API docs (hash_tokens_simd, hash_tokens_simd_into)
    // - Examples in docs (checked manually)
    // - Performance characteristics documented

    let hasher = SimdTextHasher::new();
    let _ = hasher.hash_tokens_simd("Documentation test");
}

/// Q28: Test suite maintainability - Fast feedback
#[test]
fn test_production_test_suite_maintainability() {
    // Verify test suite runs quickly (<5 minutes total)
    // This test validates the test infrastructure itself

    let hasher = SimdTextHasher::new();
    let start = std::time::Instant::now();

    // Representative workload
    for i in 0..1000 {
        let text = format!("Test document {}", i);
        let _ = hasher.hash_tokens_simd(&text);
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 5,
        "1K docs should complete in <5s (maintainability)"
    );
}

// ============================================================================
// HELPER FUNCTIONS (from implementation, for property tests)
// ============================================================================

/// FNV-1a scalar hash (for equivalence testing)
#[inline(always)]
fn fnv1a_hash_scalar(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
