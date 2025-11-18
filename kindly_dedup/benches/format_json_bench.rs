use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kindly_dedup::format::FormatRegistryCapsule;
use std::fs;

fn bench_json_loading(c: &mut Criterion) {
    let test_data_path = "test_data/synthetic_100k.json";

    if !std::path::Path::new(test_data_path).exists() {
        eprintln!("Test data not found at {}", test_data_path);
        return;
    }

    let buffer = fs::read(test_data_path).expect("Failed to read test file");

    let mut group = c.benchmark_group("format_json");
    group.sample_size(10); // Reduce sample size for large loads

    group.bench_function("load_100k_json_simd", |b| {
        b.iter(|| {
            let buffer_clone = black_box(buffer.clone());
            let registry = FormatRegistryCapsule::default();
            let reader = registry.get_reader("json").expect("JSON reader not found");

            let docs: Vec<_> = reader
                .read_from_buffer(buffer_clone, None)
                .into_iter()
                .filter_map(|r| r.ok())
                .collect();

            black_box(docs.len())
        });
    });

    group.finish();
}

// Criterion benchmarks
criterion_group!(benches, bench_json_loading);
criterion_main!(benches);
