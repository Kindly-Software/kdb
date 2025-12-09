//! # CdefFilterCapsule Benchmarks - B32 Performance Validation
//!
//! **TRADE SECRET - Criterion.rs benchmarks for AV1 CDEF capsule**
//!
//! ## Benchmark Groups
//!
//! 1. **Direction Detection**: Variance-based search across 8 directions
//! 2. **Directional Filtering**: Single-direction SIMD filter
//! 3. **Full Block Filtering**: Complete CDEF pipeline (primary + secondary)
//! 4. **Concurrent Filtering**: Multi-threaded stress test
//!
//! ## Performance Targets (B32)
//!
//! - Direction detection: <300ns
//! - Directional filter: <400ns
//! - Full block filter: <1μs (CRITICAL TARGET)
//! - Throughput: 1M+ blocks/sec single-threaded

#![cfg(feature = "encoder-cdef")]

use atomic_capsule::encoder::CdefFilterCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use std::thread;

// Test patterns
fn create_flat_block() -> [u8; 64] {
    [128u8; 64]
}

fn create_horizontal_edge() -> [u8; 64] {
    let mut block = [0u8; 64];
    for y in 0..4 {
        for x in 0..8 {
            block[y * 8 + x] = 255;
        }
    }
    block
}

fn create_vertical_edge() -> [u8; 64] {
    let mut block = [0u8; 64];
    for y in 0..8 {
        for x in 0..4 {
            block[y * 8 + x] = 255;
        }
    }
    block
}

fn create_checkerboard() -> [u8; 64] {
    let mut block = [0u8; 64];
    for y in 0..8 {
        for x in 0..8 {
            block[y * 8 + x] = if (x + y) % 2 == 0 { 0 } else { 255 };
        }
    }
    block
}

fn create_noisy_block(seed: u32) -> [u8; 64] {
    let mut block = [128u8; 64];
    for i in 0..64 {
        let noise = ((seed * 17 + i * 37) % 21) as i32 - 10;
        block[i as usize] = ((128 + noise).clamp(0, 255)) as u8;
    }
    block
}

// ============================================================================
// Benchmark 1: Direction Detection
// ============================================================================

fn bench_direction_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("cdef_direction_detection");

    let patterns = [
        ("flat", create_flat_block()),
        ("horizontal_edge", create_horizontal_edge()),
        ("vertical_edge", create_vertical_edge()),
        ("checkerboard", create_checkerboard()),
        ("noisy", create_noisy_block(42)),
    ];

    for (name, block) in &patterns {
        group.bench_with_input(BenchmarkId::from_parameter(name), block, |b, block| {
            let capsule = CdefFilterCapsule::new();
            b.iter(|| {
                let dir = capsule.find_direction(black_box(block));
                black_box(dir);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark 2: Directional Filtering (Single Direction)
// ============================================================================

fn bench_directional_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("cdef_directional_filter");

    let capsule = CdefFilterCapsule::new();
    let y_pri = [4u8, 4, 4, 4];
    let y_sec = [2u8, 2, 2, 2];
    let uv_pri = [4u8, 4, 4, 4];
    let uv_sec = [2u8, 2, 2, 2];
    capsule.set_strengths(&y_pri, &y_sec, &uv_pri, &uv_sec);

    let patterns = [
        ("flat", create_flat_block()),
        ("horizontal_edge", create_horizontal_edge()),
        ("vertical_edge", create_vertical_edge()),
        ("checkerboard", create_checkerboard()),
        ("noisy", create_noisy_block(42)),
    ];

    for (name, block) in &patterns {
        group.bench_with_input(BenchmarkId::from_parameter(name), block, |b, block| {
            b.iter(|| {
                let mut output = *block;
                capsule.apply_filter(black_box(&mut output), true, 0);
                black_box(output);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark 3: Full Block Filtering (PRIMARY + SECONDARY)
// ============================================================================

fn bench_full_block_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("cdef_full_block_filter");

    let strengths = [
        ("weak", 1u8, 0u8),
        ("medium", 4u8, 2u8),
        ("strong", 8u8, 4u8),
        ("max", 15u8, 15u8),
    ];

    let patterns = [
        ("flat", create_flat_block()),
        ("horizontal_edge", create_horizontal_edge()),
        ("vertical_edge", create_vertical_edge()),
        ("checkerboard", create_checkerboard()),
        ("noisy", create_noisy_block(42)),
    ];

    for (strength_name, primary, secondary) in &strengths {
        for (pattern_name, block) in &patterns {
            let id = format!("{}_{}", strength_name, pattern_name);
            group.bench_with_input(BenchmarkId::from_parameter(&id), block, |b, block| {
                let capsule = CdefFilterCapsule::new();
                let y_pri = [*primary; 4];
                let y_sec = [*secondary; 4];
                let uv_pri = [*primary; 4];
                let uv_sec = [*secondary; 4];
                capsule.set_strengths(&y_pri, &y_sec, &uv_pri, &uv_sec);

                b.iter(|| {
                    let mut output = *block;
                    capsule.apply_filter(black_box(&mut output), true, 0);
                    black_box(output);
                });
            });
        }
    }

    group.finish();
}

// ============================================================================
// Benchmark 4: Throughput (Blocks/Second)
// ============================================================================

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("cdef_throughput");
    group.sample_size(100);

    group.bench_function("throughput_1000_blocks", |b| {
        let capsule = CdefFilterCapsule::new();
        let y_pri = [4u8; 4];
        let y_sec = [2u8; 4];
        let uv_pri = [4u8; 4];
        let uv_sec = [2u8; 4];
        capsule.set_strengths(&y_pri, &y_sec, &uv_pri, &uv_sec);

        let blocks: Vec<[u8; 64]> = (0..1000).map(|i| create_noisy_block(i)).collect();

        b.iter(|| {
            for block in &blocks {
                let mut output = *block;
                capsule.apply_filter(black_box(&mut output), true, 0);
                black_box(output);
            }
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 5: Concurrent Filtering
// ============================================================================

fn bench_concurrent_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("cdef_concurrent");
    group.sample_size(50);

    for num_threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads", num_threads)),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let capsule = Arc::new(CdefFilterCapsule::new());
                    let y_pri = [4u8; 4];
                    let y_sec = [2u8; 4];
                    let uv_pri = [4u8; 4];
                    let uv_sec = [2u8; 4];
                    capsule.set_strengths(&y_pri, &y_sec, &uv_pri, &uv_sec);

                    let mut handles = vec![];

                    for thread_id in 0..num_threads {
                        let capsule_clone = Arc::clone(&capsule);
                        let handle = thread::spawn(move || {
                            let blocks: Vec<[u8; 64]> = (0..100)
                                .map(|i| create_noisy_block(thread_id * 100 + i))
                                .collect();

                            for block in &blocks {
                                let mut output = *block;
                                capsule_clone.apply_filter(&mut output, true, 0);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark 6: Update Strengths Performance
// ============================================================================

fn bench_update_strengths(c: &mut Criterion) {
    let mut group = c.benchmark_group("cdef_update_strengths");

    group.bench_function("update_strengths", |b| {
        let capsule = CdefFilterCapsule::new();
        let strengths: Vec<([u8; 4], [u8; 4])> = (0..100)
            .map(|i| {
                let pri = ((i % 16) as u8);
                let sec = (((i * 3) % 16) as u8);
                ([pri; 4], [sec; 4])
            })
            .collect();

        b.iter(|| {
            for (primary, secondary) in &strengths {
                capsule.set_strengths(black_box(primary), black_box(secondary), primary, secondary);
            }
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 7: Variance Computation
// ============================================================================

fn bench_variance_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cdef_variance");

    let patterns = [
        ("flat", create_flat_block()),
        ("horizontal_edge", create_horizontal_edge()),
        ("vertical_edge", create_vertical_edge()),
        ("checkerboard", create_checkerboard()),
        ("noisy", create_noisy_block(42)),
    ];

    for (name, block) in &patterns {
        group.bench_with_input(BenchmarkId::from_parameter(name), block, |b, block| {
            let capsule = CdefFilterCapsule::new();
            b.iter(|| {
                // Compute direction (includes variance calculation)
                let dir = capsule.find_direction(black_box(block));
                black_box(dir);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark 8: End-to-End Latency
// ============================================================================

fn bench_end_to_end_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cdef_end_to_end_latency");
    group.sample_size(1000);

    let block = create_noisy_block(42);

    group.bench_function("p50_latency", |b| {
        let capsule = CdefFilterCapsule::new();
        let y_pri = [4u8; 4];
        let y_sec = [2u8; 4];
        let uv_pri = [4u8; 4];
        let uv_sec = [2u8; 4];
        capsule.set_strengths(&y_pri, &y_sec, &uv_pri, &uv_sec);

        b.iter(|| {
            let mut output = block;
            capsule.apply_filter(black_box(&mut output), true, 0);
            black_box(output);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_direction_detection,
    bench_directional_filter,
    bench_full_block_filter,
    bench_throughput,
    bench_concurrent_filtering,
    bench_update_strengths,
    bench_variance_computation,
    bench_end_to_end_latency
);

criterion_main!(benches);
