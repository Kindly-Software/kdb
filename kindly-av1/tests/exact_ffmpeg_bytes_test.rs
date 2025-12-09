//! Test using exact ffmpeg output bytes

/// Test that exact ffmpeg bytes work with dav1d
#[test]
fn test_exact_ffmpeg_lossless_bytes() {
    use std::fs::File;
    use std::io::Write;
    use std::process::Command;

    // Exact bytes from: ffmpeg -y -f lavfi -i "color=gray:size=64x64:rate=30" -frames:v 1
    //                   -c:v libaom-av1 -strict experimental -cpu-used 8 -crf 0 -lossless 1
    // OBU bytes (without IVF header)
    let obu_bytes: &[u8] = &[
        // Temporal Delimiter: type=2, size=0
        0x12, 0x00, // Sequence Header: type=1, size=10
        0x0a, 0x0a, // Seq header payload (10 bytes)
        0x00, 0x00, 0x00, 0x02, 0xaf, 0xff, 0x9b, 0x5f, 0x20, 0x08,
        // Frame OBU: type=6, size=10
        0x32, 0x0a, // Frame payload (10 bytes)
        0x10, 0x00, 0x80, 0x00, 0x00, 0x4a, 0x7d, 0xf7, 0xff, 0xff,
    ];

    // Create IVF file
    let ivf_path = "/tmp/exact_ffmpeg_lossless_test.ivf";
    let mut file = File::create(ivf_path).unwrap();

    // IVF header (32 bytes)
    let mut ivf_header = [0u8; 32];
    ivf_header[0..4].copy_from_slice(b"DKIF"); // Signature
    ivf_header[4..6].copy_from_slice(&0u16.to_le_bytes()); // Version
    ivf_header[6..8].copy_from_slice(&32u16.to_le_bytes()); // Header length
    ivf_header[8..12].copy_from_slice(b"AV01"); // FourCC
    ivf_header[12..14].copy_from_slice(&64u16.to_le_bytes()); // Width
    ivf_header[14..16].copy_from_slice(&64u16.to_le_bytes()); // Height
    ivf_header[16..20].copy_from_slice(&30u32.to_le_bytes()); // Framerate num
    ivf_header[20..24].copy_from_slice(&1u32.to_le_bytes()); // Framerate den
    ivf_header[24..28].copy_from_slice(&1u32.to_le_bytes()); // Frame count
                                                             // Bytes 28-31: reserved

    file.write_all(&ivf_header).unwrap();

    // Frame header (12 bytes)
    let frame_size = obu_bytes.len() as u32;
    file.write_all(&frame_size.to_le_bytes()).unwrap(); // Frame size
    file.write_all(&0u64.to_le_bytes()).unwrap(); // Timestamp

    // OBU data
    file.write_all(obu_bytes).unwrap();
    drop(file);

    println!("Wrote {} OBU bytes to {}", obu_bytes.len(), ivf_path);

    // Run dav1d
    let output = Command::new("dav1d")
        .args(["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .expect("dav1d not found");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("dav1d exit status: {:?}", output.status);
    println!("dav1d stderr: {}", stderr);

    assert!(
        output.status.success(),
        "dav1d should succeed with exact ffmpeg bytes"
    );
    assert!(stderr.contains("Decoded 1/1"), "Should decode 1 frame");
}
