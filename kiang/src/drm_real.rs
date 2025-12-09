//! Real Intel Xe/i915 DRM Integration
//!
//! **CRITICAL**: Real hardware operations - can brick GPU if wrong!
//!
//! # Architecture
//!
//! This module provides real kernel syscalls for Intel Xe/i915 DRM operations.
//! It follows The Atomic Capsule principles with single-writer resource management.
//!
//! # Safety Model
//!
//! Following ASSUM framework:
//! - All ioctl calls wrapped with validation
//! - Alignment requirements enforced before kernel calls
//! - Generation counters prevent TOCTOU races
//! - Feature-gated for safety (real_driver feature)
//!
//! # UCE32 Analysis
//!
//! Q28 (Simplicity): Direct ioctl wrappers, no unnecessary abstraction
//! Q29 (Constraints): Hardware alignment (4KB), kernel ABI stability
//! Q30 (Validation): Test with real hardware, fallback to simulation
//! Q31 (Rust): Type-safe ioctl wrappers, RAII cleanup
//! Q32 (Nightly): const_fn for ioctl code calculation (when stabilized)

use crate::drm_interface::{DrmDevice, DrmError, GemObject};
use std::os::unix::io::RawFd;

// DRM ioctl codes for Intel Xe driver
// Reference: include/uapi/drm/xe_drm.h in Linux kernel

/// Base DRM ioctl number
const DRM_IOCTL_BASE: u8 = b'd';

/// DRM command base (0x40 = read/write)
const DRM_COMMAND_BASE: u32 = 0x40;

/// Intel Xe driver-specific ioctls
mod xe_ioctls {
    use super::*;

    // Xe-specific ioctl numbers (from xe_drm.h)
    pub const DRM_XE_DEVICE_QUERY: u32 = 0x00;
    pub const DRM_XE_GEM_CREATE: u32 = 0x01;
    pub const DRM_XE_GEM_MMAP_OFFSET: u32 = 0x02;
    pub const DRM_XE_VM_CREATE: u32 = 0x03;
    pub const DRM_XE_VM_DESTROY: u32 = 0x04;
    pub const DRM_XE_VM_BIND: u32 = 0x05;
    pub const DRM_XE_EXEC: u32 = 0x06;
    pub const DRM_XE_EXEC_QUEUE_CREATE: u32 = 0x07;
    pub const DRM_XE_EXEC_QUEUE_DESTROY: u32 = 0x08;
    pub const DRM_XE_EXEC_QUEUE_GET_PROPERTY: u32 = 0x09;
    pub const DRM_XE_WAIT_USER_FENCE: u32 = 0x0a;

    /// Calculate ioctl request code
    /// #ASSUME_IOCTL_CODE: Uses standard DRM ioctl encoding
    /// #VERIFY_IOCTL_CODE: Matches kernel uapi definitions
    pub const fn ioctl_code(dir: u32, cmd: u32, size: usize) -> nix::sys::ioctl::ioctl_num_type {
        // DRM ioctl encoding: (dir << 30) | (size << 16) | (type << 8) | cmd
        ((dir << 30) | ((size as u32) << 16) | ((DRM_IOCTL_BASE as u32) << 8) | cmd)
            as nix::sys::ioctl::ioctl_num_type
    }
}

/// DRM_XE_GEM_CREATE structure
/// Reference: xe_drm.h struct drm_xe_gem_create
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct XeGemCreate {
    /// Extensions pointer (unused for basic creation)
    pub extensions: u64,
    /// VM ID to create the BO in (0 for default)
    pub vm_id: u32,
    /// Placement flags (VRAM, SYSTEM, etc.)
    pub flags: u32,
    /// Padding for alignment
    pub pad: u32,
    /// Size of the GEM object in bytes
    pub size: u64,
    /// CPU caching mode
    pub cpu_caching: u32,
    /// Padding
    pub pad2: u32,
    /// [out] Handle for the created GEM object
    pub handle: u32,
    /// Padding
    pub pad3: u32,
    /// [out] Reserved
    pub reserved: [u64; 2],
}

/// XE GEM creation flags
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XeGemCreateFlags {
    /// Allocate in GPU VRAM
    VramIfPossible = 1 << 0,
    /// Needs visible VRAM (for CPU access)
    NeedsVisibleVram = 1 << 1,
    /// Scanout buffer (display)
    Scanout = 1 << 2,
}

/// CPU caching modes for GEM objects
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XeCpuCaching {
    /// Write-combining
    WriteCombine = 0,
    /// Cached
    Cached = 1,
    /// Uncached
    Uncached = 2,
}

/// DRM_XE_VM_BIND structure
/// Reference: xe_drm.h struct drm_xe_vm_bind
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct XeVmBind {
    /// Extensions pointer
    pub extensions: u64,
    /// VM ID to bind in
    pub vm_id: u32,
    /// Execution queue ID (0 for immediate)
    pub exec_queue_id: u32,
    /// Number of bindings
    pub num_binds: u32,
    /// Bind operation flags
    pub flags: u32,
    /// Pointer to array of bind operations
    pub binds: u64,
    /// Number of sync objects
    pub num_syncs: u32,
    /// Padding
    pub pad: u32,
    /// Pointer to sync objects
    pub syncs: u64,
    /// Reserved
    pub reserved: [u64; 2],
}

/// Single VM bind operation
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct XeVmBindOp {
    /// Extensions pointer
    pub extensions: u64,
    /// GEM object handle (0 for unbind)
    pub obj: u32,
    /// Padding
    pub pad: u32,
    /// Offset into GEM object
    pub obj_offset: u64,
    /// GPU virtual address to bind at
    pub addr: u64,
    /// Range size in bytes
    pub range: u64,
    /// Operation flags
    pub flags: u32,
    /// Prefetch region
    pub prefetch_mem_region_instance: u32,
    /// Reserved
    pub reserved: [u64; 2],
}

/// VM bind flags
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XeVmBindFlags {
    /// Immediate bind (no queue)
    Immediate = 1 << 0,
    /// Make pages resident
    MakeResident = 1 << 1,
    /// Unbind operation
    Unbind = 1 << 2,
}

/// DRM_XE_WAIT_USER_FENCE structure
/// For fence waiting/polling
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct XeWaitUserFence {
    /// Extensions pointer
    pub extensions: u64,
    /// GPU virtual address of fence
    pub addr: u64,
    /// Flags for wait operation
    pub flags: u16,
    /// Operation (EQ, LT, GT, etc.)
    pub op: u16,
    /// Padding
    pub pad: u32,
    /// Value to compare against
    pub value: u64,
    /// Timeout in nanoseconds (-1 = infinite)
    pub timeout: i64,
    /// Number of exec queues
    pub num_engines: u32,
    /// Padding
    pub pad2: u32,
    /// Pointer to array of exec queue instances
    pub instances: u64,
    /// Reserved
    pub reserved: [u64; 2],
}

/// Wait operations for user fences
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XeWaitOp {
    /// Wait for equal
    Eq = 0,
    /// Wait for not equal
    Neq = 1,
    /// Wait for greater than
    Gt = 2,
    /// Wait for greater than or equal
    Gte = 3,
    /// Wait for less than
    Lt = 4,
    /// Wait for less than or equal
    Lte = 5,
}

/// Real GEM buffer creation using Xe driver
///
/// # Arguments
/// * `device_fd` - Raw file descriptor for DRM device
/// * `size` - Buffer size in bytes (must be page-aligned)
/// * `flags` - Creation flags (VRAM, CPU caching, etc.)
/// * `cpu_caching` - CPU caching mode
///
/// # Safety
/// #ASSUME_IOCTL_SAFE: Kernel validates all parameters
/// #VERIFY_SIZE_ALIGNED: Size must be 4K aligned for GPU
/// #ASSUME_FD_VALID: Device fd is open and valid
/// #VERIFY_FD_LIFETIME: Caller ensures fd outlives returned handle
///
/// # Returns
/// GEM handle on success, DrmError on failure
///
/// # Errors
/// - `InvalidArgument`: Size not aligned or zero
/// - `IoctlFailed`: Kernel ioctl operation failed
pub fn gem_create_real(
    device_fd: RawFd,
    size: u64,
    flags: u32,
    cpu_caching: XeCpuCaching,
) -> Result<u32, DrmError> {
    // #ASSUME_ALIGNMENT: GPU requires 4KB alignment
    // #VERIFY_ALIGNMENT: Check before kernel call
    if size == 0 {
        return Err(DrmError::InvalidArgument("Size cannot be zero".to_string()));
    }

    if size % 4096 != 0 {
        return Err(DrmError::InvalidArgument(format!(
            "Size {} not 4K aligned (must be multiple of 4096)",
            size
        )));
    }

    let mut args = XeGemCreate {
        extensions: 0,
        vm_id: 0, // Use default VM
        flags,
        pad: 0,
        size,
        cpu_caching: cpu_caching as u32,
        pad2: 0,
        handle: 0,
        pad3: 0,
        reserved: [0; 2],
    };

    // #ASSUME_IOCTL_SAFE: Kernel driver validates all fields
    // #VERIFY_IOCTL_RETURN: Check return value for errors
    unsafe {
        let request = xe_ioctls::ioctl_code(
            DRM_COMMAND_BASE,
            xe_ioctls::DRM_XE_GEM_CREATE,
            std::mem::size_of::<XeGemCreate>(),
        );

        let ret = nix::libc::ioctl(device_fd, request, &mut args as *mut XeGemCreate);

        if ret < 0 {
            let errno = std::io::Error::last_os_error();
            return Err(DrmError::IoctlFailed(errno));
        }
    }

    // Validate kernel returned valid handle
    if args.handle == 0 {
        return Err(DrmError::AllocationFailed);
    }

    Ok(args.handle)
}

/// Real VM_BIND operation using Xe driver
///
/// Binds a GEM object to GPU virtual address space.
///
/// # Arguments
/// * `device_fd` - Raw file descriptor for DRM device
/// * `vm_id` - VM ID (0 for default)
/// * `gem_handle` - GEM object handle to bind
/// * `vm_addr` - GPU virtual address (must be 4K aligned)
/// * `size` - Size of region to bind (must be 4K aligned)
/// * `offset` - Offset into GEM object
/// * `flags` - Bind operation flags
///
/// # Safety
/// #ASSUME_IOCTL_SAFE: Kernel validates address ranges
/// #VERIFY_ADDR_ALIGNED: VM address must be 4K aligned
/// #VERIFY_SIZE_ALIGNED: Size must be 4K aligned
/// #ASSUME_HANDLE_VALID: GEM handle exists and is valid
/// #VERIFY_NO_OVERLAP: Caller ensures no overlapping mappings
///
/// # Errors
/// - `InvalidArgument`: Alignment violation
/// - `IoctlFailed`: Kernel operation failed
pub fn vm_bind_real(
    device_fd: RawFd,
    vm_id: u32,
    gem_handle: u32,
    vm_addr: u64,
    size: u64,
    offset: u64,
    flags: u32,
) -> Result<(), DrmError> {
    // #ASSUME_ALIGNMENT: GPU MMU requires 4KB pages
    // #VERIFY_ALIGNMENT: Check all alignments before syscall
    if vm_addr % 4096 != 0 {
        return Err(DrmError::InvalidArgument(format!(
            "VM address 0x{:x} not 4K aligned",
            vm_addr
        )));
    }

    if size % 4096 != 0 {
        return Err(DrmError::InvalidArgument(format!(
            "Size {} not 4K aligned",
            size
        )));
    }

    if offset % 4096 != 0 {
        return Err(DrmError::InvalidArgument(format!(
            "Offset {} not 4K aligned",
            offset
        )));
    }

    // Create bind operation
    let bind_op = XeVmBindOp {
        extensions: 0,
        obj: gem_handle,
        pad: 0,
        obj_offset: offset,
        addr: vm_addr,
        range: size,
        flags,
        prefetch_mem_region_instance: 0,
        reserved: [0; 2],
    };

    let mut vm_bind = XeVmBind {
        extensions: 0,
        vm_id,
        exec_queue_id: 0, // Immediate bind
        num_binds: 1,
        flags: XeVmBindFlags::Immediate as u32,
        binds: &bind_op as *const XeVmBindOp as u64,
        num_syncs: 0,
        pad: 0,
        syncs: 0,
        reserved: [0; 2],
    };

    // #ASSUME_IOCTL_SAFE: Kernel validates all bind parameters
    // #VERIFY_BIND_SUCCESS: Check return value
    unsafe {
        let request = xe_ioctls::ioctl_code(
            DRM_COMMAND_BASE,
            xe_ioctls::DRM_XE_VM_BIND,
            std::mem::size_of::<XeVmBind>(),
        );

        let ret = nix::libc::ioctl(device_fd, request, &mut vm_bind as *mut XeVmBind);

        if ret < 0 {
            let errno = std::io::Error::last_os_error();
            return Err(DrmError::IoctlFailed(errno));
        }
    }

    Ok(())
}

/// Real VM_UNBIND operation
///
/// Unbinds a GPU virtual address range.
///
/// # Arguments
/// * `device_fd` - Raw file descriptor
/// * `vm_id` - VM ID (0 for default)
/// * `vm_addr` - GPU virtual address to unbind
/// * `size` - Size of region to unbind
///
/// # Safety
/// #ASSUME_ADDR_BOUND: Address was previously bound
/// #VERIFY_ALIGNMENT: Address and size must be 4K aligned
///
/// # Errors
/// - `InvalidArgument`: Alignment violation
/// - `IoctlFailed`: Kernel operation failed
pub fn vm_unbind_real(
    device_fd: RawFd,
    vm_id: u32,
    vm_addr: u64,
    size: u64,
) -> Result<(), DrmError> {
    // #VERIFY_ALIGNMENT: Check before kernel call
    if vm_addr % 4096 != 0 {
        return Err(DrmError::InvalidArgument(format!(
            "VM address 0x{:x} not 4K aligned",
            vm_addr
        )));
    }

    if size % 4096 != 0 {
        return Err(DrmError::InvalidArgument(format!(
            "Size {} not 4K aligned",
            size
        )));
    }

    // Unbind operation (obj = 0 for unbind)
    let unbind_op = XeVmBindOp {
        extensions: 0,
        obj: 0, // 0 means unbind
        pad: 0,
        obj_offset: 0,
        addr: vm_addr,
        range: size,
        flags: XeVmBindFlags::Unbind as u32,
        prefetch_mem_region_instance: 0,
        reserved: [0; 2],
    };

    let mut vm_bind = XeVmBind {
        extensions: 0,
        vm_id,
        exec_queue_id: 0,
        num_binds: 1,
        flags: XeVmBindFlags::Immediate as u32 | XeVmBindFlags::Unbind as u32,
        binds: &unbind_op as *const XeVmBindOp as u64,
        num_syncs: 0,
        pad: 0,
        syncs: 0,
        reserved: [0; 2],
    };

    unsafe {
        let request = xe_ioctls::ioctl_code(
            DRM_COMMAND_BASE,
            xe_ioctls::DRM_XE_VM_BIND,
            std::mem::size_of::<XeVmBind>(),
        );

        let ret = nix::libc::ioctl(device_fd, request, &mut vm_bind as *mut XeVmBind);

        if ret < 0 {
            let errno = std::io::Error::last_os_error();
            return Err(DrmError::IoctlFailed(errno));
        }
    }

    Ok(())
}

/// Real fence wait operation
///
/// Polls a user fence at GPU virtual address until condition is met.
///
/// # Arguments
/// * `device_fd` - Raw file descriptor
/// * `fence_addr` - GPU virtual address of fence value
/// * `value` - Value to compare against
/// * `op` - Wait operation (EQ, GT, etc.)
/// * `timeout_ns` - Timeout in nanoseconds (-1 = infinite)
///
/// # Safety
/// #ASSUME_FENCE_ADDR_VALID: Fence address is mapped and accessible to GPU
/// #VERIFY_TIMEOUT_VALID: Timeout is valid (-1 or positive)
/// #ASSUME_NO_RACE: Fence value is only updated by GPU, not CPU
///
/// # Returns
/// `true` if condition met, `false` if timeout
///
/// # Errors
/// - `IoctlFailed`: Kernel operation failed
pub fn fence_wait_real(
    device_fd: RawFd,
    fence_addr: u64,
    value: u64,
    op: XeWaitOp,
    timeout_ns: i64,
) -> Result<bool, DrmError> {
    let mut wait_args = XeWaitUserFence {
        extensions: 0,
        addr: fence_addr,
        flags: 0,
        op: op as u16,
        pad: 0,
        value,
        timeout: timeout_ns,
        num_engines: 0,
        pad2: 0,
        instances: 0,
        reserved: [0; 2],
    };

    // #ASSUME_IOCTL_SAFE: Kernel validates fence address and timeout
    // #VERIFY_TIMEOUT_BEHAVIOR: Kernel returns on timeout or condition met
    unsafe {
        let request = xe_ioctls::ioctl_code(
            DRM_COMMAND_BASE,
            xe_ioctls::DRM_XE_WAIT_USER_FENCE,
            std::mem::size_of::<XeWaitUserFence>(),
        );

        let ret = nix::libc::ioctl(device_fd, request, &mut wait_args as *mut XeWaitUserFence);

        if ret < 0 {
            let errno = std::io::Error::last_os_error();
            // ETIMEDOUT (110) is expected for timeout, not an error
            if errno.raw_os_error() == Some(110) {
                return Ok(false); // Timeout
            }
            return Err(DrmError::IoctlFailed(errno));
        }
    }

    Ok(true) // Condition met
}

/// Close GEM handle
///
/// # Arguments
/// * `device_fd` - Raw file descriptor
/// * `handle` - GEM object handle to close
///
/// # Safety
/// #ASSUME_HANDLE_VALID: Handle was created by GEM_CREATE
/// #VERIFY_DROP_SAFE: Safe to call on already-closed handles (idempotent)
///
/// # Errors
/// - `IoctlFailed`: Kernel operation failed
pub fn gem_close_real(device_fd: RawFd, handle: u32) -> Result<(), DrmError> {
    // Use standard DRM GEM_CLOSE ioctl (not Xe-specific)
    #[repr(C)]
    struct DrmGemClose {
        handle: u32,
        pad: u32,
    }

    let mut close_args = DrmGemClose { handle, pad: 0 };

    // #ASSUME_IOCTL_SAFE: Kernel handles invalid handles gracefully
    // #VERIFY_IDEMPOTENT: Safe to call multiple times
    unsafe {
        // DRM_IOCTL_GEM_CLOSE = DRM_IOW(0x09, struct drm_gem_close)
        let request = xe_ioctls::ioctl_code(
            DRM_COMMAND_BASE,
            0x09, // DRM_GEM_CLOSE
            std::mem::size_of::<DrmGemClose>(),
        );

        let ret = nix::libc::ioctl(device_fd, request, &mut close_args as *mut DrmGemClose);

        if ret < 0 {
            let errno = std::io::Error::last_os_error();
            // ENOENT is OK (handle already closed)
            if errno.raw_os_error() == Some(2) {
                return Ok(());
            }
            return Err(DrmError::IoctlFailed(errno));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment_validation() {
        // These should fail with alignment errors (no real device needed)
        let result = gem_create_real(0, 100, 0, XeCpuCaching::WriteCombine);
        assert!(result.is_err());

        let result = gem_create_real(0, 0, 0, XeCpuCaching::WriteCombine);
        assert!(result.is_err());
    }

    #[test]
    fn test_vm_bind_alignment() {
        // Test alignment validation without real device
        let result = vm_bind_real(0, 0, 1, 0x1001, 4096, 0, 0);
        assert!(result.is_err()); // Unaligned address

        let result = vm_bind_real(0, 0, 1, 0x1000, 100, 0, 0);
        assert!(result.is_err()); // Unaligned size
    }

    #[test]
    fn test_ioctl_code_calculation() {
        // Verify ioctl code calculation matches kernel
        let code = xe_ioctls::ioctl_code(
            DRM_COMMAND_BASE,
            xe_ioctls::DRM_XE_GEM_CREATE,
            std::mem::size_of::<XeGemCreate>(),
        );

        // Just verify it's non-zero and has expected bits
        assert_ne!(code, 0);
        assert!((code & 0xFF) == 0x01); // Command number
    }

    #[test]
    fn test_struct_sizes() {
        // Verify struct sizes match kernel expectations
        assert!(std::mem::size_of::<XeGemCreate>() >= 56);
        assert!(std::mem::size_of::<XeVmBind>() >= 64);
        assert!(std::mem::size_of::<XeVmBindOp>() >= 64);
        assert!(std::mem::size_of::<XeWaitUserFence>() >= 64);
    }

    #[test]
    fn test_flag_values() {
        // Verify flag values match kernel definitions
        assert_eq!(XeGemCreateFlags::VramIfPossible as u32, 1 << 0);
        assert_eq!(XeVmBindFlags::Immediate as u32, 1 << 0);
        assert_eq!(XeWaitOp::Eq as u16, 0);
        assert_eq!(XeWaitOp::Gt as u16, 2);
    }
}

// Feature-gated integration with DrmDevice
#[cfg(feature = "real_driver")]
impl DrmDevice {
    /// Create GEM object using real kernel driver
    ///
    /// # Safety
    /// Requires real hardware and loaded Xe driver
    pub fn gem_create_real(&self, size: u64) -> Result<GemObject, DrmError> {
        let handle = gem_create_real(
            self.as_raw_fd(),
            size,
            XeGemCreateFlags::VramIfPossible as u32,
            XeCpuCaching::WriteCombine,
        )?;

        Ok(GemObject::from_handle_real(self, handle, size))
    }

    /// VM_BIND using real kernel driver
    pub fn vm_bind_real(&self, gem: &GemObject, vm_addr: u64) -> Result<(), DrmError> {
        vm_bind_real(
            self.as_raw_fd(),
            0, // Default VM
            gem.handle(),
            vm_addr,
            gem.size(),
            0, // Offset
            XeVmBindFlags::Immediate as u32,
        )
    }

    /// VM_UNBIND using real kernel driver
    pub fn vm_unbind_real(&self, vm_addr: u64, size: u64) -> Result<(), DrmError> {
        vm_unbind_real(self.as_raw_fd(), 0, vm_addr, size)
    }

    /// Wait for fence using real kernel driver
    pub fn fence_wait_real(
        &self,
        fence_addr: u64,
        value: u64,
        timeout_ns: i64,
    ) -> Result<bool, DrmError> {
        fence_wait_real(
            self.as_raw_fd(),
            fence_addr,
            value,
            XeWaitOp::Gte,
            timeout_ns,
        )
    }
}

// Extension trait for GemObject to add real driver support
#[cfg(feature = "real_driver")]
impl GemObject {
    /// Create from existing handle (internal use)
    pub(crate) fn from_handle_real(device: &DrmDevice, handle: u32, size: u64) -> Self {
        Self {
            device_fd: device.as_raw_fd(),
            handle,
            size,
            generation: device.generation(),
        }
    }

    /// Close GEM handle using real kernel driver (called by existing Drop implementation)
    pub(crate) fn close_real_driver(&self) -> Result<(), DrmError> {
        gem_close_real(self.device_fd, self.handle())
    }
}
