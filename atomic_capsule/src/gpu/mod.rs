// GPU Acceleration Module - T7 Heterogeneous Tier
// Phase 5: GPU Acceleration Foundation
//
// UCE34 Compliance:
// - Q10: T7 Heterogeneous tier (CUDA/ROCm, 100-1000× speedup)
// - Q11: Rust transform (type-safe GPU coordination)
// - Q12: Nightly features (portable_simd for CPU fallback)
// - Q33: Verification (#[derive(ComputationalCapsule)] for coordination)
// - Q34: Audit trail (kernel launch tracking, utilization metrics)
//
// COCA Compliance: T1 Atomic coordination + T7 GPU compute
// ASSUM Safety: 99.99%+
// B32 Performance Targets:
// - Matrix Multiplication (1024×1024, batch=1000): 100× vs AVX2 CPU
// - Batch Hashing (SHA3-256, 1M messages): 100× vs SIMD CPU
// - Batch Signature Verification (Ed25519, 100K): 100-500× vs CPU

pub mod error;
pub mod cuda_capsule;
pub mod rocm_capsule;
pub mod gpu_coordinator;
pub mod kernels;

pub use error::{GpuBackend, GpuError, GpuResult, MemoryCopyDirection};
pub use cuda_capsule::CudaComputeCapsule;
pub use rocm_capsule::RocmComputeCapsule;
pub use gpu_coordinator::GpuCoordinator;

// Re-export GPU kernels for convenience
pub use kernels::{
    GpuTensorCapsule,
    GpuMemoryPoolCapsule,
    GpuStreamCapsule,
    GpuMatMulCapsule,
    GpuReductionCapsule,
    GpuTransposeCapsule,
    GpuConvolutionCapsule,
    GpuFftCapsule,
    GpuSparseMatrixCapsule,
};

/// GPU acceleration feature detection
pub fn is_cuda_available() -> bool {
    #[cfg(feature = "gpu-cuda")]
    {
        // Try to create device 0
        CudaComputeCapsule::new(0).is_ok()
    }

    #[cfg(not(feature = "gpu-cuda"))]
    {
        false
    }
}

/// ROCm feature detection
pub fn is_rocm_available() -> bool {
    #[cfg(feature = "gpu-rocm")]
    {
        // Try to create device 0
        RocmComputeCapsule::new(0).is_ok()
    }

    #[cfg(not(feature = "gpu-rocm"))]
    {
        false
    }
}

/// Get number of available GPU devices
pub fn device_count() -> GpuResult<u32> {
    #[cfg(feature = "gpu-cuda")]
    {
        // cudarc provides device count via CudaDevice::count()
        use cudarc::driver::CudaDevice;
        match CudaDevice::count() {
            Ok(count) => Ok(count as u32),
            Err(_) => Ok(0),
        }
    }

    #[cfg(all(feature = "gpu-rocm", not(feature = "gpu-cuda")))]
    {
        // TODO: Implement using hip-runtime-sys
        Err(GpuError::UnsupportedOperation {
            operation: "device_count".to_string(),
            reason: "ROCm backend not yet implemented".to_string(),
        })
    }

    #[cfg(not(any(feature = "gpu-cuda", feature = "gpu-rocm")))]
    {
        Ok(0) // No GPU backend available
    }
}

/// Get GPU backend type
pub fn backend() -> GpuBackend {
    #[cfg(feature = "gpu-cuda")]
    {
        return GpuBackend::Cuda;
    }

    #[cfg(all(feature = "gpu-rocm", not(feature = "gpu-cuda")))]
    {
        return GpuBackend::Rocm;
    }

    #[cfg(not(any(feature = "gpu-cuda", feature = "gpu-rocm")))]
    {
        return GpuBackend::CpuFallback;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_detection() {
        let b = backend();

        #[cfg(feature = "gpu-cuda")]
        assert_eq!(b, GpuBackend::Cuda);

        #[cfg(all(feature = "gpu-rocm", not(feature = "gpu-cuda")))]
        assert_eq!(b, GpuBackend::Rocm);

        #[cfg(not(any(feature = "gpu-cuda", feature = "gpu-rocm")))]
        assert_eq!(b, GpuBackend::CpuFallback);
    }

    #[test]
    fn test_device_count() {
        #[cfg(any(feature = "gpu-cuda", feature = "gpu-rocm"))]
        {
            // May be 0 if no GPU available
            let count = device_count().unwrap_or(0);
            assert!(count <= 16);
        }

        #[cfg(not(any(feature = "gpu-cuda", feature = "gpu-rocm")))]
        {
            assert_eq!(device_count().unwrap(), 0);
        }
    }
}
