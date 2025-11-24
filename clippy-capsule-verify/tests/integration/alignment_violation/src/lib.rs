//! Integration tests for CAPSULE_UNALIGNED_VIOLATION lint
//! Tests detect when struct size doesn't match alignment requirement
#![deny(clippy::capsule_unaligned_violation)]

use std::sync::atomic::AtomicU64;

// ============================================================================
// Test 1: 8 bytes needs 56 bytes padding for 64B alignment (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct BadCapsule8b {
    counter: AtomicU64,  // 8 bytes, missing 56 bytes padding
    // ERROR: Capsule size (8) does not match alignment (64)
}

// ============================================================================
// Test 2: 16 bytes needs 48 bytes padding for 64B alignment (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct BadCapsule16b {
    counter: AtomicU64,
    state: AtomicU64,
    // ERROR: Capsule size (16) does not match alignment (64)
}

// ============================================================================
// Test 3: 24 bytes needs 104 bytes padding for 128B alignment (FAIL)
// ============================================================================
#[repr(C, align(128))]
pub struct BadCapsule24b {
    counter: AtomicU64,
    state: AtomicU64,
    extra: AtomicU64,
    // ERROR: Capsule size (24) does not match alignment (128)
}

// ============================================================================
// Test 4: 32 bytes with 32 bytes padding = 64B (FAIL - wrong padding size)
// ============================================================================
#[repr(C, align(64))]
pub struct BadCapsule32b {
    counter: AtomicU64,
    state: AtomicU64,
    third: AtomicU64,
    fourth: AtomicU64,
    _padding: [u8; 32],  // Wrong size, should be 0
    // ERROR: Capsule size (40) does not match alignment (64)
}

// ============================================================================
// Test 5: 256B alignment with misaligned content (FAIL)
// ============================================================================
#[repr(C, align(256))]
pub struct BadCapsule256b {
    counter: AtomicU64,
    state: AtomicU64,
    // ERROR: Capsule size (16) does not match alignment (256)
}

// ============================================================================
// Test 6: Incorrect padding calculation (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct BadPaddingCalculation {
    counter: AtomicU64,
    _padding: [u8; 50],  // Should be 56, not 50
    // ERROR: Capsule size (58) does not match alignment (64)
}

// ============================================================================
// Test 7: Correct 64B alignment (PASS)
// ============================================================================
#[repr(C, align(64))]
pub struct GoodCapsule64b {
    counter: AtomicU64,
    _padding: [u8; 56],
}

// ============================================================================
// Test 8: Correct 128B alignment (PASS)
// ============================================================================
#[repr(C, align(128))]
pub struct GoodCapsule128b {
    counter: AtomicU64,
    state: AtomicU64,
    _padding: [u8; 112],
}

// ============================================================================
// Test 9: Correct 256B alignment (PASS)
// ============================================================================
#[repr(C, align(256))]
pub struct GoodCapsule256b {
    counter: AtomicU64,
    state: AtomicU64,
    extra: AtomicU64,
    fourth: AtomicU64,
    fifth: AtomicU64,
    sixth: AtomicU64,
    seventh: AtomicU64,
    _padding: [u8; 200],
}

// ============================================================================
// Test 10: Correct dual atomic 64B (PASS)
// ============================================================================
#[repr(C, align(64))]
pub struct GoodDualAtomic {
    primary: AtomicU64,
    secondary: AtomicU64,
    _padding: [u8; 48],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_good_capsule_64b_size() {
        assert_eq!(
            std::mem::size_of::<GoodCapsule64b>(),
            64,
            "GoodCapsule64b should be exactly 64 bytes"
        );
    }

    #[test]
    fn test_good_capsule_128b_size() {
        assert_eq!(
            std::mem::size_of::<GoodCapsule128b>(),
            128,
            "GoodCapsule128b should be exactly 128 bytes"
        );
    }

    #[test]
    fn test_good_capsule_256b_size() {
        assert_eq!(
            std::mem::size_of::<GoodCapsule256b>(),
            256,
            "GoodCapsule256b should be exactly 256 bytes"
        );
    }

    #[test]
    fn test_good_dual_atomic_size() {
        assert_eq!(
            std::mem::size_of::<GoodDualAtomic>(),
            64,
            "GoodDualAtomic should be exactly 64 bytes"
        );
    }

    #[test]
    fn test_64b_alignment() {
        assert_eq!(
            std::mem::align_of::<GoodCapsule64b>(),
            64,
            "GoodCapsule64b should have 64-byte alignment"
        );
    }

    #[test]
    fn test_128b_alignment() {
        assert_eq!(
            std::mem::align_of::<GoodCapsule128b>(),
            128,
            "GoodCapsule128b should have 128-byte alignment"
        );
    }

    #[test]
    fn test_256b_alignment() {
        assert_eq!(
            std::mem::align_of::<GoodCapsule256b>(),
            256,
            "GoodCapsule256b should have 256-byte alignment"
        );
    }
}
