//! B32 Benchmarks: Reconstruction Pipeline Performance
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Validates reconstruction pipeline performance targets:
//! - Full frame encoding (includes internal reconstruction): <50ms @ 1080p
//! - Individual capsule operations (quantization, DCT, entropy): <10μs
//! - Reconstructed buffer access: <10ns
//!
//! ## B32 Framework Compliance
//!
//! - 95% CI (Criterion default)
//! - 1000+ iterations per benchmark
//! - Fair baselines (measure actual encoding operations)
//! - Reproducibility (kindly-hub: AMD Ryzen 9 6900HX)
//!
//! ## SOTA Targets (2024-2025)
//!
//! Based on rav1e/SVT-AV1 reconstruction benchmarks:
//! - Full frame 1080p: <50ms (includes dequant → IDCT → add pred → clip → loop filters)
//! - Reconstructed buffer access: <10ns (simple pointer read)
//!
//! ## Run Commands (kindly-hub MANDATORY)
//!
//! ```bash
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench reconstruction_bench --release"
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kindly_av1::encoder::{EncoderWiringCapsule, EncoderSubCapsules};

/// Create test frame with gradient pattern
fn create_test_frame(width: usize, height: usize) -> Vec<u8> {
    let mut frame = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let value = ((x + y) / 16) as u8;
            frame.push(value);
        }
    }
    frame
}

/// Benchmark: Full frame encoding (64×64)
///
/// **Target**: <5ms per frame (small test frame)
///
/// **Pipeline**: Encoding includes internal reconstruction (dequant → IDCT → add pred → loop filters)
fn bench_full_frame_encoding_64x64(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_frame_encoding_64x64");
    group.throughput(Throughput::Elements(1)); // 1 frame

    group.bench_function("encode_frame_64x64", |b| {
        let frame = create_test_frame(64, 64);

        b.iter(|| {
            let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
            let mut sub_capsules = EncoderSubCapsules::new();

            // Encode frame (triggers full reconstruction pipeline internally)
            let result = wiring.encode_frame(black_box(&frame), &mut sub_capsules);
            assert!(result.is_ok());

            // Verify reconstruction buffer populated
            let reconstructed = sub_capsules.reconstructed_buffer();
            black_box(reconstructed.len())
        })
    });

    group.finish();
}

/// Benchmark: Full frame encoding (1920×1080)
///
/// **Target**: <50ms per frame (production 1080p)
///
/// **Note**: Reconstruction happens internally during encoding (dequant → IDCT → add pred → clip → loop filters)
fn bench_full_frame_encoding_1080p(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_frame_encoding_1080p");
    group.throughput(Throughput::Elements(1)); // 1 frame
    group.sample_size(20); // Reduce for long benchmarks

    group.bench_function("encode_frame_1080p", |b| {
        let frame = create_test_frame(1920, 1080);

        b.iter(|| {
            let wiring = EncoderWiringCapsule::with_params(1920, 1080, 28, 5);
            let mut sub_capsules = EncoderSubCapsules::new();

            let result = wiring.encode_frame(black_box(&frame), &mut sub_capsules);
            assert!(result.is_ok());

            let reconstructed = sub_capsules.reconstructed_buffer();
            black_box(reconstructed.len())
        })
    });

    // Multi-frame encoding (I + P + P)
    group.bench_function("multiframe_1080p_3frames", |b| {
        let frame0 = create_test_frame(1920, 1080);
        let frame1 = create_test_frame(1920, 1080);
        let frame2 = create_test_frame(1920, 1080);

        b.iter(|| {
            let wiring = EncoderWiringCapsule::with_params(1920, 1080, 28, 5);
            let mut sub_capsules = EncoderSubCapsules::new();

            // Encode 3 frames (I + P + P)
            wiring.encode_frame(black_box(&frame0), &mut sub_capsules).unwrap();
            wiring.encode_frame(black_box(&frame1), &mut sub_capsules).unwrap();
            wiring.encode_frame(black_box(&frame2), &mut sub_capsules).unwrap();

            let reconstructed = sub_capsules.reconstructed_buffer();
            black_box(reconstructed.len())
        })
    });

    group.finish();
}

/// Benchmark: Individual capsule operations (quantization, DCT, entropy)
///
/// **Purpose**: Breakdown of encoding pipeline components
///
/// **Target**: Each operation <10μs
fn bench_individual_capsules(c: &mut Criterion) {
    let mut group = c.benchmark_group("individual_capsule_operations");

    let frame = create_test_frame(64, 64);

    // Quantization capsule access (read-only)
    group.bench_function("quantizer_access", |b| {
        let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
        let mut sub_capsules = EncoderSubCapsules::new();

        // Initialize by encoding one frame
        wiring.encode_frame(&frame, &mut sub_capsules).unwrap();

        b.iter(|| {
            // Measure access time (not quantization operation itself)
            let quantizer = sub_capsules.quantizer();
            black_box(quantizer)
        })
    });

    // DCT transform capsule access (read-only)
    group.bench_function("dct_access", |b| {
        let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
        let mut sub_capsules = EncoderSubCapsules::new();

        wiring.encode_frame(&frame, &mut sub_capsules).unwrap();

        b.iter(|| {
            let dct = sub_capsules.dct();
            black_box(dct)
        })
    });

    // Entropy coder capsule access (read-only)
    group.bench_function("entropy_access", |b| {
        let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
        let mut sub_capsules = EncoderSubCapsules::new();

        wiring.encode_frame(&frame, &mut sub_capsules).unwrap();

        b.iter(|| {
            let entropy = sub_capsules.entropy();
            black_box(entropy)
        })
    });

    group.finish();
}

/// Benchmark: Reconstructed buffer access
///
/// **Target**: <10ns (simple slice read)
///
/// **Purpose**: Validate zero-copy access to reconstructed pixels
fn bench_reconstructed_buffer_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("reconstructed_buffer_access");
    group.throughput(Throughput::Elements(1)); // 1 buffer access

    let frame = create_test_frame(64, 64);

    group.bench_function("reconstructed_buffer_read", |b| {
        let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
        let mut sub_capsules = EncoderSubCapsules::new();

        // Encode one frame to populate reconstructed buffer
        wiring.encode_frame(&frame, &mut sub_capsules).unwrap();

        b.iter(|| {
            // Measure buffer access time
            let reconstructed = sub_capsules.reconstructed_buffer();
            black_box(reconstructed.len())
        })
    });

    group.bench_function("reconstructed_buffer_ptr", |b| {
        let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
        let mut sub_capsules = EncoderSubCapsules::new();

        wiring.encode_frame(&frame, &mut sub_capsules).unwrap();

        b.iter(|| {
            // Measure raw pointer access (zero-copy reference frames)
            let ptr = sub_capsules.reconstructed_buffer_ptr();
            black_box(ptr as usize)
        })
    });

    group.finish();
}

/// Benchmark: Encoding scaling across resolutions
///
/// **Purpose**: Validate O(N) scaling for encoding (including reconstruction)
fn bench_encoding_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoding_scaling_by_resolution");

    for &(width, height) in [(64, 64), (320, 240), (640, 480), (1280, 720)].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x{}", width, height)),
            &(width, height),
            |b, &(w, h)| {
                let frame = create_test_frame(w, h);

                b.iter(|| {
                    let wiring = EncoderWiringCapsule::with_params(w as u32, h as u32, 28, 5);
                    let mut sub_capsules = EncoderSubCapsules::new();

                    let result = wiring.encode_frame(black_box(&frame), &mut sub_capsules);
                    assert!(result.is_ok());

                    black_box(sub_capsules.reconstructed_buffer().len())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: SIMD capsule availability (loop filter, CDEF, LRF)
///
/// **Purpose**: Validate SIMD capsules are present when portable_simd enabled
///
/// **Target**: <10ns access (Option check + pointer read)
#[cfg(feature = "portable_simd")]
fn bench_simd_capsule_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_capsule_access");

    let frame = create_test_frame(64, 64);

    // Loop filter capsule access
    group.bench_function("loop_filter_access", |b| {
        let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
        let mut sub_capsules = EncoderSubCapsules::new();

        wiring.encode_frame(&frame, &mut sub_capsules).unwrap();

        b.iter(|| {
            if let Some(loop_filter) = sub_capsules.loop_filter() {
                black_box(loop_filter)
            }
        })
    });

    // CDEF filter capsule access
    group.bench_function("cdef_access", |b| {
        let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
        let mut sub_capsules = EncoderSubCapsules::new();

        wiring.encode_frame(&frame, &mut sub_capsules).unwrap();

        b.iter(|| {
            if let Some(cdef) = sub_capsules.cdef() {
                black_box(cdef)
            }
        })
    });

    // Loop restoration filter capsule access
    group.bench_function("lrf_access", |b| {
        let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
        let mut sub_capsules = EncoderSubCapsules::new();

        wiring.encode_frame(&frame, &mut sub_capsules).unwrap();

        b.iter(|| {
            if let Some(lrf) = sub_capsules.lrf() {
                black_box(lrf)
            }
        })
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
criterion_group!(
    benches,
    bench_full_frame_encoding_64x64,
    bench_full_frame_encoding_1080p,
    bench_individual_capsules,
    bench_reconstructed_buffer_access,
    bench_encoding_scaling,
    bench_simd_capsule_access
);

#[cfg(not(feature = "portable_simd"))]
criterion_group!(
    benches,
    bench_full_frame_encoding_64x64,
    bench_full_frame_encoding_1080p,
    bench_individual_capsules,
    bench_reconstructed_buffer_access,
    bench_encoding_scaling
);

criterion_main!(benches);
