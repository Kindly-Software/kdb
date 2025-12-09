//! Line-of-Sight (LOS) Demo
//!
//! Demonstrates the T6 Mixed Tier LOS module with automatic tier dispatch.
//!
//! # Features Showcased
//!
//! 1. Basic single-ray LOS checks (Tactical, Sparse, Dense)
//! 2. Auto-inference with LosMetacapsule
//! 3. MapDataCapsule creation and buffer management
//! 4. Batch processing with multiple rays
//! 5. Metrics and debugging output
//! 6. CPU capability detection and tier selection
//!
//! # Performance Tiers
//!
//! - **Sparse**: Scalar fallback, stride≥4, <10ns overhead
//! - **Tactical**: portable_simd, 80-400 samples, early-exit optimization
//! - **Dense**: AVX2 8× unroll, 500-2K samples (if `los-avx2` feature enabled)
//! - **Batched**: SoA multi-ray, 4-8 rays parallel (horizontal SIMD)
//!
//! # Compilation
//!
//! ```bash
//! # Portable SIMD only (Tactical/Sparse)
//! cargo run --example los_demo --features "los"
//!
//! # With AVX2 optimization (Dense/Batched fast path)
//! RUSTFLAGS="-C target-cpu=native" cargo run --example los_demo --features "los-avx2"
//!
//! # All LOS features
//! RUSTFLAGS="-C target-cpu=native" cargo run --example los_demo --features "los-full"
//! ```

use atomic_capsule::los::{
    cpu_capabilities, traverse_ray_auto, traverse_rays_batch, LosMetacapsule, LosMetrics, LosRay,
    LosRayType, MapDataCapsule, Q16_16, SparseLosScalarCapsule, TacticalLosSimdCapsule,
};

use std::alloc::{alloc, dealloc, Layout};

/// Create a test map with specified dimensions and cover density
///
/// # Arguments
///
/// * `width` - Map width in cells
/// * `height` - Map height in cells
/// * `cover_density` - Percentage of cells with cover (0.0 = clear, 1.0 = all blocked)
///
/// # Returns
///
/// Tuple of (MapDataCapsule, cover_buffer_ptr, layout) - caller must deallocate
unsafe fn create_test_map(
    width: u16,
    height: u16,
    cover_density: f32,
) -> (MapDataCapsule, *mut i32, Layout) {
    let map = MapDataCapsule::new(width, height);
    let size = (width as usize) * (height as usize);

    // Allocate aligned buffers (32-byte for AVX2)
    let layout = Layout::from_size_align(size * 4, 32).expect("Failed to create layout");
    let cover = alloc(layout) as *mut i32;
    let mud = alloc(layout) as *mut i32;
    let cost = alloc(layout) as *mut i32;

    // Initialize with specified cover density
    let cover_value = if cover_density > 0.99 {
        0x0001_0000 // Full cover (Q16.16 1.0)
    } else if cover_density > 0.01 {
        (cover_density * 65536.0) as i32 // Partial cover
    } else {
        0 // Clear terrain
    };

    for i in 0..size {
        *cover.add(i) = cover_value;
        *mud.add(i) = 0;
        *cost.add(i) = 0;
    }

    map.attach_buffers(cover, mud, cost);

    (map, cover, layout)
}

fn main() {
    println!("═══════════════════════════════════════════════════════");
    println!("  Line-of-Sight (LOS) Demo - T6 Mixed Tier");
    println!("═══════════════════════════════════════════════════════\n");

    // ========================================================================
    // Section 1: CPU Capability Detection
    // ========================================================================
    println!("1️⃣  CPU Capability Detection\n");
    println!("───────────────────────────────────────────────────────");

    let caps = cpu_capabilities();
    println!("✓ AVX2 Available:     {}", caps.has_avx2());
    println!("✓ AVX-512F Available: {}", caps.has_avx512f());
    println!("✓ Cache Line Size:    {} bytes", caps.cache_line_size);

    #[cfg(feature = "los-avx2")]
    println!("✓ AVX2 Feature:       ENABLED (Dense/Batched fast path)");
    #[cfg(not(feature = "los-avx2"))]
    println!("⚠ AVX2 Feature:       DISABLED (portable_simd fallback)");

    println!();

    // ========================================================================
    // Section 2: MapDataCapsule Creation
    // ========================================================================
    println!("2️⃣  MapDataCapsule Creation\n");
    println!("───────────────────────────────────────────────────────");

    unsafe {
        let (map, cover_ptr, layout) = create_test_map(256, 256, 0.0);

        println!("✓ Map Dimensions:     256×256 (65,536 cells)");
        println!("✓ Buffer Alignment:   32 bytes (AVX2-ready)");
        println!("✓ Cover Density:      0% (clear terrain)");
        println!("✓ Total Memory:       768 KB (3 buffers × 256 KB)");
        println!();

        // ====================================================================
        // Section 3: Basic LOS Check (Tactical)
        // ====================================================================
        println!("3️⃣  Basic LOS Check - Tactical Ray\n");
        println!("───────────────────────────────────────────────────────");

        let ray_tactical = LosRay::new(
            Q16_16::from_i32(10),  // origin_x
            Q16_16::from_i32(10),  // origin_y
            Q16_16::from_i32(100), // target_x
            Q16_16::from_i32(100), // target_y
            Q16_16::from_i32(500), // max_range
            LosRayType::Tactical,
        );

        println!("Ray Configuration:");
        println!("  Origin:      (10, 10)");
        println!("  Target:      (100, 100)");
        println!("  Max Range:   500");
        println!("  Type:        Tactical (portable_simd, early-exit)");
        println!();

        let result = traverse_ray_auto(&ray_tactical, &map);

        println!("Result:");
        println!("  Status:      {:?}", result.status);
        println!("  Samples:     {}", result.samples_checked);
        println!("  Visibility:  {} (Q16.16 = {:.3})", result.visibility.raw(), result.visibility.to_f32());
        println!("  Cost Sum:    {} (Q16.16 = {:.3})", result.cost_accumulated.raw(), result.cost_accumulated.to_f32());
        println!();

        // ====================================================================
        // Section 4: Auto-Inference with LosMetacapsule
        // ====================================================================
        println!("4️⃣  Auto-Inference with LosMetacapsule\n");
        println!("───────────────────────────────────────────────────────");

        let metacapsule = LosMetacapsule::new();

        // Test different ray types
        let rays = vec![
            (
                "Sparse",
                LosRay::new(
                    Q16_16::from_i32(5),
                    Q16_16::from_i32(5),
                    Q16_16::from_i32(25),
                    Q16_16::from_i32(25),
                    Q16_16::from_i32(200),
                    LosRayType::Sparse,
                ),
            ),
            (
                "Tactical",
                LosRay::new(
                    Q16_16::from_i32(10),
                    Q16_16::from_i32(10),
                    Q16_16::from_i32(150),
                    Q16_16::from_i32(150),
                    Q16_16::from_i32(500),
                    LosRayType::Tactical,
                ),
            ),
            #[cfg(feature = "los-avx2")]
            (
                "Dense",
                LosRay::new(
                    Q16_16::from_i32(20),
                    Q16_16::from_i32(20),
                    Q16_16::from_i32(220),
                    Q16_16::from_i32(220),
                    Q16_16::from_i32(1000),
                    LosRayType::Dense,
                ),
            ),
        ];

        for (name, ray) in &rays {
            let result = metacapsule.cast_ray(ray, &map);
            println!(
                "✓ {} Ray: {:?} ({} samples)",
                name, result.status, result.samples_checked
            );
        }

        println!();

        // ====================================================================
        // Section 5: Batch Processing
        // ====================================================================
        println!("5️⃣  Batch Processing\n");
        println!("───────────────────────────────────────────────────────");

        let batch_rays = vec![
            LosRay::new(
                Q16_16::from_i32(0),
                Q16_16::from_i32(0),
                Q16_16::from_i32(50),
                Q16_16::from_i32(50),
                Q16_16::from_i32(200),
                LosRayType::Tactical,
            ),
            LosRay::new(
                Q16_16::from_i32(10),
                Q16_16::from_i32(10),
                Q16_16::from_i32(60),
                Q16_16::from_i32(60),
                Q16_16::from_i32(200),
                LosRayType::Tactical,
            ),
            LosRay::new(
                Q16_16::from_i32(20),
                Q16_16::from_i32(20),
                Q16_16::from_i32(70),
                Q16_16::from_i32(70),
                Q16_16::from_i32(200),
                LosRayType::Tactical,
            ),
            LosRay::new(
                Q16_16::from_i32(30),
                Q16_16::from_i32(30),
                Q16_16::from_i32(80),
                Q16_16::from_i32(80),
                Q16_16::from_i32(200),
                LosRayType::Tactical,
            ),
        ];

        println!("Processing {} rays in batch...", batch_rays.len());
        println!();

        let batch_results = traverse_rays_batch(&batch_rays, &map);

        for (i, result) in batch_results.iter().enumerate() {
            println!(
                "  Ray {}: {:?} ({} samples)",
                i + 1,
                result.status,
                result.samples_checked
            );
        }

        println!();

        // ====================================================================
        // Section 6: Metrics and Debugging
        // ====================================================================
        println!("6️⃣  Metrics and Debugging\n");
        println!("───────────────────────────────────────────────────────");

        let metrics = LosMetrics::new();

        // Record some test operations
        metrics.record_tactical_ray();
        metrics.record_tactical_ray();
        metrics.record_sparse_ray();
        metrics.record_samples(250);
        metrics.record_early_exit();

        #[cfg(target_arch = "x86_64")]
        if caps.has_avx2() {
            metrics.record_avx2_dispatch();
        } else {
            metrics.record_portable_dispatch();
        }

        println!("LOS Processing Statistics:");
        println!("  Tactical Rays:      {}", metrics.tactical_rays_processed());
        println!("  Sparse Rays:        {}", metrics.sparse_rays_processed());
        println!("  Dense Rays:         {}", metrics.dense_rays_processed());
        println!("  Batched Rays:       {}", metrics.batched_rays_processed());
        println!("  Total Samples:      {}", metrics.total_samples_evaluated());
        println!("  Early Exits:        {}", metrics.early_exits());
        println!("  AVX2 Dispatches:    {}", metrics.avx2_dispatches());
        println!("  Portable Dispatches: {}", metrics.portable_dispatches());

        println!();

        // ====================================================================
        // Section 7: Direct Capsule Usage
        // ====================================================================
        println!("7️⃣  Direct Capsule Usage\n");
        println!("───────────────────────────────────────────────────────");

        // Using TacticalLosSimdCapsule directly
        let tactical_capsule = TacticalLosSimdCapsule::new();
        let tactical_ray = LosRay::new(
            Q16_16::from_i32(15),
            Q16_16::from_i32(15),
            Q16_16::from_i32(115),
            Q16_16::from_i32(115),
            Q16_16::from_i32(500),
            LosRayType::Tactical,
        );

        let tactical_result = tactical_capsule.traverse(&tactical_ray, &map);
        println!("TacticalLosSimdCapsule (direct):");
        println!("  Status:      {:?}", tactical_result.status);
        println!("  Samples:     {}", tactical_result.samples_checked);
        println!();

        // Using SparseLosScalarCapsule directly
        let sparse_capsule = SparseLosScalarCapsule::new();
        let sparse_ray = LosRay::new(
            Q16_16::from_i32(5),
            Q16_16::from_i32(5),
            Q16_16::from_i32(30),
            Q16_16::from_i32(30),
            Q16_16::from_i32(200),
            LosRayType::Sparse,
        );

        let sparse_result = sparse_capsule.traverse(&sparse_ray, &map);
        println!("SparseLosScalarCapsule (direct):");
        println!("  Status:      {:?}", sparse_result.status);
        println!("  Samples:     {}", sparse_result.samples_checked);
        println!();

        // ====================================================================
        // Section 8: Advanced - Cover Density Testing
        // ====================================================================
        println!("8️⃣  Advanced - Cover Density Testing\n");
        println!("───────────────────────────────────────────────────────");

        // Create maps with varying cover densities
        let densities = vec![0.0, 0.25, 0.5, 0.75, 1.0];

        println!("Testing visibility through different cover densities:\n");

        for &density in &densities {
            let (dense_map, dense_cover_ptr, dense_layout) = create_test_map(128, 128, density);

            let test_ray = LosRay::new(
                Q16_16::from_i32(10),
                Q16_16::from_i32(10),
                Q16_16::from_i32(100),
                Q16_16::from_i32(100),
                Q16_16::from_i32(500),
                LosRayType::Tactical,
            );

            let result = traverse_ray_auto(&test_ray, &dense_map);

            println!(
                "  Density {:.0}%: {:?} (visibility: {:.3}, cost: {:.3})",
                density * 100.0,
                result.status,
                result.visibility.to_f32(),
                result.cost_accumulated.to_f32()
            );

            // Cleanup
            dealloc(dense_cover_ptr as *mut u8, dense_layout);
        }

        println!();

        // Cleanup main map
        dealloc(cover_ptr as *mut u8, layout);
    }

    // ========================================================================
    // Summary
    // ========================================================================
    println!("═══════════════════════════════════════════════════════");
    println!("  Demo Complete!");
    println!("═══════════════════════════════════════════════════════\n");

    println!("Key Takeaways:");
    println!("  • T6 Mixed Tier combines T1+T2+T3+T4 innovations");
    println!("  • Auto-dispatch selects optimal implementation per ray");
    println!("  • Q16.16 fixed-point ensures deterministic results");
    println!("  • Lockfree MapDataCapsule enables concurrent access");
    println!("  • Metrics provide visibility into tier selection");
    println!();

    #[cfg(feature = "los-avx2")]
    println!("  ✓ AVX2 optimization enabled (Dense/Batched fast path)");
    #[cfg(not(feature = "los-avx2"))]
    println!("  ⚠ Recompile with 'los-avx2' for 2-8× speedup on dense rays");

    println!();
}
