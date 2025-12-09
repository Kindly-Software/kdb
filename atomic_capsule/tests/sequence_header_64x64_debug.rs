// Test to debug 64×64 sequence header encoding
// This test prints hex output for comparison with reference

use atomic_capsule::encoder::ObuBitstreamWriterCapsule;

#[test]
fn test_sequence_header_64x64_detailed() {
    let writer = ObuBitstreamWriterCapsule::new();
    let obu = writer.write_sequence_header(64, 64);

    println!("\n=== Our Encoder Output (64×64) ===");
    println!("Total size: {} bytes\n", obu.len());

    // Print first 10 bytes in hex
    print!("First 10 bytes: ");
    for i in 0..10.min(obu.len()) {
        print!("{:02x} ", obu[i]);
    }
    println!("\n");

    // Print first 10 bytes in binary
    println!("Binary representation:");
    for i in 0..10.min(obu.len()) {
        println!("Byte {}: 0x{:02x} = {:08b}", i, obu[i], obu[i]);
    }
    println!();

    // Decode fields manually
    println!("=== Field Decoding ===");

    // Skip OBU header and LEB128 size
    let header = obu[0];
    println!("OBU header: 0x{:02x}", header);
    println!("  obu_type: {}", (header >> 3) & 0xF);
    println!("  obu_has_size_field: {}", (header >> 2) & 1);

    // Find payload start (after header + LEB128 size)
    let size_byte = obu[1];
    println!("\nLEB128 size byte: 0x{:02x} = {} bytes", size_byte, size_byte & 0x7F);

    let payload_start = if (size_byte & 0x80) == 0 {
        2 // Single byte size
    } else {
        3 // Two byte size (unlikely for sequence header)
    };

    println!("\nPayload starts at byte {}", payload_start);
    println!("Payload bytes:");
    for i in payload_start..10.min(obu.len()) {
        print!("{:02x} ", obu[i]);
    }
    println!("\n");

    // Decode payload bit-by-bit
    let payload = &obu[payload_start..];
    let mut bit_pos = 0;

    // Helper to read n bits from payload
    let read_bits = |start_bit: usize, n: usize| -> u64 {
        let mut value = 0u64;
        for i in 0..n {
            let byte_idx = (start_bit + i) / 8;
            let bit_idx = 7 - ((start_bit + i) % 8); // MSB first
            if byte_idx < payload.len() {
                let bit = (payload[byte_idx] >> bit_idx) & 1;
                value = (value << 1) | (bit as u64);
            }
        }
        value
    };

    println!("=== Sequence Header Fields ===");
    println!("seq_profile (3 bits): {}", read_bits(bit_pos, 3)); bit_pos += 3;
    println!("still_picture (1 bit): {}", read_bits(bit_pos, 1)); bit_pos += 1;
    println!("reduced_still_picture_header (1 bit): {}", read_bits(bit_pos, 1)); bit_pos += 1;
    println!("timing_info_present_flag (1 bit): {}", read_bits(bit_pos, 1)); bit_pos += 1;
    println!("decoder_model_info_present_flag (1 bit): {}", read_bits(bit_pos, 1)); bit_pos += 1;
    println!("initial_display_delay_present_flag (1 bit): {}", read_bits(bit_pos, 1)); bit_pos += 1;
    println!("operating_points_cnt_minus_1 (5 bits): {}", read_bits(bit_pos, 5)); bit_pos += 5;
    println!("operating_point_idc[0] (12 bits): {}", read_bits(bit_pos, 12)); bit_pos += 12;
    let level_idx = read_bits(bit_pos, 5); bit_pos += 5;
    println!("seq_level_idx[0] (5 bits): {}", level_idx);

    if level_idx > 7 {
        println!("seq_tier[0] (1 bit): {}", read_bits(bit_pos, 1)); bit_pos += 1;
    }

    let width_bits_minus_1 = read_bits(bit_pos, 4) as u8; bit_pos += 4;
    println!("frame_width_bits_minus_1 (4 bits): {} (means {} bits for width)",
             width_bits_minus_1, width_bits_minus_1 + 1);

    let height_bits_minus_1 = read_bits(bit_pos, 4) as u8; bit_pos += 4;
    println!("frame_height_bits_minus_1 (4 bits): {} (means {} bits for height)",
             height_bits_minus_1, height_bits_minus_1 + 1);

    let width_bits = (width_bits_minus_1 + 1) as usize;
    let height_bits = (height_bits_minus_1 + 1) as usize;

    let max_width_minus_1 = read_bits(bit_pos, width_bits); bit_pos += width_bits;
    println!("max_frame_width_minus_1 ({} bits): {} (width = {})",
             width_bits, max_width_minus_1, max_width_minus_1 + 1);

    let max_height_minus_1 = read_bits(bit_pos, height_bits); bit_pos += height_bits;
    println!("max_frame_height_minus_1 ({} bits): {} (height = {})",
             height_bits, max_height_minus_1, max_height_minus_1 + 1);

    println!("\n=== ANALYSIS ===");
    println!("Expected dimensions: 64×64");
    println!("Actual encoded dimensions: {}×{}",
             max_width_minus_1 + 1, max_height_minus_1 + 1);

    if max_width_minus_1 + 1 == 64 && max_height_minus_1 + 1 == 64 {
        println!("✅ Dimensions are correct!");
    } else {
        println!("❌ Dimensions are WRONG!");
        println!("   Width diff: expected 64, got {}", max_width_minus_1 + 1);
        println!("   Height diff: expected 64, got {}", max_height_minus_1 + 1);
    }
}

#[test]
fn test_compare_with_reference() {
    let reference = [0x00u8, 0x00, 0x00, 0x02, 0xaf, 0xff, 0x9b, 0x5f, 0x20, 0x08];
    let writer = ObuBitstreamWriterCapsule::new();
    let our_output = writer.write_sequence_header(64, 64);

    println!("\n=== Comparison ===");
    println!("Reference (10 bytes): {:02x?}", &reference[..]);
    println!("Our output (first 10): {:02x?}", &our_output[..10.min(our_output.len())]);

    for i in 0..10.min(our_output.len()) {
        if reference[i] == our_output[i] {
            println!("Byte {}: ✅ match (0x{:02x})", i, reference[i]);
        } else {
            println!("Byte {}: ❌ differ (reference: 0x{:02x}, ours: 0x{:02x})",
                     i, reference[i], our_output[i]);
        }
    }
}
