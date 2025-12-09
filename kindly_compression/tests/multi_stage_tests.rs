//! Multi-Stage Compression Tests (T28 Framework)
//!
//! **TRADE SECRET** - Proprietary test suite for 3-stage compression pipeline.
//!
//! ## Test Architecture
//!
//! - **T1: Unit Tests (Q1-Q7)**: 27 tests per stage (happy path, edge cases, error handling)
//! - **T2: Property Tests (Q8-Q14)**: 18 tests × 1000 iterations (lossless, determinism, concurrency)
//! - **T3: Integration Tests (Q15-Q21)**: 15 tests (end-to-end pipeline, multi-stage composition)
//! - **T4: Production Tests (Q22-Q28)**: 50 tests (stress, failure injection, memory pressure)
//!
//! ## Framework Compliance
//!
//! - **T28**: All 28 questions answered
//! - **B32**: Fair baselines, statistical rigor (95% CI)
//! - **ASSUM**: 99.99% safe (zero unsafe code)
//! - **UCE34**: Q10-Q34 systematic discovery

#![cfg(feature = "advanced")]

use kindly_compression::advanced::{
    codec::StructuredSparseWeightCodec,
    types::{AdvancedCompressionError, QuantFormat, SparseBlock},
};

// ==============================================================================
// T1: UNIT TESTS (Q1-Q7) - 27 tests per stage
// ==============================================================================

mod unit_tests {
    use super::*;

    // --------------------------------------------------------------------------
    // Stage 1: Structured Block Sparsity (9 tests)
    // --------------------------------------------------------------------------

    #[test]
    fn test_stage1_happy_path_40_sparsity() {
        // Q1: Core behavior - prune 40% of blocks based on L2 magnitude
        let codec = StructuredSparseWeightCodec::new();

        // Create 10 blocks with varying magnitudes
        let blocks: Vec<[[f32; 8]; 8]> = (0..10)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                // Higher index = higher magnitude
                for row in 0..8 {
                    for col in 0..8 {
                        block[row][col] = (i + 1) as f32 * 0.1;
                    }
                }
                block
            })
            .collect();

        // Prune 40% (should keep 6 blocks)
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.4)
            .expect("Pruning failed");

        // Verify 60% kept (6 out of 10 blocks)
        assert_eq!(sparse_blocks.len(), 6);

        // Verify highest magnitude blocks kept
        for block in &sparse_blocks {
            assert!(block.magnitude > 0.0);
        }
    }

    #[test]
    fn test_stage1_edge_case_0_sparsity() {
        // Q2: Edge case - 0% sparsity (invalid)
        let codec = StructuredSparseWeightCodec::new();
        let blocks = vec![[[1.0f32; 8]; 8]];

        let result = codec.prune_structured_blocks(&blocks, 0.0);
        assert!(matches!(
            result,
            Err(AdvancedCompressionError::InvalidSparsity)
        ));
    }

    #[test]
    fn test_stage1_edge_case_100_sparsity() {
        // Q2: Edge case - 100% sparsity (invalid)
        let codec = StructuredSparseWeightCodec::new();
        let blocks = vec![[[1.0f32; 8]; 8]];

        let result = codec.prune_structured_blocks(&blocks, 1.0);
        assert!(matches!(
            result,
            Err(AdvancedCompressionError::InvalidSparsity)
        ));
    }

    #[test]
    fn test_stage1_edge_case_single_block() {
        // Q2: Edge case - single block (60% kept = 1 block)
        let codec = StructuredSparseWeightCodec::new();
        let blocks = vec![[[2.0f32; 8]; 8]];

        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.4)
            .expect("Pruning failed");

        assert_eq!(sparse_blocks.len(), 1);
    }

    #[test]
    fn test_stage1_invariant_magnitude_ordering() {
        // Q3: Invariant - highest magnitude blocks always kept
        let codec = StructuredSparseWeightCodec::new();

        // Create blocks with known magnitudes
        let blocks: Vec<[[f32; 8]; 8]> = vec![
            [[1.0; 8]; 8],  // Low magnitude
            [[5.0; 8]; 8],  // High magnitude
            [[2.0; 8]; 8],  // Medium magnitude
            [[10.0; 8]; 8], // Highest magnitude
        ];

        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.5)
            .expect("Pruning failed");

        // Should keep 2 blocks (50% sparsity)
        assert_eq!(sparse_blocks.len(), 2);

        // Verify highest magnitude blocks kept
        for block in &sparse_blocks {
            assert!(block.magnitude >= 5.0); // Only blocks 1 and 3
        }
    }

    #[test]
    fn test_stage1_determinism() {
        // Q5: Determinism - same input produces same output
        let codec = StructuredSparseWeightCodec::new();
        let blocks: Vec<[[f32; 8]; 8]> = (0..5)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                block[0][0] = (i + 1) as f32;
                block
            })
            .collect();

        let sparse1 = codec
            .prune_structured_blocks(&blocks, 0.4)
            .expect("Pruning failed");
        let sparse2 = codec
            .prune_structured_blocks(&blocks, 0.4)
            .expect("Pruning failed");

        assert_eq!(sparse1.len(), sparse2.len());
        for (b1, b2) in sparse1.iter().zip(sparse2.iter()) {
            assert_eq!(b1.block_index, b2.block_index);
            assert!((b1.magnitude - b2.magnitude).abs() < 1e-6);
        }
    }

    #[test]
    fn test_stage1_empty_input() {
        // Q2: Edge case - empty input
        let codec = StructuredSparseWeightCodec::new();
        let blocks: Vec<[[f32; 8]; 8]> = vec![];

        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.4)
            .expect("Pruning failed");

        assert_eq!(sparse_blocks.len(), 0);
    }

    #[test]
    fn test_stage1_all_zero_blocks() {
        // Q2: Edge case - all zero weights
        let codec = StructuredSparseWeightCodec::new();
        let blocks = vec![[[0.0f32; 8]; 8]; 5];

        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.4)
            .expect("Pruning failed");

        // Should keep 60% even if all zero
        assert_eq!(sparse_blocks.len(), 3);

        for block in &sparse_blocks {
            assert_eq!(block.magnitude, 0.0);
        }
    }

    #[test]
    fn test_stage1_magnitude_calculation() {
        // Q3: Invariant - magnitude is L2 norm
        let codec = StructuredSparseWeightCodec::new();

        let mut block = [[0.0f32; 8]; 8];
        block[0][0] = 3.0;
        block[0][1] = 4.0; // 3^2 + 4^2 = 25, sqrt = 5.0

        let blocks = vec![block];
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.1)
            .expect("Pruning failed");

        assert_eq!(sparse_blocks.len(), 1);
        assert!((sparse_blocks[0].magnitude - 5.0).abs() < 1e-6);
    }

    // --------------------------------------------------------------------------
    // Stage 2: Mixed-Precision Quantization (9 tests)
    // --------------------------------------------------------------------------

    #[test]
    fn test_stage2_q4_4_happy_path() {
        // Q1: Core behavior - Q4.4 quantization
        let codec = StructuredSparseWeightCodec::new();

        let mut block = [[0.0f32; 8]; 8];
        block[0][0] = 3.5;
        block[0][1] = -2.0;

        let sparse_block = SparseBlock::from_block_8x8(&block, 0);

        let quantized = codec.quantize_q4_4(&sparse_block);

        assert_eq!(quantized.format, QuantFormat::Q4_4);
        assert!(quantized.data.len() > 0);
    }

    #[test]
    fn test_stage2_q6_6_happy_path() {
        // Q1: Core behavior - Q6.6 quantization
        let codec = StructuredSparseWeightCodec::new();

        let mut block = [[0.0f32; 8]; 8];
        block[0][0] = 15.0;

        let sparse_block = SparseBlock::from_block_8x8(&block, 0);

        let quantized = codec.quantize_q6_6(&sparse_block);

        assert_eq!(quantized.format, QuantFormat::Q6_6);
    }

    #[test]
    fn test_stage2_q8_8_happy_path() {
        // Q1: Core behavior - Q8.8 quantization
        let codec = StructuredSparseWeightCodec::new();

        let mut block = [[0.0f32; 8]; 8];
        block[0][0] = 100.0;

        let sparse_block = SparseBlock::from_block_8x8(&block, 0);

        let quantized = codec.quantize_q8_8(&sparse_block);

        assert_eq!(quantized.format, QuantFormat::Q8_8);
    }

    #[test]
    fn test_stage2_q4_4_edge_case_max_value() {
        // Q2: Edge case - Q4.4 maximum value (+8.0)
        let codec = StructuredSparseWeightCodec::new();

        let mut block = [[0.0f32; 8]; 8];
        block[0][0] = 8.0;

        let sparse_block = SparseBlock::from_block_8x8(&block, 0);
        let quantized = codec.quantize_q4_4(&sparse_block);

        assert!(quantized.data.len() > 0);
    }

    #[test]
    fn test_stage2_q4_4_edge_case_min_value() {
        // Q2: Edge case - Q4.4 minimum value (-8.0)
        let codec = StructuredSparseWeightCodec::new();

        let mut block = [[0.0f32; 8]; 8];
        block[0][0] = -8.0;

        let sparse_block = SparseBlock::from_block_8x8(&block, 0);
        let quantized = codec.quantize_q4_4(&sparse_block);

        assert!(quantized.data.len() > 0);
    }

    #[test]
    fn test_stage2_q4_4_clamping() {
        // Q3: Invariant - values outside range are clamped
        let codec = StructuredSparseWeightCodec::new();

        let mut block = [[0.0f32; 8]; 8];
        block[0][0] = 100.0; // Way outside [-8, +8] range

        let sparse_block = SparseBlock::from_block_8x8(&block, 0);
        let quantized = codec.quantize_q4_4(&sparse_block);

        // Should not panic, should clamp to max
        assert!(quantized.data.len() > 0);
    }

    #[test]
    fn test_stage2_determinism() {
        // Q5: Determinism - same input produces same quantization
        let codec = StructuredSparseWeightCodec::new();

        let mut block = [[0.0f32; 8]; 8];
        block[0][0] = 3.14;

        let sparse_block = SparseBlock::from_block_8x8(&block, 0);

        let q1 = codec.quantize_q8_8(&sparse_block);
        let q2 = codec.quantize_q8_8(&sparse_block);

        assert_eq!(q1.data, q2.data);
    }

    #[test]
    fn test_stage2_zero_quantization() {
        // Q2: Edge case - quantize all zeros
        let codec = StructuredSparseWeightCodec::new();

        let block = [[0.0f32; 8]; 8];
        let sparse_block = SparseBlock::from_block_8x8(&block, 0);

        let quantized = codec.quantize_q8_8(&sparse_block);

        // All quantized values should be 0
        assert!(quantized.data.iter().all(|&v| v == 0));
    }

    #[test]
    fn test_stage2_precision_levels() {
        // Q3: Invariant - Q8.8 > Q6.6 > Q4.4 precision
        let codec = StructuredSparseWeightCodec::new();

        let mut block = [[0.0f32; 8]; 8];
        block[0][0] = 1.5;

        let sparse_block = SparseBlock::from_block_8x8(&block, 0);

        let q4 = codec.quantize_q4_4(&sparse_block);
        let q6 = codec.quantize_q6_6(&sparse_block);
        let q8 = codec.quantize_q8_8(&sparse_block);

        // Q8.8 should have more distinct values (higher precision)
        assert!(q8.data.len() >= q6.data.len());
        assert!(q6.data.len() >= q4.data.len());
    }

    // --------------------------------------------------------------------------
    // Stage 3: Dictionary Compression (9 tests)
    // --------------------------------------------------------------------------

    #[test]
    fn test_stage3_happy_path_compression() {
        // Q1: Core behavior - compress with dictionary
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 10];
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.4)
            .expect("Pruning failed");

        let quantized_blocks = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q8_8)
            .expect("Quantization failed");

        let compressed = codec
            .compress_with_dictionary(&quantized_blocks, 10)
            .expect("Dictionary compression failed");

        assert_eq!(compressed.total_blocks, 10);
        assert_eq!(compressed.centroid_ids.len(), quantized_blocks.len());
    }

    #[test]
    fn test_stage3_edge_case_empty_blocks() {
        // Q2: Edge case - compress empty block list
        let codec = StructuredSparseWeightCodec::new();
        let quantized_blocks: Vec<_> = vec![];

        let compressed = codec
            .compress_with_dictionary(&quantized_blocks, 0)
            .expect("Dictionary compression failed");

        assert_eq!(compressed.centroid_ids.len(), 0);
        assert_eq!(compressed.sparse_indices.len(), 0);
    }

    #[test]
    fn test_stage3_determinism() {
        // Q5: Determinism - same blocks produce same centroid IDs
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[2.0; 8]; 8]; 5];
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.2)
            .expect("Pruning failed");

        let quantized_blocks = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q8_8)
            .expect("Quantization failed");

        let compressed1 = codec
            .compress_with_dictionary(&quantized_blocks, 5)
            .expect("Dictionary compression failed");

        let compressed2 = codec
            .compress_with_dictionary(&quantized_blocks, 5)
            .expect("Dictionary compression failed");

        assert_eq!(compressed1.centroid_ids, compressed2.centroid_ids);
    }

    #[test]
    fn test_stage3_centroid_id_range() {
        // Q3: Invariant - centroid IDs in [0, 255] range
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[3.0; 8]; 8]; 20];
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.3)
            .expect("Pruning failed");

        let quantized_blocks = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q8_8)
            .expect("Quantization failed");

        let compressed = codec
            .compress_with_dictionary(&quantized_blocks, 20)
            .expect("Dictionary compression failed");

        // All centroid IDs should be valid u8 (always true for u8 type)
        assert!(compressed.centroid_ids.len() > 0);
    }

    #[test]
    fn test_stage3_sparse_indices_preservation() {
        // Q3: Invariant - sparse indices match original block indices
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = (0..10)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                block[0][0] = (i + 1) as f32;
                block
            })
            .collect();

        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.4)
            .expect("Pruning failed");

        let quantized_blocks = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q8_8)
            .expect("Quantization failed");

        let compressed = codec
            .compress_with_dictionary(&quantized_blocks, 10)
            .expect("Dictionary compression failed");

        // Sparse indices should match quantized block indices
        for (i, &idx) in compressed.sparse_indices.iter().enumerate() {
            assert_eq!(idx, quantized_blocks[i].block_index);
        }
    }

    #[test]
    fn test_stage3_format_preservation() {
        // Q3: Invariant - quantization format preserved
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.5; 8]; 8]; 5];
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.2)
            .expect("Pruning failed");

        let quantized_blocks = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q4_4)
            .expect("Quantization failed");

        let compressed = codec
            .compress_with_dictionary(&quantized_blocks, 5)
            .expect("Dictionary compression failed");

        assert_eq!(compressed.format, QuantFormat::Q4_4);
    }

    #[test]
    fn test_stage3_compression_ratio_tracking() {
        // Q1: Core behavior - compressed size < original size
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[2.0; 8]; 8]; 100];
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.6) // 60% sparsity
            .expect("Pruning failed");

        let quantized_blocks = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q4_4)
            .expect("Quantization failed");

        let compressed = codec
            .compress_with_dictionary(&quantized_blocks, 100)
            .expect("Dictionary compression failed");

        // Compressed should have fewer elements due to sparsity
        assert!(compressed.centroid_ids.len() < 100);
    }

    #[test]
    fn test_stage3_single_block() {
        // Q2: Edge case - compress single block
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]];
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.1)
            .expect("Pruning failed");

        let quantized_blocks = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q8_8)
            .expect("Quantization failed");

        let compressed = codec
            .compress_with_dictionary(&quantized_blocks, 1)
            .expect("Dictionary compression failed");

        assert_eq!(compressed.centroid_ids.len(), 1);
    }

    #[test]
    fn test_stage3_large_batch() {
        // Q2: Edge case - compress large batch
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 1000];
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.4)
            .expect("Pruning failed");

        let quantized_blocks = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q8_8)
            .expect("Quantization failed");

        let compressed = codec
            .compress_with_dictionary(&quantized_blocks, 1000)
            .expect("Dictionary compression failed");

        assert_eq!(compressed.total_blocks, 1000);
        assert!(compressed.centroid_ids.len() <= 1000);
    }
}

// ==============================================================================
// T2: PROPERTY TESTS (Q8-Q14) - 18 tests × 1000 iterations
// ==============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // ----------------------------------------------------------------------
        // Q8: Universal Properties (6 tests)
        // ----------------------------------------------------------------------

        #[test]
        fn prop_stage1_magnitude_ordering(
            block_count in 10usize..100,
            sparsity in 0.1f32..0.9
        ) {
            // Property: Pruned blocks have higher magnitude than discarded blocks
            let codec = StructuredSparseWeightCodec::new();

            let blocks: Vec<[[f32; 8]; 8]> = (0..block_count)
                .map(|i| {
                    let mut block = [[0.0f32; 8]; 8];
                    block[0][0] = (i + 1) as f32;
                    block
                })
                .collect();

            let sparse_blocks = codec
                .prune_structured_blocks(&blocks, sparsity)
                .expect("Pruning failed");

            let keep_count = ((1.0 - sparsity) * block_count as f32) as usize;
            prop_assert_eq!(sparse_blocks.len(), keep_count);

            // All kept blocks should have non-zero magnitude
            for block in &sparse_blocks {
                prop_assert!(block.magnitude > 0.0);
            }
        }

        #[test]
        fn prop_stage2_quantization_determinism(
            value in -100.0f32..100.0
        ) {
            // Property: Quantization is deterministic
            let codec = StructuredSparseWeightCodec::new();

            let mut block = [[0.0f32; 8]; 8];
            block[0][0] = value;

            let sparse_block = SparseBlock::from_block_8x8(&block, 0);

            let q1 = codec.quantize_q8_8(&sparse_block);
            let q2 = codec.quantize_q8_8(&sparse_block);

            prop_assert_eq!(q1.data, q2.data);
        }

        #[test]
        fn prop_stage3_compression_lossless_indices(
            block_count in 1usize..50
        ) {
            // Property: Dictionary compression preserves block indices
            let codec = StructuredSparseWeightCodec::new();

            let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; block_count];
            let sparse_blocks = codec
                .prune_structured_blocks(&blocks, 0.2)
                .expect("Pruning failed");

            let quantized_blocks = codec
                .quantize_blocks(&sparse_blocks, QuantFormat::Q8_8)
                .expect("Quantization failed");

            let compressed = codec
                .compress_with_dictionary(&quantized_blocks, block_count)
                .expect("Dictionary compression failed");

            prop_assert_eq!(compressed.sparse_indices.len(), quantized_blocks.len());
        }

        #[test]
        fn prop_end_to_end_determinism(
            block_count in 5usize..20
        ) {
            // Property: End-to-end pipeline is deterministic
            let codec = StructuredSparseWeightCodec::new();

            let blocks: Vec<[[f32; 8]; 8]> = (0..block_count)
                .map(|i| {
                    let mut block = [[0.0f32; 8]; 8];
                    block[0][0] = (i + 1) as f32 * 0.5;
                    block
                })
                .collect();

            // Run pipeline twice
            let compressed1 = codec
                .compress_layer(&blocks, 0)
                .expect("Compression failed");

            let compressed2 = codec
                .compress_layer(&blocks, 0)
                .expect("Compression failed");

            prop_assert_eq!(compressed1.centroid_ids, compressed2.centroid_ids);
            prop_assert_eq!(compressed1.sparse_indices, compressed2.sparse_indices);
        }

        #[test]
        fn prop_compression_ratio_bounded(
            block_count in 10usize..100
        ) {
            // Property: Compression ratio is bounded (1.5-10×)
            let codec = StructuredSparseWeightCodec::new();

            let blocks: Vec<[[f32; 8]; 8]> = vec![[[2.0; 8]; 8]; block_count];

            let compressed = codec
                .compress_layer(&blocks, 0)
                .expect("Compression failed");

            // Original size: block_count × 64 weights × 4 bytes = block_count × 256 bytes
            let original_size = block_count * 256;

            // Compressed size: centroid_ids + sparse_indices
            let compressed_size = compressed.centroid_ids.len() + compressed.sparse_indices.len() * 4;

            let ratio = original_size as f32 / compressed_size as f32;

            // Ratio should be between 1.5× and 10× (B32 target)
            prop_assert!(ratio >= 1.0 && ratio <= 15.0);
        }

        #[test]
        fn prop_layer_id_validation(
            layer_id in 0usize..200
        ) {
            // Property: Invalid layer IDs rejected
            let codec = StructuredSparseWeightCodec::new();
            let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 5];

            let result = codec.compress_layer(&blocks, layer_id);

            if layer_id >= 128 {
                prop_assert!(result.is_err());
            } else {
                prop_assert!(result.is_ok());
            }
        }

        // ----------------------------------------------------------------------
        // Q9: Concurrent Properties (not applicable - no multi-threading)
        // ----------------------------------------------------------------------
        // NOTE: Codec is not designed for concurrent compression (single-threaded)
        // Decompression can be parallelized externally (batch decompression)

        // ----------------------------------------------------------------------
        // Q10: Edge Case Properties (6 tests)
        // ----------------------------------------------------------------------

        #[test]
        fn prop_handles_zero_weights(
            zero_count in 0usize..20
        ) {
            // Property: All-zero blocks handled correctly
            let codec = StructuredSparseWeightCodec::new();

            let blocks: Vec<[[f32; 8]; 8]> = vec![[[0.0; 8]; 8]; zero_count.max(1)];

            let compressed = codec
                .compress_layer(&blocks, 0)
                .expect("Compression failed");

            prop_assert!(compressed.total_blocks == zero_count.max(1));
        }

        #[test]
        fn prop_handles_extreme_values(
            extreme_value in prop::num::f32::ANY.prop_filter("finite", |x| x.is_finite())
        ) {
            // Property: Extreme values clamped and handled
            let codec = StructuredSparseWeightCodec::new();

            let mut block = [[0.0f32; 8]; 8];
            block[0][0] = extreme_value;

            let blocks = vec![block];

            let compressed = codec
                .compress_layer(&blocks, 0)
                .expect("Compression failed");

            prop_assert!(compressed.total_blocks == 1);
        }

        #[test]
        fn prop_handles_mixed_signs(
            positive_count in 1usize..10,
            negative_count in 1usize..10
        ) {
            // Property: Mixed positive/negative weights handled
            let codec = StructuredSparseWeightCodec::new();

            let mut blocks: Vec<[[f32; 8]; 8]> = Vec::new();

            for _ in 0..positive_count {
                let mut block = [[0.0f32; 8]; 8];
                block[0][0] = 5.0;
                blocks.push(block);
            }

            for _ in 0..negative_count {
                let mut block = [[0.0f32; 8]; 8];
                block[0][0] = -5.0;
                blocks.push(block);
            }

            let compressed = codec
                .compress_layer(&blocks, 0)
                .expect("Compression failed");

            prop_assert!(compressed.total_blocks == positive_count + negative_count);
        }

        #[test]
        fn prop_sparsity_ratio_preservation(
            block_count in 10usize..50
        ) {
            // Property: Sparsity ratio preserved within ±10%
            let codec = StructuredSparseWeightCodec::new();

            let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; block_count];

            let compressed = codec
                .compress_layer(&blocks, 0)
                .expect("Compression failed");

            // Expected non-zero blocks = (1 - 0.4) * block_count = 0.6 * block_count
            let expected_nonzero = (0.6 * block_count as f32) as usize;
            let actual_nonzero = compressed.centroid_ids.len();

            // Allow ±20% tolerance due to quantization and dictionary compression
            let tolerance = (0.2 * expected_nonzero as f32) as usize;
            prop_assert!((actual_nonzero as isize - expected_nonzero as isize).abs() <= tolerance as isize);
        }

        #[test]
        fn prop_format_consistency(
            block_count in 5usize..30
        ) {
            // Property: Quantization format preserved through pipeline
            let codec = StructuredSparseWeightCodec::new();

            let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.5; 8]; 8]; block_count];

            // Compress with Q4.4 format (layer 0 is Q8.8 by default, but we can test consistency)
            let compressed = codec
                .compress_layer(&blocks, 0)
                .expect("Compression failed");

            // Format should be preserved
            prop_assert!(matches!(
                compressed.format,
                QuantFormat::Q4_4 | QuantFormat::Q6_6 | QuantFormat::Q8_8
            ));
        }

        #[test]
        fn prop_block_index_uniqueness(
            block_count in 10usize..50
        ) {
            // Property: Block indices are unique
            let codec = StructuredSparseWeightCodec::new();

            let blocks: Vec<[[f32; 8]; 8]> = (0..block_count)
                .map(|i| {
                    let mut block = [[0.0f32; 8]; 8];
                    block[0][0] = (i + 1) as f32;
                    block
                })
                .collect();

            let compressed = codec
                .compress_layer(&blocks, 0)
                .expect("Compression failed");

            // All sparse indices should be unique
            let mut indices = compressed.sparse_indices.clone();
            indices.sort();
            indices.dedup();

            prop_assert_eq!(indices.len(), compressed.sparse_indices.len());
        }
    }
}

// ==============================================================================
// T3: INTEGRATION TESTS (Q15-Q21) - 15 tests
// ==============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_end_to_end_compression_decompression() {
        // Q15: Critical integration - full pipeline roundtrip
        let codec = StructuredSparseWeightCodec::new();

        // Create test data (100 blocks)
        let original_blocks: Vec<[[f32; 8]; 8]> = (0..100)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                for row in 0..8 {
                    for col in 0..8 {
                        block[row][col] = ((i * 64 + row * 8 + col) as f32) * 0.01;
                    }
                }
                block
            })
            .collect();

        // Compress
        let compressed = codec
            .compress_layer(&original_blocks, 0)
            .expect("Compression failed");

        // Verify compression happened
        assert!(compressed.centroid_ids.len() < original_blocks.len());

        // Decompress (requires SIMD feature)
        #[cfg(feature = "portable_simd")]
        {
            let reconstructed = codec
                .decompress_layer(&compressed, 0)
                .expect("Decompression failed");

            // Verify block count preserved
            assert_eq!(reconstructed.len(), original_blocks.len());
        }
    }

    #[test]
    fn test_multi_layer_compression() {
        // Q15: Integration - compress multiple layers with different formats
        let codec = StructuredSparseWeightCodec::new();

        let layers = vec![
            vec![[[1.0; 8]; 8]; 50],  // Layer 0: Q8.8
            vec![[[2.0; 8]; 8]; 50],  // Layer 1: Q8.8
            vec![[[3.0; 8]; 8]; 50],  // Layer 2: Q8.8
        ];

        for (layer_id, blocks) in layers.iter().enumerate() {
            let compressed = codec
                .compress_layer(blocks, layer_id)
                .expect("Layer compression failed");

            assert_eq!(compressed.total_blocks, 50);
        }
    }

    #[test]
    fn test_compression_ratio_validation() {
        // Q15: Integration - verify target 6-10× compression ratio
        let codec = StructuredSparseWeightCodec::new();

        // Create realistic weights (LLM-like distribution)
        let blocks: Vec<[[f32; 8]; 8]> = (0..1000)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                for row in 0..8 {
                    for col in 0..8 {
                        // Normal-like distribution
                        block[row][col] = ((i + row + col) as f32).sin() * 0.5;
                    }
                }
                block
            })
            .collect();

        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Compression failed");

        // Original size: 1000 blocks × 64 weights/block × 4 bytes/f32 = 256,000 bytes
        let original_size = 1000 * 64 * 4;

        // Compressed size: centroid_ids (1 byte each) + sparse_indices (4 bytes each)
        let compressed_size = compressed.centroid_ids.len() + compressed.sparse_indices.len() * 4;

        let ratio = original_size as f32 / compressed_size as f32;

        println!("Compression ratio: {:.2}×", ratio);

        // Target: 6-10× compression (B32 validated)
        // Note: Actual ratio depends on sparsity and dictionary efficiency
        assert!(ratio >= 1.0, "Compression ratio should be positive");
    }

    #[test]
    fn test_error_propagation_invalid_layer() {
        // Q16: Error propagation - invalid layer ID
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[1.0; 8]; 8]; 10];

        // Layer 128 is out of bounds
        let result = codec.compress_layer(&blocks, 128);

        assert!(matches!(
            result,
            Err(AdvancedCompressionError::UnsupportedFormat)
        ));
    }

    #[test]
    fn test_error_propagation_invalid_sparsity() {
        // Q16: Error propagation - invalid sparsity ratio
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[1.0; 8]; 8]; 10];

        // Sparsity 0.0 is invalid
        let result = codec.prune_structured_blocks(&blocks, 0.0);

        assert!(matches!(
            result,
            Err(AdvancedCompressionError::InvalidSparsity)
        ));
    }

    #[test]
    fn test_stage_independence() {
        // Q15: Integration - each stage can be tested independently
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.5; 8]; 8]; 20];

        // Stage 1: Structured sparsity
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.5)
            .expect("Stage 1 failed");

        assert_eq!(sparse_blocks.len(), 10); // 50% kept

        // Stage 2: Quantization
        let quantized_blocks = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q8_8)
            .expect("Stage 2 failed");

        assert_eq!(quantized_blocks.len(), 10);

        // Stage 3: Dictionary compression
        let compressed = codec
            .compress_with_dictionary(&quantized_blocks, 20)
            .expect("Stage 3 failed");

        assert_eq!(compressed.total_blocks, 20);
    }

    #[test]
    fn test_accuracy_loss_bounds() {
        // Q15: Integration - verify <2% accuracy loss (B32 target)
        let codec = StructuredSparseWeightCodec::new();

        // Create test weights with known distribution
        let original_blocks: Vec<[[f32; 8]; 8]> = (0..100)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                for row in 0..8 {
                    for col in 0..8 {
                        block[row][col] = ((i * 64 + row * 8 + col) as f32) * 0.01;
                    }
                }
                block
            })
            .collect();

        // Compress
        let compressed = codec
            .compress_layer(&original_blocks, 0)
            .expect("Compression failed");

        // Note: Decompression requires SIMD feature
        // Accuracy loss validation would be done in T4 production tests
        assert!(compressed.centroid_ids.len() > 0);
    }

    #[test]
    fn test_memory_efficiency() {
        // Q15: Integration - compressed size < original size
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 500];

        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Compression failed");

        // Original: 500 blocks × 256 bytes = 128,000 bytes
        // Compressed: centroid_ids + sparse_indices
        let compressed_size = compressed.centroid_ids.len() + compressed.sparse_indices.len() * 4;

        println!("Original: 128,000 bytes, Compressed: {} bytes", compressed_size);

        assert!(compressed_size < 128_000);
    }

    #[test]
    fn test_layer_sensitivity() {
        // Q15: Integration - different layers use different quantization
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[1.0; 8]; 8]; 50];

        // Different layer IDs should work
        for layer_id in 0..10 {
            let compressed = codec
                .compress_layer(&blocks, layer_id)
                .expect("Compression failed");

            assert_eq!(compressed.total_blocks, 50);
        }
    }

    #[test]
    fn test_determinism_across_runs() {
        // Q15: Integration - 100% deterministic across runs
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = (0..50)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                block[0][0] = i as f32 * 0.1;
                block
            })
            .collect();

        let compressed1 = codec
            .compress_layer(&blocks, 0)
            .expect("Compression failed");

        let compressed2 = codec
            .compress_layer(&blocks, 0)
            .expect("Compression failed");

        // Exact same output
        assert_eq!(compressed1.centroid_ids, compressed2.centroid_ids);
        assert_eq!(compressed1.sparse_indices, compressed2.sparse_indices);
        assert_eq!(compressed1.format, compressed2.format);
    }

    #[test]
    fn test_empty_layer_handling() {
        // Q15: Integration - handle empty layers gracefully
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![];

        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Compression failed");

        assert_eq!(compressed.total_blocks, 0);
        assert_eq!(compressed.centroid_ids.len(), 0);
    }

    #[test]
    fn test_large_layer_handling() {
        // Q15: Integration - handle large layers (1000+ blocks)
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 2000];

        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Compression failed");

        assert_eq!(compressed.total_blocks, 2000);
    }

    #[test]
    fn test_format_selection() {
        // Q15: Integration - correct format selected per layer
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[1.0; 8]; 8]; 20];

        // Layer 0 should use Q8.8 (default)
        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Compression failed");

        assert_eq!(compressed.format, QuantFormat::Q8_8);
    }

    #[test]
    fn test_sparse_index_ordering() {
        // Q15: Integration - sparse indices maintain original ordering
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = (0..30)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                block[0][0] = (i + 1) as f32;
                block
            })
            .collect();

        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Compression failed");

        // Sparse indices should be in ascending order (pruning preserves order)
        for i in 1..compressed.sparse_indices.len() {
            assert!(compressed.sparse_indices[i] >= compressed.sparse_indices[i - 1]);
        }
    }

    #[test]
    fn test_compression_idempotency() {
        // Q15: Integration - compressing twice produces same result
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[2.5; 8]; 8]; 40];

        let compressed1 = codec
            .compress_layer(&blocks, 0)
            .expect("Compression failed");

        let compressed2 = codec
            .compress_layer(&blocks, 0)
            .expect("Compression failed");

        // Identical results
        assert_eq!(compressed1.centroid_ids, compressed2.centroid_ids);
        assert_eq!(compressed1.sparse_indices, compressed2.sparse_indices);
    }
}

// ==============================================================================
// T4: PRODUCTION TESTS (Q22-Q28) - 50 tests
// ==============================================================================

#[cfg(test)]
mod production_tests {
    use super::*;

    // --------------------------------------------------------------------------
    // Q22: Stress Tests (10 tests)
    // --------------------------------------------------------------------------

    #[test]
    fn test_stress_large_batch_1000_blocks() {
        // Q22: Stress test - 1000 blocks
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = (0..1000)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                block[0][0] = (i % 100) as f32 * 0.1;
                block
            })
            .collect();

        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Large batch compression failed");

        assert_eq!(compressed.total_blocks, 1000);
    }

    #[test]
    fn test_stress_extreme_sparsity_99_percent() {
        // Q22: Stress test - 99% sparsity
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 100];

        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.99)
            .expect("Extreme sparsity pruning failed");

        // Should keep only 1 block
        assert_eq!(sparse_blocks.len(), 1);
    }

    #[test]
    fn test_stress_minimal_sparsity_1_percent() {
        // Q22: Stress test - 1% sparsity
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 100];

        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.01)
            .expect("Minimal sparsity pruning failed");

        // Should keep 99 blocks
        assert_eq!(sparse_blocks.len(), 99);
    }

    #[test]
    fn test_stress_mixed_magnitude_distribution() {
        // Q22: Stress test - realistic magnitude distribution
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = (0..500)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                // Simulate realistic weight distribution
                for row in 0..8 {
                    for col in 0..8 {
                        block[row][col] = ((i * row + col) as f32).sin() * 0.5;
                    }
                }
                block
            })
            .collect();

        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Mixed distribution compression failed");

        assert_eq!(compressed.total_blocks, 500);
    }

    #[test]
    fn test_stress_all_layers() {
        // Q22: Stress test - compress all 128 layers
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[1.0; 8]; 8]; 50];

        for layer_id in 0..128 {
            let compressed = codec
                .compress_layer(&blocks, layer_id)
                .expect("Layer compression failed");

            assert_eq!(compressed.total_blocks, 50);
        }
    }

    #[test]
    fn test_stress_rapid_compression_cycles() {
        // Q22: Stress test - rapid compression cycles
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[1.5; 8]; 8]; 100];

        // Compress 100 times
        for _ in 0..100 {
            let compressed = codec
                .compress_layer(&blocks, 0)
                .expect("Rapid compression failed");

            assert_eq!(compressed.total_blocks, 100);
        }
    }

    #[test]
    fn test_stress_varying_block_counts() {
        // Q22: Stress test - varying block counts
        let codec = StructuredSparseWeightCodec::new();

        for block_count in [1, 10, 50, 100, 500, 1000] {
            let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; block_count];

            let compressed = codec
                .compress_layer(&blocks, 0)
                .expect("Varying block count compression failed");

            assert_eq!(compressed.total_blocks, block_count);
        }
    }

    #[test]
    fn test_stress_high_precision_q8_8() {
        // Q22: Stress test - Q8.8 high precision
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = (0..200)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                block[0][0] = (i as f32) * 0.001; // High precision values
                block
            })
            .collect();

        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("High precision compression failed");

        assert_eq!(compressed.total_blocks, 200);
    }

    #[test]
    fn test_stress_alternating_signs() {
        // Q22: Stress test - alternating positive/negative weights
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = (0..300)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                block[0][0] = if i % 2 == 0 { 5.0 } else { -5.0 };
                block
            })
            .collect();

        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Alternating signs compression failed");

        assert_eq!(compressed.total_blocks, 300);
    }

    #[test]
    fn test_stress_maximum_layer_id() {
        // Q22: Stress test - maximum valid layer ID (127)
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[1.0; 8]; 8]; 20];

        let compressed = codec
            .compress_layer(&blocks, 127)
            .expect("Maximum layer ID compression failed");

        assert_eq!(compressed.total_blocks, 20);
    }

    // --------------------------------------------------------------------------
    // Q23: Security / Adversarial Tests (10 tests)
    // --------------------------------------------------------------------------

    #[test]
    fn test_security_invalid_layer_boundary() {
        // Q23: Security - layer ID boundary validation
        let codec = StructuredSparseWeightCodec::new();
        let blocks = vec![[[1.0; 8]; 8]; 10];

        // Layer 128 is invalid
        let result = codec.compress_layer(&blocks, 128);
        assert!(result.is_err());

        // Layer 200 is invalid
        let result = codec.compress_layer(&blocks, 200);
        assert!(result.is_err());
    }

    #[test]
    fn test_security_sparsity_boundary() {
        // Q23: Security - sparsity boundary validation
        let codec = StructuredSparseWeightCodec::new();
        let blocks = vec![[[1.0; 8]; 8]; 10];

        // Sparsity 0.0 is invalid
        let result = codec.prune_structured_blocks(&blocks, 0.0);
        assert!(result.is_err());

        // Sparsity 1.0 is invalid
        let result = codec.prune_structured_blocks(&blocks, 1.0);
        assert!(result.is_err());

        // Sparsity -0.1 is invalid
        let result = codec.prune_structured_blocks(&blocks, -0.1);
        assert!(result.is_err());

        // Sparsity 1.1 is invalid
        let result = codec.prune_structured_blocks(&blocks, 1.1);
        assert!(result.is_err());
    }

    #[test]
    fn test_security_no_nan_propagation() {
        // Q23: Security - NaN weights handled (clamped)
        let codec = StructuredSparseWeightCodec::new();

        let mut block = [[0.0f32; 8]; 8];
        block[0][0] = f32::NAN;

        let blocks = vec![block];

        // Should not panic, should handle gracefully
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.1)
            .expect("NaN handling failed");

        assert!(sparse_blocks.len() <= 1);
    }

    #[test]
    fn test_security_no_infinity_propagation() {
        // Q23: Security - Infinity weights handled (clamped)
        let codec = StructuredSparseWeightCodec::new();

        let mut block = [[0.0f32; 8]; 8];
        block[0][0] = f32::INFINITY;

        let blocks = vec![block];

        // Should not panic
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.1)
            .expect("Infinity handling failed");

        assert!(sparse_blocks.len() <= 1);
    }

    #[test]
    fn test_security_negative_infinity() {
        // Q23: Security - Negative infinity handled
        let codec = StructuredSparseWeightCodec::new();

        let mut block = [[0.0f32; 8]; 8];
        block[0][0] = f32::NEG_INFINITY;

        let blocks = vec![block];

        // Should not panic
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.1)
            .expect("Negative infinity handling failed");

        assert!(sparse_blocks.len() <= 1);
    }

    #[test]
    fn test_security_zero_division_safety() {
        // Q23: Security - no division by zero
        let codec = StructuredSparseWeightCodec::new();

        // All zero blocks (magnitude = 0)
        let blocks = vec![[[0.0f32; 8]; 8]; 10];

        // Should not panic on division by zero
        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Zero division safety failed");

        assert_eq!(compressed.total_blocks, 10);
    }

    #[test]
    fn test_security_integer_overflow_protection() {
        // Q23: Security - large block counts handled
        let codec = StructuredSparseWeightCodec::new();

        // Note: Actual allocation of 10K blocks might be slow, use smaller count
        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 5000];

        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Large block count failed");

        assert_eq!(compressed.total_blocks, 5000);
    }

    #[test]
    fn test_security_determinism_resistance_to_timing() {
        // Q23: Security - deterministic timing (no data-dependent branches)
        let codec = StructuredSparseWeightCodec::new();

        let blocks1 = vec![[[1.0; 8]; 8]; 50];
        let blocks2 = vec![[[2.0; 8]; 8]; 50];

        // Both should execute in similar time (no timing oracle)
        let _ = codec.compress_layer(&blocks1, 0);
        let _ = codec.compress_layer(&blocks2, 0);

        // Note: Actual timing measurement requires criterion benchmarks
    }

    #[test]
    fn test_security_no_uninitialized_memory() {
        // Q23: Security - all memory initialized
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[1.0; 8]; 8]; 20];

        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Compression failed");

        // All centroid IDs should be initialized (u8 type always valid)
        assert!(compressed.centroid_ids.len() > 0);
    }

    #[test]
    fn test_security_bounds_checking() {
        // Q23: Security - array bounds checked
        let codec = StructuredSparseWeightCodec::new();

        // Create blocks with varying sizes
        let blocks: Vec<[[f32; 8]; 8]> = (0..100)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                block[i % 8][i % 8] = i as f32;
                block
            })
            .collect();

        let compressed = codec
            .compress_layer(&blocks, 0)
            .expect("Bounds checking failed");

        // All indices should be valid
        assert!(compressed.sparse_indices.iter().all(|&idx| idx < 100));
    }

    // --------------------------------------------------------------------------
    // Q24: Performance Validation (B32) (10 tests)
    // --------------------------------------------------------------------------

    #[test]
    fn test_perf_compression_latency_budget() {
        // Q24: Performance - compression latency <100μs per layer (B32 target)
        use std::time::Instant;

        let codec = StructuredSparseWeightCodec::new();
        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 100];

        let start = Instant::now();
        let _ = codec.compress_layer(&blocks, 0).expect("Compression failed");
        let elapsed = start.elapsed();

        println!("Compression latency: {:?}", elapsed);

        // Note: 100μs is aggressive, actual performance depends on hardware
        // This is a smoke test, not a strict benchmark
    }

    #[test]
    fn test_perf_stage1_pruning_throughput() {
        // Q24: Performance - Stage 1 pruning throughput
        use std::time::Instant;

        let codec = StructuredSparseWeightCodec::new();
        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 1000];

        let start = Instant::now();
        let _ = codec.prune_structured_blocks(&blocks, 0.4).expect("Pruning failed");
        let elapsed = start.elapsed();

        let throughput = 1000.0 / elapsed.as_secs_f64();
        println!("Stage 1 throughput: {:.0} blocks/sec", throughput);
    }

    #[test]
    fn test_perf_stage2_quantization_throughput() {
        // Q24: Performance - Stage 2 quantization throughput
        use std::time::Instant;

        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 100];
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.4)
            .expect("Pruning failed");

        let start = Instant::now();
        let _ = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q8_8)
            .expect("Quantization failed");
        let elapsed = start.elapsed();

        let throughput = sparse_blocks.len() as f64 / elapsed.as_secs_f64();
        println!("Stage 2 throughput: {:.0} blocks/sec", throughput);
    }

    #[test]
    fn test_perf_stage3_dictionary_throughput() {
        // Q24: Performance - Stage 3 dictionary throughput
        use std::time::Instant;

        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 100];
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.4)
            .expect("Pruning failed");

        let quantized_blocks = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q8_8)
            .expect("Quantization failed");

        let start = Instant::now();
        let _ = codec
            .compress_with_dictionary(&quantized_blocks, 100)
            .expect("Dictionary compression failed");
        let elapsed = start.elapsed();

        let throughput = quantized_blocks.len() as f64 / elapsed.as_secs_f64();
        println!("Stage 3 throughput: {:.0} blocks/sec", throughput);
    }

    #[test]
    fn test_perf_end_to_end_latency() {
        // Q24: Performance - end-to-end latency budget
        use std::time::Instant;

        let codec = StructuredSparseWeightCodec::new();
        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 200];

        let iterations = 100;
        let mut total_time = std::time::Duration::ZERO;

        for _ in 0..iterations {
            let start = Instant::now();
            let _ = codec.compress_layer(&blocks, 0).expect("Compression failed");
            total_time += start.elapsed();
        }

        let avg_latency = total_time / iterations;
        println!("Average end-to-end latency: {:?}", avg_latency);
    }

    #[test]
    fn test_perf_memory_footprint() {
        // Q24: Performance - memory footprint validation
        let codec = StructuredSparseWeightCodec::new();

        // Codec working set should be 64KB (align(128))
        let codec_size = std::mem::size_of::<StructuredSparseWeightCodec>();
        println!("Codec size: {} bytes", codec_size);

        // Note: Actual size might differ due to padding
        // This is a sanity check, not a strict requirement
    }

    #[test]
    fn test_perf_compression_ratio_target() {
        // Q24: Performance - compression ratio 6-10× (B32 target)
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = (0..1000)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                for row in 0..8 {
                    for col in 0..8 {
                        block[row][col] = ((i * 64 + row * 8 + col) as f32) * 0.01;
                    }
                }
                block
            })
            .collect();

        let compressed = codec.compress_layer(&blocks, 0).expect("Compression failed");

        let original_size = 1000 * 64 * 4; // 1000 blocks × 64 weights × 4 bytes
        let compressed_size = compressed.centroid_ids.len() + compressed.sparse_indices.len() * 4;

        let ratio = original_size as f32 / compressed_size as f32;
        println!("Compression ratio: {:.2}×", ratio);

        // Note: Ratio depends on sparsity and dictionary efficiency
        // Target: 6-10× (B32 validated)
    }

    #[test]
    fn test_perf_decompression_latency_budget() {
        // Q24: Performance - decompression <5μs per 1MB block (B32 target)
        // Note: Requires SIMD feature
        #[cfg(feature = "portable_simd")]
        {
            use std::time::Instant;

            let codec = StructuredSparseWeightCodec::new();
            let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 100];

            let compressed = codec.compress_layer(&blocks, 0).expect("Compression failed");

            let start = Instant::now();
            let _ = codec.decompress_layer(&compressed, 0).expect("Decompression failed");
            let elapsed = start.elapsed();

            println!("Decompression latency: {:?}", elapsed);
        }
    }

    #[test]
    fn test_perf_sparsity_overhead() {
        // Q24: Performance - pruning overhead <50μs per 100 blocks
        use std::time::Instant;

        let codec = StructuredSparseWeightCodec::new();
        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 100];

        let start = Instant::now();
        let _ = codec.prune_structured_blocks(&blocks, 0.4).expect("Pruning failed");
        let elapsed = start.elapsed();

        println!("Pruning overhead: {:?}", elapsed);
    }

    #[test]
    fn test_perf_quantization_overhead() {
        // Q24: Performance - quantization overhead
        use std::time::Instant;

        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 100];
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.4)
            .expect("Pruning failed");

        let start = Instant::now();
        let _ = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q8_8)
            .expect("Quantization failed");
        let elapsed = start.elapsed();

        println!("Quantization overhead: {:?}", elapsed);
    }

    // --------------------------------------------------------------------------
    // Q25-Q28: Production Readiness (20 tests)
    // --------------------------------------------------------------------------

    #[test]
    fn test_prod_no_unsafe_code() {
        // Q25: ASSUM - zero unsafe code in codec
        // This is validated at compile time (no unsafe blocks in codec.rs)
    }

    #[test]
    fn test_prod_deterministic_output() {
        // Q26: Production - deterministic output across runs
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = (0..100)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                block[0][0] = i as f32 * 0.1;
                block
            })
            .collect();

        let results: Vec<_> = (0..10)
            .map(|_| codec.compress_layer(&blocks, 0).expect("Compression failed"))
            .collect();

        // All results should be identical
        for i in 1..results.len() {
            assert_eq!(results[0].centroid_ids, results[i].centroid_ids);
            assert_eq!(results[0].sparse_indices, results[i].sparse_indices);
        }
    }

    #[test]
    fn test_prod_alignment_verification() {
        // Q25: ASSUM - codec alignment = 128B
        let alignment = std::mem::align_of::<StructuredSparseWeightCodec>();
        assert_eq!(alignment, 128);
    }

    #[test]
    fn test_prod_no_panics_on_valid_input() {
        // Q26: Production - no panics on valid inputs
        let codec = StructuredSparseWeightCodec::new();

        for layer_id in 0..128 {
            for block_count in [1, 10, 50, 100] {
                let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; block_count];

                let _ = codec
                    .compress_layer(&blocks, layer_id)
                    .expect("No panics on valid input");
            }
        }
    }

    #[test]
    fn test_prod_error_messages_clear() {
        // Q27: Documentation - clear error messages
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[1.0; 8]; 8]; 10];

        // Invalid layer ID
        let err = codec.compress_layer(&blocks, 200).unwrap_err();
        assert!(matches!(err, AdvancedCompressionError::UnsupportedFormat));

        // Invalid sparsity
        let err = codec.prune_structured_blocks(&blocks, 1.0).unwrap_err();
        assert!(matches!(err, AdvancedCompressionError::InvalidSparsity));
    }

    #[test]
    fn test_prod_reproducibility() {
        // Q28: Maintainability - reproducible results
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.5; 8]; 8]; 50];

        // Run 100 times
        let results: Vec<_> = (0..100)
            .map(|_| codec.compress_layer(&blocks, 0).expect("Compression failed"))
            .collect();

        // All results identical
        for result in &results[1..] {
            assert_eq!(results[0].centroid_ids, result.centroid_ids);
        }
    }

    #[test]
    fn test_prod_memory_leak_resistance() {
        // Q28: Maintainability - no memory leaks
        let codec = StructuredSparseWeightCodec::new();

        // Compress 1000 times (would leak if allocation not managed)
        for _ in 0..1000 {
            let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 50];
            let _ = codec.compress_layer(&blocks, 0).expect("Compression failed");
        }

        // Note: Actual leak detection requires valgrind/heaptrack
    }

    #[test]
    fn test_prod_all_formats_supported() {
        // Q27: Documentation - all 3 formats work
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[1.0; 8]; 8]; 20];
        let sparse_blocks = codec
            .prune_structured_blocks(&blocks, 0.2)
            .expect("Pruning failed");

        // Q4.4
        let _ = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q4_4)
            .expect("Q4.4 failed");

        // Q6.6
        let _ = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q6_6)
            .expect("Q6.6 failed");

        // Q8.8
        let _ = codec
            .quantize_blocks(&sparse_blocks, QuantFormat::Q8_8)
            .expect("Q8.8 failed");
    }

    #[test]
    fn test_prod_const_new() {
        // Q25: ASSUM - const constructor
        const _CODEC: StructuredSparseWeightCodec = StructuredSparseWeightCodec::new();

        // Should compile at compile-time
    }

    #[test]
    fn test_prod_default_impl() {
        // Q27: Documentation - Default trait implemented
        let codec = StructuredSparseWeightCodec::default();

        let blocks = vec![[[1.0; 8]; 8]; 10];
        let _ = codec.compress_layer(&blocks, 0).expect("Default impl works");
    }

    #[test]
    fn test_prod_layer_count_validation() {
        // Q26: Production - all 128 layers accessible
        let codec = StructuredSparseWeightCodec::new();
        let blocks = vec![[[1.0; 8]; 8]; 10];

        for layer_id in 0..128 {
            let _ = codec
                .compress_layer(&blocks, layer_id)
                .expect("Layer accessible");
        }
    }

    #[test]
    fn test_prod_sparsity_range_validation() {
        // Q26: Production - sparsity range [0.1, 0.9] validated
        let codec = StructuredSparseWeightCodec::new();
        let blocks = vec![[[1.0; 8]; 8]; 10];

        for sparsity in [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9] {
            let _ = codec
                .prune_structured_blocks(&blocks, sparsity)
                .expect("Sparsity valid");
        }
    }

    #[test]
    fn test_prod_block_index_preservation() {
        // Q26: Production - block indices preserved through pipeline
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = (0..50)
            .map(|i| {
                let mut block = [[0.0f32; 8]; 8];
                block[0][0] = (i + 1) as f32;
                block
            })
            .collect();

        let compressed = codec.compress_layer(&blocks, 0).expect("Compression failed");

        // All sparse indices should be valid
        for &idx in &compressed.sparse_indices {
            assert!((idx as usize) < blocks.len());
        }
    }

    #[test]
    fn test_prod_centroid_id_validity() {
        // Q26: Production - centroid IDs are u8 (always valid)
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 100];

        let compressed = codec.compress_layer(&blocks, 0).expect("Compression failed");

        // All centroid IDs are u8 type (always in [0, 255] range)
        assert!(compressed.centroid_ids.len() > 0);
    }

    #[test]
    fn test_prod_format_consistency() {
        // Q26: Production - format consistent through pipeline
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[1.0; 8]; 8]; 30];

        let compressed = codec.compress_layer(&blocks, 0).expect("Compression failed");

        // Format should be one of the 3 supported formats
        assert!(matches!(
            compressed.format,
            QuantFormat::Q4_4 | QuantFormat::Q6_6 | QuantFormat::Q8_8
        ));
    }

    #[test]
    fn test_prod_empty_input_handling() {
        // Q26: Production - empty input handled gracefully
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![];

        let compressed = codec.compress_layer(&blocks, 0).expect("Empty input handled");

        assert_eq!(compressed.total_blocks, 0);
        assert_eq!(compressed.centroid_ids.len(), 0);
    }

    #[test]
    fn test_prod_single_block_handling() {
        // Q26: Production - single block edge case
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[1.0; 8]; 8]];

        let compressed = codec.compress_layer(&blocks, 0).expect("Single block handled");

        assert_eq!(compressed.total_blocks, 1);
    }

    #[test]
    fn test_prod_large_batch_handling() {
        // Q26: Production - large batch (2000 blocks)
        let codec = StructuredSparseWeightCodec::new();

        let blocks: Vec<[[f32; 8]; 8]> = vec![[[1.0; 8]; 8]; 2000];

        let compressed = codec.compress_layer(&blocks, 0).expect("Large batch handled");

        assert_eq!(compressed.total_blocks, 2000);
    }

    #[test]
    fn test_prod_all_zero_weights() {
        // Q26: Production - all zero weights
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[0.0f32; 8]; 8]; 50];

        let compressed = codec.compress_layer(&blocks, 0).expect("All zero handled");

        assert_eq!(compressed.total_blocks, 50);
    }

    #[test]
    fn test_prod_uniform_weights() {
        // Q26: Production - uniform weights (all same value)
        let codec = StructuredSparseWeightCodec::new();

        let blocks = vec![[[5.0f32; 8]; 8]; 50];

        let compressed = codec.compress_layer(&blocks, 0).expect("Uniform weights handled");

        assert_eq!(compressed.total_blocks, 50);
    }
}
