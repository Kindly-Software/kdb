//! # Audit Infrastructure Integration Tests (T28 Framework)
//!
//! **Q15-Q21: Integration Testing Tier**
//!
//! Tests complete audit trail workflows:
//! - Multi-benchmark runs (5+ entries)
//! - Hash chain integrity verification
//! - Tamper detection
//! - Environment capture integration
//! - Large audit logs (100+ entries)
//! - Audit viewer CLI functionality
//!
//! ## T28 Compliance
//!
//! - Q15: Component integration (AuditLogger + EnvironmentCapture)
//! - Q16: Data flow validation (entry → hash chain → verify)
//! - Q17: Error propagation (malformed JSON, broken chain)
//! - Q18: State consistency (prev_hash continuity)
//! - Q19: Cross-module integration (benchmarking module)
//! - Q20: Realistic workflows (multi-run scenarios)
//! - Q21: Stress testing (1000+ entries)

use kindly_dedup::benchmarking::{
    AuditLogger, BenchmarkAuditEntry, BenchmarkConfig, BenchmarkResult, EnvironmentCapture, EnvironmentInfo,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::tempdir;

// ============================================================================
// Q15: Component Integration Tests
// ============================================================================

#[test]
fn test_audit_logger_and_environment_integration() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    // Capture environment
    let env = EnvironmentCapture::capture().unwrap();

    // Create logger
    let logger = AuditLogger::new(&log_path).unwrap();

    // Log entry with captured environment
    let entry = create_entry_with_env("test_integration_001", &env);
    logger.log_benchmark(entry).unwrap();

    // Verify file exists and has content
    assert!(log_path.exists());
    let content = fs::read_to_string(&log_path).unwrap();
    assert!(!content.is_empty());

    // Verify environment fields present in JSON
    assert!(content.contains(&env.rustc_version));
    assert!(content.contains(&env.cpu_model));
}

#[test]
fn test_multi_benchmark_workflow() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = AuditLogger::new(&log_path).unwrap();
    let env = EnvironmentCapture::capture().unwrap();

    // Simulate 5 benchmark runs
    for i in 1..=5 {
        let entry = create_entry_with_env(&format!("benchmark_{:03}", i), &env);
        logger.log_benchmark(entry).unwrap();
    }

    // Verify 5 entries written
    let content = fs::read_to_string(&log_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 5);

    // Verify integrity
    assert!(logger.verify_integrity().unwrap());
}

// ============================================================================
// Q16: Data Flow Validation Tests
// ============================================================================

#[test]
fn test_hash_chain_continuity() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = AuditLogger::new(&log_path).unwrap();
    let env = EnvironmentCapture::capture().unwrap();

    // Log 3 entries
    let mut prev_hash = [0u8; 32]; // Genesis hash
    for i in 1..=3 {
        let mut entry = create_entry_with_env(&format!("hash_chain_{:03}", i), &env);

        // Log entry
        logger.log_benchmark(entry.clone()).unwrap();

        // Read back and verify prev_hash matches
        let content = fs::read_to_string(&log_path).unwrap();
        let last_line = content.lines().last().unwrap();
        let logged_entry = BenchmarkAuditEntry::from_json(last_line).unwrap();

        assert_eq!(logged_entry.prev_audit_hash, prev_hash);
        prev_hash = logged_entry.audit_hash;
    }
}

#[test]
fn test_input_and_result_hash_computation() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = AuditLogger::new(&log_path).unwrap();
    let env = EnvironmentCapture::capture().unwrap();

    let entry = create_entry_with_env("hash_test_001", &env);
    logger.log_benchmark(entry).unwrap();

    // Read back
    let content = fs::read_to_string(&log_path).unwrap();
    let logged_entry = BenchmarkAuditEntry::from_json(content.trim()).unwrap();

    // Verify hashes are non-zero (actually computed)
    assert_ne!(logged_entry.input_hash, [0u8; 32]);
    assert_ne!(logged_entry.result_hash, [0u8; 32]);
    assert_ne!(logged_entry.audit_hash, [0u8; 32]);
}

// ============================================================================
// Q17: Error Propagation Tests
// ============================================================================

#[test]
fn test_malformed_json_detection() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    // Write malformed JSON
    fs::write(&log_path, "{ invalid json\n").unwrap();

    // Logger should handle gracefully
    let logger = AuditLogger::new(&log_path);
    assert!(logger.is_ok());

    // Verify should fail
    let result = logger.unwrap().verify_integrity();
    assert!(result.is_err() || !result.unwrap());
}

#[test]
fn test_broken_hash_chain_detection() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = AuditLogger::new(&log_path).unwrap();
    let env = EnvironmentCapture::capture().unwrap();

    // Log 3 entries
    for i in 1..=3 {
        let entry = create_entry_with_env(&format!("break_test_{:03}", i), &env);
        logger.log_benchmark(entry).unwrap();
    }

    // Tamper with middle entry
    let content = fs::read_to_string(&log_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    // Replace second line with modified entry
    let mut tampered = lines[0].to_string();
    tampered.push('\n');

    // Parse and modify second entry
    let mut entry = BenchmarkAuditEntry::from_json(lines[1]).unwrap();
    entry.result.throughput_docs_per_sec += 1000.0; // Tamper with result
    tampered.push_str(&entry.to_json().unwrap());
    tampered.push('\n');

    tampered.push_str(lines[2]);
    tampered.push('\n');

    fs::write(&log_path, tampered).unwrap();

    // Verify should fail
    let logger = AuditLogger::new(&log_path).unwrap();
    assert!(!logger.verify_integrity().unwrap());
}

// ============================================================================
// Q18: State Consistency Tests
// ============================================================================

#[test]
fn test_prev_hash_state_consistency() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = AuditLogger::new(&log_path).unwrap();
    let env = EnvironmentCapture::capture().unwrap();

    // Log 10 entries and verify each prev_hash
    let mut expected_prev = [0u8; 32];

    for i in 1..=10 {
        let entry = create_entry_with_env(&format!("state_{:03}", i), &env);
        logger.log_benchmark(entry).unwrap();

        // Read back last entry
        let content = fs::read_to_string(&log_path).unwrap();
        let last_line = content.lines().last().unwrap();
        let logged = BenchmarkAuditEntry::from_json(last_line).unwrap();

        // Verify prev_hash consistency
        assert_eq!(logged.prev_audit_hash, expected_prev);
        expected_prev = logged.audit_hash;
    }
}

#[test]
fn test_logger_reload_preserves_state() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let env = EnvironmentCapture::capture().unwrap();

    // Create first logger and log 3 entries
    {
        let logger = AuditLogger::new(&log_path).unwrap();
        for i in 1..=3 {
            let entry = create_entry_with_env(&format!("reload_{:03}", i), &env);
            logger.log_benchmark(entry).unwrap();
        }
    }

    // Create new logger (should reload last hash)
    let logger = AuditLogger::new(&log_path).unwrap();

    // Log 2 more entries
    for i in 4..=5 {
        let entry = create_entry_with_env(&format!("reload_{:03}", i), &env);
        logger.log_benchmark(entry).unwrap();
    }

    // Verify full chain integrity
    assert!(logger.verify_integrity().unwrap());
}

// ============================================================================
// Q19: Cross-Module Integration Tests
// ============================================================================

#[test]
fn test_environment_capture_caching() {
    // First capture
    let env1 = EnvironmentCapture::capture().unwrap();

    // Second capture (should be cached)
    let env2 = EnvironmentCapture::capture().unwrap();

    // Should be identical (pointer equality via caching)
    assert_eq!(env1.rustc_version, env2.rustc_version);
    assert_eq!(env1.cpu_model, env2.cpu_model);
    assert_eq!(env1.cpu_cores, env2.cpu_cores);
}

#[test]
fn test_benchmarking_module_exports() {
    // Test all public exports are accessible
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");

    let _logger = AuditLogger::new(&log_path).unwrap();
    let _env = EnvironmentCapture::capture().unwrap();

    // Types should be accessible
    let _config = BenchmarkConfig {
        dataset: "test".to_string(),
        threads: 1,
        features: vec![],
        warmup_iterations: 10,
        measurement_iterations: 100,
    };

    let _result = BenchmarkResult {
        throughput_docs_per_sec: 1000.0,
        latency_p50_us: 1.0,
        latency_p95_us: 2.0,
        latency_p99_us: 3.0,
        latency_mean_us: 1.0,
        latency_stddev_us: 0.1,
        ci_95_lower_us: 0.9,
        ci_95_upper_us: 1.1,
        accuracy: None,
    };
}

// ============================================================================
// Q20: Realistic Workflow Tests
// ============================================================================

#[test]
fn test_daily_benchmark_workflow() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = AuditLogger::new(&log_path).unwrap();
    let env = EnvironmentCapture::capture().unwrap();

    // Simulate 7 days of daily benchmarks (3 runs per day)
    for day in 1..=7 {
        for run in 1..=3 {
            let benchmark_id = format!("daily_day{}_run{}", day, run);
            let mut entry = create_entry_with_env(&benchmark_id, &env);

            // Vary throughput (simulate performance changes)
            entry.result.throughput_docs_per_sec = 60000.0 + (day as f64 * 1000.0);

            logger.log_benchmark(entry).unwrap();
        }
    }

    // Verify 21 entries
    let content = fs::read_to_string(&log_path).unwrap();
    assert_eq!(content.lines().count(), 21);

    // Verify integrity
    assert!(logger.verify_integrity().unwrap());
}

#[test]
fn test_multi_version_benchmark_comparison() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = AuditLogger::new(&log_path).unwrap();
    let env = EnvironmentCapture::capture().unwrap();

    // Simulate benchmarks from 3 different versions
    let versions = ["v1.0", "v1.1", "v1.2"];
    let throughputs = [60000.0, 2_080_000.0, 1_500_000.0]; // v1.0, v1.1, v1.2

    for (version, throughput) in versions.iter().zip(throughputs.iter()) {
        let mut entry = create_entry_with_env(&format!("benchmark_{}", version), &env);
        entry.result.throughput_docs_per_sec = *throughput;
        logger.log_benchmark(entry).unwrap();
    }

    // Verify all entries logged
    assert!(logger.verify_integrity().unwrap());

    // Read back and verify throughput progression
    let content = fs::read_to_string(&log_path).unwrap();
    let entries: Vec<BenchmarkAuditEntry> = content
        .lines()
        .map(|line| BenchmarkAuditEntry::from_json(line).unwrap())
        .collect();

    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].result.throughput_docs_per_sec, 60000.0);
    assert_eq!(entries[1].result.throughput_docs_per_sec, 2_080_000.0);
    assert_eq!(entries[2].result.throughput_docs_per_sec, 1_500_000.0);
}

// ============================================================================
// Q21: Stress Testing
// ============================================================================

#[test]
fn test_large_audit_log_100_entries() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = AuditLogger::new(&log_path).unwrap();
    let env = EnvironmentCapture::capture().unwrap();

    // Log 100 entries
    for i in 1..=100 {
        let entry = create_entry_with_env(&format!("stress_{:04}", i), &env);
        logger.log_benchmark(entry).unwrap();
    }

    // Verify count
    let content = fs::read_to_string(&log_path).unwrap();
    assert_eq!(content.lines().count(), 100);

    // Verify integrity (all 100 entries)
    assert!(logger.verify_integrity().unwrap());
}

#[test]
fn test_large_audit_log_1000_entries() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("audit.jsonl");
    let logger = AuditLogger::new(&log_path).unwrap();
    let env = EnvironmentCapture::capture().unwrap();

    // Log 1000 entries (production stress test)
    for i in 1..=1000 {
        let entry = create_entry_with_env(&format!("production_{:05}", i), &env);
        logger.log_benchmark(entry).unwrap();
    }

    // Verify count
    let content = fs::read_to_string(&log_path).unwrap();
    assert_eq!(content.lines().count(), 1000);

    // Verify integrity (should complete in reasonable time)
    let start = std::time::Instant::now();
    assert!(logger.verify_integrity().unwrap());
    let duration = start.elapsed();

    // Should verify 1000 entries in <1 second
    assert!(duration.as_secs() < 1, "Verification took {:?}", duration);
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_entry_with_env(benchmark_id: &str, env: &EnvironmentInfo) -> BenchmarkAuditEntry {
    BenchmarkAuditEntry {
        benchmark_id: benchmark_id.to_string(),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        git_commit: env.git_commit.clone(),
        environment: env.clone(),
        config: BenchmarkConfig {
            dataset: "test_corpus".to_string(),
            threads: 4,
            features: vec!["simd-minhash".to_string()],
            warmup_iterations: 100,
            measurement_iterations: 1000,
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
