//! T4 Batch validation: Parallel batch processing vs sequential baseline (FIXED)
//!
//! This example validates kindly_bench against Criterion baseline benchmarks.
//! It replicates the parallel batch processing benchmarks from parallel_batch_processor_bench.rs.
//!
//! CRITICAL FIX: The previous version had 100× slower timings because it was measuring
//! closure creation overhead + data cloning instead of just the parallel operation.
//!
//! This version creates the data ONCE outside the benchmark closures, then only measures
//! the actual parallel/sequential operations.
//!
//! # Expected Results (from Criterion)
//!
//! Based on parallel_batch_processor_bench.rs expected results:
//! - Filter (100K i32): Sequential ~150µs, Parallel (8 cores) ~35µs = ~4.3× speedup (EXCEPTIONAL)
//! - Map (100K f64): Sequential ~80µs, Parallel (8 cores) ~18µs = ~4.4× speedup (EXCEPTIONAL)
//! - Reduce (100K u64): Sequential ~40µs, Parallel (8 cores) ~9µs = ~4.4× speedup (EXCEPTIONAL)

use kindly_bench::{BenchmarkConfig, run_benchmark};
use atomic_capsule::parallel::iter::{IntoParallelIterator, ParallelIterator};
use std::sync::Arc;

fn main() {
    println!("================================================================================");
    println!("T4 Batch Validation: Parallel Processing vs Sequential (FIXED)");
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

    // Test data (created ONCE, shared via Arc for zero-copy)
    let size = 100_000;
    let data_i32 = Arc::new((0..size).map(|i| i as i32).collect::<Vec<_>>());
    let data_f64 = Arc::new((0..size).map(|i| i as f64).collect::<Vec<_>>());
    let data_u64 = Arc::new((0..size).map(|i| i as u64).collect::<Vec<_>>());
    let threshold = (size / 2) as i32; // 50% selectivity for filter

    // ============================================================================
    // Benchmark 1: Filter (100K i32, >threshold predicate)
    // ============================================================================

    println!("================================================================================");
    println!("Benchmark 1/3: Filter (100K i32, >threshold)");
    println!("================================================================================");

    let config_filter = BenchmarkConfig::new(
        "Parallel_Filter_100K_Fixed",
        "T4-Batch",
        "Sequential"
    )
    .iterations(1_000)
    .warmup(50);

    // Clone Arc for each closure (Arc::clone is cheap, just bumps refcount)
    let data_filter_opt = Arc::clone(&data_i32);
    let data_filter_base = Arc::clone(&data_i32);

    run_benchmark(
        config_filter,
        move || {
            // Optimized: Parallel filter using ParallelIterator
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
        "Parallel_Map_100K_Fixed",
        "T4-Batch",
        "Sequential"
    )
    .iterations(1_000)
    .warmup(50);

    let data_map_opt = Arc::clone(&data_f64);
    let data_map_base = Arc::clone(&data_f64);

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
        "Parallel_Reduce_100K_Fixed",
        "T4-Batch",
        "Sequential"
    )
    .iterations(1_000)
    .warmup(50);

    let data_reduce_opt = Arc::clone(&data_u64);
    let data_reduce_base = Arc::clone(&data_u64);

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
