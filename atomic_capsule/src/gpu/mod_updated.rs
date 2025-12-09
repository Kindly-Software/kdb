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
// Chaos Compliance: T1 Atomic coordination + T7 GPU compute
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
pub mod vma_capsule;
pub mod descriptor_pool_capsule;
pub mod gem_object_capsule;

pub use error::{GpuBackend, GpuError, GpuResult, MemoryCopyDirection};
pub use cuda_capsule::CudaComputeCapsule;
pub use rocm_capsule::RocmComputeCapsule;
pub use gpu_coordinator::GpuCoordinator;
pub use vma_capsule::{VmaCapsule, VmaError, VmaResult, VmaFlags, VmaSnapshot};
pub use descriptor_pool_capsule::{
    DescriptorPoolCapsule, DescriptorHandle, DescriptorPoolError, DescriptorPoolResult,
};
pub use gem_object_capsule::{GemObjectCapsule, GemHandle, GemObjectState, GemError, GemResult, GemObjectSnapshot};
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
