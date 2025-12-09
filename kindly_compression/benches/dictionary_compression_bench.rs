//! Dictionary compression benchmarks (B32 framework)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use kindly_compression::dictionary::{DictionaryCodec, Provider};

/// Benchmark compression for all 3 providers
fn bench_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("dictionary_compression");

    let providers = [
        ("GPT-4", Provider::GPT4),
        ("Claude", Provider::Claude),
        ("Gemini", Provider::Gemini),
    ];

    // Test data sizes
    let sizes = [100, 500, 1000, 5000];

    for (provider_name, provider) in &providers {
        let codec = DictionaryCodec::new(*provider);

        for &size in &sizes {
            // Generate test data (repeated JSON pattern)
            let data: Vec<u8> = (0..size)
                .flat_map(|i| format!(r#"{{"id": {}, "value": "test"}}"#, i).into_bytes())
                .collect();

            group.bench_with_input(
                BenchmarkId::new(provider_name, size),
                &data,
                |b, data| {
                    b.iter(|| {
                        let compressed = codec.compress_with_dictionary(black_box(data)).unwrap();
                        black_box(compressed);
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark decompression for all 3 providers
fn bench_decompression(c: &mut Criterion) {
    let mut group = c.benchmark_group("dictionary_decompression");

    let providers = [
        ("GPT-4", Provider::GPT4),
        ("Claude", Provider::Claude),
        ("Gemini", Provider::Gemini),
    ];

    let sizes = [100, 500, 1000, 5000];

    for (provider_name, provider) in &providers {
        let codec = DictionaryCodec::new(*provider);

        for &size in &sizes {
            let data: Vec<u8> = (0..size)
                .flat_map(|i| format!(r#"{{"id": {}, "value": "test"}}"#, i).into_bytes())
                .collect();

            let compressed = codec.compress_with_dictionary(&data).unwrap();

            group.bench_with_input(
                BenchmarkId::new(provider_name, size),
                &compressed,
                |b, compressed| {
                    b.iter(|| {
                        let decompressed = codec.decompress_with_dictionary(black_box(compressed)).unwrap();
                        black_box(decompressed);
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark provider-specific optimization
fn bench_provider_specialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_specialization");

    // GPT-4 optimized data (concise, technical)
    let gpt4_data = b"const function return import export class async await if for while switch case break continue";

    // Claude optimized data (verbose, explanatory)
    let claude_data = b"Let's explore this example. However, there's a caveat. Additionally, you can try this approach. In this case, we should consider the trade-offs.";

    // Gemini optimized data (multilingual)
    let gemini_data = "你好世界 こんにちは 안녕하세요 Hello world".as_bytes();

    let test_cases = [
        ("GPT-4 specialized", Provider::GPT4, gpt4_data.to_vec()),
        ("Claude specialized", Provider::Claude, claude_data.to_vec()),
        ("Gemini specialized", Provider::Gemini, gemini_data.to_vec()),
    ];

    for (name, provider, data) in &test_cases {
        let codec = DictionaryCodec::new(*provider);

        group.bench_with_input(
            BenchmarkId::new("compression", name),
            data,
            |b, data| {
                b.iter(|| {
                    let compressed = codec.compress_with_dictionary(black_box(data)).unwrap();
                    black_box(compressed);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_compression,
    bench_decompression,
    bench_provider_specialization
);
criterion_main!(benches);
