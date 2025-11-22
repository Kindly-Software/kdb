//! PacketBufferConst Benchmark Suite
//!
//! Performance validation for zero-allocation const-generic packet buffer
//! against runtime-validated alternatives.
//!
//! **Target**: 10-50× speedup via allocation elimination (EXCEPTIONAL tier)

use atomic_capsule::network::PacketBufferConst;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_enqueue_latency(c: &mut Criterion) {
    c.bench_function("packet_buffer_enqueue_1500", |b| {
        let buf: PacketBufferConst<1500, 256> = PacketBufferConst::new();
        let packet = vec![1u8; 1000];

        b.iter(|| {
            let _ = buf.enqueue(black_box(&packet));
            // Dequeue to keep buffer from filling
            let _ = buf.dequeue();
        });
    });

    c.bench_function("packet_buffer_enqueue_9000", |b| {
        let buf: PacketBufferConst<9000, 256> = PacketBufferConst::new();
        let packet = vec![2u8; 5000];

        b.iter(|| {
            let _ = buf.enqueue(black_box(&packet));
            let _ = buf.dequeue();
        });
    });
}

fn bench_dequeue_latency(c: &mut Criterion) {
    c.bench_function("packet_buffer_dequeue_1500", |b| {
        let buf: PacketBufferConst<1500, 256> = PacketBufferConst::new();
        let packet = vec![1u8; 1000];
        let _ = buf.enqueue(&packet);

        b.iter(|| {
            let _ = buf.dequeue();
            let _ = buf.enqueue(black_box(&packet));
        });
    });
}

fn bench_throughput_1m_packets(c: &mut Criterion) {
    c.bench_function("packet_buffer_1m_packets_jumbo", |b| {
        let buf: PacketBufferConst<9000, 256> = PacketBufferConst::new();
        let packet = vec![3u8; 8000];
        let mut count = 0usize;

        b.iter(|| {
            match buf.enqueue(black_box(&packet)) {
                Ok(_) => count += 1,
                Err(_) => {
                    // Drain buffer
                    while buf.dequeue().is_some() {
                        count += 1;
                    }
                }
            }
        });
    });
}

fn bench_fill_and_drain(c: &mut Criterion) {
    c.bench_function("packet_buffer_fill_drain_256", |b| {
        let buf: PacketBufferConst<1500, 256> = PacketBufferConst::new();
        let packet = vec![4u8; 1500];

        b.iter(|| {
            // Fill to 250 packets
            for _ in 0..250 {
                let _ = buf.enqueue(black_box(&packet));
            }

            // Drain all
            while buf.dequeue().is_some() {}
        });
    });
}

fn bench_capacity_info(c: &mut Criterion) {
    c.bench_function("packet_buffer_capacity_check", |b| {
        let buf: PacketBufferConst<1500, 256> = PacketBufferConst::new();

        b.iter(|| {
            let _ = black_box(buf.capacity());
            let _ = black_box(buf.len());
            let _ = black_box(buf.is_empty());
            let _ = black_box(buf.is_full());
        });
    });
}

criterion_group!(
    benches,
    bench_enqueue_latency,
    bench_dequeue_latency,
    bench_throughput_1m_packets,
    bench_fill_and_drain,
    bench_capacity_info
);

criterion_main!(benches);
