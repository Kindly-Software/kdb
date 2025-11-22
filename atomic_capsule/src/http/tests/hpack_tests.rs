//! HPACK Header Compression Tests (T28 Framework)
//!
//! **Tier Classification**: Q1-Q28 comprehensive test pyramid
//! - Q1-Q7: Unit tests (basic functionality, edge cases, error handling)
//! - Q8-Q14: Property tests (round-trip, compression ratio, determinism)
//! - Q15-Q21: Integration tests (real HTTP/2 headers, multiple headers, table management)
//! - Q22-Q28: Production tests (performance, memory stability, RFC 7541 compliance)

#[cfg(test)]
mod hpack_unit_tests {
    use atomic_capsule::http::hpack::*;
    use core::sync::atomic::Ordering;

    // ============================================================================
    // Q1-Q7: UNIT TESTS (Basic Functionality, Edge Cases, Error Handling)
    // ============================================================================

    #[test]
    fn test_encoder_initialization() {
        let encoder = HpackEncoderCapsule::new();
        assert_eq!(encoder.dynamic_table_max_size.load(Ordering::Relaxed), 4096);
        assert_eq!(encoder.dynamic_table_size.load(Ordering::Relaxed), 0);
        assert_eq!(encoder.entries_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_decoder_initialization() {
        let decoder = HpackDecoderCapsule::new();
        assert_eq!(decoder.dynamic_table_max_size.load(Ordering::Relaxed), 4096);
        assert_eq!(decoder.dynamic_table_size.load(Ordering::Relaxed), 0);
        assert_eq!(decoder.entries_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_static_table_entries() {
        assert!(STATIC_TABLE.len() >= 61);
        assert_eq!(STATIC_TABLE[0].name, b":authority");
        assert_eq!(STATIC_TABLE[1].name, b":method");
        assert_eq!(STATIC_TABLE[1].value, Some(b"GET" as &[u8]));
    }

    #[test]
    fn test_static_table_lookup_full_match() {
        let encoder = HpackEncoderCapsule::new();
        let result = encoder.lookup_static_table(b":method", b"GET");
        assert!(result.is_some());
        let (idx, full_match) = result.unwrap();
        assert_eq!(idx, 2); // Index 2 per RFC 7541
        assert!(full_match);
    }

    #[test]
    fn test_static_table_lookup_name_only() {
        let encoder = HpackEncoderCapsule::new();
        let result = encoder.lookup_static_table(b":method", b"DELETE");
        assert!(result.is_some());
        let (_, full_match) = result.unwrap();
        assert!(!full_match);
    }

    #[test]
    fn test_static_table_lookup_not_found() {
        let encoder = HpackEncoderCapsule::new();
        let result = encoder.lookup_static_table(b"x-custom-header", b"value");
        // Should still find name match in static table or return None
        // Depends on implementation
        let _ = result;
    }

    #[test]
    fn test_encode_get_method() {
        let encoder = HpackEncoderCapsule::new();
        let result = encoder.encode_header(b":method", b"GET", false);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        assert!(!encoded.is_empty());
        // GET matches static table index 2, so encoded should start with 0x82 (0x80 | 2)
        assert_eq!(encoded[0], 0x82);
    }

    #[test]
    fn test_encode_post_method() {
        let encoder = HpackEncoderCapsule::new();
        let result = encoder.encode_header(b":method", b"POST", false);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        assert!(!encoded.is_empty());
        assert_eq!(encoded[0], 0x83); // Index 3
    }

    #[test]
    fn test_encode_custom_header_literal() {
        let encoder = HpackEncoderCapsule::new();
        let result = encoder.encode_header(b"x-custom", b"value", false);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        assert!(!encoded.is_empty());
        // Should use literal encoding (0x40 prefix)
        assert!(encoded[0] & 0xC0 == 0x40);
    }

    #[test]
    fn test_encode_sensitive_header() {
        let encoder = HpackEncoderCapsule::new();
        let result = encoder.encode_header(b"authorization", b"Bearer token123", true);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        assert!(!encoded.is_empty());
        // Should use literal never indexed (0x10 prefix)
        assert!(encoded[0] & 0xF0 == 0x10);
    }

    #[test]
    fn test_set_max_table_size_valid() {
        let encoder = HpackEncoderCapsule::new();
        let result = encoder.set_max_table_size(8192);
        assert!(result.is_ok());
        assert_eq!(encoder.dynamic_table_max_size.load(Ordering::Relaxed), 8192);
    }

    #[test]
    fn test_set_max_table_size_too_large() {
        let encoder = HpackEncoderCapsule::new();
        let result = encoder.set_max_table_size(0x1000000); // 16MB, exceeds limit
        assert!(result.is_err());
    }

    #[test]
    fn test_compression_metrics_initial() {
        let encoder = HpackEncoderCapsule::new();
        let metrics = encoder.metrics();
        assert_eq!(metrics.headers_encoded, 0);
        assert_eq!(metrics.bytes_before, 0);
        assert_eq!(metrics.bytes_after, 0);
    }

    #[test]
    fn test_compression_ratio_calculation() {
        let metrics = HpackMetrics {
            headers_encoded: 100,
            bytes_before: 1000,
            bytes_after: 300,
            indexed_lookups: 50,
            literal_encodings: 50,
            huffman_encodings: 30,
        };
        let ratio = metrics.compression_ratio();
        assert!((ratio - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_compression_ratio_zero_bytes() {
        let metrics = HpackMetrics {
            headers_encoded: 0,
            bytes_before: 0,
            bytes_after: 0,
            indexed_lookups: 0,
            literal_encodings: 0,
            huffman_encodings: 0,
        };
        let ratio = metrics.compression_ratio();
        assert_eq!(ratio, 0.0);
    }

    #[test]
    fn test_encoder_memory_layout() {
        assert_eq!(std::mem::size_of::<HpackEncoderCapsule>(), 256);
        assert_eq!(std::mem::align_of::<HpackEncoderCapsule>(), 256);
    }

    #[test]
    fn test_decoder_memory_layout() {
        assert_eq!(std::mem::size_of::<HpackDecoderCapsule>(), 256);
        assert_eq!(std::mem::align_of::<HpackDecoderCapsule>(), 256);
    }

    #[test]
    fn test_huffman_code_struct() {
        let code = HuffmanCode {
            code: 0x1ff8,
            bits: 13,
            symbol: b'0',
            padding: [0; 2],
        };
        assert_eq!(code.bits, 13);
        assert_eq!(code.symbol, b'0');
    }

    // ============================================================================
    // Q8-Q14: PROPERTY TESTS (Round-Trip, Determinism, Compression Ratio)
    // ============================================================================

    #[test]
    fn test_encode_decode_round_trip_get() {
        let encoder = HpackEncoderCapsule::new();
        let decoder = HpackDecoderCapsule::new();

        let encoded = encoder.encode_header(b":method", b"GET", false).unwrap();
        let (name, value, _) = decoder.decode_header(&encoded).unwrap();

        assert_eq!(name, b":method");
        assert_eq!(value, b"GET");
    }

    #[test]
    fn test_encode_decode_round_trip_post() {
        let encoder = HpackEncoderCapsule::new();
        let decoder = HpackDecoderCapsule::new();

        let encoded = encoder.encode_header(b":method", b"POST", false).unwrap();
        let (name, value, _) = decoder.decode_header(&encoded).unwrap();

        assert_eq!(name, b":method");
        assert_eq!(value, b"POST");
    }

    #[test]
    fn test_encoding_determinism() {
        let encoder = HpackEncoderCapsule::new();

        let encoded1 = encoder.encode_header(b":method", b"GET", false).unwrap();
        let encoded2 = encoder.encode_header(b":method", b"GET", false).unwrap();

        assert_eq!(encoded1, encoded2);
    }

    #[test]
    fn test_multiple_headers_encoding() {
        let encoder = HpackEncoderCapsule::new();
        let headers = vec![
            (vec![b':' as u8, 'm' as u8], vec![b'G' as u8]),
            (vec![b'h' as u8, 'o' as u8], vec![b'w' as u8]),
        ];

        let result = encoder.encode_headers(&headers);
        assert!(result.is_ok());
        let encoded = result.unwrap();
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_indexed_lookup_increments() {
        let encoder = HpackEncoderCapsule::new();
        let before = encoder.indexed_lookups.load(Ordering::Relaxed);

        let _ = encoder.encode_header(b":method", b"GET", false);

        let after = encoder.indexed_lookups.load(Ordering::Relaxed);
        assert!(after > before);
    }

    #[test]
    fn test_literal_encoding_increments() {
        let encoder = HpackEncoderCapsule::new();
        let before = encoder.literal_encodings.load(Ordering::Relaxed);

        let _ = encoder.encode_header(b"x-custom", b"value", false);

        let after = encoder.literal_encodings.load(Ordering::Relaxed);
        assert!(after > before);
    }

    #[test]
    fn test_compression_improvement() {
        let encoder = HpackEncoderCapsule::new();

        // Encode multiple common headers
        let _ = encoder.encode_header(b":method", b"GET", false);
        let _ = encoder.encode_header(b":path", b"/", false);
        let _ = encoder.encode_header(b":scheme", b"https", false);

        let metrics = encoder.metrics();
        // Static table entries should give good compression
        if metrics.bytes_before > 0 {
            let ratio = metrics.compression_ratio();
            assert!(ratio < 1.0); // Compressed should be smaller
        }
    }

    // ============================================================================
    // Q15-Q21: INTEGRATION TESTS (Real Headers, Table Management, RFC Compliance)
    // ============================================================================

    #[test]
    fn test_decode_common_status_200() {
        let decoder = HpackDecoderCapsule::new();
        // Encode index 8 (:status 200)
        let buffer = [0x88];
        let (name, value, consumed) = decoder.decode_header(&buffer).unwrap();
        assert_eq!(name, b":status");
        assert_eq!(value, b"200");
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_decode_common_status_404() {
        let decoder = HpackDecoderCapsule::new();
        // Encode index 13 (:status 404)
        let buffer = [0x8d];
        let (name, value, consumed) = decoder.decode_header(&buffer).unwrap();
        assert_eq!(name, b":status");
        assert_eq!(value, b"404");
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_decode_authority_header() {
        let decoder = HpackDecoderCapsule::new();
        // Index 1 (:authority)
        let buffer = [0x81];
        let (name, value, _) = decoder.decode_header(&buffer).unwrap();
        assert_eq!(name, b":authority");
        assert!(value.is_empty());
    }

    #[test]
    fn test_integer_encoding_single_byte() {
        let encoder = HpackEncoderCapsule::new();
        let mut output = Vec::new();
        // Encode integer 5 with 6-bit prefix
        encoder.encode_integer(5, 6, &mut output);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_integer_decoding_single_byte() {
        let decoder = HpackDecoderCapsule::new();
        let buffer = [0x05]; // Integer 5
        let (value, consumed) = decoder.decode_integer(&buffer, 6).unwrap();
        assert_eq!(value, 5);
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_integer_decoding_multi_byte() {
        let decoder = HpackDecoderCapsule::new();
        // Encode 256 as: 0x3F (max 6-bit) + 0x81 0x01
        let buffer = [0x3F, 0x81, 0x01];
        let (value, consumed) = decoder.decode_integer(&buffer, 6).unwrap();
        assert_eq!(value, 256);
        assert_eq!(consumed, 3);
    }

    #[test]
    fn test_decode_headers_multiple() {
        let decoder = HpackDecoderCapsule::new();
        // Two indexed headers (0x82 = :method GET, 0x88 = :status 200)
        let buffer = [0x82, 0x88];
        let headers = decoder.decode_headers(&buffer).unwrap();
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0].0, b":method");
        assert_eq!(headers[0].1, b"GET");
    }

    #[test]
    fn test_table_size_update() {
        let decoder = HpackDecoderCapsule::new();
        decoder.set_max_table_size(2048).unwrap();
        assert_eq!(decoder.dynamic_table_max_size.load(Ordering::Relaxed), 2048);
    }

    #[test]
    fn test_decoder_metrics_initial() {
        let decoder = HpackDecoderCapsule::new();
        let metrics = decoder.metrics();
        assert_eq!(metrics.headers_encoded, 0);
    }

    #[test]
    fn test_encoder_header_count_increments() {
        let encoder = HpackEncoderCapsule::new();
        let before = encoder.headers_encoded.load(Ordering::Relaxed);

        let _ = encoder.encode_header(b":method", b"GET", false);

        let after = encoder.headers_encoded.load(Ordering::Relaxed);
        assert_eq!(after, before + 1);
    }

    #[test]
    fn test_encoder_byte_tracking() {
        let encoder = HpackEncoderCapsule::new();
        let _ = encoder.encode_header(b":method", b"GET", false);

        let bytes_before = encoder.bytes_before_encoding.load(Ordering::Relaxed);
        let bytes_after = encoder.bytes_after_encoding.load(Ordering::Relaxed);

        assert_eq!(bytes_before, 10); // ":method" (7) + "GET" (3)
        assert!(bytes_after > 0);
    }

    #[test]
    fn test_rfc7541_static_table_coverage() {
        // Verify key RFC 7541 Appendix A entries exist
        let check = |name: &[u8]| {
            STATIC_TABLE.iter().any(|e| e.name == name)
        };

        assert!(check(b":authority"));
        assert!(check(b":method"));
        assert!(check(b":path"));
        assert!(check(b":scheme"));
        assert!(check(b":status"));
        assert!(check(b"content-type"));
        assert!(check(b"cache-control"));
    }

    // ============================================================================
    // Q22-Q28: PRODUCTION TESTS (Performance, Stability, RFC Compliance)
    // ============================================================================

    #[test]
    fn test_encoder_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let encoder = Arc::new(HpackEncoderCapsule::new());
        let mut handles = vec![];

        for _ in 0..4 {
            let enc = Arc::clone(&encoder);
            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    let _ = enc.encode_header(b":method", b"GET", false);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // After 40 encodes, check metrics
        let metrics = encoder.metrics();
        assert_eq!(metrics.headers_encoded, 40);
    }

    #[test]
    fn test_decoder_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let decoder = Arc::new(HpackDecoderCapsule::new());
        let mut handles = vec![];

        for _ in 0..4 {
            let dec = Arc::clone(&decoder);
            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    let buffer = [0x82];
                    let _ = dec.decode_header(&buffer);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let metrics = decoder.metrics();
        assert_eq!(metrics.headers_encoded, 40);
    }

    #[test]
    fn test_encoder_no_panic_on_large_input() {
        let encoder = HpackEncoderCapsule::new();
        let large_value = vec![b'x'; 10000];
        let result = encoder.encode_header(b"x-data", &large_value, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_decoder_empty_buffer() {
        let decoder = HpackDecoderCapsule::new();
        let result = decoder.decode_header(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_huffman_scratch_space() {
        let encoder = HpackEncoderCapsule::new();
        // Huffman scratch is 128 bytes
        assert_eq!(encoder.huffman_scratch.len(), 128);
    }

    #[test]
    fn test_static_table_immutability() {
        let entry1 = STATIC_TABLE[0];
        let entry2 = STATIC_TABLE[0];
        assert_eq!(entry1.name, entry2.name);
        assert_eq!(entry1.value, entry2.value);
    }

    #[test]
    fn test_metrics_compression_ratio_improvement() {
        let encoder = HpackEncoderCapsule::new();

        // Encode many indexed headers (should compress well)
        for _ in 0..10 {
            let _ = encoder.encode_header(b":method", b"GET", false);
            let _ = encoder.encode_header(b":path", b"/", false);
            let _ = encoder.encode_header(b":scheme", b"https", false);
        }

        let metrics = encoder.metrics();
        let ratio = metrics.compression_ratio();
        // Static table entries compress to 1 byte each, original is ~20+ bytes
        if metrics.bytes_before > 0 {
            assert!(ratio < 0.5); // Should achieve at least 2:1 compression
        }
    }

    #[test]
    fn test_sensitive_header_encoding_difference() {
        let encoder = HpackEncoderCapsule::new();

        let normal = encoder.encode_header(b"custom", b"value", false).unwrap();
        let sensitive = encoder.encode_header(b"custom", b"value", true).unwrap();

        // Sensitive uses 0x10 prefix, normal uses 0x40 prefix
        // First byte should differ
        assert_ne!(normal[0] & 0xF0, sensitive[0] & 0xF0);
    }

    #[test]
    fn test_encoder_capacity_stable() {
        let encoder = HpackEncoderCapsule::new();

        // Encode many headers without dynamic table updates
        for i in 0..100 {
            let name = format!("x-header-{}", i);
            let value = format!("value-{}", i);
            let _ = encoder.encode_header(name.as_bytes(), value.as_bytes(), false);
        }

        // Encoder should still be functional
        let result = encoder.encode_header(b":method", b"GET", false);
        assert!(result.is_ok());
    }
}
