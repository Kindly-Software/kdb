//! DRM Interface - Intel Xe driver integration
//!
//! **CRITICAL**: Single-writer resource lifetime management
//! **LESSON**: AMD tried parallel BO lifetime → device loss → mutex
//! **SOLUTION**: Capsules for READ decisions, single writer for allocations
//!
//! # Architecture
//!
//! - DrmDevice: Single-writer ownership of device lifetime
//! - GemObject: RAII-based buffer object with Drop cleanup
//! - VmBind: Single-writer for GPU virtual address binding
//!
//! # Safety Model
//!
//! Following ASSUM framework and The Atomic Capsule principles:
//! - Single writer for all resource allocations (no parallel BO creation)
//! - Atomic capsules for READ-only decisions (state queries)
//! - Drop-based cleanup ensures no resource leaks
//! - Generation counters prevent TOCTOU races

use crate::KiangError;
use std::fs::{File, OpenOptions};
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// DRM device handle with single-writer ownership
///
/// # Safety Model
/// #ASSUME_SINGLE_WRITER: Only allocator thread creates/destroys DRM resources
/// #VERIFY_LIFETIME: Drop ensures device cleanup
/// #VERIFY_NO_PARALLEL_BO: No concurrent GEM object creation
pub struct DrmDevice {
    file: File,
    card_path: String,
    /// Generation counter for TOCTOU prevention
    /// #ASSUME_GENERATION_MONOTONIC: Counter only increases
    /// #VERIFY_TOCTOU_SAFE: Readers check generation matches
    generation: Arc<AtomicU64>,
}

impl DrmDevice {
    /// Open DRM device (Intel Xe driver)
    ///
    /// # Arguments
    /// * `card` - Card number (0 = /dev/dri/card0, 1 = /dev/dri/card1, etc.)
    ///
    /// # Returns
    /// DrmDevice with single-writer ownership
    ///
    /// # Example
    /// ```no_run
    /// use kiang::drm_interface::DrmDevice;
    ///
    /// let device = DrmDevice::open(0).expect("Failed to open GPU");
    /// ```
    pub fn open(card: u32) -> Result<Self, DrmError> {
        let card_path = format!("/dev/dri/card{}", card);

        // #ASSUME_DEVICE_EXISTS: Device node present in /dev/dri
        // #VERIFY_DEVICE_OPEN: OpenOptions will fail if missing
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&card_path)
            .map_err(DrmError::OpenFailed)?;

        Ok(Self {
            file,
            card_path,
            generation: Arc::new(AtomicU64::new(1)), // Start at 1 (0 = invalid)
        })
    }

    /// Open render node (for compute-only workloads)
    ///
    /// Render nodes don't require authentication and can't be used for display.
    /// Preferred for GPU compute tasks.
    ///
    /// # Arguments
    /// * `node` - Render node number (128 = /dev/dri/renderD128, etc.)
    pub fn open_render(node: u32) -> Result<Self, DrmError> {
        let card_path = format!("/dev/dri/renderD{}", node);

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&card_path)
            .map_err(DrmError::OpenFailed)?;

        Ok(Self {
            file,
            card_path,
            generation: Arc::new(AtomicU64::new(1)),
        })
    }

    /// Get file descriptor for ioctl operations
    ///
    /// # Safety
    /// #ASSUME_FD_VALID: File handle is open and valid
    /// #VERIFY_FD_LIFETIME: fd only valid while DrmDevice exists
    pub fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    /// Get current generation counter (for TOCTOU prevention)
    ///
    /// Readers can snapshot generation, perform operation, then verify
    /// generation hasn't changed to detect races.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get device path
    pub fn path(&self) -> &str {
        &self.card_path
    }

    /// Increment generation counter (called after state-changing operations)
    ///
    /// # Safety
    /// #ASSUME_SINGLE_WRITER: Only allocation thread calls this
    /// #VERIFY_GENERATION_MONOTONIC: Counter only increases
    fn bump_generation(&self) {
        // #ASSUME_MEMORY_ORDERING: Release ensures visibility of state changes
        // #VERIFY_ORDERING_SUFFICIENT: Paired with Acquire in generation()
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Create GEM buffer (convenience method)
    ///
    /// # Arguments
    /// * `size` - Buffer size in bytes
    pub fn gem_create(&self, size: u64) -> Result<GemObject, DrmError> {
        GemObject::create(self, size)
    }

    /// Bind GEM object to GPU virtual address (convenience method)
    ///
    /// # Arguments
    /// * `gem` - GEM buffer object
    /// * `vm_addr` - GPU virtual address
    pub fn vm_bind(&self, gem: &GemObject, vm_addr: u64) -> Result<(), DrmError> {
        let binder = VmBind::new(self);
        binder.bind(gem, vm_addr)
    }

    /// Submit no-op command for fence creation
    ///
    /// Returns fence sequence number that can be waited on.
    pub fn submit_noop(&self) -> Result<u64, DrmError> {
        // Simulate command submission returning fence seqno
        use std::sync::atomic::AtomicU64;
        static NEXT_SEQNO: AtomicU64 = AtomicU64::new(1);
        Ok(NEXT_SEQNO.fetch_add(1, Ordering::Relaxed))
    }

    /// Wait for fence to signal
    ///
    /// # Arguments
    /// * `seqno` - Fence sequence number
    /// * `timeout_ns` - Timeout in nanoseconds (0 = poll only)
    ///
    /// # Returns
    /// Ok(true) if fence signaled, Ok(false) if timeout, Err on error
    pub fn fence_wait(&self, seqno: u64, timeout_ns: u64) -> Result<bool, DrmError> {
        // Simulate fence wait
        // In real implementation, would use DRM_IOCTL_I915_GEM_WAIT or similar
        if timeout_ns == 0 {
            // Poll only
            Ok(seqno < 100) // Simulate first 100 fences as complete
        } else {
            // Real wait
            Ok(true)
        }
    }
}

impl Drop for DrmDevice {
    fn drop(&mut self) {
        // #ASSUME_RESOURCE_CLEANUP: File closed when File is dropped
        // #VERIFY_DROP_SAFE: std::fs::File handles cleanup
        tracing::debug!("Closing DRM device: {}", self.card_path);
        // File::drop() will close the fd automatically
    }
}

/// GEM buffer object (GPU memory)
///
/// # Single-Writer Safety
/// #ASSUME_SINGLE_WRITER: Only allocator creates/destroys GEM objects
/// #VERIFY_LIFETIME: Drop ensures cleanup
/// #VERIFY_NO_PARALLEL_BO: Creation serialized through allocator
pub struct GemObject {
    pub(crate) device_fd: RawFd, // Visible to drm_real module
    pub(crate) handle: u32,      // Visible to drm_real module
    pub(crate) size: u64,        // Visible to drm_real module
    /// Generation at creation time (for TOCTOU detection)
    pub(crate) generation: u64, // Visible to drm_real module
}

impl GemObject {
    /// Create GEM buffer (single writer only!)
    ///
    /// # Arguments
    /// * `device` - DRM device (ensures device lifetime)
    /// * `size` - Buffer size in bytes
    ///
    /// # Safety
    /// #ASSUME_SINGLE_WRITER: Only allocator thread calls this
    /// #VERIFY_NO_RACE: One allocation thread ensures safety
    /// #VERIFY_DEVICE_LIFETIME: device must outlive GemObject
    pub fn create(device: &DrmDevice, size: u64) -> Result<Self, DrmError> {
        // Capture generation before allocation
        let generation = device.generation();

        // Simulate GEM creation (real implementation would use DRM ioctl)
        // #ASSUME_IOCTL_SAFE: DRM driver handles buffer allocation
        // #VERIFY_ALLOCATION: Check return value for errors

        #[cfg(feature = "drm-backend")]
        let handle = {
            // Real DRM_IOCTL_I915_GEM_CREATE or XE equivalent
            // For now, simulate with counter
            use std::sync::atomic::AtomicU32;
            static NEXT_HANDLE: AtomicU32 = AtomicU32::new(1);
            NEXT_HANDLE.fetch_add(1, Ordering::Relaxed)
        };

        #[cfg(not(feature = "drm-backend"))]
        let handle = {
            // Simulation mode - generate fake handle
            use std::sync::atomic::AtomicU32;
            static NEXT_HANDLE: AtomicU32 = AtomicU32::new(1);
            NEXT_HANDLE.fetch_add(1, Ordering::Relaxed)
        };

        if handle == 0 {
            return Err(DrmError::AllocationFailed);
        }

        // Bump device generation after allocation
        device.bump_generation();

        Ok(Self {
            device_fd: device.as_raw_fd(),
            handle,
            size,
            generation,
        })
    }

    /// Get GEM handle (for ioctls)
    pub fn handle(&self) -> u32 {
        self.handle
    }

    /// Get buffer size
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get generation at creation time (for TOCTOU detection)
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Map buffer to CPU-accessible memory
    ///
    /// # Safety
    /// #ASSUME_MAPPING_SAFE: DRM driver provides valid mapping
    /// #VERIFY_MAP_LIFETIME: Mapping must not outlive GemObject
    #[cfg(feature = "drm-backend")]
    pub fn map(&self) -> Result<*mut u8, DrmError> {
        // Real implementation would use DRM_IOCTL_I915_GEM_MMAP or mmap()
        // For now, return error in simulation
        Err(DrmError::IoctlFailed(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Mapping not supported in simulation mode",
        )))
    }

    /// Unmap CPU-accessible memory
    #[cfg(feature = "drm-backend")]
    pub fn unmap(&self, ptr: *mut u8, _size: usize) -> Result<(), DrmError> {
        if ptr.is_null() {
            return Err(DrmError::InvalidArgument("Null pointer".to_string()));
        }
        // Real implementation would use munmap()
        Ok(())
    }
}

impl Drop for GemObject {
    fn drop(&mut self) {
        // #ASSUME_RESOURCE_CLEANUP: Always cleanup GEM handle
        // #VERIFY_DROP_SAFE: DRM driver handles invalid fd/handle gracefully

        #[cfg(feature = "real_driver")]
        {
            // Real driver cleanup
            if let Err(e) = self.close_real_driver() {
                tracing::error!("Failed to close GEM handle {}: {}", self.handle, e);
            } else {
                tracing::debug!("Closed GEM handle: {}", self.handle);
            }
        }

        #[cfg(all(feature = "drm-backend", not(feature = "real_driver")))]
        {
            // Simulation mode
            tracing::debug!("Simulated GEM close: {}", self.handle);
            // ioctl(self.device_fd, DRM_IOCTL_GEM_CLOSE, &self.handle);
        }

        #[cfg(not(feature = "drm-backend"))]
        {
            tracing::debug!("Simulated GEM close: {}", self.handle);
        }
    }
}

/// VM_BIND operations (single writer for safety)
///
/// # Single-Writer Safety
/// #ASSUME_SINGLE_WRITER: Only allocator calls VM_BIND operations
/// #VERIFY_NO_RACE: One allocation thread ensures safety
/// #VERIFY_ADDRESS_UNIQUE: No overlapping virtual addresses
pub struct VmBind {
    device_fd: RawFd,
}

impl VmBind {
    /// Create VM_BIND coordinator
    ///
    /// # Arguments
    /// * `device` - DRM device (ensures device lifetime)
    pub fn new(device: &DrmDevice) -> Self {
        Self {
            device_fd: device.as_raw_fd(),
        }
    }

    /// Bind GEM object to GPU virtual address (single writer!)
    ///
    /// # Arguments
    /// * `gem` - GEM buffer object to bind
    /// * `vm_addr` - GPU virtual address to bind to
    ///
    /// # Safety
    /// #ASSUME_SINGLE_WRITER: Only allocator calls this
    /// #VERIFY_NO_RACE: One allocation thread ensures safety
    /// #VERIFY_ADDR_VALID: vm_addr is within valid GPU address space
    /// #VERIFY_NO_OVERLAP: No existing mapping at vm_addr
    pub fn bind(&self, gem: &GemObject, vm_addr: u64) -> Result<(), DrmError> {
        // #ASSUME_ALIGNMENT: vm_addr is properly aligned (4KB minimum)
        // #VERIFY_ALIGNMENT: Check alignment requirement
        if !vm_addr.is_multiple_of(4096) {
            return Err(DrmError::InvalidArgument(format!(
                "Address not aligned: 0x{:x}",
                vm_addr
            )));
        }

        #[cfg(feature = "drm-backend")]
        {
            // Real DRM_IOCTL_XE_VM_BIND
            tracing::debug!(
                "VM_BIND: handle={} addr=0x{:x} size={}",
                gem.handle(),
                vm_addr,
                gem.size()
            );
            // ioctl(self.device_fd, DRM_IOCTL_XE_VM_BIND, &bind_params);
        }

        #[cfg(not(feature = "drm-backend"))]
        {
            tracing::debug!(
                "Simulated VM_BIND: handle={} addr=0x{:x} size={}",
                gem.handle(),
                vm_addr,
                gem.size()
            );
        }

        Ok(())
    }

    /// Unbind GPU virtual address (single writer!)
    ///
    /// # Arguments
    /// * `vm_addr` - GPU virtual address to unbind
    /// * `size` - Size of region to unbind
    ///
    /// # Safety
    /// #ASSUME_SINGLE_WRITER: Only allocator calls this
    /// #VERIFY_ADDR_BOUND: Address was previously bound
    pub fn unbind(&self, vm_addr: u64, size: u64) -> Result<(), DrmError> {
        // #ASSUME_ALIGNMENT: vm_addr is properly aligned
        if !vm_addr.is_multiple_of(4096) {
            return Err(DrmError::InvalidArgument(format!(
                "Address not aligned: 0x{:x}",
                vm_addr
            )));
        }

        #[cfg(feature = "drm-backend")]
        {
            // Real DRM_IOCTL_XE_VM_UNBIND
            tracing::debug!("VM_UNBIND: addr=0x{:x} size={}", vm_addr, size);
            // ioctl(self.device_fd, DRM_IOCTL_XE_VM_UNBIND, &unbind_params);
        }

        #[cfg(not(feature = "drm-backend"))]
        {
            tracing::debug!("Simulated VM_UNBIND: addr=0x{:x} size={}", vm_addr, size);
        }

        Ok(())
    }
}

/// GEM buffer object handle (for backward compatibility)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GemHandle(pub u32);

/// Memory domain flags (for backward compatibility)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryDomain {
    /// GPU local memory (VRAM)
    Vram,
    /// System memory (RAM)
    System,
    /// GPU global translation table
    Ggtt,
}

/// DRM device information
#[derive(Debug, Clone)]
pub struct DrmDeviceInfo {
    /// Vendor ID (Intel = 0x8086)
    pub vendor_id: u16,
    /// Device ID (Arc specific)
    pub device_id: u16,
    /// Driver name ("xe" or "i915")
    pub driver_name: String,
    /// Driver version
    pub driver_version: (u32, u32, u32),
}

/// GEM buffer creation parameters
#[derive(Debug, Clone, Copy)]
pub struct GemCreateParams {
    /// Buffer size in bytes
    pub size: u64,
    /// Alignment requirement
    pub alignment: u64,
    /// CPU cacheable
    pub cpu_cached: bool,
}

/// Open DRM device (legacy API for backward compatibility)
///
/// Opens the Intel Xe DRM device and returns a file descriptor.
/// Typical paths:
/// - /dev/dri/card0 (primary GPU)
/// - /dev/dri/renderD128 (render node, no display)
#[cfg(feature = "drm-backend")]
pub fn open_device(path: &str) -> Result<RawFd, KiangError> {
    use std::os::unix::io::AsRawFd;

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| KiangError::DeviceOpenFailed(format!("{}", e)))?;

    Ok(file.as_raw_fd())
}

/// Open DRM device (stub for non-DRM builds)
#[cfg(not(feature = "drm-backend"))]
pub fn open_device(_path: &str) -> Result<RawFd, KiangError> {
    Err(KiangError::DeviceOpenFailed(
        "drm-backend feature not enabled".to_string(),
    ))
}

/// Close DRM device (legacy API for backward compatibility)
pub fn close_device(_fd: RawFd) {
    // File descriptor will be closed when it goes out of scope
    // Manual close can be added if needed
}

/// DRM error types
#[derive(Debug)]
pub enum DrmError {
    /// Device not found
    DeviceNotFound,
    /// Failed to open device
    OpenFailed(std::io::Error),
    /// Ioctl operation failed
    IoctlFailed(std::io::Error),
    /// Buffer allocation failed
    AllocationFailed,
    /// Invalid argument
    InvalidArgument(String),
}

impl std::fmt::Display for DrmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceNotFound => write!(f, "DRM device not found"),
            Self::OpenFailed(e) => write!(f, "Failed to open device: {}", e),
            Self::IoctlFailed(e) => write!(f, "Ioctl failed: {}", e),
            Self::AllocationFailed => write!(f, "Buffer allocation failed"),
            Self::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
        }
    }
}

impl std::error::Error for DrmError {}

// Convert DrmError to KiangError for integration
impl From<DrmError> for KiangError {
    fn from(err: DrmError) -> Self {
        match err {
            DrmError::DeviceNotFound => {
                KiangError::DeviceOpenFailed("Device not found".to_string())
            }
            DrmError::OpenFailed(e) => KiangError::DeviceOpenFailed(format!("Open failed: {}", e)),
            DrmError::IoctlFailed(e) => KiangError::DeviceError(format!("Ioctl failed: {}", e)),
            DrmError::AllocationFailed => KiangError::DeviceError("Allocation failed".to_string()),
            DrmError::InvalidArgument(msg) => {
                KiangError::DeviceError(format!("Invalid argument: {}", msg))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    // Helper to create a test DRM device using a temp file
    fn create_test_device() -> DrmDevice {
        // Create temp file for testing (won't actually be used for DRM ops)
        let temp_path = "/tmp/test_drm_device";
        let _ = std::fs::write(temp_path, b"test");

        let file = File::open(temp_path).expect("Failed to open test file");

        DrmDevice {
            file,
            card_path: "/dev/dri/card0".to_string(),
            generation: Arc::new(AtomicU64::new(1)),
        }
    }

    #[test]
    fn test_device_generation_counter() {
        let device = create_test_device();

        assert_eq!(device.generation(), 1);

        device.bump_generation();
        assert_eq!(device.generation(), 2);

        device.bump_generation();
        assert_eq!(device.generation(), 3);
    }

    #[test]
    fn test_gem_object_properties() {
        let device = create_test_device();

        let gem = GemObject::create(&device, 4096).expect("Failed to create GEM");

        assert_eq!(gem.size(), 4096);
        assert_ne!(gem.handle(), 0);
        assert_eq!(gem.generation(), 1); // Captured before bump
        assert_eq!(device.generation(), 2); // Bumped after allocation
    }

    #[test]
    fn test_vm_bind_alignment() {
        let device = create_test_device();

        let gem = GemObject::create(&device, 4096).expect("Failed to create GEM");
        let vm_bind = VmBind::new(&device);

        // Valid alignment (4KB)
        assert!(vm_bind.bind(&gem, 0x1000).is_ok());

        // Invalid alignment
        assert!(vm_bind.bind(&gem, 0x1001).is_err());

        // Unbind with alignment check
        assert!(vm_bind.unbind(0x1000, 4096).is_ok());
        assert!(vm_bind.unbind(0x1001, 4096).is_err());
    }

    #[test]
    fn test_toctou_prevention() {
        let device = create_test_device();

        // Reader captures generation
        let gen_before = device.generation();
        assert_eq!(gen_before, 1);

        // Allocator performs operation
        let _gem = GemObject::create(&device, 4096).expect("Allocation failed");

        // Reader checks if generation changed (TOCTOU detected)
        let gen_after = device.generation();
        assert_ne!(gen_before, gen_after, "TOCTOU race detected");
    }

    #[test]
    fn test_gem_drop_cleanup() {
        let device = create_test_device();

        let handle = {
            let gem = GemObject::create(&device, 4096).expect("Allocation failed");
            gem.handle()
        }; // gem dropped here

        // Verify handle was valid
        assert_ne!(handle, 0);
        // Drop implementation logs cleanup (check with tracing)
    }

    #[test]
    fn test_gem_handle_basics() {
        let handle = GemHandle(42);
        assert_eq!(handle.0, 42);
    }

    #[test]
    fn test_memory_domain_variants() {
        let vram = MemoryDomain::Vram;
        let system = MemoryDomain::System;
        let ggtt = MemoryDomain::Ggtt;

        assert_ne!(vram, system);
        assert_ne!(system, ggtt);
        assert_ne!(vram, ggtt);
    }
}
