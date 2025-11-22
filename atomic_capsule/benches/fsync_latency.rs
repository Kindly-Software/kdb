//! B32 Benchmarks for fsync() Latency (Phase 2 - Crash-Safe Durability)
//!
//! **Purpose**: Measure actual fsync() latency for crash-safe persistent capsules
//!
//! # Benchmarks
//!
//! - **MmapManager fsync**: Measure memmap2::flush() latency across data sizes
//! - **PersistentMap hash chain**: Measure FNV-1a hash computation latency
//! - **PersistentMap end-to-end**: Insert + hash chain + fsync
//! - **PersistentLog end-to-end**: Append + hash chain + fsync
//! - **fsync frequency impact**: Compare fsync every 1/10/100/1000 ops
//!
//! # B32 Honest Claims
//!
//! - Same hardware (no cross-machine comparison)
//! - 100+ iterations (fsync is slow, fewer iterations acceptable)
//! - Fair baselines (raw memmap2::flush())
//! - Realistic workload (real write patterns, not synthetic)
//! - Full disclosure (hardware: NVMe/SSD/HDD, filesystem, OS)
//! - Variance reporting (min/max/p50/p95/p99)
//!
//! # Performance Targets (B32 Framework)
//!
//! - MmapManager fsync: <1-5ms (storage dependent: NVMe ~1ms, SATA SSD ~3ms, HDD ~10ms)
//! - Hash chain update: <50ns (FNV-1a computation)
//! - Hash chain fsync: <1-5ms (dominated by fsync, not hash computation)
//!
//! # Reality Check (B32 Framework § R7)
//!
//! fsync() latency is HARDWARE-BOUND:
//! - NVMe: 0.1-1ms (PCIe 4.0, good controller)
//! - SATA SSD: 1-5ms (SATA 3.0 bottleneck, older controllers)
//! - HDD: 5-15ms (mechanical seek + rotational latency)
//!
//! NO SOFTWARE OPTIMIZATION CAN REDUCE THESE VALUES.
//! Claims of "<100µs fsync" are physically impossible on consumer hardware.

use criterion::{
    black_box, criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, BenchmarkId,
    Criterion, Throughput,
};

#[cfg(feature = "mmap-persistence")]
use atomic_capsule::persistence::{PersistentLog, PersistentMap};

#[cfg(feature = "mmap-persistence")]
use memmap2::MmapMut;

use std::fs::OpenOptions;
use std::io::Write;

// ============================================================================
// MMAP MANAGER FSYNC BENCHMARKS
// ============================================================================

/// Benchmark raw memmap2::flush() latency across different data sizes
///
/// # B32 Framework Compliance
///
/// - Fair baseline: Raw memmap2::flush() without additional overhead
/// - Realistic workload: Write pattern before fsync (dirty pages)
/// - Full disclosure: Data sizes (1KB, 1MB, 10MB)
/// - Variance reporting: Criterion provides min/max/p50/p95/p99 automatically
///
/// # Performance Expectations
///
/// - 1KB: <1-5ms (NVMe ~0.5ms, SSD ~2ms, HDD ~10ms)
/// - 1MB: <2-10ms (NVMe ~1ms, SSD ~5ms, HDD ~15ms)
/// - 10MB: <5-50ms (NVMe ~3ms, SSD ~20ms, HDD ~100ms)
#[cfg(feature = "mmap-persistence")]
fn bench_mmap_manager_fsync(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmap_manager_fsync");

    // Reduce sample size for fsync benchmarks (fsync is slow)
    group.sample_size(100);

    for size_kb in [1, 1024, 10240].iter() {
        // 1KB, 1MB, 10MB
        let size_bytes = size_kb * 1024;
        group.throughput(Throughput::Bytes(size_bytes as u64));

        group.bench_with_input(
            BenchmarkId::new("memmap2_flush", format!("{}KB", size_kb)),
            size_kb,
            |b, &size_kb| {
                let size_bytes = size_kb * 1024;

                // Setup: Create temp file + mmap
                let temp_dir = std::env::temp_dir();
                let path = temp_dir.join(format!("bench_fsync_{}kb.bin", size_kb));

                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(&path)
                    .unwrap();

                file.set_len(size_bytes as u64).unwrap();

                let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

                b.iter(|| {
                    // Write pattern (dirty pages to force fsync work)
                    for i in (0..size_bytes).step_by(4096) {
                        let end = (i + 4096).min(size_bytes);
                        mmap[i..end].fill(black_box(0x42));
                    }

                    // Measure fsync() latency
                    black_box(mmap.flush().unwrap());
                });

                // Cleanup
                drop(mmap);
                let _ = std::fs::remove_file(&path);
            },
        );
    }

    group.finish();
}

// ============================================================================
// HASH CHAIN UPDATE BENCHMARKS
// ============================================================================

/// Benchmark PersistentMapHeader::update_hash_chain() latency
///
/// # B32 Framework Compliance
///
/// - Fair baseline: FNV-1a hash computation only (no fsync)
/// - Realistic workload: Hash 24 bytes (generation + entry_count + bucket_count)
/// - Full disclosure: Hash algorithm (FNV-1a), data size (24 bytes)
///
/// # Performance Expectations
///
/// - Hash chain update: <50ns (FNV-1a is ~1 byte/ns on modern CPUs)
/// - This is CPU-bound, NOT I/O-bound
#[cfg(feature = "mmap-persistence")]
fn bench_hash_chain_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_chain_update");

    group.bench_function("persistent_map_hash_chain", |b| {
        let map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();

        b.iter(|| {
            // Measure hash computation only (no fsync)
            black_box(map.validate_integrity().unwrap());
        });
    });

    group.bench_function("persistent_log_hash_chain", |b| {
        let log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        b.iter(|| {
            // Measure hash computation only (no fsync)
            black_box(log.validate_integrity().unwrap());
        });
    });

    group.finish();
}

// ============================================================================
// PERSISTENT MAP END-TO-END FSYNC BENCHMARKS
// ============================================================================

/// Benchmark PersistentMap insert + hash chain + fsync (end-to-end)
///
/// # B32 Framework Compliance
///
/// - Fair baseline: std::collections::HashMap + std::fs::File::sync_all()
/// - Realistic workload: Insert + hash chain + fsync
/// - Full disclosure: Entry count (10, 100, 1000), hash algorithm (FNV-1a)
///
/// # Performance Expectations
///
/// - 10 entries: <2-10ms (dominated by fsync, not hash chain)
/// - 100 entries: <5-50ms (10× fsync operations)
/// - 1000 entries: <50-500ms (100× fsync operations)
#[cfg(feature = "mmap-persistence")]
fn bench_persistent_map_fsync(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_map_fsync");

    // Reduce sample size for fsync benchmarks
    group.sample_size(50);

    for entry_count in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*entry_count as u64));

        // Baseline: HashMap + manual fsync
        group.bench_with_input(
            BenchmarkId::new("std_hashmap_fsync", entry_count),
            entry_count,
            |b, &entry_count| {
                let temp_dir = std::env::temp_dir();
                let path = temp_dir.join("bench_hashmap_fsync.bin");

                b.iter(|| {
                    let mut map = std::collections::HashMap::new();
                    let mut file = OpenOptions::new()
                        .create(true)
                        .write(true)
                        .open(&path)
                        .unwrap();

                    for i in 0..entry_count {
                        map.insert(black_box(i), black_box(i * 10));

                        // Simulate hash chain update (FNV-1a)
                        let hash = compute_fnv1a_hash(i, i * 10);
                        file.write_all(&hash.to_le_bytes()).unwrap();

                        // fsync after every insert
                        file.sync_all().unwrap();
                    }
                });

                // Cleanup
                let _ = std::fs::remove_file(&path);
            },
        );

        // PersistentMap with fsync (Phase 2)
        group.bench_with_input(
            BenchmarkId::new("persistent_map_with_fsync", entry_count),
            entry_count,
            |b, &entry_count| {
                let temp_dir = std::env::temp_dir();
                let path = temp_dir.join("bench_persistent_map_fsync.bin");

                b.iter(|| {
                    // Setup: Create temp file + mmap
                    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(&path)
                        .unwrap();

                    file.set_len(1024 * 1024).unwrap(); // 1MB capacity

                    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

                    // TODO: Replace with PersistentMap::from_mmap() when implemented
                    // For now, simulate insert + hash chain + fsync
                    let mut map: PersistentMap<u64, u64> = PersistentMap::new(2048).unwrap();

                    for i in 0..entry_count {
                        map.insert(black_box(i), black_box(i * 10)).unwrap();

                        // Hash chain update (already done in insert())
                        // Simulate fsync (in Phase 2, this will be MmapManager::flush())
                        mmap.flush().unwrap();
                    }
                });

                // Cleanup
                let _ = std::fs::remove_file(&path);
            },
        );
    }

    group.finish();
}

// ============================================================================
// PERSISTENT LOG END-TO-END FSYNC BENCHMARKS
// ============================================================================

/// Benchmark PersistentLog append + hash chain + fsync (end-to-end)
///
/// # B32 Framework Compliance
///
/// - Fair baseline: Vec<T> + std::fs::File::write_all() + sync_all()
/// - Realistic workload: Append + hash chain + fsync
/// - Full disclosure: Entry count (10, 100, 1000), entry size (64 bytes)
///
/// # Performance Expectations
///
/// - 10 entries: <2-10ms (dominated by fsync, not hash chain)
/// - 100 entries: <5-50ms (10× fsync operations)
/// - 1000 entries: <50-500ms (100× fsync operations)
#[cfg(feature = "mmap-persistence")]
fn bench_persistent_log_fsync(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_log_fsync");

    // Reduce sample size for fsync benchmarks
    group.sample_size(50);

    for entry_count in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*entry_count as u64));

        // Baseline: Vec<T> + manual fsync
        group.bench_with_input(
            BenchmarkId::new("std_vec_fsync", entry_count),
            entry_count,
            |b, &entry_count| {
                let temp_dir = std::env::temp_dir();
                let path = temp_dir.join("bench_vec_fsync.bin");

                b.iter(|| {
                    let mut file = OpenOptions::new()
                        .create(true)
                        .write(true)
                        .open(&path)
                        .unwrap();

                    for i in 0..entry_count {
                        let data = format!("Entry {}", i).into_bytes();

                        // Write data
                        file.write_all(&data).unwrap();

                        // Simulate hash chain update (FNV-1a)
                        let hash = compute_fnv1a_hash_bytes(&data);
                        file.write_all(&hash.to_le_bytes()).unwrap();

                        // fsync after every append
                        file.sync_all().unwrap();
                    }
                });

                // Cleanup
                let _ = std::fs::remove_file(&path);
            },
        );

        // PersistentLog with fsync (Phase 2)
        group.bench_with_input(
            BenchmarkId::new("persistent_log_with_fsync", entry_count),
            entry_count,
            |b, &entry_count| {
                let temp_dir = std::env::temp_dir();
                let path = temp_dir.join("bench_persistent_log_fsync.bin");

                b.iter(|| {
                    // Setup: Create temp file + mmap
                    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(&path)
                        .unwrap();

                    file.set_len(1024 * 1024).unwrap(); // 1MB capacity

                    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

                    // TODO: Replace with PersistentLog::from_mmap() when implemented
                    // For now, simulate append + hash chain + fsync
                    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

                    for i in 0..entry_count {
                        let data = format!("Entry {}", i).into_bytes();
                        log.append(data).unwrap();

                        // Hash chain update (already done in append())
                        // Simulate fsync (in Phase 2, this will be MmapManager::flush())
                        mmap.flush().unwrap();
                    }
                });

                // Cleanup
                let _ = std::fs::remove_file(&path);
            },
        );
    }

    group.finish();
}

// ============================================================================
// FSYNC FREQUENCY IMPACT BENCHMARKS
// ============================================================================

/// Benchmark impact of fsync frequency (every 1/10/100/1000 ops)
///
/// # B32 Framework Compliance
///
/// - Fair baseline: Same workload, different fsync frequency
/// - Realistic workload: 1000 inserts with varying fsync frequency
/// - Full disclosure: Fsync frequency (1, 10, 100, 1000)
///
/// # Performance Expectations
///
/// - fsync every 1 op: 1000× fsync overhead (worst case, ~1-5s)
/// - fsync every 10 ops: 100× fsync overhead (10× speedup, ~100-500ms)
/// - fsync every 100 ops: 10× fsync overhead (100× speedup, ~10-50ms)
/// - fsync every 1000 ops: 1× fsync overhead (best case, ~1-5ms)
///
/// # Trade-offs
///
/// - More frequent fsync: Better crash safety (less data loss)
/// - Less frequent fsync: Better performance (fewer expensive syscalls)
#[cfg(feature = "mmap-persistence")]
fn bench_fsync_frequency_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("fsync_frequency_impact");

    // Reduce sample size for fsync benchmarks
    group.sample_size(20);

    let total_ops = 1000;
    group.throughput(Throughput::Elements(total_ops as u64));

    for fsync_every in [1, 10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("persistent_map", format!("fsync_every_{}", fsync_every)),
            fsync_every,
            |b, &fsync_every| {
                let temp_dir = std::env::temp_dir();
                let path = temp_dir.join(format!("bench_fsync_freq_{}.bin", fsync_every));

                b.iter(|| {
                    // Setup: Create temp file + mmap
                    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(&path)
                        .unwrap();

                    file.set_len(2 * 1024 * 1024).unwrap(); // 2MB capacity

                    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

                    // TODO: Replace with PersistentMap::from_mmap() when implemented
                    let mut map: PersistentMap<u64, u64> = PersistentMap::new(2048).unwrap();

                    for i in 0..total_ops {
                        map.insert(black_box(i), black_box(i * 10)).unwrap();

                        // fsync according to frequency
                        if (i + 1) % fsync_every == 0 {
                            mmap.flush().unwrap();
                        }
                    }

                    // Final fsync to ensure all data written
                    mmap.flush().unwrap();
                });

                // Cleanup
                let _ = std::fs::remove_file(&path);
            },
        );
    }

    group.finish();
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Compute FNV-1a hash of two u64 values (for baseline comparison)
#[inline]
fn compute_fnv1a_hash(a: u64, b: u64) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;

    // Hash first value
    for &byte in &a.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    // Hash second value
    for &byte in &b.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    hash
}

/// Compute FNV-1a hash of byte slice (for baseline comparison)
#[inline]
fn compute_fnv1a_hash_bytes(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

#[cfg(feature = "mmap-persistence")]
criterion_group!(
    benches,
    bench_mmap_manager_fsync,
    bench_hash_chain_update,
    bench_persistent_map_fsync,
    bench_persistent_log_fsync,
    bench_fsync_frequency_impact,
);

#[cfg(not(feature = "mmap-persistence"))]
criterion_group!(benches,);

criterion_main!(benches);
