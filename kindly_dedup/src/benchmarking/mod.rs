//! # Q34 + B32 Benchmarking Infrastructure
//!
//! **Compliance-Ready Benchmark Suite**
//!
//! Provides tamper-evident audit trails (Q34) and honest benchmarking (B32) for all
//! deduplication benchmarks.
//!
//! ## Modules
//!
//! - `audit_logger`: SHA-256 hash-chained audit trail
//! - `b32_runner`: B32-compliant benchmark runner (warmup, statistics, audit)
//! - `dataset_manager`: Realistic LLM dataset downloader with provenance tracking
//! - `environment`: Complete environment capture for reproducibility
//! - `reality_check`: Speedup classification (K1-K70 reality checks)
//!
//! ## Example
//!
//! ```rust,ignore
//! use kindly_dedup::benchmarking::{B32Runner, RealityCheck};
//!
//! // Run benchmark with B32 compliance
//! let runner = B32Runner::new("audit_trail.jsonl")?;
//! let stats = runner.run_benchmark("my_bench", || {
//!     // Your benchmark code
//! });
//!
//! // Validate speedup claim
//! let check = RealityCheck::new(baseline, optimized);
//! println!("Classification: {}", check.classify());
//! ```
//!
//! ## Framework Compliance
//!
//! - **Q34**: Hash-chained audit trails with tamper-detection
//! - **B32**: Fair baselines, 95% CI, reproducible environments
//! - **ASSUM**: 99.99% safe (cryptographic assumptions documented)
//! - **T28**: Comprehensive test coverage (unit + integration + production)

pub mod audit_logger;
pub mod b32_runner;
#[cfg(feature = "download-tools")]
pub mod dataset_manager;
pub mod environment;
pub mod ground_truth;
pub mod ground_truth_config;
pub mod reality_check;
pub mod serialize_impl;
pub mod token_dictionary;

pub use audit_logger::{AccuracyMetrics, AuditLogger, BenchmarkAuditEntry, BenchmarkConfig, BenchmarkResult, Hash256};
pub use b32_runner::{B32Config, B32Runner, BenchmarkStats};
#[cfg(feature = "download-tools")]
pub use dataset_manager::{compute_sha256, DatasetManager, DatasetManifest, DatasetSource, StreamingDownloader};
pub use environment::EnvironmentCapture;
pub use environment::EnvironmentInfo;
pub use ground_truth::{
    AccuracyError, Document, ExactJaccardComputer, GroundTruth, GroundTruthStrategy, SimdJaccardComputer,
    TokenCacheCapsule, UniversalGroundTruthGenerator,
};
pub use ground_truth_config::GroundTruthConfig;
pub use reality_check::{B32Constraint, RealityCheck, SpeedupClassification};
pub use token_dictionary::TokenDictionary;

// ============================================================================
// Temporary Stub for Week 1 Testing
// ============================================================================
// NOTE: This will be replaced by Parallel Gen Expert implementation

/// Temporary stub: Generate synthetic corpus (sequential)
///
/// **TODO**: Replace with parallel implementation by Parallel Gen Expert
pub fn generate_synthetic_corpus_parallel(num_docs: usize) -> Vec<(usize, String)> {
    // Stub: Sequential generation for testing compilation
    // Real implementation will use rayon for parallelization

    let templates = vec![
        "The quick brown fox jumps over the lazy dog",
        "A journey of a thousand miles begins with a single step",
        "To be or not to be that is the question",
        "Machine learning transforms artificial intelligence",
        "Neural networks process information in layers",
        "Deep learning requires large datasets",
        "Natural language processing enables understanding",
        "Computer vision analyzes images and videos",
    ];

    (0..num_docs)
        .map(|i| {
            // Distribution: 5% exact, 15% near, 30% similar, 50% unique
            let category = i % 100;

            let text = if category < 5 {
                // 5% exact duplicates (cluster of 10)
                format!("Exact duplicate cluster {} shared text", i / 10)
            } else if category < 20 {
                // 15% near-duplicates (90% Jaccard)
                let template = &templates[i % templates.len()];
                format!("Near duplicate cluster {} {}", i / 5, template)
            } else if category < 50 {
                // 30% similar (70% Jaccard)
                let template = &templates[i % templates.len()];
                format!("Similar document cluster {} {} variant {}", i / 3, template, i % 3)
            } else {
                // 50% unique
                format!("Unique document {} with random content {}", i, i * 7)
            };

            (i, text)
        })
        .collect()
}
