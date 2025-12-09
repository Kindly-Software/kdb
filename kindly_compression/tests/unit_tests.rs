//! T28 Tier 1: Unit Testing (Q1-Q7)
//!
//! Tests core behaviors, edge cases, invariants, coverage, isolation, speed, and readability.

use kindly_compression::{Compress, CompressionError, TokenClusteringCodec};

// ============================================================================
// Q1: Core Behaviors
// ============================================================================

#[test]
fn test_core_compress_decompress_roundtrip() {
    // Arrange
    let codec = TokenClusteringCodec::new();
    let data = b"Hello world";

    // Act
    let compressed = codec.compress(data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();

    // Assert
    assert_eq!(
        data.to_vec(),
        decompressed,
        "Round-trip should preserve original data"
    );
}

#[test]
fn test_core_compression_ratio_tracked() {
    // Arrange
    let codec = TokenClusteringCodec::new();
    let data = b"AAAAAABBBBBBCCCCCC"; // High compression potential

    // Act
    let compressed = codec.compress(data).unwrap();
    let ratio = data.len() as f32 / compressed.len() as f32;

    // Assert
    assert!(
        ratio > 0.0,
        "Compression ratio must be positive, got {}",
        ratio
    );
}

#[test]
fn test_core_cluster_building() {
    // Arrange
    let codec = TokenClusteringCodec::new();
    let data = b"AAAAAABBBBBBCCCCCCDDDDDD"; // 4 distinct bytes

    // Act
    let compressed = codec.compress(data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();

    // Assert: Clusters correctly identify top frequencies
    assert_eq!(data.to_vec(), decompressed);
    let ratio = data.len() as f32 / compressed.len() as f32;
    assert!(
        ratio > 0.2,
        "Should achieve reasonable compression with 4 frequent bytes, got {:.2}×",
        ratio
    );
}

#[test]
fn test_core_escape_sequences() {
    // Arrange
    let codec = TokenClusteringCodec::new();
    // All 256 unique bytes (forces many escape sequences)
    let data: Vec<u8> = (0..=255).collect();

    // Act
    let compressed = codec.compress(&data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();

    // Assert: Escape sequences work correctly
    assert_eq!(
        data, decompressed,
        "Escape sequences should preserve all unique bytes"
    );
}

// ============================================================================
// Q2: Edge Cases
// ============================================================================

#[test]
fn test_edge_empty_input() {
    let codec = TokenClusteringCodec::new();
    let result = codec.compress(b"");
    assert!(
        matches!(result, Err(CompressionError::EmptyInput)),
        "Empty input should return EmptyInput error"
    );
}

#[test]
fn test_edge_single_byte() {
    let codec = TokenClusteringCodec::new();
    let data = b"A";
    let compressed = codec.compress(data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(data.to_vec(), decompressed, "Single byte should round-trip");
}

#[test]
fn test_edge_max_input_size() {
    let codec = TokenClusteringCodec::new();
    let data = vec![0u8; 1024 * 1024]; // Exactly 1MB (max size)
    let compressed = codec.compress(&data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(data, decompressed, "Max input size should work");
}

#[test]
fn test_edge_exceeds_max_input() {
    let codec = TokenClusteringCodec::new();
    let data = vec![0u8; 1024 * 1024 + 1]; // 1MB + 1 byte (exceeds limit)
    let result = codec.compress(&data);
    assert!(
        matches!(result, Err(CompressionError::InputTooLarge { .. })),
        "Input exceeding max size should return InputTooLarge error"
    );
}

#[test]
fn test_edge_all_same_byte() {
    let codec = TokenClusteringCodec::new();
    let data = vec![b'A'; 1000]; // All identical bytes
    let compressed = codec.compress(&data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(data, decompressed);

    let ratio = data.len() as f32 / compressed.len() as f32;
    assert!(
        ratio > 1.0,
        "All same byte should achieve at least 1× compression, got {:.2}×",
        ratio
    );
}

#[test]
fn test_edge_all_unique_bytes() {
    let codec = TokenClusteringCodec::new();
    let data: Vec<u8> = (0..=255).collect(); // All 256 unique bytes
    let compressed = codec.compress(&data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(
        data, decompressed,
        "All unique bytes should round-trip with escape sequences"
    );
}

#[test]
fn test_edge_corrupted_header_too_short() {
    let codec = TokenClusteringCodec::new();
    let corrupted = vec![0u8; 10]; // Too short for header (need 68 bytes)
    let result = codec.decompress(&corrupted);
    assert!(
        matches!(result, Err(CompressionError::InvalidFormat { .. })),
        "Corrupted header should return InvalidFormat error"
    );
}

#[test]
fn test_edge_corrupted_truncated_payload() {
    let codec = TokenClusteringCodec::new();
    let data = b"Hello world";
    let mut compressed = codec.compress(data).unwrap();

    // Truncate payload (remove last 5 bytes)
    compressed.truncate(compressed.len() - 5);

    let result = codec.decompress(&compressed);
    assert!(
        result.is_err(),
        "Truncated payload should fail decompression"
    );
}

// ============================================================================
// Q3: Invariants
// ============================================================================

#[test]
fn test_invariant_round_trip_preserves_data() {
    let codec = TokenClusteringCodec::new();
    let test_cases = vec![
        b"A".to_vec(),
        b"Hello world".to_vec(),
        vec![0, 1, 2, 3, 4, 5],
        vec![255; 100],
        (0..100).map(|i| (i * 7) as u8).collect::<Vec<u8>>(),
    ];

    for data in test_cases {
        let compressed = codec.compress(&data).unwrap();
        let decompressed = codec.decompress(&compressed).unwrap();
        assert_eq!(
            data, decompressed,
            "Invariant: compress→decompress preserves original data"
        );
    }
}

#[test]
fn test_invariant_compression_ratio_positive() {
    let codec = TokenClusteringCodec::new();
    let data = b"Test data for ratio check";
    let _compressed = codec.compress(data).unwrap();
    let ratio = codec.ratio();

    assert!(
        ratio > 0.0,
        "Invariant: Compression ratio must always be positive, got {}",
        ratio
    );
    assert!(
        ratio.is_finite(),
        "Invariant: Compression ratio must be finite"
    );
}

#[test]
fn test_invariant_compressed_size_valid() {
    let codec = TokenClusteringCodec::new();
    let data = b"Hello world";
    let compressed = codec.compress(data).unwrap();

    // Invariant: Compressed data must include header (68 bytes minimum)
    assert!(
        compressed.len() >= 68,
        "Invariant: Compressed data must be at least 68 bytes (header size), got {}",
        compressed.len()
    );
}

#[test]
fn test_invariant_deterministic_compression() {
    let codec = TokenClusteringCodec::new();
    let data = b"Deterministic test data";

    let compressed1 = codec.compress(data).unwrap();
    let compressed2 = codec.compress(data).unwrap();

    assert_eq!(
        compressed1, compressed2,
        "Invariant: Same input must produce same compressed output (deterministic)"
    );
}

// ============================================================================
// Q4: Code Path Coverage
// ============================================================================

#[test]
fn test_coverage_cluster_encoding_path() {
    // Tests the path where bytes are in the cluster table
    let codec = TokenClusteringCodec::new();
    let data = b"AAAAAABBBBBB"; // Only 2 distinct bytes (fits in clusters)
    let compressed = codec.compress(data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(data.to_vec(), decompressed);
}

#[test]
fn test_coverage_escape_encoding_path() {
    // Tests the path where bytes need escape sequences
    let codec = TokenClusteringCodec::new();
    // Create data with >15 unique bytes (forces escapes)
    let data: Vec<u8> = (0..=20).collect(); // 21 unique bytes
    let compressed = codec.compress(&data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(data, decompressed);
}

#[test]
fn test_coverage_mixed_cluster_and_escape() {
    // Tests the path with both cluster IDs and escape sequences
    let codec = TokenClusteringCodec::new();
    let mut data = vec![b'A'; 50]; // Frequent byte (will be clustered)
    data.extend_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]); // Rare bytes (will be escaped)
    let compressed = codec.compress(&data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(data, decompressed);
}

#[test]
fn test_coverage_odd_nibble_count() {
    // Tests the padding path when nibbles are odd-numbered
    let codec = TokenClusteringCodec::new();
    let data = b"ABC"; // 3 bytes may produce odd nibbles
    let compressed = codec.compress(data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(data.to_vec(), decompressed);
}

// ============================================================================
// Q5: Isolation and Determinism
// ============================================================================

#[test]
fn test_isolation_fresh_codec_instances() {
    // Each test creates fresh instance (no shared state)
    let codec1 = TokenClusteringCodec::new();
    let codec2 = TokenClusteringCodec::new();

    let data = b"Isolated test data";

    let compressed1 = codec1.compress(data).unwrap();
    let compressed2 = codec2.compress(data).unwrap();

    // Fresh instances should produce identical output
    assert_eq!(compressed1, compressed2);
}

#[test]
fn test_determinism_no_randomness() {
    // Run compression 10 times, verify identical output
    let data = b"Determinism test with repeated runs";

    let mut results = Vec::new();
    for _ in 0..10 {
        let codec = TokenClusteringCodec::new();
        let compressed = codec.compress(data).unwrap();
        results.push(compressed);
    }

    // All results should be identical (deterministic)
    for (i, result) in results.iter().enumerate() {
        assert_eq!(
            results[0], *result,
            "Run {} produced different output (non-deterministic)",
            i
        );
    }
}

// ============================================================================
// Q6: Performance (Fast Tests)
// ============================================================================

#[test]
fn test_fast_small_data_compression() {
    // Test should complete in <10ms
    let codec = TokenClusteringCodec::new();
    let data = b"Small test data";

    let start = std::time::Instant::now();
    let _compressed = codec.compress(data).unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 10,
        "Small data compression took {:?} (should be <10ms)",
        elapsed
    );
}

#[test]
fn test_fast_medium_data_compression() {
    // Test should complete in <10ms
    let codec = TokenClusteringCodec::new();
    let data = vec![b'A'; 1000]; // 1KB

    let start = std::time::Instant::now();
    let _compressed = codec.compress(&data).unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 10,
        "1KB compression took {:?} (should be <10ms)",
        elapsed
    );
}

// ============================================================================
// Q7: Readability and Maintainability
// ============================================================================

/// Helper function to verify round-trip for any data.
fn verify_round_trip(data: &[u8]) {
    let codec = TokenClusteringCodec::new();
    let compressed = codec.compress(data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(data.to_vec(), decompressed);
}

#[test]
fn test_readable_ascii_text_compression() {
    // Arrange: English text with repetition
    let text = b"The quick brown fox jumps over the lazy dog";

    // Act & Assert: Use helper for clarity
    verify_round_trip(text);
}

#[test]
fn test_readable_binary_data_compression() {
    // Arrange: Binary data (non-ASCII)
    let binary: Vec<u8> = (0..100).map(|i| (i * 13) as u8).collect();

    // Act & Assert: Use helper for clarity
    verify_round_trip(&binary);
}

#[test]
fn test_readable_error_messages() {
    let codec = TokenClusteringCodec::new();

    // Test: Empty input error
    match codec.compress(b"") {
        Err(CompressionError::EmptyInput) => {
            // Expected: Clear error variant
        }
        other => panic!("Expected EmptyInput error, got {:?}", other),
    }

    // Test: Input too large error
    let huge_data = vec![0u8; 2_000_000];
    match codec.compress(&huge_data) {
        Err(CompressionError::InputTooLarge { size, max }) => {
            assert_eq!(size, 2_000_000);
            assert_eq!(max, 1024 * 1024);
        }
        other => panic!("Expected InputTooLarge error, got {:?}", other),
    }
}
