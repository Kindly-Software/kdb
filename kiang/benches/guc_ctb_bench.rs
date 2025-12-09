//! GuC CTB performance benchmarks
//!
//! Validates performance targets from "The Atomic Capsule" principles:
//! - Readiness check: <5ns (single atomic load)
//! - Slot reservation: <15ns (single CAS operation)
//! - State read: <10ns (two atomic loads)

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use kiang::guc_ctb::{GucCtbRingBuffer, GucCtbState, GucReadyCapsule};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Core Performance Benchmarks
// ============================================================================

/// Benchmark: Readiness check (has_space_for)
///
/// Target: <5ns per operation
/// This is the hot path for command submission gating.
fn bench_readiness_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("guc_ctb/readiness_check");
    group.throughput(Throughput::Elements(1));

    let capsule = GucReadyCapsule::with_capacity(4096);

    // Publish realistic state
    let state = GucCtbState {
        h2g_head: 0,
        h2g_tail: 1024,
        g2h_head: 0,
        g2h_tail: 512,
        capacity: 4096,
        pending_count: 5,
    };
    capsule.publish(state);

    group.bench_function("has_space_for_256", |b| {
        b.iter(|| black_box(capsule.has_space_for(black_box(256))));
    });

    group.bench_function("has_space_for_64", |b| {
        b.iter(|| black_box(capsule.has_space_for(black_box(64))));
    });

    group.bench_function("has_space_for_1024", |b| {
        b.iter(|| black_box(capsule.has_space_for(black_box(1024))));
    });

    group.finish();
}

/// Benchmark: State read (full capsule read)
///
/// Target: <10ns per operation
fn bench_state_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("guc_ctb/state_read");
    group.throughput(Throughput::Elements(1));

    let capsule = GucReadyCapsule::with_capacity(4096);

    let state = GucCtbState {
        h2g_head: 0,
        h2g_tail: 1024,
        g2h_head: 0,
        g2h_tail: 512,
        capacity: 4096,
        pending_count: 5,
    };
    capsule.publish(state);

    group.bench_function("read_full_state", |b| {
        b.iter(|| black_box(capsule.read()));
    });

    group.bench_function("h2g_head", |b| {
        b.iter(|| black_box(capsule.h2g_head()));
    });

    group.bench_function("h2g_tail", |b| {
        b.iter(|| black_box(capsule.h2g_tail()));
    });

    group.bench_function("pending_count", |b| {
        b.iter(|| black_box(capsule.pending_count()));
    });

    group.finish();
}

/// Benchmark: State publish (two-phase commit)
///
/// Target: <50ns per operation
fn bench_state_publish(c: &mut Criterion) {
    let mut group = c.benchmark_group("guc_ctb/state_publish");
    group.throughput(Throughput::Elements(1));

    let capsule = GucReadyCapsule::with_capacity(4096);

    let state = GucCtbState {
        h2g_head: 0,
        h2g_tail: 1024,
        g2h_head: 0,
        g2h_tail: 512,
        capacity: 4096,
        pending_count: 5,
    };

    group.bench_function("publish", |b| {
        b.iter(|| capsule.publish(black_box(state)));
    });

    group.finish();
}

/// Benchmark: Slot reservation (atomic CAS operation)
///
/// Target: <15ns per operation
fn bench_slot_reservation(c: &mut Criterion) {
    let mut group = c.benchmark_group("guc_ctb/slot_reservation");
    group.throughput(Throughput::Elements(1));

    // Allocate test buffers
    let h2g_buffer = vec![0u8; 4096];
    let g2h_buffer = vec![0u8; 4096];

    let ring_buffer = unsafe {
        GucCtbRingBuffer::new(
            h2g_buffer.as_ptr() as *mut u8,
            g2h_buffer.as_ptr() as *mut u8,
            4096,
        )
    };

    group.bench_function("reserve_64_bytes", |b| {
        b.iter(|| black_box(ring_buffer.reserve_h2g_slot(black_box(64))));
    });

    group.bench_function("reserve_256_bytes", |b| {
        b.iter(|| black_box(ring_buffer.reserve_h2g_slot(black_box(256))));
    });

    group.finish();
}

// ============================================================================
// Contention Benchmarks
// ============================================================================

/// Benchmark: Concurrent readers (scaling behavior)
fn bench_concurrent_readers(c: &mut Criterion) {
    let mut group = c.benchmark_group("guc_ctb/concurrent_readers");

    for thread_count in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            &thread_count,
            |b, &threads| {
                let capsule = Arc::new(GucReadyCapsule::with_capacity(4096));

                let state = GucCtbState {
                    h2g_head: 0,
                    h2g_tail: 1024,
                    g2h_head: 0,
                    g2h_tail: 512,
                    capacity: 4096,
                    pending_count: 5,
                };
                capsule.publish(state);

                b.iter(|| {
                    let mut handles = vec![];

                    for _ in 0..threads {
                        let reader = capsule.clone();
                        handles.push(thread::spawn(move || {
                            for _ in 0..1000 {
                                black_box(reader.has_space_for(256));
                            }
                        }));
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

/// Benchmark: Writer + multiple readers
fn bench_writer_reader_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("guc_ctb/writer_reader_contention");

    group.bench_function("1_writer_8_readers", |b| {
        let capsule = Arc::new(GucReadyCapsule::with_capacity(4096));

        b.iter(|| {
            let mut handles = vec![];

            // Writer thread
            let writer = capsule.clone();
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    let state = GucCtbState {
                        h2g_head: 0,
                        h2g_tail: i * 10,
                        g2h_head: 0,
                        g2h_tail: 0,
                        capacity: 4096,
                        pending_count: i as u16,
                    };
                    writer.publish(state);
                }
            }));

            // Reader threads
            for _ in 0..8 {
                let reader = capsule.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..100 {
                        black_box(reader.has_space_for(256));
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// Buffer Utilization Benchmarks
// ============================================================================

/// Benchmark: Buffer utilization calculations
fn bench_utilization_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("guc_ctb/utilization");

    let states = [
        (
            "empty",
            GucCtbState {
                h2g_head: 0,
                h2g_tail: 0,
                g2h_head: 0,
                g2h_tail: 0,
                capacity: 4096,
                pending_count: 0,
            },
        ),
        (
            "half_full",
            GucCtbState {
                h2g_head: 0,
                h2g_tail: 2048,
                g2h_head: 0,
                g2h_tail: 1024,
                capacity: 4096,
                pending_count: 0,
            },
        ),
        (
            "wrapped",
            GucCtbState {
                h2g_head: 3584,
                h2g_tail: 512,
                g2h_head: 0,
                g2h_tail: 0,
                capacity: 4096,
                pending_count: 0,
            },
        ),
    ];

    for (name, state) in states {
        group.bench_function(format!("h2g_utilization_{}", name), |b| {
            b.iter(|| black_box(state.h2g_utilization()));
        });

        group.bench_function(format!("g2h_utilization_{}", name), |b| {
            b.iter(|| black_box(state.g2h_utilization()));
        });

        group.bench_function(format!("has_space_{}", name), |b| {
            b.iter(|| black_box(state.has_h2g_space(256)));
        });
    }

    group.finish();
}

// ============================================================================
// Integration Benchmarks
// ============================================================================

/// Benchmark: Complete command submission flow
fn bench_command_submission_flow(c: &mut Criterion) {
    let mut group = c.benchmark_group("guc_ctb/command_submission");

    let h2g_buffer = vec![0u8; 4096];
    let g2h_buffer = vec![0u8; 4096];

    let ring_buffer = unsafe {
        GucCtbRingBuffer::new(
            h2g_buffer.as_ptr() as *mut u8,
            g2h_buffer.as_ptr() as *mut u8,
            4096,
        )
    };

    group.bench_function("full_submission_64bytes", |b| {
        b.iter(|| {
            // 1. Check readiness
            let state = ring_buffer.state();
            if state.is_some() {
                // 2. Reserve slot
                if let Some((offset, size)) = ring_buffer.reserve_h2g_slot(64) {
                    // 3. Simulate command write
                    black_box((offset, size));
                    // 4. Tail increment happens in reserve_h2g_slot
                }
            }
        });
    });

    group.finish();
}

/// Benchmark: G2H response processing
fn bench_g2h_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("guc_ctb/g2h_processing");

    let h2g_buffer = vec![0u8; 4096];
    let g2h_buffer = vec![0u8; 4096];

    let ring_buffer = unsafe {
        GucCtbRingBuffer::new(
            h2g_buffer.as_ptr() as *mut u8,
            g2h_buffer.as_ptr() as *mut u8,
            4096,
        )
    };

    group.bench_function("process_responses", |b| {
        b.iter(|| black_box(ring_buffer.process_g2h_responses()));
    });

    group.finish();
}

// ============================================================================
// Cache Behavior Benchmarks
// ============================================================================

/// Benchmark: Cache alignment effectiveness
fn bench_cache_alignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("guc_ctb/cache_alignment");

    // Single capsule (should fit in single cache line)
    let capsule = GucReadyCapsule::with_capacity(4096);
    let state = GucCtbState {
        h2g_head: 0,
        h2g_tail: 1024,
        g2h_head: 0,
        g2h_tail: 512,
        capacity: 4096,
        pending_count: 5,
    };
    capsule.publish(state);

    group.bench_function("single_capsule_read", |b| {
        b.iter(|| black_box(capsule.read()));
    });

    // Multiple capsules (test false sharing)
    let capsules: Vec<_> = (0..8)
        .map(|_| GucReadyCapsule::with_capacity(4096))
        .collect();

    for capsule in &capsules {
        capsule.publish(state);
    }

    group.bench_function("multiple_capsules_read", |b| {
        b.iter(|| {
            for capsule in &capsules {
                black_box(capsule.read());
            }
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark Groups
// ============================================================================

criterion_group!(
    benches,
    bench_readiness_check,
    bench_state_read,
    bench_state_publish,
    bench_slot_reservation,
    bench_concurrent_readers,
    bench_writer_reader_contention,
    bench_utilization_calculation,
    bench_command_submission_flow,
    bench_g2h_processing,
    bench_cache_alignment,
);

criterion_main!(benches);
