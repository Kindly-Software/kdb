//! License Capsule Benchmarks (B32 Framework Compliance)
//!
//! **Framework**: B32 (Fair Baselines, 95% CI, 1000+ iterations)
//! **Targets**: <5ns validation, <10ns usage recording
//! **Reality Check**: K1-K27 typical, EXCEPTIONAL (2-10×) if proven
//!
//! Run: cargo bench --bench license_capsule_bench --features benchmarking

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use kindly_dedup::license_capsule::{LicenseCapsule, LicenseStatus, LicenseTier};
use std::sync::Arc;
use std::thread;

// ============================================================================
// BASELINE: License Validation (<5ns target)
// ============================================================================

fn bench_validation_basic(c: &mut Criterion) {
    c.bench_function("license_validation_basic", |b| {
        let license = black_box(LicenseCapsule::new("BENCH-KEY", LicenseTier::Pro).expect("License creation failed"));

        b.iter(|| {
            let status = black_box(license.validate());
            // Verify it doesn't panic
            if let Err(_) = status {
                panic!("Validation failed");
            }
        });
    });
}

fn bench_validation_by_tier(c: &mut Criterion) {
    let mut group = c.benchmark_group("license_validation_by_tier");

    for tier in &[
        ("Trial", LicenseTier::Trial),
        ("Starter", LicenseTier::Starter),
        ("Pro", LicenseTier::Pro),
        ("Enterprise", LicenseTier::Enterprise),
    ] {
        let license = black_box(LicenseCapsule::new(&format!("BENCH-{}", tier.0), tier.1).unwrap());

        group.bench_with_input(BenchmarkId::from_parameter(tier.0), tier, |b, _| {
            b.iter(|| {
                let _status = black_box(license.validate());
            });
        });
    }
    group.finish();
}

// ============================================================================
// USAGE RECORDING (<10ns target)
// ============================================================================

fn bench_record_usage_single(c: &mut Criterion) {
    c.bench_function("license_record_usage_single", |b| {
        let license = black_box(LicenseCapsule::new("BENCH-USAGE", LicenseTier::Pro).expect("License creation failed"));

        b.iter(|| {
            let result = black_box(license.record_usage(1));
            // Verify it succeeds
            if let Err(_) = result {
                panic!("Record usage failed");
            }
        });
    });
}

fn bench_record_usage_bulk(c: &mut Criterion) {
    let mut group = c.benchmark_group("license_record_usage_bulk");

    for size in &[1, 10, 100, 1000] {
        group.bench_with_input(BenchmarkId::new("gb", size), size, |b, &gb| {
            let license = black_box(LicenseCapsule::new("BENCH-BULK", LicenseTier::Pro).unwrap());

            b.iter(|| {
                let result = black_box(license.record_usage(gb));
                if let Err(_) = result {
                    panic!("Bulk record failed");
                }
            });
        });
    }
    group.finish();
}

// ============================================================================
// QUOTA CHECKING (<5ns target)
// ============================================================================

fn bench_remaining_quota(c: &mut Criterion) {
    let mut group = c.benchmark_group("license_remaining_quota");

    for (name, tier) in &[
        ("Trial", LicenseTier::Trial),
        ("Starter", LicenseTier::Starter),
        ("Pro", LicenseTier::Pro),
    ] {
        let license = black_box(LicenseCapsule::new(&format!("BENCH-QUOTA-{}", name), *tier).unwrap());

        group.bench_with_input(BenchmarkId::new("tier", name), tier, |b, _| {
            b.iter(|| {
                let remaining = black_box(license.remaining_gb());
                let _ = black_box(remaining);
            });
        });
    }
    group.finish();
}

// ============================================================================
// CHECKSUM VALIDATION (<50ns target)
// ============================================================================

fn bench_checksum_verification(c: &mut Criterion) {
    c.bench_function("license_checksum_valid", |b| {
        let license =
            black_box(LicenseCapsule::new("BENCH-CHECKSUM", LicenseTier::Pro).expect("License creation failed"));

        b.iter(|| {
            let valid = black_box(license.checksum_valid());
            assert!(valid, "Checksum should be valid");
        });
    });
}

// ============================================================================
// LICENSE CREATION (one-time cost)
// ============================================================================

fn bench_license_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("license_creation");

    for (name, tier) in &[
        ("Trial", LicenseTier::Trial),
        ("Starter", LicenseTier::Starter),
        ("Pro", LicenseTier::Pro),
        ("Enterprise", LicenseTier::Enterprise),
    ] {
        group.bench_with_input(BenchmarkId::new("tier", name), tier, |b, &tier| {
            b.iter(|| {
                let license =
                    black_box(LicenseCapsule::new(&format!("BENCH-CREATE-{}", name), tier).expect("Creation failed"));
                let _ = black_box(license);
            });
        });
    }
    group.finish();
}

// ============================================================================
// CONCURRENT OPERATIONS (scalability)
// ============================================================================

fn bench_concurrent_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("license_concurrent_validation");
    group.sample_size(10); // Fewer samples for longer-running benchmarks

    for threads in &[2, 4, 8, 16] {
        group.bench_with_input(BenchmarkId::new("threads", threads), threads, |b, &num_threads| {
            let license =
                Arc::new(LicenseCapsule::new("BENCH-CONC", LicenseTier::Pro).expect("License creation failed"));

            b.iter(|| {
                let mut handles = vec![];

                for _ in 0..num_threads {
                    let lic = Arc::clone(&license);
                    handles.push(thread::spawn(move || {
                        for _ in 0..100 {
                            let _status = black_box(lic.validate());
                        }
                    }));
                }

                for h in handles {
                    h.join().unwrap();
                }
            });
        });
    }
    group.finish();
}

fn bench_concurrent_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("license_concurrent_usage");
    group.sample_size(10);

    for threads in &[2, 4, 8] {
        group.bench_with_input(BenchmarkId::new("threads", threads), threads, |b, &num_threads| {
            let license =
                Arc::new(LicenseCapsule::new("BENCH-CONC-USE", LicenseTier::Pro).expect("License creation failed"));

            b.iter(|| {
                let mut handles = vec![];

                for _ in 0..num_threads {
                    let lic = Arc::clone(&license);
                    handles.push(thread::spawn(move || {
                        for _ in 0..50 {
                            let _result = black_box(lic.record_usage(1));
                        }
                    }));
                }

                for h in handles {
                    h.join().unwrap();
                }
            });
        });
    }
    group.finish();
}

// ============================================================================
// END-TO-END CLI SIMULATION
// ============================================================================

fn bench_cli_license_check(c: &mut Criterion) {
    c.bench_function("license_cli_check_end_to_end", |b| {
        b.iter(|| {
            let license =
                black_box(LicenseCapsule::new("BENCH-CLI", LicenseTier::Starter).expect("License creation failed"));

            // Simulate: validate → check quota → record usage
            match license.validate() {
                Ok(LicenseStatus::Valid) => {
                    if license.remaining_gb().unwrap_or(u64::MAX) >= 100 {
                        let _result = black_box(license.record_usage(100));
                    }
                }
                _ => panic!("License check failed"),
            }
        });
    });
}

// ============================================================================
// CRITERION SETUP
// ============================================================================

criterion_group!(
    benches,
    bench_validation_basic,
    bench_validation_by_tier,
    bench_record_usage_single,
    bench_record_usage_bulk,
    bench_remaining_quota,
    bench_checksum_verification,
    bench_license_creation,
    bench_concurrent_validation,
    bench_concurrent_usage,
    bench_cli_license_check,
);

criterion_main!(benches);
