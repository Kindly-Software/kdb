//! JSON-RPC Handler - Week 2 JSON-RPC Protocol Integration
//!
//! Bridges UniversalApiMetaCapsule to JsonRpcCapsule for JSON-RPC 2.0 request handling.

use super::{UniversalRequest, UniversalResponse, ProtocolType, ApiError};

#[cfg(feature = "std")]
use std::collections::HashMap;

/// JSON-RPC response wrapper
pub struct JsonRpcResponse {
    status_code: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl UniversalResponse for JsonRpcResponse {
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
        ProtocolType::JsonRPC
    }
}

impl JsonRpcResponse {
    pub fn new(status_code: u16, body: Vec<u8>) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json-rpc".to_string());

        Self {
            status_code,
            headers,
            body,
        }
    }

    pub fn success(id: u64, result: &str) -> Self {
        let body = format!(
            r#"{{"jsonrpc":"2.0","result":{},"id":{}}}"#,
            result, id
        );
        Self::new(200, body.into_bytes())
    }

    pub fn error(id: Option<u64>, code: i32, message: &str) -> Self {
        let id_str = id.map(|i| i.to_string()).unwrap_or_else(|| "null".to_string());
        let body = format!(
            r#"{{"jsonrpc":"2.0","error":{{"code":{},"message":"{}"}},"id":{}}}"#,
            code, message, id_str
        );
        Self::new(200, body.into_bytes())
    }
}

/// JSON-RPC method registry
pub struct MethodRegistry {
    methods: HashMap<String, usize>,
}

impl MethodRegistry {
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, handler_id: usize) {
        self.methods.insert(name.to_string(), handler_id);
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.methods.get(name).copied()
    }
}

/// JSON-RPC handler
pub struct JsonRpcHandler {
    registry: MethodRegistry,
}

impl JsonRpcHandler {
    /// Create new JSON-RPC handler
    pub fn new() -> Self {
        Self {
            registry: MethodRegistry::new(),
        }
    }

    /// Register a method handler
    pub fn register_method(&mut self, name: &str, handler_id: usize) {
        self.registry.register(name, handler_id);
    }

    /// Handle JSON-RPC request
    ///
    /// Flow:
    /// 1. Parse JSON-RPC request from body
    /// 2. Dispatch to registered method
    /// 3. Format JSON-RPC response
    /// 4. Build JsonRpcResponse wrapping result
    pub fn handle(&self, request: &dyn UniversalRequest) -> Result<Box<dyn UniversalResponse>, ApiError> {
        // Step 1: Parse JSON-RPC request
        let parsed = self.parse_request(request.body())?;

        // Step 2: Dispatch to method
        let handler_id = self.registry.find(&parsed.method)
            .ok_or_else(|| ApiError::HandlerNotFound {
                protocol: ProtocolType::JsonRPC,
                path: parsed.method.clone(),
            })?;

        // Step 3: Execute handler (simplified for Week 2)
        let result = self.execute_method(handler_id, &parsed)?;

        // Step 4: Format JSON-RPC response
        let response = JsonRpcResponse::success(parsed.id.unwrap_or(0), &result);

        Ok(Box::new(response))
    }

    /// Parse JSON-RPC request (simplified parser)
    fn parse_request(&self, body: &[u8]) -> Result<ParsedRequest, ApiError> {
        let body_str = std::str::from_utf8(body)
            .map_err(|_| ApiError::InvalidRequest {
                protocol: ProtocolType::JsonRPC,
                reason: "Invalid UTF-8".to_string(),
            })?;

        // Simplified JSON parsing (real impl would use serde_json)
        // Format: {"jsonrpc":"2.0","method":"name","params":[...],"id":1}

        let method = self.extract_field(body_str, "method")
            .ok_or_else(|| ApiError::InvalidRequest {
                protocol: ProtocolType::JsonRPC,
                reason: "Missing method field".to_string(),
            })?;

        let id = self.extract_number_field(body_str, "id");

        Ok(ParsedRequest {
            method,
            params: None,
            id,
        })
    }

    /// Extract string field from JSON (simplified)
    fn extract_field(&self, json: &str, field: &str) -> Option<String> {
        let pattern = format!(r#""{}":"#, field);
        let start = json.find(&pattern)? + pattern.len();

        if json.chars().nth(start)? == '"' {
            let value_start = start + 1;
            let value_end = json[value_start..].find('"')? + value_start;
            Some(json[value_start..value_end].to_string())
        } else {
            None
        }
    }

    /// Extract number field from JSON (simplified)
    fn extract_number_field(&self, json: &str, field: &str) -> Option<u64> {
        let pattern = format!(r#""{}":"#, field);
        let start = json.find(&pattern)? + pattern.len();

        let value_end = json[start..].find(|c: char| !c.is_ascii_digit())? + start;
        json[start..value_end].parse().ok()
    }

    /// Execute method handler (simplified)
    fn execute_method(&self, handler_id: usize, request: &ParsedRequest) -> Result<String, ApiError> {
        // Simplified: Just return handler_id as proof of dispatch
        Ok(format!(r#"{{"handler_id":{}}}"#, handler_id))
    }
}

/// Parsed JSON-RPC request
struct ParsedRequest {
    method: String,
    params: Option<String>,
    id: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRequest {
        body: Vec<u8>,
    }

    impl UniversalRequest for MockRequest {
        fn method(&self) -> &str { "POST" }
        fn path(&self) -> &str { "/rpc" }
        fn header(&self, _name: &str) -> Option<&str> { None }
        fn body(&self) -> &[u8] { &self.body }
        fn protocol(&self) -> ProtocolType { ProtocolType::JsonRPC }
    }

    #[test]
    fn test_jsonrpc_handler_success() {
        let mut handler = JsonRpcHandler::new();
        handler.register_method("test_method", 42);

        let request = MockRequest {
            body: br#"{"jsonrpc":"2.0","method":"test_method","id":1}"#.to_vec(),
        };

        let result = handler.handle(&request);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.status_code(), 200);
        assert_eq!(response.protocol(), ProtocolType::JsonRPC);

        let body_str = std::str::from_utf8(response.body()).unwrap();
        assert!(body_str.contains(r#""jsonrpc":"2.0""#));
        assert!(body_str.contains(r#""result":"#));
        assert!(body_str.contains(r#""id":1"#));
    }

    #[test]
    fn test_jsonrpc_handler_method_not_found() {
        let handler = JsonRpcHandler::new();

        let request = MockRequest {
            body: br#"{"jsonrpc":"2.0","method":"unknown","id":1}"#.to_vec(),
        };

        let result = handler.handle(&request);
        assert!(result.is_err());

        match result {
            Err(ApiError::HandlerNotFound { protocol, path }) => {
                assert_eq!(protocol, ProtocolType::JsonRPC);
                assert_eq!(path, "unknown");
            }
            _ => panic!("Expected HandlerNotFound error"),
        }
    }

    #[test]
    fn test_jsonrpc_response_success() {
        let response = JsonRpcResponse::success(1, r#"{"status":"ok"}"#);

        assert_eq!(response.status_code(), 200);
        assert_eq!(response.protocol(), ProtocolType::JsonRPC);

        let body_str = std::str::from_utf8(response.body()).unwrap();
        assert!(body_str.contains(r#""jsonrpc":"2.0""#));
        assert!(body_str.contains(r#""result":{"status":"ok"}"#));
        assert!(body_str.contains(r#""id":1"#));
    }

    #[test]
    fn test_jsonrpc_response_error() {
        let response = JsonRpcResponse::error(Some(1), -32601, "Method not found");

        assert_eq!(response.status_code(), 200);

        let body_str = std::str::from_utf8(response.body()).unwrap();
        assert!(body_str.contains(r#""jsonrpc":"2.0""#));
        assert!(body_str.contains(r#""error":"#));
        assert!(body_str.contains(r#""code":-32601"#));
        assert!(body_str.contains(r#""message":"Method not found""#));
        assert!(body_str.contains(r#""id":1"#));
    }
}
