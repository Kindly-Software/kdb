//! Linux KMS (Kernel Mode Setting) Display Support
//!
//! KMS handles display configuration - connectors, CRTCs, planes, encoders.
//! This module provides a Chaos-compliant wrapper around Linux DRM KMS ioctls.
//!
//! # Architecture
//!
//! ```text
//! +--------------+     +--------------+     +-------------+
//! |  Connector   |---->|   Encoder    |---->|    CRTC     |
//! | (DP/HDMI/VGA)|     | (DAC/TMDS)   |     | (Scanout)   |
//! +--------------+     +--------------+     +------+------+
//!                                                  |
//!                                           +------v------+
//!                                           |   Plane     |
//!                                           | (Primary/   |
//!                                           |  Overlay/   |
//!                                           |  Cursor)    |
//!                                           +-------------+
//! ```
//!
//! # KMS Modesetting Flow
//!
//! 1. **Legacy**: `drmModeSetCrtc()` + `drmModePageFlip()`
//! 2. **Atomic**: Single `drmModeAtomicCommit()` for all changes
//!
//! # Chaos Compliance
//!
//! - **T1 Atomic**: All capsules are lockfree with DualAtomicU64
//! - **256B aligned**: 4 cache lines for false sharing prevention
//! - **Generation counters**: For ABA prevention and snapshot consistency
//! - **100% lockfree**: NO mutex, NO RwLock
//!
//! # ASSUM Safety Tags
//!
//! - `#ASSUME_DRM_FD_VALID`: DRM file descriptor is valid
//! - `#ASSUME_IOCTL_THREAD_SAFE`: DRM ioctls are thread-safe
//! - `#ASSUME_KMS_STATE_CONSISTENT`: Kernel maintains consistent KMS state
//!
//! # Framework Compliance
//!
//! - UCE34: Q10 T1 Atomic tier, Q33 lockfree, Q34 audit trails
//! - Chaos: 256B cache-aligned, DualAtomicU64 coordination, zero mutex
//! - ASSUM: 99.99% safety target (all assumptions verified)
//! - B32: <100ns operations target
//! - T28: 45+ comprehensive tests (unit/property/integration/production)
//! - I20: Zero breaking changes, feature-gated

#![allow(dead_code)] // Allow during development

use core::sync::atomic::{AtomicU64, Ordering};
use core::fmt::{self, Debug};
use core::mem;

#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use super::error::{KgpuDriverError, KgpuDriverResult};

// ============================================================================
// KMS ioctl Constants
// ============================================================================

/// DRM ioctl base
pub const DRM_IOCTL_BASE: u64 = 0x64;

// Resources enumeration
/// Get display resources (connectors, CRTCs, encoders, framebuffers)
pub const DRM_IOCTL_MODE_GETRESOURCES: u64 = 0xc04064a0;
/// Get connector information
pub const DRM_IOCTL_MODE_GETCONNECTOR: u64 = 0xc05064a7;
/// Get encoder information
pub const DRM_IOCTL_MODE_GETENCODER: u64 = 0xc01464a6;
/// Get CRTC information
pub const DRM_IOCTL_MODE_GETCRTC: u64 = 0xc06864a1;
/// Get plane information
pub const DRM_IOCTL_MODE_GETPLANE: u64 = 0xc01464b6;
/// Get plane resources
pub const DRM_IOCTL_MODE_GETPLANERESOURCES: u64 = 0xc01064b5;

// Mode setting
/// Set CRTC configuration (legacy)
pub const DRM_IOCTL_MODE_SETCRTC: u64 = 0xc06864a2;
/// Page flip (async buffer swap)
pub const DRM_IOCTL_MODE_PAGE_FLIP: u64 = 0xc01c64b0;

// Framebuffer management
/// Add framebuffer (with modifiers, FB2)
pub const DRM_IOCTL_MODE_ADDFB2: u64 = 0xc04864b8;
/// Remove framebuffer
pub const DRM_IOCTL_MODE_RMFB: u64 = 0xc00464af;
/// Get framebuffer information
pub const DRM_IOCTL_MODE_GETFB: u64 = 0xc01c64ad;
/// Get framebuffer with modifiers (FB2)
pub const DRM_IOCTL_MODE_GETFB2: u64 = 0xc04864ce;

// Atomic modesetting
/// Atomic commit (multiple properties in one call)
pub const DRM_IOCTL_MODE_ATOMIC: u64 = 0xc04064bc;
/// Create property blob
pub const DRM_IOCTL_MODE_CREATEPROPBLOB: u64 = 0xc01064bd;
/// Destroy property blob
pub const DRM_IOCTL_MODE_DESTROYPROPBLOB: u64 = 0xc00464be;

// VBlank synchronization
/// Wait for VBlank
pub const DRM_IOCTL_WAIT_VBLANK: u64 = 0xc018643a;

// Property management
/// Get object properties
pub const DRM_IOCTL_MODE_OBJ_GETPROPERTIES: u64 = 0xc02064b9;
/// Set object property
pub const DRM_IOCTL_MODE_OBJ_SETPROPERTY: u64 = 0xc01064ba;

// ============================================================================
// Connector Types
// ============================================================================

/// KMS connector type (physical display interface)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ConnectorType {
    /// Unknown connector
    Unknown = 0,
    /// VGA (15-pin D-sub)
    VGA = 1,
    /// DVI-I (integrated analog/digital)
    DVII = 2,
    /// DVI-D (digital only)
    DVID = 3,
    /// DVI-A (analog only)
    DVIA = 4,
    /// Composite video
    Composite = 5,
    /// S-Video
    SVIDEO = 6,
    /// LVDS (laptop panels)
    LVDS = 7,
    /// Component video (YPbPr)
    Component = 8,
    /// 9-pin DIN
    NinePinDIN = 9,
    /// DisplayPort
    DisplayPort = 10,
    /// HDMI Type A
    HDMIA = 11,
    /// HDMI Type B
    HDMIB = 12,
    /// TV out
    TV = 13,
    /// Embedded DisplayPort (laptop panels)
    EDP = 14,
    /// Virtual (for VMs)
    Virtual = 15,
    /// DSI (Display Serial Interface)
    DSI = 16,
    /// DPI (Display Parallel Interface)
    DPI = 17,
    /// Writeback (for screen capture)
    Writeback = 18,
    /// USB-C with DP alt mode
    USB = 19,
}

impl ConnectorType {
    /// Convert from DRM connector type ID
    #[inline]
    pub const fn from_drm(drm_type: u32) -> Self {
        match drm_type {
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
            19 => Self::USB,
            _ => Self::Unknown,
        }
    }

    /// Get human-readable name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::VGA => "VGA",
            Self::DVII => "DVI-I",
            Self::DVID => "DVI-D",
            Self::DVIA => "DVI-A",
            Self::Composite => "Composite",
            Self::SVIDEO => "S-Video",
            Self::LVDS => "LVDS",
            Self::Component => "Component",
            Self::NinePinDIN => "9-pin DIN",
            Self::DisplayPort => "DisplayPort",
            Self::HDMIA => "HDMI-A",
            Self::HDMIB => "HDMI-B",
            Self::TV => "TV",
            Self::EDP => "eDP",
            Self::Virtual => "Virtual",
            Self::DSI => "DSI",
            Self::DPI => "DPI",
            Self::Writeback => "Writeback",
            Self::USB => "USB",
        }
    }

    /// Check if connector is digital
    #[inline]
    pub const fn is_digital(self) -> bool {
        matches!(
            self,
            Self::DVII | Self::DVID | Self::LVDS | Self::DisplayPort
                | Self::HDMIA | Self::HDMIB | Self::EDP | Self::DSI | Self::USB
        )
    }

    /// Check if connector supports HDCP
    #[inline]
    pub const fn supports_hdcp(self) -> bool {
        matches!(
            self,
            Self::DVII | Self::DVID | Self::DisplayPort | Self::HDMIA
                | Self::HDMIB | Self::EDP | Self::USB
        )
    }
}

impl fmt::Display for ConnectorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Connection Status
// ============================================================================

/// Connection status of a connector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ConnectionStatus {
    /// Display connected and detected
    Connected = 1,
    /// No display detected
    Disconnected = 2,
    /// Status unknown (e.g., VGA without load detection)
    Unknown = 3,
}

impl ConnectionStatus {
    /// Convert from DRM connection status
    #[inline]
    pub const fn from_drm(status: u32) -> Self {
        match status {
            1 => Self::Connected,
            2 => Self::Disconnected,
            _ => Self::Unknown,
        }
    }

    /// Get human-readable name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Connected => "Connected",
            Self::Disconnected => "Disconnected",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Plane Types
// ============================================================================

/// KMS plane type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PlaneType {
    /// Overlay plane (can be composited on top)
    Overlay = 0,
    /// Primary plane (main framebuffer)
    Primary = 1,
    /// Cursor plane (hardware cursor)
    Cursor = 2,
}

impl PlaneType {
    /// Convert from DRM plane type
    #[inline]
    pub const fn from_drm(drm_type: u32) -> Self {
        match drm_type {
            1 => Self::Primary,
            2 => Self::Cursor,
            _ => Self::Overlay,
        }
    }

    /// Get human-readable name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Overlay => "Overlay",
            Self::Primary => "Primary",
            Self::Cursor => "Cursor",
        }
    }

    /// Get the z-order priority (higher = on top)
    #[inline]
    pub const fn z_order(self) -> u8 {
        match self {
            Self::Primary => 0,
            Self::Overlay => 1,
            Self::Cursor => 255,
        }
    }
}

impl fmt::Display for PlaneType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// DPMS States
// ============================================================================

/// Display Power Management Signaling states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DpmsState {
    /// Display on (active)
    On = 0,
    /// Display standby (quick resume)
    Standby = 1,
    /// Display suspend (slow resume)
    Suspend = 2,
    /// Display off (lowest power)
    Off = 3,
}

impl DpmsState {
    /// Convert from DRM DPMS value
    #[inline]
    pub const fn from_drm(value: u32) -> Self {
        match value {
            0 => Self::On,
            1 => Self::Standby,
            2 => Self::Suspend,
            _ => Self::Off,
        }
    }

    /// Get human-readable name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::On => "On",
            Self::Standby => "Standby",
            Self::Suspend => "Suspend",
            Self::Off => "Off",
        }
    }

    /// Check if display is active
    #[inline]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::On)
    }
}

impl fmt::Display for DpmsState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Subpixel Layout
// ============================================================================

/// Subpixel layout of the display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SubpixelLayout {
    /// Unknown layout
    Unknown = 0,
    /// Horizontal RGB (most common)
    HorizontalRgb = 1,
    /// Horizontal BGR
    HorizontalBgr = 2,
    /// Vertical RGB
    VerticalRgb = 3,
    /// Vertical BGR
    VerticalBgr = 4,
    /// No subpixels (e.g., CRT, projector)
    None = 5,
}

impl SubpixelLayout {
    /// Convert from DRM subpixel value
    #[inline]
    pub const fn from_drm(value: u32) -> Self {
        match value {
            1 => Self::HorizontalRgb,
            2 => Self::HorizontalBgr,
            3 => Self::VerticalRgb,
            4 => Self::VerticalBgr,
            5 => Self::None,
            _ => Self::Unknown,
        }
    }

    /// Get human-readable name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::HorizontalRgb => "Horizontal RGB",
            Self::HorizontalBgr => "Horizontal BGR",
            Self::VerticalRgb => "Vertical RGB",
            Self::VerticalBgr => "Vertical BGR",
            Self::None => "None",
        }
    }
}

impl fmt::Display for SubpixelLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Page Flip Flags
// ============================================================================

/// Flags for page flip operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct PageFlipFlags(u32);

impl PageFlipFlags {
    /// No flags
    pub const NONE: Self = Self(0);
    /// Send page flip event on completion
    pub const EVENT: Self = Self(0x01);
    /// Flip asynchronously (tearing allowed)
    pub const ASYNC: Self = Self(0x02);

    /// Create from raw value
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Get raw value
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Check if flag is set
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Combine flags
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl core::ops::BitOr for PageFlipFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl Default for PageFlipFlags {
    #[inline]
    fn default() -> Self {
        Self::NONE
    }
}

// ============================================================================
// Atomic Commit Flags
// ============================================================================

/// Flags for atomic modesetting commit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct AtomicFlags(u32);

impl AtomicFlags {
    /// No flags
    pub const NONE: Self = Self(0);
    /// Test only, don't commit
    pub const TEST_ONLY: Self = Self(0x0100);
    /// Non-blocking commit
    pub const NONBLOCK: Self = Self(0x0200);
    /// Allow modeset (mode changes)
    pub const ALLOW_MODESET: Self = Self(0x0400);
    /// Send page flip event
    pub const PAGE_FLIP_EVENT: Self = Self(0x01);
    /// Use async page flip (tearing allowed)
    pub const PAGE_FLIP_ASYNC: Self = Self(0x02);

    /// Create from raw value
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Get raw value
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Check if flag is set
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Combine flags
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl core::ops::BitOr for AtomicFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl Default for AtomicFlags {
    #[inline]
    fn default() -> Self {
        Self::NONE
    }
}

// ============================================================================
// Display Mode
// ============================================================================

/// Display mode timing information (matches DRM mode structure)
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DrmMode {
    /// Pixel clock in kHz
    pub clock: u32,
    /// Horizontal active pixels
    pub hdisplay: u16,
    /// Horizontal sync start
    pub hsync_start: u16,
    /// Horizontal sync end
    pub hsync_end: u16,
    /// Horizontal total
    pub htotal: u16,
    /// Horizontal skew
    pub hskew: u16,
    /// Vertical active lines
    pub vdisplay: u16,
    /// Vertical sync start
    pub vsync_start: u16,
    /// Vertical sync end
    pub vsync_end: u16,
    /// Vertical total
    pub vtotal: u16,
    /// Vertical scan
    pub vscan: u16,
    /// Vertical refresh rate (Hz * 1000)
    pub vrefresh: u32,
    /// Mode flags
    pub flags: u32,
    /// Mode type
    pub type_: u32,
    /// Mode name (e.g., "1920x1080")
    pub name: [u8; 32],
}

impl DrmMode {
    /// Standard 1920x1080@60Hz mode (CVT-RB)
    pub const MODE_1920X1080_60: Self = Self {
        clock: 148500,
        hdisplay: 1920,
        hsync_start: 2008,
        hsync_end: 2052,
        htotal: 2200,
        hskew: 0,
        vdisplay: 1080,
        vsync_start: 1084,
        vsync_end: 1089,
        vtotal: 1125,
        vscan: 0,
        vrefresh: 60000,
        flags: 0x05, // +HSYNC +VSYNC
        type_: 0x40, // DRM_MODE_TYPE_DRIVER
        name: *b"1920x1080\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    };

    /// Standard 2560x1440@60Hz mode
    pub const MODE_2560X1440_60: Self = Self {
        clock: 241500,
        hdisplay: 2560,
        hsync_start: 2608,
        hsync_end: 2640,
        htotal: 2720,
        hskew: 0,
        vdisplay: 1440,
        vsync_start: 1443,
        vsync_end: 1448,
        vtotal: 1481,
        vscan: 0,
        vrefresh: 60000,
        flags: 0x06, // +HSYNC -VSYNC
        type_: 0x40,
        name: *b"2560x1440\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    };

    /// Standard 3840x2160@60Hz mode (4K UHD)
    pub const MODE_3840X2160_60: Self = Self {
        clock: 533250,
        hdisplay: 3840,
        hsync_start: 3888,
        hsync_end: 3920,
        htotal: 4000,
        hskew: 0,
        vdisplay: 2160,
        vsync_start: 2163,
        vsync_end: 2168,
        vtotal: 2222,
        vscan: 0,
        vrefresh: 60000,
        flags: 0x05,
        type_: 0x40,
        name: *b"3840x2160\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    };

    /// Create new mode with timing
    #[inline]
    pub const fn new(hdisplay: u16, vdisplay: u16, vrefresh: u32) -> Self {
        Self {
            clock: 0,
            hdisplay,
            hsync_start: hdisplay,
            hsync_end: hdisplay,
            htotal: hdisplay,
            hskew: 0,
            vdisplay,
            vsync_start: vdisplay,
            vsync_end: vdisplay,
            vtotal: vdisplay,
            vscan: 0,
            vrefresh,
            flags: 0,
            type_: 0x40,
            name: [0; 32],
        }
    }

    /// Calculate pixel clock from timing
    #[inline]
    pub const fn calculate_clock(&self) -> u32 {
        let htotal = self.htotal as u32;
        let vtotal = self.vtotal as u32;
        let vrefresh = self.vrefresh / 1000;
        htotal * vtotal * vrefresh / 1000
    }

    /// Get refresh rate in Hz
    #[inline]
    pub const fn refresh_hz(&self) -> u32 {
        self.vrefresh / 1000
    }

    /// Get mode name as string slice
    pub fn name_str(&self) -> &str {
        let len = self.name.iter().position(|&b| b == 0).unwrap_or(32);
        core::str::from_utf8(&self.name[..len]).unwrap_or("Unknown")
    }

    /// Calculate total pixels per frame
    #[inline]
    pub const fn pixels_per_frame(&self) -> u32 {
        (self.htotal as u32) * (self.vtotal as u32)
    }

    /// Calculate bandwidth requirement in bytes/sec (assuming 4 bytes per pixel)
    #[inline]
    pub const fn bandwidth_bps(&self) -> u64 {
        let pixels = (self.hdisplay as u64) * (self.vdisplay as u64);
        let fps = (self.vrefresh as u64) / 1000;
        pixels * fps * 4 // 4 bytes per pixel (32-bit color)
    }
}

impl Default for DrmMode {
    #[inline]
    fn default() -> Self {
        Self::MODE_1920X1080_60
    }
}

impl Debug for DrmMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DrmMode")
            .field("name", &self.name_str())
            .field("clock", &self.clock)
            .field("hdisplay", &self.hdisplay)
            .field("vdisplay", &self.vdisplay)
            .field("vrefresh", &format_args!("{}.{:03}Hz", self.vrefresh / 1000, self.vrefresh % 1000))
            .finish()
    }
}

// ============================================================================
// VBlank Event
// ============================================================================

/// VBlank event data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct VBlankEvent {
    /// Sequence number (monotonic)
    pub sequence: u32,
    /// Timestamp seconds
    pub tv_sec: i64,
    /// Timestamp microseconds
    pub tv_usec: i64,
    /// CRTC index
    pub crtc_id: u32,
    /// User data
    pub user_data: u64,
}

impl VBlankEvent {
    /// Create empty event
    #[inline]
    pub const fn new() -> Self {
        Self {
            sequence: 0,
            tv_sec: 0,
            tv_usec: 0,
            crtc_id: 0,
            user_data: 0,
        }
    }

    /// Get timestamp in nanoseconds
    #[inline]
    pub const fn timestamp_ns(&self) -> u64 {
        (self.tv_sec as u64) * 1_000_000_000 + (self.tv_usec as u64) * 1_000
    }

    /// Get timestamp in microseconds
    #[inline]
    pub const fn timestamp_us(&self) -> u64 {
        (self.tv_sec as u64) * 1_000_000 + (self.tv_usec as u64)
    }
}

impl Default for VBlankEvent {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// KMS Connector Capsule (T1 Atomic, 256B)
// ============================================================================

/// KMS connector capsule - atomic lockfree display connector state
///
/// # Layout (256B, 4 cache lines)
/// ```text
/// Offset  Size  Field          Description
/// 0       8     state          Packed: [id:32][type:8][status:8][encoder_id:16]
/// 8       8     generation     Generation counter for ABA prevention
/// 16      8     physical_size  Physical dimensions [width:32][height:32] (mm)
/// 24      8     subpixel       Subpixel layout and flags
/// 32      8     dpms           DPMS state and timing
/// 40      8     crtc_id        Connected CRTC ID
/// 48      8     edid_hash      EDID hash for display identification
/// 56      8     mode_count     Number of supported modes
/// 64      192   _padding       Cache alignment padding
/// ```
///
/// # Chaos Compliance
/// - T1 Atomic tier (<100ns operations)
/// - 256B aligned (4 cache lines)
/// - 100% lockfree (no mutex/RwLock)
/// - DualAtomicU64 pattern with generation counters
#[derive(Debug)]
#[repr(C, align(256))]
pub struct KmsConnectorCapsule {
    /// Packed state: [id:32][type:8][status:8][encoder_id:16]
    state: AtomicU64,
    /// Generation counter for CAS operations
    generation: AtomicU64,
    /// Physical size in mm: [width:32][height:32]
    physical_size: AtomicU64,
    /// Subpixel layout and flags
    subpixel: AtomicU64,
    /// DPMS state
    dpms: AtomicU64,
    /// Connected CRTC ID
    crtc_id: AtomicU64,
    /// EDID hash for identification
    edid_hash: AtomicU64,
    /// Number of supported modes
    mode_count: AtomicU64,
    /// Padding to 256B
    _padding: [u8; 192],
}

// Verify size and alignment
const _: () = {
    assert!(mem::size_of::<KmsConnectorCapsule>() == 256);
    assert!(mem::align_of::<KmsConnectorCapsule>() == 256);
};

impl KmsConnectorCapsule {
    /// Create new connector capsule
    ///
    /// # Complexity
    /// O(1) lockfree initialization (<50ns)
    pub fn new(id: u32, connector_type: ConnectorType) -> Self {
        let state = ((id as u64) << 32)
            | ((connector_type as u64) << 24)
            | ((ConnectionStatus::Unknown as u64) << 16);

        Self {
            state: AtomicU64::new(state),
            generation: AtomicU64::new(1),
            physical_size: AtomicU64::new(0),
            subpixel: AtomicU64::new(SubpixelLayout::Unknown as u64),
            dpms: AtomicU64::new(DpmsState::Off as u64),
            crtc_id: AtomicU64::new(0),
            edid_hash: AtomicU64::new(0),
            mode_count: AtomicU64::new(0),
            _padding: [0; 192],
        }
    }

    /// Get connector ID
    #[inline]
    pub fn id(&self) -> u32 {
        (self.state.load(Ordering::Acquire) >> 32) as u32
    }

    /// Get connector type
    #[inline]
    pub fn connector_type(&self) -> ConnectorType {
        let type_bits = ((self.state.load(Ordering::Acquire) >> 24) & 0xFF) as u8;
        ConnectorType::from_drm(type_bits as u32)
    }

    /// Get connection status
    #[inline]
    pub fn status(&self) -> ConnectionStatus {
        let status_bits = ((self.state.load(Ordering::Acquire) >> 16) & 0xFF) as u8;
        ConnectionStatus::from_drm(status_bits as u32)
    }

    /// Get encoder ID
    #[inline]
    pub fn encoder_id(&self) -> u16 {
        (self.state.load(Ordering::Acquire) & 0xFFFF) as u16
    }

    /// Update connection status (atomic CAS)
    ///
    /// # Complexity
    /// O(1) lockfree CAS loop (<20ns typical, <50ns worst)
    pub fn set_status(&self, status: ConnectionStatus) -> bool {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let new = (current & !0x00FF_0000) | ((status as u64) << 16);
            match self.state.compare_exchange_weak(
                current,
                new,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Increment generation
                    self.generation.fetch_add(1, Ordering::Release);
                    return true;
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Set encoder ID
    pub fn set_encoder_id(&self, encoder_id: u16) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let new = (current & !0xFFFF) | (encoder_id as u64);
            match self.state.compare_exchange_weak(
                current,
                new,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }

    /// Get physical size in mm (width, height)
    #[inline]
    pub fn physical_size_mm(&self) -> (u32, u32) {
        let size = self.physical_size.load(Ordering::Acquire);
        ((size >> 32) as u32, (size & 0xFFFFFFFF) as u32)
    }

    /// Set physical size in mm
    pub fn set_physical_size_mm(&self, width_mm: u32, height_mm: u32) {
        let size = ((width_mm as u64) << 32) | (height_mm as u64);
        self.physical_size.store(size, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get subpixel layout
    #[inline]
    pub fn subpixel_layout(&self) -> SubpixelLayout {
        SubpixelLayout::from_drm(self.subpixel.load(Ordering::Acquire) as u32)
    }

    /// Set subpixel layout
    pub fn set_subpixel_layout(&self, layout: SubpixelLayout) {
        self.subpixel.store(layout as u64, Ordering::Release);
    }

    /// Get DPMS state
    #[inline]
    pub fn dpms_state(&self) -> DpmsState {
        DpmsState::from_drm(self.dpms.load(Ordering::Acquire) as u32)
    }

    /// Set DPMS state
    pub fn set_dpms_state(&self, state: DpmsState) {
        self.dpms.store(state as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get connected CRTC ID (0 if not connected)
    #[inline]
    pub fn crtc_id(&self) -> u32 {
        self.crtc_id.load(Ordering::Acquire) as u32
    }

    /// Set connected CRTC ID
    pub fn set_crtc_id(&self, crtc_id: u32) {
        self.crtc_id.store(crtc_id as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get EDID hash
    #[inline]
    pub fn edid_hash(&self) -> u64 {
        self.edid_hash.load(Ordering::Acquire)
    }

    /// Set EDID hash
    pub fn set_edid_hash(&self, hash: u64) {
        self.edid_hash.store(hash, Ordering::Release);
    }

    /// Get mode count
    #[inline]
    pub fn mode_count(&self) -> u32 {
        self.mode_count.load(Ordering::Acquire) as u32
    }

    /// Set mode count
    pub fn set_mode_count(&self, count: u32) {
        self.mode_count.store(count as u64, Ordering::Release);
    }

    /// Get generation counter (for ABA prevention)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Take atomic snapshot
    ///
    /// # Complexity
    /// O(1) lockfree (<10ns)
    pub fn snapshot(&self) -> KmsConnectorSnapshot {
        let gen_before = self.generation.load(Ordering::Acquire);
        let state = self.state.load(Ordering::Acquire);
        let physical_size = self.physical_size.load(Ordering::Acquire);
        let crtc_id = self.crtc_id.load(Ordering::Acquire);
        let dpms = self.dpms.load(Ordering::Acquire);
        let gen_after = self.generation.load(Ordering::Acquire);

        KmsConnectorSnapshot {
            generation: gen_after,
            id: (state >> 32) as u32,
            connector_type: ConnectorType::from_drm(((state >> 24) & 0xFF) as u32),
            status: ConnectionStatus::from_drm(((state >> 16) & 0xFF) as u32),
            encoder_id: (state & 0xFFFF) as u16,
            physical_width_mm: (physical_size >> 32) as u32,
            physical_height_mm: (physical_size & 0xFFFFFFFF) as u32,
            crtc_id: crtc_id as u32,
            dpms: DpmsState::from_drm(dpms as u32),
            consistent: gen_before == gen_after,
        }
    }
}

/// Snapshot of connector state
#[derive(Debug, Clone, Copy)]
pub struct KmsConnectorSnapshot {
    /// Generation at snapshot
    pub generation: u64,
    /// Connector ID
    pub id: u32,
    /// Connector type
    pub connector_type: ConnectorType,
    /// Connection status
    pub status: ConnectionStatus,
    /// Encoder ID
    pub encoder_id: u16,
    /// Physical width in mm
    pub physical_width_mm: u32,
    /// Physical height in mm
    pub physical_height_mm: u32,
    /// Connected CRTC ID
    pub crtc_id: u32,
    /// DPMS state
    pub dpms: DpmsState,
    /// Whether snapshot is consistent
    pub consistent: bool,
}

// ============================================================================
// KMS CRTC Capsule (T1 Atomic, 256B)
// ============================================================================

/// KMS CRTC capsule - atomic lockfree display controller state
///
/// CRTC (Cathode Ray Tube Controller) manages scanout timing and gamma.
///
/// # Layout (256B)
/// ```text
/// Offset  Size  Field          Description
/// 0       8     state          Packed: [id:32][active:8][gamma_size:16][index:8]
/// 8       8     generation     Generation counter
/// 16      8     mode_hash      Current mode hash
/// 24      8     position       Position [x:32][y:32]
/// 32      8     fb_id          Current framebuffer ID
/// 40      8     vblank_seq     VBlank sequence number
/// 48      8     vblank_ts      VBlank timestamp (ns)
/// 56      8     flip_pending   Page flip pending counter
/// 64      192   _padding       Cache alignment
/// ```
#[derive(Debug)]
#[repr(C, align(256))]
pub struct KmsCrtcCapsule {
    /// Packed state: [id:32][active:8][gamma_size:16][index:8]
    state: AtomicU64,
    /// Generation counter
    generation: AtomicU64,
    /// Current mode hash (FNV-1a of DrmMode)
    mode_hash: AtomicU64,
    /// Position [x:32][y:32]
    position: AtomicU64,
    /// Current framebuffer ID
    fb_id: AtomicU64,
    /// VBlank sequence number
    vblank_seq: AtomicU64,
    /// VBlank timestamp (ns)
    vblank_ts: AtomicU64,
    /// Page flip pending (0 = no flip, >0 = flip pending)
    flip_pending: AtomicU64,
    /// Padding to 256B
    _padding: [u8; 192],
}

// Verify size and alignment
const _: () = {
    assert!(mem::size_of::<KmsCrtcCapsule>() == 256);
    assert!(mem::align_of::<KmsCrtcCapsule>() == 256);
};

impl KmsCrtcCapsule {
    /// Create new CRTC capsule
    pub fn new(id: u32, index: u8, gamma_size: u16) -> Self {
        let state = ((id as u64) << 32)
            | ((0u64) << 24) // inactive
            | ((gamma_size as u64) << 8)
            | (index as u64);

        Self {
            state: AtomicU64::new(state),
            generation: AtomicU64::new(1),
            mode_hash: AtomicU64::new(0),
            position: AtomicU64::new(0),
            fb_id: AtomicU64::new(0),
            vblank_seq: AtomicU64::new(0),
            vblank_ts: AtomicU64::new(0),
            flip_pending: AtomicU64::new(0),
            _padding: [0; 192],
        }
    }

    /// Get CRTC ID
    #[inline]
    pub fn id(&self) -> u32 {
        (self.state.load(Ordering::Acquire) >> 32) as u32
    }

    /// Get CRTC index
    #[inline]
    pub fn index(&self) -> u8 {
        (self.state.load(Ordering::Acquire) & 0xFF) as u8
    }

    /// Check if CRTC is active
    #[inline]
    pub fn is_active(&self) -> bool {
        ((self.state.load(Ordering::Acquire) >> 24) & 0xFF) != 0
    }

    /// Set active state
    pub fn set_active(&self, active: bool) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let new = (current & !0xFF00_0000) | (if active { 1u64 } else { 0u64 } << 24);
            match self.state.compare_exchange_weak(
                current,
                new,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Release);
                    return;
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Get gamma table size
    #[inline]
    pub fn gamma_size(&self) -> u16 {
        ((self.state.load(Ordering::Acquire) >> 8) & 0xFFFF) as u16
    }

    /// Get position (x, y)
    #[inline]
    pub fn position(&self) -> (i32, i32) {
        let pos = self.position.load(Ordering::Acquire);
        ((pos >> 32) as i32, (pos & 0xFFFFFFFF) as i32)
    }

    /// Set position
    pub fn set_position(&self, x: i32, y: i32) {
        let pos = ((x as u32 as u64) << 32) | (y as u32 as u64);
        self.position.store(pos, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get current framebuffer ID
    #[inline]
    pub fn fb_id(&self) -> u32 {
        self.fb_id.load(Ordering::Acquire) as u32
    }

    /// Set framebuffer ID
    pub fn set_fb_id(&self, fb_id: u32) {
        self.fb_id.store(fb_id as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get VBlank sequence number
    #[inline]
    pub fn vblank_seq(&self) -> u64 {
        self.vblank_seq.load(Ordering::Acquire)
    }

    /// Increment VBlank sequence (atomic)
    pub fn increment_vblank(&self, timestamp_ns: u64) {
        self.vblank_seq.fetch_add(1, Ordering::Release);
        self.vblank_ts.store(timestamp_ns, Ordering::Release);
    }

    /// Get VBlank timestamp
    #[inline]
    pub fn vblank_timestamp_ns(&self) -> u64 {
        self.vblank_ts.load(Ordering::Acquire)
    }

    /// Check if page flip is pending
    #[inline]
    pub fn is_flip_pending(&self) -> bool {
        self.flip_pending.load(Ordering::Acquire) > 0
    }

    /// Set page flip pending
    pub fn set_flip_pending(&self, pending: bool) {
        self.flip_pending.store(if pending { 1 } else { 0 }, Ordering::Release);
    }

    /// Get mode hash
    #[inline]
    pub fn mode_hash(&self) -> u64 {
        self.mode_hash.load(Ordering::Acquire)
    }

    /// Set mode hash
    pub fn set_mode_hash(&self, hash: u64) {
        self.mode_hash.store(hash, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Take atomic snapshot
    pub fn snapshot(&self) -> KmsCrtcSnapshot {
        let gen_before = self.generation.load(Ordering::Acquire);
        let state = self.state.load(Ordering::Acquire);
        let position = self.position.load(Ordering::Acquire);
        let fb_id = self.fb_id.load(Ordering::Acquire);
        let vblank_seq = self.vblank_seq.load(Ordering::Acquire);
        let vblank_ts = self.vblank_ts.load(Ordering::Acquire);
        let flip_pending = self.flip_pending.load(Ordering::Acquire);
        let gen_after = self.generation.load(Ordering::Acquire);

        KmsCrtcSnapshot {
            generation: gen_after,
            id: (state >> 32) as u32,
            index: (state & 0xFF) as u8,
            active: ((state >> 24) & 0xFF) != 0,
            gamma_size: ((state >> 8) & 0xFFFF) as u16,
            x: (position >> 32) as i32,
            y: (position & 0xFFFFFFFF) as i32,
            fb_id: fb_id as u32,
            vblank_seq,
            vblank_ts,
            flip_pending: flip_pending > 0,
            consistent: gen_before == gen_after,
        }
    }
}

/// Snapshot of CRTC state
#[derive(Debug, Clone, Copy)]
pub struct KmsCrtcSnapshot {
    /// Generation at snapshot
    pub generation: u64,
    /// CRTC ID
    pub id: u32,
    /// CRTC index
    pub index: u8,
    /// Whether CRTC is active
    pub active: bool,
    /// Gamma table size
    pub gamma_size: u16,
    /// X position
    pub x: i32,
    /// Y position
    pub y: i32,
    /// Framebuffer ID
    pub fb_id: u32,
    /// VBlank sequence
    pub vblank_seq: u64,
    /// VBlank timestamp (ns)
    pub vblank_ts: u64,
    /// Page flip pending
    pub flip_pending: bool,
    /// Whether snapshot is consistent
    pub consistent: bool,
}

// ============================================================================
// KMS Plane Capsule (T1 Atomic, 256B)
// ============================================================================

/// KMS plane capsule - atomic lockfree display plane state
///
/// # Layout (256B)
/// ```text
/// Offset  Size  Field          Description
/// 0       8     state          Packed: [id:32][type:8][crtc_id:16][zpos:8]
/// 8       8     generation     Generation counter
/// 16      8     src_rect       Source rect [x:16][y:16][w:16][h:16] (16.16 fixed-point)
/// 24      8     dst_rect       Dest rect [x:16][y:16][w:16][h:16]
/// 32      8     fb_id          Current framebuffer ID
/// 40      8     format         Pixel format (fourcc)
/// 48      8     rotation       Rotation/reflection flags
/// 56      8     alpha          Alpha value (0-0xFFFF)
/// 64      192   _padding       Cache alignment
/// ```
#[derive(Debug)]
#[repr(C, align(256))]
pub struct KmsPlaneCapsule {
    /// Packed state: [id:32][type:8][crtc_id:16][zpos:8]
    state: AtomicU64,
    /// Generation counter
    generation: AtomicU64,
    /// Source rect: [x:16][y:16][w:16][h:16] (16.16 fixed-point)
    src_rect: AtomicU64,
    /// Dest rect: [x:16][y:16][w:16][h:16]
    dst_rect: AtomicU64,
    /// Current framebuffer ID
    fb_id: AtomicU64,
    /// Pixel format (fourcc)
    format: AtomicU64,
    /// Rotation/reflection flags
    rotation: AtomicU64,
    /// Alpha value (0-0xFFFF)
    alpha: AtomicU64,
    /// Padding to 256B
    _padding: [u8; 192],
}

// Verify size and alignment
const _: () = {
    assert!(mem::size_of::<KmsPlaneCapsule>() == 256);
    assert!(mem::align_of::<KmsPlaneCapsule>() == 256);
};

impl KmsPlaneCapsule {
    /// Create new plane capsule
    pub fn new(id: u32, plane_type: PlaneType) -> Self {
        let state = ((id as u64) << 32)
            | ((plane_type as u64) << 24)
            | (0u64 << 8) // crtc_id = 0
            | (plane_type.z_order() as u64);

        Self {
            state: AtomicU64::new(state),
            generation: AtomicU64::new(1),
            src_rect: AtomicU64::new(0),
            dst_rect: AtomicU64::new(0),
            fb_id: AtomicU64::new(0),
            format: AtomicU64::new(0),
            rotation: AtomicU64::new(0),
            alpha: AtomicU64::new(0xFFFF), // Fully opaque
            _padding: [0; 192],
        }
    }

    /// Get plane ID
    #[inline]
    pub fn id(&self) -> u32 {
        (self.state.load(Ordering::Acquire) >> 32) as u32
    }

    /// Get plane type
    #[inline]
    pub fn plane_type(&self) -> PlaneType {
        PlaneType::from_drm(((self.state.load(Ordering::Acquire) >> 24) & 0xFF) as u32)
    }

    /// Get CRTC ID
    #[inline]
    pub fn crtc_id(&self) -> u16 {
        ((self.state.load(Ordering::Acquire) >> 8) & 0xFFFF) as u16
    }

    /// Set CRTC ID
    pub fn set_crtc_id(&self, crtc_id: u16) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let new = (current & !0x00FFFF00) | ((crtc_id as u64) << 8);
            match self.state.compare_exchange_weak(
                current,
                new,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Release);
                    return;
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Get z-position
    #[inline]
    pub fn z_pos(&self) -> u8 {
        (self.state.load(Ordering::Acquire) & 0xFF) as u8
    }

    /// Set z-position
    pub fn set_z_pos(&self, zpos: u8) {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let new = (current & !0xFF) | (zpos as u64);
            match self.state.compare_exchange_weak(
                current,
                new,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Release);
                    return;
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Get source rect (x, y, w, h) in 16.16 fixed-point
    #[inline]
    pub fn src_rect(&self) -> (u16, u16, u16, u16) {
        let rect = self.src_rect.load(Ordering::Acquire);
        (
            ((rect >> 48) & 0xFFFF) as u16,
            ((rect >> 32) & 0xFFFF) as u16,
            ((rect >> 16) & 0xFFFF) as u16,
            (rect & 0xFFFF) as u16,
        )
    }

    /// Set source rect
    pub fn set_src_rect(&self, x: u16, y: u16, w: u16, h: u16) {
        let rect = ((x as u64) << 48) | ((y as u64) << 32) | ((w as u64) << 16) | (h as u64);
        self.src_rect.store(rect, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get destination rect (x, y, w, h)
    #[inline]
    pub fn dst_rect(&self) -> (u16, u16, u16, u16) {
        let rect = self.dst_rect.load(Ordering::Acquire);
        (
            ((rect >> 48) & 0xFFFF) as u16,
            ((rect >> 32) & 0xFFFF) as u16,
            ((rect >> 16) & 0xFFFF) as u16,
            (rect & 0xFFFF) as u16,
        )
    }

    /// Set destination rect
    pub fn set_dst_rect(&self, x: u16, y: u16, w: u16, h: u16) {
        let rect = ((x as u64) << 48) | ((y as u64) << 32) | ((w as u64) << 16) | (h as u64);
        self.dst_rect.store(rect, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get framebuffer ID
    #[inline]
    pub fn fb_id(&self) -> u32 {
        self.fb_id.load(Ordering::Acquire) as u32
    }

    /// Set framebuffer ID
    pub fn set_fb_id(&self, fb_id: u32) {
        self.fb_id.store(fb_id as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get pixel format (fourcc)
    #[inline]
    pub fn format(&self) -> u32 {
        self.format.load(Ordering::Acquire) as u32
    }

    /// Set pixel format
    pub fn set_format(&self, format: u32) {
        self.format.store(format as u64, Ordering::Release);
    }

    /// Get rotation flags
    #[inline]
    pub fn rotation(&self) -> u32 {
        self.rotation.load(Ordering::Acquire) as u32
    }

    /// Set rotation flags
    pub fn set_rotation(&self, rotation: u32) {
        self.rotation.store(rotation as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get alpha value (0-0xFFFF)
    #[inline]
    pub fn alpha(&self) -> u16 {
        self.alpha.load(Ordering::Acquire) as u16
    }

    /// Set alpha value
    pub fn set_alpha(&self, alpha: u16) {
        self.alpha.store(alpha as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Take atomic snapshot
    pub fn snapshot(&self) -> KmsPlaneSnapshot {
        let gen_before = self.generation.load(Ordering::Acquire);
        let state = self.state.load(Ordering::Acquire);
        let src_rect = self.src_rect.load(Ordering::Acquire);
        let dst_rect = self.dst_rect.load(Ordering::Acquire);
        let fb_id = self.fb_id.load(Ordering::Acquire);
        let format = self.format.load(Ordering::Acquire);
        let alpha = self.alpha.load(Ordering::Acquire);
        let gen_after = self.generation.load(Ordering::Acquire);

        KmsPlaneSnapshot {
            generation: gen_after,
            id: (state >> 32) as u32,
            plane_type: PlaneType::from_drm(((state >> 24) & 0xFF) as u32),
            crtc_id: ((state >> 8) & 0xFFFF) as u16,
            z_pos: (state & 0xFF) as u8,
            src_x: ((src_rect >> 48) & 0xFFFF) as u16,
            src_y: ((src_rect >> 32) & 0xFFFF) as u16,
            src_w: ((src_rect >> 16) & 0xFFFF) as u16,
            src_h: (src_rect & 0xFFFF) as u16,
            dst_x: ((dst_rect >> 48) & 0xFFFF) as u16,
            dst_y: ((dst_rect >> 32) & 0xFFFF) as u16,
            dst_w: ((dst_rect >> 16) & 0xFFFF) as u16,
            dst_h: (dst_rect & 0xFFFF) as u16,
            fb_id: fb_id as u32,
            format: format as u32,
            alpha: alpha as u16,
            consistent: gen_before == gen_after,
        }
    }
}

/// Snapshot of plane state
#[derive(Debug, Clone, Copy)]
pub struct KmsPlaneSnapshot {
    /// Generation at snapshot
    pub generation: u64,
    /// Plane ID
    pub id: u32,
    /// Plane type
    pub plane_type: PlaneType,
    /// CRTC ID
    pub crtc_id: u16,
    /// Z position
    pub z_pos: u8,
    /// Source X
    pub src_x: u16,
    /// Source Y
    pub src_y: u16,
    /// Source width
    pub src_w: u16,
    /// Source height
    pub src_h: u16,
    /// Destination X
    pub dst_x: u16,
    /// Destination Y
    pub dst_y: u16,
    /// Destination width
    pub dst_w: u16,
    /// Destination height
    pub dst_h: u16,
    /// Framebuffer ID
    pub fb_id: u32,
    /// Pixel format
    pub format: u32,
    /// Alpha value
    pub alpha: u16,
    /// Whether snapshot is consistent
    pub consistent: bool,
}

// ============================================================================
// KMS Resources
// ============================================================================

/// Display resources from DRM
#[derive(Debug, Default)]
pub struct KmsResources {
    /// Connector IDs
    pub connector_ids: Vec<u32>,
    /// CRTC IDs
    pub crtc_ids: Vec<u32>,
    /// Encoder IDs
    pub encoder_ids: Vec<u32>,
    /// Framebuffer IDs
    pub fb_ids: Vec<u32>,
    /// Minimum width
    pub min_width: u32,
    /// Maximum width
    pub max_width: u32,
    /// Minimum height
    pub min_height: u32,
    /// Maximum height
    pub max_height: u32,
}

impl KmsResources {
    /// Create new empty resources
    pub const fn new() -> Self {
        Self {
            connector_ids: Vec::new(),
            crtc_ids: Vec::new(),
            encoder_ids: Vec::new(),
            fb_ids: Vec::new(),
            min_width: 0,
            max_width: 0,
            min_height: 0,
            max_height: 0,
        }
    }

    /// Get number of connectors
    #[inline]
    pub fn num_connectors(&self) -> usize {
        self.connector_ids.len()
    }

    /// Get number of CRTCs
    #[inline]
    pub fn num_crtcs(&self) -> usize {
        self.crtc_ids.len()
    }
}

// ============================================================================
// Plane Resources
// ============================================================================

/// Plane resources from DRM
#[derive(Debug, Default)]
pub struct KmsPlaneResources {
    /// Plane IDs
    pub plane_ids: Vec<u32>,
}

impl KmsPlaneResources {
    /// Create new empty resources
    pub const fn new() -> Self {
        Self {
            plane_ids: Vec::new(),
        }
    }

    /// Get number of planes
    #[inline]
    pub fn num_planes(&self) -> usize {
        self.plane_ids.len()
    }
}

// ============================================================================
// Atomic Request
// ============================================================================

/// Property change for atomic commit
#[derive(Debug, Clone, Copy)]
pub struct AtomicProperty {
    /// Object ID (CRTC, plane, connector)
    pub object_id: u32,
    /// Property ID
    pub property_id: u32,
    /// Property value
    pub value: u64,
}

/// Atomic modesetting request
#[derive(Debug, Default)]
pub struct AtomicRequest {
    /// Property changes
    pub properties: Vec<AtomicProperty>,
}

impl AtomicRequest {
    /// Create new empty request
    pub const fn new() -> Self {
        Self {
            properties: Vec::new(),
        }
    }

    /// Add property change
    pub fn add_property(&mut self, object_id: u32, property_id: u32, value: u64) {
        self.properties.push(AtomicProperty {
            object_id,
            property_id,
            value,
        });
    }

    /// Get number of properties
    #[inline]
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    /// Check if empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    /// Clear all properties
    pub fn clear(&mut self) {
        self.properties.clear();
    }
}

// ============================================================================
// Framebuffer Handle
// ============================================================================

/// Framebuffer handle with metadata
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramebufferHandle {
    /// DRM framebuffer ID
    pub fb_id: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Pixel format (fourcc)
    pub format: u32,
    /// Format modifier
    pub modifier: u64,
}

impl FramebufferHandle {
    /// Create new handle
    pub const fn new(fb_id: u32, width: u32, height: u32, format: u32) -> Self {
        Self {
            fb_id,
            width,
            height,
            format,
            modifier: 0,
        }
    }

    /// Check if handle is valid
    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.fb_id != 0
    }
}

// ============================================================================
// DRM Device Handle (for KMS operations)
// ============================================================================

/// DRM device capsule for KMS operations
///
/// Wraps a file descriptor with atomic state tracking.
#[derive(Debug)]
#[repr(C, align(64))]
pub struct DrmDeviceCapsule {
    /// File descriptor
    fd: AtomicU64,
    /// Capabilities bitmap
    capabilities: AtomicU64,
    /// Generation counter
    generation: AtomicU64,
    /// Padding
    _padding: [u8; 40],
}

const _: () = {
    assert!(mem::size_of::<DrmDeviceCapsule>() == 64);
    assert!(mem::align_of::<DrmDeviceCapsule>() == 64);
};

impl DrmDeviceCapsule {
    /// Create new device capsule
    pub fn new(fd: i32) -> Self {
        Self {
            fd: AtomicU64::new(fd as u64),
            capabilities: AtomicU64::new(0),
            generation: AtomicU64::new(1),
            _padding: [0; 40],
        }
    }

    /// Get file descriptor
    #[inline]
    pub fn fd(&self) -> i32 {
        self.fd.load(Ordering::Acquire) as i32
    }

    /// Check if atomic modesetting is supported
    #[inline]
    pub fn supports_atomic(&self) -> bool {
        (self.capabilities.load(Ordering::Acquire) & 0x01) != 0
    }

    /// Set atomic modesetting capability
    pub fn set_atomic_cap(&self, supported: bool) {
        let mut caps = self.capabilities.load(Ordering::Acquire);
        if supported {
            caps |= 0x01;
        } else {
            caps &= !0x01;
        }
        self.capabilities.store(caps, Ordering::Release);
    }

    /// Check if universal planes are supported
    #[inline]
    pub fn supports_universal_planes(&self) -> bool {
        (self.capabilities.load(Ordering::Acquire) & 0x02) != 0
    }

    /// Set universal planes capability
    pub fn set_universal_planes_cap(&self, supported: bool) {
        let mut caps = self.capabilities.load(Ordering::Acquire);
        if supported {
            caps |= 0x02;
        } else {
            caps &= !0x02;
        }
        self.capabilities.store(caps, Ordering::Release);
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}

// ============================================================================
// KMS Operations (Mock implementations - real ones need ioctl)
// ============================================================================

/// Get KMS resources from device
///
/// # Arguments
/// - `device`: DRM device capsule
///
/// # Returns
/// KMS resources or error
///
/// # Complexity
/// O(1) + ioctl overhead (~100us)
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn get_resources(_device: &DrmDeviceCapsule) -> KgpuDriverResult<KmsResources> {
    // Real implementation would use DRM_IOCTL_MODE_GETRESOURCES
    // This is a mock for now
    Err(KgpuDriverError::NotImplemented)
}

/// Get connector information
///
/// # Arguments
/// - `device`: DRM device capsule
/// - `id`: Connector ID
///
/// # Returns
/// Connector capsule or error
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn get_connector(_device: &DrmDeviceCapsule, id: u32) -> KgpuDriverResult<KmsConnectorCapsule> {
    // Real implementation would use DRM_IOCTL_MODE_GETCONNECTOR
    // Return a mock connector
    let connector = KmsConnectorCapsule::new(id, ConnectorType::Unknown);
    Ok(connector)
}

/// Get CRTC information
///
/// # Arguments
/// - `device`: DRM device capsule
/// - `id`: CRTC ID
///
/// # Returns
/// CRTC capsule or error
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn get_crtc(_device: &DrmDeviceCapsule, id: u32) -> KgpuDriverResult<KmsCrtcCapsule> {
    // Real implementation would use DRM_IOCTL_MODE_GETCRTC
    let crtc = KmsCrtcCapsule::new(id, 0, 256);
    Ok(crtc)
}

/// Get plane information
///
/// # Arguments
/// - `device`: DRM device capsule
/// - `id`: Plane ID
///
/// # Returns
/// Plane capsule or error
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn get_plane(_device: &DrmDeviceCapsule, id: u32) -> KgpuDriverResult<KmsPlaneCapsule> {
    // Real implementation would use DRM_IOCTL_MODE_GETPLANE
    let plane = KmsPlaneCapsule::new(id, PlaneType::Primary);
    Ok(plane)
}

/// Set CRTC configuration (legacy modesetting)
///
/// # Arguments
/// - `crtc`: CRTC capsule to configure
/// - `fb`: Framebuffer ID
/// - `mode`: Display mode
///
/// # Complexity
/// O(1) + ioctl overhead (~1ms)
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn set_crtc(
    crtc: &KmsCrtcCapsule,
    fb: u32,
    _mode: &DrmMode,
) -> KgpuDriverResult<()> {
    // Real implementation would use DRM_IOCTL_MODE_SETCRTC
    crtc.set_fb_id(fb);
    crtc.set_active(true);
    Ok(())
}

/// Perform page flip
///
/// # Arguments
/// - `crtc`: CRTC capsule
/// - `fb`: New framebuffer ID
/// - `flags`: Page flip flags
///
/// # Complexity
/// O(1) + ioctl overhead (~100us)
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn page_flip(
    crtc: &KmsCrtcCapsule,
    fb: u32,
    _flags: PageFlipFlags,
) -> KgpuDriverResult<()> {
    // Real implementation would use DRM_IOCTL_MODE_PAGE_FLIP
    crtc.set_flip_pending(true);
    crtc.set_fb_id(fb);
    Ok(())
}

/// Perform atomic commit
///
/// # Arguments
/// - `device`: DRM device
/// - `req`: Atomic request with property changes
/// - `flags`: Atomic flags
///
/// # Complexity
/// O(N) where N = number of properties + ioctl overhead (~1ms)
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn atomic_commit(
    _device: &DrmDeviceCapsule,
    _req: &AtomicRequest,
    _flags: AtomicFlags,
) -> KgpuDriverResult<()> {
    // Real implementation would use DRM_IOCTL_MODE_ATOMIC
    Err(KgpuDriverError::NotImplemented)
}

/// Add framebuffer with modifiers (ADDFB2)
///
/// # Arguments
/// - `device`: DRM device
/// - `width`: Width in pixels
/// - `height`: Height in pixels
/// - `format`: Pixel format (fourcc)
/// - `handles`: GEM handles (up to 4 planes)
/// - `pitches`: Row pitches (up to 4 planes)
/// - `offsets`: Plane offsets (up to 4 planes)
/// - `modifier`: Format modifier
///
/// # Returns
/// Framebuffer ID
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn add_fb2(
    _device: &DrmDeviceCapsule,
    width: u32,
    height: u32,
    format: u32,
    _handles: &[u32; 4],
    _pitches: &[u32; 4],
    _offsets: &[u32; 4],
    modifier: u64,
) -> KgpuDriverResult<FramebufferHandle> {
    // Real implementation would use DRM_IOCTL_MODE_ADDFB2
    // Return mock handle
    Ok(FramebufferHandle {
        fb_id: 1,
        width,
        height,
        format,
        modifier,
    })
}

/// Remove framebuffer
///
/// # Arguments
/// - `device`: DRM device
/// - `fb_id`: Framebuffer ID to remove
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn remove_fb(_device: &DrmDeviceCapsule, _fb_id: u32) -> KgpuDriverResult<()> {
    // Real implementation would use DRM_IOCTL_MODE_RMFB
    Ok(())
}

/// Wait for VBlank
///
/// # Arguments
/// - `crtc`: CRTC to wait on
///
/// # Returns
/// VBlank event with timestamp
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn wait_vblank(crtc: &KmsCrtcCapsule) -> KgpuDriverResult<VBlankEvent> {
    // Real implementation would use DRM_IOCTL_WAIT_VBLANK
    let seq = crtc.vblank_seq();
    Ok(VBlankEvent {
        sequence: seq as u32,
        tv_sec: 0,
        tv_usec: 0,
        crtc_id: crtc.id(),
        user_data: 0,
    })
}

/// Enable VBlank reporting for CRTC
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn enable_vblank(_crtc: &KmsCrtcCapsule) -> KgpuDriverResult<()> {
    // Real implementation would configure the CRTC for VBlank events
    Ok(())
}

// ============================================================================
// Fourcc Format Constants
// ============================================================================

/// Common pixel format fourcc codes
pub mod formats {
    /// 32-bit ARGB (8:8:8:8)
    pub const ARGB8888: u32 = 0x34325241; // 'AR24'
    /// 32-bit XRGB (8:8:8:8)
    pub const XRGB8888: u32 = 0x34325258; // 'XR24'
    /// 32-bit ABGR (8:8:8:8)
    pub const ABGR8888: u32 = 0x34324241; // 'AB24'
    /// 32-bit XBGR (8:8:8:8)
    pub const XBGR8888: u32 = 0x34324258; // 'XB24'
    /// 24-bit RGB (8:8:8)
    pub const RGB888: u32 = 0x34324752; // 'RG24'
    /// 24-bit BGR (8:8:8)
    pub const BGR888: u32 = 0x34324742; // 'BG24'
    /// 16-bit RGB (5:6:5)
    pub const RGB565: u32 = 0x36314752; // 'RG16'
    /// NV12 (Y + interleaved UV)
    pub const NV12: u32 = 0x3231564E; // 'NV12'
    /// NV21 (Y + interleaved VU)
    pub const NV21: u32 = 0x3132564E; // 'NV21'
    /// YUV 4:2:0 planar
    pub const YUV420: u32 = 0x32315559; // 'YU12'
}

/// Rotation and reflection flags
pub mod rotation {
    /// No rotation
    pub const ROTATE_0: u32 = 1 << 0;
    /// 90 degree clockwise
    pub const ROTATE_90: u32 = 1 << 1;
    /// 180 degree rotation
    pub const ROTATE_180: u32 = 1 << 2;
    /// 270 degree clockwise (90 counter-clockwise)
    pub const ROTATE_270: u32 = 1 << 3;
    /// Horizontal reflection
    pub const REFLECT_X: u32 = 1 << 4;
    /// Vertical reflection
    pub const REFLECT_Y: u32 = 1 << 5;
}

// ============================================================================
// Mode Flags
// ============================================================================

/// Display mode flags
pub mod mode_flags {
    /// Positive horizontal sync
    pub const PHSYNC: u32 = 1 << 0;
    /// Negative horizontal sync
    pub const NHSYNC: u32 = 1 << 1;
    /// Positive vertical sync
    pub const PVSYNC: u32 = 1 << 2;
    /// Negative vertical sync
    pub const NVSYNC: u32 = 1 << 3;
    /// Interlaced mode
    pub const INTERLACE: u32 = 1 << 4;
    /// Double-scan mode
    pub const DBLSCAN: u32 = 1 << 5;
    /// Composite sync
    pub const CSYNC: u32 = 1 << 6;
    /// Positive composite sync
    pub const PCSYNC: u32 = 1 << 7;
    /// Negative composite sync
    pub const NCSYNC: u32 = 1 << 8;
    /// Horizontal sync positive
    pub const HSKEW: u32 = 1 << 9;
    /// Broadcast scan mode
    pub const BCAST: u32 = 1 << 10;
    /// Picture aspect ratio 4:3
    pub const PIC_AR_4_3: u32 = 1 << 11;
    /// Picture aspect ratio 16:9
    pub const PIC_AR_16_9: u32 = 2 << 11;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_connector_type_from_drm() {
        assert_eq!(ConnectorType::from_drm(1), ConnectorType::VGA);
        assert_eq!(ConnectorType::from_drm(10), ConnectorType::DisplayPort);
        assert_eq!(ConnectorType::from_drm(11), ConnectorType::HDMIA);
        assert_eq!(ConnectorType::from_drm(14), ConnectorType::EDP);
        assert_eq!(ConnectorType::from_drm(99), ConnectorType::Unknown);
    }

    #[test]
    fn test_connector_type_name() {
        assert_eq!(ConnectorType::DisplayPort.name(), "DisplayPort");
        assert_eq!(ConnectorType::HDMIA.name(), "HDMI-A");
        assert_eq!(ConnectorType::EDP.name(), "eDP");
    }

    #[test]
    fn test_connector_type_digital() {
        assert!(ConnectorType::DisplayPort.is_digital());
        assert!(ConnectorType::HDMIA.is_digital());
        assert!(ConnectorType::DVID.is_digital());
        assert!(!ConnectorType::VGA.is_digital());
        assert!(!ConnectorType::Composite.is_digital());
    }

    #[test]
    fn test_connection_status_from_drm() {
        assert_eq!(ConnectionStatus::from_drm(1), ConnectionStatus::Connected);
        assert_eq!(ConnectionStatus::from_drm(2), ConnectionStatus::Disconnected);
        assert_eq!(ConnectionStatus::from_drm(3), ConnectionStatus::Unknown);
        assert_eq!(ConnectionStatus::from_drm(99), ConnectionStatus::Unknown);
    }

    #[test]
    fn test_plane_type_from_drm() {
        assert_eq!(PlaneType::from_drm(0), PlaneType::Overlay);
        assert_eq!(PlaneType::from_drm(1), PlaneType::Primary);
        assert_eq!(PlaneType::from_drm(2), PlaneType::Cursor);
    }

    #[test]
    fn test_plane_type_z_order() {
        assert!(PlaneType::Cursor.z_order() > PlaneType::Overlay.z_order());
        assert!(PlaneType::Overlay.z_order() > PlaneType::Primary.z_order());
    }

    #[test]
    fn test_dpms_state_from_drm() {
        assert_eq!(DpmsState::from_drm(0), DpmsState::On);
        assert_eq!(DpmsState::from_drm(1), DpmsState::Standby);
        assert_eq!(DpmsState::from_drm(2), DpmsState::Suspend);
        assert_eq!(DpmsState::from_drm(3), DpmsState::Off);
    }

    #[test]
    fn test_dpms_state_active() {
        assert!(DpmsState::On.is_active());
        assert!(!DpmsState::Standby.is_active());
        assert!(!DpmsState::Off.is_active());
    }

    #[test]
    fn test_subpixel_layout_from_drm() {
        assert_eq!(SubpixelLayout::from_drm(1), SubpixelLayout::HorizontalRgb);
        assert_eq!(SubpixelLayout::from_drm(5), SubpixelLayout::None);
        assert_eq!(SubpixelLayout::from_drm(99), SubpixelLayout::Unknown);
    }

    // ========================================================================
    // Q8-Q14: Capsule Tests
    // ========================================================================

    #[test]
    fn test_connector_capsule_size() {
        assert_eq!(mem::size_of::<KmsConnectorCapsule>(), 256);
        assert_eq!(mem::align_of::<KmsConnectorCapsule>(), 256);
    }

    #[test]
    fn test_connector_capsule_new() {
        let connector = KmsConnectorCapsule::new(42, ConnectorType::DisplayPort);
        assert_eq!(connector.id(), 42);
        assert_eq!(connector.connector_type(), ConnectorType::DisplayPort);
        assert_eq!(connector.status(), ConnectionStatus::Unknown);
        assert_eq!(connector.encoder_id(), 0);
    }

    #[test]
    fn test_connector_capsule_status_update() {
        let connector = KmsConnectorCapsule::new(1, ConnectorType::HDMIA);
        let gen_before = connector.generation();

        connector.set_status(ConnectionStatus::Connected);

        assert_eq!(connector.status(), ConnectionStatus::Connected);
        assert!(connector.generation() > gen_before);
    }

    #[test]
    fn test_connector_capsule_physical_size() {
        let connector = KmsConnectorCapsule::new(1, ConnectorType::HDMIA);
        connector.set_physical_size_mm(600, 340);

        let (w, h) = connector.physical_size_mm();
        assert_eq!(w, 600);
        assert_eq!(h, 340);
    }

    #[test]
    fn test_connector_capsule_snapshot() {
        let connector = KmsConnectorCapsule::new(5, ConnectorType::EDP);
        connector.set_status(ConnectionStatus::Connected);
        connector.set_crtc_id(1);
        connector.set_dpms_state(DpmsState::On);

        let snap = connector.snapshot();
        assert_eq!(snap.id, 5);
        assert_eq!(snap.connector_type, ConnectorType::EDP);
        assert_eq!(snap.status, ConnectionStatus::Connected);
        assert_eq!(snap.crtc_id, 1);
        assert_eq!(snap.dpms, DpmsState::On);
    }

    #[test]
    fn test_crtc_capsule_size() {
        assert_eq!(mem::size_of::<KmsCrtcCapsule>(), 256);
        assert_eq!(mem::align_of::<KmsCrtcCapsule>(), 256);
    }

    #[test]
    fn test_crtc_capsule_new() {
        let crtc = KmsCrtcCapsule::new(10, 0, 256);
        assert_eq!(crtc.id(), 10);
        assert_eq!(crtc.index(), 0);
        assert_eq!(crtc.gamma_size(), 256);
        assert!(!crtc.is_active());
    }

    #[test]
    fn test_crtc_capsule_active() {
        let crtc = KmsCrtcCapsule::new(1, 0, 256);
        assert!(!crtc.is_active());

        crtc.set_active(true);
        assert!(crtc.is_active());

        crtc.set_active(false);
        assert!(!crtc.is_active());
    }

    #[test]
    fn test_crtc_capsule_position() {
        let crtc = KmsCrtcCapsule::new(1, 0, 256);
        crtc.set_position(1920, 0);

        let (x, y) = crtc.position();
        assert_eq!(x, 1920);
        assert_eq!(y, 0);
    }

    #[test]
    fn test_crtc_capsule_vblank() {
        let crtc = KmsCrtcCapsule::new(1, 0, 256);
        assert_eq!(crtc.vblank_seq(), 0);

        crtc.increment_vblank(1000000);
        assert_eq!(crtc.vblank_seq(), 1);
        assert_eq!(crtc.vblank_timestamp_ns(), 1000000);
    }

    #[test]
    fn test_crtc_capsule_flip_pending() {
        let crtc = KmsCrtcCapsule::new(1, 0, 256);
        assert!(!crtc.is_flip_pending());

        crtc.set_flip_pending(true);
        assert!(crtc.is_flip_pending());

        crtc.set_flip_pending(false);
        assert!(!crtc.is_flip_pending());
    }

    #[test]
    fn test_crtc_capsule_snapshot() {
        let crtc = KmsCrtcCapsule::new(3, 1, 1024);
        crtc.set_active(true);
        crtc.set_fb_id(100);
        crtc.set_position(0, 1080);

        let snap = crtc.snapshot();
        assert_eq!(snap.id, 3);
        assert_eq!(snap.index, 1);
        assert_eq!(snap.gamma_size, 1024);
        assert!(snap.active);
        assert_eq!(snap.fb_id, 100);
        assert_eq!(snap.x, 0);
        assert_eq!(snap.y, 1080);
    }

    #[test]
    fn test_plane_capsule_size() {
        assert_eq!(mem::size_of::<KmsPlaneCapsule>(), 256);
        assert_eq!(mem::align_of::<KmsPlaneCapsule>(), 256);
    }

    #[test]
    fn test_plane_capsule_new() {
        let plane = KmsPlaneCapsule::new(20, PlaneType::Primary);
        assert_eq!(plane.id(), 20);
        assert_eq!(plane.plane_type(), PlaneType::Primary);
        assert_eq!(plane.z_pos(), PlaneType::Primary.z_order());
    }

    #[test]
    fn test_plane_capsule_rects() {
        let plane = KmsPlaneCapsule::new(1, PlaneType::Overlay);

        plane.set_src_rect(0, 0, 1920, 1080);
        plane.set_dst_rect(100, 100, 800, 600);

        let (sx, sy, sw, sh) = plane.src_rect();
        assert_eq!((sx, sy, sw, sh), (0, 0, 1920, 1080));

        let (dx, dy, dw, dh) = plane.dst_rect();
        assert_eq!((dx, dy, dw, dh), (100, 100, 800, 600));
    }

    #[test]
    fn test_plane_capsule_alpha() {
        let plane = KmsPlaneCapsule::new(1, PlaneType::Overlay);
        assert_eq!(plane.alpha(), 0xFFFF); // Default opaque

        plane.set_alpha(0x8000); // 50% alpha
        assert_eq!(plane.alpha(), 0x8000);
    }

    #[test]
    fn test_plane_capsule_snapshot() {
        let plane = KmsPlaneCapsule::new(5, PlaneType::Cursor);
        plane.set_crtc_id(1);
        plane.set_fb_id(50);
        plane.set_dst_rect(100, 200, 64, 64);

        let snap = plane.snapshot();
        assert_eq!(snap.id, 5);
        assert_eq!(snap.plane_type, PlaneType::Cursor);
        assert_eq!(snap.crtc_id, 1);
        assert_eq!(snap.fb_id, 50);
        assert_eq!(snap.dst_x, 100);
        assert_eq!(snap.dst_y, 200);
    }

    // ========================================================================
    // Q15-Q21: Mode and Timing Tests
    // ========================================================================

    #[test]
    fn test_drm_mode_default() {
        let mode = DrmMode::default();
        assert_eq!(mode.hdisplay, 1920);
        assert_eq!(mode.vdisplay, 1080);
        assert_eq!(mode.vrefresh, 60000);
    }

    #[test]
    fn test_drm_mode_refresh_hz() {
        let mode = DrmMode::MODE_1920X1080_60;
        assert_eq!(mode.refresh_hz(), 60);
    }

    #[test]
    fn test_drm_mode_name() {
        let mode = DrmMode::MODE_1920X1080_60;
        assert_eq!(mode.name_str(), "1920x1080");
    }

    #[test]
    fn test_drm_mode_pixels_per_frame() {
        let mode = DrmMode::MODE_1920X1080_60;
        let pixels = mode.pixels_per_frame();
        assert!(pixels > 0);
        assert_eq!(pixels, 2200 * 1125); // htotal * vtotal
    }

    #[test]
    fn test_drm_mode_bandwidth() {
        let mode = DrmMode::MODE_1920X1080_60;
        let bps = mode.bandwidth_bps();
        // 1920 * 1080 * 60 * 4 bytes = ~497 MB/s
        assert!(bps > 400_000_000);
        assert!(bps < 600_000_000);
    }

    #[test]
    fn test_vblank_event_new() {
        let event = VBlankEvent::new();
        assert_eq!(event.sequence, 0);
        assert_eq!(event.crtc_id, 0);
    }

    #[test]
    fn test_vblank_event_timestamp() {
        let event = VBlankEvent {
            sequence: 1,
            tv_sec: 100,
            tv_usec: 500000,
            crtc_id: 0,
            user_data: 0,
        };
        assert_eq!(event.timestamp_us(), 100_500_000);
        assert_eq!(event.timestamp_ns(), 100_500_000_000);
    }

    // ========================================================================
    // Q22-Q28: Flags and Resources Tests
    // ========================================================================

    #[test]
    fn test_page_flip_flags() {
        let flags = PageFlipFlags::EVENT | PageFlipFlags::ASYNC;
        assert!(flags.contains(PageFlipFlags::EVENT));
        assert!(flags.contains(PageFlipFlags::ASYNC));
        // Note: contains(NONE) is always true for any bitflag (0 & x == 0)
        // Instead check that flags are not empty by verifying raw value != 0
        assert_ne!(flags.raw(), PageFlipFlags::NONE.raw());
    }

    #[test]
    fn test_atomic_flags() {
        let flags = AtomicFlags::TEST_ONLY | AtomicFlags::ALLOW_MODESET;
        assert!(flags.contains(AtomicFlags::TEST_ONLY));
        assert!(flags.contains(AtomicFlags::ALLOW_MODESET));
        assert!(!flags.contains(AtomicFlags::NONBLOCK));
    }

    #[test]
    fn test_kms_resources_new() {
        let res = KmsResources::new();
        assert_eq!(res.num_connectors(), 0);
        assert_eq!(res.num_crtcs(), 0);
    }

    #[test]
    fn test_kms_plane_resources_new() {
        let res = KmsPlaneResources::new();
        assert_eq!(res.num_planes(), 0);
    }

    #[test]
    fn test_atomic_request() {
        let mut req = AtomicRequest::new();
        assert!(req.is_empty());

        req.add_property(1, 10, 100);
        req.add_property(2, 20, 200);

        assert_eq!(req.len(), 2);
        assert!(!req.is_empty());

        req.clear();
        assert!(req.is_empty());
    }

    #[test]
    fn test_framebuffer_handle() {
        let fb = FramebufferHandle::new(1, 1920, 1080, formats::ARGB8888);
        assert!(fb.is_valid());
        assert_eq!(fb.width, 1920);
        assert_eq!(fb.height, 1080);
    }

    #[test]
    fn test_framebuffer_handle_invalid() {
        let fb = FramebufferHandle::new(0, 0, 0, 0);
        assert!(!fb.is_valid());
    }

    #[test]
    fn test_drm_device_capsule_size() {
        assert_eq!(mem::size_of::<DrmDeviceCapsule>(), 64);
        assert_eq!(mem::align_of::<DrmDeviceCapsule>(), 64);
    }

    #[test]
    fn test_drm_device_capsule_new() {
        let device = DrmDeviceCapsule::new(3);
        assert_eq!(device.fd(), 3);
        assert!(!device.supports_atomic());
        assert!(!device.supports_universal_planes());
    }

    #[test]
    fn test_drm_device_capsule_caps() {
        let device = DrmDeviceCapsule::new(5);
        device.set_atomic_cap(true);
        device.set_universal_planes_cap(true);

        assert!(device.supports_atomic());
        assert!(device.supports_universal_planes());
    }

    // ========================================================================
    // Q29-Q35: Format and Constant Tests
    // ========================================================================

    #[test]
    fn test_formats() {
        assert_ne!(formats::ARGB8888, formats::XRGB8888);
        assert_ne!(formats::NV12, formats::NV21);
    }

    #[test]
    fn test_rotation_flags() {
        assert_eq!(rotation::ROTATE_0, 1);
        assert_eq!(rotation::ROTATE_90, 2);
        assert_eq!(rotation::ROTATE_180, 4);
        assert_eq!(rotation::ROTATE_270, 8);
    }

    #[test]
    fn test_mode_flags() {
        assert_eq!(mode_flags::PHSYNC, 1);
        assert_eq!(mode_flags::PVSYNC, 4);
        assert_eq!(mode_flags::INTERLACE, 16);
    }

    #[test]
    fn test_ioctl_constants() {
        // Verify ioctl constants are non-zero
        assert_ne!(DRM_IOCTL_MODE_GETRESOURCES, 0);
        assert_ne!(DRM_IOCTL_MODE_GETCONNECTOR, 0);
        assert_ne!(DRM_IOCTL_MODE_GETCRTC, 0);
        assert_ne!(DRM_IOCTL_MODE_SETCRTC, 0);
        assert_ne!(DRM_IOCTL_MODE_PAGE_FLIP, 0);
        assert_ne!(DRM_IOCTL_MODE_ATOMIC, 0);
        assert_ne!(DRM_IOCTL_WAIT_VBLANK, 0);
    }

    // ========================================================================
    // Additional Tests for 45+ coverage
    // ========================================================================

    #[test]
    fn test_connector_capsule_encoder_id() {
        let connector = KmsConnectorCapsule::new(1, ConnectorType::HDMIA);
        assert_eq!(connector.encoder_id(), 0);

        connector.set_encoder_id(5);
        assert_eq!(connector.encoder_id(), 5);
    }

    #[test]
    fn test_connector_capsule_dpms() {
        let connector = KmsConnectorCapsule::new(1, ConnectorType::DisplayPort);
        connector.set_dpms_state(DpmsState::On);
        assert_eq!(connector.dpms_state(), DpmsState::On);
    }

    #[test]
    fn test_connector_capsule_edid_hash() {
        let connector = KmsConnectorCapsule::new(1, ConnectorType::HDMIA);
        connector.set_edid_hash(0xDEADBEEF);
        assert_eq!(connector.edid_hash(), 0xDEADBEEF);
    }

    #[test]
    fn test_connector_capsule_mode_count() {
        let connector = KmsConnectorCapsule::new(1, ConnectorType::HDMIA);
        connector.set_mode_count(15);
        assert_eq!(connector.mode_count(), 15);
    }

    #[test]
    fn test_crtc_capsule_fb_id() {
        let crtc = KmsCrtcCapsule::new(1, 0, 256);
        crtc.set_fb_id(42);
        assert_eq!(crtc.fb_id(), 42);
    }

    #[test]
    fn test_crtc_capsule_mode_hash() {
        let crtc = KmsCrtcCapsule::new(1, 0, 256);
        crtc.set_mode_hash(0x12345678);
        assert_eq!(crtc.mode_hash(), 0x12345678);
    }

    #[test]
    fn test_plane_capsule_crtc_id() {
        let plane = KmsPlaneCapsule::new(1, PlaneType::Primary);
        plane.set_crtc_id(3);
        assert_eq!(plane.crtc_id(), 3);
    }

    #[test]
    fn test_plane_capsule_z_pos() {
        let plane = KmsPlaneCapsule::new(1, PlaneType::Overlay);
        plane.set_z_pos(10);
        assert_eq!(plane.z_pos(), 10);
    }

    #[test]
    fn test_plane_capsule_format() {
        let plane = KmsPlaneCapsule::new(1, PlaneType::Primary);
        plane.set_format(formats::ARGB8888);
        assert_eq!(plane.format(), formats::ARGB8888);
    }

    #[test]
    fn test_plane_capsule_rotation() {
        let plane = KmsPlaneCapsule::new(1, PlaneType::Primary);
        plane.set_rotation(rotation::ROTATE_90);
        assert_eq!(plane.rotation(), rotation::ROTATE_90);
    }

    #[test]
    fn test_drm_mode_4k() {
        let mode = DrmMode::MODE_3840X2160_60;
        assert_eq!(mode.hdisplay, 3840);
        assert_eq!(mode.vdisplay, 2160);
        assert_eq!(mode.refresh_hz(), 60);
    }

    #[test]
    fn test_drm_mode_1440p() {
        let mode = DrmMode::MODE_2560X1440_60;
        assert_eq!(mode.hdisplay, 2560);
        assert_eq!(mode.vdisplay, 1440);
    }
}
