//! SubscriptionTier - Tier-based subscription model for kdb-mcp
//!
//! **Tier**: T0 (Compile-time verified enums)
//! **Performance**: 0ns - All methods are const
//! **Architecture**: #[repr(u8)] for atomic storage compatibility
//!
//! # Subscription Tiers
//!
//! | Tier | RPM | Burst | Snapshots | Retention | Features |
//! |------|-----|-------|-----------|-----------|----------|
//! | Hobby | 60 | 10 | 100 | 7 days | 0x0F |
//! | Starter | 300 | 30 | 1,000 | 7 days | 0x1F |
//! | Developer | 1,000 | 100 | 10,000 | 30 days | 0x3F |
//! | Professional | 5,000 | 500 | 100,000 | 90 days | 0xFF |
//! | Enterprise | MAX | MAX | MAX | MAX | 0x3FF |
//!
//! # Example
//!
//! ```rust
//! use kdb_mcp::subscription_tier::SubscriptionTier;
//!
//! let tier = SubscriptionTier::Developer;
//! assert_eq!(tier.requests_per_minute(), 1000);
//! assert_eq!(tier.retention_days(), 30);
//! assert_eq!(tier.name(), "Developer");
//! ```

/// Subscription tier levels for kdb-mcp
///
/// **Layout**: #[repr(u8)] for atomic storage in TierEnforcementCapsule
///
/// **Ordering**: 0 (lowest) to 4 (highest) for comparison operations
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubscriptionTier {
    /// Free tier: 60 RPM, 10 burst, 100 snapshots, 7 days retention
    Hobby = 0,
    /// Entry tier: 300 RPM, 30 burst, 1K snapshots, 7 days retention
    Starter = 1,
    /// Developer tier: 1K RPM, 100 burst, 10K snapshots, 30 days retention
    Developer = 2,
    /// Professional tier: 5K RPM, 500 burst, 100K snapshots, 90 days retention
    Professional = 3,
    /// Enterprise tier: Unlimited everything
    Enterprise = 4,
}

impl SubscriptionTier {
    // ========================================================================
    // Const Conversions
    // ========================================================================

    /// Convert from u8 (for atomic storage)
    ///
    /// Returns None for invalid values (>= 5)
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Hobby),
            1 => Some(Self::Starter),
            2 => Some(Self::Developer),
            3 => Some(Self::Professional),
            4 => Some(Self::Enterprise),
            _ => None,
        }
    }

    /// Convert to u8 (for atomic storage)
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    // ========================================================================
    // Tier Limits (Const Methods)
    // ========================================================================

    /// Requests per minute limit
    ///
    /// Enterprise returns u32::MAX for "unlimited"
    #[inline]
    pub const fn requests_per_minute(self) -> u32 {
        match self {
            Self::Hobby => 60,
            Self::Starter => 300,
            Self::Developer => 1_000,
            Self::Professional => 5_000,
            Self::Enterprise => u32::MAX,
        }
    }

    /// Burst limit (max concurrent requests)
    #[inline]
    pub const fn burst_limit(self) -> u32 {
        match self {
            Self::Hobby => 10,
            Self::Starter => 30,
            Self::Developer => 100,
            Self::Professional => 500,
            Self::Enterprise => u32::MAX,
        }
    }

    /// Snapshot rate limit (snapshots per second)
    ///
    /// Rate at which new snapshots can be captured
    #[inline]
    pub const fn snapshot_rate(self) -> u32 {
        match self {
            Self::Hobby => 1,       // 1/sec
            Self::Starter => 10,    // 10/sec
            Self::Developer => 100, // 100/sec
            Self::Professional => 1_000, // 1K/sec
            Self::Enterprise => u32::MAX,
        }
    }

    /// Maximum snapshot count (total stored)
    ///
    /// Enterprise returns u64::MAX for "unlimited"
    #[inline]
    pub const fn snapshot_limit(self) -> u64 {
        match self {
            Self::Hobby => 100,
            Self::Starter => 1_000,
            Self::Developer => 10_000,
            Self::Professional => 100_000,
            Self::Enterprise => u64::MAX,
        }
    }

    /// Retention period in days
    ///
    /// Enterprise returns u32::MAX for "unlimited"
    #[inline]
    pub const fn retention_days(self) -> u32 {
        match self {
            Self::Hobby => 7,
            Self::Starter => 7,
            Self::Developer => 30,
            Self::Professional => 90,
            Self::Enterprise => u32::MAX,
        }
    }

    /// Feature bitmask (see FeatureFlags in tier_enforcement.rs)
    ///
    /// Each bit represents an enabled feature:
    /// - Bit 0: TIME_TRAVEL
    /// - Bit 1: BREAKPOINTS
    /// - Bit 2: STACK_TRACE
    /// - Bit 3: AUDIT_TRAIL
    /// - Bit 4: MEMORY_READ
    /// - Bit 5: MEMORY_WRITE
    /// - Bit 6: SYMBOL_RESOLUTION
    /// - Bit 7: STEP_BACKWARD
    /// - Bit 8: PRIORITY_SUPPORT
    /// - Bit 9: CUSTOM_RETENTION
    #[inline]
    pub const fn feature_mask(self) -> u32 {
        match self {
            // Hobby: TIME_TRAVEL, BREAKPOINTS, STACK_TRACE, AUDIT_TRAIL (bits 0-3)
            Self::Hobby => 0x0F,
            // Starter: + MEMORY_READ (bit 4)
            Self::Starter => 0x1F,
            // Developer: + MEMORY_WRITE (bit 5)
            Self::Developer => 0x3F,
            // Professional: + SYMBOL_RESOLUTION, STEP_BACKWARD (bits 6-7)
            Self::Professional => 0xFF,
            // Enterprise: + PRIORITY_SUPPORT, CUSTOM_RETENTION (bits 8-9)
            Self::Enterprise => 0x3FF,
        }
    }

    // ========================================================================
    // Display
    // ========================================================================

    /// Human-readable tier name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Hobby => "Hobby",
            Self::Starter => "Starter",
            Self::Developer => "Developer",
            Self::Professional => "Professional",
            Self::Enterprise => "Enterprise",
        }
    }
}

impl Default for SubscriptionTier {
    fn default() -> Self {
        Self::Hobby
    }
}

// ============================================================================
// Unit Tests (T28 Q1-Q7)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_values() {
        assert_eq!(SubscriptionTier::Hobby.as_u8(), 0);
        assert_eq!(SubscriptionTier::Starter.as_u8(), 1);
        assert_eq!(SubscriptionTier::Developer.as_u8(), 2);
        assert_eq!(SubscriptionTier::Professional.as_u8(), 3);
        assert_eq!(SubscriptionTier::Enterprise.as_u8(), 4);
    }

    #[test]
    fn test_tier_from_u8() {
        assert_eq!(SubscriptionTier::from_u8(0), Some(SubscriptionTier::Hobby));
        assert_eq!(SubscriptionTier::from_u8(1), Some(SubscriptionTier::Starter));
        assert_eq!(SubscriptionTier::from_u8(2), Some(SubscriptionTier::Developer));
        assert_eq!(SubscriptionTier::from_u8(3), Some(SubscriptionTier::Professional));
        assert_eq!(SubscriptionTier::from_u8(4), Some(SubscriptionTier::Enterprise));
        assert_eq!(SubscriptionTier::from_u8(5), None);
        assert_eq!(SubscriptionTier::from_u8(255), None);
    }

    #[test]
    fn test_requests_per_minute() {
        assert_eq!(SubscriptionTier::Hobby.requests_per_minute(), 60);
        assert_eq!(SubscriptionTier::Starter.requests_per_minute(), 300);
        assert_eq!(SubscriptionTier::Developer.requests_per_minute(), 1_000);
        assert_eq!(SubscriptionTier::Professional.requests_per_minute(), 5_000);
        assert_eq!(SubscriptionTier::Enterprise.requests_per_minute(), u32::MAX);
    }

    #[test]
    fn test_burst_limit() {
        assert_eq!(SubscriptionTier::Hobby.burst_limit(), 10);
        assert_eq!(SubscriptionTier::Starter.burst_limit(), 30);
        assert_eq!(SubscriptionTier::Developer.burst_limit(), 100);
        assert_eq!(SubscriptionTier::Professional.burst_limit(), 500);
        assert_eq!(SubscriptionTier::Enterprise.burst_limit(), u32::MAX);
    }

    #[test]
    fn test_snapshot_limit() {
        assert_eq!(SubscriptionTier::Hobby.snapshot_limit(), 100);
        assert_eq!(SubscriptionTier::Starter.snapshot_limit(), 1_000);
        assert_eq!(SubscriptionTier::Developer.snapshot_limit(), 10_000);
        assert_eq!(SubscriptionTier::Professional.snapshot_limit(), 100_000);
        assert_eq!(SubscriptionTier::Enterprise.snapshot_limit(), u64::MAX);
    }

    #[test]
    fn test_retention_days() {
        assert_eq!(SubscriptionTier::Hobby.retention_days(), 7);
        assert_eq!(SubscriptionTier::Starter.retention_days(), 7);
        assert_eq!(SubscriptionTier::Developer.retention_days(), 30);
        assert_eq!(SubscriptionTier::Professional.retention_days(), 90);
        assert_eq!(SubscriptionTier::Enterprise.retention_days(), u32::MAX);
    }

    #[test]
    fn test_feature_mask() {
        assert_eq!(SubscriptionTier::Hobby.feature_mask(), 0x0F);
        assert_eq!(SubscriptionTier::Starter.feature_mask(), 0x1F);
        assert_eq!(SubscriptionTier::Developer.feature_mask(), 0x3F);
        assert_eq!(SubscriptionTier::Professional.feature_mask(), 0xFF);
        assert_eq!(SubscriptionTier::Enterprise.feature_mask(), 0x3FF);
    }

    #[test]
    fn test_tier_names() {
        assert_eq!(SubscriptionTier::Hobby.name(), "Hobby");
        assert_eq!(SubscriptionTier::Starter.name(), "Starter");
        assert_eq!(SubscriptionTier::Developer.name(), "Developer");
        assert_eq!(SubscriptionTier::Professional.name(), "Professional");
        assert_eq!(SubscriptionTier::Enterprise.name(), "Enterprise");
    }

    #[test]
    fn test_tier_ordering() {
        assert!(SubscriptionTier::Hobby < SubscriptionTier::Starter);
        assert!(SubscriptionTier::Starter < SubscriptionTier::Developer);
        assert!(SubscriptionTier::Developer < SubscriptionTier::Professional);
        assert!(SubscriptionTier::Professional < SubscriptionTier::Enterprise);
    }

    #[test]
    fn test_default_tier() {
        assert_eq!(SubscriptionTier::default(), SubscriptionTier::Hobby);
    }
}
