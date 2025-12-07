//! B32 Benchmarks for PerClientRateLimiterCapsule
//!
//! **Framework**: B32 (95% CI, 1000+ iterations, fair baseline)
//! **Tier**: T1 Atomic + T5 Streaming
//! **Target**: Validate +30ns overhead per request (vs global RateLimiterCapsule)
//!
//! **Test Groups**:
//! 1. Single-client token bucket operations (<30ns)
//! 2. Multi-client concurrent access (100 clients, fair allocation)
//! 3. Refill operations (streaming every 100ms)
//! 4. Contention scenarios (high concurrent load)
//! 5. Comparison: global vs per-client rate limiter

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Bring in the rate limiter types
use kdb_mcp::{RateLimiterCapsule, per_client_rate_limiter::*};

// ============================================================================
// Group 1: Single-Client Token Bucket Operations
// ============================================================================

fn bench_per_client_check_rate_limit_single_client(c: &mut Criterion) {
    let limiter = PerClientRateLimiterCapsule::new(1000 << 16, 2000 << 16, 100);
    let buckets = Arc::new(Mutex::new(HashMap::new()));

    c.bench_function("per_client_check_rate_limit_single", |b| {
        b.iter(|| {
            let limiter = black_box(&limiter);
            let buckets = black_box(&buckets);
            let _ = limiter.check_rate_limit(buckets, 1, 0, black_box(1 << 16));
        })
    });
}

fn bench_global_check_rate_limit_baseline(c: &mut Criterion) {
    let limiter = RateLimiterCapsule::with_rate(1000 << 16);

    c.bench_function("global_check_rate_limit_baseline", |b| {
        b.iter(|| {
            let limiter = black_box(&limiter);
            let _ = limiter.check(black_box(1 << 16));
        })
    });
}

fn bench_per_client_vs_global_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("per_client_vs_global");

    // Per-client rate limiter
    let per_client_limiter = PerClientRateLimiterCapsule::new(1000 << 16, 2000 << 16, 100);
    let per_client_buckets = Arc::new(Mutex::new(HashMap::new()));

    group.bench_function("per_client_single_request", |b| {
        b.iter(|| {
            let _ = per_client_limiter.check_rate_limit(
                &per_client_buckets,
                black_box(1),
                0,
                black_box(1 << 16),
            );
        })
    });

    // Global rate limiter
    let global_limiter = RateLimiterCapsule::with_rate(1000 << 16);

    group.bench_function("global_single_request", |b| {
        b.iter(|| {
            let _ = global_limiter.check(black_box(1 << 16));
        })
    });

    group.finish();
}

// ============================================================================
// Group 2: Multi-Client Concurrent Access
// ============================================================================

fn bench_multi_client_10(c: &mut Criterion) {
    let limiter = Arc::new(PerClientRateLimiterCapsule::new(10000 << 16, 20000 << 16, 100));
    let buckets = Arc::new(Mutex::new(HashMap::new()));

    c.bench_function("multi_client_10_clients_throughput", |b| {
        b.iter(|| {
            let barrier = Arc::new(std::sync::Barrier::new(10));
            let mut handles = vec![];

            for client_id in 0..10 {
                let l = limiter.clone();
                let b_clone = buckets.clone();
                let barrier_clone = barrier.clone();

                let handle = thread::spawn(move || {
                    barrier_clone.wait();
                    for _ in 0..100 {
                        let _ = l.check_rate_limit(&b_clone, client_id, 0, black_box(1 << 16));
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.join();
            }
        })
    });
}

fn bench_multi_client_100(c: &mut Criterion) {
    let limiter = Arc::new(PerClientRateLimiterCapsule::new(100000 << 16, 200000 << 16, 100));
    let buckets = Arc::new(Mutex::new(HashMap::new()));

    c.bench_function("multi_client_100_clients_throughput", |b| {
        b.iter(|| {
            let barrier = Arc::new(std::sync::Barrier::new(100));
            let mut handles = vec![];

            for client_id in 0..100 {
                let l = limiter.clone();
                let b_clone = buckets.clone();
                let barrier_clone = barrier.clone();

                let handle = thread::spawn(move || {
                    barrier_clone.wait();
                    for _ in 0..10 {
                        let _ = l.check_rate_limit(&b_clone, client_id, 0, black_box(1 << 16));
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.join();
            }
        })
    });
}

// ============================================================================
// Group 3: Refill Operations (Streaming)
// ============================================================================

fn bench_refill_tokens(c: &mut Criterion) {
    let limiter = PerClientRateLimiterCapsule::new(1000 << 16, 2000 << 16, 100);
    let buckets = Arc::new(Mutex::new(HashMap::new()));

    // Pre-populate buckets with 100 clients
    {
        let mut b = buckets.lock().unwrap();
        for i in 0..100 {
            b.insert(i, ClientTokenBucket::new(1000 << 16, 2000 << 16, 0));
        }
    }

    c.bench_function("refill_tokens_100_clients", |b| {
        b.iter(|| {
            let _ = limiter.refill_tokens(&buckets, black_box(100));
        })
    });
}

fn bench_refill_tokens_1000_clients(c: &mut Criterion) {
    let limiter = PerClientRateLimiterCapsule::new(1000 << 16, 2000 << 16, 100);
    let buckets = Arc::new(Mutex::new(HashMap::new()));

    // Pre-populate buckets with 1000 clients
    {
        let mut b = buckets.lock().unwrap();
        for i in 0..1000 {
            b.insert(i, ClientTokenBucket::new(1000 << 16, 2000 << 16, 0));
        }
    }

    c.bench_function("refill_tokens_1000_clients", |b| {
        b.iter(|| {
            let _ = limiter.refill_tokens(&buckets, black_box(100));
        })
    });
}

// ============================================================================
// Group 4: Contention Scenarios
// ============================================================================

fn bench_high_contention_single_client(c: &mut Criterion) {
    let limiter = Arc::new(PerClientRateLimiterCapsule::new(100000 << 16, 200000 << 16, 100));
    let buckets = Arc::new(Mutex::new(HashMap::new()));

    c.bench_function("high_contention_50_threads_single_client", |b| {
        b.iter(|| {
            let barrier = Arc::new(std::sync::Barrier::new(50));
            let mut handles = vec![];

            for _ in 0..50 {
                let l = limiter.clone();
                let b_clone = buckets.clone();
                let barrier_clone = barrier.clone();

                let handle = thread::spawn(move || {
                    barrier_clone.wait();
                    for _ in 0..10 {
                        let _ = l.check_rate_limit(&b_clone, black_box(1), 0, black_box(1 << 16));
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.join();
            }
        })
    });
}

fn bench_cas_convergence(c: &mut Criterion) {
    let bucket = Arc::new(ClientTokenBucket::new(100000 << 16, 100000 << 16, 0));

    c.bench_function("cas_convergence_100_threads", |b| {
        b.iter(|| {
            let barrier = Arc::new(std::sync::Barrier::new(100));
            let mut handles = vec![];

            for _ in 0..100 {
                let b_clone = bucket.clone();
                let barrier_clone = barrier.clone();

                let handle = thread::spawn(move || {
                    barrier_clone.wait();
                    let _ = b_clone.try_consume(black_box(1 << 16));
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.join();
            }
        })
    });
}

// ============================================================================
// Group 5: Statistics and Monitoring
// ============================================================================

fn bench_get_client_stats(c: &mut Criterion) {
    let limiter = PerClientRateLimiterCapsule::new(1000 << 16, 2000 << 16, 100);
    let buckets = Arc::new(Mutex::new(HashMap::new()));

    // Create a client
    let _ = limiter.check_rate_limit(&buckets, 1, 0, 1 << 16);

    c.bench_function("get_client_stats", |b| {
        b.iter(|| {
            let _ = limiter.get_client_stats(&buckets, black_box(1));
        })
    });
}

fn bench_get_all_client_stats(c: &mut Criterion) {
    let limiter = PerClientRateLimiterCapsule::new(1000 << 16, 2000 << 16, 100);
    let buckets = Arc::new(Mutex::new(HashMap::new()));

    // Create 100 clients
    for i in 0..100 {
        let _ = limiter.check_rate_limit(&buckets, i, 0, 1 << 16);
    }

    c.bench_function("get_all_client_stats_100_clients", |b| {
        b.iter(|| {
            let _ = limiter.get_all_client_stats(&buckets);
        })
    });
}

fn bench_cleanup_stale_clients(c: &mut Criterion) {
    let limiter = PerClientRateLimiterCapsule::new(1000 << 16, 2000 << 16, 100);
    let buckets = Arc::new(Mutex::new(HashMap::new()));

    // Create 1000 clients
    for i in 0..1000 {
        let _ = limiter.check_rate_limit(&buckets, i, 0, 1 << 16);
    }

    c.bench_function("cleanup_stale_clients_1000", |b| {
        b.iter(|| {
            let _ = limiter.cleanup_stale_clients(&buckets, black_box(1000), black_box(100));
        })
    });
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(3));
    targets =
        bench_per_client_check_rate_limit_single_client,
        bench_global_check_rate_limit_baseline,
        bench_per_client_vs_global_comparison,
        bench_multi_client_10,
        bench_multi_client_100,
        bench_refill_tokens,
        bench_refill_tokens_1000_clients,
        bench_high_contention_single_client,
        bench_cas_convergence,
        bench_get_client_stats,
        bench_get_all_client_stats,
        bench_cleanup_stale_clients
);

criterion_main!(benches);
