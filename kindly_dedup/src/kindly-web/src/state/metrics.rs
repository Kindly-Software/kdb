//! MetricsCapsule - Tier 1 Atomic (64B)
//!
//! Purpose: UI metrics tracking (page views, clicks, submissions, performance)
//! Memory Layout:
//!   [0-3]   page_views: AtomicU32 (total page views)
//!   [4-7]   clicks: AtomicU32 (total clicks)
//!   [8-11]  submissions: AtomicU32 (form submissions)
//!   [12-13] performance_p99_ms: AtomicU16 (p99 latency in milliseconds)
//!   [14-63] _padding: [u8; 50] (cache alignment)

use super::error::CapsuleResult;
use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU16, AtomicU32, Ordering};

/// Tier 1 Atomic: Metrics capsule (64B cache-aligned)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct MetricsCapsule {
    /// Total page views
    page_views: AtomicU32,
    /// Total clicks
    clicks: AtomicU32,
    /// Total form submissions
    submissions: AtomicU32,
    /// Performance p99 latency in milliseconds
    performance_p99_ms: AtomicU16,
    /// Padding to 64 bytes (cache line alignment)
    _padding: [u8; 50],
}

impl MetricsCapsule {
    /// Create new metrics capsule
    ///
    /// # Returns
    /// MetricsCapsule with all counters at zero
    pub const fn new() -> Self {
        Self {
            page_views: AtomicU32::new(0),
            clicks: AtomicU32::new(0),
            submissions: AtomicU32::new(0),
            performance_p99_ms: AtomicU16::new(0),
            _padding: [0u8; 50],
        }
    }

    /// Record page view
    ///
    /// #ASSUME: Fetch-add with Relaxed safe (page_views is monotonic counter)
    /// #VERIFY: No overflow (u32 can hold 4.2B page views)
    ///
    /// # Returns
    /// New page view count after increment
    pub fn record_page_view(&self) -> u32 {
        // #ASSUME: Relaxed ordering safe (page_views is audit counter)
        self.page_views.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }

    /// Record click event
    ///
    /// #ASSUME: Fetch-add with Relaxed safe (clicks is monotonic counter)
    ///
    /// # Returns
    /// New click count after increment
    pub fn record_click(&self) -> u32 {
        // #ASSUME: Relaxed ordering safe (clicks is audit counter)
        self.clicks.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }

    /// Record form submission
    ///
    /// #ASSUME: Fetch-add with Relaxed safe (submissions is monotonic counter)
    ///
    /// # Returns
    /// New submission count after increment
    pub fn record_submission(&self) -> u32 {
        // #ASSUME: Relaxed ordering safe (submissions is audit counter)
        self.submissions.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }

    /// Update p99 performance metric
    ///
    /// #ASSUME: Atomic store prevents race conditions
    /// #VERIFY: Latency fits in u16 (0-65535ms)
    ///
    /// # Arguments
    /// * `latency_ms` - p99 latency in milliseconds
    ///
    /// # Returns
    /// Ok or error if latency > 65535ms
    pub fn update_performance_p99(&self, latency_ms: u16) -> CapsuleResult<()> {
        // #ASSUME: Relaxed ordering safe (performance_p99 is independent metric)
        self.performance_p99_ms.store(latency_ms, Ordering::Relaxed);
        Ok(())
    }

    /// Get page view count
    ///
    /// #ASSUME: Relaxed load safe (page_views is audit counter)
    pub fn get_page_views(&self) -> u32 {
        self.page_views.load(Ordering::Relaxed)
    }

    /// Get click count
    ///
    /// #ASSUME: Relaxed load safe (clicks is audit counter)
    pub fn get_clicks(&self) -> u32 {
        self.clicks.load(Ordering::Relaxed)
    }

    /// Get submission count
    ///
    /// #ASSUME: Relaxed load safe (submissions is audit counter)
    pub fn get_submissions(&self) -> u32 {
        self.submissions.load(Ordering::Relaxed)
    }

    /// Get p99 performance metric
    ///
    /// #ASSUME: Relaxed load safe (performance_p99 is independent metric)
    pub fn get_performance_p99_ms(&self) -> u16 {
        self.performance_p99_ms.load(Ordering::Relaxed)
    }

    /// Reset all metrics to zero
    ///
    /// #ASSUME: Atomic stores prevent race conditions
    /// #VERIFY: Reset is intentional operation (use with care)
    pub fn reset(&self) {
        // #ASSUME: Relaxed ordering safe (metrics are independent counters)
        self.page_views.store(0, Ordering::Relaxed);
        self.clicks.store(0, Ordering::Relaxed);
        self.submissions.store(0, Ordering::Relaxed);
        self.performance_p99_ms.store(0, Ordering::Relaxed);
    }

    /// Get snapshot of all metrics
    ///
    /// #ASSUME: Four separate loads (order doesn't matter for metrics snapshot)
    ///
    /// # Returns
    /// (page_views, clicks, submissions, performance_p99_ms)
    pub fn snapshot(&self) -> (u32, u32, u32, u16) {
        let views = self.page_views.load(Ordering::Relaxed);
        let clicks = self.clicks.load(Ordering::Relaxed);
        let subs = self.submissions.load(Ordering::Relaxed);
        let perf = self.performance_p99_ms.load(Ordering::Relaxed);
        (views, clicks, subs, perf)
    }

    /// Calculate click-through rate (CTR)
    ///
    /// # Returns
    /// CTR as percentage (0.0-100.0) or 0.0 if no page views
    pub fn click_through_rate(&self) -> f64 {
        let views = self.get_page_views();
        let clicks = self.get_clicks();

        if views == 0 {
            0.0
        } else {
            (clicks as f64 / views as f64) * 100.0
        }
    }

    /// Calculate submission rate
    ///
    /// # Returns
    /// Submission rate as percentage (0.0-100.0) or 0.0 if no page views
    pub fn submission_rate(&self) -> f64 {
        let views = self.get_page_views();
        let subs = self.get_submissions();

        if views == 0 {
            0.0
        } else {
            (subs as f64 / views as f64) * 100.0
        }
    }
}

impl Default for MetricsCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_alignment() {
        assert_eq!(std::mem::align_of::<MetricsCapsule>(), 64);
        assert_eq!(std::mem::size_of::<MetricsCapsule>(), 64);
    }

    #[test]
    fn test_initial_state() {
        let metrics = MetricsCapsule::new();
        assert_eq!(metrics.get_page_views(), 0);
        assert_eq!(metrics.get_clicks(), 0);
        assert_eq!(metrics.get_submissions(), 0);
        assert_eq!(metrics.get_performance_p99_ms(), 0);
    }

    #[test]
    fn test_record_page_view() {
        let metrics = MetricsCapsule::new();

        let count1 = metrics.record_page_view();
        assert_eq!(count1, 1);
        assert_eq!(metrics.get_page_views(), 1);

        let count2 = metrics.record_page_view();
        assert_eq!(count2, 2);
        assert_eq!(metrics.get_page_views(), 2);
    }

    #[test]
    fn test_record_click() {
        let metrics = MetricsCapsule::new();

        let count1 = metrics.record_click();
        assert_eq!(count1, 1);

        let count2 = metrics.record_click();
        assert_eq!(count2, 2);
        assert_eq!(metrics.get_clicks(), 2);
    }

    #[test]
    fn test_record_submission() {
        let metrics = MetricsCapsule::new();

        let count1 = metrics.record_submission();
        assert_eq!(count1, 1);

        let count2 = metrics.record_submission();
        assert_eq!(count2, 2);
        assert_eq!(metrics.get_submissions(), 2);
    }

    #[test]
    fn test_update_performance() {
        let metrics = MetricsCapsule::new();

        metrics.update_performance_p99(150).unwrap();
        assert_eq!(metrics.get_performance_p99_ms(), 150);

        metrics.update_performance_p99(200).unwrap();
        assert_eq!(metrics.get_performance_p99_ms(), 200);
    }

    #[test]
    fn test_snapshot() {
        let metrics = MetricsCapsule::new();

        metrics.record_page_view();
        metrics.record_page_view();
        metrics.record_click();
        metrics.record_submission();
        metrics.update_performance_p99(100).unwrap();

        let (views, clicks, subs, perf) = metrics.snapshot();
        assert_eq!(views, 2);
        assert_eq!(clicks, 1);
        assert_eq!(subs, 1);
        assert_eq!(perf, 100);
    }

    #[test]
    fn test_click_through_rate() {
        let metrics = MetricsCapsule::new();

        // No page views
        assert_eq!(metrics.click_through_rate(), 0.0);

        metrics.record_page_view();
        metrics.record_page_view();
        metrics.record_click();

        // 1 click / 2 views = 50%
        assert!((metrics.click_through_rate() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_submission_rate() {
        let metrics = MetricsCapsule::new();

        // No page views
        assert_eq!(metrics.submission_rate(), 0.0);

        metrics.record_page_view();
        metrics.record_page_view();
        metrics.record_page_view();
        metrics.record_page_view();
        metrics.record_submission();

        // 1 submission / 4 views = 25%
        assert!((metrics.submission_rate() - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_reset() {
        let metrics = MetricsCapsule::new();

        metrics.record_page_view();
        metrics.record_click();
        metrics.record_submission();
        metrics.update_performance_p99(100).unwrap();

        metrics.reset();

        assert_eq!(metrics.get_page_views(), 0);
        assert_eq!(metrics.get_clicks(), 0);
        assert_eq!(metrics.get_submissions(), 0);
        assert_eq!(metrics.get_performance_p99_ms(), 0);
    }
}
