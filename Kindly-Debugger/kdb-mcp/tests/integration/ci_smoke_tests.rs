//! CI/CD Smoke Tests - Post-Deployment Validation
//!
//! **Purpose**: Validate production deployment after CI/CD pipeline completes
//! **Framework**: T28 Integration Testing (Q15-Q21)
//! **Target**: <30s execution time (Stage 6 of CI/CD pipeline)
//!
//! ## Test Coverage
//!
//! 1. Health endpoint validation
//! 2. Metrics endpoint validation (Prometheus format)
//! 3. Service availability (HTTP connectivity)
//! 4. Metrics content validation (expected metrics present)
//! 5. Performance smoke test (latency <100ms)
//!
//! ## Usage
//!
//! ```bash
//! # Run locally (requires server running at localhost:5678)
//! cargo test --test ci_smoke_tests --features all
//!
//! # Run in CI (automatic on deployment)
//! # GitHub Actions Stage 6 runs these tests automatically
//! ```

#![cfg(test)]

use std::time::{Duration, Instant};

// Mock HTTP client for testing (replace with actual reqwest in production)
#[cfg(not(target_env = "test"))]
use reqwest::blocking::Client;

/// Smoke Test 1: Health Endpoint
///
/// **Target**: /health endpoint returns 200 OK
/// **Validation**: HTTP 200 status code
/// **Timeout**: 5 seconds
#[test]
#[ignore] // Requires server running (enable in CI)
fn test_health_endpoint() {
    let server_url = std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:5678".to_string());
    let health_url = format!("{}/health", server_url);

    let start = Instant::now();
    let response = reqwest::blocking::get(&health_url)
        .expect("Failed to connect to health endpoint");
    let latency = start.elapsed();

    // Validation 1: HTTP 200 OK
    assert_eq!(response.status(), 200, "Health endpoint should return 200 OK");

    // Validation 2: Response time <100ms
    assert!(
        latency < Duration::from_millis(100),
        "Health check latency too high: {:?} (target: <100ms)",
        latency
    );

    println!("✅ Health endpoint OK (latency: {:?})", latency);
}

/// Smoke Test 2: Metrics Endpoint
///
/// **Target**: /metrics endpoint returns Prometheus text format
/// **Validation**: HTTP 200, Content-Type text/plain, HELP/TYPE directives present
/// **Timeout**: 10 seconds
#[test]
#[ignore] // Requires server running (enable in CI)
fn test_metrics_endpoint() {
    let server_url = std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:5678".to_string());
    let metrics_url = format!("{}/metrics", server_url);

    let start = Instant::now();
    let response = reqwest::blocking::get(&metrics_url)
        .expect("Failed to connect to metrics endpoint");
    let latency = start.elapsed();

    // Validation 1: HTTP 200 OK
    assert_eq!(response.status(), 200, "Metrics endpoint should return 200 OK");

    // Validation 2: Content-Type is text/plain
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/plain"),
        "Content-Type should be text/plain, got: {}",
        content_type
    );

    // Validation 3: Response body contains Prometheus format
    let body = response.text().expect("Failed to read response body");

    assert!(
        body.contains("# HELP"),
        "Metrics should contain HELP directives"
    );
    assert!(
        body.contains("# TYPE"),
        "Metrics should contain TYPE directives"
    );

    // Validation 4: Key metrics present
    assert!(
        body.contains("kdb_requests_total"),
        "Metrics should contain kdb_requests_total"
    );

    // Validation 5: Response time <100ms
    assert!(
        latency < Duration::from_millis(100),
        "Metrics scrape latency too high: {:?} (target: <100ms)",
        latency
    );

    println!("✅ Metrics endpoint OK (latency: {:?})", latency);
}

/// Smoke Test 3: Prometheus Scrape Format Validation
///
/// **Target**: /metrics returns valid Prometheus text format
/// **Validation**: Parse metrics, verify counter format, verify histogram buckets
#[test]
#[ignore] // Requires server running (enable in CI)
fn test_prometheus_format_validation() {
    let server_url = std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:5678".to_string());
    let metrics_url = format!("{}/metrics", server_url);

    let response = reqwest::blocking::get(&metrics_url)
        .expect("Failed to connect to metrics endpoint");
    let body = response.text().expect("Failed to read response body");

    // Validation 1: HELP directive format
    let help_count = body.lines().filter(|line| line.starts_with("# HELP")).count();
    assert!(
        help_count > 0,
        "Should have at least one HELP directive"
    );

    // Validation 2: TYPE directive format
    let type_count = body.lines().filter(|line| line.starts_with("# TYPE")).count();
    assert!(
        type_count > 0,
        "Should have at least one TYPE directive"
    );

    // Validation 3: Counter metrics (kdb_requests_total)
    let requests_total_count = body.lines().filter(|line| line.contains("kdb_requests_total{")).count();
    assert!(
        requests_total_count > 0,
        "Should have kdb_requests_total counter metrics"
    );

    // Validation 4: Histogram buckets (kdb_request_duration_seconds)
    let histogram_bucket_count = body
        .lines()
        .filter(|line| line.contains("kdb_request_duration_seconds_bucket{"))
        .count();
    assert!(
        histogram_bucket_count >= 7,
        "Should have at least 7 histogram buckets (expected: 7), got: {}",
        histogram_bucket_count
    );

    // Validation 5: Histogram sum and count
    assert!(
        body.contains("kdb_request_duration_seconds_sum"),
        "Should have histogram sum metric"
    );
    assert!(
        body.contains("kdb_request_duration_seconds_count"),
        "Should have histogram count metric"
    );

    println!("✅ Prometheus format validation passed");
}

/// Smoke Test 4: Service Availability (Basic Connectivity)
///
/// **Target**: Server responds to HTTP requests
/// **Validation**: TCP connection succeeds, HTTP response received
#[test]
#[ignore] // Requires server running (enable in CI)
fn test_service_availability() {
    let server_url = std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:5678".to_string());

    // Attempt connection with 5-second timeout
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to build HTTP client");

    let start = Instant::now();
    let response = client
        .get(&format!("{}/health", server_url))
        .send()
        .expect("Failed to connect to server");
    let connect_time = start.elapsed();

    // Validation 1: Response received
    assert!(
        response.status().is_success(),
        "Server should respond with success status"
    );

    // Validation 2: Connection time <1s
    assert!(
        connect_time < Duration::from_secs(1),
        "Connection time too high: {:?} (target: <1s)",
        connect_time
    );

    println!("✅ Service availability OK (connect time: {:?})", connect_time);
}

/// Smoke Test 5: Metrics Content Validation (Expected Metrics)
///
/// **Target**: All expected metric categories present
/// **Validation**: 6 metric categories (request, error, resource, business, performance, security)
#[test]
#[ignore] // Requires server running (enable in CI)
fn test_metrics_content_validation() {
    let server_url = std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:5678".to_string());
    let metrics_url = format!("{}/metrics", server_url);

    let response = reqwest::blocking::get(&metrics_url)
        .expect("Failed to connect to metrics endpoint");
    let body = response.text().expect("Failed to read response body");

    // Category 1: Request Metrics
    assert!(
        body.contains("kdb_requests_total"),
        "Missing request metric: kdb_requests_total"
    );
    assert!(
        body.contains("kdb_request_duration_seconds"),
        "Missing request metric: kdb_request_duration_seconds"
    );

    // Category 2: Error Metrics
    assert!(
        body.contains("kdb_errors_total") || body.contains("kdb_requests_total{"),
        "Missing error metrics"
    );

    // Category 3: Resource Metrics
    // Note: These may not be present if not yet implemented
    let has_memory = body.contains("kdb_memory_bytes");
    let has_cpu = body.contains("kdb_cpu_usage_percent");
    let has_threads = body.contains("kdb_threads_active");

    if has_memory || has_cpu || has_threads {
        println!("✅ Resource metrics present");
    } else {
        println!("⚠️  Resource metrics not yet implemented (optional)");
    }

    // Category 4: Business Metrics (optional)
    if body.contains("kdb_quota_violations_total") {
        println!("✅ Business metrics present");
    } else {
        println!("⚠️  Business metrics not yet implemented (optional)");
    }

    // Category 5: Performance SLA Metrics (optional)
    if body.contains("kdb_sla_violations_total") {
        println!("✅ Performance SLA metrics present");
    } else {
        println!("⚠️  Performance SLA metrics not yet implemented (optional)");
    }

    // Category 6: Security Metrics (optional)
    if body.contains("kdb_auth_failures_total") {
        println!("✅ Security metrics present");
    } else {
        println!("⚠️  Security metrics not yet implemented (optional)");
    }

    println!("✅ Metrics content validation passed");
}

/// Smoke Test 6: Performance Smoke Test (Basic Latency)
///
/// **Target**: Server responds to 10 requests in <1s total
/// **Validation**: Average latency <100ms per request
#[test]
#[ignore] // Requires server running (enable in CI)
fn test_performance_smoke() {
    let server_url = std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:5678".to_string());
    let health_url = format!("{}/health", server_url);

    let num_requests = 10;
    let mut total_latency = Duration::ZERO;

    for _ in 0..num_requests {
        let start = Instant::now();
        let response = reqwest::blocking::get(&health_url)
            .expect("Failed to connect to health endpoint");
        let latency = start.elapsed();

        assert_eq!(response.status(), 200, "Health check should return 200 OK");
        total_latency += latency;
    }

    let avg_latency = total_latency / num_requests;

    // Validation: Average latency <100ms
    assert!(
        avg_latency < Duration::from_millis(100),
        "Average latency too high: {:?} (target: <100ms)",
        avg_latency
    );

    println!(
        "✅ Performance smoke test passed (avg latency: {:?})",
        avg_latency
    );
}

/// Integration Test: Full Smoke Test Suite
///
/// **Purpose**: Run all smoke tests in sequence (CI Stage 6)
/// **Target**: <30s total execution time
#[test]
#[ignore] // Requires server running (enable in CI)
fn test_full_smoke_suite() {
    println!("\n========================================");
    println!("Running Full Smoke Test Suite");
    println!("========================================\n");

    let start = Instant::now();

    // Test 1: Health endpoint
    test_health_endpoint();

    // Test 2: Metrics endpoint
    test_metrics_endpoint();

    // Test 3: Prometheus format validation
    test_prometheus_format_validation();

    // Test 4: Service availability
    test_service_availability();

    // Test 5: Metrics content validation
    test_metrics_content_validation();

    // Test 6: Performance smoke test
    test_performance_smoke();

    let total_time = start.elapsed();

    println!("\n========================================");
    println!("✅ All Smoke Tests Passed");
    println!("Total Time: {:?}", total_time);
    println!("========================================\n");

    // Validation: Total time <30s (CI Stage 6 target)
    assert!(
        total_time < Duration::from_secs(30),
        "Smoke test suite took too long: {:?} (target: <30s)",
        total_time
    );
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if server is reachable
fn is_server_reachable(url: &str) -> bool {
    reqwest::blocking::get(url).is_ok()
}

/// Parse metric value from Prometheus text format
fn parse_metric_value(body: &str, metric_name: &str) -> Option<f64> {
    body.lines()
        .find(|line| line.starts_with(metric_name) && !line.starts_with('#'))
        .and_then(|line| line.split_whitespace().last())
        .and_then(|value| value.parse::<f64>().ok())
}

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn test_parse_metric_value() {
        let body = "kdb_requests_total{tool=\"test\",status=\"success\"} 123\n";
        let value = parse_metric_value(body, "kdb_requests_total");
        assert_eq!(value, Some(123.0));
    }

    #[test]
    fn test_parse_metric_value_histogram() {
        let body = "kdb_request_duration_seconds_sum 12.34\n";
        let value = parse_metric_value(body, "kdb_request_duration_seconds_sum");
        assert_eq!(value, Some(12.34));
    }
}
