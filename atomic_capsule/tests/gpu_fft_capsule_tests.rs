// GPU FFT Capsule Integration Tests
// T28 Testing: Unit + Integration tests for GpuFftCapsule

#![cfg(feature = "std")]

use atomic_capsule::gpu::kernels::{GpuFftCapsule, GpuTensorCapsule};
use atomic_capsule::gpu::kernels::fft::{FftDirection, FftType, GpuFloat};

#[test]
fn test_fft_capsule_layout() {
    // Verify 256-byte alignment and size (Chaos compliance)
    assert_eq!(core::mem::size_of::<GpuFftCapsule>(), 256);
    assert_eq!(core::mem::align_of::<GpuFftCapsule>(), 256);
}

#[test]
fn test_fft_capsule_new() {
    let fft = GpuFftCapsule::new(0).unwrap();
    assert_eq!(fft.fft_count(), 0);
    assert_eq!(fft.total_elements(), 0);

    let snapshot = fft.snapshot();
    assert_eq!(snapshot.fft_count, 0);
    assert_eq!(snapshot.generation, 0);
    assert_eq!(snapshot.total_transforms, 0);
    assert_eq!(snapshot.total_elements, 0);
}

#[test]
fn test_fft_capsule_snapshot() {
    let fft = GpuFftCapsule::new(0).unwrap();

    // Initial snapshot
    let s1 = fft.snapshot();
    assert_eq!(s1.fft_count, 0);
    assert_eq!(s1.total_transforms, 0);
    assert_eq!(s1.total_elements, 0);

    // Snapshot is Copy
    let s2 = s1;
    assert_eq!(s2.fft_count, s1.fft_count);
    assert_eq!(s2.total_transforms, s1.total_transforms);
}

#[test]
fn test_fft_1d_size_validation() {
    let fft = GpuFftCapsule::new(0).unwrap();

    // Create tensors with matching sizes (valid)
    let input = GpuTensorCapsule::<f32, 1>::new([1024], 0).unwrap();
    let mut output = GpuTensorCapsule::<f32, 1>::new([1024], 0).unwrap();

    // This will fail on CPU fallback, but size validation passes
    let _result = fft.fft_1d(&input, &mut output, FftDirection::Forward);
    // We don't assert success because CPU fallback is not implemented
}

#[test]
fn test_fft_1d_size_mismatch() {
    use atomic_capsule::gpu::error::GpuError;

    let fft = GpuFftCapsule::new(0).unwrap();

    // Create tensors with mismatched sizes
    let input = GpuTensorCapsule::<f32, 1>::new([1024], 0).unwrap();
    let mut output = GpuTensorCapsule::<f32, 1>::new([512], 0).unwrap();

    // Attempt FFT (should fail due to size mismatch)
    let result = fft.fft_1d(&input, &mut output, FftDirection::Forward);
    assert!(result.is_err());

    match result {
        Err(GpuError::UnsupportedOperation { operation, reason }) => {
            assert_eq!(operation, "fft_1d");
            assert!(reason.contains("size mismatch"));
        }
        _ => panic!("Expected UnsupportedOperation error"),
    }
}

#[test]
fn test_fft_2d_size_mismatch() {
    use atomic_capsule::gpu::error::GpuError;

    let fft = GpuFftCapsule::new(0).unwrap();

    // Create tensors with mismatched sizes
    let input = GpuTensorCapsule::<f32, 2>::new([32, 32], 0).unwrap();
    let mut output = GpuTensorCapsule::<f32, 2>::new([16, 16], 0).unwrap();

    // Attempt FFT (should fail due to size mismatch)
    let result = fft.fft_2d(&input, &mut output, FftDirection::Forward);
    assert!(result.is_err());

    match result {
        Err(GpuError::UnsupportedOperation { operation, reason }) => {
            assert_eq!(operation, "fft_2d");
            assert!(reason.contains("size mismatch"));
        }
        _ => panic!("Expected UnsupportedOperation error"),
    }
}

#[test]
fn test_batched_fft_size_mismatch() {
    use atomic_capsule::gpu::error::GpuError;

    let fft = GpuFftCapsule::new(0).unwrap();

    // Create tensors with mismatched sizes
    let input = GpuTensorCapsule::<f32, 2>::new([64, 128], 0).unwrap();
    let mut output = GpuTensorCapsule::<f32, 2>::new([32, 128], 0).unwrap();

    // Attempt batched FFT (should fail due to size mismatch)
    let result = fft.batched_fft_1d(&input, &mut output, FftDirection::Forward);
    assert!(result.is_err());

    match result {
        Err(GpuError::UnsupportedOperation { operation, reason }) => {
            assert_eq!(operation, "batched_fft_1d");
            assert!(reason.contains("size mismatch"));
        }
        _ => panic!("Expected UnsupportedOperation error"),
    }
}

#[test]
fn test_power_of_two_sizes() {
    let fft = GpuFftCapsule::new(0).unwrap();

    // Test common power-of-two sizes
    let sizes = [64, 128, 256, 512, 1024, 2048, 4096];

    for &size in &sizes {
        let input = GpuTensorCapsule::<f32, 1>::new([size], 0).unwrap();
        let mut output = GpuTensorCapsule::<f32, 1>::new([size], 0).unwrap();

        // Size validation should pass
        let _result = fft.fft_1d(&input, &mut output, FftDirection::Forward);
        // We don't check success because CPU fallback is not implemented
        // In production with CUDA, these would succeed
    }
}

#[test]
fn test_fft_direction_enum() {
    // Test enum values
    assert_ne!(FftDirection::Forward, FftDirection::Inverse);

    // Test Debug impl
    let fwd = format!("{:?}", FftDirection::Forward);
    let inv = format!("{:?}", FftDirection::Inverse);
    assert_eq!(fwd, "Forward");
    assert_eq!(inv, "Inverse");

    // Test Clone and Copy
    let dir1 = FftDirection::Forward;
    let dir2 = dir1;
    assert_eq!(dir1, dir2);
}

#[test]
fn test_fft_type_enum() {
    // Test enum values
    assert_ne!(FftType::R2C, FftType::C2R);
    assert_ne!(FftType::R2C, FftType::C2C);
    assert_ne!(FftType::C2R, FftType::C2C);

    // Test Debug impl
    let r2c = format!("{:?}", FftType::R2C);
    let c2r = format!("{:?}", FftType::C2R);
    let c2c = format!("{:?}", FftType::C2C);
    assert_eq!(r2c, "R2C");
    assert_eq!(c2r, "C2R");
    assert_eq!(c2c, "C2C");

    // Test Clone and Copy
    let t1 = FftType::R2C;
    let t2 = t1;
    assert_eq!(t1, t2);
}

#[test]
fn test_gpu_float_trait_f32() {
    // Test f32 constants
    assert_eq!(f32::ZERO, 0.0);
    assert_eq!(f32::ONE, 1.0);

    // Verify trait constraints (compile-time)
    fn assert_gpu_float<T: GpuFloat>() {}
    assert_gpu_float::<f32>();
}

#[test]
fn test_gpu_float_trait_f64() {
    // Test f64 constants
    assert_eq!(f64::ZERO, 0.0);
    assert_eq!(f64::ONE, 1.0);

    // Verify trait constraints (compile-time)
    fn assert_gpu_float<T: GpuFloat>() {}
    assert_gpu_float::<f64>();
}

#[cfg(feature = "gpu-cuda")]
#[test]
fn test_cufft_handle() {
    use atomic_capsule::gpu::kernels::fft::CufftHandle;

    // Test null handle
    let handle = CufftHandle::null();
    assert!(!handle.is_valid());

    // Test non-null handle
    let handle = CufftHandle(12345);
    assert!(handle.is_valid());
}

#[test]
fn test_fft_capsule_send_sync() {
    // Verify Send + Sync traits (compile-time)
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<GpuFftCapsule>();
    assert_sync::<GpuFftCapsule>();
}

#[test]
fn test_snapshot_debug() {
    let fft = GpuFftCapsule::new(0).unwrap();
    let snapshot = fft.snapshot();

    // Test Debug impl
    let debug_str = format!("{:?}", snapshot);
    assert!(debug_str.contains("GpuFftSnapshot"));
}

#[test]
fn test_multiple_devices() {
    // Test creating capsules for multiple devices
    for device_id in 0..4 {
        let fft = GpuFftCapsule::new(device_id);
        assert!(fft.is_ok());
    }
}

#[test]
fn test_snapshot_clone_copy() {
    let fft = GpuFftCapsule::new(0).unwrap();
    let s1 = fft.snapshot();

    // Test Clone
    let s2 = s1.clone();
    assert_eq!(s1.fft_count, s2.fft_count);
    assert_eq!(s1.generation, s2.generation);
    assert_eq!(s1.total_transforms, s2.total_transforms);
    assert_eq!(s1.total_elements, s2.total_elements);

    // Test Copy
    let s3 = s1;
    assert_eq!(s1.fft_count, s3.fft_count);
}
