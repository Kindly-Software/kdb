//! Cache Benchmarks (B32 Framework)
//!
//! # B32 Benchmarking Standards
//!
//! **Baseline**: Direct API call (100ms latency)
//! **Target**: <100ns cache hit (1,000,000× speedup)
//! **Fair Comparison**: Cache hit vs cache miss + insertion
//! **Statistical Rigor**: 95% CI, 1000+ iterations

use atomic_capsule::hash::const_fast_hash;
use clapi_core::cache::{CacheConfig, LruCache};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;

// ============================================================================
// Cache Hit Latency (Hot Path)
// ============================================================================

fn bench_cache_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_hit");
    group.throughput(Throughput::Elements(1));

    let cache = LruCache::default();

    // Prewarm cache with 1000 entries
    for i in 0..1000 {
        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        cache.insert(hash, format!("response_{}", i)).unwrap();
    }

    group.bench_function("cache_hit_warm", |b| {
        b.iter(|| {
            let hash = const_fast_hash(b"request_500"); // Middle of cache
            let result = cache.get(black_box(hash));
            black_box(result)
        });
    });

    group.finish();
}

// ============================================================================
// Cache Miss + Insertion Latency
// ============================================================================

fn bench_cache_miss_and_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_miss");
    group.throughput(Throughput::Elements(1));

    let cache = LruCache::default();

    let mut counter = 0;
    group.bench_function("cache_miss_and_insert", |b| {
        b.iter(|| {
            counter += 1;
            let hash = const_fast_hash(format!("request_{}", counter).as_bytes());
            let response = format!("response_{}", counter);

            // Miss + Insert
            let _ = cache.get(black_box(hash)).or_else(|_| {
                cache.insert(hash, response)?;
                cache.get(hash)
            });
        });
    });

    group.finish();
}

// ============================================================================
// LRU Eviction Latency
// ============================================================================

fn bench_lru_eviction(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_eviction");

    for size in [100, 1000, 10_000].iter() {
        let mut config = CacheConfig::default();
        config.max_entries = *size;

        let cache = LruCache::new(config);

        // Fill cache completely
        for i in 0..*size {
            let hash = const_fast_hash(format!("request_{}", i).as_bytes());
            cache.insert(hash, format!("response_{}", i)).unwrap();
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                cache.evict_lru().unwrap();
            });
        });
    }

    group.finish();
}

// ============================================================================
// Concurrent Cache Access (Scalability)
// ============================================================================

fn bench_concurrent_cache_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");

    let cache = Arc::new(LruCache::default());

    // Prewarm cache
    for i in 0..1000 {
        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        cache.insert(hash, format!("response_{}", i)).unwrap();
    }

    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &threads| {
                b.iter(|| {
                    let mut handles = vec![];

                    for _ in 0..threads {
                        let cache_clone = Arc::clone(&cache);
                        let handle = std::thread::spawn(move || {
                            for i in 0..100 {
                                let hash = const_fast_hash(format!("request_{}", i).as_bytes());
                                let _ = cache_clone.get(black_box(hash));
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
// Hit Rate with Realistic Workload
// ============================================================================

fn bench_realistic_workload_90_percent_duplicates(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_workload");
    group.throughput(Throughput::Elements(1000));

    let cache = LruCache::default();

    // Prewarm with 100 entries
    for i in 0..100 {
        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        cache.insert(hash, format!("response_{}", i)).unwrap();
    }

    group.bench_function("90_percent_duplicates", |b| {
        let mut counter = 100;
        b.iter(|| {
            for _ in 0..1000 {
                let i = if rand::random::<f64>() < 0.9 {
                    // 90% duplicates (cache hits)
                    rand::random::<usize>() % 100
                } else {
                    // 10% new entries (cache misses)
                    counter += 1;
                    counter
                };

                let hash = const_fast_hash(format!("request_{}", i).as_bytes());
                let _ = cache.get(black_box(hash)).or_else(|_| {
                    cache.insert(hash, format!("response_{}", i))?;
                    cache.get(hash)
                });
            }
        });
    });

    group.finish();
}

// ============================================================================
// Memory Footprint (Informational)
// ============================================================================

fn bench_memory_footprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_footprint");

    for size in [100, 1000, 10_000].iter() {
        let mut config = CacheConfig::default();
        config.max_entries = *size;

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let cache = LruCache::new(CacheConfig {
                    max_entries: size,
                    default_ttl_ns: 3_600_000_000_000,
                });

                // Fill cache
                for i in 0..size {
                    let hash = const_fast_hash(format!("request_{}", i).as_bytes());
                    cache.insert(hash, format!("response_{}", i)).unwrap();
                }

                black_box(cache);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_cache_hit,
    bench_cache_miss_and_insert,
    bench_lru_eviction,
    bench_concurrent_cache_access,
    bench_realistic_workload_90_percent_duplicates,
    bench_memory_footprint,
);

criterion_main!(benches);
