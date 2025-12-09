//! Compliance Endpoint Benchmarks - B32 Framework
//!
//! Validates performance of compliance export operations with:
//! - Fair baselines (honest reporting)
//! - Statistical rigor (1000+ iterations, 95% CI)
//! - Realistic workloads (100-1000 entry datasets)
//!
//! # Performance Targets
//! - Export metadata preparation: <1μs
//! - JSON serialization: <100μs per entry
//! - CSV serialization: <50μs per entry
//! - Hash chain verification: <10ns per link

use clapi_core::compliance::compliance_capsules::now_ns;
use clapi_core::compliance::{
    ComplianceCapsule256, ComplianceEntry, ComplianceFramework, CsvExporter, GdprExporter,
    JsonExporter, Soc2Exporter, SoxExporter,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

// ============================================================================
// Benchmark Helpers
// ============================================================================

fn create_sox_entry(
    gl_code: &str,
    amount_cents: i64,
    hash: u64,
    prev_hash: u64,
) -> ComplianceEntry {
    ComplianceEntry {
        framework: ComplianceFramework::Sox404,
        operation: format!("Transaction - GL {}", gl_code),
        timestamp_ns: now_ns(),
        hash,
        prev_hash,
        metadata: vec![
            ("gl_code".to_string(), gl_code.to_string()),
            ("approver".to_string(), "test@company.com".to_string()),
            ("fiscal_year".to_string(), "2025".to_string()),
            ("amount_cents".to_string(), amount_cents.to_string()),
        ],
    }
}

fn create_soc2_entry(change_ticket: &str, ts: u64, hash: u64, prev_hash: u64) -> ComplianceEntry {
    ComplianceEntry {
        framework: ComplianceFramework::Soc2TypeII,
        operation: format!("Change {}", change_ticket),
        timestamp_ns: ts,
        hash,
        prev_hash,
        metadata: vec![
            ("change_ticket".to_string(), change_ticket.to_string()),
            ("approved_by".to_string(), "ops@company.com".to_string()),
            ("approval_timestamp".to_string(), ts.to_string()),
        ],
    }
}

fn create_gdpr_entry(user_id: &str, hash: u64, prev_hash: u64) -> ComplianceEntry {
    ComplianceEntry {
        framework: ComplianceFramework::GdprArticle30,
        operation: format!("GDPR: User {} access", user_id),
        timestamp_ns: now_ns(),
        hash,
        prev_hash,
        metadata: vec![
            ("user_id".to_string(), user_id.to_string()),
            ("gdpr_article".to_string(), "15".to_string()),
            ("access_type".to_string(), "read".to_string()),
            ("accessor".to_string(), "api_service".to_string()),
        ],
    }
}

fn generate_sox_entries(count: usize) -> Vec<ComplianceEntry> {
    (0..count)
        .map(|i| {
            let hash = 0x1000 + i as u64;
            let prev_hash = if i == 0 { 0 } else { 0x1000 + (i - 1) as u64 };
            create_sox_entry("4100", 100_00, hash, prev_hash)
        })
        .collect()
}

fn generate_soc2_entries(count: usize) -> Vec<ComplianceEntry> {
    let base_ts = now_ns();
    (0..count)
        .map(|i| {
            let ts = base_ts + (i as u64 * 1_000_000); // 1ms intervals
            let hash = 0x2000 + i as u64;
            let prev_hash = if i == 0 { 0 } else { 0x2000 + (i - 1) as u64 };
            create_soc2_entry(&format!("CHG-{:04}", i), ts, hash, prev_hash)
        })
        .collect()
}

fn generate_gdpr_entries(count: usize) -> Vec<ComplianceEntry> {
    (0..count)
        .map(|i| {
            let user_id = format!("user_{}", i % 100); // 100 unique users
            let hash = 0x3000 + i as u64;
            let prev_hash = if i == 0 { 0 } else { 0x3000 + (i - 1) as u64 };
            create_gdpr_entry(&user_id, hash, prev_hash)
        })
        .collect()
}

// ============================================================================
// Compliance Capsule Benchmarks
// ============================================================================

fn bench_compliance_capsule_record_entry(c: &mut Criterion) {
    let capsule = ComplianceCapsule256::new();
    let ts = now_ns();

    c.bench_function("compliance_capsule_record_entry", |b| {
        b.iter(|| {
            capsule.record_entry(
                black_box(ComplianceFramework::Sox404),
                black_box(0x1234),
                black_box(ts),
            );
        });
    });
}

fn bench_compliance_capsule_metrics(c: &mut Criterion) {
    let capsule = ComplianceCapsule256::new();
    let ts = now_ns();

    // Pre-populate with entries
    for i in 0..100 {
        capsule.record_entry(ComplianceFramework::Sox404, 0x1000 + i, ts + i);
    }

    c.bench_function("compliance_capsule_metrics", |b| {
        b.iter(|| {
            black_box(capsule.metrics());
        });
    });
}

// ============================================================================
// SOX Export Benchmarks
// ============================================================================

fn bench_sox_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("sox_export");

    for size in [10, 100, 1000].iter() {
        let entries = generate_sox_entries(*size);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let report = SoxExporter::export(black_box(&entries), None).unwrap();
                black_box(report);
            });
        });
    }

    group.finish();
}

fn bench_sox_json_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("sox_json_export");

    for size in [10, 100, 1000].iter() {
        let entries = generate_sox_entries(*size);
        let report = SoxExporter::export(&entries, None).unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let json = JsonExporter::export_sox(black_box(&report)).unwrap();
                black_box(json);
            });
        });
    }

    group.finish();
}

fn bench_sox_csv_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("sox_csv_export");

    for size in [10, 100, 1000].iter() {
        let entries = generate_sox_entries(*size);
        let report = SoxExporter::export(&entries, None).unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let csv = CsvExporter::export_sox(black_box(&report)).unwrap();
                black_box(csv);
            });
        });
    }

    group.finish();
}

// ============================================================================
// SOC2 Export Benchmarks
// ============================================================================

fn bench_soc2_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("soc2_export");
    let base_ts = now_ns();
    let observation_start = base_ts;
    let observation_end = base_ts + 1_000_000_000_000;

    for size in [10, 100, 1000].iter() {
        let entries = generate_soc2_entries(*size);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let report = Soc2Exporter::export(
                    black_box(&entries),
                    black_box(observation_start),
                    black_box(observation_end),
                )
                .unwrap();
                black_box(report);
            });
        });
    }

    group.finish();
}

fn bench_soc2_json_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("soc2_json_export");
    let base_ts = now_ns();
    let observation_start = base_ts;
    let observation_end = base_ts + 1_000_000_000_000;

    for size in [10, 100, 1000].iter() {
        let entries = generate_soc2_entries(*size);
        let report = Soc2Exporter::export(&entries, observation_start, observation_end).unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let json = JsonExporter::export_soc2(black_box(&report)).unwrap();
                black_box(json);
            });
        });
    }

    group.finish();
}

// ============================================================================
// GDPR Export Benchmarks
// ============================================================================

fn bench_gdpr_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("gdpr_export");

    for size in [10, 100, 1000].iter() {
        let entries = generate_gdpr_entries(*size);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let report = GdprExporter::export(black_box(&entries), None).unwrap();
                black_box(report);
            });
        });
    }

    group.finish();
}

fn bench_gdpr_json_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("gdpr_json_export");

    for size in [10, 100, 1000].iter() {
        let entries = generate_gdpr_entries(*size);
        let report = GdprExporter::export(&entries, None).unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let json = JsonExporter::export_gdpr(black_box(&report)).unwrap();
                black_box(json);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = compliance_benches;
    config = Criterion::default()
        .sample_size(1000)  // 1000+ iterations for statistical rigor
        .measurement_time(Duration::from_secs(10))  // 10 seconds per benchmark
        .confidence_level(0.95);  // 95% CI
    targets =
        bench_compliance_capsule_record_entry,
        bench_compliance_capsule_metrics,
        bench_sox_export,
        bench_sox_json_export,
        bench_sox_csv_export,
        bench_soc2_export,
        bench_soc2_json_export,
        bench_gdpr_export,
        bench_gdpr_json_export,
}

criterion_main!(compliance_benches);
