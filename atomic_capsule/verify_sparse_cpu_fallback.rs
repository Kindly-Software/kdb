// Standalone verification of CPU fallback implementations
// Compile: cargo build --example verify_sparse_cpu_fallback --features "std,gpu-cuda"
// Run: cargo run --example verify_sparse_cpu_fallback --features "std,gpu-cuda"

use atomic_capsule::gpu::kernels::sparse_matrix::{CooData, GpuSparseMatrixCapsule};

fn main() {
    println!("=== GPU Sparse Matrix CPU Fallback Verification ===\n");

    // Test 1: COO to CSR conversion
    println!("Test 1: COO to CSR Conversion");
    let mut coo = CooData::<f32>::new(3, 3);
    coo.values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    coo.row_indices = vec![0, 0, 1, 2, 2];
    coo.col_indices = vec![0, 2, 1, 0, 2];

    match GpuSparseMatrixCapsule::coo_to_csr_with_data(&coo) {
        Ok(csr) => {
            println!("✓ COO to CSR conversion successful");
            println!("  Rows: {}, Cols: {}, NNZ: {}", csr.rows, csr.cols, csr.nnz());
            println!("  Row offsets: {:?}", csr.row_offsets);
            assert_eq!(csr.row_offsets, vec![0, 2, 3, 5], "Row offsets mismatch");
            println!("✓ Row offsets validated\n");
        }
        Err(e) => {
            eprintln!("✗ COO to CSR conversion failed: {:?}", e);
            std::process::exit(1);
        }
    }

    // Test 2: Empty rows
    println!("Test 2: Empty Rows Handling");
    let mut coo2 = CooData::<f32>::new(3, 3);
    coo2.values = vec![1.0, 2.0];
    coo2.row_indices = vec![0, 2];
    coo2.col_indices = vec![0, 1];

    match GpuSparseMatrixCapsule::coo_to_csr_with_data(&coo2) {
        Ok(csr) => {
            println!("✓ Empty row handling successful");
            println!("  Row offsets: {:?}", csr.row_offsets);
            assert_eq!(csr.row_offsets[1], csr.row_offsets[2], "Empty row not detected");
            println!("✓ Empty row validated (row_offsets[1] == row_offsets[2])\n");
        }
        Err(e) => {
            eprintln!("✗ Empty row handling failed: {:?}", e);
            std::process::exit(1);
        }
    }

    // Test 3: Large matrix
    println!("Test 3: Large Matrix (100x100, 500 non-zeros)");
    let mut coo3 = CooData::<f64>::new(100, 100);
    for i in 0..500 {
        coo3.values.push((i + 1) as f64);
        coo3.row_indices.push((i % 100) as u32);
        coo3.col_indices.push((i / 100) as u32);
    }

    match GpuSparseMatrixCapsule::coo_to_csr_with_data(&coo3) {
        Ok(csr) => {
            println!("✓ Large matrix conversion successful");
            println!("  Rows: {}, Cols: {}, NNZ: {}", csr.rows, csr.cols, csr.nnz());

            // Verify each row has 5 entries
            let mut all_correct = true;
            for i in 0..100 {
                let count = (csr.row_offsets[i + 1] - csr.row_offsets[i]) as usize;
                if count != 5 {
                    println!("  ✗ Row {} has {} entries (expected 5)", i, count);
                    all_correct = false;
                }
            }

            if all_correct {
                println!("✓ All 100 rows have exactly 5 entries\n");
            } else {
                eprintln!("✗ Row count validation failed");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("✗ Large matrix conversion failed: {:?}", e);
            std::process::exit(1);
        }
    }

    println!("=== All Tests Passed! ===\n");
    println!("CPU fallback implementations verified:");
    println!("  ✓ COO to CSR conversion");
    println!("  ✓ Empty row handling");
    println!("  ✓ Large matrix support");
    println!("\nImplementation is production-ready!");
}
