//! B32 GPU Motion Estimation Benchmarks
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! # Framework: Criterion
//!
//! - 95% confidence interval (B32 requirement)
//! - 1000+ iterations (via sample_size)
//! - Fair baseline comparison (CPU diamond search)
//!
//! # Methodology
//!
//! - Same hardware (kindly-hub: AMD Ryzen 9 6900HX, 64GB DDR5)
//! - Same compiler flags (--release)
//! - Same test data (synthetic frames with motion)
//! - Separate GPU and CPU runs to avoid interference
//!
//! # Performance Targets
//!
//! | Backend | Target Speedup | Per-Frame (1080p) | Per-Frame (4K) |
//! |---------|----------------|-------------------|----------------|
//! | ROCm    | 100-500×       | <1ms              | <5ms           |
//! | Vulkan  | 50-200×        | <2ms              | <10ms          |
//! | CPU     | 1× (baseline)  | 35-45ms           | ~50ms          |
//!
//! # B32 Compliance
//!
//! - ✅ Q1: 95% CI via Criterion confidence_level
//! - ✅ Q2: 1000+ iterations via sample_size(100)
//! - ✅ Q3: Fair baseline (optimized CPU diamond search)
//! - ✅ Q4: Reproducible (same hardware, kindly-hub)
//! - ✅ Q5: Realistic workloads (64x64, 320x240, 720p, 1080p, 4K UHD)
//! - ✅ Q6: Statistical validation (Criterion built-in)
//!
//! # Run Commands
//!
//! ```bash
//! # All benchmarks (run on kindly-hub)
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_motion_bench"
//!
//! # Specific resolution
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_motion_bench -- 1080p"
//!
//! # CPU-only baseline
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_motion_bench -- cpu"
//!
//! # GPU-only
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_motion_bench -- gpu"
//! ```

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use kindly_av1::encoder::GpuMotionEstimationCapsule;

/// Helper: Create synthetic test frames with motion pattern
///
/// Creates frames where a bright square moves from center to (dx, dy).
/// Simulates real encoding workload better than random data.
///
/// # Performance Note
///
/// Frame generation overhead is excluded from benchmarks via
/// separate setup closures.
fn create_test_frames(width: u32, height: u32, dx: i32, dy: i32) -> (Vec<u8>, Vec<u8>) {
    // Base gray background
    let mut current = vec![64u8; (width * height) as usize];
    let mut reference = vec![64u8; (width * height) as usize];

    // Bright 32x32 square with motion
    let square_size = 32;
    let base_x = (width / 4) as i32;
    let base_y = (height / 4) as i32;

    for y in 0..square_size {
        for x in 0..square_size {
            // Current frame position
            let curr_x = (base_x + dx + x) as usize;
            let curr_y = (base_y + dy + y) as usize;

            if curr_x < width as usize && curr_y < height as usize {
                current[curr_y * width as usize + curr_x] = 200;
            }

            // Reference frame position (no offset)
            let ref_x = (base_x + x) as usize;
            let ref_y = (base_y + y) as usize;

            if ref_x < width as usize && ref_y < height as usize {
                reference[ref_y * width as usize + ref_x] = 200;
            }
        }
    }

    (current, reference)
}

/// Benchmark GPU vs CPU motion estimation across multiple resolutions
///
/// # Resolutions
///
/// - 64x64: Unit test size
/// - 320x240: Classic QVGA
/// - 1280x720: 720p (HD)
/// - 1920x1088: 1080p (16-aligned)
/// - 3840x2160: 4K UHD (Wave 2 end-to-end testing)
///
/// # Methodology
///
/// - Each resolution tested with both GPU and CPU backends
/// - Same test data for both backends
/// - Criterion handles statistical analysis
fn benchmark_motion_estimation(c: &mut Criterion) {
    let mut group = c.benchmark_group("motion_estimation");

    // B32 compliance: 100 samples for statistical significance
    // Each sample runs the function 10-100 times depending on duration
    // Total iterations: 100 * 10+ = 1000+ (exceeds B32 requirement)
    group.sample_size(100);

    // B32 compliance: 95% confidence interval
    group.confidence_level(0.95);

    // Test configurations: (name, width, height, motion_x, motion_y)
    let configs = [
        ("64x64", 64u32, 64u32, 4i32, 2i32),
        ("320x240", 320, 240, 8, 4),
        ("1280x720", 1280, 720, 16, 8),
        ("1920x1088", 1920, 1088, 16, 8), // 1080p rounded to 16-align
        ("3840x2160", 3840, 2160, 16, 8), // 4K (UHD)
                                          // Performance targets (based on 1080p @ 1.37ms scaled to 4× pixels):
                                          // - CPU target: ~5.5ms per frame (scaled from 1.37ms × 4)
                                          // - GPU target: <0.5ms per frame (10-20× speedup over CPU)
                                          // Memory: 8.3M pixels × 2 frames = ~33 MB (16.6 MB per frame, f32 grayscale)
    ];

    for (name, width, height, dx, dy) in configs {
        // Pre-generate test frames to exclude setup overhead from benchmarks
        let (current, reference) = create_test_frames(width, height, dx, dy);
        let frame_size = (width * height) as usize;

        println!(
            "\n[gpu_motion_bench] Testing resolution: {} ({}x{})",
            name, width, height
        );
        println!(
            "[gpu_motion_bench] Frame size: {} bytes ({:.2} MB)",
            frame_size * 2,
            (frame_size * 2) as f64 / 1_048_576.0
        );

        // ====================================================================
        // CPU Baseline Benchmark
        // ====================================================================

        {
            let capsule = GpuMotionEstimationCapsule::new();
            capsule.disable_gpu(); // Force CPU-only mode

            // Verify CPU works before benchmarking
            let test_result = capsule.estimate_frame(&current, &reference, width, height);
            if let Err(e) = test_result {
                eprintln!(
                    "[gpu_motion_bench] CPU estimation failed for {}: {}",
                    name, e
                );
                continue; // Skip this resolution
            }

            group.bench_with_input(
                BenchmarkId::new("cpu", name),
                &(width, height),
                |b, &(w, h)| {
                    // Benchmark excludes frame generation overhead
                    b.iter(|| {
                        capsule
                            .estimate_frame(&current, &reference, w, h)
                            .expect("CPU estimation failed")
                    })
                },
            );

            let stats = capsule.stats();
            println!(
                "[gpu_motion_bench] CPU baseline: {} frames processed",
                stats.cpu_frames
            );
        }

        // ====================================================================
        // GPU Benchmark (if available)
        // ====================================================================

        {
            let capsule = GpuMotionEstimationCapsule::new();

            if capsule.is_gpu_available() {
                capsule.enable_gpu();

                // Verify GPU works before benchmarking
                let test_result = capsule.estimate_frame(&current, &reference, width, height);

                match test_result {
                    Ok(_) => {
                        println!("[gpu_motion_bench] GPU backend: {:?}", capsule.backend());

                        group.bench_with_input(
                            BenchmarkId::new("gpu", name),
                            &(width, height),
                            |b, &(w, h)| {
                                b.iter(|| {
                                    capsule
                                        .estimate_frame(&current, &reference, w, h)
                                        .expect("GPU estimation failed")
                                })
                            },
                        );

                        let stats = capsule.stats();
                        println!(
                            "[gpu_motion_bench] GPU: {} frames processed, backend: {}",
                            stats.gpu_frames, stats.device_name
                        );
                    }
                    Err(e) => {
                        println!("[gpu_motion_bench] GPU unavailable for {}: {}", name, e);
                        println!("[gpu_motion_bench] Skipping GPU benchmark for this resolution");
                    }
                }
            } else {
                println!("[gpu_motion_bench] GPU not available on this system");
                println!("[gpu_motion_bench] CPU baseline only will be benchmarked");
            }
        }
    }

    group.finish();
}

/// Benchmark search range sensitivity
///
/// # Methodology
///
/// Tests how search range affects performance:
/// - Small range (8 pixels): Fast, may miss large motion
/// - Default range (16 pixels): Balanced
/// - Large range (32 pixels): Slow, catches large motion
///
/// # Expected
///
/// - Larger search range = more computation
/// - Linear or quadratic scaling depending on algorithm
fn benchmark_search_range_sensitivity(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_range_sensitivity");
    group.sample_size(50); // Fewer samples for sensitivity analysis

    let width = 320u32;
    let height = 240u32;
    let (current, reference) = create_test_frames(width, height, 8, 4);

    for range in [8u64, 16, 32] {
        let capsule = GpuMotionEstimationCapsule::new();
        capsule.disable_gpu(); // CPU-only for determinism
        capsule.set_search_range(range);

        group.bench_with_input(BenchmarkId::new("cpu_range", range), &range, |b, _| {
            b.iter(|| {
                capsule
                    .estimate_frame(&current, &reference, width, height)
                    .expect("Estimation failed")
            })
        });
    }

    group.finish();
}

/// Benchmark batch size tuning (GPU-specific)
///
/// # Methodology
///
/// Tests GPU batch size impact:
/// - Small batch (32): Low latency, low utilization
/// - Default batch (64): Balanced
/// - Large batch (128): High throughput, higher latency
///
/// # Expected
///
/// - Larger batch = better GPU utilization
/// - Diminishing returns beyond optimal size
#[cfg(all(target_os = "linux", feature = "gpu-rocm"))]
fn benchmark_batch_size_tuning(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_size_tuning");
    group.sample_size(50);

    let width = 1280u32;
    let height = 720u32;
    let (current, reference) = create_test_frames(width, height, 16, 8);

    let capsule = GpuMotionEstimationCapsule::new();

    if capsule.is_gpu_available() {
        for batch in [32u64, 64, 128] {
            capsule.set_batch_size(batch);

            group.bench_with_input(BenchmarkId::new("gpu_batch", batch), &batch, |b, _| {
                b.iter(|| {
                    capsule
                        .estimate_frame(&current, &reference, width, height)
                        .expect("GPU estimation failed")
                })
            });
        }
    } else {
        println!("[gpu_motion_bench] GPU unavailable, skipping batch size tuning");
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

// Conditional compilation for batch size tuning (GPU-only)
#[cfg(all(target_os = "linux", feature = "gpu-rocm"))]
criterion_group!(
    benches,
    benchmark_motion_estimation,
    benchmark_search_range_sensitivity,
    benchmark_batch_size_tuning,
);

#[cfg(not(all(target_os = "linux", feature = "gpu-rocm")))]
criterion_group!(
    benches,
    benchmark_motion_estimation,
    benchmark_search_range_sensitivity,
);

criterion_main!(benches);
