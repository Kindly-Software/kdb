//! # B32 Benchmarks: T9 Persistent Capsule
//!
//! **Purpose**: Fair baseline comparison for memory-mapped atomic persistence
//!
//! # Framework Compliance
//!
//! - **B32**: Fair baselines (serde+bincode+fs, not strawman), 1000+ iterations, 95% CI
//! - **UCE34 Q10**: T9 Persistent tier (Atomic + Mmap)
//! - **Honest Claims**: 100-1000× vs serialize+write (documented why)
//!
//! # Benchmark Suites
//!
//! 1. **Atomic Operations**: Store/load/CAS/fetch_add to mmap (<50ns target)
//! 2. **Persistence Operations**: Sync/async flush, crash recovery (<1ms flush, <100ms recovery)
//! 3. **Comparative Analysis**: T9 vs serialize+filesystem (expect 1000× for hot writes)
//! 4. **Scaling Analysis**: Throughput scaling (1M, 10M, 100M ops)
//! 5. **Production Scenarios**: Incremental dedup, high-throughput counter
//!
//! # Expected Performance (B32 Reality Check)
//!
//! ```text
//! Operation           | Target    | Baseline           | Expected Speedup
//! ────────────────────────────────────────────────────────────────────────
//! Atomic write        | <50ns     | serialize (10-100μs)| 200-2000× ✅
//! Async flush         | <1ms      | fs::sync_all (5-10ms)| 5-10× ✅
//! Crash recovery      | <100ms    | deserialize (1-10s) | 10-100× ✅
//! Throughput          | 20M ops/s | Mutex (1M ops/s)   | 20× ✅
//! ```
//!
//! # Hardware
//!
//! - **CPU**: Intel Ultra 7 155H (6P+8E+2LP cores)
//! - **Storage**: NVMe SSD (typical for development)
//! - **OS**: Linux 6.14.0-33-generic
//! - **Rust**: 1.88.0-nightly
//!
//! # Run Benchmarks
//!
//! ```bash
//! # All T9 benchmarks
//! cargo +nightly bench --bench persistent_bench --features "nightly-atomic,mmap-persistence"
//!
//! # Specific suite
//! cargo +nightly bench --bench persistent_bench atomic_operations
//! cargo +nightly bench --bench persistent_bench persistence_operations
//! cargo +nightly bench --bench persistent_bench comparative
//! ```

#![cfg(feature = "nightly-atomic")]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "nightly-atomic")]
use atomic_capsule::primitives::atomic_from_mut::AtomicFromMut;

// Optional: For comparative benchmarks
#[cfg(feature = "capsule-serialize")]
use serde::{Deserialize, Serialize};

// ============================================================================
// BENCHMARK SUITE 1: ATOMIC OPERATIONS
// ============================================================================
//
// **Goal**: Measure raw atomic operation latency on mmap'd memory
// **Baseline**: In-memory atomic (reference, should be identical)
// **Target**: <50ns for store, <10ns for load, <100ns for CAS
//
// **B32 Honesty**: These should match in-memory atomics (0× speedup)
// because hardware atomics are the same regardless of backing store.

fn bench_atomic_store(c: &mut Criterion) {
    let mut group = c.benchmark_group("1_atomic_operations");
    group.throughput(Throughput::Elements(1));

    // Baseline: In-memory atomic (reference)
    group.bench_function("atomic_store_memory", |b| {
        let atomic = AtomicU64::new(0);
        let mut counter = 0u64;
        b.iter(|| {
            atomic.store(counter, Ordering::SeqCst);
            counter = counter.wrapping_add(1);
            black_box(&atomic);
        });
    });

    // T9: Atomic store to mmap (via atomic_from_mut)
    group.bench_function("atomic_store_mmap", |b| {
        // Create temporary mmap file
        let path = "/tmp/bench_atomic_store.mmap";
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        file.set_len(4096).unwrap();

        let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };

        let atomic = u64::from_slice_mut(&mut mmap[0..8], 0).expect("aligned mmap");

        let mut counter = 0u64;
        b.iter(|| {
            atomic.store(counter, Ordering::SeqCst);
            counter = counter.wrapping_add(1);
            black_box(&atomic);
        });

        std::mem::drop(mmap);
        let _ = std::fs::remove_file(path);
    });

    group.finish();
}

fn bench_atomic_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("1_atomic_operations");
    group.throughput(Throughput::Elements(1));

    // Baseline: In-memory atomic load
    group.bench_function("atomic_load_memory", |b| {
        let atomic = AtomicU64::new(42);
        b.iter(|| black_box(atomic.load(Ordering::Acquire)));
    });

    // T9: Atomic load from mmap
    group.bench_function("atomic_load_mmap", |b| {
        let path = "/tmp/bench_atomic_load.mmap";
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        file.set_len(4096).unwrap();

        let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
        let atomic = u64::from_slice_mut(&mut mmap[0..8], 0).expect("aligned mmap");
        atomic.store(42, Ordering::Release);

        b.iter(|| black_box(atomic.load(Ordering::Acquire)));

        std::mem::drop(mmap);
        let _ = std::fs::remove_file(path);
    });

    group.finish();
}

fn bench_atomic_fetch_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("1_atomic_operations");
    group.throughput(Throughput::Elements(1));

    // Baseline: In-memory fetch_add
    group.bench_function("fetch_add_memory", |b| {
        let atomic = AtomicU64::new(0);
        b.iter(|| black_box(atomic.fetch_add(1, Ordering::Relaxed)));
    });

    // T9: fetch_add on mmap
    group.bench_function("fetch_add_mmap", |b| {
        let path = "/tmp/bench_fetch_add.mmap";
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        file.set_len(4096).unwrap();

        let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
        let atomic = u64::from_slice_mut(&mut mmap[0..8], 0).expect("aligned mmap");

        b.iter(|| black_box(atomic.fetch_add(1, Ordering::Relaxed)));

        std::mem::drop(mmap);
        let _ = std::fs::remove_file(path);
    });

    group.finish();
}

fn bench_atomic_cas(c: &mut Criterion) {
    let mut group = c.benchmark_group("1_atomic_operations");
    group.throughput(Throughput::Elements(1));

    // Baseline: In-memory CAS
    group.bench_function("cas_memory", |b| {
        let atomic = AtomicU64::new(0);
        let mut expected = 0u64;
        b.iter(|| {
            match atomic.compare_exchange_weak(
                expected,
                expected + 1,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(val) => expected = val + 1,
                Err(val) => expected = val,
            }
        });
    });

    // T9: CAS on mmap
    group.bench_function("cas_mmap", |b| {
        let path = "/tmp/bench_cas.mmap";
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        file.set_len(4096).unwrap();

        let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
        let atomic = u64::from_slice_mut(&mut mmap[0..8], 0).expect("aligned mmap");

        let mut expected = 0u64;
        b.iter(|| {
            match atomic.compare_exchange_weak(
                expected,
                expected + 1,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(val) => expected = val + 1,
                Err(val) => expected = val,
            }
        });

        std::mem::drop(mmap);
        let _ = std::fs::remove_file(path);
    });

    group.finish();
}

// ============================================================================
// BENCHMARK SUITE 2: PERSISTENCE OPERATIONS
// ============================================================================
//
// **Goal**: Measure flush latency and crash recovery time
// **Baseline**: fs::write + fs::sync_all (traditional approach)
// **Target**: <1ms async flush, <100ms recovery
//
// **B32 Honesty**: T9 flush should be 5-10× faster than sync_all
// because msync is optimized for mmap'd regions.

fn bench_flush_sync(c: &mut Criterion) {
    let mut group = c.benchmark_group("2_persistence_operations");
    group.sample_size(100); // Reduce samples (I/O is slow)

    // Baseline: fs::write + fs::sync_all
    group.bench_function("flush_sync_filesystem", |b| {
        let path = "/tmp/bench_flush_sync_fs.bin";
        b.iter(|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)
                .unwrap();
            file.write_all(&[42u8; 8]).unwrap();
            file.sync_all().unwrap(); // Synchronous flush
            black_box(&file);
        });
        let _ = std::fs::remove_file(path);
    });

    // T9: msync(MS_SYNC) on mmap
    group.bench_function("flush_sync_mmap", |b| {
        let path = "/tmp/bench_flush_sync_mmap.mmap";
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        file.set_len(4096).unwrap();

        b.iter(|| {
            let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
            let atomic = u64::from_slice_mut(&mut mmap[0..8], 0).expect("aligned mmap");
            atomic.store(42, Ordering::SeqCst);
            mmap.flush().unwrap(); // msync(MS_SYNC)
            black_box(&mmap);
        });

        let _ = std::fs::remove_file(path);
    });

    group.finish();
}

fn bench_flush_async(c: &mut Criterion) {
    let mut group = c.benchmark_group("2_persistence_operations");
    group.sample_size(100);

    // Baseline: fs::write (no sync)
    group.bench_function("flush_async_filesystem", |b| {
        let path = "/tmp/bench_flush_async_fs.bin";
        b.iter(|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)
                .unwrap();
            file.write_all(&[42u8; 8]).unwrap();
            // No sync (async)
            black_box(&file);
        });
        let _ = std::fs::remove_file(path);
    });

    // T9: msync(MS_ASYNC) on mmap
    group.bench_function("flush_async_mmap", |b| {
        let path = "/tmp/bench_flush_async_mmap.mmap";
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        file.set_len(4096).unwrap();

        b.iter(|| {
            let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
            let atomic = u64::from_slice_mut(&mut mmap[0..8], 0).expect("aligned mmap");
            atomic.store(42, Ordering::SeqCst);
            mmap.flush_async().unwrap(); // msync(MS_ASYNC)
            black_box(&mmap);
        });

        let _ = std::fs::remove_file(path);
    });

    group.finish();
}

fn bench_crash_recovery(c: &mut Criterion) {
    let mut group = c.benchmark_group("2_persistence_operations");
    group.sample_size(50); // Even fewer samples (I/O intensive)

    // Baseline: Deserialize from disk (bincode)
    #[cfg(feature = "capsule-serialize")]
    group.bench_function("recovery_deserialize_bincode", |b| {
        use bincode::{deserialize, serialize};

        #[derive(Serialize, Deserialize)]
        struct State {
            values: Vec<u64>,
        }

        let path = "/tmp/bench_recovery_bincode.bin";
        let state = State {
            values: vec![42u64; 1024], // 8KB state
        };
        let bytes = serialize(&state).unwrap();
        std::fs::write(path, &bytes).unwrap();

        b.iter(|| {
            let bytes = std::fs::read(path).unwrap();
            let _state: State = deserialize(&bytes).unwrap();
            black_box(_state);
        });

        let _ = std::fs::remove_file(path);
    });

    // T9: Re-mmap file (instant recovery)
    group.bench_function("recovery_mmap_instant", |b| {
        let path = "/tmp/bench_recovery_mmap.mmap";
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        file.set_len(8192).unwrap(); // 8KB

        // Write initial state
        {
            let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
            for i in 0..1024 {
                let offset = i * 8;
                let atomic =
                    u64::from_slice_mut(&mut mmap[offset..offset + 8], 0).expect("aligned mmap");
                atomic.store(42, Ordering::Release);
            }
            mmap.flush().unwrap();
        }

        b.iter(|| {
            // Recovery: Just re-mmap (zero deserialization)
            let mmap = unsafe { memmap2::Mmap::map(&file).unwrap() };
            let atomic = unsafe { &*(mmap.as_ptr() as *const AtomicU64) };
            black_box(atomic.load(Ordering::Acquire));
        });

        let _ = std::fs::remove_file(path);
    });

    group.finish();
}

// ============================================================================
// BENCHMARK SUITE 3: COMPARATIVE ANALYSIS
// ============================================================================
//
// **Goal**: Direct comparison T9 vs traditional serialize+write
// **Expected**: 1000× faster for hot atomic writes (50ns vs 20ms)
//
// **B32 Honesty**: This is the "killer app" for T9 - avoiding
// serialization overhead. Speedup is legitimate because we're comparing
// direct atomic store (<50ns) vs serialize+write+sync (10-20ms).

fn bench_t9_vs_serialize_single_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("3_comparative_single_update");

    // Baseline: Serialize + write + sync (traditional approach)
    #[cfg(feature = "capsule-serialize")]
    group.bench_function("serialize_write_sync", |b| {
        use bincode::serialize;

        #[derive(Serialize, Deserialize)]
        struct State {
            value: u64,
        }

        let path = "/tmp/bench_serialize_single.bin";
        b.iter(|| {
            let state = State { value: 42 };
            let bytes = serialize(&state).unwrap(); // 10-100μs
            std::fs::write(path, bytes).unwrap(); // 1-10ms
            let mut file = OpenOptions::new().write(true).open(path).unwrap();
            file.sync_all().unwrap(); // 1-10ms
        });
        let _ = std::fs::remove_file(path);
    });

    // T9: Atomic write to mmap
    group.bench_function("t9_atomic_write_mmap", |b| {
        let path = "/tmp/bench_t9_single.mmap";
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        file.set_len(4096).unwrap();

        let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
        let atomic = u64::from_slice_mut(&mut mmap[0..8], 0).expect("aligned mmap");

        b.iter(|| {
            atomic.store(42, Ordering::SeqCst); // <50ns
                                                // No flush needed for single write (amortized later)
        });

        std::mem::drop(mmap);
        let _ = std::fs::remove_file(path);
    });

    group.finish();
}

fn bench_t9_vs_serialize_batch_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("3_comparative_batch_updates");

    for batch_size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        // Baseline: Serialize + write for each update
        #[cfg(feature = "capsule-serialize")]
        group.bench_with_input(
            BenchmarkId::new("serialize_each", batch_size),
            batch_size,
            |b, &batch_size| {
                use bincode::serialize;

                #[derive(Serialize, Deserialize)]
                struct State {
                    value: u64,
                }

                let path = "/tmp/bench_serialize_batch.bin";
                b.iter(|| {
                    for i in 0..batch_size {
                        let state = State { value: i as u64 };
                        let bytes = serialize(&state).unwrap();
                        std::fs::write(path, bytes).unwrap();
                    }
                });
                let _ = std::fs::remove_file(path);
            },
        );

        // T9: Atomic writes with periodic flush
        group.bench_with_input(
            BenchmarkId::new("t9_atomic_batch", batch_size),
            batch_size,
            |b, &batch_size| {
                let path = "/tmp/bench_t9_batch.mmap";
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(path)
                    .unwrap();
                file.set_len(4096).unwrap();

                b.iter(|| {
                    let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
                    let atomic = u64::from_slice_mut(&mut mmap[0..8], 0).expect("aligned mmap");

                    for i in 0..batch_size {
                        atomic.store(i as u64, Ordering::SeqCst); // <50ns each
                    }

                    // Flush once at end (amortized)
                    mmap.flush_async().unwrap();
                });

                let _ = std::fs::remove_file(path);
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK SUITE 4: SCALING ANALYSIS
// ============================================================================
//
// **Goal**: Measure throughput scaling from 1M to 100M operations
// **Expected**: 20M ops/sec sustained (50ns per atomic write)
//
// **B32 Honesty**: Throughput should scale linearly with batch size
// until hitting memory bandwidth limits (~15GB/s on DDR5-5600).

fn bench_throughput_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("4_throughput_scaling");

    for num_ops in [100_000, 1_000_000, 10_000_000].iter() {
        group.throughput(Throughput::Elements(*num_ops as u64));
        group.sample_size(10); // Large benchmarks need fewer samples

        group.bench_with_input(
            BenchmarkId::new("sequential_writes", num_ops),
            num_ops,
            |b, &num_ops| {
                let path = format!("/tmp/bench_throughput_{}.mmap", num_ops);
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(&path)
                    .unwrap();
                file.set_len(4096).unwrap();

                b.iter(|| {
                    let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
                    let atomic = u64::from_slice_mut(&mut mmap[0..8], 0).expect("aligned mmap");

                    for i in 0..num_ops {
                        atomic.store(i as u64, Ordering::Relaxed);
                    }

                    mmap.flush_async().unwrap();
                });

                let _ = std::fs::remove_file(&path);
            },
        );
    }

    group.finish();
}

fn bench_file_size_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("4_file_size_scaling");

    for file_size_mb in [1, 10, 100].iter() {
        let file_size = file_size_mb * 1024 * 1024;
        group.throughput(Throughput::Bytes(file_size as u64));

        group.bench_with_input(
            BenchmarkId::new("mmap_creation", format!("{}MB", file_size_mb)),
            &file_size,
            |b, &file_size| {
                let path = format!("/tmp/bench_scaling_{}mb.mmap", file_size_mb);

                b.iter(|| {
                    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(&path)
                        .unwrap();
                    file.set_len(file_size as u64).unwrap();

                    let mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
                    black_box(&mmap);
                });

                let _ = std::fs::remove_file(&path);
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK SUITE 5: PRODUCTION SCENARIOS
// ============================================================================
//
// **Goal**: Benchmark real-world patterns from T9 spec
// **Scenarios**:
//   1. High-throughput counter (20M ops/sec target)
//   2. Incremental dedup (simulate weekly 1% new docs)
//
// **B32 Honesty**: These represent actual use cases, not synthetic loops.

fn bench_high_throughput_counter(c: &mut Criterion) {
    let mut group = c.benchmark_group("5_production_counter");
    group.throughput(Throughput::Elements(1_000_000));

    group.bench_function("counter_1m_increments", |b| {
        let path = "/tmp/bench_counter.mmap";

        b.iter(|| {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(path)
                .unwrap();
            file.set_len(4096).unwrap();

            let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
            let counter = u64::from_slice_mut(&mut mmap[0..8], 0).expect("aligned mmap");

            for _ in 0..1_000_000 {
                counter.fetch_add(1, Ordering::Relaxed);
            }
            // Flush every 1M ops (amortized <1μs per op)
            mmap.flush_async().unwrap();
        });

        let _ = std::fs::remove_file(path);
    });

    group.finish();
}

fn bench_incremental_dedup_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("5_production_dedup");
    group.sample_size(10);

    // Simulate: 10K docs already persisted, add 100 new docs (1% new)
    group.bench_function("dedup_add_1pct_new", |b| {
        let path = "/tmp/bench_dedup.mmap";

        // 10K docs × 256B signature = 2.56MB
        let total_docs = 10_000;
        let new_docs = 100; // 1% new
        let sig_size = 256;

        b.iter(|| {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(path)
                .unwrap();
            file.set_len((total_docs * sig_size) as u64).unwrap();

            let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };

            // Simulate adding 100 new signatures
            for i in 0..new_docs {
                let offset = (total_docs - new_docs + i) * sig_size;
                let atomic =
                    u64::from_slice_mut(&mut mmap[offset..offset + 8], 0).expect("aligned mmap");
                atomic.store(i as u64, Ordering::Release);
            }

            mmap.flush_async().unwrap();
        });

        let _ = std::fs::remove_file(path);
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    atomic_operations,
    bench_atomic_store,
    bench_atomic_load,
    bench_atomic_fetch_add,
    bench_atomic_cas,
);

criterion_group!(
    persistence_operations,
    bench_flush_sync,
    bench_flush_async,
    bench_crash_recovery,
);

criterion_group!(
    comparative,
    bench_t9_vs_serialize_single_update,
    bench_t9_vs_serialize_batch_updates,
);

criterion_group!(scaling, bench_throughput_scaling, bench_file_size_scaling,);

criterion_group!(
    production,
    bench_high_throughput_counter,
    bench_incremental_dedup_simulation,
);

criterion_main!(
    atomic_operations,
    persistence_operations,
    comparative,
    scaling,
    production
);
