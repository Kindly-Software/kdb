// AV1 OBU Byte-by-Byte Decoder
// Decodes OBU structure according to AV1 specification
// Identifies parsing failures in sequence headers

use std::fmt;

/// Bit reader for precise bit-level parsing
struct BitReader {
    data: Vec<u8>,
    byte_pos: usize,
    bit_pos: u8, // 0-7, tracks position within current byte
}

impl BitReader {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    /// Read n bits (up to 64)
    fn read_bits(&mut self, n: usize) -> Result<u64, String> {
        if n > 64 {
            return Err(format!("Cannot read {} bits (max 64)", n));
        }

        let mut result = 0u64;
        let mut bits_remaining = n;

        while bits_remaining > 0 {
            if self.byte_pos >= self.data.len() {
                return Err(format!(
                    "Unexpected end of data at byte {}, bit {}",
                    self.byte_pos, self.bit_pos
                ));
            }

            let bits_available = 8 - self.bit_pos;
            let bits_to_read = bits_remaining.min(bits_available as usize);

            // Extract bits from current byte
            let byte = self.data[self.byte_pos];
            let shift = bits_available - bits_to_read as u8;

            // Create mask safely
            let mask = if bits_to_read >= 8 {
                0xFF
            } else {
                ((1u8 << bits_to_read) - 1) << shift
            };
            let bits = ((byte & mask) >> shift) as u64;

            result = (result << bits_to_read) | bits;
            bits_remaining -= bits_to_read;
            self.bit_pos += bits_to_read as u8;

            if self.bit_pos == 8 {
                self.bit_pos = 0;
                self.byte_pos += 1;
            }
        }

        Ok(result)
    }

    fn position(&self) -> String {
        format!("byte {}, bit {}", self.byte_pos, self.bit_pos)
    }

    fn read_uvlc(&mut self) -> Result<u32, String> {
        let mut leading_zeros = 0;
        while self.read_bits(1)? == 0 {
            leading_zeros += 1;
            if leading_zeros > 32 {
                return Err("UVLC too large".to_string());
            }
        }

        if leading_zeros == 0 {
            return Ok(0);
        }

        let value = self.read_bits(leading_zeros)?;
        Ok((1u32 << leading_zeros) - 1 + value as u32)
    }
}

#[derive(Debug)]
struct ObuHeader {
    obu_forbidden_bit: u8,
    obu_type: u8,
    obu_extension_flag: u8,
    obu_has_size_field: u8,
    obu_reserved_1bit: u8,
}

impl ObuHeader {
    fn type_name(&self) -> &'static str {
        match self.obu_type {
            1 => "OBU_SEQUENCE_HEADER",
            2 => "OBU_TEMPORAL_DELIMITER",
            3 => "OBU_FRAME_HEADER",
            4 => "OBU_TILE_GROUP",
            5 => "OBU_METADATA",
            6 => "OBU_FRAME",
            7 => "OBU_REDUNDANT_FRAME_HEADER",
            8 => "OBU_TILE_LIST",
            15 => "OBU_PADDING",
            _ => "UNKNOWN",
        }
    }
}

impl fmt::Display for ObuHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "OBU Header (0x{:02x}):", self.obu_forbidden_bit << 7 | self.obu_type << 3 | self.obu_extension_flag << 2 | self.obu_has_size_field << 1 | self.obu_reserved_1bit)?;
        writeln!(f, "  obu_forbidden_bit: {}", self.obu_forbidden_bit)?;
        writeln!(f, "  obu_type: {} ({})", self.obu_type, self.type_name())?;
        writeln!(f, "  obu_extension_flag: {}", self.obu_extension_flag)?;
        writeln!(f, "  obu_has_size_field: {}", self.obu_has_size_field)?;
        writeln!(f, "  obu_reserved_1bit: {}", self.obu_reserved_1bit)?;
        Ok(())
    }
}

#[derive(Debug)]
struct SequenceHeader {
    seq_profile: u8,
    still_picture: bool,
    reduced_still_picture_header: bool,
    timing_info_present_flag: bool,
    decoder_model_info_present_flag: bool,
    initial_display_delay_present_flag: bool,
    operating_points_cnt_minus_1: u8,
    operating_point_idc: Vec<u16>,
    seq_level_idx: Vec<u8>,
    seq_tier: Vec<u8>,
    decoder_model_present_for_this_op: Vec<bool>,
    frame_width_bits_minus_1: u8,
    frame_height_bits_minus_1: u8,
    max_frame_width_minus_1: u32,
    max_frame_height_minus_1: u32,
    frame_id_numbers_present_flag: bool,
    use_128x128_superblock: bool,
    enable_filter_intra: bool,
    enable_intra_edge_filter: bool,
    enable_interintra_compound: bool,
    enable_masked_compound: bool,
    enable_warped_motion: bool,
    enable_dual_filter: bool,
    enable_order_hint: bool,
    order_hint_bits: Option<u8>,
    enable_jnt_comp: bool,
    enable_ref_frame_mvs: bool,
    seq_force_screen_content_tools: u8,
    seq_force_integer_mv: u8,
    enable_superres: bool,
    enable_cdef: bool,
    enable_restoration: bool,
    color_config_parsed: bool,
}

fn decode_leb128(data: &[u8], start: usize) -> Result<(u64, usize), String> {
    let mut value = 0u64;
    let mut bytes_read = 0;

    for i in 0..8 {
        if start + i >= data.len() {
            return Err(format!("Incomplete LEB128 at byte {}", start + i));
        }

        let byte = data[start + i];
        value |= ((byte & 0x7F) as u64) << (i * 7);
        bytes_read += 1;

        if (byte & 0x80) == 0 {
            return Ok((value, bytes_read));
        }
    }

    Err("LEB128 too large (>8 bytes)".to_string())
}

fn decode_obu_header(byte: u8) -> ObuHeader {
    ObuHeader {
        obu_forbidden_bit: (byte >> 7) & 1,
        obu_type: (byte >> 3) & 0xF,
        obu_extension_flag: (byte >> 2) & 1,
        obu_has_size_field: (byte >> 1) & 1,
        obu_reserved_1bit: byte & 1,
    }
}

fn decode_sequence_header(data: &[u8]) -> Result<SequenceHeader, String> {
    let mut reader = BitReader::new(data.to_vec());

    println!("\n=== SEQUENCE HEADER PAYLOAD DECODING ===\n");

    let seq_profile = reader.read_bits(3)? as u8;
    println!("seq_profile: {} (at {})", seq_profile, reader.position());
    if seq_profile > 2 {
        return Err(format!("Invalid seq_profile: {} (must be 0-2)", seq_profile));
    }

    let still_picture = reader.read_bits(1)? == 1;
    println!("still_picture: {} (at {})", still_picture, reader.position());

    let reduced_still_picture_header = reader.read_bits(1)? == 1;
    println!(
        "reduced_still_picture_header: {} (at {})",
        reduced_still_picture_header,
        reader.position()
    );

    let timing_info_present_flag;
    let decoder_model_info_present_flag;
    let initial_display_delay_present_flag;
    let operating_points_cnt_minus_1;

    if reduced_still_picture_header {
        timing_info_present_flag = false;
        decoder_model_info_present_flag = false;
        initial_display_delay_present_flag = false;
        operating_points_cnt_minus_1 = 0;
        println!("  [Reduced still picture mode - defaults applied]");
    } else {
        timing_info_present_flag = reader.read_bits(1)? == 1;
        println!(
            "timing_info_present_flag: {} (at {})",
            timing_info_present_flag,
            reader.position()
        );

        if timing_info_present_flag {
            return Err(
                "timing_info parsing not implemented (would need timing_info() function)".to_string(),
            );
        }

        decoder_model_info_present_flag = reader.read_bits(1)? == 1;
        println!(
            "decoder_model_info_present_flag: {} (at {})",
            decoder_model_info_present_flag,
            reader.position()
        );

        if decoder_model_info_present_flag {
            return Err(
                "decoder_model_info parsing not implemented".to_string(),
            );
        }

        initial_display_delay_present_flag = reader.read_bits(1)? == 1;
        println!(
            "initial_display_delay_present_flag: {} (at {})",
            initial_display_delay_present_flag,
            reader.position()
        );

        operating_points_cnt_minus_1 = reader.read_bits(5)? as u8;
        println!(
            "operating_points_cnt_minus_1: {} (at {})",
            operating_points_cnt_minus_1,
            reader.position()
        );
    }

    let mut operating_point_idc = Vec::new();
    let mut seq_level_idx = Vec::new();
    let mut seq_tier = Vec::new();
    let mut decoder_model_present_for_this_op = Vec::new();

    for i in 0..=operating_points_cnt_minus_1 {
        println!("\n  Operating Point {}:", i);

        let idc = reader.read_bits(12)? as u16;
        println!("    operating_point_idc: 0x{:03x} (at {})", idc, reader.position());
        operating_point_idc.push(idc);

        let level = reader.read_bits(5)? as u8;
        println!("    seq_level_idx: {} (at {})", level, reader.position());
        seq_level_idx.push(level);

        let tier = if level > 7 {
            let t = reader.read_bits(1)? as u8;
            println!("    seq_tier: {} (at {})", t, reader.position());
            t
        } else {
            0
        };
        seq_tier.push(tier);

        let decoder_present = if decoder_model_info_present_flag {
            let dp = reader.read_bits(1)? == 1;
            println!("    decoder_model_present_for_this_op: {} (at {})", dp, reader.position());
            dp
        } else {
            false
        };
        decoder_model_present_for_this_op.push(decoder_present);

        if decoder_present {
            return Err("operating_parameters_info parsing not implemented".to_string());
        }

        if initial_display_delay_present_flag {
            let present = reader.read_bits(1)? == 1;
            println!("    initial_display_delay_present_for_this_op: {} (at {})", present, reader.position());
            if present {
                let delay = reader.read_bits(4)?;
                println!("    initial_display_delay_minus_1: {} (at {})", delay, reader.position());
            }
        }
    }

    println!("\n=== FRAME SIZE PARAMETERS ===\n");

    let frame_width_bits_minus_1 = reader.read_bits(4)? as u8;
    println!(
        "frame_width_bits_minus_1: {} (at {})",
        frame_width_bits_minus_1,
        reader.position()
    );

    let frame_height_bits_minus_1 = reader.read_bits(4)? as u8;
    println!(
        "frame_height_bits_minus_1: {} (at {})",
        frame_height_bits_minus_1,
        reader.position()
    );

    let width_bits = (frame_width_bits_minus_1 + 1) as usize;
    let height_bits = (frame_height_bits_minus_1 + 1) as usize;

    let max_frame_width_minus_1 = reader.read_bits(width_bits)? as u32;
    println!(
        "max_frame_width_minus_1: {} (reading {} bits, at {})",
        max_frame_width_minus_1,
        width_bits,
        reader.position()
    );
    let actual_width = max_frame_width_minus_1 + 1;
    println!("  -> Actual width: {}", actual_width);

    let max_frame_height_minus_1 = reader.read_bits(height_bits)? as u32;
    println!(
        "max_frame_height_minus_1: {} (reading {} bits, at {})",
        max_frame_height_minus_1,
        height_bits,
        reader.position()
    );
    let actual_height = max_frame_height_minus_1 + 1;
    println!("  -> Actual height: {}", actual_height);

    let frame_id_numbers_present_flag = if reduced_still_picture_header {
        false
    } else {
        let flag = reader.read_bits(1)? == 1;
        println!(
            "frame_id_numbers_present_flag: {} (at {})",
            flag,
            reader.position()
        );
        if flag {
            let delta_frame_id_length = reader.read_bits(4)?;
            let additional_frame_id_length = reader.read_bits(3)?;
            println!("  delta_frame_id_length_minus_2: {} (at {})", delta_frame_id_length, reader.position());
            println!("  additional_frame_id_length_minus_1: {} (at {})", additional_frame_id_length, reader.position());
        }
        flag
    };

    println!("\n=== FEATURE FLAGS ===\n");

    let use_128x128_superblock = reader.read_bits(1)? == 1;
    println!(
        "use_128x128_superblock: {} (at {})",
        use_128x128_superblock,
        reader.position()
    );

    let enable_filter_intra = reader.read_bits(1)? == 1;
    println!(
        "enable_filter_intra: {} (at {})",
        enable_filter_intra,
        reader.position()
    );

    let enable_intra_edge_filter = reader.read_bits(1)? == 1;
    println!(
        "enable_intra_edge_filter: {} (at {})",
        enable_intra_edge_filter,
        reader.position()
    );

    let enable_interintra_compound;
    let enable_masked_compound;
    let enable_warped_motion;
    let enable_dual_filter;
    let enable_order_hint;
    let order_hint_bits;
    let enable_jnt_comp;
    let enable_ref_frame_mvs;

    if reduced_still_picture_header {
        enable_interintra_compound = false;
        enable_masked_compound = false;
        enable_warped_motion = false;
        enable_dual_filter = false;
        enable_order_hint = false;
        order_hint_bits = None;
        enable_jnt_comp = false;
        enable_ref_frame_mvs = false;
        println!("  [Reduced still picture mode - inter-frame features disabled]");
    } else {
        enable_interintra_compound = reader.read_bits(1)? == 1;
        println!(
            "enable_interintra_compound: {} (at {})",
            enable_interintra_compound,
            reader.position()
        );

        enable_masked_compound = reader.read_bits(1)? == 1;
        println!(
            "enable_masked_compound: {} (at {})",
            enable_masked_compound,
            reader.position()
        );

        enable_warped_motion = reader.read_bits(1)? == 1;
        println!(
            "enable_warped_motion: {} (at {})",
            enable_warped_motion,
            reader.position()
        );

        enable_dual_filter = reader.read_bits(1)? == 1;
        println!(
            "enable_dual_filter: {} (at {})",
            enable_dual_filter,
            reader.position()
        );

        enable_order_hint = reader.read_bits(1)? == 1;
        println!(
            "enable_order_hint: {} (at {})",
            enable_order_hint,
            reader.position()
        );

        order_hint_bits = if enable_order_hint {
            let bits = reader.read_bits(1)? == 1;
            println!("  enable_jnt_comp: {} (at {})", bits, reader.position());
            let ref_mvs = reader.read_bits(1)? == 1;
            println!("  enable_ref_frame_mvs: {} (at {})", ref_mvs, reader.position());

            let order_bits = reader.read_bits(3)? as u8 + 1;
            println!("  order_hint_bits_minus_1: {} (at {})", order_bits - 1, reader.position());
            println!("    -> order_hint_bits: {}", order_bits);
            Some(order_bits)
        } else {
            None
        };

        enable_jnt_comp = enable_order_hint && order_hint_bits.is_some();
        enable_ref_frame_mvs = enable_order_hint && order_hint_bits.is_some();
    }

    let seq_force_screen_content_tools = reader.read_bits(1)? as u8;
    println!(
        "seq_choose_screen_content_tools: {} (at {})",
        seq_force_screen_content_tools,
        reader.position()
    );

    let seq_force_integer_mv = if seq_force_screen_content_tools == 2 {
        2
    } else {
        let val = reader.read_bits(1)? as u8;
        println!(
            "seq_force_integer_mv: {} (at {})",
            val,
            reader.position()
        );
        val
    };

    let enable_superres = reader.read_bits(1)? == 1;
    println!(
        "enable_superres: {} (at {})",
        enable_superres,
        reader.position()
    );

    let enable_cdef = reader.read_bits(1)? == 1;
    println!(
        "enable_cdef: {} (at {})",
        enable_cdef,
        reader.position()
    );

    let enable_restoration = reader.read_bits(1)? == 1;
    println!(
        "enable_restoration: {} (at {})",
        enable_restoration,
        reader.position()
    );

    println!("\n=== COLOR CONFIG (would parse here) ===");
    println!("Remaining bytes: {} bytes", reader.data.len() - reader.byte_pos);

    Ok(SequenceHeader {
        seq_profile,
        still_picture,
        reduced_still_picture_header,
        timing_info_present_flag,
        decoder_model_info_present_flag,
        initial_display_delay_present_flag,
        operating_points_cnt_minus_1,
        operating_point_idc,
        seq_level_idx,
        seq_tier,
        decoder_model_present_for_this_op,
        frame_width_bits_minus_1,
        frame_height_bits_minus_1,
        max_frame_width_minus_1,
        max_frame_height_minus_1,
        frame_id_numbers_present_flag,
        use_128x128_superblock,
        enable_filter_intra,
        enable_intra_edge_filter,
        enable_interintra_compound,
        enable_masked_compound,
        enable_warped_motion,
        enable_dual_filter,
        enable_order_hint,
        order_hint_bits,
        enable_jnt_comp,
        enable_ref_frame_mvs,
        seq_force_screen_content_tools,
        seq_force_integer_mv,
        enable_superres,
        enable_cdef,
        enable_restoration,
        color_config_parsed: false,
    })
}

fn main() {
    let obu_bytes: Vec<u8> = vec![
        0x0a, 0x0d, 0x00, 0x00, 0x00, 0x01, 0x99, 0xfb, 0xf0, 0x00, 0x88, 0x08, 0x08, 0x08, 0x00,
        0x1a,
    ];

    println!("=== AV1 OBU DECODER ===\n");
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
                    let payload = &obu_bytes[payload_start..];
                    println!("\nPayload bytes ({} bytes): {:02x?}", payload.len(), payload);

                    match decode_sequence_header(payload) {
                        Ok(seq_header) => {
                            println!("\n=== SEQUENCE HEADER DECODED SUCCESSFULLY ===");
                            println!("{:#?}", seq_header);
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
