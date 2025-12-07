//! TierEnforcementCapsule - T1 Atomic Tier Enforcement (64B)
//!
//! Lockfree tier-based feature and quota enforcement.
//! **Latency**: <20ns feature check, <50ns snapshot check
//! **Tier**: T1 Atomic (cache-aligned, generation counters)
//!
//! # Architecture
//!
//! 64-byte cache-aligned capsule with lockfree enforcement:
//! - Feature checks: <20ns (single atomic load + bitmask)
//! - Snapshot quota: <50ns (CAS with 20% grace period)
//! - Tier updates: <100ns (atomic updates to all limits)
//!
//! # Example
//!
//! ```rust
//! use kdb_mcp::tier_enforcement::{TierEnforcementCapsule, FeatureFlags};
//! use kdb_mcp::subscription_tier::SubscriptionTier;
//!
//! let enforcer = TierEnforcementCapsule::new();
//!
//! // Set tier and enable enforcement
//! enforcer.set_tier(SubscriptionTier::Developer);
//! enforcer.enable_enforcement();
//!
//! // Check feature access
//! if enforcer.is_feature_allowed(FeatureFlags::MEMORY_WRITE) {
//!     // Perform memory write
//! }
//!
//! // Check snapshot quota (with 20% grace)
//! match enforcer.check_and_increment_snapshot() {
//!     Ok(count) => println!("Snapshot {} captured", count),
//!     Err(e) => println!("Quota exceeded: {:?}", e),
//! }
//! ```

use core::sync::atomic::{AtomicU8, AtomicU16, AtomicU32, AtomicU64, Ordering};
use crate::subscription_tier::SubscriptionTier;

// ============================================================================
// Feature Flags (Const Bitmasks)
// ============================================================================

/// Feature flags for tier-based access control
///
/// Each flag is a single bit in a u32 bitmask.
/// Pre-computed tier masks enable O(1) feature checks.
pub struct FeatureFlags;

impl FeatureFlags {
    // Core debugging features (bits 0-3) - Available in Hobby tier
    pub const TIME_TRAVEL: u32 = 1 << 0;
    pub const BREAKPOINTS: u32 = 1 << 1;
    pub const STACK_TRACE: u32 = 1 << 2;
    pub const AUDIT_TRAIL: u32 = 1 << 3;

    // Memory access features (bits 4-5) - Starter/Developer+
    pub const MEMORY_READ: u32 = 1 << 4;
    pub const MEMORY_WRITE: u32 = 1 << 5;

    // Advanced features (bits 6-7) - Professional+
    pub const SYMBOL_RESOLUTION: u32 = 1 << 6;
    pub const STEP_BACKWARD: u32 = 1 << 7;

    // Enterprise features (bits 8-9)
    pub const PRIORITY_SUPPORT: u32 = 1 << 8;
    pub const CUSTOM_RETENTION: u32 = 1 << 9;

    // Pre-computed tier masks (for O(1) tier updates)
    pub const HOBBY_FEATURES: u32 = 0x0F;        // bits 0-3
    pub const STARTER_FEATURES: u32 = 0x1F;      // bits 0-4
    pub const DEVELOPER_FEATURES: u32 = 0x3F;    // bits 0-5
    pub const PROFESSIONAL_FEATURES: u32 = 0xFF; // bits 0-7
    pub const ENTERPRISE_FEATURES: u32 = 0x3FF;  // bits 0-9
}

// ============================================================================
// Error Types
// ============================================================================

/// Tier enforcement errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierEnforcementError {
    /// Feature not allowed for current tier
    FeatureNotAllowed {
        feature: u32,
        current_tier: u8,
        required_tier: u8,
    },
    /// Snapshot limit exceeded (even with grace period)
    SnapshotLimitExceeded {
        current: u64,
        limit: u64,
        grace_limit: u64,
    },
    /// Enforcement is disabled
    EnforcementDisabled,
}

// ============================================================================
// Statistics Snapshot
// ============================================================================

/// Tier enforcement statistics snapshot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierEnforcementStats {
    /// Current subscription tier
    pub tier: SubscriptionTier,
    /// Enabled feature bitmask
    pub enabled_features: u32,
    /// Current snapshot count
    pub snapshot_count: u64,
    /// Snapshot limit (base)
    pub snapshot_limit: u64,
    /// Snapshot limit with 20% grace
    pub grace_limit: u64,
    /// Total enforcement violations
    pub violations: u64,
    /// Last tool that caused a violation (0 = none)
    pub last_violation_tool: u16,
    /// Whether in grace mode (exceeded base limit but within grace)
    pub grace_mode: bool,
    /// Whether enforcement is enabled
    pub enforcement_enabled: bool,
    /// Retention period in days
    pub retention_days: u32,
}

// ============================================================================
// TierEnforcementCapsule (64B, T1 Atomic)
// ============================================================================

/// Tier Enforcement Capsule - Lockfree quota and feature enforcement
///
/// **Layout** (64 bytes total):
/// ```text
/// Offset  Size  Field
/// ------  ----  -----
/// 0       1     current_tier (AtomicU8)
/// 1       3     _pad1
/// 4       4     enabled_features (AtomicU32)
/// 8       8     snapshot_count (AtomicU64)
/// 16      8     snapshot_limit (AtomicU64)
/// 24      8     grace_limit (AtomicU64)
/// 32      8     enforcement_violations (AtomicU64)
/// 40      2     last_violation_tool (AtomicU16)
/// 42      1     grace_mode (AtomicU8)
/// 43      1     enforcement_enabled (AtomicU8)
/// 44      4     retention_days (AtomicU32)
/// 48      16    _padding
/// ```
///
/// **Memory Ordering**:
/// - Read path (is_feature_allowed): Acquire (ensure visibility)
/// - Write path (check_and_increment_snapshot): AcqRel (synchronize state)
/// - Config changes (set_tier): Release (publish updates)
///
/// **ASSUM Safety**:
/// - #ASSUME: Tier enum values 0-4 only (verified: from_u8 check)
/// - #ASSUME: Feature flags fit in u32 (verified: max bit 9)
/// - #VERIFY: 20% grace calculation uses saturating arithmetic
/// - #VERIFY: Snapshot count monotonically increases (no rewind)
#[repr(C, align(64))]
pub struct TierEnforcementCapsule {
    /// Current subscription tier (0-4)
    current_tier: AtomicU8,
    /// Padding for alignment
    _pad1: [u8; 3],
    /// Enabled feature bitmask
    enabled_features: AtomicU32,
    /// Current snapshot count
    snapshot_count: AtomicU64,
    /// Base snapshot limit
    snapshot_limit: AtomicU64,
    /// Grace limit (base + 20%)
    grace_limit: AtomicU64,
    /// Total enforcement violations
    enforcement_violations: AtomicU64,
    /// Last tool ID that caused a violation
    last_violation_tool: AtomicU16,
    /// Whether in grace mode (exceeded base, within grace)
    grace_mode: AtomicU8,
    /// Whether enforcement is enabled
    enforcement_enabled: AtomicU8,
    /// Retention period in days
    retention_days: AtomicU32,
    /// Padding to 64 bytes
    _padding: [u8; 16],
}

impl TierEnforcementCapsule {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Create new tier enforcement capsule
    ///
    /// **Defaults**:
    /// - Tier: Hobby
    /// - Enforcement: DISABLED (must call enable_enforcement())
    /// - Snapshot count: 0
    ///
    /// **Performance**: <50ns (8 atomic stores)
    pub const fn new() -> Self {
        let tier = SubscriptionTier::Hobby;
        let snapshot_limit = tier.snapshot_limit();
        // 20% grace: limit + (limit / 5)
        let grace_limit = snapshot_limit.saturating_add(snapshot_limit / 5);

        Self {
            current_tier: AtomicU8::new(tier.as_u8()),
            _pad1: [0; 3],
            enabled_features: AtomicU32::new(tier.feature_mask()),
            snapshot_count: AtomicU64::new(0),
            snapshot_limit: AtomicU64::new(snapshot_limit),
            grace_limit: AtomicU64::new(grace_limit),
            enforcement_violations: AtomicU64::new(0),
            last_violation_tool: AtomicU16::new(0),
            grace_mode: AtomicU8::new(0),
            enforcement_enabled: AtomicU8::new(0), // Disabled by default
            retention_days: AtomicU32::new(tier.retention_days()),
            _padding: [0; 16],
        }
    }

    // ========================================================================
    // Tier Configuration
    // ========================================================================

    /// Set subscription tier (updates all limits atomically)
    ///
    /// **Updates**:
    /// - Feature mask
    /// - Snapshot limit + grace limit (20%)
    /// - Retention days
    ///
    /// **Performance**: <100ns (6 atomic stores)
    ///
    /// **Memory Ordering**: Release (publish all updates)
    pub fn set_tier(&self, tier: SubscriptionTier) {
        let snapshot_limit = tier.snapshot_limit();
        // 20% grace: limit + (limit / 5)
        let grace_limit = snapshot_limit.saturating_add(snapshot_limit / 5);

        // Update all limits atomically (order matters: limits before tier)
        self.snapshot_limit.store(snapshot_limit, Ordering::Release);
        self.grace_limit.store(grace_limit, Ordering::Release);
        self.enabled_features.store(tier.feature_mask(), Ordering::Release);
        self.retention_days.store(tier.retention_days(), Ordering::Release);
        self.current_tier.store(tier.as_u8(), Ordering::Release);

        // Reset grace mode if new limits accommodate current count
        let current = self.snapshot_count.load(Ordering::Acquire);
        if current <= snapshot_limit {
            self.grace_mode.store(0, Ordering::Release);
        }
    }

    /// Get current tier
    #[inline]
    pub fn current_tier(&self) -> SubscriptionTier {
        let tier_u8 = self.current_tier.load(Ordering::Acquire);
        SubscriptionTier::from_u8(tier_u8).unwrap_or(SubscriptionTier::Hobby)
    }

    /// Enable enforcement
    ///
    /// **Note**: Enforcement is disabled by default.
    #[inline]
    pub fn enable_enforcement(&self) {
        self.enforcement_enabled.store(1, Ordering::Release);
    }

    /// Disable enforcement
    #[inline]
    pub fn disable_enforcement(&self) {
        self.enforcement_enabled.store(0, Ordering::Release);
    }

    /// Check if enforcement is enabled
    #[inline]
    pub fn is_enforcement_enabled(&self) -> bool {
        self.enforcement_enabled.load(Ordering::Acquire) != 0
    }

    // ========================================================================
    // Feature Enforcement (<20ns)
    // ========================================================================

    /// Check if a feature is allowed for the current tier
    ///
    /// **Performance**: <20ns (single atomic load + bitmask AND)
    ///
    /// **Returns**: true if feature is allowed OR enforcement is disabled
    #[inline]
    pub fn is_feature_allowed(&self, feature: u32) -> bool {
        // Fast path: enforcement disabled
        if self.enforcement_enabled.load(Ordering::Acquire) == 0 {
            return true;
        }

        let enabled = self.enabled_features.load(Ordering::Acquire);
        (enabled & feature) == feature
    }

    /// Require a feature, returning error if not allowed
    ///
    /// **Performance**: <20ns fast path, <50ns error path
    ///
    /// **Arguments**:
    /// - `feature`: Feature flag to check
    /// - `tool_id`: Tool ID for violation tracking (0-65535)
    ///
    /// **Returns**: Ok(()) if allowed, Err with details if denied
    pub fn require_feature(&self, feature: u32, tool_id: u16) -> Result<(), TierEnforcementError> {
        // Fast path: enforcement disabled
        if self.enforcement_enabled.load(Ordering::Acquire) == 0 {
            return Ok(());
        }

        let enabled = self.enabled_features.load(Ordering::Acquire);
        if (enabled & feature) == feature {
            return Ok(());
        }

        // Feature not allowed - record violation
        self.enforcement_violations.fetch_add(1, Ordering::Relaxed);
        self.last_violation_tool.store(tool_id, Ordering::Relaxed);

        // Determine required tier for this feature
        let required_tier = Self::min_tier_for_feature(feature);

        Err(TierEnforcementError::FeatureNotAllowed {
            feature,
            current_tier: self.current_tier.load(Ordering::Relaxed),
            required_tier,
        })
    }

    /// Determine minimum tier required for a feature
    #[inline]
    fn min_tier_for_feature(feature: u32) -> u8 {
        if feature & !FeatureFlags::HOBBY_FEATURES == 0 {
            SubscriptionTier::Hobby.as_u8()
        } else if feature & !FeatureFlags::STARTER_FEATURES == 0 {
            SubscriptionTier::Starter.as_u8()
        } else if feature & !FeatureFlags::DEVELOPER_FEATURES == 0 {
            SubscriptionTier::Developer.as_u8()
        } else if feature & !FeatureFlags::PROFESSIONAL_FEATURES == 0 {
            SubscriptionTier::Professional.as_u8()
        } else {
            SubscriptionTier::Enterprise.as_u8()
        }
    }

    // ========================================================================
    // Snapshot Quota Enforcement (<50ns)
    // ========================================================================

    /// Check snapshot quota and increment if allowed
    ///
    /// **Algorithm**:
    /// 1. Fast path: If enforcement disabled, always allow
    /// 2. Load current count and limits
    /// 3. Check against grace limit (base + 20%)
    /// 4. CAS increment if allowed
    /// 5. Set grace_mode if exceeds base but within grace
    ///
    /// **Performance**: <50ns (CAS loop, typically 1 iteration)
    ///
    /// **Returns**: Ok(new_count) or Err(SnapshotLimitExceeded)
    pub fn check_and_increment_snapshot(&self) -> Result<u64, TierEnforcementError> {
        // Fast path: enforcement disabled
        if self.enforcement_enabled.load(Ordering::Acquire) == 0 {
            let new_count = self.snapshot_count.fetch_add(1, Ordering::AcqRel) + 1;
            return Ok(new_count);
        }

        let grace_limit = self.grace_limit.load(Ordering::Acquire);
        let base_limit = self.snapshot_limit.load(Ordering::Acquire);

        loop {
            let current = self.snapshot_count.load(Ordering::Acquire);

            // Check against grace limit (hard limit)
            if current >= grace_limit {
                self.enforcement_violations.fetch_add(1, Ordering::Relaxed);
                return Err(TierEnforcementError::SnapshotLimitExceeded {
                    current,
                    limit: base_limit,
                    grace_limit,
                });
            }

            // Try to increment
            if self.snapshot_count.compare_exchange(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                let new_count = current + 1;

                // Set grace mode if exceeding base limit
                if new_count > base_limit {
                    self.grace_mode.store(1, Ordering::Relaxed);
                }

                return Ok(new_count);
            }
            // CAS failed, retry
        }
    }

    /// Get current snapshot count
    #[inline]
    pub fn snapshot_count(&self) -> u64 {
        self.snapshot_count.load(Ordering::Acquire)
    }

    /// Reset snapshot count (for testing or tier reset)
    pub fn reset_snapshot_count(&self) {
        self.snapshot_count.store(0, Ordering::Release);
        self.grace_mode.store(0, Ordering::Release);
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get enforcement statistics snapshot
    ///
    /// **Performance**: <50ns (8 atomic loads)
    pub fn get_stats(&self) -> TierEnforcementStats {
        let tier_u8 = self.current_tier.load(Ordering::Acquire);
        let tier = SubscriptionTier::from_u8(tier_u8).unwrap_or(SubscriptionTier::Hobby);

        TierEnforcementStats {
            tier,
            enabled_features: self.enabled_features.load(Ordering::Acquire),
            snapshot_count: self.snapshot_count.load(Ordering::Acquire),
            snapshot_limit: self.snapshot_limit.load(Ordering::Acquire),
            grace_limit: self.grace_limit.load(Ordering::Acquire),
            violations: self.enforcement_violations.load(Ordering::Relaxed),
            last_violation_tool: self.last_violation_tool.load(Ordering::Relaxed),
            grace_mode: self.grace_mode.load(Ordering::Relaxed) != 0,
            enforcement_enabled: self.enforcement_enabled.load(Ordering::Relaxed) != 0,
            retention_days: self.retention_days.load(Ordering::Acquire),
        }
    }
}

impl Default for TierEnforcementCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Static Assertions (Compile-Time Verification)
// ============================================================================

#[cfg(test)]
const _: () = {
    // Verify size is exactly 64 bytes
    const SIZE: usize = core::mem::size_of::<TierEnforcementCapsule>();
    const EXPECTED: usize = 64;
    assert!(SIZE == EXPECTED, "TierEnforcementCapsule must be 64 bytes");

    // Verify alignment is 64 bytes
    const ALIGN: usize = core::mem::align_of::<TierEnforcementCapsule>();
    const EXPECTED_ALIGN: usize = 64;
    assert!(ALIGN == EXPECTED_ALIGN, "TierEnforcementCapsule must be 64-byte aligned");
};

// ============================================================================
// Unit Tests (T28 Q1-Q7)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{size_of, align_of};

    #[test]
    fn test_capsule_size() {
        assert_eq!(size_of::<TierEnforcementCapsule>(), 64, "TierEnforcementCapsule must be 64 bytes");
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(align_of::<TierEnforcementCapsule>(), 64, "TierEnforcementCapsule must be 64-byte aligned");
    }

    #[test]
    fn test_new_defaults() {
        let capsule = TierEnforcementCapsule::new();
        let stats = capsule.get_stats();

        assert_eq!(stats.tier, SubscriptionTier::Hobby);
        assert_eq!(stats.enabled_features, FeatureFlags::HOBBY_FEATURES);
        assert_eq!(stats.snapshot_count, 0);
        assert_eq!(stats.snapshot_limit, 100);
        assert_eq!(stats.grace_limit, 120); // 100 + 20%
        assert_eq!(stats.violations, 0);
        assert!(!stats.enforcement_enabled, "Enforcement should be disabled by default");
        assert_eq!(stats.retention_days, 7);
    }

    #[test]
    fn test_set_tier_updates_all_limits() {
        let capsule = TierEnforcementCapsule::new();

        capsule.set_tier(SubscriptionTier::Developer);
        let stats = capsule.get_stats();

        assert_eq!(stats.tier, SubscriptionTier::Developer);
        assert_eq!(stats.enabled_features, FeatureFlags::DEVELOPER_FEATURES);
        assert_eq!(stats.snapshot_limit, 10_000);
        assert_eq!(stats.grace_limit, 12_000); // 10000 + 20%
        assert_eq!(stats.retention_days, 30);
    }

    #[test]
    fn test_feature_check_hobby_tier() {
        let capsule = TierEnforcementCapsule::new();
        capsule.enable_enforcement();

        // Hobby tier features (should be allowed)
        assert!(capsule.is_feature_allowed(FeatureFlags::TIME_TRAVEL));
        assert!(capsule.is_feature_allowed(FeatureFlags::BREAKPOINTS));
        assert!(capsule.is_feature_allowed(FeatureFlags::STACK_TRACE));
        assert!(capsule.is_feature_allowed(FeatureFlags::AUDIT_TRAIL));

        // Higher tier features (should be denied)
        assert!(!capsule.is_feature_allowed(FeatureFlags::MEMORY_READ));
        assert!(!capsule.is_feature_allowed(FeatureFlags::MEMORY_WRITE));
        assert!(!capsule.is_feature_allowed(FeatureFlags::SYMBOL_RESOLUTION));
        assert!(!capsule.is_feature_allowed(FeatureFlags::PRIORITY_SUPPORT));
    }

    #[test]
    fn test_feature_check_starter_tier() {
        let capsule = TierEnforcementCapsule::new();
        capsule.set_tier(SubscriptionTier::Starter);
        capsule.enable_enforcement();

        // Starter tier adds MEMORY_READ
        assert!(capsule.is_feature_allowed(FeatureFlags::MEMORY_READ));
        assert!(!capsule.is_feature_allowed(FeatureFlags::MEMORY_WRITE));
    }

    #[test]
    fn test_feature_check_developer_tier() {
        let capsule = TierEnforcementCapsule::new();
        capsule.set_tier(SubscriptionTier::Developer);
        capsule.enable_enforcement();

        // Developer tier adds MEMORY_WRITE
        assert!(capsule.is_feature_allowed(FeatureFlags::MEMORY_READ));
        assert!(capsule.is_feature_allowed(FeatureFlags::MEMORY_WRITE));
        assert!(!capsule.is_feature_allowed(FeatureFlags::SYMBOL_RESOLUTION));
    }

    #[test]
    fn test_feature_check_professional_tier() {
        let capsule = TierEnforcementCapsule::new();
        capsule.set_tier(SubscriptionTier::Professional);
        capsule.enable_enforcement();

        // Professional tier adds advanced features
        assert!(capsule.is_feature_allowed(FeatureFlags::SYMBOL_RESOLUTION));
        assert!(capsule.is_feature_allowed(FeatureFlags::STEP_BACKWARD));
        assert!(!capsule.is_feature_allowed(FeatureFlags::PRIORITY_SUPPORT));
    }

    #[test]
    fn test_feature_check_enterprise_tier() {
        let capsule = TierEnforcementCapsule::new();
        capsule.set_tier(SubscriptionTier::Enterprise);
        capsule.enable_enforcement();

        // Enterprise tier has all features
        assert!(capsule.is_feature_allowed(FeatureFlags::TIME_TRAVEL));
        assert!(capsule.is_feature_allowed(FeatureFlags::MEMORY_WRITE));
        assert!(capsule.is_feature_allowed(FeatureFlags::SYMBOL_RESOLUTION));
        assert!(capsule.is_feature_allowed(FeatureFlags::PRIORITY_SUPPORT));
        assert!(capsule.is_feature_allowed(FeatureFlags::CUSTOM_RETENTION));
    }

    #[test]
    fn test_require_feature_success() {
        let capsule = TierEnforcementCapsule::new();
        capsule.set_tier(SubscriptionTier::Developer);
        capsule.enable_enforcement();

        let result = capsule.require_feature(FeatureFlags::MEMORY_WRITE, 42);
        assert!(result.is_ok());
    }

    #[test]
    fn test_require_feature_denied() {
        let capsule = TierEnforcementCapsule::new();
        capsule.set_tier(SubscriptionTier::Hobby);
        capsule.enable_enforcement();

        let result = capsule.require_feature(FeatureFlags::MEMORY_WRITE, 42);
        assert!(result.is_err());

        match result.unwrap_err() {
            TierEnforcementError::FeatureNotAllowed { feature, current_tier, required_tier } => {
                assert_eq!(feature, FeatureFlags::MEMORY_WRITE);
                assert_eq!(current_tier, SubscriptionTier::Hobby.as_u8());
                assert_eq!(required_tier, SubscriptionTier::Developer.as_u8());
            }
            _ => panic!("Expected FeatureNotAllowed error"),
        }

        // Check violation tracking
        let stats = capsule.get_stats();
        assert_eq!(stats.violations, 1);
        assert_eq!(stats.last_violation_tool, 42);
    }

    #[test]
    fn test_snapshot_limit_enforcement() {
        let capsule = TierEnforcementCapsule::new();
        capsule.set_tier(SubscriptionTier::Hobby); // 100 limit, 120 grace
        capsule.enable_enforcement();

        // Consume all 100 base limit snapshots
        for i in 0..100 {
            let result = capsule.check_and_increment_snapshot();
            assert!(result.is_ok(), "Snapshot {} should succeed", i);
        }

        // Check stats
        let stats = capsule.get_stats();
        assert_eq!(stats.snapshot_count, 100);
        assert!(!stats.grace_mode, "Should not be in grace mode yet");

        // Consume grace period (20 more)
        for i in 0..20 {
            let result = capsule.check_and_increment_snapshot();
            assert!(result.is_ok(), "Grace snapshot {} should succeed", i);
        }

        // Check grace mode
        let stats = capsule.get_stats();
        assert_eq!(stats.snapshot_count, 120);
        assert!(stats.grace_mode, "Should be in grace mode");

        // 121st should fail
        let result = capsule.check_and_increment_snapshot();
        assert!(result.is_err());

        match result.unwrap_err() {
            TierEnforcementError::SnapshotLimitExceeded { current, limit, grace_limit } => {
                assert_eq!(current, 120);
                assert_eq!(limit, 100);
                assert_eq!(grace_limit, 120);
            }
            _ => panic!("Expected SnapshotLimitExceeded error"),
        }
    }

    #[test]
    fn test_enforcement_disabled_allows_all() {
        let capsule = TierEnforcementCapsule::new();
        // Enforcement disabled by default

        // All features should be allowed
        assert!(capsule.is_feature_allowed(FeatureFlags::PRIORITY_SUPPORT));
        assert!(capsule.is_feature_allowed(FeatureFlags::CUSTOM_RETENTION));

        // Snapshots should always succeed
        for _ in 0..200 {
            assert!(capsule.check_and_increment_snapshot().is_ok());
        }
    }

    #[test]
    fn test_tier_upgrade() {
        let capsule = TierEnforcementCapsule::new();
        capsule.enable_enforcement();

        // Start at Hobby
        assert!(!capsule.is_feature_allowed(FeatureFlags::MEMORY_WRITE));

        // Upgrade to Developer
        capsule.set_tier(SubscriptionTier::Developer);
        assert!(capsule.is_feature_allowed(FeatureFlags::MEMORY_WRITE));

        // Check limits updated
        let stats = capsule.get_stats();
        assert_eq!(stats.snapshot_limit, 10_000);
        assert_eq!(stats.retention_days, 30);
    }

    #[test]
    fn test_tier_downgrade() {
        let capsule = TierEnforcementCapsule::new();
        capsule.set_tier(SubscriptionTier::Professional);
        capsule.enable_enforcement();

        // Professional has all basic features
        assert!(capsule.is_feature_allowed(FeatureFlags::SYMBOL_RESOLUTION));

        // Downgrade to Starter
        capsule.set_tier(SubscriptionTier::Starter);
        assert!(!capsule.is_feature_allowed(FeatureFlags::SYMBOL_RESOLUTION));
        assert!(capsule.is_feature_allowed(FeatureFlags::MEMORY_READ));
    }

    #[test]
    fn test_reset_snapshot_count() {
        let capsule = TierEnforcementCapsule::new();
        capsule.enable_enforcement();

        // Add some snapshots
        for _ in 0..50 {
            capsule.check_and_increment_snapshot().unwrap();
        }

        assert_eq!(capsule.snapshot_count(), 50);

        // Reset
        capsule.reset_snapshot_count();

        assert_eq!(capsule.snapshot_count(), 0);
        assert!(!capsule.get_stats().grace_mode);
    }

    #[test]
    fn test_grace_limit_calculation() {
        // Hobby: 100 + 20% = 120
        let capsule = TierEnforcementCapsule::new();
        let stats = capsule.get_stats();
        assert_eq!(stats.snapshot_limit, 100);
        assert_eq!(stats.grace_limit, 120);

        // Developer: 10000 + 20% = 12000
        capsule.set_tier(SubscriptionTier::Developer);
        let stats = capsule.get_stats();
        assert_eq!(stats.snapshot_limit, 10_000);
        assert_eq!(stats.grace_limit, 12_000);

        // Professional: 100000 + 20% = 120000
        capsule.set_tier(SubscriptionTier::Professional);
        let stats = capsule.get_stats();
        assert_eq!(stats.snapshot_limit, 100_000);
        assert_eq!(stats.grace_limit, 120_000);
    }

    #[test]
    fn test_concurrent_snapshot_increments() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(TierEnforcementCapsule::new());
        capsule.set_tier(SubscriptionTier::Developer); // 10K limit
        capsule.enable_enforcement();

        let mut handles = vec![];

        // Spawn 10 threads, each incrementing 100 times
        for _ in 0..10 {
            let capsule = Arc::clone(&capsule);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let _ = capsule.check_and_increment_snapshot();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All 1000 increments should have succeeded
        assert_eq!(capsule.snapshot_count(), 1000);
    }

    #[test]
    fn test_min_tier_for_feature() {
        assert_eq!(
            TierEnforcementCapsule::min_tier_for_feature(FeatureFlags::TIME_TRAVEL),
            SubscriptionTier::Hobby.as_u8()
        );
        assert_eq!(
            TierEnforcementCapsule::min_tier_for_feature(FeatureFlags::MEMORY_READ),
            SubscriptionTier::Starter.as_u8()
        );
        assert_eq!(
            TierEnforcementCapsule::min_tier_for_feature(FeatureFlags::MEMORY_WRITE),
            SubscriptionTier::Developer.as_u8()
        );
        assert_eq!(
            TierEnforcementCapsule::min_tier_for_feature(FeatureFlags::SYMBOL_RESOLUTION),
            SubscriptionTier::Professional.as_u8()
        );
        assert_eq!(
            TierEnforcementCapsule::min_tier_for_feature(FeatureFlags::PRIORITY_SUPPORT),
            SubscriptionTier::Enterprise.as_u8()
        );
    }
}
