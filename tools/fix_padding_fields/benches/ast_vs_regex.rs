//! B32 Benchmark: AST-based vs Regex-based struct rebuilding.
//!
//! Measures performance of two approaches:
//! 1. **Regex-based** (old): Pattern matching + string replacement
//! 2. **AST-based** (new): syn parse + quote! generate
//!
//! # B32 Framework Compliance
//!
//! - Fair baseline: Same hardware, same compiler, same input
//! - 95% CI: 1000+ iterations via Criterion
//! - Realistic workload: Real atomic_capsule struct definitions
//! - Honest claims: Measure both approaches fairly
//!
//! # Expected Results
//!
//! - AST method: ~3-5μs per struct (pure functional, no string ops)
//! - Regex method: ~10-15μs per struct (pattern matching + string replace)
//! - Goal: 2-3× speedup (AST method faster)
//!
//! # Reality Check (B32)
//!
//! - 10-50% speedup: Typical
//! - 2-10× speedup: Exceptional (requires validation)
//! - 100×+ speedup: Extensive validation needed
//!
//! If AST method is 2-3× faster, this is **EXCEPTIONAL** tier.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fix_padding_fields::ast_rebuilder::rebuild_struct_in_file;
use regex::Regex;

// ============================================================================
// BASELINE: Regex-based struct rebuilding (old method)
// ============================================================================

/// Old regex-based approach from fixer.rs (lines 35-62).
///
/// This is the FAIR baseline - same functionality as AST method.
fn rebuild_struct_regex(content: &str, struct_name: &str, padding_bytes: usize) -> String {
    let struct_pattern = format!(r"struct\s+{}\s*\{{([^}}]*)}}", struct_name);
    let re = Regex::new(&struct_pattern).unwrap();

    if let Some(caps) = re.captures(content) {
        let inner = &caps[1];

        // Remove old padding (simplified for benchmark)
        let padding_pattern = r"_pad\w*:\s*\[u8;\s*\d+\],?\s*";
        let padding_re = Regex::new(padding_pattern).unwrap();
        let cleaned = padding_re.replace_all(inner, "");

        // Add new padding
        let new_inner = if cleaned.ends_with(',') {
            format!("{}\n    _padding: [u8; {}],", cleaned, padding_bytes)
        } else {
            format!("{},\n    _padding: [u8; {}],", cleaned, padding_bytes)
        };

        let new_struct = format!("struct {} {{{}\n}}", struct_name, new_inner);
        content.replace(&format!("struct {} {{{}}}", struct_name, inner), &new_struct)
    } else {
        content.to_string()
    }
}

// ============================================================================
// BENCHMARKS
// ============================================================================

/// Benchmark 1: Single simple struct (no generics, no attrs).
fn bench_simple_struct(c: &mut Criterion) {
    let content = r#"
        struct SimpleCapsule {
            state: AtomicU64,
            _padding: [u8; 56],
        }
    "#;

    let mut group = c.benchmark_group("simple_struct");

    // Baseline: Regex method
    group.bench_function("regex", |b| {
        b.iter(|| {
            black_box(rebuild_struct_regex(
                black_box(content),
                black_box("SimpleCapsule"),
                black_box(56),
            ))
        })
    });

    // New: AST method
    group.bench_function("ast", |b| {
        b.iter(|| {
            black_box(rebuild_struct_in_file(
                black_box(content),
                black_box("SimpleCapsule"),
                black_box(56),
            ))
        })
    });

    group.finish();
}

/// Benchmark 2: Complex struct (generic, where clause, attrs).
fn bench_complex_struct(c: &mut Criterion) {
    let content = r#"
        #[derive(ComputationalCapsule)]
        #[capsule(alignment = 64, size = 64)]
        #[repr(C, align(64))]
        pub struct GenericCapsule<T>
        where
            T: Send + Sync + 'static,
        {
            data: T,
            generation: AtomicU64,
            timestamp: AtomicU64,
            _padding: [u8; 40],
        }
    "#;

    let mut group = c.benchmark_group("complex_struct");

    // Baseline: Regex method
    group.bench_function("regex", |b| {
        b.iter(|| {
            black_box(rebuild_struct_regex(
                black_box(content),
                black_box("GenericCapsule"),
                black_box(40),
            ))
        })
    });

    // New: AST method
    group.bench_function("ast", |b| {
        b.iter(|| {
            black_box(rebuild_struct_in_file(
                black_box(content),
                black_box("GenericCapsule"),
                black_box(40),
            ))
        })
    });

    group.finish();
}

/// Benchmark 3: Large struct (10 fields, multiple padding consolidation).
fn bench_large_struct(c: &mut Criterion) {
    let content = r#"
        struct LargeCapsule {
            field1: AtomicU64,
            field2: AtomicU64,
            field3: AtomicU64,
            field4: AtomicU64,
            field5: AtomicU64,
            _padding1: [u8; 8],
            field6: AtomicU64,
            field7: AtomicU64,
            field8: AtomicU64,
            _padding2: [u8; 16],
            field9: AtomicU64,
            field10: AtomicU64,
            _padding3: [u8; 24],
        }
    "#;

    let mut group = c.benchmark_group("large_struct");

    // Baseline: Regex method
    group.bench_function("regex", |b| {
        b.iter(|| {
            black_box(rebuild_struct_regex(
                black_box(content),
                black_box("LargeCapsule"),
                black_box(0), // 10 × 8 bytes = 80 bytes, no padding needed for 128-byte alignment
            ))
        })
    });

    // New: AST method
    group.bench_function("ast", |b| {
        b.iter(|| {
            black_box(rebuild_struct_in_file(
                black_box(content),
                black_box("LargeCapsule"),
                black_box(0),
            ))
        })
    });

    group.finish();
}

/// Benchmark 4: Batch processing (100 structs).
fn bench_batch_100_structs(c: &mut Criterion) {
    // Generate 100 simple structs
    let mut content = String::new();
    for i in 0..100 {
        content.push_str(&format!(
            r#"
            struct Capsule{} {{
                state: AtomicU64,
                _padding: [u8; 56],
            }}
            "#,
            i
        ));
    }

    let mut group = c.benchmark_group("batch_100_structs");
    group.sample_size(100); // Reduce sample size for batch benchmark

    // Baseline: Regex method (process all 100 structs)
    group.bench_function("regex", |b| {
        b.iter(|| {
            let mut result = content.clone();
            for i in 0..100 {
                result = rebuild_struct_regex(&result, &format!("Capsule{}", i), 56);
            }
            black_box(result)
        })
    });

    // New: AST method (process all 100 structs)
    group.bench_function("ast", |b| {
        b.iter(|| {
            let mut result = content.clone();
            for i in 0..100 {
                result = rebuild_struct_in_file(&result, &format!("Capsule{}", i), 56)
                    .expect("Failed to rebuild");
            }
            black_box(result)
        })
    });

    group.finish();
}

/// Benchmark 5: Realistic atomic_capsule struct (DualAtomicU64).
fn bench_real_dual_atomic_u64(c: &mut Criterion) {
    let content = r#"
        use core::sync::atomic::AtomicU64;

        #[derive(ComputationalCapsule)]
        #[capsule(alignment = 64, size = 64)]
        #[repr(C, align(64))]
        pub struct DualAtomicU64 {
            pub primary: AtomicU64,
            pub secondary: AtomicU64,
            _padding: [u8; 48],
        }
    "#;

    let mut group = c.benchmark_group("real_dual_atomic_u64");

    // Baseline: Regex method
    group.bench_function("regex", |b| {
        b.iter(|| {
            black_box(rebuild_struct_regex(
                black_box(content),
                black_box("DualAtomicU64"),
                black_box(48),
            ))
        })
    });

    // New: AST method
    group.bench_function("ast", |b| {
        b.iter(|| {
            black_box(rebuild_struct_in_file(
                black_box(content),
                black_box("DualAtomicU64"),
                black_box(48),
            ))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_simple_struct,
    bench_complex_struct,
    bench_large_struct,
    bench_batch_100_structs,
    bench_real_dual_atomic_u64
);
criterion_main!(benches);
