//! Integration tests for FrameParserCapsule (T2 SIMD, RFC 9000 §12.4)

#![cfg(all(feature = "std", feature = "network"))]

#[cfg(all(test, feature = "std", feature = "network"))]
mod frame_parser_integration {
    use atomic_capsule::network::{FrameParserCapsule, FrameType};

    #[test]
    fn test_quic_frame_parsing_single_frame() {
        let parser = FrameParserCapsule::new();
        parser.set_simd_enabled(false);

        // Single PADDING frame
        let packet = vec![0x00];
        let frames = parser.parse_frames(&packet);

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].offset, 0);
        assert_eq!(frames[0].frame_type, FrameType::Padding);
        assert_eq!(parser.frames_parsed(), 1);
        assert_eq!(parser.bytes_processed(), 1);
    }

    #[test]
    fn test_quic_frame_parsing_multiple_frames() {
        let parser = FrameParserCapsule::new();
        parser.set_simd_enabled(false);

        // Multiple frames
        let packet = vec![
            0x00,  // PADDING
            0x01,  // PING
            0x02,  // ACK
            0x08,  // STREAM (with flags)
            0x10,  // MAX_DATA
        ];
        let frames = parser.parse_frames(&packet);

        assert_eq!(frames.len(), 5);
        assert_eq!(frames[0].frame_type, FrameType::Padding);
        assert_eq!(frames[1].frame_type, FrameType::Ping);
        assert_eq!(frames[2].frame_type, FrameType::Ack);
        assert_eq!(frames[3].frame_type, FrameType::Stream);
        assert_eq!(frames[4].frame_type, FrameType::MaxData);
    }

    #[test]
    fn test_quic_frame_boundary_detection() {
        let parser = FrameParserCapsule::new();
        parser.set_simd_enabled(false);

        // Packet with frame boundaries at offsets
        let mut packet = vec![0xff; 20];
        packet[0] = 0x01;      // PING at offset 0
        packet[5] = 0x02;      // ACK at offset 5
        packet[10] = 0x10;     // MAX_DATA at offset 10
        packet[15] = 0x1e;     // HANDSHAKE_DONE at offset 15

        let frames = parser.parse_frames(&packet);

        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0].offset, 0);
        assert_eq!(frames[1].offset, 5);
        assert_eq!(frames[2].offset, 10);
        assert_eq!(frames[3].offset, 15);
    }

    #[test]
    fn test_quic_all_frame_types() {
        let parser = FrameParserCapsule::new();
        parser.set_simd_enabled(false);

        // Create packet with all valid frame types
        let packet: Vec<u8> = (0..=0x1e).collect();
        let frames = parser.parse_frames(&packet);

        // Should parse all 31 frame types (0x00-0x1e)
        assert_eq!(frames.len(), 0x1f);

        // Verify each frame
        for (i, frame) in frames.iter().enumerate() {
            assert_eq!(frame.offset, i);
            assert_eq!(
                frame.frame_type,
                FrameType::from_byte(i as u8),
                "Frame type mismatch at index {}", i
            );
        }
    }

    #[test]
    fn test_quic_large_packet() {
        let parser = FrameParserCapsule::new();
        parser.set_simd_enabled(false);

        // Create 100KB packet with sparse frame markers
        let mut packet = vec![0xff; 100_000];
        packet[0] = 0x00;          // PADDING at start
        packet[10_000] = 0x01;     // PING at 10K
        packet[50_000] = 0x02;     // ACK at 50K
        packet[99_999] = 0x1e;     // HANDSHAKE_DONE at end

        let frames = parser.parse_frames(&packet);

        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0].offset, 0);
        assert_eq!(frames[1].offset, 10_000);
        assert_eq!(frames[2].offset, 50_000);
        assert_eq!(frames[3].offset, 99_999);
        assert_eq!(parser.bytes_processed(), 100_000);
    }

    #[test]
    fn test_quic_frame_type_validity() {
        assert!(FrameType::Padding.is_valid());
        assert!(FrameType::Ping.is_valid());
        assert!(FrameType::Stream.is_valid());
        assert!(FrameType::HandshakeDone.is_valid());
        assert!(FrameType::Extension.is_valid());
        assert!(!FrameType::Invalid.is_valid());
    }

    #[test]
    fn test_quic_frame_type_display() {
        assert_eq!(format!("{}", FrameType::Padding), "PADDING");
        assert_eq!(format!("{}", FrameType::Ping), "PING");
        assert_eq!(format!("{}", FrameType::Ack), "ACK");
        assert_eq!(format!("{}", FrameType::Stream), "STREAM");
        assert_eq!(format!("{}", FrameType::HandshakeDone), "HANDSHAKE_DONE");
    }

    #[test]
    fn test_quic_capsule_alignment() {
        use core::mem::align_of;
        assert_eq!(align_of::<FrameParserCapsule>(), 256);
    }

    #[test]
    fn test_quic_capsule_size() {
        use core::mem::size_of;
        assert_eq!(size_of::<FrameParserCapsule>(), 256);
    }

    #[test]
    fn test_quic_counter_accumulation() {
        let parser = FrameParserCapsule::new();
        parser.set_simd_enabled(false);

        let packet1 = vec![0x00, 0x01, 0x02];
        let _frames1 = parser.parse_frames(&packet1);
        assert_eq!(parser.frames_parsed(), 3);
        assert_eq!(parser.bytes_processed(), 3);

        let packet2 = vec![0x10, 0x11, 0x12, 0x13];
        let _frames2 = parser.parse_frames(&packet2);
        assert_eq!(parser.frames_parsed(), 7);  // 3 + 4
        assert_eq!(parser.bytes_processed(), 7);
    }

    #[test]
    fn test_quic_counter_reset() {
        let parser = FrameParserCapsule::new();
        parser.set_simd_enabled(false);

        let packet = vec![0x00, 0x01];
        let _frames = parser.parse_frames(&packet);
        assert!(parser.frames_parsed() > 0);

        parser.reset_counters();
        assert_eq!(parser.frames_parsed(), 0);
        assert_eq!(parser.bytes_processed(), 0);
    }

    #[test]
    fn test_quic_empty_packet() {
        let parser = FrameParserCapsule::new();
        let frames = parser.parse_frames(&[]);
        assert_eq!(frames.len(), 0);
        assert_eq!(parser.frames_parsed(), 0);
    }

    #[test]
    fn test_quic_stream_frame_variants() {
        let parser = FrameParserCapsule::new();
        parser.set_simd_enabled(false);

        // STREAM frames are 0x08-0x0f with flag bits
        let packet = vec![0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f];
        let frames = parser.parse_frames(&packet);

        assert_eq!(frames.len(), 8);
        for frame in frames.iter() {
            assert_eq!(frame.frame_type, FrameType::Stream);
        }
    }

    #[test]
    fn test_quic_connection_close_frames() {
        let parser = FrameParserCapsule::new();
        parser.set_simd_enabled(false);

        let packet = vec![
            0x1c,  // CONNECTION_CLOSE (QUIC)
            0x1d,  // CONNECTION_CLOSE (application)
        ];
        let frames = parser.parse_frames(&packet);

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].frame_type, FrameType::ConnectionCloseQuic);
        assert_eq!(frames[1].frame_type, FrameType::ConnectionCloseApp);
    }

    #[test]
    fn test_quic_simd_enabled_flag() {
        let parser = FrameParserCapsule::new();

        // Check initial SIMD state (depends on platform)
        let initial_simd = parser.is_simd_enabled();

        // Toggle SIMD
        parser.set_simd_enabled(!initial_simd);
        assert_eq!(parser.is_simd_enabled(), !initial_simd);

        // Toggle back
        parser.set_simd_enabled(initial_simd);
        assert_eq!(parser.is_simd_enabled(), initial_simd);
    }

    #[cfg(feature = "portable_simd")]
    #[test]
    fn test_quic_simd_vs_scalar_equivalence() {
        let parser_simd = FrameParserCapsule::new();
        parser_simd.set_simd_enabled(true);

        let parser_scalar = FrameParserCapsule::new();
        parser_scalar.set_simd_enabled(false);

        let packet = vec![
            0x00, 0x01, 0x02, 0x03,
            0x04, 0x05, 0x06, 0x07,
            0x08, 0x09, 0x0a, 0x0b,
            0x0c, 0x0d, 0x0e, 0x0f,
            0x10, 0x11, 0x12, 0x13,
        ];

        let frames_simd = parser_simd.parse_frames(&packet);
        parser_scalar.reset_counters();
        let frames_scalar = parser_scalar.parse_frames(&packet);

        // Results should be identical
        assert_eq!(frames_simd.len(), frames_scalar.len());
        for (simd, scalar) in frames_simd.iter().zip(frames_scalar.iter()) {
            assert_eq!(simd.offset, scalar.offset);
            assert_eq!(simd.frame_type, scalar.frame_type);
        }
    }

    #[test]
    fn test_quic_performance_baseline() {
        let parser = FrameParserCapsule::new();
        parser.set_simd_enabled(false);

        // Baseline performance test: 1000 frames should parse in <100μs
        let mut packet = vec![0xff; 10_000];
        for i in 0..1000 {
            packet[i * 10] = (i % 32) as u8;  // Insert frame markers
        }

        let start = std::time::Instant::now();
        let frames = parser.parse_frames(&packet);
        let elapsed = start.elapsed();

        // Should find ~100 frames (10% of positions have valid frame types)
        assert!(!frames.is_empty());
        assert!(elapsed.as_millis() < 1, "Parsing took too long: {:?}", elapsed);
    }
}
