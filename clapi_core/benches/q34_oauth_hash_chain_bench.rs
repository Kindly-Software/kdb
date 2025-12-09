//! Q34 OAuth Hash Chain Benchmarks (B32 Framework)
//!
//! **Q34 Compliance**: Measure hash chain performance overhead
//! **Framework**: B32 (Honest benchmarking with statistical rigor)
//! **Purpose**: Validate <100ns hash chain update/verification overhead
//!
//! # Benchmark Categories
//! 1. **Hash Chain Updates**: update_hash_chain() on state transitions
//! 2. **Hash Chain Verification**: verify_chain() for audit trails
//! 3. **Overhead Analysis**: With vs without hash chain (comparative)
//! 4. **Concurrent Performance**: Multi-threaded hash chain operations
//!
//! # Performance Targets (Q34 Requirements)
//! - update_hash_chain(): <50ns (7 atomic loads + XOR)
//! - verify_chain(): <100ns (recompute + compare)
//! - Hash overhead: <10% of state transition latency
//! - Concurrent throughput: >5M verifications/sec (8 threads)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use clapi_core::capsules::OAuthSessionCapsule;
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q34 Hash Chain Update Benchmarks
// ============================================================================

fn bench_q34_hash_update_on_revoke(c: &mut Criterion) {
    // Q34-B1: Measure hash chain update overhead during revoke
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    c.bench_function("q34_hash_update_on_revoke", |b| {
        b.iter(|| {
            session.revoke();
        });
    });
}

fn bench_q34_hash_update_on_expire(c: &mut Criterion) {
    // Q34-B2: Measure hash chain update overhead during expire
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    c.bench_function("q34_hash_update_on_expire", |b| {
        b.iter(|| {
            session.mark_expired();
        });
    });
}

fn bench_q34_hash_update_on_refresh(c: &mut Criterion) {
    // Q34-B3: Measure hash chain update overhead during refresh
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    c.bench_function("q34_hash_update_on_refresh", |b| {
        b.iter(|| {
            session.refresh(None);
        });
    });
}

// ============================================================================
// Q34 Hash Chain Verification Benchmarks
// ============================================================================

fn bench_q34_verify_chain_single_thread(c: &mut Criterion) {
    // Q34-B4: Single-threaded hash chain verification latency
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    // Perform some state transitions to create hash chain
    session.refresh(None);
    session.refresh(None);
    session.refresh(None);

    c.bench_function("q34_verify_chain_single", |b| {
        b.iter(|| {
            black_box(session.verify_chain())
        });
    });
}

fn bench_q34_verify_chain_after_state_transition(c: &mut Criterion) {
    // Q34-B5: Verify chain immediately after state transition
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    c.bench_function("q34_verify_chain_after_transition", |b| {
        b.iter(|| {
            session.revoke();
            black_box(session.verify_chain())
        });
    });
}

fn bench_q34_hash_getter_latency(c: &mut Criterion) {
    // Q34-B6: Measure hash() getter latency
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    c.bench_function("q34_hash_getter", |b| {
        b.iter(|| {
            black_box(session.hash())
        });
    });
}

fn bench_q34_prev_hash_getter_latency(c: &mut Criterion) {
    // Q34-B7: Measure prev_hash() getter latency
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    c.bench_function("q34_prev_hash_getter", |b| {
        b.iter(|| {
            black_box(session.prev_hash())
        });
    });
}

// ============================================================================
// Q34 Overhead Analysis Benchmarks
// ============================================================================

fn bench_q34_overhead_analysis(c: &mut Criterion) {
    // Q34-B8: Compare with vs without hash chain verification
    let mut group = c.benchmark_group("q34_overhead_analysis");

    // Baseline: State transition without verification
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);
    group.bench_function("state_transition_only", |b| {
        b.iter(|| {
            session.revoke();
        });
    });

    // With hash chain: State transition + verification
    let session_verified = OAuthSessionCapsule::new(1002, 0xBCDEF0, None);
    group.bench_function("state_transition_with_verify", |b| {
        b.iter(|| {
            session_verified.revoke();
            black_box(session_verified.verify_chain());
        });
    });

    group.finish();
}

fn bench_q34_snapshot_with_hash_chain(c: &mut Criterion) {
    // Q34-B9: Snapshot includes hash chain (audit export overhead)
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    c.bench_function("q34_snapshot_with_hash_chain", |b| {
        b.iter(|| {
            black_box(session.snapshot())
        });
    });
}

// ============================================================================
// Q34 Concurrent Performance Benchmarks
// ============================================================================

fn bench_q34_concurrent_verify_chain(c: &mut Criterion) {
    // Q34-B10: Multi-threaded hash chain verification throughput
    let mut group = c.benchmark_group("q34_concurrent_verify_chain");

    for thread_count in [1, 2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*thread_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                let session = Arc::new(OAuthSessionCapsule::new(1001, 0xABCDEF, None));

                // Create hash chain with multiple transitions
                for _ in 0..10 {
                    session.refresh(None);
                }

                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let session = Arc::clone(&session);
                            thread::spawn(move || {
                                for _ in 0..100 {
                                    black_box(session.verify_chain());
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_q34_concurrent_hash_update(c: &mut Criterion) {
    // Q34-B11: Concurrent hash chain updates (refresh operations)
    let mut group = c.benchmark_group("q34_concurrent_hash_update");

    for thread_count in [1, 2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*thread_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let session = Arc::new(OAuthSessionCapsule::new(1001, 0xABCDEF, None));
                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let session = Arc::clone(&session);
                            thread::spawn(move || {
                                for _ in 0..100 {
                                    session.refresh(None);
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Q34 Throughput Benchmarks
// ============================================================================

fn bench_q34_verify_chain_throughput(c: &mut Criterion) {
    // Q34-B12: Maximum hash chain verification throughput (8 threads)
    let mut group = c.benchmark_group("q34_verify_chain_throughput");
    group.throughput(Throughput::Elements(1_000_000));

    let session = Arc::new(OAuthSessionCapsule::new(1001, 0xABCDEF, None));

    // Create hash chain with state transitions
    for _ in 0..20 {
        session.refresh(None);
    }

    group.bench_function("1M_verifications_8_threads", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let session = Arc::clone(&session);
                    thread::spawn(move || {
                        for _ in 0..125_000 {
                            black_box(session.verify_chain());
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

fn bench_q34_hash_update_throughput(c: &mut Criterion) {
    // Q34-B13: Hash chain update throughput (refresh operations)
    let mut group = c.benchmark_group("q34_hash_update_throughput");
    group.throughput(Throughput::Elements(100_000));

    let session = Arc::new(OAuthSessionCapsule::new(1001, 0xABCDEF, None));

    group.bench_function("100K_refreshes_8_threads", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let session = Arc::clone(&session);
                    thread::spawn(move || {
                        for _ in 0..12_500 {
                            session.refresh(None);
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// Q34 Production Simulation Benchmarks
// ============================================================================

fn bench_q34_production_workload_with_audit(c: &mut Criterion) {
    // Q34-B14: Production workload (90% verify, 5% refresh, 5% revoke + audit)
    let session = Arc::new(OAuthSessionCapsule::new(1001, 0xABCDEF, None));

    c.bench_function("q34_production_workload_with_audit", |b| {
        b.iter(|| {
            for i in 0..100 {
                match i % 20 {
                    0 => {
                        session.refresh(None);
                        black_box(session.verify_chain()); // Audit after state change
                    }
                    1 => {
                        session.revoke();
                        black_box(session.verify_chain()); // Audit after state change
                    }
                    _ => {
                        black_box(session.verify_token(0xABCDEF));
                    }
                }
            }
        });
    });
}

fn bench_q34_audit_export_latency(c: &mut Criterion) {
    // Q34-B15: Audit export (snapshot + hash chain)
    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

    // Perform state transitions to create audit trail
    for _ in 0..10 {
        session.refresh(None);
    }

    c.bench_function("q34_audit_export_latency", |b| {
        b.iter(|| {
            let snapshot = session.snapshot();

            // Simulate CSV export format
            black_box(format!(
                "{},{},{:?},{},{}",
                snapshot.session_id,
                snapshot.user_id,
                snapshot.session_state,
                snapshot.hash,
                snapshot.prev_hash
            ))
        });
    });
}

// ============================================================================
// Q34 Stress & Scale Benchmarks
// ============================================================================

fn bench_q34_1000_state_transitions(c: &mut Criterion) {
    // Q34-B16: Hash chain performance over 1000 state transitions
    c.bench_function("q34_1000_state_transitions", |b| {
        b.iter(|| {
            let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);

            for i in 0..1000 {
                if i % 2 == 0 {
                    session.refresh(None);
                } else {
                    session.mark_expired();
                }
            }

            // Verify hash chain remains valid
            black_box(session.verify_chain())
        });
    });
}

fn bench_q34_10k_sessions_hash_chain_verification(c: &mut Criterion) {
    // Q34-B17: Verify hash chains for 10K sessions
    c.bench_function("q34_10k_sessions_hash_chain_verification", |b| {
        b.iter(|| {
            let sessions: Vec<_> = (0..10_000)
                .map(|i| {
                    let session = OAuthSessionCapsule::new(i as u64, 0xABCDEF, None);

                    // Create hash chain
                    if i % 3 == 0 {
                        session.refresh(None);
                    }
                    if i % 5 == 0 {
                        session.revoke();
                    }

                    session
                })
                .collect();

            // Verify all hash chains
            let valid_count = sessions.iter().filter(|s| s.verify_chain()).count();

            black_box(valid_count)
        });
    });
}

// ============================================================================
// Q34 Comparative Benchmarks (With vs Without Hash Chain)
// ============================================================================

fn bench_q34_comparison_hash_chain_overhead(c: &mut Criterion) {
    // Q34-B18: Compare latency with/without hash chain verification
    let mut group = c.benchmark_group("q34_comparison_hash_chain_overhead");

    // Scenario 1: No verification (baseline)
    let session_no_verify = OAuthSessionCapsule::new(1001, 0xABCDEF, None);
    group.bench_function("no_hash_verification", |b| {
        b.iter(|| {
            session_no_verify.refresh(None);
        });
    });

    // Scenario 2: With verification (audit trail)
    let session_with_verify = OAuthSessionCapsule::new(1002, 0xBCDEF0, None);
    group.bench_function("with_hash_verification", |b| {
        b.iter(|| {
            session_with_verify.refresh(None);
            black_box(session_with_verify.verify_chain());
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    q34_hash_updates,
    bench_q34_hash_update_on_revoke,
    bench_q34_hash_update_on_expire,
    bench_q34_hash_update_on_refresh,
);

criterion_group!(
    q34_hash_verification,
    bench_q34_verify_chain_single_thread,
    bench_q34_verify_chain_after_state_transition,
    bench_q34_hash_getter_latency,
    bench_q34_prev_hash_getter_latency,
);

criterion_group!(
    q34_overhead_analysis,
    bench_q34_overhead_analysis,
    bench_q34_snapshot_with_hash_chain,
);

criterion_group!(
    q34_concurrent,
    bench_q34_concurrent_verify_chain,
    bench_q34_concurrent_hash_update,
);

criterion_group!(
    q34_throughput,
    bench_q34_verify_chain_throughput,
    bench_q34_hash_update_throughput,
);

criterion_group!(
    q34_production,
    bench_q34_production_workload_with_audit,
    bench_q34_audit_export_latency,
);

criterion_group!(
    q34_stress,
    bench_q34_1000_state_transitions,
    bench_q34_10k_sessions_hash_chain_verification,
);

criterion_group!(
    q34_comparison,
    bench_q34_comparison_hash_chain_overhead,
);

criterion_main!(
    q34_hash_updates,
    q34_hash_verification,
    q34_overhead_analysis,
    q34_concurrent,
    q34_throughput,
    q34_production,
    q34_stress,
    q34_comparison,
);
