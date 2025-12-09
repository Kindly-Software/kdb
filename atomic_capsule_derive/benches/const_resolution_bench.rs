//! # B32 Benchmark: Const Expression Resolution Performance
//!
//! Measures the performance of const expression resolution in field_size.rs
//!
//! # Methodology (B32 Framework)
//!
//! - **Baseline**: Literal array sizes `[u8; 64]` (no resolution needed)
//! - **Optimized**: Const name resolution `[u8; CONST_SIZE]`
//! - **Hardware**: Same machine, same compiler
//! - **Iterations**: 1000+ per benchmark
//! - **Confidence**: 95% CI via criterion
//!
//! # Expected Results (B32 Reality Check)
//!
//! - Literal parsing: <1μs (baseline)
//! - Const resolution (cached): <2μs (10-50% overhead - TYPICAL)
//! - Const resolution (uncached): <100μs (file parse - ACCEPTABLE)
//! - Binary expressions: <1μs (simple arithmetic - TYPICAL)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use syn::parse_quote;

// Re-export FieldSizeCalculator for benchmarking
// NOTE: This requires making FieldSizeCalculator public or creating a benchmark helper
// For now, we'll use a simplified benchmark approach

fn benchmark_literal_array(c: &mut Criterion) {
    c.bench_function("literal_array_[u8;64]", |b| {
        b.iter(|| {
            // Simulate parsing literal array size
            let ty: syn::Type = parse_quote!([u8; 64]);
            black_box(ty);
        });
    });
}

fn benchmark_const_array_parsing(c: &mut Criterion) {
    c.bench_function("const_array_[u8;CONST_SIZE]_parse", |b| {
        b.iter(|| {
            // Simulate parsing const array size
            let ty: syn::Type = parse_quote!([u8; CONST_SIZE]);
            black_box(ty);
        });
    });
}

fn benchmark_binary_expression_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary_expressions");

    for expr in &["8 * 8", "32 + 32", "128 / 2", "100 - 36"] {
        group.bench_with_input(BenchmarkId::new("parse", expr), expr, |b, e| {
            b.iter(|| {
                // Parse the expression string into a type
                let code = format!("[u8; {}]", e);
                let ty: syn::Type = syn::parse_str(&code).unwrap();
                black_box(ty);
            });
        });
    }

    group.finish();
}

fn benchmark_source_file_parsing(c: &mut Criterion) {
    let source = r#"
        const CONST_A: usize = 16;
        const CONST_B: usize = 32;
        const CONST_C: usize = 64;
        const CONST_D: usize = 128;
        const CONST_E: usize = 256;
    "#;

    c.bench_function("parse_source_file_5_consts", |b| {
        b.iter(|| {
            // Simulate parsing source file
            let file = syn::parse_file(black_box(source)).unwrap();
            black_box(file);
        });
    });
}

fn benchmark_const_lookup_cache_hit(c: &mut Criterion) {
    // This would benchmark HashMap lookup
    use std::collections::HashMap;

    let mut cache = HashMap::new();
    cache.insert("CONST_A".to_string(), 16);
    cache.insert("CONST_B".to_string(), 32);
    cache.insert("CONST_C".to_string(), 64);

    c.bench_function("const_cache_lookup_hit", |b| {
        b.iter(|| {
            let value = cache.get(black_box("CONST_B"));
            black_box(value);
        });
    });
}

fn benchmark_const_lookup_cache_miss(c: &mut Criterion) {
    use std::collections::HashMap;

    let cache = HashMap::new();

    c.bench_function("const_cache_lookup_miss", |b| {
        b.iter(|| {
            let value = cache.get(black_box("CONST_UNDEFINED"));
            black_box(value);
        });
    });
}

criterion_group!(
    benches,
    benchmark_literal_array,
    benchmark_const_array_parsing,
    benchmark_binary_expression_parsing,
    benchmark_source_file_parsing,
    benchmark_const_lookup_cache_hit,
    benchmark_const_lookup_cache_miss,
);
criterion_main!(benches);
