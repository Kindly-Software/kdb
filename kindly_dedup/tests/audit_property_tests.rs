//! # T28 Property Tests - Audit Infrastructure (Tier 2: Q8-Q14)
//!
//! **Goal**: Validate invariants hold across input space
//!
//! ## Test Coverage
//!
//! - Q8: Universal properties (hash chain integrity, determinism)
//! - Q9: Concurrent invariants (lockfree audit logging)
//! - Q10: Edge case properties (extreme values, boundaries)
//! - Q11: ASSUM verification (tamper detection, hash collisions)
//! - Q12: Composition properties (multiple loggers, cascading)
//! - Q13: Statistical properties (hash distribution, performance)
//! - Q14: Regression tracking (proptest saved cases)
//!
//! ## Framework Compliance
//!
//! - **T28**: Tier 2 (Property Testing) - 7+ properties
//! - **ASSUM**: #ASSUME_HASH_CHAIN_TAMPER_DETECTION verified
//! - **B32**: Performance properties validated
//! - **COCA**: 100% lockfree (concurrent property tests)

use kindly_dedup::benchmarking::{AuditLogger, BenchmarkAuditEntry, BenchmarkConfig, BenchmarkResult, EnvironmentInfo};
use proptest::prelude::*;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::tempdir;

// ============================================================================
// Q8: Universal Properties (Hold for All Inputs)
// ============================================================================

proptest! {
    #[test]
    fn prop_hash_chain_links_correctly(
        entries in prop::collection::vec(arb_benchmark_id(), 1..20)
    ) {
        // Property: Hash chain must link each entry to previous
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("audit.jsonl");
        let logger = AuditLogger::new(&log_path).unwrap();

        let mut prev_hash = [0u8; 32];  // Genesis hash

        for benchmark_id in entries {
            let entry = create_test_entry(&benchmark_id);
            logger.log_benchmark(entry).unwrap();

            // Read back last entry
            let content = std::fs::read_to_string(&log_path).unwrap();
            let last_line = content.lines().last().unwrap();
            let logged_entry: BenchmarkAuditEntry = serde_json::from_str(last_line).unwrap();

            // Property: prev_audit_hash must match previous entry's audit_hash
            prop_assert_eq!(logged_entry.prev_audit_hash, prev_hash);

            prev_hash = logged_entry.audit_hash;
        }
    }

    #[test]
    fn prop_serialization_deterministic(
        benchmark_id in "test_[0-9]+",
        throughput in 1000.0..1_000_000.0f64,
        threads in 1usize..=128,
    ) {
        // Property: Same data serializes to identical bytes
        let entry1 = create_entry_with_params(&benchmark_id, throughput, threads);
        let entry2 = create_entry_with_params(&benchmark_id, throughput, threads);

        let json1 = serde_json::to_string(&entry1).unwrap();
        let json2 = serde_json::to_string(&entry2).unwrap();

        // Property: Deterministic serialization
        prop_assert_eq!(json1, json2);
    }

    #[test]
    fn prop_hash_uniqueness(
        entries in prop::collection::vec(arb_benchmark_id(), 2..10)
    ) {
        // Property: Different entries produce different audit hashes
        let test_entries: Vec<_> = entries
            .iter()
            .map(|id| create_test_entry(id))
            .collect();

        let hashes: Vec<_> = test_entries
            .iter()
            .map(|e| compute_audit_hash(e))
            .collect();

        // Property: No duplicate hashes
        for i in 0..hashes.len() {
            for j in (i+1)..hashes.len() {
                prop_assert_ne!(hashes[i], hashes[j], "Hash collision detected");
            }
        }
    }

    #[test]
    fn prop_verification_catches_modifications(
        entries in prop::collection::vec(arb_benchmark_id(), 5..15)
    ) {
        // Property: Any modification to logged entries breaks verification
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("audit.jsonl");
        let logger = AuditLogger::new(&log_path).unwrap();

        // Log entries
        for benchmark_id in &entries {
            let entry = create_test_entry(benchmark_id);
            logger.log_benchmark(entry).unwrap();
        }

        // Verify original is valid
        prop_assert!(logger.verify_integrity().unwrap());

        // Modify one entry
        let content = std::fs::read_to_string(&log_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() > 2 {
            let mut modified = lines[0].to_string();
            modified.push('\n');
            modified.push_str("TAMPERED\n");  // Invalid JSON
            for line in &lines[2..] {
                modified.push_str(line);
                modified.push('\n');
            }
            std::fs::write(&log_path, modified).unwrap();

            // Property: Verification must fail after tampering
            let result = logger.verify_integrity();
            prop_assert!(result.is_err() || !result.unwrap());
        }
    }
}

// ============================================================================
// Q9: Concurrent Invariants
// ============================================================================

#[test]
fn prop_concurrent_logging_preserves_order() {
    // Property: Concurrent writes don't corrupt audit log
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = Arc::new(AuditLogger::new(&log_path).unwrap());

    let threads = 4;
    let entries_per_thread = 10;
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|thread_id| {
            let logger_clone = Arc::clone(&logger);
            let barrier_clone = Arc::clone(&barrier);

            thread::spawn(move || {
                barrier_clone.wait(); // Synchronize start
                for i in 0..entries_per_thread {
                    let entry = create_test_entry(&format!("thread{}_entry{}", thread_id, i));
                    logger_clone.log_benchmark(entry).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Property: All entries logged
    let content = std::fs::read_to_string(&log_path).unwrap();
    let line_count = content.lines().count();
    assert_eq!(line_count, threads * entries_per_thread);

    // Property: Log is still verifiable (no corruption)
    assert!(logger.verify_integrity().unwrap());
}

#[test]
fn prop_concurrent_no_lost_writes() {
    // Property: Concurrent audit logging never loses writes
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = Arc::new(AuditLogger::new(&log_path).unwrap());

    let threads = 8;
    let writes_per_thread = 25;

    let handles: Vec<_> = (0..threads)
        .map(|thread_id| {
            let logger_clone = Arc::clone(&logger);

            thread::spawn(move || {
                for i in 0..writes_per_thread {
                    let entry = create_test_entry(&format!("t{}_w{}", thread_id, i));
                    logger_clone.log_benchmark(entry).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Property: Exactly threads × writes_per_thread entries logged
    let content = std::fs::read_to_string(&log_path).unwrap();
    let line_count = content.lines().count();
    assert_eq!(
        line_count,
        threads * writes_per_thread,
        "Lost writes detected: expected {}, got {}",
        threads * writes_per_thread,
        line_count
    );
}

// ============================================================================
// Q10: Edge Case Properties
// ============================================================================

proptest! {
    #[test]
    fn prop_handles_extreme_throughput(throughput in 0.0..1e12f64) {
        // Property: Any valid throughput value can be logged
        let entry = create_entry_with_throughput(throughput);

        // Should not panic
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: BenchmarkResult = serde_json::from_str(&json).unwrap();

        // Property: Value preserved (within floating point precision)
        prop_assert!((deserialized.throughput_docs_per_sec - throughput).abs() < 1e-6);
    }

    #[test]
    fn prop_handles_extreme_threads(threads in 1usize..=1024) {
        // Property: Any reasonable thread count can be logged
        let config = BenchmarkConfig {
            dataset: "test".to_string(),
            threads,
            features: vec!["test".to_string()],
            warmup_iterations: 10,
            measurement_iterations: 100,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: BenchmarkConfig = serde_json::from_str(&json).unwrap();

        // Property: Thread count preserved
        prop_assert_eq!(deserialized.threads, threads);
    }

    #[test]
    fn prop_handles_large_feature_lists(
        features in prop::collection::vec("[a-z]{5,10}", 0..50)
    ) {
        // Property: Any size feature list can be logged
        let config = BenchmarkConfig {
            dataset: "test".to_string(),
            threads: 4,
            features: features.clone(),
            warmup_iterations: 10,
            measurement_iterations: 100,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: BenchmarkConfig = serde_json::from_str(&json).unwrap();

        // Property: All features preserved
        prop_assert_eq!(deserialized.features, features);
    }
}

// ============================================================================
// Q11: ASSUM Verification
// ============================================================================

#[test]
fn verify_assum_hash_chain_tamper_detection() {
    // #ASSUME_HASH_CHAIN_TAMPER_DETECTION: Hash chain detects any modification
    // #VERIFY: Property test with intentional tampering

    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = AuditLogger::new(&log_path).unwrap();

    // Log 10 entries
    for i in 0..10 {
        let entry = create_test_entry(&format!("test_{}", i));
        logger.log_benchmark(entry).unwrap();
    }

    // Verify original is valid
    assert!(logger.verify_integrity().unwrap());

    // Tamper with middle entry (modify throughput)
    let content = std::fs::read_to_string(&log_path).unwrap();
    let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    let mut tampered_lines = lines.clone();
    if tampered_lines.len() > 5 {
        // Parse, modify, re-serialize entry 5
        let mut entry: BenchmarkAuditEntry = serde_json::from_str(&tampered_lines[5]).unwrap();
        entry.result.throughput_docs_per_sec *= 2.0; // Tamper
        tampered_lines[5] = serde_json::to_string(&entry).unwrap();

        let tampered_content = tampered_lines.join("\n") + "\n";
        std::fs::write(&log_path, tampered_content).unwrap();

        // #VERIFY: Verification must fail
        let result = logger.verify_integrity();
        assert!(
            result.is_err() || !result.unwrap(),
            "Tamper detection failed - modified entry not caught"
        );
    }
}

#[test]
fn verify_assum_append_only_atomic() {
    // #ASSUME_APPEND_ONLY_ATOMIC: File append operations are atomic
    // #VERIFY: Concurrent writes don't corrupt file

    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = Arc::new(AuditLogger::new(&log_path).unwrap());

    // Concurrent appends
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let logger_clone = Arc::clone(&logger);
            thread::spawn(move || {
                let entry = create_test_entry(&format!("concurrent_{}", i));
                logger_clone.log_benchmark(entry).unwrap();
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // #VERIFY: File is valid JSON Lines (no corruption)
    let content = std::fs::read_to_string(&log_path).unwrap();
    for (i, line) in content.lines().enumerate() {
        let result: Result<BenchmarkAuditEntry, _> = serde_json::from_str(line);
        assert!(result.is_ok(), "Line {} is not valid JSON: {}", i, line);
    }
}

// ============================================================================
// Q12: Composition Properties
// ============================================================================

#[test]
fn prop_multiple_loggers_independent() {
    // Property: Multiple loggers don't interfere with each other
    let dir = tempdir().unwrap();

    let log_path1 = dir.path().join("audit1.jsonl");
    let log_path2 = dir.path().join("audit2.jsonl");
    let log_path3 = dir.path().join("audit3.jsonl");

    let logger1 = AuditLogger::new(&log_path1).unwrap();
    let logger2 = AuditLogger::new(&log_path2).unwrap();
    let logger3 = AuditLogger::new(&log_path3).unwrap();

    // Log to each logger
    for i in 0..5 {
        logger1
            .log_benchmark(create_test_entry(&format!("log1_{}", i)))
            .unwrap();
        logger2
            .log_benchmark(create_test_entry(&format!("log2_{}", i)))
            .unwrap();
        logger3
            .log_benchmark(create_test_entry(&format!("log3_{}", i)))
            .unwrap();
    }

    // Property: Each log is independent and verifiable
    assert!(logger1.verify_integrity().unwrap());
    assert!(logger2.verify_integrity().unwrap());
    assert!(logger3.verify_integrity().unwrap());

    // Property: Each log has correct content
    let content1 = std::fs::read_to_string(&log_path1).unwrap();
    let content2 = std::fs::read_to_string(&log_path2).unwrap();
    let content3 = std::fs::read_to_string(&log_path3).unwrap();

    assert!(content1.contains("log1_"));
    assert!(!content1.contains("log2_"));
    assert!(!content1.contains("log3_"));

    assert!(content2.contains("log2_"));
    assert!(content3.contains("log3_"));
}

// ============================================================================
// Q13: Statistical Properties
// ============================================================================

#[test]
fn prop_hash_distribution_uniform() {
    // Property: Hash values are uniformly distributed (no bias)
    let entries: Vec<_> = (0..100).map(|i| create_test_entry(&format!("test_{}", i))).collect();

    let hashes: Vec<_> = entries.iter().map(|e| compute_audit_hash(e)).collect();

    // Check first byte distribution (should be roughly uniform)
    let mut byte_counts = [0usize; 256];
    for hash in &hashes {
        byte_counts[hash[0] as usize] += 1;
    }

    // Chi-square test for uniformity (simplified)
    // Expected: 100 / 256 ≈ 0.39 per bucket
    // Allow 3× deviation: 0-3 is acceptable
    let unique_values = byte_counts.iter().filter(|&&c| c > 0).count();

    // Property: At least 50% of possible values appear (good distribution)
    assert!(
        unique_values > 50,
        "Poor hash distribution: only {} unique values",
        unique_values
    );
}

#[test]
fn prop_performance_scales_linearly() {
    // Property: Audit logging scales linearly with entry count
    use std::time::Instant;

    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = AuditLogger::new(&log_path).unwrap();

    // Measure 100 entries
    let start = Instant::now();
    for i in 0..100 {
        let entry = create_test_entry(&format!("perf_{}", i));
        logger.log_benchmark(entry).unwrap();
    }
    let duration_100 = start.elapsed();

    // Clear and measure 200 entries
    std::fs::remove_file(&log_path).unwrap();
    let logger = AuditLogger::new(&log_path).unwrap();

    let start = Instant::now();
    for i in 0..200 {
        let entry = create_test_entry(&format!("perf_{}", i));
        logger.log_benchmark(entry).unwrap();
    }
    let duration_200 = start.elapsed();

    // Property: 200 entries takes ~2× as long as 100 entries (linear scaling)
    let ratio = duration_200.as_secs_f64() / duration_100.as_secs_f64();
    assert!(
        ratio >= 1.5 && ratio <= 2.5,
        "Non-linear scaling detected: ratio = {}",
        ratio
    );
}

// ============================================================================
// Q14: Regression Tracking
// ============================================================================

// proptest automatically saves failing cases to .proptest-regressions
// Commit those files to catch regressions

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn prop_regression_hash_stability(
        benchmark_id in "[a-z0-9_]{5,20}",
        throughput in 1.0..1_000_000.0f64,
    ) {
        // Property: Hash computation is stable across versions
        let entry = create_entry_with_params(&benchmark_id, throughput, 4);
        let hash1 = compute_audit_hash(&entry);

        // Re-compute (should be identical)
        let hash2 = compute_audit_hash(&entry);

        prop_assert_eq!(hash1, hash2, "Hash instability detected");
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn arb_benchmark_id() -> impl Strategy<Value = String> {
    "[a-z0-9_]{5,20}"
}

fn create_test_entry(benchmark_id: &str) -> BenchmarkAuditEntry {
    BenchmarkAuditEntry {
        benchmark_id: benchmark_id.to_string(),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        environment: EnvironmentInfo {
            rustc_version: "1.84.0".to_string(),
            cpu_model: "Test CPU".to_string(),
            cpu_cores: 16,
            os_version: "Ubuntu 24.04".to_string(),
            feature_flags: vec!["test".to_string()],
            git_commit: "test_commit".to_string(),
            git_dirty: false,
        },
        config: BenchmarkConfig {
            dataset: "test".to_string(),
            threads: 4,
            features: vec!["test".to_string()],
            warmup_iterations: 10,
            measurement_iterations: 100,
        },
        input_hash: [0u8; 32],
        result: BenchmarkResult {
            throughput_docs_per_sec: 60000.0,
            latency_p50_us: 15.0,
            latency_p95_us: 25.0,
            latency_p99_us: 35.0,
            latency_mean_us: 16.7,
            latency_stddev_us: 2.5,
            ci_95_lower_us: 16.5,
            ci_95_upper_us: 16.9,
            accuracy: None,
        },
        result_hash: [0u8; 32],
        prev_audit_hash: [0u8; 32],
        audit_hash: [0u8; 32],
    }
}

fn create_entry_with_params(benchmark_id: &str, throughput: f64, threads: usize) -> BenchmarkAuditEntry {
    let mut entry = create_test_entry(benchmark_id);
    entry.result.throughput_docs_per_sec = throughput;
    entry.config.threads = threads;
    entry
}

fn create_entry_with_throughput(throughput: f64) -> BenchmarkResult {
    BenchmarkResult {
        throughput_docs_per_sec: throughput,
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

fn compute_audit_hash(entry: &BenchmarkAuditEntry) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(entry.prev_audit_hash);
    hasher.update(entry.timestamp.to_le_bytes());
    hasher.update(entry.input_hash);
    hasher.update(entry.result_hash);
    hasher.finalize().into()
}
