//! WebSocket Frame Writer Benchmark
//!
//! Performance testing for RFC 6455 frame serialization

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use atomic_capsule::runtime::websocket::WebSocketFrameWriterCapsule;

fn bench_text_frame_small(c: &mut Criterion) {
    let writer = WebSocketFrameWriterCapsule::new();
    let mut buffer = vec![0u8; 256];
    let text = "Hello, WebSocket!";

    c.bench_function("text_frame_small", |b| {
        b.iter(|| {
            let _ = writer.write_text_frame(black_box(text), true, black_box(&mut buffer));
        });
    });
}

fn bench_text_frame_large(c: &mut Criterion) {
    let writer = WebSocketFrameWriterCapsule::new();
    let mut buffer = vec![0u8; 100000];
    let text = "x".repeat(70000);

    c.bench_function("text_frame_large", |b| {
        b.iter(|| {
            let _ = writer.write_text_frame(black_box(&text), true, black_box(&mut buffer));
        });
    });
}

fn bench_binary_frame(c: &mut Criterion) {
    let writer = WebSocketFrameWriterCapsule::new();
    let mut buffer = vec![0u8; 256];
    let data: &[u8] = &[0x00, 0x01, 0x02, 0x03, 0x04];

    c.bench_function("binary_frame", |b| {
        b.iter(|| {
            let _ = writer.write_binary_frame(black_box(data), true, black_box(&mut buffer));
        });
    });
}

fn bench_ping_frame(c: &mut Criterion) {
    let writer = WebSocketFrameWriterCapsule::new();
    let mut buffer = vec![0u8; 256];
    let data = b"PING";

    c.bench_function("ping_frame", |b| {
        b.iter(|| {
            let _ = writer.write_ping_frame(black_box(data), black_box(&mut buffer));
        });
    });
}

fn bench_close_frame(c: &mut Criterion) {
    let writer = WebSocketFrameWriterCapsule::new();
    let mut buffer = vec![0u8; 256];

    c.bench_function("close_frame", |b| {
        b.iter(|| {
            let _ = writer.write_close_frame(black_box(1000), Some("Normal closure"), black_box(&mut buffer));
        });
    });
}

fn bench_multiple_frames(c: &mut Criterion) {
    let writer = WebSocketFrameWriterCapsule::new();
    let mut buffer = vec![0u8; 1024];

    c.bench_function("multiple_frames_sequence", |b| {
        b.iter(|| {
            let _ = writer.write_text_frame(black_box("Frame 1"), true, black_box(&mut buffer));
            let _ = writer.write_binary_frame(black_box(b"Frame 2"), true, black_box(&mut buffer));
            let _ = writer.write_ping_frame(black_box(b"PING"), black_box(&mut buffer));
        });
    });
}

fn bench_stats_lookup(c: &mut Criterion) {
    let writer = WebSocketFrameWriterCapsule::new();
    let mut buffer = vec![0u8; 256];

    // Do some work
    for _ in 0..100 {
        let _ = writer.write_text_frame("Test", true, &mut buffer);
    }

    c.bench_function("stats_lookup", |b| {
        b.iter(|| {
            let _ = writer.stats();
        });
    });
}

criterion_group!(
    benches,
    bench_text_frame_small,
    bench_text_frame_large,
    bench_binary_frame,
    bench_ping_frame,
    bench_close_frame,
    bench_multiple_frames,
    bench_stats_lookup,
);

criterion_main!(benches);
