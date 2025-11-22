//! # ParallelBatchProcessor Demo
//!
//! Demonstrates the T4 Batch primitive for generic parallel processing.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example batch_processor_demo
//! ```

use atomic_capsule::parallel::{available_parallelism, ParallelBatchProcessor};

fn main() {
    println!("=== ParallelBatchProcessor Demo ===\n");

    // Detect available cores
    let cores = available_parallelism();
    println!("Available CPU cores: {}", cores);

    // Example 1: Filter numbers
    println!("\n--- Example 1: Filter Numbers ---");
    let processor = ParallelBatchProcessor::new();
    let numbers: Vec<i32> = (0..100_000).collect();

    let process_chunk =
        |chunk: &[i32]| -> Vec<i32> { chunk.iter().filter(|&&x| x > 50_000).copied().collect() };

    let start = std::time::Instant::now();
    let results = processor.process(&numbers, process_chunk);
    let duration = start.elapsed();

    println!("Filtered {} items in {:?}", results.len(), duration);
    println!("First 5 results: {:?}", &results[0..5.min(results.len())]);

    // Example 2: Transform strings
    println!("\n--- Example 2: Transform Strings ---");
    let processor = ParallelBatchProcessor::with_config(8, 10_000);
    let strings: Vec<String> = (0..50_000).map(|i| format!("item_{}", i)).collect();

    let process_chunk = |chunk: &[String]| -> Vec<String> {
        chunk
            .iter()
            .filter(|s| s.contains("_5"))
            .map(|s| s.to_uppercase())
            .collect()
    };

    let start = std::time::Instant::now();
    let results = processor.process(&strings, process_chunk);
    let duration = start.elapsed();

    println!("Transformed {} strings in {:?}", results.len(), duration);
    println!("First 5 results: {:?}", &results[0..5.min(results.len())]);

    // Example 3: Aggregate statistics
    println!("\n--- Example 3: Aggregate Statistics ---");
    let processor = ParallelBatchProcessor::new();
    let data: Vec<f64> = (0..100_000).map(|i| i as f64 * 1.5).collect();

    let process_chunk = |chunk: &[f64]| -> Vec<(f64, f64, f64)> {
        let sum: f64 = chunk.iter().sum();
        let min = chunk.iter().copied().fold(f64::INFINITY, f64::min);
        let max = chunk.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        vec![(sum, min, max)]
    };

    let start = std::time::Instant::now();
    let results = processor.process(&data, process_chunk);
    let duration = start.elapsed();

    // Combine results from all threads
    let (total_sum, global_min, global_max) = results.iter().fold(
        (0.0, f64::INFINITY, f64::NEG_INFINITY),
        |(sum, min, max), &(chunk_sum, chunk_min, chunk_max)| {
            (sum + chunk_sum, min.min(chunk_min), max.max(chunk_max))
        },
    );

    println!("Processed {} values in {:?}", data.len(), duration);
    println!(
        "Sum: {:.2}, Min: {:.2}, Max: {:.2}",
        total_sum, global_min, global_max
    );

    // Example 4: Crossover threshold demonstration
    println!("\n--- Example 4: Crossover Threshold ---");
    let processor = ParallelBatchProcessor::with_config(16, 10_000);

    // Small batch (below threshold → sequential)
    let small: Vec<i32> = (0..5_000).collect();
    let start = std::time::Instant::now();
    let results_small = processor.process(&small, |chunk| {
        chunk.iter().filter(|&&x| x > 1000).copied().collect()
    });
    let duration_small = start.elapsed();

    // Large batch (above threshold → parallel)
    let large: Vec<i32> = (0..100_000).collect();
    let start = std::time::Instant::now();
    let results_large = processor.process(&large, |chunk| {
        chunk.iter().filter(|&&x| x > 50_000).copied().collect()
    });
    let duration_large = start.elapsed();

    println!(
        "Small batch ({} items, sequential): {} results in {:?}",
        small.len(),
        results_small.len(),
        duration_small
    );
    println!(
        "Large batch ({} items, parallel): {} results in {:?}",
        large.len(),
        results_large.len(),
        duration_large
    );

    println!("\n=== Demo Complete ===");
}
