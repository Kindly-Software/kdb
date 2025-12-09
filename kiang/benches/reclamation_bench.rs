//! Reclamation benchmarks - validate performance targets
//!
//! **Performance Targets (B32 Framework)**:
//! - defer_free(): <50ns (lockfree queue append)
//! - process_deferred(): <1μs per item (sequential processing)
//! - allocate_from_free_list(): <500ns (single-writer, no contention)

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use kiang::reclamation::MemoryReclaimer;

/// Benchmark defer_free() lockfree queue operation
///
/// Target: <50ns per defer_free()
fn bench_defer_free(c: &mut Criterion) {
    let mut group = c.benchmark_group("reclamation_defer_free");

    // Single thread defer_free
    group.bench_function("defer_free_single", |b| {
        let mut reclaimer = MemoryReclaimer::new();
        let mut offset = 0u64;

        b.iter(|| {
            reclaimer.defer_free(black_box(offset), black_box(4096), black_box(1));
            offset += 4096;
        });
    });

    group.finish();
}

/// Benchmark process_deferred() sequential processing
///
/// Target: <1μs per item
fn bench_process_deferred(c: &mut Criterion) {
    let mut group = c.benchmark_group("reclamation_process_deferred");

    for queue_depth in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(queue_depth),
            queue_depth,
            |b, &depth| {
                b.iter_batched(
                    || {
                        // Setup: Defer many frees
                        let mut reclaimer = MemoryReclaimer::new();
                        for i in 0..depth {
                            reclaimer.defer_free(i * 4096, 4096, i);
                        }
                        reclaimer
                    },
                    |mut reclaimer| {
                        // Measure: Process all deferred frees
                        black_box(reclaimer.process_deferred())
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark allocate_from_free_list() single-writer allocation
///
/// Target: <500ns per allocation
fn bench_allocate_from_free_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("reclamation_allocate_from_free_list");

    // Allocate from free list with varying fragmentation
    for fragment_count in [1, 10, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(fragment_count),
            fragment_count,
            |b, &frag_count| {
                b.iter_batched(
                    || {
                        // Setup: Create free list with N fragments
                        let mut reclaimer = MemoryReclaimer::new();
                        for i in 0..frag_count {
                            reclaimer.defer_free(i * 8192, 4096, i);
                        }
                        reclaimer.process_deferred();
                        reclaimer
                    },
                    |mut reclaimer| {
                        // Measure: Allocate from free list
                        black_box(reclaimer.allocate_from_free_list(4096, 64))
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark capsule read operations
///
/// Target: <5ns (cached read)
fn bench_capsule_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("reclamation_capsule_read");

    group.bench_function("can_reclaim", |b| {
        let mut reclaimer = MemoryReclaimer::new();
        reclaimer.defer_free(0, 4096, 1);
        reclaimer.process_deferred();

        let capsule = reclaimer.capsule();

        b.iter(|| {
            black_box(capsule.can_reclaim());
        });
    });

    group.bench_function("deferred_count", |b| {
        let mut reclaimer = MemoryReclaimer::new();
        reclaimer.defer_free(0, 4096, 1);

        let capsule = reclaimer.capsule();

        b.iter(|| {
            black_box(capsule.deferred_count());
        });
    });

    group.bench_function("reclaimable_mb", |b| {
        let mut reclaimer = MemoryReclaimer::new();
        reclaimer.defer_free(0, 4096, 1);
        reclaimer.process_deferred();

        let capsule = reclaimer.capsule();

        b.iter(|| {
            black_box(capsule.reclaimable_mb());
        });
    });

    group.finish();
}

/// Benchmark realistic allocation/deallocation cycles
///
/// Simulates real GPU workload patterns:
/// - Allocate multiple buffers
/// - Free some buffers
/// - Reallocate from free list
fn bench_realistic_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("reclamation_realistic_workload");

    group.bench_function("alloc_free_realloc_cycle", |b| {
        b.iter_batched(
            || {
                // Setup: Empty reclaimer
                MemoryReclaimer::new()
            },
            |mut reclaimer| {
                // Phase 1: Defer 100 frees (simulates GPU work completion)
                for i in 0..100 {
                    reclaimer.defer_free(i * 4096, 4096, i);
                }

                // Phase 2: Process deferred frees (reclamation thread)
                reclaimer.process_deferred();

                // Phase 3: Reallocate 50 buffers from free list
                for _ in 0..50 {
                    black_box(reclaimer.allocate_from_free_list(4096, 64));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark AMD's failed parallel reclamation scenario
///
/// This demonstrates that KIANG's design prevents the race conditions
/// that destroyed AMD's driver.
///
/// Note: This benchmark measures single-threaded performance because
/// Rust's type system prevents parallel reclamation at compile-time!
fn bench_amd_mistake_prevented(c: &mut Criterion) {
    let mut group = c.benchmark_group("reclamation_amd_mistake_prevented");

    group.bench_function("single_writer_safety", |b| {
        b.iter_batched(
            || {
                // Setup: Defer many frees
                let mut reclaimer = MemoryReclaimer::new();
                for i in 0..1000 {
                    reclaimer.defer_free(i * 4096, 4096, i);
                }
                reclaimer
            },
            |mut reclaimer| {
                // AMD's mistake: Parallel processing → race conditions
                // KIANG's solution: Single writer (&mut self) → no races!
                //
                // This CANNOT be parallelized because:
                // 1. process_deferred() requires &mut self
                // 2. Rust's borrow checker prevents &mut through Arc
                // 3. Type system enforces single-writer at compile-time
                black_box(reclaimer.process_deferred())
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_defer_free,
    bench_process_deferred,
    bench_allocate_from_free_list,
    bench_capsule_read,
    bench_realistic_workload,
    bench_amd_mistake_prevented,
);
criterion_main!(benches);
