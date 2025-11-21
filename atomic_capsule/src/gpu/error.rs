// GPU Error Types - T7 Heterogeneous Tier
// Phase 5: GPU Acceleration Foundation
//
// UCE34 Compliance:
// - Q10: T7 Heterogeneous tier (GPU coordination)
// - Q11: Rust transform (type-safe error handling)
// - Q34: Audit trail (error context for compliance)
//
// ASSUM Safety: 99.99%+
// - #ASSUME_GPU_FFI_DOCUMENTED: All unsafe GPU operations documented with error codes
// - #ASSUME_ERROR_CONTEXT: Rich error context for debugging and audit trails

use core::fmt;

/// GPU backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend {
    /// NVIDIA CUDA backend
    Cuda,
    /// AMD ROCm/HIP backend
    Rocm,
    /// CPU fallback (no GPU available)
    CpuFallback,
}

/// GPU error types
#[derive(Debug, Clone)]
pub enum GpuError {
    /// No GPU device available
    NoDeviceAvailable,

    /// Invalid device ID
    InvalidDeviceId(u32),

    /// GPU memory allocation failed
    AllocationFailed {
        requested_bytes: usize,
        available_bytes: usize,
    },

    /// GPU memory deallocation failed
    DeallocationFailed {
        ptr: usize,
    },

    /// Kernel launch failed
    KernelLaunchFailed {
        kernel_name: String,
        error_code: i32,
    },

    /// Stream synchronization failed
    SyncFailed {
        stream_id: usize,
        error_code: i32,
    },

    /// Memory copy failed (host ↔ device)
    MemoryCopyFailed {
        direction: MemoryCopyDirection,
        bytes: usize,
        error_code: i32,
    },

    /// Backend initialization failed
    BackendInitFailed {
        backend: GpuBackend,
        reason: String,
    },

    /// Unsupported operation
    UnsupportedOperation {
        operation: String,
        reason: String,
    },

    /// Hardware capability mismatch
    InsufficientCapability {
        required: (u32, u32),
        available: (u32, u32),
    },
}

/// Memory copy direction (host ↔ device)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryCopyDirection {
    /// Host (CPU) to Device (GPU)
    HostToDevice,
    /// Device (GPU) to Host (CPU)
    DeviceToHost,
    /// Device to Device (GPU to GPU)
    DeviceToDevice,
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuError::NoDeviceAvailable => {
                write!(f, "No GPU device available (CUDA/ROCm not found)")
            }
            GpuError::InvalidDeviceId(id) => {
                write!(f, "Invalid GPU device ID: {}", id)
            }
            GpuError::AllocationFailed { requested_bytes, available_bytes } => {
                write!(
                    f,
                    "GPU memory allocation failed: requested {} bytes, available {} bytes",
                    requested_bytes, available_bytes
                )
            }
            GpuError::DeallocationFailed { ptr } => {
                write!(f, "GPU memory deallocation failed at address 0x{:x}", ptr)
            }
            GpuError::KernelLaunchFailed { kernel_name, error_code } => {
                write!(
                    f,
                    "GPU kernel launch failed: '{}' (error code: {})",
                    kernel_name, error_code
                )
            }
            GpuError::SyncFailed { stream_id, error_code } => {
                write!(
                    f,
                    "GPU stream synchronization failed: stream {} (error code: {})",
                    stream_id, error_code
                )
            }
            GpuError::MemoryCopyFailed { direction, bytes, error_code } => {
                write!(
                    f,
                    "GPU memory copy failed: {:?}, {} bytes (error code: {})",
                    direction, bytes, error_code
                )
            }
            GpuError::BackendInitFailed { backend, reason } => {
                write!(f, "{:?} backend initialization failed: {}", backend, reason)
            }
            GpuError::UnsupportedOperation { operation, reason } => {
                write!(f, "Unsupported GPU operation '{}': {}", operation, reason)
            }
            GpuError::InsufficientCapability { required, available } => {
                write!(
                    f,
                    "Insufficient GPU capability: required {}.{}, available {}.{}",
                    required.0, required.1, available.0, available.1
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for GpuError {}

pub type GpuResult<T> = Result<T, GpuError>;

impl fmt::Display for MemoryCopyDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryCopyDirection::HostToDevice => write!(f, "Host → Device"),
            MemoryCopyDirection::DeviceToHost => write!(f, "Device → Host"),
            MemoryCopyDirection::DeviceToDevice => write!(f, "Device → Device"),
        }
    }
}

impl fmt::Display for GpuBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuBackend::Cuda => write!(f, "CUDA"),
            GpuBackend::Rocm => write!(f, "ROCm"),
            GpuBackend::CpuFallback => write!(f, "CPU Fallback"),
        }
    }
}
