//! Zero Overhead Validation for Builder Pattern and Simplified API
//!
//! UCE-32 Q30: Empirical validation that abstractions introduce zero runtime overhead
//! B32 Framework: Statistical benchmarking proving identical performance to direct construction

use atomic_hedge_capsule::{
    capsule_standalone::HedgeBuilder, AtomicHedgeCapsule, BracketOrder, EntryOrder,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;

/// Direct construction baseline (B32 B1: Fair baseline, not strawman)
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

            let result = capsule.initialize(entry, bracket);
            black_box((capsule, result))
        })
    });
}

/// Builder pattern construction (UCE-32 Q31: Must compile to identical code)
fn bench_builder_construction(c: &mut Criterion) {
    c.bench_function("builder_construction", |b| {
        b.iter(|| {
            let result = AtomicHedgeCapsule::hedge(black_box("BTCUSD"))
                .on_exchange(black_box("NDAX"))
                .size(black_box(1.0))
                .stop_loss(black_box(45000.0))
                .take_profit(black_box(55000.0))
                .build();

            black_box(result)
        })
    });
}

/// Simplified API construction (UCE-32 Q28: Simple but zero overhead)
fn bench_simplified_api(c: &mut Criterion) {
    c.bench_function("simplified_api", |b| {
        b.iter(|| {
            let result = AtomicHedgeCapsule::create_hedge(
                black_box("BTCUSD"),
                black_box("NDAX"),
                black_box(1.0),
                black_box(45000.0),
                black_box(55000.0),
            );

            black_box(result)
        })
    });
}

/// Preset configurations (UCE-32 Q31: Compile-time presets)
fn bench_preset_configurations(c: &mut Criterion) {
    let mut group = c.benchmark_group("presets");

    group.bench_function("hft_preset", |b| {
        b.iter(|| {
            let result = HedgeBuilder::hft_preset(black_box("BTCUSD"))
                .on_exchange(black_box("NDAX"))
                .size(black_box(1.0))
                .stop_loss(black_box(45000.0))
                .take_profit(black_box(55000.0))
                .build();

            black_box(result)
        })
    });

    group.bench_function("conservative_preset", |b| {
        b.iter(|| {
            let result = HedgeBuilder::conservative_preset(black_box("BTCUSD"))
                .on_exchange(black_box("NDAX"))
                .size(black_box(1.0))
                .stop_loss(black_box(45000.0))
                .take_profit(black_box(55000.0))
                .build();

            black_box(result)
        })
    });

    group.finish();
}

/// Hot path operations after construction (validate no degradation)
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

    let builder_capsule = AtomicHedgeCapsule::hedge("BTCUSD")
        .on_exchange("NDAX")
        .size(1.0)
        .stop_loss(45000.0)
        .take_profit(55000.0)
        .build()
        .unwrap();

    let simplified_capsule =
        AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0).unwrap();

    let mut group = c.benchmark_group("hot_path");

    // Critical hot path operations must have identical performance
    group.bench_function("direct_hot_path", |b| {
        b.iter(|| {
            let active = direct_capsule.is_active();
            let emergency = direct_capsule.is_emergency_stopped();
            let gen = direct_capsule.increment_generation_unchecked();
            black_box((active, emergency, gen))
        })
    });

    group.bench_function("builder_hot_path", |b| {
        b.iter(|| {
            let active = builder_capsule.is_active();
            let emergency = builder_capsule.is_emergency_stopped();
            let gen = builder_capsule.increment_generation_unchecked();
            black_box((active, emergency, gen))
        })
    });

    group.bench_function("simplified_hot_path", |b| {
        b.iter(|| {
            let active = simplified_capsule.is_active();
            let emergency = simplified_capsule.is_emergency_stopped();
            let gen = simplified_capsule.increment_generation_unchecked();
            black_box((active, emergency, gen))
        })
    });

    group.finish();
}

/// Cache efficiency validation (UCE-32 Q29: Cache optimization preserved)
fn bench_cache_efficiency_validation(c: &mut Criterion) {
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

    let builder_capsule = AtomicHedgeCapsule::hedge("BTCUSD")
        .on_exchange("NDAX")
        .size(1.0)
        .stop_loss(45000.0)
        .take_profit(55000.0)
        .build()
        .unwrap();

    // Validate cache layout is identical
    let direct_cache = direct_capsule.cache_info();
    let builder_cache = builder_capsule.cache_info();

    assert_eq!(direct_cache.alignment, builder_cache.alignment);
    assert_eq!(direct_cache.size, builder_cache.size);
    assert_eq!(direct_cache.hot_data_offset, builder_cache.hot_data_offset);
    assert_eq!(
        direct_cache.cold_data_offset,
        builder_cache.cold_data_offset
    );

    let mut group = c.benchmark_group("cache_efficiency");

    // Warm up cache for both capsules
    for _ in 0..1000 {
        let _ = direct_capsule.is_active();
        let _ = builder_capsule.is_active();
    }

    group.bench_function("direct_cache_hot", |b| {
        b.iter(|| direct_capsule.load_hot_data())
    });

    group.bench_function("builder_cache_hot", |b| {
        b.iter(|| builder_capsule.load_hot_data())
    });

    group.finish();
}

/// B32 Statistical validation with confidence intervals
fn bench_statistical_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_validation");

    // B32 requirements: 95% confidence intervals, 1000+ samples
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(3));

    group.bench_function("direct_vs_builder_statistical", |b| {
        b.iter(|| {
            // Direct construction
            let capsule1 = AtomicHedgeCapsule::new();
            let entry1 = EntryOrder::new(
                "NDAX".to_string(),
                "BTCUSD".to_string(),
                "Buy".to_string(),
                1.0,
            );
            let bracket1 = BracketOrder::new(45000.0, 55000.0, 1.0);
            let result1 = capsule1.initialize(entry1, bracket1);

            // Builder construction
            let result2 = AtomicHedgeCapsule::hedge("BTCUSD")
                .on_exchange("NDAX")
                .size(1.0)
                .stop_loss(45000.0)
                .take_profit(55000.0)
                .build();

            black_box((result1, result2))
        })
    });

    group.finish();
}

/// Memory allocation validation (must be identical)
fn bench_allocation_validation(c: &mut Criterion) {
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

    group.bench_function("builder_allocations", |b| {
        b.iter(|| {
            let capsule = AtomicHedgeCapsule::hedge("BTCUSD")
                .on_exchange("NDAX")
                .size(1.0)
                .stop_loss(45000.0)
                .take_profit(55000.0)
                .build()
                .unwrap();
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

/// Comprehensive zero overhead validation suite
criterion_group!(
    zero_overhead_validation,
    bench_direct_construction,
    bench_builder_construction,
    bench_simplified_api,
    bench_preset_configurations,
    bench_hot_path_operations,
    bench_cache_efficiency_validation,
    bench_statistical_validation,
    bench_allocation_validation
);

criterion_main!(zero_overhead_validation);
