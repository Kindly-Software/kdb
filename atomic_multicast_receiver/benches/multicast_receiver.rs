use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use atomic_multicast_receiver::*;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Benchmark packet processing latency to validate <1μs requirement
fn benchmark_packet_processing(c: &mut Criterion) {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let receiver = MulticastReceiver::<1024>::new(addr).unwrap();

    let mut group = c.benchmark_group("packet_processing");
    group.significance_level(0.05);
    group.sample_size(10000);
    group.measurement_time(Duration::from_secs(30));

    // Benchmark single packet processing
    group.bench_function("single_packet", |b| {
        b.iter(|| {
            // Simulate packet processing without actual network I/O
            let start = Instant::now();

            // Create a test packet
            let mut packet = MarketPacket {
                sequence: black_box(12345),
                timestamp_ns: black_box(start.elapsed().as_nanos() as u64),
                data: [0u8; 1400],
                len: black_box(100),
            };

            // Fill test data
            packet.data[0..4].copy_from_slice(&12345u32.to_be_bytes());

            // Process sequence
            let sequencer = PacketSequencer::new();
            let _valid = sequencer.process_sequence(packet.extract_sequence());

            // Try ring buffer operations
            let buffer: LockfreeRingBuffer<1024> = LockfreeRingBuffer::new();
            let success = buffer.try_push(packet);
            assert!(success);

            let _retrieved = buffer.try_pop();

            start.elapsed()
        });
    });

    // Benchmark ring buffer throughput
    for batch_size in [1, 10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("ring_buffer_batch", batch_size),
            batch_size,
            |b, &batch_size| {
                let buffer: LockfreeRingBuffer<2048> = LockfreeRingBuffer::new();
                let packet = MarketPacket::new();

                b.iter(|| {
                    let start = Instant::now();

                    // Push batch
                    for i in 0..batch_size {
                        let mut p = packet;
                        p.sequence = i as u32;
                        assert!(buffer.try_push(black_box(p)));
                    }

                    // Pop batch
                    for _ in 0..batch_size {
                        assert!(buffer.try_pop().is_some());
                    }

                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark atomic statistics performance
fn benchmark_atomic_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("atomic_stats");
    group.significance_level(0.05);
    group.sample_size(10000);

    let stats = AtomicStats::new();

    group.bench_function("record_packet", |b| {
        b.iter(|| {
            stats.record_packet(black_box(100), black_box(500));
        });
    });

    group.bench_function("snapshot", |b| {
        b.iter(|| {
            black_box(stats.snapshot())
        });
    });

    group.finish();
}

/// Benchmark packet sequencer performance
fn benchmark_packet_sequencer(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_sequencer");
    group.significance_level(0.05);
    group.sample_size(10000);

    let sequencer = PacketSequencer::new();

    group.bench_function("process_sequence_normal", |b| {
        let mut seq = 1u32;
        b.iter(|| {
            let result = sequencer.process_sequence(black_box(seq));
            seq += 1;
            black_box(result)
        });
    });

    group.bench_function("process_sequence_gap", |b| {
        let mut seq = 1u32;
        b.iter(|| {
            seq += 2; // Create gaps
            let result = sequencer.process_sequence(black_box(seq));
            black_box(result)
        });
    });

    group.finish();
}

/// End-to-end benchmark simulating real market data processing
fn benchmark_market_data_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_data_simulation");
    group.significance_level(0.05);
    group.sample_size(1000);
    group.measurement_time(Duration::from_secs(60));

    // Simulate processing 1000 packets
    group.bench_function("process_1000_packets", |b| {
        b.iter(|| {
            let buffer: LockfreeRingBuffer<2048> = LockfreeRingBuffer::new();
            let sequencer = PacketSequencer::new();
            let stats = AtomicStats::new();

            let start_time = Instant::now();

            // Simulate receiving and processing 1000 market data packets
            for i in 1..=1000u32 {
                let process_start = Instant::now();

                // Create realistic market packet
                let mut packet = MarketPacket::new();
                packet.sequence = i;
                packet.len = 64; // Typical market data packet size
                packet.timestamp_ns = process_start.elapsed().as_nanos() as u64;

                // Write sequence number to packet data
                packet.data[0..4].copy_from_slice(&i.to_be_bytes());

                // Process sequence
                let _valid = sequencer.process_sequence(packet.extract_sequence());

                // Store in buffer
                if !buffer.try_push(packet) {
                    panic!("Buffer full during simulation");
                }

                // Record stats
                let processing_time = process_start.elapsed().as_nanos() as u64;
                stats.record_packet(packet.len as u64, processing_time);

                // Consume packet
                let _consumed = buffer.try_pop().unwrap();

                black_box(processing_time);
            }

            let total_time = start_time.elapsed();

            // Verify performance targets
            let final_stats = stats.snapshot();
            assert!(final_stats.avg_latency_ns < 1000,
                "Average latency {}ns exceeds 1μs target", final_stats.avg_latency_ns);

            total_time
        });
    });

    group.finish();
}

/// Benchmark memory utilization and cache performance
fn benchmark_memory_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_performance");
    group.significance_level(0.05);
    group.sample_size(1000);

    // Test different buffer sizes for cache performance
    for buffer_size in [256, 1024, 4096, 16384].iter() {
        group.bench_with_input(
            BenchmarkId::new("buffer_utilization", buffer_size),
            buffer_size,
            |b, &size| {
                match size {
                    256 => {
                        let buffer: LockfreeRingBuffer<256> = LockfreeRingBuffer::new();
                        b.iter(|| black_box(buffer.utilization()));
                    }
                    1024 => {
                        let buffer: LockfreeRingBuffer<1024> = LockfreeRingBuffer::new();
                        b.iter(|| black_box(buffer.utilization()));
                    }
                    4096 => {
                        let buffer: LockfreeRingBuffer<4096> = LockfreeRingBuffer::new();
                        b.iter(|| black_box(buffer.utilization()));
                    }
                    16384 => {
                        let buffer: LockfreeRingBuffer<16384> = LockfreeRingBuffer::new();
                        b.iter(|| black_box(buffer.utilization()));
                    }
                    _ => unreachable!(),
                }
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent access patterns
fn benchmark_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");
    group.significance_level(0.05);
    group.sample_size(100);

    group.bench_function("multi_threaded_ring_buffer", |b| {
        use std::sync::Arc;
        use std::thread;

        b.iter(|| {
            let buffer = Arc::new(LockfreeRingBuffer::<1024>::new());
            let packet = MarketPacket::new();

            let buffer_producer = buffer.clone();
            let buffer_consumer = buffer.clone();

            let producer = thread::spawn(move || {
                for i in 0..500 {
                    let mut p = packet;
                    p.sequence = i;

                    while !buffer_producer.try_push(p) {
                        std::hint::spin_loop();
                    }
                }
            });

            let consumer = thread::spawn(move || {
                let mut consumed = 0;
                while consumed < 500 {
                    if buffer_consumer.try_pop().is_some() {
                        consumed += 1;
                    } else {
                        std::hint::spin_loop();
                    }
                }
            });

            producer.join().unwrap();
            consumer.join().unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_packet_processing,
    benchmark_atomic_stats,
    benchmark_packet_sequencer,
    benchmark_market_data_simulation,
    benchmark_memory_performance,
    benchmark_concurrent_access
);

criterion_main!(benches);