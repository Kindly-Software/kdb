//! Minimal test suite for AuditEnhancementCapsule
//! This is a simplified version focusing on core functionality

use kdb_mcp::audit_enhancement::{
    AuditEnhancementCapsule, AuditEvent, Operation,
};
use std::mem::{size_of, align_of};

#[test]
fn test_capsule_layout() {
    assert_eq!(size_of::<AuditEnhancementCapsule>(), 4_194_560);
    assert_eq!(align_of::<AuditEnhancementCapsule>(), 256);
}

#[test]
fn test_event_structure() {
    assert_eq!(size_of::<AuditEvent>(), 16);
}

#[test]
fn test_single_append() {
    let capsule = AuditEnhancementCapsule::new();
    let result = capsule.append_event(Operation::AuthSuccess, 0);
    assert!(result.is_ok());
    
    let stats = capsule.get_stats();
    assert_eq!(stats.total_events, 1);
}

#[test]
fn test_ten_appends() {
    let capsule = AuditEnhancementCapsule::new();
    
    for _ in 0..10 {
        capsule.append_event(Operation::MemoryRead, 0).ok();
    }
    
    let stats = capsule.get_stats();
    assert_eq!(stats.total_events, 10);
}

#[test]
fn test_operation_enum() {
    assert_eq!(Operation::AuthSuccess.as_u8(), 0);
    assert_eq!(Operation::MemoryRead.as_u8(), 4);
    assert_eq!(Operation::from_u8(0), Some(Operation::AuthSuccess));
    assert_eq!(Operation::from_u8(255), None);
}

#[test]
fn test_hash_deterministic() {
    let event1 = AuditEvent::new(1000, 4, 0, 0xDEADBEEF);
    let event2 = AuditEvent::new(1000, 4, 0, 0xDEADBEEF);
    assert_eq!(event1.compute_hash(), event2.compute_hash());
}
