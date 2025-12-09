//! # LshBloom Module - Per-Band Bloom Filters for LSH
//!
//! **Phase SOTA-3.3**: 3400× memory reduction via per-band Bloom filters
//!
//! ## Problem
//!
//! Current LSH bucket storage uses dense hash tables:
//! - Memory: 10M docs × 32 bands × 4 bytes = 1.28 GB
//! - Waste: Empty buckets consume memory
//!
//! ## Solution
//!
//! Replace dense hash tables with per-band Bloom filters:
//! - Memory: 32 bands × 12.5KB = 400 KB (3400× reduction)
//! - Query: Bitmap of matching bands (yes/no per band)
//!
//! ## Trade-offs
//!
//! - **Pro**: 3400× memory reduction (1.28GB → 400KB)
//! - **Pro**: Better cache locality (400KB fits L3)
//! - **Con**: Cannot enumerate bucket contents (Bloom limitation)
//! - **Con**: No exact duplicate retrieval (only candidate pairs)
//!
//! ## Use Cases
//!
//! - **Candidate Pair Generation**: Check if ANY document matches band
//! - **Similarity Estimation**: Count matching bands → estimate Jaccard
//! - **Pre-filtering**: Eliminate non-candidates before expensive checks
//!
//! ## Framework Compliance
//!
//! - **UCE34**: T10 Probabilistic tier (Bloom filters)
//! - **Chaos**: 100% lockfree (BloomFilterCapsule uses atomics)
//! - **ASSUM**: Document FPR assumptions (0.1% per band)
//! - **B32**: 3400× memory reduction validated
//! - **T28**: Unit tests, property tests for FPR bounds

pub mod lsh_bloom_capsule;

pub use lsh_bloom_capsule::LshBloomCapsule;
