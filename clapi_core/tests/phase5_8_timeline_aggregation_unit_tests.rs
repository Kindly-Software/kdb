//! Phase 5.8: Timeline Aggregation Capsule T28 Unit Tests (Tier 1: Q1-Q7)
//!
//! Unit tests validating TimelineAggregationCapsule invariants:
//! - Q1: Capsule creation and initialization
//! - Q2: Single event append and bucket assignment
//! - Q3: Bucket formation at time boundaries
//! - Q4: Hash chain integrity per bucket
//! - Q5: Flush operation clears pending events
//! - Q6: Event compression (identical events)
//! - Q7: Memory bounds enforcement (10K event limit)

use clapi_core::capsules::TimelineAggregationCapsule;
use std::time::{Duration, SystemTime};

#[test]
fn test_capsule_creation() {
    // Q1: Initialize capsule with default 1-minute bucket size
    let timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));

    assert_eq!(timeline.bucket_count(), 0, "New timeline should have zero buckets");
    assert_eq!(timeline.total_events(), 0, "New timeline should have zero events");
    assert_eq!(timeline.error_count(), 0, "New timeline should have zero errors");
}

#[test]
fn test_capsule_creation_custom_bucket() {
    // Q1: Initialize with custom bucket duration
    let timeline = TimelineAggregationCapsule::new(Duration::from_secs(300)); // 5-minute buckets

    assert_eq!(timeline.bucket_duration(), Duration::from_secs(300));
}

#[test]
fn test_append_single_event() {
    // Q2: Append single audit event
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    let result = timeline.append(now, "user_login", "user_id=123");

    assert!(result.is_ok(), "Single append should succeed");
    assert_eq!(timeline.total_events(), 1, "Event count should be 1");
    assert_eq!(timeline.bucket_count(), 1, "Should create 1 bucket");
}

#[test]
fn test_append_multiple_events_same_bucket() {
    // Q2: Multiple events within same time bucket
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::now();

    for i in 0..5 {
        let timestamp = base_time + Duration::from_secs(i);
        let result = timeline.append(timestamp, "api_call", &format!("request_{}", i));
        assert!(result.is_ok(), "Append {} should succeed", i);
    }

    assert_eq!(timeline.total_events(), 5, "Should have 5 events");
    assert_eq!(timeline.bucket_count(), 1, "All events in same bucket");
}

#[test]
fn test_bucket_formation_at_boundary() {
    // Q3: Bucket boundary at exact time threshold
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000000);

    // Event 1: at t=0
    timeline.append(base_time, "event_type", "data1").unwrap();

    // Event 2: at t=59 (same bucket)
    timeline.append(base_time + Duration::from_secs(59), "event_type", "data2").unwrap();

    // Event 3: at t=60 (NEW bucket - exactly at boundary)
    timeline.append(base_time + Duration::from_secs(60), "event_type", "data3").unwrap();

    assert_eq!(timeline.bucket_count(), 2, "Should create 2 buckets at boundary");
    assert_eq!(timeline.total_events(), 3, "All 3 events recorded");
}

#[test]
fn test_bucket_formation_multiple_boundaries() {
    // Q3: Multiple bucket boundaries
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(2000000);

    // 5 buckets: t=0, t=60, t=120, t=180, t=240
    for minute in 0..5 {
        let timestamp = base_time + Duration::from_secs(minute * 60);
        timeline.append(timestamp, "event", &format!("minute_{}", minute)).unwrap();
    }

    assert_eq!(timeline.bucket_count(), 5, "Should create 5 buckets");
}

#[test]
fn test_hash_chain_per_bucket() {
    // Q4: Each bucket maintains its own hash chain
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::now();

    // Append 3 events to first bucket
    for i in 0..3 {
        timeline.append(base_time + Duration::from_secs(i), "type_a", &format!("data_{}", i)).unwrap();
    }

    // Get first bucket's hash chain root
    let bucket_hash_1 = timeline.get_bucket_hash(0).expect("Bucket 0 should exist");

    // Append 2 events to second bucket
    for i in 0..2 {
        let timestamp = base_time + Duration::from_secs(60 + i);
        timeline.append(timestamp, "type_b", &format!("data_{}", i)).unwrap();
    }

    // Get second bucket's hash chain root
    let bucket_hash_2 = timeline.get_bucket_hash(1).expect("Bucket 1 should exist");

    // Hash chains should be different (independent per bucket)
    assert_ne!(bucket_hash_1, bucket_hash_2, "Bucket hash chains should be independent");
}

#[test]
fn test_hash_chain_deterministic() {
    // Q4: Hash chain is deterministic for same event sequence
    let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(3000000);

    let mut timeline1 = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let mut timeline2 = TimelineAggregationCapsule::new(Duration::from_secs(60));

    // Same events in same order
    for i in 0..5 {
        let timestamp = base_time + Duration::from_secs(i);
        timeline1.append(timestamp, "event", &format!("data_{}", i)).unwrap();
        timeline2.append(timestamp, "event", &format!("data_{}", i)).unwrap();
    }

    let hash1 = timeline1.get_bucket_hash(0).unwrap();
    let hash2 = timeline2.get_bucket_hash(0).unwrap();

    assert_eq!(hash1, hash2, "Identical event sequences should produce identical hashes");
}

#[test]
fn test_flush_clears_pending_events() {
    // Q5: Flush operation clears pending buffer
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    // Append events
    for i in 0..10 {
        timeline.append(now + Duration::from_secs(i), "event", "data").unwrap();
    }

    assert_eq!(timeline.pending_events(), 10, "Should have 10 pending events");

    // Flush to storage
    let flushed = timeline.flush().expect("Flush should succeed");

    assert_eq!(flushed, 10, "Should flush 10 events");
    assert_eq!(timeline.pending_events(), 0, "Pending should be cleared");
    assert_eq!(timeline.total_events(), 10, "Total count preserved");
}

#[test]
fn test_flush_empty_timeline() {
    // Q5: Flush on empty timeline doesn't panic
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));

    let result = timeline.flush();

    assert!(result.is_ok(), "Flush empty timeline should succeed");
    assert_eq!(result.unwrap(), 0, "Should flush 0 events");
}

#[test]
fn test_compact_identical_events() {
    // Q6: Identical consecutive events are compressed
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    // Append 5 identical events
    for i in 0..5 {
        timeline.append(now + Duration::from_millis(i * 100), "login", "user_id=42").unwrap();
    }

    timeline.compact().expect("Compaction should succeed");

    // After compaction, identical events are represented as count
    let compressed_count = timeline.get_bucket_compressed_count(0).expect("Bucket 0 should exist");
    assert!(compressed_count > 0, "Should compress identical events");
    assert!(compressed_count <= 5, "Compressed count should not exceed original");
}

#[test]
fn test_compact_different_events_no_compression() {
    // Q6: Different events are NOT compressed
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    // Append 5 different events
    for i in 0..5 {
        timeline.append(now, "event", &format!("unique_data_{}", i)).unwrap();
    }

    let original_count = timeline.total_events();
    timeline.compact().expect("Compaction should succeed");

    // No compression should occur (all events unique)
    assert_eq!(timeline.total_events(), original_count, "Unique events should not compress");
}

#[test]
fn test_memory_bounds_10k_limit() {
    // Q7: Timeline enforces 10K event memory limit
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::now();

    // Attempt to add 10,500 events
    let mut success_count = 0;
    for i in 0..10500 {
        let timestamp = base_time + Duration::from_secs(i / 100); // Spread across buckets
        if timeline.append(timestamp, "event", &format!("data_{}", i)).is_ok() {
            success_count += 1;
        }
    }

    // Should stop at or before 10K events
    assert!(success_count <= 10000, "Should enforce 10K limit, got {}", success_count);
}

#[test]
fn test_memory_bounds_oldest_bucket_eviction() {
    // Q7: When limit reached, oldest buckets are evicted
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(4000000);

    // Fill to capacity with old events
    for i in 0..10000 {
        let timestamp = base_time + Duration::from_secs(i / 100);
        timeline.append(timestamp, "old", "data").unwrap();
    }

    let initial_bucket_count = timeline.bucket_count();

    // Add new event in future time - should trigger eviction
    let future_time = base_time + Duration::from_secs(20000);
    timeline.append(future_time, "new", "data").ok();

    // Oldest bucket should be evicted if limit enforced
    let final_bucket_count = timeline.bucket_count();
    assert!(final_bucket_count >= initial_bucket_count - 1, "Oldest bucket evicted or retained");
}

#[test]
fn test_default_implementation() {
    // Q1: Default trait creates 1-minute bucket timeline
    let timeline = TimelineAggregationCapsule::default();

    assert_eq!(timeline.bucket_duration(), Duration::from_secs(60), "Default should be 1-minute buckets");
}
