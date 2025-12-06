//! Time-Travel Debugging Benchmarks
//!
//! B32-compliant benchmarks for ReplayEngineCapsule.

use kdb::time_travel::ReplayEngineCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_take_snapshot(c: &mut Criterion) {
    let engine = ReplayEngineCapsule::new();

    c.bench_function("take_snapshot", |b| {
        let mut rip = 0x1000u64;
        let mut rsp = 0x7fff_0000u64;

        b.iter(|| {
            black_box(
                engine
                    .take_snapshot(black_box(rip), black_box(rsp))
                    .unwrap(),
            );
            rip = rip.wrapping_add(4);
            rsp = rsp.wrapping_sub(8);
        });
    });
}

fn bench_step_backward(c: &mut Criterion) {
    let engine = ReplayEngineCapsule::new();

    // Populate with 1000 snapshots
    for i in 0..1000 {
        engine
            .take_snapshot(0x1000 + i * 4, 0x7fff_0000 - i * 8)
            .unwrap();
    }

    c.bench_function("step_backward", |b| {
        b.iter(|| {
            // Jump to middle, then step back
            engine.jump_to_snapshot(500).ok();
            black_box(engine.step_backward().unwrap());
        });
    });
}

fn bench_step_forward(c: &mut Criterion) {
    let engine = ReplayEngineCapsule::new();

    // Populate with 1000 snapshots
    for i in 0..1000 {
        engine
            .take_snapshot(0x1000 + i * 4, 0x7fff_0000 - i * 8)
            .unwrap();
    }

    c.bench_function("step_forward", |b| {
        b.iter(|| {
            // Jump to middle, then step forward
            engine.jump_to_snapshot(500).ok();
            black_box(engine.step_forward().unwrap());
        });
    });
}

fn bench_jump_to_snapshot(c: &mut Criterion) {
    let engine = ReplayEngineCapsule::new();

    // Populate with 4000 snapshots
    for i in 0..4000 {
        engine
            .take_snapshot(0x1000 + i * 4, 0x7fff_0000 - i * 8)
            .unwrap();
    }

    c.bench_function("jump_to_snapshot", |b| {
        b.iter(|| {
            black_box(engine.jump_to_snapshot(black_box(2000)).unwrap());
        });
    });
}

fn bench_sequential_replay(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_replay");

    for size in [100, 500, 1000, 2000].iter() {
        let engine = ReplayEngineCapsule::new();

        // Populate with snapshots
        for i in 0..*size {
            engine
                .take_snapshot(0x1000 + i * 4, 0x7fff_0000 - i * 8)
                .unwrap();
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                engine.jump_to_snapshot(0).ok();
                for _ in 0..size {
                    if engine.step_forward().is_err() {
                        break;
                    }
                }
            });
        });
    }

    group.finish();
}

fn bench_wraparound(c: &mut Criterion) {
    c.bench_function("wraparound_5000_snapshots", |b| {
        b.iter(|| {
            let engine = ReplayEngineCapsule::new();

            // Record 5000 snapshots (exceeds ring buffer)
            for i in 0..5000 {
                black_box(
                    engine
                        .take_snapshot(0x1000 + i * 4, 0x7fff_0000 - i * 8)
                        .unwrap(),
                );
            }

            // Access recent snapshot
            black_box(engine.jump_to_snapshot(4900).unwrap());
        });
    });
}

criterion_group!(
    benches,
    bench_take_snapshot,
    bench_step_backward,
    bench_step_forward,
    bench_jump_to_snapshot,
    bench_sequential_replay,
    bench_wraparound,
);

criterion_main!(benches);
