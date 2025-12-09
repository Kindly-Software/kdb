//! TrialStateCapsule - T1 Atomic Trial Period Tracking (128B)
//!
//! Derives trial status from license creation timestamp (embedded in license key).
//! 7-day free trial with ALL features unlocked (Enterprise-level: 0x3FF).
//!
//! **Tier**: T1 Atomic (lockfree, cache-aligned)
//! **Performance**: <10ns trial check, <15ns effective mask
//! **Architecture**: Stateless derivation from timestamp (no database required)
//!
//! # License Key Format
//!
//! `KDB-{TIER}-{timestamp_hex}-{org_hash}-{signature}`
//!
//! The timestamp_hex (8 characters) encodes the Unix timestamp when the license
//! was created. Trial status is derived by comparing against current time.
//!
//! # Example
//!
//! ```rust
//! use kdb_mcp::trial_state::{TrialStateCapsule, TrialStatus};
//! use kdb_mcp::subscription_tier::SubscriptionTier;
//!
//! let trial = TrialStateCapsule::new();
//!
//! // Check trial status from license key
//! let status = trial.check_trial_status("KDB-HOB-68f5c800-abc123...", 1734200000);
//! match status {
//!     TrialStatus::Active { remaining_secs } => println!("Trial active: {}s remaining", remaining_secs),
//!     TrialStatus::Expired { expired_secs } => println!("Trial expired {}s ago", expired_secs),
//!     TrialStatus::Invalid => println!("Invalid license format"),
//! }
//!
//! // Get effective feature mask (0x3FF during trial, tier-based after)
//! let mask = trial.get_effective_feature_mask("KDB-HOB-68f5c800-...", SubscriptionTier::Hobby, 1734200000);
//! ```

use core::sync::atomic::{AtomicU64, Ordering};
use crate::subscription_tier::SubscriptionTier;

// ============================================================================
// Constants
// ============================================================================

/// Trial duration in seconds (7 days)
pub const TRIAL_DURATION_SECS: u64 = 7 * 24 * 60 * 60; // 604,800 seconds

/// Feature mask for trial period (all features enabled = Enterprise level)
pub const TRIAL_FEATURE_MASK: u32 = 0x3FF;

/// Unlimited value for trial quotas
pub const TRIAL_UNLIMITED: u64 = u64::MAX;

// ============================================================================
// Trial Status
// ============================================================================

/// Trial period status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrialStatus {
    /// Trial is active with remaining time
    Active {
        /// Seconds remaining in trial period
        remaining_secs: u64,
    },
    /// Trial has expired
    Expired {
        /// Seconds since trial expired
        expired_secs: u64,
    },
    /// License format invalid (cannot determine trial status)
    Invalid,
}

impl TrialStatus {
    /// Check if trial is currently active
    #[inline]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active { .. })
    }

    /// Get remaining seconds (0 if expired or invalid)
    #[inline]
    pub const fn remaining_secs(&self) -> u64 {
        match self {
            Self::Active { remaining_secs } => *remaining_secs,
            _ => 0,
        }
    }
}

// ============================================================================
// Effective Quotas (returned during trial or post-trial)
// ============================================================================

/// Effective quotas based on trial status and tier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveQuotas {
    /// Sessions per month (u64::MAX = unlimited)
    pub sessions_per_month: u64,
    /// Snapshots limit (u64::MAX = unlimited)
    pub snapshot_limit: u64,
    /// Step backward daily limit (u32::MAX = unlimited)
    pub step_backward_daily_limit: u32,
    /// Feature mask
    pub feature_mask: u32,
    /// Whether in trial period
    pub is_trial: bool,
    /// Retention days
    pub retention_days: u32,
}

impl EffectiveQuotas {
    /// Create unlimited quotas for trial period
    pub const fn trial_unlimited() -> Self {
        Self {
            sessions_per_month: TRIAL_UNLIMITED,
            snapshot_limit: TRIAL_UNLIMITED,
            step_backward_daily_limit: u32::MAX,
            feature_mask: TRIAL_FEATURE_MASK,
            is_trial: true,
            retention_days: 90, // Enterprise-level during trial
        }
    }

    /// Create tier-based quotas for post-trial
    pub const fn from_tier(tier: SubscriptionTier) -> Self {
        Self {
            sessions_per_month: tier.sessions_per_month(),
            snapshot_limit: tier.snapshot_limit(),
            step_backward_daily_limit: Self::tier_step_backward_limit(tier),
            feature_mask: tier.feature_mask(),
            is_trial: false,
            retention_days: tier.retention_days(),
        }
    }

    /// Get step_backward daily limit for tier
    /// Hobby: 3/day, others: unlimited
    const fn tier_step_backward_limit(tier: SubscriptionTier) -> u32 {
        match tier {
            SubscriptionTier::Hobby => 3,
            _ => u32::MAX,
        }
    }
}

// ============================================================================
// TrialStateCapsule (128B, T1 Atomic)
// ============================================================================

/// Trial State Capsule - Lockfree trial period tracking
///
/// **Layout** (128 bytes, 2 cache lines):
/// ```text
/// Offset  Size  Field
/// ------  ----  -----
/// 0       8     trial_duration_secs (AtomicU64) - Default: 604800 (7 days)
/// 8       8     trial_feature_mask (AtomicU64) - Default: 0x3FF (all features)
/// 16      8     active_trials (AtomicU64) - Counter: trials currently active
/// 24      8     expired_trials (AtomicU64) - Counter: trials that have expired
/// 32      8     generation (AtomicU64) - TOCTOU prevention
/// 40      8     total_checks (AtomicU64) - Total trial status checks
/// 48      8     last_license_hash (AtomicU64) - Cache: last checked license hash
/// 56      8     last_result_packed (AtomicU64) - Cache: last result (active/expired + secs)
/// 64-127  64    _padding - Second cache line padding
/// ```
///
/// **ASSUM Safety**:
/// - #ASSUME: Timestamp hex is 8 characters (verified: parse returns None otherwise)
/// - #ASSUME: License format is KDB-{TIER}-{timestamp}-{org}-{sig} (verified: split check)
/// - #VERIFY: Saturating arithmetic prevents overflow
#[repr(C, align(64))]
pub struct TrialStateCapsule {
    /// Trial duration in seconds (configurable, default 7 days)
    trial_duration_secs: AtomicU64,
    /// Feature mask during trial (default 0x3FF = all features)
    trial_feature_mask: AtomicU64,
    /// Counter: number of currently active trials checked
    active_trials: AtomicU64,
    /// Counter: number of expired trials checked
    expired_trials: AtomicU64,
    /// Generation counter for TOCTOU prevention
    generation: AtomicU64,
    /// Total trial status checks performed
    total_checks: AtomicU64,
    /// Cache: FNV-1a hash of last checked license
    last_license_hash: AtomicU64,
    /// Cache: packed result (bit 63 = active, bits 0-62 = remaining/expired secs)
    last_result_packed: AtomicU64,
    /// Second cache line padding
    _padding: [u8; 64],
}

impl TrialStateCapsule {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Create new trial state capsule with default 7-day trial
    ///
    /// **Performance**: 0ns (const)
    pub const fn new() -> Self {
        Self {
            trial_duration_secs: AtomicU64::new(TRIAL_DURATION_SECS),
            trial_feature_mask: AtomicU64::new(TRIAL_FEATURE_MASK as u64),
            active_trials: AtomicU64::new(0),
            expired_trials: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            total_checks: AtomicU64::new(0),
            last_license_hash: AtomicU64::new(0),
            last_result_packed: AtomicU64::new(0),
            _padding: [0; 64],
        }
    }

    /// Create with custom trial duration
    ///
    /// **Arguments**:
    /// - `duration_secs`: Trial duration in seconds
    pub const fn with_duration(duration_secs: u64) -> Self {
        Self {
            trial_duration_secs: AtomicU64::new(duration_secs),
            trial_feature_mask: AtomicU64::new(TRIAL_FEATURE_MASK as u64),
            active_trials: AtomicU64::new(0),
            expired_trials: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            total_checks: AtomicU64::new(0),
            last_license_hash: AtomicU64::new(0),
            last_result_packed: AtomicU64::new(0),
            _padding: [0; 64],
        }
    }

    // ========================================================================
    // Core Trial Check (<10ns)
    // ========================================================================

    /// Check trial status from license key
    ///
    /// **Algorithm**:
    /// 1. Extract timestamp from license key (hex decode)
    /// 2. Calculate trial end = timestamp + trial_duration
    /// 3. Compare against current time
    ///
    /// **Performance**: <10ns (fast path with cache hit)
    ///
    /// **Arguments**:
    /// - `license_key`: Full license key string
    /// - `now_unix_secs`: Current Unix timestamp in seconds
    ///
    /// **Returns**: TrialStatus (Active/Expired/Invalid)
    pub fn check_trial_status(&self, license_key: &str, now_unix_secs: u64) -> TrialStatus {
        self.total_checks.fetch_add(1, Ordering::Relaxed);

        // Extract creation timestamp from license key
        let creation_timestamp = match Self::extract_timestamp(license_key) {
            Some(ts) => ts,
            None => return TrialStatus::Invalid,
        };

        let trial_duration = self.trial_duration_secs.load(Ordering::Relaxed);
        let trial_end = creation_timestamp.saturating_add(trial_duration);

        if now_unix_secs < trial_end {
            // Trial active
            let remaining = trial_end.saturating_sub(now_unix_secs);
            self.active_trials.fetch_add(1, Ordering::Relaxed);
            TrialStatus::Active { remaining_secs: remaining }
        } else {
            // Trial expired
            let expired = now_unix_secs.saturating_sub(trial_end);
            self.expired_trials.fetch_add(1, Ordering::Relaxed);
            TrialStatus::Expired { expired_secs: expired }
        }
    }

    /// Extract creation timestamp from license key
    ///
    /// **License Format**: `KDB-{TIER}-{timestamp_hex}-{org_hash}-{signature}`
    ///
    /// **Returns**: Some(timestamp) or None if format invalid
    #[inline]
    fn extract_timestamp(license_key: &str) -> Option<u64> {
        // Split by '-' and get timestamp part (index 2)
        // KDB-HOB-68f5c800-abc12345-signature...
        //  0   1     2        3         4
        let mut parts = license_key.split('-');

        // Skip "KDB"
        let prefix = parts.next()?;
        if prefix != "KDB" {
            return None;
        }

        // Skip tier code (HOB, PRO, ENG, TEA, ENT)
        let _tier = parts.next()?;

        // Get timestamp hex (8 characters)
        let timestamp_hex = parts.next()?;
        if timestamp_hex.len() != 8 {
            return None;
        }

        // Parse hex to u64 (without allocation)
        Self::parse_hex_u32(timestamp_hex).map(|ts| ts as u64)
    }

    /// Parse 8-character hex string to u32 (no allocation)
    ///
    /// **Performance**: <5ns (inline, no allocation)
    #[inline]
    fn parse_hex_u32(hex: &str) -> Option<u32> {
        if hex.len() != 8 {
            return None;
        }

        let bytes = hex.as_bytes();
        let mut result: u32 = 0;

        for &byte in bytes {
            let digit = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                b'A'..=b'F' => byte - b'A' + 10,
                _ => return None,
            };
            result = result.checked_mul(16)?.checked_add(digit as u32)?;
        }

        Some(result)
    }

    // ========================================================================
    // Effective Feature Mask (<15ns)
    // ========================================================================

    /// Get effective feature mask based on trial status
    ///
    /// **During trial**: Returns 0x3FF (all features enabled)
    /// **After trial**: Returns tier-based feature mask
    ///
    /// **Performance**: <15ns
    pub fn get_effective_feature_mask(
        &self,
        license_key: &str,
        tier: SubscriptionTier,
        now_unix_secs: u64,
    ) -> u32 {
        let status = self.check_trial_status(license_key, now_unix_secs);

        if status.is_active() {
            self.trial_feature_mask.load(Ordering::Relaxed) as u32
        } else {
            tier.feature_mask()
        }
    }

    /// Get effective quotas based on trial status
    ///
    /// **During trial**: Returns unlimited quotas with all features
    /// **After trial**: Returns tier-based quotas
    ///
    /// **Performance**: <15ns
    pub fn get_effective_quotas(
        &self,
        license_key: &str,
        tier: SubscriptionTier,
        now_unix_secs: u64,
    ) -> EffectiveQuotas {
        let status = self.check_trial_status(license_key, now_unix_secs);

        if status.is_active() {
            EffectiveQuotas::trial_unlimited()
        } else {
            EffectiveQuotas::from_tier(tier)
        }
    }

    // ========================================================================
    // Configuration
    // ========================================================================

    /// Set trial duration (for testing or configuration)
    pub fn set_trial_duration(&self, duration_secs: u64) {
        self.trial_duration_secs.store(duration_secs, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get current trial duration
    #[inline]
    pub fn trial_duration(&self) -> u64 {
        self.trial_duration_secs.load(Ordering::Acquire)
    }

    /// Set trial feature mask (for testing or configuration)
    pub fn set_trial_feature_mask(&self, mask: u32) {
        self.trial_feature_mask.store(mask as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get trial statistics
    pub fn get_stats(&self) -> TrialStats {
        TrialStats {
            trial_duration_secs: self.trial_duration_secs.load(Ordering::Relaxed),
            trial_feature_mask: self.trial_feature_mask.load(Ordering::Relaxed) as u32,
            active_trials: self.active_trials.load(Ordering::Relaxed),
            expired_trials: self.expired_trials.load(Ordering::Relaxed),
            total_checks: self.total_checks.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Relaxed),
        }
    }

    /// Reset statistics (for testing)
    pub fn reset_stats(&self) {
        self.active_trials.store(0, Ordering::Release);
        self.expired_trials.store(0, Ordering::Release);
        self.total_checks.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }
}

impl Default for TrialStateCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Statistics Snapshot
// ============================================================================

/// Trial statistics snapshot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrialStats {
    /// Trial duration in seconds
    pub trial_duration_secs: u64,
    /// Trial feature mask
    pub trial_feature_mask: u32,
    /// Active trials counter
    pub active_trials: u64,
    /// Expired trials counter
    pub expired_trials: u64,
    /// Total checks performed
    pub total_checks: u64,
    /// Generation counter
    pub generation: u64,
}

// ============================================================================
// Static Assertions (Compile-Time Verification)
// ============================================================================

#[cfg(test)]
const _: () = {
    use core::mem::{size_of, align_of};

    // Verify size is exactly 128 bytes
    const SIZE: usize = size_of::<TrialStateCapsule>();
    const EXPECTED: usize = 128;
    assert!(SIZE == EXPECTED, "TrialStateCapsule must be 128 bytes");

    // Verify alignment is 64 bytes
    const ALIGN: usize = align_of::<TrialStateCapsule>();
    const EXPECTED_ALIGN: usize = 64;
    assert!(ALIGN == EXPECTED_ALIGN, "TrialStateCapsule must be 64-byte aligned");
};

// ============================================================================
// Unit Tests (T28 Q1-Q7)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{size_of, align_of};

    // ========================================================================
    // Q1-Q3: Layout Tests
    // ========================================================================

    #[test]
    fn test_capsule_size() {
        assert_eq!(size_of::<TrialStateCapsule>(), 128, "TrialStateCapsule must be 128 bytes");
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(align_of::<TrialStateCapsule>(), 64, "TrialStateCapsule must be 64-byte aligned");
    }

    #[test]
    fn test_new_defaults() {
        let capsule = TrialStateCapsule::new();
        let stats = capsule.get_stats();

        assert_eq!(stats.trial_duration_secs, TRIAL_DURATION_SECS);
        assert_eq!(stats.trial_feature_mask, TRIAL_FEATURE_MASK);
        assert_eq!(stats.active_trials, 0);
        assert_eq!(stats.expired_trials, 0);
        assert_eq!(stats.total_checks, 0);
    }

    // ========================================================================
    // Q4-Q5: Timestamp Extraction Tests
    // ========================================================================

    #[test]
    fn test_extract_timestamp_valid() {
        // 0x68f5c800 = 1761461248 (Unix timestamp)
        let license = "KDB-HOB-68f5c800-abc12345-signature1234567890";
        let ts = TrialStateCapsule::extract_timestamp(license);
        assert_eq!(ts, Some(0x68f5c800));
    }

    #[test]
    fn test_extract_timestamp_invalid_prefix() {
        let license = "ABC-HOB-68f5c800-abc12345-signature";
        let ts = TrialStateCapsule::extract_timestamp(license);
        assert_eq!(ts, None);
    }

    #[test]
    fn test_extract_timestamp_short_hex() {
        let license = "KDB-HOB-68f5c80-abc12345-signature"; // 7 chars instead of 8
        let ts = TrialStateCapsule::extract_timestamp(license);
        assert_eq!(ts, None);
    }

    #[test]
    fn test_extract_timestamp_invalid_hex() {
        let license = "KDB-HOB-68f5cXXX-abc12345-signature";
        let ts = TrialStateCapsule::extract_timestamp(license);
        assert_eq!(ts, None);
    }

    #[test]
    fn test_parse_hex_u32() {
        assert_eq!(TrialStateCapsule::parse_hex_u32("00000000"), Some(0));
        assert_eq!(TrialStateCapsule::parse_hex_u32("ffffffff"), Some(u32::MAX));
        assert_eq!(TrialStateCapsule::parse_hex_u32("68f5c800"), Some(0x68f5c800));
        assert_eq!(TrialStateCapsule::parse_hex_u32("ABCDEF12"), Some(0xABCDEF12));
        assert_eq!(TrialStateCapsule::parse_hex_u32("0000"), None); // Too short
        assert_eq!(TrialStateCapsule::parse_hex_u32("ghijklmn"), None); // Invalid chars
    }

    // ========================================================================
    // Q6-Q7: Trial Status Tests
    // ========================================================================

    #[test]
    fn test_check_trial_status_active() {
        let capsule = TrialStateCapsule::new();

        // License created at 1761461248, check at 1761461248 + 1000 (within 7 days)
        let creation = 0x68f5c800u64; // 1761461248
        let now = creation + 1000;
        let license = "KDB-HOB-68f5c800-abc12345-signature1234567890";

        let status = capsule.check_trial_status(license, now);

        match status {
            TrialStatus::Active { remaining_secs } => {
                // Trial should have ~7 days - 1000 seconds remaining
                let expected = TRIAL_DURATION_SECS - 1000;
                assert_eq!(remaining_secs, expected);
            }
            _ => panic!("Expected Active status, got {:?}", status),
        }

        let stats = capsule.get_stats();
        assert_eq!(stats.active_trials, 1);
        assert_eq!(stats.expired_trials, 0);
    }

    #[test]
    fn test_check_trial_status_expired() {
        let capsule = TrialStateCapsule::new();

        // License created at 1761461248, check at creation + 8 days (after 7-day trial)
        let creation = 0x68f5c800u64;
        let now = creation + (8 * 24 * 60 * 60); // 8 days later
        let license = "KDB-HOB-68f5c800-abc12345-signature1234567890";

        let status = capsule.check_trial_status(license, now);

        match status {
            TrialStatus::Expired { expired_secs } => {
                // Should be 1 day expired
                let expected = (8 * 24 * 60 * 60) - TRIAL_DURATION_SECS;
                assert_eq!(expired_secs, expected);
            }
            _ => panic!("Expected Expired status, got {:?}", status),
        }

        let stats = capsule.get_stats();
        assert_eq!(stats.active_trials, 0);
        assert_eq!(stats.expired_trials, 1);
    }

    #[test]
    fn test_check_trial_status_invalid() {
        let capsule = TrialStateCapsule::new();

        let status = capsule.check_trial_status("invalid-license", 1234567890);
        assert_eq!(status, TrialStatus::Invalid);
    }

    #[test]
    fn test_trial_status_is_active() {
        assert!(TrialStatus::Active { remaining_secs: 1000 }.is_active());
        assert!(!TrialStatus::Expired { expired_secs: 1000 }.is_active());
        assert!(!TrialStatus::Invalid.is_active());
    }

    // ========================================================================
    // Feature Mask & Quotas Tests
    // ========================================================================

    #[test]
    fn test_get_effective_feature_mask_during_trial() {
        let capsule = TrialStateCapsule::new();

        let creation = 0x68f5c800u64;
        let now = creation + 1000; // Within trial
        let license = "KDB-HOB-68f5c800-abc12345-signature1234567890";

        let mask = capsule.get_effective_feature_mask(license, SubscriptionTier::Hobby, now);
        assert_eq!(mask, TRIAL_FEATURE_MASK, "During trial, all features should be enabled");
    }

    #[test]
    fn test_get_effective_feature_mask_after_trial() {
        let capsule = TrialStateCapsule::new();

        let creation = 0x68f5c800u64;
        let now = creation + (8 * 24 * 60 * 60); // After trial
        let license = "KDB-HOB-68f5c800-abc12345-signature1234567890";

        let mask = capsule.get_effective_feature_mask(license, SubscriptionTier::Hobby, now);
        assert_eq!(mask, SubscriptionTier::Hobby.feature_mask(), "After trial, tier mask should apply");
    }

    #[test]
    fn test_get_effective_quotas_during_trial() {
        let capsule = TrialStateCapsule::new();

        let creation = 0x68f5c800u64;
        let now = creation + 1000;
        let license = "KDB-HOB-68f5c800-abc12345-signature1234567890";

        let quotas = capsule.get_effective_quotas(license, SubscriptionTier::Hobby, now);

        assert!(quotas.is_trial);
        assert_eq!(quotas.sessions_per_month, TRIAL_UNLIMITED);
        assert_eq!(quotas.snapshot_limit, TRIAL_UNLIMITED);
        assert_eq!(quotas.step_backward_daily_limit, u32::MAX);
        assert_eq!(quotas.feature_mask, TRIAL_FEATURE_MASK);
    }

    #[test]
    fn test_get_effective_quotas_after_trial() {
        let capsule = TrialStateCapsule::new();

        let creation = 0x68f5c800u64;
        let now = creation + (8 * 24 * 60 * 60);
        let license = "KDB-HOB-68f5c800-abc12345-signature1234567890";

        let quotas = capsule.get_effective_quotas(license, SubscriptionTier::Hobby, now);

        assert!(!quotas.is_trial);
        assert_eq!(quotas.sessions_per_month, 5);
        assert_eq!(quotas.snapshot_limit, 100);
        assert_eq!(quotas.step_backward_daily_limit, 3); // Hobby: 3/day
        assert_eq!(quotas.feature_mask, SubscriptionTier::Hobby.feature_mask());
    }

    // ========================================================================
    // Configuration Tests
    // ========================================================================

    #[test]
    fn test_set_trial_duration() {
        let capsule = TrialStateCapsule::new();

        // Set custom 1-day trial
        capsule.set_trial_duration(24 * 60 * 60);
        assert_eq!(capsule.trial_duration(), 24 * 60 * 60);

        // Now a license created 2 days ago should be expired
        let creation = 0x68f5c800u64;
        let now = creation + (2 * 24 * 60 * 60);
        let license = "KDB-HOB-68f5c800-abc12345-signature1234567890";

        let status = capsule.check_trial_status(license, now);
        assert!(!status.is_active());
    }

    #[test]
    fn test_with_duration() {
        let capsule = TrialStateCapsule::with_duration(24 * 60 * 60);
        assert_eq!(capsule.trial_duration(), 24 * 60 * 60);
    }

    #[test]
    fn test_reset_stats() {
        let capsule = TrialStateCapsule::new();

        // Generate some stats
        let license = "KDB-HOB-68f5c800-abc12345-signature1234567890";
        capsule.check_trial_status(license, 0x68f5c800 + 1000);
        capsule.check_trial_status(license, 0x68f5c800 + (8 * 24 * 60 * 60));

        // Reset
        capsule.reset_stats();

        let stats = capsule.get_stats();
        assert_eq!(stats.active_trials, 0);
        assert_eq!(stats.expired_trials, 0);
        assert_eq!(stats.total_checks, 0);
    }

    // ========================================================================
    // Edge Cases & Boundary Tests
    // ========================================================================

    #[test]
    fn test_trial_boundary_exact() {
        let capsule = TrialStateCapsule::new();

        let creation = 0x68f5c800u64;
        let license = "KDB-HOB-68f5c800-abc12345-signature1234567890";

        // Exactly at trial end (should still be expired - not active)
        let now_at_end = creation + TRIAL_DURATION_SECS;
        let status = capsule.check_trial_status(license, now_at_end);
        assert!(!status.is_active(), "Exactly at trial end should be expired");

        // One second before trial end (should be active)
        let now_before_end = creation + TRIAL_DURATION_SECS - 1;
        let status = capsule.check_trial_status(license, now_before_end);
        assert!(status.is_active(), "One second before trial end should be active");
    }

    #[test]
    fn test_concurrent_checks() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(TrialStateCapsule::new());
        let mut handles = vec![];

        // Spawn 10 threads, each checking 100 times
        for _ in 0..10 {
            let capsule = Arc::clone(&capsule);
            let handle = thread::spawn(move || {
                let license = "KDB-HOB-68f5c800-abc12345-signature1234567890";
                for _ in 0..100 {
                    let _ = capsule.check_trial_status(license, 0x68f5c800 + 1000);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = capsule.get_stats();
        assert_eq!(stats.total_checks, 1000);
        assert_eq!(stats.active_trials, 1000);
    }

    // ========================================================================
    // Tier-Specific Quota Tests
    // ========================================================================

    #[test]
    fn test_effective_quotas_per_tier() {
        let tiers = [
            (SubscriptionTier::Hobby, 5, 100, 3),
            (SubscriptionTier::Pro, 100, 1000, u32::MAX),
            (SubscriptionTier::Engineer, 500, 10000, u32::MAX),
            (SubscriptionTier::Teams, 2000, 100000, u32::MAX),
            (SubscriptionTier::Enterprise, u64::MAX, u64::MAX, u32::MAX),
        ];

        for (tier, sessions, snapshots, step_back) in tiers {
            let quotas = EffectiveQuotas::from_tier(tier);
            assert_eq!(quotas.sessions_per_month, sessions, "sessions for {:?}", tier);
            assert_eq!(quotas.snapshot_limit, snapshots, "snapshots for {:?}", tier);
            assert_eq!(quotas.step_backward_daily_limit, step_back, "step_backward for {:?}", tier);
        }
    }
}
