//! B32 HSM Availability Check - Zero Per-Request Overhead Validation
//!
//! **Framework**: B32 Performance Validation
//! - **Fair Baseline**: Software key availability vs HSM atomic read
//! - **95% CI**: Criterion.rs with 1000+ iterations
//! - **Performance Claims**: 0ns per-request overhead (HSM offline only)
//!
//! **Tier**: T1 Atomic + T8 Network (coordination overhead validation)
//!
//! ## Test Plan
//!
//! 1. **Baseline**: Software-only key availability check (spin loop, 1M iterations)
//! 2. **HSM Atomic**: HsmIntegrationCapsule::is_hsm_available() (spin loop, 1M iterations)
//! 3. **Comparison**: Prove HSM overhead is negligible (<1% of SLA)
//!
//! ## Expected Results
//!
//! - **Software key check**: ~10-20ns per operation (atomic read)
//! - **HSM availability check**: <10ns per operation (atomic read, same as software)
//! - **Overhead**: 0ns (both use atomic reads, no difference)
//! - **Impact on SLA**: <10ns / 10,000ns = 0.1% (negligible)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use kdb_mcp::HsmIntegrationCapsule;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// Baseline: Software-only key availability (no HSM)
// ============================================================================

/// Simulate software-only key availability check
struct SoftwareKeyAvailability {
    is_available: AtomicU64,
}

impl SoftwareKeyAvailability {
    fn new() -> Self {
        Self {
            is_available: AtomicU64::new(1),
        }
    }

    fn is_key_available(&self) -> bool {
        self.is_available.load(Ordering::Relaxed) != 0
    }
}

// ============================================================================
// Benchmarks
// ============================================================================

fn bench_hsm_availability(c: &mut Criterion) {
    let mut group = c.benchmark_group("hsm_availability_zero_overhead");
    group.sample_size(100); // 100 samples of 1000 iterations each
    group.measurement_time(std::time::Duration::from_secs(30));

    // ====================================================================
    // Baseline: Software-only availability check
    // ====================================================================
    group.bench_function("baseline_software_key_available", |b| {
        let sw_key = SoftwareKeyAvailability::new();
        b.iter(|| black_box(sw_key.is_key_available()))
    });

    // ====================================================================
    // HSM: Atomic availability check
    // ====================================================================
    group.bench_function("hsm_is_available_atomic_read", |b| {
        let hsm = HsmIntegrationCapsule::new();
        hsm.set_hsm_status(kdb_mcp::HsmStatus::Available);
        b.iter(|| black_box(hsm.is_hsm_available()))
    });

    // ====================================================================
    // Signature count retrieval
    // ====================================================================
    group.bench_function("hsm_get_signature_count", |b| {
        let hsm = HsmIntegrationCapsule::new();
        hsm.increment_signature_count();
        b.iter(|| black_box(hsm.get_signature_count()))
    });

    // ====================================================================
    // HSM status retrieval
    // ====================================================================
    group.bench_function("hsm_hsm_status", |b| {
        let hsm = HsmIntegrationCapsule::new();
        hsm.set_hsm_status(kdb_mcp::HsmStatus::Available);
        b.iter(|| black_box(hsm.hsm_status()))
    });

    // ====================================================================
    // Combined fast-path: is_available() + get_signature_count()
    // ====================================================================
    group.bench_function("hsm_fast_path_combined", |b| {
        let hsm = HsmIntegrationCapsule::new();
        hsm.set_hsm_status(kdb_mcp::HsmStatus::Available);
        b.iter(|| {
            black_box(hsm.is_hsm_available());
            black_box(hsm.get_signature_count())
        })
    });

    group.finish();
}

// ============================================================================
// Amdahl's Law Validation
// ============================================================================

fn bench_amdahls_law(c: &mut Criterion) {
    let mut group = c.benchmark_group("amdahls_law_impact");
    group.sample_size(100);
    group.measurement_time(std::time::Duration::from_secs(30));

    // Measure: (overhead_ns / request_sla_ns) * 100 = % impact
    // Expected: <0.1% (10ns overhead / 10,000ns SLA)

    group.bench_function("amdahls_10ns_overhead_10us_sla", |b| {
        let hsm = HsmIntegrationCapsule::new();
        hsm.set_hsm_status(kdb_mcp::HsmStatus::Available);

        // Simulate request processing:
        // 1. Check HSM availability (<10ns)
        // 2. Process request (~9,990ns)
        // Total: ~10,000ns = 10μs

        b.iter(|| {
            // HSM check (expected <10ns)
            let _available = black_box(hsm.is_hsm_available());

            // Request processing (simulated, typically 9,990ns)
            for _ in 0..1000 {
                black_box(std::hint::black_box(0u64));
            }
        })
    });

    group.finish();
}

// ============================================================================
// Concurrent Access (Thread Safety)
// ============================================================================

fn bench_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("hsm_concurrent_access");
    group.sample_size(50); // Lower sample size for concurrent tests
    group.measurement_time(std::time::Duration::from_secs(30));

    for num_threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let hsm = Arc::new(HsmIntegrationCapsule::new());
                    hsm.set_hsm_status(kdb_mcp::HsmStatus::Available);

                    let mut handles = vec![];
                    for _ in 0..num_threads {
                        let hsm_clone = Arc::clone(&hsm);
                        let handle = std::thread::spawn(move || {
                            // Each thread does 1000 availability checks
                            for _ in 0..1000 {
                                black_box(hsm_clone.is_hsm_available());
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Statistics Operations (Atomic Increments)
// ============================================================================

fn bench_statistics_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("hsm_statistics_updates");
    group.sample_size(100);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("increment_signature_count", |b| {
        let hsm = HsmIntegrationCapsule::new();
        b.iter(|| hsm.increment_signature_count())
    });

    group.bench_function("increment_signing_attempts", |b| {
        let hsm = HsmIntegrationCapsule::new();
        b.iter(|| hsm.increment_signing_attempts())
    });

    group.bench_function("increment_signing_success", |b| {
        let hsm = HsmIntegrationCapsule::new();
        b.iter(|| hsm.increment_signing_success())
    });

    group.bench_function("increment_signing_failed", |b| {
        let hsm = HsmIntegrationCapsule::new();
        b.iter(|| hsm.increment_signing_failed())
    });

    group.bench_function("get_signing_stats", |b| {
        let hsm = HsmIntegrationCapsule::new();
        hsm.increment_signing_attempts();
        hsm.increment_signing_success();
        b.iter(|| hsm.get_signing_stats())
    });

    group.finish();
}

// ============================================================================
// Key Rotation Operations (Occasional, Not on Critical Path)
// ============================================================================

fn bench_key_rotation(c: &mut Criterion) {
    let mut group = c.benchmark_group("hsm_key_rotation");
    group.sample_size(100);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("update_key_rotation", |b| {
        let hsm = HsmIntegrationCapsule::new();
        b.iter(|| hsm.update_key_rotation(1_700_000_000))
    });

    group.bench_function("last_rotation_timestamp", |b| {
        let hsm = HsmIntegrationCapsule::new();
        hsm.update_key_rotation(1_700_000_000);
        b.iter(|| hsm.last_rotation_timestamp())
    });

    group.bench_function("get_rotation_stats", |b| {
        let hsm = HsmIntegrationCapsule::new();
        hsm.update_key_rotation(1_700_000_000);
        b.iter(|| hsm.get_rotation_stats())
    });

    group.finish();
}

// ============================================================================
// Public Key Management Operations (Offline, Not on Critical Path)
// ============================================================================

fn bench_public_key_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("hsm_public_key_ops");
    group.sample_size(100);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("get_public_key_hash_cached", |b| {
        let hsm = HsmIntegrationCapsule::new();
        let key = vec![42u8; 32];
        hsm.update_public_key_hash(&key).ok();
        b.iter(|| hsm.get_public_key_hash())
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default()
        .measurement_time(std::time::Duration::from_secs(30))
        .sample_size(100);
    targets =
        bench_hsm_availability,
        bench_amdahls_law,
        bench_concurrent_access,
        bench_statistics_updates,
        bench_key_rotation,
        bench_public_key_operations
);

criterion_main!(benches);
