//! Network Layer Benchmarks
//!
//! ## B32 Framework Validation
//!
//! - Transaction pool operations (insert, lookup, remove)
//! - Circuit breaker checks
//! - Gossip routing decisions
//! - Mempool statistics

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use kindly_network::{
    AtomicTransactionPool, PoolConfig,
    GossipCapsule, GossipMessage,
    MempoolStats,
};
use kindly_core::AtomicTransactionCapsule;
use std::sync::Arc;

fn bench_pool_insert(c: &mut Criterion) {
    let pool = AtomicTransactionPool::new(PoolConfig {
        max_size: 1_000_000,
        ..Default::default()
    });

    c.bench_function("pool_insert", |b| {
        let mut counter = 0u32;
        b.iter(|| {
            let mut tx_hash = [0u8; 32];
            tx_hash[0..4].copy_from_slice(&counter.to_le_bytes());
            counter = counter.wrapping_add(1);

            let tx_capsule = Arc::new(AtomicTransactionCapsule::new());
            pool.insert(black_box(tx_hash), black_box(tx_capsule))
        });
    });
}

fn bench_pool_lookup(c: &mut Criterion) {
    let pool = AtomicTransactionPool::new(PoolConfig::default());

    // Pre-populate pool
    for i in 0..1000 {
        let mut tx_hash = [0u8; 32];
        tx_hash[0..4].copy_from_slice(&i.to_le_bytes());
        let tx_capsule = Arc::new(AtomicTransactionCapsule::new());
        pool.insert(tx_hash, tx_capsule).unwrap();
    }

    c.bench_function("pool_lookup", |b| {
        let mut counter = 0u32;
        b.iter(|| {
            let mut tx_hash = [0u8; 32];
            tx_hash[0..4].copy_from_slice(&(counter % 1000).to_le_bytes());
            counter = counter.wrapping_add(1);

            pool.get(black_box(&tx_hash))
        });
    });
}

fn bench_pool_remove(c: &mut Criterion) {
    c.bench_function("pool_remove", |b| {
        b.iter_batched(
            || {
                let pool = AtomicTransactionPool::new(PoolConfig::default());
                let tx_hash = [1u8; 32];
                let tx_capsule = Arc::new(AtomicTransactionCapsule::new());
                pool.insert(tx_hash, tx_capsule).unwrap();
                (pool, tx_hash)
            },
            |(pool, tx_hash)| {
                pool.remove(black_box(&tx_hash))
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_pool_health(c: &mut Criterion) {
    let pool = AtomicTransactionPool::new(PoolConfig::default());

    c.bench_function("pool_health", |b| {
        b.iter(|| {
            pool.health()
        });
    });
}

fn bench_gossip_publish(c: &mut Criterion) {
    let capsule = GossipCapsule::new();

    c.bench_function("gossip_publish", |b| {
        let mut counter = 0u8;
        b.iter(|| {
            let mut msg_hash = [0u8; 32];
            msg_hash[0] = counter;
            counter = counter.wrapping_add(1);

            let msg = GossipMessage {
                msg_hash,
                hop_count: 0,
                ttl: 8,
                payload: vec![1, 2, 3],
            };

            capsule.publish(black_box(&msg))
        });
    });
}

fn bench_gossip_read(c: &mut Criterion) {
    let capsule = GossipCapsule::new();

    // Pre-publish message
    let msg = GossipMessage {
        msg_hash: [1u8; 32],
        hop_count: 0,
        ttl: 8,
        payload: vec![],
    };
    capsule.publish(&msg).unwrap();

    c.bench_function("gossip_read", |b| {
        b.iter(|| {
            capsule.read()
        });
    });
}

fn bench_gossip_duplicate_check(c: &mut Criterion) {
    let capsule = GossipCapsule::new();

    let msg = GossipMessage {
        msg_hash: [2u8; 32],
        hop_count: 0,
        ttl: 8,
        payload: vec![],
    };
    capsule.publish(&msg).unwrap();
    let generation = capsule.generation();

    c.bench_function("gossip_duplicate_check", |b| {
        b.iter(|| {
            capsule.is_duplicate(black_box(generation))
        });
    });
}

fn bench_gossip_hop_increment(c: &mut Criterion) {
    c.bench_function("gossip_hop_increment", |b| {
        b.iter_batched(
            || {
                let capsule = GossipCapsule::new();
                let msg = GossipMessage {
                    msg_hash: [3u8; 32],
                    hop_count: 0,
                    ttl: 8,
                    payload: vec![],
                };
                capsule.publish(&msg).unwrap();
                capsule
            },
            |capsule| {
                capsule.increment_hop()
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_stats_record(c: &mut Criterion) {
    let stats = MempoolStats::new();

    c.bench_function("stats_record_received", |b| {
        b.iter(|| {
            stats.record_received()
        });
    });

    c.bench_function("stats_record_accepted", |b| {
        b.iter(|| {
            stats.record_accepted(black_box(100))
        });
    });
}

fn bench_stats_snapshot(c: &mut Criterion) {
    let stats = MempoolStats::new();

    // Record some data
    for _ in 0..1000 {
        stats.record_received();
        stats.record_accepted(100);
    }

    c.bench_function("stats_snapshot", |b| {
        b.iter(|| {
            stats.snapshot()
        });
    });
}

fn bench_concurrent_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_pool");

    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("insert", thread_count),
            thread_count,
            |b, &thread_count| {
                let pool = Arc::new(AtomicTransactionPool::new(PoolConfig {
                    max_size: 1_000_000,
                    ..Default::default()
                }));

                b.iter(|| {
                    let mut handles = vec![];

                    for thread_id in 0..thread_count {
                        let pool_clone = pool.clone();
                        let handle = std::thread::spawn(move || {
                            for i in 0..100 {
                                let mut tx_hash = [0u8; 32];
                                tx_hash[0] = thread_id as u8;
                                tx_hash[1] = i;
                                let tx_capsule = Arc::new(AtomicTransactionCapsule::new());
                                let _ = pool_clone.insert(tx_hash, tx_capsule);
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

criterion_group!(
    benches,
    bench_pool_insert,
    bench_pool_lookup,
    bench_pool_remove,
    bench_pool_health,
    bench_gossip_publish,
    bench_gossip_read,
    bench_gossip_duplicate_check,
    bench_gossip_hop_increment,
    bench_stats_record,
    bench_stats_snapshot,
    bench_concurrent_pool,
);

criterion_main!(benches);
