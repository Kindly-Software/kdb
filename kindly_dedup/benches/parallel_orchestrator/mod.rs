//! # ParallelDedupOrchestrator Benchmark Suite
//!
//! **Purpose**: Comprehensive Criterion.rs benchmarks for ParallelDedupOrchestrator v2.0
//!
//! **B32 Framework Compliance**:
//! - Fair baselines (sequential DedupPipeline vs parallel ParallelDedupOrchestrator)
//! - 1000+ iterations (statistical rigor)
//! - 95% confidence intervals
//! - Realistic workloads (1K-1M documents, 50% duplicate ratio)
//! - Amdahl's Law validation (1, 2, 4, 8, 16 threads)
//!
//! **Target**: Validate 4.8-5.3× speedup @ 16 threads
//!
//! **Suites**:
//! - `speedup_curve`: Amdahl's Law validation (1-16 threads)
//! - `phase_breakdown`: Per-phase performance analysis
//! - `realistic_workload`: Production-like scenarios (1K-1M docs)

use criterion::{criterion_group, criterion_main, Criterion};

pub mod speedup_curve;
pub mod phase_breakdown;
pub mod realistic_workload;

// Configuration function (copied from criterion_config, or can be imported if that becomes a proper module)
fn configure_criterion() -> Criterion {
    Criterion::default()
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets =
        speedup_curve::bench_speedup_curve,
        phase_breakdown::bench_phase_breakdown,
        realistic_workload::bench_realistic_workload
}

criterion_main!(benches);

/// Generate deterministic test corpus for reproducible benchmarks
///
/// **Determinism**: Seeded LCG (seed=42) for exact reproducibility
///
/// **Corpus Structure**:
/// - `unique_count = (1.0 - duplicate_ratio) × size` unique documents
/// - Remaining documents are duplicates of unique set
/// - Each document: 50 words, deterministic word IDs
///
/// **Arguments**:
/// - `size`: Total number of documents
/// - `duplicate_ratio`: Fraction of duplicates (0.0-1.0)
///
/// **Returns**: Vec of document texts
///
/// **Example**:
/// ```rust
/// let docs = generate_test_corpus(10_000, 0.5);  // 10K docs, 50% duplicates
/// assert_eq!(docs.len(), 10_000);
/// ```
pub fn generate_test_corpus(size: usize, duplicate_ratio: f64) -> Vec<String> {
    // Simple deterministic LCG (Linear Congruential Generator)
    let mut state = 42u64;
    let next_rand = |state: &mut u64| -> u32 {
        *state = state.wrapping_mul(1103515245).wrapping_add(12345);
        ((*state / 65536) % 32768) as u32
    };

    let unique_count = ((1.0 - duplicate_ratio) * size as f64) as usize;

    let mut docs = Vec::new();

    // Generate unique documents
    for i in 0..unique_count {
        let mut words = Vec::new();
        for _ in 0..50 {
            words.push(format!("word{}", next_rand(&mut state)));
        }
        docs.push(format!("Document {} {}", i, words.join(" ")));
    }

    // Add duplicates (cyclic sampling from unique set)
    let mut dup_idx = 0;
    while docs.len() < size {
        docs.push(docs[dup_idx % unique_count].clone());
        dup_idx += 1;
    }

    docs
}
