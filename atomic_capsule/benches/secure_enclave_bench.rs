//! SecureEnclaveCapsule Benchmarks (B32 Framework)
//!
//! Performance validation for TEE attestation and enclave operations.
//! Targets: <100ms attestation, <1μs enclave call overhead, transparent memory encryption
//!
//! Run with: cargo bench --bench secure_enclave_bench --release

use atomic_capsule::capsules::security::secure_enclave::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

// ============================================================================
// Benchmark 1: Remote Attestation Latency
// ============================================================================

fn bench_software_attestation(c: &mut Criterion) {
    c.bench_function("secure_enclave_software_attestation", |b| {
        let capsule = SecureEnclaveCapsule::new(TeeType::Software);
        b.iter(|| {
            let result = capsule.remote_attestation();
            black_box(result.unwrap());
        });
    });
}

fn bench_sgx_attestation_simulation(c: &mut Criterion) {
    c.bench_function("secure_enclave_sgx_attestation_sim", |b| {
        let capsule = SecureEnclaveCapsule::new(TeeType::IntelSgx);
        b.iter(|| {
            let result = capsule.remote_attestation();
            black_box(result.unwrap());
        });
    });
}

fn bench_sev_attestation_simulation(c: &mut Criterion) {
    c.bench_function("secure_enclave_sev_attestation_sim", |b| {
        let capsule = SecureEnclaveCapsule::new(TeeType::AmdSev);
        b.iter(|| {
            let result = capsule.remote_attestation();
            black_box(result.unwrap());
        });
    });
}

fn bench_trustzone_attestation_simulation(c: &mut Criterion) {
    c.bench_function("secure_enclave_trustzone_attestation_sim", |b| {
        let capsule = SecureEnclaveCapsule::new(TeeType::ArmTrustZone);
        b.iter(|| {
            let result = capsule.remote_attestation();
            black_box(result.unwrap());
        });
    });
}

// ============================================================================
// Benchmark 2: Enclave Call Overhead
// ============================================================================

fn bench_enclave_call_latency(c: &mut Criterion) {
    c.bench_function("secure_enclave_call_overhead", |b| {
        let capsule = SecureEnclaveCapsule::new(TeeType::Software);
        let data = black_box([1u8; 32]);
        b.iter(|| {
            let result = capsule.enclave_call(&data);
            black_box(result.unwrap());
        });
    });
}

fn bench_enclave_call_batch(c: &mut Criterion) {
    c.bench_function("secure_enclave_call_batch_100", |b| {
        let capsule = SecureEnclaveCapsule::new(TeeType::Software);
        let data = black_box([1u8; 32]);
        b.iter(|| {
            for _ in 0..100 {
                let _ = capsule.enclave_call(&data);
            }
        });
    });
}

// ============================================================================
// Benchmark 3: Measurement Hash Verification
// ============================================================================

fn bench_hash_verification(c: &mut Criterion) {
    c.bench_function("secure_enclave_hash_verify", |b| {
        let mut capsule = SecureEnclaveCapsule::new(TeeType::Software);
        let hash = black_box([42u8; 48]);
        capsule.set_measurement_hash(hash);

        b.iter(|| {
            let result = capsule.verify_measurement(&hash);
            black_box(result);
        });
    });
}

fn bench_hash_verification_mismatch(c: &mut Criterion) {
    c.bench_function("secure_enclave_hash_verify_mismatch", |b| {
        let mut capsule = SecureEnclaveCapsule::new(TeeType::Software);
        let hash1 = black_box([42u8; 48]);
        let hash2 = black_box([43u8; 48]);
        capsule.set_measurement_hash(hash1);

        b.iter(|| {
            let result = capsule.verify_measurement(&hash2);
            black_box(result);
        });
    });
}

// ============================================================================
// Benchmark 4: State Transitions
// ============================================================================

fn bench_state_suspension(c: &mut Criterion) {
    c.bench_function("secure_enclave_state_suspend", |b| {
        let capsule = SecureEnclaveCapsule::new(TeeType::Software);
        b.iter(|| {
            let _ = capsule.suspend();
            let _ = capsule.resume();
        });
    });
}

// ============================================================================
// Benchmark 5: Metrics Collection
// ============================================================================

fn bench_metrics_collection(c: &mut Criterion) {
    c.bench_function("secure_enclave_get_metrics", |b| {
        let capsule = SecureEnclaveCapsule::new(TeeType::Software);

        // Pre-populate metrics with some calls
        for _ in 0..100 {
            let _ = capsule.enclave_call(&[]);
        }

        b.iter(|| {
            let metrics = capsule.call_metrics();
            black_box(metrics);
        });
    });
}

fn bench_attestation_latency_query(c: &mut Criterion) {
    c.bench_function("secure_enclave_get_attestation_latency", |b| {
        let capsule = SecureEnclaveCapsule::new(TeeType::Software);
        let _ = capsule.remote_attestation();

        b.iter(|| {
            let latency_ms = capsule.last_attestation_latency_ms();
            black_box(latency_ms);
        });
    });
}

// ============================================================================
// Benchmark 6: Concurrent Operations
// ============================================================================

fn bench_concurrent_enclave_calls(c: &mut Criterion) {
    c.bench_function("secure_enclave_concurrent_calls_10_threads", |b| {
        b.iter(|| {
            let capsule = std::sync::Arc::new(SecureEnclaveCapsule::new(TeeType::Software));
            let handles: Vec<_> = (0..10)
                .map(|i| {
                    let capsule_clone = capsule.clone();
                    std::thread::spawn(move || {
                        for _ in 0..10 {
                            let _ = capsule_clone.enclave_call(&[i as u8]);
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }

            black_box(capsule.call_metrics());
        });
    });
}

// ============================================================================
// Benchmark 7: Memory Encryption Status
// ============================================================================

fn bench_memory_encryption_status_check(c: &mut Criterion) {
    c.bench_function("secure_enclave_check_encryption_status", |b| {
        let capsule = SecureEnclaveCapsule::new(TeeType::AmdSev);
        capsule.set_memory_encryption_status(MemoryEncryptionStatus::Transparent);

        b.iter(|| {
            let status = capsule.memory_encryption_status();
            black_box(status);
        });
    });
}

fn bench_memory_encryption_status_update(c: &mut Criterion) {
    c.bench_function("secure_enclave_update_encryption_status", |b| {
        let capsule = SecureEnclaveCapsule::new(TeeType::AmdSev);

        b.iter(|| {
            capsule.set_memory_encryption_status(MemoryEncryptionStatus::Transparent);
            capsule.set_memory_encryption_status(MemoryEncryptionStatus::Verified);
        });
    });
}

// ============================================================================
// Benchmark 8: Throughput Tests (Derived)
// ============================================================================

fn bench_enclave_calls_per_second(c: &mut Criterion) {
    c.bench_function("secure_enclave_calls_per_second", |b| {
        let capsule = SecureEnclaveCapsule::new(TeeType::Software);
        b.iter(|| {
            for _ in 0..1000 {
                let _ = capsule.enclave_call(&[]);
            }
        });
    });
}

criterion_group!(
    benches,
    // Attestation latency (Group 1)
    bench_software_attestation,
    bench_sgx_attestation_simulation,
    bench_sev_attestation_simulation,
    bench_trustzone_attestation_simulation,
    // Enclave call overhead (Group 2)
    bench_enclave_call_latency,
    bench_enclave_call_batch,
    // Hash verification (Group 3)
    bench_hash_verification,
    bench_hash_verification_mismatch,
    // State transitions (Group 4)
    bench_state_suspension,
    // Metrics collection (Group 5)
    bench_metrics_collection,
    bench_attestation_latency_query,
    // Concurrent operations (Group 6)
    bench_concurrent_enclave_calls,
    // Memory encryption (Group 7)
    bench_memory_encryption_status_check,
    bench_memory_encryption_status_update,
    // Throughput (Group 8)
    bench_enclave_calls_per_second,
);

criterion_main!(benches);
