//! Zero Overhead Builder Pattern Validation
//!
//! UCE-32 Q30: Empirical validation that builder pattern introduces zero runtime overhead
//! B32 Framework: Statistical benchmarking with 95% confidence intervals
//! Kontext27: Hardware reality checks for Intel Ultra 7 155H

use atomic_hedge_capsule::{
    capsule_standalone::HedgeBuilder, AtomicHedgeCapsule, BracketOrder, EntryOrder,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Direct construction benchmark (baseline)
///
/// UCE-32 Q30: Establish baseline performance using direct construction
/// B32 B1: Fair baseline - optimized direct construction, not strawman
fn bench_direct_construction(c: &mut Criterion) {
    c.bench_function("direct_construction_baseline", |b| {
        b.iter(|| {
            // Direct construction - no builder pattern
            let capsule = AtomicHedgeCapsule::new();

            let entry = EntryOrder::new(
                black_box("NDAX".to_string()),
                black_box("BTCUSD".to_string()),
                black_box("Buy".to_string()),
                black_box(1.0),
            );

            let bracket = BracketOrder::new(black_box(45000.0), black_box(55000.0), black_box(1.0));

            let result = capsule.initialize(entry, bracket);
            black_box(result)
        })
    });
}

/// Builder pattern benchmark
///
/// UCE-32 Q30: Measure builder pattern performance vs direct construction
/// UCE-32 Q31: Builder should compile away completely - zero overhead
fn bench_builder_construction(c: &mut Criterion) {
    c.bench_function("builder_construction", |b| {
        b.iter(|| {
            // Builder pattern construction
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

/// Simplified API benchmark
///
/// UCE-32 Q28: Validate simplified API overhead vs direct construction
fn bench_simplified_api(c: &mut Criterion) {
    c.bench_function("simplified_api_construction", |b| {
        b.iter(|| {
            // Simplified API construction
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

/// Preset configuration benchmark
///
/// UCE-32 Q31: Validate preset configurations compile away
fn bench_preset_configurations(c: &mut Criterion) {
    let mut group = c.benchmark_group("preset_configurations");

    // HFT preset
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

    // Conservative preset
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

    // Market order preset
    group.bench_function("market_order_preset", |b| {
        b.iter(|| {
            let result = HedgeBuilder::market_order(black_box("BTCUSD"))
                .on_exchange(black_box("NDAX"))
                .size(black_box(1.0))
                .stop_loss(black_box(45000.0))
                .take_profit(black_box(55000.0))
                .build();

            black_box(result)
        })
    });

    // Limit order preset
    group.bench_function("limit_order_preset", |b| {
        b.iter(|| {
            let result = HedgeBuilder::limit_order(black_box("BTCUSD"), black_box(50000.0))
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

/// Builder method chaining benchmark
///
/// UCE-32 Q31: Validate that method chaining compiles to direct assignment
fn bench_method_chaining_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("method_chaining");

    // Short chain (3 methods)
    group.bench_function("short_chain", |b| {
        b.iter(|| {
            let result = HedgeBuilder::new(black_box("BTCUSD"))
                .size(black_box(1.0))
                .stop_loss(black_box(45000.0))
                .build();

            black_box(result)
        })
    });

    // Medium chain (5 methods)
    group.bench_function("medium_chain", |b| {
        b.iter(|| {
            let result = HedgeBuilder::new(black_box("BTCUSD"))
                .on_exchange(black_box("NDAX"))
                .size(black_box(1.0))
                .stop_loss(black_box(45000.0))
                .take_profit(black_box(55000.0))
                .build();

            black_box(result)
        })
    });

    // Long chain (7 methods)
    group.bench_function("long_chain", |b| {
        b.iter(|| {
            let result = HedgeBuilder::new(black_box("BTCUSD"))
                .on_exchange(black_box("NDAX"))
                .size(black_box(1.0))
                .order_type(black_box("LIMIT"))
                .limit_price(black_box(50000.0))
                .stop_loss(black_box(45000.0))
                .take_profit(black_box(55000.0))
                .build();

            black_box(result)
        })
    });

    group.finish();
}

/// Memory allocation benchmark
///
/// UCE-32 Q29: Validate no hidden allocations in builder pattern
/// Kontext27 K13: Allocation costs - should be identical for all approaches
fn bench_memory_allocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocations");

    // Direct construction allocations
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

    // Builder allocations
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

    // Simplified API allocations
    group.bench_function("simplified_allocations", |b| {
        b.iter(|| {
            let capsule =
                AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0).unwrap();

            black_box(capsule)
        })
    });

    group.finish();
}

/// Hot path operations benchmark after construction
///
/// UCE-32 Q30: Validate no performance degradation in constructed capsules
fn bench_hot_path_operations(c: &mut Criterion) {
    // Create capsules using different construction methods
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

    let mut group = c.benchmark_group("hot_path_operations");

    // Test hot path operations on capsules created via different methods
    group.bench_function("direct_hot_path", |b| {
        b.iter(|| {
            let _ = direct_capsule.is_active();
            let _ = direct_capsule.increment_generation();
            let _ = direct_capsule.is_emergency_stopped();
            let _ = direct_capsule.get_hedge_state();
        })
    });

    group.bench_function("builder_hot_path", |b| {
        b.iter(|| {
            let _ = builder_capsule.is_active();
            let _ = builder_capsule.increment_generation();
            let _ = builder_capsule.is_emergency_stopped();
            let _ = builder_capsule.get_hedge_state();
        })
    });

    group.bench_function("simplified_hot_path", |b| {
        b.iter(|| {
            let _ = simplified_capsule.is_active();
            let _ = simplified_capsule.increment_generation();
            let _ = simplified_capsule.is_emergency_stopped();
            let _ = simplified_capsule.get_hedge_state();
        })
    });

    group.finish();
}

/// Cache efficiency benchmark
///
/// UCE-32 Q29: Validate cache optimization is preserved across construction methods
fn bench_cache_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_efficiency");

    // Benchmark cache-hot operations
    group.bench_function("direct_cache_hot", |b| {
        let capsule = {
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

        // Warm up cache
        for _ in 0..1000 {
            let _ = capsule.is_active();
        }

        b.iter(|| {
            // Cache-hot access pattern
            let active = capsule.is_active();
            let emergency = capsule.is_emergency_stopped();
            let gen = capsule.increment_generation_unchecked();
            black_box((active, emergency, gen))
        })
    });

    group.bench_function("builder_cache_hot", |b| {
        let capsule = AtomicHedgeCapsule::hedge("BTCUSD")
            .on_exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build()
            .unwrap();

        // Warm up cache
        for _ in 0..1000 {
            let _ = capsule.is_active();
        }

        b.iter(|| {
            // Cache-hot access pattern
            let active = capsule.is_active();
            let emergency = capsule.is_emergency_stopped();
            let gen = capsule.increment_generation_unchecked();
            black_box((active, emergency, gen))
        })
    });

    group.finish();
}

/// Compiler optimization validation
///
/// UCE-32 Q32: Validate nightly features work correctly with builder pattern
fn bench_compiler_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler_optimizations");

    // Test const fn optimizations with builder
    group.bench_function("const_fn_with_builder", |b| {
        b.iter(|| {
            use atomic_hedge_capsule::capsule_standalone::EMERGENCY_HEDGE_NS;

            let capsule = HedgeBuilder::hft_preset(black_box("BTCUSD"))
                .emergency_threshold(black_box(EMERGENCY_HEDGE_NS))
                .on_exchange(black_box("NDAX"))
                .size(black_box(1.0))
                .stop_loss(black_box(45000.0))
                .take_profit(black_box(55000.0))
                .build();

            black_box(capsule)
        })
    });

    // Test inline optimizations
    group.bench_function("inline_optimizations", |b| {
        b.iter(|| {
            // All these methods are #[inline(always)]
            let builder = HedgeBuilder::new(black_box("BTCUSD"))
                .on_exchange(black_box("NDAX"))
                .size(black_box(1.0))
                .stop_loss(black_box(45000.0))
                .take_profit(black_box(55000.0));

            let result = builder.build();
            black_box(result)
        })
    });

    group.finish();
}

/// Statistical validation of zero overhead claim
///
/// B32 Framework: Multiple runs with statistical analysis
fn bench_statistical_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistical_validation");

    // Configure for statistical rigor per B32 requirements
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(std::time::Duration::from_secs(3));

    // Direct vs Builder comparison with statistical validation
    for size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("direct_vs_builder", size),
            size,
            |b, &size| {
                b.iter(|| {
                    for i in 0..size {
                        let symbol = format!("BTC{}", i);

                        // Direct construction
                        let direct_start = std::time::Instant::now();
                        let capsule1 = AtomicHedgeCapsule::new();
                        let entry1 = EntryOrder::new(
                            "NDAX".to_string(),
                            symbol.clone(),
                            "Buy".to_string(),
                            1.0,
                        );
                        let bracket1 = BracketOrder::new(45000.0, 55000.0, 1.0);
                        let _ = capsule1.initialize(entry1, bracket1);
                        let direct_time = direct_start.elapsed();

                        // Builder construction
                        let builder_start = std::time::Instant::now();
                        let _ = AtomicHedgeCapsule::hedge(&symbol)
                            .on_exchange("NDAX")
                            .size(1.0)
                            .stop_loss(45000.0)
                            .take_profit(55000.0)
                            .build();
                        let builder_time = builder_start.elapsed();

                        let _ = black_box((direct_time, builder_time));
                    }
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    zero_overhead_validation,
    bench_direct_construction,
    bench_builder_construction,
    bench_simplified_api,
    bench_preset_configurations,
    bench_method_chaining_overhead,
    bench_memory_allocations,
    bench_hot_path_operations,
    bench_cache_efficiency,
    bench_compiler_optimizations,
    bench_statistical_validation
);

criterion_main!(zero_overhead_validation);
