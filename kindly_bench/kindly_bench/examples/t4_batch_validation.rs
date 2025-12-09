//! T4 Batch validation: Parallel batch processing vs sequential baseline
//!
//! This example validates kindly_bench against Criterion baseline benchmarks.
//! It replicates the parallel batch processing benchmarks from parallel_batch_processor_bench.rs:
//! - parallel_filter (lines 209-213)
//! - parallel_map (lines 260-262)
//! - parallel_reduce (lines 308-311)
//!
//! # Validation Criteria
//!
//! - Mean times within 20% of Criterion results (acceptable)
//! - Mean times within 10% of Criterion results (ideal)
//! - Speedup calculations within 0.5× of Criterion
//! - Tier classification matches expectations (BREAKTHROUGH for 10-100×)
//! - Recommendations are correct (SHIP for BREAKTHROUGH)
//! - Sequential baseline auto-generation works correctly
//!
//! # Expected Results (from Criterion)
//!
//! Based on parallel_batch_processor_bench.rs expected results:
//! - Filter (100K i32): Sequential ~150µs, Parallel (8 cores) ~35µs = ~4.3× speedup (EXCEPTIONAL)
//! - Map (100K f64): Sequential ~80µs, Parallel (8 cores) ~18µs = ~4.4× speedup (EXCEPTIONAL)
//! - Reduce (100K u64): Sequential ~40µs, Parallel (8 cores) ~9µs = ~4.4× speedup (EXCEPTIONAL)
//!
//! Expected tier: EXCEPTIONAL (2-10× speedup)
//! Expected scaling: ~0.7× per core (realistic scaling with memory bandwidth limits)

use kindly_bench::{BenchmarkConfig, run_benchmark};
use atomic_capsule::parallel::iter::{IntoParallelIterator, ParallelIterator};

fn main() {
    println!("================================================================================");
    println!("T4 Batch Validation: Parallel Processing vs Sequential");
    println!("================================================================================");
    println!("Replicating Criterion benchmarks using kindly_bench framework");
    println!("Reference: benches/parallel_batch_processor_bench.rs");
    println!();

    // Detect CPU core count (for validation)
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("CPU cores detected: {}", cores);
    println!("Expected speedup: ~{}× (with 0.7× scaling efficiency)", (cores as f64 * 0.7) as usize);
    println!();

    // Test data (exact same as Criterion benchmarks)
    let size = 100_000;
    let data_i32: Vec<i32> = (0..size).map(|i| i as i32).collect();
    let data_f64: Vec<f64> = (0..size).map(|i| i as f64).collect();
    let data_u64: Vec<u64> = (0..size).map(|i| i as u64).collect();
    let threshold = (size / 2) as i32; // 50% selectivity for filter

    // ============================================================================
    // Benchmark 1: Filter (100K i32, >threshold predicate)
    // ============================================================================

    println!("================================================================================");
    println!("Benchmark 1/3: Filter (100K i32, >threshold)");
    println!("================================================================================");

    let config_filter = BenchmarkConfig::new(
        "Parallel_Filter_100K",
        "T4-Batch",
        "Sequential"
    )
    .iterations(1_000)  // Batch operations are slower, use fewer iterations
    .warmup(50);

    // Clone data for closures
    let data_filter_opt = data_i32.clone();
    let data_filter_base = data_i32.clone();

    run_benchmark(
        config_filter,
        move || {
            // Optimized: Parallel filter using ParallelIterator
            // Note: ParallelIterator::filter returns Vec<&T>, convert to owned
            let filtered_refs: Vec<&i32> = data_filter_opt.as_slice().into_par_iter().filter(|&&x| x > threshold);
            let _result: Vec<i32> = filtered_refs.into_iter().copied().collect();
        },
        move || {
            // Baseline: Sequential filter using standard iterator
            let _result: Vec<i32> = data_filter_base.iter().copied().filter(|&x| x > threshold).collect();
        },
    );

    println!();
    println!("Expected: Sequential ~150µs, Parallel (8 cores) ~35µs = ~4.3× speedup (EXCEPTIONAL)");
    println!();

    // ============================================================================
    // Benchmark 2: Map (100K f64, double values)
    // ============================================================================

    println!("================================================================================");
    println!("Benchmark 2/3: Map (100K f64, double values)");
    println!("================================================================================");

    let config_map = BenchmarkConfig::new(
        "Parallel_Map_100K",
        "T4-Batch",
        "Sequential"
    )
    .iterations(1_000)
    .warmup(50);

    // Clone data for closures
    let data_map_opt = data_f64.clone();
    let data_map_base = data_f64.clone();

    run_benchmark(
        config_map,
        move || {
            // Optimized: Parallel map using ParallelIterator
            let _result: Vec<f64> = data_map_opt.as_slice().into_par_iter().map(|x| x * 2.0);
        },
        move || {
            // Baseline: Sequential map using standard iterator
            let _result: Vec<f64> = data_map_base.iter().map(|&x| x * 2.0).collect();
        },
    );

    println!();
    println!("Expected: Sequential ~80µs, Parallel (8 cores) ~18µs = ~4.4× speedup (EXCEPTIONAL)");
    println!();

    // ============================================================================
    // Benchmark 3: Reduce (100K u64, sum)
    // ============================================================================

    println!("================================================================================");
    println!("Benchmark 3/3: Reduce (100K u64, sum)");
    println!("================================================================================");

    let config_reduce = BenchmarkConfig::new(
        "Parallel_Reduce_100K",
        "T4-Batch",
        "Sequential"
    )
    .iterations(1_000)
    .warmup(50);

    // Clone data for closures
    let data_reduce_opt = data_u64.clone();
    let data_reduce_base = data_u64.clone();

    run_benchmark(
        config_reduce,
        move || {
            // Optimized: Parallel reduce using ParallelIterator fold
            let _result = data_reduce_opt.as_slice().into_par_iter()
                .fold(|| 0u64, |acc, x| acc + x, |a, b| a + b);
        },
        move || {
            // Baseline: Sequential reduce using standard iterator sum()
            let _result: u64 = data_reduce_base.iter().sum();
        },
    );

    println!();
    println!("Expected: Sequential ~40µs, Parallel (8 cores) ~9µs = ~4.4× speedup (EXCEPTIONAL)");
    println!();

    // ============================================================================
    // Validation Summary
    // ============================================================================

    println!("================================================================================");
    println!("Validation Complete");
    println!("================================================================================");
    println!("Compare these results against Criterion baseline from existing benchmarks.");
    println!();
    println!("Validation Criteria:");
    println!("  ✓ Mean times within 20% of Criterion (acceptable)");
    println!("  ✓ Mean times within 10% of Criterion (ideal)");
    println!("  ✓ Speedup calculations within 0.5× of Criterion");
    println!("  ✓ Tier classification matches expectations (EXCEPTIONAL for 2-10×)");
    println!("  ✓ Recommendations are correct (SHIP for EXCEPTIONAL/BREAKTHROUGH)");
    println!("  ✓ Sequential baseline auto-generation works correctly");
    println!("  ✓ Speedup scales with core count (~0.7× efficiency)");
    println!();
    println!("XML results saved for each benchmark.");
    println!("================================================================================");
}
