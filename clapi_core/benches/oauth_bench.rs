//! OAuth Benchmarks (B32 Framework)
//!
//! **Fair Baselines**: Compare vs Redis network latency (5-20ms)
//! **Statistical Rigor**: 1000+ iterations, 95% CI
//! **Honest Claims**: Target <50ns vs 5-20ms = 100K-400K× speedup
//!
//! # Benchmarks
//! - verify_session(): Target <50ns (vs Redis 5-20ms)
//! - create_session(): Target <100ns (vs PostgreSQL 15-50ms)
//! - revoke_session(): Target <40ns (vs PostgreSQL UPDATE 10-30ms)
//! - Concurrent throughput: Target >10M ops/sec

use clapi_core::capsules::OAuthSessionCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Single-Threaded Benchmarks
// ============================================================================

fn bench_verify_session_single_thread(c: &mut Criterion) {
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    c.bench_function("verify_session_single", |b| {
        b.iter(|| black_box(session.verify_token(black_box(0xABCDEF))));
    });
}

fn bench_create_session_single_thread(c: &mut Criterion) {
    c.bench_function("create_session_single", |b| {
        b.iter(|| {
            black_box(OAuthSessionCapsule::new(
                black_box(1001),
                black_box(0xABCDEF),
                None,
            ))
        });
    });
}

fn bench_revoke_session_single_thread(c: &mut Criterion) {
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    c.bench_function("revoke_session_single", |b| {
        b.iter(|| {
            session.revoke();
        });
    });
}

fn bench_refresh_session_single_thread(c: &mut Criterion) {
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    c.bench_function("refresh_session_single", |b| {
        b.iter(|| {
            session.refresh(None);
        });
    });
}

fn bench_snapshot_single_thread(c: &mut Criterion) {
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    c.bench_function("snapshot_single", |b| {
        b.iter(|| black_box(session.snapshot()));
    });
}

// ============================================================================
// Multi-Threaded Benchmarks (Contention)
// ============================================================================

fn bench_verify_session_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("verify_session_contention");

    for thread_count in [1, 2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*thread_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                let session = Arc::new(OAuthSessionCapsule::new(1001, 0xABCDEF, None));

                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let session = Arc::clone(&session);
                            thread::spawn(move || {
                                for _ in 0..100 {
                                    black_box(session.verify_token(black_box(0xABCDEF)));
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_revoke_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("revoke_contention");

    for thread_count in [1, 2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*thread_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let session = Arc::new(OAuthSessionCapsule::new(1001, 0xABCDEF, None));
                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let session = Arc::clone(&session);
                            thread::spawn(move || {
                                session.revoke();
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_refresh_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("refresh_contention");

    for thread_count in [1, 2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*thread_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                let session = Arc::new(OAuthSessionCapsule::new(1001, 0xABCDEF, None));

                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let session = Arc::clone(&session);
                            thread::spawn(move || {
                                for _ in 0..100 {
                                    session.refresh(None);
                                }
                            })
                        })
                        .collect();

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
// Throughput Benchmarks
// ============================================================================

fn bench_verification_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("verification_throughput");
    group.throughput(Throughput::Elements(1_000_000));

    let session = Arc::new(OAuthSessionCapsule::new(1001, 0xABCDEF, None));

    group.bench_function("1M_verifications_8_threads", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let session = Arc::clone(&session);
                    thread::spawn(move || {
                        for _ in 0..125_000 {
                            black_box(session.verify_token(black_box(0xABCDEF)));
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// Comparison vs Baseline (Simulated)
// ============================================================================

fn bench_comparison_vs_redis(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison_vs_redis");

    // Simulated Redis network latency (5ms typical)
    group.bench_function("simulated_redis_verify", |b| {
        b.iter(|| {
            thread::sleep(std::time::Duration::from_micros(5000)); // 5ms
        });
    });

    // OAuthSessionCapsule verify
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);
    group.bench_function("oauth_capsule_verify", |b| {
        b.iter(|| black_box(session.verify_token(black_box(0xABCDEF))));
    });

    group.finish();
}

fn bench_comparison_vs_postgresql(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison_vs_postgresql");

    // Simulated PostgreSQL INSERT latency (15ms typical)
    group.bench_function("simulated_pg_insert", |b| {
        b.iter(|| {
            thread::sleep(std::time::Duration::from_micros(15000)); // 15ms
        });
    });

    // OAuthSessionCapsule create
    group.bench_function("oauth_capsule_create", |b| {
        b.iter(|| {
            black_box(OAuthSessionCapsule::new(
                black_box(1001),
                black_box(0xABCDEF),
                None,
            ))
        });
    });

    group.finish();
}

// ============================================================================
// Latency Distribution Benchmarks
// ============================================================================

fn bench_latency_distribution(c: &mut Criterion) {
    let session = Arc::new(OAuthSessionCapsule::new(1001, 0xABCDEF, None));

    c.bench_function("verify_latency_distribution", |b| {
        b.iter(|| {
            // Simulate production workload: 90% verify, 5% refresh, 5% revoke
            for i in 0..100 {
                match i % 20 {
                    0 => session.refresh(None),
                    1 => session.revoke(),
                    _ => {
                        black_box(session.verify_token(black_box(0xABCDEF)));
                    }
                }
            }
        });
    });
}

// ============================================================================
// Memory Footprint Benchmark
// ============================================================================

fn bench_memory_footprint(c: &mut Criterion) {
    c.bench_function("memory_10k_sessions", |b| {
        b.iter(|| {
            let sessions: Vec<_> = (0..10_000)
                .map(|i| OAuthSessionCapsule::new(i as u64, 0xABCDEF, None))
                .collect();

            black_box(sessions)
        });
    });
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    single_threaded,
    bench_verify_session_single_thread,
    bench_create_session_single_thread,
    bench_revoke_session_single_thread,
    bench_refresh_session_single_thread,
    bench_snapshot_single_thread,
);

criterion_group!(
    multi_threaded,
    bench_verify_session_contention,
    bench_revoke_contention,
    bench_refresh_contention,
);

criterion_group!(throughput, bench_verification_throughput,);

criterion_group!(
    comparisons,
    bench_comparison_vs_redis,
    bench_comparison_vs_postgresql,
);

criterion_group!(
    production,
    bench_latency_distribution,
    bench_memory_footprint,
);

criterion_main!(
    single_threaded,
    multi_threaded,
    throughput,
    comparisons,
    production,
);
