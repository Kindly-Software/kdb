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
//!
//! Phase 3 Optimization: Batch LSH Index (T4 Batch + T9 Persistent)
//! - Target: 1.5× dedup speedup via batch insertions
//! - Architecture: 1000-doc batch buffer, two-phase commit, generation counter
//! - Reduces mmap fsync from 16,000/sec to 16/sec (1000× reduction)
//!
//! Phase LSH-BLOOM: LshBloom Backend Integration (4,885× memory reduction)
//! - Target: 262 KB vs 1.28 GB (memory-constrained deployments)
//! - Architecture: Pluggable backend trait (Hash Table vs Bloom Filter)
//! - Trade-offs: Bloom has no bucket enumeration (similarity estimation only)

pub mod adaptive_params;
pub mod backend;

#[cfg(feature = "batch-lsh")]
pub mod batch_lookup;

#[cfg(feature = "batch-lsh")]
pub mod batch_lsh_index;

#[cfg(feature = "batch-lsh")]
pub mod transaction_log;

#[cfg(feature = "persistent-dedup")]
pub mod mmap_bucketer;

// Export adaptive params (used by parallel_pipeline)
pub use adaptive_params::{compute_docs_per_bucket, compute_lsh_params, compute_recall, estimate_unique_buckets};

// Export LSH backend trait (NEW: pluggable storage)
pub use backend::{LshBackend, LshQueryResult};

#[cfg(feature = "batch-lsh")]
pub use batch_lookup::{BatchLSHLookup, BucketKey, DocId, DEFAULT_BATCH_SIZE, NUM_BANDS, ROWS_PER_BAND};

#[cfg(feature = "batch-lsh")]
pub use batch_lsh_index::{BatchLshIndexCapsule, BatchLshIndexError, BatchLshIndexResult, TransactionLogEntry};

#[cfg(feature = "batch-lsh")]
pub use transaction_log::{TransactionLogCapsule, TransactionLogError, LshEntry, TransactionBatch};

#[cfg(feature = "persistent-dedup")]
pub use mmap_bucketer::MmapLshBucketer;

// Phase SOTA-3.1: Sparse LSH Bucket Iteration (82× reduction via bitset)
pub mod atomic_bitset;
pub use atomic_bitset::AtomicBitSetCapsule;
