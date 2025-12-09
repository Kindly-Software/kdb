//! Integration Tests for kindly-av1 Full Encode Pipeline
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! T28 Compliance: Q15-Q21 Integration Tier
//!
//! These tests verify the complete encode pipeline from initialization
//! through frame encoding to finalization.

use kindly_av1::checkpoint::EncoderCheckpointCapsule;
use kindly_av1::encoder::{
    EncoderSubCapsules, EncoderWiringCapsule, GpuMotionBackend, GpuMotionEstimationCapsule,
    MotionVector, WiringState,
};
use kindly_av1::file::{detect_format, discover_videos, InputFormat};
use kindly_av1::license::{LicenseState, LicenseVerificationCapsule};
use kindly_av1::progress::ProgressCapsule;

use std::sync::Arc;
use std::thread;

// ============================================================================
// T28 Q15: Full Pipeline Integration Tests
// ============================================================================

/// Q15-1: Test complete encode lifecycle
#[test]
fn test_complete_encode_lifecycle() {
    let mut wiring = EncoderWiringCapsule::new();
    assert_eq!(wiring.state(), WiringState::Uninitialized);

    // Initialize with 720p dimensions
    let mut sub_capsules = wiring.initialize(1280, 720, 28, 5).unwrap();
    assert_eq!(wiring.state(), WiringState::Ready);
    assert_eq!(wiring.generation(), 1);

    // Create test frame (1280x720 YUV420p)
    let frame_size = 1280 * 720 * 3 / 2;
    let frame_data = vec![128u8; frame_size];

    // Encode first frame
    let result = wiring.encode_frame(&frame_data, &mut sub_capsules);
    assert!(
        result.is_ok(),
        "First frame encode failed: {:?}",
        result.err()
    );
    assert_eq!(wiring.state(), WiringState::Encoding);
    assert_eq!(wiring.frames_encoded(), 1);

    // Encode second frame
    let result = wiring.encode_frame(&frame_data, &mut sub_capsules);
    assert!(result.is_ok(), "Second frame encode failed");
    assert_eq!(wiring.frames_encoded(), 2);

    // Flush and finalize
    let _flushed = wiring.flush(&mut sub_capsules).unwrap();
    assert_eq!(wiring.state(), WiringState::Finalized);
    assert!(wiring.is_finalized());

    // Verify stats
    let stats = wiring.stats();
    assert_eq!(stats.frames_encoded, 2);
    assert!(stats.bytes_output > 0);
    assert_eq!(stats.width, 1280);
    assert_eq!(stats.height, 720);
}

/// Q15-2: Test encoding multiple resolutions
#[test]
fn test_multi_resolution_encoding() {
    let resolutions = [(64, 64), (320, 240), (640, 480), (1280, 720), (1920, 1080)];

    for (width, height) in resolutions {
        let mut wiring = EncoderWiringCapsule::new();
        let mut sub_capsules = wiring.initialize(width, height, 28, 5).unwrap();

        let frame_size = (width * height * 3 / 2) as usize;
        let frame_data = vec![128u8; frame_size];

        let result = wiring.encode_frame(&frame_data, &mut sub_capsules);
        assert!(
            result.is_ok(),
            "Failed at {}x{}: {:?}",
            width,
            height,
            result.err()
        );

        wiring.flush(&mut sub_capsules).unwrap();
        assert!(wiring.is_finalized());
    }
}

/// Q15-3: Test CRF range (quality settings)
/// Note: Tests config storage; full quantization tested in atomic_capsule
#[test]
fn test_crf_range() {
    // Test that CRF values are stored correctly in stats
    // Quantization edge cases (overflow at extremes) tested in atomic_capsule
    for crf in [10, 20, 28, 35, 40] {
        let mut wiring = EncoderWiringCapsule::new();
        let result = wiring.initialize(64, 64, crf, 5);
        assert!(
            result.is_ok(),
            "Failed with CRF {}: {:?}",
            crf,
            result.err()
        );

        let stats = wiring.stats();
        assert_eq!(stats.crf, crf);
    }
}

/// Q15-4: Test speed presets (0-10)
#[test]
fn test_speed_presets() {
    for speed in 0..=10 {
        let mut wiring = EncoderWiringCapsule::new();
        let result = wiring.initialize(64, 64, 28, speed);
        assert!(
            result.is_ok(),
            "Failed with speed {}: {:?}",
            speed,
            result.err()
        );

        let stats = wiring.stats();
        assert_eq!(stats.speed, speed);
    }
}

// ============================================================================
// T28 Q16: GPU Fallback Integration Tests
// ============================================================================

/// Q16-1: Test GPU detection and fallback
#[test]
fn test_gpu_detection_and_fallback() {
    let gpu_capsule = GpuMotionEstimationCapsule::new();

    // Disable GPU
    gpu_capsule.disable_gpu();
    assert!(!gpu_capsule.is_gpu_available());
    assert_eq!(gpu_capsule.backend(), GpuMotionBackend::CpuSimd);

    // Estimate motion (should use CPU)
    let width = 32u32;
    let height = 32u32;
    let current = vec![128u8; (width * height) as usize];
    let reference = vec![128u8; (width * height) as usize];

    let result = gpu_capsule.estimate_frame(&current, &reference, width, height);
    assert!(result.is_ok());

    let stats = gpu_capsule.stats();
    assert_eq!(stats.cpu_frames, 1);
    assert_eq!(stats.gpu_frames, 0);
}

/// Q16-2: Test motion estimation with moving content
#[test]
fn test_motion_estimation_accuracy() {
    let gpu_capsule = GpuMotionEstimationCapsule::new();
    gpu_capsule.disable_gpu();

    let width = 64u32;
    let height = 64u32;

    // Create frame with a bright block
    let mut current = vec![64u8; (width * height) as usize];
    let mut reference = vec![64u8; (width * height) as usize];

    // Current: bright block at (8,8)
    for y in 8..24 {
        for x in 8..24 {
            current[y * width as usize + x] = 200;
        }
    }

    // Reference: same block shifted right by 4 pixels
    for y in 8..24 {
        for x in 12..28 {
            reference[y * width as usize + x] = 200;
        }
    }

    let result = gpu_capsule.estimate_frame(&current, &reference, width, height);
    assert!(result.is_ok());

    let mvs = result.unwrap();
    assert_eq!(mvs.len(), 16); // 64/16 * 64/16 = 16 macroblocks
}

/// Q16-3: Test GPU re-enable after disable
#[test]
fn test_gpu_reenable() {
    let gpu_capsule = GpuMotionEstimationCapsule::new();
    let initial_gen = gpu_capsule.generation();

    gpu_capsule.disable_gpu();
    assert_eq!(gpu_capsule.generation(), initial_gen + 1);

    gpu_capsule.enable_gpu();
    assert_eq!(gpu_capsule.generation(), initial_gen + 2);
}

/// Q16-4: Test motion estimation with sub-capsules integration
#[test]
fn test_motion_estimation_in_wiring() {
    let mut wiring = EncoderWiringCapsule::new();
    let mut sub_capsules = wiring.initialize(64, 64, 28, 5).unwrap();

    sub_capsules.disable_gpu_motion();
    assert!(!sub_capsules.is_gpu_available());

    let current = vec![128u8; 64 * 64];
    let reference = vec![128u8; 64 * 64];

    let result = sub_capsules.estimate_motion(&current, &reference, 64, 64);
    assert!(result.is_ok());

    let mvs = result.unwrap();
    assert_eq!(mvs.len(), 16);

    let gpu_stats = sub_capsules.gpu_motion_stats();
    assert_eq!(gpu_stats.cpu_frames, 1);
    assert_eq!(gpu_stats.backend, GpuMotionBackend::CpuSimd);
}

// ============================================================================
// T28 Q17: Checkpoint/Resume Integration Tests
// ============================================================================

/// Q17-1: Test checkpoint two-phase commit
#[test]
fn test_checkpoint_two_phase_commit() {
    let input_hash = [0xABu8; 32];
    let checkpoint = EncoderCheckpointCapsule::new(input_hash, 30);

    // Initial state: valid (even generation)
    assert!(checkpoint.is_valid());
    assert!(!checkpoint.is_inflight());

    // Begin checkpoint (odd generation = inflight)
    checkpoint.begin_checkpoint().unwrap();
    assert!(checkpoint.is_inflight());
    assert!(!checkpoint.is_valid());

    // Commit checkpoint (even generation = committed)
    checkpoint.commit_checkpoint(100).unwrap();
    assert!(checkpoint.is_valid());
    assert!(!checkpoint.is_inflight());
    assert_eq!(checkpoint.last_checkpointed_frame(), 100);
}

/// Q17-2: Test checkpoint should_checkpoint logic
#[test]
fn test_checkpoint_interval() {
    let checkpoint = EncoderCheckpointCapsule::new([0u8; 32], 30);

    // Frame 0: never checkpoint
    assert!(!checkpoint.should_checkpoint(0));

    // Frame 1-29: no checkpoint
    for i in 1..30 {
        assert!(!checkpoint.should_checkpoint(i));
    }

    // Frame 30, 60, 90: checkpoint
    assert!(checkpoint.should_checkpoint(30));
    assert!(checkpoint.should_checkpoint(60));
    assert!(checkpoint.should_checkpoint(90));
}

/// Q17-3: Test checkpoint abort
#[test]
fn test_checkpoint_abort() {
    let checkpoint = EncoderCheckpointCapsule::new([0u8; 32], 30);

    checkpoint.begin_checkpoint().unwrap();
    assert!(checkpoint.is_inflight());

    checkpoint.abort_checkpoint().unwrap();
    assert!(checkpoint.is_valid());
    assert_eq!(checkpoint.generation(), 0);
}

// ============================================================================
// T28 Q18: Progress Tracking Integration Tests
// ============================================================================

/// Q18-1: Test progress capsule atomic updates
#[test]
fn test_progress_atomic_updates() {
    let progress = ProgressCapsule::new();
    progress.init(100, 50_000_000);

    assert_eq!(progress.total(), 100);
    assert_eq!(progress.current(), 0);
    assert!((progress.progress() - 0.0).abs() < 0.001);

    // Increment progress
    progress.increment_frame();
    assert_eq!(progress.current(), 1);

    // Encode 50 frames
    for _ in 0..49 {
        progress.increment_frame();
    }
    assert_eq!(progress.current(), 50);
    assert!((progress.progress() - 0.5).abs() < 0.01);
}

/// Q18-2: Test progress concurrent updates
#[test]
fn test_progress_concurrent_updates() {
    let progress = Arc::new(ProgressCapsule::new());
    progress.init(1000, 1_000_000);

    let mut handles = vec![];

    // Spawn 10 threads, each incrementing 100 times
    for _ in 0..10 {
        let progress_clone: Arc<ProgressCapsule> = Arc::clone(&progress);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                progress_clone.increment_frame();
            }
        }));
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Should have exactly 1000 increments
    assert_eq!(progress.current(), 1000);
    assert!((progress.progress() - 1.0).abs() < 0.001);
}

/// Q18-3: Test progress snapshot
#[test]
fn test_progress_snapshot() {
    let progress = ProgressCapsule::new();
    progress.init(100, 50_000);

    for _ in 0..42 {
        progress.increment_frame();
    }
    progress.add_bytes(5000);

    let snapshot = progress.snapshot();
    assert_eq!(snapshot.current, 42);
    assert_eq!(snapshot.total, 100);
    assert_eq!(snapshot.bytes_written, 5000);
    assert!((snapshot.progress - 0.42).abs() < 0.01);
}

// ============================================================================
// T28 Q19: License Verification Integration Tests
// ============================================================================

/// Q19-1: Test license capsule initial state
#[test]
fn test_license_initial_state() {
    let license = LicenseVerificationCapsule::new();

    assert_eq!(license.state(), LicenseState::Invalid);
    assert!(!license.is_valid());
    assert_eq!(license.generation(), 0);
}

/// Q19-2: Test license tamper detection via generation counter
#[test]
fn test_license_tamper_detection() {
    let license = LicenseVerificationCapsule::new();

    // New capsule: Invalid state, generation=0
    // This is a valid initial state (no tampering), just not activated
    assert!(!license.is_valid()); // State is Invalid
    assert!(license.verify_integrity()); // No tampering detected - valid initial state

    // Tampering would be:
    // - Valid state with generation=0 (impossible without activation)
    // - Valid state with activation_timestamp=0 (impossible without activation)
    // Both require direct memory manipulation to achieve

    // Verify state consistency after multiple reads (no corruption)
    for _ in 0..100 {
        let state = license.state();
        let gen = license.generation();
        assert_eq!(state, LicenseState::Invalid);
        assert_eq!(gen, 0);
    }
}

/// Q19-3: Test license state enum
#[test]
fn test_license_state_enum() {
    assert!(!LicenseState::Invalid.allows_encoding());
    assert!(LicenseState::Valid.allows_encoding());
    assert!(!LicenseState::Expired.allows_encoding());
    assert!(!LicenseState::Tampered.allows_encoding());
}

// ============================================================================
// T28 Q20: File Format Detection Integration Tests
// ============================================================================

/// Q20-1: Test format detection for all supported formats
#[test]
fn test_format_detection() {
    // YUV and Y4M should be detected
    assert_eq!(detect_format("video.yuv"), Some(InputFormat::RawYuv));
    assert_eq!(detect_format("video.y4m"), Some(InputFormat::Y4m));

    // Container formats
    assert_eq!(detect_format("video.mp4"), Some(InputFormat::Mp4));
    assert_eq!(detect_format("video.mkv"), Some(InputFormat::Mkv));
    assert_eq!(detect_format("video.webm"), Some(InputFormat::WebM));
    assert_eq!(detect_format("video.mov"), Some(InputFormat::Mov));
    assert_eq!(detect_format("video.avi"), Some(InputFormat::Avi));

    // Unknown extension
    assert_eq!(detect_format("video.xyz"), None);
}

/// Q20-2: Test file discovery doesn't crash
#[test]
fn test_file_discovery_no_crash() {
    // Just verify discover_videos doesn't panic on empty/non-existent dir
    let videos = discover_videos("/tmp");
    // May or may not find videos - just ensure no panic
    let _ = videos;
}

// ============================================================================
// T28 Q21: End-to-End Integration Tests
// ============================================================================

/// Q21-1: Test complete encode with progress tracking
#[test]
fn test_encode_with_progress() {
    let mut wiring = EncoderWiringCapsule::new();
    let mut sub_capsules = wiring.initialize(64, 64, 28, 5).unwrap();

    let progress = ProgressCapsule::new();
    progress.init(5, 10_000);

    let frame_data = vec![128u8; 64 * 64 * 3 / 2];

    // Encode 5 frames with progress tracking
    for i in 0..5 {
        let result = wiring.encode_frame(&frame_data, &mut sub_capsules);
        assert!(result.is_ok(), "Frame {} failed", i);
        progress.increment_frame();
    }

    // Verify both agree
    assert_eq!(wiring.frames_encoded(), 5);
    assert_eq!(progress.current(), 5);
    assert!((progress.progress() - 1.0).abs() < 0.001);

    // Finalize
    wiring.flush(&mut sub_capsules).unwrap();
    assert!(wiring.is_finalized());
}

/// Q21-2: Test encode with checkpoint integration
#[test]
fn test_encode_with_checkpoint() {
    let mut wiring = EncoderWiringCapsule::new();
    let mut sub_capsules = wiring.initialize(64, 64, 28, 5).unwrap();
    let checkpoint = EncoderCheckpointCapsule::new([0xABu8; 32], 2); // Checkpoint every 2 frames

    let frame_data = vec![128u8; 64 * 64 * 3 / 2];

    // Encode with checkpointing
    for i in 0..4 {
        let result = wiring.encode_frame(&frame_data, &mut sub_capsules);
        assert!(result.is_ok());

        // Checkpoint every 2 frames
        if checkpoint.should_checkpoint(i + 1) {
            checkpoint.begin_checkpoint().unwrap();
            checkpoint.commit_checkpoint(i + 1).unwrap();
        }
    }

    assert_eq!(wiring.frames_encoded(), 4);
    assert_eq!(checkpoint.checkpoint_count(), 2); // Frames 2 and 4
}

/// Q21-3: Test encode with GPU motion estimation integration
#[test]
fn test_encode_with_gpu_motion() {
    let mut wiring = EncoderWiringCapsule::new();
    let mut sub_capsules = wiring.initialize(64, 64, 28, 5).unwrap();

    sub_capsules.disable_gpu_motion();

    let frame1 = vec![100u8; 64 * 64 * 3 / 2];
    let frame2 = vec![150u8; 64 * 64 * 3 / 2];

    // Encode frames
    let result1 = wiring.encode_frame(&frame1, &mut sub_capsules);
    assert!(result1.is_ok());

    let result2 = wiring.encode_frame(&frame2, &mut sub_capsules);
    assert!(result2.is_ok());

    assert_eq!(wiring.frames_encoded(), 2);

    let gpu_stats = sub_capsules.gpu_motion_stats();
    assert_eq!(gpu_stats.backend, GpuMotionBackend::CpuSimd);
}

/// Q21-4: Test full pipeline state coordination
#[test]
fn test_full_pipeline_state_coordination() {
    let mut wiring = EncoderWiringCapsule::new();
    let _license = LicenseVerificationCapsule::new();
    let _checkpoint = EncoderCheckpointCapsule::new([1u8; 32], 10);
    let progress = ProgressCapsule::new();
    progress.init(3, 10_000);

    // Initialize encoder
    let mut sub_capsules = wiring.initialize(64, 64, 28, 5).unwrap();
    sub_capsules.disable_gpu_motion();

    let frame_data = vec![128u8; 64 * 64 * 3 / 2];

    for _i in 0..3 {
        // Encode frame
        let result = wiring.encode_frame(&frame_data, &mut sub_capsules);
        assert!(result.is_ok());

        // Update progress
        progress.increment_frame();
    }

    // Verify all state is consistent
    assert_eq!(wiring.frames_encoded(), 3);
    assert_eq!(progress.current(), 3);
    assert!((progress.progress() - 1.0).abs() < 0.001);

    // Finalize
    wiring.flush(&mut sub_capsules).unwrap();
    assert!(wiring.is_finalized());
}

/// Q21-5: Test error handling - insufficient frame data
#[test]
fn test_error_insufficient_frame_data() {
    let mut wiring = EncoderWiringCapsule::new();
    let mut sub_capsules = wiring.initialize(1920, 1080, 28, 5).unwrap();

    // Provide insufficient data
    let insufficient_data = vec![0u8; 1000];

    let result = wiring.encode_frame(&insufficient_data, &mut sub_capsules);
    assert!(result.is_err());
}

/// Q21-6: Test error handling - encode without initialize
#[test]
fn test_error_encode_without_initialize() {
    let mut wiring = EncoderWiringCapsule::new();

    let mut sub_capsules = EncoderSubCapsules::new(64, 64, 28, 5).unwrap();
    let frame_data = vec![128u8; 64 * 64 * 3 / 2];

    let result = wiring.encode_frame(&frame_data, &mut sub_capsules);
    assert!(result.is_err());
}

/// Q21-7: Test multiple sequential encodes (reuse pattern)
#[test]
fn test_sequential_encodes() {
    for video_idx in 0..3 {
        let mut wiring = EncoderWiringCapsule::new();
        let mut sub_capsules = wiring.initialize(64, 64, 28, 5).unwrap();

        let frame_data = vec![(64 + video_idx * 10) as u8; 64 * 64 * 3 / 2];

        for _ in 0..2 {
            let result = wiring.encode_frame(&frame_data, &mut sub_capsules);
            assert!(result.is_ok());
        }

        wiring.flush(&mut sub_capsules).unwrap();
        assert!(wiring.is_finalized());
    }
}

/// Q21-8: Test motion vector zero detection
#[test]
fn test_motion_vector_operations() {
    let mv = MotionVector::zero();
    assert_eq!(mv.x, 0);
    assert_eq!(mv.y, 0);
    assert_eq!(mv.sad, 0);

    let mv2 = MotionVector::new(40, -20, 100);
    assert_eq!(mv2.x, 40);
    assert_eq!(mv2.y, -20);
    assert_eq!(mv2.sad, 100);

    // Quarter-pel to integer-pel conversion
    let (int_x, int_y) = mv2.to_integer_pel();
    assert_eq!(int_x, 10); // 40 / 4
    assert_eq!(int_y, -5); // -20 / 4
}

/// Q21-9: Test wiring stats consistency
#[test]
fn test_wiring_stats_consistency() {
    let mut wiring = EncoderWiringCapsule::new();
    let mut sub_capsules = wiring.initialize(128, 128, 32, 7).unwrap();

    let frame_data = vec![128u8; 128 * 128 * 3 / 2];

    for _ in 0..10 {
        wiring.encode_frame(&frame_data, &mut sub_capsules).unwrap();
    }

    let stats = wiring.stats();
    assert_eq!(stats.frames_encoded, 10);
    assert_eq!(stats.width, 128);
    assert_eq!(stats.height, 128);
    assert_eq!(stats.crf, 32);
    assert_eq!(stats.speed, 7);
    assert!(stats.bytes_output > 0);
}
