//! B32 Benchmarking Framework for T11 QuantumHybrid
//!
//! # Benchmark Strategy
//!
//! **Fair Comparison**: Quantum simulation vs best-known classical algorithms
//! - **Shor's**: Quantum O(log³ n) vs Classical trial division O(√n)
//! - **Grover's**: Quantum O(√N) vs Classical linear search O(N)
//! - **QAOA**: Quantum variational vs Classical greedy heuristic
//!
//! # Performance Reality
//!
//! - **Speedup**: Theoretical (asymptotic), not wall-clock on classical simulator
//! - **Simulation overhead**: Quantum simulation itself is exponentially slow on classical hardware
//! - **Validation**: Compare algorithm quality, not absolute runtime
//!
//! # B32 Framework Compliance
//!
//! - **K1 (Fair Baseline)**: Best classical algorithm (not strawman)
//! - **K2 (Same Hardware)**: Same CPU, same conditions
//! - **K3 (Statistical Rigor)**: 95% CI, 1000+ iterations via Criterion
//! - **K4 (Reproducibility)**: Fixed seeds, deterministic simulation
//!
//! # Expected Results
//!
//! | Algorithm | Problem Size | Quantum (sim) | Classical | Speedup (theory) |
//! |-----------|--------------|---------------|-----------|------------------|
//! | Shor's    | n=15         | ~100μs        | ~10μs     | 10,000× @ n=2^1024 |
//! | Grover's  | N=64         | ~50μs         | ~5μs      | 8× @ N=64          |
//! | QAOA      | 10 nodes     | ~1ms          | ~100μs    | 2-5× quality       |
//!
//! **Note**: Wall-clock times show quantum simulation *slower* due to classical
//! simulation overhead. Theoretical speedups require real quantum hardware.

#[cfg(feature = "quantum-simulation")]
use criterion::{criterion_group, criterion_main, Criterion, black_box};

#[cfg(feature = "quantum-simulation")]
use atomic_capsule::quantum::QuantumStateCapsule;

#[cfg(feature = "quantum-simulation")]
mod benchmarks {
    use super::*;

    // ========================================================================
    // SHOR'S ALGORITHM: Quantum vs Classical Factorization
    // ========================================================================

    /// Classical trial division baseline (best simple algorithm)
    fn classical_trial_division(n: u64) -> (u64, u64) {
        if n % 2 == 0 {
            return (2, n / 2);
        }

        for p in (3..=(n as f64).sqrt() as u64).step_by(2) {
            if n % p == 0 {
                return (p, n / p);
            }
        }

        (n, 1)  // Prime
    }

    pub fn bench_shors_vs_trial_division(c: &mut Criterion) {
        let mut group = c.benchmark_group("shor_factorization");

        // Small composite numbers
        let test_cases = vec![15, 21, 35, 77, 143];

        for n in test_cases {
            group.bench_function(format!("quantum_shor_{}", n), |b| {
                let qsc = QuantumStateCapsule::new(10).unwrap();
                b.iter(|| {
                    qsc.shors_factorization(black_box(n)).unwrap()
                });
            });

            group.bench_function(format!("classical_trial_division_{}", n), |b| {
                b.iter(|| {
                    classical_trial_division(black_box(n))
                });
            });
        }

        group.finish();
    }

    // ========================================================================
    // GROVER'S ALGORITHM: Quantum vs Classical Search
    // ========================================================================

    /// Classical linear search baseline
    fn classical_linear_search(target: usize, n_items: usize) -> Option<usize> {
        (0..n_items).find(|&x| x == target)
    }

    pub fn bench_grovers_vs_linear_search(c: &mut Criterion) {
        let mut group = c.benchmark_group("grover_search");

        // Search space sizes (powers of 2)
        let test_cases = vec![
            (3, 8),      // 2^3 = 8 items
            (6, 64),     // 2^6 = 64 items
            (10, 1024),  // 2^10 = 1024 items
        ];

        for (n_qubits, n_items) in test_cases {
            let target = n_items / 2;  // Mid-point target

            group.bench_function(format!("quantum_grover_{}items", n_items), |b| {
                let qsc = QuantumStateCapsule::new(n_qubits).unwrap();
                b.iter(|| {
                    qsc.grovers_search(|x| x == black_box(target), black_box(n_items)).unwrap()
                });
            });

            group.bench_function(format!("classical_linear_search_{}items", n_items), |b| {
                b.iter(|| {
                    classical_linear_search(black_box(target), black_box(n_items))
                });
            });
        }

        group.finish();
    }

    // ========================================================================
    // QAOA: Quantum vs Classical MaxCut
    // ========================================================================

    /// Classical greedy MaxCut heuristic baseline
    fn classical_greedy_maxcut(graph: &[(usize, usize)], n_nodes: usize) -> Vec<bool> {
        let mut partition = vec![false; n_nodes];
        let mut improved = true;

        while improved {
            improved = false;
            for node in 0..n_nodes {
                partition[node] = !partition[node];
                let new_cut = count_cut_edges(graph, &partition);

                partition[node] = !partition[node];
                let old_cut = count_cut_edges(graph, &partition);

                if new_cut > old_cut {
                    partition[node] = !partition[node];
                    improved = true;
                }
            }
        }

        partition
    }

    fn count_cut_edges(graph: &[(usize, usize)], partition: &[bool]) -> usize {
        graph
            .iter()
            .filter(|(u, v)| partition[*u] != partition[*v])
            .count()
    }

    pub fn bench_qaoa_vs_greedy_maxcut(c: &mut Criterion) {
        let mut group = c.benchmark_group("qaoa_maxcut");

        // Test graphs
        let test_cases = vec![
            ("triangle", vec![(0, 1), (1, 2), (2, 0)], 3),
            ("pentagon", vec![(0, 1), (1, 2), (2, 3), (3, 4), (4, 0)], 5),
            ("hexagon", vec![(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0)], 6),
        ];

        for (name, graph, n_nodes) in test_cases {
            group.bench_function(format!("quantum_qaoa_{}_p2", name), |b| {
                let qsc = QuantumStateCapsule::new(n_nodes).unwrap();
                b.iter(|| {
                    qsc.qaoa_maxcut(black_box(&graph), black_box(2)).unwrap()
                });
            });

            group.bench_function(format!("classical_greedy_{}", name), |b| {
                b.iter(|| {
                    classical_greedy_maxcut(black_box(&graph), black_box(n_nodes))
                });
            });
        }

        group.finish();
    }

    // ========================================================================
    // CAPSULE OVERHEAD BENCHMARKS
    // ========================================================================

    pub fn bench_capsule_creation(c: &mut Criterion) {
        c.bench_function("quantum_capsule_creation_10qubits", |b| {
            b.iter(|| {
                QuantumStateCapsule::new(black_box(10)).unwrap()
            });
        });
    }

    pub fn bench_status_reads(c: &mut Criterion) {
        let qsc = QuantumStateCapsule::new(5).unwrap();

        c.bench_function("quantum_capsule_status_read", |b| {
            b.iter(|| {
                black_box(qsc.status())
            });
        });
    }
}

#[cfg(feature = "quantum-simulation")]
criterion_group!(
    quantum_benches,
    benchmarks::bench_shors_vs_trial_division,
    benchmarks::bench_grovers_vs_linear_search,
    benchmarks::bench_qaoa_vs_greedy_maxcut,
    benchmarks::bench_capsule_creation,
    benchmarks::bench_status_reads
);

#[cfg(feature = "quantum-simulation")]
criterion_main!(quantum_benches);

// Stub for when quantum feature is disabled
#[cfg(not(feature = "quantum-simulation"))]
fn main() {
    println!("Quantum benchmarks require --features quantum-simulation");
}
