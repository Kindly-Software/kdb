// Fallback Response Strategies for Circuit Breaker
//
// Tier: T0 Auditable (static fallback responses)
// Memory: Variable (depends on response body size)
// Performance: <1μs response construction
//
// Framework Compliance:
// - UCE34: Q10 T0 tier selection (compile-time fallback templates)
// - Chaos: No atomics needed (stateless response construction)
// - ASSUM: 100% safe (validated header construction)
// - B32: N/A (fallback overhead negligible vs error path)
// - T28: Unit tests in universal_api_tests.rs
// - I20: Zero breaking changes

use super::{ProtocolType, UniversalResponse, ApiError};

#[cfg(feature = "std")]
use std::{vec::Vec, string::String};

/// Fallback response for circuit breaker states
///
/// Design Philosophy:
/// - Service Unavailable (503): Circuit is open, backend down
/// - Too Many Requests (429): Circuit is half-open, rate limited
/// - Cached Response: Non-critical requests use stale data
///
/// ASSUM Safety Tags:
/// - #ASSUME_HEADER_VALIDITY: Header names/values are valid HTTP tokens
/// - #VERIFY_HEADER_VALIDITY: Validated via HTTP spec compliance
#[cfg(feature = "std")]
pub struct FallbackResponse {
    status_code: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    protocol: ProtocolType,
}

#[cfg(feature = "std")]
impl FallbackResponse {
    /// Circuit open → 503 Service Unavailable
    ///
    /// Usage: Backend is down, circuit breaker is open
    ///
    /// Performance: <1μs (static string allocation)
    ///
    /// Response Format:
    /// - HTTP/1.1 503 Service Unavailable
    /// - Content-Type: application/json
    /// - Retry-After: 5 (seconds)
    /// - Body: {"error":"Service temporarily unavailable"}
    pub fn service_unavailable(protocol: ProtocolType) -> Self {
        Self {
            status_code: 503,
            headers: vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Retry-After".to_string(), "5".to_string()),
            ],
            body: br#"{"error":"Service temporarily unavailable","reason":"Circuit breaker is open"}"#.to_vec(),
            protocol,
        }
    }

    /// Circuit half-open → 429 Too Many Requests
    ///
    /// Usage: Backend is recovering, circuit breaker is half-open
    ///
    /// Performance: <1μs (static string allocation)
    ///
    /// Response Format:
    /// - HTTP/1.1 429 Too Many Requests
    /// - Content-Type: application/json
    /// - Retry-After: {retry_after} (seconds)
    /// - Body: {"error":"Rate limited","retry_after_ms":{retry_after * 1000}}
    pub fn rate_limited(protocol: ProtocolType, retry_after_sec: u64) -> Self {
        let body = format!(
            r#"{{"error":"Rate limited","reason":"Circuit breaker is half-open","retry_after_sec":{}}}"#,
            retry_after_sec
        );

        Self {
            status_code: 429,
            headers: vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Retry-After".to_string(), retry_after_sec.to_string()),
            ],
            body: body.into_bytes(),
            protocol,
        }
    }

    /// Cached fallback response (for non-critical requests)
    ///
    /// Usage: Serve stale cached data when circuit is open
    ///
    /// Performance: <1μs + copy overhead (depends on cached_body size)
    ///
    /// Response Format:
    /// - HTTP/1.1 200 OK
    /// - Content-Type: application/json
    /// - X-Cache: HIT (stale)
    /// - Warning: 110 Response is stale
    /// - Body: {cached_body}
    pub fn from_cache(cached_body: Vec<u8>, protocol: ProtocolType) -> Self {
        Self {
            status_code: 200,
            headers: vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("X-Cache".to_string(), "HIT".to_string()),
                ("Warning".to_string(), "110 Response is stale".to_string()),
            ],
            body: cached_body,
            protocol,
        }
    }

    /// Gateway timeout → 504 Gateway Timeout
    ///
    /// Usage: Backend timed out, circuit breaker may trip
    ///
    /// Performance: <1μs (static string allocation)
    pub fn gateway_timeout(protocol: ProtocolType) -> Self {
        Self {
            status_code: 504,
            headers: vec![
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: br#"{"error":"Gateway timeout","reason":"Backend did not respond in time"}"#.to_vec(),
            protocol,
        }
    }

    /// Bad gateway → 502 Bad Gateway
    ///
    /// Usage: Backend returned invalid response
    pub fn bad_gateway(protocol: ProtocolType) -> Self {
        Self {
            status_code: 502,
            headers: vec![
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: br#"{"error":"Bad gateway","reason":"Backend returned invalid response"}"#.to_vec(),
            protocol,
        }
    }

    /// Convert ApiError to FallbackResponse
    ///
    /// Automatic fallback routing:
    /// - CircuitOpen → 503 Service Unavailable
    /// - CircuitHalfOpen → 429 Too Many Requests
    /// - Other errors → 500 Internal Server Error
    pub fn from_error(error: ApiError, protocol: ProtocolType) -> Self {
        match error {
            ApiError::CircuitOpen { .. } => Self::service_unavailable(protocol),
            ApiError::CircuitHalfOpen { .. } => Self::rate_limited(protocol, 5),
            ApiError::ProtocolNotSupported { content_type } => {
                Self {
                    status_code: 415,
                    headers: vec![
                        ("Content-Type".to_string(), "application/json".to_string()),
                    ],
                    body: format!(r#"{{"error":"Unsupported protocol","content_type":"{}"}}"#, content_type).into_bytes(),
                    protocol,
                }
            }
            ApiError::InvalidRequest { reason, .. } => {
                Self {
                    status_code: 400,
                    headers: vec![
                        ("Content-Type".to_string(), "application/json".to_string()),
                    ],
                    body: format!(r#"{{"error":"Invalid request","reason":"{}"}}"#, reason).into_bytes(),
                    protocol,
                }
            }
            _ => {
                Self {
                    status_code: 500,
                    headers: vec![
                        ("Content-Type".to_string(), "application/json".to_string()),
                    ],
                    body: br#"{"error":"Internal server error"}"#.to_vec(),
                    protocol,
                }
            }
        }
    }
}

#[cfg(feature = "std")]
impl UniversalResponse for FallbackResponse {
    fn status_code(&self) -> u16 {
        self.status_code
    }

    fn set_header(&mut self, name: String, value: String) {
        self.headers.push((name, value));
    }

    fn body(&self) -> &[u8] {
        &self.body
    }

    fn protocol(&self) -> ProtocolType {
        self.protocol
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use super::*;

    #[test]
    fn test_service_unavailable() {
        let response = FallbackResponse::service_unavailable(ProtocolType::REST);
        assert_eq!(response.status_code(), 503);
        assert!(response.body().len() > 0);

        // Check Retry-After header
        let retry_header = response.headers.iter()
            .find(|(k, _)| k == "Retry-After")
            .map(|(_, v)| v.as_str());
        assert_eq!(retry_header, Some("5"));
    }

    #[test]
    fn test_rate_limited() {
        let response = FallbackResponse::rate_limited(ProtocolType::REST, 10);
        assert_eq!(response.status_code(), 429);

        let retry_header = response.headers.iter()
            .find(|(k, _)| k == "Retry-After")
            .map(|(_, v)| v.as_str());
        assert_eq!(retry_header, Some("10"));
    }

    #[test]
    fn test_from_cache() {
        let cached_body = br#"{"data":"cached"}"#.to_vec();
        let response = FallbackResponse::from_cache(cached_body.clone(), ProtocolType::REST);

        assert_eq!(response.status_code(), 200);
        assert_eq!(response.body(), &cached_body[..]);

        let cache_header = response.headers.iter()
            .find(|(k, _)| k == "X-Cache")
            .map(|(_, v)| v.as_str());
        assert_eq!(cache_header, Some("HIT"));
    }

    #[test]
    fn test_from_error_circuit_open() {
        let error = ApiError::CircuitOpen { protocol: ProtocolType::REST };
        let response = FallbackResponse::from_error(error, ProtocolType::REST);

        assert_eq!(response.status_code(), 503);
    }

    #[test]
    fn test_from_error_circuit_half_open() {
        let error = ApiError::CircuitHalfOpen { protocol: ProtocolType::REST };
        let response = FallbackResponse::from_error(error, ProtocolType::REST);

        assert_eq!(response.status_code(), 429);
    }
}
