//! # Text Processing Capsules
//!
//! **SIMD-accelerated text hashing and batch processing primitives**
//!
//! This module provides computational capsules for high-performance text operations,
//! targeting 2-13× speedups via vectorization and batch processing.
//!
//! ## Modules
//!
//! - `simd_hasher`: SIMD vectorized token hashing (8-wide FNV-1a)
//! - `tokenization_batch`: Thread-local batch tokenization (zero allocator contention)
//!
//! ## Performance Targets
//!
//! - **SIMD Text Hashing**: 2-8× speedup (800ns per 8 tokens vs 4μs scalar)
//! - **Batch Tokenization**: 13× speedup (73M allocation reduction, zero contention)
//! - **Corpus Generation**: 14M docs/sec (vs 3.5M baseline, 4× improvement)
//!
//! ## Example
//!
//! ```rust,ignore
//! use atomic_capsule::text::{SimdTextHasher, TokenizationBatchCapsule};
//!
//! // SIMD hashing
//! let hasher = SimdTextHasher::new();
//! let hashes = hasher.hash_tokens_simd("machine learning neural network");
//! assert_eq!(hashes.len(), 4);
//!
//! // Batch tokenization
//! let tokenizer = TokenizationBatchCapsule::new();
//! let tokens = tokenizer.tokenize_deduplicated("machine learning");
//! ```

pub mod simd_hasher;

#[cfg(feature = "tokenization-batch")]
pub mod tokenization_batch;

pub use simd_hasher::SimdTextHasher;

#[cfg(feature = "tokenization-batch")]
pub use tokenization_batch::TokenizationBatchCapsule;
