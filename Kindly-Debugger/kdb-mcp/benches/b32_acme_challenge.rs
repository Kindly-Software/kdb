//! B32 Benchmarks - ACME Certificate Manager Performance Validation
//!
//! **Purpose**: Validate performance claims using B32 framework (fair baselines, 95% CI, 1000+ iterations)
//!
//! **Benchmark Groups**:
//! 1. **challenge_response**: HTTP-01 challenge handling latency
//! 2. **needs_renewal**: Certificate expiry check latency
//! 3. **state_machine**: State transition throughput
//! 4. **renewal_workflow**: Full renewal workflow latency
//!
//! **Performance Targets** (from UCE34 Q10c):
//! - needs_renewal: <10ns (fast path, atomic read)
//! - challenge_response: <100μs (string matching, nominal case)
//! - state transitions: <10ns each (atomic operations)
//! - Full renewal: ~5s (ACME challenge SLA, background operation)
//!
//! **Framework Compliance**:
//! - B32: Fair baseline (Let's Encrypt SLA ~5s, nginx reload ~100ms)
//! - 95% CI: 1000+ iterations per benchmark
//! - Reproducibility: Deterministic inputs, hardware-aware (K1-K70)

use kdb_mcp::acme_cert_manager::{
    AcmeCertManagerCapsule, AcmeState,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Test Fixtures
// ============================================================================

fn setup_test_capsule() -> (AcmeCertManagerCapsule, u64) {
    let test_dir = std::env::temp_dir().join("acme_bench");
    let _ = fs::remove_dir_all(&test_dir);
    let _ = fs::create_dir_all(&test_dir);

    let cert_path = test_dir.join("cert.pem");
    let key_path = test_dir.join("key.pem");

    let _ = fs::write(&cert_path, b"-----BEGIN CERTIFICATE-----\nMOCK\n-----END CERTIFICATE-----");
    let _ = fs::write(&key_path, b"-----BEGIN PRIVATE KEY-----\nMOCK\n-----END PRIVATE KEY-----");

    let capsule = AcmeCertManagerCapsule::new("bench.example.com", &cert_path, &key_path)
        .expect("create capsule");

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time error")
        .as_secs();

    let _ = fs::remove_dir_all(&test_dir);
    (capsule, now)
}

// ============================================================================
// Benchmark Group 1: needs_renewal (Fast Path)
// ============================================================================

fn bench_needs_renewal_false(c: &mut Criterion) {
    let (capsule, now) = setup_test_capsule();

    // Set expiry to 90 days from now (no renewal needed with 30-day window)
    capsule.cert_expiry_unix.store(now + 90 * 86400, Ordering::Release);

    let mut group = c.benchmark_group("needs_renewal");
    group.throughput(Throughput::Elements(1));

    group.bench_function("expiry_90days_away", |b| {
        b.iter(|| {
            black_box(capsule.needs_renewal(black_box(now), black_box(30)))
        })
    });

    group.finish();
}

fn bench_needs_renewal_true(c: &mut Criterion) {
    let (capsule, now) = setup_test_capsule();

    // Set expiry to 15 days from now (renewal needed with 30-day window)
    capsule.cert_expiry_unix.store(now + 15 * 86400, Ordering::Release);

    let mut group = c.benchmark_group("needs_renewal");
    group.throughput(Throughput::Elements(1));

    group.bench_function("expiry_15days_away", |b| {
        b.iter(|| {
            black_box(capsule.needs_renewal(black_box(now), black_box(30)))
        })
    });

    group.finish();
}

fn bench_needs_renewal_expired(c: &mut Criterion) {
    let (capsule, now) = setup_test_capsule();

    // Set expiry to past (certificate expired)
    capsule.cert_expiry_unix.store(now - 100, Ordering::Release);

    let mut group = c.benchmark_group("needs_renewal");
    group.throughput(Throughput::Elements(1));

    group.bench_function("certificate_expired", |b| {
        b.iter(|| {
            black_box(capsule.needs_renewal(black_box(now), black_box(0)))
        })
    });

    group.finish();
}

// ============================================================================
// Benchmark Group 2: State Machine Operations
// ============================================================================

fn bench_get_state(c: &mut Criterion) {
    let (capsule, _) = setup_test_capsule();

    let mut group = c.benchmark_group("state_machine");
    group.throughput(Throughput::Elements(1));

    group.bench_function("get_state", |b| {
        b.iter(|| {
            black_box(capsule.get_state())
        })
    });

    group.finish();
}

fn bench_state_transition(c: &mut Criterion) {
    let (capsule, _) = setup_test_capsule();

    let mut group = c.benchmark_group("state_machine");
    group.throughput(Throughput::Elements(1));

    let states = vec![
        AcmeState::Idle,
        AcmeState::Requesting,
        AcmeState::Challenging,
        AcmeState::Validating,
        AcmeState::Installing,
        AcmeState::Failed,
    ];

    for state in states {
        let state_name = format!("transition_to_{:?}", state);
        group.bench_with_input(
            BenchmarkId::from_parameter(&state_name),
            &state,
            |b, &s| {
                b.iter(|| {
                    capsule.state.store(black_box(s.as_u8() as u64), Ordering::Release);
                    black_box(capsule.get_state())
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark Group 3: Challenge Response
// ============================================================================

fn bench_handle_challenge_inactive(c: &mut Criterion) {
    let (capsule, _) = setup_test_capsule();

    let mut group = c.benchmark_group("challenge_response");
    group.throughput(Throughput::Elements(1));

    group.bench_function("inactive_state", |b| {
        b.iter(|| {
            black_box(capsule.handle_challenge(black_box("test_token")))
        })
    });

    group.finish();
}

fn bench_handle_challenge_active_expired(c: &mut Criterion) {
    let (capsule, now) = setup_test_capsule();

    // Set to Challenging state
    capsule.state.store(AcmeState::Challenging.as_u8() as u64, Ordering::Release);

    // Set challenge expiry to past
    capsule.challenge_expiry_unix.store(now - 100, Ordering::Release);

    let mut group = c.benchmark_group("challenge_response");
    group.throughput(Throughput::Elements(1));

    group.bench_function("active_but_expired", |b| {
        b.iter(|| {
            black_box(capsule.handle_challenge(black_box("test_token")))
        })
    });

    group.finish();
}

fn bench_handle_challenge_active_valid(c: &mut Criterion) {
    let (capsule, now) = setup_test_capsule();

    // Set to Challenging state
    capsule.state.store(AcmeState::Challenging.as_u8() as u64, Ordering::Release);

    // Set challenge expiry to future
    capsule.challenge_expiry_unix.store(now + 600, Ordering::Release);

    let mut group = c.benchmark_group("challenge_response");
    group.throughput(Throughput::Elements(1));

    group.bench_function("active_and_valid", |b| {
        b.iter(|| {
            black_box(capsule.handle_challenge(black_box("test_token")))
        })
    });

    group.finish();
}

// ============================================================================
// Benchmark Group 4: Renewal Workflow
// ============================================================================

fn bench_trigger_renewal(c: &mut Criterion) {
    let (capsule, now) = setup_test_capsule();

    let mut group = c.benchmark_group("renewal_workflow");
    group.throughput(Throughput::Elements(1));

    group.bench_function("trigger_renewal_from_idle", |b| {
        b.iter(|| {
            // Reset to Idle
            capsule.state.store(AcmeState::Idle.as_u8() as u64, Ordering::Release);
            black_box(capsule.trigger_renewal(black_box(now)))
        })
    });

    group.finish();
}

fn bench_mark_renewal_failed(c: &mut Criterion) {
    let (capsule, now) = setup_test_capsule();

    let mut group = c.benchmark_group("renewal_workflow");
    group.throughput(Throughput::Elements(1));

    group.bench_function("mark_renewal_failed", |b| {
        b.iter(|| {
            // Reset to Idle
            capsule.state.store(AcmeState::Idle.as_u8() as u64, Ordering::Release);
            capsule.failed_attempts.store(0, Ordering::Release);
            black_box(capsule.mark_renewal_failed(black_box(now)))
        })
    });

    group.finish();
}

fn bench_complete_renewal(c: &mut Criterion) {
    let (capsule, now) = setup_test_capsule();

    let mut group = c.benchmark_group("renewal_workflow");
    group.throughput(Throughput::Elements(1));

    group.bench_function("complete_renewal", |b| {
        b.iter(|| {
            // Move to Installing state
            capsule.state.store(AcmeState::Installing.as_u8() as u64, Ordering::Release);
            let new_expiry = now + 90 * 86400;
            black_box(capsule.complete_renewal(black_box(new_expiry), black_box(now)))
        })
    });

    group.finish();
}

fn bench_is_in_backoff(c: &mut Criterion) {
    let (capsule, now) = setup_test_capsule();

    // Set backoff until 1 hour from now
    capsule.backoff_until_unix.store(now + 3600, Ordering::Release);

    let mut group = c.benchmark_group("renewal_workflow");
    group.throughput(Throughput::Elements(1));

    group.bench_function("is_in_backoff", |b| {
        b.iter(|| {
            black_box(capsule.is_in_backoff(black_box(now)))
        })
    });

    group.finish();
}

fn bench_load_current_cert(c: &mut Criterion) {
    let (capsule, _) = setup_test_capsule();

    let mut group = c.benchmark_group("renewal_workflow");
    group.throughput(Throughput::Elements(1));

    group.bench_function("load_current_cert", |b| {
        b.iter(|| {
            black_box(capsule.load_current_cert())
        })
    });

    group.finish();
}

// ============================================================================
// Benchmark Group 5: Throughput Tests (Multiple Operations)
// ============================================================================

fn bench_needs_renewal_throughput(c: &mut Criterion) {
    let (capsule, now) = setup_test_capsule();
    capsule.cert_expiry_unix.store(now + 50 * 86400, Ordering::Release);

    let mut group = c.benchmark_group("throughput");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("1k_needs_renewal_checks", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(capsule.needs_renewal(black_box(now), black_box(30)));
            }
        })
    });

    group.finish();
}

fn bench_state_read_throughput(c: &mut Criterion) {
    let (capsule, _) = setup_test_capsule();

    let mut group = c.benchmark_group("throughput");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("1k_state_reads", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(capsule.get_state());
            }
        })
    });

    group.finish();
}

fn bench_full_workflow_throughput(c: &mut Criterion) {
    let (capsule, now) = setup_test_capsule();

    let mut group = c.benchmark_group("throughput");
    group.throughput(Throughput::Elements(100));

    group.bench_function("100_full_state_checks", |b| {
        b.iter(|| {
            for _ in 0..100 {
                black_box(capsule.needs_renewal(black_box(now), black_box(30)));
                black_box(capsule.get_state());
                black_box(capsule.is_in_backoff(black_box(now)));
            }
        })
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    // Fast path benchmarks
    bench_needs_renewal_false,
    bench_needs_renewal_true,
    bench_needs_renewal_expired,

    // State machine
    bench_get_state,
    bench_state_transition,

    // Challenge handling
    bench_handle_challenge_inactive,
    bench_handle_challenge_active_expired,
    bench_handle_challenge_active_valid,

    // Renewal workflow
    bench_trigger_renewal,
    bench_mark_renewal_failed,
    bench_complete_renewal,
    bench_is_in_backoff,
    bench_load_current_cert,

    // Throughput
    bench_needs_renewal_throughput,
    bench_state_read_throughput,
    bench_full_workflow_throughput,
);

criterion_main!(benches);
