// Benchmark for BloomFilterConst (Nightly Phase 2: Const Generics)
//
// This is a stub benchmark file demonstrating BloomFilterConst performance.
// Real benchmarks require running with:
// cargo bench --features nightly-const-probabilistic --bench bloom_filter_const_bench
//
// Performance targets (B32 Framework, EXCEPTIONAL tier):
// - Insert: 20-50ns (compile-time k)
// - Lookup: 50-100ns (fixed k)
// - 1MB Bloom: 50-100× speedup (allocation elimination)

use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[cfg(feature = "nightly-const-probabilistic")]
fn bloom_filter_const_insert_bench(c: &mut Criterion) {
    use atomic_capsule::probabilistic::BloomFilterConst;

    c.bench_function("bloom_const_insert_256b_4h", |b| {
        let bloom = BloomFilterConst::<256, 4, 0.01>::new();
        b.iter(|| {
            bloom.insert(black_box(42));
        });
    });

    c.bench_function("bloom_const_insert_1kb_8h", |b| {
        let bloom = BloomFilterConst::<1024, 8, 0.008>::new();
        b.iter(|| {
            bloom.insert(black_box(99));
        });
    });
}

#[cfg(feature = "nightly-const-probabilistic")]
fn bloom_filter_const_lookup_bench(c: &mut Criterion) {
    use atomic_capsule::probabilistic::BloomFilterConst;

    let bloom = BloomFilterConst::<256, 4, 0.01>::new();
    bloom.insert(42);

    c.bench_function("bloom_const_lookup_hit_256b", |b| {
        b.iter(|| {
            black_box(bloom.contains(&black_box(42)));
        });
    });

    c.bench_function("bloom_const_lookup_miss_256b", |b| {
        b.iter(|| {
            black_box(bloom.contains(&black_box(999)));
        });
    });
}

#[cfg(feature = "nightly-const-probabilistic")]
fn bloom_filter_const_fpr_bench(c: &mut Criterion) {
    use atomic_capsule::probabilistic::BloomFilterConst;

    let bloom = BloomFilterConst::<4096, 8, 0.008>::new();
    for i in 0..1000 {
        bloom.insert(i);
    }

    c.bench_function("bloom_const_estimated_fpr", |b| {
        b.iter(|| {
            black_box(bloom.estimated_fpr());
        });
    });
}

#[cfg(feature = "nightly-const-probabilistic")]
criterion_group!(
    benches,
    bloom_filter_const_insert_bench,
    bloom_filter_const_lookup_bench,
    bloom_filter_const_fpr_bench
);

#[cfg(not(feature = "nightly-const-probabilistic"))]
criterion_group!(benches,);

criterion_main!(benches);
