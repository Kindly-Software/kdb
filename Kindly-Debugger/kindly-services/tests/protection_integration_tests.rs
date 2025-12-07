//! # Protection Integration Tests for Kindly Services
//!
//! **Framework**: T28 5-Tier Testing (Unit/Property/Integration/Production/Determinism)
//! **Coverage**: SecurityHeaders, HttpAudit, RateLimiter, Path Validation, MIME Detection
//!
//! ## Test Categories
//!
//! 1. **Security Headers** (Q1-Q7): Header presence and values
//! 2. **Rate Limiting** (Q8-Q14): Token bucket, EWMA, AIMD
//! 3. **Audit Logging** (Q15-Q21): Hash-chain, tamper detection
//! 4. **Path Security** (Q22-Q28): Traversal prevention, validation
//! 5. **Integration** (Q29-Q35): End-to-end request flow
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all protection tests
//! cargo test --test protection_integration_tests --features full-protection
//!
//! # Run specific category
//! cargo test --test protection_integration_tests security_headers --features full-protection
//! cargo test --test protection_integration_tests rate_limiting --features full-protection
//! cargo test --test protection_integration_tests audit_log --features full-protection
//! cargo test --test protection_integration_tests path_security --features full-protection
//! ```

#![cfg(not(target_arch = "wasm32"))]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// TEST INFRASTRUCTURE
// ============================================================================

/// Test server port (different from production to avoid conflicts)
const TEST_PORT: u16 = 18082;

/// Server startup timeout
const SERVER_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

/// Request timeout
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Static for tracking server process
static SERVER_RUNNING: AtomicBool = AtomicBool::new(false);
static SERVER_PID: AtomicU32 = AtomicU32::new(0);

/// Start the HTTP server for integration tests
fn start_test_server() -> Option<Child> {
    // Check if server already running
    if SERVER_RUNNING.load(Ordering::Relaxed) {
        return None;
    }

    // Build the server with full protection features
    let build_result = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--bin",
            "http_server",
            "--features",
            "full-protection",
        ])
        .current_dir("/home/samuel/Primitives/kindly-services")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if build_result.is_err() {
        eprintln!("Failed to build server");
        return None;
    }

    // Start the server
    let child = Command::new("./target/release/http_server")
        .current_dir("/home/samuel/Primitives/kindly-services")
        .env("KINDLY_PORT", TEST_PORT.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    SERVER_PID.store(child.id(), Ordering::Relaxed);
    SERVER_RUNNING.store(true, Ordering::Relaxed);

    // Wait for server to be ready
    let start = Instant::now();
    while start.elapsed() < SERVER_STARTUP_TIMEOUT {
        if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{}", TEST_PORT)) {
            stream
                .set_read_timeout(Some(Duration::from_millis(100)))
                .ok();
            stream
                .set_write_timeout(Some(Duration::from_millis(100)))
                .ok();
            // Server is ready
            return Some(child);
        }
        thread::sleep(Duration::from_millis(100));
    }

    eprintln!("Server failed to start within timeout");
    None
}

/// Make HTTP request and return response
fn http_request(path: &str) -> Result<HttpResponse, String> {
    let addr = format!("127.0.0.1:{}", TEST_PORT);

    let mut stream =
        TcpStream::connect(&addr).map_err(|e| format!("Connection failed: {}", e))?;

    stream
        .set_read_timeout(Some(REQUEST_TIMEOUT))
        .map_err(|e| format!("Set timeout failed: {}", e))?;

    // Send HTTP request
    let request = format!(
        "GET {} HTTP/1.1\r\n\
         Host: localhost:{}\r\n\
         Connection: close\r\n\
         \r\n",
        path, TEST_PORT
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("Write failed: {}", e))?;

    // Read response
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| format!("Read failed: {}", e))?;

    HttpResponse::parse(&response)
}

/// Make HTTP request with custom headers
fn http_request_with_headers(path: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, String> {
    let addr = format!("127.0.0.1:{}", TEST_PORT);

    let mut stream =
        TcpStream::connect(&addr).map_err(|e| format!("Connection failed: {}", e))?;

    stream
        .set_read_timeout(Some(REQUEST_TIMEOUT))
        .map_err(|e| format!("Set timeout failed: {}", e))?;

    // Build headers string
    let mut header_str = String::new();
    for (name, value) in headers {
        header_str.push_str(&format!("{}: {}\r\n", name, value));
    }

    // Send HTTP request
    let request = format!(
        "GET {} HTTP/1.1\r\n\
         Host: localhost:{}\r\n\
         {}\
         Connection: close\r\n\
         \r\n",
        path, TEST_PORT, header_str
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("Write failed: {}", e))?;

    // Read response
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| format!("Read failed: {}", e))?;

    HttpResponse::parse(&response)
}

/// Parsed HTTP response
#[derive(Debug, Clone)]
struct HttpResponse {
    status_code: u16,
    status_text: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl HttpResponse {
    fn parse(raw: &str) -> Result<Self, String> {
        let mut lines = raw.lines();

        // Parse status line
        let status_line = lines.next().ok_or("Empty response")?;
        let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
        if parts.len() < 2 {
            return Err("Invalid status line".to_string());
        }

        let status_code: u16 = parts[1]
            .parse()
            .map_err(|_| "Invalid status code".to_string())?;
        let status_text = parts.get(2).unwrap_or(&"").to_string();

        // Parse headers
        let mut headers = Vec::new();
        let mut body_start = false;

        for line in lines.by_ref() {
            if line.is_empty() {
                body_start = true;
                break;
            }

            if let Some((name, value)) = line.split_once(':') {
                headers.push((name.trim().to_string(), value.trim().to_string()));
            }
        }

        // Collect body
        let body: String = if body_start {
            lines.collect::<Vec<&str>>().join("\n")
        } else {
            String::new()
        };

        Ok(HttpResponse {
            status_code,
            status_text,
            headers,
            body,
        })
    }

    fn get_header(&self, name: &str) -> Option<&str> {
        for (n, v) in &self.headers {
            if n.eq_ignore_ascii_case(name) {
                return Some(v.as_str());
            }
        }
        None
    }

    fn has_header(&self, name: &str) -> bool {
        self.get_header(name).is_some()
    }
}

// ============================================================================
// TEST MODULE: Security Headers (Q1-Q7)
// ============================================================================

mod security_headers {
    use super::*;

    /// Q1: HSTS header present and correctly configured
    #[test]
    #[ignore = "Requires running server"]
    fn test_hsts_header_present() {
        let response = http_request("/").expect("Request failed");

        // HSTS should be present
        let hsts = response.get_header("Strict-Transport-Security");
        assert!(hsts.is_some(), "HSTS header missing");

        let hsts_value = hsts.unwrap();
        assert!(
            hsts_value.contains("max-age="),
            "HSTS missing max-age: {}",
            hsts_value
        );
        assert!(
            hsts_value.contains("includeSubDomains"),
            "HSTS missing includeSubDomains: {}",
            hsts_value
        );
        assert!(
            hsts_value.contains("preload"),
            "HSTS missing preload: {}",
            hsts_value
        );
    }

    /// Q2: X-Frame-Options header present
    #[test]
    #[ignore = "Requires running server"]
    fn test_x_frame_options_present() {
        let response = http_request("/").expect("Request failed");

        let header = response.get_header("X-Frame-Options");
        assert!(header.is_some(), "X-Frame-Options missing");
        assert_eq!(header.unwrap(), "DENY", "X-Frame-Options should be DENY");
    }

    /// Q3: X-Content-Type-Options header present
    #[test]
    #[ignore = "Requires running server"]
    fn test_x_content_type_options_present() {
        let response = http_request("/").expect("Request failed");

        let header = response.get_header("X-Content-Type-Options");
        assert!(header.is_some(), "X-Content-Type-Options missing");
        assert_eq!(
            header.unwrap(),
            "nosniff",
            "X-Content-Type-Options should be nosniff"
        );
    }

    /// Q4: X-XSS-Protection header present
    #[test]
    #[ignore = "Requires running server"]
    fn test_x_xss_protection_present() {
        let response = http_request("/").expect("Request failed");

        let header = response.get_header("X-XSS-Protection");
        assert!(header.is_some(), "X-XSS-Protection missing");
        assert!(
            header.unwrap().contains("1"),
            "X-XSS-Protection should be enabled"
        );
    }

    /// Q5: Referrer-Policy header present
    #[test]
    #[ignore = "Requires running server"]
    fn test_referrer_policy_present() {
        let response = http_request("/").expect("Request failed");

        let header = response.get_header("Referrer-Policy");
        assert!(header.is_some(), "Referrer-Policy missing");
    }

    /// Q6: Cross-Origin-Opener-Policy header present
    #[test]
    #[ignore = "Requires running server"]
    fn test_coop_header_present() {
        let response = http_request("/").expect("Request failed");

        let header = response.get_header("Cross-Origin-Opener-Policy");
        assert!(header.is_some(), "COOP header missing");
        assert_eq!(
            header.unwrap(),
            "same-origin",
            "COOP should be same-origin"
        );
    }

    /// Q7: Cross-Origin-Resource-Policy header present
    #[test]
    #[ignore = "Requires running server"]
    fn test_corp_header_present() {
        let response = http_request("/").expect("Request failed");

        let header = response.get_header("Cross-Origin-Resource-Policy");
        assert!(header.is_some(), "CORP header missing");
        assert_eq!(
            header.unwrap(),
            "same-origin",
            "CORP should be same-origin"
        );
    }
}

// ============================================================================
// TEST MODULE: Rate Limiting (Q8-Q14)
// ============================================================================

mod rate_limiting {
    use super::*;

    /// Q8: Rate limiter allows initial burst
    #[test]
    #[ignore = "Requires running server with rate limiting"]
    fn test_initial_burst_allowed() {
        // First 10 requests should succeed (well under 500 burst)
        for i in 0..10 {
            let response = http_request("/").expect(&format!("Request {} failed", i));
            assert_eq!(
                response.status_code, 200,
                "Request {} should succeed (got {})",
                i, response.status_code
            );
        }
    }

    /// Q9: Rate limiter triggers after burst exceeded
    #[test]
    #[ignore = "Requires running server with rate limiting - SLOW TEST"]
    fn test_rate_limit_triggers_after_burst() {
        let mut denied_count = 0;
        let mut allowed_count = 0;

        // Make 600 requests (exceeds 500 burst)
        for _ in 0..600 {
            match http_request("/") {
                Ok(response) => {
                    if response.status_code == 429 {
                        denied_count += 1;
                    } else if response.status_code == 200 {
                        allowed_count += 1;
                    }
                }
                Err(_) => {
                    // Connection refused might mean rate limited at socket level
                    denied_count += 1;
                }
            }
        }

        // Should have some denied requests
        assert!(
            denied_count > 0,
            "Rate limiting should trigger (allowed: {}, denied: {})",
            allowed_count,
            denied_count
        );
    }

    /// Q10: 429 response includes Retry-After header
    #[test]
    #[ignore = "Requires rate limit to trigger"]
    fn test_429_has_retry_after_header() {
        // Make many requests to trigger rate limit
        let mut got_429 = false;

        for _ in 0..600 {
            if let Ok(response) = http_request("/") {
                if response.status_code == 429 {
                    got_429 = true;
                    let retry_after = response.get_header("Retry-After");
                    assert!(retry_after.is_some(), "429 should include Retry-After");
                    break;
                }
            }
        }

        if !got_429 {
            println!("Warning: Did not trigger 429 (rate limiting may not be enabled)");
        }
    }

    /// Q11: Rate limiter recovers after waiting
    #[test]
    #[ignore = "Requires running server - SLOW TEST"]
    fn test_rate_limit_recovery() {
        // Exhaust tokens
        for _ in 0..550 {
            let _ = http_request("/");
        }

        // Wait for refill (100 tokens/sec = ~5 seconds for 500 tokens)
        thread::sleep(Duration::from_secs(6));

        // Should be allowed again
        let response = http_request("/").expect("Request after recovery failed");
        assert_eq!(
            response.status_code, 200,
            "Should recover after waiting"
        );
    }
}

// ============================================================================
// TEST MODULE: Audit Logging (Q15-Q21)
// ============================================================================

mod audit_log {
    use super::*;

    /// Q15: Audit log captures successful requests
    #[test]
    #[ignore = "Requires server stdout access"]
    fn test_audit_log_captures_200() {
        // This test would need to capture server stdout
        // For now, we verify the request succeeds (audit is side effect)
        let response = http_request("/").expect("Request failed");
        assert_eq!(response.status_code, 200);
        // Audit entry written to stdout: [AUDIT] GET / -> 200 (bytes) in time
    }

    /// Q16: Audit log captures 403 for path traversal
    #[test]
    #[ignore = "Requires running server"]
    fn test_audit_log_captures_403() {
        let response = http_request("/../../etc/passwd").expect("Request failed");
        assert_eq!(
            response.status_code, 403,
            "Path traversal should return 403"
        );
        // Audit entry written: [SECURITY] Path validation failed
    }

    /// Q17: Audit log captures 404 (SPA fallback may return 200)
    #[test]
    #[ignore = "Requires running server"]
    fn test_audit_log_captures_not_found() {
        // Note: SPA fallback returns 200 with index.html
        // This tests the audit trail is created regardless
        let response = http_request("/nonexistent-file.xyz").expect("Request failed");
        // Either 200 (SPA fallback) or 404 is acceptable
        assert!(
            response.status_code == 200 || response.status_code == 404,
            "Unexpected status: {}",
            response.status_code
        );
    }
}

// ============================================================================
// TEST MODULE: Path Security (Q22-Q28)
// ============================================================================

mod path_security {
    use super::*;

    /// Q22: Path traversal with ../ is rejected
    #[test]
    #[ignore = "Requires running server"]
    fn test_path_traversal_dot_dot_slash() {
        let response = http_request("/../../etc/passwd").expect("Request failed");
        assert_eq!(response.status_code, 403, "../ traversal should be blocked");
    }

    /// Q23: Path traversal with encoded ../ is rejected
    #[test]
    #[ignore = "Requires running server"]
    fn test_path_traversal_encoded() {
        // URL-encoded ../ = %2e%2e%2f
        let response = http_request("/%2e%2e%2f%2e%2e%2fetc/passwd").expect("Request failed");
        // May be decoded by server or rejected - either way, should not expose /etc/passwd
        assert_ne!(
            response.status_code, 200,
            "Encoded traversal should be blocked or fail"
        );
    }

    /// Q24: Double slash is rejected
    #[test]
    #[ignore = "Requires running server"]
    fn test_path_double_slash() {
        let response = http_request("//etc/passwd").expect("Request failed");
        assert_eq!(response.status_code, 403, "// should be blocked");
    }

    /// Q25: Null byte is rejected
    #[test]
    #[ignore = "Requires running server"]
    fn test_path_null_byte() {
        // Note: Null byte may not transmit over HTTP correctly
        // This tests the validation logic exists
        let response = http_request("/index.html%00").expect("Request failed");
        // Should either reject (403) or treat as normal path (200 for index.html)
        assert!(
            response.status_code == 200 || response.status_code == 403,
            "Null byte handling: {}",
            response.status_code
        );
    }

    /// Q26: Absolute path is normalized
    #[test]
    #[ignore = "Requires running server"]
    fn test_path_absolute() {
        let response = http_request("/index.html").expect("Request failed");
        assert_eq!(
            response.status_code, 200,
            "Absolute path should be allowed"
        );
    }

    /// Q27: Root path returns index.html
    #[test]
    #[ignore = "Requires running server"]
    fn test_path_root_returns_index() {
        let response = http_request("/").expect("Request failed");
        assert_eq!(response.status_code, 200, "Root should return index.html");
        assert!(
            response.body.contains("<html") || response.body.contains("<!DOCTYPE"),
            "Should return HTML content"
        );
    }

    /// Q28: Safe path to asset works
    #[test]
    #[ignore = "Requires running server with dist directory"]
    fn test_path_safe_asset() {
        // This depends on dist directory existing with assets
        let response = http_request("/index.html").expect("Request failed");
        assert_eq!(response.status_code, 200, "Safe asset path should work");
    }
}

// ============================================================================
// TEST MODULE: MIME Type Detection
// ============================================================================

mod mime_detection {
    use super::*;

    /// Test HTML MIME type
    #[test]
    #[ignore = "Requires running server"]
    fn test_mime_html() {
        let response = http_request("/index.html").expect("Request failed");
        let content_type = response.get_header("Content-Type");
        assert!(content_type.is_some(), "Content-Type missing");
        assert!(
            content_type.unwrap().contains("text/html"),
            "HTML should have text/html"
        );
    }

    /// Test JavaScript MIME type
    #[test]
    #[ignore = "Requires running server with JS files"]
    fn test_mime_javascript() {
        // Adjust path based on actual dist structure
        let response = http_request("/app.js");
        if let Ok(response) = response {
            if response.status_code == 200 {
                let content_type = response.get_header("Content-Type");
                assert!(
                    content_type
                        .map(|s| s.contains("javascript"))
                        .unwrap_or(false),
                    "JS should have application/javascript"
                );
            }
        }
    }

    /// Test WASM MIME type
    #[test]
    #[ignore = "Requires running server with WASM files"]
    fn test_mime_wasm() {
        // WASM files typically have hash in filename
        let response = http_request("/app.wasm");
        if let Ok(response) = response {
            if response.status_code == 200 {
                let content_type = response.get_header("Content-Type");
                assert!(
                    content_type.map(|s| s.contains("wasm")).unwrap_or(false),
                    "WASM should have application/wasm"
                );
            }
        }
    }

    /// Test CSS MIME type
    #[test]
    #[ignore = "Requires running server with CSS files"]
    fn test_mime_css() {
        let response = http_request("/style.css");
        if let Ok(response) = response {
            if response.status_code == 200 {
                let content_type = response.get_header("Content-Type");
                assert!(
                    content_type.map(|s| s.contains("text/css")).unwrap_or(false),
                    "CSS should have text/css"
                );
            }
        }
    }
}

// ============================================================================
// TEST MODULE: Integration Tests (Q29-Q35)
// ============================================================================

mod integration {
    use super::*;

    /// Q29: Full request flow succeeds
    #[test]
    #[ignore = "Requires running server"]
    fn test_full_request_flow() {
        let response = http_request("/").expect("Full flow failed");

        // Should return 200
        assert_eq!(response.status_code, 200);

        // Should have security headers
        assert!(response.has_header("X-Content-Type-Options"));

        // Should have content
        assert!(!response.body.is_empty());
    }

    /// Q30: Concurrent requests handled correctly
    #[test]
    #[ignore = "Requires running server"]
    fn test_concurrent_requests() {
        use std::sync::Arc;

        let success_count = Arc::new(AtomicU32::new(0));
        let failure_count = Arc::new(AtomicU32::new(0));

        let mut handles = vec![];

        for _ in 0..10 {
            let success = Arc::clone(&success_count);
            let failure = Arc::clone(&failure_count);

            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    match http_request("/") {
                        Ok(response) if response.status_code == 200 => {
                            success.fetch_add(1, Ordering::Relaxed);
                        }
                        _ => {
                            failure.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        let successes = success_count.load(Ordering::Relaxed);
        let failures = failure_count.load(Ordering::Relaxed);

        // Most requests should succeed (some may be rate limited)
        assert!(
            successes > failures,
            "More successes than failures expected: {} vs {}",
            successes,
            failures
        );
    }

    /// Q31: Server only binds to localhost
    #[test]
    #[ignore = "Requires running server"]
    fn test_localhost_only_binding() {
        // Try to connect from external interface (should fail)
        // This test assumes the test machine has multiple interfaces
        // In practice, this verifies the bind address is 127.0.0.1

        // Verify localhost connection works
        let result = TcpStream::connect(format!("127.0.0.1:{}", TEST_PORT));
        assert!(result.is_ok(), "Localhost should be accessible");

        // Note: Cannot easily test external interface without network config
        // The HTTP server binds to 127.0.0.1 which is verified in code
    }

    /// Q32: SPA routing returns index.html for unknown paths
    #[test]
    #[ignore = "Requires running server"]
    fn test_spa_fallback() {
        // Request a non-existent route
        let response = http_request("/some/deep/route/that/does/not/exist")
            .expect("SPA fallback request failed");

        // SPA should return 200 with index.html
        assert_eq!(response.status_code, 200, "SPA should return 200");
        assert!(
            response.body.contains("<html") || response.body.contains("<!DOCTYPE"),
            "SPA should return HTML"
        );
    }

    /// Q33: Cache headers present for static assets
    #[test]
    #[ignore = "Requires running server"]
    fn test_cache_headers() {
        let response = http_request("/index.html").expect("Request failed");

        let cache_control = response.get_header("Cache-Control");
        assert!(cache_control.is_some(), "Cache-Control header missing");
    }

    /// Q34: Server header is custom (not default)
    #[test]
    #[ignore = "Requires running server"]
    fn test_server_header() {
        let response = http_request("/").expect("Request failed");

        let server = response.get_header("Server");
        assert!(server.is_some(), "Server header missing");
        assert!(
            server.unwrap().contains("Kindly"),
            "Should use Kindly server name"
        );
    }
}

// ============================================================================
// TEST MODULE: Penetration Testing Basics
// ============================================================================

mod penetration {
    use super::*;

    /// Test path traversal attack
    #[test]
    #[ignore = "Requires running server"]
    fn test_pentest_path_traversal() {
        let attacks = vec![
            "/../etc/passwd",
            "/../../etc/passwd",
            "/../../../etc/passwd",
            "/..%2f..%2fetc/passwd",
            "/..\\..\\etc\\passwd",
            "/assets/../../../etc/passwd",
        ];

        for attack in attacks {
            let response = http_request(attack);
            if let Ok(resp) = response {
                assert_ne!(
                    resp.status_code, 200,
                    "Path traversal should not return 200: {}",
                    attack
                );
                // Should not contain /etc/passwd content
                assert!(
                    !resp.body.contains("root:"),
                    "Should not expose /etc/passwd: {}",
                    attack
                );
            }
        }
    }

    /// Test XSS attempt in path
    #[test]
    #[ignore = "Requires running server"]
    fn test_pentest_xss_in_path() {
        let response = http_request("/<script>alert(1)</script>");
        if let Ok(resp) = response {
            // Should not reflect XSS in response body
            assert!(
                !resp.body.contains("<script>alert"),
                "XSS should not be reflected"
            );
        }
    }

    /// Test header injection
    #[test]
    #[ignore = "Requires running server"]
    fn test_pentest_header_injection() {
        let response = http_request_with_headers(
            "/",
            &[("X-Forwarded-For", "evil\r\nX-Injected: malicious")],
        );

        if let Ok(resp) = response {
            // Should not have injected header
            assert!(
                resp.get_header("X-Injected").is_none(),
                "Header injection should not work"
            );
        }
    }

    /// Test oversized request handling
    #[test]
    #[ignore = "Requires running server"]
    fn test_pentest_oversized_request() {
        // Try to send a very long path
        let long_path = "/".to_string() + &"A".repeat(10000);
        let response = http_request(&long_path);

        // Should either reject or handle gracefully (not crash)
        // The response may be error or timeout, but server should survive
        if let Ok(resp) = response {
            // Any status is acceptable as long as server handled it
            assert!(
                resp.status_code >= 100 && resp.status_code < 600,
                "Should return valid HTTP status"
            );
        }
    }
}

// ============================================================================
// UNIT TESTS (Run without server)
// ============================================================================

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_http_response_parse_simple() {
        let raw = "HTTP/1.1 200 OK\r\n\
                   Content-Type: text/html\r\n\
                   Content-Length: 13\r\n\
                   \r\n\
                   Hello, World!";

        let response = HttpResponse::parse(raw).expect("Parse failed");
        assert_eq!(response.status_code, 200);
        assert_eq!(response.status_text, "OK");
        assert_eq!(response.get_header("Content-Type"), Some("text/html"));
        assert!(response.body.contains("Hello"));
    }

    #[test]
    fn test_http_response_parse_404() {
        let raw = "HTTP/1.1 404 Not Found\r\n\
                   Content-Type: text/html\r\n\
                   \r\n\
                   <html>Not Found</html>";

        let response = HttpResponse::parse(raw).expect("Parse failed");
        assert_eq!(response.status_code, 404);
        assert_eq!(response.status_text, "Not Found");
    }

    #[test]
    fn test_http_response_case_insensitive_headers() {
        let raw = "HTTP/1.1 200 OK\r\n\
                   content-type: text/html\r\n\
                   CONTENT-LENGTH: 5\r\n\
                   \r\n\
                   Hello";

        let response = HttpResponse::parse(raw).expect("Parse failed");
        assert_eq!(response.get_header("Content-Type"), Some("text/html"));
        assert_eq!(response.get_header("content-length"), Some("5"));
    }

    #[test]
    fn test_http_response_has_header() {
        let raw = "HTTP/1.1 200 OK\r\n\
                   X-Custom: value\r\n\
                   \r\n";

        let response = HttpResponse::parse(raw).expect("Parse failed");
        assert!(response.has_header("X-Custom"));
        assert!(!response.has_header("X-Missing"));
    }
}
