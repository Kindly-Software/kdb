#!/usr/bin/env rust-script
//! Bit-by-bit AV1 sequence header analyzer
//!
//! Analyzes the exact bit layout of sequence headers to find discrepancies

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8, // 0-7, position within current byte
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    fn can_read(&self, n: usize) -> bool {
        let bits_remaining = (self.data.len() - self.byte_pos) * 8 - self.bit_pos as usize;
        bits_remaining >= n
    }

    fn read_bits(&mut self, n: usize) -> Option<u64> {
        if !self.can_read(n) {
            println!("WARNING: Attempted to read {} bits but only {} bits remaining",
                     n, (self.data.len() - self.byte_pos) * 8 - self.bit_pos as usize);
            return None;
        }

        let mut result = 0u64;
        for _ in 0..n {
            if self.byte_pos >= self.data.len() {
                return None;
            }

            let bit = (self.data[self.byte_pos] >> (7 - self.bit_pos)) & 1;
            result = (result << 1) | (bit as u64);

            self.bit_pos += 1;
            if self.bit_pos == 8 {
                self.bit_pos = 0;
                self.byte_pos += 1;
            }
        }
        Some(result)
    }

    fn total_bits_read(&self) -> usize {
        self.byte_pos * 8 + self.bit_pos as usize
    }
}

#[derive(Debug)]
struct SequenceHeaderFields {
    seq_profile: u8,
    still_picture: bool,
    reduced_still_picture_header: bool,
    timing_info_present_flag: Option<bool>,
    decoder_model_info_present_flag: Option<bool>,
    initial_display_delay_present_flag: Option<bool>,
    operating_points_cnt_minus_1: Option<u8>,
    operating_point_idc: Vec<u16>,
    seq_level_idx: Vec<u8>,
    seq_tier: Vec<bool>,
    frame_width_bits_minus_1: u8,
    frame_height_bits_minus_1: u8,
    max_frame_width_minus_1: u16,
    max_frame_height_minus_1: u16,
    frame_id_numbers_present_flag: Option<bool>,
    use_128x128_superblock: bool,
    enable_filter_intra: bool,
    enable_intra_edge_filter: bool,
    enable_interintra_compound: Option<bool>,
    enable_masked_compound: Option<bool>,
    enable_warped_motion: Option<bool>,
    enable_dual_filter: Option<bool>,
    enable_order_hint: Option<bool>,
    enable_jnt_comp: Option<bool>,
    enable_ref_frame_mvs: Option<bool>,
    seq_choose_screen_content_tools: Option<bool>,
    seq_force_screen_content_tools: Option<u8>,
    seq_choose_integer_mv: Option<bool>,
    order_hint_bits_minus_1: Option<u8>,
    enable_superres: Option<bool>,
    enable_cdef: Option<bool>,
    enable_restoration: Option<bool>,
    // color_config fields
    high_bitdepth: Option<bool>,
    twelve_bit: Option<bool>,
    mono_chrome: Option<bool>,
    color_primaries: Option<u8>,
    transfer_characteristics: Option<u8>,
    matrix_coefficients: Option<u8>,
    color_range: Option<bool>,
    subsampling_x: Option<bool>,
    subsampling_y: Option<bool>,
    chroma_sample_position: Option<u8>,
    separate_uv_delta_q: Option<bool>,
    film_grain_params_present: Option<bool>,
}

impl SequenceHeaderFields {
    fn print_comparison(&self, other: &Self) {
        println!("\n=== FIELD-BY-FIELD COMPARISON ===\n");

        macro_rules! compare_field {
            ($field:ident, $name:expr) => {
                if self.$field != other.$field {
                    println!("❌ MISMATCH: {}", $name);
                    println!("   Ours:      {:?}", self.$field);
                    println!("   Reference: {:?}", other.$field);
                } else {
                    println!("✓ {}: {:?}", $name, self.$field);
                }
            };
        }

        compare_field!(seq_profile, "seq_profile");
        compare_field!(still_picture, "still_picture");
        compare_field!(reduced_still_picture_header, "reduced_still_picture_header");
        compare_field!(timing_info_present_flag, "timing_info_present_flag");
        compare_field!(decoder_model_info_present_flag, "decoder_model_info_present_flag");
        compare_field!(initial_display_delay_present_flag, "initial_display_delay_present_flag");
        compare_field!(operating_points_cnt_minus_1, "operating_points_cnt_minus_1");
        compare_field!(operating_point_idc, "operating_point_idc");
        compare_field!(seq_level_idx, "seq_level_idx");
        compare_field!(seq_tier, "seq_tier");
        compare_field!(frame_width_bits_minus_1, "frame_width_bits_minus_1");
        compare_field!(frame_height_bits_minus_1, "frame_height_bits_minus_1");
        compare_field!(max_frame_width_minus_1, "max_frame_width_minus_1");
        compare_field!(max_frame_height_minus_1, "max_frame_height_minus_1");
        compare_field!(frame_id_numbers_present_flag, "frame_id_numbers_present_flag");
        compare_field!(use_128x128_superblock, "use_128x128_superblock");
        compare_field!(enable_filter_intra, "enable_filter_intra");
        compare_field!(enable_intra_edge_filter, "enable_intra_edge_filter");
        compare_field!(enable_interintra_compound, "enable_interintra_compound");
        compare_field!(enable_masked_compound, "enable_masked_compound");
        compare_field!(enable_warped_motion, "enable_warped_motion");
        compare_field!(enable_dual_filter, "enable_dual_filter");
        compare_field!(enable_order_hint, "enable_order_hint");
        compare_field!(enable_jnt_comp, "enable_jnt_comp");
        compare_field!(enable_ref_frame_mvs, "enable_ref_frame_mvs");
        compare_field!(seq_choose_screen_content_tools, "seq_choose_screen_content_tools");
        compare_field!(seq_force_screen_content_tools, "seq_force_screen_content_tools");
        compare_field!(seq_choose_integer_mv, "seq_choose_integer_mv");
        compare_field!(order_hint_bits_minus_1, "order_hint_bits_minus_1");
        compare_field!(enable_superres, "enable_superres");
        compare_field!(enable_cdef, "enable_cdef");
        compare_field!(enable_restoration, "enable_restoration");
        compare_field!(high_bitdepth, "high_bitdepth");
        compare_field!(twelve_bit, "twelve_bit");
        compare_field!(mono_chrome, "mono_chrome");
        compare_field!(color_primaries, "color_primaries");
        compare_field!(transfer_characteristics, "transfer_characteristics");
        compare_field!(matrix_coefficients, "matrix_coefficients");
        compare_field!(color_range, "color_range");
        compare_field!(subsampling_x, "subsampling_x");
        compare_field!(subsampling_y, "subsampling_y");
        compare_field!(chroma_sample_position, "chroma_sample_position");
        compare_field!(separate_uv_delta_q, "separate_uv_delta_q");
        compare_field!(film_grain_params_present, "film_grain_params_present");
    }
}

fn parse_sequence_header(data: &[u8], name: &str) -> Option<(SequenceHeaderFields, usize)> {
    println!("\n=== PARSING {} ===", name);
    println!("Raw data: {:02x?}", data);
    println!("Length: {} bytes", data.len());

    let mut reader = BitReader::new(data);

    // seq_profile (3 bits)
    let seq_profile = reader.read_bits(3)?? as u8;
    println!("\nseq_profile: {} (bits 0-2)", seq_profile);

    // still_picture (1 bit)
    let still_picture = reader.read_bits(1)? != 0;
    println!("still_picture: {} (bit 3)", still_picture);

    // reduced_still_picture_header (1 bit)
    let reduced_still_picture_header = reader.read_bits(1)? != 0;
    println!("reduced_still_picture_header: {} (bit 4)", reduced_still_picture_header);

    let mut fields = SequenceHeaderFields {
        seq_profile,
        still_picture,
        reduced_still_picture_header,
        timing_info_present_flag: None,
        decoder_model_info_present_flag: None,
        initial_display_delay_present_flag: None,
        operating_points_cnt_minus_1: None,
        operating_point_idc: Vec::new(),
        seq_level_idx: Vec::new(),
        seq_tier: Vec::new(),
        frame_width_bits_minus_1: 0,
        frame_height_bits_minus_1: 0,
        max_frame_width_minus_1: 0,
        max_frame_height_minus_1: 0,
        frame_id_numbers_present_flag: None,
        use_128x128_superblock: false,
        enable_filter_intra: false,
        enable_intra_edge_filter: false,
        enable_interintra_compound: None,
        enable_masked_compound: None,
        enable_warped_motion: None,
        enable_dual_filter: None,
        enable_order_hint: None,
        enable_jnt_comp: None,
        enable_ref_frame_mvs: None,
        seq_choose_screen_content_tools: None,
        seq_force_screen_content_tools: None,
        seq_choose_integer_mv: None,
        order_hint_bits_minus_1: None,
        enable_superres: None,
        enable_cdef: None,
        enable_restoration: None,
        high_bitdepth: None,
        twelve_bit: None,
        mono_chrome: None,
        color_primaries: None,
        transfer_characteristics: None,
        matrix_coefficients: None,
        color_range: None,
        subsampling_x: None,
        subsampling_y: None,
        chroma_sample_position: None,
        separate_uv_delta_q: None,
        film_grain_params_present: None,
    };

    if !reduced_still_picture_header {
        let timing_info = reader.read_bits(1)? != 0;
        fields.timing_info_present_flag = Some(timing_info);
        println!("timing_info_present_flag: {} (bit 5)", timing_info);

        let decoder_model = reader.read_bits(1)? != 0;
        fields.decoder_model_info_present_flag = Some(decoder_model);
        println!("decoder_model_info_present_flag: {} (bit 6)", decoder_model);

        let initial_display = reader.read_bits(1)? != 0;
        fields.initial_display_delay_present_flag = Some(initial_display);
        println!("initial_display_delay_present_flag: {} (bit 7)", initial_display);

        let op_cnt = reader.read_bits(5)? as u8;
        fields.operating_points_cnt_minus_1 = Some(op_cnt);
        println!("operating_points_cnt_minus_1: {} (bits 8-12)", op_cnt);

        for i in 0..=op_cnt {
            let op_idc = reader.read_bits(12)? as u16;
            fields.operating_point_idc.push(op_idc);
            println!("  operating_point_idc[{}]: {} (12 bits)", i, op_idc);

            let level_idx = reader.read_bits(5)? as u8;
            fields.seq_level_idx.push(level_idx);
            println!("  seq_level_idx[{}]: {} (5 bits)", i, level_idx);

            if level_idx > 7 {
                let tier = reader.read_bits(1)? != 0;
                fields.seq_tier.push(tier);
                println!("  seq_tier[{}]: {} (1 bit)", i, tier);
            }
        }
    }

    let frame_width_bits = reader.read_bits(4)? as u8;
    fields.frame_width_bits_minus_1 = frame_width_bits;
    println!("\nframe_width_bits_minus_1: {} (4 bits) - actual width uses {} bits",
             frame_width_bits, frame_width_bits + 1);

    let frame_height_bits = reader.read_bits(4)? as u8;
    fields.frame_height_bits_minus_1 = frame_height_bits;
    println!("frame_height_bits_minus_1: {} (4 bits) - actual height uses {} bits",
             frame_height_bits, frame_height_bits + 1);

    let max_width = reader.read_bits((frame_width_bits + 1)? as usize) as u16;
    fields.max_frame_width_minus_1 = max_width;
    println!("max_frame_width_minus_1: {} ({} bits) -> actual width: {}",
             max_width, frame_width_bits + 1, max_width + 1);

    let max_height = reader.read_bits((frame_height_bits + 1)? as usize) as u16;
    fields.max_frame_height_minus_1 = max_height;
    println!("max_frame_height_minus_1: {} ({} bits) -> actual height: {}",
             max_height, frame_height_bits + 1, max_height + 1);

    if !reduced_still_picture_header {
        let frame_id_flag = reader.read_bits(1)? != 0;
        fields.frame_id_numbers_present_flag = Some(frame_id_flag);
        println!("frame_id_numbers_present_flag: {} (1 bit)", frame_id_flag);
    }

    let use_128 = reader.read_bits(1)? != 0;
    fields.use_128x128_superblock = use_128;
    println!("use_128x128_superblock: {} (1 bit)", use_128);

    let filter_intra = reader.read_bits(1)? != 0;
    fields.enable_filter_intra = filter_intra;
    println!("enable_filter_intra: {} (1 bit)", filter_intra);

    let intra_edge = reader.read_bits(1)? != 0;
    fields.enable_intra_edge_filter = intra_edge;
    println!("enable_intra_edge_filter: {} (1 bit)", intra_edge);

    if !reduced_still_picture_header {
        let interintra = reader.read_bits(1)? != 0;
        fields.enable_interintra_compound = Some(interintra);
        println!("enable_interintra_compound: {} (1 bit)", interintra);

        let masked = reader.read_bits(1)? != 0;
        fields.enable_masked_compound = Some(masked);
        println!("enable_masked_compound: {} (1 bit)", masked);

        let warped = reader.read_bits(1)? != 0;
        fields.enable_warped_motion = Some(warped);
        println!("enable_warped_motion: {} (1 bit)", warped);

        let dual = reader.read_bits(1)? != 0;
        fields.enable_dual_filter = Some(dual);
        println!("enable_dual_filter: {} (1 bit)", dual);

        let order_hint = reader.read_bits(1)? != 0;
        fields.enable_order_hint = Some(order_hint);
        println!("enable_order_hint: {} (1 bit)", order_hint);

        if order_hint {
            let jnt_comp = reader.read_bits(1)? != 0;
            fields.enable_jnt_comp = Some(jnt_comp);
            println!("enable_jnt_comp: {} (1 bit)", jnt_comp);

            let ref_mvs = reader.read_bits(1)? != 0;
            fields.enable_ref_frame_mvs = Some(ref_mvs);
            println!("enable_ref_frame_mvs: {} (1 bit)", ref_mvs);
        }

        let choose_screen = reader.read_bits(1)? != 0;
        fields.seq_choose_screen_content_tools = Some(choose_screen);
        println!("seq_choose_screen_content_tools: {} (1 bit)", choose_screen);

        if !choose_screen {
            let force_screen = reader.read_bits(2)? as u8;
            fields.seq_force_screen_content_tools = Some(force_screen);
            println!("seq_force_screen_content_tools: {} (2 bits)", force_screen);
        }

        let force_screen_val = fields.seq_force_screen_content_tools.unwrap_or(2);
        if force_screen_val != 2 {
            let choose_int_mv = reader.read_bits(1)? != 0;
            fields.seq_choose_integer_mv = Some(choose_int_mv);
            println!("seq_choose_integer_mv: {} (1 bit)", choose_int_mv);
        }

        if order_hint {
            let order_bits = reader.read_bits(3)? as u8;
            fields.order_hint_bits_minus_1 = Some(order_bits);
            println!("order_hint_bits_minus_1: {} (3 bits)", order_bits);
        }
    }

    let superres = reader.read_bits(1)? != 0;
    fields.enable_superres = Some(superres);
    println!("enable_superres: {} (1 bit)", superres);

    let cdef = reader.read_bits(1)? != 0;
    fields.enable_cdef = Some(cdef);
    println!("enable_cdef: {} (1 bit)", cdef);

    let restoration = reader.read_bits(1)? != 0;
    fields.enable_restoration = Some(restoration);
    println!("enable_restoration: {} (1 bit)", restoration);

    println!("\n--- COLOR CONFIG ---");
    let high_bd = reader.read_bits(1)? != 0;
    fields.high_bitdepth = Some(high_bd);
    println!("high_bitdepth: {} (1 bit)", high_bd);

    let twelve_bit_val = if seq_profile == 2 && high_bd {
        let tb = reader.read_bits(1)? != 0;
        fields.twelve_bit = Some(tb);
        println!("twelve_bit: {} (1 bit)", tb);
        tb
    } else {
        false
    };

    let bit_depth = if twelve_bit_val { 12 } else if high_bd { 10 } else { 8 };
    println!("Computed bit_depth: {}", bit_depth);

    let mono = if seq_profile == 1 {
        false
    } else {
        let m = reader.read_bits(1)? != 0;
        fields.mono_chrome = Some(m);
        println!("mono_chrome: {} (1 bit)", m);
        m
    };

    let color_desc_present = reader.read_bits(1)? != 0;
    println!("color_description_present_flag: {} (1 bit)", color_desc_present);

    if color_desc_present {
        let cp = reader.read_bits(8)? as u8;
        fields.color_primaries = Some(cp);
        println!("color_primaries: {} (8 bits)", cp);

        let tc = reader.read_bits(8)? as u8;
        fields.transfer_characteristics = Some(tc);
        println!("transfer_characteristics: {} (8 bits)", tc);

        let mc = reader.read_bits(8)? as u8;
        fields.matrix_coefficients = Some(mc);
        println!("matrix_coefficients: {} (8 bits)", mc);
    }

    let color_range = reader.read_bits(1)? != 0;
    fields.color_range = Some(color_range);
    println!("color_range: {} (1 bit)", color_range);

    if seq_profile == 0 {
        // YUV 4:2:0
        fields.subsampling_x = Some(true);
        fields.subsampling_y = Some(true);
        println!("subsampling_x: true (implied by profile 0)");
        println!("subsampling_y: true (implied by profile 0)");
    } else if seq_profile == 1 {
        // YUV 4:4:4
        fields.subsampling_x = Some(false);
        fields.subsampling_y = Some(false);
        println!("subsampling_x: false (implied by profile 1)");
        println!("subsampling_y: false (implied by profile 1)");
    } else {
        if bit_depth == 12 {
            let sub_x = reader.read_bits(1)? != 0;
            fields.subsampling_x = Some(sub_x);
            println!("subsampling_x: {} (1 bit)", sub_x);

            if sub_x {
                let sub_y = reader.read_bits(1)? != 0;
                fields.subsampling_y = Some(sub_y);
                println!("subsampling_y: {} (1 bit)", sub_y);
            } else {
                fields.subsampling_y = Some(false);
                println!("subsampling_y: false (implied by subsampling_x=0)");
            }
        } else {
            fields.subsampling_x = Some(true);
            fields.subsampling_y = Some(true);
            println!("subsampling_x: true (implied by bit_depth != 12)");
            println!("subsampling_y: true (implied by bit_depth != 12)");
        }
    }

    if fields.subsampling_x.unwrap_or(false) && fields.subsampling_y.unwrap_or(false) && !mono {
        let chroma_pos = reader.read_bits(2)? as u8;
        fields.chroma_sample_position = Some(chroma_pos);
        println!("chroma_sample_position: {} (2 bits)", chroma_pos);
    }

    let separate_uv = reader.read_bits(1)? != 0;
    fields.separate_uv_delta_q = Some(separate_uv);
    println!("separate_uv_delta_q: {} (1 bit)", separate_uv);

    let film_grain = reader.read_bits(1)? != 0;
    fields.film_grain_params_present = Some(film_grain);
    println!("film_grain_params_present: {} (1 bit)", film_grain);

    let total_bits = reader.total_bits_read();
    println!("\nTotal bits read: {} ({} bytes + {} bits)",
             total_bits, total_bits / 8, total_bits % 8);

    (fields, total_bits)
}

fn main() {
    println!("==========================================================");
    println!("AV1 SEQUENCE HEADER BIT-BY-BIT ANALYZER");
    println!("==========================================================");

    // Our sequence header (9 bytes)
    let our_header = [
        0x00, 0x00, 0x00, 0x05, 0x57, 0xff, 0xc0, 0x02, 0x00
    ];

    // Reference sequence header (10 bytes from libaom 64x64)
    let ref_header = [
        0x00, 0x00, 0x00, 0x02, 0xaf, 0xff, 0x9b, 0x5f, 0x30, 0x08
    ];

    let (our_fields, our_bits) = parse_sequence_header(&our_header, "OUR HEADER");
    let (ref_fields, ref_bits) = parse_sequence_header(&ref_header, "REFERENCE HEADER");

    println!("\n==========================================================");
    println!("BIT COUNT COMPARISON");
    println!("==========================================================");
    println!("Our header:       {} bits ({} bytes + {} bits)", our_bits, our_bits / 8, our_bits % 8);
    println!("Reference header: {} bits ({} bytes + {} bits)", ref_bits, ref_bits / 8, ref_bits % 8);
    println!("Difference:       {} bits ({} bytes)",
             ref_bits.saturating_sub(our_bits),
             (ref_bits.saturating_sub(our_bits)) / 8);

    our_fields.print_comparison(&ref_fields);

    println!("\n==========================================================");
    println!("CONCLUSION");
    println!("==========================================================");
    if our_bits != ref_bits {
        println!("❌ Bit count mismatch detected!");
        println!("   The discrepancy is {} bits ({} bytes)",
                 ref_bits.saturating_sub(our_bits),
                 (ref_bits.saturating_sub(our_bits)) / 8);
    } else {
        println!("✓ Bit counts match!");
    }
}
