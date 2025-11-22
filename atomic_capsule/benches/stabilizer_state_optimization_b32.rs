use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use atomic_capsule::quantum::StabilizerStateCapsule;

/// B32 Benchmark Suite for Stabilizer State Optimization
///
/// **Optimization Stages**:
/// - Stage 1 (In-place XOR): 10× speedup target (150ns → 15ns)
/// - Stage 2 (SIMD): 80× total speedup target (150ns → <2ns)
///
/// **B32 Framework Compliance**:
/// - Fair baselines: Measure actual current implementation
/// - 1000+ iterations: Criterion default with sample_size(1000)
/// - Same hardware: All benchmarks on same machine
/// - 95% CI: Criterion default confidence intervals
/// - Reproducibility: Run 3 times, document variance

/// Benchmark rowsum with in-place XOR (Stage 1 optimization)
///
/// **Expected Performance**: 10× speedup vs cloning baseline
/// **Target**: <15ns per rowsum @ 20 qubits
fn bench_rowsum_stage1(c: &mut Criterion) {
    let mut group = c.benchmark_group("rowsum_stage1_inplace");
    group.sample_size(1000); // B32: 1000+ iterations

    for num_qubits in [10, 20, 50, 100].iter() {
        let mut state = StabilizerStateCapsule::new(*num_qubits).unwrap();

        group.bench_with_input(
            BenchmarkId::new("inplace", num_qubits),
            num_qubits,
            |b, _| b.iter(|| {
                // Directly test apply_h which uses rowsum internally
                // This ensures we measure the real use case
                state.apply_h(black_box(0)).unwrap();
            }),
        );
    }
    group.finish();
}

/// Benchmark rowsum with SIMD XOR (Stage 2 optimization)
///
/// **Expected Performance**: 80× total speedup vs cloning baseline
/// **Target**: <2ns per rowsum @ 20 qubits
#[cfg(feature = "quantum-stabilizer-simd")]
fn bench_rowsum_stage2(c: &mut Criterion) {
    let mut group = c.benchmark_group("rowsum_stage2_simd");
    group.sample_size(1000); // B32: 1000+ iterations

    for num_qubits in [10, 20, 50, 100].iter() {
        let mut state = StabilizerStateCapsule::new(*num_qubits).unwrap();

        group.bench_with_input(
            BenchmarkId::new("simd", num_qubits),
            num_qubits,
            |b, _| b.iter(|| {
                // apply_h with SIMD rowsum
                state.apply_h(black_box(0)).unwrap();
            }),
        );
    }
    group.finish();
}

/// Benchmark gate throughput (1000 H gates)
///
/// **Expected Performance**:
/// - Baseline: 10K gates/sec
/// - Stage 1: 100K gates/sec (10×)
/// - Stage 2: 800K gates/sec (80×)
fn bench_gate_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_throughput");
    group.sample_size(100); // Fewer samples for longer benchmark

    #[cfg(not(feature = "quantum-stabilizer-simd"))]
    {
        group.bench_function("stage1_1000_gates", |b| {
            let mut state = StabilizerStateCapsule::new(20).unwrap();
            b.iter(|| {
                // 1000 H gates (Stage 1: in-place XOR)
                for i in 0..1000 {
                    state.apply_h(black_box(i % 20)).unwrap();
                }
            });
        });
    }

    #[cfg(feature = "quantum-stabilizer-simd")]
    {
        group.bench_function("stage2_1000_gates", |b| {
            let mut state = StabilizerStateCapsule::new(20).unwrap();
            b.iter(|| {
                // 1000 H gates (Stage 2: SIMD XOR)
                for i in 0..1000 {
                    state.apply_h(black_box(i % 20)).unwrap();
                }
            });
        });
    }

    group.finish();
}

/// Benchmark CNOT gate (uses 2 rowsum calls)
///
/// **Scaling**: CNOT = 2× rowsum latency
fn bench_cnot_gate(c: &mut Criterion) {
    let mut group = c.benchmark_group("cnot_gate");
    group.sample_size(1000);

    for num_qubits in [10, 20, 50, 100].iter() {
        let mut state = StabilizerStateCapsule::new(*num_qubits).unwrap();

        group.bench_with_input(
            BenchmarkId::new("cnot", num_qubits),
            num_qubits,
            |b, _| b.iter(|| {
                // CNOT uses 2 rowsum calls internally
                state.apply_cnot(black_box(0), black_box(1)).unwrap();
            }),
        );
    }
    group.finish();
}

#[cfg(not(feature = "quantum-stabilizer-simd"))]
criterion_group!(
    benches,
    bench_rowsum_stage1,
    bench_gate_throughput,
    bench_cnot_gate
);

#[cfg(feature = "quantum-stabilizer-simd")]
criterion_group!(
    benches,
    bench_rowsum_stage1,
    bench_rowsum_stage2,
    bench_gate_throughput,
    bench_cnot_gate
);

criterion_main!(benches);
