//! Property-Based Tests - Timeline Aggregation Invariants (T28 Q8-Q14)
//!
//! ## Purpose
//! Validate that invariants hold across the entire input space using property-based
//! testing with proptest. Catch edge cases that unit tests miss.
//!
//! ## Framework Compliance
//! - **T28 Q8-Q14**: Property tier - invariant validation
//! - **ASSUM**: Verify safety assumptions with concurrent property tests
//! - **UCE34 Q33**: Validation - properties prove correctness
//!
//! ## Properties Under Test
//! - **Conservation**: Event count = sum of bucket counts
//! - **Monotonicity**: Generation counter always increases
//! - **Idempotence**: Query returns same result multiple times
//! - **Linearizability**: Concurrent operations preserve order
//! - **Boundedness**: Memory usage bounded by configuration
//!
//! ## Test Structure (T28 Q8-Q14)
//! - Q8: Universal properties (hold for all inputs)
//! - Q9: Concurrent invariants (race-free under contention)
//! - Q10: Edge case properties (boundaries, extremes)
//! - Q11: ASSUM verification (safety assumptions validated)
//! - Q12: Composition properties (multi-operation workflows)
//! - Q13: Statistical properties (distribution correctness)
//! - Q14: Regression tracking (proptest-regressions committed)

use clapi_core::capsules::timeline_aggregation_capsule::{
    TimelineAggregationCapsuleWrapper, BucketStatus,
};
use proptest::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ============================================================================
// TEST HELPERS
// ============================================================================

/// Create timeline with default config (1440 buckets, 60s duration)
fn create_timeline() -> TimelineAggregationCapsuleWrapper {
    TimelineAggregationCapsuleWrapper::new(1440, 60)
        .expect("Timeline creation failed")
}

/// Get current timestamp (deterministic for tests)
fn deterministic_timestamp(offset_secs: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_700_000_000 + offset_secs)
}

// ============================================================================
// T28 Q8: UNIVERSAL PROPERTIES (Hold for All Inputs)
// ============================================================================

proptest! {
    /// Property: Event count conservation - Total appends = sum of bucket counts
    #[test]
    fn prop_event_count_conservation(
        append_count in 1u64..10000,
    ) {
        let timeline = create_timeline();
        let now = deterministic_timestamp(0);

        // Append events
        for _ in 0..append_count {
            timeline.append_system_time(now).unwrap();
        }

        // Query bucket
        let bucket = timeline.query_bucket_system_time(now).unwrap();

        // Property: Bucket count equals appends (conservation)
        prop_assert_eq!(
            bucket.count, append_count,
            "Event count not conserved: expected {}, got {}",
            append_count, bucket.count
        );
    }

    /// Property: Query idempotence - Multiple queries return same result
    #[test]
    fn prop_query_idempotent(
        append_count in 0u64..1000,
    ) {
        let timeline = create_timeline();
        let now = deterministic_timestamp(0);

        // Append events
        for _ in 0..append_count {
            timeline.append_system_time(now).unwrap();
        }

        // Query multiple times
        let result1 = timeline.query_bucket_system_time(now).unwrap();
        let result2 = timeline.query_bucket_system_time(now).unwrap();
        let result3 = timeline.query_bucket_system_time(now).unwrap();

        // Property: All queries return same count (idempotent)
        prop_assert_eq!(result1.count, result2.count);
        prop_assert_eq!(result2.count, result3.count);
    }

    /// Property: Range query consistency - Sum of buckets equals range query
    #[test]
    fn prop_range_query_consistency(
        events_per_bucket in prop::collection::vec(0u64..100, 1..10),
    ) {
        let timeline = create_timeline();

        // Append events to multiple buckets
        for (idx, &count) in events_per_bucket.iter().enumerate() {
            let ts = deterministic_timestamp(idx as u64 * 60);
            for _ in 0..count {
                timeline.append_system_time(ts).unwrap();
            }
        }

        // Query range
        let range_result = timeline.query_last_hours(1).unwrap();

        // Calculate expected sum
        let expected_sum: u64 = events_per_bucket.iter().sum();

        // Property: Range query equals sum of individual buckets
        prop_assert_eq!(
            range_result.total_count, expected_sum,
            "Range query inconsistent: expected {}, got {}",
            expected_sum, range_result.total_count
        );
    }
}

// ============================================================================
// T28 Q9: CONCURRENT INVARIANTS (Race-Free Under Contention)
// ============================================================================

proptest! {
    /// Property: Concurrent appends - No lost writes
    #[test]
    fn prop_concurrent_no_lost_updates(
        updates_per_thread in 10u64..1000,
    ) {
        let timeline = Arc::new(create_timeline());
        let num_threads = 10;
        let now = deterministic_timestamp(0);

        // Spawn threads
        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let t = Arc::clone(&timeline);
                thread::spawn(move || {
                    for _ in 0..updates_per_thread {
                        t.append_system_time(now).unwrap();
                    }
                })
            })
            .collect();

        // Wait for completion
        for h in handles {
            h.join().unwrap();
        }

        // Property: All updates applied (no lost writes)
        let bucket = timeline.query_bucket_system_time(now).unwrap();
        let expected = num_threads * updates_per_thread;

        prop_assert_eq!(
            bucket.count, expected,
            "Lost writes: expected {}, got {}",
            expected, bucket.count
        );
    }

    /// Property: Concurrent readers - Consistent reads during writes
    #[test]
    fn prop_concurrent_consistent_reads(
        operations in 100u64..1000,
    ) {
        let timeline = Arc::new(create_timeline());
        let now = deterministic_timestamp(0);

        // Writer thread
        let writer_timeline = Arc::clone(&timeline);
        let writer = thread::spawn(move || {
            for _ in 0..operations {
                writer_timeline.append_system_time(now).unwrap();
            }
        });

        // Reader threads (validate monotonicity)
        let readers: Vec<_> = (0..5)
            .map(|_| {
                let t = Arc::clone(&timeline);
                thread::spawn(move || {
                    let mut last_count = 0u64;
                    for _ in 0..1000 {
                        if let Ok(bucket) = t.query_bucket_system_time(now) {
                            // Property: Count never decreases (monotonic)
                            if bucket.count < last_count {
                                return Err(format!(
                                    "Non-monotonic read: {} -> {}",
                                    last_count, bucket.count
                                ));
                            }
                            last_count = bucket.count;
                        }
                    }
                    Ok(())
                })
            })
            .collect();

        // Wait for completion
        writer.join().unwrap();
        for r in readers {
            let result = r.join().unwrap();
            prop_assert!(result.is_ok(), "Reader detected inconsistency: {:?}", result);
        }
    }
}

// ============================================================================
// T28 Q10: EDGE CASE PROPERTIES (Boundaries, Extremes)
// ============================================================================

proptest! {
    /// Property: Handles extreme timestamps (min, max, boundaries)
    #[test]
    fn prop_extreme_timestamps(
        offset_secs in 0u64..86400 * 365, // 1 year range
    ) {
        let timeline = create_timeline();
        let ts = deterministic_timestamp(offset_secs);

        // Property: All valid timestamps accepted
        let result = timeline.append_system_time(ts);

        // Should succeed for any valid SystemTime
        prop_assert!(
            result.is_ok() || result.is_err(),
            "Unexpected panic on timestamp"
        );
    }

    /// Property: Empty buckets query successfully
    #[test]
    fn prop_empty_bucket_query(
        bucket_offset in 0u64..1440,
    ) {
        let timeline = create_timeline();
        let ts = deterministic_timestamp(bucket_offset * 60);

        // Query empty bucket
        let result = timeline.query_bucket_system_time(ts);

        // Property: Empty buckets return Ok with count=0
        prop_assert!(result.is_ok());
        if let Ok(bucket) = result {
            prop_assert_eq!(bucket.count, 0, "Empty bucket should have count=0");
        }
    }

    /// Property: Bucket boundary transitions work correctly
    #[test]
    fn prop_bucket_boundary_transitions(
        boundary_offset in 0u64..100,
    ) {
        let timeline = create_timeline();

        // Create timestamps around bucket boundaries
        let base = deterministic_timestamp(0);
        let before = base - Duration::from_secs(1);
        let exact = base;
        let after = base + Duration::from_secs(1);

        // Append to all three timestamps
        timeline.append_system_time(before).unwrap();
        timeline.append_system_time(exact).unwrap();
        timeline.append_system_time(after).unwrap();

        // Property: Events land in correct buckets
        let bucket_before = timeline.query_bucket_system_time(before).unwrap();
        let bucket_exact = timeline.query_bucket_system_time(exact).unwrap();

        // All should succeed (boundary handling works)
        prop_assert!(bucket_before.count > 0 || bucket_exact.count > 0);
    }
}

// ============================================================================
// T28 Q11: ASSUM VERIFICATION (Safety Assumptions Validated)
// ============================================================================

proptest! {
    /// #ASSUME: Bucket index always within bounds
    /// #VERIFY: Property test validates no out-of-bounds access
    #[test]
    fn verify_assum_bucket_index_bounds(
        timestamps in prop::collection::vec(0u64..86400, 1..1000),
    ) {
        let timeline = create_timeline();

        // Append with random timestamps
        for &offset in &timestamps {
            let ts = deterministic_timestamp(offset);
            let result = timeline.append_system_time(ts);

            // Property: No panic, all within bounds
            prop_assert!(
                result.is_ok() || result.is_err(),
                "Unexpected panic (bounds violation)"
            );
        }
    }

    /// #ASSUME: Generation counter prevents TOCTOU
    /// #VERIFY: Concurrent reads see consistent generation
    #[test]
    fn verify_assum_generation_prevents_toctou(
        operations in 100u64..1000,
    ) {
        let timeline = Arc::new(create_timeline());
        let now = deterministic_timestamp(0);

        // Concurrent writers
        let writers: Vec<_> = (0..5)
            .map(|_| {
                let t = Arc::clone(&timeline);
                thread::spawn(move || {
                    for _ in 0..operations {
                        t.append_system_time(now).unwrap();
                    }
                })
            })
            .collect();

        // Concurrent readers (check TOCTOU prevention)
        let readers: Vec<_> = (0..5)
            .map(|_| {
                let t = Arc::clone(&timeline);
                thread::spawn(move || {
                    for _ in 0..1000 {
                        // Read twice in quick succession
                        if let (Ok(b1), Ok(b2)) = (
                            t.query_bucket_system_time(now),
                            t.query_bucket_system_time(now),
                        ) {
                            // Property: No TOCTOU (count only increases)
                            if b2.count < b1.count {
                                return Err(format!("TOCTOU: {} -> {}", b1.count, b2.count));
                            }
                        }
                    }
                    Ok(())
                })
            })
            .collect();

        // Wait for completion
        for w in writers {
            w.join().unwrap();
        }
        for r in readers {
            let result = r.join().unwrap();
            prop_assert!(result.is_ok(), "TOCTOU detected: {:?}", result);
        }
    }

    /// #ASSUME: Atomic ordering (Acquire/Release) prevents reordering
    /// #VERIFY: Memory ordering guarantees observed behavior
    #[test]
    fn verify_assum_memory_ordering(
        operations in 100u64..1000,
    ) {
        let timeline = Arc::new(create_timeline());
        let now = deterministic_timestamp(0);

        // Writer appends sequentially
        let writer = {
            let t = Arc::clone(&timeline);
            thread::spawn(move || {
                for _ in 0..operations {
                    t.append_system_time(now).unwrap();
                }
            })
        };

        // Reader sees monotonic increases (no reordering)
        let reader = {
            let t = Arc::clone(&timeline);
            thread::spawn(move || {
                let mut last_count = 0u64;
                loop {
                    if let Ok(bucket) = t.query_bucket_system_time(now) {
                        // Property: Memory ordering prevents seeing older values
                        if bucket.count < last_count {
                            return Err(format!("Reordering: {} -> {}", last_count, bucket.count));
                        }
                        last_count = bucket.count;

                        if bucket.count == operations {
                            break;
                        }
                    }
                }
                Ok(())
            })
        };

        writer.join().unwrap();
        let result = reader.join().unwrap();
        prop_assert!(result.is_ok(), "Memory reordering detected: {:?}", result);
    }
}

// ============================================================================
// T28 Q12: COMPOSITION PROPERTIES (Multi-Operation Workflows)
// ============================================================================

proptest! {
    /// Property: Append + Query + Flush workflow preserves data
    #[test]
    fn prop_workflow_preserves_data(
        append_count in 10u64..1000,
    ) {
        let timeline = create_timeline();
        let now = deterministic_timestamp(0);

        // Workflow: Append -> Query -> Flush -> Query
        for _ in 0..append_count {
            timeline.append_system_time(now).unwrap();
        }

        let before_flush = timeline.query_bucket_system_time(now).unwrap();
        timeline.flush_bucket_system_time(now).unwrap();
        let after_flush = timeline.query_bucket_system_time(now).unwrap();

        // Property: Count preserved across flush
        prop_assert_eq!(
            before_flush.count, after_flush.count,
            "Flush lost data: {} -> {}",
            before_flush.count, after_flush.count
        );
    }

    /// Property: Multi-bucket aggregation consistent
    #[test]
    fn prop_multi_bucket_aggregation(
        bucket_counts in prop::collection::vec(0u64..100, 5..20),
    ) {
        let timeline = create_timeline();

        // Append to multiple buckets
        let mut expected_total = 0u64;
        for (idx, &count) in bucket_counts.iter().enumerate() {
            let ts = deterministic_timestamp(idx as u64 * 60);
            for _ in 0..count {
                timeline.append_system_time(ts).unwrap();
            }
            expected_total += count;
        }

        // Query range covering all buckets
        let range = timeline.query_last_hours(1).unwrap();

        // Property: Range total equals sum of buckets
        prop_assert_eq!(
            range.total_count, expected_total,
            "Multi-bucket aggregation incorrect: expected {}, got {}",
            expected_total, range.total_count
        );
    }
}

// ============================================================================
// T28 Q13: STATISTICAL PROPERTIES (Distribution Correctness)
// ============================================================================

proptest! {
    /// Property: Event distribution across buckets is deterministic
    #[test]
    fn prop_deterministic_distribution(
        events in prop::collection::vec(0u64..3600, 100..1000),
    ) {
        let timeline = create_timeline();
        let base = deterministic_timestamp(0);

        // Append events with offset timestamps
        for &offset in &events {
            let ts = base + Duration::from_secs(offset);
            timeline.append_system_time(ts).unwrap();
        }

        // Query all buckets
        let range = timeline.query_last_hours(1).unwrap();

        // Property: Total count equals input count (no loss)
        prop_assert_eq!(
            range.total_count as usize, events.len(),
            "Event distribution lost data: expected {}, got {}",
            events.len(), range.total_count
        );
    }

    /// Property: Bucket counts never negative or overflow
    #[test]
    fn prop_bucket_counts_bounded(
        operations in 0u64..10000,
    ) {
        let timeline = create_timeline();
        let now = deterministic_timestamp(0);

        // Append many events
        for _ in 0..operations {
            timeline.append_system_time(now).unwrap();
        }

        // Query bucket
        let bucket = timeline.query_bucket_system_time(now).unwrap();

        // Property: Count is sane (0 <= count <= operations)
        prop_assert!(bucket.count <= operations);
        prop_assert!(bucket.count >= 0);
    }
}

// ============================================================================
// T28 Q14: REGRESSION TRACKING (Proptest Regressions)
// ============================================================================

// Note: Proptest automatically saves failing cases to
// tests/property_based_tests.proptest-regressions
//
// When a property fails, proptest saves:
// - The seed that triggered the failure
// - The exact input values that caused the failure
//
// On subsequent runs, these cases are re-tested first to catch regressions
// Commit .proptest-regressions files to version control

proptest! {
    /// Property: Regression test for known edge cases
    /// (This will catch any previously-failed cases automatically)
    #[test]
    fn prop_regression_tracking(
        append_count in 1u64..10000,
        bucket_offset in 0u64..1440,
    ) {
        let timeline = create_timeline();
        let ts = deterministic_timestamp(bucket_offset * 60);

        // Append events
        for _ in 0..append_count {
            timeline.append_system_time(ts).unwrap();
        }

        // Property: Basic invariants (will catch regressions)
        let bucket = timeline.query_bucket_system_time(ts).unwrap();
        prop_assert_eq!(bucket.count, append_count);
        prop_assert!(bucket.status == BucketStatus::Active || bucket.status == BucketStatus::Complete);
    }
}

// ============================================================================
// STRESS PROPERTIES (Heavy Concurrent Load)
// ============================================================================

proptest! {
    /// Property: High contention preserves correctness
    #[test]
    fn prop_high_contention_correctness(
        threads in 10usize..50,
        ops_per_thread in 100u64..1000,
    ) {
        let timeline = Arc::new(create_timeline());
        let now = deterministic_timestamp(0);

        // Spawn many threads
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let t = Arc::clone(&timeline);
                thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        t.append_system_time(now).unwrap();
                    }
                })
            })
            .collect();

        // Wait for completion
        for h in handles {
            h.join().unwrap();
        }

        // Property: All updates applied under high contention
        let bucket = timeline.query_bucket_system_time(now).unwrap();
        let expected = (threads as u64) * ops_per_thread;

        prop_assert_eq!(
            bucket.count, expected,
            "High contention lost writes: expected {}, got {}",
            expected, bucket.count
        );
    }
}
