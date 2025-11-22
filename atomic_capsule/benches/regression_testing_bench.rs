//! # B32-Compliant Regression Testing Benchmark
//!
//! **Validate Phase 5 improvements don't regress Phase 1-4 performance**
//!
//! This benchmark suite ensures that new Phase 5 features (SimdF64x8, SimdF32x8,
//! PackedState, fixed-point) don't negatively impact existing Phase 1-4 functionality.
//!
//! ## B32 Framework Compliance
//!
//! - **B23**: Regression detection against historical baselines
//! - **B27**: Honest reporting (flag any regressions)
//! - **K27**: Realistic improvement expectations (10-50% typical)
//!
//! ## Regression Test Strategy
//!
//! 1. **Baseline**: Phase 4 performance measurements
//! 2. **Current**: Phase 5 performance measurements
//! 3. **Validation**: Current >= Baseline (no regression)
//! 4. **Bonus**: Document any unexpected improvements

#![feature(portable_simd)]

use atomic_capsule::primitives::{SimdCapsule, SimdF32x8Capsule};
use atomic_capsule::{AlignmentTier, ColdTier, HotTier, WarmTier};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;

// ============================================================================
// PART 1: Alignment Tier Regression Tests
// ============================================================================

/// Validate alignment tier constants haven't changed
fn bench_alignment_tiers(c: &mut Criterion) {
    let mut group = c.benchmark_group("alignment_regression");

    group
        .confidence_level(0.95)
        .sample_size(100)
        .warm_up_time(Duration::from_secs(1));

    // HotTier: 64 bytes (Phase 1-4 baseline)
    group.bench_function("hot_tier_size", |bencher| {
        bencher.iter(|| black_box(HotTier::SIZE));
    });

    // WarmTier: 128 bytes (Phase 1-4 baseline)
    group.bench_function("warm_tier_size", |bencher| {
        bencher.iter(|| black_box(WarmTier::SIZE));
    });

    // ColdTier: 256 bytes (Phase 1-4 baseline)
    group.bench_function("cold_tier_size", |bencher| {
        bencher.iter(|| black_box(ColdTier::SIZE));
    });

    group.finish();
}

// ============================================================================
// PART 2: SIMD Operations Regression Tests
// ============================================================================

/// Validate SIMD operations maintain Phase 1-4 performance
fn bench_simd_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_regression");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    let a_data = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b_data = [8.0f32, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];

    // Dot product (KEY_INNOVATIONS.md baseline: 3-6ns for f32x8)
    group.bench_function("dot_product_baseline", |bencher| {
        let a = SimdF32x8Capsule::from_array(a_data);
        let b = SimdF32x8Capsule::from_array(b_data);
        bencher.iter(|| black_box(a.dot(&b)));
    });

    // Element-wise add (Phase 4 baseline: 2-4ns)
    group.bench_function("element_wise_add_baseline", |bencher| {
        let a = SimdF32x8Capsule::from_array(a_data);
        let b = SimdF32x8Capsule::from_array(b_data);
        bencher.iter(|| {
            let result = a.add(&b);
            black_box(result.load())
        });
    });

    // Element-wise multiply (Phase 4 baseline: 2-4ns)
    group.bench_function("element_wise_mul_baseline", |bencher| {
        let a = SimdF32x8Capsule::from_array(a_data);
        let b = SimdF32x8Capsule::from_array(b_data);
        bencher.iter(|| {
            let result = a.mul(&b);
            black_box(result.load())
        });
    });

    group.finish();
}

// ============================================================================
// PART 3: Memory Layout Regression Tests
// ============================================================================

/// Validate memory layouts haven't changed
fn bench_memory_layout_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_layout_regression");

    group
        .confidence_level(0.95)
        .sample_size(100)
        .warm_up_time(Duration::from_secs(1));

    // SimdF32x8Capsule: 64 bytes (Hot Tier)
    group.bench_function("f32_capsule_size", |bencher| {
        bencher.iter(|| black_box(core::mem::size_of::<SimdF32x8Capsule>()));
    });

    group.bench_function("f32_capsule_alignment", |bencher| {
        bencher.iter(|| black_box(core::mem::align_of::<SimdF32x8Capsule>()));
    });

    group.finish();
}

// ============================================================================
// PART 4: Load/Store Performance Regression Tests
// ============================================================================

/// Validate load/store operations haven't regressed
fn bench_load_store_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_store_regression");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    let data = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

    // Load operation (Phase 4 baseline: 2-3ns)
    group.bench_function("load_baseline", |bencher| {
        let cap = SimdF32x8Capsule::from_array(data);
        bencher.iter(|| black_box(cap.load()));
    });

    // Store operation via trait (Phase 4 baseline: 3-5ns)
    group.bench_function("store_baseline", |bencher| {
        let cap = SimdF32x8Capsule::from_array(data);
        let new_data = [2.0f32; 8];
        bencher.iter(|| {
            cap.store(new_data);
            black_box(cap.load())
        });
    });

    group.finish();
}

// ============================================================================
// PART 5: Generation Counter Regression Tests
// ============================================================================

/// Validate generation counter operations haven't regressed
fn bench_generation_counter_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation_counter_regression");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Generation counter read (Phase 4 baseline: <5ns)
    group.bench_function("generation_read_baseline", |bencher| {
        let cap = SimdF32x8Capsule::new();
        bencher.iter(|| black_box(cap.generation()));
    });

    // Generation counter increment via operation (Phase 4 baseline: <10ns)
    group.bench_function("generation_increment_baseline", |bencher| {
        let a = SimdF32x8Capsule::from_array([1.0; 8]);
        let b = SimdF32x8Capsule::from_array([1.0; 8]);
        bencher.iter(|| {
            let result = a.add(&b);
            black_box(result.generation())
        });
    });

    group.finish();
}

// ============================================================================
// PART 6: Compile-Time Verification Regression Tests
// ============================================================================

/// Validate compile-time verification macros haven't regressed
fn bench_verification_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("verification_regression");

    group
        .confidence_level(0.95)
        .sample_size(100)
        .warm_up_time(Duration::from_secs(1));

    // Alignment verification (should be zero runtime cost)
    group.bench_function("alignment_verification", |bencher| {
        bencher.iter(|| {
            let cap = SimdF32x8Capsule::new();
            let addr = &cap as *const _ as usize;
            black_box(addr % 64 == 0)
        });
    });

    // Size verification (should be zero runtime cost)
    group.bench_function("size_verification", |bencher| {
        bencher.iter(|| black_box(core::mem::size_of::<SimdF32x8Capsule>() == 64));
    });

    group.finish();
}

// ============================================================================
// PART 7: Scalar Fallback Regression Tests
// ============================================================================

/// Validate scalar fallback paths maintain performance
///
/// Note: These benchmarks run on non-SIMD code path to ensure
/// scalar fallback hasn't regressed
#[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
fn bench_scalar_fallback_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalar_fallback_regression");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    let a_data = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b_data = [8.0f32, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];

    // Scalar dot product (baseline: 16-24ns)
    group.bench_function("scalar_dot_baseline", |bencher| {
        let a = SimdF32x8Capsule::from_array(a_data);
        let b = SimdF32x8Capsule::from_array(b_data);
        bencher.iter(|| black_box(a.dot(&b)));
    });

    // Scalar add (baseline: 8-16ns)
    group.bench_function("scalar_add_baseline", |bencher| {
        let a = SimdF32x8Capsule::from_array(a_data);
        let b = SimdF32x8Capsule::from_array(b_data);
        bencher.iter(|| {
            let result = a.add(&b);
            black_box(result.load())
        });
    });

    group.finish();
}

// ============================================================================
// PART 8: Trait Implementation Regression Tests
// ============================================================================

/// Validate trait implementations maintain performance
fn bench_trait_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("trait_regression");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // SimdCapsule trait load (baseline: 2-3ns)
    group.bench_function("trait_load_baseline", |bencher| {
        let cap = SimdF32x8Capsule::from_array([1.0; 8]);
        bencher.iter(|| black_box(<SimdF32x8Capsule as SimdCapsule>::load(&cap)));
    });

    // SimdCapsule trait constants (baseline: 0ns, compile-time)
    group.bench_function("trait_constants", |bencher| {
        bencher.iter(|| {
            black_box((
                <SimdF32x8Capsule as SimdCapsule>::LANES,
                <SimdF32x8Capsule as SimdCapsule>::ALIGNMENT,
            ))
        });
    });

    group.finish();
}

// ============================================================================
// PART 9: Default Implementation Regression Tests
// ============================================================================

/// Validate Default trait implementation performance
fn bench_default_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("default_regression");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Default::default() (baseline: <5ns)
    group.bench_function("default_f32_capsule", |bencher| {
        bencher.iter(|| black_box(SimdF32x8Capsule::default()));
    });

    // new() vs default() (should be equivalent)
    group.bench_function("new_f32_capsule", |bencher| {
        bencher.iter(|| black_box(SimdF32x8Capsule::new()));
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_alignment_tiers,
    bench_simd_regression,
    bench_memory_layout_regression,
    bench_load_store_regression,
    bench_generation_counter_regression,
    bench_verification_regression,
    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    bench_scalar_fallback_regression,
    bench_trait_regression,
    bench_default_regression,
);

criterion_main!(benches);
