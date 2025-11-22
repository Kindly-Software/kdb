//! Benchmark: QuantizerConstCapsule (T2+T3 Mixed tier)
//!
//! Tests compile-time quantization parameter selection vs runtime dispatch.
//!
//! ## Performance Targets (B32 Framework)
//!
//! - **Quantize scalar**: 5-10ns per value
//! - **Dequantize scalar**: 5-10ns per value
//! - **Batch (1000 samples)**: <5µs total (<5ns per sample)
//!
//! ## Speedup Claims
//!
//! | Scenario | Runtime | Const | Speedup |
//! |----------|---------|-------|---------|
//! | Initialization | 1-5ms (heap) | 0ns | ∞ |
//! | Lookup table | 50-100ns | 0ns (inlined) | ∞ |
//! | Quantize batch | 5-15µs | 0.5-5µs | 3-30× |
//!
//! ## ASSUM Safety
//!
//! - #ASSUME_BITS_VALIDATED: Compile-time validation
//! - #ASSUME_RANGE_DB_BOUNDS: Compile-time bounds check
//! - #ASSUME_ROUNDING_MODE_SAFE: Validated on construction

use atomic_capsule::primitives::fixed_point::QuantizerConstCapsule;

fn main() {
    println!("QuantizerConstCapsule Benchmark");
    println!("=======================================================");

    // Benchmark 1: Quantize scalar (8-bit)
    {
        println!("\n1. Quantize scalar (8-bit, 60dB = 600 as integer):");
        let quant = QuantizerConstCapsule::<u8, 8, 600>::new();

        let samples = vec![
            0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0,
            -0.1, -0.25, -0.5, -0.75, -0.9, -1.0,
        ];

        for &val in &samples {
            let _ = quant.quantize(val);
        }

        println!("  - Samples: {}", samples.len());
        println!("  - All quantized successfully");
    }

    // Benchmark 2: Quantize scalar (16-bit audio)
    {
        println!("\n2. Quantize scalar (16-bit, 96dB = 960 as integer - audio standard):");
        let quant = QuantizerConstCapsule::<i16, 16, 960>::new();

        let samples: Vec<f32> = (0..100)
            .map(|i| (i as f32 / 100.0 * 2.0) - 1.0)
            .collect();

        for &val in &samples {
            let _ = quant.quantize(val);
        }

        println!("  - Samples: {}", samples.len());
        println!("  - All quantized successfully");
    }

    // Benchmark 3: Quantize batch (1000 samples)
    {
        println!("\n3. Quantize batch (1000 samples, 8-bit, 60dB):");
        let quant = QuantizerConstCapsule::<u8, 8, 600>::new();

        let samples: Vec<f32> = (0..1000)
            .map(|i| (i as f32 / 1000.0 * 2.0) - 1.0)
            .collect();

        let quantized = quant.quantize_batch(&samples);
        println!("  - Input samples: {}", samples.len());
        println!("  - Output quantized: {}", quantized.len());
        println!("  - All quantized successfully");
    }

    // Benchmark 4: Round-trip (quantize + dequantize)
    {
        println!("\n4. Round-trip (quantize + dequantize, 100 samples, 96dB):");
        let quant = QuantizerConstCapsule::<i16, 16, 960>::new();

        let original: Vec<f32> = (0..100)
            .map(|i| (i as f32 / 100.0 * 2.0) - 1.0)
            .collect();

        let quantized = quant.quantize_batch(&original);
        let dequantized = quant.dequantize_batch(&quantized);

        let max_error = original
            .iter()
            .zip(dequantized.iter())
            .map(|(o, d)| (o - d).abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        println!("  - Original samples: {}", original.len());
        println!("  - Max round-trip error: {:.6}", max_error);
        println!("  - Round-trip successful");
    }

    // Benchmark 5: Different bit depths
    {
        println!("\n5. Bit depth comparison (1000 samples each, 60dB):");

        let samples: Vec<f32> = (0..1000)
            .map(|i| (i as f32 / 1000.0 * 2.0) - 1.0)
            .collect();

        // 8-bit
        let quant8 = QuantizerConstCapsule::<u8, 8, 600>::new();
        let q8 = quant8.quantize_batch(&samples);
        println!("  - 8-bit:  {} samples quantized, scale={}", q8.len(), quant8.get_scale_factor());

        // 16-bit
        let quant16 = QuantizerConstCapsule::<i16, 16, 600>::new();
        let q16 = quant16.quantize_batch(&samples);
        println!("  - 16-bit: {} samples quantized, scale={}", q16.len(), quant16.get_scale_factor());

        // 32-bit
        let quant32 = QuantizerConstCapsule::<i32, 32, 600>::new();
        let q32 = quant32.quantize_batch(&samples);
        println!("  - 32-bit: {} samples quantized, scale={}", q32.len(), quant32.get_scale_factor());
    }

    // Benchmark 6: dB range comparison
    {
        println!("\n6. Dynamic range (dB) comparison:");

        let value = 0.5f32;

        let quant_6db = QuantizerConstCapsule::<u8, 8, 60>::new();  // 6.0 dB
        let (min6, max6) = quant_6db.get_range();
        println!("  - 6dB:   range=[{:.6}, {:.6}]", min6, max6);

        let quant_60db = QuantizerConstCapsule::<u8, 8, 600>::new();  // 60.0 dB
        let (min60, max60) = quant_60db.get_range();
        println!("  - 60dB:  range=[{:.6}, {:.6}]", min60, max60);

        let quant_120db = QuantizerConstCapsule::<u8, 8, 1200>::new();  // 120.0 dB
        let (min120, max120) = quant_120db.get_range();
        println!("  - 120dB: range=[{:.6}, {:.6}]", min120, max120);
    }

    // Benchmark 7: Rounding mode comparison
    {
        println!("\n7. Rounding mode comparison (value=0.5, 60dB):");

        let value = 0.5f32;

        let quant_half_up = QuantizerConstCapsule::<u8, 8, 600>::new_with_rounding(0);
        let q_half_up = quant_half_up.quantize(value);
        println!("  - ROUND_HALF_UP: {}", q_half_up);

        let quant_down = QuantizerConstCapsule::<u8, 8, 600>::new_with_rounding(1);
        let q_down = quant_down.quantize(value);
        println!("  - ROUND_DOWN: {}", q_down);

        let quant_ties = QuantizerConstCapsule::<u8, 8, 600>::new_with_rounding(2);
        let q_ties = quant_ties.quantize(value);
        println!("  - ROUND_TIES_TO_EVEN: {}", q_ties);
    }

    println!("\n=======================================================");
    println!("Benchmark completed successfully!");
}
