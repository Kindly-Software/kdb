//! Criterion benchmarks for fix_padding_fields (B32 Framework)
//!
//! B32 Requirements:
//! - Fair baselines (not strawman)
//! - 95% confidence interval
//! - 1000+ iterations
//! - Reproducible results
//! - Reality check: 10-50% typical, 2-10× exceptional

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use fix_padding_fields::{extract_capsules, PaddingCalculator, PaddingFixer};

// Test fixtures
const SIMPLE_CAPSULE: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct SimpleCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}
"#;

const MULTI_FIELD_CAPSULE: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::{AtomicU64, AtomicU32};

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct MultiFieldCapsule {
    counter: AtomicU64,
    flags: AtomicU32,
    timestamp: AtomicU64,
    _padding: [u8; 40],
}
"#;

const INCORRECT_PADDING: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct IncorrectCapsule {
    state: AtomicU64,
    _padding: [u8; 32],
}
"#;

// B1: Benchmark parsing simple capsule
fn bench_parse_simple(c: &mut Criterion) {
    c.bench_function("parse_simple", |b| {
        b.iter(|| {
            let capsules = extract_capsules(black_box(SIMPLE_CAPSULE)).unwrap();
            black_box(capsules);
        });
    });
}

// B2: Benchmark parsing multi-field capsule
fn bench_parse_multi_field(c: &mut Criterion) {
    c.bench_function("parse_multi_field", |b| {
        b.iter(|| {
            let capsules = extract_capsules(black_box(MULTI_FIELD_CAPSULE)).unwrap();
            black_box(capsules);
        });
    });
}

// B3: Benchmark padding calculation
fn bench_calculate_padding(c: &mut Criterion) {
    let capsules = extract_capsules(SIMPLE_CAPSULE).unwrap();
    let capsule = &capsules[0];

    c.bench_function("calculate_padding", |b| {
        b.iter(|| {
            let calc = PaddingCalculator::new(black_box(capsule)).unwrap();
            black_box(calc.required_padding());
        });
    });
}

// B4: Benchmark needs_fixing check
fn bench_needs_fixing(c: &mut Criterion) {
    let capsules = extract_capsules(SIMPLE_CAPSULE).unwrap();
    let capsule = &capsules[0];

    c.bench_function("needs_fixing", |b| {
        b.iter(|| {
            let calc = PaddingCalculator::new(black_box(capsule)).unwrap();
            black_box(calc.needs_fixing());
        });
    });
}

// B5: Benchmark applying padding fix
fn bench_apply_fix(c: &mut Criterion) {
    c.bench_function("apply_fix", |b| {
        b.iter(|| {
            let capsules = extract_capsules(INCORRECT_PADDING).unwrap();
            let mut fixer = PaddingFixer::new(INCORRECT_PADDING.to_string());
            let result = fixer.apply_padding_fix(black_box(&capsules[0])).unwrap();
            black_box(result);
        });
    });
}

// B6: Benchmark complete workflow (parse → calculate → fix)
fn bench_complete_workflow(c: &mut Criterion) {
    c.bench_function("complete_workflow", |b| {
        b.iter(|| {
            let capsules = extract_capsules(black_box(INCORRECT_PADDING)).unwrap();
            let mut fixer = PaddingFixer::new(INCORRECT_PADDING.to_string());

            for capsule in &capsules {
                let calc = PaddingCalculator::new(capsule).unwrap();
                if calc.needs_fixing() {
                    fixer.apply_padding_fix(capsule).unwrap();
                }
            }

            black_box(fixer.content());
        });
    });
}

// B7: Benchmark scalability with different alignments
fn bench_alignment_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("alignment_scalability");

    for alignment in [32, 64, 128, 256] {
        let test_capsule = format!(r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = {}, size = {})]
#[repr(C, align({}))]
struct TestCapsule {{
    state: AtomicU64,
}}
"#, alignment, alignment, alignment);

        group.bench_with_input(
            BenchmarkId::from_parameter(alignment),
            &test_capsule,
            |b, input| {
                b.iter(|| {
                    let capsules = extract_capsules(black_box(input)).unwrap();
                    let calc = PaddingCalculator::new(black_box(&capsules[0])).unwrap();
                    black_box(calc.required_padding());
                });
            },
        );
    }

    group.finish();
}

// B8: Benchmark scalability with different numbers of fields
fn bench_field_count_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_count_scalability");

    for num_fields in [1, 3, 5, 10] {
        let mut fields = String::new();
        for i in 0..num_fields {
            fields.push_str(&format!("    field{}: AtomicU64,\n", i));
        }

        let test_capsule = format!(r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct TestCapsule {{
{}
}}
"#, fields);

        group.bench_with_input(
            BenchmarkId::from_parameter(num_fields),
            &test_capsule,
            |b, input| {
                b.iter(|| {
                    let capsules = extract_capsules(black_box(input)).unwrap();
                    black_box(capsules);
                });
            },
        );
    }

    group.finish();
}

// B9: Baseline comparison - Manual calculation vs PaddingCalculator
fn bench_baseline_comparison(c: &mut Criterion) {
    let capsules = extract_capsules(SIMPLE_CAPSULE).unwrap();
    let data_size: usize = capsules[0].fields.iter().map(|f| f.size_bytes).sum();
    let alignment = capsules[0].alignment;

    let mut group = c.benchmark_group("baseline_comparison");

    // Manual calculation (baseline)
    group.bench_function("manual_calculation", |b| {
        b.iter(|| {
            let padding = (black_box(alignment) - (black_box(data_size) % black_box(alignment)))
                % black_box(alignment);
            black_box(padding);
        });
    });

    // PaddingCalculator (our implementation)
    group.bench_function("padding_calculator", |b| {
        b.iter(|| {
            let calc = PaddingCalculator::new(black_box(&capsules[0])).unwrap();
            black_box(calc.required_padding());
        });
    });

    group.finish();
}

// B10: Large file benchmark (production scenario)
fn bench_large_file(c: &mut Criterion) {
    // Create large file with 100 capsules
    let mut large_file = String::new();
    large_file.push_str("use atomic_capsule_derive::ComputationalCapsule;\n");
    large_file.push_str("use core::sync::atomic::AtomicU64;\n\n");

    for i in 0..100 {
        large_file.push_str(&format!(r#"
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct Capsule{} {{
    state: AtomicU64,
    _padding: [u8; 56],
}}
"#, i));
    }

    c.bench_function("large_file_parse", |b| {
        b.iter(|| {
            let capsules = extract_capsules(black_box(&large_file)).unwrap();
            black_box(capsules);
        });
    });
}

criterion_group!(
    benches,
    bench_parse_simple,
    bench_parse_multi_field,
    bench_calculate_padding,
    bench_needs_fixing,
    bench_apply_fix,
    bench_complete_workflow,
    bench_alignment_scalability,
    bench_field_count_scalability,
    bench_baseline_comparison,
    bench_large_file,
);

criterion_main!(benches);
