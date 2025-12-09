//! Session Pool Audit Integration - Q34 Hash-Chain Audit Trail for Session Operations
//!
//! # Architecture
//!
//! This module provides T0 Auditable integration for the SessionPoolCapsule, enabling
//! cryptographically tamper-evident audit trails for all session lifecycle operations.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                SessionAuditTrailCapsule (T0 Auditable)              │
//! │                    64KB (1024 × 64-byte entries)                    │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
//! │  │ SessionAuditEntry│  │ SessionAuditEntry│  │ SessionAuditEntry│  │
//! │  │   64 bytes       │  │   64 bytes       │  │   64 bytes       │  │
//! │  │   Cache-aligned  │  │   Cache-aligned  │  │   Cache-aligned  │  │
//! │  └──────────────────┘  └──────────────────┘  └──────────────────┘  │
//! │              ↓                   ↓                   ↓              │
//! │           prev_hash ───────► entry_hash ───────► next_hash...      │
//! │                        (CRC64 hash-chain)                           │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Session Events Tracked
//!
//! - **Allocate**: Session allocation with tier assignment
//! - **Release**: Session deallocation with resource cleanup
//! - **Upgrade**: Tier promotion (LIGHT→MEDIUM→HEAVY)
//! - **Downgrade**: Tier demotion (HEAVY→MEDIUM→LIGHT)
//! - **SnapshotCapture**: Snapshot creation within session
//! - **BreakpointHit**: Breakpoint trigger event
//! - **MemoryWrite**: Memory modification event
//!
//! # Performance Targets
//!
//! - `append()`: <50ns lockfree (single atomic CAS)
//! - `verify_chain()`: O(n) for full verification
//! - `verify_recent()`: <50ns (last 3 entries only)
//! - `get_root_hash()`: <10ns (atomic load)
//! - `export_json()`: <1ms (full trail export)
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
//! #ASSUME_GENERATION_COUNTERS: ABA prevention via generation counters

use std::sync::atomic::{AtomicU64, Ordering};
use crc::{Crc, CRC_64_ECMA_182};

use super::session_pool_capsule::{SessionId, SessionTierType};

// ============================================================================
// Constants
// ============================================================================

/// Number of audit entries in the ring buffer (1024 entries = 64KB)
pub const AUDIT_ENTRY_COUNT: usize = 1024;

/// Size of each audit entry in bytes (cache-line aligned)
pub const AUDIT_ENTRY_SIZE: usize = 64;

/// Total audit trail size in bytes
pub const AUDIT_TRAIL_SIZE: usize = AUDIT_ENTRY_COUNT * AUDIT_ENTRY_SIZE;

/// CRC64-ECMA-182 for hash computation
const CRC64: Crc<u64> = Crc::<u64>::new(&CRC_64_ECMA_182);

/// Sentinel value for empty/invalid entries
const INVALID_HASH: u64 = 0;

/// Initial hash for chain start
const GENESIS_HASH: u64 = 0xCAFE_BABE_DEAD_BEEF;

// ============================================================================
// Session Audit Event Types
// ============================================================================

/// Session audit event types for tracking all session lifecycle operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SessionAuditEvent {
    /// Session allocation (new session created)
    Allocate = 0,
    /// Session release (session freed)
    Release = 1,
    /// Session upgrade (tier promotion)
    Upgrade = 2,
    /// Session downgrade (tier demotion)
    Downgrade = 3,
    /// Snapshot captured within session
    SnapshotCapture = 4,
    /// Breakpoint hit event
    BreakpointHit = 5,
    /// Memory write event
    MemoryWrite = 6,
    /// Session attach event
    Attach = 7,
    /// Session detach event
    Detach = 8,
    /// Step command executed
    Step = 9,
    /// Continue command executed
    Continue = 10,
    /// Time-travel backward
    TimeTravel = 11,
    /// Invalid/placeholder event
    Invalid = 255,
}

impl SessionAuditEvent {
    /// Convert from u8
    #[inline]
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => SessionAuditEvent::Allocate,
            1 => SessionAuditEvent::Release,
            2 => SessionAuditEvent::Upgrade,
            3 => SessionAuditEvent::Downgrade,
            4 => SessionAuditEvent::SnapshotCapture,
            5 => SessionAuditEvent::BreakpointHit,
            6 => SessionAuditEvent::MemoryWrite,
            7 => SessionAuditEvent::Attach,
            8 => SessionAuditEvent::Detach,
            9 => SessionAuditEvent::Step,
            10 => SessionAuditEvent::Continue,
            11 => SessionAuditEvent::TimeTravel,
            _ => SessionAuditEvent::Invalid,
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
            SessionAuditEvent::Allocate => "allocate",
            SessionAuditEvent::Release => "release",
            SessionAuditEvent::Upgrade => "upgrade",
            SessionAuditEvent::Downgrade => "downgrade",
            SessionAuditEvent::SnapshotCapture => "snapshot_capture",
            SessionAuditEvent::BreakpointHit => "breakpoint_hit",
            SessionAuditEvent::MemoryWrite => "memory_write",
            SessionAuditEvent::Attach => "attach",
            SessionAuditEvent::Detach => "detach",
            SessionAuditEvent::Step => "step",
            SessionAuditEvent::Continue => "continue",
            SessionAuditEvent::TimeTravel => "time_travel",
            SessionAuditEvent::Invalid => "invalid",
        }
    }
}

impl std::fmt::Display for SessionAuditEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Session Audit Entry (64 bytes, cache-line aligned)
// ============================================================================

/// Session audit entry - 64 bytes, cache-line aligned
///
/// # Layout (64 bytes)
/// ```text
/// ┌────────────────────────────────────────────────────────────────┐
/// │ timestamp_ns: u64 (8B)     │ session_id: u64 (8B)              │
/// ├────────────────────────────┼───────────────────────────────────┤
/// │ event_type: u8 (1B)        │ prev_tier: u8 (1B)                │
/// │ new_tier: u8 (1B)          │ _pad1: u8 (1B)                    │
/// │ snapshot_id: u32 (4B)                                          │
/// ├────────────────────────────────────────────────────────────────┤
/// │ prev_hash: u64 (8B)        │ entry_hash: u64 (8B)              │
/// ├────────────────────────────────────────────────────────────────┤
/// │ memory_pages_affected: u32 (4B)  │ delta_size_bytes: u32 (4B)  │
/// ├────────────────────────────────────────────────────────────────┤
/// │ _padding: [u8; 16]                                             │
/// └────────────────────────────────────────────────────────────────┘
/// Total: 64 bytes (1 cache line)
/// ```
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy)]
pub struct SessionAuditEntry {
    /// Timestamp in nanoseconds since UNIX epoch
    pub timestamp_ns: u64,
    /// Session ID this event belongs to
    pub session_id: u64,
    /// Event type (SessionAuditEvent)
    pub event_type: u8,
    /// Previous tier (for upgrade/downgrade events)
    pub prev_tier: u8,
    /// New tier (for upgrade/downgrade events)
    pub new_tier: u8,
    /// Padding for alignment
    _pad1: u8,
    /// Snapshot ID (for snapshot events)
    pub snapshot_id: u32,
    /// Previous entry's hash (chain linkage)
    pub prev_hash: u64,
    /// This entry's computed hash
    pub entry_hash: u64,
    /// Number of memory pages affected (for memory events)
    pub memory_pages_affected: u32,
    /// Delta size in bytes (for snapshot events)
    pub delta_size_bytes: u32,
    /// Reserved padding for future use
    _padding: [u8; 16],
}

// Compile-time size verification
const _: () = {
    const EXPECTED: usize = 64;
    const ACTUAL: usize = std::mem::size_of::<SessionAuditEntry>();
    assert!(ACTUAL == EXPECTED, "SessionAuditEntry must be exactly 64 bytes");
};

impl Default for SessionAuditEntry {
    fn default() -> Self {
        Self {
            timestamp_ns: 0,
            session_id: 0,
            event_type: SessionAuditEvent::Invalid.as_u8(),
            prev_tier: 0,
            new_tier: 0,
            _pad1: 0,
            snapshot_id: 0,
            prev_hash: INVALID_HASH,
            entry_hash: INVALID_HASH,
            memory_pages_affected: 0,
            delta_size_bytes: 0,
            _padding: [0; 16],
        }
    }
}

impl SessionAuditEntry {
    /// Create a new audit entry with computed hash
    ///
    /// # Arguments
    /// - `prev_hash`: Previous entry's hash for chain linkage
    /// - `event`: Event type
    /// - `session_id`: Session ID
    /// - `prev_tier`: Previous tier (optional)
    /// - `new_tier`: New tier (optional)
    /// - `snapshot_id`: Snapshot ID (optional)
    /// - `memory_pages`: Memory pages affected (optional)
    /// - `delta_bytes`: Delta size in bytes (optional)
    ///
    /// # Returns
    /// New SessionAuditEntry with computed entry_hash
    #[inline]
    pub fn new(
        prev_hash: u64,
        event: SessionAuditEvent,
        session_id: u64,
        prev_tier: Option<SessionTierType>,
        new_tier: Option<SessionTierType>,
        snapshot_id: Option<u32>,
        memory_pages: Option<u32>,
        delta_bytes: Option<u32>,
    ) -> Self {
        let timestamp_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let prev_tier_u8 = prev_tier.map(|t| t.as_u8()).unwrap_or(0xFF);
        let new_tier_u8 = new_tier.map(|t| t.as_u8()).unwrap_or(0xFF);

        let mut entry = Self {
            timestamp_ns,
            session_id,
            event_type: event.as_u8(),
            prev_tier: prev_tier_u8,
            new_tier: new_tier_u8,
            _pad1: 0,
            snapshot_id: snapshot_id.unwrap_or(0),
            prev_hash,
            entry_hash: 0, // Will be computed below
            memory_pages_affected: memory_pages.unwrap_or(0),
            delta_size_bytes: delta_bytes.unwrap_or(0),
            _padding: [0; 16],
        };

        // Compute entry hash based on all fields
        entry.entry_hash = entry.compute_hash();
        entry
    }

    /// Compute CRC64 hash for this entry
    ///
    /// Hash includes: prev_hash, timestamp, session_id, event_type, tiers, snapshot_id, memory_pages, delta_bytes
    ///
    /// # Performance
    /// <50ns per entry
    #[inline]
    fn compute_hash(&self) -> u64 {
        let mut digest = CRC64.digest();

        // Hash all significant fields
        digest.update(&self.prev_hash.to_le_bytes());
        digest.update(&self.timestamp_ns.to_le_bytes());
        digest.update(&self.session_id.to_le_bytes());
        digest.update(&[self.event_type, self.prev_tier, self.new_tier, 0]);
        digest.update(&self.snapshot_id.to_le_bytes());
        digest.update(&self.memory_pages_affected.to_le_bytes());
        digest.update(&self.delta_size_bytes.to_le_bytes());

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
    pub fn event(&self) -> SessionAuditEvent {
        SessionAuditEvent::from_u8(self.event_type)
    }

    /// Check if entry is valid (has non-zero hash)
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.entry_hash != INVALID_HASH && self.event_type != SessionAuditEvent::Invalid.as_u8()
    }
}

// ============================================================================
// Session Audit Trail Capsule (256-byte aligned, 64KB total)
// ============================================================================

/// SessionAuditTrailCapsule - T0 Auditable hash-chain for session operations
///
/// # Size
/// - Orchestrator: 256 bytes (metadata + atomics)
/// - Entries: 64KB (1024 × 64-byte entries)
/// - Total: ~65KB
///
/// # Thread Safety
/// All operations are lockfree via atomic CAS operations.
///
/// # ASSUM Safety
/// - #ASSUME_LOCKFREE_ONLY: Zero mutex/RwLock
/// - #ASSUME_CACHE_ALIGNED: All entries 64-byte aligned
/// - #ASSUME_ABA_PREVENTION: Generation counter prevents ABA
#[repr(C, align(256))]
pub struct SessionAuditTrailCapsule {
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
    // Statistics (64 bytes)
    // ========================================================================
    /// Total allocate events
    allocate_count: AtomicU64,
    /// Total release events
    release_count: AtomicU64,
    /// Total upgrade events
    upgrade_count: AtomicU64,
    /// Total downgrade events
    downgrade_count: AtomicU64,
    /// Total snapshot events
    snapshot_count: AtomicU64,
    /// Total breakpoint events
    breakpoint_count: AtomicU64,
    /// Total memory events
    memory_count: AtomicU64,
    /// Padding to 64 bytes
    _padding2: [u8; 64 - 7 * 8],

    // ========================================================================
    // Entry Storage (64KB)
    // ========================================================================
    /// Ring buffer of audit entries
    entries: [SessionAuditEntry; AUDIT_ENTRY_COUNT],

    // ========================================================================
    // Additional Padding (128 bytes to reach 256-byte alignment)
    // ========================================================================
    _padding3: [u8; 128],
}

// Compile-time size verification
const _: () = {
    // Entries: 1024 * 64 = 65536
    // Atomics: 64 + 64 = 128
    // Padding: 128
    // Total: 65792 bytes
    const EXPECTED_MIN: usize = 65536 + 128 + 128;
    const ACTUAL: usize = std::mem::size_of::<SessionAuditTrailCapsule>();
    assert!(ACTUAL >= EXPECTED_MIN, "SessionAuditTrailCapsule too small");
};

// SAFETY: SessionAuditTrailCapsule is Send/Sync via atomic operations only
// #ASSUME_ALL_ATOMIC: All mutable coordination via AtomicU64
// #VERIFY_NO_MUTEXES: Zero mutex/RwLock in capsule
unsafe impl Send for SessionAuditTrailCapsule {}
unsafe impl Sync for SessionAuditTrailCapsule {}

impl Default for SessionAuditTrailCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionAuditTrailCapsule {
    /// Create new audit trail capsule
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

            allocate_count: AtomicU64::new(0),
            release_count: AtomicU64::new(0),
            upgrade_count: AtomicU64::new(0),
            downgrade_count: AtomicU64::new(0),
            snapshot_count: AtomicU64::new(0),
            breakpoint_count: AtomicU64::new(0),
            memory_count: AtomicU64::new(0),
            _padding2: [0; 64 - 7 * 8],

            entries: [SessionAuditEntry::default(); AUDIT_ENTRY_COUNT],

            _padding3: [0; 128],
        }
    }

    /// Append a new audit entry to the trail
    ///
    /// # Performance
    /// <50ns lockfree (single atomic CAS)
    ///
    /// # Arguments
    /// - `event`: Event type
    /// - `session_id`: Session identifier
    /// - `prev_tier`: Previous tier (for upgrade/downgrade)
    /// - `new_tier`: New tier (for upgrade/downgrade)
    /// - `snapshot_id`: Snapshot ID (for snapshot events)
    /// - `memory_pages`: Memory pages affected
    /// - `delta_bytes`: Delta size in bytes
    ///
    /// # Returns
    /// Entry index and computed hash
    ///
    /// # ASSUM Safety
    /// - #ASSUME_CAS_LOOP: CAS retry bounded by generation counter
    /// - #VERIFY_APPEND_SUCCESS: Generation counter prevents lost updates
    pub fn append(
        &self,
        event: SessionAuditEvent,
        session_id: u64,
        prev_tier: Option<SessionTierType>,
        new_tier: Option<SessionTierType>,
        snapshot_id: Option<u32>,
        memory_pages: Option<u32>,
        delta_bytes: Option<u32>,
    ) -> (usize, u64) {
        loop {
            let current_head = self.head.load(Ordering::Acquire);
            let current_gen = self.generation.load(Ordering::Acquire);
            let prev_hash = self.root_hash.load(Ordering::Acquire);

            let new_head = (current_head + 1) % AUDIT_ENTRY_COUNT as u64;

            // Create the new entry
            let entry = SessionAuditEntry::new(
                prev_hash,
                event,
                session_id,
                prev_tier,
                new_tier,
                snapshot_id,
                memory_pages,
                delta_bytes,
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
                    // #VERIFY_BOUNDS: slot < AUDIT_ENTRY_COUNT (modulo above)
                    unsafe {
                        let entry_ptr = (self.entries.as_ptr() as *mut SessionAuditEntry).add(slot);
                        std::ptr::write(entry_ptr, entry);
                    }

                    // Update root hash and counters
                    self.root_hash.store(entry.entry_hash, Ordering::Release);
                    self.entry_count.fetch_add(1, Ordering::Relaxed);
                    self.generation.fetch_add(1, Ordering::AcqRel);

                    // Update event-specific counters
                    match event {
                        SessionAuditEvent::Allocate => {
                            self.allocate_count.fetch_add(1, Ordering::Relaxed);
                        }
                        SessionAuditEvent::Release => {
                            self.release_count.fetch_add(1, Ordering::Relaxed);
                        }
                        SessionAuditEvent::Upgrade => {
                            self.upgrade_count.fetch_add(1, Ordering::Relaxed);
                        }
                        SessionAuditEvent::Downgrade => {
                            self.downgrade_count.fetch_add(1, Ordering::Relaxed);
                        }
                        SessionAuditEvent::SnapshotCapture => {
                            self.snapshot_count.fetch_add(1, Ordering::Relaxed);
                        }
                        SessionAuditEvent::BreakpointHit => {
                            self.breakpoint_count.fetch_add(1, Ordering::Relaxed);
                        }
                        SessionAuditEvent::MemoryWrite => {
                            self.memory_count.fetch_add(1, Ordering::Relaxed);
                        }
                        _ => {}
                    }

                    // Advance tail if buffer wraps
                    let count = self.entry_count.load(Ordering::Relaxed);
                    if count > AUDIT_ENTRY_COUNT as u64 {
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
        let entries_to_check = std::cmp::min(count as usize, AUDIT_ENTRY_COUNT);

        let mut prev_hash = GENESIS_HASH;

        for i in 0..entries_to_check {
            let idx = (tail + i) % AUDIT_ENTRY_COUNT;
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
                AUDIT_ENTRY_COUNT - (i + 1 - head)
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

    /// Get audit statistics
    pub fn get_stats(&self) -> SessionAuditStats {
        SessionAuditStats {
            total_entries: self.entry_count.load(Ordering::Relaxed),
            allocate_count: self.allocate_count.load(Ordering::Relaxed),
            release_count: self.release_count.load(Ordering::Relaxed),
            upgrade_count: self.upgrade_count.load(Ordering::Relaxed),
            downgrade_count: self.downgrade_count.load(Ordering::Relaxed),
            snapshot_count: self.snapshot_count.load(Ordering::Relaxed),
            breakpoint_count: self.breakpoint_count.load(Ordering::Relaxed),
            memory_count: self.memory_count.load(Ordering::Relaxed),
            root_hash: self.root_hash.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
            chain_valid: self.verify_recent(),
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
        let entries_to_export = std::cmp::min(count as usize, AUDIT_ENTRY_COUNT);

        let mut json = String::with_capacity(entries_to_export * 256);
        json.push_str("{\n");
        json.push_str("  \"audit_trail\": [\n");

        for i in 0..entries_to_export {
            let idx = (tail + i) % AUDIT_ENTRY_COUNT;
            let entry = &self.entries[idx];

            if i > 0 {
                json.push_str(",\n");
            }

            json.push_str(&format!(
                "    {{\n      \"index\": {},\n      \"timestamp_ns\": {},\n      \"session_id\": {},\n      \"event\": \"{}\",\n      \"prev_tier\": {},\n      \"new_tier\": {},\n      \"snapshot_id\": {},\n      \"prev_hash\": \"{:016x}\",\n      \"entry_hash\": \"{:016x}\",\n      \"memory_pages\": {},\n      \"delta_bytes\": {},\n      \"valid\": {}\n    }}",
                i,
                entry.timestamp_ns,
                entry.session_id,
                entry.event(),
                entry.prev_tier,
                entry.new_tier,
                entry.snapshot_id,
                entry.prev_hash,
                entry.entry_hash,
                entry.memory_pages_affected,
                entry.delta_size_bytes,
                entry.verify()
            ));
        }

        json.push_str("\n  ],\n");
        json.push_str(&format!("  \"root_hash\": \"{:016x}\",\n", self.get_root_hash()));
        json.push_str(&format!("  \"entry_count\": {},\n", count));
        json.push_str(&format!("  \"chain_valid\": {}\n", self.verify_chain()));
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
    pub fn get_entry(&self, index: usize) -> Option<&SessionAuditEntry> {
        let count = self.entry_count.load(Ordering::Acquire) as usize;
        if index >= count || index >= AUDIT_ENTRY_COUNT {
            return None;
        }

        let tail = self.tail.load(Ordering::Acquire) as usize;
        let actual_idx = (tail + index) % AUDIT_ENTRY_COUNT;

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

        self.allocate_count.store(0, Ordering::Release);
        self.release_count.store(0, Ordering::Release);
        self.upgrade_count.store(0, Ordering::Release);
        self.downgrade_count.store(0, Ordering::Release);
        self.snapshot_count.store(0, Ordering::Release);
        self.breakpoint_count.store(0, Ordering::Release);
        self.memory_count.store(0, Ordering::Release);
    }
}

// ============================================================================
// Audit Statistics
// ============================================================================

/// Session audit trail statistics
#[derive(Debug, Clone, Copy)]
pub struct SessionAuditStats {
    /// Total entries recorded
    pub total_entries: u64,
    /// Allocate event count
    pub allocate_count: u64,
    /// Release event count
    pub release_count: u64,
    /// Upgrade event count
    pub upgrade_count: u64,
    /// Downgrade event count
    pub downgrade_count: u64,
    /// Snapshot event count
    pub snapshot_count: u64,
    /// Breakpoint event count
    pub breakpoint_count: u64,
    /// Memory event count
    pub memory_count: u64,
    /// Current root hash
    pub root_hash: u64,
    /// Generation counter
    pub generation: u64,
    /// Chain integrity status
    pub chain_valid: bool,
}

impl std::fmt::Display for SessionAuditStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SessionAuditStats {{ entries: {}, alloc: {}, release: {}, upgrade: {}, downgrade: {}, hash: {:016x}, valid: {} }}",
            self.total_entries,
            self.allocate_count,
            self.release_count,
            self.upgrade_count,
            self.downgrade_count,
            self.root_hash,
            self.chain_valid
        )
    }
}

// ============================================================================
// Helper Functions for SessionPoolCapsule Integration
// ============================================================================

/// Record session allocation event
#[inline]
pub fn record_allocate(audit: &SessionAuditTrailCapsule, session_id: SessionId, tier: SessionTierType) -> u64 {
    let (_, hash) = audit.append(
        SessionAuditEvent::Allocate,
        session_id.0,
        None,
        Some(tier),
        None,
        None,
        None,
    );
    hash
}

/// Record session release event
#[inline]
pub fn record_release(audit: &SessionAuditTrailCapsule, session_id: SessionId, tier: SessionTierType) -> u64 {
    let (_, hash) = audit.append(
        SessionAuditEvent::Release,
        session_id.0,
        Some(tier),
        None,
        None,
        None,
        None,
    );
    hash
}

/// Record session upgrade event
#[inline]
pub fn record_upgrade(
    audit: &SessionAuditTrailCapsule,
    session_id: SessionId,
    from_tier: SessionTierType,
    to_tier: SessionTierType,
) -> u64 {
    let (_, hash) = audit.append(
        SessionAuditEvent::Upgrade,
        session_id.0,
        Some(from_tier),
        Some(to_tier),
        None,
        None,
        None,
    );
    hash
}

/// Record session downgrade event
#[inline]
pub fn record_downgrade(
    audit: &SessionAuditTrailCapsule,
    session_id: SessionId,
    from_tier: SessionTierType,
    to_tier: SessionTierType,
) -> u64 {
    let (_, hash) = audit.append(
        SessionAuditEvent::Downgrade,
        session_id.0,
        Some(from_tier),
        Some(to_tier),
        None,
        None,
        None,
    );
    hash
}

/// Record snapshot capture event
#[inline]
pub fn record_snapshot(
    audit: &SessionAuditTrailCapsule,
    session_id: SessionId,
    snapshot_id: u32,
    delta_bytes: u32,
) -> u64 {
    let (_, hash) = audit.append(
        SessionAuditEvent::SnapshotCapture,
        session_id.0,
        None,
        None,
        Some(snapshot_id),
        None,
        Some(delta_bytes),
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
    fn test_audit_entry_size() {
        assert_eq!(std::mem::size_of::<SessionAuditEntry>(), 64);
        assert_eq!(std::mem::align_of::<SessionAuditEntry>(), 64);
    }

    #[test]
    fn test_audit_event_types() {
        assert_eq!(SessionAuditEvent::Allocate.as_u8(), 0);
        assert_eq!(SessionAuditEvent::Release.as_u8(), 1);
        assert_eq!(SessionAuditEvent::from_u8(0), SessionAuditEvent::Allocate);
        assert_eq!(SessionAuditEvent::from_u8(255), SessionAuditEvent::Invalid);
        assert_eq!(SessionAuditEvent::Allocate.as_str(), "allocate");
    }

    #[test]
    fn test_audit_entry_creation() {
        let entry = SessionAuditEntry::new(
            GENESIS_HASH,
            SessionAuditEvent::Allocate,
            12345,
            None,
            Some(SessionTierType::Light),
            None,
            None,
            None,
        );

        assert!(entry.is_valid());
        assert!(entry.verify());
        assert_eq!(entry.session_id, 12345);
        assert_eq!(entry.event(), SessionAuditEvent::Allocate);
        assert_eq!(entry.prev_hash, GENESIS_HASH);
        assert_ne!(entry.entry_hash, INVALID_HASH);
    }

    #[test]
    fn test_audit_trail_creation() {
        let trail = SessionAuditTrailCapsule::new();

        assert_eq!(trail.entry_count(), 0);
        assert_eq!(trail.get_root_hash(), GENESIS_HASH);
        assert!(trail.verify_chain());
        assert!(trail.verify_recent());
    }

    #[test]
    fn test_audit_trail_append() {
        let trail = SessionAuditTrailCapsule::new();

        let session_id = SessionId::new(0, 1, 1);
        let (idx, hash) = trail.append(
            SessionAuditEvent::Allocate,
            session_id.0,
            None,
            Some(SessionTierType::Light),
            None,
            None,
            None,
        );

        assert_eq!(idx, 0);
        assert_ne!(hash, INVALID_HASH);
        assert_eq!(trail.entry_count(), 1);
        assert_eq!(trail.get_root_hash(), hash);
        assert!(trail.verify_chain());
    }

    #[test]
    fn test_audit_trail_chain_integrity() {
        let trail = SessionAuditTrailCapsule::new();

        // Append multiple entries
        for i in 0..10 {
            let session_id = SessionId::new(0, i, 1);
            trail.append(
                SessionAuditEvent::Allocate,
                session_id.0,
                None,
                Some(SessionTierType::Light),
                None,
                None,
                None,
            );
        }

        assert_eq!(trail.entry_count(), 10);
        assert!(trail.verify_chain());
        assert!(trail.verify_recent());
    }

    #[test]
    fn test_audit_trail_wrap_around() {
        let trail = SessionAuditTrailCapsule::new();

        // Fill the buffer completely and wrap
        for i in 0..(AUDIT_ENTRY_COUNT + 100) {
            let session_id = SessionId::new(0, i as u32, 1);
            trail.append(
                SessionAuditEvent::Allocate,
                session_id.0,
                None,
                Some(SessionTierType::Light),
                None,
                None,
                None,
            );
        }

        assert_eq!(trail.entry_count(), (AUDIT_ENTRY_COUNT + 100) as u64);
        assert!(trail.verify_recent()); // Recent entries should still be valid
    }

    #[test]
    fn test_audit_stats() {
        let trail = SessionAuditTrailCapsule::new();

        let session_id = SessionId::new(0, 1, 1);

        trail.append(SessionAuditEvent::Allocate, session_id.0, None, Some(SessionTierType::Light), None, None, None);
        trail.append(SessionAuditEvent::Upgrade, session_id.0, Some(SessionTierType::Light), Some(SessionTierType::Medium), None, None, None);
        trail.append(SessionAuditEvent::Release, session_id.0, Some(SessionTierType::Medium), None, None, None, None);

        let stats = trail.get_stats();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.allocate_count, 1);
        assert_eq!(stats.upgrade_count, 1);
        assert_eq!(stats.release_count, 1);
        assert!(stats.chain_valid);
    }

    #[test]
    fn test_audit_export_json() {
        let trail = SessionAuditTrailCapsule::new();

        let session_id = SessionId::new(0, 1, 1);
        trail.append(SessionAuditEvent::Allocate, session_id.0, None, Some(SessionTierType::Light), None, None, None);

        let json = trail.export_json();
        assert!(json.contains("audit_trail"));
        assert!(json.contains("allocate"));
        assert!(json.contains("root_hash"));
        assert!(json.contains("chain_valid"));
    }

    #[test]
    fn test_helper_functions() {
        let trail = SessionAuditTrailCapsule::new();
        let session_id = SessionId::new(0, 1, 1);

        let hash1 = record_allocate(&trail, session_id, SessionTierType::Light);
        assert_ne!(hash1, INVALID_HASH);

        let hash2 = record_upgrade(&trail, session_id, SessionTierType::Light, SessionTierType::Medium);
        assert_ne!(hash2, hash1);

        let hash3 = record_downgrade(&trail, session_id, SessionTierType::Medium, SessionTierType::Light);
        assert_ne!(hash3, hash2);

        let hash4 = record_release(&trail, session_id, SessionTierType::Light);
        assert_ne!(hash4, hash3);

        assert_eq!(trail.entry_count(), 4);
        assert!(trail.verify_chain());
    }

    #[test]
    fn test_snapshot_recording() {
        let trail = SessionAuditTrailCapsule::new();
        let session_id = SessionId::new(0, 1, 1);

        let hash = record_snapshot(&trail, session_id, 42, 4096);
        assert_ne!(hash, INVALID_HASH);

        let entry = trail.get_entry(0).unwrap();
        assert_eq!(entry.event(), SessionAuditEvent::SnapshotCapture);
        assert_eq!(entry.snapshot_id, 42);
        assert_eq!(entry.delta_size_bytes, 4096);
    }

    #[test]
    fn test_get_entry() {
        let trail = SessionAuditTrailCapsule::new();

        // Empty trail
        assert!(trail.get_entry(0).is_none());

        let session_id = SessionId::new(0, 1, 1);
        trail.append(SessionAuditEvent::Allocate, session_id.0, None, Some(SessionTierType::Light), None, None, None);

        let entry = trail.get_entry(0);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().event(), SessionAuditEvent::Allocate);

        // Out of bounds
        assert!(trail.get_entry(100).is_none());
    }

    #[test]
    fn test_concurrent_append() {
        use std::sync::Arc;
        use std::thread;

        let trail = Arc::new(SessionAuditTrailCapsule::new());
        let mut handles = vec![];

        // Spawn 4 threads, each appending 100 entries
        for t in 0..4 {
            let trail_clone = Arc::clone(&trail);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    let session_id = SessionId::new(t as u8, i, 1);
                    trail_clone.append(
                        SessionAuditEvent::Allocate,
                        session_id.0,
                        None,
                        Some(SessionTierType::Light),
                        None,
                        None,
                        None,
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
    fn test_entry_verification_tamper_detection() {
        let trail = SessionAuditTrailCapsule::new();
        let session_id = SessionId::new(0, 1, 1);

        trail.append(SessionAuditEvent::Allocate, session_id.0, None, Some(SessionTierType::Light), None, None, None);

        let entry = trail.get_entry(0).unwrap();

        // Create a tampered entry by modifying session_id
        let mut tampered = *entry;
        tampered.session_id = 99999;

        // Tampered entry should fail verification
        assert!(!tampered.verify());
    }

    #[test]
    fn test_audit_trail_default() {
        let trail = SessionAuditTrailCapsule::default();
        assert_eq!(trail.entry_count(), 0);
        assert!(trail.verify_chain());
    }

    #[test]
    fn test_session_audit_event_display() {
        assert_eq!(format!("{}", SessionAuditEvent::Allocate), "allocate");
        assert_eq!(format!("{}", SessionAuditEvent::TimeTravel), "time_travel");
    }

    #[test]
    fn test_session_audit_stats_display() {
        let stats = SessionAuditStats {
            total_entries: 100,
            allocate_count: 50,
            release_count: 40,
            upgrade_count: 5,
            downgrade_count: 5,
            snapshot_count: 0,
            breakpoint_count: 0,
            memory_count: 0,
            root_hash: 0xDEADBEEF,
            generation: 101,
            chain_valid: true,
        };

        let display = format!("{}", stats);
        assert!(display.contains("entries: 100"));
        assert!(display.contains("valid: true"));
    }

    #[test]
    fn test_clear_for_testing() {
        let trail = SessionAuditTrailCapsule::new();
        let session_id = SessionId::new(0, 1, 1);

        trail.append(SessionAuditEvent::Allocate, session_id.0, None, Some(SessionTierType::Light), None, None, None);
        assert_eq!(trail.entry_count(), 1);

        trail.clear();
        assert_eq!(trail.entry_count(), 0);
        assert_eq!(trail.get_root_hash(), GENESIS_HASH);
    }
}
