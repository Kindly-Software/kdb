//! PCI Enumerator Capsule - T4 Batch, 4096B
//!
//! # Architecture
//! - **Tier 4 (Batch)**: Parallel bus scanning with batch device discovery
//! - **4096-byte alignment**: Full page for ECAM-mapped config space caching
//! - **Generation counters**: ABA prevention for scan state transitions
//! - **100% lockfree**: Atomic CAS-based operations
//!
//! # Enumerator Overview
//! The PCI Enumerator Capsule provides comprehensive PCI/PCIe bus scanning:
//! - Full 256-bus enumeration (PCI Express hierarchy)
//! - Device presence detection via Vendor ID check
//! - Multi-function device discovery
//! - Batch processing for parallel scanning
//!
//! # PCI Configuration Access Mechanisms
//!
//! ## Legacy Access (I/O Ports 0xCF8/0xCFC)
//! - Address Port: 0xCF8 (32-bit write)
//! - Data Port: 0xCFC (32-bit read/write)
//! - Format: [31:31]=Enable, [23:16]=Bus, [15:11]=Device, [10:8]=Function, [7:0]=Register
//!
//! ## ECAM/MCFG (Memory-Mapped Enhanced Configuration Access)
//! - Base address from ACPI MCFG table
//! - Each device: 4KB config space at offset Bus*1MB + Device*32KB + Function*4KB
//! - Supports full 4KB extended configuration space
//!
//! # Memory Layout (4096 bytes, 64 cache lines)
//!
//! ## Cache Lines 0-3 (256 bytes) - Scan State
//! - state_gen: State + generation counter (8 bytes)
//! - ecam_base: ECAM memory-mapped base address (8 bytes)
//! - current_bus: Current bus being scanned (8 bytes)
//! - current_device: Current device being scanned (8 bytes)
//! - current_function: Current function being scanned (8 bytes)
//! - devices_found: Total devices discovered (8 bytes)
//! - scan_flags: Scan configuration flags (8 bytes)
//! - error_code: Last error encountered (8 bytes)
//! - scan_start_time: Timestamp when scan started (8 bytes)
//! - scan_end_time: Timestamp when scan completed (8 bytes)
//! - Reserved (176 bytes)
//!
//! ## Cache Lines 4-7 (256 bytes) - Batch Discovery Buffer
//! - discovered_devices[0-31]: Packed BDF + class for last 32 devices (256 bytes)
//!
//! ## Cache Lines 8-15 (512 bytes) - Statistics
//! - buses_scanned: Number of buses enumerated (8 bytes)
//! - devices_probed: Number of BDF slots probed (8 bytes)
//! - functions_found: Number of functions discovered (8 bytes)
//! - bridges_found: Number of PCI bridges found (8 bytes)
//! - errors_encountered: Error count during scan (8 bytes)
//! - Reserved (472 bytes)
//!
//! ## Cache Lines 16-63 (3072 bytes) - Config Space Cache
//! - Cached config space for up to 12 devices (256 bytes each)
//!
//! # Performance Targets
//! - Bus scan initiation: <10ns
//! - Single device probe: <100ns (cached) / <1μs (ECAM access)
//! - Full bus scan (256 buses): <10ms (parallel batch)
//! - Device discovery callback: <50ns
//!
//! # Safety Assumptions (ASSUM Framework)
//! - #ASSUME[ECAM-BASE]: ECAM base address from ACPI MCFG is valid
//! - #ASSUME[VENDOR-INVALID]: Vendor ID 0xFFFF indicates no device present
//! - #ASSUME[MULTIFUNCTION]: Header type bit 7 indicates multi-function device
//! - #VERIFY[SCAN-STATE]: Scan state transitions atomic via CAS
//! - #VERIFY[GENERATION]: Generation counter prevents ABA
//! - #VERIFY[BATCH-ATOMIC]: Device discovery buffer updated atomically

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU16, AtomicU8, Ordering};

/// Maximum number of PCI buses (PCIe supports 256)
/// #ASSUME[BUS-LIMIT]: PCI Express specification allows 256 buses
pub const PCI_MAX_BUSES: usize = 256;

/// Maximum devices per bus (PCI specification)
/// #ASSUME[DEVICE-LIMIT]: PCI specification defines 32 devices per bus
pub const PCI_MAX_DEVICES: usize = 32;

/// Maximum functions per device (PCI specification)
/// #ASSUME[FUNCTION-LIMIT]: PCI specification defines 8 functions per device
pub const PCI_MAX_FUNCTIONS: usize = 8;

/// ECAM stride per device (4KB config space)
/// #VERIFY[ECAM-STRIDE]: PCIe ECAM maps 4KB per function
pub const ECAM_DEVICE_STRIDE: usize = 4096;

/// Legacy PCI configuration space size
pub const CONFIG_SPACE_SIZE: usize = 256;

/// Extended PCIe configuration space size
pub const EXT_CONFIG_SPACE_SIZE: usize = 4096;

/// Legacy PCI Configuration Address Port
pub const PCI_CONFIG_ADDRESS: u16 = 0x0CF8;

/// Legacy PCI Configuration Data Port
pub const PCI_CONFIG_DATA: u16 = 0x0CFC;

/// Invalid vendor ID (device not present)
/// #ASSUME[VENDOR-INVALID]: 0xFFFF returned when no device at BDF
pub const PCI_INVALID_VENDOR: u16 = 0xFFFF;

/// Maximum devices in discovery batch buffer
pub const MAX_BATCH_DEVICES: usize = 32;

/// Maximum cached config spaces
pub const MAX_CACHED_CONFIGS: usize = 12;

// ============================================================================
// PCI Enumerator State
// ============================================================================

/// PCI Enumerator state machine
///
/// #VERIFY[STATE-ENUM]: States follow valid enumeration lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PciEnumeratorState {
    /// Enumerator not initialized
    /// #ASSUME[IDLE]: No scan in progress, can be configured
    Idle = 0,
    /// Scan configuration set, ready to start
    /// #VERIFY[CONFIGURED]: ECAM base or legacy access configured
    Configured = 1,
    /// Bus scan in progress
    /// #VERIFY[SCANNING]: Iterating through BDF space
    Scanning = 2,
    /// Scan paused (can resume)
    /// #ASSUME[PAUSED]: State preserved for resume
    Paused = 3,
    /// Scan completed successfully
    /// #VERIFY[COMPLETE]: All buses scanned, results available
    Complete = 4,
    /// Error during enumeration
    /// #VERIFY[ERROR-LOGGED]: Error code captured
    Error = 254,
    /// Enumerator disabled
    Disabled = 255,
}

impl PciEnumeratorState {
    /// Extract state from packed u64
    #[inline(always)]
    pub fn from_packed(packed: u64) -> Self {
        match (packed & 0xFF) as u8 {
            0 => PciEnumeratorState::Idle,
            1 => PciEnumeratorState::Configured,
            2 => PciEnumeratorState::Scanning,
            3 => PciEnumeratorState::Paused,
            4 => PciEnumeratorState::Complete,
            254 => PciEnumeratorState::Error,
            255 => PciEnumeratorState::Disabled,
            _ => PciEnumeratorState::Error,
        }
    }

    /// Pack state with generation counter and progress
    ///
    /// # Layout
    /// - Bits 0-7: State (8 bits)
    /// - Bits 8-15: Current bus (8 bits)
    /// - Bits 16-20: Current device (5 bits)
    /// - Bits 21-23: Current function (3 bits)
    /// - Bits 24-31: Error code (8 bits)
    /// - Bits 32-63: Generation counter (32 bits)
    #[inline(always)]
    pub const fn pack(
        self,
        generation: u64,
        bus: u8,
        device: u8,
        function: u8,
        error: u8,
    ) -> u64 {
        let state = self as u8 as u64;
        let bus_val = (bus as u64) << 8;
        let dev_val = ((device & 0x1F) as u64) << 16;
        let func_val = ((function & 0x07) as u64) << 21;
        let err_val = (error as u64) << 24;
        let gen = (generation & 0xFFFF_FFFF) << 32;
        state | bus_val | dev_val | func_val | err_val | gen
    }
}

// ============================================================================
// PCI Enumerator Error Codes
// ============================================================================

/// PCI enumeration error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PciEnumError {
    /// No error
    Success = 0,
    /// ECAM base not configured
    EcamNotConfigured = 1,
    /// Invalid bus number
    InvalidBus = 2,
    /// Invalid device number
    InvalidDevice = 3,
    /// Invalid function number
    InvalidFunction = 4,
    /// Config space access failed
    ConfigAccessFailed = 5,
    /// Scan already in progress
    ScanInProgress = 6,
    /// Scan not started
    ScanNotStarted = 7,
    /// Buffer full
    BufferFull = 8,
    /// Generation mismatch (CAS failed)
    GenerationMismatch = 9,
    /// Invalid state transition
    InvalidTransition = 10,
    /// Unknown error
    Unknown = 255,
}

impl PciEnumError {
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }

    #[inline(always)]
    pub fn from_code(code: u8) -> Self {
        match code {
            0 => PciEnumError::Success,
            1 => PciEnumError::EcamNotConfigured,
            2 => PciEnumError::InvalidBus,
            3 => PciEnumError::InvalidDevice,
            4 => PciEnumError::InvalidFunction,
            5 => PciEnumError::ConfigAccessFailed,
            6 => PciEnumError::ScanInProgress,
            7 => PciEnumError::ScanNotStarted,
            8 => PciEnumError::BufferFull,
            9 => PciEnumError::GenerationMismatch,
            10 => PciEnumError::InvalidTransition,
            _ => PciEnumError::Unknown,
        }
    }
}

/// Result type for PCI enumeration operations
pub type PciEnumResult<T> = Result<T, PciEnumError>;

// ============================================================================
// PCI Enumerator Snapshot
// ============================================================================

/// Atomic snapshot of PCI enumerator state
#[derive(Debug, Clone, Copy)]
pub struct PciEnumeratorSnapshot {
    /// Current state
    pub state: PciEnumeratorState,
    /// Generation counter
    pub generation: u64,
    /// Current bus being scanned
    pub current_bus: u8,
    /// Current device being scanned
    pub current_device: u8,
    /// Current function being scanned
    pub current_function: u8,
    /// Last error code
    pub error: PciEnumError,
    /// ECAM base address
    pub ecam_base: u64,
    /// Total devices found
    pub devices_found: u32,
    /// Total buses scanned
    pub buses_scanned: u16,
    /// Total errors encountered
    pub errors_encountered: u32,
}

impl PciEnumeratorSnapshot {
    /// Check if scan is complete
    #[inline(always)]
    pub fn is_complete(&self) -> bool {
        self.state == PciEnumeratorState::Complete
    }

    /// Check if scan is in progress
    #[inline(always)]
    pub fn is_scanning(&self) -> bool {
        self.state == PciEnumeratorState::Scanning
    }

    /// Get scan progress percentage (0-100)
    #[inline(always)]
    pub fn progress_percent(&self) -> u8 {
        if self.state == PciEnumeratorState::Complete {
            return 100;
        }
        let total_bdfs = (PCI_MAX_BUSES * PCI_MAX_DEVICES) as u64;
        let current = (self.current_bus as u64) * (PCI_MAX_DEVICES as u64)
            + (self.current_device as u64);
        ((current * 100) / total_bdfs) as u8
    }
}

// ============================================================================
// Discovered Device Entry
// ============================================================================

/// Packed discovered device entry (8 bytes)
///
/// # Layout
/// - Bits 0-7: Bus (8 bits)
/// - Bits 8-12: Device (5 bits)
/// - Bits 13-15: Function (3 bits)
/// - Bits 16-31: Vendor ID (16 bits)
/// - Bits 32-47: Device ID (16 bits)
/// - Bits 48-55: Class code (8 bits)
/// - Bits 56-63: Subclass code (8 bits)
#[derive(Debug, Clone, Copy)]
pub struct DiscoveredDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass_code: u8,
}

impl DiscoveredDevice {
    /// Pack device info into u64
    #[inline(always)]
    pub const fn pack(
        bus: u8,
        device: u8,
        function: u8,
        vendor_id: u16,
        device_id: u16,
        class_code: u8,
        subclass_code: u8,
    ) -> u64 {
        let b = bus as u64;
        let d = ((device & 0x1F) as u64) << 8;
        let f = ((function & 0x07) as u64) << 13;
        let v = (vendor_id as u64) << 16;
        let dev = (device_id as u64) << 32;
        let c = (class_code as u64) << 48;
        let s = (subclass_code as u64) << 56;
        b | d | f | v | dev | c | s
    }

    /// Unpack device info from u64
    #[inline(always)]
    pub fn unpack(packed: u64) -> Self {
        Self {
            bus: (packed & 0xFF) as u8,
            device: ((packed >> 8) & 0x1F) as u8,
            function: ((packed >> 13) & 0x07) as u8,
            vendor_id: ((packed >> 16) & 0xFFFF) as u16,
            device_id: ((packed >> 32) & 0xFFFF) as u16,
            class_code: ((packed >> 48) & 0xFF) as u8,
            subclass_code: ((packed >> 56) & 0xFF) as u8,
        }
    }
}

// ============================================================================
// PCI Configuration Access Trait
// ============================================================================

/// Platform-agnostic PCI configuration access trait
///
/// Implementations provide the actual hardware access mechanism.
pub trait PciConfigAccess: Send + Sync {
    /// Read 8-bit value from config space
    fn read_config_u8(&self, bus: u8, device: u8, function: u8, offset: u8) -> PciEnumResult<u8>;

    /// Read 16-bit value from config space
    fn read_config_u16(&self, bus: u8, device: u8, function: u8, offset: u8) -> PciEnumResult<u16>;

    /// Read 32-bit value from config space
    fn read_config_u32(&self, bus: u8, device: u8, function: u8, offset: u8) -> PciEnumResult<u32>;

    /// Write 8-bit value to config space
    fn write_config_u8(&self, bus: u8, device: u8, function: u8, offset: u8, value: u8) -> PciEnumResult<()>;

    /// Write 16-bit value to config space
    fn write_config_u16(&self, bus: u8, device: u8, function: u8, offset: u8, value: u16) -> PciEnumResult<()>;

    /// Write 32-bit value to config space
    fn write_config_u32(&self, bus: u8, device: u8, function: u8, offset: u8, value: u32) -> PciEnumResult<()>;
}

/// ECAM (Enhanced Configuration Access Mechanism) implementation
///
/// Uses memory-mapped configuration space access.
/// #ASSUME[ECAM-VALID]: ECAM base from ACPI MCFG is correctly mapped
pub struct EcamAccess {
    base: u64,
}

impl EcamAccess {
    /// Create new ECAM access with base address
    pub const fn new(base: u64) -> Self {
        Self { base }
    }

    /// Calculate ECAM offset for BDF
    #[inline(always)]
    fn offset(&self, bus: u8, device: u8, function: u8, offset: u8) -> u64 {
        // ECAM: Bus * 1MB + Device * 32KB + Function * 4KB + Register
        self.base
            + ((bus as u64) << 20)
            + (((device & 0x1F) as u64) << 15)
            + (((function & 0x07) as u64) << 12)
            + (offset as u64)
    }
}

impl PciConfigAccess for EcamAccess {
    fn read_config_u8(&self, bus: u8, device: u8, function: u8, offset: u8) -> PciEnumResult<u8> {
        let addr = self.offset(bus, device, function, offset);
        // #ASSUME[MMIO-SAFE]: ECAM region is properly mapped
        // In real implementation, this would be an MMIO read
        // For now, return placeholder (actual implementation needs platform support)
        let _ = addr;
        Ok(0xFF) // Placeholder - real impl would do volatile read
    }

    fn read_config_u16(&self, bus: u8, device: u8, function: u8, offset: u8) -> PciEnumResult<u16> {
        if offset & 1 != 0 {
            return Err(PciEnumError::ConfigAccessFailed);
        }
        let addr = self.offset(bus, device, function, offset);
        let _ = addr;
        Ok(0xFFFF) // Placeholder
    }

    fn read_config_u32(&self, bus: u8, device: u8, function: u8, offset: u8) -> PciEnumResult<u32> {
        if offset & 3 != 0 {
            return Err(PciEnumError::ConfigAccessFailed);
        }
        let addr = self.offset(bus, device, function, offset);
        let _ = addr;
        Ok(0xFFFF_FFFF) // Placeholder
    }

    fn write_config_u8(&self, bus: u8, device: u8, function: u8, offset: u8, value: u8) -> PciEnumResult<()> {
        let addr = self.offset(bus, device, function, offset);
        let _ = (addr, value);
        Ok(()) // Placeholder
    }

    fn write_config_u16(&self, bus: u8, device: u8, function: u8, offset: u8, value: u16) -> PciEnumResult<()> {
        if offset & 1 != 0 {
            return Err(PciEnumError::ConfigAccessFailed);
        }
        let addr = self.offset(bus, device, function, offset);
        let _ = (addr, value);
        Ok(()) // Placeholder
    }

    fn write_config_u32(&self, bus: u8, device: u8, function: u8, offset: u8, value: u32) -> PciEnumResult<()> {
        if offset & 3 != 0 {
            return Err(PciEnumError::ConfigAccessFailed);
        }
        let addr = self.offset(bus, device, function, offset);
        let _ = (addr, value);
        Ok(()) // Placeholder
    }
}

/// Legacy PCI Configuration Access (I/O ports 0xCF8/0xCFC)
///
/// Uses x86 I/O port access for PCI configuration.
/// #ASSUME[X86-IO]: Platform supports x86 I/O port instructions
pub struct LegacyAccess;

impl LegacyAccess {
    /// Create new legacy access
    pub const fn new() -> Self {
        Self
    }

    /// Build configuration address for legacy mechanism
    #[inline(always)]
    fn config_address(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
        // Bit 31: Enable configuration access
        // Bits 23-16: Bus number
        // Bits 15-11: Device number
        // Bits 10-8: Function number
        // Bits 7-0: Register offset (aligned to 4 bytes)
        0x8000_0000
            | ((bus as u32) << 16)
            | (((device & 0x1F) as u32) << 11)
            | (((function & 0x07) as u32) << 8)
            | ((offset & 0xFC) as u32)
    }
}

impl Default for LegacyAccess {
    fn default() -> Self {
        Self::new()
    }
}

impl PciConfigAccess for LegacyAccess {
    fn read_config_u8(&self, bus: u8, device: u8, function: u8, offset: u8) -> PciEnumResult<u8> {
        // In real implementation, this would use x86 I/O port instructions
        // For now, return placeholder
        let _ = Self::config_address(bus, device, function, offset);
        Ok(0xFF) // Placeholder
    }

    fn read_config_u16(&self, bus: u8, device: u8, function: u8, offset: u8) -> PciEnumResult<u16> {
        if offset & 1 != 0 {
            return Err(PciEnumError::ConfigAccessFailed);
        }
        let _ = Self::config_address(bus, device, function, offset);
        Ok(0xFFFF) // Placeholder
    }

    fn read_config_u32(&self, bus: u8, device: u8, function: u8, offset: u8) -> PciEnumResult<u32> {
        if offset & 3 != 0 {
            return Err(PciEnumError::ConfigAccessFailed);
        }
        let _ = Self::config_address(bus, device, function, offset);
        Ok(0xFFFF_FFFF) // Placeholder
    }

    fn write_config_u8(&self, bus: u8, device: u8, function: u8, offset: u8, value: u8) -> PciEnumResult<()> {
        let _ = (Self::config_address(bus, device, function, offset), value);
        Ok(()) // Placeholder
    }

    fn write_config_u16(&self, bus: u8, device: u8, function: u8, offset: u8, value: u16) -> PciEnumResult<()> {
        if offset & 1 != 0 {
            return Err(PciEnumError::ConfigAccessFailed);
        }
        let _ = (Self::config_address(bus, device, function, offset), value);
        Ok(()) // Placeholder
    }

    fn write_config_u32(&self, bus: u8, device: u8, function: u8, offset: u8, value: u32) -> PciEnumResult<()> {
        if offset & 3 != 0 {
            return Err(PciEnumError::ConfigAccessFailed);
        }
        let _ = (Self::config_address(bus, device, function, offset), value);
        Ok(()) // Placeholder
    }
}

/// Null access for testing
pub struct NullAccess;

impl NullAccess {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for NullAccess {
    fn default() -> Self {
        Self::new()
    }
}

impl PciConfigAccess for NullAccess {
    fn read_config_u8(&self, _bus: u8, _device: u8, _function: u8, _offset: u8) -> PciEnumResult<u8> {
        Ok(0xFF)
    }

    fn read_config_u16(&self, _bus: u8, _device: u8, _function: u8, _offset: u8) -> PciEnumResult<u16> {
        Ok(0xFFFF)
    }

    fn read_config_u32(&self, _bus: u8, _device: u8, _function: u8, _offset: u8) -> PciEnumResult<u32> {
        Ok(0xFFFF_FFFF)
    }

    fn write_config_u8(&self, _bus: u8, _device: u8, _function: u8, _offset: u8, _value: u8) -> PciEnumResult<()> {
        Ok(())
    }

    fn write_config_u16(&self, _bus: u8, _device: u8, _function: u8, _offset: u8, _value: u16) -> PciEnumResult<()> {
        Ok(())
    }

    fn write_config_u32(&self, _bus: u8, _device: u8, _function: u8, _offset: u8, _value: u32) -> PciEnumResult<()> {
        Ok(())
    }
}

// ============================================================================
// PCI Enumerator Capsule (4096 bytes)
// ============================================================================

/// PCI Enumerator Capsule (4096 bytes, page-aligned)
///
/// **Architecture**: Tier 4 (Batch)
/// - Lockfree bus scanning with batch discovery
/// - Generation counters for ABA prevention
/// - Full 256-bus PCIe enumeration
///
/// # Memory Layout (4096 bytes, 64 cache lines)
/// See module documentation for detailed layout.
///
/// #ASSUME[PAGE-ALIGN]: 4096-byte alignment for ECAM page mapping
/// #VERIFY[SIZE-4096]: Structure exactly 4096 bytes
#[repr(C, align(4096))]
pub struct PciEnumeratorCapsule {
    // === Cache Lines 0-3 (256 bytes) - Scan State ===
    /// Packed state: state (8) | bus (8) | device (5) | function (3) | error (8) | gen (32)
    /// #VERIFY[STATE-ATOMIC]: Single atomic for consistent state reads
    state_gen: AtomicU64,
    /// ECAM base address (from ACPI MCFG)
    /// #ASSUME[ECAM-PHYS]: Physical address of ECAM region
    ecam_base: AtomicU64,
    /// Scan configuration flags
    /// Bit 0: Use ECAM (1) or Legacy (0)
    /// Bit 1: Scan all functions (1) or only function 0 if not multi-function
    /// Bit 2: Stop on first error
    /// Bit 3-7: Reserved
    scan_flags: AtomicU32,
    /// Total devices found during scan
    devices_found: AtomicU32,
    /// Scan start timestamp (platform-specific ticks)
    scan_start_time: AtomicU64,
    /// Scan end timestamp
    scan_end_time: AtomicU64,
    /// Maximum bus to scan (default 255)
    max_bus: AtomicU8,
    /// Current batch write index
    batch_write_idx: AtomicU8,
    /// Current batch read index
    batch_read_idx: AtomicU8,
    /// Reserved padding for cache line 0-3
    _reserved_cl0: [u8; 197],

    // === Cache Lines 4-7 (256 bytes) - Batch Discovery Buffer ===
    /// Ring buffer of discovered devices (32 entries × 8 bytes)
    /// #VERIFY[BATCH-RING]: Lockfree SPSC ring buffer
    discovered_devices: [AtomicU64; MAX_BATCH_DEVICES],

    // === Cache Lines 8-15 (512 bytes) - Statistics ===
    /// Number of buses completely scanned
    buses_scanned: AtomicU16,
    /// Total BDF slots probed
    devices_probed: AtomicU64,
    /// Total functions discovered (devices × functions)
    functions_found: AtomicU32,
    /// PCI-to-PCI bridges found
    bridges_found: AtomicU16,
    /// Host bridges found
    host_bridges_found: AtomicU16,
    /// Errors encountered during scan
    errors_encountered: AtomicU32,
    /// Multi-function devices found
    multifunction_devices: AtomicU16,
    /// Reserved padding for cache lines 8-15
    _reserved_stats: [u8; 476],

    // === Cache Lines 16-63 (3072 bytes) - Config Space Cache ===
    /// Cached configuration space (12 devices × 256 bytes)
    /// #ASSUME[CACHE-COHERENT]: Cache updated atomically per device
    config_cache: [[AtomicU32; 64]; MAX_CACHED_CONFIGS],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<PciEnumeratorCapsule>() == 4096);
const _: () = assert!(core::mem::align_of::<PciEnumeratorCapsule>() == 4096);

// Scan flag bits
const FLAG_USE_ECAM: u32 = 1 << 0;
const FLAG_SCAN_ALL_FUNCTIONS: u32 = 1 << 1;
const FLAG_STOP_ON_ERROR: u32 = 1 << 2;

impl PciEnumeratorCapsule {
    /// Create new PCI enumerator capsule
    ///
    /// #VERIFY[INIT-IDLE]: Initial state is Idle
    pub const fn new() -> Self {
        // Initialize discovered_devices array
        const ZERO_ATOMIC: AtomicU64 = AtomicU64::new(0);
        const ZERO_U32: AtomicU32 = AtomicU32::new(0);
        const ZERO_CACHE_LINE: [AtomicU32; 64] = [ZERO_U32; 64];

        Self {
            state_gen: AtomicU64::new(PciEnumeratorState::Idle.pack(0, 0, 0, 0, 0)),
            ecam_base: AtomicU64::new(0),
            scan_flags: AtomicU32::new(FLAG_USE_ECAM | FLAG_SCAN_ALL_FUNCTIONS),
            devices_found: AtomicU32::new(0),
            scan_start_time: AtomicU64::new(0),
            scan_end_time: AtomicU64::new(0),
            max_bus: AtomicU8::new(255),
            batch_write_idx: AtomicU8::new(0),
            batch_read_idx: AtomicU8::new(0),
            _reserved_cl0: [0u8; 197],
            discovered_devices: [ZERO_ATOMIC; MAX_BATCH_DEVICES],
            buses_scanned: AtomicU16::new(0),
            devices_probed: AtomicU64::new(0),
            functions_found: AtomicU32::new(0),
            bridges_found: AtomicU16::new(0),
            host_bridges_found: AtomicU16::new(0),
            errors_encountered: AtomicU32::new(0),
            multifunction_devices: AtomicU16::new(0),
            _reserved_stats: [0u8; 476],
            config_cache: [ZERO_CACHE_LINE; MAX_CACHED_CONFIGS],
        }
    }

    /// Get atomic snapshot of enumerator state
    ///
    /// #VERIFY[SNAPSHOT-ATOMIC]: All reads use Acquire ordering
    #[inline(always)]
    pub fn snapshot(&self) -> PciEnumeratorSnapshot {
        let state_packed = self.state_gen.load(Ordering::Acquire);

        PciEnumeratorSnapshot {
            state: PciEnumeratorState::from_packed(state_packed),
            generation: (state_packed >> 32) & 0xFFFF_FFFF,
            current_bus: ((state_packed >> 8) & 0xFF) as u8,
            current_device: ((state_packed >> 16) & 0x1F) as u8,
            current_function: ((state_packed >> 21) & 0x07) as u8,
            error: PciEnumError::from_code(((state_packed >> 24) & 0xFF) as u8),
            ecam_base: self.ecam_base.load(Ordering::Acquire),
            devices_found: self.devices_found.load(Ordering::Acquire),
            buses_scanned: self.buses_scanned.load(Ordering::Acquire),
            errors_encountered: self.errors_encountered.load(Ordering::Acquire),
        }
    }

    /// Get current state only (fast path)
    #[inline(always)]
    pub fn state(&self) -> PciEnumeratorState {
        PciEnumeratorState::from_packed(self.state_gen.load(Ordering::Acquire))
    }

    /// Set ECAM base address
    ///
    /// # Arguments
    /// - `base`: Physical base address of ECAM region (from ACPI MCFG)
    ///
    /// #ASSUME[ECAM-VALID]: Caller ensures ECAM base is valid
    pub fn set_ecam_base(&self, base: u64) -> PciEnumResult<()> {
        let state = self.state();
        if state != PciEnumeratorState::Idle {
            return Err(PciEnumError::InvalidTransition);
        }

        self.ecam_base.store(base, Ordering::Release);

        // Set use ECAM flag
        self.scan_flags.fetch_or(FLAG_USE_ECAM, Ordering::AcqRel);

        // Transition to Configured
        self.transition_state(PciEnumeratorState::Idle, PciEnumeratorState::Configured)
    }

    /// Configure for legacy PCI access (I/O ports)
    pub fn use_legacy_access(&self) -> PciEnumResult<()> {
        let state = self.state();
        if state != PciEnumeratorState::Idle {
            return Err(PciEnumError::InvalidTransition);
        }

        // Clear ECAM flag
        self.scan_flags.fetch_and(!FLAG_USE_ECAM, Ordering::AcqRel);

        // Transition to Configured
        self.transition_state(PciEnumeratorState::Idle, PciEnumeratorState::Configured)
    }

    /// Set maximum bus number to scan
    pub fn set_max_bus(&self, max: u8) {
        self.max_bus.store(max, Ordering::Release);
    }

    /// Start bus enumeration
    ///
    /// #VERIFY[START-CONFIGURED]: Must be in Configured state
    pub fn start_scan(&self) -> PciEnumResult<()> {
        // Reset statistics
        self.devices_found.store(0, Ordering::Release);
        self.buses_scanned.store(0, Ordering::Release);
        self.devices_probed.store(0, Ordering::Release);
        self.functions_found.store(0, Ordering::Release);
        self.bridges_found.store(0, Ordering::Release);
        self.errors_encountered.store(0, Ordering::Release);
        self.batch_write_idx.store(0, Ordering::Release);
        self.batch_read_idx.store(0, Ordering::Release);

        // Record start time (placeholder)
        self.scan_start_time.store(0, Ordering::Release);

        self.transition_state(PciEnumeratorState::Configured, PciEnumeratorState::Scanning)
    }

    /// Perform one step of enumeration
    ///
    /// Returns true if scan is complete.
    ///
    /// #VERIFY[STEP-SCANNING]: Must be in Scanning state
    pub fn scan_step(&self) -> PciEnumResult<bool> {
        let snap = self.snapshot();
        if snap.state != PciEnumeratorState::Scanning {
            return Err(PciEnumError::ScanNotStarted);
        }

        let bus = snap.current_bus;
        let device = snap.current_device;
        let function = snap.current_function;
        let max_bus = self.max_bus.load(Ordering::Acquire);

        // Check if scan complete
        if bus > max_bus {
            self.scan_end_time.store(0, Ordering::Release); // Placeholder timestamp
            let _ = self.transition_state(PciEnumeratorState::Scanning, PciEnumeratorState::Complete);
            return Ok(true);
        }

        // Update probed count
        self.devices_probed.fetch_add(1, Ordering::AcqRel);

        // Move to next BDF
        let mut next_bus = bus;
        let mut next_device = device;
        let mut next_function = function + 1;

        if next_function >= PCI_MAX_FUNCTIONS as u8 {
            next_function = 0;
            next_device += 1;
        }

        if next_device >= PCI_MAX_DEVICES as u8 {
            next_device = 0;
            next_bus += 1;
            self.buses_scanned.fetch_add(1, Ordering::AcqRel);
        }

        // Update scan position
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let gen = ((current >> 32) & 0xFFFF_FFFF) + 1;
            let new_packed = PciEnumeratorState::Scanning.pack(
                gen,
                next_bus,
                next_device,
                next_function,
                0,
            );

            if self.state_gen.compare_exchange(
                current,
                new_packed,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                break;
            }
        }

        Ok(false)
    }

    /// Record discovered device
    ///
    /// #VERIFY[BATCH-ATOMIC]: Device added to ring buffer atomically
    pub fn record_device(
        &self,
        bus: u8,
        device: u8,
        function: u8,
        vendor_id: u16,
        device_id: u16,
        class_code: u8,
        subclass_code: u8,
    ) -> PciEnumResult<()> {
        let packed = DiscoveredDevice::pack(
            bus, device, function,
            vendor_id, device_id,
            class_code, subclass_code,
        );

        // Get write index and advance (SPSC ring buffer)
        let write_idx = self.batch_write_idx.fetch_add(1, Ordering::AcqRel) as usize;
        let idx = write_idx % MAX_BATCH_DEVICES;

        self.discovered_devices[idx].store(packed, Ordering::Release);
        self.devices_found.fetch_add(1, Ordering::AcqRel);
        self.functions_found.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Get next discovered device from batch buffer
    ///
    /// Returns None if buffer empty.
    pub fn pop_discovered(&self) -> Option<DiscoveredDevice> {
        let write_idx = self.batch_write_idx.load(Ordering::Acquire);
        let read_idx = self.batch_read_idx.load(Ordering::Acquire);

        if read_idx >= write_idx {
            return None;
        }

        let idx = (read_idx as usize) % MAX_BATCH_DEVICES;
        let packed = self.discovered_devices[idx].load(Ordering::Acquire);

        // Advance read index
        self.batch_read_idx.fetch_add(1, Ordering::AcqRel);

        Some(DiscoveredDevice::unpack(packed))
    }

    /// Pause enumeration
    pub fn pause(&self) -> PciEnumResult<()> {
        self.transition_state(PciEnumeratorState::Scanning, PciEnumeratorState::Paused)
    }

    /// Resume enumeration
    pub fn resume(&self) -> PciEnumResult<()> {
        self.transition_state(PciEnumeratorState::Paused, PciEnumeratorState::Scanning)
    }

    /// Reset enumerator to Idle state
    pub fn reset(&self) -> PciEnumResult<()> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = PciEnumeratorState::from_packed(current);

            // Can't reset while scanning
            if state == PciEnumeratorState::Scanning {
                return Err(PciEnumError::ScanInProgress);
            }

            let new_packed = PciEnumeratorState::Idle.pack(0, 0, 0, 0, 0);

            if self.state_gen.compare_exchange(
                current,
                new_packed,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                return Ok(());
            }
        }
    }

    /// Check if scan is complete
    #[inline(always)]
    pub fn is_scan_complete(&self) -> bool {
        self.state() == PciEnumeratorState::Complete
    }

    /// Get total devices found
    #[inline(always)]
    pub fn devices_found(&self) -> u32 {
        self.devices_found.load(Ordering::Acquire)
    }

    /// Get ECAM base address
    #[inline(always)]
    pub fn ecam_base(&self) -> u64 {
        self.ecam_base.load(Ordering::Acquire)
    }

    /// Record error during scan
    pub fn record_error(&self, error: PciEnumError) {
        self.errors_encountered.fetch_add(1, Ordering::AcqRel);

        // Update error in state
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let mask = 0xFFFF_FFFF_00FF_FFFF_u64; // Clear error byte
            let new = (current & mask) | ((error.code() as u64) << 24);

            if self.state_gen.compare_exchange(
                current,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                break;
            }
        }
    }

    /// State transition with CAS
    fn transition_state(
        &self,
        expected: PciEnumeratorState,
        new_state: PciEnumeratorState,
    ) -> PciEnumResult<()> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let actual_state = PciEnumeratorState::from_packed(current);

            if actual_state != expected {
                return Err(PciEnumError::InvalidTransition);
            }

            let bus = ((current >> 8) & 0xFF) as u8;
            let device = ((current >> 16) & 0x1F) as u8;
            let function = ((current >> 21) & 0x07) as u8;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let new_packed = new_state.pack(new_gen, bus, device, function, 0);

            match self.state_gen.compare_exchange(
                current,
                new_packed,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => continue,
            }
        }
    }
}

impl Default for PciEnumeratorCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_enumerator_capsule_size() {
        assert_eq!(
            core::mem::size_of::<PciEnumeratorCapsule>(),
            4096,
            "PciEnumeratorCapsule must be exactly 4096 bytes"
        );
    }

    #[test]
    fn test_enumerator_capsule_alignment() {
        assert_eq!(
            core::mem::align_of::<PciEnumeratorCapsule>(),
            4096,
            "PciEnumeratorCapsule must be 4096-byte aligned"
        );
    }

    #[test]
    fn test_enumerator_initial_state() {
        let enumerator = PciEnumeratorCapsule::new();
        let snapshot = enumerator.snapshot();

        assert_eq!(snapshot.state, PciEnumeratorState::Idle);
        assert_eq!(snapshot.current_bus, 0);
        assert_eq!(snapshot.current_device, 0);
        assert_eq!(snapshot.devices_found, 0);
    }

    #[test]
    fn test_state_packing() {
        let packed = PciEnumeratorState::Scanning.pack(12345, 100, 15, 3, 5);
        let state = PciEnumeratorState::from_packed(packed);
        assert_eq!(state, PciEnumeratorState::Scanning);

        let bus = ((packed >> 8) & 0xFF) as u8;
        let device = ((packed >> 16) & 0x1F) as u8;
        let function = ((packed >> 21) & 0x07) as u8;
        let error = ((packed >> 24) & 0xFF) as u8;
        let gen = ((packed >> 32) & 0xFFFF_FFFF) as u64;

        assert_eq!(bus, 100);
        assert_eq!(device, 15);
        assert_eq!(function, 3);
        assert_eq!(error, 5);
        assert_eq!(gen, 12345);
    }

    #[test]
    fn test_ecam_configuration() {
        let enumerator = PciEnumeratorCapsule::new();
        assert!(enumerator.set_ecam_base(0xE000_0000).is_ok());

        let snapshot = enumerator.snapshot();
        assert_eq!(snapshot.state, PciEnumeratorState::Configured);
        assert_eq!(snapshot.ecam_base, 0xE000_0000);
    }

    #[test]
    fn test_scan_lifecycle() {
        let enumerator = PciEnumeratorCapsule::new();

        // Configure
        enumerator.set_ecam_base(0xE000_0000).unwrap();
        assert_eq!(enumerator.state(), PciEnumeratorState::Configured);

        // Start scan
        enumerator.start_scan().unwrap();
        assert_eq!(enumerator.state(), PciEnumeratorState::Scanning);

        // Pause
        enumerator.pause().unwrap();
        assert_eq!(enumerator.state(), PciEnumeratorState::Paused);

        // Resume
        enumerator.resume().unwrap();
        assert_eq!(enumerator.state(), PciEnumeratorState::Scanning);
    }

    #[test]
    fn test_scan_step() {
        let enumerator = PciEnumeratorCapsule::new();
        enumerator.set_ecam_base(0xE000_0000).unwrap();
        enumerator.set_max_bus(0); // Only scan bus 0
        enumerator.start_scan().unwrap();

        // Run scan steps
        let mut complete = false;
        let mut steps = 0;
        while !complete && steps < 1000 {
            complete = enumerator.scan_step().unwrap();
            steps += 1;
        }

        assert!(complete);
        assert_eq!(enumerator.state(), PciEnumeratorState::Complete);
    }

    #[test]
    fn test_device_recording() {
        let enumerator = PciEnumeratorCapsule::new();
        enumerator.set_ecam_base(0xE000_0000).unwrap();
        enumerator.start_scan().unwrap();

        // Record a device
        enumerator.record_device(0, 2, 0, 0x8086, 0x5917, 0x03, 0x00).unwrap();

        assert_eq!(enumerator.devices_found(), 1);

        // Pop the device
        let dev = enumerator.pop_discovered().unwrap();
        assert_eq!(dev.bus, 0);
        assert_eq!(dev.device, 2);
        assert_eq!(dev.function, 0);
        assert_eq!(dev.vendor_id, 0x8086);
        assert_eq!(dev.device_id, 0x5917);
        assert_eq!(dev.class_code, 0x03);
    }

    #[test]
    fn test_discovered_device_packing() {
        let packed = DiscoveredDevice::pack(5, 10, 3, 0x1234, 0x5678, 0x06, 0x04);
        let dev = DiscoveredDevice::unpack(packed);

        assert_eq!(dev.bus, 5);
        assert_eq!(dev.device, 10);
        assert_eq!(dev.function, 3);
        assert_eq!(dev.vendor_id, 0x1234);
        assert_eq!(dev.device_id, 0x5678);
        assert_eq!(dev.class_code, 0x06);
        assert_eq!(dev.subclass_code, 0x04);
    }

    #[test]
    fn test_error_recording() {
        let enumerator = PciEnumeratorCapsule::new();
        enumerator.set_ecam_base(0xE000_0000).unwrap();
        enumerator.start_scan().unwrap();

        enumerator.record_error(PciEnumError::ConfigAccessFailed);

        let snapshot = enumerator.snapshot();
        assert_eq!(snapshot.errors_encountered, 1);
        assert_eq!(snapshot.error, PciEnumError::ConfigAccessFailed);
    }

    #[test]
    fn test_reset() {
        let enumerator = PciEnumeratorCapsule::new();
        enumerator.set_ecam_base(0xE000_0000).unwrap();
        enumerator.start_scan().unwrap();
        enumerator.pause().unwrap();

        // Reset
        enumerator.reset().unwrap();
        assert_eq!(enumerator.state(), PciEnumeratorState::Idle);
    }

    #[test]
    fn test_ecam_offset_calculation() {
        let ecam = EcamAccess::new(0xE000_0000);
        let offset = ecam.offset(1, 2, 0, 0);

        // Expected: base + bus*1MB + device*32KB + function*4KB
        let expected = 0xE000_0000 + (1 << 20) + (2 << 15);
        assert_eq!(offset, expected);
    }

    #[test]
    fn test_legacy_config_address() {
        let addr = LegacyAccess::config_address(1, 2, 0, 0x10);

        // Expected: Enable(1) | Bus(1) | Device(2) | Function(0) | Offset(0x10)
        let expected = 0x8001_1010;
        assert_eq!(addr, expected);
    }

    // ========================================================================
    // Q8-Q14: Property Tests
    // ========================================================================

    #[test]
    fn test_state_roundtrip() {
        let states = [
            PciEnumeratorState::Idle,
            PciEnumeratorState::Configured,
            PciEnumeratorState::Scanning,
            PciEnumeratorState::Paused,
            PciEnumeratorState::Complete,
            PciEnumeratorState::Error,
            PciEnumeratorState::Disabled,
        ];

        for state in states {
            let packed = state.pack(999, 128, 16, 4, 10);
            let unpacked = PciEnumeratorState::from_packed(packed);
            assert_eq!(unpacked, state);
        }
    }

    #[test]
    fn test_error_roundtrip() {
        let errors = [
            PciEnumError::Success,
            PciEnumError::EcamNotConfigured,
            PciEnumError::InvalidBus,
            PciEnumError::ConfigAccessFailed,
            PciEnumError::ScanInProgress,
            PciEnumError::GenerationMismatch,
        ];

        for error in errors {
            let code = error.code();
            let recovered = PciEnumError::from_code(code);
            assert_eq!(recovered, error);
        }
    }

    #[test]
    fn test_batch_buffer_wrap() {
        let enumerator = PciEnumeratorCapsule::new();
        enumerator.set_ecam_base(0xE000_0000).unwrap();
        enumerator.start_scan().unwrap();

        // Fill buffer beyond capacity
        for i in 0..MAX_BATCH_DEVICES + 10 {
            enumerator.record_device(
                (i % 256) as u8,
                ((i / 8) % 32) as u8,
                (i % 8) as u8,
                0x1234,
                i as u16,
                0x00,
                0x00,
            ).unwrap();
        }

        assert_eq!(enumerator.devices_found(), (MAX_BATCH_DEVICES + 10) as u32);
    }

    #[test]
    fn test_progress_calculation() {
        let enumerator = PciEnumeratorCapsule::new();
        enumerator.set_ecam_base(0xE000_0000).unwrap();
        enumerator.start_scan().unwrap();

        let snap = enumerator.snapshot();
        assert_eq!(snap.progress_percent(), 0);

        // Run half the scan (simulate)
        enumerator.set_max_bus(0);
        while !enumerator.scan_step().unwrap() {}

        let snap = enumerator.snapshot();
        assert_eq!(snap.progress_percent(), 100); // Complete
    }
}
