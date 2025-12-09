//! Risk Correlation Engine Benchmarks
//!
//! Validates Q29 performance constraints:
//! - <50ns for correlation updates
//! - <20ns for single correlation access
//! - <100ns for concentration risk calculation
//! - <30ns for cross-venue arbitrage detection
//!
//! Uses Criterion for statistical validation with 95% confidence intervals

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use atomic_portfolio_map::{
    RiskCorrelationEngine, CorrelationMatrix, DualAtomicU64, PortfolioCrossVenueCoordinator,
    VenuePriceFeed, MAX_ASSETS,
};
use std::sync::Arc;

/// Benchmark DualAtomicU64 operations
fn bench_dual_atomic_u64(c: &mut Criterion) {
    let mut group = c.benchmark_group("DualAtomicU64");
    group.throughput(Throughput::Elements(1));

    let dual = DualAtomicU64::new();

    group.bench_function("store_correlation", |b| {
        let mut counter = 0u32;
        b.iter(|| {
            counter = counter.wrapping_add(1);
            dual.store_correlation(
                black_box(0.75),
                black_box(counter),
                black_box(50000)
            );
        })
    });

    group.bench_function("load_correlation", |b| {
        dual.store_correlation(0.8, 100, 60000);
        b.iter(|| {
            let result = dual.load_correlation();
            black_box(result);
        })
    });

    group.bench_function("cas_correlation", |b| {
        let mut counter = 0u32;
        b.iter(|| {
            counter = counter.wrapping_add(1);
            let success = dual.cas_correlation(
                black_box(0.5),
                black_box(0.6),
                black_box(counter),
                black_box(55000)
            );
            black_box(success);
        })
    });

    group.finish();
}

/// Benchmark correlation matrix operations
fn bench_correlation_matrix(c: &mut Criterion) {
    let mut group = c.benchmark_group("CorrelationMatrix");
    group.throughput(Throughput::Elements(1));

    let matrix = CorrelationMatrix::new();

    // Pre-populate matrix with some correlations
    for i in 0..8 {
        for j in (i + 1)..8 {
            matrix.update_correlation(i, j, 0.5 + (i as f64 * 0.1), 50000);
        }
    }

    group.bench_function("update_correlation", |b| {
        let mut asset_counter = 0usize;
        b.iter(|| {
            let asset_a = asset_counter % 8;
            let asset_b = (asset_counter + 1) % 8;
            asset_counter = asset_counter.wrapping_add(1);

            let success = matrix.update_correlation(
                black_box(asset_a),
                black_box(asset_b),
                black_box(0.7),
                black_box(60000)
            );
            black_box(success);
        })
    });

    group.bench_function("get_correlation", |b| {
        let mut asset_counter = 0usize;
        b.iter(|| {
            let asset_a = asset_counter % 8;
            let asset_b = (asset_counter + 1) % 8;
            asset_counter = asset_counter.wrapping_add(1);

            let result = matrix.get_correlation(
                black_box(asset_a),
                black_box(asset_b)
            );
            black_box(result);
        })
    });

    // Test concentration risk calculation with different SIMD configurations
    let mut weights = [0.0; MAX_ASSETS];
    for i in 0..8 {
        weights[i] = 1.0 / 8.0; // Equal weights for first 8 assets
    }

    group.bench_function("calculate_concentration_risk", |b| {
        b.iter(|| {
            let risk = matrix.calculate_concentration_risk(black_box(&weights));
            black_box(risk);
        })
    });

    // Benchmark systemic risk detection
    group.bench_function("detect_systemic_risk", |b| {
        let mut shock_counter = 0usize;
        b.iter(|| {
            let shock_asset = shock_counter % 8;
            shock_counter = shock_counter.wrapping_add(1);

            let result = matrix.detect_systemic_risk(
                black_box(shock_asset),
                black_box(0.5)
            );
            black_box(result);
        })
    });

    group.finish();
}

/// Benchmark risk correlation engine operations
fn bench_risk_correlation_engine(c: &mut Criterion) {
    let mut group = c.benchmark_group("RiskCorrelationEngine");
    group.throughput(Throughput::Elements(1));

    let engine = RiskCorrelationEngine::new();
    let mut weights = [0.0; MAX_ASSETS];
    for i in 0..8 {
        weights[i] = 1.0 / 8.0;
    }

    group.bench_function("update_and_assess", |b| {
        let mut counter = 0usize;
        b.iter(|| {
            let asset_a = counter % 8;
            let asset_b = (counter + 1) % 8;
            counter = counter.wrapping_add(1);

            let result = engine.update_and_assess(
                black_box(asset_a),
                black_box(asset_b),
                black_box(0.6),
                black_box(55000),
                black_box(&weights)
            );
            black_box(result);
        })
    });

    group.bench_function("check_systemic_risk", |b| {
        let mut counter = 0usize;
        b.iter(|| {
            let shock_asset = counter % 8;
            counter = counter.wrapping_add(1);

            let result = engine.check_systemic_risk(
                black_box(shock_asset),
                black_box(0.4)
            );
            black_box(result);
        })
    });

    group.bench_function("assess_portfolio_risk", |b| {
        b.iter(|| {
            let assessment = engine.assess_portfolio_risk(black_box(&weights));
            black_box(assessment);
        })
    });

    group.finish();
}

/// Benchmark cross-venue coordinator operations
fn bench_cross_venue_coordinator(c: &mut Criterion) {
    let mut group = c.benchmark_group("CrossVenueCoordinator");
    group.throughput(Throughput::Elements(1));

    let risk_engine = Arc::new(RiskCorrelationEngine::new());
    let mut coordinator = PortfolioCrossVenueCoordinator::new(risk_engine);

    // Register venues
    for venue_id in 1..=4 {
        for symbol_id in 100..=110 {
            coordinator.register_venue_feed(venue_id, symbol_id);
        }
    }

    // Create test price feeds
    let mut test_feeds = Vec::new();
    for venue_id in 1..=4 {
        for symbol_id in 100..=110 {
            let mut feed = VenuePriceFeed::new(venue_id, symbol_id);
            feed.bid_price = 100_000_000 + (venue_id as u64 * 1000) + (symbol_id as u64 * 100);
            feed.ask_price = feed.bid_price + 5000; // 5 cent spread
            feed.last_price = feed.bid_price + 2500;
            feed.volume = 1000;
            feed.quality_score = 50000;
            test_feeds.push(feed);
        }
    }

    group.bench_function("update_venue_price", |b| {
        let mut feed_counter = 0usize;
        b.iter(|| {
            let feed = &test_feeds[feed_counter % test_feeds.len()];
            feed_counter = feed_counter.wrapping_add(1);

            let result = coordinator.update_venue_price(
                black_box(feed.venue_id),
                black_box(feed.symbol_id),
                black_box(feed)
            );
            black_box(result);
        })
    });

    group.finish();
}

/// Benchmark venue price feed operations
fn bench_venue_price_feed(c: &mut Criterion) {
    let mut group = c.benchmark_group("VenuePriceFeed");
    group.throughput(Throughput::Elements(1));

    let mut feed = VenuePriceFeed::new(1, 100);
    feed.bid_price = 100_000_000; // $100.00
    feed.ask_price = 100_050_000; // $100.05

    group.bench_function("spread_bps", |b| {
        b.iter(|| {
            let spread = feed.spread_bps();
            black_box(spread);
        })
    });

    group.bench_function("mid_price", |b| {
        b.iter(|| {
            let mid = feed.mid_price();
            black_box(mid);
        })
    });

    group.finish();
}

/// Benchmark scaling with different numbers of assets
fn bench_correlation_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("CorrelationScaling");

    for asset_count in [4, 8, 12, 16].iter() {
        group.throughput(Throughput::Elements(*asset_count as u64));

        let matrix = CorrelationMatrix::new();
        let mut weights = [0.0; MAX_ASSETS];
        for i in 0..*asset_count {
            weights[i] = 1.0 / (*asset_count as f64);
        }

        // Pre-populate correlations
        for i in 0..*asset_count {
            for j in (i + 1)..*asset_count {
                matrix.update_correlation(i, j, 0.3 + (i as f64 * 0.05), 55000);
            }
        }

        group.bench_with_input(
            BenchmarkId::new("concentration_risk", asset_count),
            asset_count,
            |b, &asset_count| {
                b.iter(|| {
                    let risk = matrix.calculate_concentration_risk(black_box(&weights));
                    black_box(risk);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("systemic_risk", asset_count),
            asset_count,
            |b, &asset_count| {
                let mut shock_counter = 0usize;
                b.iter(|| {
                    let shock_asset = shock_counter % asset_count;
                    shock_counter = shock_counter.wrapping_add(1);

                    let result = matrix.detect_systemic_risk(
                        black_box(shock_asset),
                        black_box(0.4)
                    );
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

/// Memory bandwidth benchmark for atomic operations
fn bench_memory_bandwidth(c: &mut Criterion) {
    let mut group = c.benchmark_group("MemoryBandwidth");

    // Create array of DualAtomicU64 to test memory bandwidth
    let atomics: Vec<DualAtomicU64> = (0..1000).map(|_| DualAtomicU64::new()).collect();

    group.bench_function("sequential_updates", |b| {
        let mut counter = 0u32;
        b.iter(|| {
            counter = counter.wrapping_add(1);
            for (i, atomic) in atomics.iter().enumerate() {
                atomic.store_correlation(
                    black_box(0.5 + (i as f64 * 0.001)),
                    black_box(counter),
                    black_box(50000)
                );
            }
        })
    });

    group.bench_function("random_access_updates", |b| {
        let mut counter = 0u32;
        let mut index = 0usize;
        b.iter(|| {
            counter = counter.wrapping_add(1);
            index = (index.wrapping_mul(1103515245).wrapping_add(12345)) % atomics.len();

            atomics[index].store_correlation(
                black_box(0.7),
                black_box(counter),
                black_box(60000)
            );
        })
    });

    group.finish();
}

/// Stress test with high contention
fn bench_contention_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("ContentionStress");

    let matrix = Arc::new(CorrelationMatrix::new());

    group.bench_function("high_contention_updates", |b| {
        let mut counter = 0u32;
        b.iter(|| {
            counter = counter.wrapping_add(1);

            // Multiple threads would be fighting for same correlation pair
            let success = matrix.update_correlation(
                black_box(0),
                black_box(1),
                black_box(0.5 + (counter as f64 * 0.001) % 1.0),
                black_box(counter as u16)
            );
            black_box(success);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_dual_atomic_u64,
    bench_correlation_matrix,
    bench_risk_correlation_engine,
    bench_cross_venue_coordinator,
    bench_venue_price_feed,
    bench_correlation_scaling,
    bench_memory_bandwidth,
    bench_contention_stress
);
criterion_main!(benches);