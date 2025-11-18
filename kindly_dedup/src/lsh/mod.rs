//! LSH (Locality-Sensitive Hashing) optimizations for deduplication
//!
//! Week 2 P1 Optimization: Batch LSH Lookup
//! - Target: 1.3-2× dedup speedup via amortized LSH bucket lookups
//! - Tier: T4 Batch (1000-doc batches, Vec pooling, cache optimization)
//! - Integration: Zero breaking changes, feature-gated
//!
//! v1.11 Optimization: Adaptive LSH Parameters
//! - Target: 3× dedup speedup via scaled bucket count (10M docs)
//! - Problem: Fixed 5×25 creates only 64K buckets → 781 docs/bucket → 39B operations
//! - Solution: Scale to 12×10 = 244K buckets → 200 docs/bucket → 3× speedup

pub mod adaptive_params;

#[cfg(feature = "batch-lsh")]
pub mod batch_lookup;

// Export adaptive params (used by parallel_pipeline)
pub use adaptive_params::{compute_docs_per_bucket, compute_lsh_params, compute_recall, estimate_unique_buckets};

#[cfg(feature = "batch-lsh")]
pub use batch_lookup::{BatchLSHLookup, BucketKey, DocId, DEFAULT_BATCH_SIZE, NUM_BANDS, ROWS_PER_BAND};
