//! Predictive Cache Benchmarks - B32 Framework
//!
//! **B32 Compliance**:
//! - Fair baselines: Reactive cache (LRU only) vs Predictive cache
//! - Statistical rigor: 1000+ iterations, report mean/median/p99
//! - Honest claims: 10-50% typical improvement (not marketing hype)
//! - Reproducibility: All benchmarks committed with results
//!
//! # Performance Targets
//!
//! - **Pattern learning**: <200ns per record_request()
//! - **Prediction query**: <100ns per get_predictions()
//! - **Prefetch hit rate**: 30-50% (under realistic workload)
//! - **False positive rate**: <10% (wasted prefetches)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use clapi_core::capsules::pattern_learner::PatternLearner256;
use clapi_core::cache::{LruCache, CacheConfig, PredictivePrefetchCache};
use std::sync::Arc;

// ============================================================================
// B32: PatternLearner256 Microbenchmarks
// ============================================================================

fn bench_pattern_learner_record_request(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_learner/record_request");
    group.throughput(Throughput::Elements(1));

    let learner = PatternLearner256::new();

    // Warmup: Build some correlations
    for i in 0..100 {
        learner.record_request(i);
    }

    group.bench_function("single_request", |b| {
        let mut counter = 100u64;
        b.iter(|| {
            learner.record_request(black_box(counter));
            counter += 1;
        })
    });

    group.finish();
}

fn bench_pattern_learner_get_predictions(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_learner/get_predictions");
    group.throughput(Throughput::Elements(1));

    let learner = PatternLearner256::new();

    // Build strong A→B correlation
    for _ in 0..100 {
        learner.record_request(0x1111_1111_1111_1111);
        learner.record_request(0x2222_2222_2222_2222);
    }

    group.bench_function("query_with_results", |b| {
        b.iter(|| {
            let predictions = learner.get_predictions(black_box(0x1111_1111_1111_1111));
            black_box(predictions);
        })
    });

    group.bench_function("query_no_results", |b| {
        b.iter(|| {
            let predictions = learner.get_predictions(black_box(0xFFFF_FFFF_FFFF_FFFF));
            black_box(predictions);
        })
    });

    group.finish();
}

fn bench_pattern_learner_concurrent_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_learner/concurrent");
    group.throughput(Throughput::Elements(1));

    use std::sync::Arc;
    use std::thread;

    for num_threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads", num_threads)),
            &num_threads,
            |b, &threads| {
                b.iter(|| {
                    let learner = Arc::new(PatternLearner256::new());
                    let mut handles = vec![];

                    for thread_id in 0..threads {
                        let learner_clone = Arc::clone(&learner);
                        let handle = thread::spawn(move || {
                            for i in 0..100 {
                                let hash = ((thread_id as u64) << 48) | ((i as u64) << 32);
                                learner_clone.record_request(hash);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(learner);
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// B32: Predictive Cache Comparison (Reactive vs Predictive)
// ============================================================================

fn bench_cache_reactive_vs_predictive(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache/reactive_vs_predictive");
    group.throughput(Throughput::Elements(1));

    // Realistic workload: 100 unique requests with temporal patterns
    // Pattern: A→B→C→D→A (repeating sequence)
    let pattern = vec![
        "request_A",
        "request_B",
        "request_C",
        "request_D",
    ];

    // Baseline: Reactive cache (LRU only)
    group.bench_function("reactive_cache_lru", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();

        b.iter(|| {
            rt.block_on(async {
                let config = CacheConfig {
                    max_entries: 100,
                    default_ttl_secs: 60,
                };
                let cache = Arc::new(LruCache::new(config));

                // Simulate 100 requests following pattern
                for i in 0..100 {
                    let request = pattern[i % pattern.len()];
                    let hash = atomic_capsule::hash::const_fast_hash(request.as_bytes());

                    // Try to get from cache
                    let result = cache.get(hash);

                    // If miss, simulate fetch and insert
                    if result.is_err() {
                        let response = format!("response_{}", request);
                        let entry = clapi_core::cache::CacheEntry {
                            hash,
                            response,
                            timestamp_ns: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_nanos() as u64,
                        };
                        let _ = cache.insert(entry);
                    }
                }

                black_box(cache);
            })
        })
    });

    // Experimental: Predictive cache (LRU + pattern learning)
    group.bench_function("predictive_cache", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();

        b.iter(|| {
            rt.block_on(async {
                let config = CacheConfig {
                    max_entries: 100,
                    default_ttl_secs: 60,
                };
                let cache = Arc::new(LruCache::new(config));
                let learner = Arc::new(PatternLearner256::new());
                let pred_cache = PredictivePrefetchCache::new(cache, learner);

                // Simulate 100 requests following pattern
                for i in 0..100 {
                    let request = pattern[i % pattern.len()];

                    let _ = pred_cache
                        .get_or_fetch(request, || async {
                            Ok(format!("response_{}", request))
                        })
                        .await;
                }

                black_box(pred_cache);
            })
        })
    });

    group.finish();
}

// ============================================================================
// B32: Prefetch Hit Rate Analysis
// ============================================================================

fn bench_prefetch_hit_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache/prefetch_hit_rate");

    // Measure prefetch effectiveness under different pattern strengths
    for pattern_strength in [50, 70, 90] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}%_pattern", pattern_strength)),
            &pattern_strength,
            |b, &strength| {
                let rt = tokio::runtime::Runtime::new().unwrap();

                b.iter(|| {
                    rt.block_on(async {
                        let config = CacheConfig {
                            max_entries: 100,
                            default_ttl_secs: 60,
                        };
                        let cache = Arc::new(LruCache::new(config));
                        let learner = Arc::new(PatternLearner256::new());
                        let pred_cache = PredictivePrefetchCache::new(cache, learner);

                        // Build pattern: A→B with 'strength'% probability
                        for _ in 0..100 {
                            pred_cache
                                .get_or_fetch("request_A", || async {
                                    Ok("response_A".to_string())
                                })
                                .await
                                .unwrap();

                            // Follow pattern with 'strength'% probability
                            let follow_pattern = (rand::random::<u8>() as usize) < (strength * 256 / 100);
                            let next_request = if follow_pattern {
                                "request_B"
                            } else {
                                "request_C"
                            };

                            pred_cache
                                .get_or_fetch(next_request, || async {
                                    Ok(format!("response_{}", next_request))
                                })
                                .await
                                .unwrap();
                        }

                        // Measure prefetch stats
                        let stats = pred_cache.get_prefetch_stats();
                        black_box(stats);
                    })
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// B32: Memory Overhead
// ============================================================================

fn bench_memory_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_learner/memory");

    group.bench_function("capsule_size", |b| {
        b.iter(|| {
            let learner = PatternLearner256::new();
            let size = std::mem::size_of_val(&learner);
            black_box(size);
        })
    });

    group.bench_function("stats_query", |b| {
        let learner = PatternLearner256::new();

        // Build some correlations
        for _ in 0..100 {
            learner.record_request(0x1111_1111_1111_1111);
            learner.record_request(0x2222_2222_2222_2222);
        }

        b.iter(|| {
            let stats = learner.get_stats();
            black_box(stats);
        })
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_pattern_learner_record_request,
    bench_pattern_learner_get_predictions,
    bench_pattern_learner_concurrent_updates,
    bench_cache_reactive_vs_predictive,
    bench_prefetch_hit_rate,
    bench_memory_overhead,
);

criterion_main!(benches);
