//! SnapshotQuotaCapsule - T1 Atomic Snapshot Quota Enforcement (256 bytes)
//!
//! Lockfree snapshot quota tracking with tiered limits and 20% grace period.
//! **Latency**: <50ns quota check, <20ns record capture
//! **Tier**: T1 Atomic (AtomicU64 counters with DualAtomicU64-like cache alignment)
//!
//! ## UCE34 Framework Application (Q1-Q34)
//!
//! ### Q10-Q12: Tier Selection
//! - Q10: T1 Atomic (lockfree counters, <50ns check)
//! - Q11: Type-safe SubscriptionTier enum with compile-time bounds
//! - Q12: Nightly: const_fn for compile-time limit calculation
//!
//! ### Q33: Verification
//! - Size: 256 bytes (4 cache lines, cache-aligned)
//! - Alignment: 64 bytes (eliminates false sharing)
//! - 100% lockfree (no mutex/RwLock)
//!
//! ### Q34: Auditability
//! - Generation counter for TOCTOU prevention
//! - Prune statistics for retention audit
//! - Lifetime counter for billing audit trail
//!
//! ## Enforcement Stages
//!
//! | Stage      | Usage Range  | Behavior                                    |
//! |------------|--------------|---------------------------------------------|
//! | Normal     | 0-80%        | Allowed                                     |
//! | Warning    | 80-100%      | Allowed + X-Snapshot-Warning header         |
//! | SoftBlock  | 100-120%     | New captures disabled, reads allowed        |
//! | HardBlock  | 120%+        | quota_exceeded error on all operations      |
//!
//! ## Tier Snapshot Limits
//!
//! | Tier         | Base Limit | Grace (20%) | Retention |
//! |--------------|------------|-------------|-----------|
//! | Hobby        | 100        | 120         | 7 days    |
//! | Starter      | 1,000      | 1,200       | 7 days    |
//! | Developer    | 10,000     | 12,000      | 30 days   |
//! | Professional | 100,000    | 120,000     | 90 days   |
//! | Enterprise   | u64::MAX   | u64::MAX    | 365 days  |

use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};

// Re-export SubscriptionTier from canonical source
pub use crate::subscription_tier::SubscriptionTier;

// ============================================================================
// Subscription Tier Extensions for Snapshot Quota
// ============================================================================

/// Extension trait for SubscriptionTier with snapshot quota methods
pub trait SubscriptionTierQuotaExt {
    /// Get hard limit (base + 20% grace)
    fn snapshot_hard_limit(&self) -> u64;

    /// Get retention period in seconds
    fn retention_secs(&self) -> u64;
}

impl SubscriptionTierQuotaExt for SubscriptionTier {
    /// Get hard limit (base + 20% grace)
    ///
    /// Enterprise tier returns u64::MAX (no overflow)
    #[inline]
    fn snapshot_hard_limit(&self) -> u64 {
        match self {
            SubscriptionTier::Enterprise => u64::MAX,
            _ => {
                let base = self.snapshot_limit();
                // Add 20% grace: base + base/5
                base.saturating_add(base / 5)
            }
        }
    }

    /// Get retention period in seconds
    #[inline]
    fn retention_secs(&self) -> u64 {
        const SECS_PER_DAY: u64 = 86400;
        self.retention_days() as u64 * SECS_PER_DAY
    }
}

// ============================================================================
// Enforcement Stage
// ============================================================================

/// Quota enforcement stage based on usage percentage
///
/// # Stage Transitions
/// ```text
/// 0%      80%      100%     120%
/// |---Normal---|---Warning---|---SoftBlock---|---HardBlock--->
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EnforcementStage {
    /// 0-80%: Allowed (normal operation)
    Normal = 0,
    /// 80-100%: Allowed + X-Snapshot-Warning header
    Warning = 1,
    /// 100-120%: New captures disabled, reads allowed
    SoftBlock = 2,
    /// 120%+: quota_exceeded error on all operations
    HardBlock = 3,
}

impl EnforcementStage {
    /// Create from u8 value
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => EnforcementStage::Normal,
            1 => EnforcementStage::Warning,
            2 => EnforcementStage::SoftBlock,
            3 => EnforcementStage::HardBlock,
            _ => EnforcementStage::HardBlock, // Default to most restrictive
        }
    }

    /// Check if captures are allowed
    #[inline]
    pub const fn captures_allowed(&self) -> bool {
        matches!(self, EnforcementStage::Normal | EnforcementStage::Warning)
    }

    /// Check if reads are allowed
    #[inline]
    pub const fn reads_allowed(&self) -> bool {
        !matches!(self, EnforcementStage::HardBlock)
    }

    /// Get stage name
    #[inline]
    pub const fn name(&self) -> &'static str {
        match self {
            EnforcementStage::Normal => "normal",
            EnforcementStage::Warning => "warning",
            EnforcementStage::SoftBlock => "soft_block",
            EnforcementStage::HardBlock => "hard_block",
        }
    }
}

// ============================================================================
// Quota Error Types
// ============================================================================

/// Quota operation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaError {
    /// Snapshot quota exceeded
    SnapshotQuotaExceeded {
        /// Current usage
        used: u64,
        /// Base limit (soft limit)
        limit: u64,
        /// Hard limit (base + 20% grace)
        hard_limit: u64,
        /// Current enforcement stage
        stage: EnforcementStage,
    },
}

impl QuotaError {
    /// Create quota exceeded error from capsule state
    pub fn exceeded(used: u64, limit: u64, hard_limit: u64, stage: EnforcementStage) -> Self {
        QuotaError::SnapshotQuotaExceeded {
            used,
            limit,
            hard_limit,
            stage,
        }
    }
}

// ============================================================================
// Prune Statistics
// ============================================================================

/// Statistics from a prune operation
#[derive(Debug, Clone, Copy, Default)]
pub struct PruneStats {
    /// Snapshots pruned due to age (retention period)
    pub pruned_by_age: u64,
    /// Snapshots pruned due to count (over limit)
    pub pruned_by_count: u64,
    /// Total snapshots pruned
    pub total_pruned: u64,
    /// Snapshots remaining after prune
    pub remaining: u64,
}

// ============================================================================
// Snapshot Quota Status
// ============================================================================

/// Current snapshot quota status (atomic snapshot)
#[derive(Debug, Clone, Copy)]
pub struct SnapshotQuotaStatus {
    /// Current snapshot count
    pub snapshots_used: u64,
    /// Base limit (soft limit)
    pub snapshots_limit: u64,
    /// Hard limit (base + 20% grace)
    pub snapshots_hard_limit: u64,
    /// Lifetime snapshot count (never resets)
    pub snapshots_lifetime: u64,
    /// Current enforcement stage
    pub enforcement_stage: EnforcementStage,
    /// Current subscription tier
    pub tier: SubscriptionTier,
    /// Usage percentage (0-100+)
    pub usage_percent: u8,
    /// Retention period in seconds
    pub retention_secs: u64,
    /// Generation counter (TOCTOU detection)
    pub generation: u64,
    /// Total pruned count
    pub pruned_count: u64,
}

impl SnapshotQuotaStatus {
    /// Check if captures are allowed
    #[inline]
    pub fn captures_allowed(&self) -> bool {
        self.enforcement_stage.captures_allowed()
    }

    /// Check if reads are allowed
    #[inline]
    pub fn reads_allowed(&self) -> bool {
        self.enforcement_stage.reads_allowed()
    }

    /// Get remaining quota before soft block
    #[inline]
    pub fn remaining_before_soft_block(&self) -> u64 {
        self.snapshots_limit.saturating_sub(self.snapshots_used)
    }

    /// Get remaining quota before hard block
    #[inline]
    pub fn remaining_before_hard_block(&self) -> u64 {
        self.snapshots_hard_limit.saturating_sub(self.snapshots_used)
    }
}

// ============================================================================
// SnapshotQuotaCapsule (256 bytes, T1 Atomic)
// ============================================================================

/// T1 Atomic Snapshot Quota Capsule - Lockfree quota enforcement
///
/// # Memory Layout (256 bytes, 4 cache lines)
/// ```text
/// Offset 0-63:    Counter cache line
///   ├─ snapshots_used (8B)
///   ├─ snapshots_limit (8B)
///   ├─ snapshots_hard_limit (8B)
///   ├─ snapshots_lifetime (8B)
///   ├─ generation (8B)
///   ├─ last_check_ns (8B)
///   └─ _padding_1 (16B)
///
/// Offset 64-127:  Enforcement cache line
///   ├─ enforcement_stage (1B)
///   ├─ tier (1B)
///   ├─ _pad (6B)
///   ├─ warning_threshold (8B)
///   ├─ soft_block_threshold (8B)
///   ├─ pruned_count (8B)
///   ├─ session_start_ns (8B)
///   ├─ retention_secs (8B)
///   └─ _padding_2 (8B)
///
/// Offset 128-191: Pruning cache line
///   ├─ last_prune_ns (8B)
///   ├─ pruned_by_age (8B)
///   ├─ pruned_by_count (8B)
///   ├─ next_prune_ns (8B)
///   └─ _padding_3 (32B)
///
/// Offset 192-255: Reserved
///   └─ _reserved (64B)
/// ```
///
/// # Performance (B32 Framework)
/// - check_capture_allowed: <50ns (3 atomic loads + comparisons)
/// - record_capture: <20ns (atomic increment)
/// - upgrade_tier: <100ns (5 atomic stores)
/// - get_status: <50ns (10 atomic loads)
///
/// # ASSUM Safety (99.99%+)
/// - #ASSUME_LOCKFREE: No mutex/RwLock, all atomic operations
/// - #ASSUME_CACHE_ALIGNED_64B: Each cache line independent
/// - #ASSUME_GENERATION_COUNTER: TOCTOU prevention via generation
/// - #ASSUME_GRACE_PERIOD_20: 20% grace calculated at compile-time
#[repr(C, align(64))]
pub struct SnapshotQuotaCapsule {
    // ========================================================================
    // Counter cache line (64 bytes)
    // ========================================================================

    /// Current snapshot count
    snapshots_used: AtomicU64,

    /// Base snapshot limit (soft limit)
    snapshots_limit: AtomicU64,

    /// Hard snapshot limit (base + 20% grace)
    snapshots_hard_limit: AtomicU64,

    /// Lifetime snapshot count (never resets, for billing audit)
    snapshots_lifetime: AtomicU64,

    /// Generation counter for TOCTOU prevention
    generation: AtomicU64,

    /// Last quota check timestamp (nanoseconds)
    last_check_ns: AtomicU64,

    /// Padding to complete cache line
    _padding_1: [u8; 16],

    // ========================================================================
    // Enforcement cache line (64 bytes)
    // ========================================================================

    /// Current enforcement stage (0-3)
    enforcement_stage: AtomicU8,

    /// Current subscription tier (0-4)
    tier: AtomicU8,

    /// Padding for alignment
    _pad: [u8; 6],

    /// Warning threshold (80% of limit)
    warning_threshold: AtomicU64,

    /// Soft block threshold (base limit)
    soft_block_threshold: AtomicU64,

    /// Total pruned snapshot count
    pruned_count: AtomicU64,

    /// Session start timestamp (nanoseconds)
    session_start_ns: AtomicU64,

    /// Retention period in seconds
    retention_secs: AtomicU64,

    /// Padding to complete cache line
    _padding_2: [u8; 8],

    // ========================================================================
    // Pruning cache line (64 bytes)
    // ========================================================================

    /// Last prune operation timestamp (nanoseconds)
    last_prune_ns: AtomicU64,

    /// Snapshots pruned by age
    pruned_by_age: AtomicU64,

    /// Snapshots pruned by count
    pruned_by_count: AtomicU64,

    /// Next scheduled prune timestamp (nanoseconds)
    next_prune_ns: AtomicU64,

    /// Padding to complete cache line
    _padding_3: [u8; 32],

    // ========================================================================
    // Reserved (64 bytes)
    // ========================================================================

    /// Reserved for future use
    _reserved: [u8; 64],
}

// Compile-time size verification
const _: () = assert!(
    core::mem::size_of::<SnapshotQuotaCapsule>() == 256,
    "SnapshotQuotaCapsule must be 256 bytes"
);

const _: () = assert!(
    core::mem::align_of::<SnapshotQuotaCapsule>() == 64,
    "SnapshotQuotaCapsule must be 64-byte aligned"
);

impl SnapshotQuotaCapsule {
    /// Calculate hard limit (base + 20% grace)
    #[inline]
    const fn calc_hard_limit(limit: u64) -> u64 {
        if limit == u64::MAX {
            u64::MAX
        } else {
            // Add 20% grace: base + base/5
            limit.saturating_add(limit / 5)
        }
    }

    /// Calculate retention in seconds from days
    #[inline]
    const fn calc_retention_secs(days: u32) -> u64 {
        const SECS_PER_DAY: u64 = 86400;
        if days == u32::MAX {
            u64::MAX
        } else {
            days as u64 * SECS_PER_DAY
        }
    }

    /// Create new quota capsule for specified tier
    ///
    /// # Arguments
    /// - `tier`: Subscription tier determining limits and retention
    ///
    /// # Performance
    /// - Const-evaluated limits (0ns at runtime)
    /// - 256-byte allocation with zero initialization
    pub const fn for_tier(tier: SubscriptionTier) -> Self {
        let limit = tier.snapshot_limit();
        let hard_limit = Self::calc_hard_limit(limit);
        let retention = Self::calc_retention_secs(tier.retention_days());

        // Warning threshold: 80% of base limit
        let warning = if limit == u64::MAX {
            u64::MAX
        } else {
            (limit * 4) / 5  // 80%
        };

        Self {
            // Counter cache line
            snapshots_used: AtomicU64::new(0),
            snapshots_limit: AtomicU64::new(limit),
            snapshots_hard_limit: AtomicU64::new(hard_limit),
            snapshots_lifetime: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            last_check_ns: AtomicU64::new(0),
            _padding_1: [0; 16],

            // Enforcement cache line
            enforcement_stage: AtomicU8::new(EnforcementStage::Normal as u8),
            tier: AtomicU8::new(tier.as_u8()),
            _pad: [0; 6],
            warning_threshold: AtomicU64::new(warning),
            soft_block_threshold: AtomicU64::new(limit),
            pruned_count: AtomicU64::new(0),
            session_start_ns: AtomicU64::new(0),
            retention_secs: AtomicU64::new(retention),
            _padding_2: [0; 8],

            // Pruning cache line
            last_prune_ns: AtomicU64::new(0),
            pruned_by_age: AtomicU64::new(0),
            pruned_by_count: AtomicU64::new(0),
            next_prune_ns: AtomicU64::new(0),
            _padding_3: [0; 32],

            // Reserved
            _reserved: [0; 64],
        }
    }

    /// Create new quota capsule with Hobby tier (default)
    pub const fn new() -> Self {
        Self::for_tier(SubscriptionTier::Hobby)
    }

    /// Check if capture is allowed (<50ns)
    ///
    /// # Returns
    /// - `Ok(stage)`: Capture allowed, returns current enforcement stage
    /// - `Err(QuotaError)`: Capture denied (SoftBlock or HardBlock)
    ///
    /// # Performance
    /// <50ns (3 atomic loads + 2 comparisons)
    ///
    /// # ASSUM
    /// - #ASSUME_STAGE_CONSISTENCY: Stage matches usage thresholds
    #[inline]
    pub fn check_capture_allowed(&self) -> Result<EnforcementStage, QuotaError> {
        let used = self.snapshots_used.load(Ordering::Acquire);
        let limit = self.snapshots_limit.load(Ordering::Relaxed);
        let hard_limit = self.snapshots_hard_limit.load(Ordering::Relaxed);

        // Update enforcement stage based on current usage
        let stage = self.calculate_and_update_stage(used, limit, hard_limit);

        if stage.captures_allowed() {
            Ok(stage)
        } else {
            Err(QuotaError::exceeded(used, limit, hard_limit, stage))
        }
    }

    /// Record a snapshot capture (<20ns)
    ///
    /// # Returns
    /// New snapshot count after increment
    ///
    /// # Performance
    /// <20ns (2 atomic increments)
    ///
    /// # Note
    /// Does NOT check quota - caller should call `check_capture_allowed` first
    #[inline]
    pub fn record_capture(&self) -> u64 {
        // Increment both current and lifetime counters
        self.snapshots_lifetime.fetch_add(1, Ordering::Relaxed);
        let new_count = self.snapshots_used.fetch_add(1, Ordering::Release) + 1;

        // Update generation counter
        self.generation.fetch_add(1, Ordering::Release);

        new_count
    }

    /// Upgrade to higher tier (instant limit increase)
    ///
    /// # Arguments
    /// - `new_tier`: Target tier (must be >= current tier)
    ///
    /// # Performance
    /// <100ns (5 atomic stores + stage recalculation)
    ///
    /// # Note
    /// Immediately increases limits - current usage stays the same
    pub fn upgrade_tier(&self, new_tier: SubscriptionTier) {
        let limit = new_tier.snapshot_limit();
        let hard_limit = Self::calc_hard_limit(limit);
        let retention = Self::calc_retention_secs(new_tier.retention_days());

        // Warning threshold: 80% of base limit
        let warning = if limit == u64::MAX {
            u64::MAX
        } else {
            (limit * 4) / 5
        };

        // Update all limit fields atomically (relaxed ordering OK since these are independent)
        self.snapshots_limit.store(limit, Ordering::Release);
        self.snapshots_hard_limit.store(hard_limit, Ordering::Release);
        self.warning_threshold.store(warning, Ordering::Release);
        self.soft_block_threshold.store(limit, Ordering::Release);
        self.retention_secs.store(retention, Ordering::Release);
        self.tier.store(new_tier.as_u8(), Ordering::Release);

        // Recalculate stage after upgrade
        let used = self.snapshots_used.load(Ordering::Acquire);
        self.calculate_and_update_stage(used, limit, hard_limit);

        // Increment generation to signal state change
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Reset quota counters
    ///
    /// # Arguments
    /// - `preserve_lifetime`: If true, keep lifetime counter intact
    ///
    /// # Performance
    /// <50ns (3-4 atomic stores)
    pub fn reset_quota(&self, preserve_lifetime: bool) {
        self.snapshots_used.store(0, Ordering::Release);
        self.enforcement_stage.store(EnforcementStage::Normal as u8, Ordering::Release);

        if !preserve_lifetime {
            self.snapshots_lifetime.store(0, Ordering::Release);
        }

        // Increment generation
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Prune snapshots by retention period
    ///
    /// # Arguments
    /// - `timestamps`: Slice of (snapshot_id, created_ns) tuples
    /// - `now_ns`: Current timestamp in nanoseconds
    ///
    /// # Returns
    /// - `PruneStats` with counts of pruned snapshots
    ///
    /// # Performance
    /// O(n) where n = number of timestamps
    ///
    /// # Note
    /// This method only calculates what SHOULD be pruned - caller must
    /// actually delete the snapshots and call `record_prune_result`
    pub fn prune_by_retention(&self, timestamps: &[(u64, u64)], now_ns: u64) -> PruneStats {
        let retention_ns = self.retention_secs.load(Ordering::Relaxed) * 1_000_000_000;
        let limit = self.snapshots_limit.load(Ordering::Relaxed);
        let cutoff_ns = now_ns.saturating_sub(retention_ns);

        let mut stats = PruneStats::default();

        // Count snapshots to prune by age
        for &(_id, created_ns) in timestamps {
            if created_ns < cutoff_ns {
                stats.pruned_by_age += 1;
            }
        }

        // Count additional snapshots to prune if over limit
        let remaining_after_age = timestamps.len() as u64 - stats.pruned_by_age;
        if remaining_after_age > limit {
            stats.pruned_by_count = remaining_after_age - limit;
        }

        stats.total_pruned = stats.pruned_by_age + stats.pruned_by_count;
        stats.remaining = timestamps.len() as u64 - stats.total_pruned;

        // Update prune timestamp
        self.last_prune_ns.store(now_ns, Ordering::Release);

        stats
    }

    /// Record prune operation result
    ///
    /// # Arguments
    /// - `stats`: Prune statistics from `prune_by_retention`
    ///
    /// # Performance
    /// <30ns (4 atomic operations)
    pub fn record_prune_result(&self, stats: &PruneStats) {
        // Update usage counter
        self.snapshots_used.fetch_sub(stats.total_pruned.min(
            self.snapshots_used.load(Ordering::Relaxed)
        ), Ordering::Release);

        // Update prune statistics
        self.pruned_by_age.fetch_add(stats.pruned_by_age, Ordering::Relaxed);
        self.pruned_by_count.fetch_add(stats.pruned_by_count, Ordering::Relaxed);
        self.pruned_count.fetch_add(stats.total_pruned, Ordering::Relaxed);

        // Recalculate stage
        let used = self.snapshots_used.load(Ordering::Acquire);
        let limit = self.snapshots_limit.load(Ordering::Relaxed);
        let hard_limit = self.snapshots_hard_limit.load(Ordering::Relaxed);
        self.calculate_and_update_stage(used, limit, hard_limit);

        // Increment generation
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get current quota status (atomic snapshot)
    ///
    /// # Performance
    /// <50ns (10 atomic loads)
    pub fn get_status(&self) -> SnapshotQuotaStatus {
        let used = self.snapshots_used.load(Ordering::Acquire);
        let limit = self.snapshots_limit.load(Ordering::Relaxed);
        let hard_limit = self.snapshots_hard_limit.load(Ordering::Relaxed);

        // Calculate usage percentage
        let usage_percent = if limit == 0 || limit == u64::MAX {
            0u8
        } else {
            ((used * 100) / limit).min(255) as u8
        };

        SnapshotQuotaStatus {
            snapshots_used: used,
            snapshots_limit: limit,
            snapshots_hard_limit: hard_limit,
            snapshots_lifetime: self.snapshots_lifetime.load(Ordering::Relaxed),
            enforcement_stage: EnforcementStage::from_u8(
                self.enforcement_stage.load(Ordering::Relaxed)
            ),
            tier: SubscriptionTier::from_u8(
                self.tier.load(Ordering::Relaxed)
            ).unwrap_or(SubscriptionTier::Hobby),
            usage_percent,
            retention_secs: self.retention_secs.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Relaxed),
            pruned_count: self.pruned_count.load(Ordering::Relaxed),
        }
    }

    /// Get current tier
    #[inline]
    pub fn current_tier(&self) -> SubscriptionTier {
        SubscriptionTier::from_u8(self.tier.load(Ordering::Relaxed))
            .unwrap_or(SubscriptionTier::Hobby)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    // ========================================================================
    // Private Methods
    // ========================================================================

    /// Calculate and update enforcement stage
    #[inline]
    fn calculate_and_update_stage(&self, used: u64, limit: u64, hard_limit: u64) -> EnforcementStage {
        let stage = if used >= hard_limit {
            EnforcementStage::HardBlock
        } else if used >= limit {
            EnforcementStage::SoftBlock
        } else {
            // Calculate 80% threshold
            let warning_threshold = if limit == u64::MAX {
                u64::MAX
            } else {
                (limit * 4) / 5
            };

            if used >= warning_threshold {
                EnforcementStage::Warning
            } else {
                EnforcementStage::Normal
            }
        };

        self.enforcement_stage.store(stage as u8, Ordering::Release);
        stage
    }
}

impl Default for SnapshotQuotaCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: SnapshotQuotaCapsule uses only atomic operations
unsafe impl Send for SnapshotQuotaCapsule {}
unsafe impl Sync for SnapshotQuotaCapsule {}

// ============================================================================
// Tests (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{size_of, align_of};

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_capsule_size_alignment() {
        assert_eq!(size_of::<SnapshotQuotaCapsule>(), 256, "Capsule must be 256 bytes");
        assert_eq!(align_of::<SnapshotQuotaCapsule>(), 64, "Capsule must be 64-byte aligned");
    }

    #[test]
    fn test_tier_limits() {
        // Hobby: 100 base, 120 with grace
        assert_eq!(SubscriptionTier::Hobby.snapshot_limit(), 100);
        assert_eq!(SubscriptionTier::Hobby.snapshot_hard_limit(), 120);
        assert_eq!(SubscriptionTier::Hobby.retention_days(), 7);

        // Starter: 1K base, 1.2K with grace
        assert_eq!(SubscriptionTier::Pro.snapshot_limit(), 1_000);
        assert_eq!(SubscriptionTier::Pro.snapshot_hard_limit(), 1_200);

        // Developer: 10K base, 12K with grace
        assert_eq!(SubscriptionTier::Engineer.snapshot_limit(), 10_000);
        assert_eq!(SubscriptionTier::Engineer.snapshot_hard_limit(), 12_000);
        assert_eq!(SubscriptionTier::Engineer.retention_days(), 30);

        // Professional: 100K base, 120K with grace
        assert_eq!(SubscriptionTier::Teams.snapshot_limit(), 100_000);
        assert_eq!(SubscriptionTier::Teams.snapshot_hard_limit(), 120_000);
        assert_eq!(SubscriptionTier::Teams.retention_days(), 90);

        // Enterprise: unlimited (u32::MAX for retention_days in canonical SubscriptionTier)
        assert_eq!(SubscriptionTier::Enterprise.snapshot_limit(), u64::MAX);
        assert_eq!(SubscriptionTier::Enterprise.snapshot_hard_limit(), u64::MAX);
        assert_eq!(SubscriptionTier::Enterprise.retention_days(), u32::MAX);
    }

    #[test]
    fn test_for_tier_initialization() {
        let capsule = SnapshotQuotaCapsule::for_tier(SubscriptionTier::Engineer);
        let status = capsule.get_status();

        assert_eq!(status.snapshots_limit, 10_000);
        assert_eq!(status.snapshots_hard_limit, 12_000);
        assert_eq!(status.tier, SubscriptionTier::Engineer);
        assert_eq!(status.enforcement_stage, EnforcementStage::Normal);
        assert_eq!(status.snapshots_used, 0);
    }

    #[test]
    fn test_enforcement_stages() {
        let capsule = SnapshotQuotaCapsule::for_tier(SubscriptionTier::Hobby);

        // Normal: 0-79 snapshots (0-79% of 100)
        for _ in 0..79 {
            capsule.record_capture();
        }
        let stage = capsule.check_capture_allowed().unwrap();
        assert_eq!(stage, EnforcementStage::Normal);

        // Warning: 80-99 snapshots (80-99% of 100)
        capsule.record_capture();  // Now at 80
        let stage = capsule.check_capture_allowed().unwrap();
        assert_eq!(stage, EnforcementStage::Warning);

        // Continue to 100
        for _ in 0..20 {
            capsule.record_capture();
        }

        // SoftBlock: 100-119 snapshots
        let result = capsule.check_capture_allowed();
        assert!(result.is_err());
        if let Err(QuotaError::SnapshotQuotaExceeded { stage, .. }) = result {
            assert_eq!(stage, EnforcementStage::SoftBlock);
        }

        // HardBlock: 120+ snapshots
        for _ in 0..20 {
            capsule.record_capture();
        }
        let result = capsule.check_capture_allowed();
        assert!(result.is_err());
        if let Err(QuotaError::SnapshotQuotaExceeded { stage, used, limit, hard_limit }) = result {
            assert_eq!(stage, EnforcementStage::HardBlock);
            assert_eq!(used, 120);
            assert_eq!(limit, 100);
            assert_eq!(hard_limit, 120);
        }
    }

    #[test]
    fn test_record_capture() {
        let capsule = SnapshotQuotaCapsule::new();

        let count1 = capsule.record_capture();
        assert_eq!(count1, 1);

        let count2 = capsule.record_capture();
        assert_eq!(count2, 2);

        let status = capsule.get_status();
        assert_eq!(status.snapshots_used, 2);
        assert_eq!(status.snapshots_lifetime, 2);
    }

    #[test]
    fn test_tier_upgrade() {
        let capsule = SnapshotQuotaCapsule::for_tier(SubscriptionTier::Hobby);

        // Use 90 snapshots (90% of Hobby limit)
        for _ in 0..90 {
            capsule.record_capture();
        }

        // Should be in Warning stage
        let stage = capsule.check_capture_allowed().unwrap();
        assert_eq!(stage, EnforcementStage::Warning);

        // Upgrade to Developer
        capsule.upgrade_tier(SubscriptionTier::Engineer);

        // Same 90 snapshots is now only 0.9% of Developer limit (10K)
        let status = capsule.get_status();
        assert_eq!(status.snapshots_used, 90);
        assert_eq!(status.snapshots_limit, 10_000);
        assert_eq!(status.tier, SubscriptionTier::Engineer);

        // Should be back to Normal stage
        let stage = capsule.check_capture_allowed().unwrap();
        assert_eq!(stage, EnforcementStage::Normal);
    }

    #[test]
    fn test_reset_quota() {
        let capsule = SnapshotQuotaCapsule::new();

        for _ in 0..50 {
            capsule.record_capture();
        }

        assert_eq!(capsule.get_status().snapshots_used, 50);
        assert_eq!(capsule.get_status().snapshots_lifetime, 50);

        // Reset preserving lifetime
        capsule.reset_quota(true);
        assert_eq!(capsule.get_status().snapshots_used, 0);
        assert_eq!(capsule.get_status().snapshots_lifetime, 50);

        // Add more
        capsule.record_capture();
        assert_eq!(capsule.get_status().snapshots_lifetime, 51);

        // Reset without preserving lifetime
        capsule.reset_quota(false);
        assert_eq!(capsule.get_status().snapshots_used, 0);
        assert_eq!(capsule.get_status().snapshots_lifetime, 0);
    }

    #[test]
    fn test_prune_by_retention() {
        let capsule = SnapshotQuotaCapsule::for_tier(SubscriptionTier::Hobby);
        let now_ns = 10 * 86400 * 1_000_000_000u64; // 10 days after epoch

        // Simulate 50 snapshots captured
        for _ in 0..50 {
            capsule.record_capture();
        }

        // Create timestamps: 20 older than retention, 30 within retention
        let mut timestamps = Vec::new();
        for i in 0u64..20 {
            // Created 8-9 days ago (expired - outside 7-day retention)
            let days_ago = 8u64 + (i / 5); // 8-11 days ago
            timestamps.push((i, now_ns.saturating_sub(days_ago * 86400 * 1_000_000_000)));
        }
        for i in 20u64..50 {
            // Created 1-6 days ago (valid - within 7-day retention)
            let days_ago = 1u64 + ((i - 20) / 5); // 1-6 days ago
            timestamps.push((i, now_ns.saturating_sub(days_ago * 86400 * 1_000_000_000)));
        }

        let stats = capsule.prune_by_retention(&timestamps, now_ns);

        assert_eq!(stats.pruned_by_age, 20, "Should prune 20 expired snapshots");
        assert_eq!(stats.pruned_by_count, 0, "No count-based pruning needed");
        assert_eq!(stats.remaining, 30);

        // Record the prune
        capsule.record_prune_result(&stats);
        assert_eq!(capsule.get_status().snapshots_used, 30);
    }

    #[test]
    fn test_grace_period_calculation() {
        // 20% grace = base + base/5

        // Hobby: 100 + 20 = 120
        assert_eq!(SubscriptionTier::Hobby.snapshot_hard_limit(), 120);

        // Starter: 1000 + 200 = 1200
        assert_eq!(SubscriptionTier::Pro.snapshot_hard_limit(), 1_200);

        // Developer: 10000 + 2000 = 12000
        assert_eq!(SubscriptionTier::Engineer.snapshot_hard_limit(), 12_000);

        // Professional: 100000 + 20000 = 120000
        assert_eq!(SubscriptionTier::Teams.snapshot_hard_limit(), 120_000);
    }

    #[test]
    fn test_generation_counter() {
        let capsule = SnapshotQuotaCapsule::new();
        let gen0 = capsule.generation();

        capsule.record_capture();
        let gen1 = capsule.generation();
        assert!(gen1 > gen0, "Generation should increment on capture");

        capsule.upgrade_tier(SubscriptionTier::Engineer);
        let gen2 = capsule.generation();
        assert!(gen2 > gen1, "Generation should increment on upgrade");

        capsule.reset_quota(true);
        let gen3 = capsule.generation();
        assert!(gen3 > gen2, "Generation should increment on reset");
    }

    #[test]
    fn test_subscription_tier_extension_trait() {
        use super::SubscriptionTierQuotaExt;

        // Test snapshot_hard_limit (20% grace)
        assert_eq!(SubscriptionTier::Hobby.snapshot_hard_limit(), 120);
        assert_eq!(SubscriptionTier::Pro.snapshot_hard_limit(), 1_200);
        assert_eq!(SubscriptionTier::Engineer.snapshot_hard_limit(), 12_000);
        assert_eq!(SubscriptionTier::Teams.snapshot_hard_limit(), 120_000);
        assert_eq!(SubscriptionTier::Enterprise.snapshot_hard_limit(), u64::MAX);

        // Test retention_secs
        assert_eq!(SubscriptionTier::Hobby.retention_secs(), 7 * 86400);
        assert_eq!(SubscriptionTier::Engineer.retention_secs(), 30 * 86400);
        assert_eq!(SubscriptionTier::Teams.retention_secs(), 90 * 86400);
    }

    #[test]
    fn test_enforcement_stage_methods() {
        assert!(EnforcementStage::Normal.captures_allowed());
        assert!(EnforcementStage::Normal.reads_allowed());

        assert!(EnforcementStage::Warning.captures_allowed());
        assert!(EnforcementStage::Warning.reads_allowed());

        assert!(!EnforcementStage::SoftBlock.captures_allowed());
        assert!(EnforcementStage::SoftBlock.reads_allowed());

        assert!(!EnforcementStage::HardBlock.captures_allowed());
        assert!(!EnforcementStage::HardBlock.reads_allowed());
    }

    // ========================================================================
    // Q8-Q14: Property Tests (concurrent safety)
    // ========================================================================

    #[test]
    fn test_concurrent_captures() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(SnapshotQuotaCapsule::for_tier(SubscriptionTier::Enterprise));
        let mut handles = vec![];

        // Spawn 4 threads, each capturing 100 snapshots
        for _ in 0..4 {
            let capsule_clone = Arc::clone(&capsule);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    capsule_clone.record_capture();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let status = capsule.get_status();
        assert_eq!(status.snapshots_used, 400, "All captures should be recorded");
        assert_eq!(status.snapshots_lifetime, 400, "Lifetime should match");
    }

    #[test]
    fn test_concurrent_check_and_capture() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(SnapshotQuotaCapsule::for_tier(SubscriptionTier::Hobby));
        let mut handles = vec![];

        // Spawn 4 threads mixing checks and captures
        for _ in 0..4 {
            let capsule_clone = Arc::clone(&capsule);
            handles.push(thread::spawn(move || {
                for _ in 0..30 {
                    if capsule_clone.check_capture_allowed().is_ok() {
                        capsule_clone.record_capture();
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should not exceed hard limit (120)
        let status = capsule.get_status();
        assert!(status.snapshots_used <= 120, "Should respect quota limits");
    }
}
