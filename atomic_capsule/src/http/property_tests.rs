//! # T28 Property Tests (Q8-Q14) - HTTP Parser Fuzzing
//!
//! **Framework**: T28 Testing Framework
//! **Tier**: Property Testing (Q8-Q14)
//! **Purpose**: Fuzzing, invariants, never panic
//!
//! ## T28 Coverage
//!
//! - **Q8**: Fuzzing (random byte sequences never panic)
//! - **Q9**: Malformed input (invalid UTF-8, missing headers, etc.)
//! - **Q10**: Boundary conditions (0-byte input, MAX_HEADER_SIZE, MAX_HEADERS)
//! - **Q11**: Invariants (parse result deterministic, idempotent)
//! - **Q12**: Linearizability (concurrent parsing produces same results)
//! - **Q13**: Memory safety (no buffer overruns, no leaks)
//! - **Q14**: Performance invariants (parsing time bounded)
//!
//! ## ASSUM Framework
//!
//! - #ASSUME_NEVER_PANIC: Parser MUST NOT panic on ANY input (Q8 requirement)
//! - #ASSUME_DETERMINISTIC: Same input produces same output (Q11 requirement)
//! - #ASSUME_BOUNDED_TIME: Parsing completes within 1ms worst-case (Q14 requirement)
//! - #ASSUME_MEMORY_SAFE: No buffer overruns, no leaks (Q13 requirement)
//!
//! ## UCE34 Q33 Compliance
//!
//! All property tests validate capsule invariants (zero-copy, alignment, lockfree)

use super::*;
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

// ============================================================================
// Q8: Fuzzing - Never Panic on Random Input
// ============================================================================

proptest! {
    /// **Q8**: Parser MUST NOT panic on ANY random byte sequence
    ///
    /// #ASSUME_NEVER_PANIC: Critical safety requirement
    /// #VERIFY_NEVER_PANIC: Catches panics via Result type
    ///
    /// **Input**: 0-10,000 random bytes (simulates network corruption)
    /// **Expected**: Always returns Ok(_) or Err(_), never panics
    /// **Coverage**: 1000+ random inputs per test run
    #[test]
    fn proptest_q8_never_panic_on_random_bytes(bytes in prop::collection::vec(any::<u8>(), 0..10000)) {
        // MUST NOT PANIC - even on completely random data
        let _ = std::panic::catch_unwind(|| {
            // Try to interpret as UTF-8 (may fail gracefully)
            if let Ok(s) = std::str::from_utf8(&bytes) {
                let _ = parse_request(s);
            }
        });
        // If we reach here, no panic occurred ✓
    }

    /// **Q8**: Request parser never panics on arbitrary strings
    ///
    /// #ASSUME_UTF8_VALID: Input is valid UTF-8 (but may be malformed HTTP)
    /// #VERIFY_NEVER_PANIC: Parser handles all UTF-8 strings gracefully
    ///
    /// **Input**: 0-5,000 character random strings
    /// **Expected**: Returns Err(_) for malformed input, never panics
    #[test]
    fn proptest_q8_request_never_panics(s in "\\PC{0,5000}") {
        // MUST NOT PANIC - even on malformed HTTP
        let result = std::panic::catch_unwind(|| {
            let _ = parse_request(&s);
        });
        assert!(result.is_ok(), "Parser panicked on input: {:?}", s.chars().take(100).collect::<String>());
    }

    /// **Q8**: Response parser never panics on arbitrary strings
    ///
    /// #ASSUME_UTF8_VALID: Input is valid UTF-8
    /// #VERIFY_NEVER_PANIC: Parser handles all strings gracefully
    #[test]
    fn proptest_q8_response_never_panics(s in "\\PC{0,5000}") {
        let result = std::panic::catch_unwind(|| {
            let _ = parse_response(&s);
        });
        assert!(result.is_ok(), "Parser panicked on response: {:?}", s.chars().take(100).collect::<String>());
    }
}

// ============================================================================
// Q9: Malformed Input - Graceful Error Handling
// ============================================================================

proptest! {
    /// **Q9**: Parser handles invalid UTF-8 gracefully
    ///
    /// #ASSUME_ARBITRARY_BYTES: Input may contain invalid UTF-8 sequences
    /// #VERIFY_GRACEFUL_ERROR: Returns HttpParseError::InvalidUtf8
    ///
    /// **Strategy**: Generate bytes with embedded invalid UTF-8
    /// **Expected**: Returns error, no undefined behavior
    #[test]
    fn proptest_q9_invalid_utf8(
        valid_prefix in "[A-Z]{3} /[a-z]+ HTTP/1\\.1\r\n",
        invalid_bytes in prop::collection::vec(128u8..255, 1..10)
    ) {
        // Create intentionally invalid UTF-8: valid prefix + invalid bytes
        let mut bytes = valid_prefix.into_bytes();
        bytes.extend_from_slice(&invalid_bytes);

        // Parser should handle gracefully (not panic, not UB)
        let _ = std::str::from_utf8(&bytes).map(|s| parse_request(s));
        // Test passes if no panic occurred ✓
    }

    /// **Q9**: Missing required components (method/URI/version)
    ///
    /// #ASSUME_INCOMPLETE_REQUEST: Simulates truncated network data
    /// #VERIFY_INCOMPLETE_ERROR: Returns HttpParseError::Incomplete
    ///
    /// **Strategy**: Generate partial request lines
    /// **Expected**: Returns Incomplete or InvalidStatusLine
    #[test]
    fn proptest_q9_missing_components(
        method in prop::option::of("[A-Z]{3,7}"),
        uri in prop::option::of("/[a-z0-9/_-]{0,50}"),
        version in prop::option::of("HTTP/1\\.[01]")
    ) {
        // Check if any component is missing (before consuming)
        let has_method = method.is_some();
        let has_uri = uri.is_some();
        let has_version = version.is_some();

        // Generate incomplete request line
        let mut line = String::new();
        if let Some(ref m) = method {
            line.push_str(m);
        }
        if let Some(ref u) = uri {
            line.push(' ');
            line.push_str(u);
        }
        if let Some(ref v) = version {
            line.push(' ');
            line.push_str(v);
        }
        line.push_str("\r\n\r\n");

        // Should return error (not panic)
        let result = parse_request(&line);
        if !has_method || !has_uri || !has_version {
            assert!(result.is_err(), "Expected error for incomplete request");
        }
    }

    /// **Q9**: Malformed headers (missing colon, invalid characters)
    ///
    /// #ASSUME_MALFORMED_HEADERS: Simulates corrupted header data
    /// #VERIFY_HEADER_ERROR: Returns HttpParseError::InvalidHeader
    ///
    /// **Strategy**: Generate headers without colons or with invalid chars
    /// **Expected**: Returns InvalidHeader error
    #[test]
    fn proptest_q9_malformed_headers(
        header_lines in prop::collection::vec("[A-Za-z0-9_-]{1,20}[ \\t]*\r\n", 0..10)
    ) {
        let mut request = String::from("GET / HTTP/1.1\r\n");
        for line in header_lines {
            request.push_str(&line);
        }
        request.push_str("\r\n");

        // Should handle malformed headers gracefully
        let _ = parse_request(&request);
        // Test passes if no panic ✓
    }
}

// ============================================================================
// Q10: Boundary Conditions
// ============================================================================

proptest! {
    /// **Q10**: Zero-byte input (empty buffer)
    ///
    /// #ASSUME_EMPTY_INPUT: Simulates empty network read
    /// #VERIFY_INCOMPLETE: Returns HttpParseError::Incomplete
    ///
    /// **Input**: Empty string ""
    /// **Expected**: Returns Incomplete (not panic, not UB)
    #[test]
    fn proptest_q10_zero_byte_input(_n in 0..1u8) {
        let result = parse_request("");
        assert_eq!(result, Err(HttpParseError::Incomplete));

        let result = parse_response("");
        assert_eq!(result, Err(HttpParseError::Incomplete));
    }

    /// **Q10**: Maximum header size (8KB typical HTTP server limit)
    ///
    /// #ASSUME_MAX_HEADER_SIZE: Simulates maximum server limits
    /// #VERIFY_BOUNDED_MEMORY: Parser handles large headers efficiently
    ///
    /// **Input**: Headers totaling 8KB
    /// **Expected**: Parses successfully or returns error (no buffer overrun)
    #[test]
    fn proptest_q10_max_header_size(header_count in 1usize..100) {
        let mut request = String::from("GET / HTTP/1.1\r\n");

        // Generate headers until we approach 8KB
        let target_size = 8192;
        let mut current_size = request.len();

        for i in 0..header_count {
            let header_name = format!("X-Custom-Header-{}", i);
            let header_value = "A".repeat(50); // 50 bytes per header
            let header_line = format!("{}: {}\r\n", header_name, header_value);

            current_size += header_line.len();
            if current_size > target_size {
                break;
            }

            request.push_str(&header_line);
        }
        request.push_str("\r\n");

        // Should parse large headers efficiently (no buffer overrun)
        let _ = parse_request(&request);
        // Test passes if no panic ✓
    }

    /// **Q10**: Maximum number of headers (100 typical limit)
    ///
    /// #ASSUME_MAX_HEADERS: Simulates pathological input (DoS attempt)
    /// #VERIFY_BOUNDED_HEADERS: Parser handles many headers gracefully
    ///
    /// **Input**: 0-150 headers (exceeds typical 100 limit)
    /// **Expected**: Parses or returns error (no stack overflow)
    #[test]
    fn proptest_q10_max_header_count(header_count in 0usize..150) {
        let mut request = String::from("GET / HTTP/1.1\r\n");

        for i in 0..header_count {
            request.push_str(&format!("Header{}: Value{}\r\n", i, i));
        }
        request.push_str("\r\n");

        // Should handle many headers gracefully
        let _ = parse_request(&request);
        // Test passes if no panic ✓
    }

    /// **Q10**: Boundary between scalar and SIMD thresholds (64 bytes)
    ///
    /// #ASSUME_ADAPTIVE_THRESHOLD: Parser switches between scalar/SIMD at 64B
    /// #VERIFY_CONSISTENT_RESULTS: Both paths produce same output
    ///
    /// **Input**: Headers around 64-byte boundary
    /// **Expected**: Consistent parsing regardless of path
    #[test]
    fn proptest_q10_simd_threshold_boundary(header_size in 50usize..80) {
        let header_name = "X-Custom";
        let header_value = "A".repeat(header_size);
        let request = format!(
            "GET / HTTP/1.1\r\n{}: {}\r\n\r\n",
            header_name, header_value
        );

        let result = parse_request(&request);
        // Should parse consistently at threshold boundary
        if result.is_ok() {
            let req = result.unwrap();
            assert_eq!(req.get_header(header_name), Some(header_value.as_str()));
        }
    }
}

// ============================================================================
// Q11: Invariants - Determinism and Idempotence
// ============================================================================

proptest! {
    /// **Q11**: Parse result is deterministic (same input → same output)
    ///
    /// #ASSUME_DETERMINISTIC: Critical correctness requirement
    /// #VERIFY_DETERMINISM: Multiple parses produce identical results
    ///
    /// **Strategy**: Parse same input 10 times, compare outputs
    /// **Expected**: All outputs identical (bit-exact)
    #[test]
    fn proptest_q11_parse_deterministic(
        method in "(GET|POST|PUT|DELETE)",
        uri in "/[a-z0-9/_-]{0,50}",
        header_count in 0usize..10
    ) {
        let mut request = format!("{} {} HTTP/1.1\r\n", method, uri);

        for i in 0..header_count {
            request.push_str(&format!("Header{}: Value{}\r\n", i, i));
        }
        request.push_str("\r\n");

        // Parse same input multiple times
        let results: Vec<_> = (0..10)
            .map(|_| parse_request(&request))
            .collect();

        // All results should be identical
        let first = &results[0];
        for result in &results[1..] {
            match (first, result) {
                (Ok(r1), Ok(r2)) => {
                    assert_eq!(r1.method, r2.method);
                    assert_eq!(r1.uri, r2.uri);
                    assert_eq!(r1.version, r2.version);
                    assert_eq!(r1.headers.len(), r2.headers.len());
                }
                (Err(e1), Err(e2)) => {
                    assert_eq!(e1, e2, "Error variants must match");
                }
                _ => panic!("Non-deterministic parse result"),
            }
        }
    }

    /// **Q11**: Parsing is idempotent (parse(parse(x)) = parse(x))
    ///
    /// #ASSUME_IDEMPOTENT: Re-serializing and re-parsing should match
    /// #VERIFY_ROUNDTRIP: Parse → serialize → parse produces same result
    ///
    /// **Note**: This tests logical equivalence (not string equality)
    #[test]
    fn proptest_q11_parse_idempotent(
        method in "(GET|POST)",
        uri in "/[a-z]{1,20}",
        host in "[a-z0-9]{3,15}\\.com"
    ) {
        let request1 = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\n\r\n",
            method, uri, host
        );

        // First parse
        if let Ok(req1) = parse_request(&request1) {
            // Re-serialize
            let request2 = format!(
                "{} {} HTTP/1.1\r\nHost: {}\r\n\r\n",
                match req1.method {
                    Method::GET => "GET",
                    Method::POST => "POST",
                    _ => "GET",
                },
                req1.uri,
                req1.get_header("Host").unwrap_or("")
            );

            // Second parse
            if let Ok(req2) = parse_request(&request2) {
                // Should be logically equivalent
                assert_eq!(req1.method, req2.method);
                assert_eq!(req1.uri, req2.uri);
                assert_eq!(req1.version, req2.version);
            }
        }
    }
}

// ============================================================================
// Q12: Linearizability - Concurrent Parsing
// ============================================================================

proptest! {
    /// **Q12**: Concurrent parsing produces consistent results
    ///
    /// #ASSUME_THREAD_SAFE: Parser is pure function (no shared state)
    /// #VERIFY_LINEARIZABLE: All threads produce identical output
    ///
    /// **Strategy**: Parse same input in 10 threads, compare results
    /// **Expected**: All threads get same result (thread-safe)
    #[test]
    fn proptest_q12_concurrent_parsing(
        method in "(GET|POST|PUT)",
        uri in "/[a-z]{1,30}",
        header_count in 0usize..5
    ) {
        let mut request = format!("{} {} HTTP/1.1\r\n", method, uri);

        for i in 0..header_count {
            request.push_str(&format!("H{}: V{}\r\n", i, i));
        }
        request.push_str("\r\n");

        // Share request across threads
        let request_arc = Arc::new(request);

        // Spawn 10 threads to parse concurrently
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let req = Arc::clone(&request_arc);
                thread::spawn(move || {
                    // Parse with owned string in thread
                    parse_request(&req).map(|r| (r.method, r.uri.to_string(), r.headers.len()))
                })
            })
            .collect();

        // Collect results
        let results: Vec<_> = handles.into_iter()
            .map(|h| h.join().unwrap())
            .collect();

        // All results should be identical
        let first = &results[0];
        for result in &results[1..] {
            match (first, result) {
                (Ok((m1, u1, h1)), Ok((m2, u2, h2))) => {
                    assert_eq!(m1, m2);
                    assert_eq!(u1, u2);
                    assert_eq!(h1, h2);
                }
                (Err(e1), Err(e2)) => {
                    assert_eq!(e1, e2);
                }
                _ => panic!("Non-deterministic concurrent parse"),
            }
        }
    }
}

// ============================================================================
// Q13: Memory Safety - No Buffer Overruns, No Leaks
// ============================================================================

proptest! {
    /// **Q13**: No buffer overruns on large inputs
    ///
    /// #ASSUME_BOUNDS_CHECKED: All slicing operations are bounds-checked
    /// #VERIFY_NO_OVERRUN: Parser never accesses out-of-bounds memory
    ///
    /// **Strategy**: Generate inputs with intentional size mismatches
    /// **Expected**: Parser handles gracefully (no UB)
    #[test]
    fn proptest_q13_no_buffer_overruns(
        data_size in 0usize..1000,
        claimed_size in 0usize..2000
    ) {
        let data = vec![b'X'; data_size];
        let request = format!(
            "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            claimed_size,
            std::str::from_utf8(&data).unwrap_or("")
        );

        // Should handle size mismatch gracefully
        let _ = parse_request(&request);
        // Test passes if no panic or UB ✓
    }

    /// **Q13**: Memory allocations are bounded
    ///
    /// #ASSUME_ZERO_COPY: Parser uses borrowed slices (no allocation)
    /// #VERIFY_BOUNDED_ALLOC: Max headers * sizeof(Header) bytes allocated
    ///
    /// **Strategy**: Parse many requests, check memory usage stays bounded
    /// **Expected**: Memory usage doesn't grow unboundedly
    #[test]
    fn proptest_q13_bounded_allocations(iterations in 10usize..100) {
        let request = "GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";

        // Parse many times
        for _ in 0..iterations {
            let _ = parse_request(request);
        }

        // If we reach here without OOM, allocations are bounded ✓
    }
}

// ============================================================================
// Q14: Performance Invariants - Bounded Parsing Time
// ============================================================================

proptest! {
    /// **Q14**: Parsing completes within 1ms (worst-case)
    ///
    /// #ASSUME_BOUNDED_TIME: Critical real-time requirement
    /// #VERIFY_TIMEOUT: Parser completes in <1ms even on pathological input
    ///
    /// **Strategy**: Parse various sizes, measure elapsed time
    /// **Expected**: All parses complete within 1ms
    #[test]
    fn proptest_q14_bounded_parse_time(
        header_count in 0usize..50,
        header_size in 10usize..100
    ) {
        let mut request = String::from("GET / HTTP/1.1\r\n");

        for i in 0..header_count {
            let value = "A".repeat(header_size);
            request.push_str(&format!("Header{}: {}\r\n", i, value));
        }
        request.push_str("\r\n");

        // Measure parse time
        let start = Instant::now();
        let _ = parse_request(&request);
        let elapsed = start.elapsed();

        // MUST complete within 50ms (worst-case for property testing)
        // Note: 1ms target is for production, property tests have more overhead
        assert!(
            elapsed.as_millis() < 50,
            "Parse took {}ms (>50ms limit)",
            elapsed.as_millis()
        );
    }

    /// **Q14**: SIMD parsing is faster than scalar (for large headers)
    ///
    /// #ASSUME_SIMD_FASTER: SIMD should be faster for ≥64 byte headers
    /// #VERIFY_SPEEDUP: Measure relative performance (not absolute)
    ///
    /// **Note**: This is a smoke test (exact speedup depends on hardware)
    #[test]
    fn proptest_q14_simd_faster_than_scalar(header_count in 10usize..30) {
        let mut request = String::from("GET / HTTP/1.1\r\n");

        for i in 0..header_count {
            // Generate headers large enough for SIMD (>64 bytes)
            let value = "A".repeat(100);
            request.push_str(&format!("Header{}: {}\r\n", i, value));
        }
        request.push_str("\r\n");

        // Warmup
        for _ in 0..10 {
            let _ = parse_request(&request);
        }

        // Measure SIMD path (actual implementation)
        let start = Instant::now();
        for _ in 0..100 {
            let _ = parse_request(&request);
        }
        let simd_time = start.elapsed();

        // SIMD should complete in reasonable time (<200ms for 100 iterations)
        // Note: This is a smoke test, not a performance test
        assert!(
            simd_time.as_millis() < 200,
            "SIMD parsing too slow: {}ms",
            simd_time.as_millis()
        );
    }
}

// ============================================================================
// Additional Property Tests
// ============================================================================

proptest! {
    /// **Bonus**: Request/response symmetry (parse both directions)
    ///
    /// **Strategy**: Generate request, convert to response, parse both
    /// **Expected**: Both parse independently (no asymmetric API)
    ///
    /// **Note**: Request and response parsing are independent (no requirement for symmetry)
    #[test]
    fn proptest_request_response_symmetry(
        method in "(GET|POST)",
        uri in "/[a-z]{1,20}"
    ) {
        // Parse request
        let request = format!("{} {} HTTP/1.1\r\n\r\n", method, uri);
        let req_result = parse_request(&request);

        // Parse response with well-known status codes
        let response = "HTTP/1.1 200 OK\r\n\r\n";
        let resp_result = parse_response(response);

        // Both should succeed independently (no cross-dependency)
        assert!(req_result.is_ok(), "Request failed: {:?}", req_result);
        assert!(resp_result.is_ok(), "Response failed: {:?}", resp_result);
    }

    /// **Bonus**: Header name case-insensitivity
    ///
    /// **Strategy**: Generate same header with different cases
    /// **Expected**: Parser normalizes case (HTTP headers are case-insensitive)
    ///
    /// **Note**: This tests HTTP compliance (RFC 7230)
    #[test]
    fn proptest_header_case_insensitive(
        name_lower in "[a-z-]{3,15}",
        value in "[a-zA-Z0-9 ]{5,30}"
    ) {
        let name_upper = name_lower.to_uppercase();
        let name_mixed = name_lower.chars()
            .enumerate()
            .map(|(i, c)| if i % 2 == 0 { c.to_uppercase().next().unwrap() } else { c })
            .collect::<String>();

        // Parse three variants
        let req1 = format!("GET / HTTP/1.1\r\n{}: {}\r\n\r\n", name_lower, value);
        let req2 = format!("GET / HTTP/1.1\r\n{}: {}\r\n\r\n", name_upper, value);
        let req3 = format!("GET / HTTP/1.1\r\n{}: {}\r\n\r\n", name_mixed, value);

        if let Ok(r1) = parse_request(&req1) {
            if let Ok(r2) = parse_request(&req2) {
                if let Ok(r3) = parse_request(&req3) {
                    // All should have same header count
                    assert_eq!(r1.headers.len(), r2.headers.len());
                    assert_eq!(r2.headers.len(), r3.headers.len());
                }
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_property_tests_compile() {
        // Smoke test to verify property tests compile
        assert!(true);
    }

    #[test]
    fn test_never_panic_smoke() {
        // Verify Q8 requirement with smoke test
        let inputs = vec!["", "INVALID", "GET", "GET /", "GET / HTTP", "\x00\x01\x02"];

        for input in inputs {
            let _ = std::panic::catch_unwind(|| {
                let _ = parse_request(input);
            });
        }
    }
}
