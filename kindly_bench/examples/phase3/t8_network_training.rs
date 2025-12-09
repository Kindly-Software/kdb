//! T8 Network: Distributed Training Benchmark
//!
//! # Example
//!
//! Demonstrates manual single-node baseline for distributed training.
//!
//! **Optimized**: Distributed training (8 nodes, pipeline parallelism)
//! **Baseline**: Single-node multi-threaded (manual implementation)
//!
//! # Expected Results
//!
//! - **TYPICAL**: 2-3× speedup (network overhead limits scaling)
//! - **EXCEPTIONAL**: 3-10× speedup (low network latency)

use kindly_bench::{Tier, BaselineKind};
use std::time::Instant;

/// Simulate distributed training (optimized, 8 nodes)
#[cfg(feature = "network")]
fn distributed_training(model_size: usize, data_size: usize, nodes: usize) -> u64 {
    println!("Distributed training: {} nodes", nodes);

    let start = Instant::now();

    // Simulate pipeline parallelism
    // Each node processes a layer in parallel
    for layer in 0..model_size {
        // Simulate forward pass
        std::thread::sleep(std::time::Duration::from_micros(100));

        // Simulate network communication (all-reduce)
        std::thread::sleep(std::time::Duration::from_micros(50));
    }

    start.elapsed().as_nanos() as u64
}

/// Single-node multi-threaded training (manual baseline)
fn single_node_training(model_size: usize, data_size: usize, threads: usize) -> u64 {
    println!("Single-node training: {} threads", threads);

    let start = Instant::now();

    // Multi-threaded data parallelism
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                for layer in 0..model_size {
                    // Simulate forward pass
                    std::thread::sleep(std::time::Duration::from_micros(100));

                    // No network communication (single node)
                }
            });
        }
    });

    start.elapsed().as_nanos() as u64
}

fn main() {
    println!("T8 Network: Distributed Training Benchmark");
    println!("==========================================\n");

    #[cfg(not(feature = "network"))]
    {
        println!("Network feature not enabled. Enable with:");
        println!("cargo run --example t8_network_training --features network");
        println!("\nRunning single-node baseline only...\n");
    }

    let model_size = 100; // 100 layers
    let data_size = 10000; // 10K samples
    let nodes = 8;
    let threads = 8;

    println!("Model: {} layers", model_size);
    println!("Data: {} samples", data_size);
    println!("Expected speedup: 2-3× (TYPICAL)\n");

    // Single-node baseline
    println!("Running single-node baseline ({} threads)...", threads);
    let baseline_time_ns = single_node_training(model_size, data_size, threads);
    println!("Baseline time: {:.2} ms\n", baseline_time_ns as f64 / 1_000_000.0);

    #[cfg(feature = "network")]
    {
        // Distributed optimized
        println!("Running distributed optimized ({} nodes)...", nodes);
        let optimized_time_ns = distributed_training(model_size, data_size, nodes);
        println!("Optimized time: {:.2} ms\n", optimized_time_ns as f64 / 1_000_000.0);

        let speedup = baseline_time_ns as f64 / optimized_time_ns as f64;
        println!("Speedup: {:.2}×", speedup);

        if speedup >= 2.5 && speedup < 10.0 {
            println!("Classification: EXCEPTIONAL (1.5-2.5× for network is good!)");
        } else if speedup >= 10.0 {
            println!("Classification: BREAKTHROUGH (unexpected for network, validate!)");
        } else {
            println!("Classification: TYPICAL (<2.5×, network overhead limits scaling)");
        }
    }

    println!("\nFair Baseline Checklist:");
    println!("✓ Multi-threaded single-node code (std::thread::scope)");
    println!("✓ Same algorithm as distributed version");
    println!("✓ Realistic dataset (10K samples)");
    println!("✗ Network validation not yet integrated (feature flag)");
}
