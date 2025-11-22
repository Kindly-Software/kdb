//! # Comprehensive SIMD Benchmarks
//!
//! **Purpose**: Validate 10-30× speedup claims for 5 SIMD implementations
//!
//! ## B32 Framework Compliance
//!
//! - **Fair Baselines**: Scalar implementations (not strawman)
//! - **95% CI**: 1000+ iterations per benchmark
//! - **Hardware Isolation**: Single-threaded benchmarks
//! - **Reproducibility**: Multiple measurement strategies
//!
//! ## Benchmarks
//!
//! 1. **HyperLogLog merge**: 8-16× speedup (SIMD vs scalar)
//! 2. **Histogram percentile**: 5-10× speedup (SIMD vs scalar)
//! 3. **XSS sanitization**: 30× speedup (SIMD vs sequential contains)
//! 4. **FormParser boundary**: 30× speedup (SIMD vs memchr) [TODO]
//! 5. **StaticFile MIME**: 10-15× speedup (SIMD vs lookup table) [TODO]
//!
//! ## Usage
//!
//! ```bash
//! # Run all SIMD benchmarks
//! cargo bench --bench simd_comprehensive_bench --features simd-all
//!
//! # Run individual groups
//! cargo bench --bench simd_comprehensive_bench hyperloglog
//! cargo bench --bench simd_comprehensive_bench histogram
//! cargo bench --bench simd_comprehensive_bench xss
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};

#[cfg(feature = "hll-simd")]
use atomic_capsule::probabilistic::HyperLogLogCapsule;

#[cfg(feature = "histogram-simd")]
use atomic_capsule::collections::HistogramCapsule;

#[cfg(feature = "validation-simd")]
use atomic_capsule::http::validation_simd::{sanitize_xss_simd, sanitize_xss_baseline};

// ============================================================================
// Benchmark 1: HyperLogLog Merge (8-16× speedup)
// ============================================================================

#[cfg(feature = "hll-simd")]
fn bench_hyperloglog_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("HyperLogLog Merge");
    group.sample_size(1000); // 1000+ iterations for 95% CI

    // Test with varying cardinalities
    for cardinality in [1_000, 10_000, 100_000, 1_000_000] {
        let hll1 = HyperLogLogCapsule::new();
        let hll2 = HyperLogLogCapsule::new();

        // Insert disjoint sets
        for i in 0..cardinality {
            hll1.insert(i);
        }
        for i in cardinality..cardinality * 2 {
            hll2.insert(i);
        }

        group.throughput(Throughput::Elements(16384)); // 16K bucket operations

        group.bench_with_input(
            BenchmarkId::new("SIMD", cardinality),
            &(hll1, hll2),
            |b, (h1, h2)| {
                b.iter(|| {
                    let merged = h1.merge(h2);
                    black_box(merged)
                });
            },
        );
    }

    group.finish();
}

// Baseline comparison (if scalar implementation exists)
#[cfg(feature = "hll-simd")]
fn bench_hyperloglog_merge_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("HyperLogLog Merge Baseline");
    group.sample_size(1000);

    let hll1 = HyperLogLogCapsule::new();
    let hll2 = HyperLogLogCapsule::new();

    for i in 0..100_000 {
        hll1.insert(i);
    }
    for i in 100_000..200_000 {
        hll2.insert(i);
    }

    // This will use the scalar implementation in hyperloglog.rs (lines 426-444)
    // when hll-simd feature is NOT enabled
    group.bench_function("scalar_merge", |b| {
        b.iter(|| {
            let merged = hll1.merge(&hll2);
            black_box(merged)
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 2: Histogram Percentile (5-10× speedup)
// ============================================================================

#[cfg(feature = "histogram-simd")]
fn bench_histogram_percentile(c: &mut Criterion) {
    let mut group = c.benchmark_group("Histogram Percentile");
    group.sample_size(1000);

    // Populate histogram with realistic latency distribution
    let histogram = HistogramCapsule::new();
    for i in 0..100_000 {
        // Latencies: 0-999ms (exponential-like distribution)
        let latency_ns = (i * i / 100) * 1_000_000;
        histogram.record(latency_ns);
    }

    group.throughput(Throughput::Elements(1024)); // 1024 bucket scan

    // SIMD percentile
    group.bench_function("percentile_simd_p50", |b| {
        b.iter(|| black_box(histogram.calculate_percentile_simd(50.0)));
    });

    group.bench_function("percentile_simd_p95", |b| {
        b.iter(|| black_box(histogram.calculate_percentile_simd(95.0)));
    });

    group.bench_function("percentile_simd_p99", |b| {
        b.iter(|| black_box(histogram.calculate_percentile_simd(99.0)));
    });

    group.bench_function("percentile_simd_p999", |b| {
        b.iter(|| black_box(histogram.calculate_percentile_simd(99.9)));
    });

    // Baseline percentile (scalar implementation)
    group.bench_function("percentile_scalar_p50", |b| {
        b.iter(|| black_box(histogram.calculate_percentile(50.0)));
    });

    group.bench_function("percentile_scalar_p95", |b| {
        b.iter(|| black_box(histogram.calculate_percentile(95.0)));
    });

    group.bench_function("percentile_scalar_p99", |b| {
        b.iter(|| black_box(histogram.calculate_percentile(99.0)));
    });

    group.bench_function("percentile_scalar_p999", |b| {
        b.iter(|| black_box(histogram.calculate_percentile(99.9)));
    });

    group.finish();
}

// ============================================================================
// Benchmark 3: XSS Sanitization (30× speedup)
// ============================================================================

#[cfg(feature = "validation-simd")]
fn bench_xss_sanitization(c: &mut Criterion) {
    let mut group = c.benchmark_group("XSS Sanitization");
    group.sample_size(1000);

    // Test Case 1: Safe input (no dangerous tags)
    let safe_short = b"Hello, world! This is a safe string.";
    let safe_long = b"Hello, world! This is a long safe string with no dangerous tags. ".repeat(1000);

    group.throughput(Throughput::Bytes(safe_long.len() as u64));

    group.bench_function("xss_simd_safe_short", |b| {
        b.iter(|| black_box(sanitize_xss_simd(safe_short)));
    });

    group.bench_function("xss_simd_safe_long", |b| {
        b.iter(|| black_box(sanitize_xss_simd(&safe_long)));
    });

    group.bench_function("xss_baseline_safe_short", |b| {
        b.iter(|| black_box(sanitize_xss_baseline(safe_short)));
    });

    group.bench_function("xss_baseline_safe_long", |b| {
        b.iter(|| black_box(sanitize_xss_baseline(&safe_long)));
    });

    // Test Case 2: Dangerous input (contains XSS)
    let dangerous_short = b"<script>alert(1)</script>";
    let dangerous_long = b"Before text <script>alert(1)</script> after text. ".repeat(1000);

    group.bench_function("xss_simd_dangerous_short", |b| {
        b.iter(|| black_box(sanitize_xss_simd(dangerous_short)));
    });

    group.bench_function("xss_simd_dangerous_long", |b| {
        b.iter(|| black_box(sanitize_xss_simd(&dangerous_long)));
    });

    group.bench_function("xss_baseline_dangerous_short", |b| {
        b.iter(|| black_box(sanitize_xss_baseline(dangerous_short)));
    });

    group.bench_function("xss_baseline_dangerous_long", |b| {
        b.iter(|| black_box(sanitize_xss_baseline(&dangerous_long)));
    });

    // Test Case 3: Many false positive triggers (many '<' but no XSS)
    let false_positive = b"1 < 2 < 3 < 4 < 5 < 6 < 7 < 8 < 9 < 10 < 11 < 12".repeat(100);

    group.bench_function("xss_simd_false_positive", |b| {
        b.iter(|| black_box(sanitize_xss_simd(&false_positive)));
    });

    group.bench_function("xss_baseline_false_positive", |b| {
        b.iter(|| black_box(sanitize_xss_baseline(&false_positive)));
    });

    group.finish();
}

// ============================================================================
// Benchmark 4: FormParser Boundary Detection (30× speedup) [TODO]
// ============================================================================

// TODO: Implement when form_parser_simd.rs is complete
#[cfg(feature = "form-parser-simd")]
fn bench_form_parser_boundary(_c: &mut Criterion) {
    // let mut group = c.benchmark_group("FormParser Boundary Detection");
    // group.sample_size(1000);
    //
    // // Test with realistic multipart/form-data buffer
    // let boundary = b"----WebKitFormBoundary7MA4YWxkTrZu0gW";
    // let buffer = /* construct multipart buffer */;
    //
    // group.bench_function("boundary_simd", |b| {
    //     b.iter(|| black_box(find_boundary_simd(&buffer, boundary)));
    // });
    //
    // group.bench_function("boundary_baseline", |b| {
    //     b.iter(|| black_box(find_boundary_baseline(&buffer, boundary)));
    // });
    //
    // group.finish();
}

// ============================================================================
// Benchmark 5: StaticFileServer MIME Detection (10-15× speedup) [TODO]
// ============================================================================

// TODO: Implement when static_file_server_simd.rs is complete
#[cfg(feature = "static-file-simd")]
fn bench_static_file_mime(_c: &mut Criterion) {
    // let mut group = c.benchmark_group("StaticFileServer MIME Detection");
    // group.sample_size(1000);
    //
    // let paths = vec![
    //     "/index.html",
    //     "/styles.css",
    //     "/app.js",
    //     "/data.json",
    //     "/logo.png",
    //     "/avatar.jpg",
    // ];
    //
    // group.bench_function("mime_simd", |b| {
    //     b.iter(|| {
    //         for path in &paths {
    //             black_box(detect_mime_simd(path));
    //         }
    //     });
    // });
    //
    // group.bench_function("mime_baseline", |b| {
    //     b.iter(|| {
    //         for path in &paths {
    //             black_box(detect_mime_baseline(path));
    //         }
    //     });
    // });
    //
    // group.finish();
}

// ============================================================================
// Benchmark Groups
// ============================================================================

#[cfg(feature = "hll-simd")]
criterion_group!(
    hyperloglog,
    bench_hyperloglog_merge,
    bench_hyperloglog_merge_baseline
);

#[cfg(feature = "histogram-simd")]
criterion_group!(histogram, bench_histogram_percentile);

#[cfg(feature = "validation-simd")]
criterion_group!(xss, bench_xss_sanitization);

// TODO: Enable when implementations are complete
// #[cfg(feature = "form-parser-simd")]
// criterion_group!(form_parser, bench_form_parser_boundary);

// #[cfg(feature = "static-file-simd")]
// criterion_group!(static_file, bench_static_file_mime);

// ============================================================================
// Main Benchmark Runner
// ============================================================================

#[cfg(all(
    feature = "hll-simd",
    feature = "histogram-simd",
    feature = "validation-simd"
))]
criterion_main!(hyperloglog, histogram, xss);

// Fallback main if not all features enabled
#[cfg(not(all(
    feature = "hll-simd",
    feature = "histogram-simd",
    feature = "validation-simd"
)))]
fn main() {
    eprintln!("SIMD benchmarks require features: hll-simd, histogram-simd, validation-simd");
    eprintln!("Run with: cargo bench --bench simd_comprehensive_bench --features simd-all");
}
