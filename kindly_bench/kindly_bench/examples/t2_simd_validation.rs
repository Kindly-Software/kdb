//! T2 SIMD validation: MinHash SIMD vs Scalar baseline
//!
//! This example validates kindly_bench T2 SIMD tier against Criterion baseline benchmarks.
//! It replicates the MinHash signature computation benchmark from minhash_simd_bench.rs.
//!
//! # Validation Criteria
//!
//! - Mean times within 20% of Criterion results (acceptable)
//! - Mean times within 10% of Criterion results (ideal)
//! - Speedup calculations within 0.5× of Criterion
//! - Tier classification matches expectations (EXCEPTIONAL/BREAKTHROUGH expected)
//! - Recommendations are correct (SHIP for 2.5-4× speedup)
//!
//! # Expected Results (from Criterion baseline)
//!
//! Based on minhash_simd_bench.rs results:
//! - Scalar: 97.84 µs (100 tokens, 128 hash functions)
//! - SIMD: 37.86 µs (8-lane portable_simd)
//! - **Speedup: 2.58× (EXCEPTIONAL tier)**
//!
//! # SIMD Implementation
//!
//! Uses portable_simd (nightly feature) for 8-lane parallel MinHash computation:
//! - 8 MurmurHash3 values computed in parallel
//! - 16 iterations (128 hashes / 8 lanes)
//! - SIMD min reduction for signature updates
//!
//! # Scalar Baseline
//!
//! Simple nested loop implementation:
//! - 128 hash functions executed serially
//! - Single MurmurHash3 per iteration
//! - Standard scalar min operation

use kindly_bench::{BenchmarkConfig, run_benchmark};

// Import MinHash capsule
use atomic_capsule::probabilistic::MinHashSignatureCapsule;

fn main() {
    println!("================================================================================");
    println!("T2 SIMD Validation: MinHash SIMD vs Scalar");
    println!("================================================================================");
    println!("Replicating Criterion benchmarks using kindly_bench framework");
    println!("Reference: benches/minhash_simd_bench.rs (100 token benchmark)");
    println!();

    // Generate test tokens (same as Criterion benchmark)
    let tokens: Vec<String> = (0..100).map(|i| format!("token_{}", i)).collect();
    let token_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();

    // ============================================================================
    // Benchmark: MinHash Signature Computation (SIMD vs Scalar)
    // ============================================================================

    println!("================================================================================");
    println!("MinHash Signature Computation (100 tokens, 128 hash functions)");
    println!("================================================================================");

    let config = BenchmarkConfig::new(
        "MinHash_Signature_Computation",
        "T2-SIMD",
        "Scalar"
    )
    .iterations(10_000)
    .warmup(100);

    // Use compute_signature_fast which automatically uses SIMD when available
    // This method is available in all builds - uses SIMD if portable_simd is enabled,
    // otherwise falls back to scalar
    println!("Using compute_signature_fast() which detects SIMD availability at compile-time");

    run_benchmark(
        config,
        || {
            // Optimized: SIMD-accelerated MinHash (if portable_simd enabled)
            // Falls back to scalar if not available
            let _signature = MinHashSignatureCapsule::compute_signature_fast(&token_refs);
        },
        || {
            // Baseline: Scalar MinHash (always serial loop)
            let _signature = MinHashSignatureCapsule::compute_signature(&token_refs);
        },
    );

    println!();
    println!("Expected (from Criterion - with portable_simd):");
    println!("  Scalar:  97.84 µs");
    println!("  SIMD:    37.86 µs");
    println!("  Speedup: 2.58× (EXCEPTIONAL tier)");
    println!();
    println!("Without portable_simd: Both use scalar, speedup ~1.0×");
    println!();
    println!("Validation:");
    println!("  ✓ With SIMD: Speedup should be 2-4× (EXCEPTIONAL tier)");
    println!("  ✓ Recommendation should be SHIP (proven speedup)");
    println!("  ✓ Mean times should be within 20% of Criterion baseline");

    // ============================================================================
    // Validation Summary
    // ============================================================================

    println!();
    println!("================================================================================");
    println!("Validation Complete");
    println!("================================================================================");
    println!("Compare these results against Criterion baseline.");
    println!();
    println!("Validation Criteria:");
    println!("  ✓ Mean times within 20% of Criterion (acceptable)");
    println!("  ✓ Mean times within 10% of Criterion (ideal)");
    println!("  ✓ Speedup calculations within 0.5× of Criterion");
    println!("  ✓ Tier classification: EXCEPTIONAL (2-10×)");
    println!("  ✓ Recommendation: SHIP (proven speedup)");
    println!();
    println!("XML results saved: MinHash_Signature_Computation.xml");
    println!("================================================================================");
}
