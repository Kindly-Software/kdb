// B32 Benchmarks - DeploymentCoordinatorCapsule
// Fair baselines, 95% confidence intervals, 1000+ iterations
//
// Performance Targets:
// - State transition: <100ns
// - Audit append: <50ns
// - Rollback decision: <500ns
// - Health validation: <1μs
// - Concurrent transitions: <10μs (16 threads)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::Arc;
use std::thread;

// Import from src/capsules/deployment_coordinator.rs
// (Assuming this module is accessible via `use kindly_verified_web::capsules::...`)
use kindly_verified_web::capsules::deployment_coordinator::*;

// ===== Core Performance Benchmarks =====

fn bench_state_transition(c: &mut Criterion) {
    let capsule = DeploymentCoordinatorCapsule::new();

    c.bench_function("state_transition_single", |b| {
        b.iter(|| {
            black_box(capsule.transition_state(DeploymentState::PreValidating));
            black_box(capsule.transition_state(DeploymentState::Idle));
        });
    });
}

fn bench_audit_append(c: &mut Criterion) {
    let capsule = DeploymentCoordinatorCapsule::new();

    c.bench_function("audit_append", |b| {
        b.iter(|| {
            capsule.append_audit_entry(black_box(b"test_event"));
        });
    });
}

fn bench_rollback_decision(c: &mut Criterion) {
    c.bench_function("rollback_decision", |b| {
        b.iter(|| {
            let capsule = DeploymentCoordinatorCapsule::new();
            black_box(capsule.start_deployment(1, 0, 0).unwrap());
            capsule.force_state(DeploymentState::HealthChecking);
            black_box(capsule.initiate_rollback(RollbackReason::HealthCheckFailed));
            black_box(capsule.complete_rollback().unwrap());
        });
    });
}

fn bench_health_validation(c: &mut Criterion) {
    let capsule = DeploymentCoordinatorCapsule::new();
    capsule.start_deployment(1, 0, 0).unwrap();
    capsule.complete_prevalidation().unwrap();
    capsule.start_health_checking().unwrap();

    c.bench_function("health_validation", |b| {
        b.iter(|| {
            black_box(capsule.record_health_check(true));
        });
    });
}

fn bench_metrics_recording(c: &mut Criterion) {
    let capsule = DeploymentCoordinatorCapsule::new();

    let mut group = c.benchmark_group("metrics");

    group.bench_function("record_traffic", |b| {
        b.iter(|| {
            black_box(capsule.record_traffic());
        });
    });

    group.bench_function("record_error", |b| {
        b.iter(|| {
            black_box(capsule.record_error());
        });
    });

    group.bench_function("error_rate_calculation", |b| {
        b.iter(|| {
            black_box(capsule.error_rate());
        });
    });

    group.finish();
}

// ===== Concurrent Benchmarks =====

fn bench_concurrent_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_transitions");

    for thread_count in [2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            &thread_count,
            |b, &threads| {
                b.iter(|| {
                    let capsule = Arc::new(DeploymentCoordinatorCapsule::new());
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let c = capsule.clone();
                            thread::spawn(move || {
                                black_box(c.transition_state(DeploymentState::PreValidating));
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_concurrent_audit_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_audit");

    for thread_count in [2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            &thread_count,
            |b, &threads| {
                b.iter(|| {
                    let capsule = Arc::new(DeploymentCoordinatorCapsule::new());
                    let handles: Vec<_> = (0..threads)
                        .map(|i| {
                            let c = capsule.clone();
                            thread::spawn(move || {
                                let event = format!("event_{}", i);
                                black_box(c.append_audit_entry(event.as_bytes()));
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_concurrent_health_checks(c: &mut Criterion) {
    let capsule = Arc::new(DeploymentCoordinatorCapsule::new());
    capsule.start_deployment(1, 0, 0).unwrap();
    capsule.complete_prevalidation().unwrap();
    capsule.start_health_checking().unwrap();

    let mut group = c.benchmark_group("concurrent_health_checks");

    for thread_count in [2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            &thread_count,
            |b, &threads| {
                b.iter(|| {
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let c = capsule.clone();
                            thread::spawn(move || {
                                black_box(c.record_health_check(true));
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ===== Full Deployment Workflow Benchmarks =====

fn bench_full_deployment_success(c: &mut Criterion) {
    c.bench_function("full_deployment_success", |b| {
        b.iter(|| {
            let capsule = DeploymentCoordinatorCapsule::new();

            // Full workflow
            black_box(capsule.start_deployment(1, 2, 3).unwrap());
            black_box(capsule.complete_prevalidation().unwrap());
            black_box(capsule.start_health_checking().unwrap());

            // Simulate health checks
            for _ in 0..5 {
                black_box(capsule.record_health_check(true));
            }

            black_box(capsule.start_warmup().unwrap());
            capsule.warmup_duration.store(0, std::sync::atomic::Ordering::Release);
            black_box(capsule.go_live().unwrap());
        });
    });
}

fn bench_full_deployment_rollback(c: &mut Criterion) {
    c.bench_function("full_deployment_rollback", |b| {
        b.iter(|| {
            let capsule = DeploymentCoordinatorCapsule::new();

            // Start deployment
            black_box(capsule.start_deployment(2, 0, 0).unwrap());
            black_box(capsule.complete_prevalidation().unwrap());
            black_box(capsule.start_health_checking().unwrap());

            // Fail health checks (triggers automatic rollback)
            for _ in 0..3 {
                black_box(capsule.record_health_check(false));
            }

            // Complete rollback
            black_box(capsule.complete_rollback().unwrap());
        });
    });
}

// ===== Version Encoding/Decoding Benchmarks =====

fn bench_version_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("version");

    group.bench_function("encode", |b| {
        b.iter(|| {
            black_box(encode_version(1, 2, 3));
        });
    });

    group.bench_function("decode", |b| {
        let version = encode_version(1, 2, 3);
        b.iter(|| {
            black_box(decode_version(version));
        });
    });

    group.bench_function("encode_decode_roundtrip", |b| {
        b.iter(|| {
            let encoded = black_box(encode_version(10, 20, 30));
            black_box(decode_version(encoded));
        });
    });

    group.finish();
}

// ===== CRC64 Hash Benchmarks =====

fn bench_crc64_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("crc64");

    for data_size in [1, 4, 16, 64] {
        let data: Vec<u64> = (0..data_size).collect();
        group.bench_with_input(
            BenchmarkId::from_parameter(data_size),
            &data,
            |b, data| {
                b.iter(|| {
                    black_box(crc64_hash(black_box(data)));
                });
            },
        );
    }

    group.finish();
}

// ===== Circuit Breaker Benchmarks =====

fn bench_circuit_breaker(c: &mut Criterion) {
    let capsule = DeploymentCoordinatorCapsule::new();

    let mut group = c.benchmark_group("circuit_breaker");

    group.bench_function("check", |b| {
        b.iter(|| {
            black_box(capsule.check_circuit_breaker());
        });
    });

    group.bench_function("increment", |b| {
        b.iter(|| {
            capsule.increment_circuit_breaker();
        });
    });

    group.bench_function("reset", |b| {
        b.iter(|| {
            capsule.reset_circuit_breaker();
        });
    });

    group.finish();
}

// ===== Configuration Validation Benchmarks =====

fn bench_config_validation(c: &mut Criterion) {
    let capsule = DeploymentCoordinatorCapsule::new();
    let config_hash = crc64_hash(&[123, 456, 789]);
    capsule.set_config_hash(config_hash);

    c.bench_function("config_validation", |b| {
        b.iter(|| {
            black_box(capsule.validate_config(config_hash));
        });
    });
}

// ===== Benchmark Groups =====

criterion_group!(
    core_benches,
    bench_state_transition,
    bench_audit_append,
    bench_rollback_decision,
    bench_health_validation,
    bench_metrics_recording,
);

criterion_group!(
    concurrent_benches,
    bench_concurrent_state_transitions,
    bench_concurrent_audit_append,
    bench_concurrent_health_checks,
);

criterion_group!(
    workflow_benches,
    bench_full_deployment_success,
    bench_full_deployment_rollback,
);

criterion_group!(
    utility_benches,
    bench_version_encoding,
    bench_crc64_hash,
    bench_circuit_breaker,
    bench_config_validation,
);

criterion_main!(
    core_benches,
    concurrent_benches,
    workflow_benches,
    utility_benches,
);
