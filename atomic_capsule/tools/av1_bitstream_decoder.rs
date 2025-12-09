//! AV1 Sequence Header Bitstream Decoder
//!
//! Decodes AV1 sequence header payloads bit-by-bit according to spec §5.5
//! to identify divergence between our encoder and libaom reference.

use std::fmt;

struct BitReader {
    bytes: Vec<u8>,
    byte_pos: usize,
    bit_pos: u8, // 0-7, tracks position within current byte
}

impl BitReader {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    /// Read n bits from the stream
    fn read_bits(&mut self, n: u8) -> Option<u64> {
        if n == 0 || n > 64 {
            return None;
        }

        let mut result = 0u64;
        let mut bits_remaining = n;

        while bits_remaining > 0 {
            if self.byte_pos >= self.bytes.len() {
                return None; // Out of data
            }

            let bits_in_current_byte = 8 - self.bit_pos;
            let bits_to_read = bits_remaining.min(bits_in_current_byte);

            // Extract bits from current byte
            let byte = self.bytes[self.byte_pos];
            let shift = bits_in_current_byte.saturating_sub(bits_to_read);

            // Use safe shifting with checked operations
            let mask = if bits_to_read < 8 {
                ((1u8 << bits_to_read) - 1) << shift
            } else {
                0xFF
            };
            let bits = ((byte & mask) >> shift) as u64;

            result = (result << bits_to_read) | bits;

            self.bit_pos += bits_to_read;
            bits_remaining -= bits_to_read;

            if self.bit_pos >= 8 {
                self.byte_pos += 1;
                self.bit_pos = 0;
            }
        }

        Some(result)
    }

    fn bit_position(&self) -> (usize, u8) {
        (self.byte_pos, self.bit_pos)
    }
}

#[derive(Debug)]
struct SequenceHeader {
    // Basic fields
    seq_profile: u8,
    still_picture: bool,
    reduced_still_picture_header: bool,
    timing_info_present_flag: bool,
    decoder_model_info_present_flag: bool,
    initial_display_delay_present_flag: bool,
    operating_points_cnt_minus_1: u8,
    operating_point_idc: Vec<u16>,
    seq_level_idx: Vec<u8>,
    seq_tier: Vec<bool>,

    // Frame size
    frame_width_bits_minus_1: u8,
    frame_height_bits_minus_1: u8,
    max_frame_width_minus_1: u32,
    max_frame_height_minus_1: u32,

    // Feature flags
    frame_id_numbers_present_flag: bool,
    use_128x128_superblock: bool,
    enable_filter_intra: bool,
    enable_intra_edge_filter: bool,
    enable_interintra_compound: bool,
    enable_masked_compound: bool,
    enable_warped_motion: bool,
    enable_dual_filter: bool,
    enable_order_hint: bool,

    // Screen content and superres
    seq_choose_screen_content_tools: bool,
    seq_force_screen_content_tools: Option<u8>,
    seq_force_integer_mv: Option<u8>,
    enable_superres: bool,
    enable_cdef: bool,
    enable_restoration: bool,

    // Color config
    high_bitdepth: bool,
    mono_chrome: bool,
    color_description_present_flag: bool,
    color_range: Option<bool>,
    chroma_sample_position: Option<u8>,
    separate_uv_delta_q: Option<bool>,

    film_grain_params_present: bool,
}

fn decode_sequence_header(reader: &mut BitReader) -> Result<SequenceHeader, String> {
    let mut header = SequenceHeader {
        seq_profile: 0,
        still_picture: false,
        reduced_still_picture_header: false,
        timing_info_present_flag: false,
        decoder_model_info_present_flag: false,
        initial_display_delay_present_flag: false,
        operating_points_cnt_minus_1: 0,
        operating_point_idc: Vec::new(),
        seq_level_idx: Vec::new(),
        seq_tier: Vec::new(),
        frame_width_bits_minus_1: 0,
        frame_height_bits_minus_1: 0,
        max_frame_width_minus_1: 0,
        max_frame_height_minus_1: 0,
        frame_id_numbers_present_flag: false,
        use_128x128_superblock: false,
        enable_filter_intra: false,
        enable_intra_edge_filter: false,
        enable_interintra_compound: false,
        enable_masked_compound: false,
        enable_warped_motion: false,
        enable_dual_filter: false,
        enable_order_hint: false,
        seq_choose_screen_content_tools: false,
        seq_force_screen_content_tools: None,
        seq_force_integer_mv: None,
        enable_superres: false,
        enable_cdef: false,
        enable_restoration: false,
        high_bitdepth: false,
        mono_chrome: false,
        color_description_present_flag: false,
        color_range: None,
        chroma_sample_position: None,
        separate_uv_delta_q: None,
        film_grain_params_present: false,
    };

    println!("=== Decoding AV1 Sequence Header ===\n");

    // seq_profile (3 bits)
    let (byte, bit) = reader.bit_position();
    header.seq_profile = reader.read_bits(3).ok_or("Failed to read seq_profile")? as u8;
    println!("Byte {}.{}: seq_profile = {} (3 bits)", byte, bit, header.seq_profile);

    // still_picture (1 bit)
    let (byte, bit) = reader.bit_position();
    header.still_picture = reader.read_bits(1).ok_or("Failed to read still_picture")? == 1;
    println!("Byte {}.{}: still_picture = {} (1 bit)", byte, bit, header.still_picture);

    // reduced_still_picture_header (1 bit)
    let (byte, bit) = reader.bit_position();
    header.reduced_still_picture_header = reader.read_bits(1).ok_or("Failed to read reduced_still_picture_header")? == 1;
    println!("Byte {}.{}: reduced_still_picture_header = {} (1 bit)", byte, bit, header.reduced_still_picture_header);

    // timing_info_present_flag (1 bit)
    let (byte, bit) = reader.bit_position();
    header.timing_info_present_flag = reader.read_bits(1).ok_or("Failed to read timing_info_present_flag")? == 1;
    println!("Byte {}.{}: timing_info_present_flag = {} (1 bit)", byte, bit, header.timing_info_present_flag);

    // decoder_model_info_present_flag (1 bit)
    let (byte, bit) = reader.bit_position();
    header.decoder_model_info_present_flag = reader.read_bits(1).ok_or("Failed to read decoder_model_info_present_flag")? == 1;
    println!("Byte {}.{}: decoder_model_info_present_flag = {} (1 bit)", byte, bit, header.decoder_model_info_present_flag);

    // initial_display_delay_present_flag (1 bit)
    let (byte, bit) = reader.bit_position();
    header.initial_display_delay_present_flag = reader.read_bits(1).ok_or("Failed to read initial_display_delay_present_flag")? == 1;
    println!("Byte {}.{}: initial_display_delay_present_flag = {} (1 bit)", byte, bit, header.initial_display_delay_present_flag);

    // operating_points_cnt_minus_1 (5 bits)
    let (byte, bit) = reader.bit_position();
    header.operating_points_cnt_minus_1 = reader.read_bits(5).ok_or("Failed to read operating_points_cnt_minus_1")? as u8;
    println!("Byte {}.{}: operating_points_cnt_minus_1 = {} (5 bits)", byte, bit, header.operating_points_cnt_minus_1);

    // Operating points
    for i in 0..=(header.operating_points_cnt_minus_1 as usize) {
        // operating_point_idc[i] (12 bits)
        let (byte, bit) = reader.bit_position();
        let idc = reader.read_bits(12).ok_or("Failed to read operating_point_idc")? as u16;
        header.operating_point_idc.push(idc);
        println!("Byte {}.{}: operating_point_idc[{}] = {} (12 bits)", byte, bit, i, idc);

        // seq_level_idx[i] (5 bits)
        let (byte, bit) = reader.bit_position();
        let level = reader.read_bits(5).ok_or("Failed to read seq_level_idx")? as u8;
        header.seq_level_idx.push(level);
        println!("Byte {}.{}: seq_level_idx[{}] = {} (5 bits)", byte, bit, i, level);

        // seq_tier[i] (1 bit, only if level > 7)
        if level > 7 {
            let (byte, bit) = reader.bit_position();
            let tier = reader.read_bits(1).ok_or("Failed to read seq_tier")? == 1;
            header.seq_tier.push(tier);
            println!("Byte {}.{}: seq_tier[{}] = {} (1 bit)", byte, bit, i, tier);
        }
    }

    // frame_width_bits_minus_1 (4 bits)
    let (byte, bit) = reader.bit_position();
    header.frame_width_bits_minus_1 = reader.read_bits(4).ok_or("Failed to read frame_width_bits_minus_1")? as u8;
    println!("Byte {}.{}: frame_width_bits_minus_1 = {} (4 bits)", byte, bit, header.frame_width_bits_minus_1);

    // frame_height_bits_minus_1 (4 bits)
    let (byte, bit) = reader.bit_position();
    header.frame_height_bits_minus_1 = reader.read_bits(4).ok_or("Failed to read frame_height_bits_minus_1")? as u8;
    println!("Byte {}.{}: frame_height_bits_minus_1 = {} (4 bits)", byte, bit, header.frame_height_bits_minus_1);

    // max_frame_width_minus_1 (frame_width_bits_minus_1 + 1 bits)
    let width_bits = header.frame_width_bits_minus_1 + 1;
    let (byte, bit) = reader.bit_position();
    header.max_frame_width_minus_1 = reader.read_bits(width_bits).ok_or("Failed to read max_frame_width_minus_1")? as u32;
    println!("Byte {}.{}: max_frame_width_minus_1 = {} (actual width = {}) ({} bits)",
             byte, bit, header.max_frame_width_minus_1, header.max_frame_width_minus_1 + 1, width_bits);

    // max_frame_height_minus_1 (frame_height_bits_minus_1 + 1 bits)
    let height_bits = header.frame_height_bits_minus_1 + 1;
    let (byte, bit) = reader.bit_position();
    header.max_frame_height_minus_1 = reader.read_bits(height_bits).ok_or("Failed to read max_frame_height_minus_1")? as u32;
    println!("Byte {}.{}: max_frame_height_minus_1 = {} (actual height = {}) ({} bits)",
             byte, bit, header.max_frame_height_minus_1, header.max_frame_height_minus_1 + 1, height_bits);

    // frame_id_numbers_present_flag (1 bit)
    let (byte, bit) = reader.bit_position();
    header.frame_id_numbers_present_flag = reader.read_bits(1).ok_or("Failed to read frame_id_numbers_present_flag")? == 1;
    println!("Byte {}.{}: frame_id_numbers_present_flag = {} (1 bit)", byte, bit, header.frame_id_numbers_present_flag);

    // use_128x128_superblock (1 bit)
    let (byte, bit) = reader.bit_position();
    header.use_128x128_superblock = reader.read_bits(1).ok_or("Failed to read use_128x128_superblock")? == 1;
    println!("Byte {}.{}: use_128x128_superblock = {} (1 bit)", byte, bit, header.use_128x128_superblock);

    // enable_filter_intra (1 bit)
    let (byte, bit) = reader.bit_position();
    header.enable_filter_intra = reader.read_bits(1).ok_or("Failed to read enable_filter_intra")? == 1;
    println!("Byte {}.{}: enable_filter_intra = {} (1 bit)", byte, bit, header.enable_filter_intra);

    // enable_intra_edge_filter (1 bit)
    let (byte, bit) = reader.bit_position();
    header.enable_intra_edge_filter = reader.read_bits(1).ok_or("Failed to read enable_intra_edge_filter")? == 1;
    println!("Byte {}.{}: enable_intra_edge_filter = {} (1 bit)", byte, bit, header.enable_intra_edge_filter);

    // Since reduced_still_picture_header = 0, read inter-frame features
    if !header.reduced_still_picture_header {
        // enable_interintra_compound (1 bit)
        let (byte, bit) = reader.bit_position();
        header.enable_interintra_compound = reader.read_bits(1).ok_or("Failed to read enable_interintra_compound")? == 1;
        println!("Byte {}.{}: enable_interintra_compound = {} (1 bit)", byte, bit, header.enable_interintra_compound);

        // enable_masked_compound (1 bit)
        let (byte, bit) = reader.bit_position();
        header.enable_masked_compound = reader.read_bits(1).ok_or("Failed to read enable_masked_compound")? == 1;
        println!("Byte {}.{}: enable_masked_compound = {} (1 bit)", byte, bit, header.enable_masked_compound);

        // enable_warped_motion (1 bit)
        let (byte, bit) = reader.bit_position();
        header.enable_warped_motion = reader.read_bits(1).ok_or("Failed to read enable_warped_motion")? == 1;
        println!("Byte {}.{}: enable_warped_motion = {} (1 bit)", byte, bit, header.enable_warped_motion);

        // enable_dual_filter (1 bit)
        let (byte, bit) = reader.bit_position();
        header.enable_dual_filter = reader.read_bits(1).ok_or("Failed to read enable_dual_filter")? == 1;
        println!("Byte {}.{}: enable_dual_filter = {} (1 bit)", byte, bit, header.enable_dual_filter);

        // enable_order_hint (1 bit)
        let (byte, bit) = reader.bit_position();
        header.enable_order_hint = reader.read_bits(1).ok_or("Failed to read enable_order_hint")? == 1;
        println!("Byte {}.{}: enable_order_hint = {} (1 bit)", byte, bit, header.enable_order_hint);
    }

    // seq_choose_screen_content_tools (1 bit)
    let (byte, bit) = reader.bit_position();
    header.seq_choose_screen_content_tools = reader.read_bits(1).ok_or("Failed to read seq_choose_screen_content_tools")? == 1;
    println!("Byte {}.{}: seq_choose_screen_content_tools = {} (1 bit)", byte, bit, header.seq_choose_screen_content_tools);

    if !header.seq_choose_screen_content_tools {
        // seq_force_screen_content_tools (2 bits)
        let (byte, bit) = reader.bit_position();
        let tools = reader.read_bits(2).ok_or("Failed to read seq_force_screen_content_tools")? as u8;
        header.seq_force_screen_content_tools = Some(tools);
        println!("Byte {}.{}: seq_force_screen_content_tools = {} (2 bits)", byte, bit, tools);
    }

    // enable_superres (1 bit)
    let (byte, bit) = reader.bit_position();
    header.enable_superres = reader.read_bits(1).ok_or("Failed to read enable_superres")? == 1;
    println!("Byte {}.{}: enable_superres = {} (1 bit)", byte, bit, header.enable_superres);

    // enable_cdef (1 bit)
    let (byte, bit) = reader.bit_position();
    header.enable_cdef = reader.read_bits(1).ok_or("Failed to read enable_cdef")? == 1;
    println!("Byte {}.{}: enable_cdef = {} (1 bit)", byte, bit, header.enable_cdef);

    // enable_restoration (1 bit)
    let (byte, bit) = reader.bit_position();
    header.enable_restoration = reader.read_bits(1).ok_or("Failed to read enable_restoration")? == 1;
    println!("Byte {}.{}: enable_restoration = {} (1 bit)", byte, bit, header.enable_restoration);

    println!("\n--- Color Config ---");

    // color_config()
    // high_bitdepth (1 bit)
    let (byte, bit) = reader.bit_position();
    header.high_bitdepth = reader.read_bits(1).ok_or("Failed to read high_bitdepth")? == 1;
    println!("Byte {}.{}: high_bitdepth = {} (1 bit)", byte, bit, header.high_bitdepth);

    // mono_chrome (1 bit) - for seq_profile != 1
    let (byte, bit) = reader.bit_position();
    header.mono_chrome = reader.read_bits(1).ok_or("Failed to read mono_chrome")? == 1;
    println!("Byte {}.{}: mono_chrome = {} (1 bit)", byte, bit, header.mono_chrome);

    // color_description_present_flag (1 bit)
    let (byte, bit) = reader.bit_position();
    header.color_description_present_flag = reader.read_bits(1).ok_or("Failed to read color_description_present_flag")? == 1;
    println!("Byte {}.{}: color_description_present_flag = {} (1 bit)", byte, bit, header.color_description_present_flag);

    if !header.mono_chrome {
        // color_range (1 bit)
        let (byte, bit) = reader.bit_position();
        let range = reader.read_bits(1).ok_or("Failed to read color_range")? == 1;
        header.color_range = Some(range);
        println!("Byte {}.{}: color_range = {} (1 bit)", byte, bit, range);

        // For Main profile (0), subsampling is fixed to 4:2:0
        if header.seq_profile == 0 {
            // chroma_sample_position (2 bits)
            let (byte, bit) = reader.bit_position();
            let pos = reader.read_bits(2).ok_or("Failed to read chroma_sample_position")? as u8;
            header.chroma_sample_position = Some(pos);
            println!("Byte {}.{}: chroma_sample_position = {} (2 bits)", byte, bit, pos);
        }

        // separate_uv_delta_q (1 bit)
        let (byte, bit) = reader.bit_position();
        let sep = reader.read_bits(1).ok_or("Failed to read separate_uv_delta_q")? == 1;
        header.separate_uv_delta_q = Some(sep);
        println!("Byte {}.{}: separate_uv_delta_q = {} (1 bit)", byte, bit, sep);
    }

    // film_grain_params_present (1 bit)
    let (byte, bit) = reader.bit_position();
    header.film_grain_params_present = reader.read_bits(1).ok_or("Failed to read film_grain_params_present")? == 1;
    println!("Byte {}.{}: film_grain_params_present = {} (1 bit)", byte, bit, header.film_grain_params_present);

    Ok(header)
}

fn main() {
    // Our encoder output (10 bytes)
    let our_payload = vec![0x00, 0x00, 0x00, 0x05, 0x57, 0xff, 0xc0, 0x01, 0x00, 0x00];

    // libaom reference output (10 bytes)
    let ref_payload = vec![0x00, 0x00, 0x00, 0x02, 0xaf, 0xff, 0x9b, 0x5f, 0x30, 0x08];

    println!("========================================");
    println!("OUR ENCODER OUTPUT (64x64)");
    println!("========================================");
    print!("Raw bytes: ");
    for b in &our_payload {
        print!("{:02x} ", b);
    }
    println!("\n");

    let mut our_reader = BitReader::new(our_payload.clone());
    match decode_sequence_header(&mut our_reader) {
        Ok(_) => {},
        Err(e) => println!("Error decoding our payload: {}", e),
    }

    println!("\n\n========================================");
    println!("LIBAOM REFERENCE OUTPUT (64x64)");
    println!("========================================");
    print!("Raw bytes: ");
    for b in &ref_payload {
        print!("{:02x} ", b);
    }
    println!("\n");

    let mut ref_reader = BitReader::new(ref_payload.clone());
    match decode_sequence_header(&mut ref_reader) {
        Ok(_) => {},
        Err(e) => println!("Error decoding reference payload: {}", e),
    }

    println!("\n\n========================================");
    println!("ANALYSIS");
    println!("========================================");

    // Find first divergence
    for (i, (our, refb)) in our_payload.iter().zip(ref_payload.iter()).enumerate() {
        if our != refb {
            println!("First byte divergence at byte {}: Our={:02x} ({:08b}), Ref={:02x} ({:08b})",
                     i, our, our, refb, refb);
            break;
        }
    }
}
