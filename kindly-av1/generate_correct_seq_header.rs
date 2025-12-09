/// Generate correct AV1 sequence header for 64x64 frame
/// Based on atomic_capsule sequence_header_impl.rs specification

struct BitWriter {
    bytes: Vec<u8>,
    current_byte: u8,
    bit_pos: u8, // 0-7, position within current byte (MSB first)
}

impl BitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            current_byte: 0,
            bit_pos: 0,
        }
    }

    fn write_bits(&mut self, num_bits: u8, value: u64) {
        let mut remaining_bits = num_bits;
        let mut remaining_value = value;

        while remaining_bits > 0 {
            let bits_available = 8 - self.bit_pos;
            let bits_to_write = remaining_bits.min(bits_available);

            // Extract the bits to write from the value
            let shift = remaining_bits - bits_to_write;
            let mask = (1u64 << bits_to_write) - 1;
            let bits = ((remaining_value >> shift) & mask) as u8;

            // Place bits in current byte
            let shift_in_byte = bits_available - bits_to_write;
            self.current_byte |= bits << shift_in_byte;

            self.bit_pos += bits_to_write;
            remaining_bits -= bits_to_write;
            remaining_value &= (1u64 << shift) - 1;

            // Flush byte if full
            if self.bit_pos == 8 {
                self.bytes.push(self.current_byte);
                self.current_byte = 0;
                self.bit_pos = 0;
            }
        }
    }

    fn flush(mut self) -> Vec<u8> {
        if self.bit_pos > 0 {
            self.bytes.push(self.current_byte);
        }
        self.bytes
    }
}

fn bits_needed(value: u32) -> u32 {
    if value == 0 {
        1
    } else {
        32 - value.leading_zeros()
    }
}

fn main() {
    let width = 64u16;
    let height = 64u16;

    let mut writer = BitWriter::new();

    // seq_profile (3 bits) = 0 (Main profile)
    writer.write_bits(3, 0);

    // still_picture (1 bit) = 0
    writer.write_bits(1, 0);

    // reduced_still_picture_header (1 bit) = 0
    writer.write_bits(1, 0);

    // timing_info_present_flag (1 bit) = 0
    writer.write_bits(1, 0);

    // decoder_model_info_present_flag (1 bit) = 0
    writer.write_bits(1, 0);

    // initial_display_delay_present_flag (1 bit) = 0
    writer.write_bits(1, 0);

    // operating_points_cnt_minus_1 (5 bits) = 0
    writer.write_bits(5, 0);

    // operating_point_idc (12 bits) = 0
    writer.write_bits(12, 0);

    // seq_level_idx (5 bits) = 1 (Level 2.1, MINIMUM for dav1d)
    // This is the critical fix!
    let seq_level_idx = 1u8; // Level 2.1 for 64x64 (4,096 pixels <= 278,784)
    writer.write_bits(5, seq_level_idx as u64);

    // frame_width_bits_minus_1 (4 bits)
    let max_width_minus_1 = (width - 1) as u32;
    let max_height_minus_1 = (height - 1) as u32;
    let width_bits = bits_needed(max_width_minus_1);
    let height_bits = bits_needed(max_height_minus_1);

    println!("max_width_minus_1: {} (0x{:x})", max_width_minus_1, max_width_minus_1);
    println!("max_height_minus_1: {} (0x{:x})", max_height_minus_1, max_height_minus_1);
    println!("width_bits: {} (need to encode {})", width_bits, max_width_minus_1);
    println!("height_bits: {} (need to encode {})", height_bits, max_height_minus_1);

    writer.write_bits(4, (width_bits - 1) as u64);

    // frame_height_bits_minus_1 (4 bits)
    writer.write_bits(4, (height_bits - 1) as u64);

    // max_frame_width_minus_1 (width_bits bits)
    writer.write_bits(width_bits as u8, max_width_minus_1 as u64);

    // max_frame_height_minus_1 (height_bits bits)
    writer.write_bits(height_bits as u8, max_height_minus_1 as u64);

    // frame_id_numbers_present_flag (1 bit) = 0
    writer.write_bits(1, 0);

    // use_128x128_superblock (1 bit) = 0
    writer.write_bits(1, 0);

    // enable_filter_intra (1 bit) = 0
    writer.write_bits(1, 0);

    // enable_intra_edge_filter (1 bit) = 0
    writer.write_bits(1, 0);

    // enable_interintra_compound (1 bit) = 0
    writer.write_bits(1, 0);

    // enable_masked_compound (1 bit) = 0
    writer.write_bits(1, 0);

    // enable_warped_motion (1 bit) = 0
    writer.write_bits(1, 0);

    // enable_dual_filter (1 bit) = 0
    writer.write_bits(1, 0);

    // enable_order_hint (1 bit) = 0
    writer.write_bits(1, 0);

    // seq_choose_screen_content_tools (1 bit) = 0
    writer.write_bits(1, 0);

    // seq_force_screen_content_tools (1 bit) = 0 (since choose = 0)
    writer.write_bits(1, 0);

    // enable_superres (1 bit) = 0
    writer.write_bits(1, 0);

    // enable_cdef (1 bit) = 1
    writer.write_bits(1, 1);

    // enable_restoration (1 bit) = 0
    writer.write_bits(1, 0);

    // Color config
    // high_bitdepth (1 bit) = 0
    writer.write_bits(1, 0);

    // mono_chrome (1 bit) = 0
    writer.write_bits(1, 0);

    // color_description_present_flag (1 bit) = 1
    writer.write_bits(1, 1);

    // color_primaries (8 bits) = 1 (BT.709)
    writer.write_bits(8, 1);

    // transfer_characteristics (8 bits) = 1 (BT.709)
    writer.write_bits(8, 1);

    // matrix_coefficients (8 bits) = 1 (BT.709)
    writer.write_bits(8, 1);

    // color_range (1 bit) = 0 (studio swing)
    writer.write_bits(1, 0);

    // chroma_sample_position (2 bits) = 0 (CSP_UNKNOWN)
    writer.write_bits(2, 0);

    // separate_uv_delta_q (1 bit) = 0
    writer.write_bits(1, 0);

    // film_grain_params_present (1 bit) = 0
    writer.write_bits(1, 0);

    let payload = writer.flush();

    println!("\n=== CORRECT SEQUENCE HEADER ===");
    println!("Payload length: {} bytes", payload.len());
    println!("Payload hex: {:02x?}", payload);

    // Create full OBU with header and size
    let mut obu = Vec::new();

    // OBU header byte
    let obu_header = 0x0a; // type=1 (sequence_header), has_size=1
    obu.push(obu_header);

    // LEB128 size
    let size = payload.len();
    obu.push(size as u8); // For sizes < 128, single byte

    // Payload
    obu.extend_from_slice(&payload);

    println!("\n=== COMPLETE OBU ===");
    println!("Total length: {} bytes", obu.len());
    println!("Full OBU hex: {:02x?}", obu);

    println!("\n=== COMPARISON ===");
    let old_bytes = vec![
        0x0a, 0x0d, 0x00, 0x00, 0x00, 0x01, 0x99, 0xfb, 0xf0, 0x00, 0x88, 0x08, 0x08, 0x08, 0x00, 0x1a,
    ];
    println!("OLD (buggy):  {:02x?}", old_bytes);
    println!("NEW (fixed):  {:02x?}", obu);

    println!("\n=== VERIFICATION ===");
    println!("Key differences:");
    println!("  - Byte 3: 0x{:02x} -> 0x{:02x} (seq_level_idx changed from 0 to 1)",
             old_bytes[3], obu[3]);
    if old_bytes.len() != obu.len() {
        println!("  - Length: {} -> {} bytes", old_bytes.len(), obu.len());
    }
}
