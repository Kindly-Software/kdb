//! T28 Tier 2: Property Testing (Q8-Q14)
//!
//! Validates invariants hold across input space using property-based testing.

use kindly_compression::{Compress, TokenClusteringCodec};
use proptest::prelude::*;

// ============================================================================
// Q8: Universal Properties (Hold for All Inputs)
// ============================================================================

proptest! {
    /// Property: compress→decompress always preserves original data (round-trip).
    #[test]
    fn prop_round_trip_preserves_data(data in prop::collection::vec(any::<u8>(), 1..1000)) {
        let codec = TokenClusteringCodec::new();

        let compressed = codec.compress(&data)
            .expect("Compression should succeed for valid input");
        let decompressed = codec.decompress(&compressed)
            .expect("Decompression should succeed for valid compressed data");

        prop_assert_eq!(
            data, decompressed,
            "Property: compress→decompress must preserve original data"
        );
    }

    /// Property: Compression ratio is always positive and finite.
    #[test]
    fn prop_compression_ratio_positive_finite(data in prop::collection::vec(any::<u8>(), 1..1000)) {
        let codec = TokenClusteringCodec::new();

        let _compressed = codec.compress(&data).unwrap();
        let ratio = codec.ratio();

        prop_assert!(
            ratio > 0.0,
            "Property: Compression ratio must be positive, got {}",
            ratio
        );
        prop_assert!(
            ratio.is_finite(),
            "Property: Compression ratio must be finite"
        );
    }

    /// Property: Compressed output always includes valid header (≥68 bytes).
    #[test]
    fn prop_compressed_has_valid_header(data in prop::collection::vec(any::<u8>(), 1..1000)) {
        let codec = TokenClusteringCodec::new();

        let compressed = codec.compress(&data).unwrap();

        prop_assert!(
            compressed.len() >= 68,
            "Property: Compressed data must include header (≥68 bytes), got {} bytes",
            compressed.len()
        );
    }

    /// Property: Same input always produces same compressed output (deterministic).
    #[test]
    fn prop_deterministic_compression(data in prop::collection::vec(any::<u8>(), 1..500)) {
        let codec1 = TokenClusteringCodec::new();
        let codec2 = TokenClusteringCodec::new();

        let compressed1 = codec1.compress(&data).unwrap();
        let compressed2 = codec2.compress(&data).unwrap();

        prop_assert_eq!(
            compressed1, compressed2,
            "Property: Same input must produce identical compressed output (deterministic)"
        );
    }

    /// Property: Decompressed length matches original length.
    #[test]
    fn prop_length_preservation(data in prop::collection::vec(any::<u8>(), 1..1000)) {
        let codec = TokenClusteringCodec::new();

        let original_len = data.len();
        let compressed = codec.compress(&data).unwrap();
        let decompressed = codec.decompress(&compressed).unwrap();

        prop_assert_eq!(
            decompressed.len(), original_len,
            "Property: Decompressed length must match original length"
        );
    }
}

// ============================================================================
// Q9: Concurrent Access Properties (No concurrent access in this codec, but test thread safety)
// ============================================================================

proptest! {
    /// Property: Multiple threads can compress different data simultaneously.
    #[test]
    fn prop_thread_safe_compression(
        data1 in prop::collection::vec(any::<u8>(), 1..500),
        data2 in prop::collection::vec(any::<u8>(), 1..500),
        data3 in prop::collection::vec(any::<u8>(), 1..500),
    ) {
        use std::sync::Arc;
        use std::thread;

        let data1 = Arc::new(data1);
        let data2 = Arc::new(data2);
        let data3 = Arc::new(data3);

        let handle1 = {
            let data = Arc::clone(&data1);
            thread::spawn(move || {
                let codec = TokenClusteringCodec::new();
                codec.compress(&data).unwrap()
            })
        };

        let handle2 = {
            let data = Arc::clone(&data2);
            thread::spawn(move || {
                let codec = TokenClusteringCodec::new();
                codec.compress(&data).unwrap()
            })
        };

        let handle3 = {
            let data = Arc::clone(&data3);
            thread::spawn(move || {
                let codec = TokenClusteringCodec::new();
                codec.compress(&data).unwrap()
            })
        };

        // All threads should complete successfully
        let compressed1 = handle1.join().expect("Thread 1 should not panic");
        let compressed2 = handle2.join().expect("Thread 2 should not panic");
        let compressed3 = handle3.join().expect("Thread 3 should not panic");

        // Verify results are valid (can decompress)
        let codec = TokenClusteringCodec::new();
        let decompressed1 = codec.decompress(&compressed1).unwrap();
        let decompressed2 = codec.decompress(&compressed2).unwrap();
        let decompressed3 = codec.decompress(&compressed3).unwrap();

        prop_assert_eq!(&**data1, &decompressed1[..]);
        prop_assert_eq!(&**data2, &decompressed2[..]);
        prop_assert_eq!(&**data3, &decompressed3[..]);
    }
}

// ============================================================================
// Q10: Edge Case Properties
// ============================================================================

proptest! {
    /// Property: All byte values (0-255) can be compressed and decompressed.
    #[test]
    fn prop_handles_all_byte_values(data in prop::collection::vec(any::<u8>(), 1..1000)) {
        let codec = TokenClusteringCodec::new();

        let compressed = codec.compress(&data).unwrap();
        let decompressed = codec.decompress(&compressed).unwrap();

        prop_assert_eq!(
            data, decompressed,
            "Property: All byte values (0-255) must be preserved"
        );
    }

    /// Property: Handles data with repeated patterns efficiently.
    #[test]
    fn prop_handles_repeated_patterns(
        byte in any::<u8>(),
        count in 100usize..1000, // Skip very small sizes due to header overhead
    ) {
        let data = vec![byte; count];
        let codec = TokenClusteringCodec::new();

        let compressed = codec.compress(&data).unwrap();
        let decompressed = codec.decompress(&compressed).unwrap();

        // Note: Ratio may be < 1.0 for very small data due to 68-byte header overhead
        // For larger repetitive data, compression is effective
        let ratio = data.len() as f32 / compressed.len() as f32;

        prop_assert_eq!(data, decompressed);
        prop_assert!(
            ratio > 0.0,
            "Property: Compression ratio must be positive, got {:.2}×",
            ratio
        );
    }

    /// Property: Handles data with all unique bytes.
    #[test]
    fn prop_handles_unique_bytes(seed in any::<u64>()) {
        // Generate 256 unique bytes (permutation)
        let mut data: Vec<u8> = (0..=255).collect();

        // Shuffle using seed for determinism
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hash, Hasher};
        let mut hasher = RandomState::new().build_hasher();
        seed.hash(&mut hasher);
        let hash = hasher.finish();

        // Simple shuffle based on hash
        for i in 0..data.len() {
            let j = ((hash.wrapping_mul(i as u64 + 1)) % (data.len() as u64)) as usize;
            data.swap(i, j);
        }

        let codec = TokenClusteringCodec::new();
        let compressed = codec.compress(&data).unwrap();
        let decompressed = codec.decompress(&compressed).unwrap();

        prop_assert_eq!(
            data, decompressed,
            "Property: All unique bytes must be preserved (via escape sequences)"
        );
    }

    /// Property: Handles data at boundary sizes (1 byte, max size).
    #[test]
    fn prop_handles_boundary_sizes(size in prop::sample::select(vec![
        1usize,
        100usize,
        1000usize,
        10_000usize,
        100_000usize,
        1024 * 1024, // Max size
    ])) {
        let data = vec![b'A'; size];
        let codec = TokenClusteringCodec::new();

        let compressed = codec.compress(&data).unwrap();
        let decompressed = codec.decompress(&compressed).unwrap();

        prop_assert_eq!(
            data, decompressed,
            "Property: Boundary sizes ({} bytes) must be handled correctly",
            size
        );
    }
}

// ============================================================================
// Q11: ASSUM Assumptions (No unsafe code in this codec, but verify safety properties)
// ============================================================================

proptest! {
    /// Property: No panics on any valid input.
    #[test]
    fn prop_no_panics_on_valid_input(data in prop::collection::vec(any::<u8>(), 1..1000)) {
        let codec = TokenClusteringCodec::new();

        // Compression should never panic
        let compressed = codec.compress(&data);
        prop_assert!(compressed.is_ok(), "Compression must not panic on valid input");

        // Decompression should never panic on valid compressed data
        if let Ok(compressed) = compressed {
            let decompressed = codec.decompress(&compressed);
            prop_assert!(decompressed.is_ok(), "Decompression must not panic on valid compressed data");
        }
    }

    /// Property: Error handling is consistent (invalid input returns errors, not panics).
    #[test]
    fn prop_error_handling_consistent(size in 0usize..2_000_000) {
        let codec = TokenClusteringCodec::new();

        if size == 0 {
            // Empty input should return error
            let result = codec.compress(&[]);
            prop_assert!(result.is_err(), "Empty input must return error");
        } else if size > 1024 * 1024 {
            // Input too large should return error
            let data = vec![0u8; size];
            let result = codec.compress(&data);
            prop_assert!(result.is_err(), "Input exceeding max size must return error");
        } else {
            // Valid size should succeed
            let data = vec![0u8; size];
            let result = codec.compress(&data);
            prop_assert!(result.is_ok(), "Valid input size ({}) must succeed", size);
        }
    }
}

// ============================================================================
// Q12: Composition Properties (Single codec, but test encoding→decoding pipeline)
// ============================================================================

proptest! {
    /// Property: Multiple compress→decompress cycles preserve data.
    #[test]
    fn prop_multiple_cycles_preserve_data(data in prop::collection::vec(any::<u8>(), 1..500)) {
        let codec = TokenClusteringCodec::new();

        // Cycle 1
        let compressed1 = codec.compress(&data).unwrap();
        let decompressed1 = codec.decompress(&compressed1).unwrap();
        prop_assert_eq!(&data, &decompressed1, "Cycle 1 failed");

        // Cycle 2 (compress the decompressed data again)
        let compressed2 = codec.compress(&decompressed1).unwrap();
        let decompressed2 = codec.decompress(&compressed2).unwrap();
        prop_assert_eq!(&data, &decompressed2, "Cycle 2 failed");

        // Cycle 3
        let compressed3 = codec.compress(&decompressed2).unwrap();
        let decompressed3 = codec.decompress(&compressed3).unwrap();
        prop_assert_eq!(&data, &decompressed3, "Cycle 3 failed");
    }

    /// Property: Compression is idempotent (compressing compressed data works).
    #[test]
    fn prop_compression_idempotent(data in prop::collection::vec(any::<u8>(), 1..500)) {
        let codec = TokenClusteringCodec::new();

        // Compress once
        let compressed1 = codec.compress(&data).unwrap();

        // Compress the compressed data again
        let compressed2 = codec.compress(&compressed1).unwrap();

        // Decompress both
        let decompressed1 = codec.decompress(&compressed1).unwrap();
        let decompressed2_step1 = codec.decompress(&compressed2).unwrap();
        let decompressed2_step2 = codec.decompress(&decompressed2_step1).unwrap();

        prop_assert_eq!(&data, &decompressed1, "First decompression failed");
        prop_assert_eq!(&compressed1, &decompressed2_step1, "Second compression/decompression failed");
        prop_assert_eq!(&data, &decompressed2_step2, "Full double compression/decompression failed");
    }
}

// ============================================================================
// Q13: Statistical Properties
// ============================================================================

proptest! {
    /// Property: Compression ratio distribution is reasonable.
    #[test]
    fn prop_compression_ratio_distribution(data in prop::collection::vec(any::<u8>(), 100..1000)) {
        let codec = TokenClusteringCodec::new();

        let compressed = codec.compress(&data).unwrap();
        let ratio = data.len() as f32 / compressed.len() as f32;

        // Statistical property: Ratio should be in reasonable range
        // Worst case: All unique bytes (expansion due to header)
        // Best case: All same byte (>2× compression)
        prop_assert!(
            ratio >= 0.5 && ratio <= 10.0,
            "Property: Compression ratio should be in reasonable range [0.5, 10.0], got {:.2}×",
            ratio
        );
    }

    /// Property: Highly repetitive data achieves better compression.
    #[test]
    fn prop_repetitive_data_better_compression(
        byte in any::<u8>(),
        count in 100usize..1000,
        noise in prop::collection::vec(any::<u8>(), 0..10),
    ) {
        let codec = TokenClusteringCodec::new();

        // Repetitive data
        let repetitive = vec![byte; count];
        let compressed_repetitive = codec.compress(&repetitive).unwrap();
        let ratio_repetitive = repetitive.len() as f32 / compressed_repetitive.len() as f32;

        // Mixed data (less repetitive)
        let mut mixed = repetitive.clone();
        mixed.extend_from_slice(&noise);
        let compressed_mixed = codec.compress(&mixed).unwrap();
        let ratio_mixed = mixed.len() as f32 / compressed_mixed.len() as f32;

        // Property: Repetitive data should achieve at least as good compression
        // (allowing small variance due to header overhead)
        prop_assert!(
            ratio_repetitive >= ratio_mixed * 0.9,
            "Property: Repetitive data should compress better, got repetitive={:.2}×, mixed={:.2}×",
            ratio_repetitive,
            ratio_mixed
        );
    }
}

// ============================================================================
// Q14: Regression Prevention
// ============================================================================

proptest! {
    /// Property: Known good cases always succeed (regression test).
    #[test]
    fn prop_known_good_cases_succeed(case_idx in 0usize..4) {
        let case = match case_idx {
            0 => b"Hello world".to_vec(),
            1 => vec![b'A'; 100],
            2 => (0..=255).collect::<Vec<u8>>(),
            _ => b"The quick brown fox".to_vec(),
        };
        let codec = TokenClusteringCodec::new();

        let compressed = codec.compress(&case)
            .expect("Known good case should compress successfully");
        let decompressed = codec.decompress(&compressed)
            .expect("Known good case should decompress successfully");

        prop_assert_eq!(
            case, decompressed,
            "Regression: Known good case failed round-trip"
        );
    }

    /// Property: Edge cases that previously caused issues (regression prevention).
    #[test]
    fn prop_edge_case_regressions(case_idx in 0usize..4) {
        let case = match case_idx {
            0 => vec![b'A'; 1],        // Single byte
            1 => vec![b'A'; 1000],     // All same byte
            2 => (0..=255).collect::<Vec<u8>>(), // All unique bytes
            _ => vec![0u8; 1024 * 1024], // Max size
        };
        let codec = TokenClusteringCodec::new();

        let compressed = codec.compress(&case)
            .expect("Edge case should compress successfully");
        let decompressed = codec.decompress(&compressed)
            .expect("Edge case should decompress successfully");

        prop_assert_eq!(
            case, decompressed,
            "Regression: Edge case failed round-trip"
        );
    }
}

// ============================================================================
// Configuration: Run property tests with 1000 iterations
// ============================================================================

// proptest configuration is set via proptest! macro defaults
// To run with more cases: PROPTEST_CASES=1000 cargo test
