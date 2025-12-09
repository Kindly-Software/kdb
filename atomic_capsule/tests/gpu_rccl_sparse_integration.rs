//! GPU RCCL and Sparse Matrix Integration Tests
//! T28 Tier 3: Integration testing for multi-GPU collectives and sparse matrix operations
//!
//! Framework Compliance:
//! - UCE34 Q15-Q21: Integration testing tier
//! - T28: Integration test tier (Q15-Q21)
//! - B32: Performance baseline validation
//! - ASSUM: All GPU ops have CPU fallback
//! - Chaos: 100% lockfree capsule coordination

#![cfg(feature = "std")]

use atomic_capsule::gpu::kernels::{
    GpuRcclCapsule, GpuRcclSnapshot, RcclOp, CollectiveType, RcclTopology,
    GpuSparseMatrixCapsule, GpuSparseMatrixSnapshot, SparseFormat,
};
use atomic_capsule::gpu::GpuBackend;

/// T28 Q15: RCCL capsule creation and initialization
#[test]
fn test_rccl_capsule_new() {
    let capsule = GpuRcclCapsule::new(GpuBackend::CpuFallback);
    assert!(capsule.is_ok(), "RCCL capsule creation should succeed");

    let capsule = capsule.unwrap();
    let snapshot = capsule.snapshot();

    assert_eq!(snapshot.total_collectives, 0, "Initial collectives should be zero");
    assert_eq!(snapshot.total_bytes, 0, "Initial bytes should be zero");
    assert_eq!(snapshot.rank, 0, "Default rank should be zero");
    assert_eq!(snapshot.world_size, 1, "Default world size should be 1");
}

/// T28 Q16: RCCL unique ID generation
#[test]
fn test_rccl_unique_id() {
    let capsule = GpuRcclCapsule::new(GpuBackend::CpuFallback).unwrap();

    // CPU fallback should generate zero-filled ID (not a real RCCL call)
    let unique_id = capsule.get_unique_id();
    assert!(unique_id.is_ok(), "get_unique_id should succeed");

    // Verify 128-byte alignment
    let id = unique_id.unwrap();
    let ptr = &id as *const _ as usize;
    assert_eq!(ptr % 128, 0, "RcclUniqueId must be 128-byte aligned");
}

/// T28 Q17: RCCL all_reduce operation
#[test]
fn test_rccl_all_reduce() {
    let capsule = GpuRcclCapsule::new(GpuBackend::CpuFallback).unwrap();

    // Single-rank all_reduce (CPU fallback is no-op)
    let data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
    let result = capsule.all_reduce(
        data.as_ptr() as u64,
        data.as_ptr() as u64,
        data.len() as u64,
        RcclOp::Sum,
    );

    assert!(result.is_ok(), "all_reduce should succeed on CPU fallback");

    let snapshot = capsule.snapshot();
    assert_eq!(snapshot.total_collectives, 1, "Should track one collective");
    assert_eq!(snapshot.total_bytes, 16, "Should track 4 floats * 4 bytes");
}

/// T28 Q18: RCCL all_gather operation
#[test]
fn test_rccl_all_gather() {
    let capsule = GpuRcclCapsule::new(GpuBackend::CpuFallback).unwrap();

    let send_data: Vec<f32> = vec![5.0, 6.0];
    let mut recv_data: Vec<f32> = vec![0.0; 2]; // world_size=1

    let result = capsule.all_gather(
        send_data.as_ptr() as u64,
        recv_data.as_mut_ptr() as u64,
        send_data.len() as u64,
    );

    assert!(result.is_ok(), "all_gather should succeed on CPU fallback");
}

/// T28 Q19: RCCL broadcast operation
#[test]
fn test_rccl_broadcast() {
    let capsule = GpuRcclCapsule::new(GpuBackend::CpuFallback).unwrap();

    let mut data: Vec<f32> = vec![10.0, 20.0, 30.0];

    let result = capsule.broadcast(
        data.as_mut_ptr() as u64,
        data.len() as u64,
        0, // root rank
    );

    assert!(result.is_ok(), "broadcast should succeed on CPU fallback");
}

/// T28 Q20: RCCL reduce_scatter operation
#[test]
fn test_rccl_reduce_scatter() {
    let capsule = GpuRcclCapsule::new(GpuBackend::CpuFallback).unwrap();

    let send_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
    let mut recv_data: Vec<f32> = vec![0.0; 4]; // world_size=1, all data goes to rank 0

    let result = capsule.reduce_scatter(
        send_data.as_ptr() as u64,
        recv_data.as_mut_ptr() as u64,
        recv_data.len() as u64,
        RcclOp::Sum,
    );

    assert!(result.is_ok(), "reduce_scatter should succeed on CPU fallback");
}

/// T28 Q21: RCCL topology auto-detection
#[test]
fn test_rccl_topology_detection() {
    // Test topology selection based on world_size
    // Ring: 2-7 ranks
    // Tree: 8-15 ranks
    // DoubleBinaryTree: 16-127 ranks
    // FullyConnected: 8 ranks (MI300X specific)

    let capsule = GpuRcclCapsule::new(GpuBackend::CpuFallback).unwrap();
    let snapshot = capsule.snapshot();

    // World size 1 should use Ring topology (simplest)
    assert_eq!(snapshot.topology, RcclTopology::Ring as u64);
}

/// T28 Q15: Sparse matrix capsule creation
#[test]
fn test_sparse_matrix_new() {
    let rows = 1000;
    let cols = 1000;
    let nnz = 5000; // 0.5% sparsity

    let capsule = GpuSparseMatrixCapsule::new(
        rows,
        cols,
        nnz,
        SparseFormat::CSR,
        GpuBackend::CpuFallback,
    );

    assert!(capsule.is_ok(), "Sparse matrix capsule creation should succeed");

    let capsule = capsule.unwrap();
    let snapshot = capsule.snapshot();

    assert_eq!(snapshot.rows, rows, "Rows should match");
    assert_eq!(snapshot.cols, cols, "Cols should match");
    assert_eq!(snapshot.nnz, nnz, "NNZ should match");
    assert_eq!(snapshot.format, SparseFormat::CSR as u64, "Format should be CSR");
}

/// T28 Q16: Sparse matrix format conversion
#[test]
fn test_sparse_matrix_convert_format() {
    let capsule = GpuSparseMatrixCapsule::new(
        100, 100, 500,
        SparseFormat::COO,
        GpuBackend::CpuFallback,
    ).unwrap();

    // Convert COO -> CSR
    let result = capsule.convert_format(SparseFormat::CSR);
    assert!(result.is_ok(), "Format conversion COO->CSR should succeed");

    let snapshot = capsule.snapshot();
    assert_eq!(snapshot.format, SparseFormat::CSR as u64, "Format should be CSR after conversion");
}

/// T28 Q17: Sparse matrix SpMV (CPU fallback)
#[test]
fn test_sparse_matrix_spmv() {
    let capsule = GpuSparseMatrixCapsule::new(
        10, 10, 30,
        SparseFormat::CSR,
        GpuBackend::CpuFallback,
    ).unwrap();

    let x_vec: Vec<f32> = vec![1.0; 10];
    let mut y_vec: Vec<f32> = vec![0.0; 10];

    // y = alpha * A * x + beta * y
    let result = capsule.spmv(
        1.0, // alpha
        x_vec.as_ptr() as u64,
        0.0, // beta
        y_vec.as_mut_ptr() as u64,
    );

    assert!(result.is_ok(), "SpMV should succeed on CPU fallback");

    let snapshot = capsule.snapshot();
    assert_eq!(snapshot.total_spmvs, 1, "Should track one SpMV operation");
}

/// T28 Q18: Sparse matrix SpMM (CPU fallback)
#[test]
fn test_sparse_matrix_spmm() {
    let capsule = GpuSparseMatrixCapsule::new(
        10, 10, 30,
        SparseFormat::CSR,
        GpuBackend::CpuFallback,
    ).unwrap();

    let b_matrix: Vec<f32> = vec![1.0; 10 * 5]; // Dense 10x5
    let mut c_matrix: Vec<f32> = vec![0.0; 10 * 5]; // Result 10x5

    // C = alpha * A * B + beta * C
    let result = capsule.spmm(
        1.0, // alpha
        b_matrix.as_ptr() as u64,
        0.0, // beta
        c_matrix.as_mut_ptr() as u64,
        5, // num_cols in B
    );

    assert!(result.is_ok(), "SpMM should succeed on CPU fallback");

    let snapshot = capsule.snapshot();
    assert_eq!(snapshot.total_spmms, 1, "Should track one SpMM operation");
}

/// T28 Q19: Sparse matrix structured 2:4 sparsity (BSR format)
#[test]
fn test_sparse_matrix_structured_2_4_sparsity() {
    // For 2:4 sparsity, use block size 4 (2 out of 4 consecutive elements)
    let capsule = GpuSparseMatrixCapsule::new(
        64, 64, 512, // 50% sparsity (2:4 pattern)
        SparseFormat::BSR,
        GpuBackend::CpuFallback,
    ).unwrap();

    // Set block size for 2:4 sparsity
    capsule.set_block_size(4);

    let snapshot = capsule.snapshot();
    assert_eq!(snapshot.block_size, 4, "Block size should be 4 for 2:4 sparsity");
    assert_eq!(snapshot.format, SparseFormat::BSR as u64, "Format should be BSR");
}

/// T28 Q20: Sparse matrix all formats (COO, CSR, CSC, BSR, ELL)
#[test]
fn test_sparse_matrix_all_formats() {
    let formats = vec![
        SparseFormat::COO,
        SparseFormat::CSR,
        SparseFormat::CSC,
        SparseFormat::BSR,
        SparseFormat::ELL,
    ];

    for format in formats {
        let capsule = GpuSparseMatrixCapsule::new(
            20, 20, 60,
            format,
            GpuBackend::CpuFallback,
        );

        assert!(
            capsule.is_ok(),
            "Sparse matrix creation should succeed for format {:?}",
            format
        );

        let snapshot = capsule.unwrap().snapshot();
        assert_eq!(
            snapshot.format,
            format as u64,
            "Format should match {:?}",
            format
        );
    }
}

/// T28 Q21: RCCL and sparse matrix integration (multi-GPU sparse AllReduce)
#[test]
fn test_rccl_sparse_integration() {
    // Create RCCL capsule for multi-GPU coordination
    let rccl = GpuRcclCapsule::new(GpuBackend::CpuFallback).unwrap();

    // Create sparse matrix capsule
    let sparse = GpuSparseMatrixCapsule::new(
        100, 100, 500,
        SparseFormat::CSR,
        GpuBackend::CpuFallback,
    ).unwrap();

    // Simulate multi-GPU sparse gradient AllReduce
    let values: Vec<f32> = vec![0.1; 500]; // Sparse gradient values

    let result = rccl.all_reduce(
        values.as_ptr() as u64,
        values.as_ptr() as u64,
        values.len() as u64,
        RcclOp::Sum,
    );

    assert!(result.is_ok(), "Multi-GPU sparse AllReduce should succeed");

    let rccl_snapshot = rccl.snapshot();
    assert_eq!(rccl_snapshot.total_collectives, 1, "Should track collective");

    let sparse_snapshot = sparse.snapshot();
    assert_eq!(sparse_snapshot.nnz, 500, "Sparse NNZ should be unchanged");
}
