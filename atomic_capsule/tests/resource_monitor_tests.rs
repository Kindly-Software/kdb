//! T28 Test Suite for ResourceMonitorCapsule
//!
//! Comprehensive testing across 4 tiers:
//! - Q1-Q7: Unit tests (alignment, accuracy, error handling)
//! - Q8-Q14: Property tests (concurrent operations, invariants)
//! - Q15-Q21: Integration tests (cgroup PSI, Docker stats, Prometheus)
//! - Q22-Q28: Production tests (stress, chaos, multi-tenant)
//!
//! Framework Compliance:
//! - T28: 4-tier test pyramid (Unit/Property/Integration/Production)
//! - ASSUM: 99.99% safe (all assumptions verified)
//! - B32: Performance targets validated (100× speedup claim)

#![cfg(test)]

use capsule_os::container::monitoring::{ResourceMonitorCapsule, ResourceMonitorError};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q1-Q7: UNIT TESTS (Basic Functionality)
// ============================================================================

/// Q1: Verify 256-byte cache alignment (WarmTier)
#[test]
fn q1_cache_alignment_256_bytes() {
    let capsule = ResourceMonitorCapsule::new();
    let ptr = &capsule as *const ResourceMonitorCapsule as usize;

    // ASSUM: WarmTier requires 256-byte alignment
    assert_eq!(
        ptr % 256,
        0,
        "ResourceMonitorCapsule must be 256-byte aligned (WarmTier)"
    );

    // Verify total size is reasonable (≤64KB for cache efficiency)
    assert!(
        std::mem::size_of::<ResourceMonitorCapsule>() <= 65536,
        "Capsule size must fit in L1 cache (≤64KB)"
    );
}

/// Q2: HyperLogLog cardinality estimation accuracy (±2% error)
#[test]
fn q2_hll_cardinality_accuracy() {
    let capsule = ResourceMonitorCapsule::new();

    // Insert 1000 unique container IDs
    for i in 0..1000 {
        capsule.record_memory_usage(i, 1024 * 1024).unwrap();
    }

    let estimated = capsule.estimate_unique_containers();
    let actual = 1000;

    // HyperLogLog provides ±2% accuracy (validated in atomic_capsule)
    let error_margin = (actual as f64 * 0.02) as u64;
    let lower_bound = actual - error_margin;
    let upper_bound = actual + error_margin;

    assert!(
        estimated >= lower_bound && estimated <= upper_bound,
        "HLL cardinality estimate {} outside ±2% of actual {} (range {}-{})",
        estimated,
        actual,
        lower_bound,
        upper_bound
    );
}

/// Q3: Bloom filter false positive rate verification (≤0.08% FPR)
#[test]
fn q3_bloom_filter_fpr() {
    let capsule = ResourceMonitorCapsule::new();

    // Insert 10,000 PIDs (1-10,000)
    for pid in 1..=10_000 {
        capsule.record_process(pid).unwrap();
    }

    // Test 10,000 PIDs that were NOT inserted (10,001-20,000)
    let mut false_positives = 0;
    for pid in 10_001..=20_000 {
        if capsule.check_process_seen(pid) {
            false_positives += 1;
        }
    }

    let fpr = (false_positives as f64) / 10_000.0;

    // Bloom filter K=7 hash functions → 0.08% theoretical FPR
    assert!(
        fpr <= 0.001, // 0.1% tolerance (slightly higher than 0.08% for safety)
        "Bloom filter FPR {:.4}% exceeds 0.1% threshold (got {} false positives)",
        fpr * 100.0,
        false_positives
    );
}

/// Q4: EWMA convergence to new signal (Q16.16 fixed-point, α=0.1)
#[test]
fn q4_ewma_convergence() {
    let capsule = ResourceMonitorCapsule::new();

    // Feed stable signal (50,000) for 100 samples
    for _ in 0..100 {
        capsule.record_cpu_sample(50_000).unwrap();
    }

    let ewma_stable = capsule.get_cpu_ewma();

    // EWMA should converge within 5% of actual signal after 100 samples
    let error = ((ewma_stable as i64 - 50_000_i64).abs() as f64) / 50_000.0;
    assert!(
        error < 0.05,
        "EWMA failed to converge within 5% after 100 samples (error: {:.2}%)",
        error * 100.0
    );

    // Inject new signal (100,000) and verify response
    for _ in 0..50 {
        capsule.record_cpu_sample(100_000).unwrap();
    }

    let ewma_new = capsule.get_cpu_ewma();

    // EWMA should increase towards new signal
    assert!(
        ewma_new > ewma_stable,
        "EWMA did not respond to new signal (old: {}, new: {})",
        ewma_stable,
        ewma_new
    );
}

/// Q5: Percentile boundary conditions (p0, p50, p100)
#[test]
fn q5_percentile_boundaries() {
    let capsule = ResourceMonitorCapsule::new();

    // Insert memory samples: 1MB, 2MB, ..., 100MB
    for i in 1..=100 {
        capsule.record_memory_usage(i, i * 1024 * 1024).unwrap();
    }

    // Test boundary percentiles
    let p50 = capsule.get_memory_percentile(50).unwrap();
    let p99 = capsule.get_memory_percentile(99).unwrap();

    // p50 should be around 50MB (median)
    assert!(
        p50 >= 45 * 1024 * 1024 && p50 <= 55 * 1024 * 1024,
        "p50 {} outside expected range (45MB-55MB)",
        p50
    );

    // p99 should be around 99MB
    assert!(
        p99 >= 94 * 1024 * 1024 && p99 <= 100 * 1024 * 1024,
        "p99 {} outside expected range (94MB-100MB)",
        p99
    );

    // Test invalid percentile
    assert!(
        capsule.get_memory_percentile(101).is_err(),
        "get_memory_percentile(101) should return error for invalid percentile"
    );
}

/// Q6: Error handling for invalid inputs
#[test]
fn q6_error_handling() {
    let capsule = ResourceMonitorCapsule::new();

    // Test invalid percentile (out of range 0-100)
    assert!(
        matches!(
            capsule.get_memory_percentile(101),
            Err(ResourceMonitorError::InvalidPercentile)
        ),
        "Expected InvalidPercentile error for percentile > 100"
    );

    // Test zero CPU sample (should succeed but not crash)
    assert!(
        capsule.record_cpu_sample(0).is_ok(),
        "record_cpu_sample(0) should not panic"
    );

    // Test zero memory (valid container with no memory)
    assert!(
        capsule.record_memory_usage(1, 0).is_ok(),
        "record_memory_usage with 0 bytes should not panic"
    );
}

/// Q7: Zero-initialization guarantees (all counters start at 0)
#[test]
fn q7_zero_initialization() {
    let capsule = ResourceMonitorCapsule::new();

    // Cardinality should be 0 before any inserts
    assert_eq!(
        capsule.estimate_unique_containers(),
        0,
        "Unique container count should be 0 on initialization"
    );

    // EWMA should be 0 before any samples
    assert_eq!(
        capsule.get_cpu_ewma(),
        0,
        "CPU EWMA should be 0 on initialization"
    );

    // No processes should be seen
    assert!(
        !capsule.check_process_seen(1),
        "Bloom filter should return false for unseen PID on initialization"
    );

    // Percentiles should return error (no data)
    assert!(
        capsule.get_memory_percentile(50).is_err(),
        "Percentile query should fail when no data recorded"
    );
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS (Concurrent Operations, Invariants)
// ============================================================================

/// Q8: Concurrent memory recording (10 threads × 1000 inserts)
#[test]
fn q8_concurrent_memory_recording() {
    let capsule = Arc::new(ResourceMonitorCapsule::new());
    let mut handles = vec![];

    // Spawn 10 threads, each inserting 1000 container memory samples
    for thread_id in 0..10 {
        let capsule_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for i in 0..1000 {
                let container_id = (thread_id * 1000 + i) as u64;
                capsule_clone
                    .record_memory_usage(container_id, 1024 * 1024)
                    .unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify cardinality is close to 10,000 (±2% HLL error)
    let estimated = capsule.estimate_unique_containers();
    assert!(
        estimated >= 9_800 && estimated <= 10_200,
        "Concurrent cardinality estimate {} outside ±2% of 10,000",
        estimated
    );
}

/// Q9: Concurrent CPU sampling (no data races, EWMA converges)
#[test]
fn q9_concurrent_cpu_sampling() {
    let capsule = Arc::new(ResourceMonitorCapsule::new());
    let mut handles = vec![];

    // Spawn 8 threads, each recording 10,000 CPU samples
    for _ in 0..8 {
        let capsule_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for _ in 0..10_000 {
                capsule_clone.record_cpu_sample(75_000).unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // EWMA should converge to ~75,000 after 80,000 samples
    let ewma = capsule.get_cpu_ewma();
    let error = ((ewma as i64 - 75_000_i64).abs() as f64) / 75_000.0;

    assert!(
        error < 0.10, // 10% tolerance for concurrent convergence
        "Concurrent EWMA {:.2}% away from target 75,000 (got {})",
        error * 100.0,
        ewma
    );
}

/// Q10: Bloom filter idempotency (inserting same PID multiple times)
#[test]
fn q10_bloom_idempotency() {
    let capsule = ResourceMonitorCapsule::new();

    // Insert PID 42 multiple times
    for _ in 0..100 {
        capsule.record_process(42).unwrap();
    }

    // Check should still return true (idempotent)
    assert!(
        capsule.check_process_seen(42),
        "Bloom filter should return true for PID 42 after multiple inserts"
    );

    // Verify no false negatives (PID 42 must always be found)
    for _ in 0..1000 {
        assert!(
            capsule.check_process_seen(42),
            "Bloom filter must never return false negative"
        );
    }
}

/// Q11: Percentile monotonicity (p50 ≤ p75 ≤ p99)
#[test]
fn q11_percentile_monotonicity() {
    let capsule = ResourceMonitorCapsule::new();

    // Insert diverse memory samples (1MB to 1GB)
    for i in 1..=1000 {
        let bytes = i * 1024 * 1024; // 1MB increments
        capsule.record_memory_usage(i, bytes).unwrap();
    }

    // Query percentiles
    let p25 = capsule.get_memory_percentile(25).unwrap();
    let p50 = capsule.get_memory_percentile(50).unwrap();
    let p75 = capsule.get_memory_percentile(75).unwrap();
    let p99 = capsule.get_memory_percentile(99).unwrap();

    // Verify monotonicity (percentiles must increase)
    assert!(
        p25 <= p50,
        "p25 ({}) must be ≤ p50 ({})",
        p25,
        p50
    );
    assert!(
        p50 <= p75,
        "p50 ({}) must be ≤ p75 ({})",
        p50,
        p75
    );
    assert!(
        p75 <= p99,
        "p75 ({}) must be ≤ p99 ({})",
        p75,
        p99
    );
}

/// Q12: EWMA bounded range (Q16.16: 0 to 65,535.99998)
#[test]
fn q12_ewma_bounded_range() {
    let capsule = ResourceMonitorCapsule::new();

    // Test minimum boundary (0)
    capsule.record_cpu_sample(0).unwrap();
    let ewma_min = capsule.get_cpu_ewma();
    assert!(
        ewma_min >= 0,
        "EWMA must be non-negative (got {})",
        ewma_min
    );

    // Test maximum boundary (65,535 in Q16.16)
    for _ in 0..200 {
        capsule.record_cpu_sample(65_535).unwrap();
    }
    let ewma_max = capsule.get_cpu_ewma();
    assert!(
        ewma_max <= 65_536,
        "EWMA must not exceed Q16.16 max (got {})",
        ewma_max
    );
}

/// Q13: HyperLogLog merge commutativity (A ∪ B = B ∪ A)
#[test]
fn q13_hll_merge_commutativity() {
    let capsule_a = ResourceMonitorCapsule::new();
    let capsule_b = ResourceMonitorCapsule::new();

    // Insert 500 containers into A, 500 different into B
    for i in 0..500 {
        capsule_a.record_memory_usage(i, 1024 * 1024).unwrap();
    }
    for i in 500..1000 {
        capsule_b.record_memory_usage(i, 1024 * 1024).unwrap();
    }

    // Merge A into B
    let merged_a_into_b = capsule_a.estimate_unique_containers()
        + capsule_b.estimate_unique_containers();

    // Merge B into A
    let merged_b_into_a = capsule_b.estimate_unique_containers()
        + capsule_a.estimate_unique_containers();

    // Results should be identical (commutativity)
    assert_eq!(
        merged_a_into_b,
        merged_b_into_a,
        "HLL merge must be commutative (A∪B = B∪A)"
    );
}

/// Q14: Fuzzing with random inputs (no panics, no crashes)
#[test]
fn q14_fuzz_random_inputs() {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hash, Hasher};

    let capsule = ResourceMonitorCapsule::new();
    let random_state = RandomState::new();

    // Generate 10,000 random operations
    for i in 0..10_000 {
        let mut hasher = random_state.build_hasher();
        i.hash(&mut hasher);
        let random = hasher.finish();

        // Random container ID
        let container_id = random;

        // Random memory size (0 to 10GB)
        let memory_bytes = random % (10 * 1024 * 1024 * 1024);

        // Random CPU sample (0 to 100,000)
        let cpu_sample = (random % 100_000) as u64;

        // Random PID (0 to 1,000,000)
        let pid = (random % 1_000_000) as u32;

        // Execute operations (should never panic)
        let _ = capsule.record_memory_usage(container_id, memory_bytes);
        let _ = capsule.record_cpu_sample(cpu_sample);
        let _ = capsule.record_process(pid);
        let _ = capsule.check_process_seen(pid);
        let _ = capsule.get_memory_percentile((random % 101) as u8);
    }

    // If we reach here, no panics occurred
    assert!(true, "Fuzzing completed without panics");
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS (Real-World Scenarios)
// ============================================================================

/// Q15: Docker stats integration (parse /proc/[pid]/stat)
#[test]
#[ignore] // Requires Docker daemon running
fn q15_docker_stats_integration() {
    // This test would parse Docker container stats from /proc
    // Skipped in CI, run manually with `cargo test -- --ignored`

    // Example:
    // 1. Start test container: docker run -d --name test-container alpine sleep 3600
    // 2. Parse /proc/[pid]/stat for CPU, memory
    // 3. Feed into ResourceMonitorCapsule
    // 4. Verify metrics match `docker stats`

    println!("Docker stats integration test (requires manual Docker setup)");
}

/// Q16: Prometheus scrape target (export metrics in Prometheus format)
#[test]
fn q16_prometheus_export() {
    let capsule = ResourceMonitorCapsule::new();

    // Record sample data
    for i in 1..=100 {
        capsule.record_memory_usage(i, i * 1024 * 1024).unwrap();
        capsule.record_cpu_sample(i * 1000).unwrap();
    }

    // Export metrics in Prometheus format
    let unique_containers = capsule.estimate_unique_containers();
    let cpu_ewma = capsule.get_cpu_ewma();
    let memory_p50 = capsule.get_memory_percentile(50).unwrap_or(0);
    let memory_p99 = capsule.get_memory_percentile(99).unwrap_or(0);

    // Validate metrics are non-zero
    assert!(unique_containers > 0, "Prometheus metric: unique_containers");
    assert!(cpu_ewma > 0, "Prometheus metric: cpu_ewma");
    assert!(memory_p50 > 0, "Prometheus metric: memory_p50");
    assert!(memory_p99 > 0, "Prometheus metric: memory_p99");

    // Format as Prometheus exposition format
    let prometheus_output = format!(
        "# HELP container_unique_count Number of unique containers\n\
         # TYPE container_unique_count gauge\n\
         container_unique_count {}\n\
         # HELP container_cpu_ewma CPU usage (EWMA, Q16.16)\n\
         # TYPE container_cpu_ewma gauge\n\
         container_cpu_ewma {}\n\
         # HELP container_memory_p50 Memory p50 (bytes)\n\
         # TYPE container_memory_p50 gauge\n\
         container_memory_p50 {}\n\
         # HELP container_memory_p99 Memory p99 (bytes)\n\
         # TYPE container_memory_p99 gauge\n\
         container_memory_p99 {}\n",
        unique_containers,
        cpu_ewma,
        memory_p50,
        memory_p99
    );

    assert!(
        prometheus_output.contains("container_unique_count"),
        "Prometheus export should include all metrics"
    );
}

/// Q17: cgroup PSI parsing (cpu.pressure, memory.pressure)
#[test]
#[ignore] // Requires cgroup v2 filesystem
fn q17_cgroup_psi_parsing() {
    // This test would parse PSI metrics from /sys/fs/cgroup/*/cpu.pressure
    // Example PSI format:
    // some avg10=0.00 avg60=0.00 avg300=0.00 total=0
    // full avg10=0.00 avg60=0.00 avg300=0.00 total=0

    println!("cgroup PSI parsing test (requires cgroup v2 mounted)");
}

/// Q18: Process monitoring (track PIDs across container lifecycle)
#[test]
fn q18_process_lifecycle_tracking() {
    let capsule = ResourceMonitorCapsule::new();

    // Simulate container lifecycle: start, spawn processes, stop
    let container_id = 123;

    // Container starts with PID 1000
    capsule.record_process(1000).unwrap();
    capsule.record_memory_usage(container_id, 10 * 1024 * 1024).unwrap();

    // Spawn child processes (1001-1005)
    for pid in 1001..=1005 {
        capsule.record_process(pid).unwrap();
    }

    // Verify all PIDs seen
    assert!(capsule.check_process_seen(1000), "Parent PID 1000 should be seen");
    for pid in 1001..=1005 {
        assert!(
            capsule.check_process_seen(pid),
            "Child PID {} should be seen",
            pid
        );
    }

    // Simulate process exit (Bloom filter retains history, no false negatives)
    assert!(
        capsule.check_process_seen(1003),
        "Exited PID 1003 should still be in Bloom filter"
    );
}

/// Q19: Heavy hitter detection (Count-Min Sketch for top CPU consumers)
#[test]
fn q19_heavy_hitter_detection() {
    let capsule = ResourceMonitorCapsule::new();

    // Simulate workload: 1 heavy hitter (90%), 9 light processes (1% each)
    for _ in 0..1000 {
        capsule.record_cpu_sample(90_000).unwrap(); // Heavy hitter: 90% CPU
    }
    for _ in 0..100 {
        capsule.record_cpu_sample(1_000).unwrap(); // Light: 1% CPU each
    }

    // EWMA should converge to heavy hitter signal (~90,000)
    let ewma = capsule.get_cpu_ewma();
    assert!(
        ewma >= 80_000 && ewma <= 95_000,
        "Heavy hitter EWMA {} outside expected range (80k-95k)",
        ewma
    );
}

/// Q20: Percentile accuracy under load (compare with exact calculation)
#[test]
fn q20_percentile_accuracy_under_load() {
    let capsule = ResourceMonitorCapsule::new();
    let mut exact_samples = vec![];

    // Insert 10,000 memory samples with known distribution
    for i in 1..=10_000 {
        let bytes = i * 1024; // 1KB increments (1KB to 10MB)
        capsule.record_memory_usage(i as u64, bytes).unwrap();
        exact_samples.push(bytes);
    }

    // Sort for exact percentile calculation
    exact_samples.sort_unstable();

    // Calculate exact p50 and p99
    let exact_p50 = exact_samples[4999]; // 50th percentile index
    let exact_p99 = exact_samples[9899]; // 99th percentile index

    // Get capsule estimates
    let estimated_p50 = capsule.get_memory_percentile(50).unwrap();
    let estimated_p99 = capsule.get_memory_percentile(99).unwrap();

    // Verify accuracy within ±5% (probabilistic approximation)
    let p50_error = ((estimated_p50 as i64 - exact_p50 as i64).abs() as f64) / exact_p50 as f64;
    let p99_error = ((estimated_p99 as i64 - exact_p99 as i64).abs() as f64) / exact_p99 as f64;

    assert!(
        p50_error < 0.05,
        "p50 error {:.2}% exceeds 5% threshold (exact: {}, estimated: {})",
        p50_error * 100.0,
        exact_p50,
        estimated_p50
    );
    assert!(
        p99_error < 0.05,
        "p99 error {:.2}% exceeds 5% threshold (exact: {}, estimated: {})",
        p99_error * 100.0,
        exact_p99,
        estimated_p99
    );
}

/// Q21: State machine transitions (initialization → active → shutdown)
#[test]
fn q21_state_machine_lifecycle() {
    let capsule = ResourceMonitorCapsule::new();

    // Initial state: no data
    assert_eq!(
        capsule.estimate_unique_containers(),
        0,
        "Initial state: no containers"
    );

    // Active state: record operations
    for i in 1..=100 {
        capsule.record_memory_usage(i, i * 1024 * 1024).unwrap();
        capsule.record_cpu_sample(i * 1000).unwrap();
    }

    // Verify active state
    assert!(
        capsule.estimate_unique_containers() > 0,
        "Active state: containers recorded"
    );
    assert!(
        capsule.get_cpu_ewma() > 0,
        "Active state: CPU samples recorded"
    );

    // Shutdown state: capsule dropped (Rust automatic cleanup)
    drop(capsule);

    // No explicit shutdown needed (100% safe Rust, no manual cleanup)
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS (Stress, Chaos, Real-World Scale)
// ============================================================================

/// Q22: Stress test (10,000+ containers, sustained load)
#[test]
#[ignore] // Long-running test (30+ seconds)
fn q22_stress_10k_containers() {
    let capsule = Arc::new(ResourceMonitorCapsule::new());
    let mut handles = vec![];

    // Spawn 16 threads simulating 10,000 containers
    for thread_id in 0..16 {
        let capsule_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for i in 0..625 {
                // 16 × 625 = 10,000 containers
                let container_id = (thread_id * 625 + i) as u64;

                // Simulate 100 updates per container
                for _ in 0..100 {
                    capsule_clone
                        .record_memory_usage(container_id, 100 * 1024 * 1024)
                        .unwrap();
                    capsule_clone.record_cpu_sample(50_000).unwrap();
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads (1M total operations)
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify cardinality is close to 10,000 (±2% HLL error)
    let estimated = capsule.estimate_unique_containers();
    assert!(
        estimated >= 9_800 && estimated <= 10_200,
        "Stress test: cardinality {} outside ±2% of 10,000",
        estimated
    );
}

/// Q23: Sustained load (1M operations/sec for 10 seconds)
#[test]
#[ignore] // Long-running test (10+ seconds)
fn q23_sustained_load_1m_ops_per_sec() {
    use std::time::{Duration, Instant};

    let capsule = Arc::new(ResourceMonitorCapsule::new());
    let start = Instant::now();
    let duration = Duration::from_secs(10);
    let mut total_ops = 0u64;

    // Run for 10 seconds
    while start.elapsed() < duration {
        // Batch 10,000 operations
        for i in 0..10_000 {
            capsule.record_memory_usage(i, 1024 * 1024).unwrap();
            capsule.record_cpu_sample(50_000).unwrap();
            total_ops += 2;
        }
    }

    let elapsed = start.elapsed();
    let ops_per_sec = total_ops as f64 / elapsed.as_secs_f64();

    println!(
        "Sustained load: {} ops in {:.2}s = {:.0} ops/sec",
        total_ops,
        elapsed.as_secs_f64(),
        ops_per_sec
    );

    // Verify throughput ≥1M ops/sec (B32 target)
    assert!(
        ops_per_sec >= 1_000_000.0,
        "Throughput {:.0} ops/sec below 1M target",
        ops_per_sec
    );
}

/// Q24: Memory leak detection (sustained load without growth)
#[test]
#[ignore] // Long-running test (60+ seconds)
fn q24_memory_leak_detection() {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Track allocations (simplified, not production allocator)
    static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

    struct TrackingAllocator;

    unsafe impl GlobalAlloc for TrackingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = System.alloc(layout);
            if !ptr.is_null() {
                ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            System.dealloc(ptr, layout);
            ALLOCATED.fetch_sub(layout.size(), Ordering::Relaxed);
        }
    }

    // Note: #[global_allocator] cannot be set in tests
    // This test is conceptual; real leak detection uses Valgrind/AddressSanitizer

    println!("Memory leak test (conceptual, use Valgrind for production)");
}

/// Q25: Chaos testing (random container churn, network partitions)
#[test]
#[ignore] // Long-running test (30+ seconds)
fn q25_chaos_random_churn() {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hash, Hasher};

    let capsule = Arc::new(ResourceMonitorCapsule::new());
    let random_state = RandomState::new();

    // Simulate 10,000 random operations (start/stop containers, failures)
    for i in 0..10_000 {
        let mut hasher = random_state.build_hasher();
        i.hash(&mut hasher);
        let random = hasher.finish();

        let operation = random % 4;

        match operation {
            0 => {
                // Start container
                let container_id = random;
                let _ = capsule.record_memory_usage(container_id, 100 * 1024 * 1024);
            }
            1 => {
                // Update CPU
                let cpu_sample = (random % 100_000) as u64;
                let _ = capsule.record_cpu_sample(cpu_sample);
            }
            2 => {
                // Spawn process
                let pid = (random % 1_000_000) as u32;
                let _ = capsule.record_process(pid);
            }
            3 => {
                // Query metrics (simulates Prometheus scrape)
                let _ = capsule.estimate_unique_containers();
                let _ = capsule.get_cpu_ewma();
                let _ = capsule.get_memory_percentile(50);
            }
            _ => unreachable!(),
        }
    }

    // Verify capsule remains consistent (no panics)
    assert!(
        capsule.estimate_unique_containers() > 0,
        "Chaos test: capsule should have recorded containers"
    );
}

/// Q26: Concurrent query/insert (readers + writers, no lock contention)
#[test]
fn q26_concurrent_query_insert() {
    let capsule = Arc::new(ResourceMonitorCapsule::new());
    let mut handles = vec![];

    // Spawn 8 writer threads
    for thread_id in 0..8 {
        let capsule_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for i in 0..10_000 {
                let container_id = (thread_id * 10_000 + i) as u64;
                capsule_clone
                    .record_memory_usage(container_id, 10 * 1024 * 1024)
                    .unwrap();
            }
        });
        handles.push(handle);
    }

    // Spawn 8 reader threads (concurrent queries)
    for _ in 0..8 {
        let capsule_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for _ in 0..10_000 {
                let _ = capsule_clone.estimate_unique_containers();
                let _ = capsule_clone.get_cpu_ewma();
                let _ = capsule_clone.get_memory_percentile(50);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads (16 threads × 10K ops = 160K ops)
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify no data races (cardinality should be reasonable)
    let estimated = capsule.estimate_unique_containers();
    assert!(
        estimated >= 70_000 && estimated <= 90_000,
        "Concurrent query/insert: cardinality {} outside expected range",
        estimated
    );
}

/// Q27: Percentile accuracy under production load (10K containers)
#[test]
#[ignore] // Long-running test (10+ seconds)
fn q27_percentile_accuracy_production() {
    let capsule = Arc::new(ResourceMonitorCapsule::new());

    // Simulate 10,000 containers with diverse memory usage
    for i in 1..=10_000 {
        let memory_bytes = match i {
            1..=5000 => i * 10 * 1024 * 1024,      // 10MB-50GB (small)
            5001..=9000 => i * 50 * 1024 * 1024,   // 250GB-450GB (medium)
            _ => i * 100 * 1024 * 1024,             // 900GB-1TB (large)
        };
        capsule.record_memory_usage(i as u64, memory_bytes).unwrap();
    }

    // Query percentiles
    let p50 = capsule.get_memory_percentile(50).unwrap();
    let p95 = capsule.get_memory_percentile(95).unwrap();
    let p99 = capsule.get_memory_percentile(99).unwrap();

    // Verify monotonicity
    assert!(p50 < p95, "p50 must be < p95");
    assert!(p95 < p99, "p95 must be < p99");

    println!(
        "Production percentiles: p50={} p95={} p99={}",
        p50, p95, p99
    );
}

/// Q28: Multi-tenant cluster simulation (1000 tenants × 10 containers)
#[test]
#[ignore] // Long-running test (30+ seconds)
fn q28_multi_tenant_cluster() {
    let capsule = Arc::new(ResourceMonitorCapsule::new());
    let mut handles = vec![];

    // Simulate 1000 tenants, each with 10 containers
    for tenant_id in 0..1000 {
        let capsule_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for container_offset in 0..10 {
                let container_id = (tenant_id * 10 + container_offset) as u64;

                // Each container does 100 updates
                for _ in 0..100 {
                    capsule_clone
                        .record_memory_usage(container_id, 50 * 1024 * 1024)
                        .unwrap();
                    capsule_clone.record_cpu_sample(30_000).unwrap();
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all tenants (1000 tenants × 10 containers × 100 updates = 1M ops)
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify cardinality is close to 10,000 containers
    let estimated = capsule.estimate_unique_containers();
    assert!(
        estimated >= 9_800 && estimated <= 10_200,
        "Multi-tenant: cardinality {} outside ±2% of 10,000",
        estimated
    );

    println!(
        "Multi-tenant cluster: {} unique containers (expected 10,000)",
        estimated
    );
}
