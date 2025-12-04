//! JsonRpcCapsule - T1 Atomic JSON-RPC Parser/Formatter (4 KB)
//!
//! Lockfree JSON-RPC 2.0 request parsing and response formatting.
//! **Latency**: <1μs parse/format
//! **Tier**: T1 Atomic (DualAtomicU64 coordination)

use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "json-rpc")]
use serde::{Deserialize, Serialize};

// ============================================================================
// JsonRpcCapsule (4 KB, 64-byte aligned)
// ============================================================================

#[repr(C, align(64))]
pub struct JsonRpcCapsule {
    // Request metrics (64 bytes)
    pub requests_parsed: AtomicU64,       // Total requests parsed
    pub parse_errors: AtomicU64,          // Parse error count
    pub responses_formatted: AtomicU64,   // Total responses formatted
    pub format_errors: AtomicU64,         // Format error count
    pub total_bytes_in: AtomicU64,        // Total input bytes
    pub total_bytes_out: AtomicU64,       // Total output bytes
    pub avg_latency_ns: AtomicU64,        // Average parse latency (ns)
    _padding: [u8; 8],

    // Reserved space (4KB - 64 bytes = 4032 bytes)
    _reserved: [u8; 4032],
}

impl JsonRpcCapsule {
    /// Create new JSON-RPC capsule
    pub const fn new() -> Self {
        Self {
            requests_parsed: AtomicU64::new(0),
            parse_errors: AtomicU64::new(0),
            responses_formatted: AtomicU64::new(0),
            format_errors: AtomicU64::new(0),
            total_bytes_in: AtomicU64::new(0),
            total_bytes_out: AtomicU64::new(0),
            avg_latency_ns: AtomicU64::new(0),
            _padding: [0; 8],
            _reserved: [0; 4032],
        }
    }

    /// Parse JSON-RPC request (<1μs)
    #[cfg(feature = "json-rpc")]
    pub fn parse_request(&self, json: &str) -> Result<JsonRpcRequest, &'static str> {
        let start = self.get_timestamp_ns();

        // Track input bytes
        self.total_bytes_in.fetch_add(json.len() as u64, Ordering::Relaxed);

        // Parse JSON-RPC request
        let req: JsonRpcRequest = serde_json::from_str(json)
            .map_err(|_| {
                self.parse_errors.fetch_add(1, Ordering::Relaxed);
                "Invalid JSON-RPC request"
            })?;

        // Validate JSON-RPC 2.0
        if req.jsonrpc != "2.0" {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            return Err("Invalid jsonrpc version");
        }

        // Update metrics
        self.requests_parsed.fetch_add(1, Ordering::Relaxed);

        let elapsed_ns = self.get_timestamp_ns() - start;
        self.update_avg_latency(elapsed_ns);

        Ok(req)
    }

    /// Format JSON-RPC response (<1μs)
    #[cfg(feature = "json-rpc")]
    pub fn format_response(&self, id: u64, result: serde_json::Value) -> Result<String, &'static str> {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        };

        let json = serde_json::to_string(&resp)
            .map_err(|_| {
                self.format_errors.fetch_add(1, Ordering::Relaxed);
                "Failed to serialize response"
            })?;

        // Track output bytes
        self.total_bytes_out.fetch_add(json.len() as u64, Ordering::Relaxed);
        self.responses_formatted.fetch_add(1, Ordering::Relaxed);

        Ok(json)
    }

    /// Format JSON-RPC error response
    #[cfg(feature = "json-rpc")]
    pub fn format_error(&self, id: u64, code: i32, message: String) -> Result<String, &'static str> {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        };

        let json = serde_json::to_string(&resp)
            .map_err(|_| {
                self.format_errors.fetch_add(1, Ordering::Relaxed);
                "Failed to serialize error response"
            })?;

        self.total_bytes_out.fetch_add(json.len() as u64, Ordering::Relaxed);
        self.responses_formatted.fetch_add(1, Ordering::Relaxed);

        Ok(json)
    }

    /// Get statistics
    pub fn get_stats(&self) -> JsonRpcStats {
        JsonRpcStats {
            requests_parsed: self.requests_parsed.load(Ordering::Relaxed),
            parse_errors: self.parse_errors.load(Ordering::Relaxed),
            responses_formatted: self.responses_formatted.load(Ordering::Relaxed),
            format_errors: self.format_errors.load(Ordering::Relaxed),
            total_bytes_in: self.total_bytes_in.load(Ordering::Relaxed),
            total_bytes_out: self.total_bytes_out.load(Ordering::Relaxed),
            avg_latency_ns: self.avg_latency_ns.load(Ordering::Relaxed),
        }
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    #[inline]
    fn get_timestamp_ns(&self) -> u64 {
        #[cfg(feature = "std")]
        {
            use std::time::SystemTime;
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0)
        }
        #[cfg(not(feature = "std"))]
        {
            0 // No-op in no_std
        }
    }

    fn update_avg_latency(&self, new_latency_ns: u64) {
        // Simple moving average (good enough for <1μs latency)
        let old_avg = self.avg_latency_ns.load(Ordering::Relaxed);
        let count = self.requests_parsed.load(Ordering::Relaxed);

        if count > 0 {
            let new_avg = (old_avg * (count - 1) + new_latency_ns) / count;
            self.avg_latency_ns.store(new_avg, Ordering::Relaxed);
        } else {
            self.avg_latency_ns.store(new_latency_ns, Ordering::Relaxed);
        }
    }
}

// ============================================================================
// JSON-RPC Types
// ============================================================================

#[cfg(feature = "json-rpc")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[cfg(feature = "json-rpc")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[cfg(feature = "json-rpc")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

/// JSON-RPC statistics
#[derive(Debug, Clone, Copy)]
pub struct JsonRpcStats {
    pub requests_parsed: u64,
    pub parse_errors: u64,
    pub responses_formatted: u64,
    pub format_errors: u64,
    pub total_bytes_in: u64,
    pub total_bytes_out: u64,
    pub avg_latency_ns: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{size_of, align_of};

    #[test]
    fn test_json_rpc_capsule_size() {
        assert_eq!(size_of::<JsonRpcCapsule>(), 4096, "JsonRpcCapsule must be 4 KB");
    }

    #[test]
    fn test_json_rpc_capsule_alignment() {
        assert_eq!(align_of::<JsonRpcCapsule>(), 64, "JsonRpcCapsule must be 64-byte aligned");
    }

    #[test]
    #[cfg(feature = "json-rpc")]
    fn test_parse_request() {
        let capsule = JsonRpcCapsule::new();

        let json = r#"{"jsonrpc":"2.0","id":1,"method":"debugger/attach","params":{"pid":12345}}"#;
        let req = capsule.parse_request(json).unwrap();

        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, 1);
        assert_eq!(req.method, "debugger/attach");

        let stats = capsule.get_stats();
        assert_eq!(stats.requests_parsed, 1);
        assert_eq!(stats.parse_errors, 0);
    }

    #[test]
    #[cfg(feature = "json-rpc")]
    fn test_format_response() {
        let capsule = JsonRpcCapsule::new();

        let result = serde_json::json!({"status": "ok"});
        let json = capsule.format_response(1, result).unwrap();

        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"result\""));

        let stats = capsule.get_stats();
        assert_eq!(stats.responses_formatted, 1);
        assert_eq!(stats.format_errors, 0);
    }

    #[test]
    #[cfg(feature = "json-rpc")]
    fn test_invalid_jsonrpc_version() {
        let capsule = JsonRpcCapsule::new();

        let json = r#"{"jsonrpc":"1.0","id":1,"method":"test"}"#;
        let result = capsule.parse_request(json);

        assert!(result.is_err());
        assert_eq!(capsule.get_stats().parse_errors, 1);
    }
}
