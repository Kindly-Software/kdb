//! Adapters - Zero-copy HTTP/JSON-RPC to Universal Request/Response adapters
//!
//! Provides zero-copy adapters between protocol-specific types and UniversalRequest/UniversalResponse traits.

use super::{UniversalRequest, UniversalResponse, ProtocolType};

#[cfg(feature = "std")]
use std::{collections::HashMap, vec::Vec, string::String};

// ============================================================================
// HTTP Adapters
// ============================================================================

/// HTTP Request adapter (zero-copy borrows)
pub struct HttpUniversalRequest<'a> {
    method: &'a str,
    path: &'a str,
    headers: &'a HashMap<String, String>,
    body: &'a [u8],
}

impl<'a> HttpUniversalRequest<'a> {
    pub fn new(
        method: &'a str,
        path: &'a str,
        headers: &'a HashMap<String, String>,
        body: &'a [u8],
    ) -> Self {
        Self {
            method,
            path,
            headers,
            body,
        }
    }
}

impl<'a> UniversalRequest for HttpUniversalRequest<'a> {
    fn method(&self) -> &str {
        self.method
    }

    fn path(&self) -> &str {
        self.path
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(|s| s.as_str())
    }

    fn body(&self) -> &[u8] {
        self.body
    }

    fn protocol(&self) -> ProtocolType {
        // Detect protocol from Content-Type header
        if let Some(content_type) = self.header("Content-Type") {
            if content_type.contains("application/json-rpc") {
                return ProtocolType::JsonRPC;
            } else if content_type.contains("application/graphql") {
                return ProtocolType::GraphQL;
            } else if content_type.contains("application/grpc") {
                return ProtocolType::Grpc;
            }
        }

        // Default to REST
        ProtocolType::REST
    }
}

/// HTTP Response adapter
pub struct HttpUniversalResponse {
    status_code: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl HttpUniversalResponse {
    pub fn new(status_code: u16) -> Self {
        Self {
            status_code,
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }

    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
}

impl UniversalResponse for HttpUniversalResponse {
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

// ============================================================================
// JSON-RPC Adapters
// ============================================================================

/// JSON-RPC Request adapter (zero-copy borrows)
pub struct JsonRpcUniversalRequest<'a> {
    body: &'a [u8],
    method: String,  // Extracted from JSON (owned for simplicity)
}

impl<'a> JsonRpcUniversalRequest<'a> {
    pub fn new(body: &'a [u8]) -> Result<Self, &'static str> {
        // Simplified: Extract method from JSON body
        let body_str = std::str::from_utf8(body).map_err(|_| "Invalid UTF-8")?;

        let method = Self::extract_method(body_str)
            .ok_or("Missing method field")?;

        Ok(Self { body, method })
    }

    fn extract_method(json: &str) -> Option<String> {
        let pattern = r#""method":"#;
        let start = json.find(pattern)? + pattern.len();

        if json.chars().nth(start)? == '"' {
            let value_start = start + 1;
            let value_end = json[value_start..].find('"')? + value_start;
            Some(json[value_start..value_end].to_string())
        } else {
            None
        }
    }
}

impl<'a> UniversalRequest for JsonRpcUniversalRequest<'a> {
    fn method(&self) -> &str {
        "POST"  // JSON-RPC is always POST
    }

    fn path(&self) -> &str {
        &self.method  // Use JSON-RPC method as "path"
    }

    fn header(&self, _name: &str) -> Option<&str> {
        None  // JSON-RPC doesn't use HTTP headers
    }

    fn body(&self) -> &[u8] {
        self.body
    }

    fn protocol(&self) -> ProtocolType {
        ProtocolType::JsonRPC
    }
}

/// JSON-RPC Response adapter
pub struct JsonRpcUniversalResponse {
    body: Vec<u8>,
}

impl JsonRpcUniversalResponse {
    pub fn success(id: u64, result: &str) -> Self {
        let body = format!(
            r#"{{"jsonrpc":"2.0","result":{},"id":{}}}"#,
            result, id
        );
        Self {
            body: body.into_bytes(),
        }
    }

    pub fn error(id: Option<u64>, code: i32, message: &str) -> Self {
        let id_str = id.map(|i| i.to_string()).unwrap_or_else(|| "null".to_string());
        let body = format!(
            r#"{{"jsonrpc":"2.0","error":{{"code":{},"message":"{}"}},"id":{}}}"#,
            code, message, id_str
        );
        Self {
            body: body.into_bytes(),
        }
    }
}

impl UniversalResponse for JsonRpcUniversalResponse {
    fn status_code(&self) -> u16 {
        200  // JSON-RPC always returns 200 (errors are in response body)
    }

    fn set_header(&mut self, _name: String, _value: String) {
        // JSON-RPC doesn't use headers
    }

    fn body(&self) -> &[u8] {
        &self.body
    }

    fn protocol(&self) -> ProtocolType {
        ProtocolType::JsonRPC
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_request_adapter() {
        let headers = HashMap::from([
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Authorization".to_string(), "Bearer token".to_string()),
        ]);

        let body = b"test body";

        let request = HttpUniversalRequest::new("GET", "/api/users", &headers, body);

        assert_eq!(request.method(), "GET");
        assert_eq!(request.path(), "/api/users");
        assert_eq!(request.header("Content-Type"), Some("application/json"));
        assert_eq!(request.header("Authorization"), Some("Bearer token"));
        assert_eq!(request.header("Missing"), None);
        assert_eq!(request.body(), b"test body");
        assert_eq!(request.protocol(), ProtocolType::REST);
    }

    #[test]
    fn test_http_response_adapter() {
        let mut response = HttpUniversalResponse::new(201).with_body(b"Created".to_vec());

        assert_eq!(response.status_code(), 201);
        assert_eq!(response.body(), b"Created");
        assert_eq!(response.protocol(), ProtocolType::REST);

        response.set_header("X-Custom".to_string(), "value".to_string());
        assert_eq!(response.headers().get("X-Custom"), Some(&"value".to_string()));
    }

    #[test]
    fn test_jsonrpc_request_adapter() {
        let body = br#"{"jsonrpc":"2.0","method":"test_method","id":1}"#;

        let request = JsonRpcUniversalRequest::new(body).unwrap();

        assert_eq!(request.method(), "POST");
        assert_eq!(request.path(), "test_method");
        assert_eq!(request.body(), body);
        assert_eq!(request.protocol(), ProtocolType::JsonRPC);
    }

    #[test]
    fn test_jsonrpc_response_adapter_success() {
        let response = JsonRpcUniversalResponse::success(1, r#"{"status":"ok"}"#);

        assert_eq!(response.status_code(), 200);
        assert_eq!(response.protocol(), ProtocolType::JsonRPC);

        let body_str = std::str::from_utf8(response.body()).unwrap();
        assert!(body_str.contains(r#""jsonrpc":"2.0""#));
        assert!(body_str.contains(r#""result":{"status":"ok"}"#));
        assert!(body_str.contains(r#""id":1"#));
    }

    #[test]
    fn test_jsonrpc_response_adapter_error() {
        let response = JsonRpcUniversalResponse::error(Some(1), -32700, "Parse error");

        assert_eq!(response.status_code(), 200);

        let body_str = std::str::from_utf8(response.body()).unwrap();
        assert!(body_str.contains(r#""error":"#));
        assert!(body_str.contains(r#""code":-32700"#));
        assert!(body_str.contains(r#""message":"Parse error""#));
    }

    #[test]
    fn test_protocol_detection_jsonrpc() {
        let headers = HashMap::from([
            ("Content-Type".to_string(), "application/json-rpc".to_string()),
        ]);

        let request = HttpUniversalRequest::new("POST", "/rpc", &headers, &[]);
        assert_eq!(request.protocol(), ProtocolType::JsonRPC);
    }

    #[test]
    fn test_protocol_detection_graphql() {
        let headers = HashMap::from([
            ("Content-Type".to_string(), "application/graphql".to_string()),
        ]);

        let request = HttpUniversalRequest::new("POST", "/graphql", &headers, &[]);
        assert_eq!(request.protocol(), ProtocolType::GraphQL);
    }
}
