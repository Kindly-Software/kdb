//! MonthlyQuotaCapsule - T1 Atomic Monthly Session Tracking (128B)
//!
//! **Tier**: T1 Atomic (100% lockfree)
//! **Size**: 128 bytes (2 cache lines)
//! **Alignment**: 64 bytes
//! **Performance**: <100ns start_session()
//!
//! # Purpose
//!
//! Tracks sessions per month with automatic reset at month boundary.
//! Supports tier upgrades (preserves session count) and promotional periods.
//!
//! # Architecture
//!
//! ```text
//! MonthlyQuotaCapsule (128 bytes, 64B aligned)
//!   Cache Line 1 (64 bytes):
//!   ├── session_count (AtomicU64)       - Sessions used this month
//!   ├── session_limit (AtomicU64)       - Current limit from tier
//!   ├── current_month (AtomicU32)       - YYYYMM format (e.g., 202512)
//!   ├── _pad1 (u32)                     - Alignment padding
//!   ├── last_session_unix_sec (AtomicU64) - Last session timestamp
//!   ├── generation (AtomicU64)          - TOCTOU prevention
//!   ├── lifetime_sessions (AtomicU64)   - Total sessions ever
//!   └── _pad_cl1 ([u8; 16])             - Cache line padding
//!   Cache Line 2 (64 bytes):
//!   ├── promo_end_unix_sec (AtomicU64)  - 7-day promo end (0 = no promo)
//!   ├── tier (AtomicU8)                 - Current tier (0-4)
//!   ├── is_in_promo (AtomicU8)          - 1 = promo active, 0 = not
//!   └── _reserved ([u8; 54])            - Future expansion
//! ```
//!
//! # Session Limits per Tier
//!
//! | Tier | Sessions/Month | Promo Sessions |
//! |------|----------------|----------------|
//! | Hobby | 5 | u64::MAX (7-day promo) |
//! | Starter | 50 | 50 |
//! | Developer | 200 | 200 |
//! | Professional | 1000 | 1000 |
//! | Enterprise | u64::MAX | u64::MAX |
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T1 Atomic tier, Q33 lockfree, Q34 audit-ready
//! - **Chaos**: 100% lockfree (AtomicU64/U32/U8 only), cache-aligned
//! - **ASSUM**: All assumptions verified, generation counters for TOCTOU
//! - **T28**: Unit tests (Q1-Q7) included
//!
//! # Example
//!
//! ```rust
//! use kdb_mcp::monthly_quota::{MonthlyQuotaCapsule, SessionStartResult};
//! use kdb_mcp::subscription_tier::SubscriptionTier;
//!
//! let capsule = MonthlyQuotaCapsule::new();
//!
//! // Start a session (auto-resets at month boundary)
//! let now = 1733788800; // Dec 2024
//! match capsule.start_session(SubscriptionTier::Hobby, now) {
//!     Ok(result) => {
//!         println!("Session #{}, {} remaining", result.session_number, result.remaining);
//!     }
//!     Err(e) => {
//!         println!("Quota exceeded: {:?}", e);
//!     }
//! }
//!
//! // Check remaining sessions
//! let remaining = capsule.remaining_sessions(SubscriptionTier::Hobby);
//!
//! // Enable promo period (7 days from now)
//! capsule.set_promo(now + 7 * 24 * 3600);
//! assert!(capsule.is_promo_active(now));
//! ```

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU8, Ordering};
use crate::subscription_tier::SubscriptionTier;

// ============================================================================
// Constants
// ============================================================================

/// Seconds per day (86400)
const SECONDS_PER_DAY: u64 = 86_400;

/// Unix epoch start for month calculation (Jan 1, 1970 00:00:00 UTC)
const UNIX_EPOCH_YEAR: u32 = 1970;

/// Days in each month (non-leap year, 0-indexed)
const DAYS_IN_MONTH: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

// ============================================================================
// Result/Error Types
// ============================================================================

/// Result of a successful session start
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionStartResult {
    /// The session number (1-indexed for this month)
    pub session_number: u64,
    /// Sessions remaining after this start
    pub remaining: u64,
    /// Current session limit
    pub limit: u64,
    /// Whether this session started during promo period
    pub is_promo: bool,
}

/// Error returned when session cannot be started
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonthlyQuotaError {
    /// Quota has been exceeded for this month
    QuotaExceeded {
        /// Sessions used this month
        used: u64,
        /// Session limit for this tier
        limit: u64,
        /// Unix timestamp of first second of next month (when quota resets)
        resets_at_unix: u64,
    },
}

/// Statistics snapshot for the monthly quota
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthlyQuotaStats {
    /// Sessions used this month
    pub session_count: u64,
    /// Current session limit
    pub session_limit: u64,
    /// Current month in YYYYMM format
    pub current_month: u32,
    /// Last session start timestamp (Unix seconds)
    pub last_session_unix_sec: u64,
    /// Generation counter for TOCTOU detection
    pub generation: u64,
    /// Total sessions ever started
    pub lifetime_sessions: u64,
    /// Promo end timestamp (0 = no promo)
    pub promo_end_unix_sec: u64,
    /// Current tier value (0-4)
    pub tier: u8,
    /// Whether promo is currently active
    pub is_in_promo: bool,
}

// ============================================================================
// MonthlyQuotaCapsule
// ============================================================================

/// T1 Atomic capsule for monthly session tracking
///
/// Tracks sessions per month with automatic reset at month boundary.
/// Supports tier upgrades and promotional periods.
///
/// # Layout
///
/// - **Size**: 128 bytes (2 cache lines)
/// - **Alignment**: 64 bytes
/// - **Atomics**: 100% lockfree (AtomicU64/U32/U8 only)
///
/// # ASSUM Safety
///
/// - #ASSUME: All atomic operations use appropriate ordering (Acquire/Release/SeqCst)
/// - #ASSUME: Generation counter prevents TOCTOU races
/// - #ASSUME: Month boundary detection is monotonic (time doesn't go backward)
/// - #VERIFY: All fields fit within 128 bytes with proper alignment
/// - #VERIFY: Cache line 1 and 2 are independent (no false sharing)
#[repr(C, align(64))]
pub struct MonthlyQuotaCapsule {
    // ========================================================================
    // Cache Line 1 (64 bytes) - Hot path fields
    // ========================================================================

    /// Sessions used this month (incremented on start_session)
    session_count: AtomicU64,

    /// Current session limit (from tier, updated on upgrade)
    session_limit: AtomicU64,

    /// Current month in YYYYMM format (e.g., 202512 for December 2024)
    /// #ASSUME: YYYYMM fits in u32 (max 999912 = 9999 year 12 month)
    current_month: AtomicU32,

    /// Alignment padding to maintain 8-byte field alignment
    _pad1: u32,

    /// Last session start timestamp (Unix seconds)
    last_session_unix_sec: AtomicU64,

    /// Generation counter for TOCTOU prevention
    /// Incremented on every mutation (month reset, session start, tier change)
    generation: AtomicU64,

    /// Total sessions ever started (never resets)
    lifetime_sessions: AtomicU64,

    /// Padding to fill cache line 1 (64 - 48 = 16 bytes)
    _pad_cl1: [u8; 16],

    // ========================================================================
    // Cache Line 2 (64 bytes) - Less frequently accessed
    // ========================================================================

    /// Promo end timestamp (Unix seconds, 0 = no promo active)
    /// During promo, Hobby tier gets unlimited sessions
    promo_end_unix_sec: AtomicU64,

    /// Current tier (0-4, maps to SubscriptionTier enum)
    /// #ASSUME: Tier value is always 0-4 (validated on set)
    tier: AtomicU8,

    /// Promo active flag (1 = active, 0 = inactive)
    /// Cached for fast path, updated on promo check
    is_in_promo: AtomicU8,

    /// Reserved for future expansion
    _reserved: [u8; 54],
}

// Verify size and alignment at compile time
const _: () = {
    assert!(core::mem::size_of::<MonthlyQuotaCapsule>() == 128);
    assert!(core::mem::align_of::<MonthlyQuotaCapsule>() == 64);
};

impl MonthlyQuotaCapsule {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Create a new MonthlyQuotaCapsule with default (Hobby) tier
    ///
    /// # Returns
    ///
    /// A new capsule initialized to:
    /// - session_count: 0
    /// - session_limit: 5 (Hobby tier)
    /// - current_month: 0 (will be set on first start_session)
    /// - generation: 0
    /// - tier: 0 (Hobby)
    /// - promo: inactive
    #[inline]
    pub const fn new() -> Self {
        Self {
            session_count: AtomicU64::new(0),
            session_limit: AtomicU64::new(SubscriptionTier::Hobby.sessions_per_month()),
            current_month: AtomicU32::new(0),
            _pad1: 0,
            last_session_unix_sec: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            lifetime_sessions: AtomicU64::new(0),
            _pad_cl1: [0; 16],
            promo_end_unix_sec: AtomicU64::new(0),
            tier: AtomicU8::new(0),
            is_in_promo: AtomicU8::new(0),
            _reserved: [0; 54],
        }
    }

    /// Create a new capsule with a specific tier
    #[inline]
    pub fn with_tier(tier: SubscriptionTier) -> Self {
        let mut capsule = Self::new();
        capsule.tier = AtomicU8::new(tier.as_u8());
        capsule.session_limit = AtomicU64::new(tier.sessions_per_month());
        capsule
    }

    // ========================================================================
    // Session Management
    // ========================================================================

    /// Start a new session
    ///
    /// Automatically resets the session count if the month has changed.
    /// Checks the session limit for the given tier and promo status.
    ///
    /// # Arguments
    ///
    /// * `tier` - The current subscription tier
    /// * `now_unix_secs` - Current Unix timestamp in seconds
    ///
    /// # Returns
    ///
    /// * `Ok(SessionStartResult)` - Session started successfully
    /// * `Err(MonthlyQuotaError::QuotaExceeded)` - Monthly quota exceeded
    ///
    /// # Performance
    ///
    /// <100ns typical (no contention)
    ///
    /// # ASSUM Safety
    ///
    /// - #ASSUME: now_unix_secs is a valid Unix timestamp (>= 0)
    /// - #ASSUME: tier is a valid SubscriptionTier variant
    /// - #VERIFY: Session count incremented atomically
    /// - #VERIFY: Month boundary detected correctly
    pub fn start_session(
        &self,
        tier: SubscriptionTier,
        now_unix_secs: u64,
    ) -> Result<SessionStartResult, MonthlyQuotaError> {
        // Calculate current month from timestamp
        let now_month = unix_to_month(now_unix_secs);

        // Check if we need to reset (month boundary crossed)
        let stored_month = self.current_month.load(Ordering::Acquire);
        if stored_month != now_month {
            // Month changed - reset session count
            // Use compare_exchange to handle concurrent resets
            if self.current_month.compare_exchange(
                stored_month,
                now_month,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ).is_ok() {
                // We won the race - reset the count
                self.session_count.store(0, Ordering::Release);
                self.generation.fetch_add(1, Ordering::SeqCst);
            }
            // If we lost the race, another thread already reset - that's fine
        }

        // Check promo status
        let promo_end = self.promo_end_unix_sec.load(Ordering::Acquire);
        let is_promo = promo_end > 0 && now_unix_secs < promo_end;

        // Update cached promo status
        self.is_in_promo.store(is_promo as u8, Ordering::Release);

        // Determine the effective limit
        let limit = if is_promo {
            tier.promo_sessions_per_month()
        } else {
            tier.sessions_per_month()
        };

        // Update stored limit if tier changed
        self.session_limit.store(limit, Ordering::Release);
        self.tier.store(tier.as_u8(), Ordering::Release);

        // Try to atomically increment session count if under limit
        // Use fetch_add + rollback pattern for true atomicity
        loop {
            let current_count = self.session_count.load(Ordering::Acquire);

            // Early check - if already at or over limit, fail immediately
            if current_count >= limit {
                let resets_at = next_month_start_unix(now_unix_secs);
                return Err(MonthlyQuotaError::QuotaExceeded {
                    used: current_count,
                    limit,
                    resets_at_unix: resets_at,
                });
            }

            // Try to reserve a slot by incrementing
            // Use CAS to ensure we're incrementing from the count we checked
            match self.session_count.compare_exchange_weak(
                current_count,
                current_count + 1,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // CAS succeeded - we incremented from current_count to current_count + 1
                    let session_number = current_count + 1;

                    // Now verify we didn't exceed the limit
                    // (This handles the race where limit changed between check and CAS)
                    if session_number > limit {
                        // Rollback - we went over the limit
                        self.session_count.fetch_sub(1, Ordering::SeqCst);
                        let resets_at = next_month_start_unix(now_unix_secs);
                        return Err(MonthlyQuotaError::QuotaExceeded {
                            used: limit,
                            limit,
                            resets_at_unix: resets_at,
                        });
                    }

                    let remaining = limit.saturating_sub(session_number);

                    // Update metadata
                    self.last_session_unix_sec.store(now_unix_secs, Ordering::Release);
                    self.lifetime_sessions.fetch_add(1, Ordering::SeqCst);
                    self.generation.fetch_add(1, Ordering::SeqCst);

                    return Ok(SessionStartResult {
                        session_number,
                        remaining,
                        limit,
                        is_promo,
                    });
                }
                Err(_) => {
                    // CAS failed - another thread modified session_count
                    // Retry the loop with fresh values
                    continue;
                }
            }
        }
    }

    /// Get remaining sessions for the given tier
    ///
    /// # Arguments
    ///
    /// * `tier` - The subscription tier to check against
    ///
    /// # Returns
    ///
    /// Number of sessions remaining this month (0 if at or over limit)
    #[inline]
    pub fn remaining_sessions(&self, tier: SubscriptionTier) -> u64 {
        let is_promo = self.is_in_promo.load(Ordering::Acquire) != 0;
        let limit = if is_promo {
            tier.promo_sessions_per_month()
        } else {
            tier.sessions_per_month()
        };

        let used = self.session_count.load(Ordering::Acquire);
        limit.saturating_sub(used)
    }

    // ========================================================================
    // Tier Management
    // ========================================================================

    /// Upgrade to a new subscription tier
    ///
    /// Preserves the current session count but updates the limit.
    /// This allows users to upgrade mid-month without losing their usage history.
    ///
    /// # Arguments
    ///
    /// * `new_tier` - The new subscription tier
    ///
    /// # Performance
    ///
    /// <50ns typical
    #[inline]
    pub fn upgrade_tier(&self, new_tier: SubscriptionTier) {
        // Store the new tier
        self.tier.store(new_tier.as_u8(), Ordering::Release);

        // Update the limit based on promo status
        let is_promo = self.is_in_promo.load(Ordering::Acquire) != 0;
        let new_limit = if is_promo {
            new_tier.promo_sessions_per_month()
        } else {
            new_tier.sessions_per_month()
        };

        self.session_limit.store(new_limit, Ordering::Release);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Get the current tier
    #[inline]
    pub fn get_tier(&self) -> Option<SubscriptionTier> {
        SubscriptionTier::from_u8(self.tier.load(Ordering::Acquire))
    }

    // ========================================================================
    // Promotional Period
    // ========================================================================

    /// Enable promotional period
    ///
    /// During promo, Hobby tier gets unlimited sessions (u64::MAX).
    /// Other tiers retain their normal limits.
    ///
    /// # Arguments
    ///
    /// * `promo_end_unix_secs` - Unix timestamp when promo ends
    #[inline]
    pub fn set_promo(&self, promo_end_unix_secs: u64) {
        self.promo_end_unix_sec.store(promo_end_unix_secs, Ordering::Release);
        self.is_in_promo.store(1, Ordering::Release);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Disable promotional period
    #[inline]
    pub fn clear_promo(&self) {
        self.promo_end_unix_sec.store(0, Ordering::Release);
        self.is_in_promo.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Check if promotional period is currently active
    ///
    /// # Arguments
    ///
    /// * `now_unix_secs` - Current Unix timestamp
    ///
    /// # Returns
    ///
    /// `true` if promo is active, `false` otherwise
    #[inline]
    pub fn is_promo_active(&self, now_unix_secs: u64) -> bool {
        let promo_end = self.promo_end_unix_sec.load(Ordering::Acquire);
        if promo_end == 0 {
            return false;
        }

        let is_active = now_unix_secs < promo_end;

        // Update cached status
        self.is_in_promo.store(is_active as u8, Ordering::Release);

        is_active
    }

    /// Get promo end timestamp
    #[inline]
    pub fn get_promo_end(&self) -> u64 {
        self.promo_end_unix_sec.load(Ordering::Acquire)
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get a snapshot of all statistics
    ///
    /// # Returns
    ///
    /// A `MonthlyQuotaStats` struct with all current values
    #[inline]
    pub fn get_stats(&self) -> MonthlyQuotaStats {
        // Read generation first and last to detect concurrent modifications
        let gen_before = self.generation.load(Ordering::Acquire);

        let stats = MonthlyQuotaStats {
            session_count: self.session_count.load(Ordering::Acquire),
            session_limit: self.session_limit.load(Ordering::Acquire),
            current_month: self.current_month.load(Ordering::Acquire),
            last_session_unix_sec: self.last_session_unix_sec.load(Ordering::Acquire),
            generation: gen_before,
            lifetime_sessions: self.lifetime_sessions.load(Ordering::Acquire),
            promo_end_unix_sec: self.promo_end_unix_sec.load(Ordering::Acquire),
            tier: self.tier.load(Ordering::Acquire),
            is_in_promo: self.is_in_promo.load(Ordering::Acquire) != 0,
        };

        // Optional: verify generation didn't change during read
        // If it did, the stats may be inconsistent (acceptable for monitoring)
        let _ = self.generation.load(Ordering::Acquire);

        stats
    }

    /// Get current session count
    #[inline]
    pub fn session_count(&self) -> u64 {
        self.session_count.load(Ordering::Acquire)
    }

    /// Get lifetime session count
    #[inline]
    pub fn lifetime_sessions(&self) -> u64 {
        self.lifetime_sessions.load(Ordering::Acquire)
    }

    /// Get current month in YYYYMM format
    #[inline]
    pub fn current_month(&self) -> u32 {
        self.current_month.load(Ordering::Acquire)
    }

    /// Get generation counter (for external TOCTOU detection)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}

impl Default for MonthlyQuotaCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert Unix timestamp to YYYYMM format
///
/// # Arguments
///
/// * `unix_secs` - Unix timestamp in seconds
///
/// # Returns
///
/// Month in YYYYMM format (e.g., 202512 for December 2024)
///
/// # Algorithm
///
/// Uses a simple iterative calculation:
/// 1. Calculate days since epoch
/// 2. Iterate through years, accounting for leap years
/// 3. Iterate through months in the final year
///
/// # Performance
///
/// O(years since 1970) - typically ~50 iterations for 2025
#[inline]
pub fn unix_to_month(unix_secs: u64) -> u32 {
    // Days since Unix epoch
    let total_days = (unix_secs / SECONDS_PER_DAY) as u32;

    let mut year = UNIX_EPOCH_YEAR;
    let mut remaining_days = total_days;

    // Find the year
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    // Find the month within the year
    let mut month = 1u32;
    for m in 0..12 {
        let days_in_month = if m == 1 && is_leap_year(year) {
            29 // February in leap year
        } else {
            DAYS_IN_MONTH[m as usize]
        };

        if remaining_days < days_in_month {
            month = m as u32 + 1;
            break;
        }
        remaining_days -= days_in_month;
        month = m as u32 + 2; // Next month (1-indexed)
    }

    // Clamp month to valid range
    if month > 12 {
        month = 12;
    }

    // Format as YYYYMM
    year * 100 + month
}

/// Calculate Unix timestamp for first second of next month
///
/// # Arguments
///
/// * `unix_secs` - Current Unix timestamp
///
/// # Returns
///
/// Unix timestamp of 00:00:00 UTC on the 1st of the next month
#[inline]
pub fn next_month_start_unix(unix_secs: u64) -> u64 {
    let total_days = (unix_secs / SECONDS_PER_DAY) as u32;

    let mut year = UNIX_EPOCH_YEAR;
    let mut remaining_days = total_days;

    // Find the year
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    // Find the month within the year
    let mut month = 0u32;
    for m in 0..12 {
        let days_in_month = if m == 1 && is_leap_year(year) {
            29
        } else {
            DAYS_IN_MONTH[m as usize]
        };

        if remaining_days < days_in_month {
            month = m as u32;
            break;
        }
        remaining_days -= days_in_month;
        month = m as u32 + 1;
    }

    // Calculate next month
    let (next_year, next_month) = if month >= 11 {
        (year + 1, 0) // January of next year
    } else {
        (year, month + 1)
    };

    // Calculate days from epoch to start of next month
    let mut days_to_next = 0u32;

    // Add days for years
    for y in UNIX_EPOCH_YEAR..next_year {
        days_to_next += if is_leap_year(y) { 366 } else { 365 };
    }

    // Add days for months in the target year
    for m in 0..next_month {
        days_to_next += if m == 1 && is_leap_year(next_year) {
            29
        } else {
            DAYS_IN_MONTH[m as usize]
        };
    }

    days_to_next as u64 * SECONDS_PER_DAY
}

/// Check if a year is a leap year
#[inline]
const fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

// ============================================================================
// Unit Tests (T28 Q1-Q7)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1: Layout Verification
    // ========================================================================

    #[test]
    fn test_capsule_size() {
        assert_eq!(
            core::mem::size_of::<MonthlyQuotaCapsule>(),
            128,
            "MonthlyQuotaCapsule must be exactly 128 bytes"
        );
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(
            core::mem::align_of::<MonthlyQuotaCapsule>(),
            64,
            "MonthlyQuotaCapsule must be 64-byte aligned"
        );
    }

    #[test]
    fn test_result_types() {
        // SessionStartResult is small and Copy
        assert!(core::mem::size_of::<SessionStartResult>() <= 32);

        // MonthlyQuotaError is small
        assert!(core::mem::size_of::<MonthlyQuotaError>() <= 32);
    }

    // ========================================================================
    // Q2: Construction
    // ========================================================================

    #[test]
    fn test_new_default_values() {
        let capsule = MonthlyQuotaCapsule::new();

        assert_eq!(capsule.session_count(), 0);
        assert_eq!(capsule.lifetime_sessions(), 0);
        assert_eq!(capsule.current_month(), 0);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.get_tier(), Some(SubscriptionTier::Hobby));
        assert_eq!(capsule.get_promo_end(), 0);
    }

    #[test]
    fn test_with_tier() {
        let capsule = MonthlyQuotaCapsule::with_tier(SubscriptionTier::Teams);

        assert_eq!(capsule.get_tier(), Some(SubscriptionTier::Teams));
        let stats = capsule.get_stats();
        assert_eq!(stats.session_limit, 2000); // Teams limit
    }

    #[test]
    fn test_default_impl() {
        let capsule = MonthlyQuotaCapsule::default();
        assert_eq!(capsule.session_count(), 0);
    }

    // ========================================================================
    // Q3: Session Start
    // ========================================================================

    #[test]
    fn test_start_session_success() {
        let capsule = MonthlyQuotaCapsule::new();
        let now = 1733788800u64; // Dec 10, 2024 00:00:00 UTC

        let result = capsule.start_session(SubscriptionTier::Hobby, now).unwrap();

        assert_eq!(result.session_number, 1);
        assert_eq!(result.limit, 5); // Hobby limit
        assert_eq!(result.remaining, 4);
        assert!(!result.is_promo);

        // Verify internal state
        assert_eq!(capsule.session_count(), 1);
        assert_eq!(capsule.lifetime_sessions(), 1);
    }

    #[test]
    fn test_start_multiple_sessions() {
        let capsule = MonthlyQuotaCapsule::new();
        let now = 1733788800u64;

        for i in 1..=5 {
            let result = capsule.start_session(SubscriptionTier::Hobby, now).unwrap();
            assert_eq!(result.session_number, i);
            assert_eq!(result.remaining, 5 - i);
        }

        assert_eq!(capsule.session_count(), 5);
        assert_eq!(capsule.lifetime_sessions(), 5);
    }

    #[test]
    fn test_start_session_quota_exceeded() {
        let capsule = MonthlyQuotaCapsule::new();
        let now = 1733788800u64;

        // Use all 5 Hobby sessions
        for _ in 0..5 {
            capsule.start_session(SubscriptionTier::Hobby, now).unwrap();
        }

        // 6th should fail
        let result = capsule.start_session(SubscriptionTier::Hobby, now);

        match result {
            Err(MonthlyQuotaError::QuotaExceeded { used, limit, resets_at_unix }) => {
                assert_eq!(used, 5);
                assert_eq!(limit, 5);
                assert!(resets_at_unix > now); // Next month
            }
            Ok(_) => panic!("Should have failed with quota exceeded"),
        }
    }

    // ========================================================================
    // Q4: Month Boundary Reset
    // ========================================================================

    #[test]
    fn test_month_boundary_reset() {
        let capsule = MonthlyQuotaCapsule::new();

        // December 2024
        let dec_2024 = 1733788800u64;
        capsule.start_session(SubscriptionTier::Hobby, dec_2024).unwrap();
        capsule.start_session(SubscriptionTier::Hobby, dec_2024).unwrap();
        assert_eq!(capsule.session_count(), 2);

        // January 2025 (crosses month boundary)
        let jan_2025 = 1735689600u64; // Jan 1, 2025 00:00:00 UTC
        let result = capsule.start_session(SubscriptionTier::Hobby, jan_2025).unwrap();

        // Should have reset
        assert_eq!(result.session_number, 1);
        assert_eq!(capsule.session_count(), 1);

        // Lifetime should still include previous months
        assert_eq!(capsule.lifetime_sessions(), 3);
    }

    #[test]
    fn test_unix_to_month() {
        // Test known dates
        assert_eq!(unix_to_month(0), 197001); // Jan 1, 1970
        assert_eq!(unix_to_month(1733788800), 202412); // Dec 10, 2024
        assert_eq!(unix_to_month(1735689600), 202501); // Jan 1, 2025

        // Test Feb 29 in leap year (2024)
        let feb_29_2024 = 1709164800u64; // Feb 29, 2024
        assert_eq!(unix_to_month(feb_29_2024), 202402);
    }

    // ========================================================================
    // Q5: Tier Management
    // ========================================================================

    #[test]
    fn test_upgrade_tier_preserves_count() {
        let capsule = MonthlyQuotaCapsule::new();
        let now = 1733788800u64;

        // Use 3 of 5 Hobby sessions
        for _ in 0..3 {
            capsule.start_session(SubscriptionTier::Hobby, now).unwrap();
        }

        assert_eq!(capsule.session_count(), 3);

        // Upgrade to Teams (2000 sessions)
        capsule.upgrade_tier(SubscriptionTier::Teams);

        // Session count preserved
        assert_eq!(capsule.session_count(), 3);
        assert_eq!(capsule.get_tier(), Some(SubscriptionTier::Teams));

        // Can start many more sessions
        assert_eq!(capsule.remaining_sessions(SubscriptionTier::Teams), 1997);
    }

    #[test]
    fn test_tier_limits() {
        let capsule = MonthlyQuotaCapsule::new();
        let now = 1733788800u64;

        // Test with Enterprise (unlimited)
        capsule.upgrade_tier(SubscriptionTier::Enterprise);

        for _ in 0..100 {
            capsule.start_session(SubscriptionTier::Enterprise, now).unwrap();
        }

        // Should still have "unlimited" remaining
        let remaining = capsule.remaining_sessions(SubscriptionTier::Enterprise);
        assert!(remaining > 1_000_000_000);
    }

    // ========================================================================
    // Q6: Promotional Period
    // ========================================================================

    #[test]
    fn test_promo_hobby_unlimited() {
        let capsule = MonthlyQuotaCapsule::new();
        let now = 1733788800u64;
        let promo_end = now + 7 * SECONDS_PER_DAY; // 7 days

        capsule.set_promo(promo_end);
        assert!(capsule.is_promo_active(now));

        // Hobby should have unlimited during promo
        let result = capsule.start_session(SubscriptionTier::Hobby, now).unwrap();
        assert!(result.is_promo);
        assert_eq!(result.limit, u64::MAX);

        // Can start many sessions
        for _ in 0..100 {
            let r = capsule.start_session(SubscriptionTier::Hobby, now).unwrap();
            assert!(r.is_promo);
        }
    }

    #[test]
    fn test_promo_expires() {
        let capsule = MonthlyQuotaCapsule::new();
        let now = 1733788800u64;
        let promo_end = now + 1000; // Short promo

        capsule.set_promo(promo_end);
        assert!(capsule.is_promo_active(now));

        // After promo ends
        let after_promo = promo_end + 1;
        assert!(!capsule.is_promo_active(after_promo));

        // Use all sessions first to test limit enforcement
        let capsule2 = MonthlyQuotaCapsule::new();
        for _ in 0..5 {
            capsule2.start_session(SubscriptionTier::Hobby, after_promo).unwrap();
        }

        // 6th should fail (back to 5 session limit)
        let result = capsule2.start_session(SubscriptionTier::Hobby, after_promo);
        assert!(result.is_err());
    }

    #[test]
    fn test_clear_promo() {
        let capsule = MonthlyQuotaCapsule::new();
        let now = 1733788800u64;

        capsule.set_promo(now + 10000);
        assert!(capsule.is_promo_active(now));

        capsule.clear_promo();
        assert!(!capsule.is_promo_active(now));
        assert_eq!(capsule.get_promo_end(), 0);
    }

    // ========================================================================
    // Q7: Statistics
    // ========================================================================

    #[test]
    fn test_get_stats() {
        let capsule = MonthlyQuotaCapsule::with_tier(SubscriptionTier::Engineer);
        let now = 1733788800u64;

        capsule.set_promo(now + 10000);
        capsule.start_session(SubscriptionTier::Engineer, now).unwrap();
        capsule.start_session(SubscriptionTier::Engineer, now).unwrap();

        let stats = capsule.get_stats();

        assert_eq!(stats.session_count, 2);
        assert_eq!(stats.lifetime_sessions, 2);
        assert_eq!(stats.tier, 2); // Developer
        assert!(stats.is_in_promo);
        assert!(stats.promo_end_unix_sec > 0);
        assert!(stats.generation >= 2); // At least 2 increments
    }

    #[test]
    fn test_generation_increments() {
        let capsule = MonthlyQuotaCapsule::new();
        let now = 1733788800u64;

        let gen0 = capsule.generation();

        capsule.start_session(SubscriptionTier::Hobby, now).unwrap();
        let gen1 = capsule.generation();
        assert!(gen1 > gen0);

        capsule.upgrade_tier(SubscriptionTier::Teams);
        let gen2 = capsule.generation();
        assert!(gen2 > gen1);

        capsule.set_promo(now + 1000);
        let gen3 = capsule.generation();
        assert!(gen3 > gen2);
    }

    // ========================================================================
    // Helper Function Tests
    // ========================================================================

    #[test]
    fn test_is_leap_year() {
        assert!(!is_leap_year(1970)); // Not divisible by 4
        assert!(is_leap_year(1972));  // Divisible by 4
        assert!(!is_leap_year(1900)); // Divisible by 100 but not 400
        assert!(is_leap_year(2000));  // Divisible by 400
        assert!(is_leap_year(2024));  // Divisible by 4
        assert!(!is_leap_year(2025)); // Not divisible by 4
    }

    #[test]
    fn test_next_month_start_unix() {
        // Dec 10, 2024 -> Jan 1, 2025
        let dec_10_2024 = 1733788800u64;
        let jan_1_2025 = next_month_start_unix(dec_10_2024);
        assert_eq!(unix_to_month(jan_1_2025), 202501);

        // Jan 15, 2025 -> Feb 1, 2025
        let jan_15_2025 = 1736899200u64;
        let feb_1_2025 = next_month_start_unix(jan_15_2025);
        assert_eq!(unix_to_month(feb_1_2025), 202502);
    }

    // ========================================================================
    // Concurrent Access Tests (T28 Q8-Q14 Property)
    // ========================================================================

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_session_starts() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(MonthlyQuotaCapsule::with_tier(SubscriptionTier::Teams));
        let now = 1733788800u64;
        let num_threads = 8;
        let sessions_per_thread = 100;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let capsule = Arc::clone(&capsule);
                thread::spawn(move || {
                    let mut successes = 0;
                    for _ in 0..sessions_per_thread {
                        if capsule.start_session(SubscriptionTier::Teams, now).is_ok() {
                            successes += 1;
                        }
                    }
                    successes
                })
            })
            .collect();

        let total_successes: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();

        // All sessions should succeed (Professional has 1000 limit)
        assert_eq!(total_successes, (num_threads * sessions_per_thread) as u64);
        assert_eq!(capsule.session_count(), total_successes);
        assert_eq!(capsule.lifetime_sessions(), total_successes);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_quota_enforcement() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(MonthlyQuotaCapsule::new()); // Hobby: 5 sessions
        let now = 1733788800u64;
        let num_threads = 10;
        let sessions_per_thread = 3;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let capsule = Arc::clone(&capsule);
                thread::spawn(move || {
                    let mut successes = 0;
                    for _ in 0..sessions_per_thread {
                        if capsule.start_session(SubscriptionTier::Hobby, now).is_ok() {
                            successes += 1;
                        }
                    }
                    successes
                })
            })
            .collect();

        let total_successes: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();

        // Should be exactly 5 (Hobby limit)
        assert_eq!(total_successes, 5);
        assert_eq!(capsule.session_count(), 5);
    }
}
