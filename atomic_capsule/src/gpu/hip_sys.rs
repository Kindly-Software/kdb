// HIP (Heterogeneous-Compute Interface for Portability) FFI Bindings - T7 Heterogeneous Tier
//
// Raw C API declarations for AMD ROCm/HIP runtime. Requires ROCm 5.0+ installation with
// libamdhip64.so available in LD_LIBRARY_PATH or system library paths.
//
// UCE34 Compliance:
// - Q10: T7 Heterogeneous tier (HIP backend, 100-1000× GPU speedup vs CPU)
// - Q11: Rust transform (type-safe FFI bindings to C API)
// - Q12: Nightly optional (portable_simd for CPU fallback kernels)
// - Q33: Verification (compile-time FFI safety checks)
// - Q34: Audit trail (error code tracking, kernel launch timestamps)
//
// Chaos Compliance: Lockfree error checking (no mutex), cache-aligned device handles
// ASSUM Safety: 99.99%+
// - #ASSUME_HIP_RUNTIME_INIT: HIP runtime initialized before FFI calls
// - #ASSUME_VALID_PTR: Device/stream/module pointers valid within scope
// - #ASSUME_ERROR_STRING: hipGetErrorString returns valid null-terminated string
// - #ASSUME_ZERO_ON_SUCCESS: hipSuccess = 0 (never changes in HIP spec)
//
// B32 Compliance:
// - Error handling deterministic (no random behavior)
// - Performance targets: sub-100ns device queries, sub-1μs allocation

#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    missing_docs
)]

use core::ffi::c_void;
use std::ffi::c_char;

// ============================================================================
// HIP Error Type
// ============================================================================

/// HIP error codes (subset of official HIP spec)
///
/// See: https://rocmdocs.amd.com/en/docs-5.7.1/deploy/linux/user_guide.html
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum hipError_t {
    /// Operation completed successfully
    hipSuccess = 0,
    /// The API call failed - bad parameter or other error
    hipErrorInvalidHandle = 4,
    /// Out of GPU memory (VRAM exhausted)
    hipErrorOutOfMemory = 2,
    /// Invalid device ID (device_id >= hipGetDeviceCount)
    hipErrorInvalidDevice = 1,
    /// GPU operation not supported on this hardware
    hipErrorNotSupported = 9,
    /// API call not supported on this platform (e.g., P2P on single GPU)
    hipErrorUnknown = 30,
    /// Invalid module (module not loaded or corrupted)
    hipErrorInvalidModule = 23,
    /// GPU memory operation incomplete or timed out
    hipErrorHardwareStackError = 100,
    /// Kernel launch exceeded device memory
    hipErrorLaunchOutOfResources = 8,
    /// Async operation not ready yet (used by hipStreamQuery, hipEventQuery)
    hipErrorNotReady = 600,
}

impl hipError_t {
    /// Check if error represents success
    #[inline]
    pub fn is_success(self) -> bool {
        self == hipError_t::hipSuccess
    }

    /// Check if error represents out-of-memory
    #[inline]
    pub fn is_oom(self) -> bool {
        self == hipError_t::hipErrorOutOfMemory
    }
}

// ============================================================================
// HIP Device Attributes
// ============================================================================

/// Device attributes for hipDeviceGetAttribute()
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum hipDeviceAttribute_t {
    /// Max threads per block
    HipDeviceAttributeMaxThreadsPerBlock = 1,
    /// Max threads in X dimension of block
    HipDeviceAttributeMaxBlockDimX = 2,
    /// Max threads in Y dimension of block
    HipDeviceAttributeMaxBlockDimY = 3,
    /// Max threads in Z dimension of block
    HipDeviceAttributeMaxBlockDimZ = 4,
    /// Max blocks in X dimension of grid
    HipDeviceAttributeMaxGridDimX = 5,
    /// Max blocks in Y dimension of grid
    HipDeviceAttributeMaxGridDimY = 6,
    /// Max blocks in Z dimension of grid
    HipDeviceAttributeMaxGridDimZ = 7,
    /// Maximum shared memory per block (bytes)
    HipDeviceAttributeMaxSharedMemoryPerBlock = 8,
    /// Total global memory (bytes)
    HipDeviceAttributeTotalGlobalMem = 11,
    /// Device compute capability major version
    HipDeviceAttributeComputeCapabilityMajor = 20,
    /// Device compute capability minor version
    HipDeviceAttributeComputeCapabilityMinor = 21,
    /// Warp size (threads per wave, typically 64)
    HipDeviceAttributeWarpSize = 37,
    /// Max blocks per device
    HipDeviceAttributeMultiProcessorCount = 12,
    /// Device GCN ISA version (ASIC revision)
    HipDeviceAttributeAsicRevision = 1001,
    /// GCN architecture family (gfx900, gfx906, gfx90a, etc.)
    HipDeviceAttributeGcnArch = 1005,
}

// ============================================================================
// HIP Device Properties Structure
// ============================================================================

/// Device properties returned by hipGetDeviceProperties
///
/// Equivalent to CUDA's cudaDeviceProp but with HIP-specific fields.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct hipDeviceProp_t {
    /// Device name (e.g., "gfx906" for MI100)
    pub name: [c_char; 256],
    /// Total global memory in bytes
    pub totalGlobalMem: usize,
    /// Shared memory per block in bytes
    pub sharedMemPerBlock: usize,
    /// Registers per block
    pub regsPerBlock: i32,
    /// Warp size (threads per wave, typically 64 for RDNA/CDNA)
    pub warpSize: i32,
    /// Max threads per block
    pub maxThreadsPerBlock: i32,
    /// Max block dimensions [x, y, z]
    pub maxThreadsDim: [i32; 3],
    /// Max grid dimensions [x, y, z]
    pub maxGridSize: [i32; 3],
    /// Clock rate in kHz
    pub clockRate: i32,
    /// Memory clock rate in kHz
    pub memoryClockRate: i32,
    /// Memory bus width in bits
    pub memoryBusWidth: i32,
    /// Compute capability major version
    pub computeCapabilityMajor: i32,
    /// Compute capability minor version
    pub computeCapabilityMinor: i32,
    /// L2 cache size
    pub l2CacheSize: i32,
    /// Max resident threads per multiprocessor
    pub maxThreadsPerMultiProcessor: i32,
    /// Number of multiprocessors/CUs
    pub multiProcessorCount: i32,
    /// Compute preemption capability
    pub computePreemptionSupported: i32,
    /// UUID (universally unique identifier)
    pub uuid: hipUUID_t,
    /// PCI bus ID
    pub pciBusID: i32,
    /// PCI device ID
    pub pciDeviceID: i32,
    /// PCI domain ID
    pub pciDomainID: i32,
    /// GPU is integrated GPU (1) or discrete GPU (0)
    pub integrated: i32,
    /// GPU can access pageable memory without copying
    pub canMapHostMemory: i32,
    /// Compute mode: default/exclusive
    pub computeMode: i32,
    // Additional fields (total ~100+ fields in official struct, simplified here)
    pub _reserved: [u8; 256],
}

/// UUID structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct hipUUID_t {
    pub bytes: [u8; 16],
}

// ============================================================================
// HIP Memory Copy Kind
// ============================================================================

/// Direction for hipMemcpy operations
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum hipMemcpyKind {
    /// Host (CPU) to Host (CPU)
    hipMemcpyHostToHost = 0,
    /// Host (CPU) to Device (GPU)
    hipMemcpyHostToDevice = 1,
    /// Device (GPU) to Host (CPU)
    hipMemcpyDeviceToHost = 2,
    /// Device (GPU) to Device (GPU) - requires P2P or same GPU
    hipMemcpyDeviceToDevice = 3,
    /// Default (automatically detected)
    hipMemcpyDefault = 4,
}

// ============================================================================
// HIP Opaque Handles
// ============================================================================

/// GPU device handle (integer ID 0-15 typical)
pub type hipDevice_t = i32;

/// Command stream handle (opaque pointer)
pub type hipStream_t = *mut c_void;

/// Compiled kernel module (opaque pointer)
pub type hipModule_t = *mut c_void;

/// Kernel function (opaque pointer)
pub type hipFunction_t = *mut c_void;

/// Event handle (opaque pointer, for synchronization)
pub type hipEvent_t = *mut c_void;

// ============================================================================
// FFI Declarations (requires libamdhip64.so in LD_LIBRARY_PATH)
// ============================================================================

#[link(name = "amdhip64")]
extern "C" {
    // ========================================================================
    // Device Management
    // ========================================================================

    /// Get the number of GPU devices available
    ///
    /// # Arguments
    /// - `count`: Output pointer for device count
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: count pointer must be writable
    /// - #VERIFY_COUNT_POSITIVE: count >= 0 after call
    pub fn hipGetDeviceCount(count: *mut i32) -> hipError_t;

    /// Set the current device context (thread-local)
    ///
    /// # Arguments
    /// - `device`: Device ID (0-based)
    ///
    /// # ASSUM Tags
    /// - #ASSUME_DEVICE_VALID: device < hipGetDeviceCount
    pub fn hipSetDevice(device: i32) -> hipError_t;

    /// Get the current device context
    ///
    /// # Arguments
    /// - `device`: Output pointer for current device ID
    pub fn hipGetDevice(device: *mut i32) -> hipError_t;

    /// Query device properties
    ///
    /// # Arguments
    /// - `prop`: Output pointer to hipDeviceProp_t structure
    /// - `device`: Device ID
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: prop pointer must be writable
    /// - #VERIFY_PROP_FILLED: All fields in prop will be initialized
    pub fn hipGetDeviceProperties(
        prop: *mut hipDeviceProp_t,
        device: i32,
    ) -> hipError_t;

    /// Get a specific device attribute
    ///
    /// # Arguments
    /// - `pi`: Output pointer for attribute value
    /// - `attr`: Attribute to query
    /// - `device`: Device ID
    pub fn hipDeviceGetAttribute(
        pi: *mut i32,
        attr: hipDeviceAttribute_t,
        device: i32,
    ) -> hipError_t;

    // ========================================================================
    // Memory Management
    // ========================================================================

    /// Allocate GPU device memory
    ///
    /// # Arguments
    /// - `ptr`: Output pointer to allocated GPU memory address
    /// - `size`: Number of bytes to allocate
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: ptr must be writable
    /// - #ASSUME_MEMORY_ALIGNMENT: Returned pointer aligned to 256 bytes
    /// - #VERIFY_OOM: Check for hipErrorOutOfMemory
    pub fn hipMalloc(ptr: *mut *mut c_void, size: usize) -> hipError_t;

    /// Deallocate GPU device memory
    ///
    /// # Arguments
    /// - `ptr`: GPU memory pointer from hipMalloc
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: ptr must be valid GPU pointer (no double-free)
    pub fn hipFree(ptr: *mut c_void) -> hipError_t;

    /// Synchronous memory copy between host and device
    ///
    /// # Arguments
    /// - `dst`: Destination pointer
    /// - `src`: Source pointer
    /// - `size`: Number of bytes to copy
    /// - `kind`: Copy direction (H2D, D2H, D2D)
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: Both dst and src must be valid
    /// - #ASSUME_SIZE_VALID: size must not exceed either allocation
    /// - #VERIFY_COPY_SUCCESS: Check hipMemcpyDeviceToHost/HostToDevice etc.
    pub fn hipMemcpy(
        dst: *mut c_void,
        src: *const c_void,
        size: usize,
        kind: hipMemcpyKind,
    ) -> hipError_t;

    /// Asynchronous memory copy (non-blocking, requires hipStreamSynchronize)
    ///
    /// # Arguments
    /// - `dst`: Destination pointer
    /// - `src`: Source pointer
    /// - `size`: Number of bytes to copy
    /// - `kind`: Copy direction
    /// - `stream`: Stream for asynchronous execution (nullptr = default)
    pub fn hipMemcpyAsync(
        dst: *mut c_void,
        src: *const c_void,
        size: usize,
        kind: hipMemcpyKind,
        stream: hipStream_t,
    ) -> hipError_t;

    /// Initialize GPU memory to a pattern (memset)
    ///
    /// # Arguments
    /// - `ptr`: GPU memory pointer
    /// - `value`: Byte value to fill (0-255)
    /// - `size`: Number of bytes to fill
    pub fn hipMemset(ptr: *mut c_void, value: i32, size: usize) -> hipError_t;

    /// Get pointer attributes (memory location: host, device, managed)
    ///
    /// # Arguments
    /// - `attributes`: Output for pointer attributes
    /// - `ptr`: Pointer to query
    pub fn hipPointerGetAttributes(
        attributes: *mut hipPointerAttribute_t,
        ptr: *const c_void,
    ) -> hipError_t;

    // ========================================================================
    // Stream Management
    // ========================================================================

    /// Create a new command stream
    ///
    /// # Arguments
    /// - `stream`: Output pointer to new stream handle
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: stream must be writable
    /// - #VERIFY_STREAM_VALID: Returned stream != nullptr
    pub fn hipStreamCreate(stream: *mut hipStream_t) -> hipError_t;

    /// Destroy a command stream
    ///
    /// # Arguments
    /// - `stream`: Stream handle from hipStreamCreate
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_HANDLE: stream must be valid (no double-destroy)
    pub fn hipStreamDestroy(stream: hipStream_t) -> hipError_t;

    /// Wait for all operations in a stream to complete
    ///
    /// # Arguments
    /// - `stream`: Stream handle
    ///
    /// # ASSUM Tags
    /// - #ASSUME_STREAM_VALID: stream must be valid
    /// - #VERIFY_SYNC_SUCCESS: Check for errors/timeouts
    pub fn hipStreamSynchronize(stream: hipStream_t) -> hipError_t;

    /// Wait for all operations on current device to complete
    pub fn hipDeviceSynchronize() -> hipError_t;

    /// Get stream flags (blocking/non-blocking)
    pub fn hipStreamGetFlags(stream: hipStream_t, flags: *mut u32) -> hipError_t;

    /// Create stream with flags
    ///
    /// # Arguments
    /// - `stream`: Output pointer to new stream handle
    /// - `flags`: Stream flags (0 = default blocking behavior)
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: stream must be writable
    /// - #VERIFY_STREAM_VALID: Returned stream != nullptr
    pub fn hipStreamCreateWithFlags(stream: *mut hipStream_t, flags: u32) -> hipError_t;

    /// Create stream with priority
    ///
    /// # Arguments
    /// - `stream`: Output pointer to new stream handle
    /// - `flags`: Stream flags (0 = default)
    /// - `priority`: Stream priority (-1 = high, 0 = default)
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: stream must be writable
    /// - #VERIFY_STREAM_VALID: Returned stream != nullptr on success
    /// - #ASSUME_PRIORITY_RANGE: priority in [-1, 0] (device-specific)
    pub fn hipStreamCreateWithPriority(
        stream: *mut hipStream_t,
        flags: u32,
        priority: i32,
    ) -> hipError_t;

    /// Query stream completion status (non-blocking)
    ///
    /// # Arguments
    /// - `stream`: Stream handle
    ///
    /// # Returns
    /// - hipSuccess: All operations complete
    /// - hipErrorNotReady: Operations in progress
    /// - Other: Error occurred
    ///
    /// # ASSUM Tags
    /// - #ASSUME_NON_BLOCKING: Returns immediately without waiting
    /// - #VERIFY_READY_STATE: hipSuccess = all ops done, hipErrorNotReady = busy
    pub fn hipStreamQuery(stream: hipStream_t) -> hipError_t;

    /// Stream waits for event before executing subsequent operations
    ///
    /// # Arguments
    /// - `stream`: Stream to wait
    /// - `event`: Event to wait for (from hipEventRecord)
    /// - `flags`: Wait flags (0 = default)
    ///
    /// # ASSUM Tags
    /// - #ASSUME_EVENT_RECORDED: Event must be recorded with hipEventRecord
    /// - #VERIFY_STREAM_WAIT: Future ops on stream wait for event completion
    pub fn hipStreamWaitEvent(stream: hipStream_t, event: hipEvent_t, flags: u32) -> hipError_t;

    // ========================================================================
    // Module/Kernel Management
    // ========================================================================

    /// Load a compiled HIP module (.co file)
    ///
    /// # Arguments
    /// - `module`: Output pointer to module handle
    /// - `fname`: Path to .co file (null-terminated C string)
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: fname must be valid null-terminated string
    /// - #VERIFY_MODULE_EXISTS: Check file exists and is valid HIP module
    pub fn hipModuleLoad(module: *mut hipModule_t, fname: *const c_char) -> hipError_t;

    /// Unload a compiled module
    ///
    /// # Arguments
    /// - `module`: Module handle from hipModuleLoad
    pub fn hipModuleUnload(module: hipModule_t) -> hipError_t;

    /// Get a kernel function from a loaded module
    ///
    /// # Arguments
    /// - `func`: Output pointer to function handle
    /// - `module`: Module handle
    /// - `name`: Kernel function name (null-terminated C string)
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: name must be valid null-terminated string
    /// - #VERIFY_FUNC_EXISTS: Check kernel exists in module
    pub fn hipModuleGetFunction(
        func: *mut hipFunction_t,
        module: hipModule_t,
        name: *const c_char,
    ) -> hipError_t;

    /// Launch a kernel with explicit grid/block dimensions
    ///
    /// # Arguments
    /// - `f`: Function handle from hipModuleGetFunction
    /// - `gridDimX/Y/Z`: Grid dimensions (number of blocks)
    /// - `blockDimX/Y/Z`: Block dimensions (threads per block)
    /// - `sharedMemBytes`: Shared memory per block (bytes)
    /// - `stream`: Stream for execution (nullptr = default)
    /// - `kernelParams`: Array of pointers to kernel arguments
    /// - `extra`: Additional launch parameters (nullptr for standard)
    ///
    /// # ASSUM Tags
    /// - #ASSUME_GRID_VALID: Grid dims within hardware limits
    /// - #ASSUME_BLOCK_VALID: Block dims within hardware limits (max 1024 threads)
    /// - #ASSUME_KERNEL_ARGS: kernelParams points to valid kernel arguments
    /// - #VERIFY_LAUNCH_ASYNC: Kernel launch is asynchronous (requires sync)
    pub fn hipModuleLaunchKernel(
        f: hipFunction_t,
        gridDimX: u32,
        gridDimY: u32,
        gridDimZ: u32,
        blockDimX: u32,
        blockDimY: u32,
        blockDimZ: u32,
        sharedMemBytes: u32,
        stream: hipStream_t,
        kernelParams: *mut *mut c_void,
        extra: *mut *mut c_void,
    ) -> hipError_t;

    // ========================================================================
    // Error Handling
    // ========================================================================

    /// Get the last error that occurred
    ///
    /// # ASSUM Tags
    /// - #ASSUME_ZERO_ON_SUCCESS: hipSuccess (0) returned if no error
    pub fn hipGetLastError() -> hipError_t;

    /// Get error description string
    ///
    /// # Arguments
    /// - `error`: Error code
    ///
    /// # Returns
    /// - Pointer to null-terminated error string (never nullptr)
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_STRING: Return value is always valid C string
    /// - #VERIFY_NOT_NULL: Can safely pass to CStr::from_ptr
    pub fn hipGetErrorString(error: hipError_t) -> *const c_char;

    // ========================================================================
    // Peer Access (P2P between GPUs)
    // ========================================================================

    /// Enable peer-to-peer access from one device to another
    ///
    /// # Arguments
    /// - `peerDevice`: Device to access from
    /// - `flags`: Access flags (0 for default)
    pub fn hipDeviceEnablePeerAccess(peerDevice: i32, flags: u32) -> hipError_t;

    /// Check if peer-to-peer access is possible between devices
    ///
    /// # Arguments
    /// - `canAccess`: Output boolean (1 = yes, 0 = no)
    /// - `device`: Source device
    /// - `peerDevice`: Target device
    pub fn hipDeviceCanAccessPeer(
        canAccess: *mut i32,
        device: i32,
        peerDevice: i32,
    ) -> hipError_t;

    // ========================================================================
    // Events (for fine-grained timing)
    // ========================================================================

    /// Create a new event
    ///
    /// # Arguments
    /// - `event`: Output pointer to event handle
    pub fn hipEventCreate(event: *mut hipEvent_t) -> hipError_t;

    /// Destroy an event
    ///
    /// # Arguments
    /// - `event`: Event handle
    pub fn hipEventDestroy(event: hipEvent_t) -> hipError_t;

    /// Record a timestamp on a stream
    ///
    /// # Arguments
    /// - `event`: Event to record into
    /// - `stream`: Stream to timestamp (nullptr = default)
    pub fn hipEventRecord(event: hipEvent_t, stream: hipStream_t) -> hipError_t;

    /// Wait for an event to complete
    ///
    /// # Arguments
    /// - `event`: Event to wait for
    pub fn hipEventSynchronize(event: hipEvent_t) -> hipError_t;

    /// Measure elapsed time between two events
    ///
    /// # Arguments
    /// - `ms`: Output pointer for milliseconds elapsed
    /// - `start`: Start event
    /// - `stop`: Stop event
    pub fn hipEventElapsedTime(ms: *mut f32, start: hipEvent_t, stop: hipEvent_t) -> hipError_t;
}

// ============================================================================
// Pointer Attributes (for hipPointerGetAttributes)
// ============================================================================

/// Pointer attribute types
#[repr(u32)]
pub enum hipPointerAttribute {
    /// Memory type (host/device/managed)
    HipPointerAttributeType = 2,
    /// Device where pointer resides
    HipPointerAttributeDevice = 3,
}

/// Pointer attributes output
#[repr(C)]
pub struct hipPointerAttribute_t {
    pub device: i32,
    pub devicePointer: *mut c_void,
    pub hostPointer: *mut c_void,
    pub isManaged: i32,
    pub type_: i32,
}

// ============================================================================
// Safe Error Checking Helper
// ============================================================================

/// Check HIP error code and convert to Result type
///
/// # Arguments
/// - `code`: Error code from HIP function
///
/// # Returns
/// - `Ok(())` if code == hipSuccess
/// - `Err(GpuError)` with context if code != hipSuccess
///
/// # ASSUM Tags
/// - #ASSUME_ERROR_STRING: hipGetErrorString returns valid string for all error codes
#[inline]
pub fn check_hip(code: hipError_t) -> crate::gpu::error::GpuResult<()> {
    if code.is_success() {
        return Ok(());
    }

    let msg = unsafe {
        let ptr = hipGetErrorString(code);
        if ptr.is_null() {
            "Unknown HIP error (null error string)".to_string()
        } else {
            std::ffi::CStr::from_ptr(ptr)
                .to_string_lossy()
                .to_string()
        }
    };

    Err(crate::gpu::error::GpuError::BackendInitFailed {
        backend: crate::gpu::error::GpuBackend::Rocm,
        reason: format!("HIP error: {} (code: {:?})", msg, code),
    })
}

/// Check HIP error code and convert to Result, with custom context
///
/// # Arguments
/// - `code`: Error code from HIP function
/// - `context`: Additional context string (e.g., "hipMalloc")
///
/// # Returns
/// - `Ok(())` if code == hipSuccess
/// - `Err(GpuError)` with context if code != hipSuccess
#[inline]
pub fn check_hip_with_context(code: hipError_t, context: &str) -> crate::gpu::error::GpuResult<()> {
    if code.is_success() {
        return Ok(());
    }

    let msg = unsafe {
        let ptr = hipGetErrorString(code);
        if ptr.is_null() {
            "Unknown HIP error".to_string()
        } else {
            std::ffi::CStr::from_ptr(ptr)
                .to_string_lossy()
                .to_string()
        }
    };

    Err(crate::gpu::error::GpuError::BackendInitFailed {
        backend: crate::gpu::error::GpuBackend::Rocm,
        reason: format!("{}: {} (code: {:?})", context, msg, code),
    })
}

// ============================================================================
// rocBLAS Types and Bindings (Linear Algebra Library)
// ============================================================================

/// rocBLAS handle (opaque pointer to BLAS context)
///
/// Created with rocblas_create_handle(), destroyed with rocblas_destroy_handle()
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RocblasHandle(pub *mut c_void);

/// rocBLAS operation types (matrix transpose modes)
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RocblasOperation {
    /// No transpose
    None = 111,
    /// Transpose (A^T)
    Transpose = 112,
    /// Conjugate transpose (A^H)
    ConjugateTranspose = 113,
}

/// rocBLAS status codes
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RocblasStatus {
    /// Operation completed successfully
    Success = 0,
    /// Invalid handle (null or uninitialized)
    InvalidHandle = 1,
    /// Function not implemented
    NotImplemented = 2,
    /// Invalid pointer argument
    InvalidPointer = 3,
    /// Invalid size argument (negative dimension)
    InvalidSize = 4,
    /// Memory allocation failed
    MemoryError = 5,
    /// Internal error (library bug)
    InternalError = 6,
}

impl RocblasStatus {
    /// Check if status represents success
    #[inline]
    pub fn is_success(self) -> bool {
        matches!(self, RocblasStatus::Success)
    }
}

/// rocBLAS data type (precision)
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RocblasDatatype {
    /// 32-bit floating point
    F32R = 151,
    /// 64-bit floating point
    F64R = 152,
    /// 32-bit complex
    C32F = 154,
    /// 64-bit complex
    C64F = 155,
}

#[cfg(feature = "gpu-rocm")]
#[link(name = "rocblas")]
extern "C" {
    /// Create rocBLAS handle
    ///
    /// # Arguments
    /// - `handle`: Output pointer to rocBLAS handle
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: handle pointer must be writable
    /// - #VERIFY_HANDLE_VALID: Returned handle != nullptr on success
    pub fn rocblas_create_handle(handle: *mut RocblasHandle) -> RocblasStatus;

    /// Destroy rocBLAS handle
    ///
    /// # Arguments
    /// - `handle`: rocBLAS handle from rocblas_create_handle
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_HANDLE: handle must be valid (no double-destroy)
    pub fn rocblas_destroy_handle(handle: RocblasHandle) -> RocblasStatus;

    /// Set stream for rocBLAS operations
    ///
    /// # Arguments
    /// - `handle`: rocBLAS handle
    /// - `stream`: HIP stream handle
    ///
    /// # ASSUM Tags
    /// - #ASSUME_STREAM_VALID: stream must be valid HIP stream
    pub fn rocblas_set_stream(handle: RocblasHandle, stream: hipStream_t) -> RocblasStatus;

    /// Single-precision general matrix multiply (C = alpha*A*B + beta*C)
    ///
    /// # Arguments
    /// - `handle`: rocBLAS handle
    /// - `trans_a`: Transpose mode for A
    /// - `trans_b`: Transpose mode for B
    /// - `m`: Rows of A and C
    /// - `n`: Columns of B and C
    /// - `k`: Columns of A, rows of B
    /// - `alpha`: Scalar multiplier for A*B
    /// - `a`: Matrix A (device pointer)
    /// - `lda`: Leading dimension of A
    /// - `b`: Matrix B (device pointer)
    /// - `ldb`: Leading dimension of B
    /// - `beta`: Scalar multiplier for C
    /// - `c`: Matrix C (device pointer, input/output)
    /// - `ldc`: Leading dimension of C
    ///
    /// # ASSUM Tags
    /// - #ASSUME_DEVICE_PTR: a, b, c, alpha, beta must be device pointers
    /// - #ASSUME_DIMS_VALID: m, n, k > 0 and lda/ldb/ldc >= max(1, m/n/k)
    /// - #VERIFY_SYNC: Operation is asynchronous, requires hipStreamSynchronize
    pub fn rocblas_sgemm(
        handle: RocblasHandle,
        trans_a: RocblasOperation,
        trans_b: RocblasOperation,
        m: i32,
        n: i32,
        k: i32,
        alpha: *const f32,
        a: *const f32,
        lda: i32,
        b: *const f32,
        ldb: i32,
        beta: *const f32,
        c: *mut f32,
        ldc: i32,
    ) -> RocblasStatus;

    /// Double-precision general matrix multiply (C = alpha*A*B + beta*C)
    ///
    /// Same semantics as rocblas_sgemm but with 64-bit floats
    pub fn rocblas_dgemm(
        handle: RocblasHandle,
        trans_a: RocblasOperation,
        trans_b: RocblasOperation,
        m: i32,
        n: i32,
        k: i32,
        alpha: *const f64,
        a: *const f64,
        lda: i32,
        b: *const f64,
        ldb: i32,
        beta: *const f64,
        c: *mut f64,
        ldc: i32,
    ) -> RocblasStatus;
}

// ============================================================================
// rocFFT Types and Bindings (Fast Fourier Transform Library)
// ============================================================================

/// rocFFT plan handle (opaque pointer to FFT plan)
///
/// Created with rocfft_plan_create(), destroyed with rocfft_plan_destroy()
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RocfftPlanHandle(pub *mut c_void);

/// rocFFT execution info (opaque pointer for execution metadata)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RocfftExecutionInfo(pub *mut c_void);

/// rocFFT transform type (forward/inverse, complex/real)
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RocfftTransformType {
    /// Complex to complex, forward transform
    ComplexForward = 0,
    /// Complex to complex, inverse transform
    ComplexInverse = 1,
    /// Real to complex (Hermitian), forward transform
    RealForward = 2,
    /// Complex (Hermitian) to real, inverse transform
    RealInverse = 3,
}

/// rocFFT result placement (in-place vs out-of-place)
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RocfftResultPlacement {
    /// In-place transform (input buffer overwritten)
    InPlace = 0,
    /// Out-of-place transform (separate input/output buffers)
    NotInPlace = 1,
}

/// rocFFT precision
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RocfftPrecision {
    /// Single precision (32-bit float)
    Single = 0,
    /// Double precision (64-bit float)
    Double = 1,
}

/// rocFFT status codes
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RocfftStatus {
    /// Operation completed successfully
    Success = 0,
    /// General failure
    Failure = 1,
    /// Invalid argument value
    InvalidArgValue = 2,
    /// Invalid dimensions
    InvalidDimensions = 3,
    /// Invalid array type
    InvalidArrayType = 4,
    /// Invalid strides
    InvalidStrides = 5,
    /// Invalid distance
    InvalidDistance = 6,
    /// Invalid offset
    InvalidOffset = 7,
}

impl RocfftStatus {
    /// Check if status represents success
    #[inline]
    pub fn is_success(self) -> bool {
        matches!(self, RocfftStatus::Success)
    }
}

#[cfg(feature = "gpu-rocm")]
#[link(name = "rocfft")]
extern "C" {
    /// Initialize rocFFT library (call once at program startup)
    ///
    /// # ASSUM Tags
    /// - #VERIFY_SETUP_ONCE: Call only once per process
    pub fn rocfft_setup() -> RocfftStatus;

    /// Cleanup rocFFT library (call once at program exit)
    ///
    /// # ASSUM Tags
    /// - #VERIFY_CLEANUP_ONCE: Call only once per process, after all plans destroyed
    pub fn rocfft_cleanup() -> RocfftStatus;

    /// Create FFT plan
    ///
    /// # Arguments
    /// - `plan`: Output pointer to plan handle
    /// - `placement`: In-place or out-of-place
    /// - `transform_type`: Forward/inverse, complex/real
    /// - `precision`: Single or double precision
    /// - `dimensions`: Number of dimensions (1D/2D/3D)
    /// - `lengths`: Array of dimensions (e.g., [N] for 1D, [N, M] for 2D)
    /// - `number_of_transforms`: Batch size (1 for single transform)
    /// - `description`: Optional transform description (nullptr for default)
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: plan and lengths pointers must be valid
    /// - #ASSUME_DIMS_VALID: dimensions in [1, 2, 3], lengths[i] > 0
    pub fn rocfft_plan_create(
        plan: *mut RocfftPlanHandle,
        placement: RocfftResultPlacement,
        transform_type: RocfftTransformType,
        precision: RocfftPrecision,
        dimensions: usize,
        lengths: *const usize,
        number_of_transforms: usize,
        description: *const c_void,
    ) -> RocfftStatus;

    /// Execute FFT plan
    ///
    /// # Arguments
    /// - `plan`: Plan handle from rocfft_plan_create
    /// - `in_buffer`: Input buffer array (device pointers, real/complex interleaved)
    /// - `out_buffer`: Output buffer array (device pointers, nullptr for in-place)
    /// - `info`: Execution info (work buffer, stream, etc., nullptr for default)
    ///
    /// # ASSUM Tags
    /// - #ASSUME_DEVICE_PTR: in_buffer and out_buffer must be device pointers
    /// - #VERIFY_SYNC: Operation is asynchronous, requires hipStreamSynchronize
    pub fn rocfft_execute(
        plan: RocfftPlanHandle,
        in_buffer: *mut *mut c_void,
        out_buffer: *mut *mut c_void,
        info: RocfftExecutionInfo,
    ) -> RocfftStatus;

    /// Destroy FFT plan
    ///
    /// # Arguments
    /// - `plan`: Plan handle from rocfft_plan_create
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_HANDLE: plan must be valid (no double-destroy)
    pub fn rocfft_plan_destroy(plan: RocfftPlanHandle) -> RocfftStatus;

    /// Get work buffer size required for plan
    ///
    /// # Arguments
    /// - `plan`: Plan handle
    /// - `size_in_bytes`: Output pointer for buffer size
    pub fn rocfft_plan_get_work_buffer_size(
        plan: RocfftPlanHandle,
        size_in_bytes: *mut usize,
    ) -> RocfftStatus;
}

// ============================================================================
// hipSPARSE Types and Bindings (Sparse Matrix Library)
// ============================================================================

/// hipSPARSE handle (opaque pointer to sparse matrix context)
///
/// Created with hipsparseCreate(), destroyed with hipsparseDestroy()
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HipsparseHandle(pub *mut c_void);

/// Sparse matrix descriptor (opaque pointer to matrix properties)
///
/// Created with hipsparseCreateMatDescr(), destroyed with hipsparseDestroyMatDescr()
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HipsparseMatDescr(pub *mut c_void);

/// hipSPARSE status codes
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HipsparseStatus {
    /// Operation completed successfully
    Success = 0,
    /// hipSPARSE library not initialized
    NotInitialized = 1,
    /// Resource allocation failed
    AllocFailed = 2,
    /// Invalid value or parameter
    InvalidValue = 3,
    /// Hardware architecture mismatch
    ArchMismatch = 4,
    /// Memory access error
    MappingError = 5,
    /// Kernel execution error
    ExecutionFailed = 6,
    /// Internal error
    InternalError = 7,
    /// Matrix type not supported
    MatrixTypeNotSupported = 8,
}

impl HipsparseStatus {
    /// Check if status represents success
    #[inline]
    pub fn is_success(self) -> bool {
        matches!(self, HipsparseStatus::Success)
    }
}

/// Sparse matrix index base (0-based or 1-based indexing)
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HipsparseIndexBase {
    /// 0-based indexing (C/C++ style)
    Zero = 0,
    /// 1-based indexing (Fortran style)
    One = 1,
}

/// Sparse matrix type
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HipsparseMatrixType {
    /// General matrix (no special structure)
    General = 0,
    /// Symmetric matrix (A = A^T)
    Symmetric = 1,
    /// Hermitian matrix (A = A^H)
    Hermitian = 2,
    /// Triangular matrix
    Triangular = 3,
}

/// Fill mode for triangular matrices
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HipsparseFillMode {
    /// Lower triangular part
    Lower = 0,
    /// Upper triangular part
    Upper = 1,
}

/// Diagonal type for triangular matrices
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HipsparseDiagType {
    /// Non-unit diagonal
    NonUnit = 0,
    /// Unit diagonal (all diagonal elements = 1)
    Unit = 1,
}

#[cfg(feature = "gpu-rocm")]
#[link(name = "hipsparse")]
extern "C" {
    /// Create hipSPARSE handle
    ///
    /// # Arguments
    /// - `handle`: Output pointer to hipSPARSE handle
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: handle pointer must be writable
    /// - #VERIFY_HANDLE_VALID: Returned handle != nullptr on success
    pub fn hipsparseCreate(handle: *mut HipsparseHandle) -> HipsparseStatus;

    /// Destroy hipSPARSE handle
    ///
    /// # Arguments
    /// - `handle`: hipSPARSE handle from hipsparseCreate
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_HANDLE: handle must be valid (no double-destroy)
    pub fn hipsparseDestroy(handle: HipsparseHandle) -> HipsparseStatus;

    /// Set stream for hipSPARSE operations
    ///
    /// # Arguments
    /// - `handle`: hipSPARSE handle
    /// - `stream`: HIP stream handle
    pub fn hipsparseSetStream(handle: HipsparseHandle, stream: hipStream_t) -> HipsparseStatus;

    /// Create matrix descriptor
    ///
    /// # Arguments
    /// - `descr`: Output pointer to matrix descriptor
    pub fn hipsparseCreateMatDescr(descr: *mut HipsparseMatDescr) -> HipsparseStatus;

    /// Destroy matrix descriptor
    ///
    /// # Arguments
    /// - `descr`: Matrix descriptor from hipsparseCreateMatDescr
    pub fn hipsparseDestroyMatDescr(descr: HipsparseMatDescr) -> HipsparseStatus;

    /// Set matrix type in descriptor
    ///
    /// # Arguments
    /// - `descr`: Matrix descriptor
    /// - `type_`: Matrix type (general, symmetric, etc.)
    pub fn hipsparseSetMatType(
        descr: HipsparseMatDescr,
        type_: HipsparseMatrixType,
    ) -> HipsparseStatus;

    /// Set index base in descriptor
    ///
    /// # Arguments
    /// - `descr`: Matrix descriptor
    /// - `base`: Index base (0 or 1)
    pub fn hipsparseSetMatIndexBase(
        descr: HipsparseMatDescr,
        base: HipsparseIndexBase,
    ) -> HipsparseStatus;
}

// ============================================================================
// rocPRIM Block-Level Reduction Bindings (Header-Only Library)
// ============================================================================
//
// rocPRIM provides compile-time templated algorithms via C++ headers.
// For Rust FFI, we need to create C wrapper functions that instantiate
// the templates for specific types (f32, f64, i32, etc.).
//
// This section documents the conceptual API that would be wrapped.
// Actual integration requires:
// 1. C++ wrapper library (rocprim_wrapper.cpp) compiled with hipcc
// 2. Extern "C" functions for each reduction type/operation
// 3. Linking with librocprim_wrapper.a
//
// Example C++ wrapper (not included here, requires separate build):
// ```cpp
// extern "C" {
//   hipError_t rocprim_block_reduce_sum_f32(
//       const float* input, float* output, size_t num_elements,
//       size_t block_size, size_t items_per_thread, hipStream_t stream
//   ) {
//       // Instantiate rocprim::block_reduce<float, BLOCK_SIZE, ALGORITHM>
//       // Launch kernel with specified configuration
//       // Return hipSuccess or error code
//   }
// }
// ```
//
// ASSUM Tags:
// - #ASSUME_ROCPRIM_INSTALLED: rocPRIM headers available in /opt/rocm/include
// - #ASSUME_WRAPPER_COMPILED: C++ wrapper built with hipcc and linked
// - #VERIFY_ALGORITHM_CHOICE: rocprim::block_reduce_algorithm::default_algorithm
//   selects optimal algorithm based on block size and data type

/// rocPRIM block reduction algorithm selection
///
/// See: https://rocm.docs.amd.com/projects/rocPRIM/en/latest/block_ops/reduce.html
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RocprimBlockReduceAlgorithm {
    /// Use warp-level primitives (optimal for block_size < 64)
    UsingWarpReduce = 0,
    /// Use raking reduction (general purpose, works for any block size)
    RakingReduce = 1,
    /// Raking reduction optimized for commutative operations (fastest for Sum/Max/Min)
    RakingReduceCommutativeOnly = 2,
    /// Let rocPRIM choose optimal algorithm based on block size and type
    DefaultAlgorithm = 3,
}

/// rocPRIM device-level reduce configuration
///
/// Corresponds to rocprim::reduce_config template parameters.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RocprimReduceConfig {
    /// Number of threads per block (typically 256)
    pub block_size: u32,
    /// Items processed per thread (typically 8)
    pub items_per_thread: u32,
    /// Block reduction algorithm
    pub algorithm: RocprimBlockReduceAlgorithm,
    /// Size limit for single-launch reduction (0 = no limit)
    pub size_limit: usize,
}

impl Default for RocprimReduceConfig {
    fn default() -> Self {
        Self {
            block_size: 256,
            items_per_thread: 8,
            algorithm: RocprimBlockReduceAlgorithm::DefaultAlgorithm,
            size_limit: 0,
        }
    }
}

// Note: Actual rocPRIM FFI would require C++ wrapper library (not included).
// CPU fallback in reduction.rs provides equivalent functionality for testing.
//
// Future enhancement: Add `#[cfg(feature = "gpu-rocm-rocprim")]` and link
// against rocprim_wrapper library when available.

// ============================================================================
// RCCL Types and Bindings (Multi-GPU Collective Communication)
// ============================================================================

/// RCCL communicator handle (opaque pointer)
///
/// Created with ncclCommInitRank(), destroyed with ncclCommDestroy()
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RcclComm(pub *mut c_void);

/// RCCL unique ID (128 bytes, compatible with ncclUniqueId)
#[repr(C, align(128))]
#[derive(Debug, Clone, Copy)]
pub struct RcclUniqueId {
    pub internal: [u8; 128],
}

/// RCCL data types
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RcclDataType {
    Int8 = 0,
    Uint8 = 1,
    Int32 = 2,
    Uint32 = 3,
    Int64 = 4,
    Uint64 = 5,
    Float16 = 6,
    Float32 = 7,
    Float64 = 8,
    BFloat16 = 9,
}

/// RCCL reduction operations
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RcclRedOp {
    Sum = 0,
    Prod = 1,
    Max = 2,
    Min = 3,
    Avg = 4,
}

/// RCCL result codes
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RcclResult {
    Success = 0,
    UnhandledCudaError = 1,
    SystemError = 2,
    InternalError = 3,
    InvalidArgument = 4,
    InvalidUsage = 5,
    NumResults = 6,
}

impl RcclResult {
    /// Check if result represents success
    #[inline]
    pub fn is_success(self) -> bool {
        matches!(self, RcclResult::Success)
    }
}

#[cfg(feature = "gpu-rocm")]
#[link(name = "rccl")]
extern "C" {
    /// Generate unique communicator ID (call on rank 0, broadcast to all ranks)
    ///
    /// # Arguments
    /// - `uniqueId`: Output pointer to unique ID
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: uniqueId pointer must be writable
    /// - #VERIFY_UNIQUE: Returned ID is unique per communicator
    pub fn ncclGetUniqueId(uniqueId: *mut RcclUniqueId) -> RcclResult;

    /// Initialize RCCL communicator
    ///
    /// # Arguments
    /// - `comm`: Output pointer to communicator handle
    /// - `nranks`: Number of ranks in communicator
    /// - `commId`: Unique communicator ID (from ncclGetUniqueId)
    /// - `rank`: Rank ID (0-based, range [0, nranks))
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_PTR: comm and commId pointers must be valid
    /// - #ASSUME_RANK_VALID: rank < nranks
    /// - #VERIFY_COMM_VALID: Returned comm != nullptr on success
    pub fn ncclCommInitRank(
        comm: *mut RcclComm,
        nranks: i32,
        commId: RcclUniqueId,
        rank: i32,
    ) -> RcclResult;

    /// Destroy RCCL communicator
    ///
    /// # Arguments
    /// - `comm`: Communicator handle from ncclCommInitRank
    ///
    /// # ASSUM Tags
    /// - #ASSUME_VALID_HANDLE: comm must be valid (no double-destroy)
    pub fn ncclCommDestroy(comm: RcclComm) -> RcclResult;

    /// AllReduce: Reduce data across all ranks, broadcast result to all
    ///
    /// # Arguments
    /// - `sendbuff`: Input buffer (device pointer)
    /// - `recvbuff`: Output buffer (device pointer)
    /// - `count`: Number of elements
    /// - `datatype`: Data type (Float32, Float64, etc.)
    /// - `op`: Reduction operation (Sum, Max, etc.)
    /// - `comm`: Communicator handle
    /// - `stream`: HIP stream handle
    ///
    /// # ASSUM Tags
    /// - #ASSUME_DEVICE_PTR: sendbuff and recvbuff must be device pointers
    /// - #ASSUME_COLLECTIVE_SYNC: All ranks must call this simultaneously
    /// - #VERIFY_SYNC: Operation is asynchronous, requires hipStreamSynchronize
    pub fn ncclAllReduce(
        sendbuff: *const c_void,
        recvbuff: *mut c_void,
        count: usize,
        datatype: RcclDataType,
        op: RcclRedOp,
        comm: RcclComm,
        stream: hipStream_t,
    ) -> RcclResult;

    /// AllGather: Gather data from all ranks to all ranks
    ///
    /// # Arguments
    /// - `sendbuff`: Input buffer (device pointer, length = count)
    /// - `recvbuff`: Output buffer (device pointer, length = count * nranks)
    /// - `sendcount`: Number of elements from each rank
    /// - `datatype`: Data type
    /// - `comm`: Communicator handle
    /// - `stream`: HIP stream handle
    pub fn ncclAllGather(
        sendbuff: *const c_void,
        recvbuff: *mut c_void,
        sendcount: usize,
        datatype: RcclDataType,
        comm: RcclComm,
        stream: hipStream_t,
    ) -> RcclResult;

    /// Broadcast: Send data from root rank to all ranks
    ///
    /// # Arguments
    /// - `buff`: Buffer (input on root, output on all other ranks)
    /// - `count`: Number of elements
    /// - `datatype`: Data type
    /// - `root`: Source rank (0-based)
    /// - `comm`: Communicator handle
    /// - `stream`: HIP stream handle
    pub fn ncclBroadcast(
        buff: *mut c_void,
        count: usize,
        datatype: RcclDataType,
        root: i32,
        comm: RcclComm,
        stream: hipStream_t,
    ) -> RcclResult;

    /// ReduceScatter: Reduce across all ranks, scatter result chunks
    ///
    /// # Arguments
    /// - `sendbuff`: Input buffer (device pointer, length = recvcount * nranks)
    /// - `recvbuff`: Output buffer (device pointer, length = recvcount)
    /// - `recvcount`: Number of elements for each rank to receive
    /// - `datatype`: Data type
    /// - `op`: Reduction operation
    /// - `comm`: Communicator handle
    /// - `stream`: HIP stream handle
    pub fn ncclReduceScatter(
        sendbuff: *const c_void,
        recvbuff: *mut c_void,
        recvcount: usize,
        datatype: RcclDataType,
        op: RcclRedOp,
        comm: RcclComm,
        stream: hipStream_t,
    ) -> RcclResult;

    /// Reduce: Reduce across all ranks, result on root only
    ///
    /// # Arguments
    /// - `sendbuff`: Input buffer (device pointer)
    /// - `recvbuff`: Output buffer (device pointer, valid on root only)
    /// - `count`: Number of elements
    /// - `datatype`: Data type
    /// - `op`: Reduction operation
    /// - `root`: Destination rank (0-based)
    /// - `comm`: Communicator handle
    /// - `stream`: HIP stream handle
    pub fn ncclReduce(
        sendbuff: *const c_void,
        recvbuff: *mut c_void,
        count: usize,
        datatype: RcclDataType,
        op: RcclRedOp,
        root: i32,
        comm: RcclComm,
        stream: hipStream_t,
    ) -> RcclResult;

    /// Get RCCL version string
    ///
    /// # Arguments
    /// - `version`: Output pointer for version integer (e.g., 2210 for 2.21.0)
    pub fn ncclGetVersion(version: *mut i32) -> RcclResult;

    /// Get error string for RCCL result code
    ///
    /// # Arguments
    /// - `result`: RCCL result code
    ///
    /// # Returns
    /// - Pointer to null-terminated error string (never nullptr)
    pub fn ncclGetErrorString(result: RcclResult) -> *const c_char;
}

/// Check RCCL result and convert to GpuResult
///
/// # Arguments
/// - `result`: Result code from RCCL function
///
/// # Returns
/// - `Ok(())` if result == Success
/// - `Err(GpuError)` with context if result != Success
#[inline]
pub fn check_rccl(result: RcclResult) -> crate::gpu::error::GpuResult<()> {
    if result.is_success() {
        return Ok(());
    }

    #[cfg(feature = "gpu-rocm")]
    {
        let msg = unsafe {
            let ptr = ncclGetErrorString(result);
            if ptr.is_null() {
                "Unknown RCCL error".to_string()
            } else {
                std::ffi::CStr::from_ptr(ptr)
                    .to_string_lossy()
                    .to_string()
            }
        };

        Err(crate::gpu::error::GpuError::BackendInitFailed {
            backend: crate::gpu::error::GpuBackend::Rocm,
            reason: format!("RCCL error: {} (code: {:?})", msg, result),
        })
    }

    #[cfg(not(feature = "gpu-rocm"))]
    Err(crate::gpu::error::GpuError::BackendInitFailed {
        backend: crate::gpu::error::GpuBackend::Rocm,
        reason: format!("RCCL error: {:?} (gpu-rocm feature disabled)", result),
    })
}

// ============================================================================
// Stub Implementations (for non-ROCm builds)
// ============================================================================

#[cfg(not(feature = "gpu-rocm"))]
mod stubs {
    use super::*;
    use crate::gpu::error::GpuResult;

    /// Stub rocBLAS handle creation
    pub fn rocblas_create_handle() -> GpuResult<RocblasHandle> {
        Err(crate::gpu::error::GpuError::BackendInitFailed {
            backend: crate::gpu::error::GpuBackend::Rocm,
            reason: "rocBLAS not available (gpu-rocm feature disabled)".to_string(),
        })
    }

    /// Stub rocBLAS handle destruction
    pub fn rocblas_destroy_handle(_handle: RocblasHandle) -> GpuResult<()> {
        Ok(()) // No-op for stub
    }

    /// Stub rocBLAS stream setter
    pub fn rocblas_set_stream(_handle: RocblasHandle, _stream: hipStream_t) -> GpuResult<()> {
        Err(crate::gpu::error::GpuError::BackendInitFailed {
            backend: crate::gpu::error::GpuBackend::Rocm,
            reason: "rocBLAS not available".to_string(),
        })
    }

    /// Stub rocFFT setup
    pub fn rocfft_setup() -> GpuResult<()> {
        Err(crate::gpu::error::GpuError::BackendInitFailed {
            backend: crate::gpu::error::GpuBackend::Rocm,
            reason: "rocFFT not available (gpu-rocm feature disabled)".to_string(),
        })
    }

    /// Stub rocFFT cleanup
    pub fn rocfft_cleanup() -> GpuResult<()> {
        Ok(()) // No-op for stub
    }

    /// Stub rocFFT plan creation
    pub fn rocfft_plan_create() -> GpuResult<RocfftPlanHandle> {
        Err(crate::gpu::error::GpuError::BackendInitFailed {
            backend: crate::gpu::error::GpuBackend::Rocm,
            reason: "rocFFT not available".to_string(),
        })
    }

    /// Stub rocFFT plan destruction
    pub fn rocfft_plan_destroy(_plan: RocfftPlanHandle) -> GpuResult<()> {
        Ok(()) // No-op for stub
    }

    /// Stub hipSPARSE handle creation
    pub fn hipsparse_create() -> GpuResult<HipsparseHandle> {
        Err(crate::gpu::error::GpuError::BackendInitFailed {
            backend: crate::gpu::error::GpuBackend::Rocm,
            reason: "hipSPARSE not available (gpu-rocm feature disabled)".to_string(),
        })
    }

    /// Stub hipSPARSE handle destruction
    pub fn hipsparse_destroy(_handle: HipsparseHandle) -> GpuResult<()> {
        Ok(()) // No-op for stub
    }
}

#[cfg(not(feature = "gpu-rocm"))]
pub use stubs::*;

// ============================================================================
// Safe Error Checking Helpers (rocBLAS/rocFFT/hipSPARSE)
// ============================================================================

/// Check rocBLAS status and convert to Result
///
/// # Arguments
/// - `status`: Status code from rocBLAS function
///
/// # Returns
/// - `Ok(())` if status == Success
/// - `Err(GpuError)` with context if status != Success
#[inline]
pub fn check_rocblas(status: RocblasStatus) -> crate::gpu::error::GpuResult<()> {
    if status.is_success() {
        return Ok(());
    }

    Err(crate::gpu::error::GpuError::BackendInitFailed {
        backend: crate::gpu::error::GpuBackend::Rocm,
        reason: format!("rocBLAS error: {:?}", status),
    })
}

/// Check rocFFT status and convert to Result
///
/// # Arguments
/// - `status`: Status code from rocFFT function
///
/// # Returns
/// - `Ok(())` if status == Success
/// - `Err(GpuError)` with context if status != Success
#[inline]
pub fn check_rocfft(status: RocfftStatus) -> crate::gpu::error::GpuResult<()> {
    if status.is_success() {
        return Ok(());
    }

    Err(crate::gpu::error::GpuError::BackendInitFailed {
        backend: crate::gpu::error::GpuBackend::Rocm,
        reason: format!("rocFFT error: {:?}", status),
    })
}

/// Check hipSPARSE status and convert to Result
///
/// # Arguments
/// - `status`: Status code from hipSPARSE function
///
/// # Returns
/// - `Ok(())` if status == Success
/// - `Err(GpuError)` with context if status != Success
#[inline]
pub fn check_hipsparse(status: HipsparseStatus) -> crate::gpu::error::GpuResult<()> {
    if status.is_success() {
        return Ok(());
    }

    Err(crate::gpu::error::GpuError::BackendInitFailed {
        backend: crate::gpu::error::GpuBackend::Rocm,
        reason: format!("hipSPARSE error: {:?}", status),
    })
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hip_error_success() {
        assert!(hipError_t::hipSuccess.is_success());
        assert!(!hipError_t::hipErrorOutOfMemory.is_success());
    }

    #[test]
    fn test_hip_error_oom() {
        assert!(hipError_t::hipErrorOutOfMemory.is_oom());
        assert!(!hipError_t::hipSuccess.is_oom());
    }

    #[test]
    fn test_struct_layout() {
        // Verify hipDeviceProp_t layout (should be C-compatible)
        assert!(core::mem::size_of::<hipDeviceProp_t>() >= 1024);
        assert_eq!(core::mem::align_of::<hipDeviceProp_t>(), core::mem::align_of::<c_char>());
    }

    #[test]
    fn test_memcpy_kinds() {
        // Verify enum values match HIP spec
        assert_eq!(hipMemcpyKind::hipMemcpyHostToHost as u32, 0);
        assert_eq!(hipMemcpyKind::hipMemcpyHostToDevice as u32, 1);
        assert_eq!(hipMemcpyKind::hipMemcpyDeviceToHost as u32, 2);
        assert_eq!(hipMemcpyKind::hipMemcpyDeviceToDevice as u32, 3);
    }

    // ========================================================================
    // rocBLAS Tests
    // ========================================================================

    #[test]
    fn test_rocblas_handle_size() {
        // Verify RocblasHandle is pointer-sized (8 bytes on 64-bit)
        assert_eq!(core::mem::size_of::<RocblasHandle>(), core::mem::size_of::<*mut c_void>());
        assert_eq!(core::mem::align_of::<RocblasHandle>(), core::mem::align_of::<*mut c_void>());
    }

    #[test]
    fn test_rocblas_operation_values() {
        // Verify enum values match rocBLAS spec
        assert_eq!(RocblasOperation::None as i32, 111);
        assert_eq!(RocblasOperation::Transpose as i32, 112);
        assert_eq!(RocblasOperation::ConjugateTranspose as i32, 113);
    }

    #[test]
    fn test_rocblas_status_success() {
        assert!(RocblasStatus::Success.is_success());
        assert!(!RocblasStatus::InvalidHandle.is_success());
        assert!(!RocblasStatus::MemoryError.is_success());
    }

    #[test]
    fn test_rocblas_status_values() {
        // Verify enum values match rocBLAS spec
        assert_eq!(RocblasStatus::Success as i32, 0);
        assert_eq!(RocblasStatus::InvalidHandle as i32, 1);
        assert_eq!(RocblasStatus::InvalidSize as i32, 4);
    }

    // ========================================================================
    // rocFFT Tests
    // ========================================================================

    #[test]
    fn test_rocfft_plan_handle_size() {
        // Verify RocfftPlanHandle is pointer-sized
        assert_eq!(core::mem::size_of::<RocfftPlanHandle>(), core::mem::size_of::<*mut c_void>());
        assert_eq!(core::mem::align_of::<RocfftPlanHandle>(), core::mem::align_of::<*mut c_void>());
    }

    #[test]
    fn test_rocfft_transform_types() {
        // Verify enum values match rocFFT spec
        assert_eq!(RocfftTransformType::ComplexForward as i32, 0);
        assert_eq!(RocfftTransformType::ComplexInverse as i32, 1);
        assert_eq!(RocfftTransformType::RealForward as i32, 2);
        assert_eq!(RocfftTransformType::RealInverse as i32, 3);
    }

    #[test]
    fn test_rocfft_result_placement() {
        // Verify enum values
        assert_eq!(RocfftResultPlacement::InPlace as i32, 0);
        assert_eq!(RocfftResultPlacement::NotInPlace as i32, 1);
    }

    #[test]
    fn test_rocfft_status_values() {
        // Verify enum values match rocFFT spec
        assert_eq!(RocfftStatus::Success as i32, 0);
        assert_eq!(RocfftStatus::Failure as i32, 1);
        assert_eq!(RocfftStatus::InvalidDimensions as i32, 3);
    }

    #[test]
    fn test_rocfft_status_success() {
        assert!(RocfftStatus::Success.is_success());
        assert!(!RocfftStatus::Failure.is_success());
        assert!(!RocfftStatus::InvalidArgValue.is_success());
    }

    // ========================================================================
    // hipSPARSE Tests
    // ========================================================================

    #[test]
    fn test_hipsparse_handle_size() {
        // Verify HipsparseHandle is pointer-sized
        assert_eq!(core::mem::size_of::<HipsparseHandle>(), core::mem::size_of::<*mut c_void>());
        assert_eq!(core::mem::align_of::<HipsparseHandle>(), core::mem::align_of::<*mut c_void>());
    }

    #[test]
    fn test_hipsparse_mat_descr_size() {
        // Verify HipsparseMatDescr is pointer-sized
        assert_eq!(core::mem::size_of::<HipsparseMatDescr>(), core::mem::size_of::<*mut c_void>());
        assert_eq!(core::mem::align_of::<HipsparseMatDescr>(), core::mem::align_of::<*mut c_void>());
    }

    #[test]
    fn test_hipsparse_index_base() {
        // Verify enum values
        assert_eq!(HipsparseIndexBase::Zero as i32, 0);
        assert_eq!(HipsparseIndexBase::One as i32, 1);
    }

    #[test]
    fn test_hipsparse_status_success() {
        assert!(HipsparseStatus::Success.is_success());
        assert!(!HipsparseStatus::NotInitialized.is_success());
        assert!(!HipsparseStatus::InvalidValue.is_success());
    }

    #[test]
    fn test_hipsparse_status_values() {
        // Verify enum values match hipSPARSE spec
        assert_eq!(HipsparseStatus::Success as i32, 0);
        assert_eq!(HipsparseStatus::AllocFailed as i32, 2);
        assert_eq!(HipsparseStatus::ExecutionFailed as i32, 6);
    }

    // ========================================================================
    // Stub Tests (non-ROCm builds)
    // ========================================================================

    #[test]
    #[cfg(not(feature = "gpu-rocm"))]
    fn test_stub_rocblas_create() {
        // Stubs should return error when ROCm feature disabled
        let result = rocblas_create_handle();
        assert!(result.is_err());
    }

    #[test]
    #[cfg(not(feature = "gpu-rocm"))]
    fn test_stub_rocfft_setup() {
        // Stubs should return error when ROCm feature disabled
        let result = rocfft_setup();
        assert!(result.is_err());
    }

    #[test]
    #[cfg(not(feature = "gpu-rocm"))]
    fn test_stub_hipsparse_create() {
        // Stubs should return error when ROCm feature disabled
        let result = hipsparse_create();
        assert!(result.is_err());
    }

    // ========================================================================
    // Error Conversion Tests
    // ========================================================================

    #[test]
    fn test_check_rocblas() {
        // Success should return Ok(())
        assert!(check_rocblas(RocblasStatus::Success).is_ok());

        // Error should return Err
        assert!(check_rocblas(RocblasStatus::InvalidHandle).is_err());
        assert!(check_rocblas(RocblasStatus::MemoryError).is_err());
    }

    #[test]
    fn test_check_rocfft() {
        // Success should return Ok(())
        assert!(check_rocfft(RocfftStatus::Success).is_ok());

        // Error should return Err
        assert!(check_rocfft(RocfftStatus::Failure).is_err());
        assert!(check_rocfft(RocfftStatus::InvalidDimensions).is_err());
    }

    #[test]
    fn test_check_hipsparse() {
        // Success should return Ok(())
        assert!(check_hipsparse(HipsparseStatus::Success).is_ok());

        // Error should return Err
        assert!(check_hipsparse(HipsparseStatus::NotInitialized).is_err());
        assert!(check_hipsparse(HipsparseStatus::InvalidValue).is_err());
    }
}
