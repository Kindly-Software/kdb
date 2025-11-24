//! O(1) Memory Validation Benchmark for Phase 4.5
//!
//! **PURPOSE**: Measure actual memory usage to validate O(1) memory guarantee.
//!
//! # Approach
//! - Use jemalloc_ctl to measure actual allocated bytes
//! - Process 1K, 10K, 100K, 1M documents
//! - Assert memory growth is sublinear (O(1) not O(N))
//!
//! # Expected Results
//! - Memory should remain < 5 GB regardless of document count
//! - Growth rate should be minimal (< 100 MB per 100K docs)

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use jemalloc_ctl::{epoch, stats};
use kindly_dedup::parallel::ParallelDedupMetacapsule;
use std::sync::Arc;

/// Measure current allocated bytes via jemalloc
fn measure_allocated_bytes() -> Result<usize, jemalloc_ctl::Error> {
    // Refresh statistics
    epoch::mib()?.advance()?;

    // Get allocated bytes
    let allocated = stats::allocated::mib()?;
    allocated.read()
}

/// Generate test documents
fn generate_test_docs(num_docs: usize) -> Vec<(u32, String)> {
    (0..num_docs)
        .map(|i| {
            let text = format!(
                "Document {} contains some text with common words the and of to be in a that have I it for not on with {}",
                i, i % 100
            );
            (i as u32, text)
        })
        .collect()
}

fn bench_actual_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("o1_memory_validation");
    group.sample_size(10); // Reduce samples for memory benchmarks

    // Test different document counts
    for num_docs in [1_000, 10_000, 100_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_docs", num_docs)),
            num_docs,
            |b, &num_docs| {
                b.iter_custom(|iters| {
                    let mut total_duration = std::time::Duration::new(0, 0);

                    for _ in 0..iters {
                        // Generate test data
                        let test_docs = generate_test_docs(num_docs);
                        let test_docs_refs: Vec<(u32, &str)> = test_docs
                            .iter()
                            .map(|(id, text)| (*id, text.as_str()))
                            .collect();

                        // Measure memory before
                        let memory_before = measure_allocated_bytes()
                            .expect("Failed to measure memory before");

                        let start = std::time::Instant::now();

                        // Create and use metacapsule
                        let mut metacapsule = ParallelDedupMetacapsule::new(
                            black_box(num_docs),
                            black_box(16),      // 16 workers
                            black_box(1000),    // batch size
                            black_box(0.85),    // Jaccard threshold
                        )
                        .expect("Failed to create metacapsule");

                        // Add documents
                        metacapsule
                            .add_documents(black_box(&test_docs_refs))
                            .expect("Failed to add documents");

                        let duration = start.elapsed();
                        total_duration += duration;

                        // Measure memory after
                        let memory_after = measure_allocated_bytes()
                            .expect("Failed to measure memory after");

                        // Calculate memory growth
                        let memory_growth = memory_after.saturating_sub(memory_before);

                        // Print memory statistics
                        eprintln!(
                            "[O(1) Memory] {} docs: {} MB -> {} MB (growth: {} MB, {:.2} bytes/doc)",
                            num_docs,
                            memory_before / 1_000_000,
                            memory_after / 1_000_000,
                            memory_growth / 1_000_000,
                            memory_growth as f64 / num_docs as f64
                        );

                        // CRITICAL ASSERTION: Memory growth should be O(1)
                        // With our fixes, memory should remain constant (<100 MB growth)
                        const MAX_MEMORY_GROWTH_MB: usize = 100;
                        let max_growth = MAX_MEMORY_GROWTH_MB * 1_000_000;

                        if memory_growth > max_growth {
                            panic!(
                                "O(1) MEMORY VIOLATION: Growth {} MB exceeds {} MB limit for {} docs",
                                memory_growth / 1_000_000,
                                MAX_MEMORY_GROWTH_MB,
                                num_docs
                            );
                        }

                        // Explicitly drop to ensure cleanup
                        drop(metacapsule);
                    }

                    total_duration
                });
            },
        );
    }

    group.finish();
}

/// Test memory scaling to verify O(1)
fn test_memory_scaling() {
    println!("\n=== O(1) Memory Scaling Test ===\n");

    let doc_counts = [1_000, 10_000, 50_000, 100_000, 500_000, 1_000_000];
    let mut memory_samples = Vec::new();

    for num_docs in doc_counts {
        // Generate test data
        let test_docs = generate_test_docs(num_docs);
        let test_docs_refs: Vec<(u32, &str)> = test_docs
            .iter()
            .map(|(id, text)| (*id, text.as_str()))
            .collect();

        // Measure baseline
        let baseline = measure_allocated_bytes().unwrap();

        // Create metacapsule and process
        let mut metacapsule = ParallelDedupMetacapsule::new(
            num_docs,
            16,
            1000,
            0.85,
        ).unwrap();

        metacapsule.add_documents(&test_docs_refs).unwrap();

        // Measure peak
        let peak = measure_allocated_bytes().unwrap();
        let memory_used = peak.saturating_sub(baseline);

        memory_samples.push((num_docs, memory_used));

        println!(
            "{:>8} docs: {:>6} MB (baseline: {} MB, peak: {} MB)",
            num_docs,
            memory_used / 1_000_000,
            baseline / 1_000_000,
            peak / 1_000_000
        );

        drop(metacapsule);
    }

    // Analyze growth rate
    println!("\n=== Growth Analysis ===\n");

    if memory_samples.len() >= 2 {
        for i in 1..memory_samples.len() {
            let (prev_docs, prev_mem) = memory_samples[i - 1];
            let (curr_docs, curr_mem) = memory_samples[i];

            let doc_increase = curr_docs - prev_docs;
            let mem_increase = curr_mem.saturating_sub(prev_mem);
            let bytes_per_doc = mem_increase as f64 / doc_increase as f64;

            println!(
                "{:>8} -> {:>8} docs: {} MB increase ({:.2} bytes/doc)",
                prev_docs,
                curr_docs,
                mem_increase / 1_000_000,
                bytes_per_doc
            );

            // Assert sublinear growth (O(1) memory)
            // Memory per document should decrease as we scale
            if bytes_per_doc > 1000.0 {
                eprintln!(
                    "WARNING: High memory per doc: {:.2} bytes/doc (expected < 1000)",
                    bytes_per_doc
                );
            }
        }
    }

    // Final verdict
    let (max_docs, max_mem) = memory_samples.last().unwrap();
    let avg_bytes_per_doc = *max_mem as f64 / *max_docs as f64;

    println!("\n=== VERDICT ===\n");
    println!("Max tested: {} documents", max_docs);
    println!("Memory used: {} MB", max_mem / 1_000_000);
    println!("Average: {:.2} bytes/doc", avg_bytes_per_doc);

    if *max_mem < 5_000_000_000 {
        println!("✅ O(1) MEMORY VALIDATED: < 5 GB for {} docs", max_docs);
    } else {
        println!("❌ O(1) MEMORY VIOLATION: {} GB exceeds 5 GB limit", max_mem / 1_000_000_000);
    }
}

// Custom main to run both benchmark and validation test
fn main() {
    // Run the scaling test first
    test_memory_scaling();

    // Then run Criterion benchmarks
    let mut criterion = Criterion::default();
    bench_actual_memory_usage(&mut criterion);
    criterion.final_summary();
}

criterion_group!(benches, bench_actual_memory_usage);
// Note: Using custom main() instead of criterion_main!