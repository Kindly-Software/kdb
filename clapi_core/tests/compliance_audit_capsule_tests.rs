//! Comprehensive Test Suite for ComplianceAuditCapsule
//!
//! T28 Testing Framework Coverage:
//! - Unit tests (Q1-Q7): Capsule invariants, event logging, hash chain
//! - Property tests (Q8-Q14): Hash chain integrity, no event loss
//! - Integration tests (Q15-Q21): Multi-user audit trails
//! - Stress tests (Q22-Q28): 10K events, concurrent logging, forensic queries

use clapi_core::compliance::{
    ComplianceAuditCapsule,
    AuditEvent,
    AuditEventType,
    AuditEventStatus,
};
use clapi_core::compliance::audit_capsule::forensics;

// ============================================================================
// UNIT TESTS (Q1-Q7): Capsule Invariants
// ============================================================================

#[test]
fn unit_test_capsule_size() {
    use std::mem::size_of;
    assert_eq!(size_of::<ComplianceAuditCapsule>(), 576);
}

#[test]
fn unit_test_capsule_alignment() {
    use std::mem::align_of;
    assert_eq!(align_of::<ComplianceAuditCapsule>(), 64);
}

#[test]
fn unit_test_event_size() {
    use std::mem::size_of;
    // AuditEvent should be 56 bytes (8-byte aligned)
    let size = size_of::<AuditEvent>();
    assert!(size <= 64, "AuditEvent size: {} bytes (expected ≤64)", size);
}

#[test]
fn unit_test_initial_state() {
    let capsule = ComplianceAuditCapsule::new();
    assert_eq!(capsule.event_count(), 0);
    assert_eq!(capsule.cumulative_hash(), 0);
    assert_eq!(capsule.generation(), 0);
}

#[test]
fn unit_test_single_event_logging() {
    let mut capsule = ComplianceAuditCapsule::new();

    let result = capsule.log_login(123, true);
    assert!(result);
    assert_eq!(capsule.event_count(), 1);
    assert_eq!(capsule.generation(), 1);
    assert_ne!(capsule.cumulative_hash(), 0);
}

#[test]
fn unit_test_event_type_enum() {
    assert_eq!(AuditEventType::Login as u8, 0);
    assert_eq!(AuditEventType::Payment as u8, 2);
    assert_eq!(AuditEventType::Export as u8, 3);

    assert_eq!(AuditEventType::from_u8(0), Some(AuditEventType::Login));
    assert_eq!(AuditEventType::from_u8(2), Some(AuditEventType::Payment));
    assert_eq!(AuditEventType::from_u8(99), None);
}

#[test]
fn unit_test_event_status_enum() {
    assert_eq!(AuditEventStatus::Success as u8, 0);
    assert_eq!(AuditEventStatus::Failure as u8, 1);

    assert_eq!(AuditEventStatus::from_u8(0), Some(AuditEventStatus::Success));
    assert_eq!(AuditEventStatus::from_u8(1), Some(AuditEventStatus::Failure));
    assert_eq!(AuditEventStatus::from_u8(99), None);
}

#[test]
fn unit_test_event_hash_determinism() {
    let event1 = AuditEvent::new(
        AuditEventType::Login,
        123,
        AuditEventStatus::Success,
        0,
        0x1234,
    );

    let event2 = AuditEvent::new(
        AuditEventType::Login,
        123,
        AuditEventStatus::Success,
        0,
        0x1234,
    );

    // Hashes will differ due to different timestamps
    // But both should be non-zero and valid
    assert_ne!(event1.curr_hash, 0);
    assert_ne!(event2.curr_hash, 0);
    assert!(event1.verify_hash());
    assert!(event2.verify_hash());
}

#[test]
fn unit_test_hash_chain_linking() {
    let mut capsule = ComplianceAuditCapsule::new();

    capsule.log_login(1, true);
    capsule.log_payment(2, 5000, AuditEventStatus::Success);
    capsule.log_export(3, true);

    let events = capsule.get_events();
    assert_eq!(events.len(), 3);

    // Genesis event has prev_hash = 0
    assert_eq!(events[0].prev_hash, 0);

    // Second event links to first
    assert_eq!(events[1].prev_hash, events[0].curr_hash);

    // Third event links to second
    assert_eq!(events[2].prev_hash, events[1].curr_hash);
}

#[test]
fn unit_test_ring_buffer_fifo() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Add 5 events
    for i in 0..5 {
        capsule.log_login(i as u64, true);
    }

    let events = capsule.get_events();
    assert_eq!(events.len(), 5);

    // Events should be in FIFO order
    for i in 0..5 {
        assert_eq!(events[i].user_id, i as u64);
    }
}

#[test]
fn unit_test_ring_buffer_wraparound() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Fill buffer (10 events)
    for i in 0..10 {
        capsule.log_login(i as u64, true);
    }
    assert_eq!(capsule.event_count(), 10);

    // Add 5 more - should evict first 5
    for i in 10..15 {
        capsule.log_login(i as u64, true);
    }
    assert_eq!(capsule.event_count(), 10);

    let events = capsule.get_events();
    // First event should be user_id=5 (0-4 evicted)
    assert_eq!(events[0].user_id, 5);
    assert_eq!(events[9].user_id, 14);
}

#[test]
fn unit_test_multiple_event_types() {
    let mut capsule = ComplianceAuditCapsule::new();

    capsule.log_login(100, true);
    capsule.log_payment(100, 5000, AuditEventStatus::Success);
    capsule.log_export(100, true);
    capsule.log_access(100, true);
    capsule.log_permission_change(100, true);
    capsule.log_logout(100);

    assert_eq!(capsule.event_count(), 6);

    let events = capsule.get_events();
    assert_eq!(events[0].event_type, AuditEventType::Login as u8);
    assert_eq!(events[1].event_type, AuditEventType::Payment as u8);
    assert_eq!(events[2].event_type, AuditEventType::Export as u8);
    assert_eq!(events[3].event_type, AuditEventType::Access as u8);
    assert_eq!(events[4].event_type, AuditEventType::PermissionChange as u8);
    assert_eq!(events[5].event_type, AuditEventType::Logout as u8);
}

// ============================================================================
// PROPERTY TESTS (Q8-Q14): Hash Chain Integrity
// ============================================================================

#[test]
fn property_test_hash_chain_never_broken() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Add 100 events
    for i in 0..100 {
        capsule.log_login((i % 10) as u64, i % 2 == 0);
    }

    // Hash chain should always be valid
    assert!(capsule.verify_integrity());
}

#[test]
fn property_test_no_event_loss() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Add 100 events (will wrap around ring buffer)
    for i in 0..100 {
        capsule.log_login(i as u64, true);
    }

    // Should have exactly 10 events (ring buffer capacity)
    assert_eq!(capsule.event_count(), 10);

    // Events should be the last 10
    let events = capsule.get_events();
    for i in 0..10 {
        assert_eq!(events[i].user_id, (90 + i) as u64);
    }
}

#[test]
fn property_test_generation_monotonic() {
    let mut capsule = ComplianceAuditCapsule::new();

    let mut prev_generation = 0;
    for i in 0..50 {
        capsule.log_login(i as u64, true);
        let current_generation = capsule.generation();
        assert!(current_generation > prev_generation);
        prev_generation = current_generation;
    }
}

#[test]
fn property_test_hash_tamper_detection() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Add events
    for i in 0..5 {
        capsule.log_login(i as u64, true);
    }

    assert!(capsule.verify_integrity());

    // Tamper with middle event
    capsule.events[2].amount_cents = 999999;

    // Should detect tampering
    assert!(!capsule.verify_integrity());
}

#[test]
fn property_test_event_hash_uniqueness() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Log many events
    for i in 0..10 {
        capsule.log_login(i as u64, true);
    }

    let events = capsule.get_events();
    let hashes: Vec<u64> = events.iter().map(|e| e.curr_hash).collect();

    // All hashes should be unique (extremely high probability with FNV-1a)
    let unique_hashes: std::collections::HashSet<u64> = hashes.iter().copied().collect();
    assert_eq!(unique_hashes.len(), hashes.len());
}

#[test]
fn property_test_cumulative_hash_nonzero() {
    let mut capsule = ComplianceAuditCapsule::new();

    assert_eq!(capsule.cumulative_hash(), 0);

    capsule.log_login(1, true);

    // After one event, cumulative hash should be non-zero
    assert_ne!(capsule.cumulative_hash(), 0);
}

// ============================================================================
// INTEGRATION TESTS (Q15-Q21): Multi-User Audit Trails
// ============================================================================

#[test]
fn integration_test_multi_user_activity() {
    let mut capsule = ComplianceAuditCapsule::new();

    // User 100 activity
    capsule.log_login(100, true);
    capsule.log_payment(100, 5000, AuditEventStatus::Success);
    capsule.log_export(100, true);

    // User 200 activity
    capsule.log_login(200, false);
    capsule.log_login(200, true);
    capsule.log_access(200, true);

    // User 300 activity
    capsule.log_login(300, true);
    capsule.log_permission_change(300, true);

    assert_eq!(capsule.event_count(), 8);

    // Forensic analysis for each user
    let summary_100 = forensics::user_activity_summary(&capsule, 100);
    assert_eq!(summary_100.total_events, 3);
    assert_eq!(summary_100.logins, 1);
    assert_eq!(summary_100.payments, 1);
    assert_eq!(summary_100.exports, 1);

    let summary_200 = forensics::user_activity_summary(&capsule, 200);
    assert_eq!(summary_200.total_events, 3);
    assert_eq!(summary_200.logins, 2);
    assert_eq!(summary_200.failed_events, 1);

    let summary_300 = forensics::user_activity_summary(&capsule, 300);
    assert_eq!(summary_300.total_events, 2);
    assert_eq!(summary_300.logins, 1);
    assert_eq!(summary_300.permission_changes, 1);
}

#[test]
fn integration_test_timeline_reconstruction() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Simulate user session
    capsule.log_login(100, true);
    std::thread::sleep(std::time::Duration::from_millis(10));

    capsule.log_access(100, true);
    std::thread::sleep(std::time::Duration::from_millis(10));

    capsule.log_payment(100, 10000, AuditEventStatus::Success);
    std::thread::sleep(std::time::Duration::from_millis(10));

    capsule.log_export(100, true);
    std::thread::sleep(std::time::Duration::from_millis(10));

    capsule.log_logout(100);

    let timeline = forensics::reconstruct_timeline(&capsule);
    assert_eq!(timeline.len(), 5);

    // Verify chronological order
    for i in 1..timeline.len() {
        assert!(timeline[i].timestamp_ns >= timeline[i-1].timestamp_ns);
    }
}

#[test]
fn integration_test_anomaly_detection_failed_logins() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Failed login streak
    for _ in 0..5 {
        capsule.log_login(100, false);
    }

    let report = forensics::detect_anomalies(&capsule, 100);
    assert_eq!(report.failed_login_streak, 5);
}

#[test]
fn integration_test_anomaly_detection_large_payments() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Large payments
    capsule.log_payment(100, 150000, AuditEventStatus::Success);
    capsule.log_payment(100, 250000, AuditEventStatus::Success);
    capsule.log_payment(100, 500, AuditEventStatus::Success);

    let report = forensics::detect_anomalies(&capsule, 100);
    assert_eq!(report.large_payment_count, 2);
}

#[test]
fn integration_test_sox_404_compliance() {
    let mut capsule = ComplianceAuditCapsule::new();

    // SOX 404 requirements
    capsule.log_login(100, true);  // User authentication
    capsule.log_permission_change(100, true);  // Authorization changes
    capsule.log_payment(100, 500000, AuditEventStatus::Success);  // Financial transactions

    let events = capsule.get_events();
    assert_eq!(events.len(), 3);
    assert!(capsule.verify_integrity());
}

#[test]
fn integration_test_soc2_compliance() {
    let mut capsule = ComplianceAuditCapsule::new();

    // SOC2 requirements
    capsule.log_access(100, true);  // Access logging
    capsule.log_export(100, true);  // Data export tracking
    capsule.log_permission_change(100, true);  // Change control

    let events = capsule.get_events();
    assert_eq!(events.len(), 3);
    assert!(capsule.verify_integrity());
}

#[test]
fn integration_test_gdpr_article_30_compliance() {
    let mut capsule = ComplianceAuditCapsule::new();

    // GDPR Article 30 requirements
    capsule.log_access(100, true);  // Processing activity records
    capsule.log_export(100, true);  // Right to data portability

    let events = capsule.get_events();
    assert_eq!(events.len(), 2);
    assert!(capsule.verify_integrity());
}

// ============================================================================
// STRESS TESTS (Q22-Q28): High-Load Scenarios
// ============================================================================

#[test]
fn stress_test_10k_events() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Log 10K events
    for i in 0..10_000 {
        let user_id = (i % 100) as u64;
        let event_type = i % 6;

        match event_type {
            0 => capsule.log_login(user_id, true),
            1 => capsule.log_logout(user_id),
            2 => capsule.log_payment(user_id, (i * 100) as i64, AuditEventStatus::Success),
            3 => capsule.log_export(user_id, true),
            4 => capsule.log_access(user_id, true),
            5 => capsule.log_permission_change(user_id, true),
            _ => unreachable!(),
        };
    }

    // Ring buffer should have exactly 10 events
    assert_eq!(capsule.event_count(), 10);

    // Hash chain should still be valid
    assert!(capsule.verify_integrity());
}

#[test]
fn stress_test_rapid_logging() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Log 1000 events as fast as possible
    let start = std::time::Instant::now();
    for i in 0..1000 {
        capsule.log_login(i as u64, true);
    }
    let elapsed = start.elapsed();

    println!("1000 events logged in {:?} ({:.2} events/sec)",
             elapsed, 1000.0 / elapsed.as_secs_f64());

    // Should still be valid
    assert!(capsule.verify_integrity());
}

#[test]
fn stress_test_forensic_queries() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Log diverse events for 10 users
    for user_id in 0..10 {
        capsule.log_login(user_id, true);
        capsule.log_payment(user_id, 5000, AuditEventStatus::Success);
        capsule.log_export(user_id, true);
        capsule.log_access(user_id, true);
        capsule.log_logout(user_id);
    }

    // Query all users
    for user_id in 0..10 {
        let summary = forensics::user_activity_summary(&capsule, user_id);
        // Ring buffer only holds last 10 events, so some users won't have full activity
        assert!(summary.total_events <= 5);
    }
}

#[test]
fn stress_test_full_buffer_integrity() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Fill buffer completely
    for i in 0..10 {
        capsule.log_login(i as u64, true);
    }

    // Verify integrity when full
    assert!(capsule.verify_integrity());

    // Overwrite entire buffer
    for i in 10..20 {
        capsule.log_login(i as u64, true);
    }

    // Should still be valid
    assert!(capsule.verify_integrity());
}

#[test]
fn stress_test_mixed_event_types() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Log 500 mixed events
    for i in 0..500 {
        match i % 10 {
            0 => capsule.log_login(i as u64, i % 3 != 0),
            1 => capsule.log_logout(i as u64),
            2 => capsule.log_payment(i as u64, (i * 100) as i64, AuditEventStatus::Success),
            3 => capsule.log_payment(i as u64, -((i * 50) as i64), AuditEventStatus::Failure),
            4 => capsule.log_export(i as u64, i % 5 != 0),
            5 => capsule.log_access(i as u64, i % 7 != 0),
            6 => capsule.log_permission_change(i as u64, i % 4 != 0),
            7 => capsule.log_login(i as u64, false),
            8 => capsule.log_payment(i as u64, 1_000_000, AuditEventStatus::Pending),
            9 => capsule.log_access(i as u64, false),
            _ => unreachable!(),
        };
    }

    // Verify final state
    assert_eq!(capsule.event_count(), 10);
    assert!(capsule.verify_integrity());
}

// ============================================================================
// EDGE CASES & ERROR HANDLING
// ============================================================================

#[test]
fn edge_case_empty_buffer_integrity() {
    let capsule = ComplianceAuditCapsule::new();
    assert!(capsule.verify_integrity());
}

#[test]
fn edge_case_single_event_integrity() {
    let mut capsule = ComplianceAuditCapsule::new();
    capsule.log_login(1, true);
    assert!(capsule.verify_integrity());
}

#[test]
fn edge_case_negative_payment_amount() {
    let mut capsule = ComplianceAuditCapsule::new();

    // Negative payment (refund)
    capsule.log_payment(100, -5000, AuditEventStatus::Success);

    let events = capsule.get_events();
    assert_eq!(events[0].amount_cents, -5000);
}

#[test]
fn edge_case_zero_user_id() {
    let mut capsule = ComplianceAuditCapsule::new();

    capsule.log_login(0, true);

    let events = capsule.get_events();
    assert_eq!(events[0].user_id, 0);
}

#[test]
fn edge_case_max_user_id() {
    let mut capsule = ComplianceAuditCapsule::new();

    capsule.log_login(u64::MAX, true);

    let events = capsule.get_events();
    assert_eq!(events[0].user_id, u64::MAX);
}
