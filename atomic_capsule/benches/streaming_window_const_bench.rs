//! Benchmark: StreamingWindowConst<T, WINDOW_MS, SAMPLE_RATE_HZ>
//!
//! Nightly Phase 2: Const Generics T5 Streaming Window
//!
//! # Benchmarks
//! - append(): <20ns target vs runtime window (~50ns)
//! - get_window(): <200ns target vs runtime window (~500ns)
//! - Audio window (48kHz, 100ms): <50µs target vs runtime (~200µs)

#![cfg(all(
    feature = "nightly-const-streaming",
    feature = "nightly"
))]

use atomic_capsule::streaming::StreamingWindowConst;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// ========== Const Generics Window Benchmarks ==========

/// Benchmark: append() operation on different window sizes
fn bench_window_const_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_window_const_append");

    // 256-sample window
    group.bench_function("window_256", |b| {
        type Window = StreamingWindowConst<u32, 256>;
        let window = Window::new();

        b.iter(|| {
            window.append(black_box(42u32));
        });
    });

    // 4096-sample window (≈100ms @ 48kHz)
    group.bench_function("window_4096", |b| {
        type Window = StreamingWindowConst<u32, 4096>;
        let window = Window::new();

        b.iter(|| {
            window.append(black_box(42u32));
        });
    });

    // 65536-sample window (large)
    group.bench_function("window_65536", |b| {
        type Window = StreamingWindowConst<f32, 65536>;
        let window = Window::new();

        b.iter(|| {
            window.append(black_box(3.14f32));
        });
    });

    group.finish();
}

/// Benchmark: get_window() operation
fn bench_window_const_get_window(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_window_const_get_window");

    // 256-sample window
    group.bench_function("window_256", |b| {
        type Window = StreamingWindowConst<u32, 256>;
        let window = Window::new();

        // Pre-fill window
        for i in 0..100 {
            window.append(i as u32);
        }

        b.iter(|| {
            let _ = black_box(window.get_window());
        });
    });

    // 4096-sample window
    group.bench_function("window_4096", |b| {
        type Window = StreamingWindowConst<u32, 4096>;
        let window = Window::new();

        for i in 0..100 {
            window.append(i as u32);
        }

        b.iter(|| {
            let _ = black_box(window.get_window());
        });
    });

    group.finish();
}

/// Benchmark: Audio window realistic workload (4096 samples)
fn bench_window_const_audio_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_window_const_audio_realistic");

    // Realistic audio workload: append 4096 samples
    group.bench_function("fill_and_query", |b| {
        type AudioWindow = StreamingWindowConst<f32, 4096>;
        let window = AudioWindow::new();

        b.iter(|| {
            // Simulate 4096 audio samples
            for i in 0..4096 {
                window.append(black_box(i as f32 / 4096.0));
            }

            // Query window
            let _ = black_box(window.get_window());
        });
    });

    group.finish();
}

/// Benchmark: Compile-time window size (0ns overhead)
fn bench_window_size_compile_time(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_window_const_compile_time");

    // Compile-time calculation (0ns)
    group.bench_function("compile_time_256", |b| {
        type Window = StreamingWindowConst<u32, 256>;
        let window = Window::new();

        b.iter(|| {
            let size = black_box(window.window_size());
            assert_eq!(size, 256);
        });
    });

    group.finish();
}

/// Benchmark: Concurrent append workload (4 threads)
fn bench_window_const_concurrent_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_window_const_concurrent");

    // Concurrent append from multiple threads (simulated)
    group.bench_function("concurrent_4_threads", |b| {
        use std::sync::Arc;
        use std::thread;

        type Window = StreamingWindowConst<u32, 100.0, 48000.0>;
        let window = Arc::new(Window::new());

        b.iter(|| {
            let mut handles = vec![];

            for thread_id in 0..4 {
                let w = Arc::clone(&window);
                let h = thread::spawn(move || {
                    for i in 0..100 {
                        w.append(black_box((thread_id * 1000 + i) as u32));
                    }
                });
                handles.push(h);
            }

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_window_const_append,
    bench_window_const_get_window,
    bench_window_const_audio_realistic,
    bench_window_size_compile_time,
    bench_window_const_concurrent_append,
);

criterion_main!(benches);
