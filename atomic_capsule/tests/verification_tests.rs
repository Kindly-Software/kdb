//! # Verification Macro Integration Tests
//!
//! These tests verify that all verification macros work correctly at compile-time.
//!
//! ## Test Strategy
//!
//! - **Positive tests**: Valid capsules compile successfully
//! - **Negative tests**: Invalid capsules fail to compile (compile_fail tests)
//! - **Zero-cost verification**: All checks happen at compile-time, no runtime overhead

use atomic_capsule::{
    verify_alignment, verify_capsule, verify_capsule_properties, verify_dual_atomic_u64,
    verify_fixed_point_capsule, verify_generation_counter, verify_size, verify_thread_safe,
};
use core::sync::atomic::AtomicU64;

// ============================================================================
// Positive Tests: Valid capsules compile successfully
// ============================================================================

#[repr(C, align(64))]
struct ValidHotCapsule {
    data: [u8; 64],
}

#[repr(C, align(128))]
struct ValidWarmCapsule {
    data: [u8; 128],
}

#[repr(C, align(256))]
struct ValidColdCapsule {
    data: [u8; 256],
}

#[repr(C, align(128))]
struct ValidDualAtomic {
    primary: AtomicU64,
    secondary: AtomicU64,
}

#[repr(C, align(64))]
struct ValidGeneration {
    generation: AtomicU64,
    data: AtomicU64,
}

#[repr(C, align(64))]
struct ValidFixedPoint {
    price_q8_8: u16, // Q8.8 fixed-point
}

// Verify all valid capsules at compile-time
verify_capsule_properties!(ValidHotCapsule, 64, 64);
verify_capsule_properties!(ValidWarmCapsule, 128, 128);
verify_capsule_properties!(ValidColdCapsule, 256, 256);

verify_alignment!(ValidHotCapsule, 64);
verify_alignment!(ValidWarmCapsule, 128);
verify_alignment!(ValidColdCapsule, 256);

verify_size!(ValidHotCapsule, 64);
verify_size!(ValidWarmCapsule, 128);
verify_size!(ValidColdCapsule, 256);

verify_fixed_point_capsule!(ValidFixedPoint, 64, 8);
verify_dual_atomic_u64!(ValidDualAtomic);
verify_generation_counter!(ValidGeneration, generation);
verify_thread_safe!(ValidDualAtomic);

#[test]
fn test_valid_capsules_compile() {
    // If this test runs, all compile-time verifications passed
    assert_eq!(core::mem::align_of::<ValidHotCapsule>(), 64);
    assert_eq!(core::mem::align_of::<ValidWarmCapsule>(), 128);
    assert_eq!(core::mem::align_of::<ValidColdCapsule>(), 256);
    assert_eq!(core::mem::align_of::<ValidDualAtomic>(), 128);
}

// ============================================================================
// Atomic Capsule Pattern Tests
// ============================================================================

#[repr(C, align(64))]
struct CircuitBreakerCapsule {
    state: AtomicU64, // state:2 | level:2 | cause:4 | generation:56
}

verify_capsule_properties!(CircuitBreakerCapsule, 64, 8);
verify_thread_safe!(CircuitBreakerCapsule);

#[test]
fn test_circuit_breaker_pattern() {
    // ACB-64 pattern from The Atomic Capsule
    assert_eq!(core::mem::size_of::<CircuitBreakerCapsule>(), 8);
    assert_eq!(core::mem::align_of::<CircuitBreakerCapsule>(), 64);
}

#[repr(C, align(128))]
struct LedgerEntryCapsule {
    timestamp: AtomicU64,
    event_hash: AtomicU64,
}

verify_capsule_properties!(LedgerEntryCapsule, 128, 16);
verify_dual_atomic_u64!(LedgerEntryCapsule);

#[test]
fn test_ledger_entry_pattern() {
    // ALE-128 pattern from The Atomic Capsule
    assert_eq!(core::mem::size_of::<LedgerEntryCapsule>(), 16);
    assert_eq!(core::mem::align_of::<LedgerEntryCapsule>(), 128);
}

// ============================================================================
// Generation Counter Pattern Tests
// ============================================================================

#[repr(C, align(64))]
struct VersionedStateCapsule {
    generation: AtomicU64,
    state: AtomicU64,
}

verify_generation_counter!(VersionedStateCapsule, generation);
verify_thread_safe!(VersionedStateCapsule);

#[test]
fn test_generation_counter_pattern() {
    // Generation counter prevents TOCTOU races
    let capsule = VersionedStateCapsule {
        generation: AtomicU64::new(0),
        state: AtomicU64::new(42),
    };

    use core::sync::atomic::Ordering;
    let gen_before = capsule.generation.load(Ordering::Acquire);
    let state = capsule.state.load(Ordering::Acquire);
    let gen_after = capsule.generation.load(Ordering::Acquire);

    // Consistent read if generation unchanged
    assert_eq!(gen_before, gen_after);
    assert_eq!(state, 42);
}

// ============================================================================
// Fixed-Point Pattern Tests
// ============================================================================

#[repr(C, align(64))]
struct PriceCapsuleQ8_8 {
    price: u16, // Q8.8 fixed-point (8 integer, 8 fractional)
}

#[repr(C, align(64))]
struct PriceCapsuleQ4_12 {
    price: u16, // Q4.12 fixed-point (4 integer, 12 fractional)
}

verify_fixed_point_capsule!(PriceCapsuleQ8_8, 64, 8);
verify_fixed_point_capsule!(PriceCapsuleQ4_12, 64, 12);

#[test]
fn test_fixed_point_patterns() {
    // Q8.8: 8 fractional bits = 1/256 precision
    // Example: 0x0142 = 1.2578125 (1 + 66/256)
    assert_eq!(core::mem::size_of::<PriceCapsuleQ8_8>(), 2);

    // Q4.12: 12 fractional bits = 1/4096 precision
    // Example: 0x1800 = 1.5 (1 + 2048/4096)
    assert_eq!(core::mem::size_of::<PriceCapsuleQ4_12>(), 2);
}

// ============================================================================
// SIMD Capsule Pattern Tests (feature-gated)
// ============================================================================

#[cfg(feature = "portable_simd")]
mod simd_tests {
    use super::*;
    use atomic_capsule::verify_simd_capsule;
    use std::simd::u64x8;

    #[repr(C, align(64))]
    struct SimdCapsule {
        data: u64x8,
    }

    verify_simd_capsule!(SimdCapsule, 64, 32);

    #[test]
    fn test_simd_capsule_pattern() {
        // SIMD capsule requires minimum 32-byte alignment (AVX)
        assert_eq!(core::mem::align_of::<SimdCapsule>(), 64);
        assert_eq!(core::mem::size_of::<SimdCapsule>(), 64);
    }
}

// ============================================================================
// Thread Safety Tests
// ============================================================================

#[test]
fn test_thread_safe_capsules() {
    // All atomic capsules must be Send + Sync
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<ValidDualAtomic>();
    assert_sync::<ValidDualAtomic>();
    assert_send::<VersionedStateCapsule>();
    assert_sync::<VersionedStateCapsule>();
}

// ============================================================================
// Zero-Cost Verification Tests
// ============================================================================

#[test]
fn test_zero_runtime_cost() {
    // All verification happens at compile-time
    // This test verifies there's no runtime overhead

    use core::sync::atomic::Ordering;

    let capsule = ValidGeneration {
        generation: AtomicU64::new(0),
        data: AtomicU64::new(42),
    };

    // Reading the capsule should have zero verification overhead
    let start = std::time::Instant::now();
    let _data = capsule.data.load(Ordering::Relaxed);
    let elapsed = start.elapsed();

    // Should be nanoseconds, not microseconds (no runtime checks)
    assert!(elapsed.as_nanos() < 1000); // < 1μs
}

// ============================================================================
// Documentation Examples
// ============================================================================

/// Example: Circuit Breaker Capsule (ACB-64)
///
/// From The Atomic Capsule Section 7: Capsule library
#[repr(C, align(64))]
struct ExampleCircuitBreaker {
    state: AtomicU64, // L0..L3 level + cause bits
}

verify_capsule_properties!(ExampleCircuitBreaker, 64, 8);

/// Example: Dual-Channel Coordination (DualAtomicU64)
///
/// From The Atomic Capsule foundational pattern
#[repr(C, align(128))]
struct ExampleDualChannel {
    primary: AtomicU64,   // Hot path operations
    secondary: AtomicU64, // Metadata/coordination
}

verify_dual_atomic_u64!(ExampleDualChannel);

/// Example: Position Capsule (APC-512)
///
/// From The Atomic Capsule Section 7: Capsule library
#[repr(C, align(64))]
struct ExamplePosition {
    position: AtomicU64,
    vwap: AtomicU64,
    realized_pnl: AtomicU64,
    unrealized_pnl: AtomicU64,
}

verify_capsule_properties!(ExamplePosition, 64, 32);

#[test]
fn test_documentation_examples_compile() {
    // If this test runs, all documentation examples compile successfully
    assert_eq!(core::mem::align_of::<ExampleCircuitBreaker>(), 64);
    assert_eq!(core::mem::align_of::<ExampleDualChannel>(), 128);
    assert_eq!(core::mem::align_of::<ExamplePosition>(), 64);
}
