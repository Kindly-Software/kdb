//! P2 E24 Enhancement: DashMap → ConcurrentMapCapsule Migration Benchmark
//!
//! **Purpose**: Validate 3-59× speedup claims for MultiTenantTimelineCapsule
//!
//! **B32 Framework**: Fair baseline comparison
//! - Baseline: DashMap (existing implementation, Phase P1)
//! - Optimized: ConcurrentMapCapsule (P2 migration, lockfree)
//! - Same hardware, same compiler, same input distribution
//!
//! **Expected Speedup**:
//! - Tenant lookup (get_or_insert): 3-10× (100ns vs 500-1000ns)
//! - Contention patterns: 10-59× at high thread counts (16+ threads)
//!
//! **Test Scenarios**:
//! 1. Single-threaded lookup (establishes baseline)
//! 2. Multi-threaded lookup (8 threads, typical)
//! 3. High contention (16 threads, 1000 tenants)
//! 4. Append workload (realistic multi-tenant pattern)

use clapi_core::capsules::multi_tenant_timeline::MultiTenantTimelineCapsule;
use clapi_core::capsules::timeline_aggregation_capsule::BucketGranularity;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::thread;

/// Benchmark: Single-threaded tenant lookup (get_or_insert)
///
/// Expected: <100ns (vs 500ns DashMap baseline)
fn bench_single_thread_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("p2_multi_tenant_lookup_single_thread");

    for tenant_count in [10, 100, 1000] {
        group.throughput(Throughput::Elements(tenant_count));

        group.bench_with_input(
            BenchmarkId::from_parameter(tenant_count),
            &tenant_count,
            |b, &tenant_count| {
                let mt = MultiTenantTimelineCapsule::new(BucketGranularity::Minute);

                b.iter(|| {
                    for i in 0..tenant_count {
                        let timeline = mt.get_timeline(i);
                        black_box(timeline);
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Multi-threaded tenant lookup (8 threads)
///
/// Expected: Linear scaling, <200ns per lookup
fn bench_multi_thread_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("p2_multi_tenant_lookup_8_threads");

    for tenant_count in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(tenant_count));

        group.bench_with_input(
            BenchmarkId::from_parameter(tenant_count),
            &tenant_count,
            |b, &tenant_count| {
                let mt = Arc::new(MultiTenantTimelineCapsule::new(BucketGranularity::Minute));

                b.iter(|| {
                    let handles: Vec<_> = (0..8)
                        .map(|thread_id| {
                            let mt_clone = Arc::clone(&mt);
                            let lookups_per_thread = tenant_count / 8;

                            thread::spawn(move || {
                                for i in 0..lookups_per_thread {
                                    let tenant_id = (thread_id * lookups_per_thread + i) as u64;
                                    let timeline = mt_clone.get_timeline(tenant_id);
                                    black_box(timeline);
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

/// Benchmark: High contention (16 threads, 1000 tenants)
///
/// Expected: 10-59× speedup vs DashMap at high contention
fn bench_high_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("p2_multi_tenant_high_contention");

    for thread_count in [4, 8, 16] {
        group.throughput(Throughput::Elements(1000));

        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            &thread_count,
            |b, &thread_count| {
                let mt = Arc::new(MultiTenantTimelineCapsule::new(BucketGranularity::Minute));

                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|_thread_id| {
                            let mt_clone = Arc::clone(&mt);

                            thread::spawn(move || {
                                // All threads access same 1000 tenants (high contention)
                                for i in 0..1000 {
                                    let timeline = mt_clone.get_timeline(i);
                                    black_box(timeline);
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

/// Benchmark: Realistic append workload
///
/// Expected: <600ns total (lookup + append)
fn bench_append_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("p2_multi_tenant_append_workload");

    for tenant_count in [10, 100, 1000] {
        group.throughput(Throughput::Elements(tenant_count));

        group.bench_with_input(
            BenchmarkId::from_parameter(tenant_count),
            &tenant_count,
            |b, &tenant_count| {
                let mt = Arc::new(MultiTenantTimelineCapsule::new(BucketGranularity::Minute));

                b.iter(|| {
                    for i in 0..tenant_count {
                        mt.append(i, 1000 + i).unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Memory usage scaling
///
/// Expected: <640MB @ 1000 tenants (same as DashMap)
fn bench_memory_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("p2_multi_tenant_memory");

    for tenant_count in [10, 100, 1000] {
        group.throughput(Throughput::Elements(tenant_count));

        group.bench_with_input(
            BenchmarkId::from_parameter(tenant_count),
            &tenant_count,
            |b, &tenant_count| {
                b.iter(|| {
                    let mt = MultiTenantTimelineCapsule::new(BucketGranularity::Minute);

                    for i in 0..tenant_count {
                        mt.append(i, 1000).unwrap();
                    }

                    let memory = mt.memory_usage_bytes();
                    black_box(memory);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_single_thread_lookup,
    bench_multi_thread_lookup,
    bench_high_contention,
    bench_append_workload,
    bench_memory_scaling
);

criterion_main!(benches);
