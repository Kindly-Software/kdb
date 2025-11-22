//! HTTP/2 Frame Parser Capsule - Usage Examples
//!
//! Demonstrates parsing various HTTP/2 frame types with RFC 9113 compliance.
//!
//! Run with: cargo run --example http2_frame_parser_demo --features std

use atomic_capsule::http::{
    Http2FrameParserCapsule, Http2FrameType, Http2Flags, Http2FrameHeader, Http2Frame,
};

fn main() {
    println!("=== HTTP/2 Frame Parser Capsule Demo (RFC 9113) ===\n");

    example_1_simple_data_frame();
    example_2_settings_frame();
    example_3_headers_frame();
    example_4_frame_with_padding();
    example_5_statistics_collection();
    example_6_error_handling();
    example_7_frame_serialization();
    example_8_max_frame_size_config();

    println!("\n=== All Examples Complete ===");
}

/// Example 1: Parse simple DATA frame (5 bytes payload)
fn example_1_simple_data_frame() {
    println!("Example 1: Parse Simple DATA Frame");
    println!("────────────────────────────────\n");

    let parser = Http2FrameParserCapsule::new();

    // Create DATA frame: "Hello" on stream 1
    let data_frame = [
        0x00, 0x00, 0x05, // Length: 5 bytes
        0x00,             // Type: DATA (0x00)
        0x01,             // Flags: END_STREAM (0x01)
        0x00, 0x00, 0x00, 0x01, // Stream ID: 1
        // Payload: "Hello"
        b'H', b'e', b'l', b'l', b'o',
    ];

    match parser.parse_frame(&data_frame) {
        Ok((header, size)) => {
            println!("✓ Frame parsed successfully");
            println!("  Frame Type:    {:?}", header.frame_type);
            println!("  Stream ID:     {}", header.stream_id);
            println!("  Payload Size:  {} bytes", header.length);
            println!("  Total Size:    {} bytes (9 header + {} payload)", size, header.length);
            println!("  END_STREAM:    {}", header.flags.end_stream());
            println!("  Max Frame Size: {}", parser.get_max_frame_size());
        }
        Err(e) => println!("✗ Parse error: {:?}", e),
    }

    let stats = parser.stats();
    println!("\n  Stats after parsing:");
    println!("    Total frames: {}", stats.frames_parsed);
    println!("    DATA frames:  {}", stats.data_frames);
    println!("    Total bytes:  {} (payload only)", stats.total_bytes_parsed);

    println!();
}

/// Example 2: Parse SETTINGS frame (connection-level, stream ID must be 0)
fn example_2_settings_frame() {
    println!("Example 2: Parse SETTINGS Frame");
    println!("──────────────────────────────\n");

    let parser = Http2FrameParserCapsule::new();

    // SETTINGS frame: empty payload (no settings), connection-level (stream ID = 0)
    let settings_frame = [
        0x00, 0x00, 0x00, // Length: 0 bytes
        0x04,             // Type: SETTINGS (0x04)
        0x00,             // Flags: none
        0x00, 0x00, 0x00, 0x00, // Stream ID: 0 (connection-level)
    ];

    match parser.parse_frame(&settings_frame) {
        Ok((header, size)) => {
            println!("✓ SETTINGS frame parsed");
            println!("  Frame Type:   {:?}", header.frame_type);
            println!("  Stream ID:    {} (connection-level)", header.stream_id);
            println!("  Total Size:   {} bytes (header only)", size);
            println!("  ACK Flag:     {}", header.flags.ack());
        }
        Err(e) => println!("✗ Parse error: {:?}", e),
    }

    println!();
}

/// Example 3: Parse HEADERS frame (with flags)
fn example_3_headers_frame() {
    println!("Example 3: Parse HEADERS Frame with Flags");
    println!("────────────────────────────────────────\n");

    let parser = Http2FrameParserCapsule::new();

    // HEADERS frame with END_HEADERS + END_STREAM flags on stream 1
    let headers_frame = [
        0x00, 0x00, 0x0C, // Length: 12 bytes
        0x01,             // Type: HEADERS (0x01)
        0x05,             // Flags: END_STREAM (0x01) | END_HEADERS (0x04) = 0x05
        0x00, 0x00, 0x00, 0x01, // Stream ID: 1
        // Payload: 12 bytes of HPACK-encoded headers (simplified)
        b'C', b'O', b'C', b'A',
        b'H', b'2', b'F', b'R',
        b'A', b'M', b'E', b'S',
    ];

    match parser.parse_frame(&headers_frame) {
        Ok((header, size)) => {
            println!("✓ HEADERS frame parsed");
            println!("  Frame Type:      {:?}", header.frame_type);
            println!("  Stream ID:       {}", header.stream_id);
            println!("  Payload Size:    {} bytes", header.length);
            println!("  END_STREAM:      {}", header.flags.end_stream());
            println!("  END_HEADERS:     {}", header.flags.end_headers());
            println!("  PRIORITY Flag:   {}", header.flags.priority());
            println!("  Total Size:      {} bytes", size);
        }
        Err(e) => println!("✗ Parse error: {:?}", e),
    }

    println!();
}

/// Example 4: Parse DATA frame with PADDED flag
fn example_4_frame_with_padding() {
    println!("Example 4: DATA Frame with Padding");
    println!("──────────────────────────────────\n");

    let parser = Http2FrameParserCapsule::new();

    // DATA frame with padding: [pad_len=3] + "data" + [padding_bytes]
    let padded_frame = [
        0x00, 0x00, 0x08, // Length: 8 bytes
        0x00,             // Type: DATA (0x00)
        0x09,             // Flags: END_STREAM (0x01) | PADDED (0x08) = 0x09
        0x00, 0x00, 0x00, 0x03, // Stream ID: 3
        // Payload: [pad_len=3] + "data" + [3 padding bytes]
        0x03,             // Pad length: 3 bytes
        b'd', b'a', b't', b'a', // Actual data: 4 bytes
        0x00, 0x00, 0x00, // Padding: 3 bytes
    ];

    match parser.parse_frame(&padded_frame) {
        Ok((header, size)) => {
            println!("✓ Padded DATA frame parsed");
            println!("  Frame Type:     {:?}", header.frame_type);
            println!("  Stream ID:      {}", header.stream_id);
            println!("  Total Size:     {} bytes", size);
            println!("  PADDED Flag:    {}", header.flags.padded());
            println!("  END_STREAM:     {}", header.flags.end_stream());

            // Extract frame and get padding info
            let frame = Http2Frame::new(header, &padded_frame[9..]);
            match frame.padding_length() {
                Ok(pad_len) => {
                    println!("  Padding Length: {} bytes", pad_len);
                    match frame.payload_data() {
                        Ok(data) => {
                            println!(
                                "  Data (unpadded): {} bytes = {:?}",
                                data.len(),
                                std::str::from_utf8(data).unwrap_or("<invalid utf8>")
                            );
                        }
                        Err(e) => println!("  ✗ Error extracting data: {:?}", e),
                    }
                }
                Err(e) => println!("  ✗ Error getting padding: {:?}", e),
            }
        }
        Err(e) => println!("✗ Parse error: {:?}", e),
    }

    println!();
}

/// Example 5: Statistics collection across multiple frames
fn example_5_statistics_collection() {
    println!("Example 5: Statistics Collection");
    println!("────────────────────────────────\n");

    let parser = Http2FrameParserCapsule::new();

    // Create multiple frame types
    let data_frame = [0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, b'H', b'i', b'!', b' ', b' '];
    let settings_frame = [0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00];
    let ping_frame = [0x00, 0x00, 0x08, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 1, 2, 3, 4, 5, 6, 7, 8];

    println!("Parsing 3 frames...");
    let _ = parser.parse_frame(&data_frame);
    let _ = parser.parse_frame(&settings_frame);
    let _ = parser.parse_frame(&ping_frame);

    let stats = parser.stats();
    println!("✓ Parsing complete\n");
    println!("  Total frames:        {}", stats.frames_parsed);
    println!("  DATA frames:         {}", stats.data_frames);
    println!("  SETTINGS frames:     {}", stats.settings_frames);
    println!("  PING frames:         {}", stats.ping_frames);
    println!("  Total bytes parsed:  {} (payload only)", stats.total_bytes_parsed);
    println!("  Last stream ID:      {}", stats.last_stream_id);
    println!("  Parse errors:        {}", stats.parse_errors);

    println!();
}

/// Example 6: Error handling for various invalid frames
fn example_6_error_handling() {
    println!("Example 6: Error Handling");
    println!("─────────────────────────\n");

    let parser = Http2FrameParserCapsule::new();

    // Test 1: Incomplete frame (< 9 bytes)
    println!("Test 1: Incomplete frame header");
    let incomplete = &[0x00, 0x00]; // Only 2 bytes
    match parser.parse_frame(incomplete) {
        Ok(_) => println!("  ✗ Should have failed"),
        Err(e) => println!("  ✓ Correctly rejected: {:?}", e),
    }

    // Test 2: Invalid frame type
    println!("\nTest 2: Invalid frame type");
    let mut bad_type = [0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00]; // Type=0xFF
    match parser.parse_frame(&bad_type) {
        Ok(_) => println!("  ✗ Should have failed"),
        Err(e) => println!("  ✓ Correctly rejected: {:?}", e),
    }

    // Test 3: Invalid stream ID (SETTINGS with non-zero stream ID)
    println!("\nTest 3: Invalid stream ID (SETTINGS on stream 1)");
    let bad_stream = [0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x01]; // SETTINGS with stream ID=1
    match parser.parse_frame(&bad_stream) {
        Ok(_) => println!("  ✗ Should have failed"),
        Err(e) => println!("  ✓ Correctly rejected: {:?}", e),
    }

    // Test 4: Frame too large (exceeds default 16KB)
    println!("\nTest 4: Frame too large");
    let mut oversized = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    oversized[0] = 0xFF; // Set length to 0xFF0000 (> 16KB)
    oversized[1] = 0xFF;
    oversized[2] = 0xFF;
    match parser.parse_frame(&oversized) {
        Ok(_) => println!("  ✗ Should have failed"),
        Err(e) => println!("  ✓ Correctly rejected: {:?}", e),
    }

    println!();
}

/// Example 7: Frame serialization (round-trip)
fn example_7_frame_serialization() {
    println!("Example 7: Frame Serialization (Round-Trip)");
    println!("───────────────────────────────────────────\n");

    // Create a frame header
    let original = Http2FrameHeader {
        length: 42,
        frame_type: Http2FrameType::Data,
        flags: Http2Flags::new(0x01),
        stream_id: 7,
    };

    println!("Original header:");
    println!("  Length:    {}", original.length);
    println!("  Type:      {:?}", original.frame_type);
    println!("  Flags:     0x{:02X}", original.flags.as_u8());
    println!("  Stream ID: {}", original.stream_id);

    // Serialize to buffer
    let mut buffer = [0u8; 9];
    match original.serialize(&mut buffer) {
        Ok(_) => {
            println!("\n✓ Serialized to 9-byte buffer");

            // Parse back from buffer
            match Http2FrameHeader::parse(&buffer) {
                Ok(parsed) => {
                    println!("✓ Parsed back from buffer\n");
                    println!("Parsed header:");
                    println!("  Length:    {}", parsed.length);
                    println!("  Type:      {:?}", parsed.frame_type);
                    println!("  Flags:     0x{:02X}", parsed.flags.as_u8());
                    println!("  Stream ID: {}", parsed.stream_id);

                    // Verify round-trip
                    if original == parsed {
                        println!("\n✓ Round-trip successful (serialize → parse == original)");
                    } else {
                        println!("\n✗ Round-trip mismatch");
                    }
                }
                Err(e) => println!("✗ Parse error: {:?}", e),
            }
        }
        Err(e) => println!("✗ Serialize error: {:?}", e),
    }

    println!();
}

/// Example 8: Configure maximum frame size
fn example_8_max_frame_size_config() {
    println!("Example 8: Configure Maximum Frame Size");
    println!("───────────────────────────────────────\n");

    let parser = Http2FrameParserCapsule::new();

    println!("Default max frame size: {} bytes", parser.get_max_frame_size());

    // Increase max frame size to 64KB
    match parser.set_max_frame_size(65536) {
        Ok(_) => {
            println!("✓ Set max frame size to 65,536 bytes");
            println!("  New max frame size: {} bytes", parser.get_max_frame_size());

            // Create a frame that would exceed default but fits in new size
            let mut large_frame = vec![0u8; 50000 + 9];
            large_frame[0] = 0x0C; // Length high byte
            large_frame[1] = 0x35; // Length middle byte
            large_frame[2] = 0x30; // Length low byte (0x0C3530 = 801,328 > 65536, too large)

            // Actually, let's make a valid large frame within new limit
            large_frame[0] = 0x00;
            large_frame[1] = 0xFF;
            large_frame[2] = 0xFF; // 0x00FFFF = 65,535 bytes
            large_frame[3] = 0x00; // DATA frame
            large_frame[4] = 0x00; // No flags
            large_frame[5] = 0x00;
            large_frame[6] = 0x00;
            large_frame[7] = 0x00;
            large_frame[8] = 0x01; // Stream ID: 1

            println!("\n  Testing frame with 65,535-byte payload...");
            match parser.parse_frame(&large_frame) {
                Ok(_) => println!("  ✓ Large frame accepted"),
                Err(e) => println!("  ✗ Large frame rejected: {:?}", e),
            }
        }
        Err(e) => println!("✗ Failed to set max frame size: {:?}", e),
    }

    // Try invalid size
    println!("\n  Testing invalid max frame size (8KB, below minimum 16KB)...");
    match parser.set_max_frame_size(8192) {
        Ok(_) => println!("  ✗ Should have rejected"),
        Err(e) => println!("  ✓ Correctly rejected: {:?}", e),
    }

    println!();
}
