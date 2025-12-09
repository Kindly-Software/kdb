// GPU Sparse Matrix Benchmarks (B32)
// Baseline: CPU scipy.sparse (via Rust nalgebra-sparse or custom impl)
// Target: 10-100× speedup for sparse operations

#![cfg(all(not(target_env = "msvc"), feature = "std"))]

use atomic_capsule::gpu::kernels::{GpuSparseCapsule, SparseFormat};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ============================================================================
// BENCHMARK GROUPS
// ============================================================================

/// Benchmark: Sparse matrix creation
fn bench_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_sparse_creation");

    for &(rows, cols, sparsity_pct) in &[(1000, 1000, 1), (1000, 1000, 5), (1000, 1000, 10)] {
        let nnz = (rows * cols * sparsity_pct) / 100;

        group.throughput(Throughput::Elements(nnz as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x{}_{}%", rows, cols, sparsity_pct)),
            &(rows, cols, nnz),
            |b, &(rows, cols, nnz)| {
                b.iter(|| {
                    let sparse = GpuSparseCapsule::new(
                        black_box(rows),
                        black_box(cols),
                        black_box(nnz),
                        SparseFormat::CSR,
                        0,
                    )
                    .unwrap();
                    black_box(sparse)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: COO → CSR format conversion
fn bench_coo_to_csr(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_sparse_coo_to_csr");

    for &(rows, cols, sparsity_pct) in &[(1000, 1000, 1), (10000, 10000, 1), (10000, 10000, 5)] {
        let nnz = (rows * cols * sparsity_pct) / 100;

        let sparse = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::COO, 0).unwrap();

        group.throughput(Throughput::Elements(nnz as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x{}_{}%_{}nnz", rows, cols, sparsity_pct, nnz)),
            &sparse,
            |b, sparse| {
                b.iter(|| {
                    // Reset to COO before each iteration
                    let _ = sparse.csr_to_coo();
                    let result = sparse.coo_to_csr();
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: CSR → COO format conversion
fn bench_csr_to_coo(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_sparse_csr_to_coo");

    for &(rows, cols, sparsity_pct) in &[(1000, 1000, 1), (10000, 10000, 1), (10000, 10000, 5)] {
        let nnz = (rows * cols * sparsity_pct) / 100;

        let sparse = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::CSR, 0).unwrap();

        group.throughput(Throughput::Elements(nnz as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x{}_{}%_{}nnz", rows, cols, sparsity_pct, nnz)),
            &sparse,
            |b, sparse| {
                b.iter(|| {
                    // Reset to CSR before each iteration
                    let _ = sparse.coo_to_csr();
                    let result = sparse.csr_to_coo();
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: SpMV (Sparse Matrix-Vector Multiplication)
/// Baseline: CPU scalar implementation O(nnz)
/// Target: 10-50× speedup (bandwidth-limited)
fn bench_spmv(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_sparse_spmv");

    for &(rows, cols, sparsity_pct) in &[(1000, 1000, 1), (10000, 10000, 1), (10000, 10000, 5)] {
        let nnz = (rows * cols * sparsity_pct) / 100;

        let sparse = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::CSR, 0).unwrap();

        // Throughput: 2 × nnz (one read from matrix, one read from vector)
        group.throughput(Throughput::Elements((2 * nnz) as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!(
                "{}x{}_{}%_{}nnz_CSR",
                rows, cols, sparsity_pct, nnz
            )),
            &sparse,
            |b, sparse| {
                b.iter(|| {
                    let result = sparse.spmv_f32(
                        black_box(0x1000),
                        black_box(0x2000),
                        black_box(1.0),
                        black_box(0.0),
                    );
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: SpGEMM (Sparse Matrix-Matrix Multiplication)
/// Baseline: CPU scalar implementation O(nnz_A × avg_nnz_per_row_B)
/// Target: 50-200× speedup (compute-bound, hash-based accumulation)
fn bench_spgemm(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_sparse_spgemm");

    for &(m, k, n, sparsity_pct) in &[
        (100, 100, 100, 5),
        (500, 500, 500, 1),
        (1000, 1000, 1000, 1),
    ] {
        let nnz_a = (m * k * sparsity_pct) / 100;
        let nnz_b = (k * n * sparsity_pct) / 100;
        let nnz_c = (m * n * sparsity_pct * 2) / 100; // Estimate: 2× sparsity

        let a = GpuSparseCapsule::new(m, k, nnz_a, SparseFormat::CSR, 0).unwrap();
        let b = GpuSparseCapsule::new(k, n, nnz_b, SparseFormat::CSR, 0).unwrap();
        let mut c = GpuSparseCapsule::new(m, n, nnz_c, SparseFormat::CSR, 0).unwrap();

        // Throughput: nnz_A + nnz_B (reads)
        group.throughput(Throughput::Elements((nnz_a + nnz_b) as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!(
                "{}x{}x{}_{}%_{}+{}nnz",
                m, k, n, sparsity_pct, nnz_a, nnz_b
            )),
            &(&a, &b, &mut c),
            |bench, (a, b, c)| {
                bench.iter(|| {
                    let result = a.spgemm(black_box(b), black_box(*c));
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Sparse triangular solve
/// Baseline: CPU forward/backward substitution O(nnz)
/// Target: 10-30× speedup (level-scheduling parallelism)
fn bench_triangular_solve(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_sparse_triangular_solve");

    for &(n, sparsity_pct) in &[(256, 5), (1000, 1), (1000, 5)] {
        let nnz = (n * n * sparsity_pct) / 100;

        let sparse = GpuSparseCapsule::new(n, n, nnz, SparseFormat::CSR, 0).unwrap();

        // Throughput: nnz (reads from matrix) + n (reads/writes from vectors)
        group.throughput(Throughput::Elements((nnz + n) as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x{}_{}%_{}nnz_lower", n, n, sparsity_pct, nnz)),
            &sparse,
            |b, sparse| {
                b.iter(|| {
                    let result = sparse.sparse_triangular_solve_f32(
                        black_box(0x1000),
                        black_box(0x2000),
                        black_box(true),
                    );
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Snapshot performance (atomic reads)
fn bench_snapshot(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_sparse_snapshot");

    let sparse = GpuSparseCapsule::new(10000, 10000, 100000, SparseFormat::CSR, 0).unwrap();

    group.bench_function("snapshot_10kx10k_100k", |b| {
        b.iter(|| {
            let snap = sparse.snapshot();
            black_box(snap)
        });
    });

    group.finish();
}

/// Benchmark: Concurrent format conversions (lockfree coordination)
fn bench_concurrent_conversions(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("gpu_sparse_concurrent");

    let sparse = Arc::new(GpuSparseCapsule::new(1000, 1000, 5000, SparseFormat::COO, 0).unwrap());

    group.bench_function("concurrent_conversions_8threads", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let sparse = Arc::clone(&sparse);
                    thread::spawn(move || {
                        for _ in 0..10 {
                            let _ = sparse.coo_to_csr();
                            let _ = sparse.csr_to_coo();
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

/// Benchmark: Sparsity pattern effects
/// Compare performance for different sparsity patterns:
/// - Uniform: random non-zeros
/// - Diagonal: banded structure
/// - Block: dense blocks
fn bench_sparsity_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_sparse_patterns");

    let rows = 10000;
    let cols = 10000;

    for &sparsity_pct in &[1, 5, 10] {
        let nnz = (rows * cols * sparsity_pct) / 100;

        let sparse = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::CSR, 0).unwrap();

        group.throughput(Throughput::Elements(nnz as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("uniform_{}%", sparsity_pct)),
            &sparse,
            |b, sparse| {
                b.iter(|| {
                    let result = sparse.spmv_f32(
                        black_box(0x1000),
                        black_box(0x2000),
                        black_box(1.0),
                        black_box(0.0),
                    );
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(10))
        .warm_up_time(std::time::Duration::from_secs(3));
    targets =
        bench_creation,
        bench_coo_to_csr,
        bench_csr_to_coo,
        bench_spmv,
        bench_spgemm,
        bench_triangular_solve,
        bench_snapshot,
        bench_concurrent_conversions,
        bench_sparsity_patterns,
);

criterion_main!(benches);
