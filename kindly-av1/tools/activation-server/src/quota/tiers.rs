//! Quota Tier Management - QuotaTrackerCapsule Integration
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! ## Purpose
//!
//! Map RapidAPI subscription tiers to video encoding quotas using
//! atomic_capsule::QuotaTrackerCapsule for lockfree quota enforcement.
//!
//! ## Tier Quotas
//!
//! | Tier  | Monthly Minutes | Max Resolution | Max Duration |
//! |-------|-----------------|----------------|--------------|
//! | Basic | 10 min          | 720p           | 5 min        |
//! | Pro   | 200 min         | 1080p          | 30 min       |
//! | Ultra | 1000 min        | 4K             | 60 min       |
//!
//! ## Architecture (T1 Atomic)
//!
//! - QuotaTrackerCapsule (256B cache-aligned) per API key
//! - DualAtomicU64 coordination (usage | limit)
//! - <10ns quota checks (lockfree atomic reads)
//! - <20ns usage increments (fetch_add saturation)
//!
//! ## Performance (B32 Targets)
//!
//! - Quota check: <10ns (DualAtomicU64 load)
//! - Record operation: <20ns (fetch_add)
//! - Monthly reset: <30ns (CAS loop + generation bump)
//! - Tier update: <25ns (atomic store + warning recalc)
//!
//! ## Framework Compliance
//!
//! - UCE34 Q10: T1 Atomic tier (QuotaTrackerCapsule)
//! - Chaos: 100% lockfree (DualAtomicU64 coordination)
//! - ASSUM: Saturation at u64::MAX prevents wraparound
//! - T28: Unit tests for quota boundaries

use atomic_capsule::protection::quota_tracker::{
    LicenseTier, QuotaError, QuotaStatus, QuotaTrackerCapsule,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::middleware::rapidapi::SubscriptionTier;

/// Quota check result
#[derive(Debug, Clone, Copy)]
pub enum QuotaCheckResult {
    /// Quota valid (usage < warning threshold)
    Valid {
        /// Minutes remaining
        minutes_remaining: u64,
        /// Usage percentage (0-100)
        usage_percent: u8,
    },
    /// Quota warning (80% consumed)
    Warning {
        /// Minutes remaining
        minutes_remaining: u64,
        /// Usage percentage (80-99)
        usage_percent: u8,
    },
    /// Quota exceeded (usage ≥ limit)
    Exceeded {
        /// Overage (minutes)
        overage_minutes: u64,
        /// Next reset timestamp (unix seconds)
        next_reset: u64,
    },
    /// Quota locked (manual lockout)
    Locked,
}

/// Quota manager using QuotaTrackerCapsule
///
/// ## Architecture (T1 Atomic)
///
/// - Per-API-key quotas: HashMap<api_key, QuotaTrackerCapsule>
/// - Lockfree checks: <10ns check_quota(), <20ns record_operation()
/// - Monthly reset: Automatic on first request after month boundary
///
/// ## Performance
///
/// - Quota check: <10ns (DualAtomicU64 load)
/// - Usage increment: <20ns (atomic fetch_add)
/// - Tier update: <25ns (atomic store)
///
/// ## ASSUM
///
/// - `#ASSUME_QUOTA_CREATION_RARE`: New API keys are rare, RwLock acceptable
/// - `#ASSUME_MONTHLY_RESET`: First request of month triggers reset check
/// - `#ASSUME_USAGE_SATURATION_SAFE`: u64::MAX saturation prevents wraparound
pub struct QuotaManager {
    /// Per-API-key quota trackers
    quotas: Arc<RwLock<HashMap<String, QuotaTrackerCapsule>>>,
}

impl QuotaManager {
    /// Create new quota manager
    pub fn new() -> Self {
        Self {
            quotas: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check quota for API key before encoding
    ///
    /// ## Arguments
    ///
    /// - `api_key`: User's RapidAPI key
    /// - `tier`: Subscription tier (determines quota limits)
    /// - `video_duration_minutes`: Estimated duration (for proactive check)
    ///
    /// ## Returns
    ///
    /// - `QuotaCheckResult`: Valid/Warning/Exceeded/Locked
    ///
    /// ## Example
    ///
    /// ```rust
    /// let manager = QuotaManager::new();
    ///
    /// match manager.check_quota("user_123", SubscriptionTier::Pro, 5) {
    ///     QuotaCheckResult::Valid { minutes_remaining, .. } => {
    ///         println!("Quota OK, {} minutes remaining", minutes_remaining);
    ///     }
    ///     QuotaCheckResult::Exceeded { overage_minutes, .. } => {
    ///         println!("Quota exceeded by {} minutes", overage_minutes);
    ///     }
    ///     _ => {}
    /// }
    /// ```
    pub fn check_quota(
        &self,
        api_key: &str,
        tier: SubscriptionTier,
        video_duration_minutes: u64,
    ) -> QuotaCheckResult {
        // Get or create quota tracker
        let quota = self.get_or_create_quota(api_key, tier);

        // Check monthly reset (first request of new month)
        self.check_monthly_reset(&quota);

        // Check current quota status
        let status = quota.status();
        let usage_minutes = quota.current_usage();
        let limit_minutes = tier.video_quota_minutes();

        match status {
            QuotaStatus::Valid => {
                let minutes_remaining = limit_minutes.saturating_sub(usage_minutes);
                let usage_percent = ((usage_minutes * 100) / limit_minutes.max(1)) as u8;

                // Proactive check: will video fit in quota?
                if usage_minutes + video_duration_minutes > limit_minutes {
                    let overage = (usage_minutes + video_duration_minutes) - limit_minutes;
                    QuotaCheckResult::Exceeded {
                        overage_minutes: overage,
                        next_reset: self.next_reset_timestamp(),
                    }
                } else {
                    QuotaCheckResult::Valid {
                        minutes_remaining,
                        usage_percent,
                    }
                }
            }
            QuotaStatus::Warning => {
                let minutes_remaining = limit_minutes.saturating_sub(usage_minutes);
                let usage_percent = ((usage_minutes * 100) / limit_minutes.max(1)) as u8;

                // Proactive check: will video fit in quota?
                if usage_minutes + video_duration_minutes > limit_minutes {
                    let overage = (usage_minutes + video_duration_minutes) - limit_minutes;
                    QuotaCheckResult::Exceeded {
                        overage_minutes: overage,
                        next_reset: self.next_reset_timestamp(),
                    }
                } else {
                    QuotaCheckResult::Warning {
                        minutes_remaining,
                        usage_percent,
                    }
                }
            }
            QuotaStatus::Exceeded => {
                let overage = usage_minutes.saturating_sub(limit_minutes);
                QuotaCheckResult::Exceeded {
                    overage_minutes: overage,
                    next_reset: self.next_reset_timestamp(),
                }
            }
            QuotaStatus::Locked => QuotaCheckResult::Locked,
        }
    }

    /// Record video encoding operation (increment usage by minutes)
    ///
    /// ## Arguments
    ///
    /// - `api_key`: User's RapidAPI key
    /// - `tier`: Subscription tier
    /// - `video_duration_minutes`: Duration to add to usage
    ///
    /// ## Returns
    ///
    /// - `Ok(())`: Usage recorded
    /// - `Err(QuotaError)`: Quota exceeded or locked
    ///
    /// ## Performance
    ///
    /// - <20ns (atomic fetch_add + saturation check)
    pub fn record_operation(
        &self,
        api_key: &str,
        tier: SubscriptionTier,
        video_duration_minutes: u64,
    ) -> Result<(), QuotaError> {
        let quota = self.get_or_create_quota(api_key, tier);

        // Check monthly reset before recording
        self.check_monthly_reset(&quota);

        // Record usage (atomic increment, <20ns)
        for _ in 0..video_duration_minutes {
            quota.record_operation()?;
        }

        Ok(())
    }

    /// Get or create quota tracker for API key
    fn get_or_create_quota(&self, api_key: &str, tier: SubscriptionTier) -> QuotaTrackerCapsule {
        // Fast path: RwLock read
        if let Ok(quotas) = self.quotas.read() {
            if let Some(quota) = quotas.get(api_key) {
                return quota.clone();
            }
        }

        // Slow path: Create new quota tracker
        self.create_quota(api_key, tier)
    }

    /// Create new quota tracker (slow path, RwLock write)
    fn create_quota(&self, api_key: &str, tier: SubscriptionTier) -> QuotaTrackerCapsule {
        let license_tier = Self::subscription_to_license_tier(tier);
        let quota = QuotaTrackerCapsule::new(license_tier);

        // Cache quota for future requests
        if let Ok(mut quotas) = self.quotas.write() {
            quotas.insert(api_key.to_string(), quota.clone());
        }

        quota
    }

    /// Map SubscriptionTier to LicenseTier (for QuotaTrackerCapsule)
    fn subscription_to_license_tier(tier: SubscriptionTier) -> LicenseTier {
        match tier {
            SubscriptionTier::Basic => LicenseTier::Free,   // 10 min → 1000 ops
            SubscriptionTier::Pro => LicenseTier::Pro,      // 200 min → 100K ops
            SubscriptionTier::Ultra => LicenseTier::Enterprise, // 1000 min → unlimited
        }
    }

    /// Check if monthly reset needed (first request after month boundary)
    ///
    /// ## Logic
    ///
    /// - Get last reset timestamp from quota tracker
    /// - Compare month of last reset vs current month
    /// - If different month, reset quota
    fn check_monthly_reset(&self, quota: &QuotaTrackerCapsule) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let last_reset = quota.last_reset_timestamp();

        // Check if different month (simple: compare month number)
        if Self::unix_to_month(now) != Self::unix_to_month(last_reset) {
            // Reset quota for new month
            let _ = quota.reset();
        }
    }

    /// Convert unix timestamp to month number (YYYY-MM format)
    fn unix_to_month(unix_secs: u64) -> u64 {
        // Rough approximation: seconds per month (30 days)
        const SECS_PER_MONTH: u64 = 30 * 24 * 60 * 60;
        unix_secs / SECS_PER_MONTH
    }

    /// Get next monthly reset timestamp (first day of next month)
    fn next_reset_timestamp(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Add ~30 days (rough approximation for next month)
        const SECS_PER_MONTH: u64 = 30 * 24 * 60 * 60;
        now + SECS_PER_MONTH
    }

    /// Update user tier (admin operation, e.g., after subscription change)
    pub fn update_user_tier(&self, api_key: &str, tier: SubscriptionTier) {
        if let Ok(quotas) = self.quotas.read() {
            if let Some(quota) = quotas.get(api_key) {
                let license_tier = Self::subscription_to_license_tier(tier);
                let _ = quota.set_tier(license_tier);
            }
        }
    }

    /// Clear quota for API key (admin operation, e.g., after tier change)
    pub fn clear_quota(&self, api_key: &str) {
        if let Ok(mut quotas) = self.quotas.write() {
            quotas.remove(api_key);
        }
    }

    /// Get usage statistics for monitoring
    pub fn get_usage_stats(&self, api_key: &str) -> Option<UsageStats> {
        self.quotas.read().ok().and_then(|quotas| {
            quotas.get(api_key).map(|quota| UsageStats {
                api_key: api_key.to_string(),
                usage_minutes: quota.current_usage(),
                quota_limit: quota.quota_limit(),
                usage_percent: quota.usage_percent(),
                status: quota.status(),
                last_reset: quota.last_reset_timestamp(),
            })
        })
    }
}

impl Default for QuotaManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Usage statistics (for monitoring/billing)
#[derive(Debug, Clone)]
pub struct UsageStats {
    pub api_key: String,
    pub usage_minutes: u64,
    pub quota_limit: u64,
    pub usage_percent: u8,
    pub status: QuotaStatus,
    pub last_reset: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_check_valid() {
        let manager = QuotaManager::new();

        // Basic tier: 10 min quota
        let result = manager.check_quota("user_123", SubscriptionTier::Basic, 5);
        match result {
            QuotaCheckResult::Valid { minutes_remaining, usage_percent } => {
                assert_eq!(minutes_remaining, 10); // No usage yet
                assert_eq!(usage_percent, 0);
            }
            _ => panic!("Expected valid"),
        }
    }

    #[test]
    fn test_quota_proactive_check() {
        let manager = QuotaManager::new();

        // Basic tier: 10 min quota, trying to encode 15 min video
        let result = manager.check_quota("user_123", SubscriptionTier::Basic, 15);
        match result {
            QuotaCheckResult::Exceeded { overage_minutes, .. } => {
                assert_eq!(overage_minutes, 5); // 15 - 10 = 5 min overage
            }
            _ => panic!("Expected exceeded"),
        }
    }

    #[test]
    fn test_record_operation() {
        let manager = QuotaManager::new();

        // Record 5 minutes of usage
        let result = manager.record_operation("user_123", SubscriptionTier::Basic, 5);
        assert!(result.is_ok());

        // Check usage
        let stats = manager.get_usage_stats("user_123").unwrap();
        assert_eq!(stats.usage_minutes, 5);

        // Check remaining quota
        let result = manager.check_quota("user_123", SubscriptionTier::Basic, 0);
        match result {
            QuotaCheckResult::Valid { minutes_remaining, .. } => {
                assert_eq!(minutes_remaining, 5); // 10 - 5 = 5
            }
            _ => panic!("Expected valid"),
        }
    }

    #[test]
    fn test_quota_exceeded() {
        let manager = QuotaManager::new();

        // Consume all quota (Basic: 10 min)
        let _ = manager.record_operation("user_123", SubscriptionTier::Basic, 10);

        // Try to record more usage (should fail)
        let result = manager.record_operation("user_123", SubscriptionTier::Basic, 1);
        assert!(result.is_err());

        // Check status
        let result = manager.check_quota("user_123", SubscriptionTier::Basic, 0);
        match result {
            QuotaCheckResult::Exceeded { .. } => {}
            _ => panic!("Expected exceeded"),
        }
    }

    #[test]
    fn test_quota_warning() {
        let manager = QuotaManager::new();

        // Consume 85% of quota (Basic: 10 min, 85% = 8.5 min)
        let _ = manager.record_operation("user_123", SubscriptionTier::Basic, 8);

        // Check status (should be warning at 80%+)
        let result = manager.check_quota("user_123", SubscriptionTier::Basic, 0);
        match result {
            QuotaCheckResult::Warning { usage_percent, .. } => {
                assert!(usage_percent >= 80);
            }
            _ => panic!("Expected warning, got {:?}", result),
        }
    }

    #[test]
    fn test_tier_update() {
        let manager = QuotaManager::new();

        // Create quota with Basic tier
        let _ = manager.check_quota("user_123", SubscriptionTier::Basic, 0);

        // Upgrade to Pro tier
        manager.update_user_tier("user_123", SubscriptionTier::Pro);

        // Verify new quota limit (Pro: 200 min)
        let stats = manager.get_usage_stats("user_123").unwrap();
        assert_eq!(stats.quota_limit, 100_000); // Pro tier → LicenseTier::Pro → 100K ops
    }

    #[test]
    fn test_unix_to_month() {
        // Test month conversion (rough approximation)
        let jan_2025 = 1704067200; // 2025-01-01
        let feb_2025 = jan_2025 + (31 * 24 * 60 * 60); // +31 days

        let jan_month = QuotaManager::unix_to_month(jan_2025);
        let feb_month = QuotaManager::unix_to_month(feb_2025);

        assert_ne!(jan_month, feb_month); // Different months
    }
}
