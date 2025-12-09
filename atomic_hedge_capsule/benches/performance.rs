//! UCE-32 Q32 Performance Benchmark
//!
//! Validates the performance improvements from various optimizations

use atomic_hedge_capsule::capsule_standalone::AtomicHedgeCapsule;
use atomic_hedge_capsule::types::{BracketOrder, EntryOrder, OrderState};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// UCE-32 Q30: Performance benchmark for nightly optimizations
fn bench_optimized_operations(c: &mut Criterion) {
    let capsule = AtomicHedgeCapsule::new();

    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    c.bench_function("optimized_state_update", |b| {
        b.iter(|| {
            let _ = capsule.update_entry_state(OrderState::Validated, black_box(0.5));
        })
    });

    c.bench_function("optimized_generation_increment", |b| {
        b.iter(|| {
            let _ = capsule.increment_generation();
        })
    });

    c.bench_function("optimized_hedge_state_check", |b| {
        b.iter(|| {
            let _ = capsule.get_hedge_state();
        })
    });

    c.bench_function("optimized_emergency_check", |b| {
        b.iter(|| {
            let _ = capsule.is_emergency_stopped();
        })
    });
}

/// UCE-32 Q30: Cache-optimized hot path benchmark
fn bench_cache_optimized_hot_path(c: &mut Criterion) {
    let capsule = AtomicHedgeCapsule::new();

    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    c.bench_function("cache_optimized_hot_path", |b| {
        b.iter(|| {
            // Typical hot path operations
            let _active = capsule.is_active();
            let _gen = capsule.increment_generation();
            let _emergency = capsule.is_emergency_stopped();
            let _state = capsule.get_hedge_state();
        })
    });
}

/// UCE-32 Q30: Branch prediction optimization benchmark
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

    // Benchmark likely path (normal operation)
    c.bench_function("likely_path_update", |b| {
        b.iter(|| {
            let _ = capsule.update_entry_state(OrderState::Validated, black_box(0.5));
        })
    });
}

/// UCE-32 Q30: Memory ordering optimization benchmark
fn bench_memory_ordering_optimization(c: &mut Criterion) {
    let capsule = AtomicHedgeCapsule::new();

    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    capsule.initialize(entry, bracket).unwrap();

    c.bench_function("optimized_memory_ordering", |b| {
        b.iter(|| {
            // Operations using optimized memory ordering
            let _ = capsule.is_active(); // Relaxed ordering
            let _ = capsule.increment_generation(); // Relaxed ordering
            let _ = capsule.is_emergency_stopped(); // Acquire ordering
        })
    });
}

/// UCE-32 Q30: Const fn optimization benchmark
fn bench_const_fn_optimization(c: &mut Criterion) {
    use atomic_hedge_capsule::capsule_standalone::{EMERGENCY_HEDGE_NS, HEDGE_TIMEOUT_MS};

    c.bench_function("const_fn_calculations", |b| {
        b.iter(|| {
            // These should be compile-time optimized
            let _emergency = black_box(EMERGENCY_HEDGE_NS);
            let _timeout = black_box(HEDGE_TIMEOUT_MS);
        })
    });
}

criterion_group!(
    benches,
    bench_optimized_operations,
    bench_cache_optimized_hot_path,
    bench_branch_prediction,
    bench_memory_ordering_optimization,
    bench_const_fn_optimization
);

criterion_main!(benches);
