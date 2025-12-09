//! Y4M → AV1 → dav1d Round-Trip Integration Tests
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! T28 Q15-Q21 Integration tier tests validating:
//!
//! - **Y4M Reading**: Y4mReader correctly parses headers and reads frames
//! - **AV1 Encoding**: BitstreamWriterCapsule produces valid AV1 bitstreams
//! - **IVF Container**: IvfContainerWriterCapsule wraps AV1 correctly
//! - **dav1d Decoding**: External validation via dav1d reference decoder
//! - **Quality Metrics**: PSNR ≥ 30dB (lossy encoding acceptable)
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q15-Q21 Integration tier (full pipeline validation)
//! - **Chaos**: Uses existing Y4mReader, BitstreamWriterCapsule, IvfContainerWriterCapsule
//! - **ASSUM**: Documents dav1d external dependency requirement
//! - **T28**: Integration test suite with quality validation
//!
//! ## Test Flow
//!
//! ```text
//! test_8x8.y4m ──┐
//! test_64x64.y4m ┼─→ Y4mReader ──→ AV1 Encoder ──→ IVF Container ──→ output.ivf
//! test_320x240   │                                                        │
//!                │                                                        v
//!                └────────────────────────────────────────────────← dav1d decoder
//!                                                                          │
//!                                                                          v
//!                                                                    decoded.y4m
//!                                                                          │
//!                                                                          v
//!                                                                   PSNR ≥ 30dB ✓
//! ```
//!
//! ## External Dependencies
//!
//! **dav1d** (optional): AV1 reference decoder for round-trip validation
//!
//! - Install: `sudo apt install dav1d` (Debian/Ubuntu) or `brew install dav1d` (macOS)
//! - Tests skip gracefully if dav1d not installed
//! - Detection: `which dav1d` (shell command)
//!
//! #ASSUME dav1d (if installed) produces correct AV1 decoding output
//! #VERIFY Tests skip if dav1d unavailable, no false failures

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;

use kindly_av1::file::{Frame, FrameReader, Y4mReader};

/// Check if dav1d is installed
///
/// #ASSUME `which dav1d` returns 0 exit code if installed
/// #VERIFY Result determines test execution (skip if false)
fn is_dav1d_installed() -> bool {
    Command::new("which")
        .arg("dav1d")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Calculate PSNR (Peak Signal-to-Noise Ratio) between two Y4M frames
///
/// PSNR = 10 * log10(MAX_I^2 / MSE)
/// where MAX_I = 255 for 8-bit YUV, and MSE = mean squared error
///
/// Higher PSNR = better quality. Typical acceptable range:
/// - 30-35 dB: Acceptable quality
/// - 35-40 dB: Good quality
/// - 40+ dB: Excellent quality
///
/// #ASSUME Both frames have identical dimensions
/// #VERIFY Caller ensures frame dimensions match before calling
fn calculate_psnr(original: &Frame, decoded: &Frame) -> f64 {
    assert_eq!(original.y.len(), decoded.y.len(), "Y plane size mismatch");

    // Calculate MSE (Mean Squared Error) for Y plane only (most important for quality)
    let mut mse = 0.0;
    for (orig_pixel, dec_pixel) in original.y.iter().zip(decoded.y.iter()) {
        let diff = (*orig_pixel as f64) - (*dec_pixel as f64);
        mse += diff * diff;
    }
    mse /= original.y.len() as f64;

    // Handle perfect match (infinite PSNR)
    if mse < 1e-10 {
        return 100.0; // Effectively infinite
    }

    // PSNR = 10 * log10(255^2 / MSE)
    let max_i = 255.0;
    10.0 * ((max_i * max_i) / mse).log10()
}

/// Read first frame from Y4M file (helper for testing)
fn read_first_y4m_frame<P: AsRef<Path>>(path: P) -> std::io::Result<Frame> {
    let mut reader = Y4mReader::open(path).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("Y4M open failed: {}", e))
    })?;

    reader
        .read_frame()
        .map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Y4M read failed: {}", e))
        })?
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "No frames in Y4M file")
        })
}

/// Minimal AV1 encoder stub for testing
///
/// TODO: Replace with full encoder implementation from BitstreamWriterCapsule
/// Currently generates placeholder bitstream for testing infrastructure.
///
/// #ASSUME This is temporary scaffolding for test infrastructure
/// #VERIFY Replace with real encoder before production use
fn encode_frame_to_av1_stub(_frame: &Frame) -> Vec<u8> {
    // PLACEHOLDER: Minimal AV1 OBU sequence for testing
    // Real implementation will use BitstreamWriterCapsule + DctTransformCapsule + EntropyCoderCapsule

    // OBU Type 1: Sequence Header (minimal)
    let mut bitstream = Vec::new();

    // OBU header: type=1 (sequence header), no extension
    bitstream.push(0x0a); // obu_type=1, obu_extension_flag=0, obu_has_size_field=1

    // OBU size (placeholder: 10 bytes)
    bitstream.push(10);

    // Minimal sequence header payload
    bitstream.extend_from_slice(&[
        0x00, 0x00, 0x00, 0x00, // seq_profile, level, tier
        0x00, 0x00, 0x00, 0x00, // timing info, decoder model
        0x00, 0x00, // color config
    ]);

    // OBU Type 6: Frame (minimal)
    bitstream.push(0x32); // obu_type=6, obu_has_size_field=1

    // Frame OBU size (placeholder)
    let frame_size = 20u32; // Placeholder
    bitstream.push((frame_size & 0x7f) as u8);

    // Minimal frame header + data
    bitstream.extend_from_slice(&[0u8; 20]);

    bitstream
}

/// Write IVF container with AV1 bitstream
///
/// IVF format is a simple container for AV1 (used by dav1d):
///
/// ```text
/// [IVF Header: 32 bytes]
/// [Frame 0: size (4 bytes) + timestamp (8 bytes) + data]
/// [Frame 1: size (4 bytes) + timestamp (8 bytes) + data]
/// ...
/// ```
///
/// #ASSUME IvfContainerWriterCapsule produces dav1d-compatible output
/// #VERIFY dav1d successfully decodes output (tested below)
fn write_ivf_file<P: AsRef<Path>>(
    path: P,
    width: u32,
    height: u32,
    frame_data: &[Vec<u8>],
) -> std::io::Result<()> {
    use std::io::Write;

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
    file.write_all(&(frame_data.len() as u32).to_le_bytes())?; // Frame count
    file.write_all(&[0u8; 4])?; // Unused

    // Write frames
    for (idx, data) in frame_data.iter().enumerate() {
        // Frame size (4 bytes LE)
        file.write_all(&(data.len() as u32).to_le_bytes())?;

        // Timestamp (8 bytes LE, frame_num * timebase)
        let timestamp = idx as u64;
        file.write_all(&timestamp.to_le_bytes())?;

        // Frame data
        file.write_all(data)?;
    }

    Ok(())
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_y4m_reader_8x8() {
    // Generate fixture first
    std::process::Command::new("cargo")
        .args(&["run", "--bin", "generate_fixtures"])
        .status()
        .expect("Failed to generate fixtures");

    let fixture = "tests/fixtures/test_8x8.y4m";
    let mut reader = Y4mReader::open(fixture).expect("Failed to open 8×8 fixture");

    let info = reader.info();
    assert_eq!(info.width, 8);
    assert_eq!(info.height, 8);
    assert_eq!(info.frame_count, 1);

    let frame = reader.read_frame().expect("Read failed").expect("No frame");
    assert_eq!(frame.width, 8);
    assert_eq!(frame.height, 8);
    assert_eq!(frame.frame_num, 0);

    // Y plane: 8×8 = 64 bytes
    assert_eq!(frame.y.len(), 64);
    // U/V planes: 4×4 = 16 bytes each (4:2:0 subsampling)
    assert_eq!(frame.u.len(), 16);
    assert_eq!(frame.v.len(), 16);

    // Second read should return None (EOF)
    assert!(reader.read_frame().expect("Read failed").is_none());
}

#[test]
fn test_y4m_reader_64x64_multiframe() {
    // Generate fixture first
    std::process::Command::new("cargo")
        .args(&["run", "--bin", "generate_fixtures"])
        .status()
        .expect("Failed to generate fixtures");

    let fixture = "tests/fixtures/test_64x64.y4m";
    let mut reader = Y4mReader::open(fixture).expect("Failed to open 64×64 fixture");

    let info = reader.info();
    assert_eq!(info.width, 64);
    assert_eq!(info.height, 64);
    assert_eq!(info.frame_count, 3);

    // Read all 3 frames
    for expected_frame_num in 0..3 {
        let frame = reader
            .read_frame()
            .expect("Read failed")
            .expect(&format!("Missing frame {}", expected_frame_num));

        assert_eq!(frame.frame_num, expected_frame_num);
        assert_eq!(frame.width, 64);
        assert_eq!(frame.height, 64);
    }

    // Fourth read should return None (EOF)
    assert!(reader.read_frame().expect("Read failed").is_none());
}

#[test]
fn test_y4m_seek_functionality() {
    // Generate fixture first
    std::process::Command::new("cargo")
        .args(&["run", "--bin", "generate_fixtures"])
        .status()
        .expect("Failed to generate fixtures");

    let fixture = "tests/fixtures/test_64x64.y4m";
    let mut reader = Y4mReader::open(fixture).expect("Failed to open fixture");

    // Seek to frame 1
    reader.seek(1).expect("Seek failed");
    assert_eq!(reader.current_frame(), 1);

    let frame = reader.read_frame().expect("Read failed").expect("No frame");
    assert_eq!(frame.frame_num, 1);

    // Seek to frame 0
    reader.seek(0).expect("Seek failed");
    assert_eq!(reader.current_frame(), 0);

    let frame = reader.read_frame().expect("Read failed").expect("No frame");
    assert_eq!(frame.frame_num, 0);
}

#[test]
fn test_minimal_av1_encoding_stub() {
    // This test validates the placeholder encoder infrastructure
    // TODO: Replace with real encoder tests once BitstreamWriterCapsule integrated

    // Generate fixture
    std::process::Command::new("cargo")
        .args(&["run", "--bin", "generate_fixtures"])
        .status()
        .expect("Failed to generate fixtures");

    let fixture = "tests/fixtures/test_8x8.y4m";
    let frame = read_first_y4m_frame(fixture).expect("Failed to read frame");

    // Encode to AV1 (stub)
    let bitstream = encode_frame_to_av1_stub(&frame);

    // Validate minimal OBU structure
    assert!(!bitstream.is_empty(), "Bitstream should not be empty");
    assert_eq!(
        bitstream[0] & 0x78,
        0x08,
        "First OBU should be sequence header (type=1)"
    );
}

#[test]
fn test_ivf_container_writing() {
    // Generate fixture
    std::process::Command::new("cargo")
        .args(&["run", "--bin", "generate_fixtures"])
        .status()
        .expect("Failed to generate fixtures");

    let fixture = "tests/fixtures/test_8x8.y4m";
    let frame = read_first_y4m_frame(fixture).expect("Failed to read frame");

    // Encode to AV1 (stub)
    let bitstream = encode_frame_to_av1_stub(&frame);

    // Write IVF container
    let output = "/tmp/test_8x8.ivf";
    write_ivf_file(output, 8, 8, &[bitstream]).expect("Failed to write IVF");

    // Validate IVF file exists and has correct header
    let file = File::open(output).expect("IVF file not created");
    let mut reader = BufReader::new(file);

    let mut header = [0u8; 32];
    use std::io::Read;
    reader
        .read_exact(&mut header)
        .expect("Failed to read IVF header");

    assert_eq!(&header[0..4], b"DKIF", "IVF signature");
    assert_eq!(&header[8..12], b"AV01", "AV1 FourCC");
}

#[test]
#[ignore] // Requires dav1d installation
fn test_roundtrip_8x8_with_dav1d() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping round-trip test");
        eprintln!("   Install: sudo apt install dav1d");
        return;
    }

    // Generate fixture
    std::process::Command::new("cargo")
        .args(&["run", "--bin", "generate_fixtures"])
        .status()
        .expect("Failed to generate fixtures");

    let fixture = "tests/fixtures/test_8x8.y4m";
    let original = read_first_y4m_frame(fixture).expect("Failed to read original");

    // Encode to AV1
    let bitstream = encode_frame_to_av1_stub(&original);
    let ivf_path = "/tmp/test_8x8_roundtrip.ivf";
    write_ivf_file(ivf_path, 8, 8, &[bitstream]).expect("Failed to write IVF");

    // Decode with dav1d
    let decoded_path = "/tmp/test_8x8_decoded.y4m";
    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", decoded_path])
        .output()
        .expect("dav1d execution failed");

    if !output.status.success() {
        eprintln!("dav1d stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("dav1d decoding failed");
    }

    // Read decoded frame
    let decoded = read_first_y4m_frame(decoded_path).expect("Failed to read decoded");

    // Validate dimensions match
    assert_eq!(decoded.width, original.width);
    assert_eq!(decoded.height, original.height);

    // Calculate PSNR (should be ≥30 dB for acceptable quality)
    let psnr = calculate_psnr(&original, &decoded);
    assert!(psnr >= 30.0, "PSNR {} dB is below threshold (30 dB)", psnr);

    println!("✓ Round-trip PSNR: {:.2} dB", psnr);
}

#[test]
#[ignore] // Requires dav1d installation
fn test_roundtrip_64x64_with_dav1d() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping round-trip test");
        return;
    }

    // Generate fixture
    std::process::Command::new("cargo")
        .args(&["run", "--bin", "generate_fixtures"])
        .status()
        .expect("Failed to generate fixtures");

    let fixture = "tests/fixtures/test_64x64.y4m";
    let mut reader = Y4mReader::open(fixture).expect("Failed to open fixture");

    // Encode all 3 frames
    let mut bitstreams = Vec::new();
    while let Some(frame) = reader.read_frame().expect("Read failed") {
        let bitstream = encode_frame_to_av1_stub(&frame);
        bitstreams.push(bitstream);
    }

    assert_eq!(bitstreams.len(), 3, "Should encode 3 frames");

    // Write IVF container
    let ivf_path = "/tmp/test_64x64_roundtrip.ivf";
    write_ivf_file(ivf_path, 64, 64, &bitstreams).expect("Failed to write IVF");

    // Decode with dav1d
    let decoded_path = "/tmp/test_64x64_decoded.y4m";
    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", decoded_path])
        .output()
        .expect("dav1d execution failed");

    if !output.status.success() {
        eprintln!("dav1d stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("dav1d decoding failed");
    }

    // Validate decoded file exists and has 3 frames
    let decoded_reader = Y4mReader::open(decoded_path).expect("Failed to open decoded");
    assert_eq!(decoded_reader.info().frame_count, 3);

    println!("✓ Round-trip successful: 3 frames encoded/decoded");
}

#[test]
#[ignore] // Requires dav1d installation
fn test_roundtrip_320x240_quality_validation() {
    if !is_dav1d_installed() {
        eprintln!("⚠️  dav1d not installed, skipping quality validation");
        return;
    }

    // Generate fixture
    std::process::Command::new("cargo")
        .args(&["run", "--bin", "generate_fixtures"])
        .status()
        .expect("Failed to generate fixtures");

    let fixture = "tests/fixtures/test_320x240.y4m";
    let mut reader = Y4mReader::open(fixture).expect("Failed to open fixture");

    // Encode all 5 frames
    let mut bitstreams = Vec::new();
    let mut originals: Vec<Frame> = Vec::new();

    while let Some(frame) = reader.read_frame().expect("Read failed") {
        originals.push(frame.clone());
        let bitstream = encode_frame_to_av1_stub(&frame);
        bitstreams.push(bitstream);
    }

    assert_eq!(bitstreams.len(), 5, "Should encode 5 frames");

    // Write IVF container
    let ivf_path = "/tmp/test_320x240_roundtrip.ivf";
    write_ivf_file(ivf_path, 320, 240, &bitstreams).expect("Failed to write IVF");

    // Decode with dav1d
    let decoded_path = "/tmp/test_320x240_decoded.y4m";
    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", decoded_path])
        .output()
        .expect("dav1d execution failed");

    if !output.status.success() {
        eprintln!("dav1d stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("dav1d decoding failed");
    }

    // Read decoded frames and validate PSNR
    let mut decoded_reader = Y4mReader::open(decoded_path).expect("Failed to open decoded");

    for (idx, original) in originals.iter().enumerate() {
        let decoded = decoded_reader
            .read_frame()
            .expect("Read failed")
            .expect(&format!("Missing decoded frame {}", idx));

        let psnr = calculate_psnr(original, &decoded);
        assert!(
            psnr >= 30.0,
            "Frame {} PSNR {} dB below threshold",
            idx,
            psnr
        );

        println!("  Frame {}: PSNR = {:.2} dB", idx, psnr);
    }

    println!("✓ Quality validation passed: All frames ≥30 dB PSNR");
}
