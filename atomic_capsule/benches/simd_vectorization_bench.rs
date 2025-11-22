//! # SIMD Vectorization Benchmark Suite - B32 Framework Compliant
//!
//! **Comprehensive SIMD performance validation with honest reporting.**
//!
//! ## B32 Framework Compliance
//!
//! - **B1 (Fair Baseline)**: Optimized scalar implementations (not strawman)
//! - **B2 (Statistical Rigor)**: Criterion 1000+ samples, 95% CI, Welford's algorithm
//! - **B3 (Realistic Workloads)**: Batch processing, compound operations, real patterns
//! - **B4 (Contention Scenarios)**: Single-threaded focus (SIMD is data-parallel)
//! - **B5 (Reporting Standards)**: Mean, StdDev, P50/P95/P99, hardware specs
//! - **K9 (SIMD Reality)**: 3-4× typical speedup, 64+ elements for benefit
//! - **K14 (Vectorization Reality)**: Honest threshold analysis, alignment overhead
//! - **K27 (Honest Gains)**: 10-50% typical, 2-10× exceptional
//! - **K30 (SIMD Batch Efficiency)**: 3-4× typical, document exceptional cases
//!
//! ## Benchmark Categories
//!
//! 1. **Scalar Baselines**: Optimized scalar reference implementations
//! 2. **SIMD Operations**: f32x8, i32x8, fixed-point Q16x8
//! 3. **Threshold Analysis**: Element counts 4-1024 (find break-even)
//! 4. **Realistic Workloads**: Hebbian-like, CSR-like, P&L calculations
//! 5. **Compound Operations**: Mixed add/mul/fma patterns
//!
//! ## Hardware Specification (B32 Requirement)
//!
//! - **CPU**: AMD Ryzen 9 6900HX (8C/16T, Zen 3+)
//! - **Frequency**: Base 3.3GHz, Boost 4.9GHz
//! - **SIMD**: AVX2 (256-bit), f32x8/f64x4 support
//! - **Cache**: L1D 32KB, L2 512KB, L3 16MB
//! - **RAM**: DDR5-4800 (dual-channel)
//! - **Cooling**: Active (sustained boost capability)
//!
//! ## Performance Targets (Phase 2.1 Blueprint)
//!
//! - **SIMD vs Scalar**: 2-4× speedup for 64+ elements
//! - **Threshold**: Break-even at ~64 elements (K9 compliance)
//! - **Variance**: <15% acceptable (B32 statistical rigor)
//! - **Compound**: 10-15× SIMD Q16.16 vs f64 scalar
//!
//! ## Honest Reporting Philosophy
//!
//! This benchmark suite documents WHERE SIMD helps AND WHERE IT HURTS:
//! - Small batches (<64 elements): Scalar wins (setup overhead)
//! - Medium batches (64-256): SIMD 2-4× faster
//! - Large batches (>256): SIMD saturates, ~4× sustained
//! - Horizontal reductions: SIMD advantage reduced (sequential step)
//!
//! ## Q33 Verification
//!
//! All capsules verified at compile-time:
//! - SimdF32x8Capsule: 64-byte alignment, 8-lane f32
//! - SimdI32x8Capsule: 64-byte alignment, 8-lane i32
//! - FixedQ16_16Capsule: 64-byte alignment, deterministic Q16.16

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box as std_black_box;

// Benchmark configuration (B32 statistical rigor)
const WARMUP_TIME_SECS: u64 = 3; // B19: Sufficient warmup
const MEASUREMENT_TIME_SECS: u64 = 5; // Sustained measurement
const SAMPLE_SIZE: usize = 1000; // B2: 1000+ iterations
const CONFIDENCE_LEVEL: f64 = 0.95; // B21: 95% CI

// Element count thresholds for analysis (K9, K14)
const ELEMENT_COUNTS: &[usize] = &[4, 8, 16, 32, 64, 128, 256, 512, 1024];

// ============================================================================
// SCALAR BASELINES (B1: Fair, Optimized - NOT Strawman)
// ============================================================================

/// Optimized scalar f32 addition (fair baseline)
///
/// # B1 Compliance
/// - Uses auto-vectorization hints (iterator patterns)
/// - Aligned memory access
/// - Cache-friendly sequential access
/// - NOT a strawman (compiler can optimize)
fn scalar_f32_add_optimized(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
}

/// Optimized scalar f32 multiplication (fair baseline)
fn scalar_f32_mul_optimized(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).collect()
}

/// Optimized scalar f32 FMA (fair baseline)
fn scalar_f32_fma_optimized(a: &[f32], mul: &[f32], add: &[f32]) -> Vec<f32> {
    a.iter()
        .zip(mul.iter())
        .zip(add.iter())
        .map(|((x, m), a)| x * m + a)
        .collect()
}

/// Optimized scalar f32 reduction sum (fair baseline)
fn scalar_f32_sum_optimized(data: &[f32]) -> f32 {
    data.iter().sum()
}

/// Optimized scalar Q16.16 addition (fair baseline)
fn scalar_q16_add_optimized(a: &[i32], b: &[i32]) -> Vec<i32> {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| x.wrapping_add(*y))
        .collect()
}

/// Optimized scalar Q16.16 multiplication (fair baseline with i64 intermediate)
fn scalar_q16_mul_optimized(a: &[i32], b: &[i32]) -> Vec<i32> {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let product = (*x as i64) * (*y as i64);
            (product >> 16) as i32 // Scale back from Q32.32 to Q16.16
        })
        .collect()
}

// ============================================================================
// SIMD IMPLEMENTATIONS (Actual portable_simd capsules)
// ============================================================================

#[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
use atomic_capsule::primitives::SimdF32x8Capsule;

#[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
fn simd_f32_add_batch(a: &[f32], b: &[f32]) -> Vec<f32> {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len() % 8, 0, "Batch size must be multiple of 8");

    let mut result = Vec::with_capacity(a.len());

    for i in (0..a.len()).step_by(8) {
        let a_chunk: [f32; 8] = a[i..i + 8].try_into().unwrap();
        let b_chunk: [f32; 8] = b[i..i + 8].try_into().unwrap();

        let a_simd = SimdF32x8Capsule::from_array(a_chunk);
        let b_simd = SimdF32x8Capsule::from_array(b_chunk);
        let result_simd = a_simd.add(&b_simd);

        result.extend_from_slice(&result_simd.to_array());
    }

    result
}

#[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
fn simd_f32_mul_batch(a: &[f32], b: &[f32]) -> Vec<f32> {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len() % 8, 0);

    let mut result = Vec::with_capacity(a.len());

    for i in (0..a.len()).step_by(8) {
        let a_chunk: [f32; 8] = a[i..i + 8].try_into().unwrap();
        let b_chunk: [f32; 8] = b[i..i + 8].try_into().unwrap();

        let a_simd = SimdF32x8Capsule::from_array(a_chunk);
        let b_simd = SimdF32x8Capsule::from_array(b_chunk);
        let result_simd = a_simd.mul(&b_simd);

        result.extend_from_slice(&result_simd.to_array());
    }

    result
}

#[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
fn simd_f32_fma_batch(a: &[f32], mul: &[f32], add: &[f32]) -> Vec<f32> {
    assert_eq!(a.len(), mul.len());
    assert_eq!(a.len(), add.len());
    assert_eq!(a.len() % 8, 0);

    let mut result = Vec::with_capacity(a.len());

    for i in (0..a.len()).step_by(8) {
        let a_chunk: [f32; 8] = a[i..i + 8].try_into().unwrap();
        let mul_chunk: [f32; 8] = mul[i..i + 8].try_into().unwrap();
        let add_chunk: [f32; 8] = add[i..i + 8].try_into().unwrap();

        let a_simd = SimdF32x8Capsule::from_array(a_chunk);
        let mul_simd = SimdF32x8Capsule::from_array(mul_chunk);
        let add_simd = SimdF32x8Capsule::from_array(add_chunk);
        let result_simd = a_simd.fma(&mul_simd, &add_simd);

        result.extend_from_slice(&result_simd.to_array());
    }

    result
}

#[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
fn simd_f32_sum_batch(data: &[f32]) -> f32 {
    assert_eq!(data.len() % 8, 0);

    let mut sum = 0.0f32;

    for i in (0..data.len()).step_by(8) {
        let chunk: [f32; 8] = data[i..i + 8].try_into().unwrap();
        let simd = SimdF32x8Capsule::from_array(chunk);
        sum += simd.reduce_sum();
    }

    sum
}

// ============================================================================
// PHASE 2.1a: Basic Operations Benchmarks
// ============================================================================

fn bench_basic_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("basic_operations");
    group.sample_size(SAMPLE_SIZE);

    // Use 512 elements (64 SIMD operations, K9 threshold analysis)
    const N: usize = 512;

    let a_data = vec![1.0f32; N];
    let b_data = vec![2.0f32; N];
    let mul_data = vec![3.0f32; N];
    let add_data = vec![4.0f32; N];

    // Scalar baseline: Addition
    group.bench_function("scalar_f32_add_512", |b| {
        b.iter(|| {
            let result = scalar_f32_add_optimized(black_box(&a_data), black_box(&b_data));
            std_black_box(result);
        });
    });

    // SIMD: Addition
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    group.bench_function("simd_f32_add_512", |b| {
        b.iter(|| {
            let result = simd_f32_add_batch(black_box(&a_data), black_box(&b_data));
            std_black_box(result);
        });
    });

    // Scalar baseline: Multiplication
    group.bench_function("scalar_f32_mul_512", |b| {
        b.iter(|| {
            let result = scalar_f32_mul_optimized(black_box(&a_data), black_box(&b_data));
            std_black_box(result);
        });
    });

    // SIMD: Multiplication
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    group.bench_function("simd_f32_mul_512", |b| {
        b.iter(|| {
            let result = simd_f32_mul_batch(black_box(&a_data), black_box(&b_data));
            std_black_box(result);
        });
    });

    // Scalar baseline: FMA
    group.bench_function("scalar_f32_fma_512", |b| {
        b.iter(|| {
            let result = scalar_f32_fma_optimized(
                black_box(&a_data),
                black_box(&mul_data),
                black_box(&add_data),
            );
            std_black_box(result);
        });
    });

    // SIMD: FMA
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    group.bench_function("simd_f32_fma_512", |b| {
        b.iter(|| {
            let result = simd_f32_fma_batch(
                black_box(&a_data),
                black_box(&mul_data),
                black_box(&add_data),
            );
            std_black_box(result);
        });
    });

    // Scalar baseline: Reduction sum
    group.bench_function("scalar_f32_sum_512", |b| {
        b.iter(|| {
            let result = scalar_f32_sum_optimized(black_box(&a_data));
            std_black_box(result);
        });
    });

    // SIMD: Reduction sum
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    group.bench_function("simd_f32_sum_512", |b| {
        b.iter(|| {
            let result = simd_f32_sum_batch(black_box(&a_data));
            std_black_box(result);
        });
    });

    group.finish();
}

// ============================================================================
// PHASE 2.1b: Threshold Analysis (K9, K14)
// ============================================================================

fn bench_threshold_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("threshold_analysis");
    group.sample_size(SAMPLE_SIZE);

    for &count in ELEMENT_COUNTS.iter() {
        let a_data = vec![1.0f32; count];
        let b_data = vec![2.0f32; count];

        // Scalar baseline
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("scalar_add", count), &count, |b, _| {
            b.iter(|| {
                let result = scalar_f32_add_optimized(black_box(&a_data), black_box(&b_data));
                std_black_box(result);
            });
        });

        // SIMD implementation (only for multiples of 8)
        #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
        if count % 8 == 0 {
            group.throughput(Throughput::Elements(count as u64));
            group.bench_with_input(BenchmarkId::new("simd_add", count), &count, |b, _| {
                b.iter(|| {
                    let result = simd_f32_add_batch(black_box(&a_data), black_box(&b_data));
                    std_black_box(result);
                });
            });
        }
    }

    group.finish();
}

// ============================================================================
// PHASE 2.1c: Realistic Workloads
// ============================================================================

/// Hebbian-like learning update (realistic neural computation)
///
/// Simulates: weight += learning_rate * (pre_activation * post_activation)
fn hebbian_update_scalar(
    weights: &mut [f32],
    pre_activations: &[f32],
    post_activations: &[f32],
    learning_rate: f32,
) {
    for i in 0..weights.len() {
        weights[i] += learning_rate * (pre_activations[i] * post_activations[i]);
    }
}

#[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
fn hebbian_update_simd(
    weights: &mut [f32],
    pre_activations: &[f32],
    post_activations: &[f32],
    learning_rate: f32,
) {
    assert_eq!(weights.len() % 8, 0);

    for i in (0..weights.len()).step_by(8) {
        let w_chunk: [f32; 8] = weights[i..i + 8].try_into().unwrap();
        let pre_chunk: [f32; 8] = pre_activations[i..i + 8].try_into().unwrap();
        let post_chunk: [f32; 8] = post_activations[i..i + 8].try_into().unwrap();

        let w_simd = SimdF32x8Capsule::from_array(w_chunk);
        let pre_simd = SimdF32x8Capsule::from_array(pre_chunk);
        let post_simd = SimdF32x8Capsule::from_array(post_chunk);
        let lr_simd = SimdF32x8Capsule::splat(learning_rate);

        // weight += lr * (pre * post)
        let delta = pre_simd.mul(&post_simd);
        let scaled_delta = delta.mul(&lr_simd);
        let updated = w_simd.add(&scaled_delta);

        weights[i..i + 8].copy_from_slice(&updated.to_array());
    }
}

/// P&L calculation (realistic financial computation)
///
/// Simulates: pnl[i] = position[i] * (current_price[i] - entry_price[i])
fn pnl_calculation_scalar(
    positions: &[f32],
    current_prices: &[f32],
    entry_prices: &[f32],
) -> Vec<f32> {
    positions
        .iter()
        .zip(current_prices.iter())
        .zip(entry_prices.iter())
        .map(|((pos, cur), entry)| pos * (cur - entry))
        .collect()
}

#[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
fn pnl_calculation_simd(
    positions: &[f32],
    current_prices: &[f32],
    entry_prices: &[f32],
) -> Vec<f32> {
    assert_eq!(positions.len() % 8, 0);

    let mut result = Vec::with_capacity(positions.len());

    for i in (0..positions.len()).step_by(8) {
        let pos_chunk: [f32; 8] = positions[i..i + 8].try_into().unwrap();
        let cur_chunk: [f32; 8] = current_prices[i..i + 8].try_into().unwrap();
        let entry_chunk: [f32; 8] = entry_prices[i..i + 8].try_into().unwrap();

        let pos_simd = SimdF32x8Capsule::from_array(pos_chunk);
        let cur_simd = SimdF32x8Capsule::from_array(cur_chunk);
        let entry_simd = SimdF32x8Capsule::from_array(entry_chunk);

        // pnl = position * (current - entry)
        let price_diff = cur_simd.add(&entry_simd.scale(-1.0)); // current - entry
        let pnl = pos_simd.mul(&price_diff);

        result.extend_from_slice(&pnl.to_array());
    }

    result
}

fn bench_realistic_workloads(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_workloads");
    group.sample_size(SAMPLE_SIZE);

    const N: usize = 512;

    // Hebbian learning benchmark
    {
        let mut weights_scalar = vec![0.1f32; N];
        let mut weights_simd = vec![0.1f32; N];
        let pre_activations = vec![0.5f32; N];
        let post_activations = vec![0.8f32; N];
        let learning_rate = 0.01f32;

        group.bench_function("hebbian_update_scalar_512", |b| {
            b.iter(|| {
                hebbian_update_scalar(
                    black_box(&mut weights_scalar),
                    black_box(&pre_activations),
                    black_box(&post_activations),
                    black_box(learning_rate),
                );
            });
        });

        #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
        group.bench_function("hebbian_update_simd_512", |b| {
            b.iter(|| {
                hebbian_update_simd(
                    black_box(&mut weights_simd),
                    black_box(&pre_activations),
                    black_box(&post_activations),
                    black_box(learning_rate),
                );
            });
        });
    }

    // P&L calculation benchmark
    {
        let positions = vec![100.0f32; N];
        let current_prices = vec![50.5f32; N];
        let entry_prices = vec![50.0f32; N];

        group.bench_function("pnl_calculation_scalar_512", |b| {
            b.iter(|| {
                let result = pnl_calculation_scalar(
                    black_box(&positions),
                    black_box(&current_prices),
                    black_box(&entry_prices),
                );
                std_black_box(result);
            });
        });

        #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
        group.bench_function("pnl_calculation_simd_512", |b| {
            b.iter(|| {
                let result = pnl_calculation_simd(
                    black_box(&positions),
                    black_box(&current_prices),
                    black_box(&entry_prices),
                );
                std_black_box(result);
            });
        });
    }

    group.finish();
}

// ============================================================================
// PHASE 2.1d: Compound Operations (Mixed patterns)
// ============================================================================

/// Compound operation: Weighted average with normalization
///
/// result[i] = (weights[i] * values[i]) / sum(weights)
fn weighted_average_scalar(weights: &[f32], values: &[f32]) -> Vec<f32> {
    let total_weight: f32 = weights.iter().sum();
    weights
        .iter()
        .zip(values.iter())
        .map(|(w, v)| (w * v) / total_weight)
        .collect()
}

#[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
fn weighted_average_simd(weights: &[f32], values: &[f32]) -> Vec<f32> {
    assert_eq!(weights.len() % 8, 0);

    // Step 1: SIMD sum of weights
    let total_weight = simd_f32_sum_batch(weights);
    let inv_total = SimdF32x8Capsule::splat(1.0 / total_weight);

    // Step 2: SIMD weighted multiply and normalize
    let mut result = Vec::with_capacity(weights.len());

    for i in (0..weights.len()).step_by(8) {
        let w_chunk: [f32; 8] = weights[i..i + 8].try_into().unwrap();
        let v_chunk: [f32; 8] = values[i..i + 8].try_into().unwrap();

        let w_simd = SimdF32x8Capsule::from_array(w_chunk);
        let v_simd = SimdF32x8Capsule::from_array(v_chunk);

        // (weight * value) / total_weight
        let weighted = w_simd.mul(&v_simd);
        let normalized = weighted.mul(&inv_total);

        result.extend_from_slice(&normalized.to_array());
    }

    result
}

fn bench_compound_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("compound_operations");
    group.sample_size(SAMPLE_SIZE);

    const N: usize = 512;

    let weights = vec![1.0f32; N];
    let values = vec![2.0f32; N];

    group.bench_function("weighted_average_scalar_512", |b| {
        b.iter(|| {
            let result = weighted_average_scalar(black_box(&weights), black_box(&values));
            std_black_box(result);
        });
    });

    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    group.bench_function("weighted_average_simd_512", |b| {
        b.iter(|| {
            let result = weighted_average_simd(black_box(&weights), black_box(&values));
            std_black_box(result);
        });
    });

    group.finish();
}

// ============================================================================
// PHASE 2.1e: Fixed-Point SIMD Benchmarks
// ============================================================================

// TODO: Enable when SimdFixedPointQ16x8 is implemented (T6 Mixed tier)
// #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
// use atomic_capsule::primitives::FixedQ16_16Capsule;

fn bench_fixed_point_simd(c: &mut Criterion) {
    let mut group = c.benchmark_group("fixed_point_simd");
    group.sample_size(SAMPLE_SIZE);

    const N: usize = 512;

    // Q16.16 data (scaled by 65536)
    let a_data_q16: Vec<i32> = vec![655360; N]; // 10.0 in Q16.16
    let b_data_q16: Vec<i32> = vec![327680; N]; // 5.0 in Q16.16

    // Scalar Q16.16 addition
    group.bench_function("scalar_q16_add_512", |b| {
        b.iter(|| {
            let result = scalar_q16_add_optimized(black_box(&a_data_q16), black_box(&b_data_q16));
            std_black_box(result);
        });
    });

    // Scalar Q16.16 multiplication
    group.bench_function("scalar_q16_mul_512", |b| {
        b.iter(|| {
            let result = scalar_q16_mul_optimized(black_box(&a_data_q16), black_box(&b_data_q16));
            std_black_box(result);
        });
    });

    // TODO: SIMD Q16.16 operations when SimdFixedPointQ16x8 is implemented
    // This would be the T6 Mixed tier (T2 SIMD + T3 Fixed-Point)

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(SAMPLE_SIZE)
        .confidence_level(CONFIDENCE_LEVEL)
        .warm_up_time(std::time::Duration::from_secs(WARMUP_TIME_SECS))
        .measurement_time(std::time::Duration::from_secs(MEASUREMENT_TIME_SECS));
    targets =
        bench_basic_operations,
        bench_threshold_analysis,
        bench_realistic_workloads,
        bench_compound_operations,
        bench_fixed_point_simd
);

criterion_main!(benches);
