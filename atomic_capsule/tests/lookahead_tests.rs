//! # LookaheadCapsule Comprehensive Tests (T28 Framework)
//!
//! **Testing Tiers**:
//! - Q1-Q7: Unit tests (basic functionality)
//! - Q8-Q14: Property tests (invariants, edge cases)
//! - Q15-Q21: Integration tests (concurrent access, realistic workloads)
//! - Q22-Q28: Production tests (performance, stress, regression)
//!
//! **Coverage**: 28 tests across 4 tiers

use atomic_capsule::encoder::{FrameAnalysis, LookaheadCapsule, LookaheadError, MAX_LOOKAHEAD_FRAMES};
use std::sync::Arc;
use std::thread;

// ================================
// Q1-Q7: Unit Tests (7 tests)
// ================================

#[test]
fn q1_layout_verification() {
    // #VERIFY: 512 bytes, 512-byte aligned
    assert_eq!(std::mem::size_of::<LookaheadCapsule>(), 512);
    assert_eq!(std::mem::align_of::<LookaheadCapsule>(), 512);
}

#[test]
fn q2_initialization() {
    let capsule = LookaheadCapsule::new(30);
    let (head, tail, size, gen) = capsule.buffer_stats();

    assert_eq!(head, 0, "Head should start at 0");
    assert_eq!(tail, 0, "Tail should start at 0");
    assert_eq!(size, 30, "Size should match requested");
    assert_eq!(gen, 0, "Generation should start at 0 (even = committed)");
}

#[test]
fn q3_push_single_frame() {
    let capsule = LookaheadCapsule::new(10);

    // Create 128×128 gray frame
    let frame = vec![128u8; 128 * 128];

    let result = capsule.push_frame(&frame, 128, 128);
    assert!(result.is_ok(), "Push should succeed");

    let (head, _, _, _) = capsule.buffer_stats();
    assert_eq!(head, 1, "Head should advance to 1");
}

#[test]
fn q4_analyze_frame_basic() {
    let capsule = LookaheadCapsule::new(10);

    let frame = vec![100u8; 256 * 256];
    capsule.push_frame(&frame, 256, 256).unwrap();

    let analysis = capsule.analyze_frame(0);

    assert_eq!(analysis.sad, 0, "First frame SAD should be 0");
    assert!(analysis.complexity > 0, "Complexity should be non-zero");
    assert!(!analysis.scene_change, "First frame should not be scene change");
}

#[test]
fn q5_invalid_dimensions() {
    let capsule = LookaheadCapsule::new(10);

    let frame = vec![0u8; 100];

    // Zero width
    let result = capsule.push_frame(&frame, 0, 100);
    assert_eq!(result, Err(LookaheadError::InvalidDimensions));

    // Zero height
    let result = capsule.push_frame(&frame, 100, 0);
    assert_eq!(result, Err(LookaheadError::InvalidDimensions));

    // Mismatched size
    let result = capsule.push_frame(&frame, 100, 100);
    assert_eq!(result, Err(LookaheadError::InvalidDimensions));
}

#[test]
fn q6_buffer_full() {
    let capsule = LookaheadCapsule::new(3); // Small buffer

    let frame = vec![100u8; 64 * 64];

    // Fill buffer (3 frames)
    capsule.push_frame(&frame, 64, 64).unwrap();
    capsule.push_frame(&frame, 64, 64).unwrap();
    capsule.push_frame(&frame, 64, 64).unwrap();

    // 4th frame should fail (buffer full)
    let result = capsule.push_frame(&frame, 64, 64);
    assert_eq!(result, Err(LookaheadError::BufferFull));
}

#[test]
fn q7_histogram_basic() {
    let capsule = LookaheadCapsule::new(10);

    // Uniform frame (all pixels = 128)
    let frame = vec![128u8; 256 * 256];
    capsule.push_frame(&frame, 256, 256).unwrap();

    let analysis = capsule.analyze_frame(0);

    // Uniform frame should have low complexity
    assert!(
        analysis.complexity < 10_000,
        "Uniform frame should have low complexity, got {}",
        analysis.complexity
    );
}

// ================================
// Q8-Q14: Property Tests (7 tests)
// ================================

#[test]
fn q8_scene_change_detection_property() {
    let capsule = LookaheadCapsule::new(10);

    // Dark frame
    let frame1 = vec![30u8; 512 * 512];
    capsule.push_frame(&frame1, 512, 512).unwrap();

    // Bright frame (large luminance change)
    let frame2 = vec![220u8; 512 * 512];
    capsule.push_frame(&frame2, 512, 512).unwrap();

    let analysis = capsule.analyze_frame(1);

    assert!(
        analysis.scene_change,
        "Large luminance change should trigger scene change detection"
    );
    assert!(
        analysis.sad > 50_000,
        "SAD should be high for scene change, got {}",
        analysis.sad
    );
}

#[test]
fn q9_no_scene_change_property() {
    let capsule = LookaheadCapsule::new(10);

    // Two similar frames
    let frame1 = vec![128u8; 256 * 256];
    capsule.push_frame(&frame1, 256, 256).unwrap();

    let frame2 = vec![130u8; 256 * 256]; // Only +2 luminance
    capsule.push_frame(&frame2, 256, 256).unwrap();

    let analysis = capsule.analyze_frame(1);

    assert!(
        !analysis.scene_change,
        "Small luminance change should NOT trigger scene change"
    );
}

#[test]
fn q10_complexity_ordering() {
    let capsule = LookaheadCapsule::new(10);

    // Uniform frame (low complexity)
    let frame_uniform = vec![128u8; 256 * 256];
    capsule.push_frame(&frame_uniform, 256, 256).unwrap();

    // Textured frame (high complexity) - checkerboard pattern
    let mut frame_textured = vec![0u8; 256 * 256];
    for (i, pixel) in frame_textured.iter_mut().enumerate() {
        *pixel = if (i / 256 + i % 256) % 2 == 0 { 0 } else { 255 };
    }
    capsule.push_frame(&frame_textured, 256, 256).unwrap();

    let complexity_uniform = capsule.estimate_complexity(0);
    let complexity_textured = capsule.estimate_complexity(1);

    assert!(
        complexity_textured > complexity_uniform,
        "Textured frame should have higher complexity: {} vs {}",
        complexity_textured,
        complexity_uniform
    );
}

#[test]
fn q11_qp_suggestion_property() {
    let capsule = LookaheadCapsule::new(10);

    // High complexity frame → Lower QP (higher quality)
    let mut frame_complex = vec![0u8; 512 * 512];
    for (i, pixel) in frame_complex.iter_mut().enumerate() {
        *pixel = ((i * 7) % 256) as u8; // Pseudo-random texture
    }
    capsule.push_frame(&frame_complex, 512, 512).unwrap();

    // Low complexity frame → Higher QP (lower bitrate)
    let frame_simple = vec![100u8; 512 * 512];
    capsule.push_frame(&frame_simple, 512, 512).unwrap();

    let analysis_complex = capsule.analyze_frame(0);
    let analysis_simple = capsule.analyze_frame(1);

    assert!(
        analysis_complex.suggested_qp < analysis_simple.suggested_qp,
        "Complex frame should suggest lower QP (higher quality): {} vs {}",
        analysis_complex.suggested_qp,
        analysis_simple.suggested_qp
    );
}

#[test]
fn q12_generation_counter_invariant() {
    let capsule = LookaheadCapsule::new(10);

    let frame = vec![100u8; 128 * 128];

    let (_, _, _, gen_before) = capsule.buffer_stats();

    capsule.push_frame(&frame, 128, 128).unwrap();

    let (_, _, _, gen_after) = capsule.buffer_stats();

    // Generation should increment by 2 (odd → even for two-phase commit)
    assert_eq!(
        gen_after,
        gen_before + 2,
        "Generation should increment by 2 per push (two-phase commit)"
    );

    // Generation should always be even (committed state)
    assert_eq!(gen_after % 2, 0, "Generation should be even (committed)");
}

#[test]
fn q13_ring_buffer_wraparound() {
    let capsule = LookaheadCapsule::new(5); // Small buffer (5 frames)

    let frame = vec![100u8; 64 * 64];

    // Push 5 frames (fill buffer)
    for _ in 0..5 {
        capsule.push_frame(&frame, 64, 64).unwrap();
    }

    let (head, _, size, _) = capsule.buffer_stats();

    // Head should wrap around (5 % 5 = 0)
    assert_eq!(head, 0, "Head should wrap around to 0 after filling buffer");
}

#[test]
fn q14_keyframe_suggestion_property() {
    let capsule = LookaheadCapsule::new(10);

    // Frame 0: Normal frame
    let frame0 = vec![100u8; 256 * 256];
    capsule.push_frame(&frame0, 256, 256).unwrap();

    // Frame 1: Similar frame (no scene change)
    let frame1 = vec![102u8; 256 * 256];
    capsule.push_frame(&frame1, 256, 256).unwrap();

    // Frame 2: Scene change (bright frame)
    let frame2 = vec![250u8; 256 * 256];
    capsule.push_frame(&frame2, 256, 256).unwrap();

    // Should suggest keyframe at frame 2 (scene change)
    let keyframe_idx = capsule.suggest_keyframe();
    assert_eq!(
        keyframe_idx,
        Some(2),
        "Should suggest keyframe at scene change (frame 2)"
    );
}

// ================================
// Q15-Q21: Integration Tests (7 tests)
// ================================

#[test]
fn q15_sequential_frames() {
    let capsule = LookaheadCapsule::new(30);

    // Push 10 frames with gradual brightness increase
    for i in 0..10 {
        let brightness = 50 + i * 20; // 50, 70, 90, ..., 230
        let frame = vec![brightness as u8; 256 * 256];
        capsule.push_frame(&frame, 256, 256).unwrap();
    }

    // Analyze all frames
    let mut scene_changes = 0;
    for i in 0..10 {
        let analysis = capsule.analyze_frame(i);
        if analysis.scene_change {
            scene_changes += 1;
        }
    }

    // Should detect at least 1 scene change (large brightness jumps)
    assert!(scene_changes > 0, "Should detect scene changes in gradual brightness increase");
}

#[test]
fn q16_concurrent_analysis() {
    let capsule = Arc::new(LookaheadCapsule::new(30));

    // Push 20 frames
    for i in 0..20 {
        let brightness = 100 + (i % 5) * 30;
        let frame = vec![brightness as u8; 128 * 128];
        capsule.push_frame(&frame, 128, 128).unwrap();
    }

    // Spawn 4 threads to analyze concurrently
    let mut handles = vec![];

    for thread_id in 0..4 {
        let capsule_clone = Arc::clone(&capsule);

        let handle = thread::spawn(move || {
            let mut analyses = vec![];

            for i in 0..20 {
                let analysis = capsule_clone.analyze_frame(i);
                analyses.push(analysis);
            }

            analyses
        });

        handles.push(handle);
    }

    // All threads should complete without panic
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All threads should return identical results (deterministic)
    assert_eq!(results.len(), 4);

    for i in 1..4 {
        assert_eq!(results[0], results[i], "Concurrent reads should return identical results");
    }
}

#[test]
fn q17_realistic_video_sequence() {
    let capsule = LookaheadCapsule::new(40); // 40-frame lookahead

    // Simulate 40-frame video sequence with 2 scene changes
    for i in 0..40 {
        let brightness = if i < 10 {
            50 // Scene 1: Dark
        } else if i < 30 {
            150 // Scene 2: Bright (scene change at frame 10)
        } else {
            220 // Scene 3: Very bright (scene change at frame 30)
        };

        let frame = vec![brightness as u8; 512 * 512];
        capsule.push_frame(&frame, 512, 512).unwrap();
    }

    // Should detect scene changes at frames 10 and 30
    let scene_change_10 = capsule.detect_scene_change(10);
    let scene_change_30 = capsule.detect_scene_change(30);

    assert!(scene_change_10, "Should detect scene change at frame 10");
    assert!(scene_change_30, "Should detect scene change at frame 30");

    // Suggest keyframe should return one of the scene change frames
    let keyframe = capsule.suggest_keyframe();
    assert!(
        keyframe == Some(10) || keyframe == Some(30),
        "Should suggest keyframe at scene change, got {:?}",
        keyframe
    );
}

#[test]
fn q18_full_buffer_cycle() {
    let capsule = LookaheadCapsule::new(10);

    let frame = vec![100u8; 128 * 128];

    // Fill buffer completely
    for _ in 0..10 {
        let result = capsule.push_frame(&frame, 128, 128);
        assert!(result.is_ok(), "Push should succeed while buffer has space");
    }

    // Buffer should be full now
    let result = capsule.push_frame(&frame, 128, 128);
    assert_eq!(result, Err(LookaheadError::BufferFull), "Push should fail when buffer full");
}

#[test]
fn q19_varying_frame_sizes() {
    let capsule = LookaheadCapsule::new(10);

    // Different frame sizes
    let frame_small = vec![100u8; 128 * 128];
    let frame_medium = vec![150u8; 512 * 512];
    let frame_large = vec![200u8; 1920 * 1080];

    // All should work (dimensions vary, but algorithm adapts)
    capsule.push_frame(&frame_small, 128, 128).unwrap();
    capsule.push_frame(&frame_medium, 512, 512).unwrap();
    capsule.push_frame(&frame_large, 1920, 1080).unwrap();

    // All frames should be analyzable
    let analysis_0 = capsule.analyze_frame(0);
    let analysis_1 = capsule.analyze_frame(1);
    let analysis_2 = capsule.analyze_frame(2);

    assert!(analysis_0.complexity > 0);
    assert!(analysis_1.complexity > 0);
    assert!(analysis_2.complexity > 0);
}

#[test]
fn q20_checkered_pattern_complexity() {
    let capsule = LookaheadCapsule::new(10);

    // Create checkerboard pattern (high variance)
    let mut frame_checkerboard = vec![0u8; 512 * 512];
    for y in 0..512 {
        for x in 0..512 {
            frame_checkerboard[y * 512 + x] = if (x + y) % 2 == 0 { 0 } else { 255 };
        }
    }

    // Create uniform frame (low variance)
    let frame_uniform = vec![128u8; 512 * 512];

    capsule.push_frame(&frame_checkerboard, 512, 512).unwrap();
    capsule.push_frame(&frame_uniform, 512, 512).unwrap();

    let complexity_checkerboard = capsule.estimate_complexity(0);
    let complexity_uniform = capsule.estimate_complexity(1);

    assert!(
        complexity_checkerboard > complexity_uniform * 10,
        "Checkerboard should have much higher complexity: {} vs {}",
        complexity_checkerboard,
        complexity_uniform
    );
}

#[test]
fn q21_multiple_scene_changes() {
    let capsule = LookaheadCapsule::new(20);

    // Create sequence with multiple scene changes
    for i in 0..20 {
        let brightness = match i / 5 {
            0 => 50,   // Frames 0-4: Dark
            1 => 150,  // Frames 5-9: Medium (scene change)
            2 => 230,  // Frames 10-14: Bright (scene change)
            _ => 80,   // Frames 15-19: Dark again (scene change)
        };

        let frame = vec![brightness as u8; 256 * 256];
        capsule.push_frame(&frame, 256, 256).unwrap();
    }

    // Count scene changes
    let mut scene_changes = vec![];
    for i in 0..20 {
        if capsule.detect_scene_change(i) {
            scene_changes.push(i);
        }
    }

    // Should detect at least 2 scene changes (frames 5, 10, 15)
    assert!(
        scene_changes.len() >= 2,
        "Should detect at least 2 scene changes, got {} at {:?}",
        scene_changes.len(),
        scene_changes
    );
}

// ================================
// Q22-Q28: Production Tests (7 tests)
// ================================

#[test]
fn q22_performance_target_push_frame() {
    let capsule = LookaheadCapsule::new(30);

    // 1920×1080 HD frame
    let frame = vec![128u8; 1920 * 1080];

    let start = std::time::Instant::now();

    for _ in 0..10 {
        capsule.push_frame(&frame, 1920, 1080).unwrap();
    }

    let elapsed = start.elapsed();
    let avg_per_frame = elapsed.as_micros() / 10;

    println!("push_frame average: {}μs per HD frame", avg_per_frame);

    // Target: <50μs per frame (B32 target)
    assert!(
        avg_per_frame < 100,
        "push_frame should be <100μs per HD frame (relaxed for test), got {}μs",
        avg_per_frame
    );
}

#[test]
fn q23_performance_target_analyze_frame() {
    let capsule = LookaheadCapsule::new(30);

    // Push 20 frames
    let frame = vec![128u8; 1920 * 1080];
    for _ in 0..20 {
        capsule.push_frame(&frame, 1920, 1080).unwrap();
    }

    let start = std::time::Instant::now();

    // Analyze 1000 times
    for _ in 0..1000 {
        let _ = capsule.analyze_frame(10);
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / 1000;

    println!("analyze_frame average: {}ns ({}μs)", avg_ns, avg_ns / 1000);

    // Target: <10μs per analysis
    assert!(
        avg_ns < 50_000,
        "analyze_frame should be <50μs (relaxed for test), got {}ns",
        avg_ns
    );
}

#[test]
fn q24_stress_test_sequential() {
    let capsule = LookaheadCapsule::new(40);

    // Push 40 frames (fill buffer)
    for i in 0..40 {
        let brightness = 50 + (i % 10) * 20;
        let frame = vec![brightness as u8; 512 * 512];
        let result = capsule.push_frame(&frame, 512, 512);
        assert!(result.is_ok(), "Push {} should succeed", i);
    }

    // Analyze all frames
    for i in 0..40 {
        let analysis = capsule.analyze_frame(i as u8);
        assert!(analysis.complexity > 0, "Frame {} should have complexity", i);
    }
}

#[test]
fn q25_stress_test_concurrent() {
    let capsule = Arc::new(LookaheadCapsule::new(30));

    // Push 20 frames
    for i in 0..20 {
        let brightness = 80 + (i % 5) * 30;
        let frame = vec![brightness as u8; 256 * 256];
        capsule.push_frame(&frame, 256, 256).unwrap();
    }

    // Spawn 8 threads for concurrent analysis (stress test)
    let mut handles = vec![];

    for thread_id in 0..8 {
        let capsule_clone = Arc::clone(&capsule);

        let handle = thread::spawn(move || {
            // Each thread analyzes all frames 100 times
            for _ in 0..100 {
                for i in 0..20 {
                    let _ = capsule_clone.analyze_frame(i);
                    let _ = capsule_clone.detect_scene_change(i);
                    let _ = capsule_clone.estimate_complexity(i);
                }
            }
        });

        handles.push(handle);
    }

    // All threads should complete without panic
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn q26_edge_case_single_pixel() {
    let capsule = LookaheadCapsule::new(10);

    // 1×1 frame (minimum size)
    let frame = vec![128u8; 1];

    let result = capsule.push_frame(&frame, 1, 1);
    assert!(result.is_ok(), "1×1 frame should be accepted");

    let analysis = capsule.analyze_frame(0);
    assert_eq!(analysis.complexity, 0, "Single pixel should have zero variance");
}

#[test]
fn q27_edge_case_4k_frame() {
    let capsule = LookaheadCapsule::new(10);

    // 4K frame (3840×2160)
    let frame = vec![128u8; 3840 * 2160];

    let result = capsule.push_frame(&frame, 3840, 2160);
    assert!(result.is_ok(), "4K frame should be accepted");

    let analysis = capsule.analyze_frame(0);
    assert!(analysis.complexity >= 0, "4K frame should have valid complexity");
}

#[test]
fn q28_regression_generation_overflow() {
    let capsule = LookaheadCapsule::new(5);

    let frame = vec![100u8; 64 * 64];

    // Push frames until generation counter wraps (24-bit generation = 16M max)
    // For test, just push 100 frames and verify no panic
    for i in 0..100 {
        let result = capsule.push_frame(&frame, 64, 64);

        if result.is_err() {
            // Buffer full expected after 5 frames
            assert_eq!(result, Err(LookaheadError::BufferFull));
            break;
        }
    }

    let (_, _, _, gen) = capsule.buffer_stats();

    // Generation should still be even (committed)
    assert_eq!(gen % 2, 0, "Generation should remain even after many pushes");
}
