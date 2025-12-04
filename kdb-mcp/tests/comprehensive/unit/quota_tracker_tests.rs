//! Unit Tests for QuotaTrackerCapsule (Q1-Q7: 18 tests)
//!
//! Bug #1 Fix Validation: Tests verify proper month calculation with chrono

use kdb_mcp::QuotaTrackerCapsule;

#[test]
fn test_quota_tracker_size() {
    assert_eq!(
        std::mem::size_of::<QuotaTrackerCapsule>(),
        4096,
        "QuotaTrackerCapsule must be 4 KB"
    );
}

#[test]
fn test_quota_tracker_alignment() {
    assert_eq!(
        std::mem::align_of::<QuotaTrackerCapsule>(),
        64,
        "QuotaTrackerCapsule must be 64-byte aligned"
    );
}

#[test]
fn test_quota_allow_basic() {
    let tracker = QuotaTrackerCapsule::with_limits(10, 100, 1000);

    for _ in 0..9 {
        assert!(tracker.check(100).is_ok());
    }

    let stats = tracker.get_stats();
    assert_eq!(stats.total_requests, 9);
    assert_eq!(stats.daily_requests, 9);
    assert_eq!(stats.bytes_processed, 900);
}

#[test]
fn test_quota_daily_limit_exact() {
    let tracker = QuotaTrackerCapsule::with_limits(5, 100, 1000);

    // Consume exact daily quota
    for i in 0..5 {
        let result = tracker.check(10);
        assert!(result.is_ok(), "Request {} should succeed", i);
    }

    // Next should fail
    let result = tracker.check(10);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "daily_limit_exceeded");

    let stats = tracker.get_stats();
    assert_eq!(stats.quota_exceeded, 1);
}

#[test]
fn test_quota_monthly_limit() {
    let tracker = QuotaTrackerCapsule::with_limits(1000, 5, 1000);

    // Consume monthly quota
    for _ in 0..5 {
        assert!(tracker.check(100).is_ok());
    }

    // Next should fail with monthly limit
    let result = tracker.check(100);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "monthly_limit_exceeded");
}

#[test]
fn test_quota_total_limit() {
    let tracker = QuotaTrackerCapsule::with_limits(1000, 1000, 3);

    // Consume total quota
    for _ in 0..3 {
        assert!(tracker.check(10).is_ok());
    }

    // Next should fail with total limit
    let result = tracker.check(10);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "total_limit_exceeded");
}

#[test]
fn test_quota_bytes_tracking() {
    let tracker = QuotaTrackerCapsule::with_limits(100, 1000, 10000);

    tracker.check(1024).unwrap();
    tracker.check(2048).unwrap();
    tracker.check(512).unwrap();

    let stats = tracker.get_stats();
    assert_eq!(stats.bytes_processed, 1024 + 2048 + 512);
    assert_eq!(stats.total_requests, 3);
}

#[test]
fn test_quota_zero_bytes() {
    let tracker = QuotaTrackerCapsule::with_limits(10, 100, 1000);

    // Zero bytes should still count as request
    assert!(tracker.check(0).is_ok());

    let stats = tracker.get_stats();
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.bytes_processed, 0);
}

#[test]
fn test_quota_large_bytes() {
    let tracker = QuotaTrackerCapsule::with_limits(10, 100, 1000);

    // Large byte counts should work
    let bytes = u64::MAX / 2;
    assert!(tracker.check(bytes).is_ok());

    let stats = tracker.get_stats();
    assert_eq!(stats.bytes_processed, bytes);
}

#[test]
fn test_quota_exceeded_accumulation() {
    let tracker = QuotaTrackerCapsule::with_limits(2, 100, 1000);

    // Exceed daily limit multiple times
    assert!(tracker.check(10).is_ok());
    assert!(tracker.check(10).is_ok());
    assert!(tracker.check(10).is_err());
    assert!(tracker.check(10).is_err());
    assert!(tracker.check(10).is_err());

    let stats = tracker.get_stats();
    assert_eq!(stats.quota_exceeded, 3);
}

// ============================================================================
// Bug #1 Fix Validation: Month Boundary Tests (Critical)
// ============================================================================

#[test]
#[cfg(feature = "quota-tracker")]
fn test_month_boundaries_february() {
    use chrono::prelude::*;

    let tracker = QuotaTrackerCapsule::with_limits(1000, 100, 10000);

    // February 28, 2024 (leap year)
    let feb_28 = Utc.with_ymd_and_hms(2024, 2, 28, 23, 59, 59).unwrap().timestamp() as u64;
    let feb_28_month = tracker.get_unix_month(feb_28);

    // March 1, 2024
    let mar_1 = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap().timestamp() as u64;
    let mar_1_month = tracker.get_unix_month(mar_1);

    // Months should be different (February → March transition)
    assert_ne!(
        feb_28_month, mar_1_month,
        "February 28 and March 1 must have different month IDs (got {}, {})",
        feb_28_month, mar_1_month
    );
}

#[test]
#[cfg(feature = "quota-tracker")]
fn test_month_boundaries_february_non_leap_year() {
    use chrono::prelude::*;

    let tracker = QuotaTrackerCapsule::with_limits(1000, 100, 10000);

    // February 28, 2023 (non-leap year)
    let feb_28 = Utc.with_ymd_and_hms(2023, 2, 28, 23, 59, 59).unwrap().timestamp() as u64;
    let feb_28_month = tracker.get_unix_month(feb_28);

    // March 1, 2023
    let mar_1 = Utc.with_ymd_and_hms(2023, 3, 1, 0, 0, 0).unwrap().timestamp() as u64;
    let mar_1_month = tracker.get_unix_month(mar_1);

    // Months should be different
    assert_ne!(feb_28_month, mar_1_month, "Non-leap year: Feb 28 → Mar 1 transition failed");
}

#[test]
#[cfg(feature = "quota-tracker")]
fn test_month_boundaries_all_months() {
    use chrono::prelude::*;

    let tracker = QuotaTrackerCapsule::with_limits(1000, 100, 10000);

    // Test all month boundaries for 2024
    let month_days = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]; // 2024 is leap year

    for (month, &days) in month_days.iter().enumerate() {
        let month_num = (month + 1) as u32;

        // Last day of month
        let last_day = Utc
            .with_ymd_and_hms(2024, month_num, days, 23, 59, 59)
            .unwrap()
            .timestamp() as u64;
        let last_month = tracker.get_unix_month(last_day);

        // First day of next month
        let next_month_num = if month_num == 12 { 1 } else { month_num + 1 };
        let next_year = if month_num == 12 { 2025 } else { 2024 };
        let first_day_next = Utc
            .with_ymd_and_hms(next_year, next_month_num, 1, 0, 0, 0)
            .unwrap()
            .timestamp() as u64;
        let next_month = tracker.get_unix_month(first_day_next);

        assert_ne!(
            last_month, next_month,
            "Month boundary failed: {} day {} → {} day 1",
            month_num, days, next_month_num
        );
    }
}

#[test]
#[cfg(feature = "quota-tracker")]
fn test_month_same_within_month() {
    use chrono::prelude::*;

    let tracker = QuotaTrackerCapsule::with_limits(1000, 100, 10000);

    // All days within March 2024 should have same month ID
    let mar_1 = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap().timestamp() as u64;
    let mar_15 = Utc.with_ymd_and_hms(2024, 3, 15, 12, 30, 0).unwrap().timestamp() as u64;
    let mar_31 = Utc.with_ymd_and_hms(2024, 3, 31, 23, 59, 59).unwrap().timestamp() as u64;

    let month_1 = tracker.get_unix_month(mar_1);
    let month_15 = tracker.get_unix_month(mar_15);
    let month_31 = tracker.get_unix_month(mar_31);

    assert_eq!(month_1, month_15, "March 1 and March 15 must have same month ID");
    assert_eq!(month_1, month_31, "March 1 and March 31 must have same month ID");
}

#[test]
#[cfg(not(feature = "quota-tracker"))]
fn test_month_fallback_without_chrono() {
    let tracker = QuotaTrackerCapsule::with_limits(1000, 100, 10000);

    // Without chrono feature, should use 30-day approximation
    let now = 1704067200u64; // Jan 1, 2024 00:00:00 UTC
    let month1 = tracker.get_unix_month(now);
    let month2 = tracker.get_unix_month(now + 30 * 86400); // +30 days

    // Should be different months (30-day approximation)
    assert_ne!(month1, month2, "30-day approximation should detect month change");
}

#[test]
fn test_quota_stats_all_fields() {
    let tracker = QuotaTrackerCapsule::with_limits(100, 1000, 10000);

    // Generate some activity
    for i in 0..10 {
        let _ = tracker.check((i + 1) * 100);
    }

    // Exceed quota
    for _ in 0..5 {
        let _ = tracker.check(100);
    }

    let stats = tracker.get_stats();

    // Verify all fields populated
    assert!(stats.total_requests > 0);
    assert!(stats.daily_requests > 0);
    assert!(stats.monthly_requests > 0);
    assert_eq!(stats.daily_limit, 100);
    assert_eq!(stats.monthly_limit, 1000);
    assert_eq!(stats.total_limit, 10000);
    assert!(stats.bytes_processed > 0);
}
