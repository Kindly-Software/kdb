//! # Common Test Utilities
//!
//! Shared utilities for all kindly_dedup integration tests.
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q1-Q7 systematic test infrastructure
//! - **T28**: Tier-agnostic utilities (Unit/Property/Integration/Production)
//! - **ASSUM**: 100% safe test helpers
//! - **Chaos**: Zero-copy utilities where possible

use atomic_capsule::primitives::fixed_point::Q16_16;
use kindly_dedup::benchmarking::{
    AccuracyMetrics, BenchmarkAuditEntry, BenchmarkConfig, BenchmarkResult, EnvironmentInfo,
};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Audit Trail Test Utilities
// ============================================================================

/// Create a minimal test environment for audit testing
///
/// **Framework Compliance**: T28 Q1 (unit test helper)
pub fn create_test_environment() -> EnvironmentInfo {
    EnvironmentInfo {
        rust_version: "1.75.0".to_string(),
        rustc_version: "rustc 1.75.0 (82e1608df 2023-12-21)".to_string(),
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
        cpu_model: "AMD Ryzen 9 6900HX".to_string(),
        cpu_cores: 8,
        cpu_threads: 16,
        ram_gb: 64,
    }
}

/// Create a minimal test benchmark config
///
/// **Framework Compliance**: T28 Q1 (unit test helper)
pub fn create_test_config() -> BenchmarkConfig {
    BenchmarkConfig {
        num_documents: 10_000,
        num_permutations: 128,
        num_bands: 16,
        similarity_threshold: Q16_16::from_f32(0.85),
        num_threads: 1,
    }
}

/// Create a minimal test benchmark result
///
/// **Framework Compliance**: T28 Q1 (unit test helper)
pub fn create_test_result() -> BenchmarkResult {
    BenchmarkResult {
        total_duration_ms: 1000,
        throughput: 10_000.0,
        cpu_time_ms: 900,
        wall_time_ms: 1000,
        accuracy: Some(AccuracyMetrics {
            true_positives: 95,
            false_positives: 5,
            true_negatives: 9850,
            false_negatives: 50,
            precision: Q16_16::from_f32(0.95),
            recall: Q16_16::from_f32(0.65),
            f1_score: Q16_16::from_f32(0.77),
        }),
    }
}

/// Create a test audit entry with specified benchmark ID
///
/// **Framework Compliance**: T28 Q1 (unit test helper)
///
/// # Arguments
///
/// * `benchmark_id` - Unique identifier for the benchmark
///
/// # Returns
///
/// A fully initialized `BenchmarkAuditEntry` with deterministic fields
pub fn create_test_entry(benchmark_id: &str) -> BenchmarkAuditEntry {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    BenchmarkAuditEntry {
        benchmark_id: benchmark_id.to_string(),
        timestamp,
        environment: create_test_environment(),
        config: create_test_config(),
        input_hash: [0u8; 32],
        result: create_test_result(),
        result_hash: [0u8; 32],
        prev_audit_hash: [0u8; 32],
        audit_hash: [0u8; 32],
    }
}

/// Create a test audit entry with custom throughput
///
/// **Framework Compliance**: T28 Q2 (edge case testing)
///
/// # Arguments
///
/// * `benchmark_id` - Unique identifier for the benchmark
/// * `throughput` - Documents per second throughput
///
/// # Returns
///
/// A `BenchmarkAuditEntry` with specified throughput
pub fn create_entry_with_throughput(benchmark_id: &str, throughput: f64) -> BenchmarkAuditEntry {
    let mut entry = create_test_entry(benchmark_id);
    entry.result.throughput = throughput;
    entry
}

/// Create a test audit entry with custom parameters
///
/// **Framework Compliance**: T28 Q2 (edge case testing)
///
/// # Arguments
///
/// * `benchmark_id` - Unique identifier
/// * `throughput` - Documents per second
/// * `threads` - Number of threads used
///
/// # Returns
///
/// A `BenchmarkAuditEntry` with specified parameters
pub fn create_entry_with_params(benchmark_id: &str, throughput: f64, threads: usize) -> BenchmarkAuditEntry {
    let mut entry = create_entry_with_throughput(benchmark_id, throughput);
    entry.config.num_threads = threads;
    entry
}

/// Compute SHA-256 hash of an audit entry (for testing hash chains)
///
/// **Framework Compliance**: T28 Q3 (invariant verification)
///
/// **ASSUM**: Uses sha2 crate (external dependency, safe)
///
/// # Arguments
///
/// * `entry` - The audit entry to hash
///
/// # Returns
///
/// 32-byte SHA-256 hash
pub fn compute_audit_hash(entry: &BenchmarkAuditEntry) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(entry.benchmark_id.as_bytes());
    hasher.update(&entry.timestamp.to_le_bytes());
    hasher.update(&entry.input_hash);
    hasher.update(&entry.result_hash);
    hasher.update(&entry.prev_audit_hash);

    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

// ============================================================================
// Corpus Generation Utilities
// ============================================================================

/// Document type for test corpus generation
#[derive(Clone, Debug)]
pub struct Document {
    pub id: u64,
    pub text: String,
}

/// Create a test corpus with unique documents
///
/// **Framework Compliance**: T28 Q1 (unit test data generation)
///
/// # Arguments
///
/// * `n` - Number of documents to generate
///
/// # Returns
///
/// Vector of `n` unique documents with deterministic content
pub fn create_test_corpus(n: usize) -> Vec<Document> {
    (0..n)
        .map(|i| Document {
            id: i as u64,
            text: format!("Document {} with unique content {}", i, i * 13),
        })
        .collect()
}

/// Create a test corpus with known duplicates
///
/// **Framework Compliance**: T28 Q2 (edge case testing - duplicate detection)
///
/// # Arguments
///
/// * `n` - Total number of documents (must be even)
///
/// # Returns
///
/// Vector where every second document is a duplicate of the previous one
///
/// # Panics
///
/// Panics if `n` is odd
pub fn create_test_corpus_with_duplicates(n: usize) -> Vec<Document> {
    assert!(n % 2 == 0, "n must be even for duplicate corpus");

    (0..n)
        .map(|i| {
            let base_id = i / 2; // Create duplicate pairs
            Document {
                id: i as u64,
                text: format!("Document {} with duplicate content {}", base_id, base_id * 13),
            }
        })
        .collect()
}

// ============================================================================
// Accuracy Metrics Utilities
// ============================================================================

/// Calculate F1 score from precision and recall
///
/// **Framework Compliance**: T28 Q1 (accuracy validation)
///
/// # Arguments
///
/// * `precision` - Precision (TP / (TP + FP))
/// * `recall` - Recall (TP / (TP + FN))
///
/// # Returns
///
/// F1 score (2 * precision * recall / (precision + recall))
pub fn calculate_f1_score(precision: f64, recall: f64) -> f64 {
    if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    }
}

/// Normalize clusters for comparison (sort IDs within clusters, sort clusters)
///
/// **Framework Compliance**: T28 Q2 (edge case testing - cluster comparison)
///
/// # Arguments
///
/// * `clusters` - Vector of document ID clusters
///
/// # Returns
///
/// Normalized clusters (sorted for deterministic comparison)
pub fn normalize_clusters(mut clusters: Vec<Vec<u32>>) -> Vec<Vec<u32>> {
    // Sort IDs within each cluster
    for cluster in &mut clusters {
        cluster.sort_unstable();
    }

    // Sort clusters by first ID
    clusters.sort_by_key(|c| c.first().copied().unwrap_or(u32::MAX));

    clusters
}

// ============================================================================
// Proptest Strategies (for property testing)
// ============================================================================

#[cfg(test)]
mod proptest_helpers {
    use proptest::prelude::*;

    /// Arbitrary benchmark ID strategy
    ///
    /// **Framework Compliance**: T28 Q8 (property testing)
    pub fn arb_benchmark_id() -> impl Strategy<Value = String> {
        prop::string::string_regex("test_[0-9]{1,6}").unwrap()
    }

    /// Arbitrary throughput strategy (realistic range)
    ///
    /// **Framework Compliance**: T28 Q8 (property testing)
    pub fn arb_throughput() -> impl Strategy<Value = f64> {
        1000.0..1_000_000.0f64
    }

    /// Arbitrary thread count strategy
    ///
    /// **Framework Compliance**: T28 Q8 (property testing)
    pub fn arb_threads() -> impl Strategy<Value = usize> {
        1usize..=128
    }
}

#[cfg(test)]
pub use proptest_helpers::*;

// ============================================================================
// Test Utilities Tests (Meta-testing)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_environment() {
        let env = create_test_environment();
        assert_eq!(env.cpu_cores, 8);
        assert_eq!(env.cpu_threads, 16);
        assert_eq!(env.ram_gb, 64);
    }

    #[test]
    fn test_create_test_config() {
        let config = create_test_config();
        assert_eq!(config.num_documents, 10_000);
        assert_eq!(config.num_permutations, 128);
        assert_eq!(config.num_bands, 16);
    }

    #[test]
    fn test_create_test_corpus() {
        let corpus = create_test_corpus(10);
        assert_eq!(corpus.len(), 10);
        assert_eq!(corpus[0].id, 0);
        assert_eq!(corpus[9].id, 9);
    }

    #[test]
    fn test_create_test_corpus_with_duplicates() {
        let corpus = create_test_corpus_with_duplicates(10);
        assert_eq!(corpus.len(), 10);

        // Check duplicate pairs
        for i in (0..10).step_by(2) {
            assert_eq!(corpus[i].text, corpus[i + 1].text, "Pair {} should be duplicates", i / 2);
        }
    }

    #[test]
    fn test_calculate_f1_score() {
        let f1 = calculate_f1_score(0.95, 0.65);
        assert!((f1 - 0.77).abs() < 0.01, "F1 score should be ~0.77");
    }

    #[test]
    fn test_normalize_clusters() {
        let clusters = vec![vec![3, 1, 2], vec![6, 4, 5], vec![0]];
        let normalized = normalize_clusters(clusters);

        // First cluster should be [0]
        assert_eq!(normalized[0], vec![0]);
        // Second cluster should be [1, 2, 3]
        assert_eq!(normalized[1], vec![1, 2, 3]);
        // Third cluster should be [4, 5, 6]
        assert_eq!(normalized[2], vec![4, 5, 6]);
    }
}
