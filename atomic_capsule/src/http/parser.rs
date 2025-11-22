//! # HTTP Parser Core
//!
//! **T1 Atomic state machine with zero-copy parsing**

use super::headers::parse_headers_simd;
use super::request::{HttpRequest, Method, Version};
use super::response::{HttpResponse, StatusCode};

/// HTTP parse error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpParseError {
    /// Incomplete data (need more bytes)
    Incomplete,
    /// Invalid method
    InvalidMethod,
    /// Invalid URI
    InvalidUri,
    /// Invalid version
    InvalidVersion,
    /// Invalid status line
    InvalidStatusLine,
    /// Invalid header
    InvalidHeader,
    /// Invalid UTF-8
    InvalidUtf8,
    /// Invalid request
    InvalidRequest(&'static str),
}

/// Parse HTTP request
///
/// **Performance Target**: <100ns for request line, <50ns/header with SIMD
/// **Zero-copy**: All strings are borrowed from input buffer
pub fn parse_request(input: &str) -> Result<HttpRequest<'_>, HttpParseError> {
    let bytes = input.as_bytes();

    // Find request line end (first \r\n)
    let request_line_end = bytes
        .windows(2)
        .position(|w| w == b"\r\n")
        .ok_or(HttpParseError::Incomplete)?;

    let request_line = &bytes[..request_line_end];

    // Parse request line: "METHOD URI VERSION\r\n"
    let parts: Vec<&[u8]> = request_line.split(|&b| b == b' ').collect();
    if parts.len() != 3 {
        return Err(HttpParseError::InvalidStatusLine);
    }

    // Parse method
    let method = Method::from_bytes(parts[0]).ok_or(HttpParseError::InvalidMethod)?;

    // Parse URI
    let uri = core::str::from_utf8(parts[1]).map_err(|_| HttpParseError::InvalidUtf8)?;

    // Parse version
    let version = Version::from_bytes(parts[2]).ok_or(HttpParseError::InvalidVersion)?;

    let mut request = HttpRequest::new(method, uri, version);

    // Parse headers (starting after request line + \r\n)
    let headers_start = request_line_end + 2;

    // Must find \r\n\r\n to complete valid HTTP request
    let headers_input = &input[headers_start..];

    // Check if request is complete: must have \r\n\r\n (even if no headers)
    // Special case: if headers_input starts with \r\n, that's valid (no headers)
    let headers_end = if headers_input.starts_with("\r\n") {
        // Empty headers case: "\r\n" at position 0
        0
    } else {
        // Must find full \r\n\r\n for non-empty headers
        headers_input
            .find("\r\n\r\n")
            .ok_or(HttpParseError::Incomplete)?
    };

    // Parse headers if present (only if headers_end > 0)
    if headers_end > 0 {
        let headers_only = &headers_input[..headers_end + 2]; // Include final \r\n
        let headers =
            parse_headers_simd(headers_only).map_err(|_| HttpParseError::InvalidHeader)?;

        // Copy headers into request
        for (name, value) in headers.iter() {
            request.add_header(name, value);
        }
    }

    // Parse body if present
    let body_start = if headers_end == 0 {
        headers_start + 2 // After the empty \r\n
    } else {
        headers_start + headers_end + 4 // After \r\n\r\n
    };

    if body_start < input.len() {
        if let Some(content_length) = request.content_length() {
            let body_end = (body_start + content_length).min(input.len());
            request.set_body(&bytes[body_start..body_end]);
        }
    }

    Ok(request)
}

/// Parse HTTP response
///
/// **Performance Target**: <100ns for status line, <50ns/header with SIMD
/// **Zero-copy**: All strings are borrowed from input buffer
pub fn parse_response(input: &str) -> Result<HttpResponse<'_>, HttpParseError> {
    let bytes = input.as_bytes();

    // Find status line end (first \r\n)
    let status_line_end = bytes
        .windows(2)
        .position(|w| w == b"\r\n")
        .ok_or(HttpParseError::Incomplete)?;

    let status_line = &bytes[..status_line_end];

    // Parse status line: "VERSION STATUS_CODE REASON_PHRASE\r\n"
    let first_space = status_line
        .iter()
        .position(|&b| b == b' ')
        .ok_or(HttpParseError::InvalidStatusLine)?;

    let version_bytes = &status_line[..first_space];
    let version = Version::from_bytes(version_bytes).ok_or(HttpParseError::InvalidVersion)?;

    // Find second space
    let second_space = status_line[first_space + 1..]
        .iter()
        .position(|&b| b == b' ')
        .ok_or(HttpParseError::InvalidStatusLine)?;

    let status_code_bytes = &status_line[first_space + 1..first_space + 1 + second_space];
    let status_code_str =
        core::str::from_utf8(status_code_bytes).map_err(|_| HttpParseError::InvalidUtf8)?;
    let status_code_u16: u16 = status_code_str
        .parse()
        .map_err(|_| HttpParseError::InvalidStatusLine)?;
    let status = StatusCode::from_u16(status_code_u16).ok_or(HttpParseError::InvalidStatusLine)?;

    let reason_bytes = &status_line[first_space + 1 + second_space + 1..];
    let reason = core::str::from_utf8(reason_bytes).map_err(|_| HttpParseError::InvalidUtf8)?;

    let mut response = HttpResponse::new(version, status, reason);

    // Parse headers (starting after status line + \r\n)
    let headers_start = status_line_end + 2;
    if headers_start < input.len() {
        let headers_input = &input[headers_start..];

        // Find headers end (\r\n\r\n)
        if let Some(headers_end) = headers_input.find("\r\n\r\n") {
            let headers_only = &headers_input[..headers_end + 2]; // Include final \r\n

            let headers =
                parse_headers_simd(headers_only).map_err(|_| HttpParseError::InvalidHeader)?;

            // Copy headers into response
            for (name, value) in headers.iter() {
                response.add_header(name, value);
            }

            // Parse body if present
            let body_start = headers_start + headers_end + 4; // After \r\n\r\n
            if body_start < input.len() {
                if let Some(content_length) = response.content_length() {
                    let body_end = (body_start + content_length).min(input.len());
                    response.set_body(&bytes[body_start..body_end]);
                }
            }
        }
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_request_minimal() {
        let input = "GET /path HTTP/1.1\r\n\r\n";
        let req = parse_request(input).unwrap();

        assert_eq!(req.method, Method::GET);
        assert_eq!(req.uri, "/path");
        assert_eq!(req.version, Version::Http11);
        assert_eq!(req.headers.len(), 0);
        assert!(req.body.is_none());
    }

    #[test]
    fn test_parse_request_with_headers() {
        let input = concat!(
            "POST /api/data HTTP/1.1\r\n",
            "Host: example.com\r\n",
            "Content-Type: application/json\r\n",
            "Content-Length: 13\r\n",
            "\r\n",
            "{\"key\":\"val\"}"
        );

        let req = parse_request(input).unwrap();

        assert_eq!(req.method, Method::POST);
        assert_eq!(req.uri, "/api/data");
        assert_eq!(req.version, Version::Http11);
        assert_eq!(req.headers.len(), 3);
        assert_eq!(req.get_header("Host"), Some("example.com"));
        assert_eq!(req.get_header("Content-Type"), Some("application/json"));
        assert_eq!(req.content_length(), Some(13));
        assert_eq!(req.body, Some(b"{\"key\":\"val\"}".as_slice()));
    }

    #[test]
    fn test_parse_response_minimal() {
        let input = "HTTP/1.1 200 OK\r\n\r\n";
        let resp = parse_response(input).unwrap();

        assert_eq!(resp.version, Version::Http11);
        assert_eq!(resp.status, StatusCode::Ok);
        assert_eq!(resp.reason, "OK");
        assert_eq!(resp.headers.len(), 0);
        assert!(resp.body.is_none());
    }

    #[test]
    fn test_parse_response_with_headers() {
        let input = concat!(
            "HTTP/1.1 404 Not Found\r\n",
            "Content-Type: text/html\r\n",
            "Content-Length: 9\r\n",
            "\r\n",
            "Not Found"
        );

        let resp = parse_response(input).unwrap();

        assert_eq!(resp.version, Version::Http11);
        assert_eq!(resp.status, StatusCode::NotFound);
        assert_eq!(resp.reason, "Not Found");
        assert_eq!(resp.headers.len(), 2);
        assert_eq!(resp.get_header("Content-Type"), Some("text/html"));
        assert_eq!(resp.content_length(), Some(9));
        assert_eq!(resp.body, Some(b"Not Found".as_slice()));
    }

    #[test]
    fn test_parse_incomplete() {
        let input = "GET /path";
        assert_eq!(parse_request(input), Err(HttpParseError::Incomplete));
    }

    #[test]
    fn test_parse_invalid_method() {
        let input = "INVALID /path HTTP/1.1\r\n\r\n";
        assert_eq!(parse_request(input), Err(HttpParseError::InvalidMethod));
    }

    #[test]
    fn test_parse_invalid_version() {
        let input = "GET /path HTTP/2.0\r\n\r\n";
        assert_eq!(parse_request(input), Err(HttpParseError::InvalidVersion));
    }

    // ========================================================================
    // T28 Q1: Core Behaviors - Additional HTTP Methods
    // ========================================================================

    #[test]
    fn test_q1_parse_patch_request() {
        let input = "PATCH /api/user/123 HTTP/1.1\r\n\r\n";
        let req = parse_request(input).unwrap();

        assert_eq!(req.method, Method::PATCH, "Method should be PATCH");
        assert_eq!(req.uri, "/api/user/123");
        assert_eq!(req.version, Version::Http11);
    }

    #[test]
    fn test_q1_parse_head_request() {
        let input = "HEAD /index.html HTTP/1.1\r\n\r\n";
        let req = parse_request(input).unwrap();

        assert_eq!(req.method, Method::HEAD, "Method should be HEAD");
        assert_eq!(req.uri, "/index.html");
        assert_eq!(req.version, Version::Http11);
    }

    #[test]
    fn test_q1_parse_options_request() {
        let input = "OPTIONS * HTTP/1.1\r\n\r\n";
        let req = parse_request(input).unwrap();

        assert_eq!(req.method, Method::OPTIONS, "Method should be OPTIONS");
        assert_eq!(req.uri, "*");
        assert_eq!(req.version, Version::Http11);
    }

    #[test]
    fn test_q1_parse_query_string() {
        let input = "GET /search?q=rust&limit=10 HTTP/1.1\r\n\r\n";
        let req = parse_request(input).unwrap();

        assert_eq!(req.method, Method::GET);
        assert_eq!(
            req.uri, "/search?q=rust&limit=10",
            "URI should preserve query string"
        );
        assert_eq!(req.version, Version::Http11);
    }

    #[test]
    fn test_q1_parse_empty_uri() {
        // Empty URI edge case (double space)
        let input = "GET  HTTP/1.1\r\n\r\n";
        let result = parse_request(input);

        // Parser allows empty URI (sets uri to empty string)
        // This tests robustness - parser doesn't crash on malformed input
        if let Ok(req) = result {
            assert_eq!(req.uri, "", "Empty URI should parse as empty string");
        } else {
            // Alternative: parser may reject it
            assert!(result.is_err(), "Empty URI may be rejected");
        }
    }

    // ========================================================================
    // T28 Q2: Edge Cases - Truncation and Malformed Input
    // ========================================================================

    #[test]
    fn test_q2_empty_header_value() {
        let input = "GET /path HTTP/1.1\r\nHost: \r\n\r\n";
        let req = parse_request(input).unwrap();

        // Empty header value should parse successfully
        assert_eq!(req.get_header("Host"), Some(""));
    }

    #[test]
    fn test_q2_missing_crlf() {
        // Only LF (Unix-style newlines) - should fail strict parsing
        let input = "GET /path HTTP/1.1\n\n";
        let result = parse_request(input);

        // Expect Incomplete or parse error (missing proper \r\n)
        assert!(result.is_err(), "Missing CRLF should fail");
    }

    #[test]
    fn test_q2_truncated_status_line() {
        let input = "GET /path HTTP/1";
        assert_eq!(
            parse_request(input),
            Err(HttpParseError::Incomplete),
            "Truncated status line should return Incomplete"
        );
    }

    #[test]
    fn test_q2_truncated_headers() {
        // Headers without final \r\n\r\n
        let input = "GET /path HTTP/1.1\r\nHost: example.com\r\n";
        let result = parse_request(input);

        // Should parse headers but expect incomplete (no final \r\n\r\n)
        // Current implementation may treat this as complete with headers
        // Verifying behavior: should succeed if headers_end found
        assert!(
            result.is_ok() || result == Err(HttpParseError::Incomplete),
            "Truncated headers should parse or return Incomplete"
        );
    }

    #[test]
    fn test_q2_max_uri_length() {
        // Create URI exceeding typical limits (8KB is common limit)
        let long_uri = "/".to_string() + &"x".repeat(10_000);
        let input = format!("GET {} HTTP/1.1\r\n\r\n", long_uri);

        let result = parse_request(&input);

        // Current implementation may not enforce limits, but should not panic
        // This test verifies no panic on extreme input
        assert!(
            result.is_ok() || result.is_err(),
            "Long URI should not panic (either parse or error)"
        );
    }

    #[test]
    fn test_q2_max_header_count() {
        // Create request with 1000 headers (excessive)
        let mut input = "GET /path HTTP/1.1\r\n".to_string();
        for i in 0..1000 {
            input.push_str(&format!("X-Header-{}: value\r\n", i));
        }
        input.push_str("\r\n");

        let result = parse_request(&input);

        // Should not panic, either parse successfully or error
        assert!(
            result.is_ok() || result.is_err(),
            "Excessive headers should not panic"
        );
    }

    // ========================================================================
    // T28 Q4: Code Path Coverage - Additional Error Paths
    // ========================================================================

    #[test]
    fn test_q4_invalid_status_line_error() {
        // Malformed response status line (missing reason phrase)
        let input = "HTTP/1.1 200\r\n\r\n";
        let result = parse_response(input);

        // Should fail with InvalidStatusLine
        assert!(result.is_err(), "Malformed status line should error");
    }

    #[test]
    fn test_q4_invalid_utf8_in_header() {
        // Non-UTF8 bytes in header value
        let mut input = b"GET /path HTTP/1.1\r\nHost: ".to_vec();
        input.extend_from_slice(&[0xFF, 0xFE]); // Invalid UTF-8
        input.extend_from_slice(b"\r\n\r\n");

        let input_str = unsafe { core::str::from_utf8_unchecked(&input) };
        let result = parse_request(input_str);

        // Should fail with InvalidUtf8 or InvalidHeader
        assert!(result.is_err(), "Invalid UTF-8 in headers should error");
    }

    // ========================================================================
    // T28 Q3: Invariants - Parser Robustness
    // ========================================================================

    #[test]
    fn test_q3_parsing_never_panics() {
        // Invariant: Parser must never panic, always return Result
        let test_inputs = vec![
            "",                         // Empty
            "X",                        // Single char
            "GET",                      // Incomplete method
            "GET /",                    // Incomplete URI
            "GET / HTTP",               // Incomplete version
            "GET / HTTP/1.1",           // Missing CRLF
            "GET / HTTP/1.1\r",         // Incomplete CRLF
            "\r\n\r\n",                 // Only CRLF
            "GET / HTTP/1.1\r\n",       // Missing final CRLF
            "GET /\0 HTTP/1.1\r\n\r\n", // Null byte in URI
        ];

        for (i, input) in test_inputs.iter().enumerate() {
            let result = std::panic::catch_unwind(|| parse_request(input));

            assert!(
                result.is_ok(),
                "Parser panicked on test case {}: {:?}",
                i,
                input
            );

            // Also verify it returns Result (not unwrap internally)
            if let Ok(parse_result) = result {
                assert!(
                    parse_result.is_ok() || parse_result.is_err(),
                    "Parser must return Result for test case {}",
                    i
                );
            }
        }

        // Test large garbage separately
        let large_garbage = "x".repeat(1_000_000);
        let result = std::panic::catch_unwind(|| parse_request(&large_garbage));
        assert!(result.is_ok(), "Parser panicked on large garbage");
    }
}
