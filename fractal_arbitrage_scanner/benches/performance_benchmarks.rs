//! Performance benchmarks for quantum arbitrage scanner
//!
//! Implements T42 framework performance validation with UCE32 Q30 (Empirical Validation)
//! using Criterion for statistical benchmark analysis.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use fractal_arbitrage_scanner::{
    QuantumArbitrageScanner, TemporalArbitrageOpportunity, TunnelingScanner,
    Aid96, ArbitrageOpportunity, OpportunityParams, aid_class,
};
use std::time::Duration;

/// Benchmark arbitrage opportunity creation
fn bench_arbitrage_creation(c: &mut Criterion) {
    let scanner = QuantumArbitrageScanner::new(42);

    c.bench_function("arbitrage_creation", |b| {
        b.iter(|| {
            let result = scanner.scan_arbitrage(
                black_box("BTC/USD"),
                black_box("binance"),
                black_box("coinbase"),
                black_box(50_000.0),
                black_box(50_100.0),
                black_box(1.0),
            );
            black_box(result)
        })
    });
}

/// Benchmark arbitrage opportunity validation with different error cases
fn bench_arbitrage_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("arbitrage_validation");

    // Valid case
    group.bench_function("valid", |b| {
        b.iter(|| {
            let id = Aid96::new(aid_class::PEX);
            let params = OpportunityParams {
                buy_exchange: "binance".to_string(),
                sell_exchange: "coinbase".to_string(),
                symbol: "BTC/USD".to_string(),
                buy_price: black_box(50_000.0),
                sell_price: black_box(50_100.0),
                volume: black_box(1.0),
                timestamp_nanos: black_box(1_000_000_000_000),
                ttl_nanos: black_box(Duration::from_millis(250).as_nanos() as u64),
            };
            let result = ArbitrageOpportunity::new(id, params);
            black_box(result)
        })
    });

    // Invalid price case
    group.bench_function("invalid_price", |b| {
        b.iter(|| {
            let id = Aid96::new(aid_class::PEX);
            let params = OpportunityParams {
                buy_exchange: "binance".to_string(),
                sell_exchange: "coinbase".to_string(),
                symbol: "BTC/USD".to_string(),
                buy_price: black_box(-50_000.0), // Invalid
                sell_price: black_box(50_100.0),
                volume: black_box(1.0),
                timestamp_nanos: black_box(1_000_000_000_000),
                ttl_nanos: black_box(Duration::from_millis(250).as_nanos() as u64),
            };
            let result = ArbitrageOpportunity::new(id, params);
            black_box(result)
        })
    });

    group.finish();
}

/// Benchmark temporal opportunity creation with confidence clamping
fn bench_temporal_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("temporal_creation");

    group.bench_function("normal_confidence", |b| {
        b.iter(|| {
            let temporal = TemporalArbitrageOpportunity::new(
                black_box("BTC/USD"),
                black_box(50_000.0),
                black_box(51_000.0),
                black_box(0.75), // Normal confidence
                black_box(Duration::from_millis(100)),
            );
            black_box(temporal)
        })
    });

    group.bench_function("clamped_confidence", |b| {
        b.iter(|| {
            let temporal = TemporalArbitrageOpportunity::new(
                black_box("BTC/USD"),
                black_box(50_000.0),
                black_box(51_000.0),
                black_box(1.5), // Needs clamping
                black_box(Duration::from_millis(100)),
            );
            black_box(temporal)
        })
    });

    group.finish();
}

/// Benchmark tunneling opportunity creation and barrier classification
fn bench_tunneling_creation(c: &mut Criterion) {
    let scanner = TunnelingScanner::new(42);
    let mut group = c.benchmark_group("tunneling_creation");

    group.bench_function("resistance_barrier", |b| {
        b.iter(|| {
            let tunneling = scanner.derive_opportunity(
                black_box("BTC/USD"),
                black_box(50_000.0),
                black_box(51_000.0), // Resistance
            );
            black_box(tunneling)
        })
    });

    group.bench_function("support_barrier", |b| {
        b.iter(|| {
            let tunneling = scanner.derive_opportunity(
                black_box("BTC/USD"),
                black_box(50_000.0),
                black_box(49_000.0), // Support
            );
            black_box(tunneling)
        })
    });

    group.finish();
}

/// Benchmark high-throughput scanning scenarios
fn bench_high_throughput_scanning(c: &mut Criterion) {
    let scanner = QuantumArbitrageScanner::new(1337);
    let mut group = c.benchmark_group("high_throughput");

    // Configure throughput measurement
    group.throughput(Throughput::Elements(1000));

    group.bench_function("arbitrage_batch", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let base_price = 50_000.0 + i as f64;
                let result = scanner.scan_arbitrage(
                    "BTC/USD",
                    "binance",
                    "coinbase",
                    base_price,
                    base_price + 10.0,
                    1.0,
                );
                black_box(result);
            }
        })
    });

    group.bench_function("temporal_batch", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let base_price = 50_000.0 + i as f64;
                let temporal = scanner.temporal_hint(
                    "BTC/USD",
                    base_price,
                    base_price + 20.0,
                    0.7,
                    Duration::from_millis(100),
                );
                black_box(temporal);
            }
        })
    });

    group.bench_function("tunneling_batch", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let base_price = 50_000.0 + i as f64;
                let tunneling = scanner.tunneling_hint(
                    "BTC/USD",
                    base_price,
                    base_price + 30.0,
                );
                black_box(tunneling);
            }
        })
    });

    group.finish();
}

/// Benchmark mixed workload performance
fn bench_mixed_workload(c: &mut Criterion) {
    let scanner = QuantumArbitrageScanner::new(42);

    c.bench_function("mixed_operations", |b| {
        b.iter(|| {
            for i in 0..100 {
                let base_price = 50_000.0 + i as f64;

                // Arbitrage (might fail validation)
                let _arbitrage = scanner.scan_arbitrage(
                    "BTC/USD",
                    "binance",
                    "coinbase",
                    base_price,
                    base_price + 5.0,
                    1.0,
                );

                // Temporal (always succeeds)
                let _temporal = scanner.temporal_hint(
                    "BTC/USD",
                    base_price,
                    base_price + 10.0,
                    0.6,
                    Duration::from_millis(75),
                );

                // Tunneling (always succeeds)
                let _tunneling = scanner.tunneling_hint(
                    "BTC/USD",
                    base_price,
                    base_price + 15.0,
                );
            }
        })
    });
}

/// Benchmark profit calculation performance
fn bench_profit_calculations(c: &mut Criterion) {
    let id = Aid96::new(aid_class::PEX);
    let params = OpportunityParams {
        buy_exchange: "binance".to_string(),
        sell_exchange: "coinbase".to_string(),
        symbol: "BTC/USD".to_string(),
        buy_price: 50_000.0,
        sell_price: 50_100.0,
        volume: 1.0,
        timestamp_nanos: 1_000_000_000_000,
        ttl_nanos: Duration::from_millis(250).as_nanos() as u64,
    };

    let opportunity = ArbitrageOpportunity::new(id, params).unwrap();

    c.bench_function("profit_calculation", |b| {
        b.iter(|| {
            let profit = opportunity.estimated_profit();
            black_box(profit)
        })
    });
}

/// Benchmark ID generation performance
fn bench_id_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("id_generation");

    group.bench_function("pex_class", |b| {
        b.iter(|| {
            let id = Aid96::new(aid_class::PEX);
            black_box(id)
        })
    });

    group.bench_function("alt_class", |b| {
        b.iter(|| {
            let id = Aid96::new(aid_class::ALT);
            black_box(id)
        })
    });

    group.bench_function("dos_class", |b| {
        b.iter(|| {
            let id = Aid96::new(aid_class::DOS);
            black_box(id)
        })
    });

    group.finish();
}

/// Benchmark serialization performance
fn bench_serialization(c: &mut Criterion) {
    let scanner = QuantumArbitrageScanner::new(42);

    // Create test opportunities
    let arbitrage = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        50_000.0,
        50_100.0,
        1.0,
    ).unwrap();

    let temporal = scanner.temporal_hint(
        "BTC/USD",
        50_000.0,
        51_000.0,
        0.8,
        Duration::from_millis(100),
    );

    let tunneling = scanner.tunneling_hint("BTC/USD", 50_000.0, 51_000.0);

    let mut group = c.benchmark_group("serialization");

    group.bench_function("arbitrage_serialize", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(&arbitrage).unwrap();
            black_box(serialized)
        })
    });

    group.bench_function("arbitrage_deserialize", |b| {
        let serialized = serde_json::to_string(&arbitrage).unwrap();
        b.iter(|| {
            let deserialized: ArbitrageOpportunity =
                serde_json::from_str(&serialized).unwrap();
            black_box(deserialized)
        })
    });

    group.bench_function("temporal_serialize", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(&temporal).unwrap();
            black_box(serialized)
        })
    });

    group.bench_function("tunneling_serialize", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(&tunneling).unwrap();
            black_box(serialized)
        })
    });

    group.finish();
}

/// Benchmark circuit breaker overhead (UCE32 Q30: <10ns requirement)
fn bench_circuit_breaker_overhead(c: &mut Criterion) {
    use fractal_arbitrage_scanner::hydra::HydraCoordinationEngine;
    use atomic_breaker::{AtomicBreakerGuard, AtomicBreakerSWeMR};
    use atomic_breaker::breaker::State as BreakerState;

    let engine = HydraCoordinationEngine::new();

    let mut group = c.benchmark_group("circuit_breaker");
    group.throughput(Throughput::Elements(1));

    // Benchmark the critical path: breaker check overhead
    group.bench_function("breaker_check_overhead", |b| {
        b.iter(|| {
            // This simulates the exact check in perform_coordinated_analysis
            let breaker_guard = AtomicBreakerGuard::new(black_box(engine.breaker.load_acquire()));
            let _state_check = match breaker_guard.state() {
                BreakerState::Open | BreakerState::ForcedOpen => false,
                _ => true,
            };
            black_box(_state_check)
        })
    });

    // Benchmark baseline without breaker for comparison
    group.bench_function("baseline_no_breaker", |b| {
        b.iter(|| {
            // Minimal work to establish baseline
            black_box(true)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_arbitrage_creation,
    bench_arbitrage_validation,
    bench_temporal_creation,
    bench_tunneling_creation,
    bench_high_throughput_scanning,
    bench_mixed_workload,
    bench_profit_calculations,
    bench_id_generation,
    bench_serialization,
    bench_circuit_breaker_overhead,
);

criterion_main!(benches);