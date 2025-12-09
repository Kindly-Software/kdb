/// Verify the fixed sequence header decodes correctly
/// Load the av1_obu_decoder.rs implementation

include!("av1_obu_decoder.rs");

// Override main to test fixed bytes
#[allow(dead_code)]
fn main_original() {
    // Original implementation
}

fn main() {
    let obu_bytes: Vec<u8> = vec![
        0x0a, 0x0c, 0x00, 0x00, 0x00, 0x05, 0x57, 0xff, 0xc0, 0x02, 0x20, 0x20, 0x20, 0x20,
    ];

    println!("=== AV1 OBU DECODER (FIXED BYTES) ===\n");
    println!("Input bytes ({}): {:02x?}\n", obu_bytes.len(), obu_bytes);

    // Decode OBU header
    println!("=== BYTE 0: OBU HEADER ===\n");
    let header = decode_obu_header(obu_bytes[0]);
    print!("{}", header);

    if header.obu_forbidden_bit != 0 {
        println!("ERROR: obu_forbidden_bit must be 0!");
    }

    if header.obu_reserved_1bit != 0 {
        println!("WARNING: obu_reserved_1bit should be 0!");
    }

    // Decode size field
    println!("\n=== BYTE 1+: SIZE FIELD ===\n");
    if header.obu_has_size_field == 1 {
        match decode_leb128(&obu_bytes, 1) {
            Ok((size, bytes_read)) => {
                println!("LEB128 size: {} bytes", size);
                println!("LEB128 encoding: {} byte(s)", bytes_read);

                let payload_start = 1 + bytes_read;
                let expected_end = payload_start + size as usize;

                println!("Payload starts at byte: {}", payload_start);
                println!("Payload should end at byte: {}", expected_end);
                println!("Actual data length: {} bytes", obu_bytes.len());

                if expected_end > obu_bytes.len() {
                    println!(
                        "\nERROR: Size field indicates {} bytes but only {} bytes available!",
                        size,
                        obu_bytes.len() - payload_start
                    );
                    return;
                }

                if expected_end < obu_bytes.len() {
                    println!(
                        "\nWARNING: Extra {} bytes after payload",
                        obu_bytes.len() - expected_end
                    );
                }

                // Decode sequence header
                if header.obu_type == 1 {
                    let payload = &obu_bytes[payload_start..expected_end];
                    println!("\nPayload bytes ({} bytes): {:02x?}", payload.len(), payload);

                    match decode_sequence_header(payload) {
                        Ok(seq_header) => {
                            println!("\n=== SEQUENCE HEADER DECODED SUCCESSFULLY ===");
                            println!("{:#?}", seq_header);

                            println!("\n=== VERIFICATION CHECKS ===");
                            if seq_header.seq_level_idx[0] == 1 {
                                println!("✓ seq_level_idx = 1 (Level 2.1) - dav1d compatible!");
                            } else {
                                println!("✗ seq_level_idx = {} - NOT Level 2.1", seq_header.seq_level_idx[0]);
                            }

                            if seq_header.max_frame_width_minus_1 == 63 {
                                println!("✓ Width = 64 pixels");
                            } else {
                                println!("✗ Width = {} pixels (expected 64)", seq_header.max_frame_width_minus_1 + 1);
                            }

                            if seq_header.max_frame_height_minus_1 == 63 {
                                println!("✓ Height = 64 pixels");
                            } else {
                                println!("✗ Height = {} pixels (expected 64)", seq_header.max_frame_height_minus_1 + 1);
                            }

                            if seq_header.enable_cdef {
                                println!("✓ CDEF enabled");
                            } else {
                                println!("✗ CDEF disabled");
                            }
                        }
                        Err(e) => {
                            println!("\n=== DECODING ERROR ===");
                            println!("Error: {}", e);
                        }
                    }
                } else {
                    println!(
                        "\nOBU type {} ({}) payload parsing not implemented",
                        header.obu_type,
                        header.type_name()
                    );
                }
            }
            Err(e) => {
                println!("ERROR decoding LEB128 size: {}", e);
            }
        }
    } else {
        println!("No size field present (obu_has_size_field = 0)");
        println!("Size must be determined externally");
    }
}
