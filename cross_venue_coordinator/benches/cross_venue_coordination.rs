//! Cross-Venue Coordination Benchmarks
//!
//! Performance validation following B32 benchmarking framework with
//! fair baselines and statistical rigor.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BatchSize};
use cross_venue_coordinator::{
    CrossVenueCoordinator, CoordinatorConfig, CoordinationRequest, CoordinationType,
    SelectionStrategy, VenueSelectionConfig, CoordinationPriority,
};

/// Benchmark coordinator creation and initialization
fn bench_coordinator_creation(c: &mut Criterion) {
    c.bench_function("coordinator_creation", |b| {
        b.iter(|| {
            let config = CoordinatorConfig::default();
            black_box(CrossVenueCoordinator::new(config))
        });
    });
}

/// Benchmark simple arbitrage coordination
fn bench_simple_arbitrage(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_arbitrage");

    // Configure for statistical validity (B32 framework)
    group.confidence_level(0.95)
         .sample_size(1000);

    group.bench_function("two_venue_coordination", |b| {
        let coordinator = CrossVenueCoordinator::with_defaults();

        b.iter_batched(
            || {
                CoordinationRequest {
                    venues: vec![0, 1],
                    coordination_type: CoordinationType::SimpleArbitrage { venue_a: 0, venue_b: 1 },
                    max_latency_ns: 1_000_000, // 1ms
                    priority: 0,
                }
            },
            |request| {
                black_box(coordinator.coordinate(request))
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark triangle arbitrage coordination
fn bench_triangle_arbitrage(c: &mut Criterion) {
    let mut group = c.benchmark_group("triangle_arbitrage");

    group.confidence_level(0.95)
         .sample_size(500); // Fewer samples for more complex operation

    group.bench_function("three_venue_coordination", |b| {
        let coordinator = CrossVenueCoordinator::with_defaults();

        b.iter_batched(
            || {
                CoordinationRequest {
                    venues: vec![0, 1, 2],
                    coordination_type: CoordinationType::TriangleArbitrage { venues: [0, 1, 2] },
                    max_latency_ns: 2_000_000, // 2ms
                    priority: 0,
                }
            },
            |request| {
                black_box(coordinator.coordinate(request))
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark venue selection strategies
fn bench_venue_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("venue_selection");

    for strategy in [
        SelectionStrategy::RoundRobin,
        SelectionStrategy::LowestLatency,
        SelectionStrategy::WeightedHealth,
        SelectionStrategy::LoadBalanced,
    ] {
        group.bench_with_input(
            format!("{:?}", strategy),
            &strategy,
            |b, &strategy| {
                use cross_venue_coordinator::{VenueSelector, VenueArray};

                let config = VenueSelectionConfig::default();
                let mut selector = VenueSelector::new(strategy, config);
                let venue_array = VenueArray::new();
                let venues = vec![0, 1, 2, 3, 4, 5];

                b.iter(|| {
                    black_box(selector.select_venues(
                        &venue_array,
                        &venues,
                        CoordinationPriority::Normal,
                    ))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark coordination state operations
fn bench_coordination_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("coordination_state");

    group.bench_function("dual_atomic_operations", |b| {
        use cross_venue_coordinator::DualAtomicU64;
        use core::sync::atomic::Ordering;

        let dual_atomic = DualAtomicU64::new(0, 0);

        b.iter(|| {
            dual_atomic.store_primary(black_box(42), Ordering::Release);
            dual_atomic.store_secondary(black_box(84), Ordering::Release);
            let (primary, secondary) = dual_atomic.load_both();
            black_box((primary, secondary))
        });
    });

    group.bench_function("generation_counter", |b| {
        use cross_venue_coordinator::GenerationCounter;

        let counter = GenerationCounter::new();

        b.iter(|| {
            let (_gen, _seq) = black_box(counter.next_sequence());
        });
    });

    group.finish();
}

/// Benchmark concurrent coordination operations
fn bench_concurrent_coordination(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_coordination");

    // Test scaling up to hardware limits (12 threads based on Intel Ultra 7 155H)
    for thread_count in [1, 2, 4, 8, 12] {
        group.bench_with_input(
            format!("{}_threads", thread_count),
            &thread_count,
            |b, &thread_count| {
                use std::sync::Arc;
                use std::thread;

                let coordinator = Arc::new(CrossVenueCoordinator::with_defaults());

                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|i| {
                            let coordinator = Arc::clone(&coordinator);
                            thread::spawn(move || {
                                let request = CoordinationRequest {
                                    venues: vec![i % 4, (i + 1) % 4], // Cycle through venues
                                    coordination_type: CoordinationType::SimpleArbitrage {
                                        venue_a: i % 4,
                                        venue_b: (i + 1) % 4,
                                    },
                                    max_latency_ns: 1_000_000,
                                    priority: 0,
                                };
                                coordinator.coordinate(request)
                            })
                        })
                        .collect();

                    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
                    black_box(results)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory layout efficiency
fn bench_memory_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_layout");

    group.bench_function("venue_array_access", |b| {
        use cross_venue_coordinator::VenueArray;

        let venue_array = VenueArray::new();

        b.iter(|| {
            // Sequential access pattern (cache-friendly)
            for venue_id in 0..16 {
                let venue = black_box(venue_array.venue(venue_id).unwrap());
                let _metrics = black_box(venue.metrics());
            }
        });
    });

    group.bench_function("random_venue_access", |b| {
        use cross_venue_coordinator::VenueArray;

        let venue_array = VenueArray::new();
        // Pseudo-random access pattern
        let access_pattern = [7, 3, 11, 1, 13, 5, 9, 15, 2, 8, 4, 12, 6, 10, 0, 14];

        b.iter(|| {
            for &venue_id in &access_pattern {
                let venue = black_box(venue_array.venue(venue_id).unwrap());
                let _metrics = black_box(venue.metrics());
            }
        });
    });

    group.finish();
}

/// Benchmark circuit breaker integration
#[cfg(feature = "circuit_breaker")]
fn bench_circuit_breaker(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker");

    group.bench_function("venue_breaker_check", |b| {
        use cross_venue_coordinator::{CircuitBreakerIntegration, BreakerConfig};

        let config = BreakerConfig::default();
        let integration = CircuitBreakerIntegration::new(config);

        b.iter(|| {
            for venue_id in 0..16 {
                let _result = black_box(integration.check_venue_breaker(venue_id));
            }
        });
    });

    group.finish();
}

/// Benchmark arbitrage scanner integration
#[cfg(feature = "arbitrage_scanner")]
fn bench_arbitrage_scanner(c: &mut Criterion) {
    let mut group = c.benchmark_group("arbitrage_scanner");

    group.bench_function("simple_arbitrage_scan", |b| {
        use cross_venue_coordinator::{ArbitrageIntegration, ScannerConfig};
        use atomic_venue_snapshot::Avs128Snapshot;

        let config = ScannerConfig::default();
        let mut integration = ArbitrageIntegration::new(config);
        let snapshot_a = Avs128Snapshot::default();
        let snapshot_b = Avs128Snapshot::default();

        b.iter(|| {
            let _result = black_box(integration.scan_simple_arbitrage(
                0, &snapshot_a,
                1, &snapshot_b,
            ));
        });
    });

    group.finish();
}

// Benchmark group configuration
criterion_group!(
    benches,
    bench_coordinator_creation,
    bench_simple_arbitrage,
    bench_triangle_arbitrage,
    bench_venue_selection,
    bench_coordination_state,
    bench_concurrent_coordination,
    bench_memory_layout,
    #[cfg(feature = "circuit_breaker")]
    bench_circuit_breaker,
    #[cfg(feature = "arbitrage_scanner")]
    bench_arbitrage_scanner,
);

criterion_main!(benches);