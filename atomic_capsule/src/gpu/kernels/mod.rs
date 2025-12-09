// GPU Kernels Module - T7 Heterogeneous Tier
// Phase 5.1: GPU Primitives Implementation
//
// This module provides 10 production GPU primitives for ML/scientific workloads:
// 1. GpuTensorCapsule - N-dimensional tensor storage (foundation)
// 2. GpuMemoryPoolCapsule - Device memory management (lockfree allocation)
// 3. GpuStreamCapsule - Async kernel dispatch (concurrent execution)
// 4. GpuMatMulCapsule - Matrix multiplication (cuBLAS integration)
// 5. GpuReductionCapsule - Parallel reduction (sum/max/min)
// 6. GpuTransposeCapsule - In-place transpose (cache-optimal)
// 7. GpuConvolutionCapsule - 2D/3D convolution (cuDNN integration)
// 8. GpuFftCapsule - Fast Fourier Transform (cuFFT integration)
// 9. GpuSparseMatrixCapsule - Sparse matrix operations (COO/CSR formats)
// 10. GpuDecompressionCapsule - KV cache decompression (fused decompress+attention)

pub mod tensor;
pub mod memory_pool;
pub mod stream;
pub mod matmul;
pub mod reduction;
pub mod transpose;
pub mod convolution;
pub mod fft;
pub mod sparse_matrix;
pub mod kv_decompression;

pub use tensor::GpuTensorCapsule;
pub use memory_pool::GpuMemoryPoolCapsule;
pub use stream::GpuStreamCapsule;
pub use matmul::GpuMatMulCapsule;
pub use reduction::GpuReductionCapsule;
pub use transpose::GpuTransposeCapsule;
pub use convolution::GpuConvolutionCapsule;
pub use fft::GpuFftCapsule;
pub use sparse_matrix::GpuSparseMatrixCapsule;
pub use kv_decompression::{
    GpuDecompressionCapsule, GpuBuffer, DataType, CompressedKV,
    GpuDecompressionSnapshot, GpuDecompressionError,
};
