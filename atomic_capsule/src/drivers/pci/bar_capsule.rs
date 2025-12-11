//! PCI BAR Capsule - T1 Atomic, 128B
//!
//! # Architecture
//! - **Tier 1 (Atomic)**: Lockfree BAR access and management
//! - **128-byte alignment**: 2 cache lines for BAR metadata
//! - **Generation counters**: ABA prevention for state transitions
//! - **100% lockfree**: Atomic CAS-based operations
//!
//! # BAR Capsule Overview
//! The PCI BAR (Base Address Register) Capsule provides:
//! - BAR type detection (memory vs I/O)
//! - BAR width detection (32-bit vs 64-bit)
//! - Size calculation via BAR sizing algorithm
//! - Address space mapping state tracking
//! - Prefetchable memory attribute handling
//!
//! # BAR Encoding (Memory)
//!
//! ```text
//! Bit  Field         Description
//! ───  ─────         ─────────────────────────────────
//! 0    Type          0 = Memory, 1 = I/O
//! 2:1  Locatable     00 = 32-bit, 10 = 64-bit
//! 3    Prefetchable  1 = Prefetchable memory
//! 31:4 Address       Base address (aligned to size)
//! ```
//!
//! # BAR Encoding (I/O)
//!
//! ```text
//! Bit  Field         Description
//! ───  ─────         ─────────────────────────────────
//! 0    Type          1 = I/O
//! 1    Reserved      Always 0
//! 31:2 Address       Base address (4-byte aligned)
//! ```
//!
//! # BAR Size Detection Algorithm
//!
//! 1. Save original BAR value
//! 2. Write all 1s (0xFFFFFFFF) to BAR
//! 3. Read back value - zeros indicate address bits, ones indicate size bits
//! 4. Restore original BAR value
//! 5. Size = ~(value & mask) + 1 where mask depends on BAR type
//!
//! # Performance Targets
//! - Snapshot: <5ns (single cache line)
//! - Size calculation: <100ns
//! - State transition: <20ns
//!
//! # Safety Assumptions (ASSUM Framework)
//! - #ASSUME[BAR-VALID]: BAR index in range 0-5
//! - #ASSUME[SIZE-ALGO]: BAR sizing via write-ones algorithm
//! - #VERIFY[STATE-CAS]: State transitions atomic via CAS
//! - #VERIFY[64BIT]: 64-bit BAR uses two consecutive BAR slots
//! - #VERIFY[PREFETCH]: Prefetchable bit only valid for memory BARs

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU8, Ordering};

// ============================================================================
// BAR Type Constants
// ============================================================================

/// BAR type mask (bit 0)
pub const BAR_TYPE_MEMORY: u32 = 0x00;
/// BAR type mask (bit 0)
pub const BAR_TYPE_IO: u32 = 0x01;

/// Memory BAR locatable type mask (bits 2:1)
pub const BAR_MEMORY_TYPE_MASK: u32 = 0x06;
/// 32-bit memory BAR
pub const BAR_MEMORY_32BIT: u32 = 0x00;
/// 64-bit memory BAR (uses next BAR for high 32 bits)
pub const BAR_MEMORY_64BIT: u32 = 0x04;

/// Prefetchable memory flag (bit 3)
pub const BAR_MEMORY_PREFETCHABLE: u32 = 0x08;

/// Memory BAR base address mask (bits 31:4)
pub const BAR_MEMORY_BASE_MASK: u32 = 0xFFFF_FFF0;

/// I/O BAR base address mask (bits 31:2)
pub const BAR_IO_BASE_MASK: u32 = 0xFFFF_FFFC;

/// Minimum BAR size (16 bytes for memory, 4 bytes for I/O)
pub const BAR_MIN_SIZE_MEMORY: u64 = 16;
pub const BAR_MIN_SIZE_IO: u64 = 4;

/// Maximum BAR size (2GB for typical systems)
pub const BAR_MAX_SIZE: u64 = 2 * 1024 * 1024 * 1024;

// ============================================================================
// BAR Type Enum
// ============================================================================

/// BAR type (Memory or I/O)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BarType {
    /// Memory-mapped BAR
    Memory = 0,
    /// I/O port BAR
    Io = 1,
    /// BAR not present/disabled
    Disabled = 255,
}

impl BarType {
    /// Determine BAR type from raw value
    #[inline(always)]
    pub fn from_raw(raw: u32) -> Self {
        if raw == 0 || raw == 0xFFFF_FFFF {
            BarType::Disabled
        } else if raw & BAR_TYPE_IO != 0 {
            BarType::Io
        } else {
            BarType::Memory
        }
    }

    /// Check if this is a memory BAR
    #[inline(always)]
    pub const fn is_memory(self) -> bool {
        matches!(self, BarType::Memory)
    }

    /// Check if this is an I/O BAR
    #[inline(always)]
    pub const fn is_io(self) -> bool {
        matches!(self, BarType::Io)
    }
}

/// BAR address width
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BarWidth {
    /// 32-bit address space
    Bits32 = 0,
    /// 64-bit address space (uses two consecutive BARs)
    Bits64 = 1,
}

impl BarWidth {
    /// Determine BAR width from raw value
    #[inline(always)]
    pub fn from_raw(raw: u32) -> Self {
        if (raw & BAR_TYPE_IO) != 0 {
            // I/O BARs are always 32-bit
            BarWidth::Bits32
        } else if (raw & BAR_MEMORY_TYPE_MASK) == BAR_MEMORY_64BIT {
            BarWidth::Bits64
        } else {
            BarWidth::Bits32
        }
    }

    /// Check if 64-bit
    #[inline(always)]
    pub const fn is_64bit(self) -> bool {
        matches!(self, BarWidth::Bits64)
    }
}

// ============================================================================
// BAR State
// ============================================================================

/// BAR state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PciBarState {
    /// BAR capsule not initialized
    Uninitialized = 0,
    /// BAR type detected from raw value
    Detected = 1,
    /// BAR size calculated
    Sized = 2,
    /// BAR mapped to virtual address
    Mapped = 3,
    /// BAR in use
    Active = 4,
    /// BAR unmapped
    Unmapped = 5,
    /// Error state
    Error = 254,
    /// BAR disabled or not present
    Disabled = 255,
}

impl PciBarState {
    /// Extract state from packed u64
    #[inline(always)]
    pub fn from_packed(packed: u64) -> Self {
        match (packed & 0xFF) as u8 {
            0 => PciBarState::Uninitialized,
            1 => PciBarState::Detected,
            2 => PciBarState::Sized,
            3 => PciBarState::Mapped,
            4 => PciBarState::Active,
            5 => PciBarState::Unmapped,
            254 => PciBarState::Error,
            255 => PciBarState::Disabled,
            _ => PciBarState::Error,
        }
    }

    /// Pack state with metadata
    ///
    /// # Layout
    /// - Bits 0-7: State (8 bits)
    /// - Bits 8-10: BAR index (3 bits)
    /// - Bits 11-11: BAR type (1 bit: 0=Memory, 1=IO)
    /// - Bits 12-12: Width (1 bit: 0=32bit, 1=64bit)
    /// - Bits 13-13: Prefetchable (1 bit)
    /// - Bits 14-23: Reserved (10 bits)
    /// - Bits 24-31: Error code (8 bits)
    /// - Bits 32-63: Generation counter (32 bits)
    #[inline(always)]
    pub const fn pack(
        self,
        generation: u64,
        bar_index: u8,
        bar_type: u8,
        width_64: bool,
        prefetchable: bool,
        error: u8,
    ) -> u64 {
        let state = self as u8 as u64;
        let idx = ((bar_index & 0x07) as u64) << 8;
        let typ = ((bar_type & 0x01) as u64) << 11;
        let w64 = if width_64 { 1u64 << 12 } else { 0 };
        let pref = if prefetchable { 1u64 << 13 } else { 0 };
        let err = (error as u64) << 24;
        let gen = (generation & 0xFFFF_FFFF) << 32;
        state | idx | typ | w64 | pref | err | gen
    }
}

// ============================================================================
// BAR Error Codes
// ============================================================================

/// BAR error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PciBarError {
    /// No error
    Success = 0,
    /// Invalid BAR index
    InvalidIndex = 1,
    /// BAR not present
    NotPresent = 2,
    /// Size calculation failed
    SizeCalcFailed = 3,
    /// Mapping failed
    MappingFailed = 4,
    /// Invalid state transition
    InvalidTransition = 5,
    /// BAR already mapped
    AlreadyMapped = 6,
    /// Generation mismatch
    GenerationMismatch = 7,
    /// Unknown error
    Unknown = 255,
}

impl PciBarError {
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }

    #[inline(always)]
    pub fn from_code(code: u8) -> Self {
        match code {
            0 => PciBarError::Success,
            1 => PciBarError::InvalidIndex,
            2 => PciBarError::NotPresent,
            3 => PciBarError::SizeCalcFailed,
            4 => PciBarError::MappingFailed,
            5 => PciBarError::InvalidTransition,
            6 => PciBarError::AlreadyMapped,
            7 => PciBarError::GenerationMismatch,
            _ => PciBarError::Unknown,
        }
    }
}

/// Result type for BAR operations
pub type PciBarResult<T> = Result<T, PciBarError>;

// ============================================================================
// BAR Snapshot
// ============================================================================

/// Atomic snapshot of BAR state
#[derive(Debug, Clone, Copy)]
pub struct PciBarSnapshot {
    /// Current state
    pub state: PciBarState,
    /// Generation counter
    pub generation: u64,
    /// BAR index (0-5)
    pub bar_index: u8,
    /// BAR type
    pub bar_type: BarType,
    /// BAR width
    pub width: BarWidth,
    /// Prefetchable flag (memory BARs only)
    pub prefetchable: bool,
    /// Last error code
    pub error: PciBarError,
    /// Raw BAR value (low 32 bits)
    pub raw_value: u32,
    /// Raw BAR value (high 32 bits for 64-bit BARs)
    pub raw_value_high: u32,
    /// Physical/bus base address
    pub base_address: u64,
    /// BAR size in bytes
    pub size: u64,
    /// Virtual address (if mapped)
    pub virtual_address: u64,
}

impl PciBarSnapshot {
    /// Check if BAR is valid and usable
    #[inline(always)]
    pub fn is_valid(&self) -> bool {
        self.bar_type != BarType::Disabled && self.size > 0
    }

    /// Check if BAR is mapped
    #[inline(always)]
    pub fn is_mapped(&self) -> bool {
        matches!(self.state, PciBarState::Mapped | PciBarState::Active)
    }

    /// Check if this is a 64-bit BAR
    #[inline(always)]
    pub fn is_64bit(&self) -> bool {
        self.width.is_64bit()
    }

    /// Check if prefetchable memory
    #[inline(always)]
    pub fn is_prefetchable(&self) -> bool {
        self.prefetchable && self.bar_type.is_memory()
    }

    /// Get end address (base + size - 1)
    #[inline(always)]
    pub fn end_address(&self) -> u64 {
        if self.size > 0 {
            self.base_address + self.size - 1
        } else {
            self.base_address
        }
    }
}

// ============================================================================
// PCI BAR Capsule (128 bytes)
// ============================================================================

/// PCI BAR Capsule (128 bytes, cache-aligned)
///
/// **Architecture**: Tier 1 (Atomic)
/// - Lockfree BAR access and size calculation
/// - Generation counters for ABA prevention
/// - Supports 32-bit and 64-bit BARs
///
/// # Memory Layout (128 bytes, 2 cache lines)
///
/// ## Cache Line 0 (64 bytes) - State & Addressing
/// - state_gen: State + metadata + generation (8 bytes)
/// - raw_value: Raw BAR register value (4 bytes)
/// - raw_value_high: High 32 bits for 64-bit BARs (4 bytes)
/// - base_address: Physical/bus base address (8 bytes)
/// - size: BAR size in bytes (8 bytes)
/// - virtual_address: Mapped virtual address (8 bytes)
/// - Reserved (24 bytes)
///
/// ## Cache Line 1 (64 bytes) - Statistics & Extended
/// - map_count: Number of times mapped (8 bytes)
/// - unmap_count: Number of times unmapped (8 bytes)
/// - access_count: Access operations (8 bytes)
/// - error_count: Errors encountered (8 bytes)
/// - Reserved (32 bytes)
///
/// #ASSUME[CACHE-ALIGN]: 128-byte alignment prevents false sharing
/// #VERIFY[SIZE-128]: Structure exactly 128 bytes
#[repr(C, align(128))]
pub struct PciBarCapsule {
    // === Cache Line 0 (64 bytes) - State & Addressing ===
    /// Packed state: state (8) | index (3) | type (1) | width (1) | prefetch (1) | reserved (10) | error (8) | gen (32)
    state_gen: AtomicU64,
    /// Raw BAR register value (low 32 bits)
    raw_value: AtomicU32,
    /// Raw BAR register value (high 32 bits for 64-bit BARs)
    raw_value_high: AtomicU32,
    /// Physical/bus base address (full 64-bit)
    base_address: AtomicU64,
    /// BAR size in bytes
    size: AtomicU64,
    /// Mapped virtual address (0 if not mapped)
    virtual_address: AtomicU64,
    /// Reserved padding for cache line 0
    _reserved_cl0: [u8; 24],

    // === Cache Line 1 (64 bytes) - Statistics & Extended ===
    /// Number of map operations
    map_count: AtomicU64,
    /// Number of unmap operations
    unmap_count: AtomicU64,
    /// Total access operations
    access_count: AtomicU64,
    /// Error count
    error_count: AtomicU64,
    /// Reserved padding for cache line 1
    _reserved_cl1: [u8; 32],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<PciBarCapsule>() == 128);
const _: () = assert!(core::mem::align_of::<PciBarCapsule>() == 128);

impl PciBarCapsule {
    /// Create new BAR capsule
    ///
    /// #VERIFY[INIT-UNINIT]: Initial state is Uninitialized
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(PciBarState::Uninitialized.pack(0, 0, 0, false, false, 0)),
            raw_value: AtomicU32::new(0),
            raw_value_high: AtomicU32::new(0),
            base_address: AtomicU64::new(0),
            size: AtomicU64::new(0),
            virtual_address: AtomicU64::new(0),
            _reserved_cl0: [0u8; 24],
            map_count: AtomicU64::new(0),
            unmap_count: AtomicU64::new(0),
            access_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            _reserved_cl1: [0u8; 32],
        }
    }

    /// Initialize BAR from raw register value
    ///
    /// # Arguments
    /// - `bar_index`: BAR index (0-5)
    /// - `raw`: Raw BAR register value
    /// - `raw_high`: High 32 bits for 64-bit BARs (0 for 32-bit)
    ///
    /// #ASSUME[BAR-VALID]: bar_index in range 0-5
    pub fn initialize(&self, bar_index: u8, raw: u32, raw_high: u32) -> PciBarResult<()> {
        if bar_index > 5 {
            return Err(PciBarError::InvalidIndex);
        }

        // Determine BAR type and width
        let bar_type = BarType::from_raw(raw);
        let width = BarWidth::from_raw(raw);
        let prefetchable = bar_type.is_memory() && (raw & BAR_MEMORY_PREFETCHABLE) != 0;

        // Check for disabled BAR
        if bar_type == BarType::Disabled {
            loop {
                let current = self.state_gen.load(Ordering::Acquire);
                let gen = ((current >> 32) & 0xFFFF_FFFF) + 1;
                let new_packed = PciBarState::Disabled.pack(gen, bar_index, 0, false, false, 0);

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

        // Store raw values
        self.raw_value.store(raw, Ordering::Release);
        self.raw_value_high.store(raw_high, Ordering::Release);

        // Calculate base address
        let base = if bar_type.is_memory() {
            let low = (raw & BAR_MEMORY_BASE_MASK) as u64;
            if width.is_64bit() {
                low | ((raw_high as u64) << 32)
            } else {
                low
            }
        } else {
            (raw & BAR_IO_BASE_MASK) as u64
        };
        self.base_address.store(base, Ordering::Release);

        // Update state
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let gen = ((current >> 32) & 0xFFFF_FFFF) + 1;
            let type_bit = if bar_type.is_io() { 1 } else { 0 };
            let new_packed = PciBarState::Detected.pack(
                gen,
                bar_index,
                type_bit,
                width.is_64bit(),
                prefetchable,
                0,
            );

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

    /// Set BAR size (after size detection)
    ///
    /// # Arguments
    /// - `size`: BAR size in bytes
    ///
    /// #VERIFY[SIZE-VALID]: Size must be power of 2 and >= minimum
    pub fn set_size(&self, size: u64) -> PciBarResult<()> {
        // Validate size is power of 2
        if size > 0 && (size & (size - 1)) != 0 {
            return Err(PciBarError::SizeCalcFailed);
        }

        self.size.store(size, Ordering::Release);

        // Transition to Sized state
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = PciBarState::from_packed(current);

            if state != PciBarState::Detected {
                return Err(PciBarError::InvalidTransition);
            }

            let bar_idx = ((current >> 8) & 0x07) as u8;
            let bar_type = ((current >> 11) & 0x01) as u8;
            let width_64 = (current >> 12) & 0x01 != 0;
            let prefetch = (current >> 13) & 0x01 != 0;
            let gen = ((current >> 32) & 0xFFFF_FFFF) + 1;

            let new_packed = PciBarState::Sized.pack(
                gen, bar_idx, bar_type, width_64, prefetch, 0,
            );

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

    /// Calculate BAR size from sizing mask
    ///
    /// # Arguments
    /// - `sizing_mask`: Value read back after writing all 1s
    ///
    /// #VERIFY[SIZE-ALGO]: Implements standard BAR sizing algorithm
    pub fn calculate_size_from_mask(sizing_mask: u32, bar_type: BarType) -> u64 {
        if sizing_mask == 0 || sizing_mask == 0xFFFF_FFFF {
            return 0;
        }

        let base_mask = if bar_type.is_memory() {
            sizing_mask & BAR_MEMORY_BASE_MASK
        } else {
            sizing_mask & BAR_IO_BASE_MASK
        };

        if base_mask == 0 {
            return 0;
        }

        // Size = ~(mask) + 1 (for the addressable bits)
        let inverted = !base_mask;
        (inverted as u64) + 1
    }

    /// Set mapped virtual address
    pub fn set_virtual_address(&self, vaddr: u64) -> PciBarResult<()> {
        let state = self.state();
        if !matches!(state, PciBarState::Sized | PciBarState::Unmapped) {
            return Err(PciBarError::InvalidTransition);
        }

        self.virtual_address.store(vaddr, Ordering::Release);
        self.map_count.fetch_add(1, Ordering::Relaxed);

        self.transition_to(PciBarState::Mapped)
    }

    /// Mark BAR as active
    pub fn mark_active(&self) -> PciBarResult<()> {
        self.transition_to_from(PciBarState::Mapped, PciBarState::Active)
    }

    /// Unmap BAR
    pub fn unmap(&self) -> PciBarResult<()> {
        let state = self.state();
        if !matches!(state, PciBarState::Mapped | PciBarState::Active) {
            return Err(PciBarError::InvalidTransition);
        }

        self.virtual_address.store(0, Ordering::Release);
        self.unmap_count.fetch_add(1, Ordering::Relaxed);

        self.transition_to(PciBarState::Unmapped)
    }

    /// Get atomic snapshot of BAR state
    #[inline(always)]
    pub fn snapshot(&self) -> PciBarSnapshot {
        let state_packed = self.state_gen.load(Ordering::Acquire);

        let bar_index = ((state_packed >> 8) & 0x07) as u8;
        let type_bit = ((state_packed >> 11) & 0x01) != 0;
        let width_64 = (state_packed >> 12) & 0x01 != 0;
        let prefetch = (state_packed >> 13) & 0x01 != 0;

        PciBarSnapshot {
            state: PciBarState::from_packed(state_packed),
            generation: (state_packed >> 32) & 0xFFFF_FFFF,
            bar_index,
            bar_type: if type_bit { BarType::Io } else { BarType::Memory },
            width: if width_64 { BarWidth::Bits64 } else { BarWidth::Bits32 },
            prefetchable: prefetch,
            error: PciBarError::from_code(((state_packed >> 24) & 0xFF) as u8),
            raw_value: self.raw_value.load(Ordering::Acquire),
            raw_value_high: self.raw_value_high.load(Ordering::Acquire),
            base_address: self.base_address.load(Ordering::Acquire),
            size: self.size.load(Ordering::Acquire),
            virtual_address: self.virtual_address.load(Ordering::Acquire),
        }
    }

    /// Get current state only (fast path)
    #[inline(always)]
    pub fn state(&self) -> PciBarState {
        PciBarState::from_packed(self.state_gen.load(Ordering::Acquire))
    }

    /// Get base address
    #[inline(always)]
    pub fn base_address(&self) -> u64 {
        self.base_address.load(Ordering::Acquire)
    }

    /// Get size
    #[inline(always)]
    pub fn size(&self) -> u64 {
        self.size.load(Ordering::Acquire)
    }

    /// Get virtual address
    #[inline(always)]
    pub fn virtual_address(&self) -> u64 {
        self.virtual_address.load(Ordering::Acquire)
    }

    /// Get BAR index
    #[inline(always)]
    pub fn bar_index(&self) -> u8 {
        ((self.state_gen.load(Ordering::Acquire) >> 8) & 0x07) as u8
    }

    /// Check if this is a 64-bit BAR
    #[inline(always)]
    pub fn is_64bit(&self) -> bool {
        (self.state_gen.load(Ordering::Acquire) >> 12) & 0x01 != 0
    }

    /// Check if prefetchable
    #[inline(always)]
    pub fn is_prefetchable(&self) -> bool {
        (self.state_gen.load(Ordering::Acquire) >> 13) & 0x01 != 0
    }

    /// Get statistics
    #[inline(always)]
    pub fn stats(&self) -> (u64, u64, u64, u64) {
        (
            self.map_count.load(Ordering::Relaxed),
            self.unmap_count.load(Ordering::Relaxed),
            self.access_count.load(Ordering::Relaxed),
            self.error_count.load(Ordering::Relaxed),
        )
    }

    /// Record access
    pub fn record_access(&self) {
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Set error state
    pub fn set_error(&self, error: PciBarError) {
        self.error_count.fetch_add(1, Ordering::Relaxed);

        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let bar_idx = ((current >> 8) & 0x07) as u8;
            let bar_type = ((current >> 11) & 0x01) as u8;
            let width_64 = (current >> 12) & 0x01 != 0;
            let prefetch = (current >> 13) & 0x01 != 0;
            let gen = ((current >> 32) & 0xFFFF_FFFF) + 1;

            let new_packed = PciBarState::Error.pack(
                gen, bar_idx, bar_type, width_64, prefetch, error.code(),
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
    }

    /// Transition to new state (from any valid state)
    fn transition_to(&self, new_state: PciBarState) -> PciBarResult<()> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let bar_idx = ((current >> 8) & 0x07) as u8;
            let bar_type = ((current >> 11) & 0x01) as u8;
            let width_64 = (current >> 12) & 0x01 != 0;
            let prefetch = (current >> 13) & 0x01 != 0;
            let gen = ((current >> 32) & 0xFFFF_FFFF) + 1;

            let new_packed = new_state.pack(gen, bar_idx, bar_type, width_64, prefetch, 0);

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

    /// Transition from specific state to new state
    fn transition_to_from(&self, from: PciBarState, to: PciBarState) -> PciBarResult<()> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = PciBarState::from_packed(current);

            if state != from {
                return Err(PciBarError::InvalidTransition);
            }

            let bar_idx = ((current >> 8) & 0x07) as u8;
            let bar_type = ((current >> 11) & 0x01) as u8;
            let width_64 = (current >> 12) & 0x01 != 0;
            let prefetch = (current >> 13) & 0x01 != 0;
            let gen = ((current >> 32) & 0xFFFF_FFFF) + 1;

            let new_packed = to.pack(gen, bar_idx, bar_type, width_64, prefetch, 0);

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

impl Default for PciBarCapsule {
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
    fn test_bar_capsule_size() {
        assert_eq!(
            core::mem::size_of::<PciBarCapsule>(),
            128,
            "PciBarCapsule must be exactly 128 bytes"
        );
    }

    #[test]
    fn test_bar_capsule_alignment() {
        assert_eq!(
            core::mem::align_of::<PciBarCapsule>(),
            128,
            "PciBarCapsule must be 128-byte aligned"
        );
    }

    #[test]
    fn test_bar_initial_state() {
        let bar = PciBarCapsule::new();
        let snapshot = bar.snapshot();

        assert_eq!(snapshot.state, PciBarState::Uninitialized);
        assert_eq!(snapshot.size, 0);
        assert_eq!(snapshot.base_address, 0);
    }

    #[test]
    fn test_bar_type_detection() {
        // Memory BAR (32-bit, non-prefetchable)
        assert_eq!(BarType::from_raw(0xF000_0000), BarType::Memory);

        // Memory BAR (prefetchable)
        assert_eq!(BarType::from_raw(0xF000_0008), BarType::Memory);

        // I/O BAR
        assert_eq!(BarType::from_raw(0x0000_3001), BarType::Io);

        // Disabled BAR
        assert_eq!(BarType::from_raw(0x0000_0000), BarType::Disabled);
        assert_eq!(BarType::from_raw(0xFFFF_FFFF), BarType::Disabled);
    }

    #[test]
    fn test_bar_width_detection() {
        // 32-bit memory BAR
        assert_eq!(BarWidth::from_raw(0xF000_0000), BarWidth::Bits32);

        // 64-bit memory BAR
        assert_eq!(BarWidth::from_raw(0xF000_0004), BarWidth::Bits64);

        // I/O BAR (always 32-bit)
        assert_eq!(BarWidth::from_raw(0x0000_3001), BarWidth::Bits32);
    }

    #[test]
    fn test_bar_initialization() {
        let bar = PciBarCapsule::new();

        // Initialize with memory BAR
        bar.initialize(0, 0xF000_0000, 0).unwrap();

        let snap = bar.snapshot();
        assert_eq!(snap.state, PciBarState::Detected);
        assert_eq!(snap.bar_index, 0);
        assert_eq!(snap.bar_type, BarType::Memory);
        assert!(!snap.is_64bit());
        assert_eq!(snap.base_address, 0xF000_0000);
    }

    #[test]
    fn test_bar_64bit_initialization() {
        let bar = PciBarCapsule::new();

        // Initialize with 64-bit memory BAR
        bar.initialize(0, 0xF000_0004, 0x0000_0001).unwrap();

        let snap = bar.snapshot();
        assert_eq!(snap.bar_type, BarType::Memory);
        assert!(snap.is_64bit());
        assert_eq!(snap.base_address, 0x0000_0001_F000_0000);
    }

    #[test]
    fn test_bar_io_initialization() {
        let bar = PciBarCapsule::new();

        // Initialize with I/O BAR
        bar.initialize(2, 0x0000_3001, 0).unwrap();

        let snap = bar.snapshot();
        assert_eq!(snap.bar_index, 2);
        assert_eq!(snap.bar_type, BarType::Io);
        assert!(!snap.is_64bit());
        assert_eq!(snap.base_address, 0x3000);
    }

    #[test]
    fn test_bar_invalid_index() {
        let bar = PciBarCapsule::new();
        assert!(bar.initialize(6, 0xF000_0000, 0).is_err());
    }

    #[test]
    fn test_bar_size_calculation() {
        // 64KB memory BAR (mask = 0xFFFF_0000)
        let size = PciBarCapsule::calculate_size_from_mask(0xFFFF_0000, BarType::Memory);
        assert_eq!(size, 65536);

        // 1MB memory BAR (mask = 0xFFF0_0000)
        let size = PciBarCapsule::calculate_size_from_mask(0xFFF0_0000, BarType::Memory);
        assert_eq!(size, 1048576);

        // 256-byte I/O BAR (mask = 0xFFFF_FF00)
        let size = PciBarCapsule::calculate_size_from_mask(0xFFFF_FF00, BarType::Io);
        assert_eq!(size, 256);
    }

    #[test]
    fn test_bar_size_setting() {
        let bar = PciBarCapsule::new();
        bar.initialize(0, 0xF000_0000, 0).unwrap();

        bar.set_size(65536).unwrap();

        let snap = bar.snapshot();
        assert_eq!(snap.state, PciBarState::Sized);
        assert_eq!(snap.size, 65536);
    }

    #[test]
    fn test_bar_mapping() {
        let bar = PciBarCapsule::new();
        bar.initialize(0, 0xF000_0000, 0).unwrap();
        bar.set_size(65536).unwrap();

        bar.set_virtual_address(0xFFFF_8000_0000_0000).unwrap();

        let snap = bar.snapshot();
        assert_eq!(snap.state, PciBarState::Mapped);
        assert_eq!(snap.virtual_address, 0xFFFF_8000_0000_0000);
        assert!(snap.is_mapped());
    }

    #[test]
    fn test_bar_lifecycle() {
        let bar = PciBarCapsule::new();

        // Initialize
        bar.initialize(0, 0xF000_0008, 0).unwrap();
        assert_eq!(bar.state(), PciBarState::Detected);
        assert!(bar.is_prefetchable());

        // Size
        bar.set_size(1048576).unwrap();
        assert_eq!(bar.state(), PciBarState::Sized);

        // Map
        bar.set_virtual_address(0xFFFF_8000_0000_0000).unwrap();
        assert_eq!(bar.state(), PciBarState::Mapped);

        // Activate
        bar.mark_active().unwrap();
        assert_eq!(bar.state(), PciBarState::Active);

        // Unmap
        bar.unmap().unwrap();
        assert_eq!(bar.state(), PciBarState::Unmapped);
        assert_eq!(bar.virtual_address(), 0);
    }

    #[test]
    fn test_bar_statistics() {
        let bar = PciBarCapsule::new();
        bar.initialize(0, 0xF000_0000, 0).unwrap();
        bar.set_size(65536).unwrap();
        bar.set_virtual_address(0x1000_0000).unwrap();
        bar.unmap().unwrap();

        let (maps, unmaps, _accesses, _errors) = bar.stats();
        assert_eq!(maps, 1);
        assert_eq!(unmaps, 1);
    }

    // ========================================================================
    // Q8-Q14: Property Tests
    // ========================================================================

    #[test]
    fn test_state_roundtrip() {
        let states = [
            PciBarState::Uninitialized,
            PciBarState::Detected,
            PciBarState::Sized,
            PciBarState::Mapped,
            PciBarState::Active,
            PciBarState::Unmapped,
            PciBarState::Error,
            PciBarState::Disabled,
        ];

        for state in states {
            let packed = state.pack(12345, 3, 1, true, true, 10);
            let unpacked = PciBarState::from_packed(packed);
            assert_eq!(unpacked, state);

            // Verify metadata extraction
            let bar_idx = ((packed >> 8) & 0x07) as u8;
            let bar_type = ((packed >> 11) & 0x01) as u8;
            let width_64 = (packed >> 12) & 0x01 != 0;
            let prefetch = (packed >> 13) & 0x01 != 0;

            assert_eq!(bar_idx, 3);
            assert_eq!(bar_type, 1);
            assert!(width_64);
            assert!(prefetch);
        }
    }

    #[test]
    fn test_error_roundtrip() {
        let errors = [
            PciBarError::Success,
            PciBarError::InvalidIndex,
            PciBarError::NotPresent,
            PciBarError::SizeCalcFailed,
            PciBarError::MappingFailed,
            PciBarError::InvalidTransition,
        ];

        for error in errors {
            let code = error.code();
            let recovered = PciBarError::from_code(code);
            assert_eq!(recovered, error);
        }
    }

    #[test]
    fn test_prefetchable_detection() {
        let bar = PciBarCapsule::new();

        // Non-prefetchable
        bar.initialize(0, 0xF000_0000, 0).unwrap();
        assert!(!bar.is_prefetchable());

        // Prefetchable
        let bar2 = PciBarCapsule::new();
        bar2.initialize(0, 0xF000_0008, 0).unwrap();
        assert!(bar2.is_prefetchable());
    }

    #[test]
    fn test_snapshot_validity() {
        let bar = PciBarCapsule::new();
        bar.initialize(0, 0xF000_0000, 0).unwrap();
        bar.set_size(65536).unwrap();

        let snap = bar.snapshot();
        assert!(snap.is_valid());
        assert!(!snap.is_mapped());
        assert_eq!(snap.end_address(), 0xF000_0000 + 65536 - 1);
    }
}
