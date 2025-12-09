//! # RMS Normalization Capsule (T2 SIMD)
//!
//! **Production-ready RMS normalization for Qwen3 and other LLMs using portable_simd.**
//!
//! ## Algorithm
//!
//! RMSNorm computes: `output = x * rsqrt(mean(x^2) + eps) * weight`
//!
//! Unlike LayerNorm, RMSNorm:
//! - Does NOT subtract mean (no centering)
//! - Does NOT have a bias term
//! - Uses only scale (weight) parameter
//! - Is 15-20% faster due to reduced operations
//!
//! ## UCE34 Analysis
//!
//! - **Q10 (Tier Selection)**: T2 (SIMD) for vectorized normalization
//! - **Q11 (Rust Transform)**: portable_simd f32x8 for 8-wide parallelism
//! - **Q12 (Nightly)**: portable_simd (nightly feature required)
//! - **Q31 (Simplicity)**: Single forward() API hides SIMD complexity
//! - **Q33 (Validation)**: Compile-time alignment verification via verify_alignment_only!
//!
//! ## Performance Targets (B32 Validation Required)
//!
//! - Single forward: <100ns for 4096 hidden_size (Qwen3 8B)
//! - SIMD: 8-wide parallel sum-of-squares and multiplication
//! - Zero allocation: in-place variant available (forward_inplace)
//! - Memory: O(hidden_size) for weights only
//!
//! ## Qwen3 Model Specifications
//!
//! | Model | hidden_size | eps |
//! |-------|-------------|-----|
//! | Qwen3 0.6B | 1024 | 1e-6 |
//! | Qwen3 1.7B | 2048 | 1e-6 |
//! | Qwen3 4B | 2560 | 1e-6 |
//! | Qwen3 8B | 4096 | 1e-6 |
//! | Qwen3 14B | 5120 | 1e-6 |
//! | Qwen3 30B | 6144 | 1e-6 |
//! | Qwen3 72B | 8192 | 1e-6 |
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use atomic_capsule::inference::RMSNormCapsule;
//!
//! // Create RMSNorm for Qwen3 8B (4096 hidden dim)
//! let rmsnorm = RMSNormCapsule::new(4096);
//!
//! // Or from learned weights
//! let weights = vec![1.0f32; 4096]; // Load from model file
//! let rmsnorm = RMSNormCapsule::from_weights(weights, 1e-6);
//!
//! // Forward pass
//! let input = vec![0.5f32; 4096];
//! let output = rmsnorm.forward(&input);
//!
//! // In-place forward (zero allocation)
//! let mut buffer = vec![0.5f32; 4096];
//! rmsnorm.forward_inplace(&mut buffer);
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_ALIGNMENT`: 128B alignment for SIMD f32x8 cache line optimization
//! - `#VERIFY_ALIGNMENT`: Compile-time verification via verify_alignment_only! macro
//! - `#ASSUME_HIDDEN_SIZE`: Must be multiple of 8 for SIMD vectorization
//! - `#ASSUME_FINITE`: Input values must be finite (no NaN/Inf)
//!
//! ## Architecture Patterns
//!
//! ### T2 SIMD Optimization
//!
//! - **Sum of Squares**: f32x8 parallel multiply-accumulate (8 elements per cycle)
//! - **Normalization**: f32x8 parallel rsqrt approximation + scaling
//! - **Weight Application**: f32x8 parallel multiplication with learned weights
//!
//! ## Testing
//!
//! ```bash
//! # Run RMSNorm tests
//! cargo test --lib --features "inference-rmsnorm" rmsnorm::
//!
//! # Benchmark (B32 validation)
//! cargo bench --features "inference-rmsnorm" rmsnorm_
//! ```

#![cfg(feature = "portable_simd")]

use std::simd::f32x8;
use std::simd::num::SimdFloat;

/// RMS Normalization Capsule (T2 SIMD)
///
/// High-performance RMS normalization using f32x8 SIMD operations.
/// Optimized for LLM inference (Qwen3, LLaMA, Mistral, etc.).
///
/// # Cache Layout
///
/// - 128B aligned for SIMD cache line optimization
/// - Weights stored as f32x8 vectors for direct SIMD operations
/// - Padding ensures no false sharing in multi-threaded context
///
/// # ASSUM Framework
///
/// - `#ASSUME_ALIGNMENT`: 128B alignment for SIMD f32x8
/// - `#VERIFY_ALIGNMENT`: Compile-time verification macro required
/// - `#ASSUME_HIDDEN_SIZE`: hidden_size must be multiple of 8 for SIMD
/// - `#ASSUME_POSITIVE_EPS`: epsilon must be positive (typically 1e-6)
#[repr(C, align(128))]
pub struct RMSNormCapsule {
    /// Learned weight parameters stored as SIMD vectors
    /// Layout: [vec0, vec1, ..., vecN] where each vec is f32x8
    weights: Vec<f32x8>,

    /// Hidden dimension (must be multiple of 8 for SIMD)
    hidden_size: usize,

    /// Epsilon for numerical stability (default: 1e-6 for Qwen3)
    eps: f32,

    /// Number of SIMD vectors (hidden_size / 8)
    num_vectors: usize,

    /// Padding to complete 128B alignment
    /// Total struct: 8 (weights ptr) + 8 (len) + 8 (cap) + 8 (hidden_size)
    ///             + 4 (eps) + 8 (num_vectors) = 44 bytes
    /// Padding: 128 - 44 = 84 bytes (round to nice number)
    _padding: [u8; 84],
}

// Compile-time verification - 128B alignment MANDATORY for T2 SIMD tier
crate::verify_alignment_only!(RMSNormCapsule, 128);

impl RMSNormCapsule {
    /// Default epsilon for Qwen3 models
    pub const QWEN3_EPS: f32 = 1e-6;

    /// Create new RMSNorm capsule with default weights (all 1.0)
    ///
    /// # Arguments
    ///
    /// - `hidden_size`: Hidden dimension (must be multiple of 8 for SIMD)
    ///
    /// # Panics
    ///
    /// Panics if `hidden_size` is not a multiple of 8.
    ///
    /// # Performance
    ///
    /// - Initialization: O(hidden_size / 8) vector allocations
    /// - Memory: hidden_size * 4 bytes for weights
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Qwen3 8B model
    /// let rmsnorm = RMSNormCapsule::new(4096);
    /// ```
    #[inline]
    pub fn new(hidden_size: usize) -> Self {
        Self::with_eps(hidden_size, Self::QWEN3_EPS)
    }

    /// Create RMSNorm with custom epsilon
    ///
    /// # Arguments
    ///
    /// - `hidden_size`: Hidden dimension (must be multiple of 8)
    /// - `eps`: Epsilon for numerical stability (typically 1e-6)
    #[inline]
    pub fn with_eps(hidden_size: usize, eps: f32) -> Self {
        assert!(
            hidden_size % 8 == 0,
            "hidden_size must be multiple of 8 for SIMD (got {})",
            hidden_size
        );
        assert!(eps > 0.0, "epsilon must be positive (got {})", eps);

        let num_vectors = hidden_size / 8;

        Self {
            weights: vec![f32x8::splat(1.0); num_vectors],
            hidden_size,
            eps,
            num_vectors,
            _padding: [0u8; 84],
        }
    }

    /// Create from learned weights (Qwen3 model weights)
    ///
    /// # Arguments
    ///
    /// - `weights`: Flat weight array (length = hidden_size)
    /// - `eps`: Epsilon for numerical stability
    ///
    /// # Panics
    ///
    /// Panics if weights.len() is not a multiple of 8.
    ///
    /// # Performance
    ///
    /// - Conversion: O(hidden_size / 8) with cache-friendly access
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Load weights from GGUF file
    /// let weights = load_gguf_tensor("model.norm.weight");
    /// let rmsnorm = RMSNormCapsule::from_weights(weights, 1e-6);
    /// ```
    #[inline]
    pub fn from_weights(weights: Vec<f32>, eps: f32) -> Self {
        let hidden_size = weights.len();
        assert!(
            hidden_size % 8 == 0,
            "weight count must be multiple of 8 for SIMD (got {})",
            hidden_size
        );
        assert!(eps > 0.0, "epsilon must be positive (got {})", eps);

        let num_vectors = hidden_size / 8;
        let mut simd_weights = Vec::with_capacity(num_vectors);

        // Convert flat weights to SIMD vectors
        for chunk in weights.chunks_exact(8) {
            // SAFETY: chunks_exact guarantees exactly 8 elements
            let arr: [f32; 8] = [
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ];
            simd_weights.push(f32x8::from_array(arr));
        }

        Self {
            weights: simd_weights,
            hidden_size,
            eps,
            num_vectors,
            _padding: [0u8; 84],
        }
    }

    /// Forward pass: output = x * rsqrt(mean(x^2) + eps) * weight
    ///
    /// # Arguments
    ///
    /// - `input`: Input tensor (length = hidden_size)
    ///
    /// # Returns
    ///
    /// - Normalized output tensor (length = hidden_size)
    ///
    /// # Performance (B32 Target)
    ///
    /// - Latency: <100ns for hidden_size=4096 (Qwen3 8B)
    /// - SIMD: 8-wide parallel sum-of-squares and normalization
    /// - Memory: Allocates output vector (use forward_inplace for zero allocation)
    ///
    /// # Algorithm
    ///
    /// 1. Compute sum of squares: sum(x^2) using SIMD
    /// 2. Compute RMS: sqrt(mean(x^2) + eps) = sqrt(sum(x^2)/n + eps)
    /// 3. Compute rsqrt: 1.0 / rms (fast reciprocal sqrt)
    /// 4. Normalize and scale: output[i] = input[i] * rsqrt * weight[i]
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_INPUT_SIZE`: Input length equals hidden_size
    /// - `#VERIFY_OUTPUT`: Output length equals hidden_size
    /// - `#ASSUME_FINITE`: Input values are finite (no NaN/Inf)
    #[inline(always)]
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        assert_eq!(
            input.len(),
            self.hidden_size,
            "input size mismatch: expected {}, got {}",
            self.hidden_size,
            input.len()
        );

        // Step 1: Compute sum of squares using SIMD
        let mut sum_sq = f32x8::splat(0.0);

        for chunk in input.chunks_exact(8) {
            let x = f32x8::from_array([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ]);
            sum_sq += x * x;
        }

        // Reduce SIMD vector to scalar sum
        let total_sum_sq: f32 = sum_sq.reduce_sum();

        // Step 2: Compute rsqrt(mean(x^2) + eps)
        let mean_sq = total_sum_sq / (self.hidden_size as f32);
        let rsqrt_val = (mean_sq + self.eps).sqrt().recip();
        let rsqrt_vec = f32x8::splat(rsqrt_val);

        // Step 3: Normalize and scale: output = input * rsqrt * weight
        let mut output = vec![0.0f32; self.hidden_size];

        for (i, (chunk, weight)) in input
            .chunks_exact(8)
            .zip(self.weights.iter())
            .enumerate()
        {
            let x = f32x8::from_array([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ]);

            // output = x * rsqrt * weight (fused operation)
            let normalized = x * rsqrt_vec * *weight;

            // Write back to output
            let base_idx = i * 8;
            let arr = normalized.to_array();
            output[base_idx..base_idx + 8].copy_from_slice(&arr);
        }

        output
    }

    /// In-place forward pass (zero allocation)
    ///
    /// # Arguments
    ///
    /// - `input`: Mutable input tensor, modified in-place to contain output
    ///
    /// # Performance (B32 Target)
    ///
    /// - Latency: <100ns for hidden_size=4096
    /// - Memory: Zero allocation (operates in-place)
    /// - SIMD: Same vectorization as forward()
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut buffer = vec![0.5f32; 4096];
    /// rmsnorm.forward_inplace(&mut buffer);
    /// // buffer now contains normalized values
    /// ```
    #[inline(always)]
    pub fn forward_inplace(&self, input: &mut [f32]) {
        assert_eq!(
            input.len(),
            self.hidden_size,
            "input size mismatch: expected {}, got {}",
            self.hidden_size,
            input.len()
        );

        // Step 1: Compute sum of squares using SIMD
        let mut sum_sq = f32x8::splat(0.0);

        for chunk in input.chunks_exact(8) {
            let x = f32x8::from_array([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ]);
            sum_sq += x * x;
        }

        // Reduce SIMD vector to scalar sum
        let total_sum_sq: f32 = sum_sq.reduce_sum();

        // Step 2: Compute rsqrt(mean(x^2) + eps)
        let mean_sq = total_sum_sq / (self.hidden_size as f32);
        let rsqrt_val = (mean_sq + self.eps).sqrt().recip();
        let rsqrt_vec = f32x8::splat(rsqrt_val);

        // Step 3: Normalize and scale in-place
        for (chunk, weight) in input.chunks_exact_mut(8).zip(self.weights.iter()) {
            let x = f32x8::from_array([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ]);

            // output = x * rsqrt * weight
            let normalized = x * rsqrt_vec * *weight;

            // Write back in-place
            let arr = normalized.to_array();
            chunk.copy_from_slice(&arr);
        }
    }

    /// Forward pass with output buffer (reuse allocation)
    ///
    /// # Arguments
    ///
    /// - `input`: Input tensor (length = hidden_size)
    /// - `output`: Pre-allocated output buffer (length = hidden_size)
    ///
    /// # Performance
    ///
    /// - Latency: <100ns for hidden_size=4096
    /// - Memory: Zero allocation (uses provided buffer)
    #[inline(always)]
    pub fn forward_into(&self, input: &[f32], output: &mut [f32]) {
        assert_eq!(
            input.len(),
            self.hidden_size,
            "input size mismatch: expected {}, got {}",
            self.hidden_size,
            input.len()
        );
        assert_eq!(
            output.len(),
            self.hidden_size,
            "output size mismatch: expected {}, got {}",
            self.hidden_size,
            output.len()
        );

        // Step 1: Compute sum of squares
        let mut sum_sq = f32x8::splat(0.0);

        for chunk in input.chunks_exact(8) {
            let x = f32x8::from_array([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ]);
            sum_sq += x * x;
        }

        let total_sum_sq: f32 = sum_sq.reduce_sum();
        let mean_sq = total_sum_sq / (self.hidden_size as f32);
        let rsqrt_val = (mean_sq + self.eps).sqrt().recip();
        let rsqrt_vec = f32x8::splat(rsqrt_val);

        // Step 2: Normalize and scale into output buffer
        for (i, (in_chunk, weight)) in input
            .chunks_exact(8)
            .zip(self.weights.iter())
            .enumerate()
        {
            let x = f32x8::from_array([
                in_chunk[0],
                in_chunk[1],
                in_chunk[2],
                in_chunk[3],
                in_chunk[4],
                in_chunk[5],
                in_chunk[6],
                in_chunk[7],
            ]);

            let normalized = x * rsqrt_vec * *weight;

            let base_idx = i * 8;
            let arr = normalized.to_array();
            output[base_idx..base_idx + 8].copy_from_slice(&arr);
        }
    }

    /// Get hidden size dimension
    #[inline(always)]
    pub const fn hidden_size(&self) -> usize {
        self.hidden_size
    }

    /// Get epsilon value
    #[inline(always)]
    pub const fn eps(&self) -> f32 {
        self.eps
    }

    /// Get number of SIMD vectors
    #[inline(always)]
    pub const fn num_vectors(&self) -> usize {
        self.num_vectors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test identity transform: weights=1.0, input normalized
    #[test]
    fn test_rmsnorm_identity() {
        let rmsnorm = RMSNormCapsule::new(8);

        // Input with RMS = 1.0 (each element = 1/sqrt(8))
        // mean(x^2) = 8 * (1/8) / 8 = 1/8, rsqrt = sqrt(8) = 2.828...
        // But with eps=1e-6, we get slight deviation

        // Simple test: uniform input
        let input = vec![1.0f32; 8];
        let output = rmsnorm.forward(&input);

        // RMS of [1,1,1,1,1,1,1,1] = sqrt(8/8) = 1.0
        // rsqrt(1.0 + 1e-6) ~= 1.0
        // Output should be close to input (since weights = 1.0)
        assert_eq!(output.len(), 8);
        for val in &output {
            assert!(
                (*val - 1.0).abs() < 0.001,
                "expected ~1.0, got {}",
                val
            );
        }
    }

    /// Test scaling: all-2.0 weights should double the output
    #[test]
    fn test_rmsnorm_scaling() {
        let weights = vec![2.0f32; 8];
        let rmsnorm = RMSNormCapsule::from_weights(weights, 1e-6);

        let input = vec![1.0f32; 8];
        let output = rmsnorm.forward(&input);

        // With weights=2.0, output should be 2x the normalized input
        // RMS of [1,1,1,1,1,1,1,1] = 1.0
        // Output = input * rsqrt(1.0 + eps) * 2.0 ~= 2.0
        assert_eq!(output.len(), 8);
        for val in &output {
            assert!(
                (*val - 2.0).abs() < 0.001,
                "expected ~2.0, got {}",
                val
            );
        }
    }

    /// Test numerical stability with very small inputs
    #[test]
    fn test_rmsnorm_numerical_stability() {
        let rmsnorm = RMSNormCapsule::new(8);

        // Very small inputs that could cause division issues without eps
        let input = vec![1e-10f32; 8];
        let output = rmsnorm.forward(&input);

        // Should not produce NaN or Inf
        assert_eq!(output.len(), 8);
        for val in &output {
            assert!(val.is_finite(), "output should be finite, got {}", val);
        }
    }

    /// Test numerical stability with zero inputs
    #[test]
    fn test_rmsnorm_zero_input() {
        let rmsnorm = RMSNormCapsule::new(8);

        // Zero input - eps prevents division by zero
        let input = vec![0.0f32; 8];
        let output = rmsnorm.forward(&input);

        // With input=0, output should be 0 (0 * anything = 0)
        assert_eq!(output.len(), 8);
        for val in &output {
            assert!(val.is_finite(), "output should be finite, got {}", val);
            assert_eq!(*val, 0.0, "output should be 0 for zero input");
        }
    }

    /// Test in-place forward matches regular forward
    #[test]
    fn test_rmsnorm_inplace_matches() {
        let weights = vec![1.5f32; 16];
        let rmsnorm = RMSNormCapsule::from_weights(weights, 1e-6);

        let input = vec![0.5f32, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0];
        let expected = rmsnorm.forward(&input);

        let mut buffer = input.clone();
        rmsnorm.forward_inplace(&mut buffer);

        for (i, (a, b)) in expected.iter().zip(buffer.iter()).enumerate() {
            assert!(
                (*a - *b).abs() < 1e-6,
                "mismatch at index {}: expected {}, got {}",
                i,
                a,
                b
            );
        }
    }

    /// Test forward_into matches regular forward
    #[test]
    fn test_rmsnorm_forward_into_matches() {
        let weights = vec![0.8f32; 8];
        let rmsnorm = RMSNormCapsule::from_weights(weights, 1e-6);

        let input = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let expected = rmsnorm.forward(&input);

        let mut output = vec![0.0f32; 8];
        rmsnorm.forward_into(&input, &mut output);

        for (i, (a, b)) in expected.iter().zip(output.iter()).enumerate() {
            assert!(
                (*a - *b).abs() < 1e-6,
                "mismatch at index {}: expected {}, got {}",
                i,
                a,
                b
            );
        }
    }

    /// Test Qwen3 8B typical dimensions (4096)
    #[test]
    fn test_rmsnorm_qwen3_8b() {
        let hidden_size = 4096;
        let rmsnorm = RMSNormCapsule::new(hidden_size);

        // Random-ish input pattern
        let input: Vec<f32> = (0..hidden_size)
            .map(|i| ((i as f32 * 0.001).sin() + 0.5))
            .collect();

        let output = rmsnorm.forward(&input);

        assert_eq!(output.len(), hidden_size);
        assert!(
            output.iter().all(|x| x.is_finite()),
            "all outputs should be finite"
        );
    }

    /// Test alignment verification
    #[test]
    fn test_rmsnorm_alignment() {
        let rmsnorm = RMSNormCapsule::new(8);
        let addr = &rmsnorm as *const _ as usize;
        assert_eq!(
            addr % 128,
            0,
            "RMSNormCapsule must be 128-byte aligned"
        );
    }

    /// Test struct size (should fit in 2 cache lines)
    #[test]
    fn test_rmsnorm_size() {
        // Size may vary based on Vec layout, but must be cache-aligned
        let size = core::mem::size_of::<RMSNormCapsule>();
        assert!(
            size >= 128 && size % 128 == 0,
            "RMSNormCapsule should be at least 128 bytes and 128B aligned, got {size}"
        );
    }

    /// Test epsilon constant
    #[test]
    fn test_qwen3_eps_constant() {
        assert_eq!(RMSNormCapsule::QWEN3_EPS, 1e-6);
    }

    /// Test dimension assertions
    #[test]
    #[should_panic(expected = "hidden_size must be multiple of 8")]
    fn test_rmsnorm_invalid_dimension() {
        let _ = RMSNormCapsule::new(7); // Not multiple of 8
    }

    /// Test weight count assertion
    #[test]
    #[should_panic(expected = "weight count must be multiple of 8")]
    fn test_rmsnorm_invalid_weights() {
        let _ = RMSNormCapsule::from_weights(vec![1.0f32; 7], 1e-6);
    }
}
