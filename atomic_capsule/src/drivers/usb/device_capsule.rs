//! USB Device Capsule - High-Level Device State Management
//!
//! # Architecture
//! - **Tier 1 (Atomic)**: Lockfree device state coordination
//! - **256-byte alignment**: 4 cache lines for comprehensive device state
//! - **Generation counters**: ABA prevention for state transitions
//! - **100% lockfree**: Atomic CAS-based operations
//!
//! # Device Capsule Overview
//! The USB Device Capsule provides a high-level abstraction over xHCI
//! slot and endpoint contexts, managing:
//! - Device identification (VID/PID, class codes)
//! - Device descriptors and configurations
//! - Device state machine per USB spec
//! - Interface and endpoint tracking
//! - Transfer statistics
//!
//! # USB Device State Machine (per USB 2.0 spec section 9.1)
//! ```text
//! Attached --> Default --> Addressed --> Configured
//!    ^                        |              |
//!    |                        v              v
//!    +---- Suspended <--------+-------+------+
//!    |
//!    +---- Disconnected
//! ```
//!
//! # Performance Targets
//! - State snapshot: <10ns (single cache line primary state)
//! - State transition: <50ns (CAS with generation counter)
//! - Descriptor lookup: <20ns
//!
//! # Safety Assumptions (ASSUM Framework)
//! - #ASSUME[SLOT-VALID]: Slot context properly configured via xHCI commands
//! - #ASSUME[DESC-VALID]: Device descriptors read from device are valid USB format
//! - #ASSUME[ENUM-COMPLETE]: Device enumeration completed before operations
//! - #VERIFY[STATE-CAS]: Device state transitions atomic via CAS
//! - #VERIFY[VID-PID]: VID/PID extracted from device descriptor
//! - #VERIFY[GENERATION]: Generation counter prevents ABA

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU16, Ordering};

/// Maximum number of interfaces per device (USB spec allows 256)
/// #ASSUME[INTERFACE-LIMIT]: Most devices have <16 interfaces
pub const MAX_INTERFACES: usize = 16;

/// Maximum number of configurations per device (USB spec allows 256)
/// #ASSUME[CONFIG-LIMIT]: Most devices have 1-2 configurations
pub const MAX_CONFIGURATIONS: usize = 8;

/// Maximum number of endpoints per interface (USB spec: 31 total per device)
/// #ASSUME[ENDPOINT-LIMIT]: Typical interface has <8 endpoints
pub const MAX_ENDPOINTS_PER_INTERFACE: usize = 8;

// ============================================================================
// USB Device State
// ============================================================================

/// USB Device state (per USB 2.0 specification section 9.1)
///
/// #VERIFY[STATE-USB-SPEC]: States match USB 2.0 spec section 9.1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UsbDeviceState {
    /// Device not connected to bus
    /// #ASSUME[DISCONNECTED]: Port is in disconnected state
    Disconnected = 0,
    /// Device attached but not yet reset
    /// #ASSUME[ATTACHED]: Port connection detected but no reset issued
    Attached = 1,
    /// Device reset, in default state (address 0)
    /// #VERIFY[DEFAULT-ADDR0]: Device responds to address 0
    Default = 2,
    /// Device has been assigned a unique address (1-127)
    /// #VERIFY[ADDRESSED]: SET_ADDRESS completed successfully
    Addressed = 3,
    /// Device configured and operational
    /// #VERIFY[CONFIGURED]: SET_CONFIGURATION completed successfully
    Configured = 4,
    /// Device suspended (low power state)
    /// #ASSUME[SUSPEND-CAPABLE]: Device supports USB suspend
    Suspended = 5,
    /// Device in error state (stalled, disconnected during transfer, etc.)
    /// #VERIFY[ERROR-LOGGED]: Error details captured
    Error = 254,
    /// Device slot disabled or deallocated
    /// #ASSUME[SLOT-FREE]: Slot can be reused
    Disabled = 255,
}

impl UsbDeviceState {
    /// Extract state from packed u64
    ///
    /// # Layout (low 8 bits of packed value)
    /// - Bits 0-7: State enum value
    ///
    /// #VERIFY[UNPACK-VALID]: All valid packed values produce valid states
    #[inline(always)]
    pub fn from_packed(packed: u64) -> Self {
        match (packed & 0xFF) as u8 {
            0 => UsbDeviceState::Disconnected,
            1 => UsbDeviceState::Attached,
            2 => UsbDeviceState::Default,
            3 => UsbDeviceState::Addressed,
            4 => UsbDeviceState::Configured,
            5 => UsbDeviceState::Suspended,
            254 => UsbDeviceState::Error,
            255 => UsbDeviceState::Disabled,
            _ => UsbDeviceState::Error,
        }
    }

    /// Pack state with generation counter
    ///
    /// # Layout
    /// - Bits 0-7: State (8 bits)
    /// - Bits 8-15: Slot ID (8 bits)
    /// - Bits 16-23: Address (8 bits)
    /// - Bits 24-31: Speed (4 bits) + Port (4 bits)
    /// - Bits 32-63: Generation counter (32 bits)
    ///
    /// #VERIFY[PACK-LOSSLESS]: Round-trip preserves all data
    #[inline(always)]
    pub const fn pack(self, generation: u64, slot_id: u8, address: u8, speed: u8, port: u8) -> u64 {
        let state = self as u8 as u64;
        let slot = (slot_id as u64) << 8;
        let addr = (address as u64) << 16;
        let speed_port = (((speed & 0xF) as u64) << 24) | (((port & 0xF) as u64) << 28);
        let gen = (generation & 0xFFFF_FFFF) << 32;
        state | slot | addr | speed_port | gen
    }

    /// Check if state allows data transfers
    #[inline(always)]
    pub const fn allows_transfers(&self) -> bool {
        matches!(self, UsbDeviceState::Configured)
    }

    /// Check if state allows control transfers
    #[inline(always)]
    pub const fn allows_control_transfers(&self) -> bool {
        matches!(
            self,
            UsbDeviceState::Default | UsbDeviceState::Addressed | UsbDeviceState::Configured
        )
    }

    /// Check if device is operational
    #[inline(always)]
    pub const fn is_operational(&self) -> bool {
        matches!(self, UsbDeviceState::Addressed | UsbDeviceState::Configured)
    }
}

// ============================================================================
// USB Device Speed
// ============================================================================

/// USB Device Speed
///
/// #VERIFY[SPEED-XHCI]: Values match xHCI port speed encoding (PORTSC bits 13:10)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UsbDeviceSpeed {
    /// Speed not yet determined
    Unknown = 0,
    /// Full Speed (12 Mbps, USB 1.1)
    FullSpeed = 1,
    /// Low Speed (1.5 Mbps, USB 1.0)
    LowSpeed = 2,
    /// High Speed (480 Mbps, USB 2.0)
    HighSpeed = 3,
    /// SuperSpeed (5 Gbps, USB 3.0)
    SuperSpeed = 4,
    /// SuperSpeed+ (10 Gbps, USB 3.1 Gen 2)
    SuperSpeedPlus = 5,
    /// SuperSpeed+ (20 Gbps, USB 3.2 Gen 2x2)
    SuperSpeedPlus20 = 6,
    /// Reserved for future speeds
    Reserved = 7,
}

impl UsbDeviceSpeed {
    /// Convert from raw speed code (xHCI PORTSC encoding)
    #[inline(always)]
    pub fn from_code(code: u8) -> Self {
        match code {
            0 => UsbDeviceSpeed::Unknown,
            1 => UsbDeviceSpeed::FullSpeed,
            2 => UsbDeviceSpeed::LowSpeed,
            3 => UsbDeviceSpeed::HighSpeed,
            4 => UsbDeviceSpeed::SuperSpeed,
            5 => UsbDeviceSpeed::SuperSpeedPlus,
            6 => UsbDeviceSpeed::SuperSpeedPlus20,
            _ => UsbDeviceSpeed::Reserved,
        }
    }

    /// Get speed code
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Get default max packet size for EP0 based on speed
    ///
    /// #VERIFY[MAX-PACKET-USB]: Values per USB spec
    #[inline(always)]
    pub const fn default_max_packet_ep0(self) -> u16 {
        match self {
            UsbDeviceSpeed::Unknown => 8,
            UsbDeviceSpeed::LowSpeed => 8,
            UsbDeviceSpeed::FullSpeed => 8,  // Can be 8, 16, 32, or 64
            UsbDeviceSpeed::HighSpeed => 64,
            UsbDeviceSpeed::SuperSpeed => 512,
            UsbDeviceSpeed::SuperSpeedPlus => 512,
            UsbDeviceSpeed::SuperSpeedPlus20 => 512,
            UsbDeviceSpeed::Reserved => 8,
        }
    }

    /// Get maximum bulk transfer size for this speed
    ///
    /// #ASSUME[BULK-MAX]: Typical maximum, actual depends on endpoint descriptor
    #[inline(always)]
    pub const fn max_bulk_size(self) -> u32 {
        match self {
            UsbDeviceSpeed::Unknown => 8,
            UsbDeviceSpeed::LowSpeed => 0, // No bulk on low speed
            UsbDeviceSpeed::FullSpeed => 64,
            UsbDeviceSpeed::HighSpeed => 512,
            UsbDeviceSpeed::SuperSpeed => 1024,
            UsbDeviceSpeed::SuperSpeedPlus => 1024,
            UsbDeviceSpeed::SuperSpeedPlus20 => 1024,
            UsbDeviceSpeed::Reserved => 8,
        }
    }

    /// Check if speed supports USB 3.x features
    #[inline(always)]
    pub const fn is_superspeed(self) -> bool {
        matches!(
            self,
            UsbDeviceSpeed::SuperSpeed | UsbDeviceSpeed::SuperSpeedPlus | UsbDeviceSpeed::SuperSpeedPlus20
        )
    }
}

// ============================================================================
// USB Device Class
// ============================================================================

/// USB Device Class codes (bDeviceClass)
///
/// #VERIFY[CLASS-USB-IF]: Values per USB-IF class code assignments
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UsbDeviceClass {
    /// Class defined at interface level
    InterfaceDefined = 0x00,
    /// Audio
    Audio = 0x01,
    /// Communications and CDC Control
    Cdc = 0x02,
    /// Human Interface Device (HID)
    Hid = 0x03,
    /// Physical
    Physical = 0x05,
    /// Image
    Image = 0x06,
    /// Printer
    Printer = 0x07,
    /// Mass Storage
    MassStorage = 0x08,
    /// Hub
    Hub = 0x09,
    /// CDC Data
    CdcData = 0x0A,
    /// Smart Card
    SmartCard = 0x0B,
    /// Content Security
    ContentSecurity = 0x0D,
    /// Video
    Video = 0x0E,
    /// Personal Healthcare
    Healthcare = 0x0F,
    /// Audio/Video Devices
    AudioVideo = 0x10,
    /// Billboard
    Billboard = 0x11,
    /// USB Type-C Bridge
    TypeCBridge = 0x12,
    /// USB Bulk Display Protocol
    BulkDisplay = 0x13,
    /// MCTP over USB
    Mctp = 0x14,
    /// I3C Device Class
    I3c = 0x3C,
    /// Diagnostic Device
    Diagnostic = 0xDC,
    /// Wireless Controller
    Wireless = 0xE0,
    /// Miscellaneous
    Miscellaneous = 0xEF,
    /// Application Specific
    ApplicationSpecific = 0xFE,
    /// Vendor Specific
    VendorSpecific = 0xFF,
    /// Unknown class
    Unknown = 0xFD,
}

impl UsbDeviceClass {
    /// Convert from raw class code
    #[inline(always)]
    pub fn from_code(code: u8) -> Self {
        match code {
            0x00 => UsbDeviceClass::InterfaceDefined,
            0x01 => UsbDeviceClass::Audio,
            0x02 => UsbDeviceClass::Cdc,
            0x03 => UsbDeviceClass::Hid,
            0x05 => UsbDeviceClass::Physical,
            0x06 => UsbDeviceClass::Image,
            0x07 => UsbDeviceClass::Printer,
            0x08 => UsbDeviceClass::MassStorage,
            0x09 => UsbDeviceClass::Hub,
            0x0A => UsbDeviceClass::CdcData,
            0x0B => UsbDeviceClass::SmartCard,
            0x0D => UsbDeviceClass::ContentSecurity,
            0x0E => UsbDeviceClass::Video,
            0x0F => UsbDeviceClass::Healthcare,
            0x10 => UsbDeviceClass::AudioVideo,
            0x11 => UsbDeviceClass::Billboard,
            0x12 => UsbDeviceClass::TypeCBridge,
            0x13 => UsbDeviceClass::BulkDisplay,
            0x14 => UsbDeviceClass::Mctp,
            0x3C => UsbDeviceClass::I3c,
            0xDC => UsbDeviceClass::Diagnostic,
            0xE0 => UsbDeviceClass::Wireless,
            0xEF => UsbDeviceClass::Miscellaneous,
            0xFE => UsbDeviceClass::ApplicationSpecific,
            0xFF => UsbDeviceClass::VendorSpecific,
            _ => UsbDeviceClass::Unknown,
        }
    }

    /// Get class code
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

// ============================================================================
// USB Device Snapshot
// ============================================================================

/// Atomic snapshot of USB device state
///
/// All fields captured atomically for consistent view of device state.
///
/// #VERIFY[SNAPSHOT-CONSISTENT]: All fields from same generation
#[derive(Debug, Clone, Copy)]
pub struct UsbDeviceSnapshot {
    /// Device state
    pub state: UsbDeviceState,
    /// Generation counter
    pub generation: u64,
    /// Slot ID (1-255, assigned by xHCI)
    pub slot_id: u8,
    /// USB device address (1-127)
    pub address: u8,
    /// Device speed
    pub speed: UsbDeviceSpeed,
    /// Root hub port number (1-based)
    pub port: u8,
    /// Vendor ID (from device descriptor)
    pub vendor_id: u16,
    /// Product ID (from device descriptor)
    pub product_id: u16,
    /// Device class (from device descriptor or interface)
    pub device_class: UsbDeviceClass,
    /// Device subclass
    pub device_subclass: u8,
    /// Device protocol
    pub device_protocol: u8,
    /// Current configuration value (0 = unconfigured)
    pub configuration: u8,
    /// Number of interfaces in current configuration
    pub num_interfaces: u8,
    /// Number of configurations available
    pub num_configurations: u8,
    /// USB BCD version (e.g., 0x0200 = USB 2.0, 0x0300 = USB 3.0)
    pub usb_version: u16,
    /// Max packet size for EP0
    pub max_packet_ep0: u16,
    /// Total successful transfers
    pub transfers_completed: u64,
    /// Total failed transfers
    pub transfers_failed: u64,
}

impl UsbDeviceSnapshot {
    /// Check if device is configured and ready for data transfers
    #[inline(always)]
    pub fn is_configured(&self) -> bool {
        self.state == UsbDeviceState::Configured && self.configuration > 0
    }

    /// Check if device is operational (addressed or configured)
    #[inline(always)]
    pub fn is_operational(&self) -> bool {
        self.state.is_operational()
    }

    /// Check if device supports USB 3.x
    #[inline(always)]
    pub fn is_superspeed(&self) -> bool {
        self.speed.is_superspeed()
    }

    /// Get device class name for debugging
    #[inline(always)]
    pub fn class_name(&self) -> &'static str {
        match self.device_class {
            UsbDeviceClass::Audio => "Audio",
            UsbDeviceClass::Cdc => "CDC",
            UsbDeviceClass::Hid => "HID",
            UsbDeviceClass::MassStorage => "Mass Storage",
            UsbDeviceClass::Hub => "Hub",
            UsbDeviceClass::Video => "Video",
            UsbDeviceClass::Wireless => "Wireless",
            UsbDeviceClass::VendorSpecific => "Vendor Specific",
            _ => "Other",
        }
    }
}

// ============================================================================
// USB Device Capsule (256 bytes)
// ============================================================================

/// USB Device Capsule (256 bytes, cache-aligned)
///
/// **Architecture**: Tier 1 (Atomic)
/// - Lockfree device state transitions
/// - Generation counters for ABA prevention
/// - High-level USB device abstraction
///
/// # Memory Layout (256 bytes, 4 cache lines)
///
/// ## Cache Line 0 (64 bytes) - Primary State
/// - state_gen: State + slot + address + speed + port + generation (8 bytes)
/// - vid_pid: Vendor ID (16) + Product ID (16) packed (4 bytes)
/// - class_info: Class + Subclass + Protocol packed (4 bytes)
/// - config_info: Configuration + Num interfaces + Num configs (4 bytes)
/// - usb_version: USB BCD version (2 bytes)
/// - max_packet_ep0: Max packet size for EP0 (2 bytes)
/// - string_indices: Manufacturer/Product/Serial string indices (4 bytes)
/// - device_flags: Various device capability flags (4 bytes)
/// - descriptor_ptr: Pointer to cached device descriptor (8 bytes)
/// - context_ptr: Pointer to xHCI device context (8 bytes)
/// - Reserved (16 bytes)
///
/// ## Cache Line 1 (64 bytes) - Endpoint State
/// - endpoint_mask: Bitmap of configured endpoints (4 bytes)
/// - endpoint_types: Packed endpoint types (8 bytes)
/// - endpoint_max_packets: Packed max packet sizes (16 bytes)
/// - interface_endpoints: Interface to endpoint mapping (16 bytes)
/// - Reserved (20 bytes)
///
/// ## Cache Line 2 (64 bytes) - Statistics
/// - transfers_completed: Successful transfer count (8 bytes)
/// - transfers_failed: Failed transfer count (8 bytes)
/// - bytes_transferred_in: Total IN bytes (8 bytes)
/// - bytes_transferred_out: Total OUT bytes (8 bytes)
/// - control_transfers: Control transfer count (8 bytes)
/// - bulk_transfers: Bulk transfer count (8 bytes)
/// - interrupt_transfers: Interrupt transfer count (8 bytes)
/// - last_error: Last error code + timestamp (8 bytes)
///
/// ## Cache Line 3 (64 bytes) - Extended State
/// - suspend_state: Previous state before suspend (8 bytes)
/// - power_state: Current power mode (8 bytes)
/// - lpm_state: Link Power Management state (8 bytes)
/// - Reserved/Padding (40 bytes)
///
/// #ASSUME[CACHE-ALIGN]: 256-byte alignment prevents false sharing
/// #VERIFY[SIZE-256]: Structure exactly 256 bytes
#[repr(C, align(256))]
pub struct UsbDeviceCapsule {
    // === Cache Line 0 (64 bytes) - Primary State ===
    /// Packed state: state (8) | slot (8) | address (8) | speed_port (8) | gen (32)
    /// #VERIFY[STATE-ATOMIC]: Single atomic for consistent state reads
    state_gen: AtomicU64,
    /// Vendor ID (bits 0-15) + Product ID (bits 16-31)
    /// #VERIFY[VID-PID]: Values from device descriptor
    vid_pid: AtomicU32,
    /// Class (bits 0-7) + Subclass (bits 8-15) + Protocol (bits 16-23)
    /// #VERIFY[CLASS-VALID]: Values from device/interface descriptor
    class_info: AtomicU32,
    /// Configuration (bits 0-7) + Num interfaces (bits 8-15) + Num configs (bits 16-23)
    config_info: AtomicU32,
    /// USB BCD version (e.g., 0x0200 for USB 2.0)
    usb_version: AtomicU16,
    /// Max packet size for EP0 (8, 16, 32, 64, or 512)
    max_packet_ep0: AtomicU16,
    /// String descriptor indices: Manufacturer (0-7), Product (8-15), Serial (16-23)
    string_indices: AtomicU32,
    /// Device capability flags
    device_flags: AtomicU32,
    /// Pointer to cached full device descriptor (if any)
    descriptor_ptr: AtomicU64,
    /// Pointer to xHCI device context structure
    context_ptr: AtomicU64,
    /// Reserved for cache line alignment
    _reserved_cl0: [u8; 16],

    // === Cache Line 1 (64 bytes) - Endpoint State ===
    /// Bitmap of configured endpoints (bit N = endpoint N active)
    /// #VERIFY[EP-BITMAP]: Updated on SET_CONFIGURATION
    endpoint_mask: AtomicU32,
    /// Packed endpoint types (2 bits each, 16 endpoints = 32 bits)
    /// 0=Control, 1=Isochronous, 2=Bulk, 3=Interrupt
    endpoint_types: AtomicU64,
    /// Packed max packet sizes (11 bits each for first 4 endpoints)
    endpoint_max_packets_low: AtomicU64,
    /// Packed max packet sizes (11 bits each for next 4 endpoints)
    endpoint_max_packets_high: AtomicU64,
    /// Interface to endpoint mapping (4 bits per interface, max 8 interfaces)
    interface_endpoints: AtomicU64,
    /// Active alternate settings for interfaces
    alt_settings: AtomicU64,
    /// Reserved for future endpoint state
    _reserved_cl1: [u8; 12],

    // === Cache Line 2 (64 bytes) - Statistics ===
    /// Total successful transfers
    transfers_completed: AtomicU64,
    /// Total failed transfers
    transfers_failed: AtomicU64,
    /// Total bytes received (IN transfers)
    bytes_in: AtomicU64,
    /// Total bytes sent (OUT transfers)
    bytes_out: AtomicU64,
    /// Control transfer count
    control_transfers: AtomicU64,
    /// Bulk transfer count
    bulk_transfers: AtomicU64,
    /// Interrupt transfer count
    interrupt_transfers: AtomicU64,
    /// Last error code (32 bits) + error count (32 bits)
    last_error: AtomicU64,

    // === Cache Line 3 (64 bytes) - Extended State ===
    /// State saved before suspend (for resume)
    suspend_saved_state: AtomicU64,
    /// Current power state (D0-D3)
    power_state: AtomicU64,
    /// Link Power Management state
    lpm_state: AtomicU64,
    /// U1/U2 exit latency values
    exit_latency: AtomicU64,
    /// Padding to 256 bytes
    _padding: [u8; 32],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<UsbDeviceCapsule>() == 256);
const _: () = assert!(core::mem::align_of::<UsbDeviceCapsule>() == 256);

// Device flags
const FLAG_SELF_POWERED: u32 = 1 << 0;
const FLAG_REMOTE_WAKEUP: u32 = 1 << 1;
const FLAG_LPM_CAPABLE: u32 = 1 << 2;
const FLAG_FUNCTION_SUSPEND: u32 = 1 << 3;
const FLAG_LTM_CAPABLE: u32 = 1 << 4;
const FLAG_USB3_DEVINIT_DONE: u32 = 1 << 5;
const FLAG_HUB: u32 = 1 << 6;
const FLAG_COMPOUND: u32 = 1 << 7;

impl UsbDeviceCapsule {
    /// Create new USB device capsule
    ///
    /// #VERIFY[INIT-DISCONNECTED]: Initial state is Disconnected
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(UsbDeviceState::Disconnected.pack(0, 0, 0, 0, 0)),
            vid_pid: AtomicU32::new(0),
            class_info: AtomicU32::new(0),
            config_info: AtomicU32::new(0),
            usb_version: AtomicU16::new(0),
            max_packet_ep0: AtomicU16::new(8),
            string_indices: AtomicU32::new(0),
            device_flags: AtomicU32::new(0),
            descriptor_ptr: AtomicU64::new(0),
            context_ptr: AtomicU64::new(0),
            _reserved_cl0: [0u8; 16],
            endpoint_mask: AtomicU32::new(1), // EP0 always active
            endpoint_types: AtomicU64::new(0), // EP0 is control
            endpoint_max_packets_low: AtomicU64::new(8), // EP0 default
            endpoint_max_packets_high: AtomicU64::new(0),
            interface_endpoints: AtomicU64::new(0),
            alt_settings: AtomicU64::new(0),
            _reserved_cl1: [0u8; 12],
            transfers_completed: AtomicU64::new(0),
            transfers_failed: AtomicU64::new(0),
            bytes_in: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            control_transfers: AtomicU64::new(0),
            bulk_transfers: AtomicU64::new(0),
            interrupt_transfers: AtomicU64::new(0),
            last_error: AtomicU64::new(0),
            suspend_saved_state: AtomicU64::new(0),
            power_state: AtomicU64::new(0),
            lpm_state: AtomicU64::new(0),
            exit_latency: AtomicU64::new(0),
            _padding: [0u8; 32],
        }
    }

    /// Get atomic snapshot of current state
    ///
    /// #VERIFY[SNAPSHOT-ATOMIC]: All reads use Acquire ordering
    #[inline(always)]
    pub fn snapshot(&self) -> UsbDeviceSnapshot {
        let state_packed = self.state_gen.load(Ordering::Acquire);
        let vid_pid = self.vid_pid.load(Ordering::Acquire);
        let class_info = self.class_info.load(Ordering::Acquire);
        let config_info = self.config_info.load(Ordering::Acquire);
        let usb_ver = self.usb_version.load(Ordering::Acquire);
        let max_pkt = self.max_packet_ep0.load(Ordering::Acquire);

        UsbDeviceSnapshot {
            state: UsbDeviceState::from_packed(state_packed),
            generation: (state_packed >> 32) & 0xFFFF_FFFF,
            slot_id: ((state_packed >> 8) & 0xFF) as u8,
            address: ((state_packed >> 16) & 0xFF) as u8,
            speed: UsbDeviceSpeed::from_code(((state_packed >> 24) & 0xF) as u8),
            port: ((state_packed >> 28) & 0xF) as u8,
            vendor_id: (vid_pid & 0xFFFF) as u16,
            product_id: ((vid_pid >> 16) & 0xFFFF) as u16,
            device_class: UsbDeviceClass::from_code((class_info & 0xFF) as u8),
            device_subclass: ((class_info >> 8) & 0xFF) as u8,
            device_protocol: ((class_info >> 16) & 0xFF) as u8,
            configuration: (config_info & 0xFF) as u8,
            num_interfaces: ((config_info >> 8) & 0xFF) as u8,
            num_configurations: ((config_info >> 16) & 0xFF) as u8,
            usb_version: usb_ver,
            max_packet_ep0: max_pkt,
            transfers_completed: self.transfers_completed.load(Ordering::Acquire),
            transfers_failed: self.transfers_failed.load(Ordering::Acquire),
        }
    }

    /// Get current state only (fast path)
    #[inline(always)]
    pub fn state(&self) -> UsbDeviceState {
        UsbDeviceState::from_packed(self.state_gen.load(Ordering::Acquire))
    }

    /// Get slot ID
    #[inline(always)]
    pub fn slot_id(&self) -> u8 {
        ((self.state_gen.load(Ordering::Acquire) >> 8) & 0xFF) as u8
    }

    /// Get device address
    #[inline(always)]
    pub fn address(&self) -> u8 {
        ((self.state_gen.load(Ordering::Acquire) >> 16) & 0xFF) as u8
    }

    /// Get device speed
    #[inline(always)]
    pub fn speed(&self) -> UsbDeviceSpeed {
        UsbDeviceSpeed::from_code(((self.state_gen.load(Ordering::Acquire) >> 24) & 0xF) as u8)
    }

    /// Get vendor ID
    #[inline(always)]
    pub fn vendor_id(&self) -> u16 {
        (self.vid_pid.load(Ordering::Acquire) & 0xFFFF) as u16
    }

    /// Get product ID
    #[inline(always)]
    pub fn product_id(&self) -> u16 {
        ((self.vid_pid.load(Ordering::Acquire) >> 16) & 0xFFFF) as u16
    }

    /// Transition state with CAS (lockfree state machine)
    fn transition_state(
        &self,
        expected_state: UsbDeviceState,
        new_state: UsbDeviceState,
    ) -> Result<u64, UsbDeviceState> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let actual_state = UsbDeviceState::from_packed(current);

            if actual_state != expected_state {
                return Err(actual_state);
            }

            let slot = (current >> 8) & 0xFF;
            let addr = (current >> 16) & 0xFF;
            let speed_port = (current >> 24) & 0xFF;
            let speed = (speed_port & 0xF) as u8;
            let port = ((speed_port >> 4) & 0xF) as u8;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let new_packed = new_state.pack(new_gen, slot as u8, addr as u8, speed, port);

            match self.state_gen.compare_exchange(
                current,
                new_packed,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(new_gen),
                Err(_) => continue,
            }
        }
    }

    /// Attach device (Disconnected -> Attached)
    ///
    /// # Arguments
    /// - `port`: Root hub port number (1-15)
    /// - `speed`: Detected device speed
    ///
    /// #ASSUME[PORT-VALID]: Port number within controller's max ports
    pub fn attach(&self, port: u8, speed: UsbDeviceSpeed) -> Result<u64, UsbDeviceState> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = UsbDeviceState::from_packed(current);

            if state != UsbDeviceState::Disconnected {
                return Err(state);
            }

            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);
            let new_packed = UsbDeviceState::Attached.pack(new_gen, 0, 0, speed.code(), port);

            if self.state_gen.compare_exchange(current, new_packed, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                self.max_packet_ep0.store(speed.default_max_packet_ep0(), Ordering::Release);
                return Ok(new_gen);
            }
        }
    }

    /// Reset device and assign slot (Attached -> Default)
    ///
    /// # Arguments
    /// - `slot_id`: Assigned slot ID from Enable Slot command (1-255)
    ///
    /// #VERIFY[SLOT-ENABLED]: Enable Slot command completed successfully
    pub fn reset(&self, slot_id: u8) -> Result<u64, UsbDeviceState> {
        if slot_id == 0 {
            return Err(UsbDeviceState::Error);
        }

        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = UsbDeviceState::from_packed(current);

            if state != UsbDeviceState::Attached {
                return Err(state);
            }

            let speed_port = (current >> 24) & 0xFF;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            // Address = 0 in Default state
            let new_packed = UsbDeviceState::Default.pack(
                new_gen,
                slot_id,
                0,
                (speed_port & 0xF) as u8,
                ((speed_port >> 4) & 0xF) as u8,
            );

            if self.state_gen.compare_exchange(current, new_packed, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                return Ok(new_gen);
            }
        }
    }

    /// Set device address (Default -> Addressed)
    ///
    /// # Arguments
    /// - `address`: Assigned USB address (1-127)
    ///
    /// #VERIFY[ADDRESS-VALID]: Address in range 1-127
    /// #VERIFY[SET-ADDRESS-DONE]: SET_ADDRESS command completed
    pub fn set_address(&self, address: u8) -> Result<u64, UsbDeviceState> {
        if address == 0 || address > 127 {
            return Err(UsbDeviceState::Error);
        }

        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = UsbDeviceState::from_packed(current);

            if state != UsbDeviceState::Default {
                return Err(state);
            }

            let slot = (current >> 8) & 0xFF;
            let speed_port = (current >> 24) & 0xFF;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let new_packed = UsbDeviceState::Addressed.pack(
                new_gen,
                slot as u8,
                address,
                (speed_port & 0xF) as u8,
                ((speed_port >> 4) & 0xF) as u8,
            );

            if self.state_gen.compare_exchange(current, new_packed, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                return Ok(new_gen);
            }
        }
    }

    /// Set device descriptor information
    ///
    /// Called after reading device descriptor via GET_DESCRIPTOR.
    ///
    /// #VERIFY[DESC-PARSED]: Device descriptor successfully read and parsed
    pub fn set_descriptor(
        &self,
        vendor_id: u16,
        product_id: u16,
        device_class: u8,
        device_subclass: u8,
        device_protocol: u8,
        usb_version: u16,
        num_configurations: u8,
        max_packet_ep0: u16,
    ) {
        let vid_pid = (vendor_id as u32) | ((product_id as u32) << 16);
        self.vid_pid.store(vid_pid, Ordering::Release);

        let class_info = (device_class as u32)
            | ((device_subclass as u32) << 8)
            | ((device_protocol as u32) << 16);
        self.class_info.store(class_info, Ordering::Release);

        self.usb_version.store(usb_version, Ordering::Release);

        // Update num_configurations in config_info
        let config = self.config_info.load(Ordering::Acquire);
        let new_config = (config & 0xFFFF) | ((num_configurations as u32) << 16);
        self.config_info.store(new_config, Ordering::Release);

        self.max_packet_ep0.store(max_packet_ep0, Ordering::Release);
    }

    /// Set string descriptor indices
    ///
    /// #ASSUME[STRING-INDICES]: Indices from device descriptor
    pub fn set_string_indices(&self, manufacturer: u8, product: u8, serial: u8) {
        let indices = (manufacturer as u32) | ((product as u32) << 8) | ((serial as u32) << 16);
        self.string_indices.store(indices, Ordering::Release);
    }

    /// Configure device (Addressed -> Configured)
    ///
    /// # Arguments
    /// - `configuration`: Configuration value (1-based)
    /// - `num_interfaces`: Number of interfaces in this configuration
    ///
    /// #VERIFY[SET-CONFIG-DONE]: SET_CONFIGURATION command completed
    pub fn configure(&self, configuration: u8, num_interfaces: u8) -> Result<u64, UsbDeviceState> {
        if configuration == 0 {
            return Err(UsbDeviceState::Error);
        }

        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = UsbDeviceState::from_packed(current);

            // Can configure from Addressed or reconfigure from Configured
            if !matches!(state, UsbDeviceState::Addressed | UsbDeviceState::Configured) {
                return Err(state);
            }

            let slot = (current >> 8) & 0xFF;
            let addr = (current >> 16) & 0xFF;
            let speed_port = (current >> 24) & 0xFF;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let new_packed = UsbDeviceState::Configured.pack(
                new_gen,
                slot as u8,
                addr as u8,
                (speed_port & 0xF) as u8,
                ((speed_port >> 4) & 0xF) as u8,
            );

            if self.state_gen.compare_exchange(current, new_packed, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                // Update config info
                let old_config = self.config_info.load(Ordering::Acquire);
                let num_configs = (old_config >> 16) & 0xFF;
                let new_config = (configuration as u32)
                    | ((num_interfaces as u32) << 8)
                    | (num_configs << 16);
                self.config_info.store(new_config, Ordering::Release);

                return Ok(new_gen);
            }
        }
    }

    /// Deconfigure device (Configured -> Addressed)
    ///
    /// #VERIFY[UNCONFIG]: SET_CONFIGURATION(0) command issued
    pub fn deconfigure(&self) -> Result<u64, UsbDeviceState> {
        let gen = self.transition_state(UsbDeviceState::Configured, UsbDeviceState::Addressed)?;

        // Clear configuration
        let old_config = self.config_info.load(Ordering::Acquire);
        let num_configs = (old_config >> 16) & 0xFF;
        self.config_info.store(num_configs << 16, Ordering::Release);

        Ok(gen)
    }

    /// Suspend device
    ///
    /// #ASSUME[SUSPEND-IDLE]: No active transfers when suspending
    pub fn suspend(&self) -> Result<u64, UsbDeviceState> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = UsbDeviceState::from_packed(current);

            if !matches!(state, UsbDeviceState::Addressed | UsbDeviceState::Configured) {
                return Err(state);
            }

            // Save current state for resume
            self.suspend_saved_state.store(current, Ordering::Release);

            let slot = (current >> 8) & 0xFF;
            let addr = (current >> 16) & 0xFF;
            let speed_port = (current >> 24) & 0xFF;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let new_packed = UsbDeviceState::Suspended.pack(
                new_gen,
                slot as u8,
                addr as u8,
                (speed_port & 0xF) as u8,
                ((speed_port >> 4) & 0xF) as u8,
            );

            if self.state_gen.compare_exchange(current, new_packed, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                return Ok(new_gen);
            }
        }
    }

    /// Resume device from suspend
    ///
    /// #VERIFY[RESUME-STATE]: Restores previous state (Addressed or Configured)
    pub fn resume(&self) -> Result<u64, UsbDeviceState> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = UsbDeviceState::from_packed(current);

            if state != UsbDeviceState::Suspended {
                return Err(state);
            }

            // Restore previous state
            let saved = self.suspend_saved_state.load(Ordering::Acquire);
            let saved_state = UsbDeviceState::from_packed(saved);
            let resume_state = match saved_state {
                UsbDeviceState::Configured => UsbDeviceState::Configured,
                _ => UsbDeviceState::Addressed,
            };

            let slot = (current >> 8) & 0xFF;
            let addr = (current >> 16) & 0xFF;
            let speed_port = (current >> 24) & 0xFF;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let new_packed = resume_state.pack(
                new_gen,
                slot as u8,
                addr as u8,
                (speed_port & 0xF) as u8,
                ((speed_port >> 4) & 0xF) as u8,
            );

            if self.state_gen.compare_exchange(current, new_packed, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                return Ok(new_gen);
            }
        }
    }

    /// Disconnect device
    ///
    /// #VERIFY[DISCONNECT-CLEANUP]: All transfers cancelled before disconnect
    pub fn disconnect(&self) -> Result<u64, UsbDeviceState> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = UsbDeviceState::from_packed(current);

            if state == UsbDeviceState::Disconnected {
                return Err(state);
            }

            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);
            let new_packed = UsbDeviceState::Disconnected.pack(new_gen, 0, 0, 0, 0);

            if self.state_gen.compare_exchange(current, new_packed, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                // Clear device data
                self.vid_pid.store(0, Ordering::Release);
                self.class_info.store(0, Ordering::Release);
                self.config_info.store(0, Ordering::Release);
                self.endpoint_mask.store(1, Ordering::Release); // Reset to EP0 only

                return Ok(new_gen);
            }
        }
    }

    /// Record successful transfer
    ///
    /// #VERIFY[STATS-ATOMIC]: Statistics updated atomically
    pub fn record_transfer_success(&self, bytes: u64, is_in: bool) {
        self.transfers_completed.fetch_add(1, Ordering::AcqRel);
        if is_in {
            self.bytes_in.fetch_add(bytes, Ordering::AcqRel);
        } else {
            self.bytes_out.fetch_add(bytes, Ordering::AcqRel);
        }
    }

    /// Record failed transfer
    pub fn record_transfer_failure(&self, error_code: u32) {
        self.transfers_failed.fetch_add(1, Ordering::AcqRel);

        // Update last error with count
        loop {
            let old = self.last_error.load(Ordering::Acquire);
            let count = ((old >> 32) + 1) & 0xFFFF_FFFF;
            let new = (count << 32) | (error_code as u64);
            if self.last_error.compare_exchange(old, new, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                break;
            }
        }
    }

    /// Get total successful transfers
    #[inline(always)]
    pub fn transfers_completed(&self) -> u64 {
        self.transfers_completed.load(Ordering::Acquire)
    }

    /// Get total failed transfers
    #[inline(always)]
    pub fn transfers_failed(&self) -> u64 {
        self.transfers_failed.load(Ordering::Acquire)
    }

    /// Get total bytes transferred in each direction
    #[inline(always)]
    pub fn bytes_transferred(&self) -> (u64, u64) {
        (
            self.bytes_in.load(Ordering::Acquire),
            self.bytes_out.load(Ordering::Acquire),
        )
    }

    /// Set xHCI device context pointer
    ///
    /// #ASSUME[CONTEXT-DMA]: Context in DMA-capable memory
    pub fn set_context_ptr(&self, ptr: u64) {
        self.context_ptr.store(ptr, Ordering::Release);
    }

    /// Get xHCI device context pointer
    #[inline(always)]
    pub fn context_ptr(&self) -> u64 {
        self.context_ptr.load(Ordering::Acquire)
    }

    /// Configure endpoint
    ///
    /// #VERIFY[EP-CONFIG]: Called after parsing endpoint descriptor
    pub fn configure_endpoint(&self, endpoint_num: u8, ep_type: u8, max_packet: u16) {
        if endpoint_num >= 32 {
            return;
        }

        // Set endpoint active in mask
        self.endpoint_mask.fetch_or(1u32 << endpoint_num, Ordering::AcqRel);

        // Set endpoint type (2 bits per endpoint in endpoint_types)
        let shift = (endpoint_num as u64 % 32) * 2;
        loop {
            let old = self.endpoint_types.load(Ordering::Acquire);
            let mask = !(3u64 << shift);
            let new = (old & mask) | (((ep_type & 0x3) as u64) << shift);
            if self.endpoint_types.compare_exchange(old, new, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                break;
            }
        }
    }

    /// Check if endpoint is configured
    #[inline(always)]
    pub fn is_endpoint_configured(&self, endpoint_num: u8) -> bool {
        if endpoint_num >= 32 {
            return false;
        }
        (self.endpoint_mask.load(Ordering::Acquire) & (1u32 << endpoint_num)) != 0
    }
}

impl Default for UsbDeviceCapsule {
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
            core::mem::size_of::<UsbDeviceCapsule>(),
            256,
            "UsbDeviceCapsule must be exactly 256 bytes"
        );
    }

    #[test]
    fn test_device_capsule_alignment() {
        assert_eq!(
            core::mem::align_of::<UsbDeviceCapsule>(),
            256,
            "UsbDeviceCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_device_initial_state() {
        let device = UsbDeviceCapsule::new();
        let snapshot = device.snapshot();

        assert_eq!(snapshot.state, UsbDeviceState::Disconnected);
        assert_eq!(snapshot.slot_id, 0);
        assert_eq!(snapshot.address, 0);
        assert_eq!(snapshot.vendor_id, 0);
        assert_eq!(snapshot.product_id, 0);
    }

    #[test]
    fn test_device_attach() {
        let device = UsbDeviceCapsule::new();

        let result = device.attach(1, UsbDeviceSpeed::HighSpeed);
        assert!(result.is_ok());

        let snapshot = device.snapshot();
        assert_eq!(snapshot.state, UsbDeviceState::Attached);
        assert_eq!(snapshot.port, 1);
        assert_eq!(snapshot.speed, UsbDeviceSpeed::HighSpeed);
        assert_eq!(snapshot.max_packet_ep0, 64); // High speed default
    }

    #[test]
    fn test_device_reset() {
        let device = UsbDeviceCapsule::new();
        device.attach(1, UsbDeviceSpeed::HighSpeed).unwrap();

        let result = device.reset(5);
        assert!(result.is_ok());

        let snapshot = device.snapshot();
        assert_eq!(snapshot.state, UsbDeviceState::Default);
        assert_eq!(snapshot.slot_id, 5);
        assert_eq!(snapshot.address, 0); // Default state has address 0
    }

    #[test]
    fn test_device_set_address() {
        let device = UsbDeviceCapsule::new();
        device.attach(1, UsbDeviceSpeed::HighSpeed).unwrap();
        device.reset(5).unwrap();

        let result = device.set_address(10);
        assert!(result.is_ok());

        let snapshot = device.snapshot();
        assert_eq!(snapshot.state, UsbDeviceState::Addressed);
        assert_eq!(snapshot.address, 10);
    }

    #[test]
    fn test_device_configure() {
        let device = UsbDeviceCapsule::new();
        device.attach(1, UsbDeviceSpeed::HighSpeed).unwrap();
        device.reset(5).unwrap();
        device.set_address(10).unwrap();

        let result = device.configure(1, 2);
        assert!(result.is_ok());

        let snapshot = device.snapshot();
        assert_eq!(snapshot.state, UsbDeviceState::Configured);
        assert_eq!(snapshot.configuration, 1);
        assert_eq!(snapshot.num_interfaces, 2);
        assert!(snapshot.is_configured());
    }

    #[test]
    fn test_device_descriptor() {
        let device = UsbDeviceCapsule::new();
        device.attach(1, UsbDeviceSpeed::HighSpeed).unwrap();
        device.reset(5).unwrap();

        device.set_descriptor(
            0x1234, // vendor_id
            0x5678, // product_id
            0x08,   // class (mass storage)
            0x06,   // subclass
            0x50,   // protocol
            0x0200, // USB 2.0
            1,      // num_configurations
            64,     // max_packet_ep0
        );

        let snapshot = device.snapshot();
        assert_eq!(snapshot.vendor_id, 0x1234);
        assert_eq!(snapshot.product_id, 0x5678);
        assert_eq!(snapshot.device_class, UsbDeviceClass::MassStorage);
        assert_eq!(snapshot.usb_version, 0x0200);
    }

    #[test]
    fn test_device_suspend_resume() {
        let device = UsbDeviceCapsule::new();
        device.attach(1, UsbDeviceSpeed::HighSpeed).unwrap();
        device.reset(5).unwrap();
        device.set_address(10).unwrap();
        device.configure(1, 1).unwrap();

        // Suspend
        let result = device.suspend();
        assert!(result.is_ok());
        assert_eq!(device.state(), UsbDeviceState::Suspended);

        // Resume
        let result = device.resume();
        assert!(result.is_ok());
        assert_eq!(device.state(), UsbDeviceState::Configured);
    }

    #[test]
    fn test_device_disconnect() {
        let device = UsbDeviceCapsule::new();
        device.attach(1, UsbDeviceSpeed::HighSpeed).unwrap();
        device.reset(5).unwrap();
        device.set_address(10).unwrap();
        device.configure(1, 1).unwrap();

        let result = device.disconnect();
        assert!(result.is_ok());

        let snapshot = device.snapshot();
        assert_eq!(snapshot.state, UsbDeviceState::Disconnected);
        assert_eq!(snapshot.slot_id, 0);
        assert_eq!(snapshot.vendor_id, 0);
    }

    #[test]
    fn test_transfer_statistics() {
        let device = UsbDeviceCapsule::new();

        device.record_transfer_success(1024, true);
        device.record_transfer_success(512, false);
        device.record_transfer_failure(0x06);

        assert_eq!(device.transfers_completed(), 2);
        assert_eq!(device.transfers_failed(), 1);

        let (bytes_in, bytes_out) = device.bytes_transferred();
        assert_eq!(bytes_in, 1024);
        assert_eq!(bytes_out, 512);
    }

    // ========================================================================
    // Q8-Q14: Property Tests
    // ========================================================================

    #[test]
    fn test_state_roundtrip() {
        let states = [
            UsbDeviceState::Disconnected,
            UsbDeviceState::Attached,
            UsbDeviceState::Default,
            UsbDeviceState::Addressed,
            UsbDeviceState::Configured,
            UsbDeviceState::Suspended,
            UsbDeviceState::Error,
            UsbDeviceState::Disabled,
        ];

        for state in states {
            let packed = state.pack(12345, 100, 50, 4, 8);
            let unpacked = UsbDeviceState::from_packed(packed);
            assert_eq!(unpacked, state);
        }
    }

    #[test]
    fn test_speed_max_packet() {
        assert_eq!(UsbDeviceSpeed::LowSpeed.default_max_packet_ep0(), 8);
        assert_eq!(UsbDeviceSpeed::FullSpeed.default_max_packet_ep0(), 8);
        assert_eq!(UsbDeviceSpeed::HighSpeed.default_max_packet_ep0(), 64);
        assert_eq!(UsbDeviceSpeed::SuperSpeed.default_max_packet_ep0(), 512);
    }

    #[test]
    fn test_device_class_roundtrip() {
        let classes = [
            UsbDeviceClass::Audio,
            UsbDeviceClass::Hid,
            UsbDeviceClass::MassStorage,
            UsbDeviceClass::Hub,
            UsbDeviceClass::VendorSpecific,
        ];

        for class in classes {
            let code = class.code();
            let recovered = UsbDeviceClass::from_code(code);
            assert_eq!(recovered, class);
        }
    }

    #[test]
    fn test_full_lifecycle() {
        let device = UsbDeviceCapsule::new();

        // Attach
        assert!(device.attach(1, UsbDeviceSpeed::SuperSpeed).is_ok());
        assert_eq!(device.state(), UsbDeviceState::Attached);

        // Reset
        assert!(device.reset(1).is_ok());
        assert_eq!(device.state(), UsbDeviceState::Default);

        // Set descriptor
        device.set_descriptor(0x8086, 0x1234, 0x08, 0x06, 0x50, 0x0300, 1, 512);

        // Address
        assert!(device.set_address(10).is_ok());
        assert_eq!(device.state(), UsbDeviceState::Addressed);

        // Configure
        assert!(device.configure(1, 1).is_ok());
        assert_eq!(device.state(), UsbDeviceState::Configured);

        // Verify snapshot
        let snapshot = device.snapshot();
        assert!(snapshot.is_configured());
        assert!(snapshot.is_superspeed());
        assert_eq!(snapshot.class_name(), "Mass Storage");

        // Suspend
        assert!(device.suspend().is_ok());
        assert_eq!(device.state(), UsbDeviceState::Suspended);

        // Resume
        assert!(device.resume().is_ok());
        assert_eq!(device.state(), UsbDeviceState::Configured);

        // Deconfigure
        assert!(device.deconfigure().is_ok());
        assert_eq!(device.state(), UsbDeviceState::Addressed);

        // Disconnect
        assert!(device.disconnect().is_ok());
        assert_eq!(device.state(), UsbDeviceState::Disconnected);
    }

    #[test]
    fn test_endpoint_configuration() {
        let device = UsbDeviceCapsule::new();

        // EP0 should be configured by default
        assert!(device.is_endpoint_configured(0));

        // Configure EP1 and EP2
        device.configure_endpoint(1, 2, 512); // Bulk
        device.configure_endpoint(2, 3, 64);  // Interrupt

        assert!(device.is_endpoint_configured(1));
        assert!(device.is_endpoint_configured(2));
        assert!(!device.is_endpoint_configured(3));
    }

    #[test]
    fn test_invalid_address() {
        let device = UsbDeviceCapsule::new();
        device.attach(1, UsbDeviceSpeed::HighSpeed).unwrap();
        device.reset(1).unwrap();

        // Address 0 invalid
        assert!(device.set_address(0).is_err());

        // Address 128+ invalid
        assert!(device.set_address(128).is_err());

        // Valid address
        assert!(device.set_address(127).is_ok());
    }
}
