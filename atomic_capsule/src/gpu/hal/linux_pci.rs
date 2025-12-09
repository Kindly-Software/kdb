//! Linux PCIe Device Access for Intel GPUs
//!
//! Implements PCIe BAR mapping and config space access via sysfs/mmap.
//! Connects to MmioRegionCapsule for lockfree MMIO operations.
//!
//! # Design
//!
//! **Tier**: T1 Atomic (lockfree coordination) + T8 Network (kernel IPC)
//! **Portability**: Linux-only (feature-gated: `linux-gpu`)
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    IntelGpuDevice                                │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │  ┌──────────────────────────────────────────────────────────┐  │
//! │  │              PCIe Configuration                           │  │
//! │  │  /sys/bus/pci/devices/DDDD:BB:DD.F/config                │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! │                           │                                      │
//! │                           ▼                                      │
//! │  ┌──────────────────────────────────────────────────────────┐  │
//! │  │              BAR Mapping                                  │  │
//! │  │  /sys/bus/pci/devices/DDDD:BB:DD.F/resource0             │  │
//! │  │  mmap() → MMIO virtual address                           │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! │                           │                                      │
//! │                           ▼                                      │
//! │  ┌──────────────────────────────────────────────────────────┐  │
//! │  │           MmioRegionCapsule                               │  │
//! │  │  volatile reads/writes with generation counters          │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_SYSFS_AVAILABLE`: /sys filesystem mounted (standard Linux)
//! - `#ASSUME_PCI_DEVICE_EXISTS`: Intel GPU present at specified BDF
//! - `#ASSUME_BAR_VALID`: BAR region valid for mmap after PCI enumeration
//! - `#ASSUME_MMAP_SUCCESS`: mmap succeeds with appropriate permissions
//! - `#ASSUME_FD_VALID`: File descriptor valid after successful open
//!
//! # Examples
//!
//! ```ignore
//! use atomic_capsule::gpu::hal::linux_pci::IntelGpuDevice;
//!
//! // Open Intel GPU at 0000:00:02.0 (typical integrated GPU)
//! let device = IntelGpuDevice::open(0, 0, 2, 0)?;
//!
//! // Read MMIO register (offset 0x44000 = FORCEWAKE)
//! let value = device.read_mmio(0x44000)?;
//!
//! // Write MMIO register
//! device.write_mmio(0x44000, 0x1)?;
//!
//! // Get device info
//! println!("Device ID: 0x{:04x}", device.device_id());
//! println!("MMIO size: {} bytes", device.mmio_size());
//! ```

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicPtr, Ordering};
use core::ptr;

use super::linux_hal::{
    LinuxHalError, LinuxHalResult, LinuxHalState,
    LinuxPciAccess, GemHandle, IntelGpuGen,
};

// ============================================================================
// PCI BDF (Bus/Device/Function) Address
// ============================================================================

/// PCI Bus/Device/Function address
///
/// Standard PCI addressing: Domain:Bus:Device.Function (DDDD:BB:DD.F)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciBdf {
    /// PCI domain (usually 0000)
    pub domain: u16,
    /// Bus number (0-255)
    pub bus: u8,
    /// Device number (0-31)
    pub device: u8,
    /// Function number (0-7)
    pub function: u8,
}

impl PciBdf {
    /// Create new PCI BDF address
    #[inline]
    pub const fn new(domain: u16, bus: u8, device: u8, function: u8) -> Self {
        Self { domain, bus, device, function }
    }

    /// Get sysfs device path
    ///
    /// Returns path like "/sys/bus/pci/devices/0000:00:02.0"
    #[cfg(feature = "std")]
    pub fn sysfs_path(&self) -> String {
        format!(
            "/sys/bus/pci/devices/{:04x}:{:02x}:{:02x}.{}",
            self.domain, self.bus, self.device, self.function
        )
    }

    /// Get DRM card path (typically /dev/dri/card0)
    #[cfg(feature = "std")]
    pub fn drm_path(&self) -> String {
        // For integrated Intel GPU at 00:02.0, this is usually card0
        // For discrete, need to enumerate /dev/dri/by-path/
        format!("/dev/dri/card0")
    }

    /// Get render node path (typically /dev/dri/renderD128)
    #[cfg(feature = "std")]
    pub fn render_path(&self) -> String {
        format!("/dev/dri/renderD128")
    }
}

impl core::fmt::Display for PciBdf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:04x}:{:02x}:{:02x}.{}",
            self.domain, self.bus, self.device, self.function
        )
    }
}

// ============================================================================
// Intel PCI Device IDs
// ============================================================================

/// Intel GPU PCI device ID ranges
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntelPciId {
    /// Vendor ID (always 0x8086 for Intel)
    pub vendor: u16,
    /// Device ID
    pub device: u16,
    /// Revision ID
    pub revision: u8,
}

impl IntelPciId {
    /// Intel vendor ID
    pub const INTEL_VENDOR_ID: u16 = 0x8086;

    /// Create new Intel PCI ID
    #[inline]
    pub const fn new(device: u16, revision: u8) -> Self {
        Self {
            vendor: Self::INTEL_VENDOR_ID,
            device,
            revision,
        }
    }

    /// Determine GPU generation from device ID
    pub fn detect_generation(&self) -> IntelGpuGen {
        // Device ID ranges for Intel GPUs (simplified)
        // See: https://dgpu-docs.intel.com/devices/hardware-table.html
        match self.device {
            // Gen7: Ivy Bridge, Haswell
            0x0152..=0x016A => IntelGpuGen::Gen7,
            0x0402..=0x041E => IntelGpuGen::Gen7,

            // Gen8: Broadwell
            0x1602..=0x163D => IntelGpuGen::Gen8,

            // Gen9: Skylake, Kaby Lake, Coffee Lake
            0x1902..=0x193D => IntelGpuGen::Gen9,
            0x5902..=0x593D => IntelGpuGen::Gen9,
            0x3E90..=0x3EA9 => IntelGpuGen::Gen9,

            // Gen11: Ice Lake
            0x8A50..=0x8A5D => IntelGpuGen::Gen11,

            // Gen12: Tiger Lake, Rocket Lake, Alder Lake
            0x9A40..=0x9A7F => IntelGpuGen::Gen12,
            0x4C80..=0x4C9F => IntelGpuGen::Gen12,
            0x4680..=0x46CF => IntelGpuGen::Gen12,

            // Gen12.5: DG1, DG2
            0x4905..=0x4908 => IntelGpuGen::Gen12p5,
            0x56A0..=0x56C1 => IntelGpuGen::Gen12p5,

            // Xe HPG: Arc Alchemist
            0x5690..=0x56FF => IntelGpuGen::XeHpg,

            // Xe LPG: Meteor Lake
            0x7D40..=0x7D67 => IntelGpuGen::XeLpg,

            _ => IntelGpuGen::Unknown,
        }
    }
}

// ============================================================================
// BAR Mapping State
// ============================================================================

/// BAR mapping information
#[derive(Debug)]
pub struct BarMapping {
    /// Virtual address of mapped region
    pub address: *mut u8,
    /// Size of mapped region in bytes
    pub size: usize,
    /// BAR index (0-5)
    pub bar_index: u8,
}

impl BarMapping {
    /// Check if mapping is valid
    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.address.is_null() && self.size > 0
    }
}

// ============================================================================
// Intel GPU Device
// ============================================================================

/// Intel GPU device wrapper
///
/// Provides access to Intel GPU via Linux sysfs and mmap.
/// Connects to MmioRegionCapsule for lockfree MMIO operations.
///
/// # Memory Layout (256B, 128B-aligned)
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │ Offset │ Field           │ Size │ Description                  │
/// ├────────┼─────────────────┼──────┼──────────────────────────────┤
/// │ 0x00   │ drm_fd          │ 4B   │ DRM file descriptor          │
/// │ 0x04   │ config_fd       │ 4B   │ PCI config space fd          │
/// │ 0x08   │ mmio_base       │ 8B   │ MMIO virtual address         │
/// │ 0x10   │ mmio_size       │ 8B   │ MMIO region size             │
/// │ 0x18   │ device_id       │ 2B   │ PCI device ID                │
/// │ 0x1A   │ revision        │ 1B   │ PCI revision ID              │
/// │ 0x1B   │ generation      │ 1B   │ Intel GPU generation         │
/// │ 0x1C   │ flags           │ 4B   │ State flags (atomic)         │
/// │ 0x20   │ bdf             │ 4B   │ Bus/Device/Function          │
/// │ 0x24   │ bar_sizes[6]    │ 48B  │ BAR sizes (u64 each)         │
/// │ 0x54   │ bar_ptrs[6]     │ 48B  │ BAR mappings (ptr each)      │
/// │ 0x84   │ state           │ 8B   │ Pointer to LinuxHalState     │
/// │ 0x8C   │ gen_counter     │ 4B   │ Generation counter (ABA)     │
/// │ 0x90   │ _padding        │ 48B  │ Padding to 256B              │
/// └─────────────────────────────────────────────────────────────────┘
/// ```
#[repr(C, align(128))]
pub struct IntelGpuDevice {
    /// DRM file descriptor (/dev/dri/cardN)
    drm_fd: AtomicU32,
    /// PCI config space file descriptor
    config_fd: AtomicU32,
    /// MMIO base virtual address (BAR0 typically)
    mmio_base: AtomicPtr<u8>,
    /// MMIO region size in bytes
    mmio_size: AtomicU64,
    /// PCI device ID
    device_id: u16,
    /// PCI revision ID
    revision: u8,
    /// Intel GPU generation
    generation: IntelGpuGen,
    /// State flags (bit 0: open, bit 1: mmio mapped, bit 2: master)
    flags: AtomicU32,
    /// PCI BDF address
    bdf: PciBdf,
    /// BAR sizes (6 BARs maximum)
    bar_sizes: [AtomicU64; 6],
    /// BAR virtual addresses
    bar_ptrs: [AtomicPtr<u8>; 6],
    /// Shared state pointer
    state: AtomicPtr<LinuxHalState>,
    /// Generation counter for ABA prevention
    gen_counter: AtomicU32,
    /// Padding to 256B
    _padding: [u8; 16],
}

// SAFETY: IntelGpuDevice uses atomic operations for all shared state
unsafe impl Send for IntelGpuDevice {}
unsafe impl Sync for IntelGpuDevice {}

impl IntelGpuDevice {
    /// Flag: Device is open
    const FLAG_OPEN: u32 = 0x01;
    /// Flag: MMIO is mapped
    const FLAG_MMIO_MAPPED: u32 = 0x02;
    /// Flag: DRM master
    const FLAG_DRM_MASTER: u32 = 0x04;

    /// Invalid file descriptor sentinel
    const INVALID_FD: u32 = u32::MAX;

    /// Create uninitialized device (for static allocation)
    ///
    /// Must call `open()` before use.
    #[inline]
    pub const fn uninit() -> Self {
        Self {
            drm_fd: AtomicU32::new(Self::INVALID_FD),
            config_fd: AtomicU32::new(Self::INVALID_FD),
            mmio_base: AtomicPtr::new(ptr::null_mut()),
            mmio_size: AtomicU64::new(0),
            device_id: 0,
            revision: 0,
            generation: IntelGpuGen::Unknown,
            flags: AtomicU32::new(0),
            bdf: PciBdf::new(0, 0, 0, 0),
            bar_sizes: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            ],
            bar_ptrs: [
                AtomicPtr::new(ptr::null_mut()), AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()), AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()), AtomicPtr::new(ptr::null_mut()),
            ],
            state: AtomicPtr::new(ptr::null_mut()),
            gen_counter: AtomicU32::new(0),
            _padding: [0u8; 16],
        }
    }

    /// Open Intel GPU device at specified PCI address
    ///
    /// # Arguments
    /// * `domain` - PCI domain (usually 0)
    /// * `bus` - PCI bus number
    /// * `device` - PCI device number
    /// * `function` - PCI function number
    ///
    /// # Performance
    /// ~1-5ms (file opens, mmap, device enumeration)
    ///
    /// # ASSUM
    /// - `#ASSUME_PCI_DEVICE_EXISTS`: Intel GPU present at BDF
    /// - `#ASSUME_SYSFS_AVAILABLE`: /sys mounted (standard Linux)
    #[cfg(feature = "std")]
    pub fn open(domain: u16, bus: u8, device: u8, function: u8) -> LinuxHalResult<Self> {
        use std::fs::{File, OpenOptions};
        use std::io::{Read, Seek, SeekFrom};
        use std::os::unix::io::AsRawFd;

        let bdf = PciBdf::new(domain, bus, device, function);
        let sysfs_path = bdf.sysfs_path();

        // Read vendor/device ID from config space
        let config_path = format!("{}/config", sysfs_path);
        let mut config_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    LinuxHalError::DeviceNotFound
                } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                    LinuxHalError::PermissionDenied
                } else {
                    LinuxHalError::InternalError
                }
            })?;

        // Read vendor ID (offset 0x00)
        let mut buf = [0u8; 4];
        config_file.seek(SeekFrom::Start(0)).map_err(|_| LinuxHalError::PciConfigReadFailed(0))?;
        config_file.read_exact(&mut buf).map_err(|_| LinuxHalError::PciConfigReadFailed(0))?;
        let vendor_id = u16::from_le_bytes([buf[0], buf[1]]);
        let device_id = u16::from_le_bytes([buf[2], buf[3]]);

        // Verify Intel vendor ID
        if vendor_id != IntelPciId::INTEL_VENDOR_ID {
            return Err(LinuxHalError::DeviceNotFound);
        }

        // Read revision (offset 0x08)
        config_file.seek(SeekFrom::Start(0x08)).map_err(|_| LinuxHalError::PciConfigReadFailed(0x08))?;
        config_file.read_exact(&mut buf).map_err(|_| LinuxHalError::PciConfigReadFailed(0x08))?;
        let revision = buf[0];

        let pci_id = IntelPciId::new(device_id, revision);
        let generation = pci_id.detect_generation();

        // Read BAR0 size from resource file
        let resource_path = format!("{}/resource", sysfs_path);
        let resource_content = std::fs::read_to_string(&resource_path)
            .map_err(|_| LinuxHalError::BarMappingFailed(0))?;

        let mut bar_sizes = [0u64; 6];
        for (i, line) in resource_content.lines().take(6).enumerate() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                // Parse start and end addresses
                let start = u64::from_str_radix(parts[0].trim_start_matches("0x"), 16).unwrap_or(0);
                let end = u64::from_str_radix(parts[1].trim_start_matches("0x"), 16).unwrap_or(0);
                if end > start {
                    bar_sizes[i] = end - start + 1;
                }
            }
        }

        // Open DRM device
        let drm_path = bdf.drm_path();
        let drm_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&drm_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    LinuxHalError::PermissionDenied
                } else {
                    LinuxHalError::DeviceNotFound
                }
            })?;

        let drm_fd = drm_file.as_raw_fd() as u32;
        let config_fd = config_file.as_raw_fd() as u32;

        // Don't close files - we're keeping the FDs
        std::mem::forget(drm_file);
        std::mem::forget(config_file);

        let mut dev = Self {
            drm_fd: AtomicU32::new(drm_fd),
            config_fd: AtomicU32::new(config_fd),
            mmio_base: AtomicPtr::new(ptr::null_mut()),
            mmio_size: AtomicU64::new(0),
            device_id,
            revision,
            generation,
            flags: AtomicU32::new(Self::FLAG_OPEN),
            bdf,
            bar_sizes: [
                AtomicU64::new(bar_sizes[0]), AtomicU64::new(bar_sizes[1]),
                AtomicU64::new(bar_sizes[2]), AtomicU64::new(bar_sizes[3]),
                AtomicU64::new(bar_sizes[4]), AtomicU64::new(bar_sizes[5]),
            ],
            bar_ptrs: [
                AtomicPtr::new(ptr::null_mut()), AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()), AtomicPtr::new(ptr::null_mut()),
                AtomicPtr::new(ptr::null_mut()), AtomicPtr::new(ptr::null_mut()),
            ],
            state: AtomicPtr::new(ptr::null_mut()),
            gen_counter: AtomicU32::new(1),
            _padding: [0u8; 16],
        };

        // Map MMIO region (BAR0) if available
        if bar_sizes[0] > 0 {
            dev.map_mmio_bar0()?;
        }

        Ok(dev)
    }

    /// Map BAR0 MMIO region
    #[cfg(feature = "std")]
    fn map_mmio_bar0(&mut self) -> LinuxHalResult<()> {
        use std::fs::OpenOptions;
        use std::os::unix::io::AsRawFd;

        let bar0_size = self.bar_sizes[0].load(Ordering::Relaxed);
        if bar0_size == 0 {
            return Err(LinuxHalError::BarMappingFailed(0));
        }

        // Open resource0 file for mmap
        let resource_path = format!("{}/resource0", self.bdf.sysfs_path());
        let resource_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&resource_path)
            .map_err(|_| LinuxHalError::BarMappingFailed(0))?;

        // mmap the BAR
        // SAFETY: We're mapping a valid PCI BAR resource via sysfs
        // The kernel validates the mapping parameters
        let ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                bar0_size as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                resource_file.as_raw_fd(),
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            return Err(LinuxHalError::BarMappingFailed(0));
        }

        self.mmio_base.store(ptr as *mut u8, Ordering::Release);
        self.mmio_size.store(bar0_size, Ordering::Release);
        self.bar_ptrs[0].store(ptr as *mut u8, Ordering::Release);
        self.flags.fetch_or(Self::FLAG_MMIO_MAPPED, Ordering::Release);

        // Keep file open - close would invalidate mmap
        std::mem::forget(resource_file);

        Ok(())
    }

    /// Close device and release resources
    #[cfg(feature = "std")]
    pub fn close(&self) -> LinuxHalResult<()> {
        // Unmap BARs
        for i in 0..6 {
            let ptr = self.bar_ptrs[i].swap(ptr::null_mut(), Ordering::AcqRel);
            let size = self.bar_sizes[i].load(Ordering::Relaxed);
            if !ptr.is_null() && size > 0 {
                // SAFETY: ptr was previously mmap'd with this size
                unsafe {
                    libc::munmap(ptr as *mut libc::c_void, size as usize);
                }
            }
        }

        // Close file descriptors
        let drm_fd = self.drm_fd.swap(Self::INVALID_FD, Ordering::AcqRel);
        if drm_fd != Self::INVALID_FD {
            // SAFETY: drm_fd is a valid file descriptor we opened
            unsafe { libc::close(drm_fd as i32); }
        }

        let config_fd = self.config_fd.swap(Self::INVALID_FD, Ordering::AcqRel);
        if config_fd != Self::INVALID_FD {
            // SAFETY: config_fd is a valid file descriptor we opened
            unsafe { libc::close(config_fd as i32); }
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

    /// Check if MMIO is mapped
    #[inline]
    pub fn is_mmio_mapped(&self) -> bool {
        (self.flags.load(Ordering::Acquire) & Self::FLAG_MMIO_MAPPED) != 0
    }

    /// Get device ID
    #[inline]
    pub fn device_id(&self) -> u16 {
        self.device_id
    }

    /// Get revision ID
    #[inline]
    pub fn revision(&self) -> u8 {
        self.revision
    }

    /// Get GPU generation
    #[inline]
    pub fn generation(&self) -> IntelGpuGen {
        self.generation
    }

    /// Get MMIO size
    #[inline]
    pub fn mmio_size(&self) -> u64 {
        self.mmio_size.load(Ordering::Relaxed)
    }

    /// Get PCI BDF address
    #[inline]
    pub fn bdf(&self) -> PciBdf {
        self.bdf
    }

    /// Read 32-bit MMIO register
    ///
    /// # Arguments
    /// * `offset` - Register offset from BAR0 base
    ///
    /// # Performance
    /// ~5ns (volatile read, no syscall)
    ///
    /// # Safety
    /// - Device must be open with MMIO mapped
    /// - Offset must be within MMIO region bounds
    /// - Offset must be 4-byte aligned
    #[inline]
    pub fn read_mmio(&self, offset: u64) -> LinuxHalResult<u32> {
        if !self.is_mmio_mapped() {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let size = self.mmio_size.load(Ordering::Relaxed);
        if offset + 4 > size {
            return Err(LinuxHalError::InvalidPciConfigOffset(offset as u16));
        }

        if offset % 4 != 0 {
            return Err(LinuxHalError::InvalidPciConfigOffset(offset as u16));
        }

        let base = self.mmio_base.load(Ordering::Acquire);
        if base.is_null() {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        // SAFETY:
        // - base is valid mmap'd pointer (#ASSUME_MMAP_SUCCESS)
        // - offset is within bounds (checked above)
        // - offset is 4-byte aligned (checked above)
        // - volatile read prevents compiler reordering
        let value = unsafe {
            let ptr = base.add(offset as usize) as *const u32;
            ptr::read_volatile(ptr)
        };

        Ok(value)
    }

    /// Write 32-bit MMIO register
    ///
    /// # Arguments
    /// * `offset` - Register offset from BAR0 base
    /// * `value` - Value to write
    ///
    /// # Performance
    /// ~5ns (volatile write, no syscall)
    ///
    /// # Safety
    /// - Device must be open with MMIO mapped
    /// - Offset must be within MMIO region bounds
    /// - Offset must be 4-byte aligned
    #[inline]
    pub fn write_mmio(&self, offset: u64, value: u32) -> LinuxHalResult<()> {
        if !self.is_mmio_mapped() {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        let size = self.mmio_size.load(Ordering::Relaxed);
        if offset + 4 > size {
            return Err(LinuxHalError::InvalidPciConfigOffset(offset as u16));
        }

        if offset % 4 != 0 {
            return Err(LinuxHalError::InvalidPciConfigOffset(offset as u16));
        }

        let base = self.mmio_base.load(Ordering::Acquire);
        if base.is_null() {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        // SAFETY: Same as read_mmio
        unsafe {
            let ptr = base.add(offset as usize) as *mut u32;
            ptr::write_volatile(ptr, value);
        }

        Ok(())
    }

    /// Read 64-bit MMIO register
    #[inline]
    pub fn read_mmio64(&self, offset: u64) -> LinuxHalResult<u64> {
        if offset % 8 != 0 {
            return Err(LinuxHalError::InvalidPciConfigOffset(offset as u16));
        }

        // Read as two 32-bit values for portability
        let low = self.read_mmio(offset)?;
        let high = self.read_mmio(offset + 4)?;
        Ok(((high as u64) << 32) | (low as u64))
    }

    /// Write 64-bit MMIO register
    #[inline]
    pub fn write_mmio64(&self, offset: u64, value: u64) -> LinuxHalResult<()> {
        if offset % 8 != 0 {
            return Err(LinuxHalError::InvalidPciConfigOffset(offset as u16));
        }

        // Write as two 32-bit values for portability
        self.write_mmio(offset, value as u32)?;
        self.write_mmio(offset + 4, (value >> 32) as u32)?;
        Ok(())
    }

    /// Get DRM file descriptor for ioctl operations
    #[inline]
    pub fn drm_fd(&self) -> Option<i32> {
        let fd = self.drm_fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            None
        } else {
            Some(fd as i32)
        }
    }

    /// Get BAR size
    #[inline]
    pub fn get_bar_size(&self, bar_index: u8) -> LinuxHalResult<u64> {
        if bar_index >= 6 {
            return Err(LinuxHalError::InvalidBarIndex(bar_index));
        }
        Ok(self.bar_sizes[bar_index as usize].load(Ordering::Relaxed))
    }

    /// Get BAR virtual address (if mapped)
    #[inline]
    pub fn get_bar_ptr(&self, bar_index: u8) -> LinuxHalResult<*mut u8> {
        if bar_index >= 6 {
            return Err(LinuxHalError::InvalidBarIndex(bar_index));
        }
        Ok(self.bar_ptrs[bar_index as usize].load(Ordering::Acquire))
    }
}

impl Drop for IntelGpuDevice {
    fn drop(&mut self) {
        #[cfg(feature = "std")]
        {
            let _ = self.close();
        }
    }
}

// ============================================================================
// LinuxPciAccess Implementation
// ============================================================================

#[cfg(feature = "std")]
impl LinuxPciAccess for IntelGpuDevice {
    fn read_config(&self, offset: u16) -> LinuxHalResult<u32> {
        use std::io::{Read, Seek, SeekFrom};
        use std::os::unix::io::FromRawFd;

        let fd = self.config_fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        if offset > 0xFFC {
            return Err(LinuxHalError::InvalidPciConfigOffset(offset));
        }

        // SAFETY: fd is valid (we opened it)
        let mut file = unsafe { std::fs::File::from_raw_fd(fd as i32) };

        let result = (|| {
            file.seek(SeekFrom::Start(offset as u64))
                .map_err(|_| LinuxHalError::PciConfigReadFailed(offset))?;

            let mut buf = [0u8; 4];
            file.read_exact(&mut buf)
                .map_err(|_| LinuxHalError::PciConfigReadFailed(offset))?;

            Ok(u32::from_le_bytes(buf))
        })();

        // Don't close the file - we need to keep the fd
        std::mem::forget(file);

        result
    }

    fn write_config(&self, offset: u16, value: u32) -> LinuxHalResult<()> {
        use std::io::{Write, Seek, SeekFrom};
        use std::os::unix::io::FromRawFd;

        let fd = self.config_fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            return Err(LinuxHalError::DeviceNotOpen);
        }

        if offset > 0xFFC {
            return Err(LinuxHalError::InvalidPciConfigOffset(offset));
        }

        // SAFETY: fd is valid
        let mut file = unsafe { std::fs::File::from_raw_fd(fd as i32) };

        let result = (|| {
            file.seek(SeekFrom::Start(offset as u64))
                .map_err(|_| LinuxHalError::PciConfigWriteFailed(offset))?;

            let buf = value.to_le_bytes();
            file.write_all(&buf)
                .map_err(|_| LinuxHalError::PciConfigWriteFailed(offset))?;

            Ok(())
        })();

        std::mem::forget(file);

        result
    }

    fn map_bar(&self, bar_index: u8) -> LinuxHalResult<*mut u8> {
        use std::fs::OpenOptions;
        use std::os::unix::io::AsRawFd;

        if bar_index >= 6 {
            return Err(LinuxHalError::InvalidBarIndex(bar_index));
        }

        // Check if already mapped
        let existing = self.bar_ptrs[bar_index as usize].load(Ordering::Acquire);
        if !existing.is_null() {
            return Ok(existing);
        }

        let size = self.bar_sizes[bar_index as usize].load(Ordering::Relaxed);
        if size == 0 {
            return Err(LinuxHalError::BarMappingFailed(bar_index));
        }

        let resource_path = format!("{}/resource{}", self.bdf.sysfs_path(), bar_index);
        let resource_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&resource_path)
            .map_err(|_| LinuxHalError::BarMappingFailed(bar_index))?;

        // SAFETY: We're mapping a valid PCI BAR resource via sysfs
        let ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                size as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                resource_file.as_raw_fd(),
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            return Err(LinuxHalError::BarMappingFailed(bar_index));
        }

        self.bar_ptrs[bar_index as usize].store(ptr as *mut u8, Ordering::Release);
        std::mem::forget(resource_file);

        Ok(ptr as *mut u8)
    }

    fn unmap_bar(&self, bar_index: u8) -> LinuxHalResult<()> {
        if bar_index >= 6 {
            return Err(LinuxHalError::InvalidBarIndex(bar_index));
        }

        let ptr = self.bar_ptrs[bar_index as usize].swap(ptr::null_mut(), Ordering::AcqRel);
        let size = self.bar_sizes[bar_index as usize].load(Ordering::Relaxed);

        if !ptr.is_null() && size > 0 {
            // SAFETY: ptr was mapped with this size
            let result = unsafe { libc::munmap(ptr as *mut libc::c_void, size as usize) };
            if result != 0 {
                return Err(LinuxHalError::BarUnmappingFailed(bar_index));
            }
        }

        Ok(())
    }

    fn get_bar_size(&self, bar_index: u8) -> LinuxHalResult<usize> {
        if bar_index >= 6 {
            return Err(LinuxHalError::InvalidBarIndex(bar_index));
        }
        Ok(self.bar_sizes[bar_index as usize].load(Ordering::Relaxed) as usize)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pci_bdf_display() {
        let bdf = PciBdf::new(0, 0, 2, 0);
        assert_eq!(format!("{}", bdf), "0000:00:02.0");
    }

    #[test]
    fn test_intel_pci_id_generation_detection() {
        // Skylake
        let id = IntelPciId::new(0x1912, 0);
        assert_eq!(id.detect_generation(), IntelGpuGen::Gen9);

        // Tiger Lake
        let id = IntelPciId::new(0x9A49, 0);
        assert_eq!(id.detect_generation(), IntelGpuGen::Gen12);

        // Unknown
        let id = IntelPciId::new(0x0000, 0);
        assert_eq!(id.detect_generation(), IntelGpuGen::Unknown);
    }

    #[test]
    fn test_intel_gpu_device_uninit() {
        let device = IntelGpuDevice::uninit();
        assert!(!device.is_open());
        assert!(!device.is_mmio_mapped());
        assert_eq!(device.device_id(), 0);
        assert_eq!(device.generation(), IntelGpuGen::Unknown);
    }

    #[test]
    fn test_intel_gpu_device_size_and_alignment() {
        // Verify struct is properly aligned
        assert!(core::mem::align_of::<IntelGpuDevice>() >= 128);
    }

    #[test]
    fn test_bar_mapping_struct() {
        let mapping = BarMapping {
            address: 0x1000 as *mut u8,
            size: 4096,
            bar_index: 0,
        };
        assert!(mapping.is_valid());

        let invalid = BarMapping {
            address: ptr::null_mut(),
            size: 0,
            bar_index: 0,
        };
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_mmio_bounds_check() {
        let device = IntelGpuDevice::uninit();

        // Should fail - device not open
        assert!(device.read_mmio(0).is_err());
        assert!(device.write_mmio(0, 0).is_err());
    }
}
