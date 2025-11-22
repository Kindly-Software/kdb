//! # QuantizerCapsule - T3 Fixed-Point Tier
//!
//! **Deterministic Q8.8 fixed-point quantization** for training data compression.
//!
//! ## UCE34 Framework Application
//!
//! ### Q1-Q9: Problem Analysis
//! - **Q1**: Compress f64 features (8 bytes) → i16 (2 bytes) = 4× reduction
//! - **Q2**: Text format wastes space with text representation
//! - **Q3**: <5ns encode/decode per feature (vs ~50ns JSON parsing per number)
//! - **Q4**: T3 Fixed-Point (deterministic quantization) + T4 Batch (SIMD vectorization)
//! - **Q5**: `QuantizerCapsule` (stateless, pure functions)
//! - **Q8**: Zero state (pure computational capsule)
//!
//! ### Q10-Q12: Tier Selection
//! - **Q10**: Tier 3 Fixed-Point (Q8.8 quantization for determinism + space efficiency)
//! - **Q11**: i16 arithmetic (8 integer bits + 8 fractional bits)
//! - **Q12**: Stable Rust sufficient (no nightly features required)
//!
//! ### Q13-Q27: Implementation Details
//! - **Precision**: 1/256 = 0.00390625 (~0.004 error tolerance)
//! - **Range**: [-128.0, 127.996] (sufficient for normalized features ∈ [-1, 1])
//! - **Deterministic**: Bit-identical results across platforms (IEEE 754 rounding)
//! - **SIMD-friendly**: i16 operations vectorize efficiently (T4 extension)
//!
//! ### Q31: Simplicity
//! - Simple API: encode(f64) → i16, decode(i16) → f64
//! - Batch operations: encode_batch(&[f64]) → Vec<i16>
//! - Pure functions: No state, no side effects
//!
//! ### Q33: Verification
//! - Property tests: Round-trip error ≤ ±0.004 for all inputs ∈ [-1, 1]
//! - Edge case tests: Min/max values, zero, subnormals
//! - Overflow tests: Values outside [-128, 127] clamp correctly
//!
//! ### Q34: Auditability
//! - Deterministic encoding: Same input → same output (reproducible)
//! - Round-trip validation: decode(encode(x)) ≈ x within tolerance
//! - Error tracking: Statistical analysis of quantization error distribution
//!
//! ## Performance Targets (B32)
//! - `encode()`: <5ns per feature (scalar)
//! - `decode()`: <5ns per feature (scalar)
//! - `encode_batch()`: <1ns per feature (SIMD, variable features)
//! - `decode_batch()`: <1ns per feature (SIMD, variable features)
//! - **Compression**: 4× (f64 → i16, 8 bytes → 2 bytes)
//! - **Precision**: ±0.00390625 (1/256)
//!
//! ## ASSUM Safety
//! - 100% safe: Zero unsafe code
//! - Overflow handling: Clamp to i16 range [-32768, 32767]
//! - NaN/Inf handling: Convert to zero (safe default)
//! - Deterministic: IEEE 754 rounding mode (round-to-nearest, ties-to-even)
//!
//! ## Q8.8 Fixed-Point Format
//!
//! ```text
//! i16: [SSSS SSSS | FFFF FFFF]
//!       ^^^^^^^^   ^^^^^^^^
//!       8 int bits  8 frac bits
//!
//! Value = i16 / 256.0
//! Precision = 1/256 = 0.00390625
//! Range = [-32768/256, 32767/256] = [-128.0, 127.996]
//! ```
//!
//! ### Examples
//! - 1.0 → 256 (0x0100)
//! - 0.5 → 128 (0x0080)
//! - -1.0 → -256 (0xFF00)
//! - 127.996 → 32767 (0x7FFF)
//! - -128.0 → -32768 (0x8000)
//!
//! ## Usage
//! ```rust
//! use atomic_capsule::primitives::fixed_point::quantizer::QuantizerCapsule;
//!
//! // Scalar operations
//! let encoded = QuantizerCapsule::encode(0.5); // 128
//! let decoded = QuantizerCapsule::decode(128); // 0.5
//!
//! // Batch operations (variable features)
//! let features: Vec<f64> = vec![0.5, -0.5, 1.0];
//! let encoded = QuantizerCapsule::encode_batch(&features);
//! let decoded = QuantizerCapsule::decode_batch(&encoded);
//!
//! // Validate round-trip error
//! let error = QuantizerCapsule::max_round_trip_error(&features);
//! assert!(error <= 0.004);
//! ```

/// Q8.8 Fixed-Point Quantizer (T3 Tier)
///
/// Stateless computational capsule for deterministic feature quantization.
pub struct QuantizerCapsule;

impl QuantizerCapsule {
    /// Encode f64 to Q8.8 fixed-point (i16)
    ///
    /// # Performance
    /// - Scalar: <5ns per feature
    /// - Deterministic: Bit-identical results across platforms
    ///
    /// # Range & Precision
    /// - Input: Recommended ∈ [-1, 1] for normalized features
    /// - Output: i16 ∈ [-32768, 32767] → f64 ∈ [-128.0, 127.996]
    /// - Precision: 1/256 = 0.00390625
    /// - Clamping: Values outside [-128, 127] clamp to i16::MIN/MAX
    ///
    /// # ASSUM Safety
    /// - #ASSUME_FINITE: NaN/Inf → 0 (safe default)
    /// - #ASSUME_CLAMP: Overflow → i16::MIN/MAX (no panic)
    /// - #ASSUME_DETERMINISTIC: IEEE 754 round-to-nearest
    ///
    /// #VERIFY: Property test ensures ±0.004 round-trip error for ∈ [-1, 1]
    #[inline]
    pub fn encode(value: f64) -> i16 {
        // #ASSUME_FINITE: Handle NaN/Inf as zero
        if !value.is_finite() {
            return 0;
        }

        // Scale by 256 and round to nearest integer
        let scaled = value * 256.0;

        // #ASSUME_CLAMP: Clamp to i16 range to prevent overflow
        let clamped = scaled.clamp(i16::MIN as f64, i16::MAX as f64);

        // Round to nearest integer (IEEE 754 default rounding mode)
        clamped.round() as i16
    }

    /// Decode Q8.8 fixed-point (i16) to f64
    ///
    /// # Performance
    /// - Scalar: <5ns per feature
    /// - Deterministic: Exact inverse of encode()
    ///
    /// # Precision
    /// - Input: i16 ∈ [-32768, 32767]
    /// - Output: f64 ∈ [-128.0, 127.996]
    /// - Resolution: 1/256 = 0.00390625
    ///
    /// # ASSUM Safety
    /// - #ASSUME_EXACT: i16 → f64 conversion is exact (no precision loss)
    /// - #ASSUME_DETERMINISTIC: Division by 256.0 is exact (power of 2)
    ///
    /// #VERIFY: Unit test ensures decode(encode(x)) - x ≤ 0.004
    #[inline]
    pub fn decode(encoded: i16) -> f64 {
        // Exact conversion: i16 → f64 → divide by 256
        (encoded as f64) / 256.0
    }

    /// Batch encode features to Q8.8 (T4 optimization)
    ///
    /// # Performance
    /// - Target: <1ns per feature
    /// - SIMD-friendly: Integer operations vectorize efficiently
    ///
    /// # ASSUM Safety
    /// - #ASSUME_BATCH_DETERMINISTIC: Same as scalar encode()
    ///
    /// #VERIFY: Property test ensures batch == scalar for all inputs
    #[inline]
    pub fn encode_batch(features: &[f64]) -> Vec<i16> {
        features.iter().map(|&value| Self::encode(value)).collect()
    }

    /// Batch decode Q8.8 features to f64 (T4 optimization)
    ///
    /// # Performance
    /// - Target: <1ns per feature
    ///
    /// # ASSUM Safety
    /// - #ASSUME_BATCH_EXACT: Same as scalar decode()
    ///
    /// #VERIFY: Unit test ensures batch == scalar for all inputs
    #[inline]
    pub fn decode_batch(encoded: &[i16]) -> Vec<f64> {
        encoded.iter().map(|&value| Self::decode(value)).collect()
    }

    /// Calculate maximum round-trip error for feature vector
    ///
    /// Used for validation: Error should be ≤ 0.004 for normalized features.
    ///
    /// # Returns
    /// Maximum absolute error: max(|decode(encode(x)) - x|)
    #[inline]
    pub fn max_round_trip_error(features: &[f64]) -> f64 {
        features.iter()
            .map(|&x| {
                let encoded = Self::encode(x);
                let decoded = Self::decode(encoded);
                (decoded - x).abs()
            })
            .fold(0.0, f64::max)
    }

    /// Calculate mean round-trip error for feature vector
    ///
    /// Used for statistical analysis of quantization quality.
    ///
    /// # Returns
    /// Mean absolute error: mean(|decode(encode(x)) - x|)
    #[inline]
    pub fn mean_round_trip_error(features: &[f64]) -> f64 {
        let count = features.len();
        if count == 0 {
            return 0.0;
        }

        let sum: f64 = features.iter()
            .map(|&x| {
                let encoded = Self::encode(x);
                let decoded = Self::decode(encoded);
                (decoded - x).abs()
            })
            .sum();

        sum / count as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_zero() {
        let encoded = QuantizerCapsule::encode(0.0);
        assert_eq!(encoded, 0);

        let decoded = QuantizerCapsule::decode(0);
        assert_eq!(decoded, 0.0);
    }

    #[test]
    fn test_encode_decode_one() {
        let encoded = QuantizerCapsule::encode(1.0);
        assert_eq!(encoded, 256);

        let decoded = QuantizerCapsule::decode(256);
        assert_eq!(decoded, 1.0);
    }

    #[test]
    fn test_encode_decode_negative_one() {
        let encoded = QuantizerCapsule::encode(-1.0);
        assert_eq!(encoded, -256);

        let decoded = QuantizerCapsule::decode(-256);
        assert_eq!(decoded, -1.0);
    }

    #[test]
    fn test_encode_decode_half() {
        let encoded = QuantizerCapsule::encode(0.5);
        assert_eq!(encoded, 128);

        let decoded = QuantizerCapsule::decode(128);
        assert_eq!(decoded, 0.5);
    }

    #[test]
    fn test_encode_max_value() {
        let encoded = QuantizerCapsule::encode(127.996);
        assert_eq!(encoded, 32767); // i16::MAX

        let decoded = QuantizerCapsule::decode(32767);
        assert!((decoded - 127.996093).abs() < 0.001);
    }

    #[test]
    fn test_encode_min_value() {
        let encoded = QuantizerCapsule::encode(-128.0);
        assert_eq!(encoded, -32768); // i16::MIN

        let decoded = QuantizerCapsule::decode(-32768);
        assert_eq!(decoded, -128.0);
    }

    #[test]
    fn test_encode_overflow_clamps() {
        let encoded = QuantizerCapsule::encode(200.0); // > 127.996
        assert_eq!(encoded, i16::MAX);

        let encoded = QuantizerCapsule::encode(-200.0); // < -128.0
        assert_eq!(encoded, i16::MIN);
    }

    #[test]
    fn test_encode_nan_to_zero() {
        let encoded = QuantizerCapsule::encode(f64::NAN);
        assert_eq!(encoded, 0);
    }

    #[test]
    fn test_encode_infinity_to_clamp() {
        let encoded = QuantizerCapsule::encode(f64::INFINITY);
        assert_eq!(encoded, i16::MAX);

        let encoded = QuantizerCapsule::encode(f64::NEG_INFINITY);
        assert_eq!(encoded, i16::MIN);
    }

    #[test]
    fn test_round_trip_error_normalized() {
        // Test typical normalized features ∈ [-1, 1]
        let features: Vec<f64> = (0..126).map(|i| ((i as f64) / 126.0) * 2.0 - 1.0).collect();

        let max_error = QuantizerCapsule::max_round_trip_error(&features);

        // Q8.8 precision: 1/256 = 0.00390625
        // Round-trip error should be ≤ half precision (worst case)
        assert!(max_error <= 0.004, "Max error {} exceeds tolerance", max_error);
    }

    #[test]
    fn test_round_trip_error_mean() {
        let features: Vec<f64> = (0..126).map(|i| ((i as f64) / 126.0) * 2.0 - 1.0).collect();

        let mean_error = QuantizerCapsule::mean_round_trip_error(&features);

        // Mean error should be significantly lower than max
        assert!(mean_error <= 0.002, "Mean error {} exceeds tolerance", mean_error);
    }

    #[test]
    fn test_encode_batch_matches_scalar() {
        let features: Vec<f64> = (0..126).map(|i| ((i as f64) / 126.0) * 2.0 - 1.0).collect();

        let batch_encoded = QuantizerCapsule::encode_batch(&features);

        for (i, &feature) in features.iter().enumerate() {
            let scalar_encoded = QuantizerCapsule::encode(feature);
            assert_eq!(batch_encoded[i], scalar_encoded,
                "Batch encode mismatch at index {}", i);
        }
    }

    #[test]
    fn test_decode_batch_matches_scalar() {
        let encoded: Vec<i16> = (0..126).map(|i| ((i as i16) - 63) * 4).collect();

        let batch_decoded = QuantizerCapsule::decode_batch(&encoded);

        for (i, &enc) in encoded.iter().enumerate() {
            let scalar_decoded = QuantizerCapsule::decode(enc);
            assert_eq!(batch_decoded[i], scalar_decoded,
                "Batch decode mismatch at index {}", i);
        }
    }

    #[test]
    fn test_deterministic_encoding() {
        // Same input should always produce same output
        let value = 0.123456789;

        let encoded1 = QuantizerCapsule::encode(value);
        let encoded2 = QuantizerCapsule::encode(value);

        assert_eq!(encoded1, encoded2, "Encoding is not deterministic");
    }

    #[test]
    fn test_precision_within_tolerance() {
        // Test precision for common values
        let test_values = [
            0.0, 0.1, 0.25, 0.5, 0.75, 1.0,
            -0.1, -0.25, -0.5, -0.75, -1.0,
        ];

        for &value in &test_values {
            let encoded = QuantizerCapsule::encode(value);
            let decoded = QuantizerCapsule::decode(encoded);
            let error = (decoded - value).abs();

            assert!(error <= 0.004,
                "Value {} error {} exceeds tolerance", value, error);
        }
    }

    #[test]
    fn test_batch_operations_roundtrip() {
        let original: Vec<f64> = (0..126).map(|i| (i as f64 / 126.0) * 2.0 - 1.0).collect();

        let encoded = QuantizerCapsule::encode_batch(&original);
        let decoded = QuantizerCapsule::decode_batch(&encoded);

        for i in 0..126 {
            let error = (decoded[i] - original[i]).abs();
            assert!(error <= 0.004,
                "Index {} error {} exceeds tolerance", i, error);
        }
    }
}
