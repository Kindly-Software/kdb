use aid_96::{class, Aid96};
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};

fn bench_aid_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("aid96_generation");
    group.throughput(Throughput::Elements(1));
    group.bench_function(BenchmarkId::new("new_aid", "class_unspecified"), |b| {
        b.iter(|| Aid96::new(class::UNSPECIFIED))
    });
    group.bench_function(BenchmarkId::new("new_aid", "class_aeb"), |b| {
        b.iter(|| Aid96::new(class::AEB))
    });
    group.finish();
}

fn bench_base32_encoding(c: &mut Criterion) {
    let id = Aid96::new(class::UNSPECIFIED);
    c.bench_function("aid96_base32_encode", |b| b.iter(|| id.to_base32()));

    let encoded = id.to_base32();
    c.bench_function("aid96_base32_decode", |b| {
        b.iter(|| Aid96::from_base32(&encoded).expect("decode"))
    });
}

fn bench_batch_generation(c: &mut Criterion) {
    c.bench_function("aid96_batch_generate", |b| {
        b.iter_batched_ref(
            || Vec::with_capacity(1_000),
            |batch| {
                batch.clear();
                for _ in 0..1_000 {
                    batch.push(Aid96::new(class::UNSPECIFIED));
                }
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    bench_aid_generation,
    bench_base32_encoding,
    bench_batch_generation
);
criterion_main!(benches);
