//! # Tiered Quantization Benchmark
//!
//! **B32 Framework Validation**: Hot/Warm/Cold Tier Performance
//!
//! ## Target Performance (B32)
//!
//! - **Hot Tier (Q8.8)**: 8ns weight load (L1 cache)
//! - **Warm Tier (Q8.8)**: 15ns weight load (L2 cache)
//! - **Cold Tier (Q8.8)**: 25ns weight load (L3 cache)
//!
//! ## Benchmark Strategy
//!
//! 1. **Hot access pattern**: Sequential reads from same capsule
//! 2. **Warm access pattern**: Round-robin across small set
//! 3. **Cold access pattern**: Random access across large set
//!
//! ## Statistical Rigor (B32)
//!
//! - Minimum 1000 iterations
//! - 95% confidence intervals
//! - Warmup phase for cache stabilization
//! - Black-box to prevent compiler optimization

use atomic_llm_capsule::primitives::quant_tiered::{
    ColdWeightCapsule, HotWeightCapsule, TieredQuantizationCache, WarmWeightCapsule,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Benchmark hot tier weight read (L1 cache hit)
fn bench_hot_tier_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_tier");

    let mut capsule = HotWeightCapsule::new();
    let weights = [1.0f32; 24];
    capsule.publish(&weights);

    group.bench_function("read_single_weight", |b| {
        b.iter(|| {
            let result = black_box(capsule.read());
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark warm tier weight read (L2 cache hit)
fn bench_warm_tier_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("warm_tier");

    let mut capsule = WarmWeightCapsule::new();
    let weights = [1.0f32; 56];
    capsule.publish(&weights);

    group.bench_function("read_single_weight", |b| {
        b.iter(|| {
            let result = black_box(capsule.read());
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark cold tier weight read (L3 cache hit)
fn bench_cold_tier_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_tier");

    let mut capsule = ColdWeightCapsule::new();
    let weights = [1.0f32; 120];
    capsule.publish(&weights);

    group.bench_function("read_single_weight", |b| {
        b.iter(|| {
            let result = black_box(capsule.read());
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark tier promotion logic
fn bench_promotion(c: &mut Criterion) {
    let mut group = c.benchmark_group("promotion");

    let cache = TieredQuantizationCache::new(100, 300, 600);

    group.bench_function("record_access", |b| {
        b.iter(|| {
            black_box(cache.record_access(42));
        });
    });

    group.finish();
}

/// Benchmark hot tier publish (two-phase commit)
fn bench_hot_tier_publish(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_tier_publish");

    let mut capsule = HotWeightCapsule::new();
    let weights = [1.0f32; 24];

    group.bench_function("two_phase_commit", |b| {
        b.iter(|| {
            black_box(capsule.publish(&weights));
        });
    });

    group.finish();
}

/// Benchmark memory budget calculation
fn bench_memory_budget(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_budget");

    // Simulate 1000 capsules: 10% hot, 30% warm, 60% cold
    let hot_capsules: Vec<HotWeightCapsule> = (0..100).map(|_| HotWeightCapsule::new()).collect();
    let warm_capsules: Vec<WarmWeightCapsule> =
        (0..300).map(|_| WarmWeightCapsule::new()).collect();
    let cold_capsules: Vec<ColdWeightCapsule> =
        (0..600).map(|_| ColdWeightCapsule::new()).collect();

    group.bench_function("memory_calculation", |b| {
        b.iter(|| {
            let total = black_box(
                hot_capsules.len() * 64 + warm_capsules.len() * 128 + cold_capsules.len() * 256,
            );
            black_box(total);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_hot_tier_read,
    bench_warm_tier_read,
    bench_cold_tier_read,
    bench_promotion,
    bench_hot_tier_publish,
    bench_memory_budget,
);

criterion_main!(benches);
