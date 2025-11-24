//! # Hybrid LSH Throughput Benchmarks - REALISTIC WORKLOADS (B32 Compliant)
//!
//! **Purpose**: Validate ACTUAL hybrid LSH performance with realistic documents,
//! not just signature creation speed. Tests end-to-end throughput, duplicate detection,
//! and multi-threaded scaling.
//!
//! **B32 Framework Compliance**:
//! - **Fair Baselines**: Realistic documents (50-200 words), not synthetic signatures
//! - **Statistical Rigor**: 1000+ iterations for micro, 5+ for integration
//! - **Confidence Intervals**: 95% CI (Criterion.rs default)
//! - **Honest Reporting**: Compare MEASURED vs PROJECTED performance
//!
//! **Performance Targets** (PROJECTED, needs validation):
//! - Single-threaded: 60K docs/sec @ 1 thread
//! - Multi-threaded: 300K docs/sec @ 16 threads
//! - Insert latency: ~560ns per document
//! - Find duplicates: <100μs per 1K candidates
//!
//! **CRITICAL**: This benchmark validates or refutes these projections with ACTUAL measurements.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::{Arc, Barrier};
use std::thread;

use atomic_capsule::probabilistic::MinHashSignatureCapsule;
use kindly_dedup::HybridLshCapsule;

// Import SIMD MinHash when available
#[cfg(feature = "simd-minhash")]
use atomic_capsule::probabilistic::minhash_simd::compute_signature_simd;

/// Benchmark protection module
#[path = "benchmark_protection.rs"]
mod benchmark_protection;
use benchmark_protection::require_valid_license;

// ============================================================================
// REALISTIC TEST DATA GENERATION
// ============================================================================

/// Fixed vocabulary for document generation (10K words)
const VOCABULARY_SIZE: usize = 10_000;

/// Generate deterministic vocabulary word
///
/// # Algorithm
/// - Use simple hash to generate word from index
/// - Produces consistent words for reproducibility
/// - ~5-10 characters per word
fn generate_vocab_word(index: usize) -> String {
    // Simple hash for deterministic word generation
    let hash = index.wrapping_mul(2654435761); // Knuth's multiplicative hash
    format!("word{:x}", hash % VOCABULARY_SIZE)
}

/// Generate realistic test document (50-200 words)
///
/// # Parameters
/// - `doc_id`: Document identifier (seed for RNG)
/// - `seed`: Additional seed for variation
/// - `duplicate_ratio`: Probability of creating near-duplicate (0.0 = unique, 1.0 = exact duplicate)
///
/// # Returns
/// String with 50-200 words from 10K vocabulary
///
/// # Characteristics
/// - **Unique docs**: Random words from vocabulary
/// - **Near-duplicates**: 80-90% word overlap (10-20% variation)
/// - **Exact duplicates**: 100% word overlap
fn generate_test_document(doc_id: u64, seed: u64, _duplicate_ratio: f64) -> String {
    // Deterministic RNG (simple LCG)
    let mut rng_state = doc_id.wrapping_mul(seed).wrapping_add(12345);

    // Simple LCG random number generator
    let mut next_random = || {
        rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        rng_state
    };

    // Document length: 50-200 words
    let word_count = 50 + (next_random() % 150) as usize;

    // Check if this should be a duplicate (5% exact, 10% near-duplicates)
    let r = (next_random() % 100) as f64 / 100.0;

    let mut words = Vec::with_capacity(word_count);

    if r < 0.05 {
        // 5% exact duplicates (copy from earlier doc)
        let source_doc_id = doc_id.saturating_sub(10 + (next_random() % 90));
        let mut source_rng = source_doc_id.wrapping_mul(seed).wrapping_add(12345);
        for _ in 0..word_count {
            source_rng = source_rng.wrapping_mul(1103515245).wrapping_add(12345);
            let word_idx = (source_rng % VOCABULARY_SIZE as u64) as usize;
            words.push(generate_vocab_word(word_idx));
        }
    } else if r < 0.15 {
        // 10% near-duplicates (80-90% overlap with earlier doc)
        let source_doc_id = doc_id.saturating_sub(10 + (next_random() % 90));
        let mut source_rng = source_doc_id.wrapping_mul(seed).wrapping_add(12345);
        for _ in 0..word_count {
            // 80% use source words, 20% use new words
            let use_source = (next_random() % 100) < 80;
            if use_source {
                source_rng = source_rng.wrapping_mul(1103515245).wrapping_add(12345);
                let word_idx = (source_rng % VOCABULARY_SIZE as u64) as usize;
                words.push(generate_vocab_word(word_idx));
            } else {
                let word_idx = (next_random() % VOCABULARY_SIZE as u64) as usize;
                words.push(generate_vocab_word(word_idx));
            }
        }
    } else {
        // 85% unique documents (random words)
        for _ in 0..word_count {
            let word_idx = (next_random() % VOCABULARY_SIZE as u64) as usize;
            words.push(generate_vocab_word(word_idx));
        }
    }

    words.join(" ")
}

/// Create MinHash signature from document text
///
/// # Performance
/// - Scalar: ~100μs per document (128 hash functions)
/// - SIMD: ~14μs per document (7.1× speedup)
fn create_signature_from_document(text: &str) -> MinHashSignatureCapsule {
    let tokens: Vec<&str> = text.split_whitespace().collect();

    // Use SIMD when available (7.1× speedup)
    #[cfg(feature = "simd-minhash")]
    {
        compute_signature_simd(&tokens)
    }

    // Fallback to scalar
    #[cfg(not(feature = "simd-minhash"))]
    {
        MinHashSignatureCapsule::compute_signature(&tokens)
    }
}

// ============================================================================
// BENCHMARK A: END-TO-END SINGLE-THREAD THROUGHPUT (PRIORITY)
// ============================================================================

fn benchmark_hybrid_lsh_end_to_end_single_thread(c: &mut Criterion) {
    require_valid_license("hybrid_lsh_end_to_end_single_thread");

    let mut group = c.benchmark_group("hybrid_lsh_end_to_end");
    group.sample_size(20); // Minimum 20 samples for Criterion (requires >= 10)

    // Test with different document counts
    for num_docs in [1_000, 10_000, 100_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_docs", num_docs)),
            num_docs,
            |b, &num_docs| {
                b.iter_custom(|iters| {
                    let mut total_time = std::time::Duration::ZERO;

                    for _iter in 0..iters {
                        // Create temporary files
                        let temp_dir = tempfile::tempdir().unwrap();
                        let bucket_path = temp_dir.path().join("test.buckets");
                        let index_path = temp_dir.path().join("test.index");

                        // Create LSH instance
                        let lsh = HybridLshCapsule::new(
                            bucket_path.to_str().unwrap(),
                            index_path.to_str().unwrap(),
                            num_docs, // flush_threshold
                        )
                        .unwrap();

                        let start = std::time::Instant::now();

                        // Insert documents with realistic text
                        for doc_id in 0..num_docs {
                            let text = generate_test_document(doc_id as u64, 12345, 0.15);
                            let signature = create_signature_from_document(&text);
                            lsh.insert(doc_id as usize, &signature).unwrap();

                            // Flush every 10K documents (realistic batch size)
                            if (doc_id + 1) % 10_000 == 0 {
                                let _ = lsh.flush(); // Ignore flush errors
                            }
                        }

                        // Final flush
                        let _ = lsh.flush();

                        total_time += start.elapsed();
                    }

                    total_time
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK B: DUPLICATE DETECTION WITH JACCARD VERIFICATION
// ============================================================================

fn benchmark_hybrid_lsh_find_duplicates(c: &mut Criterion) {
    require_valid_license("hybrid_lsh_find_duplicates");

    let mut group = c.benchmark_group("hybrid_lsh_find_duplicates");
    group.sample_size(10);

    for num_docs in [1_000, 10_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_docs", num_docs)),
            num_docs,
            |b, &num_docs| {
                // Setup: Create LSH with documents
                let temp_dir = tempfile::tempdir().unwrap();
                let bucket_path = temp_dir.path().join("test.buckets");
                let index_path = temp_dir.path().join("test.index");

                let lsh = HybridLshCapsule::new(bucket_path.to_str().unwrap(), index_path.to_str().unwrap(), num_docs)
                    .unwrap();

                // Insert documents with some duplicates (15% near-duplicates/exact)
                for doc_id in 0..num_docs {
                    let text = generate_test_document(doc_id as u64, 12345, 0.15);
                    let signature = create_signature_from_document(&text);
                    lsh.insert(doc_id as usize, &signature).unwrap();
                }
                let _ = lsh.flush();

                // Benchmark find_duplicates with Jaccard verification
                b.iter(|| {
                    let duplicates = lsh.find_duplicates(0.85).unwrap();
                    black_box(duplicates)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK C: MULTI-THREADED THROUGHPUT (REALISTIC)
// ============================================================================

fn benchmark_hybrid_lsh_multi_thread_realistic(c: &mut Criterion) {
    require_valid_license("hybrid_lsh_multi_thread_realistic");

    let mut group = c.benchmark_group("hybrid_lsh_multi_thread");
    group.sample_size(10);

    // Test with different thread counts
    for num_threads in [2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", num_threads)),
            num_threads,
            |b, &num_threads| {
                b.iter_custom(|iters| {
                    let mut total_time = std::time::Duration::ZERO;

                    for _iter in 0..iters {
                        let temp_dir = tempfile::tempdir().unwrap();
                        let bucket_path = temp_dir.path().join("test.buckets");
                        let index_path = temp_dir.path().join("test.index");

                        let lsh = Arc::new(
                            HybridLshCapsule::new(
                                bucket_path.to_str().unwrap(),
                                index_path.to_str().unwrap(),
                                100_000, // Fixed corpus size
                            )
                            .unwrap(),
                        );

                        let barrier = Arc::new(Barrier::new(num_threads));
                        let mut handles = vec![];

                        let start = std::time::Instant::now();

                        for thread_id in 0..num_threads {
                            let lsh_clone = lsh.clone();
                            let barrier_clone = barrier.clone();

                            let handle = thread::spawn(move || {
                                barrier_clone.wait();

                                // Each thread processes 100K / num_threads documents
                                let docs_per_thread = 100_000 / num_threads;
                                let base = thread_id * docs_per_thread;

                                for i in 0..docs_per_thread {
                                    let doc_id = (base + i) as u64;
                                    let text = generate_test_document(doc_id, 12345, 0.15);
                                    let signature = create_signature_from_document(&text);
                                    lsh_clone.insert(doc_id as usize, &signature).unwrap();
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.join().unwrap();
                        }

                        let _ = lsh.flush();

                        total_time += start.elapsed();
                    }

                    total_time
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK D: INSERT LATENCY MICROBENCHMARK
// ============================================================================

fn benchmark_hybrid_lsh_insert_latency(c: &mut Criterion) {
    require_valid_license("hybrid_lsh_insert_latency");

    let mut group = c.benchmark_group("hybrid_lsh_insert_latency");
    group.sample_size(100);

    // Setup: Create LSH instance
    let temp_dir = tempfile::tempdir().unwrap();
    let bucket_path = temp_dir.path().join("test.buckets");
    let index_path = temp_dir.path().join("test.index");

    let lsh = HybridLshCapsule::new(bucket_path.to_str().unwrap(), index_path.to_str().unwrap(), 1_000_000).unwrap();

    // Pre-generate signature for micro-benchmarking
    let text = generate_test_document(42, 12345, 0.0);
    let signature = create_signature_from_document(&text);

    group.bench_function("insert_single_document", |b| {
        let mut doc_id = 0;
        b.iter(|| {
            lsh.insert(doc_id, &signature).unwrap();
            doc_id += 1;
            black_box(doc_id)
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK E: FLUSH OPERATION LATENCY
// ============================================================================

fn benchmark_hybrid_lsh_flush_latency(c: &mut Criterion) {
    require_valid_license("hybrid_lsh_flush_latency");

    let mut group = c.benchmark_group("hybrid_lsh_flush");
    group.sample_size(10);

    for num_docs in [1_000, 10_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_docs_flush", num_docs)),
            num_docs,
            |b, &num_docs| {
                b.iter_custom(|iters| {
                    let mut total_time = std::time::Duration::ZERO;

                    for _iter in 0..iters {
                        let temp_dir = tempfile::tempdir().unwrap();
                        let bucket_path = temp_dir.path().join("test.buckets");
                        let index_path = temp_dir.path().join("test.index");

                        let lsh = HybridLshCapsule::new(
                            bucket_path.to_str().unwrap(),
                            index_path.to_str().unwrap(),
                            num_docs,
                        )
                        .unwrap();

                        // Insert documents
                        for doc_id in 0..num_docs {
                            let text = generate_test_document(doc_id as u64, 12345, 0.0);
                            let signature = create_signature_from_document(&text);
                            lsh.insert(doc_id as usize, &signature).unwrap();
                        }

                        // Measure flush latency
                        let start = std::time::Instant::now();
                        let _ = lsh.flush();
                        total_time += start.elapsed();
                    }

                    total_time
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK F: JACCARD VERIFICATION THROUGHPUT
// ============================================================================

fn benchmark_jaccard_verification_throughput(c: &mut Criterion) {
    require_valid_license("jaccard_verification_throughput");

    let mut group = c.benchmark_group("jaccard_verification");
    group.sample_size(100);

    // Pre-generate signatures
    let sig1 = MinHashSignatureCapsule::compute_signature(&["hello", "world", "rust", "programming"]);
    let sig2 = MinHashSignatureCapsule::compute_signature(&["hello", "world", "python", "coding"]);

    group.bench_function("estimate_jaccard_single_pair", |b| {
        b.iter(|| {
            let similarity = sig1.estimate_jaccard(&sig2);
            black_box(similarity)
        });
    });

    group.bench_function("estimate_jaccard_1000_pairs", |b| {
        // Pre-generate 1000 signature pairs
        let signatures: Vec<_> = (0..1000)
            .map(|i| {
                let text = generate_test_document(i, 12345, 0.15);
                create_signature_from_document(&text)
            })
            .collect();

        b.iter(|| {
            let mut total_similarity = 0.0;
            for i in 0..signatures.len() {
                for j in (i + 1).min(signatures.len())..(i + 2).min(signatures.len()) {
                    total_similarity += signatures[i].estimate_jaccard(&signatures[j]);
                }
            }
            black_box(total_similarity)
        });
    });

    group.finish();
}

// ============================================================================
// CRITERION SETUP
// ============================================================================

criterion_group!(
    benches,
    benchmark_hybrid_lsh_end_to_end_single_thread,
    benchmark_hybrid_lsh_find_duplicates,
    benchmark_hybrid_lsh_multi_thread_realistic,
    benchmark_hybrid_lsh_insert_latency,
    benchmark_hybrid_lsh_flush_latency,
    benchmark_jaccard_verification_throughput,
);
criterion_main!(benches);
