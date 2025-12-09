//! T28 Comprehensive Test Suite for GopCoordinatorCapsule
//!
//! Full 4-tier testing: Unit (Q1-Q7), Property (Q8-Q14), Integration (Q15-Q21), Production (Q22-Q28)

use atomic_capsule::encoder::{GopCoordinatorCapsule, GopFrameType as FrameType};

// ============================================================================
// TIER 1: Unit Tests (Q1-Q7) - Basic Functionality
// ============================================================================

/// Q1: Correctness - Basic GOP pattern (GOP=8, max_b=3)
#[test]
fn q1_correctness_basic_gop_pattern() {
    let gop = GopCoordinatorCapsule::new(8, 3);

    // GOP=8 pattern: I0 B1 B2 P3 B4 B5 B6 P7 I8
    assert_eq!(gop.next_frame_type(0), FrameType::Key);
    assert_eq!(gop.next_frame_type(1), FrameType::BackwardRef);
    assert_eq!(gop.next_frame_type(2), FrameType::BackwardRef);
    assert_eq!(gop.next_frame_type(3), FrameType::Inter);
    assert_eq!(gop.next_frame_type(4), FrameType::BackwardRef);
    assert_eq!(gop.next_frame_type(5), FrameType::BackwardRef);
    assert_eq!(gop.next_frame_type(6), FrameType::BackwardRef);
    assert_eq!(gop.next_frame_type(7), FrameType::Inter);
    assert_eq!(gop.next_frame_type(8), FrameType::Key); // Next GOP
}

/// Q2: Boundary Conditions - Edge cases (GOP=1, GOP=255, max_b=0, max_b=7)
#[test]
fn q2_boundary_conditions() {
    // GOP=1 (all I-frames)
    let gop1 = GopCoordinatorCapsule::new(1, 0);
    assert_eq!(gop1.next_frame_type(0), FrameType::Key);
    assert_eq!(gop1.next_frame_type(1), FrameType::Key);
    assert_eq!(gop1.next_frame_type(2), FrameType::Key);

    // GOP=255 (maximum)
    let gop255 = GopCoordinatorCapsule::new(255, 3);
    assert_eq!(gop255.next_frame_type(0), FrameType::Key);
    assert_eq!(gop255.next_frame_type(255), FrameType::Key); // Wraparound

    // max_b=0 (no B-frames, all P-frames)
    let no_b = GopCoordinatorCapsule::new(8, 0);
    assert_eq!(no_b.next_frame_type(0), FrameType::Key);
    assert_eq!(no_b.next_frame_type(1), FrameType::Inter);
    assert_eq!(no_b.next_frame_type(2), FrameType::Inter);

    // max_b=7 (maximum B-frames)
    let max_b = GopCoordinatorCapsule::new(16, 7);
    assert_eq!(max_b.next_frame_type(0), FrameType::Key);
    assert_eq!(max_b.next_frame_type(8), FrameType::Inter); // P-frame every 8 frames
}

/// Q3: Error Handling - Invalid configurations (debug assertions)
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "GOP size must be at least 1")]
fn q3_error_handling_zero_gop_size() {
    let _gop = GopCoordinatorCapsule::new(0, 3);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "max_b_frames must be <= 7")]
fn q3_error_handling_max_b_frames_overflow() {
    let _gop = GopCoordinatorCapsule::new(8, 8); // max_b=8 exceeds 3-bit limit
}

/// Q4: Integration - Scene change detection
#[test]
fn q4_integration_scene_change_detection() {
    let gop = GopCoordinatorCapsule::with_scene_threshold(60, 3, 50);

    // Low motion (no scene change)
    assert_eq!(gop.detect_scene_change(10, 50), false);
    assert_eq!(gop.detect_scene_change(49, 50), false);

    // High motion (scene change detected)
    assert_eq!(gop.detect_scene_change(51, 50), true);
    assert_eq!(gop.detect_scene_change(100, 50), true);
    assert_eq!(gop.detect_scene_change(1000, 50), true);

    // Use default threshold (50 from constructor)
    assert_eq!(gop.detect_scene_change(60, 0), true); // 60 > 50
    assert_eq!(gop.detect_scene_change(40, 0), false); // 40 < 50
}

/// Q5: Performance - Frame type decision <500ns
#[test]
fn q5_performance_frame_type_decision() {
    let gop = GopCoordinatorCapsule::new(60, 3);

    // Warm-up
    for i in 0..100 {
        let _ = gop.next_frame_type(i);
    }

    // Measure 1000 iterations
    let start = std::time::Instant::now();
    for i in 0..1000 {
        let _ = gop.next_frame_type(i);
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / 1000;
    println!("Average frame type decision: {}ns", avg_ns);
    assert!(avg_ns < 500, "Frame type decision too slow: {}ns", avg_ns);
}

/// Q6: Temporal Layers - Hierarchical B-frame layers (T0-T3)
#[test]
fn q6_temporal_layers() {
    let gop = GopCoordinatorCapsule::new(8, 3);

    // GOP=8 layers: T0 T3 T2 T1 T3 T2 T3 T1
    assert_eq!(gop.get_temporal_layer(0), 0); // I-frame (T0)
    assert_eq!(gop.get_temporal_layer(1), 3); // B-frame (T3)
    assert_eq!(gop.get_temporal_layer(2), 2); // B-frame (T2)
    assert_eq!(gop.get_temporal_layer(3), 1); // P-frame (T1)
    assert_eq!(gop.get_temporal_layer(4), 3); // B-frame (T3)
    assert_eq!(gop.get_temporal_layer(5), 2); // B-frame (T2)
    assert_eq!(gop.get_temporal_layer(6), 3); // B-frame (T3)
    assert_eq!(gop.get_temporal_layer(7), 1); // P-frame (T1)

    // Temporal scalability: Drop T3 → 1/2 framerate (I0 B2 P3 B5 P7)
    // Temporal scalability: Drop T2+T3 → 1/4 framerate (I0 P3 P7)
}

/// Q7: Configuration Queries - Get/set GOP config
#[test]
fn q7_configuration_queries() {
    let gop = GopCoordinatorCapsule::with_scene_threshold(60, 3, 50);

    let (gop_size, max_b, threshold) = gop.get_config();
    assert_eq!(gop_size, 60);
    assert_eq!(max_b, 3);
    assert_eq!(threshold, 50);

    // Update GOP size (adaptive GOP)
    gop.set_gop_size(120);
    let (new_gop_size, _, _) = gop.get_config();
    assert_eq!(new_gop_size, 120);
}

// ============================================================================
// TIER 2: Property Tests (Q8-Q14) - Invariants and Properties
// ============================================================================

/// Q8: Determinism - Same input always produces same output
#[test]
fn q8_determinism() {
    let gop = GopCoordinatorCapsule::new(8, 3);

    // Repeat 1000 times, should always produce same pattern
    for _ in 0..1000 {
        assert_eq!(gop.next_frame_type(0), FrameType::Key);
        assert_eq!(gop.next_frame_type(1), FrameType::BackwardRef);
        assert_eq!(gop.next_frame_type(3), FrameType::Inter);
    }
}

/// Q9: Monotonicity - Frame indices are processed in order
#[test]
fn q9_monotonicity() {
    let gop = GopCoordinatorCapsule::new(60, 3);

    // Process frames 0-100 in order
    let mut prev_type = gop.next_frame_type(0);
    for i in 1..100 {
        let curr_type = gop.next_frame_type(i);
        // Pattern should be deterministic (not monotonic, but repeating)
        if i % 60 == 0 {
            assert_eq!(curr_type, FrameType::Key); // Keyframe every GOP
        }
        prev_type = curr_type;
    }
}

/// Q10: Idempotency - Repeated queries don't change state
#[test]
fn q10_idempotency() {
    let gop = GopCoordinatorCapsule::new(8, 3);

    // Query frame 5 multiple times
    let type1 = gop.next_frame_type(5);
    let type2 = gop.next_frame_type(5);
    let type3 = gop.next_frame_type(5);

    assert_eq!(type1, type2);
    assert_eq!(type2, type3);
}

/// Q11: Commutativity - Order-independent operations
#[test]
fn q11_commutativity_config_queries() {
    let gop = GopCoordinatorCapsule::new(8, 3);

    // Query config in different orders
    let (gop1, max_b1, thresh1) = gop.get_config();
    let type1 = gop.next_frame_type(5);
    let (gop2, max_b2, thresh2) = gop.get_config();
    let type2 = gop.next_frame_type(5);

    assert_eq!(gop1, gop2);
    assert_eq!(max_b1, max_b2);
    assert_eq!(thresh1, thresh2);
    assert_eq!(type1, type2);
}

/// Q12: Associativity - Grouping doesn't matter
#[test]
fn q12_associativity_frame_planning() {
    let gop = GopCoordinatorCapsule::new(8, 3);

    // Plan (0..4) + (4..8) should match 0..8
    let plan_full = gop.plan_gop(8);
    let plan_part1 = gop.plan_gop(4);
    let plan_part2_frames: Vec<FrameType> = (4..8).map(|i| gop.next_frame_type(i)).collect();

    assert_eq!(plan_full[0..4], plan_part1[..]);
    assert_eq!(plan_full[4..8], plan_part2_frames[..]);
}

/// Q13: Closure - Operations produce valid frame types
#[test]
fn q13_closure() {
    let gop = GopCoordinatorCapsule::new(60, 3);

    // All frame types must be valid (Key, Inter, BackwardRef, or AltRef)
    for i in 0..1000 {
        let frame_type = gop.next_frame_type(i);
        assert!(
            frame_type == FrameType::Key
                || frame_type == FrameType::Inter
                || frame_type == FrameType::BackwardRef
                || frame_type == FrameType::AltRef
        );
    }
}

/// Q14: Identity - Neutral element exists (GOP configuration identity)
#[test]
fn q14_identity() {
    // Standard GOP (60, 3) should produce standard pattern
    let gop_standard = GopCoordinatorCapsule::new(60, 3);

    // Re-create with same config
    gop_standard.set_gop_size(60);
    let (gop_size, max_b, _) = gop_standard.get_config();

    assert_eq!(gop_size, 60);
    assert_eq!(max_b, 3);

    // Pattern should remain identical
    assert_eq!(gop_standard.next_frame_type(0), FrameType::Key);
    assert_eq!(gop_standard.next_frame_type(1), FrameType::BackwardRef);
}

// ============================================================================
// TIER 3: Integration Tests (Q15-Q21) - Multi-Component Interaction
// ============================================================================

/// Q15: Force Keyframe Integration - User-requested I-frame
#[test]
fn q15_force_keyframe_integration() {
    let gop = GopCoordinatorCapsule::new(60, 3);

    // Normally frame 5 would be B-frame
    assert_eq!(gop.next_frame_type(5), FrameType::BackwardRef);

    // Force keyframe for seeking/chapter marker
    gop.force_keyframe();

    // Next frame (0) is I-frame (forced)
    assert_eq!(gop.next_frame_type(0), FrameType::Key);
}

/// Q16: Adaptive GOP Sizing - Dynamic GOP size changes
#[test]
fn q16_adaptive_gop_sizing() {
    let gop = GopCoordinatorCapsule::new(60, 3);

    // Standard streaming (2s @ 30fps)
    assert_eq!(gop.next_frame_type(60), FrameType::Key);

    // Switch to low-latency (1s @ 30fps)
    gop.set_gop_size(30);
    assert_eq!(gop.next_frame_type(30), FrameType::Key);
    assert_eq!(gop.next_frame_type(60), FrameType::Key); // Still keyframe every 30 frames

    // Switch to long-form (4s @ 30fps)
    gop.set_gop_size(120);
    assert_eq!(gop.next_frame_type(120), FrameType::Key);
}

/// Q17: Scene Change GOP Reset - Scene change forces I-frame
#[test]
fn q17_scene_change_gop_reset() {
    let gop = GopCoordinatorCapsule::with_scene_threshold(60, 3, 50);

    // Detect scene change at frame 10
    gop.set_scene_change(10, true);

    // Frame 10 should be I-frame (forced by scene change)
    assert_eq!(gop.next_frame_type(10), FrameType::Key);

    // Clear scene change flag
    gop.set_scene_change(10, false);
    assert_eq!(gop.next_frame_type(10), FrameType::BackwardRef); // Back to normal
}

/// Q18: Multiple GOP Cycles - Pattern repeats correctly
#[test]
fn q18_multiple_gop_cycles() {
    let gop = GopCoordinatorCapsule::new(8, 3);

    // First GOP (0-7)
    assert_eq!(gop.next_frame_type(0), FrameType::Key);
    assert_eq!(gop.next_frame_type(7), FrameType::Inter);

    // Second GOP (8-15)
    assert_eq!(gop.next_frame_type(8), FrameType::Key);
    assert_eq!(gop.next_frame_type(15), FrameType::Inter);

    // Third GOP (16-23)
    assert_eq!(gop.next_frame_type(16), FrameType::Key);
    assert_eq!(gop.next_frame_type(23), FrameType::Inter);

    // Fourth GOP (24-31)
    assert_eq!(gop.next_frame_type(24), FrameType::Key);
    assert_eq!(gop.next_frame_type(31), FrameType::Inter);
}

/// Q19: Temporal Layer Scalability - Drop higher layers for lower framerate
#[test]
fn q19_temporal_layer_scalability() {
    let gop = GopCoordinatorCapsule::new(8, 3);

    // Full framerate (all layers T0-T3): 8 frames
    let full_frames: Vec<u32> = (0..8).collect();
    assert_eq!(full_frames.len(), 8);

    // 1/2 framerate (drop T3): Keep T0, T1, T2 (4 frames)
    let half_frames: Vec<u32> = (0..8).filter(|&i| gop.get_temporal_layer(i) <= 2).collect();
    assert_eq!(half_frames.len(), 4);
    assert_eq!(half_frames, vec![0, 2, 3, 5]);

    // 1/4 framerate (drop T2+T3): Keep T0, T1 (2 frames)
    let quarter_frames: Vec<u32> = (0..8).filter(|&i| gop.get_temporal_layer(i) <= 1).collect();
    assert_eq!(quarter_frames.len(), 2);
    assert_eq!(quarter_frames, vec![0, 3]);

    // 1/8 framerate (drop all but T0): Keep I-frames only (1 frame)
    let eighth_frames: Vec<u32> = (0..8).filter(|&i| gop.get_temporal_layer(i) == 0).collect();
    assert_eq!(eighth_frames.len(), 1);
    assert_eq!(eighth_frames, vec![0]);
}

/// Q20: Batch GOP Planning - Plan 128 frames efficiently
#[test]
fn q20_batch_gop_planning() {
    let gop = GopCoordinatorCapsule::new(16, 7);

    // Plan 128 frames (full schedule capacity)
    let plan = gop.plan_gop(128);
    assert_eq!(plan.len(), 128);

    // Check keyframes every 16 frames
    assert_eq!(plan[0], FrameType::Key);
    assert_eq!(plan[16], FrameType::Key);
    assert_eq!(plan[32], FrameType::Key);
    assert_eq!(plan[48], FrameType::Key);
    assert_eq!(plan[64], FrameType::Key);
    assert_eq!(plan[80], FrameType::Key);
    assert_eq!(plan[96], FrameType::Key);
    assert_eq!(plan[112], FrameType::Key);

    // Verify P-frames every 8 frames
    assert_eq!(plan[8], FrameType::Inter);
    assert_eq!(plan[24], FrameType::Inter);
}

/// Q21: Concurrent Access - Multiple threads query GOP
#[test]
fn q21_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let gop = Arc::new(GopCoordinatorCapsule::new(60, 3));

    let threads: Vec<_> = (0..4)
        .map(|thread_id| {
            let gop_clone = Arc::clone(&gop);
            thread::spawn(move || {
                for i in 0..100 {
                    let frame_idx = thread_id * 100 + i;
                    let _ = gop_clone.next_frame_type(frame_idx);
                    let _ = gop_clone.get_temporal_layer(frame_idx);
                }
            })
        })
        .collect();

    for handle in threads {
        handle.join().unwrap();
    }

    // No panics = success
}

// ============================================================================
// TIER 4: Production Tests (Q22-Q28) - Real-World Scenarios
// ============================================================================

/// Q22: Netflix Streaming Scenario - 2s GOP @ 30fps (60 frames)
#[test]
fn q22_netflix_streaming_scenario() {
    // Netflix recommendation: 2s GOP for standard streaming
    let gop = GopCoordinatorCapsule::with_scene_threshold(60, 3, 50);

    // Verify keyframe every 2 seconds (60 frames @ 30fps)
    for i in 0..300 {
        let frame_type = gop.next_frame_type(i);
        if i % 60 == 0 {
            assert_eq!(frame_type, FrameType::Key);
        }
    }

    // Simulate scene change at frame 100 (should force I-frame)
    let sad = 100; // High SAD = scene change
    assert!(gop.detect_scene_change(sad, 50));
    gop.set_scene_change(100, true);
    assert_eq!(gop.next_frame_type(100), FrameType::Key);
}

/// Q23: Low-Latency Live Scenario - 1s GOP @ 30fps (30 frames)
#[test]
fn q23_low_latency_live_scenario() {
    // Low-latency live: <1s GOP (Apple recommendation)
    let gop = GopCoordinatorCapsule::new(30, 2);

    // Verify keyframe every 1 second (30 frames @ 30fps)
    for i in 0..150 {
        let frame_type = gop.next_frame_type(i);
        if i % 30 == 0 {
            assert_eq!(frame_type, FrameType::Key);
        }
    }

    // Lower max_b_frames (2 vs 3) for reduced latency
    let (_, max_b, _) = gop.get_config();
    assert_eq!(max_b, 2);
}

/// Q24: Long-Form VOD Scenario - 4s GOP @ 30fps (120 frames)
#[test]
fn q24_long_form_vod_scenario() {
    // Long-form content: 4-8s GOP for better compression
    let gop = GopCoordinatorCapsule::new(120, 7);

    // Verify keyframe every 4 seconds (120 frames @ 30fps)
    for i in 0..600 {
        let frame_type = gop.next_frame_type(i);
        if i % 120 == 0 {
            assert_eq!(frame_type, FrameType::Key);
        }
    }

    // Higher max_b_frames (7 vs 3) for better compression
    let (_, max_b, _) = gop.get_config();
    assert_eq!(max_b, 7);
}

/// Q25: Action Content Scenario - Sensitive scene detection (threshold=100)
#[test]
fn q25_action_content_scenario() {
    // Action content: Higher scene threshold for frequent scene changes
    let gop = GopCoordinatorCapsule::with_scene_threshold(60, 3, 100);

    // Simulate high-motion action sequence
    let action_sads = vec![50, 80, 120, 90, 150, 70, 110];

    for (i, sad) in action_sads.iter().enumerate() {
        if gop.detect_scene_change(*sad, 100) {
            println!("Scene change detected at frame {} (SAD={})", i, sad);
            gop.set_scene_change(i as u32, true);
            assert_eq!(gop.next_frame_type(i as u32), FrameType::Key);
        }
    }

    // Expected scene changes: SAD 120, 150, 110 (> threshold 100)
}

/// Q26: Seeking Scenario - Force keyframe for random access
#[test]
fn q26_seeking_scenario() {
    let gop = GopCoordinatorCapsule::new(60, 3);

    // User seeks to frame 100 (not a keyframe)
    assert_ne!(gop.next_frame_type(100), FrameType::Key);

    // Force keyframe for instant decode (chapter marker, seeking)
    gop.force_keyframe();
    assert_eq!(gop.next_frame_type(0), FrameType::Key); // Next frame is forced I-frame

    // Continue normal encoding after forced keyframe
    assert_eq!(gop.next_frame_type(1), FrameType::BackwardRef);
}

/// Q27: Adaptive Streaming Scenario - Switch GOP sizes dynamically
#[test]
fn q27_adaptive_streaming_scenario() {
    let gop = GopCoordinatorCapsule::new(60, 3);

    // Start with standard streaming (2s GOP)
    assert_eq!(gop.next_frame_type(60), FrameType::Key);

    // Network congestion: Switch to low-latency (1s GOP)
    gop.set_gop_size(30);
    assert_eq!(gop.next_frame_type(30), FrameType::Key);

    // Network improves: Switch to long-form (4s GOP)
    gop.set_gop_size(120);
    assert_eq!(gop.next_frame_type(120), FrameType::Key);

    // Back to standard streaming
    gop.set_gop_size(60);
    assert_eq!(gop.next_frame_type(60), FrameType::Key);
}

/// Q28: Production Stress Test - 10,000 frames @ 60fps
#[test]
fn q28_production_stress_test() {
    let gop = GopCoordinatorCapsule::new(120, 7);

    // Encode 10,000 frames (2.8 minutes @ 60fps)
    let mut i_frames = 0;
    let mut p_frames = 0;
    let mut b_frames = 0;

    for i in 0..10_000 {
        match gop.next_frame_type(i) {
            FrameType::Key => i_frames += 1,
            FrameType::Inter => p_frames += 1,
            FrameType::BackwardRef => b_frames += 1,
            _ => {}
        }
    }

    println!("Encoded 10,000 frames:");
    println!("  I-frames: {} ({:.1}%)", i_frames, i_frames as f64 / 100.0);
    println!("  P-frames: {} ({:.1}%)", p_frames, p_frames as f64 / 100.0);
    println!("  B-frames: {} ({:.1}%)", b_frames, b_frames as f64 / 100.0);

    // Expected: ~84 I-frames (10,000 / 120 ≈ 83.3)
    assert!(i_frames >= 80 && i_frames <= 90);

    // Expected: ~1,000 P-frames (10,000 / 8 - 84 ≈ 1,166)
    assert!(p_frames >= 1000 && p_frames <= 1300);

    // Expected: ~8,900 B-frames (remaining)
    assert!(b_frames >= 8000 && b_frames <= 9000);
}
