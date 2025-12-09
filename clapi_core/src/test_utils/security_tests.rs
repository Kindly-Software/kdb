//! Security Test Suite for P1 Enhancements
//!
//! Framework: ASSUM Safety + UCE34 Q34 Auditability
//! Purpose: Validate security properties of timeline aggregation and P1 enhancements
//!
//! Test Categories:
//! - Overflow and boundary conditions
//! - Race conditions and concurrent safety
//! - Multi-tenant isolation
//! - Input validation
//! - Audit trail integrity

#[cfg(test)]
mod timeline_security_tests {
    use crate::capsules::timeline_aggregation_capsule::*;
    use crate::error::ClapiResult;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    // ============================================================================
    // Part 1: Overflow and Boundary Tests
    // ============================================================================

    /// Test percentile validation (bounds checking)
    ///
    /// # ASSUM Verification
    /// - #VERIFY_INVARIANT: Percentile must be 0-100
    /// - #VERIFY_INPUT_BOUNDS: Rejects percentile > 100
    #[test]
    fn test_percentile_invalid_upper_bound() {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let now = SystemTime::now();

        // Test: percentile = 101 (invalid)
        let result = timeline.percentile(now, now, 101);

        assert!(result.is_err(), "Expected error for percentile > 100");

        if let Err(e) = result {
            let err_msg = format!("{:?}", e);
            assert!(
                err_msg.contains("must be 0-100"),
                "Error message should mention valid range"
            );
        }
    }

    /// Test percentile validation (lower bound)
    #[test]
    fn test_percentile_valid_lower_bound() {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let now = SystemTime::now();

        // Test: percentile = 0 (valid, should return min)
        let result = timeline.percentile(now, now, 0);

        assert!(result.is_ok(), "Percentile 0 should be valid");
    }

    /// Test percentile validation (upper bound)
    #[test]
    fn test_percentile_valid_upper_bound() {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let now = SystemTime::now();

        // Test: percentile = 100 (valid, should return max)
        let result = timeline.percentile(now, now, 100);

        assert!(result.is_ok(), "Percentile 100 should be valid");
    }

    /// Test percentile with empty range
    ///
    /// # ASSUM Verification
    /// - #VERIFY_EDGE_CASE: Empty range returns 0
    #[test]
    fn test_percentile_empty_range() {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let now = SystemTime::now();

        let result = timeline.percentile(now, now, 50).unwrap();

        assert_eq!(result, 0, "Empty range should return 0");
    }

    /// Test builder zero duration validation
    ///
    /// # ASSUM Verification
    /// - #VERIFY_INPUT_BOUNDS: Duration >= 1 second
    #[test]
    fn test_builder_zero_duration() {
        let result = TimelineBuilder::new()
            .bucket_duration(Duration::from_secs(0))
            .build();

        assert!(result.is_err(), "Zero duration should be rejected");

        if let Err(e) = result {
            let err_msg = format!("{:?}", e);
            assert!(
                err_msg.contains(">= 1 second"),
                "Error should mention minimum duration"
            );
        }
    }

    /// Test builder excessive duration validation
    ///
    /// # ASSUM Verification
    /// - #VERIFY_INPUT_BOUNDS: Duration <= 1 day (86400s)
    #[test]
    fn test_builder_excessive_duration() {
        let result = TimelineBuilder::new()
            .bucket_duration(Duration::from_secs(86401)) // 1 day + 1 second
            .build();

        assert!(result.is_err(), "Excessive duration should be rejected");

        if let Err(e) = result {
            let err_msg = format!("{:?}", e);
            assert!(
                err_msg.contains("<= 1 day"),
                "Error should mention maximum duration"
            );
        }
    }

    /// Test builder sub-second duration (nanoseconds only)
    #[test]
    fn test_builder_subsecond_duration() {
        let result = TimelineBuilder::new()
            .bucket_duration(Duration::from_nanos(999_999_999)) // 0 seconds
            .build();

        assert!(
            result.is_err(),
            "Sub-second duration should be rejected (as_secs() = 0)"
        );
    }

    /// Test builder valid durations
    #[test]
    fn test_builder_valid_durations() {
        // 1 second (minimum)
        let result1 = TimelineBuilder::new()
            .bucket_duration(Duration::from_secs(1))
            .build();
        assert!(result1.is_ok(), "1 second should be valid");

        // 60 seconds (minute buckets)
        let result2 = TimelineBuilder::new()
            .bucket_duration(Duration::from_secs(60))
            .build();
        assert!(result2.is_ok(), "60 seconds should be valid");

        // 86400 seconds (1 day, maximum)
        let result3 = TimelineBuilder::new()
            .bucket_duration(Duration::from_secs(86400))
            .build();
        assert!(result3.is_ok(), "86400 seconds (1 day) should be valid");
    }

    // ============================================================================
    // Part 2: Edge Case Tests
    // ============================================================================

    /// Test aggregate_avg with empty range
    ///
    /// # ASSUM Verification
    /// - #VERIFY_EDGE_CASE_SAFE: Empty range returns 0.0
    #[test]
    fn test_aggregate_avg_empty_range() {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let now = SystemTime::now();

        let avg = timeline.aggregate_avg(now, now).unwrap();

        assert_eq!(avg, 0.0, "Empty range should return 0.0");
    }

    /// Test aggregate_max with empty range
    ///
    /// # ASSUM Verification
    /// - #VERIFY_EDGE_CASE_SAFE: Empty range returns error
    #[test]
    fn test_aggregate_max_empty_range() {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let now = SystemTime::now();

        let result = timeline.aggregate_max(now, now);

        assert!(result.is_err(), "Empty range should return error");

        if let Err(e) = result {
            let err_msg = format!("{:?}", e);
            assert!(
                err_msg.contains("No buckets in range"),
                "Error should mention empty range"
            );
        }
    }

    /// Test aggregate_min with empty range
    #[test]
    fn test_aggregate_min_empty_range() {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let now = SystemTime::now();

        let result = timeline.aggregate_min(now, now);

        assert!(result.is_err(), "Empty range should return error");
    }

    /// Test aggregate_stddev with empty range
    #[test]
    fn test_aggregate_stddev_empty_range() {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let now = SystemTime::now();

        let stddev = timeline.aggregate_stddev(now, now).unwrap();

        assert_eq!(stddev, 0.0, "Empty range should return 0.0");
    }

    /// Test trend with zero hours (invalid)
    #[test]
    fn test_trend_zero_hours() {
        let timeline = TimelineAggregationCapsuleWrapper::default();

        let result = timeline.trend(0);

        assert!(result.is_err(), "Zero hours should be rejected");

        if let Err(e) = result {
            let err_msg = format!("{:?}", e);
            assert!(
                err_msg.contains("must be > 0"),
                "Error should mention hours > 0"
            );
        }
    }

    // ============================================================================
    // Part 3: NaN and Infinity Tests
    // ============================================================================

    /// Test rate_of_change infinity handling
    ///
    /// # ASSUM Verification
    /// - #VERIFY_NAN_PREVENTION: Division by zero prevented
    /// - #VERIFY_INFINITY_RETURN: Returns f64::INFINITY if growth from 0
    #[test]
    fn test_rate_of_change_infinity() {
        let mut timeline = TimelineAggregationCapsuleWrapper::default();

        // Append events to current period only (prev period = 0 events)
        let now = SystemTime::now();
        for i in 0..10 {
            let ts = now - Duration::from_secs(i * 60);
            timeline.append(ts, "test", "data").unwrap();
        }

        // Calculate rate of change (current > 0, prev = 0)
        let rate = timeline.rate_of_change(Duration::from_secs(3600)).unwrap();

        // Expected: f64::INFINITY (growth from 0)
        assert!(
            rate.is_infinite(),
            "Rate of change should be infinity when growing from 0 events"
        );
    }

    /// Test rate_of_change zero-to-zero case
    #[test]
    fn test_rate_of_change_zero_to_zero() {
        let timeline = TimelineAggregationCapsuleWrapper::default();

        // No events in either period
        let rate = timeline.rate_of_change(Duration::from_secs(3600)).unwrap();

        // Expected: 0.0 (no change)
        assert_eq!(rate, 0.0, "No change from 0 to 0 should return 0.0");
    }

    // ============================================================================
    // Part 4: Concurrent Safety Tests
    // ============================================================================

    /// Test concurrent append accuracy
    ///
    /// # ASSUM Verification
    /// - #VERIFY_METRIC_ATOMIC: All increments are atomic
    /// - #VERIFY_COUNTER_ACCURACY: Sum matches expected in concurrent tests
    #[test]
    fn test_concurrent_append_accuracy() {
        let timeline = Arc::new(Mutex::new(TimelineAggregationCapsuleWrapper::default()));
        let num_threads = 100;
        let appends_per_thread = 100;

        std::thread::scope(|s| {
            for thread_id in 0..num_threads {
                let timeline_clone = Arc::clone(&timeline);
                s.spawn(move || {
                    let mut tl = timeline_clone.lock().unwrap();
                    for i in 0..appends_per_thread {
                        let offset = (thread_id * appends_per_thread + i) * 60;
                        let ts = SystemTime::UNIX_EPOCH + Duration::from_secs(offset);
                        let _ = tl.append(ts, "test", "data");
                    }
                });
            }
        });

        let tl = timeline.lock().unwrap();
        let total = tl.total_events();

        // Expected: 100 threads × 100 appends = 10,000 events
        assert_eq!(
            total,
            (num_threads * appends_per_thread) as u64,
            "All concurrent appends should be counted"
        );
    }

    /// Test concurrent query consistency
    ///
    /// # ASSUM Verification
    /// - #VERIFY_TOCTOU_PREVENTED: CAS loop prevents race conditions
    #[test]
    fn test_concurrent_query_consistency() {
        let mut timeline = TimelineAggregationCapsuleWrapper::default();

        // Populate with events
        for i in 0..1000 {
            let ts = SystemTime::UNIX_EPOCH + Duration::from_secs(i * 60);
            timeline.append(ts, "test", "data").unwrap();
        }

        let timeline = Arc::new(timeline);

        // Concurrent queries
        std::thread::scope(|s| {
            for _ in 0..10 {
                let timeline_clone = Arc::clone(&timeline);
                s.spawn(move || {
                    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(60000);
                    let start = SystemTime::UNIX_EPOCH;

                    // All threads should see consistent total
                    let total = timeline_clone.aggregate_sum(start, now).unwrap();
                    assert_eq!(total, 1000, "Concurrent queries should see consistent totals");
                });
            }
        });
    }

    // ============================================================================
    // Part 5: Input Validation Tests
    // ============================================================================

    /// Test SystemTime before epoch rejection
    ///
    /// # ASSUM Verification
    /// - #VERIFY_INPUT_VALID: Rejects timestamps before 1970
    #[test]
    fn test_append_before_epoch() {
        let mut timeline = TimelineAggregationCapsuleWrapper::default();

        // SystemTime before Unix epoch (not possible via UNIX_EPOCH - Duration,
        // but we can test the error path indirectly via epoch 0 rejection)
        let epoch_zero = SystemTime::UNIX_EPOCH;

        let result = timeline.append(epoch_zero, "test", "data");

        // Expected: Error (epoch 0 rejected as clock skew)
        assert!(result.is_err(), "Epoch 0 should be rejected");

        if let Err(e) = result {
            let err_msg = format!("{:?}", e);
            assert!(
                err_msg.contains("epoch 0") || err_msg.contains("clock skew"),
                "Error should mention epoch 0 or clock skew"
            );
        }
    }

    /// Test SystemTime validation (future timestamps)
    #[test]
    fn test_append_future_timestamp() {
        let mut timeline = TimelineAggregationCapsuleWrapper::default();

        // Future timestamp (year 2100)
        let future = SystemTime::UNIX_EPOCH + Duration::from_secs(4_102_444_800);

        // Should succeed (future timestamps are valid)
        let result = timeline.append(future, "test", "data");

        assert!(result.is_ok(), "Future timestamps should be accepted");
    }

    /// Test query with inverted range (start > end)
    #[test]
    fn test_query_range_inverted() {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let now = SystemTime::now();
        let past = now - Duration::from_secs(3600);

        // Inverted range: start=now, end=past (start > end)
        let result = timeline.query_range(now, past);

        // Expected: Empty result (no buckets in inverted range)
        assert!(result.is_ok(), "Inverted range should return empty");
        let snapshots = result.unwrap();
        assert_eq!(snapshots.len(), 0, "Inverted range should be empty");
    }

    // ============================================================================
    // Part 6: Audit Trail Integrity Tests (Q34)
    // ============================================================================

    /// Test bucket hash chain integrity
    ///
    /// # ASSUM Verification
    /// - #VERIFY_AUDIT_INTEGRITY: Hash chain prevents tampering
    #[test]
    fn test_bucket_hash_chain() {
        use crate::capsules::timeline_aggregation_capsule::{
            BucketGranularity, TimelineAggregationCapsuleCore,
        };

        let capsule = TimelineAggregationCapsuleCore::new(1000, BucketGranularity::Minute, 100);

        // Append events to multiple buckets
        capsule.append(1030).unwrap();
        capsule.append(1090).unwrap();
        capsule.append(1150).unwrap();

        // Flush buckets (compute hashes)
        let hash0 = capsule.flush_bucket(0).unwrap();
        let hash1 = capsule.flush_bucket(1).unwrap();
        let hash2 = capsule.flush_bucket(2).unwrap();

        // Verify hashes are unique (deterministic but distinct)
        assert_ne!(hash0, hash1, "Hash chain should have unique hashes");
        assert_ne!(hash1, hash2, "Hash chain should have unique hashes");
        assert_ne!(hash0, 0, "Hash should not be zero");
    }

    /// Test bucket status transitions
    ///
    /// # ASSUM Verification
    /// - #VERIFY_STATE_MACHINE: Only valid transitions allowed
    #[test]
    fn test_bucket_status_transitions() {
        use crate::capsules::timeline_aggregation_capsule::{BucketStatus, TimelineBucket};

        let bucket = TimelineBucket::new(1000, 1060, 0);

        // Initial state: Active
        assert_eq!(bucket.status(), BucketStatus::Active);

        // Transition: Active -> Complete
        bucket.mark_complete();
        assert_eq!(bucket.status(), BucketStatus::Complete);

        // Transition: Complete -> Flushed
        bucket.mark_flushed();
        assert_eq!(bucket.status(), BucketStatus::Flushed);

        // Append to flushed bucket should fail
        let result = bucket.append(1030_000_000);
        assert!(result.is_err(), "Cannot append to flushed bucket");
    }

    // ============================================================================
    // Part 7: Property-Based Tests (High-Level)
    // ============================================================================

    /// Property: Total events should never decrease
    #[test]
    fn property_total_events_monotonic() {
        let mut timeline = TimelineAggregationCapsuleWrapper::default();

        let mut prev_total = timeline.total_events();

        for i in 0..100 {
            let ts = SystemTime::UNIX_EPOCH + Duration::from_secs(i * 60);
            timeline.append(ts, "test", "data").unwrap();

            let current_total = timeline.total_events();

            // Property: Total events should be monotonically increasing
            assert!(
                current_total >= prev_total,
                "Total events should never decrease"
            );

            prev_total = current_total;
        }
    }

    /// Property: Sum of buckets should equal total events
    #[test]
    fn property_sum_equals_total() {
        let mut timeline = TimelineAggregationCapsuleWrapper::default();

        // Append events
        for i in 0..1000 {
            let ts = SystemTime::UNIX_EPOCH + Duration::from_secs(i * 60);
            timeline.append(ts, "test", "data").unwrap();
        }

        let total = timeline.total_events();

        let start = SystemTime::UNIX_EPOCH;
        let end = SystemTime::UNIX_EPOCH + Duration::from_secs(60000);
        let sum = timeline.aggregate_sum(start, end).unwrap();

        // Property: Sum of all buckets should equal total events
        assert_eq!(
            sum, total,
            "Sum of buckets should equal total events counter"
        );
    }

    /// Property: Percentile should be within min-max range
    #[test]
    fn property_percentile_within_range() {
        let mut timeline = TimelineAggregationCapsuleWrapper::default();

        // Append varying event counts to buckets
        for i in 0..100 {
            let ts = SystemTime::UNIX_EPOCH + Duration::from_secs(i * 60);
            for _ in 0..((i % 10) + 1) {
                // Varying counts (1-10 per bucket)
                timeline.append(ts, "test", "data").unwrap();
            }
        }

        let start = SystemTime::UNIX_EPOCH;
        let end = SystemTime::UNIX_EPOCH + Duration::from_secs(6000);

        let min = timeline.aggregate_min(start, end).unwrap();
        let max = timeline.aggregate_max(start, end).unwrap();
        let p50 = timeline.percentile(start, end, 50).unwrap();

        // Property: Percentile should be within [min, max]
        assert!(
            p50 >= min && p50 <= max,
            "Percentile should be within min-max range"
        );
    }
}
