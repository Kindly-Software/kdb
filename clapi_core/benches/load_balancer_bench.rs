//! Load Balancer Benchmarks - B32 Framework Compliance
//!
//! # B32 Benchmarking Standards
//! - Fair baseline: Scalar scoring (not strawman comparison)
//! - Statistical rigor: 1000+ iterations, 95% CI via Criterion
//! - Honest reporting: Document where SIMD helps AND overhead
//! - Reproducibility: All benchmarks committed to repo
//!
//! # Expected Results (Hardware Reality Checks)
//! - SIMD vs Scalar: 2-4× speedup (f32x8 parallel computation)
//! - Provider selection: <500ns (scoring + filtering + selection)
//! - Round-robin baseline: ~50ns (simple counter, unfair comparison)
//!
//! # Reality Check (B32 § K27)
//! - 10-50% typical improvement (most optimizations)
//! - 2-10× exceptional (SIMD, proven patterns)
//! - 100×+ rare (requires extensive validation)
//! - Our claim: 4× SIMD speedup (exceptional but achievable with f32x8)

use clapi_core::capsules::ProviderCircuitArray;
use clapi_core::load_balancer::{create_default_balancer, LoadBalancer, ScoringWeights};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

// ============================================================================
// Benchmark 1: SIMD vs Scalar Scoring
// ============================================================================

fn bench_simd_vs_scalar(c: &mut Criterion) {
    let mut group = c.benchmark_group("scoring_comparison");

    let circuits = Arc::new(ProviderCircuitArray::new());
    let balancer = LoadBalancer::new(circuits, ScoringWeights::default());

    // Benchmark scalar scoring (baseline)
    group.bench_function("scalar_score", |b| {
        b.iter(|| black_box(balancer.scalar_score()))
    });

    // Benchmark SIMD scoring (optimized)
    #[cfg(feature = "portable_simd")]
    group.bench_function("simd_score", |b| {
        b.iter(|| black_box(balancer.simd_score()))
    });

    group.finish();
}

// ============================================================================
// Benchmark 2: Provider Selection (End-to-End)
// ============================================================================

fn bench_provider_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_selection");

    let circuits = Arc::new(ProviderCircuitArray::new());

    // Latency-optimized balancer
    let latency_balancer = LoadBalancer::new(
        circuits.clone(),
        ScoringWeights {
            latency_weight: 0.7,
            cost_weight: 0.3,
        },
    );

    // Cost-optimized balancer
    let cost_balancer = LoadBalancer::new(
        circuits,
        ScoringWeights {
            latency_weight: 0.3,
            cost_weight: 0.7,
        },
    );

    group.bench_function("latency_optimized", |b| {
        b.iter(|| black_box(latency_balancer.select_provider()))
    });

    group.bench_function("cost_optimized", |b| {
        b.iter(|| black_box(cost_balancer.select_provider()))
    });

    group.finish();
}

// ============================================================================
// Benchmark 3: Load Balancer vs Round-Robin (Baseline Comparison)
// ============================================================================

// Simple round-robin implementation (unfair baseline, but educational)
struct RoundRobinBalancer {
    counter: AtomicU8,
}

impl RoundRobinBalancer {
    fn new() -> Self {
        Self {
            counter: AtomicU8::new(0),
        }
    }

    fn select_provider(&self) -> u8 {
        let current = self.counter.fetch_add(1, Ordering::Relaxed);
        current % 8
    }
}

fn bench_balancer_vs_round_robin(c: &mut Criterion) {
    let mut group = c.benchmark_group("balancer_comparison");

    let circuits = Arc::new(ProviderCircuitArray::new());
    let smart_balancer = create_default_balancer(circuits);
    let round_robin = RoundRobinBalancer::new();

    // Smart balancer (multi-factor scoring)
    group.bench_function("smart_balancer", |b| {
        b.iter(|| black_box(smart_balancer.select_provider()))
    });

    // Round-robin (simple counter, unfair comparison)
    group.bench_function("round_robin", |b| {
        b.iter(|| black_box(round_robin.select_provider()))
    });

    group.finish();
}

// ============================================================================
// Benchmark 4: Scaling with Provider Count
// ============================================================================

fn bench_scaling_provider_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_provider_count");

    let circuits = Arc::new(ProviderCircuitArray::new());
    let balancer = create_default_balancer(circuits.clone());

    // Benchmark with different numbers of active providers
    for num_providers in [1, 2, 4, 8] {
        // Open circuits for providers beyond num_providers
        for i in num_providers..8 {
            balancer.update_latency(i as u8, f32::MAX); // Make them unattractive
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(num_providers),
            &num_providers,
            |b, _| b.iter(|| black_box(balancer.select_provider())),
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark 5: Concurrent Selection Performance
// ============================================================================

fn bench_concurrent_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_selection");

    let circuits = Arc::new(ProviderCircuitArray::new());
    let balancer = Arc::new(create_default_balancer(circuits));

    // Single-threaded baseline
    group.bench_function("single_thread", |b| {
        b.iter(|| black_box(balancer.select_provider()))
    });

    // Multi-threaded (2 threads)
    group.bench_function("two_threads", |b| {
        let bal1 = Arc::clone(&balancer);
        let bal2 = Arc::clone(&balancer);

        b.iter(|| {
            use std::thread;

            let h1 = thread::spawn(move || bal1.select_provider());
            let h2 = thread::spawn(move || bal2.select_provider());

            let _ = black_box(h1.join());
            let _ = black_box(h2.join());
        })
    });

    group.finish();
}

// ============================================================================
// Benchmark 6: Update Operations
// ============================================================================

fn bench_update_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("update_operations");

    let circuits = Arc::new(ProviderCircuitArray::new());
    let balancer = create_default_balancer(circuits);

    group.bench_function("update_latency", |b| {
        b.iter(|| balancer.update_latency(black_box(0), black_box(100.0)))
    });

    group.bench_function("update_cost", |b| {
        b.iter(|| balancer.update_cost(black_box(0), black_box(50.0)))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_simd_vs_scalar,
    bench_provider_selection,
    bench_balancer_vs_round_robin,
    bench_scaling_provider_count,
    bench_concurrent_selection,
    bench_update_operations,
);

criterion_main!(benches);
