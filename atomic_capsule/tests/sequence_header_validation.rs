//! Sequence Header OBU Validation Tests
//!
//! Tests for spec-compliant AV1 sequence header generation.
//! Validates against AV1 specification §5.5.

use atomic_capsule::encoder::ObuBitstreamWriterCapsule;

#[test]
fn test_sequence_header_obu_structure_1920x1080() {
    let writer = ObuBitstreamWriterCapsule::new();
    let obu = writer.write_sequence_header(1920, 1080);

    // OBU header: type=1 (SequenceHeader), has_size=1
    // Binary: 0b0000_1010 = 0x0A
    assert_eq!(obu[0], 0x0A, "OBU header must be 0x0A (type=1, has_size=1)");

    // LEB128 size (payload length, should be ~23-25 bytes)
    let size = obu[1];
    assert!(
        size >= 20 && size < 128,
        "Payload size must be 20-127 bytes (1-byte LEB128), got {}",
        size
    );

    // Total OBU length
    let expected_len = 1 + 1 + size as usize; // header + size + payload
    assert_eq!(
        obu.len(),
        expected_len,
        "OBU length mismatch: expected {}, got {}",
        expected_len,
        obu.len()
    );

    println!(
        "✓ Sequence header OBU structure valid: {} bytes (header:1 + size:1 + payload:{})",
        obu.len(),
        size
    );
}

#[test]
fn test_sequence_header_various_resolutions() {
    let writer = ObuBitstreamWriterCapsule::new();

    let resolutions = [
        (320, 240, "QVGA"),
        (640, 360, "nHD"),
        (1280, 720, "HD 720p"),
        (1920, 1080, "Full HD 1080p"),
        (3840, 2160, "4K UHD"),
    ];

    for (width, height, name) in resolutions {
        let obu = writer.write_sequence_header(width, height);

        // All sequence headers should have same structure
        assert_eq!(
            obu[0], 0x0A,
            "{}: OBU header must be 0x0A",
            name
        );

        // Payload size varies based on dimension bit requirements
        let payload_size = obu[1];
        assert!(
            payload_size >= 15 && payload_size <= 40,
            "{}: Payload size {} out of expected range 15-40",
            name,
            payload_size
        );

        println!(
            "✓ {}: {} bytes ({}×{})",
            name,
            obu.len(),
            width,
            height
        );
    }
}

#[test]
fn test_sequence_header_determinism() {
    let writer = ObuBitstreamWriterCapsule::new();

    // Generate same header 10 times
    let reference = writer.write_sequence_header(1920, 1080);

    for i in 0..10 {
        let obu = writer.write_sequence_header(1920, 1080);
        assert_eq!(
            obu, reference,
            "Iteration {}: Sequence header must be deterministic",
            i
        );
    }

    println!("✓ Sequence header is deterministic (10 iterations)");
}

#[test]
fn test_sequence_header_bit_patterns() {
    let writer = ObuBitstreamWriterCapsule::new();
    let obu = writer.write_sequence_header(1920, 1080);

    // Extract payload (skip OBU header + size bytes)
    let payload = &obu[2..];

    // First byte should start with 000_0_0_0_0_0 (profile=0, flags=0, timing=0, decoder=0, display=0)
    // Followed by operating_points_cnt_minus_1 (5 bits = 0)
    // = 0b0000_0000 = 0x00
    assert_eq!(
        payload[0], 0x00,
        "First payload byte must be 0x00 (profile=0, all flags=0)"
    );

    println!("✓ Sequence header bit patterns valid");
    println!("  Payload bytes: {:?}", &payload[..8.min(payload.len())]);
}

#[test]
fn test_sequence_header_checksum_update() {
    let writer = ObuBitstreamWriterCapsule::new();
    let checksum_before = writer.checksum();

    let _obu = writer.write_sequence_header(1920, 1080);

    let checksum_after = writer.checksum();
    assert_ne!(
        checksum_before, checksum_after,
        "Checksum must be updated after OBU write"
    );

    println!(
        "✓ Checksum updated: 0x{:016X} → 0x{:016X}",
        checksum_before, checksum_after
    );
}

#[test]
fn test_sequence_header_obu_count() {
    let writer = ObuBitstreamWriterCapsule::new();
    assert_eq!(writer.obu_count(), 0, "Initial OBU count must be 0");

    writer.write_sequence_header(1920, 1080);
    assert_eq!(writer.obu_count(), 1, "OBU count must be 1 after first write");

    writer.write_sequence_header(1920, 1080);
    assert_eq!(
        writer.obu_count(),
        2,
        "OBU count must be 2 after second write"
    );

    println!("✓ OBU count tracking works correctly");
}

#[test]
fn test_sequence_header_leb128_encoding() {
    let writer = ObuBitstreamWriterCapsule::new();
    let obu = writer.write_sequence_header(1920, 1080);

    // LEB128 size should be < 128 (single byte encoding)
    let size_byte = obu[1];
    assert!(
        size_byte < 0x80,
        "Size should use 1-byte LEB128 encoding, got 0x{:02X}",
        size_byte
    );

    // Size value (lower 7 bits) should match actual payload length
    let size_value = size_byte & 0x7F;
    let actual_payload_len = obu.len() - 2; // minus header and size byte
    assert_eq!(
        size_value as usize, actual_payload_len,
        "LEB128 size value {} doesn't match actual payload length {}",
        size_value, actual_payload_len
    );

    println!("✓ LEB128 encoding correct: {} bytes payload", size_value);
}

#[test]
fn test_sequence_header_minimum_size() {
    let writer = ObuBitstreamWriterCapsule::new();

    // Even smallest resolution should have reasonable size
    let obu = writer.write_sequence_header(64, 64);

    assert!(
        obu.len() >= 15,
        "Even 64×64 sequence header must be at least 15 bytes, got {}",
        obu.len()
    );

    println!("✓ Minimum size validation passed: {} bytes for 64×64", obu.len());
}

#[test]
fn test_sequence_header_hexdump() {
    let writer = ObuBitstreamWriterCapsule::new();
    let obu = writer.write_sequence_header(1920, 1080);

    println!("\n=== Sequence Header OBU Hexdump (1920×1080) ===");
    println!("Total size: {} bytes\n", obu.len());

    for (i, chunk) in obu.chunks(16).enumerate() {
        print!("{:04X}  ", i * 16);

        // Hex bytes
        for (j, byte) in chunk.iter().enumerate() {
            print!("{:02X} ", byte);
            if j == 7 {
                print!(" ");
            }
        }

        // Padding
        for _ in chunk.len()..16 {
            print!("   ");
            if chunk.len() <= 8 {
                print!(" ");
            }
        }

        // ASCII representation
        print!(" |");
        for byte in chunk {
            if byte.is_ascii_graphic() {
                print!("{}", *byte as char);
            } else {
                print!(".");
            }
        }
        println!("|");
    }

    println!("\nByte breakdown:");
    println!("  [0]    : 0x{:02X} = OBU header (type=1, has_size=1)", obu[0]);
    println!("  [1]    : 0x{:02X} = LEB128 size ({} bytes)", obu[1], obu[1] & 0x7F);
    println!("  [2-{}]: Sequence header payload", obu.len() - 1);
}
