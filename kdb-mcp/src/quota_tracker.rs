//! QuotaTrackerCapsule - T1 Atomic Usage Tracking (4 KB)
//!
//! Lockfree quota tracking with daily/monthly/total limits.
//! **Latency**: <70ns check + increment
//! **Tier**: T1 Atomic (AtomicU64 counters)

use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// QuotaTrackerCapsule (4 KB, 64-byte aligned)
// ============================================================================

#[repr(C, align(64))]
pub struct QuotaTrackerCapsule {
    // Usage counters (64 bytes, single cache line)
    pub total_requests: AtomicU64,       // Total requests (all time)
    pub daily_requests: AtomicU64,       // Requests today
    pub monthly_requests: AtomicU64,     // Requests this month
    pub last_reset_day: AtomicU64,       // Last daily reset (Unix day)
    pub last_reset_month: AtomicU64,     // Last monthly reset (Unix month)
    pub quota_exceeded: AtomicU64,       // Quota exceeded count
    pub bytes_processed: AtomicU64,      // Total bytes processed
    _padding: [u8; 8],

    // Quota limits (64 bytes, second cache line)
    pub daily_limit: AtomicU64,          // Daily request limit
    pub monthly_limit: AtomicU64,        // Monthly request limit
    pub total_limit: AtomicU64,          // Total request limit
    _padding2: [u8; 40],

    // Reserved space (4KB - 128 bytes = 3968 bytes)
    _reserved: [u8; 3968],
}

impl QuotaTrackerCapsule {
    /// Create new quota tracker (default: 10K daily, 100K monthly)
    pub const fn new() -> Self {
        Self::with_limits(10_000, 100_000, u64::MAX)
    }

    /// Create quota tracker with custom limits
    pub const fn with_limits(daily: u64, monthly: u64, total: u64) -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            daily_requests: AtomicU64::new(0),
            monthly_requests: AtomicU64::new(0),
            last_reset_day: AtomicU64::new(0),
            last_reset_month: AtomicU64::new(0),
            quota_exceeded: AtomicU64::new(0),
            bytes_processed: AtomicU64::new(0),
            _padding: [0; 8],
            daily_limit: AtomicU64::new(daily),
            monthly_limit: AtomicU64::new(monthly),
            total_limit: AtomicU64::new(total),
            _padding2: [0; 40],
            _reserved: [0; 3968],
        }
    }

    /// Check quota and increment usage (<70ns)
    ///
    /// Returns `Ok(())` if within quota, `Err(limit_type)` if exceeded.
    pub fn check_and_increment(&self, bytes: u64) -> Result<(), &'static str> {
        // Reset daily/monthly counters if needed
        self.maybe_reset();

        // Check total limit
        let total = self.total_requests.load(Ordering::Relaxed);
        let total_limit = self.total_limit.load(Ordering::Relaxed);
        if total >= total_limit {
            self.quota_exceeded.fetch_add(1, Ordering::Relaxed);
            return Err("total_limit_exceeded");
        }

        // Check monthly limit
        let monthly = self.monthly_requests.load(Ordering::Relaxed);
        let monthly_limit = self.monthly_limit.load(Ordering::Relaxed);
        if monthly >= monthly_limit {
            self.quota_exceeded.fetch_add(1, Ordering::Relaxed);
            return Err("monthly_limit_exceeded");
        }

        // Check daily limit
        let daily = self.daily_requests.load(Ordering::Relaxed);
        let daily_limit = self.daily_limit.load(Ordering::Relaxed);
        if daily >= daily_limit {
            self.quota_exceeded.fetch_add(1, Ordering::Relaxed);
            return Err("daily_limit_exceeded");
        }

        // Increment all counters
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.monthly_requests.fetch_add(1, Ordering::Relaxed);
        self.daily_requests.fetch_add(1, Ordering::Relaxed);
        self.bytes_processed.fetch_add(bytes, Ordering::Relaxed);

        Ok(())
    }

    /// Reset daily/monthly counters if time period changed
    fn maybe_reset(&self) {
        let now_unix = self.get_unix_seconds();
        let current_day = now_unix / 86400; // Unix day number
        let current_month = self.get_unix_month(now_unix);

        // Reset daily counter
        let last_day = self.last_reset_day.load(Ordering::Relaxed);
        if current_day > last_day {
            // Try to reset (CAS to handle concurrent resets)
            if self.last_reset_day.compare_exchange(
                last_day,
                current_day,
                Ordering::Release,
                Ordering::Relaxed,
            ).is_ok() {
                self.daily_requests.store(0, Ordering::Release);
            }
        }

        // Reset monthly counter
        let last_month = self.last_reset_month.load(Ordering::Relaxed);
        if current_month > last_month {
            if self.last_reset_month.compare_exchange(
                last_month,
                current_month,
                Ordering::Release,
                Ordering::Relaxed,
            ).is_ok() {
                self.monthly_requests.store(0, Ordering::Release);
            }
        }
    }

    fn get_unix_month(&self, unix_seconds: u64) -> u64 {
        // Approximate: 30.44 days/month average
        unix_seconds / (86400 * 30)
    }

    /// Get statistics
    pub fn get_stats(&self) -> QuotaStats {
        QuotaStats {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            daily_requests: self.daily_requests.load(Ordering::Relaxed),
            monthly_requests: self.monthly_requests.load(Ordering::Relaxed),
            daily_limit: self.daily_limit.load(Ordering::Relaxed),
            monthly_limit: self.monthly_limit.load(Ordering::Relaxed),
            total_limit: self.total_limit.load(Ordering::Relaxed),
            quota_exceeded: self.quota_exceeded.load(Ordering::Relaxed),
            bytes_processed: self.bytes_processed.load(Ordering::Relaxed),
        }
    }

    #[inline]
    fn get_unix_seconds(&self) -> u64 {
        #[cfg(feature = "std")]
        {
            use std::time::SystemTime;
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        }
        #[cfg(not(feature = "std"))]
        {
            0 // No-op in no_std
        }
    }

    // ========================================================================
    // Test Helper Methods (integration test support)
    // ========================================================================

    /// Check quota (alias for check_and_increment, for backward compatibility)
    #[doc(hidden)]
    pub fn check(&self, bytes: u64) -> Result<(), &'static str> {
        self.check_and_increment(bytes)
    }

    /// Reset all counters (test-only)
    #[doc(hidden)]
    pub fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.daily_requests.store(0, Ordering::Relaxed);
        self.monthly_requests.store(0, Ordering::Relaxed);
        self.quota_exceeded.store(0, Ordering::Relaxed);
        self.bytes_processed.store(0, Ordering::Relaxed);
    }
}

/// Quota tracking statistics
#[derive(Debug, Clone, Copy)]
pub struct QuotaStats {
    pub total_requests: u64,
    pub daily_requests: u64,
    pub monthly_requests: u64,
    pub daily_limit: u64,
    pub monthly_limit: u64,
    pub total_limit: u64,
    pub quota_exceeded: u64,
    pub bytes_processed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{size_of, align_of};

    #[test]
    fn test_quota_tracker_size() {
        assert_eq!(size_of::<QuotaTrackerCapsule>(), 4096, "QuotaTrackerCapsule must be 4 KB");
    }

    #[test]
    fn test_quota_tracker_alignment() {
        assert_eq!(align_of::<QuotaTrackerCapsule>(), 64, "QuotaTrackerCapsule must be 64-byte aligned");
    }

    #[test]
    fn test_quota_allow() {
        let tracker = QuotaTrackerCapsule::with_limits(10, 100, 1000);

        // Should allow first requests
        for _ in 0..9 {
            assert!(tracker.check_and_increment(100).is_ok());
        }

        let stats = tracker.get_stats();
        assert_eq!(stats.total_requests, 9);
        assert_eq!(stats.daily_requests, 9);
        assert_eq!(stats.bytes_processed, 900);
    }

    #[test]
    fn test_quota_daily_limit() {
        let tracker = QuotaTrackerCapsule::with_limits(5, 100, 1000);

        // Consume daily quota
        for _ in 0..5 {
            assert!(tracker.check_and_increment(100).is_ok());
        }

        // Next should fail
        let result = tracker.check_and_increment(100);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "daily_limit_exceeded");

        let stats = tracker.get_stats();
        assert_eq!(stats.quota_exceeded, 1);
    }
}
