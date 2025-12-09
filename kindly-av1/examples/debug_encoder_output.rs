//! Debug test to dump encoder output and compare to ffmpeg

use kindly_av1::encoder::{
    BitstreamWriterCapsule, ColorConfig, EncoderWiringCapsule, FrameHeader, FrameType,
    SequenceHeader,
};

fn main() {
    // FFmpeg exact bytes for 64x64 gray lossless
    let ffmpeg_bytes: &[u8] = &[
        // Temporal Delimiter: type=2, size=0
        0x12, 0x00, // Sequence Header: type=1, size=10
        0x0a, 0x0a, // Seq header payload (10 bytes)
        0x00, 0x00, 0x00, 0x02, 0xaf, 0xff, 0x9b, 0x5f, 0x20, 0x08,
        // Frame OBU: type=6, size=10
        0x32, 0x0a, // Frame payload (10 bytes)
        0x10, 0x00, 0x80, 0x00, 0x00, 0x4a, 0x7d, 0xf7, 0xff, 0xff,
    ];

    println!("FFmpeg reference ({} bytes):", ffmpeg_bytes.len());
    print!("  ");
    for (i, b) in ffmpeg_bytes.iter().enumerate() {
        print!("{:02x} ", b);
        if (i + 1) % 16 == 0 {
            print!("\n  ");
        }
    }
    println!("\n");

    // Generate our output
    let width = 64u32;
    let height = 64u32;

    let seq_hdr = SequenceHeader {
        seq_profile: 0,
        max_frame_width: width,
        max_frame_height: height,
        bit_depth: 8,
        use_128x128_superblock: false,
        enable_filter_intra: true,
        enable_intra_edge_filter: true,
        enable_interintra_compound: true,
        enable_masked_compound: true,
        enable_warped_motion: true,
        enable_dual_filter: true,
        enable_order_hint: true,
        order_hint_bits: 8,
    };

    let color_cfg = ColorConfig {
        high_bitdepth: false,
        twelve_bit: false,
        mono_chrome: false,
        color_description_present: false,
        color_primaries: 2,
        transfer_characteristics: 2,
        matrix_coefficients: 2,
        color_range: false,
        subsampling_x: true,
        subsampling_y: true,
        chroma_sample_position: 0,
        separate_uv_delta_q: false,
    };

    let mut bitstream_writer = BitstreamWriterCapsule::new();
    let mut encoded_data = Vec::new();

    // Write temporal delimiter
    let td_size = bitstream_writer.write_temporal_delimiter();
    encoded_data.extend_from_slice(&bitstream_writer.buffer()[..td_size]);

    println!("After TD ({} bytes):", encoded_data.len());
    print!("  ");
    for (i, b) in encoded_data.iter().enumerate() {
        print!("{:02x} ", b);
        if (i + 1) % 16 == 0 {
            print!("\n  ");
        }
    }
    println!("\n");

    // Write sequence header (full mode)
    bitstream_writer.reset();
    let sh_size = bitstream_writer.write_sequence_header_full(&seq_hdr, &color_cfg);
    encoded_data.extend_from_slice(&bitstream_writer.buffer()[..sh_size]);

    println!(
        "After SeqHeader ({} bytes, sh_size={}):",
        encoded_data.len(),
        sh_size
    );
    print!("  ");
    for (i, b) in encoded_data.iter().enumerate() {
        print!("{:02x} ", b);
        if (i + 1) % 16 == 0 {
            print!("\n  ");
        }
    }
    println!("\n");

    // Write frame OBU (full mode)
    let frame_hdr = FrameHeader {
        frame_type: FrameType::KeyFrame,
        show_frame: true,
        show_existing_frame: false,
        frame_width: width,
        frame_height: height,
        render_width: width,
        render_height: height,
        tile_cols_log2: 0,
        tile_rows_log2: 0,
        reduced_tx_set: false,
        base_q_idx: 0,
    };

    let tile_data = vec![0x4a, 0x7d, 0xf7, 0xff, 0xff];

    bitstream_writer.reset();
    let frame_obu_size = bitstream_writer.write_frame_obu_full(&frame_hdr, &seq_hdr, &tile_data);
    encoded_data.extend_from_slice(&bitstream_writer.buffer()[..frame_obu_size]);

    println!(
        "Final output ({} bytes, frame_obu_size={}):",
        encoded_data.len(),
        frame_obu_size
    );
    print!("  ");
    for (i, b) in encoded_data.iter().enumerate() {
        print!("{:02x} ", b);
        if (i + 1) % 16 == 0 {
            print!("\n  ");
        }
    }
    println!("\n");

    // Compare byte by byte
    println!("Comparison (our bytes vs ffmpeg):");
    let max_len = std::cmp::max(encoded_data.len(), ffmpeg_bytes.len());
    for i in 0..max_len {
        let ours = encoded_data
            .get(i)
            .map(|b| format!("{:02x}", b))
            .unwrap_or_else(|| "--".to_string());
        let ffmp = ffmpeg_bytes
            .get(i)
            .map(|b| format!("{:02x}", b))
            .unwrap_or_else(|| "--".to_string());
        let match_str = if encoded_data.get(i) == ffmpeg_bytes.get(i) {
            "OK"
        } else {
            "MISMATCH"
        };
        println!("  [{:2}] ours={} ffmpeg={} {}", i, ours, ffmp, match_str);
    }
}
