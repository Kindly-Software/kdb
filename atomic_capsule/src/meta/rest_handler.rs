//! REST Handler - Week 2 REST Protocol Integration
//!
//! Bridges UniversalApiMetaCapsule to HttpRouterCapsule for REST request handling.

use super::{UniversalRequest, UniversalResponse, ProtocolType, ApiError};

// Conditional import based on feature flags
#[cfg(all(feature = "std", feature = "universal-api", feature = "http"))]
use crate::http::router::{HttpRouterCapsule, Method};

// Fallback types when http module unavailable
#[cfg(not(all(feature = "std", feature = "universal-api", feature = "http")))]
mod fallback {
    #[derive(Debug, Clone, Copy)]
    pub enum Method {
        GET,
        POST,
        PUT,
        DELETE,
        PATCH,
        HEAD,
        OPTIONS,
    }

    pub struct HttpRouterCapsule;

    impl HttpRouterCapsule {
        pub fn new(_capacity: usize) -> Result<Self, String> {
            Ok(Self)
        }

        pub fn add_route(&self, _method: Method, _path: &str, _handler_id: usize) -> Result<(), String> {
            Ok(())
        }

        pub fn match_route(&self, _method: Method, _path: &str) -> Option<(usize, std::collections::HashMap<String, String>)> {
            None
        }
    }
}

#[cfg(not(all(feature = "std", feature = "universal-api", feature = "http")))]
use fallback::{HttpRouterCapsule, Method};

#[cfg(feature = "std")]
use std::collections::HashMap;

/// REST response wrapper
pub struct RestResponse {
    status_code: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl UniversalResponse for RestResponse {
    fn status_code(&self) -> u16 {
        self.status_code
    }

    fn set_header(&mut self, name: String, value: String) {
        self.headers.insert(name, value);
    }

    fn body(&self) -> &[u8] {
        &self.body
    }

    fn protocol(&self) -> ProtocolType {
        ProtocolType::REST
    }
}

impl RestResponse {
    pub fn new(status_code: u16, body: Vec<u8>) -> Self {
        Self {
            status_code,
            headers: HashMap::new(),
            body,
        }
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
}

/// REST handler integrating with HttpRouterCapsule
pub struct RestHandler<'a> {
    router: &'a HttpRouterCapsule,
}

impl<'a> RestHandler<'a> {
    /// Create new REST handler
    pub fn new(router: &'a HttpRouterCapsule) -> Self {
        Self { router }
    }

    /// Handle REST request through router
    ///
    /// Flow:
    /// 1. Extract method and path from UniversalRequest
    /// 2. Call router.find_route(path, method)
    /// 3. Execute handler function
    /// 4. Build RestResponse wrapping result
    pub fn handle(&self, request: &dyn UniversalRequest) -> Result<Box<dyn UniversalResponse>, ApiError> {
        // Step 1: Extract method and path
        let method = self.parse_method(request.method())?;
        let path = request.path();

        // Step 2: Find route in router
        let route_match = self.router.match_route(method, path)
            .ok_or_else(|| ApiError::HandlerNotFound {
                protocol: ProtocolType::REST,
                path: path.to_string(),
            })?;

        // Step 3: Execute handler (simplified - real handler would accept Request/Params)
        // For Week 2, we'll simulate handler execution
        let response_body = self.execute_handler(&route_match, request)?;

        // Step 4: Build RestResponse
        let mut response = RestResponse::new(200, response_body);
        response.set_header("Content-Type".to_string(), "application/json".to_string());

        Ok(Box::new(response))
    }

    /// Parse HTTP method string to Method enum
    fn parse_method(&self, method: &str) -> Result<Method, ApiError> {
        match method.to_uppercase().as_str() {
            "GET" => Ok(Method::GET),
            "POST" => Ok(Method::POST),
            "PUT" => Ok(Method::PUT),
            "DELETE" => Ok(Method::DELETE),
            "PATCH" => Ok(Method::PATCH),
            "HEAD" => Ok(Method::HEAD),
            "OPTIONS" => Ok(Method::OPTIONS),
            other => Err(ApiError::InvalidRequest {
                protocol: ProtocolType::REST,
                reason: format!("Unsupported HTTP method: {}", other),
            }),
        }
    }

    /// Execute handler function (simplified for Week 2)
    fn execute_handler(
        &self,
        route_match: &(usize, HashMap<String, String>),
        request: &dyn UniversalRequest,
    ) -> Result<Vec<u8>, ApiError> {
        let (_handler_id, params) = route_match;

        // Simplified: Just echo back the path and params
        let response = if params.is_empty() {
            format!(r#"{{"path":"{}","params":{{}}}}"#, request.path())
        } else {
            let params_json: Vec<String> = params
                .iter()
                .map(|(k, v)| format!(r#""{}":"{}""#, k, v))
                .collect();
            format!(
                r#"{{"path":"{}","params":{{{}}}}}"#,
                request.path(),
                params_json.join(",")
            )
        };

        Ok(response.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Method is already imported via super::* (either from http or fallback)

    struct MockRequest {
        method: &'static str,
        path: &'static str,
    }

    impl UniversalRequest for MockRequest {
        fn method(&self) -> &str { self.method }
        fn path(&self) -> &str { self.path }
        fn header(&self, _name: &str) -> Option<&str> { None }
        fn body(&self) -> &[u8] { &[] }
        fn protocol(&self) -> ProtocolType { ProtocolType::REST }
    }

    fn dummy_handler(_req: &(), _params: &HashMap<String, String>) -> Vec<u8> {
        b"OK".to_vec()
    }

    #[test]
    fn test_rest_handler_static_route() {
        let router = HttpRouterCapsule::new(64).unwrap();
        router.add_route(Method::GET, "/api/users", 1).unwrap();

        let handler = RestHandler::new(&router);
        let request = MockRequest {
            method: "GET",
            path: "/api/users",
        };

        let result = handler.handle(&request);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.status_code(), 200);
        assert_eq!(response.protocol(), ProtocolType::REST);
    }

    #[test]
    fn test_rest_handler_not_found() {
        let router = HttpRouterCapsule::new(64).unwrap();

        let handler = RestHandler::new(&router);
        let request = MockRequest {
            method: "GET",
            path: "/api/nonexistent",
        };

        let result = handler.handle(&request);
        assert!(result.is_err());

        match result {
            Err(ApiError::HandlerNotFound { protocol, path }) => {
                assert_eq!(protocol, ProtocolType::REST);
                assert_eq!(path, "/api/nonexistent");
            }
            _ => panic!("Expected HandlerNotFound error"),
        }
    }

    #[test]
    fn test_parse_method() {
        let router = HttpRouterCapsule::new(64).unwrap();
        let handler = RestHandler::new(&router);

        assert!(matches!(handler.parse_method("GET"), Ok(Method::GET)));
        assert!(matches!(handler.parse_method("POST"), Ok(Method::POST)));
        assert!(matches!(handler.parse_method("PUT"), Ok(Method::PUT)));
        assert!(matches!(handler.parse_method("DELETE"), Ok(Method::DELETE)));
        assert!(matches!(handler.parse_method("PATCH"), Ok(Method::PATCH)));
        assert!(matches!(handler.parse_method("HEAD"), Ok(Method::HEAD)));
        assert!(matches!(handler.parse_method("OPTIONS"), Ok(Method::OPTIONS)));

        assert!(handler.parse_method("INVALID").is_err());
    }
}
