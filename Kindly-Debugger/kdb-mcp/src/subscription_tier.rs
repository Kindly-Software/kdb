//! SubscriptionTier - Tier-based subscription model for kdb-mcp
//!
//! **Tier**: T0 (Compile-time verified enums)
//! **Performance**: 0ns - All methods are const
//! **Architecture**: #[repr(u8)] for atomic storage compatibility
//!
//! # Subscription Tiers (Updated Tier Matrix 2025-12)
//!
//! | Tier | Price | RPM | Burst | Snapshots | Retention | Sessions/Mo | Concurrent | Features |
//! |------|-------|-----|-------|-----------|-----------|-------------|------------|----------|
//! | Hobby | $0 | 60 | 10 | 100 | 7 days | 5 | 2 | 0x0F |
//! | Pro | $19 | 300 | 30 | 1,000 | 7 days | 100 | 5 | 0x3F |
//! | Engineer | $49 | 1,000 | 100 | 10,000 | 30 days | 500 | 10 | 0x3FF |
//! | Teams | $129 | 5,000 | 500 | 100,000 | 90 days | 2,000 | 50 | 0x3FF |
//! | Enterprise | $999+ | MAX | MAX | MAX | MAX | MAX | MAX | 0x1FFF |
//!
//! # Feature Flags (Updated Tier Matrix)
//!
//! | Bit | Flag | Tiers |
//! |-----|------|-------|
//! | 0-3 | Core (TIME_TRAVEL, BREAKPOINTS, STACK_TRACE, AUDIT_TRAIL) | ALL |
//! | 4-5 | Memory Read, Basic Replay | Pro+ |
//! | 6-9 | Full Replay, Historical Memory, LSH Bugs, Symbols | Engineer+ |
//! | 10-12 | Priority Support, Custom Retention, Dedicated Infra | Enterprise |
//!
//! # Example
//!
//! ```rust
//! use kdb_mcp::subscription_tier::{SubscriptionTier, MemoryReplayLevel};
//!
//! let tier = SubscriptionTier::Engineer;
//! assert_eq!(tier.requests_per_minute(), 1000);
//! assert_eq!(tier.retention_days(), 30);
//! assert_eq!(tier.max_concurrent_sessions(), 10);
//! assert_eq!(tier.sessions_per_month(), 500);
//! assert_eq!(tier.name(), "Engineer");
//! assert_eq!(tier.memory_replay_level(), MemoryReplayLevel::Full);
//! assert!(tier.can_read_memory_at_snapshot());
//! assert!(tier.can_find_similar_bugs());
//! ```

/// Memory replay access level for tier-based feature gating
///
/// **Layout**: Simple enum for memory replay feature access
///
/// # Levels
///
/// - `None`: No memory replay access (Hobby tier)
/// - `Basic`: Basic memory replay (Pro tier)
/// - `Full`: Full memory replay with all features (Engineer+ tiers)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryReplayLevel {
    /// No memory replay access
    None,
    /// Basic memory replay (limited features)
    Basic,
    /// Full memory replay with all features
    Full,
}

/// Subscription tier levels for kdb-mcp
///
/// **Layout**: #[repr(u8)] for atomic storage in TierEnforcementCapsule
///
/// **Ordering**: 0 (lowest) to 4 (highest) for comparison operations
///
/// # Pricing (as of 2025)
///
/// | Tier | Price | Sessions/Mo |
/// |------|-------|-------------|
/// | Hobby | $0 | 5 |
/// | Pro | $19 | 100 |
/// | Engineer | $49 | 500 |
/// | Teams | $129 | 2,000 |
/// | Enterprise | $999+ | Unlimited |
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubscriptionTier {
    /// Free tier ($0): 60 RPM, 10 burst, 100 snapshots, 7 days retention, 5 sessions/mo
    Hobby = 0,
    /// Pro tier ($19): 300 RPM, 30 burst, 1K snapshots, 7 days retention, 100 sessions/mo
    Pro = 1,
    /// Engineer tier ($49): 1K RPM, 100 burst, 10K snapshots, 30 days retention, 500 sessions/mo
    Engineer = 2,
    /// Teams tier ($129): 5K RPM, 500 burst, 100K snapshots, 90 days retention, 2000 sessions/mo
    Teams = 3,
    /// Enterprise tier ($999+): Unlimited everything
    Enterprise = 4,
}

impl SubscriptionTier {
    // ========================================================================
    // Const Conversions
    // ========================================================================

    /// Convert from u8 (for atomic storage)
    ///
    /// Returns None for invalid values (>= 5)
    ///
    /// # Backward Compatibility
    ///
    /// u8 values are preserved from previous tier names:
    /// - 0: Hobby (unchanged)
    /// - 1: Pro (was Starter)
    /// - 2: Engineer (was Developer)
    /// - 3: Teams (was Professional)
    /// - 4: Enterprise (unchanged)
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Hobby),
            1 => Some(Self::Pro),
            2 => Some(Self::Engineer),
            3 => Some(Self::Teams),
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
            Self::Pro => 300,
            Self::Engineer => 1_000,
            Self::Teams => 5_000,
            Self::Enterprise => u32::MAX,
        }
    }

    /// Burst limit (max concurrent requests)
    #[inline]
    pub const fn burst_limit(self) -> u32 {
        match self {
            Self::Hobby => 10,
            Self::Pro => 30,
            Self::Engineer => 100,
            Self::Teams => 500,
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
            Self::Pro => 10,        // 10/sec
            Self::Engineer => 100,  // 100/sec
            Self::Teams => 1_000,   // 1K/sec
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
            Self::Pro => 1_000,
            Self::Engineer => 10_000,
            Self::Teams => 100_000,
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
            Self::Pro => 7,
            Self::Engineer => 30,
            Self::Teams => 90,
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
    /// - Bit 5: MEMORY_REPLAY_BASIC
    /// - Bit 6: MEMORY_REPLAY_FULL
    /// - Bit 7: READ_MEMORY_AT_SNAPSHOT
    /// - Bit 8: FIND_SIMILAR_BUGS
    /// - Bit 9: SYMBOL_RESOLUTION
    /// - Bit 10: PRIORITY_SUPPORT
    /// - Bit 11: CUSTOM_RETENTION
    /// - Bit 12: DEDICATED_INFRA
    #[inline]
    pub const fn feature_mask(self) -> u32 {
        match self {
            // Hobby: TIME_TRAVEL, BREAKPOINTS, STACK_TRACE, AUDIT_TRAIL (bits 0-3)
            Self::Hobby => 0x0F,
            // Pro: + MEMORY_READ, MEMORY_REPLAY_BASIC (bits 4-5)
            Self::Pro => 0x3F,
            // Engineer: + MEMORY_REPLAY_FULL, READ_MEMORY_AT_SNAPSHOT, FIND_SIMILAR_BUGS, SYMBOL_RESOLUTION (bits 6-9)
            Self::Engineer => 0x3FF,
            // Teams: Same as Engineer (team sharing handled separately)
            Self::Teams => 0x3FF,
            // Enterprise: + PRIORITY_SUPPORT, CUSTOM_RETENTION, DEDICATED_INFRA (bits 10-12)
            Self::Enterprise => 0x1FFF,
        }
    }

    // ========================================================================
    // Session Limits (Const Methods)
    // ========================================================================

    /// Maximum concurrent debugging sessions
    ///
    /// Hobby: 2 concurrent sessions (free tier limitation)
    /// Pro: 5 concurrent sessions
    /// Engineer: 10 concurrent sessions
    /// Teams: 50 concurrent sessions
    /// Enterprise: Unlimited
    #[inline]
    pub const fn max_concurrent_sessions(self) -> u64 {
        match self {
            Self::Hobby => 2,
            Self::Pro => 5,
            Self::Engineer => 10,
            Self::Teams => 50,
            Self::Enterprise => u64::MAX,
        }
    }

    /// Sessions per month limit (after promotional period)
    ///
    /// Hobby: 5 sessions/month ($0)
    /// Pro: 100 sessions/month ($19)
    /// Engineer: 500 sessions/month ($49)
    /// Teams: 2000 sessions/month ($129)
    /// Enterprise: Unlimited ($999+)
    #[inline]
    pub const fn sessions_per_month(self) -> u64 {
        match self {
            Self::Hobby => 5,           // Free tier limit after promo
            Self::Pro => 100,           // $19/mo
            Self::Engineer => 500,      // $49/mo
            Self::Teams => 2_000,       // $129/mo
            Self::Enterprise => u64::MAX, // $999+/mo
        }
    }

    /// Sessions per month during promotional period (first week)
    ///
    /// Hobby: Unlimited during promo week
    /// Other tiers: Same as regular limits
    #[inline]
    pub const fn promo_sessions_per_month(self) -> u64 {
        match self {
            Self::Hobby => u64::MAX,   // Unlimited during 1-week promo
            _ => self.sessions_per_month(),
        }
    }

    // ========================================================================
    // Time-Travel & Memory Replay Features (Const Methods)
    // ========================================================================

    /// Daily limit for step_backward operations
    ///
    /// Hobby: 3 step_backward operations per day
    /// All other tiers: Unlimited (u32::MAX)
    ///
    /// This prevents abuse of time-travel debugging on the free tier
    /// while allowing full access for paid tiers.
    #[inline]
    pub const fn step_backward_daily_limit(self) -> u32 {
        match self {
            Self::Hobby => 3,
            _ => u32::MAX,
        }
    }

    /// Memory replay access level
    ///
    /// Hobby: None - No memory replay access
    /// Pro: Basic - Basic memory replay features
    /// Engineer+: Full - Full memory replay with all features
    ///
    /// Memory replay enables historical memory state inspection
    /// for time-travel debugging.
    #[inline]
    pub const fn memory_replay_level(self) -> MemoryReplayLevel {
        match self {
            Self::Hobby => MemoryReplayLevel::None,
            Self::Pro => MemoryReplayLevel::Basic,
            Self::Engineer | Self::Teams | Self::Enterprise => MemoryReplayLevel::Full,
        }
    }

    /// Can read memory at historical snapshots
    ///
    /// Only Engineer+ tiers can read memory at specific snapshots.
    /// This is a premium time-travel debugging feature.
    #[inline]
    pub const fn can_read_memory_at_snapshot(self) -> bool {
        matches!(self, Self::Engineer | Self::Teams | Self::Enterprise)
    }

    /// Can use the find_similar_bugs T10 probabilistic search
    ///
    /// Only Engineer+ tiers can use LSH-based similar bug detection.
    /// This is a premium debugging feature using T10 Probabilistic capsules.
    #[inline]
    pub const fn can_find_similar_bugs(self) -> bool {
        matches!(self, Self::Engineer | Self::Teams | Self::Enterprise)
    }

    // ========================================================================
    // Display
    // ========================================================================

    /// Human-readable tier name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Hobby => "Hobby",
            Self::Pro => "Pro",
            Self::Engineer => "Engineer",
            Self::Teams => "Teams",
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
        // Backward compatibility: u8 values must remain the same
        assert_eq!(SubscriptionTier::Hobby.as_u8(), 0);
        assert_eq!(SubscriptionTier::Pro.as_u8(), 1);        // was Starter
        assert_eq!(SubscriptionTier::Engineer.as_u8(), 2);   // was Developer
        assert_eq!(SubscriptionTier::Teams.as_u8(), 3);      // was Professional
        assert_eq!(SubscriptionTier::Enterprise.as_u8(), 4);
    }

    #[test]
    fn test_tier_from_u8() {
        assert_eq!(SubscriptionTier::from_u8(0), Some(SubscriptionTier::Hobby));
        assert_eq!(SubscriptionTier::from_u8(1), Some(SubscriptionTier::Pro));
        assert_eq!(SubscriptionTier::from_u8(2), Some(SubscriptionTier::Engineer));
        assert_eq!(SubscriptionTier::from_u8(3), Some(SubscriptionTier::Teams));
        assert_eq!(SubscriptionTier::from_u8(4), Some(SubscriptionTier::Enterprise));
        assert_eq!(SubscriptionTier::from_u8(5), None);
        assert_eq!(SubscriptionTier::from_u8(255), None);
    }

    #[test]
    fn test_requests_per_minute() {
        assert_eq!(SubscriptionTier::Hobby.requests_per_minute(), 60);
        assert_eq!(SubscriptionTier::Pro.requests_per_minute(), 300);
        assert_eq!(SubscriptionTier::Engineer.requests_per_minute(), 1_000);
        assert_eq!(SubscriptionTier::Teams.requests_per_minute(), 5_000);
        assert_eq!(SubscriptionTier::Enterprise.requests_per_minute(), u32::MAX);
    }

    #[test]
    fn test_burst_limit() {
        assert_eq!(SubscriptionTier::Hobby.burst_limit(), 10);
        assert_eq!(SubscriptionTier::Pro.burst_limit(), 30);
        assert_eq!(SubscriptionTier::Engineer.burst_limit(), 100);
        assert_eq!(SubscriptionTier::Teams.burst_limit(), 500);
        assert_eq!(SubscriptionTier::Enterprise.burst_limit(), u32::MAX);
    }

    #[test]
    fn test_snapshot_limit() {
        assert_eq!(SubscriptionTier::Hobby.snapshot_limit(), 100);
        assert_eq!(SubscriptionTier::Pro.snapshot_limit(), 1_000);
        assert_eq!(SubscriptionTier::Engineer.snapshot_limit(), 10_000);
        assert_eq!(SubscriptionTier::Teams.snapshot_limit(), 100_000);
        assert_eq!(SubscriptionTier::Enterprise.snapshot_limit(), u64::MAX);
    }

    #[test]
    fn test_retention_days() {
        assert_eq!(SubscriptionTier::Hobby.retention_days(), 7);
        assert_eq!(SubscriptionTier::Pro.retention_days(), 7);
        assert_eq!(SubscriptionTier::Engineer.retention_days(), 30);
        assert_eq!(SubscriptionTier::Teams.retention_days(), 90);
        assert_eq!(SubscriptionTier::Enterprise.retention_days(), u32::MAX);
    }

    #[test]
    fn test_feature_mask() {
        // Updated tier matrix (2025-12)
        assert_eq!(SubscriptionTier::Hobby.feature_mask(), 0x0F);      // bits 0-3
        assert_eq!(SubscriptionTier::Pro.feature_mask(), 0x3F);        // bits 0-5
        assert_eq!(SubscriptionTier::Engineer.feature_mask(), 0x3FF);  // bits 0-9
        assert_eq!(SubscriptionTier::Teams.feature_mask(), 0x3FF);     // bits 0-9 (same as Engineer)
        assert_eq!(SubscriptionTier::Enterprise.feature_mask(), 0x1FFF); // bits 0-12
    }

    #[test]
    fn test_tier_names() {
        assert_eq!(SubscriptionTier::Hobby.name(), "Hobby");
        assert_eq!(SubscriptionTier::Pro.name(), "Pro");
        assert_eq!(SubscriptionTier::Engineer.name(), "Engineer");
        assert_eq!(SubscriptionTier::Teams.name(), "Teams");
        assert_eq!(SubscriptionTier::Enterprise.name(), "Enterprise");
    }

    #[test]
    fn test_tier_ordering() {
        assert!(SubscriptionTier::Hobby < SubscriptionTier::Pro);
        assert!(SubscriptionTier::Pro < SubscriptionTier::Engineer);
        assert!(SubscriptionTier::Engineer < SubscriptionTier::Teams);
        assert!(SubscriptionTier::Teams < SubscriptionTier::Enterprise);
    }

    #[test]
    fn test_default_tier() {
        assert_eq!(SubscriptionTier::default(), SubscriptionTier::Hobby);
    }

    // ========================================================================
    // Session Limit Tests
    // ========================================================================

    #[test]
    fn test_max_concurrent_sessions() {
        assert_eq!(SubscriptionTier::Hobby.max_concurrent_sessions(), 2);
        assert_eq!(SubscriptionTier::Pro.max_concurrent_sessions(), 5);
        assert_eq!(SubscriptionTier::Engineer.max_concurrent_sessions(), 10);
        assert_eq!(SubscriptionTier::Teams.max_concurrent_sessions(), 50);
        assert_eq!(SubscriptionTier::Enterprise.max_concurrent_sessions(), u64::MAX);
    }

    #[test]
    fn test_sessions_per_month() {
        // Updated pricing: Hobby $0, Pro $19, Engineer $49, Teams $129, Enterprise $999+
        assert_eq!(SubscriptionTier::Hobby.sessions_per_month(), 5);
        assert_eq!(SubscriptionTier::Pro.sessions_per_month(), 100);
        assert_eq!(SubscriptionTier::Engineer.sessions_per_month(), 500);
        assert_eq!(SubscriptionTier::Teams.sessions_per_month(), 2_000);
        assert_eq!(SubscriptionTier::Enterprise.sessions_per_month(), u64::MAX);
    }

    #[test]
    fn test_promo_sessions_per_month() {
        // Hobby gets unlimited during promo
        assert_eq!(SubscriptionTier::Hobby.promo_sessions_per_month(), u64::MAX);
        // Other tiers get same as regular limits
        assert_eq!(SubscriptionTier::Pro.promo_sessions_per_month(), 100);
        assert_eq!(SubscriptionTier::Engineer.promo_sessions_per_month(), 500);
        assert_eq!(SubscriptionTier::Teams.promo_sessions_per_month(), 2_000);
        assert_eq!(SubscriptionTier::Enterprise.promo_sessions_per_month(), u64::MAX);
    }

    #[test]
    fn test_hobby_promo_is_unlimited() {
        // During promo week, Hobby tier should be unlimited
        let hobby = SubscriptionTier::Hobby;
        assert_eq!(hobby.promo_sessions_per_month(), u64::MAX);
        // After promo, Hobby tier is limited to 5 sessions/month
        assert_eq!(hobby.sessions_per_month(), 5);
    }

    #[test]
    fn test_concurrent_sessions_scale_with_tier() {
        // Verify concurrent sessions scale appropriately with tier
        let tiers = [
            SubscriptionTier::Hobby,
            SubscriptionTier::Pro,
            SubscriptionTier::Engineer,
            SubscriptionTier::Teams,
            SubscriptionTier::Enterprise,
        ];

        for i in 0..tiers.len() - 1 {
            assert!(
                tiers[i].max_concurrent_sessions() < tiers[i + 1].max_concurrent_sessions(),
                "Tier {:?} should have fewer concurrent sessions than {:?}",
                tiers[i],
                tiers[i + 1]
            );
        }
    }

    #[test]
    fn test_sessions_per_month_scale_with_tier() {
        // Verify monthly sessions scale appropriately with tier
        let tiers = [
            SubscriptionTier::Hobby,
            SubscriptionTier::Pro,
            SubscriptionTier::Engineer,
            SubscriptionTier::Teams,
            SubscriptionTier::Enterprise,
        ];

        for i in 0..tiers.len() - 1 {
            assert!(
                tiers[i].sessions_per_month() < tiers[i + 1].sessions_per_month(),
                "Tier {:?} should have fewer monthly sessions than {:?}",
                tiers[i],
                tiers[i + 1]
            );
        }
    }

    // ========================================================================
    // Time-Travel & Memory Replay Feature Tests
    // ========================================================================

    #[test]
    fn test_step_backward_daily_limit() {
        // Hobby tier has limited step_backward operations
        assert_eq!(SubscriptionTier::Hobby.step_backward_daily_limit(), 3);
        // All paid tiers have unlimited
        assert_eq!(SubscriptionTier::Pro.step_backward_daily_limit(), u32::MAX);
        assert_eq!(SubscriptionTier::Engineer.step_backward_daily_limit(), u32::MAX);
        assert_eq!(SubscriptionTier::Teams.step_backward_daily_limit(), u32::MAX);
        assert_eq!(SubscriptionTier::Enterprise.step_backward_daily_limit(), u32::MAX);
    }

    #[test]
    fn test_memory_replay_level() {
        // Hobby: None
        assert_eq!(SubscriptionTier::Hobby.memory_replay_level(), MemoryReplayLevel::None);
        // Pro: Basic
        assert_eq!(SubscriptionTier::Pro.memory_replay_level(), MemoryReplayLevel::Basic);
        // Engineer+: Full
        assert_eq!(SubscriptionTier::Engineer.memory_replay_level(), MemoryReplayLevel::Full);
        assert_eq!(SubscriptionTier::Teams.memory_replay_level(), MemoryReplayLevel::Full);
        assert_eq!(SubscriptionTier::Enterprise.memory_replay_level(), MemoryReplayLevel::Full);
    }

    #[test]
    fn test_can_read_memory_at_snapshot() {
        // Hobby and Pro cannot read memory at snapshots
        assert!(!SubscriptionTier::Hobby.can_read_memory_at_snapshot());
        assert!(!SubscriptionTier::Pro.can_read_memory_at_snapshot());
        // Engineer+ can read memory at snapshots
        assert!(SubscriptionTier::Engineer.can_read_memory_at_snapshot());
        assert!(SubscriptionTier::Teams.can_read_memory_at_snapshot());
        assert!(SubscriptionTier::Enterprise.can_read_memory_at_snapshot());
    }

    #[test]
    fn test_can_find_similar_bugs() {
        // Hobby and Pro cannot use T10 probabilistic bug search
        assert!(!SubscriptionTier::Hobby.can_find_similar_bugs());
        assert!(!SubscriptionTier::Pro.can_find_similar_bugs());
        // Engineer+ can use similar bug detection
        assert!(SubscriptionTier::Engineer.can_find_similar_bugs());
        assert!(SubscriptionTier::Teams.can_find_similar_bugs());
        assert!(SubscriptionTier::Enterprise.can_find_similar_bugs());
    }

    #[test]
    fn test_memory_replay_level_enum() {
        // Test MemoryReplayLevel enum properties
        assert_ne!(MemoryReplayLevel::None, MemoryReplayLevel::Basic);
        assert_ne!(MemoryReplayLevel::Basic, MemoryReplayLevel::Full);
        assert_ne!(MemoryReplayLevel::None, MemoryReplayLevel::Full);

        // Test Debug trait
        let none = MemoryReplayLevel::None;
        let basic = MemoryReplayLevel::Basic;
        let full = MemoryReplayLevel::Full;
        assert_eq!(format!("{:?}", none), "None");
        assert_eq!(format!("{:?}", basic), "Basic");
        assert_eq!(format!("{:?}", full), "Full");

        // Test Clone
        let cloned = full;
        assert_eq!(cloned, MemoryReplayLevel::Full);
    }

    // ========================================================================
    // Backward Compatibility Tests
    // ========================================================================

    #[test]
    fn test_u8_backward_compatibility() {
        // CRITICAL: u8 values must remain unchanged for atomic storage compatibility
        // These values are stored in TierEnforcementCapsule and must never change

        // Hobby was always 0
        assert_eq!(SubscriptionTier::from_u8(0).unwrap().name(), "Hobby");

        // Pro (was Starter) must still be 1
        assert_eq!(SubscriptionTier::from_u8(1).unwrap().name(), "Pro");

        // Engineer (was Developer) must still be 2
        assert_eq!(SubscriptionTier::from_u8(2).unwrap().name(), "Engineer");

        // Teams (was Professional) must still be 3
        assert_eq!(SubscriptionTier::from_u8(3).unwrap().name(), "Teams");

        // Enterprise was always 4
        assert_eq!(SubscriptionTier::from_u8(4).unwrap().name(), "Enterprise");
    }

    #[test]
    fn test_repr_u8_layout() {
        // Verify #[repr(u8)] is working correctly
        assert_eq!(std::mem::size_of::<SubscriptionTier>(), 1);

        // Round-trip test for all tiers
        for tier_u8 in 0u8..=4 {
            let tier = SubscriptionTier::from_u8(tier_u8).unwrap();
            assert_eq!(tier.as_u8(), tier_u8);
        }
    }
}
