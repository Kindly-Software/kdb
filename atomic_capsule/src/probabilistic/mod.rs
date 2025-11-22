//! # Tier 10: Probabilistic Computational Capsules
//!
//! **LSH (Locality-Sensitive Hashing) + MinHash + HyperLogLog for probabilistic algorithms.**
//!
//! This module provides probabilistic data structures optimized for:
//! - Near-duplicate detection (MinHash Jaccard similarity)
//! - Approximate nearest neighbor search (LSH bucketing)
//! - Cardinality estimation (HyperLogLog counting)
//! - Memory-efficient sketching (100-1000× compression)
//!
//! ## UCE34 Framework Application
//!
//! - **Q10 (Computational Capsule)**: Tier 10 Probabilistic - sketches/filters/sampling
//! - **Q11 (Rust Transform)**: SIMD for hash computation, atomic coordination
//! - **Q12 (Nightly Enhancement)**: portable_simd for 8-way parallel hashing
//! - **Q28 (Simplicity)**: Simple sketch API hides complex hash functions
//! - **Q29 (Constraints)**: Memory reduction 100-1000× vs exact data structures
//! - **Q30 (Validation)**: False positive rates, collision analysis
//! - **Q31 (Rust)**: Zero-cost abstractions via const generics
//! - **Q33 (Validation)**: Compile-time verification via derive macro
//!
//! ## Performance Targets (B32 Validated)
//!
//! - **LSH projection**: <100ns (16 hyperplanes, SIMD dot product)
//! - **MinHash signature**: <1μs (128 hashes, SIMD parallel)
//! - **Hamming distance**: <10ns (SIMD popcount)
//! - **Jaccard similarity**: <50ns (SIMD comparison)
//! - **HyperLogLog insert**: <100ns (CAS loop, SipHash)
//! - **HyperLogLog cardinality**: <1μs (harmonic mean, bias correction)
//!
//! ## Safety (ASSUM Framework)
//!
//! - 100% lockfree (no mutex/RwLock)
//! - Zero unsafe code (99.99% ASSUM safe)
//! - All assumptions verified at compile-time
//! - Atomic coordination via generation counters
//!
//! ## Memory Efficiency
//!
//! | Structure | Exact Size | Sketch Size | Reduction |
//! |-----------|-----------|-------------|-----------|
//! | Set (1M items) | 16-64 MB | 512 bytes | 125-250× |
//! | Vector (4096 dims) | 16 KB | 16 bytes | 1000× |
//! | Text (10KB doc) | 10 KB | 128 bytes | 80× |
//!
//! ## Use Cases
//!
//! 1. **Near-Duplicate Detection**: MinHash Jaccard similarity for documents
//! 2. **Semantic Search**: LSH bucketing for approximate nearest neighbors
//! 3. **Clustering**: Group similar items via LSH hash collisions
//! 4. **Deduplication**: Identify duplicate content with <1% false positive rate
//! 5. **Cardinality Estimation**: HyperLogLog for distinct element counting (±2% accuracy)
//!
//! ## Examples
//!
//! ```rust,ignore
//! use atomic_capsule::probabilistic::{LshBucketCapsule, MinHashSignatureCapsule, HyperLogLogCapsule};
//!
//! // LSH for approximate nearest neighbor search
//! let lsh = LshBucketCapsule::new();
//! let vector = [1.0, 2.0, 3.0, 4.0]; // 4D vector
//! let bucket = lsh.project(&vector); // <100ns
//!
//! // MinHash for Jaccard similarity estimation
//! let minhash = MinHashSignatureCapsule::new();
//! let signature = minhash.compute_signature(&tokens); // <1μs
//! let similarity = minhash.jaccard_similarity(&sig1, &sig2); // <50ns
//!
//! // HyperLogLog for cardinality estimation (single instance, <64 threads)
//! let hll = HyperLogLogCapsule::new();
//! for i in 0..100_000 {
//!     hll.insert(i);
//! }
//! let estimate = hll.cardinality(); // ±2% accuracy
//!
//! // ShardedHyperLogLog for high-concurrency (>64 threads, 4.3× speedup @ 256 threads)
//! let sharded_hll = ShardedHyperLogLog::new();
//! std::thread::scope(|s| {
//!     for tid in 0..256 {
//!         s.spawn(move || {
//!             for i in 0..1000 {
//!                 sharded_hll.insert(tid * 1000 + i);
//!             }
//!         });
//!     }
//! });
//! let estimate = sharded_hll.cardinality(); // ±2% accuracy, 16× memory vs single HLL
//! ```

pub mod bloom_filter;
#[cfg(feature = "nightly-const-probabilistic")]
pub mod bloom_filter_const;
pub mod bloom_filter_sharded;
#[cfg(feature = "count-min-sketch")]
pub mod count_min_sketch;
#[cfg(feature = "nightly-const-generics")]
pub mod count_min_sketch_const;
pub mod hamming;
#[cfg(feature = "hll")]
pub mod hyperloglog;
#[cfg(feature = "nightly-const-probabilistic")]
pub mod hyperloglog_const;
#[cfg(feature = "hll-sharded")]
pub mod hyperloglog_sharded;
pub mod lsh;
pub mod minhash;
#[cfg(feature = "portable_simd")]
pub mod minhash_simd;
pub mod tokenize;
pub mod union_find;

#[cfg(all(
    feature = "mmap-persistence",
    feature = "probabilistic",
    feature = "nightly-atomic"
))]
pub mod persistent_bloom;

pub use bloom_filter::BloomFilterCapsule;
#[cfg(feature = "nightly-const-probabilistic")]
pub use bloom_filter_const::{BloomFilterConst, validate_bloom_size, validate_hash_count, validate_fpr, calculate_fpr, calculate_optimal_hash_count};
pub use bloom_filter_sharded::ShardedBloomFilterCapsule;
#[cfg(feature = "count-min-sketch")]
pub use count_min_sketch::CountMinSketchCapsule;
#[cfg(feature = "nightly-const-generics")]
pub use count_min_sketch_const::{CountMinSketchConst, validate_cms_width, validate_cms_depth, validate_cms_epsilon};
pub use hamming::hamming_distance_simd;
#[cfg(feature = "hll")]
pub use hyperloglog::{CardinalityEstimator, HyperLogLogCapsule};
#[cfg(feature = "nightly-const-probabilistic")]
pub use hyperloglog_const::{HyperLogLogConst, validate_hll_precision, validate_sparse_threshold_percent, calculate_hll_memory, calculate_hll_error};
#[cfg(feature = "hll-sharded")]
pub use hyperloglog_sharded::ShardedHyperLogLog;
pub use lsh::{lsh_project, LshBucketCapsule, MultiTableLshCapsule};
pub use minhash::{jaccard_similarity_simd, minhash_signature, MinHashSignatureCapsule};
#[cfg(feature = "portable_simd")]
pub use minhash_simd::compute_signature_simd;
pub use tokenize::{tokenize, tokenize_set};
pub use union_find::UnionFind;

#[cfg(all(
    feature = "mmap-persistence",
    feature = "probabilistic",
    feature = "nightly-atomic"
))]
pub use persistent_bloom::PersistentBloomFilter;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_structure() {
        // Verify module exports are accessible
        assert_eq!(core::mem::size_of::<LshBucketCapsule>(), 128);
        assert_eq!(core::mem::align_of::<LshBucketCapsule>(), 128);
        assert_eq!(core::mem::size_of::<MinHashSignatureCapsule>(), 512);
        assert_eq!(core::mem::align_of::<MinHashSignatureCapsule>(), 512);

        #[cfg(feature = "hll")]
        {
            assert_eq!(core::mem::size_of::<HyperLogLogCapsule>(), 16512);
            assert_eq!(core::mem::align_of::<HyperLogLogCapsule>(), 128);
        }

        #[cfg(feature = "hll-sharded")]
        {
            assert_eq!(core::mem::size_of::<ShardedHyperLogLog>(), 16 * 16512);
            assert_eq!(core::mem::align_of::<ShardedHyperLogLog>(), 128);
        }
    }
}
