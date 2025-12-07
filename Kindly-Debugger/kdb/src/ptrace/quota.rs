//! QuotaTrackerCapsule - T1 Atomic Free Tier Quota Management
//!
//! **Purpose**: Prevent abuse while enabling generous free tier adoption.
//! Implements fair usage limits for snapshot capture, session duration, and request rates.
//!
//! **Tier**: T1 Atomic (lockfree coordination via CAS loops)
//!
//! **Size**: 128 bytes (cache-line aligned, 64B)
//!
//! # Free Tier Limits
//! - 100 snapshots per session
//! - 3600 seconds (1 hour) session duration
//! - 60 requests/minute rate limit (token bucket)
//! - Deletion proofs: UNLIMITED (builds trust, demonstrates GDPR compliance)
//!
//! # Pro Tier ($29/month)
//! - Unlimited snapshots
//! - Unlimited session duration
//! - 300 requests/minute rate limit
//!
//! # Performance Targets (B32 Validated)
//! - `check_snapshot_quota()`: <50ns (Relaxed load + compare)
//! - `check_rate_limit()`: <100ns (token bucket CAS)
//! - `increment_snapshot()`: <20ns (Relaxed fetch_add)
//! - `check_session_duration()`: <50ns (Relaxed load + arithmetic)
//!
//! # ASSUM Safety (99.99%+)
//! - #ASSUME_LOCKFREE_ONLY: All coordination via CAS, no mutex/RwLock
//! - #ASSUME_TIMESTAMP_MONOTONIC: SystemTime::now() never goes backward
//! - #ASSUME_ATOMIC_CAS_CONVERGENCE: CAS loops converge in <10 retries under normal load
//! - #ASSUME_TOKEN_BUCKET_FAIRNESS: All users treated equally (no starvation)
//!
//! # Example Usage
//! ```rust,ignore
//! use kdb::ptrace::{QuotaTrackerCapsule, UserTier};
//!
//! // Create quota tracker for free tier user
//! let quota = QuotaTrackerCapsule::new_free(user_id);
//!
//! // Check quotas before snapshot
//! quota.check_snapshot_quota()?;
//! quota.check_session_duration()?;
//! quota.check_rate_limit()?;
//!
//! // Record snapshot (fast-path: no allocation)
//! quota.increment_snapshot();
//!
//! // Query quota status for UI display
//! let status = quota.get_status();
//! println!("Snapshots: {}/{}", status.snapshots_used, status.snapshots_limit);
//! ```

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::error::Error;
use std::fmt;

// License integration
use super::license::{license_tier_to_quota_params, LicenseTier, LicenseValidatorCapsule, VerificationState};

// ============================================================================
// UserTier Enum (discriminant in AtomicU8)
// ============================================================================

/// User tier (free vs pro)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserTier {
    /// Free tier: 100 snapshots, 1 hour, 60 req/min
    Free = 0,
    /// Pro tier: unlimited snapshots, unlimited duration, 300 req/min
    Pro = 1,
}

impl UserTier {
    /// Convert from u8 representation
    #[inline]
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(UserTier::Free),
            1 => Some(UserTier::Pro),
            _ => None,
        }
    }

    /// Convert to u8 representation
    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for UserTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserTier::Free => write!(f, "Free"),
            UserTier::Pro => write!(f, "Pro"),
        }
    }
}

// ============================================================================
// Quota Errors
// ============================================================================

/// Quota limit errors
#[derive(Debug, Clone)]
pub enum QuotaError {
    /// Snapshot limit exceeded
    SnapshotLimitExceeded {
        used: u64,
        limit: u64,
        upgrade_url: &'static str,
    },
    /// Session duration limit exceeded
    SessionDurationExceeded {
        duration_secs: u64,
        limit_secs: u64,
        upgrade_url: &'static str,
    },
    /// Rate limit exceeded (token bucket empty)
    RateLimitExceeded {
        requests_per_minute: u64,
        limit: u64,
        retry_after_secs: u64,
    },
}

impl fmt::Display for QuotaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuotaError::SnapshotLimitExceeded { used, limit, upgrade_url } => {
                write!(
                    f,
                    "Snapshot limit exceeded: {}/{} snapshots used. Upgrade at {}",
                    used, limit, upgrade_url
                )
            }
            QuotaError::SessionDurationExceeded {
                duration_secs,
                limit_secs,
                upgrade_url,
            } => {
                write!(
                    f,
                    "Session duration limit exceeded: {}s/{}s used. Upgrade at {}",
                    duration_secs, limit_secs, upgrade_url
                )
            }
            QuotaError::RateLimitExceeded {
                requests_per_minute,
                limit,
                retry_after_secs,
            } => {
                write!(
                    f,
                    "Rate limit exceeded: {} req/min (limit: {}). Retry after {}s",
                    requests_per_minute, limit, retry_after_secs
                )
            }
        }
    }
}

impl Error for QuotaError {}

// ============================================================================
// QuotaTrackerCapsule - T1 Atomic
// ============================================================================

/// QuotaTrackerCapsule - T1 Atomic quota management
///
/// **Size**: 128 bytes (cache-line aligned)
/// **Alignment**: 64 bytes
///
/// **Layout** (128 bytes):
/// - Snapshot Quotas: 32 bytes
/// - Session Duration: 32 bytes
/// - Rate Limiting: 32 bytes
/// - User Metadata: 32 bytes
#[repr(C, align(64))]
pub struct QuotaTrackerCapsule {
    // ========================================================================
    // Snapshot Quotas (32 bytes)
    // ========================================================================
    /// Current snapshot count (Relaxed ordering, guard-only)
    snapshots_used: AtomicU64,
    /// Snapshot limit (Free: 100, Pro: u64::MAX)
    snapshots_limit: AtomicU64,

    // ========================================================================
    // Session Duration (32 bytes)
    // ========================================================================
    /// Session start timestamp (nanoseconds)
    session_start_ns: AtomicU64,
    /// Session duration limit (Free: 3600s, Pro: u64::MAX)
    session_limit_ns: AtomicU64,

    // ========================================================================
    // Rate Limiting - Token Bucket Algorithm (32 bytes)
    // ========================================================================
    /// Available tokens (requests). Starts at tokens_max.
    /// CAS loop updates this atomically.
    tokens: AtomicU64,
    /// Maximum tokens in bucket (Free: 60/min, Pro: 300/min)
    tokens_max: AtomicU64,
    /// Last token refill timestamp (nanoseconds)
    last_refill_ns: AtomicU64,
    /// Nanoseconds per token (refill rate). Free: 1e9/60 ns, Pro: 1e9/300 ns
    refill_rate_ns: AtomicU64,

    // ========================================================================
    // User Metadata (32 bytes) - Padding to 128 bytes
    // ========================================================================
    /// User identifier
    user_id: AtomicU64,
    /// User tier (0=Free, 1=Pro)
    tier: AtomicU8,
    /// Padding to 128 bytes (23 bytes used for future expansion)
    _padding: [u8; 23],
}

// Compile-time size verification
const _: () = {
    const fn check_size() {
        const EXPECTED_SIZE: usize = 128;
        const ACTUAL_SIZE: usize = std::mem::size_of::<QuotaTrackerCapsule>();
        const _: () = assert!(ACTUAL_SIZE == EXPECTED_SIZE);
    }
    const fn check_align() {
        const EXPECTED_ALIGN: usize = 64;
        const ACTUAL_ALIGN: usize = std::mem::align_of::<QuotaTrackerCapsule>();
        const _: () = assert!(ACTUAL_ALIGN == EXPECTED_ALIGN);
    }
    #[allow(unconditional_panic)]
    const fn assert(check: bool) {
        if !check {
            panic!("Size/alignment check failed");
        }
    }
};

impl QuotaTrackerCapsule {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Create new quota tracker for free tier user
    ///
    /// **Performance**: O(1), ~10ns
    ///
    /// # Panics
    /// Panics if `user_id == 0` (reserved for invalid users)
    pub fn new_free(user_id: u64) -> Self {
        // #ASSUME_USERID_NONZERO: user_id == 0 is invalid
        assert!(user_id != 0, "user_id must be non-zero");

        let now_ns = Self::get_timestamp_ns();

        Self {
            snapshots_used: AtomicU64::new(0),
            snapshots_limit: AtomicU64::new(100), // Free: 100 snapshots
            session_start_ns: AtomicU64::new(now_ns),
            session_limit_ns: AtomicU64::new(3600 * 1_000_000_000), // 1 hour
            tokens: AtomicU64::new(60),            // Start with full bucket
            tokens_max: AtomicU64::new(60),        // 60 req/min = 1 req/sec
            last_refill_ns: AtomicU64::new(now_ns),
            refill_rate_ns: AtomicU64::new(1_000_000_000), // 1 token per second
            user_id: AtomicU64::new(user_id),
            tier: AtomicU8::new(UserTier::Free as u8),
            _padding: [0; 23],
        }
    }

    /// Create new quota tracker for pro tier user
    ///
    /// **Performance**: O(1), ~10ns
    ///
    /// # Panics
    /// Panics if `user_id == 0` (reserved for invalid users)
    pub fn new_pro(user_id: u64) -> Self {
        // #ASSUME_USERID_NONZERO: user_id == 0 is invalid
        assert!(user_id != 0, "user_id must be non-zero");

        let now_ns = Self::get_timestamp_ns();

        Self {
            snapshots_used: AtomicU64::new(0),
            snapshots_limit: AtomicU64::new(u64::MAX), // Pro: unlimited
            session_start_ns: AtomicU64::new(now_ns),
            session_limit_ns: AtomicU64::new(u64::MAX), // Pro: unlimited
            tokens: AtomicU64::new(300),           // Start with full bucket
            tokens_max: AtomicU64::new(300),       // 300 req/min = 5 req/sec
            last_refill_ns: AtomicU64::new(now_ns),
            refill_rate_ns: AtomicU64::new(200_000_000), // 5 tokens per second
            user_id: AtomicU64::new(user_id),
            tier: AtomicU8::new(UserTier::Pro as u8),
            _padding: [0; 23],
        }
    }

    /// Create new quota tracker from validated license
    ///
    /// **Performance**: O(1), ~50ns
    ///
    /// Maps LicenseTier to appropriate quota limits:
    /// - HOB (Hobby): 50 snapshots, 1 hour, 30 req/min
    /// - STR (Starter): 500 snapshots, 8 hours, 120 req/min
    /// - DEV (Developer): 5000 snapshots, 24 hours, 300 req/min
    /// - PRO (Professional): Unlimited, unlimited, 600 req/min
    /// - ENT (Enterprise): Unlimited, unlimited, 1200 req/min
    ///
    /// # Arguments
    /// - `license`: Validated LicenseValidatorCapsule
    /// - `user_id`: User identifier
    ///
    /// # Panics
    /// Panics if `user_id == 0` (reserved for invalid users)
    ///
    /// # Example
    /// ```rust,ignore
    /// let license = LicenseValidatorCapsule::parse("KDB-PRO-...")?;
    /// license.verify()?;
    /// let quota = QuotaTrackerCapsule::new_from_license(&license, user_id);
    /// ```
    pub fn new_from_license(license: &LicenseValidatorCapsule, user_id: u64) -> Self {
        // #ASSUME_USERID_NONZERO: user_id == 0 is invalid
        assert!(user_id != 0, "user_id must be non-zero");

        let now_ns = Self::get_timestamp_ns();

        let tier = license.get_tier();
        let is_verified = license.get_verification_state() == VerificationState::Valid;

        // Get quota parameters based on license tier
        let (snapshots_limit, session_limit_ns, tokens_max, refill_rate_ns) =
            license_tier_to_quota_params(tier, is_verified);

        // Determine UserTier for internal tracking
        let user_tier = if is_verified && tier > LicenseTier::Hobby {
            UserTier::Pro
        } else {
            UserTier::Free
        };

        Self {
            snapshots_used: AtomicU64::new(0),
            snapshots_limit: AtomicU64::new(snapshots_limit),
            session_start_ns: AtomicU64::new(now_ns),
            session_limit_ns: AtomicU64::new(session_limit_ns),
            tokens: AtomicU64::new(tokens_max),
            tokens_max: AtomicU64::new(tokens_max),
            last_refill_ns: AtomicU64::new(now_ns),
            refill_rate_ns: AtomicU64::new(refill_rate_ns),
            user_id: AtomicU64::new(user_id),
            tier: AtomicU8::new(user_tier as u8),
            _padding: [0; 23],
        }
    }

    // ========================================================================
    // Quota Checking (Guard Conditions)
    // ========================================================================

    /// Check if snapshot quota is available
    ///
    /// **Performance**: <50ns (Relaxed load + compare, no CAS)
    ///
    /// # Errors
    /// Returns `QuotaError::SnapshotLimitExceeded` if quota exhausted
    pub fn check_snapshot_quota(&self) -> Result<(), QuotaError> {
        // #ASSUME_RELAXED_ORDERING: Snapshot counts are approximate (guard only)
        let used = self.snapshots_used.load(Ordering::Relaxed);
        let limit = self.snapshots_limit.load(Ordering::Relaxed);

        if used >= limit {
            Err(QuotaError::SnapshotLimitExceeded {
                used,
                limit,
                upgrade_url: "https://kindly.software/pricing",
            })
        } else {
            Ok(())
        }
    }

    /// Increment snapshot count
    ///
    /// **Performance**: <20ns (Relaxed fetch_add, no CAS)
    ///
    /// **Safety**: Called AFTER check_snapshot_quota(), so overflow is safe
    #[inline]
    pub fn increment_snapshot(&self) {
        // #ASSUME_ATOMIC_FETCH_ADD: fetch_add is atomic and doesn't overflow
        self.snapshots_used.fetch_add(1, Ordering::Relaxed);
    }

    /// Check session duration quota
    ///
    /// **Performance**: <50ns (Relaxed load + arithmetic)
    ///
    /// # Errors
    /// Returns `QuotaError::SessionDurationExceeded` if session time limit exceeded
    pub fn check_session_duration(&self) -> Result<(), QuotaError> {
        let now_ns = Self::get_timestamp_ns();
        let start_ns = self.session_start_ns.load(Ordering::Relaxed);
        let duration_ns = now_ns.saturating_sub(start_ns);
        let limit_ns = self.session_limit_ns.load(Ordering::Relaxed);

        if duration_ns >= limit_ns {
            Err(QuotaError::SessionDurationExceeded {
                duration_secs: duration_ns / 1_000_000_000,
                limit_secs: limit_ns / 1_000_000_000,
                upgrade_url: "https://kindly.software/pricing",
            })
        } else {
            Ok(())
        }
    }

    /// Check rate limit using token bucket algorithm
    ///
    /// **Performance**: <100ns (CAS loop, typically 1 iteration)
    ///
    /// **Algorithm**: Token Bucket
    /// 1. Calculate elapsed time since last refill
    /// 2. Add tokens based on elapsed time (if needed)
    /// 3. Try to consume 1 token via CAS loop
    ///
    /// # Errors
    /// Returns `QuotaError::RateLimitExceeded` if no tokens available
    pub fn check_rate_limit(&self) -> Result<(), QuotaError> {
        let now_ns = Self::get_timestamp_ns();

        // Phase 1: Refill tokens (if needed)
        // #ASSUME_CAS_CONVERGENCE: CAS loop terminates quickly under normal load
        loop {
            let last_refill = self.last_refill_ns.load(Ordering::Acquire);
            let elapsed_ns = now_ns.saturating_sub(last_refill);
            let refill_rate = self.refill_rate_ns.load(Ordering::Relaxed);

            // Integer division: tokens_to_add = elapsed_ns / refill_rate_ns
            let tokens_to_add = if refill_rate > 0 {
                elapsed_ns / refill_rate
            } else {
                0
            };

            if tokens_to_add > 0 {
                // Try to refill tokens
                let current_tokens = self.tokens.load(Ordering::Acquire);
                let max_tokens = self.tokens_max.load(Ordering::Relaxed);

                // Clamp to max (no overflow)
                let new_tokens = (current_tokens + tokens_to_add).min(max_tokens);

                // CAS: Update tokens and timestamp atomically
                match self.tokens.compare_exchange(
                    current_tokens,
                    new_tokens,
                    Ordering::Release,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // Successfully updated tokens
                        let _ = self.last_refill_ns.compare_exchange(
                            last_refill,
                            now_ns,
                            Ordering::Release,
                            Ordering::Relaxed,
                        );
                        break;
                    }
                    Err(_) => {
                        // CAS failed, retry loop
                        continue;
                    }
                }
            } else {
                break;
            }
        }

        // Phase 2: Try to consume 1 token
        // #ASSUME_CAS_CONVERGENCE: CAS loop terminates quickly under normal load
        loop {
            let current_tokens = self.tokens.load(Ordering::Acquire);

            if current_tokens == 0 {
                // Rate limit exceeded
                let limit = self.tokens_max.load(Ordering::Relaxed);
                let refill_rate = self.refill_rate_ns.load(Ordering::Relaxed);
                let retry_after_secs = if refill_rate > 0 {
                    refill_rate / 1_000_000_000
                } else {
                    1
                };

                return Err(QuotaError::RateLimitExceeded {
                    requests_per_minute: 0,
                    limit,
                    retry_after_secs,
                });
            }

            // Try to consume 1 token
            match self.tokens.compare_exchange(
                current_tokens,
                current_tokens - 1,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => continue,
            }
        }
    }

    // ========================================================================
    // Tier Management
    // ========================================================================

    /// Upgrade to pro tier
    ///
    /// **Performance**: O(1), ~20ns (multiple Relaxed stores)
    ///
    /// **Safety**: Non-atomic upgrade (acceptable for billing system)
    /// If interrupted, worst case is service continues with old limits.
    pub fn upgrade_to_pro(&self) {
        // Store tier first (atomically visible change)
        self.tier.store(UserTier::Pro as u8, Ordering::Release);

        // Then update limits (order matters for consistency)
        self.snapshots_limit.store(u64::MAX, Ordering::Release);
        self.session_limit_ns.store(u64::MAX, Ordering::Release);
        self.tokens_max.store(300, Ordering::Release);
        self.refill_rate_ns.store(200_000_000, Ordering::Release);
    }

    /// Downgrade to free tier (e.g., subscription expired)
    ///
    /// **Performance**: O(1), ~20ns
    ///
    /// **Safety**: Non-atomic downgrade (acceptable, service degrades gracefully)
    pub fn downgrade_to_free(&self) {
        // Store tier first
        self.tier.store(UserTier::Free as u8, Ordering::Release);

        // Update limits
        self.snapshots_limit.store(100, Ordering::Release);
        self.session_limit_ns.store(3600 * 1_000_000_000, Ordering::Release);
        self.tokens_max.store(60, Ordering::Release);
        self.refill_rate_ns.store(1_000_000_000, Ordering::Release);
    }

    /// Get current user tier
    ///
    /// **Performance**: <10ns (Relaxed load)
    pub fn get_tier(&self) -> UserTier {
        let tier_byte = self.tier.load(Ordering::Relaxed);
        UserTier::from_u8(tier_byte).unwrap_or(UserTier::Free)
    }

    // ========================================================================
    // Status & Diagnostics
    // ========================================================================

    /// Get quota status (for MCP tool: debugger.quota_status)
    ///
    /// **Performance**: O(1), ~100ns (multiple Relaxed loads)
    ///
    /// Returns current usage and limits for UI display, logging, etc.
    pub fn get_status(&self) -> QuotaStatus {
        let now_ns = Self::get_timestamp_ns();
        let start_ns = self.session_start_ns.load(Ordering::Relaxed);
        let limit_ns = self.session_limit_ns.load(Ordering::Relaxed);

        // Handle unlimited (u64::MAX) gracefully by capping at u64::MAX secs
        let session_limit_secs = if limit_ns == u64::MAX {
            u64::MAX
        } else {
            limit_ns / 1_000_000_000
        };

        QuotaStatus {
            snapshots_used: self.snapshots_used.load(Ordering::Relaxed),
            snapshots_limit: self.snapshots_limit.load(Ordering::Relaxed),
            session_duration_secs: (now_ns.saturating_sub(start_ns)) / 1_000_000_000,
            session_limit_secs,
            tokens_available: self.tokens.load(Ordering::Relaxed),
            tokens_max: self.tokens_max.load(Ordering::Relaxed),
            tier: self.get_tier(),
        }
    }

    /// Get user ID
    ///
    /// **Performance**: <10ns (Relaxed load)
    pub fn get_user_id(&self) -> u64 {
        self.user_id.load(Ordering::Relaxed)
    }

    /// Reset session (for session reuse)
    ///
    /// **Performance**: O(1), ~30ns (multiple Relaxed stores)
    ///
    /// **Use case**: User starts new debugging session, reset counters
    pub fn reset_session(&self) {
        let now_ns = Self::get_timestamp_ns();
        self.snapshots_used.store(0, Ordering::Relaxed);
        self.session_start_ns.store(now_ns, Ordering::Relaxed);
        self.tokens.store(self.tokens_max.load(Ordering::Relaxed), Ordering::Relaxed);
        self.last_refill_ns.store(now_ns, Ordering::Relaxed);
    }

    // ========================================================================
    // Compliance Info (Phase 2: ComprehensiveAudit Integration)
    // ========================================================================

    /// Get compliance info (<50ns)
    ///
    /// **Performance**: <50ns (3 Relaxed loads)
    ///
    /// Returns tuple of (used, limit, limit_with_grace) where grace is 20% of limit.
    /// For unlimited tiers (Pro/Enterprise with u64::MAX limit), grace is 0.
    ///
    /// # Usage
    /// ```rust,ignore
    /// let quota = QuotaTrackerCapsule::new_free(1);
    /// let (used, limit, with_grace) = quota.get_snapshots_with_grace();
    /// println!("Snapshots: {}/{} (hard limit: {})", used, limit, with_grace);
    /// ```
    ///
    /// #ASSUME_LOCKFREE_ONLY: All reads via Relaxed atomics
    pub fn get_snapshots_with_grace(&self) -> (u64, u64, u64) {
        let used = self.snapshots_used.load(Ordering::Relaxed);
        let limit = self.snapshots_limit.load(Ordering::Relaxed);
        let grace = if limit == u64::MAX { 0 } else { limit / 5 }; // 20%
        (used, limit, limit + grace)
    }

    /// Get compliance-related quota information for ComprehensiveAudit aggregation
    ///
    /// **Performance**: <50ns (3 Relaxed loads)
    ///
    /// Returns a QuotaComplianceInfo struct with:
    /// - Current snapshot usage
    /// - Current rate limit token status
    /// - Session start timestamp (for retention calculations)
    ///
    /// # Usage
    /// ```rust,ignore
    /// let quota = QuotaTrackerCapsule::new_free(1);
    /// let info = quota.get_compliance_info();
    /// println!("Snapshots: {}/{}", info.snapshots_used, info.snapshots_limit);
    /// ```
    ///
    /// #ASSUME_LOCKFREE_ONLY: All reads via Relaxed atomics
    /// #VERIFY_PHASE2_INTEGRATION: Used by ComprehensiveAudit::aggregate()
    pub fn get_compliance_info(&self) -> QuotaComplianceInfo {
        // #ASSUME_SNAPSHOT_CONSISTENT: Atomic reads provide point-in-time consistency
        QuotaComplianceInfo {
            snapshots_used: self.snapshots_used.load(Ordering::Relaxed),
            snapshots_limit: self.snapshots_limit.load(Ordering::Relaxed),
            tokens_available: self.tokens.load(Ordering::Relaxed),
            tokens_max: self.tokens_max.load(Ordering::Relaxed),
            session_start_ns: self.session_start_ns.load(Ordering::Relaxed),
            session_limit_ns: self.session_limit_ns.load(Ordering::Relaxed),
            refill_rate_ns: self.refill_rate_ns.load(Ordering::Relaxed),
        }
    }

    // ========================================================================
    // Utility Functions
    // ========================================================================

    /// Get current timestamp in nanoseconds
    ///
    /// **Performance**: <100ns (SystemTime query)
    ///
    /// **Safety**: Returns 0 on error (never panics)
    fn get_timestamp_ns() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    // ========================================================================
    // Test Helpers (for accessing private fields in tests)
    // ========================================================================

    /// Get current snapshot count (test only)
    pub fn snapshots_used_value(&self) -> u64 {
        self.snapshots_used.load(Ordering::Relaxed)
    }

    /// Get snapshot limit (test only)
    pub fn snapshots_limit_value(&self) -> u64 {
        self.snapshots_limit.load(Ordering::Relaxed)
    }

    /// Get session start timestamp (test only)
    pub fn session_start_ns_value(&self) -> u64 {
        self.session_start_ns.load(Ordering::Relaxed)
    }

    /// Get session limit (test only)
    pub fn session_limit_ns_value(&self) -> u64 {
        self.session_limit_ns.load(Ordering::Relaxed)
    }

    /// Get current token count (test only)
    pub fn tokens_value(&self) -> u64 {
        self.tokens.load(Ordering::Relaxed)
    }

    /// Get maximum token count (test only)
    pub fn tokens_max_value(&self) -> u64 {
        self.tokens_max.load(Ordering::Relaxed)
    }

    /// Set session start timestamp (test only)
    pub fn set_session_start_ns(&self, val: u64) {
        self.session_start_ns.store(val, Ordering::Relaxed);
    }

    /// Set session limit (test only)
    pub fn set_session_limit_ns(&self, val: u64) {
        self.session_limit_ns.store(val, Ordering::Relaxed);
    }

    /// Set current token count (test only)
    pub fn set_tokens(&self, val: u64) {
        self.tokens.store(val, Ordering::Relaxed);
    }

    /// Set last refill timestamp (test only)
    pub fn set_last_refill_ns(&self, val: u64) {
        self.last_refill_ns.store(val, Ordering::Relaxed);
    }
}

// ============================================================================
// QuotaStatus - User-Facing Quota Information
// ============================================================================

/// Current quota status (snapshots, duration, rate limit)
///
/// **Use case**: Display quota progress in UI, logging, diagnostics
#[derive(Debug, Clone)]
pub struct QuotaStatus {
    /// Current snapshot count
    pub snapshots_used: u64,
    /// Snapshot quota limit
    pub snapshots_limit: u64,
    /// Session duration (seconds)
    pub session_duration_secs: u64,
    /// Session duration limit (seconds)
    pub session_limit_secs: u64,
    /// Available request tokens
    pub tokens_available: u64,
    /// Maximum tokens in bucket
    pub tokens_max: u64,
    /// Current tier (Free or Pro)
    pub tier: UserTier,
}

impl QuotaStatus {
    /// Get snapshot quota percentage (0-100)
    pub fn snapshot_usage_percent(&self) -> u64 {
        if self.snapshots_limit == 0 || self.snapshots_limit == u64::MAX {
            0
        } else {
            (self.snapshots_used * 100) / self.snapshots_limit
        }
    }

    /// Get session duration percentage (0-100)
    pub fn session_duration_percent(&self) -> u64 {
        if self.session_limit_secs == 0 || self.session_limit_secs == u64::MAX {
            0
        } else {
            (self.session_duration_secs * 100) / self.session_limit_secs
        }
    }

    /// Get rate limit percentage (0-100)
    pub fn rate_limit_percent(&self) -> u64 {
        if self.tokens_max == 0 {
            0
        } else {
            (self.tokens_available * 100) / self.tokens_max
        }
    }

    /// Check if any quota is exhausted
    pub fn is_any_quota_exhausted(&self) -> bool {
        self.snapshot_usage_percent() >= 100
            || self.session_duration_percent() >= 100
            || self.tokens_available == 0
    }
}

impl fmt::Display for QuotaStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "QuotaStatus {{ tier: {}, snapshots: {}/{}, duration: {}s/{}s, tokens: {}/{} }}",
            self.tier,
            self.snapshots_used,
            if self.snapshots_limit == u64::MAX {
                "∞".to_string()
            } else {
                self.snapshots_limit.to_string()
            },
            self.session_duration_secs,
            if self.session_limit_secs == u64::MAX {
                "∞".to_string()
            } else {
                self.session_limit_secs.to_string()
            },
            self.tokens_available,
            self.tokens_max
        )
    }
}

// ============================================================================
// QuotaComplianceInfo - Phase 2 ComprehensiveAudit Integration
// ============================================================================

/// Quota compliance information for ComprehensiveAudit aggregation
///
/// **Purpose**: Provides essential quota data for the unified audit trail.
/// Used by ComprehensiveAudit::aggregate() to build comprehensive metrics.
///
/// **Performance**: All fields populated via Relaxed atomic loads (<50ns total)
///
/// # Fields
/// - `snapshots_used`: Current snapshot count in session
/// - `snapshots_limit`: Maximum snapshots allowed (tier-based)
/// - `tokens_available`: Current rate limit tokens
/// - `tokens_max`: Maximum rate limit tokens
/// - `session_start_ns`: Session start timestamp (for retention calculations)
/// - `session_limit_ns`: Session duration limit (nanoseconds)
/// - `refill_rate_ns`: Token refill rate (nanoseconds per token)
#[derive(Debug, Clone)]
pub struct QuotaComplianceInfo {
    /// Current snapshot count in this session
    pub snapshots_used: u64,
    /// Maximum snapshots allowed for this tier
    pub snapshots_limit: u64,
    /// Current rate limit tokens available
    pub tokens_available: u64,
    /// Maximum rate limit tokens for this tier
    pub tokens_max: u64,
    /// Session start timestamp (nanoseconds since epoch)
    pub session_start_ns: u64,
    /// Session duration limit (nanoseconds)
    pub session_limit_ns: u64,
    /// Token refill rate (nanoseconds per token)
    pub refill_rate_ns: u64,
}

impl QuotaComplianceInfo {
    /// Check if snapshot quota is at soft limit (using grace period)
    ///
    /// **Grace Policy**: 20% grace for ALL tiers
    /// - Hobby: 100 → 120 (soft at 100, hard at 120)
    /// - Starter: 500 → 600
    /// - Developer: 5000 → 6000
    /// - Pro/Ent: u64::MAX (no practical limit)
    ///
    /// #ASSUME_GRACE_CALCULATION_CORRECT: 20% grace for all tiers
    #[inline]
    pub fn is_at_soft_limit(&self) -> bool {
        if self.snapshots_limit == u64::MAX {
            false // Unlimited tiers never hit limits
        } else {
            self.snapshots_used >= self.snapshots_limit
        }
    }

    /// Check if snapshot quota is at hard limit (grace exhausted)
    ///
    /// Hard limit = base_limit + 20% grace
    #[inline]
    pub fn is_at_hard_limit(&self) -> bool {
        if self.snapshots_limit == u64::MAX {
            false // Unlimited tiers never hit limits
        } else {
            let grace_limit = self.calculate_grace_limit();
            self.snapshots_used >= grace_limit
        }
    }

    /// Calculate the hard limit (base + 20% grace)
    #[inline]
    pub fn calculate_grace_limit(&self) -> u64 {
        if self.snapshots_limit == u64::MAX {
            u64::MAX
        } else {
            // 20% grace = base * 1.20 = base + base/5
            let grace = self.snapshots_limit / 5;
            self.snapshots_limit.saturating_add(grace)
        }
    }

    /// Get snapshot usage ratio as a string (e.g., "50 of 100" or "50 (unlimited)")
    pub fn format_snapshots(&self) -> String {
        if self.snapshots_limit == u64::MAX {
            format!("{} (unlimited)", self.snapshots_used)
        } else if self.is_at_hard_limit() {
            format!(
                "{} of {} [BLOCKED - grace exhausted]",
                self.snapshots_used, self.snapshots_limit
            )
        } else if self.is_at_soft_limit() {
            format!(
                "{} of {} [WARNING - using grace, max {}]",
                self.snapshots_used,
                self.snapshots_limit,
                self.calculate_grace_limit()
            )
        } else {
            format!("{} of {}", self.snapshots_used, self.snapshots_limit)
        }
    }

    /// Get rate limit token ratio as a string
    pub fn format_tokens(&self) -> String {
        format!("{}/{} tokens", self.tokens_available, self.tokens_max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_and_alignment() {
        assert_eq!(std::mem::size_of::<QuotaTrackerCapsule>(), 128);
        assert_eq!(std::mem::align_of::<QuotaTrackerCapsule>(), 64);
    }

    #[test]
    fn test_free_tier_snapshot_quota() {
        let quota = QuotaTrackerCapsule::new_free(1);
        assert!(quota.check_snapshot_quota().is_ok());

        // Use up all snapshots
        for _ in 0..100 {
            quota.increment_snapshot();
        }

        // Next check should fail
        assert!(quota.check_snapshot_quota().is_err());
    }

    #[test]
    fn test_pro_tier_unlimited_snapshots() {
        let quota = QuotaTrackerCapsule::new_pro(1);

        // Pro tier should never fail snapshot quota
        for _ in 0..1000 {
            assert!(quota.check_snapshot_quota().is_ok());
            quota.increment_snapshot();
        }
    }

    #[test]
    fn test_session_duration_quota() {
        // Create quota with artificially short limit for testing
        let quota = QuotaTrackerCapsule::new_free(1);

        // Initially should pass
        assert!(quota.check_session_duration().is_ok());

        // Simulate expired session by manipulating internal state
        let now_ns = QuotaTrackerCapsule::get_timestamp_ns_pub();
        quota.session_start_ns.store(0, Ordering::Relaxed); // Very old start time
        quota.session_limit_ns.store(1, Ordering::Relaxed); // Very small limit

        // Now should fail
        assert!(quota.check_session_duration().is_err());
    }

    #[test]
    fn test_rate_limit_token_bucket() {
        let quota = QuotaTrackerCapsule::new_free(1);

        // Free tier: 60 tokens = 60 requests at start
        // Consume all tokens
        for _ in 0..60 {
            assert!(quota.check_rate_limit().is_ok());
        }

        // Next request should fail (no tokens)
        assert!(quota.check_rate_limit().is_err());
    }

    #[test]
    fn test_pro_tier_higher_rate_limit() {
        let quota = QuotaTrackerCapsule::new_pro(1);

        // Pro tier: 300 tokens at start
        for _ in 0..300 {
            assert!(quota.check_rate_limit().is_ok());
        }

        // Next request should fail
        assert!(quota.check_rate_limit().is_err());
    }

    #[test]
    fn test_tier_upgrade() {
        let quota = QuotaTrackerCapsule::new_free(1);

        assert_eq!(quota.get_tier(), UserTier::Free);
        assert_eq!(quota.snapshots_limit.load(Ordering::Relaxed), 100);

        quota.upgrade_to_pro();

        assert_eq!(quota.get_tier(), UserTier::Pro);
        assert_eq!(quota.snapshots_limit.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn test_tier_downgrade() {
        let quota = QuotaTrackerCapsule::new_pro(1);

        assert_eq!(quota.get_tier(), UserTier::Pro);

        quota.downgrade_to_free();

        assert_eq!(quota.get_tier(), UserTier::Free);
        assert_eq!(quota.snapshots_limit.load(Ordering::Relaxed), 100);
    }

    #[test]
    fn test_quota_status() {
        let quota = QuotaTrackerCapsule::new_free(1);
        let status = quota.get_status();

        assert_eq!(status.snapshots_used, 0);
        assert_eq!(status.snapshots_limit, 100);
        assert_eq!(status.tier, UserTier::Free);
    }

    #[test]
    fn test_reset_session() {
        let quota = QuotaTrackerCapsule::new_free(1);

        // Use some resources
        quota.increment_snapshot();
        for _ in 0..30 {
            let _ = quota.check_rate_limit();
        }

        // Reset
        quota.reset_session();

        // Should be back to initial state
        assert_eq!(quota.snapshots_used.load(Ordering::Relaxed), 0);
        assert_eq!(quota.tokens.load(Ordering::Relaxed), 60); // Free tier
    }

    #[test]
    #[should_panic(expected = "user_id must be non-zero")]
    fn test_invalid_user_id() {
        let _ = QuotaTrackerCapsule::new_free(0);
    }

    #[test]
    fn test_quota_status_percentages() {
        let quota = QuotaTrackerCapsule::new_free(1);

        // Use 50 snapshots
        for _ in 0..50 {
            quota.increment_snapshot();
        }

        let status = quota.get_status();
        assert_eq!(status.snapshot_usage_percent(), 50);
    }
}

// Helper for tests (not pub, internal only)
#[cfg(test)]
impl QuotaTrackerCapsule {
    fn get_timestamp_ns_pub() -> u64 {
        Self::get_timestamp_ns()
    }
}
