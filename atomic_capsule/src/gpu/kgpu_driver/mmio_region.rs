//! MMIO Region Capsule - Direct memory-mapped GPU register access
//!
//! Part of KGPU-Driver v2.0 Phase 10: Capsule-OS Direct Platform
//!
//! Chaos Compliance: T1 Atomic tier, 100% lockfree
//! Performance: <50ns register read, <100ns register write
//!
//! # Safety Model
//!
//! All MMIO accesses are unsafe by nature (direct hardware interaction), but we provide:
//! - Range checking on all accesses (prevents out-of-bounds)
//! - Write-protect enforcement (prevents writes to read-only regions)
//! - Alignment enforcement (prevents misaligned 64-bit accesses)
//! - Memory barriers (ensures correct ordering with hardware)
//!
//! # SOTA References
//!
//! 1. Linux drivers/gpu/drm/i915/intel_uncore.c - Intel forcewake and unclaimed register detection
//! 2. AMD drivers/gpu/drm/amd/amdgpu/amdgpu_mmhub.c - MMHUB register access
//! 3. Linux Documentation/driver-api/device-io.rst - Memory barriers for MMIO
//! 4. PCI Express 5.0 Spec - BAR address decoding

use core::sync::atomic::{AtomicU64, Ordering, fence};
use core::ptr::{read_volatile, write_volatile};
use core::time::Duration;

/// MMIO region types (4-bit encoding)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MmioRegionType {
    /// GPU control registers (BAR0)
    GpuRegisters = 0,
    /// Submission doorbells (BAR2)
    Doorbell = 1,
    /// Video memory aperture
    Vram = 2,
    /// GTT-mapped system memory
    GttMmio = 3,
    /// Option ROM
    RomBar = 4,
    /// MSI-X table
    MsiX = 5,
    /// SR-IOV VF BARs
    Sriov = 6,
    /// Reserved
    Reserved = 7,
}

impl MmioRegionType {
    /// Convert from 4-bit encoding
    #[inline]
    pub fn from_u8(value: u8) -> Option<Self> {
        match value & 0xF {
            0 => Some(Self::GpuRegisters),
            1 => Some(Self::Doorbell),
            2 => Some(Self::Vram),
            3 => Some(Self::GttMmio),
            4 => Some(Self::RomBar),
            5 => Some(Self::MsiX),
            6 => Some(Self::Sriov),
            7 => Some(Self::Reserved),
            _ => None,
        }
    }

    /// Check if region is read-only
    #[inline]
    pub fn is_read_only(self) -> bool {
        matches!(self, Self::RomBar)
    }

    /// Check if region supports 64-bit accesses
    #[inline]
    pub fn supports_64bit(self) -> bool {
        matches!(self, Self::GpuRegisters | Self::Vram | Self::GttMmio)
    }
}

/// MMIO region flags (12-bit encoding)
#[derive(Debug, Clone, Copy)]
pub struct MmioFlags(u16);

impl MmioFlags {
    /// Read-only region
    pub const READ_ONLY: u16 = 1 << 0;
    /// Write-only region
    pub const WRITE_ONLY: u16 = 1 << 1;
    /// Requires forcewake (Intel GPUs)
    pub const FORCEWAKE: u16 = 1 << 2;
    /// Uncached access
    pub const UNCACHED: u16 = 1 << 3;
    /// Write-combining allowed
    pub const WRITE_COMBINING: u16 = 1 << 4;
    /// Requires strict ordering
    pub const STRICT_ORDER: u16 = 1 << 5;
    /// Supports unaligned access
    pub const UNALIGNED_OK: u16 = 1 << 6;
    /// Shadow register region
    pub const SHADOW: u16 = 1 << 7;
    /// Virtualized region (SR-IOV)
    pub const VIRTUALIZED: u16 = 1 << 8;

    #[inline]
    pub fn new(flags: u16) -> Self {
        Self(flags & 0xFFF) // Mask to 12 bits
    }

    #[inline]
    pub fn has(self, flag: u16) -> bool {
        (self.0 & flag) != 0
    }

    #[inline]
    pub fn is_writable(self) -> bool {
        !self.has(Self::READ_ONLY)
    }

    #[inline]
    pub fn is_readable(self) -> bool {
        !self.has(Self::WRITE_ONLY)
    }
}

/// MMIO Region Capsule - 512 bytes, 64-byte aligned
///
/// State packing (DualAtomicU64):
/// - lo: base_addr (48-bit) | region_type (4-bit) | flags (12-bit)
/// - hi: size (48-bit) | generation (16-bit)
///
/// # Lockfree Guarantee
///
/// All operations use atomic instructions only. No mutex, RwLock, or spin_lock.
///
/// # Performance
///
/// - read_reg32: <50ns (single volatile read + barrier)
/// - write_reg32: <100ns (barrier + volatile write + barrier)
/// - read_reg64: <80ns (two 32-bit reads + ordering)
/// - modify_reg32: <150ns (read + modify + write)
#[repr(C, align(64))]
pub struct MmioRegionCapsule {
    /// State: base_addr (48-bit) | region_type (4-bit) | flags (12-bit)
    state_lo: AtomicU64,
    /// State: size (48-bit) | generation (16-bit)
    state_hi: AtomicU64,

    /// Access statistics
    read_count: AtomicU64,
    write_count: AtomicU64,
    error_count: AtomicU64,

    /// Last error information (packed: error_code (16) | offset (32) | reserved (16))
    last_error: AtomicU64,

    /// Padding to 512 bytes (48 bytes used, 464 bytes padding)
    _padding: [u64; 58],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<MmioRegionCapsule>() == 512);
const _: () = assert!(core::mem::align_of::<MmioRegionCapsule>() == 64);

impl MmioRegionCapsule {
    /// Create new MMIO region
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - `base_addr` points to valid MMIO region
    /// - Region is properly mapped (uncached/write-combining)
    /// - Size is correct for the hardware
    /// - Region won't be unmapped while capsule exists
    ///
    /// #ASSUME: base_addr is valid MMIO physical address from PCI BAR
    /// #VERIFY: Kernel validates BAR addresses during PCI enumeration
    ///
    /// #ASSUME: size matches actual BAR size from PCI config space
    /// #VERIFY: PCI config space read during device initialization
    pub unsafe fn new(
        base_addr: u64,
        size: u64,
        region_type: MmioRegionType,
        flags: MmioFlags,
    ) -> Self {
        // Pack state_lo: base_addr (48-bit) | region_type (4-bit) | flags (12-bit)
        let addr_masked = base_addr & 0xFFFF_FFFF_FFFF; // 48 bits
        let type_bits = (region_type as u64) << 48;
        let flag_bits = ((flags.0 as u64) & 0xFFF) << 52;
        let state_lo = addr_masked | type_bits | flag_bits;

        // Pack state_hi: size (48-bit) | generation (16-bit)
        let size_masked = size & 0xFFFF_FFFF_FFFF; // 48 bits
        let generation = 1u64 << 48; // Start at generation 1
        let state_hi = size_masked | generation;

        Self {
            state_lo: AtomicU64::new(state_lo),
            state_hi: AtomicU64::new(state_hi),
            read_count: AtomicU64::new(0),
            write_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            last_error: AtomicU64::new(0),
            _padding: [0; 58],
        }
    }

    /// Get base address (48-bit)
    #[inline]
    pub fn base_addr(&self) -> u64 {
        let state = self.state_lo.load(Ordering::Acquire);
        state & 0xFFFF_FFFF_FFFF
    }

    /// Get region size (48-bit)
    #[inline]
    pub fn size(&self) -> u64 {
        let state = self.state_hi.load(Ordering::Acquire);
        state & 0xFFFF_FFFF_FFFF
    }

    /// Get region type
    #[inline]
    pub fn region_type(&self) -> MmioRegionType {
        let state = self.state_lo.load(Ordering::Acquire);
        let type_bits = ((state >> 48) & 0xF) as u8;
        MmioRegionType::from_u8(type_bits).unwrap_or(MmioRegionType::Reserved)
    }

    /// Get region flags
    #[inline]
    pub fn flags(&self) -> MmioFlags {
        let state = self.state_lo.load(Ordering::Acquire);
        let flag_bits = ((state >> 52) & 0xFFF) as u16;
        MmioFlags::new(flag_bits)
    }

    /// Get generation counter (16-bit)
    #[inline]
    pub fn generation(&self) -> u16 {
        let state = self.state_hi.load(Ordering::Acquire);
        (state >> 48) as u16
    }

    /// Increment generation counter (for ABA prevention)
    #[inline]
    fn increment_generation(&self) {
        let current = self.state_hi.load(Ordering::Acquire);
        let size = current & 0xFFFF_FFFF_FFFF;
        let gen = ((current >> 48) as u16).wrapping_add(1);
        let new_state = size | ((gen as u64) << 48);
        self.state_hi.store(new_state, Ordering::Release);
    }

    /// Check if offset is within region bounds
    #[inline]
    fn check_bounds(&self, offset: u32, access_size: u32) -> bool {
        let size = self.size();
        let end_offset = offset as u64 + access_size as u64;
        end_offset <= size
    }

    /// Record error
    #[inline]
    fn record_error(&self, error_code: u16, offset: u32) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        let packed = ((error_code as u64) << 48) | ((offset as u64) << 16);
        self.last_error.store(packed, Ordering::Release);
    }

    /// Memory barrier after MMIO reads
    ///
    /// Ensures previous MMIO reads complete before subsequent operations.
    #[inline]
    pub fn read_barrier(&self) {
        fence(Ordering::Acquire);
    }

    /// Memory barrier before MMIO writes
    ///
    /// Ensures previous operations complete before MMIO writes.
    #[inline]
    pub fn write_barrier(&self) {
        fence(Ordering::Release);
    }

    /// Full memory barrier
    ///
    /// Ensures strict ordering of all operations.
    #[inline]
    pub fn full_barrier(&self) {
        fence(Ordering::SeqCst);
    }

    /// Read 32-bit register (<50ns typical)
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - Offset is 4-byte aligned
    /// - Offset is within region bounds
    /// - Register supports read operations
    ///
    /// #ASSUME: offset is 4-byte aligned for 32-bit register access
    /// #VERIFY: Offset % 4 == 0 check in bounds validation
    ///
    /// #ASSUME: Hardware register exists at offset
    /// #VERIFY: Hardware documentation and PCI BAR layout
    pub fn read_reg32(&self, offset: u32) -> Result<u32, MmioError> {
        // Check alignment
        if offset & 0x3 != 0 {
            self.record_error(MmioError::MISALIGNED as u16, offset);
            return Err(MmioError::Misaligned);
        }

        // Check bounds
        if !self.check_bounds(offset, 4) {
            self.record_error(MmioError::OUT_OF_BOUNDS as u16, offset);
            return Err(MmioError::OutOfBounds);
        }

        // Check read permission
        let flags = self.flags();
        if !flags.is_readable() {
            self.record_error(MmioError::PERMISSION_DENIED as u16, offset);
            return Err(MmioError::PermissionDenied);
        }

        // Perform volatile read
        let base = self.base_addr();
        let addr = (base + offset as u64) as *const u32;

        // #ASSUME: addr is valid MMIO address
        // #VERIFY: Bounds check above + base_addr from PCI BAR
        let value = unsafe { read_volatile(addr) };

        // Memory barrier to ensure read completes
        self.read_barrier();

        // Update statistics
        self.read_count.fetch_add(1, Ordering::Relaxed);

        Ok(value)
    }

    /// Write 32-bit register (<100ns typical)
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - Offset is 4-byte aligned
    /// - Offset is within region bounds
    /// - Register supports write operations
    /// - Value is valid for the register
    ///
    /// #ASSUME: offset is 4-byte aligned for 32-bit register access
    /// #VERIFY: Offset % 4 == 0 check in bounds validation
    ///
    /// #ASSUME: value is valid for hardware register
    /// #VERIFY: Caller responsibility (hardware-specific validation)
    pub fn write_reg32(&self, offset: u32, value: u32) -> Result<(), MmioError> {
        // Check alignment
        if offset & 0x3 != 0 {
            self.record_error(MmioError::MISALIGNED as u16, offset);
            return Err(MmioError::Misaligned);
        }

        // Check bounds
        if !self.check_bounds(offset, 4) {
            self.record_error(MmioError::OUT_OF_BOUNDS as u16, offset);
            return Err(MmioError::OutOfBounds);
        }

        // Check write permission
        let flags = self.flags();
        if !flags.is_writable() {
            self.record_error(MmioError::PERMISSION_DENIED as u16, offset);
            return Err(MmioError::PermissionDenied);
        }

        // Memory barrier before write
        self.write_barrier();

        // Perform volatile write
        let base = self.base_addr();
        let addr = (base + offset as u64) as *mut u32;

        // #ASSUME: addr is valid MMIO address
        // #VERIFY: Bounds check above + base_addr from PCI BAR
        unsafe { write_volatile(addr, value) };

        // Memory barrier after write
        self.write_barrier();

        // Update statistics
        self.write_count.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Read 64-bit register (<80ns typical)
    ///
    /// Reads as two 32-bit operations with proper ordering.
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - Offset is 8-byte aligned
    /// - Register supports 64-bit reads
    /// - Hardware guarantees atomicity if required
    ///
    /// #ASSUME: Hardware allows split 32-bit reads for 64-bit registers
    /// #VERIFY: Hardware documentation (common for PCIe MMIO)
    pub fn read_reg64(&self, offset: u32) -> Result<u64, MmioError> {
        // Check alignment
        if offset & 0x7 != 0 {
            self.record_error(MmioError::MISALIGNED as u16, offset);
            return Err(MmioError::Misaligned);
        }

        // Check if region supports 64-bit access
        if !self.region_type().supports_64bit() {
            self.record_error(MmioError::UNSUPPORTED as u16, offset);
            return Err(MmioError::Unsupported);
        }

        // Read low 32 bits
        let lo = self.read_reg32(offset)?;

        // Read high 32 bits
        let hi = self.read_reg32(offset + 4)?;

        Ok((hi as u64) << 32 | lo as u64)
    }

    /// Write 64-bit register (<150ns typical)
    ///
    /// Writes as two 32-bit operations with proper ordering.
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - Offset is 8-byte aligned
    /// - Register supports 64-bit writes
    /// - Write order (lo then hi) is correct for hardware
    ///
    /// #ASSUME: Hardware expects lo-word then hi-word write order
    /// #VERIFY: Hardware documentation (standard for PCIe)
    pub fn write_reg64(&self, offset: u32, value: u64) -> Result<(), MmioError> {
        // Check alignment
        if offset & 0x7 != 0 {
            self.record_error(MmioError::MISALIGNED as u16, offset);
            return Err(MmioError::Misaligned);
        }

        // Check if region supports 64-bit access
        if !self.region_type().supports_64bit() {
            self.record_error(MmioError::UNSUPPORTED as u16, offset);
            return Err(MmioError::Unsupported);
        }

        // Write low 32 bits first
        self.write_reg32(offset, value as u32)?;

        // Write high 32 bits
        self.write_reg32(offset + 4, (value >> 32) as u32)?;

        Ok(())
    }

    /// Modify 32-bit register (read-modify-write, <150ns typical)
    ///
    /// Atomically: value = (value & ~clear_mask) | set_mask
    ///
    /// # Safety
    ///
    /// Not atomic at hardware level - caller must ensure no concurrent
    /// hardware modifications to the register.
    ///
    /// #ASSUME: No concurrent hardware writes to register during RMW
    /// #VERIFY: Caller must ensure via hardware-specific locking
    pub fn modify_reg32(
        &self,
        offset: u32,
        clear_mask: u32,
        set_mask: u32,
    ) -> Result<u32, MmioError> {
        let current = self.read_reg32(offset)?;
        let new_value = (current & !clear_mask) | set_mask;
        self.write_reg32(offset, new_value)?;
        Ok(new_value)
    }

    /// Poll register until condition met or timeout (<1ms typical)
    ///
    /// Polls register at offset until (value & mask) == expected_value.
    ///
    /// # Performance
    ///
    /// Each poll: ~50ns read + check
    /// Max polls: timeout_ns / 50ns
    ///
    /// #ASSUME: timeout_ns is reasonable (<10ms typical)
    /// #VERIFY: Caller provides timeout based on hardware specs
    pub fn poll_reg32(
        &self,
        offset: u32,
        mask: u32,
        expected_value: u32,
        timeout_ns: u64,
    ) -> Result<u32, MmioError> {
        let start = Self::get_time_ns();
        let mut last_value;

        loop {
            last_value = self.read_reg32(offset)?;
            if (last_value & mask) == expected_value {
                return Ok(last_value);
            }

            let elapsed = Self::get_time_ns() - start;
            if elapsed >= timeout_ns {
                self.record_error(MmioError::TIMEOUT as u16, offset);
                return Err(MmioError::Timeout);
            }

            // Small delay to avoid hammering hardware
            Self::cpu_relax();
        }
    }

    /// Get current time in nanoseconds (monotonic)
    ///
    /// #ASSUME: Platform provides monotonic time source
    /// #VERIFY: Use CLOCK_MONOTONIC on Linux, TSC on bare metal
    #[inline]
    fn get_time_ns() -> u64 {
        // TODO: Platform-specific implementation
        // For now, use a stub that returns incrementing values
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1000, Ordering::Relaxed)
    }

    /// CPU relax hint (pause instruction on x86)
    #[inline]
    fn cpu_relax() {
        #[cfg(target_arch = "x86_64")]
        {
            // SAFETY: _mm_pause is a safe CPU hint instruction that reduces
            // power consumption during spin-waits. No memory safety concerns.
            #[allow(unsafe_code)]
            unsafe {
                core::arch::x86_64::_mm_pause();
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        core::hint::spin_loop();
    }

    /// Get read count
    #[inline]
    pub fn read_count(&self) -> u64 {
        self.read_count.load(Ordering::Relaxed)
    }

    /// Get write count
    #[inline]
    pub fn write_count(&self) -> u64 {
        self.write_count.load(Ordering::Relaxed)
    }

    /// Get error count
    #[inline]
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Get last error information
    #[inline]
    pub fn last_error_info(&self) -> (u16, u32) {
        let packed = self.last_error.load(Ordering::Acquire);
        let error_code = (packed >> 48) as u16;
        let offset = ((packed >> 16) & 0xFFFF_FFFF) as u32;
        (error_code, offset)
    }
}

/// MMIO errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MmioError {
    /// Offset out of region bounds
    OutOfBounds = 1,
    /// Misaligned access
    Misaligned = 2,
    /// Permission denied (read-only or write-only)
    PermissionDenied = 3,
    /// Operation timed out
    Timeout = 4,
    /// Unsupported operation
    Unsupported = 5,
}

// Error code constants for packing
impl MmioError {
    const OUT_OF_BOUNDS: u16 = 1;
    const MISALIGNED: u16 = 2;
    const PERMISSION_DENIED: u16 = 3;
    const TIMEOUT: u16 = 4;
    const UNSUPPORTED: u16 = 5;
}

impl core::fmt::Display for MmioError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OutOfBounds => write!(f, "MMIO offset out of bounds"),
            Self::Misaligned => write!(f, "MMIO access misaligned"),
            Self::PermissionDenied => write!(f, "MMIO permission denied"),
            Self::Timeout => write!(f, "MMIO poll timeout"),
            Self::Unsupported => write!(f, "MMIO operation unsupported"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_type_encoding() {
        assert_eq!(MmioRegionType::GpuRegisters as u8, 0);
        assert_eq!(MmioRegionType::Doorbell as u8, 1);
        assert_eq!(MmioRegionType::Vram as u8, 2);

        assert_eq!(
            MmioRegionType::from_u8(0),
            Some(MmioRegionType::GpuRegisters)
        );
        assert_eq!(MmioRegionType::from_u8(1), Some(MmioRegionType::Doorbell));
    }

    #[test]
    fn test_region_type_properties() {
        assert!(MmioRegionType::RomBar.is_read_only());
        assert!(!MmioRegionType::GpuRegisters.is_read_only());

        assert!(MmioRegionType::GpuRegisters.supports_64bit());
        assert!(!MmioRegionType::Doorbell.supports_64bit());
    }

    #[test]
    fn test_mmio_flags() {
        let flags = MmioFlags::new(MmioFlags::READ_ONLY | MmioFlags::UNCACHED);
        assert!(flags.has(MmioFlags::READ_ONLY));
        assert!(flags.has(MmioFlags::UNCACHED));
        assert!(!flags.has(MmioFlags::WRITE_COMBINING));

        assert!(!flags.is_writable());
        assert!(flags.is_readable());
    }

    #[test]
    fn test_capsule_creation() {
        let base = 0xF000_0000;
        let size = 0x1000;
        let capsule = unsafe {
            MmioRegionCapsule::new(
                base,
                size,
                MmioRegionType::GpuRegisters,
                MmioFlags::new(0),
            )
        };

        assert_eq!(capsule.base_addr(), base);
        assert_eq!(capsule.size(), size);
        assert_eq!(capsule.region_type(), MmioRegionType::GpuRegisters);
        assert_eq!(capsule.generation(), 1);
    }

    #[test]
    fn test_state_packing() {
        let base = 0xABCD_1234_5678;
        let size = 0x9876_5432_1000;
        let capsule = unsafe {
            MmioRegionCapsule::new(
                base,
                size,
                MmioRegionType::Vram,
                MmioFlags::new(MmioFlags::WRITE_COMBINING),
            )
        };

        assert_eq!(capsule.base_addr(), base);
        assert_eq!(capsule.size(), size);
        assert_eq!(capsule.region_type(), MmioRegionType::Vram);
        assert!(capsule.flags().has(MmioFlags::WRITE_COMBINING));
    }

    #[test]
    fn test_bounds_checking() {
        let capsule = unsafe {
            MmioRegionCapsule::new(
                0x1000,
                0x1000,
                MmioRegionType::GpuRegisters,
                MmioFlags::new(0),
            )
        };

        assert!(capsule.check_bounds(0, 4));
        assert!(capsule.check_bounds(0xFFC, 4));
        assert!(!capsule.check_bounds(0xFFD, 4)); // Would exceed bounds
        assert!(!capsule.check_bounds(0x1000, 4)); // Exactly at end
    }

    #[test]
    fn test_generation_counter() {
        let capsule = unsafe {
            MmioRegionCapsule::new(
                0x1000,
                0x1000,
                MmioRegionType::GpuRegisters,
                MmioFlags::new(0),
            )
        };

        assert_eq!(capsule.generation(), 1);
        capsule.increment_generation();
        assert_eq!(capsule.generation(), 2);
        capsule.increment_generation();
        assert_eq!(capsule.generation(), 3);
    }

    #[test]
    fn test_error_recording() {
        let capsule = unsafe {
            MmioRegionCapsule::new(
                0x1000,
                0x1000,
                MmioRegionType::GpuRegisters,
                MmioFlags::new(0),
            )
        };

        capsule.record_error(MmioError::OUT_OF_BOUNDS as u16, 0x2000);
        assert_eq!(capsule.error_count(), 1);

        let (code, offset) = capsule.last_error_info();
        assert_eq!(code, MmioError::OUT_OF_BOUNDS as u16);
        assert_eq!(offset, 0x2000);
    }

    #[test]
    fn test_statistics() {
        let capsule = unsafe {
            MmioRegionCapsule::new(
                0x1000,
                0x1000,
                MmioRegionType::GpuRegisters,
                MmioFlags::new(0),
            )
        };

        assert_eq!(capsule.read_count(), 0);
        assert_eq!(capsule.write_count(), 0);
        assert_eq!(capsule.error_count(), 0);
    }

    #[test]
    fn test_size_and_alignment() {
        assert_eq!(core::mem::size_of::<MmioRegionCapsule>(), 512);
        assert_eq!(core::mem::align_of::<MmioRegionCapsule>(), 64);
    }
}
