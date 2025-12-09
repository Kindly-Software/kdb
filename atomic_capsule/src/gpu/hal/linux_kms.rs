//! Linux KMS (Kernel Mode Setting) Display Management
//!
//! Implements display output configuration and page flipping via DRM/KMS.
//! Connects to DisplayEngineCapsule for lockfree display state tracking.
//!
//! # Design
//!
//! **Tier**: T1 Atomic (lockfree coordination) + T5 Streaming (vsync events)
//! **Portability**: Linux-only (feature-gated: `linux-gpu`)
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    KMS Display Stack                             │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐    │
//! │  │   Connector    │  │     CRTC       │  │     Plane      │    │
//! │  │  (HDMI/DP/eDP) │  │ (Scanout ctrl) │  │  (Framebuffer) │    │
//! │  └───────┬────────┘  └───────┬────────┘  └───────┬────────┘    │
//! │          │                   │                   │              │
//! │          └───────────────────┼───────────────────┘              │
//! │                              │                                   │
//! │                              ▼                                   │
//! │  ┌──────────────────────────────────────────────────────────┐  │
//! │  │           KmsDisplay (Atomic modesetting)                 │  │
//! │  │  - Mode enumeration                                       │  │
//! │  │  - Atomic commits                                         │  │
//! │  │  - Page flipping                                          │  │
//! │  │  - Vblank synchronization                                 │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! │                              │                                   │
//! │                              ▼                                   │
//! │  ┌──────────────────────────────────────────────────────────┐  │
//! │  │           DisplayEngineCapsule                            │  │
//! │  │  Lockfree state tracking, vsync coordination             │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # KMS Concepts
//!
//! - **Connector**: Physical output (HDMI, DisplayPort, eDP, VGA)
//! - **Encoder**: Signal encoder (tied to connector)
//! - **CRTC**: Scanout controller (reads framebuffer, generates timing)
//! - **Plane**: Overlay/primary/cursor layer
//! - **Framebuffer**: Scanout buffer (GEM buffer object)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_KMS_MASTER`: DRM master required for modesetting
//! - `#ASSUME_CONNECTOR_VALID`: Connector ID valid after enumeration
//! - `#ASSUME_MODE_VALID`: Mode info valid from connector probing
//! - `#ASSUME_FB_BOUND`: Framebuffer bound to plane before flip
//!
//! # Examples
//!
//! ```ignore
//! use atomic_capsule::gpu::hal::linux_kms::{KmsDisplay, KmsMode};
//!
//! // Open display
//! let mut display = KmsDisplay::open("/dev/dri/card0")?;
//!
//! // List connectors
//! for conn in display.connectors() {
//!     println!("Connector {}: {:?}", conn.id, conn.connector_type);
//! }
//!
//! // Find preferred mode
//! let mode = display.preferred_mode(connector_id)?;
//! println!("Mode: {}x{}@{}Hz", mode.hdisplay, mode.vdisplay, mode.vrefresh);
//!
//! // Create framebuffer
//! let fb = display.create_framebuffer(mode.hdisplay, mode.vdisplay, 32)?;
//!
//! // Set mode
//! display.set_mode(connector_id, crtc_id, &mode, fb)?;
//!
//! // Page flip
//! display.page_flip(crtc_id, fb)?;
//! ```

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use core::ptr;

use super::linux_hal::{LinuxHalError, LinuxHalResult};

// ============================================================================
// DRM KMS IOCTL Definitions
// ============================================================================

const fn drm_iowr(nr: u32, size: u32) -> u64 {
    (3u64 << 30) | ((size as u64) << 16) | ((b'd' as u64) << 8) | (nr as u64)
}

const fn drm_iow(nr: u32, size: u32) -> u64 {
    (1u64 << 30) | ((size as u64) << 16) | ((b'd' as u64) << 8) | (nr as u64)
}

// KMS IOCTLs
const DRM_IOCTL_MODE_GETRESOURCES: u64 = drm_iowr(0xa0, 64);
const DRM_IOCTL_MODE_GETCONNECTOR: u64 = drm_iowr(0xa7, 80);
const DRM_IOCTL_MODE_GETCRTC: u64 = drm_iowr(0xa1, 72);
const DRM_IOCTL_MODE_SETCRTC: u64 = drm_iowr(0xa2, 72);
const DRM_IOCTL_MODE_ADDFB: u64 = drm_iowr(0xae, 28);
const DRM_IOCTL_MODE_RMFB: u64 = drm_iowr(0xaf, 4);
const DRM_IOCTL_MODE_PAGE_FLIP: u64 = drm_iowr(0xb0, 16);

// Page flip flags
const DRM_MODE_PAGE_FLIP_EVENT: u32 = 0x01;
const DRM_MODE_PAGE_FLIP_ASYNC: u32 = 0x02;

// Connector status
const DRM_MODE_CONNECTED: u32 = 1;
const DRM_MODE_DISCONNECTED: u32 = 2;
const DRM_MODE_UNKNOWNCONNECTION: u32 = 3;

// ============================================================================
// KMS Types
// ============================================================================

/// Connector type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ConnectorType {
    Unknown = 0,
    VGA = 1,
    DVII = 2,
    DVID = 3,
    DVIA = 4,
    Composite = 5,
    SVIDEO = 6,
    LVDS = 7,
    Component = 8,
    NinePinDIN = 9,
    DisplayPort = 10,
    HDMIA = 11,
    HDMIB = 12,
    TV = 13,
    EDP = 14,
    Virtual = 15,
    DSI = 16,
    DPI = 17,
    Writeback = 18,
    SPI = 19,
    USB = 20,
}

impl From<u32> for ConnectorType {
    fn from(val: u32) -> Self {
        match val {
            1 => Self::VGA,
            2 => Self::DVII,
            3 => Self::DVID,
            4 => Self::DVIA,
            5 => Self::Composite,
            6 => Self::SVIDEO,
            7 => Self::LVDS,
            8 => Self::Component,
            9 => Self::NinePinDIN,
            10 => Self::DisplayPort,
            11 => Self::HDMIA,
            12 => Self::HDMIB,
            13 => Self::TV,
            14 => Self::EDP,
            15 => Self::Virtual,
            16 => Self::DSI,
            17 => Self::DPI,
            18 => Self::Writeback,
            19 => Self::SPI,
            20 => Self::USB,
            _ => Self::Unknown,
        }
    }
}

/// Connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Unknown,
}

impl From<u32> for ConnectionStatus {
    fn from(val: u32) -> Self {
        match val {
            DRM_MODE_CONNECTED => Self::Connected,
            DRM_MODE_DISCONNECTED => Self::Disconnected,
            _ => Self::Unknown,
        }
    }
}

/// Display mode information
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct KmsMode {
    /// Pixel clock in kHz
    pub clock: u32,
    /// Horizontal display size
    pub hdisplay: u16,
    /// Horizontal sync start
    pub hsync_start: u16,
    /// Horizontal sync end
    pub hsync_end: u16,
    /// Horizontal total size
    pub htotal: u16,
    /// Horizontal skew
    pub hskew: u16,
    /// Vertical display size
    pub vdisplay: u16,
    /// Vertical sync start
    pub vsync_start: u16,
    /// Vertical sync end
    pub vsync_end: u16,
    /// Vertical total size
    pub vtotal: u16,
    /// Vertical scan
    pub vscan: u16,
    /// Vertical refresh rate
    pub vrefresh: u32,
    /// Mode flags
    pub flags: u32,
    /// Mode type (preferred, driver, userdef)
    pub mode_type: u32,
    /// Mode name (e.g., "1920x1080")
    pub name: [u8; 32],
}

impl Default for KmsMode {
    fn default() -> Self {
        Self {
            clock: 0,
            hdisplay: 0,
            hsync_start: 0,
            hsync_end: 0,
            htotal: 0,
            hskew: 0,
            vdisplay: 0,
            vsync_start: 0,
            vsync_end: 0,
            vtotal: 0,
            vscan: 0,
            vrefresh: 0,
            flags: 0,
            mode_type: 0,
            name: [0u8; 32],
        }
    }
}

impl KmsMode {
    /// Get mode name as string
    pub fn name_str(&self) -> &str {
        let len = self.name.iter().position(|&c| c == 0).unwrap_or(32);
        core::str::from_utf8(&self.name[..len]).unwrap_or("")
    }

    /// Check if this is the preferred mode
    pub fn is_preferred(&self) -> bool {
        (self.mode_type & 0x08) != 0 // DRM_MODE_TYPE_PREFERRED
    }
}

/// Connector information
#[derive(Debug, Clone)]
pub struct KmsConnector {
    /// Connector ID
    pub id: u32,
    /// Connector type
    pub connector_type: ConnectorType,
    /// Type index (e.g., HDMI-1, HDMI-2)
    pub connector_type_id: u32,
    /// Connection status
    pub status: ConnectionStatus,
    /// Physical width in mm
    pub mm_width: u32,
    /// Physical height in mm
    pub mm_height: u32,
    /// Available modes
    pub modes: Vec<KmsMode>,
    /// Associated encoder ID
    pub encoder_id: u32,
}

/// CRTC information
#[derive(Debug, Clone, Copy)]
pub struct KmsCrtc {
    /// CRTC ID
    pub id: u32,
    /// Current framebuffer ID
    pub fb_id: u32,
    /// X position
    pub x: u32,
    /// Y position
    pub y: u32,
    /// Current mode
    pub mode: KmsMode,
    /// Mode valid flag
    pub mode_valid: bool,
    /// Gamma size
    pub gamma_size: u32,
}

// ============================================================================
// IOCTL Argument Structures
// ============================================================================

/// Mode resources ioctl argument
#[repr(C)]
#[derive(Debug, Default)]
struct DrmModeResArg {
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

/// Connector ioctl argument
#[repr(C)]
#[derive(Debug)]
struct DrmModeConnectorArg {
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
    pad: u32,
}

impl Default for DrmModeConnectorArg {
    fn default() -> Self {
        // SAFETY: All fields are numeric types that can be zero-initialized
        unsafe { core::mem::zeroed() }
    }
}

/// CRTC ioctl argument
#[repr(C)]
#[derive(Debug, Default)]
struct DrmModeCrtcArg {
    set_connectors_ptr: u64,
    count_connectors: u32,
    crtc_id: u32,
    fb_id: u32,
    x: u32,
    y: u32,
    gamma_size: u32,
    mode_valid: u32,
    mode: KmsMode,
}

/// Add framebuffer ioctl argument
#[repr(C)]
#[derive(Debug, Default)]
struct DrmModeAddFbArg {
    width: u32,
    height: u32,
    pitch: u32,
    bpp: u32,
    depth: u32,
    handle: u32,
    fb_id: u32,
}

/// Page flip ioctl argument
#[repr(C)]
#[derive(Debug, Default)]
struct DrmModePageFlipArg {
    crtc_id: u32,
    fb_id: u32,
    flags: u32,
    reserved: u32,
    user_data: u64,
}

// ============================================================================
// KMS Display
// ============================================================================

/// KMS display controller
///
/// Manages display configuration and page flipping.
/// Thread-safe via atomic state tracking.
///
/// # Memory Layout (256B, 128B-aligned)
#[repr(C, align(128))]
pub struct KmsDisplay {
    /// DRM file descriptor
    fd: AtomicU32,
    /// State flags
    flags: AtomicU32,
    /// Generation counter
    gen_counter: AtomicU32,
    /// Active CRTC count
    active_crtcs: AtomicU32,
    /// Page flip counter
    flip_count: AtomicU64,
    /// Vblank counter (from kernel events)
    vblank_count: AtomicU64,
    /// Last flip timestamp (nanoseconds)
    last_flip_ns: AtomicU64,
    /// Cached resource counts
    count_crtcs: AtomicU32,
    count_connectors: AtomicU32,
    /// Padding to 128B
    _padding: [u8; 72],
}

// SAFETY: KmsDisplay uses atomic operations for all shared state
unsafe impl Send for KmsDisplay {}
unsafe impl Sync for KmsDisplay {}

impl KmsDisplay {
    /// Invalid file descriptor sentinel
    const INVALID_FD: u32 = u32::MAX;

    /// Flag: Device is open
    const FLAG_OPEN: u32 = 0x01;
    /// Flag: DRM master
    const FLAG_MASTER: u32 = 0x02;
    /// Flag: Atomic modesetting enabled
    const FLAG_ATOMIC: u32 = 0x04;

    /// Create uninitialized display
    #[inline]
    pub const fn uninit() -> Self {
        Self {
            fd: AtomicU32::new(Self::INVALID_FD),
            flags: AtomicU32::new(0),
            gen_counter: AtomicU32::new(0),
            active_crtcs: AtomicU32::new(0),
            flip_count: AtomicU64::new(0),
            vblank_count: AtomicU64::new(0),
            last_flip_ns: AtomicU64::new(0),
            count_crtcs: AtomicU32::new(0),
            count_connectors: AtomicU32::new(0),
            _padding: [0u8; 72],
        }
    }

    /// Open display device
    #[cfg(feature = "std")]
    pub fn open(path: &str) -> LinuxHalResult<Self> {
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

        let display = Self {
            fd: AtomicU32::new(fd as u32),
            flags: AtomicU32::new(Self::FLAG_OPEN),
            gen_counter: AtomicU32::new(1),
            active_crtcs: AtomicU32::new(0),
            flip_count: AtomicU64::new(0),
            vblank_count: AtomicU64::new(0),
            last_flip_ns: AtomicU64::new(0),
            count_crtcs: AtomicU32::new(0),
            count_connectors: AtomicU32::new(0),
            _padding: [0u8; 72],
        };

        // Probe resources
        let _ = display.probe_resources();

        Ok(display)
    }

    /// Close display
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

    /// Check if display is open
    #[inline]
    pub fn is_open(&self) -> bool {
        (self.flags.load(Ordering::Acquire) & Self::FLAG_OPEN) != 0
    }

    /// Probe KMS resources
    #[cfg(feature = "std")]
    fn probe_resources(&self) -> LinuxHalResult<()> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let mut res = DrmModeResArg::default();

        // First call to get counts
        // SAFETY: fd is valid, res is properly initialized
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_MODE_GETRESOURCES, &mut res as *mut _) };

        if ret < 0 {
            return Err(LinuxHalError::KmsModeFailed);
        }

        self.count_crtcs.store(res.count_crtcs, Ordering::Release);
        self.count_connectors.store(res.count_connectors, Ordering::Release);

        Ok(())
    }

    /// Get connector IDs
    #[cfg(feature = "std")]
    pub fn get_connector_ids(&self) -> LinuxHalResult<Vec<u32>> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let count = self.count_connectors.load(Ordering::Acquire);
        if count == 0 {
            return Ok(Vec::new());
        }

        let mut ids = vec![0u32; count as usize];
        let mut res = DrmModeResArg {
            connector_id_ptr: ids.as_mut_ptr() as u64,
            count_connectors: count,
            ..Default::default()
        };

        // SAFETY: fd is valid, res points to valid memory
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_MODE_GETRESOURCES, &mut res as *mut _) };

        if ret < 0 {
            return Err(LinuxHalError::KmsModeFailed);
        }

        ids.truncate(res.count_connectors as usize);
        Ok(ids)
    }

    /// Get CRTC IDs
    #[cfg(feature = "std")]
    pub fn get_crtc_ids(&self) -> LinuxHalResult<Vec<u32>> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let count = self.count_crtcs.load(Ordering::Acquire);
        if count == 0 {
            return Ok(Vec::new());
        }

        let mut ids = vec![0u32; count as usize];
        let mut res = DrmModeResArg {
            crtc_id_ptr: ids.as_mut_ptr() as u64,
            count_crtcs: count,
            ..Default::default()
        };

        // SAFETY: fd is valid, res points to valid memory
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_MODE_GETRESOURCES, &mut res as *mut _) };

        if ret < 0 {
            return Err(LinuxHalError::KmsModeFailed);
        }

        ids.truncate(res.count_crtcs as usize);
        Ok(ids)
    }

    /// Get connector information
    #[cfg(feature = "std")]
    pub fn get_connector(&self, connector_id: u32) -> LinuxHalResult<KmsConnector> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        // First call to get counts
        let mut arg = DrmModeConnectorArg {
            connector_id,
            ..Default::default()
        };

        // SAFETY: fd is valid, arg is properly initialized
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_MODE_GETCONNECTOR, &mut arg as *mut _) };

        if ret < 0 {
            return Err(LinuxHalError::KmsConnectorNotFound(connector_id));
        }

        // Allocate space for modes
        let mode_count = arg.count_modes as usize;
        let mut modes = vec![KmsMode::default(); mode_count];

        if mode_count > 0 {
            arg.modes_ptr = modes.as_mut_ptr() as u64;

            // SAFETY: fd is valid, modes pointer is valid
            let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_MODE_GETCONNECTOR, &mut arg as *mut _) };

            if ret < 0 {
                return Err(LinuxHalError::KmsConnectorNotFound(connector_id));
            }

            modes.truncate(arg.count_modes as usize);
        }

        Ok(KmsConnector {
            id: connector_id,
            connector_type: ConnectorType::from(arg.connector_type),
            connector_type_id: arg.connector_type_id,
            status: ConnectionStatus::from(arg.connection),
            mm_width: arg.mm_width,
            mm_height: arg.mm_height,
            modes,
            encoder_id: arg.encoder_id,
        })
    }

    /// Get preferred mode for a connector
    #[cfg(feature = "std")]
    pub fn preferred_mode(&self, connector_id: u32) -> LinuxHalResult<KmsMode> {
        let connector = self.get_connector(connector_id)?;

        // Find preferred mode
        connector
            .modes
            .iter()
            .find(|m| m.is_preferred())
            .or_else(|| connector.modes.first())
            .copied()
            .ok_or(LinuxHalError::KmsModeFailed)
    }

    /// Create framebuffer from GEM handle
    #[cfg(feature = "std")]
    pub fn create_framebuffer(
        &self,
        width: u32,
        height: u32,
        bpp: u32,
        gem_handle: u32,
    ) -> LinuxHalResult<u32> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let pitch = width * (bpp / 8);
        let depth = if bpp == 32 { 24 } else { bpp };

        let mut arg = DrmModeAddFbArg {
            width,
            height,
            pitch,
            bpp,
            depth,
            handle: gem_handle,
            fb_id: 0,
        };

        // SAFETY: fd is valid, arg is properly initialized
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_MODE_ADDFB, &mut arg as *mut _) };

        if ret < 0 {
            return Err(LinuxHalError::BufferAllocationFailed);
        }

        Ok(arg.fb_id)
    }

    /// Remove framebuffer
    #[cfg(feature = "std")]
    pub fn remove_framebuffer(&self, fb_id: u32) -> LinuxHalResult<()> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let mut arg = fb_id;

        // SAFETY: fd is valid
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_MODE_RMFB, &mut arg as *mut _) };

        if ret < 0 {
            return Err(LinuxHalError::InternalError);
        }

        Ok(())
    }

    /// Set CRTC mode
    #[cfg(feature = "std")]
    pub fn set_mode(
        &self,
        crtc_id: u32,
        connector_id: u32,
        mode: &KmsMode,
        fb_id: u32,
    ) -> LinuxHalResult<()> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let mut connectors = [connector_id];
        let mut arg = DrmModeCrtcArg {
            crtc_id,
            fb_id,
            x: 0,
            y: 0,
            mode_valid: 1,
            mode: *mode,
            set_connectors_ptr: connectors.as_mut_ptr() as u64,
            count_connectors: 1,
            gamma_size: 0,
        };

        // SAFETY: fd is valid, arg and connectors are valid
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_MODE_SETCRTC, &mut arg as *mut _) };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(LinuxHalError::IoctlFailed(errno));
        }

        self.active_crtcs.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Page flip (async buffer swap)
    #[cfg(feature = "std")]
    pub fn page_flip(&self, crtc_id: u32, fb_id: u32) -> LinuxHalResult<()> {
        let fd = self.fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let mut arg = DrmModePageFlipArg {
            crtc_id,
            fb_id,
            flags: DRM_MODE_PAGE_FLIP_EVENT,
            reserved: 0,
            user_data: 0,
        };

        // SAFETY: fd is valid, arg is properly initialized
        let ret = unsafe { libc::ioctl(fd as i32, DRM_IOCTL_MODE_PAGE_FLIP, &mut arg as *mut _) };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(LinuxHalError::PageFlipFailed(errno));
        }

        self.flip_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Get page flip count
    #[inline]
    pub fn flip_count(&self) -> u64 {
        self.flip_count.load(Ordering::Relaxed)
    }

    /// Get vblank count
    #[inline]
    pub fn vblank_count(&self) -> u64 {
        self.vblank_count.load(Ordering::Relaxed)
    }
}

impl Drop for KmsDisplay {
    fn drop(&mut self) {
        #[cfg(feature = "std")]
        {
            let _ = self.close();
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kms_display_uninit() {
        let display = KmsDisplay::uninit();
        assert!(!display.is_open());
    }

    #[test]
    fn test_kms_display_size_and_alignment() {
        assert!(core::mem::size_of::<KmsDisplay>() <= 256);
        assert_eq!(core::mem::align_of::<KmsDisplay>(), 128);
    }

    #[test]
    fn test_connector_type_conversion() {
        assert_eq!(ConnectorType::from(10), ConnectorType::DisplayPort);
        assert_eq!(ConnectorType::from(11), ConnectorType::HDMIA);
        assert_eq!(ConnectorType::from(14), ConnectorType::EDP);
        assert_eq!(ConnectorType::from(999), ConnectorType::Unknown);
    }

    #[test]
    fn test_connection_status_conversion() {
        assert_eq!(ConnectionStatus::from(1), ConnectionStatus::Connected);
        assert_eq!(ConnectionStatus::from(2), ConnectionStatus::Disconnected);
        assert_eq!(ConnectionStatus::from(99), ConnectionStatus::Unknown);
    }

    #[test]
    fn test_kms_mode_default() {
        let mode = KmsMode::default();
        assert_eq!(mode.hdisplay, 0);
        assert_eq!(mode.vdisplay, 0);
        assert!(!mode.is_preferred());
    }

    #[test]
    fn test_kms_mode_name() {
        let mut mode = KmsMode::default();
        mode.name[0..9].copy_from_slice(b"1920x1080");
        assert_eq!(mode.name_str(), "1920x1080");
    }
}
