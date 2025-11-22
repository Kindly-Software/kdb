//! Ring Buffer Broadcast Benchmarks
//!
//! Compares RingBufferBroadcast vs tokio::broadcast for:
//! - Single producer, single consumer throughput
//! - Single producer, multi-consumer throughput
//! - Latency (P50, P99, P999)

use atomic_capsule::collections::channel as ring_channel;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::thread;
use tokio::sync::broadcast as tokio_broadcast;

/// Benchmark: Single producer, single consumer (send + recv)
fn bench_spsc_ring(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_ring_buffer");
    group.throughput(Throughput::Elements(1));

    group.bench_function("send_recv", |b| {
        let (tx, mut rx) = ring_channel();
        b.iter(|| {
            tx.send(black_box(42u64)).unwrap();
            black_box(rx.recv().unwrap());
        });
    });

    group.finish();
}

/// Benchmark: Single producer, single consumer with tokio::broadcast
fn bench_spsc_tokio(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_tokio_broadcast");
    group.throughput(Throughput::Elements(1));

    group.bench_function("send_recv", |b| {
        let (tx, mut rx) = tokio_broadcast::channel(16384);
        b.iter(|| {
            tx.send(black_box(42u64)).unwrap();
            black_box(rx.recv().unwrap());
        });
    });

    group.finish();
}

/// Benchmark: Multi-consumer broadcast (1 producer, 3 consumers)
fn bench_mpmc_ring(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpmc_ring_buffer");
    group.throughput(Throughput::Elements(3)); // 3 consumers

    group.bench_function("broadcast_3_consumers", |b| {
        let (tx, mut rx1) = ring_channel();
        let mut rx2 = tx.subscribe();
        let mut rx3 = tx.subscribe();

        b.iter(|| {
            tx.send(black_box(42u64)).unwrap();
            black_box(rx1.recv().unwrap());
            black_box(rx2.recv().unwrap());
            black_box(rx3.recv().unwrap());
        });
    });

    group.finish();
}

/// Benchmark: Multi-consumer broadcast with tokio::broadcast
fn bench_mpmc_tokio(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpmc_tokio_broadcast");
    group.throughput(Throughput::Elements(3)); // 3 consumers

    group.bench_function("broadcast_3_consumers", |b| {
        let (tx, mut rx1) = tokio_broadcast::channel(16384);
        let mut rx2 = tx.subscribe();
        let mut rx3 = tx.subscribe();

        b.iter(|| {
            tx.send(black_box(42u64)).unwrap();
            // tokio::broadcast may drop messages, so use blocking recv
            black_box(rx1.blocking_recv().unwrap());
            black_box(rx2.blocking_recv().unwrap());
            black_box(rx3.blocking_recv().unwrap());
        });
    });

    group.finish();
}

/// Benchmark: Throughput test (1M messages)
fn bench_throughput_ring(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_ring_buffer");
    const MESSAGES: usize = 1_000_000;
    group.throughput(Throughput::Elements(MESSAGES as u64));

    group.bench_function("1M_messages", |b| {
        b.iter(|| {
            let (tx, mut rx) = ring_channel();

            let sender = thread::spawn(move || {
                for i in 0..MESSAGES {
                    tx.send(black_box(i as u64)).unwrap();
                }
            });

            let receiver = thread::spawn(move || {
                for _ in 0..MESSAGES {
                    black_box(rx.recv().unwrap());
                }
            });

            sender.join().unwrap();
            receiver.join().unwrap();
        });
    });

    group.finish();
}

/// Benchmark: Throughput test with tokio::broadcast (1M messages)
fn bench_throughput_tokio(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_tokio_broadcast");
    const MESSAGES: usize = 1_000_000;
    group.throughput(Throughput::Elements(MESSAGES as u64));

    group.bench_function("1M_messages", |b| {
        b.iter(|| {
            let (tx, mut rx) = tokio_broadcast::channel(16384);

            let sender = thread::spawn(move || {
                for i in 0..MESSAGES {
                    tx.send(black_box(i as u64)).unwrap();
                }
            });

            let receiver = thread::spawn(move || {
                for _ in 0..MESSAGES {
                    match rx.blocking_recv() {
                        Ok(v) => black_box(v),
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            // Message dropped, continue
                            continue;
                        }
                        Err(_) => break,
                    };
                }
            });

            sender.join().unwrap();
            receiver.join().unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_spsc_ring,
    bench_spsc_tokio,
    bench_mpmc_ring,
    bench_mpmc_tokio,
    bench_throughput_ring,
    bench_throughput_tokio
);
criterion_main!(benches);
