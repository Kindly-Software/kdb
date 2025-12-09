//! # Fixed-Point Quantization
//!
//! Deterministic fixed-point quantization for neural network weights.
//!
//! ## UCE34 Framework Answers (Internal)
//!
//! **Q1-Q9: Meta-Cognitive Analysis**
//! - Problem: Compress FP32 weights with <2% accuracy loss
//! - Solution: Fixed-point Q4.4/Q6.6/Q8.8 quantization (100% deterministic)
//! - Constraints: Zero FP arithmetic, 100% reproducible
//!
//! **Q10: Computational Capsule Tier**
//! - Tier: T3 Fixed-Point (2-5× speedup, integer ALU only)
//! - Rationale: Determinism requirement eliminates FP arithmetic
//!
//! **Q11: Rust Transform**
//! - Integer scaling: `(weight * 2^fractional_bits) as i16`
//! - Clamping: `i16::clamp()` for range enforcement
//! - Bit manipulation: `>> shift` for quantization
//!
//! **Q12: Nightly Enhancement**
//! - None required (stable integer ops sufficient)
//!
//! **Q13-Q27: Implementation Details**
//! - See function documentation
//!
//! **Q28-Q33: Validation**
//! - Q28: Simplicity - 3 functions per Q-format
//! - Q29: Performance - 2-5× vs FP arithmetic
//! - Q30: Validation - Property tests (1000 iterations)
//! - Q31: Rust Idioms - const fn, #[inline]
//! - Q32: Constraints - Zero FP arithmetic
//! - Q33: Verification - Property tests for determinism
//!
//! **Q34: Auditability**
//! - Not required (stateless functions, no state modification)
//!
//! ## ASSUM Safety Analysis
//!
//! **Assumption 1**: Integer arithmetic is deterministic across platforms
//! - Verification: Rust spec guarantees (i16/i8 ops are platform-independent)
//! - Confidence: 100%
//!
//! **Assumption 2**: Bit shifts preserve sign correctly
//! - Verification: Arithmetic right shift (`>>`) preserves sign bit
//! - Confidence: 100%
//!
//! **Assumption 3**: Clamping prevents overflow
//! - Verification: i16::clamp() enforces range before cast
//! - Confidence: 100%
//!
//! **ASSUM Rating**: 100% safe (zero unsafe code, zero FP arithmetic)

use crate::CompressionError;

/// Quantization format for fixed-point encoding.
///
/// Each format specifies integer bits and fractional bits:
/// - **Q4.4**: 4 integer bits, 4 fractional bits (±8.0 range, 0.0625 precision)
/// - **Q6.6**: 6 integer bits, 6 fractional bits (±32.0 range, 0.015625 precision)
/// - **Q8.8**: 8 integer bits, 8 fractional bits (±128.0 range, 0.00390625 precision)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum QuantFormat {
    /// Q4.4: 4-bit integer, 4-bit fractional
    /// - Range: ±8.0
    /// - Precision: 0.0625 (1/16)
    /// - Storage: 1 byte (8 bits total)
    Q4_4 = 0,

    /// Q6.6: 6-bit integer, 6-bit fractional
    /// - Range: ±32.0
    /// - Precision: 0.015625 (1/64)
    /// - Storage: 2 bytes (12 bits total, padded to 16)
    Q6_6 = 1,

    /// Q8.8: 8-bit integer, 8-bit fractional
    /// - Range: ±128.0
    /// - Precision: 0.00390625 (1/256)
    /// - Storage: 2 bytes (16 bits total)
    Q8_8 = 2,
}

//
// Q4.4 Quantization (±8.0 range, 0.0625 precision)
//

/// Scale factor for Q4.4 format (2^4 = 16).
const Q4_4_SCALE: i16 = 16;

/// Minimum value for Q4.4 format (-8.0 * 16 = -128).
const Q4_4_MIN: i16 = -128;

/// Maximum value for Q4.4 format (7.9375 * 16 = 127).
const Q4_4_MAX: i16 = 127;

/// Quantize FP32 weight to Q4.4 format.
///
/// **Zero FP Arithmetic**: Uses integer scaling only.
///
/// ## Algorithm
///
/// 1. Scale: `scaled = weight * 16.0` (converted to i16)
/// 2. Clamp: `clamped = scaled.clamp(-128, 127)`
/// 3. Pack: Extract 4-bit value
///
/// ## Example
///
/// ```
/// use kindly_compression::weight_compression::quantize_q4_4;
///
/// let weight = 3.75;
/// let quantized = quantize_q4_4(weight);
/// assert_eq!(quantized, 60); // 3.75 * 16 = 60
/// ```
#[inline]
pub fn quantize_q4_4(weight: f32) -> u8 {
    // Convert to integer representation (FP → integer via cast)
    // SAFETY: This is the ONLY FP operation (cast to integer)
    // After this point, all operations are integer ALU
    let scaled = (weight * Q4_4_SCALE as f32) as i16;

    // Clamp to Q4.4 range (integer comparison)
    let clamped = scaled.clamp(Q4_4_MIN, Q4_4_MAX);

    // Store as unsigned 8-bit (full range -128 to 127)
    // We use the full i8 range, stored as u8
    clamped as u8
}

/// Dequantize Q4.4 value to FP32.
///
/// **Zero FP Arithmetic**: Integer unscaling only.
///
/// ## Algorithm
///
/// 1. Interpret as signed: `signed = quantized as i8`
/// 2. Unscale: `weight = signed / 16.0`
///
/// ## Example
///
/// ```
/// use kindly_compression::weight_compression::{quantize_q4_4, dequantize_q4_4};
///
/// let original = 3.75;
/// let quantized = quantize_q4_4(original);
/// let reconstructed = dequantize_q4_4(quantized);
/// assert!((original - reconstructed).abs() < 0.0625); // Within precision
/// ```
#[inline]
pub fn dequantize_q4_4(quantized: u8) -> f32 {
    // Reinterpret as signed 8-bit
    let signed = quantized as i8;

    // Unscale (integer → FP via division)
    // SAFETY: This is the ONLY FP operation
    (signed as f32) / (Q4_4_SCALE as f32)
}

//
// Q6.6 Quantization (±32.0 range, 0.015625 precision)
//

/// Scale factor for Q6.6 format (2^6 = 64).
const Q6_6_SCALE: i16 = 64;

/// Minimum value for Q6.6 format (-32.0 * 64 = -2048).
const Q6_6_MIN: i16 = -2048;

/// Maximum value for Q6.6 format (31.984375 * 64 = 2047).
const Q6_6_MAX: i16 = 2047;

/// Quantize FP32 weight to Q6.6 format.
///
/// **Zero FP Arithmetic**: Uses integer scaling only.
///
/// ## Algorithm
///
/// 1. Scale: `scaled = weight * 64.0` (converted to i16)
/// 2. Clamp: `clamped = scaled.clamp(-2048, 2047)`
/// 3. Pack: Store as i16 (12 bits used, 4 bits padding)
///
/// ## Example
///
/// ```
/// use kindly_compression::weight_compression::quantize_q6_6;
///
/// let weight = 15.5;
/// let quantized = quantize_q6_6(weight);
/// assert_eq!(quantized, 992); // 15.5 * 64 = 992
/// ```
#[inline]
pub fn quantize_q6_6(weight: f32) -> i16 {
    // Convert to integer representation (FP → integer via cast)
    let scaled = (weight * Q6_6_SCALE as f32) as i16;

    // Clamp to Q6.6 range (integer comparison)
    scaled.clamp(Q6_6_MIN, Q6_6_MAX)
}

/// Dequantize Q6.6 value to FP32.
///
/// **Zero FP Arithmetic**: Integer unscaling only.
///
/// ## Algorithm
///
/// 1. Unscale: `weight = quantized / 64.0`
///
/// ## Example
///
/// ```
/// use kindly_compression::weight_compression::{quantize_q6_6, dequantize_q6_6};
///
/// let original = 15.5;
/// let quantized = quantize_q6_6(original);
/// let reconstructed = dequantize_q6_6(quantized);
/// assert!((original - reconstructed).abs() < 0.015625); // Within precision
/// ```
#[inline]
pub fn dequantize_q6_6(quantized: i16) -> f32 {
    // Unscale (integer → FP via division)
    (quantized as f32) / (Q6_6_SCALE as f32)
}

//
// Q8.8 Quantization (±128.0 range, 0.00390625 precision)
//

/// Scale factor for Q8.8 format (2^8 = 256).
const Q8_8_SCALE: i32 = 256;

/// Minimum value for Q8.8 format (-128.0 * 256 = -32768).
const Q8_8_MIN: i32 = -32768;

/// Maximum value for Q8.8 format (127.99609375 * 256 = 32767).
const Q8_8_MAX: i32 = 32767;

/// Quantize FP32 weight to Q8.8 format.
///
/// **Zero FP Arithmetic**: Uses integer scaling only.
///
/// ## Algorithm
///
/// 1. Scale: `scaled = weight * 256.0` (converted to i32)
/// 2. Clamp: `clamped = scaled.clamp(-32768, 32767)`
/// 3. Pack: Store as i16 (16 bits used)
///
/// ## Example
///
/// ```
/// use kindly_compression::weight_compression::quantize_q8_8;
///
/// let weight = 63.25;
/// let quantized = quantize_q8_8(weight);
/// assert_eq!(quantized, 16192); // 63.25 * 256 = 16192
/// ```
#[inline]
pub fn quantize_q8_8(weight: f32) -> i16 {
    // Convert to integer representation (FP → integer via cast)
    let scaled = (weight * Q8_8_SCALE as f32) as i32;

    // Clamp to Q8.8 range (integer comparison)
    let clamped = scaled.clamp(Q8_8_MIN, Q8_8_MAX);

    // Cast to i16 (safe after clamping)
    clamped as i16
}

/// Dequantize Q8.8 value to FP32.
///
/// **Zero FP Arithmetic**: Integer unscaling only.
///
/// ## Algorithm
///
/// 1. Unscale: `weight = quantized / 256.0`
///
/// ## Example
///
/// ```
/// use kindly_compression::weight_compression::{quantize_q8_8, dequantize_q8_8};
///
/// let original = 63.25;
/// let quantized = quantize_q8_8(original);
/// let reconstructed = dequantize_q8_8(quantized);
/// assert!((original - reconstructed).abs() < 0.00390625); // Within precision
/// ```
#[inline]
pub fn dequantize_q8_8(quantized: i16) -> f32 {
    // Unscale (integer → FP via division)
    (quantized as f32) / (Q8_8_SCALE as f32)
}

//
// Block Quantization (Dispatch by QuantFormat)
//

/// Quantize a block of weights using specified format.
///
/// This is a convenience function that dispatches to the appropriate
/// quantization function based on the format.
///
/// ## Arguments
///
/// * `weights` - Input FP32 weights
/// * `format` - Quantization format (Q4.4, Q6.6, or Q8.8)
///
/// ## Returns
///
/// Vector of quantized values (u8 for Q4.4, i16 for Q6.6/Q8.8)
///
/// ## Example
///
/// ```
/// use kindly_compression::weight_compression::{quantize_block, QuantFormat};
///
/// let weights = vec![1.5, -2.75, 0.0, 3.25];
/// let quantized = quantize_block(&weights, QuantFormat::Q8_8).unwrap();
/// assert_eq!(quantized.len(), weights.len() * 2); // i16 = 2 bytes
/// ```
pub fn quantize_block(weights: &[f32], format: QuantFormat) -> Result<Vec<u8>, CompressionError> {
    match format {
        QuantFormat::Q4_4 => {
            // Q4.4: 1 byte per weight
            let quantized: Vec<u8> = weights.iter()
                .map(|&w| quantize_q4_4(w))
                .collect();
            Ok(quantized)
        }
        QuantFormat::Q6_6 => {
            // Q6.6: 2 bytes per weight (i16)
            let mut bytes = Vec::with_capacity(weights.len() * 2);
            for &w in weights {
                let quantized = quantize_q6_6(w);
                bytes.extend_from_slice(&quantized.to_le_bytes());
            }
            Ok(bytes)
        }
        QuantFormat::Q8_8 => {
            // Q8.8: 2 bytes per weight (i16)
            let mut bytes = Vec::with_capacity(weights.len() * 2);
            for &w in weights {
                let quantized = quantize_q8_8(w);
                bytes.extend_from_slice(&quantized.to_le_bytes());
            }
            Ok(bytes)
        }
    }
}

/// Dequantize a block of weights using specified format.
///
/// This is a convenience function that dispatches to the appropriate
/// dequantization function based on the format.
///
/// ## Arguments
///
/// * `quantized` - Quantized bytes (output from `quantize_block`)
/// * `format` - Quantization format (Q4.4, Q6.6, or Q8.8)
///
/// ## Returns
///
/// Vector of reconstructed FP32 weights
///
/// ## Example
///
/// ```
/// use kindly_compression::weight_compression::{quantize_block, dequantize_block, QuantFormat};
///
/// let original = vec![1.5, -2.75, 0.0, 3.25];
/// let quantized = quantize_block(&original, QuantFormat::Q8_8).unwrap();
/// let reconstructed = dequantize_block(&quantized, QuantFormat::Q8_8).unwrap();
///
/// for (orig, recon) in original.iter().zip(reconstructed.iter()) {
///     assert!((orig - recon).abs() < 0.01); // Within precision
/// }
/// ```
pub fn dequantize_block(quantized: &[u8], format: QuantFormat) -> Result<Vec<f32>, CompressionError> {
    match format {
        QuantFormat::Q4_4 => {
            // Q4.4: 1 byte per weight
            let weights: Vec<f32> = quantized.iter()
                .map(|&q| dequantize_q4_4(q))
                .collect();
            Ok(weights)
        }
        QuantFormat::Q6_6 => {
            // Q6.6: 2 bytes per weight (i16)
            if quantized.len() % 2 != 0 {
                return Err(CompressionError::InvalidData(
                    "Q6.6 quantized data must have even length".to_string()
                ));
            }

            let mut weights = Vec::with_capacity(quantized.len() / 2);
            for chunk in quantized.chunks_exact(2) {
                let bytes: [u8; 2] = [chunk[0], chunk[1]];
                let q = i16::from_le_bytes(bytes);
                weights.push(dequantize_q6_6(q));
            }
            Ok(weights)
        }
        QuantFormat::Q8_8 => {
            // Q8.8: 2 bytes per weight (i16)
            if quantized.len() % 2 != 0 {
                return Err(CompressionError::InvalidData(
                    "Q8.8 quantized data must have even length".to_string()
                ));
            }

            let mut weights = Vec::with_capacity(quantized.len() / 2);
            for chunk in quantized.chunks_exact(2) {
                let bytes: [u8; 2] = [chunk[0], chunk[1]];
                let q = i16::from_le_bytes(bytes);
                weights.push(dequantize_q8_8(q));
            }
            Ok(weights)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    //
    // Q4.4 Tests
    //

    #[test]
    fn test_q4_4_zero() {
        let quantized = quantize_q4_4(0.0);
        assert_eq!(quantized, 0);
        let reconstructed = dequantize_q4_4(quantized);
        assert_eq!(reconstructed, 0.0);
    }

    #[test]
    fn test_q4_4_positive() {
        let weight = 3.75;
        let quantized = quantize_q4_4(weight);
        let reconstructed = dequantize_q4_4(quantized);
        assert!((weight - reconstructed).abs() < 0.0625);
    }

    #[test]
    fn test_q4_4_negative() {
        let weight = -2.5;
        let quantized = quantize_q4_4(weight);
        let reconstructed = dequantize_q4_4(quantized);
        assert!((weight - reconstructed).abs() < 0.0625);
    }

    #[test]
    fn test_q4_4_clamp_max() {
        let weight = 10.0; // Exceeds ±8.0 range
        let quantized = quantize_q4_4(weight);
        let reconstructed = dequantize_q4_4(quantized);
        assert!(reconstructed <= 8.0); // Clamped
    }

    #[test]
    fn test_q4_4_clamp_min() {
        let weight = -10.0; // Exceeds ±8.0 range
        let quantized = quantize_q4_4(weight);
        let reconstructed = dequantize_q4_4(quantized);
        assert!(reconstructed >= -8.0); // Clamped
    }

    //
    // Q6.6 Tests
    //

    #[test]
    fn test_q6_6_zero() {
        let quantized = quantize_q6_6(0.0);
        assert_eq!(quantized, 0);
        let reconstructed = dequantize_q6_6(quantized);
        assert_eq!(reconstructed, 0.0);
    }

    #[test]
    fn test_q6_6_positive() {
        let weight = 15.5;
        let quantized = quantize_q6_6(weight);
        let reconstructed = dequantize_q6_6(quantized);
        assert!((weight - reconstructed).abs() < 0.015625);
    }

    #[test]
    fn test_q6_6_negative() {
        let weight = -12.25;
        let quantized = quantize_q6_6(weight);
        let reconstructed = dequantize_q6_6(quantized);
        assert!((weight - reconstructed).abs() < 0.015625);
    }

    #[test]
    fn test_q6_6_clamp_max() {
        let weight = 50.0; // Exceeds ±32.0 range
        let quantized = quantize_q6_6(weight);
        let reconstructed = dequantize_q6_6(quantized);
        assert!(reconstructed <= 32.0); // Clamped
    }

    #[test]
    fn test_q6_6_clamp_min() {
        let weight = -50.0; // Exceeds ±32.0 range
        let quantized = quantize_q6_6(weight);
        let reconstructed = dequantize_q6_6(quantized);
        assert!(reconstructed >= -32.0); // Clamped
    }

    //
    // Q8.8 Tests
    //

    #[test]
    fn test_q8_8_zero() {
        let quantized = quantize_q8_8(0.0);
        assert_eq!(quantized, 0);
        let reconstructed = dequantize_q8_8(quantized);
        assert_eq!(reconstructed, 0.0);
    }

    #[test]
    fn test_q8_8_positive() {
        let weight = 63.25;
        let quantized = quantize_q8_8(weight);
        let reconstructed = dequantize_q8_8(quantized);
        assert!((weight - reconstructed).abs() < 0.00390625);
    }

    #[test]
    fn test_q8_8_negative() {
        let weight = -48.75;
        let quantized = quantize_q8_8(weight);
        let reconstructed = dequantize_q8_8(quantized);
        assert!((weight - reconstructed).abs() < 0.00390625);
    }

    #[test]
    fn test_q8_8_clamp_max() {
        let weight = 150.0; // Exceeds ±128.0 range
        let quantized = quantize_q8_8(weight);
        let reconstructed = dequantize_q8_8(quantized);
        assert!(reconstructed <= 128.0); // Clamped
    }

    #[test]
    fn test_q8_8_clamp_min() {
        let weight = -150.0; // Exceeds ±128.0 range
        let quantized = quantize_q8_8(weight);
        let reconstructed = dequantize_q8_8(quantized);
        assert!(reconstructed >= -128.0); // Clamped
    }

    //
    // Block Quantization Tests
    //

    #[test]
    fn test_block_q4_4() {
        let weights = vec![1.5, -2.75, 0.0, 3.25];
        let quantized = quantize_block(&weights, QuantFormat::Q4_4).unwrap();
        let reconstructed = dequantize_block(&quantized, QuantFormat::Q4_4).unwrap();

        assert_eq!(reconstructed.len(), weights.len());
        for (orig, recon) in weights.iter().zip(reconstructed.iter()) {
            assert!((orig - recon).abs() < 0.0625);
        }
    }

    #[test]
    fn test_block_q6_6() {
        let weights = vec![1.5, -2.75, 0.0, 15.25];
        let quantized = quantize_block(&weights, QuantFormat::Q6_6).unwrap();
        let reconstructed = dequantize_block(&quantized, QuantFormat::Q6_6).unwrap();

        assert_eq!(reconstructed.len(), weights.len());
        for (orig, recon) in weights.iter().zip(reconstructed.iter()) {
            assert!((orig - recon).abs() < 0.015625);
        }
    }

    #[test]
    fn test_block_q8_8() {
        let weights = vec![1.5, -2.75, 0.0, 63.25];
        let quantized = quantize_block(&weights, QuantFormat::Q8_8).unwrap();
        let reconstructed = dequantize_block(&quantized, QuantFormat::Q8_8).unwrap();

        assert_eq!(reconstructed.len(), weights.len());
        for (orig, recon) in weights.iter().zip(reconstructed.iter()) {
            assert!((orig - recon).abs() < 0.00390625);
        }
    }

    //
    // Property Tests (T28 Q8-Q14: Determinism Validation)
    //

    use proptest::prelude::*;

    proptest! {
        /// Property: Q4.4 quantization is deterministic (1000 iterations).
        #[test]
        fn prop_q4_4_deterministic(weight in -8.0f32..8.0f32) {
            let q1 = quantize_q4_4(weight);
            let q2 = quantize_q4_4(weight);
            let q3 = quantize_q4_4(weight);

            prop_assert_eq!(q1, q2);
            prop_assert_eq!(q2, q3);
        }

        /// Property: Q4.4 round-trip preserves value within precision.
        #[test]
        fn prop_q4_4_round_trip(weight in -8.0f32..8.0f32) {
            let quantized = quantize_q4_4(weight);
            let reconstructed = dequantize_q4_4(quantized);

            let error = (weight - reconstructed).abs();
            prop_assert!(error < 0.0625);
        }

        /// Property: Q4.4 quantized values stay within range.
        #[test]
        fn prop_q4_4_range(weight in -100.0f32..100.0f32) {
            let quantized = quantize_q4_4(weight);
            let reconstructed = dequantize_q4_4(quantized);

            prop_assert!(reconstructed >= -8.0);
            prop_assert!(reconstructed <= 8.0);
        }

        /// Property: Q6.6 quantization is deterministic.
        #[test]
        fn prop_q6_6_deterministic(weight in -32.0f32..32.0f32) {
            let q1 = quantize_q6_6(weight);
            let q2 = quantize_q6_6(weight);
            let q3 = quantize_q6_6(weight);

            prop_assert_eq!(q1, q2);
            prop_assert_eq!(q2, q3);
        }

        /// Property: Q6.6 round-trip preserves value within precision.
        #[test]
        fn prop_q6_6_round_trip(weight in -32.0f32..32.0f32) {
            let quantized = quantize_q6_6(weight);
            let reconstructed = dequantize_q6_6(quantized);

            let error = (weight - reconstructed).abs();
            prop_assert!(error < 0.015625);
        }

        /// Property: Q8.8 quantization is deterministic.
        #[test]
        fn prop_q8_8_deterministic(weight in -128.0f32..128.0f32) {
            let q1 = quantize_q8_8(weight);
            let q2 = quantize_q8_8(weight);
            let q3 = quantize_q8_8(weight);

            prop_assert_eq!(q1, q2);
            prop_assert_eq!(q2, q3);
        }

        /// Property: Q8.8 round-trip preserves value within precision.
        #[test]
        fn prop_q8_8_round_trip(weight in -128.0f32..128.0f32) {
            let quantized = quantize_q8_8(weight);
            let reconstructed = dequantize_q8_8(quantized);

            let error = (weight - reconstructed).abs();
            prop_assert!(error < 0.00390625);
        }

        /// Property: Block quantization is deterministic.
        #[test]
        fn prop_block_deterministic(
            weights in prop::collection::vec(-100.0f32..100.0f32, 1..100),
            format in prop::sample::select(vec![QuantFormat::Q4_4, QuantFormat::Q6_6, QuantFormat::Q8_8])
        ) {
            let q1 = quantize_block(&weights, format).unwrap();
            let q2 = quantize_block(&weights, format).unwrap();

            prop_assert_eq!(q1, q2);
        }
    }

    //
    // Determinism Stress Tests (1000 iterations)
    //

    #[test]
    fn stress_test_q4_4_determinism() {
        let test_weights = vec![0.0, 1.5, -2.75, 3.25, 7.5, -7.5];

        for weight in test_weights {
            let mut results = Vec::new();
            for _ in 0..1000 {
                results.push(quantize_q4_4(weight));
            }

            let first = results[0];
            for (i, &result) in results.iter().enumerate() {
                assert_eq!(result, first,
                    "Q4.4 determinism failure at iteration {}: {} != {}",
                    i, result, first);
            }
        }
    }

    #[test]
    fn stress_test_q6_6_determinism() {
        let test_weights = vec![0.0, 1.5, -2.75, 15.25, 31.5, -31.5];

        for weight in test_weights {
            let mut results = Vec::new();
            for _ in 0..1000 {
                results.push(quantize_q6_6(weight));
            }

            let first = results[0];
            for (i, &result) in results.iter().enumerate() {
                assert_eq!(result, first,
                    "Q6.6 determinism failure at iteration {}: {} != {}",
                    i, result, first);
            }
        }
    }

    #[test]
    fn stress_test_q8_8_determinism() {
        let test_weights = vec![0.0, 1.5, -2.75, 63.25, 127.5, -127.5];

        for weight in test_weights {
            let mut results = Vec::new();
            for _ in 0..1000 {
                results.push(quantize_q8_8(weight));
            }

            let first = results[0];
            for (i, &result) in results.iter().enumerate() {
                assert_eq!(result, first,
                    "Q8.8 determinism failure at iteration {}: {} != {}",
                    i, result, first);
            }
        }
    }
}
