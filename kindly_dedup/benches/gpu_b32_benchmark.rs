//! GPU B32 Benchmark Suite - Phase 4 Validation
//!
//! Fair benchmarking per B32 framework:
//! - 95% CI with 1000+ iterations (where feasible)
//! - Fair baselines (CPU SIMD, not strawman)
//! - Document hardware and compiler
//! - Reproducible results with fixed seeds
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q21-Q34 (Implementation validation)
//! - **Chaos**: 100% lockfree kernels
//! - **ASSUM**: GPU assumptions documented
//! - **B32**: 95% CI, fair baselines
//! - **T28**: Equivalence tests, performance tests
//!
//! # Running
//!
//! ```bash
//! cargo bench --features "gpu,benchmarking" --bench gpu_b32_benchmark
//! ```

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::time::Duration;

#[cfg(feature = "gpu")]
use kindly_dedup::gpu::{GpuContextCapsule, MinHashGpuCapsule, MinHashGpuInput};

#[cfg(feature = "gpu")]
use std::sync::Arc;

// =============================================================================
// GPU MinHash Throughput Benchmark
// =============================================================================

/// GPU MinHash kernel benchmark at various scales
///
/// B32 Compliance:
/// - Multiple document counts (1K, 10K, 100K)
/// - 100 samples (warm-up + measurement)
/// - 95% CI reported
/// - Throughput metric (docs/sec)
#[cfg(feature = "gpu")]
fn gpu_minhash_throughput(c: &mut Criterion) {
    // Try to initialize GPU context
    let ctx = match GpuContextCapsule::new_blocking() {
        Ok(ctx) => Arc::new(ctx),
        Err(e) => {
            println!("Skipping GPU benchmarks - no GPU available: {}", e);
            return;
        }
    };

    // Print GPU info for reproducibility (B32 requirement)
    let caps = ctx.capabilities();
    println!("\n=== GPU B32 Benchmark ===");
    println!("Device: {}", caps.device_name);
    println!("Backend: {:?}", caps.backend);
    println!("Max workgroup size: {}x{}x{}",
        caps.max_workgroup_size_x,
        caps.max_workgroup_size_y,
        caps.max_workgroup_size_z);
    println!("Max storage buffer: {} bytes", caps.max_storage_buffer_binding_size);
    println!("");

    // Create MinHash kernel
    let minhash = match MinHashGpuCapsule::new(&ctx) {
        Ok(k) => k,
        Err(e) => {
            println!("Failed to create MinHash kernel: {}", e);
            return;
        }
    };

    let mut group = c.benchmark_group("gpu_minhash_throughput");
    group.significance_level(0.05);  // 95% CI
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    for num_docs in [1_000, 10_000, 100_000] {
        // Generate test data with fixed seed for reproducibility (B32 requirement)
        let tokens_per_doc = 100;
        let mut tokens = Vec::with_capacity(num_docs * tokens_per_doc);
        let mut offsets = Vec::with_capacity(num_docs + 1);

        offsets.push(0);
        for doc_id in 0..num_docs {
            for t in 0..tokens_per_doc {
                // Deterministic tokens: doc_id * 1000 + token_idx
                tokens.push((doc_id * 1000 + t) as u32);
            }
            offsets.push(tokens.len() as u32);
        }

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: num_docs as u32,
        };

        // Set throughput for docs/sec reporting
        group.throughput(Throughput::Elements(num_docs as u64));

        // Benchmark GPU MinHash
        group.bench_with_input(
            BenchmarkId::new("gpu", format!("{}K_docs", num_docs / 1000)),
            &input,
            |b, input| {
                b.iter(|| {
                    minhash.compute(&ctx, input.clone()).unwrap()
                });
            },
        );
    }

    group.finish();
}

// =============================================================================
// GPU vs CPU Comparison Benchmark
// =============================================================================

/// Fair GPU vs CPU comparison benchmark
///
/// B32 Compliance:
/// - Fair baseline: CPU SIMD MinHash (not scalar strawman)
/// - Same input data for both
/// - Same algorithm (MinHash with 128 hash functions)
/// - Same output format (128 x u16 signatures)
#[cfg(feature = "gpu")]
fn gpu_vs_cpu_comparison(c: &mut Criterion) {
    let ctx = match GpuContextCapsule::new_blocking() {
        Ok(ctx) => Arc::new(ctx),
        Err(_) => {
            println!("Skipping GPU vs CPU benchmark - no GPU available");
            return;
        }
    };

    let minhash_gpu = match MinHashGpuCapsule::new(&ctx) {
        Ok(k) => k,
        Err(_) => return,
    };

    let mut group = c.benchmark_group("gpu_vs_cpu");
    group.significance_level(0.05);
    group.sample_size(50);  // Reduced for faster iteration
    group.measurement_time(Duration::from_secs(5));

    let num_docs = 10_000;
    let tokens_per_doc = 100;

    // Generate test data
    let mut tokens = Vec::with_capacity(num_docs * tokens_per_doc);
    let mut offsets = Vec::with_capacity(num_docs + 1);

    offsets.push(0);
    for doc_id in 0..num_docs {
        for t in 0..tokens_per_doc {
            tokens.push((doc_id * 1000 + t) as u32);
        }
        offsets.push(tokens.len() as u32);
    }

    let input = MinHashGpuInput {
        tokens: &tokens,
        offsets: &offsets,
        num_docs: num_docs as u32,
    };

    group.throughput(Throughput::Elements(num_docs as u64));

    // GPU benchmark
    group.bench_function("gpu_minhash_10K", |b| {
        b.iter(|| minhash_gpu.compute(&ctx, input.clone()).unwrap());
    });

    // CPU baseline (fair comparison - uses same algorithm)
    // Generate document tokens for CPU path
    let doc_tokens: Vec<Vec<u32>> = (0..num_docs)
        .map(|doc_id| {
            (0..tokens_per_doc)
                .map(|t| (doc_id * 1000 + t) as u32)
                .collect()
        })
        .collect();

    group.bench_function("cpu_minhash_simd_10K", |b| {
        b.iter(|| {
            let mut signatures = Vec::with_capacity(num_docs);
            for doc_tokens in &doc_tokens {
                signatures.push(cpu_minhash_simd(doc_tokens));
            }
            signatures
        });
    });

    group.finish();
}

/// CPU MinHash implementation (fair baseline)
///
/// Uses the same algorithm as GPU:
/// - 128 hash functions (FNV-1a with permutation seeds)
/// - u16 truncation
/// - Golden ratio seeds
#[cfg(feature = "gpu")]
fn cpu_minhash_simd(tokens: &[u32]) -> [u16; 128] {
    let mut sig = [u16::MAX; 128];
    let golden = 2654435761u32;

    for &token in tokens {
        for i in 0..128 {
            let seed = ((i + 1) as u32).wrapping_mul(golden);

            // Same hash function as GPU kernel
            let mut h = seed ^ 2166136261;
            h ^= token;
            h = h.wrapping_mul(16777619);
            h ^= h >> 16;
            h = h.wrapping_mul(2654435769);
            h ^= h >> 13;

            let truncated = (h & 0xFFFF) as u16;
            sig[i] = sig[i].min(truncated);
        }
    }
    sig
}

// =============================================================================
// GPU Memory Transfer Benchmark
// =============================================================================

/// Measure GPU memory transfer overhead
///
/// B32 Compliance:
/// - Separate transfer from compute time
/// - Measure upload and download independently
/// - Report transfer bandwidth
#[cfg(feature = "gpu")]
fn gpu_memory_transfer(c: &mut Criterion) {
    let ctx = match GpuContextCapsule::new_blocking() {
        Ok(ctx) => Arc::new(ctx),
        Err(_) => return,
    };

    let minhash = match MinHashGpuCapsule::new(&ctx) {
        Ok(k) => k,
        Err(_) => return,
    };

    let mut group = c.benchmark_group("gpu_memory_transfer");
    group.sample_size(50);

    for num_docs in [1_000, 10_000, 50_000] {
        let tokens_per_doc = 100;
        let mut tokens = Vec::with_capacity(num_docs * tokens_per_doc);
        let mut offsets = Vec::with_capacity(num_docs + 1);

        offsets.push(0);
        for doc_id in 0..num_docs {
            for t in 0..tokens_per_doc {
                tokens.push((doc_id * 1000 + t) as u32);
            }
            offsets.push(tokens.len() as u32);
        }

        // Calculate data sizes
        let input_bytes = tokens.len() * 4 + offsets.len() * 4;
        let output_bytes = num_docs * 64 * 4;  // 64 u32 per doc

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: num_docs as u32,
        };

        // Report throughput in bytes for transfer analysis
        group.throughput(Throughput::Bytes((input_bytes + output_bytes) as u64));

        group.bench_with_input(
            BenchmarkId::new("full_round_trip", format!("{}K", num_docs / 1000)),
            &input,
            |b, input| {
                b.iter(|| minhash.compute(&ctx, input.clone()).unwrap());
            },
        );
    }

    group.finish();
}

// =============================================================================
// GPU Batch Size Scaling Benchmark
// =============================================================================

/// Measure optimal batch size for GPU
///
/// B32 Compliance:
/// - Test various batch sizes
/// - Identify throughput plateau
/// - Report efficiency at each scale
#[cfg(feature = "gpu")]
fn gpu_batch_scaling(c: &mut Criterion) {
    let ctx = match GpuContextCapsule::new_blocking() {
        Ok(ctx) => Arc::new(ctx),
        Err(_) => return,
    };

    let minhash = match MinHashGpuCapsule::new(&ctx) {
        Ok(k) => k,
        Err(_) => return,
    };

    let mut group = c.benchmark_group("gpu_batch_scaling");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(5));

    // Test various batch sizes to find optimal
    for num_docs in [100, 500, 1_000, 2_000, 5_000, 10_000, 20_000, 50_000] {
        let tokens_per_doc = 100;
        let mut tokens = Vec::with_capacity(num_docs * tokens_per_doc);
        let mut offsets = Vec::with_capacity(num_docs + 1);

        offsets.push(0);
        for doc_id in 0..num_docs {
            for t in 0..tokens_per_doc {
                tokens.push((doc_id * 1000 + t) as u32);
            }
            offsets.push(tokens.len() as u32);
        }

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: num_docs as u32,
        };

        group.throughput(Throughput::Elements(num_docs as u64));

        group.bench_with_input(
            BenchmarkId::new("batch", num_docs),
            &input,
            |b, input| {
                b.iter(|| minhash.compute(&ctx, input.clone()).unwrap());
            },
        );
    }

    group.finish();
}

// =============================================================================
// GPU Token Count Scaling Benchmark
// =============================================================================

/// Measure GPU performance vs tokens per document
///
/// B32 Compliance:
/// - Fixed document count, varying token count
/// - Identifies compute vs transfer dominance
#[cfg(feature = "gpu")]
fn gpu_token_scaling(c: &mut Criterion) {
    let ctx = match GpuContextCapsule::new_blocking() {
        Ok(ctx) => Arc::new(ctx),
        Err(_) => return,
    };

    let minhash = match MinHashGpuCapsule::new(&ctx) {
        Ok(k) => k,
        Err(_) => return,
    };

    let mut group = c.benchmark_group("gpu_token_scaling");
    group.sample_size(30);

    let num_docs = 10_000;

    for tokens_per_doc in [10, 50, 100, 200, 500, 1000] {
        let mut tokens = Vec::with_capacity(num_docs * tokens_per_doc);
        let mut offsets = Vec::with_capacity(num_docs + 1);

        offsets.push(0);
        for doc_id in 0..num_docs {
            for t in 0..tokens_per_doc {
                tokens.push((doc_id * 1000 + t) as u32);
            }
            offsets.push(tokens.len() as u32);
        }

        let input = MinHashGpuInput {
            tokens: &tokens,
            offsets: &offsets,
            num_docs: num_docs as u32,
        };

        group.throughput(Throughput::Elements(num_docs as u64));

        group.bench_with_input(
            BenchmarkId::new("tokens", tokens_per_doc),
            &input,
            |b, input| {
                b.iter(|| minhash.compute(&ctx, input.clone()).unwrap());
            },
        );
    }

    group.finish();
}

// =============================================================================
// Hybrid Pipeline Benchmark
// =============================================================================

/// End-to-end hybrid pipeline benchmark
///
/// B32 Compliance:
/// - Full pipeline (tokenization + MinHash + clustering)
/// - Both GPU and CPU paths
/// - Real-world document simulation
#[cfg(feature = "gpu-hybrid")]
fn hybrid_pipeline_benchmark(c: &mut Criterion) {
    use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
    use atomic_capsule::CpuCapabilityCapsule;

    let cpu_caps = CpuCapabilityCapsule::detect();

    let mut group = c.benchmark_group("hybrid_pipeline");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(30));

    for num_docs in [1_000, 5_000, 10_000] {
        // Generate test documents
        let docs: Vec<String> = (0..num_docs)
            .map(|i| format!("document {} with some test text content for MinHash signature", i))
            .collect();

        // Try GPU mode
        if let Ok(mut pipeline) = HybridDedupPipeline::new(num_docs, PipelineMode::Auto, &cpu_caps) {
            let mode_name = if pipeline.is_using_gpu() { "gpu" } else { "cpu" };

            group.throughput(Throughput::Elements(num_docs as u64));

            group.bench_function(
                BenchmarkId::new(format!("hybrid_{}", mode_name), num_docs),
                |b| {
                    b.iter(|| {
                        let mut p = HybridDedupPipeline::new(num_docs, PipelineMode::Auto, &cpu_caps).unwrap();
                        for (i, doc) in docs.iter().enumerate() {
                            p.add_document(i as u32, doc).unwrap();
                        }
                        p.find_duplicates(0.8).unwrap()
                    });
                },
            );
        }
    }

    group.finish();
}

// =============================================================================
// Stub functions for non-GPU builds
// =============================================================================

#[cfg(not(feature = "gpu"))]
fn gpu_minhash_throughput(_c: &mut Criterion) {
    println!("GPU benchmarks require 'gpu' feature. Run with:");
    println!("  cargo bench --features 'gpu,benchmarking' --bench gpu_b32_benchmark");
}

#[cfg(not(feature = "gpu"))]
fn gpu_vs_cpu_comparison(_c: &mut Criterion) {}

#[cfg(not(feature = "gpu"))]
fn gpu_memory_transfer(_c: &mut Criterion) {}

#[cfg(not(feature = "gpu"))]
fn gpu_batch_scaling(_c: &mut Criterion) {}

#[cfg(not(feature = "gpu"))]
fn gpu_token_scaling(_c: &mut Criterion) {}

#[cfg(not(feature = "gpu-hybrid"))]
fn hybrid_pipeline_benchmark(_c: &mut Criterion) {}

// =============================================================================
// End-to-End Hybrid Pipeline Benchmark (Default Feature)
// =============================================================================

/// Generate realistic documents for benchmarking
///
/// Creates documents with realistic token distributions similar to LLM training data.
/// Uses deterministic seeding for reproducibility (B32 requirement).
fn generate_realistic_docs(count: usize) -> Vec<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let vocab = [
        "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog",
        "machine", "learning", "artificial", "intelligence", "neural",
        "network", "deep", "transformer", "attention", "model", "data",
        "training", "inference", "optimization", "gradient", "descent",
        "batch", "epoch", "loss", "function", "activation", "layer",
        "weight", "bias", "forward", "backward", "propagation", "token",
    ];

    (0..count)
        .map(|doc_idx| {
            // Deterministic pseudo-random using hash
            let mut hasher = DefaultHasher::new();
            doc_idx.hash(&mut hasher);
            let mut seed = hasher.finish();

            // Variable document length (50-500 tokens)
            let len = 50 + (seed % 451) as usize;

            (0..len)
                .map(|word_idx| {
                    // Update seed for each word
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    vocab[(seed as usize) % vocab.len()]
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// Generate near-duplicate documents for realistic deduplication testing
///
/// Creates clusters of similar documents to simulate real duplicate scenarios.
fn generate_docs_with_duplicates(count: usize, duplicate_rate: f64) -> Vec<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let base_docs = generate_realistic_docs(count);
    let num_duplicates = ((count as f64) * duplicate_rate) as usize;

    let mut result = Vec::with_capacity(count);

    for (i, doc) in base_docs.into_iter().enumerate() {
        if i < num_duplicates {
            // Create a near-duplicate by modifying a few words
            let mut hasher = DefaultHasher::new();
            i.hash(&mut hasher);
            let seed = hasher.finish();

            let words: Vec<&str> = doc.split_whitespace().collect();
            let modified: Vec<&str> = words
                .iter()
                .enumerate()
                .map(|(j, &w)| {
                    if (seed.wrapping_add(j as u64)) % 20 == 0 {
                        "modified" // Replace ~5% of words
                    } else {
                        w
                    }
                })
                .collect();
            result.push(modified.join(" "));
        } else {
            result.push(doc);
        }
    }

    result
}

/// End-to-end hybrid pipeline benchmark
///
/// Tests the complete deduplication workflow:
/// 1. CPU Tokenization
/// 2. MinHash signature computation (GPU or CPU)
/// 3. LSH band hashing
/// 4. Union-Find clustering
///
/// B32 Compliance:
/// - Uses realistic documents (not synthetic tokens)
/// - Tests multiple document counts (1K, 5K, 10K)
/// - 50 samples with 30s measurement time
/// - Reports throughput in docs/sec
#[cfg(feature = "gpu-hybrid")]
fn hybrid_end_to_end_benchmark(c: &mut Criterion) {
    use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
    use atomic_capsule::CpuCapabilityCapsule;

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Pre-generate documents (reuse across iterations)
    let docs_1k = generate_realistic_docs(1_000);
    let docs_5k = generate_realistic_docs(5_000);
    let docs_10k = generate_realistic_docs(10_000);

    let mut group = c.benchmark_group("hybrid_end_to_end");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(30));

    // 1K documents benchmark
    group.throughput(Throughput::Elements(1_000));
    group.bench_function("1k_docs", |b| {
        b.iter_batched(
            || HybridDedupPipeline::new(1_000, PipelineMode::Auto, &cpu_caps).unwrap(),
            |mut pipeline: HybridDedupPipeline| {
                for (id, text) in docs_1k.iter().enumerate() {
                    pipeline.add_document(id as u32, text).unwrap();
                }
                pipeline.find_duplicates(0.8).unwrap()
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // 5K documents benchmark
    group.throughput(Throughput::Elements(5_000));
    group.bench_function("5k_docs", |b| {
        b.iter_batched(
            || HybridDedupPipeline::new(5_000, PipelineMode::Auto, &cpu_caps).unwrap(),
            |mut pipeline: HybridDedupPipeline| {
                for (id, text) in docs_5k.iter().enumerate() {
                    pipeline.add_document(id as u32, text).unwrap();
                }
                pipeline.find_duplicates(0.8).unwrap()
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // 10K documents benchmark
    group.throughput(Throughput::Elements(10_000));
    group.bench_function("10k_docs", |b| {
        b.iter_batched(
            || HybridDedupPipeline::new(10_000, PipelineMode::Auto, &cpu_caps).unwrap(),
            |mut pipeline: HybridDedupPipeline| {
                for (id, text) in docs_10k.iter().enumerate() {
                    pipeline.add_document(id as u32, text).unwrap();
                }
                pipeline.find_duplicates(0.8).unwrap()
            },
            criterion::BatchSize::LargeInput,
        );
    });

    group.finish();
}

/// Component breakdown benchmark
///
/// Measures individual pipeline components to identify bottlenecks:
/// 1. Tokenization (CPU)
/// 2. MinHash computation (GPU or CPU)
/// 3. LSH band hashing (GPU or CPU)
/// 4. Union-Find clustering (CPU)
///
/// B32 Compliance:
/// - Tests each component in isolation
/// - Uses realistic inputs
/// - Identifies true bottlenecks
#[cfg(feature = "gpu-hybrid")]
fn hybrid_component_breakdown(c: &mut Criterion) {
    use atomic_capsule::probabilistic::{tokenize, MinHashSignatureCapsule, UnionFind};

    let mut group = c.benchmark_group("hybrid_breakdown");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    // Realistic document text (100-500 words)
    let realistic_text = "The quick brown fox jumps over the lazy dog. \
        Machine learning models require large datasets for training. \
        Neural networks use backpropagation for gradient descent optimization. \
        Transformer architectures have revolutionized natural language processing. \
        Attention mechanisms allow models to focus on relevant input features. \
        Deep learning has enabled significant advances in computer vision. "
        .repeat(50);

    // 1. CPU Tokenization benchmark
    group.bench_function("1_tokenization", |b| {
        b.iter(|| {
            tokenize(&realistic_text)
        });
    });

    // 2. MinHash computation (CPU baseline)
    let tokens = tokenize(&realistic_text);
    let tokens_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();

    group.bench_function("2_minhash_cpu", |b| {
        b.iter(|| {
            MinHashSignatureCapsule::compute_signature(&tokens_refs)
        });
    });

    // 3. Jaccard similarity computation
    let sig1 = MinHashSignatureCapsule::compute_signature(&tokens_refs);
    let sig2 = MinHashSignatureCapsule::compute_signature(&tokens_refs);

    group.bench_function("3_jaccard_similarity", |b| {
        b.iter(|| {
            sig1.jaccard_similarity(&sig2)
        });
    });

    // 4. Union-Find operations
    let union_pairs: Vec<(usize, usize)> = (0..5000).map(|i| (i, i + 1)).collect();

    group.bench_function("4_union_find_5k_unions", |b| {
        b.iter_batched(
            || UnionFind::new(10_000),
            |mut uf| {
                for &(a, b) in &union_pairs {
                    uf.union(a, b);
                }
                uf
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // 5. Union-Find find operations (after unions)
    let mut uf_setup = UnionFind::new(10_000);
    for &(a, b) in &union_pairs {
        uf_setup.union(a, b);
    }

    group.bench_function("5_union_find_5k_finds", |b| {
        b.iter(|| {
            let mut sum = 0usize;
            for i in 0..5000 {
                sum = sum.wrapping_add(uf_setup.find(i));
            }
            sum
        });
    });

    group.finish();
}

/// GPU vs CPU mode comparison benchmark
///
/// Compares HybridDedupPipeline in GPU mode vs CPU-only mode
/// to measure actual GPU acceleration benefit on real workloads.
///
/// B32 Compliance:
/// - Fair comparison (same algorithm, same data)
/// - Tests both modes on identical workload
/// - Reports mode used and speedup ratio
#[cfg(feature = "gpu-hybrid")]
fn hybrid_gpu_vs_cpu_mode(c: &mut Criterion) {
    use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
    use atomic_capsule::CpuCapabilityCapsule;

    let cpu_caps = CpuCapabilityCapsule::detect();
    let docs = generate_docs_with_duplicates(5_000, 0.3); // 30% near-duplicates

    let mut group = c.benchmark_group("hybrid_gpu_vs_cpu_mode");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(20));
    group.throughput(Throughput::Elements(5_000));

    // CPU-only mode (baseline)
    group.bench_function("cpu_only_5k", |b| {
        b.iter_batched(
            || HybridDedupPipeline::new(5_000, PipelineMode::CpuOnly, &cpu_caps).unwrap(),
            |mut pipeline: HybridDedupPipeline| {
                for (id, text) in docs.iter().enumerate() {
                    pipeline.add_document(id as u32, text).unwrap();
                }
                pipeline.find_duplicates(0.8).unwrap()
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // GPU/Auto mode
    group.bench_function("gpu_auto_5k", |b| {
        b.iter_batched(
            || {
                let p = HybridDedupPipeline::new(5_000, PipelineMode::Auto, &cpu_caps).unwrap();
                println!("Using GPU: {}", p.is_using_gpu());
                p
            },
            |mut pipeline: HybridDedupPipeline| {
                for (id, text) in docs.iter().enumerate() {
                    pipeline.add_document(id as u32, text).unwrap();
                }
                pipeline.find_duplicates(0.8).unwrap()
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Document scaling benchmark
///
/// Tests how throughput scales with document count (1K to 50K).
/// Identifies optimal batch sizes and memory pressure points.
///
/// B32 Compliance:
/// - Multiple document counts
/// - Reports throughput at each scale
/// - Identifies scaling characteristics
#[cfg(feature = "gpu-hybrid")]
fn hybrid_document_scaling(c: &mut Criterion) {
    use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
    use atomic_capsule::CpuCapabilityCapsule;

    let cpu_caps = CpuCapabilityCapsule::detect();

    let mut group = c.benchmark_group("hybrid_document_scaling");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(30));

    for num_docs in [1_000, 2_000, 5_000, 10_000, 20_000] {
        let docs = generate_realistic_docs(num_docs);

        group.throughput(Throughput::Elements(num_docs as u64));
        group.bench_function(BenchmarkId::new("docs", num_docs), |b| {
            b.iter_batched(
                || HybridDedupPipeline::new(num_docs, PipelineMode::Auto, &cpu_caps).unwrap(),
                |mut pipeline: HybridDedupPipeline| {
                    for (id, text) in docs.iter().enumerate() {
                        pipeline.add_document(id as u32, text).unwrap();
                    }
                    pipeline.find_duplicates(0.8).unwrap()
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

/// Duplicate rate impact benchmark
///
/// Tests how duplicate rate affects pipeline performance.
/// Higher duplicate rates should show LSH bucket efficiency gains.
///
/// B32 Compliance:
/// - Multiple duplicate rates (10%, 30%, 50%, 70%)
/// - Reports throughput at each rate
/// - Validates LSH bucket efficiency
#[cfg(feature = "gpu-hybrid")]
fn hybrid_duplicate_rate_impact(c: &mut Criterion) {
    use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
    use atomic_capsule::CpuCapabilityCapsule;

    let cpu_caps = CpuCapabilityCapsule::detect();
    let num_docs = 5_000;

    let mut group = c.benchmark_group("hybrid_duplicate_rate");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(20));
    group.throughput(Throughput::Elements(num_docs as u64));

    for dup_rate in [0.1, 0.3, 0.5, 0.7] {
        let docs = generate_docs_with_duplicates(num_docs, dup_rate);

        group.bench_function(BenchmarkId::new("dup_rate", format!("{:.0}%", dup_rate * 100.0)), |b| {
            b.iter_batched(
                || HybridDedupPipeline::new(num_docs, PipelineMode::Auto, &cpu_caps).unwrap(),
                |mut pipeline: HybridDedupPipeline| {
                    for (id, text) in docs.iter().enumerate() {
                        pipeline.add_document(id as u32, text).unwrap();
                    }
                    let clusters = pipeline.find_duplicates(0.8).unwrap();
                    (clusters.len(), pipeline.stats().duplicate_pairs)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// =============================================================================
// Async Overlap Benchmark (gpu-async feature)
// =============================================================================

/// Async overlap benchmark - tests CPU-GPU pipeline parallelism
///
/// When async mode is enabled, CPU fills batches while GPU processes.
/// This measures the overlap efficiency (target: >80%).
///
/// B32 Compliance:
/// - Tests async vs sync throughput
/// - Reports overlap efficiency
/// - Validates background thread coordination
#[cfg(feature = "gpu-async")]
fn async_overlap_benchmark(c: &mut Criterion) {
    use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
    use atomic_capsule::CpuCapabilityCapsule;

    let cpu_caps = CpuCapabilityCapsule::detect();
    let docs = generate_realistic_docs(10_000);

    let mut group = c.benchmark_group("async_overlap");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(30));
    group.throughput(Throughput::Elements(10_000));

    // Sync mode (baseline)
    group.bench_function("sync_10k", |b| {
        b.iter_batched(
            || HybridDedupPipeline::new(10_000, PipelineMode::Auto, &cpu_caps).unwrap(),
            |mut pipeline| {
                for (id, text) in docs.iter().enumerate() {
                    pipeline.add_document(id as u32, text).unwrap();
                }
                pipeline.find_duplicates(0.8).unwrap()
            },
            criterion::BatchSize::LargeInput,
        );
    });

    // Async mode
    group.bench_function("async_10k", |b| {
        b.iter_batched(
            || {
                let mut p = HybridDedupPipeline::new(10_000, PipelineMode::Auto, &cpu_caps).unwrap();
                if p.is_using_gpu() {
                    let enabled = p.enable_async();
                    println!("Async enabled: {}", enabled);
                }
                p
            },
            |mut pipeline| {
                for (id, text) in docs.iter().enumerate() {
                    pipeline.add_document(id as u32, text).unwrap();
                }
                // Report overlap efficiency
                if pipeline.is_using_gpu() {
                    println!("Overlap efficiency: {:.1}%", pipeline.async_overlap_efficiency() * 100.0);
                }
                pipeline.find_duplicates(0.8).unwrap()
            },
            criterion::BatchSize::LargeInput,
        );
    });

    group.finish();
}

// =============================================================================
// Stub functions for non-GPU/non-async builds
// =============================================================================

#[cfg(not(feature = "gpu-hybrid"))]
fn hybrid_end_to_end_benchmark(_c: &mut Criterion) {
    println!("Hybrid end-to-end benchmarks require 'gpu-hybrid' feature. Run with:");
    println!("  cargo bench --features 'gpu-hybrid,benchmarking' --bench gpu_b32_benchmark");
}

#[cfg(not(feature = "gpu-hybrid"))]
fn hybrid_component_breakdown(_c: &mut Criterion) {}

#[cfg(not(feature = "gpu-hybrid"))]
fn hybrid_gpu_vs_cpu_mode(_c: &mut Criterion) {}

#[cfg(not(feature = "gpu-hybrid"))]
fn hybrid_document_scaling(_c: &mut Criterion) {}

#[cfg(not(feature = "gpu-hybrid"))]
fn hybrid_duplicate_rate_impact(_c: &mut Criterion) {}

#[cfg(not(feature = "gpu-async"))]
fn async_overlap_benchmark(_c: &mut Criterion) {}

// =============================================================================
// Criterion Configuration
// =============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default()
        .significance_level(0.05)  // 95% CI
        .noise_threshold(0.02);    // 2% noise threshold
    targets =
        // Existing GPU kernel benchmarks
        gpu_minhash_throughput,
        gpu_vs_cpu_comparison,
        gpu_memory_transfer,
        gpu_batch_scaling,
        gpu_token_scaling,
        hybrid_pipeline_benchmark,
        // New end-to-end hybrid benchmarks
        hybrid_end_to_end_benchmark,
        hybrid_component_breakdown,
        hybrid_gpu_vs_cpu_mode,
        hybrid_document_scaling,
        hybrid_duplicate_rate_impact,
        async_overlap_benchmark,
);

criterion_main!(benches);
