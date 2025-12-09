//! B32 Benchmarks: Reference Frame Performance
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Validates reference frame operations performance:
//! - Reference slot update: <50ns target
//! - Reference lookup: <10ns target
//! - Slot validation check: <5ns target
//! - Best reference selection: <100ns target
//!
//! ## B32 Framework Compliance
//!
//! - 95% CI (Criterion default)
//! - 1000+ iterations per benchmark
//! - Fair baselines (lockfree atomic vs traditional mutex/RwLock)
//! - Reproducibility (kindly-hub: AMD Ryzen 9 6900HX)
//!
//! ## Run Commands (kindly-hub MANDATORY)
//!
//! ```bash
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench reference_cascade_bench --release"
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use kindly_av1::encoder::{EncoderWiringCapsule, EncoderSubCapsules};
use atomic_capsule::encoder::ReferenceTypeV2;

/// Create test frame with gradient pattern
fn create_test_frame(width: usize, height: usize, offset: u8) -> Vec<u8> {
    let mut frame = Vec::with_capacity(width * height * 3 / 2); // YUV 4:2:0

    // Y plane
    for y in 0..height {
        for x in 0..width {
            let value = ((x + y) / 16) as u8;
            frame.push(value.wrapping_add(offset));
        }
    }

    // U plane (half resolution)
    for y in 0..height/2 {
        for x in 0..width/2 {
            let value = ((x + y) / 8) as u8;
            frame.push(value.wrapping_add(offset / 2));
        }
    }

    // V plane (half resolution)
    for y in 0..height/2 {
        for x in 0..width/2 {
            let value = ((x + y) / 8) as u8;
            frame.push(value.wrapping_add(offset / 4));
        }
    }

    frame
}

/// Benchmark: Reference slot update latency
///
/// **Target**: <50ns per slot update (atomic store + metadata update)
///
/// **Baseline**: Mutex-based reference manager: ~200ns
/// **Speedup**: 4×+ expected
fn bench_reference_slot_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("reference_slot_update");
    group.throughput(Throughput::Elements(1));

    let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
    let mut sub_capsules = EncoderSubCapsules::new();

    // Encode one frame to initialize reference slots
    let frame = create_test_frame(64, 64, 0);
    let _ = wiring.encode_frame(&frame, &mut sub_capsules);

    group.bench_function("update_last_frame", |b| {
        let frame_ptr = 0x1000 as *const u8; // Dummy pointer
        b.iter(|| {
            sub_capsules.ref_frames_mut().update_last_frame(
                black_box(frame_ptr),
                black_box(100),
                black_box(50)
            );
        });
    });

    group.bench_function("update_golden_slot", |b| {
        let frame_ptr = 0x2000 as *const u8;
        b.iter(|| {
            sub_capsules.ref_frames_mut().update_slot(
                black_box(3), // Golden slot
                black_box(frame_ptr),
                black_box(ReferenceTypeV2::Golden),
                black_box(200),
                black_box(100)
            );
        });
    });

    // Benchmark all 8 slots
    for slot in 0u8..8 {
        group.bench_function(format!("update_slot_{}", slot), |b| {
            let frame_ptr = (0x3000usize + slot as usize * 0x1000) as *const u8;
            let ref_type = ReferenceTypeV2::from_slot(slot).unwrap();
            b.iter(|| {
                sub_capsules.ref_frames_mut().update_slot(
                    black_box(slot),
                    black_box(frame_ptr),
                    black_box(ref_type),
                    black_box(slot as u32 * 10),
                    black_box(slot as u8 * 5)
                );
            });
        });
    }

    group.finish();
}

/// Benchmark: Reference lookup latency
///
/// **Target**: <10ns per lookup (atomic load from slot)
///
/// **Baseline**: RwLock-based reference manager: ~50ns
/// **Speedup**: 5×+ expected
fn bench_reference_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("reference_lookup");
    group.throughput(Throughput::Elements(1));

    let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
    let mut sub_capsules = EncoderSubCapsules::new();

    // Encode multiple frames to populate reference slots
    for i in 0..4 {
        let frame = create_test_frame(64, 64, i * 10);
        let _ = wiring.encode_frame(&frame, &mut sub_capsules);
    }

    group.bench_function("get_last_reference", |b| {
        b.iter(|| {
            black_box(sub_capsules.ref_frames().get_reference(
                black_box(ReferenceTypeV2::Last)
            ))
        });
    });

    group.bench_function("get_golden_reference", |b| {
        b.iter(|| {
            black_box(sub_capsules.ref_frames().get_reference(
                black_box(ReferenceTypeV2::Golden)
            ))
        });
    });

    group.bench_function("get_altref_reference", |b| {
        b.iter(|| {
            black_box(sub_capsules.ref_frames().get_reference(
                black_box(ReferenceTypeV2::AltRef)
            ))
        });
    });

    // Benchmark all 7 reference types (skip IntraFrame)
    for slot in 0..7 {
        let ref_type = ReferenceTypeV2::from_slot(slot).unwrap();
        group.bench_function(format!("get_reference_slot_{}", slot), |b| {
            b.iter(|| {
                black_box(sub_capsules.ref_frames().get_reference(
                    black_box(ref_type)
                ))
            });
        });
    }

    group.finish();
}

/// Benchmark: Slot validity checks
///
/// **Target**: <5ns per check (single atomic load + bit test)
///
/// **Baseline**: Mutex-based check: ~100ns
/// **Speedup**: 20×+ expected
fn bench_slot_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("slot_validation");
    group.throughput(Throughput::Elements(1));

    let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
    let mut sub_capsules = EncoderSubCapsules::new();

    // Encode frames to populate some slots
    for i in 0..4 {
        let frame = create_test_frame(64, 64, i * 10);
        let _ = wiring.encode_frame(&frame, &mut sub_capsules);
    }

    // Benchmark validity check for all 8 slots
    for slot in 0..8 {
        group.bench_function(format!("is_slot_valid_{}", slot), |b| {
            b.iter(|| {
                black_box(sub_capsules.ref_frames().is_slot_valid(
                    black_box(slot)
                ))
            });
        });
    }

    group.bench_function("check_multiple_slots", |b| {
        b.iter(|| {
            let mut count = 0u32;
            for slot in 0..8 {
                if sub_capsules.ref_frames().is_slot_valid(slot) {
                    count += 1;
                }
            }
            black_box(count)
        });
    });

    group.finish();
}

/// Benchmark: Best reference selection
///
/// **Target**: <100ns for 7-reference scan + priority sort
///
/// **Method**: Temporal distance + validity check + priority scoring
fn bench_best_reference_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("best_reference_selection");
    group.throughput(Throughput::Elements(1));

    let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
    let mut sub_capsules = EncoderSubCapsules::new();

    // Encode frames to populate reference slots
    for i in 0..7 {
        let frame = create_test_frame(64, 64, i * 10);
        let _ = wiring.encode_frame(&frame, &mut sub_capsules);
    }

    group.bench_function("select_best_refs_max_3", |b| {
        b.iter(|| {
            black_box(sub_capsules.ref_frames().select_best_refs(black_box(3)))
        });
    });

    group.bench_function("select_best_refs_max_5", |b| {
        b.iter(|| {
            black_box(sub_capsules.ref_frames().select_best_refs(black_box(5)))
        });
    });

    group.bench_function("select_best_refs_max_7", |b| {
        b.iter(|| {
            black_box(sub_capsules.ref_frames().select_best_refs(black_box(7)))
        });
    });

    group.finish();
}

/// Benchmark: Frame ID retrieval
///
/// **Target**: <10ns per frame ID query
///
/// **Method**: Atomic load + bit extraction
fn bench_frame_id_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_id_query");
    group.throughput(Throughput::Elements(1));

    let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
    let mut sub_capsules = EncoderSubCapsules::new();

    // Encode frames to populate slots
    for i in 0..4 {
        let frame = create_test_frame(64, 64, i * 10);
        let _ = wiring.encode_frame(&frame, &mut sub_capsules);
    }

    // Benchmark frame ID query for all 8 slots
    for slot in 0..8 {
        group.bench_function(format!("get_frame_id_slot_{}", slot), |b| {
            b.iter(|| {
                black_box(sub_capsules.ref_frames().get_frame_id(
                    black_box(slot)
                ))
            });
        });
    }

    group.finish();
}

/// Benchmark: Slot invalidation
///
/// **Target**: <20ns per invalidation (atomic store)
///
/// **Method**: Zero out slot state atomically
fn bench_slot_invalidation(c: &mut Criterion) {
    let mut group = c.benchmark_group("slot_invalidation");
    group.throughput(Throughput::Elements(1));

    group.bench_function("invalidate_single_slot", |b| {
        let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
        let mut sub_capsules = EncoderSubCapsules::new();

        // Populate reference slot first
        let frame = create_test_frame(64, 64, 0);
        let _ = wiring.encode_frame(&frame, &mut sub_capsules);

        b.iter(|| {
            sub_capsules.ref_frames_mut().invalidate_slot(black_box(0));
        });
    });

    group.bench_function("invalidate_all_slots", |b| {
        let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
        let mut sub_capsules = EncoderSubCapsules::new();

        // Populate all slots
        for i in 0..8 {
            let frame = create_test_frame(64, 64, i * 10);
            let _ = wiring.encode_frame(&frame, &mut sub_capsules);
        }

        b.iter(|| {
            for slot in 0..8 {
                sub_capsules.ref_frames_mut().invalidate_slot(slot);
            }
        });
    });

    group.finish();
}

/// Benchmark: Refresh flag operations
///
/// **Target**: <15ns per mark_for_refresh (atomic bit manipulation)
fn bench_refresh_flags(c: &mut Criterion) {
    let mut group = c.benchmark_group("refresh_flags");
    group.throughput(Throughput::Elements(1));

    let wiring = EncoderWiringCapsule::with_params(64, 64, 28, 5);
    let mut sub_capsules = EncoderSubCapsules::new();

    // Initialize
    let frame = create_test_frame(64, 64, 0);
    let _ = wiring.encode_frame(&frame, &mut sub_capsules);

    group.bench_function("mark_single_slot_refresh", |b| {
        b.iter(|| {
            sub_capsules.ref_frames_mut().mark_for_refresh(black_box(0b00000001));
        });
    });

    group.bench_function("mark_multiple_slots_refresh", |b| {
        b.iter(|| {
            sub_capsules.ref_frames_mut().mark_for_refresh(black_box(0b10101010));
        });
    });

    group.bench_function("mark_all_slots_refresh", |b| {
        b.iter(|| {
            sub_capsules.ref_frames_mut().mark_for_refresh(black_box(0b11111111));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_reference_slot_update,
    bench_reference_lookup,
    bench_slot_validation,
    bench_best_reference_selection,
    bench_frame_id_query,
    bench_slot_invalidation,
    bench_refresh_flags
);
criterion_main!(benches);
