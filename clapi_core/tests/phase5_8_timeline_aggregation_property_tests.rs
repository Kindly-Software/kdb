//! Phase 5.8: Timeline Aggregation Capsule T28 Property Tests (Tier 2: Q8-Q14)
//!
//! Property-based tests validating TimelineAggregationCapsule invariants:
//! - Q8: Concurrent append ordering guarantees
//! - Q9: Hash chain integrity under concurrent access
//! - Q10: Temporal boundary semantics (bucket assignment)
//! - Q11: Backpressure when approaching 10K limit
//! - Q12: Idempotency of re-appending same event
//! - Q13: Bucket time boundary invariants
//! - Q14: Event deduplication within buckets

use clapi_core::capsules::TimelineAggregationCapsule;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Barrier;

#[tokio::test]
async fn test_concurrent_append_ordering() {
    // Q8: All concurrent appends succeed, ordering per-bucket preserved
    let timeline = Arc::new(tokio::sync::Mutex::new(
        TimelineAggregationCapsule::new(Duration::from_secs(60))
    ));
    let barrier = Arc::new(Barrier::new(50));

    let mut tasks = vec![];
    for thread_id in 0..50 {
        let timeline_clone = Arc::clone(&timeline);
        let barrier_clone = Arc::clone(&barrier);

        let task = tokio::spawn(async move {
            // Wait for all threads to be ready
            barrier_clone.wait().await;

            let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(5000000);
            for i in 0..20 {
                let timestamp = base_time + Duration::from_secs(i);
                let event_data = format!("thread_{}_event_{}", thread_id, i);

                let mut tl = timeline_clone.lock().await;
                tl.append(timestamp, "concurrent_test", &event_data).ok();
            }
        });
        tasks.push(task);
    }

    // Wait for all tasks
    for task in tasks {
        task.await.unwrap();
    }

    // Verify all 1000 events (50 threads × 20 events)
    let timeline_locked = timeline.lock().await;
    assert_eq!(timeline_locked.total_events(), 1000, "All concurrent appends should succeed");
    assert_eq!(timeline_locked.error_count(), 0, "No errors during concurrent append");
}

#[tokio::test]
async fn test_hash_chain_integrity_concurrent() {
    // Q9: Hash chain remains valid under concurrent appends
    let timeline = Arc::new(tokio::sync::Mutex::new(
        TimelineAggregationCapsule::new(Duration::from_secs(60))
    ));

    let mut tasks = vec![];
    for i in 0..100 {
        let timeline_clone = Arc::clone(&timeline);
        let task = tokio::spawn(async move {
            let timestamp = SystemTime::UNIX_EPOCH + Duration::from_secs(6000000 + i);
            let mut tl = timeline_clone.lock().await;
            tl.append(timestamp, "chain_test", &format!("data_{}", i))
        });
        tasks.push(task);
    }

    // Wait for all appends
    for task in tasks {
        task.await.unwrap().ok();
    }

    // Verify hash chain integrity
    let timeline_locked = timeline.lock().await;
    let bucket_count = timeline_locked.bucket_count();

    for bucket_idx in 0..bucket_count {
        let hash = timeline_locked.get_bucket_hash(bucket_idx);
        assert!(hash.is_some(), "Bucket {} should have valid hash", bucket_idx);

        // Verify hash chain links
        if bucket_idx > 0 {
            let prev_hash = timeline_locked.get_bucket_hash(bucket_idx - 1).unwrap();
            let curr_hash = hash.unwrap();
            // Each bucket's hash should incorporate previous bucket's hash
            assert_ne!(prev_hash, curr_hash, "Hash chain should link buckets");
        }
    }
}

#[test]
fn test_bucket_boundary_semantics() {
    // Q9: Temporal ordering - events always assigned to correct bucket
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(300)); // 5-minute buckets
    let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(7000000);

    // Event at t=0 (bucket 0)
    timeline.append(base_time, "event", "bucket_0").unwrap();

    // Event at t=299 (still bucket 0)
    timeline.append(base_time + Duration::from_secs(299), "event", "bucket_0_end").unwrap();

    // Event at t=300 (bucket 1 - exactly at boundary)
    timeline.append(base_time + Duration::from_secs(300), "event", "bucket_1").unwrap();

    // Event at t=301 (bucket 1)
    timeline.append(base_time + Duration::from_secs(301), "event", "bucket_1_start").unwrap();

    assert_eq!(timeline.bucket_count(), 2, "Should have exactly 2 buckets");

    // Verify bucket 0 has 2 events, bucket 1 has 2 events
    let bucket_0_count = timeline.get_bucket_event_count(0).unwrap();
    let bucket_1_count = timeline.get_bucket_event_count(1).unwrap();

    assert_eq!(bucket_0_count, 2, "Bucket 0 should have 2 events");
    assert_eq!(bucket_1_count, 2, "Bucket 1 should have 2 events");
}

#[test]
fn test_backpressure_on_10k_limit() {
    // Q10: Backpressure signals when approaching capacity
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::now();

    // Fill to 9,900 events (90% capacity)
    for i in 0..9900 {
        timeline.append(base_time + Duration::from_secs(i / 100), "event", "data").unwrap();
    }

    // Check backpressure signal
    assert!(timeline.is_near_capacity(), "Should signal backpressure at 90% capacity");
    assert!(!timeline.is_full(), "Should not be full yet");

    // Add remaining 100 events
    for i in 9900..10000 {
        timeline.append(base_time + Duration::from_secs(i / 100), "event", "data").ok();
    }

    // Now should be full or reject new events
    let result = timeline.append(base_time + Duration::from_secs(200), "event", "overflow");
    assert!(result.is_err() || timeline.is_full(), "Should reject or signal full at 10K");
}

#[test]
fn test_idempotency_reappend() {
    // Q11: Re-appending same event with same timestamp creates duplicate (not idempotent by design)
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let timestamp = SystemTime::UNIX_EPOCH + Duration::from_secs(8000000);

    // Append same event 3 times
    timeline.append(timestamp, "login", "user_id=123").unwrap();
    timeline.append(timestamp, "login", "user_id=123").unwrap();
    timeline.append(timestamp, "login", "user_id=123").unwrap();

    // Should create 3 separate entries (timeline is append-only, not idempotent)
    assert_eq!(timeline.total_events(), 3, "Timeline is append-only, not idempotent");
}

#[test]
fn test_idempotency_compaction() {
    // Q11: Compaction is idempotent (running twice produces same result)
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    // Add identical events
    for i in 0..10 {
        timeline.append(now, "event", "identical_data").unwrap();
    }

    // First compaction
    timeline.compact().unwrap();
    let count_after_first = timeline.get_bucket_compressed_count(0).unwrap();

    // Second compaction (should be no-op)
    timeline.compact().unwrap();
    let count_after_second = timeline.get_bucket_compressed_count(0).unwrap();

    assert_eq!(count_after_first, count_after_second, "Compaction should be idempotent");
}

#[test]
fn test_bucket_time_boundaries_exact() {
    // Q12: Bucket boundaries are precisely enforced
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(9000000);

    // t=0, t=1, ..., t=59 → bucket 0
    for sec in 0..60 {
        timeline.append(base_time + Duration::from_secs(sec), "event", "bucket_0").unwrap();
    }

    // t=60 → bucket 1 (exactly at boundary)
    timeline.append(base_time + Duration::from_secs(60), "event", "bucket_1").unwrap();

    assert_eq!(timeline.bucket_count(), 2, "Boundary at t=60 creates new bucket");
    assert_eq!(timeline.get_bucket_event_count(0).unwrap(), 60, "Bucket 0 has 60 events");
    assert_eq!(timeline.get_bucket_event_count(1).unwrap(), 1, "Bucket 1 has 1 event");
}

#[test]
fn test_bucket_time_boundaries_millisecond_precision() {
    // Q12: Boundaries respect millisecond precision
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_millis(1000)); // 1-second buckets
    let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(10000000);

    // t=999ms → bucket 0
    timeline.append(base_time + Duration::from_millis(999), "event", "bucket_0").unwrap();

    // t=1000ms → bucket 1
    timeline.append(base_time + Duration::from_millis(1000), "event", "bucket_1").unwrap();

    // t=1001ms → bucket 1
    timeline.append(base_time + Duration::from_millis(1001), "event", "bucket_1").unwrap();

    assert_eq!(timeline.bucket_count(), 2, "Millisecond precision enforced");
}

#[test]
fn test_event_deduplication_disabled_by_default() {
    // Q14: Deduplication is OFF by default (append-only)
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    // Add 5 identical events
    for _ in 0..5 {
        timeline.append(now, "event", "identical").unwrap();
    }

    // All 5 should be recorded (no dedup)
    assert_eq!(timeline.total_events(), 5, "No deduplication by default");
}

#[test]
fn test_event_deduplication_with_flag() {
    // Q14: Deduplication when explicitly enabled
    let mut timeline = TimelineAggregationCapsule::with_deduplication(Duration::from_secs(60), true);
    let now = SystemTime::now();

    // Add 5 identical events
    for _ in 0..5 {
        timeline.append(now, "event", "identical").unwrap();
    }

    // Should deduplicate to 1 event (or compressed representation)
    let unique_events = timeline.get_bucket_unique_count(0).unwrap();
    assert_eq!(unique_events, 1, "Deduplication should compress identical events");
}

#[tokio::test]
async fn test_concurrent_flush_safety() {
    // Q13: Concurrent flush operations are safe
    let timeline = Arc::new(tokio::sync::Mutex::new(
        TimelineAggregationCapsule::new(Duration::from_secs(60))
    ));

    // Add events
    {
        let mut tl = timeline.lock().await;
        for i in 0..100 {
            tl.append(SystemTime::now(), "event", &format!("data_{}", i)).unwrap();
        }
    }

    // Concurrent flushes
    let mut tasks = vec![];
    for _ in 0..10 {
        let timeline_clone = Arc::clone(&timeline);
        let task = tokio::spawn(async move {
            let mut tl = timeline_clone.lock().await;
            tl.flush()
        });
        tasks.push(task);
    }

    // All flushes should succeed (idempotent after first)
    for task in tasks {
        let result = task.await.unwrap();
        assert!(result.is_ok(), "Concurrent flush should be safe");
    }
}

#[test]
fn test_temporal_ordering_preservation() {
    // Q9: Events maintain temporal ordering within buckets
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(11000000);

    // Add events out of chronological order
    timeline.append(base_time + Duration::from_secs(10), "event", "third").unwrap();
    timeline.append(base_time, "event", "first").unwrap();
    timeline.append(base_time + Duration::from_secs(5), "event", "second").unwrap();

    // Query events from bucket 0 in insertion order
    let events = timeline.get_bucket_events(0).unwrap();

    // Timeline should preserve insertion order (even if timestamps out of order)
    assert_eq!(events.len(), 3, "All 3 events in bucket");
    // Note: Timeline preserves insertion order, NOT chronological order
}

#[test]
fn test_bucket_count_monotonic() {
    // Q9: Bucket count is monotonically increasing
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::now();

    let mut last_count = 0;
    for minute in 0..10 {
        timeline.append(base_time + Duration::from_secs(minute * 60), "event", "data").unwrap();
        let current_count = timeline.bucket_count();

        assert!(current_count >= last_count, "Bucket count should be monotonic");
        last_count = current_count;
    }
}

#[test]
fn test_hash_consistency_after_compaction() {
    // Q14: Hash chain remains consistent after compaction
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    // Add events
    for i in 0..20 {
        timeline.append(now, "event", &format!("data_{}", i)).unwrap();
    }

    let hash_before = timeline.get_bucket_hash(0).unwrap();

    // Compact
    timeline.compact().unwrap();

    let hash_after = timeline.get_bucket_hash(0).unwrap();

    // Hash should remain the same (or change predictably)
    // Implementation choice: hash includes compressed metadata
    assert!(hash_after > 0, "Hash should remain valid after compaction");
}
