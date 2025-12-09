//! QpackEncoderCapsule Integration Tests
//! T28 Framework: Unit, Property, Integration, Production Tiers

use atomic_capsule::quic::QpackEncoderCapsule;

// ============================================================================
// TIER Q1-Q7: Unit Tests
// ============================================================================

#[test]
fn q1_encoder_new() {
    let encoder = QpackEncoderCapsule::new();
    let stats = encoder.stats();
    assert_eq!(stats.headers_encoded, 0);
    assert_eq!(stats.dynamic_table_capacity, 4096);
    assert_eq!(stats.dynamic_table_size, 0);
}

#[test]
fn q2_encoder_with_capacity() {
    let encoder = QpackEncoderCapsule::with_capacity(2048);
    let stats = encoder.stats();
    assert_eq!(stats.dynamic_table_capacity, 2048);
}

#[test]
fn q3_size_alignment() {
    assert_eq!(std::mem::size_of::<QpackEncoderCapsule>(), 1024);
    assert_eq!(std::mem::align_of::<QpackEncoderCapsule>(), 1024);
}

#[test]
fn q4_fnv1a_hash_deterministic() {
    let h1 = QpackEncoderCapsule::fnv1a_hash("content-type");
    let h2 = QpackEncoderCapsule::fnv1a_hash("content-type");
    assert_eq!(h1, h2, "Hash must be deterministic");
}

#[test]
fn q5_fnv1a_hash_different() {
    let h1 = QpackEncoderCapsule::fnv1a_hash("content-type");
    let h2 = QpackEncoderCapsule::fnv1a_hash("accept");
    assert_ne!(h1, h2, "Different inputs must have different hashes");
}

#[test]
fn q6_lookup_static_scalar() {
    let encoder = QpackEncoderCapsule::new();
    // :authority is index 0
    let idx = encoder.lookup_static_scalar(":authority");
    assert_eq!(idx, Some(0), "Should find :authority at index 0");
}

#[test]
fn q7_lookup_not_found() {
    let encoder = QpackEncoderCapsule::new();
    let idx = encoder.lookup_static_scalar("x-custom-header");
    assert_eq!(idx, None, "Custom header should not be in static table");
}

// ============================================================================
// TIER Q8-Q14: Property Tests
// ============================================================================

#[test]
fn q8_lookup_consistency() {
    let encoder = QpackEncoderCapsule::new();
    // Multiple lookups of same header should be consistent
    let idx1 = encoder.lookup_static_scalar("cache-control");
    let idx2 = encoder.lookup_static_scalar("cache-control");
    assert_eq!(idx1, idx2, "Multiple lookups must be consistent");
}

#[test]
fn q9_encode_single_increments_counter() {
    let encoder = QpackEncoderCapsule::new();
    encoder.encode_header("content-type", "application/json");
    let stats = encoder.stats();
    assert_eq!(stats.headers_encoded, 1, "Counter should increment");
}

#[test]
fn q10_encode_batch_increments_counter() {
    let encoder = QpackEncoderCapsule::new();
    let headers = vec![
        (":authority", "example.com"),
        (":path", "/"),
        (":scheme", "https"),
    ];
    encoder.encode_headers_batch(&headers);
    let stats = encoder.stats();
    assert_eq!(stats.headers_encoded, 3, "Batch counter should increment correctly");
}

#[test]
fn q11_capacity_update() {
    let encoder = QpackEncoderCapsule::new();
    encoder.update_capacity(2048);
    let stats = encoder.stats();
    assert_eq!(stats.dynamic_table_capacity, 2048, "Capacity should update");
}

#[test]
fn q12_capacity_capped_at_max() {
    let encoder = QpackEncoderCapsule::new();
    encoder.update_capacity(16384); // Exceeds 8192 limit
    let stats = encoder.stats();
    assert_eq!(
        stats.dynamic_table_capacity, 8192,
        "Capacity should be capped at 8192"
    );
}

#[test]
fn q13_default_trait() {
    let encoder = QpackEncoderCapsule::default();
    let stats = encoder.stats();
    assert_eq!(stats.dynamic_table_capacity, 4096);
}

#[test]
fn q14_encode_returns_non_empty() {
    let encoder = QpackEncoderCapsule::new();
    let encoded = encoder.encode_header("content-type", "application/json");
    assert!(!encoded.is_empty(), "Encoded output must not be empty");
}

// ============================================================================
// TIER Q15-Q21: Integration Tests
// ============================================================================

#[test]
fn q15_multiple_batches() {
    let encoder = QpackEncoderCapsule::new();

    // First batch
    let headers1 = vec![(":authority", "example.com"), (":path", "/")];
    encoder.encode_headers_batch(&headers1);

    // Second batch
    let headers2 = vec![(":scheme", "https"), (":method", "GET")];
    encoder.encode_headers_batch(&headers2);

    let stats = encoder.stats();
    assert_eq!(stats.headers_encoded, 4, "Should count headers from both batches");
}

#[test]
fn q16_scalar_and_simd_equivalence() {
    let encoder = QpackEncoderCapsule::new();

    // Both scalar and SIMD should find the same result
    let scalar_idx = encoder.lookup_static_scalar(":authority");
    let simd_idx = encoder.lookup_static_simd(":authority");

    assert_eq!(scalar_idx, simd_idx, "Scalar and SIMD lookups must agree");
}

#[test]
fn q17_not_found_both_methods() {
    let encoder = QpackEncoderCapsule::new();

    let scalar_idx = encoder.lookup_static_scalar("x-nonexistent");
    let simd_idx = encoder.lookup_static_simd("x-nonexistent");

    assert_eq!(scalar_idx, None, "Scalar should not find");
    assert_eq!(simd_idx, None, "SIMD should not find");
}

#[test]
fn q18_batch_with_mix_of_found_and_not_found() {
    let encoder = QpackEncoderCapsule::new();

    // Mix of headers that exist and don't exist in static table
    let headers = vec![
        (":authority", "example.com"),     // Found
        ("x-custom", "value"),              // Not found
        (":path", "/"),                     // Found
        ("another-custom", "value"),        // Not found
    ];

    let encoded = encoder.encode_headers_batch(&headers);
    assert!(!encoded.is_empty(), "Should encode mixed headers");

    let stats = encoder.stats();
    assert_eq!(stats.headers_encoded, 4, "Should count all headers");
}

#[test]
fn q19_large_batch() {
    let encoder = QpackEncoderCapsule::new();

    // Create a vec of header tuples with small header names
    let headers: Vec<_> = (0..10)
        .map(|_| (":authority", "example.com"))
        .collect();

    encoder.encode_headers_batch(&headers);

    let stats = encoder.stats();
    assert_eq!(stats.headers_encoded, 10, "Should handle batches");
}

#[test]
fn q20_repeated_lookups() {
    let encoder = QpackEncoderCapsule::new();

    // Repeated lookups of same header
    for _ in 0..10 {
        let idx = encoder.lookup_static_simd(":authority");
        assert_eq!(idx, Some(0), "Repeated lookups must succeed");
    }
}

#[test]
fn q21_stats_aggregation() {
    let encoder = QpackEncoderCapsule::new();

    encoder.encode_header("content-type", "application/json");
    encoder.encode_header("cache-control", "no-cache");

    let headers = vec![(":authority", "example.com"), (":path", "/")];
    encoder.encode_headers_batch(&headers);

    let stats = encoder.stats();
    assert_eq!(
        stats.headers_encoded, 4,
        "Stats should aggregate from all encode calls"
    );
}

// ============================================================================
// TIER Q22-Q28: Production Tests
// ============================================================================

#[test]
fn q22_production_http_request_headers() {
    let encoder = QpackEncoderCapsule::new();

    // Typical HTTP/3 request
    let headers = vec![
        (":method", "POST"),
        (":scheme", "https"),
        (":authority", "api.example.com"),
        (":path", "/v1/messages"),
        ("content-type", "application/json"),
        ("accept", "*/*"),
        ("accept-encoding", "gzip, deflate"),
        ("user-agent", "curl/7.64.1"),
        ("content-length", "256"),
    ];

    let encoded = encoder.encode_headers_batch(&headers);
    assert!(!encoded.is_empty());

    let stats = encoder.stats();
    assert_eq!(stats.headers_encoded, 9);
}

#[test]
fn q23_production_response_headers() {
    let encoder = QpackEncoderCapsule::new();

    // Typical HTTP/3 response
    let headers = vec![
        (":status", "200"),
        ("content-type", "application/json"),
        ("content-length", "1234"),
        ("cache-control", "max-age=3600"),
        ("date", "Mon, 23 Nov 2025 12:00:00 GMT"),
        ("server", "nginx/1.20"),
    ];

    let _encoded = encoder.encode_headers_batch(&headers);

    let stats = encoder.stats();
    assert_eq!(stats.headers_encoded, 6);
}

#[test]
fn q24_production_large_response() {
    let encoder = QpackEncoderCapsule::new();

    // Large response with many headers
    let mut headers = vec![
        (":status", "200"),
        ("content-type", "text/html"),
        ("content-length", "50000"),
    ];

    // Add custom headers (simulate real-world scenario)
    for i in 0..50 {
        headers.push(("x-custom", Box::leak(format!("value-{}", i).into_boxed_str())));
    }

    let headers_refs: Vec<_> = headers.iter().map(|(a, b)| (*a, *b)).collect();
    encoder.encode_headers_batch(&headers_refs);

    let stats = encoder.stats();
    assert!(stats.headers_encoded >= 50);
}

#[test]
fn q25_production_concurrent_stats() {
    use std::sync::Arc;
    use std::thread;

    let encoder = Arc::new(QpackEncoderCapsule::new());
    let mut handles = vec![];

    // Simulate concurrent encoding (simplified - no actual concurrency in capsule)
    for _ in 0..5 {
        let enc = encoder.clone();
        let handle = thread::spawn(move || {
            let headers = vec![(":authority", "example.com"), (":path", "/")];
            enc.encode_headers_batch(&headers);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = encoder.stats();
    assert_eq!(stats.headers_encoded, 10);
}

#[test]
fn q26_production_encoding_determinism() {
    let encoder1 = QpackEncoderCapsule::new();
    let encoder2 = QpackEncoderCapsule::new();

    let headers = vec![
        (":authority", "example.com"),
        (":path", "/api/users"),
        ("content-type", "application/json"),
    ];

    let encoded1 = encoder1.encode_headers_batch(&headers);
    let encoded2 = encoder2.encode_headers_batch(&headers);

    assert_eq!(encoded1, encoded2, "Encoding must be deterministic");
}

#[test]
fn q27_production_zero_capacity() {
    let encoder = QpackEncoderCapsule::with_capacity(0);
    let stats = encoder.stats();
    assert_eq!(stats.dynamic_table_capacity, 0, "Should respect zero capacity");

    // Should still be able to encode (static table only)
    let headers = vec![(":authority", "example.com")];
    encoder.encode_headers_batch(&headers);

    let stats = encoder.stats();
    assert_eq!(stats.headers_encoded, 1);
}

#[test]
fn q28_production_max_capacity() {
    let encoder = QpackEncoderCapsule::new();
    encoder.update_capacity(8192);

    let stats = encoder.stats();
    assert_eq!(stats.dynamic_table_capacity, 8192, "Should support max capacity");

    // Should encode successfully with max capacity
    let headers = vec![(":authority", "example.com"), (":path", "/")];
    encoder.encode_headers_batch(&headers);

    let final_stats = encoder.stats();
    assert_eq!(final_stats.headers_encoded, 2);
}
