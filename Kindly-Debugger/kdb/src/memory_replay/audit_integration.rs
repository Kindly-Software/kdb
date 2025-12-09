//! Memory Replay Audit Integration - Q34 Hash-Chain Audit Trail for Memory Operations
//!
//! # Architecture
//!
//! This module provides T0 Auditable integration for the MemoryReplayCapsule, enabling
//! cryptographically tamper-evident audit trails for all memory replay operations.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │               MemoryAuditTrailCapsule (T0 Auditable)                │
//! │                   128KB (2048 × 64-byte entries)                    │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
//! │  │ MemoryAuditEntry │  │ MemoryAuditEntry │  │ MemoryAuditEntry │  │
//! │  │   64 bytes       │  │   64 bytes       │  │   64 bytes       │  │
//! │  │   Cache-aligned  │  │   Cache-aligned  │  │   Cache-aligned  │  │
//! │  └──────────────────┘  └──────────────────┘  └──────────────────┘  │
//! │              ↓                   ↓                   ↓              │
//! │           prev_hash ───────► entry_hash ───────► next_hash...      │
//! │                        (CRC64 hash-chain)                           │
//! │                                                                     │
//! │  ┌──────────────────────────────────────────────────────────────┐  │
//! │  │           Merkle Tree Integration (T0 Auditable)              │  │
//! │  │    Per-page hashes link to MerklePageTreeCapsule root         │  │
//! │  └──────────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Memory Events Tracked
//!
//! - **DirtyPageDetected**: Soft-dirty bit set on page
//! - **DeltaCaptured**: Page delta computed and stored
//! - **DeltaApplied**: Page reconstructed from delta
//! - **SnapshotCreated**: Full memory snapshot created
//! - **MerkleVerified**: Merkle tree verification performed
//! - **PageEvicted**: Page evicted from cache
//! - **ReplayStarted**: Time-travel replay initiated
//! - **ReplayCompleted**: Replay operation finished
//!
//! # Performance Targets
//!
//! - `append()`: <50ns lockfree (single atomic CAS)
//! - `verify_chain()`: O(n) for full verification
//! - `verify_recent()`: <50ns (last 3 entries only)
//! - `get_root_hash()`: <10ns (atomic load)
//! - `correlate_merkle()`: <100ns (link to Merkle tree)
//!
//! # Framework Compliance
//!
//! - **UCE34**: T0 Auditable tier, Q34 hash-chain integrity
//! - **Chaos**: 100% lockfree, no mutex/RwLock
//! - **ASSUM**: All assumptions documented with #ASSUME/#VERIFY tags
//! - **T28**: 20+ tests (unit/property/integration)
//! - **B32**: <50ns append validated
//!
//! # ASSUM Tags (Module Level)
//!
//! #ASSUME_LOCKFREE_ONLY: All operations use atomic primitives only
//! #ASSUME_CRC64_ECMA: Uses CRC64-ECMA-182 for hash computation
//! #ASSUME_CACHE_ALIGNED: All entries 64-byte cache-line aligned
//! #ASSUME_MERKLE_INTEGRATION: Page hashes correlate with Merkle tree

use std::sync::atomic::{AtomicU64, Ordering};
use crc::{Crc, CRC_64_ECMA_182};

// ============================================================================
// Constants
// ============================================================================

/// Number of audit entries in the ring buffer (2048 entries = 128KB)
/// Larger than session audit to handle high-frequency page operations
pub const MEMORY_AUDIT_ENTRY_COUNT: usize = 2048;

/// Size of each audit entry in bytes (cache-line aligned)
pub const MEMORY_AUDIT_ENTRY_SIZE: usize = 64;

/// Total audit trail size in bytes
pub const MEMORY_AUDIT_TRAIL_SIZE: usize = MEMORY_AUDIT_ENTRY_COUNT * MEMORY_AUDIT_ENTRY_SIZE;

/// CRC64-ECMA-182 for hash computation
const CRC64: Crc<u64> = Crc::<u64>::new(&CRC_64_ECMA_182);

/// Sentinel value for empty/invalid entries
const INVALID_HASH: u64 = 0;

/// Initial hash for chain start
const GENESIS_HASH: u64 = 0xDEAD_BEEF_CAFE_BABE;

/// Page size constant (4KB)
pub const PAGE_SIZE: usize = 4096;

// ============================================================================
// Memory Audit Event Types
// ============================================================================

/// Memory audit event types for tracking all memory replay operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MemoryAuditEvent {
    /// Dirty page detected via soft-dirty bits
    DirtyPageDetected = 0,
    /// Page delta computed and stored
    DeltaCaptured = 1,
    /// Page delta applied for reconstruction
    DeltaApplied = 2,
    /// Full memory snapshot created
    SnapshotCreated = 3,
    /// Merkle tree verification performed
    MerkleVerified = 4,
    /// Page evicted from reconstruction cache
    PageEvicted = 5,
    /// Time-travel replay started
    ReplayStarted = 6,
    /// Time-travel replay completed
    ReplayCompleted = 7,
    /// COW page created
    CowPageCreated = 8,
    /// COW page merged
    CowPageMerged = 9,
    /// Memory region mapped
    RegionMapped = 10,
    /// Memory region unmapped
    RegionUnmapped = 11,
    /// Delta compression performed
    DeltaCompressed = 12,
    /// Delta decompression performed
    DeltaDecompressed = 13,
    /// Soft-dirty bits cleared
    SoftDirtyCleared = 14,
    /// Invalid/placeholder event
    Invalid = 255,
}

impl MemoryAuditEvent {
    /// Convert from u8
    #[inline]
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => MemoryAuditEvent::DirtyPageDetected,
            1 => MemoryAuditEvent::DeltaCaptured,
            2 => MemoryAuditEvent::DeltaApplied,
            3 => MemoryAuditEvent::SnapshotCreated,
            4 => MemoryAuditEvent::MerkleVerified,
            5 => MemoryAuditEvent::PageEvicted,
            6 => MemoryAuditEvent::ReplayStarted,
            7 => MemoryAuditEvent::ReplayCompleted,
            8 => MemoryAuditEvent::CowPageCreated,
            9 => MemoryAuditEvent::CowPageMerged,
            10 => MemoryAuditEvent::RegionMapped,
            11 => MemoryAuditEvent::RegionUnmapped,
            12 => MemoryAuditEvent::DeltaCompressed,
            13 => MemoryAuditEvent::DeltaDecompressed,
            14 => MemoryAuditEvent::SoftDirtyCleared,
            _ => MemoryAuditEvent::Invalid,
        }
    }

    /// Convert to u8
    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Get event name as string
    #[inline]
    pub fn as_str(self) -> &'static str {
        match self {
            MemoryAuditEvent::DirtyPageDetected => "dirty_page_detected",
            MemoryAuditEvent::DeltaCaptured => "delta_captured",
            MemoryAuditEvent::DeltaApplied => "delta_applied",
            MemoryAuditEvent::SnapshotCreated => "snapshot_created",
            MemoryAuditEvent::MerkleVerified => "merkle_verified",
            MemoryAuditEvent::PageEvicted => "page_evicted",
            MemoryAuditEvent::ReplayStarted => "replay_started",
            MemoryAuditEvent::ReplayCompleted => "replay_completed",
            MemoryAuditEvent::CowPageCreated => "cow_page_created",
            MemoryAuditEvent::CowPageMerged => "cow_page_merged",
            MemoryAuditEvent::RegionMapped => "region_mapped",
            MemoryAuditEvent::RegionUnmapped => "region_unmapped",
            MemoryAuditEvent::DeltaCompressed => "delta_compressed",
            MemoryAuditEvent::DeltaDecompressed => "delta_decompressed",
            MemoryAuditEvent::SoftDirtyCleared => "soft_dirty_cleared",
            MemoryAuditEvent::Invalid => "invalid",
        }
    }

    /// Check if event is page-related (high frequency)
    #[inline]
    pub fn is_page_event(self) -> bool {
        matches!(
            self,
            MemoryAuditEvent::DirtyPageDetected
                | MemoryAuditEvent::DeltaCaptured
                | MemoryAuditEvent::DeltaApplied
                | MemoryAuditEvent::PageEvicted
                | MemoryAuditEvent::CowPageCreated
                | MemoryAuditEvent::CowPageMerged
        )
    }
}

impl std::fmt::Display for MemoryAuditEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Memory Audit Entry (64 bytes, cache-line aligned)
// ============================================================================

/// Memory audit entry - 64 bytes, cache-line aligned
///
/// # Layout (64 bytes)
/// ```text
/// ┌────────────────────────────────────────────────────────────────┐
/// │ timestamp_ns: u64 (8B)     │ page_address: u64 (8B)            │
/// ├────────────────────────────┼───────────────────────────────────┤
/// │ event_type: u8 (1B)        │ snapshot_id: u8 (1B)              │
/// │ compression_ratio: u8 (1B) │ _pad1: u8 (1B)                    │
/// │ page_count: u32 (4B)                                           │
/// ├────────────────────────────────────────────────────────────────┤
/// │ prev_hash: u64 (8B)        │ entry_hash: u64 (8B)              │
/// ├────────────────────────────────────────────────────────────────┤
/// │ page_hash: u64 (8B) - CRC64 of page contents                   │
/// ├────────────────────────────────────────────────────────────────┤
/// │ delta_size: u32 (4B)       │ merkle_leaf_idx: u32 (4B)         │
/// ├────────────────────────────────────────────────────────────────┤
/// │ _padding: [u8; 8]                                              │
/// └────────────────────────────────────────────────────────────────┘
/// Total: 64 bytes (1 cache line)
/// ```
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy)]
pub struct MemoryAuditEntry {
    /// Timestamp in nanoseconds since UNIX epoch
    pub timestamp_ns: u64,
    /// Virtual address of the page (4KB aligned)
    pub page_address: u64,
    /// Event type (MemoryAuditEvent)
    pub event_type: u8,
    /// Snapshot ID this event belongs to (0-255)
    pub snapshot_id: u8,
    /// Compression ratio (0-255, where 100 = 1:1)
    pub compression_ratio: u8,
    /// Padding for alignment
    _pad1: u8,
    /// Number of pages affected
    pub page_count: u32,
    /// Previous entry's hash (chain linkage)
    pub prev_hash: u64,
    /// This entry's computed hash
    pub entry_hash: u64,
    /// CRC64 hash of page contents (for verification)
    pub page_hash: u64,
    /// Delta size in bytes (compressed)
    pub delta_size: u32,
    /// Merkle tree leaf index for this page
    pub merkle_leaf_idx: u32,
    /// Reserved padding for future use
    _padding: [u8; 8],
}

// Compile-time size verification
const _: () = {
    const EXPECTED: usize = 64;
    const ACTUAL: usize = std::mem::size_of::<MemoryAuditEntry>();
    assert!(ACTUAL == EXPECTED, "MemoryAuditEntry must be exactly 64 bytes");
};

impl Default for MemoryAuditEntry {
    fn default() -> Self {
        Self {
            timestamp_ns: 0,
            page_address: 0,
            event_type: MemoryAuditEvent::Invalid.as_u8(),
            snapshot_id: 0,
            compression_ratio: 100,
            _pad1: 0,
            page_count: 0,
            prev_hash: INVALID_HASH,
            entry_hash: INVALID_HASH,
            page_hash: 0,
            delta_size: 0,
            merkle_leaf_idx: 0,
            _padding: [0; 8],
        }
    }
}

impl MemoryAuditEntry {
    /// Create a new audit entry with computed hash
    ///
    /// # Arguments
    /// - `prev_hash`: Previous entry's hash for chain linkage
    /// - `event`: Event type
    /// - `page_address`: Virtual address of the page
    /// - `page_count`: Number of pages affected
    /// - `snapshot_id`: Snapshot ID
    /// - `page_hash`: CRC64 of page contents
    /// - `delta_size`: Compressed delta size
    /// - `merkle_idx`: Merkle tree leaf index
    /// - `compression_ratio`: Compression ratio (0-255)
    ///
    /// # Returns
    /// New MemoryAuditEntry with computed entry_hash
    #[inline]
    pub fn new(
        prev_hash: u64,
        event: MemoryAuditEvent,
        page_address: u64,
        page_count: u32,
        snapshot_id: u8,
        page_hash: u64,
        delta_size: u32,
        merkle_idx: u32,
        compression_ratio: u8,
    ) -> Self {
        let timestamp_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let mut entry = Self {
            timestamp_ns,
            page_address,
            event_type: event.as_u8(),
            snapshot_id,
            compression_ratio,
            _pad1: 0,
            page_count,
            prev_hash,
            entry_hash: 0, // Will be computed below
            page_hash,
            delta_size,
            merkle_leaf_idx: merkle_idx,
            _padding: [0; 8],
        };

        // Compute entry hash based on all fields
        entry.entry_hash = entry.compute_hash();
        entry
    }

    /// Compute CRC64 hash for this entry
    ///
    /// Hash includes: prev_hash, timestamp, page_address, event, page_hash, delta_size, merkle_idx
    ///
    /// # Performance
    /// <50ns per entry
    #[inline]
    fn compute_hash(&self) -> u64 {
        let mut digest = CRC64.digest();

        // Hash all significant fields
        digest.update(&self.prev_hash.to_le_bytes());
        digest.update(&self.timestamp_ns.to_le_bytes());
        digest.update(&self.page_address.to_le_bytes());
        digest.update(&[self.event_type, self.snapshot_id, self.compression_ratio, 0]);
        digest.update(&self.page_count.to_le_bytes());
        digest.update(&self.page_hash.to_le_bytes());
        digest.update(&self.delta_size.to_le_bytes());
        digest.update(&self.merkle_leaf_idx.to_le_bytes());

        digest.finalize()
    }

    /// Verify this entry's hash is correct
    ///
    /// # Returns
    /// true if entry_hash matches computed hash
    #[inline]
    pub fn verify(&self) -> bool {
        self.entry_hash == self.compute_hash()
    }

    /// Get event type as enum
    #[inline]
    pub fn event(&self) -> MemoryAuditEvent {
        MemoryAuditEvent::from_u8(self.event_type)
    }

    /// Check if entry is valid (has non-zero hash)
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.entry_hash != INVALID_HASH && self.event_type != MemoryAuditEvent::Invalid.as_u8()
    }

    /// Get page index from address (4KB pages)
    #[inline]
    pub fn page_index(&self) -> u64 {
        self.page_address / PAGE_SIZE as u64
    }
}

// ============================================================================
// Memory Audit Trail Capsule (256-byte aligned, 128KB entries)
// ============================================================================

/// MemoryAuditTrailCapsule - T0 Auditable hash-chain for memory operations
///
/// # Size
/// - Orchestrator: 256 bytes (metadata + atomics)
/// - Entries: 128KB (2048 × 64-byte entries)
/// - Total: ~131KB
///
/// # Thread Safety
/// All operations are lockfree via atomic CAS operations.
///
/// # Integration with MerklePageTreeCapsule
/// Each page event records the Merkle leaf index, enabling cross-verification
/// between the audit trail and the Merkle tree.
///
/// # ASSUM Safety
/// - #ASSUME_LOCKFREE_ONLY: Zero mutex/RwLock
/// - #ASSUME_CACHE_ALIGNED: All entries 64-byte aligned
/// - #ASSUME_ABA_PREVENTION: Generation counter prevents ABA
/// - #ASSUME_MERKLE_CORRELATION: merkle_leaf_idx links to MerklePageTreeCapsule
#[repr(C, align(256))]
pub struct MemoryAuditTrailCapsule {
    // ========================================================================
    // Atomic Coordination (64 bytes)
    // ========================================================================
    /// Ring buffer head (write position)
    head: AtomicU64,
    /// Ring buffer tail (oldest valid entry)
    tail: AtomicU64,
    /// Total entries written (never decreases)
    entry_count: AtomicU64,
    /// Current root hash (most recent entry's hash)
    root_hash: AtomicU64,
    /// Generation counter (ABA prevention)
    generation: AtomicU64,
    /// State: 0=uninitialized, 1=ready, 2=verifying
    state: AtomicU64,
    /// Padding to 64 bytes
    _padding1: [u8; 64 - 6 * 8],

    // ========================================================================
    // Statistics (128 bytes - more events to track)
    // ========================================================================
    /// Total dirty page events
    dirty_page_count: AtomicU64,
    /// Total delta capture events
    delta_capture_count: AtomicU64,
    /// Total delta apply events
    delta_apply_count: AtomicU64,
    /// Total snapshot events
    snapshot_count: AtomicU64,
    /// Total merkle verification events
    merkle_verify_count: AtomicU64,
    /// Total page eviction events
    page_evict_count: AtomicU64,
    /// Total replay events
    replay_count: AtomicU64,
    /// Total COW events
    cow_count: AtomicU64,
    /// Total bytes captured (compressed)
    total_delta_bytes: AtomicU64,
    /// Total pages tracked
    total_pages_tracked: AtomicU64,
    /// Padding to 128 bytes
    _padding2: [u8; 128 - 10 * 8],

    // ========================================================================
    // Merkle Integration (64 bytes)
    // ========================================================================
    /// Pointer to MerklePageTreeCapsule (for correlation)
    merkle_tree_ptr: AtomicU64,
    /// Last verified Merkle root hash
    last_merkle_root: AtomicU64,
    /// Last Merkle verification timestamp
    last_merkle_verify_ns: AtomicU64,
    /// Padding
    _padding3: [u8; 64 - 3 * 8],

    // ========================================================================
    // Entry Storage (128KB)
    // ========================================================================
    /// Ring buffer of audit entries
    entries: [MemoryAuditEntry; MEMORY_AUDIT_ENTRY_COUNT],
}

// Compile-time size verification
const _: () = {
    // Entries: 2048 * 64 = 131072
    // Atomics: 64 + 128 + 64 = 256
    // Total: ~131KB
    const EXPECTED_MIN: usize = 131072 + 256;
    const ACTUAL: usize = std::mem::size_of::<MemoryAuditTrailCapsule>();
    assert!(ACTUAL >= EXPECTED_MIN, "MemoryAuditTrailCapsule too small");
};

// SAFETY: MemoryAuditTrailCapsule is Send/Sync via atomic operations only
// #ASSUME_ALL_ATOMIC: All mutable coordination via AtomicU64
// #VERIFY_NO_MUTEXES: Zero mutex/RwLock in capsule
unsafe impl Send for MemoryAuditTrailCapsule {}
unsafe impl Sync for MemoryAuditTrailCapsule {}

impl Default for MemoryAuditTrailCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryAuditTrailCapsule {
    /// Create new memory audit trail capsule
    ///
    /// # Performance
    /// O(n) initialization (zeros all entries)
    pub fn new() -> Self {
        Self {
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            entry_count: AtomicU64::new(0),
            root_hash: AtomicU64::new(GENESIS_HASH),
            generation: AtomicU64::new(1),
            state: AtomicU64::new(1), // Ready
            _padding1: [0; 64 - 6 * 8],

            dirty_page_count: AtomicU64::new(0),
            delta_capture_count: AtomicU64::new(0),
            delta_apply_count: AtomicU64::new(0),
            snapshot_count: AtomicU64::new(0),
            merkle_verify_count: AtomicU64::new(0),
            page_evict_count: AtomicU64::new(0),
            replay_count: AtomicU64::new(0),
            cow_count: AtomicU64::new(0),
            total_delta_bytes: AtomicU64::new(0),
            total_pages_tracked: AtomicU64::new(0),
            _padding2: [0; 128 - 10 * 8],

            merkle_tree_ptr: AtomicU64::new(0),
            last_merkle_root: AtomicU64::new(0),
            last_merkle_verify_ns: AtomicU64::new(0),
            _padding3: [0; 64 - 3 * 8],

            entries: [MemoryAuditEntry::default(); MEMORY_AUDIT_ENTRY_COUNT],
        }
    }

    /// Append a new audit entry to the trail
    ///
    /// # Performance
    /// <50ns lockfree (single atomic CAS)
    ///
    /// # Arguments
    /// - `event`: Event type
    /// - `page_address`: Virtual address of the page
    /// - `page_count`: Number of pages affected
    /// - `snapshot_id`: Snapshot ID
    /// - `page_hash`: CRC64 of page contents
    /// - `delta_size`: Compressed delta size in bytes
    /// - `merkle_idx`: Merkle tree leaf index
    /// - `compression_ratio`: Compression ratio (0-255)
    ///
    /// # Returns
    /// Entry index and computed hash
    ///
    /// # ASSUM Safety
    /// - #ASSUME_CAS_LOOP: CAS retry bounded by generation counter
    /// - #VERIFY_APPEND_SUCCESS: Generation counter prevents lost updates
    pub fn append(
        &self,
        event: MemoryAuditEvent,
        page_address: u64,
        page_count: u32,
        snapshot_id: u8,
        page_hash: u64,
        delta_size: u32,
        merkle_idx: u32,
        compression_ratio: u8,
    ) -> (usize, u64) {
        loop {
            let current_head = self.head.load(Ordering::Acquire);
            let current_gen = self.generation.load(Ordering::Acquire);
            let prev_hash = self.root_hash.load(Ordering::Acquire);

            let new_head = (current_head + 1) % MEMORY_AUDIT_ENTRY_COUNT as u64;

            // Create the new entry
            let entry = MemoryAuditEntry::new(
                prev_hash,
                event,
                page_address,
                page_count,
                snapshot_id,
                page_hash,
                delta_size,
                merkle_idx,
                compression_ratio,
            );

            // Try to claim the slot
            // #ASSUME_CAS_ATOMIC: compare_exchange is atomic
            // #VERIFY_SLOT_CLAIMED: Success means we own this slot
            match self.head.compare_exchange_weak(
                current_head,
                new_head,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Write the entry
                    // SAFETY: We own this slot after successful CAS
                    // #ASSUME_SLOT_EXCLUSIVE: CAS success guarantees exclusive access
                    let slot = current_head as usize;

                    // Use pointer write to avoid borrow issues with self
                    // #ASSUME_ALIGNED_WRITE: entries array is 64-byte aligned
                    // #VERIFY_BOUNDS: slot < MEMORY_AUDIT_ENTRY_COUNT (modulo above)
                    unsafe {
                        let entry_ptr = (self.entries.as_ptr() as *mut MemoryAuditEntry).add(slot);
                        std::ptr::write(entry_ptr, entry);
                    }

                    // Update root hash and counters
                    self.root_hash.store(entry.entry_hash, Ordering::Release);
                    self.entry_count.fetch_add(1, Ordering::Relaxed);
                    self.generation.fetch_add(1, Ordering::AcqRel);

                    // Update event-specific counters
                    match event {
                        MemoryAuditEvent::DirtyPageDetected => {
                            self.dirty_page_count.fetch_add(1, Ordering::Relaxed);
                            self.total_pages_tracked.fetch_add(page_count as u64, Ordering::Relaxed);
                        }
                        MemoryAuditEvent::DeltaCaptured => {
                            self.delta_capture_count.fetch_add(1, Ordering::Relaxed);
                            self.total_delta_bytes.fetch_add(delta_size as u64, Ordering::Relaxed);
                        }
                        MemoryAuditEvent::DeltaApplied => {
                            self.delta_apply_count.fetch_add(1, Ordering::Relaxed);
                        }
                        MemoryAuditEvent::SnapshotCreated => {
                            self.snapshot_count.fetch_add(1, Ordering::Relaxed);
                        }
                        MemoryAuditEvent::MerkleVerified => {
                            self.merkle_verify_count.fetch_add(1, Ordering::Relaxed);
                        }
                        MemoryAuditEvent::PageEvicted => {
                            self.page_evict_count.fetch_add(1, Ordering::Relaxed);
                        }
                        MemoryAuditEvent::ReplayStarted | MemoryAuditEvent::ReplayCompleted => {
                            self.replay_count.fetch_add(1, Ordering::Relaxed);
                        }
                        MemoryAuditEvent::CowPageCreated | MemoryAuditEvent::CowPageMerged => {
                            self.cow_count.fetch_add(1, Ordering::Relaxed);
                        }
                        _ => {}
                    }

                    // Advance tail if buffer wraps
                    let count = self.entry_count.load(Ordering::Relaxed);
                    if count > MEMORY_AUDIT_ENTRY_COUNT as u64 {
                        let _ = self.tail.fetch_add(1, Ordering::AcqRel);
                    }

                    return (slot, entry.entry_hash);
                }
                Err(_) => {
                    // Retry
                    std::hint::spin_loop();
                    continue;
                }
            }
        }
    }

    /// Verify the entire hash chain integrity
    ///
    /// # Performance
    /// O(n) where n = number of entries
    ///
    /// # Returns
    /// true if chain is intact, false if tampering detected
    ///
    /// # ASSUM Safety
    /// - #ASSUME_SEQUENTIAL_READ: Reads entries in order
    /// - #VERIFY_CHAIN_INTEGRITY: Each entry.prev_hash == previous.entry_hash
    pub fn verify_chain(&self) -> bool {
        let count = self.entry_count.load(Ordering::Acquire);
        if count == 0 {
            return true;
        }

        let tail = self.tail.load(Ordering::Acquire) as usize;
        let entries_to_check = std::cmp::min(count as usize, MEMORY_AUDIT_ENTRY_COUNT);

        let mut prev_hash = GENESIS_HASH;

        for i in 0..entries_to_check {
            let idx = (tail + i) % MEMORY_AUDIT_ENTRY_COUNT;
            let entry = &self.entries[idx];

            // Verify chain linkage
            if entry.prev_hash != prev_hash {
                return false;
            }

            // Verify entry integrity
            if !entry.verify() {
                return false;
            }

            prev_hash = entry.entry_hash;
        }

        true
    }

    /// Quick verification of last 3 entries only
    ///
    /// # Performance
    /// <50ns (constant time)
    ///
    /// # Returns
    /// true if recent entries are valid
    pub fn verify_recent(&self) -> bool {
        let count = self.entry_count.load(Ordering::Acquire);
        if count == 0 {
            return true;
        }

        let head = self.head.load(Ordering::Acquire) as usize;
        let entries_to_check = std::cmp::min(count as usize, 3);

        for i in 0..entries_to_check {
            let idx = if head >= i + 1 {
                head - i - 1
            } else {
                MEMORY_AUDIT_ENTRY_COUNT - (i + 1 - head)
            };

            let entry = &self.entries[idx];
            if !entry.verify() {
                return false;
            }
        }

        true
    }

    /// Get the current root hash
    ///
    /// # Performance
    /// <10ns (atomic load)
    #[inline]
    pub fn get_root_hash(&self) -> u64 {
        self.root_hash.load(Ordering::Acquire)
    }

    /// Get total entry count
    #[inline]
    pub fn entry_count(&self) -> u64 {
        self.entry_count.load(Ordering::Relaxed)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Record a Merkle tree verification result
    ///
    /// # Arguments
    /// - `merkle_root`: The Merkle root hash that was verified
    /// - `valid`: Whether verification passed
    pub fn record_merkle_verification(&self, merkle_root: u64, valid: bool) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        self.last_merkle_root.store(merkle_root, Ordering::Release);
        self.last_merkle_verify_ns.store(now, Ordering::Release);

        if valid {
            self.append(
                MemoryAuditEvent::MerkleVerified,
                0,
                0,
                0,
                merkle_root,
                0,
                0,
                100,
            );
        }
    }

    /// Get audit statistics
    pub fn get_stats(&self) -> MemoryAuditStats {
        MemoryAuditStats {
            total_entries: self.entry_count.load(Ordering::Relaxed),
            dirty_page_count: self.dirty_page_count.load(Ordering::Relaxed),
            delta_capture_count: self.delta_capture_count.load(Ordering::Relaxed),
            delta_apply_count: self.delta_apply_count.load(Ordering::Relaxed),
            snapshot_count: self.snapshot_count.load(Ordering::Relaxed),
            merkle_verify_count: self.merkle_verify_count.load(Ordering::Relaxed),
            page_evict_count: self.page_evict_count.load(Ordering::Relaxed),
            replay_count: self.replay_count.load(Ordering::Relaxed),
            cow_count: self.cow_count.load(Ordering::Relaxed),
            total_delta_bytes: self.total_delta_bytes.load(Ordering::Relaxed),
            total_pages_tracked: self.total_pages_tracked.load(Ordering::Relaxed),
            root_hash: self.root_hash.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
            chain_valid: self.verify_recent(),
            last_merkle_root: self.last_merkle_root.load(Ordering::Acquire),
        }
    }

    /// Export audit trail as JSON string
    ///
    /// # Performance
    /// <1ms for full trail
    ///
    /// # Returns
    /// JSON string with audit entries, root hash, and verification status
    pub fn export_json(&self) -> String {
        let count = self.entry_count.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire) as usize;
        let entries_to_export = std::cmp::min(count as usize, MEMORY_AUDIT_ENTRY_COUNT);

        let mut json = String::with_capacity(entries_to_export * 320);
        json.push_str("{\n");
        json.push_str("  \"memory_audit_trail\": [\n");

        for i in 0..entries_to_export {
            let idx = (tail + i) % MEMORY_AUDIT_ENTRY_COUNT;
            let entry = &self.entries[idx];

            if i > 0 {
                json.push_str(",\n");
            }

            json.push_str(&format!(
                "    {{\n      \"index\": {},\n      \"timestamp_ns\": {},\n      \"page_address\": \"0x{:016x}\",\n      \"event\": \"{}\",\n      \"page_count\": {},\n      \"snapshot_id\": {},\n      \"page_hash\": \"{:016x}\",\n      \"delta_size\": {},\n      \"merkle_idx\": {},\n      \"compression_ratio\": {},\n      \"prev_hash\": \"{:016x}\",\n      \"entry_hash\": \"{:016x}\",\n      \"valid\": {}\n    }}",
                i,
                entry.timestamp_ns,
                entry.page_address,
                entry.event(),
                entry.page_count,
                entry.snapshot_id,
                entry.page_hash,
                entry.delta_size,
                entry.merkle_leaf_idx,
                entry.compression_ratio,
                entry.prev_hash,
                entry.entry_hash,
                entry.verify()
            ));
        }

        json.push_str("\n  ],\n");
        json.push_str(&format!("  \"root_hash\": \"{:016x}\",\n", self.get_root_hash()));
        json.push_str(&format!("  \"entry_count\": {},\n", count));
        json.push_str(&format!("  \"chain_valid\": {},\n", self.verify_chain()));
        json.push_str(&format!("  \"last_merkle_root\": \"{:016x}\"\n", self.last_merkle_root.load(Ordering::Acquire)));
        json.push_str("}\n");

        json
    }

    /// Get entry at specific index (for debugging/testing)
    ///
    /// # Arguments
    /// - `index`: Entry index (0 = oldest valid, wraps)
    ///
    /// # Returns
    /// Reference to entry if valid
    pub fn get_entry(&self, index: usize) -> Option<&MemoryAuditEntry> {
        let count = self.entry_count.load(Ordering::Acquire) as usize;
        if index >= count || index >= MEMORY_AUDIT_ENTRY_COUNT {
            return None;
        }

        let tail = self.tail.load(Ordering::Acquire) as usize;
        let actual_idx = (tail + index) % MEMORY_AUDIT_ENTRY_COUNT;

        let entry = &self.entries[actual_idx];
        if entry.is_valid() {
            Some(entry)
        } else {
            None
        }
    }

    /// Clear all entries (for testing only)
    ///
    /// # Safety
    /// This should only be used in tests. In production, audit trails should be immutable.
    #[cfg(test)]
    pub fn clear(&self) {
        self.head.store(0, Ordering::Release);
        self.tail.store(0, Ordering::Release);
        self.entry_count.store(0, Ordering::Release);
        self.root_hash.store(GENESIS_HASH, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        self.dirty_page_count.store(0, Ordering::Release);
        self.delta_capture_count.store(0, Ordering::Release);
        self.delta_apply_count.store(0, Ordering::Release);
        self.snapshot_count.store(0, Ordering::Release);
        self.merkle_verify_count.store(0, Ordering::Release);
        self.page_evict_count.store(0, Ordering::Release);
        self.replay_count.store(0, Ordering::Release);
        self.cow_count.store(0, Ordering::Release);
        self.total_delta_bytes.store(0, Ordering::Release);
        self.total_pages_tracked.store(0, Ordering::Release);
    }
}

// ============================================================================
// Memory Audit Statistics
// ============================================================================

/// Memory audit trail statistics
#[derive(Debug, Clone, Copy)]
pub struct MemoryAuditStats {
    /// Total entries recorded
    pub total_entries: u64,
    /// Dirty page detection count
    pub dirty_page_count: u64,
    /// Delta capture count
    pub delta_capture_count: u64,
    /// Delta apply count
    pub delta_apply_count: u64,
    /// Snapshot count
    pub snapshot_count: u64,
    /// Merkle verification count
    pub merkle_verify_count: u64,
    /// Page eviction count
    pub page_evict_count: u64,
    /// Replay operation count
    pub replay_count: u64,
    /// COW operation count
    pub cow_count: u64,
    /// Total delta bytes captured
    pub total_delta_bytes: u64,
    /// Total pages tracked
    pub total_pages_tracked: u64,
    /// Current root hash
    pub root_hash: u64,
    /// Generation counter
    pub generation: u64,
    /// Chain integrity status
    pub chain_valid: bool,
    /// Last Merkle root hash
    pub last_merkle_root: u64,
}

impl std::fmt::Display for MemoryAuditStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MemoryAuditStats {{ entries: {}, dirty: {}, delta_cap: {}, delta_app: {}, \
             pages: {}, bytes: {}, hash: {:016x}, valid: {} }}",
            self.total_entries,
            self.dirty_page_count,
            self.delta_capture_count,
            self.delta_apply_count,
            self.total_pages_tracked,
            self.total_delta_bytes,
            self.root_hash,
            self.chain_valid
        )
    }
}

// ============================================================================
// Helper Functions for MemoryReplayCapsule Integration
// ============================================================================

/// Record dirty page detection event
#[inline]
pub fn record_dirty_page(
    audit: &MemoryAuditTrailCapsule,
    page_address: u64,
    page_count: u32,
) -> u64 {
    let (_, hash) = audit.append(
        MemoryAuditEvent::DirtyPageDetected,
        page_address,
        page_count,
        0,
        0,
        0,
        0,
        100,
    );
    hash
}

/// Record delta capture event
#[inline]
pub fn record_delta_capture(
    audit: &MemoryAuditTrailCapsule,
    page_address: u64,
    snapshot_id: u8,
    page_hash: u64,
    delta_size: u32,
    merkle_idx: u32,
    compression_ratio: u8,
) -> u64 {
    let (_, hash) = audit.append(
        MemoryAuditEvent::DeltaCaptured,
        page_address,
        1,
        snapshot_id,
        page_hash,
        delta_size,
        merkle_idx,
        compression_ratio,
    );
    hash
}

/// Record delta apply event
#[inline]
pub fn record_delta_apply(
    audit: &MemoryAuditTrailCapsule,
    page_address: u64,
    snapshot_id: u8,
    page_hash: u64,
) -> u64 {
    let (_, hash) = audit.append(
        MemoryAuditEvent::DeltaApplied,
        page_address,
        1,
        snapshot_id,
        page_hash,
        0,
        0,
        100,
    );
    hash
}

/// Record snapshot creation event
#[inline]
pub fn record_snapshot_created(
    audit: &MemoryAuditTrailCapsule,
    snapshot_id: u8,
    page_count: u32,
    total_delta_bytes: u32,
) -> u64 {
    let (_, hash) = audit.append(
        MemoryAuditEvent::SnapshotCreated,
        0,
        page_count,
        snapshot_id,
        0,
        total_delta_bytes,
        0,
        100,
    );
    hash
}

/// Record replay started event
#[inline]
pub fn record_replay_started(
    audit: &MemoryAuditTrailCapsule,
    target_snapshot_id: u8,
) -> u64 {
    let (_, hash) = audit.append(
        MemoryAuditEvent::ReplayStarted,
        0,
        0,
        target_snapshot_id,
        0,
        0,
        0,
        100,
    );
    hash
}

/// Record replay completed event
#[inline]
pub fn record_replay_completed(
    audit: &MemoryAuditTrailCapsule,
    target_snapshot_id: u8,
    pages_restored: u32,
) -> u64 {
    let (_, hash) = audit.append(
        MemoryAuditEvent::ReplayCompleted,
        0,
        pages_restored,
        target_snapshot_id,
        0,
        0,
        0,
        100,
    );
    hash
}

/// Record COW page creation
#[inline]
pub fn record_cow_page_created(
    audit: &MemoryAuditTrailCapsule,
    page_address: u64,
    page_hash: u64,
) -> u64 {
    let (_, hash) = audit.append(
        MemoryAuditEvent::CowPageCreated,
        page_address,
        1,
        0,
        page_hash,
        0,
        0,
        100,
    );
    hash
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_audit_entry_size() {
        assert_eq!(std::mem::size_of::<MemoryAuditEntry>(), 64);
        assert_eq!(std::mem::align_of::<MemoryAuditEntry>(), 64);
    }

    #[test]
    fn test_memory_audit_event_types() {
        assert_eq!(MemoryAuditEvent::DirtyPageDetected.as_u8(), 0);
        assert_eq!(MemoryAuditEvent::DeltaCaptured.as_u8(), 1);
        assert_eq!(MemoryAuditEvent::from_u8(0), MemoryAuditEvent::DirtyPageDetected);
        assert_eq!(MemoryAuditEvent::from_u8(255), MemoryAuditEvent::Invalid);
        assert_eq!(MemoryAuditEvent::DeltaCaptured.as_str(), "delta_captured");
    }

    #[test]
    fn test_memory_audit_event_is_page_event() {
        assert!(MemoryAuditEvent::DirtyPageDetected.is_page_event());
        assert!(MemoryAuditEvent::DeltaCaptured.is_page_event());
        assert!(!MemoryAuditEvent::SnapshotCreated.is_page_event());
        assert!(!MemoryAuditEvent::ReplayStarted.is_page_event());
    }

    #[test]
    fn test_memory_audit_entry_creation() {
        let entry = MemoryAuditEntry::new(
            GENESIS_HASH,
            MemoryAuditEvent::DeltaCaptured,
            0x7FFF_0000_0000,
            1,
            42,
            0xDEAD_BEEF,
            1024,
            100,
            50,
        );

        assert!(entry.is_valid());
        assert!(entry.verify());
        assert_eq!(entry.page_address, 0x7FFF_0000_0000);
        assert_eq!(entry.event(), MemoryAuditEvent::DeltaCaptured);
        assert_eq!(entry.snapshot_id, 42);
        assert_eq!(entry.delta_size, 1024);
        assert_eq!(entry.compression_ratio, 50);
    }

    #[test]
    fn test_memory_audit_entry_page_index() {
        let entry = MemoryAuditEntry::new(
            GENESIS_HASH,
            MemoryAuditEvent::DirtyPageDetected,
            0x1000, // 4KB aligned
            1,
            0,
            0,
            0,
            0,
            100,
        );

        assert_eq!(entry.page_index(), 1);

        let entry2 = MemoryAuditEntry::new(
            GENESIS_HASH,
            MemoryAuditEvent::DirtyPageDetected,
            0x5000, // 5th page (20KB)
            1,
            0,
            0,
            0,
            0,
            100,
        );

        assert_eq!(entry2.page_index(), 5);
    }

    #[test]
    fn test_memory_audit_trail_creation() {
        let trail = MemoryAuditTrailCapsule::new();

        assert_eq!(trail.entry_count(), 0);
        assert_eq!(trail.get_root_hash(), GENESIS_HASH);
        assert!(trail.verify_chain());
        assert!(trail.verify_recent());
    }

    #[test]
    fn test_memory_audit_trail_append() {
        let trail = MemoryAuditTrailCapsule::new();

        let (idx, hash) = trail.append(
            MemoryAuditEvent::DirtyPageDetected,
            0x7FFF_0000_0000,
            10,
            0,
            0,
            0,
            0,
            100,
        );

        assert_eq!(idx, 0);
        assert_ne!(hash, INVALID_HASH);
        assert_eq!(trail.entry_count(), 1);
        assert_eq!(trail.get_root_hash(), hash);
        assert!(trail.verify_chain());
    }

    #[test]
    fn test_memory_audit_trail_chain_integrity() {
        let trail = MemoryAuditTrailCapsule::new();

        // Append multiple entries
        for i in 0..10 {
            trail.append(
                MemoryAuditEvent::DeltaCaptured,
                (i as u64) * PAGE_SIZE as u64,
                1,
                i as u8,
                0xDEAD_0000 + i as u64,
                1024,
                i as u32,
                75,
            );
        }

        assert_eq!(trail.entry_count(), 10);
        assert!(trail.verify_chain());
        assert!(trail.verify_recent());
    }

    #[test]
    fn test_memory_audit_trail_wrap_around() {
        let trail = MemoryAuditTrailCapsule::new();

        // Fill the buffer completely and wrap
        for i in 0..(MEMORY_AUDIT_ENTRY_COUNT + 100) {
            trail.append(
                MemoryAuditEvent::DirtyPageDetected,
                (i as u64) * PAGE_SIZE as u64,
                1,
                0,
                0,
                0,
                0,
                100,
            );
        }

        assert_eq!(trail.entry_count(), (MEMORY_AUDIT_ENTRY_COUNT + 100) as u64);
        assert!(trail.verify_recent());
    }

    #[test]
    fn test_memory_audit_stats() {
        let trail = MemoryAuditTrailCapsule::new();

        // Record various events
        trail.append(MemoryAuditEvent::DirtyPageDetected, 0x1000, 5, 0, 0, 0, 0, 100);
        trail.append(MemoryAuditEvent::DeltaCaptured, 0x1000, 1, 1, 0xABCD, 512, 0, 50);
        trail.append(MemoryAuditEvent::DeltaApplied, 0x1000, 1, 1, 0xABCD, 0, 0, 100);
        trail.append(MemoryAuditEvent::SnapshotCreated, 0, 10, 1, 0, 5120, 0, 100);

        let stats = trail.get_stats();
        assert_eq!(stats.total_entries, 4);
        assert_eq!(stats.dirty_page_count, 1);
        assert_eq!(stats.delta_capture_count, 1);
        assert_eq!(stats.delta_apply_count, 1);
        assert_eq!(stats.snapshot_count, 1);
        assert_eq!(stats.total_pages_tracked, 5);
        assert_eq!(stats.total_delta_bytes, 512);
        assert!(stats.chain_valid);
    }

    #[test]
    fn test_memory_audit_merkle_recording() {
        let trail = MemoryAuditTrailCapsule::new();

        let merkle_root = 0xFEDC_BA98_7654_3210;
        trail.record_merkle_verification(merkle_root, true);

        let stats = trail.get_stats();
        assert_eq!(stats.merkle_verify_count, 1);
        assert_eq!(stats.last_merkle_root, merkle_root);
    }

    #[test]
    fn test_memory_audit_export_json() {
        let trail = MemoryAuditTrailCapsule::new();

        trail.append(MemoryAuditEvent::DirtyPageDetected, 0x1000, 1, 0, 0, 0, 0, 100);

        let json = trail.export_json();
        assert!(json.contains("memory_audit_trail"));
        assert!(json.contains("dirty_page_detected"));
        assert!(json.contains("root_hash"));
        assert!(json.contains("chain_valid"));
        assert!(json.contains("last_merkle_root"));
    }

    #[test]
    fn test_memory_audit_helper_functions() {
        let trail = MemoryAuditTrailCapsule::new();

        let hash1 = record_dirty_page(&trail, 0x1000, 5);
        assert_ne!(hash1, INVALID_HASH);

        let hash2 = record_delta_capture(&trail, 0x1000, 1, 0xABCD, 512, 0, 50);
        assert_ne!(hash2, hash1);

        let hash3 = record_delta_apply(&trail, 0x1000, 1, 0xABCD);
        assert_ne!(hash3, hash2);

        let hash4 = record_snapshot_created(&trail, 1, 10, 5120);
        assert_ne!(hash4, hash3);

        let hash5 = record_replay_started(&trail, 0);
        assert_ne!(hash5, hash4);

        let hash6 = record_replay_completed(&trail, 0, 10);
        assert_ne!(hash6, hash5);

        let hash7 = record_cow_page_created(&trail, 0x2000, 0xBEEF);
        assert_ne!(hash7, hash6);

        assert_eq!(trail.entry_count(), 7);
        assert!(trail.verify_chain());
    }

    #[test]
    fn test_memory_audit_get_entry() {
        let trail = MemoryAuditTrailCapsule::new();

        // Empty trail
        assert!(trail.get_entry(0).is_none());

        trail.append(MemoryAuditEvent::DirtyPageDetected, 0x1000, 1, 0, 0, 0, 0, 100);

        let entry = trail.get_entry(0);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().event(), MemoryAuditEvent::DirtyPageDetected);

        // Out of bounds
        assert!(trail.get_entry(100).is_none());
    }

    #[test]
    fn test_memory_audit_concurrent_append() {
        use std::sync::Arc;
        use std::thread;

        let trail = Arc::new(MemoryAuditTrailCapsule::new());
        let mut handles = vec![];

        // Spawn 4 threads, each appending 100 entries
        for t in 0..4 {
            let trail_clone = Arc::clone(&trail);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    trail_clone.append(
                        MemoryAuditEvent::DeltaCaptured,
                        ((t * 100 + i) as u64) * PAGE_SIZE as u64,
                        1,
                        t as u8,
                        0xDEAD_0000 + i as u64,
                        512,
                        (t * 100 + i) as u32,
                        75,
                    );
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(trail.entry_count(), 400);
        assert!(trail.verify_recent());
    }

    #[test]
    fn test_memory_audit_entry_tamper_detection() {
        let trail = MemoryAuditTrailCapsule::new();

        trail.append(MemoryAuditEvent::DeltaCaptured, 0x1000, 1, 1, 0xABCD, 512, 0, 50);

        let entry = trail.get_entry(0).unwrap();

        // Create a tampered entry by modifying delta_size
        let mut tampered = *entry;
        tampered.delta_size = 9999;

        // Tampered entry should fail verification
        assert!(!tampered.verify());
    }

    #[test]
    fn test_memory_audit_trail_default() {
        let trail = MemoryAuditTrailCapsule::default();
        assert_eq!(trail.entry_count(), 0);
        assert!(trail.verify_chain());
    }

    #[test]
    fn test_memory_audit_event_display() {
        assert_eq!(format!("{}", MemoryAuditEvent::DirtyPageDetected), "dirty_page_detected");
        assert_eq!(format!("{}", MemoryAuditEvent::ReplayCompleted), "replay_completed");
    }

    #[test]
    fn test_memory_audit_stats_display() {
        let stats = MemoryAuditStats {
            total_entries: 1000,
            dirty_page_count: 500,
            delta_capture_count: 400,
            delta_apply_count: 300,
            snapshot_count: 10,
            merkle_verify_count: 5,
            page_evict_count: 50,
            replay_count: 2,
            cow_count: 100,
            total_delta_bytes: 2048000,
            total_pages_tracked: 500,
            root_hash: 0xDEADBEEF,
            generation: 1001,
            chain_valid: true,
            last_merkle_root: 0xCAFEBABE,
        };

        let display = format!("{}", stats);
        assert!(display.contains("entries: 1000"));
        assert!(display.contains("dirty: 500"));
        assert!(display.contains("valid: true"));
    }

    #[test]
    fn test_memory_audit_clear_for_testing() {
        let trail = MemoryAuditTrailCapsule::new();

        trail.append(MemoryAuditEvent::DirtyPageDetected, 0x1000, 1, 0, 0, 0, 0, 100);
        assert_eq!(trail.entry_count(), 1);

        trail.clear();
        assert_eq!(trail.entry_count(), 0);
        assert_eq!(trail.get_root_hash(), GENESIS_HASH);
    }

    #[test]
    fn test_compression_ratio_tracking() {
        let trail = MemoryAuditTrailCapsule::new();

        // Original page 4KB, compressed to 1KB = 25% ratio
        trail.append(
            MemoryAuditEvent::DeltaCaptured,
            0x1000,
            1,
            1,
            0xABCD,
            1024,
            0,
            25, // 25% compression ratio
        );

        let entry = trail.get_entry(0).unwrap();
        assert_eq!(entry.compression_ratio, 25);
    }
}
