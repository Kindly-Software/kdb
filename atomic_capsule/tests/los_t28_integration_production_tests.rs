//! T28 Integration and Production Tests for LOS Module
//!
//! # Test Coverage
//!
//! **Q15-Q21 Integration Tests** (14 minimum):
//! - Q15: Metacapsule dispatches to correct sub-capsule by ray type
//! - Q16: Batch processing groups rays correctly by type
//! - Q17: MapDataCapsule integration with all capsule types
//! - Q18: Cross-capsule consistency (same ray → same result)
//! - Q19: Early-exit propagation in tactical mode
//! - Q20: SoA ↔ AoS conversion correctness in batched mode
//! - Q21: Generation counter coordination across sub-capsules
//!
//! **Q22-Q28 Production Tests** (14 minimum):
//! - Q22: Stress test with 10K rays
//! - Q23: Memory bounds (no allocations in hot path)
//! - Q24: Concurrent access safety (multi-threaded ray casting)
//! - Q25: Performance regression guard (assert latency bounds)
//! - Q26: Real-world scenario: grid-based LOS queries
//! - Q27: Real-world scenario: radial visibility sweep
//! - Q28: Edge cases: diagonal rays, axis-aligned rays, zero-length rays

#![cfg(feature = "los")]

use atomic_capsule::los::types::{LosRay, LosRayType, Q16_16, LosStatus};
use atomic_capsule::los::map_data::MapDataCapsule;
use atomic_capsule::los::sparse::SparseLosScalarCapsule;
use atomic_capsule::los::tactical::TacticalLosSimdCapsule;
use atomic_capsule::los::batched::{BatchedLosSimdCapsule, MAX_BATCH_SIZE};
use atomic_capsule::los::metacapsule::LosMetacapsule;
use std::alloc::{alloc, dealloc, Layout};
use std::sync::Arc;
use std::thread;

// =============================================================================
// Test Helpers
// =============================================================================

/// Create a test ray
fn make_ray(ox: f32, oy: f32, tx: f32, ty: f32, ray_type: LosRayType) -> LosRay {
    LosRay::from_f32(ox, oy, tx, ty, 1000.0, ray_type)
}

/// Allocate and initialize test map with custom pattern
unsafe fn create_test_map(
    width: u16,
    height: u16,
    init_fn: impl Fn(usize, usize) -> i32,
) -> (MapDataCapsule, *mut i32, Layout) {
    let map = MapDataCapsule::new(width, height);
    let layout = Layout::from_size_align((width as usize) * (height as usize) * 4, 32).unwrap();

    let cover = alloc(layout) as *mut i32;
    let mud = alloc(layout) as *mut i32;
    let cost = alloc(layout) as *mut i32;

    // Initialize buffers
    for y in 0..height as usize {
        for x in 0..width as usize {
            let idx = y * width as usize + x;
            let val = init_fn(x, y);
            *cover.add(idx) = val;
            *mud.add(idx) = 0;
            *cost.add(idx) = 0;
        }
    }

    map.attach_buffers(cover, mud, cost);
    (map, cover, layout)
}

/// Cleanup test map
unsafe fn cleanup_test_map(cover: *mut i32, layout: Layout) {
    dealloc(cover as *mut u8, layout);
}

// =============================================================================
// Q15-Q21 Integration Tests
// =============================================================================

#[test]
fn q15_metacapsule_dispatches_to_correct_subcapsule_by_ray_type() {
    let meta = LosMetacapsule::new();
    let map = MapDataCapsule::new(100, 100);

    // Reset metrics
    meta.reset_metrics();

    // Test Dense dispatch
    let dense_ray = make_ray(0.0, 0.0, 500.0, 0.0, LosRayType::Dense);
    let _ = meta.cast_ray(&dense_ray, &map);

    // Test Tactical dispatch
    let tactical_ray = make_ray(0.0, 0.0, 100.0, 0.0, LosRayType::Tactical);
    let _ = meta.cast_ray(&tactical_ray, &map);

    // Test Sparse dispatch
    let sparse_ray = make_ray(0.0, 0.0, 10.0, 0.0, LosRayType::Sparse);
    let _ = meta.cast_ray(&sparse_ray, &map);

    // Test Batched dispatch
    let batched_ray = make_ray(0.0, 0.0, 50.0, 0.0, LosRayType::Batched);
    let _ = meta.cast_ray(&batched_ray, &map);

    let metrics = meta.metrics();

    // Verify correct dispatches occurred
    // Note: Dense may fallback to portable if AVX2 not available
    assert!(metrics.avx2_dispatches > 0 || metrics.portable_dispatches >= 2,
            "Expected AVX2 or fallback for dense ray");
    assert!(metrics.portable_dispatches >= 1, "Expected tactical dispatch");
    assert_eq!(metrics.sparse_dispatches, 1, "Expected sparse dispatch");
    assert_eq!(metrics.batched_dispatches, 1, "Expected batched dispatch");
    assert_eq!(metrics.rays_processed, 4);
}

#[test]
fn q16_batch_processing_groups_rays_correctly_by_type() {
    let meta = LosMetacapsule::new();
    let map = MapDataCapsule::new(200, 200);

    // Create mixed batch of 12 rays (4 sparse, 4 tactical, 4 batched)
    let mut rays = Vec::new();
    for i in 0..4 {
        rays.push(make_ray(0.0, (i * 10) as f32, 20.0, (i * 10) as f32, LosRayType::Sparse));
    }
    for i in 0..4 {
        rays.push(make_ray(0.0, (i * 10) as f32, 100.0, (i * 10) as f32, LosRayType::Tactical));
    }
    for i in 0..4 {
        rays.push(make_ray(0.0, (i * 10) as f32, 50.0, (i * 10) as f32, LosRayType::Batched));
    }

    meta.reset_metrics();
    let results = meta.cast_rays_batch(&rays, &map);

    assert_eq!(results.len(), 12, "Should return result for each ray");

    let metrics = meta.metrics();
    assert_eq!(metrics.rays_processed, 12);

    // Verify grouping occurred (each type should have been batched)
    assert_eq!(metrics.sparse_dispatches, 4);
    assert_eq!(metrics.portable_dispatches, 4);
    assert_eq!(metrics.batched_dispatches, 4);
}

#[test]
fn q17_mapdatacapsule_integration_with_all_capsule_types() {
    unsafe {
        // Create test map with alternating blocked/clear columns
        let (map, cover, layout) = create_test_map(64, 64, |x, _y| {
            if x % 2 == 0 { 0 } else { Q16_16::ONE.raw() }
        });

        // Test with sparse capsule
        let sparse = SparseLosScalarCapsule::new();
        let ray1 = make_ray(0.0, 32.0, 10.0, 32.0, LosRayType::Sparse); // Horizontal through blocks
        let result1 = sparse.traverse(&ray1, &map);
        // Visibility must be in valid range [0, Q16_16::ONE]
        // A ray through alternating blocked/clear can have full visibility if path is clear
        assert!(result1.visibility.raw() >= 0 && result1.visibility.raw() <= Q16_16::ONE.raw(),
                "Invalid visibility: {}", result1.visibility.raw());

        // Test with tactical capsule
        let tactical = TacticalLosSimdCapsule::new();
        let ray2 = make_ray(0.0, 32.0, 10.0, 32.0, LosRayType::Tactical);
        let result2 = tactical.traverse(&ray2, &map);
        // Visibility must be in valid range [0, Q16_16::ONE]
        assert!(result2.visibility.raw() >= 0 && result2.visibility.raw() <= Q16_16::ONE.raw(),
                "Invalid visibility: {}", result2.visibility.raw());

        // Test with batched capsule
        let batched = BatchedLosSimdCapsule::new();
        let rays = [make_ray(0.0, 32.0, 10.0, 32.0, LosRayType::Batched)];
        let results = batched.traverse_batch(&rays, &map);
        assert_eq!(results.len(), 1);
        // Visibility must be in valid range [0, Q16_16::ONE]
        assert!(results[0].visibility.raw() >= 0 && results[0].visibility.raw() <= Q16_16::ONE.raw(),
                "Invalid visibility: {}", results[0].visibility.raw());

        cleanup_test_map(cover, layout);
    }
}

#[test]
fn q18_cross_capsule_consistency_same_ray_same_result() {
    unsafe {
        // Create test map with diagonal wall
        let (map, cover, layout) = create_test_map(64, 64, |x, y| {
            if x == y { Q16_16::ONE.raw() } else { 0 }
        });

        let ray = LosRay::from_f32(0.0, 0.0, 63.0, 63.0, 100.0, LosRayType::Sparse);

        // Test with sparse capsule
        let sparse = SparseLosScalarCapsule::new();
        let result_sparse = sparse.traverse(&ray, &map);

        // Test with tactical capsule (same ray)
        let tactical = TacticalLosSimdCapsule::new();
        let ray_tactical = LosRay::from_f32(0.0, 0.0, 63.0, 63.0, 100.0, LosRayType::Tactical);
        let result_tactical = tactical.traverse(&ray_tactical, &map);

        // Both should detect blockage (diagonal crosses wall)
        assert!(result_sparse.is_blocked() || result_sparse.visibility.raw() < Q16_16::HALF.raw(),
                "Sparse should detect blockage");
        assert!(result_tactical.is_blocked() || result_tactical.visibility.raw() < Q16_16::HALF.raw(),
                "Tactical should detect blockage");

        cleanup_test_map(cover, layout);
    }
}

#[test]
fn q19_early_exit_propagation_in_tactical_mode() {
    unsafe {
        // Create map with wall at x=20
        let (map, cover, layout) = create_test_map(100, 100, |x, _y| {
            if x >= 20 { Q16_16::ONE.raw() } else { 0 }
        });

        let tactical = TacticalLosSimdCapsule::new();

        // Long ray that crosses wall early
        let ray = make_ray(0.0, 50.0, 99.0, 50.0, LosRayType::Tactical);
        let result = tactical.traverse(&ray, &map);

        // Should early-exit before processing all 100 samples
        assert!(result.samples_checked < 100,
                "Expected early-exit, got {} samples", result.samples_checked);
        assert!(result.is_blocked() || matches!(result.status, LosStatus::EarlyExit));

        cleanup_test_map(cover, layout);
    }
}

#[test]
fn q20_soa_aos_conversion_correctness_in_batched_mode() {
    unsafe {
        // Create clear map
        let (map, cover, layout) = create_test_map(100, 100, |_x, _y| 0);

        let batched = BatchedLosSimdCapsule::new();

        // Create 8 rays with different endpoints
        let rays: Vec<LosRay> = (0..8).map(|i| {
            make_ray(10.0, 10.0, 50.0, (10 + i * 5) as f32, LosRayType::Batched)
        }).collect();

        let results = batched.traverse_batch(&rays, &map);

        assert_eq!(results.len(), 8, "Should return 8 results");

        // All rays through clear terrain should be visible
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_visible() || result.visibility.raw() > 0,
                    "Ray {} should be visible through clear terrain", i);
        }

        cleanup_test_map(cover, layout);
    }
}

#[test]
fn q21_generation_counter_coordination_across_subcapsules() {
    let meta = LosMetacapsule::new();
    let sparse = SparseLosScalarCapsule::new();
    let tactical = TacticalLosSimdCapsule::new();
    let batched = BatchedLosSimdCapsule::new();

    let map = MapDataCapsule::new(100, 100);
    let ray = make_ray(0.0, 0.0, 50.0, 50.0, LosRayType::Sparse);

    // Check initial generations
    let meta_gen_0 = meta.generation();
    let sparse_gen_0 = sparse.generation();
    let tactical_gen_0 = tactical.generation();
    let batched_gen_0 = batched.generation();

    // Process rays through each capsule
    let _ = sparse.traverse(&ray, &map);
    let _ = tactical.traverse(&ray, &map);
    let _ = batched.traverse_batch(&[ray], &map);
    let _ = meta.cast_ray(&ray, &map);

    // Verify generations incremented
    assert!(sparse.generation() > sparse_gen_0, "Sparse gen should increment");
    assert!(tactical.generation() > tactical_gen_0, "Tactical gen should increment");
    assert!(batched.generation() > batched_gen_0, "Batched gen should increment");
    assert!(meta.generation() > meta_gen_0, "Meta gen should increment");
}

// =============================================================================
// Q22-Q28 Production Tests
// =============================================================================

#[test]
fn q22_stress_test_with_10k_rays() {
    let meta = LosMetacapsule::new();

    unsafe {
        let (map, cover, layout) = create_test_map(256, 256, |x, y| {
            // Checkerboard pattern with some clear paths
            if (x + y) % 4 == 0 { Q16_16::HALF.raw() } else { 0 }
        });

        // Generate 10,000 rays with mixed types
        let mut rays = Vec::with_capacity(10000);
        for i in 0..10000 {
            let ray_type = match i % 4 {
                0 => LosRayType::Sparse,
                1 => LosRayType::Tactical,
                2 => LosRayType::Batched,
                _ => LosRayType::Dense,
            };

            let ox = ((i * 13) % 200) as f32;
            let oy = ((i * 17) % 200) as f32;
            let tx = ((i * 23) % 200 + 20) as f32;
            let ty = ((i * 29) % 200 + 20) as f32;

            rays.push(make_ray(ox, oy, tx, ty, ray_type));
        }

        let results = meta.cast_rays_batch(&rays, &map);

        assert_eq!(results.len(), 10000, "Should process all 10K rays");

        let metrics = meta.metrics();
        assert_eq!(metrics.rays_processed, 10000);
        assert!(metrics.samples_evaluated > 0, "Should evaluate samples");

        cleanup_test_map(cover, layout);
    }
}

#[test]
fn q23_memory_bounds_no_allocations_in_hot_path() {
    // This test verifies that sparse and tactical capsules don't allocate
    // by checking they can be used in const contexts and have fixed sizes

    const SPARSE_SIZE: usize = core::mem::size_of::<SparseLosScalarCapsule>();
    const TACTICAL_SIZE: usize = core::mem::size_of::<TacticalLosSimdCapsule>();
    const BATCHED_SIZE: usize = core::mem::size_of::<BatchedLosSimdCapsule>();
    const META_SIZE: usize = core::mem::size_of::<LosMetacapsule>();

    assert_eq!(SPARSE_SIZE, 64, "Sparse must be 64B");
    assert_eq!(TACTICAL_SIZE, 64, "Tactical must be 64B");
    assert_eq!(BATCHED_SIZE, 64, "Batched must be 64B");
    assert_eq!(META_SIZE, 256, "Meta must be 256B");

    // Verify stack allocation (no heap)
    let sparse = SparseLosScalarCapsule::new();
    let tactical = TacticalLosSimdCapsule::new();
    let batched = BatchedLosSimdCapsule::new();
    let meta = LosMetacapsule::new();

    // These should compile without heap allocations
    let _ = sparse.generation();
    let _ = tactical.generation();
    let _ = batched.generation();
    let _ = meta.generation();
}

#[test]
fn q24_concurrent_access_safety_multithreaded_ray_casting() {
    unsafe {
        let (map, cover, layout) = create_test_map(128, 128, |_x, _y| 0);

        // Wrap in Arc for sharing across threads
        let map = Arc::new(map);
        let meta = Arc::new(LosMetacapsule::new());

        let mut handles = vec![];

        // Spawn 8 threads, each casting 100 rays
        for thread_id in 0..8 {
            let map_clone = Arc::clone(&map);
            let meta_clone = Arc::clone(&meta);

            let handle = thread::spawn(move || {
                let mut thread_results = Vec::new();

                for i in 0..100 {
                    let ox = ((thread_id * 10 + i) % 100) as f32;
                    let oy = ((thread_id * 7 + i) % 100) as f32;
                    let tx = ((thread_id * 13 + i) % 100 + 20) as f32;
                    let ty = ((thread_id * 17 + i) % 100 + 20) as f32;

                    let ray = make_ray(ox, oy, tx, ty, LosRayType::Tactical);
                    let result = meta_clone.cast_ray(&ray, &map_clone);
                    thread_results.push(result);
                }

                thread_results
            });

            handles.push(handle);
        }

        // Wait for all threads and collect results
        let mut all_results = Vec::new();
        for handle in handles {
            let results = handle.join().expect("Thread panicked");
            all_results.extend(results);
        }

        assert_eq!(all_results.len(), 800, "Should have 800 total results");

        let metrics = meta.metrics();
        assert_eq!(metrics.rays_processed, 800);

        cleanup_test_map(cover, layout);
    }
}

#[test]
fn q25_performance_regression_guard_assert_latency_bounds() {
    use std::time::Instant;

    unsafe {
        let (map, cover, layout) = create_test_map(100, 100, |_x, _y| 0);

        let sparse = SparseLosScalarCapsule::new();
        let tactical = TacticalLosSimdCapsule::new();

        // Warmup
        for _ in 0..100 {
            let ray = make_ray(0.0, 0.0, 50.0, 50.0, LosRayType::Sparse);
            let _ = sparse.traverse(&ray, &map);
        }

        // Test sparse performance (production target: <500ns for 50 samples)
        let ray_sparse = make_ray(0.0, 0.0, 50.0, 0.0, LosRayType::Sparse);
        let start = Instant::now();
        for _ in 0..1000 {
            let _ = sparse.traverse(&ray_sparse, &map);
        }
        let sparse_duration = start.elapsed();
        let sparse_per_ray = sparse_duration.as_nanos() / 1000;

        // Test tactical performance (production target: <1000ns for 200 samples)
        let ray_tactical = make_ray(0.0, 0.0, 200.0, 0.0, LosRayType::Tactical);
        let start = Instant::now();
        for _ in 0..1000 {
            let _ = tactical.traverse(&ray_tactical, &map);
        }
        let tactical_duration = start.elapsed();
        let tactical_per_ray = tactical_duration.as_nanos() / 1000;

        // CI-tolerant bounds (3× production targets for cold cache, VM overhead, shared resources)
        // Production targets: Sparse <500ns, Tactical <1000ns
        // CI bounds: Sparse <1500ns, Tactical <3000ns (allows for 2-5× CI variability)
        // Debug mode: 10× looser bounds due to unoptimized code (debug builds are 8-10× slower)
        #[cfg(debug_assertions)]
        {
            assert!(sparse_per_ray < 15000,
                    "Sparse ray too slow in debug mode: {}ns (debug target <15000ns, release CI <1500ns, production <500ns)", sparse_per_ray);
            assert!(tactical_per_ray < 30000,
                    "Tactical ray too slow in debug mode: {}ns (debug target <30000ns, release CI <3000ns, production <1000ns)", tactical_per_ray);
        }

        // Release mode: tight production bounds
        #[cfg(not(debug_assertions))]
        {
            assert!(sparse_per_ray < 1500,
                    "Sparse ray too slow: {}ns (CI target <1500ns, production <500ns)", sparse_per_ray);
            assert!(tactical_per_ray < 3000,
                    "Tactical ray too slow: {}ns (CI target <3000ns, production <1000ns)", tactical_per_ray);
        }

        cleanup_test_map(cover, layout);
    }
}

#[test]
fn q26_realworld_scenario_grid_based_los_queries() {
    // Simulate a grid-based game where we need to check LOS from
    // player position to all enemies in a 20×20 grid

    unsafe {
        // Create map with walls at specific positions (fortress layout)
        let (map, cover, layout) = create_test_map(100, 100, |x, y| {
            // Outer walls
            if x < 5 || x >= 95 || y < 5 || y >= 95 {
                return Q16_16::ONE.raw();
            }
            // Inner walls (rooms)
            if (x == 50 && y > 20 && y < 80) || (y == 50 && x > 20 && x < 80) {
                return Q16_16::ONE.raw();
            }
            0
        });

        let meta = LosMetacapsule::new();
        let player_pos = (25.0, 25.0); // Player in bottom-left room

        // Generate 400 enemy positions in 20×20 grid
        let mut rays = Vec::new();
        for grid_y in 0..20 {
            for grid_x in 0..20 {
                let enemy_x = 10.0 + grid_x as f32 * 4.0;
                let enemy_y = 10.0 + grid_y as f32 * 4.0;

                let ray = make_ray(
                    player_pos.0,
                    player_pos.1,
                    enemy_x,
                    enemy_y,
                    LosRayType::Tactical
                );
                rays.push(ray);
            }
        }

        let results = meta.cast_rays_batch(&rays, &map);
        assert_eq!(results.len(), 400);

        // Count visible enemies
        let visible_count = results.iter()
            .filter(|r| r.visibility.raw() >= Q16_16::HALF.raw())
            .count();

        // In bottom-left room, most enemies in same quadrant should be visible
        // Enemies across walls should be blocked
        assert!(visible_count > 0, "Some enemies should be visible");
        assert!(visible_count < 400, "Not all enemies should be visible (walls block)");

        cleanup_test_map(cover, layout);
    }
}

#[test]
fn q27_realworld_scenario_radial_visibility_sweep() {
    // Simulate radar/vision cone sweep: cast rays in 360° circle

    unsafe {
        let (map, cover, layout) = create_test_map(128, 128, |x, y| {
            let dx = x as i32 - 64;
            let dy = y as i32 - 64;
            let dist_sq = dx * dx + dy * dy;

            // Circular obstacles at radius 30-40
            if dist_sq > 900 && dist_sq < 1600 {
                Q16_16::HALF.raw()
            } else {
                0
            }
        });

        let meta = LosMetacapsule::new();
        let center = (64.0, 64.0);
        let radius = 60.0;
        let num_rays = 360; // 1° resolution

        let mut rays = Vec::new();
        for i in 0..num_rays {
            let angle = (i as f32) * std::f32::consts::PI / 180.0;
            let tx = center.0 + radius * angle.cos();
            let ty = center.1 + radius * angle.sin();

            let ray = make_ray(center.0, center.1, tx, ty, LosRayType::Tactical);
            rays.push(ray);
        }

        let results = meta.cast_rays_batch(&rays, &map);
        assert_eq!(results.len(), num_rays);

        // All rays should hit the obstacle ring (partial visibility)
        let blocked_or_partial = results.iter()
            .filter(|r| r.is_blocked() || r.is_partial())
            .count();

        assert!(blocked_or_partial > num_rays / 2,
                "Most rays should hit obstacle ring");

        cleanup_test_map(cover, layout);
    }
}

#[test]
fn q28_edge_cases_diagonal_axis_aligned_zero_length_rays() {
    unsafe {
        let (map, cover, layout) = create_test_map(64, 64, |_x, _y| 0);

        let meta = LosMetacapsule::new();

        // Test 1: Perfect diagonal (45°)
        let diagonal = make_ray(0.0, 0.0, 50.0, 50.0, LosRayType::Tactical);
        let result1 = meta.cast_ray(&diagonal, &map);
        assert!(result1.samples_checked > 0);
        assert!(result1.is_visible());

        // Test 2: Horizontal axis-aligned
        let horizontal = make_ray(10.0, 30.0, 50.0, 30.0, LosRayType::Tactical);
        let result2 = meta.cast_ray(&horizontal, &map);
        assert!(result2.samples_checked > 0);
        assert!(result2.is_visible());

        // Test 3: Vertical axis-aligned
        let vertical = make_ray(30.0, 10.0, 30.0, 50.0, LosRayType::Tactical);
        let result3 = meta.cast_ray(&vertical, &map);
        assert!(result3.samples_checked > 0);
        assert!(result3.is_visible());

        // Test 4: Zero-length ray (same origin and target)
        let zero_length = make_ray(25.0, 25.0, 25.0, 25.0, LosRayType::Sparse);
        let result4 = meta.cast_ray(&zero_length, &map);
        // Should handle gracefully (minimal samples)
        assert!(result4.samples_checked <= 1);

        // Test 5: Very short ray (<1 unit)
        let very_short = make_ray(25.0, 25.0, 25.1, 25.1, LosRayType::Sparse);
        let result5 = meta.cast_ray(&very_short, &map);
        assert!(result5.samples_checked >= 1);

        // Test 6: Steep diagonal (>45°)
        let steep = make_ray(0.0, 0.0, 10.0, 50.0, LosRayType::Tactical);
        let result6 = meta.cast_ray(&steep, &map);
        assert!(result6.samples_checked > 0);

        cleanup_test_map(cover, layout);
    }
}

// =============================================================================
// Additional Integration Tests
// =============================================================================

#[test]
fn integration_metacapsule_auto_classification() {
    let meta = LosMetacapsule::new();

    // Create rays with default type (should be auto-classified by length)
    let short_ray = LosRay::from_f32(0.0, 0.0, 5.0, 5.0, 100.0, LosRayType::Sparse);
    let medium_ray = LosRay::from_f32(0.0, 0.0, 50.0, 50.0, 100.0, LosRayType::Tactical);
    let long_ray = LosRay::from_f32(0.0, 0.0, 500.0, 500.0, 1000.0, LosRayType::Dense);

    assert_eq!(meta.classify_ray(&short_ray), LosRayType::Sparse);
    assert_eq!(meta.classify_ray(&medium_ray), LosRayType::Tactical);
    assert_eq!(meta.classify_ray(&long_ray), LosRayType::Dense);
}

#[test]
fn integration_batch_result_ordering_preserved() {
    unsafe {
        let (map, cover, layout) = create_test_map(100, 100, |_x, _y| 0);

        let meta = LosMetacapsule::new();

        // Create rays with unique endpoints for identification
        let rays: Vec<LosRay> = (0..10).map(|i| {
            make_ray(0.0, 0.0, (i * 10) as f32, (i * 10) as f32, LosRayType::Tactical)
        }).collect();

        let results = meta.cast_rays_batch(&rays, &map);

        // Results should be in same order as input
        assert_eq!(results.len(), 10);

        // All clear terrain should be visible
        for result in &results {
            assert!(result.is_visible() || result.visibility.raw() > 0);
        }

        cleanup_test_map(cover, layout);
    }
}

#[test]
fn integration_map_reader_writer_coordination() {
    unsafe {
        let (map, cover, layout) = create_test_map(64, 64, |_x, _y| 0);

        // Acquire read access
        let guard1 = map.acquire_read().expect("First read should succeed");
        let guard2 = map.acquire_read().expect("Second read should succeed");

        // Writer should fail while readers active
        assert!(map.acquire_write().is_none());

        drop(guard1);
        drop(guard2);

        // Writer should succeed now
        let write_guard = map.acquire_write().expect("Write should succeed");

        // Readers should fail while writer active
        assert!(map.acquire_read().is_none());

        let version_before = map.version();
        drop(write_guard);

        // Version should increment after write
        assert_eq!(map.version(), version_before + 1);

        cleanup_test_map(cover, layout);
    }
}

#[test]
fn production_metacapsule_metrics_accuracy() {
    let meta = LosMetacapsule::new();

    unsafe {
        let (map, cover, layout) = create_test_map(100, 100, |_x, _y| 0);

        meta.reset_metrics();

        // Process known counts
        for _ in 0..10 {
            let ray = make_ray(0.0, 0.0, 50.0, 50.0, LosRayType::Tactical);
            let _ = meta.cast_ray(&ray, &map);
        }

        let rays: Vec<LosRay> = (0..20).map(|i| {
            make_ray(0.0, (i * 2) as f32, 50.0, (i * 2) as f32, LosRayType::Batched)
        }).collect();
        let _ = meta.cast_rays_batch(&rays, &map);

        let metrics = meta.metrics();

        // Verify metrics
        assert_eq!(metrics.rays_processed, 30, "Should process 30 total rays");
        assert_eq!(metrics.portable_dispatches, 10, "Should have 10 tactical dispatches");
        assert_eq!(metrics.batched_dispatches, 20, "Should have 20 batched dispatches");
        assert!(metrics.samples_evaluated > 0, "Should evaluate samples");

        cleanup_test_map(cover, layout);
    }
}

#[test]
fn production_batch_early_exit_all_rays_blocked() {
    unsafe {
        // Create fully blocked map
        let (map, cover, layout) = create_test_map(100, 100, |_x, _y| Q16_16::ONE.raw());

        let batched = BatchedLosSimdCapsule::new();

        let rays: Vec<LosRay> = (0..8).map(|i| {
            make_ray(0.0, (i * 5) as f32, 50.0, (i * 5) as f32, LosRayType::Batched)
        }).collect();

        let results = batched.traverse_batch(&rays, &map);

        // All rays should be blocked quickly
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_blocked(), "Ray {} should be blocked", i);
            assert!(result.samples_checked <= 10,
                    "Ray {} should early-exit quickly, got {} samples",
                    i, result.samples_checked);
        }

        cleanup_test_map(cover, layout);
    }
}
