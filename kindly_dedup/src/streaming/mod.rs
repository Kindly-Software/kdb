//! T5 Streaming primitives for Phase 3 refactoring
//!
//! This module implements the Stage 1 (Document Stream) component of the 3-stage pipeline.
//!
//! ## Architecture
//!
//! **Stage 1: DocumentStreamCapsule (T5 Streaming)**
//! - Zero-copy mmap-based JSONL streaming
//! - Outputs Arc<str> for efficient sharing across Stage 2 workers
//! - O(1) memory (constant <200 MB resident)
//! - Target: ~436K docs/sec throughput
//!
//! ## Module Structure
//!
//! - `document_stream` - DocumentStreamCapsule implementation
//!
//! ## Example
//!
//! ```rust,ignore
//! use kindly_dedup::streaming::DocumentStreamCapsule;
//!
//! // Create stream from corpus path
//! let stream = DocumentStreamCapsule::new("corpus.jsonl", 0, 10_000_000)?;
//!
//! // Stream documents (zero-copy, Arc<str> for sharing)
//! while let Some((doc_id, text)) = stream.next_document()? {
//!     // text is Arc<str> - can be sent to multiple Stage 2 workers
//!     send_to_worker(doc_id, text.clone());
//! }
//! ```

pub mod document_stream;
pub mod tokenizer;
pub mod minhash_builder;
pub mod bounded_token_queue;
pub mod mmap_lsh_bucket_capsule;

/// StreamingLshBucketerCapsule (Treiber Stack) - T5 + T1 Lockfree LSH bucketing
///
/// Agent 10 implementation: Lockfree LSH bucket insertions using Treiber stack pattern.
/// - Target: 1.3-1.5× speedup via contention elimination (50% → 5%)
/// - Architecture: 4 shards × Treiber stacks (LIFO bucket ordering)
/// - Performance: <100ns per band insertion, 500ns per document
pub mod lsh_bucketer_treiber;

pub use document_stream::DocumentStreamCapsule;
pub use tokenizer::{StreamingTokenizerCapsule, TokenBatch};
pub use minhash_builder::StreamingMinHashBuilderCapsule;
pub use lsh_bucketer_treiber::StreamingLshBucketerTreiber;

// Comprehensive T28 test suite (28 tests: unit/property/integration/production)
#[cfg(test)]
mod tests;
