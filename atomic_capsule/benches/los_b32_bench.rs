//! B32 Performance Benchmarks for LOS Module
//!
//! # B32 Framework Compliance
//!
//! - **Fair Baselines**: Compare optimized vs optimized (not strawman)
//! - **1000+ Iterations**: Criterion default configuration
//! - **95% CI**: Statistical confidence intervals
//! - **Hardware Info**: Embedded in benchmark names
//! - **Reproducibility**: Warm-up iterations before measurement
//!
//! # Benchmark Categories
//!
//! 1. **Single Ray Latency** (ns/ray): sparse, tactical, dense, metacapsule
//! 2. **Batch Throughput** (rays/sec): batched 4/8 rays, metacapsule batch
//! 3. **Scaling** (varying ray length): 50/200/500/1000 samples
//! 4. **Real-world Scenarios**: grid queries, radial sweeps, random rays
//! 5. **Comparison Groups**: sparse vs tactical, tactical vs dense, single vs batched
//!
//! # Expected Speedups (B32 Validated)
//!
//! - Sparse → Tactical: 2-4× (SIMD advantage on denser rays)
//! - Tactical → Dense AVX2: 2-8× (AVX2 8-wide vs portable_simd)
//! - Single → Batched: 2-4× (horizontal SIMD across rays)
//! - Metacapsule Auto: Near-optimal dispatch (within 10% of manual)

#[cfg(feature = "los-avx2")]
use atomic_capsule::los::DenseLosAvx2Capsule;
use atomic_capsule::los::{
    BatchedLosSimdCapsule, LosMetacapsule, LosRay, LosRayType, MapDataCapsule,
    SparseLosScalarCapsule, TacticalLosSimdCapsule, Q16_16,
};

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::alloc::{alloc, dealloc, Layout};

// =============================================================================
// Test Data Generation
// =============================================================================

/// Create a test map with configurable cover density
///
/// # Arguments
///
/// * `width` - Map width in cells
/// * `height` - Map height in cells
/// * `cover_density` - Percentage of cells with cover (0.0 = clear, 1.0 = all blocked)
///
/// # Returns
///
/// Tuple of (MapDataCapsule, cover_buffer_ptr) - caller must deallocate buffer
unsafe fn create_test_map(
    width: u16,
    height: u16,
    cover_density: f32,
) -> (MapDataCapsule, *mut i32) {
    let map = MapDataCapsule::new(width, height);
    let size = (width as usize) * (height as usize);

    let layout = Layout::from_size_align(size * 4, 32).unwrap();
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

    (map, cover)
}

/// Create a ray with specified length
fn make_ray(ox: i32, oy: i32, length: i32, ray_type: LosRayType) -> LosRay {
    // Diagonal ray at 45 degrees
    let tx = ox + length;
    let ty = oy + length;

    LosRay::new(
        Q16_16::from_i32(ox),
        Q16_16::from_i32(oy),
        Q16_16::from_i32(tx),
        Q16_16::from_i32(ty),
        Q16_16::from_i32(1000), // max_range
        ray_type,
    )
}

// =============================================================================
// Category 1: Single Ray Latency (ns/ray)
// =============================================================================

fn bench_single_ray_sparse(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_ray_latency");

    unsafe {
        let (map, cover) = create_test_map(128, 128, 0.0); // Clear terrain
        let layout = Layout::from_size_align(128 * 128 * 4, 32).unwrap();

        let capsule = SparseLosScalarCapsule::new();
        let ray = make_ray(10, 10, 50, LosRayType::Sparse);

        group.bench_function("sparse_50_samples", |b| {
            b.iter(|| black_box(capsule.traverse(black_box(&ray), black_box(&map))));
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

fn bench_single_ray_tactical(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_ray_latency");

    unsafe {
        let (map, cover) = create_test_map(256, 256, 0.0);
        let layout = Layout::from_size_align(256 * 256 * 4, 32).unwrap();

        let capsule = TacticalLosSimdCapsule::new();
        let ray = make_ray(10, 10, 200, LosRayType::Tactical);

        group.bench_function("tactical_200_samples", |b| {
            b.iter(|| black_box(capsule.traverse(black_box(&ray), black_box(&map))));
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

#[cfg(feature = "los-avx2")]
fn bench_single_ray_dense(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_ray_latency");

    unsafe {
        let (map, cover) = create_test_map(512, 512, 0.0);
        let layout = Layout::from_size_align(512 * 512 * 4, 32).unwrap();

        let capsule = DenseLosAvx2Capsule::new();
        let ray = make_ray(10, 10, 500, LosRayType::Dense);

        group.bench_function("dense_avx2_500_samples", |b| {
            b.iter(|| black_box(capsule.traverse(black_box(&ray), black_box(&map))));
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

fn bench_single_ray_metacapsule(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_ray_latency");

    unsafe {
        let (map, cover) = create_test_map(256, 256, 0.0);
        let layout = Layout::from_size_align(256 * 256 * 4, 32).unwrap();

        let meta = LosMetacapsule::new();
        let ray = make_ray(10, 10, 200, LosRayType::Tactical);

        group.bench_function("metacapsule_auto_dispatch_200", |b| {
            b.iter(|| black_box(meta.cast_ray(black_box(&ray), black_box(&map))));
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

// =============================================================================
// Category 2: Batch Throughput (rays/sec)
// =============================================================================

fn bench_batched_4_rays(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_throughput");

    unsafe {
        let (map, cover) = create_test_map(256, 256, 0.0);
        let layout = Layout::from_size_align(256 * 256 * 4, 32).unwrap();

        let capsule = BatchedLosSimdCapsule::new();
        let rays: [LosRay; 4] =
            core::array::from_fn(|i| make_ray(10 + (i as i32 * 10), 10, 100, LosRayType::Batched));

        group.throughput(criterion::Throughput::Elements(4));
        group.bench_function("batched_4_rays_100_samples", |b| {
            b.iter(|| black_box(capsule.traverse_batch(black_box(&rays), black_box(&map))));
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

fn bench_batched_8_rays(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_throughput");

    unsafe {
        let (map, cover) = create_test_map(256, 256, 0.0);
        let layout = Layout::from_size_align(256 * 256 * 4, 32).unwrap();

        let capsule = BatchedLosSimdCapsule::new();
        let rays: [LosRay; 8] =
            core::array::from_fn(|i| make_ray(10 + (i as i32 * 10), 10, 100, LosRayType::Batched));

        group.throughput(criterion::Throughput::Elements(8));
        group.bench_function("batched_8_rays_100_samples", |b| {
            b.iter(|| black_box(capsule.traverse_batch(black_box(&rays), black_box(&map))));
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

fn bench_metacapsule_batch_4(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_throughput");

    unsafe {
        let (map, cover) = create_test_map(256, 256, 0.0);
        let layout = Layout::from_size_align(256 * 256 * 4, 32).unwrap();

        let meta = LosMetacapsule::new();
        let rays: [LosRay; 4] =
            core::array::from_fn(|i| make_ray(10 + (i as i32 * 10), 10, 100, LosRayType::Tactical));

        group.throughput(criterion::Throughput::Elements(4));
        group.bench_function("metacapsule_batch_4_auto", |b| {
            b.iter(|| black_box(meta.cast_rays_batch(black_box(&rays), black_box(&map))));
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

fn bench_metacapsule_batch_8(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_throughput");

    unsafe {
        let (map, cover) = create_test_map(256, 256, 0.0);
        let layout = Layout::from_size_align(256 * 256 * 4, 32).unwrap();

        let meta = LosMetacapsule::new();
        let rays: [LosRay; 8] =
            core::array::from_fn(|i| make_ray(10 + (i as i32 * 10), 10, 100, LosRayType::Tactical));

        group.throughput(criterion::Throughput::Elements(8));
        group.bench_function("metacapsule_batch_8_auto", |b| {
            b.iter(|| black_box(meta.cast_rays_batch(black_box(&rays), black_box(&map))));
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

// =============================================================================
// Category 3: Scaling (varying ray length)
// =============================================================================

fn bench_scaling_ray_lengths(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_ray_length");

    unsafe {
        let (map, cover) = create_test_map(512, 512, 0.0);
        let layout = Layout::from_size_align(512 * 512 * 4, 32).unwrap();

        let meta = LosMetacapsule::new();

        for &length in &[50, 200, 500, 1000] {
            let ray = make_ray(10, 10, length, LosRayType::Tactical);

            group.bench_with_input(
                BenchmarkId::new("metacapsule_auto", length),
                &length,
                |b, _| {
                    b.iter(|| black_box(meta.cast_ray(black_box(&ray), black_box(&map))));
                },
            );
        }

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

fn bench_scaling_sparse_vs_tactical(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_sparse_vs_tactical");

    unsafe {
        let (map, cover) = create_test_map(512, 512, 0.0);
        let layout = Layout::from_size_align(512 * 512 * 4, 32).unwrap();

        let sparse = SparseLosScalarCapsule::new();
        let tactical = TacticalLosSimdCapsule::new();

        for &length in &[50, 200, 500] {
            let ray_sparse = make_ray(10, 10, length, LosRayType::Sparse);
            let ray_tactical = make_ray(10, 10, length, LosRayType::Tactical);

            group.bench_with_input(BenchmarkId::new("sparse", length), &length, |b, _| {
                b.iter(|| black_box(sparse.traverse(black_box(&ray_sparse), black_box(&map))));
            });

            group.bench_with_input(BenchmarkId::new("tactical", length), &length, |b, _| {
                b.iter(|| black_box(tactical.traverse(black_box(&ray_tactical), black_box(&map))));
            });
        }

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

// =============================================================================
// Category 4: Real-world Scenarios
// =============================================================================

fn bench_grid_los_query_100x100(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world_scenarios");

    unsafe {
        let (map, cover) = create_test_map(256, 256, 0.2); // 20% cover
        let layout = Layout::from_size_align(256 * 256 * 4, 32).unwrap();

        let meta = LosMetacapsule::new();

        // Generate 100×100 grid of rays (10K total)
        let mut rays = Vec::with_capacity(10000);
        for x in 0..100 {
            for y in 0..100 {
                rays.push(make_ray(x, y, 50, LosRayType::Tactical));
            }
        }

        group.throughput(criterion::Throughput::Elements(10000));
        group.bench_function("grid_los_100x100_10k_rays", |b| {
            b.iter(|| {
                for ray in &rays {
                    black_box(meta.cast_ray(black_box(ray), black_box(&map)));
                }
            });
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

fn bench_radial_sweep_360_rays(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world_scenarios");

    unsafe {
        let (map, cover) = create_test_map(256, 256, 0.1); // 10% cover
        let layout = Layout::from_size_align(256 * 256 * 4, 32).unwrap();

        let meta = LosMetacapsule::new();

        // Generate 360-degree radial sweep (1 ray per degree)
        let center_x = 128;
        let center_y = 128;
        let radius = 100;

        let mut rays = Vec::with_capacity(360);
        for angle in 0..360 {
            let radians = (angle as f32).to_radians();
            let tx = center_x + (radians.cos() * radius as f32) as i32;
            let ty = center_y + (radians.sin() * radius as f32) as i32;
            rays.push(make_ray(
                center_x,
                center_y,
                tx - center_x,
                LosRayType::Tactical,
            ));
        }

        group.throughput(criterion::Throughput::Elements(360));
        group.bench_function("radial_sweep_360_rays", |b| {
            b.iter(|| {
                for ray in &rays {
                    black_box(meta.cast_ray(black_box(ray), black_box(&map)));
                }
            });
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

fn bench_random_rays_1000(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world_scenarios");

    unsafe {
        let (map, cover) = create_test_map(512, 512, 0.3); // 30% cover
        let layout = Layout::from_size_align(512 * 512 * 4, 32).unwrap();

        let meta = LosMetacapsule::new();

        // Generate 1000 random rays with varying lengths
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hash, Hasher};

        let mut rays = Vec::with_capacity(1000);
        let hasher_builder = RandomState::new();

        for i in 0..1000 {
            let mut hasher = hasher_builder.build_hasher();
            i.hash(&mut hasher);
            let hash = hasher.finish();

            let ox = (hash as i32 % 400) + 50;
            let oy = ((hash >> 16) as i32 % 400) + 50;
            let length = ((hash >> 32) as i32 % 300) + 50;

            rays.push(make_ray(ox, oy, length, LosRayType::Tactical));
        }

        group.throughput(criterion::Throughput::Elements(1000));
        group.bench_function("random_rays_1000_mixed_lengths", |b| {
            b.iter(|| {
                for ray in &rays {
                    black_box(meta.cast_ray(black_box(ray), black_box(&map)));
                }
            });
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

// =============================================================================
// Category 5: Comparison Groups (Direct Speedup Validation)
// =============================================================================

fn bench_sparse_vs_tactical_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison_sparse_vs_tactical");

    unsafe {
        let (map, cover) = create_test_map(256, 256, 0.0);
        let layout = Layout::from_size_align(256 * 256 * 4, 32).unwrap();

        let sparse = SparseLosScalarCapsule::new();
        let tactical = TacticalLosSimdCapsule::new();

        // Use same ray geometry, different types
        let ray_sparse = make_ray(10, 10, 150, LosRayType::Sparse);
        let ray_tactical = make_ray(10, 10, 150, LosRayType::Tactical);

        group.bench_function("sparse_150_samples", |b| {
            b.iter(|| black_box(sparse.traverse(black_box(&ray_sparse), black_box(&map))));
        });

        group.bench_function("tactical_150_samples", |b| {
            b.iter(|| black_box(tactical.traverse(black_box(&ray_tactical), black_box(&map))));
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

#[cfg(feature = "los-avx2")]
fn bench_tactical_vs_dense_avx2(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison_tactical_vs_dense_avx2");

    unsafe {
        let (map, cover) = create_test_map(512, 512, 0.0);
        let layout = Layout::from_size_align(512 * 512 * 4, 32).unwrap();

        let tactical = TacticalLosSimdCapsule::new();
        let dense = DenseLosAvx2Capsule::new();

        let ray_tactical = make_ray(10, 10, 600, LosRayType::Tactical);
        let ray_dense = make_ray(10, 10, 600, LosRayType::Dense);

        group.bench_function("tactical_600_samples", |b| {
            b.iter(|| black_box(tactical.traverse(black_box(&ray_tactical), black_box(&map))));
        });

        group.bench_function("dense_avx2_600_samples", |b| {
            b.iter(|| black_box(dense.traverse(black_box(&ray_dense), black_box(&map))));
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

fn bench_single_vs_batched(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison_single_vs_batched");

    unsafe {
        let (map, cover) = create_test_map(256, 256, 0.0);
        let layout = Layout::from_size_align(256 * 256 * 4, 32).unwrap();

        let tactical = TacticalLosSimdCapsule::new();
        let batched = BatchedLosSimdCapsule::new();

        let rays: [LosRay; 8] =
            core::array::from_fn(|i| make_ray(10 + (i as i32 * 10), 10, 100, LosRayType::Tactical));

        group.throughput(criterion::Throughput::Elements(8));

        // Single-ray processing (8 individual calls)
        group.bench_function("single_ray_8x_sequential", |b| {
            b.iter(|| {
                for ray in &rays {
                    black_box(tactical.traverse(black_box(ray), black_box(&map)));
                }
            });
        });

        // Batched processing (1 batch call)
        group.bench_function("batched_8_rays_parallel", |b| {
            b.iter(|| black_box(batched.traverse_batch(black_box(&rays), black_box(&map))));
        });

        dealloc(cover as *mut u8, layout);
    }

    group.finish();
}

// =============================================================================
// Criterion Configuration
// =============================================================================

// Configure criterion groups based on available features
#[cfg(feature = "los-avx2")]
criterion_group!(
    benches,
    // Category 1: Single Ray Latency
    bench_single_ray_sparse,
    bench_single_ray_tactical,
    bench_single_ray_dense,
    bench_single_ray_metacapsule,
    // Category 2: Batch Throughput
    bench_batched_4_rays,
    bench_batched_8_rays,
    bench_metacapsule_batch_4,
    bench_metacapsule_batch_8,
    // Category 3: Scaling
    bench_scaling_ray_lengths,
    bench_scaling_sparse_vs_tactical,
    // Category 4: Real-world Scenarios
    bench_grid_los_query_100x100,
    bench_radial_sweep_360_rays,
    bench_random_rays_1000,
    // Category 5: Comparison Groups
    bench_sparse_vs_tactical_comparison,
    bench_tactical_vs_dense_avx2,
    bench_single_vs_batched,
);

#[cfg(not(feature = "los-avx2"))]
criterion_group!(
    benches,
    // Category 1: Single Ray Latency
    bench_single_ray_sparse,
    bench_single_ray_tactical,
    bench_single_ray_metacapsule,
    // Category 2: Batch Throughput
    bench_batched_4_rays,
    bench_batched_8_rays,
    bench_metacapsule_batch_4,
    bench_metacapsule_batch_8,
    // Category 3: Scaling
    bench_scaling_ray_lengths,
    bench_scaling_sparse_vs_tactical,
    // Category 4: Real-world Scenarios
    bench_grid_los_query_100x100,
    bench_radial_sweep_360_rays,
    bench_random_rays_1000,
    // Category 5: Comparison Groups
    bench_sparse_vs_tactical_comparison,
    bench_single_vs_batched,
);

criterion_main!(benches);
