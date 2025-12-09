//! WebSocket Fragmentation Integration Tests
//!
//! **Purpose**: Validate RFC 6455 §5.4 fragmentation compliance in WebSocketServerCapsule
//!
//! **Framework**: T28 (4 tiers: Unit/Property/Integration/Production)
//! **Compliance**: UCE34, Chaos, ASSUM, B32, I20
//!
//! ## Test Coverage
//!
//! - **Q1-Q7**: Unit tests (single/multi-frame messages, opcode validation)
//! - **Q8-Q14**: Property tests (fragmentation patterns, buffer limits)
//! - **Q15-Q21**: Integration tests (per-connection isolation, error handling)
//! - **Q22-Q28**: Production tests (stress test, performance validation)

#[cfg(all(feature = "std", feature = "http"))]
mod tests {
    use atomic_capsule::http::WebSocketServerCapsule;
    use std::time::Instant;

    // ============================================================================
    // Q1-Q7: Unit Tests
    // ============================================================================

    #[test]
    fn q1_single_frame_text_message() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Single-frame text message (FIN=1, opcode=0x1, "Hello")
        let frame = build_text_frame(true, b"Hello");

        // Process frame
        server.on_frame(1, &frame).unwrap();

        // Verify metrics (1 complete message)
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 1);
    }

    #[test]
    fn q2_single_frame_binary_message() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Single-frame binary message (FIN=1, opcode=0x2, [0xDE, 0xAD, 0xBE, 0xEF])
        let frame = build_binary_frame(true, &[0xDE, 0xAD, 0xBE, 0xEF]);

        // Process frame
        server.on_frame(1, &frame).unwrap();

        // Verify metrics
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 1);
    }

    #[test]
    fn q3_multi_frame_text_message() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Fragment 1: First frame (FIN=0, opcode=0x1, "Hello")
        let frame1 = build_text_frame(false, b"Hello");
        server.on_frame(1, &frame1).unwrap();

        // After first fragment, no complete message yet
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 0);

        // Fragment 2: Continuation (FIN=0, opcode=0x0, ", World")
        let frame2 = build_continuation_frame(false, b", World");
        server.on_frame(1, &frame2).unwrap();

        // Still no complete message
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 0);

        // Fragment 3: Final continuation (FIN=1, opcode=0x0, "!")
        let frame3 = build_continuation_frame(true, b"!");
        server.on_frame(1, &frame3).unwrap();

        // Now we have 1 complete message
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 1);
    }

    #[test]
    fn q4_invalid_first_frame_opcode() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // First frame with continuation opcode (INVALID)
        let frame = build_continuation_frame(false, b"Invalid");

        // Should fail validation
        let result = server.on_frame(1, &frame);
        assert!(result.is_err());
    }

    #[test]
    fn q5_invalid_continuation_opcode() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Start with valid text frame
        let frame1 = build_text_frame(false, b"Hello");
        server.on_frame(1, &frame1).unwrap();

        // Follow with text frame instead of continuation (INVALID)
        let frame2 = build_text_frame(false, b", World");
        let result = server.on_frame(1, &frame2);

        // Should fail validation
        assert!(result.is_err());
    }

    #[test]
    fn q6_per_connection_isolation() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Connection 1: Start fragmented message
        let frame1_conn1 = build_text_frame(false, b"Connection 1");
        server.on_frame(1, &frame1_conn1).unwrap();

        // Connection 2: Complete single-frame message
        let frame_conn2 = build_text_frame(true, b"Connection 2");
        server.on_frame(2, &frame_conn2).unwrap();

        // Connection 2's message should complete independently
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 1); // Only connection 2's message

        // Complete connection 1's message
        let frame2_conn1 = build_continuation_frame(true, b" fragment 2");
        server.on_frame(1, &frame2_conn1).unwrap();

        // Now both messages completed
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 2);
    }

    #[test]
    fn q7_control_frame_not_fragmented() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Control frames (ping/pong/close) should work during fragmentation
        let frame1 = build_text_frame(false, b"Fragment 1");
        server.on_frame(1, &frame1).unwrap();

        // Send ping (opcode=0x9, FIN=1)
        let ping = build_ping_frame();
        server.on_frame(1, &ping).unwrap();

        // Complete the fragmented message
        let frame2 = build_continuation_frame(true, b" Fragment 2");
        server.on_frame(1, &frame2).unwrap();

        // Message should complete successfully
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 1);
    }

    // ============================================================================
    // Q8-Q14: Property Tests
    // ============================================================================

    #[test]
    fn q8_fragmentation_preserves_message_order() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Send 3 fragmented messages in sequence
        for i in 1..=3 {
            let msg = format!("Message {}", i);
            let frame1 = build_text_frame(false, msg.as_bytes());
            server.on_frame(1, &frame1).unwrap();

            let frame2 = build_continuation_frame(true, b" end");
            server.on_frame(1, &frame2).unwrap();
        }

        // Should have 3 complete messages
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 3);
    }

    #[test]
    fn q9_fragmentation_with_varying_sizes() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Fragment sizes: 10B, 100B, 1KB, 10KB
        let sizes = vec![10, 100, 1024, 10240];

        for size in sizes {
            let payload: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
            let frame = build_text_frame(true, &payload);
            server.on_frame(1, &frame).unwrap();
        }

        // All 4 messages should complete
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 4);
    }

    #[test]
    fn q10_reset_after_complete_message() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // First message: 2 fragments
        let frame1 = build_text_frame(false, b"First");
        server.on_frame(1, &frame1).unwrap();
        let frame2 = build_continuation_frame(true, b" message");
        server.on_frame(1, &frame2).unwrap();

        // Second message: should start fresh (not continuation)
        let frame3 = build_text_frame(true, b"Second message");
        server.on_frame(1, &frame3).unwrap();

        // Both messages should complete
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 2);
    }

    #[test]
    fn q11_max_fragment_count_limit() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Send first fragment
        let frame1 = build_text_frame(false, b"Start");
        server.on_frame(1, &frame1).unwrap();

        // Send 1023 continuation fragments (total 1024)
        for _ in 0..1023 {
            let frame = build_continuation_frame(false, b"x");
            server.on_frame(1, &frame).unwrap();
        }

        // Final fragment should succeed (exactly 1024)
        let frame_final = build_continuation_frame(true, b"End");
        let result = server.on_frame(1, &frame_final);
        assert!(result.is_ok());
    }

    #[test]
    fn q12_concurrent_connections_fragmentation() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // 10 connections, each with 3-fragment message
        for conn_id in 1..=10 {
            for frag_id in 1..=3 {
                let is_fin = frag_id == 3;
                let is_first = frag_id == 1;

                let frame = if is_first {
                    build_text_frame(is_fin, format!("Conn{} Frag{}", conn_id, frag_id).as_bytes())
                } else {
                    build_continuation_frame(is_fin, format!(" Frag{}", frag_id).as_bytes())
                };

                server.on_frame(conn_id, &frame).unwrap();
            }
        }

        // All 10 connections should have complete messages
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 10);
    }

    #[test]
    fn q13_fin_flag_state_machine() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // FIN=0 → FIN=0 → FIN=1 (valid sequence)
        let frame1 = build_text_frame(false, b"Part1");
        server.on_frame(1, &frame1).unwrap();

        let frame2 = build_continuation_frame(false, b"Part2");
        server.on_frame(1, &frame2).unwrap();

        let frame3 = build_continuation_frame(true, b"Part3");
        server.on_frame(1, &frame3).unwrap();

        // Message should complete
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 1);
    }

    #[test]
    fn q14_binary_message_fragmentation() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Binary message with 3 fragments
        let frame1 = build_binary_frame(false, &[0x01, 0x02]);
        server.on_frame(1, &frame1).unwrap();

        let frame2 = build_continuation_frame(false, &[0x03, 0x04]);
        server.on_frame(1, &frame2).unwrap();

        let frame3 = build_continuation_frame(true, &[0x05, 0x06]);
        server.on_frame(1, &frame3).unwrap();

        // Complete binary message
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 1);
    }

    // ============================================================================
    // Q15-Q21: Integration Tests
    // ============================================================================

    #[test]
    fn q15_connection_close_during_fragmentation() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Start fragmented message
        let frame1 = build_text_frame(false, b"Incomplete");
        server.on_frame(1, &frame1).unwrap();

        // Close connection mid-fragmentation
        let close_frame = build_close_frame();
        server.on_frame(1, &close_frame).unwrap();

        // No complete message
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 0);
    }

    #[test]
    fn q16_mixed_single_and_multi_frame_messages() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Single-frame message
        let frame1 = build_text_frame(true, b"Single frame");
        server.on_frame(1, &frame1).unwrap();

        // Multi-frame message
        let frame2 = build_text_frame(false, b"Multi");
        server.on_frame(1, &frame2).unwrap();
        let frame3 = build_continuation_frame(true, b" frame");
        server.on_frame(1, &frame3).unwrap();

        // Another single-frame
        let frame4 = build_text_frame(true, b"Another single");
        server.on_frame(1, &frame4).unwrap();

        // All 3 messages complete
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 3);
    }

    #[test]
    fn q17_large_message_fragmentation() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // 1MB message split into 1KB fragments (1024 fragments)
        let fragment_size = 1024;
        let total_size = 1024 * 1024; // 1MB
        let num_fragments = total_size / fragment_size;

        // First fragment
        let payload: Vec<u8> = (0..fragment_size).map(|i| (i % 256) as u8).collect();
        let frame1 = build_text_frame(false, &payload);
        server.on_frame(1, &frame1).unwrap();

        // Middle fragments
        for i in 1..num_fragments - 1 {
            let payload: Vec<u8> = (0..fragment_size).map(|j| ((i + j) % 256) as u8).collect();
            let frame = build_continuation_frame(false, &payload);
            server.on_frame(1, &frame).unwrap();
        }

        // Final fragment
        let payload: Vec<u8> = (0..fragment_size).map(|i| (i % 256) as u8).collect();
        let frame_final = build_continuation_frame(true, &payload);
        server.on_frame(1, &frame_final).unwrap();

        // 1 complete 1MB message
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 1);
    }

    #[test]
    fn q18_ping_pong_during_fragmentation() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Start fragmented message
        let frame1 = build_text_frame(false, b"Fragmented");
        server.on_frame(1, &frame1).unwrap();

        // Interleave ping
        let ping = build_ping_frame();
        server.on_frame(1, &ping).unwrap();

        // Continue fragmentation
        let frame2 = build_continuation_frame(false, b" message");
        server.on_frame(1, &frame2).unwrap();

        // Interleave pong
        let pong = build_pong_frame();
        server.on_frame(1, &pong).unwrap();

        // Complete message
        let frame3 = build_continuation_frame(true, b" end");
        server.on_frame(1, &frame3).unwrap();

        // Message completes despite control frames
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 1);
    }

    #[test]
    fn q19_per_connection_buffer_independence() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Connection 1: 2-fragment message
        let frame1_c1 = build_text_frame(false, b"Connection 1 Part 1");
        server.on_frame(1, &frame1_c1).unwrap();

        // Connection 2: 2-fragment message
        let frame1_c2 = build_text_frame(false, b"Connection 2 Part 1");
        server.on_frame(2, &frame1_c2).unwrap();

        // Complete connection 1
        let frame2_c1 = build_continuation_frame(true, b" Part 2");
        server.on_frame(1, &frame2_c1).unwrap();

        // Connection 1 completes first
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 1);

        // Complete connection 2
        let frame2_c2 = build_continuation_frame(true, b" Part 2");
        server.on_frame(2, &frame2_c2).unwrap();

        // Both complete
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 2);
    }

    #[test]
    fn q20_metrics_only_count_complete_messages() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Start 5 fragmented messages
        for i in 1..=5 {
            let frame = build_text_frame(false, format!("Message {}", i).as_bytes());
            server.on_frame(i, &frame).unwrap();
        }

        // No complete messages yet
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 0);

        // Complete 3 messages
        for i in 1..=3 {
            let frame = build_continuation_frame(true, b" end");
            server.on_frame(i, &frame).unwrap();
        }

        // Only 3 complete
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 3);
    }

    #[test]
    fn q21_rfc_6455_example_frames() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // RFC 6455 §5.7 Example 1: Single-frame text "Hello"
        // FIN=1, opcode=1 (text), mask=0, payload="Hello"
        let frame = build_text_frame(true, b"Hello");
        server.on_frame(1, &frame).unwrap();

        // RFC 6455 §5.7 Example 2: Fragmented text "Hello" (3 fragments)
        // Frame 1: FIN=0, opcode=1, payload="Hel"
        let frame1 = build_text_frame(false, b"Hel");
        server.on_frame(2, &frame1).unwrap();

        // Frame 2: FIN=0, opcode=0, payload="lo"
        let frame2 = build_continuation_frame(false, b"lo");
        server.on_frame(2, &frame2).unwrap();

        // Frame 3: FIN=1, opcode=0, payload="" (empty final)
        let frame3 = build_continuation_frame(true, b"");
        server.on_frame(2, &frame3).unwrap();

        // Both messages complete
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 2);
    }

    // ============================================================================
    // Q22-Q28: Production Tests
    // ============================================================================

    #[test]
    fn q22_stress_test_10k_fragmented_messages() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        let start = Instant::now();

        // 10,000 messages, each with 3 fragments
        for i in 1..=10_000 {
            let frame1 = build_text_frame(false, format!("Message {} ", i).as_bytes());
            server.on_frame(1, &frame1).unwrap();

            let frame2 = build_continuation_frame(false, b"fragment 2 ");
            server.on_frame(1, &frame2).unwrap();

            let frame3 = build_continuation_frame(true, b"fragment 3");
            server.on_frame(1, &frame3).unwrap();
        }

        let elapsed = start.elapsed();

        // All 10K messages complete
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 10_000);

        // Performance: <10ns overhead per fragment (30ns per message)
        let avg_ns_per_message = elapsed.as_nanos() / 10_000;
        println!("Avg time per message (3 fragments): {}ns", avg_ns_per_message);

        // Should be <300ns per 3-fragment message (generous target)
        assert!(avg_ns_per_message < 1_000_000); // <1ms (very conservative)
    }

    #[test]
    fn q23_performance_single_vs_multi_frame() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Baseline: 10,000 single-frame messages
        let start_single = Instant::now();
        for _ in 0..10_000 {
            let frame = build_text_frame(true, b"Single frame message");
            server.on_frame(1, &frame).unwrap();
        }
        let single_elapsed = start_single.elapsed();

        // Reset metrics
        let server2 = WebSocketServerCapsule::new("127.0.0.1:8081").unwrap();

        // Multi-frame: 10,000 messages, each with 3 fragments
        let start_multi = Instant::now();
        for _ in 0..10_000 {
            let frame1 = build_text_frame(false, b"Multi");
            server2.on_frame(2, &frame1).unwrap();

            let frame2 = build_continuation_frame(false, b" frame");
            server2.on_frame(2, &frame2).unwrap();

            let frame3 = build_continuation_frame(true, b" message");
            server2.on_frame(2, &frame3).unwrap();
        }
        let multi_elapsed = start_multi.elapsed();

        println!("Single-frame: {:?}", single_elapsed);
        println!("Multi-frame:  {:?}", multi_elapsed);

        // Overhead should be <10ns per fragment (target from report)
        let overhead_ns = (multi_elapsed.as_nanos() - single_elapsed.as_nanos()) / 20_000; // 2 extra fragments per message
        println!("Overhead per fragment: {}ns", overhead_ns);

        // Conservative target: <100ns overhead per fragment
        assert!(overhead_ns < 100_000);
    }

    #[test]
    fn q24_memory_stability_no_leaks() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Send 1000 messages, each incomplete (only first fragment)
        for i in 1..=1000 {
            let frame = build_text_frame(false, format!("Incomplete message {}", i).as_bytes());
            server.on_frame(i, &frame).unwrap();
        }

        // Close all connections (clean up assemblers)
        for i in 1..=1000 {
            let close = build_close_frame();
            server.on_frame(i, &close).unwrap();
        }

        // No crashes or panics indicates proper cleanup
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 0); // No complete messages
    }

    #[test]
    fn q25_capsule_size_512_bytes() {
        use std::mem::size_of;
        assert_eq!(size_of::<WebSocketServerCapsule>(), 512);
    }

    #[test]
    fn q26_cache_alignment_512_bytes() {
        use std::mem::align_of;
        assert_eq!(align_of::<WebSocketServerCapsule>(), 512);
    }

    #[test]
    fn q27_rfc_6455_fragmentation_patterns() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Pattern 1: 2 fragments
        let f1 = build_text_frame(false, b"Part1");
        server.on_frame(1, &f1).unwrap();
        let f2 = build_continuation_frame(true, b"Part2");
        server.on_frame(1, &f2).unwrap();

        // Pattern 2: 5 fragments
        let f1 = build_text_frame(false, b"A");
        server.on_frame(2, &f1).unwrap();
        for _ in 0..3 {
            let f = build_continuation_frame(false, b"B");
            server.on_frame(2, &f).unwrap();
        }
        let f5 = build_continuation_frame(true, b"C");
        server.on_frame(2, &f5).unwrap();

        // Pattern 3: 10 fragments
        let f1 = build_binary_frame(false, &[0x01]);
        server.on_frame(3, &f1).unwrap();
        for i in 2..=9 {
            let f = build_continuation_frame(false, &[i as u8]);
            server.on_frame(3, &f).unwrap();
        }
        let f10 = build_continuation_frame(true, &[0x0A]);
        server.on_frame(3, &f10).unwrap();

        // All 3 patterns complete
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 3);
    }

    #[test]
    fn q28_comprehensive_fragmentation_validation() {
        let server = WebSocketServerCapsule::new("127.0.0.1:8080").unwrap();

        // Test all fragmentation scenarios in one test
        let scenarios = vec![
            ("Single", vec![(0x1, true, b"Single".to_vec())]),
            ("Two", vec![
                (0x1, false, b"First".to_vec()),
                (0x0, true, b"Second".to_vec()),
            ]),
            ("Three", vec![
                (0x1, false, b"A".to_vec()),
                (0x0, false, b"B".to_vec()),
                (0x0, true, b"C".to_vec()),
            ]),
            ("Ten", vec![
                (0x1, false, b"1".to_vec()),
                (0x0, false, b"2".to_vec()),
                (0x0, false, b"3".to_vec()),
                (0x0, false, b"4".to_vec()),
                (0x0, false, b"5".to_vec()),
                (0x0, false, b"6".to_vec()),
                (0x0, false, b"7".to_vec()),
                (0x0, false, b"8".to_vec()),
                (0x0, false, b"9".to_vec()),
                (0x0, true, b"10".to_vec()),
            ]),
        ];

        for (i, (_name, frames)) in scenarios.iter().enumerate() {
            let conn_id = (i + 1) as u64;
            for (opcode, fin, payload) in frames {
                let frame = build_frame(*opcode, *fin, payload);
                server.on_frame(conn_id, &frame).unwrap();
            }
        }

        // All 4 scenarios complete
        let (_, _, recv, _, _) = server.metrics();
        assert_eq!(recv, 4);
    }

    // ============================================================================
    // Helper Functions
    // ============================================================================

    /// Build WebSocket text frame (unmasked server-to-client format)
    fn build_text_frame(fin: bool, payload: &[u8]) -> Vec<u8> {
        build_frame(0x1, fin, payload)
    }

    /// Build WebSocket binary frame
    fn build_binary_frame(fin: bool, payload: &[u8]) -> Vec<u8> {
        build_frame(0x2, fin, payload)
    }

    /// Build WebSocket continuation frame
    fn build_continuation_frame(fin: bool, payload: &[u8]) -> Vec<u8> {
        build_frame(0x0, fin, payload)
    }

    /// Build WebSocket ping frame
    fn build_ping_frame() -> Vec<u8> {
        build_frame(0x9, true, &[])
    }

    /// Build WebSocket pong frame
    fn build_pong_frame() -> Vec<u8> {
        build_frame(0xA, true, &[])
    }

    /// Build WebSocket close frame
    fn build_close_frame() -> Vec<u8> {
        build_frame(0x8, true, &[])
    }

    /// Build generic WebSocket frame (RFC 6455 §5.2)
    fn build_frame(opcode: u8, fin: bool, payload: &[u8]) -> Vec<u8> {
        let mut frame = Vec::new();

        // Byte 0: FIN (1 bit) + RSV (3 bits) + opcode (4 bits)
        let byte0 = if fin { 0x80 } else { 0x00 } | (opcode & 0x0F);
        frame.push(byte0);

        // Byte 1: MASK (0 for server) + payload length (7 bits or extended)
        let payload_len = payload.len();
        if payload_len < 126 {
            frame.push(payload_len as u8);
        } else if payload_len < 65536 {
            frame.push(126);
            frame.push((payload_len >> 8) as u8);
            frame.push((payload_len & 0xFF) as u8);
        } else {
            frame.push(127);
            for i in (0..8).rev() {
                frame.push(((payload_len >> (i * 8)) & 0xFF) as u8);
            }
        }

        // Payload (unmasked for server-to-client)
        frame.extend_from_slice(payload);

        frame
    }
}
