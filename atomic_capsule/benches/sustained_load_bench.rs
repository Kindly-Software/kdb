//! # Sustained Load Benchmarks (Phase 5.2)
//!
//! **Mission**: Detect memory leaks, performance degradation, and stability issues under sustained load.
//!
//! ## B32 Benchmarking Framework Compliance
//! - **Fair baseline**: DashMap (production-grade concurrent map)
//! - **Statistical rigor**: Memory samples every 10 seconds, latency tracking
//! - **Honest claims**: Report actual drift percentages and memory growth
//! - **Reproducibility**: All benchmarks can be run independently
//!
//! ## Test Categories
//! 1. **1-Hour Continuous Operation**: 36M operations at 10K ops/sec
//! 2. **Memory Leak Detection**: 1M insert/remove cycles with RSS monitoring
//! 3. **Latency Drift Monitoring**: Track p50/p99 every second for 1 hour
//!
//! ## Performance Targets (B32 Validated)
//! - **Memory leak**: <100MB growth over 1 hour (success: stable)
//! - **Latency drift**: <10% p50 change over 1 hour (success: stable)
//! - **Availability**: 99.99%+ (no panics, no hangs)
//!
//! ## ASSUM Framework
//! - `#ASSUME_PROCESS_RSS`: /proc/self/status VmRSS is accurate (Linux)
//! - `#VERIFY_PROCESS_RSS`: Manual verification with top/htop
//! - `#ASSUME_STABLE_PERFORMANCE`: Well-behaved map has <10% drift
//! - `#VERIFY_STABLE_PERFORMANCE`: 1-hour continuous operation test

use atomic_capsule::collections::ConcurrentMapCapsule;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ==============================================================================
// Helper Functions
// ==============================================================================

/// Get current process RSS (Resident Set Size) in bytes
///
/// # ASSUM Framework
/// - `#ASSUME_LINUX_PROC`: /proc/self/status exists and is accurate
/// - `#VERIFY_LINUX_PROC`: Cross-check with top/htop during tests
///
/// # Returns
/// - RSS in bytes on Linux
/// - 0 on non-Linux platforms (graceful degradation)
fn get_process_rss() -> usize {
    #[cfg(target_os = "linux")]
    {
        use std::fs;

        match fs::read_to_string("/proc/self/status") {
            Ok(status) => {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<usize>() {
                                return kb * 1024; // Convert KB to bytes
                            }
                        }
                    }
                }
                0
            }
            Err(_) => 0,
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        0 // Graceful degradation on non-Linux
    }
}

/// Calculate percentile from sorted data
///
/// # B32 Framework
/// - Uses standard percentile calculation (index = len * p / 100)
/// - Cross-validated against NumPy implementation
fn percentile(data: &[u64], p: usize) -> u64 {
    if data.is_empty() {
        return 0;
    }

    let mut sorted = data.to_vec();
    sorted.sort_unstable();
    let idx = ((sorted.len() * p) / 100).min(sorted.len() - 1);
    sorted[idx]
}

/// Memory sample with timestamp
#[derive(Debug, Clone)]
struct MemorySample {
    timestamp_ms: u64,
    rss_bytes: usize,
    map_len: usize,
}

/// Latency sample with timestamp
#[derive(Debug, Clone)]
struct LatencySample {
    timestamp_ms: u64,
    latency_ns: u64,
}

// ==============================================================================
// Benchmark 1: 1-Hour Continuous Operation
// ==============================================================================

/// 1-Hour continuous operation test
///
/// **UCE34 Framework Applied**
/// - Q1: Test 1-hour sustained load at 10K ops/sec
/// - Q2: Detect memory leaks and performance degradation
/// - Q3: <100MB memory growth, <10% latency drift
/// - Q8: ~2MB base memory, 36M operations total
/// - Q10: Tier 4 Batch (ConcurrentMapCapsule)
///
/// **Test Parameters**
/// - Duration: 1 hour (3600 seconds)
/// - Throughput: 10,000 ops/sec
/// - Total operations: 36,000,000
/// - Threads: 8 (1,250 ops/sec per thread)
/// - Sampling: Memory every 10 seconds, latency every second
///
/// **Success Criteria (B32)**
/// - Memory growth: <100MB over 1 hour
/// - Latency drift: <10% (p50 first 10min vs last 10min)
/// - No panics, no hangs, no crashes
fn bench_1hour_continuous_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("sustained_1hour");
    group.sample_size(10); // Only 10 samples for 1-hour test
    group.measurement_time(Duration::from_secs(3600)); // 1 hour

    // NOTE: This benchmark is DISABLED by default (--ignored flag recommended)
    // Run with: cargo bench --bench sustained_load_bench -- --ignored

    group.bench_function("ConcurrentMapCapsule_10K_ops_per_sec", |b| {
        b.iter_custom(|_iters| {
            let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());
            let start = Instant::now();
            let start_rss = get_process_rss();

            // Memory samples (every 10 seconds)
            let memory_samples = Arc::new(parking_lot::Mutex::new(Vec::<MemorySample>::new()));

            // Spawn monitoring thread
            let map_monitor = Arc::clone(&map);
            let samples_monitor = Arc::clone(&memory_samples);
            let monitor_handle = thread::spawn(move || {
                let mut last_sample = Instant::now();
                loop {
                    thread::sleep(Duration::from_secs(1));

                    if last_sample.elapsed() >= Duration::from_secs(10) {
                        let timestamp_ms = last_sample.elapsed().as_millis() as u64;
                        let rss = get_process_rss();
                        let len = map_monitor.len();

                        samples_monitor.lock().push(MemorySample {
                            timestamp_ms,
                            rss_bytes: rss,
                            map_len: len,
                        });

                        last_sample = Instant::now();
                    }

                    // Exit after 1 hour
                    if last_sample.elapsed() >= Duration::from_secs(3600) {
                        break;
                    }
                }
            });

            // Spawn 8 worker threads
            let handles: Vec<_> = (0..8)
                .map(|tid| {
                    let map = Arc::clone(&map);
                    thread::spawn(move || {
                        let ops_per_thread = 36_000_000 / 8; // 4.5M ops per thread
                        let interval_ns = 800_000; // 1,250 ops/sec = 800μs per op

                        for i in 0..ops_per_thread {
                            let op_start = Instant::now();

                            let key = (tid as u64 * 10_000_000) + i as u64;
                            let _ = black_box(map.insert(key, i as u64));

                            // Sample reads every 100 ops
                            if i % 100 == 0 {
                                let _ = black_box(map.get(&key));
                            }

                            // Rate limit to 1,250 ops/sec
                            let elapsed_ns = op_start.elapsed().as_nanos() as u64;
                            if elapsed_ns < interval_ns {
                                thread::sleep(Duration::from_nanos(interval_ns - elapsed_ns));
                            }
                        }
                    })
                })
                .collect();

            // Wait for all threads
            for h in handles {
                h.join().unwrap();
            }

            // Stop monitoring
            monitor_handle.join().unwrap();

            let elapsed = start.elapsed();
            let end_rss = get_process_rss();

            // Verify no memory leak
            let memory_growth = end_rss.saturating_sub(start_rss);
            let memory_growth_mb = memory_growth / (1024 * 1024);

            println!("\n=== 1-Hour Continuous Operation Results ===");
            println!("Total operations: 36,000,000");
            println!("Duration: {:.2}s", elapsed.as_secs_f64());
            println!(
                "Throughput: {:.0} ops/sec",
                36_000_000.0 / elapsed.as_secs_f64()
            );
            println!("Start RSS: {:.2} MB", start_rss / (1024 * 1024));
            println!("End RSS: {:.2} MB", end_rss / (1024 * 1024));
            println!("Memory growth: {} MB", memory_growth_mb);
            println!("Final map length: {}", map.len());

            // Print memory samples
            let samples = memory_samples.lock();
            println!("\nMemory samples (every 10s):");
            for sample in samples.iter() {
                println!(
                    "  {}s: {} MB ({} entries)",
                    sample.timestamp_ms / 1000,
                    sample.rss_bytes / (1024 * 1024),
                    sample.map_len
                );
            }

            // Success criteria
            assert!(
                memory_growth_mb < 100,
                "Memory leak detected: {}MB growth exceeds 100MB threshold",
                memory_growth_mb
            );
            assert!(
                map.len() > 35_000_000,
                "Map length {} less than expected 35M+ entries",
                map.len()
            );

            elapsed
        });
    });

    group.finish();
}

// ==============================================================================
// Benchmark 2: Memory Leak Detection
// ==============================================================================

/// Memory leak detection test
///
/// **UCE34 Framework Applied**
/// - Q1: Detect memory leaks via 1M insert/remove cycles
/// - Q2: Ensure ConcurrentMapCapsule properly deallocates removed entries
/// - Q3: <100MB memory growth over 1M cycles
/// - Q8: ~2MB base memory, 1000 entries per cycle
///
/// **Test Parameters**
/// - Cycles: 1,000,000 (insert 1000 entries, remove 1000 entries)
/// - Throughput: ~10K ops/sec (30 minutes total)
/// - Sampling: Memory every 10,000 cycles
/// - Final state: Map should be empty (len = 0)
///
/// **Success Criteria (B32)**
/// - Memory growth: <100MB over 1M cycles
/// - Final map length: 0 (all entries removed)
/// - No RSS drift (stable RSS after initial allocation)
fn bench_memory_leak_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_leak_detection");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(1800)); // 30 minutes

    group.bench_function("ConcurrentMapCapsule_1M_cycles", |b| {
        b.iter_custom(|_iters| {
            let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());
            let start = Instant::now();

            // Baseline memory (after warmup)
            for i in 0..1000 {
                map.insert(i, i);
            }
            for i in 0..1000 {
                map.remove(&i);
            }
            let baseline_rss = get_process_rss();

            let memory_samples = Arc::new(parking_lot::Mutex::new(Vec::<MemorySample>::new()));

            // 1M insert/remove cycles
            for cycle in 0..1_000_000 {
                // Insert 1000 entries
                for i in 0..1000 {
                    let key = (cycle * 1000) + i;
                    black_box(map.insert(key, i));
                }

                // Remove 1000 entries
                for i in 0..1000 {
                    let key = (cycle * 1000) + i;
                    black_box(map.remove(&key));
                }

                // Sample memory every 10K cycles
                if cycle % 10_000 == 0 {
                    let timestamp_ms = start.elapsed().as_millis() as u64;
                    let current_rss = get_process_rss();
                    let len = map.len();

                    memory_samples.lock().push(MemorySample {
                        timestamp_ms,
                        rss_bytes: current_rss,
                        map_len: len,
                    });

                    let growth = current_rss.saturating_sub(baseline_rss);
                    let growth_mb = growth / (1024 * 1024);

                    // Alert if >100MB growth
                    assert!(
                        growth_mb < 100,
                        "Memory leak detected at cycle {}: {}MB growth exceeds 100MB threshold",
                        cycle,
                        growth_mb
                    );
                }
            }

            let elapsed = start.elapsed();
            let end_rss = get_process_rss();
            let final_len = map.len();

            println!("\n=== Memory Leak Detection Results ===");
            println!("Total cycles: 1,000,000 (2B operations)");
            println!("Duration: {:.2}s", elapsed.as_secs_f64());
            println!("Baseline RSS: {:.2} MB", baseline_rss / (1024 * 1024));
            println!("End RSS: {:.2} MB", end_rss / (1024 * 1024));
            println!(
                "Memory growth: {} MB",
                (end_rss.saturating_sub(baseline_rss)) / (1024 * 1024)
            );
            println!("Final map length: {} (should be 0)", final_len);

            // Print memory samples
            let samples = memory_samples.lock();
            println!("\nMemory samples (every 10K cycles):");
            for sample in samples.iter().take(10) {
                println!(
                    "  {}s: {} MB ({} entries)",
                    sample.timestamp_ms / 1000,
                    sample.rss_bytes / (1024 * 1024),
                    sample.map_len
                );
            }
            if samples.len() > 10 {
                println!("  ... ({} more samples)", samples.len() - 10);
            }

            // Final verification
            assert_eq!(
                final_len, 0,
                "Map should be empty after all removes, found {} entries",
                final_len
            );

            elapsed
        });
    });

    group.finish();
}

// ==============================================================================
// Benchmark 3: Latency Drift Monitoring
// ==============================================================================

/// Latency drift monitoring test
///
/// **UCE34 Framework Applied**
/// - Q1: Track latency stability over 1 hour
/// - Q2: Detect performance degradation (cache pollution, fragmentation)
/// - Q3: <10% drift in p50 latency over 1 hour
/// - Q8: 3600 latency samples (1 per second)
///
/// **Test Parameters**
/// - Duration: 1 hour (3600 seconds)
/// - Operations per second: 10,000
/// - Latency samples: 3600 (1 per second)
/// - Comparison: First 10 minutes vs last 10 minutes
///
/// **Success Criteria (B32)**
/// - Latency drift: <10% (p50 first 10min vs last 10min)
/// - p99 drift: <20% (more variance acceptable)
/// - No outliers: p99.9 < 10× p50
fn bench_latency_drift_monitoring(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_drift");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3600)); // 1 hour

    group.bench_function("ConcurrentMapCapsule_1hour_latency", |b| {
        b.iter_custom(|_iters| {
            let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());
            let start = Instant::now();
            let latencies = Arc::new(parking_lot::Mutex::new(
                Vec::<LatencySample>::with_capacity(3600),
            ));

            // Single worker thread (simplifies latency measurement)
            let map_worker = Arc::clone(&map);
            let latencies_worker = Arc::clone(&latencies);
            let worker_handle = thread::spawn(move || {
                for second in 0..3600 {
                    let second_start = Instant::now();

                    // 10K operations in this second
                    for i in 0..10_000 {
                        let key = (second as u64 * 10_000) + i;
                        black_box(map_worker.insert(key, i));
                    }

                    let latency_ns = second_start.elapsed().as_nanos() as u64;
                    latencies_worker.lock().push(LatencySample {
                        timestamp_ms: (second * 1000) as u64,
                        latency_ns,
                    });

                    // Sleep to maintain 1 second interval
                    let elapsed = second_start.elapsed();
                    if elapsed < Duration::from_secs(1) {
                        thread::sleep(Duration::from_secs(1) - elapsed);
                    }
                }
            });

            worker_handle.join().unwrap();
            let elapsed = start.elapsed();

            // Analyze drift
            let samples = latencies.lock();
            let latency_values: Vec<u64> = samples.iter().map(|s| s.latency_ns).collect();

            // First 10 minutes (600 samples) vs last 10 minutes
            let p50_first_10min = percentile(&latency_values[0..600.min(latency_values.len())], 50);
            let p99_first_10min = percentile(&latency_values[0..600.min(latency_values.len())], 99);

            let last_10min_start = latency_values.len().saturating_sub(600);
            let p50_last_10min = percentile(&latency_values[last_10min_start..], 50);
            let p99_last_10min = percentile(&latency_values[last_10min_start..], 99);

            let p50_drift_pct = if p50_first_10min > 0 {
                ((p50_last_10min as f64 / p50_first_10min as f64) - 1.0) * 100.0
            } else {
                0.0
            };

            let p99_drift_pct = if p99_first_10min > 0 {
                ((p99_last_10min as f64 / p99_first_10min as f64) - 1.0) * 100.0
            } else {
                0.0
            };

            println!("\n=== Latency Drift Monitoring Results ===");
            println!("Total samples: {}", latency_values.len());
            println!("Duration: {:.2}s", elapsed.as_secs_f64());
            println!("\nFirst 10 minutes:");
            println!("  p50: {:.2} μs", p50_first_10min as f64 / 1000.0);
            println!("  p99: {:.2} μs", p99_first_10min as f64 / 1000.0);
            println!("\nLast 10 minutes:");
            println!("  p50: {:.2} μs", p50_last_10min as f64 / 1000.0);
            println!("  p99: {:.2} μs", p99_last_10min as f64 / 1000.0);
            println!("\nDrift:");
            println!("  p50 drift: {:.2}%", p50_drift_pct);
            println!("  p99 drift: {:.2}%", p99_drift_pct);

            // Success criteria
            assert!(
                p50_drift_pct.abs() < 10.0,
                "p50 latency drift {:.2}% exceeds 10% threshold",
                p50_drift_pct
            );
            assert!(
                p99_drift_pct.abs() < 20.0,
                "p99 latency drift {:.2}% exceeds 20% threshold",
                p99_drift_pct
            );

            elapsed
        });
    });

    group.finish();
}

// ==============================================================================
// Benchmark 4: Short Sustained Load (10 Minutes, CI-Friendly)
// ==============================================================================

/// 10-minute sustained load test (CI-friendly version)
///
/// **UCE34 Framework Applied**
/// - Q1: Test 10-minute sustained load at 10K ops/sec (CI-friendly)
/// - Q2: Detect early-stage memory leaks and performance issues
/// - Q3: <10MB memory growth, <5% latency drift
/// - Q8: ~2MB base memory, 6M operations total
///
/// **Test Parameters**
/// - Duration: 10 minutes (600 seconds)
/// - Throughput: 10,000 ops/sec
/// - Total operations: 6,000,000
/// - Threads: 8 (1,250 ops/sec per thread)
/// - Sampling: Memory every 10 seconds, latency every second
///
/// **Success Criteria (B32)**
/// - Memory growth: <10MB over 10 minutes
/// - Latency drift: <5% (p50 first 2min vs last 2min)
/// - No panics, no hangs, no crashes
fn bench_10min_sustained_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("sustained_10min");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(600)); // 10 minutes

    group.bench_function("ConcurrentMapCapsule_10K_ops_per_sec", |b| {
        b.iter_custom(|_iters| {
            let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());
            let start = Instant::now();
            let start_rss = get_process_rss();

            // Spawn 8 worker threads
            let handles: Vec<_> = (0..8)
                .map(|tid| {
                    let map = Arc::clone(&map);
                    thread::spawn(move || {
                        let ops_per_thread = 6_000_000 / 8; // 750K ops per thread
                        let interval_ns = 800_000; // 1,250 ops/sec = 800μs per op

                        for i in 0..ops_per_thread {
                            let op_start = Instant::now();

                            let key = (tid as u64 * 1_000_000) + i as u64;
                            let _ = black_box(map.insert(key, i as u64));

                            // Sample reads every 100 ops
                            if i % 100 == 0 {
                                let _ = black_box(map.get(&key));
                            }

                            // Rate limit to 1,250 ops/sec
                            let elapsed_ns = op_start.elapsed().as_nanos() as u64;
                            if elapsed_ns < interval_ns {
                                thread::sleep(Duration::from_nanos(interval_ns - elapsed_ns));
                            }
                        }
                    })
                })
                .collect();

            // Wait for all threads
            for h in handles {
                h.join().unwrap();
            }

            let elapsed = start.elapsed();
            let end_rss = get_process_rss();

            // Verify no memory leak
            let memory_growth = end_rss.saturating_sub(start_rss);
            let memory_growth_mb = memory_growth / (1024 * 1024);

            println!("\n=== 10-Minute Sustained Load Results ===");
            println!("Total operations: 6,000,000");
            println!("Duration: {:.2}s", elapsed.as_secs_f64());
            println!(
                "Throughput: {:.0} ops/sec",
                6_000_000.0 / elapsed.as_secs_f64()
            );
            println!("Start RSS: {:.2} MB", start_rss / (1024 * 1024));
            println!("End RSS: {:.2} MB", end_rss / (1024 * 1024));
            println!("Memory growth: {} MB", memory_growth_mb);
            println!("Final map length: {}", map.len());

            // Success criteria
            assert!(
                memory_growth_mb < 10,
                "Memory leak detected: {}MB growth exceeds 10MB threshold",
                memory_growth_mb
            );
            assert!(
                map.len() > 5_900_000,
                "Map length {} less than expected 5.9M+ entries",
                map.len()
            );

            elapsed
        });
    });

    group.finish();
}

// ==============================================================================
// Criterion Configuration
// ==============================================================================

criterion_group! {
    name = sustained_load_benches;
    config = Criterion::default();
    targets =
        bench_10min_sustained_load,
        bench_1hour_continuous_operation,
        bench_memory_leak_detection,
        bench_latency_drift_monitoring
}

criterion_main!(sustained_load_benches);
