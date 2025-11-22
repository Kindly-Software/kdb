//! # Tier 7: GPU Capsule - Massively Parallel Computation
//!
//! **UCE33 Q10**: Tier 7 GPU capsules provide massively parallel computation on GPU accelerators.
//!
//! ## Performance Expectations (B32 Guidelines)
//!
//! - **Throughput**: 100-1000× vs CPU for embarrassingly parallel workloads
//! - **Latency**: 50-500μs kernel launch overhead (PCIe transfer)
//! - **Bandwidth**: 500-1500 GB/s (GPU memory bandwidth)
//! - **Sweet Spot**: >100K elements (amortizes transfer overhead)
//!
//! ## Use Cases
//!
//! - Matrix operations (BLAS: GEMM, GEMV)
//! - Signal processing (FFT, convolution, filtering)
//! - Monte Carlo simulation (risk, pricing, optimization)
//! - Neural network inference (brain forward pass)
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_GPU_AVAILABLE`: GPU device is present and initialized
//! - `#VERIFY_GPU_AVAILABLE`: Runtime device detection in implementations
//! - `#ASSUME_MEMORY_TRANSFER`: Host-device transfer cost is amortized over batch
//! - `#VERIFY_MEMORY_TRANSFER`: B32 benchmarks include full transfer time
//!
//! ## B32 Reality Checks
//!
//! - **Theoretical**: 100-1000× speedup for perfectly parallel workloads
//! - **Realistic**: 100-1000× for large datasets (>100K elements)
//! - **Small Data**: <10× for <1K elements (transfer overhead dominates)
//! - **Memory Bound**: 10-50× for memory bandwidth-limited operations
//!
//! ## Implementation Notes
//!
//! This is a **foundation trait** - actual GPU implementations will require:
//! - External crates (cuda, vulkan, opencl)
//! - Runtime device detection
//! - Error handling for device failures
//! - Fallback to CPU for small workloads
//!
//! ## Example Use Cases
//!
//! ```rust,ignore
//! // Brain forward pass (960K neurons × 5K connections)
//! // CPU: 350μs | GPU: <5μs = 70× speedup
//!
//! // Monte Carlo risk (1M scenarios)
//! // CPU: 100ms | GPU: 200μs = 500× speedup
//!
//! // Matrix multiply (4096×4096 f32)
//! // CPU: 50ms | GPU: 100μs = 500× speedup
//! ```

use crate::traits::ComputationalCapsule;
use core::fmt;

/// Error types for GPU capsule operations.
///
/// ## UCE33 Q20: Error Handling
///
/// GPU operations can fail in several ways:
/// - No GPU device available (runtime check)
/// - Out of GPU memory
/// - Kernel execution failure
/// - Host-device transfer failure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuError {
    /// No GPU device found (runtime detection failure)
    NoDevice,
    /// Insufficient GPU memory for allocation
    OutOfMemory {
        /// Requested bytes
        requested: usize,
        /// Available bytes
        available: usize,
    },
    /// Kernel execution failed (device error, timeout, etc.)
    KernelFailed(&'static str),
    /// Memory transfer failed (PCIe error, invalid pointer, etc.)
    TransferFailed(&'static str),
    /// Invalid configuration (grid size, block size, etc.)
    InvalidConfiguration(&'static str),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuError::NoDevice => write!(f, "No GPU device available"),
            GpuError::OutOfMemory {
                requested,
                available,
            } => {
                write!(
                    f,
                    "Out of GPU memory: requested {} bytes, {} available",
                    requested, available
                )
            }
            GpuError::KernelFailed(msg) => write!(f, "Kernel execution failed: {}", msg),
            GpuError::TransferFailed(msg) => write!(f, "Memory transfer failed: {}", msg),
            GpuError::InvalidConfiguration(msg) => {
                write!(f, "Invalid GPU configuration: {}", msg)
            }
        }
    }
}

impl core::error::Error for GpuError {}

/// Tier 7: GPU Capsule trait for massively parallel computation.
///
/// ## UCE33 Q10: Tier 7 GPU
///
/// GPU capsules provide:
/// - 100-1000× speedup for parallel workloads (B32 validated range)
/// - Massive parallelism (1000-10000 CUDA cores)
/// - High memory bandwidth (500-1500 GB/s)
///
/// ## ASSUM Framework
///
/// - `#ASSUME_GPU_AVAILABLE`: GPU device present and initialized
/// - `#VERIFY_GPU_AVAILABLE`: Runtime detection in `upload()` method
/// - `#ASSUME_MEMORY_TRANSFER`: Host-device transfer cost amortized
/// - `#VERIFY_MEMORY_TRANSFER`: B32 benchmarks include transfer time
/// - `#ASSUME_KERNEL_SAFE`: GPU kernel doesn't access invalid memory
/// - `#VERIFY_KERNEL_SAFE`: Runtime bounds checks in kernel code
///
/// ## Safety
///
/// This trait is unsafe to implement because:
/// - GPU memory management requires careful synchronization
/// - Host-device transfers can cause data races
/// - Kernel execution can fail unpredictably
///
/// ## Example
///
/// ```rust,ignore
/// use atomic_capsule::traits::gpu::{GpuCapsule, GpuError};
/// use atomic_capsule::traits::ComputationalCapsule;
///
/// #[repr(C, align(64))]
/// struct MatrixCapsule {
///     data: Vec<f32>,
///     rows: usize,
///     cols: usize,
/// }
///
/// unsafe impl ComputationalCapsule for MatrixCapsule {
///     const ALIGNMENT: usize = 64;
///     const SIZE: usize = 64; // Capsule header size
///     const TYPE_ID: &'static str = "MatrixCapsule";
/// }
///
/// unsafe impl GpuCapsule for MatrixCapsule {
///     type GpuBuffer = CudaBuffer; // Implementation-specific
///
///     fn upload(&self) -> Result<Self::GpuBuffer, GpuError> {
///         // Transfer host data to GPU
///         Ok(CudaBuffer::from_host(&self.data)?)
///     }
///
///     fn execute_kernel(&self, buffer: &mut Self::GpuBuffer) -> Result<(), GpuError> {
///         // Launch GPU kernel
///         cuda_kernel_launch(buffer)?;
///         Ok(())
///     }
///
///     fn download(&self, buffer: &Self::GpuBuffer) -> Result<(), GpuError> {
///         // Transfer GPU results back to host
///         buffer.copy_to_host(&mut self.data)?;
///         Ok(())
///     }
/// }
/// ```
pub unsafe trait GpuCapsule: ComputationalCapsule {
    /// GPU-specific buffer type.
    ///
    /// ## Implementation-Specific
    ///
    /// - CUDA: `cuda::DevicePtr<T>` or similar
    /// - Vulkan: `vk::Buffer` or similar
    /// - OpenCL: `cl::Buffer<T>` or similar
    ///
    /// This type should handle:
    /// - Device memory allocation
    /// - Memory synchronization
    /// - Device-specific addressing
    type GpuBuffer;

    /// Upload data from host to GPU device.
    ///
    /// ## UCE33 Q20: Error Handling
    ///
    /// This operation can fail if:
    /// - No GPU device is available
    /// - Insufficient GPU memory
    /// - PCIe transfer error
    ///
    /// ## B32 Reality Check
    ///
    /// - Transfer cost: 1-10ms for PCIe (depends on size)
    /// - Threshold: >100KB for amortization
    ///
    /// # Returns
    ///
    /// - `Ok(GpuBuffer)` with allocated device memory
    /// - `Err(GpuError)` if allocation or transfer fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let gpu_buffer = capsule.upload()?;
    /// // Device memory now contains host data
    /// ```
    fn upload(&self) -> Result<Self::GpuBuffer, GpuError>;

    /// Execute GPU kernel on device buffer.
    ///
    /// ## UCE33 Q20: Error Handling
    ///
    /// Kernel execution can fail if:
    /// - Invalid grid/block configuration
    /// - Kernel timeout (watchdog)
    /// - Device error (memory fault, etc.)
    ///
    /// ## B32 Reality Check
    ///
    /// - Kernel launch overhead: 50-500μs
    /// - Execution time: 1μs-10ms (depends on workload)
    /// - Synchronization overhead: 1-10μs
    ///
    /// # Arguments
    ///
    /// - `buffer`: Mutable reference to device buffer
    ///
    /// # Returns
    ///
    /// - `Ok(())` if kernel executed successfully
    /// - `Err(GpuError)` if execution failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// capsule.execute_kernel(&mut gpu_buffer)?;
    /// // Kernel has completed on device
    /// ```
    fn execute_kernel(&self, buffer: &mut Self::GpuBuffer) -> Result<(), GpuError>;

    /// Download results from GPU device to host.
    ///
    /// ## UCE33 Q20: Error Handling
    ///
    /// Download can fail if:
    /// - PCIe transfer error
    /// - Invalid host pointer
    /// - Device memory corruption
    ///
    /// ## B32 Reality Check
    ///
    /// - Transfer cost: 1-10ms (same as upload)
    /// - Total overhead: Upload + Kernel + Download = 2-20ms
    /// - Amortization: Need 100-1000× speedup to justify
    ///
    /// # Arguments
    ///
    /// - `buffer`: Reference to device buffer with results
    ///
    /// # Returns
    ///
    /// - `Ok(())` if transfer succeeded
    /// - `Err(GpuError)` if transfer failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// capsule.download(&gpu_buffer)?;
    /// // Host memory now contains results
    /// ```
    fn download(&self, buffer: &Self::GpuBuffer) -> Result<(), GpuError>;

    /// Full GPU pipeline: upload → execute → download.
    ///
    /// ## UCE33 Q28: Simplicity
    ///
    /// This convenience method combines all three operations into one call,
    /// simplifying the most common use case.
    ///
    /// ## B32 Reality Check
    ///
    /// - Total latency: 2-20ms (transfer + kernel)
    /// - Sweet spot: >100K elements (100-1000× speedup)
    /// - Small data: Use CPU (<1K elements)
    ///
    /// # Returns
    ///
    /// - `Ok(())` if entire pipeline succeeded
    /// - `Err(GpuError)` at first failure point
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Simple one-line GPU processing
    /// capsule.process_on_gpu()?;
    /// ```
    fn process_on_gpu(&mut self) -> Result<(), GpuError> {
        let mut buffer = self.upload()?;
        self.execute_kernel(&mut buffer)?;
        self.download(&buffer)?;
        Ok(())
    }

    /// Check if GPU device is available.
    ///
    /// ## ASSUM Framework
    ///
    /// - `#ASSUME_GPU_AVAILABLE`: This returns true before GPU operations
    /// - `#VERIFY_GPU_AVAILABLE`: Call this before attempting operations
    ///
    /// # Returns
    ///
    /// - `true` if at least one GPU device is available
    /// - `false` if no GPU found (fallback to CPU)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if capsule.is_gpu_available() {
    ///     capsule.process_on_gpu()?;
    /// } else {
    ///     capsule.process_on_cpu()?;
    /// }
    /// ```
    fn is_gpu_available(&self) -> bool {
        // Default implementation: assume available
        // Implementations should override with actual detection
        true
    }

    /// Get GPU device properties.
    ///
    /// ## UCE33 Q13: Resources
    ///
    /// Returns information about available GPU resources:
    /// - Compute capability
    /// - Memory capacity
    /// - Number of cores
    ///
    /// # Returns
    ///
    /// - `Some(GpuProperties)` if GPU is available
    /// - `None` if no GPU detected
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(props) = capsule.gpu_properties() {
    ///     println!("GPU Memory: {} GB", props.memory_gb);
    /// }
    /// ```
    fn gpu_properties(&self) -> Option<GpuProperties> {
        None // Default: no properties available
    }
}

/// GPU device properties.
///
/// ## UCE33 Q13: Resources
///
/// Information about available GPU hardware.
#[derive(Debug, Clone, Copy)]
pub struct GpuProperties {
    /// Device name (e.g., "NVIDIA GeForce RTX 4090")
    pub name: &'static str,
    /// Total device memory in gigabytes
    pub memory_gb: usize,
    /// Number of compute units (CUDA cores, compute units, etc.)
    pub compute_units: usize,
    /// Memory bandwidth in GB/s
    pub bandwidth_gbps: usize,
    /// Compute capability (CUDA) or equivalent
    pub compute_capability: (u32, u32),
}

impl Default for GpuProperties {
    fn default() -> Self {
        Self {
            name: "Unknown GPU",
            memory_gb: 0,
            compute_units: 0,
            bandwidth_gbps: 0,
            compute_capability: (0, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_error_display() {
        let err = GpuError::NoDevice;
        assert!(err.to_string().contains("No GPU"));

        let err = GpuError::OutOfMemory {
            requested: 1024,
            available: 512,
        };
        assert!(err.to_string().contains("1024"));
        assert!(err.to_string().contains("512"));

        let err = GpuError::KernelFailed("timeout");
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn test_gpu_properties_default() {
        let props = GpuProperties::default();
        assert_eq!(props.name, "Unknown GPU");
        assert_eq!(props.memory_gb, 0);
    }
}
