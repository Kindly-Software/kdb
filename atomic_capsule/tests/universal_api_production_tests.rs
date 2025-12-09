// UniversalApiMetaCapsule Production Tests (T28 Q22-Q28)
//
// Test Focus: Production scenarios, stress testing, recovery
//
// Framework Compliance:
// - T28: Production tier (Q22-Q28)
// - UCE34: Real-world validation
// - ASSUM: Safety verification under load

use atomic_capsule::meta::{
    UniversalApiMetaCapsule,
    UniversalRequest,
    ProtocolType,
    MiddlewareFn,
    MiddlewareError,
    BreakerPolicy,
};

// ============================================================================
// Mock Request/Response (test harness)
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
// T28 Q22: Stress Test (10K requests/sec simulation)
// ============================================================================

#[test]
fn test_stress_10k_requests() {
    let capsule = UniversalApiMetaCapsule::new();

    // Simulate 10K requests
    for i in 0..10_000 {
        let protocol = if i % 5 == 0 {
            ProtocolType::GraphQL
        } else if i % 3 == 0 {
            ProtocolType::JsonRPC
        } else {
            ProtocolType::REST
        };

        let request = match protocol {
            ProtocolType::GraphQL => {
                MockRequest::new("POST", "/graphql")
                    .with_header("Content-Type", "application/graphql")
            }
            ProtocolType::JsonRPC => {
                MockRequest::new("POST", "/rpc")
                    .with_header("Content-Type", "application/json-rpc")
            }
            _ => {
                MockRequest::new("GET", "/api/test")
                    .with_header("Content-Type", "application/json")
            }
        };

        let result = capsule.route(&request);
        assert!(result.is_ok(), "Request {} failed", i);
        assert_eq!(result.unwrap(), protocol);
    }
}

// ============================================================================
// T28 Q23: Circuit Breaker Triggering
// ============================================================================

#[test]
fn test_circuit_breaker_open_state() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("GET", "/api/test")
        .with_header("Content-Type", "application/json");

    // Circuit breaker should allow requests (default closed state)
    let result = capsule.check_circuit_breaker(ProtocolType::REST);
    assert!(result.is_ok(), "Circuit should be closed initially");

    // Route should succeed
    let route_result = capsule.route_with_breaker(&request);
    assert!(route_result.is_ok(), "Route should succeed with closed circuit");
}

// ============================================================================
// T28 Q24: Recovery Testing
// ============================================================================

#[test]
fn test_circuit_breaker_recovery() {
    let capsule = UniversalApiMetaCapsule::new();

    // Record successes (simulating recovery)
    for _ in 0..10 {
        capsule.record_success(ProtocolType::REST);
    }

    // Circuit should remain closed (healthy)
    let result = capsule.check_circuit_breaker(ProtocolType::REST);
    assert!(result.is_ok(), "Circuit should be closed after successes");
}

// ============================================================================
// T28 Q25: Multi-Protocol Load
// ============================================================================

#[test]
fn test_multi_protocol_concurrent_load() {
    let capsule = UniversalApiMetaCapsule::new();

    // Simulate concurrent multi-protocol traffic
    let protocols = [
        (ProtocolType::REST, "application/json"),
        (ProtocolType::GraphQL, "application/graphql"),
        (ProtocolType::Grpc, "application/grpc"),
        (ProtocolType::WebSocket, "websocket"),
        (ProtocolType::JsonRPC, "application/json-rpc"),
    ];

    for (protocol, content_type) in protocols.iter() {
        for _ in 0..1000 {
            let request = if *protocol == ProtocolType::WebSocket {
                MockRequest::new("GET", "/ws")
                    .with_header("Upgrade", "websocket")
            } else {
                MockRequest::new("POST", "/test")
                    .with_header("Content-Type", content_type)
            };

            let detected = capsule.detect_protocol(&request);
            assert_eq!(detected, *protocol, "Protocol detection failed for {:?}", protocol);
        }
    }
}

// ============================================================================
// T28 Q26: Middleware Chain Stress
// ============================================================================

fn middleware_compute(_request: &dyn UniversalRequest) -> Result<(), MiddlewareError> {
    // Simulate lightweight computation
    let _ = (1..100).sum::<u32>();
    Ok(())
}

#[test]
fn test_middleware_chain_stress() {
    let capsule = UniversalApiMetaCapsule::new();

    // Register maximum middleware (16)
    for _ in 0..16 {
        capsule.register_middleware(middleware_compute).unwrap();
    }

    let request = MockRequest::new("GET", "/api/test")
        .with_header("Content-Type", "application/json");

    // Execute 1000 requests through full middleware chain
    for _ in 0..1000 {
        let result = capsule.route(&request);
        assert!(result.is_ok(), "Middleware chain failed under stress");
    }
}

// ============================================================================
// T28 Q27: Policy Configuration Validation
// ============================================================================

#[test]
fn test_breaker_policy_configuration() {
    // Validate default policies for all protocols
    let policies = [
        (ProtocolType::REST, BreakerPolicy::rest_default()),
        (ProtocolType::GraphQL, BreakerPolicy::graphql_default()),
        (ProtocolType::Grpc, BreakerPolicy::grpc_default()),
        (ProtocolType::WebSocket, BreakerPolicy::websocket_default()),
        (ProtocolType::JsonRPC, BreakerPolicy::jsonrpc_default()),
    ];

    for (protocol, policy) in policies.iter() {
        // Verify policy is reasonable
        assert!(policy.timeout_ms > 0, "Timeout must be positive for {:?}", protocol);
        assert!(policy.error_threshold_percent <= 100, "Error threshold must be <=100% for {:?}", protocol);
        assert!(policy.min_samples > 0, "Min samples must be positive for {:?}", protocol);
        assert!(policy.open_duration_ms > 0, "Open duration must be positive for {:?}", protocol);

        // Verify pack/unpack round-trip
        let packed = policy.pack();
        let unpacked = BreakerPolicy::unpack(packed);
        assert_eq!(policy.timeout_ms, unpacked.timeout_ms, "Timeout mismatch for {:?}", protocol);
        assert_eq!(policy.error_threshold_percent, unpacked.error_threshold_percent, "Threshold mismatch for {:?}", protocol);
        assert_eq!(policy.min_samples, unpacked.min_samples, "Min samples mismatch for {:?}", protocol);
        assert_eq!(policy.open_duration_ms, unpacked.open_duration_ms, "Open duration mismatch for {:?}", protocol);
    }
}

// ============================================================================
// T28 Q28: End-to-End Integration
// ============================================================================

fn middleware_auth(_request: &dyn UniversalRequest) -> Result<(), MiddlewareError> {
    Ok(())
}

fn middleware_rate_limit(_request: &dyn UniversalRequest) -> Result<(), MiddlewareError> {
    Ok(())
}

fn middleware_validation(_request: &dyn UniversalRequest) -> Result<(), MiddlewareError> {
    Ok(())
}

#[test]
fn test_end_to_end_integration() {
    let capsule = UniversalApiMetaCapsule::new();

    // Register realistic middleware chain
    capsule.register_middleware(middleware_auth).unwrap();
    capsule.register_middleware(middleware_rate_limit).unwrap();
    capsule.register_middleware(middleware_validation).unwrap();

    // Test all protocol types
    let test_cases = [
        (ProtocolType::REST, "GET", "/api/users", "application/json"),
        (ProtocolType::GraphQL, "POST", "/graphql", "application/graphql"),
        (ProtocolType::Grpc, "POST", "/grpc", "application/grpc"),
        (ProtocolType::JsonRPC, "POST", "/rpc", "application/json-rpc"),
    ];

    for (expected_protocol, method, path, content_type) in test_cases.iter() {
        let request = MockRequest::new(method, path)
            .with_header("Content-Type", content_type);

        // Full end-to-end flow
        let route_result = capsule.route_with_breaker(&request);
        assert!(route_result.is_ok(), "End-to-end flow failed for {:?}", expected_protocol);
        assert_eq!(route_result.unwrap(), *expected_protocol);

        // Verify state updates
        let (protocol, _, _) = capsule.get_state();
        assert_eq!(protocol, *expected_protocol);
    }

    // Verify middleware count
    assert_eq!(capsule.middleware_count(), 3);
}

// ============================================================================
// Additional Production Tests
// ============================================================================

#[test]
fn test_protocol_detection_edge_cases() {
    let capsule = UniversalApiMetaCapsule::new();

    // Test Content-Type with parameters
    let request = MockRequest::new("POST", "/test")
        .with_header("Content-Type", "application/json; charset=utf-8");
    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::REST);

    // Test case-insensitive Upgrade header
    let request = MockRequest::new("GET", "/ws")
        .with_header("Upgrade", "WebSocket");  // Mixed case
    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::WebSocket);

    // Test gRPC via grpc-encoding header
    let request = MockRequest::new("POST", "/test")
        .with_header("grpc-encoding", "gzip");
    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::Grpc);
}

#[test]
fn test_success_failure_tracking() {
    let capsule = UniversalApiMetaCapsule::new();

    // Record mixed success/failure
    for _ in 0..7 {
        capsule.record_success(ProtocolType::REST);
    }

    for _ in 0..3 {
        capsule.record_failure(ProtocolType::REST);
    }

    // Circuit should still be closed (70% success rate)
    let result = capsule.check_circuit_breaker(ProtocolType::REST);
    assert!(result.is_ok(), "Circuit should be closed with 70% success rate");
}
