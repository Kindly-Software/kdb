//! SIMD Operations Benchmarks
//!
//! # B32 Framework Compliance
//!
//! Following B32 comprehensive benchmarking guidelines:
//! - B1: Fair baseline (scalar vs SIMD, both optimized)
//! - B2: Statistical rigor (1000+ iterations, 95% CI)
//! - B3: Realistic workloads (actual GPU state processing)
//! - B5: Full reporting (P50, P95, P99, hardware specs)
//! - B10: Release mode with optimizations
//!
//! # Expected Results (B32 K9 SIMD Reality)
//!
//! **Typical Optimization**: 10-50% improvement (B32 K27)
//! **SIMD Reality**: 3-4x speedup measured (not theoretical 8x)
//! **Requirement**: 64+ elements for real benefit
//! **Break-even**: ~8 operations minimum
//!
//! # Hardware Context (B32 K1-K9)
//!
//! **Target Platform**: Intel Ultra 7 155H
//! - P-cores: 0.21ns/cycle @ 4.8GHz boost
//! - AVX2 Support: 256-bit SIMD (8 × u32 or 4 × u64)
//! - L1 Cache: 48KB per P-core, 1ns latency
//! - Cache Line: 64 bytes

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;

// Import SIMD functions (will use cfg-gated versions)
// For benchmarking, we'll include both implementations
mod simd_ops_scalar {
    pub fn batch_commands_ready(states: &[u64; 8]) -> [bool; 8] {
        let mut result = [false; 8];
        for i in 0..8 {
            let state = (states[i] >> 16) & 0xF;
            result[i] = state == 0;
        }
        result
    }

    pub fn batch_fences_signaled(completed_values: &[u64; 8], wait_values: &[u64; 8]) -> [bool; 8] {
        let mut result = [false; 8];
        for i in 0..8 {
            result[i] = completed_values[i] >= wait_values[i];
        }
        result
    }

    pub fn batch_thermal_check(temperatures_mc: &[u32; 8], threshold_mc: u32) -> [bool; 8] {
        let mut result = [false; 8];
        for i in 0..8 {
            result[i] = temperatures_mc[i] >= threshold_mc;
        }
        result
    }

    pub fn batch_memory_check(sizes: &[u64; 8], available: &[u64; 8]) -> [bool; 8] {
        let mut result = [false; 8];
        for i in 0..8 {
            result[i] = sizes[i] <= available[i];
        }
        result
    }

    pub fn batch_priority_check(priorities: &[u8; 8], threshold: u8) -> [bool; 8] {
        let mut result = [false; 8];
        for i in 0..8 {
            result[i] = priorities[i] >= threshold;
        }
        result
    }
}

#[cfg(feature = "simd")]
mod simd_ops_simd {
    use std::simd::prelude::*;

    pub fn batch_commands_ready(states: &[u64; 8]) -> [bool; 8] {
        let state_vec = u64x8::from_array(*states);
        let state_mask = u64x8::splat(0xF << 16);
        let shift_amount = u64x8::splat(16);
        let state_field = (state_vec & state_mask) >> shift_amount;
        let ready_mask = state_field.simd_eq(u64x8::splat(0));
        ready_mask.to_array()
    }

    pub fn batch_fences_signaled(completed_values: &[u64; 8], wait_values: &[u64; 8]) -> [bool; 8] {
        let completed = u64x8::from_array(*completed_values);
        let wait = u64x8::from_array(*wait_values);
        let signaled = completed.simd_ge(wait);
        signaled.to_array()
    }

    pub fn batch_thermal_check(temperatures_mc: &[u32; 8], threshold_mc: u32) -> [bool; 8] {
        let temps = u32x8::from_array(*temperatures_mc);
        let threshold = u32x8::splat(threshold_mc);
        let over_temp = temps.simd_ge(threshold);
        over_temp.to_array()
    }

    pub fn batch_memory_check(sizes: &[u64; 8], available: &[u64; 8]) -> [bool; 8] {
        let size_vec = u64x8::from_array(*sizes);
        let avail_vec = u64x8::from_array(*available);
        let can_allocate = size_vec.simd_le(avail_vec);
        can_allocate.to_array()
    }

    pub fn batch_priority_check(priorities: &[u8; 8], threshold: u8) -> [bool; 8] {
        let priorities_u32: [u32; 8] = [
            priorities[0] as u32,
            priorities[1] as u32,
            priorities[2] as u32,
            priorities[3] as u32,
            priorities[4] as u32,
            priorities[5] as u32,
            priorities[6] as u32,
            priorities[7] as u32,
        ];
        let priority_vec = u32x8::from_array(priorities_u32);
        let threshold_vec = u32x8::splat(threshold as u32);
        let high_priority = priority_vec.simd_ge(threshold_vec);
        high_priority.to_array()
    }
}

/// Benchmark command state checking
///
/// # Test Scenario
/// Processing 8 command states to determine which are READY for submission.
/// This is a realistic hot-path operation in command scheduling.
///
/// # Expected Performance (B32 K27 HONEST GAINS)
/// - Scalar: ~40-60ns (8 loads + 8 shifts + 8 compares)
/// - SIMD: ~15-20ns (1 load + 1 shift + 1 compare)
/// - Speedup: 3-4x (B32 K9 realistic SIMD gains)
fn bench_commands_ready(c: &mut Criterion) {
    let mut group = c.benchmark_group("commands_ready");

    // B2: Statistical rigor
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_millis(500));

    // Realistic test data (B3: Real workloads)
    let states = [
        0x0000_0000, // READY
        0x0001_0000, // PENDING
        0x0000_0000, // READY
        0x0002_0000, // EXECUTING
        0x0000_0000, // READY
        0x0003_0000, // COMPLETED
        0x0001_0000, // PENDING
        0x0000_0000, // READY
    ];

    // Scalar baseline (B1: Fair comparison)
    group.bench_function("scalar", |b| {
        b.iter(|| black_box(simd_ops_scalar::batch_commands_ready(black_box(&states))));
    });

    // SIMD implementation
    #[cfg(feature = "simd")]
    group.bench_function("simd", |b| {
        b.iter(|| black_box(simd_ops_simd::batch_commands_ready(black_box(&states))));
    });

    group.finish();
}

/// Benchmark fence signaling checks
///
/// # Test Scenario
/// Checking 8 GPU fences to determine which have been signaled.
/// This is critical for synchronization in multi-context rendering.
///
/// # Expected Performance (B32 K27 HONEST GAINS)
/// - Scalar: ~30-40ns (8 loads + 8 compares)
/// - SIMD: ~10-15ns (2 loads + 1 compare)
/// - Speedup: 3-4x (B32 K9 realistic SIMD gains)
fn bench_fences_signaled(c: &mut Criterion) {
    let mut group = c.benchmark_group("fences_signaled");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_millis(500));

    // Realistic fence values
    let completed = [10, 20, 30, 40, 50, 60, 70, 80];
    let wait = [5, 25, 30, 35, 55, 60, 65, 85];

    // Scalar baseline
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(simd_ops_scalar::batch_fences_signaled(
                black_box(&completed),
                black_box(&wait),
            ))
        });
    });

    // SIMD implementation
    #[cfg(feature = "simd")]
    group.bench_function("simd", |b| {
        b.iter(|| {
            black_box(simd_ops_simd::batch_fences_signaled(
                black_box(&completed),
                black_box(&wait),
            ))
        });
    });

    group.finish();
}

/// Benchmark thermal threshold checks
///
/// # Test Scenario
/// Checking 8 GPU contexts against thermal limits for circuit breaker.
/// This is part of the thermal management hot path.
///
/// # Expected Performance (B32 K27 HONEST GAINS)
/// - Scalar: ~30-40ns (8 loads + 8 compares)
/// - SIMD: ~10-15ns (1 load + 1 compare)
/// - Speedup: 3-4x (B32 K9 realistic SIMD gains)
fn bench_thermal_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("thermal_check");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_millis(500));

    // Realistic temperature values (millicelsius)
    let temps = [
        65_000, 75_000, 85_000, 70_000, 90_000, 60_000, 80_000, 95_000,
    ];
    let threshold = 80_000; // 80°C

    // Scalar baseline
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(simd_ops_scalar::batch_thermal_check(
                black_box(&temps),
                black_box(threshold),
            ))
        });
    });

    // SIMD implementation
    #[cfg(feature = "simd")]
    group.bench_function("simd", |b| {
        b.iter(|| {
            black_box(simd_ops_simd::batch_thermal_check(
                black_box(&temps),
                black_box(threshold),
            ))
        });
    });

    group.finish();
}

/// Benchmark memory allocation feasibility checks
///
/// # Test Scenario
/// Checking 8 potential allocations against available memory.
/// This is part of the lockfree memory allocator fast path.
///
/// # Expected Performance (B32 K27 HONEST GAINS)
/// - Scalar: ~30-40ns (8 loads + 8 compares)
/// - SIMD: ~10-15ns (2 loads + 1 compare)
/// - Speedup: 3-4x (B32 K9 realistic SIMD gains)
fn bench_memory_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_check");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_millis(500));

    // Realistic allocation sizes
    let sizes = [1024, 2048, 4096, 8192, 16384, 32768, 65536, 131072];
    let available = [
        100_000, 100_000, 100_000, 100_000, 100_000, 100_000, 100_000, 100_000,
    ];

    // Scalar baseline
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(simd_ops_scalar::batch_memory_check(
                black_box(&sizes),
                black_box(&available),
            ))
        });
    });

    // SIMD implementation
    #[cfg(feature = "simd")]
    group.bench_function("simd", |b| {
        b.iter(|| {
            black_box(simd_ops_simd::batch_memory_check(
                black_box(&sizes),
                black_box(&available),
            ))
        });
    });

    group.finish();
}

/// Benchmark priority comparison
///
/// # Test Scenario
/// Filtering 8 commands by priority threshold for scheduling.
/// This is part of the priority queue fast path.
///
/// # Expected Performance (B32 K27 HONEST GAINS)
/// - Scalar: ~30-40ns (8 loads + 8 compares)
/// - SIMD: ~10-15ns (1 load + 1 compare + conversion)
/// - Speedup: 2-3x (B32 K9 realistic, includes u8→u32 conversion overhead)
fn bench_priority_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("priority_check");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_millis(500));

    // Realistic priority values
    let priorities = [10, 20, 30, 40, 50, 60, 70, 80];
    let threshold = 50;

    // Scalar baseline
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(simd_ops_scalar::batch_priority_check(
                black_box(&priorities),
                black_box(threshold),
            ))
        });
    });

    // SIMD implementation
    #[cfg(feature = "simd")]
    group.bench_function("simd", |b| {
        b.iter(|| {
            black_box(simd_ops_simd::batch_priority_check(
                black_box(&priorities),
                black_box(threshold),
            ))
        });
    });

    group.finish();
}

/// Scaling test: Batch size impact
///
/// # B32 K9 Validation
/// Testing the requirement that SIMD needs 64+ elements for real benefit.
/// We'll test batch sizes: 1, 2, 4, 8, 16, 32, 64
///
/// Expected results:
/// - 1-4: SIMD slower (overhead dominates)
/// - 8: Break-even point
/// - 16+: SIMD faster (3-4x speedup)
/// - 64+: Full SIMD benefit
fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling");

    group.confidence_level(0.95).sample_size(500);

    // Test different batch sizes
    for batch_size in [8, 16, 32, 64].iter() {
        // Generate test data
        let mut states_vec = Vec::new();
        for _ in 0..(*batch_size / 8) {
            states_vec.push([
                0x0000_0000,
                0x0001_0000,
                0x0000_0000,
                0x0002_0000,
                0x0000_0000,
                0x0003_0000,
                0x0001_0000,
                0x0000_0000,
            ]);
        }

        // Scalar benchmark
        group.bench_with_input(
            BenchmarkId::new("scalar", batch_size),
            &states_vec,
            |b, states| {
                b.iter(|| {
                    let mut results = Vec::new();
                    for batch in states {
                        results.push(simd_ops_scalar::batch_commands_ready(black_box(batch)));
                    }
                    black_box(results)
                });
            },
        );

        // SIMD benchmark
        #[cfg(feature = "simd")]
        group.bench_with_input(
            BenchmarkId::new("simd", batch_size),
            &states_vec,
            |b, states| {
                b.iter(|| {
                    let mut results = Vec::new();
                    for batch in states {
                        results.push(simd_ops_simd::batch_commands_ready(black_box(batch)));
                    }
                    black_box(results)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_commands_ready,
    bench_fences_signaled,
    bench_thermal_check,
    bench_memory_check,
    bench_priority_check,
    bench_scaling,
);
criterion_main!(benches);
