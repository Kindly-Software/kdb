//! Example demonstrating ScaleTestSuiteCapsule - Progressive Scale Testing
//!
//! This example shows how to configure and run the T4 Batch scale test suite
//! for progressive validation from 1M → 10M → 100M documents.

use kindly_dedup::testing::{ScaleTestConfig, ScaleTestSuiteCapsule};
use std::time::Duration;

fn main() {
    println!("=== ScaleTestSuiteCapsule Example ===\n");

    // Configure scale tests with progressive progression
    let config = ScaleTestConfig {
        scales: vec![100_000, 500_000, 1_000_000],  // Conservative 3-step progression
        timeout_per_scale: Duration::from_secs(300),  // 5 minutes per scale
        memory_limit_gb: 64.0,  // AMD 6900HX DDR5 capacity
        min_throughput: 40_000.0,  // Conservative (66.7% of 60K baseline)
        min_f1_score: 0.85,  // Minimum acceptable accuracy
    };

    println!("Configuration:");
    println!("  Scales: {:?}", config.scales);
    println!("  Timeout: {:?} per scale", config.timeout_per_scale);
    println!("  Memory limit: {:.1} GB", config.memory_limit_gb);
    println!("  Min throughput: {:.0} docs/sec", config.min_throughput);
    println!("  Min F1 score: {:.2}", config.min_f1_score);
    println!();

    // Create and run scale test suite
    let suite = ScaleTestSuiteCapsule::new(config);
    println!("Starting progressive scale testing...\n");

    let results = suite.run();

    // Print results
    println!("\n=== Test Summary ===");
    for result in &results {
        result.print_report();
    }

    // Summary statistics
    let total_scales = results.len();
    let passed_scales = results.iter().filter(|r| r.is_pass()).count();
    let failed_scales = total_scales - passed_scales;

    println!("\n=== Summary ===");
    println!("Total scales tested: {}", total_scales);
    println!("Passed: {}", passed_scales);
    println!("Failed: {}", failed_scales);

    if failed_scales > 0 {
        println!("\nFirst failure:");
        for result in &results {
            if !result.is_pass() {
                println!("  Scale: {} docs", result.scale);
                println!("  Reason: {}", result.status);
                println!("  Throughput: {:.0} docs/sec", result.throughput_docs_per_sec);
                println!("  Memory: {:.2} GB", result.peak_memory_gb);
                break;
            }
        }
    } else {
        println!("\nAll scales PASSED! Progressive validation successful.");
    }
}
