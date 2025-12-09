//! Reference Frame Management Tests (T28 5-Tier Framework)
//!
//! - Q1-Q7: Unit Tests
//! - Q8-Q14: Property Tests
//! - Q15-Q21: Integration Tests
//! - Q22-Q28: Production Tests
//! - Q29-Q35: Determinism Tests (Future Wave)

use atomic_capsule::encoder::ReferenceType;
use kindly_av1::encoder::{
    FrameUpdateStrategy, ReferenceFrameManagerCapsule, ReferenceManagerError,
};
use kindly_av1::pipeline::frame_pool::{FrameBufferPoolCapsule, PoolConfig};

// ============================================================================
// Q1-Q7: UNIT TESTS
// ============================================================================

#[test]
fn q1_manager_layout() {
    // Verify 1024B cache alignment
    assert_eq!(core::mem::size_of::<ReferenceFrameManagerCapsule>(), 1024);
    assert_eq!(core::mem::align_of::<ReferenceFrameManagerCapsule>(), 1024);
}

#[test]
fn q2_manager_creation() {
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let stats = manager.stats();
    assert_eq!(stats.total_frames, 0);
    assert_eq!(stats.keyframes, 0);
    assert_eq!(stats.p_frames, 0);
    assert_eq!(stats.b_frames, 0);
    assert_eq!(stats.golden_refreshes, 0);
    assert_eq!(stats.golden_age, 0);
}

#[test]
fn q3_keyframe_update() {
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0x1000 as *const u8;
    manager
        .update_frame(frame_ptr, 0, 0, FrameUpdateStrategy::Keyframe, false)
        .unwrap();

    let stats = manager.stats();
    assert_eq!(stats.total_frames, 1);
    assert_eq!(stats.keyframes, 1);
    assert_eq!(stats.golden_age, 0); // Reset on keyframe

    // All 7 reference slots should point to keyframe
    assert!(manager.get_reference(ReferenceType::Last).is_some());
    assert!(manager.get_reference(ReferenceType::Last2).is_some());
    assert!(manager.get_reference(ReferenceType::Last3).is_some());
    assert!(manager.get_reference(ReferenceType::Golden).is_some());
    assert!(manager.get_reference(ReferenceType::Backward).is_some());
    assert!(manager.get_reference(ReferenceType::AltRef2).is_some());
    assert!(manager.get_reference(ReferenceType::AltRef).is_some());
}

#[test]
fn q4_p_frame_update() {
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0x2000 as *const u8;
    manager
        .update_frame(
            frame_ptr,
            1,
            1,
            FrameUpdateStrategy::PFrame {
                refresh_golden: false,
            },
            false,
        )
        .unwrap();

    let stats = manager.stats();
    assert_eq!(stats.total_frames, 1);
    assert_eq!(stats.p_frames, 1);
    assert_eq!(stats.golden_age, 1); // Incremented

    // LAST slot should be updated
    assert!(manager.get_reference(ReferenceType::Last).is_some());
}

#[test]
fn q5_b_frame_update() {
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0x3000 as *const u8;
    manager
        .update_frame(frame_ptr, 1, 1, FrameUpdateStrategy::BFrame, false)
        .unwrap();

    let stats = manager.stats();
    assert_eq!(stats.total_frames, 1);
    assert_eq!(stats.b_frames, 1);
    assert_eq!(stats.altref_updates, 1);

    // BWDREF slot should be updated
    assert!(manager.get_reference(ReferenceType::Backward).is_some());
}

#[test]
fn q6_golden_refresh_forced() {
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 64); // Long period

    let frame_ptr = 0x4000 as *const u8;
    manager
        .update_frame(
            frame_ptr,
            0,
            0,
            FrameUpdateStrategy::PFrame {
                refresh_golden: true,
            },
            false,
        )
        .unwrap();

    let stats = manager.stats();
    assert_eq!(stats.golden_refreshes, 1);
    assert_eq!(stats.golden_age, 0); // Reset on refresh
}

#[test]
fn q7_buffer_allocation() {
    let config = PoolConfig {
        max_buffers: 4,
        ..Default::default()
    };
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    // Allocate all buffers
    let mut handles = Vec::new();
    for _ in 0..4 {
        let handle = manager.allocate_buffer().unwrap();
        handles.push(handle);
    }

    // Pool exhausted
    assert!(matches!(
        manager.allocate_buffer(),
        Err(ReferenceManagerError::PoolExhausted)
    ));

    // Release and retry
    manager.release_buffer(handles.pop().unwrap()).unwrap();
    assert!(manager.allocate_buffer().is_ok());
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS
// ============================================================================

#[test]
fn q8_golden_age_invariant() {
    // Property: golden_age increments with each frame, resets on refresh
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 16);

    let frame_ptr = 0x5000 as *const u8;

    for i in 0..20 {
        let stats = manager.stats();
        let refresh_golden = manager.should_refresh_golden(false);

        manager
            .update_frame(
                frame_ptr,
                i,
                i as u8,
                FrameUpdateStrategy::PFrame { refresh_golden },
                false,
            )
            .unwrap();

        let new_stats = manager.stats();

        if refresh_golden {
            // Age should reset to 0
            assert_eq!(
                new_stats.golden_age, 0,
                "GOLDEN age should reset on refresh"
            );
        } else {
            // Age should increment
            assert!(
                new_stats.golden_age > stats.golden_age,
                "GOLDEN age should increment: {} -> {}",
                stats.golden_age,
                new_stats.golden_age
            );
        }
    }
}

#[test]
fn q9_stats_consistency() {
    // Property: total_frames = keyframes + p_frames + b_frames
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0x6000 as *const u8;

    // Process 100 frames (mix of types)
    for i in 0..100 {
        let strategy = match i % 10 {
            0 => FrameUpdateStrategy::Keyframe,
            1 | 2 => FrameUpdateStrategy::BFrame,
            _ => FrameUpdateStrategy::PFrame {
                refresh_golden: false,
            },
        };

        manager
            .update_frame(frame_ptr, i, i as u8, strategy, false)
            .unwrap();
    }

    let stats = manager.stats();
    assert_eq!(
        stats.total_frames,
        stats.keyframes + stats.p_frames + stats.b_frames,
        "Frame type counts should sum to total"
    );
}

#[test]
fn q10_reference_slot_validity() {
    // Property: get_reference returns Some after keyframe for all slots
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0x7000 as *const u8;
    manager
        .update_frame(frame_ptr, 0, 0, FrameUpdateStrategy::Keyframe, false)
        .unwrap();

    // All 7 reference types should be valid
    for i in 0..7 {
        let ref_type = ReferenceType::from_slot(i).unwrap();
        assert!(
            manager.get_reference(ref_type).is_some(),
            "Slot {} should be valid after keyframe",
            i
        );
    }
}

#[test]
fn q11_golden_period_clamping() {
    // Property: golden_period is clamped to 16-64 range
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();

    // Test below range
    let manager = ReferenceFrameManagerCapsule::new(&pool, 8);
    assert!(!manager.should_refresh_golden(false)); // Should use clamped period

    // Test above range
    let manager = ReferenceFrameManagerCapsule::new(&pool, 128);
    let frame_ptr = 0x8000 as *const u8;

    // Process 70 frames (more than 64)
    for i in 0..70 {
        manager
            .update_frame(
                frame_ptr,
                i,
                i as u8,
                FrameUpdateStrategy::PFrame {
                    refresh_golden: false,
                },
                false,
            )
            .unwrap();
    }

    let stats = manager.stats();
    assert!(
        stats.golden_refreshes >= 1,
        "GOLDEN should refresh within 64 frames (clamped max)"
    );
}

#[test]
fn q12_scene_change_forces_refresh() {
    // Property: scene_change=true always forces GOLDEN refresh
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 64); // Long period

    // Scene change should force refresh even at frame 1
    assert!(manager.should_refresh_golden(true));

    let frame_ptr = 0x9000 as *const u8;
    manager
        .update_frame(
            frame_ptr,
            0,
            0,
            FrameUpdateStrategy::PFrame {
                refresh_golden: true,
            },
            true, // Scene change
        )
        .unwrap();

    let stats = manager.stats();
    assert_eq!(stats.golden_refreshes, 1);
}

#[test]
fn q13_altref_update_counter() {
    // Property: altref_updates increments only for B-frames
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0xA000 as *const u8;

    // Process 10 B-frames
    for i in 0..10 {
        manager
            .update_frame(frame_ptr, i, i as u8, FrameUpdateStrategy::BFrame, false)
            .unwrap();
    }

    let stats = manager.stats();
    assert_eq!(stats.altref_updates, 10);
    assert_eq!(stats.b_frames, 10);

    // P-frames should not increment ALTREF
    manager
        .update_frame(
            frame_ptr,
            10,
            10,
            FrameUpdateStrategy::PFrame {
                refresh_golden: false,
            },
            false,
        )
        .unwrap();

    let stats = manager.stats();
    assert_eq!(stats.altref_updates, 10); // Unchanged
}

#[test]
fn q14_pool_integration() {
    // Property: manager.allocate_buffer() ≡ pool.try_acquire()
    let config = PoolConfig {
        max_buffers: 8,
        ..Default::default()
    };
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let pool_available = pool.available_count();
    let handle = manager.allocate_buffer().unwrap();
    assert_eq!(pool.available_count(), pool_available - 1);

    manager.release_buffer(handle).unwrap();
    assert_eq!(pool.available_count(), pool_available);
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS
// ============================================================================

#[test]
fn q15_full_gop_simulation() {
    // Simulate GOP structure: K + 15 P-frames
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 16);

    let frame_ptr = 0xB000 as *const u8;

    // Keyframe
    manager
        .update_frame(frame_ptr, 0, 0, FrameUpdateStrategy::Keyframe, false)
        .unwrap();

    // 15 P-frames
    for i in 1..16 {
        let refresh_golden = manager.should_refresh_golden(false);
        manager
            .update_frame(
                frame_ptr,
                i,
                i as u8,
                FrameUpdateStrategy::PFrame { refresh_golden },
                false,
            )
            .unwrap();
    }

    let stats = manager.stats();
    assert_eq!(stats.total_frames, 16);
    assert_eq!(stats.keyframes, 1);
    assert_eq!(stats.p_frames, 15);
    assert!(
        stats.golden_refreshes >= 1,
        "GOLDEN should refresh within GOP"
    );
}

#[test]
fn q16_hierarchical_b_frames() {
    // Simulate hierarchical B-frame structure: K + B + P + B + P
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0xC000 as *const u8;

    // Keyframe
    manager
        .update_frame(frame_ptr, 0, 0, FrameUpdateStrategy::Keyframe, false)
        .unwrap();

    // B-frame
    manager
        .update_frame(frame_ptr, 1, 1, FrameUpdateStrategy::BFrame, false)
        .unwrap();

    // P-frame
    manager
        .update_frame(
            frame_ptr,
            2,
            2,
            FrameUpdateStrategy::PFrame {
                refresh_golden: false,
            },
            false,
        )
        .unwrap();

    // B-frame
    manager
        .update_frame(frame_ptr, 3, 3, FrameUpdateStrategy::BFrame, false)
        .unwrap();

    // P-frame
    manager
        .update_frame(
            frame_ptr,
            4,
            4,
            FrameUpdateStrategy::PFrame {
                refresh_golden: false,
            },
            false,
        )
        .unwrap();

    let stats = manager.stats();
    assert_eq!(stats.total_frames, 5);
    assert_eq!(stats.keyframes, 1);
    assert_eq!(stats.p_frames, 2);
    assert_eq!(stats.b_frames, 2);
    assert_eq!(stats.altref_updates, 2);

    // All reference slots should remain valid
    for i in 0..7 {
        let ref_type = ReferenceType::from_slot(i).unwrap();
        assert!(
            manager.get_reference(ref_type).is_some(),
            "Slot {} should be valid",
            i
        );
    }
}

#[test]
fn q17_scene_change_integration() {
    // Simulate scene change at frame 10
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 64); // Long period

    let frame_ptr = 0xD000 as *const u8;

    // Keyframe
    manager
        .update_frame(frame_ptr, 0, 0, FrameUpdateStrategy::Keyframe, false)
        .unwrap();

    // 9 P-frames (no scene change)
    for i in 1..10 {
        manager
            .update_frame(
                frame_ptr,
                i,
                i as u8,
                FrameUpdateStrategy::PFrame {
                    refresh_golden: false,
                },
                false,
            )
            .unwrap();
    }

    let stats_before = manager.stats();
    assert_eq!(stats_before.golden_refreshes, 1); // Keyframe refreshed GOLDEN

    // Frame 10 with scene change
    manager
        .update_frame(
            frame_ptr,
            10,
            10,
            FrameUpdateStrategy::PFrame {
                refresh_golden: true,
            },
            true, // Scene change
        )
        .unwrap();

    let stats_after = manager.stats();
    assert_eq!(stats_after.golden_refreshes, 2); // Keyframe + scene change refresh
    assert_eq!(stats_after.golden_age, 0); // Reset
}

#[test]
fn q18_switch_frame_update() {
    // Test switch frame (all slots updated, no intra coding)
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0xE000 as *const u8;

    manager
        .update_frame(frame_ptr, 0, 0, FrameUpdateStrategy::SwitchFrame, false)
        .unwrap();

    // All 7 reference slots should be valid
    for i in 0..7 {
        let ref_type = ReferenceType::from_slot(i).unwrap();
        assert!(
            manager.get_reference(ref_type).is_some(),
            "Slot {} should be valid after switch frame",
            i
        );
    }
}

#[test]
fn q19_buffer_pool_exhaustion_recovery() {
    // Test recovery from pool exhaustion
    let config = PoolConfig {
        max_buffers: 2,
        ..Default::default()
    };
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    // Allocate all buffers
    let h1 = manager.allocate_buffer().unwrap();
    let h2 = manager.allocate_buffer().unwrap();

    // Exhausted
    assert!(matches!(
        manager.allocate_buffer(),
        Err(ReferenceManagerError::PoolExhausted)
    ));

    // Release one
    manager.release_buffer(h1).unwrap();

    // Should allocate again
    let _h3 = manager.allocate_buffer().unwrap();

    // Cleanup
    manager.release_buffer(h2).unwrap();
}

#[test]
fn q20_multiple_golden_refreshes() {
    // Test multiple GOLDEN refreshes over long sequence
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 16);

    let frame_ptr = 0xF000 as *const u8;

    // Process 64 P-frames (4 golden periods)
    for i in 0..64 {
        let refresh_golden = manager.should_refresh_golden(false);
        manager
            .update_frame(
                frame_ptr,
                i,
                i as u8,
                FrameUpdateStrategy::PFrame { refresh_golden },
                false,
            )
            .unwrap();
    }

    let stats = manager.stats();
    assert!(
        stats.golden_refreshes >= 3,
        "Expected at least 3 GOLDEN refreshes in 64 frames, got {}",
        stats.golden_refreshes
    );
}

#[test]
fn q21_mixed_frame_types() {
    // Test realistic mix of frame types
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0x10000 as *const u8;

    // Realistic GOP: K + 7P + 8B interleaved
    manager
        .update_frame(frame_ptr, 0, 0, FrameUpdateStrategy::Keyframe, false)
        .unwrap();

    for i in 1..16 {
        let strategy = if i % 2 == 0 {
            FrameUpdateStrategy::BFrame
        } else {
            FrameUpdateStrategy::PFrame {
                refresh_golden: false,
            }
        };
        manager
            .update_frame(frame_ptr, i, i as u8, strategy, false)
            .unwrap();
    }

    let stats = manager.stats();
    assert_eq!(stats.total_frames, 16);
    assert_eq!(stats.keyframes, 1);
    assert_eq!(stats.p_frames, 8); // Frames 1,3,5,7,9,11,13,15 (odd)
    assert_eq!(stats.b_frames, 7); // Frames 2,4,6,8,10,12,14 (even)
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS
// ============================================================================

#[test]
fn q22_high_throughput_updates() {
    // 1000 frame sequence
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0x11000 as *const u8;

    for i in 0..1000 {
        let strategy = match i % 30 {
            0 => FrameUpdateStrategy::Keyframe,
            1..=3 => FrameUpdateStrategy::BFrame,
            _ => FrameUpdateStrategy::PFrame {
                refresh_golden: false,
            },
        };
        manager
            .update_frame(frame_ptr, i, (i & 0xFF) as u8, strategy, false)
            .unwrap();
    }

    let stats = manager.stats();
    assert_eq!(stats.total_frames, 1000);
    assert!(stats.keyframes >= 33); // At least every 30 frames
    assert!(stats.golden_refreshes > 0);
}

#[test]
fn q23_4k_workflow() {
    // Test with 4K configuration
    let config = PoolConfig::preset_4k();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0x12000 as *const u8;

    // Process 100 frames
    for i in 0..100 {
        let strategy = match i {
            0 => FrameUpdateStrategy::Keyframe,
            _ if i % 10 == 0 => FrameUpdateStrategy::PFrame {
                refresh_golden: true,
            },
            _ => FrameUpdateStrategy::PFrame {
                refresh_golden: false,
            },
        };
        manager
            .update_frame(frame_ptr, i, (i & 0xFF) as u8, strategy, false)
            .unwrap();
    }

    let stats = manager.stats();
    assert_eq!(stats.total_frames, 100);
    assert!(stats.golden_refreshes >= 9); // Every 10 frames forced
}

#[test]
fn q24_rapid_scene_changes() {
    // Simulate rapid scene changes (every 5 frames)
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 64);

    let frame_ptr = 0x13000 as *const u8;

    for i in 0..50 {
        let scene_change = i % 5 == 0;
        let refresh_golden = manager.should_refresh_golden(scene_change);
        manager
            .update_frame(
                frame_ptr,
                i,
                (i & 0xFF) as u8,
                FrameUpdateStrategy::PFrame { refresh_golden },
                scene_change,
            )
            .unwrap();
    }

    let stats = manager.stats();
    assert!(
        stats.golden_refreshes >= 9,
        "Expected at least 9 GOLDEN refreshes with rapid scene changes, got {}",
        stats.golden_refreshes
    );
}

#[test]
fn q25_all_reference_slots_used() {
    // Verify all 7 reference slots get used over time
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0x14000 as *const u8;

    // Keyframe initializes all slots
    manager
        .update_frame(frame_ptr, 0, 0, FrameUpdateStrategy::Keyframe, false)
        .unwrap();

    // Process mix of frame types
    for i in 1..32 {
        let strategy = match i % 8 {
            0 => FrameUpdateStrategy::BFrame,
            _ => FrameUpdateStrategy::PFrame {
                refresh_golden: false,
            },
        };
        manager
            .update_frame(frame_ptr, i, i as u8, strategy, false)
            .unwrap();
    }

    // All 7 reference types should remain valid
    for i in 0..7 {
        let ref_type = ReferenceType::from_slot(i).unwrap();
        assert!(
            manager.get_reference(ref_type).is_some(),
            "Slot {} should be valid",
            i
        );
    }
}

#[test]
fn q26_golden_period_adaptation() {
    // Test different GOLDEN periods (16, 32, 64)
    let periods = [16, 32, 64];
    let frame_ptr = 0x15000 as *const u8;

    for &period in &periods {
        let config = PoolConfig::preset_1080p();
        let pool = FrameBufferPoolCapsule::new(&config).unwrap();
        let manager = ReferenceFrameManagerCapsule::new(&pool, period);

        // Process 128 frames
        for i in 0..128 {
            let refresh_golden = manager.should_refresh_golden(false);
            manager
                .update_frame(
                    frame_ptr,
                    i,
                    (i & 0xFF) as u8,
                    FrameUpdateStrategy::PFrame { refresh_golden },
                    false,
                )
                .unwrap();
        }

        let stats = manager.stats();
        let expected_refreshes = 128 / period as u64;
        assert!(
            stats.golden_refreshes >= expected_refreshes - 1,
            "Period {}: expected ~{} refreshes, got {}",
            period,
            expected_refreshes,
            stats.golden_refreshes
        );
    }
}

#[test]
fn q27_stats_accuracy() {
    // Verify statistics accuracy over large sequence
    let config = PoolConfig::preset_1080p();
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let frame_ptr = 0x16000 as *const u8;

    let mut expected_keyframes = 0u64;
    let mut expected_p_frames = 0u64;
    let mut expected_b_frames = 0u64;

    for i in 0..500 {
        let strategy = match i % 25 {
            0 => {
                expected_keyframes += 1;
                FrameUpdateStrategy::Keyframe
            }
            1 | 2 => {
                expected_b_frames += 1;
                FrameUpdateStrategy::BFrame
            }
            _ => {
                expected_p_frames += 1;
                FrameUpdateStrategy::PFrame {
                    refresh_golden: false,
                }
            }
        };
        manager
            .update_frame(frame_ptr, i, (i & 0xFF) as u8, strategy, false)
            .unwrap();
    }

    let stats = manager.stats();
    assert_eq!(stats.keyframes, expected_keyframes);
    assert_eq!(stats.p_frames, expected_p_frames);
    assert_eq!(stats.b_frames, expected_b_frames);
    assert_eq!(
        stats.total_frames,
        expected_keyframes + expected_p_frames + expected_b_frames
    );
}

#[test]
fn q28_concurrent_buffer_operations() {
    // Simulate concurrent buffer allocation/release
    let config = PoolConfig {
        max_buffers: 8,
        ..Default::default()
    };
    let pool = FrameBufferPoolCapsule::new(&config).unwrap();
    let manager = ReferenceFrameManagerCapsule::new(&pool, 32);

    let mut handles = Vec::new();

    for i in 0..100 {
        if i % 3 == 0 && !handles.is_empty() {
            // Release
            let handle = handles.pop().unwrap();
            manager.release_buffer(handle).unwrap();
        } else {
            // Try allocate
            if let Ok(handle) = manager.allocate_buffer() {
                handles.push(handle);
            }
        }
    }

    // Cleanup
    for handle in handles {
        manager.release_buffer(handle).unwrap();
    }

    assert_eq!(pool.available_count(), 8);
}
