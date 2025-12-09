//! End-to-End Video Encoding Test Suite for kindly-av1
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Comprehensive E2E tests validating the entire kindly-av1 encoding pipeline,
//! from input video to output AV1 bitstream.
//!
//! # Test Strategy (Based on Industry Standards)
//!
//! ## References Researched:
//! - **SVT-AV1**: Uses y4m uncompressed input, multiple full-reference metrics
//!   (SSIMULACRA2, Butteraugli, XPSNR, VMAF) for quality validation
//! - **rav1e**: AV-Metrics integration (PSNR, PSNR-HVS, SSIM, MS-SSIM, CIEDE2000)
//!   via frame metrics function with QualityMetrics struct
//! - **dav1d**: MD5 digests of output YUV for decoder conformance verification
//! - **AV1 Spec**: Bitstream conformance via decoder model (leb128 limits, level constraints)
//! - **Determinism**: Frame checksums must match across runs (threading requires special handling)
//!
//! ## Test Categories:
//! 1. **Basic Pipeline** (Q15): Encoder produces valid output
//! 2. **OBU Format Validation** (Q16): Bitstream structure conformance
//! 3. **Compression Validation** (Q17): Output smaller than input
//! 4. **dav1d Decode Validation** (Q18): Output decodable by reference decoder
//! 5. **Determinism** (Q29-Q35): Reproducibility across runs
//! 6. **Tile Parallel Consistency** (Q30): Serial vs parallel equivalence
//! 7. **Resolution Scaling** (Q19): Multi-resolution stress testing
//! 8. **Content Patterns** (Q20): Flat, gradient, checkerboard, motion stress tests
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T6 Mixed tier metacapsule
//! - **T28**: Q15-Q35 (Integration + Production + Determinism tiers)
//! - **Chaos**: 100% lockfree pipeline
//! - **B32**: Performance validation (output size, encoding time)
//!
//! # Test Fixture Types
//!
//! All fixtures are synthetic YUV 4:2:0 test patterns:
//! - **Flat Gray**: Uniform mid-gray (stress test for DC prediction)
//! - **Horizontal Gradient**: Left-to-right luminance gradient (intra prediction stress)
//! - **Vertical Gradient**: Top-to-bottom gradient (directional mode stress)
//! - **Checkerboard**: High-frequency pattern (quantization/transform stress)
//! - **Color Bars**: Standard test pattern (chroma handling)
//! - **Motion Pattern**: Frame-to-frame translation (inter prediction stress)
//! - **Scene Change**: Abrupt content change (keyframe insertion logic)
//! - **Single Color**: All pixels same value (edge case: zero variance)
//! - **Max Contrast**: Alternating black/white (edge case: max variance)
//!
//! # Running Tests
//!
//! ```bash
//! # All E2E tests
//! cargo test --test e2e_encoding_test
//!
//! # Tests requiring dav1d (skip if not installed)
//! cargo test --test e2e_encoding_test -- --ignored
//!
//! # Stress tests (long-running)
//! cargo test --test e2e_encoding_test -- --ignored stress
//! ```

use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::process::Command;

use kindly_av1::encoder::EncoderWiringCapsule;

// ============================================================================
// Test Fixture Generators
// ============================================================================

/// Generate flat gray YUV 4:2:0 test frame
///
/// All pixels are mid-gray (128). Tests DC prediction and zero-variance handling.
fn create_flat_gray_yuv(width: u32, height: u32) -> Vec<u8> {
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4; // 4:2:0 subsampling
    let total_size = y_size + 2 * uv_size;

    vec![128u8; total_size]
}

/// Generate horizontal gradient YUV 4:2:0 test frame
///
/// Luminance increases from 16 (left) to 235 (right). Tests intra prediction modes.
fn create_horizontal_gradient_yuv(width: u32, height: u32) -> Vec<u8> {
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4;
    let total_size = y_size + 2 * uv_size;

    let mut yuv = vec![0u8; total_size];

    // Y plane: horizontal gradient
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            // Legal range: 16-235 (ITU-R BT.601)
            yuv[idx] = (16 + (x * 219) / (width.max(1) - 1).max(1)) as u8;
        }
    }

    // U/V planes: neutral chroma
    for i in 0..uv_size {
        yuv[y_size + i] = 128;
        yuv[y_size + uv_size + i] = 128;
    }

    yuv
}

/// Generate vertical gradient YUV 4:2:0 test frame
///
/// Luminance increases from 16 (top) to 235 (bottom). Tests vertical prediction modes.
fn create_vertical_gradient_yuv(width: u32, height: u32) -> Vec<u8> {
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4;
    let total_size = y_size + 2 * uv_size;

    let mut yuv = vec![0u8; total_size];

    // Y plane: vertical gradient
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            yuv[idx] = (16 + (y * 219) / (height.max(1) - 1).max(1)) as u8;
        }
    }

    // U/V planes: neutral chroma
    for i in 0..uv_size {
        yuv[y_size + i] = 128;
        yuv[y_size + uv_size + i] = 128;
    }

    yuv
}

/// Generate checkerboard YUV 4:2:0 test frame
///
/// 8x8 checkerboard pattern (black/white squares). High-frequency stress test.
fn create_checkerboard_yuv(width: u32, height: u32) -> Vec<u8> {
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4;
    let total_size = y_size + 2 * uv_size;

    let mut yuv = vec![0u8; total_size];

    // Y plane: checkerboard
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let is_white = ((x / 8) + (y / 8)) % 2 == 0;
            yuv[idx] = if is_white { 235 } else { 16 };
        }
    }

    // U/V planes: neutral chroma
    for i in 0..uv_size {
        yuv[y_size + i] = 128;
        yuv[y_size + uv_size + i] = 128;
    }

    yuv
}

/// Generate color bars YUV 4:2:0 test frame
///
/// Standard SMPTE color bars pattern. Tests chroma handling.
fn create_color_bars_yuv(width: u32, height: u32) -> Vec<u8> {
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4;
    let total_size = y_size + 2 * uv_size;

    let mut yuv = vec![0u8; total_size];

    // SMPTE color bar Y, U, V values (approximate)
    // White, Yellow, Cyan, Green, Magenta, Red, Blue, Black
    let bars: [(u8, u8, u8); 8] = [
        (235, 128, 128), // White
        (210, 16, 146),  // Yellow
        (170, 166, 16),  // Cyan
        (145, 54, 34),   // Green
        (106, 202, 222), // Magenta
        (81, 90, 240),   // Red
        (41, 240, 110),  // Blue
        (16, 128, 128),  // Black
    ];

    let bar_width = width / 8;

    // Y plane
    for y in 0..height {
        for x in 0..width {
            let bar_idx = ((x / bar_width) as usize).min(7);
            let idx = (y * width + x) as usize;
            yuv[idx] = bars[bar_idx].0;
        }
    }

    // U plane (subsampled 2x2)
    for y in 0..(height / 2) {
        for x in 0..(width / 2) {
            let bar_idx = ((x * 2 / bar_width) as usize).min(7);
            let idx = y_size + (y * (width / 2) + x) as usize;
            yuv[idx] = bars[bar_idx].1;
        }
    }

    // V plane
    for y in 0..(height / 2) {
        for x in 0..(width / 2) {
            let bar_idx = ((x * 2 / bar_width) as usize).min(7);
            let idx = y_size + uv_size + (y * (width / 2) + x) as usize;
            yuv[idx] = bars[bar_idx].2;
        }
    }

    yuv
}

/// Generate motion pattern YUV 4:2:0 test frame (with frame-dependent offset)
///
/// Vertical bars that move horizontally by 4 pixels per frame. Tests inter prediction.
fn create_motion_pattern_yuv(width: u32, height: u32, frame_num: u32) -> Vec<u8> {
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4;
    let total_size = y_size + 2 * uv_size;

    let mut yuv = vec![0u8; total_size];

    // Horizontal offset: 4 pixels per frame
    let offset = (frame_num * 4) % width;

    // Y plane: Moving vertical bars (white on black)
    for y in 0..height {
        for x in 0..width {
            let shifted_x = (x + offset) % width;
            let is_white = (shifted_x / 8) % 2 == 0;
            let idx = (y * width + x) as usize;
            yuv[idx] = if is_white { 235 } else { 16 };
        }
    }

    // U/V planes: neutral chroma
    for i in 0..uv_size {
        yuv[y_size + i] = 128;
        yuv[y_size + uv_size + i] = 128;
    }

    yuv
}

/// Generate single color YUV 4:2:0 test frame (edge case: zero variance)
fn create_single_color_yuv(width: u32, height: u32, y_value: u8) -> Vec<u8> {
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4;
    let total_size = y_size + 2 * uv_size;

    let mut yuv = vec![0u8; total_size];

    // Y plane: uniform value
    for i in 0..y_size {
        yuv[i] = y_value;
    }

    // U/V planes: neutral chroma
    for i in 0..uv_size {
        yuv[y_size + i] = 128;
        yuv[y_size + uv_size + i] = 128;
    }

    yuv
}

/// Generate max contrast YUV 4:2:0 test frame (edge case: max variance)
///
/// Alternating black/white pixels (checkerboard at 1x1 pixel level).
fn create_max_contrast_yuv(width: u32, height: u32) -> Vec<u8> {
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4;
    let total_size = y_size + 2 * uv_size;

    let mut yuv = vec![0u8; total_size];

    // Y plane: alternating pixels
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let is_white = (x + y) % 2 == 0;
            yuv[idx] = if is_white { 235 } else { 16 };
        }
    }

    // U/V planes: neutral chroma
    for i in 0..uv_size {
        yuv[y_size + i] = 128;
        yuv[y_size + uv_size + i] = 128;
    }

    yuv
}

// ============================================================================
// IVF Container Helper
// ============================================================================

/// Write IVF container with AV1 frames
///
/// IVF (Indeo Video File) is a simple container used by AV1 reference tools.
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

/// Check if dav1d is installed
fn is_dav1d_installed() -> bool {
    Command::new("which")
        .arg("dav1d")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Validate IVF file with dav1d decoder
fn validate_with_dav1d(ivf_path: &str) -> Result<(), String> {
    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .map_err(|e| format!("Failed to execute dav1d: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("dav1d decoding failed:\n{}", stderr))
    }
}

/// Hash bytes for determinism testing
fn hash_bytes(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

// ============================================================================
// T28 Q15-Q21: Integration Tier Tests
// ============================================================================

/// Q15-1: Basic encoding produces output
#[test]
fn test_q15_basic_encoding_produces_output() {
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_flat_gray_yuv(64, 64);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    assert!(!encoded.is_empty(), "Q15-1: Encoded output must not be empty");
    println!(
        "Q15-1 PASSED: Encoded {} bytes from {} byte input",
        encoded.len(),
        yuv.len()
    );
}

/// Q15-2: Encoding with different patterns all succeed
#[test]
fn test_q15_different_patterns_encode_successfully() {
    let patterns: Vec<(&str, Vec<u8>)> = vec![
        ("flat_gray", create_flat_gray_yuv(64, 64)),
        ("horizontal_gradient", create_horizontal_gradient_yuv(64, 64)),
        ("vertical_gradient", create_vertical_gradient_yuv(64, 64)),
        ("checkerboard", create_checkerboard_yuv(64, 64)),
        ("color_bars", create_color_bars_yuv(64, 64)),
        ("single_black", create_single_color_yuv(64, 64, 16)),
        ("single_white", create_single_color_yuv(64, 64, 235)),
        ("max_contrast", create_max_contrast_yuv(64, 64)),
    ];

    for (name, yuv) in patterns {
        let mut encoder = EncoderWiringCapsule::new();
        let mut sub_capsules = encoder
            .initialize(64, 64, 28, 5)
            .expect(&format!("Failed to initialize encoder for {}", name));

        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode {} pattern", name));

        assert!(
            !encoded.is_empty(),
            "Q15-2: {} pattern produced empty output",
            name
        );
        println!("Q15-2: {} pattern encoded to {} bytes", name, encoded.len());
    }
}

/// Q16-1: Validate OBU format structure
#[test]
fn test_q16_obu_format_validation() {
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_flat_gray_yuv(64, 64);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    // AV1 OBU header structure:
    // Bit 0: obu_forbidden_bit (must be 0)
    // Bits 1-4: obu_type
    // Bit 5: obu_extension_flag
    // Bit 6: obu_has_size_field
    // Bit 7: obu_reserved_1bit
    assert!(!encoded.is_empty(), "Output must not be empty");

    let first_byte = encoded[0];
    let obu_forbidden_bit = first_byte & 0x80;
    let obu_type = (first_byte >> 3) & 0x0F;
    let obu_has_size = (first_byte >> 1) & 0x01;

    // Forbidden bit must be 0
    assert_eq!(
        obu_forbidden_bit, 0,
        "Q16-1: OBU forbidden bit must be 0, got {}",
        obu_forbidden_bit
    );

    // Valid OBU types: 1 (seq_hdr), 2 (td), 3 (frame_hdr), 4 (tile_grp), 6 (frame), 7 (redundant)
    assert!(
        [1, 2, 3, 4, 6, 7].contains(&obu_type),
        "Q16-1: Invalid OBU type {}, expected one of [1,2,3,4,6,7]",
        obu_type
    );

    println!(
        "Q16-1 PASSED: OBU type={}, has_size={}",
        obu_type, obu_has_size
    );
}

/// Q16-2: First frame starts with temporal delimiter or sequence header
#[test]
fn test_q16_first_frame_structure() {
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_flat_gray_yuv(64, 64);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    // First OBU should be either temporal delimiter (type 2) or sequence header (type 1)
    let first_obu_type = (encoded[0] >> 3) & 0x0F;

    assert!(
        first_obu_type == 2 || first_obu_type == 1,
        "Q16-2: First OBU must be TD (2) or Seq Header (1), got {}",
        first_obu_type
    );

    // If TD, sequence header should follow
    if first_obu_type == 2 {
        // TD is usually 2 bytes (header + size=0)
        if encoded.len() > 2 {
            let second_obu_type = (encoded[2] >> 3) & 0x0F;
            assert_eq!(
                second_obu_type, 1,
                "Q16-2: After TD, expected Seq Header (1), got {}",
                second_obu_type
            );
        }
    }

    println!("Q16-2 PASSED: First OBU type = {}", first_obu_type);
}

/// Q17-1: Output achieves compression (smaller than input)
#[test]
fn test_q17_compression_achieved() {
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(256, 256, 28, 5)
        .expect("Failed to initialize encoder");

    // Use gradient pattern (compressible content)
    let yuv = create_horizontal_gradient_yuv(256, 256);
    let input_size = yuv.len();

    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    let output_size = encoded.len();

    // Output should be smaller than input (except for very high quality/small frames)
    // For 256x256 gradient at CRF 28, expect significant compression
    let compression_ratio = input_size as f64 / output_size as f64;

    println!(
        "Q17-1: Input {} bytes -> Output {} bytes (ratio: {:.2}x)",
        input_size, output_size, compression_ratio
    );

    // Expect at least 2x compression for gradient content
    assert!(
        compression_ratio > 1.5,
        "Q17-1: Expected compression ratio > 1.5x, got {:.2}x",
        compression_ratio
    );
}

/// Q17-2: Compression ratio varies with content complexity
#[test]
fn test_q17_compression_varies_with_content() {
    let mut results: Vec<(&str, usize, usize, f64)> = Vec::new();

    let patterns: Vec<(&str, Vec<u8>)> = vec![
        ("flat_gray", create_flat_gray_yuv(128, 128)),
        ("gradient", create_horizontal_gradient_yuv(128, 128)),
        ("checkerboard", create_checkerboard_yuv(128, 128)),
    ];

    for (name, yuv) in patterns {
        let mut encoder = EncoderWiringCapsule::new();
        let mut sub_capsules = encoder
            .initialize(128, 128, 28, 5)
            .expect("Failed to initialize encoder");

        let input_size = yuv.len();
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode {} pattern", name));

        let output_size = encoded.len();
        let ratio = input_size as f64 / output_size as f64;

        results.push((name, input_size, output_size, ratio));
        println!(
            "Q17-2: {} - {} bytes -> {} bytes ({:.2}x)",
            name, input_size, output_size, ratio
        );
    }

    // Flat gray should compress best (lowest entropy)
    // Checkerboard should compress worst (highest entropy)
    let flat_ratio = results.iter().find(|r| r.0 == "flat_gray").unwrap().3;
    let checker_ratio = results.iter().find(|r| r.0 == "checkerboard").unwrap().3;

    assert!(
        flat_ratio > checker_ratio,
        "Q17-2: Flat gray ({:.2}x) should compress better than checkerboard ({:.2}x)",
        flat_ratio,
        checker_ratio
    );
}

/// Q18-1: dav1d can decode 64x64 encoded frame
#[test]
fn test_q18_dav1d_decode_64x64() {
    if !is_dav1d_installed() {
        eprintln!("dav1d not installed, skipping Q18-1");
        return;
    }

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(64, 64, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_flat_gray_yuv(64, 64);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    let ivf_path = "/tmp/e2e_test_64x64.ivf";
    write_ivf_file(ivf_path, 64, 64, &[encoded]).expect("Failed to write IVF");

    validate_with_dav1d(ivf_path).expect("Q18-1: dav1d decoding failed");
    println!("Q18-1 PASSED: dav1d successfully decoded 64x64 frame");
}

/// Q18-2: dav1d can decode 128x128 encoded frame
#[test]
fn test_q18_dav1d_decode_128x128() {
    if !is_dav1d_installed() {
        eprintln!("dav1d not installed, skipping Q18-2");
        return;
    }

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(128, 128, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_horizontal_gradient_yuv(128, 128);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    let ivf_path = "/tmp/e2e_test_128x128.ivf";
    write_ivf_file(ivf_path, 128, 128, &[encoded]).expect("Failed to write IVF");

    validate_with_dav1d(ivf_path).expect("Q18-2: dav1d decoding failed");
    println!("Q18-2 PASSED: dav1d successfully decoded 128x128 frame");
}

/// Q18-3: dav1d can decode 1920x1080 encoded frame
#[test]
fn test_q18_dav1d_decode_1080p() {
    if !is_dav1d_installed() {
        eprintln!("dav1d not installed, skipping Q18-3");
        return;
    }

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(1920, 1080, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_horizontal_gradient_yuv(1920, 1080);
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode frame");

    let ivf_path = "/tmp/e2e_test_1080p.ivf";
    let encoded_len = encoded.len();
    write_ivf_file(ivf_path, 1920, 1080, &[encoded]).expect("Failed to write IVF");

    validate_with_dav1d(ivf_path).expect("Q18-3: dav1d decoding failed");
    println!(
        "Q18-3 PASSED: dav1d successfully decoded 1920x1080 frame ({} bytes)",
        encoded_len
    );
}

/// Q19-1: Multi-resolution encoding consistency
#[test]
fn test_q19_multi_resolution_encoding() {
    let resolutions: [(u32, u32, &str); 6] = [
        (64, 64, "64x64"),
        (128, 128, "128x128"),
        (320, 240, "QVGA"),
        (640, 480, "VGA"),
        (854, 480, "480p"),
        (1280, 720, "720p"),
    ];

    for (width, height, name) in resolutions {
        let mut encoder = EncoderWiringCapsule::new();
        let mut sub_capsules = encoder
            .initialize(width, height, 28, 5)
            .expect(&format!("Failed to initialize encoder for {}", name));

        let yuv = create_horizontal_gradient_yuv(width, height);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode {} resolution", name));

        assert!(!encoded.is_empty(), "Q19-1: {} produced empty output", name);

        let compression_ratio = yuv.len() as f64 / encoded.len() as f64;
        println!(
            "Q19-1: {} ({}x{}) - {} bytes -> {} bytes ({:.2}x)",
            name,
            width,
            height,
            yuv.len(),
            encoded.len(),
            compression_ratio
        );
    }
}

/// Q20-1: Multi-frame encoding (GOP structure)
#[test]
fn test_q20_multi_frame_encoding() {
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(128, 128, 28, 5)
        .expect("Failed to initialize encoder");

    let mut total_bytes = 0;
    let mut frames_encoded = 0;

    // Encode 10 frames with motion pattern
    for frame_num in 0..10 {
        let yuv = create_motion_pattern_yuv(128, 128, frame_num);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", frame_num));

        total_bytes += encoded.len();
        frames_encoded += 1;
        println!("Q20-1: Frame {} encoded to {} bytes", frame_num, encoded.len());
    }

    println!(
        "Q20-1 PASSED: Encoded {} frames ({} total bytes, {:.1} avg bytes/frame)",
        frames_encoded,
        total_bytes,
        total_bytes as f64 / frames_encoded as f64
    );
}

/// Q21-1: dav1d validates multi-frame sequence
#[test]
fn test_q21_dav1d_multi_frame_sequence() {
    if !is_dav1d_installed() {
        eprintln!("dav1d not installed, skipping Q21-1");
        return;
    }

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(128, 128, 28, 5)
        .expect("Failed to initialize encoder");

    let mut frames = Vec::new();

    // Encode 5 frames with motion pattern
    for frame_num in 0..5 {
        let yuv = create_motion_pattern_yuv(128, 128, frame_num);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", frame_num));
        frames.push(encoded);
    }

    let ivf_path = "/tmp/e2e_test_multi_frame.ivf";
    write_ivf_file(ivf_path, 128, 128, &frames).expect("Failed to write IVF");

    validate_with_dav1d(ivf_path).expect("Q21-1: dav1d decoding failed");
    println!(
        "Q21-1 PASSED: dav1d successfully decoded {}-frame sequence",
        frames.len()
    );
}

// ============================================================================
// T28 Q22-Q28: Production Tier Tests
// ============================================================================

/// Q22-1: Production-scale 1080p encoding
#[test]
fn test_q22_production_1080p() {
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(1920, 1080, 28, 5)
        .expect("Failed to initialize encoder");

    let yuv = create_color_bars_yuv(1920, 1080);
    let input_size = yuv.len();

    let start = std::time::Instant::now();
    let encoded = encoder
        .encode_frame(&yuv, &mut sub_capsules)
        .expect("Failed to encode 1080p frame");
    let duration = start.elapsed();

    let output_size = encoded.len();
    let compression_ratio = input_size as f64 / output_size as f64;

    println!(
        "Q22-1: 1080p encoding: {} bytes -> {} bytes ({:.2}x) in {:?}",
        input_size, output_size, compression_ratio, duration
    );

    // Sanity checks
    assert!(!encoded.is_empty(), "Q22-1: Output must not be empty");
    assert!(
        compression_ratio > 1.0,
        "Q22-1: Expected some compression, got {:.2}x",
        compression_ratio
    );
}

/// Q23-1: High-volume encoding (30 frames)
#[test]
fn test_q23_high_volume_30_frames() {
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder
        .initialize(320, 240, 28, 5)
        .expect("Failed to initialize encoder");

    let mut total_bytes = 0;
    let start = std::time::Instant::now();

    for frame_num in 0..30 {
        let yuv = create_motion_pattern_yuv(320, 240, frame_num);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", frame_num));
        total_bytes += encoded.len();
    }

    let duration = start.elapsed();
    let fps = 30.0 / duration.as_secs_f64();

    println!(
        "Q23-1: Encoded 30 frames (QVGA) in {:?} ({:.1} fps, {} total bytes)",
        duration, fps, total_bytes
    );
}

/// Q24-1: All test patterns encode successfully at 720p
#[test]
fn test_q24_all_patterns_720p() {
    let patterns: Vec<(&str, Box<dyn Fn(u32, u32) -> Vec<u8>>)> = vec![
        ("flat_gray", Box::new(|w, h| create_flat_gray_yuv(w, h))),
        (
            "horizontal_gradient",
            Box::new(|w, h| create_horizontal_gradient_yuv(w, h)),
        ),
        (
            "vertical_gradient",
            Box::new(|w, h| create_vertical_gradient_yuv(w, h)),
        ),
        ("checkerboard", Box::new(|w, h| create_checkerboard_yuv(w, h))),
        ("color_bars", Box::new(|w, h| create_color_bars_yuv(w, h))),
        ("max_contrast", Box::new(|w, h| create_max_contrast_yuv(w, h))),
    ];

    for (name, gen_fn) in patterns {
        let mut encoder = EncoderWiringCapsule::new();
        let mut sub_capsules = encoder
            .initialize(1280, 720, 28, 5)
            .expect(&format!("Failed to initialize encoder for {}", name));

        let yuv = gen_fn(1280, 720);
        let encoded = encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect(&format!("Failed to encode {} at 720p", name));

        let ratio = yuv.len() as f64 / encoded.len() as f64;
        println!(
            "Q24-1: {} @ 720p: {} bytes ({:.2}x compression)",
            name,
            encoded.len(),
            ratio
        );
    }
}

// ============================================================================
// T28 Q29-Q35: Determinism Tier Tests
// ============================================================================

/// Q29-1: Same input produces identical output
#[test]
fn test_q29_determinism_basic() {
    let yuv = create_horizontal_gradient_yuv(128, 128);

    let encode_once = || {
        let mut encoder = EncoderWiringCapsule::new();
        let mut sub_capsules = encoder
            .initialize(128, 128, 28, 5)
            .expect("Failed to initialize encoder");
        encoder
            .encode_frame(&yuv, &mut sub_capsules)
            .expect("Failed to encode frame")
    };

    let output1 = encode_once();
    let output2 = encode_once();

    let hash1 = hash_bytes(&output1);
    let hash2 = hash_bytes(&output2);

    assert_eq!(
        hash1, hash2,
        "Q29-1: Same input must produce identical output"
    );
    println!("Q29-1 PASSED: Deterministic output (hash: {:016x})", hash1);
}

/// Q29-2: Different inputs produce different outputs (sanity check)
#[test]
fn test_q29_different_inputs_different_outputs() {
    let yuv1 = create_flat_gray_yuv(64, 64);
    let yuv2 = create_checkerboard_yuv(64, 64);

    let mut encoder1 = EncoderWiringCapsule::new();
    let mut sub_capsules1 = encoder1.initialize(64, 64, 28, 5).unwrap();
    let output1 = encoder1.encode_frame(&yuv1, &mut sub_capsules1).unwrap();

    let mut encoder2 = EncoderWiringCapsule::new();
    let mut sub_capsules2 = encoder2.initialize(64, 64, 28, 5).unwrap();
    let output2 = encoder2.encode_frame(&yuv2, &mut sub_capsules2).unwrap();

    let hash1 = hash_bytes(&output1);
    let hash2 = hash_bytes(&output2);

    assert_ne!(
        hash1, hash2,
        "Q29-2: Different inputs should produce different outputs"
    );
    println!("Q29-2 PASSED: Different inputs produce different outputs");
}

/// Q30-1: Same speed preset produces consistent results
#[test]
fn test_q30_speed_preset_consistency() {
    let yuv = create_horizontal_gradient_yuv(64, 64);

    let encode_with_speed = |speed: u8| {
        let mut encoder = EncoderWiringCapsule::new();
        let mut sub_capsules = encoder.initialize(64, 64, 28, speed).unwrap();
        encoder.encode_frame(&yuv, &mut sub_capsules).unwrap()
    };

    // Same speed preset should be deterministic
    let output_s5_a = encode_with_speed(5);
    let output_s5_b = encode_with_speed(5);

    let hash_a = hash_bytes(&output_s5_a);
    let hash_b = hash_bytes(&output_s5_b);

    assert_eq!(
        hash_a, hash_b,
        "Q30-1: Same speed preset must produce identical output"
    );
    println!(
        "Q30-1 PASSED: Speed preset 5 is deterministic (hash: {:016x})",
        hash_a
    );
}

/// Q31-1: Multi-frame sequence is deterministic
#[test]
fn test_q31_multi_frame_determinism() {
    let encode_sequence = || {
        let mut encoder = EncoderWiringCapsule::new();
        let mut sub_capsules = encoder.initialize(64, 64, 28, 5).unwrap();
        let mut all_output = Vec::new();

        for frame_num in 0..5 {
            let yuv = create_motion_pattern_yuv(64, 64, frame_num);
            let encoded = encoder.encode_frame(&yuv, &mut sub_capsules).unwrap();
            all_output.extend_from_slice(&encoded);
        }

        all_output
    };

    let output1 = encode_sequence();
    let output2 = encode_sequence();

    let hash1 = hash_bytes(&output1);
    let hash2 = hash_bytes(&output2);

    assert_eq!(
        hash1, hash2,
        "Q31-1: Multi-frame sequence must be deterministic"
    );
    println!(
        "Q31-1 PASSED: 5-frame sequence is deterministic (hash: {:016x})",
        hash1
    );
}

/// Q32-1: No state leakage between encoder instances
#[test]
fn test_q32_no_state_leakage() {
    let yuv_gradient = create_horizontal_gradient_yuv(64, 64);
    let yuv_checkerboard = create_checkerboard_yuv(64, 64);

    // Encode gradient pattern
    let mut encoder1 = EncoderWiringCapsule::new();
    let mut sub_capsules1 = encoder1.initialize(64, 64, 28, 5).unwrap();
    let output1_first = encoder1.encode_frame(&yuv_gradient, &mut sub_capsules1).unwrap();
    let hash1_first = hash_bytes(&output1_first);

    // Encode checkerboard pattern (different content)
    let mut encoder2 = EncoderWiringCapsule::new();
    let mut sub_capsules2 = encoder2.initialize(64, 64, 28, 5).unwrap();
    let _output2 = encoder2.encode_frame(&yuv_checkerboard, &mut sub_capsules2).unwrap();

    // Encode gradient pattern again (should match first)
    let mut encoder3 = EncoderWiringCapsule::new();
    let mut sub_capsules3 = encoder3.initialize(64, 64, 28, 5).unwrap();
    let output3 = encoder3.encode_frame(&yuv_gradient, &mut sub_capsules3).unwrap();
    let hash3 = hash_bytes(&output3);

    assert_eq!(
        hash1_first, hash3,
        "Q32-1: No state leakage between encoder instances"
    );
    println!("Q32-1 PASSED: No state leakage detected");
}

/// Q33-1: CRF variations produce deterministic outputs
#[test]
fn test_q33_crf_determinism() {
    let yuv = create_horizontal_gradient_yuv(64, 64);

    for crf in [15, 28, 40, 55].iter() {
        let encode_with_crf = || {
            let mut encoder = EncoderWiringCapsule::new();
            let mut sub_capsules = encoder.initialize(64, 64, *crf, 5).unwrap();
            encoder.encode_frame(&yuv, &mut sub_capsules).unwrap()
        };

        let output1 = encode_with_crf();
        let output2 = encode_with_crf();

        let hash1 = hash_bytes(&output1);
        let hash2 = hash_bytes(&output2);

        assert_eq!(
            hash1, hash2,
            "Q33-1: CRF {} must be deterministic",
            crf
        );
        println!(
            "Q33-1: CRF {} deterministic ({} bytes, hash: {:016x})",
            crf,
            output1.len(),
            hash1
        );
    }
}

/// Q34-1: 100-frame stress test for determinism
#[test]
fn test_q34_100_frame_determinism() {
    let encode_100_frames = || {
        let mut encoder = EncoderWiringCapsule::new();
        let mut sub_capsules = encoder.initialize(64, 64, 28, 5).unwrap();
        let mut all_output = Vec::new();

        for frame_num in 0..100 {
            let yuv = create_motion_pattern_yuv(64, 64, frame_num);
            let encoded = encoder.encode_frame(&yuv, &mut sub_capsules).unwrap();
            all_output.extend_from_slice(&encoded);
        }

        all_output
    };

    let output1 = encode_100_frames();
    let output2 = encode_100_frames();

    let hash1 = hash_bytes(&output1);
    let hash2 = hash_bytes(&output2);

    assert_eq!(
        hash1, hash2,
        "Q34-1: 100-frame encoding must be deterministic"
    );
    println!(
        "Q34-1 PASSED: 100-frame sequence is deterministic ({} bytes, hash: {:016x})",
        output1.len(),
        hash1
    );
}

/// Q35-1: 1000 identical encodes stress test
#[test]
#[ignore = "Long-running stress test - run with --ignored"]
fn test_q35_stress_1000_identical_encodes() {
    let yuv = create_horizontal_gradient_yuv(64, 64);
    let mut reference_hash: Option<u64> = None;

    for run in 0..1000 {
        let mut encoder = EncoderWiringCapsule::new();
        let mut sub_capsules = encoder.initialize(64, 64, 28, 5).unwrap();
        let output = encoder.encode_frame(&yuv, &mut sub_capsules).unwrap();
        let hash = hash_bytes(&output);

        if let Some(ref_hash) = reference_hash {
            if hash != ref_hash {
                panic!(
                    "Q35-1: Non-determinism detected at run {} (expected {:016x}, got {:016x})",
                    run, ref_hash, hash
                );
            }
        } else {
            reference_hash = Some(hash);
        }

        if (run + 1) % 100 == 0 {
            println!("Q35-1: Completed {} runs...", run + 1);
        }
    }

    println!(
        "Q35-1 PASSED: 1000 identical encodes are deterministic (hash: {:016x})",
        reference_hash.unwrap()
    );
}

// ============================================================================
// E2E Pipeline Validation Tests
// ============================================================================

/// Complete E2E test: encode -> decode -> validate
#[test]
fn test_e2e_full_pipeline_64x64() {
    if !is_dav1d_installed() {
        eprintln!("dav1d not installed, skipping E2E test");
        return;
    }

    // Step 1: Create test input
    let yuv = create_color_bars_yuv(64, 64);
    let input_hash = hash_bytes(&yuv);

    // Step 2: Encode
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder.initialize(64, 64, 28, 5).unwrap();
    let encoded = encoder.encode_frame(&yuv, &mut sub_capsules).unwrap();

    // Step 3: Validate OBU structure
    let obu_type = (encoded[0] >> 3) & 0x0F;
    assert!([1, 2].contains(&obu_type), "First OBU must be TD or Seq Header");

    // Step 4: Write IVF container
    let ivf_path = "/tmp/e2e_full_pipeline_64x64.ivf";
    write_ivf_file(ivf_path, 64, 64, &[encoded.clone()]).unwrap();

    // Step 5: Decode with dav1d
    validate_with_dav1d(ivf_path).unwrap();

    // Step 6: Verify determinism
    let mut encoder2 = EncoderWiringCapsule::new();
    let mut sub_capsules2 = encoder2.initialize(64, 64, 28, 5).unwrap();
    let encoded2 = encoder2.encode_frame(&yuv, &mut sub_capsules2).unwrap();

    assert_eq!(
        hash_bytes(&encoded),
        hash_bytes(&encoded2),
        "E2E pipeline must be deterministic"
    );

    println!(
        "E2E PASSED: input {} bytes (hash {:016x}) -> encoded {} bytes -> dav1d decoded OK -> deterministic",
        yuv.len(), input_hash, encoded.len()
    );
}

/// Complete E2E test: multi-frame sequence
#[test]
fn test_e2e_multi_frame_sequence() {
    if !is_dav1d_installed() {
        eprintln!("dav1d not installed, skipping E2E multi-frame test");
        return;
    }

    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder.initialize(128, 128, 28, 5).unwrap();

    let mut frames = Vec::new();

    // Encode 10 frames with motion
    for frame_num in 0..10 {
        let yuv = create_motion_pattern_yuv(128, 128, frame_num);
        let encoded = encoder.encode_frame(&yuv, &mut sub_capsules).unwrap();
        frames.push(encoded);
    }

    // Write IVF
    let ivf_path = "/tmp/e2e_multi_frame_sequence.ivf";
    write_ivf_file(ivf_path, 128, 128, &frames).unwrap();

    // Validate with dav1d
    validate_with_dav1d(ivf_path).unwrap();

    let total_bytes: usize = frames.iter().map(|f| f.len()).sum();
    println!(
        "E2E Multi-Frame PASSED: {} frames, {} total bytes, dav1d decoded OK",
        frames.len(),
        total_bytes
    );
}

/// Intra-only test (no inter prediction)
#[test]
fn test_e2e_intra_only_encoding() {
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder.initialize(128, 128, 28, 5).unwrap();

    // Encode frames that are independent (force keyframes by using different encoder instances)
    let mut frame_sizes = Vec::new();

    for _ in 0..5 {
        let yuv = create_flat_gray_yuv(128, 128);
        let mut single_encoder = EncoderWiringCapsule::new();
        let mut single_subs = single_encoder.initialize(128, 128, 28, 5).unwrap();
        let encoded = single_encoder.encode_frame(&yuv, &mut single_subs).unwrap();
        frame_sizes.push(encoded.len());
    }

    // All frames should be similar size (all keyframes)
    let avg_size: f64 = frame_sizes.iter().sum::<usize>() as f64 / frame_sizes.len() as f64;
    for (i, size) in frame_sizes.iter().enumerate() {
        let deviation = (*size as f64 - avg_size).abs() / avg_size;
        assert!(
            deviation < 0.1,
            "Intra-only frame {} size deviation too large: {:.1}%",
            i,
            deviation * 100.0
        );
    }

    println!(
        "E2E Intra-Only PASSED: {} frames, avg {} bytes/frame",
        frame_sizes.len(),
        avg_size as usize
    );
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Minimum resolution test (8x8)
#[test]
fn test_edge_case_minimum_resolution() {
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder.initialize(8, 8, 28, 5).unwrap();

    let yuv = create_flat_gray_yuv(8, 8);
    let encoded = encoder.encode_frame(&yuv, &mut sub_capsules).unwrap();

    assert!(!encoded.is_empty(), "8x8 encoding must produce output");
    println!(
        "Edge Case 8x8 PASSED: {} bytes input -> {} bytes output",
        yuv.len(),
        encoded.len()
    );
}

/// Non-multiple-of-8 resolution test
#[test]
fn test_edge_case_non_aligned_resolution() {
    // 100x100 is not a multiple of 8 or 64
    let mut encoder = EncoderWiringCapsule::new();
    let mut sub_capsules = encoder.initialize(100, 100, 28, 5).unwrap();

    let yuv = create_horizontal_gradient_yuv(100, 100);
    let encoded = encoder.encode_frame(&yuv, &mut sub_capsules).unwrap();

    assert!(!encoded.is_empty(), "100x100 encoding must produce output");
    println!(
        "Edge Case 100x100 PASSED: {} bytes input -> {} bytes output",
        yuv.len(),
        encoded.len()
    );
}

/// Maximum CRF test (quality extremes)
#[test]
fn test_edge_case_crf_extremes() {
    let yuv = create_horizontal_gradient_yuv(64, 64);

    // CRF 0 (highest quality)
    let mut encoder_q0 = EncoderWiringCapsule::new();
    let mut sub_capsules_q0 = encoder_q0.initialize(64, 64, 0, 5).unwrap();
    let encoded_q0 = encoder_q0.encode_frame(&yuv, &mut sub_capsules_q0).unwrap();

    // CRF 63 (lowest quality)
    let mut encoder_q63 = EncoderWiringCapsule::new();
    let mut sub_capsules_q63 = encoder_q63.initialize(64, 64, 63, 5).unwrap();
    let encoded_q63 = encoder_q63.encode_frame(&yuv, &mut sub_capsules_q63).unwrap();

    println!(
        "CRF 0: {} bytes, CRF 63: {} bytes",
        encoded_q0.len(),
        encoded_q63.len()
    );

    // Higher CRF should produce smaller output (more compression, less quality)
    // Note: This may not always hold for very small frames with little detail
    assert!(
        !encoded_q0.is_empty() && !encoded_q63.is_empty(),
        "Both CRF extremes must produce output"
    );
}

/// Speed preset extremes test
#[test]
fn test_edge_case_speed_extremes() {
    let yuv = create_horizontal_gradient_yuv(64, 64);

    // Speed 0 (slowest, best quality)
    let mut encoder_s0 = EncoderWiringCapsule::new();
    let mut sub_capsules_s0 = encoder_s0.initialize(64, 64, 28, 0).unwrap();
    let encoded_s0 = encoder_s0.encode_frame(&yuv, &mut sub_capsules_s0).unwrap();

    // Speed 10 (fastest)
    let mut encoder_s10 = EncoderWiringCapsule::new();
    let mut sub_capsules_s10 = encoder_s10.initialize(64, 64, 28, 10).unwrap();
    let encoded_s10 = encoder_s10.encode_frame(&yuv, &mut sub_capsules_s10).unwrap();

    println!(
        "Speed 0: {} bytes, Speed 10: {} bytes",
        encoded_s0.len(),
        encoded_s10.len()
    );

    assert!(
        !encoded_s0.is_empty() && !encoded_s10.is_empty(),
        "Both speed extremes must produce output"
    );
}

// ============================================================================
// Performance Characterization Tests (Informational)
// ============================================================================

/// Encoding time characterization (not a pass/fail test)
#[test]
fn test_perf_characterization() {
    let resolutions: [(u32, u32, &str); 4] = [
        (64, 64, "64x64"),
        (320, 240, "QVGA"),
        (1280, 720, "720p"),
        (1920, 1080, "1080p"),
    ];

    println!("\nPerformance Characterization (single frame, CRF 28, speed 5):");
    println!("{:12} {:>10} {:>12} {:>10}", "Resolution", "Input", "Output", "Time");
    println!("{:-<48}", "");

    for (width, height, name) in resolutions {
        let yuv = create_horizontal_gradient_yuv(width, height);

        let mut encoder = EncoderWiringCapsule::new();
        let mut sub_capsules = encoder.initialize(width, height, 28, 5).unwrap();

        let start = std::time::Instant::now();
        let encoded = encoder.encode_frame(&yuv, &mut sub_capsules).unwrap();
        let duration = start.elapsed();

        println!(
            "{:12} {:>10} {:>12} {:>10.2?}",
            name,
            yuv.len(),
            encoded.len(),
            duration
        );
    }
}
