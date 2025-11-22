//! # AVX2 Quantization Demo
//!
//! Demonstrates custom AVX2 intrinsics for Q8.8 quantization with 10-20× speedup target.
//!
//! ## Usage
//!
//! ```bash
//! cargo +nightly run --example avx2_quantization_demo --features inference-avx2-quant
//! ```
//!
//! ## Performance Comparison
//!
//! - Scalar: ~50ns per weight (baseline from quantization.rs)
//! - AVX2: ~2.5-5ns per weight (10-20× target speedup)
//! - Throughput: 16 weights per iteration (2× f32x8 → i16x16)

#![cfg(all(target_arch = "x86_64", feature = "portable_simd"))]
#![feature(portable_simd)]

use atomic_capsule::primitives::inference::{Avx2QuantizerQ88, QuantizationCapsule};
use std::time::Instant;

fn main() {
    println!("=== AVX2 Quantization Demo ===\n");

    // Check AVX2 availability
    #[cfg(target_arch = "x86_64")]
    if !is_x86_feature_detected!("avx2") {
        println!("❌ AVX2 not supported on this CPU");
        println!("This demo requires an x86_64 CPU with AVX2 support.");
        return;
    }

    println!("✅ AVX2 support detected\n");

    // Create test data (1024 weights)
    let weights: Vec<f32> = (0..1024).map(|i| ((i as f32 - 512.0) / 5.0)).collect();

    println!("Test data: {} weights", weights.len());
    println!(
        "Range: [{:.2}, {:.2}]\n",
        weights.iter().copied().fold(f32::INFINITY, f32::min),
        weights.iter().copied().fold(f32::NEG_INFINITY, f32::max)
    );

    // Create quantizers
    let avx2_quant = Avx2QuantizerQ88::from_range(-200.0, 200.0);
    let scalar_quant = QuantizationCapsule::from_range(-200.0, 200.0);

    println!("--- Scalar Quantization (Baseline) ---");
    let start = Instant::now();
    let scalar_quantized = scalar_quant.quantize(&weights);
    let scalar_time = start.elapsed();
    println!("Time: {:?}", scalar_time);
    println!(
        "Throughput: {:.2} weights/µs\n",
        weights.len() as f64 / scalar_time.as_micros() as f64
    );

    println!("--- AVX2 Quantization (Optimized) ---");
    let start = Instant::now();
    let avx2_quantized = avx2_quant.quantize_auto(&weights);
    let avx2_time = start.elapsed();
    println!("Time: {:?}", avx2_time);
    println!(
        "Throughput: {:.2} weights/µs",
        weights.len() as f64 / avx2_time.as_micros() as f64
    );

    // Calculate speedup
    let speedup = scalar_time.as_nanos() as f64 / avx2_time.as_nanos() as f64;
    println!("Speedup: {:.2}×\n", speedup);

    // Verify equivalence
    println!("--- Verification ---");
    let mut max_diff = 0i16;
    let mut mismatch_count = 0;
    for (i, (&avx2, &scalar)) in avx2_quantized
        .iter()
        .zip(scalar_quantized.iter())
        .enumerate()
    {
        let diff = (avx2 - scalar).abs();
        if diff > max_diff {
            max_diff = diff;
        }
        if diff > 256 {
            // Q8.8 tolerance: 1 unit = 1/256
            mismatch_count += 1;
            if mismatch_count <= 5 {
                println!(
                    "  Mismatch at {}: AVX2={}, Scalar={}, Diff={}",
                    i, avx2, scalar, diff
                );
            }
        }
    }

    if mismatch_count == 0 {
        println!(
            "✅ All results within Q8.8 tolerance (max diff: {})",
            max_diff
        );
    } else {
        println!(
            "⚠️  {} mismatches out of {} ({}%)",
            mismatch_count,
            weights.len(),
            100.0 * mismatch_count as f64 / weights.len() as f64
        );
    }

    // Test dequantization
    println!("\n--- Dequantization ---");
    let start = Instant::now();
    let scalar_dequant = scalar_quant.dequantize(&scalar_quantized);
    let scalar_dequant_time = start.elapsed();

    let start = Instant::now();
    let avx2_dequant = avx2_quant.dequantize_auto(&avx2_quantized);
    let avx2_dequant_time = start.elapsed();

    let dequant_speedup =
        scalar_dequant_time.as_nanos() as f64 / avx2_dequant_time.as_nanos() as f64;
    println!("Scalar: {:?}", scalar_dequant_time);
    println!("AVX2: {:?}", avx2_dequant_time);
    println!("Speedup: {:.2}×\n", dequant_speedup);

    // Verify round-trip error
    println!("--- Round-Trip Error ---");
    let mut max_error = 0.0f32;
    for (orig, deq) in weights.iter().zip(avx2_dequant.iter()) {
        let error = (orig - deq).abs();
        if error > max_error {
            max_error = error;
        }
    }
    println!("Max error: {:.6}", max_error);
    println!(
        "Relative error: {:.2}%\n",
        100.0 * max_error / weights.iter().copied().fold(f32::NEG_INFINITY, f32::max)
    );

    println!("=== Summary ===");
    println!("Quantization speedup: {:.2}×", speedup);
    println!("Dequantization speedup: {:.2}×", dequant_speedup);
    println!("Average speedup: {:.2}×", (speedup + dequant_speedup) / 2.0);

    if speedup >= 10.0 {
        println!("✅ Target 10-20× speedup ACHIEVED");
    } else {
        println!(
            "⚠️  Target 10-20× speedup not reached (got {:.2}×)",
            speedup
        );
        println!("Note: Speedup depends on batch size (amortize over 128+ elements)");
    }
}
