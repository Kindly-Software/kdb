// GPU Tensor Integration Tests
// Tests the enhanced GpuTensorCapsule with device memory management

use atomic_capsule::gpu::kernels::{GpuTensorCapsule, GpuTensorSnapshot, TensorFlags};
use atomic_capsule::gpu::error::GpuError;

#[test]
fn test_tensor_construction() {
    let tensor = GpuTensorCapsule::<f32, 2>::new([128, 256], 0).unwrap();
    assert_eq!(tensor.num_elements(), 128 * 256);
    assert_eq!(tensor.byte_size(), 128 * 256 * 4);
    assert_eq!(tensor.shape(), &[128, 256]);
}

#[test]
fn test_tensor_snapshot() {
    let tensor = GpuTensorCapsule::<f32, 1>::new([1024], 0).unwrap();

    let snap = tensor.snapshot();
    assert_eq!(snap.element_count, 1024);
    assert_eq!(snap.byte_size, 1024 * 4);
    assert_eq!(snap.transfer_count, 0);
    assert_eq!(snap.generation, 0);
    assert!(snap.is_allocated);
}

#[test]
fn test_tensor_host_device_copy() {
    let tensor = GpuTensorCapsule::<f32, 1>::new([100], 0).unwrap();

    // Upload data
    let data: Vec<f32> = (0..100).map(|i| i as f32).collect();
    tensor.copy_from_host(&data).unwrap();

    // Check snapshot updated
    let snap = tensor.snapshot();
    assert_eq!(snap.transfer_count, 1);
    assert_eq!(snap.generation, 1);

    // Download data
    let mut buffer = vec![0.0f32; 100];
    tensor.to_host(&mut buffer).unwrap();

    // Check snapshot updated again
    let snap2 = tensor.snapshot();
    assert_eq!(snap2.transfer_count, 2);
    assert_eq!(snap2.generation, 2);
}

#[test]
fn test_tensor_from_host() {
    let data: Vec<f32> = vec![1.0; 512];
    let tensor = GpuTensorCapsule::<f32, 1>::from_host(&data, [512], 0).unwrap();

    assert_eq!(tensor.num_elements(), 512);

    // Should have one transfer from construction
    let snap = tensor.snapshot();
    assert_eq!(snap.transfer_count, 1);
}

#[test]
fn test_tensor_zeros() {
    let tensor = GpuTensorCapsule::<f32, 2>::zeros([64, 64], 0).unwrap();
    assert_eq!(tensor.num_elements(), 64 * 64);
    assert_eq!(tensor.byte_size(), 64 * 64 * 4);
}

#[test]
fn test_tensor_device_to_device_copy() {
    let src = GpuTensorCapsule::<f32, 1>::new([256], 0).unwrap();
    let dst = GpuTensorCapsule::<f32, 1>::new([256], 0).unwrap();

    src.copy_to_device(&dst).unwrap();

    // Both should have transfer counts incremented
    let snap_src = src.snapshot();
    let snap_dst = dst.snapshot();
    assert_eq!(snap_src.transfer_count, 1);
    assert_eq!(snap_dst.transfer_count, 1);
}

#[test]
fn test_tensor_fill() {
    let tensor = GpuTensorCapsule::<f32, 1>::new([128], 0).unwrap();

    tensor.fill(42.0).unwrap();

    // Fill increments generation
    let snap = tensor.snapshot();
    assert_eq!(snap.generation, 1);
    assert_eq!(snap.transfer_count, 0); // Fill doesn't count as transfer
}

#[test]
fn test_tensor_strides() {
    // 2D matrix: shape=[10, 20], element size=4 bytes (f32)
    let tensor = GpuTensorCapsule::<f32, 2>::new([10, 20], 0).unwrap();

    // Row-major: strides=[80, 4]
    // stride[0] = 20 elements × 4 bytes = 80 bytes (jump to next row)
    // stride[1] = 1 element × 4 bytes = 4 bytes (jump to next column)
    assert_eq!(tensor.strides(), &[80, 4]);
}

#[test]
fn test_tensor_invalid_rank() {
    // Rank 0 should fail
    let result = GpuTensorCapsule::<f32, 0>::new([], 0);
    assert!(result.is_err());
}

#[test]
fn test_tensor_invalid_shape() {
    // Zero dimension should fail
    let result = GpuTensorCapsule::<f32, 2>::new([0, 256], 0);
    assert!(result.is_err());

    let result = GpuTensorCapsule::<f32, 2>::new([128, 0], 0);
    assert!(result.is_err());
}

#[test]
fn test_tensor_layout_alignment() {
    // Verify 256B alignment for all ranks
    assert_eq!(core::mem::align_of::<GpuTensorCapsule<f32, 1>>(), 256);
    assert_eq!(core::mem::align_of::<GpuTensorCapsule<f32, 2>>(), 256);
    assert_eq!(core::mem::align_of::<GpuTensorCapsule<f32, 4>>(), 256);

    // Verify 256B size for ranks ≤4
    assert_eq!(core::mem::size_of::<GpuTensorCapsule<f32, 1>>(), 256);
    assert_eq!(core::mem::size_of::<GpuTensorCapsule<f32, 2>>(), 256);
    assert_eq!(core::mem::size_of::<GpuTensorCapsule<f32, 4>>(), 256);
}

#[test]
fn test_tensor_device_ptr() {
    let tensor = GpuTensorCapsule::<f32, 1>::new([100], 0).unwrap();

    // CPU fallback has zero device_ptr
    assert_eq!(tensor.device_ptr(), 0);
}

#[test]
fn test_tensor_device_id() {
    let tensor = GpuTensorCapsule::<f32, 1>::new([100], 5).unwrap();
    assert_eq!(tensor.device_id(), 5);
}
