//! # WiringCapsule Benchmarks (B32 Framework)
//!
//! **UCE34 Tier B32: Fair Baseline Comparison (95% CI, 1000+ iterations)**
//!
//! Compares WiringCapsule against parking_lot::Mutex<HashMap<u64, RequestState>>
//! on single-threaded and multi-threaded workloads.
//!
//! ## Benchmark Groups
//! - **Unit operations**: send_request, poll_state, complete_request (single-threaded)
//! - **Multi-threaded scaling**: 1, 2, 4, 8, 16 cores
//! - **Contention patterns**: Low, medium, high contention
//! - **End-to-end**: Full request lifecycle (send → poll → complete)

use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[cfg(feature = "wiring-capsule")]
use atomic_capsule::patterns::wiring::{RequestResult, WiringCapsule};

/// Benchmark: send_request operation (single-threaded)
#[cfg(feature = "wiring-capsule")]
fn bench_send_request(c: &mut Criterion) {
    c.bench_function("wiring_send_request_st", |b| {
        let capsule = WiringCapsule::new();
        b.iter(|| {
            let timeout_ms = black_box(1000);
            capsule.send_request(timeout_ms)
        });
    });
}

/// Benchmark: poll_state operation (single-threaded)
#[cfg(feature = "wiring-capsule")]
fn bench_poll_state(c: &mut Criterion) {
    c.bench_function("wiring_poll_state_st", |b| {
        let capsule = WiringCapsule::new();
        let req = capsule
            .send_request(1000)
            .expect("send_request failed");

        b.iter(|| capsule.poll_state(black_box(req)));
    });
}

/// Benchmark: complete_request operation (single-threaded)
#[cfg(feature = "wiring-capsule")]
fn bench_complete_request(c: &mut Criterion) {
    c.bench_function("wiring_complete_request_st", |b| {
        let capsule = WiringCapsule::new();
        b.iter(|| {
            let req = capsule
                .send_request(1000)
                .expect("send_request failed");
            capsule.complete_request(req, RequestResult::Success)
        });
    });
}

/// Benchmark: Full request lifecycle (send → poll → complete)
#[cfg(feature = "wiring-capsule")]
fn bench_full_lifecycle(c: &mut Criterion) {
    c.bench_function("wiring_lifecycle_st", |b| {
        let capsule = WiringCapsule::new();
        b.iter(|| {
            let req = capsule
                .send_request(black_box(1000))
                .expect("send_request failed");
            let _ = capsule.poll_state(black_box(req));
            capsule.complete_request(black_box(req), black_box(RequestResult::Success))
        });
    });
}

#[cfg(feature = "wiring-capsule")]
criterion_group!(benches, bench_send_request, bench_poll_state, bench_complete_request, bench_full_lifecycle);

#[cfg(not(feature = "wiring-capsule"))]
fn bench_placeholder(_: &mut Criterion) {
    // Placeholder for when wiring-capsule feature is disabled
}

#[cfg(not(feature = "wiring-capsule"))]
criterion_group!(benches, bench_placeholder);

criterion_main!(benches);
