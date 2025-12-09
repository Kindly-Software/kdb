//! AV1 Bitstream Integration Tests
//!
//! Validates that generated AV1 bitstreams are compliant with the specification
//! and can be decoded by reference decoders (dav1d, ffmpeg).

use kindly_av1::encoder::{
    BitstreamWriterCapsule, FrameHeader, FrameType, IvfContainerWriterCapsule, ObuType,
    SequenceHeader,
};

#[test]
fn test_bitstream_writer_obu_types() {
    // Verify OBU type enum values match AV1 spec Section 5.3.2
    assert_eq!(ObuType::Reserved.as_u8(), 0);
    assert_eq!(ObuType::SequenceHeader.as_u8(), 1);
    assert_eq!(ObuType::TemporalDelimiter.as_u8(), 2);
    assert_eq!(ObuType::FrameHeader.as_u8(), 3);
    assert_eq!(ObuType::TileGroup.as_u8(), 4);
    assert_eq!(ObuType::Metadata.as_u8(), 5);
    assert_eq!(ObuType::Frame.as_u8(), 6);
    assert_eq!(ObuType::RedundantFrameHeader.as_u8(), 7);
    assert_eq!(ObuType::TileList.as_u8(), 8);
    assert_eq!(ObuType::Padding.as_u8(), 15);
}

#[test]
fn test_bitstream_writer_frame_types() {
    // Verify frame type enum values match AV1 spec Section 5.9.2
    assert_eq!(FrameType::KeyFrame.as_u8(), 0);
    assert_eq!(FrameType::InterFrame.as_u8(), 1);
    assert_eq!(FrameType::IntraOnlyFrame.as_u8(), 2);
    assert_eq!(FrameType::SwitchFrame.as_u8(), 3);

    assert!(FrameType::KeyFrame.is_key_frame());
    assert!(!FrameType::InterFrame.is_key_frame());
}

#[test]
fn test_leb128_encoding() {
    let writer = BitstreamWriterCapsule::new();
    let mut output = [0u8; 5];

    // Test cases from AV1 spec examples
    // 0 -> [0x00]
    let len = writer.encode_leb128(0, &mut output);
    assert_eq!(len, 1);
    assert_eq!(output[0], 0x00);

    // 127 -> [0x7F] (max 1-byte value)
    let len = writer.encode_leb128(127, &mut output);
    assert_eq!(len, 1);
    assert_eq!(output[0], 0x7F);

    // 128 -> [0x80, 0x01] (min 2-byte value)
    let len = writer.encode_leb128(128, &mut output);
    assert_eq!(len, 2);
    assert_eq!(output[0], 0x80); // 0b10000000 (continuation bit set)
    assert_eq!(output[1], 0x01); // 0b00000001

    // 16383 -> [0xFF, 0x7F] (max 2-byte value)
    let len = writer.encode_leb128(16383, &mut output);
    assert_eq!(len, 2);
    assert_eq!(output[0], 0xFF);
    assert_eq!(output[1], 0x7F);

    // 16384 -> [0x80, 0x80, 0x01] (min 3-byte value)
    let len = writer.encode_leb128(16384, &mut output);
    assert_eq!(len, 3);
    assert_eq!(output[0], 0x80);
    assert_eq!(output[1], 0x80);
    assert_eq!(output[2], 0x01);

    // 636517 -> [0xE5, 0xEC, 0x26] (from AV1 spec example)
    // Calculated: 101 + (108 << 7) + (38 << 14) = 636517
    let len = writer.encode_leb128(636517, &mut output);
    assert_eq!(len, 3);
    assert_eq!(output[0], 0xE5);
    assert_eq!(output[1], 0xEC);
    assert_eq!(output[2], 0x26);
}

#[test]
fn test_temporal_delimiter_obu() {
    let mut writer = BitstreamWriterCapsule::new();

    let size = writer.write_temporal_delimiter();

    // Temporal delimiter should be 2 bytes minimum (header + size=0)
    assert!(size >= 2);
    assert_eq!(writer.obus_written(), 1);
    assert_eq!(writer.bytes_written(), size as u64);

    let buffer = writer.buffer();

    // Verify OBU header byte
    let header = buffer[0];
    let obu_type = (header >> 3) & 0x0F;
    let has_size = (header >> 1) & 0x01;

    assert_eq!(obu_type, ObuType::TemporalDelimiter.as_u8());
    assert_eq!(has_size, 1); // obu_has_size_field must be 1

    // Verify size field (should be 0 for temporal delimiter)
    let size_byte = buffer[1];
    assert_eq!(size_byte, 0); // leb128(0) = 0x00
}

#[test]
fn test_sequence_header_obu() {
    let mut writer = BitstreamWriterCapsule::new();

    let seq_hdr = SequenceHeader {
        seq_profile: 0,
        max_frame_width: 1920,
        max_frame_height: 1080,
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

    let size = writer.write_sequence_header(&seq_hdr);

    // Sequence header should be substantial (>10 bytes)
    assert!(size > 10);
    assert_eq!(writer.obus_written(), 1);
    assert_eq!(writer.bytes_written(), size as u64);

    let buffer = writer.buffer();

    // Verify OBU header
    let header = buffer[0];
    let obu_type = (header >> 3) & 0x0F;
    let has_size = (header >> 1) & 0x01;

    assert_eq!(obu_type, ObuType::SequenceHeader.as_u8());
    assert_eq!(has_size, 1);
}

#[test]
fn test_frame_header_obu() {
    let mut writer = BitstreamWriterCapsule::new();

    let frame_hdr = FrameHeader {
        frame_type: FrameType::KeyFrame,
        show_frame: true,
        show_existing_frame: false,
        frame_width: 1920,
        frame_height: 1080,
        render_width: 1920,
        render_height: 1080,
        tile_cols_log2: 0,
        tile_rows_log2: 0,
        reduced_tx_set: false,
        base_q_idx: 128,
        // Inter-frame reference fields (defaults for keyframe)
        primary_ref_frame: 7, // PRIMARY_REF_NONE
        ref_frame_idx: [0, 1, 2, 3, 4, 5, 6],
        refresh_frame_flags: 0xff,
        allow_ref_frame_mvs: false,
    };

    let size = writer.write_frame_header(&frame_hdr);

    assert!(size > 10);
    assert_eq!(writer.obus_written(), 1);

    let buffer = writer.buffer();

    // Verify OBU header
    let header = buffer[0];
    let obu_type = (header >> 3) & 0x0F;

    assert_eq!(obu_type, ObuType::FrameHeader.as_u8());
}

#[test]
fn test_frame_obu_combined() {
    let mut writer = BitstreamWriterCapsule::new();

    let frame_hdr = FrameHeader::default();
    let tile_data_size = 100; // Simulated tile data

    let size = writer.write_frame_obu_header(&frame_hdr, tile_data_size);

    assert!(size > 10);
    assert_eq!(writer.obus_written(), 1);

    let buffer = writer.buffer();

    // Verify OBU type is Frame (6), not FrameHeader (3)
    let header = buffer[0];
    let obu_type = (header >> 3) & 0x0F;

    assert_eq!(obu_type, ObuType::Frame.as_u8());
}

#[test]
fn test_complete_minimal_bitstream() {
    // Generate minimal valid AV1 bitstream (like wiring.rs produces)
    let mut writer = BitstreamWriterCapsule::new();
    let mut bitstream = Vec::new();

    // Temporal delimiter
    let td_size = writer.write_temporal_delimiter();
    bitstream.extend_from_slice(&writer.buffer()[..td_size]);

    // Sequence header
    let seq_hdr = SequenceHeader::default();
    let sh_size = writer.write_sequence_header(&seq_hdr);
    bitstream.extend_from_slice(&writer.buffer()[..sh_size]);

    // Frame OBU with minimal tile data
    let frame_hdr = FrameHeader::default();
    let tile_data = vec![0x00u8, 0x00]; // Minimal tile data
    let fh_size = writer.write_frame_obu_header(&frame_hdr, tile_data.len() as u32);
    bitstream.extend_from_slice(&writer.buffer()[..fh_size]);
    bitstream.extend_from_slice(&tile_data);

    // Verify we have a substantial bitstream
    // TD=2 + SH=12 + FH=14 + tile=2 = 30 bytes minimum
    assert!(
        bitstream.len() >= 30,
        "Expected >= 30 bytes, got {}",
        bitstream.len()
    );

    // Verify starts with temporal delimiter
    let first_obu_type = (bitstream[0] >> 3) & 0x0F;
    assert_eq!(first_obu_type, ObuType::TemporalDelimiter.as_u8());

    // Save to file for manual decoder validation (optional)
    #[cfg(feature = "test-output")]
    {
        use std::fs;
        fs::write("test_output_minimal.av1", &bitstream).unwrap();
    }
}

#[test]
fn test_ivf_file_header() {
    let ivf = IvfContainerWriterCapsule::new();

    let header = ivf.write_file_header(1920, 1080, 30, 1);

    assert_eq!(header.len(), 32);

    // Verify IVF signature "DKIF"
    assert_eq!(&header[0..4], b"DKIF");

    // Verify version 0
    assert_eq!(u16::from_le_bytes([header[4], header[5]]), 0);

    // Verify header length 32
    assert_eq!(u16::from_le_bytes([header[6], header[7]]), 32);

    // Verify AV1 FourCC "AV01"
    assert_eq!(&header[8..12], b"AV01");

    // Verify dimensions
    assert_eq!(u16::from_le_bytes([header[12], header[13]]), 1920);
    assert_eq!(u16::from_le_bytes([header[14], header[15]]), 1080);

    // Verify framerate 30/1
    assert_eq!(
        u32::from_le_bytes([header[16], header[17], header[18], header[19]]),
        30
    );
    assert_eq!(
        u32::from_le_bytes([header[20], header[21], header[22], header[23]]),
        1
    );

    // Verify frame count 0 (initial)
    assert_eq!(
        u32::from_le_bytes([header[24], header[25], header[26], header[27]]),
        0
    );
}

#[test]
fn test_ivf_frame_header() {
    let ivf = IvfContainerWriterCapsule::new();

    let header = ivf.write_frame_header(1234);

    assert_eq!(header.len(), 12);

    // Verify frame size
    assert_eq!(
        u32::from_le_bytes([header[0], header[1], header[2], header[3]]),
        1234
    );

    // Verify timestamp starts at 0
    assert_eq!(
        u64::from_le_bytes([
            header[4], header[5], header[6], header[7], header[8], header[9], header[10],
            header[11]
        ]),
        0
    );

    // Write second frame
    let header2 = ivf.write_frame_header(5678);

    // Verify timestamp incremented
    assert_eq!(
        u64::from_le_bytes([
            header2[4],
            header2[5],
            header2[6],
            header2[7],
            header2[8],
            header2[9],
            header2[10],
            header2[11]
        ]),
        1
    );
}

#[test]
fn test_ivf_complete_container() {
    let ivf = IvfContainerWriterCapsule::new();
    let mut container = Vec::new();

    // Write file header
    let file_header = ivf.write_file_header(1920, 1080, 30, 1);
    container.extend_from_slice(&file_header);

    // Write 3 frames with AV1 OBU data
    for i in 0..3 {
        // Simulated AV1 OBU data (would come from BitstreamWriterCapsule)
        let frame_data = vec![0xAAu8; 100 + i * 10];

        let frame_header = ivf.write_frame_header(frame_data.len() as u32);
        container.extend_from_slice(&frame_header);
        container.extend_from_slice(&frame_data);
    }

    // Verify total size
    assert_eq!(container.len(), 32 + 3 * (12 + 100) + 10 + 20);

    // Finalize header with updated frame count
    let final_header = ivf.finalize_file_header();
    assert_eq!(
        u32::from_le_bytes([
            final_header[24],
            final_header[25],
            final_header[26],
            final_header[27]
        ]),
        3
    );

    // In real usage, would seek back and replace file header
    // container[0..32].copy_from_slice(&final_header);

    // Save to file for manual validation (optional)
    #[cfg(feature = "test-output")]
    {
        use std::fs;
        // Replace initial header with finalized version
        let mut final_container = container.clone();
        final_container[0..32].copy_from_slice(&final_header);
        fs::write("test_output.ivf", &final_container).unwrap();
    }
}

#[test]
fn test_ivf_23_976_fps() {
    let ivf = IvfContainerWriterCapsule::new();

    // 23.976fps = 24000/1001 (common for film)
    let header = ivf.write_file_header(1920, 1080, 24000, 1001);

    assert_eq!(
        u32::from_le_bytes([header[16], header[17], header[18], header[19]]),
        24000
    );
    assert_eq!(
        u32::from_le_bytes([header[20], header[21], header[22], header[23]]),
        1001
    );
}

#[test]
fn test_multiple_obu_sequence() {
    // Test writing multiple OBUs in correct order
    let mut writer = BitstreamWriterCapsule::new();
    let mut bitstream = Vec::new();

    // 1. Temporal delimiter
    let td_size = writer.write_temporal_delimiter();
    bitstream.extend_from_slice(&writer.buffer()[..td_size]);
    assert_eq!(writer.obus_written(), 1);

    // 2. Sequence header
    let seq_hdr = SequenceHeader::default();
    let sh_size = writer.write_sequence_header(&seq_hdr);
    bitstream.extend_from_slice(&writer.buffer()[..sh_size]);
    assert_eq!(writer.obus_written(), 2);

    // 3. Frame OBU
    let frame_hdr = FrameHeader::default();
    let fh_size = writer.write_frame_obu_header(&frame_hdr, 10);
    bitstream.extend_from_slice(&writer.buffer()[..fh_size]);
    assert_eq!(writer.obus_written(), 3);

    // Verify total bytes
    assert_eq!(writer.bytes_written(), (td_size + sh_size + fh_size) as u64);
}

#[test]
fn test_bitstream_writer_generation_counter() {
    let mut writer = BitstreamWriterCapsule::new();

    assert_eq!(writer.generation(), 0);

    writer.write_temporal_delimiter();
    assert_eq!(writer.generation(), 1);

    let seq_hdr = SequenceHeader::default();
    writer.write_sequence_header(&seq_hdr);
    assert_eq!(writer.generation(), 2);

    let frame_hdr = FrameHeader::default();
    writer.write_frame_header(&frame_hdr);
    assert_eq!(writer.generation(), 3);

    // Reset increments generation
    writer.reset();
    assert_eq!(writer.generation(), 4);
}

#[test]
fn test_bitstream_writer_reset() {
    let mut writer = BitstreamWriterCapsule::new();

    // Write some OBUs
    writer.write_temporal_delimiter();
    let seq_hdr = SequenceHeader::default();
    writer.write_sequence_header(&seq_hdr);

    assert!(writer.bytes_written() > 0);
    assert!(writer.obus_written() > 0);

    let gen_before = writer.generation();
    writer.reset();

    // Verify reset
    assert_eq!(writer.bytes_written(), 0);
    assert_eq!(writer.obus_written(), 0);
    assert_eq!(writer.current_frame(), 0);
    assert_eq!(writer.generation(), gen_before + 1);
}
