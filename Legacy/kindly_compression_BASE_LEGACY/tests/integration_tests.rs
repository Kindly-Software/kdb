//! T28 Tier 3: Integration Testing (Q15-Q21)
//!
//! Validates components work together in realistic scenarios.

use kindly_compression::{Compress, CompressionError, TokenClusteringCodec};

// ============================================================================
// Q15: Critical Integration Points
// ============================================================================

#[test]
fn test_integration_full_pipeline_compress_decompress() {
    // Arrange: Realistic data pipeline
    let input_data = b"The computational capsule architecture provides lockfree coordination \
                       through atomic primitives and deterministic behavior through fixed-point \
                       arithmetic while achieving high performance through SIMD vectorization.";

    // Act: Full pipeline (compress → decompress)
    let codec = TokenClusteringCodec::new();
    let compressed = codec.compress(input_data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();

    // Assert: Integration preserves data
    assert_eq!(
        input_data.to_vec(),
        decompressed,
        "Full pipeline integration must preserve original data"
    );

    // Assert: Compression ratio is tracked
    let ratio = codec.ratio();
    assert!(
        ratio > 0.0,
        "Pipeline must track compression ratio, got {}",
        ratio
    );
}

#[test]
fn test_integration_multiple_codec_instances() {
    // Arrange: Multiple codec instances (simulating distributed compression)
    let data = b"Distributed compression scenario with multiple codec instances";

    // Act: Compress with codec A, decompress with codec B
    let codec_a = TokenClusteringCodec::new();
    let codec_b = TokenClusteringCodec::new();

    let compressed = codec_a.compress(data).unwrap();
    let decompressed = codec_b.decompress(&compressed).unwrap();

    // Assert: Different codec instances can decompress each other's output
    assert_eq!(
        data.to_vec(),
        decompressed,
        "Different codec instances must be compatible"
    );
}

#[test]
fn test_integration_batch_compression() {
    // Arrange: Batch of multiple items (simulating batch processing)
    let items = vec![
        b"Item 1: Short message".to_vec(),
        b"Item 2: Medium length message with more content".to_vec(),
        b"Item 3: Longer message with even more content to compress effectively".to_vec(),
    ];

    // Act: Compress all items
    let codec = TokenClusteringCodec::new();
    let mut compressed_items = Vec::new();

    for item in &items {
        let compressed = codec.compress(item).unwrap();
        compressed_items.push(compressed);
    }

    // Assert: All items can be decompressed correctly
    for (i, compressed) in compressed_items.iter().enumerate() {
        let decompressed = codec.decompress(compressed).unwrap();
        assert_eq!(
            items[i], decompressed,
            "Batch item {} failed integration",
            i
        );
    }
}

// ============================================================================
// Q16: Error Condition Propagation
// ============================================================================

#[test]
fn test_integration_error_propagation_empty_input() {
    // Arrange: Empty input (error condition)
    let codec = TokenClusteringCodec::new();

    // Act: Attempt compression
    let result = codec.compress(b"");

    // Assert: Error propagates correctly
    assert!(
        matches!(result, Err(CompressionError::EmptyInput)),
        "Empty input error must propagate through pipeline"
    );
}

#[test]
fn test_integration_error_propagation_oversized_input() {
    // Arrange: Input exceeding max size (error condition)
    let codec = TokenClusteringCodec::new();
    let huge_data = vec![0u8; 2_000_000]; // 2MB (exceeds 1MB limit)

    // Act: Attempt compression
    let result = codec.compress(&huge_data);

    // Assert: Error propagates correctly
    match result {
        Err(CompressionError::InputTooLarge { size, max }) => {
            assert_eq!(size, 2_000_000);
            assert_eq!(max, 1024 * 1024);
        }
        other => panic!("Expected InputTooLarge error, got {:?}", other),
    }
}

#[test]
fn test_integration_error_propagation_corrupted_data() {
    // Arrange: Corrupted compressed data (error condition)
    let codec = TokenClusteringCodec::new();
    let corrupted = vec![0u8; 10]; // Too short for valid header

    // Act: Attempt decompression
    let result = codec.decompress(&corrupted);

    // Assert: Error propagates correctly
    assert!(
        matches!(result, Err(CompressionError::InvalidFormat { .. })),
        "Corrupted data error must propagate through pipeline"
    );
}

#[test]
fn test_integration_error_recovery() {
    // Arrange: Pipeline with error followed by success
    let codec = TokenClusteringCodec::new();

    // Act: Error case
    let error_result = codec.compress(b"");
    assert!(error_result.is_err(), "Error case should fail");

    // Act: Success case (codec should recover)
    let success_data = b"Valid data after error";
    let success_result = codec.compress(success_data).unwrap();
    let decompressed = codec.decompress(&success_result).unwrap();

    // Assert: Codec recovers from error and processes successfully
    assert_eq!(
        success_data.to_vec(),
        decompressed,
        "Codec must recover from error and process subsequent valid input"
    );
}

// ============================================================================
// Q17: Performance Budget Integration
// ============================================================================

#[test]
fn test_integration_performance_budget_compression() {
    // Arrange: 1KB data (realistic payload size)
    let data = vec![b"Hello world, this is a test. ".repeat(35)]
        .into_iter()
        .flatten()
        .collect::<Vec<u8>>();
    let codec = TokenClusteringCodec::new();

    // Act: Measure end-to-end compression time
    let start = std::time::Instant::now();
    let compressed = codec.compress(&data).unwrap();
    let compress_time = start.elapsed();

    // Assert: Compression meets performance budget (<10ms for 1KB)
    assert!(
        compress_time.as_millis() < 10,
        "Compression time {} ms exceeds budget (10ms) for 1KB",
        compress_time.as_millis()
    );

    // Assert: Output is valid
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(data, decompressed);
}

#[test]
fn test_integration_performance_budget_decompression() {
    // Arrange: Pre-compressed data
    let data = vec![b"Test data for decompression. ".repeat(30)]
        .into_iter()
        .flatten()
        .collect::<Vec<u8>>();
    let codec = TokenClusteringCodec::new();
    let compressed = codec.compress(&data).unwrap();

    // Act: Measure end-to-end decompression time
    let start = std::time::Instant::now();
    let decompressed = codec.decompress(&compressed).unwrap();
    let decompress_time = start.elapsed();

    // Assert: Decompression meets performance budget (<5ms for 1KB)
    assert!(
        decompress_time.as_millis() < 5,
        "Decompression time {} ms exceeds budget (5ms) for 1KB",
        decompress_time.as_millis()
    );

    // Assert: Output is valid
    assert_eq!(data, decompressed);
}

#[test]
fn test_integration_performance_throughput() {
    // Arrange: Multiple items (batch throughput test)
    let items: Vec<Vec<u8>> = (0..100)
        .map(|i| format!("Item {}: Test data for throughput measurement", i).into_bytes())
        .collect();
    let codec = TokenClusteringCodec::new();

    // Act: Measure batch throughput
    let start = std::time::Instant::now();
    let mut total_bytes = 0;

    for item in &items {
        let compressed = codec.compress(item).unwrap();
        let _decompressed = codec.decompress(&compressed).unwrap();
        total_bytes += item.len();
    }

    let elapsed = start.elapsed();

    // Assert: Throughput meets target (>1MB/s)
    let throughput_mb_per_sec = (total_bytes as f64 / 1_000_000.0) / elapsed.as_secs_f64();
    println!(
        "Throughput: {:.2} MB/s ({} items, {} bytes, {:?})",
        throughput_mb_per_sec,
        items.len(),
        total_bytes,
        elapsed
    );

    assert!(
        throughput_mb_per_sec > 1.0,
        "Throughput {} MB/s is below target (1 MB/s)",
        throughput_mb_per_sec
    );
}

// ============================================================================
// Q18: Production Load Handling
// ============================================================================

#[test]
fn test_integration_handles_production_load() {
    // Arrange: Simulate production load (1000 compression operations)
    let codec = TokenClusteringCodec::new();
    let test_data = b"Production load test data with realistic content";

    // Act: Process 1000 items (simulating production throughput)
    let start = std::time::Instant::now();

    for _ in 0..1000 {
        let compressed = codec.compress(test_data).unwrap();
        let decompressed = codec.decompress(&compressed).unwrap();
        assert_eq!(test_data.to_vec(), decompressed);
    }

    let elapsed = start.elapsed();

    // Assert: Maintains throughput under load
    let ops_per_sec = 1000.0 / elapsed.as_secs_f64();
    println!(
        "Production load: {:.0} ops/sec ({:?} total)",
        ops_per_sec, elapsed
    );

    assert!(
        ops_per_sec > 100.0,
        "Production throughput {} ops/sec is below target (100 ops/sec)",
        ops_per_sec
    );
}

#[test]
fn test_integration_varying_data_sizes() {
    // Arrange: Varying data sizes (simulating real-world diversity)
    let codec = TokenClusteringCodec::new();
    let sizes = vec![
        10,    // Small
        100,   // Medium
        1000,  // Large
        10000, // Very large
    ];

    // Act & Assert: All sizes process successfully
    for size in sizes {
        let data = vec![b'A'; size];
        let compressed = codec.compress(&data).unwrap();
        let decompressed = codec.decompress(&compressed).unwrap();

        assert_eq!(
            data, decompressed,
            "Failed to handle data size {}",
            size
        );

        let ratio = data.len() as f32 / compressed.len() as f32;
        println!("Size {} bytes: Compression ratio {:.2}×", size, ratio);
    }
}

// ============================================================================
// Q19: Rollback Scenarios (Not applicable - stateless codec)
// ============================================================================

// Note: This codec is stateless (no persistent state), so rollback scenarios
// are not applicable. Each compression is independent.

// ============================================================================
// Q20: Integration Test Validation (Self-validation)
// ============================================================================

#[test]
fn test_integration_self_validation() {
    // Validate that integration tests cover critical paths

    // 1. Data preservation through full pipeline
    let codec = TokenClusteringCodec::new();
    let data = b"Self-validation test data";
    let compressed = codec.compress(data).unwrap();
    let decompressed = codec.decompress(&compressed).unwrap();
    assert_eq!(data.to_vec(), decompressed, "Full pipeline validation");

    // 2. Error handling integration
    assert!(
        codec.compress(b"").is_err(),
        "Error handling validation"
    );

    // 3. Performance budget adherence
    let start = std::time::Instant::now();
    let _compressed = codec.compress(data).unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 10,
        "Performance budget validation"
    );

    // 4. Codec compatibility
    let codec2 = TokenClusteringCodec::new();
    let decompressed2 = codec2.decompress(&compressed).unwrap();
    assert_eq!(data.to_vec(), decompressed2, "Codec compatibility validation");
}

// ============================================================================
// Q21: Integration Monitoring (Metrics collection)
// ============================================================================

#[test]
fn test_integration_metrics_collection() {
    // Arrange: Simulate metrics collection for integration monitoring
    let codec = TokenClusteringCodec::new();
    let test_data = b"Metrics collection test data";

    struct Metrics {
        compressions: usize,
        decompressions: usize,
        total_input_bytes: usize,
        total_compressed_bytes: usize,
        failures: usize,
    }

    let mut metrics = Metrics {
        compressions: 0,
        decompressions: 0,
        total_input_bytes: 0,
        total_compressed_bytes: 0,
        failures: 0,
    };

    // Act: Process multiple items with metrics tracking
    for i in 0..100 {
        let data = format!("Item {}: {}", i, std::str::from_utf8(test_data).unwrap());
        let data_bytes = data.as_bytes();

        match codec.compress(data_bytes) {
            Ok(compressed) => {
                metrics.compressions += 1;
                metrics.total_input_bytes += data_bytes.len();
                metrics.total_compressed_bytes += compressed.len();

                match codec.decompress(&compressed) {
                    Ok(_) => metrics.decompressions += 1,
                    Err(_) => metrics.failures += 1,
                }
            }
            Err(_) => metrics.failures += 1,
        }
    }

    // Assert: Metrics are collected correctly
    assert_eq!(
        metrics.compressions, 100,
        "Metrics: Expected 100 compressions"
    );
    assert_eq!(
        metrics.decompressions, 100,
        "Metrics: Expected 100 decompressions"
    );
    assert_eq!(metrics.failures, 0, "Metrics: Expected 0 failures");

    let avg_compression_ratio =
        metrics.total_input_bytes as f32 / metrics.total_compressed_bytes as f32;

    println!("Integration Metrics:");
    println!("  Compressions: {}", metrics.compressions);
    println!("  Decompressions: {}", metrics.decompressions);
    println!("  Total input: {} bytes", metrics.total_input_bytes);
    println!("  Total compressed: {} bytes", metrics.total_compressed_bytes);
    println!("  Avg compression ratio: {:.2}×", avg_compression_ratio);
    println!("  Failures: {}", metrics.failures);

    assert!(
        avg_compression_ratio > 0.0,
        "Average compression ratio must be positive"
    );
}

#[test]
fn test_integration_error_rate_tracking() {
    // Arrange: Mix of valid and invalid inputs
    let codec = TokenClusteringCodec::new();

    let mut total_operations = 0;
    let mut successful_operations = 0;
    let mut failed_operations = 0;

    // Valid inputs
    let valid_inputs = vec![
        b"Valid input 1".to_vec(),
        b"Valid input 2".to_vec(),
        vec![b'A'; 1000],
    ];

    // Invalid inputs
    let invalid_inputs = vec![
        Vec::new(),                // Empty (should fail)
        vec![0u8; 2_000_000],      // Too large (should fail)
    ];

    // Act: Process all inputs
    for input in valid_inputs {
        total_operations += 1;
        match codec.compress(&input) {
            Ok(_) => successful_operations += 1,
            Err(_) => failed_operations += 1,
        }
    }

    for input in invalid_inputs {
        total_operations += 1;
        match codec.compress(&input) {
            Ok(_) => successful_operations += 1,
            Err(_) => failed_operations += 1,
        }
    }

    // Assert: Error rate is tracked correctly
    let error_rate = failed_operations as f32 / total_operations as f32;
    println!("Error rate: {:.2}% ({}/{})", error_rate * 100.0, failed_operations, total_operations);

    assert_eq!(
        total_operations, 5,
        "Expected 5 total operations"
    );
    assert_eq!(
        successful_operations, 3,
        "Expected 3 successful operations"
    );
    assert_eq!(
        failed_operations, 2,
        "Expected 2 failed operations"
    );
}
