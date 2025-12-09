//! Bitstream comparison test - compare our output with FFmpeg's reference bytes
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL

use kindly_av1::encoder::{
    BitstreamWriterCapsule, ColorConfig, FrameHeader, FrameType, SequenceHeader,
};

#[test]
fn test_compare_sequence_header() {
    let mut bw = BitstreamWriterCapsule::new();

    // FFmpeg's working bytes
    let ffmpeg_bytes: [u8; 8] = [0x0a, 0x06, 0x18, 0x15, 0x7f, 0xfd, 0xb0, 0x08];

    // Our config
    let seq_hdr = SequenceHeader {
        seq_profile: 0,
        max_frame_width: 64,
        max_frame_height: 64,
        bit_depth: 8,
        use_128x128_superblock: false,
        enable_filter_intra: false,
        enable_intra_edge_filter: false,
        enable_interintra_compound: false,
        enable_masked_compound: false,
        enable_warped_motion: false,
        enable_dual_filter: false,
        enable_order_hint: false,
        order_hint_bits: 0,
    };

    let color_cfg = ColorConfig {
        high_bitdepth: false,
        twelve_bit: false,
        mono_chrome: true,
        color_description_present: false, // No color descriptors
        color_primaries: 2,               // Not written when present=false
        transfer_characteristics: 2,      // Not written when present=false
        matrix_coefficients: 2,           // Not written when present=false
        color_range: true,                // Full range (matching FFmpeg's bit 32)
        subsampling_x: false,
        subsampling_y: false,
        chroma_sample_position: 0,
        separate_uv_delta_q: false,
    };

    let size = bw.write_sequence_header_reduced(&seq_hdr, &color_cfg);
    let our_bytes = &bw.buffer()[..size];

    println!("\n=== SEQUENCE HEADER COMPARISON ===");
    println!(
        "FFmpeg ({} bytes): {:02x?}",
        ffmpeg_bytes.len(),
        ffmpeg_bytes
    );
    println!("Ours   ({} bytes): {:02x?}", size, our_bytes);

    // Bit-by-bit comparison
    for i in 0..ffmpeg_bytes.len().max(size) {
        let ff = ffmpeg_bytes.get(i).copied().unwrap_or(0);
        let ours = our_bytes.get(i).copied().unwrap_or(0);
        let match_str = if ff == ours { "✓" } else { "✗ DIFF" };
        println!(
            "Byte {}: FFmpeg={:02x} ({:08b})  Ours={:02x} ({:08b}) {}",
            i, ff, ff, ours, ours, match_str
        );
    }
}

#[test]
fn test_compare_frame_obu() {
    let mut bw = BitstreamWriterCapsule::new();

    // FFmpeg's working bytes
    let ffmpeg_bytes: [u8; 12] = [
        0x32, 0x0a, 0x18, 0x00, 0x00, 0x00, 0x50, 0x00, 0x00, 0x00, 0x09, 0xac,
    ];

    // Our config with base_q_idx=48 (as identified by agent)
    let seq_hdr = SequenceHeader {
        seq_profile: 0,
        max_frame_width: 64,
        max_frame_height: 64,
        bit_depth: 8,
        use_128x128_superblock: false,
        enable_filter_intra: false,
        enable_intra_edge_filter: false,
        enable_interintra_compound: false,
        enable_masked_compound: false,
        enable_warped_motion: false,
        enable_dual_filter: false,
        enable_order_hint: false,
        order_hint_bits: 0,
    };

    let frame_hdr = FrameHeader {
        frame_type: FrameType::KeyFrame,
        show_frame: true,
        show_existing_frame: false,
        frame_width: 64,
        frame_height: 64,
        render_width: 64,
        render_height: 64,
        tile_cols_log2: 0,
        tile_rows_log2: 0,
        reduced_tx_set: true,
        base_q_idx: 48, // Agent says FFmpeg uses 48
        // Inter-frame reference fields (defaults for keyframe)
        primary_ref_frame: 7, // PRIMARY_REF_NONE
        ref_frame_idx: [0; 7],
        refresh_frame_flags: 0xff, // Refresh all slots for keyframe
        allow_ref_frame_mvs: false,
    };

    // FFmpeg's tile data for comparison
    let tile_data: &[u8] = &[0x09, 0xac];

    let size = bw.write_frame_obu_reduced(&frame_hdr, &seq_hdr, tile_data);
    let our_bytes = &bw.buffer()[..size];

    println!("\n=== FRAME OBU COMPARISON ===");
    println!(
        "FFmpeg ({} bytes): {:02x?}",
        ffmpeg_bytes.len(),
        ffmpeg_bytes
    );
    println!("Ours   ({} bytes): {:02x?}", size, our_bytes);

    // Bit-by-bit comparison
    for i in 0..ffmpeg_bytes.len().max(size) {
        let ff = ffmpeg_bytes.get(i).copied().unwrap_or(0);
        let ours = our_bytes.get(i).copied().unwrap_or(0);
        let match_str = if ff == ours { "✓" } else { "✗ DIFF" };
        println!(
            "Byte {}: FFmpeg={:02x} ({:08b})  Ours={:02x} ({:08b}) {}",
            i, ff, ff, ours, ours, match_str
        );
    }
}
