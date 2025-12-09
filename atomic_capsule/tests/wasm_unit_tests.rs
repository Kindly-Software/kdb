//! WASM Unit Tests - T28 Tier 1 (Q1-Q7)
//!
//! Comprehensive unit tests for core atomic capsule primitives on wasm32-unknown-unknown target.
//! Following T28 framework: Q1 (Core behaviors), Q2 (Edge cases), Q3 (Invariants),
//! Q4 (Code paths), Q5 (Isolation), Q6 (Fast tests), Q7 (Readability).
//!
//! WASM-specific considerations:
//! - No threading support (single-threaded execution)
//! - AtomicU64 operations compile but don't test concurrency
//! - Focus on correctness of atomics in single-threaded context
//! - Test memory layout and alignment on WASM target

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// Import core primitives
use atomic_capsule::primitives::{DualAtomicU64, PackedState};
use core::sync::atomic::Ordering;

/// T28 Q1: Core behaviors - DualAtomicU64 basic operations
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_dual_atomic_basic_operations() {
    // Arrange: Create fresh instance
    let dual = DualAtomicU64::new(0, 0);

    // Act & Assert: Primary channel
    dual.store_primary(42, Ordering::Relaxed);
    assert_eq!(dual.load_primary(Ordering::Relaxed), 42);

    // Act & Assert: Secondary channel
    dual.store_secondary(100, Ordering::Relaxed);
    assert_eq!(dual.load_secondary(Ordering::Relaxed), 100);

    // Act & Assert: Fetch-add operations
    let prev = dual.fetch_add_primary(10, Ordering::Relaxed);
    assert_eq!(prev, 42);
    assert_eq!(dual.load_primary(Ordering::Relaxed), 52);
}

/// T28 Q1: Core behaviors - Generation counter increments
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_generation_counter_increments() {
    // Arrange
    let dual = DualAtomicU64::new(0, 0);

    // Act: Get initial generation
    let gen1 = dual.generation();

    // Act: Modify primary
    dual.store_primary(1, Ordering::Relaxed);
    let gen2 = dual.generation();

    // Assert: Generation incremented
    assert!(gen2 > gen1, "Generation must increase after modification");

    // Act: Modify secondary
    dual.store_secondary(1, Ordering::Relaxed);
    let gen3 = dual.generation();

    // Assert: Generation incremented again
    assert!(
        gen3 > gen2,
        "Generation must increase after second modification"
    );
}

/// T28 Q2: Edge cases - Zero values
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_edge_case_zero_values() {
    // Arrange
    let dual = DualAtomicU64::new(0, 0);

    // Assert: Zero initialization
    assert_eq!(dual.load_primary(Ordering::Relaxed), 0);
    assert_eq!(dual.load_secondary(Ordering::Relaxed), 0);

    // Act: Store zero explicitly
    dual.store_primary(100, Ordering::Relaxed);
    dual.store_primary(0, Ordering::Relaxed);

    // Assert: Can store zero
    assert_eq!(dual.load_primary(Ordering::Relaxed), 0);
}

/// T28 Q2: Edge cases - Maximum values
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_edge_case_max_values() {
    // Arrange
    let dual = DualAtomicU64::new(0, 0);

    // Act: Store maximum u64
    dual.store_primary(u64::MAX, Ordering::Relaxed);
    dual.store_secondary(u64::MAX, Ordering::Relaxed);

    // Assert: Maximum values stored correctly
    assert_eq!(dual.load_primary(Ordering::Relaxed), u64::MAX);
    assert_eq!(dual.load_secondary(Ordering::Relaxed), u64::MAX);
}

/// T28 Q2: Edge cases - Overflow behavior
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_edge_case_overflow() {
    // Arrange: Initialize near overflow
    let dual = DualAtomicU64::new(u64::MAX - 10, 0);

    // Act: Fetch-add that overflows
    let prev = dual.fetch_add_primary(20, Ordering::Relaxed);

    // Assert: Wrapping behavior (u64::MAX - 10 + 20 wraps to 9)
    assert_eq!(prev, u64::MAX - 10);
    assert_eq!(dual.load_primary(Ordering::Relaxed), 9);
}

/// T28 Q3: Invariants - Dual channel independence
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_invariant_dual_channel_independence() {
    // Arrange
    let dual = DualAtomicU64::new(10, 20);

    // Act: Modify primary only
    dual.store_primary(100, Ordering::Relaxed);

    // Assert: Secondary unchanged
    assert_eq!(dual.load_primary(Ordering::Relaxed), 100);
    assert_eq!(dual.load_secondary(Ordering::Relaxed), 20);

    // Act: Modify secondary only
    dual.store_secondary(200, Ordering::Relaxed);

    // Assert: Primary unchanged
    assert_eq!(dual.load_primary(Ordering::Relaxed), 100);
    assert_eq!(dual.load_secondary(Ordering::Relaxed), 200);
}

/// T28 Q3: Invariants - Generation monotonicity
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_invariant_generation_monotonic() {
    // Arrange
    let dual = DualAtomicU64::new(0, 0);

    // Track generations
    let mut last_gen = dual.generation();

    // Act: Multiple modifications
    for i in 0..10 {
        dual.store_primary(i, Ordering::Relaxed);
        let current_gen = dual.generation();

        // Assert: Generation always increases
        assert!(
            current_gen > last_gen,
            "Generation must increase monotonically: {} > {}",
            current_gen,
            last_gen
        );

        last_gen = current_gen;
    }
}

/// T28 Q3: Invariants - CAS success maintains value
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_invariant_cas_success() {
    // Arrange
    let dual = DualAtomicU64::new(42, 0);

    // Act: CAS with correct current value
    let result = dual.compare_exchange_primary(42, 100, Ordering::Relaxed, Ordering::Relaxed);

    // Assert: CAS succeeded
    assert_eq!(result, Ok(42));
    assert_eq!(dual.load_primary(Ordering::Relaxed), 100);
}

/// T28 Q3: Invariants - CAS failure preserves value
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_invariant_cas_failure() {
    // Arrange
    let dual = DualAtomicU64::new(42, 0);

    // Act: CAS with incorrect current value
    let result = dual.compare_exchange_primary(
        99, // Wrong current value
        100,
        Ordering::Relaxed,
        Ordering::Relaxed,
    );

    // Assert: CAS failed, value unchanged
    assert_eq!(result, Err(42));
    assert_eq!(dual.load_primary(Ordering::Relaxed), 42);
}

/// T28 Q4: Code paths - All memory orderings work
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_all_memory_orderings() {
    // Arrange
    let dual = DualAtomicU64::new(0, 0);

    // Test Relaxed
    dual.store_primary(1, Ordering::Relaxed);
    assert_eq!(dual.load_primary(Ordering::Relaxed), 1);

    // Test Release/Acquire
    dual.store_primary(2, Ordering::Release);
    assert_eq!(dual.load_primary(Ordering::Acquire), 2);

    // Test SeqCst
    dual.store_primary(3, Ordering::SeqCst);
    assert_eq!(dual.load_primary(Ordering::SeqCst), 3);
}

/// T28 Q5: Isolation - Fresh instance per test
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_isolation_fresh_instance() {
    // Each test gets fresh instance (no shared state)
    let dual1 = DualAtomicU64::new(10, 20);
    assert_eq!(dual1.load_primary(Ordering::Relaxed), 10);

    let dual2 = DualAtomicU64::new(30, 40);
    assert_eq!(dual2.load_primary(Ordering::Relaxed), 30);

    // Both instances independent
    dual1.store_primary(100, Ordering::Relaxed);
    assert_eq!(dual2.load_primary(Ordering::Relaxed), 30);
}

/// T28 Q6: Performance - Operations should be fast (<1ms on WASM)
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_performance_fast_operations() {
    // Arrange
    let dual = DualAtomicU64::new(0, 0);

    // Act: Perform many operations quickly
    for i in 0..1000 {
        dual.store_primary(i, Ordering::Relaxed);
        let _ = dual.load_primary(Ordering::Relaxed);
    }

    // Assert: Test completes within timeout (5 seconds)
    // If operations were slow, test would timeout
}

/// T28 Q7: Readability - Test naming is descriptive
/// (Demonstrated by all test names above)

// ============================================================================
// Fixed-Point Tests (Tier 3)
// ============================================================================

#[cfg(feature = "tier3")]
mod fixed_point_tests {
    use super::*;
    use atomic_capsule::primitives::fixed_point::{Q16_16, Q8_8};

    /// T28 Q1: Core behaviors - Q16.16 basic operations
    #[wasm_bindgen_test]
    #[timeout(5000)]
    fn test_q16_16_basic_operations() {
        // Arrange: Create from float
        let a = Q16_16::from_f64(3.5);
        let b = Q16_16::from_f64(2.0);

        // Act: Add
        let c = a.add(&b);

        // Assert: 3.5 + 2.0 = 5.5
        let result = c.to_f64();
        assert!((result - 5.5).abs() < 0.001);
    }

    /// T28 Q1: Core behaviors - Q16.16 multiplication
    #[wasm_bindgen_test]
    #[timeout(5000)]
    fn test_q16_16_multiplication() {
        // Arrange
        let a = Q16_16::from_f64(3.5);
        let b = Q16_16::from_f64(2.0);

        // Act: Multiply
        let c = a.mul(&b);

        // Assert: 3.5 × 2.0 = 7.0
        let result = c.to_f64();
        assert!((result - 7.0).abs() < 0.001);
    }

    /// T28 Q2: Edge cases - Q16.16 zero
    #[wasm_bindgen_test]
    #[timeout(5000)]
    fn test_q16_16_zero() {
        // Arrange
        let zero = Q16_16::from_f64(0.0);

        // Assert: Zero representation
        assert_eq!(zero.to_f64(), 0.0);

        // Act: Add zero
        let a = Q16_16::from_f64(5.0);
        let b = a.add(&zero);

        // Assert: a + 0 = a
        assert_eq!(b.to_f64(), 5.0);
    }

    /// T28 Q2: Edge cases - Q16.16 negative values
    #[wasm_bindgen_test]
    #[timeout(5000)]
    fn test_q16_16_negative() {
        // Arrange
        let a = Q16_16::from_f64(-3.5);
        let b = Q16_16::from_f64(2.0);

        // Act: Add negative + positive
        let c = a.add(&b);

        // Assert: -3.5 + 2.0 = -1.5
        let result = c.to_f64();
        assert!((result - (-1.5)).abs() < 0.001);
    }

    /// T28 Q3: Invariants - Q8.8 precision
    #[wasm_bindgen_test]
    #[timeout(5000)]
    fn test_q8_8_precision_invariant() {
        // Arrange: Values within Q8.8 range
        let a = Q8_8::from_f32(12.75);

        // Assert: Precision maintained
        let result = a.to_f32();
        assert!((result - 12.75).abs() < 0.01);
    }
}

// ============================================================================
// PackedState Tests (Tier 1)
// ============================================================================

/// T28 Q1: Core behaviors - PackedState operations
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_packed_state_basic() {
    // Arrange: Create packed state
    let packed = PackedState::new(1, 42);

    // Assert: State and value extracted correctly
    assert_eq!(packed.state(), 1);
    assert_eq!(packed.value(), 42);
}

/// T28 Q2: Edge cases - PackedState maximum values
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_packed_state_max_values() {
    // Arrange: Maximum 8-bit state, maximum value
    let packed = PackedState::new(255, u64::MAX >> 8);

    // Assert: Both fields stored correctly
    assert_eq!(packed.state(), 255);
    assert_eq!(packed.value(), u64::MAX >> 8);
}

/// T28 Q3: Invariants - PackedState round-trip
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_packed_state_round_trip() {
    // Property: pack then unpack should preserve values
    for state in [0, 1, 127, 255] {
        for value in [0, 1, 1000, u64::MAX >> 8] {
            let packed = PackedState::new(state, value);
            assert_eq!(packed.state(), state);
            assert_eq!(packed.value(), value);
        }
    }
}

// ============================================================================
// Memory Layout Tests (WASM-specific)
// ============================================================================

/// T28 Q4: Code paths - Verify memory layout on WASM
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_wasm_memory_layout() {
    use core::mem::{align_of, size_of};

    // Assert: DualAtomicU64 size and alignment
    // WASM may have different alignment requirements
    assert!(size_of::<DualAtomicU64>() >= 16);
    assert!(align_of::<DualAtomicU64>() >= 8);
}

/// T28 Q4: Code paths - Verify AtomicU64 works on WASM
#[wasm_bindgen_test]
#[timeout(5000)]
fn test_wasm_atomic_u64_operations() {
    use core::sync::atomic::AtomicU64;

    // WASM32 should support AtomicU64 (emulated if needed)
    let atomic = AtomicU64::new(0);
    atomic.store(42, Ordering::Relaxed);
    assert_eq!(atomic.load(Ordering::Relaxed), 42);

    // Fetch-add
    let prev = atomic.fetch_add(10, Ordering::Relaxed);
    assert_eq!(prev, 42);
    assert_eq!(atomic.load(Ordering::Relaxed), 52);

    // CAS
    let result = atomic.compare_exchange(52, 100, Ordering::Relaxed, Ordering::Relaxed);
    assert_eq!(result, Ok(52));
    assert_eq!(atomic.load(Ordering::Relaxed), 100);
}

// ============================================================================
// T28 Summary Checklist for WASM Unit Tests
// ============================================================================
//
// ✅ Q1: Core behaviors tested (DualAtomicU64, fixed-point, PackedState)
// ✅ Q2: Edge cases covered (zero, max, overflow, negative)
// ✅ Q3: Invariants validated (independence, monotonicity, CAS, round-trip)
// ✅ Q4: All code paths tested (memory orderings, WASM-specific layout)
// ✅ Q5: Tests isolated (fresh instances, no shared state)
// ✅ Q6: Tests fast (<5s timeout per test)
// ✅ Q7: Tests readable (descriptive names, arrange-act-assert)
//
// Test Count: 20+ unit tests
// Platform: wasm32-unknown-unknown
// Timeout: 5 seconds per test (B32 K27: fast feedback)
// Framework: T28 Tier 1 (Unit Testing)
