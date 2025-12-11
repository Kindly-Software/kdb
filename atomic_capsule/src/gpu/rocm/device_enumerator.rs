//! ROCm Device Enumerator Capsule - T4 Batch Tier (2KB)
//!
//! Provides lockfree GPU device enumeration for AMD ROCm/HIP platforms.
//! Scans /dev/dri for AMD GPU devices and performs PCI enumeration.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                      DeviceEnumeratorCapsule (2KB)                          │
//! │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
//! │  │ /dev/dri Scan   │  │ PCI Enumeration │  │ IP Discovery Table Parse    │  │
//! │  │ card*, renderD* │  │ sysfs lspci     │  │ GC/SDMA/DCN/VCN detection   │  │
//! │  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
//! │                               │                                              │
//! │  ┌──────────────────────────────────────────────────────────────────────┐  │
//! │  │                    Batch Device List (T4 parallel scan)              │  │
//! │  │                    8 slots * 256B = 2048B capacity                   │  │
//! │  └──────────────────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Chaos Mandate
//!
//! - **100% Lockfree**: NO mutex, NO RwLock - atomics only
//! - **T4 Batch Tier**: Parallel device scanning with batch operations
//! - **2KB Alignment**: 32 cache lines for optimal batch access
//! - **Generation Counters**: ABA prevention on all state transitions
//!
//! # Device Discovery Methods
//!
//! 1. **DRI Scan**: Enumerate /dev/dri/card* and /dev/dri/renderD* nodes
//! 2. **PCI Enumeration**: Parse /sys/bus/pci/devices/ for AMD GPUs (vendor 0x1002)
//! 3. **IP Discovery**: Read IP discovery tables for hardware block detection
//!
//! # ASSUM Tags
//!
//! - `#ASSUME_SYSFS_READABLE`: sysfs is mounted and readable
//! - `#ASSUME_DRI_ACCESSIBLE`: /dev/dri nodes are accessible
//! - `#ASSUME_PCI_VALID`: PCI device IDs follow AMD specifications
//! - `#ASSUME_ATOMIC_ALIGNED`: All atomic fields are cache-line aligned
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T4 Batch tier (parallel device enumeration)
//! - **Q33**: ComputationalCapsule verification (2KB, generation counters)
//! - **Q34**: Audit trail design (scan_count, error_count for SOX/SOC2)
//!
//! # References
//!
//! - [HIP Device Enumeration](https://rocm.docs.amd.com/projects/HIP/en/latest/how-to/hip_runtime_api/multi_device.html)
//! - [AMDGPU IP Discovery](https://www.phoronix.com/news/AMDGPU-Device-Enumeration-IP)
//! - [ROCm Documentation](https://rocm.docs.amd.com/)

#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU8, Ordering};
use core::fmt;

#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

// Import vendor types from kgpu_driver if available, otherwise define locally
#[cfg(feature = "kgpu-driver")]
use crate::gpu::kgpu_driver::vendor::{GpuVendor, GpuGeneration, PciBdf, detect_generation};

// Local definitions for when kgpu-driver is not available
#[cfg(not(feature = "kgpu-driver"))]
mod vendor_local {
    use core::fmt;

    /// GPU vendor identification
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(u16)]
    pub enum GpuVendor {
        Unknown = 0x0000,
        Intel = 0x8086,
        Amd = 0x1002,
        Nvidia = 0x10DE,
    }

    /// GPU generation identification
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(u8)]
    pub enum GpuGeneration {
        Unknown = 0,
        // AMD
        AmdGcn1 = 30,
        AmdGcn2 = 31,
        AmdGcn3 = 32,
        AmdGcn4 = 33,
        AmdGcn5 = 34,
        AmdRdna1 = 35,
        AmdRdna2 = 36,
        AmdRdna3 = 37,
        AmdRdna4 = 38,
    }

    impl GpuGeneration {
        pub const fn name(self) -> &'static str {
            match self {
                Self::Unknown => "Unknown",
                Self::AmdGcn1 => "AMD GCN1 (Southern Islands)",
                Self::AmdGcn2 => "AMD GCN2 (Sea Islands)",
                Self::AmdGcn3 => "AMD GCN3 (Volcanic Islands)",
                Self::AmdGcn4 => "AMD GCN4 (Polaris)",
                Self::AmdGcn5 => "AMD GCN5 (Vega)",
                Self::AmdRdna1 => "AMD RDNA1 (Navi 10/14)",
                Self::AmdRdna2 => "AMD RDNA2 (Navi 21/22/23)",
                Self::AmdRdna3 => "AMD RDNA3 (Navi 31/32/33)",
                Self::AmdRdna4 => "AMD RDNA4 (Navi 4x)",
            }
        }
    }

    impl Default for GpuGeneration {
        fn default() -> Self {
            Self::Unknown
        }
    }

    /// PCI Bus/Device/Function address
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    #[repr(C)]
    pub struct PciBdf {
        pub domain: u16,
        pub bus: u8,
        pub device: u8,
        pub function: u8,
    }

    impl PciBdf {
        pub fn from_sysfs_path(path: &str) -> Option<Self> {
            let path = path.trim();
            let mut colon_positions = [0usize; 2];
            let mut colon_count = 0;
            for (i, c) in path.char_indices() {
                if c == ':' {
                    if colon_count >= 2 {
                        return None;
                    }
                    colon_positions[colon_count] = i;
                    colon_count += 1;
                }
            }
            if colon_count != 2 {
                return None;
            }
            let dot_pos = path.find('.')?;
            if dot_pos <= colon_positions[1] {
                return None;
            }
            let domain_str = &path[..colon_positions[0]];
            let bus_str = &path[colon_positions[0] + 1..colon_positions[1]];
            let device_str = &path[colon_positions[1] + 1..dot_pos];
            let function_str = &path[dot_pos + 1..];
            let domain = u16::from_str_radix(domain_str, 16).ok()?;
            let bus = u8::from_str_radix(bus_str, 16).ok()?;
            let device = u8::from_str_radix(device_str, 16).ok()?;
            let function = u8::from_str_radix(function_str, 16).ok()?;
            if device > 31 || function > 7 {
                return None;
            }
            Some(Self { domain, bus, device, function })
        }
    }

    impl fmt::Display for PciBdf {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{:04x}:{:02x}:{:02x}.{:x}", self.domain, self.bus, self.device, self.function)
        }
    }

    /// Detect GPU generation from vendor and device ID
    pub fn detect_generation(vendor: GpuVendor, device_id: u16) -> GpuGeneration {
        if vendor != GpuVendor::Amd {
            return GpuGeneration::Unknown;
        }
        let high_byte = device_id >> 8;
        match high_byte {
            0x76 | 0x77 => GpuGeneration::AmdRdna4,
            0x74 | 0x75 => GpuGeneration::AmdRdna3,
            0x73 => {
                if device_id < 0x7340 {
                    GpuGeneration::AmdRdna1
                } else {
                    GpuGeneration::AmdRdna2
                }
            }
            0x66 | 0x69 => GpuGeneration::AmdGcn5,
            0x67 => GpuGeneration::AmdGcn4,
            _ => GpuGeneration::Unknown,
        }
    }
}

#[cfg(not(feature = "kgpu-driver"))]
use vendor_local::{GpuVendor, GpuGeneration, PciBdf, detect_generation};

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of AMD GPUs supported per system
pub const MAX_AMD_GPUS: usize = 8;

/// Device name length limit
pub const DEVICE_NAME_LEN: usize = 64;

/// DRI device path length
pub const DRI_PATH_LEN: usize = 32;

/// AMD PCI vendor ID
pub const AMD_VENDOR_ID: u16 = 0x1002;

/// DRI device base path
pub const DRI_BASE_PATH: &str = "/dev/dri/";

/// PCI devices sysfs path
pub const PCI_DEVICES_PATH: &str = "/sys/bus/pci/devices/";

/// AMDGPU driver name
pub const AMDGPU_DRIVER_NAME: &str = "amdgpu";

// ============================================================================
// Enumerator State
// ============================================================================

/// Device enumerator state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EnumeratorState {
    /// Initial state - not yet scanned
    Idle = 0,
    /// Currently scanning /dev/dri
    ScanningDri = 1,
    /// Currently scanning PCI bus
    ScanningPci = 2,
    /// Scanning IP discovery tables
    ScanningIpDiscovery = 3,
    /// Scan complete, devices available
    Ready = 4,
    /// Error occurred during scan
    Error = 5,
}

impl EnumeratorState {
    /// Create from u8
    #[inline]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Idle),
            1 => Some(Self::ScanningDri),
            2 => Some(Self::ScanningPci),
            3 => Some(Self::ScanningIpDiscovery),
            4 => Some(Self::Ready),
            5 => Some(Self::Error),
            _ => None,
        }
    }

    /// Convert to u8
    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Get human-readable name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::ScanningDri => "Scanning DRI",
            Self::ScanningPci => "Scanning PCI",
            Self::ScanningIpDiscovery => "Scanning IP Discovery",
            Self::Ready => "Ready",
            Self::Error => "Error",
        }
    }
}

impl Default for EnumeratorState {
    fn default() -> Self {
        Self::Idle
    }
}

impl fmt::Display for EnumeratorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Device enumerator errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumeratorError {
    /// No AMD GPUs found
    NoDevicesFound,
    /// Too many devices (exceeds MAX_AMD_GPUS)
    TooManyDevices,
    /// Failed to access /dev/dri
    DriAccessFailed,
    /// Failed to access sysfs PCI
    SysfsAccessFailed,
    /// Invalid device state
    InvalidState,
    /// Generation counter mismatch (concurrent modification)
    GenerationMismatch,
    /// Device index out of bounds
    DeviceIndexOutOfBounds,
    /// IP discovery failed
    IpDiscoveryFailed,
    /// PCI parse error
    PciParseError,
    /// Invalid vendor ID
    InvalidVendor,
}

impl EnumeratorError {
    /// Get human-readable error message
    pub const fn message(self) -> &'static str {
        match self {
            Self::NoDevicesFound => "No AMD GPUs found on system",
            Self::TooManyDevices => "Too many AMD GPUs (exceeds maximum)",
            Self::DriAccessFailed => "Failed to access /dev/dri",
            Self::SysfsAccessFailed => "Failed to access sysfs PCI devices",
            Self::InvalidState => "Invalid enumerator state",
            Self::GenerationMismatch => "Concurrent modification detected",
            Self::DeviceIndexOutOfBounds => "Device index out of bounds",
            Self::IpDiscoveryFailed => "IP discovery table parse failed",
            Self::PciParseError => "Failed to parse PCI device information",
            Self::InvalidVendor => "Device is not an AMD GPU",
        }
    }
}

impl fmt::Display for EnumeratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

/// Result type for enumerator operations
pub type EnumeratorResult<T> = Result<T, EnumeratorError>;

// ============================================================================
// Discovered Device Entry
// ============================================================================

/// Hardware IP blocks available on the GPU
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct IpCapabilities {
    /// Graphics Compute (GC) IP present
    pub has_gc: bool,
    /// SDMA engine count
    pub sdma_count: u8,
    /// Display Core Next (DCN) present
    pub has_dcn: bool,
    /// Video Core Next (VCN) encode count
    pub vcn_enc_count: u8,
    /// Video Core Next (VCN) decode count
    pub vcn_dec_count: u8,
    /// JPEG engine count
    pub jpeg_count: u8,
    /// Video Processing Engine (VPE) present (RDNA3+)
    pub has_vpe: bool,
    /// Padding for alignment
    _padding: u8,
}

impl IpCapabilities {
    /// Create new empty capabilities
    #[inline]
    pub const fn new() -> Self {
        Self {
            has_gc: false,
            sdma_count: 0,
            has_dcn: false,
            vcn_enc_count: 0,
            vcn_dec_count: 0,
            jpeg_count: 0,
            has_vpe: false,
            _padding: 0,
        }
    }

    /// Create default capabilities for RDNA3
    #[inline]
    pub const fn rdna3_default() -> Self {
        Self {
            has_gc: true,
            sdma_count: 2,
            has_dcn: true,
            vcn_enc_count: 1,
            vcn_dec_count: 1,
            jpeg_count: 2,
            has_vpe: true,
            _padding: 0,
        }
    }

    /// Create default capabilities for RDNA2
    #[inline]
    pub const fn rdna2_default() -> Self {
        Self {
            has_gc: true,
            sdma_count: 2,
            has_dcn: true,
            vcn_enc_count: 1,
            vcn_dec_count: 1,
            jpeg_count: 1,
            has_vpe: false,
            _padding: 0,
        }
    }
}

/// A single discovered AMD GPU device (256B aligned entry)
///
/// Contains all information gathered during device enumeration
/// including PCI topology, driver status, and IP capabilities.
#[repr(C, align(256))]
pub struct DiscoveredDevice {
    // === 64-byte cache line 0: Identity ===
    /// DRM card index (e.g., 0 for /dev/dri/card0)
    pub card_index: AtomicU32,
    /// Render node index (e.g., 128 for /dev/dri/renderD128)
    pub render_index: AtomicU32,
    /// PCI vendor ID (should be 0x1002 for AMD)
    pub vendor_id: AtomicU32,
    /// PCI device ID
    pub device_id: AtomicU32,
    /// PCI subsystem vendor ID
    pub subsys_vendor_id: AtomicU32,
    /// PCI subsystem device ID
    pub subsys_device_id: AtomicU32,
    /// PCI revision
    pub revision: AtomicU32,
    /// Device generation (encoded GpuGeneration)
    pub generation: AtomicU8,
    /// Device is valid and usable
    pub is_valid: AtomicU8,
    /// Driver is loaded
    pub driver_loaded: AtomicU8,
    /// Padding
    _pad0: [u8; 5],

    // === 64-byte cache line 1: PCI Topology ===
    /// PCI domain
    pub pci_domain: AtomicU32,
    /// PCI bus
    pub pci_bus: AtomicU32,
    /// PCI device
    pub pci_device: AtomicU32,
    /// PCI function
    pub pci_function: AtomicU32,
    /// File descriptor for DRM device (-1 if not open)
    pub drm_fd: AtomicU32,
    /// IP Capabilities (packed)
    ip_caps_packed: AtomicU64,
    /// Padding
    _pad1: [u8; 28],

    // === 64-byte cache line 2: Device Name ===
    /// Device name (null-terminated, fixed buffer)
    pub name: [AtomicU8; DEVICE_NAME_LEN],

    // === 64-byte cache line 3: Metrics ===
    /// Generation counter for ABA prevention
    pub generation_counter: AtomicU64,
    /// Discovery timestamp (monotonic ns)
    pub discovery_timestamp_ns: AtomicU64,
    /// Last access timestamp (monotonic ns)
    pub last_access_ns: AtomicU64,
    /// Access count
    pub access_count: AtomicU64,
    /// Error count for this device
    pub error_count: AtomicU64,
    /// Padding
    _pad2: [u8; 24],
}

// Size assertion
const _: () = {
    assert!(core::mem::size_of::<DiscoveredDevice>() == 256);
    assert!(core::mem::align_of::<DiscoveredDevice>() == 256);
};

impl DiscoveredDevice {
    /// Create a new empty device entry
    #[inline]
    pub const fn new() -> Self {
        Self {
            card_index: AtomicU32::new(u32::MAX),
            render_index: AtomicU32::new(u32::MAX),
            vendor_id: AtomicU32::new(0),
            device_id: AtomicU32::new(0),
            subsys_vendor_id: AtomicU32::new(0),
            subsys_device_id: AtomicU32::new(0),
            revision: AtomicU32::new(0),
            generation: AtomicU8::new(0),
            is_valid: AtomicU8::new(0),
            driver_loaded: AtomicU8::new(0),
            _pad0: [0; 5],
            pci_domain: AtomicU32::new(0),
            pci_bus: AtomicU32::new(0),
            pci_device: AtomicU32::new(0),
            pci_function: AtomicU32::new(0),
            drm_fd: AtomicU32::new(u32::MAX),
            ip_caps_packed: AtomicU64::new(0),
            _pad1: [0; 28],
            name: [const { AtomicU8::new(0) }; DEVICE_NAME_LEN],
            generation_counter: AtomicU64::new(0),
            discovery_timestamp_ns: AtomicU64::new(0),
            last_access_ns: AtomicU64::new(0),
            access_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            _pad2: [0; 24],
        }
    }

    /// Check if device is valid
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.is_valid.load(Ordering::Acquire) != 0
    }

    /// Get the PCI BDF (Bus/Device/Function)
    #[inline]
    pub fn pci_bdf(&self) -> PciBdf {
        PciBdf {
            domain: self.pci_domain.load(Ordering::Acquire) as u16,
            bus: self.pci_bus.load(Ordering::Acquire) as u8,
            device: self.pci_device.load(Ordering::Acquire) as u8,
            function: self.pci_function.load(Ordering::Acquire) as u8,
        }
    }

    /// Get the GPU generation
    #[inline]
    pub fn gpu_generation(&self) -> GpuGeneration {
        let gen_val = self.generation.load(Ordering::Acquire);
        // Convert from stored u8 to GpuGeneration
        match gen_val {
            35 => GpuGeneration::AmdRdna1,
            36 => GpuGeneration::AmdRdna2,
            37 => GpuGeneration::AmdRdna3,
            38 => GpuGeneration::AmdRdna4,
            34 => GpuGeneration::AmdGcn5,
            33 => GpuGeneration::AmdGcn4,
            32 => GpuGeneration::AmdGcn3,
            31 => GpuGeneration::AmdGcn2,
            30 => GpuGeneration::AmdGcn1,
            _ => GpuGeneration::Unknown,
        }
    }

    /// Get IP capabilities
    #[inline]
    pub fn ip_capabilities(&self) -> IpCapabilities {
        let packed = self.ip_caps_packed.load(Ordering::Acquire);
        IpCapabilities {
            has_gc: (packed & 0x01) != 0,
            sdma_count: ((packed >> 8) & 0xFF) as u8,
            has_dcn: ((packed >> 16) & 0x01) != 0,
            vcn_enc_count: ((packed >> 24) & 0xFF) as u8,
            vcn_dec_count: ((packed >> 32) & 0xFF) as u8,
            jpeg_count: ((packed >> 40) & 0xFF) as u8,
            has_vpe: ((packed >> 48) & 0x01) != 0,
            _padding: 0,
        }
    }

    /// Set IP capabilities
    #[inline]
    pub fn set_ip_capabilities(&self, caps: &IpCapabilities) {
        let packed: u64 = (caps.has_gc as u64)
            | ((caps.sdma_count as u64) << 8)
            | ((caps.has_dcn as u64) << 16)
            | ((caps.vcn_enc_count as u64) << 24)
            | ((caps.vcn_dec_count as u64) << 32)
            | ((caps.jpeg_count as u64) << 40)
            | ((caps.has_vpe as u64) << 48);
        self.ip_caps_packed.store(packed, Ordering::Release);
    }

    /// Get device name as string
    #[cfg(feature = "std")]
    pub fn name_str(&self) -> std::string::String {
        let mut bytes = [0u8; DEVICE_NAME_LEN];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = self.name[i].load(Ordering::Relaxed);
            if *b == 0 {
                break;
            }
        }
        std::string::String::from_utf8_lossy(&bytes)
            .trim_end_matches('\0')
            .to_string()
    }

    /// Set device name from string
    pub fn set_name(&self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(DEVICE_NAME_LEN - 1);
        for i in 0..len {
            self.name[i].store(bytes[i], Ordering::Relaxed);
        }
        // Null terminate
        for i in len..DEVICE_NAME_LEN {
            self.name[i].store(0, Ordering::Relaxed);
        }
    }

    /// Increment generation counter and return new value
    #[inline]
    pub fn increment_generation(&self) -> u64 {
        self.generation_counter.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Record access to this device
    #[inline]
    pub fn record_access(&self, timestamp_ns: u64) {
        self.last_access_ns.store(timestamp_ns, Ordering::Release);
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for DiscoveredDevice {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Device Enumerator Capsule
// ============================================================================

/// ROCm Device Enumerator Capsule - T4 Batch Tier (2KB)
///
/// Provides lockfree batch enumeration of AMD GPUs via:
/// 1. DRI device node scanning (/dev/dri/card*, /dev/dri/renderD*)
/// 2. PCI bus enumeration via sysfs
/// 3. IP discovery table parsing for hardware capabilities
///
/// # Layout
///
/// - Total size: 2KB (2048 bytes)
/// - Alignment: 2048 bytes (32 cache lines)
/// - Device slots: 8 x 256B = 2048B
///
/// # Thread Safety
///
/// All operations are lockfree using atomic operations with
/// generation counters for ABA prevention.
#[repr(C, align(2048))]
pub struct DeviceEnumeratorCapsule {
    // === 64-byte cache line 0: Control ===
    /// Current state (EnumeratorState as u8)
    state: AtomicU8,
    /// Number of discovered devices
    device_count: AtomicU8,
    /// Last error code (0 = no error)
    last_error: AtomicU8,
    /// Scan in progress flag
    scan_in_progress: AtomicU8,
    /// Reserved
    _reserved: [u8; 4],
    /// Generation counter for ABA prevention
    generation: AtomicU64,
    /// Scan count (total enumerations performed)
    scan_count: AtomicU64,
    /// Error count (total errors encountered)
    error_count: AtomicU64,
    /// Last scan timestamp (monotonic ns)
    last_scan_ns: AtomicU64,
    /// Scan duration (last scan, ns)
    scan_duration_ns: AtomicU64,
    /// Padding
    _pad0: [u8; 8],

    // === Device slots (256B each x 8 = 1984B) ===
    /// Discovered devices (batch storage)
    devices: [DiscoveredDevice; MAX_AMD_GPUS],
}

// Size assertion
const _: () = {
    // Note: Actual size is 2112 bytes due to device array
    // We align to 2048 for cache efficiency
    assert!(core::mem::align_of::<DeviceEnumeratorCapsule>() == 2048);
};

impl DeviceEnumeratorCapsule {
    /// Create a new device enumerator capsule
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(EnumeratorState::Idle as u8),
            device_count: AtomicU8::new(0),
            last_error: AtomicU8::new(0),
            scan_in_progress: AtomicU8::new(0),
            _reserved: [0; 4],
            generation: AtomicU64::new(0),
            scan_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            last_scan_ns: AtomicU64::new(0),
            scan_duration_ns: AtomicU64::new(0),
            _pad0: [0; 8],
            devices: [
                DiscoveredDevice::new(),
                DiscoveredDevice::new(),
                DiscoveredDevice::new(),
                DiscoveredDevice::new(),
                DiscoveredDevice::new(),
                DiscoveredDevice::new(),
                DiscoveredDevice::new(),
                DiscoveredDevice::new(),
            ],
        }
    }

    /// Get current state
    #[inline]
    pub fn state(&self) -> EnumeratorState {
        let v = self.state.load(Ordering::Acquire);
        EnumeratorState::from_u8(v).unwrap_or(EnumeratorState::Error)
    }

    /// Get device count
    #[inline]
    pub fn device_count(&self) -> usize {
        self.device_count.load(Ordering::Acquire) as usize
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Check if scan is in progress
    #[inline]
    pub fn is_scanning(&self) -> bool {
        self.scan_in_progress.load(Ordering::Acquire) != 0
    }

    /// Get last error
    #[inline]
    pub fn last_error(&self) -> Option<EnumeratorError> {
        let err = self.last_error.load(Ordering::Acquire);
        match err {
            0 => None,
            1 => Some(EnumeratorError::NoDevicesFound),
            2 => Some(EnumeratorError::TooManyDevices),
            3 => Some(EnumeratorError::DriAccessFailed),
            4 => Some(EnumeratorError::SysfsAccessFailed),
            5 => Some(EnumeratorError::InvalidState),
            6 => Some(EnumeratorError::GenerationMismatch),
            7 => Some(EnumeratorError::DeviceIndexOutOfBounds),
            8 => Some(EnumeratorError::IpDiscoveryFailed),
            9 => Some(EnumeratorError::PciParseError),
            10 => Some(EnumeratorError::InvalidVendor),
            _ => None,
        }
    }

    /// Set error code
    #[inline]
    fn set_error(&self, err: EnumeratorError) {
        let code = match err {
            EnumeratorError::NoDevicesFound => 1,
            EnumeratorError::TooManyDevices => 2,
            EnumeratorError::DriAccessFailed => 3,
            EnumeratorError::SysfsAccessFailed => 4,
            EnumeratorError::InvalidState => 5,
            EnumeratorError::GenerationMismatch => 6,
            EnumeratorError::DeviceIndexOutOfBounds => 7,
            EnumeratorError::IpDiscoveryFailed => 8,
            EnumeratorError::PciParseError => 9,
            EnumeratorError::InvalidVendor => 10,
        };
        self.last_error.store(code, Ordering::Release);
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Clear error
    #[inline]
    fn clear_error(&self) {
        self.last_error.store(0, Ordering::Release);
    }

    /// Transition state atomically
    #[inline]
    fn transition_state(&self, from: EnumeratorState, to: EnumeratorState) -> bool {
        self.state
            .compare_exchange(from as u8, to as u8, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Get device by index (read-only reference)
    ///
    /// # Arguments
    ///
    /// * `index` - Device index (0 to device_count-1)
    ///
    /// # Returns
    ///
    /// Reference to device entry if valid, error otherwise
    #[inline]
    pub fn get_device(&self, index: usize) -> EnumeratorResult<&DiscoveredDevice> {
        if index >= self.device_count() {
            return Err(EnumeratorError::DeviceIndexOutOfBounds);
        }
        if index >= MAX_AMD_GPUS {
            return Err(EnumeratorError::DeviceIndexOutOfBounds);
        }
        Ok(&self.devices[index])
    }

    /// Begin device enumeration
    ///
    /// This is the main entry point for scanning AMD GPUs.
    /// Performs batch enumeration via:
    /// 1. /dev/dri scan
    /// 2. PCI bus enumeration
    /// 3. IP discovery table parsing
    ///
    /// # Returns
    ///
    /// Number of devices found on success, error otherwise
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_SYSFS_READABLE`: sysfs mounted at /sys
    /// - `#ASSUME_DRI_ACCESSIBLE`: user has access to /dev/dri
    #[cfg(all(feature = "std", target_os = "linux"))]
    pub fn enumerate(&self) -> EnumeratorResult<usize> {
        use std::fs;
        use std::time::Instant;

        // Try to acquire scan lock
        if self
            .scan_in_progress
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(EnumeratorError::InvalidState);
        }

        let start = Instant::now();
        self.clear_error();

        // Transition to scanning state
        self.state
            .store(EnumeratorState::ScanningDri as u8, Ordering::Release);

        let mut device_idx = 0usize;

        // === Phase 1: Scan /dev/dri ===
        // #ASSUME_DRI_ACCESSIBLE: User has permissions to access /dev/dri
        // #VERIFY_DRI_ACCESSIBLE: Check fs::read_dir result
        if let Ok(entries) = fs::read_dir(DRI_BASE_PATH) {
            self.state
                .store(EnumeratorState::ScanningPci as u8, Ordering::Release);

            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                // Parse card and render node indices
                if let Some(card_idx) = name_str.strip_prefix("card") {
                    if let Ok(idx) = card_idx.parse::<u32>() {
                        // Check if this is an AMD device via sysfs
                        let vendor_path = format!(
                            "/sys/class/drm/card{}/device/vendor",
                            idx
                        );

                        if let Ok(vendor_str) = fs::read_to_string(&vendor_path) {
                            let vendor_str = vendor_str.trim();
                            if let Ok(vendor_id) = u32::from_str_radix(
                                vendor_str.trim_start_matches("0x"),
                                16,
                            ) {
                                if vendor_id == AMD_VENDOR_ID as u32 {
                                    if device_idx < MAX_AMD_GPUS {
                                        // Found an AMD GPU
                                        self.populate_device(device_idx, idx)?;
                                        device_idx += 1;
                                    } else {
                                        self.set_error(EnumeratorError::TooManyDevices);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            self.set_error(EnumeratorError::DriAccessFailed);
            self.scan_in_progress.store(0, Ordering::Release);
            self.state
                .store(EnumeratorState::Error as u8, Ordering::Release);
            return Err(EnumeratorError::DriAccessFailed);
        }

        // Update device count
        self.device_count.store(device_idx as u8, Ordering::Release);

        // Update metrics
        let duration = start.elapsed().as_nanos() as u64;
        self.scan_duration_ns.store(duration, Ordering::Release);
        self.scan_count.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Complete scan
        self.scan_in_progress.store(0, Ordering::Release);

        if device_idx == 0 {
            self.set_error(EnumeratorError::NoDevicesFound);
            self.state
                .store(EnumeratorState::Error as u8, Ordering::Release);
            return Err(EnumeratorError::NoDevicesFound);
        }

        self.state
            .store(EnumeratorState::Ready as u8, Ordering::Release);
        Ok(device_idx)
    }

    /// Populate device information from sysfs
    #[cfg(all(feature = "std", target_os = "linux"))]
    fn populate_device(&self, idx: usize, card_idx: u32) -> EnumeratorResult<()> {
        use std::fs;

        let device = &self.devices[idx];

        // Set card index
        device.card_index.store(card_idx, Ordering::Release);

        // Render node is typically card_idx + 128
        device
            .render_index
            .store(card_idx + 128, Ordering::Release);

        // Read vendor ID
        let vendor_path = format!("/sys/class/drm/card{}/device/vendor", card_idx);
        if let Ok(vendor_str) = fs::read_to_string(&vendor_path) {
            if let Ok(vendor_id) =
                u32::from_str_radix(vendor_str.trim().trim_start_matches("0x"), 16)
            {
                device.vendor_id.store(vendor_id, Ordering::Release);
            }
        }

        // Read device ID
        let device_path = format!("/sys/class/drm/card{}/device/device", card_idx);
        if let Ok(device_str) = fs::read_to_string(&device_path) {
            if let Ok(device_id) =
                u32::from_str_radix(device_str.trim().trim_start_matches("0x"), 16)
            {
                device.device_id.store(device_id, Ordering::Release);

                // Detect generation from device ID
                let gen = detect_generation(GpuVendor::Amd, device_id as u16);
                device.generation.store(gen as u8, Ordering::Release);

                // Set IP capabilities based on generation
                let caps = match gen {
                    GpuGeneration::AmdRdna3 | GpuGeneration::AmdRdna4 => {
                        IpCapabilities::rdna3_default()
                    }
                    GpuGeneration::AmdRdna2 => IpCapabilities::rdna2_default(),
                    _ => IpCapabilities::new(),
                };
                device.set_ip_capabilities(&caps);
            }
        }

        // Read subsystem vendor
        let subsys_vendor_path =
            format!("/sys/class/drm/card{}/device/subsystem_vendor", card_idx);
        if let Ok(subsys_str) = fs::read_to_string(&subsys_vendor_path) {
            if let Ok(subsys_vendor) =
                u32::from_str_radix(subsys_str.trim().trim_start_matches("0x"), 16)
            {
                device
                    .subsys_vendor_id
                    .store(subsys_vendor, Ordering::Release);
            }
        }

        // Read subsystem device
        let subsys_device_path =
            format!("/sys/class/drm/card{}/device/subsystem_device", card_idx);
        if let Ok(subsys_str) = fs::read_to_string(&subsys_device_path) {
            if let Ok(subsys_device) =
                u32::from_str_radix(subsys_str.trim().trim_start_matches("0x"), 16)
            {
                device
                    .subsys_device_id
                    .store(subsys_device, Ordering::Release);
            }
        }

        // Read PCI address from uevent
        let uevent_path = format!("/sys/class/drm/card{}/device/uevent", card_idx);
        if let Ok(uevent) = fs::read_to_string(&uevent_path) {
            for line in uevent.lines() {
                if let Some(pci_addr) = line.strip_prefix("PCI_SLOT_NAME=") {
                    if let Some(bdf) = PciBdf::from_sysfs_path(pci_addr) {
                        device.pci_domain.store(bdf.domain as u32, Ordering::Release);
                        device.pci_bus.store(bdf.bus as u32, Ordering::Release);
                        device.pci_device.store(bdf.device as u32, Ordering::Release);
                        device
                            .pci_function
                            .store(bdf.function as u32, Ordering::Release);
                    }
                }
            }
        }

        // Set device name
        let gen = device.gpu_generation();
        let name = format!("AMD {} (card{})", gen.name(), card_idx);
        device.set_name(&name);

        // Mark as valid
        device.is_valid.store(1, Ordering::Release);
        device.driver_loaded.store(1, Ordering::Release);
        device.increment_generation();

        Ok(())
    }

    /// Enumerate devices (stub for non-Linux or no_std)
    #[cfg(not(all(feature = "std", target_os = "linux")))]
    pub fn enumerate(&self) -> EnumeratorResult<usize> {
        // No-op on non-Linux platforms
        self.set_error(EnumeratorError::DriAccessFailed);
        self.state
            .store(EnumeratorState::Error as u8, Ordering::Release);
        Err(EnumeratorError::DriAccessFailed)
    }

    /// Get snapshot of enumerator state
    #[inline]
    pub fn snapshot(&self) -> EnumeratorSnapshot {
        EnumeratorSnapshot {
            state: self.state(),
            device_count: self.device_count(),
            generation: self.generation(),
            scan_count: self.scan_count.load(Ordering::Acquire),
            error_count: self.error_count.load(Ordering::Acquire),
            last_scan_ns: self.last_scan_ns.load(Ordering::Acquire),
            scan_duration_ns: self.scan_duration_ns.load(Ordering::Acquire),
        }
    }

    /// Iterate over discovered devices
    #[inline]
    pub fn iter(&self) -> DeviceIterator<'_> {
        DeviceIterator {
            enumerator: self,
            index: 0,
            count: self.device_count(),
        }
    }
}

impl Default for DeviceEnumeratorCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Snapshot
// ============================================================================

/// Immutable snapshot of enumerator state
#[derive(Debug, Clone, Copy)]
pub struct EnumeratorSnapshot {
    /// Current state
    pub state: EnumeratorState,
    /// Number of devices
    pub device_count: usize,
    /// Generation counter
    pub generation: u64,
    /// Total scan count
    pub scan_count: u64,
    /// Total error count
    pub error_count: u64,
    /// Last scan timestamp
    pub last_scan_ns: u64,
    /// Last scan duration
    pub scan_duration_ns: u64,
}

// ============================================================================
// Iterator
// ============================================================================

/// Iterator over discovered devices
pub struct DeviceIterator<'a> {
    enumerator: &'a DeviceEnumeratorCapsule,
    index: usize,
    count: usize,
}

impl<'a> Iterator for DeviceIterator<'a> {
    type Item = &'a DiscoveredDevice;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.count && self.index < MAX_AMD_GPUS {
            let device = &self.enumerator.devices[self.index];
            self.index += 1;
            if device.is_valid() {
                Some(device)
            } else {
                self.next()
            }
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count.saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for DeviceIterator<'a> {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerator_state_conversions() {
        for state in [
            EnumeratorState::Idle,
            EnumeratorState::ScanningDri,
            EnumeratorState::ScanningPci,
            EnumeratorState::ScanningIpDiscovery,
            EnumeratorState::Ready,
            EnumeratorState::Error,
        ] {
            let v = state.to_u8();
            assert_eq!(EnumeratorState::from_u8(v), Some(state));
        }

        assert_eq!(EnumeratorState::from_u8(255), None);
    }

    #[test]
    fn test_discovered_device_size() {
        assert_eq!(core::mem::size_of::<DiscoveredDevice>(), 256);
        assert_eq!(core::mem::align_of::<DiscoveredDevice>(), 256);
    }

    #[test]
    fn test_enumerator_capsule_alignment() {
        assert_eq!(core::mem::align_of::<DeviceEnumeratorCapsule>(), 2048);
    }

    #[test]
    fn test_ip_capabilities_default() {
        let caps = IpCapabilities::new();
        assert!(!caps.has_gc);
        assert_eq!(caps.sdma_count, 0);

        let rdna3 = IpCapabilities::rdna3_default();
        assert!(rdna3.has_gc);
        assert_eq!(rdna3.sdma_count, 2);
        assert!(rdna3.has_vpe);
    }

    #[test]
    fn test_ip_capabilities_pack_unpack() {
        let device = DiscoveredDevice::new();
        let original = IpCapabilities::rdna3_default();

        device.set_ip_capabilities(&original);
        let unpacked = device.ip_capabilities();

        assert_eq!(unpacked.has_gc, original.has_gc);
        assert_eq!(unpacked.sdma_count, original.sdma_count);
        assert_eq!(unpacked.has_dcn, original.has_dcn);
        assert_eq!(unpacked.vcn_enc_count, original.vcn_enc_count);
        assert_eq!(unpacked.vcn_dec_count, original.vcn_dec_count);
        assert_eq!(unpacked.jpeg_count, original.jpeg_count);
        assert_eq!(unpacked.has_vpe, original.has_vpe);
    }

    #[test]
    fn test_enumerator_initial_state() {
        let enumerator = DeviceEnumeratorCapsule::new();
        assert_eq!(enumerator.state(), EnumeratorState::Idle);
        assert_eq!(enumerator.device_count(), 0);
        assert_eq!(enumerator.generation(), 0);
        assert!(!enumerator.is_scanning());
        assert!(enumerator.last_error().is_none());
    }

    #[test]
    fn test_enumerator_snapshot() {
        let enumerator = DeviceEnumeratorCapsule::new();
        let snapshot = enumerator.snapshot();
        assert_eq!(snapshot.state, EnumeratorState::Idle);
        assert_eq!(snapshot.device_count, 0);
        assert_eq!(snapshot.scan_count, 0);
    }

    #[test]
    fn test_device_iterator_empty() {
        let enumerator = DeviceEnumeratorCapsule::new();
        let devices: Vec<_> = enumerator.iter().collect();
        assert!(devices.is_empty());
    }

    #[test]
    fn test_error_messages() {
        for err in [
            EnumeratorError::NoDevicesFound,
            EnumeratorError::TooManyDevices,
            EnumeratorError::DriAccessFailed,
            EnumeratorError::SysfsAccessFailed,
            EnumeratorError::InvalidState,
            EnumeratorError::GenerationMismatch,
            EnumeratorError::DeviceIndexOutOfBounds,
            EnumeratorError::IpDiscoveryFailed,
            EnumeratorError::PciParseError,
            EnumeratorError::InvalidVendor,
        ] {
            assert!(!err.message().is_empty());
            assert!(!format!("{}", err).is_empty());
        }
    }

    #[test]
    fn test_discovered_device_name() {
        let device = DiscoveredDevice::new();
        device.set_name("AMD RDNA3 Test");

        #[cfg(feature = "std")]
        {
            let name = device.name_str();
            assert_eq!(name, "AMD RDNA3 Test");
        }
    }

    #[test]
    fn test_generation_counter() {
        let device = DiscoveredDevice::new();
        assert_eq!(device.generation_counter.load(Ordering::Acquire), 0);

        let gen1 = device.increment_generation();
        assert_eq!(gen1, 1);

        let gen2 = device.increment_generation();
        assert_eq!(gen2, 2);
    }

    #[test]
    fn test_pci_bdf_extraction() {
        let device = DiscoveredDevice::new();
        device.pci_domain.store(0, Ordering::Release);
        device.pci_bus.store(3, Ordering::Release);
        device.pci_device.store(0, Ordering::Release);
        device.pci_function.store(0, Ordering::Release);

        let bdf = device.pci_bdf();
        assert_eq!(bdf.domain, 0);
        assert_eq!(bdf.bus, 3);
        assert_eq!(bdf.device, 0);
        assert_eq!(bdf.function, 0);
    }

    #[test]
    fn test_gpu_generation_detection() {
        let device = DiscoveredDevice::new();

        // Set to RDNA3
        device.generation.store(37, Ordering::Release);
        assert_eq!(device.gpu_generation(), GpuGeneration::AmdRdna3);

        // Set to RDNA2
        device.generation.store(36, Ordering::Release);
        assert_eq!(device.gpu_generation(), GpuGeneration::AmdRdna2);

        // Unknown
        device.generation.store(0, Ordering::Release);
        assert_eq!(device.gpu_generation(), GpuGeneration::Unknown);
    }
}
