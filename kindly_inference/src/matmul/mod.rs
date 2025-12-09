//! SIMD CPU Matmul (T2 Tier)
//!
//! **Architecture:** Vectorized matrix multiplication using SIMD instructions
//! **Performance Target:** 2-3× faster than llama.cpp
//! **Framework:** UCE34 Q10 (T2 SIMD tier)
//!
//! ## Performance
//!
//! - f32x8: 8-wide SIMD lanes (AVX2, NEON)
//! - f64x8: 8-wide SIMD lanes (AVX512, SVE)
//! - Cache tiling: 64B cache line optimization
//! - Target: 15-30 tokens/sec (Llama 13B, CPU only)
//!
//! ## Usage
//!
//! ```ignore
//! use kindly_inference::matmul::SimdMatmul;
//!
//! let matmul = SimdMatmul::<1024, 512>::new();
//! let output = matmul.forward_f32x8(&input, &weights);
//! ```

#[cfg(feature = "nightly")]
pub mod simd_kernel;

#[cfg(not(feature = "nightly"))]
pub mod fallback;

/// Matrix multiplication trait
pub trait Matmul {
    /// Perform matrix multiplication (N×M) × (M×K) = (N×K)
    fn forward(&self, input: &[f32], weights: &[f32], output: &mut [f32]);
}

/// SIMD-optimized matmul capsule
#[cfg(feature = "nightly")]
pub use simd_kernel::SimdMatmul;

/// Fallback scalar matmul (stable Rust)
#[cfg(not(feature = "nightly"))]
pub use fallback::ScalarMatmul as SimdMatmul;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matmul_trait() {
        // Placeholder test
        // Will be implemented in Phase 1 (Month 1-2)
    }
}
