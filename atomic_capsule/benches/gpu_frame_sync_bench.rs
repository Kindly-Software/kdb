//! B32 Benchmarks for GpuFrameSyncCapsule
//!
//! Performance targets:
//! - begin_frame: <5ns
//! - submit_frame: <10ns
//! - poll_completion: <5ns
//! - stats: <10ns

#![cfg(all(feature = "std", feature = "tui-terminal"))]

use atomic_capsule::terminal::render::GpuFrameSyncCapsule;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_begin_frame(c: &mut Criterion) {
    let sync = GpuFrameSyncCapsule::new(60, false);

    c.bench_function("begin_frame", |b| {
        b.iter(|| {
            black_box(sync.begin_frame());
        });
    });
}

fn bench_submit_frame(c: &mut Criterion) {
    let sync = GpuFrameSyncCapsule::new(60, false);
    sync.begin_frame();

    let mut fence = 0u64;
    c.bench_function("submit_frame", |b| {
        b.iter(|| {
            fence += 1;
            sync.submit_frame(black_box(fence));
        });
    });
}

fn bench_poll_completion(c: &mut Criterion) {
    let sync = GpuFrameSyncCapsule::new(60, false);
    sync.begin_frame();
    sync.submit_frame(100);

    c.bench_function("poll_completion", |b| {
        b.iter(|| {
            black_box(sync.poll_completion(100));
        });
    });
}

fn bench_stats(c: &mut Criterion) {
    let sync = GpuFrameSyncCapsule::new(60, false);
    sync.begin_frame();
    sync.submit_frame(100);
    sync.poll_completion(100);

    c.bench_function("stats", |b| {
        b.iter(|| {
            black_box(sync.stats());
        });
    });
}

fn bench_frame_pipeline(c: &mut Criterion) {
    let sync = GpuFrameSyncCapsule::new(60, false);

    let mut fence = 0u64;
    c.bench_function("full_frame_pipeline", |b| {
        b.iter(|| {
            fence += 1;
            sync.begin_frame();
            sync.submit_frame(fence);
            sync.poll_completion(fence);
        });
    });
}

criterion_group!(
    benches,
    bench_begin_frame,
    bench_submit_frame,
    bench_poll_completion,
    bench_stats,
    bench_frame_pipeline
);
criterion_main!(benches);
