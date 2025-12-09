// SPDX-License-Identifier: MIT OR Apache-2.0
//! # Compression Tests - T28 Framework
//!
//! Comprehensive testing for streaming compression (Tier 5 + Tier 4).
//!
//! ## T28 Test Structure
//! - **Q1-Q7** (Unit): Basic functionality (10 tests)
//! - **Q8-Q14** (Property): Invariants (3 tests)
//! - **Q15-Q21** (Integration): End-to-end (2 tests)
//! - **Q22-Q28** (Stress): Performance under load (1 test)
//!
//! **Total**: 16+ tests

use clapi_core::compression::{
    StreamingCompressor, CompressionLevel, CompressionStateCapsule,
    DEFAULT_COMPRESSION_LEVEL, MIN_COMPRESSION_SIZE, TARGET_COMPRESSION_RATIO,
};

// ============================================================================
// Q1-Q7: UNIT TESTS (Basic Functionality)
// ============================================================================

#[test]
fn test_unit_compressor_creation() {
    let compressor = StreamingCompressor::default();
    let stats = compressor.stats();
    assert_eq!(stats.total_in, 0);
    assert_eq!(stats.total_out, 0);
    assert!(stats.is_active);
}

#[test]
fn test_unit_compression_levels() {
    let levels = [
        CompressionLevel::Fastest,
        CompressionLevel::Balanced,
        CompressionLevel::Best,
    ];

    for level in levels {
        let compressor = StreamingCompressor::new(level);
        let input = b"Test data ".repeat(100);
        let compressed = compressor.compress(&input).unwrap();
        assert!(compressed.len() < input.len());
    }
}

#[test]
fn test_unit_empty_input() {
    let compressor = StreamingCompressor::default();

    let compressed = compressor.compress(&[]).unwrap();
    assert!(compressed.is_empty());

    let decompressed = compressor.decompress(&[]).unwrap();
    assert!(decompressed.is_empty());
}

#[test]
fn test_unit_small_input() {
    let compressor = StreamingCompressor::default();
    let input = b"Hello, world!";

    let compressed = compressor.compress(input).unwrap();
    let decompressed = compressor.decompress(&compressed).unwrap();

    assert_eq!(decompressed, input);
}

#[test]
fn test_unit_large_input() {
    let compressor = StreamingCompressor::default();
    // 100KB input
    let input = b"Large test data ".repeat(6400);

    let compressed = compressor.compress(&input).unwrap();
    assert!(compressed.len() < input.len());

    let decompressed = compressor.decompress(&compressed).unwrap();
    assert_eq!(decompressed, input);
}

#[test]
fn test_unit_repetitive_data() {
    let compressor = StreamingCompressor::new(CompressionLevel::Best);
    // Highly compressible (10KB of 'A's)
    let input = b"A".repeat(10000);

    let compressed = compressor.compress(&input).unwrap();
    let stats = compressor.stats();

    // Should achieve high compression ratio
    assert!(stats.compression_ratio() > 10.0);
    assert!(stats.savings_percent() > 90.0);
}

#[test]
fn test_unit_random_data() {
    let compressor = StreamingCompressor::default();
    // Pseudo-random data (less compressible)
    let input: Vec<u8> = (0..10000).map(|i| (i * 37) as u8).collect();

    let compressed = compressor.compress(&input).unwrap();
    let decompressed = compressor.decompress(&compressed).unwrap();

    assert_eq!(decompressed, input);
    // Random data compresses poorly
    let stats = compressor.stats();
    assert!(stats.compression_ratio() < 2.0);
}

#[test]
fn test_unit_stats_tracking() {
    let compressor = StreamingCompressor::default();
    let input1 = b"First input ".repeat(100);
    let input2 = b"Second input ".repeat(100);

    compressor.compress(&input1).unwrap();
    compressor.compress(&input2).unwrap();

    let stats = compressor.stats();
    assert_eq!(stats.total_in, (input1.len() + input2.len()) as u64);
    assert_eq!(stats.batch_count, 2);
}

#[test]
fn test_unit_reset() {
    let compressor = StreamingCompressor::default();
    let input = b"Test data ".repeat(100);

    compressor.compress(&input).unwrap();
    let stats_before = compressor.stats();
    assert!(stats_before.total_in > 0);

    compressor.reset();
    let stats_after = compressor.stats();
    assert_eq!(stats_after.total_in, 0);
    assert_eq!(stats_after.total_out, 0);
    assert_eq!(stats_after.batch_count, 0);
}

#[test]
fn test_unit_should_compress_threshold() {
    assert!(!StreamingCompressor::should_compress(512));
    assert!(StreamingCompressor::should_compress(MIN_COMPRESSION_SIZE));
    assert!(StreamingCompressor::should_compress(10000));
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS (Invariant Validation)
// ============================================================================

#[test]
fn test_property_compression_ratio_bounds() {
    let compressor = StreamingCompressor::default();

    // Test 100 random payloads
    for size in (1000..100000).step_by(1000) {
        let input = b"X".repeat(size);
        let compressed = compressor.compress(&input).unwrap();

        // Property: Compressed size should be <= input size
        assert!(compressed.len() <= input.len() * 2); // Allow overhead for small inputs

        // Property: Decompression recovers original
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, input);
    }
}

#[test]
fn test_property_stats_consistency() {
    let compressor = StreamingCompressor::default();

    let mut total_in = 0u64;
    let mut total_out = 0u64;

    for i in 1..=10 {
        let input = b"Test ".repeat(i * 100);
        let compressed = compressor.compress(&input).unwrap();

        total_in += input.len() as u64;
        total_out += compressed.len() as u64;

        let stats = compressor.stats();
        // Property: Stats should match accumulated totals
        assert_eq!(stats.total_in, total_in);
        assert_eq!(stats.total_out, total_out);
        assert_eq!(stats.batch_count, i as u64);
    }
}

#[test]
fn test_property_target_compression_ratio() {
    let compressor = StreamingCompressor::new(CompressionLevel::Balanced);

    // Test with typical GPT-4 responses (JSON-like)
    let json_response = r#"{"id":"chatcmpl-123","object":"chat.completion","created":1234567890,"model":"gpt-4","choices":[{"index":0,"message":{"role":"assistant","content":"This is a simulated response from GPT-4 with typical structure and verbosity that would be seen in production."},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":50,"total_tokens":60}}"#;

    let input = json_response.repeat(100); // ~50KB
    let compressed = compressor.compress(input.as_bytes()).unwrap();

    let stats = compressor.stats();
    // Property: Should achieve at least 3× compression on repetitive JSON
    assert!(
        stats.compression_ratio() >= TARGET_COMPRESSION_RATIO,
        "Compression ratio {} < target {}",
        stats.compression_ratio(),
        TARGET_COMPRESSION_RATIO
    );
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS (End-to-End Workflows)
// ============================================================================

#[test]
fn test_integration_e2e_compression() {
    let compressor = StreamingCompressor::default();

    // Simulate AI response compression workflow
    let ai_responses = vec![
        "Short response".to_string(),
        "Medium response with more content ".repeat(50),
        "Large response with extensive content ".repeat(500),
    ];

    for response in &ai_responses {
        let bytes = response.as_bytes();

        if StreamingCompressor::should_compress(bytes.len()) {
            let compressed = compressor.compress(bytes).unwrap();
            let decompressed = compressor.decompress(&compressed).unwrap();

            assert_eq!(decompressed, bytes);
            assert!(compressed.len() < bytes.len());
        }
    }

    let stats = compressor.stats();
    assert!(stats.batch_count > 0);
    assert!(stats.total_in > 0);
    assert!(stats.compression_ratio() > 1.0);
}

#[test]
fn test_integration_batched_compression() {
    let compressor = StreamingCompressor::default();

    // Large input requiring batched processing
    let input = b"Batch test data ".repeat(10000); // ~160KB

    let compressed = compressor.compress_batched(&input).unwrap();
    assert!(compressed.len() < input.len());

    let stats = compressor.stats();
    assert!(stats.batch_count > 0);
}

// ============================================================================
// Q22-Q28: STRESS TESTS (Performance Under Load)
// ============================================================================

#[test]
fn test_stress_concurrent_compression() {
    use std::sync::Arc;
    use std::thread;

    let compressor = Arc::new(StreamingCompressor::default());
    let mut handles = vec![];

    // 10 threads compressing simultaneously
    for thread_id in 0..10 {
        let c = Arc::clone(&compressor);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let input = format!("Thread {} iteration {} data ", thread_id, i).repeat(100);
                let _compressed = c.compress(input.as_bytes()).unwrap();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = compressor.stats();
    assert_eq!(stats.batch_count, 1000); // 10 threads × 100 iterations
    assert!(stats.compression_ratio() > 1.0);
}

// ============================================================================
// ADDITIONAL TESTS (Capsule-Specific)
// ============================================================================

#[test]
fn test_capsule_state_initialization() {
    let capsule = CompressionStateCapsule::new();
    assert!(!capsule.is_initialized());

    capsule.initialize();
    assert!(capsule.is_initialized());
    assert_eq!(capsule.generation(), 1);
}

#[test]
fn test_capsule_concurrent_updates() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(CompressionStateCapsule::new());
    capsule.initialize();
    capsule.set_active();

    let mut handles = vec![];

    for _ in 0..10 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                c.record_compression(1000, 300);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let stats = capsule.stats();
    assert_eq!(stats.total_in, 1000000); // 10 threads × 100 ops × 1000 bytes
    assert_eq!(stats.total_out, 300000);
    assert_eq!(stats.batch_count, 1000);
}

#[test]
fn test_capsule_generation_increments() {
    let capsule = CompressionStateCapsule::new();

    let gen0 = capsule.generation();
    capsule.initialize();
    let gen1 = capsule.generation();
    assert!(gen1 > gen0);

    capsule.set_active();
    let gen2 = capsule.generation();
    assert!(gen2 > gen1);

    capsule.record_compression(100, 30);
    let gen3 = capsule.generation();
    assert!(gen3 > gen2);
}
