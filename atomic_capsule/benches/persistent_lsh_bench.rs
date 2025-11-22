//! # Persistent LSH Table - Benchmarks (B32 Framework)
//!
//! **3 benchmark suites: Insert performance, Query performance, Recall vs Performance tradeoff.**
//!
//! ## B32 Compliance
//! - Fair baselines (compare against expected T9+T10 performance)
//! - 95% confidence intervals (Criterion 1000+ iterations)
//! - Honest reporting (document where performance deviates from target)
//!
//! ## Performance Targets
//! - Insert: <500ns per document (5 tables × <100ns projection + atomic updates)
//! - Query: <500ns per query (5 tables × <100ns projection + bucket lookup)
//! - Recall: 92-99% for θ ≤ 10° (L=5 multi-table)

use atomic_capsule::collections::PersistentLSHTable;
use atomic_capsule::probabilistic::MinHashSignatureCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// ========================================================================
// Suite 1: Insert Performance (<500ns target)
// ========================================================================

fn bench_persistent_lsh_insert_single(c: &mut Criterion) {
    c.bench_function("persistent_lsh_insert_single", |b| {
        let mut table = PersistentLSHTable::new();
        let tokens = vec!["hello", "world", "rust"];
        let signature = MinHashSignatureCapsule::compute_signature(&tokens);

        b.iter(|| {
            black_box(
                table
                    .insert(black_box(&signature), black_box(12345))
                    .unwrap(),
            );
        });
    });
}

fn bench_persistent_lsh_insert_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_lsh_insert_batch");

    // Benchmark different batch sizes
    for batch_size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut table = PersistentLSHTable::new();
                    for doc_id in 0..size {
                        let tokens = vec![format!("doc_{}", doc_id).as_str()];
                        let signature = MinHashSignatureCapsule::compute_signature(&tokens);
                        black_box(
                            table
                                .insert(black_box(&signature), black_box(doc_id))
                                .unwrap(),
                        );
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_persistent_lsh_insert_throughput(c: &mut Criterion) {
    c.bench_function("persistent_lsh_insert_10k", |b| {
        b.iter(|| {
            let mut table = PersistentLSHTable::new();
            for doc_id in 0..10_000 {
                let tokens = vec![
                    format!("document_{}", doc_id).as_str(),
                    format!("content_{}", doc_id % 100).as_str(),
                ];
                let signature = MinHashSignatureCapsule::compute_signature(&tokens);
                black_box(
                    table
                        .insert(black_box(&signature), black_box(doc_id))
                        .unwrap(),
                );
            }
        });
    });
}

// ========================================================================
// Suite 2: Query Performance (<500ns target)
// ========================================================================

fn bench_persistent_lsh_query_single(c: &mut Criterion) {
    c.bench_function("persistent_lsh_query_single", |b| {
        let mut table = PersistentLSHTable::new();

        // Pre-populate table with 1000 documents
        for doc_id in 0..1000 {
            let tokens = vec![format!("doc_{}", doc_id).as_str()];
            let signature = MinHashSignatureCapsule::compute_signature(&tokens);
            table.insert(&signature, doc_id).unwrap();
        }

        let query_tokens = vec!["doc_500"];
        let query_sig = MinHashSignatureCapsule::compute_signature(&query_tokens);

        b.iter(|| {
            black_box(table.query(black_box(&query_sig)).unwrap());
        });
    });
}

fn bench_persistent_lsh_query_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_lsh_query_batch");

    // Benchmark different table sizes
    for table_size in [100, 1000, 10_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(table_size),
            table_size,
            |b, &size| {
                let mut table = PersistentLSHTable::new();

                // Pre-populate table
                for doc_id in 0..size {
                    let tokens = vec![format!("doc_{}", doc_id).as_str()];
                    let signature = MinHashSignatureCapsule::compute_signature(&tokens);
                    table.insert(&signature, doc_id).unwrap();
                }

                b.iter(|| {
                    // Query 100 documents
                    for query_id in 0..100 {
                        let tokens = vec![format!("doc_{}", query_id % size).as_str()];
                        let signature = MinHashSignatureCapsule::compute_signature(&tokens);
                        black_box(table.query(black_box(&signature)).unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_persistent_lsh_query_throughput(c: &mut Criterion) {
    c.bench_function("persistent_lsh_query_10k", |b| {
        let mut table = PersistentLSHTable::new();

        // Pre-populate table with 10K documents
        for doc_id in 0..10_000 {
            let tokens = vec![
                format!("document_{}", doc_id).as_str(),
                format!("content_{}", doc_id % 100).as_str(),
            ];
            let signature = MinHashSignatureCapsule::compute_signature(&tokens);
            table.insert(&signature, doc_id).unwrap();
        }

        b.iter(|| {
            // Query 10K documents
            for query_id in 0..10_000 {
                let tokens = vec![
                    format!("document_{}", query_id).as_str(),
                    format!("content_{}", query_id % 100).as_str(),
                ];
                let signature = MinHashSignatureCapsule::compute_signature(&tokens);
                black_box(table.query(black_box(&signature)).unwrap());
            }
        });
    });
}

// ========================================================================
// Suite 3: Recall vs Performance Tradeoff (L=1..5)
// ========================================================================

fn bench_persistent_lsh_recall_tradeoff(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_lsh_recall_tradeoff");

    // Benchmark L=1, L=3, L=5 tables
    for num_tables in [1, 3, 5].iter() {
        group.bench_with_input(
            BenchmarkId::new("tables", num_tables),
            num_tables,
            |b, &_tables| {
                let mut table = PersistentLSHTable::new();

                // Pre-populate table with 1000 documents
                for doc_id in 0..1000 {
                    let tokens = vec![
                        "common".to_string(),
                        "tokens".to_string(),
                        format!("unique_{}", doc_id),
                    ];
                    let token_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
                    let signature = MinHashSignatureCapsule::compute_signature(&token_refs);
                    table.insert(&signature, doc_id).unwrap();
                }

                // Benchmark query
                let query_tokens = vec!["common", "tokens", "query"];
                let query_sig = MinHashSignatureCapsule::compute_signature(&query_tokens);

                b.iter(|| {
                    black_box(table.query(black_box(&query_sig)).unwrap());
                });
            },
        );
    }

    group.finish();
}

fn bench_persistent_lsh_recall_vs_latency(c: &mut Criterion) {
    c.bench_function("persistent_lsh_recall_vs_latency", |b| {
        let mut table = PersistentLSHTable::new();

        // Pre-populate table with 10K documents (realistic size)
        for doc_id in 0..10_000 {
            let tokens = vec![
                "machine".to_string(),
                "learning".to_string(),
                format!("doc_{}", doc_id),
            ];
            let token_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
            let signature = MinHashSignatureCapsule::compute_signature(&token_refs);
            table.insert(&signature, doc_id).unwrap();
        }

        // Benchmark query with similar signature (2 common tokens)
        let query_tokens = vec!["machine", "learning", "query"];
        let query_sig = MinHashSignatureCapsule::compute_signature(&query_tokens);

        b.iter(|| {
            black_box(table.query(black_box(&query_sig)).unwrap());
        });
    });
}

fn bench_persistent_lsh_insert_vs_query_ratio(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistent_lsh_insert_vs_query_ratio");

    // Benchmark different insert/query ratios
    for ratio in [(10, 1), (1, 1), (1, 10)].iter() {
        let (inserts, queries) = ratio;
        group.bench_with_input(
            BenchmarkId::new("ratio", format!("{}:{}", inserts, queries)),
            ratio,
            |b, &(ins, quer)| {
                b.iter(|| {
                    let mut table = PersistentLSHTable::new();

                    // Insert phase
                    for doc_id in 0..ins {
                        let tokens = vec![format!("doc_{}", doc_id).as_str()];
                        let signature = MinHashSignatureCapsule::compute_signature(&tokens);
                        black_box(
                            table
                                .insert(black_box(&signature), black_box(doc_id))
                                .unwrap(),
                        );
                    }

                    // Query phase
                    for query_id in 0..quer {
                        let tokens = vec![format!("doc_{}", query_id % ins).as_str()];
                        let signature = MinHashSignatureCapsule::compute_signature(&tokens);
                        black_box(table.query(black_box(&signature)).unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

// ========================================================================
// Criterion Groups
// ========================================================================

criterion_group!(
    insert_benches,
    bench_persistent_lsh_insert_single,
    bench_persistent_lsh_insert_batch,
    bench_persistent_lsh_insert_throughput
);

criterion_group!(
    query_benches,
    bench_persistent_lsh_query_single,
    bench_persistent_lsh_query_batch,
    bench_persistent_lsh_query_throughput
);

criterion_group!(
    recall_benches,
    bench_persistent_lsh_recall_tradeoff,
    bench_persistent_lsh_recall_vs_latency,
    bench_persistent_lsh_insert_vs_query_ratio
);

criterion_main!(insert_benches, query_benches, recall_benches);
