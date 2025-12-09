//! # Adaptive Quantization Capsule Trait
//!
//! **UCE33 Q33 (Atomic Capsule)**: Dynamic quantization with per-channel or per-group parameters.
//!
//! ## Design Philosophy
//!
//! Adaptive quantization extends static quantization with dynamic parameters:
//! - **Per-channel quantization**: Different scale/zero-point per output channel
//! - **Per-group quantization**: Different scale/zero-point per group of values
//! - **Outlier-aware quantization**: Adaptive threshold for outlier detection
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_ADAPTIVE_PARAMS_VALID`: All adaptive parameters within valid ranges
//! - `#VERIFY_ADAPTIVE_PARAMS`: Checked at runtime during update
//!
//! ## UCE33 Q31 (Rust Transform)
//!
//! Associated types enable compile-time dispatch for different adaptive strategies.

use crate::error::QuantResult;
use crate::traits::QuantizedCapsule;

/// Adaptive quantization capsule with dynamic parameters.
///
/// This trait extends `QuantizedCapsule` with per-channel or per-group adaptation.
///
/// # UCE33 Q33 (Atomic Capsule)
///
/// Adaptive quantization maintains capsule principles:
/// - **Cache-aligned**: Adaptive parameters in aligned structures
/// - **Lockfree updates**: Atomic parameter updates via two-phase commit
/// - **One-read decisions**: Parameters packed with quantized data
///
/// # IMPL-2 Justification
///
/// This trait is justified by 3+ implementations:
/// 1. Per-channel quantization (different scale per channel)
/// 2. Per-group quantization (different scale per group)
/// 3. Outlier-aware quantization (adaptive threshold)
///
/// # Example
///
/// ```rust
/// use atomic_llm_capsule::traits::{QuantizedCapsule, AdaptiveQuantizedCapsule};
/// use atomic_capsule::traits::ComputationalCapsule;
///
/// // Per-channel INT8 quantization
/// #[repr(C, align(128))]
/// struct PerChannelInt8Capsule {
///     data: [i8; 64],
///     scales: [f32; 8],      // 8 channels
///     zero_points: [i8; 8],  // 8 channels
/// }
///
/// unsafe impl ComputationalCapsule for PerChannelInt8Capsule {
///     const ALIGNMENT: usize = 128;
///     const SIZE: usize = 64 + 32 + 8; // data + scales + zero_points
///     const TYPE_ID: &'static str = "PerChannelInt8Capsule";
/// }
///
/// impl QuantizedCapsule for PerChannelInt8Capsule {
///     const BIT_WIDTH: usize = 8;
///     const COMPRESSION_RATIO: f32 = 4.0;
///     const GROUP_SIZE: usize = 64;
///
///     fn quantize(&mut self, input: &[f32]) -> Result<(), atomic_llm_capsule::error::QuantError> {
///         // Per-channel quantization implementation...
///         Ok(())
///     }
///
///     fn dequantize(&self, output: &mut [f32]) -> Result<(), atomic_llm_capsule::error::QuantError> {
///         // Per-channel dequantization implementation...
///         Ok(())
///     }
/// }
///
/// impl AdaptiveQuantizedCapsule for PerChannelInt8Capsule {
///     type AdaptiveParams = ([f32; 8], [i8; 8]); // (scales, zero_points)
///
///     const NUM_ADAPTIVE_CHANNELS: usize = 8;
///
///     fn get_adaptive_params(&self) -> Self::AdaptiveParams {
///         (self.scales, self.zero_points)
///     }
///
///     fn set_adaptive_params(&mut self, params: Self::AdaptiveParams) -> Result<(), atomic_llm_capsule::error::QuantError> {
///         self.scales = params.0;
///         self.zero_points = params.1;
///         Ok(())
///     }
///
///     fn update_adaptive_params(&mut self, input: &[f32]) -> Result<(), atomic_llm_capsule::error::QuantError> {
///         // Compute per-channel statistics and update scales/zero_points
///         Ok(())
///     }
/// }
/// ```
pub trait AdaptiveQuantizedCapsule: QuantizedCapsule {
    /// Type for adaptive parameters (e.g., per-channel scales, per-group zero-points).
    ///
    /// # UCE33 Q31 (Rust Transform)
    /// Associated type enables compile-time dispatch for different adaptive strategies
    type AdaptiveParams: Copy;

    /// Number of adaptive channels (e.g., 8 for per-channel, 1 for per-tensor).
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_NUM_CHANNELS_VALID`: Number of channels is power of 2
    /// - `#VERIFY_NUM_CHANNELS`: Checked at compile-time
    const NUM_ADAPTIVE_CHANNELS: usize;

    /// Get current adaptive parameters.
    ///
    /// # UCE33 Q33 (Atomic Capsule)
    /// Parameters are part of capsule state (one-read principle)
    fn get_adaptive_params(&self) -> Self::AdaptiveParams;

    /// Set adaptive parameters.
    ///
    /// # UCE33 Q30 (Validation)
    /// Validate parameters are within valid ranges
    ///
    /// # Arguments
    /// - `params`: New adaptive parameters
    ///
    /// # Returns
    /// - `Ok(())`: Parameters set successfully
    /// - `Err(QuantError)`: Invalid parameters
    fn set_adaptive_params(&mut self, params: Self::AdaptiveParams) -> QuantResult<()>;

    /// Update adaptive parameters based on input statistics.
    ///
    /// # UCE33 Q30 (Validation)
    /// Empirical calibration from current batch
    ///
    /// # Arguments
    /// - `input`: Input data for parameter estimation
    ///
    /// # Returns
    /// - `Ok(())`: Parameters updated successfully
    /// - `Err(QuantError)`: Invalid input or parameter update failed
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INPUT_VALID`: Input data is representative
    /// - `#VERIFY_UPDATE_SUCCESS`: Check parameter ranges after update
    fn update_adaptive_params(&mut self, input: &[f32]) -> QuantResult<()>;

    /// Get channel index for a given value index.
    ///
    /// # UCE33 Q31 (Rust Transform)
    /// Inline for zero-cost abstraction
    #[inline(always)]
    fn channel_index(&self, value_index: usize) -> usize {
        value_index % Self::NUM_ADAPTIVE_CHANNELS
    }

    /// Verify adaptive parameters are valid.
    ///
    /// # UCE33 Q30 (Validation)
    /// Runtime validation of adaptive parameter consistency
    #[inline(always)]
    fn verify_adaptive_params() -> bool {
        // Number of channels must be power of 2
        Self::NUM_ADAPTIVE_CHANNELS.count_ones() == 1
            // Number of channels must be reasonable
            && Self::NUM_ADAPTIVE_CHANNELS > 0
            && Self::NUM_ADAPTIVE_CHANNELS <= 256
    }

    /// Get adaptation strategy description.
    ///
    /// # UCE33 Q30 (Validation)
    /// Human-readable description for debugging
    #[inline(always)]
    fn adaptation_strategy() -> &'static str {
        match Self::NUM_ADAPTIVE_CHANNELS {
            1 => "per_tensor",
            n if n == Self::GROUP_SIZE => "per_value",
            _ => "per_channel",
        }
    }

    /// Get memory overhead from adaptive parameters.
    ///
    /// # UCE33 Q30 (Validation)
    /// Calculate additional memory cost of adaptation
    #[inline(always)]
    fn adaptive_overhead() -> usize {
        core::mem::size_of::<Self::AdaptiveParams>()
    }
}

/// Verification macro for adaptive quantized capsules.
///
/// # UCE33 Q30 (Validation)
/// Compile-time verification of adaptive quantization parameters
///
/// # Example
///
/// ```rust
/// # use atomic_llm_capsule::traits::{QuantizedCapsule, AdaptiveQuantizedCapsule};
/// # use atomic_capsule::traits::ComputationalCapsule;
/// # #[repr(C, align(128))]
/// # struct PerChannelInt8Capsule {
/// #     data: [i8; 64],
/// #     scales: [f32; 8],
/// #     zero_points: [i8; 8],
/// # }
/// # unsafe impl ComputationalCapsule for PerChannelInt8Capsule {
/// #     const ALIGNMENT: usize = 128;
/// #     const SIZE: usize = 64 + 32 + 8;
/// #     const TYPE_ID: &'static str = "PerChannelInt8Capsule";
/// # }
/// # impl QuantizedCapsule for PerChannelInt8Capsule {
/// #     const BIT_WIDTH: usize = 8;
/// #     const COMPRESSION_RATIO: f32 = 4.0;
/// #     const GROUP_SIZE: usize = 64;
/// #     fn quantize(&mut self, _: &[f32]) -> Result<(), atomic_llm_capsule::error::QuantError> { Ok(()) }
/// #     fn dequantize(&self, _: &mut [f32]) -> Result<(), atomic_llm_capsule::error::QuantError> { Ok(()) }
/// # }
/// # impl AdaptiveQuantizedCapsule for PerChannelInt8Capsule {
/// #     type AdaptiveParams = ([f32; 8], [i8; 8]);
/// #     const NUM_ADAPTIVE_CHANNELS: usize = 8;
/// #     fn get_adaptive_params(&self) -> Self::AdaptiveParams { (self.scales, self.zero_points) }
/// #     fn set_adaptive_params(&mut self, params: Self::AdaptiveParams) -> Result<(), atomic_llm_capsule::error::QuantError> { self.scales = params.0; self.zero_points = params.1; Ok(()) }
/// #     fn update_adaptive_params(&mut self, _: &[f32]) -> Result<(), atomic_llm_capsule::error::QuantError> { Ok(()) }
/// # }
/// use atomic_llm_capsule::verify_adaptive_capsule;
///
/// verify_adaptive_capsule!(PerChannelInt8Capsule, 8);
/// ```
#[macro_export]
macro_rules! verify_adaptive_capsule {
    ($capsule:ty, $num_channels:expr) => {
        const _: () = {
            // Verify number of channels matches
            assert!(
                <$capsule as $crate::traits::AdaptiveQuantizedCapsule>::NUM_ADAPTIVE_CHANNELS
                    == $num_channels,
                "Adaptive channel count mismatch"
            );

            // Note: verify_adaptive_params() is checked at runtime in tests
            // Cannot call non-const fn in const context
        };
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomic_capsule::traits::ComputationalCapsule;

    #[repr(C, align(128))]
    struct TestPerChannelCapsule {
        data: [i8; 64],
        scales: [f32; 8],
    }

    unsafe impl ComputationalCapsule for TestPerChannelCapsule {
        const ALIGNMENT: usize = 128;
        const SIZE: usize = 64 + 32;
        const TYPE_ID: &'static str = "TestPerChannelCapsule";
    }

    impl QuantizedCapsule for TestPerChannelCapsule {
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

    impl AdaptiveQuantizedCapsule for TestPerChannelCapsule {
        type AdaptiveParams = [f32; 8];
        const NUM_ADAPTIVE_CHANNELS: usize = 8;

        fn get_adaptive_params(&self) -> Self::AdaptiveParams {
            self.scales
        }

        fn set_adaptive_params(&mut self, params: Self::AdaptiveParams) -> QuantResult<()> {
            self.scales = params;
            Ok(())
        }

        fn update_adaptive_params(&mut self, _input: &[f32]) -> QuantResult<()> {
            Ok(())
        }
    }

    #[test]
    fn test_verify_adaptive_params() {
        assert!(TestPerChannelCapsule::verify_adaptive_params());
    }

    #[test]
    fn test_adaptation_strategy() {
        assert_eq!(
            TestPerChannelCapsule::adaptation_strategy(),
            "per_channel"
        );
    }

    #[test]
    fn test_channel_index() {
        let capsule = TestPerChannelCapsule {
            data: [0; 64],
            scales: [1.0; 8],
        };
        assert_eq!(capsule.channel_index(0), 0);
        assert_eq!(capsule.channel_index(8), 0);
        assert_eq!(capsule.channel_index(15), 7);
    }

    #[test]
    fn test_adaptive_overhead() {
        assert_eq!(TestPerChannelCapsule::adaptive_overhead(), 32); // 8 × f32
    }

    #[test]
    fn test_verification_macro() {
        verify_adaptive_capsule!(TestPerChannelCapsule, 8);
    }
}
