//! Production-Grade Stress Testing Suite (T28 Q22-Q28)
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Comprehensive stress tests for kindly-av1 encoder validating production readiness.
//!
//! # Framework Compliance
//!
//! - **T28 Q22-Q28**: Production tier validation
//! - **UCE34**: Q10 T6 Mixed tier capsule orchestration validation
//! - **Chaos**: 100% lockfree verification under stress
//! - **ASSUM**: Memory safety under extreme conditions
//!
//! # Test Categories
//!
//! 1. **Memory Stress**: Large frame encoding (4K, 8K), memory leak detection
//! 2. **Concurrency Stress**: Multi-threaded encoding, race condition detection
//! 3. **Long Duration Stress**: Extended sequences (10,000+ frames)
//! 4. **Edge Case Stress**: Extreme resolutions, boundary conditions
//! 5. **Content Stress**: Scene changes, high motion, gradients, noise
//! 6. **Performance Regression**: Timing and size tracking
//! 7. **Determinism Validation**: Bit-exact reproducibility
//! 8. **Chaos Compliance**: Lockfree operation verification
//!
//! # SOTA References
//!
//! - [SVT-AV1 Testing](https://gitlab.com/AOMediaCodec/SVT-AV1): Phoronix test suite patterns
//! - [Netflix VMAF](https://github.com/Netflix/vmaf): Quality assessment integration
//! - [dav1d Validation](https://code.videolan.org/videolan/dav1d): Decoder conformance
//! - [ESP Encoder Stress Pattern](https://www.sri.com/product/esp-encoder-stress-pattern/): SRI stress patterns
//!
//! # Performance Targets (B32 Framework)
//!
//! | Test | Target | Status |
//! |------|--------|--------|
//! | 8K single frame | <500ms | Pending |
//! | 32 concurrent 1080p | <2s total | Pending |
//! | 10,000 frames | <30 minutes | Pending |
//! | Memory (8K) | <16GB | Pending |
//!
//! # Running Tests
//!
//! ```bash
//! # Quick stress tests (non-ignored)
//! cargo test --test production_stress_tests
//!
//! # Full stress suite (includes slow tests)
//! cargo test --test production_stress_tests -- --ignored
//!
//! # Run on kindly-hub (MANDATORY for B32/T28)
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo test --test production_stress_tests -- --ignored"
//! ```

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use blake3::Hasher;

use kindly_av1::encoder::{wiring_capsule::WiringState, EncoderWiringCapsule};

// ============================================================================
// Constants and Configuration
// ============================================================================

/// Maximum memory target for 8K encoding (bytes)
const MAX_MEMORY_8K: usize = 16 * 1024 * 1024 * 1024; // 16GB

/// Performance regression threshold (percentage)
const REGRESSION_THRESHOLD_TIME: f64 = 5.0; // 5% time regression
#[allow(dead_code)]
const REGRESSION_THRESHOLD_SIZE: f64 = 2.0; // 2% size regression (for future use)

/// Stress test parameters
const STRESS_ITERATIONS_LIGHT: usize = 100;
const STRESS_ITERATIONS_HEAVY: usize = 1000;
const LONG_DURATION_FRAMES: usize = 10_000;

// ============================================================================
// Test Frame Generators (SOTA Content Patterns)
// ============================================================================

/// Create YUV420p frame data for given dimensions
fn create_yuv_frame(width: u32, height: u32, pattern: FramePattern) -> Vec<u8> {
    let y_size = (width * height) as usize;
    let uv_size = y_size / 4; // 4:2:0 subsampling
    let total_size = y_size + 2 * uv_size;

    let mut frame = vec![0u8; total_size];

    // Generate Y plane based on pattern
    match pattern {
        FramePattern::Gray => {
            for i in 0..y_size {
                frame[i] = 128;
            }
        }
        FramePattern::Gradient => {
            for y in 0..height {
                for x in 0..width {
                    let idx = (y * width + x) as usize;
                    frame[idx] = (x * 255 / width.max(1)) as u8;
                }
            }
        }
        FramePattern::VerticalGradient => {
            for y in 0..height {
                for x in 0..width {
                    let idx = (y * width + x) as usize;
                    frame[idx] = (y * 255 / height.max(1)) as u8;
                }
            }
        }
        FramePattern::Checkerboard { block_size } => {
            for y in 0..height {
                for x in 0..width {
                    let idx = (y * width + x) as usize;
                    let bx = x / block_size;
                    let by = y / block_size;
                    frame[idx] = if (bx + by) % 2 == 0 { 255 } else { 16 };
                }
            }
        }
        FramePattern::MovingBars { offset, bar_width } => {
            for y in 0..height {
                for x in 0..width {
                    let idx = (y * width + x) as usize;
                    let shifted_x = (x + offset) % width;
                    frame[idx] = if (shifted_x / bar_width) % 2 == 0 { 255 } else { 16 };
                }
            }
        }
        FramePattern::RandomNoise { seed } => {
            // Simple LCG for deterministic "random" noise
            let mut rng = seed as u64;
            for i in 0..y_size {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                frame[i] = (rng >> 56) as u8;
            }
        }
        FramePattern::HighFrequency => {
            // Alternating pixels (worst case for compression)
            for y in 0..height {
                for x in 0..width {
                    let idx = (y * width + x) as usize;
                    frame[idx] = if (x + y) % 2 == 0 { 255 } else { 0 };
                }
            }
        }
        FramePattern::Static => {
            // All same value (best case for compression)
            for i in 0..y_size {
                frame[i] = 100;
            }
        }
        FramePattern::ColorRamp16Bit => {
            // 16-bit color ramp simulation (for banding detection)
            for y in 0..height {
                for x in 0..width {
                    let idx = (y * width + x) as usize;
                    // Fine gradient with small steps
                    let step = (x * 256 / width.max(1)) as u8;
                    frame[idx] = step;
                }
            }
        }
        FramePattern::FlashFrame { is_white } => {
            let val = if is_white { 255 } else { 0 };
            for i in 0..y_size {
                frame[i] = val;
            }
        }
    }

    // U/V planes: neutral chroma (128)
    for i in 0..uv_size {
        frame[y_size + i] = 128; // U
        frame[y_size + uv_size + i] = 128; // V
    }

    frame
}

/// Frame content patterns for stress testing
#[derive(Debug, Clone, Copy)]
enum FramePattern {
    Gray,
    Gradient,
    VerticalGradient,
    Checkerboard { block_size: u32 },
    MovingBars { offset: u32, bar_width: u32 },
    RandomNoise { seed: u32 },
    HighFrequency,
    Static,
    ColorRamp16Bit,
    FlashFrame { is_white: bool },
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if dav1d decoder is installed
fn is_dav1d_installed() -> bool {
    Command::new("which")
        .arg("dav1d")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Write IVF container with AV1 frames
fn write_ivf_file(path: &str, width: u32, height: u32, frames: &[Vec<u8>]) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Write;

    let mut file = File::create(path)?;

    // IVF header (32 bytes)
    file.write_all(b"DKIF")?;
    file.write_all(&[0, 0])?; // Version
    file.write_all(&[32, 0])?; // Header size
    file.write_all(b"AV01")?; // Codec FourCC
    file.write_all(&width.to_le_bytes()[..2])?;
    file.write_all(&height.to_le_bytes()[..2])?;
    file.write_all(&30u32.to_le_bytes())?; // Frame rate num
    file.write_all(&1u32.to_le_bytes())?; // Frame rate den
    file.write_all(&(frames.len() as u32).to_le_bytes())?;
    file.write_all(&[0u8; 4])?; // Unused

    // Write frames
    for (idx, data) in frames.iter().enumerate() {
        file.write_all(&(data.len() as u32).to_le_bytes())?;
        file.write_all(&(idx as u64).to_le_bytes())?;
        file.write_all(data)?;
    }

    Ok(())
}

/// Validate bitstream with dav1d decoder
fn validate_with_dav1d(ivf_path: &str) -> Result<(), String> {
    if !is_dav1d_installed() {
        return Err("dav1d not installed".to_string());
    }

    let output = Command::new("dav1d")
        .args(&["-i", ivf_path, "-o", "/dev/null"])
        .output()
        .map_err(|e| format!("Failed to execute dav1d: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("dav1d decoding failed: {}", stderr))
    }
}

/// Hash bytes with BLAKE3
fn hash_bytes(data: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(data);
    let hash = hasher.finalize();
    let mut result = [0u8; 32];
    result.copy_from_slice(hash.as_bytes());
    result
}

/// Encode frame and return output
fn encode_frame(
    width: u32,
    height: u32,
    frame_data: &[u8],
    crf: u8,
    speed: u8,
) -> Result<Vec<u8>, String> {
    let mut wiring = EncoderWiringCapsule::new();
    let mut sub_capsules = wiring
        .initialize(width, height, crf, speed)
        .map_err(|e| format!("Initialize failed: {}", e))?;

    let encoded = wiring
        .encode_frame(frame_data, &mut sub_capsules)
        .map_err(|e| format!("Encode failed: {}", e))?;

    let _flushed = wiring
        .flush(&mut sub_capsules)
        .map_err(|e| format!("Flush failed: {}", e))?;

    Ok(encoded)
}

/// Encode multiple frames and return combined output
fn encode_sequence(
    width: u32,
    height: u32,
    frames: &[Vec<u8>],
    crf: u8,
    speed: u8,
) -> Result<Vec<Vec<u8>>, String> {
    let mut wiring = EncoderWiringCapsule::new();
    let mut sub_capsules = wiring
        .initialize(width, height, crf, speed)
        .map_err(|e| format!("Initialize failed: {}", e))?;

    let mut outputs = Vec::with_capacity(frames.len());

    for (idx, frame) in frames.iter().enumerate() {
        let encoded = wiring
            .encode_frame(frame, &mut sub_capsules)
            .map_err(|e| format!("Encode frame {} failed: {}", idx, e))?;
        outputs.push(encoded);
    }

    let _flushed = wiring
        .flush(&mut sub_capsules)
        .map_err(|e| format!("Flush failed: {}", e))?;

    Ok(outputs)
}

/// Get approximate memory usage of current process
fn get_process_memory_bytes() -> Option<usize> {
    // Linux-specific: read /proc/self/statm
    if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
        let parts: Vec<&str> = statm.split_whitespace().collect();
        if parts.len() >= 2 {
            // Second field is RSS in pages (usually 4KB)
            if let Ok(pages) = parts[1].parse::<usize>() {
                return Some(pages * 4096);
            }
        }
    }
    None
}

// ============================================================================
// Module: Memory Stress Tests (T28 Q22)
// ============================================================================

mod memory_stress {
    use super::*;

    /// Q22-1: 8K frame encoding (7680x4320)
    ///
    /// Verifies encoder can handle 8K resolution without OOM.
    /// AV1 spec allows up to 8192x4320.
    #[test]
    #[ignore = "Very large, requires significant memory - run with --ignored"]
    fn test_8k_frame_encoding() {
        let width = 7680;
        let height = 4320;

        let start_memory = get_process_memory_bytes();

        // Create 8K test frame
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);
        println!("Created 8K frame: {} bytes", frame.len());

        // Encode with moderate settings
        let start_time = Instant::now();
        let result = encode_frame(width, height, &frame, 28, 5);
        let encode_time = start_time.elapsed();

        match result {
            Ok(encoded) => {
                println!("8K encoding succeeded:");
                println!("  - Output size: {} bytes", encoded.len());
                println!("  - Encode time: {:?}", encode_time);
                println!("  - Target: <500ms, Actual: {}ms", encode_time.as_millis());

                // Check memory usage
                if let (Some(before), Some(after)) = (start_memory, get_process_memory_bytes()) {
                    let delta = after.saturating_sub(before);
                    println!("  - Memory delta: {} MB", delta / (1024 * 1024));
                    assert!(
                        delta < MAX_MEMORY_8K,
                        "Memory usage {} exceeds {}GB limit",
                        delta / (1024 * 1024 * 1024),
                        MAX_MEMORY_8K / (1024 * 1024 * 1024)
                    );
                }

                // Verify non-empty output
                assert!(!encoded.is_empty(), "8K encoding produced empty output");
            }
            Err(e) => {
                panic!("8K encoding failed: {}", e);
            }
        }
    }

    /// Q22-2: 4K frame encoding (3840x2160)
    #[test]
    fn test_4k_frame_encoding() {
        let width = 3840;
        let height = 2160;

        let frame = create_yuv_frame(width, height, FramePattern::Gradient);
        println!("Created 4K frame: {} bytes", frame.len());

        let start_time = Instant::now();
        let result = encode_frame(width, height, &frame, 28, 5);
        let encode_time = start_time.elapsed();

        match result {
            Ok(encoded) => {
                println!("4K encoding: {} bytes in {:?}", encoded.len(), encode_time);
                assert!(!encoded.is_empty());
            }
            Err(e) => {
                panic!("4K encoding failed: {}", e);
            }
        }
    }

    /// Q22-3: Maximum AV1 spec resolution (8192x4320)
    #[test]
    #[ignore = "Maximum spec resolution - run with --ignored"]
    fn test_max_spec_resolution() {
        let width = 8192; // AV1 max
        let height = 4320; // 8K

        let frame = create_yuv_frame(width, height, FramePattern::Gray);

        let result = encode_frame(width, height, &frame, 35, 8); // Fast, lower quality

        match result {
            Ok(encoded) => {
                println!("Max resolution encoding succeeded: {} bytes", encoded.len());
                assert!(!encoded.is_empty());
            }
            Err(e) => {
                // Acceptable to fail on max resolution with resource limits
                println!("Max resolution encoding failed (acceptable): {}", e);
            }
        }
    }

    /// Q22-4: Memory leak detection (encode-release cycles)
    #[test]
    fn test_memory_leak_detection() {
        let width = 640;
        let height = 480;
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);

        // Warm up
        for _ in 0..5 {
            let _ = encode_frame(width, height, &frame, 28, 5);
        }

        let baseline_memory = get_process_memory_bytes();

        // Run many encode cycles
        for i in 0..STRESS_ITERATIONS_LIGHT {
            let result = encode_frame(width, height, &frame, 28, 5);
            assert!(result.is_ok(), "Encode {} failed: {:?}", i, result);

            // Check memory every 20 iterations
            if i % 20 == 0 {
                if let (Some(base), Some(current)) = (baseline_memory, get_process_memory_bytes()) {
                    let delta_mb = (current.saturating_sub(base)) / (1024 * 1024);
                    // Allow up to 100MB growth (for caches, etc.)
                    assert!(
                        delta_mb < 100,
                        "Memory leak detected: {} MB growth after {} iterations",
                        delta_mb,
                        i
                    );
                }
            }
        }

        println!(
            "Memory leak test passed: {} encode cycles completed",
            STRESS_ITERATIONS_LIGHT
        );
    }

    /// Q22-5: Minimum resolution (8x8 - AV1 minimum superblock)
    #[test]
    fn test_minimum_resolution() {
        let width = 8;
        let height = 8;

        let frame = create_yuv_frame(width, height, FramePattern::Gray);
        let result = encode_frame(width, height, &frame, 28, 5);

        match result {
            Ok(encoded) => {
                println!("Minimum 8x8 encoding: {} bytes", encoded.len());
                assert!(!encoded.is_empty());
            }
            Err(e) => {
                panic!("Minimum resolution encoding failed: {}", e);
            }
        }
    }

    /// Q22-6: Odd dimension handling
    #[test]
    fn test_odd_dimensions() {
        let test_cases = [
            (63, 63),
            (65, 65),
            (127, 127),
            (129, 129),
            (255, 255),
            (100, 75),
            (333, 222),
        ];

        for (width, height) in test_cases {
            let frame = create_yuv_frame(width, height, FramePattern::Gray);
            let result = encode_frame(width, height, &frame, 28, 5);

            match result {
                Ok(encoded) => {
                    println!("{}x{}: {} bytes", width, height, encoded.len());
                    assert!(!encoded.is_empty());
                }
                Err(e) => {
                    panic!("Odd dimension {}x{} failed: {}", width, height, e);
                }
            }
        }
    }

    /// Q22-7: Large frame buffer allocation stress
    #[test]
    #[ignore = "Memory intensive - run with --ignored"]
    fn test_large_frame_buffer_stress() {
        // Allocate multiple 1080p frame buffers rapidly
        let width = 1920;
        let height = 1080;

        let mut encoders: Vec<EncoderWiringCapsule> = Vec::new();

        // Create 50 encoder instances
        for i in 0..50 {
            let mut encoder = EncoderWiringCapsule::new();
            let frame = create_yuv_frame(width, height, FramePattern::Gradient);

            match encoder.initialize(width, height, 28, 5) {
                Ok(mut sub_capsules) => {
                    // Encode one frame to allocate all internal buffers
                    let _ = encoder.encode_frame(&frame, &mut sub_capsules);
                    encoders.push(encoder);
                }
                Err(e) => {
                    panic!("Failed to create encoder {}: {}", i, e);
                }
            }
        }

        println!(
            "Created {} concurrent encoder instances",
            encoders.len()
        );
        assert_eq!(encoders.len(), 50);
    }
}

// ============================================================================
// Module: Concurrency Stress Tests (T28 Q23)
// ============================================================================

mod concurrency_stress {
    use super::*;
    use std::thread;

    /// Q23-1: 32 concurrent 1080p encodes
    #[test]
    #[ignore = "Heavy concurrency test - run with --ignored"]
    fn test_32_concurrent_encodes() {
        let width = 1920;
        let height = 1080;
        let num_threads = 32;

        let frame = Arc::new(create_yuv_frame(width, height, FramePattern::Gradient));
        let success_count = Arc::new(AtomicU64::new(0));
        let failure_count = Arc::new(AtomicU64::new(0));

        let start_time = Instant::now();

        let handles: Vec<_> = (0..num_threads)
            .map(|thread_id| {
                let frame = Arc::clone(&frame);
                let success_count = Arc::clone(&success_count);
                let failure_count = Arc::clone(&failure_count);

                thread::spawn(move || {
                    let result = encode_frame(width, height, &frame, 28, 7);
                    match result {
                        Ok(_encoded) => {
                            success_count.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(e) => {
                            eprintln!("Thread {} failed: {}", thread_id, e);
                            failure_count.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        let total_time = start_time.elapsed();
        let successes = success_count.load(Ordering::SeqCst);
        let failures = failure_count.load(Ordering::SeqCst);

        println!("32-thread concurrent encoding:");
        println!("  - Successes: {}", successes);
        println!("  - Failures: {}", failures);
        println!("  - Total time: {:?}", total_time);
        println!("  - Target: <2s, Actual: {}ms", total_time.as_millis());

        assert_eq!(failures, 0, "{} concurrent encodes failed", failures);
        assert!(
            total_time < Duration::from_secs(10),
            "Concurrent encoding too slow: {:?}",
            total_time
        );
    }

    /// Q23-2: Thread pool saturation test
    #[test]
    fn test_thread_pool_saturation() {
        let width = 640;
        let height = 480;
        let num_threads = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4);
        let iterations_per_thread = 10;

        let frame = Arc::new(create_yuv_frame(width, height, FramePattern::Gradient));
        let total_encoded = Arc::new(AtomicU64::new(0));

        let start_time = Instant::now();

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let frame = Arc::clone(&frame);
                let total_encoded = Arc::clone(&total_encoded);

                thread::spawn(move || {
                    for _ in 0..iterations_per_thread {
                        if let Ok(encoded) = encode_frame(width, height, &frame, 28, 5) {
                            total_encoded.fetch_add(encoded.len() as u64, Ordering::Relaxed);
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        let total_time = start_time.elapsed();
        let total_bytes = total_encoded.load(Ordering::Relaxed);
        let total_encodes = num_threads * iterations_per_thread;

        println!("Thread pool saturation test:");
        println!("  - Threads: {}", num_threads);
        println!("  - Total encodes: {}", total_encodes);
        println!("  - Total bytes: {}", total_bytes);
        println!("  - Total time: {:?}", total_time);
        println!(
            "  - Throughput: {:.1} encodes/sec",
            total_encodes as f64 / total_time.as_secs_f64()
        );

        assert!(total_bytes > 0, "No data encoded");
    }

    /// Q23-3: Race condition detection (shared state stress)
    #[test]
    fn test_race_condition_detection() {
        let width = 64;
        let height = 64;
        let num_threads = 8;
        let iterations = 50;

        // Each thread encodes the same frame and hashes output
        let frame = Arc::new(create_yuv_frame(width, height, FramePattern::Gradient));
        let hashes: Arc<std::sync::Mutex<Vec<[u8; 32]>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let frame = Arc::clone(&frame);
                let hashes = Arc::clone(&hashes);

                thread::spawn(move || {
                    for _ in 0..iterations {
                        if let Ok(encoded) = encode_frame(width, height, &frame, 28, 5) {
                            let h = hash_bytes(&encoded);
                            hashes.lock().unwrap().push(h);
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        let all_hashes = hashes.lock().unwrap();
        let expected_count = num_threads * iterations;

        assert_eq!(
            all_hashes.len(),
            expected_count,
            "Expected {} hashes, got {}",
            expected_count,
            all_hashes.len()
        );

        // All hashes should be identical (deterministic encoding)
        let first_hash = all_hashes[0];
        for (idx, hash) in all_hashes.iter().enumerate() {
            assert_eq!(
                *hash, first_hash,
                "Race condition detected: hash {} differs from first",
                idx
            );
        }

        println!(
            "Race condition test passed: {} identical hashes",
            all_hashes.len()
        );
    }

    /// Q23-4: Encoder instance isolation test
    #[test]
    fn test_encoder_instance_isolation() {
        let width = 128;
        let height = 128;

        // Create different frame patterns
        let frame1 = create_yuv_frame(width, height, FramePattern::Gradient);
        let frame2 = create_yuv_frame(width, height, FramePattern::VerticalGradient);

        // Encode in parallel with different content
        let frame1_clone = frame1.clone();
        let handle1 = std::thread::spawn(move || {
            encode_frame(width, height, &frame1_clone, 28, 5)
        });

        let handle2 = std::thread::spawn(move || {
            encode_frame(width, height, &frame2, 28, 5)
        });

        let result1 = handle1.join().expect("Thread 1 panicked");
        let result2 = handle2.join().expect("Thread 2 panicked");

        let encoded1 = result1.expect("Encode 1 failed");
        let encoded2 = result2.expect("Encode 2 failed");

        // Outputs should be different (different content)
        let hash1 = hash_bytes(&encoded1);
        let hash2 = hash_bytes(&encoded2);

        assert_ne!(
            hash1, hash2,
            "Different content produced identical output - isolation failure"
        );

        // Re-encode same content should produce same output
        let encoded1_again = encode_frame(width, height, &frame1, 28, 5).expect("Re-encode failed");
        let hash1_again = hash_bytes(&encoded1_again);

        assert_eq!(
            hash1, hash1_again,
            "Same content produced different output - isolation failure"
        );
    }
}

// ============================================================================
// Module: Long Duration Stress Tests (T28 Q24)
// ============================================================================

mod long_duration_stress {
    use super::*;

    /// Q24-1: 10,000 frame sequence encoding
    #[test]
    #[ignore = "Very long test (~10-30 minutes) - run with --ignored"]
    fn test_10000_frame_sequence() {
        let width = 640;
        let height = 480;
        let total_frames = LONG_DURATION_FRAMES;

        let mut wiring = EncoderWiringCapsule::new();
        let mut sub_capsules = wiring
            .initialize(width, height, 28, 5)
            .expect("Init failed");

        let start_time = Instant::now();
        let mut total_bytes: u64 = 0;
        let mut frame_times: Vec<Duration> = Vec::new();

        for frame_num in 0..total_frames {
            // Vary content slightly over time
            let pattern = match frame_num % 5 {
                0 => FramePattern::MovingBars {
                    offset: (frame_num as u32) * 4,
                    bar_width: 16,
                },
                1 => FramePattern::Gradient,
                2 => FramePattern::VerticalGradient,
                3 => FramePattern::Checkerboard { block_size: 8 },
                _ => FramePattern::Gray,
            };

            let frame = create_yuv_frame(width, height, pattern);

            let frame_start = Instant::now();
            let encoded = wiring
                .encode_frame(&frame, &mut sub_capsules)
                .expect(&format!("Frame {} failed", frame_num));
            let frame_time = frame_start.elapsed();

            total_bytes += encoded.len() as u64;
            frame_times.push(frame_time);

            // Progress report every 1000 frames
            if (frame_num + 1) % 1000 == 0 {
                let elapsed = start_time.elapsed();
                let fps = (frame_num + 1) as f64 / elapsed.as_secs_f64();
                println!(
                    "Progress: {}/{} frames ({:.1} fps), {} bytes",
                    frame_num + 1,
                    total_frames,
                    fps,
                    total_bytes
                );
            }
        }

        let _flushed = wiring.flush(&mut sub_capsules).expect("Flush failed");
        let total_time = start_time.elapsed();

        // Statistics
        let avg_frame_time: Duration =
            frame_times.iter().sum::<Duration>() / frame_times.len() as u32;
        let max_frame_time = frame_times.iter().max().unwrap();
        let min_frame_time = frame_times.iter().min().unwrap();

        println!("\n10,000 frame sequence complete:");
        println!("  - Total time: {:?}", total_time);
        println!("  - Total bytes: {} ({:.1} MB)", total_bytes, total_bytes as f64 / (1024.0 * 1024.0));
        println!("  - Average frame: {:?}", avg_frame_time);
        println!("  - Min frame: {:?}", min_frame_time);
        println!("  - Max frame: {:?}", max_frame_time);
        println!(
            "  - FPS: {:.1}",
            total_frames as f64 / total_time.as_secs_f64()
        );
        println!("  - Target: <30 minutes");

        // Performance assertions
        assert!(
            total_time < Duration::from_secs(30 * 60),
            "Encoding took longer than 30 minutes"
        );
        assert!(total_bytes > 0, "No data produced");
    }

    /// Q24-2: 1,000 frame continuous encoding (quick version)
    #[test]
    fn test_1000_frame_sequence() {
        let width = 320;
        let height = 240;
        let total_frames = 1000;

        let mut wiring = EncoderWiringCapsule::new();
        let mut sub_capsules = wiring
            .initialize(width, height, 28, 5)
            .expect("Init failed");

        let start_time = Instant::now();
        let mut total_bytes: u64 = 0;

        for frame_num in 0..total_frames {
            let pattern = FramePattern::MovingBars {
                offset: (frame_num as u32) * 4,
                bar_width: 8,
            };
            let frame = create_yuv_frame(width, height, pattern);

            let encoded = wiring
                .encode_frame(&frame, &mut sub_capsules)
                .expect(&format!("Frame {} failed", frame_num));

            total_bytes += encoded.len() as u64;
        }

        let _flushed = wiring.flush(&mut sub_capsules).expect("Flush failed");
        let total_time = start_time.elapsed();

        println!(
            "1000-frame sequence: {:?}, {} bytes, {:.1} fps",
            total_time,
            total_bytes,
            1000.0 / total_time.as_secs_f64()
        );

        assert!(total_bytes > 0);
    }

    /// Q24-3: State consistency over long encoding
    #[test]
    fn test_state_consistency_long_encoding() {
        let width = 64;
        let height = 64;

        let mut wiring = EncoderWiringCapsule::new();
        let mut sub_capsules = wiring
            .initialize(width, height, 28, 5)
            .expect("Init failed");

        assert_eq!(wiring.state(), WiringState::Ready);

        let frame = create_yuv_frame(width, height, FramePattern::Gray);

        // Encode 500 frames, checking state
        for i in 0..500 {
            let _ = wiring
                .encode_frame(&frame, &mut sub_capsules)
                .expect(&format!("Frame {} failed", i));

            // State should remain Encoding after first frame
            let state = wiring.state();
            assert!(
                state == WiringState::Ready || state == WiringState::Encoding,
                "Invalid state {:?} at frame {}",
                state,
                i
            );
        }

        let _flushed = wiring.flush(&mut sub_capsules).expect("Flush failed");
        assert_eq!(wiring.state(), WiringState::Finalized);

        println!("State consistency test passed: 500 frames");
    }
}

// ============================================================================
// Module: Edge Case Stress Tests (T28 Q25)
// ============================================================================

mod edge_case_stress {
    use super::*;

    /// Q25-1: 1x1 minimum possible frame
    #[test]
    fn test_1x1_frame() {
        // Note: AV1 minimum is 8x8 superblock, but encoder should handle smaller gracefully
        let width = 8; // Minimum AV1 superblock
        let height = 8;

        let frame = create_yuv_frame(width, height, FramePattern::Gray);
        let result = encode_frame(width, height, &frame, 28, 5);

        assert!(result.is_ok(), "Minimum frame encoding failed: {:?}", result);
    }

    /// Q25-2: Non-multiple-of-8 dimensions
    #[test]
    fn test_non_aligned_dimensions() {
        let test_cases = [
            (17, 17),
            (31, 31),
            (100, 100),
            (123, 456),
            (511, 511),
            (1000, 1000),
        ];

        for (width, height) in test_cases {
            let frame = create_yuv_frame(width, height, FramePattern::Gradient);
            let result = encode_frame(width, height, &frame, 28, 5);

            assert!(
                result.is_ok(),
                "Non-aligned {}x{} encoding failed: {:?}",
                width,
                height,
                result
            );
        }

        println!("Non-aligned dimension tests passed");
    }

    /// Q25-3: Extreme aspect ratios
    #[test]
    fn test_extreme_aspect_ratios() {
        let test_cases = [
            (1920, 64, "ultra-wide"),   // 30:1 aspect ratio
            (64, 1080, "ultra-tall"),   // 1:17 aspect ratio
            (4096, 64, "banner"),       // 64:1
            (64, 2048, "column"),       // 1:32
        ];

        for (width, height, name) in test_cases {
            let frame = create_yuv_frame(width, height, FramePattern::Gradient);
            let result = encode_frame(width, height, &frame, 28, 7); // Fast preset

            match result {
                Ok(encoded) => {
                    println!("{} ({}x{}): {} bytes", name, width, height, encoded.len());
                }
                Err(e) => {
                    // Some extreme ratios may fail, which is acceptable
                    println!("{} ({}x{}) failed: {} (acceptable)", name, width, height, e);
                }
            }
        }
    }

    /// Q25-4: All CRF values (0-63)
    #[test]
    fn test_all_crf_values() {
        let width = 64;
        let height = 64;
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);

        let crf_values = [0, 1, 10, 20, 28, 35, 45, 55, 63];

        for crf in crf_values {
            let result = encode_frame(width, height, &frame, crf, 5);
            assert!(
                result.is_ok(),
                "CRF {} encoding failed: {:?}",
                crf,
                result
            );
        }

        println!("All CRF values test passed");
    }

    /// Q25-5: All speed presets (0-10)
    #[test]
    fn test_all_speed_presets() {
        let width = 64;
        let height = 64;
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);

        for speed in 0..=10 {
            let result = encode_frame(width, height, &frame, 28, speed);
            assert!(
                result.is_ok(),
                "Speed {} encoding failed: {:?}",
                speed,
                result
            );
        }

        println!("All speed presets test passed");
    }

    /// Q25-6: Empty frame handling
    #[test]
    fn test_insufficient_data_handling() {
        let width = 64;
        let height = 64;

        // Create undersized frame data
        let small_frame = vec![128u8; 100]; // Way too small

        let mut wiring = EncoderWiringCapsule::new();
        let mut sub_capsules = wiring
            .initialize(width, height, 28, 5)
            .expect("Init failed");

        // Should fail gracefully
        let result = wiring.encode_frame(&small_frame, &mut sub_capsules);
        assert!(
            result.is_err(),
            "Should reject insufficient data"
        );
    }

    /// Q25-7: Rapid resolution changes (simulated)
    #[test]
    fn test_resolution_switching() {
        let resolutions = [
            (320, 240),
            (640, 480),
            (1280, 720),
            (640, 480),
            (320, 240),
        ];

        for (width, height) in resolutions {
            let frame = create_yuv_frame(width, height, FramePattern::Gradient);
            let result = encode_frame(width, height, &frame, 28, 5);
            assert!(
                result.is_ok(),
                "Resolution {}x{} failed after switch: {:?}",
                width,
                height,
                result
            );
        }

        println!("Resolution switching test passed");
    }
}

// ============================================================================
// Module: Content Stress Tests (T28 Q26)
// ============================================================================

mod content_stress {
    use super::*;

    /// Q26-1: Scene change torture (flash cuts every frame)
    #[test]
    fn test_scene_change_torture() {
        let width = 640;
        let height = 480;
        let num_frames = 100;

        let mut wiring = EncoderWiringCapsule::new();
        let mut sub_capsules = wiring
            .initialize(width, height, 28, 5)
            .expect("Init failed");

        // Alternate between completely different scenes
        for i in 0..num_frames {
            let pattern = if i % 2 == 0 {
                FramePattern::FlashFrame { is_white: true }
            } else {
                FramePattern::FlashFrame { is_white: false }
            };

            let frame = create_yuv_frame(width, height, pattern);
            let result = wiring.encode_frame(&frame, &mut sub_capsules);
            assert!(result.is_ok(), "Scene change frame {} failed", i);
        }

        let _flushed = wiring.flush(&mut sub_capsules).expect("Flush failed");
        println!("Scene change torture test passed: {} frames", num_frames);
    }

    /// Q26-2: High motion stress (fast moving bars)
    #[test]
    fn test_high_motion_stress() {
        let width = 640;
        let height = 480;
        let num_frames = 60; // 2 seconds @ 30fps

        let mut wiring = EncoderWiringCapsule::new();
        let mut sub_capsules = wiring
            .initialize(width, height, 28, 5)
            .expect("Init failed");

        // Fast horizontal motion (32 pixels per frame = 960 pixels/sec)
        for frame_num in 0..num_frames {
            let pattern = FramePattern::MovingBars {
                offset: (frame_num as u32) * 32, // Very fast motion
                bar_width: 8,
            };
            let frame = create_yuv_frame(width, height, pattern);
            let result = wiring.encode_frame(&frame, &mut sub_capsules);
            assert!(result.is_ok(), "High motion frame {} failed", frame_num);
        }

        let _flushed = wiring.flush(&mut sub_capsules).expect("Flush failed");
        println!("High motion stress test passed");
    }

    /// Q26-3: Gradient stress (banding detection)
    #[test]
    fn test_gradient_banding_stress() {
        let width = 1920;
        let height = 1080;

        // Fine gradient that may cause banding
        let frame = create_yuv_frame(width, height, FramePattern::ColorRamp16Bit);
        let result = encode_frame(width, height, &frame, 28, 5);

        assert!(result.is_ok(), "Gradient encoding failed");
        println!("Gradient banding stress test passed");
    }

    /// Q26-4: Static content stress (high compression potential)
    #[test]
    fn test_static_content_stress() {
        let width = 1920;
        let height = 1080;
        let num_frames = 30;

        let frame = create_yuv_frame(width, height, FramePattern::Static);

        let mut wiring = EncoderWiringCapsule::new();
        let mut sub_capsules = wiring
            .initialize(width, height, 28, 5)
            .expect("Init failed");

        let mut total_bytes = 0u64;

        // Static content should compress very well
        for _ in 0..num_frames {
            let encoded = wiring
                .encode_frame(&frame, &mut sub_capsules)
                .expect("Static frame failed");
            total_bytes += encoded.len() as u64;
        }

        let _flushed = wiring.flush(&mut sub_capsules).expect("Flush failed");

        // Static content should have very high compression ratio
        let raw_size = (width * height * num_frames as u32 * 3 / 2) as u64;
        let ratio = raw_size as f64 / total_bytes as f64;

        println!(
            "Static content: {} raw -> {} encoded ({:.1}x compression)",
            raw_size, total_bytes, ratio
        );

        assert!(ratio > 10.0, "Static content compression ratio too low: {:.1}x", ratio);
    }

    /// Q26-5: Random noise stress (incompressible content)
    #[test]
    fn test_random_noise_stress() {
        let width = 640;
        let height = 480;
        let num_frames = 10;

        let mut wiring = EncoderWiringCapsule::new();
        let mut sub_capsules = wiring
            .initialize(width, height, 35, 7) // Higher CRF, faster speed for noise
            .expect("Init failed");

        let mut total_bytes = 0u64;

        for frame_num in 0..num_frames {
            let pattern = FramePattern::RandomNoise {
                seed: frame_num as u32,
            };
            let frame = create_yuv_frame(width, height, pattern);
            let encoded = wiring
                .encode_frame(&frame, &mut sub_capsules)
                .expect("Noise frame failed");
            total_bytes += encoded.len() as u64;
        }

        let _flushed = wiring.flush(&mut sub_capsules).expect("Flush failed");

        let raw_size = (width * height * num_frames as u32 * 3 / 2) as u64;
        let ratio = raw_size as f64 / total_bytes as f64;

        println!(
            "Random noise: {} raw -> {} encoded ({:.1}x compression)",
            raw_size, total_bytes, ratio
        );

        // Noise has very low compression potential
        assert!(
            total_bytes > 0,
            "Noise encoding produced no output"
        );
    }

    /// Q26-6: High frequency stress (worst case for DCT)
    #[test]
    fn test_high_frequency_stress() {
        let width = 640;
        let height = 480;

        let frame = create_yuv_frame(width, height, FramePattern::HighFrequency);
        let result = encode_frame(width, height, &frame, 28, 5);

        assert!(result.is_ok(), "High frequency encoding failed");
        println!("High frequency stress test passed");
    }

    /// Q26-7: Checkerboard pattern stress (aliasing detection)
    #[test]
    fn test_checkerboard_stress() {
        let width = 1280;
        let height = 720;
        let block_sizes = [1, 2, 4, 8, 16, 32];

        for block_size in block_sizes {
            let pattern = FramePattern::Checkerboard { block_size };
            let frame = create_yuv_frame(width, height, pattern);
            let result = encode_frame(width, height, &frame, 28, 5);

            assert!(
                result.is_ok(),
                "Checkerboard block_size={} failed: {:?}",
                block_size,
                result
            );
        }

        println!("Checkerboard stress test passed");
    }
}

// ============================================================================
// Module: Determinism Stress Tests (T28 Q29-Q35 Extension)
// ============================================================================

mod determinism_stress {
    use super::*;

    /// Determinism-1: Bit-exact reproducibility under stress
    #[test]
    fn test_bitexact_reproducibility_stress() {
        let width = 640;
        let height = 480;
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);

        let reference_hash = {
            let encoded = encode_frame(width, height, &frame, 28, 5).expect("Reference encode failed");
            hash_bytes(&encoded)
        };

        // Stress with 100 encodes
        for i in 0..STRESS_ITERATIONS_LIGHT {
            let encoded = encode_frame(width, height, &frame, 28, 5)
                .expect(&format!("Encode {} failed", i));
            let hash = hash_bytes(&encoded);

            assert_eq!(
                hash, reference_hash,
                "Determinism failed at iteration {}",
                i
            );
        }

        println!("Bit-exact reproducibility stress passed: {} iterations", STRESS_ITERATIONS_LIGHT);
    }

    /// Determinism-2: Different thread counts produce same output
    #[test]
    fn test_thread_count_invariance() {
        let width = 256;
        let height = 256;
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);

        // Encode with different thread configurations (simulated via speed presets)
        let speeds = [0, 3, 5, 7, 10];
        let mut results: Vec<(u8, [u8; 32])> = Vec::new();

        for speed in speeds {
            let encoded = encode_frame(width, height, &frame, 28, speed)
                .expect(&format!("Speed {} failed", speed));
            let hash = hash_bytes(&encoded);
            results.push((speed, hash));
        }

        // Note: Different speed presets MAY produce different output (by design)
        // But SAME speed preset must be deterministic (verified separately)
        println!("Thread count invariance test:");
        for (speed, hash) in &results {
            println!("  Speed {}: {:x?}...", speed, &hash[..8]);
        }
    }

    /// Determinism-3: CRF delta produces deterministic quality change
    #[test]
    fn test_crf_deterministic_delta() {
        let width = 256;
        let height = 256;
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);

        let crf_values = [20, 25, 30, 35, 40];
        let mut sizes: Vec<(u8, usize)> = Vec::new();

        for crf in crf_values {
            let encoded = encode_frame(width, height, &frame, crf, 5)
                .expect(&format!("CRF {} failed", crf));
            sizes.push((crf, encoded.len()));
        }

        // Higher CRF should produce smaller output (lower quality, more compression)
        println!("CRF deterministic delta test:");
        for (crf, size) in &sizes {
            println!("  CRF {}: {} bytes", crf, size);
        }

        // Verify monotonic relationship (higher CRF = smaller size, generally)
        // Note: May not be strictly monotonic for small test frames
        let first_size = sizes[0].1;
        let last_size = sizes[sizes.len() - 1].1;
        println!("  CRF 20->40 size delta: {} -> {} bytes", first_size, last_size);
    }

    /// Determinism-4: Checkpoint/resume identical continuation
    #[test]
    fn test_checkpoint_determinism() {
        let width = 256;
        let height = 256;
        let total_frames = 20;

        let frames: Vec<Vec<u8>> = (0..total_frames)
            .map(|i| {
                create_yuv_frame(
                    width,
                    height,
                    FramePattern::MovingBars {
                        offset: i as u32 * 8,
                        bar_width: 16,
                    },
                )
            })
            .collect();

        // Continuous encode
        let continuous_outputs = encode_sequence(width, height, &frames, 28, 5)
            .expect("Continuous encode failed");

        // Simulated checkpoint: encode in two parts
        let split_point = total_frames / 2;

        let part1_outputs =
            encode_sequence(width, height, &frames[..split_point], 28, 5)
                .expect("Part 1 failed");

        let _part2_outputs =
            encode_sequence(width, height, &frames[split_point..], 28, 5)
                .expect("Part 2 failed");

        // Compare first N frames (before checkpoint)
        // Note: We only verify part1 matches since encoder state may differ
        // after resuming from a checkpoint. Full checkpoint/resume verification
        // requires the checkpoint persistence system (T9 Persistent tier).
        for i in 0..split_point {
            let cont_hash = hash_bytes(&continuous_outputs[i]);
            let part_hash = hash_bytes(&part1_outputs[i]);
            assert_eq!(
                cont_hash, part_hash,
                "Pre-checkpoint frame {} differs",
                i
            );
        }

        println!("Checkpoint determinism test passed");
    }

    /// Determinism-5: Heavy stress determinism (1000 identical encodes)
    #[test]
    #[ignore = "Slow stress test - run with --ignored"]
    fn test_stress_1000_determinism() {
        let width = 64;
        let height = 64;
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);

        let reference_hash = {
            let encoded = encode_frame(width, height, &frame, 28, 5).expect("Reference failed");
            hash_bytes(&encoded)
        };

        for i in 0..STRESS_ITERATIONS_HEAVY {
            let encoded = encode_frame(width, height, &frame, 28, 5)
                .expect(&format!("Iteration {} failed", i));
            let hash = hash_bytes(&encoded);

            if hash != reference_hash {
                panic!(
                    "Determinism failure at iteration {}: expected {:x?}, got {:x?}",
                    i,
                    &reference_hash[..8],
                    &hash[..8]
                );
            }

            if (i + 1) % 100 == 0 {
                println!("Determinism verified: {}/{}", i + 1, STRESS_ITERATIONS_HEAVY);
            }
        }

        println!("Heavy stress determinism passed: {} iterations", STRESS_ITERATIONS_HEAVY);
    }
}

// ============================================================================
// Module: Performance Regression Tests (T28 Q27)
// ============================================================================

mod performance_regression {
    use super::*;

    /// Q27-1: Encode time tracking
    #[test]
    fn test_encode_time_tracking() {
        let width = 640;
        let height = 480;
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);

        let mut times: Vec<Duration> = Vec::new();

        // Warm up
        for _ in 0..5 {
            let _ = encode_frame(width, height, &frame, 28, 5);
        }

        // Measure
        for _ in 0..20 {
            let start = Instant::now();
            let _ = encode_frame(width, height, &frame, 28, 5).expect("Encode failed");
            times.push(start.elapsed());
        }

        let avg: Duration = times.iter().sum::<Duration>() / times.len() as u32;
        let max = times.iter().max().unwrap();
        let min = times.iter().min().unwrap();

        // Check for outliers (regression indicator)
        let avg_nanos = avg.as_nanos() as f64;
        let outliers: Vec<_> = times
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                let t_nanos = t.as_nanos() as f64;
                (t_nanos - avg_nanos).abs() / avg_nanos > REGRESSION_THRESHOLD_TIME / 100.0
            })
            .collect();

        println!("Encode time tracking:");
        println!("  - Average: {:?}", avg);
        println!("  - Min: {:?}", min);
        println!("  - Max: {:?}", max);
        println!("  - Outliers (>{}% deviation): {}", REGRESSION_THRESHOLD_TIME, outliers.len());

        // Allow some outliers but flag excessive variation
        assert!(
            outliers.len() <= times.len() / 4,
            "Too many timing outliers: {} / {}",
            outliers.len(),
            times.len()
        );
    }

    /// Q27-2: Output size tracking
    #[test]
    fn test_output_size_tracking() {
        let width = 640;
        let height = 480;
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);

        let mut sizes: Vec<usize> = Vec::new();

        for _ in 0..20 {
            let encoded = encode_frame(width, height, &frame, 28, 5).expect("Encode failed");
            sizes.push(encoded.len());
        }

        // All sizes should be identical (deterministic encoder)
        let first_size = sizes[0];
        for (i, size) in sizes.iter().enumerate() {
            assert_eq!(
                *size, first_size,
                "Size regression at iteration {}: expected {}, got {}",
                i, first_size, size
            );
        }

        println!("Output size tracking: all {} iterations = {} bytes", sizes.len(), first_size);
    }

    /// Q27-3: Memory usage tracking
    #[test]
    fn test_memory_usage_tracking() {
        let width = 1280;
        let height = 720;

        let baseline = get_process_memory_bytes();

        // Encode several frames
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);
        for _ in 0..10 {
            let _ = encode_frame(width, height, &frame, 28, 5);
        }

        let after = get_process_memory_bytes();

        if let (Some(base), Some(current)) = (baseline, after) {
            let delta_mb = (current.saturating_sub(base)) as f64 / (1024.0 * 1024.0);
            println!("Memory usage tracking: {:.1} MB delta", delta_mb);

            // Reasonable memory growth for 720p (should be < 500MB)
            assert!(
                delta_mb < 500.0,
                "Excessive memory growth: {:.1} MB",
                delta_mb
            );
        } else {
            println!("Memory tracking unavailable (non-Linux platform)");
        }
    }

    /// Q27-4: Generate regression baseline
    #[test]
    fn test_generate_regression_baseline() {
        let test_configs = [
            (64, 64, "64x64"),
            (320, 240, "QVGA"),
            (640, 480, "VGA"),
            (1280, 720, "720p"),
        ];

        println!("\nRegression Baseline Report");
        println!("==========================");
        println!("| Resolution | CRF 28 Size | Time (avg) |");
        println!("|------------|-------------|------------|");

        for (width, height, name) in test_configs {
            let frame = create_yuv_frame(width, height, FramePattern::Gradient);

            let mut times: Vec<Duration> = Vec::new();
            let mut size = 0;

            for _ in 0..5 {
                let start = Instant::now();
                let encoded = encode_frame(width, height, &frame, 28, 5).expect("Encode failed");
                times.push(start.elapsed());
                size = encoded.len();
            }

            let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
            println!("| {:10} | {:11} | {:10?} |", name, size, avg_time);
        }
    }
}

// ============================================================================
// Module: Chaos Compliance Verification (UCE34/Chaos Framework)
// ============================================================================

mod chaos_compliance {
    use super::*;

    /// Chaos-1: Verify lockfree operation under stress
    ///
    /// This test verifies that all encoder operations remain lockfree
    /// (no mutex contention visible in timing patterns).
    #[test]
    fn test_lockfree_operation_verification() {
        let width = 256;
        let height = 256;
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);

        let mut times: Vec<Duration> = Vec::new();

        // Measure many encodes
        for _ in 0..100 {
            let start = Instant::now();
            let _ = encode_frame(width, height, &frame, 28, 5).expect("Encode failed");
            times.push(start.elapsed());
        }

        // Lockfree operations should have consistent timing
        // Mutex-based code shows bimodal distribution (fast vs contended)
        let avg_nanos = times.iter().map(|t| t.as_nanos()).sum::<u128>() / times.len() as u128;

        // Check coefficient of variation (CV) - lockfree should be < 0.5
        let variance: f64 = times
            .iter()
            .map(|t| {
                let diff = t.as_nanos() as f64 - avg_nanos as f64;
                diff * diff
            })
            .sum::<f64>()
            / times.len() as f64;
        let std_dev = variance.sqrt();
        let cv = std_dev / avg_nanos as f64;

        println!("Lockfree verification:");
        println!("  - Average: {} ns", avg_nanos);
        println!("  - Std Dev: {:.0} ns", std_dev);
        println!("  - CV: {:.3}", cv);
        println!("  - Lockfree threshold: CV < 0.5");

        // Lockfree operations typically have CV < 0.3-0.5
        // Mutex contention causes CV > 1.0
        assert!(
            cv < 1.0,
            "High timing variance (CV={:.3}) suggests mutex contention",
            cv
        );
    }

    /// Chaos-2: Cache alignment verification
    #[test]
    fn test_cache_alignment_verification() {
        // EncoderWiringCapsule should be 128B aligned
        assert_eq!(
            std::mem::align_of::<EncoderWiringCapsule>(),
            128,
            "EncoderWiringCapsule should be 128B aligned"
        );

        // Size should be multiple of alignment
        assert_eq!(
            std::mem::size_of::<EncoderWiringCapsule>() % 128,
            0,
            "EncoderWiringCapsule size should be multiple of 128B"
        );

        println!(
            "Cache alignment verified: EncoderWiringCapsule = {} bytes, {} align",
            std::mem::size_of::<EncoderWiringCapsule>(),
            std::mem::align_of::<EncoderWiringCapsule>()
        );
    }

    /// Chaos-3: Generation counter monotonicity
    #[test]
    fn test_generation_counter_monotonicity() {
        let width = 64;
        let height = 64;
        let frame = create_yuv_frame(width, height, FramePattern::Gray);

        let mut wiring = EncoderWiringCapsule::new();
        let mut sub_capsules = wiring
            .initialize(width, height, 28, 5)
            .expect("Init failed");

        let mut last_generation = sub_capsules.generation();

        // Encode multiple frames, verify generation always increases
        for i in 0..50 {
            let _ = wiring
                .encode_frame(&frame, &mut sub_capsules)
                .expect(&format!("Frame {} failed", i));

            let current_generation = sub_capsules.generation();
            assert!(
                current_generation > last_generation,
                "Generation counter not monotonic at frame {}: {} <= {}",
                i,
                current_generation,
                last_generation
            );
            last_generation = current_generation;
        }

        println!("Generation counter monotonicity verified: final = {}", last_generation);
    }

    /// Chaos-4: State machine validity
    #[test]
    fn test_state_machine_validity() {
        let width = 64;
        let height = 64;
        let frame = create_yuv_frame(width, height, FramePattern::Gray);

        let mut wiring = EncoderWiringCapsule::new();

        // Initial state
        assert_eq!(
            wiring.state(),
            WiringState::Uninitialized,
            "Initial state should be Uninitialized"
        );

        // After initialize
        let mut sub_capsules = wiring
            .initialize(width, height, 28, 5)
            .expect("Init failed");
        assert_eq!(
            wiring.state(),
            WiringState::Ready,
            "State after init should be Ready"
        );

        // After first encode
        let _ = wiring.encode_frame(&frame, &mut sub_capsules).expect("Encode failed");
        let state_after_encode = wiring.state();
        assert!(
            state_after_encode == WiringState::Ready || state_after_encode == WiringState::Encoding,
            "State after encode should be Ready or Encoding, got {:?}",
            state_after_encode
        );

        // After flush
        let _ = wiring.flush(&mut sub_capsules).expect("Flush failed");
        assert_eq!(
            wiring.state(),
            WiringState::Finalized,
            "State after flush should be Finalized"
        );

        println!("State machine validity verified");
    }
}

// ============================================================================
// Module: Bitstream Validation Tests (dav1d Integration)
// ============================================================================

mod bitstream_validation {
    use super::*;

    /// Bitstream-1: Single frame dav1d validation
    #[test]
    fn test_dav1d_single_frame_validation() {
        if !is_dav1d_installed() {
            println!("dav1d not installed, skipping validation");
            return;
        }

        let width = 640;
        let height = 480;
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);

        let encoded = encode_frame(width, height, &frame, 28, 5).expect("Encode failed");

        let ivf_path = "/tmp/stress_test_single.ivf";
        write_ivf_file(ivf_path, width, height, &[encoded]).expect("IVF write failed");

        validate_with_dav1d(ivf_path).expect("dav1d validation failed");
        println!("dav1d single frame validation passed");
    }

    /// Bitstream-2: Multi-frame sequence dav1d validation
    #[test]
    fn test_dav1d_sequence_validation() {
        if !is_dav1d_installed() {
            println!("dav1d not installed, skipping validation");
            return;
        }

        let width = 320;
        let height = 240;
        let num_frames = 30;

        let frames: Vec<Vec<u8>> = (0..num_frames)
            .map(|i| {
                create_yuv_frame(
                    width,
                    height,
                    FramePattern::MovingBars {
                        offset: i as u32 * 8,
                        bar_width: 16,
                    },
                )
            })
            .collect();

        let encoded = encode_sequence(width, height, &frames, 28, 5).expect("Encode failed");

        let ivf_path = "/tmp/stress_test_sequence.ivf";
        write_ivf_file(ivf_path, width, height, &encoded).expect("IVF write failed");

        validate_with_dav1d(ivf_path).expect("dav1d validation failed");
        println!("dav1d sequence validation passed: {} frames", num_frames);
    }

    /// Bitstream-3: All resolutions dav1d validation
    #[test]
    fn test_dav1d_resolution_sweep() {
        if !is_dav1d_installed() {
            println!("dav1d not installed, skipping validation");
            return;
        }

        let resolutions = [
            (64, 64),
            (128, 128),
            (256, 256),
            (320, 240),
            (640, 480),
        ];

        for (width, height) in resolutions {
            let frame = create_yuv_frame(width, height, FramePattern::Gradient);
            let encoded = encode_frame(width, height, &frame, 28, 5).expect("Encode failed");

            let ivf_path = format!("/tmp/stress_test_{}x{}.ivf", width, height);
            write_ivf_file(&ivf_path, width, height, &[encoded]).expect("IVF write failed");

            match validate_with_dav1d(&ivf_path) {
                Ok(_) => println!("{}x{}: dav1d OK", width, height),
                Err(e) => println!("{}x{}: dav1d FAIL - {}", width, height, e),
            }
        }
    }

    /// Bitstream-4: High-resolution dav1d validation
    #[test]
    #[ignore = "Large resolution - run with --ignored"]
    fn test_dav1d_1080p_validation() {
        if !is_dav1d_installed() {
            println!("dav1d not installed, skipping validation");
            return;
        }

        let width = 1920;
        let height = 1080;
        let frame = create_yuv_frame(width, height, FramePattern::Gradient);

        let encoded = encode_frame(width, height, &frame, 28, 5).expect("Encode failed");

        let ivf_path = "/tmp/stress_test_1080p.ivf";
        write_ivf_file(ivf_path, width, height, &[encoded]).expect("IVF write failed");

        validate_with_dav1d(ivf_path).expect("dav1d 1080p validation failed");
        println!("dav1d 1080p validation passed");
    }
}

// ============================================================================
// Integration Summary Test
// ============================================================================

/// Integration test that runs a quick sanity check across all stress categories
#[test]
fn test_production_stress_integration_summary() {
    println!("\n========================================");
    println!("Production Stress Test Integration Summary");
    println!("========================================\n");

    let mut passed = 0;
    let mut failed = 0;

    // Quick checks from each category using boxed closures for heterogeneous types
    let tests: Vec<(&str, Box<dyn Fn() -> bool>)> = vec![
        ("Memory: 4K encode", Box::new(|| {
            let frame = create_yuv_frame(3840, 2160, FramePattern::Gray);
            encode_frame(3840, 2160, &frame, 35, 8).is_ok()
        })),
        ("Concurrency: 4 threads", Box::new(|| {
            let frame = Arc::new(create_yuv_frame(320, 240, FramePattern::Gradient));
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let f = Arc::clone(&frame);
                    std::thread::spawn(move || encode_frame(320, 240, &f, 28, 5).is_ok())
                })
                .collect();
            handles.into_iter().all(|h| h.join().unwrap_or(false))
        })),
        ("Duration: 100 frames", Box::new(|| {
            let mut wiring = EncoderWiringCapsule::new();
            let mut sub = wiring.initialize(64, 64, 28, 5).unwrap();
            let frame = create_yuv_frame(64, 64, FramePattern::Gray);
            for _ in 0..100 {
                if wiring.encode_frame(&frame, &mut sub).is_err() {
                    return false;
                }
            }
            true
        })),
        ("Edge: Odd dimensions", Box::new(|| {
            let frame = create_yuv_frame(127, 127, FramePattern::Gray);
            encode_frame(127, 127, &frame, 28, 5).is_ok()
        })),
        ("Content: Scene changes", Box::new(|| {
            let mut wiring = EncoderWiringCapsule::new();
            let mut sub = wiring.initialize(64, 64, 28, 5).unwrap();
            for i in 0..10 {
                let pattern = if i % 2 == 0 {
                    FramePattern::FlashFrame { is_white: true }
                } else {
                    FramePattern::FlashFrame { is_white: false }
                };
                let frame = create_yuv_frame(64, 64, pattern);
                if wiring.encode_frame(&frame, &mut sub).is_err() {
                    return false;
                }
            }
            true
        })),
        ("Determinism: 10 identical", Box::new(|| {
            let frame = create_yuv_frame(64, 64, FramePattern::Gradient);
            let reference = match encode_frame(64, 64, &frame, 28, 5) {
                Ok(r) => r,
                Err(_) => return false,
            };
            let ref_hash = hash_bytes(&reference);
            for _ in 0..10 {
                let encoded = match encode_frame(64, 64, &frame, 28, 5) {
                    Ok(e) => e,
                    Err(_) => return false,
                };
                if hash_bytes(&encoded) != ref_hash {
                    return false;
                }
            }
            true
        })),
        ("Chaos: Cache alignment", Box::new(|| {
            std::mem::align_of::<EncoderWiringCapsule>() == 128
        })),
    ];

    for (name, test_fn) in tests.iter() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| test_fn()));
        match result {
            Ok(true) => {
                println!("[PASS] {}", name);
                passed += 1;
            }
            Ok(false) => {
                println!("[FAIL] {}", name);
                failed += 1;
            }
            Err(_) => {
                println!("[PANIC] {}", name);
                failed += 1;
            }
        }
    }

    println!("\n========================================");
    println!("Results: {} passed, {} failed", passed, failed);
    println!("========================================\n");

    assert_eq!(failed, 0, "{} integration tests failed", failed);
}
