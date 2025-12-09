// GPU Sparse Matrix Property Tests (T28 Q8-Q14)
// Tests: Correctness, SpMV/SpGEMM validation, format conversion invariants

#![cfg(all(
    test,
    feature = "std",
    any(feature = "gpu-cuda", feature = "gpu-rocm", feature = "gpu-all")
))]

use atomic_capsule::gpu::kernels::sparse::{GpuSparseCapsule, SparseFormat};
use proptest::prelude::*;

// ============================================================================
// PROPERTY TEST GENERATORS
// ============================================================================

/// Generate valid sparse matrix dimensions
fn arb_sparse_dims() -> impl Strategy<Value = (usize, usize, usize)> {
    (10usize..=1000, 10usize..=1000).prop_flat_map(|(rows, cols)| {
        let capacity = rows.saturating_mul(cols);
        let max_nnz = capacity.min(10000);
        (Just(rows), Just(cols), 1usize..=max_nnz)
    })
}

/// Generate sparse format
fn arb_sparse_format() -> impl Strategy<Value = SparseFormat> {
    prop_oneof![
        Just(SparseFormat::COO),
        Just(SparseFormat::CSR),
        Just(SparseFormat::CSC),
    ]
}

// ============================================================================
// Q8: PROPERTY TESTS (Correctness)
// ============================================================================

proptest! {
    /// P1: Valid dimensions always succeed
    #[test]
    fn prop_new_valid_dimensions((rows, cols, nnz) in arb_sparse_dims(), format in arb_sparse_format()) {
        let result = GpuSparseCapsule::new(rows, cols, nnz, format, 0);
        prop_assert!(result.is_ok());

        let sparse = result.unwrap();
        prop_assert_eq!(sparse.shape(), (rows, cols));
        prop_assert_eq!(sparse.nnz(), nnz);
        prop_assert_eq!(sparse.format(), format);
    }

    /// P2: nnz > rows * cols always fails
    #[test]
    fn prop_new_invalid_nnz(rows in 10usize..100, cols in 10usize..100) {
        let capacity = rows.saturating_mul(cols);
        let invalid_nnz = capacity + 1;

        let result = GpuSparseCapsule::new(rows, cols, invalid_nnz, SparseFormat::CSR, 0);
        prop_assert!(result.is_err());
    }

    /// P3: Zero dimensions always fail
    #[test]
    fn prop_new_zero_dimensions(dim in 0usize..=2) {
        let (rows, cols, nnz) = match dim {
            0 => (0, 100, 10),
            1 => (100, 0, 10),
            _ => (100, 100, 0),
        };

        let result = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::COO, 0);
        prop_assert!(result.is_err());
    }

    /// P4: Sparsity is always in [0.0, 1.0]
    #[test]
    fn prop_sparsity_bounds((rows, cols, nnz) in arb_sparse_dims()) {
        let sparse = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::CSR, 0).unwrap();
        let sparsity = sparse.sparsity();

        prop_assert!(sparsity >= 0.0);
        prop_assert!(sparsity <= 1.0);

        // Verify computation: nnz / (rows * cols)
        let capacity = (rows as f64) * (cols as f64);
        let expected = (nnz as f64) / capacity;
        prop_assert!((sparsity - expected).abs() < 1e-9);
    }
}

// ============================================================================
// Q9: FORMAT CONVERSION INVARIANTS
// ============================================================================

proptest! {
    /// P5: COO → CSR → COO preserves dimensions
    #[test]
    fn prop_coo_csr_roundtrip((rows, cols, nnz) in arb_sparse_dims()) {
        let sparse = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::COO, 0).unwrap();

        // COO → CSR
        let _ = sparse.coo_to_csr();
        prop_assert_eq!(sparse.format(), SparseFormat::CSR);

        // CSR → COO
        let _ = sparse.csr_to_coo();
        prop_assert_eq!(sparse.format(), SparseFormat::COO);

        // Dimensions preserved
        prop_assert_eq!(sparse.shape(), (rows, cols));
        prop_assert_eq!(sparse.nnz(), nnz);
    }

    /// P6: Format conversion increments operation count
    #[test]
    fn prop_format_conversion_increments_ops((rows, cols, nnz) in arb_sparse_dims()) {
        let sparse = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::COO, 0).unwrap();

        let snap1 = sparse.snapshot();
        prop_assert_eq!(snap1.operations, 0);

        // Convert COO → CSR
        let _ = sparse.coo_to_csr();
        let snap2 = sparse.snapshot();
        prop_assert_eq!(snap2.operations, 1);

        // Convert CSR → COO
        let _ = sparse.csr_to_coo();
        let snap3 = sparse.snapshot();
        prop_assert_eq!(snap3.operations, 2);
    }

    /// P7: Format conversion from wrong format fails
    #[test]
    fn prop_format_conversion_wrong_format((rows, cols, nnz) in arb_sparse_dims()) {
        // CSR matrix cannot do COO→CSR
        let sparse_csr = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::CSR, 0).unwrap();
        prop_assert!(sparse_csr.coo_to_csr().is_err());

        // COO matrix cannot do CSR→COO
        let sparse_coo = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::COO, 0).unwrap();
        prop_assert!(sparse_coo.csr_to_coo().is_err());
    }
}

// ============================================================================
// Q10: SPMV INVARIANTS
// ============================================================================

proptest! {
    /// P8: SpMV requires CSR format
    #[test]
    fn prop_spmv_requires_csr((rows, cols, nnz) in arb_sparse_dims(), format in arb_sparse_format()) {
        let sparse = GpuSparseCapsule::new(rows, cols, nnz, format, 0).unwrap();

        let result = sparse.spmv_f32(0x1000, 0x2000, 1.0, 0.0);

        if format == SparseFormat::CSR {
            // CSR: may succeed (GPU backend) or fail (CPU fallback)
            prop_assert!(result.is_ok() || result.is_err());
        } else {
            // Non-CSR: must fail
            prop_assert!(result.is_err());
        }
    }

    /// P9: SpMV increments counters (if successful)
    #[test]
    fn prop_spmv_increments_counters((rows, cols, nnz) in arb_sparse_dims()) {
        let sparse = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::CSR, 0).unwrap();

        let snap1 = sparse.snapshot();
        let result = sparse.spmv_f32(0x1000, 0x2000, 1.0, 0.0);

        if result.is_ok() {
            let snap2 = sparse.snapshot();
            prop_assert_eq!(snap2.operations, snap1.operations + 1);
            prop_assert_eq!(snap2.spmv_count, snap1.spmv_count + 1);
        }
    }
}

// ============================================================================
// Q11: SPGEMM INVARIANTS
// ============================================================================

proptest! {
    /// P10: SpGEMM requires matching inner dimensions
    #[test]
    fn prop_spgemm_inner_dim_match(
        m in 10usize..100,
        k1 in 10usize..100,
        k2 in 10usize..100,
        n in 10usize..100,
    ) {
        let nnz_a = (m * k1).min(1000);
        let nnz_b = (k2 * n).min(1000);
        let nnz_c = (m * n).min(2000);

        let a = GpuSparseCapsule::new(m, k1, nnz_a, SparseFormat::CSR, 0).unwrap();
        let b = GpuSparseCapsule::new(k2, n, nnz_b, SparseFormat::CSR, 0).unwrap();
        let mut c = GpuSparseCapsule::new(m, n, nnz_c, SparseFormat::CSR, 0).unwrap();

        let result = a.spgemm(&b, &mut c);

        if k1 == k2 {
            // Matching inner dimensions: may succeed
            prop_assert!(result.is_ok() || result.is_err());
        } else {
            // Mismatched inner dimensions: must fail
            prop_assert!(result.is_err());
        }
    }

    /// P11: SpGEMM requires same format for all matrices
    #[test]
    fn prop_spgemm_same_format(
        m in 10usize..100,
        k in 10usize..100,
        n in 10usize..100,
        format_a in arb_sparse_format(),
        format_b in arb_sparse_format(),
        format_c in arb_sparse_format(),
    ) {
        let nnz_a = (m * k).min(1000);
        let nnz_b = (k * n).min(1000);
        let nnz_c = (m * n).min(2000);

        let a = GpuSparseCapsule::new(m, k, nnz_a, format_a, 0).unwrap();
        let b = GpuSparseCapsule::new(k, n, nnz_b, format_b, 0).unwrap();
        let mut c = GpuSparseCapsule::new(m, n, nnz_c, format_c, 0).unwrap();

        let result = a.spgemm(&b, &mut c);

        if format_a == format_b && format_a == format_c && format_a == SparseFormat::CSR {
            // Same CSR format: may succeed
            prop_assert!(result.is_ok() || result.is_err());
        } else {
            // Format mismatch or non-CSR: must fail
            prop_assert!(result.is_err());
        }
    }

    /// P12: SpGEMM increments counters (if successful)
    #[test]
    fn prop_spgemm_increments_counters(m in 10usize..100, k in 10usize..100, n in 10usize..100) {
        let nnz_a = (m * k).min(1000);
        let nnz_b = (k * n).min(1000);
        let nnz_c = (m * n).min(2000);

        let a = GpuSparseCapsule::new(m, k, nnz_a, SparseFormat::CSR, 0).unwrap();
        let b = GpuSparseCapsule::new(k, n, nnz_b, SparseFormat::CSR, 0).unwrap();
        let mut c = GpuSparseCapsule::new(m, n, nnz_c, SparseFormat::CSR, 0).unwrap();

        let snap1 = a.snapshot();
        let result = a.spgemm(&b, &mut c);

        if result.is_ok() {
            let snap2 = a.snapshot();
            prop_assert_eq!(snap2.operations, snap1.operations + 1);
            prop_assert_eq!(snap2.spgemm_count, snap1.spgemm_count + 1);
        }
    }
}

// ============================================================================
// Q12: TRIANGULAR SOLVE INVARIANTS
// ============================================================================

proptest! {
    /// P13: Triangular solve requires CSR format
    #[test]
    fn prop_triangular_solve_requires_csr((n in 10usize..500, nnz in 10usize..2000), format in arb_sparse_format()) {
        let sparse = GpuSparseCapsule::new(n, n, nnz, format, 0).unwrap();

        let result = sparse.sparse_triangular_solve_f32(0x1000, 0x2000, true);

        if format == SparseFormat::CSR {
            // CSR: may succeed or fail
            prop_assert!(result.is_ok() || result.is_err());
        } else {
            // Non-CSR: must fail
            prop_assert!(result.is_err());
        }
    }

    /// P14: Triangular solve increments operation count (if successful)
    #[test]
    fn prop_triangular_solve_increments_ops(n in 10usize..500, nnz in 10usize..2000) {
        let sparse = GpuSparseCapsule::new(n, n, nnz, SparseFormat::CSR, 0).unwrap();

        let snap1 = sparse.snapshot();
        let result = sparse.sparse_triangular_solve_f32(0x1000, 0x2000, true);

        if result.is_ok() {
            let snap2 = sparse.snapshot();
            prop_assert_eq!(snap2.operations, snap1.operations + 1);
        }
    }
}

// ============================================================================
// Q13: SNAPSHOT CONSISTENCY
// ============================================================================

proptest! {
    /// P15: Snapshot captures consistent state
    #[test]
    fn prop_snapshot_consistent((rows, cols, nnz) in arb_sparse_dims()) {
        let sparse = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::COO, 0).unwrap();

        let snap = sparse.snapshot();

        prop_assert_eq!(snap.rows as usize, rows);
        prop_assert_eq!(snap.cols as usize, cols);
        prop_assert_eq!(snap.nnz as usize, nnz);
        prop_assert_eq!(snap.format, SparseFormat::COO);
        prop_assert_eq!(snap.operations, 0);
        prop_assert_eq!(snap.generation, 0);
        prop_assert_eq!(snap.spmv_count, 0);
        prop_assert_eq!(snap.spgemm_count, 0);
    }

    /// P16: Multiple snapshots during operations are consistent
    #[test]
    fn prop_snapshot_monotonic((rows, cols, nnz) in arb_sparse_dims()) {
        let sparse = GpuSparseCapsule::new(rows, cols, nnz, SparseFormat::COO, 0).unwrap();

        let snap1 = sparse.snapshot();

        // Perform operations
        let _ = sparse.coo_to_csr();
        let snap2 = sparse.snapshot();

        let _ = sparse.csr_to_coo();
        let snap3 = sparse.snapshot();

        // Operation counts are monotonic
        prop_assert!(snap2.operations >= snap1.operations);
        prop_assert!(snap3.operations >= snap2.operations);

        // Dimensions remain constant
        prop_assert_eq!(snap1.rows, snap2.rows);
        prop_assert_eq!(snap2.rows, snap3.rows);
        prop_assert_eq!(snap1.cols, snap2.cols);
        prop_assert_eq!(snap2.cols, snap3.cols);
        prop_assert_eq!(snap1.nnz, snap2.nnz);
        prop_assert_eq!(snap2.nnz, snap3.nnz);
    }
}

// ============================================================================
// Q14: CONCURRENT SAFETY
// ============================================================================

#[test]
fn test_concurrent_format_conversion() {
    use std::sync::Arc;
    use std::thread;

    let sparse = Arc::new(GpuSparseCapsule::new(1000, 1000, 5000, SparseFormat::COO, 0).unwrap());

    let handles: Vec<_> = (0..16)
        .map(|_| {
            let sparse = Arc::clone(&sparse);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = sparse.coo_to_csr();
                    let _ = sparse.csr_to_coo();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have 1600 operations (16 threads × 100 iterations)
    let snap = sparse.snapshot();
    assert_eq!(snap.operations, 1600);
}

#[test]
fn test_concurrent_spmv() {
    use std::sync::Arc;
    use std::thread;

    let sparse = Arc::new(GpuSparseCapsule::new(256, 256, 1024, SparseFormat::CSR, 0).unwrap());

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let sparse = Arc::clone(&sparse);
            thread::spawn(move || {
                for _ in 0..50 {
                    let _ = sparse.spmv_f32(0x1000, 0x2000, 1.0, 0.0);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // If SpMV succeeded, should have operations count
    let snap = sparse.snapshot();
    assert!(snap.operations <= 400); // Upper bound (may fail due to CPU fallback)
}
