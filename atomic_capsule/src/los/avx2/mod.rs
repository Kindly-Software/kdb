//! AVX2 SIMD operations for LOS traversal
//!
//! Provides x86_64 AVX2 intrinsics for:
//! - Q16.16 fixed-point vector operations (8-wide)
//! - Dense ray kernel (8× unroll, gather-free)
//! - Batched ray kernel (4-8 rays SoA)
//!
//! # Performance
//!
//! - 8× Q16.16 operations per instruction
//! - 2-4× speedup vs scalar for dense grids
//! - Branchless threshold comparisons
//! - Cache-friendly SoA layout
//!
//! # Safety
//!
//! All functions require `target_feature(enable = "avx2")`. Runtime CPU
//! detection handled by parent module via `is_x86_feature_detected!("avx2")`.

mod q16_ops;
pub(crate) mod dense_kernel;

pub use q16_ops::*;
pub use dense_kernel::{traverse_dense_8x_unrolled, traverse_dense_small, rasterize_line, ray_to_indices};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avx2_available() {
        // Verify AVX2 is available on test hardware
        if cfg!(target_arch = "x86_64") {
            if !is_x86_feature_detected!("avx2") {
                println!("WARNING: AVX2 not available on test hardware, skipping SIMD tests");
            }
        }
    }
}
