//! B32 Framework: CAKES Manifold Engine Performance Benchmarks
//! Following UCE32 Q30 (Empirical Validation) + Q31 (Rust Transform) + Kontext27 Hardware Reality
//!
//! Benchmark Categories:
//! 1. DualAtomicU64 coordination - lockfree operations
//! 2. k-NN graph construction - O(N log N) complexity
//! 3. Manifold distance calculations - fractal-aware metrics
//! 4. k-NN search performance - targeting O(1) amortized
//! 5. √N memory complexity validation
//! 6. Real-time update patterns for HFT scenarios

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use fractal_arbitrage_scanner::cakes_manifold::{
    CakesManifoldEngine, DualAtomicU64, LocalFractalDimensionCalculator, MarketPoint
};
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Kontext27 Reality Check: Expected performance baselines
const TYPICAL_SPEEDUP: f64 = 1.5;        // 50% improvement typical
const EXCEPTIONAL_SPEEDUP: f64 = 5.0;     // 5x requires validation
const REVOLUTIONARY_THRESHOLD: f64 = 100.0; // 100x needs extensive proof

/// B32 Framework: Statistical validation requirements
const MIN_ITERATIONS: u64 = 1000;
const CONFIDENCE_INTERVAL: f64 = 0.95;

/// Generate realistic OHLC market data for benchmarking
fn generate_ohlc_data(count: usize, base_price: f64, volatility: f64) -> Vec<MarketPoint> {
    let mut points = Vec::with_capacity(count);
    let mut price = base_price;

    for i in 0..count {
        // Simulate realistic OHLC relationships
        let open = price;
        let close = price + volatility * ((i as f64 * 0.1).sin() - 0.5);
        let high = open.max(close) + volatility * 0.5 * (i as f64 * 0.05).cos().abs();
        let low = open.min(close) - volatility * 0.3 * (i as f64 * 0.07).sin().abs();

        let point = MarketPoint::new(
            [open, high, low, close],
            (1_000_000_000 + i * 1000) as u64, // Microsecond timestamps
            1.3 + 0.1 * (i as f64 * 0.02).sin(), // Varying fractal dimension
            1000.0 + 100.0 * (i as f64 * 0.03).cos(), // Volume variation
        );

        points.push(point);
        price = close; // Next open = previous close
    }

    points
}

/// Benchmark DualAtomicU64 lockfree coordination
fn bench_dual_atomic_coordination(c: &mut Criterion) {
    let mut group = c.benchmark_group("dual_atomic_coordination");

    let dual = DualAtomicU64::new(0, 0);

    // Test different access patterns
    group.bench_function("load_primary", |b| {
        b.iter(|| {
            black_box(dual.load_primary(black_box(Ordering::Relaxed)))
        });
    });

    group.bench_function("load_secondary", |b| {
        b.iter(|| {
            black_box(dual.load_secondary(black_box(Ordering::Relaxed)))
        });
    });

    group.bench_function("store_primary", |b| {
        b.iter(|| {
            dual.store_primary(black_box(42), black_box(Ordering::Relaxed))
        });
    });

    group.bench_function("compare_exchange_primary", |b| {
        b.iter(|| {
            let current = dual.load_primary(Ordering::Relaxed);
            black_box(dual.compare_exchange_primary(
                black_box(current),
                black_box(current + 1),
                black_box(Ordering::SeqCst),
                black_box(Ordering::Relaxed)
            ))
        });
    });

    // Concurrent access simulation
    group.bench_function("alternating_access", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            if counter % 2 == 0 {
                dual.store_primary(black_box(counter), Ordering::Release);
            } else {
                dual.store_secondary(black_box(counter), Ordering::Release);
            }
            counter += 1;
        });
    });

    group.finish();
}

/// Benchmark fractal dimension calculations
fn bench_fractal_dimension_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("fractal_dimension");

    let sizes = [50, 100, 200, 500, 1000];

    for size in sizes {
        let points = generate_ohlc_data(size, 100.0, 0.02);

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("calculate_lfd", size),
            &points,
            |b, points| {
                let mut calculator = LocalFractalDimensionCalculator::new();
                b.iter(|| {
                    black_box(calculator.calculate_lfd(black_box(points)).unwrap())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark manifold distance calculations
fn bench_manifold_distances(c: &mut Criterion) {
    let mut group = c.benchmark_group("manifold_distances");

    let points = generate_ohlc_data(1000, 100.0, 0.01);
    let query_point = &points[500]; // Middle point as query

    group.bench_function("euclidean_distance", |b| {
        b.iter(|| {
            let mut total_distance = 0.0;
            for point in &points {
                total_distance += query_point.euclidean_distance(black_box(point));
            }
            black_box(total_distance)
        });
    });

    group.bench_function("manifold_distance", |b| {
        b.iter(|| {
            let mut total_distance = 0.0;
            for point in &points {
                total_distance += query_point.manifold_distance(black_box(point), black_box(1.3));
            }
            black_box(total_distance)
        });
    });

    // Test different fractal dimensions
    let dimensions = [1.0, 1.3, 1.5, 1.8, 2.0];
    for dim in dimensions {
        group.bench_with_input(
            BenchmarkId::new("manifold_varying_dimension", (dim * 10.0) as u32),
            &dim,
            |b, &dimension| {
                b.iter(|| {
                    query_point.manifold_distance(black_box(&points[0]), black_box(dimension))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark k-NN graph construction scaling
fn bench_knn_graph_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("knn_graph_construction");
    group.measurement_time(Duration::from_secs(20)); // Longer measurement for complex operations
    group.sample_size(30);

    // Test different dataset sizes
    let sizes = [100, 250, 500, 1000, 2000];

    for size in sizes {
        let points = generate_ohlc_data(size, 100.0, 0.015);

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("build_graph", size),
            &points,
            |b, points| {
                b.iter(|| {
                    let mut engine = CakesManifoldEngine::new();

                    // Add all points
                    for point in points {
                        engine.add_point(point.clone()).unwrap();
                    }

                    // Build the graph - this is the expensive operation
                    black_box(engine.build_graph().unwrap())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark k-NN search performance - targeting O(1) amortized
fn bench_knn_search_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("knn_search");

    let sizes = [500, 1000, 2000, 5000];
    let k_values = [5, 10, 20, 50];

    for size in sizes {
        let points = generate_ohlc_data(size, 100.0, 0.01);

        // Pre-build the engine
        let mut engine = CakesManifoldEngine::new();
        for point in &points {
            engine.add_point(point.clone()).unwrap();
        }
        engine.build_graph().unwrap();

        // Query point (not in the dataset)
        let query = MarketPoint::new([105.0, 106.0, 104.0, 105.25], 999999999, 1.35, 1500.0);

        for k in k_values {
            group.throughput(Throughput::Elements(1)); // Per-search throughput

            group.bench_with_input(
                BenchmarkId::new(format!("search_k{}", k), size),
                &(&engine, &query, k),
                |b, (engine, query, &k)| {
                    b.iter(|| {
                        black_box(engine.search_knn(black_box(query), black_box(k)).unwrap())
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark memory allocation patterns - √N complexity validation
fn bench_memory_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_complexity");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    // Test √N allocation pattern vs linear
    let sizes = [1024, 4096, 9216, 16384, 25600]; // Roughly square numbers

    for size in sizes {
        let sqrt_size = (size as f64).sqrt() as usize;

        group.bench_with_input(
            BenchmarkId::new("sqrt_n_pattern", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    // Simulate √N memory allocation for CAKES manifold storage
                    let mut manifold_blocks: Vec<Vec<MarketPoint>> = Vec::new();

                    for i in 0..sqrt_size {
                        let block = generate_ohlc_data(sqrt_size, 100.0 + i as f64, 0.01);
                        manifold_blocks.push(black_box(block));
                    }

                    black_box(manifold_blocks)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("linear_pattern", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    // Linear allocation for comparison
                    let points = generate_ohlc_data(size, 100.0, 0.01);
                    black_box(points)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark real-time update scenarios for HFT
fn bench_realtime_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("realtime_updates");

    // Pre-build engine with base dataset
    let base_points = generate_ohlc_data(1000, 100.0, 0.01);
    let mut engine = CakesManifoldEngine::new();
    for point in &base_points {
        engine.add_point(point.clone()).unwrap();
    }
    engine.build_graph().unwrap();

    // New incoming market data
    let new_points = generate_ohlc_data(10, 105.0, 0.015);

    group.bench_function("incremental_point_addition", |b| {
        b.iter(|| {
            let mut engine_copy = CakesManifoldEngine::new();

            // Add base dataset
            for point in &base_points {
                engine_copy.add_point(point.clone()).unwrap();
            }

            // Add new points incrementally
            for point in &new_points {
                engine_copy.add_point(black_box(point.clone())).unwrap();
            }

            black_box(engine_copy)
        });
    });

    group.bench_function("batch_graph_rebuild", |b| {
        b.iter(|| {
            let mut engine_copy = CakesManifoldEngine::new();

            // Add all points (base + new)
            for point in base_points.iter().chain(new_points.iter()) {
                engine_copy.add_point(point.clone()).unwrap();
            }

            // Rebuild entire graph
            black_box(engine_copy.build_graph().unwrap())
        });
    });

    // Benchmark search performance with continuously updating data
    group.bench_function("search_during_updates", |b| {
        let query = MarketPoint::new([103.0, 104.0, 102.0, 103.5], 999999998, 1.32, 1250.0);

        b.iter(|| {
            // Simulate searches happening during data updates
            let results1 = engine.search_knn(&query, 5).unwrap();
            let results2 = engine.search_knn(&query, 10).unwrap();
            black_box((results1, results2))
        });
    });

    group.finish();
}

/// Comprehensive CAKES pipeline benchmark
fn bench_cakes_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("cakes_pipeline");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(25);

    let points = generate_ohlc_data(1500, 100.0, 0.02);
    let queries = generate_ohlc_data(10, 105.0, 0.01);

    group.bench_function("complete_cakes_workflow", |b| {
        b.iter(|| {
            // Complete CAKES workflow from data ingestion to search
            let mut engine = CakesManifoldEngine::new();

            // 1. Data ingestion
            for point in &points {
                engine.add_point(point.clone()).unwrap();
            }

            // 2. Graph construction
            engine.build_graph().unwrap();

            // 3. Multiple searches
            let mut all_results = Vec::new();
            for query in &queries {
                let results = engine.search_knn(query, 10).unwrap();
                all_results.push(results);
            }

            // 4. Statistics collection
            let stats = engine.stats();

            black_box((all_results, stats))
        });
    });

    group.finish();
}

/// Performance scaling analysis - validate O(1) search claims
fn bench_search_scaling_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_scaling_analysis");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(50);

    // Test if search time remains constant as dataset grows (O(1) amortized)
    let sizes = [100, 500, 1000, 2500, 5000, 10000];

    for size in sizes {
        let points = generate_ohlc_data(size, 100.0, 0.01);

        // Pre-build engine
        let mut engine = CakesManifoldEngine::new();
        for point in &points {
            engine.add_point(point.clone()).unwrap();
        }
        engine.build_graph().unwrap();

        let query = MarketPoint::new([102.5, 103.5, 101.5, 103.0], 999999997, 1.34, 1300.0);

        group.throughput(Throughput::Elements(1)); // Per-search throughput

        group.bench_with_input(
            BenchmarkId::new("single_search", size),
            &(&engine, &query),
            |b, (engine, query)| {
                b.iter(|| {
                    black_box(engine.search_knn(black_box(query), black_box(10)).unwrap())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    cakes_benches,
    bench_dual_atomic_coordination,
    bench_fractal_dimension_calculation,
    bench_manifold_distances,
    bench_knn_graph_construction,
    bench_knn_search_performance,
    bench_memory_complexity,
    bench_realtime_updates,
    bench_cakes_pipeline,
    bench_search_scaling_analysis
);

criterion_main!(cakes_benches);