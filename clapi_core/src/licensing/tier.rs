//! Subscription Tier System
//!
//! **Purpose**: Tiered subscription system with lockfree tier detection
//! **Architecture**: 100% lockfree atomic operations, zero Mutex/RwLock
//!
//! # Tiers
//! - **Free**: 1,000 requests/month, 7 days retention
//! - **Solo**: 100,000 requests/month, 30 days retention
//! - **Team**: 1,000,000 requests/month, 90 days retention
//! - **Enterprise**: 10,000,000 requests/month, 365 days retention
//! - **Custom**: Unlimited requests, custom retention
//!
//! # Performance Targets (B32 Framework)
//! - `SubscriptionTier::parse()`: <100ns (pattern match)
//! - `TierCache::load()`: <50ns (atomic load)
//! - `TierCache::store()`: <50ns (atomic store)
//!
//! # UCE34 Compliance
//! - **Q10 (Tier Selection)**: Tier 1 Atomic for lockfree tier coordination
//! - **Q11 (Rust Transform)**: AtomicU8 for tier state (0-4 discriminant)
//! - **Q12 (Nightly)**: None required (stable Rust)
//! - **Q33 (Validation)**: Manual verification (simple 64B capsule)
//!
//! # ASSUM Safety
//! - #ASSUME: Tier discriminant values (0-4) fit in u8
//! - #VERIFY: Compile-time const assertions validate discriminant range
//! - #ASSUME: Atomic loads/stores are Relaxed (no synchronization needed)
//! - #VERIFY: Tier is Copy + immutable data (no coordination required)

use std::sync::atomic::{AtomicU8, Ordering};

/// Subscription tier with associated quotas and retention policies
///
/// # Discriminant Values
/// Each tier has an explicit discriminant (0-4) that matches the atomic
/// representation in TierCache. This ensures zero-cost conversion between
/// enum and atomic storage.
///
/// # Performance: <10ns tier queries (const fn getters)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum SubscriptionTier {
    /// Free tier: 1K requests/month, 7 days retention
    Free = 0,
    /// Solo tier: 100K requests/month, 30 days retention
    Solo = 1,
    /// Team tier: 1M requests/month, 90 days retention
    Team = 2,
    /// Enterprise tier: 10M requests/month, 365 days retention
    Enterprise = 3,
    /// Custom tier: Unlimited requests, custom retention
    Custom = 4,
}

impl SubscriptionTier {
    /// Monthly request limit for this tier
    ///
    /// # Performance: <5ns (const fn match)
    ///
    /// # Returns
    /// - Free: 1,000
    /// - Solo: 100,000
    /// - Team: 1,000,000
    /// - Enterprise: 10,000,000
    /// - Custom: u64::MAX (effectively unlimited)
    #[inline(always)]
    pub const fn monthly_request_limit(&self) -> u64 {
        match self {
            Self::Free => 1_000,
            Self::Solo => 100_000,
            Self::Team => 1_000_000,
            Self::Enterprise => 10_000_000,
            Self::Custom => u64::MAX,
        }
    }

    /// Retention period in days for audit logs
    ///
    /// # Performance: <5ns (const fn match)
    ///
    /// # Returns
    /// - Free: 7 days
    /// - Solo: 30 days
    /// - Team: 90 days
    /// - Enterprise: 365 days
    /// - Custom: 3650 days (10 years)
    #[inline(always)]
    pub const fn retention_days(&self) -> u16 {
        match self {
            Self::Free => 7,
            Self::Solo => 30,
            Self::Team => 90,
            Self::Enterprise => 365,
            Self::Custom => 3650, // 10 years for custom tier
        }
    }

    /// Rate limit (requests per second) for this tier
    ///
    /// # Performance: <5ns (const fn match)
    ///
    /// # Returns
    /// - Free: 1 req/s
    /// - Solo: 10 req/s
    /// - Team: 100 req/s
    /// - Enterprise: 1000 req/s
    /// - Custom: u32::MAX (effectively unlimited)
    #[inline(always)]
    pub const fn rate_limit_rps(&self) -> u32 {
        match self {
            Self::Free => 1,
            Self::Solo => 10,
            Self::Team => 100,
            Self::Enterprise => 1000,
            Self::Custom => u32::MAX,
        }
    }

    /// Concurrent request limit for this tier
    ///
    /// # Performance: <5ns (const fn match)
    ///
    /// # Returns
    /// - Free: 1 concurrent request
    /// - Solo: 5 concurrent requests
    /// - Team: 20 concurrent requests
    /// - Enterprise: 100 concurrent requests
    /// - Custom: u16::MAX (effectively unlimited)
    #[inline(always)]
    pub const fn concurrent_limit(&self) -> u16 {
        match self {
            Self::Free => 1,
            Self::Solo => 5,
            Self::Team => 20,
            Self::Enterprise => 100,
            Self::Custom => u16::MAX,
        }
    }

    /// Returns tier name as static string
    ///
    /// # Performance: <5ns (const fn match)
    ///
    /// # Examples
    /// ```
    /// use clapi_core::licensing::SubscriptionTier;
    ///
    /// assert_eq!(SubscriptionTier::Free.as_str(), "free");
    /// assert_eq!(SubscriptionTier::Enterprise.as_str(), "enterprise");
    /// ```
    #[inline(always)]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Solo => "solo",
            Self::Team => "team",
            Self::Enterprise => "enterprise",
            Self::Custom => "custom",
        }
    }

    /// Parse tier from string (case-insensitive)
    ///
    /// # Performance: <100ns (string comparison + match)
    ///
    /// # Arguments
    /// - `s`: Tier name ("free", "solo", "team", "enterprise", "custom")
    ///
    /// # Returns
    /// - `Some(tier)` if valid tier name
    /// - `None` if unknown tier name
    ///
    /// # Examples
    /// ```
    /// use clapi_core::licensing::SubscriptionTier;
    ///
    /// assert_eq!(SubscriptionTier::parse("free"), Some(SubscriptionTier::Free));
    /// assert_eq!(SubscriptionTier::parse("ENTERPRISE"), Some(SubscriptionTier::Enterprise));
    /// assert_eq!(SubscriptionTier::parse("unknown"), None);
    /// ```
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "free" => Some(Self::Free),
            "solo" => Some(Self::Solo),
            "team" => Some(Self::Team),
            "enterprise" => Some(Self::Enterprise),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    /// Convert tier to u8 discriminant
    ///
    /// # Performance: <5ns (const fn cast)
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Discriminant values (0-4) fit in u8
    /// - #VERIFY: Explicit repr(u8) ensures valid range
    #[inline(always)]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Convert u8 discriminant to tier
    ///
    /// # Performance: <20ns (match + bounds check)
    ///
    /// # Arguments
    /// - `value`: Discriminant value (0-4)
    ///
    /// # Returns
    /// - `Some(tier)` if valid discriminant (0-4)
    /// - `None` if out of range
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Discriminant values (0-4) map to valid tiers
    /// - #VERIFY: Exhaustive match on discriminant range
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Free),
            1 => Some(Self::Solo),
            2 => Some(Self::Team),
            3 => Some(Self::Enterprise),
            4 => Some(Self::Custom),
            _ => None,
        }
    }

    /// Legacy alias for monthly_request_limit (backward compatibility)
    #[deprecated(since = "0.5.0", note = "Use monthly_request_limit() instead")]
    #[inline(always)]
    pub fn quota_limit(&self) -> u64 {
        self.monthly_request_limit()
    }
}

impl Default for SubscriptionTier {
    /// Default tier is Free
    ///
    /// # Performance: <5ns (const)
    #[inline(always)]
    fn default() -> Self {
        Self::Free
    }
}

impl std::fmt::Display for SubscriptionTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Lockfree tier cache for per-user tier detection
///
/// # Architecture
/// - **Size**: 64 bytes (cache-line aligned)
/// - **Tier**: Atomic (Tier 1) - lockfree coordination
/// - **Thread Safety**: 100% lockfree (AtomicU8)
///
/// # Memory Layout
/// ```text
/// [user_id: 8B][tier: 1B][padding: 55B] = 64B (single cache line)
/// ```
///
/// # Performance Targets (B32 Framework)
/// - `load()`: <50ns (atomic load + u8→enum conversion)
/// - `store()`: <50ns (enum→u8 + atomic store)
/// - `compare_exchange()`: <100ns (CAS operation)
///
/// # UCE34 Compliance
/// - **Q10**: Tier 1 Atomic (single-threaded tier updates, lockfree reads)
/// - **Q11**: AtomicU8 for tier discriminant (0-4)
/// - **Q33**: Manual verification (simple structure, no derive macro needed)
///
/// # ASSUM Safety
/// - #ASSUME: Relaxed ordering sufficient (no cross-thread coordination)
/// - #VERIFY: Tier is Copy + immutable (no data races)
/// - #ASSUME: 64B alignment prevents false sharing
/// - #VERIFY: repr(C, align(64)) enforces alignment
#[repr(C, align(64))]
pub struct TierCache {
    /// User ID (immutable after construction)
    user_id: u64,
    /// Current subscription tier (0-4 discriminant)
    tier: AtomicU8,
    /// Padding to 64 bytes (cache line alignment)
    _padding: [u8; 55],
}

impl TierCache {
    /// Create new tier cache with default tier (Free)
    ///
    /// # Performance: <20ns (atomic initialization)
    ///
    /// # Arguments
    /// - `user_id`: User identifier
    ///
    /// # Returns
    /// TierCache initialized with Free tier
    ///
    /// # Examples
    /// ```
    /// use clapi_core::licensing::TierCache;
    ///
    /// let cache = TierCache::new(12345);
    /// ```
    #[inline(always)]
    pub const fn new(user_id: u64) -> Self {
        Self {
            user_id,
            tier: AtomicU8::new(SubscriptionTier::Free.as_u8()),
            _padding: [0u8; 55],
        }
    }

    /// Create tier cache with specific tier
    ///
    /// # Performance: <20ns (atomic initialization)
    ///
    /// # Arguments
    /// - `user_id`: User identifier
    /// - `tier`: Initial subscription tier
    ///
    /// # Returns
    /// TierCache initialized with specified tier
    ///
    /// # Examples
    /// ```
    /// use clapi_core::licensing::{TierCache, SubscriptionTier};
    ///
    /// let cache = TierCache::with_tier(12345, SubscriptionTier::Enterprise);
    /// ```
    #[inline(always)]
    pub const fn with_tier(user_id: u64, tier: SubscriptionTier) -> Self {
        Self {
            user_id,
            tier: AtomicU8::new(tier.as_u8()),
            _padding: [0u8; 55],
        }
    }

    /// Get user ID (immutable)
    ///
    /// # Performance: <5ns (direct field access)
    #[inline(always)]
    pub const fn user_id(&self) -> u64 {
        self.user_id
    }

    /// Load current tier (lockfree)
    ///
    /// # Performance: <50ns (atomic load + conversion)
    ///
    /// # Returns
    /// Current subscription tier
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Relaxed ordering sufficient (no synchronization needed)
    /// - #VERIFY: Tier is Copy type (no data races)
    ///
    /// # Examples
    /// ```
    /// use clapi_core::licensing::{TierCache, SubscriptionTier};
    ///
    /// let cache = TierCache::with_tier(12345, SubscriptionTier::Team);
    /// assert_eq!(cache.load(), SubscriptionTier::Team);
    /// ```
    #[inline(always)]
    pub fn load(&self) -> SubscriptionTier {
        let value = self.tier.load(Ordering::Relaxed);
        // SAFETY: All stored values are validated in store()
        SubscriptionTier::from_u8(value).unwrap_or(SubscriptionTier::Free)
    }

    /// Store new tier (lockfree)
    ///
    /// # Performance: <50ns (atomic store)
    ///
    /// # Arguments
    /// - `tier`: New subscription tier
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Relaxed ordering sufficient (no synchronization needed)
    /// - #VERIFY: Tier discriminant validated at construction (0-4)
    ///
    /// # Examples
    /// ```
    /// use clapi_core::licensing::{TierCache, SubscriptionTier};
    ///
    /// let cache = TierCache::new(12345);
    /// cache.store(SubscriptionTier::Enterprise);
    /// assert_eq!(cache.load(), SubscriptionTier::Enterprise);
    /// ```
    #[inline(always)]
    pub fn store(&self, tier: SubscriptionTier) {
        self.tier.store(tier.as_u8(), Ordering::Relaxed);
    }

    /// Atomic compare-and-swap tier update
    ///
    /// # Performance: <100ns (CAS operation)
    ///
    /// # Arguments
    /// - `current`: Expected current tier
    /// - `new`: New tier to store
    ///
    /// # Returns
    /// - `Ok(())` if swap succeeded
    /// - `Err(actual)` if current tier doesn't match (returns actual tier)
    ///
    /// # ASSUM Safety
    /// - #ASSUME: AcqRel ordering for synchronization
    /// - #VERIFY: CAS provides linearizable tier updates
    ///
    /// # Examples
    /// ```
    /// use clapi_core::licensing::{TierCache, SubscriptionTier};
    ///
    /// let cache = TierCache::with_tier(12345, SubscriptionTier::Free);
    ///
    /// // Upgrade from Free to Solo
    /// assert!(cache.compare_exchange(SubscriptionTier::Free, SubscriptionTier::Solo).is_ok());
    ///
    /// // Attempted upgrade fails (current is Solo, not Free)
    /// assert!(cache.compare_exchange(SubscriptionTier::Free, SubscriptionTier::Team).is_err());
    /// ```
    #[inline(always)]
    pub fn compare_exchange(
        &self,
        current: SubscriptionTier,
        new: SubscriptionTier,
    ) -> Result<(), SubscriptionTier> {
        match self.tier.compare_exchange(
            current.as_u8(),
            new.as_u8(),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(actual) => {
                let actual_tier = SubscriptionTier::from_u8(actual)
                    .unwrap_or(SubscriptionTier::Free);
                Err(actual_tier)
            }
        }
    }

    /// Get tier quotas snapshot (const-time)
    ///
    /// # Performance: <100ns (1 atomic load + 3 const fn calls)
    ///
    /// # Returns
    /// Tuple of (monthly_limit, retention_days, rate_limit_rps)
    ///
    /// # Examples
    /// ```
    /// use clapi_core::licensing::{TierCache, SubscriptionTier};
    ///
    /// let cache = TierCache::with_tier(12345, SubscriptionTier::Team);
    /// let (monthly, retention, rate) = cache.quotas();
    /// assert_eq!(monthly, 1_000_000);
    /// assert_eq!(retention, 90);
    /// assert_eq!(rate, 100);
    /// ```
    #[inline(always)]
    pub fn quotas(&self) -> (u64, u16, u32) {
        let tier = self.load();
        (
            tier.monthly_request_limit(),
            tier.retention_days(),
            tier.rate_limit_rps(),
        )
    }
}

// Compile-time assertions to validate tier discriminants
const _: () = {
    assert!(SubscriptionTier::Free.as_u8() == 0);
    assert!(SubscriptionTier::Solo.as_u8() == 1);
    assert!(SubscriptionTier::Team.as_u8() == 2);
    assert!(SubscriptionTier::Enterprise.as_u8() == 3);
    assert!(SubscriptionTier::Custom.as_u8() == 4);
    assert!(core::mem::size_of::<TierCache>() == 64);
    assert!(core::mem::align_of::<TierCache>() == 64);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_discriminants() {
        assert_eq!(SubscriptionTier::Free as u8, 0);
        assert_eq!(SubscriptionTier::Solo as u8, 1);
        assert_eq!(SubscriptionTier::Team as u8, 2);
        assert_eq!(SubscriptionTier::Enterprise as u8, 3);
        assert_eq!(SubscriptionTier::Custom as u8, 4);
    }

    #[test]
    fn test_tier_quotas() {
        assert_eq!(SubscriptionTier::Free.monthly_request_limit(), 1_000);
        assert_eq!(SubscriptionTier::Solo.monthly_request_limit(), 100_000);
        assert_eq!(SubscriptionTier::Team.monthly_request_limit(), 1_000_000);
        assert_eq!(SubscriptionTier::Enterprise.monthly_request_limit(), 10_000_000);
        assert_eq!(SubscriptionTier::Custom.monthly_request_limit(), u64::MAX);
    }

    #[test]
    fn test_tier_retention() {
        assert_eq!(SubscriptionTier::Free.retention_days(), 7);
        assert_eq!(SubscriptionTier::Solo.retention_days(), 30);
        assert_eq!(SubscriptionTier::Team.retention_days(), 90);
        assert_eq!(SubscriptionTier::Enterprise.retention_days(), 365);
        assert_eq!(SubscriptionTier::Custom.retention_days(), 3650);
    }

    #[test]
    fn test_tier_rate_limits() {
        assert_eq!(SubscriptionTier::Free.rate_limit_rps(), 1);
        assert_eq!(SubscriptionTier::Solo.rate_limit_rps(), 10);
        assert_eq!(SubscriptionTier::Team.rate_limit_rps(), 100);
        assert_eq!(SubscriptionTier::Enterprise.rate_limit_rps(), 1000);
        assert_eq!(SubscriptionTier::Custom.rate_limit_rps(), u32::MAX);
    }

    #[test]
    fn test_tier_concurrent_limits() {
        assert_eq!(SubscriptionTier::Free.concurrent_limit(), 1);
        assert_eq!(SubscriptionTier::Solo.concurrent_limit(), 5);
        assert_eq!(SubscriptionTier::Team.concurrent_limit(), 20);
        assert_eq!(SubscriptionTier::Enterprise.concurrent_limit(), 100);
        assert_eq!(SubscriptionTier::Custom.concurrent_limit(), u16::MAX);
    }

    #[test]
    fn test_tier_from_str() {
        assert_eq!(SubscriptionTier::parse("free"), Some(SubscriptionTier::Free));
        assert_eq!(SubscriptionTier::parse("FREE"), Some(SubscriptionTier::Free));
        assert_eq!(SubscriptionTier::parse("solo"), Some(SubscriptionTier::Solo));
        assert_eq!(SubscriptionTier::parse("team"), Some(SubscriptionTier::Team));
        assert_eq!(SubscriptionTier::parse("enterprise"), Some(SubscriptionTier::Enterprise));
        assert_eq!(SubscriptionTier::parse("custom"), Some(SubscriptionTier::Custom));
        assert_eq!(SubscriptionTier::parse("unknown"), None);
    }

    #[test]
    fn test_tier_as_str() {
        assert_eq!(SubscriptionTier::Free.as_str(), "free");
        assert_eq!(SubscriptionTier::Solo.as_str(), "solo");
        assert_eq!(SubscriptionTier::Team.as_str(), "team");
        assert_eq!(SubscriptionTier::Enterprise.as_str(), "enterprise");
        assert_eq!(SubscriptionTier::Custom.as_str(), "custom");
    }

    #[test]
    fn test_tier_from_u8() {
        assert_eq!(SubscriptionTier::from_u8(0), Some(SubscriptionTier::Free));
        assert_eq!(SubscriptionTier::from_u8(1), Some(SubscriptionTier::Solo));
        assert_eq!(SubscriptionTier::from_u8(2), Some(SubscriptionTier::Team));
        assert_eq!(SubscriptionTier::from_u8(3), Some(SubscriptionTier::Enterprise));
        assert_eq!(SubscriptionTier::from_u8(4), Some(SubscriptionTier::Custom));
        assert_eq!(SubscriptionTier::from_u8(5), None);
        assert_eq!(SubscriptionTier::from_u8(255), None);
    }

    #[test]
    fn test_tier_default() {
        assert_eq!(SubscriptionTier::default(), SubscriptionTier::Free);
    }

    #[test]
    fn test_tier_display() {
        assert_eq!(format!("{}", SubscriptionTier::Free), "free");
        assert_eq!(format!("{}", SubscriptionTier::Enterprise), "enterprise");
    }

    #[test]
    fn test_tier_cache_basic() {
        let cache = TierCache::new(12345);
        assert_eq!(cache.user_id(), 12345);
        assert_eq!(cache.load(), SubscriptionTier::Free);
    }

    #[test]
    fn test_tier_cache_with_tier() {
        let cache = TierCache::with_tier(12345, SubscriptionTier::Enterprise);
        assert_eq!(cache.user_id(), 12345);
        assert_eq!(cache.load(), SubscriptionTier::Enterprise);
    }

    #[test]
    fn test_tier_cache_store() {
        let cache = TierCache::new(12345);
        cache.store(SubscriptionTier::Enterprise);
        assert_eq!(cache.load(), SubscriptionTier::Enterprise);
    }

    #[test]
    fn test_tier_cache_compare_exchange() {
        let cache = TierCache::with_tier(12345, SubscriptionTier::Free);

        // Successful upgrade
        assert!(cache.compare_exchange(SubscriptionTier::Free, SubscriptionTier::Solo).is_ok());
        assert_eq!(cache.load(), SubscriptionTier::Solo);

        // Failed upgrade (current tier mismatch)
        let result = cache.compare_exchange(SubscriptionTier::Free, SubscriptionTier::Team);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SubscriptionTier::Solo);
    }

    #[test]
    fn test_tier_cache_quotas() {
        let cache = TierCache::with_tier(12345, SubscriptionTier::Team);
        let (monthly, retention, rate) = cache.quotas();
        assert_eq!(monthly, 1_000_000);
        assert_eq!(retention, 90);
        assert_eq!(rate, 100);
    }

    #[test]
    fn test_tier_cache_size_alignment() {
        assert_eq!(std::mem::size_of::<TierCache>(), 64);
        assert_eq!(std::mem::align_of::<TierCache>(), 64);
    }
}
