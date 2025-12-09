//! UCE-32 Q32 Nightly Features Benchmark
//!
//! Empirical validation of nightly feature performance improvements

use atomic_hedge_capsule::capsule_standalone::{AtomicHedgeCapsule, SimdValidator};
use atomic_hedge_capsule::types::{BracketOrder, EntryOrder, OrderState};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(all(feature = "nightly", feature = "portable_simd"))]
use std::simd::prelude::*;

/// UCE-32 Q30: Baseline benchmark for comparison
fn bench_baseline_operations(c: &mut Criterion) {
    let capsule = AtomicHedgeCapsule::new();

    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    c.bench_function("baseline_state_update", |b| {
        b.iter(|| {
            let _ = capsule.update_entry_state(OrderState::Validated, black_box(0.5));
        })
    });

    c.bench_function("baseline_generation_increment", |b| {
        b.iter(|| {
            let _ = capsule.increment_generation();
        })
    });

    c.bench_function("baseline_hedge_state_check", |b| {
        b.iter(|| {
            let _ = capsule.get_hedge_state();
        })
    });
}

/// UCE-32 Q32: SIMD acceleration benchmarks
#[cfg(all(feature = "nightly", feature = "portable_simd"))]
fn bench_simd_operations(c: &mut Criterion) {
    let validator = SimdValidator::new();
    let test_values = [1000, 5000, 25000, 100000];

    c.bench_function("simd_batch_validation", |b| {
        b.iter(|| {
            let _ = validator.validate_batch(black_box(test_values));
        })
    });

    c.bench_function("simd_batch_processing", |b| {
        b.iter(|| {
            let _ = validator.process_batch(black_box(test_values));
        })
    });

    c.bench_function("simd_hedge_states", |b| {
        b.iter(|| {
            let _ = validator.process_hedge_states(black_box(test_values));
        })
    });

    // Compare SIMD vs scalar operations
    let mut group = c.benchmark_group("simd_vs_scalar");

    group.bench_function("simd_4x_multiply", |b| {
        b.iter(|| {
            let input = u64x4::from_array(black_box(test_values));
            let multipliers = u64x4::from_array([1, 2, 4, 8]);
            let _ = (input * multipliers).to_array();
        })
    });

    group.bench_function("scalar_4x_multiply", |b| {
        b.iter(|| {
            let values = black_box(test_values);
            let multipliers = [1, 2, 4, 8];
            let _: Vec<u64> = values
                .iter()
                .zip(multipliers.iter())
                .map(|(v, m)| v * m)
                .collect();
        })
    });

    group.finish();
}

/// UCE-32 Q32: Const fn floating-point arithmetic benchmarks
fn bench_const_fn_operations(c: &mut Criterion) {
    use atomic_hedge_capsule::capsule_standalone::{EMERGENCY_HEDGE_NS, HEDGE_TIMEOUT_MS};

    c.bench_function("const_emergency_threshold", |b| {
        b.iter(|| {
            // This should be compile-time optimized away
            let _ = black_box(EMERGENCY_HEDGE_NS);
        })
    });

    c.bench_function("const_golden_timeout", |b| {
        b.iter(|| {
            // This should be compile-time optimized away
            let _ = black_box(HEDGE_TIMEOUT_MS);
        })
    });

    // Compare with runtime calculation
    c.bench_function("runtime_golden_calculation", |b| {
        b.iter(|| {
            const PHI: f64 = 1.6180339887498948;
            let _ = black_box((100.0 * PHI) as u64);
        })
    });
}

/// UCE-32 Q32: Branch prediction optimization benchmarks
fn bench_branch_prediction(c: &mut Criterion) {
    let capsule = AtomicHedgeCapsule::new();

    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    // Benchmark hot path with likely/unlikely hints
    c.bench_function("optimized_update_with_hints", |b| {
        b.iter(|| {
            // Normal operation - should hit the likely path
            let _ = capsule.update_entry_state(OrderState::Validated, black_box(0.5));
        })
    });

    // Benchmark emergency path (unlikely path)
    let emergency_capsule = AtomicHedgeCapsule::new();
    emergency_capsule
        .initialize(entry.clone(), bracket.clone())
        .unwrap();
    emergency_capsule.emergency_stop("Test").unwrap();

    c.bench_function("emergency_path", |b| {
        b.iter(|| {
            // This should hit the unlikely path
            let _ = emergency_capsule.update_entry_state(OrderState::Validated, black_box(0.5));
        })
    });
}

/// UCE-32 Q32: Cache optimization with nightly features
fn bench_cache_optimized_operations(c: &mut Criterion) {
    let capsule = AtomicHedgeCapsule::new();

    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    let mut group = c.benchmark_group("cache_optimized");

    for threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("hot_path_access", threads),
            threads,
            |b, &threads| {
                b.iter(|| {
                    // Simulate concurrent access to hot data
                    for _ in 0..threads {
                        let _ = capsule.is_active();
                        let _ = capsule.is_emergency_stopped();
                        let _ = capsule.increment_generation();
                    }
                })
            },
        );
    }

    group.finish();
}

/// UCE-32 Q30: Comprehensive performance regression test
fn bench_performance_regression(c: &mut Criterion) {
    let capsule = AtomicHedgeCapsule::new();

    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    // UCE-32 Q30: Target performance thresholds
    // Based on real-world constraints and UCE-32 analysis
    c.bench_function("regression_hot_path_composite", |b| {
        b.iter(|| {
            // Composite operation representing typical usage
            let _active = capsule.is_active();
            let _gen = capsule.increment_generation();
            let _emergency = capsule.is_emergency_stopped();
            let _state = capsule.get_hedge_state();
            let _ = capsule.update_entry_state(OrderState::Validated, black_box(0.1));
        })
    });
}

/// UCE-32 Q32: Measure nightly feature overhead
fn bench_feature_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_overhead");

    // Measure the cost of feature detection
    group.bench_function("feature_check_overhead", |b| {
        b.iter(|| {
            #[cfg(all(feature = "nightly", feature = "portable_simd"))]
            let _simd_available = true;
            #[cfg(not(all(feature = "nightly", feature = "portable_simd")))]
            let _simd_available = false;

            #[cfg(all(feature = "nightly", feature = "const_fn_floating_point_arithmetic"))]
            let _const_fn_available = true;
            #[cfg(not(all(feature = "nightly", feature = "const_fn_floating_point_arithmetic")))]
            let _const_fn_available = false;

            black_box((_simd_available, _const_fn_available));
        })
    });

    group.finish();
}

/// UCE-32 Q30: Overall nightly features impact assessment
fn bench_nightly_impact_assessment(c: &mut Criterion) {
    let capsule = AtomicHedgeCapsule::new();

    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    // Benchmark typical hedge operation workflow
    c.bench_function("nightly_optimized_workflow", |b| {
        b.iter(|| {
            // Typical hedge operation sequence with all nightly optimizations
            let _check1 = capsule.is_active(); // Branch prediction optimized
            let _gen = capsule.increment_generation(); // Overflow check optimized

            #[cfg(all(feature = "nightly", feature = "portable_simd"))]
            {
                let validator = SimdValidator::new();
                let _validation = validator.validate_batch([1000, 2000, 3000, 4000]);
            }

            let _update = capsule.update_entry_state(OrderState::Validated, black_box(0.5));
            let _state = capsule.get_hedge_state();
        })
    });
}

criterion_group!(
    benches,
    bench_baseline_operations,
    #[cfg(all(feature = "nightly", feature = "portable_simd"))]
    bench_simd_operations,
    bench_const_fn_operations,
    bench_branch_prediction,
    bench_cache_optimized_operations,
    bench_performance_regression,
    bench_feature_overhead,
    bench_nightly_impact_assessment
);

criterion_main!(benches);
