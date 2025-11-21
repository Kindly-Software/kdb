//! # Phase 4 Compilation Performance Benchmark
//!
//! **B32 Framework Compliance**: Measure compile-time impact of FixedPointSerialize trait
//!
//! ## Mission
//!
//! Validate that FixedPointSerialize trait integration has <10% compile-time impact:
//! 1. **Baseline**: Compile atomic_capsule without trait
//! 2. **With Trait**: Compile with FixedPointSerialize trait
//! 3. **Per-Migration**: Baseline → clapi_core integration → full
//!
//! ## B32 Honest Claims
//!
//! - Target: <10% compile-time impact (realistic for trait addition)
//! - If <5%: Claim "negligible compile-time overhead"
//! - If 5-10%: Claim "minimal compile-time overhead"
//! - If >10%: Document reason and mitigation plan
//!
//! ## Methodology
//!
//! 1. Clean build: `cargo clean`
//! 2. Baseline: `cargo build --lib` (no features)
//! 3. With trait: `cargo build --lib --features capsule-serialize`
//! 4. Repeat 10 times, report P50/P95/P99
//!
//! ## Expected Results
//!
//! - Baseline: ~5-10s (atomic_capsule is 14K+ LOC)
//! - With trait: ~5.5-11s (<10% overhead expected)
//! - Total Phase 4 overhead: <500ms (trait + impls)

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::process::Command;
use std::time::{Duration, Instant};

/// Measure compilation time for a given feature set
fn measure_compile_time(features: &[&str]) -> Duration {
    // Clean build
    Command::new("cargo")
        .args(&["clean"])
        .output()
        .expect("Failed to clean");

    // Build with features
    let start = Instant::now();
    let mut cmd = Command::new("cargo");
    cmd.args(&["build", "--lib", "--release"]);

    if !features.is_empty() {
        cmd.arg("--features");
        cmd.arg(features.join(","));
    }

    let output = cmd.output().expect("Failed to compile");

    if !output.status.success() {
        panic!(
            "Compilation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    start.elapsed()
}

/// Benchmark baseline compilation (no capsule-serialize)
fn bench_baseline_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation/baseline");
    group.sample_size(10); // Reduce sample size for long-running benchmarks
    group.measurement_time(Duration::from_secs(60)); // Allow enough time

    group.bench_function("no_features", |b| {
        b.iter_custom(|_| black_box(measure_compile_time(&[])));
    });

    group.finish();
}

/// Benchmark with FixedPointSerialize trait
fn bench_with_trait_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation/with_trait");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));

    group.bench_function("capsule_serialize", |b| {
        b.iter_custom(|_| black_box(measure_compile_time(&["capsule-serialize"])));
    });

    group.finish();
}

/// Benchmark incremental compilation impact
fn bench_incremental_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation/incremental");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));

    // First clean build
    measure_compile_time(&["capsule-serialize"]);

    // Touch a trait implementation file
    std::fs::write(
        "src/serialize/fixed_point_serialize.rs.touch",
        "// Touch file for incremental build test\n",
    )
    .ok();

    group.bench_function("incremental_trait_change", |b| {
        b.iter_custom(|_| {
            // Touch the file to trigger recompilation
            Command::new("touch")
                .arg("src/serialize/fixed_point_serialize.rs")
                .output()
                .ok();

            let start = Instant::now();
            let output = Command::new("cargo")
                .args(&[
                    "build",
                    "--lib",
                    "--release",
                    "--features",
                    "capsule-serialize",
                ])
                .output()
                .expect("Failed to compile");

            if !output.status.success() {
                panic!("Incremental compilation failed");
            }

            black_box(start.elapsed())
        });
    });

    group.finish();
}

/// Benchmark parallel compilation (check for bottlenecks)
fn bench_parallel_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation/parallel");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));

    for jobs in [1, 2, 4, 8] {
        group.bench_function(format!("{}_jobs", jobs), |b| {
            b.iter_custom(|_| {
                Command::new("cargo").args(&["clean"]).output().ok();

                let start = Instant::now();
                let output = Command::new("cargo")
                    .args(&[
                        "build",
                        "--lib",
                        "--release",
                        "--features",
                        "capsule-serialize",
                        "-j",
                        &jobs.to_string(),
                    ])
                    .output()
                    .expect("Failed to compile");

                if !output.status.success() {
                    panic!("Parallel compilation failed");
                }

                black_box(start.elapsed())
            });
        });
    }

    group.finish();
}

/// Measure type checking overhead (cargo check vs cargo build)
fn bench_type_checking_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation/type_checking");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));

    group.bench_function("cargo_check", |b| {
        b.iter_custom(|_| {
            Command::new("cargo").args(&["clean"]).output().ok();

            let start = Instant::now();
            let output = Command::new("cargo")
                .args(&["check", "--lib", "--features", "capsule-serialize"])
                .output()
                .expect("Failed to check");

            if !output.status.success() {
                panic!("Type checking failed");
            }

            black_box(start.elapsed())
        });
    });

    group.bench_function("cargo_build", |b| {
        b.iter_custom(|_| {
            Command::new("cargo").args(&["clean"]).output().ok();

            let start = Instant::now();
            let output = Command::new("cargo")
                .args(&[
                    "build",
                    "--lib",
                    "--release",
                    "--features",
                    "capsule-serialize",
                ])
                .output()
                .expect("Failed to build");

            if !output.status.success() {
                panic!("Build failed");
            }

            black_box(start.elapsed())
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_baseline_compilation,
    bench_with_trait_compilation,
    bench_incremental_compilation,
    bench_parallel_compilation,
    bench_type_checking_overhead,
);

criterion_main!(benches);
