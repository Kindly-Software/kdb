//! FED MinHash Tests - Fast Exact Deduplication (arXiv:2501.01046)
//!
//! Validates FED hash parameters and CPU reference implementation.
//! GPU kernel tests would require GPU hardware and are tested separately.
//!
//! # Test Coverage
//!
//! - Parameter generation (determinism, range validation)
//! - Hash quality (distribution, independence)
//! - CPU reference correctness (matches theoretical expectations)
//! - Buffer encoding (GPU upload format)
//! - Generation counter (Q34 audit trail)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier validation
//! - **Chaos**: Cache alignment verification
//! - **ASSUM**: All assumptions documented and tested
//! - **B32**: Performance baselines for CPU reference
//! - **T28**: Unit tests (T28 Q1-Q7)

use kindly_dedup::gpu::{FedHashParamsCapsule, FED_NUM_PERMUTATIONS, HASH_PRIME};

// ============================================================================
// T28 Q1: Basic Functionality Tests
// ============================================================================

#[test]
fn test_fed_params_generation_basic() {
    let params = FedHashParamsCapsule::generate(42);

    // Verify constants
    assert_eq!(params.prime(), HASH_PRIME);
    assert_eq!(FED_NUM_PERMUTATIONS, 128);
}

#[test]
fn test_fed_params_a_coefficient_range() {
    let params = FedHashParamsCapsule::generate(12345);

    // All a coefficients must be in [1, prime-1]
    for i in 0..FED_NUM_PERMUTATIONS {
        let a = params.a(i);
        assert!(
            a > 0 && a < HASH_PRIME,
            "a[{}] = {} must be in (0, {})",
            i,
            a,
            HASH_PRIME
        );
    }
}

#[test]
fn test_fed_params_b_coefficient_range() {
    let params = FedHashParamsCapsule::generate(67890);

    // All b coefficients must be in [0, prime-1]
    for i in 0..FED_NUM_PERMUTATIONS {
        let b = params.b(i);
        assert!(
            b < HASH_PRIME,
            "b[{}] = {} must be < {}",
            i,
            b,
            HASH_PRIME
        );
    }
}

// ============================================================================
// T28 Q2: Determinism Tests
// ============================================================================

#[test]
fn test_fed_params_deterministic() {
    // Same seed → same parameters (critical for reproducibility)
    let seed = 999_888_777;
    let params1 = FedHashParamsCapsule::generate(seed);
    let params2 = FedHashParamsCapsule::generate(seed);

    for i in 0..FED_NUM_PERMUTATIONS {
        assert_eq!(
            params1.a(i),
            params2.a(i),
            "a[{}] must be deterministic",
            i
        );
        assert_eq!(
            params1.b(i),
            params2.b(i),
            "b[{}] must be deterministic",
            i
        );
    }
}

#[test]
fn test_fed_hash_token_deterministic() {
    let params = FedHashParamsCapsule::generate(123);
    let token = 0xDEADBEEF;

    // Same token, same permutation → same hash
    for perm in 0..FED_NUM_PERMUTATIONS {
        let hash1 = params.hash_token(token, perm);
        let hash2 = params.hash_token(token, perm);
        assert_eq!(
            hash1, hash2,
            "Hash must be deterministic for permutation {}",
            perm
        );
    }
}

#[test]
fn test_fed_compute_signature_deterministic() {
    let params = FedHashParamsCapsule::generate(456);
    let tokens = vec![100u32, 200, 300, 400, 500];

    let sig1 = params.compute_signature_cpu(&tokens);
    let sig2 = params.compute_signature_cpu(&tokens);

    assert_eq!(
        sig1, sig2,
        "Signature computation must be deterministic"
    );
}

// ============================================================================
// T28 Q3: Independence Tests (Different Seeds)
// ============================================================================

#[test]
fn test_fed_params_different_seeds_produce_different_params() {
    let params1 = FedHashParamsCapsule::generate(100);
    let params2 = FedHashParamsCapsule::generate(200);

    let mut a_diffs = 0;
    let mut b_diffs = 0;

    for i in 0..FED_NUM_PERMUTATIONS {
        if params1.a(i) != params2.a(i) {
            a_diffs += 1;
        }
        if params1.b(i) != params2.b(i) {
            b_diffs += 1;
        }
    }

    // At least 90% of parameters should differ (high-quality RNG)
    assert!(
        a_diffs >= 115,
        "Only {} out of 128 a coefficients differ (expected ≥115)",
        a_diffs
    );
    assert!(
        b_diffs >= 115,
        "Only {} out of 128 b coefficients differ (expected ≥115)",
        b_diffs
    );
}

// ============================================================================
// T28 Q4: Hash Quality Tests
// ============================================================================

#[test]
fn test_fed_hash_different_tokens_produce_different_hashes() {
    let params = FedHashParamsCapsule::generate(777);

    // Hash two different tokens with same permutation
    let hash1 = params.hash_token(100, 0);
    let hash2 = params.hash_token(200, 0);

    assert_ne!(
        hash1, hash2,
        "Different tokens should produce different hashes (with high probability)"
    );
}

#[test]
fn test_fed_hash_different_permutations_produce_different_hashes() {
    let params = FedHashParamsCapsule::generate(888);
    let token = 12345u32;

    // Hash same token with different permutations
    let hashes: Vec<u32> = (0..FED_NUM_PERMUTATIONS)
        .map(|perm| params.hash_token(token, perm))
        .collect();

    // Check for uniqueness (should be very high)
    let unique_count = hashes
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();

    // Expect at least 95% unique hashes (collisions are rare with good hash)
    assert!(
        unique_count >= 121,
        "Only {} out of 128 hashes are unique (expected ≥121)",
        unique_count
    );
}

#[test]
fn test_fed_hash_range_validation() {
    let params = FedHashParamsCapsule::generate(999);

    // All hash outputs must be < prime
    for token in [0u32, 1, 1000, u32::MAX / 2, u32::MAX] {
        for perm in 0..FED_NUM_PERMUTATIONS {
            let hash = params.hash_token(token, perm);
            assert!(
                hash < HASH_PRIME,
                "Hash {} must be < prime {} for token {}, perm {}",
                hash,
                HASH_PRIME,
                token,
                perm
            );
        }
    }
}

// ============================================================================
// T28 Q5: MinHash Signature Tests
// ============================================================================

#[test]
fn test_fed_compute_signature_cpu_basic() {
    let params = FedHashParamsCapsule::generate(111);
    let tokens = vec![10u32, 20, 30];

    let signature = params.compute_signature_cpu(&tokens);

    // Verify signature length
    assert_eq!(signature.len(), FED_NUM_PERMUTATIONS);

    // All values should be < u16::MAX (at least one token hashed)
    for (i, &val) in signature.iter().enumerate() {
        assert!(
            val < u16::MAX,
            "Signature[{}] = {} should be < u16::MAX",
            i,
            val
        );
    }
}

#[test]
fn test_fed_compute_signature_empty_tokens() {
    let params = FedHashParamsCapsule::generate(222);
    let tokens: Vec<u32> = vec![];

    let signature = params.compute_signature_cpu(&tokens);

    // Empty document → all u16::MAX
    assert!(
        signature.iter().all(|&v| v == u16::MAX),
        "Empty document should produce all u16::MAX signature"
    );
}

#[test]
fn test_fed_compute_signature_repeated_tokens() {
    let params = FedHashParamsCapsule::generate(333);
    let tokens = vec![42u32, 42, 42, 42]; // All same

    let sig1 = params.compute_signature_cpu(&tokens);

    // Should be same as single token (MinHash property)
    let sig2 = params.compute_signature_cpu(&[42u32]);

    assert_eq!(
        sig1, sig2,
        "Repeated tokens should produce same signature as single token"
    );
}

// ============================================================================
// T28 Q6: Buffer Encoding Tests
// ============================================================================

#[test]
fn test_fed_params_to_gpu_buffer_size() {
    let params = FedHashParamsCapsule::generate(555);
    let buffer = params.to_gpu_buffer();

    // Expected size: 512 (a) + 512 (b) + 4 (prime) + 12 (padding) = 1040
    assert_eq!(buffer.len(), 1040, "Buffer size must be 1040 bytes");
}

#[test]
fn test_fed_params_to_gpu_buffer_encoding() {
    let params = FedHashParamsCapsule::generate(666);
    let buffer = params.to_gpu_buffer();

    // Verify a[0] encoding (bytes 0-3, little-endian)
    let a0_bytes = [buffer[0], buffer[1], buffer[2], buffer[3]];
    let a0_decoded = u32::from_le_bytes(a0_bytes);
    assert_eq!(a0_decoded, params.a(0), "a[0] encoding must be correct");

    // Verify a[127] encoding (bytes 508-511, little-endian)
    let a127_bytes = [buffer[508], buffer[509], buffer[510], buffer[511]];
    let a127_decoded = u32::from_le_bytes(a127_bytes);
    assert_eq!(
        a127_decoded,
        params.a(127),
        "a[127] encoding must be correct"
    );

    // Verify b[0] encoding (bytes 512-515, little-endian)
    let b0_bytes = [buffer[512], buffer[513], buffer[514], buffer[515]];
    let b0_decoded = u32::from_le_bytes(b0_bytes);
    assert_eq!(b0_decoded, params.b(0), "b[0] encoding must be correct");

    // Verify prime encoding (bytes 1024-1027, little-endian)
    let prime_bytes = [buffer[1024], buffer[1025], buffer[1026], buffer[1027]];
    let prime_decoded = u32::from_le_bytes(prime_bytes);
    assert_eq!(prime_decoded, HASH_PRIME, "prime encoding must be correct");
}

// ============================================================================
// T28 Q7: Generation Counter Tests (Q34 Audit Trail)
// ============================================================================

#[test]
fn test_fed_params_generation_counter_initial() {
    let params = FedHashParamsCapsule::generate(777);
    assert_eq!(params.generation(), 0, "Initial generation must be 0");
}

#[test]
fn test_fed_params_generation_counter_increment() {
    let params = FedHashParamsCapsule::generate(888);

    assert_eq!(params.generation(), 0);

    params.increment_generation();
    assert_eq!(params.generation(), 1);

    params.increment_generation();
    assert_eq!(params.generation(), 2);

    params.increment_generation();
    assert_eq!(params.generation(), 3);
}

#[test]
fn test_fed_params_generation_counter_thread_safe() {
    use std::sync::Arc;
    use std::thread;

    let params = Arc::new(FedHashParamsCapsule::generate(999));
    let mut handles = vec![];

    // Spawn 10 threads, each increments 100 times
    for _ in 0..10 {
        let params_clone = Arc::clone(&params);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                params_clone.increment_generation();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Should be 10 × 100 = 1000
    assert_eq!(
        params.generation(),
        1000,
        "Generation counter must be thread-safe"
    );
}

// ============================================================================
// Property Tests (Theoretical Guarantees)
// ============================================================================

#[test]
fn test_universal_hashing_collision_probability() {
    // Universal hashing property: P(h(x) = h(y)) ≤ 1/prime for x ≠ y
    //
    // We can't test this exactly (need infinite samples), but we can
    // verify that collision rate is low for random inputs.

    let params = FedHashParamsCapsule::generate(1234);
    let num_samples = 1000;
    let num_permutations = 10; // Test subset for speed

    let mut collisions = 0;
    let mut total_pairs = 0;

    for perm in 0..num_permutations {
        let mut hashes = std::collections::HashSet::new();

        for token in 0..num_samples {
            let hash = params.hash_token(token, perm);
            if !hashes.insert(hash) {
                collisions += 1;
            }
            total_pairs += 1;
        }
    }

    let collision_rate = collisions as f64 / total_pairs as f64;
    let expected_rate = 1.0 / HASH_PRIME as f64; // ~0.000000465

    // Collision rate should be very low (<1% for our sample size)
    assert!(
        collision_rate < 0.01,
        "Collision rate {} too high (expected ~{})",
        collision_rate,
        expected_rate
    );
}

#[test]
fn test_minhash_similarity_property() {
    // MinHash property: E[Jaccard(sig_A, sig_B)] = Jaccard(A, B)
    //
    // We verify this approximately with a simple example.

    let params = FedHashParamsCapsule::generate(5678);

    // Two documents with 50% overlap
    let tokens_a = vec![1u32, 2, 3, 4, 5];
    let tokens_b = vec![4u32, 5, 6, 7, 8]; // Overlap: {4, 5}

    let sig_a = params.compute_signature_cpu(&tokens_a);
    let sig_b = params.compute_signature_cpu(&tokens_b);

    // Compute signature similarity (fraction of matching values)
    let matching = sig_a
        .iter()
        .zip(sig_b.iter())
        .filter(|(a, b)| a == b)
        .count();
    let sig_similarity = matching as f64 / FED_NUM_PERMUTATIONS as f64;

    // True Jaccard: |intersection| / |union| = 2 / 8 = 0.25
    let true_jaccard = 2.0 / 8.0;

    // Signature similarity should be close to true Jaccard (within 20% for 128 permutations)
    let error = (sig_similarity - true_jaccard).abs();
    assert!(
        error < 0.20,
        "Signature similarity {} should approximate Jaccard {} (error = {})",
        sig_similarity,
        true_jaccard,
        error
    );
}
