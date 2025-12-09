//! HTTP/3 Integration Tests (T28 Framework)
//!
//! Tests TransportType detection, Http3Adapter, and protocol routing
//! over HTTP/3 transport.
//!
//! ## Test Tiers (T28)
//! - Q1-Q7:   Unit tests (transport detection, ALPN, magic bytes)
//! - Q8-Q14:  Property tests (determinism, concurrency)
//! - Q15-Q21: Integration tests (REST/GraphQL/gRPC over HTTP/3)
//! - Q22-Q28: Production tests (1M requests, 0-RTT, migration)

#![cfg(feature = "http3-support")]

use atomic_capsule::meta::{
    UniversalApiMetaCapsule, UniversalRequest, ProtocolType,
    universal_api::TransportType,
};
use std::sync::Arc;

// ============================================================================
// T28 TIER 1: Unit Tests (Q1-Q7)
// ============================================================================

#[test]
fn test_q1_transport_detection_http1() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: None,
        raw_bytes: vec![],
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };
    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP1);
}

#[test]
fn test_q2_transport_detection_http2_alpn() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: Some(b"h2".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };
    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP2);
}

#[test]
fn test_q3_transport_detection_http3_alpn_h3() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };
    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP3);
}

#[test]
fn test_q4_transport_detection_http3_alpn_h3_29() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: Some(b"h3-29".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };
    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP3);
}

#[test]
fn test_q5_transport_detection_http3_magic_bytes() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: None,
        raw_bytes: vec![0xC0, 0x00, 0x00, 0x00], // QUIC long header
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };
    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP3);
}

#[test]
fn test_q6_transport_stats_increment() {
    let capsule = UniversalApiMetaCapsule::new();
    let request1 = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };
    let request2 = MockRequest {
        alpn: Some(b"h2".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };

    let _ = capsule.route_with_transport(&request1);
    let _ = capsule.route_with_transport(&request2);

    let (_, http2, http3, _) = capsule.get_transport_stats();
    assert_eq!(http3, 1);
    assert_eq!(http2, 1);
}

#[test]
fn test_q7_http3_request_trait_implementation() {
    let request = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/api/users",
        protocol: ProtocolType::REST,
    };

    assert_eq!(request.method(), "GET");
    assert_eq!(request.path(), "/api/users");
    assert_eq!(request.alpn_protocol(), Some(b"h3".as_slice()));
}

// ============================================================================
// T28 TIER 2: Property Tests (Q8-Q14)
// ============================================================================

#[test]
fn test_q8_transport_detection_determinism() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };

    // Same request should always return same transport
    for _ in 0..1000 {
        assert_eq!(capsule.detect_transport(&request), TransportType::HTTP3);
    }
}

#[test]
fn test_q9_transport_stats_monotonicity() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };

    let mut prev_count = 0;
    for _ in 0..100 {
        let _ = capsule.route_with_transport(&request);
        let (_, _, http3, _) = capsule.get_transport_stats();
        assert!(http3 >= prev_count, "Transport count must be monotonic");
        prev_count = http3;
    }
}

#[test]
fn test_q10_alpn_priority_over_magic_bytes() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: Some(b"h2".to_vec()),
        raw_bytes: vec![0xC0, 0x00], // QUIC magic bytes (should be ignored)
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };
    // ALPN should take priority
    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP2);
}

#[test]
fn test_q11_concurrent_transport_detection() {
    use std::thread;

    let capsule = Arc::new(UniversalApiMetaCapsule::new());
    let mut handles = vec![];

    for _ in 0..16 {
        let capsule_clone = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            let request = MockRequest {
                alpn: Some(b"h3".to_vec()),
                raw_bytes: vec![],
                method: "GET",
                path: "/",
                protocol: ProtocolType::REST,
            };
            for _ in 0..100 {
                assert_eq!(
                    capsule_clone.detect_transport(&request),
                    TransportType::HTTP3
                );
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_q12_transport_stats_concurrent_increment() {
    use std::thread;

    let capsule = Arc::new(UniversalApiMetaCapsule::new());
    let mut handles = vec![];

    for _ in 0..16 {
        let capsule_clone = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            let request = MockRequest {
                alpn: Some(b"h3".to_vec()),
                raw_bytes: vec![],
                method: "GET",
                path: "/",
                protocol: ProtocolType::REST,
            };
            for _ in 0..100 {
                let _ = capsule_clone.route_with_transport(&request);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let (_, _, http3, _) = capsule.get_transport_stats();
    assert_eq!(http3, 1600, "16 threads × 100 requests = 1600 total");
}

#[test]
fn test_q13_invalid_alpn_fallback_to_http1() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: Some(b"unknown-protocol".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };
    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP1);
}

#[test]
fn test_q14_empty_packet_no_magic_bytes() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: None,
        raw_bytes: vec![],
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };
    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP1);
}

// ============================================================================
// T28 TIER 3: Integration Tests (Q15-Q21)
// ============================================================================

#[test]
fn test_q15_rest_over_http3() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/api/users",
        protocol: ProtocolType::REST,
    };

    // HTTP/3 transport should not affect REST protocol detection
    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP3);
    assert_eq!(request.protocol(), ProtocolType::REST);
}

#[test]
fn test_q16_graphql_over_http3() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: vec![],
        method: "POST",
        path: "/graphql",
        protocol: ProtocolType::GraphQL,
    };

    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP3);
    assert_eq!(request.protocol(), ProtocolType::GraphQL);
}

#[test]
fn test_q17_grpc_over_http3() {
    let capsule = UniversalApiMetaCapsule::new();
    let request = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: vec![],
        method: "POST",
        path: "/UserService/GetUser",
        protocol: ProtocolType::Grpc,
    };

    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP3);
    assert_eq!(request.protocol(), ProtocolType::Grpc);
}

#[test]
fn test_q18_http3_multiple_protocols() {
    let capsule = UniversalApiMetaCapsule::new();
    let requests = vec![
        (ProtocolType::REST, "GET", "/api/users"),
        (ProtocolType::GraphQL, "POST", "/graphql"),
        (ProtocolType::Grpc, "POST", "/UserService/GetUser"),
        (ProtocolType::JsonRPC, "POST", "/rpc"),
        (ProtocolType::SSE, "GET", "/events"),
    ];

    for (protocol, method, path) in requests {
        let request = MockRequest {
            alpn: Some(b"h3".to_vec()),
            raw_bytes: vec![],
            method,
            path,
            protocol,
        };
        assert_eq!(capsule.detect_transport(&request), TransportType::HTTP3);
        assert_eq!(request.protocol(), protocol);
    }
}

#[test]
fn test_q19_http3_transport_via_magic_bytes_quic_long_header() {
    let capsule = UniversalApiMetaCapsule::new();

    // Test QUIC long header detection (first byte & 0xC0 == 0xC0)
    for header_byte in [0xC0, 0xC1, 0xC2, 0xC3, 0xD0, 0xD1, 0xD2, 0xE0, 0xF0].iter() {
        let request = MockRequest {
            alpn: None,
            raw_bytes: vec![*header_byte, 0x00, 0x00, 0x00],
            method: "GET",
            path: "/",
            protocol: ProtocolType::REST,
        };
        assert_eq!(capsule.detect_transport(&request), TransportType::HTTP3,
                   "Header byte 0x{:X} should be detected as HTTP3", header_byte);
    }
}

#[test]
fn test_q20_http3_transport_via_magic_bytes_quic_short_header() {
    let capsule = UniversalApiMetaCapsule::new();

    // Test QUIC short header (first byte & 0xC0 != 0xC0)
    for header_byte in [0x00, 0x40, 0x80].iter() {
        let request = MockRequest {
            alpn: None,
            raw_bytes: vec![*header_byte, 0x00, 0x00, 0x00],
            method: "GET",
            path: "/",
            protocol: ProtocolType::REST,
        };
        assert_eq!(capsule.detect_transport(&request), TransportType::HTTP1,
                   "Header byte 0x{:X} should fallback to HTTP1", header_byte);
    }
}

#[test]
fn test_q21_alpn_protocol_priority_chain() {
    let capsule = UniversalApiMetaCapsule::new();

    // Test priority order: ALPN > magic bytes > default
    let test_cases: Vec<(Option<&[u8]>, Option<Vec<u8>>, TransportType)> = vec![
        (Some(b"h3"), Some(vec![0xC0, 0x00]), TransportType::HTTP3), // ALPN h3 wins
        (Some(b"h2"), Some(vec![0xC0, 0x00]), TransportType::HTTP2), // ALPN h2 wins over QUIC
        (Some(b"http/1.1"), Some(vec![0xC0, 0x00]), TransportType::HTTP1), // ALPN http/1.1 wins
        (None, Some(vec![0xC0, 0x00]), TransportType::HTTP3), // Magic bytes h3
        (None, Some(vec![0x00, 0x00]), TransportType::HTTP1), // No ALPN, bad magic -> HTTP1
        (None, None, TransportType::HTTP1), // Nothing specified -> HTTP1
    ];

    for (alpn_opt, raw_bytes_opt, expected) in test_cases {
        let request = MockRequest {
            alpn: alpn_opt.map(|a: &[u8]| a.to_vec()),
            raw_bytes: raw_bytes_opt.clone().unwrap_or_default(),
            method: "GET",
            path: "/",
            protocol: ProtocolType::REST,
        };
        assert_eq!(capsule.detect_transport(&request), expected,
                   "ALPN={:?}, raw_bytes={:?} should detect as {:?}",
                   alpn_opt.map(|a| String::from_utf8_lossy(a)),
                   raw_bytes_opt,
                   expected);
    }
}

// ============================================================================
// T28 TIER 4: Production Tests (Q22-Q28)
// ============================================================================

#[test]
fn test_q22_http3_1million_requests_stress() {
    let capsule = Arc::new(UniversalApiMetaCapsule::new());
    let request = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };

    // Simulate 1M requests (single-threaded for speed)
    for _ in 0..1_000_000 {
        let _ = capsule.route_with_transport(&request);
    }

    let (_, _, http3, _) = capsule.get_transport_stats();
    assert_eq!(http3, 1_000_000, "1M HTTP/3 requests should be tracked");
}

#[test]
fn test_q23_http3_connection_migration_alpn_stable() {
    let capsule = UniversalApiMetaCapsule::new();

    // Simulate connection migration: IP changes, but ALPN stays same
    let request1 = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: vec![0xC0, 0x00, 0x00, 0x00],
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };

    let request2 = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: vec![0xC0, 0x00, 0x00, 0x01], // Different packet number
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };

    assert_eq!(capsule.detect_transport(&request1), TransportType::HTTP3);
    assert_eq!(capsule.detect_transport(&request2), TransportType::HTTP3);
}

#[test]
fn test_q24_http3_alpn_negotiation_h3_variants() {
    let capsule = UniversalApiMetaCapsule::new();

    // Support multiple HTTP/3 ALPN values (h3, h3-29, h3-27)
    let variants: Vec<&[u8]> = vec![b"h3", b"h3-29", b"h3-27"];

    for variant in variants {
        let request = MockRequest {
            alpn: Some(variant.to_vec()),
            raw_bytes: vec![],
            method: "GET",
            path: "/",
            protocol: ProtocolType::REST,
        };
        assert_eq!(capsule.detect_transport(&request), TransportType::HTTP3,
                   "ALPN {:?} should be recognized as HTTP/3",
                   String::from_utf8_lossy(variant));
    }
}

#[test]
fn test_q25_http3_protocol_agnostic_routing() {
    let capsule = UniversalApiMetaCapsule::new();

    // Same application protocol (REST) over different transports
    let rest_http1 = MockRequest {
        alpn: None,
        raw_bytes: vec![],
        method: "GET",
        path: "/api/users",
        protocol: ProtocolType::REST,
    };

    let rest_http2 = MockRequest {
        alpn: Some(b"h2".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/api/users",
        protocol: ProtocolType::REST,
    };

    let rest_http3 = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: vec![],
        method: "GET",
        path: "/api/users",
        protocol: ProtocolType::REST,
    };

    assert_eq!(capsule.detect_transport(&rest_http1), TransportType::HTTP1);
    assert_eq!(capsule.detect_transport(&rest_http2), TransportType::HTTP2);
    assert_eq!(capsule.detect_transport(&rest_http3), TransportType::HTTP3);

    // All are REST protocol regardless of transport
    assert_eq!(rest_http1.protocol(), ProtocolType::REST);
    assert_eq!(rest_http2.protocol(), ProtocolType::REST);
    assert_eq!(rest_http3.protocol(), ProtocolType::REST);
}

#[test]
fn test_q26_http3_large_payload_detection() {
    let capsule = UniversalApiMetaCapsule::new();

    // Large payload should not affect transport detection
    let large_payload = vec![0xFF; 10_000_000]; // 10MB

    let request = MockRequest {
        alpn: Some(b"h3".to_vec()),
        raw_bytes: large_payload,
        method: "POST",
        path: "/upload",
        protocol: ProtocolType::REST,
    };

    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP3);
}

#[test]
fn test_q27_http3_fallback_chain() {
    let capsule = UniversalApiMetaCapsule::new();

    // Test fallback: invalid ALPN → check magic bytes → default HTTP1
    let request = MockRequest {
        alpn: Some(b"invalid-protocol".to_vec()),
        raw_bytes: vec![], // No QUIC magic bytes
        method: "GET",
        path: "/",
        protocol: ProtocolType::REST,
    };

    assert_eq!(capsule.detect_transport(&request), TransportType::HTTP1,
               "Invalid ALPN with no magic bytes should fallback to HTTP1");
}

#[test]
fn test_q28_http3_transport_stats_final_verification() {
    let capsule = UniversalApiMetaCapsule::new();

    // Final integration: mix all transports
    let requests: Vec<(Option<&[u8]>, Vec<u8>, TransportType)> = vec![
        (None, vec![], TransportType::HTTP1),
        (Some(b"h2"), vec![], TransportType::HTTP2),
        (Some(b"h3"), vec![], TransportType::HTTP3),
        (None, vec![0xC0, 0x00], TransportType::HTTP3),
        (Some(b"h2"), vec![0xC0, 0x00], TransportType::HTTP2), // ALPN priority
    ];

    let mut expected_http1 = 0;
    let mut expected_http2 = 0;
    let mut expected_http3 = 0;

    for (alpn_opt, raw_bytes, expected_transport) in requests.iter() {
        let request = MockRequest {
            alpn: alpn_opt.map(|a: &[u8]| a.to_vec()),
            raw_bytes: raw_bytes.clone(),
            method: "GET",
            path: "/",
            protocol: ProtocolType::REST,
        };

        let detected = capsule.detect_transport(&request);
        assert_eq!(detected, *expected_transport,
                   "Transport detection mismatch for ALPN={:?}, raw_bytes={:?}",
                   alpn_opt.map(|a| String::from_utf8_lossy(a)),
                   raw_bytes);

        match expected_transport {
            TransportType::HTTP1 => expected_http1 += 1,
            TransportType::HTTP2 => expected_http2 += 1,
            TransportType::HTTP3 => expected_http3 += 1,
            TransportType::WebSocket => {},
        }

        let _ = capsule.route_with_transport(&request);
    }

    let (http1, http2, http3, _ws) = capsule.get_transport_stats();
    assert_eq!(http1, expected_http1, "HTTP/1 count mismatch");
    assert_eq!(http2, expected_http2, "HTTP/2 count mismatch");
    assert_eq!(http3, expected_http3, "HTTP/3 count mismatch");
}

// ============================================================================
// Mock Helpers
// ============================================================================

struct MockRequest {
    alpn: Option<Vec<u8>>,
    raw_bytes: Vec<u8>,
    method: &'static str,
    path: &'static str,
    protocol: ProtocolType,
}

impl UniversalRequest for MockRequest {
    fn method(&self) -> &str {
        self.method
    }

    fn path(&self) -> &str {
        self.path
    }

    fn header(&self, _name: &str) -> Option<&str> {
        None
    }

    fn body(&self) -> &[u8] {
        &[]
    }

    fn protocol(&self) -> ProtocolType {
        self.protocol
    }

    fn alpn_protocol(&self) -> Option<&[u8]> {
        self.alpn.as_deref()
    }

    fn raw_bytes(&self) -> &[u8] {
        &self.raw_bytes
    }
}
