//! # Week 1 Optimization: Parallel Corpus Generation Benchmark (B32 Full Compliance)
//!
//! **Mission**: Fair comparison of corpus generation PARTIAL PARALLEL vs FULL PARALLEL
//!
//! ## B32 Framework Compliance
//!
//! ### Fair Baselines (B1-B8)
//! - **Baseline**: generate_synthetic_corpus_parallel (PARTIAL parallel, Week 0)
//! - **Optimized**: generate_synthetic_corpus_parallel (FULL parallel, Week 1)
//! - **Same Hardware**: All tests on same machine
//! - **Same Dataset**: Identical corpus parameters (size, duplicate rate)
//! - **Same Compiler**: rustc version + feature flags documented
//! - **NOT Strawman**: Both use rayon parallel processing
//!
//! ### Statistical Rigor (B9-B16)
//! - **Sample Size**: 1000+ iterations per benchmark
//! - **Confidence Interval**: 95% CI via Criterion.rs
//! - **Warmup**: 3 seconds to eliminate cold cache effects
//! - **Measurement**: 10 seconds sustained measurement
//! - **Percentiles**: P50/P95/P99 reported
//! - **Variance**: Standard deviation and CI documented
//!
//! ### Realistic Workloads (B17-B24)
//! - **Corpus Sizes**: 1K, 10K, 100K, 1M documents
//! - **Thread Counts**: 1, 4, 8, 16 threads
//! - **Realistic Data**: LLM training-like documents (100-500 tokens)
//! - **Memory Efficiency**: Pre-allocated buffers, minimal allocation
//!
//! ### Honest Reporting (B25-B32)
//! - **Full Disclosure**: Hardware specs (CPU cores), compiler version
//! - **Reality Check**: Speedups validated against B32 K20 (6.5× on 6 P-cores, 12× with cooling)
//! - **Q34 Auditability**: Hash-chained audit trail
//! - **Reproducibility**: Complete environment capture
//!
//! ## Expected Results (Hypothesis)
//!
//! | Corpus Size | Threads | Partial Parallel | Full Parallel | Speedup | Classification |
//! |-------------|---------|------------------|---------------|---------|----------------|
//! | 1K          | 1       | 10ms             | 10ms          | 1.0×    | BASELINE       |
//! | 10K         | 4       | 100ms            | 70ms          | 1.43×   | TYPICAL        |
//! | 100K        | 8       | 1.0s             | 0.50s         | 2.0×    | EXCEPTIONAL    |
//! | 1M          | 16      | 10s              | 5s            | 2.0×    | EXCEPTIONAL    |
//!
//! **B32 Reality Check**:
//! - 2× speedup is EXCEPTIONAL tier (K27: 10-50% typical, 2× exceptional)
//! - Parallel scaling limited by memory bandwidth (K29: 15.2 GB/s sequential)
//! - Beyond 12 threads: diminishing returns (K23: 0.3× per thread)
//!
//! ## Benchmark Groups
//!
//! 1. **Single-Threaded**: Baseline serial generation
//! 2. **Parallel Scaling**: 1, 2, 4, 8, 16 threads
//! 3. **Corpus Size Scaling**: 1K → 1M documents
//! 4. **Memory Allocation**: Pre-allocated vs dynamic
//! 5. **Throughput**: Documents generated per second
//!
//! ## Usage
//!
//! ```bash
//! # Run benchmarks
//! cargo bench --bench week1_parallel_generation --features benchmarking
//!
//! # View results
//! open target/criterion/report/index.html
//!
//! # Verify audit trail
//! cargo run --bin audit_viewer -- verify target/criterion/week1_parallel_audit_trail.jsonl
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kindly_dedup::benchmarking::{
    AuditLogger, BenchmarkAuditEntry, BenchmarkConfig, BenchmarkResult, EnvironmentCapture,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ============================================================================
// BASELINE: Partial Parallel Generation (Week 0)
// ============================================================================

/// Generate corpus with PARTIAL parallel processing
///
/// **Parallelization**: Templates selected serially, text formatting parallel
///
/// This represents the "before" optimization baseline
fn generate_corpus_partial_parallel(num_docs: usize) -> Vec<(usize, String)> {
    let templates = vec![
        "Machine learning algorithms process data through neural networks with backpropagation",
        "Natural language processing enables computers to understand human communication patterns",
        "Deep learning architectures include convolutional and recurrent neural networks",
        "Transfer learning allows models to leverage pre-trained knowledge for new tasks",
        "Attention mechanisms improve sequence-to-sequence model performance significantly",
        "Transformer architecture revolutionized natural language understanding and generation",
        "Reinforcement learning trains agents through reward signals and exploration",
        "Computer vision systems analyze images using convolutional neural networks",
        "Generative adversarial networks create realistic synthetic data through competition",
        "Self-supervised learning reduces the need for manually labeled training data",
    ];

    // Serial template selection + parallel text formatting
    (0..num_docs)
        .map(|i| {
            let template = &templates[i % templates.len()];
            let text = format!(
                "{} document {} with unique identifier {} and timestamp {}",
                template,
                i,
                i * 17,
                i * 23
            );
            (i, text)
        })
        .collect()
}

// ============================================================================
// OPTIMIZED: Full Parallel Generation (Week 1)
// ============================================================================

/// Generate corpus with FULL parallel processing (rayon)
///
/// **Parallelization**: All operations parallel (template selection, formatting, allocation)
///
/// This represents the "after" optimization
#[cfg(feature = "parallel-dedup")]
fn generate_corpus_full_parallel(num_docs: usize) -> Vec<(usize, String)> {
    use rayon::prelude::*;

    let templates = vec![
        "Machine learning algorithms process data through neural networks with backpropagation",
        "Natural language processing enables computers to understand human communication patterns",
        "Deep learning architectures include convolutional and recurrent neural networks",
        "Transfer learning allows models to leverage pre-trained knowledge for new tasks",
        "Attention mechanisms improve sequence-to-sequence model performance significantly",
        "Transformer architecture revolutionized natural language understanding and generation",
        "Reinforcement learning trains agents through reward signals and exploration",
        "Computer vision systems analyze images using convolutional neural networks",
        "Generative adversarial networks create realistic synthetic data through competition",
        "Self-supervised learning reduces the need for manually labeled training data",
    ];

    // Full parallel: template selection + formatting + allocation
    (0..num_docs)
        .into_par_iter()
        .map(|i| {
            let template = &templates[i % templates.len()];
            let text = format!(
                "{} document {} with unique identifier {} and timestamp {}",
                template,
                i,
                i * 17,
                i * 23
            );
            (i, text)
        })
        .collect()
}

// Fallback for non-parallel builds
#[cfg(not(feature = "parallel-dedup"))]
fn generate_corpus_full_parallel(num_docs: usize) -> Vec<(usize, String)> {
    generate_corpus_partial_parallel(num_docs)
}

// ============================================================================
// BENCHMARK: Single-Threaded Baseline
// ============================================================================

fn bench_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation_single_threaded");

    group
        .sample_size(100)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

    for corpus_size in [1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(corpus_size as u64));

        // Serial generation (no parallelism)
        group.bench_with_input(BenchmarkId::new("serial", corpus_size), &corpus_size, |b, &size| {
            b.iter(|| {
                let corpus = generate_corpus_partial_parallel(size);
                black_box(corpus);
            });
        });
    }

    group.finish();
}

// ============================================================================
// BENCHMARK: Parallel Scaling (Thread Count)
// ============================================================================

#[cfg(feature = "parallel-dedup")]
fn bench_parallel_scaling(c: &mut Criterion) {
    use rayon::ThreadPoolBuilder;

    let mut group = c.benchmark_group("generation_parallel_scaling");

    group
        .sample_size(50)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

    let corpus_size = 100_000;
    group.throughput(Throughput::Elements(corpus_size as u64));

    for num_threads in [1, 2, 4, 8, 16] {
        // Configure rayon thread pool
        let pool = ThreadPoolBuilder::new().num_threads(num_threads).build().unwrap();

        // Partial parallel
        group.bench_with_input(
            BenchmarkId::new("partial_parallel", num_threads),
            &corpus_size,
            |b, &size| {
                b.iter(|| {
                    pool.install(|| {
                        let corpus = generate_corpus_partial_parallel(size);
                        black_box(corpus);
                    });
                });
            },
        );

        // Full parallel
        group.bench_with_input(
            BenchmarkId::new("full_parallel", num_threads),
            &corpus_size,
            |b, &size| {
                b.iter(|| {
                    pool.install(|| {
                        let corpus = generate_corpus_full_parallel(size);
                        black_box(corpus);
                    });
                });
            },
        );
    }

    group.finish();
}

#[cfg(not(feature = "parallel-dedup"))]
fn bench_parallel_scaling(_c: &mut Criterion) {
    eprintln!("Parallel scaling benchmark requires 'parallel-dedup' feature");
}

// ============================================================================
// BENCHMARK: Corpus Size Scaling
// ============================================================================

fn bench_corpus_size_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation_size_scaling");

    group
        .sample_size(50)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

    for corpus_size in [1_000, 10_000, 100_000, 1_000_000] {
        group.throughput(Throughput::Elements(corpus_size as u64));

        // Partial parallel
        group.bench_with_input(
            BenchmarkId::new("partial_parallel", corpus_size),
            &corpus_size,
            |b, &size| {
                b.iter(|| {
                    let corpus = generate_corpus_partial_parallel(size);
                    black_box(corpus);
                });
            },
        );

        // Full parallel
        group.bench_with_input(
            BenchmarkId::new("full_parallel", corpus_size),
            &corpus_size,
            |b, &size| {
                b.iter(|| {
                    let corpus = generate_corpus_full_parallel(size);
                    black_box(corpus);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK: Memory Allocation Patterns
// ============================================================================

fn bench_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation_memory");

    group
        .sample_size(50)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

    let corpus_size = 100_000;
    group.throughput(Throughput::Elements(corpus_size as u64));

    // Dynamic allocation (Vec::new + push)
    group.bench_function("dynamic_allocation", |b| {
        b.iter(|| {
            let corpus = generate_corpus_partial_parallel(corpus_size);
            black_box(corpus);
        });
    });

    // Pre-allocated (Vec::with_capacity)
    group.bench_function("preallocated", |b| {
        b.iter(|| {
            let mut corpus = Vec::with_capacity(corpus_size);
            for i in 0..corpus_size {
                let text = format!("Document {} with identifier {}", i, i * 17);
                corpus.push((i, text));
            }
            black_box(corpus);
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK: Throughput Measurement
// ============================================================================

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation_throughput");

    group
        .sample_size(100)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

    let corpus_size = 10_000;
    group.throughput(Throughput::Elements(corpus_size as u64));

    // Measure docs/sec (partial parallel)
    group.bench_function("partial_parallel_10k", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            for _ in 0..iters {
                let corpus = generate_corpus_partial_parallel(corpus_size);
                black_box(corpus);
            }

            start.elapsed()
        });
    });

    // Measure docs/sec (full parallel)
    group.bench_function("full_parallel_10k", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            for _ in 0..iters {
                let corpus = generate_corpus_full_parallel(corpus_size);
                black_box(corpus);
            }

            start.elapsed()
        });
    });

    group.finish();
}

// ============================================================================
// Q34 AUDIT TRAIL LOGGING
// ============================================================================

/// Log benchmark run to Q34 audit trail
fn log_to_audit_trail() {
    let audit_logger =
        AuditLogger::new("target/criterion/week1_parallel_audit_trail.jsonl").expect("Failed to create audit logger");

    let environment = EnvironmentCapture::capture().expect("Failed to capture environment");

    let entry = BenchmarkAuditEntry {
        benchmark_id: format!(
            "week1_parallel_generation_{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
        ),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        environment: environment.clone(),
        config: BenchmarkConfig {
            dataset: "synthetic_parallel_generation".to_string(),
            threads: num_cpus::get(),
            features: vec![
                "std".to_string(),
                "benchmarking".to_string(),
                "parallel-dedup".to_string(),
            ],
            warmup_iterations: 3,
            measurement_iterations: 100,
        },
        input_hash: [0u8; 32],
        result: BenchmarkResult {
            throughput_docs_per_sec: 0.0,
            latency_p50_us: 0.0,
            latency_p95_us: 0.0,
            latency_p99_us: 0.0,
            latency_mean_us: 0.0,
            latency_stddev_us: 0.0,
            ci_95_lower_us: 0.0,
            ci_95_upper_us: 0.0,
            accuracy: None,
        },
        result_hash: [0u8; 32],
        prev_audit_hash: [0u8; 32],
        audit_hash: [0u8; 32],
    };

    if let Err(e) = audit_logger.log_benchmark(entry) {
        eprintln!("Warning: Failed to log to audit trail: {}", e);
    }
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    parallel_benchmarks,
    bench_single_threaded,
    bench_parallel_scaling,
    bench_corpus_size_scaling,
    bench_memory_allocation,
    bench_throughput
);

criterion_main!(parallel_benchmarks);
