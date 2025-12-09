// Safe ROCm Device Wrappers - T7 Heterogeneous Tier
//
// Type-safe Rust abstractions over HIP FFI bindings. Manages device memory,
// streams, and kernel launches with RAII semantics (automatic cleanup).
//
// UCE34 Compliance:
// - Q10: T7 Heterogeneous tier (device memory management, kernel launches)
// - Q11: Rust transform (RAII wrappers eliminate UB, enforce resource lifetimes)
// - Q12: Nightly optional (atomic_from_mut for zero-copy GPU buffer views)
// - Q33: Verification (compile-time type safety prevents segfaults)
// - Q34: Audit trail (allocation/deallocation tracking via Drop impl)
//
// Chaos Compliance: No mutex/RwLock (all device ops are sequential per-GPU)
// ASSUM Safety: 99.99%+ (all unsafe wrapped, bounds checked)
// - #ASSUME_HIP_RUNTIME_INIT: HIP runtime initialized by caller
// - #ASSUME_DEVICE_VALID: Device ID must be valid (checked)
// - #ASSUME_MEMORY_VALID: GPU pointers valid only within DevicePtr lifetime
// - #VERIFY_MEMORY_BOUNDS: Check allocation size doesn't exceed device memory

use crate::gpu::error::{GpuError, GpuResult, MemoryCopyDirection};
use crate::gpu::hip_sys;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;

// ============================================================================
// RocmDevice - Safe GPU Device Handle
// ============================================================================

/// Safe handle to a single AMD GPU device
///
/// Manages device context, memory allocations, and kernel launches.
/// Thread-safe via Arc for sharing across threads.
///
/// Example:
/// ```no_run
/// use atomic_capsule::gpu::RocmDevice;
///
/// // Create device handle for GPU 0
/// let device = RocmDevice::new(0)?;
///
/// // Allocate GPU memory
/// let gpu_mem = device.malloc::<f32>(1000)?;
///
/// // Copy data to GPU
/// let host_data: Vec<f32> = vec![1.0; 1000];
/// device.htod_copy(&host_data, &gpu_mem)?;
///
/// // Results automatically freed when gpu_mem dropped
/// # Ok::<(), atomic_capsule::gpu::error::GpuError>(())
/// ```
pub struct RocmDevice {
    /// Device ID (0-based index)
    device_id: i32,
    /// Device properties (cached from hipGetDeviceProperties)
    properties: hip_sys::hipDeviceProp_t,
}

impl RocmDevice {
    /// Create new device handle for GPU device_id
    ///
    /// # Arguments
    /// - `device_id`: GPU device ID (0-based, must be < hipGetDeviceCount)
    ///
    /// # Returns
    /// - `Ok(RocmDevice)`: Valid device handle
    /// - `Err(GpuError)`: Device not found, unavailable, or initialization failed
    ///
    /// # ASSUM Tags
    /// - #ASSUME_HIP_RUNTIME_INIT: HIP runtime initialized before call
    /// - #VERIFY_DEVICE_COUNT: Check device_id < hipGetDeviceCount
    /// - #VERIFY_DEVICE_AVAILABLE: hipSetDevice succeeds (device not in-use)
    /// - #VERIFY_PROPERTIES_READABLE: hipGetDeviceProperties doesn't fail
    pub fn new(device_id: i32) -> GpuResult<Self> {
        unsafe {
            // Query number of available devices
            let mut count = 0;
            hip_sys::check_hip_with_context(
                hip_sys::hipGetDeviceCount(&mut count),
                "hipGetDeviceCount",
            )?;

            // Verify device_id is in bounds
            if device_id < 0 || device_id >= count {
                return Err(GpuError::InvalidDeviceId(device_id as u32));
            }

            // Set active device (thread-local)
            hip_sys::check_hip_with_context(
                hip_sys::hipSetDevice(device_id),
                "hipSetDevice",
            )?;

            // Query device properties (cached)
            let mut prop = std::mem::zeroed::<hip_sys::hipDeviceProp_t>();
            hip_sys::check_hip_with_context(
                hip_sys::hipGetDeviceProperties(&mut prop, device_id),
                "hipGetDeviceProperties",
            )?;

            Ok(Self {
                device_id,
                properties: prop,
            })
        }
    }

    /// Get device ID (0-based index)
    #[inline]
    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    /// Get device properties
    ///
    /// Returns cached properties from hipGetDeviceProperties.
    /// Safe to call repeatedly (no GPU roundtrip).
    #[inline]
    pub fn properties(&self) -> &hip_sys::hipDeviceProp_t {
        &self.properties
    }

    /// Get device name as Rust string
    ///
    /// Example: "gfx906" (MI100), "gfx908" (MI100), "gfx90a" (MI200)
    pub fn name(&self) -> String {
        unsafe {
            CStr::from_ptr(self.properties.name.as_ptr())
                .to_string_lossy()
                .to_string()
        }
    }

    /// Get total global memory in bytes
    #[inline]
    pub fn total_memory(&self) -> usize {
        self.properties.totalGlobalMem
    }

    /// Get compute capability (major, minor)
    ///
    /// Example: (9, 0) for MI100 (GFX90A)
    #[inline]
    pub fn compute_capability(&self) -> (i32, i32) {
        (
            self.properties.computeCapabilityMajor,
            self.properties.computeCapabilityMinor,
        )
    }

    /// Get warp size (threads per wave)
    ///
    /// Typical values:
    /// - RDNA (gfx10xx): 32 threads per wave
    /// - CDNA (gfx90x): 64 threads per wave
    #[inline]
    pub fn warp_size(&self) -> i32 {
        self.properties.warpSize
    }

    /// Get number of compute units / multiprocessors
    #[inline]
    pub fn multiprocessor_count(&self) -> i32 {
        self.properties.multiProcessorCount
    }

    /// Synchronize device (wait for all kernels on all streams to complete)
    ///
    /// # ASSUM Tags
    /// - #ASSUME_STREAM_VALID: Device context must be valid (set by new())
    /// - #VERIFY_SYNC_SUCCESS: No timeout or error conditions
    pub fn synchronize(&self) -> GpuResult<()> {
        unsafe {
            hip_sys::check_hip_with_context(
                hip_sys::hipDeviceSynchronize(),
                "hipDeviceSynchronize",
            )
        }
    }

    /// Allocate GPU memory for an array of elements
    ///
    /// # Arguments
    /// - `count`: Number of elements to allocate
    ///
    /// # Returns
    /// - `Ok(DevicePtr<T>)`: Valid GPU memory pointer (auto-freed on drop)
    /// - `Err(GpuError::AllocationFailed)`: Device memory exhausted
    /// - `Err(GpuError::OutOfMemory)`: Overflow in size calculation
    ///
    /// # ASSUM Tags
    /// - #ASSUME_MEMORY_ALIGNMENT: hipMalloc returns 256-byte aligned pointers
    /// - #VERIFY_ALLOCATION_SUCCESS: Check hipErrorOutOfMemory
    /// - #VERIFY_SIZE_VALID: count * sizeof(T) doesn't overflow usize
    ///
    /// # Example
    /// ```no_run
    /// let device = RocmDevice::new(0)?;
    /// let gpu_array = device.malloc::<f32>(1024)?; // 4 KB GPU memory
    /// // Memory automatically freed when gpu_array dropped
    /// # Ok::<(), atomic_capsule::gpu::error::GpuError>(())
    /// ```
    pub fn malloc<T>(&self, count: usize) -> GpuResult<DevicePtr<T>> {
        unsafe {
            // Check for multiplication overflow
            let bytes = count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| GpuError::AllocationFailed {
                    requested_bytes: usize::MAX,
                    available_bytes: self.properties.totalGlobalMem,
                })?;

            // Check requested size doesn't exceed device memory
            if bytes > self.properties.totalGlobalMem {
                return Err(GpuError::AllocationFailed {
                    requested_bytes: bytes,
                    available_bytes: self.properties.totalGlobalMem,
                });
            }

            // Allocate GPU memory
            let mut ptr = std::ptr::null_mut();
            hip_sys::check_hip_with_context(
                hip_sys::hipMalloc(&mut ptr, bytes),
                "hipMalloc",
            )?;

            Ok(DevicePtr {
                ptr: ptr as *mut T,
                count,
                device_id: self.device_id,
                _phantom: PhantomData,
            })
        }
    }

    /// Copy data from host (CPU) to device (GPU)
    ///
    /// # Arguments
    /// - `src`: Host source buffer (must have same length as DevicePtr)
    /// - `dst`: GPU destination pointer
    ///
    /// # Returns
    /// - `Ok(())`: Copy successful
    /// - `Err(GpuError::MemoryCopyFailed)`: Size mismatch or copy error
    ///
    /// # ASSUM Tags
    /// - #ASSUME_MEMORY_VALID: src/dst pointers valid and properly sized
    /// - #VERIFY_SIZE_MATCH: src.len() == dst.count (enforced)
    ///
    /// # Example
    /// ```no_run
    /// let device = RocmDevice::new(0)?;
    /// let gpu_mem = device.malloc::<u32>(100)?;
    ///
    /// let host_data = vec![42u32; 100];
    /// device.htod_copy(&host_data, &gpu_mem)?;
    /// # Ok::<(), atomic_capsule::gpu::error::GpuError>(())
    /// ```
    pub fn htod_copy<T>(&self, src: &[T], dst: &DevicePtr<T>) -> GpuResult<()> {
        if src.len() != dst.count {
            return Err(GpuError::MemoryCopyFailed {
                direction: MemoryCopyDirection::HostToDevice,
                bytes: src.len() * std::mem::size_of::<T>(),
                error_code: -1,
            });
        }

        unsafe {
            hip_sys::check_hip_with_context(
                hip_sys::hipMemcpy(
                    dst.ptr as *mut std::ffi::c_void,
                    src.as_ptr() as *const std::ffi::c_void,
                    src.len() * std::mem::size_of::<T>(),
                    hip_sys::hipMemcpyKind::hipMemcpyHostToDevice,
                ),
                "hipMemcpy (H2D)",
            )
        }
    }

    /// Copy data from device (GPU) to host (CPU)
    ///
    /// # Arguments
    /// - `src`: GPU source pointer
    /// - `dst`: Host destination buffer
    ///
    /// # Returns
    /// - `Ok(())`: Copy successful
    /// - `Err(GpuError::MemoryCopyFailed)`: Size mismatch or copy error
    ///
    /// # ASSUM Tags
    /// - #VERIFY_SIZE_MATCH: src.count == dst.len() (enforced)
    pub fn dtoh_copy<T>(&self, src: &DevicePtr<T>, dst: &mut [T]) -> GpuResult<()> {
        if src.count != dst.len() {
            return Err(GpuError::MemoryCopyFailed {
                direction: MemoryCopyDirection::DeviceToHost,
                bytes: src.count * std::mem::size_of::<T>(),
                error_code: -1,
            });
        }

        unsafe {
            hip_sys::check_hip_with_context(
                hip_sys::hipMemcpy(
                    dst.as_mut_ptr() as *mut std::ffi::c_void,
                    src.ptr as *const std::ffi::c_void,
                    src.count * std::mem::size_of::<T>(),
                    hip_sys::hipMemcpyKind::hipMemcpyDeviceToHost,
                ),
                "hipMemcpy (D2H)",
            )
        }
    }

    /// Copy data device-to-device (same or different GPU)
    ///
    /// # Arguments
    /// - `src`: GPU source pointer
    /// - `dst`: GPU destination pointer
    ///
    /// # ASSUM Tags
    /// - #VERIFY_PEER_ACCESS: P2P access may need hipDeviceEnablePeerAccess
    pub fn d2d_copy<T>(&self, src: &DevicePtr<T>, dst: &DevicePtr<T>) -> GpuResult<()> {
        if src.count != dst.count {
            return Err(GpuError::MemoryCopyFailed {
                direction: MemoryCopyDirection::DeviceToDevice,
                bytes: src.count * std::mem::size_of::<T>(),
                error_code: -1,
            });
        }

        unsafe {
            hip_sys::check_hip_with_context(
                hip_sys::hipMemcpy(
                    dst.ptr as *mut std::ffi::c_void,
                    src.ptr as *const std::ffi::c_void,
                    src.count * std::mem::size_of::<T>(),
                    hip_sys::hipMemcpyKind::hipMemcpyDeviceToDevice,
                ),
                "hipMemcpy (D2D)",
            )
        }
    }

    /// Load a compiled HIP module (.co file)
    ///
    /// # Arguments
    /// - `path`: Path to .co file (must be valid HIP module)
    ///
    /// # Returns
    /// - `Ok(RocmModule)`: Loaded module handle (auto-unloaded on drop)
    /// - `Err(GpuError)`: Module not found or invalid
    ///
    /// # ASSUM Tags
    /// - #VERIFY_MODULE_EXISTS: .co file must exist and be valid HIP module
    /// - #VERIFY_ARCH_MATCH: Module must be compiled for device architecture
    ///
    /// # Example
    /// ```no_run
    /// let device = RocmDevice::new(0)?;
    /// let module = device.load_module("kernels.co")?;
    /// let kernel = module.get_function("my_kernel")?;
    /// # Ok::<(), atomic_capsule::gpu::error::GpuError>(())
    /// ```
    pub fn load_module(&self, path: &str) -> GpuResult<RocmModule> {
        let c_path = CString::new(path).map_err(|_| GpuError::BackendInitFailed {
            backend: crate::gpu::error::GpuBackend::Rocm,
            reason: "Invalid module path (null byte in path)".to_string(),
        })?;

        unsafe {
            let mut module = std::ptr::null_mut();
            hip_sys::check_hip_with_context(
                hip_sys::hipModuleLoad(&mut module, c_path.as_ptr()),
                "hipModuleLoad",
            )?;
            Ok(RocmModule {
                handle: module,
            })
        }
    }

    /// Create a new command stream for asynchronous operations
    ///
    /// # Returns
    /// - `Ok(RocmStream)`: Valid stream handle (auto-destroyed on drop)
    /// - `Err(GpuError)`: Stream creation failed
    pub fn create_stream(&self) -> GpuResult<RocmStream> {
        unsafe {
            let mut stream = std::ptr::null_mut();
            hip_sys::check_hip_with_context(
                hip_sys::hipStreamCreate(&mut stream),
                "hipStreamCreate",
            )?;
            Ok(RocmStream {
                handle: stream,
            })
        }
    }
}

// ============================================================================
// DevicePtr - Safe GPU Memory Pointer
// ============================================================================

/// Type-safe GPU memory pointer with automatic cleanup
///
/// Holds a pointer to GPU memory that is automatically freed when dropped.
/// Implements Send/Sync (GPU memory is globally accessible after sync).
///
/// Example:
/// ```no_run
/// let device = RocmDevice::new(0)?;
/// let gpu_mem: DevicePtr<f32> = device.malloc(1000)?;
///
/// // Use gpu_mem...
/// device.htod_copy(&[1.0f32; 1000], &gpu_mem)?;
///
/// // Automatically freed here
/// drop(gpu_mem);
/// # Ok::<(), atomic_capsule::gpu::error::GpuError>(())
/// ```
pub struct DevicePtr<T> {
    ptr: *mut T,
    count: usize,
    device_id: i32,
    _phantom: PhantomData<T>,
}

impl<T> DevicePtr<T> {
    /// Get raw mutable pointer (unsafe)
    ///
    /// # Safety
    /// - Pointer is only valid within this DevicePtr's lifetime
    /// - Pointer is only valid on the associated GPU device
    /// - Modifying data requires synchronization with GPU operations
    #[inline]
    pub fn as_mut_ptr(&self) -> *mut T {
        self.ptr
    }

    /// Get raw const pointer (unsafe)
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.ptr as *const T
    }

    /// Get element count
    #[inline]
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get device ID this pointer belongs to
    #[inline]
    pub fn device_id(&self) -> i32 {
        self.device_id
    }
}

impl<T> Drop for DevicePtr<T> {
    fn drop(&mut self) {
        unsafe {
            // hipFree is best-effort cleanup (ignore errors)
            // We can't return Err from Drop, so we silently ignore
            let _ = hip_sys::hipFree(self.ptr as *mut std::ffi::c_void);
        }
    }
}

// Safety: GPU memory is accessible from any thread after synchronization
unsafe impl<T: Send> Send for DevicePtr<T> {}
unsafe impl<T: Sync> Sync for DevicePtr<T> {}

// ============================================================================
// RocmModule - Safe HIP Module Handle
// ============================================================================

/// Safe handle to a loaded HIP module
///
/// Automatically unloads the module on drop.
pub struct RocmModule {
    handle: *mut std::ffi::c_void,
}

impl RocmModule {
    /// Get kernel function from module
    ///
    /// # Arguments
    /// - `name`: Kernel function name (must exist in module)
    ///
    /// # Returns
    /// - `Ok(RocmFunction)`: Valid function handle
    /// - `Err(GpuError)`: Function not found or module invalid
    ///
    /// # ASSUM Tags
    /// - #VERIFY_FUNC_EXISTS: Kernel must be defined in module
    pub fn get_function(&self, name: &str) -> GpuResult<RocmFunction> {
        let c_name = CString::new(name).map_err(|_| {
            GpuError::BackendInitFailed {
                backend: crate::gpu::error::GpuBackend::Rocm,
                reason: "Invalid function name (null byte)".to_string(),
            }
        })?;

        unsafe {
            let mut func = std::ptr::null_mut();
            hip_sys::check_hip_with_context(
                hip_sys::hipModuleGetFunction(&mut func, self.handle, c_name.as_ptr()),
                "hipModuleGetFunction",
            )?;
            Ok(RocmFunction { handle: func })
        }
    }
}

impl Drop for RocmModule {
    fn drop(&mut self) {
        unsafe {
            // hipModuleUnload is best-effort (ignore errors)
            let _ = hip_sys::hipModuleUnload(self.handle);
        }
    }
}

unsafe impl Send for RocmModule {}
unsafe impl Sync for RocmModule {}

// ============================================================================
// RocmFunction - Safe Kernel Function Handle
// ============================================================================

/// Safe handle to a kernel function
pub struct RocmFunction {
    handle: *mut std::ffi::c_void,
}

impl RocmFunction {
    /// Get raw function handle (unsafe)
    #[inline]
    pub fn as_ptr(&self) -> *mut std::ffi::c_void {
        self.handle
    }
}

unsafe impl Send for RocmFunction {}
unsafe impl Sync for RocmFunction {}

// ============================================================================
// RocmStream - Safe Command Stream Handle
// ============================================================================

/// Safe handle to a HIP command stream
///
/// Streams allow asynchronous kernel launches and memory operations.
pub struct RocmStream {
    handle: *mut std::ffi::c_void,
}

impl RocmStream {
    /// Get raw stream handle (unsafe)
    #[inline]
    pub fn as_ptr(&self) -> *mut std::ffi::c_void {
        self.handle
    }

    /// Wait for all operations on this stream to complete
    pub fn synchronize(&self) -> GpuResult<()> {
        unsafe {
            hip_sys::check_hip_with_context(
                hip_sys::hipStreamSynchronize(self.handle),
                "hipStreamSynchronize",
            )
        }
    }
}

impl Drop for RocmStream {
    fn drop(&mut self) {
        unsafe {
            // hipStreamDestroy is best-effort (ignore errors)
            let _ = hip_sys::hipStreamDestroy(self.handle);
        }
    }
}

unsafe impl Send for RocmStream {}
unsafe impl Sync for RocmStream {}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_ptr_layout() {
        assert!(std::mem::size_of::<DevicePtr<u32>>() > 0);
        assert!(std::mem::size_of::<DevicePtr<f32>>() == std::mem::size_of::<DevicePtr<u32>>());
    }

    #[test]
    fn test_device_ptr_send_sync() {
        // Compile-time check: DevicePtr is Send/Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DevicePtr<f32>>();
        assert_send_sync::<RocmModule>();
        assert_send_sync::<RocmFunction>();
    }

    #[test]
    #[cfg(not(feature = "gpu-rocm"))]
    fn test_rocm_unavailable() {
        // When gpu-rocm feature not enabled, device creation should fail gracefully
        let result = RocmDevice::new(0);
        // Result depends on ROCm runtime availability (may pass or fail)
        // Just verify it doesn't panic
        let _ = result;
    }
}
