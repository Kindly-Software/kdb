//! Integration tests for AnimationCapsule
//!
//! Tier: T1 (Atomic) + T3 (Fixed-Point)
//! Framework: T28 5-tier testing

#[cfg(feature = "terminal-style")]
mod animation_tests {
    use atomic_capsule::terminal::{AnimationCapsule, AnimationDirection, AnimationState, AnimatedProperties, EasingFunction, FillMode};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_ns() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    #[test]
    fn test_size_alignment() {
        assert_eq!(core::mem::size_of::<AnimationCapsule>(), 128);
        assert_eq!(core::mem::align_of::<AnimationCapsule>(), 64);
    }

    #[test]
    fn test_linear_easing() {
        let anim = AnimationCapsule::new();
        assert_eq!(anim.apply_easing(0), 0);
        assert_eq!(
            anim.apply_easing(AnimationCapsule::FIXED_HALF),
            AnimationCapsule::FIXED_HALF
        );
        assert_eq!(
            anim.apply_easing(AnimationCapsule::FIXED_ONE),
            AnimationCapsule::FIXED_ONE
        );
    }

    #[test]
    fn test_ease_in_cubic() {
        let t_half = AnimationCapsule::FIXED_HALF;
        let result = AnimationCapsule::ease_in_cubic(t_half);
        // 0.5^3 = 0.125 in Q16.16 = 8192
        assert!(result > 8000 && result < 8400); // Allow some rounding
    }

    #[test]
    fn test_ease_out_cubic() {
        let t_half = AnimationCapsule::FIXED_HALF;
        let result = AnimationCapsule::ease_out_cubic(t_half);
        // 1 - (1-0.5)^3 = 1 - 0.125 = 0.875 in Q16.16 = 57344
        assert!(result > 57000 && result < 57600);
    }

    #[test]
    fn test_animation_start_finish() {
        let anim = AnimationCapsule::new();

        // Start 100ms animation
        anim.start(0, 100, EasingFunction::Linear);
        assert!(anim.is_running());

        // At 50ms, should be halfway
        let progress = anim.tick(50_000_000);
        assert!(progress > 32000 && progress < 33000); // ~0.5 in Q16.16

        // At 100ms, should be complete
        let progress = anim.tick(100_000_000);
        assert_eq!(progress, AnimationCapsule::FIXED_ONE);
        assert!(anim.is_finished());
    }

    #[test]
    fn test_animation_with_delay() {
        let anim = AnimationCapsule::new();

        // Start with 50ms delay, 100ms duration
        anim.start_delayed(0, 50, 100, EasingFunction::Linear);
        assert!(anim.is_running());

        // At 25ms, still in delay
        let progress = anim.tick(25_000_000);
        assert_eq!(progress, 0);

        // At 100ms (50ms delay + 50ms into animation), should be halfway
        let progress = anim.tick(100_000_000);
        assert!(progress > 32000 && progress < 33000);
    }

    #[test]
    fn test_pause_resume() {
        let anim = AnimationCapsule::new();

        anim.start(0, 100, EasingFunction::Linear);

        // Pause at 50ms
        anim.pause(50_000_000);
        // State should be paused (can't check internal state from test)

        // Progress should stay at 50%
        let progress1 = anim.tick(75_000_000);
        let progress2 = anim.tick(100_000_000);
        assert_eq!(progress1, progress2);

        // Resume at 100ms (was paused for 50ms)
        anim.resume(100_000_000);
        assert!(anim.is_running());

        // At 150ms (100ms effective time), should be complete
        let progress = anim.tick(150_000_000);
        assert_eq!(progress, AnimationCapsule::FIXED_ONE);
    }

    #[test]
    fn test_iteration() {
        let anim = AnimationCapsule::new();
        anim.set_iterations(2);

        anim.start(0, 100, EasingFunction::Linear);

        // First iteration completes
        anim.tick(100_000_000);
        assert!(anim.is_running()); // Should start iteration 2

        // Second iteration completes
        anim.tick(200_000_000);
        assert!(anim.is_finished());
    }

    #[test]
    fn test_reverse_direction() {
        let anim = AnimationCapsule::new();
        anim.set_direction(AnimationDirection::Reverse);

        anim.start(0, 100, EasingFunction::Linear);

        // At 50ms, should be at 50% reversed = 50%
        let progress = anim.tick(50_000_000);
        assert!(progress > 32000 && progress < 33000);
    }

    #[test]
    fn test_property_mask() {
        let anim = AnimationCapsule::new();

        anim.set_properties(AnimatedProperties::OPACITY | AnimatedProperties::BG_COLOR);

        assert!(anim.animates(AnimatedProperties::OPACITY));
        assert!(anim.animates(AnimatedProperties::BG_COLOR));
        assert!(!anim.animates(AnimatedProperties::BORDER_RADIUS));
    }

    #[test]
    fn test_steps_easing() {
        let anim = AnimationCapsule::new();
        anim.set_steps(4);
        anim.start(0, 100, EasingFunction::Steps);

        // 4 steps: 0%, 25%, 50%, 75%, 100%
        let step_size = AnimationCapsule::FIXED_ONE / 4;

        assert_eq!(anim.apply_easing(0), 0);
        assert_eq!(anim.apply_easing(step_size / 2), 0); // Still in first step
        assert_eq!(anim.apply_easing(step_size), step_size);
        assert_eq!(anim.apply_easing(step_size * 2), step_size * 2);
    }

    #[test]
    fn test_fill_mode() {
        let anim = AnimationCapsule::new();
        anim.set_fill_mode(FillMode::Forwards);

        anim.start(0, 100, EasingFunction::Linear);
        anim.tick(100_000_000); // Complete

        // After finish, should retain 100% due to fill_mode=forwards
        let progress = anim.tick(200_000_000);
        assert_eq!(progress, AnimationCapsule::FIXED_ONE);
    }

    #[test]
    fn test_generation_counter() {
        let anim = AnimationCapsule::new();
        let gen1 = anim.generation();

        anim.start(0, 100, EasingFunction::Linear);
        let gen2 = anim.generation();
        assert_eq!(gen2, gen1 + 1);

        anim.pause(50_000_000);
        let gen3 = anim.generation();
        assert_eq!(gen3, gen2 + 1);
    }

    // === Q8-Q14: Property tests ===

    #[test]
    fn test_easing_boundaries() {
        let anim = AnimationCapsule::new();

        // All easing functions should map 0 -> 0 and 1 -> 1
        for easing in [
            EasingFunction::Linear,
            EasingFunction::EaseIn,
            EasingFunction::EaseOut,
            EasingFunction::EaseInOut,
            EasingFunction::EaseInQuad,
            EasingFunction::EaseOutQuad,
            EasingFunction::EaseInOutQuad,
            EasingFunction::EaseInCubic,
            EasingFunction::EaseOutCubic,
            EasingFunction::EaseInOutCubic,
        ] {
            let anim = AnimationCapsule::new();
            anim.start(0, 100, easing);

            let zero = anim.apply_easing(0);
            let one = anim.apply_easing(AnimationCapsule::FIXED_ONE);

            assert!(zero < 100, "Easing {:?} maps 0 to {}", easing, zero);
            assert!(
                one > AnimationCapsule::FIXED_ONE - 100,
                "Easing {:?} maps 1 to {}",
                easing,
                one
            );
        }
    }

    #[test]
    fn test_easing_monotonicity() {
        // Most easing functions should be monotonic (except elastic/bounce)
        for easing in [
            EasingFunction::Linear,
            EasingFunction::EaseIn,
            EasingFunction::EaseOut,
            EasingFunction::EaseInOut,
            EasingFunction::EaseInQuad,
            EasingFunction::EaseOutQuad,
            EasingFunction::EaseInOutQuad,
            EasingFunction::EaseInCubic,
            EasingFunction::EaseOutCubic,
            EasingFunction::EaseInOutCubic,
        ] {
            let anim = AnimationCapsule::new();
            anim.start(0, 100, easing);

            let mut prev = 0;
            for i in 0..=10 {
                let t = (i * AnimationCapsule::FIXED_ONE) / 10;
                let result = anim.apply_easing(t);
                assert!(
                    result >= prev,
                    "Easing {:?} not monotonic: {} < {} at t={}",
                    easing,
                    result,
                    prev,
                    t
                );
                prev = result;
            }
        }
    }

    #[test]
    fn test_concurrent_animations() {
        // Multiple animations should not interfere
        let anim1 = AnimationCapsule::new();
        let anim2 = AnimationCapsule::new();

        anim1.start(0, 100, EasingFunction::Linear);
        anim2.start(0, 200, EasingFunction::EaseIn);

        let progress1 = anim1.tick(50_000_000);
        let progress2 = anim2.tick(50_000_000);

        assert!(progress1 > 32000 && progress1 < 33000); // ~50%
        assert!(progress2 < 16384); // <25% due to ease-in
    }

    // === Q15-Q21: Integration tests ===

    #[test]
    fn test_60fps_animation_loop() {
        let anim = AnimationCapsule::new();
        let start = now_ns();

        anim.start(start, 1000, EasingFunction::EaseInOut); // 1 second animation

        let frame_duration_ns = 16_666_667; // 60 FPS = 16.67ms
        let mut frame_count = 0;

        loop {
            let now = start + (frame_count * frame_duration_ns);
            let progress = anim.tick(now);

            if anim.is_finished() {
                break;
            }

            frame_count += 1;

            // Should reach ~60 frames for 1 second animation
            assert!(frame_count < 100, "Animation took too many frames");
        }

        assert!(frame_count >= 55 && frame_count <= 65, "Expected ~60 frames, got {}", frame_count);
    }

    // === Q22-Q28: Production stress tests ===

    #[test]
    fn test_high_frequency_updates() {
        let anim = AnimationCapsule::new();
        let start = now_ns();

        anim.start(start, 100, EasingFunction::Linear);

        // Simulate 1000 FPS (1ms per frame) for 100ms animation
        for i in 0..=100 {
            let now = start + (i * 1_000_000);
            let _ = anim.tick(now);
        }

        assert!(anim.is_finished());
    }

    #[test]
    fn test_property_changes_during_animation() {
        let anim = AnimationCapsule::new();

        anim.start(0, 100, EasingFunction::Linear);
        anim.set_properties(AnimatedProperties::OPACITY);

        // Should be able to change properties mid-animation
        anim.tick(50_000_000);
        anim.set_properties(AnimatedProperties::BG_COLOR);

        assert!(anim.animates(AnimatedProperties::BG_COLOR));
        assert!(!anim.animates(AnimatedProperties::OPACITY));
    }
}
