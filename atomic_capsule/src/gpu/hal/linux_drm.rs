//! Linux DRM/GEM Buffer Management
//!
//! Implements DRM and GEM operations for Intel GPUs via ioctl.
//! Connects to DmaBufferCapsule for lockfree buffer lifecycle tracking.
//!
//! # Design
//!
//! **Tier**: T1 Atomic (lockfree coordination) + T8 Network (kernel IPC via ioctl)
//! **Portability**: Linux-only (feature-gated: `linux-gpu`)
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    DRM/GEM Operations                            │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │  ┌──────────────────────────────────────────────────────────┐  │
//! │  │              DrmDevice                                    │  │
//! │  │  /dev/dri/cardN or /dev/dri/renderD128                   │  │
//! │  │  ioctl(DRM_IOCTL_*)                                      │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! │                           │                                      │
//! │                           ▼                                      │
//! │  ┌──────────────────────────────────────────────────────────┐  │
//! │  │              GEM Buffer Operations                        │  │
//! │  │  - gem_create: Allocate GPU buffer                       │  │
//! │  │  - gem_close: Release GPU buffer                         │  │
//! │  │  - gem_mmap: Map buffer for CPU access                   │  │
//! │  │  - gem_set_domain: Cache coherency transitions           │  │
//! │  │  - gem_wait: Wait for GPU completion                     │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! │                           │                                      │
//! │                           ▼                                      │
//! │  ┌──────────────────────────────────────────────────────────┐  │
//! │  │           DmaBufferCapsule                                │  │
//! │  │  Lockfree buffer lifecycle + fence tracking              │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # i915 DRM IOCTLs
//!
//! Key IOCTLs used:
//! - `DRM_IOCTL_I915_GEM_CREATE`: Create GEM buffer object
//! - `DRM_IOCTL_GEM_CLOSE`: Close GEM handle
//! - `DRM_IOCTL_I915_GEM_MMAP_GTT`: Map GEM BO via GTT for CPU access
//! - `DRM_IOCTL_I915_GEM_SET_DOMAIN`: Set buffer domain for cache coherency
//! - `DRM_IOCTL_I915_GEM_WAIT`: Wait for buffer to become idle
//! - `DRM_IOCTL_I915_GEM_EXECBUFFER2`: Submit GPU commands
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_DRM_FD_VALID`: DRM file descriptor valid after successful open
//! - `#ASSUME_IOCTL_SERIALIZED`: Kernel serializes ioctl calls (no user-space races)
//! - `#ASSUME_GEM_HANDLE_VALID`: GEM handle valid until gem_close() called
//! - `#ASSUME_GTT_MAPPING_VALID`: GTT mapping valid while GEM handle open
//!
//! # Examples
//!
//! ```ignore
//! use atomic_capsule::gpu::hal::linux_drm::{DrmDevice, GemBuffer};
//!
//! // Open DRM device
//! let drm = DrmDevice::open_render()?;
//!
//! // Check capabilities
//! let caps = drm.capabilities()?;
//! println!("Supports PRIME: export={}, import={}", caps.prime_export, caps.prime_import);
//!
//! // Create GEM buffer
//! let buffer = GemBuffer::create(&drm, 4096)?;
//! println!("Created buffer: handle={}, size={}", buffer.handle.raw(), buffer.size);
//!
//! // Map for CPU access
//! let ptr = buffer.mmap()?;
//!
//! // Write data
//! unsafe { ptr::write(ptr as *mut u32, 0xDEADBEEF); }
//!
//! // Set domain for GPU access
//! buffer.set_domain(GemDomain::Render as u32, GemDomain::Render as u32)?;
//!
//! // Wait for GPU completion
//! buffer.wait(1_000_000_000)?; // 1 second timeout
//!
//! // Cleanup (automatic via Drop)
//! ```

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicPtr, Ordering};
use core::ptr;

use super::linux_hal::{
    LinuxHalError, LinuxHalResult, LinuxHalState,
    LinuxDrmAccess, LinuxGemAccess,
    DrmVersion, DrmCapabilities, GemHandle, GemDomain,
};

// ============================================================================
// DRM IOCTL Definitions
// ============================================================================

/// DRM ioctl command builder
const fn drm_io(nr: u32) -> u64 {
    // _IO('d', nr)
    ((b'd' as u64) << 8) | (nr as u64)
}

const fn drm_ior(nr: u32, size: u32) -> u64 {
    // _IOR('d', nr, size)
    (2u64 << 30) | ((size as u64) << 16) | ((b'd' as u64) << 8) | (nr as u64)
}

const fn drm_iow(nr: u32, size: u32) -> u64 {
    // _IOW('d', nr, size)
    (1u64 << 30) | ((size as u64) << 16) | ((b'd' as u64) << 8) | (nr as u64)
}

const fn drm_iowr(nr: u32, size: u32) -> u64 {
    // _IOWR('d', nr, size)
    (3u64 << 30) | ((size as u64) << 16) | ((b'd' as u64) << 8) | (nr as u64)
}

// DRM base IOCTLs
const DRM_IOCTL_VERSION: u64 = drm_iowr(0x00, 36);
const DRM_IOCTL_GET_CAP: u64 = drm_iowr(0x0c, 16);
const DRM_IOCTL_GEM_CLOSE: u64 = drm_iow(0x09, 8);

// i915-specific IOCTLs (base 0x40)
const DRM_I915_GEM_CREATE: u32 = 0x1b;
const DRM_I915_GEM_MMAP_GTT: u32 = 0x24;
const DRM_I915_GEM_SET_DOMAIN: u32 = 0x1f;
const DRM_I915_GEM_WAIT: u32 = 0x2c;
const DRM_I915_GEM_CREATE_EXT: u32 = 0x3d;

const DRM_IOCTL_I915_GEM_CREATE: u64 = drm_iowr(0x40 + DRM_I915_GEM_CREATE, 16);
const DRM_IOCTL_I915_GEM_MMAP_GTT: u64 = drm_iowr(0x40 + DRM_I915_GEM_MMAP_GTT, 16);
const DRM_IOCTL_I915_GEM_SET_DOMAIN: u64 = drm_iow(0x40 + DRM_I915_GEM_SET_DOMAIN, 16);
const DRM_IOCTL_I915_GEM_WAIT: u64 = drm_iowr(0x40 + DRM_I915_GEM_WAIT, 16);

// DRM capabilities
const DRM_CAP_DUMB_BUFFER: u64 = 0x1;
const DRM_CAP_VBLANK_HIGH_CRTC: u64 = 0x2;
const DRM_CAP_PRIME: u64 = 0x5;
const DRM_CAP_TIMESTAMP_MONOTONIC: u64 = 0x6;
const DRM_CAP_ASYNC_PAGE_FLIP: u64 = 0x7;
const DRM_CAP_CURSOR_WIDTH: u64 = 0x8;
const DRM_CAP_CURSOR_HEIGHT: u64 = 0x9;
const DRM_CAP_SYNCOBJ: u64 = 0x13;
const DRM_CAP_SYNCOBJ_TIMELINE: u64 = 0x14;

// PRIME capability flags
const DRM_PRIME_CAP_IMPORT: u64 = 0x1;
const DRM_PRIME_CAP_EXPORT: u64 = 0x2;

// ============================================================================
// IOCTL Argument Structures
// ============================================================================

/// DRM version ioctl argument
#[repr(C)]
#[derive(Debug, Default)]
struct DrmVersionArg {
    version_major: i32,
    version_minor: i32,
    version_patchlevel: i32,
    name_len: u64,
    name: *mut u8,
    date_len: u64,
    date: *mut u8,
    desc_len: u64,
    desc: *mut u8,
}

/// DRM get capability ioctl argument
#[repr(C)]
#[derive(Debug, Default)]
struct DrmGetCapArg {
    capability: u64,
    value: u64,
}

/// GEM close ioctl argument
#[repr(C)]
#[derive(Debug, Default)]
struct DrmGemCloseArg {
    handle: u32,
    pad: u32,
}

/// i915 GEM create ioctl argument
#[repr(C)]
#[derive(Debug, Default)]
struct I915GemCreateArg {
    size: u64,
    handle: u32,
    pad: u32,
}

/// i915 GEM mmap GTT ioctl argument
#[repr(C)]
#[derive(Debug, Default)]
struct I915GemMmapGttArg {
    handle: u32,
    pad: u32,
    offset: u64,
}

/// i915 GEM set domain ioctl argument
#[repr(C)]
#[derive(Debug, Default)]
struct I915GemSetDomainArg {
    handle: u32,
    read_domains: u32,
    write_domain: u32,
}

/// i915 GEM wait ioctl argument
#[repr(C)]
#[derive(Debug, Default)]
struct I915GemWaitArg {
    bo_handle: u32,
    flags: u32,
    timeout_ns: i64,
}

// ============================================================================
// DRM Device
// ============================================================================

/// DRM device wrapper
///
/// Provides access to DRM/KMS functionality via ioctl.
/// Thread-safe via atomic state tracking.
///
/// # Memory Layout (128B, 128B-aligned)
#[repr(C, align(128))]
pub struct DrmDevice {
    /// DRM file descriptor
    fd: AtomicU32,
    /// Node type (0=card, 1=render)
    node_type: AtomicU32,
    /// DRM version (packed: major.minor.patch)
    version: AtomicU64,
    /// Capability flags (cached)
    capabilities: AtomicU64,
    /// State flags (bit 0: open, bit 1: authenticated)
    flags: AtomicU32,
    /// Generation counter
    gen_counter: AtomicU32,
    /// Active GEM handle count
    active_handles: AtomicU64,
    /// Shared state pointer
    state: AtomicPtr<LinuxHalState>,
    /// Padding to 128B
    _padding: [u8; 72],
}

// SAFETY: DrmDevice uses atomic operations for all shared state
unsafe impl Send for DrmDevice {}
unsafe impl Sync for DrmDevice {}

impl DrmDevice {
    /// Invalid file descriptor sentinel
    const INVALID_FD: u32 = u32::MAX;

    /// Flag: Device is open
    const FLAG_OPEN: u32 = 0x01;
    /// Flag: Authenticated (DRM master)
    const FLAG_AUTHENTICATED: u32 = 0x02;

    /// Create uninitialized device
    #[inline]
    pub const fn uninit() -> Self {
        Self {
            fd: AtomicU32::new(Self::INVALID_FD),
            node_type: AtomicU32::new(0),
            version: AtomicU64::new(0),
            capabilities: AtomicU64::new(0),
            flags: AtomicU32::new(0),
            gen_counter: AtomicU32::new(0),
            active_handles: AtomicU64::new(0),
            state: AtomicPtr::new(ptr::null_mut()),
            _padding: [0u8; 72],
        }
    }

    /// Open DRM card device (/dev/dri/cardN)
    ///
    /// Requires DRM master (root or seat membership).
    #[cfg(feature = "std")]
    pub fn open_card(card_number: u32) -> LinuxHalResult<Self> {
        Self::open_device(&format!("/dev/dri/card{}", card_number), 0)
    }

    /// Open DRM render device (/dev/dri/renderD128+)
    ///
    /// Does not require DRM master, suitable for compute.
    #[cfg(feature = "std")]
    pub fn open_render() -> LinuxHalResult<Self> {
        // Try render nodes D128-D191
        for i in 128..192 {
            let path = format!("/dev/dri/renderD{}", i);
            if std::path::Path::new(&path).exists() {
                return Self::open_device(&path, 1);
            }
        }
        Err(LinuxHalError::DeviceNotFound)
    }

    /// Open DRM device by path
    #[cfg(feature = "std")]
    fn open_device(path: &str, node_type: u32) -> LinuxHalResult<Self> {
        use std::ffi::CString;

        let c_path = CString::new(path).map_err(|_| LinuxHalError::InvalidDevicePath)?;

        // SAFETY: c_path is a valid null-terminated string
        let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };

        if fd < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(match errno {
                libc::ENOENT => LinuxHalError::DeviceNotFound,
                libc::EACCES | libc::EPERM => LinuxHalError::PermissionDenied,
                _ => LinuxHalError::IoctlFailed(errno),
            });
        }

        let mut device = Self {
            fd: AtomicU32::new(fd as u32),
            node_type: AtomicU32::new(node_type),
            version: AtomicU64::new(0),
            capabilities: AtomicU64::new(0),
            flags: AtomicU32::new(Self::FLAG_OPEN),
            gen_counter: AtomicU32::new(1),
            active_handles: AtomicU64::new(0),
            state: AtomicPtr::new(ptr::null_mut()),
            _padding: [0u8; 72],
        };

        // Query version
        if let Ok(version) = device.query_version() {
            let packed = ((version.major as u64) << 32)
                | ((version.minor as u64) << 16)
                | (version.patch as u64);
            device.version.store(packed, Ordering::Release);
        }

        Ok(device)
    }

    /// Query DRM version
    fn query_version(&self) -> LinuxHalResult<DrmVersion> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let mut arg = DrmVersionArg::default();

        // SAFETY: fd is valid, arg is properly initialized
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_VERSION, &mut arg as *mut _) };

        if ret < 0 {
            return Err(LinuxHalError::VersionQueryFailed);
        }

        Ok(DrmVersion::new(
            arg.version_major as u32,
            arg.version_minor as u32,
            arg.version_patchlevel as u32,
        ))
    }

    /// Query single capability
    fn query_cap(&self, cap: u64) -> LinuxHalResult<u64> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let mut arg = DrmGetCapArg {
            capability: cap,
            value: 0,
        };

        // SAFETY: fd is valid, arg is properly initialized
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_GET_CAP, &mut arg as *mut _) };

        if ret < 0 {
            return Ok(0); // Capability not supported
        }

        Ok(arg.value)
    }

    /// Close device
    #[cfg(feature = "std")]
    pub fn close(&self) -> LinuxHalResult<()> {
        let fd = self.fd.swap(Self::INVALID_FD, Ordering::AcqRel);
        if fd != Self::INVALID_FD {
            // SAFETY: fd is valid
            unsafe { libc::close(fd as i32); }
        }
        self.flags.store(0, Ordering::Release);
        self.gen_counter.fetch_add(1, Ordering::Release);
        Ok(())
    }

    /// Check if device is open
    #[inline]
    pub fn is_open(&self) -> bool {
        (self.flags.load(Ordering::Acquire) & Self::FLAG_OPEN) != 0
    }

    /// Get file descriptor
    #[inline]
    pub fn fd(&self) -> Option<i32> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            None
        } else {
            Some(fd as i32)
        }
    }

    /// Get active GEM handle count
    #[inline]
    pub fn active_handles(&self) -> u64 {
        self.active_handles.load(Ordering::Relaxed)
    }

    /// Increment active handle count
    #[inline]
    fn increment_handles(&self) {
        self.active_handles.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active handle count
    #[inline]
    fn decrement_handles(&self) {
        self.active_handles.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Drop for DrmDevice {
    fn drop(&mut self) {
        #[cfg(feature = "std")]
        {
            let _ = self.close();
        }
    }
}

// ============================================================================
// LinuxDrmAccess Implementation
// ============================================================================

#[cfg(feature = "std")]
impl LinuxDrmAccess for DrmDevice {
    fn open_device(path: &str) -> LinuxHalResult<Self> {
        Self::open_device(path, 0)
    }

    fn close_device(&self) -> LinuxHalResult<()> {
        self.close()
    }

    fn get_version(&self) -> LinuxHalResult<DrmVersion> {
        let packed = self.version.load(Ordering::Acquire);
        if packed == 0 {
            self.query_version()
        } else {
            Ok(DrmVersion::new(
                (packed >> 32) as u32,
                ((packed >> 16) & 0xFFFF) as u32,
                (packed & 0xFFFF) as u32,
            ))
        }
    }

    fn get_capabilities(&self) -> LinuxHalResult<DrmCapabilities> {
        let dumb = self.query_cap(DRM_CAP_DUMB_BUFFER)? != 0;
        let vblank = self.query_cap(DRM_CAP_VBLANK_HIGH_CRTC)? != 0;
        let async_flip = self.query_cap(DRM_CAP_ASYNC_PAGE_FLIP)? != 0;
        let cursor_w = self.query_cap(DRM_CAP_CURSOR_WIDTH)?.max(64) as u32;
        let cursor_h = self.query_cap(DRM_CAP_CURSOR_HEIGHT)?.max(64) as u32;
        let prime = self.query_cap(DRM_CAP_PRIME)?;
        let timestamp = self.query_cap(DRM_CAP_TIMESTAMP_MONOTONIC)? != 0;
        let syncobj_timeline = self.query_cap(DRM_CAP_SYNCOBJ_TIMELINE)? != 0;

        Ok(DrmCapabilities {
            dumb_buffer: dumb,
            vblank_high_crtc: vblank,
            async_page_flip: async_flip,
            cursor_width: cursor_w,
            cursor_height: cursor_h,
            prime_export: (prime & DRM_PRIME_CAP_EXPORT) != 0,
            prime_import: (prime & DRM_PRIME_CAP_IMPORT) != 0,
            timestamp_monotonic: timestamp,
            atomic_async_page_flip: async_flip, // Same for now
            syncobj_timeline,
        })
    }

    unsafe fn ioctl_raw(&self, request: u64, arg: *mut u8) -> LinuxHalResult<()> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let ret = libc::ioctl(fd as i32, request, arg);
        if ret < 0 {
            let errno = *libc::__errno_location();
            return Err(LinuxHalError::IoctlFailed(errno));
        }

        Ok(())
    }
}

// ============================================================================
// LinuxGemAccess Implementation
// ============================================================================

#[cfg(feature = "std")]
impl LinuxGemAccess for DrmDevice {
    fn gem_create(&self, size: u64) -> LinuxHalResult<GemHandle> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        // Page-align size
        let aligned_size = (size + 4095) & !4095;

        let mut arg = I915GemCreateArg {
            size: aligned_size,
            handle: 0,
            pad: 0,
        };

        // SAFETY: fd is valid, arg is properly initialized
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_I915_GEM_CREATE, &mut arg as *mut _) };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(LinuxHalError::GemCreateFailed(errno));
        }

        self.increment_handles();
        Ok(GemHandle::new(arg.handle))
    }

    fn gem_close(&self, handle: GemHandle) -> LinuxHalResult<()> {
        if !handle.is_valid() {
            return Err(LinuxHalError::InvalidGemHandle(handle.raw()));
        }

        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let mut arg = DrmGemCloseArg {
            handle: handle.raw(),
            pad: 0,
        };

        // SAFETY: fd is valid, arg is properly initialized
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_GEM_CLOSE, &mut arg as *mut _) };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(LinuxHalError::GemCloseFailed(errno));
        }

        self.decrement_handles();
        Ok(())
    }

    fn gem_mmap(&self, handle: GemHandle, _offset: u64, size: u64) -> LinuxHalResult<*mut u8> {
        if !handle.is_valid() {
            return Err(LinuxHalError::InvalidGemHandle(handle.raw()));
        }

        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        // Get GTT offset for mmap
        let mut arg = I915GemMmapGttArg {
            handle: handle.raw(),
            pad: 0,
            offset: 0,
        };

        // SAFETY: fd is valid, arg is properly initialized
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_I915_GEM_MMAP_GTT, &mut arg as *mut _) };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(LinuxHalError::GemMmapFailed(errno));
        }

        // mmap using the GTT offset
        // SAFETY: offset from ioctl is valid for this fd
        let ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                size as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd as i32,
                arg.offset as i64,
            )
        };

        if ptr == libc::MAP_FAILED {
            let errno = unsafe { *libc::__errno_location() };
            return Err(LinuxHalError::GemMmapFailed(errno));
        }

        Ok(ptr as *mut u8)
    }

    fn gem_munmap(&self, ptr: *mut u8, size: u64) -> LinuxHalResult<()> {
        if ptr.is_null() {
            return Ok(());
        }

        // SAFETY: ptr was obtained from gem_mmap with this size
        let ret = unsafe { libc::munmap(ptr as *mut libc::c_void, size as usize) };

        if ret < 0 {
            return Err(LinuxHalError::InternalError);
        }

        Ok(())
    }

    fn gem_set_domain(
        &self,
        handle: GemHandle,
        read_domains: u32,
        write_domain: u32,
    ) -> LinuxHalResult<()> {
        if !handle.is_valid() {
            return Err(LinuxHalError::InvalidGemHandle(handle.raw()));
        }

        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let mut arg = I915GemSetDomainArg {
            handle: handle.raw(),
            read_domains,
            write_domain,
        };

        // SAFETY: fd is valid, arg is properly initialized
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_I915_GEM_SET_DOMAIN, &mut arg as *mut _) };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(LinuxHalError::GemSetDomainFailed(errno));
        }

        Ok(())
    }

    fn gem_wait(&self, handle: GemHandle, timeout_ns: i64) -> LinuxHalResult<()> {
        if !handle.is_valid() {
            return Err(LinuxHalError::InvalidGemHandle(handle.raw()));
        }

        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let mut arg = I915GemWaitArg {
            bo_handle: handle.raw(),
            flags: 0,
            timeout_ns,
        };

        // SAFETY: fd is valid, arg is properly initialized
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_I915_GEM_WAIT, &mut arg as *mut _) };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            if errno == libc::ETIME || errno == libc::ETIMEDOUT {
                return Err(LinuxHalError::TimeoutExpired);
            }
            return Err(LinuxHalError::IoctlFailed(errno));
        }

        Ok(())
    }
}

// ============================================================================
// GEM Buffer Wrapper
// ============================================================================

/// GEM buffer object wrapper
///
/// RAII wrapper for GEM buffer lifecycle management.
/// Automatically closes handle on drop.
#[cfg(feature = "std")]
pub struct GemBuffer<'a> {
    /// Reference to DRM device
    device: &'a DrmDevice,
    /// GEM handle
    pub handle: GemHandle,
    /// Buffer size
    pub size: u64,
    /// Mapped pointer (if mapped)
    mapped_ptr: *mut u8,
    /// Mapped size
    mapped_size: u64,
}

#[cfg(feature = "std")]
impl<'a> GemBuffer<'a> {
    /// Create new GEM buffer
    ///
    /// # Arguments
    /// * `device` - DRM device
    /// * `size` - Buffer size in bytes
    pub fn create(device: &'a DrmDevice, size: u64) -> LinuxHalResult<Self> {
        let handle = device.gem_create(size)?;
        Ok(Self {
            device,
            handle,
            size: (size + 4095) & !4095,
            mapped_ptr: ptr::null_mut(),
            mapped_size: 0,
        })
    }

    /// Map buffer for CPU access
    ///
    /// Returns pointer to mapped region.
    pub fn mmap(&mut self) -> LinuxHalResult<*mut u8> {
        if !self.mapped_ptr.is_null() {
            return Ok(self.mapped_ptr);
        }

        let ptr = self.device.gem_mmap(self.handle, 0, self.size)?;
        self.mapped_ptr = ptr;
        self.mapped_size = self.size;
        Ok(ptr)
    }

    /// Unmap buffer
    pub fn munmap(&mut self) -> LinuxHalResult<()> {
        if !self.mapped_ptr.is_null() {
            self.device.gem_munmap(self.mapped_ptr, self.mapped_size)?;
            self.mapped_ptr = ptr::null_mut();
            self.mapped_size = 0;
        }
        Ok(())
    }

    /// Set buffer domain for cache coherency
    pub fn set_domain(&self, read_domains: u32, write_domain: u32) -> LinuxHalResult<()> {
        self.device.gem_set_domain(self.handle, read_domains, write_domain)
    }

    /// Wait for buffer to become idle
    pub fn wait(&self, timeout_ns: i64) -> LinuxHalResult<()> {
        self.device.gem_wait(self.handle, timeout_ns)
    }

    /// Check if buffer is mapped
    #[inline]
    pub fn is_mapped(&self) -> bool {
        !self.mapped_ptr.is_null()
    }
}

#[cfg(feature = "std")]
impl<'a> Drop for GemBuffer<'a> {
    fn drop(&mut self) {
        // Unmap if mapped
        let _ = self.munmap();
        // Close handle
        let _ = self.device.gem_close(self.handle);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drm_device_uninit() {
        let device = DrmDevice::uninit();
        assert!(!device.is_open());
        assert!(device.fd().is_none());
    }

    #[test]
    fn test_drm_device_size_and_alignment() {
        assert_eq!(core::mem::size_of::<DrmDevice>(), 128);
        assert_eq!(core::mem::align_of::<DrmDevice>(), 128);
    }

    #[test]
    fn test_gem_handle_creation() {
        let handle = GemHandle::new(42);
        assert_eq!(handle.raw(), 42);
        assert!(handle.is_valid());

        let invalid = GemHandle::new(0);
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_gem_domain_values() {
        assert_eq!(GemDomain::Cpu as u32, 0x01);
        assert_eq!(GemDomain::Gtt as u32, 0x02);
        assert_eq!(GemDomain::Render as u32, 0x04);
        assert_eq!(GemDomain::Display as u32, 0x10);
    }

    #[test]
    fn test_drm_version_arg_size() {
        // Verify struct sizes match kernel expectations
        assert_eq!(core::mem::size_of::<DrmGemCloseArg>(), 8);
        assert_eq!(core::mem::size_of::<I915GemCreateArg>(), 16);
        assert_eq!(core::mem::size_of::<I915GemMmapGttArg>(), 16);
        assert_eq!(core::mem::size_of::<I915GemSetDomainArg>(), 12);
        assert_eq!(core::mem::size_of::<I915GemWaitArg>(), 16);
    }
}
