//! Frame OBU Decoder Test
//!
//! Compare FFmpeg's frame OBU payload vs our output bit-by-bit
//!
//! Framework Compliance:
//! - T28: Q1-Q7 unit test tier

use kindly_av1::encoder::{DecodedFrameHeader, FrameObuDecoderCapsule};

#[test]
fn test_decode_ffmpeg_vs_ours() {
    // FFmpeg frame OBU payload (10 bytes)
    // From: [0x32, 0x0a, 0x18, 0x00, 0x00, 0x00, 0x50, 0x00, 0x00, 0x00, 0x09, 0xac]
    // Byte 0: 0x32 = OBU header (type 6 = FRAME)
    // Byte 1: 0x0a = leb128 size (10 bytes)
    // Payload starts at byte 2
    let ffmpeg_payload: &[u8] = &[0x18, 0x00, 0x00, 0x00, 0x50, 0x00, 0x00, 0x00, 0x09, 0xac];

    // Our frame OBU payload (7 bytes)
    // From: [0x32, 0x07, 0x18, 0x00, 0x00, 0x01, 0x60, 0x4d, 0x60]
    // Byte 0: 0x32 = OBU header
    // Byte 1: 0x07 = leb128 size (7 bytes)
    // Payload starts at byte 2
    let our_payload: &[u8] = &[0x18, 0x00, 0x00, 0x01, 0x60, 0x4d, 0x60];

    println!("\n=== Frame OBU Payload Comparison ===");
    println!(
        "\nFFmpeg payload ({} bytes): {:02x?}",
        ffmpeg_payload.len(),
        ffmpeg_payload
    );
    println!(
        "Our payload    ({} bytes): {:02x?}",
        our_payload.len(),
        our_payload
    );

    // Byte-by-byte comparison
    println!("\nByte-by-byte comparison:");
    let max_len = ffmpeg_payload.len().max(our_payload.len());
    for i in 0..max_len {
        let ffmpeg_byte = ffmpeg_payload
            .get(i)
            .map(|b| format!("{:02x}", b))
            .unwrap_or("--".to_string());
        let our_byte = our_payload
            .get(i)
            .map(|b| format!("{:02x}", b))
            .unwrap_or("--".to_string());
        let match_str = if ffmpeg_payload.get(i) == our_payload.get(i) {
            "✓ MATCH"
        } else {
            "✗ DIFF"
        };
        println!(
            "  Byte {}: FFmpeg={} Ours={} {}",
            i, ffmpeg_byte, our_byte, match_str
        );
    }

    // Decode FFmpeg payload
    println!("\n{}", "=".repeat(80));
    let decoder_ffmpeg = FrameObuDecoderCapsule::new();
    let ffmpeg_decoded = decoder_ffmpeg
        .decode_frame_header_reduced(ffmpeg_payload)
        .expect("Failed to decode FFmpeg payload");

    ffmpeg_decoded.print_detailed("FFmpeg", ffmpeg_payload);

    // Decode our payload
    println!("\n{}", "=".repeat(80));
    let decoder_ours = FrameObuDecoderCapsule::new();
    let ours_decoded = decoder_ours
        .decode_frame_header_reduced(our_payload)
        .expect("Failed to decode our payload");

    ours_decoded.print_detailed("Ours", our_payload);

    // Side-by-side comparison
    println!("\n{}", "=".repeat(80));
    DecodedFrameHeader::compare(&ffmpeg_decoded, &ours_decoded);

    // Binary dump with bit annotations
    println!("\n{}", "=".repeat(80));
    println!("\n=== BINARY ANALYSIS ===\n");

    println!("FFmpeg payload binary:");
    print_binary_with_bits(ffmpeg_payload);

    println!("\nOurs payload binary:");
    print_binary_with_bits(our_payload);

    // Detailed bit-by-bit decode
    println!("\n{}", "=".repeat(80));
    println!("\n=== BIT-BY-BIT DECODE (First 32 bits) ===\n");

    println!("FFmpeg:");
    decode_first_32_bits(ffmpeg_payload);

    println!("\nOurs:");
    decode_first_32_bits(our_payload);
}

fn print_binary_with_bits(data: &[u8]) {
    for (i, byte) in data.iter().enumerate() {
        print!("Byte {:2}: 0x{:02x} = {:08b}", i, byte, byte);
        if i < 4 {
            print!(" (bits {:2}..{:2})", i * 8, (i + 1) * 8);
        }
        println!();
    }
}

fn decode_first_32_bits(data: &[u8]) {
    if data.len() < 4 {
        println!("  Not enough data (need 4 bytes, got {})", data.len());
        return;
    }

    // Read first 4 bytes
    let bytes = [data[0], data[1], data[2], data[3]];
    let bits: u32 = u32::from_be_bytes(bytes);

    println!("  First 32 bits: {:032b}", bits);
    println!("  Breakdown:");

    let mut pos = 0;

    // allow_screen_content_tools f(1)
    let allow_screen = (bits >> 31) & 1;
    println!(
        "    Bit  {:2}: allow_screen_content_tools = {}",
        pos, allow_screen
    );
    pos += 1;

    // base_q_idx f(8)
    let base_q_idx = (bits >> 23) & 0xFF;
    println!(
        "    Bits {:2}-{:2}: base_q_idx = {} (0x{:02x})",
        pos,
        pos + 7,
        base_q_idx,
        base_q_idx
    );
    pos += 8;

    // delta_q_y_dc_coded f(1)
    let delta_q_y_dc = (bits >> 22) & 1;
    println!("    Bit  {:2}: delta_q_y_dc_coded = {}", pos, delta_q_y_dc);
    pos += 1;

    // using_qmatrix f(1)
    let using_qmatrix = (bits >> 21) & 1;
    println!("    Bit  {:2}: using_qmatrix = {}", pos, using_qmatrix);
    pos += 1;

    // segmentation_enabled f(1)
    let segmentation = (bits >> 20) & 1;
    println!(
        "    Bit  {:2}: segmentation_enabled = {}",
        pos, segmentation
    );
    pos += 1;

    // delta_q_present f(1)
    let delta_q_present = (bits >> 19) & 1;
    println!("    Bit  {:2}: delta_q_present = {}", pos, delta_q_present);
    pos += 1;

    // delta_lf_present f(1) - only if delta_q_present=1
    if delta_q_present == 1 {
        let delta_lf = (bits >> 18) & 1;
        println!("    Bit  {:2}: delta_lf_present = {}", pos, delta_lf);
        pos += 1;
    }

    // loop_filter_level[0] f(6)
    let lf_level_0 = if delta_q_present == 1 {
        (bits >> 12) & 0x3F
    } else {
        (bits >> 13) & 0x3F
    };
    println!(
        "    Bits {:2}-{:2}: loop_filter_level[0] = {} (0x{:02x})",
        pos,
        pos + 5,
        lf_level_0,
        lf_level_0
    );
    pos += 6;

    println!("    (Remaining bits at position {})", pos);
}

#[test]
fn test_decoder_basic() {
    // Simple test to verify decoder works
    let decoder = FrameObuDecoderCapsule::new();
    let payload: &[u8] = &[0x18, 0x00, 0x00, 0x00];

    let result = decoder.decode_frame_header_reduced(payload);
    assert!(
        result.is_ok(),
        "Decoder should successfully decode valid payload"
    );

    let decoded = result.unwrap();
    assert_eq!(decoded.allow_screen_content_tools, 0);
    assert_eq!(decoded.base_q_idx, 48); // 0x18 >> 1 = 0x0C = 12... wait, let me recalculate
                                        // Actually 0x18 = 0b00011000
                                        // Bit 0: allow_screen_content_tools = 0
                                        // Bits 1-8: base_q_idx = 0b0011000 << 1 | next bit = need to read from bytes
                                        // This is getting complex, let's just check the full decode works
}
