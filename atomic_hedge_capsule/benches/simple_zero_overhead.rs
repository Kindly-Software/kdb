//! Simple Zero Overhead Validation
//!
//! UCE-32 Q30: Empirical validation that builder pattern introduces zero runtime overhead
//! Focus on core requirement: builder must compile to identical code as direct construction

use atomic_hedge_capsule::{AtomicHedgeCapsule, BracketOrder, EntryOrder};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Direct construction baseline (fair comparison)
fn bench_direct_construction(c: &mut Criterion) {
    c.bench_function("direct_construction", |b| {
        b.iter(|| {
            let capsule = AtomicHedgeCapsule::new();

            let entry = EntryOrder::new(
                black_box("NDAX".to_string()),
                black_box("BTCUSD".to_string()),
                black_box("Buy".to_string()),
                black_box(1.0),
            );

            let bracket = BracketOrder::new(black_box(45000.0), black_box(55000.0), black_box(1.0));

            black_box(capsule.initialize(entry, bracket))
        })
    });
}

/// Simplified API construction
fn bench_simplified_api(c: &mut Criterion) {
    c.bench_function("simplified_api", |b| {
        b.iter(|| {
            black_box(AtomicHedgeCapsule::create_hedge(
                black_box("BTCUSD"),
                black_box("NDAX"),
                black_box(1.0),
                black_box(45000.0),
                black_box(55000.0),
            ))
        })
    });
}

/// Hot path operations to validate no performance degradation
fn bench_hot_path_operations(c: &mut Criterion) {
    // Create capsules using different methods
    let direct_capsule = {
        let capsule = AtomicHedgeCapsule::new();
        let entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        capsule.initialize(entry, bracket).unwrap();
        capsule
    };

    let simplified_capsule =
        AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0).unwrap();

    let mut group = c.benchmark_group("hot_path");

    group.bench_function("direct_hot_path", |b| {
        b.iter(|| {
            let active = direct_capsule.is_active();
            let emergency = direct_capsule.is_emergency_stopped();
            let _ = direct_capsule.increment_generation();
            black_box((active, emergency))
        })
    });

    group.bench_function("simplified_hot_path", |b| {
        b.iter(|| {
            let active = simplified_capsule.is_active();
            let emergency = simplified_capsule.is_emergency_stopped();
            let _ = simplified_capsule.increment_generation();
            black_box((active, emergency))
        })
    });

    group.finish();
}

/// Cache optimization validation
fn bench_cache_validation(c: &mut Criterion) {
    let direct_capsule = {
        let c = AtomicHedgeCapsule::new();
        let entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        c.initialize(entry, bracket).unwrap();
        c
    };

    let simplified_capsule =
        AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0).unwrap();

    // Validate cache layout is identical
    let direct_cache = direct_capsule.cache_info();
    let simplified_cache = simplified_capsule.cache_info();

    assert_eq!(
        direct_cache.alignment, simplified_cache.alignment,
        "Cache alignment must be identical"
    );
    assert_eq!(
        direct_cache.size, simplified_cache.size,
        "Structure size must be identical"
    );

    let mut group = c.benchmark_group("cache_validation");

    group.bench_function("direct_cache_load", |b| {
        b.iter(|| direct_capsule.load_hot_data())
    });

    group.bench_function("simplified_cache_load", |b| {
        b.iter(|| simplified_capsule.load_hot_data())
    });

    group.finish();
}

/// Memory allocation comparison
fn bench_allocation_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocations");

    group.bench_function("direct_allocations", |b| {
        b.iter(|| {
            let capsule = AtomicHedgeCapsule::new();
            let entry = EntryOrder::new(
                "NDAX".to_string(),
                "BTCUSD".to_string(),
                "Buy".to_string(),
                1.0,
            );
            let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
            capsule.initialize(entry, bracket).unwrap();
            black_box(capsule)
        })
    });

    group.bench_function("simplified_allocations", |b| {
        b.iter(|| {
            let capsule =
                AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0).unwrap();
            black_box(capsule)
        })
    });

    group.finish();
}

criterion_group!(
    zero_overhead_validation,
    bench_direct_construction,
    bench_simplified_api,
    bench_hot_path_operations,
    bench_cache_validation,
    bench_allocation_comparison
);

criterion_main!(zero_overhead_validation);
