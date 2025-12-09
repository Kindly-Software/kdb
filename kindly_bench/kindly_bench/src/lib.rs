//! kindly_bench - B32-compliant benchmark framework for computational capsule primitives
//!
//! # Overview
//!
//! `kindly_bench` is a specialized benchmarking framework designed for computational capsule
//! primitives across all 11 tiers (T0-T11). It provides:
//!
//! - **Automatic baseline generation** per tier (T1→RwLock, T2→Scalar, T3→F64, etc.)
//! - **B32 compliance enforcement** (95% CI, 1000+ iterations, 27 hardware checks)
//! - **Self-documenting output** (XML + terminal with recommendations)
//! - **Tier classification** (TYPICAL/EXCEPTIONAL/BREAKTHROUGH/SUSPICIOUS)
//!
//! # Phase 1 MVP
//!
//! This is Phase 1 (MVP) supporting T1-T3:
//! - T1 Atomic: Lockfree capsules vs RwLock/Mutex baselines
//! - T2 SIMD: Vectorized operations vs scalar baselines
//! - T3 Fixed-Point: Deterministic arithmetic vs f64 baselines
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use kindly_bench::{BenchmarkConfig, run_benchmark};
//! use std::sync::RwLock;
//!
//! // Define your optimized implementation
//! fn optimized_increment() {
//!     use std::sync::atomic::{AtomicU64, Ordering};
//!     let counter = AtomicU64::new(0);
//!     for _ in 0..1000 {
//!         counter.fetch_add(1, Ordering::Release);
//!     }
//! }
//!
//! // Define fair baseline (RwLock)
//! fn baseline_increment() {
//!     let counter = RwLock::new(0u64);
//!     for _ in 0..1000 {
//!         *counter.write().unwrap() += 1;
//!     }
//! }
//!
//! // Run benchmark
//! let config = BenchmarkConfig {
//!     name: "AtomicCounter vs RwLock".to_string(),
//!     tier: "T1-Atomic".to_string(),
//!     baseline_kind: "RwLock".to_string(),
//!     iterations: 10_000,
//!     warmup: 100,
//! };
//!
//! run_benchmark(config, optimized_increment, baseline_increment);
//! ```

pub mod baseline;
pub mod classification;
pub mod output;
pub mod stats;
pub mod timing;
pub mod validation;

pub use classification::{Classification, PerformanceTier, ConfidenceLevel, RecommendationAction};
pub use stats::{Statistics, Speedup};
pub use timing::{Timer, TimerKind};
pub use validation::HardwareInfo;

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// Benchmark name
    pub name: String,
    /// Capsule tier (T0-T11)
    pub tier: String,
    /// Baseline kind (RwLock, Scalar, F64, etc.)
    pub baseline_kind: String,
    /// Number of iterations (minimum 1000 for B32 compliance)
    pub iterations: usize,
    /// Number of warmup iterations
    pub warmup: usize,
}

impl BenchmarkConfig {
    /// Create a new benchmark configuration
    pub fn new(name: impl Into<String>, tier: impl Into<String>, baseline_kind: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tier: tier.into(),
            baseline_kind: baseline_kind.into(),
            iterations: 10_000, // Default: 10K iterations
            warmup: 100,         // Default: 100 warmup rounds
        }
    }

    /// Set number of iterations (minimum 1000 for B32 compliance)
    pub fn iterations(mut self, iterations: usize) -> Self {
        assert!(iterations >= 1000, "B32 requires minimum 1000 iterations");
        self.iterations = iterations;
        self
    }

    /// Set number of warmup iterations
    pub fn warmup(mut self, warmup: usize) -> Self {
        self.warmup = warmup;
        self
    }
}

/// Run a benchmark with optimized and baseline implementations
///
/// # Arguments
/// * `config` - Benchmark configuration
/// * `optimized` - Optimized implementation to benchmark
/// * `baseline` - Fair baseline implementation for comparison
///
/// # B32 Compliance
///
/// This function enforces B32 compliance:
/// - Minimum 1000 iterations (configurable)
/// - 95% confidence intervals
/// - 27 hardware validation checks
/// - Fair baseline comparison
/// - Tier classification (TYPICAL/EXCEPTIONAL/BREAKTHROUGH/SUSPICIOUS)
pub fn run_benchmark<F, G>(
    config: BenchmarkConfig,
    optimized: F,
    baseline: G,
) where
    F: Fn() + Send + Sync,
    G: Fn() + Send + Sync,
{
    println!("\n🔬 kindly_bench - B32-compliant benchmarking");
    println!("Benchmarking: {}", config.name);

    // Collect hardware info
    let hardware = HardwareInfo::collect();

    // Create timer
    #[cfg(all(target_arch = "x86_64", feature = "tsc-timing"))]
    let timer = timing::tsc::TscTimer::new();

    #[cfg(not(all(target_arch = "x86_64", feature = "tsc-timing")))]
    let timer = timing::tsc::InstantTimer;

    // Warmup
    println!("Warming up ({} iterations)...", config.warmup);
    for _ in 0..config.warmup {
        optimized();
        baseline();
    }

    // Benchmark optimized implementation
    println!("Benchmarking optimized ({} iterations)...", config.iterations);
    let mut optimized_samples = Vec::with_capacity(config.iterations);
    for _ in 0..config.iterations {
        let start = timer.start();
        optimized();
        let end = timer.end();
        let elapsed_ns = timer.elapsed_ns(start, end);
        optimized_samples.push(elapsed_ns as f64);
    }

    // Benchmark baseline implementation
    println!("Benchmarking baseline ({} iterations)...", config.iterations);
    let mut baseline_samples = Vec::with_capacity(config.iterations);
    for _ in 0..config.iterations {
        let start = timer.start();
        baseline();
        let end = timer.end();
        let elapsed_ns = timer.elapsed_ns(start, end);
        baseline_samples.push(elapsed_ns as f64);
    }

    // Calculate statistics
    let optimized_stats = Statistics::from_samples(optimized_samples);
    let baseline_stats = Statistics::from_samples(baseline_samples);

    // Calculate speedup
    let speedup = optimized_stats.speedup(&baseline_stats);

    // Classify performance
    let classification = Classification::classify(&speedup);

    // Print terminal output
    output::print_results(
        &config.name,
        &config.tier,
        &config.baseline_kind,
        &optimized_stats,
        &baseline_stats,
        &classification,
        &hardware,
    );

    // Generate and save XML output
    let xml = output::xml::generate_xml(
        &config.name,
        &config.tier,
        &config.baseline_kind,
        &optimized_stats,
        &baseline_stats,
        &classification,
        &hardware,
    );

    let xml_filename = format!("{}_results.xml", config.name.replace(" ", "_").to_lowercase());
    if let Err(e) = output::xml::save_xml(&xml, &xml_filename) {
        eprintln!("Warning: Failed to save XML output: {}", e);
    } else {
        println!("\n✓ XML results saved to: {}", xml_filename);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_config() {
        let config = BenchmarkConfig::new("test", "T1-Atomic", "RwLock")
            .iterations(5000)
            .warmup(50);

        assert_eq!(config.name, "test");
        assert_eq!(config.tier, "T1-Atomic");
        assert_eq!(config.iterations, 5000);
        assert_eq!(config.warmup, 50);
    }

    #[test]
    #[should_panic(expected = "B32 requires minimum 1000 iterations")]
    fn test_benchmark_config_min_iterations() {
        BenchmarkConfig::new("test", "T1", "RwLock")
            .iterations(500); // Should panic
    }
}
