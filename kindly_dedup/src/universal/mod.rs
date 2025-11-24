//! # Universal Pipeline v3.0 - Zero-Copy Orchestration & Output
//!
//! High-performance zero-copy deduplication pipeline orchestration using
//! computational capsule architecture (T6 Mixed + T9 Persistent + T5 Streaming).
//!
//! ## Architecture
//!
//! **UniversalDedupPipeline (T6 Mixed)** orchestrates 5 mmap-backed capsules:
//! 1. **MmapCorpusReaderCapsule** (T9+T5): Zero-copy JSONL parsing, O(1) 5 MB memory
//! 2. **MmapSignatureCapsule** (T9+T2): SIMD MinHash, O(1) 260 KB memory
//! 3. **MmapLshBucketCapsule** (T9+T10): LSH bucketing, O(1) 136 MB memory
//! 4. **MmapUnionFindCapsule** (T9+T10): Clustering, O(1) 80 MB memory
//! 5. **MmapOutputWriterCapsule** (T9): JSONL output, O(1) 1 MB memory
//!
//! **Total Memory**: 222 MB O(1) constant (independent of corpus size)
//!
//! ## Modules
//!
//! - `corpus_reader`: MmapCorpusReaderCapsule (T9+T5 - Zero-copy JSONL parsing)
//! - `signature_writer`: MmapSignatureCapsule (T9+T2 persistent SIMD MinHash)
//! - `lsh_bucket`: MmapLshBucketCapsule (T9+T10 - Zero-copy SSTable-based LSH)
//! - `union_find`: MmapUnionFindCapsule (T9+T10 - Zero-copy mmap-backed clustering)
//! - `output_writer`: MmapOutputWriterCapsule (T9 - Persistent JSONL output)
//! - `pipeline`: UniversalDedupPipeline (T6 Mixed - Full pipeline orchestration)
//! - `chunk_splitter`: ChunkSplitterCapsule (T5 Streaming - Zero-copy corpus splitting for job-level parallelism)

pub mod corpus_reader;
pub mod signature_writer;
pub mod lockfree_signature_writer;
pub mod lsh_bucket;
pub mod lockfree_lsh_bucket;
pub mod union_find;
pub mod parallel_union_find;
pub mod parallel_bucket_processor;
pub mod output_writer;
pub mod pipeline;
pub mod result_merger;
pub mod job_level_pipeline;
pub mod job_coordinator;
pub mod chunk_splitter;
#[cfg(feature = "parallel-dedup")]
pub mod parallel_dedup_v2;

/// MinHashSig - Zero-cost newtype wrapper for MinHash signatures
///
/// This wrapper enables `RingBufferCapsule<MinHashSig>` usage by implementing
/// the `RingBufferEntry` trait. It's a zero-cost abstraction with identical
/// layout to `[u16; 128]` thanks to `#[repr(transparent)]`.
///
/// **Performance**: Zero-cost (0ns overhead due to `#[repr(transparent)]`)
///
/// **ASSUM Safety**:
/// - #ASSUME_TRANSPARENT_LAYOUT: `#[repr(transparent)]` guarantees identical layout to `[u16; 128]`
/// - #ASSUME_COPY_TYPE: MinHashSig is Copy, safe for RingBufferCapsule
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MinHashSig(pub [u16; 128]);

impl MinHashSig {
    /// Create new signature from array
    #[inline]
    pub fn new(sig: [u16; 128]) -> Self {
        MinHashSig(sig)
    }

    /// Get inner array reference
    #[inline]
    pub fn as_array(&self) -> &[u16; 128] {
        &self.0
    }

    /// Get inner array as mutable reference
    #[inline]
    pub fn as_array_mut(&mut self) -> &mut [u16; 128] {
        &mut self.0
    }
}

// RingBufferEntry implementation for MinHashSig
impl atomic_capsule::collections::RingBufferEntry for MinHashSig {
    #[inline]
    fn empty() -> Self {
        MinHashSig([0; 128])
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.0.iter().all(|&x| x == 0)
    }
}

// Backward compatibility shims
impl From<[u16; 128]> for MinHashSig {
    #[inline]
    fn from(sig: [u16; 128]) -> Self {
        MinHashSig(sig)
    }
}

impl From<MinHashSig> for [u16; 128] {
    #[inline]
    fn from(sig: MinHashSig) -> Self {
        sig.0
    }
}

impl AsRef<[u16; 128]> for MinHashSig {
    #[inline]
    fn as_ref(&self) -> &[u16; 128] {
        &self.0
    }
}

impl std::ops::Deref for MinHashSig {
    type Target = [u16; 128];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for MinHashSig {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// Re-export all public types from capsules
pub use corpus_reader::{
    CorpusReaderError, CorpusReaderResult, Document, MmapCorpusReaderCapsule,
};
pub use signature_writer::{
    MmapSignatureCapsule, MmapSignatureError, MinHashSignature,
};
pub use lockfree_signature_writer::{
    LockfreeMmapSignatureCapsule, SignatureError, SignatureResult,
};
pub use lsh_bucket::{
    BandHash, MmapLshBucketCapsule, MmapLshError, Result as LshResult,
};
pub use lockfree_lsh_bucket::{
    LockfreeMmapLshBucketCapsule, LshError, LshStats,
};
pub use union_find::{
    DocId, MmapUnionFindCapsule, UnionFindError,
};
pub use output_writer::{
    MmapOutputWriterCapsule, OutputError, OutputResult,
};

// Re-export orchestrator types
pub use pipeline::{
    UniversalDedupPipeline, UniversalPipelineError, Phase, PipelineProgress,
};
pub use result_merger::{
    ResultMergerCapsule, ResultMergerError, MergerStats, Cluster,
};
pub use parallel_union_find::{
    ParallelUnionFindCapsule, ParallelUFError,
};
pub use parallel_bucket_processor::{
    ParallelBucketProcessorCapsule, BucketId, BucketProcessResult,
};

// Re-export ParallelDedupPipelineV2 types (T6 Mixed Tier)
#[cfg(feature = "parallel-dedup")]
pub use parallel_dedup_v2::{
    ParallelDedupPipelineV2MetaCapsule,
    ParallelDedupV2Config,
    Phase as DedupPhaseV2,
    PipelineStats,
    PipelineError as DedupPipelineError,
};

// Re-export ChunkSplitterCapsule types (T5 Streaming Zero-Copy Splitting)
pub use chunk_splitter::{
    ChunkSplitterCapsule,
    ChunkDescriptor,
    ChunkSplitterStats,
};

// Re-export JobLevelDedupPipelineMetaCapsule types (T6 Mixed Job-Level Parallelism)
pub use job_level_pipeline::{
    JobLevelDedupPipelineMetaCapsule,
    Phase as JobPhase,
};

// Re-export JobCoordinatorCapsule types (T1+T4 Parallel Job Coordination)
pub use job_coordinator::{
    JobCoordinatorCapsule,
    JobResult,
    CoordinatorStats,
    Phase as CoordinatorPhase,
};
