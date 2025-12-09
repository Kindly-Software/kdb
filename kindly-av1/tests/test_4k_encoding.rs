//! 4K (3840x2160) Encoding Integration Test
//!
//! Tests the kindly-av1 encoder at 4K resolution with dav1d validation.

use std::fs::write;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use kindly_av1::encoder::{EncoderWiringCapsule, EncoderSubCapsules};
use kindly_av1::file::{Frame, FrameReader, Y4mReader};

/// Load frames from Y4M file
fn load_y4m_frames<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<Frame>> {
    let mut reader = Y4mReader::open(path).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("Y4M open failed: {}", e))
    })?;

    let mut frames = Vec::new();
    while let Some(frame) = reader.read_frame().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("Y4M read failed: {}", e))
    })? {
        frames.push(frame);
    }

    Ok(frames)
}

#[test]
fn test_4k_load_y4m_fixture() {
    let fixture = "tests/fixtures/test_4k.y4m";

    if !Path::new(fixture).exists() {
        println!("SKIPPED: 4K fixture not found at {}", fixture);
        return;
    }

    println!("Loading 4K Y4M fixture...");
    let start = Instant::now();
    let frames = load_y4m_frames(fixture).expect("Failed to load 4K fixture");
    let load_time_ms = start.elapsed().as_secs_f64() * 1000.0;

    println!("✓ Loaded {} frames in {:.2} ms", frames.len(), load_time_ms);

    // Verify frame count
    assert_eq!(frames.len(), 5, "Expected 5 frames in 4K fixture");

    // Verify dimensions (4K = 3840×2160)
    for (idx, frame) in frames.iter().enumerate() {
        assert_eq!(frame.width, 3840, "Frame {} width mismatch", idx);
        assert_eq!(frame.height, 2160, "Frame {} height mismatch", idx);

        // Validate plane sizes
        let y_size = (frame.width * frame.height) as usize;
        let uv_size = ((frame.width / 2) * (frame.height / 2)) as usize; // 4:2:0

        assert_eq!(frame.y.len(), y_size, "Frame {} Y plane size mismatch", idx);
        assert_eq!(frame.u.len(), uv_size, "Frame {} U plane size mismatch", idx);
        assert_eq!(frame.v.len(), uv_size, "Frame {} V plane size mismatch", idx);
    }

    // Performance validation: should load in <1s
    assert!(load_time_ms < 1000.0, "Load time {} ms exceeds 1s", load_time_ms);
}

#[test]
fn test_4k_encode_single_frame() {
    let fixture = "tests/fixtures/test_4k.y4m";

    if !Path::new(fixture).exists() {
        println!("SKIPPED: 4K fixture not found");
        return;
    }

    // Load first frame
    println!("Loading 4K frames...");
    let frames = load_y4m_frames(fixture).expect("Failed to load 4K fixture");
    assert!(!frames.is_empty(), "No frames loaded");

    let frame = &frames[0];
    println!("✓ Loaded frame: {}x{}", frame.width, frame.height);

    // Create encoder
    println!("Creating encoder capsules...");
    let mut wiring = EncoderWiringCapsule::with_params(
        frame.width,
        frame.height,
        28, // CRF quality
        3,  // Speed preset
    );
    let mut sub_capsules = wiring.initialize(frame.width, frame.height, 28, 3)
        .expect("Failed to initialize encoder");

    // Encode frame (pass Y plane data)
    println!("Encoding 4K frame...");
    let start = Instant::now();

    let output = wiring
        .encode_frame(&frame.y, &mut sub_capsules)
        .expect("Failed to encode 4K frame");

    let encode_time_ms = start.elapsed().as_secs_f64() * 1000.0;

    println!("✓ Encoded {} bytes in {:.2} ms", output.len(), encode_time_ms);
    println!("  FPS: {:.2}", 1000.0 / encode_time_ms);

    // Validate output
    assert!(output.len() > 0, "Encoded output is empty");
    assert!(output.len() < frame.y.len(), "Compressed output should be smaller than input");

    // Write output for dav1d validation
    let output_path = "/tmp/test_4k_encoded.ivf";
    write(output_path, &output).expect("Failed to write encoded output");

    println!("✓ Wrote encoded output to {}", output_path);

    // Validate with dav1d
    println!("Validating with dav1d decoder...");
    let result = Command::new("dav1d")
        .args(&["-i", output_path, "-o", "/dev/null"])
        .output()
        .expect("Failed to run dav1d");

    let stderr = String::from_utf8_lossy(&result.stderr);
    println!("dav1d output:\n{}", stderr);

    // Check for successful decode
    assert!(result.status.success(), "dav1d failed to decode 4K frame");
    assert!(stderr.contains("Decoded 1/1 frames"), "dav1d did not decode frame");

    println!("✓ dav1d successfully validated 4K encoded frame!");
}

#[test]
#[ignore] // Heavy test - run manually
fn test_4k_encode_all_frames() {
    let fixture = "tests/fixtures/test_4k.y4m";

    if !Path::new(fixture).exists() {
        println!("SKIPPED: 4K fixture not found");
        return;
    }

    // Load all frames
    println!("Loading 4K frames...");
    let frames = load_y4m_frames(fixture).expect("Failed to load 4K fixture");
    println!("✓ Loaded {} frames", frames.len());

    // Create encoder
    let mut wiring = EncoderWiringCapsule::with_params(
        frames[0].width,
        frames[0].height,
        28, // CRF quality
        3,  // Speed preset
    );
    let mut sub_capsules = wiring.initialize(frames[0].width, frames[0].height, 28, 3)
        .expect("Failed to initialize encoder");

    let mut total_bytes = 0;
    let mut total_time_ms = 0.0;

    // Encode all frames
    for (idx, frame) in frames.iter().enumerate() {
        println!("Encoding frame {}/{}...", idx + 1, frames.len());

        let start = Instant::now();
        let output = wiring
            .encode_frame(&frame.y, &mut sub_capsules)
            .expect(&format!("Failed to encode frame {}", idx));
        let encode_time_ms = start.elapsed().as_secs_f64() * 1000.0;

        total_bytes += output.len();
        total_time_ms += encode_time_ms;

        println!("  Frame {}: {} bytes in {:.2} ms ({:.2} fps)",
                 idx + 1, output.len(), encode_time_ms, 1000.0 / encode_time_ms);
    }

    let avg_time_ms = total_time_ms / frames.len() as f64;
    let avg_fps = 1000.0 / avg_time_ms;

    println!("\n=== 4K Encoding Summary ===");
    println!("Frames: {}", frames.len());
    println!("Total bytes: {}", total_bytes);
    println!("Total time: {:.2} ms", total_time_ms);
    println!("Avg time per frame: {:.2} ms", avg_time_ms);
    println!("Avg FPS: {:.2}", avg_fps);
    println!("==========================");
}

#[test]
fn test_4k_frame_properties() {
    let fixture = "tests/fixtures/test_4k.y4m";

    if !Path::new(fixture).exists() {
        println!("SKIPPED: 4K fixture not found");
        return;
    }

    let frames = load_y4m_frames(fixture).expect("Failed to load 4K fixture");
    let frame = &frames[0];

    // Verify frame properties
    println!("Frame properties:");
    println!("  Width: {}", frame.width);
    println!("  Height: {}", frame.height);
    println!("  Y plane: {} bytes", frame.y.len());
    println!("  U plane: {} bytes", frame.u.len());
    println!("  V plane: {} bytes", frame.v.len());
    println!("  Total: {} bytes", frame.y.len() + frame.u.len() + frame.v.len());

    // 4K dimensions
    assert_eq!(frame.width, 3840);
    assert_eq!(frame.height, 2160);

    // Plane sizes (4:2:0 subsampling)
    assert_eq!(frame.y.len(), 3840 * 2160);
    assert_eq!(frame.u.len(), (3840 / 2) * (2160 / 2));
    assert_eq!(frame.v.len(), (3840 / 2) * (2160 / 2));

    // Total size = Y + U + V = (w*h) + 2*(w/2*h/2) = w*h*1.5
    let expected_total = (3840 * 2160 * 3) / 2;
    let actual_total = frame.y.len() + frame.u.len() + frame.v.len();
    assert_eq!(actual_total, expected_total);

    println!("✓ All 4K frame properties validated");
}
