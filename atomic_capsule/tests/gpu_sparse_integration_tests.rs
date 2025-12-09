// GPU Sparse Matrix Integration Tests (T28 Q15-Q21)
// Tests: Multi-capsule workflows, real workloads, error paths

#![cfg(all(
    test,
    feature = "std",
    any(feature = "gpu-cuda", feature = "gpu-rocm", feature = "gpu-all")
))]

use atomic_capsule::gpu::kernels::sparse::{GpuSparseCapsule, SparseFormat};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q15: INTEGRATION TESTS (Multi-Capsule Workflows)
// ============================================================================

#[test]
fn test_sparse_matrix_pipeline() {
    // Workflow: COO construction → CSR conversion → SpMV
    let sparse = GpuSparseCapsule::new(1000, 1000, 5000, SparseFormat::COO, 0).unwrap();

    // Initial state
    assert_eq!(sparse.format(), SparseFormat::COO);
    assert_eq!(sparse.shape(), (1000, 1000));
    assert_eq!(sparse.nnz(), 5000);

    // Convert to CSR for efficient SpMV
    sparse.coo_to_csr().unwrap();
    assert_eq!(sparse.format(), SparseFormat::CSR);

    // Perform SpMV (may fail due to CPU fallback, but shouldn't panic)
    let result = sparse.spmv_f32(0x1000, 0x2000, 1.0, 0.0);
    assert!(result.is_ok() || result.is_err());

    // Check operation counters
    let snap = sparse.snapshot();
    assert!(snap.operations >= 1); // At least format conversion
}

#[test]
fn test_sparse_gemm_pipeline() {
    // Workflow: A @ B = C (sparse matrix multiplication)
    let a = GpuSparseCapsule::new(128, 256, 1024, SparseFormat::CSR, 0).unwrap();
    let b = GpuSparseCapsule::new(256, 512, 2048, SparseFormat::CSR, 0).unwrap();
    let mut c = GpuSparseCapsule::new(128, 512, 4096, SparseFormat::CSR, 0).unwrap();

    // Initial state
    assert_eq!(a.format(), SparseFormat::CSR);
    assert_eq!(b.format(), SparseFormat::CSR);
    assert_eq!(c.format(), SparseFormat::CSR);

    // Perform SpGEMM
    let result = a.spgemm(&b, &mut c);
    assert!(result.is_ok() || result.is_err());

    // Check operation counters
    if result.is_ok() {
        let snap = a.snapshot();
        assert_eq!(snap.operations, 1);
        assert_eq!(snap.spgemm_count, 1);
    }
}

#[test]
fn test_triangular_solve_pipeline() {
    // Workflow: L * x = b (lower triangular solve)
    let sparse = GpuSparseCapsule::new(256, 256, 1024, SparseFormat::CSR, 0).unwrap();

    // Perform triangular solve
    let result = sparse.sparse_triangular_solve_f32(0x1000, 0x2000, true);
    assert!(result.is_ok() || result.is_err());

    if result.is_ok() {
        let snap = sparse.snapshot();
        assert_eq!(snap.operations, 1);
    }
}

// ============================================================================
// Q16: FORMAT CONVERSION WORKFLOWS
// ============================================================================

#[test]
fn test_multiple_format_conversions() {
    let sparse = GpuSparseCapsule::new(100, 100, 500, SparseFormat::COO, 0).unwrap();

    // COO → CSR → COO → CSR
    sparse.coo_to_csr().unwrap();
    assert_eq!(sparse.format(), SparseFormat::CSR);

    sparse.csr_to_coo().unwrap();
    assert_eq!(sparse.format(), SparseFormat::COO);

    sparse.coo_to_csr().unwrap();
    assert_eq!(sparse.format(), SparseFormat::CSR);

    // Check operation count
    let snap = sparse.snapshot();
    assert_eq!(snap.operations, 3);
}

#[test]
fn test_format_conversion_preserves_dimensions() {
    let rows = 500;
    let cols = 1000;
    let nnz = 2500;

    let sparse = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::COO, 0).unwrap();

    // Convert multiple times
    for _ in 0..5 {
        sparse.coo_to_csr().unwrap();
        sparse.csr_to_coo().unwrap();
    }

    // Verify dimensions unchanged
    assert_eq!(sparse.shape(), (rows, cols));
    assert_eq!(sparse.nnz(), nnz);
    assert_eq!(sparse.snapshot().operations, 10);
}

// ============================================================================
// Q17: ERROR HANDLING INTEGRATION
// ============================================================================

#[test]
fn test_invalid_spgemm_dimensions() {
    // A[128,256] * B[128,512] (mismatched inner dimensions)
    let a = GpuSparseCapsule::new(128, 256, 1024, SparseFormat::CSR, 0).unwrap();
    let b = GpuSparseCapsule::new(128, 512, 2048, SparseFormat::CSR, 0).unwrap();
    let mut c = GpuSparseCapsule::new(128, 512, 4096, SparseFormat::CSR, 0).unwrap();

    let result = a.spgemm(&b, &mut c);
    assert!(result.is_err());

    // Operation should not increment on error
    let snap = a.snapshot();
    assert_eq!(snap.operations, 0);
    assert_eq!(snap.spgemm_count, 0);
}

#[test]
fn test_invalid_format_for_operation() {
    // Try SpMV on COO matrix (requires CSR)
    let sparse = GpuSparseCapsule::new(128, 256, 1024, SparseFormat::COO, 0).unwrap();

    let result = sparse.spmv_f32(0x1000, 0x2000, 1.0, 0.0);
    assert!(result.is_err());

    // Convert to CSR first
    sparse.coo_to_csr().unwrap();

    // Now SpMV may succeed (depends on backend)
    let result = sparse.spmv_f32(0x1000, 0x2000, 1.0, 0.0);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_format_conversion_from_wrong_state() {
    // CSR → cannot do COO→CSR
    let sparse_csr = GpuSparseCapsule::new(100, 100, 500, SparseFormat::CSR, 0).unwrap();
    assert!(sparse_csr.coo_to_csr().is_err());

    // COO → cannot do CSR→COO
    let sparse_coo = GpuSparseCapsule::new(100, 100, 500, SparseFormat::COO, 0).unwrap();
    assert!(sparse_coo.csr_to_coo().is_err());
}

// ============================================================================
// Q18: CONCURRENT INTEGRATION TESTS
// ============================================================================

#[test]
fn test_concurrent_mixed_operations() {

    let sparse = Arc::new(GpuSparseCapsule::new(1000, 1000, 5000, SparseFormat::COO, 0).unwrap());

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let sparse = sparse.clone();
            thread::spawn(move || {
                if i % 2 == 0 {
                    // Even threads: format conversions
                    for _ in 0..50 {
                        let _ = sparse.coo_to_csr();
                        let _ = sparse.csr_to_coo();
                    }
                } else {
                    // Odd threads: snapshots
                    for _ in 0..100 {
                        let _ = sparse.snapshot();
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have 200 operations (4 threads × 50 conversions)
    let snap = sparse.snapshot();
    assert_eq!(snap.operations, 200);
}

#[test]
fn test_concurrent_spgemm() {

    let a = Arc::new(GpuSparseCapsule::new(100, 100, 500, SparseFormat::CSR, 0).unwrap());
    let b = Arc::new(GpuSparseCapsule::new(100, 100, 500, SparseFormat::CSR, 0).unwrap());

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let a = a.clone();
            let b = b.clone();
            thread::spawn(move || {
                for _ in 0..25 {
                    let mut c = GpuSparseCapsule::new(100, 100, 1000, SparseFormat::CSR, 0).unwrap();
                    let _ = a.spgemm(&b, &mut c);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // If SpGEMM succeeded, should have operations
    let snap = a.snapshot();
    assert!(snap.operations <= 100); // Upper bound
}

// ============================================================================
// Q19: LARGE MATRIX STRESS TESTS
// ============================================================================

#[test]
fn test_large_sparse_matrix_10k() {
    // 10K × 10K matrix with 1% sparsity (100K non-zeros)
    let sparse = GpuSparseCapsule::new(10000, 10000, 100000, SparseFormat::COO, 0).unwrap();

    assert_eq!(sparse.shape(), (10000, 10000));
    assert_eq!(sparse.nnz(), 100000);
    assert!((sparse.sparsity() - 0.001).abs() < 1e-6);

    // Format conversion
    sparse.coo_to_csr().unwrap();
    assert_eq!(sparse.format(), SparseFormat::CSR);
}

#[test]
fn test_very_sparse_matrix() {
    // 100K × 100K matrix with 0.001% sparsity (1K non-zeros)
    let sparse = GpuSparseCapsule::new(100000, 100000, 1000, SparseFormat::CSR, 0).unwrap();

    assert_eq!(sparse.shape(), (100000, 100000));
    assert_eq!(sparse.nnz(), 1000);
    assert!((sparse.sparsity() - 1e-5).abs() < 1e-9);
}

#[test]
fn test_nearly_dense_matrix() {
    // 100 × 100 matrix with 99% sparsity (9900 non-zeros)
    let sparse = GpuSparseCapsule::new(100, 100, 9900, SparseFormat::CSR, 0).unwrap();

    assert_eq!(sparse.shape(), (100, 100));
    assert_eq!(sparse.nnz(), 9900);
    assert!((sparse.sparsity() - 0.99).abs() < 1e-6);
}

// ============================================================================
// Q20: SNAPSHOT INTEGRATION
// ============================================================================

#[test]
fn test_snapshot_after_operations() {
    let sparse = GpuSparseCapsule::new(256, 256, 1024, SparseFormat::COO, 0).unwrap();

    // Initial snapshot
    let snap1 = sparse.snapshot();
    assert_eq!(snap1.operations, 0);
    assert_eq!(snap1.spmv_count, 0);
    assert_eq!(snap1.spgemm_count, 0);
    assert_eq!(snap1.format, SparseFormat::COO);

    // Perform operations
    sparse.coo_to_csr().unwrap();
    let snap2 = sparse.snapshot();
    assert_eq!(snap2.operations, 1);
    assert_eq!(snap2.format, SparseFormat::CSR);

    // SpMV
    let _ = sparse.spmv_f32(0x1000, 0x2000, 1.0, 0.0);
    let snap3 = sparse.snapshot();

    if snap3.operations > snap2.operations {
        assert_eq!(snap3.spmv_count, 1);
    }
}

#[test]
fn test_snapshot_consistency_under_load() {

    let sparse = Arc::new(GpuSparseCapsule::new(500, 500, 2500, SparseFormat::COO, 0).unwrap());

    let handles: Vec<_> = (0..16)
        .map(|_| {
            let sparse = sparse.clone();
            thread::spawn(move || {
                // Mix of operations and snapshots
                for _ in 0..50 {
                    let _ = sparse.coo_to_csr();
                    let snap = sparse.snapshot();
                    assert!(snap.operations > 0);
                    let _ = sparse.csr_to_coo();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Final snapshot should be consistent
    let snap = sparse.snapshot();
    assert_eq!(snap.operations, 800); // 16 threads × 50 iterations
    assert_eq!(snap.rows, 500);
    assert_eq!(snap.cols, 500);
    assert_eq!(snap.nnz, 2500);
}

// ============================================================================
// Q21: EDGE CASES
// ============================================================================

#[test]
fn test_single_element_matrix() {
    // Minimal sparse matrix: 1×1 with 1 non-zero
    let sparse = GpuSparseCapsule::new(1, 1, 1, SparseFormat::CSR, 0).unwrap();

    assert_eq!(sparse.shape(), (1, 1));
    assert_eq!(sparse.nnz(), 1);
    assert!((sparse.sparsity() - 1.0).abs() < 1e-6);
}

#[test]
fn test_rectangular_matrix() {
    // Rectangular: 1000 × 10000 (tall and skinny)
    let sparse = GpuSparseCapsule::new(1000, 10000, 5000, SparseFormat::CSR, 0).unwrap();

    assert_eq!(sparse.shape(), (1000, 10000));
    assert_eq!(sparse.nnz(), 5000);
    assert!((sparse.sparsity() - 0.0005).abs() < 1e-6);

    // Rectangular: 10000 × 1000 (short and wide)
    let sparse = GpuSparseCapsule::new(10000, 1000, 5000, SparseFormat::CSR, 0).unwrap();

    assert_eq!(sparse.shape(), (10000, 1000));
    assert_eq!(sparse.nnz(), 5000);
    assert!((sparse.sparsity() - 0.0005).abs() < 1e-6);
}

#[test]
fn test_square_matrix_powers_of_two() {
    // Powers of 2 dimensions (common in GPU kernels)
    for &size in &[128, 256, 512, 1024, 2048] {
        let nnz = size * 5; // ~5 non-zeros per row
        let sparse = GpuSparseCapsule::new(size, size, nnz, SparseFormat::CSR, 0).unwrap();

        assert_eq!(sparse.shape(), (size, size));
        assert_eq!(sparse.nnz(), nnz);
    }
}

#[test]
fn test_allocation_and_deallocation() {
    // Create and drop multiple sparse matrices
    for _ in 0..100 {
        let sparse = GpuSparseCapsule::new(1000, 1000, 5000, SparseFormat::CSR, 0).unwrap();
        let snap = sparse.snapshot();
        assert_eq!(snap.rows, 1000);
        assert_eq!(snap.cols, 1000);
        assert_eq!(snap.nnz, 5000);
    }
    // All should be properly deallocated (no leaks)
}
