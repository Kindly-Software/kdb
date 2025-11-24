//! Integration tests for CAPSULE_MISSING_GENERATION lint
//! Tests detect when T1 Atomic capsules lack generation counter field
#![deny(clippy::capsule_missing_generation)]

use std::sync::atomic::AtomicU64;

// ============================================================================
// Test 1: T1 Atomic without generation (FAIL)
// ============================================================================
// Note: ComputationalCapsule derive would be here, but we can't use it
// without proper plugin loading. This demonstrates the violation pattern.
#[repr(C, align(64))]
pub struct BadAtomicNoGen {
    state: AtomicU64,
    _padding: [u8; 56],
    // ERROR: T1 Atomic capsule missing generation counter field
}

// ============================================================================
// Test 2: T1 Atomic with generation (PASS)
// ============================================================================
#[repr(C, align(64))]
pub struct GoodAtomicWithGen {
    state: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 48],
}

// ============================================================================
// Test 3: Dual atomic without gen (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct BadDualAtomicNoGen {
    primary: AtomicU64,
    secondary: AtomicU64,
    _padding: [u8; 48],
    // ERROR: T1 Atomic capsule missing generation counter
}

// ============================================================================
// Test 4: Non-atomic tier OK without gen (PASS)
// ============================================================================
// T3 Fixed-Point doesn't require generation counter
#[repr(C, align(64))]
pub struct GoodNonAtomicTier {
    value: i32,
    state: i32,
    _padding: [u8; 56],
}

// ============================================================================
// Test 5: Abbreviated gen field (PASS)
// ============================================================================
#[repr(C, align(64))]
pub struct GoodAbbreviatedGen {
    state: AtomicU64,
    gen: AtomicU64,  // Abbreviated name accepted
    _padding: [u8; 48],
}

// ============================================================================
// Test 6: Multiple atomics without gen (FAIL)
// ============================================================================
#[repr(C, align(128))]
pub struct BadMultipleAtomicsNoGen {
    state: AtomicU64,
    counter: AtomicU64,
    extra: AtomicU64,
    _padding: [u8; 104],
    // ERROR: T1 Atomic capsule missing generation counter
}

// ============================================================================
// Test 7: Fixed-point tier OK (PASS)
// ============================================================================
#[repr(C, align(64))]
pub struct GoodFixedPointTier {
    value: i64,
    scale: i32,
    _padding: [u8; 28],
}

// ============================================================================
// Test 8: "generation" not "gen" misspelled (FAIL)
// ============================================================================
#[repr(C, align(64))]
pub struct BadGenerationMisspelled {
    state: AtomicU64,
    generational: AtomicU64,  // Wrong field name
    _padding: [u8; 48],
    // ERROR: Missing "generation" or "gen" field
}

// ============================================================================
// Test 9: Batch tier OK without gen (PASS)
// ============================================================================
#[repr(C, align(64))]
pub struct GoodBatchTier {
    state: u64,
    counter: u64,
    _padding: [u8; 48],
}

// ============================================================================
// Test 10: Mixed tier with gen (PASS)
// ============================================================================
#[repr(C, align(128))]
pub struct GoodMixedTierWithGen {
    state: AtomicU64,
    counter: i64,
    generation: AtomicU64,
    _padding: [u8; 104],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_good_atomic_with_gen_size() {
        assert_eq!(
            std::mem::size_of::<GoodAtomicWithGen>(),
            64,
            "GoodAtomicWithGen should be 64 bytes"
        );
    }

    #[test]
    fn test_good_abbreviated_gen_size() {
        assert_eq!(
            std::mem::size_of::<GoodAbbreviatedGen>(),
            64,
            "GoodAbbreviatedGen should be 64 bytes"
        );
    }

    #[test]
    fn test_good_fixed_point_tier_size() {
        assert_eq!(
            std::mem::size_of::<GoodFixedPointTier>(),
            64,
            "GoodFixedPointTier should be 64 bytes"
        );
    }

    #[test]
    fn test_good_batch_tier_size() {
        assert_eq!(
            std::mem::size_of::<GoodBatchTier>(),
            64,
            "GoodBatchTier should be 64 bytes"
        );
    }

    #[test]
    fn test_good_mixed_tier_size() {
        assert_eq!(
            std::mem::size_of::<GoodMixedTierWithGen>(),
            128,
            "GoodMixedTierWithGen should be 128 bytes"
        );
    }
}
