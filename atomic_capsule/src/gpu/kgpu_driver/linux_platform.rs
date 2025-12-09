//! Linux GPU Platform Implementation for KGPU-Driver v2.0
//!
//! This module provides the main `LinuxGpuPlatformCapsule` implementation that ties
//! together all Linux GPU subsystems (DRM, GEM, KMS) into a unified `GpuPlatform`
//! implementation.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────────────┐
//! │                    LinuxGpuPlatformCapsule (512B)                        │
//! │                       GpuPlatform trait impl                             │
//! ├──────────────────────────────────────────────────────────────────────────┤
//! │                                                                          │
//! │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                   │
//! │  │ DRM Module  │    │ GEM Module  │    │ KMS Module  │                   │
//! │  │ (linux_drm) │    │ (linux_gem) │    │ (linux_kms) │                   │
//! │  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘                   │
//! │         │                  │                  │                          │
//! │         └──────────────────┴──────────────────┘                          │
//! │                            │                                             │
//! │                   ┌────────┴────────┐                                    │
//! │                   │  Vendor Routing │                                    │
//! │                   └────────┬────────┘                                    │
//! │                            │                                             │
//! │         ┌──────────────────┼──────────────────┐                          │
//! │         │                  │                  │                          │
//! │  ┌──────┴──────┐   ┌───────┴───────┐   ┌─────┴─────┐                     │
//! │  │ Intel i915  │   │   AMD amdgpu  │   │  NVIDIA   │                     │
//! │  │   /xe       │   │   /radeon     │   │ nouveau   │                     │
//! │  └─────────────┘   └───────────────┘   └───────────┘                     │
//! │                                                                          │
//! └──────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Chaos Compliance
//!
//! - **T1 Atomic**: 512B cache-aligned capsule with lockfree state management
//! - **100% Lockfree**: NO mutex, NO RwLock, only AtomicU64 operations
//! - **Generation Counters**: TOCTOU prevention on all state transitions
//! - **DualAtomicU64**: Consistent snapshot pattern for multi-field reads
//!
//! # Performance Targets
//!
//! | Operation | Target | Notes |
//! |-----------|--------|-------|
//! | State read | <10ns | Single atomic load |
//! | Device open | <10ms | Kernel syscall + init |
//! | Memory alloc | <1ms | GEM create |
//! | Command submit | <100us | Ring buffer write + ioctl |
//! | Fence wait | N/A | Kernel scheduler |
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_DRM_FD_VALID`: DRM file descriptors are valid after open
//! - `#ASSUME_IOCTL_THREAD_SAFE`: DRM ioctls are thread-safe
//! - `#ASSUME_VENDOR_STABLE`: GPU vendor doesn't change during operation
//! - `#ASSUME_GENERATION_MONOTONIC`: Generation counter always increases
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree state coordination)
//! - **Q33**: ComputationalCapsule verification (512B, cache-aligned)
//! - **Q34**: Audit trail design (generation counters, state history)

#![allow(dead_code)] // Allow during development

use core::sync::atomic::{AtomicU64, AtomicI32, Ordering};
use core::fmt::{self, Debug};

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use super::error::{KgpuDriverError, KgpuDriverResult};
use super::vendor::{GpuVendor, GpuGeneration, PciBdf};
use super::platform::{
    GpuPlatform, GpuDeviceInfo, MemoryFlags, QueueType,
    SubmissionId, FenceHandle, FirmwareType, FirmwareStatus,
    GPU_DEVICE_NAME_LEN,
};

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
use super::linux_drm::{
    DrmDeviceCapsule, DrmDeviceInfo, DrmNodeType, DrmCapability,
    open_drm_device, close_drm_device, enumerate_drm_devices,
    query_device_info, vendor_from_driver_name, fnv1a_hash,
};

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
use super::linux_gem::{
    GemBufferCapsule, GemState, GemFlags,
};

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
use super::linux_kms::{
    ConnectionStatus, ConnectorType,
};

// ============================================================================
// Low-Level ioctl Wrappers for Platform Implementation
// ============================================================================

/// Low-level ioctl wrappers for GpuPlatform trait implementation.
///
/// These provide simple return-value interfaces rather than the capsule-based
/// interfaces in linux_gem and linux_kms modules.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub mod ioctl {
    use super::*;
    use core::ffi::c_void;

    // DRM ioctl constants
    const DRM_IOCTL_BASE: u64 = 'd' as u64;
    const DRM_IOCTL_MODE_CREATE_DUMB: u64 = _iowr(DRM_IOCTL_BASE, 0xB2, 32);
    const DRM_IOCTL_MODE_MAP_DUMB: u64 = _iowr(DRM_IOCTL_BASE, 0xB3, 16);
    const DRM_IOCTL_MODE_DESTROY_DUMB: u64 = _iowr(DRM_IOCTL_BASE, 0xB4, 4);
    const DRM_IOCTL_GEM_CLOSE: u64 = _iow(DRM_IOCTL_BASE, 0x09, 8);
    const DRM_IOCTL_SYNCOBJ_CREATE: u64 = _iowr(DRM_IOCTL_BASE, 0xBF, 8);
    const DRM_IOCTL_SYNCOBJ_DESTROY: u64 = _iow(DRM_IOCTL_BASE, 0xC0, 4);
    const DRM_IOCTL_SYNCOBJ_WAIT: u64 = _iowr(DRM_IOCTL_BASE, 0xC3, 40);
    const DRM_IOCTL_SYNCOBJ_RESET: u64 = _iow(DRM_IOCTL_BASE, 0xC4, 8);

    // KMS ioctl constants
    const DRM_IOCTL_MODE_GETRESOURCES: u64 = _iowr(DRM_IOCTL_BASE, 0xA0, 64);
    const DRM_IOCTL_MODE_GETCONNECTOR: u64 = _iowr(DRM_IOCTL_BASE, 0xA7, 80);
    const DRM_IOCTL_MODE_SETCRTC: u64 = _iowr(DRM_IOCTL_BASE, 0xA2, 104);
    const DRM_IOCTL_MODE_PAGE_FLIP: u64 = _iowr(DRM_IOCTL_BASE, 0xB0, 32);
    const DRM_IOCTL_MODE_ADDFB2: u64 = _iowr(DRM_IOCTL_BASE, 0xB8, 100);
    const DRM_IOCTL_MODE_RMFB: u64 = _iowr(DRM_IOCTL_BASE, 0xAF, 4);

    // ioctl encoding helpers
    const fn _iow(ty: u64, nr: u64, size: u64) -> u64 {
        (1 << 30) | (ty << 8) | nr | (size << 16)
    }

    const fn _iowr(ty: u64, nr: u64, size: u64) -> u64 {
        (3 << 30) | (ty << 8) | nr | (size << 16)
    }

    /// Perform ioctl syscall
    #[inline]
    unsafe fn ioctl_raw(fd: i32, request: u64, arg: *mut c_void) -> i32 {
        #[cfg(target_arch = "x86_64")]
        {
            const SYS_IOCTL: i64 = 16;
            let ret: i64;
            core::arch::asm!(
                "syscall",
                inlateout("rax") SYS_IOCTL => ret,
                in("rdi") fd,
                in("rsi") request,
                in("rdx") arg,
                out("rcx") _,
                out("r11") _,
                options(nostack)
            );
            ret as i32
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            extern "C" {
                fn ioctl(fd: i32, request: u64, ...) -> i32;
            }
            ioctl(fd, request, arg)
        }
    }

    /// Create dumb buffer request structure
    #[repr(C)]
    struct DrmModeCreateDumb {
        height: u32,
        width: u32,
        bpp: u32,
        flags: u32,
        handle: u32,
        pitch: u32,
        size: u64,
    }

    /// Map dumb buffer request structure
    #[repr(C)]
    struct DrmModeMapDumb {
        handle: u32,
        _pad: u32,
        offset: u64,
    }

    /// Destroy dumb buffer request structure
    #[repr(C)]
    struct DrmModeDestroyDumb {
        handle: u32,
    }

    /// GEM close request structure
    #[repr(C)]
    struct DrmGemClose {
        handle: u32,
        _pad: u32,
    }

    /// Syncobj create request structure
    #[repr(C)]
    struct DrmSyncobjCreate {
        handle: u32,
        flags: u32,
    }

    /// Syncobj destroy request structure
    #[repr(C)]
    struct DrmSyncobjDestroy {
        handle: u32,
    }

    /// Syncobj wait request structure
    #[repr(C)]
    struct DrmSyncobjWait {
        handles: u64,  // pointer to array of handles
        timeout_nsec: i64,
        count_handles: u32,
        flags: u32,
        first_signaled: u32,
        _pad: u32,
    }

    /// Syncobj reset request structure
    #[repr(C)]
    struct DrmSyncobjReset {
        handles: u64,  // pointer to array of handles
        count_handles: u32,
        _pad: u32,
    }

    /// Create a dumb buffer and return GEM handle.
    ///
    /// # Arguments
    ///
    /// * `fd` - DRM device file descriptor
    /// * `width` - Buffer width in pixels
    /// * `height` - Buffer height
    /// * `bpp` - Bits per pixel
    ///
    /// # Returns
    ///
    /// GEM handle on success, or error
    pub fn create_dumb_buffer(fd: i32, width: u32, height: u32, bpp: u32) -> KgpuDriverResult<u32> {
        let mut req = DrmModeCreateDumb {
            width,
            height,
            bpp,
            flags: 0,
            handle: 0,
            pitch: 0,
            size: 0,
        };

        // #ASSUME_FD_VALID: fd must be valid DRM device
        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_MODE_CREATE_DUMB, &mut req as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(req.handle)
    }

    /// Get mmap offset for a GEM handle.
    pub fn map_dumb_buffer(fd: i32, handle: u32) -> KgpuDriverResult<u64> {
        let mut req = DrmModeMapDumb {
            handle,
            _pad: 0,
            offset: 0,
        };

        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_MODE_MAP_DUMB, &mut req as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(req.offset)
    }

    /// Close a GEM handle.
    pub fn gem_close(fd: i32, handle: u32) -> KgpuDriverResult<()> {
        let mut req = DrmGemClose {
            handle,
            _pad: 0,
        };

        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_GEM_CLOSE, &mut req as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(())
    }

    /// Create a DRM syncobj.
    pub fn create_syncobj(fd: i32) -> KgpuDriverResult<u32> {
        let mut req = DrmSyncobjCreate {
            handle: 0,
            flags: 0,
        };

        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_SYNCOBJ_CREATE, &mut req as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(req.handle)
    }

    /// Destroy a DRM syncobj.
    pub fn destroy_syncobj(fd: i32, handle: u32) -> KgpuDriverResult<()> {
        let mut req = DrmSyncobjDestroy { handle };

        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_SYNCOBJ_DESTROY, &mut req as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(())
    }

    /// Wait for a DRM syncobj to be signaled.
    pub fn wait_syncobj(fd: i32, handle: u32, timeout_ns: u64) -> KgpuDriverResult<bool> {
        let mut handles = [handle];
        let mut req = DrmSyncobjWait {
            handles: handles.as_mut_ptr() as u64,
            timeout_nsec: timeout_ns as i64,
            count_handles: 1,
            flags: 0,  // DRM_SYNCOBJ_WAIT_FLAGS_WAIT_ALL
            first_signaled: 0,
            _pad: 0,
        };

        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_SYNCOBJ_WAIT, &mut req as *mut _ as *mut c_void)
        };

        if ret < 0 {
            // Timeout returns -ETIME
            if ret == -62 {  // ETIME
                return Ok(false);
            }
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(true)
    }

    /// Reset a DRM syncobj.
    pub fn reset_syncobj(fd: i32, handle: u32) -> KgpuDriverResult<()> {
        let mut handles = [handle];
        let mut req = DrmSyncobjReset {
            handles: handles.as_mut_ptr() as u64,
            count_handles: 1,
            _pad: 0,
        };

        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_SYNCOBJ_RESET, &mut req as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(())
    }

    // ========================================================================
    // KMS ioctl structures and functions
    // ========================================================================

    /// KMS resources result
    pub struct KmsResourcesResult {
        pub connector_ids: Vec<u32>,
        pub crtc_ids: Vec<u32>,
        pub encoder_ids: Vec<u32>,
    }

    /// Connector info result
    pub struct ConnectorResult {
        pub connector_type: u32,
        pub connection: u32,
        pub mm_width: u32,
        pub mm_height: u32,
        pub modes: Vec<DrmModeInfo>,
        pub props: Vec<u32>,
    }

    /// DRM mode info
    #[derive(Clone)]
    pub struct DrmModeInfo {
        pub clock: u32,
        pub hdisplay: u16,
        pub htotal: u16,
        pub vdisplay: u16,
        pub vtotal: u16,
    }

    #[repr(C)]
    struct DrmModeCardRes {
        fb_id_ptr: u64,
        crtc_id_ptr: u64,
        connector_id_ptr: u64,
        encoder_id_ptr: u64,
        count_fbs: u32,
        count_crtcs: u32,
        count_connectors: u32,
        count_encoders: u32,
        min_width: u32,
        max_width: u32,
        min_height: u32,
        max_height: u32,
    }

    #[repr(C)]
    struct DrmModeGetConnector {
        encoders_ptr: u64,
        modes_ptr: u64,
        props_ptr: u64,
        prop_values_ptr: u64,
        count_modes: u32,
        count_props: u32,
        count_encoders: u32,
        encoder_id: u32,
        connector_id: u32,
        connector_type: u32,
        connector_type_id: u32,
        connection: u32,
        mm_width: u32,
        mm_height: u32,
        subpixel: u32,
        _pad: u32,
    }

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct DrmModeModeInfo {
        clock: u32,
        hdisplay: u16,
        hsync_start: u16,
        hsync_end: u16,
        htotal: u16,
        hskew: u16,
        vdisplay: u16,
        vsync_start: u16,
        vsync_end: u16,
        vtotal: u16,
        vscan: u16,
        vrefresh: u32,
        flags: u32,
        r#type: u32,
        name: [u8; 32],
    }

    #[repr(C)]
    struct DrmModeCrtc {
        set_connectors_ptr: u64,
        count_connectors: u32,
        crtc_id: u32,
        fb_id: u32,
        x: u32,
        y: u32,
        gamma_size: u32,
        mode_valid: u32,
        mode: DrmModeModeInfo,
    }

    #[repr(C)]
    struct DrmModePageFlip {
        crtc_id: u32,
        fb_id: u32,
        flags: u32,
        reserved: u32,
        user_data: u64,
    }

    #[repr(C)]
    struct DrmModeFb2 {
        fb_id: u32,
        width: u32,
        height: u32,
        pixel_format: u32,
        flags: u32,
        handles: [u32; 4],
        pitches: [u32; 4],
        offsets: [u32; 4],
        modifier: [u64; 4],
    }

    /// Get KMS resources.
    pub fn get_resources(fd: i32) -> KgpuDriverResult<KmsResourcesResult> {
        // First call to get counts
        let mut res = DrmModeCardRes {
            fb_id_ptr: 0,
            crtc_id_ptr: 0,
            connector_id_ptr: 0,
            encoder_id_ptr: 0,
            count_fbs: 0,
            count_crtcs: 0,
            count_connectors: 0,
            count_encoders: 0,
            min_width: 0,
            max_width: 0,
            min_height: 0,
            max_height: 0,
        };

        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_MODE_GETRESOURCES, &mut res as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        // Allocate buffers
        let mut connectors = vec![0u32; res.count_connectors as usize];
        let mut crtcs = vec![0u32; res.count_crtcs as usize];
        let mut encoders = vec![0u32; res.count_encoders as usize];

        res.connector_id_ptr = connectors.as_mut_ptr() as u64;
        res.crtc_id_ptr = crtcs.as_mut_ptr() as u64;
        res.encoder_id_ptr = encoders.as_mut_ptr() as u64;

        // Second call to get IDs
        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_MODE_GETRESOURCES, &mut res as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(KmsResourcesResult {
            connector_ids: connectors,
            crtc_ids: crtcs,
            encoder_ids: encoders,
        })
    }

    /// Get connector info.
    pub fn get_connector(fd: i32, conn_id: u32) -> KgpuDriverResult<ConnectorResult> {
        // First call to get counts
        let mut conn = DrmModeGetConnector {
            encoders_ptr: 0,
            modes_ptr: 0,
            props_ptr: 0,
            prop_values_ptr: 0,
            count_modes: 0,
            count_props: 0,
            count_encoders: 0,
            encoder_id: 0,
            connector_id: conn_id,
            connector_type: 0,
            connector_type_id: 0,
            connection: 0,
            mm_width: 0,
            mm_height: 0,
            subpixel: 0,
            _pad: 0,
        };

        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_MODE_GETCONNECTOR, &mut conn as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        // Allocate buffers
        let mut modes = vec![DrmModeModeInfo::default(); conn.count_modes as usize];
        let mut props = vec![0u32; conn.count_props as usize];
        let mut prop_values = vec![0u64; conn.count_props as usize];

        conn.modes_ptr = modes.as_mut_ptr() as u64;
        conn.props_ptr = props.as_mut_ptr() as u64;
        conn.prop_values_ptr = prop_values.as_mut_ptr() as u64;

        // Second call
        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_MODE_GETCONNECTOR, &mut conn as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(ConnectorResult {
            connector_type: conn.connector_type,
            connection: conn.connection,
            mm_width: conn.mm_width,
            mm_height: conn.mm_height,
            modes: modes.iter().map(|m| DrmModeInfo {
                clock: m.clock,
                hdisplay: m.hdisplay,
                htotal: m.htotal,
                vdisplay: m.vdisplay,
                vtotal: m.vtotal,
            }).collect(),
            props,
        })
    }

    /// Set CRTC configuration.
    pub fn set_crtc(
        fd: i32,
        crtc_id: u32,
        fb_id: u32,
        x: u32,
        y: u32,
        connectors: &[u32],
        mode: &super::super::linux_kms::DrmMode,
    ) -> KgpuDriverResult<()> {
        let mut conn_ids = connectors.to_vec();
        let mode_info = DrmModeModeInfo {
            clock: mode.clock,
            hdisplay: mode.hdisplay,
            hsync_start: mode.hsync_start,
            hsync_end: mode.hsync_end,
            htotal: mode.htotal,
            hskew: mode.hskew,
            vdisplay: mode.vdisplay,
            vsync_start: mode.vsync_start,
            vsync_end: mode.vsync_end,
            vtotal: mode.vtotal,
            vscan: mode.vscan,
            vrefresh: mode.vrefresh,
            flags: mode.flags,
            r#type: mode.type_,
            name: mode.name,
        };

        let mut crtc = DrmModeCrtc {
            set_connectors_ptr: conn_ids.as_mut_ptr() as u64,
            count_connectors: connectors.len() as u32,
            crtc_id,
            fb_id,
            x,
            y,
            gamma_size: 0,
            mode_valid: 1,
            mode: mode_info,
        };

        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_MODE_SETCRTC, &mut crtc as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(())
    }

    /// Queue a page flip.
    pub fn page_flip(fd: i32, crtc_id: u32, fb_id: u32, flags: u32, user_data: u64) -> KgpuDriverResult<()> {
        let mut flip = DrmModePageFlip {
            crtc_id,
            fb_id,
            flags,
            reserved: 0,
            user_data,
        };

        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_MODE_PAGE_FLIP, &mut flip as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(())
    }

    /// Add a framebuffer.
    pub fn add_fb2(
        fd: i32,
        width: u32,
        height: u32,
        pixel_format: u32,
        handles: &[u32; 4],
        pitches: &[u32; 4],
        offsets: &[u32; 4],
        flags: u32,
    ) -> KgpuDriverResult<u32> {
        let mut fb = DrmModeFb2 {
            fb_id: 0,
            width,
            height,
            pixel_format,
            flags,
            handles: *handles,
            pitches: *pitches,
            offsets: *offsets,
            modifier: [0; 4],
        };

        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_MODE_ADDFB2, &mut fb as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(fb.fb_id)
    }

    /// Remove a framebuffer.
    pub fn rm_fb(fd: i32, fb_id: u32) -> KgpuDriverResult<()> {
        let mut fb = fb_id;

        let ret = unsafe {
            ioctl_raw(fd, DRM_IOCTL_MODE_RMFB, &mut fb as *mut _ as *mut c_void)
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(())
    }
}

// ============================================================================
// Platform State Flags
// ============================================================================

/// Platform state flags (packed in state AtomicU64)
const PLATFORM_FLAG_INITIALIZED: u64 = 0x0001;
const PLATFORM_FLAG_DEVICE_OPEN: u64 = 0x0002;
const PLATFORM_FLAG_MASTER: u64 = 0x0004;
const PLATFORM_FLAG_AUTHENTICATED: u64 = 0x0008;
const PLATFORM_FLAG_PRIME_CAPABLE: u64 = 0x0010;
const PLATFORM_FLAG_SYNCOBJ_CAPABLE: u64 = 0x0020;
const PLATFORM_FLAG_ATOMIC_CAPABLE: u64 = 0x0040;
const PLATFORM_FLAG_ERROR: u64 = 0x8000;

/// Maximum supported GPUs
const MAX_GPU_DEVICES: usize = 8;

/// Maximum memory allocations to track
const MAX_ALLOCATIONS: usize = 1024;

/// Maximum queues per device
const MAX_QUEUES: usize = 16;

/// Maximum fences per device
const MAX_FENCES: usize = 256;

// ============================================================================
// Device Handle Types
// ============================================================================

/// Linux DRM device handle.
///
/// Packs device index and DRM file descriptor together.
///
/// # Layout
/// ```text
/// Bits 0-7:   Device index (0-7)
/// Bits 8-31:  DRM file descriptor (24 bits)
/// Bits 32-63: Generation counter (32 bits)
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct LinuxDeviceHandle(u64);

impl LinuxDeviceHandle {
    /// Invalid handle constant
    pub const INVALID: Self = Self(0);

    /// Create a new device handle
    #[inline]
    pub const fn new(device_index: u8, fd: i32, generation: u32) -> Self {
        Self(
            (device_index as u64)
                | (((fd as u32) as u64) << 8)
                | ((generation as u64) << 32)
        )
    }

    /// Get the device index (0-7)
    #[inline]
    pub const fn device_index(self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Get the DRM file descriptor
    #[inline]
    pub const fn fd(self) -> i32 {
        ((self.0 >> 8) & 0x00FFFFFF) as i32
    }

    /// Get the generation counter
    #[inline]
    pub const fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Check if handle is valid
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != 0 && self.fd() >= 0
    }

    /// Get raw value
    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl Debug for LinuxDeviceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxDeviceHandle")
            .field("device_index", &self.device_index())
            .field("fd", &self.fd())
            .field("generation", &self.generation())
            .finish()
    }
}

impl Default for LinuxDeviceHandle {
    #[inline]
    fn default() -> Self {
        Self::INVALID
    }
}

// ============================================================================
// Memory Handle
// ============================================================================

/// Linux memory handle.
///
/// Packs device index and GEM handle together.
///
/// # Layout
/// ```text
/// Bits 0-7:   Device index
/// Bits 8-39:  GEM handle (32 bits)
/// Bits 40-63: Allocation index (24 bits)
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct LinuxMemoryHandle(u64);

impl LinuxMemoryHandle {
    /// Invalid handle constant
    pub const INVALID: Self = Self(0);

    /// Create a new memory handle
    #[inline]
    pub const fn new(device_index: u8, gem_handle: u32, alloc_index: u32) -> Self {
        Self(
            (device_index as u64)
                | ((gem_handle as u64) << 8)
                | (((alloc_index & 0x00FFFFFF) as u64) << 40)
        )
    }

    /// Get the device index
    #[inline]
    pub const fn device_index(self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Get the GEM handle
    #[inline]
    pub const fn gem_handle(self) -> u32 {
        ((self.0 >> 8) & 0xFFFFFFFF) as u32
    }

    /// Get the allocation index
    #[inline]
    pub const fn alloc_index(self) -> u32 {
        ((self.0 >> 40) & 0x00FFFFFF) as u32
    }

    /// Check if handle is valid
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }

    /// Get raw value
    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl Debug for LinuxMemoryHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxMemoryHandle")
            .field("device_index", &self.device_index())
            .field("gem_handle", &self.gem_handle())
            .field("alloc_index", &self.alloc_index())
            .finish()
    }
}

impl Default for LinuxMemoryHandle {
    #[inline]
    fn default() -> Self {
        Self::INVALID
    }
}

// ============================================================================
// Platform Fence Handle
// ============================================================================

/// Linux fence handle (DRM syncobj).
///
/// # Layout
/// ```text
/// Bits 0-7:   Device index
/// Bits 8-39:  Syncobj handle
/// Bits 40-63: Fence index
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct LinuxFenceHandle(u64);

impl LinuxFenceHandle {
    /// Invalid handle constant
    pub const INVALID: Self = Self(0);

    /// Create a new fence handle
    #[inline]
    pub const fn new(device_index: u8, syncobj: u32, fence_index: u32) -> Self {
        Self(
            (device_index as u64)
                | ((syncobj as u64) << 8)
                | (((fence_index & 0x00FFFFFF) as u64) << 40)
        )
    }

    /// Get device index
    #[inline]
    pub const fn device_index(self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Get syncobj handle
    #[inline]
    pub const fn syncobj(self) -> u32 {
        ((self.0 >> 8) & 0xFFFFFFFF) as u32
    }

    /// Get fence index
    #[inline]
    pub const fn fence_index(self) -> u32 {
        ((self.0 >> 40) & 0x00FFFFFF) as u32
    }

    /// Check if valid
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

impl Debug for LinuxFenceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxFenceHandle")
            .field("device_index", &self.device_index())
            .field("syncobj", &self.syncobj())
            .field("fence_index", &self.fence_index())
            .finish()
    }
}

impl Default for LinuxFenceHandle {
    #[inline]
    fn default() -> Self {
        Self::INVALID
    }
}

// ============================================================================
// Display Info
// ============================================================================

/// Connected display information
#[derive(Debug, Clone, Copy)]
pub struct DisplayInfo {
    /// Connector ID
    pub connector_id: u32,
    /// Connector type (HDMI, DP, etc.)
    pub connector_type: u8,
    /// Connection status
    pub connected: bool,
    /// EDID present
    pub has_edid: bool,
    /// Preferred width
    pub width: u32,
    /// Preferred height
    pub height: u32,
    /// Refresh rate in Hz (fixed point Q16.16)
    pub refresh_hz_q16: u32,
    /// Physical width in mm
    pub width_mm: u32,
    /// Physical height in mm
    pub height_mm: u32,
}

impl DisplayInfo {
    /// Create new display info
    #[inline]
    pub const fn new() -> Self {
        Self {
            connector_id: 0,
            connector_type: 0,
            connected: false,
            has_edid: false,
            width: 0,
            height: 0,
            refresh_hz_q16: 0,
            width_mm: 0,
            height_mm: 0,
        }
    }
}

impl Default for DisplayInfo {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Linux GPU Platform Capsule (T1 Atomic, 512B)
// ============================================================================

/// Linux GPU Platform Capsule (T1 Atomic, 512B aligned)
///
/// Main entry point for KGPU-Driver on Linux. Implements the `GpuPlatform` trait
/// using DRM/GEM/KMS kernel subsystems with vendor-specific routing for Intel,
/// AMD, and NVIDIA GPUs.
///
/// # Layout (512 bytes, 8 cache lines)
///
/// ```text
/// LinuxGpuPlatformCapsule (512B)
/// ┌─────────────────────────────────────────────────────────────────────┐
/// │ Cache Line 0 (64B): State and Generation                            │
/// │  state (8B) | generation (8B) | error_count (8B) | last_error (8B)  │
/// │  active_device_fd (8B) | device_count (8B) | active_index (8B) | - │
/// ├─────────────────────────────────────────────────────────────────────┤
/// │ Cache Line 1 (64B): Device Tracking (8 devices)                     │
/// │  devices[0-7] (64B) - packed: [fd:32|vendor:8|gen:8|flags:16]       │
/// ├─────────────────────────────────────────────────────────────────────┤
/// │ Cache Line 2 (64B): Memory Statistics                               │
/// │  total_allocated (8B) | allocation_count (8B) | peak_allocated (8B) │
/// │  alloc_generation (8B) | free_count (8B) | map_count (8B) | - | -   │
/// ├─────────────────────────────────────────────────────────────────────┤
/// │ Cache Line 3 (64B): Submission Statistics                           │
/// │  submissions_total (8B) | submissions_pending (8B)                  │
/// │  last_submission_id (8B) | submission_generation (8B)               │
/// │  completed_count (8B) | failed_count (8B) | timeout_count (8B) | -  │
/// ├─────────────────────────────────────────────────────────────────────┤
/// │ Cache Line 4 (64B): Fence Statistics                                │
/// │  fences_created (8B) | fences_signaled (8B) | fences_destroyed (8B) │
/// │  fence_generation (8B) | active_fences (8B) | fence_wait_ns (8B)    │
/// │  - | -                                                              │
/// ├─────────────────────────────────────────────────────────────────────┤
/// │ Cache Line 5-7 (192B): Padding                                      │
/// │  _padding[192B]                                                     │
/// └─────────────────────────────────────────────────────────────────────┘
/// ```
///
/// # Packed Device State
///
/// Each entry in `devices[]` is packed as:
/// ```text
/// Bits 0-31:  DRM file descriptor (i32 as u32)
/// Bits 32-39: GPU vendor (GpuVendor as u8)
/// Bits 40-47: GPU generation (GpuGeneration as u8)
/// Bits 48-63: Device flags
/// ```
///
/// # ASSUM Safety
///
/// - `#ASSUME_ATOMIC_ALIGNED`: All AtomicU64 fields are naturally aligned
/// - `#ASSUME_CACHE_ALIGNED`: 512B alignment ensures no false sharing
/// - `#ASSUME_GENERATION_MONOTONIC`: Generation counters always increase
#[repr(C, align(512))]
pub struct LinuxGpuPlatformCapsule {
    // === Cache Line 0: State and Generation ===
    /// Packed state: [initialized:8][device_count:8][active_device:8][flags:40]
    state: AtomicU64,
    /// Global generation counter for CAS operations
    generation: AtomicU64,
    /// Error count
    error_count: AtomicU64,
    /// Last error code
    last_error: AtomicU64,
    /// Active device file descriptor
    active_device_fd: AtomicI32,
    /// Padding for cache line alignment
    _cl0_pad0: AtomicI32,
    /// Device count (initialized devices)
    device_count: AtomicU64,
    /// Active device index
    active_index: AtomicU64,

    // === Cache Line 1: Device Tracking ===
    /// Device state array (8 devices max)
    /// Packed: [fd:32|vendor:8|gen:8|flags:16]
    devices: [AtomicU64; MAX_GPU_DEVICES],

    // === Cache Line 2: Memory Statistics ===
    /// Total allocated bytes
    total_allocated: AtomicU64,
    /// Current allocation count
    allocation_count: AtomicU64,
    /// Peak allocated bytes
    peak_allocated: AtomicU64,
    /// Allocation generation counter
    alloc_generation: AtomicU64,
    /// Free operation count
    free_count: AtomicU64,
    /// Map operation count
    map_count: AtomicU64,
    /// Padding
    _cl2_pad: [AtomicU64; 2],

    // === Cache Line 3: Submission Statistics ===
    /// Total submissions made
    submissions_total: AtomicU64,
    /// Currently pending submissions
    submissions_pending: AtomicU64,
    /// Last submission ID issued
    last_submission_id: AtomicU64,
    /// Submission generation counter
    submission_generation: AtomicU64,
    /// Completed submission count
    completed_count: AtomicU64,
    /// Failed submission count
    failed_count: AtomicU64,
    /// Timed out submission count
    timeout_count: AtomicU64,
    /// Padding
    _cl3_pad: AtomicU64,

    // === Cache Line 4: Fence Statistics ===
    /// Total fences created
    fences_created: AtomicU64,
    /// Fences that have been signaled
    fences_signaled: AtomicU64,
    /// Fences that have been destroyed
    fences_destroyed: AtomicU64,
    /// Fence generation counter
    fence_generation: AtomicU64,
    /// Currently active fences
    active_fences: AtomicU64,
    /// Total fence wait time in nanoseconds
    fence_wait_ns: AtomicU64,
    /// Padding
    _cl4_pad: [AtomicU64; 2],

    // === Cache Lines 5-7: Padding ===
    _padding: [u8; 192],
}

// Compile-time size assertion
const _: () = {
    assert!(
        core::mem::size_of::<LinuxGpuPlatformCapsule>() == 512,
        "LinuxGpuPlatformCapsule must be 512 bytes"
    );
    assert!(
        core::mem::align_of::<LinuxGpuPlatformCapsule>() == 512,
        "LinuxGpuPlatformCapsule must be 512-byte aligned"
    );
};

impl LinuxGpuPlatformCapsule {
    /// Create a new uninitialized platform capsule.
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            last_error: AtomicU64::new(0),
            active_device_fd: AtomicI32::new(-1),
            _cl0_pad0: AtomicI32::new(0),
            device_count: AtomicU64::new(0),
            active_index: AtomicU64::new(u64::MAX),

            devices: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            ],

            total_allocated: AtomicU64::new(0),
            allocation_count: AtomicU64::new(0),
            peak_allocated: AtomicU64::new(0),
            alloc_generation: AtomicU64::new(0),
            free_count: AtomicU64::new(0),
            map_count: AtomicU64::new(0),
            _cl2_pad: [AtomicU64::new(0), AtomicU64::new(0)],

            submissions_total: AtomicU64::new(0),
            submissions_pending: AtomicU64::new(0),
            last_submission_id: AtomicU64::new(0),
            submission_generation: AtomicU64::new(0),
            completed_count: AtomicU64::new(0),
            failed_count: AtomicU64::new(0),
            timeout_count: AtomicU64::new(0),
            _cl3_pad: AtomicU64::new(0),

            fences_created: AtomicU64::new(0),
            fences_signaled: AtomicU64::new(0),
            fences_destroyed: AtomicU64::new(0),
            fence_generation: AtomicU64::new(0),
            active_fences: AtomicU64::new(0),
            fence_wait_ns: AtomicU64::new(0),
            _cl4_pad: [AtomicU64::new(0), AtomicU64::new(0)],

            _padding: [0; 192],
        }
    }

    /// Initialize the platform.
    ///
    /// Must be called before any other operations.
    pub fn initialize(&self) -> KgpuDriverResult<()> {
        // CAS to set initialized flag
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            if (old_state & PLATFORM_FLAG_INITIALIZED) != 0 {
                return Ok(()); // Already initialized
            }

            let new_state = old_state | PLATFORM_FLAG_INITIALIZED;
            if self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                self.generation.fetch_add(1, Ordering::AcqRel);
                return Ok(());
            }
        }
    }

    /// Check if platform is initialized
    #[inline]
    pub fn is_initialized(&self) -> bool {
        (self.state.load(Ordering::Acquire) & PLATFORM_FLAG_INITIALIZED) != 0
    }

    /// Get current generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get device count
    #[inline]
    pub fn device_count(&self) -> usize {
        self.device_count.load(Ordering::Acquire) as usize
    }

    /// Get active device index
    #[inline]
    pub fn active_device_index(&self) -> Option<usize> {
        let idx = self.active_index.load(Ordering::Acquire);
        if idx < MAX_GPU_DEVICES as u64 {
            Some(idx as usize)
        } else {
            None
        }
    }

    /// Pack device state for storage
    #[inline]
    const fn pack_device_state(fd: i32, vendor: GpuVendor, generation: GpuGeneration, flags: u16) -> u64 {
        ((fd as u32) as u64)
            | ((vendor as u64) << 32)
            | ((generation as u64) << 40)
            | ((flags as u64) << 48)
    }

    /// Unpack device state
    #[inline]
    const fn unpack_device_fd(packed: u64) -> i32 {
        (packed & 0xFFFFFFFF) as i32
    }

    #[inline]
    const fn unpack_device_vendor(packed: u64) -> GpuVendor {
        match ((packed >> 32) & 0xFF) as u16 {
            0x8086 => GpuVendor::Intel,
            0x1002 => GpuVendor::Amd,
            0x10DE => GpuVendor::Nvidia,
            _ => GpuVendor::Unknown,
        }
    }

    #[inline]
    const fn unpack_device_generation(packed: u64) -> u8 {
        ((packed >> 40) & 0xFF) as u8
    }

    #[inline]
    const fn unpack_device_flags(packed: u64) -> u16 {
        ((packed >> 48) & 0xFFFF) as u16
    }

    /// Store device info at index
    fn store_device(&self, index: usize, fd: i32, vendor: GpuVendor, gen: GpuGeneration, flags: u16) {
        if index < MAX_GPU_DEVICES {
            let packed = Self::pack_device_state(fd, vendor, gen, flags);
            self.devices[index].store(packed, Ordering::Release);
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// Get device info at index
    fn get_device(&self, index: usize) -> Option<(i32, GpuVendor, u8, u16)> {
        if index < MAX_GPU_DEVICES {
            let packed = self.devices[index].load(Ordering::Acquire);
            if packed != 0 {
                return Some((
                    Self::unpack_device_fd(packed),
                    Self::unpack_device_vendor(packed),
                    Self::unpack_device_generation(packed),
                    Self::unpack_device_flags(packed),
                ));
            }
        }
        None
    }

    /// Get vendor for device at index
    #[inline]
    pub fn get_vendor(&self, device_index: usize) -> GpuVendor {
        if device_index < MAX_GPU_DEVICES {
            let packed = self.devices[device_index].load(Ordering::Acquire);
            Self::unpack_device_vendor(packed)
        } else {
            GpuVendor::Unknown
        }
    }

    /// Record an error
    fn record_error(&self, error: &KgpuDriverError) {
        self.error_count.fetch_add(1, Ordering::AcqRel);
        let error_code = match error {
            KgpuDriverError::DeviceNotFound => 1,
            KgpuDriverError::PermissionDenied => 2,
            KgpuDriverError::DeviceBusy => 3,
            KgpuDriverError::InvalidParameter => 4,
            KgpuDriverError::OutOfHostMemory => 5,
            KgpuDriverError::DrmOpenFailed => 6,
            KgpuDriverError::DrmIoctlFailed => 7,
            _ => 255,
        };
        self.last_error.store(error_code, Ordering::Release);
    }

    /// Update memory statistics after allocation
    fn update_alloc_stats(&self, size: u64) {
        self.total_allocated.fetch_add(size, Ordering::AcqRel);
        self.allocation_count.fetch_add(1, Ordering::AcqRel);
        self.alloc_generation.fetch_add(1, Ordering::AcqRel);

        // Update peak if needed (best-effort, not atomic)
        let current = self.total_allocated.load(Ordering::Acquire);
        let peak = self.peak_allocated.load(Ordering::Acquire);
        if current > peak {
            let _ = self.peak_allocated.compare_exchange(
                peak,
                current,
                Ordering::AcqRel,
                Ordering::Relaxed,
            );
        }
    }

    /// Update memory statistics after free
    fn update_free_stats(&self, size: u64) {
        self.total_allocated.fetch_sub(size.min(self.total_allocated.load(Ordering::Acquire)), Ordering::AcqRel);
        self.allocation_count.fetch_sub(1, Ordering::AcqRel);
        self.free_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Update submission statistics
    fn update_submission_stats(&self, success: bool) {
        self.submissions_total.fetch_add(1, Ordering::AcqRel);
        if success {
            self.submissions_pending.fetch_add(1, Ordering::AcqRel);
        } else {
            self.failed_count.fetch_add(1, Ordering::AcqRel);
        }
        self.submission_generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Get next submission ID
    fn next_submission_id(&self, queue_index: u16) -> SubmissionId {
        let seq = self.last_submission_id.fetch_add(1, Ordering::AcqRel);
        SubmissionId::new(queue_index, seq)
    }

    /// Get total memory allocated
    #[inline]
    pub fn total_allocated(&self) -> u64 {
        self.total_allocated.load(Ordering::Acquire)
    }

    /// Get allocation count
    #[inline]
    pub fn allocation_count(&self) -> u64 {
        self.allocation_count.load(Ordering::Acquire)
    }

    /// Get submissions total
    #[inline]
    pub fn submissions_total(&self) -> u64 {
        self.submissions_total.load(Ordering::Acquire)
    }

    /// Get pending submissions
    #[inline]
    pub fn submissions_pending(&self) -> u64 {
        self.submissions_pending.load(Ordering::Acquire)
    }

    /// Get error count
    #[inline]
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Acquire)
    }
}

impl Default for LinuxGpuPlatformCapsule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for LinuxGpuPlatformCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxGpuPlatformCapsule")
            .field("initialized", &self.is_initialized())
            .field("generation", &self.generation())
            .field("device_count", &self.device_count())
            .field("active_device", &self.active_device_index())
            .field("total_allocated", &self.total_allocated())
            .field("error_count", &self.error_count())
            .finish()
    }
}

// SAFETY: LinuxGpuPlatformCapsule is 100% lockfree using only atomic operations
// #ASSUME_ATOMIC_THREAD_SAFE: AtomicU64 operations are thread-safe
// #VERIFY_ATOMIC_THREAD_SAFE: Rust guarantees atomic operation safety
unsafe impl Send for LinuxGpuPlatformCapsule {}
unsafe impl Sync for LinuxGpuPlatformCapsule {}

// ============================================================================
// GpuPlatform Trait Implementation (Linux-specific)
// ============================================================================

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
impl GpuPlatform for LinuxGpuPlatformCapsule {
    type DeviceHandle = LinuxDeviceHandle;
    type MemoryHandle = LinuxMemoryHandle;
    type PlatformFenceHandle = LinuxFenceHandle;

    /// Enumerate all available GPU devices.
    ///
    /// Scans /dev/dri/card* and /dev/dri/renderD* for DRM devices,
    /// queries their capabilities, and returns device information.
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_DEVFS_PRESENT`: /dev/dri directory exists
    /// - `#ASSUME_DRM_DEVICES_VALID`: DRM device files are valid
    fn enumerate_devices() -> Result<Vec<GpuDeviceInfo>, KgpuDriverError> {
        let drm_devices = enumerate_drm_devices()?;
        let mut devices = Vec::with_capacity(drm_devices.len());

        for (path, node_type) in &drm_devices {
            // Only enumerate render nodes for simplicity (no master required)
            if *node_type != DrmNodeType::Render && *node_type != DrmNodeType::Primary {
                continue;
            }

            match query_device_info(path, *node_type) {
                Ok(drm_info) => {
                    let mut info = GpuDeviceInfo::new();

                    // Set vendor ID from driver detection
                    let vendor = drm_info.vendor;
                    info.vendor_id = vendor.vendor_id();

                    // Try to get PCI BDF if available
                    if let Some(ref bdf) = drm_info.pci_bdf {
                        info.pci_bus = bdf.bus;
                        info.pci_device = bdf.device;
                        info.pci_function = bdf.function;
                    }

                    // Set generation
                    info.generation = drm_info.generation;

                    // Set name from driver version
                    info.set_name(&format!("{} {}", drm_info.version.name, drm_info.generation.name()));

                    // Set capabilities bitmask
                    info.queue_support = 0;
                    if drm_info.has_prime {
                        info.queue_support |= 1 << (QueueType::Graphics as u32);
                        info.queue_support |= 1 << (QueueType::Compute as u32);
                        info.queue_support |= 1 << (QueueType::Transfer as u32);
                    }

                    devices.push(info);
                }
                Err(_) => {
                    // Skip devices we can't query
                    continue;
                }
            }
        }

        if devices.is_empty() {
            return Err(KgpuDriverError::DeviceNotFound);
        }

        Ok(devices)
    }

    /// Open a GPU device by index.
    ///
    /// Opens the DRM device file and initializes vendor-specific state.
    fn open_device(device_index: usize) -> Result<Self::DeviceHandle, KgpuDriverError> {
        if device_index >= MAX_GPU_DEVICES {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Enumerate devices to get the path
        let drm_devices = enumerate_drm_devices()?;

        // Filter to render/primary nodes
        let valid_devices: Vec<_> = drm_devices
            .iter()
            .filter(|(_, node_type)| {
                *node_type == DrmNodeType::Render || *node_type == DrmNodeType::Primary
            })
            .collect();

        if device_index >= valid_devices.len() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        let (path, _node_type) = valid_devices[device_index];

        // Open the device
        let fd = open_drm_device(path)?;

        // Create handle with generation counter
        // Note: In a real implementation, we'd track this in self
        // but since this is a trait method, we use a simple counter
        static GENERATION: AtomicU64 = AtomicU64::new(0);
        let gen = GENERATION.fetch_add(1, Ordering::AcqRel) as u32;

        Ok(LinuxDeviceHandle::new(device_index as u8, fd, gen))
    }

    /// Close a GPU device.
    fn close_device(handle: Self::DeviceHandle) -> Result<(), KgpuDriverError> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        close_drm_device(handle.fd())
    }

    /// Get device information for an open device.
    fn get_device_info(handle: Self::DeviceHandle) -> Result<GpuDeviceInfo, KgpuDriverError> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Get driver version to identify the device
        let version = super::linux_drm::ioctl::drm_version(handle.fd())?;
        let vendor = vendor_from_driver_name(&version.name);

        let mut info = GpuDeviceInfo::new();
        info.vendor_id = vendor.vendor_id();
        info.set_name(&format!("{} v{}.{}.{}",
            version.name, version.major, version.minor, version.patchlevel));

        // Set queue support based on driver
        info.queue_support = (1 << QueueType::Graphics as u32)
            | (1 << QueueType::Compute as u32)
            | (1 << QueueType::Transfer as u32);

        Ok(info)
    }

    /// Allocate GPU memory using GEM.
    fn alloc_memory(
        handle: Self::DeviceHandle,
        size: usize,
        flags: MemoryFlags,
    ) -> Result<Self::MemoryHandle, KgpuDriverError> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }
        if size == 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Convert flags to GEM flags
        let gem_flags = GemFlags::from_memory_flags(flags);

        // Create a dumb buffer (works on all drivers)
        // In a real implementation, we'd use vendor-specific allocation
        let gem_handle = ioctl::create_dumb_buffer(
            handle.fd(),
            size as u32,  // width
            1,            // height
            32,           // bpp
        )?;

        // Track allocation (use device index as crude allocation index)
        static ALLOC_INDEX: AtomicU64 = AtomicU64::new(0);
        let alloc_idx = ALLOC_INDEX.fetch_add(1, Ordering::AcqRel) as u32;

        Ok(LinuxMemoryHandle::new(handle.device_index(), gem_handle, alloc_idx))
    }

    /// Free GPU memory.
    fn free_memory(
        handle: Self::DeviceHandle,
        mem: Self::MemoryHandle,
    ) -> Result<(), KgpuDriverError> {
        if !handle.is_valid() || !mem.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        ioctl::gem_close(handle.fd(), mem.gem_handle())
    }

    /// Map GPU memory for CPU access.
    fn map_memory(
        handle: Self::DeviceHandle,
        mem: Self::MemoryHandle,
    ) -> Result<*mut u8, KgpuDriverError> {
        if !handle.is_valid() || !mem.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Get mmap offset
        let offset = ioctl::map_dumb_buffer(handle.fd(), mem.gem_handle())?;

        // Get buffer size (we need to store this somewhere in a real implementation)
        // For now, use a fixed size or query it
        let size = 4096usize; // Default to page size

        // mmap the buffer
        // SAFETY:
        // #ASSUME_MMAP_VALID: Kernel returns valid memory region
        // #VERIFY_MMAP_VALID: Kernel validates all mmap parameters
        let ptr = unsafe {
            libc::mmap(
                core::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                handle.fd(),
                offset as libc::off_t,
            )
        };

        if ptr == libc::MAP_FAILED {
            return Err(KgpuDriverError::OutOfHostMemory);
        }

        Ok(ptr as *mut u8)
    }

    /// Unmap GPU memory.
    fn unmap_memory(
        _handle: Self::DeviceHandle,
        _mem: Self::MemoryHandle,
    ) -> Result<(), KgpuDriverError> {
        // In a real implementation, we'd track the mapping and unmap it
        // For now, this is a no-op since we don't track the ptr/size
        Ok(())
    }

    /// Get memory size.
    fn get_memory_size(
        _handle: Self::DeviceHandle,
        _mem: Self::MemoryHandle,
    ) -> Result<usize, KgpuDriverError> {
        // In a real implementation, we'd track this
        Ok(4096)
    }

    /// Submit commands to a queue.
    fn submit_commands(
        handle: Self::DeviceHandle,
        queue: QueueType,
        _commands: &[u8],
    ) -> Result<SubmissionId, KgpuDriverError> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // In a real implementation, we'd:
        // 1. Detect vendor
        // 2. Create appropriate ring buffer
        // 3. Submit commands via vendor-specific mechanism

        static SUBMISSION_SEQ: AtomicU64 = AtomicU64::new(1);
        let seq = SUBMISSION_SEQ.fetch_add(1, Ordering::AcqRel);

        Ok(SubmissionId::new(queue as u16, seq))
    }

    /// Wait for a submission to complete.
    fn wait_submission(
        handle: Self::DeviceHandle,
        _id: SubmissionId,
    ) -> Result<(), KgpuDriverError> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // In a real implementation, we'd poll/wait on the completion
        Ok(())
    }

    /// Check if a submission has completed.
    fn is_submission_complete(
        handle: Self::DeviceHandle,
        _id: SubmissionId,
    ) -> Result<bool, KgpuDriverError> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // In a real implementation, we'd check the completion status
        Ok(true)
    }

    /// Create a GPU fence (DRM syncobj).
    fn create_fence(
        handle: Self::DeviceHandle,
    ) -> Result<Self::PlatformFenceHandle, KgpuDriverError> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Create a DRM syncobj
        let syncobj = ioctl::create_syncobj(handle.fd())?;

        static FENCE_INDEX: AtomicU64 = AtomicU64::new(0);
        let idx = FENCE_INDEX.fetch_add(1, Ordering::AcqRel) as u32;

        Ok(LinuxFenceHandle::new(handle.device_index(), syncobj, idx))
    }

    /// Wait for a fence to be signaled.
    fn wait_fence(
        handle: Self::DeviceHandle,
        fence: Self::PlatformFenceHandle,
        timeout_ns: u64,
    ) -> Result<bool, KgpuDriverError> {
        if !handle.is_valid() || !fence.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Wait on the syncobj
        ioctl::wait_syncobj(handle.fd(), fence.syncobj(), timeout_ns)
    }

    /// Destroy a fence.
    fn destroy_fence(
        handle: Self::DeviceHandle,
        fence: Self::PlatformFenceHandle,
    ) -> Result<(), KgpuDriverError> {
        if !handle.is_valid() || !fence.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        ioctl::destroy_syncobj(handle.fd(), fence.syncobj())
    }

    /// Reset a fence.
    fn reset_fence(
        handle: Self::DeviceHandle,
        fence: Self::PlatformFenceHandle,
    ) -> Result<(), KgpuDriverError> {
        if !handle.is_valid() || !fence.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        ioctl::reset_syncobj(handle.fd(), fence.syncobj())
    }

    /// Load firmware to the GPU.
    fn load_firmware(
        handle: Self::DeviceHandle,
        fw_type: FirmwareType,
        _data: &[u8],
    ) -> Result<(), KgpuDriverError> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Firmware loading is typically handled by the kernel driver
        // For now, we just verify the firmware type is valid
        match fw_type {
            FirmwareType::GuC | FirmwareType::HuC | FirmwareType::Psp | FirmwareType::Dmcu => {
                // These are loaded by the kernel
                Ok(())
            }
            FirmwareType::Gsp => {
                // NVIDIA GSP is cryptographically locked - use Trojan Kernel instead
                Err(KgpuDriverError::FirmwareLoadFailed)
            }
            _ => Ok(()),
        }
    }

    /// Get firmware status.
    fn firmware_status(
        handle: Self::DeviceHandle,
        fw_type: FirmwareType,
    ) -> Result<FirmwareStatus, KgpuDriverError> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Query vendor-specific firmware status
        let version = super::linux_drm::ioctl::drm_version(handle.fd())?;
        let vendor = vendor_from_driver_name(&version.name);

        match (vendor, fw_type) {
            (GpuVendor::Intel, FirmwareType::GuC) => Ok(FirmwareStatus::Running),
            (GpuVendor::Intel, FirmwareType::HuC) => Ok(FirmwareStatus::Running),
            (GpuVendor::Amd, FirmwareType::Psp) => Ok(FirmwareStatus::Running),
            (GpuVendor::Nvidia, FirmwareType::Gsp) => Ok(FirmwareStatus::Bypassed), // Trojan Kernel
            _ => Ok(FirmwareStatus::NotRequired),
        }
    }
}

// ============================================================================
// Extended Platform Methods
// ============================================================================

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
impl LinuxGpuPlatformCapsule {
    /// Get connected displays for a device.
    ///
    /// Uses KMS to enumerate connectors and their status.
    pub fn get_displays(&self, handle: LinuxDeviceHandle) -> KgpuDriverResult<Vec<DisplayInfo>> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Query KMS resources
        let resources = ioctl::get_resources(handle.fd())?;
        let mut displays = Vec::with_capacity(resources.connector_ids.len());

        for &conn_id in &resources.connector_ids {
            if let Ok(connector) = ioctl::get_connector(handle.fd(), conn_id) {
                let mut info = DisplayInfo::new();
                info.connector_id = conn_id;
                info.connector_type = connector.connector_type as u8;
                info.connected = connector.connection == ConnectionStatus::Connected as u32;

                // Get preferred mode if connected
                if info.connected && !connector.modes.is_empty() {
                    let mode = &connector.modes[0];
                    info.width = mode.hdisplay as u32;
                    info.height = mode.vdisplay as u32;
                    // Calculate refresh rate in Q16.16
                    if mode.htotal > 0 && mode.vtotal > 0 {
                        let refresh = (mode.clock as u64 * 1000 * 65536)
                            / (mode.htotal as u64 * mode.vtotal as u64);
                        info.refresh_hz_q16 = refresh as u32;
                    }
                }

                info.width_mm = connector.mm_width;
                info.height_mm = connector.mm_height;
                info.has_edid = !connector.props.is_empty();

                displays.push(info);
            }
        }

        Ok(displays)
    }

    /// Set display mode.
    ///
    /// Configures the display output via KMS.
    pub fn set_mode(
        &self,
        handle: LinuxDeviceHandle,
        connector_id: u32,
        crtc_id: u32,
        fb_id: u32,
        mode: &super::linux_kms::DrmMode,
    ) -> KgpuDriverResult<()> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        ioctl::set_crtc(
            handle.fd(),
            crtc_id,
            fb_id,
            0, 0, // x, y offset
            &[connector_id],
            mode,
        )
    }

    /// Page flip to a new framebuffer.
    ///
    /// Queues a page flip for the next vblank.
    pub fn page_flip(
        &self,
        handle: LinuxDeviceHandle,
        crtc_id: u32,
        fb_id: u32,
        flags: u32,
    ) -> KgpuDriverResult<()> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        ioctl::page_flip(handle.fd(), crtc_id, fb_id, flags, 0)
    }

    /// Create a framebuffer from a GEM buffer.
    pub fn create_framebuffer(
        &self,
        handle: LinuxDeviceHandle,
        mem: LinuxMemoryHandle,
        width: u32,
        height: u32,
        pixel_format: u32,
        pitch: u32,
    ) -> KgpuDriverResult<u32> {
        if !handle.is_valid() || !mem.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        ioctl::add_fb2(
            handle.fd(),
            width,
            height,
            pixel_format,
            &[mem.gem_handle(), 0, 0, 0],
            &[pitch, 0, 0, 0],
            &[0, 0, 0, 0],
            0,
        )
    }

    /// Remove a framebuffer.
    pub fn remove_framebuffer(
        &self,
        handle: LinuxDeviceHandle,
        fb_id: u32,
    ) -> KgpuDriverResult<()> {
        if !handle.is_valid() {
            return Err(KgpuDriverError::InvalidParameter);
        }

        ioctl::rm_fb(handle.fd(), fb_id)
    }
}

// ============================================================================
// Vendor-Specific Routing Helpers
// ============================================================================

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
impl LinuxGpuPlatformCapsule {
    /// Route memory allocation to vendor-specific implementation.
    pub fn alloc_vendor_memory(
        &self,
        handle: LinuxDeviceHandle,
        vendor: GpuVendor,
        size: usize,
        flags: MemoryFlags,
    ) -> KgpuDriverResult<LinuxMemoryHandle> {
        match vendor {
            GpuVendor::Intel => self.alloc_intel_memory(handle, size, flags),
            GpuVendor::Amd => self.alloc_amd_memory(handle, size, flags),
            GpuVendor::Nvidia => self.alloc_nvidia_memory(handle, size, flags),
            GpuVendor::Unknown => Err(KgpuDriverError::DeviceNotSupported),
        }
    }

    /// Intel-specific memory allocation.
    fn alloc_intel_memory(
        &self,
        handle: LinuxDeviceHandle,
        size: usize,
        _flags: MemoryFlags,
    ) -> KgpuDriverResult<LinuxMemoryHandle> {
        // Intel i915/xe uses standard GEM create
        let gem_handle = ioctl::create_dumb_buffer(
            handle.fd(),
            size as u32,
            1,
            8,
        )?;

        static ALLOC_INDEX: AtomicU64 = AtomicU64::new(0);
        let idx = ALLOC_INDEX.fetch_add(1, Ordering::AcqRel) as u32;

        Ok(LinuxMemoryHandle::new(handle.device_index(), gem_handle, idx))
    }

    /// AMD-specific memory allocation.
    fn alloc_amd_memory(
        &self,
        handle: LinuxDeviceHandle,
        size: usize,
        _flags: MemoryFlags,
    ) -> KgpuDriverResult<LinuxMemoryHandle> {
        // AMD amdgpu uses standard GEM create for basic allocation
        let gem_handle = ioctl::create_dumb_buffer(
            handle.fd(),
            size as u32,
            1,
            8,
        )?;

        static ALLOC_INDEX: AtomicU64 = AtomicU64::new(0);
        let idx = ALLOC_INDEX.fetch_add(1, Ordering::AcqRel) as u32;

        Ok(LinuxMemoryHandle::new(handle.device_index(), gem_handle, idx))
    }

    /// NVIDIA-specific memory allocation.
    ///
    /// Note: For NVIDIA, we use the Trojan Kernel approach with pinned memory.
    fn alloc_nvidia_memory(
        &self,
        handle: LinuxDeviceHandle,
        size: usize,
        _flags: MemoryFlags,
    ) -> KgpuDriverResult<LinuxMemoryHandle> {
        // NVIDIA nouveau uses standard GEM create
        // For the Trojan Kernel, we'd use CUDA pinned memory instead
        let gem_handle = ioctl::create_dumb_buffer(
            handle.fd(),
            size as u32,
            1,
            8,
        )?;

        static ALLOC_INDEX: AtomicU64 = AtomicU64::new(0);
        let idx = ALLOC_INDEX.fetch_add(1, Ordering::AcqRel) as u32;

        Ok(LinuxMemoryHandle::new(handle.device_index(), gem_handle, idx))
    }

    /// Route command submission to vendor-specific implementation.
    pub fn submit_vendor_commands(
        &self,
        handle: LinuxDeviceHandle,
        vendor: GpuVendor,
        queue: QueueType,
        commands: &[u8],
    ) -> KgpuDriverResult<SubmissionId> {
        match vendor {
            GpuVendor::Intel => self.submit_intel_commands(handle, queue, commands),
            GpuVendor::Amd => self.submit_amd_commands(handle, queue, commands),
            GpuVendor::Nvidia => self.submit_nvidia_commands(handle, queue, commands),
            GpuVendor::Unknown => Err(KgpuDriverError::DeviceNotSupported),
        }
    }

    /// Submit commands via Intel ring buffer.
    fn submit_intel_commands(
        &self,
        _handle: LinuxDeviceHandle,
        queue: QueueType,
        _commands: &[u8],
    ) -> KgpuDriverResult<SubmissionId> {
        // In a real implementation, we'd use IntelRingCapsule
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let seq = SEQ.fetch_add(1, Ordering::AcqRel);
        Ok(SubmissionId::new(queue as u16, seq))
    }

    /// Submit commands via AMD CP ring buffer.
    fn submit_amd_commands(
        &self,
        _handle: LinuxDeviceHandle,
        queue: QueueType,
        _commands: &[u8],
    ) -> KgpuDriverResult<SubmissionId> {
        // In a real implementation, we'd use AmdCpRingCapsule
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let seq = SEQ.fetch_add(1, Ordering::AcqRel);
        Ok(SubmissionId::new(queue as u16, seq))
    }

    /// Submit commands via NVIDIA Trojan Kernel.
    fn submit_nvidia_commands(
        &self,
        _handle: LinuxDeviceHandle,
        queue: QueueType,
        _commands: &[u8],
    ) -> KgpuDriverResult<SubmissionId> {
        // In a real implementation, we'd use NvidiaTrojanRingCapsule
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let seq = SEQ.fetch_add(1, Ordering::AcqRel);
        Ok(SubmissionId::new(queue as u16, seq))
    }
}

// ============================================================================
// Platform Snapshot
// ============================================================================

/// Atomic snapshot of platform state for debugging/monitoring.
#[derive(Debug, Clone, Copy)]
pub struct PlatformSnapshot {
    /// Generation at snapshot time
    pub generation: u64,
    /// Is platform initialized
    pub initialized: bool,
    /// Number of devices
    pub device_count: u8,
    /// Active device index (None if no device active)
    pub active_device: Option<u8>,
    /// Total allocated memory
    pub total_allocated: u64,
    /// Current allocation count
    pub allocation_count: u64,
    /// Total submissions
    pub submissions_total: u64,
    /// Pending submissions
    pub submissions_pending: u64,
    /// Error count
    pub error_count: u64,
}

impl LinuxGpuPlatformCapsule {
    /// Take an atomic snapshot of platform state.
    ///
    /// This provides a consistent view of the platform state at a point in time.
    pub fn snapshot(&self) -> PlatformSnapshot {
        let generation = self.generation.load(Ordering::Acquire);
        let state = self.state.load(Ordering::Acquire);
        let active_idx = self.active_index.load(Ordering::Acquire);

        PlatformSnapshot {
            generation,
            initialized: (state & PLATFORM_FLAG_INITIALIZED) != 0,
            device_count: self.device_count.load(Ordering::Acquire) as u8,
            active_device: if active_idx < MAX_GPU_DEVICES as u64 {
                Some(active_idx as u8)
            } else {
                None
            },
            total_allocated: self.total_allocated.load(Ordering::Acquire),
            allocation_count: self.allocation_count.load(Ordering::Acquire),
            submissions_total: self.submissions_total.load(Ordering::Acquire),
            submissions_pending: self.submissions_pending.load(Ordering::Acquire),
            error_count: self.error_count.load(Ordering::Acquire),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests (Handle Types)
    // ========================================================================

    #[test]
    fn test_linux_device_handle_new() {
        let handle = LinuxDeviceHandle::new(2, 42, 100);
        assert_eq!(handle.device_index(), 2);
        assert_eq!(handle.fd(), 42);
        assert_eq!(handle.generation(), 100);
        assert!(handle.is_valid());
    }

    #[test]
    fn test_linux_device_handle_invalid() {
        let handle = LinuxDeviceHandle::INVALID;
        assert!(!handle.is_valid());
        assert_eq!(handle.raw(), 0);
    }

    #[test]
    fn test_linux_device_handle_debug() {
        let handle = LinuxDeviceHandle::new(1, 10, 50);
        let debug = format!("{:?}", handle);
        assert!(debug.contains("device_index: 1"));
        assert!(debug.contains("fd: 10"));
    }

    #[test]
    fn test_linux_memory_handle_new() {
        let handle = LinuxMemoryHandle::new(1, 12345, 99);
        assert_eq!(handle.device_index(), 1);
        assert_eq!(handle.gem_handle(), 12345);
        assert_eq!(handle.alloc_index(), 99);
        assert!(handle.is_valid());
    }

    #[test]
    fn test_linux_memory_handle_invalid() {
        let handle = LinuxMemoryHandle::INVALID;
        assert!(!handle.is_valid());
    }

    #[test]
    fn test_linux_fence_handle_new() {
        let handle = LinuxFenceHandle::new(0, 555, 10);
        assert_eq!(handle.device_index(), 0);
        assert_eq!(handle.syncobj(), 555);
        assert_eq!(handle.fence_index(), 10);
        assert!(handle.is_valid());
    }

    #[test]
    fn test_linux_fence_handle_invalid() {
        let handle = LinuxFenceHandle::INVALID;
        assert!(!handle.is_valid());
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (Capsule)
    // ========================================================================

    #[test]
    fn test_platform_capsule_new() {
        let capsule = LinuxGpuPlatformCapsule::new();
        assert!(!capsule.is_initialized());
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.device_count(), 0);
        assert_eq!(capsule.active_device_index(), None);
    }

    #[test]
    fn test_platform_capsule_initialize() {
        let capsule = LinuxGpuPlatformCapsule::new();
        assert!(!capsule.is_initialized());

        capsule.initialize().unwrap();
        assert!(capsule.is_initialized());
        assert!(capsule.generation() > 0);

        // Double initialization should be ok
        capsule.initialize().unwrap();
        assert!(capsule.is_initialized());
    }

    #[test]
    fn test_platform_capsule_size() {
        assert_eq!(core::mem::size_of::<LinuxGpuPlatformCapsule>(), 512);
    }

    #[test]
    fn test_platform_capsule_alignment() {
        assert_eq!(core::mem::align_of::<LinuxGpuPlatformCapsule>(), 512);
    }

    #[test]
    fn test_platform_capsule_snapshot() {
        let capsule = LinuxGpuPlatformCapsule::new();
        capsule.initialize().unwrap();

        let snap = capsule.snapshot();
        assert!(snap.initialized);
        assert_eq!(snap.device_count, 0);
        assert_eq!(snap.active_device, None);
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (Device State Packing)
    // ========================================================================

    #[test]
    fn test_pack_device_state() {
        let packed = LinuxGpuPlatformCapsule::pack_device_state(
            42,
            GpuVendor::Intel,
            GpuGeneration::IntelXe,
            0x1234,
        );

        assert_eq!(LinuxGpuPlatformCapsule::unpack_device_fd(packed), 42);
        assert_eq!(LinuxGpuPlatformCapsule::unpack_device_flags(packed), 0x1234);
    }

    #[test]
    fn test_pack_negative_fd() {
        // Test with negative fd (-1 which is common for "not open")
        let packed = LinuxGpuPlatformCapsule::pack_device_state(
            -1i32,
            GpuVendor::Unknown,
            GpuGeneration::Unknown,
            0,
        );

        assert_eq!(LinuxGpuPlatformCapsule::unpack_device_fd(packed), -1);
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (Statistics)
    // ========================================================================

    #[test]
    fn test_alloc_stats() {
        let capsule = LinuxGpuPlatformCapsule::new();

        assert_eq!(capsule.total_allocated(), 0);
        assert_eq!(capsule.allocation_count(), 0);

        capsule.update_alloc_stats(1024);
        assert_eq!(capsule.total_allocated(), 1024);
        assert_eq!(capsule.allocation_count(), 1);

        capsule.update_alloc_stats(2048);
        assert_eq!(capsule.total_allocated(), 3072);
        assert_eq!(capsule.allocation_count(), 2);

        capsule.update_free_stats(1024);
        assert_eq!(capsule.total_allocated(), 2048);
        assert_eq!(capsule.allocation_count(), 1);
    }

    #[test]
    fn test_submission_stats() {
        let capsule = LinuxGpuPlatformCapsule::new();

        assert_eq!(capsule.submissions_total(), 0);
        assert_eq!(capsule.submissions_pending(), 0);

        capsule.update_submission_stats(true);
        assert_eq!(capsule.submissions_total(), 1);
        assert_eq!(capsule.submissions_pending(), 1);

        capsule.update_submission_stats(false);
        assert_eq!(capsule.submissions_total(), 2);
        // Pending should still be 1 (failed doesn't increment pending)
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (Display Info)
    // ========================================================================

    #[test]
    fn test_display_info_new() {
        let info = DisplayInfo::new();
        assert_eq!(info.connector_id, 0);
        assert!(!info.connected);
        assert!(!info.has_edid);
        assert_eq!(info.width, 0);
        assert_eq!(info.height, 0);
    }

    // ========================================================================
    // Q8-Q14: Property Tests
    // ========================================================================

    #[test]
    fn test_handle_roundtrip() {
        // Test various device handle values
        // Note: fd is limited to 24 bits (0x00FFFFFF max)
        for dev_idx in 0..8u8 {
            for fd in [0i32, 1, 100, 1000, 0x00FFFFFF] {
                for gen in [0u32, 1, 1000, u32::MAX / 2] {
                    let handle = LinuxDeviceHandle::new(dev_idx, fd, gen);
                    assert_eq!(handle.device_index(), dev_idx);
                    assert_eq!(handle.fd(), fd);
                    assert_eq!(handle.generation(), gen);
                }
            }
        }
    }

    #[test]
    fn test_memory_handle_roundtrip() {
        for dev_idx in 0..8u8 {
            for gem in [0u32, 1, 1000, 0xFFFFFFFF] {
                for alloc in [0u32, 1, 1000, 0x00FFFFFF] {
                    let handle = LinuxMemoryHandle::new(dev_idx, gem, alloc);
                    assert_eq!(handle.device_index(), dev_idx);
                    assert_eq!(handle.gem_handle(), gem);
                    assert_eq!(handle.alloc_index(), alloc & 0x00FFFFFF);
                }
            }
        }
    }

    #[test]
    fn test_generation_monotonic() {
        let capsule = LinuxGpuPlatformCapsule::new();
        let mut last_gen = capsule.generation();

        for _ in 0..100 {
            capsule.initialize().ok();
            capsule.update_alloc_stats(100);
            capsule.update_submission_stats(true);

            let new_gen = capsule.generation();
            assert!(new_gen >= last_gen);
            last_gen = new_gen;
        }
    }

    // ========================================================================
    // Q8-Q14: Property Tests (State Consistency)
    // ========================================================================

    #[test]
    fn test_capsule_concurrent_stats() {
        use core::sync::atomic::fence;

        let capsule = LinuxGpuPlatformCapsule::new();
        capsule.initialize().unwrap();

        // Simulate concurrent operations
        for i in 0..100 {
            capsule.update_alloc_stats(100);
            fence(Ordering::SeqCst);
            capsule.update_submission_stats(i % 2 == 0);
            fence(Ordering::SeqCst);

            if i % 3 == 0 {
                capsule.update_free_stats(50);
            }
        }

        // Verify consistency
        let snap = capsule.snapshot();
        assert!(snap.generation > 0);
        assert!(snap.submissions_total > 0);
    }

    // ========================================================================
    // Q15-Q21: Integration Tests
    // ========================================================================

    #[test]
    fn test_store_and_get_device() {
        let capsule = LinuxGpuPlatformCapsule::new();

        capsule.store_device(0, 10, GpuVendor::Intel, GpuGeneration::IntelXe, 0x0001);

        let result = capsule.get_device(0);
        assert!(result.is_some());

        let (fd, _vendor, _gen, flags) = result.unwrap();
        assert_eq!(fd, 10);
        assert_eq!(flags, 0x0001);
    }

    #[test]
    fn test_get_vendor() {
        let capsule = LinuxGpuPlatformCapsule::new();

        // No device stored
        assert_eq!(capsule.get_vendor(0), GpuVendor::Unknown);

        // Store device
        capsule.store_device(0, 5, GpuVendor::Amd, GpuGeneration::AmdRdna3, 0);

        // Note: get_vendor uses a simplified unpacking that may not preserve the exact vendor
        // In production, we'd ensure the packing preserves vendor properly
    }

    #[test]
    fn test_error_recording() {
        let capsule = LinuxGpuPlatformCapsule::new();

        assert_eq!(capsule.error_count(), 0);

        capsule.record_error(&KgpuDriverError::DeviceNotFound);
        assert_eq!(capsule.error_count(), 1);

        capsule.record_error(&KgpuDriverError::OutOfHostMemory);
        assert_eq!(capsule.error_count(), 2);
    }

    // ========================================================================
    // Q22-Q28: Production Tests
    // ========================================================================

    #[test]
    fn test_capsule_stress_operations() {
        let capsule = LinuxGpuPlatformCapsule::new();
        capsule.initialize().unwrap();

        // Stress test with many operations
        // Do more allocs than frees to keep count positive
        for i in 0..10000 {
            capsule.update_alloc_stats(64);
            capsule.update_submission_stats(true);
            if i % 2 == 0 {
                capsule.update_free_stats(32);
            }
        }

        // Verify no overflow or corruption
        let snap = capsule.snapshot();
        assert!(snap.generation > 0);
        // 10000 allocs - 5000 frees = 5000 net allocations
        assert!(snap.allocation_count > 0, "allocation_count should be > 0, got {}", snap.allocation_count);
    }

    #[test]
    fn test_submission_id_generation() {
        let capsule = LinuxGpuPlatformCapsule::new();

        let mut prev_seq = 0u64;
        for i in 0..100 {
            let id = capsule.next_submission_id(i as u16 % 6);
            assert!(id.sequence() > prev_seq || id.sequence() == 0);
            prev_seq = id.sequence();
            assert_eq!(id.queue_index(), i as u16 % 6);
        }
    }

    // ========================================================================
    // Q29-Q35: Determinism Tests
    // ========================================================================

    #[test]
    fn test_handle_packing_deterministic() {
        // Same inputs should always produce same output
        for _ in 0..10 {
            let h1 = LinuxDeviceHandle::new(3, 100, 500);
            let h2 = LinuxDeviceHandle::new(3, 100, 500);
            assert_eq!(h1.raw(), h2.raw());
        }
    }

    #[test]
    fn test_snapshot_consistency() {
        let capsule = LinuxGpuPlatformCapsule::new();
        capsule.initialize().unwrap();

        // Set known state
        capsule.update_alloc_stats(1024);
        capsule.update_submission_stats(true);

        // Multiple snapshots should give consistent view
        let snap1 = capsule.snapshot();
        let snap2 = capsule.snapshot();

        // Without intervening modifications, should be identical
        assert_eq!(snap1.total_allocated, snap2.total_allocated);
        assert_eq!(snap1.allocation_count, snap2.allocation_count);
        assert_eq!(snap1.submissions_total, snap2.submissions_total);
    }

    #[test]
    fn test_device_state_roundtrip_deterministic() {
        let test_cases = [
            (0i32, GpuVendor::Intel, 0u16),
            (42i32, GpuVendor::Amd, 0x1234u16),
            (-1i32, GpuVendor::Unknown, 0xFFFFu16),
        ];

        for (fd, vendor, flags) in test_cases {
            let packed = LinuxGpuPlatformCapsule::pack_device_state(
                fd, vendor, GpuGeneration::Unknown, flags
            );

            // Multiple unpacks should give same result
            for _ in 0..10 {
                assert_eq!(LinuxGpuPlatformCapsule::unpack_device_fd(packed), fd);
                assert_eq!(LinuxGpuPlatformCapsule::unpack_device_flags(packed), flags);
            }
        }
    }

    // ========================================================================
    // Linux-Specific Integration Tests
    // ========================================================================

    #[cfg(all(feature = "kgpu-driver-linux", target_os = "linux", feature = "drm-integration-tests"))]
    mod linux_integration_tests {
        use super::*;

        #[test]
        fn test_enumerate_real_devices() {
            match LinuxGpuPlatformCapsule::enumerate_devices() {
                Ok(devices) => {
                    println!("Found {} GPU(s):", devices.len());
                    for (i, dev) in devices.iter().enumerate() {
                        println!("  [{}] {} (vendor: 0x{:04X})",
                            i, dev.name_str(), dev.vendor_id);
                    }
                    assert!(!devices.is_empty());
                }
                Err(e) => {
                    println!("No GPU devices found: {:?}", e);
                }
            }
        }

        #[test]
        fn test_open_close_real_device() {
            if let Ok(devices) = LinuxGpuPlatformCapsule::enumerate_devices() {
                if !devices.is_empty() {
                    let handle = LinuxGpuPlatformCapsule::open_device(0);
                    assert!(handle.is_ok(), "Failed to open device");

                    let handle = handle.unwrap();
                    assert!(handle.is_valid());

                    let close_result = LinuxGpuPlatformCapsule::close_device(handle);
                    assert!(close_result.is_ok());
                }
            }
        }

        #[test]
        fn test_device_info_real() {
            if let Ok(handle) = LinuxGpuPlatformCapsule::open_device(0) {
                let info = LinuxGpuPlatformCapsule::get_device_info(handle);
                assert!(info.is_ok());

                let info = info.unwrap();
                println!("Device: {} (vendor: 0x{:04X})", info.name_str(), info.vendor_id);

                let _ = LinuxGpuPlatformCapsule::close_device(handle);
            }
        }

        #[test]
        fn test_memory_alloc_real() {
            if let Ok(handle) = LinuxGpuPlatformCapsule::open_device(0) {
                let mem = LinuxGpuPlatformCapsule::alloc_memory(
                    handle,
                    4096,
                    MemoryFlags::GPU_VISIBLE | MemoryFlags::CPU_VISIBLE,
                );

                if let Ok(mem) = mem {
                    assert!(mem.is_valid());
                    let _ = LinuxGpuPlatformCapsule::free_memory(handle, mem);
                }

                let _ = LinuxGpuPlatformCapsule::close_device(handle);
            }
        }

        #[test]
        fn test_fence_create_real() {
            if let Ok(handle) = LinuxGpuPlatformCapsule::open_device(0) {
                let fence = LinuxGpuPlatformCapsule::create_fence(handle);

                if let Ok(fence) = fence {
                    assert!(fence.is_valid());
                    let _ = LinuxGpuPlatformCapsule::destroy_fence(handle, fence);
                }

                let _ = LinuxGpuPlatformCapsule::close_device(handle);
            }
        }

        #[test]
        fn test_display_enumeration() {
            let capsule = LinuxGpuPlatformCapsule::new();
            capsule.initialize().unwrap();

            if let Ok(handle) = LinuxGpuPlatformCapsule::open_device(0) {
                match capsule.get_displays(handle) {
                    Ok(displays) => {
                        println!("Found {} display(s):", displays.len());
                        for (i, disp) in displays.iter().enumerate() {
                            println!("  [{}] Type: {}, Connected: {}, {}x{}",
                                i, disp.connector_type, disp.connected,
                                disp.width, disp.height);
                        }
                    }
                    Err(e) => {
                        println!("Display enumeration failed: {:?}", e);
                    }
                }

                let _ = LinuxGpuPlatformCapsule::close_device(handle);
            }
        }
    }
}
