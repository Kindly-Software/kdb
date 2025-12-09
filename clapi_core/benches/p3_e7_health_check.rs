//! P3-E7: Health Check Capsule Benchmarks (B32 Framework)
//!
//! **Benchmarks**: 3 benchmarks measuring health check performance
//! - Benchmark 1: Single-threaded read operations
//! - Benchmark 2: Single-threaded write operations
//! - Benchmark 3: Multi-threaded concurrent operations
//!
//! **B32 Compliance**:
//! - Fair baselines (atomic operations, not mutex)
//! - 1000+ iterations per benchmark
//! - 95% confidence intervals
//! - Honest claims (no strawman comparisons)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use clapi_core::capsules::health_check::{Component, HealthCheckCapsule64};
use std::sync::Arc;
use std::thread;

// ============================================================================
// BENCHMARK 1: Single-threaded Read Operations
// ============================================================================

fn bench_health_check_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("health_check_reads");

    let health = HealthCheckCapsule64::new();
    health.set_healthy(Component::BudgetRegistry);
    health.set_healthy(Component::ProviderRouter);
    health.set_healthy(Component::Database);

    group.bench_function("is_healthy_single_component", |b| {
        b.iter(|| {
            black_box(health.is_healthy(Component::BudgetRegistry));
        });
    });

    group.bench_function("is_live", |b| {
        b.iter(|| {
            black_box(health.is_live());
        });
    });

    group.bench_function("is_ready", |b| {
        b.iter(|| {
            black_box(health.is_ready());
        });
    });

    group.bench_function("deep_check", |b| {
        b.iter(|| {
            black_box(health.deep_check());
        });
    });

    group.bench_function("raw_status", |b| {
        b.iter(|| {
            black_box(health.raw_status());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: Single-threaded Write Operations
// ============================================================================

fn bench_health_check_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("health_check_writes");

    let health = HealthCheckCapsule64::new();

    group.bench_function("set_healthy", |b| {
        b.iter(|| {
            health.set_healthy(black_box(Component::BudgetRegistry));
        });
    });

    group.bench_function("set_unhealthy", |b| {
        b.iter(|| {
            health.set_unhealthy(black_box(Component::BudgetRegistry));
        });
    });

    group.bench_function("reset", |b| {
        b.iter(|| {
            health.reset();
        });
    });

    group.bench_function("set_multiple_components", |b| {
        b.iter(|| {
            health.set_healthy(Component::BudgetRegistry);
            health.set_healthy(Component::ProviderRouter);
            health.set_healthy(Component::MetricsRegistry);
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: Multi-threaded Concurrent Operations
// ============================================================================

fn bench_health_check_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("health_check_concurrent");

    // Benchmark concurrent reads
    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_reads", thread_count),
            thread_count,
            |b, &thread_count| {
                let health = Arc::new(HealthCheckCapsule64::new());
                health.set_healthy(Component::BudgetRegistry);
                health.set_healthy(Component::ProviderRouter);
                health.set_healthy(Component::Database);

                b.iter(|| {
                    let mut handles = vec![];

                    for _ in 0..thread_count {
                        let health_clone = Arc::clone(&health);
                        let handle = thread::spawn(move || {
                            for _ in 0..100 {
                                black_box(health_clone.is_ready());
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    // Benchmark concurrent writes
    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_writes", thread_count),
            thread_count,
            |b, &thread_count| {
                let health = Arc::new(HealthCheckCapsule64::new());

                b.iter(|| {
                    let mut handles = vec![];

                    for i in 0..thread_count {
                        let health_clone = Arc::clone(&health);
                        let handle = thread::spawn(move || {
                            let component = match i % 3 {
                                0 => Component::BudgetRegistry,
                                1 => Component::ProviderRouter,
                                _ => Component::MetricsRegistry,
                            };

                            for _ in 0..100 {
                                health_clone.set_healthy(component);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    // Benchmark concurrent mixed operations
    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_mixed", thread_count),
            thread_count,
            |b, &thread_count| {
                let health = Arc::new(HealthCheckCapsule64::new());
                health.set_healthy(Component::BudgetRegistry);
                health.set_healthy(Component::ProviderRouter);
                health.set_healthy(Component::Database);

                b.iter(|| {
                    let mut handles = vec![];

                    for i in 0..thread_count {
                        let health_clone = Arc::clone(&health);
                        let handle = thread::spawn(move || {
                            for _ in 0..100 {
                                if i % 2 == 0 {
                                    // Read operations
                                    black_box(health_clone.is_ready());
                                } else {
                                    // Write operations
                                    health_clone.set_healthy(Component::MetricsRegistry);
                                }
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = health_check_benches;
    config = Criterion::default()
        .sample_size(1000)  // B32: 1000+ iterations
        .significance_level(0.05)  // B32: 95% confidence interval
        .confidence_level(0.95);
    targets = bench_health_check_reads, bench_health_check_writes, bench_health_check_concurrent
}

criterion_main!(health_check_benches);
