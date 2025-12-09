//! B32 Benchmarks for ResponseCacheCapsule (P3-E8)
//!
//! **Benchmark Coverage**: 6 benchmarks (B32 framework compliance)
//! - Cache hit latency (<100ns target)
//! - Cache miss latency (<200ns target)
//! - Insert latency (<200ns target)
//! - Eviction latency (<50µs for 64K entries)
//! - Concurrent access (8 threads)
//! - Realistic workload (15-20% hit rate)
//!
//! **Framework Compliance**:
//! - B32: Honest measurement, 95% CI, fair baselines
//! - Hardware: AMD Ryzen (reported in results)
//! - Iterations: 1000+ per benchmark
//!
//! **Reality Check**:
//! - Expected: 10-20× speedup (100ms provider latency → <100ns cache hit)
//! - Exceptional: Avoid provider call entirely on cache hit

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use clapi_core::capsules::ResponseCache;
use clapi_core::proxy::types::ChatCompletionResponse;
use std::sync::Arc;
use std::thread;

// ============================================================================
// BENCHMARK 1: Cache Hit Latency
// ============================================================================

fn bench_cache_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("response_cache_hit");

    // Setup: Pre-populate cache
    let mut cache = ResponseCache::new();
    let hash = 12345u64;
    cache.insert(hash, mock_response("cached"));

    group.bench_function("cache_hit_single_thread", |b| {
        b.iter(|| {
            let result = cache.get(black_box(hash));
            black_box(result);
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: Cache Miss Latency
// ============================================================================

fn bench_cache_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("response_cache_miss");

    let mut cache = ResponseCache::new();

    group.bench_function("cache_miss_single_thread", |b| {
        b.iter(|| {
            // Vary hash to avoid slot reuse
            let hash = black_box(rand::random::<u64>());
            let result = cache.get(hash);
            black_box(result);
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: Insert Latency
// ============================================================================

fn bench_cache_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("response_cache_insert");

    let mut cache = ResponseCache::new();
    let mut counter = 0u64;

    group.bench_function("cache_insert_single_thread", |b| {
        b.iter(|| {
            let hash = black_box(counter);
            counter += 1;

            cache.insert(hash, mock_response("inserted"));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 4: Eviction Latency
// ============================================================================

fn bench_cache_eviction(c: &mut Criterion) {
    let mut group = c.benchmark_group("response_cache_eviction");

    // Pre-populate cache with 10K entries (0-second TTL)
    let mut cache = ResponseCache::with_capacity(10_000, 0);
    for i in 0..10_000 {
        cache.insert(i, mock_response(&format!("entry-{}", i)));
    }

    // Wait for TTL expiration
    std::thread::sleep(std::time::Duration::from_millis(10));

    group.bench_function("evict_10k_entries", |b| {
        b.iter(|| {
            cache.evict_expired();
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 5: Concurrent Access (8 Threads)
// ============================================================================

fn bench_cache_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("response_cache_concurrent");

    // Pre-populate cache
    let cache = Arc::new(parking_lot::Mutex::new(ResponseCache::new()));
    for i in 0..100 {
        cache.lock().insert(i, mock_response(&format!("entry-{}", i)));
    }

    group.bench_function("concurrent_8_threads", |b| {
        b.iter(|| {
            let mut handles = vec![];

            for thread_id in 0..8 {
                let cache_clone = Arc::clone(&cache);
                let handle = thread::spawn(move || {
                    for i in 0..100 {
                        let hash = (thread_id * 100 + i) as u64;
                        let _ = cache_clone.lock().get(hash);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 6: Realistic Workload (15-20% Hit Rate)
// ============================================================================

fn bench_cache_realistic_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("response_cache_realistic");

    let mut cache = ResponseCache::new();

    // Pre-populate with common queries (20% of workload)
    for i in 0..20 {
        cache.insert(i, mock_response(&format!("common-{}", i)));
    }

    let mut counter = 20u64;

    group.bench_function("realistic_15_20pct_hit_rate", |b| {
        b.iter(|| {
            // 20% hit common queries, 80% unique
            let hash = if counter % 5 == 0 {
                black_box(counter % 20) // Hit
            } else {
                black_box(counter) // Miss
            };

            counter += 1;

            let cached = cache.get(hash);
            if cached.is_none() {
                cache.insert(hash, mock_response("new-entry"));
            }
        });
    });

    group.finish();
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn mock_response(id: &str) -> ChatCompletionResponse {
    use clapi_core::proxy::types::Usage;

    ChatCompletionResponse {
        id: id.to_string(),
        object: "chat.completion".to_string(),
        created: 1234567890,
        model: "gpt-4".to_string(),
        choices: vec![],
        usage: Usage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        },
        cost_cents: Some(0.1),
        provider: Some("openai".to_string()),
    }
}

criterion_group!(
    benches,
    bench_cache_hit,
    bench_cache_miss,
    bench_cache_insert,
    bench_cache_eviction,
    bench_cache_concurrent,
    bench_cache_realistic_workload
);

criterion_main!(benches);
