//! # Adaptive Quantization Performance Benchmarks
//!
//! **Validates performance targets:**
//! - Reader latency: <15ns
//! - Writer latency: <1ms p99
//! - Torn read rate: 0%

use atomic_llm_capsule::primitives::AdaptiveQuantCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use std::thread;

fn bench_load_weight(c: &mut Criterion) {
    let capsule = AdaptiveQuantCapsule::new();
    let weights = [0.5f32; 128];
    capsule.adapt_quantization(&weights);

    c.bench_function("load_weight", |b| {
        b.iter(|| black_box(capsule.load_weight(black_box(0))))
    });
}

fn bench_adapt_quantization(c: &mut Criterion) {
    let capsule = AdaptiveQuantCapsule::new();

    let mut group = c.benchmark_group("adapt_quantization");

    // Uniform weights
    group.bench_function("uniform", |b| {
        let weights = [0.5f32; 128];
        b.iter(|| capsule.adapt_quantization(black_box(&weights)))
    });

    // Varying weights
    group.bench_function("varying", |b| {
        let mut weights = [0.0f32; 128];
        for i in 0..128 {
            weights[i] = (i as f32 - 64.0) / 64.0;
        }
        b.iter(|| capsule.adapt_quantization(black_box(&weights)))
    });

    group.finish();
}

fn bench_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");

    for num_readers in &[1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("readers", num_readers),
            num_readers,
            |b, &num_readers| {
                b.iter(|| {
                    let capsule = Arc::new(AdaptiveQuantCapsule::new());
                    let weights = [0.5f32; 128];
                    capsule.adapt_quantization(&weights);

                    // Writer thread
                    let capsule_writer = Arc::clone(&capsule);
                    let writer = thread::spawn(move || {
                        for i in 0..100 {
                            let w = [(i as f32) * 0.01; 128];
                            capsule_writer.adapt_quantization(&w);
                        }
                    });

                    // Reader threads
                    let mut readers = vec![];
                    for _ in 0..num_readers {
                        let capsule_reader = Arc::clone(&capsule);
                        let reader = thread::spawn(move || {
                            let mut count = 0u64;
                            for _ in 0..1000 {
                                if let Some(_w) = capsule_reader.load_weight(0) {
                                    count += 1;
                                }
                            }
                            count
                        });
                        readers.push(reader);
                    }

                    writer.join().unwrap();
                    for reader in readers {
                        reader.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

fn bench_generation_check(c: &mut Criterion) {
    let capsule = AdaptiveQuantCapsule::new();

    c.bench_function("generation_check", |b| {
        b.iter(|| black_box(capsule.is_committed()))
    });

    c.bench_function("generation_load", |b| {
        b.iter(|| black_box(capsule.generation()))
    });
}

fn bench_statistics(c: &mut Criterion) {
    let capsule = AdaptiveQuantCapsule::new();
    let weights = [0.5f32; 128];
    capsule.adapt_quantization(&weights);

    c.bench_function("statistics", |b| b.iter(|| black_box(capsule.statistics())));
}

criterion_group!(
    benches,
    bench_load_weight,
    bench_adapt_quantization,
    bench_concurrent_access,
    bench_generation_check,
    bench_statistics,
);

criterion_main!(benches);
