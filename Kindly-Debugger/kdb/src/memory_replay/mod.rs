//! Memory Replay Module - Copy-on-Write Memory Tracking for Full State Replay
//!
//! Provides page-level delta tracking using Linux soft-dirty bits for efficient
//! memory state capture and time-travel debugging with Q34 audit compliance.
//!
//! # Architecture Overview
//!
//! The memory replay system uses a hierarchical approach to track and replay
//! process memory state with minimal overhead:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    MemoryReplayCapsule (T6 Mixed)                   │
//! │                         Orchestrator Layer                           │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
//! │  │ DirtyPageTracker │  │ MemoryDeltaRing  │  │ MerklePageTree   │  │
//! │  │   Capsule (T2)   │  │  Buffer (T5)     │  │  Capsule (T0)    │  │
//! │  │   256KB bitmap   │  │  32-60MB deltas  │  │  512KB hashes    │  │
//! │  └──────────────────┘  └──────────────────┘  └──────────────────┘  │
//! │                              │                                       │
//! │                              ▼                                       │
//! │                    ┌──────────────────┐                             │
//! │                    │ MemoryReconstructor │                          │
//! │                    │   Capsule (T6)      │                          │
//! │                    │   128KB workspace   │                          │
//! │                    └──────────────────┘                             │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Component Tiers
//!
//! - **PageDelta + PageDeltaBuffer**: Core delta compression primitives with
//!   XOR-based diffing, RLE compression, and Q34 hash-chain integrity.
//!
//! - **DirtyPageTrackerCapsule (T2 SIMD)**: 256KB bitmap for tracking dirty pages
//!   via `/proc/<pid>/pagemap` and soft-dirty bits. SIMD-accelerated scanning.
//!
//! - **MemoryDeltaRingBufferCapsule (T5 Streaming)**: 32-60MB ring buffer for storing
//!   compressed page deltas with O(1) append and lockfree operation.
//!
//! - **MerklePageTreeCapsule (T0 Auditable)**: 512KB Merkle tree for Q34 hash-chain
//!   integrity verification and tamper detection.
//!
//! - **MemoryReconstructorCapsule (T6 Mixed)**: 128KB workspace for page reconstruction
//!   combining delta application with Merkle verification.
//!
//! - **MemoryReplayCapsule (T6 Mixed)**: Orchestrator metacapsule coordinating all
//!   components for full memory state replay.
//!
//! # Performance Targets
//!
//! - XOR delta computation: <1us per 4KB page
//! - LZ4-style compression: 2-10x reduction for typical deltas
//! - Zero page detection: <50ns via SIMD (T2)
//! - CRC64 hash: <100ns per page
//! - Dirty page scan: <10ms for 1GB address space (SIMD-accelerated)
//! - Delta storage: <1us per page (lockfree ring buffer)
//! - Page reconstruction: <5us per page (including verification)
//! - Merkle verification: O(log n) per page, O(n) for full tree
//!
//! # Framework Compliance
//!
//! - **UCE34**: T0 Auditable + T2 SIMD + T5 Streaming + T6 Mixed composition
//! - **COCA**: 100% lockfree, SeqLock for consistent reads
//! - **ASSUM**: All pointer operations and index calculations documented
//! - **T28**: 20+ tests (unit, property, integration)
//! - **Q34**: Hash-chain integrity for audit trail
//!
//! # ASSUM Tags (Module Level)
//!
//! #ASSUME_LOCKFREE_ONLY: All capsules use atomic operations only
//! #ASSUME_LINUX_ONLY: Soft-dirty bits require Linux 3.11+
//! #ASSUME_PAGE_SIZE_4K: All page operations assume 4KB pages
//! #ASSUME_PTRACE_ATTACHED: Memory reads require ptrace attachment

// ============================================================================
// Sub-modules
// ============================================================================

pub mod page_delta;
pub mod dirty_page_tracker_capsule;
// Phase 2 Agent 4: MemoryReconstructorCapsule + MemoryReplayCapsule (T6 Orchestrators)
pub mod memory_reconstructor_capsule;
pub mod memory_replay_capsule;
// Phase 2 Agent 3: MemoryDeltaRingBuffer + MerklePageTreeCapsule
pub mod memory_delta_ring_buffer;
pub mod merkle_page_tree_capsule;
// Q34 Audit Trail Integration (Phase 4)
pub mod audit_integration;

// ============================================================================
// Public Re-exports
// ============================================================================

// PageDelta primitives (T0 Auditable + T2 SIMD)
pub use page_delta::{
    PageDelta, PageDeltaBuffer, PageDeltaFlags, AtomicPageDeltaHash,
    PAGE_SIZE, MAX_COMPRESSED_SIZE,
    compute_xor_delta, apply_xor_delta, apply_delta, is_zero_page,
    sparse_regions, compute_crc64, compress_rle, decompress_rle,
};

// DirtyPageTracker (T2 SIMD)
pub use dirty_page_tracker_capsule::{
    DirtyPageTrackerCapsule, DirtyPageIterator, TrackerError,
    TRACKED_PAGES, BITMAP_WORDS, SOFT_DIRTY_BIT,
    state as dirty_state,
};

// Phase 2 Agent 4: MemoryReconstructorCapsule (T6 Mixed)
pub use memory_reconstructor_capsule::{
    MemoryReconstructorCapsule, PageCacheEntry, ReconstructError,
    ReconstructorState, ReconstructStats, CacheFlags,
    CACHE_CAPACITY, PAGE_SIZE as RECONSTRUCTOR_PAGE_SIZE,
};

// Phase 2 Agent 4: MemoryReplayCapsule (T6 Mixed Orchestrator)
pub use memory_replay_capsule::{
    MemoryReplayCapsule, ReplayConfig, ReplayError, ReplayState, ReplayStats,
    DirtyPageTrackerStub, PageDelta as ReplayPageDelta,
    MAX_TRACKED_PAGES, MAX_DELTAS_PER_SNAPSHOT,
};

// Phase 2 Agent 3: MemoryDeltaRingBuffer (T5 Streaming)
pub use memory_delta_ring_buffer::{
    DeltaIterator, MemoryDeltaRingBufferCapsule,
    PageDeltaBuffer as RingPageDeltaBuffer, // Alias to avoid conflict with page_delta::PageDeltaBuffer
    RingError, RingStats,
    DEFAULT_CAPACITY_MB, MAX_CAPACITY_MB, MIN_CAPACITY_MB,
    INDEX_CAPACITY, HEADER_SIZE, INDEX_SIZE, MAX_DELTA_SIZE, DELTA_MAGIC,
};

// Phase 2 Agent 3: MerklePageTreeCapsule (T0 Auditable)
pub use merkle_page_tree_capsule::{
    MerklePageTreeCapsule, MerkleProof, TreeError, TreeStats,
    LEAF_COUNT, TREE_HEIGHT, INTERNAL_NODE_COUNT, TOTAL_NODES, COVERED_MEMORY,
};

// Q34 Audit Trail Integration (Phase 4)
pub use audit_integration::{
    MemoryAuditEvent, MemoryAuditEntry, MemoryAuditTrailCapsule, MemoryAuditStats,
    record_dirty_page, record_delta_capture, record_delta_apply,
    record_snapshot_created, record_replay_started, record_replay_completed,
    record_cow_page_created,
    MEMORY_AUDIT_ENTRY_COUNT, MEMORY_AUDIT_ENTRY_SIZE, MEMORY_AUDIT_TRAIL_SIZE,
    PAGE_SIZE as AUDIT_PAGE_SIZE,
};
