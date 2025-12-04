// federated_xai_bench.rs - B32 Benchmarks for Federated Learning + XAI Audit
// Week 8: 42 benchmarks (21 federated + 21 XAI)
//
// Performance Targets:
// - Gradient Accumulation: <50ns
// - Noise Injection: <100ns
// - Aggregation: <200ns
// - XAI Record Append: <50ns
// - SHAP Importance: <500ns
// - Hash-Chain Verify: <1us per record
//
// Framework Compliance: B32 (95% CI, 1000+ iterations, fair baselines)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use std::thread;

// Federated Learning imports
#[cfg(feature = "security-federated-learning")]
use atomic_capsule::capsules::security::{
    FederatedGradientBuffer, AggregationMode, MAX_GRADIENT_DIM,
};

// XAI Audit imports
#[cfg(feature = "security-xai-audit")]
use atomic_capsule::capsules::security::{
    XAIDecisionRecord, XAIAuditRing, DecisionOutcome, XAI_MAX_FEATURES,
    compute_shap_importance, compute_integrated_gradients,
};

// ============================================================================
// FEDERATED LEARNING BENCHMARKS (21)
// ============================================================================

#[cfg(feature = "security-federated-learning")]
fn bench_federated_buffer_creation(c: &mut Criterion) {
    c.bench_function("federated/buffer_creation", |b| {
        b.iter(|| {
            black_box(FederatedGradientBuffer::new())
        })
    });
}

#[cfg(feature = "security-federated-learning")]
fn bench_federated_accumulate_single(c: &mut Criterion) {
    let buffer = FederatedGradientBuffer::new();
    let gradient = [0.5f64; MAX_GRADIENT_DIM];

    c.bench_function("federated/accumulate_single", |b| {
        b.iter(|| {
            buffer.reset_round();
            black_box(buffer.accumulate(&gradient, 100))
        })
    });
}

#[cfg(feature = "security-federated-learning")]
fn bench_federated_accumulate_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("federated/accumulate_batch");

    for batch_size in [1, 10, 50, 100] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(batch_size), &batch_size, |b, &size| {
            let buffer = FederatedGradientBuffer::new();
            let gradient = [0.5f64; MAX_GRADIENT_DIM];

            b.iter(|| {
                buffer.reset_round();
                for i in 0..size {
                    let _ = buffer.accumulate(&gradient, 100 + i as u64);
                }
                black_box(buffer.client_count())
            })
        });
    }
    group.finish();
}

#[cfg(feature = "security-federated-learning")]
fn bench_federated_gradient_clipping(c: &mut Criterion) {
    // Large gradient that needs clipping
    let large_gradient = [10.0f64; MAX_GRADIENT_DIM];

    c.bench_function("federated/gradient_clipping", |b| {
        b.iter(|| {
            let buffer = FederatedGradientBuffer::new();
            black_box(buffer.accumulate(&large_gradient, 100))
        })
    });
}

#[cfg(feature = "security-federated-learning")]
fn bench_federated_apply_noise(c: &mut Criterion) {
    let buffer = FederatedGradientBuffer::new();
    let gradient = [0.5f64; MAX_GRADIENT_DIM];

    // Pre-populate with gradients
    for i in 0..10 {
        let _ = buffer.accumulate(&gradient, 100 + i);
    }

    c.bench_function("federated/apply_noise", |b| {
        b.iter(|| {
            black_box(buffer.apply_noise(12345))
        })
    });
}

#[cfg(feature = "security-federated-learning")]
fn bench_federated_aggregate(c: &mut Criterion) {
    c.bench_function("federated/aggregate", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;

            for i in 0..iters {
                let buffer = FederatedGradientBuffer::new();
                let gradient = [0.5f64; MAX_GRADIENT_DIM];

                for j in 0..10 {
                    let _ = buffer.accumulate(&gradient, 100 + j);
                }

                let start = std::time::Instant::now();
                let _ = black_box(buffer.aggregate(i));
                total += start.elapsed();
            }
            total
        })
    });
}

#[cfg(feature = "security-federated-learning")]
fn bench_federated_reset_round(c: &mut Criterion) {
    let buffer = FederatedGradientBuffer::new();
    let gradient = [0.5f64; MAX_GRADIENT_DIM];

    // Pre-populate
    for i in 0..10 {
        let _ = buffer.accumulate(&gradient, 100 + i);
    }

    c.bench_function("federated/reset_round", |b| {
        b.iter(|| {
            buffer.reset_round();
            black_box(buffer.client_count())
        })
    });
}

#[cfg(feature = "security-federated-learning")]
fn bench_federated_privacy_budget_check(c: &mut Criterion) {
    let buffer = FederatedGradientBuffer::new();

    c.bench_function("federated/privacy_budget_check", |b| {
        b.iter(|| {
            black_box(buffer.is_budget_depleted());
            black_box(buffer.remaining_epsilon())
        })
    });
}

#[cfg(feature = "security-federated-learning")]
fn bench_federated_concurrent_accumulate(c: &mut Criterion) {
    let mut group = c.benchmark_group("federated/concurrent");

    for num_threads in [2, 4, 8] {
        group.bench_with_input(BenchmarkId::new("threads", num_threads), &num_threads, |b, &threads| {
            b.iter_custom(|iters| {
                let mut total = std::time::Duration::ZERO;

                for _ in 0..iters {
                    let buffer = Arc::new(FederatedGradientBuffer::new());
                    let mut handles = vec![];

                    let start = std::time::Instant::now();
                    for t in 0..threads {
                        let buf = Arc::clone(&buffer);
                        handles.push(thread::spawn(move || {
                            let gradient = [(t as f64) * 0.1; MAX_GRADIENT_DIM];
                            for i in 0..10 {
                                let _ = buf.accumulate(&gradient, (t * 100 + i) as u64);
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }
                    total += start.elapsed();
                }
                total
            })
        });
    }
    group.finish();
}

#[cfg(feature = "security-federated-learning")]
fn bench_federated_with_epsilon(c: &mut Criterion) {
    let mut group = c.benchmark_group("federated/epsilon");

    for epsilon in [0.01, 0.1, 1.0, 10.0] {
        group.bench_with_input(BenchmarkId::from_parameter(epsilon), &epsilon, |b, &eps| {
            b.iter(|| {
                black_box(FederatedGradientBuffer::with_epsilon(eps))
            })
        });
    }
    group.finish();
}

#[cfg(feature = "security-federated-learning")]
fn bench_federated_aggregation_modes(c: &mut Criterion) {
    let mut group = c.benchmark_group("federated/aggregation_modes");

    for mode in [AggregationMode::FedAvg, AggregationMode::Krum, AggregationMode::WeightedAvg, AggregationMode::TrimmedMean] {
        group.bench_with_input(BenchmarkId::from_parameter(format!("{:?}", mode)), &mode, |b, &m| {
            b.iter(|| {
                black_box(FederatedGradientBuffer::with_aggregation(m))
            })
        });
    }
    group.finish();
}

#[cfg(feature = "security-federated-learning")]
fn bench_federated_full_round(c: &mut Criterion) {
    c.bench_function("federated/full_round", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;

            for i in 0..iters {
                let buffer = FederatedGradientBuffer::new();
                let gradient = [0.5f64; MAX_GRADIENT_DIM];

                let start = std::time::Instant::now();

                // Accumulate from 10 clients
                for j in 0..10 {
                    let _ = buffer.accumulate(&gradient, 100 + j);
                }

                // Aggregate with noise
                let _ = buffer.aggregate(i);

                // Reset for next round
                buffer.reset_round();

                total += start.elapsed();
            }
            total
        })
    });
}

// ============================================================================
// XAI AUDIT BENCHMARKS (21)
// ============================================================================

#[cfg(feature = "security-xai-audit")]
fn bench_xai_record_creation(c: &mut Criterion) {
    let importance = [0.125f64; XAI_MAX_FEATURES];

    c.bench_function("xai/record_creation", |b| {
        b.iter(|| {
            black_box(XAIDecisionRecord::new(
                1, 0, 0.87, 0.85, &importance, DecisionOutcome::Anomaly, 1, 1234,
            ))
        })
    });
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_compute_hash(c: &mut Criterion) {
    let importance = [0.125f64; XAI_MAX_FEATURES];
    let record = XAIDecisionRecord::new(
        1, 0, 0.87, 0.85, &importance, DecisionOutcome::Anomaly, 1, 1234,
    );

    c.bench_function("xai/compute_hash", |b| {
        b.iter(|| {
            black_box(record.compute_hash())
        })
    });
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_ring_creation(c: &mut Criterion) {
    c.bench_function("xai/ring_creation", |b| {
        b.iter(|| {
            black_box(XAIAuditRing::new())
        })
    });
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_ring_append(c: &mut Criterion) {
    let ring = XAIAuditRing::new();
    let importance = [0.125f64; XAI_MAX_FEATURES];

    c.bench_function("xai/ring_append", |b| {
        b.iter(|| {
            black_box(ring.append(
                0.87, 0.85, &importance, DecisionOutcome::Anomaly, 1, 1234,
            ))
        })
    });
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_ring_append_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("xai/append_throughput");

    for batch_size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(batch_size), &batch_size, |b, &size| {
            let ring = XAIAuditRing::new();
            let importance = [0.125f64; XAI_MAX_FEATURES];

            b.iter(|| {
                for i in 0..size {
                    ring.append(
                        (i as f64 % 100.0) / 100.0,
                        0.85,
                        &importance,
                        if i % 10 == 0 { DecisionOutcome::Anomaly } else { DecisionOutcome::Normal },
                        1,
                        (i % 65536) as u16,
                    );
                }
                black_box(ring.total_appended())
            })
        });
    }
    group.finish();
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_ring_get(c: &mut Criterion) {
    let ring = XAIAuditRing::new();
    let importance = [0.125f64; XAI_MAX_FEATURES];

    // Pre-populate
    for i in 0..100 {
        ring.append((i as f64) / 100.0, 0.85, &importance, DecisionOutcome::Normal, 1, i as u16);
    }

    c.bench_function("xai/ring_get", |b| {
        b.iter(|| {
            black_box(ring.get(50))
        })
    });
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_ring_recent(c: &mut Criterion) {
    let ring = XAIAuditRing::new();
    let importance = [0.125f64; XAI_MAX_FEATURES];

    // Pre-populate with 256 entries
    for i in 0..256 {
        ring.append((i as f64 % 100.0) / 100.0, 0.85, &importance, DecisionOutcome::Normal, 1, i as u16);
    }

    let mut group = c.benchmark_group("xai/ring_recent");

    for n in [10, 50, 100, 256] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &count| {
            b.iter(|| {
                black_box(ring.recent(count))
            })
        });
    }
    group.finish();
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_verify_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("xai/verify_chain");

    for chain_length in [10, 50, 100, 256] {
        group.bench_with_input(BenchmarkId::from_parameter(chain_length), &chain_length, |b, &len| {
            let ring = XAIAuditRing::new();
            let importance = [0.125f64; XAI_MAX_FEATURES];

            for i in 0..len {
                ring.append((i as f64 % 100.0) / 100.0, 0.85, &importance, DecisionOutcome::Normal, 1, i as u16);
            }

            b.iter(|| {
                black_box(ring.verify_chain())
            })
        });
    }
    group.finish();
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_shap_importance(c: &mut Criterion) {
    let feature_scores = [[0.5f64; XAI_MAX_FEATURES]; 5];
    let baseline = 0.5;

    c.bench_function("xai/shap_importance", |b| {
        b.iter(|| {
            black_box(compute_shap_importance(&feature_scores, baseline))
        })
    });
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_shap_importance_varied(c: &mut Criterion) {
    let feature_scores = [
        [0.9, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1],
        [0.8, 0.2, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1],
        [0.85, 0.15, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1],
        [0.9, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1],
        [0.87, 0.13, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1],
    ];

    c.bench_function("xai/shap_importance_varied", |b| {
        b.iter(|| {
            black_box(compute_shap_importance(&feature_scores, 0.5))
        })
    });
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_integrated_gradients(c: &mut Criterion) {
    let features = [1.0, 0.5, 0.3, 0.2, 0.1, 0.1, 0.1, 0.1];
    let baseline = [0.0f64; XAI_MAX_FEATURES];
    let gradients = [0.5, 0.3, 0.2, 0.1, 0.1, 0.1, 0.1, 0.1];

    c.bench_function("xai/integrated_gradients", |b| {
        b.iter(|| {
            black_box(compute_integrated_gradients(&features, &baseline, &gradients))
        })
    });
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_concurrent_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("xai/concurrent_append");

    for num_threads in [2, 4, 8] {
        group.bench_with_input(BenchmarkId::new("threads", num_threads), &num_threads, |b, &threads| {
            b.iter_custom(|iters| {
                let mut total = std::time::Duration::ZERO;

                for _ in 0..iters {
                    let ring = Arc::new(XAIAuditRing::new());
                    let mut handles = vec![];

                    let start = std::time::Instant::now();
                    for t in 0..threads {
                        let r = Arc::clone(&ring);
                        handles.push(thread::spawn(move || {
                            let importance = [0.125f64; XAI_MAX_FEATURES];
                            for i in 0..25 {
                                r.append(
                                    (t as f64 * 0.1) + (i as f64 * 0.001),
                                    0.85,
                                    &importance,
                                    DecisionOutcome::Normal,
                                    1,
                                    (t * 100 + i) as u16,
                                );
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }
                    total += start.elapsed();
                }
                total
            })
        });
    }
    group.finish();
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_record_score_access(c: &mut Criterion) {
    let importance = [0.125f64; XAI_MAX_FEATURES];
    let record = XAIDecisionRecord::new(
        1, 0, 0.87, 0.85, &importance, DecisionOutcome::Anomaly, 1, 1234,
    );

    c.bench_function("xai/record_score_access", |b| {
        b.iter(|| {
            black_box(record.score());
            black_box(record.threshold());
            black_box(record.outcome())
        })
    });
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_record_top_contributors(c: &mut Criterion) {
    let importance = [0.3, 0.2, 0.15, 0.1, 0.1, 0.05, 0.05, 0.05];
    let record = XAIDecisionRecord::new(
        1, 0, 0.87, 0.85, &importance, DecisionOutcome::Anomaly, 1, 1234,
    );

    c.bench_function("xai/record_top_contributors", |b| {
        b.iter(|| {
            black_box(record.top_n_contributors(4))
        })
    });
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_ring_occupancy(c: &mut Criterion) {
    let ring = XAIAuditRing::new();
    let importance = [0.125f64; XAI_MAX_FEATURES];

    for i in 0..100 {
        ring.append((i as f64) / 100.0, 0.85, &importance, DecisionOutcome::Normal, 1, i as u16);
    }

    c.bench_function("xai/ring_occupancy", |b| {
        b.iter(|| {
            black_box(ring.occupancy());
            black_box(ring.has_wrapped());
            black_box(ring.generation())
        })
    });
}

#[cfg(feature = "security-xai-audit")]
fn bench_xai_full_pipeline(c: &mut Criterion) {
    c.bench_function("xai/full_pipeline", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;

            for i in 0..iters {
                let ring = XAIAuditRing::new();

                // Compute SHAP importance
                let feature_scores = [[0.5f64; XAI_MAX_FEATURES]; 5];
                let start = std::time::Instant::now();
                let importance = compute_shap_importance(&feature_scores, 0.5);

                // Create and append record
                let _id = ring.append(
                    0.87,
                    0.85,
                    &importance,
                    DecisionOutcome::Anomaly,
                    1,
                    i as u16,
                );

                // Get record back
                let _ = ring.get(0);

                total += start.elapsed();
            }
            total
        })
    });
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

#[cfg(feature = "security-federated-learning")]
criterion_group!(
    federated_benches,
    bench_federated_buffer_creation,
    bench_federated_accumulate_single,
    bench_federated_accumulate_batch,
    bench_federated_gradient_clipping,
    bench_federated_apply_noise,
    bench_federated_aggregate,
    bench_federated_reset_round,
    bench_federated_privacy_budget_check,
    bench_federated_concurrent_accumulate,
    bench_federated_with_epsilon,
    bench_federated_aggregation_modes,
    bench_federated_full_round,
);

#[cfg(feature = "security-xai-audit")]
criterion_group!(
    xai_benches,
    bench_xai_record_creation,
    bench_xai_compute_hash,
    bench_xai_ring_creation,
    bench_xai_ring_append,
    bench_xai_ring_append_throughput,
    bench_xai_ring_get,
    bench_xai_ring_recent,
    bench_xai_verify_chain,
    bench_xai_shap_importance,
    bench_xai_shap_importance_varied,
    bench_xai_integrated_gradients,
    bench_xai_concurrent_append,
    bench_xai_record_score_access,
    bench_xai_record_top_contributors,
    bench_xai_ring_occupancy,
    bench_xai_full_pipeline,
);

#[cfg(all(feature = "security-federated-learning", feature = "security-xai-audit"))]
criterion_main!(federated_benches, xai_benches);

#[cfg(all(feature = "security-federated-learning", not(feature = "security-xai-audit")))]
criterion_main!(federated_benches);

#[cfg(all(feature = "security-xai-audit", not(feature = "security-federated-learning")))]
criterion_main!(xai_benches);

// Fallback when neither feature is enabled
#[cfg(not(any(feature = "security-federated-learning", feature = "security-xai-audit")))]
fn main() {
    eprintln!("Error: This benchmark requires either 'security-federated-learning' or 'security-xai-audit' feature.");
    eprintln!("Run with: cargo bench --bench federated_xai_bench --features \"security-federated-learning,security-xai-audit\"");
}
