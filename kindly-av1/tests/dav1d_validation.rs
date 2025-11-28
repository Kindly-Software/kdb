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
fn write_ivf_file(
    path: &str,
    width: u32,
    height: u32,
    frames: &[Vec<u8>],
) -> std::io::Result<()> {
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

    let sub_capsules = encoder.initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(64, 64);
    let encoded = encoder.encode_frame(&yuv, &sub_capsules)
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
    let sub_capsules = encoder.initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    // Create simple gray frame
    let yuv = create_gray_yuv(64, 64);

    // Encode frame
    let encoded = encoder.encode_frame(&yuv, &sub_capsules)
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
    write_ivf_file(ivf_path, 64, 64, &[encoded])
        .expect("Failed to write IVF file");

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
    let sub_capsules = encoder.initialize(8, 8, 28, 5)
        .expect("Failed to initialize encoder");

    // Create minimal frame
    let yuv = create_gray_yuv(8, 8);

    // Encode
    let encoded = encoder.encode_frame(&yuv, &sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 8x8 frame", encoded.len());

    // Write IVF
    let ivf_path = "/tmp/test_dav1d_8x8.ivf";
    write_ivf_file(ivf_path, 8, 8, &[encoded])
        .expect("Failed to write IVF file");

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

// ==================== P2.9: Multi-Size Validation Tests ====================

#[test]
fn test_dav1d_validation_32x32() {
    // 32x32: sbCols=1, sbRows=1 → single tile case
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping validation test");
        return;
    }

    let mut encoder = EncoderWiringCapsule::new();
    let sub_capsules = encoder.initialize(32, 32, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(32, 32);
    let encoded = encoder.encode_frame(&yuv, &sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 32x32 frame (single tile)", encoded.len());

    let ivf_path = "/tmp/test_dav1d_32x32.ivf";
    write_ivf_file(ivf_path, 32, 32, &[encoded])
        .expect("Failed to write IVF file");

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
    let sub_capsules = encoder.initialize(128, 128, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(128, 128);
    let encoded = encoder.encode_frame(&yuv, &sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 128x128 frame", encoded.len());

    let ivf_path = "/tmp/test_dav1d_128x128.ivf";
    write_ivf_file(ivf_path, 128, 128, &[encoded])
        .expect("Failed to write IVF file");

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
    let sub_capsules = encoder.initialize(256, 256, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(256, 256);
    let encoded = encoder.encode_frame(&yuv, &sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 256x256 frame", encoded.len());

    let ivf_path = "/tmp/test_dav1d_256x256.ivf";
    write_ivf_file(ivf_path, 256, 256, &[encoded])
        .expect("Failed to write IVF file");

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
    let sub_capsules = encoder.initialize(160, 120, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(160, 120);
    let encoded = encoder.encode_frame(&yuv, &sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 160x120 frame", encoded.len());

    let ivf_path = "/tmp/test_dav1d_160x120.ivf";
    write_ivf_file(ivf_path, 160, 120, &[encoded])
        .expect("Failed to write IVF file");

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
    let sub_capsules = encoder.initialize(320, 240, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_gray_yuv(320, 240);
    let encoded = encoder.encode_frame(&yuv, &sub_capsules)
        .expect("Failed to encode frame");

    println!("Encoded {} bytes for 320x240 frame", encoded.len());

    let ivf_path = "/tmp/test_dav1d_320x240.ivf";
    write_ivf_file(ivf_path, 320, 240, &[encoded])
        .expect("Failed to write IVF file");

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
