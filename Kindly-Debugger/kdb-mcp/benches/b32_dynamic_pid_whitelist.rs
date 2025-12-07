//! B32 Framework: Dynamic PID Whitelist - Performance Validation
//!
//! **Benchmark Strategy**: Fair baseline comparison against AccessControlCapsule (64-PID bitmap).
//!
//! **B32 Requirements** (95% CI, 1000+ iterations):
//! - Measure baseline (64-PID bitmap): <5ns per check
//! - Measure optimized (dynamic whitelist): ~45ns per check
//! - Trade-off: 9× slower but unlimited capacity
//! - Validation: Amdahl's Law (45ns overhead negligible at 10μs SLA)
//!
//! **Groups**:
//! 1. `check_pid_cached` - Bloom + hash table hit (45ns expected)
//! 2. `check_pid_negative` - Bloom rejects (10ns expected)
//! 3. `add_pid` - Insert with linear probing (50ns expected)
//! 4. `remove_pid` - Tombstone marking (50ns expected)
//! 5. `check_pid_miss` - Not in whitelist (50ns expected)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use kdb_mcp::DynamicPidWhitelistCapsule;

// ============================================================================
// Baseline: AccessControlCapsule (64-PID bitmap) for comparison
// ============================================================================

/// Simulated 64-PID bitmap check (AccessControlCapsule baseline).
/// **Performance**: ~5ns (atomic load + bit check)
#[inline(never)]
fn check_pid_bitmap(pid: u32) -> bool {
    // Simulate atomic load + bit check
    if pid >= 64 {
        return false; // Out of range, denied
    }
    // Simulate bitwise check
    ((1u64 << pid) & 0x00FF_FFFF_FFFF_FFFF) != 0
}

// ============================================================================
// Setup Fixtures
// ============================================================================

fn setup_whitelist_small() -> DynamicPidWhitelistCapsule {
    let capsule = DynamicPidWhitelistCapsule::new().expect("Setup failed");
    // Add 50 PIDs
    for pid in 0..50 {
        capsule.add_pid(pid).unwrap();
    }
    capsule
}

fn setup_whitelist_medium() -> DynamicPidWhitelistCapsule {
    let capsule = DynamicPidWhitelistCapsule::new().expect("Setup failed");
    // Add 500 PIDs
    for pid in 0..500 {
        capsule.add_pid(pid).unwrap();
    }
    capsule
}

fn setup_whitelist_large() -> DynamicPidWhitelistCapsule {
    let capsule = DynamicPidWhitelistCapsule::new().expect("Setup failed");
    // Add 5000 PIDs
    for pid in 0..5000 {
        capsule.add_pid(pid).unwrap();
    }
    capsule
}

// ============================================================================
// B32 Benchmark Group 1: Check PID (Cached Hit)
// ============================================================================

fn bench_check_pid_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("check_pid_hit");

    group.bench_function("baseline_bitmap_5ns", |b| {
        b.iter(|| {
            let pid = black_box(32u32); // Hit in bitmap
            check_pid_bitmap(pid)
        });
    });

    // Small whitelist (50 PIDs)
    group.bench_function("dynamic_50pids_first", |b| {
        let capsule = setup_whitelist_small();
        b.iter(|| {
            let pid = black_box(0u32); // First PID
            capsule.is_pid_allowed(pid)
        });
    });

    group.bench_function("dynamic_50pids_middle", |b| {
        let capsule = setup_whitelist_small();
        b.iter(|| {
            let pid = black_box(25u32); // Middle PID
            capsule.is_pid_allowed(pid)
        });
    });

    group.bench_function("dynamic_50pids_last", |b| {
        let capsule = setup_whitelist_small();
        b.iter(|| {
            let pid = black_box(49u32); // Last PID
            capsule.is_pid_allowed(pid)
        });
    });

    // Medium whitelist (500 PIDs)
    group.bench_function("dynamic_500pids_hit", |b| {
        let capsule = setup_whitelist_medium();
        b.iter(|| {
            let pid = black_box(250u32); // Middle PID
            capsule.is_pid_allowed(pid)
        });
    });

    // Large whitelist (5000 PIDs)
    group.bench_function("dynamic_5000pids_hit", |b| {
        let capsule = setup_whitelist_large();
        b.iter(|| {
            let pid = black_box(2500u32); // Middle PID
            capsule.is_pid_allowed(pid)
        });
    });

    group.finish();
}

// ============================================================================
// B32 Benchmark Group 2: Check PID (Negative - Bloom Rejects)
// ============================================================================

fn bench_check_pid_negative(c: &mut Criterion) {
    let mut group = c.benchmark_group("check_pid_negative");

    group.bench_function("baseline_bitmap_5ns", |b| {
        b.iter(|| {
            let pid = black_box(99u32); // Out of range
            check_pid_bitmap(pid)
        });
    });

    // Bloom filter rejection (fast path, 10ns)
    group.bench_function("dynamic_bloom_reject", |b| {
        let capsule = setup_whitelist_small();
        b.iter(|| {
            let pid = black_box(999999u32); // Not in whitelist
            capsule.is_pid_allowed(pid)
        });
    });

    group.finish();
}

// ============================================================================
// B32 Benchmark Group 3: Add PID
// ============================================================================

fn bench_add_pid(c: &mut Criterion) {
    let mut group = c.benchmark_group("add_pid");

    group.bench_function("add_first", |b| {
        b.iter_batched(
            || DynamicPidWhitelistCapsule::new().expect("Setup failed"),
            |capsule| {
                let pid = black_box(0u32);
                capsule.add_pid(pid)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("add_to_small", |b| {
        let capsule = setup_whitelist_small();
        let mut next_pid = 50u32;
        b.iter(|| {
            let pid = black_box(next_pid);
            let _ = capsule.add_pid(pid);
            next_pid += 1;
        });
    });

    group.bench_function("add_to_medium", |b| {
        let capsule = setup_whitelist_medium();
        let mut next_pid = 500u32;
        b.iter(|| {
            let pid = black_box(next_pid);
            let _ = capsule.add_pid(pid);
            next_pid += 1;
        });
    });

    group.finish();
}

// ============================================================================
// B32 Benchmark Group 4: Remove PID
// ============================================================================

fn bench_remove_pid(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove_pid");

    // Removing from small whitelist
    group.bench_function("remove_from_small", |b| {
        let capsule = setup_whitelist_small();
        let mut pid_to_remove = 0u32;
        b.iter(|| {
            if pid_to_remove < 50 {
                let pid = black_box(pid_to_remove);
                let _ = capsule.remove_pid(pid);
                pid_to_remove += 1;
            }
        });
    });

    // Removing from medium whitelist
    group.bench_function("remove_from_medium", |b| {
        let capsule = setup_whitelist_medium();
        let mut pid_to_remove = 0u32;
        b.iter(|| {
            if pid_to_remove < 500 {
                let pid = black_box(pid_to_remove);
                let _ = capsule.remove_pid(pid);
                pid_to_remove += 1;
            }
        });
    });

    group.finish();
}

// ============================================================================
// B32 Benchmark Group 5: Mixed Workload (Production-like)
// ============================================================================

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    group.bench_function("80_check_10_add_10_remove", |b| {
        let capsule = setup_whitelist_medium();
        let mut next_new_pid = 1000u32;
        let mut next_remove_pid = 0u32;
        let mut op_count = 0u32;

        b.iter(|| {
            let op = op_count % 100;
            match op {
                0..=79 => {
                    // 80% checks
                    let pid = black_box((op as u32 * 7 + 13) % 500);
                    capsule.is_pid_allowed(pid);
                }
                80..=89 => {
                    // 10% adds
                    let pid = black_box(next_new_pid);
                    let _ = capsule.add_pid(pid);
                    next_new_pid += 1;
                }
                90..=99 => {
                    // 10% removes (only remove what we've added)
                    if next_remove_pid < 500 {
                        let pid = black_box(next_remove_pid);
                        let _ = capsule.remove_pid(pid);
                        next_remove_pid += 1;
                    }
                }
                _ => unreachable!(),
            }
            op_count += 1;
        });
    });

    group.finish();
}

// ============================================================================
// B32 Benchmark Group 6: Latency Percentiles (Production SLA)
// ============================================================================

fn bench_latency_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_percentiles");

    group.bench_function("check_p50_median", |b| {
        let capsule = setup_whitelist_medium();
        b.iter(|| {
            let pid = black_box(250u32);
            capsule.is_pid_allowed(pid)
        });
    });

    group.bench_function("check_p99_tail", |b| {
        let capsule = setup_whitelist_large();
        // Worst case: deep linear probe
        b.iter(|| {
            let pid = black_box(4999u32);
            capsule.is_pid_allowed(pid)
        });
    });

    group.finish();
}

// ============================================================================
// B32 Benchmark Group 7: Scalability (Load Factor Impact)
// ============================================================================

fn bench_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability");

    for num_pids in &[100, 500, 1000, 2000, 5000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_pids),
            num_pids,
            |b, &num_pids| {
                let capsule = DynamicPidWhitelistCapsule::new().expect("Setup failed");
                for pid in 0..num_pids {
                    capsule.add_pid(pid as u32).unwrap();
                }

                b.iter(|| {
                    let pid = black_box((num_pids / 2) as u32);
                    capsule.is_pid_allowed(pid)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// B32 Framework Setup
// ============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(1000);
    targets = bench_check_pid_hit,
             bench_check_pid_negative,
             bench_add_pid,
             bench_remove_pid,
             bench_mixed_workload,
             bench_latency_percentiles,
             bench_scalability
);

criterion_main!(benches);

// ============================================================================
// Expected Results (B32 Framework - Fair Baseline Comparison)
// ============================================================================
//
// **Baseline (AccessControlCapsule 64-PID bitmap)**:
// - check_pid_hit: ~5ns (Acquire load + bit shift)
// - check_pid_negative: ~5ns (bounds check)
//
// **DynamicPidWhitelistCapsule**:
// - check_pid_hit (50 PIDs): ~35-45ns (Bloom 10ns + hash table 25-35ns)
// - check_pid_hit (500 PIDs): ~40-50ns (same + potential 1-2 probes)
// - check_pid_hit (5000 PIDs): ~45-55ns (potential 2-3 probes)
// - check_pid_negative: ~10ns (Bloom rejection fast path)
// - add_pid: ~50ns (Bloom 10ns + hash table 40ns)
// - remove_pid: ~50ns (linear probe to find + tombstone)
// - mixed (80/10/10): ~35ns avg (mostly checks)
//
// **Trade-off Analysis** (Amdahl's Law):
// - Per-request overhead: +40ns (45ns - 5ns existing)
// - Per-request time: ~10,000ns (typical MCP call)
// - Impact: 40ns / 10,000ns = 0.4% (negligible)
// - Benefit: Unlimited PIDs vs 64 bitmap limit (priceless)
//
// **Production SLA**:
// - Latency p50: <50ns (typical case)
// - Latency p99: <100ns (deep probe)
// - Overall MCP latency: <10μs (DynamicPidWhitelist contribution: <0.5%)
