//! Av1EncoderMetacapsule B32 Benchmarks - Minimal Core Functionality
//!
//! [TRADE SECRET] Performance benchmarking for lockfree AV1 encoder state machine.
//! Focuses on core atomic coordination without tile encoding implementation.
//!
//! # Benchmark Groups (8 groups)
//!
//! 1. **State Transitions**: Atomic CAS operations (<100ns target)
//! 2. **Phase Completion**: Atomic OR operations (<50ns target)
//! 3. **Phase Queries**: Atomic load + bit test (<50ns target)
//! 4. **Statistics Snapshot**: Multi-field atomic reads (<50ns target)
//! 5. **Concurrent State Transitions**: Multi-threaded coordination (1-16 threads)
//! 6. **Phase Tracking Overhead**: Bulk phase operations (<1μs target)
//! 7. **Error State Handling**: Error transition + recovery (<100ns target)
//! 8. **Full Workflow Simulation**: Complete encode cycle state machine (<1μs target)

use atomic_capsule::encoder::encoder_metacapsule::{
    Av1EncoderMetacapsule, EncoderPhase, EncoderState,
};
use atomic_capsule::encoder::{
    frame_buffer::FrameType, lrf::LrfCapsule, DctTransformCapsule,
    EncoderStateCapsule, EntropyCoderCapsule, FrameBufferCapsule, GopCoordinatorCapsule,
    LookaheadCapsule, ObuBitstreamWriterCapsule, QualityMode, QuantizationCapsule,
    ReferenceFrameCapsule, SpeedPreset, TemporalRDOCapsule, TileCoordinatorCapsule,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use std::thread;

// ============================================================================
// BENCHMARK GROUP 1: State Transitions
// ============================================================================

fn benchmark_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_transitions");
    group.sample_size(1000);

    group.bench_function("idle_to_lookahead", |b| {
        let metacapsule = create_test_metacapsule();
        b.iter(|| {
            black_box(metacapsule.transition_state(EncoderState::Idle, EncoderState::Lookahead));
            let _ = metacapsule.transition_state(EncoderState::Lookahead, EncoderState::Idle);
        })
    });

    group.bench_function("lookahead_to_gopplanning", |b| {
        let metacapsule = create_test_metacapsule();
        let _ = metacapsule.transition_state(EncoderState::Idle, EncoderState::Lookahead);
        b.iter(|| {
            black_box(
                metacapsule.transition_state(EncoderState::Lookahead, EncoderState::GopPlanning),
            );
            let _ =
                metacapsule.transition_state(EncoderState::GopPlanning, EncoderState::Lookahead);
        })
    });

    group.bench_function("gopplanning_to_encoding", |b| {
        let metacapsule = create_test_metacapsule();
        let _ = metacapsule.transition_state(EncoderState::Idle, EncoderState::Lookahead);
        let _ = metacapsule.transition_state(EncoderState::Lookahead, EncoderState::GopPlanning);
        b.iter(|| {
            black_box(
                metacapsule.transition_state(EncoderState::GopPlanning, EncoderState::Encoding),
            );
            let _ = metacapsule.transition_state(EncoderState::Encoding, EncoderState::GopPlanning);
        })
    });

    group.bench_function("encoding_to_postprocessing", |b| {
        let metacapsule = create_test_metacapsule();
        let _ = metacapsule.transition_state(EncoderState::Idle, EncoderState::Lookahead);
        let _ = metacapsule.transition_state(EncoderState::Lookahead, EncoderState::Encoding);
        b.iter(|| {
            black_box(
                metacapsule.transition_state(EncoderState::Encoding, EncoderState::PostProcessing),
            );
            let _ =
                metacapsule.transition_state(EncoderState::PostProcessing, EncoderState::Encoding);
        })
    });

    group.finish();
}

// ============================================================================
// BENCHMARK GROUP 2: Phase Completion
// ============================================================================

fn benchmark_phase_completion(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase_completion");
    group.sample_size(1000);

    group.bench_function("complete_lookahead", |b| {
        let metacapsule = create_test_metacapsule();
        b.iter(|| {
            black_box(metacapsule.complete_phase(EncoderPhase::Lookahead));
            metacapsule.reset_phases();
        })
    });

    group.bench_function("complete_gopplanning", |b| {
        let metacapsule = create_test_metacapsule();
        b.iter(|| {
            black_box(metacapsule.complete_phase(EncoderPhase::GopPlanning));
            metacapsule.reset_phases();
        })
    });

    group.bench_function("complete_dcttransform", |b| {
        let metacapsule = create_test_metacapsule();
        b.iter(|| {
            black_box(metacapsule.complete_phase(EncoderPhase::DctTransform));
            metacapsule.reset_phases();
        })
    });

    group.bench_function("complete_quantization", |b| {
        let metacapsule = create_test_metacapsule();
        b.iter(|| {
            black_box(metacapsule.complete_phase(EncoderPhase::Quantization));
            metacapsule.reset_phases();
        })
    });

    group.finish();
}

// ============================================================================
// BENCHMARK GROUP 3: Phase Queries
// ============================================================================

fn benchmark_phase_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase_queries");
    group.sample_size(1000);

    group.bench_function("is_phase_complete_lookahead", |b| {
        let metacapsule = create_test_metacapsule();
        metacapsule.complete_phase(EncoderPhase::Lookahead);
        b.iter(|| black_box(metacapsule.is_phase_complete(EncoderPhase::Lookahead)))
    });

    group.bench_function("is_phase_complete_gopplanning", |b| {
        let metacapsule = create_test_metacapsule();
        metacapsule.complete_phase(EncoderPhase::GopPlanning);
        b.iter(|| black_box(metacapsule.is_phase_complete(EncoderPhase::GopPlanning)))
    });

    group.bench_function("is_phase_complete_dcttransform", |b| {
        let metacapsule = create_test_metacapsule();
        metacapsule.complete_phase(EncoderPhase::DctTransform);
        b.iter(|| black_box(metacapsule.is_phase_complete(EncoderPhase::DctTransform)))
    });

    group.finish();
}

// ============================================================================
// BENCHMARK GROUP 4: Statistics Snapshot
// ============================================================================

fn benchmark_statistics_snapshot(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistics_snapshot");
    group.sample_size(1000);

    group.bench_function("stats_snapshot", |b| {
        let metacapsule = create_test_metacapsule();
        b.iter(|| black_box(metacapsule.stats()))
    });

    group.bench_function("state_query", |b| {
        let metacapsule = create_test_metacapsule();
        b.iter(|| black_box(metacapsule.state()))
    });

    group.finish();
}

// ============================================================================
// BENCHMARK GROUP 5: Concurrent State Transitions
// ============================================================================

fn benchmark_concurrent_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_state_transitions");
    group.sample_size(100);

    for thread_count in [1, 2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let metacapsule = Arc::new(create_test_metacapsule());
                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let mc = Arc::clone(&metacapsule);
                            thread::spawn(move || {
                                for _ in 0..100 {
                                    let _ = mc.transition_state(
                                        EncoderState::Idle,
                                        EncoderState::Lookahead,
                                    );
                                    let _ = mc.transition_state(
                                        EncoderState::Lookahead,
                                        EncoderState::Idle,
                                    );
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK GROUP 6: Phase Tracking Overhead
// ============================================================================

fn benchmark_phase_tracking_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase_tracking_overhead");
    group.sample_size(100);

    group.bench_function("complete_10_phases", |b| {
        let metacapsule = create_test_metacapsule();
        b.iter(|| {
            for _ in 0..10 {
                metacapsule.complete_phase(EncoderPhase::Lookahead);
                metacapsule.complete_phase(EncoderPhase::GopPlanning);
                metacapsule.complete_phase(EncoderPhase::DctTransform);
                metacapsule.complete_phase(EncoderPhase::Quantization);
                metacapsule.complete_phase(EncoderPhase::EntropyCoding);
            }
            metacapsule.reset_phases();
        })
    });

    group.bench_function("query_10_phases", |b| {
        let metacapsule = create_test_metacapsule();
        metacapsule.complete_phase(EncoderPhase::Lookahead);
        metacapsule.complete_phase(EncoderPhase::GopPlanning);
        b.iter(|| {
            for _ in 0..10 {
                black_box(metacapsule.is_phase_complete(EncoderPhase::Lookahead));
                black_box(metacapsule.is_phase_complete(EncoderPhase::GopPlanning));
                black_box(metacapsule.is_phase_complete(EncoderPhase::DctTransform));
            }
        })
    });

    group.finish();
}

// ============================================================================
// BENCHMARK GROUP 7: Error State Handling
// ============================================================================

fn benchmark_error_state_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_state_handling");
    group.sample_size(1000);

    group.bench_function("error_transition", |b| {
        let metacapsule = create_test_metacapsule();
        b.iter(|| {
            let _ = metacapsule.transition_state(EncoderState::Idle, EncoderState::Error);
            let _ = metacapsule.transition_state(EncoderState::Error, EncoderState::Idle);
        })
    });

    group.bench_function("error_recovery", |b| {
        let metacapsule = create_test_metacapsule();
        let _ = metacapsule.transition_state(EncoderState::Idle, EncoderState::Error);
        b.iter(|| {
            black_box(metacapsule.transition_state(EncoderState::Error, EncoderState::Idle));
            let _ = metacapsule.transition_state(EncoderState::Idle, EncoderState::Error);
        })
    });

    group.finish();
}

// ============================================================================
// BENCHMARK GROUP 8: Full Workflow Simulation
// ============================================================================

fn benchmark_full_workflow_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_workflow_simulation");
    group.sample_size(100);

    group.bench_function("full_encode_cycle", |b| {
        let metacapsule = create_test_metacapsule();
        b.iter(|| {
            let _ = metacapsule.transition_state(EncoderState::Idle, EncoderState::Lookahead);
            metacapsule.complete_phase(EncoderPhase::Lookahead);

            let _ =
                metacapsule.transition_state(EncoderState::Lookahead, EncoderState::GopPlanning);
            metacapsule.complete_phase(EncoderPhase::GopPlanning);

            let _ = metacapsule.transition_state(EncoderState::GopPlanning, EncoderState::Encoding);
            metacapsule.complete_phase(EncoderPhase::IntraPrediction);
            metacapsule.complete_phase(EncoderPhase::DctTransform);
            metacapsule.complete_phase(EncoderPhase::Quantization);

            let _ =
                metacapsule.transition_state(EncoderState::Encoding, EncoderState::PostProcessing);
            metacapsule.complete_phase(EncoderPhase::LoopFilter);

            let _ = metacapsule
                .transition_state(EncoderState::PostProcessing, EncoderState::BitstreamWrite);
            metacapsule.complete_phase(EncoderPhase::BitstreamWrite);

            let _ = metacapsule.transition_state(EncoderState::BitstreamWrite, EncoderState::Idle);
            metacapsule.reset_phases();

            black_box(metacapsule.state())
        })
    });

    group.bench_function("intra_only_workflow", |b| {
        let metacapsule = create_test_metacapsule();
        b.iter(|| {
            let _ = metacapsule.transition_state(EncoderState::Idle, EncoderState::Lookahead);
            metacapsule.complete_phase(EncoderPhase::Lookahead);

            let _ =
                metacapsule.transition_state(EncoderState::Lookahead, EncoderState::GopPlanning);
            metacapsule.complete_phase(EncoderPhase::GopPlanning);

            let _ = metacapsule.transition_state(EncoderState::GopPlanning, EncoderState::Encoding);
            metacapsule.complete_phase(EncoderPhase::IntraPrediction);
            metacapsule.complete_phase(EncoderPhase::DctTransform);

            let _ =
                metacapsule.transition_state(EncoderState::Encoding, EncoderState::BitstreamWrite);
            metacapsule.complete_phase(EncoderPhase::BitstreamWrite);

            let _ = metacapsule.transition_state(EncoderState::BitstreamWrite, EncoderState::Idle);
            metacapsule.reset_phases();

            black_box(metacapsule.state())
        })
    });

    group.bench_function("inter_frame_workflow", |b| {
        let metacapsule = create_test_metacapsule();
        b.iter(|| {
            let _ = metacapsule.transition_state(EncoderState::Idle, EncoderState::Lookahead);
            metacapsule.complete_phase(EncoderPhase::Lookahead);

            let _ =
                metacapsule.transition_state(EncoderState::Lookahead, EncoderState::GopPlanning);
            metacapsule.complete_phase(EncoderPhase::GopPlanning);

            let _ = metacapsule.transition_state(EncoderState::GopPlanning, EncoderState::Encoding);
            metacapsule.complete_phase(EncoderPhase::DctTransform);
            metacapsule.complete_phase(EncoderPhase::Quantization);
            metacapsule.complete_phase(EncoderPhase::TemporalRdo);

            let _ =
                metacapsule.transition_state(EncoderState::Encoding, EncoderState::BitstreamWrite);
            metacapsule.complete_phase(EncoderPhase::BitstreamWrite);

            let _ = metacapsule.transition_state(EncoderState::BitstreamWrite, EncoderState::Idle);
            metacapsule.reset_phases();

            black_box(metacapsule.state())
        })
    });

    group.finish();
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn create_test_metacapsule() -> Av1EncoderMetacapsule {
    let encoder_state = EncoderStateCapsule::new(
        1920,
        1080,
        SpeedPreset::Medium,
        QualityMode::ConstantQuality,
    );
    let frame_buffer = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    let dct_transform = DctTransformCapsule::new();
    let quantization = QuantizationCapsule::new(32);
    let entropy_coder = EntropyCoderCapsule::new();
    let tile_coordinator = TileCoordinatorCapsule::new(4, 4);
    let obu_writer = ObuBitstreamWriterCapsule::new();
    let ref_frame = ReferenceFrameCapsule::new();
    let gop_coordinator = GopCoordinatorCapsule::new(60, 7);
    let temporal_rdo = TemporalRDOCapsule::new(32);
    let lookahead = LookaheadCapsule::new(16);
    let lrf = LrfCapsule::new();

    Av1EncoderMetacapsule::new(
        &encoder_state,
        &frame_buffer,
        &dct_transform,
        &quantization,
        &entropy_coder,
        &tile_coordinator,
        &obu_writer,
        &ref_frame,
        &gop_coordinator,
        &temporal_rdo,
        &lookahead,
        &lrf,
    )
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    benches,
    benchmark_state_transitions,
    benchmark_phase_completion,
    benchmark_phase_queries,
    benchmark_statistics_snapshot,
    benchmark_concurrent_state_transitions,
    benchmark_phase_tracking_overhead,
    benchmark_error_state_handling,
    benchmark_full_workflow_simulation
);

criterion_main!(benches);
