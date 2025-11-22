use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_simple(c: &mut Criterion) {
    c.bench_function("simple", |b| {
        b.iter(|| {
            let x = black_box(42);
            x + 1
        });
    });
}

criterion_group!(benches, bench_simple);
criterion_main!(benches);
