//! B32 Performance Benchmarks for GopCoordinatorCapsule
//!
//! Validates <500ns frame type decision, <200ns scene detection, <5μs GOP planning

use atomic_capsule::encoder::{GopCoordinatorCapsule, GopFrameType as FrameType};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Benchmark frame type decision (target <500ns)
fn bench_frame_type_decision(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_type_decision");

    // Standard streaming GOP (60 frames, 2s @ 30fps)
    let gop_standard = GopCoordinatorCapsule::new(60, 3);

    group.bench_function("gop60_standard", |b| {
        b.iter(|| {
            for i in 0..16 {
                black_box(gop_standard.next_frame_type(black_box(i)));
            }
        });
    });

    // Low-latency GOP (30 frames, 1s @ 30fps)
    let gop_live = GopCoordinatorCapsule::new(30, 2);

    group.bench_function("gop30_live", |b| {
        b.iter(|| {
            for i in 0..16 {
                black_box(gop_live.next_frame_type(black_box(i)));
            }
        });
    });

    // Long-form GOP (120 frames, 4s @ 30fps)
    let gop_longform = GopCoordinatorCapsule::new(120, 7);

    group.bench_function("gop120_longform", |b| {
        b.iter(|| {
            for i in 0..16 {
                black_box(gop_longform.next_frame_type(black_box(i)));
            }
        });
    });

    group.finish();
}

/// Benchmark scene change detection (target <200ns)
fn bench_scene_change_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_change_detection");

    let gop = GopCoordinatorCapsule::with_scene_threshold(60, 3, 50);

    // Low motion (no scene change)
    group.bench_function("low_motion", |b| {
        b.iter(|| {
            black_box(gop.detect_scene_change(black_box(10), black_box(50)));
        });
    });

    // High motion (scene change detected)
    group.bench_function("high_motion", |b| {
        b.iter(|| {
            black_box(gop.detect_scene_change(black_box(100), black_box(50)));
        });
    });

    // Use default threshold
    group.bench_function("default_threshold", |b| {
        b.iter(|| {
            black_box(gop.detect_scene_change(black_box(60), black_box(0)));
        });
    });

    group.finish();
}

/// Benchmark temporal layer lookup (target <50ns)
fn bench_temporal_layer(c: &mut Criterion) {
    let mut group = c.benchmark_group("temporal_layer_lookup");

    let gop = GopCoordinatorCapsule::new(8, 3);

    group.bench_function("gop8_hierarchical", |b| {
        b.iter(|| {
            for i in 0..8 {
                black_box(gop.get_temporal_layer(black_box(i)));
            }
        });
    });

    group.finish();
}

/// Benchmark GOP planning (target <5μs for 16 frames)
fn bench_gop_planning(c: &mut Criterion) {
    let mut group = c.benchmark_group("gop_planning");

    let gop = GopCoordinatorCapsule::new(16, 7);

    for num_frames in [8, 16, 32, 64, 128].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_frames),
            num_frames,
            |b, &num_frames| {
                b.iter(|| {
                    black_box(gop.plan_gop(black_box(num_frames)));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark force keyframe (target <100ns)
fn bench_force_keyframe(c: &mut Criterion) {
    let mut group = c.benchmark_group("force_keyframe");

    let gop = GopCoordinatorCapsule::new(60, 3);

    group.bench_function("force_single", |b| {
        b.iter(|| {
            gop.force_keyframe();
        });
    });

    group.finish();
}

/// Benchmark GOP config get/set (target <100ns)
fn bench_config_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_operations");

    let gop = GopCoordinatorCapsule::new(60, 3);

    group.bench_function("get_config", |b| {
        b.iter(|| {
            black_box(gop.get_config());
        });
    });

    group.bench_function("set_gop_size", |b| {
        b.iter(|| {
            gop.set_gop_size(black_box(120));
        });
    });

    group.finish();
}

/// Benchmark concurrent access (target <500ns per thread)
fn bench_concurrent_access(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("concurrent_access");

    let gop = Arc::new(GopCoordinatorCapsule::new(60, 3));

    group.bench_function("4_threads_100_queries", |b| {
        b.iter(|| {
            let threads: Vec<_> = (0..4)
                .map(|thread_id| {
                    let gop_clone = Arc::clone(&gop);
                    thread::spawn(move || {
                        for i in 0..100 {
                            let frame_idx = thread_id * 100 + i;
                            black_box(gop_clone.next_frame_type(black_box(frame_idx)));
                        }
                    })
                })
                .collect();

            for handle in threads {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

/// Benchmark real-world scenarios
fn bench_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world_scenarios");

    // Netflix streaming (2s GOP @ 30fps)
    group.bench_function("netflix_streaming", |b| {
        let gop = GopCoordinatorCapsule::with_scene_threshold(60, 3, 50);
        b.iter(|| {
            for i in 0..300 {
                black_box(gop.next_frame_type(black_box(i)));
                if i % 10 == 0 {
                    // Simulate scene change detection every 10 frames
                    let sad = if i % 60 == 0 { 100 } else { 20 };
                    black_box(gop.detect_scene_change(black_box(sad), black_box(50)));
                }
            }
        });
    });

    // Low-latency live (1s GOP @ 30fps)
    group.bench_function("low_latency_live", |b| {
        let gop = GopCoordinatorCapsule::new(30, 2);
        b.iter(|| {
            for i in 0..150 {
                black_box(gop.next_frame_type(black_box(i)));
            }
        });
    });

    // Long-form VOD (4s GOP @ 30fps)
    group.bench_function("long_form_vod", |b| {
        let gop = GopCoordinatorCapsule::new(120, 7);
        b.iter(|| {
            for i in 0..600 {
                black_box(gop.next_frame_type(black_box(i)));
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_frame_type_decision,
    bench_scene_change_detection,
    bench_temporal_layer,
    bench_gop_planning,
    bench_force_keyframe,
    bench_config_operations,
    bench_concurrent_access,
    bench_scenarios,
);

criterion_main!(benches);
