//! Quorum Read Capsule Benchmark (T8 Network)
//!
//! **B32 Framework Validation**: K56-K60 (Network Reality Checks)
//!
//! ## Performance Targets
//!
//! | Operation | Target | K-Check |
//! |-----------|--------|---------|
//! | set_generation | <5ns | K56 (lockfree atomic) |
//! | mark_completed | <10ns | K56 (atomic OR) |
//! | select_winner | <15ns | K56 (3 atomic loads) |
//! | has_quorum | <5ns | K56 (atomic load + popcount) |
//! | reset | <15ns | K56 (4 atomic stores) |
//! | Full quorum read | <100ns | K58 (local coordination) |
//!
//! ## UCE34 Tier Classification
//!
//! - **Tier**: T1 Atomic + T8 Network compound
//! - **Speedup**: N/A (consistency feature, not optimization)
//! - **Use Case**: Distributed cache quorum coordination
//!
//! ## ASSUM Safety
//!
//! #ASSUME_LOCKFREE: All operations are lockfree atomics (<50ns)
//! #VERIFY_LOCKFREE: No mutex, no CAS contention, pure atomic ops
//!
//! #ASSUME_256B_ALIGNMENT: Prevents false sharing across replicas
//! #VERIFY_ALIGNMENT: QuorumReadCapsule::new() enforces 256B alignment

use atomic_capsule::network::quorum_read::QuorumReadCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box as bh;

/// B32 Benchmark: set_generation latency (K56)
///
/// **Target**: <5ns (single atomic store, Relaxed ordering)
/// **Reality Check**: Atomic stores are cheap, no CAS overhead
fn bench_set_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("quorum_read/set_generation");
    group.throughput(Throughput::Elements(1));

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    group.bench_function("set_generation_single", |b| {
        let mut gen = 0u64;
        b.iter(|| {
            capsule.set_generation(black_box(0), black_box(gen));
            gen = gen.wrapping_add(1);
            black_box(&capsule);
        });
    });

    group.bench_function("set_generation_all_replicas", |b| {
        let mut gen = 0u64;
        b.iter(|| {
            capsule.set_generation(0, gen);
            capsule.set_generation(1, gen + 1);
            capsule.set_generation(2, gen + 2);
            gen = gen.wrapping_add(3);
            black_box(&capsule);
        });
    });

    group.finish();
}

/// B32 Benchmark: mark_completed latency (K56)
///
/// **Target**: <10ns (atomic fetch_or, Relaxed ordering)
/// **Reality Check**: Atomic bitwise OR is lockfree and fast
fn bench_mark_completed(c: &mut Criterion) {
    let mut group = c.benchmark_group("quorum_read/mark_completed");
    group.throughput(Throughput::Elements(1));

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    group.bench_function("mark_completed_single", |b| {
        b.iter(|| {
            capsule.reset();
            capsule.mark_completed(black_box(0));
            black_box(&capsule);
        });
    });

    group.bench_function("mark_completed_quorum", |b| {
        b.iter(|| {
            capsule.reset();
            capsule.mark_completed(0);
            capsule.mark_completed(1);
            black_box(&capsule);
        });
    });

    group.bench_function("mark_completed_all", |b| {
        b.iter(|| {
            capsule.reset();
            capsule.mark_completed(0);
            capsule.mark_completed(1);
            capsule.mark_completed(2);
            black_box(&capsule);
        });
    });

    group.finish();
}

/// B32 Benchmark: select_winner latency (K56)
///
/// **Target**: <15ns (3 atomic loads + comparison)
/// **Reality Check**: Max-finding is cheap with only 3 values
fn bench_select_winner(c: &mut Criterion) {
    let mut group = c.benchmark_group("quorum_read/select_winner");
    group.throughput(Throughput::Elements(1));

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    // Setup different generation scenarios
    group.bench_function("select_winner_identical", |b| {
        capsule.set_generation(0, 10);
        capsule.set_generation(1, 10);
        capsule.set_generation(2, 10);

        b.iter(|| {
            let result = capsule.select_winner();
            black_box(result);
        });
    });

    group.bench_function("select_winner_divergent", |b| {
        capsule.set_generation(0, 5);
        capsule.set_generation(1, 20);
        capsule.set_generation(2, 15);

        b.iter(|| {
            let result = capsule.select_winner();
            black_box(result);
        });
    });

    group.bench_function("select_winner_sequential", |b| {
        capsule.set_generation(0, 1);
        capsule.set_generation(1, 2);
        capsule.set_generation(2, 3);

        b.iter(|| {
            let result = capsule.select_winner();
            black_box(result);
        });
    });

    group.finish();
}

/// B32 Benchmark: has_quorum latency (K56)
///
/// **Target**: <5ns (atomic load + popcount)
/// **Reality Check**: Popcount is single-cycle POPCNT instruction
fn bench_has_quorum(c: &mut Criterion) {
    let mut group = c.benchmark_group("quorum_read/has_quorum");
    group.throughput(Throughput::Elements(1));

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    group.bench_function("has_quorum_zero", |b| {
        capsule.reset();

        b.iter(|| {
            let result = capsule.has_quorum();
            black_box(result);
        });
    });

    group.bench_function("has_quorum_one", |b| {
        capsule.reset();
        capsule.mark_completed(0);

        b.iter(|| {
            let result = capsule.has_quorum();
            black_box(result);
        });
    });

    group.bench_function("has_quorum_two", |b| {
        capsule.reset();
        capsule.mark_completed(0);
        capsule.mark_completed(1);

        b.iter(|| {
            let result = capsule.has_quorum();
            black_box(result);
        });
    });

    group.bench_function("has_quorum_three", |b| {
        capsule.reset();
        capsule.mark_completed(0);
        capsule.mark_completed(1);
        capsule.mark_completed(2);

        b.iter(|| {
            let result = capsule.has_quorum();
            black_box(result);
        });
    });

    group.finish();
}

/// B32 Benchmark: reset latency (K56)
///
/// **Target**: <15ns (4 atomic stores, Relaxed ordering)
/// **Reality Check**: Multiple relaxed stores are cheap
fn bench_reset(c: &mut Criterion) {
    let mut group = c.benchmark_group("quorum_read/reset");
    group.throughput(Throughput::Elements(1));

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    group.bench_function("reset_clean", |b| {
        b.iter(|| {
            capsule.reset();
            black_box(&capsule);
        });
    });

    group.bench_function("reset_after_quorum", |b| {
        b.iter(|| {
            capsule.mark_completed(0);
            capsule.mark_completed(1);
            capsule.set_generation(0, 10);
            capsule.set_generation(1, 20);
            capsule.select_winner();

            capsule.reset();
            black_box(&capsule);
        });
    });

    group.finish();
}

/// B32 Benchmark: Full quorum read simulation (K58)
///
/// **Target**: <100ns (local coordination overhead)
/// **Reality Check**: Network latency (5-10ms) dominates, coordination is negligible
///
/// ## Simulation Steps
///
/// 1. Reset capsule (<15ns)
/// 2. Set 3 generation counters (<15ns)
/// 3. Mark 2 replicas completed (<20ns)
/// 4. Check quorum (<5ns)
/// 5. Select winner (<15ns)
/// 6. Get winner result (<5ns)
///
/// **Total Local Overhead**: <75ns (vs 5-10ms network latency = 0.001% overhead)
fn bench_full_quorum_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("quorum_read/full_simulation");
    group.throughput(Throughput::Elements(1));

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    group.bench_function("quorum_read_success", |b| {
        let mut gen = 0u64;

        b.iter(|| {
            // Step 1: Reset (<15ns)
            capsule.reset();

            // Step 2: Set generations (<15ns for 3 replicas)
            capsule.set_generation(0, gen);
            capsule.set_generation(1, gen + 1);
            capsule.set_generation(2, gen + 2);

            // Step 3: Mark quorum completed (<20ns for 2 replicas)
            capsule.mark_completed(1);
            capsule.mark_completed(2);

            // Step 4: Check quorum (<5ns)
            let has_quorum = capsule.has_quorum();
            assert!(has_quorum);

            // Step 5: Select winner (<15ns)
            let (winner_idx, winner_gen) = capsule.select_winner();

            // Step 6: Get result (<5ns)
            black_box((winner_idx, winner_gen));

            gen = gen.wrapping_add(3);
        });
    });

    group.bench_function("quorum_read_failure", |b| {
        let mut gen = 0u64;

        b.iter(|| {
            // Step 1: Reset (<15ns)
            capsule.reset();

            // Step 2: Set generations (<15ns)
            capsule.set_generation(0, gen);
            capsule.set_generation(1, gen + 1);
            capsule.set_generation(2, gen + 2);

            // Step 3: Only 1 replica completed (<10ns)
            capsule.mark_completed(0);

            // Step 4: Check quorum - should fail (<5ns)
            let has_quorum = capsule.has_quorum();
            assert!(!has_quorum);

            black_box(has_quorum);

            gen = gen.wrapping_add(3);
        });
    });

    group.bench_function("quorum_read_with_failures", |b| {
        let mut gen = 0u64;

        b.iter(|| {
            // Step 1: Reset (<15ns)
            capsule.reset();

            // Step 2: Set generations (<15ns)
            capsule.set_generation(0, gen);
            capsule.set_generation(1, gen + 1);
            capsule.set_generation(2, gen + 2);

            // Step 3: 2 completed, 1 failed (<30ns)
            capsule.mark_completed(0);
            capsule.mark_failed(1);
            capsule.mark_completed(2);

            // Step 4: Check quorum - should succeed (2/3) (<5ns)
            let has_quorum = capsule.has_quorum();
            assert!(has_quorum);

            // Step 5: Select winner (<15ns)
            let (winner_idx, winner_gen) = capsule.select_winner();

            black_box((winner_idx, winner_gen));

            gen = gen.wrapping_add(3);
        });
    });

    group.finish();
}

/// B32 Benchmark: Quorum read with different replica counts (K58)
///
/// **Reality Check**: K58 states quorum reads are 2-3× network RTT minimum
/// This benchmark validates local coordination overhead is negligible (<0.001%)
fn bench_quorum_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("quorum_read/scaling");

    for num_replicas in &[1, 2, 3] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_replicas),
            num_replicas,
            |b, &num_replicas| {
                let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();
                let mut gen = 0u64;

                b.iter(|| {
                    capsule.reset();

                    // Set generations for all replicas
                    for i in 0..3 {
                        capsule.set_generation(i, gen + i as u64);
                    }

                    // Mark only num_replicas as completed
                    for i in 0..num_replicas {
                        capsule.mark_completed(i);
                    }

                    let has_quorum = capsule.has_quorum();

                    if has_quorum {
                        let (winner_idx, winner_gen) = capsule.select_winner();
                        black_box((winner_idx, winner_gen));
                    }

                    gen = gen.wrapping_add(3);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_set_generation,
    bench_mark_completed,
    bench_select_winner,
    bench_has_quorum,
    bench_reset,
    bench_full_quorum_read,
    bench_quorum_scaling,
);

criterion_main!(benches);
