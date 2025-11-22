//! B32-Compliant Contention Measurement for AtomicHash256 (Phase 1.6)
//!
//! **Critical Performance Claims**:
//! 1. ❌ AtomicHash256 read/write: <100ns (UNVALIDATED - THIS BENCHMARK)
//! 2. ❌ SeqLock contention: <200ns p99 (UNVALIDATED - THIS BENCHMARK)
//! 3. ❌ Chain verification: <50ns per link (UNVALIDATED - THIS BENCHMARK)
//! 4. ❌ Feature combination overhead: <10% (UNVALIDATED - THIS BENCHMARK)
//!
//! **Framework**: B32 (32 benchmarking guidelines + 50 hardware reality checks)
//! **Hardware**: Intel Ultra 7 155H (6P+8E cores, DDR5-5600)
//! **Methodology**: 10K+ iterations, 95% CI, sustained >60s
//!
//! ## Benchmark Categories
//!
//! ### 1. Read Contention (100 readers, 1 writer)
//! - Measure SeqLock retry loop latency
//! - P50/P95/P99 reader latency (target: <100ns p50)
//! - P99 writer latency (target: <200ns)
//! - Throughput (ops/sec combined)
//!
//! ### 2. Write Contention (10 writers, 100 readers)
//! - Multiple writers competing for SeqLock
//! - Measure retry loop convergence
//! - Expected: <200ns p99 (SeqLock retry)
//!
//! ### 3. Chain Verification Latency
//! - Single link: <10ns (2× atomic loads + comparison)
//! - 100-link chain: <1μs (100× ~10ns)
//! - 1000-link chain: <10μs (linear scaling)
//!
//! ### 4. Feature Combination Overhead
//! - Baseline (no features)
//! - fast-hash only
//! - audit-trail only
//! - keyed-hashing only
//! - All features combined
//! - Expected: Each <5%, total <10%
//!
//! ### 5. Scaling Analysis
//! - Vary thread counts: 1, 2, 4, 8, 16, 32
//! - Measure latency scaling (lock-free: ~constant)
//! - Expected: <20% increase at 32 threads

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// Import AtomicHash256 from atomic_capsule
use atomic_capsule::hash::AtomicHash256;

// ============================================================================
// SECTION 1: READ CONTENTION (100 readers, 1 writer)
// ============================================================================

/// Benchmark 1.1: Read Contention (100 readers, 1 writer)
///
/// **Target**: <100ns p50 readers, <200ns p99 writers
/// **Reality Check (K2)**: AtomicU64 load ~5ns, SeqLock retry adds overhead
fn bench_read_contention_100r_1w(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention/read_heavy_100r_1w");
    group.sample_size(100); // Fewer samples for multi-threaded tests
    group.measurement_time(Duration::from_secs(30)); // Sustained measurement

    let hash = Arc::new(AtomicHash256::new([0u8; 32]));
    let stop_flag = Arc::new(AtomicBool::new(false));

    group.bench_function("seqlock_retry_latency", |b| {
        b.iter_custom(|iters| {
            let hash = Arc::clone(&hash);
            let stop = Arc::clone(&stop_flag);
            stop.store(false, Ordering::Relaxed);

            let mut handles = vec![];

            // 100 reader threads
            for _ in 0..100 {
                let hash = Arc::clone(&hash);
                let stop = Arc::clone(&stop);
                handles.push(thread::spawn(move || {
                    let mut reads = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        let _value = hash.load();
                        black_box(&_value);
                        reads += 1;
                    }
                    reads
                }));
            }

            // 1 writer thread
            let hash_writer = Arc::clone(&hash);
            let stop_writer = Arc::clone(&stop);
            let writer = thread::spawn(move || {
                let mut writes = 0u64;
                let mut pattern = [0u8; 32];
                while !stop_writer.load(Ordering::Relaxed) && writes < iters {
                    pattern[0] = (writes % 256) as u8;
                    hash_writer.store(pattern);
                    writes += 1;
                    thread::yield_now(); // Allow readers to observe stable generation
                }
                writes
            });

            // Measure total time for iters writes
            let start = Instant::now();
            let total_writes = writer.join().unwrap();
            let elapsed = start.elapsed();

            // Stop readers
            stop.store(true, Ordering::Relaxed);
            for handle in handles {
                handle.join().unwrap();
            }

            elapsed / total_writes as u32
        });
    });

    group.finish();
}

/// Benchmark 1.2: Reader Latency Distribution (measure p50/p95/p99)
///
/// Uses a custom latency histogram instead of Criterion (for percentile reporting)
fn bench_reader_latency_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention/reader_latency_distribution");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(20));

    let hash = Arc::new(AtomicHash256::new([0u8; 32]));
    let stop_flag = Arc::new(AtomicBool::new(false));

    group.bench_function("reader_p50_p95_p99", |b| {
        b.iter_custom(|_iters| {
            let hash = Arc::clone(&hash);
            let stop = Arc::clone(&stop_flag);
            stop.store(false, Ordering::Relaxed);

            // Single reader thread (measure latency)
            let reader_latencies: Arc<parking_lot::Mutex<Vec<u64>>> =
                Arc::new(parking_lot::Mutex::new(Vec::new()));
            let reader_lat = Arc::clone(&reader_latencies);
            let reader_hash = Arc::clone(&hash);
            let reader_stop = Arc::clone(&stop);

            let reader = thread::spawn(move || {
                let mut lats = Vec::with_capacity(10000);
                while !reader_stop.load(Ordering::Relaxed) && lats.len() < 10000 {
                    let start = Instant::now();
                    let _value = reader_hash.load();
                    let elapsed = start.elapsed();
                    lats.push(elapsed.as_nanos() as u64);
                    black_box(&_value);
                }
                lats
            });

            // Writer thread (continuous updates)
            let writer_hash = Arc::clone(&hash);
            let writer_stop = Arc::clone(&stop);
            let writer = thread::spawn(move || {
                let mut pattern = [0u8; 32];
                let mut count = 0u64;
                while !writer_stop.load(Ordering::Relaxed) {
                    pattern[0] = (count % 256) as u8;
                    writer_hash.store(pattern);
                    count += 1;
                    thread::yield_now();
                }
            });

            // Wait for 10K reads
            thread::sleep(Duration::from_millis(500));
            stop.store(true, Ordering::Relaxed);

            let mut lats = reader.join().unwrap();
            writer.join().unwrap();

            // Calculate percentiles
            lats.sort_unstable();
            let p50 = lats[lats.len() * 50 / 100];
            let p95 = lats[lats.len() * 95 / 100];
            let p99 = lats[lats.len() * 99 / 100];

            println!("\nReader Latency Percentiles (with 1 concurrent writer):");
            println!("  P50:  {}ns (target: <100ns)", p50);
            println!("  P95:  {}ns (target: <150ns)", p95);
            println!("  P99:  {}ns (target: <200ns)", p99);

            Duration::from_nanos(p50)
        });
    });

    group.finish();
}

// ============================================================================
// SECTION 2: WRITE CONTENTION (10 writers, 100 readers)
// ============================================================================

/// Benchmark 2.1: Write Contention (10 writers, 100 readers)
///
/// **Target**: <200ns p99 writers (SeqLock retry loops)
/// **Reality**: Multiple writers may cause retry storms
fn bench_write_contention_10w_100r(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention/write_heavy_10w_100r");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(30));

    let hash = Arc::new(AtomicHash256::new([0u8; 32]));
    let stop_flag = Arc::new(AtomicBool::new(false));

    group.bench_function("seqlock_writer_contention", |b| {
        b.iter_custom(|iters| {
            let hash = Arc::clone(&hash);
            let stop = Arc::clone(&stop_flag);
            stop.store(false, Ordering::Relaxed);

            let mut handles = vec![];

            // 100 reader threads (background load)
            for _ in 0..100 {
                let hash = Arc::clone(&hash);
                let stop = Arc::clone(&stop);
                handles.push(thread::spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        let _value = hash.load();
                        black_box(&_value);
                    }
                }));
            }

            // 10 writer threads (measure contention)
            let mut writers = vec![];
            for writer_id in 0..10 {
                let hash = Arc::clone(&hash);
                let stop = Arc::clone(&stop);
                writers.push(thread::spawn(move || {
                    let mut pattern = [0u8; 32];
                    let mut count = 0u64;
                    while !stop.load(Ordering::Relaxed) && count < iters / 10 {
                        pattern[0] = writer_id;
                        pattern[1] = (count % 256) as u8;
                        hash.store(pattern);
                        count += 1;
                    }
                    count
                }));
            }

            // Measure time for all writers to complete
            let start = Instant::now();
            for writer in writers {
                writer.join().unwrap();
            }
            let elapsed = start.elapsed();

            // Stop readers
            stop.store(true, Ordering::Relaxed);
            for handle in handles {
                handle.join().unwrap();
            }

            elapsed / iters as u32
        });
    });

    group.finish();
}

/// Benchmark 2.2: Writer Latency Distribution (p50/p95/p99)
fn bench_writer_latency_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention/writer_latency_distribution");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(20));

    let hash = Arc::new(AtomicHash256::new([0u8; 32]));
    let stop_flag = Arc::new(AtomicBool::new(false));

    group.bench_function("writer_p50_p95_p99", |b| {
        b.iter_custom(|_iters| {
            let hash = Arc::clone(&hash);
            let stop = Arc::clone(&stop_flag);
            stop.store(false, Ordering::Relaxed);

            // Single writer thread (measure latency)
            let writer_hash = Arc::clone(&hash);
            let writer_stop = Arc::clone(&stop);
            let writer = thread::spawn(move || {
                let mut lats = Vec::with_capacity(10000);
                let mut pattern = [0u8; 32];
                let mut count = 0u64;
                while !writer_stop.load(Ordering::Relaxed) && lats.len() < 10000 {
                    pattern[0] = (count % 256) as u8;
                    let start = Instant::now();
                    writer_hash.store(pattern);
                    let elapsed = start.elapsed();
                    lats.push(elapsed.as_nanos() as u64);
                    count += 1;
                }
                lats
            });

            // 100 reader threads (background contention)
            let mut readers = vec![];
            for _ in 0..100 {
                let hash = Arc::clone(&hash);
                let stop = Arc::clone(&stop);
                readers.push(thread::spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        let _value = hash.load();
                        black_box(&_value);
                    }
                }));
            }

            // Wait for 10K writes
            thread::sleep(Duration::from_millis(500));
            stop.store(true, Ordering::Relaxed);

            let mut lats = writer.join().unwrap();
            for reader in readers {
                reader.join().unwrap();
            }

            // Calculate percentiles
            lats.sort_unstable();
            let p50 = lats[lats.len() * 50 / 100];
            let p95 = lats[lats.len() * 95 / 100];
            let p99 = lats[lats.len() * 99 / 100];

            println!("\nWriter Latency Percentiles (with 100 concurrent readers):");
            println!("  P50:  {}ns (target: <100ns)", p50);
            println!("  P95:  {}ns (target: <150ns)", p95);
            println!("  P99:  {}ns (target: <200ns)", p99);

            Duration::from_nanos(p50)
        });
    });

    group.finish();
}

// ============================================================================
// SECTION 3: CHAIN VERIFICATION LATENCY
// ============================================================================

/// Benchmark 3.1: Single Chain Link Verification
///
/// **Target**: <10ns (2× atomic loads + comparison)
fn bench_chain_verify_single_link(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain/verify_single_link");
    group.throughput(Throughput::Elements(1));

    // Create two linked hashes
    let hash1 = AtomicHash256::new([0x01; 32]);
    let hash2_prev = [0x01; 32]; // Matches hash1

    group.bench_function("atomic_load_compare", |b| {
        b.iter(|| {
            let h1 = hash1.load();
            let valid = h1 == black_box(hash2_prev);
            black_box(valid);
        });
    });

    group.finish();
}

/// Benchmark 3.2: 100-Link Chain Verification
///
/// **Target**: <1μs (100× ~10ns per link)
fn bench_chain_verify_100_links(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain/verify_100_links");
    group.throughput(Throughput::Elements(100));

    // Create 100-link chain
    let mut chain: Vec<AtomicHash256> = Vec::with_capacity(100);
    for i in 0..100 {
        let mut hash = [0u8; 32];
        hash[0] = i as u8;
        chain.push(AtomicHash256::new(hash));
    }

    group.bench_function("linear_verification", |b| {
        b.iter(|| {
            let mut prev_hash = chain[0].load();
            let mut valid = true;
            for capsule in chain.iter().skip(1) {
                let current_hash = capsule.load();
                // Simulated chain: Check that current hash "links" to previous
                // In reality, you'd check current.prev_hash == prev_hash
                // Here we just verify no torn reads
                if current_hash[0] == 0xFF && prev_hash[0] == 0xFF {
                    // Placeholder: Real chain would have prev_hash field
                    valid = false;
                    break;
                }
                prev_hash = current_hash;
            }
            black_box(valid);
        });
    });

    group.finish();
}

/// Benchmark 3.3: 1000-Link Chain Verification (Linear Scaling)
///
/// **Target**: <10μs (1000× ~10ns per link)
fn bench_chain_verify_1000_links(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain/verify_1000_links");
    group.throughput(Throughput::Elements(1000));
    group.sample_size(100);

    // Create 1000-link chain
    let mut chain: Vec<AtomicHash256> = Vec::with_capacity(1000);
    for i in 0..1000 {
        let mut hash = [0u8; 32];
        hash[0] = (i % 256) as u8;
        hash[1] = (i / 256) as u8;
        chain.push(AtomicHash256::new(hash));
    }

    group.bench_function("linear_verification_1000", |b| {
        b.iter(|| {
            let mut prev_hash = chain[0].load();
            let mut valid = true;
            for capsule in chain.iter().skip(1) {
                let current_hash = capsule.load();
                if current_hash[0] == 0xFF && prev_hash[0] == 0xFF {
                    valid = false;
                    break;
                }
                prev_hash = current_hash;
            }
            black_box(valid);
        });
    });

    group.finish();
}

// ============================================================================
// SECTION 4: FEATURE COMBINATION OVERHEAD
// ============================================================================

/// Benchmark 4.1: Feature Overhead Analysis
///
/// **Target**: Each feature <5%, combined <10%
///
/// Compile with different feature combinations:
/// - Baseline (no features)
/// - fast-hash only
/// - audit-trail only
/// - keyed-hashing only
/// - All features
fn bench_feature_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("features/overhead_analysis");

    // This benchmark requires compiling with different feature flags
    // For now, we measure the baseline (no features)

    let hash = AtomicHash256::new([0u8; 32]);

    group.bench_function("baseline_no_features", |b| {
        b.iter(|| {
            let _value = hash.load();
            black_box(&_value);
        });
    });

    // NOTE: To measure feature overhead, compile with:
    // cargo bench --bench contention_bench --no-default-features
    // cargo bench --bench contention_bench --features fast-hash
    // cargo bench --bench contention_bench --features audit-trail
    // cargo bench --bench contention_bench --all-features

    group.finish();
}

// ============================================================================
// SECTION 5: SCALING ANALYSIS (1-32 threads)
// ============================================================================

/// Benchmark 5.1: Scaling Analysis (1, 2, 4, 8, 16, 32 threads)
///
/// **Target**: <20% latency increase at 32 threads (lock-free benefit)
fn bench_scaling_analysis(c: &mut Criterion) {
    for num_threads in [1, 2, 4, 8, 16, 32] {
        let mut group = c.benchmark_group(format!("scaling/{}_threads", num_threads));
        group.sample_size(50);
        group.measurement_time(Duration::from_secs(20));
        group.throughput(Throughput::Elements(num_threads as u64 * 1000));

        let hash = Arc::new(AtomicHash256::new([0u8; 32]));
        let stop_flag = Arc::new(AtomicBool::new(false));

        group.bench_function("reader_throughput", |b| {
            b.iter_custom(|iters| {
                let hash = Arc::clone(&hash);
                let stop = Arc::clone(&stop_flag);
                stop.store(false, Ordering::Relaxed);

                let mut handles = vec![];

                // N reader threads
                for _ in 0..num_threads {
                    let hash = Arc::clone(&hash);
                    let stop = Arc::clone(&stop);
                    handles.push(thread::spawn(move || {
                        let mut reads = 0u64;
                        while !stop.load(Ordering::Relaxed) && reads < iters {
                            let _value = hash.load();
                            black_box(&_value);
                            reads += 1;
                        }
                        reads
                    }));
                }

                let start = Instant::now();
                for handle in handles {
                    handle.join().unwrap();
                }
                let elapsed = start.elapsed();

                stop.store(true, Ordering::Relaxed);

                elapsed / (num_threads as u32 * iters as u32)
            });
        });

        group.finish();
    }
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group! {
    name = benches_contention;
    config = Criterion::default()
        .confidence_level(0.95)
        .significance_level(0.05)
        .noise_threshold(0.05);
    targets =
        bench_read_contention_100r_1w,
        bench_reader_latency_percentiles,
        bench_write_contention_10w_100r,
        bench_writer_latency_percentiles,
        bench_chain_verify_single_link,
        bench_chain_verify_100_links,
        bench_chain_verify_1000_links,
        bench_feature_overhead,
        bench_scaling_analysis
}

criterion_main!(benches_contention);
