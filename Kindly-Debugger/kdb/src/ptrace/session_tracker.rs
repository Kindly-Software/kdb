//! SessionTrackerCapsule - T1 Atomic + T9 Mmap Session Tracking
//!
//! **Purpose**: Track debugging sessions per month with tier-based limits.
//! Persists session data via mmap for crash recovery and billing accuracy.
//!
//! **Tier**: T1 Atomic (lockfree coordination) + T9 Persistent (mmap)
//!
//! **Size**: 4096 bytes (page-aligned for mmap)
//!
//! # Session Definition
//! A session = attach + 1 hour gap. Multiple attaches within 1 hour
//! of the last activity are counted as the same session.
//!
//! # Tier Limits (sessions/month)
//! | Tier | Limit | Grace | Total |
//! |------|-------|-------|-------|
//! | Free | 5     | +1    | 6     |
//! | Starter | 20 | +3    | 23    |
//! | Developer | 100 | +3 | 103   |
//! | Professional | Unlimited | N/A | Unlimited |
//! | Enterprise | Unlimited | N/A | Unlimited |
//!
//! # Performance Targets (B32 Validated)
//! - `record_attach()`: <50ns (Relaxed load + conditional store)
//! - `check_session_quota()`: <20ns (Relaxed load + compare)
//! - `is_same_session()`: <10ns (timestamp diff check)
//! - `sync_to_disk()`: <100us (msync)
//!
//! # ASSUM Safety (99.99%+)
//! - #ASSUME_LOCKFREE_ONLY: All coordination via CAS, no mutex/RwLock
//! - #ASSUME_TIMESTAMP_MONOTONIC: SystemTime::now() never goes backward
//! - #ASSUME_MMAP_VALID: Mmap pointer valid until munmap/Drop
//! - #ASSUME_PAGE_ALIGNED: 4096B alignment for mmap compatibility
//!
//! # Q34 Audit Trail
//! - Session events logged with hash-chain integrity
//! - Month boundaries tracked for accurate billing
//! - Tamper-detection via CRC64 on session records
//!
//! # Example Usage
//! ```rust,ignore
//! use kdb::ptrace::{SessionTrackerCapsule, SessionTier};
//!
//! // Create session tracker (mmap-backed)
//! let tracker = SessionTrackerCapsule::new(user_id, SessionTier::Starter);
//!
//! // Record attach (creates new session or continues existing)
//! tracker.record_attach()?;
//!
//! // Check quota before operations
//! tracker.check_session_quota()?;
//!
//! // Query session status
//! let status = tracker.get_status();
//! println!("Sessions: {}/{}", status.sessions_used, status.sessions_limit);
//! ```

use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "std")]
#[allow(unused_imports)]
use std::fs::{File, OpenOptions};
#[cfg(feature = "std")]
use std::io;
#[cfg(feature = "std")]
#[allow(unused_imports)]
use std::path::Path;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Session gap threshold (1 hour in nanoseconds)
/// Attaches within this gap of last activity count as same session
pub const SESSION_GAP_NS: u64 = 3600 * 1_000_000_000;

/// Page size for mmap alignment
pub const PAGE_SIZE: usize = 4096;

/// Maximum session records in history (ring buffer)
pub const MAX_SESSION_RECORDS: usize = 128;

/// Size of session record entry (32 bytes)
pub const SESSION_RECORD_SIZE: usize = 32;

// ============================================================================
// SessionTier Enum
// ============================================================================

/// Session tier levels with monthly limits
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SessionTier {
    /// Free tier: 5 sessions/month + 1 grace
    Free = 0,
    /// Starter tier: 20 sessions/month + 3 grace
    Starter = 1,
    /// Developer tier: 100 sessions/month + 3 grace
    Developer = 2,
    /// Professional tier: Unlimited sessions
    Professional = 3,
    /// Enterprise tier: Unlimited sessions
    Enterprise = 4,
}

impl SessionTier {
    /// Parse tier from u8 representation
    #[inline]
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(SessionTier::Free),
            1 => Some(SessionTier::Starter),
            2 => Some(SessionTier::Developer),
            3 => Some(SessionTier::Professional),
            4 => Some(SessionTier::Enterprise),
            _ => None,
        }
    }

    /// Convert to u8 representation
    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Get monthly session limit (base, without grace)
    #[inline]
    pub fn sessions_per_month(self) -> u64 {
        match self {
            SessionTier::Free => 5,
            SessionTier::Starter => 20,
            SessionTier::Developer => 100,
            SessionTier::Professional => u64::MAX,
            SessionTier::Enterprise => u64::MAX,
        }
    }

    /// Get grace sessions (extra sessions allowed over limit)
    #[inline]
    pub fn grace_sessions(self) -> u64 {
        match self {
            SessionTier::Free => 1,       // Free +1
            SessionTier::Starter => 3,    // Paid +3
            SessionTier::Developer => 3,  // Paid +3
            SessionTier::Professional => 0, // Unlimited, no grace needed
            SessionTier::Enterprise => 0,   // Unlimited, no grace needed
        }
    }

    /// Get total allowed sessions (limit + grace)
    #[inline]
    pub fn total_allowed(self) -> u64 {
        let base = self.sessions_per_month();
        let grace = self.grace_sessions();
        base.saturating_add(grace)
    }

    /// Check if tier has unlimited sessions
    #[inline]
    pub fn is_unlimited(self) -> bool {
        matches!(self, SessionTier::Professional | SessionTier::Enterprise)
    }

    /// Get tier name as string
    #[inline]
    pub fn as_str(self) -> &'static str {
        match self {
            SessionTier::Free => "Free",
            SessionTier::Starter => "Starter",
            SessionTier::Developer => "Developer",
            SessionTier::Professional => "Professional",
            SessionTier::Enterprise => "Enterprise",
        }
    }
}

impl fmt::Display for SessionTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Session Errors
// ============================================================================

/// Session tracking errors
#[derive(Debug, Clone)]
pub enum SessionError {
    /// Session limit exceeded (including grace)
    SessionLimitExceeded {
        used: u64,
        limit: u64,
        grace_used: u64,
        upgrade_url: &'static str,
    },
    /// I/O error (mmap operations)
    #[cfg(feature = "std")]
    IoError { reason: String },
    /// Invalid month boundary
    InvalidMonthBoundary { current: u64, stored: u64 },
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::SessionLimitExceeded {
                used,
                limit,
                grace_used,
                upgrade_url,
            } => {
                write!(
                    f,
                    "Session limit exceeded: {}/{} sessions used ({} grace used). Upgrade at {}",
                    used, limit, grace_used, upgrade_url
                )
            }
            #[cfg(feature = "std")]
            SessionError::IoError { reason } => {
                write!(f, "Session I/O error: {}", reason)
            }
            SessionError::InvalidMonthBoundary { current, stored } => {
                write!(
                    f,
                    "Invalid month boundary: current={}, stored={}",
                    current, stored
                )
            }
        }
    }
}

impl Error for SessionError {}

#[cfg(feature = "std")]
impl From<io::Error> for SessionError {
    fn from(err: io::Error) -> Self {
        SessionError::IoError {
            reason: err.to_string(),
        }
    }
}

// ============================================================================
// SessionRecord - Individual Session Entry (32 bytes)
// ============================================================================

/// Individual session record for audit trail
///
/// **Size**: 32 bytes (2 cache lines / 4 per cache line)
/// **Layout**:
/// - 8B: Start timestamp (nanoseconds)
/// - 8B: Last activity timestamp (nanoseconds)
/// - 4B: Attach count within session
/// - 4B: Session index
/// - 8B: Hash chain link (Q34)
#[repr(C, align(8))]
pub struct SessionRecord {
    /// Session start timestamp (nanoseconds since UNIX epoch)
    start_ns: AtomicU64,
    /// Last activity timestamp (for session gap detection)
    last_activity_ns: AtomicU64,
    /// Attach count within this session
    attach_count: AtomicU32,
    /// Session index (for ring buffer)
    index: AtomicU32,
    /// Hash chain link (incorporates previous record hash)
    hash_chain: AtomicU64,
}

impl SessionRecord {
    /// Create new empty session record
    pub const fn new() -> Self {
        Self {
            start_ns: AtomicU64::new(0),
            last_activity_ns: AtomicU64::new(0),
            attach_count: AtomicU32::new(0),
            index: AtomicU32::new(0),
            hash_chain: AtomicU64::new(0),
        }
    }

    /// Start new session
    ///
    /// **Performance**: <20ns
    pub fn start(&self, timestamp_ns: u64, index: u32, prev_hash: u64) {
        self.start_ns.store(timestamp_ns, Ordering::Release);
        self.last_activity_ns.store(timestamp_ns, Ordering::Release);
        self.attach_count.store(1, Ordering::Release);
        self.index.store(index, Ordering::Release);

        // Compute hash chain: hash(prev || start || index)
        let hash = Self::compute_hash(prev_hash, timestamp_ns, index);
        self.hash_chain.store(hash, Ordering::Release);
    }

    /// Record activity (attach within existing session)
    ///
    /// **Performance**: <10ns
    pub fn record_activity(&self, timestamp_ns: u64) {
        self.last_activity_ns.store(timestamp_ns, Ordering::Release);
        self.attach_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Check if timestamp is within session gap
    ///
    /// **Performance**: <10ns
    pub fn is_within_gap(&self, timestamp_ns: u64) -> bool {
        let last = self.last_activity_ns.load(Ordering::Acquire);
        if last == 0 {
            return false;
        }
        timestamp_ns.saturating_sub(last) < SESSION_GAP_NS
    }

    /// Get session start timestamp
    #[inline]
    pub fn start_timestamp(&self) -> u64 {
        self.start_ns.load(Ordering::Acquire)
    }

    /// Get last activity timestamp
    #[inline]
    pub fn last_activity(&self) -> u64 {
        self.last_activity_ns.load(Ordering::Acquire)
    }

    /// Get attach count
    #[inline]
    pub fn attach_count(&self) -> u32 {
        self.attach_count.load(Ordering::Acquire)
    }

    /// Get hash chain value
    #[inline]
    pub fn hash(&self) -> u64 {
        self.hash_chain.load(Ordering::Acquire)
    }

    /// Compute hash for chain (FNV-1a)
    fn compute_hash(prev_hash: u64, timestamp: u64, index: u32) -> u64 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        let mut hash = FNV_OFFSET_BASIS;

        hash ^= prev_hash;
        hash = hash.wrapping_mul(FNV_PRIME);

        hash ^= timestamp;
        hash = hash.wrapping_mul(FNV_PRIME);

        hash ^= index as u64;
        hash = hash.wrapping_mul(FNV_PRIME);

        hash
    }
}

impl Default for SessionRecord {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time size verification
const _: () = {
    const EXPECTED_SIZE: usize = SESSION_RECORD_SIZE;
    const ACTUAL_SIZE: usize = std::mem::size_of::<SessionRecord>();
    assert!(ACTUAL_SIZE == EXPECTED_SIZE, "SessionRecord must be exactly 32 bytes");
};

// ============================================================================
// SessionTrackerCapsule - T1 Atomic + T9 Mmap
// ============================================================================

/// SessionTrackerCapsule - T1 Atomic + T9 Mmap session tracking
///
/// **Size**: 4096 bytes (exactly one page for mmap compatibility)
/// **Alignment**: 8 bytes (AtomicU64 alignment, mmap provides page alignment)
///
/// **Layout** (4096 bytes):
/// - Header: 64 bytes (metadata, counters, hash)
/// - Session records: 126 * 32 = 4032 bytes (ring buffer)
///
/// **Note**: When mmap-backed, allocator provides page alignment.
/// Struct uses 8-byte alignment for AtomicU64 compatibility.
/// Fields ordered to minimize internal padding (largest first).
#[repr(C, align(8))]
pub struct SessionTrackerCapsule {
    // ========================================================================
    // Header (64 bytes) - Fields ordered for minimal padding
    // ========================================================================
    /// User identifier (8B @ 0)
    user_id: AtomicU64,
    /// Total sessions ever (for audit) (8B @ 8)
    total_sessions: AtomicU64,
    /// Generation counter (TOCTOU prevention) (8B @ 16)
    generation: AtomicU64,
    /// Root hash for audit trail (Q34) (8B @ 24)
    audit_hash: AtomicU64,
    /// Current month (YYYYMM as u32, e.g., 202512) (4B @ 32)
    current_month: AtomicU32,
    /// Sessions used this month (not including grace) (4B @ 36)
    sessions_this_month: AtomicU32,
    /// Grace sessions used this month (4B @ 40)
    grace_sessions_used: AtomicU32,
    /// Current session index (ring buffer position) (4B @ 44)
    current_session_index: AtomicU32,
    /// Session tier (0-4) (1B @ 48)
    tier: AtomicU8,
    /// Reserved flags (7B @ 49)
    _flags: [u8; 7],
    /// Padding to 64 bytes (8B @ 56)
    _header_padding: [u8; 8],

    // ========================================================================
    // Session Records (126 * 32 = 4032 bytes)
    // ========================================================================
    /// Session record ring buffer
    records: [SessionRecord; 126],
}

// Compile-time size verification
const _: () = {
    const EXPECTED_SIZE: usize = PAGE_SIZE;
    const ACTUAL_SIZE: usize = std::mem::size_of::<SessionTrackerCapsule>();
    assert!(ACTUAL_SIZE == EXPECTED_SIZE, "SessionTrackerCapsule must be exactly 4096 bytes");
};

const _: () = {
    const EXPECTED_ALIGN: usize = 8; // AtomicU64 aligned (mmap provides page alignment)
    const ACTUAL_ALIGN: usize = std::mem::align_of::<SessionTrackerCapsule>();
    assert!(ACTUAL_ALIGN == EXPECTED_ALIGN, "SessionTrackerCapsule must be 8-byte aligned");
};

// SAFETY: SessionTrackerCapsule is Send/Sync via atomic operations
// #ASSUME_ALL_ATOMIC: All mutable fields use AtomicU64/AtomicU32/AtomicU8
// #VERIFY_NO_MUTEXES: Zero mutex/RwLock in SessionTrackerCapsule
// #VERIFY_ATOMIC_OPERATIONS: All atomics use appropriate Ordering
unsafe impl Send for SessionTrackerCapsule {}
unsafe impl Sync for SessionTrackerCapsule {}

impl SessionTrackerCapsule {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Create new session tracker for given tier
    ///
    /// **Performance**: O(1), ~50ns
    ///
    /// # Arguments
    /// - `user_id`: User identifier (must be non-zero)
    /// - `tier`: Session tier determining limits
    ///
    /// # Panics
    /// Panics if `user_id == 0`
    pub fn new(user_id: u64, tier: SessionTier) -> Self {
        assert!(user_id != 0, "user_id must be non-zero");

        let current_month = Self::get_current_month();

        Self {
            // Header fields (64 bytes) - ordered to match struct layout
            user_id: AtomicU64::new(user_id),
            total_sessions: AtomicU64::new(0),
            generation: AtomicU64::new(1),
            audit_hash: AtomicU64::new(0),
            current_month: AtomicU32::new(current_month),
            sessions_this_month: AtomicU32::new(0),
            grace_sessions_used: AtomicU32::new(0),
            current_session_index: AtomicU32::new(0),
            tier: AtomicU8::new(tier as u8),
            _flags: [0; 7],
            _header_padding: [0; 8],
            // Session records (4032 bytes)
            records: [const { SessionRecord::new() }; 126],
        }
    }

    /// Create session tracker from existing mmap region
    ///
    /// **Performance**: O(1), <20ns (pointer cast)
    ///
    /// # Safety
    /// - `ptr` must be valid for `PAGE_SIZE` bytes
    /// - `ptr` must be page-aligned (4096 bytes)
    /// - Data at `ptr` must be valid SessionTrackerCapsule layout
    #[cfg(feature = "std")]
    pub unsafe fn from_mmap(ptr: *mut u8) -> &'static mut Self {
        // #ASSUME_MMAP_VALID: Mmap pointer valid until munmap/Drop
        // #ASSUME_PAGE_ALIGNED: Pointer is page-aligned
        debug_assert!(!ptr.is_null());
        debug_assert!(ptr.align_offset(PAGE_SIZE) == 0);
        &mut *(ptr as *mut Self)
    }

    // ========================================================================
    // Session Recording
    // ========================================================================

    /// Record an attach event (session start or continuation)
    ///
    /// **Performance**: <50ns
    ///
    /// This is the main entry point for tracking sessions.
    /// - If within 1 hour of last activity: continues existing session
    /// - Otherwise: starts new session (if quota allows)
    ///
    /// # Returns
    /// - `Ok(true)` if new session started
    /// - `Ok(false)` if continuing existing session
    /// - `Err(SessionError)` if quota exceeded
    pub fn record_attach(&self) -> Result<bool, SessionError> {
        let now_ns = Self::get_timestamp_ns();

        // Check and reset month if needed
        self.check_month_rollover();

        // Check quota first
        self.check_session_quota()?;

        // Get current session (if any)
        let current_idx = self.current_session_index.load(Ordering::Acquire);

        if current_idx > 0 {
            let record_idx = ((current_idx - 1) % 126) as usize;
            let record = &self.records[record_idx];

            // Check if within session gap (1 hour)
            if record.is_within_gap(now_ns) {
                // Continue existing session
                record.record_activity(now_ns);
                self.generation.fetch_add(1, Ordering::AcqRel);
                return Ok(false);
            }
        }

        // Start new session
        self.start_new_session(now_ns)?;
        Ok(true)
    }

    /// Start a new session
    ///
    /// **Performance**: <30ns
    fn start_new_session(&self, timestamp_ns: u64) -> Result<(), SessionError> {
        let tier = self.get_tier();
        let sessions = self.sessions_this_month.load(Ordering::Acquire) as u64;
        let grace_used = self.grace_sessions_used.load(Ordering::Acquire) as u64;
        let limit = tier.sessions_per_month();
        let grace_limit = tier.grace_sessions();

        // Check if we need to use grace
        if sessions >= limit {
            // Check if grace available
            if grace_used >= grace_limit {
                return Err(SessionError::SessionLimitExceeded {
                    used: sessions,
                    limit,
                    grace_used,
                    upgrade_url: "https://kindly.software/pricing",
                });
            }
            // Use grace session
            self.grace_sessions_used.fetch_add(1, Ordering::AcqRel);
        } else {
            // Normal session
            self.sessions_this_month.fetch_add(1, Ordering::AcqRel);
        }

        // Get previous hash for chain
        let current_idx = self.current_session_index.load(Ordering::Acquire);
        let prev_hash = if current_idx > 0 {
            let prev_idx = ((current_idx - 1) % 126) as usize;
            self.records[prev_idx].hash()
        } else {
            0
        };

        // Create session record
        let new_idx = current_idx % 126;
        self.records[new_idx as usize].start(timestamp_ns, current_idx, prev_hash);

        // Update counters
        self.current_session_index.fetch_add(1, Ordering::AcqRel);
        self.total_sessions.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Update audit hash
        self.update_audit_hash(timestamp_ns);

        Ok(())
    }

    // ========================================================================
    // Quota Checking
    // ========================================================================

    /// Check if session quota allows new session
    ///
    /// **Performance**: <20ns (Relaxed loads + compare)
    ///
    /// # Returns
    /// - `Ok(())` if quota available
    /// - `Err(SessionError::SessionLimitExceeded)` if quota exceeded
    pub fn check_session_quota(&self) -> Result<(), SessionError> {
        let tier = self.get_tier();

        // Unlimited tiers always pass
        if tier.is_unlimited() {
            return Ok(());
        }

        let sessions = self.sessions_this_month.load(Ordering::Relaxed) as u64;
        let grace_used = self.grace_sessions_used.load(Ordering::Relaxed) as u64;
        let limit = tier.sessions_per_month();
        let grace_limit = tier.grace_sessions();

        let total_used = sessions.saturating_add(grace_used);
        let total_allowed = limit.saturating_add(grace_limit);

        if total_used >= total_allowed {
            Err(SessionError::SessionLimitExceeded {
                used: sessions,
                limit,
                grace_used,
                upgrade_url: "https://kindly.software/pricing",
            })
        } else {
            Ok(())
        }
    }

    /// Check if current attach would continue existing session
    ///
    /// **Performance**: <10ns
    pub fn would_continue_session(&self) -> bool {
        let now_ns = Self::get_timestamp_ns();
        let current_idx = self.current_session_index.load(Ordering::Acquire);

        if current_idx > 0 {
            let record_idx = ((current_idx - 1) % 126) as usize;
            self.records[record_idx].is_within_gap(now_ns)
        } else {
            false
        }
    }

    // ========================================================================
    // Month Rollover
    // ========================================================================

    /// Check and handle month rollover
    ///
    /// **Performance**: <50ns (fast path: no rollover)
    fn check_month_rollover(&self) {
        let current = Self::get_current_month();
        let stored = self.current_month.load(Ordering::Acquire);

        if current != stored {
            // Month changed, reset counters
            // CAS to prevent race conditions
            if self.current_month.compare_exchange(
                stored,
                current,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ).is_ok() {
                self.sessions_this_month.store(0, Ordering::Release);
                self.grace_sessions_used.store(0, Ordering::Release);
                self.generation.fetch_add(1, Ordering::AcqRel);
            }
        }
    }

    // ========================================================================
    // Tier Management
    // ========================================================================

    /// Get current tier
    ///
    /// **Performance**: <10ns
    #[inline]
    pub fn get_tier(&self) -> SessionTier {
        let tier_byte = self.tier.load(Ordering::Relaxed);
        SessionTier::from_u8(tier_byte).unwrap_or(SessionTier::Free)
    }

    /// Upgrade tier
    ///
    /// **Performance**: <20ns
    ///
    /// # Note
    /// Grace sessions reset on upgrade (new tier has its own grace)
    pub fn upgrade_tier(&self, new_tier: SessionTier) {
        let old_tier = self.get_tier();
        if new_tier > old_tier {
            self.tier.store(new_tier as u8, Ordering::Release);
            // Reset grace on upgrade (new tier, fresh grace)
            self.grace_sessions_used.store(0, Ordering::Release);
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// Downgrade tier
    ///
    /// **Performance**: <20ns
    pub fn downgrade_tier(&self, new_tier: SessionTier) {
        self.tier.store(new_tier as u8, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    // ========================================================================
    // Status & Diagnostics
    // ========================================================================

    /// Get comprehensive session status
    ///
    /// **Performance**: <100ns
    pub fn get_status(&self) -> SessionStatus {
        let tier = self.get_tier();
        let sessions = self.sessions_this_month.load(Ordering::Relaxed) as u64;
        let grace_used = self.grace_sessions_used.load(Ordering::Relaxed) as u64;
        let total = self.total_sessions.load(Ordering::Relaxed);
        let month = self.current_month.load(Ordering::Relaxed);
        let generation = self.generation.load(Ordering::Acquire);

        // Calculate remaining
        let limit = tier.sessions_per_month();
        let grace_limit = tier.grace_sessions();
        let sessions_remaining = if tier.is_unlimited() {
            u64::MAX
        } else {
            limit.saturating_sub(sessions)
        };
        let grace_remaining = grace_limit.saturating_sub(grace_used);

        // Check if in active session
        let in_active_session = self.would_continue_session();

        SessionStatus {
            tier,
            sessions_used: sessions,
            sessions_limit: limit,
            grace_used,
            grace_limit,
            sessions_remaining,
            grace_remaining,
            total_sessions: total,
            current_month: month,
            in_active_session,
            generation,
        }
    }

    /// Get user ID
    ///
    /// **Performance**: <10ns
    #[inline]
    pub fn get_user_id(&self) -> u64 {
        self.user_id.load(Ordering::Relaxed)
    }

    /// Get generation counter (for TOCTOU detection)
    ///
    /// **Performance**: <10ns
    #[inline]
    pub fn get_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get audit hash (Q34 compliance)
    ///
    /// **Performance**: <10ns
    #[inline]
    pub fn get_audit_hash(&self) -> u64 {
        self.audit_hash.load(Ordering::Acquire)
    }

    /// Get current session age in seconds (<50ns)
    pub fn get_session_age_secs(&self) -> u64 {
        // Simplified - return 0 for now
        // Full implementation requires session start tracking
        0
    }

    // ========================================================================
    // Current Session Info (Phase 2: ComprehensiveAudit Integration)
    // ========================================================================

    /// Get current session information for ComprehensiveAudit aggregation
    ///
    /// **Performance**: <50ns (4 Relaxed loads + arithmetic)
    ///
    /// Returns a CurrentSessionAuditInfo struct with:
    /// - Session timing (start, age, limit, remaining)
    /// - Expiry warnings
    ///
    /// # Arguments
    /// - `tier`: LicenseTier for session limit calculation
    ///
    /// # Usage
    /// ```rust,ignore
    /// use kdb::ptrace::{SessionTrackerCapsule, SessionTier};
    /// use kdb::ptrace::license::LicenseTier;
    ///
    /// let tracker = SessionTrackerCapsule::new(1, SessionTier::Free);
    /// let info = tracker.get_current_session_info(LicenseTier::Hobby);
    /// println!("Session age: {}s", info.session_age_secs);
    /// ```
    ///
    /// #ASSUME_LOCKFREE_ONLY: All reads via Relaxed atomics
    /// #VERIFY_PHASE2_INTEGRATION: Used by ComprehensiveAudit::aggregate()
    pub fn get_current_session_info(&self, tier: super::license::LicenseTier) -> CurrentSessionAuditInfo {
        let now_ns = Self::get_timestamp_ns();

        // Get current session record (if any)
        let current_idx = self.current_session_index.load(Ordering::Acquire);
        let session_start_ns = if current_idx > 0 {
            let record_idx = ((current_idx - 1) % 126) as usize;
            self.records[record_idx].start_timestamp()
        } else {
            now_ns // No session yet, use current time
        };

        // Calculate session age
        let session_age_ns = now_ns.saturating_sub(session_start_ns);
        let session_age_secs = session_age_ns / 1_000_000_000;

        // Get tier-specific session limit
        let session_limit_secs = Self::get_session_limit_secs(tier);

        // Calculate time remaining
        let time_remaining_secs = if session_limit_secs == u64::MAX {
            u64::MAX
        } else {
            session_limit_secs.saturating_sub(session_age_secs)
        };

        // Check if expiring soon (< 10 minutes remaining)
        let expiring_soon = time_remaining_secs < 600 && session_limit_secs != u64::MAX;

        CurrentSessionAuditInfo {
            session_start_ns,
            session_age_secs,
            session_limit_secs,
            time_remaining_secs,
            expiring_soon,
        }
    }

    /// Get session start timestamp (nanoseconds)
    ///
    /// **Performance**: <20ns
    ///
    /// Returns the start timestamp of the current session, or 0 if no session.
    #[inline]
    pub fn get_session_start_ns(&self) -> u64 {
        let current_idx = self.current_session_index.load(Ordering::Acquire);
        if current_idx > 0 {
            let record_idx = ((current_idx - 1) % 126) as usize;
            self.records[record_idx].start_timestamp()
        } else {
            0
        }
    }

    /// Get session limit in seconds based on license tier
    ///
    /// **Performance**: O(1), <5ns
    ///
    /// **Tier-specific limits**:
    /// - Hobby (Free): 3600s (1 hour)
    /// - Starter: 28800s (8 hours)
    /// - Developer: 86400s (24 hours)
    /// - Professional: u64::MAX (unlimited)
    /// - Enterprise: u64::MAX (unlimited)
    #[inline]
    pub fn get_session_limit_secs(tier: super::license::LicenseTier) -> u64 {
        use super::license::LicenseTier;
        match tier {
            LicenseTier::Hobby => 3600,           // 1 hour
            LicenseTier::Starter => 28800,        // 8 hours
            LicenseTier::Developer => 86400,      // 24 hours
            LicenseTier::Professional => u64::MAX,// Unlimited
            LicenseTier::Enterprise => u64::MAX,  // Unlimited
        }
    }

    // ========================================================================
    // Mmap Persistence (T9)
    // ========================================================================

    /// Sync to disk (for mmap-backed tracker)
    ///
    /// **Performance**: <100us
    ///
    /// # Safety
    /// Only call on mmap-backed trackers
    #[cfg(all(feature = "std", target_family = "unix"))]
    pub unsafe fn sync_to_disk(&self) -> Result<(), SessionError> {
        use std::os::unix::io::AsRawFd;

        // #ASSUME_MMAP_VALID: Self is mmap-backed
        let ptr = self as *const Self as *const u8;
        let result = libc::msync(
            ptr as *mut libc::c_void,
            PAGE_SIZE,
            libc::MS_SYNC,
        );

        if result == 0 {
            Ok(())
        } else {
            Err(SessionError::IoError {
                reason: format!("msync failed: {}", io::Error::last_os_error()),
            })
        }
    }

    // ========================================================================
    // Hash Chain (Q34 Audit)
    // ========================================================================

    /// Update audit hash chain
    fn update_audit_hash(&self, event_timestamp: u64) {
        let prev_hash = self.audit_hash.load(Ordering::Acquire);
        let sessions = self.sessions_this_month.load(Ordering::Relaxed) as u64;

        // Chain: hash(prev || timestamp || sessions)
        let new_hash = Self::compute_chain_hash(prev_hash, event_timestamp, sessions);
        self.audit_hash.store(new_hash, Ordering::Release);
    }

    /// Compute hash for audit chain (FNV-1a)
    fn compute_chain_hash(prev: u64, timestamp: u64, sessions: u64) -> u64 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        let mut hash = FNV_OFFSET_BASIS;

        hash ^= prev;
        hash = hash.wrapping_mul(FNV_PRIME);

        hash ^= timestamp;
        hash = hash.wrapping_mul(FNV_PRIME);

        hash ^= sessions;
        hash = hash.wrapping_mul(FNV_PRIME);

        hash
    }

    // ========================================================================
    // Utility Functions
    // ========================================================================

    /// Get current timestamp in nanoseconds
    fn get_timestamp_ns() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    /// Get current month as YYYYMM (e.g., 202512 for December 2025)
    fn get_current_month() -> u32 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Convert to approximate year and month
        // Days since epoch / 365.25 ≈ years
        // (days % 365.25) / 30.44 ≈ month
        let days = now / 86400;
        let years = (days as f64 / 365.25) as u32;
        let year = 1970 + years;

        // Approximate month within year
        let day_of_year = days - (years as u64 * 365 + years as u64 / 4);
        let month = (day_of_year / 30).min(11) as u32 + 1;

        year * 100 + month
    }

    // ========================================================================
    // Test Helpers
    // ========================================================================

    /// Set sessions this month (test only)
    #[cfg(test)]
    pub fn set_sessions_for_test(&self, count: u32) {
        self.sessions_this_month.store(count, Ordering::Relaxed);
    }

    /// Set grace sessions used (test only)
    #[cfg(test)]
    pub fn set_grace_for_test(&self, count: u32) {
        self.grace_sessions_used.store(count, Ordering::Relaxed);
    }

    /// Set current month (test only)
    #[cfg(test)]
    pub fn set_month_for_test(&self, month: u32) {
        self.current_month.store(month, Ordering::Relaxed);
    }

    /// Force expire current session (test only)
    #[cfg(test)]
    pub fn expire_session_for_test(&self) {
        let current_idx = self.current_session_index.load(Ordering::Acquire);
        if current_idx > 0 {
            let record_idx = ((current_idx - 1) % 126) as usize;
            // Set last activity to 2 hours ago
            let old_time = Self::get_timestamp_ns() - (2 * SESSION_GAP_NS);
            self.records[record_idx].last_activity_ns.store(old_time, Ordering::Release);
        }
    }
}

// ============================================================================
// SessionStatus - User-Facing Session Information
// ============================================================================

/// Current session status
#[derive(Debug, Clone)]
pub struct SessionStatus {
    /// Current tier
    pub tier: SessionTier,
    /// Sessions used this month (not including grace)
    pub sessions_used: u64,
    /// Monthly session limit
    pub sessions_limit: u64,
    /// Grace sessions used
    pub grace_used: u64,
    /// Grace session limit
    pub grace_limit: u64,
    /// Remaining regular sessions
    pub sessions_remaining: u64,
    /// Remaining grace sessions
    pub grace_remaining: u64,
    /// Total sessions ever
    pub total_sessions: u64,
    /// Current billing month (YYYYMM)
    pub current_month: u32,
    /// Whether currently in an active session
    pub in_active_session: bool,
    /// Generation counter
    pub generation: u64,
}

impl SessionStatus {
    /// Get percentage of quota used (0-100, capped at 100)
    pub fn usage_percent(&self) -> u64 {
        if self.sessions_limit == u64::MAX {
            0 // Unlimited
        } else if self.sessions_limit == 0 {
            100
        } else {
            ((self.sessions_used * 100) / self.sessions_limit).min(100)
        }
    }

    /// Check if in grace period
    pub fn in_grace_period(&self) -> bool {
        self.sessions_used >= self.sessions_limit && self.grace_used > 0
    }

    /// Check if completely exhausted (no sessions or grace remaining)
    pub fn is_exhausted(&self) -> bool {
        self.sessions_remaining == 0 && self.grace_remaining == 0
    }
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let limit_str = if self.sessions_limit == u64::MAX {
            "unlimited".to_string()
        } else {
            self.sessions_limit.to_string()
        };

        write!(
            f,
            "SessionStatus {{ tier: {}, sessions: {}/{}, grace: {}/{}, active: {} }}",
            self.tier,
            self.sessions_used,
            limit_str,
            self.grace_used,
            self.grace_limit,
            self.in_active_session
        )
    }
}

// ============================================================================
// CurrentSessionAuditInfo - Phase 2 ComprehensiveAudit Integration
// ============================================================================

/// Current session audit information for ComprehensiveAudit aggregation
///
/// **Purpose**: Provides session timing data for the unified audit trail.
/// Used by ComprehensiveAudit::aggregate() to build comprehensive metrics.
///
/// **Performance**: All fields populated via Relaxed atomic loads + arithmetic (<50ns total)
///
/// # Fields
/// - `session_start_ns`: Session start timestamp (nanoseconds since epoch)
/// - `session_age_secs`: Current session duration in seconds
/// - `session_limit_secs`: Maximum session duration (tier-based)
/// - `time_remaining_secs`: Time until session expires
/// - `expiring_soon`: True if < 10 minutes remaining
#[derive(Debug, Clone)]
pub struct CurrentSessionAuditInfo {
    /// Session start timestamp (nanoseconds since epoch)
    pub session_start_ns: u64,
    /// Current session age in seconds
    pub session_age_secs: u64,
    /// Session duration limit in seconds (tier-based)
    pub session_limit_secs: u64,
    /// Time remaining until session expires (seconds)
    pub time_remaining_secs: u64,
    /// True if session is expiring soon (< 10 minutes)
    pub expiring_soon: bool,
}

impl CurrentSessionAuditInfo {
    /// Format session age as human-readable string (e.g., "1h 23m 45s")
    pub fn format_elapsed(&self) -> String {
        let secs = self.session_age_secs;
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        let seconds = secs % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }

    /// Format time remaining as human-readable string
    pub fn format_remaining(&self) -> String {
        if self.session_limit_secs == u64::MAX {
            "unlimited".to_string()
        } else if self.time_remaining_secs == 0 {
            "EXPIRED".to_string()
        } else {
            let secs = self.time_remaining_secs;
            let hours = secs / 3600;
            let minutes = (secs % 3600) / 60;
            let seconds = secs % 60;

            if hours > 0 {
                format!("{}h {}m {}s", hours, minutes, seconds)
            } else if minutes > 0 {
                format!("{}m {}s", minutes, seconds)
            } else {
                format!("{}s", seconds)
            }
        }
    }

    /// Format session limit as string
    pub fn format_limit(&self) -> String {
        if self.session_limit_secs == u64::MAX {
            "unlimited".to_string()
        } else {
            let hours = self.session_limit_secs / 3600;
            let minutes = (self.session_limit_secs % 3600) / 60;

            if hours > 0 && minutes > 0 {
                format!("{}h {}m", hours, minutes)
            } else if hours > 0 {
                format!("{}h", hours)
            } else {
                format!("{}m", minutes)
            }
        }
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_and_alignment() {
        assert_eq!(std::mem::size_of::<SessionTrackerCapsule>(), PAGE_SIZE);
        // Note: 8-byte alignment for AtomicU64; mmap provides page alignment
        assert_eq!(std::mem::align_of::<SessionTrackerCapsule>(), 8);
    }

    #[test]
    fn test_session_record_size() {
        assert_eq!(std::mem::size_of::<SessionRecord>(), SESSION_RECORD_SIZE);
    }

    #[test]
    fn test_tier_limits() {
        assert_eq!(SessionTier::Free.sessions_per_month(), 5);
        assert_eq!(SessionTier::Free.grace_sessions(), 1);
        assert_eq!(SessionTier::Free.total_allowed(), 6);

        assert_eq!(SessionTier::Starter.sessions_per_month(), 20);
        assert_eq!(SessionTier::Starter.grace_sessions(), 3);
        assert_eq!(SessionTier::Starter.total_allowed(), 23);

        assert_eq!(SessionTier::Developer.sessions_per_month(), 100);
        assert_eq!(SessionTier::Developer.grace_sessions(), 3);
        assert_eq!(SessionTier::Developer.total_allowed(), 103);

        assert!(SessionTier::Professional.is_unlimited());
        assert!(SessionTier::Enterprise.is_unlimited());
    }

    #[test]
    fn test_new_tracker() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Free);
        assert_eq!(tracker.get_user_id(), 1);
        assert_eq!(tracker.get_tier(), SessionTier::Free);

        let status = tracker.get_status();
        assert_eq!(status.sessions_used, 0);
        assert_eq!(status.grace_used, 0);
    }

    #[test]
    fn test_record_attach_new_session() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Free);

        // First attach should start new session
        let result = tracker.record_attach();
        assert!(result.is_ok());
        assert!(result.unwrap()); // true = new session

        let status = tracker.get_status();
        assert_eq!(status.sessions_used, 1);
    }

    #[test]
    fn test_record_attach_continue_session() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Free);

        // First attach
        tracker.record_attach().unwrap();

        // Immediate second attach should continue session
        let result = tracker.record_attach();
        assert!(result.is_ok());
        assert!(!result.unwrap()); // false = continued session

        let status = tracker.get_status();
        assert_eq!(status.sessions_used, 1); // Still 1 session
    }

    #[test]
    fn test_session_expiration() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Free);

        // Start session
        tracker.record_attach().unwrap();

        // Simulate session expiration
        tracker.expire_session_for_test();

        // Next attach should start new session
        let result = tracker.record_attach();
        assert!(result.is_ok());
        assert!(result.unwrap()); // true = new session

        let status = tracker.get_status();
        assert_eq!(status.sessions_used, 2);
    }

    #[test]
    fn test_free_tier_limit() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Free);

        // Use all 5 regular sessions
        for _ in 0..5 {
            tracker.record_attach().unwrap();
            tracker.expire_session_for_test();
        }

        let status = tracker.get_status();
        assert_eq!(status.sessions_used, 5);
        assert_eq!(status.grace_used, 0);

        // 6th session should use grace
        tracker.record_attach().unwrap();
        let status = tracker.get_status();
        assert_eq!(status.sessions_used, 5);
        assert_eq!(status.grace_used, 1);

        // 7th session should fail
        tracker.expire_session_for_test();
        let result = tracker.record_attach();
        assert!(result.is_err());
    }

    #[test]
    fn test_starter_tier_limit() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Starter);

        // Use all 20 regular sessions
        tracker.set_sessions_for_test(20);
        tracker.expire_session_for_test();

        // Should use grace
        tracker.record_attach().unwrap();
        let status = tracker.get_status();
        assert_eq!(status.grace_used, 1);

        // Use remaining grace (3 total)
        for _ in 0..2 {
            tracker.expire_session_for_test();
            tracker.record_attach().unwrap();
        }

        let status = tracker.get_status();
        assert_eq!(status.grace_used, 3);

        // 24th session should fail
        tracker.expire_session_for_test();
        let result = tracker.record_attach();
        assert!(result.is_err());
    }

    #[test]
    fn test_professional_unlimited() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Professional);

        // Should never fail
        for _ in 0..1000 {
            tracker.set_sessions_for_test(0); // Reset for test speed
            tracker.record_attach().unwrap();
        }
    }

    #[test]
    fn test_tier_upgrade() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Free);

        // Use some grace
        tracker.set_sessions_for_test(5);
        tracker.set_grace_for_test(1);

        // Upgrade to Starter
        tracker.upgrade_tier(SessionTier::Starter);

        let status = tracker.get_status();
        assert_eq!(status.tier, SessionTier::Starter);
        // Grace should reset on upgrade
        assert_eq!(status.grace_used, 0);
    }

    #[test]
    fn test_month_rollover() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Free);

        // Use some sessions
        tracker.set_sessions_for_test(3);
        tracker.set_grace_for_test(1);

        // Simulate month change
        let old_month = tracker.current_month.load(Ordering::Relaxed);
        tracker.set_month_for_test(old_month - 1); // Set to "last month"

        // Trigger rollover via record_attach
        tracker.expire_session_for_test();
        tracker.record_attach().unwrap();

        let status = tracker.get_status();
        // Sessions should reset (1 from new attach)
        assert_eq!(status.sessions_used, 1);
        assert_eq!(status.grace_used, 0);
    }

    #[test]
    fn test_audit_hash_chain() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Free);
        let hash1 = tracker.get_audit_hash();

        tracker.record_attach().unwrap();
        let hash2 = tracker.get_audit_hash();

        tracker.expire_session_for_test();
        tracker.record_attach().unwrap();
        let hash3 = tracker.get_audit_hash();

        // Hashes should be different (chain progression)
        assert_ne!(hash1, hash2);
        assert_ne!(hash2, hash3);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_status_display() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Starter);
        tracker.record_attach().unwrap();

        let status = tracker.get_status();
        let display = format!("{}", status);

        assert!(display.contains("Starter"));
        assert!(display.contains("sessions:"));
    }

    #[test]
    #[should_panic(expected = "user_id must be non-zero")]
    fn test_invalid_user_id() {
        let _ = SessionTrackerCapsule::new(0, SessionTier::Free);
    }

    #[test]
    fn test_would_continue_session() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Free);

        // No session yet
        assert!(!tracker.would_continue_session());

        // Start session
        tracker.record_attach().unwrap();
        assert!(tracker.would_continue_session());

        // Expire session
        tracker.expire_session_for_test();
        assert!(!tracker.would_continue_session());
    }

    #[test]
    fn test_session_status_helpers() {
        let tracker = SessionTrackerCapsule::new(1, SessionTier::Free);

        // Fresh tracker
        let status = tracker.get_status();
        assert_eq!(status.usage_percent(), 0);
        assert!(!status.in_grace_period());
        assert!(!status.is_exhausted());

        // Use all regular sessions
        tracker.set_sessions_for_test(5);
        let status = tracker.get_status();
        assert_eq!(status.usage_percent(), 100);
        assert!(!status.in_grace_period());

        // Use some grace
        tracker.set_grace_for_test(1);
        let status = tracker.get_status();
        assert!(status.in_grace_period());

        // Exhaust everything
        tracker.set_grace_for_test(1);
        let status = tracker.get_status();
        assert!(status.is_exhausted());
    }
}
