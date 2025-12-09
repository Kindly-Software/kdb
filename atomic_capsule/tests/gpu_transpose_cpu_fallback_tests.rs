// GPU Transpose CPU Fallback Tests - Verification of cache-efficient CPU transpose
//
// Tests verify:
// 1. Out-of-place 2D transpose correctness (square and non-square)
// 2. In-place 2D transpose correctness (square matrices)
// 3. Batched transpose correctness
// 4. General N-dimensional permutation correctness
// 5. Cache-efficient blocking (tile size usage)
// 6. Roundtrip invariance (transpose twice = identity)
//
// UCE34: Q15-Q21 Integration Testing
// T28: 5-tier testing (Unit → Property → Integration → Production)
// B32: Fair baseline (naive nested loops)

#![cfg(any(feature = "gpu-cuda", feature = "gpu-rocm", feature = "gpu-intel"))]

use atomic_capsule::gpu::kernels::{GpuTransposeCapsule, GpuTensorCapsule};
use atomic_capsule::gpu::error::GpuError;

// Helper: Initialize test tensor with known pattern
fn init_tensor_2d(rows: usize, cols: usize) -> Vec<f32> {
    (0..rows * cols)
        .map(|i| i as f32)
        .collect()
}

// Helper: Verify transpose correctness
fn verify_transpose_2d(input: &[f32], output: &[f32], rows: usize, cols: usize) -> bool {
    for i in 0..rows {
        for j in 0..cols {
            let input_val = input[i * cols + j];
            let output_val = output[j * rows + i];
            if (input_val - output_val).abs() > 1e-6 {
                eprintln!("Mismatch at ({}, {}): input {} != output {}", i, j, input_val, output_val);
                return false;
            }
        }
    }
    true
}

#[test]
fn test_cpu_transpose_2d_square() -> Result<(), GpuError> {
    let transpose = GpuTransposeCapsule::new(0, 32)?;

    // Create 4×4 square matrix
    let rows = 4;
    let cols = 4;
    let input_data = init_tensor_2d(rows, cols);

    let input = GpuTensorCapsule::<f32, 2>::from_host(&input_data, [rows, cols], 0)?;
    let mut output = GpuTensorCapsule::<f32, 2>::new([cols, rows], 0)?;

    // Transpose
    transpose.transpose_2d(&input, &mut output)?;

    // Verify
    let mut output_data = vec![0.0; rows * cols];
    output.to_host(&mut output_data)?;

    assert!(verify_transpose_2d(&input_data, &output_data, rows, cols));

    // Verify stats
    let snapshot = transpose.snapshot();
    assert_eq!(snapshot.transpose_count, 1);
    assert_eq!(snapshot.total_elements, rows * cols);

    Ok(())
}

#[test]
fn test_cpu_transpose_2d_non_square() -> Result<(), GpuError> {
    let transpose = GpuTransposeCapsule::new(0, 32)?;

    // Create 8×16 non-square matrix
    let rows = 8;
    let cols = 16;
    let input_data = init_tensor_2d(rows, cols);

    let input = GpuTensorCapsule::<f32, 2>::from_host(&input_data, [rows, cols], 0)?;
    let mut output = GpuTensorCapsule::<f32, 2>::new([cols, rows], 0)?;

    // Transpose
    transpose.transpose_2d(&input, &mut output)?;

    // Verify
    let mut output_data = vec![0.0; rows * cols];
    output.to_host(&mut output_data)?;

    assert!(verify_transpose_2d(&input_data, &output_data, rows, cols));

    Ok(())
}

#[test]
fn test_cpu_transpose_2d_large() -> Result<(), GpuError> {
    let transpose = GpuTransposeCapsule::new(0, 32)?;

    // Create 64×128 matrix (tests cache blocking)
    let rows = 64;
    let cols = 128;
    let input_data = init_tensor_2d(rows, cols);

    let input = GpuTensorCapsule::<f32, 2>::from_host(&input_data, [rows, cols], 0)?;
    let mut output = GpuTensorCapsule::<f32, 2>::new([cols, rows], 0)?;

    // Transpose
    transpose.transpose_2d(&input, &mut output)?;

    // Verify
    let mut output_data = vec![0.0; rows * cols];
    output.to_host(&mut output_data)?;

    assert!(verify_transpose_2d(&input_data, &output_data, rows, cols));

    Ok(())
}

#[test]
fn test_cpu_transpose_2d_inplace() -> Result<(), GpuError> {
    let transpose = GpuTransposeCapsule::new(0, 32)?;

    // Create 8×8 square matrix
    let n = 8;
    let input_data = init_tensor_2d(n, n);

    let mut data = GpuTensorCapsule::<f32, 2>::from_host(&input_data, [n, n], 0)?;

    // In-place transpose
    transpose.transpose_2d_inplace(&mut data)?;

    // Verify
    let mut output_data = vec![0.0; n * n];
    data.to_host(&mut output_data)?;

    assert!(verify_transpose_2d(&input_data, &output_data, n, n));

    Ok(())
}

#[test]
fn test_cpu_transpose_2d_roundtrip() -> Result<(), GpuError> {
    let transpose = GpuTransposeCapsule::new(0, 32)?;

    // Create 4×8 matrix
    let rows = 4;
    let cols = 8;
    let input_data = init_tensor_2d(rows, cols);

    let input = GpuTensorCapsule::<f32, 2>::from_host(&input_data, [rows, cols], 0)?;
    let mut temp = GpuTensorCapsule::<f32, 2>::new([cols, rows], 0)?;
    let mut output = GpuTensorCapsule::<f32, 2>::new([rows, cols], 0)?;

    // Transpose twice: input → temp → output (should equal input)
    transpose.transpose_2d(&input, &mut temp)?;
    transpose.transpose_2d(&temp, &mut output)?;

    // Verify roundtrip
    let mut output_data = vec![0.0; rows * cols];
    output.to_host(&mut output_data)?;

    for i in 0..rows * cols {
        assert!((input_data[i] - output_data[i]).abs() < 1e-6,
                "Roundtrip failed at index {}: {} != {}", i, input_data[i], output_data[i]);
    }

    // Verify stats
    let snapshot = transpose.snapshot();
    assert_eq!(snapshot.transpose_count, 2);
    assert_eq!(snapshot.total_transposes, 2);

    Ok(())
}

#[test]
fn test_cpu_batched_transpose() -> Result<(), GpuError> {
    let transpose = GpuTransposeCapsule::new(0, 32)?;

    // Create batched tensor: [4, 8, 16]
    let batch = 4;
    let rows = 8;
    let cols = 16;
    let total = batch * rows * cols;

    let input_data: Vec<f32> = (0..total).map(|i| i as f32).collect();

    let input = GpuTensorCapsule::<f32, 3>::from_host(&input_data, [batch, rows, cols], 0)?;
    let mut output = GpuTensorCapsule::<f32, 3>::new([batch, cols, rows], 0)?;

    // Batched transpose
    transpose.batched_transpose(&input, &mut output)?;

    // Verify each batch
    let mut output_data = vec![0.0; total];
    output.to_host(&mut output_data)?;

    for b in 0..batch {
        let batch_offset_in = b * rows * cols;
        let batch_offset_out = b * cols * rows;

        for i in 0..rows {
            for j in 0..cols {
                let input_val = input_data[batch_offset_in + i * cols + j];
                let output_val = output_data[batch_offset_out + j * rows + i];
                assert!((input_val - output_val).abs() < 1e-6,
                        "Batch {} mismatch at ({}, {}): {} != {}", b, i, j, input_val, output_val);
            }
        }
    }

    Ok(())
}

#[test]
fn test_cpu_permute_identity() -> Result<(), GpuError> {
    let transpose = GpuTransposeCapsule::new(0, 32)?;

    // Create 3D tensor: [2, 4, 8]
    let shape = [2, 4, 8];
    let total = shape.iter().product();
    let input_data: Vec<f32> = (0..total).map(|i| i as f32).collect();

    let input = GpuTensorCapsule::<f32, 3>::from_host(&input_data, shape, 0)?;
    let mut output = GpuTensorCapsule::<f32, 3>::new(shape, 0)?;

    // Identity permutation: [0, 1, 2]
    transpose.permute(&input, &mut output, [0, 1, 2])?;

    // Verify output equals input
    let mut output_data = vec![0.0; total];
    output.to_host(&mut output_data)?;

    for i in 0..total {
        assert!((input_data[i] - output_data[i]).abs() < 1e-6,
                "Identity permutation failed at {}: {} != {}", i, input_data[i], output_data[i]);
    }

    Ok(())
}

#[test]
fn test_cpu_permute_reverse() -> Result<(), GpuError> {
    let transpose = GpuTransposeCapsule::new(0, 32)?;

    // Create 3D tensor: [2, 3, 4]
    let shape = [2, 3, 4];
    let total = shape.iter().product();
    let input_data: Vec<f32> = (0..total).map(|i| i as f32).collect();

    let input = GpuTensorCapsule::<f32, 3>::from_host(&input_data, shape, 0)?;
    let mut output = GpuTensorCapsule::<f32, 3>::new([4, 3, 2], 0)?;

    // Reverse permutation: [2, 1, 0]
    transpose.permute(&input, &mut output, [2, 1, 0])?;

    // Verify correctness
    let mut output_data = vec![0.0; total];
    output.to_host(&mut output_data)?;

    // Check a few known values
    // input[0][0][0] (linear 0) → output[0][0][0] (linear 0)
    assert!((input_data[0] - output_data[0]).abs() < 1e-6);

    // input[0][0][1] (linear 1) → output[1][0][0] (linear 6)
    assert!((input_data[1] - output_data[6]).abs() < 1e-6);

    Ok(())
}

#[test]
fn test_cpu_permute_last_two() -> Result<(), GpuError> {
    let transpose = GpuTransposeCapsule::new(0, 32)?;

    // Create 3D tensor: [2, 4, 8]
    let shape = [2, 4, 8];
    let total = shape.iter().product();
    let input_data: Vec<f32> = (0..total).map(|i| i as f32).collect();

    let input = GpuTensorCapsule::<f32, 3>::from_host(&input_data, shape, 0)?;
    let mut output = GpuTensorCapsule::<f32, 3>::new([2, 8, 4], 0)?;

    // Permute last two dimensions: [0, 2, 1]
    transpose.permute(&input, &mut output, [0, 2, 1])?;

    // Verify correctness
    let mut output_data = vec![0.0; total];
    output.to_host(&mut output_data)?;

    // Verify each batch is transposed correctly
    let batch_size = 4 * 8;
    for b in 0..2 {
        let batch_offset = b * batch_size;
        for i in 0..4 {
            for j in 0..8 {
                let input_idx = batch_offset + i * 8 + j;
                let output_idx = batch_offset + j * 4 + i;
                assert!((input_data[input_idx] - output_data[output_idx]).abs() < 1e-6,
                        "Batch {} permute failed at ({}, {})", b, i, j);
            }
        }
    }

    Ok(())
}

#[test]
fn test_tile_size_16() -> Result<(), GpuError> {
    let transpose = GpuTransposeCapsule::new(0, 16)?;

    // Create 32×32 matrix (tests 16×16 tile blocking)
    let n = 32;
    let input_data = init_tensor_2d(n, n);

    let input = GpuTensorCapsule::<f32, 2>::from_host(&input_data, [n, n], 0)?;
    let mut output = GpuTensorCapsule::<f32, 2>::new([n, n], 0)?;

    // Transpose
    transpose.transpose_2d(&input, &mut output)?;

    // Verify
    let mut output_data = vec![0.0; n * n];
    output.to_host(&mut output_data)?;

    assert!(verify_transpose_2d(&input_data, &output_data, n, n));
    assert_eq!(transpose.tile_size(), 16);

    Ok(())
}

#[test]
fn test_tile_size_64() -> Result<(), GpuError> {
    let transpose = GpuTransposeCapsule::new(0, 64)?;

    // Create 128×128 matrix (tests 64×64 tile blocking)
    let n = 128;
    let input_data = init_tensor_2d(n, n);

    let input = GpuTensorCapsule::<f32, 2>::from_host(&input_data, [n, n], 0)?;
    let mut output = GpuTensorCapsule::<f32, 2>::new([n, n], 0)?;

    // Transpose
    transpose.transpose_2d(&input, &mut output)?;

    // Verify
    let mut output_data = vec![0.0; n * n];
    output.to_host(&mut output_data)?;

    assert!(verify_transpose_2d(&input_data, &output_data, n, n));
    assert_eq!(transpose.tile_size(), 64);

    Ok(())
}
