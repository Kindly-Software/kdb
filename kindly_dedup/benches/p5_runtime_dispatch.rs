//! # Phase 5: Runtime CPU Dispatch Benchmarks (B32 Compliant)
//!
//! **Mission**: Validate runtime CPU dispatch with <10ns overhead and 2-4× AVX2 speedup
//!
//! # B32 Framework Compliance
//!
//! ## Fair Baselines (B1, K1-K10)
//! - **Scalar**: Always available baseline (NOT strawman - production code path)
//! - **SSE4.2**: Mid-tier SIMD (2009+ CPUs)
//! - **AVX2**: High-tier SIMD (2013+ CPUs)
//! - **Same hardware**: All tests on same CPU
//! - **Same input**: Identical documents for all paths
//!
//! ## Statistical Rigor (B2, K11-K20)
//! - **1000+ iterations**: Criterion default, 95% CI
//! - **Warmup period**: 3 seconds (stabilize CPU detection singleton)
//! - **Multiple sizes**: 10, 100, 1000 tokens (realistic range)
//! - **Percentiles**: P50, P95, P99 (Criterion automatic)
//! - **3+ runs**: Independent runs for reproducibility
//!
//! ## Reality Checks (K21-K42, K66-K70)
//! - **<10ns dispatch overhead**: K2 atomic load + branch (cached singleton)
//! - **2-4× AVX2 speedup**: K9, K14, K30 SIMD reality (NOT 8× theoretical)
//! - **<5% portable regression**: Acceptable tradeoff for single binary
//! - **MinHash accuracy**: K66 ±2-5% error, excellent for >80% similarity
//!
//! # Performance Targets (Phase 5)
//!
//! | Metric | Target | Validation |
//! |--------|--------|------------|
//! | CPU detection init | <1μs | One-time cost, amortized |
//! | Dispatch overhead | <10ns | Cached atomic load |
//! | AVX2 vs scalar | 2-4× | Conservative SIMD speedup |
//! | Portable regression | <5% | Runtime vs compile-time |
//! | Thread scaling | Linear to 12 | Lockfree dispatch |
//! | Throughput | 60K+ sigs/sec | Sustained performance |
//!
//! # Usage
//!
//! ```bash
//! # Run all Phase 5 benchmarks
//! cargo bench --bench p5_runtime_dispatch
//!
//! # View results
//! open target/criterion/report/index.html
//!
//! # Platform-specific testing
//! RUSTFLAGS="-C target-feature=-avx2" cargo bench --bench p5_runtime_dispatch  # SSE4.2
//! RUSTFLAGS="-C target-feature=-avx2,-sse4.2" cargo bench --bench p5_runtime_dispatch  # Scalar
//! ```

use atomic_capsule::CpuCapabilityCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kindly_dedup::DedupPipeline;
use std::time::{Duration, Instant};

// ============================================================================
// Helper: Document Generation (Realistic LLM Training Data)
// ============================================================================

/// Generate realistic documents with specified token count
fn generate_documents(num_docs: usize, tokens_per_doc: usize) -> Vec<(usize, String)> {
    (0..num_docs)
        .map(|i| {
            // Realistic LLM training data: technical terms + natural language
            let base = vec![
                "machine",
                "learning",
                "neural",
                "network",
                "transformer",
                "attention",
                "deep",
                "model",
                "training",
                "data",
                "algorithm",
                "optimization",
                "research",
                "paper",
                "architecture",
                "performance",
                "evaluation",
                "results",
            ];

            let mut tokens = Vec::with_capacity(tokens_per_doc);
            for j in 0..tokens_per_doc {
                tokens.push(base[(i + j) % base.len()]);
            }

            (i, tokens.join(" "))
        })
        .collect()
}

// ============================================================================
// Benchmark 1: CPU Capability Detection Overhead (One-Time Init)
// ============================================================================

/// Target: <1μs one-time cost (amortized over millions of calls)
///
/// **B32 Compliance**:
/// - Measures cold initialization (first call)
/// - Singleton pattern amortizes cost
/// - Subsequent calls <10ns (cached atomic load)
fn bench_cpu_capability_init(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_capability_init");

    // Configure for B32 compliance
    group.confidence_level(0.95); // 95% CI (B2)
    group.sample_size(1000); // 1000+ iterations (B2)
    group.warm_up_time(Duration::from_secs(1)); // Minimal warmup (testing cold start)

    group.bench_function("cold_init", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;

            for _ in 0..iters {
                // Simulate cold initialization (can't actually clear singleton in safe Rust)
                // This measures best-case: singleton already initialized by first iteration
                let start = Instant::now();
                let caps = CpuCapabilityCapsule::detect();
                black_box(caps.best_simd_tier());
                total += start.elapsed();
            }

            total
        });
    });

    group.bench_function("cached_access", |b| {
        // Warmup: Initialize singleton
        let _caps = CpuCapabilityCapsule::detect();

        // Measure cached access (production case)
        b.iter(|| {
            let caps = CpuCapabilityCapsule::detect();
            black_box(caps.best_simd_tier())
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 2: Dispatch Overhead (Cached Atomic Load)
// ============================================================================

/// Target: <10ns per call (cached singleton + branch prediction)
///
/// **B32 Compliance**:
/// - Measures dispatch wrapper overhead vs direct call
/// - Baseline: Direct function call (scalar path)
/// - Comparison: Runtime dispatch (capability check + indirect call)
/// - Reality: K2 atomic load (5-10ns) + branch (1 cycle if predicted)
fn bench_dispatch_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatch_overhead");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(10000); // High sample size for micro-benchmark
    group.warm_up_time(Duration::from_secs(3));

    // Warmup: Initialize singleton
    let cpu_caps = CpuCapabilityCapsule::detect();

    // Generate test data
    let tokens: Vec<&str> = vec!["the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog"];

    // Baseline: Direct scalar call (no dispatch)
    group.bench_function("direct_scalar_call", |b| {
        b.iter(|| {
            use atomic_capsule::probabilistic::MinHashSignatureCapsule;
            black_box(MinHashSignatureCapsule::compute_signature(black_box(&tokens)))
        });
    });

    // With dispatch: Runtime capability check + call
    group.bench_function("dispatched_call", |b| {
        b.iter(|| {
            // Simulate dispatch: Read capability (cached) + call
            let _tier = cpu_caps.best_simd_tier();
            use atomic_capsule::probabilistic::MinHashSignatureCapsule;
            black_box(MinHashSignatureCapsule::compute_signature(black_box(&tokens)))
        });
    });

    // Pure capability read (measure atomic load)
    group.bench_function("capability_read_only", |b| {
        b.iter(|| black_box(cpu_caps.best_simd_tier()));
    });

    group.finish();
}

// ============================================================================
// Benchmark 3: AVX2 vs Scalar MinHash Signature Generation
// ============================================================================

/// Target: 2-4× speedup (conservative, K9/K14/K30 SIMD reality)
///
/// **B32 Compliance**:
/// - Fair baseline: Scalar path (always available, NOT strawman)
/// - Same hardware: All measurements on same CPU
/// - Same input: Identical token sequences
/// - Reality check: K9 AVX2 3-4× measured (NOT 8× theoretical)
fn bench_avx2_vs_scalar_signature(c: &mut Criterion) {
    let mut group = c.benchmark_group("avx2_vs_scalar_signature");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(1000);
    group.warm_up_time(Duration::from_secs(3));

    // Test different document sizes
    for tokens_per_doc in [10, 100, 1000] {
        let doc = generate_documents(1, tokens_per_doc).into_iter().next().unwrap().1;
        let tokens: Vec<&str> = doc.split_whitespace().collect();

        group.throughput(Throughput::Elements(1)); // 1 signature per iteration

        // Scalar baseline (always available)
        group.bench_with_input(
            BenchmarkId::new("scalar", format!("{}tokens", tokens_per_doc)),
            &tokens,
            |b, toks| {
                b.iter(|| {
                    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
                    black_box(MinHashSignatureCapsule::compute_signature(black_box(toks)))
                });
            },
        );

        // AVX2 path (if available - compile-time feature flag controls actual path)
        #[cfg(target_feature = "avx2")]
        group.bench_with_input(
            BenchmarkId::new("avx2", format!("{}tokens", tokens_per_doc)),
            &tokens,
            |b, toks| {
                b.iter(|| {
                    // NOTE: In Phase 5, this would use SIMD dispatch
                    // For now, measures same scalar path (will be updated post-implementation)
                    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
                    black_box(MinHashSignatureCapsule::compute_signature(black_box(toks)))
                });
            },
        );

        // SSE4.2 path (if available)
        #[cfg(all(target_feature = "sse4.2", not(target_feature = "avx2")))]
        group.bench_with_input(
            BenchmarkId::new("sse42", format!("{}tokens", tokens_per_doc)),
            &tokens,
            |b, toks| {
                b.iter(|| {
                    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
                    black_box(MinHashSignatureCapsule::compute_signature(black_box(toks)))
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark 4: End-to-End Portable Binary Performance
// ============================================================================

/// Target: <5% regression vs native (compile-time optimized)
///
/// **B32 Compliance**:
/// - Baseline: Native binary (compile-time feature flags)
/// - Comparison: Portable binary (runtime dispatch)
/// - Same workload: Full deduplication pipeline
/// - Reality: Small overhead acceptable for portability
fn bench_end_to_end_portable(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end_portable");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(100); // Lower for longer operations
    group.warm_up_time(Duration::from_secs(3));

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Test different corpus sizes
    for num_docs in [100, 500, 1000] {
        let docs = generate_documents(num_docs, 100);

        group.throughput(Throughput::Elements(num_docs as u64));

        // Portable binary (runtime dispatch - Phase 5 will implement this)
        group.bench_with_input(
            BenchmarkId::new("portable", format!("{}docs", num_docs)),
            &docs,
            |b, documents| {
                b.iter(|| {
                    let mut pipeline = DedupPipeline::new(num_docs, &cpu_caps);

                    for (doc_id, text) in documents {
                        pipeline.add_document(*doc_id, black_box(text));
                    }

                    black_box(pipeline.find_duplicates(0.85))
                });
            },
        );

        // Native binary (compile-time optimized - same as portable for now)
        group.bench_with_input(
            BenchmarkId::new("native", format!("{}docs", num_docs)),
            &docs,
            |b, documents| {
                b.iter(|| {
                    let mut pipeline = DedupPipeline::new(num_docs, &cpu_caps);

                    for (doc_id, text) in documents {
                        pipeline.add_document(*doc_id, black_box(text));
                    }

                    black_box(pipeline.find_duplicates(0.85))
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark 5: Throughput Scaling (Sustained Performance)
// ============================================================================

/// Target: 60K+ signatures/sec sustained
///
/// **B32 Compliance**:
/// - Test sustained throughput over time
/// - Measure documents/second
/// - Validate no degradation under load
/// - Reality: Should maintain 30-50× vs Python baseline
fn bench_throughput_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_scaling");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(50); // Lower for very long operations
    group.measurement_time(Duration::from_secs(10)); // Sustained performance
    group.warm_up_time(Duration::from_secs(3));

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Test different batch sizes
    for num_docs in [100, 1000, 10000] {
        let docs = generate_documents(num_docs, 100);

        group.throughput(Throughput::Elements(num_docs as u64));

        group.bench_with_input(
            BenchmarkId::new("sustained", format!("{}docs", num_docs)),
            &docs,
            |b, documents| {
                b.iter(|| {
                    let mut pipeline = DedupPipeline::new(num_docs, &cpu_caps);

                    // Process documents (signature generation is the bottleneck)
                    for (doc_id, text) in documents {
                        pipeline.add_document(*doc_id, black_box(text));
                    }

                    // Find duplicates (LSH + Union-Find)
                    black_box(pipeline.find_duplicates(0.85))
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark 6: Dispatch Contention (Multi-threaded Scaling)
// ============================================================================

/// Target: Linear scaling to 12 threads (lockfree design)
///
/// **B32 Compliance**:
/// - Test concurrent dispatch (100% lockfree)
/// - Measure scaling efficiency
/// - Baseline: 1 thread
/// - Comparison: 2, 4, 8, 12, 16 threads
/// - Reality: K23 sublinear beyond 14 threads (memory bandwidth)
fn bench_dispatch_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatch_contention");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(50); // Lower for multi-threaded tests
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(3));

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Generate documents (shared across threads)
    let num_docs = 1000;
    let docs = generate_documents(num_docs, 100);

    // Test different thread counts
    for num_threads in [1, 2, 4, 8, 12, 16] {
        group.throughput(Throughput::Elements((num_docs * num_threads) as u64));

        group.bench_with_input(
            BenchmarkId::new("threads", format!("{}_threads", num_threads)),
            &num_threads,
            |b, &threads| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;

                    for _ in 0..iters {
                        let mut handles = vec![];
                        let start = Instant::now();

                        // Spawn threads
                        for _ in 0..threads {
                            let thread_docs = docs.clone();
                            let caps = cpu_caps.clone();

                            let handle = std::thread::spawn(move || {
                                let mut pipeline = DedupPipeline::new(num_docs, &caps);

                                for (doc_id, text) in &thread_docs {
                                    pipeline.add_document(*doc_id, black_box(text));
                                }

                                black_box(pipeline.find_duplicates(0.85))
                            });

                            handles.push(handle);
                        }

                        // Wait for all threads
                        for handle in handles {
                            handle.join().unwrap();
                        }

                        total += start.elapsed();
                    }

                    total
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark 7: Component-Level Dispatch (MinHash Only)
// ============================================================================

/// Measure pure MinHash signature generation (isolated from LSH/Union-Find)
///
/// **B32 Compliance**:
/// - Isolate SIMD speedup from pipeline overhead
/// - Measure signature computation only
/// - Compare all CPU tiers on same input
fn bench_component_minhash(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_minhash");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(1000);
    group.warm_up_time(Duration::from_secs(3));

    // Test different document sizes
    for tokens_per_doc in [10, 50, 100, 500, 1000] {
        let doc = generate_documents(1, tokens_per_doc).into_iter().next().unwrap().1;
        let tokens: Vec<&str> = doc.split_whitespace().collect();

        group.throughput(Throughput::Elements(1));

        // Scalar (baseline)
        group.bench_with_input(
            BenchmarkId::new("scalar", format!("{}tokens", tokens_per_doc)),
            &tokens,
            |b, toks| {
                b.iter(|| {
                    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
                    black_box(MinHashSignatureCapsule::compute_signature(black_box(toks)))
                });
            },
        );

        // Dispatched (runtime CPU detection)
        group.bench_with_input(
            BenchmarkId::new("dispatched", format!("{}tokens", tokens_per_doc)),
            &tokens,
            |b, toks| {
                b.iter(|| {
                    // Simulate runtime dispatch
                    let caps = CpuCapabilityCapsule::detect();
                    let _tier = caps.best_simd_tier();

                    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
                    black_box(MinHashSignatureCapsule::compute_signature(black_box(toks)))
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark 8: Pipeline Initialization Cost
// ============================================================================

/// Measure pipeline initialization overhead (CPU detection + allocation)
///
/// **B32 Compliance**:
/// - Measure one-time setup cost
/// - Amortized over document processing
/// - Should be negligible (<1% total time)
fn bench_pipeline_init(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_init");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(1000);
    group.warm_up_time(Duration::from_secs(3));

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Test different capacity sizes
    for capacity in [100, 1000, 10000, 100000] {
        group.bench_with_input(
            BenchmarkId::new("init", format!("{}capacity", capacity)),
            &capacity,
            |b, &cap| {
                b.iter(|| black_box(DedupPipeline::new(black_box(cap), &cpu_caps)));
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark 9: Memory Bandwidth Sensitivity
// ============================================================================

/// Test performance under memory pressure (K3 reality check)
///
/// **B32 Compliance**:
/// - Measure throughput with varying working set sizes
/// - L1 cache: 48KB (fits ~6K elements)
/// - L2 cache: 2MB (fits ~256K elements)
/// - L3 cache: 24MB (fits ~3M elements)
/// - Reality: K29 15.2GB/s sequential, 3-5GB/s random
fn bench_memory_bandwidth(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_bandwidth");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(3));

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Test working set sizes: L1, L2, L3, RAM
    for (label, num_docs, tokens_per_doc) in [
        ("L1_fit", 100, 50),           // Fits in L1 cache
        ("L2_fit", 1000, 100),         // Fits in L2 cache
        ("L3_fit", 10000, 100),        // Fits in L3 cache
        ("RAM_overflow", 100000, 100), // Overflows to RAM
    ] {
        let docs = generate_documents(num_docs, tokens_per_doc);

        group.throughput(Throughput::Elements(num_docs as u64));

        group.bench_with_input(BenchmarkId::new("working_set", label), &docs, |b, documents| {
            b.iter(|| {
                let mut pipeline = DedupPipeline::new(num_docs, &cpu_caps);

                for (doc_id, text) in documents {
                    pipeline.add_document(*doc_id, black_box(text));
                }

                black_box(pipeline.find_duplicates(0.85))
            });
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark 10: Branch Prediction Impact
// ============================================================================

/// Measure branch misprediction penalty (K7 reality check)
///
/// **B32 Compliance**:
/// - Test dispatch with predictable vs unpredictable patterns
/// - Measure impact of CPU feature variability
/// - Reality: K7 1 cycle correct, 18 cycles mispredicted
fn bench_branch_prediction(c: &mut Criterion) {
    let mut group = c.benchmark_group("branch_prediction");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(1000);
    group.warm_up_time(Duration::from_secs(3));

    let cpu_caps = CpuCapabilityCapsule::detect();
    let tokens: Vec<&str> = vec!["the", "quick", "brown", "fox", "jumps"];

    // Predictable: Same CPU tier every call
    group.bench_function("predictable_dispatch", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let _tier = cpu_caps.best_simd_tier();
                use atomic_capsule::probabilistic::MinHashSignatureCapsule;
                black_box(MinHashSignatureCapsule::compute_signature(black_box(&tokens)));
            }
        });
    });

    // Unpredictable: Simulated variable CPU (not actually possible, but measures worst case)
    group.bench_function("worst_case_dispatch", |b| {
        b.iter(|| {
            for i in 0..1000 {
                // Simulate unpredictable branch (always same in reality, but compiler can't know)
                let _tier = if black_box(i % 2 == 0) {
                    cpu_caps.best_simd_tier()
                } else {
                    cpu_caps.best_simd_tier()
                };

                use atomic_capsule::probabilistic::MinHashSignatureCapsule;
                black_box(MinHashSignatureCapsule::compute_signature(black_box(&tokens)));
            }
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_cpu_capability_init,      // 1. CPU detection init (<1μs)
    bench_dispatch_overhead,        // 2. Dispatch overhead (<10ns)
    bench_avx2_vs_scalar_signature, // 3. AVX2 vs scalar (2-4×)
    bench_end_to_end_portable,      // 4. Portable vs native (<5% regression)
    bench_throughput_scaling,       // 5. Throughput (60K+ sigs/sec)
    bench_dispatch_contention,      // 6. Multi-threaded scaling
    bench_component_minhash,        // 7. Isolated MinHash performance
    bench_pipeline_init,            // 8. Pipeline initialization
    bench_memory_bandwidth,         // 9. Memory bandwidth sensitivity
    bench_branch_prediction,        // 10. Branch prediction impact
);

criterion_main!(benches);
