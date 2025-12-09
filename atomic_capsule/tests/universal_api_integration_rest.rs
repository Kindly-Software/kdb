//! Universal API Integration Tests - REST Protocol
//!
//! Tests REST routing through UniversalApiMetaCapsule → RestHandler → HttpRouterCapsule

use atomic_capsule::meta::{
    UniversalApiMetaCapsule, UniversalRequest, ProtocolType, RestHandler,
    HttpUniversalRequest, HttpUniversalResponse,
};
use atomic_capsule::http::router::{HttpRouterCapsule, Method};

#[cfg(feature = "std")]
use std::collections::HashMap;

// ============================================================================
// Helper: Mock Request
// ============================================================================

struct MockRequest {
    method: &'static str,
    path: &'static str,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl MockRequest {
    fn new(method: &'static str, path: &'static str) -> Self {
        Self {
            method,
            path,
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }

    fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(name.to_string(), value.to_string());
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
        self.headers.get(name).map(|s| s.as_str())
    }
    fn body(&self) -> &[u8] { &self.body }
    fn protocol(&self) -> ProtocolType { ProtocolType::REST }
}

// ============================================================================
// T28 Q15-Q21: Integration Tests
// ============================================================================

#[test]
fn test_rest_static_route_integration() {
    // Setup: Create router with static route
    let router = HttpRouterCapsule::new(128).expect("Failed to create router");
    router.add_route(Method::GET, "/api/users", 1).expect("Failed to add route");

    // Setup: Create REST handler
    let handler = RestHandler::new(&router);

    // Setup: Create request
    let request = MockRequest::new("GET", "/api/users")
        .with_header("Content-Type", "application/json");

    // Execute: Handle request
    let result = handler.handle(&request);

    // Verify: Success
    assert!(result.is_ok(), "Request should succeed");

    let response = result.unwrap();
    assert_eq!(response.status_code(), 200);
    assert_eq!(response.protocol(), ProtocolType::REST);

    // Verify: Body contains path
    let body_str = std::str::from_utf8(response.body()).unwrap();
    assert!(body_str.contains("/api/users"));
}

#[test]
fn test_rest_dynamic_route_integration() {
    // Setup: Create router with dynamic route
    let router = HttpRouterCapsule::new(128).expect("Failed to create router");
    router.add_route(Method::GET, "/api/users/:id", 2).expect("Failed to add route");

    // Setup: Create REST handler
    let handler = RestHandler::new(&router);

    // Setup: Create request with dynamic parameter
    let request = MockRequest::new("GET", "/api/users/123");

    // Execute: Handle request
    let result = handler.handle(&request);

    // Verify: Success
    assert!(result.is_ok(), "Request should succeed");

    let response = result.unwrap();
    assert_eq!(response.status_code(), 200);

    // Verify: Body contains parameter
    let body_str = std::str::from_utf8(response.body()).unwrap();
    assert!(body_str.contains("id"));
    assert!(body_str.contains("123"));
}

#[test]
fn test_rest_404_not_found() {
    // Setup: Empty router
    let router = HttpRouterCapsule::new(128).expect("Failed to create router");

    // Setup: Create REST handler
    let handler = RestHandler::new(&router);

    // Setup: Request to non-existent route
    let request = MockRequest::new("GET", "/api/nonexistent");

    // Execute: Handle request
    let result = handler.handle(&request);

    // Verify: 404 error
    assert!(result.is_err(), "Request should fail");
}

#[test]
fn test_rest_multiple_routes() {
    // Setup: Router with multiple routes
    let router = HttpRouterCapsule::new(128).expect("Failed to create router");
    router.add_route(Method::GET, "/api/users", 1).expect("Failed to add route");
    router.add_route(Method::POST, "/api/users", 2).expect("Failed to add route");
    router.add_route(Method::GET, "/api/posts", 3).expect("Failed to add route");

    let handler = RestHandler::new(&router);

    // Test: GET /api/users
    let request1 = MockRequest::new("GET", "/api/users");
    assert!(handler.handle(&request1).is_ok());

    // Test: POST /api/users
    let request2 = MockRequest::new("POST", "/api/users");
    assert!(handler.handle(&request2).is_ok());

    // Test: GET /api/posts
    let request3 = MockRequest::new("GET", "/api/posts");
    assert!(handler.handle(&request3).is_ok());
}

#[test]
fn test_rest_method_not_allowed() {
    // Setup: Router with GET only
    let router = HttpRouterCapsule::new(128).expect("Failed to create router");
    router.add_route(Method::GET, "/api/users", 1).expect("Failed to add route");

    let handler = RestHandler::new(&router);

    // Test: POST to GET-only route
    let request = MockRequest::new("POST", "/api/users");
    let result = handler.handle(&request);

    // Verify: Should fail (no POST handler)
    assert!(result.is_err());
}

#[test]
fn test_rest_parameter_extraction() {
    // Setup: Router with multiple parameters
    let router = HttpRouterCapsule::new(128).expect("Failed to create router");
    router.add_route(Method::GET, "/api/users/:user_id/posts/:post_id", 1)
        .expect("Failed to add route");

    let handler = RestHandler::new(&router);

    // Test: Request with parameters
    let request = MockRequest::new("GET", "/api/users/42/posts/100");
    let result = handler.handle(&request);

    assert!(result.is_ok());

    let response = result.unwrap();
    let body_str = std::str::from_utf8(response.body()).unwrap();

    // Verify: Both parameters extracted
    assert!(body_str.contains("user_id"));
    assert!(body_str.contains("42"));
    assert!(body_str.contains("post_id"));
    assert!(body_str.contains("100"));
}

#[test]
fn test_rest_with_middleware() {
    // Setup: Create metacapsule with middleware
    let metacapsule = UniversalApiMetaCapsule::new();

    // Add simple middleware
    fn auth_middleware(request: &dyn UniversalRequest) -> Result<(), atomic_capsule::meta::MiddlewareError> {
        if request.header("Authorization").is_some() {
            Ok(())
        } else {
            Err(atomic_capsule::meta::MiddlewareError::AuthFailed {
                reason: "Missing Authorization header".to_string(),
            })
        }
    }

    metacapsule.register_middleware(auth_middleware).expect("Failed to register middleware");

    // Setup: Router
    let router = HttpRouterCapsule::new(128).expect("Failed to create router");
    router.add_route(Method::GET, "/api/secure", 1).expect("Failed to add route");

    // Test 1: Request without auth (should fail in metacapsule)
    let request1 = MockRequest::new("GET", "/api/secure");
    let result1 = metacapsule.route(&request1);
    assert!(result1.is_err(), "Request without auth should fail middleware");

    // Test 2: Request with auth (should pass metacapsule)
    let request2 = MockRequest::new("GET", "/api/secure")
        .with_header("Authorization", "Bearer token");
    let result2 = metacapsule.route(&request2);
    assert!(result2.is_ok(), "Request with auth should pass middleware");
}

#[test]
fn test_rest_adapter_zero_copy() {
    // Setup: Headers and body (owned)
    let headers = HashMap::from([
        ("Content-Type".to_string(), "application/json".to_string()),
    ]);
    let body = b"test body";

    // Create adapter (borrows headers and body)
    let request = HttpUniversalRequest::new("GET", "/api/test", &headers, body);

    // Verify: Can still access original headers after adapter creation
    assert_eq!(headers.get("Content-Type"), Some(&"application/json".to_string()));

    // Verify: Adapter returns borrowed references
    assert_eq!(request.method(), "GET");
    assert_eq!(request.path(), "/api/test");
    assert_eq!(request.header("Content-Type"), Some("application/json"));
    assert_eq!(request.body(), b"test body");
}
