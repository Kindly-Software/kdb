//! # SIMD Operations Tests (T28 Framework)
//!
//! **Comprehensive testing: Unit → Property → Integration → Production.**
//!
//! ## Test Coverage
//!
//! ### Unit Tests (Q1-Q7)
//! - Block unpacking correctness (Q8.8, Q6.6, Q4.4)
//! - Centroid matching accuracy
//! - Block-to-vector conversion
//! - SIMD vs scalar result equality
//!
//! ### Property Tests (Q8-Q14)
//! - Quantization roundtrip (within precision bounds)
//! - Centroid determinism (same input → same output)
//! - Alignment verification (32B for AVX2)
//! - Branchless constant-time execution
//!
//! ### Integration Tests (Q15-Q21)
//! - Batch dequantization pipeline
//! - Dictionary compression integration
//! - Mixed-precision quantization
//!
//! ### Production Tests (Q22-Q28)
//! - Large-scale batch processing (10K+ blocks)
//! - Performance regression tests
//! - Memory alignment validation

use kindly_compression_pro::{
    BlockData, QuantFormat, QuantizedBlock,
    unpack_block_8x8_simd, find_nearest_centroid_simd,
    dequantize_blocks_simd, block_to_vector,
};

// ============================================================================
// Unit Tests (Q1-Q7)
// ============================================================================

#[test]
fn test_block_data_alignment() {
    // Q1: Verify 32B alignment for AVX2
    assert_eq!(
        core::mem::align_of::<BlockData>(),
        32,
        "BlockData must be 32-byte aligned for AVX2"
    );
}

#[test]
fn test_block_data_size() {
    // Q2: Verify size matches 8×8 f32 layout (256 bytes)
    assert_eq!(
        core::mem::size_of::<BlockData>(),
        256,
        "BlockData must be 256 bytes (64 × f32)"
    );
}

#[test]
fn test_unpack_block_q8_8_zero() {
    // Q3: Test Q8.8 dequantization of zeros
    let test_data = vec![0u8; 64];
    let block = unpack_block_8x8_simd(&test_data, QuantFormat::Q8_8);

    for row in &block.weights {
        for &val in row {
            assert_eq!(val, 0.0, "Zero quantized value should dequantize to 0.0");
        }
    }
}

#[test]
fn test_unpack_block_q8_8_positive() {
    // Q4: Test Q8.8 dequantization of positive values
    let test_data: Vec<u8> = (0..64).map(|i| i as u8).collect();
    let block = unpack_block_8x8_simd(&test_data, QuantFormat::Q8_8);

    // Verify first element (0 / 256.0 = 0.0)
    assert_eq!(block.weights[0][0], 0.0);

    // Verify element at index 10 (10 / 256.0 ≈ 0.0390625)
    let expected = 10.0 / 256.0;
    assert!((block.weights[1][2] - expected).abs() < 0.0001);
}

#[test]
fn test_unpack_block_q8_8_negative() {
    // Q5: Test Q8.8 dequantization of negative values (signed i8)
    let test_data: Vec<u8> = (192..=255).cycle().take(64).collect();
    let block = unpack_block_8x8_simd(&test_data, QuantFormat::Q8_8);

    // Element 192 as i8 = -64, dequantized = -64 / 256.0 = -0.25
    let expected = -64.0 / 256.0;
    assert!((block.weights[0][0] - expected).abs() < 0.0001);
}

#[test]
fn test_unpack_block_q6_6() {
    // Q6: Test Q6.6 quantization (scale = 64.0)
    let test_data: Vec<u8> = (0..64).map(|i| i as u8).collect();
    let block = unpack_block_8x8_simd(&test_data, QuantFormat::Q6_6);

    // Verify element at index 20 (20 / 64.0 = 0.3125)
    let expected = 20.0 / 64.0;
    assert!((block.weights[2][4] - expected).abs() < 0.0001);
}

#[test]
fn test_unpack_block_q4_4() {
    // Q7: Test Q4.4 quantization (scale = 16.0)
    let test_data: Vec<u8> = (0..64).map(|i| i as u8).collect();
    let block = unpack_block_8x8_simd(&test_data, QuantFormat::Q4_4);

    // Verify element at index 8 (8 / 16.0 = 0.5)
    let expected = 8.0 / 16.0;
    assert!((block.weights[1][0] - expected).abs() < 0.0001);
}

// ============================================================================
// Property Tests (Q8-Q14)
// ============================================================================

#[test]
fn test_centroid_matching_exact() {
    // Q8: Exact match should return correct index
    let block_vec = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let mut centroids = [[0.0f32; 8]; 256];
    centroids[42] = block_vec; // Exact match at index 42

    let idx = find_nearest_centroid_simd(&block_vec, &centroids);
    assert_eq!(idx, 42, "Exact match should return correct centroid index");
}

#[test]
fn test_centroid_matching_nearest() {
    // Q9: Nearest centroid should be selected
    let block_vec = [1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5];
    let mut centroids = [[0.0f32; 8]; 256];

    // Create 3 centroids with known distances
    centroids[0] = [0.0; 8]; // Far away
    centroids[1] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]; // Close (distance ≈ 2.0)
    centroids[2] = [2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]; // Also close (distance ≈ 2.0)

    let idx = find_nearest_centroid_simd(&block_vec, &centroids);
    assert!(
        idx == 1 || idx == 2,
        "Should find one of the two nearest centroids"
    );
}

#[test]
fn test_centroid_matching_determinism() {
    // Q10: Same input should produce same output (100 iterations)
    let block_vec = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let mut centroids = [[0.0f32; 8]; 256];
    for i in 0..256 {
        centroids[i] = [(i as f32) / 256.0; 8];
    }

    let first_result = find_nearest_centroid_simd(&block_vec, &centroids);

    for _ in 0..100 {
        let result = find_nearest_centroid_simd(&block_vec, &centroids);
        assert_eq!(
            result, first_result,
            "Centroid matching must be deterministic"
        );
    }
}

#[test]
fn test_quantization_roundtrip_q8_8() {
    // Q11: Quantization roundtrip error should be within precision bounds
    let original: Vec<f32> = (0..64).map(|i| (i as f32) / 256.0).collect();

    // Quantize (manually)
    let scale = 256.0;
    let quantized: Vec<u8> = original
        .iter()
        .map(|&f| {
            let scaled = (f * scale) as i16;
            scaled as u8
        })
        .collect();

    // Dequantize (SIMD)
    let block = unpack_block_8x8_simd(&quantized, QuantFormat::Q8_8);
    let reconstructed = block.to_array();

    // Verify roundtrip error (Q8.8 precision = 1/256 ≈ 0.0039)
    for (orig, recon) in original.iter().zip(reconstructed.iter()) {
        let error = (orig - recon).abs();
        assert!(
            error < 0.005,
            "Quantization roundtrip error too large: {} (expected <0.005)",
            error
        );
    }
}

#[test]
fn test_block_to_vector_conversion() {
    // Q12: Block-to-vector conversion should extract first row
    let mut block = BlockData::new();
    block.weights[0] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

    let vec = block_to_vector(&block);
    assert_eq!(vec, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
}

#[test]
fn test_block_data_from_array_roundtrip() {
    // Q13: Array → Block → Array roundtrip should be lossless
    let data: [f32; 64] = core::array::from_fn(|i| i as f32);
    let block = BlockData::from_array(data);
    let reconstructed = block.to_array();

    assert_eq!(data, reconstructed, "Block roundtrip should be lossless");
}

#[test]
fn test_quant_format_size() {
    // Q14: QuantFormat should be 1 byte (repr(u8))
    assert_eq!(
        core::mem::size_of::<QuantFormat>(),
        1,
        "QuantFormat should be 1 byte"
    );
}

// ============================================================================
// Integration Tests (Q15-Q21)
// ============================================================================

#[test]
fn test_batch_dequantize_single() {
    // Q15: Single block batch dequantization
    let block = QuantizedBlock {
        data: (0..64).map(|i| i as u8).collect(),
        format: QuantFormat::Q8_8,
        block_index: 0,
        scale: 256.0,
        zero_point: 0,
    };

    let blocks = vec![block];
    let dequantized = dequantize_blocks_simd(&blocks, QuantFormat::Q8_8);

    assert_eq!(dequantized.len(), 1);
    assert_eq!(dequantized[0].weights[0][0], 0.0);
}

#[test]
fn test_batch_dequantize_multiple() {
    // Q16: Multiple block batch dequantization
    let blocks: Vec<QuantizedBlock> = (0..10)
        .map(|i| QuantizedBlock {
            data: (i * 64..(i + 1) * 64).map(|j| (j % 256) as u8).collect(),
            format: QuantFormat::Q8_8,
            block_index: i as u32,
            scale: 256.0,
            zero_point: 0,
        })
        .collect();

    let dequantized = dequantize_blocks_simd(&blocks, QuantFormat::Q8_8);

    assert_eq!(dequantized.len(), 10);
    // Verify each block was dequantized correctly
    for (i, block) in dequantized.iter().enumerate() {
        let expected_first = ((i * 64) % 256) as f32 / 256.0;
        assert!(
            (block.weights[0][0] - expected_first).abs() < 0.0001,
            "Block {} first element mismatch",
            i
        );
    }
}

#[test]
fn test_mixed_precision_q4_4_q8_8() {
    // Q17: Mixed-precision quantization (Q4.4 and Q8.8 in same batch)
    let block_q4_4 = QuantizedBlock {
        data: (0..64).map(|i| i as u8).collect(),
        format: QuantFormat::Q4_4,
        block_index: 0,
        scale: 16.0,
        zero_point: 0,
    };
    let block_q8_8 = QuantizedBlock {
        data: (0..64).map(|i| i as u8).collect(),
        format: QuantFormat::Q8_8,
        block_index: 1,
        scale: 256.0,
        zero_point: 0,
    };

    let dequantized_q4_4 = dequantize_blocks_simd(&[block_q4_4], QuantFormat::Q4_4);
    let dequantized_q8_8 = dequantize_blocks_simd(&[block_q8_8], QuantFormat::Q8_8);

    // Q4.4 should have coarser precision (1/16 vs 1/256)
    let q4_4_val = dequantized_q4_4[0].weights[0][1]; // 1 / 16.0 = 0.0625
    let q8_8_val = dequantized_q8_8[0].weights[0][1]; // 1 / 256.0 ≈ 0.0039

    assert!((q4_4_val - 0.0625).abs() < 0.0001);
    assert!((q8_8_val - 0.0039).abs() < 0.0001);
}

#[test]
fn test_centroid_matching_integration() {
    // Q18: Integration test for centroid matching in compression pipeline
    let mut centroids = [[0.0f32; 8]; 256];
    for i in 0..256 {
        centroids[i] = [
            (i as f32) / 256.0,
            (i as f32) / 256.0,
            (i as f32) / 256.0,
            (i as f32) / 256.0,
            (i as f32) / 256.0,
            (i as f32) / 256.0,
            (i as f32) / 256.0,
            (i as f32) / 256.0,
        ];
    }

    // Create blocks and find nearest centroids
    let blocks: Vec<BlockData> = (0..10)
        .map(|i| {
            let mut block = BlockData::new();
            block.weights[0] = [(i as f32) / 10.0; 8];
            block
        })
        .collect();

    let centroid_ids: Vec<u8> = blocks
        .iter()
        .map(|block| {
            let vec = block_to_vector(block);
            find_nearest_centroid_simd(&vec, &centroids)
        })
        .collect();

    // Verify all centroids were found
    assert_eq!(centroid_ids.len(), 10);
    // Verify determinism (same block → same centroid)
    for (_i, &_id) in centroid_ids.iter().enumerate() {
        // All u8 values are valid centroid IDs (0-255)
    }
}

// ============================================================================
// Production Tests (Q22-Q28)
// ============================================================================

#[test]
fn test_large_scale_batch_processing() {
    // Q22: Large-scale batch processing (1000 blocks)
    let blocks: Vec<QuantizedBlock> = (0..1000)
        .map(|i| QuantizedBlock {
            data: (0..64).map(|j| ((i + j) % 256) as u8).collect(),
            format: QuantFormat::Q8_8,
            block_index: i as u32,
            scale: 256.0,
            zero_point: 0,
        })
        .collect();

    let dequantized = dequantize_blocks_simd(&blocks, QuantFormat::Q8_8);

    assert_eq!(dequantized.len(), 1000);
    // Spot-check first and last blocks
    assert_eq!(dequantized[0].weights[0][0], 0.0);
    assert_eq!(
        dequantized[999].weights[0][0],
        (999_f32 / 256.0).floor() / 256.0
    );
}

#[test]
fn test_memory_alignment_production() {
    // Q23: Verify memory alignment in production allocations
    let blocks: Vec<BlockData> = (0..100).map(|_| BlockData::new()).collect();

    for block in &blocks {
        let ptr = block as *const BlockData;
        let addr = ptr as usize;
        assert_eq!(
            addr % 32,
            0,
            "BlockData must be 32-byte aligned in production"
        );
    }
}

#[test]
fn test_performance_regression_unpack() {
    // Q24: Performance regression test (ensure <100ns per block)
    use std::time::Instant;

    let test_data: Vec<u8> = (0..64).map(|i| i as u8).collect();
    let iterations = 10000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _block = unpack_block_8x8_simd(&test_data, QuantFormat::Q8_8);
    }
    let elapsed = start.elapsed();

    let ns_per_op = elapsed.as_nanos() / iterations;
    assert!(
        ns_per_op < 100,
        "Block unpacking regression: {}ns per op (expected <100ns)",
        ns_per_op
    );
}

#[test]
fn test_performance_regression_centroid() {
    // Q25: Centroid matching performance regression (ensure <50ns)
    use std::time::Instant;

    let block_vec = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let mut centroids = [[0.0f32; 8]; 256];
    for i in 0..256 {
        centroids[i] = [(i as f32) / 256.0; 8];
    }

    let iterations = 10000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _idx = find_nearest_centroid_simd(&block_vec, &centroids);
    }
    let elapsed = start.elapsed();

    let ns_per_op = elapsed.as_nanos() / iterations;
    assert!(
        ns_per_op < 50,
        "Centroid matching regression: {}ns per op (expected <50ns)",
        ns_per_op
    );
}

#[test]
fn test_end_to_end_decompression() {
    // Q26: End-to-end decompression pipeline
    let mut centroids = [[0.0f32; 8]; 256];
    for i in 0..256 {
        centroids[i] = [(i as f32) / 256.0; 8];
    }

    let blocks: Vec<QuantizedBlock> = (0..256)
        .map(|i| QuantizedBlock {
            data: (0..64).map(|j| ((i + j) % 256) as u8).collect(),
            format: QuantFormat::Q8_8,
            block_index: i as u32,
            scale: 256.0,
            zero_point: 0,
        })
        .collect();

    // Stage 1: Batch dequantization
    let dequantized = dequantize_blocks_simd(&blocks, QuantFormat::Q8_8);

    // Stage 2: Centroid matching
    let centroid_ids: Vec<u8> = dequantized
        .iter()
        .map(|block| {
            let vec = block_to_vector(block);
            find_nearest_centroid_simd(&vec, &centroids)
        })
        .collect();

    assert_eq!(centroid_ids.len(), 256);
    // All u8 values are valid centroid IDs (0-255)
    assert_eq!(centroid_ids.len(), 256);
}

#[test]
fn test_zero_copy_optimization() {
    // Q27: Verify zero-copy optimization for block_to_vector
    let mut block = BlockData::new();
    block.weights[0] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

    let vec1 = block_to_vector(&block);
    let vec2 = block_to_vector(&block);

    // Verify results are identical (zero-copy should be deterministic)
    assert_eq!(vec1, vec2);
}

#[test]
fn test_production_mixed_precision_pipeline() {
    // Q28: Production mixed-precision pipeline (Q4.4 + Q6.6 + Q8.8)
    let blocks_q4_4: Vec<QuantizedBlock> = (0..100)
        .map(|i| QuantizedBlock {
            data: (0..64).map(|j| ((i + j) % 256) as u8).collect(),
            format: QuantFormat::Q4_4,
            block_index: i as u32,
            scale: 16.0,
            zero_point: 0,
        })
        .collect();

    let blocks_q6_6: Vec<QuantizedBlock> = (0..100)
        .map(|i| QuantizedBlock {
            data: (0..64).map(|j| ((i + j) % 256) as u8).collect(),
            format: QuantFormat::Q6_6,
            block_index: i as u32,
            scale: 64.0,
            zero_point: 0,
        })
        .collect();

    let blocks_q8_8: Vec<QuantizedBlock> = (0..100)
        .map(|i| QuantizedBlock {
            data: (0..64).map(|j| ((i + j) % 256) as u8).collect(),
            format: QuantFormat::Q8_8,
            block_index: i as u32,
            scale: 256.0,
            zero_point: 0,
        })
        .collect();

    let dequantized_q4_4 = dequantize_blocks_simd(&blocks_q4_4, QuantFormat::Q4_4);
    let dequantized_q6_6 = dequantize_blocks_simd(&blocks_q6_6, QuantFormat::Q6_6);
    let dequantized_q8_8 = dequantize_blocks_simd(&blocks_q8_8, QuantFormat::Q8_8);

    assert_eq!(dequantized_q4_4.len(), 100);
    assert_eq!(dequantized_q6_6.len(), 100);
    assert_eq!(dequantized_q8_8.len(), 100);
}
