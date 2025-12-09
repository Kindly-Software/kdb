//! QUIC Frame Parser Capsule Demo (T2 SIMD, RFC 9000 §12.4)
//!
//! Demonstrates high-performance SIMD-accelerated QUIC frame boundary detection.

fn main() {
    println!("QUIC Frame Parser Capsule Demo");
    println!("==============================\n");

    // This example requires the network feature and std
    #[cfg(all(feature = "std", feature = "network"))]
    {
        use atomic_capsule::network::{FrameParserCapsule, FrameType};

        // Create parser capsule
        let parser = FrameParserCapsule::new();
        println!("Created FrameParserCapsule (256B cache-aligned)");
        println!("SIMD enabled: {}\n", parser.is_simd_enabled());

        // Example 1: Single frame
        println!("Example 1: Single QUIC frame");
        let packet1 = vec![0x00];  // PADDING frame
        let frames1 = parser.parse_frames(&packet1);
        for frame in &frames1 {
            println!("  Frame at offset {}: {}", frame.offset, frame.frame_type);
        }
        println!("  Total: {} frames\n", frames1.len());

        // Example 2: Multiple frames
        println!("Example 2: Multiple QUIC frames");
        let packet2 = vec![
            0x01,  // PING
            0x02,  // ACK
            0x08,  // STREAM (with flags)
            0x10,  // MAX_DATA
            0x1e,  // HANDSHAKE_DONE
        ];
        let frames2 = parser.parse_frames(&packet2);
        for frame in &frames2 {
            println!("  Frame at offset {}: {}", frame.offset, frame.frame_type);
        }
        println!("  Total: {} frames\n", frames2.len());

        // Example 3: Large packet with sparse frames
        println!("Example 3: Large packet with sparse frames");
        let mut packet3 = vec![0xff; 1000];  // Filler bytes
        packet3[0] = 0x00;      // PADDING
        packet3[100] = 0x01;    // PING
        packet3[500] = 0x02;    // ACK
        packet3[999] = 0x1e;    // HANDSHAKE_DONE

        let frames3 = parser.parse_frames(&packet3);
        println!("  Found {} frames in 1000-byte packet", frames3.len());
        for frame in &frames3 {
            println!("    Offset {}: {}", frame.offset, frame.frame_type);
        }
        println!();

        // Performance metrics
        println!("Performance Metrics:");
        println!("  Frames parsed: {}", parser.frames_parsed());
        println!("  Bytes processed: {}", parser.bytes_processed());
        println!("  Expected: <100ns per packet (SIMD), <2μs per packet (scalar)\n");

        // Frame type examples
        println!("QUIC Frame Types (RFC 9000 §12.4):");
        let frame_types = vec![
            (0x00, "PADDING"),
            (0x01, "PING"),
            (0x02, "ACK"),
            (0x08, "STREAM"),
            (0x10, "MAX_DATA"),
            (0x1c, "CONNECTION_CLOSE (QUIC)"),
            (0x1d, "CONNECTION_CLOSE (app)"),
            (0x1e, "HANDSHAKE_DONE"),
        ];
        for (code, name) in frame_types {
            let ft = FrameType::from_byte(code);
            println!("  0x{:02x}: {} (valid: {})", code, name, ft.is_valid());
        }

        // Size and alignment verification
        println!("\nCapsule Layout:");
        println!("  Size: {} bytes", std::mem::size_of::<FrameParserCapsule>());
        println!("  Alignment: {} bytes", std::mem::align_of::<FrameParserCapsule>());
    }

    #[cfg(not(all(feature = "std", feature = "network")))]
    {
        println!("This example requires features: std,network");
        println!("Run with: cargo run --example frame_parser_demo --features std,network");
    }
}
