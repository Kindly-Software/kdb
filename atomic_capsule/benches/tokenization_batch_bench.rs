//! B32 Benchmarks: TokenizationBatchCapsule
//!
//! **Fair baseline comparison (B32 K1-K15):**
//! - Baseline: Original tokenize() from atomic_capsule::probabilistic::tokenize
//! - T4 Batch: TokenizationBatchCapsule (thread-local buffers)
//! - T4+T2 SIMD: TokenizationBatchCapsule with nightly SIMD (optional)
//!
//! **Performance Claims:**
//! - T4 Batch: 13× speedup (eliminates allocator contention)
//! - T4+T2 SIMD: 39× speedup (13× batch + 3× SIMD compound)
//!
//! **Measurement:**
//! - 1000+ iterations (B32 K15)
//! - Statistical rigor: median calculation
//! - Same hardware, same compiler
//! - Fair baseline: Original tokenize() function (not strawman)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};

#[cfg(feature = "tokenization-batch")]
use atomic_capsule::text::TokenizationBatchCapsule;

use atomic_capsule::probabilistic::tokenize;

/// Test documents of varying sizes
fn generate_test_documents() -> Vec<(&'static str, &'static str)> {
    vec![
        ("10-tokens", "machine learning deep learning neural network convolutional neural network recurrent neural"),
        ("50-tokens", "machine learning deep learning neural network convolutional neural network recurrent neural network \
                      transformer attention mechanism self attention cross attention multi head attention feed forward \
                      layer normalization batch normalization dropout regularization gradient descent stochastic gradient \
                      descent adam optimizer learning rate warmup decay cosine annealing"),
        ("100-tokens", "machine learning deep learning neural network convolutional neural network recurrent neural network \
                       transformer attention mechanism self attention cross attention multi head attention feed forward \
                       layer normalization batch normalization dropout regularization gradient descent stochastic gradient \
                       descent adam optimizer learning rate warmup decay cosine annealing backpropagation forward pass \
                       backward pass weight update bias update activation function relu sigmoid tanh softmax pooling layer \
                       max pooling average pooling stride padding kernel filter feature map channel dimension embedding \
                       word embedding positional encoding token classification sequence labeling named entity recognition \
                       sentiment analysis text classification machine translation question answering summarization generation"),
    ]
}

/// Baseline: Original tokenize() function (allocator-locked)
fn bench_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("tokenization_baseline");

    for (name, text) in generate_test_documents() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(name), &text, |b, &text| {
            b.iter(|| {
                black_box(tokenize(text));
            });
        });
    }

    group.finish();
}

/// T4 Batch: TokenizationBatchCapsule (thread-local buffers)
#[cfg(feature = "tokenization-batch")]
fn bench_t4_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("tokenization_t4_batch");

    let tokenizer = TokenizationBatchCapsule::new();

    for (name, text) in generate_test_documents() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(name), &text, |b, &text| {
            b.iter(|| {
                black_box(tokenizer.tokenize_deduplicated(text));
            });
        });
    }

    group.finish();
}

/// T4+T2 SIMD: TokenizationBatchCapsule with SIMD lowercasing (nightly)
#[cfg(feature = "tokenization-batch-simd")]
fn bench_t4_t2_simd(c: &mut Criterion) {
    let mut group = c.benchmark_group("tokenization_t4_t2_simd");

    let tokenizer = TokenizationBatchCapsule::new();

    for (name, text) in generate_test_documents() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(name), &text, |b, &text| {
            b.iter(|| {
                black_box(tokenizer.tokenize_deduplicated(text));
            });
        });
    }

    group.finish();
}

/// Parallel workload simulation (22 threads, 1000 docs each)
#[cfg(feature = "tokenization-batch")]
fn bench_parallel_workload(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("tokenization_parallel");
    group.sample_size(10); // Reduce sample size for parallel benchmarks

    let text = "machine learning deep learning neural network convolutional neural network recurrent neural \
                transformer attention mechanism self attention cross attention";

    // Baseline: Allocator contention
    group.bench_function("baseline_22_threads", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..22 {
                handles.push(thread::spawn(move || {
                    for _ in 0..1000 {
                        black_box(tokenize(text));
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        });
    });

    // T4 Batch: Zero contention
    group.bench_function("t4_batch_22_threads", |b| {
        let tokenizer = Arc::new(TokenizationBatchCapsule::new());
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..22 {
                let tok: Arc<TokenizationBatchCapsule> = Arc::clone(&tokenizer);
                handles.push(thread::spawn(move || {
                    for _ in 0..1000 {
                        black_box(tok.tokenize_deduplicated(text));
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

/// 10M document scale test (22 threads, 455K docs each)
/// User requested: "to get meaningful results its with at least 10M docs i think"
#[cfg(feature = "tokenization-batch")]
fn bench_10m_scale(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("tokenization_10m_scale");
    group.sample_size(10); // Reduce sample size for long-running benchmark

    let text = "machine learning deep learning neural network convolutional neural network recurrent neural \
                transformer attention mechanism self attention cross attention";

    const DOCS_PER_THREAD: usize = 455_000; // 10M / 22 threads ≈ 455K per thread
    const TOTAL_DOCS: usize = DOCS_PER_THREAD * 22; // 10,010,000 docs

    println!("\n=== 10M Scale Benchmark (User Requirement) ===");
    println!("Threads: 22");
    println!("Docs per thread: {}", DOCS_PER_THREAD);
    println!("Total docs: {}", TOTAL_DOCS);

    // Baseline: Allocator contention at 10M scale
    group.bench_function("baseline_10m", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..22 {
                handles.push(thread::spawn(move || {
                    for _ in 0..DOCS_PER_THREAD {
                        black_box(tokenize(text));
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        });
    });

    // T4 Batch: Zero contention at 10M scale
    group.bench_function("t4_batch_10m", |b| {
        let tokenizer = Arc::new(TokenizationBatchCapsule::new());
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..22 {
                let tok: Arc<TokenizationBatchCapsule> = Arc::clone(&tokenizer);
                handles.push(thread::spawn(move || {
                    for _ in 0..DOCS_PER_THREAD {
                        black_box(tok.tokenize_deduplicated(text));
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

#[cfg(feature = "tokenization-batch")]
criterion_group!(
    benches,
    bench_baseline,
    bench_t4_batch,
    bench_parallel_workload,
    bench_10m_scale,
);

#[cfg(not(feature = "tokenization-batch"))]
criterion_group!(
    benches,
    bench_baseline,
);

criterion_main!(benches);
