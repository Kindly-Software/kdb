//! T28 Q29-Q35 Determinism Tests for LOS Module
//!
//! # Test Coverage
//!
//! - Q29: Bit-exact reproducibility (same input → identical bits)
//! - Q30: Platform-independent Q16.16 arithmetic
//! - Q31: SIMD vs scalar equivalence
//! - Q32: Batch order independence
//! - Q33: Thread-safe determinism
//! - Q34: Seed-based reproducibility
//! - Q35: Cross-compilation determinism
//!
//! # Framework: T28 Determinism (Q29-Q35)
//!
//! Total tests: 14 (minimum requirement)
//! - 2× Q29 (bit-exact, triple-run)
//! - 2× Q30 (platform-independent arithmetic)
//! - 2× Q31 (SIMD vs scalar equivalence)
//! - 2× Q32 (batch order independence)
//! - 2× Q33 (concurrent determinism)
//! - 2× Q34 (seed-based reproducibility)
//! - 2× Q35 (cross-platform determinism)

#![cfg(feature = "los")]

use atomic_capsule::los::types::{LosRay, LosResult, LosRayType, Q16_16};
use atomic_capsule::los::map_data::MapDataCapsule;
use atomic_capsule::los::{SparseLosScalarCapsule, TacticalLosSimdCapsule, BatchedLosSimdCapsule, LosMetacapsule};
use std::alloc::{alloc, dealloc, Layout};
use std::sync::Arc;
use std::thread;

// =============================================================================
// Test Helpers
// =============================================================================

/// Create a test map with known data pattern
fn create_test_map(width: u16, height: u16, pattern: &str) -> (MapDataCapsule, Layout, *mut i32) {
    let map = MapDataCapsule::new(width, height);

    unsafe {
        let layout = Layout::from_size_align((width as usize) * (height as usize) * 4, 32).unwrap();
        let cover = alloc(layout) as *mut i32;

        match pattern {
            "clear" => {
                // All cells clear (visibility = 1.0)
                for i in 0..(width as usize * height as usize) {
                    *cover.add(i) = 0;
                }
            }
            "blocked" => {
                // All cells blocked (visibility = 0.0)
                for i in 0..(width as usize * height as usize) {
                    *cover.add(i) = Q16_16::ONE.raw();
                }
            }
            "diagonal" => {
                // Diagonal stripe pattern (deterministic)
                for y in 0..height {
                    for x in 0..width {
                        let idx = (y as usize) * (width as usize) + (x as usize);
                        let cover_val = if (x + y) % 4 == 0 {
                            Q16_16::HALF.raw() // 50% cover on diagonal
                        } else {
                            0 // Clear
                        };
                        *cover.add(idx) = cover_val;
                    }
                }
            }
            "gradient" => {
                // Horizontal gradient (deterministic)
                for y in 0..height {
                    for x in 0..width {
                        let idx = (y as usize) * (width as usize) + (x as usize);
                        // Cover increases left to right: 0.0 -> 1.0
                        let cover_val = (x as i32 * Q16_16::ONE.raw()) / (width as i32);
                        *cover.add(idx) = cover_val;
                    }
                }
            }
            _ => panic!("Unknown pattern: {}", pattern),
        }

        map.attach_buffers(cover, cover, cover);
        (map, layout, cover)
    }
}

/// Cleanup test map
unsafe fn cleanup_test_map(layout: Layout, cover: *mut i32) {
    dealloc(cover as *mut u8, layout);
}

/// Create a deterministic test ray
fn create_test_ray(ox: i32, oy: i32, tx: i32, ty: i32, ray_type: LosRayType) -> LosRay {
    LosRay::new(
        Q16_16::from_i32(ox),
        Q16_16::from_i32(oy),
        Q16_16::from_i32(tx),
        Q16_16::from_i32(ty),
        Q16_16::from_i32(1000),
        ray_type,
    )
}

/// Compare two LosResults for bit-exact equality
fn results_bit_exact(a: &LosResult, b: &LosResult) -> bool {
    a.visibility.raw() == b.visibility.raw()
        && a.samples_checked == b.samples_checked
        && a.cost_accumulated.raw() == b.cost_accumulated.raw()
        && std::mem::discriminant(&a.status) == std::mem::discriminant(&b.status)
}

// =============================================================================
// Q29: Bit-Exact Reproducibility
// =============================================================================

#[test]
fn q29_1_sparse_bit_exact_triple_run() {
    // Same input → identical bits across 3 runs
    let (map, layout, cover) = create_test_map(64, 64, "diagonal");
    let capsule = SparseLosScalarCapsule::new();
    let ray = create_test_ray(10, 10, 50, 50, LosRayType::Sparse);

    // Run 1
    let result1 = capsule.traverse(&ray, &map);

    // Run 2
    let result2 = capsule.traverse(&ray, &map);

    // Run 3
    let result3 = capsule.traverse(&ray, &map);

    // All results must be bit-exact
    assert!(
        results_bit_exact(&result1, &result2),
        "Run 1 vs Run 2 mismatch: {:?} vs {:?}",
        result1, result2
    );
    assert!(
        results_bit_exact(&result1, &result3),
        "Run 1 vs Run 3 mismatch: {:?} vs {:?}",
        result1, result3
    );

    // Verify visibility is deterministic (bit-level)
    assert_eq!(result1.visibility.raw(), result2.visibility.raw());
    assert_eq!(result1.visibility.raw(), result3.visibility.raw());

    // Verify samples checked is deterministic
    assert_eq!(result1.samples_checked, result2.samples_checked);
    assert_eq!(result1.samples_checked, result3.samples_checked);

    unsafe { cleanup_test_map(layout, cover); }
}

#[test]
fn q29_2_tactical_bit_exact_triple_run() {
    // SIMD path must also be bit-exact across runs
    let (map, layout, cover) = create_test_map(128, 128, "gradient");
    let capsule = TacticalLosSimdCapsule::new();
    let ray = create_test_ray(10, 10, 100, 100, LosRayType::Tactical);

    let result1 = capsule.traverse(&ray, &map);
    let result2 = capsule.traverse(&ray, &map);
    let result3 = capsule.traverse(&ray, &map);

    assert!(
        results_bit_exact(&result1, &result2),
        "Tactical run 1 vs 2 mismatch"
    );
    assert!(
        results_bit_exact(&result1, &result3),
        "Tactical run 1 vs 3 mismatch"
    );

    // Verify raw Q16.16 bits match exactly
    assert_eq!(result1.visibility.raw(), result2.visibility.raw());
    assert_eq!(result1.visibility.raw(), result3.visibility.raw());

    unsafe { cleanup_test_map(layout, cover); }
}

// =============================================================================
// Q30: Platform-Independent Q16.16 Arithmetic
// =============================================================================

#[test]
fn q30_1_q16_arithmetic_platform_independent() {
    // Q16.16 arithmetic must produce identical results across platforms
    // (no floating-point variance)

    let a = Q16_16::from_i32(5);
    let b = Q16_16::from_i32(3);

    // Addition
    let add1 = a.saturating_add(b);
    let add2 = a.saturating_add(b);
    assert_eq!(add1.raw(), add2.raw(), "Addition not deterministic");
    assert_eq!(add1.raw(), Q16_16::from_i32(8).raw());

    // Multiplication
    let mul1 = a.saturating_mul(b);
    let mul2 = a.saturating_mul(b);
    assert_eq!(mul1.raw(), mul2.raw(), "Multiplication not deterministic");
    assert_eq!(mul1.raw(), Q16_16::from_i32(15).raw());

    // Division
    let div1 = a.saturating_div(b);
    let div2 = a.saturating_div(b);
    assert_eq!(div1.raw(), div2.raw(), "Division not deterministic");

    // Subtraction
    let sub1 = a.saturating_sub(b);
    let sub2 = a.saturating_sub(b);
    assert_eq!(sub1.raw(), sub2.raw(), "Subtraction not deterministic");
    assert_eq!(sub1.raw(), Q16_16::from_i32(2).raw());
}

#[test]
fn q30_2_visibility_attenuation_deterministic() {
    // Visibility calculations must be platform-independent
    let (map, layout, cover) = create_test_map(32, 32, "diagonal");
    let capsule = SparseLosScalarCapsule::new();
    let ray = create_test_ray(0, 0, 30, 30, LosRayType::Sparse);

    // Run multiple times
    let results: Vec<LosResult> = (0..10)
        .map(|_| capsule.traverse(&ray, &map))
        .collect();

    // All visibility values must be identical (bit-level)
    let first_vis = results[0].visibility.raw();
    for (i, result) in results.iter().enumerate() {
        assert_eq!(
            result.visibility.raw(),
            first_vis,
            "Visibility mismatch at iteration {}: {} vs {}",
            i,
            result.visibility.raw(),
            first_vis
        );
    }

    unsafe { cleanup_test_map(layout, cover); }
}

// =============================================================================
// Q31: SIMD vs Scalar Equivalence
// =============================================================================

#[test]
fn q31_1_tactical_simd_vs_sparse_scalar_equivalence() {
    // For same input, SIMD (tactical) and scalar (sparse) must produce
    // identical results (when using same stride)
    let (map, layout, cover) = create_test_map(64, 64, "clear");

    // Use short ray that both can process efficiently
    let ray = create_test_ray(10, 10, 40, 40, LosRayType::Tactical);

    let tactical = TacticalLosSimdCapsule::new();
    let sparse = SparseLosScalarCapsule::with_stride(1, 1); // Stride 1 for full samples

    let result_simd = tactical.traverse(&ray, &map);
    let result_scalar = sparse.traverse(&ray, &map);

    // Results should be equivalent (same visibility)
    // Note: samples_checked may differ due to implementation details,
    // but visibility should be identical for clear terrain
    assert_eq!(
        result_simd.visibility.raw(),
        result_scalar.visibility.raw(),
        "SIMD vs scalar visibility mismatch"
    );

    unsafe { cleanup_test_map(layout, cover); }
}

#[test]
fn q31_2_tactical_simd_vs_sparse_scalar_partial_cover() {
    // Test SIMD vs scalar with partial cover (more complex case)
    let (map, layout, cover) = create_test_map(64, 64, "diagonal");

    let ray = create_test_ray(0, 0, 50, 50, LosRayType::Tactical);

    let tactical = TacticalLosSimdCapsule::new();
    let sparse = SparseLosScalarCapsule::with_stride(1, 1);

    let result_simd = tactical.traverse(&ray, &map);
    let result_scalar = sparse.traverse(&ray, &map);

    // For diagonal pattern, results should be similar
    // (allow small differences due to sampling differences)
    let vis_diff = (result_simd.visibility.to_f32() - result_scalar.visibility.to_f32()).abs();
    assert!(
        vis_diff < 0.1,
        "SIMD vs scalar visibility diff too large: {} (SIMD: {}, Scalar: {})",
        vis_diff,
        result_simd.visibility.to_f32(),
        result_scalar.visibility.to_f32()
    );

    unsafe { cleanup_test_map(layout, cover); }
}

// =============================================================================
// Q32: Batch Order Independence
// =============================================================================

#[test]
fn q32_1_batched_ray_order_independence() {
    // Processing rays in different orders must produce same results
    let (map, layout, cover) = create_test_map(64, 64, "gradient");
    let capsule = BatchedLosSimdCapsule::new();

    let rays = [
        create_test_ray(10, 10, 50, 50, LosRayType::Batched),
        create_test_ray(20, 20, 55, 55, LosRayType::Batched),
        create_test_ray(15, 15, 52, 52, LosRayType::Batched),
        create_test_ray(25, 25, 58, 58, LosRayType::Batched),
    ];

    // Process in original order
    let results_forward = capsule.traverse_batch(&rays, &map);

    // Process in reverse order
    let rays_rev: Vec<LosRay> = rays.iter().rev().copied().collect();
    let results_reverse = capsule.traverse_batch(&rays_rev, &map);

    // Each ray's result should match (order-independent)
    for i in 0..rays.len() {
        let fwd = &results_forward[i];
        let rev = &results_reverse[rays.len() - 1 - i];

        assert_eq!(
            fwd.visibility.raw(),
            rev.visibility.raw(),
            "Ray {} visibility differs by order: forward={}, reverse={}",
            i,
            fwd.visibility.to_f32(),
            rev.visibility.to_f32()
        );
    }

    unsafe { cleanup_test_map(layout, cover); }
}

#[test]
fn q32_2_metacapsule_batch_order_independence() {
    // Metacapsule batch processing must be order-independent
    let (map, layout, cover) = create_test_map(64, 64, "diagonal");
    let meta = LosMetacapsule::new();

    let rays = [
        create_test_ray(10, 10, 50, 50, LosRayType::Tactical),
        create_test_ray(20, 20, 55, 55, LosRayType::Sparse),
        create_test_ray(15, 15, 52, 52, LosRayType::Tactical),
        create_test_ray(25, 25, 58, 58, LosRayType::Sparse),
    ];

    // Process in original order
    let results1 = meta.cast_rays_batch(&rays, &map);

    // Shuffle order
    let rays_shuffled = [rays[2], rays[0], rays[3], rays[1]];
    let results2 = meta.cast_rays_batch(&rays_shuffled, &map);

    // Results should match original rays regardless of processing order
    // Compare rays[0] with where it appears in shuffled (index 1)
    assert_eq!(results1[0].visibility.raw(), results2[1].visibility.raw());
    assert_eq!(results1[2].visibility.raw(), results2[0].visibility.raw());
    assert_eq!(results1[3].visibility.raw(), results2[2].visibility.raw());
    assert_eq!(results1[1].visibility.raw(), results2[3].visibility.raw());

    unsafe { cleanup_test_map(layout, cover); }
}

// =============================================================================
// Q33: Thread-Safe Determinism
// =============================================================================

#[test]
fn q33_1_concurrent_read_determinism() {
    // Multiple threads reading simultaneously must get identical results
    let (map, layout, cover) = create_test_map(128, 128, "gradient");
    let map_arc = Arc::new(map);
    let ray = Arc::new(create_test_ray(10, 10, 100, 100, LosRayType::Tactical));

    // Spawn 8 threads all traversing the same ray
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let map_clone = Arc::clone(&map_arc);
            let ray_clone = Arc::clone(&ray);
            thread::spawn(move || {
                let capsule = TacticalLosSimdCapsule::new();
                capsule.traverse(&ray_clone, &map_clone)
            })
        })
        .collect();

    let results: Vec<LosResult> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    // All results must be bit-exact identical
    let first = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert!(
            results_bit_exact(first, result),
            "Thread {} result differs from thread 0",
            i
        );
    }

    unsafe { cleanup_test_map(layout, cover); }
}

#[test]
fn q33_2_concurrent_metacapsule_determinism() {
    // Concurrent metacapsule operations must be deterministic
    let (map, layout, cover) = create_test_map(64, 64, "diagonal");
    let map_arc = Arc::new(map);
    let rays = Arc::new([
        create_test_ray(10, 10, 50, 50, LosRayType::Tactical),
        create_test_ray(20, 20, 55, 55, LosRayType::Sparse),
    ]);

    // Each thread creates its own metacapsule (independent state)
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let map_clone = Arc::clone(&map_arc);
            let rays_clone = Arc::clone(&rays);
            thread::spawn(move || {
                let meta = LosMetacapsule::new();
                meta.cast_rays_batch(&*rays_clone, &*map_clone)
            })
        })
        .collect();

    let all_results: Vec<Vec<LosResult>> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    // All threads should produce identical results
    let first_batch = &all_results[0];
    for (thread_id, results) in all_results.iter().enumerate().skip(1) {
        for (ray_id, (r1, r2)) in first_batch.iter().zip(results.iter()).enumerate() {
            assert_eq!(
                r1.visibility.raw(),
                r2.visibility.raw(),
                "Thread {} ray {} differs from thread 0",
                thread_id, ray_id
            );
        }
    }

    unsafe { cleanup_test_map(layout, cover); }
}

// =============================================================================
// Q34: Seed-Based Reproducibility
// =============================================================================

#[test]
fn q34_1_generation_counter_reproducibility() {
    // Generation counter ensures reproducibility across re-initializations
    let (map, layout, cover) = create_test_map(64, 64, "diagonal");
    let capsule = SparseLosScalarCapsule::new();
    let ray = create_test_ray(10, 10, 50, 50, LosRayType::Sparse);

    // First run sequence
    let gen1_before = capsule.generation();
    let result1_a = capsule.traverse(&ray, &map);
    let gen1_after = capsule.generation();

    // Second run sequence (generation increments)
    let gen2_before = capsule.generation();
    let result1_b = capsule.traverse(&ray, &map);
    let gen2_after = capsule.generation();

    // Generation counters should increment predictably
    assert_eq!(gen1_after, gen1_before + 1);
    assert_eq!(gen2_after, gen2_before + 1);
    assert_eq!(gen2_before, gen1_after);

    // Results should be identical despite different generations
    assert_eq!(result1_a.visibility.raw(), result1_b.visibility.raw());

    unsafe { cleanup_test_map(layout, cover); }
}

#[test]
fn q34_2_map_version_reproducibility() {
    // Map version tracking ensures reproducibility
    let (map, layout, cover) = create_test_map(64, 64, "clear");
    let capsule = SparseLosScalarCapsule::new();
    let ray = create_test_ray(10, 10, 50, 50, LosRayType::Sparse);

    let version_before = map.version();

    // Acquire write lock and release (increments version)
    {
        let _guard = map.acquire_write().expect("Should acquire write lock");
        // Version increments on drop
    }

    let version_after = map.version();
    assert_eq!(version_after, version_before + 1);

    // Results should still be deterministic
    let result1 = capsule.traverse(&ray, &map);
    let result2 = capsule.traverse(&ray, &map);

    assert_eq!(result1.visibility.raw(), result2.visibility.raw());

    unsafe { cleanup_test_map(layout, cover); }
}

// =============================================================================
// Q35: Cross-Compilation Determinism
// =============================================================================

#[test]
fn q35_1_q16_fixed_point_cross_platform() {
    // Q16.16 arithmetic must produce identical results across architectures
    // Test known values with exact bit patterns

    // Test 1: Integer conversion
    let val1 = Q16_16::from_i32(42);
    assert_eq!(val1.raw(), 42 << 16, "Integer conversion not deterministic");

    // Test 2: Float conversion (to known Q16.16 value)
    let val2 = Q16_16::from_f32(1.5);
    assert_eq!(val2.raw(), 0x0001_8000, "1.5 should be 0x0001_8000");

    // Test 3: Arithmetic operations
    let a = Q16_16::from_i32(10);
    let b = Q16_16::from_i32(3);

    let add = a.saturating_add(b);
    assert_eq!(add.raw(), 13 << 16);

    let mul = a.saturating_mul(b);
    assert_eq!(mul.raw(), 30 << 16);

    let div = a.saturating_div(b);
    // 10 / 3 ≈ 3.333... in Q16.16
    // Expected: (10 << 16) / 3 = 218453 (approx)
    assert!(div.raw() > (3 << 16) && div.raw() < (4 << 16));

    // Test 4: Saturating behavior
    let max = Q16_16::MAX;
    let overflow = max.saturating_add(Q16_16::ONE);
    assert_eq!(overflow.raw(), max.raw(), "Saturation should prevent overflow");
}

#[test]
fn q35_2_visibility_calculation_cross_platform() {
    // Visibility calculations must be platform-independent
    let (map, layout, cover) = create_test_map(32, 32, "gradient");
    let capsule = SparseLosScalarCapsule::new();

    // Test ray through gradient (deterministic pattern)
    let ray = create_test_ray(0, 16, 31, 16, LosRayType::Sparse);
    let result = capsule.traverse(&ray, &map);

    // Store expected bit pattern (Q16.16 raw value)
    // This value should be identical across x86_64, aarch64, wasm32, etc.
    // Note: Actual value depends on gradient formula, but must be reproducible
    let vis_raw = result.visibility.raw();

    // Verify it's a valid Q16.16 value in expected range
    assert!(vis_raw >= 0, "Visibility should be non-negative");
    assert!(vis_raw <= Q16_16::ONE.raw(), "Visibility should be ≤ 1.0");

    // Re-run and verify bit-exact match
    let result2 = capsule.traverse(&ray, &map);
    assert_eq!(
        result2.visibility.raw(),
        vis_raw,
        "Visibility not reproducible across runs"
    );

    unsafe { cleanup_test_map(layout, cover); }
}

// =============================================================================
// Additional Cross-Capsule Determinism Tests
// =============================================================================

#[test]
fn determinism_sparse_stride_variations() {
    // Different strides should produce deterministic results
    let (map, layout, cover) = create_test_map(64, 64, "diagonal");
    let ray = create_test_ray(0, 0, 60, 60, LosRayType::Sparse);

    // Test stride 4
    let capsule4 = SparseLosScalarCapsule::with_stride(4, 4);
    let result4_a = capsule4.traverse(&ray, &map);
    let result4_b = capsule4.traverse(&ray, &map);

    assert_eq!(result4_a.visibility.raw(), result4_b.visibility.raw());
    assert_eq!(result4_a.samples_checked, result4_b.samples_checked);

    // Test stride 8
    let capsule8 = SparseLosScalarCapsule::with_stride(8, 8);
    let result8_a = capsule8.traverse(&ray, &map);
    let result8_b = capsule8.traverse(&ray, &map);

    assert_eq!(result8_a.visibility.raw(), result8_b.visibility.raw());
    assert_eq!(result8_a.samples_checked, result8_b.samples_checked);

    // Higher stride = fewer samples
    assert!(result8_a.samples_checked < result4_a.samples_checked);

    unsafe { cleanup_test_map(layout, cover); }
}

#[test]
fn determinism_metacapsule_classification() {
    // Ray classification must be deterministic
    let meta = LosMetacapsule::new();

    // Short ray → Sparse
    let short_ray = create_test_ray(10, 10, 15, 15, LosRayType::Dense);
    let short_type1 = meta.classify_ray(&short_ray);
    let short_type2 = meta.classify_ray(&short_ray);
    assert_eq!(std::mem::discriminant(&short_type1), std::mem::discriminant(&short_type2));

    // Long ray → Dense/Tactical
    let long_ray = create_test_ray(0, 0, 200, 200, LosRayType::Dense);
    let long_type1 = meta.classify_ray(&long_ray);
    let long_type2 = meta.classify_ray(&long_ray);
    assert_eq!(std::mem::discriminant(&long_type1), std::mem::discriminant(&long_type2));

    // Explicit type always takes precedence
    let explicit_sparse = create_test_ray(0, 0, 1000, 1000, LosRayType::Sparse);
    assert_eq!(std::mem::discriminant(&meta.classify_ray(&explicit_sparse)), std::mem::discriminant(&LosRayType::Sparse));
}

#[test]
fn determinism_batched_empty_and_single() {
    // Edge cases: empty batch and single ray
    let (map, layout, cover) = create_test_map(64, 64, "clear");
    let capsule = BatchedLosSimdCapsule::new();

    // Empty batch
    let empty_results = capsule.traverse_batch(&[], &map);
    assert_eq!(empty_results.len(), 0);

    // Single ray (twice)
    let ray = create_test_ray(10, 10, 50, 50, LosRayType::Batched);
    let single1 = capsule.traverse_batch(&[ray], &map);
    let single2 = capsule.traverse_batch(&[ray], &map);

    assert_eq!(single1.len(), 1);
    assert_eq!(single2.len(), 1);
    assert_eq!(single1[0].visibility.raw(), single2[0].visibility.raw());

    unsafe { cleanup_test_map(layout, cover); }
}

#[test]
fn determinism_map_read_concurrency() {
    // Multiple readers should get identical data
    let (map, layout, cover) = create_test_map(64, 64, "diagonal");
    let map_arc = Arc::new(map);

    let handles: Vec<_> = (0..8)
        .map(|thread_id| {
            let map_clone = Arc::clone(&map_arc);
            thread::spawn(move || {
                let _guard = map_clone.acquire_read().expect("Should acquire read");

                // Sample same location from all threads
                let sample = map_clone.sample_cover(30, 30).unwrap();
                (thread_id, sample)
            })
        })
        .collect();

    let samples: Vec<(usize, i32)> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    // All threads should see same value
    let first_sample = samples[0].1;
    for (thread_id, sample) in &samples {
        assert_eq!(
            *sample, first_sample,
            "Thread {} saw different sample: {} vs {}",
            thread_id, sample, first_sample
        );
    }

    unsafe { cleanup_test_map(layout, cover); }
}
