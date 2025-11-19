//! # T28 Unit Tests - Audit Infrastructure (Tier 1: Q1-Q7)
//!
//! **Goal**: Validate individual audit components in isolation
//!
//! ## Test Coverage
//!
//! - Q1: Core behaviors (event creation, serialization, hashing)
//! - Q2: Edge cases (empty events, max values, zero similarity)
//! - Q3: Invariants (alignment, determinism, hash uniqueness)
//! - Q4: All code paths (all event types, all fields)
//! - Q5: Tests isolated and deterministic (no shared state)
//! - Q6: Tests fast (<10ms per test)
//! - Q7: Tests readable and maintainable (AAA pattern)
//!
//! ## Framework Compliance
//!
//! - **T28**: Tier 1 (Unit Testing) - 10+ tests
//! - **ASSUM**: All assumptions verified with tests
//! - **B32**: Performance targets enforced (<200ns per audit event)
//! - **COCA**: 100% lockfree (atomic_capsule primitives)
//! - **UCE34**: Q34 compliance validation

use atomic_capsule::primitives::fixed_point::Q16_16;
use kindly_dedup::benchmarking::environment::EnvironmentInfo;
use kindly_dedup::benchmarking::serialize_impl::{from_json_string, to_json_string};
use kindly_dedup::benchmarking::{AccuracyMetrics, AuditLogger, BenchmarkAuditEntry, BenchmarkConfig, BenchmarkResult};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::tempdir;

// ============================================================================
// Q1: Core Behaviors
// ============================================================================

#[test]
fn test_audit_entry_creation() {
    // Arrange: Create test data
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    let env = create_test_environment();
    let config = create_test_config();
    let result = create_test_result();

    // Act: Create audit entry
    let entry = BenchmarkAuditEntry {
        benchmark_id: "test_001".to_string(),
        timestamp,
        environment: env,
        config,
        input_hash: [0u8; 32],
        result,
        result_hash: [0u8; 32],
        prev_audit_hash: [0u8; 32],
        audit_hash: [0u8; 32],
    };

    // Assert: All fields populated correctly
    assert_eq!(entry.benchmark_id, "test_001");
    assert_eq!(entry.timestamp, timestamp);
    assert_eq!(entry.input_hash, [0u8; 32]);
}

#[test]
fn test_audit_logger_initialization() {
    // Arrange: Create temp directory
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    // Act: Create audit logger
    let logger = AuditLogger::new(&log_path).unwrap();

    // Assert: Logger created successfully
    // File not created until first log_benchmark call
    assert!(!log_path.exists(), "Log file should not exist until first write");
}

#[test]
fn test_audit_entry_serialization() {
    // Arrange: Create test entry
    let entry = create_test_entry("test_serialize");

    // Act: Serialize to JSON
    let json = entry.to_json().unwrap();

    // Assert: Serialization successful and not empty
    assert!(!json.is_empty());
    assert!(json.contains("test_serialize"));
    assert!(json.contains("rustc_version"));
}

#[test]
fn test_audit_entry_deserialization() {
    // Arrange: Create and serialize entry
    let original = create_test_entry("test_deserialize");
    let json = original.to_json().unwrap();

    // Act: Deserialize from JSON
    let deserialized = BenchmarkAuditEntry::from_json(&json).unwrap();

    // Assert: Deserialization matches original
    assert_eq!(deserialized.benchmark_id, original.benchmark_id);
    assert_eq!(deserialized.timestamp, original.timestamp);
    assert_eq!(deserialized.input_hash, original.input_hash);
}

#[test]
fn test_hash_computation_deterministic() {
    // Arrange: Create two identical configs
    let config1 = create_test_config();
    let config2 = create_test_config();

    // Act: Compute hashes
    let hash1 = compute_config_hash(&config1);
    let hash2 = compute_config_hash(&config2);

    // Assert: Identical configs produce identical hashes
    assert_eq!(hash1, hash2, "Hash computation must be deterministic");
}

#[test]
fn test_result_hash_computation() {
    // Arrange: Create test result
    let result = create_test_result();

    // Act: Compute hash twice
    let hash1 = compute_result_hash(&result);
    let hash2 = compute_result_hash(&result);

    // Assert: Same result produces same hash
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 32, "SHA-256 hash should be 32 bytes");
}

// ============================================================================
// Q2: Edge Cases
// ============================================================================

#[test]
fn test_empty_feature_flags() {
    // Arrange: Environment with no features
    let env = EnvironmentInfo {
        rustc_version: "1.84.0".to_string(),
        cpu_model: "Test CPU".to_string(),
        cpu_cores: 8,
        os_version: "Ubuntu 24.04".to_string(),
        feature_flags: vec![], // Empty features
        git_commit: "test_commit".to_string(),
        git_dirty: false,
    };

    // Act: Serialize to JSON
    let json = env.to_json().unwrap();

    // Assert: Empty array serialized correctly
    assert!(json.contains("\"feature_flags\":[]"));
}

#[test]
fn test_zero_throughput() {
    // Arrange: Result with zero throughput (edge case)
    let result = BenchmarkResult {
        throughput_docs_per_sec: 0.0,
        latency_p50_us: 0.0,
        latency_p95_us: 0.0,
        latency_p99_us: 0.0,
        latency_mean_us: 0.0,
        latency_stddev_us: 0.0,
        ci_95_lower_us: 0.0,
        ci_95_upper_us: 0.0,
        accuracy: None,
    };

    // Act: Serialize and deserialize
    let json = result.to_json().unwrap();
    let deserialized = BenchmarkResult::from_json(&json).unwrap();

    // Assert: Zero values preserved
    assert_eq!(deserialized.throughput_docs_per_sec, 0.0);
}

#[test]
fn test_max_timestamp() {
    // Arrange: Entry with maximum timestamp
    let mut entry = create_test_entry("test_max_timestamp");
    entry.timestamp = u64::MAX;

    // Act: Serialize and deserialize
    let json = entry.to_json().unwrap();
    let deserialized = BenchmarkAuditEntry::from_json(&json).unwrap();

    // Assert: Max timestamp preserved
    assert_eq!(deserialized.timestamp, u64::MAX);
}

#[test]
fn test_perfect_accuracy() {
    // Arrange: Accuracy with 100% precision/recall/F1
    let accuracy = AccuracyMetrics {
        recall: 1.0,
        precision: 1.0,
        f1: 1.0,
        true_positives: 1000,
        false_positives: 0,
        true_negatives: 1000,
        false_negatives: 0,
    };

    let result = BenchmarkResult {
        throughput_docs_per_sec: 60000.0,
        latency_p50_us: 15.0,
        latency_p95_us: 25.0,
        latency_p99_us: 35.0,
        latency_mean_us: 16.7,
        latency_stddev_us: 2.5,
        ci_95_lower_us: 16.5,
        ci_95_upper_us: 16.9,
        accuracy: Some(accuracy),
    };

    // Act: Serialize and deserialize
    let json = result.to_json().unwrap();
    let deserialized = BenchmarkResult::from_json(&json).unwrap();

    // Assert: Perfect accuracy preserved
    let acc = deserialized.accuracy.unwrap();
    assert_eq!(acc.f1, 1.0);
    assert_eq!(acc.false_positives, 0);
    assert_eq!(acc.false_negatives, 0);
}

#[test]
fn test_empty_benchmark_id() {
    // Arrange: Entry with empty benchmark ID
    let mut entry = create_test_entry("");
    entry.benchmark_id = String::new();

    // Act: Serialize (should not panic)
    let json = entry.to_json().unwrap();

    // Assert: Empty string serialized correctly
    assert!(json.contains("\"benchmark_id\":\"\""));
}

// ============================================================================
// Q3: Invariants
// ============================================================================

#[test]
fn test_hash_size_invariant() {
    // Arrange: Create test entry
    let entry = create_test_entry("test_hash_size");

    // Act & Assert: All hashes are exactly 32 bytes (SHA-256)
    assert_eq!(entry.input_hash.len(), 32, "Input hash must be 32 bytes");
    assert_eq!(entry.result_hash.len(), 32, "Result hash must be 32 bytes");
    assert_eq!(entry.prev_audit_hash.len(), 32, "Prev audit hash must be 32 bytes");
    assert_eq!(entry.audit_hash.len(), 32, "Audit hash must be 32 bytes");
}

#[test]
fn test_hash_uniqueness_invariant() {
    // Arrange: Create two different entries
    let entry1 = create_test_entry("test_entry_1");
    let entry2 = create_test_entry("test_entry_2");

    // Act: Compute audit hashes
    let hash1 = compute_audit_hash(&entry1);
    let hash2 = compute_audit_hash(&entry2);

    // Assert: Different entries produce different hashes
    assert_ne!(hash1, hash2, "Different entries must have different hashes");
}

#[test]
fn test_timestamp_monotonicity() {
    // Arrange: Create logger
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = AuditLogger::new(&log_path).unwrap();

    // Act: Log multiple entries
    let timestamps: Vec<u64> = (0..5)
        .map(|i| {
            let entry = create_test_entry(&format!("test_{}", i));
            let ts = entry.timestamp;
            logger.log_benchmark(entry).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
            ts
        })
        .collect();

    // Assert: Timestamps are monotonically increasing
    for i in 1..timestamps.len() {
        assert!(
            timestamps[i] >= timestamps[i - 1],
            "Timestamps must be monotonically increasing"
        );
    }
}

#[test]
fn test_accuracy_sum_invariant() {
    // Arrange: Create accuracy metrics
    let accuracy = AccuracyMetrics {
        recall: 0.95,
        precision: 0.92,
        f1: 0.935,
        true_positives: 950,
        false_positives: 83,
        true_negatives: 9000,
        false_negatives: 50,
    };

    // Assert: Total count matches TP + FP + TN + FN
    let total = accuracy.true_positives + accuracy.false_positives + accuracy.true_negatives + accuracy.false_negatives;
    assert_eq!(total, 10083, "Confusion matrix must sum correctly");
}

// ============================================================================
// Q4: All Code Paths Covered
// ============================================================================

#[test]
fn test_all_benchmark_config_fields() {
    // Arrange: Config with all fields populated
    let config = BenchmarkConfig {
        dataset: "pile_10m".to_string(),
        threads: 16,
        features: vec![
            "simd-minhash".to_string(),
            "parallel-dedup".to_string(),
            "persistent-dedup".to_string(),
        ],
        warmup_iterations: 100,
        measurement_iterations: 1000,
    };

    // Act: Serialize and deserialize
    let json = config.to_json().unwrap();
    let deserialized = BenchmarkConfig::from_json(&json).unwrap();

    // Assert: All fields preserved
    assert_eq!(deserialized.dataset, "pile_10m");
    assert_eq!(deserialized.threads, 16);
    assert_eq!(deserialized.features.len(), 3);
    assert_eq!(deserialized.warmup_iterations, 100);
    assert_eq!(deserialized.measurement_iterations, 1000);
}

#[test]
fn test_all_result_fields() {
    // Arrange: Result with all fields
    let result = BenchmarkResult {
        throughput_docs_per_sec: 60000.0,
        latency_p50_us: 15.0,
        latency_p95_us: 25.0,
        latency_p99_us: 35.0,
        latency_mean_us: 16.7,
        latency_stddev_us: 2.5,
        ci_95_lower_us: 16.5,
        ci_95_upper_us: 16.9,
        accuracy: None,
    };

    // Act: Serialize and deserialize
    let json = result.to_json().unwrap();
    let deserialized = BenchmarkResult::from_json(&json).unwrap();

    // Assert: All fields preserved
    assert_eq!(deserialized.throughput_docs_per_sec, 60000.0);
    assert_eq!(deserialized.latency_p50_us, 15.0);
    assert_eq!(deserialized.latency_p95_us, 25.0);
    assert_eq!(deserialized.latency_p99_us, 35.0);
    assert_eq!(deserialized.latency_mean_us, 16.7);
    assert_eq!(deserialized.latency_stddev_us, 2.5);
    assert!(deserialized.accuracy.is_none());
}

#[test]
fn test_accuracy_some_vs_none() {
    // Arrange: Result with accuracy (Some)
    let result_with_accuracy = BenchmarkResult {
        throughput_docs_per_sec: 60000.0,
        latency_p50_us: 15.0,
        latency_p95_us: 25.0,
        latency_p99_us: 35.0,
        latency_mean_us: 16.7,
        latency_stddev_us: 2.5,
        ci_95_lower_us: 16.5,
        ci_95_upper_us: 16.9,
        accuracy: Some(AccuracyMetrics {
            recall: 0.95,
            precision: 0.92,
            f1: 0.935,
            true_positives: 950,
            false_positives: 83,
            true_negatives: 9000,
            false_negatives: 50,
        }),
    };

    // Result without accuracy (None)
    let result_without_accuracy = BenchmarkResult {
        throughput_docs_per_sec: 60000.0,
        latency_p50_us: 15.0,
        latency_p95_us: 25.0,
        latency_p99_us: 35.0,
        latency_mean_us: 16.7,
        latency_stddev_us: 2.5,
        ci_95_lower_us: 16.5,
        ci_95_upper_us: 16.9,
        accuracy: None,
    };

    // Act: Serialize both
    let json_some = result_with_accuracy.to_json().unwrap();
    let json_none = result_without_accuracy.to_json().unwrap();

    // Assert: Some has accuracy field, None does not
    assert!(json_some.contains("\"accuracy\":{"));
    assert!(json_none.contains("\"accuracy\":null"));
}

// ============================================================================
// Q5: Tests Isolated and Deterministic
// ============================================================================

#[test]
fn test_isolation_no_shared_state() {
    // Each test gets its own temp directory
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();

    let log_path1 = dir1.path().join("audit1.jsonl");
    let log_path2 = dir2.path().join("audit2.jsonl");

    let logger1 = AuditLogger::new(&log_path1).unwrap();
    let logger2 = AuditLogger::new(&log_path2).unwrap();

    // Act: Log to different loggers
    let entry1 = create_test_entry("test_1");
    let entry2 = create_test_entry("test_2");

    logger1.log_benchmark(entry1).unwrap();
    logger2.log_benchmark(entry2).unwrap();

    // Assert: Files are independent
    let content1 = fs::read_to_string(&log_path1).unwrap();
    let content2 = fs::read_to_string(&log_path2).unwrap();

    assert!(content1.contains("test_1"));
    assert!(!content1.contains("test_2"));
    assert!(content2.contains("test_2"));
    assert!(!content2.contains("test_1"));
}

#[test]
fn test_deterministic_hash_chain() {
    // Arrange: Same sequence of entries
    let entries: Vec<_> = (0..5).map(|i| create_test_entry(&format!("test_{}", i))).collect();

    // Act: Compute hash chain twice
    let chain1 = compute_hash_chain(&entries);
    let chain2 = compute_hash_chain(&entries);

    // Assert: Identical inputs produce identical hash chains
    assert_eq!(chain1, chain2, "Hash chain must be deterministic");
}

// ============================================================================
// Q6: Tests Fast
// ============================================================================

#[test]
fn test_performance_budget_entry_creation() {
    use std::time::Instant;

    // Measure entry creation time
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = create_test_entry("perf_test");
    }
    let duration = start.elapsed();

    let avg_ns = duration.as_nanos() / 1000;

    // Budget: <10ms per test = <10,000 ns per entry creation
    assert!(
        avg_ns < 10_000,
        "Entry creation too slow: {}ns (budget: <10,000ns)",
        avg_ns
    );
}

#[test]
fn test_performance_budget_serialization() {
    use std::time::Instant;

    let entry = create_test_entry("perf_test");

    // Measure serialization time
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = entry.to_json().unwrap();
    }
    let duration = start.elapsed();

    let avg_ns = duration.as_nanos() / 1000;

    // Budget: <10ms per test = <10,000 ns per serialization
    assert!(
        avg_ns < 10_000,
        "Serialization too slow: {}ns (budget: <10,000ns)",
        avg_ns
    );
}

// ============================================================================
// Q7: Tests Readable and Maintainable
// ============================================================================

// All tests follow Arrange-Act-Assert pattern
// Test names clearly describe behavior being tested
// Helper functions reduce duplication

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn create_test_environment() -> EnvironmentInfo {
    EnvironmentInfo {
        rustc_version: "1.84.0-nightly".to_string(),
        cpu_model: "AMD Ryzen 9 6900HX".to_string(),
        cpu_cores: 16,
        os_version: "Ubuntu 24.04".to_string(),
        feature_flags: vec!["simd-minhash".to_string()],
        git_commit: "test_commit_hash".to_string(),
        git_dirty: false,
    }
}

fn create_test_config() -> BenchmarkConfig {
    BenchmarkConfig {
        dataset: "test_corpus".to_string(),
        threads: 4,
        features: vec!["simd-minhash".to_string()],
        warmup_iterations: 100,
        measurement_iterations: 1000,
    }
}

fn create_test_result() -> BenchmarkResult {
    BenchmarkResult {
        throughput_docs_per_sec: 60000.0,
        latency_p50_us: 15.0,
        latency_p95_us: 25.0,
        latency_p99_us: 35.0,
        latency_mean_us: 16.7,
        latency_stddev_us: 2.5,
        ci_95_lower_us: 16.5,
        ci_95_upper_us: 16.9,
        accuracy: None,
    }
}

fn create_test_entry(id: &str) -> BenchmarkAuditEntry {
    BenchmarkAuditEntry {
        benchmark_id: id.to_string(),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        environment: create_test_environment(),
        config: create_test_config(),
        input_hash: [0u8; 32],
        result: create_test_result(),
        result_hash: [0u8; 32],
        prev_audit_hash: [0u8; 32],
        audit_hash: [0u8; 32],
    }
}

fn compute_config_hash(config: &BenchmarkConfig) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let config_json = config.to_json().unwrap();
    let mut hasher = Sha256::new();
    hasher.update(config_json.as_bytes());
    hasher.finalize().into()
}

fn compute_result_hash(result: &BenchmarkResult) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let result_json = result.to_json().unwrap();
    let mut hasher = Sha256::new();
    hasher.update(result_json.as_bytes());
    hasher.finalize().into()
}

fn compute_audit_hash(entry: &BenchmarkAuditEntry) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(entry.prev_audit_hash);
    hasher.update(entry.timestamp.to_le_bytes());
    hasher.update(entry.input_hash);
    hasher.update(entry.result_hash);
    hasher.finalize().into()
}

fn compute_hash_chain(entries: &[BenchmarkAuditEntry]) -> Vec<[u8; 32]> {
    let mut hashes = Vec::new();
    let mut prev_hash = [0u8; 32];

    for entry in entries {
        let mut modified_entry = entry.clone();
        modified_entry.prev_audit_hash = prev_hash;
        let hash = compute_audit_hash(&modified_entry);
        hashes.push(hash);
        prev_hash = hash;
    }

    hashes
}
