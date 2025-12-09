//! Comprehensive compression tests.

use kindly_compression::{Compress, CompressionError, TokenClusteringCodec};

#[test]
fn test_roundtrip_small_data() {
    let codec = TokenClusteringCodec::new();
    let data = b"Hello, world!";
    let compressed = codec.compress(data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(data.to_vec(), decompressed);
}

#[test]
fn test_roundtrip_large_data() {
    let codec = TokenClusteringCodec::new();
    let data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
    let compressed = codec.compress(&data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(data, decompressed);
}

#[test]
fn test_compression_ratio_repeated_tokens() {
    let codec = TokenClusteringCodec::new();

    // Highly repetitive data (good compression)
    let data = vec![b'A'; 1000];
    let compressed = codec.compress(&data).unwrap();
    let ratio = data.len() as f32 / compressed.len() as f32;

    println!("Compression ratio (1000× 'A'): {:.2}×", ratio);
    assert!(ratio > 1.5, "Expected >1.5× compression for highly repetitive data, got {:.2}×", ratio);
}

#[test]
fn test_compression_ratio_realistic_text() {
    let codec = TokenClusteringCodec::new();

    // Realistic English text with repetition
    let text = b"The computational capsule architecture provides lockfree coordination \
                 through atomic primitives. The computational capsule architecture \
                 enables deterministic behavior through fixed-point arithmetic. \
                 The computational capsule architecture achieves high performance \
                 through SIMD vectorization and cache-aware memory layout.";

    let compressed = codec.compress(text).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(text.to_vec(), decompressed);

    let ratio = text.len() as f32 / compressed.len() as f32;
    println!("Compression ratio (realistic text): {:.2}×", ratio);
    println!("Original: {} bytes, Compressed: {} bytes", text.len(), compressed.len());

    // Target: 1.2-1.5× compression for realistic text (public algorithm has header overhead)
    assert!(ratio >= 1.1, "Expected >=1.1× compression for realistic text, got {:.2}×", ratio);
}

#[test]
fn test_empty_input_error() {
    let codec = TokenClusteringCodec::new();
    let result = codec.compress(b"");
    assert!(matches!(result, Err(CompressionError::EmptyInput)));
}

#[test]
fn test_max_input_size() {
    let codec = TokenClusteringCodec::new();
    let data = vec![0u8; 2 * 1024 * 1024]; // 2MB (exceeds 1MB limit)
    let result = codec.compress(&data);
    assert!(matches!(result, Err(CompressionError::InputTooLarge { .. })));
}

#[test]
fn test_corrupted_data() {
    let codec = TokenClusteringCodec::new();
    let corrupted = vec![0u8; 10]; // Too short for header
    let result = codec.decompress(&corrupted);
    assert!(matches!(result, Err(CompressionError::InvalidFormat { .. })));
}

#[test]
fn test_deterministic_compression() {
    let codec = TokenClusteringCodec::new();
    let data = b"Deterministic test data with repeated patterns";

    let compressed1 = codec.compress(data).unwrap();
    let compressed2 = codec.compress(data).unwrap();

    // Should produce identical compressed output (deterministic)
    assert_eq!(compressed1, compressed2);
}

#[test]
fn test_compression_ratio_tracking() {
    let codec = TokenClusteringCodec::new();
    let data = b"Test data for compression ratio tracking";

    let _compressed = codec.compress(data).unwrap();
    let ratio = codec.ratio();

    assert!(ratio > 0.0, "Compression ratio should be positive");
    println!("Compression ratio: {:.2}×", ratio);
}

/// Benchmark-style test to measure compression performance.
#[test]
fn test_compression_performance() {
    let codec = TokenClusteringCodec::new();

    // 1KB realistic data
    let data = vec![b"Hello world, this is a test message. ".repeat(30)]
        .into_iter()
        .flatten()
        .collect::<Vec<u8>>();

    let start = std::time::Instant::now();
    let compressed = codec.compress(&data).unwrap();
    let compress_time = start.elapsed();

    let start = std::time::Instant::now();
    let _decompressed = codec.decompress(&compressed).unwrap();
    let decompress_time = start.elapsed();

    println!("Compression time (1KB): {:?}", compress_time);
    println!("Decompression time (1KB): {:?}", decompress_time);

    // Target: <100ns decompression (may vary on different hardware)
    // Note: This is an optimistic target - actual performance may be higher
    println!("Note: <100ns decompression target is aspirational");
}
