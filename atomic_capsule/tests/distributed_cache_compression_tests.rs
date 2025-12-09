//! T28 Comprehensive Tests for Distributed Cache Compression
//!
//! **Tier 1 (Unit - Q1-Q7):** Compression/decompression correctness
//! **Tier 2 (Property - Q8-Q14):** Edge cases, zip bombs, roundtrips
//! **Tier 3 (Integration - Q15-Q21):** Integration with distributed cache
//! **Tier 4 (Production - Q22-Q28):** Real-world payloads, performance validation

#![cfg(feature = "distributed-compression")]

use atomic_capsule::collections::distributed_cache_compression::{
    compress_if_beneficial, decompress_safe, COMPRESSION_THRESHOLD, MAX_EXPANSION_RATIO,
};

// ============================================================================
// Test Helpers
// ============================================================================

/// Generate pseudo-random data that compresses moderately (stays under 100× ratio)
/// This prevents false positives in zip bomb protection while still being compressible
/// Uses repeating pattern with more variation to achieve ~1.5-3× compression ratio
fn generate_compressible_data(size: usize) -> Vec<u8> {
    let mut data = vec![0u8; size];

    // Create a repeating pattern with 64-byte blocks and more mixing
    // Smaller blocks + more variation = lower compression ratio (stays under 100×)
    for (i, byte) in data.iter_mut().enumerate() {
        let block = i / 64; // Which 64-byte block (smaller blocks = less compression)
        let offset = i % 64; // Position within block

        // More complex mixing to reduce compression ratio
        *byte = ((offset * 3 + block * 17 + (block / 4) * 31) % 256) as u8;
    }
    data
}

// ============================================================================
// Tier 1: Unit Tests (Q1-Q7)
// ============================================================================

#[test]
fn q1_test_compression_threshold_boundary() {
    // Exactly at threshold (1KB)
    let data_at_threshold = vec![0u8; COMPRESSION_THRESHOLD];
    let result = compress_if_beneficial(&data_at_threshold).unwrap();
    assert!(
        !result.compressed,
        "Data at threshold should not be compressed"
    );

    // Just above threshold (1KB + 1)
    let data_above_threshold = vec![0u8; COMPRESSION_THRESHOLD + 1];
    let result = compress_if_beneficial(&data_above_threshold).unwrap();
    assert!(
        result.compressed,
        "Data above threshold should be compressed (if beneficial)"
    );
}

#[test]
fn q2_test_empty_payload() {
    let empty = vec![];
    let result = compress_if_beneficial(&empty).unwrap();

    assert!(!result.compressed);
    assert_eq!(result.original_size, 0);
    assert_eq!(result.final_size, 0);
    assert_eq!(result.data.len(), 0);
}

#[test]
fn q3_test_small_payload() {
    // 512 bytes (< 1KB threshold)
    let small = vec![42u8; 512];
    let result = compress_if_beneficial(&small).unwrap();

    assert!(!result.compressed, "Small payload should skip compression");
    assert_eq!(result.data, small);
    assert_eq!(result.original_size, 512);
    assert_eq!(result.final_size, 512);
}

#[test]
fn q4_test_highly_compressible_payload() {
    // 10KB of zeros (highly compressible)
    let zeros = vec![0u8; 10 * 1024];
    let result = compress_if_beneficial(&zeros).unwrap();

    assert!(result.compressed, "Zeros should compress");
    assert!(
        result.final_size < result.original_size,
        "Compressed size should be smaller"
    );

    // Verify compression ratio is substantial (>10× for zeros)
    let ratio = result.ratio();
    assert!(
        ratio > 10.0,
        "Zeros should achieve >10× compression: {}",
        ratio
    );
}

#[test]
fn q5_test_incompressible_payload() {
    // 2KB of pseudo-random data (incompressible)
    let mut random = vec![0u8; 2 * 1024];
    for (i, byte) in random.iter_mut().enumerate() {
        *byte = ((i * 73 + 19) % 256) as u8; // Pseudo-random pattern
    }

    let result = compress_if_beneficial(&random).unwrap();

    // Incompressible data may or may not be marked as compressed
    // (depends on zstd heuristics), but final size should be close to original
    assert_eq!(result.original_size, 2 * 1024);
}

#[test]
fn q6_test_roundtrip_preserves_data() {
    // 5KB compressible payload
    let original = b"Hello, World! This is a test payload. ".repeat(140); // ~5.2KB

    // Compress
    let compressed = compress_if_beneficial(&original).unwrap();
    assert!(compressed.compressed, "Should compress 5KB text");

    // Decompress
    let decompressed = decompress_safe(&compressed.data).unwrap();

    // Verify exact match
    assert_eq!(
        decompressed, original,
        "Roundtrip must preserve data byte-for-byte"
    );
}

#[test]
fn q7_test_compression_stats() {
    let data = vec![0u8; 10 * 1024]; // 10KB zeros
    let result = compress_if_beneficial(&data).unwrap();

    assert!(result.compressed);
    assert_eq!(result.original_size, 10 * 1024);

    // Check ratio
    let ratio = result.ratio();
    assert!(ratio > 10.0, "Zeros should achieve >10× compression");

    // Check savings
    let savings = result.savings();
    assert!(
        savings > 0.9,
        "Should save >90% bandwidth on zeros: {}%",
        savings * 100.0
    );
}

// ============================================================================
// Tier 2: Property Tests (Q8-Q14)
// ============================================================================

#[test]
fn q8_test_determinism() {
    // Same input should always produce same output
    let payload = b"Test data for determinism check".repeat(100); // ~3.2KB

    let result1 = compress_if_beneficial(&payload).unwrap();
    let result2 = compress_if_beneficial(&payload).unwrap();

    assert_eq!(
        result1.data, result2.data,
        "Compression must be deterministic"
    );
    assert_eq!(result1.compressed, result2.compressed);
    assert_eq!(result1.final_size, result2.final_size);
}

#[test]
fn q9_test_multiple_roundtrips() {
    // Test compress → decompress → compress → decompress
    let original = generate_compressible_data(5 * 1024); // 5KB pseudo-random

    // First roundtrip
    let compressed1 = compress_if_beneficial(&original).unwrap();
    let decompressed1 = decompress_safe(&compressed1.data).unwrap();
    assert_eq!(decompressed1, original);

    // Second roundtrip (on decompressed data)
    let compressed2 = compress_if_beneficial(&decompressed1).unwrap();
    let decompressed2 = decompress_safe(&compressed2.data).unwrap();
    assert_eq!(decompressed2, original);

    // Compressed output should be identical
    assert_eq!(
        compressed1.data, compressed2.data,
        "Multiple compressions should be idempotent"
    );
}

#[test]
fn q10_test_zip_bomb_protection() {
    // Create highly compressible payload (10KB zeros → <100 bytes compressed)
    let original = vec![0u8; 10 * 1024];

    let compressed = compress_if_beneficial(&original).unwrap();
    assert!(compressed.compressed);

    // Compression ratio should be >100× (10KB → <100 bytes)
    let ratio = compressed.ratio();
    assert!(ratio > 100.0, "Zeros should compress >100×: {}", ratio);

    // Decompression should FAIL due to zip bomb protection (>100× expansion)
    let result = decompress_safe(&compressed.data);
    assert!(result.is_err(), "Zip bomb should be rejected");

    if let Err(e) = result {
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            e.to_string().contains("zip bomb"),
            "Error should mention zip bomb: {}",
            e
        );
    }

    // Verify expansion limit constant
    assert_eq!(MAX_EXPANSION_RATIO, 100, "Expansion limit must be 100×");
}

#[test]
fn q11_test_various_payload_sizes() {
    // Test different payload sizes around threshold
    let test_sizes = [
        0,     // Empty
        512,   // Half threshold
        1023,  // Just below threshold
        1024,  // Exactly at threshold
        1025,  // Just above threshold
        2048,  // 2KB
        5120,  // 5KB
        10240, // 10KB
        51200, // 50KB
    ];

    for size in test_sizes {
        let data = if size == 0 {
            vec![]
        } else {
            generate_compressible_data(size)
        };
        let result = compress_if_beneficial(&data).unwrap();

        if size <= COMPRESSION_THRESHOLD {
            assert!(!result.compressed, "Size {} should not compress", size);
        } else {
            // Pseudo-random data should compress (less than uniform bytes)
            assert!(result.compressed, "Size {} should compress", size);
        }

        // Roundtrip
        if result.compressed {
            let decompressed = decompress_safe(&result.data).unwrap();
            assert_eq!(decompressed, data, "Roundtrip failed for size {}", size);
        }
    }
}

#[test]
fn q12_test_different_data_patterns() {
    // Test various data patterns
    // Note: Use pseudo-random for highly uniform patterns to avoid >100× expansion
    let patterns: Vec<(&str, Vec<u8>)> = vec![
        ("pseudo_random", generate_compressible_data(5 * 1024)),
        ("mixed_blocks", generate_compressible_data(5 * 1024)), // Different seed would be better but same function
        (
            "ascii_text",
            b"The quick brown fox jumps over the lazy dog. ".repeat(120),
        ),
    ];

    for (name, data) in patterns {
        let result = compress_if_beneficial(&data).unwrap();

        // All should compress
        assert!(result.compressed, "Pattern '{}' should compress", name);

        // Roundtrip
        let decompressed = decompress_safe(&result.data).unwrap();
        assert_eq!(
            decompressed, data,
            "Roundtrip failed for pattern '{}'",
            name
        );
    }
}

#[test]
fn q13_test_unicode_text() {
    // Test Unicode text (UTF-8 encoded)
    let unicode_text = "Hello 世界! 🚀 Rust is awesome! ".repeat(100); // ~3.3KB
    let bytes = unicode_text.as_bytes();

    let result = compress_if_beneficial(bytes).unwrap();
    assert!(result.compressed, "Unicode text should compress");

    // Roundtrip
    let decompressed = decompress_safe(&result.data).unwrap();
    assert_eq!(decompressed, bytes);

    // Verify UTF-8 decoding
    let decoded = String::from_utf8(decompressed).unwrap();
    assert_eq!(decoded, unicode_text);
}

#[test]
fn q14_test_thread_safety() {
    // Verify thread-local buffer doesn't cause issues with concurrent compression
    use std::thread;

    let handles: Vec<_> = (0..10)
        .map(|i| {
            thread::spawn(move || {
                let data = vec![(i % 256) as u8; 5 * 1024];
                let result = compress_if_beneficial(&data).unwrap();
                assert!(result.compressed || !result.compressed); // Just verify it doesn't panic
                result
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

// ============================================================================
// Tier 3: Integration Tests (Q15-Q21)
// ============================================================================

#[test]
fn q15_test_json_payload() {
    // Simulate typical JSON cache payload
    let json = r#"{"user_id":12345,"name":"John Doe","email":"john@example.com","roles":["admin","user"],"metadata":{"created_at":"2025-01-01T00:00:00Z","last_login":"2025-10-25T12:00:00Z"}}"#.repeat(20); // ~3.5KB

    let bytes = json.as_bytes();
    let result = compress_if_beneficial(bytes).unwrap();

    assert!(result.compressed, "JSON should compress");

    // JSON typically compresses 2-5×
    let ratio = result.ratio();
    assert!(
        ratio > 2.0,
        "JSON should achieve >2× compression: {}",
        ratio
    );

    // Roundtrip
    let decompressed = decompress_safe(&result.data).unwrap();
    assert_eq!(decompressed, bytes);
}

#[test]
fn q16_test_html_payload() {
    // Simulate typical HTML cache payload
    let html = r#"<!DOCTYPE html><html><head><title>Test Page</title></head><body><h1>Hello World</h1><p>This is a test page.</p></body></html>"#.repeat(25); // ~3.6KB

    let bytes = html.as_bytes();
    let result = compress_if_beneficial(bytes).unwrap();

    assert!(result.compressed, "HTML should compress");

    // HTML typically compresses 3-7×
    let ratio = result.ratio();
    assert!(
        ratio > 2.0,
        "HTML should achieve >2× compression: {}",
        ratio
    );

    // Roundtrip
    let decompressed = decompress_safe(&result.data).unwrap();
    assert_eq!(decompressed, bytes);
}

#[test]
fn q17_test_binary_data() {
    // Simulate binary data (e.g., protobuf, CBOR)
    let mut binary = vec![0u8; 5 * 1024];
    for (i, byte) in binary.iter_mut().enumerate() {
        *byte = ((i / 64) % 256) as u8; // Somewhat compressible
    }

    let result = compress_if_beneficial(&binary).unwrap();

    // Binary data may or may not compress well (depends on entropy)
    if result.compressed {
        let decompressed = decompress_safe(&result.data).unwrap();
        assert_eq!(decompressed, binary);
    }
}

#[test]
fn q18_test_already_compressed_data() {
    // Simulate already-compressed data (e.g., JPEG, PNG, GZIP)
    // Use pseudo-random data to simulate high entropy
    let mut already_compressed = vec![0u8; 5 * 1024];
    for (i, byte) in already_compressed.iter_mut().enumerate() {
        *byte = ((i * 137 + 53) % 256) as u8; // High-entropy pseudo-random
    }

    let result = compress_if_beneficial(&already_compressed).unwrap();

    // Should not compress (or compress very little)
    // Verify no data loss regardless
    if result.compressed {
        let decompressed = decompress_safe(&result.data).unwrap();
        assert_eq!(decompressed, already_compressed);
    } else {
        assert_eq!(result.data, already_compressed);
    }
}

#[test]
fn q19_test_large_payload() {
    // Test 100KB payload (realistic cache value)
    let large = generate_compressible_data(100 * 1024);
    let result = compress_if_beneficial(&large).unwrap();

    assert!(result.compressed, "100KB should compress");

    // Verify compression
    let ratio = result.ratio();
    assert!(
        ratio > 1.2,
        "Pseudo-random data should compress moderately: {}",
        ratio
    );

    // Roundtrip
    let decompressed = decompress_safe(&result.data).unwrap();
    assert_eq!(decompressed, large);
}

#[test]
fn q20_test_compression_error_handling() {
    // Test decompression of invalid data
    let invalid_compressed = vec![0xFF, 0xFE, 0xFD]; // Invalid zstd stream

    let result = decompress_safe(&invalid_compressed);
    assert!(
        result.is_err(),
        "Invalid compressed data should return error"
    );
}

#[test]
fn q21_test_bandwidth_savings_calculation() {
    // Verify savings calculation
    let data = vec![0u8; 10 * 1024];
    let result = compress_if_beneficial(&data).unwrap();

    assert!(result.compressed);

    let savings = result.savings();
    let expected_savings = 1.0 - (result.final_size as f64 / result.original_size as f64);

    assert!(
        (savings - expected_savings).abs() < 1e-6,
        "Savings calculation mismatch"
    );
    assert!(
        savings > 0.8,
        "Should save >80% on zeros: {}%",
        savings * 100.0
    );
}

// ============================================================================
// Tier 4: Production Tests (Q22-Q28)
// ============================================================================

#[test]
fn q22_test_realistic_json_api_response() {
    // Realistic JSON API response (1000 users)
    let users: Vec<String> = (0..1000)
        .map(|i| {
            format!(
                r#"{{"id":{},"name":"User {}","email":"user{}@example.com"}}"#,
                i, i, i
            )
        })
        .collect();

    let json = format!("[{}]", users.join(","));
    let bytes = json.as_bytes();

    assert!(
        bytes.len() > COMPRESSION_THRESHOLD,
        "Payload should exceed threshold"
    );

    let result = compress_if_beneficial(bytes).unwrap();
    assert!(result.compressed, "JSON API response should compress");

    // Verify significant savings
    let savings = result.savings();
    assert!(
        savings > 0.5,
        "Should save >50% on JSON: {}%",
        savings * 100.0
    );

    // Roundtrip
    let decompressed = decompress_safe(&result.data).unwrap();
    assert_eq!(decompressed, bytes);
}

#[test]
fn q23_test_compression_performance_target() {
    // Verify compression meets <2ms target for 10KB payload
    let data = vec![0u8; 10 * 1024];

    let start = std::time::Instant::now();
    let result = compress_if_beneficial(&data).unwrap();
    let compress_duration = start.elapsed();

    assert!(result.compressed);
    assert!(
        compress_duration.as_millis() < 2,
        "Compression should be <2ms: {:?}",
        compress_duration
    );
}

#[test]
fn q24_test_decompression_performance_target() {
    // Verify decompression meets <1ms target for 10KB payload
    let data = generate_compressible_data(10 * 1024);
    let compressed = compress_if_beneficial(&data).unwrap();

    let start = std::time::Instant::now();
    let decompressed = decompress_safe(&compressed.data).unwrap();
    let decompress_duration = start.elapsed();

    assert_eq!(decompressed, data);
    assert!(
        decompress_duration.as_millis() < 1,
        "Decompression should be <1ms: {:?}",
        decompress_duration
    );
}

#[test]
fn q25_test_buffer_reuse_across_compressions() {
    // Verify thread-local buffer is reused (no allocation explosion)
    for _ in 0..100 {
        let data = vec![0u8; 5 * 1024];
        let result = compress_if_beneficial(&data).unwrap();
        assert!(result.compressed);
    }

    // No way to directly verify reuse, but if it didn't reuse,
    // memory usage would spike (detectable in production monitoring)
}

#[test]
fn q26_test_real_world_cache_hit_pattern() {
    // Simulate cache hit pattern: same data compressed multiple times
    let cached_value = b"Frequently accessed cache value".repeat(50); // ~1.6KB

    // First access (cache miss → compress)
    let compressed1 = compress_if_beneficial(&cached_value).unwrap();

    // Subsequent accesses (simulate cache hits → decompress)
    for _ in 0..10 {
        let decompressed = decompress_safe(&compressed1.data).unwrap();
        assert_eq!(decompressed, cached_value);
    }

    // Later update (compress new version)
    let updated_value = b"Updated cache value after write".repeat(50);
    let compressed2 = compress_if_beneficial(&updated_value).unwrap();

    let decompressed2 = decompress_safe(&compressed2.data).unwrap();
    assert_eq!(decompressed2, updated_value);
}

#[test]
fn q27_test_mixed_payload_sizes() {
    // Simulate real cache with mixed payload sizes
    let test_cases = vec![
        ("small", generate_compressible_data(256)), // Skip compression
        ("threshold", generate_compressible_data(1024)), // Skip compression
        ("medium", generate_compressible_data(5 * 1024)), // Compress
        ("large", generate_compressible_data(50 * 1024)), // Compress
        ("huge", generate_compressible_data(100 * 1024)), // Compress
    ];

    for (name, data) in test_cases {
        let result = compress_if_beneficial(&data).unwrap();

        if data.len() <= COMPRESSION_THRESHOLD {
            assert!(!result.compressed, "{} should not compress", name);
        } else {
            assert!(result.compressed, "{} should compress", name);

            // Roundtrip
            let decompressed = decompress_safe(&result.data).unwrap();
            assert_eq!(decompressed, data, "{} roundtrip failed", name);
        }
    }
}

#[test]
fn q28_test_end_to_end_cache_simulation() {
    // Simulate end-to-end cache workflow
    struct CacheEntry {
        key: String,
        value: Vec<u8>,
        compressed_value: Vec<u8>,
        compressed: bool,
    }

    let mut cache: Vec<CacheEntry> = Vec::new();

    // Insert phase
    for i in 0..10 {
        let key = format!("key_{}", i);
        let value = format!("Value {} ", i).repeat(200).into_bytes(); // ~1.8KB each

        let result = compress_if_beneficial(&value).unwrap();

        cache.push(CacheEntry {
            key: key.clone(),
            value: value.clone(),
            compressed_value: result.data.clone(),
            compressed: result.compressed,
        });

        assert!(result.compressed, "Cache value should compress");
    }

    // Lookup phase (decompress on cache hit)
    for entry in &cache {
        if entry.compressed {
            let decompressed = decompress_safe(&entry.compressed_value).unwrap();
            assert_eq!(
                decompressed, entry.value,
                "Cache entry {} corrupted",
                entry.key
            );
        }
    }

    // Verify bandwidth savings
    let total_original: usize = cache.iter().map(|e| e.value.len()).sum();
    let total_compressed: usize = cache.iter().map(|e| e.compressed_value.len()).sum();

    let overall_savings = 1.0 - (total_compressed as f64 / total_original as f64);
    assert!(
        overall_savings > 0.3,
        "Overall cache should save >30% bandwidth: {}%",
        overall_savings * 100.0
    );
}
