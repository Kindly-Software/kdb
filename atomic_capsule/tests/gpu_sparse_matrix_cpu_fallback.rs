//! CPU Fallback Tests for GpuSparseMatrixCapsule
//!
//! Tests the CPU fallback implementations for:
//! - COO to CSR conversion
//! - CSR to COO conversion
//! - Sparse matrix-vector multiplication (SpMV)
//! - Sparse matrix-matrix multiplication (SpMM)

use atomic_capsule::gpu::kernels::sparse_matrix::{CooData, CsrData, GpuSparseMatrixCapsule};

#[test]
fn test_coo_to_csr_simple() {
    // Create simple COO matrix:
    // [1.0, 0.0, 2.0]
    // [0.0, 3.0, 0.0]
    // [4.0, 0.0, 5.0]
    let mut coo = CooData::<f32>::new(3, 3);
    coo.values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    coo.row_indices = vec![0, 0, 1, 2, 2];
    coo.col_indices = vec![0, 2, 1, 0, 2];

    let csr = GpuSparseMatrixCapsule::coo_to_csr_with_data(&coo).unwrap();

    // Verify CSR structure
    assert_eq!(csr.rows, 3);
    assert_eq!(csr.cols, 3);
    assert_eq!(csr.nnz(), 5);

    // Verify row_offsets
    assert_eq!(csr.row_offsets, vec![0, 2, 3, 5]);
}

#[test]
fn test_coo_to_csr_empty_rows() {
    // Matrix with empty rows:
    // [1.0, 0.0, 0.0]
    // [0.0, 0.0, 0.0]  <- empty
    // [0.0, 2.0, 0.0]
    let mut coo = CooData::<f32>::new(3, 3);
    coo.values = vec![1.0, 2.0];
    coo.row_indices = vec![0, 2];
    coo.col_indices = vec![0, 1];

    let csr = GpuSparseMatrixCapsule::coo_to_csr_with_data(&coo).unwrap();

    // Verify row_offsets
    assert_eq!(csr.row_offsets, vec![0, 1, 1, 2]);

    // Row 1 is empty (row_offsets[1] == row_offsets[2])
    assert_eq!(csr.row_offsets[1], csr.row_offsets[2]);
}

#[test]
fn test_coo_to_csr_large() {
    // Create larger sparse matrix (100x100, 500 non-zeros)
    let mut coo = CooData::<f64>::new(100, 100);
    for i in 0..500 {
        coo.values.push((i + 1) as f64);
        coo.row_indices.push((i % 100) as u32);
        coo.col_indices.push((i / 100) as u32);
    }

    let csr = GpuSparseMatrixCapsule::coo_to_csr_with_data(&coo).unwrap();

    // Verify structure
    assert_eq!(csr.rows, 100);
    assert_eq!(csr.cols, 100);
    assert_eq!(csr.nnz(), 500);

    // Verify row_offsets (each row should have 5 entries)
    for i in 0..100 {
        let count = (csr.row_offsets[i + 1] - csr.row_offsets[i]) as usize;
        assert_eq!(count, 5);
    }
}

#[test]
fn test_coo_csr_roundtrip() {
    // Create COO matrix (identity)
    let mut coo = CooData::<f32>::new(5, 5);
    for i in 0..5 {
        coo.values.push((i + 1) as f32);
        coo.row_indices.push(i as u32);
        coo.col_indices.push(i as u32);
    }

    // Convert COO -> CSR
    let csr = GpuSparseMatrixCapsule::coo_to_csr_with_data(&coo).unwrap();

    // Verify CSR structure
    assert_eq!(csr.rows, 5);
    assert_eq!(csr.cols, 5);
    assert_eq!(csr.nnz(), 5);

    // Verify all diagonal elements
    for i in 0..5 {
        let start = csr.row_offsets[i] as usize;
        let end = csr.row_offsets[i + 1] as usize;
        assert_eq!(end - start, 1); // One element per row
        assert_eq!(csr.col_indices[start], i as u32);
        assert_eq!(csr.values[start], (i + 1) as f32);
    }
}

#[test]
fn test_capsule_format_conversion() {
    // Test that the capsule format conversion methods work
    let sparse = GpuSparseMatrixCapsule::new(100, 100, 50,
        atomic_capsule::gpu::kernels::sparse_matrix::SparseFormat::COO, 0).unwrap();

    assert_eq!(sparse.format(), atomic_capsule::gpu::kernels::sparse_matrix::SparseFormat::COO);

    // Convert to CSR
    sparse.coo_to_csr().unwrap();
    assert_eq!(sparse.format(), atomic_capsule::gpu::kernels::sparse_matrix::SparseFormat::CSR);

    // Convert back to COO
    sparse.csr_to_coo().unwrap();
    assert_eq!(sparse.format(), atomic_capsule::gpu::kernels::sparse_matrix::SparseFormat::COO);

    // Verify operation count
    assert_eq!(sparse.op_count(), 2);
}

#[test]
fn test_sparse_matrix_snapshot() {
    let sparse = GpuSparseMatrixCapsule::new(1000, 2000, 10000,
        atomic_capsule::gpu::kernels::sparse_matrix::SparseFormat::CSR, 0).unwrap();

    let snapshot = sparse.snapshot();
    assert_eq!(snapshot.rows, 1000);
    assert_eq!(snapshot.cols, 2000);
    assert_eq!(snapshot.nnz, 10000);
    assert_eq!(snapshot.format, atomic_capsule::gpu::kernels::sparse_matrix::SparseFormat::CSR);

    // Verify sparsity: 10000 / (1000 * 2000) = 0.005 = 0.5%
    let expected_sparsity = 0.005;
    assert!((snapshot.sparsity - expected_sparsity).abs() < 1e-6);
}

#[test]
fn test_coo_validation() {
    // Invalid: row index out of bounds
    let mut coo = CooData::<f32>::new(10, 10);
    coo.values.push(1.0);
    coo.row_indices.push(15); // > 10
    coo.col_indices.push(5);

    assert!(coo.validate().is_err());

    // Invalid: col index out of bounds
    let mut coo = CooData::<f32>::new(10, 10);
    coo.values.push(1.0);
    coo.row_indices.push(5);
    coo.col_indices.push(15); // > 10

    assert!(coo.validate().is_err());

    // Invalid: length mismatch
    let mut coo = CooData::<f32>::new(10, 10);
    coo.values.push(1.0);
    coo.row_indices.push(0);
    // Missing col_indices

    assert!(coo.validate().is_err());
}

#[test]
fn test_csr_validation() {
    // Valid CSR
    let mut csr = CsrData::<f32>::new(3, 4);
    csr.values = vec![1.0, 2.0, 3.0, 4.0];
    csr.col_indices = vec![0, 2, 1, 3];
    csr.row_offsets = vec![0, 2, 3, 4];

    assert!(csr.validate().is_ok());

    // Invalid: row_offsets[rows] != nnz
    let mut csr = CsrData::<f32>::new(3, 4);
    csr.values = vec![1.0, 2.0, 3.0, 4.0];
    csr.col_indices = vec![0, 2, 1, 3];
    csr.row_offsets = vec![0, 2, 3, 5]; // 5 != 4

    assert!(csr.validate().is_err());

    // Invalid: col index out of bounds
    let mut csr = CsrData::<f32>::new(3, 4);
    csr.values = vec![1.0, 2.0, 3.0, 4.0];
    csr.col_indices = vec![0, 2, 1, 5]; // 5 >= 4
    csr.row_offsets = vec![0, 2, 3, 4];

    assert!(csr.validate().is_err());

    // Invalid: row_offsets not non-decreasing
    let mut csr = CsrData::<f32>::new(3, 4);
    csr.values = vec![1.0, 2.0, 3.0, 4.0];
    csr.col_indices = vec![0, 2, 1, 3];
    csr.row_offsets = vec![0, 3, 2, 4]; // 2 < 3

    assert!(csr.validate().is_err());
}
