//! PCI Device Capsule - T1 Atomic, 256B
//!
//! # Architecture
//! - **Tier 1 (Atomic)**: Lockfree device configuration space access
//! - **256-byte alignment**: 4 cache lines for full standard config space
//! - **Generation counters**: ABA prevention for state transitions
//! - **100% lockfree**: Atomic CAS-based operations
//!
//! # Device Capsule Overview
//! The PCI Device Capsule provides access to PCI configuration space:
//! - Standard 256-byte configuration header
//! - Device identification (Vendor ID, Device ID, Class codes)
//! - BAR references and capability pointers
//! - Command/Status register management
//! - Device state machine tracking
//!
//! # PCI Configuration Space Header (Type 0, Standard Device)
//!
//! ```text
//! Offset  Size  Field
//! ──────  ────  ─────────────────────────────────
//! 0x00    2     Vendor ID
//! 0x02    2     Device ID
//! 0x04    2     Command
//! 0x06    2     Status
//! 0x08    1     Revision ID
//! 0x09    1     Prog IF
//! 0x0A    1     Subclass
//! 0x0B    1     Class Code
//! 0x0C    1     Cache Line Size
//! 0x0D    1     Latency Timer
//! 0x0E    1     Header Type
//! 0x0F    1     BIST
//! 0x10    4     BAR0
//! 0x14    4     BAR1
//! 0x18    4     BAR2
//! 0x1C    4     BAR3
//! 0x20    4     BAR4
//! 0x24    4     BAR5
//! 0x28    4     CardBus CIS Pointer
//! 0x2C    2     Subsystem Vendor ID
//! 0x2E    2     Subsystem ID
//! 0x30    4     Expansion ROM Base
//! 0x34    1     Capabilities Pointer
//! 0x35    7     Reserved
//! 0x3C    1     Interrupt Line
//! 0x3D    1     Interrupt Pin
//! 0x3E    1     Min Grant
//! 0x3F    1     Max Latency
//! ```
//!
//! # Performance Targets
//! - State snapshot: <10ns (single cache line)
//! - Config read (cached): <5ns
//! - Config write: <100ns (atomic CAS)
//! - State transition: <50ns
//!
//! # Safety Assumptions (ASSUM Framework)
//! - #ASSUME[CONFIG-VALID]: Configuration space properly mapped
//! - #ASSUME[BDF-VALID]: Bus/Device/Function within valid range
//! - #VERIFY[STATE-CAS]: State transitions atomic via CAS
//! - #VERIFY[VID-VALID]: Vendor ID != 0xFFFF indicates valid device
//! - #VERIFY[GENERATION]: Generation counter prevents ABA

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU16, AtomicU8, Ordering};

// ============================================================================
// Configuration Space Offsets
// ============================================================================

/// Vendor ID (16 bits, offset 0x00)
pub const CONFIG_VENDOR_ID: u8 = 0x00;
/// Device ID (16 bits, offset 0x02)
pub const CONFIG_DEVICE_ID: u8 = 0x02;
/// Command Register (16 bits, offset 0x04)
pub const CONFIG_COMMAND: u8 = 0x04;
/// Status Register (16 bits, offset 0x06)
pub const CONFIG_STATUS: u8 = 0x06;
/// Revision ID (8 bits, offset 0x08)
pub const CONFIG_REVISION: u8 = 0x08;
/// Programming Interface (8 bits, offset 0x09)
pub const CONFIG_PROG_IF: u8 = 0x09;
/// Subclass Code (8 bits, offset 0x0A)
pub const CONFIG_SUBCLASS: u8 = 0x0A;
/// Class Code (8 bits, offset 0x0B)
pub const CONFIG_CLASS: u8 = 0x0B;
/// Cache Line Size (8 bits, offset 0x0C)
pub const CONFIG_CACHE_LINE_SIZE: u8 = 0x0C;
/// Latency Timer (8 bits, offset 0x0D)
pub const CONFIG_LATENCY_TIMER: u8 = 0x0D;
/// Header Type (8 bits, offset 0x0E)
pub const CONFIG_HEADER_TYPE: u8 = 0x0E;
/// Built-in Self Test (8 bits, offset 0x0F)
pub const CONFIG_BIST: u8 = 0x0F;
/// Base Address Register 0 (32 bits, offset 0x10)
pub const CONFIG_BAR0: u8 = 0x10;
/// Base Address Register 1 (32 bits, offset 0x14)
pub const CONFIG_BAR1: u8 = 0x14;
/// Base Address Register 2 (32 bits, offset 0x18)
pub const CONFIG_BAR2: u8 = 0x18;
/// Base Address Register 3 (32 bits, offset 0x1C)
pub const CONFIG_BAR3: u8 = 0x1C;
/// Base Address Register 4 (32 bits, offset 0x20)
pub const CONFIG_BAR4: u8 = 0x20;
/// Base Address Register 5 (32 bits, offset 0x24)
pub const CONFIG_BAR5: u8 = 0x24;
/// CardBus CIS Pointer (32 bits, offset 0x28)
pub const CONFIG_CARDBUS_CIS: u8 = 0x28;
/// Subsystem Vendor ID (16 bits, offset 0x2C)
pub const CONFIG_SUBSYS_VENDOR: u8 = 0x2C;
/// Subsystem ID (16 bits, offset 0x2E)
pub const CONFIG_SUBSYS_ID: u8 = 0x2E;
/// Expansion ROM Base Address (32 bits, offset 0x30)
pub const CONFIG_ROM_BASE: u8 = 0x30;
/// Capabilities Pointer (8 bits, offset 0x34)
pub const CONFIG_CAPABILITIES: u8 = 0x34;
/// Interrupt Line (8 bits, offset 0x3C)
pub const CONFIG_INT_LINE: u8 = 0x3C;
/// Interrupt Pin (8 bits, offset 0x3D)
pub const CONFIG_INT_PIN: u8 = 0x3D;
/// Min Grant (8 bits, offset 0x3E)
pub const CONFIG_MIN_GRANT: u8 = 0x3E;
/// Max Latency (8 bits, offset 0x3F)
pub const CONFIG_MAX_LATENCY: u8 = 0x3F;

// ============================================================================
// Command Register Bits
// ============================================================================

/// I/O Space Enable
pub const CMD_IO_SPACE: u16 = 1 << 0;
/// Memory Space Enable
pub const CMD_MEMORY_SPACE: u16 = 1 << 1;
/// Bus Master Enable
pub const CMD_BUS_MASTER: u16 = 1 << 2;
/// Special Cycles Enable
pub const CMD_SPECIAL_CYCLES: u16 = 1 << 3;
/// Memory Write and Invalidate Enable
pub const CMD_MWI_ENABLE: u16 = 1 << 4;
/// VGA Palette Snoop
pub const CMD_VGA_SNOOP: u16 = 1 << 5;
/// Parity Error Response
pub const CMD_PARITY_ERROR: u16 = 1 << 6;
/// SERR# Enable
pub const CMD_SERR_ENABLE: u16 = 1 << 8;
/// Fast Back-to-Back Enable
pub const CMD_FAST_B2B: u16 = 1 << 9;
/// Interrupt Disable
pub const CMD_INT_DISABLE: u16 = 1 << 10;

// ============================================================================
// Status Register Bits
// ============================================================================

/// Interrupt Status
pub const STATUS_INT: u16 = 1 << 3;
/// Capabilities List
pub const STATUS_CAP_LIST: u16 = 1 << 4;
/// 66 MHz Capable
pub const STATUS_66MHZ: u16 = 1 << 5;
/// Fast Back-to-Back Capable
pub const STATUS_FAST_B2B: u16 = 1 << 7;
/// Master Data Parity Error
pub const STATUS_PARITY_ERROR: u16 = 1 << 8;
/// Signaled Target Abort
pub const STATUS_SIG_TARGET_ABORT: u16 = 1 << 11;
/// Received Target Abort
pub const STATUS_RCV_TARGET_ABORT: u16 = 1 << 12;
/// Received Master Abort
pub const STATUS_RCV_MASTER_ABORT: u16 = 1 << 13;
/// Signaled System Error
pub const STATUS_SIG_SYSTEM_ERROR: u16 = 1 << 14;
/// Detected Parity Error
pub const STATUS_PARITY_DETECTED: u16 = 1 << 15;

// ============================================================================
// Device Class Codes
// ============================================================================

/// PCI Device Class
///
/// #VERIFY[CLASS-USB-IF]: Values per USB-IF/PCI-SIG class code assignments
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PciDeviceClass {
    /// Unclassified device (class 0x00)
    Unclassified = 0x00,
    /// Mass Storage Controller (class 0x01)
    MassStorage = 0x01,
    /// Network Controller (class 0x02)
    Network = 0x02,
    /// Display Controller (class 0x03)
    Display = 0x03,
    /// Multimedia Controller (class 0x04)
    Multimedia = 0x04,
    /// Memory Controller (class 0x05)
    Memory = 0x05,
    /// Bridge (class 0x06)
    Bridge = 0x06,
    /// Simple Communication Controller (class 0x07)
    Communication = 0x07,
    /// Base System Peripheral (class 0x08)
    SystemPeripheral = 0x08,
    /// Input Device Controller (class 0x09)
    Input = 0x09,
    /// Docking Station (class 0x0A)
    DockingStation = 0x0A,
    /// Processor (class 0x0B)
    Processor = 0x0B,
    /// Serial Bus Controller (class 0x0C)
    SerialBus = 0x0C,
    /// Wireless Controller (class 0x0D)
    Wireless = 0x0D,
    /// Intelligent Controller (class 0x0E)
    Intelligent = 0x0E,
    /// Satellite Communication (class 0x0F)
    Satellite = 0x0F,
    /// Encryption Controller (class 0x10)
    Encryption = 0x10,
    /// Signal Processing Controller (class 0x11)
    SignalProcessing = 0x11,
    /// Processing Accelerator (class 0x12)
    Accelerator = 0x12,
    /// Non-Essential Instrumentation (class 0x13)
    Instrumentation = 0x13,
    /// Coprocessor (class 0x40)
    Coprocessor = 0x40,
    /// Unassigned Class (class 0xFF)
    Unassigned = 0xFF,
    /// Unknown class
    Unknown = 0xFE,
}

impl PciDeviceClass {
    /// Convert from raw class code
    #[inline(always)]
    pub fn from_code(code: u8) -> Self {
        match code {
            0x00 => PciDeviceClass::Unclassified,
            0x01 => PciDeviceClass::MassStorage,
            0x02 => PciDeviceClass::Network,
            0x03 => PciDeviceClass::Display,
            0x04 => PciDeviceClass::Multimedia,
            0x05 => PciDeviceClass::Memory,
            0x06 => PciDeviceClass::Bridge,
            0x07 => PciDeviceClass::Communication,
            0x08 => PciDeviceClass::SystemPeripheral,
            0x09 => PciDeviceClass::Input,
            0x0A => PciDeviceClass::DockingStation,
            0x0B => PciDeviceClass::Processor,
            0x0C => PciDeviceClass::SerialBus,
            0x0D => PciDeviceClass::Wireless,
            0x0E => PciDeviceClass::Intelligent,
            0x0F => PciDeviceClass::Satellite,
            0x10 => PciDeviceClass::Encryption,
            0x11 => PciDeviceClass::SignalProcessing,
            0x12 => PciDeviceClass::Accelerator,
            0x13 => PciDeviceClass::Instrumentation,
            0x40 => PciDeviceClass::Coprocessor,
            0xFF => PciDeviceClass::Unassigned,
            _ => PciDeviceClass::Unknown,
        }
    }

    /// Get class code
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Get human-readable name
    #[inline(always)]
    pub const fn name(self) -> &'static str {
        match self {
            PciDeviceClass::Unclassified => "Unclassified",
            PciDeviceClass::MassStorage => "Mass Storage",
            PciDeviceClass::Network => "Network",
            PciDeviceClass::Display => "Display",
            PciDeviceClass::Multimedia => "Multimedia",
            PciDeviceClass::Memory => "Memory",
            PciDeviceClass::Bridge => "Bridge",
            PciDeviceClass::Communication => "Communication",
            PciDeviceClass::SystemPeripheral => "System Peripheral",
            PciDeviceClass::Input => "Input",
            PciDeviceClass::DockingStation => "Docking Station",
            PciDeviceClass::Processor => "Processor",
            PciDeviceClass::SerialBus => "Serial Bus",
            PciDeviceClass::Wireless => "Wireless",
            PciDeviceClass::Intelligent => "Intelligent",
            PciDeviceClass::Satellite => "Satellite",
            PciDeviceClass::Encryption => "Encryption",
            PciDeviceClass::SignalProcessing => "Signal Processing",
            PciDeviceClass::Accelerator => "Accelerator",
            PciDeviceClass::Instrumentation => "Instrumentation",
            PciDeviceClass::Coprocessor => "Coprocessor",
            PciDeviceClass::Unassigned => "Unassigned",
            PciDeviceClass::Unknown => "Unknown",
        }
    }
}

// ============================================================================
// Header Type
// ============================================================================

/// PCI Header Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PciHeaderType {
    /// Standard device (Type 0)
    Standard = 0x00,
    /// PCI-to-PCI Bridge (Type 1)
    PciToPciBridge = 0x01,
    /// CardBus Bridge (Type 2)
    CardBusBridge = 0x02,
    /// Multi-function device (bit 7 set)
    MultiFunction = 0x80,
    /// Unknown header type
    Unknown = 0xFF,
}

impl PciHeaderType {
    /// Parse header type from raw value
    pub fn from_raw(raw: u8) -> Self {
        match raw & 0x7F {
            0x00 => {
                if raw & 0x80 != 0 {
                    PciHeaderType::MultiFunction
                } else {
                    PciHeaderType::Standard
                }
            }
            0x01 => PciHeaderType::PciToPciBridge,
            0x02 => PciHeaderType::CardBusBridge,
            _ => PciHeaderType::Unknown,
        }
    }

    /// Check if multi-function
    pub const fn is_multifunction(raw: u8) -> bool {
        raw & 0x80 != 0
    }

    /// Get base type (without multi-function bit)
    pub const fn base_type(raw: u8) -> u8 {
        raw & 0x7F
    }
}

// ============================================================================
// Device State
// ============================================================================

/// PCI Device state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PciDeviceState {
    /// Device capsule not initialized
    Uninitialized = 0,
    /// Device detected but config not read
    Detected = 1,
    /// Configuration space read successfully
    Enumerated = 2,
    /// BARs mapped and device ready
    Configured = 3,
    /// Device active and operational
    Active = 4,
    /// Device suspended/powered down
    Suspended = 5,
    /// Error state
    Error = 254,
    /// Device removed/disabled
    Disabled = 255,
}

impl PciDeviceState {
    /// Extract state from packed u64
    #[inline(always)]
    pub fn from_packed(packed: u64) -> Self {
        match (packed & 0xFF) as u8 {
            0 => PciDeviceState::Uninitialized,
            1 => PciDeviceState::Detected,
            2 => PciDeviceState::Enumerated,
            3 => PciDeviceState::Configured,
            4 => PciDeviceState::Active,
            5 => PciDeviceState::Suspended,
            254 => PciDeviceState::Error,
            255 => PciDeviceState::Disabled,
            _ => PciDeviceState::Error,
        }
    }

    /// Pack state with BDF and generation
    ///
    /// # Layout
    /// - Bits 0-7: State (8 bits)
    /// - Bits 8-15: Bus (8 bits)
    /// - Bits 16-20: Device (5 bits)
    /// - Bits 21-23: Function (3 bits)
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
        let b = (bus as u64) << 8;
        let d = ((device & 0x1F) as u64) << 16;
        let f = ((function & 0x07) as u64) << 21;
        let e = (error as u64) << 24;
        let g = (generation & 0xFFFF_FFFF) << 32;
        state | b | d | f | e | g
    }

    /// Check if device is operational
    #[inline(always)]
    pub const fn is_operational(&self) -> bool {
        matches!(self, PciDeviceState::Configured | PciDeviceState::Active)
    }
}

// ============================================================================
// Device Error Codes
// ============================================================================

/// PCI Device error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PciDeviceError {
    /// No error
    Success = 0,
    /// Invalid BDF
    InvalidBdf = 1,
    /// No device at BDF (vendor ID = 0xFFFF)
    NoDevice = 2,
    /// Config read failed
    ConfigReadFailed = 3,
    /// Config write failed
    ConfigWriteFailed = 4,
    /// Invalid state transition
    InvalidTransition = 5,
    /// BAR mapping failed
    BarMappingFailed = 6,
    /// Generation mismatch
    GenerationMismatch = 7,
    /// Device removed
    DeviceRemoved = 8,
    /// Unknown error
    Unknown = 255,
}

impl PciDeviceError {
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }

    #[inline(always)]
    pub fn from_code(code: u8) -> Self {
        match code {
            0 => PciDeviceError::Success,
            1 => PciDeviceError::InvalidBdf,
            2 => PciDeviceError::NoDevice,
            3 => PciDeviceError::ConfigReadFailed,
            4 => PciDeviceError::ConfigWriteFailed,
            5 => PciDeviceError::InvalidTransition,
            6 => PciDeviceError::BarMappingFailed,
            7 => PciDeviceError::GenerationMismatch,
            8 => PciDeviceError::DeviceRemoved,
            _ => PciDeviceError::Unknown,
        }
    }
}

/// Result type for PCI device operations
pub type PciDeviceResult<T> = Result<T, PciDeviceError>;

// ============================================================================
// Device Snapshot
// ============================================================================

/// Atomic snapshot of PCI device state
#[derive(Debug, Clone, Copy)]
pub struct PciDeviceSnapshot {
    /// Current state
    pub state: PciDeviceState,
    /// Generation counter
    pub generation: u64,
    /// Bus number
    pub bus: u8,
    /// Device number
    pub device: u8,
    /// Function number
    pub function: u8,
    /// Last error code
    pub error: PciDeviceError,
    /// Vendor ID
    pub vendor_id: u16,
    /// Device ID
    pub device_id: u16,
    /// Class code
    pub class_code: PciDeviceClass,
    /// Subclass code
    pub subclass: u8,
    /// Programming interface
    pub prog_if: u8,
    /// Revision ID
    pub revision: u8,
    /// Header type
    pub header_type: u8,
    /// Subsystem vendor ID
    pub subsys_vendor_id: u16,
    /// Subsystem ID
    pub subsys_id: u16,
    /// Command register
    pub command: u16,
    /// Status register
    pub status: u16,
    /// Capabilities pointer
    pub capabilities_ptr: u8,
    /// Interrupt line
    pub interrupt_line: u8,
    /// Interrupt pin
    pub interrupt_pin: u8,
    /// BAR values (raw)
    pub bars: [u32; 6],
}

impl PciDeviceSnapshot {
    /// Check if device is valid (vendor ID != 0xFFFF)
    #[inline(always)]
    pub fn is_valid(&self) -> bool {
        self.vendor_id != 0xFFFF
    }

    /// Check if device is operational
    #[inline(always)]
    pub fn is_operational(&self) -> bool {
        self.state.is_operational()
    }

    /// Check if device is a bridge
    #[inline(always)]
    pub fn is_bridge(&self) -> bool {
        self.class_code == PciDeviceClass::Bridge
    }

    /// Check if multi-function device
    #[inline(always)]
    pub fn is_multifunction(&self) -> bool {
        PciHeaderType::is_multifunction(self.header_type)
    }

    /// Check if has capabilities list
    #[inline(always)]
    pub fn has_capabilities(&self) -> bool {
        (self.status & STATUS_CAP_LIST) != 0
    }

    /// Get class name
    #[inline(always)]
    pub fn class_name(&self) -> &'static str {
        self.class_code.name()
    }
}

// ============================================================================
// PCI Device Capsule (256 bytes)
// ============================================================================

/// PCI Device Capsule (256 bytes, cache-aligned)
///
/// **Architecture**: Tier 1 (Atomic)
/// - Lockfree configuration space access
/// - Generation counters for ABA prevention
/// - Cached config space header
///
/// # Memory Layout (256 bytes, 4 cache lines)
///
/// ## Cache Line 0 (64 bytes) - Identity & State
/// - state_gen: State + BDF + error + generation (8 bytes)
/// - vendor_device: Vendor ID (16) + Device ID (16) (4 bytes)
/// - class_rev: Class (8) + Subclass (8) + ProgIF (8) + Revision (8) (4 bytes)
/// - header_bist: Header Type (8) + BIST (8) + reserved (16) (4 bytes)
/// - subsys: Subsystem Vendor (16) + Subsystem ID (16) (4 bytes)
/// - command_status: Command (16) + Status (16) (4 bytes)
/// - cache_latency: Cache Line Size (8) + Latency Timer (8) + reserved (16) (4 bytes)
/// - cap_int: Capabilities (8) + Int Line (8) + Int Pin (8) + reserved (8) (4 bytes)
/// - Reserved (28 bytes)
///
/// ## Cache Line 1 (64 bytes) - BAR Values
/// - bar0: BAR0 raw value (4 bytes)
/// - bar1: BAR1 raw value (4 bytes)
/// - bar2: BAR2 raw value (4 bytes)
/// - bar3: BAR3 raw value (4 bytes)
/// - bar4: BAR4 raw value (4 bytes)
/// - bar5: BAR5 raw value (4 bytes)
/// - rom_base: Expansion ROM base (4 bytes)
/// - Reserved (36 bytes)
///
/// ## Cache Line 2 (64 bytes) - Statistics
/// - config_reads: Total config reads (8 bytes)
/// - config_writes: Total config writes (8 bytes)
/// - errors: Total errors (8 bytes)
/// - Reserved (40 bytes)
///
/// ## Cache Line 3 (64 bytes) - Extended
/// - Reserved for extended config space caching (64 bytes)
///
/// #ASSUME[CACHE-ALIGN]: 256-byte alignment prevents false sharing
/// #VERIFY[SIZE-256]: Structure exactly 256 bytes
#[repr(C, align(256))]
pub struct PciDeviceCapsule {
    // === Cache Line 0 (64 bytes) - Identity & State ===
    /// Packed state: state (8) | bus (8) | device (5) | function (3) | error (8) | gen (32)
    state_gen: AtomicU64,
    /// Vendor ID (bits 0-15) + Device ID (bits 16-31)
    vendor_device: AtomicU32,
    /// Class (bits 0-7) + Subclass (bits 8-15) + ProgIF (bits 16-23) + Revision (bits 24-31)
    class_rev: AtomicU32,
    /// Header Type (bits 0-7) + BIST (bits 8-15)
    header_bist: AtomicU16,
    /// Cache Line Size (bits 0-7) + Latency Timer (bits 8-15)
    cache_latency: AtomicU16,
    /// Subsystem Vendor (bits 0-15) + Subsystem ID (bits 16-31)
    subsys: AtomicU32,
    /// Command (bits 0-15) + Status (bits 16-31)
    command_status: AtomicU32,
    /// Capabilities (bits 0-7) + Int Line (bits 8-15) + Int Pin (bits 16-23)
    cap_int: AtomicU32,
    /// Reserved padding for cache line 0
    _reserved_cl0: [u8; 28],

    // === Cache Line 1 (64 bytes) - BAR Values ===
    /// BAR0 raw value
    bar0: AtomicU32,
    /// BAR1 raw value
    bar1: AtomicU32,
    /// BAR2 raw value
    bar2: AtomicU32,
    /// BAR3 raw value
    bar3: AtomicU32,
    /// BAR4 raw value
    bar4: AtomicU32,
    /// BAR5 raw value
    bar5: AtomicU32,
    /// Expansion ROM base address
    rom_base: AtomicU32,
    /// Reserved padding for cache line 1
    _reserved_cl1: [u8; 36],

    // === Cache Line 2 (64 bytes) - Statistics ===
    /// Total configuration space reads
    config_reads: AtomicU64,
    /// Total configuration space writes
    config_writes: AtomicU64,
    /// Total errors encountered
    errors: AtomicU64,
    /// Reserved padding for cache line 2
    _reserved_cl2: [u8; 40],

    // === Cache Line 3 (64 bytes) - Extended ===
    /// Reserved for extended config space / capabilities
    _reserved_cl3: [u8; 64],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<PciDeviceCapsule>() == 256);
const _: () = assert!(core::mem::align_of::<PciDeviceCapsule>() == 256);

impl PciDeviceCapsule {
    /// Create new PCI device capsule
    ///
    /// #VERIFY[INIT-UNINIT]: Initial state is Uninitialized
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(PciDeviceState::Uninitialized.pack(0, 0, 0, 0, 0)),
            vendor_device: AtomicU32::new(0xFFFF_FFFF), // Invalid vendor/device
            class_rev: AtomicU32::new(0),
            header_bist: AtomicU16::new(0),
            cache_latency: AtomicU16::new(0),
            subsys: AtomicU32::new(0),
            command_status: AtomicU32::new(0),
            cap_int: AtomicU32::new(0),
            _reserved_cl0: [0u8; 28],
            bar0: AtomicU32::new(0),
            bar1: AtomicU32::new(0),
            bar2: AtomicU32::new(0),
            bar3: AtomicU32::new(0),
            bar4: AtomicU32::new(0),
            bar5: AtomicU32::new(0),
            rom_base: AtomicU32::new(0),
            _reserved_cl1: [0u8; 36],
            config_reads: AtomicU64::new(0),
            config_writes: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            _reserved_cl2: [0u8; 40],
            _reserved_cl3: [0u8; 64],
        }
    }

    /// Initialize device from BDF
    ///
    /// # Arguments
    /// - `bus`: PCI bus number (0-255)
    /// - `device`: Device number (0-31)
    /// - `function`: Function number (0-7)
    ///
    /// #ASSUME[BDF-VALID]: Caller provides valid BDF
    pub fn initialize(&self, bus: u8, device: u8, function: u8) -> PciDeviceResult<()> {
        if device > 31 || function > 7 {
            return Err(PciDeviceError::InvalidBdf);
        }

        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = PciDeviceState::from_packed(current);

            // Can only initialize from Uninitialized or Error state
            if !matches!(state, PciDeviceState::Uninitialized | PciDeviceState::Error) {
                return Err(PciDeviceError::InvalidTransition);
            }

            let gen = ((current >> 32) & 0xFFFF_FFFF) + 1;
            let new_packed = PciDeviceState::Detected.pack(gen, bus, device, function, 0);

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

    /// Get atomic snapshot of device state
    ///
    /// #VERIFY[SNAPSHOT-ATOMIC]: All reads use Acquire ordering
    #[inline(always)]
    pub fn snapshot(&self) -> PciDeviceSnapshot {
        let state_packed = self.state_gen.load(Ordering::Acquire);
        let vendor_device = self.vendor_device.load(Ordering::Acquire);
        let class_rev = self.class_rev.load(Ordering::Acquire);
        let header_bist = self.header_bist.load(Ordering::Acquire);
        let subsys = self.subsys.load(Ordering::Acquire);
        let command_status = self.command_status.load(Ordering::Acquire);
        let cap_int = self.cap_int.load(Ordering::Acquire);

        PciDeviceSnapshot {
            state: PciDeviceState::from_packed(state_packed),
            generation: (state_packed >> 32) & 0xFFFF_FFFF,
            bus: ((state_packed >> 8) & 0xFF) as u8,
            device: ((state_packed >> 16) & 0x1F) as u8,
            function: ((state_packed >> 21) & 0x07) as u8,
            error: PciDeviceError::from_code(((state_packed >> 24) & 0xFF) as u8),
            vendor_id: (vendor_device & 0xFFFF) as u16,
            device_id: ((vendor_device >> 16) & 0xFFFF) as u16,
            class_code: PciDeviceClass::from_code((class_rev & 0xFF) as u8),
            subclass: ((class_rev >> 8) & 0xFF) as u8,
            prog_if: ((class_rev >> 16) & 0xFF) as u8,
            revision: ((class_rev >> 24) & 0xFF) as u8,
            header_type: (header_bist & 0xFF) as u8,
            subsys_vendor_id: (subsys & 0xFFFF) as u16,
            subsys_id: ((subsys >> 16) & 0xFFFF) as u16,
            command: (command_status & 0xFFFF) as u16,
            status: ((command_status >> 16) & 0xFFFF) as u16,
            capabilities_ptr: (cap_int & 0xFF) as u8,
            interrupt_line: ((cap_int >> 8) & 0xFF) as u8,
            interrupt_pin: ((cap_int >> 16) & 0xFF) as u8,
            bars: [
                self.bar0.load(Ordering::Acquire),
                self.bar1.load(Ordering::Acquire),
                self.bar2.load(Ordering::Acquire),
                self.bar3.load(Ordering::Acquire),
                self.bar4.load(Ordering::Acquire),
                self.bar5.load(Ordering::Acquire),
            ],
        }
    }

    /// Get current state only (fast path)
    #[inline(always)]
    pub fn state(&self) -> PciDeviceState {
        PciDeviceState::from_packed(self.state_gen.load(Ordering::Acquire))
    }

    /// Get vendor ID
    #[inline(always)]
    pub fn vendor_id(&self) -> u16 {
        (self.vendor_device.load(Ordering::Acquire) & 0xFFFF) as u16
    }

    /// Get device ID
    #[inline(always)]
    pub fn device_id(&self) -> u16 {
        ((self.vendor_device.load(Ordering::Acquire) >> 16) & 0xFFFF) as u16
    }

    /// Get class code
    #[inline(always)]
    pub fn class_code(&self) -> PciDeviceClass {
        PciDeviceClass::from_code((self.class_rev.load(Ordering::Acquire) & 0xFF) as u8)
    }

    /// Get BDF as tuple
    #[inline(always)]
    pub fn bdf(&self) -> (u8, u8, u8) {
        let packed = self.state_gen.load(Ordering::Acquire);
        (
            ((packed >> 8) & 0xFF) as u8,
            ((packed >> 16) & 0x1F) as u8,
            ((packed >> 21) & 0x07) as u8,
        )
    }

    /// Set vendor and device ID from config space read
    ///
    /// #VERIFY[VID-VALID]: Vendor ID 0xFFFF means no device
    pub fn set_vendor_device(&self, vendor_id: u16, device_id: u16) -> PciDeviceResult<()> {
        if vendor_id == 0xFFFF {
            self.set_error(PciDeviceError::NoDevice);
            return Err(PciDeviceError::NoDevice);
        }

        let packed = (vendor_id as u32) | ((device_id as u32) << 16);
        self.vendor_device.store(packed, Ordering::Release);
        self.config_reads.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Set class codes from config space read
    pub fn set_class(&self, class: u8, subclass: u8, prog_if: u8, revision: u8) {
        let packed = (class as u32)
            | ((subclass as u32) << 8)
            | ((prog_if as u32) << 16)
            | ((revision as u32) << 24);
        self.class_rev.store(packed, Ordering::Release);
        self.config_reads.fetch_add(1, Ordering::Relaxed);
    }

    /// Set header type and BIST
    pub fn set_header(&self, header_type: u8, bist: u8) {
        let packed = (header_type as u16) | ((bist as u16) << 8);
        self.header_bist.store(packed, Ordering::Release);
    }

    /// Set subsystem IDs
    pub fn set_subsystem(&self, vendor: u16, id: u16) {
        let packed = (vendor as u32) | ((id as u32) << 16);
        self.subsys.store(packed, Ordering::Release);
    }

    /// Set command and status registers
    pub fn set_command_status(&self, command: u16, status: u16) {
        let packed = (command as u32) | ((status as u32) << 16);
        self.command_status.store(packed, Ordering::Release);
    }

    /// Set capabilities and interrupt info
    pub fn set_cap_interrupt(&self, cap_ptr: u8, int_line: u8, int_pin: u8) {
        let packed = (cap_ptr as u32) | ((int_line as u32) << 8) | ((int_pin as u32) << 16);
        self.cap_int.store(packed, Ordering::Release);
    }

    /// Set BAR value
    ///
    /// #ASSUME[BAR-INDEX]: bar_index in range 0-5
    pub fn set_bar(&self, bar_index: u8, value: u32) -> PciDeviceResult<()> {
        match bar_index {
            0 => self.bar0.store(value, Ordering::Release),
            1 => self.bar1.store(value, Ordering::Release),
            2 => self.bar2.store(value, Ordering::Release),
            3 => self.bar3.store(value, Ordering::Release),
            4 => self.bar4.store(value, Ordering::Release),
            5 => self.bar5.store(value, Ordering::Release),
            _ => return Err(PciDeviceError::BarMappingFailed),
        }
        Ok(())
    }

    /// Get BAR value
    #[inline(always)]
    pub fn get_bar(&self, bar_index: u8) -> PciDeviceResult<u32> {
        match bar_index {
            0 => Ok(self.bar0.load(Ordering::Acquire)),
            1 => Ok(self.bar1.load(Ordering::Acquire)),
            2 => Ok(self.bar2.load(Ordering::Acquire)),
            3 => Ok(self.bar3.load(Ordering::Acquire)),
            4 => Ok(self.bar4.load(Ordering::Acquire)),
            5 => Ok(self.bar5.load(Ordering::Acquire)),
            _ => Err(PciDeviceError::BarMappingFailed),
        }
    }

    /// Set expansion ROM base
    pub fn set_rom_base(&self, value: u32) {
        self.rom_base.store(value, Ordering::Release);
    }

    /// Get expansion ROM base
    #[inline(always)]
    pub fn rom_base(&self) -> u32 {
        self.rom_base.load(Ordering::Acquire)
    }

    /// Transition to Enumerated state
    pub fn mark_enumerated(&self) -> PciDeviceResult<()> {
        self.transition_state(PciDeviceState::Detected, PciDeviceState::Enumerated)
    }

    /// Transition to Configured state
    pub fn mark_configured(&self) -> PciDeviceResult<()> {
        self.transition_state(PciDeviceState::Enumerated, PciDeviceState::Configured)
    }

    /// Transition to Active state
    pub fn mark_active(&self) -> PciDeviceResult<()> {
        self.transition_state(PciDeviceState::Configured, PciDeviceState::Active)
    }

    /// Suspend device
    pub fn suspend(&self) -> PciDeviceResult<()> {
        let state = self.state();
        if !matches!(state, PciDeviceState::Configured | PciDeviceState::Active) {
            return Err(PciDeviceError::InvalidTransition);
        }
        self.transition_state(state, PciDeviceState::Suspended)
    }

    /// Resume device
    pub fn resume(&self) -> PciDeviceResult<()> {
        self.transition_state(PciDeviceState::Suspended, PciDeviceState::Configured)
    }

    /// Disable device
    pub fn disable(&self) -> PciDeviceResult<()> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let gen = ((current >> 32) & 0xFFFF_FFFF) + 1;
            let new_packed = PciDeviceState::Disabled.pack(gen, 0, 0, 0, 0);

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

    /// Set error state
    pub fn set_error(&self, error: PciDeviceError) {
        self.errors.fetch_add(1, Ordering::Relaxed);

        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let bus = ((current >> 8) & 0xFF) as u8;
            let device = ((current >> 16) & 0x1F) as u8;
            let function = ((current >> 21) & 0x07) as u8;
            let gen = ((current >> 32) & 0xFFFF_FFFF) + 1;

            let new_packed = PciDeviceState::Error.pack(gen, bus, device, function, error.code());

            if self.state_gen.compare_exchange(
                current,
                new_packed,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                break;
            }
        }
    }

    /// Get statistics
    #[inline(always)]
    pub fn stats(&self) -> (u64, u64, u64) {
        (
            self.config_reads.load(Ordering::Relaxed),
            self.config_writes.load(Ordering::Relaxed),
            self.errors.load(Ordering::Relaxed),
        )
    }

    /// State transition with CAS
    fn transition_state(
        &self,
        expected: PciDeviceState,
        new_state: PciDeviceState,
    ) -> PciDeviceResult<()> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let actual_state = PciDeviceState::from_packed(current);

            if actual_state != expected {
                return Err(PciDeviceError::InvalidTransition);
            }

            let bus = ((current >> 8) & 0xFF) as u8;
            let device = ((current >> 16) & 0x1F) as u8;
            let function = ((current >> 21) & 0x07) as u8;
            let gen = ((current >> 32) & 0xFFFF_FFFF) + 1;

            let new_packed = new_state.pack(gen, bus, device, function, 0);

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
}

impl Default for PciDeviceCapsule {
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
    fn test_device_capsule_size() {
        assert_eq!(
            core::mem::size_of::<PciDeviceCapsule>(),
            256,
            "PciDeviceCapsule must be exactly 256 bytes"
        );
    }

    #[test]
    fn test_device_capsule_alignment() {
        assert_eq!(
            core::mem::align_of::<PciDeviceCapsule>(),
            256,
            "PciDeviceCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_device_initial_state() {
        let device = PciDeviceCapsule::new();
        let snapshot = device.snapshot();

        assert_eq!(snapshot.state, PciDeviceState::Uninitialized);
        assert_eq!(snapshot.vendor_id, 0xFFFF);
        assert_eq!(snapshot.device_id, 0xFFFF);
    }

    #[test]
    fn test_device_initialization() {
        let device = PciDeviceCapsule::new();
        assert!(device.initialize(0, 2, 0).is_ok());

        let snapshot = device.snapshot();
        assert_eq!(snapshot.state, PciDeviceState::Detected);
        assert_eq!(snapshot.bus, 0);
        assert_eq!(snapshot.device, 2);
        assert_eq!(snapshot.function, 0);
    }

    #[test]
    fn test_device_invalid_bdf() {
        let device = PciDeviceCapsule::new();

        // Device > 31 should fail
        assert!(device.initialize(0, 32, 0).is_err());

        // Function > 7 should fail
        let device2 = PciDeviceCapsule::new();
        assert!(device2.initialize(0, 0, 8).is_err());
    }

    #[test]
    fn test_device_vendor_setting() {
        let device = PciDeviceCapsule::new();
        device.initialize(0, 2, 0).unwrap();

        device.set_vendor_device(0x8086, 0x5917).unwrap();

        let snapshot = device.snapshot();
        assert_eq!(snapshot.vendor_id, 0x8086);
        assert_eq!(snapshot.device_id, 0x5917);
    }

    #[test]
    fn test_device_invalid_vendor() {
        let device = PciDeviceCapsule::new();
        device.initialize(0, 2, 0).unwrap();

        // 0xFFFF vendor should fail
        assert!(device.set_vendor_device(0xFFFF, 0x0000).is_err());
    }

    #[test]
    fn test_device_class_setting() {
        let device = PciDeviceCapsule::new();
        device.initialize(0, 2, 0).unwrap();

        device.set_class(0x03, 0x00, 0x00, 0x01);

        let snapshot = device.snapshot();
        assert_eq!(snapshot.class_code, PciDeviceClass::Display);
        assert_eq!(snapshot.subclass, 0x00);
        assert_eq!(snapshot.revision, 0x01);
    }

    #[test]
    fn test_device_bar_operations() {
        let device = PciDeviceCapsule::new();
        device.initialize(0, 2, 0).unwrap();

        device.set_bar(0, 0xF000_0000).unwrap();
        device.set_bar(2, 0xE000_0000).unwrap();

        assert_eq!(device.get_bar(0).unwrap(), 0xF000_0000);
        assert_eq!(device.get_bar(2).unwrap(), 0xE000_0000);

        // Invalid BAR index
        assert!(device.set_bar(6, 0).is_err());
        assert!(device.get_bar(6).is_err());
    }

    #[test]
    fn test_device_state_transitions() {
        let device = PciDeviceCapsule::new();

        device.initialize(0, 2, 0).unwrap();
        assert_eq!(device.state(), PciDeviceState::Detected);

        device.mark_enumerated().unwrap();
        assert_eq!(device.state(), PciDeviceState::Enumerated);

        device.mark_configured().unwrap();
        assert_eq!(device.state(), PciDeviceState::Configured);

        device.mark_active().unwrap();
        assert_eq!(device.state(), PciDeviceState::Active);
    }

    #[test]
    fn test_device_suspend_resume() {
        let device = PciDeviceCapsule::new();
        device.initialize(0, 2, 0).unwrap();
        device.mark_enumerated().unwrap();
        device.mark_configured().unwrap();

        device.suspend().unwrap();
        assert_eq!(device.state(), PciDeviceState::Suspended);

        device.resume().unwrap();
        assert_eq!(device.state(), PciDeviceState::Configured);
    }

    #[test]
    fn test_device_class_parsing() {
        let classes = [
            (0x01, PciDeviceClass::MassStorage),
            (0x02, PciDeviceClass::Network),
            (0x03, PciDeviceClass::Display),
            (0x06, PciDeviceClass::Bridge),
            (0x0C, PciDeviceClass::SerialBus),
        ];

        for (code, expected) in classes {
            let class = PciDeviceClass::from_code(code);
            assert_eq!(class, expected);
            assert_eq!(class.code(), code);
        }
    }

    #[test]
    fn test_header_type_parsing() {
        assert_eq!(PciHeaderType::from_raw(0x00), PciHeaderType::Standard);
        assert_eq!(PciHeaderType::from_raw(0x01), PciHeaderType::PciToPciBridge);
        assert_eq!(PciHeaderType::from_raw(0x80), PciHeaderType::MultiFunction);

        assert!(PciHeaderType::is_multifunction(0x80));
        assert!(!PciHeaderType::is_multifunction(0x00));
    }

    // ========================================================================
    // Q8-Q14: Property Tests
    // ========================================================================

    #[test]
    fn test_state_roundtrip() {
        let states = [
            PciDeviceState::Uninitialized,
            PciDeviceState::Detected,
            PciDeviceState::Enumerated,
            PciDeviceState::Configured,
            PciDeviceState::Active,
            PciDeviceState::Suspended,
            PciDeviceState::Error,
            PciDeviceState::Disabled,
        ];

        for state in states {
            let packed = state.pack(12345, 100, 20, 5, 10);
            let unpacked = PciDeviceState::from_packed(packed);
            assert_eq!(unpacked, state);

            // Verify BDF extraction
            let bus = ((packed >> 8) & 0xFF) as u8;
            let device = ((packed >> 16) & 0x1F) as u8;
            let function = ((packed >> 21) & 0x07) as u8;

            assert_eq!(bus, 100);
            assert_eq!(device, 20);
            assert_eq!(function, 5);
        }
    }

    #[test]
    fn test_error_roundtrip() {
        let errors = [
            PciDeviceError::Success,
            PciDeviceError::InvalidBdf,
            PciDeviceError::NoDevice,
            PciDeviceError::ConfigReadFailed,
            PciDeviceError::InvalidTransition,
        ];

        for error in errors {
            let code = error.code();
            let recovered = PciDeviceError::from_code(code);
            assert_eq!(recovered, error);
        }
    }

    #[test]
    fn test_full_lifecycle() {
        let device = PciDeviceCapsule::new();

        // Initialize
        device.initialize(1, 5, 2).unwrap();
        assert_eq!(device.bdf(), (1, 5, 2));

        // Set device info
        device.set_vendor_device(0x10DE, 0x1B80).unwrap();
        device.set_class(0x03, 0x00, 0x00, 0xA1);
        device.set_header(0x00, 0x00);
        device.set_subsystem(0x10DE, 0x1234);
        device.set_cap_interrupt(0x60, 11, 1);

        // Set BARs
        device.set_bar(0, 0xFB00_0000).unwrap();
        device.set_bar(1, 0xE000_0000).unwrap();

        // Transition through states
        device.mark_enumerated().unwrap();
        device.mark_configured().unwrap();
        device.mark_active().unwrap();

        // Verify final snapshot
        let snap = device.snapshot();
        assert!(snap.is_valid());
        assert!(snap.is_operational());
        assert_eq!(snap.vendor_id, 0x10DE);
        assert_eq!(snap.device_id, 0x1B80);
        assert_eq!(snap.class_code, PciDeviceClass::Display);
        assert_eq!(snap.bars[0], 0xFB00_0000);
        assert_eq!(snap.interrupt_line, 11);
    }

    #[test]
    fn test_statistics() {
        let device = PciDeviceCapsule::new();
        device.initialize(0, 2, 0).unwrap();

        device.set_vendor_device(0x8086, 0x5917).unwrap();
        device.set_class(0x03, 0x00, 0x00, 0x01);

        let (reads, writes, errors) = device.stats();
        assert_eq!(reads, 2); // set_vendor_device + set_class
        assert_eq!(writes, 0);
        assert_eq!(errors, 0);
    }
}
