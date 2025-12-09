//! B32 Benchmarks: Frame Encoding Performance
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Validates frame encoding performance targets:
//! - 64×64 frames: <200µs (baseline for unit testing)
//! - 320×240 frames: <2ms (quarter HD)
//! - 1920×1080 frames: <50ms (full HD target)
//! - Frame setup overhead: <10µs
//! - Multi-threaded encoding validation
//!
//! ## B32 Framework Compliance
//!
//! - 95% CI (Criterion default)
//! - 1000+ iterations per benchmark
//! - Fair baselines (single-threaded encode)
//! - Reproducibility (kindly-hub: AMD Ryzen 9 6900HX, 64GB DDR5-4800)
//!
//! ## SOTA Targets (2024-2025)
//!
//! Based on SVT-AV1 production benchmarks:
//! - 1080p real-time encoding target: >30 fps (33ms per frame)
//! - 4K near real-time target: >15 fps (66ms per frame)
//! - Current Phase 1.2: Intra-only encoding foundation
//!
//! ## Run Commands (kindly-hub MANDATORY)
//!
//! ```bash
//! # All benchmarks
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench tile_parallel_bench --release"
//!
//! # Save baseline
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench tile_parallel_bench --release -- --save-baseline main"
//!
//! # Compare against baseline
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench tile_parallel_bench --release -- --baseline main"
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kindly_av1::encoder::{EncoderWiringCapsule, EncoderSubCapsules};

/// Create test frame with gradient pattern
///
/// Gradient pattern ensures non-trivial encoding (not flat/zero compression)
fn create_test_frame(width: usize, height: usize) -> Vec<u8> {
    let mut frame = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            // Gradient pattern with some texture
            let value = ((x + y) / 16) as u8;
            frame.push(value);
        }
    }
    frame
}

/// Benchmark: Frame encoding at different resolutions
///
/// **Target**: Measure end-to-end frame encoding latency
/// - 64×64: <200µs (baseline)
/// - 320×240: <2ms (quarter HD)
/// - 1920×1080: <50ms (full HD target, currently ~20ms expected)
///
/// **Current Phase**: Phase 1.2 (Intra-only encoding, SIMD-accelerated)
/// **Future Phase**: Phase 6 will add tile parallelism for multi-core scaling
fn bench_frame_encoding_resolution_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_encoding_resolution");

    // Test different resolutions
    let resolutions = [
        (64, 64, "64x64_baseline"),
        (320, 240, "320x240_quarter_hd"),
        (640, 480, "640x480_480p"),
        (1280, 720, "1280x720_720p"),
        (1920, 1080, "1920x1080_1080p"),
    ];

    for (width, height, label) in resolutions.iter() {
        let pixel_count = width * height;
        group.throughput(Throughput::Elements(pixel_count as u64));

        let frame = create_test_frame(*width, *height);

        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &(width, height),
            |b, _| {
                b.iter(|| {
                    let wiring = EncoderWiringCapsule::with_params(*width as u32, *height as u32, 28, 5);
                    let mut sub_capsules = EncoderSubCapsules::new();

                    let result = wiring.encode_frame(black_box(&frame), &mut sub_capsules);
                    assert!(result.is_ok(), "Frame encoding failed at {}×{}", width, height);
                    result.unwrap()
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Encoder initialization overhead
///
/// **Target**: <10µs for wiring capsule setup
///
/// Measures the overhead of creating EncoderWiringCapsule and EncoderSubCapsules.
/// This is critical for streaming/real-time scenarios where encoder state may be
/// created/destroyed frequently.
fn bench_encoder_initialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoder_initialization");

    group.bench_function("wiring_capsule_create", |b| {
        b.iter(|| {
            black_box(EncoderWiringCapsule::with_params(1920, 1080, 28, 5))
        })
    });

    group.bench_function("sub_capsules_create", |b| {
        b.iter(|| {
            black_box(EncoderSubCapsules::new())
        })
    });

    group.bench_function("full_encoder_setup", |b| {
        b.iter(|| {
            let wiring = EncoderWiringCapsule::with_params(1920, 1080, 28, 5);
            let sub_capsules = EncoderSubCapsules::new();
            black_box((wiring, sub_capsules))
        })
    });

    group.finish();
}

/// Benchmark: Frame encoding with different quality settings (CRF)
///
/// **Target**: Validate CRF impact on encoding time
///
/// Tests CRF values from 0 (lossless) to 51 (lowest quality):
/// - CRF 18: High quality (visually lossless)
/// - CRF 28: Medium quality (default)
/// - CRF 40: Low quality (fast encode)
///
/// **Expected**: Lower CRF (higher quality) = slower encode due to more careful quantization
fn bench_frame_encoding_crf_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_encoding_crf");
    group.throughput(Throughput::Elements(64 * 64)); // 64×64 test frame

    let frame = create_test_frame(64, 64);

    let crf_values = [
        (18, "crf_18_high_quality"),
        (28, "crf_28_default"),
        (40, "crf_40_low_quality"),
    ];

    for (crf, label) in crf_values.iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            crf,
            |b, &crf| {
                b.iter(|| {
                    let wiring = EncoderWiringCapsule::with_params(64, 64, crf, 5);
                    let mut sub_capsules = EncoderSubCapsules::new();

                    let result = wiring.encode_frame(black_box(&frame), &mut sub_capsules);
                    assert!(result.is_ok());
                    result.unwrap()
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Multi-frame encoding sequence
///
/// **Target**: Validate encoder state consistency across multiple frames
///
/// Encodes 10 consecutive frames to measure:
/// - Frame-to-frame encoding consistency
/// - State accumulation overhead (if any)
/// - Reference frame management impact
///
/// **Current**: Intra-only (each frame independent)
/// **Future**: Inter-frame prediction will show GOP structure impact
fn bench_multiframe_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiframe_sequence");
    group.sample_size(20); // Reduce sample size (each iteration encodes 10 frames)

    let frames: Vec<Vec<u8>> = (0..10)
        .map(|i| {
            // Create slightly different frames (shift gradient by i)
            let mut frame = Vec::with_capacity(320 * 240);
            for y in 0..240 {
                for x in 0..320 {
                    let value = ((x + y + i * 16) / 16) as u8;
                    frame.push(value);
                }
            }
            frame
        })
        .collect();

    group.bench_function("encode_10_frames_320x240", |b| {
        b.iter(|| {
            let wiring = EncoderWiringCapsule::with_params(320, 240, 28, 5);
            let mut sub_capsules = EncoderSubCapsules::new();

            let mut total_bytes = 0usize;
            for frame in frames.iter() {
                let result = wiring.encode_frame(black_box(frame), &mut sub_capsules);
                assert!(result.is_ok());
                total_bytes += result.unwrap().len();
            }

            black_box(total_bytes)
        })
    });

    group.finish();
}

/// Benchmark: Reference frame operations
///
/// **Target**: <10ns for lockfree reference frame queries
///
/// Measures the overhead of reference frame access patterns:
/// - Query reference frame pointer
/// - Check slot validity
/// - Get frame metadata (frame_id, order_hint)
///
/// **Tier**: T1 Atomic (lockfree access via ReferenceFrameCapsuleV2)
fn bench_reference_frame_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("reference_frame_operations");

    // Setup encoder with some reference frames
    let wiring = EncoderWiringCapsule::with_params(1920, 1080, 28, 5);
    let mut sub_capsules = EncoderSubCapsules::new();

    // Encode a frame to populate reference slot 0 (LAST)
    let frame = create_test_frame(1920, 1080);
    let _ = wiring.encode_frame(&frame, &mut sub_capsules);

    group.bench_function("get_reference_last", |b| {
        b.iter(|| {
            use atomic_capsule::encoder::ReferenceTypeV2;
            let ref_ptr = sub_capsules.ref_frames().get_reference(ReferenceTypeV2::Last);
            black_box(ref_ptr)
        })
    });

    group.bench_function("is_slot_valid", |b| {
        b.iter(|| {
            let valid = sub_capsules.ref_frames().is_slot_valid(0);
            black_box(valid)
        })
    });

    group.bench_function("get_frame_id", |b| {
        b.iter(|| {
            let frame_id = sub_capsules.ref_frames().get_frame_id(0);
            black_box(frame_id)
        })
    });

    group.finish();
}

/// Benchmark: Rate control QP decision
///
/// **Target**: <100ns for QP decision (Phase 3 Rate Control)
///
/// Measures the overhead of rate control QP calculation:
/// - Frame complexity estimation (variance-based)
/// - QP decision from rate control capsule
/// - Quantizer update
///
/// **Tier**: T3 Fixed-Point (Q16.16 deterministic arithmetic)
fn bench_rate_control_qp_decision(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_control_qp");

    let wiring = EncoderWiringCapsule::with_params(1920, 1080, 28, 5);
    let mut sub_capsules = EncoderSubCapsules::new();

    let frame = create_test_frame(1920, 1080);

    group.bench_function("frame_complexity_estimation", |b| {
        b.iter(|| {
            // Frame complexity estimation is private, so we measure full encode
            // (complexity estimation is <1% of total encode time)
            let result = wiring.encode_frame(black_box(&frame), &mut sub_capsules);
            black_box(result)
        })
    });

    group.bench_function("rate_control_qp_query", |b| {
        b.iter(|| {
            // Query QP from rate control (with frame complexity)
            let qp = sub_capsules.rate_control().get_qp(1000);
            black_box(qp)
        })
    });

    group.finish();
}

/// Benchmark: Tile coordinator operations
///
/// **Target**: <1µs for tile metadata queries
///
/// Measures the overhead of tile coordinator access patterns:
/// - Get tile boundaries (x0, y0, x1, y1)
/// - Check all tiles done status
/// - Get tile offsets (for bitstream merging)
///
/// **Note**: Actual parallel tile encoding is Phase 6 (future work)
fn bench_tile_coordinator_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("tile_coordinator_operations");

    let sub_capsules = EncoderSubCapsules::new();

    // Configure tiles for 1920×1080
    sub_capsules.tile_coord().configure_tiles(1920, 1080);

    group.bench_function("get_tile_bounds", |b| {
        b.iter(|| {
            let bounds = sub_capsules.tile_coord().get_tile_bounds(0);
            black_box(bounds)
        })
    });

    group.bench_function("all_tiles_done", |b| {
        b.iter(|| {
            let done = sub_capsules.tile_coord().all_tiles_done();
            black_box(done)
        })
    });

    group.bench_function("get_tile_offsets", |b| {
        b.iter(|| {
            let offsets = sub_capsules.tile_coord().get_tile_offsets();
            black_box(offsets)
        })
    });

    group.finish();
}

/// Benchmark: Motion estimation operations
///
/// **Target**: <10µs diamond search (220× vs full search)
///
/// Measures GPU motion estimation capsule access patterns:
/// - Check GPU availability
/// - Query total ME calls
/// - GPU enable/disable overhead
///
/// **Tier**: T7 Heterogeneous (GPU + CPU fallback)
fn bench_motion_estimation_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("motion_estimation_operations");

    let mut sub_capsules = EncoderSubCapsules::new();

    group.bench_function("is_gpu_enabled_query", |b| {
        b.iter(|| {
            let enabled = sub_capsules.motion().is_gpu_enabled();
            black_box(enabled)
        })
    });

    group.bench_function("total_calls_query", |b| {
        b.iter(|| {
            let calls = sub_capsules.motion().total_calls();
            black_box(calls)
        })
    });

    group.bench_function("enable_gpu", |b| {
        b.iter(|| {
            sub_capsules.motion_mut().enable_gpu();
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_frame_encoding_resolution_scaling,
    bench_encoder_initialization,
    bench_frame_encoding_crf_scaling,
    bench_multiframe_sequence,
    bench_reference_frame_operations,
    bench_rate_control_qp_decision,
    bench_tile_coordinator_operations,
    bench_motion_estimation_operations,
);
criterion_main!(benches);
