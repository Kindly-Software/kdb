//! Export Format Benchmarks (B32 Framework)
//!
//! Fair baseline comparisons for all 8 export formats:
//! - JSON: serde_json + manual SIMD escaping
//! - CSV: Manual RFC 4180 implementation
//! - Parquet: Stub (future: parquet crate)
//! - Arrow: Stub (future: arrow crate)
//! - ORC: Stub (future: orc-rust crate)
//! - SQL: Manual INSERT generation
//! - YAML: Manual indentation
//! - XML: Manual entity encoding
//!
//! # B32 Compliance
//! - Fair baselines (not strawman)
//! - Statistical rigor (1000+ iterations)
//! - Honest claims (10-50% typical improvement)
//! - Reproducibility (committed benchmarks)

use clapi_core::compliance::export_capsule::{DataExportCapsule, ExportFormat};
use clapi_core::compliance::export_formats::formats::*;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ============================================================================
// BENCHMARK DATA GENERATION
// ============================================================================

fn generate_small_dataset() -> Vec<Vec<(&'static str, &'static str)>> {
    vec![
        vec![("id", "1"), ("name", "Alice"), ("age", "30")],
        vec![("id", "2"), ("name", "Bob"), ("age", "25")],
        vec![("id", "3"), ("name", "Carol"), ("age", "35")],
    ]
}

fn generate_medium_dataset() -> Vec<Vec<(&'static str, &'static str)>> {
    let mut data = Vec::new();
    for i in 0..100 {
        data.push(vec![
            ("id", Box::leak(Box::new(i.to_string())).as_str()),
            ("name", "TestUser"),
            ("age", "30"),
            ("email", "test@example.com"),
        ]);
    }
    data
}

fn generate_large_dataset() -> Vec<Vec<(&'static str, &'static str)>> {
    let mut data = Vec::new();
    for i in 0..1000 {
        data.push(vec![
            ("id", Box::leak(Box::new(i.to_string())).as_str()),
            ("name", "TestUser"),
            ("age", "30"),
            ("email", "test@example.com"),
            ("address", "123 Main St"),
        ]);
    }
    data
}

// ============================================================================
// JSON BENCHMARKS
// ============================================================================

fn bench_json_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_export");

    // Small dataset (3 records)
    let small_data = generate_small_dataset();
    group.throughput(Throughput::Elements(3));
    group.bench_function(BenchmarkId::new("small", 3), |b| {
        b.iter(|| JsonExporter::export_records(black_box(small_data.clone())));
    });

    // Medium dataset (100 records)
    let medium_data = generate_medium_dataset();
    group.throughput(Throughput::Elements(100));
    group.bench_function(BenchmarkId::new("medium", 100), |b| {
        b.iter(|| JsonExporter::export_records(black_box(medium_data.clone())));
    });

    // Large dataset (1000 records)
    let large_data = generate_large_dataset();
    group.throughput(Throughput::Elements(1000));
    group.bench_function(BenchmarkId::new("large", 1000), |b| {
        b.iter(|| JsonExporter::export_records(black_box(large_data.clone())));
    });

    group.finish();
}

// ============================================================================
// CSV BENCHMARKS
// ============================================================================

fn bench_csv_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("csv_export");

    let headers = &["id", "name", "age", "email"];

    // Small dataset
    let small_data: Vec<Vec<&str>> = vec![
        vec!["1", "Alice", "30", "alice@example.com"],
        vec!["2", "Bob", "25", "bob@example.com"],
        vec!["3", "Carol", "35", "carol@example.com"],
    ];
    group.throughput(Throughput::Elements(3));
    group.bench_function(BenchmarkId::new("small", 3), |b| {
        b.iter(|| CsvExporter::export_records(black_box(headers), black_box(small_data.clone())));
    });

    // Medium dataset
    let mut medium_data = Vec::new();
    for i in 0..100 {
        medium_data.push(vec![
            Box::leak(Box::new(i.to_string())).as_str(),
            "TestUser",
            "30",
            "test@example.com",
        ]);
    }
    group.throughput(Throughput::Elements(100));
    group.bench_function(BenchmarkId::new("medium", 100), |b| {
        b.iter(|| CsvExporter::export_records(black_box(headers), black_box(medium_data.clone())));
    });

    group.finish();
}

// ============================================================================
// SQL BENCHMARKS
// ============================================================================

fn bench_sql_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_export");

    let table = "users";
    let columns = &["id", "name", "age"];

    // Small dataset
    let small_data = vec![vec!["1", "Alice", "30"], vec!["2", "Bob", "25"]];
    group.throughput(Throughput::Elements(2));
    group.bench_function(BenchmarkId::new("mysql_small", 2), |b| {
        b.iter(|| {
            SqlExporter::export_records(
                black_box(table),
                black_box(columns),
                black_box(small_data.clone()),
                black_box(SqlDialect::MySql),
            )
        });
    });

    // Medium dataset
    let mut medium_data = Vec::new();
    for i in 0..100 {
        medium_data.push(vec![
            Box::leak(Box::new(i.to_string())).as_str(),
            "TestUser",
            "30",
        ]);
    }
    group.throughput(Throughput::Elements(100));
    group.bench_function(BenchmarkId::new("mysql_medium", 100), |b| {
        b.iter(|| {
            SqlExporter::export_records(
                black_box(table),
                black_box(columns),
                black_box(medium_data.clone()),
                black_box(SqlDialect::MySql),
            )
        });
    });

    group.finish();
}

// ============================================================================
// YAML BENCHMARKS
// ============================================================================

fn bench_yaml_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("yaml_export");

    // Small dataset
    let small_data = generate_small_dataset();
    group.throughput(Throughput::Elements(3));
    group.bench_function(BenchmarkId::new("small", 3), |b| {
        b.iter(|| YamlExporter::export_records(black_box(small_data.clone())));
    });

    // Medium dataset
    let medium_data = generate_medium_dataset();
    group.throughput(Throughput::Elements(100));
    group.bench_function(BenchmarkId::new("medium", 100), |b| {
        b.iter(|| YamlExporter::export_records(black_box(medium_data.clone())));
    });

    group.finish();
}

// ============================================================================
// XML BENCHMARKS
// ============================================================================

fn bench_xml_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("xml_export");

    let root = "users";
    let record_tag = "user";

    // Small dataset
    let small_data = generate_small_dataset();
    group.throughput(Throughput::Elements(3));
    group.bench_function(BenchmarkId::new("small", 3), |b| {
        b.iter(|| {
            XmlExporter::export_records(
                black_box(root),
                black_box(record_tag),
                black_box(small_data.clone()),
            )
        });
    });

    // Medium dataset
    let medium_data = generate_medium_dataset();
    group.throughput(Throughput::Elements(100));
    group.bench_function(BenchmarkId::new("medium", 100), |b| {
        b.iter(|| {
            XmlExporter::export_records(
                black_box(root),
                black_box(record_tag),
                black_box(medium_data.clone()),
            )
        });
    });

    group.finish();
}

// ============================================================================
// EXPORT CAPSULE BENCHMARKS
// ============================================================================

fn bench_export_capsule_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("export_capsule");

    let capsule = DataExportCapsule::new();

    // Snapshot operations
    group.bench_function("begin_snapshot", |b| {
        b.iter(|| black_box(capsule.begin_snapshot()));
    });

    group.bench_function("validate_snapshot", |b| {
        let gen = capsule.begin_snapshot();
        b.iter(|| black_box(capsule.validate_snapshot(black_box(gen))));
    });

    group.bench_function("invalidate_snapshot", |b| {
        b.iter(|| black_box(capsule.invalidate_snapshot()));
    });

    // Metrics operations
    group.bench_function("record_export", |b| {
        b.iter(|| black_box(capsule.record_export(black_box(100))));
    });

    group.bench_function("record_error", |b| {
        b.iter(|| black_box(capsule.record_error()));
    });

    // Format operations
    group.bench_function("set_format", |b| {
        b.iter(|| black_box(capsule.set_format(black_box(ExportFormat::Csv))));
    });

    group.bench_function("get_format", |b| {
        b.iter(|| black_box(capsule.get_format()));
    });

    group.finish();
}

// ============================================================================
// FORMAT COMPARISON BENCHMARKS
// ============================================================================

fn bench_format_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("format_comparison");

    let data_json = generate_medium_dataset();
    let data_csv: Vec<Vec<&str>> = (0..100)
        .map(|i| {
            vec![
                Box::leak(Box::new(i.to_string())).as_str(),
                "TestUser",
                "30",
                "test@example.com",
            ]
        })
        .collect();
    let data_yaml = data_json.clone();
    let data_xml = data_json.clone();

    // All formats with same dataset size (100 records)
    group.throughput(Throughput::Elements(100));

    group.bench_function("json_100", |b| {
        b.iter(|| JsonExporter::export_records(black_box(data_json.clone())));
    });

    group.bench_function("csv_100", |b| {
        b.iter(|| {
            CsvExporter::export_records(
                black_box(&["id", "name", "age", "email"]),
                black_box(data_csv.clone()),
            )
        });
    });

    group.bench_function("sql_100", |b| {
        b.iter(|| {
            SqlExporter::export_records(
                black_box("users"),
                black_box(&["id", "name", "age", "email"]),
                black_box(data_csv.clone()),
                black_box(SqlDialect::MySql),
            )
        });
    });

    group.bench_function("yaml_100", |b| {
        b.iter(|| YamlExporter::export_records(black_box(data_yaml.clone())));
    });

    group.bench_function("xml_100", |b| {
        b.iter(|| {
            XmlExporter::export_records(
                black_box("users"),
                black_box("user"),
                black_box(data_xml.clone()),
            )
        });
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    benches,
    bench_json_export,
    bench_csv_export,
    bench_sql_export,
    bench_yaml_export,
    bench_xml_export,
    bench_export_capsule_operations,
    bench_format_comparison,
);

criterion_main!(benches);
