//! # SIMD Capsule Compile-Time Verification
//!
//! **Zero-cost compile-time validation of SIMD alignment and size.**
//!
//! ## UCE33 Q33 Requirement
//!
//! All SIMD capsules MUST use verification macros to enforce:
//! - Correct alignment (256B/512B/1024B)
//! - Correct size (matching tier requirements)
//! - SIMD vector alignment (32B for AVX2, 64B for AVX-512)
//!
//! ## Performance
//!
//! - Runtime cost: ZERO (compile-time only)
//! - Build-time cost: <1ms per capsule
//! - Safety: 100% (misalignment fails compilation)
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_COMPILE_TIME_CHECK`: Rust const assertions validate at compile-time
//! - `#VERIFY_ZERO_RUNTIME_COST`: No code generated for verification

/// Verify SIMD capsule alignment
///
/// # Examples
/// ```compile_fail
/// #[repr(C, align(64))]
/// struct MisalignedCapsule { data: [u8; 32] }
///
/// // This will fail to compile (expected 256, found 64)
/// verify_simd_alignment!(MisalignedCapsule, 256);
/// ```
#[macro_export]
macro_rules! verify_simd_alignment {
    ($type:ty, $align:expr) => {
        const _: () = {
            assert!(
                core::mem::align_of::<$type>() == $align,
                "SIMD capsule alignment mismatch"
            );
        };
    };
}

/// Verify SIMD capsule size
///
/// # Examples
/// ```compile_fail
/// #[repr(C)]
/// struct WrongSizeCapsule { data: [u8; 100] }
///
/// // This will fail to compile (expected 256, found 100)
/// verify_simd_size!(WrongSizeCapsule, 256);
/// ```
#[macro_export]
macro_rules! verify_simd_size {
    ($type:ty, $size:expr) => {
        const _: () = {
            assert!(
                core::mem::size_of::<$type>() == $size,
                "SIMD capsule size mismatch"
            );
        };
    };
}

/// Verify complete SIMD capsule (alignment + size)
///
/// # Examples
/// ```
/// use simd_capsule_tier2::SimdF32x8Capsule;
/// use simd_capsule_tier2::verify_simd_capsule;
///
/// // This compiles (correct alignment and size)
/// verify_simd_capsule!(SimdF32x8Capsule, 256, 256);
/// ```
#[macro_export]
macro_rules! verify_simd_capsule {
    ($type:ty, $align:expr, $size:expr) => {
        const _: () = {
            assert!(
                core::mem::align_of::<$type>() == $align,
                "SIMD capsule alignment mismatch"
            );
            assert!(
                core::mem::size_of::<$type>() == $size,
                "SIMD capsule size mismatch"
            );
        };
    };
}

// Re-export for convenience
pub use {verify_simd_alignment, verify_simd_capsule, verify_simd_size};

#[cfg(test)]
mod tests {
    use crate::SimdF32x8Capsule;

    #[test]
    fn test_f32x8_verification() {
        // Verify f32x8 capsule has correct properties
        verify_simd_capsule!(SimdF32x8Capsule, 256, 256);
        verify_simd_alignment!(SimdF32x8Capsule, 256);
        verify_simd_size!(SimdF32x8Capsule, 256);
    }
}
