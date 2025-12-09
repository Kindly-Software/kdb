//! GPU Vendor Detection and Generation Identification for KGPU-Driver v2.0
//!
//! This module provides types and functions for:
//! - Detecting GPU vendor from PCI vendor IDs
//! - Identifying GPU generations (architecture families)
//! - Parsing PCI Bus/Device/Function addresses
//!
//! # Chaos Compliance
//!
//! - NO mutex/RwLock - All functions are pure/const where possible
//! - Fixed-size types - All enums use #[repr(u8/u16)]
//! - #[repr(C)] on PciBdf for FFI compatibility
//! - const fn wherever possible for compile-time evaluation
//!
//! # Vendor Support
//!
//! | Vendor | PCI ID | Generations |
//! |--------|--------|-------------|
//! | Intel | 0x8086 | Gen9, Gen11, Gen12, Xe, Xe2 |
//! | AMD | 0x1002 | GCN1-5, RDNA1-4 |
//! | NVIDIA | 0x10DE | Kepler, Maxwell, Pascal, Turing, Ampere, Ada, Blackwell |
//!
//! # Example
//!
//! ```
//! use atomic_capsule::gpu::kgpu_driver::vendor::{GpuVendor, GpuGeneration, PciBdf, detect_generation};
//!
//! // Detect vendor from PCI ID
//! let vendor = GpuVendor::from_pci_vendor_id(0x10DE);
//! assert_eq!(vendor, GpuVendor::Nvidia);
//! assert_eq!(vendor.name(), "NVIDIA");
//!
//! // Parse sysfs path
//! let bdf = PciBdf::from_sysfs_path("0000:01:00.0").unwrap();
//! assert_eq!(bdf.bus, 1);
//! assert_eq!(bdf.device, 0);
//! assert_eq!(bdf.function, 0);
//! ```

#![allow(dead_code)] // Allow during development

use core::fmt;

// ============================================================================
// GpuVendor - Primary GPU manufacturers
// ============================================================================

/// GPU hardware vendor identification.
///
/// Uses PCI vendor IDs as discriminants for efficient comparison.
/// The three major discrete GPU vendors are supported, with Unknown
/// as a fallback for unsupported or unrecognized hardware.
///
/// # Layout
///
/// - Size: 2 bytes (u16)
/// - Alignment: 2 bytes
/// - Discriminant: PCI vendor ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum GpuVendor {
    /// Unknown or unsupported GPU vendor
    Unknown = 0x0000,
    /// Intel Corporation (integrated and discrete GPUs)
    Intel = 0x8086,
    /// Advanced Micro Devices (AMD/ATI)
    Amd = 0x1002,
    /// NVIDIA Corporation
    Nvidia = 0x10DE,
    // Future vendors (reserved ranges):
    // ARM Mali: 0x13B5 (ARM Limited)
    // Qualcomm Adreno: 0x5143 (Qualcomm)
    // Apple: 0x106B (Apple Inc.)
}

impl GpuVendor {
    /// Create GpuVendor from a PCI vendor ID.
    ///
    /// This is a const fn allowing compile-time vendor detection.
    ///
    /// # Arguments
    ///
    /// * `id` - 16-bit PCI vendor ID
    ///
    /// # Returns
    ///
    /// The corresponding GpuVendor, or GpuVendor::Unknown if not recognized.
    ///
    /// # Example
    ///
    /// ```
    /// use atomic_capsule::gpu::kgpu_driver::vendor::GpuVendor;
    ///
    /// assert_eq!(GpuVendor::from_pci_vendor_id(0x8086), GpuVendor::Intel);
    /// assert_eq!(GpuVendor::from_pci_vendor_id(0x1002), GpuVendor::Amd);
    /// assert_eq!(GpuVendor::from_pci_vendor_id(0x10DE), GpuVendor::Nvidia);
    /// assert_eq!(GpuVendor::from_pci_vendor_id(0x1234), GpuVendor::Unknown);
    /// ```
    #[inline]
    pub const fn from_pci_vendor_id(id: u16) -> Self {
        match id {
            0x8086 => Self::Intel,
            0x1002 => Self::Amd,
            0x10DE => Self::Nvidia,
            _ => Self::Unknown,
        }
    }

    /// Get the PCI vendor ID for this vendor.
    ///
    /// # Returns
    ///
    /// The 16-bit PCI vendor ID.
    #[inline]
    pub const fn vendor_id(self) -> u16 {
        self as u16
    }

    /// Get the human-readable vendor name.
    ///
    /// # Returns
    ///
    /// A static string containing the vendor name.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Intel => "Intel",
            Self::Amd => "AMD",
            Self::Nvidia => "NVIDIA",
            Self::Unknown => "Unknown",
        }
    }

    /// Check if this vendor's GPUs support open-source drivers.
    ///
    /// Intel and AMD provide open-source kernel drivers (i915, amdgpu),
    /// while NVIDIA's open-source support is limited.
    #[inline]
    pub const fn has_open_source_driver(self) -> bool {
        matches!(self, Self::Intel | Self::Amd)
    }

    /// Check if this is a known/supported vendor.
    #[inline]
    pub const fn is_known(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

impl Default for GpuVendor {
    fn default() -> Self {
        Self::Unknown
    }
}

impl fmt::Display for GpuVendor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// GpuGeneration - GPU architecture generations
// ============================================================================

/// GPU architecture generation identification.
///
/// Each major GPU vendor releases new architectures every 1-3 years.
/// This enum tracks these generations for feature detection and
/// driver behavior optimization.
///
/// # Value Ranges
///
/// - 0: Unknown
/// - 10-19: Intel generations
/// - 30-39: AMD generations
/// - 50-59: NVIDIA generations
///
/// # Layout
///
/// - Size: 1 byte (u8)
/// - Alignment: 1 byte
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GpuGeneration {
    /// Unknown or unsupported GPU generation
    Unknown = 0,

    // ========================================================================
    // Intel Generations (10-19)
    // ========================================================================

    /// Intel Gen9 - Skylake/Kaby Lake (2015-2017)
    /// EU count: 24-72, supports OpenCL 2.1
    IntelGen9 = 10,

    /// Intel Gen11 - Ice Lake (2019)
    /// EU count: 64, improved media engine
    IntelGen11 = 11,

    /// Intel Gen12 - Tiger Lake/DG1 (2020)
    /// EU count: 96, first discrete GPU (DG1)
    IntelGen12 = 12,

    /// Intel Xe - Arc (2022+)
    /// Ray tracing support, XMX AI acceleration
    IntelXe = 13,

    /// Intel Xe2 - Battlemage (2024+)
    /// Next-gen discrete GPU architecture
    IntelXe2 = 14,

    // ========================================================================
    // AMD Generations (30-39)
    // ========================================================================

    /// AMD GCN1 - Southern Islands (2012)
    /// First GCN architecture (Tahiti, Pitcairn, Cape Verde)
    AmdGcn1 = 30,

    /// AMD GCN2 - Sea Islands (2013)
    /// Improved compute (Hawaii, Bonaire)
    AmdGcn2 = 31,

    /// AMD GCN3 - Volcanic Islands (2014)
    /// Delta color compression (Tonga, Fiji)
    AmdGcn3 = 32,

    /// AMD GCN4 - Polaris (2016)
    /// 14nm FinFET (Polaris 10/11)
    AmdGcn4 = 33,

    /// AMD GCN5 - Vega (2017)
    /// HBM2, NCU improvements (Vega 10/20)
    AmdGcn5 = 34,

    /// AMD RDNA1 - Navi 10/14 (2019)
    /// First RDNA, PCIe 4.0 (RX 5000 series)
    AmdRdna1 = 35,

    /// AMD RDNA2 - Navi 21/22/23 (2020)
    /// Ray tracing, Infinity Cache (RX 6000 series)
    AmdRdna2 = 36,

    /// AMD RDNA3 - Navi 31/32/33 (2022)
    /// Chiplet design, AI accelerators (RX 7000 series)
    AmdRdna3 = 37,

    /// AMD RDNA4 - Navi 4x (2024+)
    /// Next-gen architecture
    AmdRdna4 = 38,

    // ========================================================================
    // NVIDIA Generations (50-59)
    // ========================================================================

    /// NVIDIA Kepler - GK1xx (2012)
    /// First unified memory architecture
    NvidiaKepler = 50,

    /// NVIDIA Maxwell - GM1xx/GM2xx (2014)
    /// Improved power efficiency, MFAA
    NvidiaMaxwell = 51,

    /// NVIDIA Pascal - GP1xx (2016)
    /// 16nm FinFET, NVLink, supports Trojan Kernel
    NvidiaPascal = 52,

    /// NVIDIA Turing - TU1xx (2018)
    /// RT cores, Tensor cores, supports Trojan Kernel
    NvidiaTuring = 53,

    /// NVIDIA Ampere - GA1xx (2020)
    /// 2nd gen RT, 3rd gen Tensor, supports Trojan Kernel
    NvidiaAmpere = 54,

    /// NVIDIA Ada Lovelace - AD1xx (2022)
    /// 3rd gen RT, 4th gen Tensor, supports Trojan Kernel
    NvidiaAdaLovelace = 55,

    /// NVIDIA Blackwell - GB1xx (2024+)
    /// Next-gen architecture, supports Trojan Kernel
    NvidiaBlackwell = 56,
}

impl GpuGeneration {
    /// Get the vendor for this GPU generation.
    ///
    /// Uses the generation value ranges to determine vendor:
    /// - 10-19: Intel
    /// - 30-39: AMD
    /// - 50-59: NVIDIA
    ///
    /// # Returns
    ///
    /// The GpuVendor this generation belongs to.
    #[inline]
    pub const fn vendor(self) -> GpuVendor {
        match self as u8 {
            0 => GpuVendor::Unknown,
            10..=19 => GpuVendor::Intel,
            30..=39 => GpuVendor::Amd,
            50..=59 => GpuVendor::Nvidia,
            _ => GpuVendor::Unknown,
        }
    }

    /// Get the human-readable name for this generation.
    ///
    /// # Returns
    ///
    /// A static string containing the generation name and codename.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",

            // Intel
            Self::IntelGen9 => "Intel Gen9 (Skylake)",
            Self::IntelGen11 => "Intel Gen11 (Ice Lake)",
            Self::IntelGen12 => "Intel Gen12 (Tiger Lake)",
            Self::IntelXe => "Intel Xe (Arc)",
            Self::IntelXe2 => "Intel Xe2 (Battlemage)",

            // AMD
            Self::AmdGcn1 => "AMD GCN1 (Southern Islands)",
            Self::AmdGcn2 => "AMD GCN2 (Sea Islands)",
            Self::AmdGcn3 => "AMD GCN3 (Volcanic Islands)",
            Self::AmdGcn4 => "AMD GCN4 (Polaris)",
            Self::AmdGcn5 => "AMD GCN5 (Vega)",
            Self::AmdRdna1 => "AMD RDNA1 (Navi 10/14)",
            Self::AmdRdna2 => "AMD RDNA2 (Navi 21/22/23)",
            Self::AmdRdna3 => "AMD RDNA3 (Navi 31/32/33)",
            Self::AmdRdna4 => "AMD RDNA4 (Navi 4x)",

            // NVIDIA
            Self::NvidiaKepler => "NVIDIA Kepler (GK1xx)",
            Self::NvidiaMaxwell => "NVIDIA Maxwell (GM1xx/GM2xx)",
            Self::NvidiaPascal => "NVIDIA Pascal (GP1xx)",
            Self::NvidiaTuring => "NVIDIA Turing (TU1xx)",
            Self::NvidiaAmpere => "NVIDIA Ampere (GA1xx)",
            Self::NvidiaAdaLovelace => "NVIDIA Ada Lovelace (AD1xx)",
            Self::NvidiaBlackwell => "NVIDIA Blackwell (GB1xx)",
        }
    }

    /// Returns true if this generation supports the NVIDIA Trojan Kernel approach.
    ///
    /// The Trojan Kernel is a persistent CUDA kernel technique that allows
    /// sovereign control over NVIDIA GPUs by bypassing the GSP firmware.
    /// This requires Pascal (2016) or newer architecture with CUDA support.
    ///
    /// # Returns
    ///
    /// `true` if the generation supports Trojan Kernel, `false` otherwise.
    #[inline]
    pub const fn supports_trojan_kernel(self) -> bool {
        matches!(
            self,
            Self::NvidiaPascal
                | Self::NvidiaTuring
                | Self::NvidiaAmpere
                | Self::NvidiaAdaLovelace
                | Self::NvidiaBlackwell
        )
    }

    /// Returns true if this generation has open-source firmware.
    ///
    /// Intel and AMD provide open-source or loadable firmware (GuC/HuC, PSP),
    /// while NVIDIA's GSP firmware is cryptographically locked.
    ///
    /// # Returns
    ///
    /// `true` if open-source firmware is available, `false` otherwise.
    #[inline]
    pub const fn has_open_firmware(self) -> bool {
        matches!(self.vendor(), GpuVendor::Intel | GpuVendor::Amd)
    }

    /// Returns true if this generation supports hardware ray tracing.
    #[inline]
    pub const fn supports_ray_tracing(self) -> bool {
        matches!(
            self,
            Self::IntelXe
                | Self::IntelXe2
                | Self::AmdRdna2
                | Self::AmdRdna3
                | Self::AmdRdna4
                | Self::NvidiaTuring
                | Self::NvidiaAmpere
                | Self::NvidiaAdaLovelace
                | Self::NvidiaBlackwell
        )
    }

    /// Returns true if this generation supports AI/tensor acceleration.
    #[inline]
    pub const fn supports_ai_acceleration(self) -> bool {
        matches!(
            self,
            Self::IntelXe
                | Self::IntelXe2
                | Self::AmdRdna3
                | Self::AmdRdna4
                | Self::NvidiaTuring
                | Self::NvidiaAmpere
                | Self::NvidiaAdaLovelace
                | Self::NvidiaBlackwell
        )
    }

    /// Get the approximate year this architecture was released.
    #[inline]
    pub const fn release_year(self) -> u16 {
        match self {
            Self::Unknown => 0,

            // Intel
            Self::IntelGen9 => 2015,
            Self::IntelGen11 => 2019,
            Self::IntelGen12 => 2020,
            Self::IntelXe => 2022,
            Self::IntelXe2 => 2024,

            // AMD
            Self::AmdGcn1 => 2012,
            Self::AmdGcn2 => 2013,
            Self::AmdGcn3 => 2014,
            Self::AmdGcn4 => 2016,
            Self::AmdGcn5 => 2017,
            Self::AmdRdna1 => 2019,
            Self::AmdRdna2 => 2020,
            Self::AmdRdna3 => 2022,
            Self::AmdRdna4 => 2024,

            // NVIDIA
            Self::NvidiaKepler => 2012,
            Self::NvidiaMaxwell => 2014,
            Self::NvidiaPascal => 2016,
            Self::NvidiaTuring => 2018,
            Self::NvidiaAmpere => 2020,
            Self::NvidiaAdaLovelace => 2022,
            Self::NvidiaBlackwell => 2024,
        }
    }
}

impl Default for GpuGeneration {
    fn default() -> Self {
        Self::Unknown
    }
}

impl fmt::Display for GpuGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// PciBdf - PCI Bus/Device/Function Address
// ============================================================================

/// PCI Bus/Device/Function address.
///
/// Represents a unique identifier for a PCI device in the system.
/// The format follows the standard "DDDD:BB:DD.F" notation used
/// in sysfs and lspci output.
///
/// # Layout
///
/// - Size: 5 bytes (packed, 6 with alignment)
/// - Alignment: 2 bytes (for domain field)
///
/// # Fields
///
/// - `domain`: PCI domain/segment (usually 0)
/// - `bus`: Bus number (0-255)
/// - `device`: Device number (5 bits, 0-31)
/// - `function`: Function number (3 bits, 0-7)
///
/// # Example
///
/// ```
/// use atomic_capsule::gpu::kgpu_driver::vendor::PciBdf;
///
/// let bdf = PciBdf::new(0, 1, 0, 0);
/// assert_eq!(bdf.bus, 1);
///
/// let bdf = PciBdf::from_sysfs_path("0000:01:00.0").unwrap();
/// assert_eq!(bdf.domain, 0);
/// assert_eq!(bdf.bus, 1);
/// assert_eq!(bdf.device, 0);
/// assert_eq!(bdf.function, 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct PciBdf {
    /// PCI domain (usually 0 on most systems)
    pub domain: u16,
    /// Bus number (0-255)
    pub bus: u8,
    /// Device number (0-31, 5 bits)
    pub device: u8,
    /// Function number (0-7, 3 bits)
    pub function: u8,
}

impl PciBdf {
    /// Create a new PCI BDF address.
    ///
    /// # Arguments
    ///
    /// * `domain` - PCI domain (usually 0)
    /// * `bus` - Bus number (0-255)
    /// * `device` - Device number (0-31)
    /// * `function` - Function number (0-7)
    ///
    /// # Returns
    ///
    /// A new PciBdf instance.
    #[inline]
    pub const fn new(domain: u16, bus: u8, device: u8, function: u8) -> Self {
        Self {
            domain,
            bus,
            device,
            function,
        }
    }

    /// Parse from sysfs path format "DDDD:BB:DD.F".
    ///
    /// Common examples:
    /// - "0000:00:02.0" - Intel integrated GPU
    /// - "0000:01:00.0" - First PCIe slot discrete GPU
    /// - "0000:06:00.0" - Other PCIe slot
    ///
    /// # Arguments
    ///
    /// * `path` - String in "DDDD:BB:DD.F" format
    ///
    /// # Returns
    ///
    /// `Some(PciBdf)` if parsing succeeds, `None` otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use atomic_capsule::gpu::kgpu_driver::vendor::PciBdf;
    ///
    /// let bdf = PciBdf::from_sysfs_path("0000:01:00.0").unwrap();
    /// assert_eq!(bdf.domain, 0);
    /// assert_eq!(bdf.bus, 1);
    /// assert_eq!(bdf.device, 0);
    /// assert_eq!(bdf.function, 0);
    ///
    /// // Invalid formats return None
    /// assert!(PciBdf::from_sysfs_path("invalid").is_none());
    /// assert!(PciBdf::from_sysfs_path("0000:01:00").is_none());
    /// ```
    pub fn from_sysfs_path(path: &str) -> Option<Self> {
        // Parse "DDDD:BB:DD.F" format
        let path = path.trim();

        // Find the colons - need exactly 2
        let mut colon_positions = [0usize; 2];
        let mut colon_count = 0;
        for (i, c) in path.char_indices() {
            if c == ':' {
                if colon_count >= 2 {
                    return None; // Too many colons
                }
                colon_positions[colon_count] = i;
                colon_count += 1;
            }
        }

        if colon_count != 2 {
            return None;
        }

        // Find the dot
        let dot_pos = path.find('.')?;
        if dot_pos <= colon_positions[1] {
            return None;
        }

        // Extract parts
        let domain_str = &path[..colon_positions[0]];
        let bus_str = &path[colon_positions[0] + 1..colon_positions[1]];
        let device_str = &path[colon_positions[1] + 1..dot_pos];
        let function_str = &path[dot_pos + 1..];

        // Parse hexadecimal values
        let domain = u16::from_str_radix(domain_str, 16).ok()?;
        let bus = u8::from_str_radix(bus_str, 16).ok()?;
        let device = u8::from_str_radix(device_str, 16).ok()?;
        let function = u8::from_str_radix(function_str, 16).ok()?;

        // Validate ranges
        if device > 31 || function > 7 {
            return None;
        }

        Some(Self {
            domain,
            bus,
            device,
            function,
        })
    }

    /// Calculate the PCI configuration space address for x86 port I/O.
    ///
    /// This generates the 32-bit value to write to the CONFIG_ADDRESS
    /// port (0xCF8) before reading from CONFIG_DATA (0xCFC).
    ///
    /// # Arguments
    ///
    /// * `offset` - Register offset (must be 4-byte aligned, low 2 bits masked)
    ///
    /// # Returns
    ///
    /// The 32-bit configuration address with enable bit set.
    ///
    /// # Layout of returned value
    ///
    /// ```text
    /// 31      24 23    16 15    11 10      8  7       2 1 0
    /// +---------+--------+--------+---------+---------+---+
    /// | Enable  |  Bus   | Device | Function| Register| 00|
    /// +---------+--------+--------+---------+---------+---+
    ///     1bit    8 bits   5 bits   3 bits    6 bits   2bit
    /// ```
    ///
    /// Note: Domain is ignored for legacy x86 PCI (single domain).
    #[inline]
    pub const fn config_address(self, offset: u8) -> u32 {
        // Set enable bit (bit 31), combine BDF and offset
        0x8000_0000
            | ((self.bus as u32) << 16)
            | (((self.device as u32) & 0x1F) << 11)
            | (((self.function as u32) & 0x07) << 8)
            | ((offset as u32) & 0xFC) // Mask to 4-byte alignment
    }

    /// Get the combined BDF value as a single u16.
    ///
    /// Format: Bus[15:8] | Device[7:3] | Function[2:0]
    ///
    /// This is useful for compact storage and comparison.
    #[inline]
    pub const fn as_bdf_u16(self) -> u16 {
        ((self.bus as u16) << 8) | (((self.device as u16) & 0x1F) << 3) | ((self.function as u16) & 0x07)
    }

    /// Create from combined BDF u16 value.
    #[inline]
    pub const fn from_bdf_u16(domain: u16, bdf: u16) -> Self {
        Self {
            domain,
            bus: (bdf >> 8) as u8,
            device: ((bdf >> 3) & 0x1F) as u8,
            function: (bdf & 0x07) as u8,
        }
    }

    /// Check if this is a multi-function device (function > 0).
    #[inline]
    pub const fn is_multifunction(self) -> bool {
        self.function > 0
    }

    /// Get the base BDF (function 0) for this device.
    #[inline]
    pub const fn base_function(self) -> Self {
        Self {
            domain: self.domain,
            bus: self.bus,
            device: self.device,
            function: 0,
        }
    }
}

impl Default for PciBdf {
    fn default() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

impl fmt::Display for PciBdf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04x}:{:02x}:{:02x}.{:x}",
            self.domain, self.bus, self.device, self.function
        )
    }
}

// ============================================================================
// Detection Functions
// ============================================================================

/// Detect GPU vendor from PCI vendor ID.
///
/// This is a const fn wrapper around GpuVendor::from_pci_vendor_id
/// for convenience.
///
/// # Arguments
///
/// * `vendor_id` - 16-bit PCI vendor ID
///
/// # Returns
///
/// The corresponding GpuVendor.
#[inline]
pub const fn detect_vendor(vendor_id: u16) -> GpuVendor {
    GpuVendor::from_pci_vendor_id(vendor_id)
}

/// Detect GPU generation from vendor and device ID.
///
/// Uses vendor-specific heuristics to determine the architecture
/// generation based on the PCI device ID patterns.
///
/// # Arguments
///
/// * `vendor` - The GPU vendor
/// * `device_id` - 16-bit PCI device ID
///
/// # Returns
///
/// The detected GpuGeneration, or GpuGeneration::Unknown if not recognized.
///
/// # Example
///
/// ```
/// use atomic_capsule::gpu::kgpu_driver::vendor::{GpuVendor, GpuGeneration, detect_generation};
///
/// // RTX 4090 (Ada Lovelace)
/// let gen = detect_generation(GpuVendor::Nvidia, 0x2684);
/// assert_eq!(gen, GpuGeneration::NvidiaAdaLovelace);
///
/// // Unknown device
/// let gen = detect_generation(GpuVendor::Unknown, 0x1234);
/// assert_eq!(gen, GpuGeneration::Unknown);
/// ```
pub fn detect_generation(vendor: GpuVendor, device_id: u16) -> GpuGeneration {
    match vendor {
        GpuVendor::Intel => detect_intel_generation(device_id),
        GpuVendor::Amd => detect_amd_generation(device_id),
        GpuVendor::Nvidia => detect_nvidia_generation(device_id),
        GpuVendor::Unknown => GpuGeneration::Unknown,
    }
}

/// Detect Intel GPU generation from device ID.
///
/// Intel uses complex device ID schemes. This uses high byte patterns
/// as a simplified heuristic.
///
/// # Device ID Ranges (Reference)
///
/// | Architecture | Device ID Range | Example |
/// |--------------|-----------------|---------|
/// | Gen9 (Skylake) | 0x1900-0x5A00 | 0x1912 (HD 530) |
/// | Gen11 (Ice Lake) | 0x8A00-0x8AFF | 0x8A52 (Iris Plus) |
/// | Gen12 (Tiger Lake) | 0x9A00-0x9AFF | 0x9A49 (Xe) |
/// | Xe (Arc Alchemist) | 0x5600-0x56FF | 0x56A0 (A770) |
/// | Xe2 (Meteor Lake) | 0x7D40-0x7D67 | 0x7D55 (Arc Graphics) |
/// | Xe2 (Battlemage) | 0x6400-0x64FF | TBD |
fn detect_intel_generation(device_id: u16) -> GpuGeneration {
    // Intel device ID patterns (simplified - real detection is more complex)
    // See: https://pci-ids.ucw.cz/read/PC/8086
    match device_id >> 8 {
        // Skylake/Kaby Lake (Gen9)
        0x19 | 0x59 | 0x5A | 0x3E => GpuGeneration::IntelGen9,

        // Ice Lake (Gen11)
        0x8A => GpuGeneration::IntelGen11,

        // Tiger Lake/DG1 (Gen12)
        0x9A | 0x4C | 0x46 => GpuGeneration::IntelGen12,

        // Arc (Xe) - Alchemist discrete GPUs
        0x56 => GpuGeneration::IntelXe,

        // Meteor Lake (Xe2-LPG) - 0x7D40-0x7D67
        // Xe2 integrated graphics in Meteor Lake CPUs
        0x7D => GpuGeneration::IntelXe2,

        // Battlemage (Xe2) - Future discrete GPUs
        0x64 => GpuGeneration::IntelXe2,

        _ => GpuGeneration::Unknown,
    }
}

/// Detect AMD GPU generation from device ID.
///
/// AMD device IDs follow loose patterns. This uses high byte
/// as a simplified heuristic.
fn detect_amd_generation(device_id: u16) -> GpuGeneration {
    // AMD device ID patterns (simplified)
    // See: https://pci-ids.ucw.cz/read/PC/1002
    let high_byte = device_id >> 8;
    let low_byte = device_id & 0xFF;

    match high_byte {
        // RDNA4 - placeholder (check first for newest)
        0x76 | 0x77 => GpuGeneration::AmdRdna4,

        // Navi 31/32/33 (RDNA3) - RX 7000 series
        0x74 | 0x75 => GpuGeneration::AmdRdna3,

        // Navi (RDNA1/RDNA2) - RX 5000/6000 series
        0x73 => {
            if device_id < 0x7340 {
                GpuGeneration::AmdRdna1
            } else {
                GpuGeneration::AmdRdna2
            }
        }

        // Vega (GCN5) - 0x66xx, 0x68xx, 0x69xx
        0x66 | 0x69 => GpuGeneration::AmdGcn5,

        // 0x68xx - mostly Vega, some older GCN
        0x68 => {
            if low_byte >= 0x60 {
                GpuGeneration::AmdGcn5 // Vega
            } else {
                GpuGeneration::AmdGcn1 // Southern Islands
            }
        }

        // Polaris (GCN4) - 0x67xx RX 400/500 series
        0x67 => GpuGeneration::AmdGcn4,

        // Sea Islands (GCN2)
        0x6E | 0x6F => GpuGeneration::AmdGcn2,

        _ => GpuGeneration::Unknown,
    }
}

/// Detect NVIDIA GPU generation from device ID.
///
/// NVIDIA device IDs use consistent prefixes per architecture.
fn detect_nvidia_generation(device_id: u16) -> GpuGeneration {
    // NVIDIA device ID patterns
    // See: https://pci-ids.ucw.cz/read/PC/10de
    match device_id >> 8 {
        // Kepler (GK1xx)
        0x0F | 0x10 | 0x11 | 0x12 => GpuGeneration::NvidiaKepler,

        // Maxwell (GM1xx/GM2xx)
        0x13 | 0x14 | 0x17 => GpuGeneration::NvidiaMaxwell,

        // Pascal (GP1xx) - GTX 10 series
        0x15 | 0x1B | 0x1C | 0x1D => GpuGeneration::NvidiaPascal,

        // Turing (TU1xx) - RTX 20 series, GTX 16 series
        0x1E | 0x1F | 0x21 => GpuGeneration::NvidiaTuring,

        // Ampere (GA1xx) - RTX 30 series
        0x20 | 0x22 | 0x23 | 0x24 | 0x25 => GpuGeneration::NvidiaAmpere,

        // Ada Lovelace (AD1xx) - RTX 40 series
        0x26 | 0x27 | 0x28 => GpuGeneration::NvidiaAdaLovelace,

        // Blackwell (GB1xx) - RTX 50 series (placeholder)
        0x29 | 0x2A | 0x2B => GpuGeneration::NvidiaBlackwell,

        _ => GpuGeneration::Unknown,
    }
}

// ============================================================================
// Convenience Types
// ============================================================================

/// Combined vendor and device information.
///
/// Useful for passing around complete PCI identification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PciDeviceId {
    /// PCI vendor ID
    pub vendor_id: u16,
    /// PCI device ID
    pub device_id: u16,
    /// Subsystem vendor ID (optional)
    pub subsys_vendor_id: u16,
    /// Subsystem device ID (optional)
    pub subsys_device_id: u16,
}

impl PciDeviceId {
    /// Create a new PCI device ID.
    #[inline]
    pub const fn new(vendor_id: u16, device_id: u16) -> Self {
        Self {
            vendor_id,
            device_id,
            subsys_vendor_id: 0,
            subsys_device_id: 0,
        }
    }

    /// Create with subsystem IDs.
    #[inline]
    pub const fn with_subsystem(
        vendor_id: u16,
        device_id: u16,
        subsys_vendor_id: u16,
        subsys_device_id: u16,
    ) -> Self {
        Self {
            vendor_id,
            device_id,
            subsys_vendor_id,
            subsys_device_id,
        }
    }

    /// Get the vendor.
    #[inline]
    pub const fn vendor(self) -> GpuVendor {
        GpuVendor::from_pci_vendor_id(self.vendor_id)
    }

    /// Detect the generation.
    #[inline]
    pub fn generation(self) -> GpuGeneration {
        detect_generation(self.vendor(), self.device_id)
    }
}

impl Default for PciDeviceId {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl fmt::Display for PciDeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vendor_id, self.device_id)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // GpuVendor Tests
    // ========================================================================

    #[test]
    fn test_vendor_from_pci_id() {
        assert_eq!(GpuVendor::from_pci_vendor_id(0x8086), GpuVendor::Intel);
        assert_eq!(GpuVendor::from_pci_vendor_id(0x1002), GpuVendor::Amd);
        assert_eq!(GpuVendor::from_pci_vendor_id(0x10DE), GpuVendor::Nvidia);
        assert_eq!(GpuVendor::from_pci_vendor_id(0x0000), GpuVendor::Unknown);
        assert_eq!(GpuVendor::from_pci_vendor_id(0x1234), GpuVendor::Unknown);
        assert_eq!(GpuVendor::from_pci_vendor_id(0xFFFF), GpuVendor::Unknown);
    }

    #[test]
    fn test_vendor_id_roundtrip() {
        assert_eq!(GpuVendor::Intel.vendor_id(), 0x8086);
        assert_eq!(GpuVendor::Amd.vendor_id(), 0x1002);
        assert_eq!(GpuVendor::Nvidia.vendor_id(), 0x10DE);
        assert_eq!(GpuVendor::Unknown.vendor_id(), 0x0000);

        // Roundtrip
        for vendor in [GpuVendor::Intel, GpuVendor::Amd, GpuVendor::Nvidia] {
            assert_eq!(GpuVendor::from_pci_vendor_id(vendor.vendor_id()), vendor);
        }
    }

    #[test]
    fn test_vendor_name() {
        assert_eq!(GpuVendor::Intel.name(), "Intel");
        assert_eq!(GpuVendor::Amd.name(), "AMD");
        assert_eq!(GpuVendor::Nvidia.name(), "NVIDIA");
        assert_eq!(GpuVendor::Unknown.name(), "Unknown");
    }

    #[test]
    fn test_vendor_open_source_driver() {
        assert!(GpuVendor::Intel.has_open_source_driver());
        assert!(GpuVendor::Amd.has_open_source_driver());
        assert!(!GpuVendor::Nvidia.has_open_source_driver());
        assert!(!GpuVendor::Unknown.has_open_source_driver());
    }

    #[test]
    fn test_vendor_is_known() {
        assert!(GpuVendor::Intel.is_known());
        assert!(GpuVendor::Amd.is_known());
        assert!(GpuVendor::Nvidia.is_known());
        assert!(!GpuVendor::Unknown.is_known());
    }

    #[test]
    fn test_vendor_default() {
        assert_eq!(GpuVendor::default(), GpuVendor::Unknown);
    }

    #[test]
    fn test_vendor_display() {
        assert_eq!(format!("{}", GpuVendor::Intel), "Intel");
        assert_eq!(format!("{}", GpuVendor::Nvidia), "NVIDIA");
    }

    // ========================================================================
    // GpuGeneration Tests
    // ========================================================================

    #[test]
    fn test_generation_vendor() {
        // Intel range (10-19)
        assert_eq!(GpuGeneration::IntelGen9.vendor(), GpuVendor::Intel);
        assert_eq!(GpuGeneration::IntelGen11.vendor(), GpuVendor::Intel);
        assert_eq!(GpuGeneration::IntelXe.vendor(), GpuVendor::Intel);
        assert_eq!(GpuGeneration::IntelXe2.vendor(), GpuVendor::Intel);

        // AMD range (30-39)
        assert_eq!(GpuGeneration::AmdGcn1.vendor(), GpuVendor::Amd);
        assert_eq!(GpuGeneration::AmdRdna3.vendor(), GpuVendor::Amd);
        assert_eq!(GpuGeneration::AmdRdna4.vendor(), GpuVendor::Amd);

        // NVIDIA range (50-59)
        assert_eq!(GpuGeneration::NvidiaKepler.vendor(), GpuVendor::Nvidia);
        assert_eq!(GpuGeneration::NvidiaAdaLovelace.vendor(), GpuVendor::Nvidia);
        assert_eq!(GpuGeneration::NvidiaBlackwell.vendor(), GpuVendor::Nvidia);

        // Unknown
        assert_eq!(GpuGeneration::Unknown.vendor(), GpuVendor::Unknown);
    }

    #[test]
    fn test_generation_trojan_kernel_support() {
        // NVIDIA Pascal+ supports Trojan Kernel
        assert!(GpuGeneration::NvidiaPascal.supports_trojan_kernel());
        assert!(GpuGeneration::NvidiaTuring.supports_trojan_kernel());
        assert!(GpuGeneration::NvidiaAmpere.supports_trojan_kernel());
        assert!(GpuGeneration::NvidiaAdaLovelace.supports_trojan_kernel());
        assert!(GpuGeneration::NvidiaBlackwell.supports_trojan_kernel());

        // Pre-Pascal does not
        assert!(!GpuGeneration::NvidiaMaxwell.supports_trojan_kernel());
        assert!(!GpuGeneration::NvidiaKepler.supports_trojan_kernel());

        // Non-NVIDIA never support it
        assert!(!GpuGeneration::IntelXe.supports_trojan_kernel());
        assert!(!GpuGeneration::AmdRdna3.supports_trojan_kernel());
        assert!(!GpuGeneration::Unknown.supports_trojan_kernel());
    }

    #[test]
    fn test_generation_open_firmware() {
        // Intel and AMD have open firmware
        assert!(GpuGeneration::IntelGen9.has_open_firmware());
        assert!(GpuGeneration::IntelXe.has_open_firmware());
        assert!(GpuGeneration::AmdGcn5.has_open_firmware());
        assert!(GpuGeneration::AmdRdna3.has_open_firmware());

        // NVIDIA does not
        assert!(!GpuGeneration::NvidiaPascal.has_open_firmware());
        assert!(!GpuGeneration::NvidiaAdaLovelace.has_open_firmware());

        // Unknown returns false (vendor is Unknown)
        assert!(!GpuGeneration::Unknown.has_open_firmware());
    }

    #[test]
    fn test_generation_ray_tracing() {
        // RT support starts with certain generations
        assert!(GpuGeneration::IntelXe.supports_ray_tracing());
        assert!(GpuGeneration::IntelXe2.supports_ray_tracing());
        assert!(GpuGeneration::AmdRdna2.supports_ray_tracing());
        assert!(GpuGeneration::AmdRdna3.supports_ray_tracing());
        assert!(GpuGeneration::NvidiaTuring.supports_ray_tracing());
        assert!(GpuGeneration::NvidiaAmpere.supports_ray_tracing());
        assert!(GpuGeneration::NvidiaAdaLovelace.supports_ray_tracing());

        // Older generations don't have RT
        assert!(!GpuGeneration::IntelGen9.supports_ray_tracing());
        assert!(!GpuGeneration::AmdRdna1.supports_ray_tracing());
        assert!(!GpuGeneration::NvidiaPascal.supports_ray_tracing());
    }

    #[test]
    fn test_generation_ai_acceleration() {
        // AI/Tensor cores
        assert!(GpuGeneration::IntelXe.supports_ai_acceleration());
        assert!(GpuGeneration::AmdRdna3.supports_ai_acceleration());
        assert!(GpuGeneration::NvidiaTuring.supports_ai_acceleration());
        assert!(GpuGeneration::NvidiaAmpere.supports_ai_acceleration());

        // No dedicated AI hardware
        assert!(!GpuGeneration::IntelGen9.supports_ai_acceleration());
        assert!(!GpuGeneration::AmdRdna1.supports_ai_acceleration());
        assert!(!GpuGeneration::NvidiaPascal.supports_ai_acceleration());
    }

    #[test]
    fn test_generation_release_year() {
        assert_eq!(GpuGeneration::IntelGen9.release_year(), 2015);
        assert_eq!(GpuGeneration::AmdRdna2.release_year(), 2020);
        assert_eq!(GpuGeneration::NvidiaAdaLovelace.release_year(), 2022);
        assert_eq!(GpuGeneration::NvidiaBlackwell.release_year(), 2024);
        assert_eq!(GpuGeneration::Unknown.release_year(), 0);
    }

    #[test]
    fn test_generation_name() {
        assert!(GpuGeneration::IntelXe.name().contains("Arc"));
        assert!(GpuGeneration::AmdRdna3.name().contains("Navi 31"));
        assert!(GpuGeneration::NvidiaAdaLovelace.name().contains("AD1xx"));
    }

    #[test]
    fn test_generation_default() {
        assert_eq!(GpuGeneration::default(), GpuGeneration::Unknown);
    }

    // ========================================================================
    // PciBdf Tests
    // ========================================================================

    #[test]
    fn test_pci_bdf_new() {
        let bdf = PciBdf::new(0, 1, 2, 3);
        assert_eq!(bdf.domain, 0);
        assert_eq!(bdf.bus, 1);
        assert_eq!(bdf.device, 2);
        assert_eq!(bdf.function, 3);
    }

    #[test]
    fn test_pci_bdf_from_sysfs_path() {
        // Standard GPU path
        let bdf = PciBdf::from_sysfs_path("0000:01:00.0").unwrap();
        assert_eq!(bdf.domain, 0);
        assert_eq!(bdf.bus, 1);
        assert_eq!(bdf.device, 0);
        assert_eq!(bdf.function, 0);

        // Intel integrated
        let bdf = PciBdf::from_sysfs_path("0000:00:02.0").unwrap();
        assert_eq!(bdf.domain, 0);
        assert_eq!(bdf.bus, 0);
        assert_eq!(bdf.device, 2);
        assert_eq!(bdf.function, 0);

        // Higher bus/device/function
        let bdf = PciBdf::from_sysfs_path("0001:ff:1f.7").unwrap();
        assert_eq!(bdf.domain, 1);
        assert_eq!(bdf.bus, 255);
        assert_eq!(bdf.device, 31);
        assert_eq!(bdf.function, 7);

        // With whitespace
        let bdf = PciBdf::from_sysfs_path("  0000:01:00.0  ").unwrap();
        assert_eq!(bdf.bus, 1);
    }

    #[test]
    fn test_pci_bdf_from_sysfs_path_invalid() {
        // Missing components
        assert!(PciBdf::from_sysfs_path("").is_none());
        assert!(PciBdf::from_sysfs_path("invalid").is_none());
        assert!(PciBdf::from_sysfs_path("0000:01").is_none());
        assert!(PciBdf::from_sysfs_path("0000:01:00").is_none()); // Missing function

        // Wrong separators
        assert!(PciBdf::from_sysfs_path("0000.01.00.0").is_none());
        assert!(PciBdf::from_sysfs_path("0000:01:00:0").is_none());

        // Too many colons
        assert!(PciBdf::from_sysfs_path("0000:01:00:00.0").is_none());

        // Out of range values
        assert!(PciBdf::from_sysfs_path("0000:01:20.0").is_some()); // device 32 = 0x20, valid
        assert!(PciBdf::from_sysfs_path("0000:01:20.8").is_none()); // function 8 invalid
    }

    #[test]
    fn test_pci_bdf_config_address() {
        let bdf = PciBdf::new(0, 1, 0, 0);

        // Offset 0 (vendor ID)
        let addr = bdf.config_address(0x00);
        assert_eq!(addr & 0x8000_0000, 0x8000_0000); // Enable bit
        assert_eq!((addr >> 16) & 0xFF, 1); // Bus 1
        assert_eq!((addr >> 11) & 0x1F, 0); // Device 0
        assert_eq!((addr >> 8) & 0x07, 0); // Function 0
        assert_eq!(addr & 0xFC, 0); // Offset 0

        // Offset 0x10 (BAR0)
        let addr = bdf.config_address(0x10);
        assert_eq!(addr & 0xFC, 0x10);

        // Test device and function bits
        let bdf = PciBdf::new(0, 0, 31, 7);
        let addr = bdf.config_address(0x00);
        assert_eq!((addr >> 11) & 0x1F, 31); // Device 31
        assert_eq!((addr >> 8) & 0x07, 7); // Function 7
    }

    #[test]
    fn test_pci_bdf_as_bdf_u16() {
        let bdf = PciBdf::new(0, 1, 2, 3);
        let packed = bdf.as_bdf_u16();

        assert_eq!(packed >> 8, 1); // Bus
        assert_eq!((packed >> 3) & 0x1F, 2); // Device
        assert_eq!(packed & 0x07, 3); // Function

        // Roundtrip
        let recovered = PciBdf::from_bdf_u16(0, packed);
        assert_eq!(recovered.bus, bdf.bus);
        assert_eq!(recovered.device, bdf.device);
        assert_eq!(recovered.function, bdf.function);
    }

    #[test]
    fn test_pci_bdf_multifunction() {
        assert!(!PciBdf::new(0, 0, 0, 0).is_multifunction());
        assert!(PciBdf::new(0, 0, 0, 1).is_multifunction());
        assert!(PciBdf::new(0, 0, 0, 7).is_multifunction());
    }

    #[test]
    fn test_pci_bdf_base_function() {
        let bdf = PciBdf::new(0, 1, 2, 5);
        let base = bdf.base_function();
        assert_eq!(base.domain, 0);
        assert_eq!(base.bus, 1);
        assert_eq!(base.device, 2);
        assert_eq!(base.function, 0);
    }

    #[test]
    fn test_pci_bdf_display() {
        let bdf = PciBdf::new(0, 1, 0, 0);
        assert_eq!(format!("{}", bdf), "0000:01:00.0");

        let bdf = PciBdf::new(1, 255, 31, 7);
        assert_eq!(format!("{}", bdf), "0001:ff:1f.7");
    }

    #[test]
    fn test_pci_bdf_default() {
        let bdf = PciBdf::default();
        assert_eq!(bdf.domain, 0);
        assert_eq!(bdf.bus, 0);
        assert_eq!(bdf.device, 0);
        assert_eq!(bdf.function, 0);
    }

    // ========================================================================
    // Detection Function Tests
    // ========================================================================

    #[test]
    fn test_detect_vendor() {
        assert_eq!(detect_vendor(0x8086), GpuVendor::Intel);
        assert_eq!(detect_vendor(0x1002), GpuVendor::Amd);
        assert_eq!(detect_vendor(0x10DE), GpuVendor::Nvidia);
        assert_eq!(detect_vendor(0x0000), GpuVendor::Unknown);
    }

    #[test]
    fn test_detect_intel_generation() {
        // Gen9 (Skylake)
        assert_eq!(
            detect_generation(GpuVendor::Intel, 0x1912),
            GpuGeneration::IntelGen9
        );
        assert_eq!(
            detect_generation(GpuVendor::Intel, 0x5912),
            GpuGeneration::IntelGen9
        );

        // Gen11 (Ice Lake)
        assert_eq!(
            detect_generation(GpuVendor::Intel, 0x8A52),
            GpuGeneration::IntelGen11
        );

        // Gen12 (Tiger Lake)
        assert_eq!(
            detect_generation(GpuVendor::Intel, 0x9A49),
            GpuGeneration::IntelGen12
        );

        // Arc (Xe)
        assert_eq!(
            detect_generation(GpuVendor::Intel, 0x56A0),
            GpuGeneration::IntelXe
        );
    }

    #[test]
    fn test_detect_meteor_lake_generation() {
        // Meteor Lake-P device IDs (0x7D40-0x7D67)
        assert_eq!(
            detect_generation(GpuVendor::Intel, 0x7D40),
            GpuGeneration::IntelXe2
        );
        assert_eq!(
            detect_generation(GpuVendor::Intel, 0x7D45),
            GpuGeneration::IntelXe2
        );
        assert_eq!(
            detect_generation(GpuVendor::Intel, 0x7D55),
            GpuGeneration::IntelXe2
        );
        assert_eq!(
            detect_generation(GpuVendor::Intel, 0x7D67),
            GpuGeneration::IntelXe2
        );

        // Xe2 Meteor Lake should support ray tracing and AI
        let gen = detect_generation(GpuVendor::Intel, 0x7D55);
        assert!(gen.supports_ray_tracing());
        assert!(gen.supports_ai_acceleration());
        assert!(gen.has_open_firmware());
        assert!(!gen.supports_trojan_kernel()); // Intel doesn't need Trojan
    }

    #[test]
    fn test_detect_amd_generation() {
        // GCN4 (Polaris)
        assert_eq!(
            detect_generation(GpuVendor::Amd, 0x67DF),
            GpuGeneration::AmdGcn4
        );

        // GCN5 (Vega)
        assert_eq!(
            detect_generation(GpuVendor::Amd, 0x6867),
            GpuGeneration::AmdGcn5
        );

        // RDNA2 (Navi 2x)
        assert_eq!(
            detect_generation(GpuVendor::Amd, 0x73BF),
            GpuGeneration::AmdRdna2
        );

        // RDNA3 (Navi 3x)
        assert_eq!(
            detect_generation(GpuVendor::Amd, 0x7400),
            GpuGeneration::AmdRdna3
        );
    }

    #[test]
    fn test_detect_nvidia_generation() {
        // Kepler
        assert_eq!(
            detect_generation(GpuVendor::Nvidia, 0x1180),
            GpuGeneration::NvidiaKepler
        );

        // Maxwell
        assert_eq!(
            detect_generation(GpuVendor::Nvidia, 0x13C0),
            GpuGeneration::NvidiaMaxwell
        );

        // Pascal (GTX 1080)
        assert_eq!(
            detect_generation(GpuVendor::Nvidia, 0x1B80),
            GpuGeneration::NvidiaPascal
        );

        // Turing (RTX 2080)
        assert_eq!(
            detect_generation(GpuVendor::Nvidia, 0x1E82),
            GpuGeneration::NvidiaTuring
        );

        // Ampere (RTX 3090)
        assert_eq!(
            detect_generation(GpuVendor::Nvidia, 0x2204),
            GpuGeneration::NvidiaAmpere
        );

        // Ada Lovelace (RTX 4090)
        assert_eq!(
            detect_generation(GpuVendor::Nvidia, 0x2684),
            GpuGeneration::NvidiaAdaLovelace
        );
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(
            detect_generation(GpuVendor::Unknown, 0x1234),
            GpuGeneration::Unknown
        );
        assert_eq!(
            detect_generation(GpuVendor::Intel, 0x0000),
            GpuGeneration::Unknown
        );
    }

    // ========================================================================
    // PciDeviceId Tests
    // ========================================================================

    #[test]
    fn test_pci_device_id_new() {
        let id = PciDeviceId::new(0x10DE, 0x2684);
        assert_eq!(id.vendor_id, 0x10DE);
        assert_eq!(id.device_id, 0x2684);
        assert_eq!(id.subsys_vendor_id, 0);
        assert_eq!(id.subsys_device_id, 0);
    }

    #[test]
    fn test_pci_device_id_with_subsystem() {
        let id = PciDeviceId::with_subsystem(0x10DE, 0x2684, 0x1043, 0x8800);
        assert_eq!(id.subsys_vendor_id, 0x1043);
        assert_eq!(id.subsys_device_id, 0x8800);
    }

    #[test]
    fn test_pci_device_id_vendor() {
        let id = PciDeviceId::new(0x10DE, 0x2684);
        assert_eq!(id.vendor(), GpuVendor::Nvidia);

        let id = PciDeviceId::new(0x8086, 0x9A49);
        assert_eq!(id.vendor(), GpuVendor::Intel);
    }

    #[test]
    fn test_pci_device_id_generation() {
        let id = PciDeviceId::new(0x10DE, 0x2684);
        assert_eq!(id.generation(), GpuGeneration::NvidiaAdaLovelace);

        let id = PciDeviceId::new(0x1002, 0x73BF);
        assert_eq!(id.generation(), GpuGeneration::AmdRdna2);
    }

    #[test]
    fn test_pci_device_id_display() {
        let id = PciDeviceId::new(0x10DE, 0x2684);
        assert_eq!(format!("{}", id), "10de:2684");
    }

    // ========================================================================
    // Size and Alignment Tests (Chaos compliance)
    // ========================================================================

    #[test]
    fn test_type_sizes() {
        assert_eq!(core::mem::size_of::<GpuVendor>(), 2);
        assert_eq!(core::mem::size_of::<GpuGeneration>(), 1);
        assert_eq!(core::mem::size_of::<PciBdf>(), 5); // packed
        assert_eq!(core::mem::size_of::<PciDeviceId>(), 8);
    }

    #[test]
    fn test_type_alignment() {
        assert_eq!(core::mem::align_of::<GpuVendor>(), 2);
        assert_eq!(core::mem::align_of::<GpuGeneration>(), 1);
        assert_eq!(core::mem::align_of::<PciBdf>(), 2);
        assert_eq!(core::mem::align_of::<PciDeviceId>(), 2);
    }
}
