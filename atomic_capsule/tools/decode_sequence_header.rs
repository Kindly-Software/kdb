//! AV1 Sequence Header Decoder Tool
//!
//! Decodes sequence header payloads bit-by-bit to compare with reference.

struct BitReader {
    data: Vec<u8>,
    bit_pos: usize,
}

impl BitReader {
    fn new(data: Vec<u8>) -> Self {
        Self { data, bit_pos: 0 }
    }

    fn read_bits(&mut self, n: u8) -> u64 {
        let mut value = 0u64;
        for _ in 0..n {
            let byte_index = self.bit_pos / 8;
            let bit_index = 7 - (self.bit_pos % 8);

            if byte_index >= self.data.len() {
                break;
            }

            let bit = (self.data[byte_index] >> bit_index) & 1;
            value = (value << 1) | (bit as u64);
            self.bit_pos += 1;
        }
        value
    }

    fn pos(&self) -> usize {
        self.bit_pos
    }
}

fn decode_sequence_header(name: &str, payload: &[u8]) {
    println!("\n=== {} ===", name);
    println!("Hex: {}", payload.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" "));

    let mut reader = BitReader::new(payload.to_vec());

    // seq_profile (3 bits)
    let seq_profile = reader.read_bits(3);
    println!("Bit {:3}: seq_profile = {} ({})", reader.pos() - 3, seq_profile,
             match seq_profile {
                 0 => "Main",
                 1 => "High",
                 2 => "Professional",
                 _ => "Unknown",
             });

    // still_picture (1 bit)
    let still_picture = reader.read_bits(1);
    println!("Bit {:3}: still_picture = {} ({})", reader.pos() - 1, still_picture,
             if still_picture == 0 { "Video" } else { "Still" });

    // reduced_still_picture_header (1 bit)
    let reduced = reader.read_bits(1);
    println!("Bit {:3}: reduced_still_picture_header = {} ({})", reader.pos() - 1, reduced,
             if reduced == 0 { "Full header" } else { "Reduced" });

    if reduced == 0 {
        // timing_info_present_flag (1 bit)
        let timing = reader.read_bits(1);
        println!("Bit {:3}: timing_info_present_flag = {}", reader.pos() - 1, timing);

        // decoder_model_info_present_flag (1 bit)
        let decoder_model = reader.read_bits(1);
        println!("Bit {:3}: decoder_model_info_present_flag = {}", reader.pos() - 1, decoder_model);

        // initial_display_delay_present_flag (1 bit)
        let display_delay = reader.read_bits(1);
        println!("Bit {:3}: initial_display_delay_present_flag = {}", reader.pos() - 1, display_delay);

        // operating_points_cnt_minus_1 (5 bits)
        let op_points_cnt = reader.read_bits(5);
        println!("Bit {:3}: operating_points_cnt_minus_1 = {} ({} operating points)",
                 reader.pos() - 5, op_points_cnt, op_points_cnt + 1);

        for i in 0..=op_points_cnt {
            // operating_point_idc (12 bits)
            let op_idc = reader.read_bits(12);
            println!("Bit {:3}: operating_point_idc[{}] = 0x{:03x}", reader.pos() - 12, i, op_idc);

            // seq_level_idx (5 bits)
            let level_idx = reader.read_bits(5);
            let level_name = match level_idx {
                0 => "2.0",
                1 => "2.1",
                2 => "2.2",
                3 => "2.3",
                4 => "3.0",
                5 => "3.1",
                8 => "4.0",
                12 => "5.0",
                16 => "6.0",
                _ => "Unknown",
            };
            println!("Bit {:3}: seq_level_idx[{}] = {} (Level {})", reader.pos() - 5, i, level_idx, level_name);

            // seq_tier (1 bit) if level_idx > 7
            if level_idx > 7 {
                let tier = reader.read_bits(1);
                println!("Bit {:3}: seq_tier[{}] = {} ({})", reader.pos() - 1, i, tier,
                         if tier == 0 { "Main" } else { "High" });
            }
        }
    }

    // frame_width_bits_minus_1 (4 bits)
    let width_bits_minus_1 = reader.read_bits(4);
    let width_bits = width_bits_minus_1 + 1;
    println!("Bit {:3}: frame_width_bits_minus_1 = {} ({} bits for width)",
             reader.pos() - 4, width_bits_minus_1, width_bits);

    // frame_height_bits_minus_1 (4 bits)
    let height_bits_minus_1 = reader.read_bits(4);
    let height_bits = height_bits_minus_1 + 1;
    println!("Bit {:3}: frame_height_bits_minus_1 = {} ({} bits for height)",
             reader.pos() - 4, height_bits_minus_1, height_bits);

    // max_frame_width_minus_1 (width_bits bits)
    let max_width_minus_1 = reader.read_bits(width_bits as u8);
    let max_width = max_width_minus_1 + 1;
    println!("Bit {:3}: max_frame_width_minus_1 = {} (max_width = {})",
             reader.pos() - width_bits as usize, max_width_minus_1, max_width);

    // max_frame_height_minus_1 (height_bits bits)
    let max_height_minus_1 = reader.read_bits(height_bits as u8);
    let max_height = max_height_minus_1 + 1;
    println!("Bit {:3}: max_frame_height_minus_1 = {} (max_height = {})",
             reader.pos() - height_bits as usize, max_height_minus_1, max_height);

    // frame_id_numbers_present_flag (1 bit)
    let frame_id = reader.read_bits(1);
    println!("Bit {:3}: frame_id_numbers_present_flag = {}", reader.pos() - 1, frame_id);

    // Feature flags
    let use_128x128 = reader.read_bits(1);
    println!("Bit {:3}: use_128x128_superblock = {} ({})", reader.pos() - 1, use_128x128,
             if use_128x128 == 0 { "64x64" } else { "128x128" });

    let filter_intra = reader.read_bits(1);
    println!("Bit {:3}: enable_filter_intra = {}", reader.pos() - 1, filter_intra);

    let edge_filter = reader.read_bits(1);
    println!("Bit {:3}: enable_intra_edge_filter = {}", reader.pos() - 1, edge_filter);

    if reduced == 0 {
        let interintra = reader.read_bits(1);
        println!("Bit {:3}: enable_interintra_compound = {}", reader.pos() - 1, interintra);

        let masked = reader.read_bits(1);
        println!("Bit {:3}: enable_masked_compound = {}", reader.pos() - 1, masked);

        let warped = reader.read_bits(1);
        println!("Bit {:3}: enable_warped_motion = {}", reader.pos() - 1, warped);

        let dual_filter = reader.read_bits(1);
        println!("Bit {:3}: enable_dual_filter = {}", reader.pos() - 1, dual_filter);

        let order_hint = reader.read_bits(1);
        println!("Bit {:3}: enable_order_hint = {}", reader.pos() - 1, order_hint);

        if order_hint == 1 {
            let jnt_comp = reader.read_bits(1);
            println!("Bit {:3}: enable_jnt_comp = {}", reader.pos() - 1, jnt_comp);

            let ref_frame_mvs = reader.read_bits(1);
            println!("Bit {:3}: enable_ref_frame_mvs = {}", reader.pos() - 1, ref_frame_mvs);
        }

        let choose_screen = reader.read_bits(1);
        println!("Bit {:3}: seq_choose_screen_content_tools = {}", reader.pos() - 1, choose_screen);

        if choose_screen == 0 {
            let force_screen = reader.read_bits(2);
            println!("Bit {:3}: seq_force_screen_content_tools = {}", reader.pos() - 2, force_screen);

            if force_screen > 0 {
                let force_int_mv = reader.read_bits(1);
                println!("Bit {:3}: seq_force_integer_mv = {}", reader.pos() - 1, force_int_mv);
            }
        } else {
            let force_int_mv = reader.read_bits(2);
            println!("Bit {:3}: seq_force_integer_mv = {}", reader.pos() - 2, force_int_mv);
        }

        // Enable superres
        let superres = reader.read_bits(1);
        println!("Bit {:3}: enable_superres = {}", reader.pos() - 1, superres);

        // Enable CDEF
        let cdef = reader.read_bits(1);
        println!("Bit {:3}: enable_cdef = {}", reader.pos() - 1, cdef);

        // Enable restoration
        let restoration = reader.read_bits(1);
        println!("Bit {:3}: enable_restoration = {}", reader.pos() - 1, restoration);
    }

    // color_config()
    println!("\n--- color_config() ---");
    let high_bitdepth = reader.read_bits(1);
    println!("Bit {:3}: high_bitdepth = {} ({})", reader.pos() - 1, high_bitdepth,
             if high_bitdepth == 0 { "8-bit" } else { "10/12-bit" });

    if seq_profile == 2 && high_bitdepth == 1 {
        let twelve_bit = reader.read_bits(1);
        println!("Bit {:3}: twelve_bit = {}", reader.pos() - 1, twelve_bit);
    }

    let mono_chrome = reader.read_bits(1);
    println!("Bit {:3}: mono_chrome = {} ({})", reader.pos() - 1, mono_chrome,
             if mono_chrome == 0 { "Color" } else { "Monochrome" });

    let color_desc = reader.read_bits(1);
    println!("Bit {:3}: color_description_present_flag = {}", reader.pos() - 1, color_desc);

    if color_desc == 1 {
        let primaries = reader.read_bits(8);
        println!("Bit {:3}: color_primaries = {}", reader.pos() - 8, primaries);

        let transfer = reader.read_bits(8);
        println!("Bit {:3}: transfer_characteristics = {}", reader.pos() - 8, transfer);

        let matrix = reader.read_bits(8);
        println!("Bit {:3}: matrix_coefficients = {}", reader.pos() - 8, matrix);
    }

    if mono_chrome == 0 {
        let color_range = reader.read_bits(1);
        println!("Bit {:3}: color_range = {} ({})", reader.pos() - 1, color_range,
                 if color_range == 0 { "Studio" } else { "Full" });

        if seq_profile == 0 {
            println!("         (subsampling_x=1, subsampling_y=1 implicit for Main profile)");
        } else if seq_profile == 1 {
            println!("         (subsampling_x=0, subsampling_y=0 implicit for High profile)");
        } else {
            let subsamp_x = reader.read_bits(1);
            let subsamp_y = reader.read_bits(1);
            println!("Bit {:3}: subsampling_x = {}, subsampling_y = {}",
                     reader.pos() - 2, subsamp_x, subsamp_y);
        }

        // For 4:2:0 (subsamp_x=1, subsamp_y=1)
        if seq_profile == 0 || (seq_profile == 2 && high_bitdepth == 1) {
            let chroma_pos = reader.read_bits(2);
            println!("Bit {:3}: chroma_sample_position = {}", reader.pos() - 2, chroma_pos);
        }

        let sep_uv_delta = reader.read_bits(1);
        println!("Bit {:3}: separate_uv_delta_q = {}", reader.pos() - 1, sep_uv_delta);
    }

    // film_grain_params_present (1 bit)
    let film_grain = reader.read_bits(1);
    println!("\nBit {:3}: film_grain_params_present = {}", reader.pos() - 1, film_grain);

    println!("\nTotal bits consumed: {}", reader.pos());
    println!("Total bytes: {}", (reader.pos() + 7) / 8);
}

fn main() {
    let reference: Vec<u8> = vec![0x00, 0x00, 0x00, 0x02, 0xaf, 0xff, 0x9b, 0x5f, 0x20, 0x08];
    let our_output: Vec<u8> = vec![0x00, 0x00, 0x00, 0x05, 0x57, 0xff, 0xc0, 0x01, 0x00, 0x00];

    decode_sequence_header("REFERENCE (libaom, supposedly 64x64)", &reference);
    decode_sequence_header("OUR OUTPUT (64x64)", &our_output);
}
