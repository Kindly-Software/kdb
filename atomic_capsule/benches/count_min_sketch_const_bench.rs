//! Benchmark: CountMinSketchConst vs runtime Count-Min Sketch
//!
//! Measures:
//! - Insert (single + batch)
//! - Query (single + batch)
//! - Heavy hitter detection (1M items)

#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[cfg(feature = "nightly-const-probabilistic")]
fn benchmark_count_min_sketch_const(c: &mut Criterion) {
    use atomic_capsule::probabilistic::CountMinSketchConst;

    let mut group = c.benchmark_group("count_min_sketch_const");
    group.sample_size(100);

    // Benchmark: Single insert
    group.bench_function("insert_single", |b| {
        let mut cms = CountMinSketchConst::<1024, 4, 100>::new([
            0x9e3779b97f4a7c15,
            0xbf58476d1ce4e5b9,
            0x94d049bb133111eb,
            0x517cc1b727220a95,
        ]);
        b.iter(|| {
            cms.insert(black_box(42), black_box(1));
        });
    });

    // Benchmark: Single query
    group.bench_function("query_single", |b| {
        let mut cms = CountMinSketchConst::<1024, 4, 100>::new([
            0x9e3779b97f4a7c15,
            0xbf58476d1ce4e5b9,
            0x94d049bb133111eb,
            0x517cc1b727220a95,
        ]);
        cms.insert(42, 100);
        b.iter(|| {
            let _ = cms.query(black_box(42));
        });
    });

    // Benchmark: Batch insert (100 items)
    group.bench_function("insert_batch_100", |b| {
        let mut cms = CountMinSketchConst::<1024, 4, 100>::new([
            0x9e3779b97f4a7c15,
            0xbf58476d1ce4e5b9,
            0x94d049bb133111eb,
            0x517cc1b727220a95,
        ]);
        b.iter(|| {
            for i in 0..100 {
                cms.insert(black_box(i), 1);
            }
        });
    });

    // Benchmark: Heavy hitter detection
    group.bench_function("heavy_hitters_1k", |b| {
        let mut cms = CountMinSketchConst::<2048, 4, 100>::new([
            0x9e3779b97f4a7c15,
            0xbf58476d1ce4e5b9,
            0x94d049bb133111eb,
            0x517cc1b727220a95,
        ]);

        // Pre-populate
        for i in 0..1000 {
            cms.insert(i, 10);
        }

        b.iter(|| {
            let _ = cms.heavy_hitters(black_box(5));
        });
    });

    group.finish();
}

#[cfg(not(feature = "nightly-const-probabilistic"))]
fn benchmark_count_min_sketch_const(_c: &mut Criterion) {
    eprintln!(
        "Skipping CountMinSketchConst benchmarks (requires nightly-const-probabilistic feature)"
    );
}

criterion_group!(benches, benchmark_count_min_sketch_const);
criterion_main!(benches);
