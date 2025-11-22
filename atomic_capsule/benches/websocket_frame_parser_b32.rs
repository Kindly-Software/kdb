//! WebSocket Frame Parser Benchmarks (B32 Framework)
//!
//! Measures RFC 6455 frame parsing performance using Criterion.rs
//! with fair baselines and statistical validation.

use atomic_capsule::websocket::{WebSocketFrameParserCapsule, ParseResult};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};

fn bench_parse_simple_text_frame(c: &mut Criterion) {
    // RFC 6455 Example 7.1: Simple unmasked text frame
    let frame = vec![
        0x81, // FIN=1, RSV=0, Opcode=TEXT(1)
        0x05, // MASK=0, Length=5
        0x48, 0x65, 0x6c, 0x6c, 0x6f, // "Hello"
    ];

    let mut group = c.benchmark_group("frame_parsing");
    group.throughput(Throughput::Bytes(frame.len() as u64));

    group.bench_function("parse_simple_text_frame", |b| {
        b.iter(|| {
            let parser = WebSocketFrameParserCapsule::new();
            parser.parse_frame(black_box(&frame))
        })
    });

    group.finish();
}

fn bench_parse_masked_binary_frame(c: &mut Criterion) {
    // RFC 6455 Example 7.2: Masked binary frame
    let frame = vec![
        0x82, // FIN=1, RSV=0, Opcode=BINARY(2)
        0x83, // MASK=1, Length=3
        0x37, 0xfa, 0x21, 0x3d, // Masking key
        0x7f, 0x9f, 0x4d, // Masked data
    ];

    let mut group = c.benchmark_group("frame_parsing");
    group.throughput(Throughput::Bytes(frame.len() as u64));

    group.bench_function("parse_masked_binary_frame", |b| {
        b.iter(|| {
            let parser = WebSocketFrameParserCapsule::new();
            parser.parse_frame(black_box(&frame))
        })
    });

    group.finish();
}

fn bench_parse_various_payload_lengths(c: &mut Criterion) {
    let mut group = c.benchmark_group("payload_length_variants");

    // 7-bit length (0-125 bytes)
    let frame_7bit = vec![
        0x81, 0x50, // FIN=1, Opcode=TEXT, Length=80
    ].into_iter().chain(vec![0x41u8; 80]).collect::<Vec<_>>();

    group.throughput(Throughput::Bytes(frame_7bit.len() as u64));
    group.bench_function("7bit_length_80bytes", |b| {
        b.iter(|| {
            let parser = WebSocketFrameParserCapsule::new();
            parser.parse_frame(black_box(&frame_7bit))
        })
    });

    // 16-bit length (126 bytes)
    let mut frame_16bit = vec![
        0x81, // FIN=1, Opcode=TEXT
        0x7e, // MASK=0, Length=126
        0x00, 0x7e, // 126 bytes in big-endian
    ];
    frame_16bit.extend(vec![0x42u8; 126]);

    group.throughput(Throughput::Bytes(frame_16bit.len() as u64));
    group.bench_function("16bit_length_126bytes", |b| {
        b.iter(|| {
            let parser = WebSocketFrameParserCapsule::new();
            parser.parse_frame(black_box(&frame_16bit))
        })
    });

    // 64-bit length (256 bytes)
    let mut frame_64bit = vec![
        0x81, // FIN=1, Opcode=TEXT
        0x7f, // MASK=0, Length=127
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, // 256 bytes
    ];
    frame_64bit.extend(vec![0x43u8; 256]);

    group.throughput(Throughput::Bytes(frame_64bit.len() as u64));
    group.bench_function("64bit_length_256bytes", |b| {
        b.iter(|| {
            let parser = WebSocketFrameParserCapsule::new();
            parser.parse_frame(black_box(&frame_64bit))
        })
    });

    group.finish();
}

fn bench_parser_metrics(c: &mut Criterion) {
    let frame = vec![
        0x81, 0x05,
        0x48, 0x65, 0x6c, 0x6c, 0x6f,
    ];

    let mut group = c.benchmark_group("metrics");

    group.bench_function("frames_parsed_counter", |b| {
        b.iter(|| {
            let parser = WebSocketFrameParserCapsule::new();
            let _ = parser.parse_frame(&frame);
            black_box(parser.frames_parsed())
        })
    });

    group.bench_function("error_count_counter", |b| {
        b.iter(|| {
            let parser = WebSocketFrameParserCapsule::new();
            black_box(parser.error_count())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_parse_simple_text_frame,
    bench_parse_masked_binary_frame,
    bench_parse_various_payload_lengths,
    bench_parser_metrics,
);
criterion_main!(benches);
