//! # B32 Benchmarks for MotionEstimationCapsule
//!
//! **CRITICAL PATH VALIDATION**: Motion estimation accounts for 76% of video encoding time.
//!
//! ## Performance Targets (Conservative)
//!
//! | Operation | Target | Exceptional | Notes |
//! |-----------|--------|-------------|-------|
//! | 16×16 Block | <100μs CPU | <10μs GPU | GPU acceleration critical |
//! | Diamond Search | <80μs CPU | <6μs GPU | Most common algorithm |
//! | Hexagon Search | <90μs CPU | <7μs GPU | Better quality |
//! | Full Search | <2ms CPU | <50μs GPU | Exhaustive (benchmark only) |
//! | Sub-pixel Refinement | <40μs CPU | <3μs GPU | Quarter-pixel |
//!
//! ## Baseline Comparison
//!
//! - **x264 (C, AVX2)**: ~50μs per 16×16 block (optimized reference)
//! - **rav1e (Rust)**: ~80μs per 16×16 block (pure Rust CPU)
//! - **Our CPU Target**: <100μs (conservative, 1.25× rav1e)
//! - **Our GPU Target**: <10μs (exceptional, 5-10× faster than x264)
//!
//! ## Methodology (B32 Framework)
//!
//! - **Fair Baseline**: rav1e CPU implementation (Rust reference)
//! - **Hardware**: K1-K70 (consumer to workstation CPUs), GPU optional
//! - **Iterations**: 1000+ per benchmark
//! - **Confidence**: 95% CI
//! - **Reproducibility**: 3 separate benchmark runs

#![cfg(not(target_env = "msvc"))]

use atomic_capsule::encoder::motion_estimation::{
    BlockSize, MotionEstimationCapsule, SearchAlgorithm, SubPixelMode,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

/// Generate synthetic reference frame (realistic video data)
fn generate_reference_frame(width: usize, height: usize) -> Vec<u8> {
    let mut frame = vec![0u8; width * height];

    // Simulate realistic luma values (natural images have structure)
    for y in 0..height {
        for x in 0..width {
            let val = ((x * 7 + y * 11) % 256) as u8;
            frame[y * width + x] = val;
        }
    }

    frame
}

/// Generate current frame (shifted from reference to create motion)
fn generate_current_frame(reference: &[u8], width: usize, height: usize, shift_x: i32, shift_y: i32) -> Vec<u8> {
    let mut frame = vec![0u8; width * height];

    for y in 0..height {
        for x in 0..width {
            let src_x = (x as i32 + shift_x).max(0).min(width as i32 - 1) as usize;
            let src_y = (y as i32 + shift_y).max(0).min(height as i32 - 1) as usize;
            frame[y * width + x] = reference[src_y * width + src_x];
        }
    }

    frame
}

// ============================================================================
// GROUP 1: CPU FALLBACK SEARCH ALGORITHMS
// ============================================================================

fn bench_cpu_search_algorithms(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_search_algorithms");
    group.throughput(Throughput::Elements(1)); // 1 block per iteration

    let width = 128;
    let height = 128;
    let reference = generate_reference_frame(width, height);
    let current = generate_current_frame(&reference, width, height, 4, 4);

    let algorithms = [
        SearchAlgorithm::Diamond,
        SearchAlgorithm::Hexagonal,
        SearchAlgorithm::FullSearch,
    ];

    for &algo in &algorithms {
        let mut capsule = MotionEstimationCapsule::new();
        capsule.configure(64, SubPixelMode::QuarterPixel, algo);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", algo)),
            &algo,
            |b, _| {
                b.iter(|| {
                    black_box(capsule.estimate_block(
                        black_box(&reference),
                        black_box(&current),
                        black_box(width),
                        black_box(width),
                        black_box(32),
                        black_box(32),
                        black_box(BlockSize::Block16x16),
                    ))
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// GROUP 2: BLOCK SIZE COMPARISON
// ============================================================================

fn bench_block_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_sizes");
    group.throughput(Throughput::Elements(1));

    let width = 256;
    let height = 256;
    let reference = generate_reference_frame(width, height);
    let current = generate_current_frame(&reference, width, height, 2, 2);

    let block_sizes = [
        (BlockSize::Block8x8, 64, 64),
        (BlockSize::Block16x16, 64, 64),
        (BlockSize::Block32x32, 64, 64),
        (BlockSize::Block64x64, 64, 64),
    ];

    for &(bsize, bx, by) in &block_sizes {
        let mut capsule = MotionEstimationCapsule::new();
        capsule.configure(32, SubPixelMode::Integer, SearchAlgorithm::Diamond);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", bsize)),
            &bsize,
            |b, _| {
                b.iter(|| {
                    black_box(capsule.estimate_block(
                        black_box(&reference),
                        black_box(&current),
                        black_box(width),
                        black_box(width),
                        black_box(bx),
                        black_box(by),
                        black_box(bsize),
                    ))
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// GROUP 3: SEARCH RANGE SCALING
// ============================================================================

fn bench_search_range_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_range_scaling");
    group.throughput(Throughput::Elements(1));

    let width = 128;
    let height = 128;
    let reference = generate_reference_frame(width, height);
    let current = generate_current_frame(&reference, width, height, 8, 8);

    let search_ranges = [16, 32, 64, 128];

    for &range in &search_ranges {
        let mut capsule = MotionEstimationCapsule::new();
        capsule.configure(range, SubPixelMode::Integer, SearchAlgorithm::Diamond);

        group.bench_with_input(BenchmarkId::new("diamond_search", range), &range, |b, _| {
            b.iter(|| {
                black_box(capsule.estimate_block(
                    black_box(&reference),
                    black_box(&current),
                    black_box(width),
                    black_box(width),
                    black_box(48),
                    black_box(48),
                    black_box(BlockSize::Block16x16),
                ))
            })
        });
    }

    group.finish();
}

// ============================================================================
// GROUP 4: SUB-PIXEL REFINEMENT MODES
// ============================================================================

fn bench_subpixel_modes(c: &mut Criterion) {
    let mut group = c.benchmark_group("subpixel_modes");
    group.throughput(Throughput::Elements(1));

    let width = 128;
    let height = 128;
    let reference = generate_reference_frame(width, height);
    let current = generate_current_frame(&reference, width, height, 4, 4);

    let subpixel_modes = [
        SubPixelMode::Integer,
        SubPixelMode::HalfPixel,
        SubPixelMode::QuarterPixel,
    ];

    for &mode in &subpixel_modes {
        let mut capsule = MotionEstimationCapsule::new();
        capsule.configure(32, mode, SearchAlgorithm::Diamond);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", mode)),
            &mode,
            |b, _| {
                b.iter(|| {
                    black_box(capsule.estimate_block(
                        black_box(&reference),
                        black_box(&current),
                        black_box(width),
                        black_box(width),
                        black_box(48),
                        black_box(48),
                        black_box(BlockSize::Block16x16),
                    ))
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// GROUP 5: RESOLUTION SCALING (HD/FHD)
// ============================================================================

fn bench_resolution_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("resolution_scaling");
    group.throughput(Throughput::Elements(1));

    let resolutions = [
        (128, 128, "SD_128x128"),
        (256, 256, "HD_256x256"),
        (512, 512, "FHD_512x512"),
    ];

    for &(width, height, label) in &resolutions {
        let reference = generate_reference_frame(width, height);
        let current = generate_current_frame(&reference, width, height, 4, 4);

        let mut capsule = MotionEstimationCapsule::new();
        capsule.configure(32, SubPixelMode::Integer, SearchAlgorithm::Diamond);

        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &(width, height),
            |b, _| {
                b.iter(|| {
                    black_box(capsule.estimate_block(
                        black_box(&reference),
                        black_box(&current),
                        black_box(width),
                        black_box(width),
                        black_box(width / 4),
                        black_box(height / 4),
                        black_box(BlockSize::Block16x16),
                    ))
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// GROUP 6: THROUGHPUT (MULTIPLE BLOCKS)
// ============================================================================

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    group.throughput(Throughput::Elements(64)); // 8×8 grid = 64 blocks
    group.sample_size(50); // Reduce sample size for long-running test

    let width = 256;
    let height = 256;
    let reference = generate_reference_frame(width, height);
    let current = generate_current_frame(&reference, width, height, 2, 2);

    let mut capsule = MotionEstimationCapsule::new();
    capsule.configure(32, SubPixelMode::Integer, SearchAlgorithm::Diamond);

    group.bench_function("64_blocks_8x8_grid", |b| {
        b.iter(|| {
            for y in (0..192).step_by(32) {
                for x in (0..192).step_by(32) {
                    black_box(capsule.estimate_block(
                        black_box(&reference),
                        black_box(&current),
                        black_box(width),
                        black_box(width),
                        black_box(x),
                        black_box(y),
                        black_box(BlockSize::Block16x16),
                    ));
                }
            }
        })
    });

    group.finish();
}

// ============================================================================
// GROUP 7: BASELINE COMPARISON (CPU FALLBACK)
// ============================================================================

fn bench_baseline_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline_comparison");
    group.throughput(Throughput::Elements(1));

    let width = 128;
    let height = 128;
    let reference = generate_reference_frame(width, height);
    let current = generate_current_frame(&reference, width, height, 4, 4);

    // Our CPU implementation (Diamond search, search_range=32, integer-only)
    let mut capsule = MotionEstimationCapsule::new();
    capsule.configure(32, SubPixelMode::Integer, SearchAlgorithm::Diamond);

    group.bench_function("cpu_diamond_16x16_r32", |b| {
        b.iter(|| {
            black_box(capsule.estimate_block(
                black_box(&reference),
                black_box(&current),
                black_box(width),
                black_box(width),
                black_box(48),
                black_box(48),
                black_box(BlockSize::Block16x16),
            ))
        })
    });

    // Note: rav1e baseline would require external dependency (not included here)
    // Benchmark reports will compare against published rav1e numbers (~80μs)

    group.finish();
}

// ============================================================================
// GROUP 8: GPU COORDINATION OVERHEAD
// ============================================================================

fn bench_gpu_coordination(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_coordination");
    group.throughput(Throughput::Elements(1));

    let width = 128;
    let height = 128;
    let reference = generate_reference_frame(width, height);
    let current = generate_current_frame(&reference, width, height, 4, 4);

    // CPU baseline (no GPU)
    let mut cpu_capsule = MotionEstimationCapsule::new();
    cpu_capsule.configure(32, SubPixelMode::Integer, SearchAlgorithm::Diamond);

    group.bench_function("cpu_baseline", |b| {
        b.iter(|| {
            black_box(cpu_capsule.estimate_block(
                black_box(&reference),
                black_box(&current),
                black_box(width),
                black_box(width),
                black_box(48),
                black_box(48),
                black_box(BlockSize::Block16x16),
            ))
        })
    });

    // GPU enabled (falls back to CPU if GPU unavailable)
    let mut gpu_capsule = MotionEstimationCapsule::new();
    gpu_capsule.configure(32, SubPixelMode::Integer, SearchAlgorithm::Diamond);
    gpu_capsule.enable_gpu(0x1234567890ABCDEF, 0); // Dummy handle (will fallback)

    group.bench_function("gpu_fallback", |b| {
        b.iter(|| {
            black_box(gpu_capsule.estimate_block(
                black_box(&reference),
                black_box(&current),
                black_box(width),
                black_box(width),
                black_box(48),
                black_box(48),
                black_box(BlockSize::Block16x16),
            ))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_cpu_search_algorithms,
    bench_block_sizes,
    bench_search_range_scaling,
    bench_subpixel_modes,
    bench_resolution_scaling,
    bench_throughput,
    bench_baseline_comparison,
    bench_gpu_coordination,
);

criterion_main!(benches);
