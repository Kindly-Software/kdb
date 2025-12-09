//! Universal API Integration Tests - JSON-RPC Protocol
//!
//! Tests JSON-RPC routing through UniversalApiMetaCapsule → JsonRpcHandler

use atomic_capsule::meta::{
    UniversalApiMetaCapsule, UniversalRequest, ProtocolType, JsonRpcHandler,
    JsonRpcUniversalRequest, JsonRpcUniversalResponse,
};

// ============================================================================
// Helper: Mock Request
// ============================================================================

struct MockRequest {
    body: Vec<u8>,
}

impl MockRequest {
    fn new(body: Vec<u8>) -> Self {
        Self { body }
    }

    fn with_method(method: &str, id: u64) -> Self {
        let body = format!(r#"{{"jsonrpc":"2.0","method":"{}","id":{}}}"#, method, id);
        Self {
            body: body.into_bytes(),
        }
    }
}

impl UniversalRequest for MockRequest {
    fn method(&self) -> &str { "POST" }
    fn path(&self) -> &str { "/rpc" }
    fn header(&self, _name: &str) -> Option<&str> { None }
    fn body(&self) -> &[u8] { &self.body }
    fn protocol(&self) -> ProtocolType { ProtocolType::JsonRPC }
}

// ============================================================================
// T28 Q15-Q21: Integration Tests
// ============================================================================

#[test]
fn test_jsonrpc_method_dispatch() {
    // Setup: Create handler with registered methods
    let mut handler = JsonRpcHandler::new();
    handler.register_method("get_balance", 1);
    handler.register_method("send_transaction", 2);

    // Test: Dispatch to get_balance
    let request1 = MockRequest::with_method("get_balance", 1);
    let result1 = handler.handle(&request1);

    assert!(result1.is_ok(), "Request should succeed");

    let response1 = result1.unwrap();
    assert_eq!(response1.status_code(), 200);
    assert_eq!(response1.protocol(), ProtocolType::JsonRPC);

    // Verify: Response contains result
    let body_str = std::str::from_utf8(response1.body()).unwrap();
    assert!(body_str.contains(r#""jsonrpc":"2.0""#));
    assert!(body_str.contains(r#""result":"#));
    assert!(body_str.contains(r#""id":1"#));

    // Test: Dispatch to send_transaction
    let request2 = MockRequest::with_method("send_transaction", 2);
    let result2 = handler.handle(&request2);
    assert!(result2.is_ok());
}

#[test]
fn test_jsonrpc_method_not_found() {
    // Setup: Empty handler
    let handler = JsonRpcHandler::new();

    // Test: Request to unregistered method
    let request = MockRequest::with_method("unknown_method", 1);
    let result = handler.handle(&request);

    // Verify: Error
    assert!(result.is_err(), "Request should fail");
}

#[test]
fn test_jsonrpc_error_response() {
    // Test: Create error response
    let response = JsonRpcUniversalResponse::error(Some(1), -32700, "Parse error");

    assert_eq!(response.status_code(), 200); // JSON-RPC always returns 200
    assert_eq!(response.protocol(), ProtocolType::JsonRPC);

    // Verify: Error structure
    let body_str = std::str::from_utf8(response.body()).unwrap();
    assert!(body_str.contains(r#""jsonrpc":"2.0""#));
    assert!(body_str.contains(r#""error":"#));
    assert!(body_str.contains(r#""code":-32700"#));
    assert!(body_str.contains(r#""message":"Parse error""#));
    assert!(body_str.contains(r#""id":1"#));
}

#[test]
fn test_jsonrpc_success_response() {
    // Test: Create success response
    let response = JsonRpcUniversalResponse::success(1, r#"{"balance":1000}"#);

    assert_eq!(response.status_code(), 200);
    assert_eq!(response.protocol(), ProtocolType::JsonRPC);

    // Verify: Result structure
    let body_str = std::str::from_utf8(response.body()).unwrap();
    assert!(body_str.contains(r#""jsonrpc":"2.0""#));
    assert!(body_str.contains(r#""result":{"balance":1000}"#));
    assert!(body_str.contains(r#""id":1"#));
}

#[test]
fn test_jsonrpc_adapter_request() {
    // Test: Parse valid JSON-RPC request
    let body = br#"{"jsonrpc":"2.0","method":"test_method","id":42}"#;
    let request = JsonRpcUniversalRequest::new(body);

    assert!(request.is_ok(), "Valid request should parse");

    let req = request.unwrap();
    assert_eq!(req.method(), "POST");
    assert_eq!(req.path(), "test_method"); // Method name as path
    assert_eq!(req.body(), body);
    assert_eq!(req.protocol(), ProtocolType::JsonRPC);
}

#[test]
fn test_jsonrpc_adapter_invalid_request() {
    // Test: Invalid JSON
    let body = b"not valid json";
    let request = JsonRpcUniversalRequest::new(body);

    assert!(request.is_err(), "Invalid JSON should fail");
}

#[test]
fn test_jsonrpc_with_metacapsule() {
    // Setup: Create metacapsule
    let metacapsule = UniversalApiMetaCapsule::new();

    // Add middleware (e.g., logging)
    fn logging_middleware(request: &dyn UniversalRequest) -> Result<(), atomic_capsule::meta::MiddlewareError> {
        // Simple logging (would log in real impl)
        let _path = request.path();
        Ok(())
    }

    metacapsule.register_middleware(logging_middleware).expect("Failed to register middleware");

    // Test: Route JSON-RPC request through metacapsule
    let request = MockRequest::with_method("test", 1)
        .body; // Get body vec

    let mock_req = MockRequest::new(request);
    let result = metacapsule.route(&mock_req);

    // Verify: Passes middleware and detects JSON-RPC protocol
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ProtocolType::JsonRPC);
}

#[test]
fn test_jsonrpc_multiple_methods() {
    // Setup: Handler with multiple methods
    let mut handler = JsonRpcHandler::new();
    handler.register_method("method_a", 1);
    handler.register_method("method_b", 2);
    handler.register_method("method_c", 3);

    // Test: Each method
    for (method, id) in &[("method_a", 10), ("method_b", 20), ("method_c", 30)] {
        let request = MockRequest::with_method(method, *id);
        let result = handler.handle(&request);

        assert!(result.is_ok(), "Method {} should succeed", method);

        let response = result.unwrap();
        let body_str = std::str::from_utf8(response.body()).unwrap();
        assert!(body_str.contains(&format!(r#""id":{}"#, id)));
    }
}

#[test]
fn test_jsonrpc_null_id() {
    // Test: Error response with null id (notification)
    let response = JsonRpcUniversalResponse::error(None, -32600, "Invalid Request");

    let body_str = std::str::from_utf8(response.body()).unwrap();
    assert!(body_str.contains(r#""id":null"#));
}
