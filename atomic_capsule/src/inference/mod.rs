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

// RMSNorm module (T2 SIMD) - RMS Normalization for Qwen3/LLaMA
#[cfg(feature = "inference-rmsnorm")]
pub mod rmsnorm;

// SwiGLU activation module (T2 SIMD)
#[cfg(feature = "inference-swiglu")]
pub mod swiglu;

// RoPE module (T2 SIMD rotary position embedding for Qwen3/LLaMA)
#[cfg(feature = "inference-rope")]
pub mod rope;

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

// BPE tokenizer module (T4 Batch - parallel BPE with thread-local buffers)
#[cfg(feature = "inference-bpe-tokenizer")]
pub mod bpe_tokenizer;

// Re-export matmul types
#[cfg(feature = "inference-matmul")]
pub use matmul::MatMulCapsule;

// Re-export rmsnorm types
#[cfg(feature = "inference-rmsnorm")]
pub use rmsnorm::RMSNormCapsule;

// Re-export swiglu types
#[cfg(feature = "inference-swiglu")]
pub use swiglu::SwiGLUCapsule;

// Re-export rope types
#[cfg(feature = "inference-rope")]
pub use rope::RoPECapsule;

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

// Re-export bpe-tokenizer types
#[cfg(feature = "inference-bpe-tokenizer")]
pub use bpe_tokenizer::{
    BPETokenizerCapsule, MergePair, TokenEntry, TokenizerError,
};

// Lockfree Vector Quantization module (T1+T2 Atomic + SIMD)
#[cfg(feature = "inference-vector-quant")]
pub mod lockfree_vector_quant;

// Re-export lockfree-vector-quant types
#[cfg(feature = "inference-vector-quant")]
pub use lockfree_vector_quant::{
    LockfreeVectorQuantCapsule, VQConfig, VQError, MAX_CODEBOOKS, DEFAULT_CODEBOOK_SIZE,
    DEFAULT_VECTOR_DIM,
};

// Qwen3 Architecture Capsule (T6 Mixed tier metacapsule)
// Complete Qwen3 8B/30B architecture binding for inference
#[cfg(feature = "inference-qwen3")]
pub mod qwen3_architecture;

// Re-export qwen3-architecture types
#[cfg(feature = "inference-qwen3")]
pub use qwen3_architecture::{
    EmbeddingCapsule, GenerationConfig, InferencePhase, KVCache, LoadError,
    Qwen3ArchitectureCapsule, Qwen3Config, Qwen3LayerCapsule,
};

// Ternary MatMul module (T2+T3 SIMD + Fixed-Point)
// BitNet b1.58 breakthrough: 2.71x speedup via addition-only matmul
#[cfg(feature = "inference-ternary-matmul")]
pub mod ternary_matmul;

// Re-export ternary-matmul types
#[cfg(feature = "inference-ternary-matmul")]
pub use ternary_matmul::{
    TernaryMatMulCapsule, TernaryMatMulError, TernaryValue,
};

// Streaming KV module (T5+T10 StreamingLLM + MiniKV + H2O)
// Combines attention sink, sliding window, heavy-hitter oracle, and 2-bit quantization
// for 86% memory reduction with unlimited context
#[cfg(feature = "inference-streaming-kv")]
pub mod streaming_kv;

// Re-export streaming-kv types
#[cfg(feature = "inference-streaming-kv")]
pub use streaming_kv::{
    CompressedKVEntry, KVEntry, StreamingKVCapsule, StreamingKVConfig, StreamingKVSnapshot,
};
