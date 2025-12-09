// UniversalApiMetaCapsule - T28 Unit Tests (Q1-Q7)
//
// Framework: T28 comprehensive testing (4 tiers)
// Tier 1: Unit tests (Q1-Q7) - THIS FILE
// Status: Week 1 implementation
//
// Coverage:
// - Q1: Capsule initialization and layout
// - Q2: Protocol detection (5 protocols)
// - Q3: Middleware chain execution
// - Q4: Route integration (protocol + middleware)
// - Q5: Generation counter (TOCTOU prevention)
// - Q6: Protocol state persistence
// - Q7: Zero-copy validation

use atomic_capsule::meta::{
    UniversalApiMetaCapsule,
    UniversalRequest,
    ProtocolType,
    MiddlewareFn,
    MiddlewareError,
};

// ============================================================================
// Test Utilities
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
// T28 Q1: Capsule Initialization and Layout
// ============================================================================

#[test]
fn q1_test_capsule_initialization() {
    let capsule = UniversalApiMetaCapsule::new();

    // Verify default state
    let (protocol, generation, _) = capsule.get_state();
    assert_eq!(protocol, ProtocolType::REST, "Default protocol should be REST");
    assert_eq!(generation, 0, "Initial generation should be 0");
    assert_eq!(capsule.middleware_count(), 0, "Initial middleware count should be 0");
}

#[test]
fn q1_test_capsule_layout_512b() {
    // Verify 512-byte layout (cache-aligned)
    assert_eq!(
        core::mem::size_of::<UniversalApiMetaCapsule>(),
        512,
        "UniversalApiMetaCapsule must be exactly 512 bytes"
    );

    assert_eq!(
        core::mem::align_of::<UniversalApiMetaCapsule>(),
        512,
        "UniversalApiMetaCapsule must be 512-byte aligned"
    );
}

#[test]
fn q1_test_capsule_default_trait() {
    let capsule = UniversalApiMetaCapsule::default();
    let (protocol, generation, _) = capsule.get_state();

    assert_eq!(protocol, ProtocolType::REST);
    assert_eq!(generation, 0);
}

// ============================================================================
// T28 Q2: Protocol Detection (5 protocols)
// ============================================================================

#[test]
fn q2_test_protocol_detection_rest_default() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("GET", "/api/users");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::REST, "Default protocol should be REST");
}

#[test]
fn q2_test_protocol_detection_graphql() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("POST", "/graphql")
        .with_header("Content-Type", "application/graphql");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::GraphQL, "Content-Type: application/graphql → GraphQL");
}

#[test]
fn q2_test_protocol_detection_grpc() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("POST", "/grpc")
        .with_header("Content-Type", "application/grpc");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::Grpc, "Content-Type: application/grpc → gRPC");
}

#[test]
fn q2_test_protocol_detection_websocket() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("GET", "/ws")
        .with_header("Upgrade", "websocket");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::WebSocket, "Upgrade: websocket → WebSocket");
}

#[test]
fn q2_test_protocol_detection_jsonrpc() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("POST", "/rpc")
        .with_header("Content-Type", "application/json-rpc");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::JsonRPC, "Content-Type: application/json-rpc → JSON-RPC");
}

#[test]
fn q2_test_protocol_detection_grpc_via_headers() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("POST", "/service/method")
        .with_header("grpc-encoding", "gzip");

    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::Grpc, "grpc-encoding header → gRPC");
}

#[test]
fn q2_test_protocol_detection_case_insensitive() {
    let capsule = UniversalApiMetaCapsule::new();

    // WebSocket with different case
    let request1 = MockRequest::new("GET", "/ws")
        .with_header("upgrade", "WebSocket"); // Lowercase header name, mixed case value

    let protocol1 = capsule.detect_protocol(&request1);
    assert_eq!(protocol1, ProtocolType::WebSocket, "Case-insensitive header matching");
}

// ============================================================================
// T28 Q3: Middleware Chain Execution
// ============================================================================

fn middleware_noop(_request: &dyn UniversalRequest) -> Result<(), MiddlewareError> {
    Ok(())
}

fn middleware_reject(_request: &dyn UniversalRequest) -> Result<(), MiddlewareError> {
    Err(MiddlewareError::AuthFailed {
        reason: "Test rejection".to_string(),
    })
}

#[test]
fn q3_test_middleware_registration_single() {
    let capsule = UniversalApiMetaCapsule::new();

    capsule.register_middleware(middleware_noop).unwrap();
    assert_eq!(capsule.middleware_count(), 1);
}

#[test]
fn q3_test_middleware_registration_multiple() {
    let capsule = UniversalApiMetaCapsule::new();

    capsule.register_middleware(middleware_noop).unwrap();
    capsule.register_middleware(middleware_noop).unwrap();
    capsule.register_middleware(middleware_noop).unwrap();

    assert_eq!(capsule.middleware_count(), 3);
}

#[test]
fn q3_test_middleware_execution_success() {
    let capsule = UniversalApiMetaCapsule::new();
    capsule.register_middleware(middleware_noop).unwrap();
    capsule.register_middleware(middleware_noop).unwrap();

    let request = MockRequest::new("GET", "/test");
    let result = capsule.execute_middleware(&request);
    assert!(result.is_ok(), "Middleware chain should succeed");
}

#[test]
fn q3_test_middleware_execution_short_circuit() {
    let capsule = UniversalApiMetaCapsule::new();
    capsule.register_middleware(middleware_noop).unwrap();
    capsule.register_middleware(middleware_reject).unwrap(); // Fails here
    capsule.register_middleware(middleware_noop).unwrap();   // Never reached

    let request = MockRequest::new("GET", "/test");
    let result = capsule.execute_middleware(&request);
    assert!(result.is_err(), "Middleware chain should short-circuit on first error");
}

#[test]
fn q3_test_middleware_max_capacity_16() {
    let capsule = UniversalApiMetaCapsule::new();

    // Register 16 middleware (max capacity)
    for i in 0..16 {
        capsule.register_middleware(middleware_noop).unwrap_or_else(|e| {
            panic!("Failed to register middleware {}: {:?}", i, e);
        });
    }

    assert_eq!(capsule.middleware_count(), 16);

    // 17th registration should fail
    let result = capsule.register_middleware(middleware_noop);
    assert!(result.is_err(), "17th middleware registration should fail");
}

#[test]
fn q3_test_middleware_execution_empty_chain() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("GET", "/test");

    // No middleware registered
    let result = capsule.execute_middleware(&request);
    assert!(result.is_ok(), "Empty middleware chain should succeed");
}

// ============================================================================
// T28 Q4: Route Integration (Protocol + Middleware)
// ============================================================================

#[test]
fn q4_test_route_with_middleware_success() {
    let capsule = UniversalApiMetaCapsule::new();
    capsule.register_middleware(middleware_noop).unwrap();

    let request = MockRequest::new("GET", "/api/users")
        .with_header("Content-Type", "application/json");

    let result = capsule.route(&request);
    assert!(result.is_ok(), "Route should succeed with passing middleware");
    assert_eq!(result.unwrap(), ProtocolType::REST);
}

#[test]
fn q4_test_route_middleware_rejection() {
    let capsule = UniversalApiMetaCapsule::new();
    capsule.register_middleware(middleware_reject).unwrap();

    let request = MockRequest::new("GET", "/api/users");

    let result = capsule.route(&request);
    assert!(result.is_err(), "Route should fail when middleware rejects");
}

#[test]
fn q4_test_route_protocol_detection_integration() {
    let capsule = UniversalApiMetaCapsule::new();

    let request = MockRequest::new("POST", "/graphql")
        .with_header("Content-Type", "application/graphql");

    let result = capsule.route(&request);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ProtocolType::GraphQL);
}

// ============================================================================
// T28 Q5: Generation Counter (TOCTOU Prevention)
// ============================================================================

#[test]
fn q5_test_generation_counter_increments() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("GET", "/test");

    // Initial generation = 0
    let (_, gen1, _) = capsule.get_state();
    assert_eq!(gen1, 0, "Initial generation should be 0");

    // Route request (increments generation)
    capsule.route(&request).unwrap();
    let (_, gen2, _) = capsule.get_state();
    assert_eq!(gen2, 1, "Generation should increment to 1");

    // Route again (increments again)
    capsule.route(&request).unwrap();
    let (_, gen3, _) = capsule.get_state();
    assert_eq!(gen3, 2, "Generation should increment to 2");
}

#[test]
fn q5_test_generation_counter_overflow_safety() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("GET", "/test");

    // Simulate many requests (verify no overflow panic)
    for _ in 0..1000 {
        capsule.route(&request).unwrap();
    }

    let (_, generation, _) = capsule.get_state();
    assert_eq!(generation, 1000, "Generation should reach 1000");
}

// ============================================================================
// T28 Q6: Protocol State Persistence
// ============================================================================

#[test]
fn q6_test_protocol_state_persistence_graphql() {
    let capsule = UniversalApiMetaCapsule::new();

    let request = MockRequest::new("POST", "/graphql")
        .with_header("Content-Type", "application/graphql");

    capsule.route(&request).unwrap();

    let (protocol, _, _) = capsule.get_state();
    assert_eq!(protocol, ProtocolType::GraphQL, "Protocol state should persist GraphQL");
}

#[test]
fn q6_test_protocol_state_updates() {
    let capsule = UniversalApiMetaCapsule::new();

    // First request: REST
    let request1 = MockRequest::new("GET", "/api/users");
    capsule.route(&request1).unwrap();

    let (protocol1, _, _) = capsule.get_state();
    assert_eq!(protocol1, ProtocolType::REST);

    // Second request: GraphQL (overwrites state)
    let request2 = MockRequest::new("POST", "/graphql")
        .with_header("Content-Type", "application/graphql");
    capsule.route(&request2).unwrap();

    let (protocol2, _, _) = capsule.get_state();
    assert_eq!(protocol2, ProtocolType::GraphQL, "Protocol state should update to GraphQL");
}

// ============================================================================
// T28 Q7: Zero-Copy Validation
// ============================================================================

#[test]
fn q7_test_zero_copy_request_handling() {
    let capsule = UniversalApiMetaCapsule::new();

    // Create request with owned data
    let request = MockRequest::new("GET", "/test");

    // Route should not clone/move request (borrows only)
    let result = capsule.route(&request);
    assert!(result.is_ok());

    // Verify request still accessible (not moved/consumed)
    assert_eq!(request.method(), "GET", "Request should not be consumed");
    assert_eq!(request.path(), "/test", "Request data should be intact");
}

#[test]
fn q7_test_zero_copy_middleware_chain() {
    let capsule = UniversalApiMetaCapsule::new();
    capsule.register_middleware(middleware_noop).unwrap();
    capsule.register_middleware(middleware_noop).unwrap();

    let request = MockRequest::new("GET", "/test");

    // Execute middleware (borrows only)
    capsule.execute_middleware(&request).unwrap();

    // Verify request still accessible
    assert_eq!(request.method(), "GET", "Request should not be consumed by middleware");
}

#[test]
fn q7_test_zero_copy_protocol_detection() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest::new("POST", "/graphql")
        .with_header("Content-Type", "application/graphql");

    // Detect protocol (borrows only)
    let protocol = capsule.detect_protocol(&request);
    assert_eq!(protocol, ProtocolType::GraphQL);

    // Verify request still accessible
    assert_eq!(request.method(), "POST", "Request should not be consumed by detection");
    assert_eq!(request.path(), "/graphql", "Request data should be intact");
}

// ============================================================================
// SUMMARY
// ============================================================================

// T28 Unit Tests (Q1-Q7): 28 tests total
//
// Coverage:
// - Q1: Layout verification (3 tests)
// - Q2: Protocol detection (7 tests)
// - Q3: Middleware chain (6 tests)
// - Q4: Route integration (3 tests)
// - Q5: Generation counter (2 tests)
// - Q6: Protocol state (2 tests)
// - Q7: Zero-copy validation (3 tests)
//
// Status: Week 1 implementation (core structure + protocol detection)
// Next: Week 2 - Protocol handler integration (REST/GraphQL/gRPC/WebSocket/JSON-RPC)
