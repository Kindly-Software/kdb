//! # Phase 2: Inference Primitives
//!
//! **Nightly-first inference capsules for LLM and ML workloads.**
//!
//! This module provides cutting-edge inference primitives following IMPL-2 V3.1:
//! - **T2 (SIMD)**: f32x8 matmul, i16x8 quantized inference
//! - **T3 (Fixed-Point)**: Deterministic Q8/Q16 quantization
//! - **T4 (Batch)**: Rayon parallel batch processing
//! - **T5 (Streaming)**: Incremental attention computation
//! - **T6 (Mixed)**: Full-tier compound (T2+T3+T4+T5, 10-50× breakthrough)
//!
//! ## Nightly Features Required (IMPL-2 V3.1: Nightly-First)
//!
//! - `portable_simd`: MANDATORY (f32x8, i16x8 vectorization)
//! - `const_fn_floating_point`: BENEFICIAL (compile-time weight init, 0ns runtime, STABLE in Rust 1.82+)
//! - `generic_const_exprs`: OPTIONAL (compile-time dimension validation, incomplete)
//!
//! ## Performance Targets (B32 Validation Required)
//!
//! - MatMul (T2+T4): 4-8× speedup via f32x8 SIMD + Rayon batching
//! - Attention (T2+T5): 3-6× speedup via incremental computation
//! - Quantization (T3): 2-5× speedup via fixed-point Q8/Q16 arithmetic
//! - Combined (T2+T3+T4+T5): 10-50× compound speedup (full-tier integration)
//!
//! ## Modules
//!
//! - `matmul`: T2+T4 SIMD matrix multiplication with batch parallelism
//! - `attention`: T2+T5 flash attention with streaming computation (TODO)
//! - `quantization`: T3 fixed-point quantization (Q8/Q16) (TODO)
//!
//! ## Example
//!
//! ```rust,ignore
//! use atomic_capsule::inference::matmul::MatMulCapsule;
//!
//! // T2+T4 SIMD matmul with Rayon batch processing
//! let matmul = MatMulCapsule::new(1024, 1024, 1024);
//! let result = matmul.multiply(&weights, &inputs);
//! ```

// MatMul module (T2+T4 SIMD + Batch)
#[cfg(feature = "inference-matmul")]
pub mod matmul;

// Attention module (T2+T5 SIMD + Streaming) - TODO
// #[cfg(feature = "inference-attention")]
// pub mod attention;

// Quantization module (T3 Fixed-Point) - TODO
// #[cfg(feature = "inference-quantization")]
// pub mod quantization;

// Re-export matmul types
#[cfg(feature = "inference-matmul")]
pub use matmul::MatMulCapsule;
