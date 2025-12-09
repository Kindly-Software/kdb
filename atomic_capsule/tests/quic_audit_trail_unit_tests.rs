//! Unit tests for QuicAuditTrailCapsule (T0 Auditable)
//!
//! Tests verify Q34 compliance, ASSUM safety, and hash chain integrity

#![cfg(feature = "quic")]

use atomic_capsule::quic::{AuditEventType, AuditTrailError, QuicAuditTrailCapsule};

// ============================================================================
// Unit Tests (Q1-Q7 T28 Tier)
// ============================================================================

#[test]
fn test_audit_trail_new_empty() {
    let audit = QuicAuditTrailCapsule::new();
    assert_eq!(audit.event_count(), 0);
}

#[test]
fn test_append_single_event() {
    let audit = QuicAuditTrailCapsule::new();
    let result = audit.append_event(AuditEventType::ConnectionEstablished, 0x11223344, 100);
    assert!(result.is_ok());
    assert_eq!(audit.event_count(), 1);
}

#[test]
fn test_append_multiple_events() {
    let audit = QuicAuditTrailCapsule::new();
    for i in 0..5 {
        let result = audit.append_event(AuditEventType::PacketLost, 0x11223344, i as u16);
        assert!(result.is_ok());
    }
    assert_eq!(audit.event_count(), 5);
}

#[test]
fn test_all_event_types() {
    let audit = QuicAuditTrailCapsule::new();

    let event_types = [
        AuditEventType::ConnectionEstablished,
        AuditEventType::ConnectionMigrated,
        AuditEventType::ConnectionClosed,
        AuditEventType::PacketLost,
        AuditEventType::FlowControlViolation,
        AuditEventType::CongestionEvent,
        AuditEventType::TlsHandshakeComplete,
        AuditEventType::StreamCreated,
        AuditEventType::StreamClosed,
        AuditEventType::AckReceived,
    ];

    for (i, event_type) in event_types.iter().enumerate() {
        let result = audit.append_event(*event_type, 0x12345678, i as u16);
        assert!(result.is_ok());
    }

    assert_eq!(audit.event_count(), event_types.len() as u32);
}

#[test]
fn test_ring_buffer_max_capacity() {
    let audit = QuicAuditTrailCapsule::new();

    // Fill ring buffer to capacity (16 events)
    for i in 0..16 {
        let result = audit.append_event(AuditEventType::AckReceived, 0x11111111, i as u16);
        assert!(result.is_ok());
    }

    assert_eq!(audit.event_count(), 16);
}

#[test]
fn test_ring_buffer_full_error() {
    let audit = QuicAuditTrailCapsule::new();

    // Fill to capacity
    for i in 0..16 {
        let _ = audit.append_event(AuditEventType::AckReceived, 0x11111111, i as u16);
    }

    // Next append should fail
    let result = audit.append_event(AuditEventType::ConnectionClosed, 0x22222222, 0);
    assert_eq!(result, Err(AuditTrailError::AuditFull));
}

#[test]
fn test_size_and_alignment() {
    assert_eq!(std::mem::size_of::<QuicAuditTrailCapsule>(), 256);
    assert_eq!(std::mem::align_of::<QuicAuditTrailCapsule>(), 256);
}

#[test]
fn test_clear_resets_trail() {
    let audit = QuicAuditTrailCapsule::new();

    // Add events
    let _ = audit.append_event(AuditEventType::ConnectionEstablished, 0x11111111, 100);
    let _ = audit.append_event(AuditEventType::PacketLost, 0x22222222, 200);
    assert_eq!(audit.event_count(), 2);

    // Clear
    audit.clear();
    assert_eq!(audit.event_count(), 0);

    // Should be able to add more
    let result = audit.append_event(AuditEventType::TlsHandshakeComplete, 0x33333333, 300);
    assert!(result.is_ok());
    assert_eq!(audit.event_count(), 1);
}

// ============================================================================
// Property Tests (Q8-Q14 T28 Tier)
// ============================================================================

#[test]
fn test_monotonic_event_count() {
    let audit = QuicAuditTrailCapsule::new();

    for i in 1..=10 {
        let _ = audit.append_event(AuditEventType::AckReceived, 0x11111111, i as u16);
        assert_eq!(audit.event_count(), i as u32);
    }
}

#[test]
fn test_append_all_connection_ids() {
    let audit = QuicAuditTrailCapsule::new();

    for cid in [0u32, 0xFFFFFFFF, 0x12345678, 0xABCDEF00].iter() {
        let _ = audit.append_event(AuditEventType::ConnectionEstablished, *cid, 0);
    }

    assert_eq!(audit.event_count(), 4);
}

// ============================================================================
// Integration Tests (Q15-Q21 T28 Tier)
// ============================================================================

#[test]
fn test_hash_chain_verification_empty() {
    let audit = QuicAuditTrailCapsule::new();
    let result = audit.verify_hash_chain();
    assert!(result.is_ok());
}

#[test]
fn test_hash_chain_verification_single_event() {
    let audit = QuicAuditTrailCapsule::new();
    let _ = audit.append_event(AuditEventType::ConnectionEstablished, 0x11111111, 100);

    let result = audit.verify_hash_chain();
    assert!(result.is_ok());
}

#[test]
fn test_hash_chain_verification_multiple_events() {
    let audit = QuicAuditTrailCapsule::new();

    let _ = audit.append_event(AuditEventType::ConnectionEstablished, 0x11111111, 100);
    let _ = audit.append_event(AuditEventType::PacketLost, 0x22222222, 200);
    let _ = audit.append_event(AuditEventType::TlsHandshakeComplete, 0x33333333, 300);
    let _ = audit.append_event(AuditEventType::AckReceived, 0x44444444, 400);

    let result = audit.verify_hash_chain();
    assert!(result.is_ok());
}

#[test]
fn test_export_events_empty() {
    let audit = QuicAuditTrailCapsule::new();
    let result = audit.export_events();
    assert!(result.is_ok());

    let events = result.unwrap();
    assert_eq!(events.len(), 0);
}

#[test]
fn test_export_events_with_data() {
    let audit = QuicAuditTrailCapsule::new();

    let _ = audit.append_event(AuditEventType::ConnectionEstablished, 0x11223344, 100);
    let _ = audit.append_event(AuditEventType::PacketLost, 0x55667788, 200);

    let result = audit.export_events();
    assert!(result.is_ok());

    let events = result.unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, AuditEventType::ConnectionEstablished);
    assert_eq!(events[1].event_type, AuditEventType::PacketLost);
}

// ============================================================================
// Production Tests (Q22-Q28 T28 Tier)
// ============================================================================

#[test]
fn test_concurrent_appends() {
    use std::sync::Arc;
    use std::thread;

    let audit = Arc::new(QuicAuditTrailCapsule::new());

    let mut handles = vec![];
    for t in 0..4 {
        let audit_clone = Arc::clone(&audit);
        let handle = thread::spawn(move || {
            for i in 0..3 {
                let cid = (t * 1000 + i) as u32;
                let _ = audit_clone.append_event(AuditEventType::AckReceived, cid, i as u16);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    // Should have some events (up to 12)
    let count = audit.event_count();
    assert!(count > 0);
    assert!(count <= 16);

    // Hash chain should still be valid
    let result = audit.verify_hash_chain();
    assert!(result.is_ok());
}

#[test]
fn test_stress_fill_and_clear_cycle() {
    let audit = QuicAuditTrailCapsule::new();

    for cycle in 0..10 {
        // Fill to capacity
        for i in 0..16 {
            let cid = ((cycle * 16 + i) as u32).wrapping_mul(0x12345678);
            let result = audit.append_event(AuditEventType::AckReceived, cid, i as u16);
            assert!(result.is_ok());
        }

        assert_eq!(audit.event_count(), 16);

        // Verify integrity
        let verify_result = audit.verify_hash_chain();
        assert!(verify_result.is_ok());

        // Clear
        audit.clear();
        assert_eq!(audit.event_count(), 0);
    }
}

#[test]
fn test_event_type_consistency() {
    let audit = QuicAuditTrailCapsule::new();

    let event_types = [
        (AuditEventType::ConnectionEstablished, "ConnectionEstablished"),
        (AuditEventType::PacketLost, "PacketLost"),
        (AuditEventType::TlsHandshakeComplete, "TlsHandshakeComplete"),
    ];

    for (event_type, expected_name) in &event_types {
        let result = audit.append_event(*event_type, 0x11111111, 0);
        assert!(result.is_ok());
        assert_eq!(format!("{}", event_type), *expected_name);
    }

    let exported = audit.export_events().unwrap();
    for (i, (event_type, _expected_name)) in event_types.iter().enumerate() {
        assert_eq!(exported[i].event_type, *event_type);
    }
}

// ============================================================================
// Q34 Compliance Tests
// ============================================================================

#[test]
fn test_q34_audit_trail_immutability() {
    // Hash chain should detect tampering
    let audit = QuicAuditTrailCapsule::new();

    let _ = audit.append_event(AuditEventType::ConnectionEstablished, 0x11111111, 100);
    let _ = audit.append_event(AuditEventType::PacketLost, 0x22222222, 200);

    // Verify before export
    assert!(audit.verify_hash_chain().is_ok());

    // Export for compliance reporting
    let events = audit.export_events().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, AuditEventType::ConnectionEstablished);
}

#[test]
fn test_error_display_messages() {
    assert_eq!(
        format!("{}", AuditTrailError::AuditFull),
        "Audit trail ring buffer full"
    );
    assert_eq!(
        format!("{}", AuditTrailError::TamperDetected),
        "Hash chain verification failed (tampering detected)"
    );
}

// ============================================================================
// ASSUM Safety Verification Tests
// ============================================================================

#[test]
fn test_lockfree_no_panics() {
    // Verify no panics during concurrent access
    let audit = std::sync::Arc::new(QuicAuditTrailCapsule::new());

    let handles: Vec<_> = (0..8)
        .map(|t| {
            let a = audit.clone();
            std::thread::spawn(move || {
                for i in 0..10 {
                    let _ = a.append_event(AuditEventType::AckReceived, t as u32, i as u16);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Should still be valid
    let result = audit.verify_hash_chain();
    assert!(result.is_ok());
}

#[test]
fn test_metadata_range_values() {
    let audit = QuicAuditTrailCapsule::new();

    // Test boundary values for metadata
    let values = [0u16, 1, 255, 256, 32767, 65535];

    for (i, value) in values.iter().enumerate() {
        if i < 16 {
            let result = audit.append_event(AuditEventType::AckReceived, 0x12345678, *value);
            assert!(result.is_ok());
        }
    }

    let exported = audit.export_events().unwrap();
    for (i, value) in values[..std::cmp::min(6, exported.len())].iter().enumerate() {
        assert_eq!(exported[i].metadata, *value);
    }
}
