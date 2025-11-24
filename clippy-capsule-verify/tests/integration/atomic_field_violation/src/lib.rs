//! Integration tests for CAPSULE_NON_ATOMIC_FIELD lint
//! Tests detect when T1 Atomic capsules contain non-atomic data fields
#![deny(clippy::capsule_non_atomic_field)]

use std::sync::atomic::AtomicU64;

// ============================================================================
// Test 1: T1 Atomic with non-atomic u64 field (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct BadU64Field {
    state: AtomicU64,
    count: u64,  // ERROR: Non-atomic u64 in Atomic capsule
    generation: AtomicU64,
    _padding: [u8; 40],
}

// ============================================================================
// Test 2: T1 Atomic with non-atomic bool field (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct BadBoolField {
    state: AtomicU64,
    flag: bool,  // ERROR: Non-atomic bool in Atomic capsule
    generation: AtomicU64,
    _padding: [u8; 39],
}

// ============================================================================
// Test 3: Multiple non-atomic fields (FAIL)
// ============================================================================
#[repr(C, align(128))]
pub struct BadMultipleFields {
    state: AtomicU64,
    count: u64,  // ERROR: Non-atomic u64
    flag: bool,  // ERROR: Non-atomic bool
    value: i32,  // ERROR: Non-atomic i32
    generation: AtomicU64,
    _padding: [u8; 103],
}

// ============================================================================
// Test 4: Padding field allowed (PASS)
// ============================================================================
#[repr(C, align(64))]
pub struct GoodWithPadding {
    state: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 48],
}

// ============================================================================
// Test 5: T1 Atomic with non-atomic i64 field (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct BadI64Field {
    state: AtomicU64,
    count: i64,  // ERROR: Non-atomic i64 in Atomic capsule
    generation: AtomicU64,
    _padding: [u8; 40],
}

// ============================================================================
// Test 6: T1 Atomic with non-atomic usize field (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct BadUsizeField {
    state: AtomicU64,
    index: usize,  // ERROR: Non-atomic usize
    generation: AtomicU64,
    _padding: [u8; 40],
}

// ============================================================================
// Test 7: Non-Atomic tier allows u64 (PASS)
// ============================================================================
// T3 Fixed-Point tier doesn't require atomic fields
#[repr(C, align(64))]
pub struct GoodNonAtomicTier {
    state: i64,
    count: u64,  // Allowed in non-Atomic tiers
    _padding: [u8; 48],
}

// ============================================================================
// Test 8: Atomic with AtomicI64 (PASS)
// ============================================================================
use std::sync::atomic::AtomicI64;

#[repr(C, align(64))]
pub struct GoodAtomicI64 {
    state: AtomicU64,
    count: AtomicI64,  // Atomic variant allowed
    _padding: [u8; 48],
}

// ============================================================================
// Test 9: Multiple violations in one struct (FAIL)
// ============================================================================
#[repr(C, align(128))]
pub struct BadMultipleViolations {
    state: AtomicU64,
    counter: u64,  // ERROR
    flag: bool,    // ERROR
    extra: i32,    // ERROR
    generation: AtomicU64,
    _padding: [u8; 103],
}

// ============================================================================
// Test 10: Nested structs with correct atomics (PASS)
// ============================================================================
#[repr(C, align(64))]
pub struct GoodNested {
    state: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 48],
}

#[repr(C, align(64))]
pub struct GoodNestedParent {
    child: AtomicU64,
    sibling: AtomicU64,
    _padding: [u8; 48],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_good_with_padding_size() {
        assert_eq!(
            std::mem::size_of::<GoodWithPadding>(),
            64,
            "GoodWithPadding should be 64 bytes"
        );
    }

    #[test]
    fn test_good_non_atomic_tier_size() {
        assert_eq!(
            std::mem::size_of::<GoodNonAtomicTier>(),
            64,
            "GoodNonAtomicTier should be 64 bytes"
        );
    }

    #[test]
    fn test_good_atomic_i64_size() {
        assert_eq!(
            std::mem::size_of::<GoodAtomicI64>(),
            64,
            "GoodAtomicI64 should be 64 bytes"
        );
    }

    #[test]
    fn test_good_nested_size() {
        assert_eq!(
            std::mem::size_of::<GoodNested>(),
            64,
            "GoodNested should be 64 bytes"
        );
    }

    #[test]
    fn test_atomic_u64_size() {
        assert_eq!(
            std::mem::size_of::<AtomicU64>(),
            8,
            "AtomicU64 should be 8 bytes"
        );
    }

    #[test]
    fn test_atomic_i64_size() {
        assert_eq!(
            std::mem::size_of::<AtomicI64>(),
            8,
            "AtomicI64 should be 8 bytes"
        );
    }
}
