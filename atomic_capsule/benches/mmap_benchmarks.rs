//! B32 Benchmarks for Capsule-Native Mmap vs memmap2 Baseline
//!
//! **Purpose**: Fair comparison of capsule-native lockfree mmap vs memmap2 with mutex coordination
//!
//! # Architecture Comparison
//!
//! **Baseline (memmap2)**:
//! - Memory-mapped I/O via memmap2::MmapMut
//! - Coordination via std::sync::Mutex (blocking, contention)
//! - Sequential allocation with lock overhead
//!
//! **Capsule-Native (MmapManager)**:
//! - Memory-mapped I/O via memmap2 (same underlying syscall)
//! - Coordination via lockfree CAS loops (T1 Atomic tier)
//! - Concurrent allocation without blocking
//! - Generation counters for TOCTOU prevention
//!
//! # Benchmarks
//!
//! 1. **File Initialization**: Create + mmap 1GB file (OS-bound, no speedup expected)
//! 2. **Region Allocation**: Lockfree CAS vs Mutex lock (target: <20ns vs ~50ns)
//! 3. **Concurrent Allocation**: 8-thread contention (target: 3-10× speedup)
//! 4. **fsync() Latency**: Durability syscall (baseline: <1ms NVMe)
//! 5. **Region Access**: Array index vs HashMap lookup (target: <5ns)
//!
//! # B32 Framework Compliance
//!
//! - **Fair Baseline**: Same machine, same compiler, same memmap2 syscall
//! - **Statistical Rigor**: 100+ iterations for initialization, 1000+ for micro-ops
//! - **Realistic Workload**: Real mmap syscalls, not synthetic mocks
//! - **Full Disclosure**: Hardware (AMD Ryzen 9 6900HX), storage (NVMe SSD)
//! - **Honest Claims**: 10-50% typical, 2-10× exceptional, 100×+ requires extensive validation
//!
//! # Performance Targets
//!
//! | Benchmark | Baseline (memmap2) | Capsule-Native (MmapManager) | Expected Speedup |
//! |-----------|--------------------|-----------------------------|------------------|
//! | Initialization | <10ms | <10ms | 1× (OS-bound) |
//! | Allocation | ~50ns (mutex) | <20ns (CAS) | 2-3× |
//! | Concurrent (8T) | ~400ns (contention) | <50ns (lockfree) | 3-10× |
//! | fsync() | <1ms (NVMe) | <1ms (NVMe) | 1× (hardware-bound) |
//! | Region Access | ~10ns (HashMap) | <5ns (array index) | 2× |
//!
//! # Reality Check (B32 § R7)
//!
//! **No Software Optimization Can Reduce**:
//! - fsync() latency: Hardware-bound (NVMe ~1ms, SSD ~3ms, HDD ~10ms)
//! - File creation: OS syscall overhead (~1ms)
//! - mmap() syscall: Page table setup (~1ms)
//!
//! **Software Can Optimize**:
//! - Allocation coordination: Lockfree CAS vs mutex (2-10× speedup)
//! - Concurrent access: No contention vs blocking (3-10× at 8 threads)
//! - Region lookup: Array index vs HashMap (2× speedup)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

#[cfg(feature = "mmap-persistence")]
use atomic_capsule::persistence::{MmapLayout, MmapManager};

#[cfg(feature = "mmap-persistence")]
use memmap2::MmapMut;

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// BENCHMARK 1: FILE INITIALIZATION
// ============================================================================

/// Benchmark file initialization (create + mmap 1GB file)
///
/// # B32 Framework Compliance
///
/// - Fair baseline: Both use same memmap2::MmapMut::map_mut syscall
/// - Realistic workload: Real 1GB file creation + mmap
/// - Full disclosure: File size (1GB), filesystem (ext4/XFS/APFS)
///
/// # Performance Expectations
///
/// - Baseline (memmap2): <10ms (OS syscall overhead)
/// - Capsule-native (MmapManager): <10ms (same syscall, +region init)
/// - Expected speedup: **1× (no speedup)** - Both OS-bound
///
/// # Reality Check
///
/// File creation + mmap is **hardware-bound**. No software optimization can
/// reduce this latency. Claims of "<1ms 1GB mmap" are physically impossible.
#[cfg(feature = "mmap-persistence")]
fn bench_file_initialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmap_file_initialization");

    // Reduce sample size (file I/O is slow)
    group.sample_size(100);

    let file_size = 1024 * 1024 * 1024; // 1GB
    group.throughput(Throughput::Bytes(file_size as u64));

    // Baseline: memmap2 with mutex wrapper
    group.bench_function("baseline_memmap2_1gb", |b| {
        let temp_dir = std::env::temp_dir();

        b.iter(|| {
            let path = temp_dir.join(format!("bench_baseline_{}.bin", rand::random::<u64>()));

            // Create file
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&path)
                .unwrap();

            // Set size
            file.set_len(file_size).unwrap();

            // Create mmap
            let _mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

            // Cleanup
            let _ = std::fs::remove_file(&path);
        });
    });

    // Capsule-native: MmapManager with lockfree regions
    group.bench_function("capsule_native_mmapmanager_1gb", |b| {
        let temp_dir = std::env::temp_dir();

        b.iter(|| {
            let path = temp_dir.join(format!("bench_capsule_{}.bin", rand::random::<u64>()));

            // Create layout (8 regions)
            let layout = MmapLayout::new(file_size, 8).unwrap();

            // Create MmapManager (includes file creation + mmap + region init)
            let _manager = MmapManager::new(&path, &layout).unwrap();

            // Cleanup
            let _ = std::fs::remove_file(&path);
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: REGION ALLOCATION
// ============================================================================

/// Benchmark region allocation (lockfree CAS vs mutex)
///
/// # B32 Framework Compliance
///
/// - Fair baseline: Single-threaded, same allocation pattern
/// - Realistic workload: 10K sequential allocations (1KB each)
/// - Full disclosure: Allocation size (1KB), allocation count (10K)
///
/// # Performance Expectations
///
/// - Baseline (memmap2 + mutex): ~50ns per allocation
/// - Capsule-native (MmapManager CAS): <20ns per allocation
/// - Expected speedup: **2-3×** - Lockfree CAS faster than mutex
#[cfg(feature = "mmap-persistence")]
fn bench_region_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmap_region_allocation");

    let allocation_size = 1024; // 1KB per allocation
    let allocation_count = 10_000;
    group.throughput(Throughput::Elements(allocation_count as u64));

    // Baseline: memmap2 with mutex-protected offset tracking
    group.bench_function("baseline_mutex_10k_allocs", |b| {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("bench_baseline_alloc.bin");

        // Setup: Create 100MB file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .unwrap();

        let file_size = 100 * 1024 * 1024; // 100MB
        file.set_len(file_size).unwrap();

        let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

        // Mutex-protected offset tracker
        let offset = Arc::new(Mutex::new(0usize));

        b.iter(|| {
            // Reset offset
            *offset.lock().unwrap() = 0;

            for _ in 0..allocation_count {
                let mut current_offset = offset.lock().unwrap();
                let allocated_offset = *current_offset;
                *current_offset += allocation_size;
                drop(current_offset); // Release lock

                black_box(allocated_offset);
            }
        });

        // Cleanup
        drop(mmap);
        let _ = std::fs::remove_file(&path);
    });

    // Capsule-native: MmapManager with lockfree CAS
    group.bench_function("capsule_native_lockfree_10k_allocs", |b| {
        b.iter(|| {
            let temp_dir = std::env::temp_dir();
            let path = temp_dir.join(format!("bench_capsule_alloc_{}.bin", rand::random::<u64>()));

            // Setup: Create 100MB file with 1 region (fresh manager per iteration)
            let layout = MmapLayout::new(100 * 1024 * 1024, 1).unwrap();
            let manager = MmapManager::new(&path, &layout).unwrap();

            for _ in 0..allocation_count {
                let offset = manager
                    .region(0)
                    .unwrap()
                    .allocate(allocation_size)
                    .unwrap();
                black_box(offset);
            }

            // Cleanup
            drop(manager);
            let _ = std::fs::remove_file(&path);
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: CONCURRENT ALLOCATION
// ============================================================================

/// Benchmark concurrent allocation (8 threads, lockfree vs mutex contention)
///
/// # B32 Framework Compliance
///
/// - Fair baseline: Same workload, 8 threads, 1K allocations per thread
/// - Realistic workload: Concurrent producers allocating from shared pool
/// - Full disclosure: Thread count (8), allocations per thread (1K)
///
/// # Performance Expectations
///
/// - Baseline (mutex): ~400ns per allocation (high contention at 8 threads)
/// - Capsule-native (CAS): <50ns per allocation (lockfree, no blocking)
/// - Expected speedup: **3-10×** - Lockfree eliminates contention
///
/// # Scaling Analysis
///
/// - 1 thread: 1× (no contention)
/// - 2 threads: 1.5× (light contention)
/// - 4 threads: 3× (moderate contention)
/// - 8 threads: 5-10× (high contention)
#[cfg(feature = "mmap-persistence")]
fn bench_concurrent_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmap_concurrent_allocation");

    // Reduce sample size (thread spawning is slow)
    group.sample_size(50);

    let allocation_size = 1024; // 1KB
    let allocations_per_thread = 1000;
    let thread_counts = [1, 2, 4, 8];

    for &thread_count in &thread_counts {
        let total_allocations = thread_count * allocations_per_thread;
        group.throughput(Throughput::Elements(total_allocations as u64));

        // Baseline: memmap2 with mutex (contention)
        group.bench_with_input(
            BenchmarkId::new("baseline_mutex", thread_count),
            &thread_count,
            |b, &thread_count| {
                let temp_dir = std::env::temp_dir();
                let path = temp_dir.join("bench_baseline_concurrent.bin");

                // Setup: 100MB file
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(&path)
                    .unwrap();

                file.set_len(100 * 1024 * 1024).unwrap();

                let mmap = Arc::new(Mutex::new(unsafe { MmapMut::map_mut(&file).unwrap() }));
                let offset = Arc::new(Mutex::new(0usize));

                b.iter(|| {
                    // Reset offset
                    *offset.lock().unwrap() = 0;

                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let offset = Arc::clone(&offset);
                            thread::spawn(move || {
                                for _ in 0..allocations_per_thread {
                                    let mut current_offset = offset.lock().unwrap();
                                    let allocated_offset = *current_offset;
                                    *current_offset += allocation_size;
                                    drop(current_offset); // Release lock

                                    black_box(allocated_offset);
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });

                // Cleanup
                drop(mmap);
                let _ = std::fs::remove_file(&path);
            },
        );

        // Capsule-native: MmapManager with lockfree CAS (no contention)
        group.bench_with_input(
            BenchmarkId::new("capsule_native_lockfree", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let temp_dir = std::env::temp_dir();
                    let path = temp_dir.join(format!(
                        "bench_capsule_concurrent_{}.bin",
                        rand::random::<u64>()
                    ));

                    // Setup: 100MB file with 1 region (fresh manager per iteration)
                    let layout = MmapLayout::new(100 * 1024 * 1024, 1).unwrap();
                    let manager = Arc::new(MmapManager::new(&path, &layout).unwrap());

                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let manager = Arc::clone(&manager);
                            thread::spawn(move || {
                                for _ in 0..allocations_per_thread {
                                    let offset = manager
                                        .region(0)
                                        .unwrap()
                                        .allocate(allocation_size)
                                        .unwrap();
                                    black_box(offset);
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    // Cleanup
                    drop(manager);
                    let _ = std::fs::remove_file(&path);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 4: FSYNC LATENCY
// ============================================================================

/// Benchmark fsync() latency (hardware-bound durability)
///
/// # B32 Framework Compliance
///
/// - Fair baseline: Both use memmap2::flush() (same syscall)
/// - Realistic workload: Flush 1MB dirty pages
/// - Full disclosure: Data size (1MB), storage (NVMe SSD)
///
/// # Performance Expectations
///
/// - Baseline (memmap2): <1ms (NVMe), <3ms (SATA SSD), <10ms (HDD)
/// - Capsule-native (MmapManager): <1ms (same syscall, +generation bump)
/// - Expected speedup: **1× (no speedup)** - Both hardware-bound
///
/// # Reality Check
///
/// fsync() latency is **HARDWARE-BOUND**. No software optimization can reduce
/// this latency. Claims of "<100µs fsync" are physically impossible on consumer
/// hardware without battery-backed write cache or NVMe with supercap.
#[cfg(feature = "mmap-persistence")]
fn bench_fsync_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmap_fsync_latency");

    // Reduce sample size (fsync is slow)
    group.sample_size(100);

    let data_size = 1024 * 1024; // 1MB
    group.throughput(Throughput::Bytes(data_size as u64));

    // Baseline: memmap2::flush()
    group.bench_function("baseline_memmap2_flush_1mb", |b| {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("bench_baseline_fsync.bin");

        // Setup: 10MB file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .unwrap();

        file.set_len(10 * 1024 * 1024).unwrap();

        let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

        b.iter(|| {
            // Write 1MB dirty pages
            for i in (0..data_size).step_by(4096) {
                let end = (i + 4096).min(data_size);
                mmap[i..end].fill(black_box(0x42));
            }

            // Measure fsync() latency
            black_box(mmap.flush().unwrap());
        });

        // Cleanup
        drop(mmap);
        let _ = std::fs::remove_file(&path);
    });

    // Capsule-native: MmapManager::fsync()
    group.bench_function("capsule_native_fsync_1mb", |b| {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("bench_capsule_fsync.bin");

        // Setup: 10MB file with 1 region
        let layout = MmapLayout::new(10 * 1024 * 1024, 1).unwrap();
        let mut manager = MmapManager::new(&path, &layout).unwrap();

        b.iter(|| {
            // Write 1MB dirty pages via unsafe mmap slice access
            unsafe {
                let slice = manager.mmap_slice_at(0, data_size);
                for i in (0..data_size).step_by(4096) {
                    let end = (i + 4096).min(data_size);
                    slice[i..end].fill(black_box(0x42));
                }
            }

            // Measure fsync() latency (includes generation bump)
            use atomic_capsule::persistence::Durable;
            black_box(manager.fsync().unwrap());
        });

        // Cleanup
        drop(manager);
        let _ = std::fs::remove_file(&path);
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 5: REGION ACCESS
// ============================================================================

/// Benchmark region access (array index vs HashMap lookup)
///
/// # B32 Framework Compliance
///
/// - Fair baseline: Same access pattern, 10K lookups
/// - Realistic workload: Region metadata access during allocation
/// - Full disclosure: Access count (10K), region count (8)
///
/// # Performance Expectations
///
/// - Baseline (HashMap): ~10ns per lookup (hash + bucket search)
/// - Capsule-native (array): <5ns per lookup (direct index)
/// - Expected speedup: **2×** - Array index faster than HashMap
#[cfg(feature = "mmap-persistence")]
fn bench_region_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmap_region_access");

    let access_count = 10_000;
    group.throughput(Throughput::Elements(access_count as u64));

    // Baseline: HashMap region lookup
    group.bench_function("baseline_hashmap_10k_lookups", |b| {
        // Setup: HashMap with 8 regions
        let mut regions = HashMap::new();
        for i in 0..8 {
            regions.insert(i, (i as u64 * 4096, 4096u32)); // (base_offset, capacity)
        }

        b.iter(|| {
            for i in 0..access_count {
                let region_idx = i % 8;
                let region_meta = regions.get(&region_idx).unwrap();
                black_box(region_meta);
            }
        });
    });

    // Capsule-native: Array region access
    group.bench_function("capsule_native_array_10k_lookups", |b| {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("bench_capsule_access.bin");

        // Setup: MmapManager with 8 regions
        let layout = MmapLayout::new(4096 * 8, 8).unwrap();
        let manager = MmapManager::new(&path, &layout).unwrap();

        b.iter(|| {
            for i in 0..access_count {
                let region_idx = i % 8;
                let region = manager.region(region_idx).unwrap();
                black_box((region.base_offset(), region.capacity()));
            }
        });

        // Cleanup
        drop(manager);
        let _ = std::fs::remove_file(&path);
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

#[cfg(feature = "mmap-persistence")]
criterion_group!(
    benches,
    bench_file_initialization,
    bench_region_allocation,
    bench_concurrent_allocation,
    bench_fsync_latency,
    bench_region_access,
);

#[cfg(not(feature = "mmap-persistence"))]
criterion_group!(benches,);

criterion_main!(benches);
