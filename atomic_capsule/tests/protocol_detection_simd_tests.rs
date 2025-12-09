// Protocol Detection SIMD Tests (T28 Compliance: 28 tests)
//
// Framework Compliance:
// - T28 Q1-Q7: Unit tests (basic SIMD functionality)
// - T28 Q8-Q14: Property tests (random inputs, alignment)
// - T28 Q15-Q21: Integration tests (full routing flow)
// - T28 Q22-Q28: Production tests (stress testing, CPU detection)
//
// Feature: nightly-simd-protocol
// Target: 5-10× speedup (<40ns vs ~100-200ns scalar)

#![cfg(all(test, feature = "nightly-simd-protocol"))]

use atomic_capsule::meta::universal_api::{UniversalApiMetaCapsule, UniversalRequest, ProtocolType};

// ============================================================================
// Mock Request (Test Harness)
// ============================================================================

struct MockRequest {
    headers: Vec<(&'static str, &'static str)>,
    method: &'static str,
    path: &'static str,
    body: Vec<u8>,
}

impl MockRequest {
    fn new(method: &'static str, path: &'static str) -> Self {
        Self {
            headers: Vec::new(),
            method,
            path,
            body: Vec::new(),
        }
    }

    fn with_header(mut self, name: &'static str, value: &'static str) -> Self {
        self.headers.push((name, value));
        self
    }

    fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }
}

impl UniversalRequest for MockRequest {
    fn method(&self) -> &str { self.method }
    fn path(&self) -> &str { self.path }
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| *v)
    }
    fn body(&self) -> &[u8] { &self.body }
    fn protocol(&self) -> ProtocolType { ProtocolType::REST }
}

// ============================================================================
// T28 Q1-Q7: Unit Tests (Basic SIMD Functionality)
// ============================================================================

#[test]
fn test_q1_simd_rest_get() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("GET", "/api/users");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::REST);
}

#[test]
fn test_q2_simd_rest_post() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("POST", "/api/users")
        .with_header("Content-Type", "application/json");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::REST);
}

#[test]
fn test_q3_simd_graphql_header() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("POST", "/graphql")
        .with_header("Content-Type", "application/graphql");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::GraphQL);
}

#[test]
fn test_q4_simd_grpc_content_type() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("POST", "/grpc.Service/Method")
        .with_header("Content-Type", "application/grpc");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::Grpc);
}

#[test]
fn test_q5_simd_websocket_upgrade() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("GET", "/ws")
        .with_header("Upgrade", "websocket")
        .with_header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::WebSocket);
}

#[test]
fn test_q6_simd_jsonrpc_header() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("POST", "/rpc")
        .with_header("Content-Type", "application/json-rpc");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::JsonRPC);
}

#[test]
fn test_q7_simd_jsonrpc_body_prefix() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("POST", "/rpc")
        .with_header("Content-Type", "application/json")
        .with_body(b"{\"jsonrpc\":\"2.0\",\"method\":\"sum\",\"params\":[1,2]}".to_vec());

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::JsonRPC);
}

// ============================================================================
// T28 Q8-Q14: Property Tests (Alignment, Random Inputs, Edge Cases)
// ============================================================================

#[test]
fn test_q8_simd_short_method_less_than_32_bytes() {
    let capsule = UniversalApiMetaCapsule::new();

    // Method < 32 bytes (should not crash, should detect correctly)
    let request = MockRequest::new("GET", "/");
    assert_eq!(capsule.detect_protocol(&request), ProtocolType::REST);

    let request = MockRequest::new("PUT", "/");
    assert_eq!(capsule.detect_protocol(&request), ProtocolType::REST);
}

#[test]
fn test_q9_simd_short_header_less_than_32_bytes() {
    let capsule = UniversalApiMetaCapsule::new();

    // Content-Type < 32 bytes
    let request = MockRequest::new("POST", "/api")
        .with_header("Content-Type", "app/grpc");  // Short version

    // Should still detect (SIMD pads with zeros)
    let protocol = capsule.detect_protocol(&request);
    // Note: "app/grpc" doesn't match "application/grpc", so defaults to REST
    assert_eq!(protocol, ProtocolType::REST);
}

#[test]
fn test_q10_simd_empty_body() {
    let capsule = UniversalApiMetaCapsule::new();

    let request = MockRequest::new("POST", "/rpc")
        .with_body(vec![]);

    // Empty body should not crash
    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::REST);  // Default
}

#[test]
fn test_q11_simd_long_body_over_32_bytes() {
    let capsule = UniversalApiMetaCapsule::new();

    let long_body = b"{\"jsonrpc\":\"2.0\",\"method\":\"very_long_method_name_that_exceeds_32_bytes\"}".to_vec();
    let request = MockRequest::new("POST", "/rpc")
        .with_body(long_body);

    // Should detect JSON-RPC from first 32 bytes
    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::JsonRPC);
}

#[test]
fn test_q12_simd_unaligned_input() {
    let capsule = UniversalApiMetaCapsule::new();

    // Unaligned string (not necessarily 32-byte aligned in memory)
    let unaligned_method = String::from("POST");
    let request = MockRequest::new(Box::leak(unaligned_method.into_boxed_str()), "/api");

    // SIMD should handle unaligned loads gracefully (portable_simd uses from_slice)
    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::REST);
}

#[test]
fn test_q13_simd_all_protocol_types() {
    let capsule = UniversalApiMetaCapsule::new();

    // Test all 5 protocol types
    let protocols = vec![
        (MockRequest::new("GET", "/api"), ProtocolType::REST),
        (MockRequest::new("POST", "/graphql").with_header("Content-Type", "application/graphql"), ProtocolType::GraphQL),
        (MockRequest::new("POST", "/grpc").with_header("Content-Type", "application/grpc"), ProtocolType::Grpc),
        (MockRequest::new("GET", "/ws").with_header("Upgrade", "websocket"), ProtocolType::WebSocket),
        (MockRequest::new("POST", "/rpc").with_header("Content-Type", "application/json-rpc"), ProtocolType::JsonRPC),
    ];

    for (request, expected) in protocols {
        assert_eq!(capsule.detect_protocol(&request), expected);
    }
}

#[test]
fn test_q14_simd_case_insensitive_headers() {
    let capsule = UniversalApiMetaCapsule::new();

    // HTTP headers are case-insensitive
    let request = MockRequest::new("GET", "/ws")
        .with_header("upgrade", "websocket");  // Lowercase

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::WebSocket);
}

// ============================================================================
// T28 Q15-Q21: Integration Tests (Full Routing Flow)
// ============================================================================

#[test]
fn test_q15_simd_rest_with_content_type() {
    let capsule = UniversalApiMetaCapsule::new();

    // REST with various Content-Types
    let requests = vec![
        MockRequest::new("POST", "/api").with_header("Content-Type", "application/json"),
        MockRequest::new("POST", "/api").with_header("Content-Type", "application/xml"),
        MockRequest::new("POST", "/api").with_header("Content-Type", "text/plain"),
    ];

    for request in requests {
        assert_eq!(capsule.detect_protocol(&request), ProtocolType::REST);
    }
}

#[test]
fn test_q16_simd_grpc_with_headers() {
    let capsule = UniversalApiMetaCapsule::new();

    // gRPC with grpc-encoding header
    let request = MockRequest::new("POST", "/grpc.Service/Method")
        .with_header("grpc-encoding", "gzip");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::Grpc);

    // gRPC with grpc-timeout header
    let request2 = MockRequest::new("POST", "/grpc.Service/Method")
        .with_header("grpc-timeout", "10s");

    let protocol2 = capsule.detect_protocol(&request2);
    assert_eq!(protocol2, ProtocolType::Grpc);
}

#[test]
fn test_q17_simd_websocket_with_sec_key() {
    let capsule = UniversalApiMetaCapsule::new();

    // WebSocket with only Sec-WebSocket-Key (no Upgrade header)
    let request = MockRequest::new("GET", "/ws")
        .with_header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::WebSocket);
}

#[test]
fn test_q18_simd_jsonrpc_fallback_to_body() {
    let capsule = UniversalApiMetaCapsule::new();

    // JSON-RPC without Content-Type header (detect from body)
    let request = MockRequest::new("POST", "/rpc")
        .with_body(b"{\"jsonrpc\":\"2.0\",\"id\":1}".to_vec());

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::JsonRPC);
}

#[test]
fn test_q19_simd_rest_default_for_unknown() {
    let capsule = UniversalApiMetaCapsule::new();

    // Unknown protocol defaults to REST
    let request = MockRequest::new("POST", "/api")
        .with_header("Content-Type", "application/unknown");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::REST);
}

#[test]
fn test_q20_simd_graphql_over_rest() {
    let capsule = UniversalApiMetaCapsule::new();

    // POST with application/graphql takes precedence over REST
    let request = MockRequest::new("POST", "/api")
        .with_header("Content-Type", "application/graphql");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::GraphQL);
}

#[test]
fn test_q21_simd_concurrent_detection() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(UniversalApiMetaCapsule::new());

    // Spawn 10 threads concurrently
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let capsule = Arc::clone(&capsule);
            thread::spawn(move || {
                let request = if i % 5 == 0 {
                    MockRequest::new("GET", "/api")
                } else if i % 5 == 1 {
                    MockRequest::new("POST", "/graphql").with_header("Content-Type", "application/graphql")
                } else if i % 5 == 2 {
                    MockRequest::new("POST", "/grpc").with_header("Content-Type", "application/grpc")
                } else if i % 5 == 3 {
                    MockRequest::new("GET", "/ws").with_header("Upgrade", "websocket")
                } else {
                    MockRequest::new("POST", "/rpc").with_header("Content-Type", "application/json-rpc")
                };

                capsule.detect_protocol(&request)
            })
        })
        .collect();

    // Wait for all threads and verify results
    for (i, handle) in handles.into_iter().enumerate() {
        let protocol = handle.join().unwrap();
        let expected = match i % 5 {
            0 => ProtocolType::REST,
            1 => ProtocolType::GraphQL,
            2 => ProtocolType::Grpc,
            3 => ProtocolType::WebSocket,
            4 => ProtocolType::JsonRPC,
            _ => unreachable!(),
        };
        assert_eq!(protocol, expected);
    }
}

// ============================================================================
// T28 Q22-Q28: Production Tests (Stress Testing, CPU Detection, Limits)
// ============================================================================

#[test]
fn test_q22_simd_stress_1000_requests() {
    let capsule = UniversalApiMetaCapsule::new();

    // Stress test with 1000 requests
    for i in 0..1000 {
        let request = match i % 5 {
            0 => MockRequest::new("GET", "/api"),
            1 => MockRequest::new("POST", "/graphql").with_header("Content-Type", "application/graphql"),
            2 => MockRequest::new("POST", "/grpc").with_header("Content-Type", "application/grpc"),
            3 => MockRequest::new("GET", "/ws").with_header("Upgrade", "websocket"),
            4 => MockRequest::new("POST", "/rpc").with_header("Content-Type", "application/json-rpc"),
            _ => unreachable!(),
        };

        let protocol = capsule.detect_protocol(&request);

        let expected = match i % 5 {
            0 => ProtocolType::REST,
            1 => ProtocolType::GraphQL,
            2 => ProtocolType::Grpc,
            3 => ProtocolType::WebSocket,
            4 => ProtocolType::JsonRPC,
            _ => unreachable!(),
        };

        assert_eq!(protocol, expected, "Failed at iteration {}", i);
    }
}

#[test]
#[cfg(target_arch = "x86_64")]
fn test_q23_simd_cpu_feature_detection() {
    // Verify AVX2 detection works correctly
    let has_avx2 = is_x86_feature_detected!("avx2");

    // This test just confirms CPU detection runs without panic
    println!("AVX2 available: {}", has_avx2);

    // If AVX2 is available, SIMD path should be used
    // If not, scalar fallback should be used
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("GET", "/api");
    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::REST);
}

#[test]
fn test_q24_simd_maximum_body_size() {
    let capsule = UniversalApiMetaCapsule::new();

    // Very large body (1MB)
    let large_body = vec![b'x'; 1_000_000];
    let request = MockRequest::new("POST", "/api")
        .with_body(large_body);

    // Should only read first 32 bytes (no performance penalty for large bodies)
    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::REST);
}

#[test]
fn test_q25_simd_content_type_with_charset() {
    let capsule = UniversalApiMetaCapsule::new();

    // Content-Type with charset parameter
    let request = MockRequest::new("POST", "/graphql")
        .with_header("Content-Type", "application/graphql; charset=utf-8");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::GraphQL);
}

#[test]
fn test_q26_simd_multiple_headers() {
    let capsule = UniversalApiMetaCapsule::new();

    // Request with many headers (gRPC)
    let request = MockRequest::new("POST", "/grpc.Service/Method")
        .with_header("Content-Type", "application/grpc")
        .with_header("grpc-encoding", "gzip")
        .with_header("grpc-timeout", "10s")
        .with_header("grpc-message-type", "unary");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::Grpc);
}

#[test]
fn test_q27_simd_determinism() {
    let capsule = UniversalApiMetaCapsule::new();

    // Same request should always return same protocol
    let request = MockRequest::new("POST", "/rpc")
        .with_header("Content-Type", "application/json-rpc");

    for _ in 0..100 {
        assert_eq!(capsule.detect_protocol(&request), ProtocolType::JsonRPC);
    }
}

#[test]
fn test_q28_simd_fallback_to_scalar() {
    let capsule = UniversalApiMetaCapsule::new();

    // Request that can't be detected by SIMD (falls back to scalar)
    let request = MockRequest::new("OPTIONS", "/api")  // OPTIONS not in SIMD signatures
        .with_header("Content-Type", "text/html");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::REST);  // Default
}
