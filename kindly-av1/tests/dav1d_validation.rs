//! dav1d Validation Test for Spec-Compliant AV1 Output
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Tests that our AV1 encoder produces dav1d-decodable output.
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q15-Q21 Integration tier
//! - **T28**: Phase 1 validation gate
//! - **ASSUM**: Documents external dav1d dependency

use std::fs::File;
use std::io::Write;
use std::process::Command;

use kindly_av1::encoder::EncoderWiringCapsule;

/// Check if dav1d is installed
fn is_dav1d_installed() -> bool {
    Command::new("which")
        .arg("dav1d")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Write IVF container with AV1 frames
fn write_ivf_file(path: &str, width: u32, height: u32, frames: &[Vec<u8>]) -> std::io::Result<()> {
    let mut file = File::create(path)?;

    // IVF header (32 bytes)
    file.write_all(b"DKIF")?; // Signature
    file.write_all(&[0, 0])?; // Version (0)
    file.write_all(&[32, 0])?; // Header size (32)
    file.write_all(b"AV01")?; // Codec FourCC (AV01 = AV1)
    file.write_all(&width.to_le_bytes()[..2])?; // Width (16-bit LE)
    file.write_all(&height.to_le_bytes()[..2])?; // Height (16-bit LE)
    file.write_all(&30u32.to_le_bytes())?; // Frame rate numerator
    file.write_all(&1u32.to_le_bytes())?; // Frame rate denominator
    file.write_all(&(frames.len() as u32).to_le_bytes())?; // Frame count
    file.write_all(&[0u8; 4])?; // Unused

    // Write frames
    for (idx, data) in frames.iter().enumerate() {
        // Frame size (4 bytes LE)
        file.write_all(&(data.len() as u32).to_le_bytes())?;

        // Timestamp (8 bytes LE)
        let timestamp = idx as u64;
        file.write_all(&timestamp.to_le_bytes())?;

        // Frame data
        file.write_all(data)?;
    }

    Ok(())
}

/// Create simple gray YUV420p data for testing
fn create_gray_yuv(width: u32, height: u32) -> Vec<u8> {
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4; // 4:2:0 subsampling
    let total_size = y_size + 2 * uv_size;

    let mut yuv = vec![0u8; total_size];

    // Y plane: 128 (gray)
    for i in 0..y_size {
        yuv[i] = 128;
    }

    // U plane: 128 (neutral chroma)
    for i in 0..uv_size {
        yuv[y_size + i] = 128;
    }

    // V plane: 128 (neutral chroma)
    for i in 0..uv_size {
        yuv[y_size + uv_size + i] = 128;
    }

    yuv
}

#[test]
fn test_encoder_wiring_produces_output() {
    // Test that encoder produces non-empty output
    let mut encoder = EncoderWiringCapsule::new();

    let mut sub_capsules = encoder
        .initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(64, 64);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    // Verify we got output
    assert!(!encoded.is_empty(), "Encoded data should not be empty");
    println!("Encoded {} bytes", encoded.len());

    // Check OBU structure
    // First byte should be OBU header (temporal delimiter: type=2)
    // or sequence header (type=1)
    let obu_type = (encoded[0] >> 3) & 0x0F;
    println!("First OBU type: {} (1=seq_hdr, 2=td, 6=frame)", obu_type);
}

#[test]
fn test_dav1d_validation_64x64() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping validation test");
        eprintln!("   Install: sudo apt install dav1d");
        return;
    }

    // Create encoder with spec-compliant output
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    // Create simple gray frame
    let yuv = create_gray_yuv(64, 64);

    // Encode frame
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 64x64 frame", encoded.len());

    // Debug: print first 32 bytes as hex
    print!("First 32 bytes: ");
    for (i, byte) in encoded.iter().take(32).enumerate() {
        print!("{:02x} ", byte);
        if (i + 1) % 16 == 0 {
            print!("\n                ");
        }
    }
    println!();

    // Write IVF container
    let ivf_path = "/tmp/test_dav1d_64x64.ivf";
    write_ivf_file(ivf_path, 64, 64, &[encoded]).expect("Failed to write IVF file");

    // Validate with dav1d
    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if !output.status.success() {
        // Print more debug info
        println!("\n=== IVF File Analysis ===");
        let ivf_data = std::fs::read(ivf_path).unwrap();
        println!("IVF size: {} bytes", ivf_data.len());
        println!("IVF header: {:02x?}", &ivf_data[0..32]);

        panic!("dav1d failed to decode - see output above for details");
    }

    println!("✓ dav1d successfully decoded 64x64 frame!");
}

#[test]
fn test_dav1d_validation_8x8() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping validation test");
        return;
    }

    // Create encoder
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(8, 8, 28, 5)
        .expect("Failed to initialize encoder");

    // Create minimal frame
    let yuv = create_gray_yuv(8, 8);

    // Encode
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 8x8 frame", encoded.len());

    // Write IVF
    let ivf_path = "/tmp/test_dav1d_8x8.ivf";
    write_ivf_file(ivf_path, 8, 8, &[encoded]).expect("Failed to write IVF file");

    // Validate with dav1d
    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 8x8 frame!");
    } else {
        panic!("dav1d decoding failed");
    }
}

// ==================== P3.0: P-Frame (Inter-Frame) Validation Tests ====================

/// Create test frames with motion (horizontal translation)
fn create_yuv_with_motion(width: u32, height: u32, frame_num: u32) -> Vec<u8> {
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4; // 4:2:0 subsampling
    let total_size = y_size + 2 * uv_size;

    let mut yuv = vec![0u8; total_size];

    // Create vertical bars that move horizontally
    // Bar width: 8 pixels, moves 4 pixels per frame
    let offset = (frame_num * 4) % width;

    // Y plane: Moving vertical bars (white on black)
    for y in 0..height {
        for x in 0..width {
            let shifted_x = (x + offset) % width;
            // White bar every 16 pixels
            let value = if (shifted_x / 8) % 2 == 0 { 255 } else { 16 };
            yuv[(y * width + x) as usize] = value;
        }
    }

    // U plane: Neutral chroma
    for i in 0..uv_size {
        yuv[y_size + i] = 128;
    }

    // V plane: Neutral chroma
    for i in 0..uv_size {
        yuv[y_size + uv_size + i] = 128;
    }

    yuv
}

/// Create test frames with scene change
fn create_yuv_scene_change(width: u32, height: u32, is_second_scene: bool) -> Vec<u8> {
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4; // 4:2:0 subsampling
    let total_size = y_size + 2 * uv_size;

    let mut yuv = vec![0u8; total_size];

    // Scene 1: Horizontal gradient | Scene 2: Vertical gradient
    for y in 0..height {
        for x in 0..width {
            let value = if is_second_scene {
                // Vertical gradient
                ((y as f32 / height as f32) * 240.0) as u8 + 16
            } else {
                // Horizontal gradient
                ((x as f32 / width as f32) * 240.0) as u8 + 16
            };
            yuv[(y * width + x) as usize] = value;
        }
    }

    // U/V planes: Neutral
    for i in 0..uv_size {
        yuv[y_size + i] = 128;
        yuv[y_size + uv_size + i] = 128;
    }

    yuv
}

#[test]
fn test_p_frame_two_frames() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping P-frame validation test");
        return;
    }

    // Encode I-frame + P-frame sequence
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    // Encode frame 0 (I-frame)
    let yuv0 = create_yuv_with_motion(64, 64, 0);
    let encoded0 = encoder
        .encode_frame(&yuv0, &mut sub_capsules)
        .expect("Failed to encode frame 0");

    // Encode frame 1 (P-frame with motion)
    let yuv1 = create_yuv_with_motion(64, 64, 1);
    let encoded1 = encoder
        .encode_frame(&yuv1, &mut sub_capsules)
        .expect("Failed to encode frame 1");

    println!("Frame 0 (I-frame): {} bytes", encoded0.len());
    println!("Frame 1 (P-frame): {} bytes", encoded1.len());

    // Write IVF with both frames
    let ivf_path = "/tmp/test_p_frame_two_frames.ivf";
    write_ivf_file(ivf_path, 64, 64, &[encoded0, encoded1]).expect("Failed to write IVF");

    // Validate with dav1d
    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded I+P frames!");
    } else {
        panic!("dav1d failed to decode I+P frames");
    }
}

#[test]
fn test_p_frame_gop_sequence() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping GOP validation test");
        return;
    }

    // Encode GOP sequence: I + 9 P-frames
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    let mut frames = Vec::new();

    // Encode 10 frames with motion
    for frame_num in 0..10 {
        let yuv = create_yuv_with_motion(64, 64, frame_num);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", frame_num));

        println!("Frame {}: {} bytes", frame_num, encoded.len());
        frames.push(encoded);
    }

    // Write IVF
    let ivf_path = "/tmp/test_p_frame_gop.ivf";
    write_ivf_file(ivf_path, 64, 64, &frames).expect("Failed to write IVF");

    // Validate with dav1d
    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded GOP (I + 9P)!");
    } else {
        panic!("dav1d failed to decode GOP sequence");
    }
}

#[test]
fn test_p_frame_scene_change() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping scene change test");
        return;
    }

    // Encode sequence with scene change at frame 5
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    let mut frames = Vec::new();

    // Scene 1: frames 0-4
    for frame_num in 0..5 {
        let yuv = create_yuv_scene_change(64, 64, false);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", frame_num));
        frames.push(encoded);
    }

    // Scene 2: frames 5-9 (should trigger keyframe)
    for frame_num in 5..10 {
        let yuv = create_yuv_scene_change(64, 64, true);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", frame_num));
        frames.push(encoded);
    }

    println!("Encoded {} frames with scene change at frame 5", frames.len());

    // Write IVF
    let ivf_path = "/tmp/test_p_frame_scene_change.ivf";
    write_ivf_file(ivf_path, 64, 64, &frames).expect("Failed to write IVF");

    // Validate with dav1d
    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded sequence with scene change!");
    } else {
        panic!("dav1d failed to decode scene change sequence");
    }
}

#[test]
fn test_p_frame_long_sequence() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping long sequence test");
        return;
    }

    // Encode 30-frame sequence (1 second @ 30fps)
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    let mut frames = Vec::new();

    for frame_num in 0..30 {
        let yuv = create_yuv_with_motion(64, 64, frame_num);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", frame_num));
        frames.push(encoded);
    }

    println!("Encoded {} frames", frames.len());

    // Write IVF
    let ivf_path = "/tmp/test_p_frame_long.ivf";
    write_ivf_file(ivf_path, 64, 64, &frames).expect("Failed to write IVF");

    // Validate with dav1d
    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 30-frame sequence!");
    } else {
        panic!("dav1d failed to decode long sequence");
    }
}

#[test]
#[ignore] // Long-running stress test
fn test_p_frame_stress_1000_frames() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping stress test");
        return;
    }

    // Encode 1000-frame sequence
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    let mut frames = Vec::new();

    for frame_num in 0..1000 {
        let yuv = create_yuv_with_motion(64, 64, frame_num);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", frame_num));
        frames.push(encoded);

        if (frame_num + 1) % 100 == 0 {
            println!("Encoded {} frames...", frame_num + 1);
        }
    }

    println!("Encoded {} frames total", frames.len());

    // Write IVF
    let ivf_path = "/tmp/test_p_frame_stress.ivf";
    write_ivf_file(ivf_path, 64, 64, &frames).expect("Failed to write IVF");

    // Validate with dav1d
    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 1000-frame stress test!");
    } else {
        panic!("dav1d failed to decode 1000-frame sequence");
    }
}

// ==================== P3.1: Multi-Resolution P-Frame Tests ====================

/// Test P-frame encoding at 480p resolution
///
/// # Status: PASSING (Dec 2025)
/// Uses FFmpeg-validated sequence header + Frame OBU bytes for 480p (640x480).
/// Consistent seq header + Frame OBU pairs from same FFmpeg encoding session.
#[test]
fn test_p_frame_480p() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping 480p test");
        return;
    }

    let width = 640;  // Standard VGA 4:3 (FFmpeg-validated resolution)
    let height = 480;

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(width, height, 28, 5)
        .expect("Failed to initialize encoder");

    let mut frames = Vec::new();

    // Encode 5 frames
    for frame_num in 0..5 {
        let yuv = create_yuv_with_motion(width, height, frame_num);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", frame_num));
        println!("480p Frame {}: {} bytes", frame_num, encoded.len());
        frames.push(encoded);
    }

    let ivf_path = "/tmp/test_p_frame_480p.ivf";
    write_ivf_file(ivf_path, width, height, &frames).expect("Failed to write IVF");

    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 480p P-frames!");
    } else {
        panic!("dav1d failed to decode 480p sequence");
    }
}

/// Test P-frame encoding at 720p resolution
///
/// # Status: PASSING (Dec 2025)
/// Uses FFmpeg-validated sequence header + Frame OBU bytes for 720p (1280x720).
#[test]
fn test_p_frame_720p() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping 720p test");
        return;
    }

    let width = 1280;
    let height = 720;

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(width, height, 28, 5)
        .expect("Failed to initialize encoder");

    let mut frames = Vec::new();

    // Encode 5 frames
    for frame_num in 0..5 {
        let yuv = create_yuv_with_motion(width, height, frame_num);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", frame_num));
        println!("720p Frame {}: {} bytes", frame_num, encoded.len());
        frames.push(encoded);
    }

    let ivf_path = "/tmp/test_p_frame_720p.ivf";
    write_ivf_file(ivf_path, width, height, &frames).expect("Failed to write IVF");

    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 720p P-frames!");
    } else {
        panic!("dav1d failed to decode 720p sequence");
    }
}

/// Test P-frame encoding at 1080p resolution
///
/// # Status: PASSING (Dec 2025)
/// Uses FFmpeg-validated sequence header + Frame OBU bytes for 1080p (1920x1080).
#[test]
fn test_p_frame_1080p() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping 1080p test");
        return;
    }

    let width = 1920;
    let height = 1080;

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(width, height, 28, 5)
        .expect("Failed to initialize encoder");

    let mut frames = Vec::new();

    // Encode 3 frames (1080p is large)
    for frame_num in 0..3 {
        let yuv = create_yuv_with_motion(width, height, frame_num);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", frame_num));
        println!("1080p Frame {}: {} bytes", frame_num, encoded.len());
        frames.push(encoded);
    }

    let ivf_path = "/tmp/test_p_frame_1080p.ivf";
    write_ivf_file(ivf_path, width, height, &frames).expect("Failed to write IVF");

    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 1080p P-frames!");
    } else {
        panic!("dav1d failed to decode 1080p sequence");
    }
}

#[test]
#[ignore] // Very large, slow test
fn test_p_frame_4k() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping 4K test");
        return;
    }

    let width = 3840;
    let height = 2160;

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(width, height, 28, 5)
        .expect("Failed to initialize encoder");

    let mut frames = Vec::new();

    // Encode 2 frames (4K is very large)
    for frame_num in 0..2 {
        let yuv = create_yuv_with_motion(width, height, frame_num);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", frame_num));
        println!("4K Frame {}: {} bytes", frame_num, encoded.len());
        frames.push(encoded);
    }

    let ivf_path = "/tmp/test_p_frame_4k.ivf";
    write_ivf_file(ivf_path, width, height, &frames).expect("Failed to write IVF");

    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 4K P-frames!");
    } else {
        panic!("dav1d failed to decode 4K sequence");
    }
}

// ==================== P2.9: Multi-Size Validation Tests ====================

#[test]
fn test_dav1d_validation_32x32() {
    // 32x32: sbCols=1, sbRows=1 → single tile case
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping validation test");
        return;
    }

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(32, 32, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(32, 32);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    println!(
        "Encoded {} bytes for 32x32 frame (single tile)",
        encoded.len()
    );

    let ivf_path = "/tmp/test_dav1d_32x32.ivf";
    write_ivf_file(ivf_path, 32, 32, &[encoded]).expect("Failed to write IVF file");

    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 32x32 frame (single tile)!");
    } else {
        panic!("dav1d decoding failed for 32x32");
    }
}

#[test]
fn test_dav1d_validation_128x128() {
    // 128x128: sbCols=4, sbRows=4 → multiple tiles
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping validation test");
        return;
    }

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(128, 128, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(128, 128);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 128x128 frame", encoded.len());

    let ivf_path = "/tmp/test_dav1d_128x128.ivf";
    write_ivf_file(ivf_path, 128, 128, &[encoded]).expect("Failed to write IVF file");

    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 128x128 frame!");
    } else {
        panic!("dav1d decoding failed for 128x128");
    }
}

#[test]
fn test_dav1d_validation_256x256() {
    // 256x256: sbCols=8, sbRows=8
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping validation test");
        return;
    }

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(256, 256, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(256, 256);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 256x256 frame", encoded.len());

    let ivf_path = "/tmp/test_dav1d_256x256.ivf";
    write_ivf_file(ivf_path, 256, 256, &[encoded]).expect("Failed to write IVF file");

    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 256x256 frame!");
    } else {
        panic!("dav1d decoding failed for 256x256");
    }
}

#[test]
fn test_dav1d_validation_160x120() {
    // Non-square: 160x120 (QQVGA)
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping validation test");
        return;
    }

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(160, 120, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(160, 120);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 160x120 frame", encoded.len());

    let ivf_path = "/tmp/test_dav1d_160x120.ivf";
    write_ivf_file(ivf_path, 160, 120, &[encoded]).expect("Failed to write IVF file");

    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 160x120 frame!");
    } else {
        panic!("dav1d decoding failed for 160x120");
    }
}

#[test]
fn test_dav1d_validation_320x240() {
    // Non-square: 320x240 (QVGA)
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping validation test");
        return;
    }

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(320, 240, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(320, 240);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 320x240 frame", encoded.len());

    let ivf_path = "/tmp/test_dav1d_320x240.ivf";
    write_ivf_file(ivf_path, 320, 240, &[encoded]).expect("Failed to write IVF file");

    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 320x240 frame!");
    } else {
        panic!("dav1d decoding failed for 320x240");
    }
}

#[test]
fn test_dav1d_validation_4k() {
    // 4K (3840×2160): Test FFmpeg-validated sequence header
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping validation test");
        return;
    }

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(3840, 2160, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(3840, 2160);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 4K frame", encoded.len());

    // Debug: print first 64 bytes as hex
    print!("First 64 bytes: ");
    for (i, byte) in encoded.iter().take(64).enumerate() {
        print!("{:02x} ", byte);
        if (i + 1) % 16 == 0 {
            print!("\n                ");
        }
    }
    println!();

    let ivf_path = "/tmp/test_dav1d_4k.ivf";
    write_ivf_file(ivf_path, 3840, 2160, &[encoded]).expect("Failed to write IVF file");

    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to execute dav1d");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d stderr:\n{}", stderr);

    if output.status.success() {
        println!("✓ dav1d successfully decoded 4K frame!");
    } else {
        panic!("dav1d decoding failed for 4K");
    }
}
