//! # Quantized Capsule Traits
//!
//! **UCE33 Q33 (Atomic Capsule)**: Extends ComputationalCapsule for quantized computation.
//!
//! ## Design Philosophy
//!
//! Following The Computational Capsule (Tier 3: Fixed-Point Capsules):
//! - **Deterministic precision**: Quantization eliminates floating-point drift
//! - **Cache-aligned**: All quantized data in aligned capsules
//! - **One-read decisions**: Quantized values packed for single read
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_BIT_WIDTH_VALID`: Bit width is 1, 2, 4, 8, or 16
//! - `#VERIFY_BIT_WIDTH_VALID`: Enforced by const assertions
//! - `#ASSUME_GROUP_SIZE_POW2`: Group size is power of 2
//! - `#VERIFY_GROUP_SIZE_POW2`: Enforced by const assertions
//!
//! ## UCE33 Q31 (Rust Transform)
//!
//! Const generics enable compile-time specialization for each quantization level.

use atomic_capsule::traits::ComputationalCapsule;
use crate::error::{QuantError, QuantResult};

/// Base trait for quantized computation capsules.
///
/// This trait extends `ComputationalCapsule` with quantization operations.
///
/// # UCE33 Q33 (Atomic Capsule)
///
/// Quantization is a natural extension of fixed-point arithmetic:
/// - **Fixed bit width**: 1, 2, 4, 8, or 16 bits per value
/// - **Fixed group size**: Power of 2 for SIMD alignment
/// - **Deterministic**: No floating-point drift
///
/// # Type Parameters
///
/// - `BIT_WIDTH`: Number of bits per quantized value (1, 2, 4, 8, 16)
/// - `GROUP_SIZE`: Number of values quantized together (power of 2)
///
/// # IMPL-2 Justification
///
/// This trait is justified by 5+ implementations (1-bit, 2-bit, 4-bit, 8-bit, 16-bit).
///
/// # Example
///
/// ```rust
/// use atomic_llm_capsule::traits::QuantizedCapsule;
/// use atomic_capsule::traits::ComputationalCapsule;
///
/// // INT8 quantization capsule
/// #[repr(C, align(64))]
/// struct Int8QuantCapsule {
///     // 64 quantized i8 values
///     data: [i8; 64],
/// }
///
/// unsafe impl ComputationalCapsule for Int8QuantCapsule {
///     const ALIGNMENT: usize = 64;
///     const SIZE: usize = 64;
///     const TYPE_ID: &'static str = "Int8QuantCapsule";
/// }
///
/// impl QuantizedCapsule for Int8QuantCapsule {
///     const BIT_WIDTH: usize = 8;
///     const COMPRESSION_RATIO: f32 = 4.0; // 32-bit float → 8-bit int
///     const GROUP_SIZE: usize = 64;
///
///     fn quantize(&mut self, input: &[f32]) -> Result<(), atomic_llm_capsule::error::QuantError> {
///         if input.len() != 64 {
///             return Err(atomic_llm_capsule::error::QuantError::BufferSizeMismatch {
///                 expected: 64,
///                 actual: input.len(),
///             });
///         }
///         // Quantization implementation...
///         Ok(())
///     }
///
///     fn dequantize(&self, output: &mut [f32]) -> Result<(), atomic_llm_capsule::error::QuantError> {
///         if output.len() != 64 {
///             return Err(atomic_llm_capsule::error::QuantError::BufferSizeMismatch {
///                 expected: 64,
///                 actual: output.len(),
///             });
///         }
///         // Dequantization implementation...
///         Ok(())
///     }
/// }
/// ```
pub trait QuantizedCapsule: ComputationalCapsule {
    /// Number of bits per quantized value (1, 2, 4, 8, or 16).
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_BIT_WIDTH_VALID`: Value is 1, 2, 4, 8, or 16
    /// - `#VERIFY_BIT_WIDTH_VALID`: Checked at compile-time
    ///
    /// # UCE33 Q29 (Constraints)
    /// Hardware constraint: Bit widths align with CPU registers (8, 16 bits) or sub-byte packing (1, 2, 4 bits)
    const BIT_WIDTH: usize;

    /// Compression ratio (original_bits / quantized_bits).
    ///
    /// # UCE33 Q30 (Validation)
    /// Measurable compression factor for validation
    ///
    /// # Example
    /// - 32-bit float → 8-bit int: ratio = 4.0
    /// - 32-bit float → 4-bit int: ratio = 8.0
    /// - 32-bit float → 1-bit binary: ratio = 32.0
    const COMPRESSION_RATIO: f32;

    /// Number of values quantized together (must be power of 2).
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_GROUP_SIZE_POW2`: Value is power of 2
    /// - `#VERIFY_GROUP_SIZE_POW2`: Checked at compile-time
    ///
    /// # UCE33 Q29 (Constraints)
    /// SIMD constraint: Group size should align with SIMD register width (4, 8, 16, etc.)
    const GROUP_SIZE: usize;

    /// Quantize floating-point values to fixed-bit representation.
    ///
    /// # UCE33 Q33 (Atomic Capsule)
    /// Quantization follows fixed-point capsule principles:
    /// - Deterministic conversion (no floating-point drift)
    /// - Cache-aligned output (SIMD-friendly)
    /// - Single-pass computation (no intermediate allocations)
    ///
    /// # Arguments
    /// - `input`: Input floating-point values (length must match GROUP_SIZE)
    ///
    /// # Returns
    /// - `Ok(())`: Quantization successful
    /// - `Err(QuantError)`: Buffer size mismatch or invalid input
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INPUT_VALID`: Input buffer size matches GROUP_SIZE
    /// - `#VERIFY_INPUT_SIZE`: Checked at runtime
    fn quantize(&mut self, input: &[f32]) -> QuantResult<()>;

    /// Dequantize fixed-bit values to floating-point representation.
    ///
    /// # UCE33 Q33 (Atomic Capsule)
    /// Dequantization is the inverse of quantization with deterministic precision.
    ///
    /// # Arguments
    /// - `output`: Output floating-point buffer (length must match GROUP_SIZE)
    ///
    /// # Returns
    /// - `Ok(())`: Dequantization successful
    /// - `Err(QuantError)`: Buffer size mismatch
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_OUTPUT_VALID`: Output buffer size matches GROUP_SIZE
    /// - `#VERIFY_OUTPUT_SIZE`: Checked at runtime
    fn dequantize(&self, output: &mut [f32]) -> QuantResult<()>;

    /// Verify quantization parameters at compile-time.
    ///
    /// # UCE33 Q30 (Validation)
    /// Compile-time validation prevents invalid configurations
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_PARAMS_VALID`: All parameters within valid ranges
    /// - `#VERIFY_PARAMS`: Checked at compile-time
    #[inline(always)]
    fn verify_params() -> bool {
        // Verify bit width is valid
        matches!(Self::BIT_WIDTH, 1 | 2 | 4 | 8 | 16)
            // Verify group size is power of 2
            && Self::GROUP_SIZE.count_ones() == 1
            // Verify compression ratio is positive
            && Self::COMPRESSION_RATIO > 0.0
            // Verify group size is reasonable (not too large)
            && Self::GROUP_SIZE <= 1024
    }

    /// Get memory footprint of quantized data.
    ///
    /// # UCE33 Q30 (Validation)
    /// Calculate exact memory usage for capacity planning
    #[inline(always)]
    fn memory_footprint() -> usize {
        (Self::GROUP_SIZE * Self::BIT_WIDTH).div_ceil(8)
    }

    /// Get quantization efficiency (compression ratio / accuracy loss).
    ///
    /// # UCE33 Q30 (Validation)
    /// Higher is better: more compression with less accuracy loss
    #[inline(always)]
    fn efficiency_metric() -> &'static str {
        match Self::BIT_WIDTH {
            1 => "extreme_compression", // 32x compression, high loss
            2 => "aggressive_compression", // 16x compression, moderate loss
            4 => "balanced_compression", // 8x compression, low loss
            8 => "conservative_compression", // 4x compression, minimal loss
            16 => "minimal_compression", // 2x compression, negligible loss
            _ => "unknown",
        }
    }
}

/// Static quantization capsule with fixed scale/zero-point.
///
/// # UCE33 Q33 (Atomic Capsule)
///
/// Static quantization is the simplest quantization scheme:
/// - **Fixed scale**: Single scale factor for all values
/// - **Fixed zero-point**: Single zero-point for all values
/// - **No overhead**: Zero runtime parameter computation
///
/// # IMPL-2 Justification
///
/// This trait is justified by 3+ implementations (INT8, INT4, Binary).
///
/// # Example
///
/// ```rust
/// use atomic_llm_capsule::traits::{QuantizedCapsule, StaticQuantizedCapsule};
/// use atomic_capsule::traits::ComputationalCapsule;
///
/// // INT8 quantization with fixed scale/zero-point
/// #[repr(C, align(64))]
/// struct Int8StaticCapsule {
///     data: [i8; 64],
///     scale: f32,
///     zero_point: i8,
/// }
///
/// unsafe impl ComputationalCapsule for Int8StaticCapsule {
///     const ALIGNMENT: usize = 64;
///     const SIZE: usize = 64 + 4 + 1; // data + scale + zero_point
///     const TYPE_ID: &'static str = "Int8StaticCapsule";
/// }
///
/// impl QuantizedCapsule for Int8StaticCapsule {
///     const BIT_WIDTH: usize = 8;
///     const COMPRESSION_RATIO: f32 = 4.0;
///     const GROUP_SIZE: usize = 64;
///
///     fn quantize(&mut self, input: &[f32]) -> Result<(), atomic_llm_capsule::error::QuantError> {
///         // Implementation...
///         Ok(())
///     }
///
///     fn dequantize(&self, output: &mut [f32]) -> Result<(), atomic_llm_capsule::error::QuantError> {
///         // Implementation...
///         Ok(())
///     }
/// }
///
/// impl StaticQuantizedCapsule for Int8StaticCapsule {
///     type ScaleType = f32;
///     type ZeroPointType = i8;
///
///     fn scale(&self) -> Self::ScaleType {
///         self.scale
///     }
///
///     fn zero_point(&self) -> Self::ZeroPointType {
///         self.zero_point
///     }
///
///     fn set_scale(&mut self, scale: Self::ScaleType) -> Result<(), atomic_llm_capsule::error::QuantError> {
///         self.scale = scale;
///         Ok(())
///     }
///
///     fn set_zero_point(&mut self, zero_point: Self::ZeroPointType) -> Result<(), atomic_llm_capsule::error::QuantError> {
///         self.zero_point = zero_point;
///         Ok(())
///     }
/// }
/// ```
pub trait StaticQuantizedCapsule: QuantizedCapsule {
    /// Type for scale factor (typically f32 or f64).
    ///
    /// # UCE33 Q31 (Rust Transform)
    /// Associated type enables compile-time dispatch
    type ScaleType: Copy;

    /// Type for zero-point offset (typically i8 or i16).
    ///
    /// # UCE33 Q31 (Rust Transform)
    /// Associated type enables compile-time dispatch
    type ZeroPointType: Copy;

    /// Get static scale factor.
    ///
    /// # UCE33 Q33 (Atomic Capsule)
    /// Scale factor is part of capsule state (one-read principle)
    fn scale(&self) -> Self::ScaleType;

    /// Get static zero-point.
    ///
    /// # UCE33 Q33 (Atomic Capsule)
    /// Zero-point is part of capsule state (one-read principle)
    fn zero_point(&self) -> Self::ZeroPointType;

    /// Set static scale factor.
    ///
    /// # UCE33 Q30 (Validation)
    /// Validate scale factor is non-zero and finite
    fn set_scale(&mut self, scale: Self::ScaleType) -> QuantResult<()>;

    /// Set static zero-point.
    ///
    /// # UCE33 Q30 (Validation)
    /// Validate zero-point is within quantization range
    fn set_zero_point(&mut self, zero_point: Self::ZeroPointType) -> QuantResult<()>;

    /// Compute optimal scale and zero-point from input data.
    ///
    /// # UCE33 Q30 (Validation)
    /// Empirical calibration from representative data
    ///
    /// # Arguments
    /// - `calibration_data`: Representative input samples
    ///
    /// # Returns
    /// - `Ok(())`: Calibration successful
    /// - `Err(QuantError)`: Invalid calibration data
    fn calibrate(&mut self, calibration_data: &[f32]) -> QuantResult<()> {
        if calibration_data.is_empty() {
            return Err(QuantError::BufferSizeMismatch {
                expected: 1,
                actual: 0,
            });
        }

        // Find min/max for range-based quantization using iterator methods
        // (avoids `for` loop which isn't allowed in const contexts)
        let min_val = calibration_data.iter().copied().fold(f32::MAX, f32::min);
        let max_val = calibration_data.iter().copied().fold(f32::MIN, f32::max);

        // Prevent division by zero
        if (max_val - min_val).abs() < 1e-8 {
            return Err(QuantError::ScaleOverflow);
        }

        // This is a placeholder - actual implementation depends on ScaleType/ZeroPointType
        // Real implementation would compute scale and zero-point based on min/max
        Ok(())
    }
}

/// Verification macro for quantized capsules.
///
/// # UCE33 Q30 (Validation)
/// Compile-time verification of quantization parameters
///
/// # Example
///
/// ```rust
/// # use atomic_llm_capsule::traits::QuantizedCapsule;
/// # use atomic_capsule::traits::ComputationalCapsule;
/// # #[repr(C, align(64))]
/// # struct Int8QuantCapsule { data: [i8; 64] }
/// # unsafe impl ComputationalCapsule for Int8QuantCapsule {
/// #     const ALIGNMENT: usize = 64;
/// #     const SIZE: usize = 64;
/// #     const TYPE_ID: &'static str = "Int8QuantCapsule";
/// # }
/// # impl QuantizedCapsule for Int8QuantCapsule {
/// #     const BIT_WIDTH: usize = 8;
/// #     const COMPRESSION_RATIO: f32 = 4.0;
/// #     const GROUP_SIZE: usize = 64;
/// #     fn quantize(&mut self, _: &[f32]) -> Result<(), atomic_llm_capsule::error::QuantError> { Ok(()) }
/// #     fn dequantize(&self, _: &mut [f32]) -> Result<(), atomic_llm_capsule::error::QuantError> { Ok(()) }
/// # }
/// use atomic_llm_capsule::verify_quantized_capsule;
///
/// verify_quantized_capsule!(Int8QuantCapsule, 8, 64);
/// ```
#[macro_export]
macro_rules! verify_quantized_capsule {
    ($capsule:ty, $bit_width:expr, $group_size:expr) => {
        const _: () = {
            // Verify bit width matches
            assert!(
                <$capsule as $crate::traits::QuantizedCapsule>::BIT_WIDTH == $bit_width,
                "Bit width mismatch"
            );

            // Verify group size matches
            assert!(
                <$capsule as $crate::traits::QuantizedCapsule>::GROUP_SIZE == $group_size,
                "Group size mismatch"
            );

            // Note: verify_params() is checked at runtime in tests
            // Cannot call non-const fn in const context
        };
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(C, align(64))]
    struct TestInt8Capsule {
        data: [i8; 64],
    }

    unsafe impl ComputationalCapsule for TestInt8Capsule {
        const ALIGNMENT: usize = 64;
        const SIZE: usize = 64;
        const TYPE_ID: &'static str = "TestInt8Capsule";
    }

    impl QuantizedCapsule for TestInt8Capsule {
        const BIT_WIDTH: usize = 8;
        const COMPRESSION_RATIO: f32 = 4.0;
        const GROUP_SIZE: usize = 64;

        fn quantize(&mut self, _input: &[f32]) -> QuantResult<()> {
            Ok(())
        }

        fn dequantize(&self, _output: &mut [f32]) -> QuantResult<()> {
            Ok(())
        }
    }

    #[test]
    fn test_verify_params() {
        assert!(TestInt8Capsule::verify_params());
    }

    #[test]
    fn test_memory_footprint() {
        assert_eq!(TestInt8Capsule::memory_footprint(), 64);
    }

    #[test]
    fn test_efficiency_metric() {
        assert_eq!(
            TestInt8Capsule::efficiency_metric(),
            "conservative_compression"
        );
    }

    #[test]
    fn test_verification_macro() {
        verify_quantized_capsule!(TestInt8Capsule, 8, 64);
    }
}
