//! RequestCapsule benchmarks (Tier 6 Mixed)
//!
//! Validates <100ns operations across all 5 component capsules

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use clapi_core::capsules::{
    RequestCapsule, RequestCoordinator, RequestCapsule128, RoutingCapsule128,
    ResponseCapsule256, AuditLogEntry128,
};
use std::sync::Arc;

fn bench_request_capsule_new(c: &mut Criterion) {
    c.bench_function("request_capsule_new", |b| {
        b.iter(|| {
            let capsule = RequestCapsule::new(
                black_box(123),
                black_box(456),
                black_box(789),
            );
            black_box(capsule);
        });
    });
}

fn bench_request_capsule_record_latency(c: &mut Criterion) {
    let capsule = RequestCapsule::new(123, 456, 789);

    c.bench_function("request_capsule_record_latency", |b| {
        b.iter(|| {
            capsule.record_latency(black_box(50_000));
        });
    });
}

fn bench_request_capsule_calculate_cost(c: &mut Criterion) {
    let capsule = RequestCapsule::new(123, 456, 789);

    c.bench_function("request_capsule_calculate_cost", |b| {
        b.iter(|| {
            capsule.calculate_cost(black_box(1.50), black_box(1000_00));
        });
    });
}

fn bench_request_capsule_snapshot(c: &mut Criterion) {
    let capsule = RequestCapsule::new(123, 456, 789);
    capsule.record_status(200);
    capsule.record_latency(50_000);
    capsule.calculate_cost(1.50, 1000_00);

    c.bench_function("request_capsule_snapshot", |b| {
        b.iter(|| {
            let snapshot = capsule.snapshot();
            black_box(snapshot);
        });
    });
}

fn bench_coordinator_init(c: &mut Criterion) {
    let budget = Arc::new(RequestCapsule128::new(10_000_00));
    let routing = Arc::new(RoutingCapsule128::new(1, 2));
    let response = Arc::new(ResponseCapsule256::new());
    let audit = Arc::new(AuditLogEntry128::new());

    let coordinator = RequestCoordinator::new(
        123,
        456,
        789,
        budget.clone(),
        routing.clone(),
        response.clone(),
        audit.clone(),
    );

    c.bench_function("coordinator_init", |b| {
        b.iter(|| {
            // Create new budget for each iteration
            let budget = Arc::new(RequestCapsule128::new(10_000_00));
            let routing = Arc::new(RoutingCapsule128::new(1, 2));
            let response = Arc::new(ResponseCapsule256::new());
            let audit = Arc::new(AuditLogEntry128::new());

            let coordinator = RequestCoordinator::new(
                123,
                456,
                789,
                budget.clone(),
                routing.clone(),
                response.clone(),
                audit.clone(),
            );

            let result = coordinator.init(black_box(50_00));
            black_box(result);
        });
    });
}

fn bench_coordinator_record_response(c: &mut Criterion) {
    let budget = Arc::new(RequestCapsule128::new(10_000_00));
    let routing = Arc::new(RoutingCapsule128::new(1, 2));
    let response = Arc::new(ResponseCapsule256::new());
    let audit = Arc::new(AuditLogEntry128::new());

    let coordinator = RequestCoordinator::new(
        123,
        456,
        789,
        budget.clone(),
        routing.clone(),
        response.clone(),
        audit.clone(),
    );

    coordinator.init(50_00).unwrap();

    c.bench_function("coordinator_record_response", |b| {
        b.iter(|| {
            coordinator.record_response(
                black_box(200),
                black_box(25_000),
                black_box(50.0),
                black_box(1000),
            );
        });
    });
}

fn bench_coordinator_stream_metrics(c: &mut Criterion) {
    let budget = Arc::new(RequestCapsule128::new(10_000_00));
    let routing = Arc::new(RoutingCapsule128::new(1, 2));
    let response = Arc::new(ResponseCapsule256::new());
    let audit = Arc::new(AuditLogEntry128::new());

    let coordinator = RequestCoordinator::new(
        123,
        456,
        789,
        budget.clone(),
        routing.clone(),
        response.clone(),
        audit.clone(),
    );

    coordinator.init(50_00).unwrap();
    coordinator.record_response(200, 25_000, 50.0, 1000);

    c.bench_function("coordinator_stream_metrics", |b| {
        b.iter(|| {
            let metrics = coordinator.stream_metrics();
            black_box(metrics);
        });
    });
}

criterion_group!(
    benches,
    bench_request_capsule_new,
    bench_request_capsule_record_latency,
    bench_request_capsule_calculate_cost,
    bench_request_capsule_snapshot,
    bench_coordinator_init,
    bench_coordinator_record_response,
    bench_coordinator_stream_metrics,
);
criterion_main!(benches);
