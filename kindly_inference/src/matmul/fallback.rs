//! Fallback scalar matmul (no SIMD)
//!
//! **Purpose:** Fallback implementation when SIMD unavailable
//! **Performance:** Baseline (1× reference, no vectorization)

use super::Matmul;

/// Scalar matrix multiplication (fallback)
pub struct ScalarMatmul;

impl Matmul for ScalarMatmul {
    fn forward(&self, _input: &[f32], _weights: &[f32], _output: &mut [f32]) {
        // Placeholder: Will be implemented in Phase 1 (Month 1-2)
        unimplemented!("Scalar matmul fallback will be implemented in Phase 1")
    }
}
