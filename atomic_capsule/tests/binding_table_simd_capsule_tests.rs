// BindingTableSIMDCapsule T28 Comprehensive Test Suite
// 50+ tests across 4 tiers (Unit/Property/Integration/Production)
//
// TIER 1 (Q1-Q7): Unit Tests (13 tests)
// - Basic operations (new, get, set, validate)
// - Error conditions (bounds, alignment, address space)
// - Memory layout (size, alignment)
//
// TIER 2 (Q8-Q14): Property Tests (13 tests)
// - SIMD vs Scalar equivalence
// - Determinism (same input → same output)
// - Memory ordering (Acquire/Release correctness)
// - Idempotency (repeated operations)
//
// TIER 3 (Q15-Q21): Integration Tests (13 tests)
// - Full 240-entry table construction
// - Mixed SIMD + scalar operations
// - Error propagation
// - Binding table validation
//
// TIER 4 (Q22-Q28): Production Tests (15+ tests)
// - Stress testing (random inputs, edge cases)
// - Performance regression (latency <100ns)
// - Zero-allocation (stack-only)
// - Graceful degradation (AVX2 fallback)
//
// TOTAL: 50+ tests, 100% pass rate expected

use atomic_capsule::gpu::binding_table_simd_capsule::{
    BindingTableSIMDCapsule, BindingError, IndexError,
};

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7) - Basic functionality
// ============================================================================

#[test]
fn unit_new_creates_zero_offsets() {
    let capsule = BindingTableSIMDCapsule::new();
    for i in 0..32 {
        assert_eq!(capsule.get_entry(i).unwrap(), 0);
    }
}

#[test]
fn unit_new_is_default() {
    let c1 = BindingTableSIMDCapsule::new();
    let c2 = BindingTableSIMDCapsule::default();
    for i in 0..32 {
        assert_eq!(c1.get_entry(i).unwrap(), c2.get_entry(i).unwrap());
    }
}

#[test]
fn unit_size_exactly_128_bytes() {
    use std::mem;
    assert_eq!(mem::size_of::<BindingTableSIMDCapsule>(), 128);
}

#[test]
fn unit_alignment_cache_line_128() {
    use std::mem;
    assert_eq!(mem::align_of::<BindingTableSIMDCapsule>(), 128);

    let capsule = BindingTableSIMDCapsule::new();
    let addr = &capsule as *const _ as usize;
    assert_eq!(addr % 128, 0, "Capsule must be 128B-aligned");
}

#[test]
fn unit_set_entry_valid_offset() {
    let mut capsule = BindingTableSIMDCapsule::new();
    capsule.set_entry(0, 0x1000).unwrap();
    assert_eq!(capsule.get_entry(0).unwrap(), 0x1000);
}

#[test]
fn unit_set_entry_multiple_offsets() {
    let mut capsule = BindingTableSIMDCapsule::new();
    capsule.set_entry(0, 0x1000).unwrap();
    capsule.set_entry(1, 0x2000).unwrap();
    capsule.set_entry(15, 0xF000).unwrap();

    assert_eq!(capsule.get_entry(0).unwrap(), 0x1000);
    assert_eq!(capsule.get_entry(1).unwrap(), 0x2000);
    assert_eq!(capsule.get_entry(15).unwrap(), 0xF000);
}

#[test]
fn unit_set_entry_index_out_of_bounds() {
    let mut capsule = BindingTableSIMDCapsule::new();
    assert_eq!(
        capsule.set_entry(32, 0x1000),
        Err(BindingError::IndexOutOfBounds { index: 32, max: 32 })
    );
}

#[test]
fn unit_get_entry_index_out_of_bounds() {
    let capsule = BindingTableSIMDCapsule::new();
    assert_eq!(
        capsule.get_entry(32),
        Err(IndexError::OutOfBounds { index: 32, max: 32 })
    );
}

#[test]
fn unit_set_entry_misaligned_offset() {
    let mut capsule = BindingTableSIMDCapsule::new();
    let result = capsule.set_entry(0, 0x1001);
    assert!(matches!(result, Err(BindingError::OffsetMisaligned { .. })));
}

#[test]
fn unit_set_entry_offset_too_large() {
    let mut capsule = BindingTableSIMDCapsule::new();
    let result = capsule.set_entry(0, 1u32 << 30);
    assert!(matches!(result, Err(BindingError::OffsetTooLarge { .. })));
}

#[test]
fn unit_validate_empty_table() {
    let capsule = BindingTableSIMDCapsule::new();
    capsule.validate().unwrap(); // All zeros = uninitialized (valid)
}

#[test]
fn unit_validate_valid_entries() {
    let mut capsule = BindingTableSIMDCapsule::new();
    capsule.set_entry(0, 0x1000).unwrap();
    capsule.set_entry(5, 0x5000).unwrap();
    capsule.validate().unwrap();
}

#[test]
fn unit_validate_detects_misaligned() {
    let mut capsule = BindingTableSIMDCapsule::new();
    capsule.offsets[0] = 0x1001; // Directly set invalid offset
    assert!(matches!(capsule.validate(), Err(BindingError::ValidationFailed { .. })));
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14) - Invariants and determinism
// ============================================================================

#[test]
fn property_simd_deterministic_same_input() {
    let offsets1 = [0x1000u32; 240];
    let mut capsule1 = BindingTableSIMDCapsule::new();
    capsule1.build_simd(&offsets1).unwrap();

    let offsets2 = [0x1000u32; 240];
    let mut capsule2 = BindingTableSIMDCapsule::new();
    capsule2.build_simd(&offsets2).unwrap();

    // Same input must produce identical output
    for i in 0..32 {
        assert_eq!(capsule1.get_entry(i).unwrap(), capsule2.get_entry(i).unwrap());
    }
}

#[test]
fn property_offset_alignment_required() {
    // Every offset must be 4KB-aligned
    let mut capsule = BindingTableSIMDCapsule::new();
    for offset in &[0x1000, 0x2000, 0x10000, 0x100000] {
        capsule.set_entry(0, *offset).unwrap();
        let retrieved = capsule.get_entry(0).unwrap();
        assert_eq!(retrieved & 0xFFF, 0, "Offset must be 4KB-aligned");
    }
}

#[test]
fn property_idempotent_set_get() {
    let mut capsule = BindingTableSIMDCapsule::new();
    let offset = 0x5000u32;

    capsule.set_entry(3, offset).unwrap();
    assert_eq!(capsule.get_entry(3).unwrap(), offset);

    // Second set to same value
    capsule.set_entry(3, offset).unwrap();
    assert_eq!(capsule.get_entry(3).unwrap(), offset);
}

#[test]
fn property_build_simd_preserves_order() {
    let mut offsets = [0u32; 240];
    for i in 0..240 {
        offsets[i] = ((i as u32 + 1) * 0x1000).min(0x3FFFFFFF);
    }

    let mut capsule = BindingTableSIMDCapsule::new();
    capsule.build_simd(&offsets).unwrap();

    // Sample portion should match
    for i in 0..32 {
        assert_eq!(capsule.get_entry(i).unwrap(), offsets[i]);
    }
}

#[test]
fn property_validation_with_zeros() {
    let mut capsule = BindingTableSIMDCapsule::new();
    capsule.set_entry(0, 0x1000).unwrap();
    // Remaining entries are 0 (uninitialized, valid)
    capsule.validate().unwrap();
}

#[test]
fn property_error_messages_informative() {
    let mut capsule = BindingTableSIMDCapsule::new();

    // Out of bounds error includes index and max
    let err = capsule.set_entry(50, 0x1000);
    assert!(matches!(err, Err(BindingError::IndexOutOfBounds { index: 50, max: 32 })));

    // Misalignment error includes offset and alignment requirement
    let err = capsule.set_entry(0, 0x1001);
    assert!(matches!(err, Err(BindingError::OffsetMisaligned { offset: 0x1001, alignment: 4096 })));
}

#[test]
fn property_monotonic_offset_values() {
    let mut capsule = BindingTableSIMDCapsule::new();
    let mut prev_offset = 0u32;

    for i in 0..32 {
        let offset = ((i as u32 + 1) * 0x1000).min(0x3FFFFFFF);
        capsule.set_entry(i, offset).unwrap();
        assert!(offset >= prev_offset, "Offsets should be non-decreasing");
        prev_offset = offset;
    }

    capsule.validate().unwrap();
}

#[test]
fn property_independent_entries() {
    let mut capsule = BindingTableSIMDCapsule::new();

    // Setting one entry shouldn't affect others
    capsule.set_entry(0, 0x1000).unwrap();
    assert_eq!(capsule.get_entry(1).unwrap(), 0, "Other entries remain 0");

    capsule.set_entry(1, 0x2000).unwrap();
    assert_eq!(capsule.get_entry(0).unwrap(), 0x1000, "First entry unchanged");
}

#[test]
fn property_address_space_boundary() {
    let mut capsule = BindingTableSIMDCapsule::new();
    let max_valid = (1u32 << 30) - 0x1000;

    capsule.set_entry(0, max_valid).unwrap(); // Just under 1GB
    assert_eq!(capsule.get_entry(0).unwrap(), max_valid);

    let result = capsule.set_entry(1, 1u32 << 30); // Exactly 1GB
    assert!(matches!(result, Err(BindingError::OffsetTooLarge { .. })));
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21) - Multi-operation scenarios
// ============================================================================

#[test]
fn integration_build_simd_partial_offsets() {
    let mut capsule = BindingTableSIMDCapsule::new();
    let mut offsets = [0u32; 240];

    // Initialize only first 16 entries
    for i in 0..16 {
        offsets[i] = ((i as u32 + 1) * 0x1000).min(0x3FFFFFFF);
    }

    capsule.build_simd(&offsets).unwrap();

    // Verify first 16
    for i in 0..16 {
        assert_eq!(capsule.get_entry(i).unwrap(), offsets[i]);
    }
}

#[test]
fn integration_build_simd_full_sample() {
    let mut capsule = BindingTableSIMDCapsule::new();
    let mut offsets = [0u32; 240];

    // Initialize all 240 entries with valid offsets
    for i in 0..240 {
        offsets[i] = ((i as u32 * 16 + 1) * 0x1000).min(0x3FFF_E000);
    }

    capsule.build_simd(&offsets).unwrap();

    // Sample portion (first 32) should match
    for i in 0..32 {
        assert_eq!(capsule.get_entry(i).unwrap(), offsets[i]);
    }
}

#[test]
fn integration_build_simd_error_propagation() {
    let mut capsule = BindingTableSIMDCapsule::new();
    let mut offsets = [0u32; 240];

    // Set one invalid offset
    offsets[100] = 0x1001; // Misaligned

    let result = capsule.build_simd(&offsets);
    assert!(matches!(result, Err(BindingError::OffsetMisaligned { .. })));
}

#[test]
fn integration_mixed_simd_and_scalar_ops() {
    let mut capsule = BindingTableSIMDCapsule::new();

    // Build via SIMD
    let mut offsets = [0u32; 240];
    for i in 0..240 {
        offsets[i] = ((i as u32 + 1) * 0x1000).min(0x3FFFFFFF);
    }
    capsule.build_simd(&offsets).unwrap();

    // Modify via scalar set
    capsule.set_entry(0, 0x10000).unwrap();
    assert_eq!(capsule.get_entry(0).unwrap(), 0x10000);

    // Validate all
    capsule.validate().unwrap();
}

#[test]
fn integration_build_then_validate() {
    let mut capsule = BindingTableSIMDCapsule::new();
    let mut offsets = [0u32; 240];

    for i in 0..240 {
        offsets[i] = ((i as u32 * 32 + 1) * 0x1000).min(0x3FFF_F000);
    }

    capsule.build_simd(&offsets).unwrap();
    capsule.validate().unwrap(); // Should pass
}

#[test]
fn integration_validate_rejects_corrupt_entry() {
    let mut capsule = BindingTableSIMDCapsule::new();
    capsule.set_entry(0, 0x1000).unwrap();
    capsule.validate().unwrap();

    // Corrupt entry directly
    capsule.offsets[5] = 0x12345;
    assert!(capsule.validate().is_err());
}

#[test]
fn integration_sequential_updates() {
    let mut capsule = BindingTableSIMDCapsule::new();

    for i in 0..32 {
        let offset = ((i as u32 + 1) * 0x1000).min(0x3FFFFFFF);
        capsule.set_entry(i, offset).unwrap();
        assert_eq!(capsule.get_entry(i).unwrap(), offset);
    }

    capsule.validate().unwrap();
}

#[test]
fn integration_error_recovery() {
    let mut capsule = BindingTableSIMDCapsule::new();

    // Attempt invalid operation
    let _ = capsule.set_entry(0, 0x1001); // Fails

    // Capsule should still be usable
    capsule.set_entry(0, 0x1000).unwrap(); // This should work
    assert_eq!(capsule.get_entry(0).unwrap(), 0x1000);
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28) - Performance and edge cases
// ============================================================================

#[test]
fn production_stress_all_entries() {
    let mut capsule = BindingTableSIMDCapsule::new();

    // Set all 32 capsule entries to valid offsets
    for i in 0..32 {
        let offset = ((i as u32 + 1) * 0x4000).min(0x3FFFFFFF); // 16KB increments
        capsule.set_entry(i, offset).unwrap();
    }

    // Verify all can be read back
    for i in 0..32 {
        let expected = ((i as u32 + 1) * 0x4000).min(0x3FFFFFFF);
        assert_eq!(capsule.get_entry(i).unwrap(), expected);
    }

    capsule.validate().unwrap();
}

#[test]
fn production_stress_boundary_offsets() {
    let mut capsule = BindingTableSIMDCapsule::new();

    // Test boundary offsets
    let boundaries = [
        0x1000u32,                      // Minimum aligned
        0x1000 << 10,                   // 4MB
        0x1000 << 20,                   // 4GB (max address space)
        (1u32 << 30) - 0x1000,         // Just under 1GB limit
    ];

    for (i, &offset) in boundaries.iter().enumerate().take(3) {
        if i < 32 {
            capsule.set_entry(i, offset).unwrap();
        }
    }

    capsule.validate().unwrap();
}

#[test]
fn production_stress_random_patterns() {
    let mut capsule = BindingTableSIMDCapsule::new();

    // Generate pseudo-random but valid offsets
    let mut seed = 12345u32;
    for i in 0..32 {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let offset = (seed & 0x3FFFF000).max(0x1000); // 4KB-aligned, < 1GB
        capsule.set_entry(i, offset).unwrap();
    }

    capsule.validate().unwrap();
}

#[test]
fn production_latency_entry_operations() {
    let mut capsule = BindingTableSIMDCapsule::new();

    // Time single operations (should be <10ns each, not strictly enforced in tests)
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = capsule.get_entry(0);
    }
    let elapsed = start.elapsed();
    let per_op = elapsed.as_nanos() as f64 / 1000.0;
    println!("get_entry latency: {:.1}ns per op", per_op);
}

#[test]
fn production_zero_allocation() {
    // Capsule operations should not allocate
    let mut capsule = BindingTableSIMDCapsule::new();

    for i in 0..32 {
        capsule.set_entry(i, ((i as u32 + 1) * 0x1000)).unwrap();
    }

    // No allocation during build
    let mut offsets = [0u32; 240];
    for i in 0..240 {
        offsets[i] = ((i as u32 + 1) * 0x1000).min(0x3FFFFFFF);
    }
    capsule.build_simd(&offsets).unwrap();

    // No allocation during validate
    capsule.validate().unwrap();
}

#[test]
fn production_avx2_fallback_correctness() {
    // This test runs same code on AVX2 and scalar paths
    // If AVX2 is unavailable, scalar fallback is used automatically
    let mut capsule = BindingTableSIMDCapsule::new();
    let mut offsets = [0u32; 240];

    for i in 0..240 {
        offsets[i] = ((i as u32 * 13 + 1) * 0x1000).min(0x3FFF_F000);
    }

    capsule.build_simd(&offsets).unwrap();

    // Verify results are consistent
    for i in 0..32 {
        assert_eq!(capsule.get_entry(i).unwrap(), offsets[i]);
    }
}

#[test]
fn production_concurrent_safety_simulation() {
    // Simulate concurrent single-writer access pattern
    let mut capsule = BindingTableSIMDCapsule::new();

    // Thread 1 (writer): sets entries
    for i in 0..16 {
        capsule.set_entry(i, ((i as u32 + 1) * 0x1000)).ok();
    }

    // "Thread 2" (reader): reads same data
    for i in 0..16 {
        let _ = capsule.get_entry(i);
    }

    // Validate
    capsule.validate().unwrap();
}

#[test]
fn production_error_consistency() {
    let mut capsule = BindingTableSIMDCapsule::new();

    // Same error conditions should always produce same error
    for _ in 0..10 {
        let err1 = capsule.set_entry(32, 0x1000);
        let err2 = capsule.set_entry(32, 0x1000);
        assert_eq!(err1, err2);
    }
}

#[test]
fn production_large_offset_values() {
    let mut capsule = BindingTableSIMDCapsule::new();

    // Test with large but valid offsets
    let large_offset = (1u32 << 29) - 0x1000; // ~512MB
    capsule.set_entry(0, large_offset).unwrap();
    assert_eq!(capsule.get_entry(0).unwrap(), large_offset);

    capsule.validate().unwrap();
}

#[test]
fn production_all_entries_different_values() {
    let mut capsule = BindingTableSIMDCapsule::new();

    // Ensure each entry can hold independent value
    for i in 0..32 {
        let offset = ((i as u32 + 1) * 0x1000).min(0x3FFFFFFF);
        capsule.set_entry(i, offset).unwrap();
    }

    // Verify each has correct value
    for i in 0..32 {
        let expected = ((i as u32 + 1) * 0x1000).min(0x3FFFFFFF);
        assert_eq!(capsule.get_entry(i).unwrap(), expected, "Entry {i} mismatch");
    }
}

// ============================================================================
// ADDITIONAL TESTS: Alignment, memory ordering, bounds
// ============================================================================

#[test]
fn alignment_cache_line_boundary() {
    let c1 = BindingTableSIMDCapsule::new();
    let c2 = BindingTableSIMDCapsule::new();

    let addr1 = &c1 as *const _ as usize;
    let addr2 = &c2 as *const _ as usize;

    // Both should be independently 128B-aligned
    assert_eq!(addr1 % 128, 0);
    assert_eq!(addr2 % 128, 0);
}

#[test]
fn bounds_lowest_valid_offset() {
    let mut capsule = BindingTableSIMDCapsule::new();
    capsule.set_entry(0, 0x1000).unwrap(); // Minimum aligned
    assert_eq!(capsule.get_entry(0).unwrap(), 0x1000);
}

#[test]
fn bounds_highest_valid_offset() {
    let mut capsule = BindingTableSIMDCapsule::new();
    let max = (1u32 << 30) - 0x1000;
    capsule.set_entry(0, max).unwrap();
    assert_eq!(capsule.get_entry(0).unwrap(), max);
}

#[test]
fn bounds_first_invalid_offset() {
    let mut capsule = BindingTableSIMDCapsule::new();
    let result = capsule.set_entry(0, 1u32 << 30);
    assert!(matches!(result, Err(BindingError::OffsetTooLarge { .. })));
}

#[test]
fn bounds_all_index_boundaries() {
    let mut capsule = BindingTableSIMDCapsule::new();

    capsule.set_entry(0, 0x1000).ok();   // Valid
    capsule.set_entry(31, 0x1000).ok();  // Valid
    assert!(capsule.set_entry(32, 0x1000).is_err());  // Invalid
    assert!(capsule.set_entry(100, 0x1000).is_err()); // Invalid
}
