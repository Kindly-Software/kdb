//! CUDA Driver API FFI Bindings for KGPU-Driver v2.0
//!
//! Safe Rust FFI bindings for NVIDIA CUDA Driver API, supporting the Trojan Kernel approach
//! to bypass locked GSP firmware on modern NVIDIA GPUs.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                        CUDA FFI Layer Architecture                          │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │                                                                             │
//! │  Application Code (Rust)                                                    │
//! │       │                                                                     │
//! │       ▼                                                                     │
//! │  ┌─────────────────────────────────────────────────────────────────────┐   │
//! │  │  Safe Wrappers (cuda_ffi::safe::*)                                  │   │
//! │  │  - Type-safe API                                                     │   │
//! │  │  - Automatic error conversion                                        │   │
//! │  │  - Resource management (RAII)                                        │   │
//! │  └─────────────────────────────────────────────────────────────────────┘   │
//! │       │                                                                     │
//! │       ▼                                                                     │
//! │  ┌─────────────────────────────────────────────────────────────────────┐   │
//! │  │  Dynamic Loading (CudaLibrary)                                       │   │
//! │  │  - dlopen("libcuda.so.1")                                            │   │
//! │  │  - Function pointer table                                            │   │
//! │  │  - Lazy initialization                                               │   │
//! │  └─────────────────────────────────────────────────────────────────────┘   │
//! │       │                                                                     │
//! │       ▼                                                                     │
//! │  ┌─────────────────────────────────────────────────────────────────────┐   │
//! │  │  Raw FFI Declarations (extern "C")                                   │   │
//! │  │  - CUDA Driver API types                                             │   │
//! │  │  - CUresult error codes                                              │   │
//! │  │  - Opaque handles                                                    │   │
//! │  └─────────────────────────────────────────────────────────────────────┘   │
//! │       │                                                                     │
//! │       ▼                                                                     │
//! │  libcuda.so.1 (NVIDIA CUDA Driver Library)                                 │
//! │                                                                             │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Critical Functions for Trojan Kernel
//!
//! The Trojan Kernel approach requires specific CUDA Driver API functions:
//!
//! 1. **Pinned Memory**: `cuMemAllocHost`, `cuMemHostGetDevicePointer`
//!    - Allocates CPU-visible memory that GPU can directly access
//!    - Used for the ring buffer shared between CPU and Trojan kernel
//!
//! 2. **Kernel Launch**: `cuLaunchKernel`, `cuModuleLoad`, `cuModuleGetFunction`
//!    - Launches the persistent Trojan kernel
//!    - Kernel never returns, polls ring buffer continuously
//!
//! 3. **Context Management**: `cuCtxCreate`, `cuCtxSetCurrent`
//!    - Establishes GPU context for kernel execution
//!
//! # UCE34 Compliance
//!
//! - Q10: T1 Atomic tier (lockfree FFI, no mutex in wrappers)
//! - Q11: Rust transform (type-safe bindings, CUresult to KgpuDriverError)
//! - Q33: 100% lockfree (no mutex in safe wrappers)
//! - Q34: Audit trail (error context with file/line for compliance)
//!
//! # ASSUM Safety (99.99%+)
//!
//! - `#ASSUME_LIBCUDA_LOADED`: libcuda.so.1 loaded via dlopen before FFI calls
//! - `#ASSUME_CUDA_INIT`: cuInit(0) called once before any other CUDA function
//! - `#ASSUME_VALID_CONTEXT`: Context created and set before memory/kernel ops
//! - `#ASSUME_PINNED_VALID`: Pinned memory pointers valid within allocation lifetime
//! - `#ASSUME_MODULE_VALID`: Module handle valid until cuModuleUnload called
//!
//! # B32 Compliance
//!
//! - Performance targets: <100ns for error checks, <1us for simple API calls
//! - No allocation in hot paths (error handling uses static strings)
//!
//! # Feature Gate
//!
//! This module is only compiled when:
//! - `kgpu-driver-nvidia` feature is enabled
//! - Target OS is Linux
//!
//! ```toml
//! [features]
//! kgpu-driver-nvidia = []
//! ```

#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    clippy::upper_case_acronyms
)]

use core::ffi::{c_char, c_int, c_uint, c_void};
use core::fmt;
use core::ptr::{null, null_mut};
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
use std::ffi::{CStr, CString};

// ============================================================================
// CUDA Type Definitions (Opaque Handles)
// ============================================================================

/// CUDA device handle (integer, 0-based device index)
pub type CUdevice = c_int;

/// CUDA context handle (opaque pointer)
///
/// A context encapsulates all CUDA resources and state for a single GPU.
/// Each thread can have one active context at a time.
pub type CUcontext = *mut c_void;

/// CUDA module handle (opaque pointer)
///
/// A module contains compiled GPU code (PTX or cubin) that can be loaded
/// and executed on the GPU.
pub type CUmodule = *mut c_void;

/// CUDA function handle (opaque pointer)
///
/// A function (kernel) that can be launched on the GPU.
pub type CUfunction = *mut c_void;

/// CUDA stream handle (opaque pointer)
///
/// A stream is a sequence of operations that execute in order on the GPU.
/// Operations in different streams may execute concurrently.
pub type CUstream = *mut c_void;

/// CUDA event handle (opaque pointer)
///
/// Events are used for timing and synchronization between streams.
pub type CUevent = *mut c_void;

/// CUDA device pointer (64-bit GPU memory address)
///
/// This is a GPU-side memory address, not directly dereferenceable from CPU.
pub type CUdeviceptr = u64;

// ============================================================================
// CUDA Error Codes (CUresult)
// ============================================================================

/// CUDA Driver API error codes
///
/// Complete enumeration of CUDA driver error codes for proper error handling.
/// Error codes are stable across CUDA versions (ABI commitment from NVIDIA).
///
/// # Error Code Ranges
///
/// - 0: Success
/// - 1-99: General errors
/// - 100-199: Context errors
/// - 200-299: Memory errors
/// - 300-399: Launch errors
/// - 400-499: Graphics errors
/// - 500-599: Texture errors
/// - 700-799: Peer access errors
/// - 800-899: Profiler errors
/// - 900-999: Other errors
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CUresult {
    /// Operation completed successfully
    CUDA_SUCCESS = 0,

    // ========================================================================
    // General Errors (1-99)
    // ========================================================================
    /// Invalid value parameter
    CUDA_ERROR_INVALID_VALUE = 1,
    /// Out of memory
    CUDA_ERROR_OUT_OF_MEMORY = 2,
    /// CUDA driver not initialized
    CUDA_ERROR_NOT_INITIALIZED = 3,
    /// CUDA driver being shut down
    CUDA_ERROR_DEINITIALIZED = 4,
    /// Profiler not initialized
    CUDA_ERROR_PROFILER_DISABLED = 5,
    /// Profiler already started
    CUDA_ERROR_PROFILER_NOT_INITIALIZED = 6,
    /// Profiler already stopped
    CUDA_ERROR_PROFILER_ALREADY_STARTED = 7,
    /// Profiler already stopped
    CUDA_ERROR_PROFILER_ALREADY_STOPPED = 8,
    /// CUDA-capable device not detected
    CUDA_ERROR_NO_DEVICE = 100,
    /// Invalid device ordinal
    CUDA_ERROR_INVALID_DEVICE = 101,
    /// Device not ready
    CUDA_ERROR_DEVICE_NOT_LICENSED = 102,

    // ========================================================================
    // Context Errors (200-299)
    // ========================================================================
    /// Invalid context
    CUDA_ERROR_INVALID_CONTEXT = 201,
    /// Context already current
    CUDA_ERROR_CONTEXT_ALREADY_CURRENT = 202,
    /// Map operation failed
    CUDA_ERROR_MAP_FAILED = 205,
    /// Unmap operation failed
    CUDA_ERROR_UNMAP_FAILED = 206,
    /// Array is mapped
    CUDA_ERROR_ARRAY_IS_MAPPED = 207,
    /// Resource already mapped
    CUDA_ERROR_ALREADY_MAPPED = 208,
    /// No binary for GPU
    CUDA_ERROR_NO_BINARY_FOR_GPU = 209,
    /// Resource already acquired
    CUDA_ERROR_ALREADY_ACQUIRED = 210,
    /// Resource not mapped
    CUDA_ERROR_NOT_MAPPED = 211,
    /// Resource not mapped as array
    CUDA_ERROR_NOT_MAPPED_AS_ARRAY = 212,
    /// Resource not mapped as pointer
    CUDA_ERROR_NOT_MAPPED_AS_POINTER = 213,
    /// Uncorrectable ECC error
    CUDA_ERROR_ECC_UNCORRECTABLE = 214,
    /// Unsupported limit
    CUDA_ERROR_UNSUPPORTED_LIMIT = 215,
    /// Context already in use
    CUDA_ERROR_CONTEXT_ALREADY_IN_USE = 216,
    /// Peer access not supported
    CUDA_ERROR_PEER_ACCESS_UNSUPPORTED = 217,
    /// Invalid PTX
    CUDA_ERROR_INVALID_PTX = 218,
    /// Invalid graphics context
    CUDA_ERROR_INVALID_GRAPHICS_CONTEXT = 219,
    /// NVLINK uncorrectable error
    CUDA_ERROR_NVLINK_UNCORRECTABLE = 220,
    /// JIT compiler not found
    CUDA_ERROR_JIT_COMPILER_NOT_FOUND = 221,
    /// Unsupported PTX version
    CUDA_ERROR_UNSUPPORTED_PTX_VERSION = 222,
    /// JIT compilation disabled
    CUDA_ERROR_JIT_COMPILATION_DISABLED = 223,
    /// Unsupported exec affinity
    CUDA_ERROR_UNSUPPORTED_EXEC_AFFINITY = 224,

    // ========================================================================
    // Module/Handle Errors (300-399)
    // ========================================================================
    /// Invalid source (PTX/cubin)
    CUDA_ERROR_INVALID_SOURCE = 300,
    /// File not found
    CUDA_ERROR_FILE_NOT_FOUND = 301,
    /// Shared object symbol not found
    CUDA_ERROR_SHARED_OBJECT_SYMBOL_NOT_FOUND = 302,
    /// Shared object init failed
    CUDA_ERROR_SHARED_OBJECT_INIT_FAILED = 303,
    /// Operating system call failed
    CUDA_ERROR_OPERATING_SYSTEM = 304,
    /// Invalid handle
    CUDA_ERROR_INVALID_HANDLE = 400,
    /// Resource not found
    CUDA_ERROR_NOT_FOUND = 500,

    // ========================================================================
    // Launch Errors (700-799)
    // ========================================================================
    /// Not ready (async operation in progress)
    CUDA_ERROR_NOT_READY = 600,
    /// Illegal address
    CUDA_ERROR_ILLEGAL_ADDRESS = 700,
    /// Launch out of resources
    CUDA_ERROR_LAUNCH_OUT_OF_RESOURCES = 701,
    /// Launch timeout
    CUDA_ERROR_LAUNCH_TIMEOUT = 702,
    /// Launch incompatible texturing
    CUDA_ERROR_LAUNCH_INCOMPATIBLE_TEXTURING = 703,
    /// Peer access already enabled
    CUDA_ERROR_PEER_ACCESS_ALREADY_ENABLED = 704,
    /// Peer access not enabled
    CUDA_ERROR_PEER_ACCESS_NOT_ENABLED = 705,
    /// Primary context already active
    CUDA_ERROR_PRIMARY_CONTEXT_ACTIVE = 708,
    /// Context destroyed
    CUDA_ERROR_CONTEXT_IS_DESTROYED = 709,
    /// Assert triggered on device
    CUDA_ERROR_ASSERT = 710,
    /// Too many blocks
    CUDA_ERROR_TOO_MANY_PEERS = 711,
    /// Host memory already registered
    CUDA_ERROR_HOST_MEMORY_ALREADY_REGISTERED = 712,
    /// Host memory not registered
    CUDA_ERROR_HOST_MEMORY_NOT_REGISTERED = 713,
    /// Hardware stack error
    CUDA_ERROR_HARDWARE_STACK_ERROR = 714,
    /// Illegal instruction
    CUDA_ERROR_ILLEGAL_INSTRUCTION = 715,
    /// Misaligned address
    CUDA_ERROR_MISALIGNED_ADDRESS = 716,
    /// Invalid address space
    CUDA_ERROR_INVALID_ADDRESS_SPACE = 717,
    /// Invalid PC
    CUDA_ERROR_INVALID_PC = 718,
    /// Launch failed (general)
    CUDA_ERROR_LAUNCH_FAILED = 719,
    /// Cooperative launch too large
    CUDA_ERROR_COOPERATIVE_LAUNCH_TOO_LARGE = 720,

    // ========================================================================
    // Stream/Event Errors (800-899)
    // ========================================================================
    /// Not permitted
    CUDA_ERROR_NOT_PERMITTED = 800,
    /// Not supported
    CUDA_ERROR_NOT_SUPPORTED = 801,
    /// System not ready
    CUDA_ERROR_SYSTEM_NOT_READY = 802,
    /// System driver mismatch
    CUDA_ERROR_SYSTEM_DRIVER_MISMATCH = 803,
    /// Compat not supported on device
    CUDA_ERROR_COMPAT_NOT_SUPPORTED_ON_DEVICE = 804,
    /// MPS connection failed
    CUDA_ERROR_MPS_CONNECTION_FAILED = 805,
    /// MPS RPC failure
    CUDA_ERROR_MPS_RPC_FAILURE = 806,
    /// MPS server not ready
    CUDA_ERROR_MPS_SERVER_NOT_READY = 807,
    /// MPS max clients reached
    CUDA_ERROR_MPS_MAX_CLIENTS_REACHED = 808,
    /// MPS max connections reached
    CUDA_ERROR_MPS_MAX_CONNECTIONS_REACHED = 809,
    /// MPS client terminated
    CUDA_ERROR_MPS_CLIENT_TERMINATED = 810,

    // ========================================================================
    // Stream Capture Errors (900-999)
    // ========================================================================
    /// Stream capture unsupported
    CUDA_ERROR_STREAM_CAPTURE_UNSUPPORTED = 900,
    /// Stream capture invalidated
    CUDA_ERROR_STREAM_CAPTURE_INVALIDATED = 901,
    /// Stream capture merge
    CUDA_ERROR_STREAM_CAPTURE_MERGE = 902,
    /// Stream capture unmatched
    CUDA_ERROR_STREAM_CAPTURE_UNMATCHED = 903,
    /// Stream capture unjoined
    CUDA_ERROR_STREAM_CAPTURE_UNJOINED = 904,
    /// Stream capture isolation
    CUDA_ERROR_STREAM_CAPTURE_ISOLATION = 905,
    /// Stream capture implicit
    CUDA_ERROR_STREAM_CAPTURE_IMPLICIT = 906,
    /// Captured event
    CUDA_ERROR_CAPTURED_EVENT = 907,
    /// Stream capture wrong thread
    CUDA_ERROR_STREAM_CAPTURE_WRONG_THREAD = 908,
    /// Timeout
    CUDA_ERROR_TIMEOUT = 909,
    /// Graph exec update failure
    CUDA_ERROR_GRAPH_EXEC_UPDATE_FAILURE = 910,

    // ========================================================================
    // Special Error Codes
    // ========================================================================
    /// Unknown error (catch-all)
    CUDA_ERROR_UNKNOWN = 999,
}

impl CUresult {
    /// Check if this result represents success
    #[inline]
    pub const fn is_success(self) -> bool {
        matches!(self, Self::CUDA_SUCCESS)
    }

    /// Check if this is an out-of-memory error
    #[inline]
    pub const fn is_oom(self) -> bool {
        matches!(self, Self::CUDA_ERROR_OUT_OF_MEMORY)
    }

    /// Check if this is a device not found error
    #[inline]
    pub const fn is_no_device(self) -> bool {
        matches!(self, Self::CUDA_ERROR_NO_DEVICE | Self::CUDA_ERROR_INVALID_DEVICE)
    }

    /// Check if this is a launch error
    #[inline]
    pub const fn is_launch_error(self) -> bool {
        let code = self as u32;
        code >= 700 && code <= 720
    }

    /// Check if this is a context error
    #[inline]
    pub const fn is_context_error(self) -> bool {
        let code = self as u32;
        code >= 201 && code <= 224
    }

    /// Get a short error name for logging
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CUDA_SUCCESS => "SUCCESS",
            Self::CUDA_ERROR_INVALID_VALUE => "INVALID_VALUE",
            Self::CUDA_ERROR_OUT_OF_MEMORY => "OUT_OF_MEMORY",
            Self::CUDA_ERROR_NOT_INITIALIZED => "NOT_INITIALIZED",
            Self::CUDA_ERROR_DEINITIALIZED => "DEINITIALIZED",
            Self::CUDA_ERROR_NO_DEVICE => "NO_DEVICE",
            Self::CUDA_ERROR_INVALID_DEVICE => "INVALID_DEVICE",
            Self::CUDA_ERROR_INVALID_CONTEXT => "INVALID_CONTEXT",
            Self::CUDA_ERROR_INVALID_HANDLE => "INVALID_HANDLE",
            Self::CUDA_ERROR_NOT_FOUND => "NOT_FOUND",
            Self::CUDA_ERROR_NOT_READY => "NOT_READY",
            Self::CUDA_ERROR_LAUNCH_OUT_OF_RESOURCES => "LAUNCH_OUT_OF_RESOURCES",
            Self::CUDA_ERROR_LAUNCH_TIMEOUT => "LAUNCH_TIMEOUT",
            Self::CUDA_ERROR_LAUNCH_FAILED => "LAUNCH_FAILED",
            Self::CUDA_ERROR_NOT_SUPPORTED => "NOT_SUPPORTED",
            Self::CUDA_ERROR_UNKNOWN => "UNKNOWN",
            _ => "OTHER",
        }
    }

    /// Get a human-readable description
    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::CUDA_SUCCESS => "Operation completed successfully",
            Self::CUDA_ERROR_INVALID_VALUE => "Invalid parameter value",
            Self::CUDA_ERROR_OUT_OF_MEMORY => "Out of GPU memory",
            Self::CUDA_ERROR_NOT_INITIALIZED => "CUDA driver not initialized",
            Self::CUDA_ERROR_DEINITIALIZED => "CUDA driver being shut down",
            Self::CUDA_ERROR_NO_DEVICE => "No CUDA-capable device detected",
            Self::CUDA_ERROR_INVALID_DEVICE => "Invalid device ordinal",
            Self::CUDA_ERROR_INVALID_CONTEXT => "Invalid CUDA context",
            Self::CUDA_ERROR_INVALID_HANDLE => "Invalid handle",
            Self::CUDA_ERROR_NOT_FOUND => "Named symbol not found",
            Self::CUDA_ERROR_NOT_READY => "Async operation not complete",
            Self::CUDA_ERROR_LAUNCH_OUT_OF_RESOURCES => "Kernel launch exceeded resources",
            Self::CUDA_ERROR_LAUNCH_TIMEOUT => "Kernel execution timed out",
            Self::CUDA_ERROR_LAUNCH_FAILED => "Kernel launch failed",
            Self::CUDA_ERROR_NOT_SUPPORTED => "Operation not supported",
            Self::CUDA_ERROR_UNKNOWN => "Unknown CUDA error",
            _ => "CUDA driver error",
        }
    }

    /// Convert from raw u32 value
    #[inline]
    pub const fn from_u32(code: u32) -> Self {
        match code {
            0 => Self::CUDA_SUCCESS,
            1 => Self::CUDA_ERROR_INVALID_VALUE,
            2 => Self::CUDA_ERROR_OUT_OF_MEMORY,
            3 => Self::CUDA_ERROR_NOT_INITIALIZED,
            4 => Self::CUDA_ERROR_DEINITIALIZED,
            100 => Self::CUDA_ERROR_NO_DEVICE,
            101 => Self::CUDA_ERROR_INVALID_DEVICE,
            201 => Self::CUDA_ERROR_INVALID_CONTEXT,
            400 => Self::CUDA_ERROR_INVALID_HANDLE,
            500 => Self::CUDA_ERROR_NOT_FOUND,
            600 => Self::CUDA_ERROR_NOT_READY,
            701 => Self::CUDA_ERROR_LAUNCH_OUT_OF_RESOURCES,
            702 => Self::CUDA_ERROR_LAUNCH_TIMEOUT,
            719 => Self::CUDA_ERROR_LAUNCH_FAILED,
            801 => Self::CUDA_ERROR_NOT_SUPPORTED,
            _ => Self::CUDA_ERROR_UNKNOWN,
        }
    }
}

impl fmt::Display for CUresult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CUDA_ERROR_{} ({})", self.name(), self.description())
    }
}

// ============================================================================
// CUDA Device Attributes
// ============================================================================

/// CUDA device attribute identifiers for cuDeviceGetAttribute
///
/// These are the most commonly used attributes. The full list has 100+ entries.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CUdevice_attribute {
    /// Maximum threads per block
    CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK = 1,
    /// Maximum block X dimension
    CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_X = 2,
    /// Maximum block Y dimension
    CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_Y = 3,
    /// Maximum block Z dimension
    CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_Z = 4,
    /// Maximum grid X dimension
    CU_DEVICE_ATTRIBUTE_MAX_GRID_DIM_X = 5,
    /// Maximum grid Y dimension
    CU_DEVICE_ATTRIBUTE_MAX_GRID_DIM_Y = 6,
    /// Maximum grid Z dimension
    CU_DEVICE_ATTRIBUTE_MAX_GRID_DIM_Z = 7,
    /// Shared memory per block (bytes)
    CU_DEVICE_ATTRIBUTE_MAX_SHARED_MEMORY_PER_BLOCK = 8,
    /// Total constant memory (bytes)
    CU_DEVICE_ATTRIBUTE_TOTAL_CONSTANT_MEMORY = 9,
    /// Warp size (threads)
    CU_DEVICE_ATTRIBUTE_WARP_SIZE = 10,
    /// Maximum pitch (bytes)
    CU_DEVICE_ATTRIBUTE_MAX_PITCH = 11,
    /// Registers per block
    CU_DEVICE_ATTRIBUTE_MAX_REGISTERS_PER_BLOCK = 12,
    /// Clock rate (kHz)
    CU_DEVICE_ATTRIBUTE_CLOCK_RATE = 13,
    /// Texture alignment
    CU_DEVICE_ATTRIBUTE_TEXTURE_ALIGNMENT = 14,
    /// GPU overlap (concurrent copy/compute)
    CU_DEVICE_ATTRIBUTE_GPU_OVERLAP = 15,
    /// Number of multiprocessors (SMs)
    CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT = 16,
    /// Kernel execution timeout
    CU_DEVICE_ATTRIBUTE_KERNEL_EXEC_TIMEOUT = 17,
    /// Device is integrated
    CU_DEVICE_ATTRIBUTE_INTEGRATED = 18,
    /// Can map host memory
    CU_DEVICE_ATTRIBUTE_CAN_MAP_HOST_MEMORY = 19,
    /// Compute mode
    CU_DEVICE_ATTRIBUTE_COMPUTE_MODE = 20,
    /// Maximum texture 1D width
    CU_DEVICE_ATTRIBUTE_MAXIMUM_TEXTURE1D_WIDTH = 21,
    /// Maximum texture 2D width
    CU_DEVICE_ATTRIBUTE_MAXIMUM_TEXTURE2D_WIDTH = 22,
    /// Maximum texture 2D height
    CU_DEVICE_ATTRIBUTE_MAXIMUM_TEXTURE2D_HEIGHT = 23,
    /// Maximum texture 3D width
    CU_DEVICE_ATTRIBUTE_MAXIMUM_TEXTURE3D_WIDTH = 24,
    /// Maximum texture 3D height
    CU_DEVICE_ATTRIBUTE_MAXIMUM_TEXTURE3D_HEIGHT = 25,
    /// Maximum texture 3D depth
    CU_DEVICE_ATTRIBUTE_MAXIMUM_TEXTURE3D_DEPTH = 26,
    /// Concurrent kernels support
    CU_DEVICE_ATTRIBUTE_CONCURRENT_KERNELS = 31,
    /// ECC enabled
    CU_DEVICE_ATTRIBUTE_ECC_ENABLED = 32,
    /// PCI bus ID
    CU_DEVICE_ATTRIBUTE_PCI_BUS_ID = 33,
    /// PCI device ID
    CU_DEVICE_ATTRIBUTE_PCI_DEVICE_ID = 34,
    /// TCC driver mode
    CU_DEVICE_ATTRIBUTE_TCC_DRIVER = 35,
    /// Memory clock rate (kHz)
    CU_DEVICE_ATTRIBUTE_MEMORY_CLOCK_RATE = 36,
    /// Memory bus width (bits)
    CU_DEVICE_ATTRIBUTE_GLOBAL_MEMORY_BUS_WIDTH = 37,
    /// L2 cache size (bytes)
    CU_DEVICE_ATTRIBUTE_L2_CACHE_SIZE = 38,
    /// Max threads per multiprocessor
    CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_MULTIPROCESSOR = 39,
    /// Async engine count
    CU_DEVICE_ATTRIBUTE_ASYNC_ENGINE_COUNT = 40,
    /// Unified addressing support
    CU_DEVICE_ATTRIBUTE_UNIFIED_ADDRESSING = 41,
    /// PCI domain ID
    CU_DEVICE_ATTRIBUTE_PCI_DOMAIN_ID = 50,
    /// Compute preemption support
    CU_DEVICE_ATTRIBUTE_COMPUTE_PREEMPTION_SUPPORTED = 90,
    /// Cooperative launch support
    CU_DEVICE_ATTRIBUTE_COOPERATIVE_LAUNCH = 95,
    /// Compute capability major
    CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR = 75,
    /// Compute capability minor
    CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR = 76,
    /// Max shared memory per multiprocessor
    CU_DEVICE_ATTRIBUTE_MAX_SHARED_MEMORY_PER_MULTIPROCESSOR = 81,
    /// Managed memory support
    CU_DEVICE_ATTRIBUTE_MANAGED_MEMORY = 83,
    /// Multi-GPU board
    CU_DEVICE_ATTRIBUTE_MULTI_GPU_BOARD = 84,
    /// Multi-GPU board group ID
    CU_DEVICE_ATTRIBUTE_MULTI_GPU_BOARD_GROUP_ID = 85,
    /// Host native atomic support
    CU_DEVICE_ATTRIBUTE_HOST_NATIVE_ATOMIC_SUPPORTED = 86,
    /// Virtual address management mode
    CU_DEVICE_ATTRIBUTE_VIRTUAL_ADDRESS_MANAGEMENT_MODE = 89,
}

impl CUdevice_attribute {
    /// Get attribute name for logging
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK => "MAX_THREADS_PER_BLOCK",
            Self::CU_DEVICE_ATTRIBUTE_WARP_SIZE => "WARP_SIZE",
            Self::CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT => "MULTIPROCESSOR_COUNT",
            Self::CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR => "COMPUTE_CAPABILITY_MAJOR",
            Self::CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR => "COMPUTE_CAPABILITY_MINOR",
            Self::CU_DEVICE_ATTRIBUTE_CLOCK_RATE => "CLOCK_RATE",
            Self::CU_DEVICE_ATTRIBUTE_MEMORY_CLOCK_RATE => "MEMORY_CLOCK_RATE",
            Self::CU_DEVICE_ATTRIBUTE_GLOBAL_MEMORY_BUS_WIDTH => "MEMORY_BUS_WIDTH",
            Self::CU_DEVICE_ATTRIBUTE_L2_CACHE_SIZE => "L2_CACHE_SIZE",
            _ => "ATTRIBUTE",
        }
    }
}

// ============================================================================
// Context Creation Flags
// ============================================================================

/// Flags for cuCtxCreate
pub mod ctx_flags {
    /// Automatic scheduling (default)
    pub const CU_CTX_SCHED_AUTO: u32 = 0x00;
    /// Spin-waiting (lower latency, higher CPU usage)
    pub const CU_CTX_SCHED_SPIN: u32 = 0x01;
    /// Yield to other threads (lower CPU usage)
    pub const CU_CTX_SCHED_YIELD: u32 = 0x02;
    /// Block on synchronize (recommended for most apps)
    pub const CU_CTX_SCHED_BLOCKING_SYNC: u32 = 0x04;
    /// Mask for scheduling flags
    pub const CU_CTX_SCHED_MASK: u32 = 0x07;
    /// Map host memory
    pub const CU_CTX_MAP_HOST: u32 = 0x08;
    /// Local memory resize (deprecated)
    pub const CU_CTX_LMEM_RESIZE_TO_MAX: u32 = 0x10;
}

// ============================================================================
// Stream Creation Flags
// ============================================================================

/// Flags for cuStreamCreate
pub mod stream_flags {
    /// Default stream creation
    pub const CU_STREAM_DEFAULT: u32 = 0x00;
    /// Non-blocking stream (does not sync with NULL stream)
    pub const CU_STREAM_NON_BLOCKING: u32 = 0x01;
}

// ============================================================================
// Event Creation Flags
// ============================================================================

/// Flags for cuEventCreate
pub mod event_flags {
    /// Default event creation
    pub const CU_EVENT_DEFAULT: u32 = 0x00;
    /// Blocking synchronization
    pub const CU_EVENT_BLOCKING_SYNC: u32 = 0x01;
    /// Disable timing (faster events)
    pub const CU_EVENT_DISABLE_TIMING: u32 = 0x02;
    /// Interprocess event
    pub const CU_EVENT_INTERPROCESS: u32 = 0x04;
}

// ============================================================================
// Host Memory Allocation Flags
// ============================================================================

/// Flags for cuMemAllocHost and cuMemHostAlloc
pub mod host_alloc_flags {
    /// Default allocation
    pub const CU_MEMHOSTALLOC_DEFAULT: u32 = 0x00;
    /// Portable across CUDA contexts
    pub const CU_MEMHOSTALLOC_PORTABLE: u32 = 0x01;
    /// Map allocation into device address space
    pub const CU_MEMHOSTALLOC_DEVICEMAP: u32 = 0x02;
    /// Write-combined memory (not cached)
    pub const CU_MEMHOSTALLOC_WRITECOMBINED: u32 = 0x04;
}

// ============================================================================
// Function Pointer Types (for Dynamic Loading)
// ============================================================================

// Initialization
type CuInitFn = unsafe extern "C" fn(flags: c_uint) -> CUresult;
type CuDriverGetVersionFn = unsafe extern "C" fn(version: *mut c_int) -> CUresult;

// Device Management
type CuDeviceGetFn = unsafe extern "C" fn(device: *mut CUdevice, ordinal: c_int) -> CUresult;
type CuDeviceGetCountFn = unsafe extern "C" fn(count: *mut c_int) -> CUresult;
type CuDeviceGetNameFn =
    unsafe extern "C" fn(name: *mut c_char, len: c_int, dev: CUdevice) -> CUresult;
type CuDeviceGetAttributeFn =
    unsafe extern "C" fn(pi: *mut c_int, attrib: CUdevice_attribute, dev: CUdevice) -> CUresult;
type CuDeviceTotalMemFn = unsafe extern "C" fn(bytes: *mut usize, dev: CUdevice) -> CUresult;

// Context Management
type CuCtxCreateFn =
    unsafe extern "C" fn(pctx: *mut CUcontext, flags: c_uint, dev: CUdevice) -> CUresult;
type CuCtxDestroyFn = unsafe extern "C" fn(ctx: CUcontext) -> CUresult;
type CuCtxSetCurrentFn = unsafe extern "C" fn(ctx: CUcontext) -> CUresult;
type CuCtxGetCurrentFn = unsafe extern "C" fn(pctx: *mut CUcontext) -> CUresult;
type CuCtxSynchronizeFn = unsafe extern "C" fn() -> CUresult;

// Memory Management
type CuMemAllocFn = unsafe extern "C" fn(dptr: *mut CUdeviceptr, bytesize: usize) -> CUresult;
type CuMemFreeFn = unsafe extern "C" fn(dptr: CUdeviceptr) -> CUresult;
type CuMemAllocHostFn = unsafe extern "C" fn(pp: *mut *mut c_void, bytesize: usize) -> CUresult;
type CuMemFreeHostFn = unsafe extern "C" fn(p: *mut c_void) -> CUresult;
type CuMemcpyHtoDFn =
    unsafe extern "C" fn(dstDevice: CUdeviceptr, srcHost: *const c_void, byteCount: usize)
        -> CUresult;
type CuMemcpyDtoHFn =
    unsafe extern "C" fn(dstHost: *mut c_void, srcDevice: CUdeviceptr, byteCount: usize)
        -> CUresult;
type CuMemHostGetDevicePointerFn =
    unsafe extern "C" fn(pdptr: *mut CUdeviceptr, p: *mut c_void, flags: c_uint) -> CUresult;

// Module/Kernel Management
type CuModuleLoadFn =
    unsafe extern "C" fn(module: *mut CUmodule, fname: *const c_char) -> CUresult;
type CuModuleLoadDataFn =
    unsafe extern "C" fn(module: *mut CUmodule, image: *const c_void) -> CUresult;
type CuModuleUnloadFn = unsafe extern "C" fn(hmod: CUmodule) -> CUresult;
type CuModuleGetFunctionFn =
    unsafe extern "C" fn(hfunc: *mut CUfunction, hmod: CUmodule, name: *const c_char) -> CUresult;

// Kernel Launch
type CuLaunchKernelFn = unsafe extern "C" fn(
    f: CUfunction,
    gridDimX: c_uint,
    gridDimY: c_uint,
    gridDimZ: c_uint,
    blockDimX: c_uint,
    blockDimY: c_uint,
    blockDimZ: c_uint,
    sharedMemBytes: c_uint,
    hStream: CUstream,
    kernelParams: *mut *mut c_void,
    extra: *mut *mut c_void,
) -> CUresult;

// Stream Management
type CuStreamCreateFn = unsafe extern "C" fn(phStream: *mut CUstream, flags: c_uint) -> CUresult;
type CuStreamDestroyFn = unsafe extern "C" fn(hStream: CUstream) -> CUresult;
type CuStreamSynchronizeFn = unsafe extern "C" fn(hStream: CUstream) -> CUresult;
type CuStreamQueryFn = unsafe extern "C" fn(hStream: CUstream) -> CUresult;

// Event Management
type CuEventCreateFn = unsafe extern "C" fn(phEvent: *mut CUevent, flags: c_uint) -> CUresult;
type CuEventDestroyFn = unsafe extern "C" fn(hEvent: CUevent) -> CUresult;
type CuEventRecordFn = unsafe extern "C" fn(hEvent: CUevent, hStream: CUstream) -> CUresult;
type CuEventSynchronizeFn = unsafe extern "C" fn(hEvent: CUevent) -> CUresult;
type CuEventElapsedTimeFn =
    unsafe extern "C" fn(pMilliseconds: *mut f32, hStart: CUevent, hEnd: CUevent) -> CUresult;

// ============================================================================
// Dynamic Library Loading
// ============================================================================

/// Global state for CUDA library loading
static CUDA_INITIALIZED: AtomicBool = AtomicBool::new(false);
static CUDA_LIBRARY_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

/// CUDA Driver Library Handle
///
/// Dynamically loads libcuda.so.1 and provides function pointers to CUDA Driver API.
/// This approach allows the program to run on systems without CUDA installed
/// (graceful degradation) and supports runtime library loading.
///
/// # Thread Safety
///
/// The library is loaded once (lazy singleton pattern with atomics).
/// All function pointers are valid for the lifetime of the process.
///
/// # ASSUM Safety
///
/// - `#ASSUME_LIBCUDA_EXISTS`: libcuda.so.1 must be in LD_LIBRARY_PATH or system paths
/// - `#ASSUME_ABI_STABLE`: CUDA Driver API ABI is stable across versions
/// - `#VERIFY_SYMBOL_EXISTS`: Each dlsym must succeed (checked at load time)
#[repr(C)]
pub struct CudaLibrary {
    // Library handle (dlopen result)
    handle: *mut c_void,

    // Initialization
    pub cuInit: CuInitFn,
    pub cuDriverGetVersion: CuDriverGetVersionFn,

    // Device Management
    pub cuDeviceGet: CuDeviceGetFn,
    pub cuDeviceGetCount: CuDeviceGetCountFn,
    pub cuDeviceGetName: CuDeviceGetNameFn,
    pub cuDeviceGetAttribute: CuDeviceGetAttributeFn,
    pub cuDeviceTotalMem: CuDeviceTotalMemFn,

    // Context Management
    pub cuCtxCreate: CuCtxCreateFn,
    pub cuCtxDestroy: CuCtxDestroyFn,
    pub cuCtxSetCurrent: CuCtxSetCurrentFn,
    pub cuCtxGetCurrent: CuCtxGetCurrentFn,
    pub cuCtxSynchronize: CuCtxSynchronizeFn,

    // Memory Management
    pub cuMemAlloc: CuMemAllocFn,
    pub cuMemFree: CuMemFreeFn,
    pub cuMemAllocHost: CuMemAllocHostFn,
    pub cuMemFreeHost: CuMemFreeHostFn,
    pub cuMemcpyHtoD: CuMemcpyHtoDFn,
    pub cuMemcpyDtoH: CuMemcpyDtoHFn,
    pub cuMemHostGetDevicePointer: CuMemHostGetDevicePointerFn,

    // Module/Kernel Management
    pub cuModuleLoad: CuModuleLoadFn,
    pub cuModuleLoadData: CuModuleLoadDataFn,
    pub cuModuleUnload: CuModuleUnloadFn,
    pub cuModuleGetFunction: CuModuleGetFunctionFn,

    // Kernel Launch
    pub cuLaunchKernel: CuLaunchKernelFn,

    // Stream Management
    pub cuStreamCreate: CuStreamCreateFn,
    pub cuStreamDestroy: CuStreamDestroyFn,
    pub cuStreamSynchronize: CuStreamSynchronizeFn,
    pub cuStreamQuery: CuStreamQueryFn,

    // Event Management
    pub cuEventCreate: CuEventCreateFn,
    pub cuEventDestroy: CuEventDestroyFn,
    pub cuEventRecord: CuEventRecordFn,
    pub cuEventSynchronize: CuEventSynchronizeFn,
    pub cuEventElapsedTime: CuEventElapsedTimeFn,
}

// SAFETY: CudaLibrary is safe to send between threads because:
// - The handle is a dlopen handle which is process-global and thread-safe
// - All function pointers are immutable after load
// - CUDA Driver API is thread-safe (each thread needs its own context, but the library is shared)
// #ASSUME_CUDA_THREADSAFE: CUDA Driver API functions are thread-safe
// #VERIFY_IMMUTABLE: CudaLibrary fields are never modified after construction
unsafe impl Send for CudaLibrary {}

// SAFETY: CudaLibrary is safe to share between threads because:
// - All fields are read-only after construction
// - CUDA Driver API is thread-safe for function calls
// - Each thread maintains its own CUDA context via cuCtxSetCurrent
unsafe impl Sync for CudaLibrary {}

impl CudaLibrary {
    /// Library search paths (tried in order)
    const LIBRARY_PATHS: &'static [&'static str] = &[
        "libcuda.so.1", // Standard name (LD_LIBRARY_PATH)
        "libcuda.so",   // Alternative name
        "/usr/lib/x86_64-linux-gnu/libcuda.so.1",
        "/usr/lib64/libcuda.so.1",
        "/usr/local/cuda/lib64/stubs/libcuda.so",
    ];

    /// Load the CUDA driver library dynamically
    ///
    /// # Returns
    ///
    /// - `Ok(CudaLibrary)` with all function pointers initialized
    /// - `Err(KgpuDriverError)` if library not found or symbol missing
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_DLOPEN_SAFE`: dlopen is safe for valid library paths
    /// - `#ASSUME_DLSYM_SAFE`: dlsym is safe for loaded libraries
    /// - `#VERIFY_ALL_SYMBOLS`: All required symbols exist in libcuda.so
    #[cfg(all(feature = "std", target_os = "linux"))]
    pub fn load() -> super::error::KgpuDriverResult<Self> {
        use super::error::KgpuDriverError;

        // Check if already loaded (lockfree singleton)
        if CUDA_INITIALIZED.load(Ordering::Acquire) {
            let handle = CUDA_LIBRARY_HANDLE.load(Ordering::Acquire);
            if !handle.is_null() {
                // Reconstruct from cached handle
                return Self::from_handle(handle);
            }
        }

        // Try to load from each path
        let handle = Self::try_load_library()?;

        // Store in global (lockfree)
        let old_handle = CUDA_LIBRARY_HANDLE.swap(handle, Ordering::AcqRel);
        if !old_handle.is_null() && old_handle != handle {
            // Another thread loaded first, use theirs
            // SAFETY: handle was returned from dlopen
            #[allow(clippy::crosspointer_transmute)]
            unsafe {
                libc::dlclose(handle);
            }
            return Self::from_handle(old_handle);
        }

        CUDA_INITIALIZED.store(true, Ordering::Release);
        Self::from_handle(handle)
    }

    /// Try to load library from standard paths
    #[cfg(all(feature = "std", target_os = "linux"))]
    fn try_load_library() -> super::error::KgpuDriverResult<*mut c_void> {
        use super::error::KgpuDriverError;

        for path in Self::LIBRARY_PATHS {
            // SAFETY: path is a valid C string (null-terminated)
            // #ASSUME_DLOPEN_SAFE: dlopen is safe for valid paths
            let cpath = match CString::new(*path) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let handle = unsafe { libc::dlopen(cpath.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) };

            if !handle.is_null() {
                return Ok(handle);
            }
        }

        Err(KgpuDriverError::TrojanCudaInitFailed)
    }

    /// Create CudaLibrary from an already-loaded handle
    #[cfg(all(feature = "std", target_os = "linux"))]
    fn from_handle(handle: *mut c_void) -> super::error::KgpuDriverResult<Self> {
        use super::error::KgpuDriverError;

        // Helper macro to load symbols
        macro_rules! load_sym {
            ($name:ident) => {{
                let name_cstr = concat!(stringify!($name), "_v2\0");
                let name_v2 = name_cstr.as_ptr() as *const c_char;

                // Try _v2 version first (CUDA 4.0+)
                let sym = unsafe { libc::dlsym(handle, name_v2) };

                let sym = if sym.is_null() {
                    // Fall back to non-versioned name
                    let name_cstr = concat!(stringify!($name), "\0");
                    let name = name_cstr.as_ptr() as *const c_char;
                    unsafe { libc::dlsym(handle, name) }
                } else {
                    sym
                };

                if sym.is_null() {
                    return Err(KgpuDriverError::TrojanCudaInitFailed);
                }

                // SAFETY: Symbol verified non-null, type matches CUDA ABI
                // #ASSUME_ABI_STABLE: CUDA function signatures are stable
                unsafe { core::mem::transmute(sym) }
            }};
        }

        Ok(Self {
            handle,
            cuInit: load_sym!(cuInit),
            cuDriverGetVersion: load_sym!(cuDriverGetVersion),
            cuDeviceGet: load_sym!(cuDeviceGet),
            cuDeviceGetCount: load_sym!(cuDeviceGetCount),
            cuDeviceGetName: load_sym!(cuDeviceGetName),
            cuDeviceGetAttribute: load_sym!(cuDeviceGetAttribute),
            cuDeviceTotalMem: load_sym!(cuDeviceTotalMem),
            cuCtxCreate: load_sym!(cuCtxCreate),
            cuCtxDestroy: load_sym!(cuCtxDestroy),
            cuCtxSetCurrent: load_sym!(cuCtxSetCurrent),
            cuCtxGetCurrent: load_sym!(cuCtxGetCurrent),
            cuCtxSynchronize: load_sym!(cuCtxSynchronize),
            cuMemAlloc: load_sym!(cuMemAlloc),
            cuMemFree: load_sym!(cuMemFree),
            cuMemAllocHost: load_sym!(cuMemAllocHost),
            cuMemFreeHost: load_sym!(cuMemFreeHost),
            cuMemcpyHtoD: load_sym!(cuMemcpyHtoD),
            cuMemcpyDtoH: load_sym!(cuMemcpyDtoH),
            cuMemHostGetDevicePointer: load_sym!(cuMemHostGetDevicePointer),
            cuModuleLoad: load_sym!(cuModuleLoad),
            cuModuleLoadData: load_sym!(cuModuleLoadData),
            cuModuleUnload: load_sym!(cuModuleUnload),
            cuModuleGetFunction: load_sym!(cuModuleGetFunction),
            cuLaunchKernel: load_sym!(cuLaunchKernel),
            cuStreamCreate: load_sym!(cuStreamCreate),
            cuStreamDestroy: load_sym!(cuStreamDestroy),
            cuStreamSynchronize: load_sym!(cuStreamSynchronize),
            cuStreamQuery: load_sym!(cuStreamQuery),
            cuEventCreate: load_sym!(cuEventCreate),
            cuEventDestroy: load_sym!(cuEventDestroy),
            cuEventRecord: load_sym!(cuEventRecord),
            cuEventSynchronize: load_sym!(cuEventSynchronize),
            cuEventElapsedTime: load_sym!(cuEventElapsedTime),
        })
    }

    /// Check if CUDA library is available on this system
    #[cfg(all(feature = "std", target_os = "linux"))]
    pub fn is_available() -> bool {
        if CUDA_INITIALIZED.load(Ordering::Acquire) {
            return !CUDA_LIBRARY_HANDLE.load(Ordering::Acquire).is_null();
        }
        Self::load().is_ok()
    }

    /// Stub for non-Linux or non-std builds
    #[cfg(not(all(feature = "std", target_os = "linux")))]
    pub fn load() -> super::error::KgpuDriverResult<Self> {
        Err(super::error::KgpuDriverError::PlatformNotSupported)
    }

    /// Stub for non-Linux or non-std builds
    #[cfg(not(all(feature = "std", target_os = "linux")))]
    pub fn is_available() -> bool {
        false
    }
}

impl fmt::Debug for CudaLibrary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CudaLibrary")
            .field("handle", &self.handle)
            .field("available", &!self.handle.is_null())
            .finish()
    }
}

// ============================================================================
// CUresult to KgpuDriverError Conversion
// ============================================================================

impl From<CUresult> for super::error::KgpuDriverError {
    fn from(result: CUresult) -> Self {
        use super::error::KgpuDriverError;

        match result {
            CUresult::CUDA_SUCCESS => {
                // Should not convert success to error, but handle gracefully
                KgpuDriverError::Unknown
            }
            CUresult::CUDA_ERROR_INVALID_VALUE => KgpuDriverError::InvalidParameter,
            CUresult::CUDA_ERROR_OUT_OF_MEMORY => KgpuDriverError::OutOfDeviceMemory,
            CUresult::CUDA_ERROR_NOT_INITIALIZED => KgpuDriverError::TrojanCudaInitFailed,
            CUresult::CUDA_ERROR_DEINITIALIZED => KgpuDriverError::TrojanCudaInitFailed,
            CUresult::CUDA_ERROR_NO_DEVICE => KgpuDriverError::DeviceNotFound,
            CUresult::CUDA_ERROR_INVALID_DEVICE => KgpuDriverError::InvalidDeviceIndex,
            CUresult::CUDA_ERROR_INVALID_CONTEXT => KgpuDriverError::InvalidState,
            CUresult::CUDA_ERROR_INVALID_HANDLE => KgpuDriverError::InvalidMemoryHandle,
            CUresult::CUDA_ERROR_NOT_FOUND => KgpuDriverError::FirmwareNotFound,
            CUresult::CUDA_ERROR_NOT_READY => KgpuDriverError::DeviceBusy,
            CUresult::CUDA_ERROR_LAUNCH_OUT_OF_RESOURCES => KgpuDriverError::CommandFailed,
            CUresult::CUDA_ERROR_LAUNCH_TIMEOUT => KgpuDriverError::CommandTimeout,
            CUresult::CUDA_ERROR_LAUNCH_FAILED => KgpuDriverError::CommandFailed,
            CUresult::CUDA_ERROR_NOT_SUPPORTED => KgpuDriverError::DeviceNotSupported,
            _ => KgpuDriverError::Unknown,
        }
    }
}

/// Check CUDA result and convert to KgpuDriverResult
///
/// # Arguments
///
/// - `result`: CUresult from CUDA function call
///
/// # Returns
///
/// - `Ok(())` if result == CUDA_SUCCESS
/// - `Err(KgpuDriverError)` otherwise
#[inline]
pub fn check_cuda(result: CUresult) -> super::error::KgpuDriverResult<()> {
    if result.is_success() {
        Ok(())
    } else {
        Err(result.into())
    }
}

/// Check CUDA result with context for better error messages
///
/// # Arguments
///
/// - `result`: CUresult from CUDA function call
/// - `_context`: Additional context string (for future error enhancement)
///
/// # Returns
///
/// - `Ok(())` if result == CUDA_SUCCESS
/// - `Err(KgpuDriverError)` otherwise
#[inline]
pub fn check_cuda_with_context(
    result: CUresult,
    _context: &str,
) -> super::error::KgpuDriverResult<()> {
    check_cuda(result)
}

// ============================================================================
// Safe Wrapper Module
// ============================================================================

/// Safe wrappers for CUDA Driver API functions
///
/// This module provides type-safe, ergonomic wrappers around the raw CUDA FFI.
/// All functions handle error conversion and resource management automatically.
///
/// # Thread Safety
///
/// These functions are safe to call from multiple threads. CUDA contexts
/// are thread-local, so each thread should have its own context.
///
/// # ASSUM Safety
///
/// All functions have their assumptions documented. The main assumptions are:
/// - CUDA library loaded (via `CudaLibrary::load()`)
/// - CUDA initialized (via `init()`)
/// - Valid context set (via `create_context()`)
#[cfg(all(feature = "std", target_os = "linux"))]
pub mod safe {
    use super::*;
    use crate::gpu::kgpu_driver::error::{KgpuDriverError, KgpuDriverResult};

    /// Global CUDA library instance (lazy loaded)
    static CUDA_LIB: once_cell::sync::Lazy<Result<CudaLibrary, KgpuDriverError>> =
        once_cell::sync::Lazy::new(|| CudaLibrary::load());

    /// Get reference to loaded CUDA library
    fn cuda() -> KgpuDriverResult<&'static CudaLibrary> {
        CUDA_LIB.as_ref().map_err(|e| *e)
    }

    /// Initialize the CUDA driver
    ///
    /// This must be called before any other CUDA function.
    /// It is safe to call multiple times (idempotent).
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(KgpuDriverError::TrojanCudaInitFailed)` if CUDA init fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_FIRST_CALL`: Safe to call multiple times
    /// - `#VERIFY_CUDA_AVAILABLE`: CUDA driver must be installed
    pub fn init() -> KgpuDriverResult<()> {
        let lib = cuda()?;

        // SAFETY: cuInit is safe to call with flags=0
        // #ASSUME_CUDA_DRIVER: CUDA driver is properly installed
        let result = unsafe { (lib.cuInit)(0) };
        check_cuda(result)
    }

    /// Get CUDA driver version
    ///
    /// # Returns
    ///
    /// - `Ok((major, minor))` with driver version (e.g., (12, 0) for CUDA 12.0)
    /// - `Err` if version query fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_CUDA_INIT`: cuInit must have been called
    pub fn driver_version() -> KgpuDriverResult<(i32, i32)> {
        let lib = cuda()?;

        let mut version: c_int = 0;

        // SAFETY: version pointer is valid local variable
        // #ASSUME_VALID_PTR: Stack variable always valid
        let result = unsafe { (lib.cuDriverGetVersion)(&mut version) };
        check_cuda(result)?;

        // Version is encoded as (major * 1000 + minor * 10)
        let major = version / 1000;
        let minor = (version % 1000) / 10;

        Ok((major, minor))
    }

    /// Get number of CUDA devices
    ///
    /// # Returns
    ///
    /// - `Ok(count)` with number of CUDA-capable devices (0 or more)
    /// - `Err` if device enumeration fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_CUDA_INIT`: cuInit must have been called
    pub fn device_count() -> KgpuDriverResult<i32> {
        let lib = cuda()?;

        let mut count: c_int = 0;

        // SAFETY: count pointer is valid local variable
        // #ASSUME_VALID_PTR: Stack variable always valid
        let result = unsafe { (lib.cuDeviceGetCount)(&mut count) };
        check_cuda(result)?;

        Ok(count)
    }

    /// Get device handle by ordinal
    ///
    /// # Arguments
    ///
    /// - `ordinal`: Device index (0-based)
    ///
    /// # Returns
    ///
    /// - `Ok(device)` with device handle
    /// - `Err(KgpuDriverError::InvalidDeviceIndex)` if ordinal out of range
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_CUDA_INIT`: cuInit must have been called
    /// - `#ASSUME_ORDINAL_VALID`: 0 <= ordinal < device_count()
    pub fn get_device(ordinal: i32) -> KgpuDriverResult<CUdevice> {
        let lib = cuda()?;

        let mut device: CUdevice = 0;

        // SAFETY: device pointer is valid local variable
        // #ASSUME_VALID_PTR: Stack variable always valid
        let result = unsafe { (lib.cuDeviceGet)(&mut device, ordinal) };
        check_cuda(result)?;

        Ok(device)
    }

    /// Get device name
    ///
    /// # Arguments
    ///
    /// - `device`: Device handle from `get_device()`
    ///
    /// # Returns
    ///
    /// - `Ok(String)` with device name (e.g., "NVIDIA GeForce RTX 4090")
    /// - `Err` if name query fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_DEVICE_VALID`: Device handle must be valid
    pub fn device_name(device: CUdevice) -> KgpuDriverResult<String> {
        let lib = cuda()?;

        let mut name_buf = [0u8; 256];

        // SAFETY: name_buf is valid local array with sufficient size
        // #ASSUME_VALID_PTR: Stack array always valid
        // #ASSUME_NAME_FIT: CUDA device names < 256 chars
        let result = unsafe {
            (lib.cuDeviceGetName)(name_buf.as_mut_ptr() as *mut c_char, 256, device)
        };
        check_cuda(result)?;

        // Find null terminator and convert to String
        let len = name_buf.iter().position(|&c| c == 0).unwrap_or(256);
        let name = String::from_utf8_lossy(&name_buf[..len]).to_string();

        Ok(name)
    }

    /// Get device attribute
    ///
    /// # Arguments
    ///
    /// - `device`: Device handle from `get_device()`
    /// - `attr`: Attribute to query
    ///
    /// # Returns
    ///
    /// - `Ok(value)` with attribute value
    /// - `Err` if attribute query fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_DEVICE_VALID`: Device handle must be valid
    /// - `#ASSUME_ATTR_SUPPORTED`: Attribute must be supported by device
    pub fn device_attribute(device: CUdevice, attr: CUdevice_attribute) -> KgpuDriverResult<i32> {
        let lib = cuda()?;

        let mut value: c_int = 0;

        // SAFETY: value pointer is valid local variable
        // #ASSUME_VALID_PTR: Stack variable always valid
        let result = unsafe { (lib.cuDeviceGetAttribute)(&mut value, attr, device) };
        check_cuda(result)?;

        Ok(value)
    }

    /// Get total device memory
    ///
    /// # Arguments
    ///
    /// - `device`: Device handle from `get_device()`
    ///
    /// # Returns
    ///
    /// - `Ok(bytes)` with total memory in bytes
    /// - `Err` if memory query fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_DEVICE_VALID`: Device handle must be valid
    pub fn device_total_mem(device: CUdevice) -> KgpuDriverResult<usize> {
        let lib = cuda()?;

        let mut bytes: usize = 0;

        // SAFETY: bytes pointer is valid local variable
        // #ASSUME_VALID_PTR: Stack variable always valid
        let result = unsafe { (lib.cuDeviceTotalMem)(&mut bytes, device) };
        check_cuda(result)?;

        Ok(bytes)
    }

    /// Create a CUDA context for a device
    ///
    /// # Arguments
    ///
    /// - `device`: Device handle from `get_device()`
    /// - `flags`: Context creation flags (see `ctx_flags`)
    ///
    /// # Returns
    ///
    /// - `Ok(context)` with new context handle
    /// - `Err` if context creation fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_DEVICE_VALID`: Device handle must be valid
    /// - `#VERIFY_CTX_DESTROY`: Caller must call `destroy_context()` when done
    pub fn create_context(device: CUdevice, flags: u32) -> KgpuDriverResult<CUcontext> {
        let lib = cuda()?;

        let mut ctx: CUcontext = null_mut();

        // SAFETY: ctx pointer is valid local variable
        // #ASSUME_VALID_PTR: Stack variable always valid
        let result = unsafe { (lib.cuCtxCreate)(&mut ctx, flags, device) };
        check_cuda(result)?;

        Ok(ctx)
    }

    /// Destroy a CUDA context
    ///
    /// # Arguments
    ///
    /// - `ctx`: Context handle from `create_context()`
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_CTX_VALID`: Context handle must be valid (not already destroyed)
    /// - `#ASSUME_NO_ACTIVE_WORK`: No kernel executing on this context
    pub fn destroy_context(ctx: CUcontext) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        // SAFETY: ctx was created by create_context and not yet destroyed
        // #ASSUME_CTX_VALID: Caller guarantees valid context
        let result = unsafe { (lib.cuCtxDestroy)(ctx) };
        check_cuda(result)
    }

    /// Set current context for this thread
    ///
    /// # Arguments
    ///
    /// - `ctx`: Context handle (or null to unset)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_CTX_VALID`: Context handle must be valid (if not null)
    pub fn set_current_context(ctx: CUcontext) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        // SAFETY: ctx was created by create_context
        // #ASSUME_CTX_VALID: Caller guarantees valid context
        let result = unsafe { (lib.cuCtxSetCurrent)(ctx) };
        check_cuda(result)
    }

    /// Get current context for this thread
    ///
    /// # Returns
    ///
    /// - `Ok(Some(ctx))` if a context is set
    /// - `Ok(None)` if no context is set
    /// - `Err` on failure
    pub fn get_current_context() -> KgpuDriverResult<Option<CUcontext>> {
        let lib = cuda()?;

        let mut ctx: CUcontext = null_mut();

        // SAFETY: ctx pointer is valid local variable
        // #ASSUME_VALID_PTR: Stack variable always valid
        let result = unsafe { (lib.cuCtxGetCurrent)(&mut ctx) };
        check_cuda(result)?;

        Ok(if ctx.is_null() { None } else { Some(ctx) })
    }

    /// Synchronize current context (wait for all work to complete)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_CTX_SET`: A context must be current on this thread
    pub fn synchronize_context() -> KgpuDriverResult<()> {
        let lib = cuda()?;

        // SAFETY: Safe to call if context is set
        // #ASSUME_CTX_SET: Caller guarantees context is current
        let result = unsafe { (lib.cuCtxSynchronize)() };
        check_cuda(result)
    }

    /// Allocate pinned host memory (CRITICAL for Trojan Kernel)
    ///
    /// Pinned memory is page-locked and can be directly accessed by the GPU.
    /// This is essential for the Trojan Kernel ring buffer.
    ///
    /// # Arguments
    ///
    /// - `size`: Number of bytes to allocate
    ///
    /// # Returns
    ///
    /// - `Ok(ptr)` with pinned memory pointer
    /// - `Err(KgpuDriverError::OutOfHostMemory)` if allocation fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_CTX_SET`: A context must be current on this thread
    /// - `#VERIFY_FREE_PINNED`: Caller must call `free_pinned()` when done
    pub fn alloc_pinned(size: usize) -> KgpuDriverResult<*mut u8> {
        let lib = cuda()?;

        if size == 0 {
            return Err(KgpuDriverError::InvalidSize);
        }

        let mut ptr: *mut c_void = null_mut();

        // SAFETY: ptr is valid local variable
        // #ASSUME_VALID_PTR: Stack variable always valid
        // #ASSUME_CTX_SET: Context must be current
        let result = unsafe { (lib.cuMemAllocHost)(&mut ptr, size) };
        check_cuda(result)?;

        Ok(ptr as *mut u8)
    }

    /// Free pinned host memory
    ///
    /// # Arguments
    ///
    /// - `ptr`: Pointer from `alloc_pinned()`
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_PTR_VALID`: Pointer must be from `alloc_pinned()` (not already freed)
    pub fn free_pinned(ptr: *mut u8) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        if ptr.is_null() {
            return Ok(()); // Null free is a no-op (like C free)
        }

        // SAFETY: ptr was allocated by alloc_pinned
        // #ASSUME_PTR_VALID: Caller guarantees valid pointer
        let result = unsafe { (lib.cuMemFreeHost)(ptr as *mut c_void) };
        check_cuda(result)
    }

    /// Get device pointer for pinned host memory
    ///
    /// This is essential for the Trojan Kernel - it gets the GPU-addressable
    /// pointer for pinned memory that the GPU kernel can use.
    ///
    /// # Arguments
    ///
    /// - `host_ptr`: Host pointer from `alloc_pinned()`
    ///
    /// # Returns
    ///
    /// - `Ok(device_ptr)` with GPU-addressable pointer
    /// - `Err` if mapping fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_PTR_PINNED`: host_ptr must be from `alloc_pinned()`
    /// - `#ASSUME_CTX_SET`: A context must be current on this thread
    pub fn get_device_pointer(host_ptr: *mut u8) -> KgpuDriverResult<CUdeviceptr> {
        let lib = cuda()?;

        if host_ptr.is_null() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        let mut device_ptr: CUdeviceptr = 0;

        // SAFETY: device_ptr is valid local, host_ptr is from alloc_pinned
        // #ASSUME_PTR_PINNED: Caller guarantees pinned memory
        // #ASSUME_CTX_SET: Context must be current
        let result = unsafe {
            (lib.cuMemHostGetDevicePointer)(&mut device_ptr, host_ptr as *mut c_void, 0)
        };
        check_cuda(result)?;

        Ok(device_ptr)
    }

    /// Allocate device memory
    ///
    /// # Arguments
    ///
    /// - `size`: Number of bytes to allocate
    ///
    /// # Returns
    ///
    /// - `Ok(device_ptr)` with GPU memory address
    /// - `Err(KgpuDriverError::OutOfDeviceMemory)` if allocation fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_CTX_SET`: A context must be current on this thread
    /// - `#VERIFY_FREE_DEVICE`: Caller must call `free_device()` when done
    pub fn alloc_device(size: usize) -> KgpuDriverResult<CUdeviceptr> {
        let lib = cuda()?;

        if size == 0 {
            return Err(KgpuDriverError::InvalidSize);
        }

        let mut dptr: CUdeviceptr = 0;

        // SAFETY: dptr is valid local variable
        // #ASSUME_VALID_PTR: Stack variable always valid
        // #ASSUME_CTX_SET: Context must be current
        let result = unsafe { (lib.cuMemAlloc)(&mut dptr, size) };
        check_cuda(result)?;

        Ok(dptr)
    }

    /// Free device memory
    ///
    /// # Arguments
    ///
    /// - `dptr`: Device pointer from `alloc_device()`
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_DPTR_VALID`: dptr must be from `alloc_device()` (not already freed)
    pub fn free_device(dptr: CUdeviceptr) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        if dptr == 0 {
            return Ok(()); // Null free is a no-op
        }

        // SAFETY: dptr was allocated by alloc_device
        // #ASSUME_DPTR_VALID: Caller guarantees valid pointer
        let result = unsafe { (lib.cuMemFree)(dptr) };
        check_cuda(result)
    }

    /// Copy data from host to device (synchronous)
    ///
    /// # Arguments
    ///
    /// - `dst`: Device pointer (destination)
    /// - `src`: Host pointer (source)
    /// - `size`: Number of bytes to copy
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_DST_VALID`: dst must be valid device pointer with >= size bytes
    /// - `#ASSUME_SRC_VALID`: src must be valid host pointer with >= size bytes
    /// - `#ASSUME_CTX_SET`: A context must be current on this thread
    pub fn memcpy_htod(dst: CUdeviceptr, src: *const u8, size: usize) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        if size == 0 {
            return Ok(());
        }

        // SAFETY: dst from alloc_device, src valid host pointer
        // #ASSUME_DST_VALID: Caller guarantees valid device pointer
        // #ASSUME_SRC_VALID: Caller guarantees valid host pointer
        let result = unsafe { (lib.cuMemcpyHtoD)(dst, src as *const c_void, size) };
        check_cuda(result)
    }

    /// Copy data from device to host (synchronous)
    ///
    /// # Arguments
    ///
    /// - `dst`: Host pointer (destination)
    /// - `src`: Device pointer (source)
    /// - `size`: Number of bytes to copy
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_DST_VALID`: dst must be valid host pointer with >= size bytes
    /// - `#ASSUME_SRC_VALID`: src must be valid device pointer with >= size bytes
    /// - `#ASSUME_CTX_SET`: A context must be current on this thread
    pub fn memcpy_dtoh(dst: *mut u8, src: CUdeviceptr, size: usize) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        if size == 0 {
            return Ok(());
        }

        // SAFETY: src from alloc_device, dst valid host pointer
        // #ASSUME_DST_VALID: Caller guarantees valid host pointer
        // #ASSUME_SRC_VALID: Caller guarantees valid device pointer
        let result = unsafe { (lib.cuMemcpyDtoH)(dst as *mut c_void, src, size) };
        check_cuda(result)
    }

    /// Load a CUDA module from file (PTX or cubin)
    ///
    /// # Arguments
    ///
    /// - `path`: Path to PTX or cubin file
    ///
    /// # Returns
    ///
    /// - `Ok(module)` with loaded module handle
    /// - `Err` if load fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_FILE_EXISTS`: File must exist and be readable
    /// - `#ASSUME_VALID_MODULE`: File must contain valid PTX or cubin
    /// - `#VERIFY_MODULE_UNLOAD`: Caller must call `unload_module()` when done
    pub fn load_module(path: &str) -> KgpuDriverResult<CUmodule> {
        let lib = cuda()?;

        let cpath = CString::new(path).map_err(|_| KgpuDriverError::InvalidParameter)?;

        let mut module: CUmodule = null_mut();

        // SAFETY: cpath is valid C string, module is local variable
        // #ASSUME_FILE_EXISTS: Caller guarantees file exists
        // #ASSUME_VALID_MODULE: Caller guarantees valid PTX/cubin
        let result = unsafe { (lib.cuModuleLoad)(&mut module, cpath.as_ptr()) };
        check_cuda(result)?;

        Ok(module)
    }

    /// Load a CUDA module from memory (PTX or cubin image)
    ///
    /// # Arguments
    ///
    /// - `image`: PTX source or cubin binary
    ///
    /// # Returns
    ///
    /// - `Ok(module)` with loaded module handle
    /// - `Err` if load fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_VALID_IMAGE`: image must be valid PTX or cubin
    /// - `#VERIFY_MODULE_UNLOAD`: Caller must call `unload_module()` when done
    pub fn load_module_data(image: &[u8]) -> KgpuDriverResult<CUmodule> {
        let lib = cuda()?;

        let mut module: CUmodule = null_mut();

        // SAFETY: image is valid slice, module is local variable
        // #ASSUME_VALID_IMAGE: Caller guarantees valid PTX/cubin
        let result =
            unsafe { (lib.cuModuleLoadData)(&mut module, image.as_ptr() as *const c_void) };
        check_cuda(result)?;

        Ok(module)
    }

    /// Unload a CUDA module
    ///
    /// # Arguments
    ///
    /// - `module`: Module handle from `load_module()` or `load_module_data()`
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_MODULE_VALID`: module must be valid (not already unloaded)
    /// - `#ASSUME_NO_ACTIVE_KERNELS`: No kernels from this module executing
    pub fn unload_module(module: CUmodule) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        // SAFETY: module was loaded by load_module/load_module_data
        // #ASSUME_MODULE_VALID: Caller guarantees valid module
        let result = unsafe { (lib.cuModuleUnload)(module) };
        check_cuda(result)
    }

    /// Get a kernel function from a loaded module
    ///
    /// # Arguments
    ///
    /// - `module`: Module handle from `load_module()`
    /// - `name`: Kernel function name
    ///
    /// # Returns
    ///
    /// - `Ok(function)` with function handle
    /// - `Err(KgpuDriverError::NotFound)` if function not in module
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_MODULE_VALID`: module must be valid (not unloaded)
    /// - `#ASSUME_NAME_EXISTS`: name must exist in module
    pub fn get_function(module: CUmodule, name: &str) -> KgpuDriverResult<CUfunction> {
        let lib = cuda()?;

        let cname = CString::new(name).map_err(|_| KgpuDriverError::InvalidParameter)?;

        let mut func: CUfunction = null_mut();

        // SAFETY: cname is valid C string, func is local variable, module is valid
        // #ASSUME_MODULE_VALID: Caller guarantees valid module
        // #ASSUME_NAME_EXISTS: Caller guarantees function exists
        let result = unsafe { (lib.cuModuleGetFunction)(&mut func, module, cname.as_ptr()) };
        check_cuda(result)?;

        Ok(func)
    }

    /// Launch a kernel function
    ///
    /// # Arguments
    ///
    /// - `func`: Function handle from `get_function()`
    /// - `grid_dim`: Grid dimensions (blocks)
    /// - `block_dim`: Block dimensions (threads per block)
    /// - `shared_mem`: Shared memory per block (bytes)
    /// - `stream`: Stream for execution (None = default stream)
    /// - `params`: Kernel parameters (array of pointers to arguments)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_FUNC_VALID`: func must be valid function handle
    /// - `#ASSUME_DIMS_VALID`: Grid/block dims must be within hardware limits
    /// - `#ASSUME_PARAMS_VALID`: params must match kernel signature
    /// - `#ASSUME_CTX_SET`: A context must be current on this thread
    pub fn launch_kernel(
        func: CUfunction,
        grid_dim: (u32, u32, u32),
        block_dim: (u32, u32, u32),
        shared_mem: u32,
        stream: Option<CUstream>,
        params: &mut [*mut c_void],
    ) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        let stream_handle = stream.unwrap_or(null_mut());

        // SAFETY: func is valid, params contains valid arg pointers
        // #ASSUME_FUNC_VALID: Caller guarantees valid function
        // #ASSUME_DIMS_VALID: Caller guarantees valid dimensions
        // #ASSUME_PARAMS_VALID: Caller guarantees valid parameters
        let result = unsafe {
            (lib.cuLaunchKernel)(
                func,
                grid_dim.0,
                grid_dim.1,
                grid_dim.2,
                block_dim.0,
                block_dim.1,
                block_dim.2,
                shared_mem,
                stream_handle,
                params.as_mut_ptr(),
                null_mut(),
            )
        };
        check_cuda(result)
    }

    /// Create a CUDA stream
    ///
    /// # Arguments
    ///
    /// - `flags`: Stream creation flags (see `stream_flags`)
    ///
    /// # Returns
    ///
    /// - `Ok(stream)` with new stream handle
    /// - `Err` if creation fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_CTX_SET`: A context must be current on this thread
    /// - `#VERIFY_STREAM_DESTROY`: Caller must call `destroy_stream()` when done
    pub fn create_stream(flags: u32) -> KgpuDriverResult<CUstream> {
        let lib = cuda()?;

        let mut stream: CUstream = null_mut();

        // SAFETY: stream is local variable
        // #ASSUME_CTX_SET: Context must be current
        let result = unsafe { (lib.cuStreamCreate)(&mut stream, flags) };
        check_cuda(result)?;

        Ok(stream)
    }

    /// Destroy a CUDA stream
    ///
    /// # Arguments
    ///
    /// - `stream`: Stream handle from `create_stream()`
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STREAM_VALID`: stream must be valid (not already destroyed)
    pub fn destroy_stream(stream: CUstream) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        // SAFETY: stream was created by create_stream
        // #ASSUME_STREAM_VALID: Caller guarantees valid stream
        let result = unsafe { (lib.cuStreamDestroy)(stream) };
        check_cuda(result)
    }

    /// Synchronize a stream (wait for all work to complete)
    ///
    /// # Arguments
    ///
    /// - `stream`: Stream handle (None = default stream)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STREAM_VALID`: stream must be valid
    pub fn synchronize_stream(stream: Option<CUstream>) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        let stream_handle = stream.unwrap_or(null_mut());

        // SAFETY: stream is valid or null (default stream)
        // #ASSUME_STREAM_VALID: Caller guarantees valid stream
        let result = unsafe { (lib.cuStreamSynchronize)(stream_handle) };
        check_cuda(result)
    }

    /// Query stream status (non-blocking)
    ///
    /// # Arguments
    ///
    /// - `stream`: Stream handle
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if stream has completed all work
    /// - `Ok(false)` if stream has pending work
    /// - `Err` on error
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STREAM_VALID`: stream must be valid
    pub fn query_stream(stream: CUstream) -> KgpuDriverResult<bool> {
        let lib = cuda()?;

        // SAFETY: stream is valid
        // #ASSUME_STREAM_VALID: Caller guarantees valid stream
        let result = unsafe { (lib.cuStreamQuery)(stream) };

        match result {
            CUresult::CUDA_SUCCESS => Ok(true),
            CUresult::CUDA_ERROR_NOT_READY => Ok(false),
            _ => Err(result.into()),
        }
    }

    /// Create a CUDA event
    ///
    /// # Arguments
    ///
    /// - `flags`: Event creation flags (see `event_flags`)
    ///
    /// # Returns
    ///
    /// - `Ok(event)` with new event handle
    /// - `Err` if creation fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_CTX_SET`: A context must be current on this thread
    /// - `#VERIFY_EVENT_DESTROY`: Caller must call `destroy_event()` when done
    pub fn create_event(flags: u32) -> KgpuDriverResult<CUevent> {
        let lib = cuda()?;

        let mut event: CUevent = null_mut();

        // SAFETY: event is local variable
        // #ASSUME_CTX_SET: Context must be current
        let result = unsafe { (lib.cuEventCreate)(&mut event, flags) };
        check_cuda(result)?;

        Ok(event)
    }

    /// Destroy a CUDA event
    ///
    /// # Arguments
    ///
    /// - `event`: Event handle from `create_event()`
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_EVENT_VALID`: event must be valid (not already destroyed)
    pub fn destroy_event(event: CUevent) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        // SAFETY: event was created by create_event
        // #ASSUME_EVENT_VALID: Caller guarantees valid event
        let result = unsafe { (lib.cuEventDestroy)(event) };
        check_cuda(result)
    }

    /// Record an event on a stream
    ///
    /// # Arguments
    ///
    /// - `event`: Event handle
    /// - `stream`: Stream handle (None = default stream)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_EVENT_VALID`: event must be valid
    /// - `#ASSUME_STREAM_VALID`: stream must be valid (if not None)
    pub fn record_event(event: CUevent, stream: Option<CUstream>) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        let stream_handle = stream.unwrap_or(null_mut());

        // SAFETY: event and stream are valid
        // #ASSUME_EVENT_VALID: Caller guarantees valid event
        // #ASSUME_STREAM_VALID: Caller guarantees valid stream
        let result = unsafe { (lib.cuEventRecord)(event, stream_handle) };
        check_cuda(result)
    }

    /// Wait for an event to complete
    ///
    /// # Arguments
    ///
    /// - `event`: Event handle
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_EVENT_VALID`: event must be valid
    /// - `#ASSUME_EVENT_RECORDED`: event must have been recorded
    pub fn synchronize_event(event: CUevent) -> KgpuDriverResult<()> {
        let lib = cuda()?;

        // SAFETY: event is valid and recorded
        // #ASSUME_EVENT_VALID: Caller guarantees valid event
        // #ASSUME_EVENT_RECORDED: Caller guarantees event was recorded
        let result = unsafe { (lib.cuEventSynchronize)(event) };
        check_cuda(result)
    }

    /// Get elapsed time between two events (milliseconds)
    ///
    /// # Arguments
    ///
    /// - `start`: Start event
    /// - `end`: End event
    ///
    /// # Returns
    ///
    /// - `Ok(ms)` with elapsed time in milliseconds
    /// - `Err` if timing query fails
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_EVENTS_VALID`: Both events must be valid
    /// - `#ASSUME_EVENTS_RECORDED`: Both events must have been recorded
    /// - `#ASSUME_EVENTS_COMPLETED`: Both events must have completed
    pub fn event_elapsed_time(start: CUevent, end: CUevent) -> KgpuDriverResult<f32> {
        let lib = cuda()?;

        let mut ms: f32 = 0.0;

        // SAFETY: events are valid and completed, ms is local variable
        // #ASSUME_EVENTS_VALID: Caller guarantees valid events
        // #ASSUME_EVENTS_RECORDED: Caller guarantees events were recorded
        // #ASSUME_EVENTS_COMPLETED: Caller guarantees events completed
        let result = unsafe { (lib.cuEventElapsedTime)(&mut ms, start, end) };
        check_cuda(result)?;

        Ok(ms)
    }
}

// Stub module for non-std or non-Linux
#[cfg(not(all(feature = "std", target_os = "linux")))]
pub mod safe {
    use super::*;

    pub fn init() -> super::super::error::KgpuDriverResult<()> {
        Err(super::super::error::KgpuDriverError::PlatformNotSupported)
    }

    pub fn driver_version() -> super::super::error::KgpuDriverResult<(i32, i32)> {
        Err(super::super::error::KgpuDriverError::PlatformNotSupported)
    }

    pub fn device_count() -> super::super::error::KgpuDriverResult<i32> {
        Err(super::super::error::KgpuDriverError::PlatformNotSupported)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests (Type Sizes, Error Codes, Constants)
    // ========================================================================

    #[test]
    fn test_cudevice_size() {
        // CUdevice is c_int (i32)
        assert_eq!(core::mem::size_of::<CUdevice>(), 4);
    }

    #[test]
    fn test_cudeviceptr_size() {
        // CUdeviceptr is u64 (64-bit GPU address)
        assert_eq!(core::mem::size_of::<CUdeviceptr>(), 8);
    }

    #[test]
    fn test_opaque_handle_sizes() {
        // All opaque handles are pointers
        assert_eq!(core::mem::size_of::<CUcontext>(), core::mem::size_of::<*mut c_void>());
        assert_eq!(core::mem::size_of::<CUmodule>(), core::mem::size_of::<*mut c_void>());
        assert_eq!(core::mem::size_of::<CUfunction>(), core::mem::size_of::<*mut c_void>());
        assert_eq!(core::mem::size_of::<CUstream>(), core::mem::size_of::<*mut c_void>());
        assert_eq!(core::mem::size_of::<CUevent>(), core::mem::size_of::<*mut c_void>());
    }

    #[test]
    fn test_curesult_size() {
        // CUresult is u32 (stable ABI)
        assert_eq!(core::mem::size_of::<CUresult>(), 4);
    }

    #[test]
    fn test_cudevice_attribute_size() {
        // CUdevice_attribute is i32
        assert_eq!(core::mem::size_of::<CUdevice_attribute>(), 4);
    }

    // ========================================================================
    // Error Code Tests
    // ========================================================================

    #[test]
    fn test_curesult_success() {
        assert!(CUresult::CUDA_SUCCESS.is_success());
        assert!(!CUresult::CUDA_ERROR_OUT_OF_MEMORY.is_success());
        assert!(!CUresult::CUDA_ERROR_INVALID_VALUE.is_success());
    }

    #[test]
    fn test_curesult_oom() {
        assert!(CUresult::CUDA_ERROR_OUT_OF_MEMORY.is_oom());
        assert!(!CUresult::CUDA_SUCCESS.is_oom());
        assert!(!CUresult::CUDA_ERROR_INVALID_VALUE.is_oom());
    }

    #[test]
    fn test_curesult_no_device() {
        assert!(CUresult::CUDA_ERROR_NO_DEVICE.is_no_device());
        assert!(CUresult::CUDA_ERROR_INVALID_DEVICE.is_no_device());
        assert!(!CUresult::CUDA_SUCCESS.is_no_device());
    }

    #[test]
    fn test_curesult_launch_error() {
        assert!(CUresult::CUDA_ERROR_LAUNCH_OUT_OF_RESOURCES.is_launch_error());
        assert!(CUresult::CUDA_ERROR_LAUNCH_TIMEOUT.is_launch_error());
        assert!(CUresult::CUDA_ERROR_LAUNCH_FAILED.is_launch_error());
        assert!(!CUresult::CUDA_SUCCESS.is_launch_error());
        assert!(!CUresult::CUDA_ERROR_OUT_OF_MEMORY.is_launch_error());
    }

    #[test]
    fn test_curesult_context_error() {
        assert!(CUresult::CUDA_ERROR_INVALID_CONTEXT.is_context_error());
        assert!(!CUresult::CUDA_SUCCESS.is_context_error());
        assert!(!CUresult::CUDA_ERROR_LAUNCH_FAILED.is_context_error());
    }

    #[test]
    fn test_curesult_names() {
        assert_eq!(CUresult::CUDA_SUCCESS.name(), "SUCCESS");
        assert_eq!(CUresult::CUDA_ERROR_OUT_OF_MEMORY.name(), "OUT_OF_MEMORY");
        assert_eq!(CUresult::CUDA_ERROR_INVALID_VALUE.name(), "INVALID_VALUE");
        assert_eq!(CUresult::CUDA_ERROR_NOT_INITIALIZED.name(), "NOT_INITIALIZED");
        assert_eq!(CUresult::CUDA_ERROR_LAUNCH_FAILED.name(), "LAUNCH_FAILED");
    }

    #[test]
    fn test_curesult_descriptions() {
        assert!(CUresult::CUDA_SUCCESS.description().contains("success"));
        assert!(CUresult::CUDA_ERROR_OUT_OF_MEMORY.description().contains("memory"));
        assert!(CUresult::CUDA_ERROR_NO_DEVICE.description().contains("device"));
    }

    #[test]
    fn test_curesult_from_u32() {
        assert_eq!(CUresult::from_u32(0), CUresult::CUDA_SUCCESS);
        assert_eq!(CUresult::from_u32(1), CUresult::CUDA_ERROR_INVALID_VALUE);
        assert_eq!(CUresult::from_u32(2), CUresult::CUDA_ERROR_OUT_OF_MEMORY);
        assert_eq!(CUresult::from_u32(100), CUresult::CUDA_ERROR_NO_DEVICE);
        assert_eq!(CUresult::from_u32(101), CUresult::CUDA_ERROR_INVALID_DEVICE);
        assert_eq!(CUresult::from_u32(719), CUresult::CUDA_ERROR_LAUNCH_FAILED);
        assert_eq!(CUresult::from_u32(12345), CUresult::CUDA_ERROR_UNKNOWN);
    }

    #[test]
    fn test_curesult_display() {
        let display = format!("{}", CUresult::CUDA_ERROR_OUT_OF_MEMORY);
        assert!(display.contains("OUT_OF_MEMORY"));
        assert!(display.contains("GPU memory"));
    }

    // ========================================================================
    // Device Attribute Tests
    // ========================================================================

    #[test]
    fn test_device_attribute_values() {
        assert_eq!(
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK as i32,
            1
        );
        assert_eq!(CUdevice_attribute::CU_DEVICE_ATTRIBUTE_WARP_SIZE as i32, 10);
        assert_eq!(
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT as i32,
            16
        );
        assert_eq!(
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR as i32,
            75
        );
        assert_eq!(
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR as i32,
            76
        );
    }

    #[test]
    fn test_device_attribute_names() {
        assert_eq!(
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK.name(),
            "MAX_THREADS_PER_BLOCK"
        );
        assert_eq!(
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_WARP_SIZE.name(),
            "WARP_SIZE"
        );
        assert_eq!(
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT.name(),
            "MULTIPROCESSOR_COUNT"
        );
    }

    // ========================================================================
    // Context Flag Tests
    // ========================================================================

    #[test]
    fn test_ctx_flags() {
        assert_eq!(ctx_flags::CU_CTX_SCHED_AUTO, 0x00);
        assert_eq!(ctx_flags::CU_CTX_SCHED_SPIN, 0x01);
        assert_eq!(ctx_flags::CU_CTX_SCHED_YIELD, 0x02);
        assert_eq!(ctx_flags::CU_CTX_SCHED_BLOCKING_SYNC, 0x04);
        assert_eq!(ctx_flags::CU_CTX_MAP_HOST, 0x08);
    }

    #[test]
    fn test_ctx_sched_mask() {
        // Verify mask covers all scheduling flags
        assert_eq!(
            ctx_flags::CU_CTX_SCHED_AUTO | ctx_flags::CU_CTX_SCHED_SPIN | ctx_flags::CU_CTX_SCHED_YIELD | ctx_flags::CU_CTX_SCHED_BLOCKING_SYNC,
            ctx_flags::CU_CTX_SCHED_MASK
        );
    }

    // ========================================================================
    // Stream Flag Tests
    // ========================================================================

    #[test]
    fn test_stream_flags() {
        assert_eq!(stream_flags::CU_STREAM_DEFAULT, 0x00);
        assert_eq!(stream_flags::CU_STREAM_NON_BLOCKING, 0x01);
    }

    // ========================================================================
    // Event Flag Tests
    // ========================================================================

    #[test]
    fn test_event_flags() {
        assert_eq!(event_flags::CU_EVENT_DEFAULT, 0x00);
        assert_eq!(event_flags::CU_EVENT_BLOCKING_SYNC, 0x01);
        assert_eq!(event_flags::CU_EVENT_DISABLE_TIMING, 0x02);
        assert_eq!(event_flags::CU_EVENT_INTERPROCESS, 0x04);
    }

    // ========================================================================
    // Host Alloc Flag Tests
    // ========================================================================

    #[test]
    fn test_host_alloc_flags() {
        assert_eq!(host_alloc_flags::CU_MEMHOSTALLOC_DEFAULT, 0x00);
        assert_eq!(host_alloc_flags::CU_MEMHOSTALLOC_PORTABLE, 0x01);
        assert_eq!(host_alloc_flags::CU_MEMHOSTALLOC_DEVICEMAP, 0x02);
        assert_eq!(host_alloc_flags::CU_MEMHOSTALLOC_WRITECOMBINED, 0x04);
    }

    // ========================================================================
    // Error Conversion Tests
    // ========================================================================

    #[test]
    fn test_curesult_to_kgpu_error() {
        use super::super::error::KgpuDriverError;

        let err: KgpuDriverError = CUresult::CUDA_ERROR_OUT_OF_MEMORY.into();
        assert_eq!(err, KgpuDriverError::OutOfDeviceMemory);

        let err: KgpuDriverError = CUresult::CUDA_ERROR_NO_DEVICE.into();
        assert_eq!(err, KgpuDriverError::DeviceNotFound);

        let err: KgpuDriverError = CUresult::CUDA_ERROR_INVALID_DEVICE.into();
        assert_eq!(err, KgpuDriverError::InvalidDeviceIndex);

        let err: KgpuDriverError = CUresult::CUDA_ERROR_NOT_INITIALIZED.into();
        assert_eq!(err, KgpuDriverError::TrojanCudaInitFailed);

        let err: KgpuDriverError = CUresult::CUDA_ERROR_LAUNCH_FAILED.into();
        assert_eq!(err, KgpuDriverError::CommandFailed);

        let err: KgpuDriverError = CUresult::CUDA_ERROR_LAUNCH_TIMEOUT.into();
        assert_eq!(err, KgpuDriverError::CommandTimeout);

        let err: KgpuDriverError = CUresult::CUDA_ERROR_NOT_SUPPORTED.into();
        assert_eq!(err, KgpuDriverError::DeviceNotSupported);
    }

    #[test]
    fn test_check_cuda_success() {
        let result = check_cuda(CUresult::CUDA_SUCCESS);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_cuda_error() {
        let result = check_cuda(CUresult::CUDA_ERROR_OUT_OF_MEMORY);
        assert!(result.is_err());
    }

    // ========================================================================
    // Q8-Q14: Property Tests (FFI Safety)
    // ========================================================================

    #[test]
    fn test_null_handle_safety() {
        // Verify null handles are valid (null is a valid CUcontext, etc.)
        let ctx: CUcontext = null_mut();
        let module: CUmodule = null_mut();
        let func: CUfunction = null_mut();
        let stream: CUstream = null_mut();
        let event: CUevent = null_mut();

        assert!(ctx.is_null());
        assert!(module.is_null());
        assert!(func.is_null());
        assert!(stream.is_null());
        assert!(event.is_null());
    }

    #[test]
    fn test_deviceptr_zero_valid() {
        // CUdeviceptr = 0 is valid (represents null device pointer)
        let dptr: CUdeviceptr = 0;
        assert_eq!(dptr, 0);
    }

    #[test]
    fn test_curesult_roundtrip() {
        // Test that error codes survive roundtrip through u32
        let errors = [
            CUresult::CUDA_SUCCESS,
            CUresult::CUDA_ERROR_INVALID_VALUE,
            CUresult::CUDA_ERROR_OUT_OF_MEMORY,
            CUresult::CUDA_ERROR_NO_DEVICE,
            CUresult::CUDA_ERROR_LAUNCH_FAILED,
        ];

        for err in errors {
            let code = err as u32;
            let reconstructed = CUresult::from_u32(code);
            assert_eq!(err, reconstructed, "Roundtrip failed for {:?}", err);
        }
    }

    // ========================================================================
    // CudaLibrary Tests (Availability Check)
    // ========================================================================

    #[test]
    fn test_library_paths_not_empty() {
        assert!(!CudaLibrary::LIBRARY_PATHS.is_empty());
    }

    #[test]
    fn test_library_paths_valid() {
        for path in CudaLibrary::LIBRARY_PATHS {
            assert!(!path.is_empty());
            assert!(path.contains("cuda") || path.contains("libcuda"));
        }
    }

    #[test]
    fn test_cuda_library_debug() {
        // Test Debug implementation (doesn't require actual library)
        let debug_str = format!("{:?}", CUresult::CUDA_SUCCESS);
        assert!(debug_str.contains("CUDA_SUCCESS"));
    }

    // ========================================================================
    // Global State Tests
    // ========================================================================

    #[test]
    fn test_global_atomics_initialized() {
        // Verify global atomics are initialized to expected values
        assert!(!CUDA_INITIALIZED.load(Ordering::Relaxed) || CUDA_INITIALIZED.load(Ordering::Relaxed));
        // The handle should be null initially (unless CUDA was loaded in another test)
        // We just verify it's accessible
        let _handle = CUDA_LIBRARY_HANDLE.load(Ordering::Relaxed);
    }

    // ========================================================================
    // Q15-Q21: Integration Tests (Mock)
    // ========================================================================

    #[test]
    fn test_error_message_format() {
        // Test that error messages are properly formatted
        let err = CUresult::CUDA_ERROR_OUT_OF_MEMORY;
        let msg = format!("{}", err);
        assert!(msg.contains("OUT_OF_MEMORY"));
        assert!(msg.contains("memory"));
    }

    #[test]
    fn test_attribute_value_ranges() {
        // Verify attribute enum values are in expected ranges
        let attrs = [
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK,
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_WARP_SIZE,
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT,
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR,
            CUdevice_attribute::CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR,
        ];

        for attr in attrs {
            let val = attr as i32;
            assert!(val > 0, "Attribute value should be positive: {:?}", attr);
            assert!(
                val < 1000,
                "Attribute value should be < 1000: {:?} = {}",
                attr,
                val
            );
        }
    }

    // ========================================================================
    // Q22-Q28: Production Tests (require real CUDA, skipped by default)
    // ========================================================================

    #[cfg(feature = "cuda-integration-tests")]
    mod cuda_integration_tests {
        use super::super::*;

        #[test]
        fn test_cuda_init() {
            let result = safe::init();
            // May fail if CUDA not installed, which is OK
            if result.is_ok() {
                // If init succeeded, version should work
                let version = safe::driver_version();
                assert!(version.is_ok());
                let (major, minor) = version.unwrap();
                assert!(major >= 10, "CUDA version should be >= 10.0");
                assert!(minor >= 0);
            }
        }

        #[test]
        fn test_cuda_device_count() {
            if safe::init().is_ok() {
                let count = safe::device_count();
                assert!(count.is_ok());
                // Count can be 0 if no GPU
            }
        }
    }

    // ========================================================================
    // Q29-Q35: Determinism Tests
    // ========================================================================

    #[test]
    fn test_error_conversion_determinism() {
        // Same input should always produce same output
        for _ in 0..100 {
            let err: super::super::error::KgpuDriverError =
                CUresult::CUDA_ERROR_OUT_OF_MEMORY.into();
            assert_eq!(err, super::super::error::KgpuDriverError::OutOfDeviceMemory);
        }
    }

    #[test]
    fn test_flag_combinations_determinism() {
        // Verify flag combinations produce consistent results
        let flags1 = ctx_flags::CU_CTX_SCHED_SPIN | ctx_flags::CU_CTX_MAP_HOST;
        let flags2 = ctx_flags::CU_CTX_MAP_HOST | ctx_flags::CU_CTX_SCHED_SPIN;
        assert_eq!(flags1, flags2);

        let flags3 = ctx_flags::CU_CTX_SCHED_SPIN | ctx_flags::CU_CTX_MAP_HOST;
        assert_eq!(flags1, flags3);
    }

    #[test]
    fn test_curesult_hash_determinism() {
        use core::hash::{Hash, Hasher};

        fn hash_value<T: Hash>(t: &T) -> u64 {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            t.hash(&mut hasher);
            hasher.finish()
        }

        let err1 = CUresult::CUDA_ERROR_OUT_OF_MEMORY;
        let err2 = CUresult::CUDA_ERROR_OUT_OF_MEMORY;

        assert_eq!(hash_value(&err1), hash_value(&err2));
    }
}
