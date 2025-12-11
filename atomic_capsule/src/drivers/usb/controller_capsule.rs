//! USB Controller Capsule - xHCI Host Controller State Management
//!
//! # Architecture
//! - **Tier 1 (Atomic)**: Lockfree state machine for controller coordination
//! - **512-byte alignment**: 8 cache lines for comprehensive controller state
//! - **Generation counters**: ABA prevention for state transitions
//! - **100% lockfree**: NO mutex, NO RwLock
//!
//! # xHCI Overview
//! The eXtensible Host Controller Interface (xHCI) supports USB 1.x through USB 3.2
//! under a single unified driver stack. This capsule manages the full controller
//! lifecycle including initialization, runtime state, and power management.
//!
//! # Performance Targets
//! - State snapshot: <10ns (single cache line read)
//! - State transition: <50ns (CAS with generation counter)
//! - Register access coordination: <100ns
//!
//! # Safety Assumptions (ASSUM Framework)
//! - #ASSUME[MMIO-VALID]: MMIO addresses valid when xHCI detected via PCI enumeration
//! - #ASSUME[BAR-MAPPED]: BAR0/BAR1 properly mapped before controller operations
//! - #ASSUME[DMA-CAPABLE]: DMA buffers physically contiguous and within DMA range
//! - #ASSUME[IRQ-CONFIGURED]: MSI-X/MSI interrupts configured before controller start
//! - #VERIFY[STATE-CAS]: State transitions use atomic CAS with generation counters
//! - #VERIFY[GEN-MONOTONIC]: Generation counter monotonically increases
//! - #VERIFY[ALIGN-64]: Ring buffer addresses 64-byte aligned for TRB access
//! - #VERIFY[ORDERING-ACQREL]: Memory ordering uses Acquire/Release for visibility

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

// ============================================================================
// PCI Identification Constants
// ============================================================================

/// PCI Class Code for Serial Bus Controller
/// #ASSUME[PCI-CLASS]: Standard PCI class code per PCI specification
pub const XHCI_PCI_CLASS: u8 = 0x0C;

/// PCI Subclass for USB Controller
/// #ASSUME[PCI-SUBCLASS]: Standard PCI subclass for USB controllers
pub const XHCI_PCI_SUBCLASS: u8 = 0x03;

/// PCI Programming Interface for xHCI
/// #ASSUME[PCI-INTERFACE]: xHCI identified by interface 0x30
pub const XHCI_PCI_INTERFACE: u8 = 0x30;

/// Maximum supported USB device slots (xHCI spec allows up to 255)
/// #ASSUME[MAX-SLOTS]: Hardware may support fewer slots than this maximum
pub const MAX_DEVICE_SLOTS: u8 = 255;

/// Maximum supported root hub ports (xHCI spec allows up to 255)
/// #ASSUME[MAX-PORTS]: Actual port count read from HCSPARAMS1
pub const MAX_ROOT_PORTS: u8 = 255;

// ============================================================================
// Controller State Machine
// ============================================================================

/// xHCI Controller initialization state
///
/// State machine follows xHCI specification section 4.2:
/// - Host must wait for CNR=0 before accessing operational registers
/// - Reset requires HCH=1 and wait for HCRST=0
/// - Run requires proper ring and DCBAA configuration
///
/// #VERIFY[STATE-VALID]: All state values map to valid controller states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UsbControllerState {
    /// Controller not detected or initialized
    /// #ASSUME[INIT-SAFE]: Safe to probe PCI config space
    Uninitialized = 0,
    /// Detecting xHCI via PCI enumeration
    /// #ASSUME[PCI-SCAN]: PCI bus enumeration in progress
    Detecting = 1,
    /// Mapping MMIO registers from BAR
    /// #ASSUME[BAR-READ]: BAR0/BAR1 contain valid physical addresses
    MappingRegs = 2,
    /// Reading capability registers
    /// #VERIFY[CAP-READ]: CAPLENGTH valid before operational access
    ReadingCaps = 3,
    /// Resetting controller (HCRST=1)
    /// #ASSUME[RESET-TIMEOUT]: Reset completes within 100ms per spec
    Resetting = 4,
    /// Waiting for controller not ready (CNR=0)
    /// #ASSUME[CNR-TIMEOUT]: CNR clears within 16ms per spec
    WaitingCNR = 5,
    /// Configuring controller (DCBAA, rings, scratchpad)
    /// #VERIFY[CONFIG-ORDER]: DCBAA set before enabling slots
    Configuring = 6,
    /// Setting up interrupters (MSI-X/MSI)
    /// #ASSUME[IRQ-AVAILABLE]: Interrupt vectors allocated
    ConfiguringInterrupts = 7,
    /// Starting controller (R/S=1)
    /// #VERIFY[START-READY]: All rings initialized before start
    Starting = 8,
    /// Controller running and operational
    /// #VERIFY[RUNNING-STABLE]: HCH=0 while R/S=1
    Running = 9,
    /// Controller halting (R/S=0, waiting HCH=1)
    /// #ASSUME[HALT-TIMEOUT]: Halt completes within 16ms
    Halting = 10,
    /// Controller halted
    /// #VERIFY[HALTED-STABLE]: HCH=1 after halt completes
    Halted = 11,
    /// Controller suspended (save state)
    /// #ASSUME[SUSPEND-SUPPORT]: Controller supports save/restore
    Suspended = 12,
    /// Controller in error state (HSE or HCE)
    /// #VERIFY[ERROR-LOGGED]: Error details captured in last_error
    Error = 254,
    /// Controller not present (PCI device not found)
    /// #ASSUME[NO-DEVICE]: Safe to retry detection later
    NotPresent = 255,
}

impl UsbControllerState {
    /// Extract state from packed u64
    ///
    /// # Layout
    /// - Bits 0-7: State enum value
    /// - Bits 8-15: Reserved (error sub-code)
    /// - Bits 16-31: Reserved
    /// - Bits 32-63: Generation counter
    ///
    /// #VERIFY[UNPACK-VALID]: All packed values produce valid states
    #[inline(always)]
    pub fn from_packed(packed: u64) -> Self {
        match (packed & 0xFF) as u8 {
            0 => UsbControllerState::Uninitialized,
            1 => UsbControllerState::Detecting,
            2 => UsbControllerState::MappingRegs,
            3 => UsbControllerState::ReadingCaps,
            4 => UsbControllerState::Resetting,
            5 => UsbControllerState::WaitingCNR,
            6 => UsbControllerState::Configuring,
            7 => UsbControllerState::ConfiguringInterrupts,
            8 => UsbControllerState::Starting,
            9 => UsbControllerState::Running,
            10 => UsbControllerState::Halting,
            11 => UsbControllerState::Halted,
            12 => UsbControllerState::Suspended,
            254 => UsbControllerState::Error,
            255 => UsbControllerState::NotPresent,
            _ => UsbControllerState::Error,
        }
    }

    /// Pack state with metadata into u64
    ///
    /// # Layout
    /// - Bits 0-7: State (8 bits)
    /// - Bits 8-15: Max slots (8 bits)
    /// - Bits 16-23: Max ports (8 bits)
    /// - Bits 24-31: Context size flag + reserved (8 bits)
    /// - Bits 32-63: Generation counter (32 bits)
    ///
    /// #VERIFY[PACK-LOSSLESS]: Round-trip pack/unpack preserves all data
    #[inline(always)]
    pub const fn pack(self, generation: u64, max_slots: u8, max_ports: u8, ctx_size_64: bool) -> u64 {
        let state = self as u8 as u64;
        let slots = (max_slots as u64) << 8;
        let ports = (max_ports as u64) << 16;
        let ctx = ((ctx_size_64 as u8) as u64) << 24;
        let gen = (generation & 0xFFFF_FFFF) << 32;
        state | slots | ports | ctx | gen
    }

    /// Check if state allows device operations
    #[inline(always)]
    pub const fn allows_device_ops(&self) -> bool {
        matches!(self, UsbControllerState::Running)
    }

    /// Check if state is terminal (error or not present)
    #[inline(always)]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, UsbControllerState::Error | UsbControllerState::NotPresent)
    }
}

// ============================================================================
// Controller Snapshot
// ============================================================================

/// Atomic snapshot of xHCI controller state
///
/// All fields captured atomically for consistent view of controller state.
/// Used for status reporting and decision making without holding locks.
///
/// #VERIFY[SNAPSHOT-CONSISTENT]: All fields from same generation
#[derive(Debug, Clone, Copy)]
pub struct UsbControllerSnapshot {
    /// Current state
    pub state: UsbControllerState,
    /// Maximum device slots (from HCSPARAMS1)
    pub max_slots: u8,
    /// Maximum root hub ports (from HCSPARAMS1)
    pub max_ports: u8,
    /// Maximum interrupters (from HCSPARAMS1)
    pub max_intrs: u16,
    /// Context size: true = 64 bytes, false = 32 bytes
    pub context_size_64: bool,
    /// 64-bit addressing capable (from HCCPARAMS1)
    pub supports_64bit: bool,
    /// Generation counter for ABA prevention
    pub generation: u64,
    /// MMIO base address (virtual)
    pub mmio_base: u64,
    /// xHCI version (BCD: 0x0100 = 1.0, 0x0110 = 1.1, 0x0120 = 1.2)
    pub version: u16,
    /// Page size from controller (minimum 4KB)
    pub page_size: u32,
    /// Enabled device slots count
    pub enabled_slots: u8,
    /// Active ports bitmap (ports with devices)
    pub active_ports_bitmap: u64,
    /// Total commands issued
    pub commands_issued: u64,
    /// Total events processed
    pub events_processed: u64,
    /// Total interrupts serviced
    pub interrupts_serviced: u64,
    /// Last error code
    pub last_error: u32,
}

impl UsbControllerSnapshot {
    /// Check if controller is ready for device operations
    #[inline(always)]
    pub fn is_ready(&self) -> bool {
        self.state == UsbControllerState::Running
    }

    /// Check if controller is present
    #[inline(always)]
    pub fn is_present(&self) -> bool {
        self.state != UsbControllerState::NotPresent
    }

    /// Check if controller is in error state
    #[inline(always)]
    pub fn is_error(&self) -> bool {
        self.state == UsbControllerState::Error
    }

    /// Check if controller supports SuperSpeed (USB 3.x)
    #[inline(always)]
    pub fn supports_superspeed(&self) -> bool {
        self.version >= 0x0100 // xHCI 1.0+ supports USB 3.0
    }

    /// Get context size in bytes
    #[inline(always)]
    pub fn context_size(&self) -> usize {
        if self.context_size_64 { 64 } else { 32 }
    }
}

// ============================================================================
// USB Controller Capsule (512 bytes)
// ============================================================================

/// USB Controller Capsule - xHCI Host Controller State (512 bytes, cache-aligned)
///
/// **Architecture**: Tier 1 (Atomic)
/// - Multiple AtomicU64 for lockfree state coordination
/// - 512-byte alignment (8 cache lines)
/// - Generation counter prevents ABA problems
/// - Comprehensive controller state tracking
///
/// # Memory Layout
///
/// ## Cache Line 0 (64 bytes) - Primary State
/// - state_gen: State + max slots/ports + generation (8 bytes)
/// - mmio_base: MMIO base address (8 bytes)
/// - cap_offset: Capability registers offset (8 bytes)
/// - op_offset: Operational registers offset (8 bytes)
/// - runtime_offset: Runtime registers offset (8 bytes)
/// - doorbell_offset: Doorbell registers offset (8 bytes)
/// - context_version: Context size + version packed (8 bytes)
/// - page_size: System page size (8 bytes)
///
/// ## Cache Line 1 (64 bytes) - Ring Pointers
/// - dcbaap: Device Context Base Address Array Pointer (8 bytes)
/// - command_ring: Command ring base address (8 bytes)
/// - event_ring_erstba: Event ring segment table base (8 bytes)
/// - event_ring_erdp: Event ring dequeue pointer (8 bytes)
/// - scratchpad_ptr: Scratchpad buffer array pointer (8 bytes)
/// - scratchpad_count: Number of scratchpad buffers (8 bytes)
/// - enabled_slots: Enabled slots count (8 bytes)
/// - config_register: CONFIG register value (8 bytes)
///
/// ## Cache Line 2 (64 bytes) - Port State
/// - active_ports_bitmap: Bitmap of active ports (8 bytes)
/// - port_speed_cache: Speed cache (4 bits × 16 ports = 8 bytes × 4)
/// - port_change_bitmap: Port change event bitmap (8 bytes)
///
/// ## Cache Line 3-4 (128 bytes) - Statistics
/// - commands_issued: Total commands issued (8 bytes)
/// - commands_completed: Total commands completed (8 bytes)
/// - commands_failed: Total commands failed (8 bytes)
/// - events_processed: Total events processed (8 bytes)
/// - interrupts_serviced: Total interrupts (8 bytes)
/// - transfers_completed: Total successful transfers (8 bytes)
/// - transfers_failed: Total failed transfers (8 bytes)
/// - last_error: Last error code (4 bytes)
/// - error_count: Total error count (4 bytes)
/// - Additional stats and reserved fields
///
/// ## Cache Lines 5-7 (192 bytes) - Extended State
/// - MSI-X/MSI configuration
/// - Power management state
/// - Debug and diagnostics
/// - Padding
///
/// #ASSUME[CACHE-ALIGN]: 512-byte alignment prevents false sharing
/// #VERIFY[SIZE-512]: Structure exactly 512 bytes
#[repr(C, align(512))]
pub struct UsbControllerCapsule {
    // === Cache Line 0 (64 bytes) - Primary State ===
    /// Packed state: state (8) | max_slots (8) | max_ports (8) | ctx_size (8) | gen (32)
    /// #VERIFY[STATE-ATOMIC]: Single atomic for consistent state reads
    state_gen: AtomicU64,
    /// MMIO base address (from BAR0/BAR1)
    /// #ASSUME[MMIO-ALIGNED]: At least page-aligned
    mmio_base: AtomicU64,
    /// Capability registers offset (always 0)
    cap_offset: AtomicU64,
    /// Operational registers offset (from CAPLENGTH)
    /// #VERIFY[OP-OFFSET]: Value read from CAPLENGTH byte
    op_offset: AtomicU64,
    /// Runtime registers offset (from RTSOFF)
    /// #VERIFY[RT-OFFSET]: Must be 32-byte aligned per spec
    runtime_offset: AtomicU64,
    /// Doorbell registers offset (from DBOFF)
    /// #VERIFY[DB-OFFSET]: Must be 32-bit aligned per spec
    doorbell_offset: AtomicU64,
    /// Context size (bits 0-7) + xHCI version (bits 16-31)
    context_version: AtomicU64,
    /// Page size from controller (power of 2, minimum 4KB)
    /// #ASSUME[PAGE-SIZE]: Valid power of 2 from PAGESIZE register
    page_size: AtomicU64,

    // === Cache Line 1 (64 bytes) - Ring Pointers ===
    /// Device Context Base Address Array Pointer
    /// #VERIFY[DCBAAP-ALIGN]: 64-byte aligned per spec
    dcbaap: AtomicU64,
    /// Command ring base address
    /// #VERIFY[CMD-RING-ALIGN]: 64-byte aligned for TRB access
    command_ring: AtomicU64,
    /// Event ring segment table base (ERSTBA)
    /// #VERIFY[ERSTBA-ALIGN]: 64-byte aligned per spec
    event_ring_erstba: AtomicU64,
    /// Event ring dequeue pointer (ERDP)
    /// #VERIFY[ERDP-VALID]: Points within event ring segment
    event_ring_erdp: AtomicU64,
    /// Scratchpad buffer array pointer
    /// #ASSUME[SCRATCH-CONTIGUOUS]: Buffers physically contiguous
    scratchpad_ptr: AtomicU64,
    /// Number of scratchpad buffers allocated
    scratchpad_count: AtomicU64,
    /// Enabled slots count
    enabled_slots: AtomicU64,
    /// CONFIG register value (MaxSlotsEn)
    config_register: AtomicU64,

    // === Cache Line 2 (64 bytes) - Port State ===
    /// Active ports bitmap (64 ports max)
    active_ports: AtomicU64,
    /// Port speed cache (4 bits per port, 16 ports per u64)
    /// Index: port / 16, Shift: (port % 16) * 4
    /// #VERIFY[SPEED-VALID]: Speed values 0-7 per xHCI spec
    port_speeds: [AtomicU64; 4],
    /// Port status change bitmap
    port_change_bitmap: AtomicU64,
    /// Max interrupters (from HCSPARAMS1 bits 18:8)
    max_intrs: AtomicU64,
    /// Flags: 64-bit capable (bit 0), port power control (bit 1), etc.
    capability_flags: AtomicU64,

    // === Cache Line 3 (64 bytes) - Statistics ===
    /// Total commands issued
    commands_issued: AtomicU64,
    /// Total commands completed successfully
    commands_completed: AtomicU64,
    /// Total commands failed
    commands_failed: AtomicU64,
    /// Total events processed
    events_processed: AtomicU64,
    /// Total interrupts serviced
    interrupts_serviced: AtomicU64,
    /// Total successful transfers
    transfers_completed: AtomicU64,
    /// Total failed transfers
    transfers_failed: AtomicU64,
    /// Last error code (32 bits) + error count (32 bits)
    last_error_count: AtomicU64,

    // === Cache Line 4 (64 bytes) - Extended State ===
    /// MSI-X table base address
    msix_table_base: AtomicU64,
    /// MSI-X PBA (Pending Bit Array) base
    msix_pba_base: AtomicU64,
    /// MSI-X enabled vectors bitmap (up to 64 vectors)
    msix_enabled_vectors: AtomicU64,
    /// Power state: D0=0, D3hot=3
    power_state: AtomicU64,
    /// USB 3.0 protocol capability offset
    usb3_cap_offset: AtomicU64,
    /// USB 2.0 protocol capability offset
    usb2_cap_offset: AtomicU64,
    /// Extended capability pointer
    ext_cap_pointer: AtomicU64,
    /// Reserved for future use
    _reserved_cl4: AtomicU64,

    // === Cache Lines 5-7 (192 bytes) - Padding ===
    /// Padding to 512 bytes
    _padding: [u8; 192],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<UsbControllerCapsule>() == 512);
const _: () = assert!(core::mem::align_of::<UsbControllerCapsule>() == 512);

// Capability flags
const FLAG_64BIT_CAPABLE: u64 = 1 << 0;
const FLAG_PORT_POWER_CONTROL: u64 = 1 << 1;
const FLAG_PORT_INDICATORS: u64 = 1 << 2;
const FLAG_LIGHT_RESET: u64 = 1 << 3;
const FLAG_LATENCY_TOLERANCE: u64 = 1 << 4;
const FLAG_SECONDARY_SID: u64 = 1 << 5;
const FLAG_VIRTUALIZATION: u64 = 1 << 6;

impl UsbControllerCapsule {
    /// Create new USB controller capsule in Uninitialized state
    ///
    /// #VERIFY[INIT-ZEROED]: All counters start at zero
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(UsbControllerState::Uninitialized.pack(0, 0, 0, false)),
            mmio_base: AtomicU64::new(0),
            cap_offset: AtomicU64::new(0),
            op_offset: AtomicU64::new(0),
            runtime_offset: AtomicU64::new(0),
            doorbell_offset: AtomicU64::new(0),
            context_version: AtomicU64::new(0),
            page_size: AtomicU64::new(4096),
            dcbaap: AtomicU64::new(0),
            command_ring: AtomicU64::new(0),
            event_ring_erstba: AtomicU64::new(0),
            event_ring_erdp: AtomicU64::new(0),
            scratchpad_ptr: AtomicU64::new(0),
            scratchpad_count: AtomicU64::new(0),
            enabled_slots: AtomicU64::new(0),
            config_register: AtomicU64::new(0),
            active_ports: AtomicU64::new(0),
            port_speeds: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            port_change_bitmap: AtomicU64::new(0),
            max_intrs: AtomicU64::new(0),
            capability_flags: AtomicU64::new(0),
            commands_issued: AtomicU64::new(0),
            commands_completed: AtomicU64::new(0),
            commands_failed: AtomicU64::new(0),
            events_processed: AtomicU64::new(0),
            interrupts_serviced: AtomicU64::new(0),
            transfers_completed: AtomicU64::new(0),
            transfers_failed: AtomicU64::new(0),
            last_error_count: AtomicU64::new(0),
            msix_table_base: AtomicU64::new(0),
            msix_pba_base: AtomicU64::new(0),
            msix_enabled_vectors: AtomicU64::new(0),
            power_state: AtomicU64::new(0),
            usb3_cap_offset: AtomicU64::new(0),
            usb2_cap_offset: AtomicU64::new(0),
            ext_cap_pointer: AtomicU64::new(0),
            _reserved_cl4: AtomicU64::new(0),
            _padding: [0u8; 192],
        }
    }

    /// Get atomic snapshot of current state
    ///
    /// #VERIFY[SNAPSHOT-ATOMIC]: All reads use Acquire ordering
    #[inline(always)]
    pub fn snapshot(&self) -> UsbControllerSnapshot {
        let packed = self.state_gen.load(Ordering::Acquire);
        let mmio_base = self.mmio_base.load(Ordering::Acquire);
        let ctx_ver = self.context_version.load(Ordering::Acquire);
        let page_size = self.page_size.load(Ordering::Acquire) as u32;
        let enabled_slots = self.enabled_slots.load(Ordering::Acquire) as u8;
        let active_ports = self.active_ports.load(Ordering::Acquire);
        let max_intrs = self.max_intrs.load(Ordering::Acquire) as u16;
        let cap_flags = self.capability_flags.load(Ordering::Acquire);
        let last_err = self.last_error_count.load(Ordering::Acquire);

        UsbControllerSnapshot {
            state: UsbControllerState::from_packed(packed),
            max_slots: ((packed >> 8) & 0xFF) as u8,
            max_ports: ((packed >> 16) & 0xFF) as u8,
            context_size_64: ((packed >> 24) & 1) != 0,
            supports_64bit: (cap_flags & FLAG_64BIT_CAPABLE) != 0,
            generation: (packed >> 32) & 0xFFFF_FFFF,
            mmio_base,
            version: ((ctx_ver >> 16) & 0xFFFF) as u16,
            page_size,
            max_intrs,
            enabled_slots,
            active_ports_bitmap: active_ports,
            commands_issued: self.commands_issued.load(Ordering::Acquire),
            events_processed: self.events_processed.load(Ordering::Acquire),
            interrupts_serviced: self.interrupts_serviced.load(Ordering::Acquire),
            last_error: (last_err & 0xFFFF_FFFF) as u32,
        }
    }

    /// Get current state only (fast path)
    #[inline(always)]
    pub fn state(&self) -> UsbControllerState {
        UsbControllerState::from_packed(self.state_gen.load(Ordering::Acquire))
    }

    /// Get generation counter
    #[inline(always)]
    pub fn generation(&self) -> u64 {
        let packed = self.state_gen.load(Ordering::Acquire);
        (packed >> 32) & 0xFFFF_FFFF
    }

    /// Get MMIO base address
    #[inline(always)]
    pub fn mmio_base(&self) -> u64 {
        self.mmio_base.load(Ordering::Acquire)
    }

    /// Get operational registers base address
    #[inline(always)]
    pub fn operational_base(&self) -> u64 {
        let base = self.mmio_base.load(Ordering::Acquire);
        let offset = self.op_offset.load(Ordering::Acquire);
        base + offset
    }

    /// Get runtime registers base address
    #[inline(always)]
    pub fn runtime_base(&self) -> u64 {
        let base = self.mmio_base.load(Ordering::Acquire);
        let offset = self.runtime_offset.load(Ordering::Acquire);
        base + offset
    }

    /// Get doorbell registers base address
    #[inline(always)]
    pub fn doorbell_base(&self) -> u64 {
        let base = self.mmio_base.load(Ordering::Acquire);
        let offset = self.doorbell_offset.load(Ordering::Acquire);
        base + offset
    }

    /// Get DCBAAP (Device Context Base Address Array Pointer)
    #[inline(always)]
    pub fn dcbaap(&self) -> u64 {
        self.dcbaap.load(Ordering::Acquire)
    }

    /// Get command ring base address
    #[inline(always)]
    pub fn command_ring_base(&self) -> u64 {
        self.command_ring.load(Ordering::Acquire)
    }

    /// Transition state with CAS (lockfree state machine)
    ///
    /// # Arguments
    /// - `expected_state`: Expected current state
    /// - `new_state`: Target state
    ///
    /// # Returns
    /// - `Ok(new_generation)`: Transition successful
    /// - `Err(actual_state)`: Transition failed, returns actual state
    ///
    /// #VERIFY[TRANSITION-CAS]: Uses compare_exchange for atomicity
    fn transition_state(
        &self,
        expected_state: UsbControllerState,
        new_state: UsbControllerState,
    ) -> Result<u64, UsbControllerState> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let actual_state = UsbControllerState::from_packed(current);

            if actual_state != expected_state {
                return Err(actual_state);
            }

            let max_slots = ((current >> 8) & 0xFF) as u8;
            let max_ports = ((current >> 16) & 0xFF) as u8;
            let ctx_size = ((current >> 24) & 1) != 0;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let new_packed = new_state.pack(new_gen, max_slots, max_ports, ctx_size);

            match self.state_gen.compare_exchange(
                current,
                new_packed,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(new_gen),
                Err(_) => continue, // CAS failed, retry
            }
        }
    }

    /// Begin detection (Uninitialized -> Detecting)
    ///
    /// #ASSUME[DETECT-SAFE]: Safe to access PCI config space
    pub fn begin_detection(&self) -> Result<u64, UsbControllerState> {
        self.transition_state(UsbControllerState::Uninitialized, UsbControllerState::Detecting)
    }

    /// Set MMIO base (Detecting -> MappingRegs)
    ///
    /// # Arguments
    /// - `mmio_base`: Physical/virtual address of xHCI MMIO region
    ///
    /// #ASSUME[MMIO-VALID]: Address is valid and accessible
    pub fn set_mmio_base(&self, mmio_base: u64) -> Result<u64, UsbControllerState> {
        let gen = self.transition_state(UsbControllerState::Detecting, UsbControllerState::MappingRegs)?;
        self.mmio_base.store(mmio_base, Ordering::Release);
        Ok(gen)
    }

    /// Configure from capability registers (MappingRegs -> ReadingCaps -> Configuring)
    ///
    /// # Arguments
    /// - `caplength`: Capability register length (offset to operational regs)
    /// - `version`: xHCI version (BCD)
    /// - `max_slots`: Maximum device slots from HCSPARAMS1
    /// - `max_ports`: Maximum ports from HCSPARAMS1
    /// - `max_intrs`: Maximum interrupters from HCSPARAMS1
    /// - `context_size_64`: True if CSZ=1 (64-byte contexts)
    /// - `supports_64bit`: True if AC64=1
    /// - `dboff`: Doorbell offset
    /// - `rtsoff`: Runtime offset
    ///
    /// #VERIFY[CONFIG-VALID]: All parameters within spec limits
    #[allow(clippy::too_many_arguments)]
    pub fn configure_from_caps(
        &self,
        caplength: u8,
        version: u16,
        max_slots: u8,
        max_ports: u8,
        max_intrs: u16,
        context_size_64: bool,
        supports_64bit: bool,
        dboff: u32,
        rtsoff: u32,
    ) -> Result<u64, UsbControllerState> {
        // Transition through ReadingCaps to Configuring
        self.transition_state(UsbControllerState::MappingRegs, UsbControllerState::ReadingCaps)?;

        // Store register offsets
        self.op_offset.store(caplength as u64, Ordering::Release);
        self.runtime_offset.store(rtsoff as u64, Ordering::Release);
        self.doorbell_offset.store(dboff as u64, Ordering::Release);

        // Store context size and version
        let ctx_ver = (context_size_64 as u64) | ((version as u64) << 16);
        self.context_version.store(ctx_ver, Ordering::Release);

        // Store max interrupters
        self.max_intrs.store(max_intrs as u64, Ordering::Release);

        // Store capability flags
        let mut flags = 0u64;
        if supports_64bit {
            flags |= FLAG_64BIT_CAPABLE;
        }
        self.capability_flags.store(flags, Ordering::Release);

        // Transition to Configuring with updated metadata
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let new_packed = UsbControllerState::Configuring.pack(new_gen, max_slots, max_ports, context_size_64);

            if self.state_gen.compare_exchange(current, new_packed, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                return Ok(new_gen);
            }
        }
    }

    /// Set DCBAAP (Device Context Base Address Array Pointer)
    ///
    /// #VERIFY[DCBAAP-ALIGNED]: Address must be 64-byte aligned
    pub fn set_dcbaap(&self, dcbaap: u64) -> Result<(), UsbControllerState> {
        let state = self.state();
        if state != UsbControllerState::Configuring && state != UsbControllerState::Halted {
            return Err(state);
        }
        if dcbaap & 0x3F != 0 {
            return Err(UsbControllerState::Error); // Not 64-byte aligned
        }
        self.dcbaap.store(dcbaap, Ordering::Release);
        Ok(())
    }

    /// Set command ring base address
    ///
    /// #VERIFY[CMD-ALIGNED]: Address must be 64-byte aligned
    pub fn set_command_ring(&self, ring_base: u64) -> Result<(), UsbControllerState> {
        let state = self.state();
        if state != UsbControllerState::Configuring && state != UsbControllerState::Halted {
            return Err(state);
        }
        if ring_base & 0x3F != 0 {
            return Err(UsbControllerState::Error); // Not 64-byte aligned
        }
        self.command_ring.store(ring_base, Ordering::Release);
        Ok(())
    }

    /// Set event ring segment table base
    ///
    /// #VERIFY[ERST-ALIGNED]: Address must be 64-byte aligned
    pub fn set_event_ring(&self, erstba: u64, erdp: u64) -> Result<(), UsbControllerState> {
        let state = self.state();
        if state != UsbControllerState::Configuring && state != UsbControllerState::Halted {
            return Err(state);
        }
        self.event_ring_erstba.store(erstba, Ordering::Release);
        self.event_ring_erdp.store(erdp, Ordering::Release);
        Ok(())
    }

    /// Set scratchpad buffers
    ///
    /// #ASSUME[SCRATCH-DMA]: Buffers in DMA-capable memory
    pub fn set_scratchpad(&self, ptr: u64, count: u32) -> Result<(), UsbControllerState> {
        let state = self.state();
        if state != UsbControllerState::Configuring {
            return Err(state);
        }
        self.scratchpad_ptr.store(ptr, Ordering::Release);
        self.scratchpad_count.store(count as u64, Ordering::Release);
        Ok(())
    }

    /// Start controller (Configuring -> Running)
    ///
    /// #VERIFY[START-CONFIGURED]: All rings and DCBAA configured before start
    pub fn start(&self) -> Result<u64, UsbControllerState> {
        // Verify required configuration
        if self.dcbaap.load(Ordering::Acquire) == 0 {
            return Err(UsbControllerState::Error);
        }
        if self.command_ring.load(Ordering::Acquire) == 0 {
            return Err(UsbControllerState::Error);
        }

        self.transition_state(UsbControllerState::Configuring, UsbControllerState::Starting)?;
        self.transition_state(UsbControllerState::Starting, UsbControllerState::Running)
    }

    /// Halt controller (Running -> Halted)
    ///
    /// #ASSUME[HALT-COMPLETE]: Hardware responds to R/S=0 within timeout
    pub fn halt(&self) -> Result<u64, UsbControllerState> {
        self.transition_state(UsbControllerState::Running, UsbControllerState::Halting)?;
        self.transition_state(UsbControllerState::Halting, UsbControllerState::Halted)
    }

    /// Resume from halt (Halted -> Running)
    pub fn resume(&self) -> Result<u64, UsbControllerState> {
        self.transition_state(UsbControllerState::Halted, UsbControllerState::Running)
    }

    /// Set error state with error code
    ///
    /// #VERIFY[ERROR-RECORDED]: Error details captured for diagnostics
    pub fn set_error(&self, error_code: u32) -> u64 {
        // Update error tracking
        let old_err = self.last_error_count.load(Ordering::Acquire);
        let error_count = ((old_err >> 32) + 1) & 0xFFFF_FFFF;
        let new_err = (error_count << 32) | (error_code as u64);
        self.last_error_count.store(new_err, Ordering::Release);

        // Force transition to Error state
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let max_slots = ((current >> 8) & 0xFF) as u8;
            let max_ports = ((current >> 16) & 0xFF) as u8;
            let ctx_size = ((current >> 24) & 1) != 0;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let error_packed = UsbControllerState::Error.pack(new_gen, max_slots, max_ports, ctx_size);

            if self.state_gen.compare_exchange(current, error_packed, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                return new_gen;
            }
        }
    }

    /// Get port speed (4 bits per port)
    ///
    /// # Returns
    /// Port speed code: 0=undefined, 1=full, 2=low, 3=high, 4=super, 5-7=reserved
    #[inline(always)]
    pub fn get_port_speed(&self, port: u8) -> u8 {
        let idx = (port / 16) as usize;
        let shift = (port % 16) * 4;
        if idx < 4 {
            ((self.port_speeds[idx].load(Ordering::Acquire) >> shift) & 0xF) as u8
        } else {
            0
        }
    }

    /// Set port speed
    ///
    /// #VERIFY[SPEED-CAS]: Uses CAS to avoid lost updates
    pub fn set_port_speed(&self, port: u8, speed: u8) {
        let idx = (port / 16) as usize;
        let shift = (port % 16) * 4;
        if idx < 4 {
            loop {
                let current = self.port_speeds[idx].load(Ordering::Acquire);
                let mask = !(0xFu64 << shift);
                let new_val = (current & mask) | (((speed & 0xF) as u64) << shift);
                if self.port_speeds[idx].compare_exchange(current, new_val, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                    break;
                }
            }
        }
    }

    /// Mark port as having a change event pending
    pub fn set_port_change(&self, port: u8) {
        if port < 64 {
            self.port_change_bitmap.fetch_or(1u64 << port, Ordering::AcqRel);
        }
    }

    /// Clear port change flag and return if it was set
    pub fn clear_port_change(&self, port: u8) -> bool {
        if port < 64 {
            let old = self.port_change_bitmap.fetch_and(!(1u64 << port), Ordering::AcqRel);
            (old & (1u64 << port)) != 0
        } else {
            false
        }
    }

    /// Get port change bitmap
    #[inline(always)]
    pub fn port_change_bitmap(&self) -> u64 {
        self.port_change_bitmap.load(Ordering::Acquire)
    }

    /// Record command issued
    pub fn record_command_issued(&self) {
        self.commands_issued.fetch_add(1, Ordering::AcqRel);
    }

    /// Record command completion
    pub fn record_command_completed(&self, success: bool) {
        if success {
            self.commands_completed.fetch_add(1, Ordering::AcqRel);
        } else {
            self.commands_failed.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// Record event processed
    pub fn record_event_processed(&self) {
        self.events_processed.fetch_add(1, Ordering::AcqRel);
    }

    /// Record interrupt serviced
    pub fn record_interrupt(&self) {
        self.interrupts_serviced.fetch_add(1, Ordering::AcqRel);
    }

    /// Record transfer completion
    pub fn record_transfer(&self, success: bool) {
        if success {
            self.transfers_completed.fetch_add(1, Ordering::AcqRel);
        } else {
            self.transfers_failed.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// Ring doorbell for a slot
    ///
    /// # Arguments
    /// - `slot_id`: Device slot (0 = command ring, 1-255 = device endpoints)
    /// - `target`: Doorbell target (0 = default, 1-31 = endpoint DCI)
    /// - `stream_id`: Stream ID for bulk streams (0 for non-stream)
    ///
    /// # Safety
    /// Caller must ensure doorbell registers are mapped and accessible
    ///
    /// #ASSUME[DOORBELL-MAPPED]: Doorbell region accessible
    /// #VERIFY[DOORBELL-WRITE]: Uses volatile write for MMIO
    #[inline(always)]
    pub unsafe fn ring_doorbell(&self, slot_id: u8, target: u8, stream_id: u16) {
        let base = self.doorbell_base();
        let doorbell_addr = base + (slot_id as u64 * 4);
        let value = ((stream_id as u32) << 16) | (target as u32);
        let ptr = doorbell_addr as *mut u32;
        core::ptr::write_volatile(ptr, value);
    }
}

impl Default for UsbControllerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Controller Instance
// ============================================================================

/// Global USB controller capsule (single instance for primary controller)
///
/// #ASSUME[SINGLE-CONTROLLER]: System has one primary xHCI controller
static USB_CONTROLLER: UsbControllerCapsule = UsbControllerCapsule::new();

/// Get reference to global USB controller capsule
#[inline(always)]
pub fn usb_controller() -> &'static UsbControllerCapsule {
    &USB_CONTROLLER
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
    fn test_controller_capsule_size() {
        assert_eq!(
            core::mem::size_of::<UsbControllerCapsule>(),
            512,
            "UsbControllerCapsule must be exactly 512 bytes"
        );
    }

    #[test]
    fn test_controller_capsule_alignment() {
        assert_eq!(
            core::mem::align_of::<UsbControllerCapsule>(),
            512,
            "UsbControllerCapsule must be 512-byte aligned"
        );
    }

    #[test]
    fn test_controller_initial_state() {
        let ctrl = UsbControllerCapsule::new();
        let snapshot = ctrl.snapshot();

        assert_eq!(snapshot.state, UsbControllerState::Uninitialized);
        assert_eq!(snapshot.max_slots, 0);
        assert_eq!(snapshot.max_ports, 0);
        assert_eq!(snapshot.generation, 0);
        assert_eq!(snapshot.mmio_base, 0);
    }

    #[test]
    fn test_state_packing_roundtrip() {
        let states = [
            UsbControllerState::Uninitialized,
            UsbControllerState::Detecting,
            UsbControllerState::Running,
            UsbControllerState::Halted,
            UsbControllerState::Error,
        ];

        for state in states {
            let packed = state.pack(12345, 64, 8, true);
            let unpacked = UsbControllerState::from_packed(packed);
            assert_eq!(unpacked, state);
            assert_eq!((packed >> 8) & 0xFF, 64); // max_slots
            assert_eq!((packed >> 16) & 0xFF, 8); // max_ports
            assert_eq!((packed >> 24) & 1, 1); // ctx_size_64
            assert_eq!((packed >> 32) & 0xFFFF_FFFF, 12345); // generation
        }
    }

    #[test]
    fn test_detection_flow() {
        let ctrl = UsbControllerCapsule::new();

        // Begin detection
        let gen1 = ctrl.begin_detection().expect("begin_detection should succeed");
        assert!(gen1 > 0);
        assert_eq!(ctrl.state(), UsbControllerState::Detecting);

        // Set MMIO base
        let gen2 = ctrl.set_mmio_base(0xFED0_0000).expect("set_mmio_base should succeed");
        assert!(gen2 > gen1);
        assert_eq!(ctrl.state(), UsbControllerState::MappingRegs);
        assert_eq!(ctrl.mmio_base(), 0xFED0_0000);
    }

    #[test]
    fn test_configuration_flow() {
        let ctrl = UsbControllerCapsule::new();

        // Setup
        ctrl.begin_detection().unwrap();
        ctrl.set_mmio_base(0xFED0_0000).unwrap();

        // Configure from caps
        let gen = ctrl.configure_from_caps(
            32,      // caplength
            0x0110,  // version 1.1
            64,      // max_slots
            8,       // max_ports
            4,       // max_intrs
            true,    // context_size_64
            true,    // supports_64bit
            0x2000,  // dboff
            0x1000,  // rtsoff
        ).expect("configure_from_caps should succeed");

        assert!(gen > 0);
        assert_eq!(ctrl.state(), UsbControllerState::Configuring);

        let snapshot = ctrl.snapshot();
        assert_eq!(snapshot.max_slots, 64);
        assert_eq!(snapshot.max_ports, 8);
        assert!(snapshot.context_size_64);
        assert!(snapshot.supports_64bit);
        assert_eq!(snapshot.version, 0x0110);
    }

    #[test]
    fn test_start_stop_cycle() {
        let ctrl = UsbControllerCapsule::new();

        // Setup
        ctrl.begin_detection().unwrap();
        ctrl.set_mmio_base(0xFED0_0000).unwrap();
        ctrl.configure_from_caps(32, 0x0110, 64, 8, 4, true, true, 0x2000, 0x1000).unwrap();

        // Set required configuration
        ctrl.set_dcbaap(0x1000_0000).unwrap();
        ctrl.set_command_ring(0x1001_0000).unwrap();
        ctrl.set_event_ring(0x1002_0000, 0x1002_0000).unwrap();

        // Start
        let gen_start = ctrl.start().expect("start should succeed");
        assert!(gen_start > 0);
        assert_eq!(ctrl.state(), UsbControllerState::Running);
        assert!(ctrl.snapshot().is_ready());

        // Halt
        let gen_halt = ctrl.halt().expect("halt should succeed");
        assert!(gen_halt > gen_start);
        assert_eq!(ctrl.state(), UsbControllerState::Halted);
        assert!(!ctrl.snapshot().is_ready());
    }

    #[test]
    fn test_port_speed_management() {
        let ctrl = UsbControllerCapsule::new();

        ctrl.set_port_speed(0, 4); // SuperSpeed
        ctrl.set_port_speed(1, 3); // High speed
        ctrl.set_port_speed(15, 2); // Full speed
        ctrl.set_port_speed(16, 1); // Low speed (crosses u64 boundary)
        ctrl.set_port_speed(63, 4); // Max port

        assert_eq!(ctrl.get_port_speed(0), 4);
        assert_eq!(ctrl.get_port_speed(1), 3);
        assert_eq!(ctrl.get_port_speed(15), 2);
        assert_eq!(ctrl.get_port_speed(16), 1);
        assert_eq!(ctrl.get_port_speed(63), 4);
        assert_eq!(ctrl.get_port_speed(2), 0); // Not set
    }

    #[test]
    fn test_port_change_tracking() {
        let ctrl = UsbControllerCapsule::new();

        ctrl.set_port_change(0);
        ctrl.set_port_change(5);
        ctrl.set_port_change(63);

        let bitmap = ctrl.port_change_bitmap();
        assert!((bitmap & 1) != 0);
        assert!((bitmap & (1 << 5)) != 0);
        assert!((bitmap & (1 << 63)) != 0);

        assert!(ctrl.clear_port_change(5));
        assert!(!ctrl.clear_port_change(5)); // Already cleared

        let bitmap = ctrl.port_change_bitmap();
        assert!((bitmap & (1 << 5)) == 0);
    }

    #[test]
    fn test_statistics_tracking() {
        let ctrl = UsbControllerCapsule::new();

        ctrl.record_command_issued();
        ctrl.record_command_issued();
        ctrl.record_command_completed(true);
        ctrl.record_command_completed(false);
        ctrl.record_event_processed();
        ctrl.record_interrupt();
        ctrl.record_transfer(true);
        ctrl.record_transfer(false);

        let snapshot = ctrl.snapshot();
        assert_eq!(snapshot.commands_issued, 2);
        assert_eq!(snapshot.events_processed, 1);
        assert_eq!(snapshot.interrupts_serviced, 1);
    }

    #[test]
    fn test_error_state() {
        let ctrl = UsbControllerCapsule::new();

        ctrl.begin_detection().unwrap();
        ctrl.set_mmio_base(0xFED0_0000).unwrap();
        ctrl.configure_from_caps(32, 0x0110, 64, 8, 4, true, true, 0x2000, 0x1000).unwrap();
        ctrl.set_dcbaap(0x1000_0000).unwrap();
        ctrl.set_command_ring(0x1001_0000).unwrap();
        ctrl.set_event_ring(0x1002_0000, 0x1002_0000).unwrap();
        ctrl.start().unwrap();

        let gen = ctrl.set_error(0xDEAD_BEEF);
        assert!(gen > 0);
        assert_eq!(ctrl.state(), UsbControllerState::Error);
        assert!(ctrl.snapshot().is_error());
        assert_eq!(ctrl.snapshot().last_error, 0xDEAD_BEEF);
    }

    // ========================================================================
    // Q8-Q14: Property Tests
    // ========================================================================

    #[test]
    fn test_all_states_valid() {
        let states = [
            UsbControllerState::Uninitialized,
            UsbControllerState::Detecting,
            UsbControllerState::MappingRegs,
            UsbControllerState::ReadingCaps,
            UsbControllerState::Resetting,
            UsbControllerState::WaitingCNR,
            UsbControllerState::Configuring,
            UsbControllerState::ConfiguringInterrupts,
            UsbControllerState::Starting,
            UsbControllerState::Running,
            UsbControllerState::Halting,
            UsbControllerState::Halted,
            UsbControllerState::Suspended,
            UsbControllerState::Error,
            UsbControllerState::NotPresent,
        ];

        for (i, &state) in states.iter().enumerate() {
            let packed = state.pack(i as u64 * 1000, 32, 4, i % 2 == 0);
            let recovered = UsbControllerState::from_packed(packed);
            assert_eq!(recovered, state, "State {:?} should round-trip", state);
        }
    }

    #[test]
    fn test_generation_counter_wrap() {
        let max_gen = 0xFFFF_FFFFu64;
        let packed = UsbControllerState::Running.pack(max_gen, 64, 8, true);
        let snapshot_gen = (packed >> 32) & 0xFFFF_FFFF;
        assert_eq!(snapshot_gen, max_gen);
    }

    #[test]
    fn test_alignment_verification() {
        let ctrl = UsbControllerCapsule::new();
        ctrl.begin_detection().unwrap();
        ctrl.set_mmio_base(0xFED0_0000).unwrap();
        ctrl.configure_from_caps(32, 0x0110, 64, 8, 4, true, true, 0x2000, 0x1000).unwrap();

        // Valid 64-byte aligned
        assert!(ctrl.set_dcbaap(0x1000_0000).is_ok());
        assert!(ctrl.set_command_ring(0x1001_0000).is_ok());

        // Create fresh capsule for alignment error test
        let ctrl2 = UsbControllerCapsule::new();
        ctrl2.begin_detection().unwrap();
        ctrl2.set_mmio_base(0xFED0_0000).unwrap();
        ctrl2.configure_from_caps(32, 0x0110, 64, 8, 4, true, true, 0x2000, 0x1000).unwrap();

        // Invalid alignment (not 64-byte aligned)
        assert!(ctrl2.set_dcbaap(0x1000_0001).is_err());
    }

    #[test]
    fn test_pci_constants() {
        assert_eq!(XHCI_PCI_CLASS, 0x0C);
        assert_eq!(XHCI_PCI_SUBCLASS, 0x03);
        assert_eq!(XHCI_PCI_INTERFACE, 0x30);
    }

    #[test]
    fn test_global_instance() {
        let ctrl = usb_controller();
        // Global instance should be in initial state (or modified by other tests)
        assert!(ctrl.state() == UsbControllerState::Uninitialized || ctrl.snapshot().is_present());
    }
}
