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

// KV-cache compression module (T2+T10 SIMD + Probabilistic)
#[cfg(feature = "inference-kv-cache-compression")]
pub mod kv_cache_compression;

// Learned codebook module (T0+T10 Auditable + Probabilistic)
#[cfg(feature = "inference-learned-codebook")]
pub mod learned_codebook;

// Multi-token prediction module (T5 Streaming)
#[cfg(feature = "inference-multi-token-prediction")]
pub mod multi_token_prediction;

// Speculative draft module (T1+T5 Atomic + Streaming)
#[cfg(feature = "inference-speculative-draft")]
pub mod speculative_draft;

// Prefetch scheduler module (T4+T5 Batch + Streaming)
#[cfg(feature = "inference-prefetch-scheduler")]
pub mod prefetch_scheduler;

// LLM inference metacapsule (T6 Mixed tier orchestrator)
#[cfg(feature = "inference-llm-metacapsule")]
pub mod llm_inference_metacapsule;

// Re-export matmul types
#[cfg(feature = "inference-matmul")]
pub use matmul::MatMulCapsule;

// Re-export kv-cache-compression types
#[cfg(feature = "inference-kv-cache-compression")]
pub use kv_cache_compression::{
    CompressedKV, CompressionType, KVCacheCompressionCapsule,
};

// Re-export learned-codebook types
#[cfg(feature = "inference-learned-codebook")]
pub use learned_codebook::{CodebookError, LearnedCodebookCapsule};

// Re-export multi-token-prediction types
#[cfg(feature = "inference-multi-token-prediction")]
pub use multi_token_prediction::{
    MultiTokenPredictionCapsule, PredictionResult, MtpStatistics, MtpError,
};

// Re-export speculative-draft types
#[cfg(feature = "inference-speculative-draft")]
pub use speculative_draft::{
    AcceptanceStats, DraftError, RejectionReason, SpeculativeDraftCapsule, VerifyResult,
};

// Re-export prefetch-scheduler types
#[cfg(feature = "inference-prefetch-scheduler")]
pub use prefetch_scheduler::{
    PrefetchError, PrefetchRequest, PrefetchSchedulerCapsule, PrefetchStatistics, PrefetchType,
};

// Re-export llm-inference-metacapsule types
#[cfg(feature = "inference-llm-metacapsule")]
pub use llm_inference_metacapsule::{
    CompressionFlags, GenerateResult, GenerationConfig, InferenceMode, InferenceStatistics,
    LLMInferenceMetacapsule, Phase,
};
