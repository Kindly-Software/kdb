//! B32 Benchmarks - AuditCompressionCapsule
//!
//! **Framework**: B32 (honest benchmarking with fair baselines)
//! **Capsule**: AuditCompressionCapsule (T0+T5)
//! **Baseline**: Uncompressed sequential audit log (Vec<AuditEvent>)
//!
//! # Benchmark Groups
//!
//! 1. **Append Latency**: Single-threaded append operations
//! 2. **Compression Ratio**: Size reduction validation
//! 3. **Verification Time**: Hash chain integrity checks
//! 4. **Concurrent Append**: Multi-threaded throughput
//!
//! # Performance Targets (B32)
//!
//! - Append: <100ns (lockfree atomic + streaming LZ4)
//! - Compression: 10-50× size reduction
//! - Verification: O(log N) Merkle tree vs O(N) linear
//! - Concurrent: 10M+ events/sec @ 22 cores

#![cfg(feature = "audit-compression")]

use atomic_capsule::auditable::{AuditCompressionCapsule, AuditEvent, AuditEventType};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::thread;

// ============================================================================
// BASELINE: UNCOMPRESSED SEQUENTIAL LOG
// ============================================================================

/// Baseline: Uncompressed Vec<AuditEvent> (sequential write)
///
/// Fair baseline characteristics:
/// - Same data structure (AuditEvent)
/// - Sequential writes (no concurrency overhead)
/// - No compression (pure write latency)
/// - Mutex-protected for thread safety
struct UncompressedAuditLog {
    events: std::sync::Mutex<Vec<AuditEvent>>,
}

impl UncompressedAuditLog {
    fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::with_capacity(16384)),
        }
    }

    fn append(&self, event: AuditEvent) {
        let mut events = self.events.lock().unwrap();
        events.push(event);
    }

    fn len(&self) -> usize {
        self.events.lock().unwrap().len()
    }
}

// ============================================================================
// BENCHMARK 1: APPEND LATENCY (SINGLE-THREADED)
// ============================================================================

fn bench_append_latency_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("append_latency_baseline");
    group.throughput(Throughput::Elements(1));

    group.bench_function("uncompressed_vec", |b| {
        let log = UncompressedAuditLog::new();
        let mut i = 0;

        b.iter(|| {
            let event = AuditEvent::new(
                AuditEventType::FileAdd,
                1,
                &format!("/data/file{}.txt", i),
                "add",
            );
            log.append(black_box(event));
            i += 1;
        });
    });

    group.finish();
}

fn bench_append_latency_optimized(c: &mut Criterion) {
    let mut group = c.benchmark_group("append_latency_optimized");
    group.throughput(Throughput::Elements(1));

    group.bench_function("audit_compression_capsule", |b| {
        let capsule = AuditCompressionCapsule::new();
        let mut i = 0;

        b.iter(|| {
            let event = AuditEvent::new(
                AuditEventType::FileAdd,
                1,
                &format!("/data/file{}.txt", i),
                "add",
            );
            capsule.append(black_box(event)).unwrap();
            i += 1;
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: BATCH APPEND (1000 EVENTS)
// ============================================================================

fn bench_batch_append_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_append_baseline");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("uncompressed_vec_1000", |b| {
        b.iter(|| {
            let log = UncompressedAuditLog::new();
            for i in 0..1000 {
                let event = AuditEvent::new(
                    AuditEventType::FileAdd,
                    1,
                    &format!("/data/file{}.txt", i),
                    "add",
                );
                log.append(event);
            }
            black_box(log.len());
        });
    });

    group.finish();
}

fn bench_batch_append_optimized(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_append_optimized");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("audit_compression_capsule_1000", |b| {
        b.iter(|| {
            let capsule = AuditCompressionCapsule::new();
            for i in 0..1000 {
                let event = AuditEvent::new(
                    AuditEventType::FileAdd,
                    1,
                    &format!("/data/file{}.txt", i),
                    "add",
                );
                capsule.append(event).unwrap();
            }
            black_box(capsule.get_stats());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: VERIFICATION TIME
// ============================================================================

fn bench_verification_time(c: &mut Criterion) {
    let mut group = c.benchmark_group("verification_time");

    for size in [100, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("verify_merkle_range", size),
            size,
            |b, &size| {
                let capsule = AuditCompressionCapsule::new();

                // Pre-populate with events
                for i in 0..size {
                    let event = AuditEvent::new(
                        AuditEventType::FileAdd,
                        1,
                        &format!("/data/file{}.txt", i),
                        "add",
                    );
                    capsule.append(event).unwrap();
                }

                b.iter(|| {
                    let result = capsule.verify_merkle_range(0, (size - 1) as u64);
                    black_box(result.unwrap());
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 4: CONCURRENT APPEND
// ============================================================================

fn bench_concurrent_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_append");

    for thread_count in [2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements((thread_count * 100) as u64));

        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let capsule = Arc::new(AuditCompressionCapsule::new());
                    let mut handles = vec![];

                    for tid in 0..thread_count {
                        let capsule_clone = Arc::clone(&capsule);
                        let handle = thread::spawn(move || {
                            for i in 0..100 {
                                let event = AuditEvent::new(
                                    AuditEventType::FileAdd,
                                    tid as u8,
                                    &format!("/data/thread{}_file{}.txt", tid, i),
                                    "add",
                                );
                                let _ = capsule_clone.append(event);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(capsule.get_stats());
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 5: COMPRESSION RATIO
// ============================================================================

fn bench_compression_ratio(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression_ratio");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("repetitive_data", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let capsule = AuditCompressionCapsule::new();

                    // Repetitive data (highly compressible)
                    for _ in 0..size {
                        let event = AuditEvent::new(
                            AuditEventType::FileAdd,
                            1,
                            "/data/same_file.txt",
                            "same action",
                        );
                        capsule.append(event).unwrap();
                    }

                    let stats = capsule.get_stats();
                    black_box(stats);
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("unique_data", size), size, |b, &size| {
            b.iter(|| {
                let capsule = AuditCompressionCapsule::new();

                // Unique data (less compressible)
                for i in 0..size {
                    let event = AuditEvent::new(
                        AuditEventType::FileAdd,
                        1,
                        &format!("/data/unique_file_{}.txt", i),
                        &format!("unique action {}", i),
                    );
                    capsule.append(event).unwrap();
                }

                let stats = capsule.get_stats();
                black_box(stats);
            });
        });
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 6: FULL TRAIL VERIFICATION
// ============================================================================

fn bench_full_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_verification");

    for size in [100, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("verify_full", size), size, |b, &size| {
            let capsule = AuditCompressionCapsule::new();

            // Pre-populate with events
            for i in 0..size {
                let event = AuditEvent::new(
                    AuditEventType::FileAdd,
                    1,
                    &format!("/data/file{}.txt", i),
                    "add",
                );
                capsule.append(event).unwrap();
            }

            b.iter(|| {
                let result = capsule.verify_full();
                black_box(result.unwrap());
            });
        });
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 7: MIXED EVENT TYPES
// ============================================================================

fn bench_mixed_event_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_event_types");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("mixed_1000", |b| {
        let event_types = [
            AuditEventType::FileAdd,
            AuditEventType::FileModify,
            AuditEventType::FileDelete,
            AuditEventType::TrainStart,
            AuditEventType::TrainComplete,
            AuditEventType::CheckpointSave,
            AuditEventType::LicenseCheck,
            AuditEventType::SystemEvent,
        ];

        b.iter(|| {
            let capsule = AuditCompressionCapsule::new();

            for i in 0..1000 {
                let event_type = event_types[i % 8];
                let event = AuditEvent::new(
                    event_type,
                    (i % 256) as u8,
                    &format!("/data/file{}.txt", i),
                    &format!("action_{}", i),
                );
                capsule.append(event).unwrap();
            }

            black_box(capsule.get_stats());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK GROUPS
// ============================================================================

criterion_group!(
    benches,
    bench_append_latency_baseline,
    bench_append_latency_optimized,
    bench_batch_append_baseline,
    bench_batch_append_optimized,
    bench_verification_time,
    bench_concurrent_append,
    bench_compression_ratio,
    bench_full_verification,
    bench_mixed_event_types
);

criterion_main!(benches);
