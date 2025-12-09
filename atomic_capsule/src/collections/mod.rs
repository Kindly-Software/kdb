//! # Collections Module - Lockfree Concurrent Data Structures
//!
//! **UCE34 Tier 1/4 concurrent collections with computational capsule architecture.**
//!
//! ## Migrating from DashMap or atomic_capsule_map?
//!
//! **Welcome!** This module is the recommended replacement for:
//! - **DashMap** → Use [`ConcurrentMapCapsule`] (3-10× faster, 100% lockfree)
//! - **atomic_capsule_map** → Use [`ConcurrentMapCapsule`] (deprecated crate, see [Migration Guide](https://github.com/yourusername/atomic_capsule/blob/main/docs/DASHMAP_MIGRATION_GUIDE.md))
//!
//! **Migration Time**: 1-4 hours | **Expected Speedup**: 3-59× (median: 10-20×)
//!
//! **See**: [DASHMAP_MIGRATION_GUIDE.md](https://github.com/yourusername/atomic_capsule/blob/main/docs/DASHMAP_MIGRATION_GUIDE.md) for step-by-step instructions and 7 migration patterns.
//!
//! ---
//!
//! ## Available Collections
//!
//! This module provides high-performance lockfree data structures:
//! - **LockfreeHashTable**: Lockfree hash table with chaining (T1+T4, 3-10× faster than RwLock<HashMap>)
//! - **StatsCapsule64**: Lockfree statistics collection (T1, 10-30× faster than Mutex<Stats>)
//! - **ConcurrentMapCapsule**: Generic lockfree hash map (T4, 3-10× faster than DashMap, **replacement for atomic_capsule_map**)
//! - **RingBufferBroadcast**: Lossless multi-consumer broadcast channel (T4, 2-5× faster than tokio::broadcast)
//! - **RingBufferCapsule**: Generic T5 streaming ring buffer (T5, <10ns append, 100% lockfree, for trace/debug/monitoring)
//! - **AppendOnlyMapCapsule**: Insert-heavy append-only map (T4, 10× insert, 100% correct)
//! - **AppendOnlyMapCapsuleOptimized**: IMPL-2 V3.1 optimized (T6, 7× SIMD + 5× batch + 100× binary)
//!
//! ## Performance Targets (B32 Framework)
//!
//! ### LockfreeHashTable (T1+T4 Hybrid)
//! - Get: <20ns (vs 50-200ns RwLock<HashMap>)
//! - Insert: <100ns (vs 200-500ns RwLock<HashMap>)
//! - Remove: <150ns (vs 300-600ns RwLock<HashMap>)
//! - Zero reader blocking (100% lockfree)
//!
//! ### StatsCapsule64 (T1 Atomic)
//! - Increment: <10ns (vs 100-500ns Mutex)
//! - Record latency: <15ns (vs 150-600ns Mutex)
//! - Get stats: <20ns (vs 200-800ns Mutex)
//!
//! ### ConcurrentMapCapsule (T4 Batch)
//! - Insert: <100ns (vs 200-400ns DashMap)
//! - Get: <50ns (vs 150-300ns DashMap)
//! - Remove: <150ns (vs 250-500ns DashMap)
//! - Concurrent throughput: 10M+ ops/sec
//!
//! ### RingBufferBroadcast (T4 Batch)
//! - Send: <200ns (vs 100ns tokio::broadcast but lossless)
//! - Recv: <100ns (vs 50ns tokio::broadcast)
//! - P99 latency: <500ns (vs 10-50μs tokio::broadcast with drops)
//! - Throughput: 5M+ msgs/sec
//! - Lossless guarantee (blocks sender when buffer full)
//!
//! ## Design Principles
//! - 100% lockfree (zero RwLock/Mutex)
//! - Chaining or linear probing for collision resolution
//! - Generation counters for TOCTOU prevention
//! - AtomicPtr for lockfree value storage
//! - Compile-time verification via #[derive(ComputationalCapsule)]

// Unified error types for all collections (Phase 2.1 - Error Handling)
pub mod error;

// Shared utilities for generation counter packing (T1 Atomic primitive)
pub mod generation_counter;

// BitwiseSerializable trait for type-safe atomic storage (Arc, String, primitives)
pub mod serializable;

pub mod append_only_map;
pub mod append_only_map_optimized;
pub mod concurrent_map;
pub mod concurrent_map_v2;
// pub mod concurrent_map_v3; // REMOVED: V3 reverted to V2 (commit 1e704c0)

// Specialized u64 map (15-30× speedup vs generic)
#[cfg(feature = "specialized-u64")]
pub mod concurrent_map_u64;

// mod concurrent_map_v3_tests; // REMOVED: V3 tests reverted with V3 module

pub mod entry;
pub mod lockfree_table;
pub mod ring_broadcast;
#[cfg(feature = "ring-trace")]
pub mod ring_trace;
pub mod stats_capsule;

// Robin Hood hash map (T1 Atomic, linear probing with robin hood heuristic)
pub mod robin_hood_hash;

// Temporarily commented out - requires tokio dependency
pub mod async_log;

// Cache module (Phase 3 E8 - Response caching)
pub mod cache;

// Cache integrated module (128B CacheSlot with all security features - I20 Migration)
pub mod cache_integrated;

// Cache HMAC integrity module (Q34 Auditability - SOX/SOC2/GDPR/HIPAA)
#[cfg(all(feature = "std", feature = "cache-hmac"))]
pub mod cache_hmac;

// Cache integration helpers (insert/get HMAC integration)
pub mod cache_integration_helpers;

// Cache encryption module (Optional AES-256-GCM for GDPR/HIPAA compliance)
#[cfg(feature = "cache-encryption")]
pub mod cache_encryption;

// Cache batch operations module (T4 Batch Tier)
pub mod cache_batch;

// Multi-tenant cache support (Phase 2.5 - Cryptographic namespace isolation)
#[cfg(feature = "cache")]
pub mod cache_multi_tenant;

// Queue module (Phase 1: Bounded SPSC/MPMC)
#[cfg(feature = "queue-bounded")]
pub mod queue;

// Synchronous flush task (Tokio removal - std::thread + lockfree queue)
#[cfg(all(feature = "std", feature = "queue-bounded"))]
pub mod sync_flush_task;

// Distributed cache module (Phase L3 - Enterprise Multi-Region Cache)
#[cfg(feature = "distributed")]
pub mod distributed_cache;

// Distributed cache compression (Phase P1.1 - zstd for large payloads)
#[cfg(feature = "distributed-compression")]
pub mod distributed_cache_compression;

// Distributed cache audit trail (Phase P1.3 - Q34 Auditability)
#[cfg(feature = "distributed-audit")]
pub mod distributed_cache_audit;

// Histogram (T6 Mixed: T1 Atomic + T4 Batch)
#[cfg(feature = "histogram")]
pub mod histogram;

// Histogram with const generics (T0+T1: Auditable + Atomic, 99.996% allocation speedup)
#[cfg(all(feature = "histogram", feature = "nightly-const-generics"))]
pub mod histogram_const;

// Streaming Stats (T5 Streaming: T-Digest percentiles)
#[cfg(feature = "streaming-stats")]
pub mod streaming_stats;

// Persistent LSH Table (T9 Persistent + T10 Probabilistic)
#[cfg(feature = "probabilistic")]
pub mod persistent_lsh;

// Persistent Deduplication Index (T9 Persistent + T10 Probabilistic)
#[cfg(feature = "probabilistic")]
pub mod persistent_dedup;

// Persistent MinHash Index (T9 Persistent + T10 Probabilistic) - SUBAGENT 5
#[cfg(all(
    feature = "mmap-persistence",
    feature = "nightly-atomic",
    feature = "probabilistic"
))]
pub mod persistent_minhash;

// Re-export for convenience
pub use cache::CacheSlot;

// Cache encryption exports (optional feature)
#[cfg(feature = "cache-encryption")]
pub use cache_encryption::{decrypt_value, encrypt_value, EncryptionError};

pub use append_only_map::AppendOnlyMapCapsule;
pub use append_only_map_optimized::AppendOnlyMapCapsuleOptimized;
pub use cache_batch::LockfreeCacheCapsule;
pub use concurrent_map::ConcurrentMapCapsule;
pub use concurrent_map_v2::ConcurrentMapCapsuleV2;
// pub use concurrent_map_v3::ConcurrentMapCapsule as ConcurrentMapCapsuleV3; // REMOVED: V3 reverted

// Specialized u64 map export (15-30× speedup)
#[cfg(feature = "specialized-u64")]
pub use concurrent_map_u64::ConcurrentMapU64;

pub use entry::{Entry, OccupiedEntry, VacantEntry};
pub use error::{CacheError, CacheResult, MapError, MapResult};
pub use lockfree_table::LockfreeHashTable;
pub use ring_broadcast::{
    channel, BroadcastError, BroadcastReceiver, BroadcastSender, Result as BroadcastResult,
};

// Ring Trace - T5 Streaming generic ring buffer (migrated from atomic_debugger)
#[cfg(feature = "ring-trace")]
pub use ring_trace::{RingBufferCapsule, RingBufferEntry, TraceEntry, TraceFlags};

// Ring Buffer Const Generic - T0+T5 (Nightly const generics optimization)
#[cfg(all(feature = "ring-trace", feature = "nightly-const-generics"))]
pub mod ring_buffer_const;

#[cfg(all(feature = "ring-trace", feature = "nightly-const-generics"))]
pub use ring_buffer_const::RingBufferCapsuleConst;

pub use serializable::BitwiseSerializable;
pub use stats_capsule::{StatsCapsule64, StatsSnapshot};

// Robin Hood hash map (T1 Atomic, linear probing)
pub use robin_hood_hash::RobinHoodHashCapsule;

#[cfg(feature = "async-log")]
pub use async_log::{AsyncLogCapsule, LogEntry};

// Re-export both cache implementations (different use cases)
#[cfg(feature = "std")]
pub use cache::LockfreeCacheCapsule as LockfreeCacheCapsuleKV; // Generic <K,V> hash table with linear probing

// Distributed cache exports (Phase L3)
#[cfg(feature = "distributed")]
pub use distributed_cache::{
    DistributedCache, DistributedCacheError, DistributedCacheKey, DistributedCacheNode,
    DistributedCacheStats, NodeConfig, Result as DistributedResult,
};

// Distributed cache audit exports (Phase P1.3 - Q34 Auditability)
#[cfg(feature = "distributed-audit")]
pub use distributed_cache_audit::{AuditableDistributedCache, CacheAuditEntry};

// Histogram exports (T6 Mixed: T1 Atomic + T4 Batch)
#[cfg(feature = "histogram")]
pub use histogram::{HistogramCapsule, PercentileSnapshot};

// Histogram const generics exports (T0+T1: Auditable + Atomic, 99.996% allocation speedup)
#[cfg(all(feature = "histogram", feature = "nightly-const-generics"))]
pub use histogram_const::{HistogramConst, PercentileSnapshotConst, is_power_of_two};

// Streaming Stats exports (T5 Streaming: T-Digest percentiles)
#[cfg(feature = "streaming-stats")]
pub use streaming_stats::{StreamingStatsCapsule, StreamingSnapshot};

// Persistent MinHash exports (T9 + T10: Incremental deduplication)
#[cfg(all(
    feature = "mmap-persistence",
    feature = "nightly-atomic",
    feature = "probabilistic"
))]
pub use persistent_minhash::{PersistentMinHashEntry, PersistentMinHashIndex};

// Persistent LSH Table exports (T9 Persistent + T10 Probabilistic)
#[cfg(feature = "probabilistic")]
pub use persistent_lsh::{LshError, PersistentLSHTable};

// Persistent Deduplication Index exports (T9 Persistent + T10 Probabilistic)
#[cfg(feature = "probabilistic")]
pub use persistent_dedup::{
    DedupError, DeduplicationStats, PersistentDedupCore, PersistentDedupImpl, PersistentDedupIndex,
};

// Multi-tenant cache exports (Phase 2.5 - Cryptographic namespace isolation)
#[cfg(feature = "cache")]
pub use cache_multi_tenant::hash_key_tenant;

// Queue exports (Phase 1: Bounded SPSC/MPMC)
#[cfg(feature = "queue-bounded")]
pub use queue::{QueueCapsule, QueueError, PushError, SPSC, MPMC};

// Synchronous flush task exports (Tokio removal)
#[cfg(all(feature = "std", feature = "queue-bounded"))]
pub use sync_flush_task::{SyncFlushTask, SyncLogEntry};

// Phase 11.0: Lockfree B-tree (lockfree ordered data structure)
#[cfg(feature = "lockfree-btree")]
pub mod lockfree_btree;
#[cfg(feature = "lockfree-btree")]
pub use lockfree_btree::{
    LockfreeBTree, BTreeNode, BTreeStatsCapsule, BTreeError, NodeType,
    BTreeIter, BTreeSnapshot,
};

// ScalableHashMapCapsule - Unbounded lockfree hash map (T1 Atomic + T2 SIMD)
// Phase 2: Basic Hopscotch hashing (H=32 neighborhood, 64B buckets, pre-sized capacity)
#[cfg(feature = "std")]
pub mod scalable_hashmap;
#[cfg(feature = "std")]
pub use scalable_hashmap::ScalableHashMapCapsule;

// BulkCollectorCapsule - T4 Batch lockfree bulk collection
// Phase 1: Append-only collector for parallel signature gathering
#[cfg(feature = "bulk-collector")]
pub mod bulk_collector;
#[cfg(feature = "bulk-collector")]
pub use bulk_collector::{BulkCollectorCapsule, BulkCollectorError};
