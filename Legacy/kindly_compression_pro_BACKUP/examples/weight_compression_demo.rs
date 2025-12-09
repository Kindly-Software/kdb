//! Weight Compression Demonstration
//!
//! Shows the complete 3-stage compression pipeline:
//! 1. Structured block sparsity (40% pruning)
//! 2. Mixed-precision quantization (Q4.4/Q6.6/Q8.8)
//! 3. Dictionary compression (K-means clustering)
//!
//! Run with:
//! ```bash
//! cargo run --example weight_compression_demo --features nightly-all
//! ```

use kindly_compression_pro::{
    StructuredSparseWeightCodec,
    QuantFormat,
};

fn main() {
    println!("Weight Compression Breakthrough Demo");
    println!("=====================================\n");

    // Create codec
    let codec = StructuredSparseWeightCodec::new();

    // Simulate a layer of 8×8 weight blocks
    let num_blocks = 100;
    let mut layer_weights: Vec<[[f32; 8]; 8]> = Vec::with_capacity(num_blocks);

    // Generate synthetic weights (simulating neural network layer)
    for block_idx in 0..num_blocks {
        let mut block = [[0.0f32; 8]; 8];
        for i in 0..8 {
            for j in 0..8 {
                // Simulate weight distribution (some blocks with high magnitude, some low)
                let magnitude = if block_idx % 3 == 0 {
                    // High magnitude blocks (will be kept)
                    ((i + j) as f32) * 0.1 + 0.5
                } else {
                    // Low magnitude blocks (will be pruned)
                    ((i + j) as f32) * 0.01
                };
                block[i][j] = magnitude;
            }
        }
        layer_weights.push(block);
    }

    let layer_id = 0;

    // Original size
    let original_size = num_blocks * 8 * 8 * std::mem::size_of::<f32>();
    println!("Original layer size: {} bytes ({} KB)", original_size, original_size / 1024);
    println!("Number of 8×8 blocks: {}\n", num_blocks);

    // Compress the layer
    println!("Compressing layer...");
    let compressed = codec.compress_layer(&layer_weights, layer_id)
        .expect("Compression failed");

    // Calculate compressed size
    let compressed_size = compressed.centroid_ids.len()  // 8 bits per block
        + compressed.sparse_indices.len() * std::mem::size_of::<u32>()
        + std::mem::size_of::<QuantFormat>();

    println!("Compressed layer size: {} bytes ({} KB)", compressed_size, compressed_size / 1024);
    println!("Compression ratio: {:.2}×", original_size as f32 / compressed_size as f32);
    println!("Sparse blocks kept: {} / {}", compressed.sparse_indices.len(), num_blocks);
    println!("Sparsity: {:.1}%", 100.0 * (1.0 - compressed.sparse_indices.len() as f32 / num_blocks as f32));
    println!();

    // Decompress the layer (SIMD path)
    #[cfg(feature = "portable_simd")]
    {
        println!("Decompressing layer (SIMD path)...");
        let decompressed = codec.decompress_layer(&compressed, layer_id)
            .expect("Decompression failed");

        println!("Decompressed {} blocks\n", decompressed.len());

        // Verify reconstruction accuracy
        let mut total_error = 0.0f32;
        let mut max_error = 0.0f32;

        for (orig, decomp) in layer_weights.iter().zip(decompressed.iter()) {
            for i in 0..8 {
                for j in 0..8 {
                    let error = (orig[i][j] - decomp[i][j]).abs();
                    total_error += error;
                    max_error = max_error.max(error);
                }
            }
        }

        let avg_error = total_error / (num_blocks * 64) as f32;
        println!("Reconstruction quality:");
        println!("  Average error: {:.6}", avg_error);
        println!("  Max error: {:.6}", max_error);
        println!("  Accuracy preservation: {:.2}%", 100.0 * (1.0 - avg_error));
    }

    #[cfg(not(feature = "portable_simd"))]
    {
        println!("SIMD decompression requires --features portable_simd");
        println!("Run with: cargo run --example weight_compression_demo --features nightly-all");
    }

    println!("\n=====================================");
    println!("Compression pipeline summary:");
    println!("  Stage 1 (Sparsity): {:.1}% blocks pruned", 100.0 * (1.0 - compressed.sparse_indices.len() as f32 / num_blocks as f32));
    println!("  Stage 2 (Quantization): {:?} format", compressed.format);
    println!("  Stage 3 (Dictionary): {} centroids", compressed.centroid_ids.len());
    println!("  Total compression: {:.2}×", original_size as f32 / compressed_size as f32);
    println!("=====================================");
}
