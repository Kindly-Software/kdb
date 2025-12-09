//! # Fixed-Point Quantization Demo
//!
//! Demonstrates deterministic Q4.4/Q6.6/Q8.8 fixed-point quantization
//! for neural network weight compression.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example quantization_demo
//! ```

use kindly_compression::weight_compression::{
    QuantFormat,
    quantize_q4_4, dequantize_q4_4,
    quantize_q6_6, dequantize_q6_6,
    quantize_q8_8, dequantize_q8_8,
    quantize_block, dequantize_block,
};

fn main() {
    println!("=== Fixed-Point Quantization Demo ===\n");

    // Q4.4 Example (±8.0 range, 0.0625 precision)
    println!("Q4.4 Quantization (±8.0 range, 0.0625 precision):");
    let weight_q4 = 3.75;
    let quantized_q4 = quantize_q4_4(weight_q4);
    let reconstructed_q4 = dequantize_q4_4(quantized_q4);
    println!("  Original: {}", weight_q4);
    println!("  Quantized: {} (u8)", quantized_q4);
    println!("  Reconstructed: {}", reconstructed_q4);
    println!("  Error: {}\n", (weight_q4 - reconstructed_q4).abs());

    // Q6.6 Example (±32.0 range, 0.015625 precision)
    println!("Q6.6 Quantization (±32.0 range, 0.015625 precision):");
    let weight_q6 = 15.5;
    let quantized_q6 = quantize_q6_6(weight_q6);
    let reconstructed_q6 = dequantize_q6_6(quantized_q6);
    println!("  Original: {}", weight_q6);
    println!("  Quantized: {} (i16)", quantized_q6);
    println!("  Reconstructed: {}", reconstructed_q6);
    println!("  Error: {}\n", (weight_q6 - reconstructed_q6).abs());

    // Q8.8 Example (±128.0 range, 0.00390625 precision)
    println!("Q8.8 Quantization (±128.0 range, 0.00390625 precision):");
    let weight_q8 = 63.25;
    let quantized_q8 = quantize_q8_8(weight_q8);
    let reconstructed_q8 = dequantize_q8_8(quantized_q8);
    println!("  Original: {}", weight_q8);
    println!("  Quantized: {} (i16)", quantized_q8);
    println!("  Reconstructed: {}", reconstructed_q8);
    println!("  Error: {}\n", (weight_q8 - reconstructed_q8).abs());

    // Block Quantization Example
    println!("Block Quantization (Q8.8 format):");
    let weights = vec![1.5, -2.75, 0.0, 63.25, -48.5, 127.0];
    println!("  Original weights: {:?}", weights);

    let quantized_block = quantize_block(&weights, QuantFormat::Q8_8).unwrap();
    println!("  Quantized size: {} bytes (vs {} bytes FP32)",
        quantized_block.len(), weights.len() * 4);
    println!("  Compression ratio: {:.2}×",
        (weights.len() * 4) as f32 / quantized_block.len() as f32);

    let reconstructed_block = dequantize_block(&quantized_block, QuantFormat::Q8_8).unwrap();
    println!("  Reconstructed: {:?}", reconstructed_block);

    let max_error = weights.iter()
        .zip(reconstructed_block.iter())
        .map(|(orig, recon)| (orig - recon).abs())
        .fold(0.0f32, f32::max);
    println!("  Max error: {}\n", max_error);

    // Determinism Demonstration
    println!("Determinism Validation (1000 iterations):");
    let test_weight = 42.5;
    let mut results = Vec::new();
    for _ in 0..1000 {
        results.push(quantize_q8_8(test_weight));
    }

    let first = results[0];
    let all_equal = results.iter().all(|&r| r == first);
    println!("  Test weight: {}", test_weight);
    println!("  All 1000 iterations equal: {}", all_equal);
    println!("  Result: {} (i16)\n", first);

    // Compression Comparison
    println!("Compression Comparison:");
    let test_weights: Vec<f32> = (0..1000).map(|i| (i as f32 - 500.0) / 10.0).collect();
    let original_size = test_weights.len() * 4; // FP32 = 4 bytes

    for format in [QuantFormat::Q4_4, QuantFormat::Q6_6, QuantFormat::Q8_8] {
        let quantized = quantize_block(&test_weights, format).unwrap();
        let compression = original_size as f32 / quantized.len() as f32;
        println!("  {:?}: {:.2}× compression ({} → {} bytes)",
            format, compression, original_size, quantized.len());
    }

    println!("\n=== Demo Complete ===");
}
