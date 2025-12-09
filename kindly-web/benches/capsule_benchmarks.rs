//! B32 Benchmarks: ThemeCapsule + GlassmorphismCapsule
//!
//! Honest benchmarking with fair baselines:
//! - ThemeCapsule vs Mutex<HashMap<String, Color>>
//! - GlassmorphismCapsule SIMD vs scalar implementation
//! - 1000+ iterations, 95% confidence intervals
//!
//! Performance Reality Check:
//! - 10-50% typical improvements
//! - 2× exceptional (proven in circuit breaker)
//! - 10×+ requires extensive validation

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

// Import capsules
use kindly_web::capsules::{ThemeCapsule, GlassmorphismCapsule, ColorRGBA, ThemeMode, BlurLevel};

// ============================================================================
// BASELINE IMPLEMENTATIONS (Fair Comparison)
// ============================================================================

/// Baseline: Mutex<HashMap> for theme colors
struct MutexTheme {
    colors: Mutex<HashMap<String, u32>>,
}

impl MutexTheme {
    fn new() -> Self {
        let mut colors = HashMap::new();
        // Purple spectrum
        for i in 0..10 {
            colors.insert(format!("purple_{}", i), 0xFF000000 + (i as u32 * 0x1000));
        }
        // Gold spectrum
        for i in 0..5 {
            colors.insert(format!("gold_{}", i), 0xFFFFAA00 + (i as u32 * 0x100));
        }
        Self {
            colors: Mutex::new(colors),
        }
    }

    fn get_color(&self, key: &str) -> Option<u32> {
        let colors = self.colors.lock().unwrap();
        colors.get(key).copied()
    }
}

/// Baseline: Scalar glass effect calculation
struct ScalarGlass {
    blur_levels: [f32; 4],
    opacity_layers: [f32; 4],
    saturation: f32,
}

impl ScalarGlass {
    fn new() -> Self {
        Self {
            blur_levels: [8.0, 16.0, 24.0, 32.0],
            opacity_layers: [0.1, 0.15, 0.2, 0.25],
            saturation: 1.0,
        }
    }

    fn calculate_blended_scalar(&self, weights: [f32; 4]) -> (f32, f32) {
        let mut blur_sum = 0.0;
        let mut opacity_sum = 0.0;

        for i in 0..4 {
            blur_sum += self.blur_levels[i] * weights[i];
            opacity_sum += self.opacity_layers[i] * weights[i];
        }

        (blur_sum, opacity_sum)
    }
}

// ============================================================================
// THEME CAPSULE BENCHMARKS
// ============================================================================

fn bench_theme_color_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("theme_color_lookup");

    // ThemeCapsule (atomic)
    let theme_capsule = Arc::new(ThemeCapsule::new());
    group.bench_function("capsule_purple_0", |b| {
        b.iter(|| {
            black_box(theme_capsule.get_purple(black_box(0)))
        })
    });

    // Mutex<HashMap> baseline
    let mutex_theme = Arc::new(MutexTheme::new());
    group.bench_function("mutex_hashmap_purple_0", |b| {
        b.iter(|| {
            black_box(mutex_theme.get_color(black_box("purple_0")))
        })
    });

    group.finish();
}

fn bench_theme_spectrum_retrieval(c: &mut Criterion) {
    let mut group = c.benchmark_group("theme_spectrum_retrieval");

    let theme_capsule = Arc::new(ThemeCapsule::new());

    // Full purple spectrum (10 colors)
    group.bench_function("capsule_purple_spectrum", |b| {
        b.iter(|| {
            black_box(theme_capsule.get_purple_spectrum())
        })
    });

    // Full gold spectrum (5 colors)
    group.bench_function("capsule_gold_spectrum", |b| {
        b.iter(|| {
            black_box(theme_capsule.get_gold_spectrum())
        })
    });

    group.finish();
}

fn bench_theme_mode_toggle(c: &mut Criterion) {
    let mut group = c.benchmark_group("theme_mode_toggle");

    let theme_capsule = Arc::new(ThemeCapsule::new());

    // Mode toggle (CAS operation)
    group.bench_function("capsule_toggle_mode", |b| {
        b.iter(|| {
            black_box(theme_capsule.toggle_mode())
        })
    });

    // Mode set (simple store)
    group.bench_function("capsule_set_mode_dark", |b| {
        b.iter(|| {
            theme_capsule.set_mode(black_box(ThemeMode::Dark))
        })
    });

    group.finish();
}

// ============================================================================
// GLASSMORPHISM CAPSULE BENCHMARKS
// ============================================================================

fn bench_glass_blur_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("glass_blur_levels");

    let glass_capsule = Arc::new(GlassmorphismCapsule::new());

    // Single blur level read
    group.bench_function("capsule_get_blur_level_0", |b| {
        b.iter(|| {
            black_box(glass_capsule.get_blur_level(black_box(0)))
        })
    });

    // All blur levels (array)
    group.bench_function("capsule_get_blur_levels_array", |b| {
        b.iter(|| {
            black_box(glass_capsule.get_blur_levels())
        })
    });

    // All blur levels (SIMD) - requires portable_simd feature
    #[cfg(feature = "portable_simd")]
    group.bench_function("capsule_get_blur_levels_simd", |b| {
        b.iter(|| {
            black_box(glass_capsule.get_blur_levels_simd())
        })
    });

    group.finish();
}

fn bench_glass_blended_effect(c: &mut Criterion) {
    let mut group = c.benchmark_group("glass_blended_effect");

    let glass_capsule = Arc::new(GlassmorphismCapsule::new());
    let scalar_glass = ScalarGlass::new();

    let weights = [0.25, 0.25, 0.25, 0.25];

    // Scalar baseline
    group.bench_function("scalar_4layer_blend", |b| {
        b.iter(|| {
            black_box(scalar_glass.calculate_blended_scalar(black_box(weights)))
        })
    });

    // SIMD capsule (BREAKTHROUGH operation)
    #[cfg(feature = "portable_simd")]
    group.bench_function("capsule_simd_4layer_blend", |b| {
        b.iter(|| {
            black_box(glass_capsule.calculate_blended_effect_simd(black_box(weights)))
        })
    });

    group.finish();
}

fn bench_glass_saturation(c: &mut Criterion) {
    let mut group = c.benchmark_group("glass_saturation");

    let glass_capsule = Arc::new(GlassmorphismCapsule::new());

    // Get saturation (Q16.16 fixed-point read)
    group.bench_function("capsule_get_saturation", |b| {
        b.iter(|| {
            black_box(glass_capsule.get_saturation())
        })
    });

    // Set saturation (Q16.16 fixed-point write)
    group.bench_function("capsule_set_saturation", |b| {
        b.iter(|| {
            glass_capsule.set_saturation(black_box(1.5))
        })
    });

    group.finish();
}

fn bench_glass_active_effect(c: &mut Criterion) {
    let mut group = c.benchmark_group("glass_active_effect");

    let glass_capsule = Arc::new(GlassmorphismCapsule::new());

    // Get active effect (combined blur + opacity + saturation)
    group.bench_function("capsule_get_active_effect", |b| {
        b.iter(|| {
            black_box(glass_capsule.get_active_effect())
        })
    });

    group.finish();
}

// ============================================================================
// SCALING BENCHMARKS
// ============================================================================

fn bench_theme_concurrent_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("theme_concurrent_reads");

    let theme_capsule = Arc::new(ThemeCapsule::new());

    // Simulate concurrent readers (1, 10, 100, 1000)
    for num_readers in [1, 10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("capsule_purple_0", num_readers),
            num_readers,
            |b, &n| {
                let handles: Vec<_> = (0..n)
                    .map(|_| {
                        let theme = Arc::clone(&theme_capsule);
                        std::thread::spawn(move || {
                            for _ in 0..100 {
                                black_box(theme.get_purple(black_box(0)));
                            }
                        })
                    })
                    .collect();

                b.iter(|| {
                    // Measure time to join all threads
                    for handle in handles.into_iter() {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_glass_preset_switching(c: &mut Criterion) {
    let mut group = c.benchmark_group("glass_preset_switching");

    let glass_capsule = Arc::new(GlassmorphismCapsule::new());

    // Preset switching (simulates UI preset changes)
    group.bench_function("capsule_set_blur_preset_medium", |b| {
        b.iter(|| {
            glass_capsule.set_blur_preset(black_box(BlurLevel::Medium))
        })
    });

    group.bench_function("capsule_set_blur_preset_xlarge", |b| {
        b.iter(|| {
            glass_capsule.set_blur_preset(black_box(BlurLevel::XLarge))
        })
    });

    group.finish();
}

// ============================================================================
// BENCHMARK GROUPS
// ============================================================================

criterion_group!(
    theme_benches,
    bench_theme_color_lookup,
    bench_theme_spectrum_retrieval,
    bench_theme_mode_toggle,
    bench_theme_concurrent_reads,
);

criterion_group!(
    glass_benches,
    bench_glass_blur_levels,
    bench_glass_blended_effect,
    bench_glass_saturation,
    bench_glass_active_effect,
    bench_glass_preset_switching,
);

criterion_main!(theme_benches, glass_benches);
