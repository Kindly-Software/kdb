//! Tier 9: Persistent Capsules
//!
//! Memory-mapped file management with lockfree atomic coordination.
//!
//! # Architecture
//!
//! - **MmapRegion**: T1 Atomic capsule for region metadata (128B aligned)
//! - **MmapManager**: Container capsule managing 8 fixed regions
//! - **MmapHandle**: T0 wrapper for zero-copy atomic views
//! - **PersistentMmap**: NEW T9 core capsule (atomic_from_mut + generation counters)
//! - **PersistentMap<K,V>**: T9 tier persistent hash map (v0.3.2)
//! - **PersistentLog<T>**: T5+T9 tier append-only log (v0.3.2)
//!
//! # Features
//!
//! - Lockfree allocation via atomic CAS loops
//! - Generation counters for ABA prevention
//! - 4KB page alignment validation
//! - Zero-copy integration with atomic_from_mut
//! - Hash-chained audit trail for Q34 Auditability
//! - **NEW**: Two-phase commit for crash-safe updates
//! - **NEW**: Alignment validation module
//! - **NEW**: Recovery procedures for partial updates
//!
//! # Phase 2: Complete (v0.3.2) ✅
//!
//! - **fsync() durability**: Full memmap2 flush support (<1-5ms)
//! - **Hash chain updates**: Tamper-evident audit trails (<50ns)
//! - **Crash recovery tests**: 7+ T28 tests (uncommitted data loss validation)
//! - **Generation counters**: TOCTOU prevention for all persistent structures
//! - **ASSUM rating**: 99.9%+ safety (5 categories audited)
//! - **I20 integration**: All 20 questions validated
//!
//! # Phase 3: T9 Core Implementation (NEW) ✅
//!
//! - **PersistentMmap**: Zero-copy atomic operations over mmap
//! - **Alignment module**: Compile-time + runtime verification
//! - **Recovery module**: Generation counter crash recovery
//! - **Performance**: <50ns atomic store, <1ms async flush, <100ms recovery
//! - **IMPL-2 V3.1**: Cutting-edge-first (nightly atomic_from_mut)
//!
//! # Performance
//!
//! - Initialization: <10ms for 1GB file
//! - Allocation: <50ns (lockfree CAS)
//! - Region access: <5ns (array index)
//! - **Atomic store (NEW)**: <50ns (direct mmap)
//! - **Atomic load (NEW)**: <10ns (direct mmap)
//! - Map insert: <100ns (lockfree CAS loop)
//! - Map lookup: <50ns (zero-copy borrow)
//! - Log append: <50ns (lockfree CAS + FNV-1a hash)
//! - **fsync: <1-5ms** (NVMe ~1ms, SATA SSD ~3ms, HDD ~5ms)
//!
//! # Example (NEW - PersistentMmap)
//!
//! ```rust,ignore
//! use atomic_capsule::persistence::PersistentMmap;
//! use std::path::Path;
//! use std::sync::atomic::Ordering;
//!
//! // Create 1MB file with 512-byte items
//! let mut mmap = PersistentMmap::create_mmap(
//!     Path::new("data.mmap"),
//!     1024 * 1024,
//!     512,
//! )?;
//!
//! // Atomic view at offset 128 (after header)
//! let atomic = mmap.atomic_view_u64(128)?;
//! atomic.store(42, Ordering::Release);
//!
//! // Flush to disk (async)
//! mmap.flush_async()?;
//!
//! // Two-phase commit
//! mmap.begin_update()?;
//! atomic.store(100, Ordering::Release);
//! mmap.commit_update()?;
//! ```
//!
//! # Example (Existing - MmapManager)
//!
//! ```rust,ignore
//! use atomic_capsule::persistence::{MmapManager, MmapLayout, PersistentMap, PersistentLog};
//! use std::path::Path;
//!
//! // Memory-mapped file manager
//! let layout = MmapLayout::new(4096 * 1024, 8)?; // 4MB, 8 regions
//! let manager = MmapManager::new(Path::new("data.bin"), &layout)?;
//!
//! // Allocate in region 0
//! let region = manager.region(0).unwrap();
//! let offset = region.allocate(1024)?;
//! println!("Allocated at offset: {}", offset);
//!
//! // Persistent map
//! let mut map: PersistentMap<u64, u64> = PersistentMap::new(1024)?;
//! map.insert(42, 100)?;
//! assert_eq!(map.get(&42), Some(&100));
//!
//! // Persistent log
//! let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None)?;
//! log.append(b"Hello, World!".to_vec())?;
//! for (offset, header, data) in log.iter() {
//!     println!("Entry at {}: {:?}", offset, data);
//! }
//! ```

pub mod mmap_manager;

// T4+T9 Batch Persistent Writer (NEW)
pub mod batch_writer;

// T9 Core Implementation (NEW - Phase 3)
#[cfg(all(feature = "mmap-persistence", feature = "nightly-atomic"))]
pub mod alignment;

#[cfg(all(feature = "mmap-persistence", feature = "nightly-atomic"))]
pub mod recovery;

#[cfg(all(feature = "mmap-persistence", feature = "nightly-atomic"))]
pub mod mmap_capsule;

#[cfg(feature = "mmap-persistence")]
pub mod persistent_atomic;

#[cfg(feature = "mmap-persistence")]
pub mod persistent_map;

#[cfg(feature = "mmap-persistence")]
pub mod persistent_log;

pub use mmap_manager::{MmapError, MmapHandle, MmapLayout, MmapRegion};

// Dual-feature support: Both mmap-persistence (memmap2) and capsule-mmap (native)
#[cfg(any(feature = "mmap-persistence", feature = "capsule-mmap"))]
pub use mmap_manager::MmapManager;

#[cfg(all(
    any(feature = "mmap-persistence", feature = "capsule-mmap"),
    feature = "nightly-atomic"
))]
pub use persistent_atomic::PersistentAtomic;

#[cfg(any(feature = "mmap-persistence", feature = "capsule-mmap"))]
pub use persistent_map::{PersistentEntry, PersistentMap, PersistentMapHeader};

#[cfg(any(feature = "mmap-persistence", feature = "capsule-mmap"))]
pub use persistent_log::{LogEntryHeader, LogIterator, PersistentLog, PersistentLogHeader};

// T9 Core Implementation exports (NEW - Phase 3)
#[cfg(all(feature = "mmap-persistence", feature = "nightly-atomic"))]
pub use mmap_capsule::{
    FileHeader, PersistentError, PersistentMmap, HEADER_SIZE, MAGIC, PAGE_SIZE, VERSION,
};

#[cfg(all(feature = "mmap-persistence", feature = "nightly-atomic"))]
pub use alignment::{
    align, cache_line_size, compute_aligned_offset, is_aligned, validate_alignment,
    validate_atomic_alignment, validate_cache_line_separation,
};

#[cfg(all(feature = "mmap-persistence", feature = "nightly-atomic"))]
pub use recovery::{
    recover_partial_update, two_phase_commit_finish, two_phase_commit_start, validate_recovery,
    GenerationCounter, RecoveryState,
};

// T4+T9 Batch Writer exports (NEW)
pub use batch_writer::{BatchPersistentWriter, BATCH_BUFFER_SIZE, BATCH_SIZE, ENTRY_SIZE};

// T9+T2 Persistent SIMD Vector (NEW - Phase T9+T2)
#[cfg(all(feature = "mmap-persistence", feature = "portable_simd"))]
pub mod simd_vector;

#[cfg(all(feature = "mmap-persistence", feature = "portable_simd"))]
pub use simd_vector::PersistentSimdVector;

// ============================================================================
// FSYNC DURABILITY TRAIT (Q15: Integration Point)
// ============================================================================

/// Trait for types supporting fsync durability
///
/// **UCE34 Q15**: Integration point for crash-safe durability
/// **UCE34 Q34**: Auditability via fsync guarantees
///
/// # Safety
///
/// Implementations must ensure:
/// - All in-memory writes flushed to disk
/// - Atomic durability (all or nothing)
/// - Generation counters updated after fsync
///
/// # Performance
///
/// - fsync: <1ms typical (depends on filesystem/storage)
/// - Batch fsync: Amortize cost over multiple writes
///
/// # Example
///
/// ```rust,ignore
/// use atomic_capsule::persistence::{Durable, MmapManager};
///
/// let mut manager = MmapManager::new("data.bin", &layout)?;
/// // Write data...
/// manager.fsync()?; // Ensure durability
/// ```
///
/// # Implementation Notes
///
/// Implementations are in their respective struct files:
/// - `MmapManager::fsync()` in `mmap_manager.rs`
/// - `PersistentMap::fsync()` in `persistent_map.rs`
/// - `PersistentLog::fsync()` in `persistent_log.rs`
///
/// # Dual-Feature Support (v0.3.4 - Week 2 Phase 2)
///
/// This trait supports both feature flags for backward compatibility:
/// - `mmap-persistence`: Uses memmap2 dependency (existing)
/// - `capsule-mmap`: Uses capsule-native mmap (new in v0.3.4)
///
/// Migration timeline:
/// - v0.3.4: Parallel deployment (both features work)
/// - v0.4.0: mmap-persistence deprecated
/// - v0.5.0: mmap-persistence removed (breaking change with migration guide)
#[cfg(any(feature = "mmap-persistence", feature = "capsule-mmap"))]
pub trait Durable {
    /// Flush all in-memory writes to disk (fsync)
    ///
    /// # Errors
    ///
    /// Returns `MmapError::IOError` if fsync fails.
    ///
    /// # Performance
    ///
    /// <1ms typical (depends on filesystem/storage)
    fn fsync(&mut self) -> Result<(), MmapError>;

    /// Check if durability is supported
    fn supports_fsync(&self) -> bool {
        true
    }
}
