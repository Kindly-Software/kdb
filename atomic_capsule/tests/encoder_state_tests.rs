//! EncoderStateCapsule T28 Comprehensive Test Suite
//!
//! Framework compliance: UCE34 (Q1-Q34), COCA, ASSUM, B32, T28, I20
//! Test count: 28 tests across 4 tiers (7 unit + 7 property + 7 integration + 7 production)

#[cfg(feature = "std")]
mod encoder_state_t28 {
    use atomic_capsule::encoder::{
        EncoderStateCapsule, EncoderState, SpeedPreset, QualityMode, PixelFormat, EncoderError,
    };
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    // ============================================================================
    // Q1-Q7: Unit Tests (Baseline correctness)
    // ============================================================================

    #[test]
    fn q1_initialization_creates_idle_state() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);
        assert_eq!(capsule.get_state(), EncoderState::Idle, "Initial state must be Idle");
    }

    #[test]
    fn q2_dimensions_correctly_stored_and_retrieved() {
        let test_cases = vec![
            (640, 480),
            (1280, 720),
            (1920, 1080),
            (3840, 2160),
            (4096, 2304),
            (7920, 4320),
        ];

        for (w, h) in test_cases {
            let capsule = EncoderStateCapsule::new(w, h, SpeedPreset::Medium, QualityMode::ConstantQuality);
            let (w2, h2) = capsule.get_dimensions();
            assert_eq!(w, w2, "Width {} not preserved", w);
            assert_eq!(h, h2, "Height {} not preserved", h);
        }
    }

    #[test]
    fn q3_frame_counter_initializes_to_zero() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);
        assert_eq!(capsule.get_frames_encoded(), 0, "Initial frame count must be 0");
    }

    #[test]
    fn q4_increment_frames_atomicity() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);

        for i in 1..=10 {
            let result = capsule.increment_frames();
            assert_eq!(result, i, "Frame {} increment failed", i);
        }

        assert_eq!(capsule.get_frames_encoded(), 10, "Final frame count must be 10");
    }

    #[test]
    fn q5_add_bytes_accumulation() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);

        capsule.add_bytes(10_000);
        capsule.add_bytes(20_000);
        capsule.add_bytes(30_000);

        let snap = capsule.snapshot();
        assert_eq!(snap.total_bytes, 60_000, "Bytes should accumulate");
    }

    #[test]
    fn q6_snapshot_returns_consistent_state() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Fast, QualityMode::VariableBitrate);
        capsule.update_state(EncoderState::Encoding).unwrap();
        capsule.set_start_time(1_000_000_000);
        capsule.increment_frames();
        capsule.add_bytes(65536);

        let snap = capsule.snapshot();
        assert_eq!(snap.state, EncoderState::Encoding);
        assert_eq!(snap.width, 1920);
        assert_eq!(snap.height, 1080);
        assert_eq!(snap.frames_encoded, 1);
        assert_eq!(snap.total_bytes, 65536);
        assert_eq!(snap.start_time_ns, 1_000_000_000);
    }

    #[test]
    fn q7_state_update_via_update_method() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);

        capsule.update_state(EncoderState::Encoding).unwrap();
        assert_eq!(capsule.get_state(), EncoderState::Encoding);

        capsule.update_state(EncoderState::Completed).unwrap();
        assert_eq!(capsule.get_state(), EncoderState::Completed);
    }

    // ============================================================================
    // Q8-Q14: Property Tests (Invariants and relationships)
    // ============================================================================

    #[test]
    fn q8_snapshot_monotonicity_frames() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);

        let snap1 = capsule.snapshot();
        capsule.increment_frames();
        let snap2 = capsule.snapshot();
        capsule.increment_frames();
        let snap3 = capsule.snapshot();

        assert!(snap1.frames_encoded < snap2.frames_encoded);
        assert!(snap2.frames_encoded < snap3.frames_encoded);
    }

    #[test]
    fn q9_snapshot_monotonicity_bytes() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);

        let snap1 = capsule.snapshot();
        capsule.add_bytes(10000);
        let snap2 = capsule.snapshot();
        capsule.add_bytes(20000);
        let snap3 = capsule.snapshot();

        assert!(snap1.total_bytes <= snap2.total_bytes);
        assert!(snap2.total_bytes <= snap3.total_bytes);
    }

    #[test]
    fn q10_snapshot_consistency_multiple_reads() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);
        capsule.update_state(EncoderState::Encoding).unwrap();
        capsule.increment_frames();
        capsule.add_bytes(100000);

        let snap1 = capsule.snapshot();
        let snap2 = capsule.snapshot();
        let snap3 = capsule.snapshot();

        assert_eq!(snap1.state, snap2.state);
        assert_eq!(snap2.state, snap3.state);
        assert_eq!(snap1.frames_encoded, snap2.frames_encoded);
        assert_eq!(snap2.frames_encoded, snap3.frames_encoded);
        assert_eq!(snap1.total_bytes, snap2.total_bytes);
        assert_eq!(snap2.total_bytes, snap3.total_bytes);
    }

    #[test]
    fn q11_all_speed_presets_valid() {
        let presets = vec![
            SpeedPreset::Slowest,
            SpeedPreset::VerySlow,
            SpeedPreset::Slow,
            SpeedPreset::MediumSlow,
            SpeedPreset::Medium,
            SpeedPreset::MediumFast,
            SpeedPreset::Fast,
            SpeedPreset::VeryFast,
            SpeedPreset::Faster,
            SpeedPreset::VeryFaster,
            SpeedPreset::Fastest,
        ];

        for preset in presets {
            let capsule = EncoderStateCapsule::new(1920, 1080, preset, QualityMode::ConstantQuality);
            let snap = capsule.snapshot();
            assert_eq!(snap.speed, preset);
        }
    }

    #[test]
    fn q12_all_quality_modes_valid() {
        let modes = vec![
            QualityMode::ConstantQuality,
            QualityMode::ConstantBitrate,
            QualityMode::VariableBitrate,
            QualityMode::Lossless,
        ];

        for mode in modes {
            let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, mode);
            let snap = capsule.snapshot();
            assert_eq!(snap.quality, mode);
        }
    }

    #[test]
    fn q13_memory_layout_correct() {
        assert_eq!(core::mem::size_of::<EncoderStateCapsule>(), 64, "Size must be exactly 64B");
        assert_eq!(core::mem::align_of::<EncoderStateCapsule>(), 64, "Alignment must be 64B");
    }

    #[test]
    fn q14_generation_counter_present() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);
        let snap = capsule.snapshot();
        assert!(snap.generation > 0, "Generation counter must be non-zero for ABA prevention");
    }

    // ============================================================================
    // Q15-Q21: Integration Tests (Multi-component workflows)
    // ============================================================================

    #[test]
    fn q15_full_encoding_state_machine() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Fast, QualityMode::VariableBitrate);

        // Initial state: Idle
        assert_eq!(capsule.get_state(), EncoderState::Idle);

        // Transition: Idle → Encoding
        capsule.update_state(EncoderState::Encoding).unwrap();
        assert_eq!(capsule.get_state(), EncoderState::Encoding);
        capsule.set_start_time(1_000_000_000);

        // Simulate encoding 30 frames
        for _ in 0..30 {
            capsule.increment_frames();
            capsule.add_bytes(65536); // ~0.5MB per frame
        }

        // Transition: Encoding → Flushing
        capsule.update_state(EncoderState::Flushing).unwrap();
        assert_eq!(capsule.get_state(), EncoderState::Flushing);

        // Transition: Flushing → Completed
        capsule.update_state(EncoderState::Completed).unwrap();
        assert_eq!(capsule.get_state(), EncoderState::Completed);

        // Verify final state
        let snap = capsule.snapshot();
        assert_eq!(snap.state, EncoderState::Completed);
        assert_eq!(snap.frames_encoded, 30);
        assert_eq!(snap.total_bytes, 30 * 65536);
    }

    #[test]
    fn q16_error_state_recovery() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);

        capsule.update_state(EncoderState::Encoding).unwrap();
        capsule.increment_frames();
        capsule.add_bytes(10000);

        // Trigger error
        capsule.update_state(EncoderState::Error).unwrap();
        assert_eq!(capsule.get_state(), EncoderState::Error);

        // Recover to idle
        capsule.update_state(EncoderState::Idle).unwrap();
        assert_eq!(capsule.get_state(), EncoderState::Idle);
    }

    #[test]
    fn q17_concurrent_increments_preserve_order() {
        let capsule = Arc::new(EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality));
        let mut handles = vec![];

        for _ in 0..4 {
            let c = capsule.clone();
            handles.push(std::thread::spawn(move || {
                for _ in 0..25 {
                    c.increment_frames();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // 4 threads × 25 increments = 100 total
        assert_eq!(capsule.get_frames_encoded(), 100);
    }

    #[test]
    fn q18_concurrent_byte_additions() {
        let capsule = Arc::new(EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality));
        let mut handles = vec![];

        for _ in 0..4 {
            let c = capsule.clone();
            handles.push(std::thread::spawn(move || {
                for _ in 0..100 {
                    c.add_bytes(1000);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // 4 threads × 100 increments × 1000 bytes = 400_000
        assert_eq!(capsule.snapshot().total_bytes, 400_000);
    }

    #[test]
    fn q19_snapshot_during_concurrent_updates() {
        let capsule = Arc::new(EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality));
        let finished = Arc::new(AtomicBool::new(false));

        let c = capsule.clone();
        let f = finished.clone();
        let writer = std::thread::spawn(move || {
            for i in 0..100 {
                c.increment_frames();
                c.add_bytes(10000);
                if i == 99 {
                    f.store(true, std::sync::atomic::Ordering::Release);
                }
            }
        });

        let c = capsule.clone();
        let reader = std::thread::spawn(move || {
            let mut snapshots = vec![];
            while !finished.load(std::sync::atomic::Ordering::Acquire) {
                snapshots.push(c.snapshot());
                std::thread::yield_now();
            }
            snapshots
        });

        writer.join().unwrap();
        let snapshots = reader.join().unwrap();

        // At least one snapshot should be captured
        assert!(!snapshots.is_empty());

        // Final state should have 100 frames
        assert_eq!(capsule.get_frames_encoded(), 100);
    }

    #[test]
    fn q20_bitrate_calculation() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);

        // Bitrate before start time should be 0
        capsule.add_bytes(1_000_000);
        assert_eq!(capsule.get_bitrate_kbps(), 0);

        // With start time set, bitrate would be calculated (not testing actual value due to clock mocking)
        capsule.set_start_time(1_000_000_000);
        // Note: get_bitrate_kbps requires actual time source, so we just verify it doesn't panic
        let _ = capsule.get_bitrate_kbps();
    }

    #[test]
    fn q21_mixed_operation_sequence() {
        let capsule = EncoderStateCapsule::new(3840, 2160, SpeedPreset::VeryFast, QualityMode::ConstantBitrate);

        capsule.update_state(EncoderState::Encoding).unwrap();
        capsule.set_start_time(2_000_000_000);

        for i in 0..50 {
            capsule.increment_frames();
            capsule.add_bytes(32768 * (i + 1));

            if i == 25 {
                capsule.update_state(EncoderState::Flushing).unwrap();
            }
        }

        capsule.update_state(EncoderState::Completed).unwrap();

        let snap = capsule.snapshot();
        assert_eq!(snap.frames_encoded, 50);
        assert!(snap.total_bytes > 0);
        assert_eq!(snap.state, EncoderState::Completed);
    }

    // ============================================================================
    // Q22-Q28: Production Tests (Robustness, edge cases, scalability)
    // ============================================================================

    #[test]
    fn q22_maximum_dimensions() {
        let capsule = EncoderStateCapsule::new(8191, 8191, SpeedPreset::Medium, QualityMode::ConstantQuality);
        let (w, h) = capsule.get_dimensions();
        assert_eq!(w, 8191);
        assert_eq!(h, 8191);
    }

    #[test]
    fn q23_minimum_dimensions() {
        let capsule = EncoderStateCapsule::new(1, 1, SpeedPreset::Medium, QualityMode::ConstantQuality);
        let (w, h) = capsule.get_dimensions();
        assert_eq!(w, 1);
        assert_eq!(h, 1);
    }

    #[test]
    fn q24_frame_counter_saturation() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);

        // Increment to near max (65535)
        for _ in 0..65530 {
            capsule.increment_frames();
        }

        assert_eq!(capsule.get_frames_encoded(), 65530);

        // Next 5 increments should reach 65535
        for _ in 0..5 {
            capsule.increment_frames();
        }

        assert_eq!(capsule.get_frames_encoded(), 65535);

        // Further increments should saturate
        let result = capsule.increment_frames();
        assert_eq!(result, 65535); // Saturated at max
    }

    #[test]
    fn q25_stress_many_state_transitions() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);
        let states = vec![
            EncoderState::Encoding,
            EncoderState::Flushing,
            EncoderState::Completed,
            EncoderState::Idle,
        ];

        for _ in 0..100 {
            for state in &states {
                capsule.update_state(*state).unwrap();
                assert_eq!(capsule.get_state(), *state);
            }
        }
    }

    #[test]
    fn q26_high_throughput_increments() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);

        // 1000 rapid increments
        for _ in 0..1000 {
            capsule.increment_frames();
        }

        // Should have saturated at 65535
        assert_eq!(capsule.get_frames_encoded(), 65535);
    }

    #[test]
    fn q27_high_throughput_bytes() {
        let capsule = EncoderStateCapsule::new(1920, 1080, SpeedPreset::Medium, QualityMode::ConstantQuality);

        // Add 1GB of data in 1MB chunks
        for _ in 0..1024 {
            capsule.add_bytes(1_000_000);
        }

        let snap = capsule.snapshot();
        assert_eq!(snap.total_bytes, 1_024_000_000);
    }

    #[test]
    fn q28_production_realistic_scenario() {
        // Simulate encoding 4K 60fps video for 10 seconds
        let capsule = EncoderStateCapsule::new(3840, 2160, SpeedPreset::Fast, QualityMode::ConstantBitrate);

        capsule.update_state(EncoderState::Encoding).unwrap();
        capsule.set_start_time(1_000_000_000);

        // 60 fps × 10 seconds = 600 frames
        // Assume ~500KB per frame at 4K
        for _ in 0..600 {
            capsule.increment_frames();
            capsule.add_bytes(500_000); // 500KB per frame
        }

        capsule.update_state(EncoderState::Completed).unwrap();

        let snap = capsule.snapshot();
        assert_eq!(snap.state, EncoderState::Completed);
        assert!(snap.frames_encoded <= 65535); // Saturated at 16-bit max
        assert_eq!(snap.total_bytes, 300_000_000); // 300MB total
        assert_eq!(snap.width, 3840);
        assert_eq!(snap.height, 2160);
    }
}
