//! T28 Comprehensive Tests: ThemeCapsule + GlassmorphismCapsule
//!
//! Test Tiers:
//! - Unit: Basic functionality (included in capsule files)
//! - Property: Invariants and linearizability
//! - Integration: Cross-capsule integration
//! - Production: Stress testing and concurrency

use kindly_web::capsules::{
    ThemeCapsule, GlassmorphismCapsule, ColorRGBA, ThemeMode, BlurLevel, GlassEffect,
    purple_spectrum, gold_spectrum, glass_tints,
};
use std::sync::Arc;
use std::thread;

// ============================================================================
// PROPERTY TESTS (Extended)
// ============================================================================

#[test]
fn property_theme_mode_toggle_exhaustive() {
    let theme = ThemeCapsule::new();

    // 1000 toggles should alternate perfectly
    for i in 0..1000 {
        let mode = theme.toggle_mode();
        let expected = if i % 2 == 0 { ThemeMode::Dark } else { ThemeMode::Light };
        assert_eq!(mode, expected, "Toggle {} failed", i);
    }
}

#[test]
fn property_theme_colors_immutable() {
    let theme = ThemeCapsule::new();

    // Read same color 1000 times, should always be identical
    let first = theme.get_purple(5).unwrap();
    for _ in 0..1000 {
        let current = theme.get_purple(5).unwrap();
        assert_eq!(current, first, "Color should be immutable");
    }
}

#[test]
fn property_glass_saturation_clamping() {
    let glass = GlassmorphismCapsule::new();

    // Test clamping to 0.0-2.0 range
    let test_values = vec![
        (-1.0, 0.0),
        (0.0, 0.0),
        (1.0, 1.0),
        (2.0, 2.0),
        (3.0, 2.0),
        (100.0, 2.0),
    ];

    for (input, expected) in test_values {
        glass.set_saturation(input);
        let actual = glass.get_saturation();
        assert!((actual - expected).abs() < 0.01, "Expected {}, got {} for input {}", expected, actual, input);
    }
}

#[cfg(feature = "portable_simd")]
#[test]
fn property_simd_scalar_equivalence_exhaustive() {
    let glass = GlassmorphismCapsule::new();

    // Test 100 random weight combinations
    for _ in 0..100 {
        let weights = [
            rand::random::<f32>(),
            rand::random::<f32>(),
            rand::random::<f32>(),
            rand::random::<f32>(),
        ];

        let simd_result = glass.calculate_blended_effect_simd(weights);

        // Calculate scalar baseline
        let blur_levels = glass.get_blur_levels();
        let opacity_layers = glass.get_opacity_layers();
        let mut scalar_blur = 0.0;
        let mut scalar_opacity = 0.0;
        for i in 0..4 {
            scalar_blur += blur_levels[i] * weights[i];
            scalar_opacity += opacity_layers[i] * weights[i];
        }

        assert!((simd_result.blur_px - scalar_blur).abs() < 0.001, "SIMD blur mismatch");
        assert!((simd_result.background_opacity() - scalar_opacity).abs() < 0.001, "SIMD opacity mismatch");
    }
}

// ============================================================================
// INTEGRATION TESTS
// ============================================================================

#[test]
fn integration_theme_glass_combined() {
    let theme = ThemeCapsule::new();
    let glass = GlassmorphismCapsule::new();

    // Get glass tint from theme
    let tint = theme.get_glass_tint(1).unwrap();

    // Get blur effect from glass
    let effect = glass.get_active_effect();

    // Combined effect should be usable
    assert!(effect.blur_px > 0.0);
    assert!(effect.opacity > 0.0);
    assert_ne!(tint.0, 0);
}

#[test]
fn integration_theme_mode_all_variants() {
    let theme = ThemeCapsule::new();

    // Test all mode variants
    theme.set_mode(ThemeMode::Light);
    assert_eq!(theme.get_mode(), ThemeMode::Light);

    theme.set_mode(ThemeMode::Dark);
    assert_eq!(theme.get_mode(), ThemeMode::Dark);

    theme.set_mode(ThemeMode::Auto);
    assert_eq!(theme.get_mode(), ThemeMode::Auto);

    // Toggle from auto should go to light
    let new_mode = theme.toggle_mode();
    assert_eq!(new_mode, ThemeMode::Light);
}

#[test]
fn integration_glass_all_presets() {
    let glass = GlassmorphismCapsule::new();

    // Test all blur presets
    let presets = vec![
        BlurLevel::Small,
        BlurLevel::Medium,
        BlurLevel::Large,
        BlurLevel::XLarge,
    ];

    for preset in presets {
        glass.set_blur_preset(preset);
        let levels = glass.get_blur_levels();

        // Verify levels are within expected range for preset
        match preset {
            BlurLevel::Small => assert!(levels[0] < 15.0),
            BlurLevel::Medium => assert!(levels[1] == 16.0),
            BlurLevel::Large => assert!(levels[2] >= 30.0),
            BlurLevel::XLarge => assert!(levels[3] >= 40.0),
        }
    }
}

// ============================================================================
// PRODUCTION TESTS (Stress & Concurrency)
// ============================================================================

#[test]
fn production_theme_concurrent_readers_1000() {
    let theme = Arc::new(ThemeCapsule::new());
    let num_readers = 1000;
    let reads_per_thread = 100;

    let handles: Vec<_> = (0..num_readers)
        .map(|_| {
            let theme_clone = Arc::clone(&theme);
            thread::spawn(move || {
                for _ in 0..reads_per_thread {
                    // Read different colors
                    let _ = theme_clone.get_purple(0);
                    let _ = theme_clone.get_purple(5);
                    let _ = theme_clone.get_purple(9);
                    let _ = theme_clone.get_gold(0);
                    let _ = theme_clone.get_gold(4);
                    let _ = theme_clone.get_mode();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // If we get here, no race conditions occurred
}

#[test]
fn production_theme_concurrent_mode_toggles() {
    let theme = Arc::new(ThemeCapsule::new());
    let num_togglers = 100;
    let toggles_per_thread = 10;

    let handles: Vec<_> = (0..num_togglers)
        .map(|_| {
            let theme_clone = Arc::clone(&theme);
            thread::spawn(move || {
                for _ in 0..toggles_per_thread {
                    theme_clone.toggle_mode();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Final mode should be deterministic based on total toggles
    let total_toggles = num_togglers * toggles_per_thread;
    let final_mode = theme.get_mode();
    let expected = if total_toggles % 2 == 0 { ThemeMode::Light } else { ThemeMode::Dark };
    assert_eq!(final_mode, expected, "Mode after {} toggles", total_toggles);
}

#[cfg(feature = "portable_simd")]
#[test]
fn production_glass_concurrent_blends_1000() {
    let glass = Arc::new(GlassmorphismCapsule::new());
    let num_threads = 100;
    let blends_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let glass_clone = Arc::clone(&glass);
            thread::spawn(move || {
                for _ in 0..blends_per_thread {
                    let weights = [0.25, 0.25, 0.25, 0.25];
                    let _ = glass_clone.calculate_blended_effect_simd(weights);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn production_glass_concurrent_read_write() {
    let glass = Arc::new(GlassmorphismCapsule::new());

    // 100 readers, 1 writer
    let num_readers = 100;
    let num_writes = 1000;

    // Writer thread
    let glass_writer = Arc::clone(&glass);
    let writer_handle = thread::spawn(move || {
        for i in 0..num_writes {
            let saturation = 1.0 + (i % 10) as f32 * 0.1;
            glass_writer.set_saturation(saturation);
        }
    });

    // Reader threads
    let reader_handles: Vec<_> = (0..num_readers)
        .map(|_| {
            let glass_reader = Arc::clone(&glass);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = glass_reader.get_saturation();
                    let _ = glass_reader.get_blur_levels();
                    let _ = glass_reader.get_opacity_layers();
                }
            })
        })
        .collect();

    writer_handle.join().unwrap();
    for handle in reader_handles {
        handle.join().unwrap();
    }
}

#[test]
fn production_memory_layout_verification() {
    // Verify alignment and size for both capsules
    assert_eq!(std::mem::align_of::<ThemeCapsule>(), 128);
    assert_eq!(std::mem::size_of::<ThemeCapsule>(), 128);

    assert_eq!(std::mem::align_of::<GlassmorphismCapsule>(), 128);
    assert_eq!(std::mem::size_of::<GlassmorphismCapsule>(), 128);

    // Verify no false sharing (128B = 2 cache lines on most platforms)
    // Both capsules fit in 2 cache lines (64B each) or 1 warm cache line (128B)
}

#[test]
fn production_color_constants_validation() {
    // Verify purple spectrum is non-zero and ascending
    let expected_purples = [
        purple_spectrum::PURPLE_50,
        purple_spectrum::PURPLE_100,
        purple_spectrum::PURPLE_200,
        purple_spectrum::PURPLE_300,
        purple_spectrum::PURPLE_400,
        purple_spectrum::PURPLE_500,
        purple_spectrum::PURPLE_600,
        purple_spectrum::PURPLE_700,
        purple_spectrum::PURPLE_800,
        purple_spectrum::PURPLE_900,
    ];

    for (i, color) in expected_purples.iter().enumerate() {
        assert_ne!(color.0, 0, "Purple {} should be non-zero", i);
    }

    // Verify gold spectrum
    let expected_golds = [
        gold_spectrum::GOLD_400,
        gold_spectrum::GOLD_500,
        gold_spectrum::GOLD_600,
        gold_spectrum::GOLD_700,
        gold_spectrum::GOLD_800,
    ];

    for (i, color) in expected_golds.iter().enumerate() {
        assert_ne!(color.0, 0, "Gold {} should be non-zero", i);
    }

    // Verify glass tints
    assert_ne!(glass_tints::LIGHT.0, 0);
    assert_ne!(glass_tints::MEDIUM.0, 0);
    assert_ne!(glass_tints::HEAVY.0, 0);
}

// ============================================================================
// STRESS TESTS (Edge Cases)
// ============================================================================

#[test]
fn stress_theme_all_colors_1000_times() {
    let theme = ThemeCapsule::new();

    for _ in 0..1000 {
        // Read all purple colors
        for i in 0..10 {
            let _ = theme.get_purple(i).unwrap();
        }

        // Read all gold colors
        for i in 0..5 {
            let _ = theme.get_gold(i).unwrap();
        }

        // Read all glass tints
        for i in 0..3 {
            let _ = theme.get_glass_tint(i).unwrap();
        }

        // Read mode
        let _ = theme.get_mode();
    }
}

#[test]
fn stress_glass_saturation_round_trip_1000() {
    let glass = GlassmorphismCapsule::new();

    for i in 0..1000 {
        let saturation = (i % 20) as f32 * 0.1; // 0.0-2.0 range
        glass.set_saturation(saturation);
        let retrieved = glass.get_saturation();
        assert!((retrieved - saturation.clamp(0.0, 2.0)).abs() < 0.01);
    }
}

#[test]
fn stress_glass_opacity_layers_extreme_values() {
    let glass = GlassmorphismCapsule::new();

    // Test extreme opacity values (should be clamped)
    let extreme_opacities = [
        [-1.0, 0.0, 0.5, 1.0],   // Negative
        [0.0, 0.0, 0.0, 0.0],    // All zeros
        [1.0, 1.0, 1.0, 1.0],    // All ones
        [100.0, 200.0, -50.0, 0.5], // Wild values
    ];

    for opacities in &extreme_opacities {
        glass.set_opacity_layers(*opacities);
        let retrieved = glass.get_opacity_layers();

        // All values should be clamped to 0.0-1.0
        for val in &retrieved {
            assert!(*val >= 0.0 && *val <= 1.0, "Opacity {} out of range", val);
        }
    }
}
