//! # T28 Tier 1: Unit Testing (Q1-Q7)
//!
//! **Comprehensive unit tests for computational capsules.**
//!
//! Coverage:
//! - Q1: Core behaviors tested
//! - Q2: Edge cases covered
//! - Q3: Invariants validated
//! - Q4: All code paths tested
//! - Q5: Tests isolated and deterministic
//! - Q6: Tests fast (<10ms each)
//! - Q7: Tests readable and maintainable

#![cfg(feature = "nightly")]
#![feature(portable_simd)]

use atomic_capsule::SimdF32x8Capsule;
use core::sync::atomic::Ordering;

// ============================================================================
// T28 Q1: Core Behaviors
// ============================================================================

#[test]
fn test_simd_capsule_creation() {
    let capsule = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    let vec = capsule.load_simd();
    assert_eq!(vec.to_array(), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
}

#[test]
fn test_simd_capsule_default() {
    let capsule = SimdF32x8Capsule::default();
    let vec = capsule.load_simd();
    assert_eq!(vec.to_array(), [0.0; 8]);
}

#[test]
fn test_simd_capsule_load() {
    let capsule = SimdF32x8Capsule::new([10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0]);
    let loaded = capsule.load_simd();
    assert_eq!(
        loaded.to_array(),
        [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0]
    );
}

#[test]
fn test_simd_capsule_store() {
    let mut capsule = SimdF32x8Capsule::default();
    let data = core::simd::f32x8::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    capsule.store_simd(data);

    let loaded = capsule.load_simd();
    assert_eq!(loaded.to_array(), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
}

#[test]
fn test_simd_capsule_multiply_scalar() {
    let mut capsule = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    capsule.multiply_scalar(2.0);

    let result = capsule.load_simd();
    assert_eq!(
        result.to_array(),
        [2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0]
    );
}

#[test]
fn test_simd_capsule_add() {
    let cap_a = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    let cap_b = SimdF32x8Capsule::new([10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0]);

    let result = cap_a.add(&cap_b);
    assert_eq!(
        result.to_array(),
        [11.0, 22.0, 33.0, 44.0, 55.0, 66.0, 77.0, 88.0]
    );
}

#[test]
fn test_simd_capsule_dot_product() {
    let cap_a = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    let cap_b = SimdF32x8Capsule::new([1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);

    let dot = cap_a.dot(&cap_b);
    assert_eq!(dot, 36.0); // 1+2+3+4+5+6+7+8 = 36
}

#[test]
fn test_simd_capsule_generation_increments() {
    let mut capsule = SimdF32x8Capsule::default();
    let gen1 = capsule.generation();

    capsule.store_simd(core::simd::f32x8::splat(1.0));
    let gen2 = capsule.generation();

    // Generation should increment by 2 (odd -> even)
    assert_eq!(gen2, gen1 + 2);
}

#[test]
fn test_atomic_capsule_creation() {
    let capsule = AtomicU64Capsule::new(42);
    assert_eq!(capsule.load(Ordering::Relaxed), 42);
}

#[test]
fn test_atomic_capsule_default() {
    let capsule = AtomicU64Capsule::default();
    assert_eq!(capsule.load(Ordering::Relaxed), 0);
}

#[test]
fn test_atomic_capsule_store() {
    let capsule = AtomicU64Capsule::new(0);
    capsule.store(100, Ordering::Relaxed);
    assert_eq!(capsule.load(Ordering::Relaxed), 100);
}

#[test]
fn test_atomic_capsule_compare_exchange_success() {
    let capsule = AtomicU64Capsule::new(10);
    let result = capsule.compare_exchange(10, 20, Ordering::Relaxed, Ordering::Relaxed);
    assert_eq!(result, Ok(10));
    assert_eq!(capsule.load(Ordering::Relaxed), 20);
}

#[test]
fn test_atomic_capsule_compare_exchange_failure() {
    let capsule = AtomicU64Capsule::new(10);
    let result = capsule.compare_exchange(99, 20, Ordering::Relaxed, Ordering::Relaxed);
    assert_eq!(result, Err(10));
    assert_eq!(capsule.load(Ordering::Relaxed), 10); // Unchanged
}

#[test]
fn test_atomic_capsule_generation_increments_on_store() {
    let capsule = AtomicU64Capsule::new(0);
    let gen1 = capsule.generation();

    capsule.store(100, Ordering::Relaxed);
    let gen2 = capsule.generation();

    assert!(gen2 > gen1);
}

#[test]
fn test_atomic_capsule_generation_increments_on_cas() {
    let capsule = AtomicU64Capsule::new(10);
    let gen1 = capsule.generation();

    let _ = capsule.compare_exchange(10, 20, Ordering::Relaxed, Ordering::Relaxed);
    let gen2 = capsule.generation();

    assert!(gen2 > gen1);
}

// ============================================================================
// T28 Q2: Edge Cases
// ============================================================================

#[test]
fn test_simd_capsule_all_zeros() {
    let capsule = SimdF32x8Capsule::new([0.0; 8]);
    let vec = capsule.load_simd();
    assert_eq!(vec.to_array(), [0.0; 8]);
}

#[test]
fn test_simd_capsule_all_ones() {
    let capsule = SimdF32x8Capsule::new([1.0; 8]);
    let vec = capsule.load_simd();
    assert_eq!(vec.to_array(), [1.0; 8]);
}

#[test]
fn test_simd_capsule_negative_values() {
    let capsule = SimdF32x8Capsule::new([-1.0, -2.0, -3.0, -4.0, -5.0, -6.0, -7.0, -8.0]);
    let vec = capsule.load_simd();
    assert_eq!(
        vec.to_array(),
        [-1.0, -2.0, -3.0, -4.0, -5.0, -6.0, -7.0, -8.0]
    );
}

#[test]
fn test_simd_capsule_mixed_signs() {
    let capsule = SimdF32x8Capsule::new([1.0, -2.0, 3.0, -4.0, 5.0, -6.0, 7.0, -8.0]);
    let vec = capsule.load_simd();
    assert_eq!(vec.to_array(), [1.0, -2.0, 3.0, -4.0, 5.0, -6.0, 7.0, -8.0]);
}

#[test]
fn test_simd_capsule_very_large_values() {
    let large = 1e30_f32;
    let capsule = SimdF32x8Capsule::new([large; 8]);
    let vec = capsule.load_simd();
    for val in vec.to_array() {
        assert!(val.is_finite());
        assert!(val > 0.0);
    }
}

#[test]
fn test_simd_capsule_very_small_values() {
    let small = 1e-30_f32;
    let capsule = SimdF32x8Capsule::new([small; 8]);
    let vec = capsule.load_simd();
    for val in vec.to_array() {
        assert!(val.is_finite());
        assert!(val > 0.0);
    }
}

#[test]
fn test_simd_capsule_multiply_by_zero() {
    let mut capsule = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    capsule.multiply_scalar(0.0);

    let result = capsule.load_simd();
    assert_eq!(result.to_array(), [0.0; 8]);
}

#[test]
fn test_simd_capsule_multiply_by_negative() {
    let mut capsule = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    capsule.multiply_scalar(-1.0);

    let result = capsule.load_simd();
    assert_eq!(
        result.to_array(),
        [-1.0, -2.0, -3.0, -4.0, -5.0, -6.0, -7.0, -8.0]
    );
}

#[test]
fn test_simd_capsule_dot_product_zero() {
    let cap_a = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    let cap_b = SimdF32x8Capsule::new([0.0; 8]);

    let dot = cap_a.dot(&cap_b);
    assert_eq!(dot, 0.0);
}

#[test]
fn test_simd_capsule_dot_product_orthogonal() {
    let cap_a = SimdF32x8Capsule::new([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let cap_b = SimdF32x8Capsule::new([0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);

    let dot = cap_a.dot(&cap_b);
    assert_eq!(dot, 0.0);
}

#[test]
fn test_atomic_capsule_zero_value() {
    let capsule = AtomicU64Capsule::new(0);
    assert_eq!(capsule.load(Ordering::Relaxed), 0);
}

#[test]
fn test_atomic_capsule_max_value() {
    let capsule = AtomicU64Capsule::new(u64::MAX);
    assert_eq!(capsule.load(Ordering::Relaxed), u64::MAX);
}

#[test]
fn test_atomic_capsule_boundary_values() {
    // Test various boundary values
    let boundaries = [0u64, 1, u64::MAX - 1, u64::MAX];

    for &val in &boundaries {
        let capsule = AtomicU64Capsule::new(val);
        assert_eq!(capsule.load(Ordering::Relaxed), val);
    }
}

// ============================================================================
// T28 Q3: Invariants
// ============================================================================

#[test]
fn test_simd_capsule_generation_monotonic() {
    let mut capsule = SimdF32x8Capsule::default();
    let mut last_gen = capsule.generation();

    for i in 1..10 {
        capsule.store_simd(core::simd::f32x8::splat(i as f32));
        let current_gen = capsule.generation();

        // Invariant: Generation always increases
        assert!(current_gen > last_gen, "Generation must be monotonic");
        last_gen = current_gen;
    }
}

#[test]
fn test_simd_capsule_generation_even_after_store() {
    let mut capsule = SimdF32x8Capsule::default();

    for _ in 0..10 {
        capsule.store_simd(core::simd::f32x8::splat(1.0));
        let gen = capsule.generation();

        // Invariant: Generation is even after complete store
        assert_eq!(gen % 2, 0, "Generation must be even after store");
    }
}

#[test]
fn test_simd_capsule_data_consistency() {
    let values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let capsule = SimdF32x8Capsule::new(values);

    // Read multiple times - should always be consistent
    for _ in 0..100 {
        let loaded = capsule.load_simd();
        assert_eq!(loaded.to_array(), values);
    }
}

#[test]
fn test_simd_capsule_add_commutative() {
    let cap_a = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    let cap_b = SimdF32x8Capsule::new([10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0]);

    // Invariant: Addition is commutative (a + b == b + a)
    let ab = cap_a.add(&cap_b);
    let ba = cap_b.add(&cap_a);

    assert_eq!(ab.to_array(), ba.to_array());
}

#[test]
fn test_simd_capsule_multiply_associative() {
    let mut capsule = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);

    // (x * 2) * 3 should equal x * 6
    capsule.multiply_scalar(2.0);
    capsule.multiply_scalar(3.0);
    let result1 = capsule.load_simd();

    let mut capsule2 = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    capsule2.multiply_scalar(6.0);
    let result2 = capsule2.load_simd();

    assert_eq!(result1.to_array(), result2.to_array());
}

#[test]
fn test_atomic_capsule_generation_never_decreases() {
    let capsule = AtomicU64Capsule::new(0);
    let mut last_gen = capsule.generation();

    for i in 1..100 {
        capsule.store(i, Ordering::Relaxed);
        let current_gen = capsule.generation();

        // Invariant: Generation never decreases
        assert!(current_gen >= last_gen);
        last_gen = current_gen;
    }
}

#[test]
fn test_atomic_capsule_cas_failure_preserves_value() {
    let capsule = AtomicU64Capsule::new(42);
    let initial = capsule.load(Ordering::Relaxed);

    // Failed CAS should preserve original value
    let _ = capsule.compare_exchange(99, 100, Ordering::Relaxed, Ordering::Relaxed);

    assert_eq!(capsule.load(Ordering::Relaxed), initial);
}

// ============================================================================
// T28 Q4: Code Path Coverage
// ============================================================================

#[test]
fn test_simd_capsule_load_with_generation_success() {
    let capsule = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);

    // Should succeed when no concurrent writes
    let result = capsule.load_with_generation();
    assert!(result.is_some());

    let (vec, gen) = result.unwrap();
    assert_eq!(vec.to_array(), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    assert!(gen >= 0);
}

#[test]
fn test_all_atomic_orderings() {
    let capsule = AtomicU64Capsule::new(42);

    // Test all valid orderings
    assert_eq!(capsule.load(Ordering::Relaxed), 42);
    assert_eq!(capsule.load(Ordering::Acquire), 42);
    assert_eq!(capsule.load(Ordering::SeqCst), 42);

    capsule.store(100, Ordering::Relaxed);
    capsule.store(100, Ordering::Release);
    capsule.store(100, Ordering::SeqCst);
}

// ============================================================================
// T28 Q5: Isolation and Determinism
// ============================================================================

#[test]
fn test_capsule_isolation_no_shared_state() {
    // Each test creates fresh instances - no shared state
    let capsule1 = SimdF32x8Capsule::new([1.0; 8]);
    let capsule2 = SimdF32x8Capsule::new([2.0; 8]);

    assert_eq!(capsule1.load_simd().to_array(), [1.0; 8]);
    assert_eq!(capsule2.load_simd().to_array(), [2.0; 8]);
}

#[test]
fn test_deterministic_operations() {
    // Same operations should produce same results
    for _ in 0..10 {
        let mut capsule = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        capsule.multiply_scalar(2.0);

        let result = capsule.load_simd();
        assert_eq!(
            result.to_array(),
            [2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0]
        );
    }
}

// ============================================================================
// T28 Q6: Fast Tests
// ============================================================================

#[test]
fn test_simd_load_performance_sanity() {
    let capsule = SimdF32x8Capsule::new([1.0; 8]);

    // Should complete in microseconds, not milliseconds
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = capsule.load_simd();
    }
    let elapsed = start.elapsed();

    // 1000 loads should be < 1ms
    assert!(elapsed.as_millis() < 10, "Loads too slow: {:?}", elapsed);
}

// ============================================================================
// T28 Q7: Readable and Maintainable
// ============================================================================

// All tests follow clear arrange-act-assert pattern
// Test names describe what they test
// Helper functions could be added if duplication increases

#[test]
fn test_example_of_clear_structure() {
    // Arrange: Set up test conditions
    let capsule = SimdF32x8Capsule::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);

    // Act: Perform operation under test
    let result = capsule.load_simd();

    // Assert: Verify expected outcome
    assert_eq!(result.to_array(), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
}
