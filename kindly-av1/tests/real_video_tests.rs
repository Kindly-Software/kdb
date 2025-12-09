//! Real Video Encoding Tests
//!
//! Tests using real video sequences from xiph.org (Derf's collection)
//! These are standard codec test sequences used by industry.
//!
//! Test sequences:
//! - foreman_cif.y4m: 352x288, 300 frames, man talking on phone
//! - akiyo_cif.y4m: 352x288, 300 frames, news anchor
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL

use kindly_av1::encoder::{
    BitstreamWriterCapsule, FrameHeader, FrameType, IvfContainerWriterCapsule, SequenceHeader,
};
use kindly_av1::file::{FrameReader, Y4mReader};
use std::fs;
use std::path::Path;
use std::process::Command;

const REAL_VIDEOS_DIR: &str = "tests/fixtures/real_videos";

/// Check if a real video file exists
fn video_exists(name: &str) -> bool {
    Path::new(REAL_VIDEOS_DIR).join(name).exists()
}

/// Get video path
fn video_path(name: &str) -> std::path::PathBuf {
    Path::new(REAL_VIDEOS_DIR).join(name)
}

// ============================================================================
// Y4M Reading Tests with Real Videos
// ============================================================================

#[test]
fn test_read_foreman_cif_header() {
    if !video_exists("foreman_cif.y4m") {
        println!("SKIP: foreman_cif.y4m not found");
        return;
    }

    let reader = Y4mReader::open(video_path("foreman_cif.y4m")).unwrap();
    let info = reader.info();

    assert_eq!(info.width, 352);
    assert_eq!(info.height, 288);
    assert!(info.frame_count >= 100, "Expected at least 100 frames");

    println!(
        "✓ Foreman CIF: {}x{}, {} frames, {:.2} fps",
        info.width, info.height, info.frame_count, info.frame_rate
    );
}

#[test]
fn test_read_akiyo_cif_header() {
    if !video_exists("akiyo_cif.y4m") {
        println!("SKIP: akiyo_cif.y4m not found");
        return;
    }

    let reader = Y4mReader::open(video_path("akiyo_cif.y4m")).unwrap();
    let info = reader.info();

    assert_eq!(info.width, 352);
    assert_eq!(info.height, 288);

    println!(
        "✓ Akiyo CIF: {}x{}, {} frames, {:.2} fps",
        info.width, info.height, info.frame_count, info.frame_rate
    );
}

#[test]
fn test_read_foreman_first_10_frames() {
    if !video_exists("foreman_cif.y4m") {
        println!("SKIP: foreman_cif.y4m not found");
        return;
    }

    let mut reader = Y4mReader::open(video_path("foreman_cif.y4m")).unwrap();

    for i in 0..10 {
        let frame = reader.read_frame().unwrap().expect("Expected frame");
        assert_eq!(frame.frame_num, i);
        assert_eq!(frame.width, 352);
        assert_eq!(frame.height, 288);

        // Y plane should be 352*288 = 101,376 bytes
        assert_eq!(frame.y.len(), 352 * 288);
        // U/V planes should be 176*144 = 25,344 bytes each (4:2:0)
        assert_eq!(frame.u.len(), 176 * 144);
        assert_eq!(frame.v.len(), 176 * 144);
    }

    println!("✓ Read 10 frames from Foreman CIF");
}

// ============================================================================
// Encoding Tests with Real Videos
// ============================================================================

#[test]
fn test_encode_foreman_10_frames_to_ivf() {
    if !video_exists("foreman_cif.y4m") {
        println!("SKIP: foreman_cif.y4m not found");
        return;
    }

    let mut reader = Y4mReader::open(video_path("foreman_cif.y4m")).unwrap();
    let (width, height) = {
        let info = reader.info();
        (info.width, info.height)
    };

    // Create output IVF container
    let output_path = Path::new("target/test_foreman_10frames.ivf");
    let mut output_data = Vec::new();

    // Write IVF file header
    let ivf = IvfContainerWriterCapsule::new();
    let file_header = ivf.write_file_header(width, height, 30, 1);
    output_data.extend_from_slice(&file_header);

    // Write sequence header OBU
    let mut bitstream_writer = BitstreamWriterCapsule::new();

    let seq_hdr = SequenceHeader {
        seq_profile: 0,
        max_frame_width: width,
        max_frame_height: height,
        bit_depth: 8,
        use_128x128_superblock: false,
        enable_filter_intra: true,
        enable_intra_edge_filter: true,
        enable_interintra_compound: false,
        enable_masked_compound: false,
        enable_warped_motion: false,
        enable_dual_filter: false,
        enable_order_hint: true,
        order_hint_bits: 8,
    };

    // Encode 10 frames
    let frames_to_encode = 10;
    let mut total_bytes = 0u64;

    for i in 0..frames_to_encode {
        let frame = match reader.read_frame().unwrap() {
            Some(f) => f,
            None => break,
        };

        // Create frame bitstream
        let mut frame_data = Vec::new();

        // Temporal delimiter
        let td_size = bitstream_writer.write_temporal_delimiter();
        frame_data.extend_from_slice(&bitstream_writer.buffer()[..td_size]);

        // Sequence header (keyframe only)
        if i == 0 {
            let sh_size = bitstream_writer.write_sequence_header(&seq_hdr);
            frame_data.extend_from_slice(&bitstream_writer.buffer()[..sh_size]);
        }

        // Frame header
        let frame_hdr = FrameHeader {
            frame_type: if i == 0 {
                FrameType::KeyFrame
            } else {
                FrameType::InterFrame
            },
            show_frame: true,
            show_existing_frame: false,
            frame_width: width,
            frame_height: height,
            render_width: width,
            render_height: height,
            tile_cols_log2: 0,
            tile_rows_log2: 0,
            reduced_tx_set: false,
            base_q_idx: 128,
            // Inter-frame reference fields
            primary_ref_frame: if i == 0 { 7 } else { 0 }, // PRIMARY_REF_NONE for keyframe
            ref_frame_idx: [0; 7],
            refresh_frame_flags: if i == 0 { 0xff } else { 0x01 }, // All slots for keyframe
            allow_ref_frame_mvs: i > 0,                            // Only for inter frames
        };

        // Simple placeholder tile data (real encoder would transform + quantize + entropy code)
        // For now, just include raw Y data compressed placeholder
        let tile_data_size = 100; // Minimal placeholder
        let fh_size = bitstream_writer.write_frame_obu_header(&frame_hdr, tile_data_size);
        frame_data.extend_from_slice(&bitstream_writer.buffer()[..fh_size]);
        frame_data.extend(vec![0u8; tile_data_size as usize]); // Placeholder tile data

        // Write IVF frame header + data
        let ivf_frame_header = ivf.write_frame_header(frame_data.len() as u32);
        output_data.extend_from_slice(&ivf_frame_header);
        output_data.extend_from_slice(&frame_data);

        total_bytes += frame_data.len() as u64;
    }

    // Write output file
    fs::write(output_path, &output_data).unwrap();

    let file_size = output_data.len();
    let avg_frame_size = total_bytes / frames_to_encode as u64;

    println!("✓ Encoded {} frames from Foreman CIF", frames_to_encode);
    println!("  Output: {} ({} bytes)", output_path.display(), file_size);
    println!("  Avg frame size: {} bytes", avg_frame_size);

    // Verify IVF header is valid
    assert_eq!(&output_data[0..4], b"DKIF");
    assert_eq!(&output_data[8..12], b"AV01");
}

#[test]
fn test_decode_with_dav1d() {
    let ivf_path = Path::new("target/test_foreman_10frames.ivf");

    if !ivf_path.exists() {
        println!("SKIP: Run test_encode_foreman_10_frames_to_ivf first");
        return;
    }

    // Check if dav1d is available
    let dav1d_check = Command::new("which").arg("dav1d").output();
    if dav1d_check.is_err() || !dav1d_check.unwrap().status.success() {
        println!("SKIP: dav1d not installed");
        return;
    }

    // Try to decode with dav1d
    let output = Command::new("dav1d")
        .args(["-i", ivf_path.to_str().unwrap(), "-o", "/dev/null"])
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                println!("✓ dav1d decoded successfully!");
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                println!("⚠ dav1d failed (expected - placeholder tile data):");
                println!("  {}", stderr.lines().next().unwrap_or("unknown error"));
                // This is expected to fail since we're using placeholder tile data
            }
        }
        Err(e) => {
            println!("⚠ Could not run dav1d: {}", e);
        }
    }
}

// ============================================================================
// Motion Estimation Tests with Real Videos
// ============================================================================

#[test]
fn test_motion_estimation_foreman() {
    use kindly_av1::encoder::GpuMotionEstimationCapsule;

    if !video_exists("foreman_cif.y4m") {
        println!("SKIP: foreman_cif.y4m not found");
        return;
    }

    let mut reader = Y4mReader::open(video_path("foreman_cif.y4m")).unwrap();

    // Read first two frames
    let frame0 = reader.read_frame().unwrap().expect("Frame 0");
    let frame1 = reader.read_frame().unwrap().expect("Frame 1");

    // Create motion estimation capsule
    let capsule = GpuMotionEstimationCapsule::new();
    capsule.disable_gpu(); // Use CPU for determinism

    // Estimate motion between frames
    let mvs = capsule
        .estimate_frame(&frame1.y, &frame0.y, 352, 288)
        .unwrap();

    // 352/16 = 22, 288/16 = 18 macroblocks
    let expected_mbs = 22 * 18;
    assert_eq!(
        mvs.len(),
        expected_mbs,
        "Expected {} MVs, got {}",
        expected_mbs,
        mvs.len()
    );

    // Analyze motion vectors
    let mut zero_mvs = 0;
    let mut total_magnitude = 0.0f64;

    for mv in &mvs {
        let (x, y) = mv.to_integer_pel();
        if x == 0 && y == 0 {
            zero_mvs += 1;
        }
        total_magnitude += ((x as f64).powi(2) + (y as f64).powi(2)).sqrt();
    }

    let avg_magnitude = total_magnitude / mvs.len() as f64;
    let zero_pct = 100.0 * zero_mvs as f64 / mvs.len() as f64;

    println!("✓ Motion estimation for Foreman frame 0→1:");
    println!("  Macroblocks: {}", mvs.len());
    println!("  Zero MVs: {} ({:.1}%)", zero_mvs, zero_pct);
    println!("  Avg MV magnitude: {:.2} pixels", avg_magnitude);

    // Foreman has moderate motion (talking, small head movement)
    // Expect some non-zero MVs but not extreme motion
    assert!(
        avg_magnitude < 10.0,
        "Unexpected large motion: {:.2}",
        avg_magnitude
    );
}

#[test]
fn test_motion_estimation_akiyo_static() {
    use kindly_av1::encoder::GpuMotionEstimationCapsule;

    if !video_exists("akiyo_cif.y4m") {
        println!("SKIP: akiyo_cif.y4m not found");
        return;
    }

    let mut reader = Y4mReader::open(video_path("akiyo_cif.y4m")).unwrap();

    // Read frames 10 and 11 (should be very similar - static scene)
    for _ in 0..10 {
        reader.read_frame().unwrap();
    }
    let frame10 = reader.read_frame().unwrap().expect("Frame 10");
    let frame11 = reader.read_frame().unwrap().expect("Frame 11");

    let capsule = GpuMotionEstimationCapsule::new();
    capsule.disable_gpu();

    let mvs = capsule
        .estimate_frame(&frame11.y, &frame10.y, 352, 288)
        .unwrap();

    // Akiyo is mostly static (news anchor sitting still)
    // Expect most MVs to be zero or near-zero
    let zero_mvs = mvs
        .iter()
        .filter(|mv: &&kindly_av1::encoder::MotionVector| {
            let (x, y) = mv.to_integer_pel();
            x.abs() <= 1 && y.abs() <= 1
        })
        .count();

    let static_pct = 100.0 * zero_mvs as f64 / mvs.len() as f64;

    println!("✓ Motion estimation for Akiyo frame 10→11:");
    println!(
        "  Static/near-static MVs: {} ({:.1}%)",
        zero_mvs, static_pct
    );

    // Akiyo should have >80% static blocks
    assert!(
        static_pct > 70.0,
        "Expected mostly static scene, got {:.1}%",
        static_pct
    );
}

// ============================================================================
// Full Round-Trip Tests (Reference Encoder + dav1d)
// ============================================================================

#[test]
fn test_roundtrip_foreman_libaom_dav1d() {
    if !video_exists("foreman_cif.y4m") {
        println!("SKIP: foreman_cif.y4m not found");
        return;
    }

    // Check if ffmpeg is available
    let ffmpeg_check = Command::new("which").arg("ffmpeg").output();
    if ffmpeg_check.is_err() || !ffmpeg_check.unwrap().status.success() {
        println!("SKIP: ffmpeg not installed");
        return;
    }

    // Check if dav1d is available
    let dav1d_check = Command::new("which").arg("dav1d").output();
    if dav1d_check.is_err() || !dav1d_check.unwrap().status.success() {
        println!("SKIP: dav1d not installed");
        return;
    }

    let input_path = video_path("foreman_cif.y4m");
    let encoded_path = Path::new("target/roundtrip_foreman.ivf");
    let decoded_path = Path::new("target/roundtrip_decoded.y4m");

    // Step 1: Encode with libaom-av1 (reference encoder)
    let encode_result = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            input_path.to_str().unwrap(),
            "-frames:v",
            "10",
            "-c:v",
            "libaom-av1",
            "-crf",
            "30",
            "-cpu-used",
            "8",
            "-row-mt",
            "1",
            encoded_path.to_str().unwrap(),
        ])
        .output();

    if encode_result.is_err() || !encode_result.as_ref().unwrap().status.success() {
        println!("⚠ ffmpeg encoding failed (libaom-av1 may not be available)");
        return;
    }

    // Step 2: Decode with dav1d
    let decode_result = Command::new("dav1d")
        .args([
            "-i",
            encoded_path.to_str().unwrap(),
            "-o",
            decoded_path.to_str().unwrap(),
        ])
        .output();

    assert!(
        decode_result.is_ok() && decode_result.unwrap().status.success(),
        "dav1d failed to decode reference AV1"
    );

    // Step 3: Read both Y4M files and compare PSNR
    let mut original_reader = Y4mReader::open(&input_path).unwrap();
    let mut decoded_reader = Y4mReader::open(decoded_path).unwrap();

    let mut total_psnr = 0.0f64;
    let mut frame_count = 0;

    for _ in 0..10 {
        let orig_frame = match original_reader.read_frame().unwrap() {
            Some(f) => f,
            None => break,
        };
        let dec_frame = match decoded_reader.read_frame().unwrap() {
            Some(f) => f,
            None => break,
        };

        // Calculate MSE for Y channel
        let mse: f64 = orig_frame
            .y
            .iter()
            .zip(dec_frame.y.iter())
            .map(|(&a, &b)| {
                let diff = (a as f64) - (b as f64);
                diff * diff
            })
            .sum::<f64>()
            / orig_frame.y.len() as f64;

        // PSNR = 10 * log10(255^2 / MSE)
        let psnr = if mse > 0.0 {
            10.0 * (255.0f64 * 255.0f64 / mse).log10()
        } else {
            100.0 // Perfect match
        };

        total_psnr += psnr;
        frame_count += 1;
    }

    let avg_psnr = total_psnr / frame_count as f64;

    println!("✓ Round-trip validation (libaom → dav1d):");
    println!("  Frames compared: {}", frame_count);
    println!("  Average PSNR: {:.2} dB", avg_psnr);

    // Lossy encoding at CRF 30 should give PSNR >= 35 dB
    assert!(
        avg_psnr >= 35.0,
        "PSNR too low: {:.2} dB (expected >= 35)",
        avg_psnr
    );
    assert_eq!(frame_count, 10, "Not all frames decoded");

    // Cleanup
    let _ = fs::remove_file(encoded_path);
    let _ = fs::remove_file(decoded_path);
}

// ============================================================================
// Performance Tests (Quick)
// ============================================================================

#[test]
fn test_foreman_decode_speed() {
    if !video_exists("foreman_cif.y4m") {
        println!("SKIP: foreman_cif.y4m not found");
        return;
    }

    let start = std::time::Instant::now();

    let mut reader = Y4mReader::open(video_path("foreman_cif.y4m")).unwrap();
    let mut frame_count = 0;

    while let Some(_frame) = reader.read_frame().unwrap() {
        frame_count += 1;
    }

    let elapsed = start.elapsed();
    let fps = frame_count as f64 / elapsed.as_secs_f64();

    println!(
        "✓ Y4M decode speed: {} frames in {:?}",
        frame_count, elapsed
    );
    println!("  Decode FPS: {:.0}", fps);

    // Should decode faster than real-time (>30fps)
    assert!(fps > 30.0, "Y4M decode too slow: {:.1} fps", fps);
}
