//! LRU Cache System for AI Request/Response Deduplication (UCE34 Q10-Q12)
//!
//! **Tier Selection**: Tier 6 Mixed (T1 Atomic + T4 Batch)
//! **Target Performance**: <100ns cache hit, 90%+ hit rate, 10-100× speedup
//! **Architecture**: 100% lockfree with generation counters for TOCTOU prevention
//!
//! # UCE34 Q1-Q9: Meta-Cognitive Analysis
//!
//! **Q1 (Scope)**: AI request/response caching to eliminate redundant API calls
//! **Q2 (Assumptions)**: Requests are deterministic (same input → same output)
//! **Q3 (Constraints)**: <100ns cache hit, memory bounded (10K entries = 1.28MB)
//! **Q4 (Context)**: Integrated with clapi_core proxy (Phase 3)
//! **Q5 (Success)**: 90%+ hit rate, <100ns lookup, 10-100× cost savings
//! **Q6 (Failure)**: Hash collisions, TTL expiration, memory exhaustion
//! **Q7 (Patterns)**: LRU eviction, generation counters, cache-aligned capsules
//! **Q8 (Alternatives)**: LFU, ARC, 2Q (rejected: LRU simpler, proven effective)
//! **Q9 (Trade-offs)**: Optimizing for hit rate (90%+) over memory efficiency
//!
//! # UCE34 Q10-Q12: Foundation (Computational Capsule Architecture)
//!
//! **Q10 (Capsule Tier)**: Tier 6 Mixed (Atomic coordination + Batch processing)
//!   - **Tier 1 (Atomic)**: Lockfree CacheKeyCapsule with generation counters
//!   - **Tier 4 (Batch)**: Batch eviction for 10K+ entries
//!   - **Compound Speedup**: 3-10× (Atomic) × 10-100× (Batch) = 30-1000× potential
//!
//! **Q11 (Rust Transform)**: AtomicU64 for all fields, #[repr(C, align(128))]
//! **Q12 (Nightly Enhancement)**: portable_simd for batch hash computation (optional)
//!
//! # UCE34 Q13-Q34: Implementation Details
//!
//! See inline documentation for domain analysis (Q13-Q21), implementation (Q22-Q30),
//! and refinement (Q31-Q34).

pub mod capsule;
pub mod llm_adapter;
pub mod lru;
pub mod predictive_prefetch;

// Phase 2: Semantic Cache - L0 Fuzzy Layer with LSH + MinHash
#[cfg(feature = "semantic-cache")]
pub mod semantic_adapter;

// DEPRECATED: persistent_l2 removed - use atomic_capsule::persistence::PersistentMap instead
// #[cfg(feature = "mmap-persistence")]
// pub mod persistent_l2;

pub use capsule::{CacheKeyCapsule, CacheEntry};
pub use llm_adapter::{
    DefaultLlmCacheAdapter, LlmCacheAdapter, LlmCacheKeyCapsule, LlmCachePolicyCapsule,
    LlmCacheStatsCapsule,
};
pub use lru::{LruCache, CacheConfig, CacheStats};
pub use predictive_prefetch::{PredictivePrefetchCache, PrefetchStatsSnapshot};

// Re-export semantic cache types (when feature enabled)
#[cfg(feature = "semantic-cache")]
pub use semantic_adapter::{
    AccuracyTrackerCapsule,
    SemanticCacheAdapter,
    SemanticCacheMetadataCapsule,
    ThresholdConfigCapsule,
};

// DEPRECATED: Use atomic_capsule::persistence::PersistentMap instead
// #[cfg(feature = "mmap-persistence")]
// pub use persistent_l2::{PersistentL2Cache, CacheStats as L2CacheStats, L2CacheError};

#[cfg(test)]
mod tests;

/// Cache error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum CacheError {
    /// Cache entry not found
    #[error("Cache miss: entry not found for hash {0:016x}")]
    CacheMiss(u64),

    /// Cache is full (all entries occupied)
    #[error("Cache full: {current}/{max} entries occupied")]
    CacheFull { current: usize, max: usize },

    /// TTL expired
    #[error("TTL expired: entry {hash:016x} expired at {expired_ns}ns")]
    TtlExpired { hash: u64, expired_ns: u64 },

    /// Generation mismatch (TOCTOU detected)
    #[error("Generation mismatch: expected {expected}, found {found}")]
    GenerationMismatch { expected: u64, found: u64 },

    /// Invalid hash (zero hash reserved for empty slots)
    #[error("Invalid hash: zero hash is reserved")]
    InvalidHash,
}

pub type Result<T> = std::result::Result<T, CacheError>;
