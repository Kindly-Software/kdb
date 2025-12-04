//! Common Test Infrastructure for Integration Tests
//!
//! Provides shared helpers, mocks, and fixtures for T28 Q15-Q21 integration testing.

use kdb_mcp::*;
use kdb::DebuggerCapsule;
use dashmap::DashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

// ============================================================================
// Static Debugger Instance (required for McpServerCapsule::new())
// ============================================================================

/// Static debugger for test server creation
/// SAFETY: Tests run sequentially, no concurrent debugger access
static DEBUGGER: OnceLock<DebuggerCapsule> = OnceLock::new();

fn get_debugger() -> &'static DebuggerCapsule {
    DEBUGGER.get_or_init(|| DebuggerCapsule::new(0))
}

// ============================================================================
// Test Server Helpers
// ============================================================================

/// Create a test MCP server with default configuration
pub fn create_test_server() -> McpServerCapsule {
    McpServerCapsule::new(get_debugger())
}

/// Create test server with custom license
pub fn create_test_server_with_license(license_key: &str) -> McpServerCapsule {
    let server = McpServerCapsule::new(get_debugger());

    // Set license on the server's license validator
    let expiry = 2000000000; // Year 2033
    server.license.set_license(license_key, expiry);

    server
}

/// Get debugger reference for handle_request() calls
pub fn get_test_debugger() -> &'static DebuggerCapsule {
    get_debugger()
}

// ============================================================================
// Mock Request Builders
// ============================================================================

/// Build a test JSON-RPC request
pub fn build_test_request(method: &str, params: serde_json::Value, id: i64) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": id
    }).to_string()
}

/// Build debugger attach request
pub fn build_attach_request(pid: u32, id: i64) -> String {
    build_test_request("debugger/attach", serde_json::json!({"pid": pid}), id)
}

/// Build breakpoint request
pub fn build_breakpoint_request(address: u64, id: i64) -> String {
    build_test_request(
        "debugger/set_breakpoint",
        serde_json::json!({"address": address}),
        id
    )
}

/// Build stack trace request
pub fn build_stack_trace_request(id: i64) -> String {
    build_test_request("debugger/get_stack_trace", serde_json::json!({}), id)
}

// ============================================================================
// Authentication Helpers
// ============================================================================

/// Generate test API key
pub fn generate_test_api_key() -> String {
    "test_api_key_1234567890abcdef".to_string()
}

/// Generate valid license key
pub fn generate_test_license() -> String {
    "KINDLY-PRO-test-license-key".to_string()
}

/// Generate test session token
pub fn generate_test_session_token() -> String {
    "test_session_token_abcdef123456".to_string()
}

// ============================================================================
// Timing Helpers
// ============================================================================

/// Measure operation latency
pub fn measure_latency<F, R>(f: F) -> (R, Duration)
where
    F: FnOnce() -> R,
{
    let start = Instant::now();
    let result = f();
    let latency = start.elapsed();
    (result, latency)
}

/// Assert latency within target
pub fn assert_latency_within(actual: Duration, target: Duration, operation: &str) {
    assert!(
        actual <= target,
        "{} latency too high: {:?} (target: {:?})",
        operation,
        actual,
        target
    );
}

// ============================================================================
// Concurrent Test Helpers
// ============================================================================

/// Run function concurrently across multiple threads
pub fn run_concurrent<F>(num_threads: usize, iterations_per_thread: usize, f: F)
where
    F: FnMut(usize, usize) + Send + Clone + 'static,
{
    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let mut f_clone = f.clone();
            std::thread::spawn(move || {
                for iteration in 0..iterations_per_thread {
                    f_clone(thread_id, iteration);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}

/// Run concurrent stress test and measure throughput
pub fn stress_test<F>(
    num_threads: usize,
    iterations_per_thread: usize,
    f: F,
) -> (Duration, f64)
where
    F: Fn(usize, usize) + Send + Sync + 'static,
{
    let f = Arc::new(f);
    let start = Instant::now();

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let f_clone = Arc::clone(&f);
            std::thread::spawn(move || {
                for iteration in 0..iterations_per_thread {
                    f_clone(thread_id, iteration);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let total_time = start.elapsed();
    let total_ops = (num_threads * iterations_per_thread) as f64;
    let throughput = total_ops / total_time.as_secs_f64();

    (total_time, throughput)
}

// ============================================================================
// Validation Helpers
// ============================================================================

/// Assert JSON-RPC success response
pub fn assert_success_response(response: &str) {
    let parsed: serde_json::Value = serde_json::from_str(response)
        .expect("Failed to parse JSON response");

    assert!(
        !parsed.get("error").is_some(),
        "Response should not contain error: {}",
        response
    );
    assert!(
        parsed.get("result").is_some(),
        "Response should contain result field"
    );
}

/// Assert JSON-RPC error response with code
pub fn assert_error_response(response: &str, expected_code: i32) {
    let parsed: serde_json::Value = serde_json::from_str(response)
        .expect("Failed to parse JSON response");

    assert!(
        parsed.get("error").is_some(),
        "Response should contain error field"
    );

    let error = parsed.get("error").unwrap();
    let code = error.get("code").and_then(|c| c.as_i64()).unwrap();

    assert_eq!(
        code as i32,
        expected_code,
        "Error code mismatch. Expected: {}, Got: {}",
        expected_code,
        code
    );
}

// ============================================================================
// Process Helpers (for debugger testing)
// ============================================================================

/// Get current process ID (for safe testing)
pub fn get_test_pid() -> u32 {
    std::process::id()
}

/// Spawn a test process that we can debug
#[cfg(target_os = "linux")]
pub fn spawn_debuggable_process() -> std::process::Child {
    std::process::Command::new("sleep")
        .arg("60")
        .spawn()
        .expect("Failed to spawn test process")
}

// ============================================================================
// Metrics Helpers
// ============================================================================

/// Extract metric value from server
pub fn get_server_metric(server: &McpServerCapsule, metric_name: &str) -> u64 {
    use core::sync::atomic::Ordering;

    match metric_name {
        "total_requests" => server.total_requests.load(Ordering::Relaxed),
        "successful_requests" => server.successful_requests.load(Ordering::Relaxed),
        "failed_requests" => server.failed_requests.load(Ordering::Relaxed),
        "avg_latency_ns" => server.avg_latency_ns.load(Ordering::Relaxed),
        "max_latency_ns" => server.max_latency_ns.load(Ordering::Relaxed),
        _ => panic!("Unknown metric: {}", metric_name),
    }
}

// ============================================================================
// Memory Helpers
// ============================================================================

/// Get current memory usage in bytes
#[cfg(target_os = "linux")]
pub fn get_memory_usage_bytes() -> usize {
    let status = std::fs::read_to_string("/proc/self/status")
        .expect("Failed to read /proc/self/status");

    for line in status.lines() {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb: usize = parts[1].parse().unwrap_or(0);
                return kb * 1024; // Convert KB to bytes
            }
        }
    }
    0
}

#[cfg(not(target_os = "linux"))]
pub fn get_memory_usage_bytes() -> usize {
    0 // Unsupported on non-Linux
}

/// Assert memory usage below threshold
pub fn assert_memory_below(threshold_bytes: usize, context: &str) {
    let actual = get_memory_usage_bytes();
    assert!(
        actual <= threshold_bytes || actual == 0,
        "{} memory usage too high: {} bytes (threshold: {} bytes)",
        context,
        actual,
        threshold_bytes
    );
}

// ============================================================================
// Rate Limiting Test Helpers
// ============================================================================

/// Create test rate limiter buckets (required for per-client rate limiter)
pub fn create_rate_limiter_buckets() -> Arc<DashMap<ClientId, ClientTokenBucket>> {
    Arc::new(DashMap::new())
}

/// Create test rate limiter with default config
pub fn create_test_rate_limiter() -> PerClientRateLimiterCapsule {
    PerClientRateLimiterCapsule::new(100 << 16, 200 << 16, 100)
}

/// Get current time in milliseconds
pub fn get_current_time_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Exhaust rate limit by making multiple requests
pub fn exhaust_rate_limit(
    limiter: &PerClientRateLimiterCapsule,
    buckets: &Arc<DashMap<ClientId, ClientTokenBucket>>,
    client_id: ClientId,
    limit: usize,
) {
    let now_ms = get_current_time_ms();
    for _ in 0..limit {
        let _ = limiter.check_rate_limit(buckets, client_id, now_ms, 1 << 16);
    }
}

// ============================================================================
// Feature Flag Helpers
// ============================================================================

/// Set feature flag for testing
#[cfg(feature = "feature-flags")]
pub fn set_test_feature_flag(_flag_name: &str, _enabled: bool) {
    // Implementation depends on FeatureFlagsCapsuleAPI
    // Placeholder for now
}

// ============================================================================
// Cleanup Helpers
// ============================================================================

/// Cleanup test processes
pub fn cleanup_test_processes(pids: Vec<u32>) {
    for pid in pids {
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("kill")
                .arg("-9")
                .arg(pid.to_string())
                .output();
        }
    }
}

// ============================================================================
// Test Fixtures
// ============================================================================

/// Standard test configuration
pub struct TestConfig {
    pub api_key: String,
    pub license_key: String,
    pub session_token: String,
    pub rate_limit: usize,
    pub quota_limit: usize,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            api_key: generate_test_api_key(),
            license_key: generate_test_license(),
            session_token: generate_test_session_token(),
            rate_limit: 100,
            quota_limit: 1000,
        }
    }
}

// ============================================================================
// Constants
// ============================================================================

/// Performance targets (from B32 validation)
pub const TARGET_END_TO_END_LATENCY_US: u64 = 10;
pub const TARGET_AUTH_OVERHEAD_NS: u64 = 500;
pub const TARGET_TOOL_DISPATCH_US: u64 = 1;
pub const TARGET_AUDIT_METRICS_NS: u64 = 100;
pub const TARGET_MEMORY_MB: usize = 512;

/// Test timeouts
pub const DEFAULT_TEST_TIMEOUT_SECS: u64 = 30;
pub const STRESS_TEST_TIMEOUT_SECS: u64 = 120;

/// Concurrent test parameters
pub const DEFAULT_THREAD_COUNT: usize = 10;
pub const DEFAULT_ITERATIONS_PER_THREAD: usize = 100;
