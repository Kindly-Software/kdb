// Integration tests for GpuDecompressionCapsule
// These tests run in the tests/ directory to ensure proper module visibility

use atomic_capsule::gpu::kernels::{
    CompressedKV, DataType, GpuBuffer, GpuDecompressionCapsule,
};

#[test]
fn test_capsule_layout() {
    assert_eq!(core::mem::size_of::<GpuDecompressionCapsule>(), 256);
    assert_eq!(core::mem::align_of::<GpuDecompressionCapsule>(), 256);
}

#[test]
fn test_new_capsule() {
    let decompressor = GpuDecompressionCapsule::new(0).unwrap();
    assert_eq!(decompressor.device_id(), 0);
    assert_eq!(decompressor.codebook_size(), 0);
    assert_eq!(decompressor.codebook_dim(), 0);
}

#[test]
fn test_upload_codebook() {
    let decompressor = GpuDecompressionCapsule::new(0).unwrap();

    // Upload 256 entries × 128 dim = 32768 FP16 elements
    let codebook: Vec<u16> = (0..32768).map(|i| i as u16).collect();
    decompressor.upload_codebook(&codebook).unwrap();

    assert_eq!(decompressor.codebook_size(), 256);
    assert_eq!(decompressor.codebook_dim(), 128);
}

#[test]
fn test_decompress_and_attend() {
    let decompressor = GpuDecompressionCapsule::new(0).unwrap();

    // Upload codebook
    let codebook: Vec<u16> = (0..32768).map(|i| i as u16).collect();
    decompressor.upload_codebook(&codebook).unwrap();

    // Prepare compressed KV
    let compressed_kv = CompressedKV {
        indices: vec![0, 1, 2, 3],
        residuals: None,
        seq_len: 4,
        dim: 128,
    };

    let query = GpuBuffer {
        device_ptr: 0,
        size: 4 * 128 * 2, // 4 tokens × 128 dim × 2 bytes (FP16)
        dtype: DataType::F16,
    };

    let mut output = GpuBuffer {
        device_ptr: 0,
        size: 4 * 2, // 4 tokens × 2 bytes (attention scores)
        dtype: DataType::F16,
    };

    decompressor
        .decompress_and_attend(&compressed_kv, &query, &mut output)
        .unwrap();

    let snapshot = decompressor.snapshot();
    assert_eq!(snapshot.kernel_launches, 1);
    assert_eq!(snapshot.completed_kernels, 1);
    assert_eq!(snapshot.total_tokens_processed, 4);
}

#[test]
fn test_statistics_tracking() {
    let decompressor = GpuDecompressionCapsule::new(0).unwrap();

    // Upload codebook
    let codebook: Vec<u16> = vec![0; 32768];
    decompressor.upload_codebook(&codebook).unwrap();

    // Multiple decompressions
    for i in 0..10 {
        let compressed_kv = CompressedKV {
            indices: vec![0, 1, 2],
            residuals: None,
            seq_len: 3,
            dim: 128,
        };

        let query = GpuBuffer {
            device_ptr: 0,
            size: 3 * 128 * 2,
            dtype: DataType::F16,
        };

        let mut output = GpuBuffer {
            device_ptr: 0,
            size: 3 * 2,
            dtype: DataType::F16,
        };

        decompressor
            .decompress_and_attend(&compressed_kv, &query, &mut output)
            .unwrap();

        let snapshot = decompressor.snapshot();
        assert_eq!(snapshot.kernel_launches, (i + 1) as u64);
        assert_eq!(snapshot.completed_kernels, (i + 1) as u64);
        assert_eq!(snapshot.total_tokens_processed, (i + 1) as u64 * 3);
    }
}

#[test]
fn test_thread_safety() {
    use std::sync::Arc;
    use std::thread;

    let decompressor = Arc::new(GpuDecompressionCapsule::new(0).unwrap());

    // Upload codebook
    let codebook: Vec<u16> = vec![0; 32768];
    decompressor.upload_codebook(&codebook).unwrap();

    // Spawn multiple threads
    let mut handles = vec![];
    for _ in 0..4 {
        let decompressor_clone: Arc<GpuDecompressionCapsule> = Arc::clone(&decompressor);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let compressed_kv = CompressedKV {
                    indices: vec![0, 1],
                    residuals: None,
                    seq_len: 2,
                    dim: 128,
                };

                let query = GpuBuffer {
                    device_ptr: 0,
                    size: 2 * 128 * 2,
                    dtype: DataType::F16,
                };

                let mut output = GpuBuffer {
                    device_ptr: 0,
                    size: 2 * 2,
                    dtype: DataType::F16,
                };

                let _ = decompressor_clone.decompress_and_attend(&compressed_kv, &query, &mut output);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let snapshot = decompressor.snapshot();
    assert_eq!(snapshot.kernel_launches, 400);
    assert_eq!(snapshot.completed_kernels, 400);
}
