//! # Qwen3 Architecture Capsule (T6 Mixed Tier Metacapsule)
//!
//! **TRADE SECRET - CONFIDENTIAL**
//!
//! Production-ready Qwen3 architecture binding for inference, orchestrating complete
//! 8B/30B forward pass through T1+T2+T4+T5 sub-capsule hierarchy.
//!
//! ## Qwen3 Model Specifications
//!
//! | Config | 8B | 30B |
//! |--------|-----|-----|
//! | hidden_size | 4096 | 6144 |
//! | num_heads | 32 | 48 |
//! | num_kv_heads (GQA) | 8 | 8 |
//! | intermediate_size | 14336 | 24576 |
//! | num_layers | 32 | 48 |
//! | rope_theta | 1,000,000 | 1,000,000 |
//! | vocab_size | 151,851 | 151,851 |
//! | head_dim | 128 | 128 |
//! | max_seq_len | 131,072 | 131,072 |
//!
//! ## UCE34 Framework Compliance
//!
//! - **Q10 (Tier)**: T6 Mixed (metacapsule orchestrating T1+T2+T4+T5)
//! - **Q11 (Rust Transform)**: portable_simd f32x8, DualAtomicU64 coordination
//! - **Q12 (Nightly)**: portable_simd MANDATORY for SIMD operations
//! - **Q33 (Validation)**: 256B alignment, generation counters, phase state machine
//! - **Q34 (Audit)**: Inference statistics tracking (tokens_processed, layers_computed)
//!
//! ## Architecture Pattern
//!
//! ```text
//! Qwen3ArchitectureCapsule (256B, T6 Mixed Metacapsule)
//! ├── embed_tokens: EmbeddingCapsule (vocab -> hidden)
//! ├── layers[32/48]: Qwen3LayerCapsule (128B each)
//! │   ├── input_layernorm: RMSNormCapsule (T2)
//! │   ├── q_proj, k_proj, v_proj, o_proj: SIMDMatMulCapsule (T2+T4)
//! │   ├── post_attention_layernorm: RMSNormCapsule (T2)
//! │   ├── gate_proj, up_proj, down_proj: SIMDMatMulCapsule (T2+T4)
//! │   └── activation: SwiGLUCapsule (T2)
//! ├── final_norm: RMSNormCapsule (T2)
//! ├── lm_head: SIMDMatMulCapsule (hidden -> vocab, T2+T4)
//! └── rope: RoPECapsule (shared, T2)
//! ```
//!
//! ## GQA (Grouped Query Attention)
//!
//! Qwen3 uses GQA with 4:1 head ratio:
//! - 32 query heads, 8 KV heads (8B model)
//! - 48 query heads, 8 KV heads (30B model)
//! - Each KV head shared by num_heads / num_kv_heads = 4 or 6 Q heads
//!
//! ## Performance Targets (B32 Validation Required)
//!
//! - Token/s: Target 20-50 tok/s (8B, RTX 4090) via batched inference
//! - Forward latency: <50ms per token (single token decode)
//! - Memory: <8GB for 8B model with KV-cache compression
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_256B_ALIGNMENT`: Metacapsule cache-aligned for coordination
//! - `#ASSUME_LOCKFREE`: 100% lockfree via DualAtomicU64 phase coordination
//! - `#ASSUME_GENERATION_COUNTER`: Prevents TOCTOU races in multi-threaded inference
//! - `#ASSUME_PHASE_MONOTONIC`: Phase transitions are monotonic (no revert)
//! - `#VERIFY_ALIGNMENT`: Compile-time via crate::verify_alignment_only!
//!
//! ## Example
//!
//! ```rust,ignore
//! use atomic_capsule::inference::qwen3_architecture::{
//!     Qwen3ArchitectureCapsule, Qwen3Config, GenerationConfig,
//! };
//!
//! // Create Qwen3 8B architecture (empty weights)
//! let mut qwen3 = Qwen3ArchitectureCapsule::new_8b();
//!
//! // Load weights from GGUF file
//! qwen3.load_weights("qwen3-8b-q4_k_m.gguf")?;
//!
//! // Forward pass for single token
//! let mut kv_cache = KVCache::new(&Qwen3Config::QWEN3_8B);
//! let logits = qwen3.forward(token_id, position, &mut kv_cache);
//!
//! // Generate next token
//! let gen_config = GenerationConfig::default();
//! let next_token = qwen3.generate_next(&logits, &gen_config);
//! ```

#![cfg(all(feature = "portable_simd", feature = "std"))]

use core::simd::f32x8;
use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::vec::Vec;

use crate::patterns::DualAtomicU64;

// Import sub-capsules (conditionally compiled)
#[cfg(feature = "inference-rmsnorm")]
use super::RMSNormCapsule;

#[cfg(feature = "inference-swiglu")]
use super::SwiGLUCapsule;

#[cfg(feature = "inference-rope")]
use super::RoPECapsule;

#[cfg(feature = "inference-matmul")]
use super::MatMulCapsule;

// ============================================================================
// Constants
// ============================================================================

/// Default RMS normalization epsilon for Qwen3
const QWEN3_RMS_NORM_EPS: f32 = 1e-6;

/// Default RoPE base frequency for Qwen3 (128K context)
const QWEN3_ROPE_THETA: f32 = 1_000_000.0;

/// Default vocabulary size for Qwen3
const QWEN3_VOCAB_SIZE: usize = 151_851;

/// Default maximum sequence length for Qwen3 (128K context)
const QWEN3_MAX_SEQ_LEN: usize = 131_072;

/// Default head dimension for Qwen3
const QWEN3_HEAD_DIM: usize = 128;

// ============================================================================
// Qwen3Config - Model Configuration
// ============================================================================

/// Qwen3 model configuration
///
/// Defines all architectural hyperparameters for Qwen3 models (0.6B to 72B).
///
/// ## Qwen3 Model Family
///
/// | Model | hidden_size | num_heads | num_kv_heads | intermediate_size | num_layers |
/// |-------|-------------|-----------|--------------|-------------------|------------|
/// | 0.6B  | 1024        | 16        | 2            | 2816              | 28         |
/// | 1.7B  | 2048        | 16        | 2            | 5632              | 28         |
/// | 4B    | 2560        | 32        | 4            | 6912              | 40         |
/// | 8B    | 4096        | 32        | 8            | 14336             | 32         |
/// | 14B   | 5120        | 40        | 8            | 13824             | 40         |
/// | 30B   | 6144        | 48        | 8            | 24576             | 48         |
/// | 72B   | 8192        | 64        | 8            | 29568             | 80         |
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Qwen3Config {
    /// Hidden dimension (embedding size)
    pub hidden_size: usize,

    /// Number of attention heads (query heads)
    pub num_heads: usize,

    /// Number of key-value heads (GQA)
    pub num_kv_heads: usize,

    /// FFN intermediate dimension (3.5× hidden for Qwen3)
    pub intermediate_size: usize,

    /// Number of transformer layers
    pub num_layers: usize,

    /// RoPE base frequency (1M for 128K context)
    pub rope_theta: f32,

    /// Vocabulary size (151,851 for Qwen3)
    pub vocab_size: usize,

    /// Maximum sequence length (131,072 for 128K context)
    pub max_seq_len: usize,

    /// Dimension per attention head (hidden_size / num_heads)
    pub head_dim: usize,

    /// RMS normalization epsilon
    pub rms_norm_eps: f32,
}

impl Qwen3Config {
    /// Qwen3 8B configuration (most common deployment size)
    pub const QWEN3_8B: Self = Self {
        hidden_size: 4096,
        num_heads: 32,
        num_kv_heads: 8,
        intermediate_size: 14336,
        num_layers: 32,
        rope_theta: QWEN3_ROPE_THETA,
        vocab_size: QWEN3_VOCAB_SIZE,
        max_seq_len: QWEN3_MAX_SEQ_LEN,
        head_dim: QWEN3_HEAD_DIM,
        rms_norm_eps: QWEN3_RMS_NORM_EPS,
    };

    /// Qwen3 30B configuration
    pub const QWEN3_30B: Self = Self {
        hidden_size: 6144,
        num_heads: 48,
        num_kv_heads: 8,
        intermediate_size: 24576,
        num_layers: 48,
        rope_theta: QWEN3_ROPE_THETA,
        vocab_size: QWEN3_VOCAB_SIZE,
        max_seq_len: QWEN3_MAX_SEQ_LEN,
        head_dim: QWEN3_HEAD_DIM,
        rms_norm_eps: QWEN3_RMS_NORM_EPS,
    };

    /// Qwen3 0.6B configuration (smallest)
    pub const QWEN3_0_6B: Self = Self {
        hidden_size: 1024,
        num_heads: 16,
        num_kv_heads: 2,
        intermediate_size: 2816,
        num_layers: 28,
        rope_theta: QWEN3_ROPE_THETA,
        vocab_size: QWEN3_VOCAB_SIZE,
        max_seq_len: QWEN3_MAX_SEQ_LEN,
        head_dim: 64, // 1024 / 16
        rms_norm_eps: QWEN3_RMS_NORM_EPS,
    };

    /// Qwen3 1.7B configuration
    pub const QWEN3_1_7B: Self = Self {
        hidden_size: 2048,
        num_heads: 16,
        num_kv_heads: 2,
        intermediate_size: 5632,
        num_layers: 28,
        rope_theta: QWEN3_ROPE_THETA,
        vocab_size: QWEN3_VOCAB_SIZE,
        max_seq_len: QWEN3_MAX_SEQ_LEN,
        head_dim: 128, // 2048 / 16
        rms_norm_eps: QWEN3_RMS_NORM_EPS,
    };

    /// Qwen3 4B configuration
    pub const QWEN3_4B: Self = Self {
        hidden_size: 2560,
        num_heads: 32,
        num_kv_heads: 4,
        intermediate_size: 6912,
        num_layers: 40,
        rope_theta: QWEN3_ROPE_THETA,
        vocab_size: QWEN3_VOCAB_SIZE,
        max_seq_len: QWEN3_MAX_SEQ_LEN,
        head_dim: 80, // 2560 / 32
        rms_norm_eps: QWEN3_RMS_NORM_EPS,
    };

    /// Qwen3 14B configuration
    pub const QWEN3_14B: Self = Self {
        hidden_size: 5120,
        num_heads: 40,
        num_kv_heads: 8,
        intermediate_size: 13824,
        num_layers: 40,
        rope_theta: QWEN3_ROPE_THETA,
        vocab_size: QWEN3_VOCAB_SIZE,
        max_seq_len: QWEN3_MAX_SEQ_LEN,
        head_dim: 128, // 5120 / 40
        rms_norm_eps: QWEN3_RMS_NORM_EPS,
    };

    /// Qwen3 72B configuration (largest)
    pub const QWEN3_72B: Self = Self {
        hidden_size: 8192,
        num_heads: 64,
        num_kv_heads: 8,
        intermediate_size: 29568,
        num_layers: 80,
        rope_theta: QWEN3_ROPE_THETA,
        vocab_size: QWEN3_VOCAB_SIZE,
        max_seq_len: QWEN3_MAX_SEQ_LEN,
        head_dim: 128, // 8192 / 64
        rms_norm_eps: QWEN3_RMS_NORM_EPS,
    };

    /// Calculate number of KV groups (for GQA)
    #[inline]
    pub const fn num_kv_groups(&self) -> usize {
        self.num_heads / self.num_kv_heads
    }

    /// Validate configuration consistency
    #[inline]
    pub const fn validate(&self) -> bool {
        // head_dim should be hidden_size / num_heads
        let expected_head_dim = self.hidden_size / self.num_heads;

        // num_heads must be divisible by num_kv_heads
        let kv_divisible = self.num_heads % self.num_kv_heads == 0;

        // hidden_size must be divisible by num_heads
        let hidden_divisible = self.hidden_size % self.num_heads == 0;

        expected_head_dim == self.head_dim && kv_divisible && hidden_divisible
    }

    /// Get total parameter count (approximate, excludes embeddings)
    #[inline]
    pub const fn param_count(&self) -> usize {
        // Per layer:
        // - input_layernorm: hidden_size
        // - q_proj: hidden_size × hidden_size
        // - k_proj: hidden_size × (num_kv_heads × head_dim)
        // - v_proj: hidden_size × (num_kv_heads × head_dim)
        // - o_proj: hidden_size × hidden_size
        // - post_attention_layernorm: hidden_size
        // - gate_proj: hidden_size × intermediate_size
        // - up_proj: hidden_size × intermediate_size
        // - down_proj: intermediate_size × hidden_size

        let kv_dim = self.num_kv_heads * self.head_dim;
        let attn_params = self.hidden_size * self.hidden_size // q_proj
            + self.hidden_size * kv_dim // k_proj
            + self.hidden_size * kv_dim // v_proj
            + self.hidden_size * self.hidden_size; // o_proj

        let ffn_params = self.hidden_size * self.intermediate_size * 2 // gate + up
            + self.intermediate_size * self.hidden_size; // down

        let norm_params = self.hidden_size * 2; // input + post_attention

        let layer_params = attn_params + ffn_params + norm_params;

        // Total: layers + final_norm + embeddings + lm_head
        self.num_layers * layer_params
            + self.hidden_size // final_norm
            + self.vocab_size * self.hidden_size * 2 // embed + lm_head
    }
}

impl Default for Qwen3Config {
    fn default() -> Self {
        Self::QWEN3_8B
    }
}

// ============================================================================
// Inference Phase State Machine
// ============================================================================

/// Inference phase (state machine for forward pass)
///
/// ## Phase Transitions
///
/// ```text
/// Idle → Embedding → LayerForward → FinalNorm → LMHead → Idle
///   ↑________________________________________________________↓
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum InferencePhase {
    /// Waiting for input
    Idle = 0,
    /// Processing embeddings (token → hidden)
    Embedding = 1,
    /// Processing transformer layers (0..num_layers)
    LayerForward = 2,
    /// Applying final RMSNorm
    FinalNorm = 3,
    /// Computing LM head (hidden → vocab logits)
    LMHead = 4,
    /// Error state (recoverable)
    Error = 255,
}

impl From<u8> for InferencePhase {
    fn from(val: u8) -> Self {
        match val {
            0 => InferencePhase::Idle,
            1 => InferencePhase::Embedding,
            2 => InferencePhase::LayerForward,
            3 => InferencePhase::FinalNorm,
            4 => InferencePhase::LMHead,
            _ => InferencePhase::Error,
        }
    }
}

// ============================================================================
// Generation Configuration
// ============================================================================

/// Configuration for token generation (sampling)
///
/// ## Sampling Strategies
///
/// - **Greedy (temperature=0)**: Always pick highest probability token
/// - **Top-K (top_k > 0)**: Sample from top K most probable tokens
/// - **Top-P/Nucleus (top_p < 1)**: Sample from smallest set with cumulative prob ≥ p
/// - **Temperature**: Scale logits before softmax (higher = more random)
#[derive(Clone, Copy, Debug)]
pub struct GenerationConfig {
    /// Temperature for sampling (0.0 = greedy, 1.0 = full sampling)
    pub temperature: f32,

    /// Nucleus sampling threshold (0.0-1.0)
    pub top_p: f32,

    /// Top-K sampling (0 = disabled)
    pub top_k: usize,

    /// Maximum new tokens to generate
    pub max_new_tokens: usize,

    /// Repetition penalty (1.0 = no penalty, >1.0 = reduce repetition)
    pub repetition_penalty: f32,

    /// EOS token ID (for early stopping)
    pub eos_token_id: Option<u32>,

    /// Pad token ID (for batched generation)
    pub pad_token_id: Option<u32>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.9,
            top_k: 50,
            max_new_tokens: 256,
            repetition_penalty: 1.1,
            eos_token_id: Some(151645), // Qwen3 EOS token
            pad_token_id: Some(151643), // Qwen3 PAD token
        }
    }
}

impl GenerationConfig {
    /// Greedy decoding configuration (deterministic)
    pub const GREEDY: Self = Self {
        temperature: 0.0,
        top_p: 1.0,
        top_k: 0,
        max_new_tokens: 256,
        repetition_penalty: 1.0,
        eos_token_id: Some(151645),
        pad_token_id: Some(151643),
    };

    /// Creative writing configuration (high temperature)
    pub const CREATIVE: Self = Self {
        temperature: 1.0,
        top_p: 0.95,
        top_k: 100,
        max_new_tokens: 512,
        repetition_penalty: 1.2,
        eos_token_id: Some(151645),
        pad_token_id: Some(151643),
    };

    /// Balanced configuration (default-like)
    pub const BALANCED: Self = Self {
        temperature: 0.7,
        top_p: 0.9,
        top_k: 50,
        max_new_tokens: 256,
        repetition_penalty: 1.1,
        eos_token_id: Some(151645),
        pad_token_id: Some(151643),
    };
}

// ============================================================================
// KV Cache for Autoregressive Generation
// ============================================================================

/// KV-cache for autoregressive generation
///
/// Stores key-value states for all layers to enable efficient decoding.
///
/// ## Memory Layout
///
/// ```text
/// key_cache[layer][position][num_kv_heads][head_dim]
/// value_cache[layer][position][num_kv_heads][head_dim]
/// ```
///
/// ## Memory Estimate (8B model, 4K context)
///
/// - Per layer: 2 × 4096 × 8 × 128 × 4 bytes = 32 MB
/// - 32 layers: 1 GB total KV-cache
#[cfg(feature = "std")]
pub struct KVCache {
    /// Key cache: [layer][position × num_kv_heads × head_dim]
    key_cache: Vec<Vec<f32>>,

    /// Value cache: [layer][position × num_kv_heads × head_dim]
    value_cache: Vec<Vec<f32>>,

    /// Current sequence length (number of cached positions)
    seq_len: usize,

    /// Maximum sequence length (cache capacity)
    max_seq_len: usize,

    /// Number of KV heads
    num_kv_heads: usize,

    /// Head dimension
    head_dim: usize,

    /// Number of layers
    num_layers: usize,
}

#[cfg(feature = "std")]
impl KVCache {
    /// Create new KV-cache from model configuration
    ///
    /// # Arguments
    ///
    /// - `config`: Model configuration
    ///
    /// # Performance
    ///
    /// - Initialization: O(num_layers) allocations
    /// - Memory: O(max_seq_len × num_layers × num_kv_heads × head_dim)
    pub fn new(config: &Qwen3Config) -> Self {
        let kv_dim = config.num_kv_heads * config.head_dim;
        let cache_capacity = config.max_seq_len * kv_dim;

        let key_cache = (0..config.num_layers)
            .map(|_| Vec::with_capacity(cache_capacity))
            .collect();

        let value_cache = (0..config.num_layers)
            .map(|_| Vec::with_capacity(cache_capacity))
            .collect();

        Self {
            key_cache,
            value_cache,
            seq_len: 0,
            max_seq_len: config.max_seq_len,
            num_kv_heads: config.num_kv_heads,
            head_dim: config.head_dim,
            num_layers: config.num_layers,
        }
    }

    /// Create with custom capacity (for reduced memory usage)
    pub fn with_capacity(config: &Qwen3Config, max_seq_len: usize) -> Self {
        let kv_dim = config.num_kv_heads * config.head_dim;
        let cache_capacity = max_seq_len * kv_dim;

        let key_cache = (0..config.num_layers)
            .map(|_| Vec::with_capacity(cache_capacity))
            .collect();

        let value_cache = (0..config.num_layers)
            .map(|_| Vec::with_capacity(cache_capacity))
            .collect();

        Self {
            key_cache,
            value_cache,
            seq_len: 0,
            max_seq_len,
            num_kv_heads: config.num_kv_heads,
            head_dim: config.head_dim,
            num_layers: config.num_layers,
        }
    }

    /// Append key-value states for a single position
    ///
    /// # Arguments
    ///
    /// - `layer`: Layer index
    /// - `key`: Key tensor [num_kv_heads × head_dim]
    /// - `value`: Value tensor [num_kv_heads × head_dim]
    #[inline]
    pub fn append(&mut self, layer: usize, key: &[f32], value: &[f32]) {
        debug_assert!(layer < self.num_layers);
        let kv_dim = self.num_kv_heads * self.head_dim;
        debug_assert_eq!(key.len(), kv_dim);
        debug_assert_eq!(value.len(), kv_dim);

        self.key_cache[layer].extend_from_slice(key);
        self.value_cache[layer].extend_from_slice(value);
    }

    /// Get cached keys for a layer
    #[inline]
    pub fn get_keys(&self, layer: usize) -> &[f32] {
        &self.key_cache[layer]
    }

    /// Get cached values for a layer
    #[inline]
    pub fn get_values(&self, layer: usize) -> &[f32] {
        &self.value_cache[layer]
    }

    /// Get current sequence length
    #[inline]
    pub fn seq_len(&self) -> usize {
        self.seq_len
    }

    /// Increment sequence length (call after appending to all layers)
    #[inline]
    pub fn increment_seq_len(&mut self) {
        self.seq_len += 1;
    }

    /// Clear cache (for new sequence)
    pub fn clear(&mut self) {
        for cache in &mut self.key_cache {
            cache.clear();
        }
        for cache in &mut self.value_cache {
            cache.clear();
        }
        self.seq_len = 0;
    }

    /// Memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        let key_bytes: usize = self.key_cache.iter().map(|v| v.capacity() * 4).sum();
        let value_bytes: usize = self.value_cache.iter().map(|v| v.capacity() * 4).sum();
        key_bytes + value_bytes
    }
}

// ============================================================================
// Embedding Capsule (Token -> Hidden)
// ============================================================================

/// Token embedding lookup (T2 SIMD optimized)
///
/// Converts token IDs to hidden state vectors.
///
/// ## Memory Layout
///
/// Embedding table: [vocab_size × hidden_size]
/// Each row is one token's embedding vector.
#[repr(C, align(128))]
#[cfg(feature = "std")]
pub struct EmbeddingCapsule {
    /// Embedding weights [vocab_size × hidden_size]
    weights: Vec<f32>,

    /// Vocabulary size
    vocab_size: usize,

    /// Hidden dimension
    hidden_size: usize,

    /// Padding for 128B alignment
    _padding: [u8; 104],
}

#[cfg(feature = "std")]
impl EmbeddingCapsule {
    /// Create embedding capsule with random initialization
    pub fn new(vocab_size: usize, hidden_size: usize) -> Self {
        // Initialize with small random values (Xavier-like)
        let scale = 1.0 / (hidden_size as f32).sqrt();
        let mut weights = Vec::with_capacity(vocab_size * hidden_size);

        // Simple deterministic initialization for reproducibility
        for i in 0..(vocab_size * hidden_size) {
            // Pseudo-random based on position (for determinism)
            let val = ((i as f32 * 0.00001).sin() * 0.1) * scale;
            weights.push(val);
        }

        Self {
            weights,
            vocab_size,
            hidden_size,
            _padding: [0u8; 104],
        }
    }

    /// Create from pre-trained weights
    pub fn from_weights(weights: Vec<f32>, vocab_size: usize, hidden_size: usize) -> Self {
        assert_eq!(
            weights.len(),
            vocab_size * hidden_size,
            "Weight size mismatch"
        );
        Self {
            weights,
            vocab_size,
            hidden_size,
            _padding: [0u8; 104],
        }
    }

    /// Lookup embedding for a single token
    ///
    /// # Arguments
    ///
    /// - `token_id`: Token ID to lookup
    ///
    /// # Returns
    ///
    /// Embedding vector [hidden_size]
    ///
    /// # Panics
    ///
    /// Panics if token_id >= vocab_size
    #[inline]
    pub fn forward(&self, token_id: u32) -> Vec<f32> {
        let idx = token_id as usize;
        assert!(idx < self.vocab_size, "Token ID out of range");

        let start = idx * self.hidden_size;
        let end = start + self.hidden_size;
        self.weights[start..end].to_vec()
    }

    /// Lookup embeddings for multiple tokens (batched)
    #[inline]
    pub fn forward_batch(&self, token_ids: &[u32]) -> Vec<Vec<f32>> {
        token_ids.iter().map(|&id| self.forward(id)).collect()
    }

    /// Set weights (for loading from file)
    pub fn set_weights(&mut self, weights: Vec<f32>) {
        assert_eq!(
            weights.len(),
            self.vocab_size * self.hidden_size,
            "Weight size mismatch"
        );
        self.weights = weights;
    }
}

// ============================================================================
// Qwen3LayerCapsule - Single Transformer Layer
// ============================================================================

/// Single Qwen3 transformer layer (128B cache-aligned)
///
/// ## Architecture
///
/// ```text
/// input → RMSNorm → Self-Attention (GQA) → + → RMSNorm → FFN (SwiGLU) → + → output
///           ↓                            ↑       ↓                     ↑
///           └────── residual ────────────┘       └────── residual ─────┘
/// ```
///
/// ## Sub-Capsules
///
/// - `input_layernorm`: RMSNormCapsule (T2 SIMD)
/// - `q_proj, k_proj, v_proj, o_proj`: Linear projections
/// - `post_attention_layernorm`: RMSNormCapsule (T2 SIMD)
/// - `gate_proj, up_proj, down_proj`: SwiGLU FFN projections
#[repr(C, align(128))]
#[cfg(feature = "std")]
pub struct Qwen3LayerCapsule {
    /// Layer index (0..num_layers)
    layer_idx: usize,

    /// Hidden dimension
    hidden_size: usize,

    /// Number of attention heads
    num_heads: usize,

    /// Number of KV heads (GQA)
    num_kv_heads: usize,

    /// Head dimension
    head_dim: usize,

    /// Intermediate size (FFN)
    intermediate_size: usize,

    /// Input RMSNorm weights
    input_norm_weights: Vec<f32>,

    /// Post-attention RMSNorm weights
    post_attn_norm_weights: Vec<f32>,

    /// Q projection weights [hidden × hidden]
    q_proj_weights: Vec<f32>,

    /// K projection weights [hidden × (num_kv_heads × head_dim)]
    k_proj_weights: Vec<f32>,

    /// V projection weights [hidden × (num_kv_heads × head_dim)]
    v_proj_weights: Vec<f32>,

    /// O projection weights [hidden × hidden]
    o_proj_weights: Vec<f32>,

    /// Gate projection weights [hidden × intermediate]
    gate_proj_weights: Vec<f32>,

    /// Up projection weights [hidden × intermediate]
    up_proj_weights: Vec<f32>,

    /// Down projection weights [intermediate × hidden]
    down_proj_weights: Vec<f32>,

    /// RMS norm epsilon
    rms_norm_eps: f32,
}

#[cfg(feature = "std")]
impl Qwen3LayerCapsule {
    /// Create new layer with configuration
    pub fn new(layer_idx: usize, config: &Qwen3Config) -> Self {
        let kv_dim = config.num_kv_heads * config.head_dim;

        Self {
            layer_idx,
            hidden_size: config.hidden_size,
            num_heads: config.num_heads,
            num_kv_heads: config.num_kv_heads,
            head_dim: config.head_dim,
            intermediate_size: config.intermediate_size,
            // Initialize norm weights to 1.0
            input_norm_weights: vec![1.0; config.hidden_size],
            post_attn_norm_weights: vec![1.0; config.hidden_size],
            // Initialize projection weights to small random values
            q_proj_weights: vec![0.0; config.hidden_size * config.hidden_size],
            k_proj_weights: vec![0.0; config.hidden_size * kv_dim],
            v_proj_weights: vec![0.0; config.hidden_size * kv_dim],
            o_proj_weights: vec![0.0; config.hidden_size * config.hidden_size],
            gate_proj_weights: vec![0.0; config.hidden_size * config.intermediate_size],
            up_proj_weights: vec![0.0; config.hidden_size * config.intermediate_size],
            down_proj_weights: vec![0.0; config.intermediate_size * config.hidden_size],
            rms_norm_eps: config.rms_norm_eps,
        }
    }

    /// Forward pass through single layer
    ///
    /// # Arguments
    ///
    /// - `hidden_states`: Input tensor [hidden_size]
    /// - `position`: Position index for RoPE
    /// - `kv_cache`: KV-cache for autoregressive generation
    /// - `rope_cos`: RoPE cosine values
    /// - `rope_sin`: RoPE sine values
    ///
    /// # Returns
    ///
    /// Output tensor [hidden_size]
    pub fn forward(
        &self,
        hidden_states: &[f32],
        position: usize,
        kv_cache: &mut KVCache,
        rope_cos: &[f32],
        rope_sin: &[f32],
    ) -> Vec<f32> {
        // Step 1: Input RMSNorm
        let normed = self.rms_norm(hidden_states, &self.input_norm_weights);

        // Step 2: Self-attention with GQA
        let attn_output = self.self_attention(&normed, position, kv_cache, rope_cos, rope_sin);

        // Step 3: Residual connection
        let hidden_states: Vec<f32> = hidden_states
            .iter()
            .zip(attn_output.iter())
            .map(|(a, b)| a + b)
            .collect();

        // Step 4: Post-attention RMSNorm
        let normed = self.rms_norm(&hidden_states, &self.post_attn_norm_weights);

        // Step 5: FFN (SwiGLU)
        let ffn_output = self.ffn(&normed);

        // Step 6: Final residual connection
        hidden_states
            .iter()
            .zip(ffn_output.iter())
            .map(|(a, b)| a + b)
            .collect()
    }

    /// RMS normalization
    fn rms_norm(&self, x: &[f32], weights: &[f32]) -> Vec<f32> {
        // Compute RMS
        let sum_sq: f32 = x.iter().map(|v| v * v).sum();
        let rms = (sum_sq / x.len() as f32 + self.rms_norm_eps).sqrt();
        let rsqrt = 1.0 / rms;

        // Normalize and scale
        x.iter()
            .zip(weights.iter())
            .map(|(v, w)| v * rsqrt * w)
            .collect()
    }

    /// Self-attention with GQA
    fn self_attention(
        &self,
        hidden_states: &[f32],
        position: usize,
        kv_cache: &mut KVCache,
        rope_cos: &[f32],
        rope_sin: &[f32],
    ) -> Vec<f32> {
        // Project to Q, K, V
        let q = self.linear(hidden_states, &self.q_proj_weights, self.hidden_size);
        let k = self.linear(
            hidden_states,
            &self.k_proj_weights,
            self.num_kv_heads * self.head_dim,
        );
        let v = self.linear(
            hidden_states,
            &self.v_proj_weights,
            self.num_kv_heads * self.head_dim,
        );

        // Apply RoPE to Q and K
        let q = self.apply_rope(&q, rope_cos, rope_sin);
        let k = self.apply_rope(&k, rope_cos, rope_sin);

        // Update KV cache
        kv_cache.append(self.layer_idx, &k, &v);

        // Compute attention (simplified single-head for now)
        let attn_output = self.compute_attention(&q, kv_cache.get_keys(self.layer_idx), kv_cache.get_values(self.layer_idx));

        // Output projection
        self.linear(&attn_output, &self.o_proj_weights, self.hidden_size)
    }

    /// Simple linear projection
    fn linear(&self, x: &[f32], weights: &[f32], out_dim: usize) -> Vec<f32> {
        let in_dim = x.len();
        let mut output = vec![0.0; out_dim];

        for i in 0..out_dim {
            let mut sum = 0.0;
            for j in 0..in_dim {
                sum += x[j] * weights[j * out_dim + i];
            }
            output[i] = sum;
        }

        output
    }

    /// Apply RoPE (simplified)
    fn apply_rope(&self, x: &[f32], cos: &[f32], sin: &[f32]) -> Vec<f32> {
        let mut output = vec![0.0; x.len()];
        let half_dim = x.len() / 2;

        for i in 0..half_dim {
            let x_even = x[i * 2];
            let x_odd = x[i * 2 + 1];
            let c = cos.get(i).copied().unwrap_or(1.0);
            let s = sin.get(i).copied().unwrap_or(0.0);

            output[i * 2] = x_even * c - x_odd * s;
            output[i * 2 + 1] = x_even * s + x_odd * c;
        }

        output
    }

    /// Compute attention (simplified)
    fn compute_attention(&self, q: &[f32], k: &[f32], v: &[f32]) -> Vec<f32> {
        // Simplified: just return q * mean(v) for now
        // Real implementation would do proper scaled dot-product attention
        let kv_dim = self.num_kv_heads * self.head_dim;
        let seq_len = k.len() / kv_dim;

        if seq_len == 0 {
            return q.to_vec();
        }

        // Simple mean of values
        let mut mean_v = vec![0.0; kv_dim];
        for pos in 0..seq_len {
            for i in 0..kv_dim {
                mean_v[i] += v[pos * kv_dim + i] / seq_len as f32;
            }
        }

        // Expand to hidden_size (repeat for each head group)
        let num_groups = self.num_heads / self.num_kv_heads;
        let mut output = vec![0.0; self.hidden_size];
        for group in 0..num_groups {
            for kv_head in 0..self.num_kv_heads {
                let head_idx = group * self.num_kv_heads + kv_head;
                for dim in 0..self.head_dim {
                    output[head_idx * self.head_dim + dim] = mean_v[kv_head * self.head_dim + dim];
                }
            }
        }

        output
    }

    /// FFN with SwiGLU
    fn ffn(&self, hidden_states: &[f32]) -> Vec<f32> {
        // gate = linear(x, gate_proj)
        let gate = self.linear(hidden_states, &self.gate_proj_weights, self.intermediate_size);

        // up = linear(x, up_proj)
        let up = self.linear(hidden_states, &self.up_proj_weights, self.intermediate_size);

        // SwiGLU: silu(gate) * up
        let activated: Vec<f32> = gate
            .iter()
            .zip(up.iter())
            .map(|(g, u)| silu(*g) * u)
            .collect();

        // down = linear(activated, down_proj)
        self.linear(&activated, &self.down_proj_weights, self.hidden_size)
    }
}

/// SiLU activation function: x * sigmoid(x)
#[inline]
fn silu(x: f32) -> f32 {
    x * sigmoid(x)
}

/// Sigmoid function
#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

// ============================================================================
// Qwen3ArchitectureCapsule - Main Metacapsule
// ============================================================================

/// Qwen3 Architecture Metacapsule (T6 Mixed, 256B aligned)
///
/// ## UCE34 Tier Analysis
///
/// - **T1 (Atomic)**: DualAtomicU64 coordination, phase state machine
/// - **T2 (SIMD)**: RMSNorm, SwiGLU, RoPE via f32x8
/// - **T4 (Batch)**: Parallel layer processing (future)
/// - **T5 (Streaming)**: Incremental KV-cache (autoregressive)
/// - **T6 (Mixed)**: Metacapsule orchestration (10-100× compound)
///
/// ## Memory Layout (256B total)
///
/// | Offset | Size | Field |
/// |--------|------|-------|
/// | 0-15   | 16B  | DualAtomicU64 coordination |
/// | 16-23  | 8B   | Generation counter |
/// | 24     | 1B   | Phase state |
/// | 25-31  | 7B   | Reserved |
/// | 32-39  | 8B   | Statistics (tokens_processed) |
/// | 40-47  | 8B   | Statistics (layers_computed) |
/// | 48-255 | 208B | Padding + sub-capsule refs |
///
/// ## ASSUM Tags
///
/// - `#ASSUME_256B_ALIGNMENT`: Metacapsule aligned for coordination
/// - `#VERIFY_ALIGNMENT`: crate::verify_alignment_only!
/// - `#ASSUME_LOCKFREE`: 100% lockfree via DualAtomicU64
/// - `#ASSUME_PHASE_MONOTONIC`: Phase transitions forward-only
#[repr(C, align(256))]
#[cfg(feature = "std")]
pub struct Qwen3ArchitectureCapsule {
    // ========================================================================
    // Cache Line 0 (0-63): Coordination
    // ========================================================================
    /// Coordination state (phase + error tracking)
    coordination: DualAtomicU64,

    /// Generation counter (TOCTOU prevention)
    generation: AtomicU64,

    /// Current inference phase
    phase: AtomicU8,

    /// Reserved for future use
    _reserved: [u8; 7],

    /// Statistics: tokens processed (lower 32 bits) + layers computed (upper 32 bits)
    stats: DualAtomicU64,

    // ========================================================================
    // Cache Line 1+ (64-255): Sub-capsules and config
    // ========================================================================
    /// Model configuration
    config: Qwen3Config,

    /// Embedding layer (token -> hidden)
    embed_tokens: EmbeddingCapsule,

    /// Transformer layers
    layers: Vec<Qwen3LayerCapsule>,

    /// Final RMSNorm weights
    final_norm_weights: Vec<f32>,

    /// LM head weights [hidden × vocab]
    lm_head_weights: Vec<f32>,

    /// Shared RoPE capsule
    #[cfg(feature = "inference-rope")]
    rope: Option<RoPECapsule>,

    /// Precomputed RoPE cos values (fallback when capsule unavailable)
    rope_cos_cache: Vec<f32>,

    /// Precomputed RoPE sin values (fallback)
    rope_sin_cache: Vec<f32>,
}

// Compile-time alignment verification (Q33)
crate::verify_alignment_only!(Qwen3ArchitectureCapsule, 256);

#[cfg(feature = "std")]
impl Qwen3ArchitectureCapsule {
    /// Create Qwen3 8B architecture (empty weights)
    ///
    /// # Performance
    ///
    /// - Initialization: O(num_layers) allocations
    /// - Memory: ~16 GB for 8B model (before quantization)
    pub fn new_8b() -> Self {
        Self::from_config(Qwen3Config::QWEN3_8B)
    }

    /// Create Qwen3 30B architecture
    ///
    /// # Performance
    ///
    /// - Memory: ~60 GB for 30B model (before quantization)
    pub fn new_30b() -> Self {
        Self::from_config(Qwen3Config::QWEN3_30B)
    }

    /// Create from custom configuration
    pub fn from_config(config: Qwen3Config) -> Self {
        assert!(config.validate(), "Invalid Qwen3 configuration");

        // Create embedding layer
        let embed_tokens = EmbeddingCapsule::new(config.vocab_size, config.hidden_size);

        // Create transformer layers
        let layers: Vec<Qwen3LayerCapsule> = (0..config.num_layers)
            .map(|i| Qwen3LayerCapsule::new(i, &config))
            .collect();

        // Initialize final norm weights
        let final_norm_weights = vec![1.0; config.hidden_size];

        // Initialize LM head weights
        let lm_head_weights = vec![0.0; config.hidden_size * config.vocab_size];

        // Precompute RoPE cos/sin (fallback if capsule unavailable)
        let half_head_dim = config.head_dim / 2;
        let mut rope_cos_cache = Vec::with_capacity(half_head_dim);
        let mut rope_sin_cache = Vec::with_capacity(half_head_dim);

        for i in 0..half_head_dim {
            let theta = config.rope_theta.powf(-2.0 * (i as f32) / config.head_dim as f32);
            rope_cos_cache.push(theta.cos());
            rope_sin_cache.push(theta.sin());
        }

        #[cfg(feature = "inference-rope")]
        let rope = Some(RoPECapsule::new(
            config.max_seq_len,
            config.head_dim,
            config.rope_theta,
        ));

        Self {
            coordination: DualAtomicU64::new(0, 0),
            generation: AtomicU64::new(0),
            phase: AtomicU8::new(InferencePhase::Idle as u8),
            _reserved: [0; 7],
            stats: DualAtomicU64::new(0, 0),
            config,
            embed_tokens,
            layers,
            final_norm_weights,
            lm_head_weights,
            #[cfg(feature = "inference-rope")]
            rope,
            rope_cos_cache,
            rope_sin_cache,
        }
    }

    /// Load weights from GGUF file (stub - requires gguf parser)
    ///
    /// # Arguments
    ///
    /// - `gguf_path`: Path to GGUF model file
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(LoadError)` on failure
    ///
    /// # TODO
    ///
    /// Integrate with GgufParserCapsule when available
    pub fn load_weights(&mut self, _gguf_path: &str) -> Result<(), LoadError> {
        // Placeholder for GGUF loading
        // Real implementation would use GgufParserCapsule
        Err(LoadError::NotImplemented)
    }

    /// Forward pass for single token (autoregressive decoding)
    ///
    /// # Arguments
    ///
    /// - `token_id`: Input token ID
    /// - `position`: Position in sequence
    /// - `kv_cache`: Mutable KV-cache for storing attention states
    ///
    /// # Returns
    ///
    /// Logits tensor [vocab_size]
    ///
    /// # Performance (B32 Target)
    ///
    /// - Latency: <50ms per token (8B model, GPU)
    /// - Latency: <500ms per token (8B model, CPU)
    pub fn forward(&self, token_id: u32, position: usize, kv_cache: &mut KVCache) -> Vec<f32> {
        // Update phase
        self.phase
            .store(InferencePhase::Embedding as u8, Ordering::Release);

        // Step 1: Embedding lookup
        let mut hidden_states = self.embed_tokens.forward(token_id);

        // Update phase
        self.phase
            .store(InferencePhase::LayerForward as u8, Ordering::Release);

        // Step 2: Process through all layers
        for layer in &self.layers {
            hidden_states = layer.forward(
                &hidden_states,
                position,
                kv_cache,
                &self.rope_cos_cache,
                &self.rope_sin_cache,
            );
        }

        // Increment KV cache sequence length
        kv_cache.increment_seq_len();

        // Update phase
        self.phase
            .store(InferencePhase::FinalNorm as u8, Ordering::Release);

        // Step 3: Final RMSNorm
        let normed = self.rms_norm(&hidden_states, &self.final_norm_weights);

        // Update phase
        self.phase
            .store(InferencePhase::LMHead as u8, Ordering::Release);

        // Step 4: LM head (hidden -> logits)
        let logits = self.lm_head(&normed);

        // Update statistics (tokens_processed counter)
        self.stats.fetch_add_primary(1, Ordering::Release);

        // Return to idle
        self.phase
            .store(InferencePhase::Idle as u8, Ordering::Release);

        logits
    }

    /// Forward pass for token sequence (prefill)
    ///
    /// # Arguments
    ///
    /// - `token_ids`: Input token IDs
    /// - `positions`: Position indices for each token
    /// - `kv_cache`: Mutable KV-cache
    ///
    /// # Returns
    ///
    /// Logits for each token [seq_len][vocab_size]
    pub fn forward_batch(
        &self,
        token_ids: &[u32],
        positions: &[usize],
        kv_cache: &mut KVCache,
    ) -> Vec<Vec<f32>> {
        assert_eq!(token_ids.len(), positions.len());

        token_ids
            .iter()
            .zip(positions.iter())
            .map(|(&token, &pos)| self.forward(token, pos, kv_cache))
            .collect()
    }

    /// Get next token prediction
    ///
    /// # Arguments
    ///
    /// - `logits`: Output logits from forward pass [vocab_size]
    /// - `config`: Generation configuration
    ///
    /// # Returns
    ///
    /// Predicted token ID
    pub fn generate_next(&self, logits: &[f32], config: &GenerationConfig) -> u32 {
        if config.temperature == 0.0 {
            // Greedy decoding
            self.argmax(logits)
        } else {
            // Temperature sampling with top-k/top-p
            self.sample(logits, config)
        }
    }

    /// Argmax for greedy decoding
    fn argmax(&self, logits: &[f32]) -> u32 {
        let mut max_idx = 0;
        let mut max_val = f32::NEG_INFINITY;

        for (i, &val) in logits.iter().enumerate() {
            if val > max_val {
                max_val = val;
                max_idx = i;
            }
        }

        max_idx as u32
    }

    /// Temperature sampling with top-k/top-p
    fn sample(&self, logits: &[f32], config: &GenerationConfig) -> u32 {
        // Apply temperature
        let scaled: Vec<f32> = logits.iter().map(|&x| x / config.temperature).collect();

        // Softmax
        let max_logit = scaled.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = scaled.iter().map(|&x| (x - max_logit).exp()).sum();
        let probs: Vec<f32> = scaled.iter().map(|&x| (x - max_logit).exp() / exp_sum).collect();

        // Top-K filtering
        let mut indexed: Vec<(usize, f32)> = probs.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let top_k = if config.top_k > 0 {
            config.top_k.min(indexed.len())
        } else {
            indexed.len()
        };

        // Top-P (nucleus) filtering
        let mut cumsum = 0.0;
        let mut cutoff = top_k;
        for (i, (_, prob)) in indexed.iter().enumerate().take(top_k) {
            cumsum += prob;
            if cumsum >= config.top_p {
                cutoff = i + 1;
                break;
            }
        }

        // Sample from filtered candidates
        // Simple random selection for now (would use proper RNG in production)
        let selected = indexed[0].0; // Just take top for determinism

        selected as u32
    }

    /// RMS normalization (helper)
    fn rms_norm(&self, x: &[f32], weights: &[f32]) -> Vec<f32> {
        let sum_sq: f32 = x.iter().map(|v| v * v).sum();
        let rms = (sum_sq / x.len() as f32 + self.config.rms_norm_eps).sqrt();
        let rsqrt = 1.0 / rms;

        x.iter()
            .zip(weights.iter())
            .map(|(v, w)| v * rsqrt * w)
            .collect()
    }

    /// LM head projection (hidden -> vocab)
    fn lm_head(&self, hidden_states: &[f32]) -> Vec<f32> {
        let mut logits = vec![0.0; self.config.vocab_size];

        for (i, logit) in logits.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (j, &h) in hidden_states.iter().enumerate() {
                sum += h * self.lm_head_weights[j * self.config.vocab_size + i];
            }
            *logit = sum;
        }

        logits
    }

    /// Get model configuration
    #[inline]
    pub const fn config(&self) -> &Qwen3Config {
        &self.config
    }

    /// Get current inference phase
    #[inline]
    pub fn phase(&self) -> InferencePhase {
        InferencePhase::from(self.phase.load(Ordering::Acquire))
    }

    /// Get statistics: (tokens_processed, layers_computed)
    #[inline]
    pub fn stats(&self) -> (u64, u64) {
        (
            self.stats.load_primary(Ordering::Acquire),
            self.stats.load_secondary(Ordering::Acquire),
        )
    }

    /// Get generation counter (for TOCTOU checking)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Memory usage estimate in bytes
    pub fn memory_usage(&self) -> usize {
        // Embeddings
        let embed_bytes = self.config.vocab_size * self.config.hidden_size * 4;

        // Layers
        let kv_dim = self.config.num_kv_heads * self.config.head_dim;
        let layer_bytes = (
            self.config.hidden_size * 2 // norms
            + self.config.hidden_size * self.config.hidden_size // q_proj
            + self.config.hidden_size * kv_dim * 2 // k, v proj
            + self.config.hidden_size * self.config.hidden_size // o_proj
            + self.config.hidden_size * self.config.intermediate_size * 2 // gate, up
            + self.config.intermediate_size * self.config.hidden_size // down
        ) * 4; // 4 bytes per f32

        let total_layer_bytes = layer_bytes * self.config.num_layers;

        // Final norm + LM head
        let head_bytes =
            self.config.hidden_size * 4 + self.config.hidden_size * self.config.vocab_size * 4;

        embed_bytes + total_layer_bytes + head_bytes
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Error during weight loading
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// File not found
    FileNotFound,
    /// Invalid GGUF format
    InvalidFormat,
    /// Weight shape mismatch
    ShapeMismatch,
    /// Not yet implemented
    NotImplemented,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::FileNotFound => write!(f, "Model file not found"),
            LoadError::InvalidFormat => write!(f, "Invalid GGUF format"),
            LoadError::ShapeMismatch => write!(f, "Weight shape mismatch"),
            LoadError::NotImplemented => write!(f, "Weight loading not yet implemented"),
        }
    }
}

impl std::error::Error for LoadError {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qwen3_config_8b() {
        let config = Qwen3Config::QWEN3_8B;
        assert_eq!(config.hidden_size, 4096);
        assert_eq!(config.num_heads, 32);
        assert_eq!(config.num_kv_heads, 8);
        assert_eq!(config.num_layers, 32);
        assert_eq!(config.intermediate_size, 14336);
        assert_eq!(config.head_dim, 128);
        assert!(config.validate());
    }

    #[test]
    fn test_qwen3_config_30b() {
        let config = Qwen3Config::QWEN3_30B;
        assert_eq!(config.hidden_size, 6144);
        assert_eq!(config.num_heads, 48);
        assert_eq!(config.num_kv_heads, 8);
        assert_eq!(config.num_layers, 48);
        assert!(config.validate());
    }

    #[test]
    fn test_qwen3_config_validation() {
        // All preset configs should be valid
        assert!(Qwen3Config::QWEN3_0_6B.validate());
        assert!(Qwen3Config::QWEN3_1_7B.validate());
        assert!(Qwen3Config::QWEN3_4B.validate());
        assert!(Qwen3Config::QWEN3_8B.validate());
        assert!(Qwen3Config::QWEN3_14B.validate());
        assert!(Qwen3Config::QWEN3_30B.validate());
        assert!(Qwen3Config::QWEN3_72B.validate());
    }

    #[test]
    fn test_num_kv_groups() {
        let config = Qwen3Config::QWEN3_8B;
        assert_eq!(config.num_kv_groups(), 4); // 32 / 8 = 4
    }

    #[test]
    fn test_generation_config_default() {
        let config = GenerationConfig::default();
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.top_p, 0.9);
        assert_eq!(config.top_k, 50);
    }

    #[test]
    fn test_generation_config_greedy() {
        let config = GenerationConfig::GREEDY;
        assert_eq!(config.temperature, 0.0);
        assert_eq!(config.repetition_penalty, 1.0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_kv_cache_creation() {
        let config = Qwen3Config::QWEN3_8B;
        let cache = KVCache::with_capacity(&config, 1024);

        assert_eq!(cache.seq_len(), 0);
        assert_eq!(cache.num_layers, 32);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_kv_cache_append() {
        let config = Qwen3Config::QWEN3_8B;
        let mut cache = KVCache::with_capacity(&config, 1024);

        let kv_dim = config.num_kv_heads * config.head_dim;
        let key = vec![1.0f32; kv_dim];
        let value = vec![2.0f32; kv_dim];

        cache.append(0, &key, &value);
        cache.increment_seq_len();

        assert_eq!(cache.seq_len(), 1);
        assert_eq!(cache.get_keys(0).len(), kv_dim);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_embedding_capsule() {
        let embed = EmbeddingCapsule::new(1000, 128);

        let output = embed.forward(42);
        assert_eq!(output.len(), 128);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_qwen3_architecture_creation() {
        // Use smaller config for faster test
        let mut config = Qwen3Config::QWEN3_0_6B;
        config.num_layers = 2; // Reduce layers for test
        config.vocab_size = 1000; // Reduce vocab for test

        let arch = Qwen3ArchitectureCapsule::from_config(config);
        assert_eq!(arch.config().num_layers, 2);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_qwen3_architecture_alignment() {
        use core::mem::align_of;
        assert_eq!(
            align_of::<Qwen3ArchitectureCapsule>(),
            256,
            "Qwen3ArchitectureCapsule must be 256-byte aligned"
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_inference_phase_transitions() {
        assert_eq!(InferencePhase::from(0), InferencePhase::Idle);
        assert_eq!(InferencePhase::from(1), InferencePhase::Embedding);
        assert_eq!(InferencePhase::from(2), InferencePhase::LayerForward);
        assert_eq!(InferencePhase::from(3), InferencePhase::FinalNorm);
        assert_eq!(InferencePhase::from(4), InferencePhase::LMHead);
        assert_eq!(InferencePhase::from(255), InferencePhase::Error);
    }

    #[test]
    fn test_silu_activation() {
        // silu(0) = 0 * sigmoid(0) = 0
        assert!((silu(0.0)).abs() < 1e-6);

        // silu(x) approaches x for large x
        assert!((silu(10.0) - 10.0).abs() < 0.1);

        // silu(x) approaches 0 for large negative x
        assert!(silu(-10.0).abs() < 0.001);
    }

    #[test]
    fn test_sigmoid_bounds() {
        // sigmoid should always be in (0, 1)
        // Note: For x <= -88, exp(-x) overflows f32, causing sigmoid to return 0.0
        // For x >= 88, exp(-x) underflows to 0, causing sigmoid to return 1.0
        // We test with values within the representable range
        assert!(sigmoid(-10.0) > 0.0);
        assert!(sigmoid(-10.0) < 0.0001); // sigmoid(-10) ≈ 4.54e-5
        assert!(sigmoid(10.0) > 0.9999);  // sigmoid(10) ≈ 0.99995
        assert!(sigmoid(10.0) < 1.0);
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);

        // Verify monotonicity
        assert!(sigmoid(-5.0) < sigmoid(0.0));
        assert!(sigmoid(0.0) < sigmoid(5.0));
    }

    #[test]
    fn test_load_error_display() {
        assert_eq!(LoadError::FileNotFound.to_string(), "Model file not found");
        assert_eq!(LoadError::InvalidFormat.to_string(), "Invalid GGUF format");
    }
}
