//! EncoderStateCapsule B32 Benchmarks
//!
//! Performance validation framework (95% CI, 1000+ iterations)
//! Target: <50ns query, <100ns update operations

use atomic_capsule::encoder::{
    EncoderStateCapsule, EncoderState, SpeedPreset, QualityMode,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

// ============================================================================
// Baseline Benchmarks: Pure Atomic Operations
// ============================================================================

fn bench_atomics_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("atomics_baseline");
    group.sample_size(1000);
    group.measurement_time(core::time::Duration::from_secs(30));

    // Atomic load baseline
    let capsule = black_box(EncoderStateCapsule::new(
        1920,
        1080,
        SpeedPreset::Medium,
        QualityMode::ConstantQuality,
    ));

    group.bench_function("atomic_load", |b| {
        b.iter(|| {
            let state = capsule.get_state();
            black_box(state);
        });
    });

    group.finish();
}

// ============================================================================
// Query Benchmarks: <50ns targets (get operations)
// ============================================================================

fn bench_queries_under_50ns(c: &mut Criterion) {
    let mut group = c.benchmark_group("queries_50ns");
    group.sample_size(1000);
    group.measurement_time(core::time::Duration::from_secs(30));

    let capsule = black_box(EncoderStateCapsule::new(
        1920,
        1080,
        SpeedPreset::Fast,
        QualityMode::VariableBitrate,
    ));

    group.bench_function("get_state", |b| {
        b.iter(|| {
            let state = capsule.get_state();
            black_box(state);
        });
    });

    group.bench_function("get_dimensions", |b| {
        b.iter(|| {
            let dims = capsule.get_dimensions();
            black_box(dims);
        });
    });

    group.bench_function("get_frames_encoded", |b| {
        b.iter(|| {
            let frames = capsule.get_frames_encoded();
            black_box(frames);
        });
    });

    group.bench_function("snapshot", |b| {
        b.iter(|| {
            let snap = capsule.snapshot();
            black_box(snap);
        });
    });

    group.finish();
}

// ============================================================================
// Update Benchmarks: <100ns targets (mutating operations)
// ============================================================================

fn bench_updates_under_100ns(c: &mut Criterion) {
    let mut group = c.benchmark_group("updates_100ns");
    group.sample_size(1000);
    group.measurement_time(core::time::Duration::from_secs(30));

    // Separate capsule for each update operation (avoid state coupling)
    let capsule_state = black_box(EncoderStateCapsule::new(
        1920,
        1080,
        SpeedPreset::Medium,
        QualityMode::ConstantQuality,
    ));

    group.bench_function("update_state_idle_to_encoding", |b| {
        b.iter(|| {
            capsule_state.update_state(EncoderState::Encoding).ok();
            capsule_state.update_state(EncoderState::Idle).ok();
        });
    });

    let capsule_frames = black_box(EncoderStateCapsule::new(
        1920,
        1080,
        SpeedPreset::Medium,
        QualityMode::ConstantQuality,
    ));

    group.bench_function("increment_frames", |b| {
        b.iter(|| {
            let frame_count = capsule_frames.increment_frames();
            black_box(frame_count);
        });
    });

    let capsule_bytes = black_box(EncoderStateCapsule::new(
        1920,
        1080,
        SpeedPreset::Medium,
        QualityMode::ConstantQuality,
    ));

    group.bench_function("add_bytes", |b| {
        b.iter(|| {
            capsule_bytes.add_bytes(65536);
        });
    });

    group.finish();
}

// ============================================================================
// Contention Benchmarks: Multi-threaded throughput
// ============================================================================

fn bench_concurrent_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_throughput");
    group.sample_size(100);
    group.measurement_time(core::time::Duration::from_secs(30));

    for thread_count in [2, 4, 8, 16].iter() {
        let capsule = std::sync::Arc::new(EncoderStateCapsule::new(
            1920,
            1080,
            SpeedPreset::Medium,
            QualityMode::ConstantQuality,
        ));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads_increments", thread_count)),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let mut handles = vec![];

                    for _ in 0..thread_count {
                        let c = capsule.clone();
                        handles.push(std::thread::spawn(move || {
                            for _ in 0..100 {
                                c.increment_frames();
                            }
                        }));
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
// State Transition Benchmarks: Real-world workflow patterns
// ============================================================================

fn bench_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_transitions");
    group.sample_size(1000);
    group.measurement_time(core::time::Duration::from_secs(30));

    group.bench_function("idle_encoding_flushing_completed", |b| {
        b.iter(|| {
            let capsule = black_box(EncoderStateCapsule::new(
                1920,
                1080,
                SpeedPreset::Medium,
                QualityMode::ConstantQuality,
            ));

            capsule.update_state(EncoderState::Encoding).ok();
            capsule.increment_frames();
            capsule.add_bytes(65536);

            capsule.update_state(EncoderState::Flushing).ok();
            capsule.increment_frames();

            capsule.update_state(EncoderState::Completed).ok();
            let snap = capsule.snapshot();
            black_box(snap);
        });
    });

    group.finish();
}

// ============================================================================
// Mixed Operations: Realistic encoding scenario
// ============================================================================

fn bench_realistic_encoding_scenario(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_scenario");
    group.sample_size(100);
    group.measurement_time(core::time::Duration::from_secs(30));

    group.bench_function("encode_10_frames", |b| {
        b.iter(|| {
            let capsule = black_box(EncoderStateCapsule::new(
                3840,
                2160,
                SpeedPreset::Fast,
                QualityMode::VariableBitrate,
            ));

            capsule.update_state(EncoderState::Encoding).ok();
            capsule.set_start_time(1_000_000_000);

            for _ in 0..10 {
                capsule.increment_frames();
                capsule.add_bytes(500_000); // 500KB per frame
            }

            let bitrate = capsule.get_bitrate_kbps();
            black_box(bitrate);

            let snap = capsule.snapshot();
            black_box(snap);
        });
    });

    group.finish();
}

// ============================================================================
// Layout and Memory Benchmarks: Verify no unexpected overhead
// ============================================================================

fn bench_layout_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_verification");
    group.sample_size(1000);

    group.bench_function("size_check", |b| {
        b.iter(|| {
            let size = core::mem::size_of::<EncoderStateCapsule>();
            assert_eq!(black_box(size), 64);
        });
    });

    group.bench_function("align_check", |b| {
        b.iter(|| {
            let align = core::mem::align_of::<EncoderStateCapsule>();
            assert_eq!(black_box(align), 64);
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(1000)
        .measurement_time(core::time::Duration::from_secs(30));
    targets =
        bench_atomics_baseline,
        bench_queries_under_50ns,
        bench_updates_under_100ns,
        bench_concurrent_throughput,
        bench_state_transitions,
        bench_realistic_encoding_scenario,
        bench_layout_verification
);

criterion_main!(benches);
