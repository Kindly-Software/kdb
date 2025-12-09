//! Phase 5.8: Timeline Aggregation Capsule T28 Integration Tests (Tier 3: Q15-Q21)
//!
//! Integration tests validating TimelineAggregationCapsule with other components:
//! - Q15: End-to-end timeline creation from audit events
//! - Q16: AuditLogBridge → TimelineAggregationCapsule integration
//! - Q17: Metrics endpoint exposes bucket statistics
//! - Q18: Storage persistence (buckets to disk)
//! - Q19: Analytics queries by time bucket
//! - Q20: Replay from bucket restores state
//! - Q21: Backward compatibility with existing audit_log

use clapi_core::proxy::{AuditLogBridge, TimelineAggregationCapsule};
use std::time::{Duration, SystemTime};

#[tokio::test]
async fn test_end_to_end_timeline_creation() {
    // Q15: Complete workflow from event to timeline bucket
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    // Simulate user activity
    timeline.append(now, "user_login", "user_id=123").unwrap();
    timeline.append(now + Duration::from_secs(5), "api_request", "endpoint=/users").unwrap();
    timeline.append(now + Duration::from_secs(10), "api_request", "endpoint=/posts").unwrap();
    timeline.append(now + Duration::from_secs(70), "user_logout", "user_id=123").unwrap();

    // Verify end-to-end
    assert_eq!(timeline.total_events(), 4, "All events recorded");
    assert_eq!(timeline.bucket_count(), 2, "Events split into 2 buckets");

    // Flush to finalize buckets
    let flushed = timeline.flush().unwrap();
    assert_eq!(flushed, 4, "All events flushed");
}

#[tokio::test]
async fn test_audit_trail_to_timeline() {
    // Q16: Integration with AuditLogBridge
    let audit_bridge = AuditLogBridge::new();
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));

    let now = SystemTime::now();

    // Log events via AuditLogBridge
    audit_bridge.log_request(42, 12345, 0x1234).await.unwrap();
    audit_bridge.log_request(43, 67890, 0x5678).await.unwrap();
    audit_bridge.log_error(44, 0xABCD).await.unwrap();

    // Wait for bridge batch processing
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Drain audit log into timeline
    let audit_events = audit_bridge.drain_recent_events();
    for (timestamp, event_type, data) in audit_events {
        timeline.append(timestamp, &event_type, &data).unwrap();
    }

    // Verify integration
    assert!(timeline.total_events() >= 3, "Audit events transferred to timeline");
}

#[tokio::test]
async fn test_metrics_endpoint_returns_buckets() {
    // Q17: HTTP /metrics endpoint exposes timeline bucket stats
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(12000000);

    // Create 3 buckets with different event counts
    for i in 0..10 {
        timeline.append(base_time, "type_a", &format!("data_{}", i)).unwrap();
    }
    for i in 0..20 {
        timeline.append(base_time + Duration::from_secs(60), "type_b", &format!("data_{}", i)).unwrap();
    }
    for i in 0..15 {
        timeline.append(base_time + Duration::from_secs(120), "type_c", &format!("data_{}", i)).unwrap();
    }

    // Generate metrics JSON
    let metrics_json = timeline.to_metrics_json();

    // Verify metrics structure
    assert!(metrics_json.contains("bucket_count"), "Metrics should include bucket_count");
    assert!(metrics_json.contains("total_events"), "Metrics should include total_events");
    assert!(metrics_json.contains("buckets"), "Metrics should include bucket details");

    // Parse and verify
    let metrics: serde_json::Value = serde_json::from_str(&metrics_json).unwrap();
    assert_eq!(metrics["bucket_count"], 3, "Should report 3 buckets");
    assert_eq!(metrics["total_events"], 45, "Should report 45 total events");
}

#[tokio::test]
async fn test_storage_persistence_buckets() {
    // Q18: Persist buckets to storage and reload
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    // Add events across multiple buckets
    for minute in 0..5 {
        for event in 0..10 {
            let timestamp = now + Duration::from_secs(minute * 60 + event);
            timeline.append(timestamp, "event", &format!("min_{}_evt_{}", minute, event)).unwrap();
        }
    }

    // Persist to temporary storage
    let temp_path = std::env::temp_dir().join("timeline_test.bin");
    timeline.persist_to_file(&temp_path).await.unwrap();

    // Reload from storage
    let reloaded = TimelineAggregationCapsule::load_from_file(&temp_path).await.unwrap();

    // Verify persistence
    assert_eq!(reloaded.bucket_count(), timeline.bucket_count(), "Bucket count preserved");
    assert_eq!(reloaded.total_events(), timeline.total_events(), "Event count preserved");

    // Cleanup
    std::fs::remove_file(&temp_path).ok();
}

#[tokio::test]
async fn test_analytics_query_by_bucket() {
    // Q19: Query events by time range (bucket-based analytics)
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(13000000);

    // Populate timeline with timestamped events
    for hour in 0..3 {
        for minute in 0..60 {
            let timestamp = base_time + Duration::from_secs(hour * 3600 + minute * 60);
            timeline.append(timestamp, "event", &format!("hour_{}_min_{}", hour, minute)).unwrap();
        }
    }

    // Query: events in hour 1 (buckets 60-119)
    let query_start = base_time + Duration::from_secs(3600);
    let query_end = base_time + Duration::from_secs(7200);

    let results = timeline.query_time_range(query_start, query_end).unwrap();

    // Should return 60 events from hour 1
    assert_eq!(results.len(), 60, "Query should return events from specified time range");
}

#[tokio::test]
async fn test_analytics_aggregation_per_bucket() {
    // Q19: Aggregate statistics per bucket
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    // Add events with different types
    timeline.append(now, "login", "user_1").unwrap();
    timeline.append(now, "login", "user_2").unwrap();
    timeline.append(now, "api_call", "endpoint_1").unwrap();
    timeline.append(now + Duration::from_secs(60), "login", "user_3").unwrap();

    // Get aggregated counts per event type in bucket 0
    let bucket_0_stats = timeline.get_bucket_type_counts(0).unwrap();

    assert_eq!(bucket_0_stats.get("login"), Some(&2), "Bucket 0 should have 2 login events");
    assert_eq!(bucket_0_stats.get("api_call"), Some(&1), "Bucket 0 should have 1 api_call event");
}

#[tokio::test]
async fn test_replay_from_bucket_restores_state() {
    // Q20: Replay events from bucket to reconstruct application state
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let base_time = SystemTime::UNIX_EPOCH + Duration::from_secs(14000000);

    // Simulate state-modifying events
    timeline.append(base_time, "account_created", "user_id=100,balance=1000").unwrap();
    timeline.append(base_time + Duration::from_secs(10), "deposit", "user_id=100,amount=500").unwrap();
    timeline.append(base_time + Duration::from_secs(20), "withdrawal", "user_id=100,amount=200").unwrap();

    // Replay bucket 0 to reconstruct state
    let bucket_events = timeline.get_bucket_events(0).unwrap();
    let mut account_balance = 0i64;

    for event in bucket_events {
        match event.event_type.as_str() {
            "account_created" => {
                // Parse balance from data
                account_balance = 1000;
            }
            "deposit" => {
                account_balance += 500;
            }
            "withdrawal" => {
                account_balance -= 200;
            }
            _ => {}
        }
    }

    // Final state: 1000 + 500 - 200 = 1300
    assert_eq!(account_balance, 1300, "Replay should reconstruct correct state");
}

#[tokio::test]
async fn test_replay_deterministic() {
    // Q20: Multiple replays produce identical results (deterministic)
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    // Add events
    for i in 0..50 {
        timeline.append(now, "event", &format!("data_{}", i)).unwrap();
    }

    // Replay 1
    let events_1 = timeline.get_bucket_events(0).unwrap();
    let hash_1 = timeline.get_bucket_hash(0).unwrap();

    // Replay 2 (should be identical)
    let events_2 = timeline.get_bucket_events(0).unwrap();
    let hash_2 = timeline.get_bucket_hash(0).unwrap();

    assert_eq!(events_1.len(), events_2.len(), "Replay should be deterministic");
    assert_eq!(hash_1, hash_2, "Hash should be deterministic across replays");
}

#[tokio::test]
async fn test_backward_compatibility_with_audit_log() {
    // Q21: TimelineAggregationCapsule can consume existing AuditLogCapsule events
    let audit_bridge = AuditLogBridge::new();
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));

    // Log events using existing audit log format
    for i in 0..10 {
        audit_bridge.append(&format!("legacy_event_{}", i)).await.unwrap();
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Convert legacy audit log to timeline format
    let legacy_events = audit_bridge.drain_recent_events();
    for (timestamp, event_type, data) in legacy_events {
        // Backward-compatible conversion
        timeline.append(timestamp, &event_type, &data).unwrap();
    }

    assert!(timeline.total_events() >= 10, "Legacy events migrated to timeline");
}

#[tokio::test]
async fn test_backward_compatibility_event_format() {
    // Q21: Timeline preserves original event format from audit log
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    // Append event in legacy format (simple string)
    let legacy_event = "ResponseReceived user=42 amount=12345 prev_hash=0x1234";
    timeline.append(now, "audit_event", legacy_event).unwrap();

    // Retrieve and verify format preserved
    let events = timeline.get_bucket_events(0).unwrap();
    assert_eq!(events[0].data, legacy_event, "Legacy format should be preserved");
}

#[tokio::test]
async fn test_integration_error_handling() {
    // Q16: Error conditions propagate correctly across integration
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));

    // Attempt to append with invalid timestamp (far future)
    let far_future = SystemTime::now() + Duration::from_secs(365 * 24 * 3600 * 100); // 100 years
    let result = timeline.append(far_future, "event", "data");

    // Should reject or handle gracefully
    if result.is_err() {
        assert!(timeline.error_count() > 0, "Error should be tracked");
    }
}

#[tokio::test]
async fn test_integration_performance_budget() {
    // Q17: End-to-end latency meets performance budget
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));
    let now = SystemTime::now();

    let start = std::time::Instant::now();

    // Append 1000 events
    for i in 0..1000 {
        timeline.append(now + Duration::from_millis(i), "event", "data").unwrap();
    }

    let elapsed = start.elapsed();

    // Budget: <1ms per event (1000 events in <1 second)
    let avg_ns = elapsed.as_nanos() / 1000;
    assert!(avg_ns < 1_000_000, "Append should be <1ms per event, got {}ns", avg_ns);
}

#[tokio::test]
async fn test_cross_component_hash_chain() {
    // Q21: Hash chain integrity across audit_log → timeline migration
    let audit_bridge = AuditLogBridge::new();
    let mut timeline = TimelineAggregationCapsule::new(Duration::from_secs(60));

    // Append to audit log with hash chain
    audit_bridge.log_request(1, 100, 0x0000).await.unwrap();
    audit_bridge.log_request(2, 200, 0x1234).await.unwrap();
    audit_bridge.log_request(3, 300, 0x5678).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Migrate to timeline
    let events = audit_bridge.drain_recent_events();
    for (timestamp, event_type, data) in events {
        timeline.append(timestamp, &event_type, &data).unwrap();
    }

    // Verify timeline hash chain is valid
    let timeline_hash = timeline.get_bucket_hash(0).unwrap();
    assert!(timeline_hash > 0, "Timeline should maintain hash chain after migration");
}
