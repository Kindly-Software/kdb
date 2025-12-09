//! [TRADE SECRET] LoopFilterCapsule Benchmarks - B32 Framework Compliance
//!
//! Fair baseline comparison: rav1e loop filter (conservative 2-5× speedup target)
//!
//! # Benchmark Groups
//!
//! 1. **filter_4x4_block**: 4×4 block edge filtering (most common)
//! 2. **filter_8x8_block**: 8×8 block edge filtering
//! 3. **filter_16x16_block**: 16×16 block edge filtering
//! 4. **full_frame_1024x1024**: Complete frame deblocking
//! 5. **concurrent_filtering**: Multi-threaded stress test
//!
//! # Performance Targets (B32 Conservative)
//!
//! - **4×4 block**: <500ns (vs rav1e ~1μs baseline, 2× speedup)
//! - **1024×1024 frame**: <50ms (vs rav1e ~100ms baseline, 2× speedup)
//! - **Concurrent**: Linear scaling up to 16 threads

#![cfg(feature = "portable_simd")]

use atomic_capsule::encoder::LoopFilterCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use std::thread;

fn bench_filter_4x4_vertical(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_4x4_block");

    for level in [16, 32, 48, 63].iter() {
        group.bench_with_input(BenchmarkId::new("vertical", level), level, |b, &level| {
            let filter = LoopFilterCapsule::new(level, 3);
            let mut pixels = vec![128u8; 16]; // 4×4 block

            b.iter(|| {
                filter.filter_edge_vertical(black_box(&mut pixels), black_box(4));
            });
        });
    }

    group.finish();
}

fn bench_filter_4x4_horizontal(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_4x4_block");

    for level in [16, 32, 48, 63].iter() {
        group.bench_with_input(BenchmarkId::new("horizontal", level), level, |b, &level| {
            let filter = LoopFilterCapsule::new(level, 3);
            let mut pixels = vec![128u8; 16]; // 4×4 block

            b.iter(|| {
                filter.filter_edge_horizontal(black_box(&mut pixels), black_box(4));
            });
        });
    }

    group.finish();
}

fn bench_filter_8x8(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_8x8_block");

    group.bench_function("vertical", |b| {
        let filter = LoopFilterCapsule::new(32, 3);
        let mut pixels = vec![128u8; 64]; // 8×8 block

        b.iter(|| {
            filter.filter_edge_vertical(black_box(&mut pixels), black_box(8));
        });
    });

    group.bench_function("horizontal", |b| {
        let filter = LoopFilterCapsule::new(32, 3);
        let mut pixels = vec![128u8; 64];

        b.iter(|| {
            filter.filter_edge_horizontal(black_box(&mut pixels), black_box(8));
        });
    });

    group.finish();
}

fn bench_filter_16x16(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_16x16_block");

    group.bench_function("vertical", |b| {
        let filter = LoopFilterCapsule::new(32, 3);
        let mut pixels = vec![128u8; 256]; // 16×16 block

        b.iter(|| {
            filter.filter_edge_vertical(black_box(&mut pixels), black_box(16));
        });
    });

    group.bench_function("horizontal", |b| {
        let filter = LoopFilterCapsule::new(32, 3);
        let mut pixels = vec![128u8; 256];

        b.iter(|| {
            filter.filter_edge_horizontal(black_box(&mut pixels), black_box(16));
        });
    });

    group.finish();
}

fn bench_full_frame_1024x1024(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_frame_1024x1024");
    group.sample_size(10); // Fewer samples for expensive benchmark

    group.bench_function("complete_deblocking", |b| {
        let filter = LoopFilterCapsule::new(32, 3);
        let width = 1024;
        let height = 1024;
        let mut pixels = vec![128u8; width * height];

        b.iter(|| {
            // Filter all 4×4 block edges (vertical + horizontal)
            // Vertical edges
            for y in 0..height {
                for x in (0..width).step_by(4) {
                    let offset = y * width + x;
                    if offset + 16 <= pixels.len() {
                        filter.filter_edge_vertical(
                            black_box(&mut pixels[offset..]),
                            black_box(width),
                        );
                    }
                }
            }

            // Horizontal edges
            for y in (0..height).step_by(4) {
                let offset = y * width;
                if offset + width * 4 <= pixels.len() {
                    filter
                        .filter_edge_horizontal(black_box(&mut pixels[offset..]), black_box(width));
                }
            }
        });
    });

    group.finish();
}

fn bench_compute_filter_strength(c: &mut Criterion) {
    let mut group = c.benchmark_group("compute_filter_strength");

    group.bench_function("typical_q_diff", |b| {
        let filter = LoopFilterCapsule::new(32, 3);

        b.iter(|| filter.compute_filter_strength(black_box(16), black_box(32)));
    });

    group.bench_function("large_q_diff", |b| {
        let filter = LoopFilterCapsule::new(63, 7);

        b.iter(|| filter.compute_filter_strength(black_box(127), black_box(63)));
    });

    group.finish();
}

fn bench_concurrent_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_filtering");

    for num_threads in [1, 2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("threads", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let filter = Arc::new(LoopFilterCapsule::new(32, 3));
                    let mut handles = vec![];

                    for _ in 0..num_threads {
                        let filter_clone = Arc::clone(&filter);
                        let handle = thread::spawn(move || {
                            let mut pixels = vec![128u8; 256]; // 16×16 block
                            filter_clone.filter_edge_vertical(&mut pixels, 16);
                            filter_clone.filter_edge_horizontal(&mut pixels, 16);
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        let _ = handle.join();
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_filter_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_levels");

    for level in [0, 16, 32, 48, 63].iter() {
        group.bench_with_input(BenchmarkId::new("level", level), level, |b, &level| {
            let filter = LoopFilterCapsule::new(level, 3);
            let mut pixels = vec![128u8; 64]; // 8×8 block

            b.iter(|| {
                filter.filter_edge_vertical(black_box(&mut pixels), black_box(8));
            });
        });
    }

    group.finish();
}

fn bench_sharpness_values(c: &mut Criterion) {
    let mut group = c.benchmark_group("sharpness_values");

    for sharpness in [0, 2, 4, 6, 7].iter() {
        group.bench_with_input(
            BenchmarkId::new("sharpness", sharpness),
            sharpness,
            |b, &sharpness| {
                let filter = LoopFilterCapsule::new(32, sharpness);
                let mut pixels = vec![128u8; 64];

                b.iter(|| {
                    filter.filter_edge_horizontal(black_box(&mut pixels), black_box(8));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_filter_4x4_vertical,
    bench_filter_4x4_horizontal,
    bench_filter_8x8,
    bench_filter_16x16,
    bench_full_frame_1024x1024,
    bench_compute_filter_strength,
    bench_concurrent_filtering,
    bench_filter_levels,
    bench_sharpness_values
);

criterion_main!(benches);
