//! Benchmark: RingBufferCapsule (heap) vs RingBufferCapsuleConst (const generic)
//!
//! **Purpose**: Validate 5-15% speedup claim from const generic optimizations
//!
//! **Optimizations Tested**:
//! 1. **Zero allocation**: Stack/static vs heap allocation (0ns vs 1-5ms for 16K entries)
//! 2. **Compile-time modulo**: Bitwise AND vs runtime modulo (1-2 cycles vs 3-5 cycles)
//! 3. **Better inlining**: All sizes known to compiler (aggressive optimization)
//!
//! **B32 Framework Compliance**:
//! - Fair baseline: Both use same atomic coordination (CAS loops, generation counters)
//! - Same hardware: x86_64, same compiler flags (--release)
//! - 95% CI, 1000+ iterations via Criterion.rs
//! - Conservative claims: 5-15% target (not 10-100×)
//!
//! **Expected Results**:
//! - **Allocation**: 0ns vs 1-5ms (NEW vs ORIGINAL, constructor only)
//! - **Record**: 5-15% faster (modulo optimization + better codegen)
//! - **Get recent**: 5-10% faster (better inlining)
//! - **Concurrent**: 5-10% faster (reduced contention from zero-alloc)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use atomic_capsule::collections::TraceEntry;

// Original heap-allocated version
use atomic_capsule::collections::RingBufferCapsule;

// Const generic version (nightly feature required)
#[cfg(feature = "nightly-const-generics")]
use atomic_capsule::collections::RingBufferCapsuleConst;

fn benchmark_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_allocation");

    // Baseline: Original heap-allocated version
    group.bench_function("original_heap_16k", |b| {
        b.iter(|| {
            let capsule = RingBufferCapsule::<u64>::new();
            black_box(capsule);
        });
    });

    // Optimized: Const generic version (zero allocation!)
    #[cfg(feature = "nightly-const-generics")]
    group.bench_function("const_generic_stack_16k", |b| {
        b.iter(|| {
            let capsule = RingBufferCapsuleConst::<u64, 16384>::new();
            black_box(capsule);
        });
    });

    group.finish();
}

fn benchmark_record_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_record_single");

    // Baseline: Original heap-allocated version
    let capsule_orig = RingBufferCapsule::<u64>::new();
    group.bench_function("original_heap", |b| {
        let mut i = 0u64;
        b.iter(|| {
            capsule_orig.record(black_box(i));
            i = i.wrapping_add(1);
        });
    });

    // Optimized: Const generic version
    #[cfg(feature = "nightly-const-generics")]
    {
        let capsule_const = RingBufferCapsuleConst::<u64, 16384>::new();
        group.bench_function("const_generic_stack", |b| {
            let mut i = 0u64;
            b.iter(|| {
                capsule_const.record(black_box(i));
                i = i.wrapping_add(1);
            });
        });
    }

    group.finish();
}

fn benchmark_record_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_record_batch");

    for batch_size in [100, 1000, 10000].iter() {
        // Baseline: Original heap-allocated version
        group.bench_with_input(
            BenchmarkId::new("original_heap", batch_size),
            batch_size,
            |b, &size| {
                let capsule = RingBufferCapsule::<u64>::new();
                b.iter(|| {
                    for i in 0..size {
                        capsule.record(black_box(i));
                    }
                });
            },
        );

        // Optimized: Const generic version
        #[cfg(feature = "nightly-const-generics")]
        group.bench_with_input(
            BenchmarkId::new("const_generic_stack", batch_size),
            batch_size,
            |b, &size| {
                let capsule = RingBufferCapsuleConst::<u64, 16384>::new();
                b.iter(|| {
                    for i in 0..size {
                        capsule.record(black_box(i));
                    }
                });
            },
        );
    }

    group.finish();
}

fn benchmark_get_recent(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_get_recent");

    // Setup: Pre-fill both capsules with 1000 entries
    let capsule_orig = RingBufferCapsule::<u64>::new();
    for i in 0..1000 {
        capsule_orig.record(i);
    }

    #[cfg(feature = "nightly-const-generics")]
    let capsule_const = RingBufferCapsuleConst::<u64, 16384>::new();
    #[cfg(feature = "nightly-const-generics")]
    for i in 0..1000 {
        capsule_const.record(i);
    }

    for count in [10, 100, 500].iter() {
        // Baseline: Original heap-allocated version
        group.bench_with_input(
            BenchmarkId::new("original_heap", count),
            count,
            |b, &size| {
                b.iter(|| {
                    black_box(capsule_orig.get_recent(size));
                });
            },
        );

        // Optimized: Const generic version
        #[cfg(feature = "nightly-const-generics")]
        group.bench_with_input(
            BenchmarkId::new("const_generic_stack", count),
            count,
            |b, &size| {
                b.iter(|| {
                    black_box(capsule_const.get_recent(size));
                });
            },
        );
    }

    group.finish();
}

fn benchmark_concurrent_record(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("ring_buffer_concurrent");
    group.sample_size(10);  // Reduce sample size for concurrent benchmarks

    // Baseline: Original heap-allocated version
    group.bench_function("original_heap_4_threads", |b| {
        b.iter(|| {
            let capsule = Arc::new(RingBufferCapsule::<u64>::new());
            let mut handles = vec![];

            for thread_id in 0..4 {
                let capsule_clone = Arc::clone(&capsule);
                let handle = thread::spawn(move || {
                    for i in 0..1000 {
                        let value = (thread_id * 10000 + i) as u64;
                        let _ = capsule_clone.record(value);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }

            black_box(capsule);
        });
    });

    // Optimized: Const generic version
    #[cfg(feature = "nightly-const-generics")]
    group.bench_function("const_generic_stack_4_threads", |b| {
        b.iter(|| {
            let capsule = Arc::new(RingBufferCapsuleConst::<u64, 16384>::new());
            let mut handles = vec![];

            for thread_id in 0..4 {
                let capsule_clone = Arc::clone(&capsule);
                let handle = thread::spawn(move || {
                    for i in 0..1000 {
                        let value = (thread_id * 10000 + i) as u64;
                        let _ = capsule_clone.record(value);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }

            black_box(capsule);
        });
    });

    group.finish();
}

fn benchmark_trace_entry_record(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_trace_entry");

    // Baseline: Original heap-allocated version
    let capsule_orig = RingBufferCapsule::<TraceEntry>::new();
    group.bench_function("original_heap", |b| {
        let mut i = 0u64;
        b.iter(|| {
            let entry = TraceEntry::new(0x1000 + i, i as u32, 1, 0);
            capsule_orig.record(black_box(entry));
            i = i.wrapping_add(1);
        });
    });

    // Optimized: Const generic version
    #[cfg(feature = "nightly-const-generics")]
    {
        let capsule_const = RingBufferCapsuleConst::<TraceEntry, 16384>::new();
        group.bench_function("const_generic_stack", |b| {
            let mut i = 0u64;
            b.iter(|| {
                let entry = TraceEntry::new(0x1000 + i, i as u32, 1, 0);
                capsule_const.record(black_box(entry));
                i = i.wrapping_add(1);
            });
        });
    }

    group.finish();
}

fn benchmark_wraparound(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_wraparound");

    // Use small capacity for faster wraparound testing
    #[cfg(feature = "nightly-const-generics")]
    {
        let capsule_const = RingBufferCapsuleConst::<u64, 1024>::new();
        group.bench_function("const_generic_1k", |b| {
            let mut i = 0u64;
            b.iter(|| {
                // Write 2000 entries to trigger wraparound
                for _ in 0..2000 {
                    capsule_const.record(black_box(i));
                    i = i.wrapping_add(1);
                }
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_allocation,
    benchmark_record_single,
    benchmark_record_batch,
    benchmark_get_recent,
    benchmark_concurrent_record,
    benchmark_trace_entry_record,
    benchmark_wraparound,
);
criterion_main!(benches);
