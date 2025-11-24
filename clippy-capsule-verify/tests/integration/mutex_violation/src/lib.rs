//! Integration tests for CAPSULE_MUTEX_VIOLATION lint
//! Tests detect when mutex/rwlock are used in computational capsules
#![deny(clippy::capsule_mutex_violation)]

use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::Arc;

// ============================================================================
// Test 1: Simple Mutex (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct SimpleMutexCapsule {
    lock: Mutex<u64>,  // ERROR: Mutex forbidden
    _padding: [u8; 48],
}

// ============================================================================
// Test 2: RwLock (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct RwLockCapsule {
    lock: RwLock<u64>,  // ERROR: RwLock forbidden
    _padding: [u8; 48],
}

// ============================================================================
// Test 3: Arc<Mutex> (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct ArcMutexCapsule {
    lock: Arc<Mutex<u64>>,  // ERROR: Mutex forbidden (even wrapped in Arc)
    _padding: [u8; 32],
}

// ============================================================================
// Test 4: Nested Mutex (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct NestedMutexCapsule {
    inner: Mutex<InnerData>,  // ERROR: Mutex forbidden
    _padding: [u8; 48],
}

struct InnerData {
    value: u64,
}

// ============================================================================
// Test 5: Multiple Mutexes (FAIL)
// ============================================================================
#[repr(C, align(128))]
pub struct MultipleMutexesCapsule {
    lock1: Mutex<u64>,  // ERROR: Mutex forbidden
    lock2: RwLock<u32>,  // ERROR: RwLock forbidden
    _padding: [u8; 96],
}

// ============================================================================
// Test 6: Valid - Atomic only (PASS)
// ============================================================================
use std::sync::atomic::AtomicU64;

#[repr(C, align(64))]
pub struct ValidAtomicCapsule {
    state: AtomicU64,
    counter: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 40],
}

// ============================================================================
// Test 7: Valid - Dual Atomic (PASS)
// ============================================================================
#[repr(C, align(64))]
pub struct ValidDualAtomicCapsule {
    primary: AtomicU64,
    secondary: AtomicU64,
    _padding: [u8; 48],
}

// ============================================================================
// Test 8: Valid - Multiple Atomics (PASS)
// ============================================================================
#[repr(C, align(128))]
pub struct ValidMultipleAtomicsCapsule {
    state: AtomicU64,
    counter: AtomicU64,
    generation: AtomicU64,
    extra: AtomicU64,
    _padding: [u8; 96],
}

// ============================================================================
// Test 9: Mixed - Mutex with primitives (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct MixedMutexCapsule {
    lock: Mutex<u64>,  // ERROR: Mutex forbidden
    flag: bool,
    _padding: [u8; 39],
}

// ============================================================================
// Test 10: Parking lot Mutex (FAIL - discouraged alternative)
// ============================================================================
#[cfg(feature = "parking_lot")]
#[repr(C, align(64))]
pub struct ParkingLotMutexCapsule {
    lock: parking_lot::Mutex<u64>,  // ERROR: Mutex variant forbidden
    _padding: [u8; 48],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_atomic_size() {
        assert_eq!(
            std::mem::size_of::<ValidAtomicCapsule>(),
            64,
            "ValidAtomicCapsule should be exactly 64 bytes"
        );
    }

    #[test]
    fn test_valid_dual_atomic_size() {
        assert_eq!(
            std::mem::size_of::<ValidDualAtomicCapsule>(),
            64,
            "ValidDualAtomicCapsule should be exactly 64 bytes"
        );
    }

    #[test]
    fn test_valid_multiple_atomics_size() {
        assert_eq!(
            std::mem::size_of::<ValidMultipleAtomicsCapsule>(),
            128,
            "ValidMultipleAtomicsCapsule should be exactly 128 bytes"
        );
    }
}
