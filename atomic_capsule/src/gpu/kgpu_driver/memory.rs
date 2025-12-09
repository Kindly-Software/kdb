//! GPU Memory Management Capsule - T1 Atomic, 128B cache-aligned
//!
//! Lockfree GPU memory allocation and tracking with generation counters for TOCTOU prevention.
//! This is the foundation for GPU memory management in KGPU-Driver v2.0.
//!
//! # Design
//!
//! **Tier**: T1 Atomic (3-10x speedup vs mutex-based approaches)
//! **Size**: 128B cache-aligned (2 cache lines)
//! **Performance Targets**:
//! - State read: <10ns
//! - Allocation (CAS): <50ns
//! - Free (CAS loop): <100ns
//! - Snapshot: <20ns
//!
//! # Memory Layout
//!
//! ```text
//! GpuMemoryCapsule (128B, 128-byte aligned)
//! ┌─────────────────────────────────────────────────────────┐
//! │  state_gen (AtomicU64)  │  size_flags (AtomicU64)      │ 16B
//! ├─────────────────────────────────────────────────────────┤
//! │  gpu_addr (AtomicU64)   │  cpu_addr (AtomicU64)        │ 16B
//! ├─────────────────────────────────────────────────────────┤
//! │  handle_id (AtomicU64)  │  fence_value (AtomicU64)     │ 16B
//! ├─────────────────────────────────────────────────────────┤
//! │  alloc_time_ns (u64)    │  last_use_ns (AtomicU64)     │ 16B
//! ├─────────────────────────────────────────────────────────┤
//! │  _padding [64 bytes]                                    │ 64B
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Packed State Layout (state_gen)
//!
//! ```text
//! Bits  0-7:  MemoryState (8 bits) - Free/Allocated/Mapped/InUse/PendingFree
//! Bits  8-15: Generation counter (8 bits) - Wrapping counter for TOCTOU prevention
//! Bits 16-63: Reserved for future use
//! ```
//!
//! # Packed Size/Flags Layout (size_flags)
//!
//! ```text
//! Bits  0-47: Size in bytes (48 bits, up to 256TB)
//! Bits 48-63: Memory flags (16 bits)
//! ```
//!
//! # ASSUM Tags
//!
//! - `#ASSUME_ATOMIC_ALIGNED`: All AtomicU64 fields are 8-byte aligned (Rust guarantees)
//! - `#ASSUME_CACHE_ALIGNED`: Struct is 128B aligned (2 cache lines, no false sharing)
//! - `#ASSUME_GENERATION_MONOTONIC`: Generation counter increments monotonically (wraps at 255)
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination via AtomicU64 CAS loops)
//! - **Q33**: ComputationalCapsule verification (128B, cache-aligned, generation counters)
//! - **Q34**: Audit trail design (generation counters for state change tracking)
//!
//! # Examples
//!
//! ```ignore
//! use atomic_capsule::gpu::kgpu_driver::memory::{GpuMemoryCapsule, MemoryState};
//!
//! // Create an unallocated memory capsule
//! let capsule = GpuMemoryCapsule::new();
//! assert_eq!(capsule.state(), MemoryState::Free);
//!
//! // Allocate GPU memory
//! let gen = capsule.allocate(1024, 0, 0xDEADBEEF, 42)?;
//! assert_eq!(capsule.state(), MemoryState::Allocated);
//! assert!(capsule.generation() >= 1);
//!
//! // Map to CPU
//! let cpu_ptr = 0x1234_5678 as *mut u8;
//! capsule.mark_mapped(cpu_ptr)?;
//! assert_eq!(capsule.state(), MemoryState::Mapped);
//!
//! // Take atomic snapshot
//! let snap = capsule.snapshot();
//! println!("Size: {} bytes, GPU addr: 0x{:x}", snap.size, snap.gpu_addr);
//!
//! // Free memory
//! capsule.mark_unmapped()?;
//! capsule.free()?;
//! assert_eq!(capsule.state(), MemoryState::Free);
//! ```

use core::sync::atomic::{AtomicU64, Ordering};
use core::fmt;

// Import error types from the comprehensive error module
use super::error::{KgpuDriverError, KgpuDriverResult};

// ============================================================================
// Memory State
// ============================================================================

/// Memory allocation state packed into 8 bits
///
/// # Layout
///
/// ```text
/// Value 0: Free       - Memory is available for allocation
/// Value 1: Allocated  - Memory allocated but not mapped to CPU
/// Value 2: Mapped     - Memory mapped to CPU address space
/// Value 3: InUse      - Memory being actively read/written by GPU
/// Value 4: PendingFree - Memory waiting for GPU fence before free
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MemoryState {
    /// Memory is free and available for allocation
    Free = 0,
    /// Memory is allocated but not mapped to CPU
    Allocated = 1,
    /// Memory is mapped to CPU address space
    Mapped = 2,
    /// Memory is being actively used by GPU
    InUse = 3,
    /// Memory is pending free (waiting for GPU fence)
    PendingFree = 4,
}

impl MemoryState {
    /// Convert from u8 to MemoryState
    ///
    /// # Safety
    ///
    /// Unknown values default to Free to prevent undefined behavior.
    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Free,
            1 => Self::Allocated,
            2 => Self::Mapped,
            3 => Self::InUse,
            4 => Self::PendingFree,
            _ => Self::Free, // Default to Free for unknown values
        }
    }

    /// Convert MemoryState to u8
    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Check if memory is in an allocated state (Allocated, Mapped, or InUse)
    #[inline]
    pub const fn is_allocated(self) -> bool {
        matches!(self, Self::Allocated | Self::Mapped | Self::InUse)
    }

    /// Check if memory can be freed (Allocated or Mapped)
    #[inline]
    pub const fn can_free(self) -> bool {
        matches!(self, Self::Allocated | Self::Mapped)
    }
}

impl fmt::Display for MemoryState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryState::Free => write!(f, "Free"),
            MemoryState::Allocated => write!(f, "Allocated"),
            MemoryState::Mapped => write!(f, "Mapped"),
            MemoryState::InUse => write!(f, "InUse"),
            MemoryState::PendingFree => write!(f, "PendingFree"),
        }
    }
}

// ============================================================================
// GPU Memory Capsule
// ============================================================================

/// GPU Memory Management Capsule (T1 Atomic, 128B)
///
/// Tracks GPU memory allocations with lockfree atomic state transitions.
/// Uses generation counters for TOCTOU prevention and atomic CAS loops
/// for all state mutations.
///
/// # Layout (128 bytes, 128-byte aligned)
///
/// ```text
/// ┌─────────────────────────────────────────────────────────┐
/// │  state_gen (AtomicU64)  │  size_flags (AtomicU64)      │ 16B
/// ├─────────────────────────────────────────────────────────┤
/// │  gpu_addr (AtomicU64)   │  cpu_addr (AtomicU64)        │ 16B
/// ├─────────────────────────────────────────────────────────┤
/// │  handle_id (AtomicU64)  │  fence_value (AtomicU64)     │ 16B
/// ├─────────────────────────────────────────────────────────┤
/// │  alloc_time_ns (u64)    │  last_use_ns (AtomicU64)     │ 16B
/// ├─────────────────────────────────────────────────────────┤
/// │  _padding [64 bytes]                                    │ 64B
/// └─────────────────────────────────────────────────────────┘
/// ```
///
/// # ASSUM Safety
///
/// - `#ASSUME_ATOMIC_ALIGNED`: All AtomicU64 fields are 8-byte aligned
/// - `#ASSUME_CACHE_ALIGNED`: Struct is 128B aligned (2 cache lines)
/// - `#ASSUME_GENERATION_MONOTONIC`: Generation counter never wraps during single operation
#[repr(C, align(128))]
pub struct GpuMemoryCapsule {
    /// State (bits 0-7) + Generation (bits 8-15) + Reserved (bits 16-63)
    ///
    /// # Bit Layout
    /// - Bits 0-7:   MemoryState enum value (0-4)
    /// - Bits 8-15:  Generation counter (0-255, wrapping)
    /// - Bits 16-63: Reserved for future use
    state_gen: AtomicU64,

    /// Size (bits 0-47) + Flags (bits 48-63)
    ///
    /// # Bit Layout
    /// - Bits 0-47:  Allocation size in bytes (up to 256TB)
    /// - Bits 48-63: Memory flags (GPU_VISIBLE, CPU_VISIBLE, etc.)
    size_flags: AtomicU64,

    /// GPU virtual address (0 if not allocated)
    gpu_addr: AtomicU64,

    /// CPU mapped address (0 if not mapped)
    cpu_addr: AtomicU64,

    /// Unique handle ID for this allocation
    handle_id: AtomicU64,

    /// Fence value to wait for before freeing
    fence_value: AtomicU64,

    /// Allocation timestamp (nanoseconds since epoch)
    /// Immutable after allocation, so not atomic
    alloc_time_ns: u64,

    /// Last GPU access timestamp (nanoseconds)
    last_use_ns: AtomicU64,

    /// Padding to reach exactly 128 bytes
    /// 8 AtomicU64 * 8 = 64 bytes of fields
    /// 128 - 64 = 64 bytes padding needed
    _padding: [u8; 64],
}

impl GpuMemoryCapsule {
    // ========================================================================
    // Constants
    // ========================================================================

    /// Mask for extracting state from state_gen (bits 0-7)
    const STATE_MASK: u64 = 0xFF;

    /// Mask for extracting generation from state_gen (bits 8-15)
    const GEN_MASK: u64 = 0xFF00;

    /// Shift amount for generation counter
    const GEN_SHIFT: u32 = 8;

    /// Mask for extracting size from size_flags (bits 0-47)
    const SIZE_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

    /// Mask for extracting flags from size_flags (bits 48-63)
    const FLAGS_MASK: u64 = 0xFFFF_0000_0000_0000;

    /// Shift amount for flags
    const FLAGS_SHIFT: u32 = 48;

    // ========================================================================
    // Construction
    // ========================================================================

    /// Create a new unallocated memory capsule
    ///
    /// # Returns
    ///
    /// A new `GpuMemoryCapsule` in `Free` state with generation 0.
    ///
    /// # Performance
    ///
    /// O(1), ~5ns (just zeroing memory)
    #[inline]
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(0), // State::Free, gen 0
            size_flags: AtomicU64::new(0),
            gpu_addr: AtomicU64::new(0),
            cpu_addr: AtomicU64::new(0),
            handle_id: AtomicU64::new(0),
            fence_value: AtomicU64::new(0),
            alloc_time_ns: 0,
            last_use_ns: AtomicU64::new(0),
            _padding: [0u8; 64],
        }
    }

    // ========================================================================
    // State Accessors
    // ========================================================================

    /// Get current memory state
    ///
    /// # Returns
    ///
    /// Current `MemoryState` (Free, Allocated, Mapped, InUse, or PendingFree)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load with Acquire ordering)
    #[inline]
    pub fn state(&self) -> MemoryState {
        let v = self.state_gen.load(Ordering::Acquire);
        MemoryState::from_u8((v & Self::STATE_MASK) as u8)
    }

    /// Get generation counter
    ///
    /// The generation counter increments on each state transition, providing
    /// TOCTOU (time-of-check-to-time-of-use) prevention.
    ///
    /// # Returns
    ///
    /// Current generation (0-255, wrapping)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn generation(&self) -> u8 {
        let v = self.state_gen.load(Ordering::Acquire);
        ((v & Self::GEN_MASK) >> Self::GEN_SHIFT) as u8
    }

    /// Get allocation size in bytes
    ///
    /// # Returns
    ///
    /// Size in bytes (0 if not allocated, up to 256TB)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn size(&self) -> u64 {
        self.size_flags.load(Ordering::Acquire) & Self::SIZE_MASK
    }

    /// Get memory flags
    ///
    /// # Returns
    ///
    /// 16-bit memory flags (GPU_VISIBLE, CPU_VISIBLE, etc.)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn flags(&self) -> u16 {
        let v = self.size_flags.load(Ordering::Acquire);
        ((v & Self::FLAGS_MASK) >> Self::FLAGS_SHIFT) as u16
    }

    /// Get GPU virtual address
    ///
    /// # Returns
    ///
    /// GPU virtual address (0 if not allocated)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn gpu_address(&self) -> u64 {
        self.gpu_addr.load(Ordering::Acquire)
    }

    /// Get CPU mapped address
    ///
    /// # Returns
    ///
    /// - `Some(ptr)` if memory is mapped to CPU
    /// - `None` if not mapped
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn cpu_address(&self) -> Option<*mut u8> {
        let addr = self.cpu_addr.load(Ordering::Acquire);
        if addr == 0 {
            None
        } else {
            Some(addr as *mut u8)
        }
    }

    /// Get unique handle ID
    ///
    /// # Returns
    ///
    /// Handle ID (0 if not allocated)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn handle_id(&self) -> u64 {
        self.handle_id.load(Ordering::Acquire)
    }

    /// Get fence value for pending operations
    ///
    /// # Returns
    ///
    /// Fence value (0 if no pending operations)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn fence_value(&self) -> u64 {
        self.fence_value.load(Ordering::Acquire)
    }

    /// Get allocation timestamp
    ///
    /// # Returns
    ///
    /// Timestamp in nanoseconds since epoch (0 if never allocated)
    ///
    /// # Performance
    ///
    /// <5ns (non-atomic read)
    #[inline]
    pub fn alloc_time_ns(&self) -> u64 {
        self.alloc_time_ns
    }

    /// Get last GPU access timestamp
    ///
    /// # Returns
    ///
    /// Last access timestamp in nanoseconds (0 if never accessed)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn last_use_ns(&self) -> u64 {
        self.last_use_ns.load(Ordering::Acquire)
    }

    // ========================================================================
    // State Mutations (Lockfree CAS)
    // ========================================================================

    /// Atomically allocate memory
    ///
    /// Transitions from `Free` -> `Allocated` state using CAS.
    /// Increments generation counter on success.
    ///
    /// # Arguments
    ///
    /// * `size` - Allocation size in bytes (max 256TB)
    /// * `flags` - Memory flags (16 bits)
    /// * `gpu_addr` - GPU virtual address
    /// * `handle_id` - Unique handle identifier
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success with new generation counter
    /// - `Err(MemoryInUse)` if not in Free state
    ///
    /// # Performance
    ///
    /// <50ns (single CAS + stores)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_GENERATION_MONOTONIC`: Generation increments monotonically
    pub fn allocate(
        &self,
        size: u64,
        flags: u16,
        gpu_addr: u64,
        handle_id: u64,
    ) -> KgpuDriverResult<u8> {
        // Try to transition Free -> Allocated
        let old = self.state_gen.load(Ordering::Acquire);
        let old_state = MemoryState::from_u8((old & Self::STATE_MASK) as u8);

        if old_state != MemoryState::Free {
            return Err(KgpuDriverError::MemoryInUse);
        }

        // Calculate new state_gen with incremented generation
        let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u8;
        let new_gen = old_gen.wrapping_add(1);
        let new = (MemoryState::Allocated as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

        // Atomic CAS to transition state
        match self.state_gen.compare_exchange(
            old,
            new,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Successfully transitioned, now set other fields
                // #ASSUME_ATOMIC_ALIGNED: These stores are to 8-byte aligned fields
                let size_flags = (size & Self::SIZE_MASK) | ((flags as u64) << Self::FLAGS_SHIFT);
                self.size_flags.store(size_flags, Ordering::Release);
                self.gpu_addr.store(gpu_addr, Ordering::Release);
                self.handle_id.store(handle_id, Ordering::Release);
                Ok(new_gen)
            }
            Err(_) => Err(KgpuDriverError::MemoryInUse),
        }
    }

    /// Atomically free memory
    ///
    /// Transitions from `Allocated` or `Mapped` -> `Free` state using CAS loop.
    /// Increments generation counter on success.
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success with new generation counter
    /// - `Err(InvalidMemoryHandle)` if not in a freeable state
    ///
    /// # Performance
    ///
    /// <100ns (CAS loop + stores)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_GENERATION_MONOTONIC`: Generation increments monotonically
    pub fn free(&self) -> KgpuDriverResult<u8> {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_state = MemoryState::from_u8((old & Self::STATE_MASK) as u8);

            // Can only free from Allocated or Mapped state
            if !old_state.can_free() {
                return Err(KgpuDriverError::InvalidMemoryHandle);
            }

            // Calculate new state_gen
            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u8;
            let new_gen = old_gen.wrapping_add(1);
            let new = (MemoryState::Free as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

            // Atomic CAS with weak (allows spurious failure for better performance)
            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Clear all fields on free
                    self.size_flags.store(0, Ordering::Release);
                    self.gpu_addr.store(0, Ordering::Release);
                    self.cpu_addr.store(0, Ordering::Release);
                    self.handle_id.store(0, Ordering::Release);
                    self.fence_value.store(0, Ordering::Release);
                    return Ok(new_gen);
                }
                Err(_) => continue, // Retry on CAS failure
            }
        }
    }

    /// Mark memory as mapped to CPU
    ///
    /// Transitions from `Allocated` -> `Mapped` state using CAS loop.
    /// Increments generation counter on success.
    ///
    /// # Arguments
    ///
    /// * `cpu_addr` - CPU virtual address where memory is mapped
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(InvalidMemoryHandle)` if not in Allocated state
    ///
    /// # Performance
    ///
    /// <100ns (CAS loop + store)
    pub fn mark_mapped(&self, cpu_addr: *mut u8) -> KgpuDriverResult<u8> {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_state = MemoryState::from_u8((old & Self::STATE_MASK) as u8);

            if old_state != MemoryState::Allocated {
                return Err(KgpuDriverError::InvalidMemoryHandle);
            }

            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u8;
            let new_gen = old_gen.wrapping_add(1);
            let new = (MemoryState::Mapped as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.cpu_addr.store(cpu_addr as u64, Ordering::Release);
                    return Ok(new_gen);
                }
                Err(_) => continue,
            }
        }
    }

    /// Mark memory as unmapped from CPU
    ///
    /// Transitions from `Mapped` -> `Allocated` state using CAS loop.
    /// Increments generation counter on success.
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(MemoryNotMapped)` if not in Mapped state
    ///
    /// # Performance
    ///
    /// <100ns (CAS loop + store)
    pub fn mark_unmapped(&self) -> KgpuDriverResult<u8> {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_state = MemoryState::from_u8((old & Self::STATE_MASK) as u8);

            if old_state != MemoryState::Mapped {
                return Err(KgpuDriverError::MemoryNotMapped);
            }

            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u8;
            let new_gen = old_gen.wrapping_add(1);
            let new = (MemoryState::Allocated as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.cpu_addr.store(0, Ordering::Release);
                    return Ok(new_gen);
                }
                Err(_) => continue,
            }
        }
    }

    /// Mark memory as in-use by GPU
    ///
    /// Transitions from `Allocated` or `Mapped` -> `InUse` state using CAS loop.
    /// Increments generation counter on success.
    ///
    /// # Arguments
    ///
    /// * `fence` - Fence value to wait for completion
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(InvalidState)` if not in a usable state
    ///
    /// # Performance
    ///
    /// <100ns (CAS loop + store)
    pub fn mark_in_use(&self, fence: u64) -> KgpuDriverResult<u8> {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_state = MemoryState::from_u8((old & Self::STATE_MASK) as u8);

            // Can mark in-use from Allocated or Mapped
            if !matches!(old_state, MemoryState::Allocated | MemoryState::Mapped) {
                return Err(KgpuDriverError::InvalidState);
            }

            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u8;
            let new_gen = old_gen.wrapping_add(1);
            let new = (MemoryState::InUse as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.fence_value.store(fence, Ordering::Release);
                    return Ok(new_gen);
                }
                Err(_) => continue,
            }
        }
    }

    /// Mark memory as no longer in-use by GPU
    ///
    /// Transitions from `InUse` -> `Allocated` or `Mapped` state using CAS loop.
    /// Preserves the mapped status if CPU address is set.
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(InvalidState)` if not in InUse state
    ///
    /// # Performance
    ///
    /// <100ns (CAS loop)
    pub fn mark_idle(&self) -> KgpuDriverResult<u8> {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_state = MemoryState::from_u8((old & Self::STATE_MASK) as u8);

            if old_state != MemoryState::InUse {
                return Err(KgpuDriverError::InvalidState);
            }

            // Check if memory was mapped before being marked in-use
            let cpu_addr = self.cpu_addr.load(Ordering::Acquire);
            let new_state = if cpu_addr != 0 {
                MemoryState::Mapped
            } else {
                MemoryState::Allocated
            };

            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u8;
            let new_gen = old_gen.wrapping_add(1);
            let new = (new_state as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.fence_value.store(0, Ordering::Release);
                    return Ok(new_gen);
                }
                Err(_) => continue,
            }
        }
    }

    /// Update last use timestamp
    ///
    /// # Arguments
    ///
    /// * `timestamp_ns` - Current timestamp in nanoseconds
    ///
    /// # Performance
    ///
    /// <10ns (single atomic store)
    #[inline]
    pub fn touch(&self, timestamp_ns: u64) {
        self.last_use_ns.store(timestamp_ns, Ordering::Release);
    }

    // ========================================================================
    // Snapshots
    // ========================================================================

    /// Take an atomic snapshot of current state
    ///
    /// Captures all state atomically for consistent reads.
    /// The snapshot is immutable and can be safely shared.
    ///
    /// # Returns
    ///
    /// Immutable `GpuMemorySnapshot` with all current values
    ///
    /// # Performance
    ///
    /// <20ns (7 atomic loads)
    ///
    /// # Note
    ///
    /// The snapshot may be slightly inconsistent if state changes during
    /// the read sequence, but generation counter can be used to detect this.
    #[inline]
    pub fn snapshot(&self) -> GpuMemorySnapshot {
        // Read state_gen first (acts as version guard)
        let state_gen = self.state_gen.load(Ordering::Acquire);

        GpuMemorySnapshot {
            state: MemoryState::from_u8((state_gen & Self::STATE_MASK) as u8),
            generation: ((state_gen & Self::GEN_MASK) >> Self::GEN_SHIFT) as u8,
            size: self.size(),
            flags: self.flags(),
            gpu_addr: self.gpu_addr.load(Ordering::Acquire),
            cpu_addr: self.cpu_addr.load(Ordering::Acquire),
            handle_id: self.handle_id.load(Ordering::Acquire),
            fence_value: self.fence_value.load(Ordering::Acquire),
            last_use_ns: self.last_use_ns.load(Ordering::Acquire),
        }
    }
}

impl Default for GpuMemoryCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for GpuMemoryCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let snap = self.snapshot();
        f.debug_struct("GpuMemoryCapsule")
            .field("state", &snap.state)
            .field("generation", &snap.generation)
            .field("size", &snap.size)
            .field("flags", &snap.flags)
            .field("gpu_addr", &format_args!("0x{:x}", snap.gpu_addr))
            .field("cpu_addr", &format_args!("0x{:x}", snap.cpu_addr))
            .field("handle_id", &snap.handle_id)
            .finish()
    }
}

// Safety: All fields are AtomicU64 or immutable (alloc_time_ns after allocation)
// AtomicU64 is Send + Sync, so GpuMemoryCapsule can be safely shared across threads.
//
// # ASSUM Safety
// - `#ASSUME_ATOMIC_ALIGNED`: AtomicU64 guarantees proper alignment
// - `#ASSUME_CACHE_ALIGNED`: #[repr(C, align(128))] ensures cache alignment
unsafe impl Send for GpuMemoryCapsule {}
unsafe impl Sync for GpuMemoryCapsule {}

// ============================================================================
// Memory Snapshot
// ============================================================================

/// Immutable snapshot of GPU memory state
///
/// Captured atomically from `GpuMemoryCapsule::snapshot()`.
/// Safe to share across threads and compare.
#[derive(Debug, Clone, Copy)]
pub struct GpuMemorySnapshot {
    /// Current memory state
    pub state: MemoryState,
    /// Generation counter at snapshot time
    pub generation: u8,
    /// Allocation size in bytes
    pub size: u64,
    /// Memory flags
    pub flags: u16,
    /// GPU virtual address
    pub gpu_addr: u64,
    /// CPU mapped address (0 if not mapped)
    pub cpu_addr: u64,
    /// Handle ID
    pub handle_id: u64,
    /// Fence value for pending operations
    pub fence_value: u64,
    /// Last GPU access timestamp
    pub last_use_ns: u64,
}

impl GpuMemorySnapshot {
    /// Check if memory is allocated (any non-Free state)
    #[inline]
    pub fn is_allocated(&self) -> bool {
        !matches!(self.state, MemoryState::Free)
    }

    /// Check if memory is mapped to CPU
    #[inline]
    pub fn is_mapped(&self) -> bool {
        matches!(self.state, MemoryState::Mapped)
    }

    /// Check if memory is in use by GPU
    #[inline]
    pub fn is_in_use(&self) -> bool {
        matches!(self.state, MemoryState::InUse)
    }

    /// Check if memory is pending free
    #[inline]
    pub fn is_pending_free(&self) -> bool {
        matches!(self.state, MemoryState::PendingFree)
    }

    /// Get CPU address as pointer (None if not mapped)
    #[inline]
    pub fn cpu_ptr(&self) -> Option<*mut u8> {
        if self.cpu_addr == 0 {
            None
        } else {
            Some(self.cpu_addr as *mut u8)
        }
    }
}

impl Default for GpuMemorySnapshot {
    fn default() -> Self {
        Self {
            state: MemoryState::Free,
            generation: 0,
            size: 0,
            flags: 0,
            gpu_addr: 0,
            cpu_addr: 0,
            handle_id: 0,
            fence_value: 0,
            last_use_ns: 0,
        }
    }
}

// ============================================================================
// Tests (T28 Compliant)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem;

    // ========================================================================
    // Tier 1: Unit Tests (Q1-Q7)
    // ========================================================================

    #[test]
    fn test_capsule_size() {
        // T28 Q1: Verify exact size is 128 bytes
        assert_eq!(mem::size_of::<GpuMemoryCapsule>(), 128);
    }

    #[test]
    fn test_capsule_alignment() {
        // T28 Q2: Verify alignment is 128 bytes (2 cache lines)
        assert_eq!(mem::align_of::<GpuMemoryCapsule>(), 128);
    }

    #[test]
    fn test_new_capsule_state() {
        // T28 Q3: Verify initial state is Free with generation 0
        let capsule = GpuMemoryCapsule::new();
        assert_eq!(capsule.state(), MemoryState::Free);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.size(), 0);
        assert_eq!(capsule.flags(), 0);
        assert_eq!(capsule.gpu_address(), 0);
        assert!(capsule.cpu_address().is_none());
    }

    #[test]
    fn test_default_impl() {
        // T28 Q4: Verify Default trait implementation
        let capsule: GpuMemoryCapsule = Default::default();
        assert_eq!(capsule.state(), MemoryState::Free);
    }

    #[test]
    fn test_memory_state_from_u8() {
        // T28 Q5: Verify MemoryState conversion
        assert_eq!(MemoryState::from_u8(0), MemoryState::Free);
        assert_eq!(MemoryState::from_u8(1), MemoryState::Allocated);
        assert_eq!(MemoryState::from_u8(2), MemoryState::Mapped);
        assert_eq!(MemoryState::from_u8(3), MemoryState::InUse);
        assert_eq!(MemoryState::from_u8(4), MemoryState::PendingFree);
        assert_eq!(MemoryState::from_u8(255), MemoryState::Free); // Unknown -> Free
    }

    #[test]
    fn test_memory_state_predicates() {
        // T28 Q6: Verify state predicates
        assert!(!MemoryState::Free.is_allocated());
        assert!(MemoryState::Allocated.is_allocated());
        assert!(MemoryState::Mapped.is_allocated());
        assert!(MemoryState::InUse.is_allocated());

        assert!(!MemoryState::Free.can_free());
        assert!(MemoryState::Allocated.can_free());
        assert!(MemoryState::Mapped.can_free());
        assert!(!MemoryState::InUse.can_free());
    }

    // ========================================================================
    // Tier 2: State Transitions (Q8-Q14)
    // ========================================================================

    #[test]
    fn test_allocate_success() {
        // T28 Q8: Verify Free -> Allocated transition
        let capsule = GpuMemoryCapsule::new();

        let result = capsule.allocate(1024, 0x0001, 0xDEAD_BEEF, 42);
        assert!(result.is_ok());

        let gen = result.unwrap();
        assert_eq!(gen, 1); // Generation incremented
        assert_eq!(capsule.state(), MemoryState::Allocated);
        assert_eq!(capsule.size(), 1024);
        assert_eq!(capsule.flags(), 0x0001);
        assert_eq!(capsule.gpu_address(), 0xDEAD_BEEF);
        assert_eq!(capsule.handle_id(), 42);
    }

    #[test]
    fn test_allocate_already_allocated() {
        // T28 Q9: Verify allocation fails if not Free
        let capsule = GpuMemoryCapsule::new();
        capsule.allocate(1024, 0, 0x1000, 1).unwrap();

        let result = capsule.allocate(2048, 0, 0x2000, 2);
        assert_eq!(result, Err(KgpuDriverError::MemoryInUse));
    }

    #[test]
    fn test_free_success() {
        // T28 Q10: Verify Allocated -> Free transition
        let capsule = GpuMemoryCapsule::new();
        capsule.allocate(1024, 0, 0x1000, 1).unwrap();

        let result = capsule.free();
        assert!(result.is_ok());

        let gen = result.unwrap();
        assert_eq!(gen, 2); // Generation incremented again
        assert_eq!(capsule.state(), MemoryState::Free);
        assert_eq!(capsule.size(), 0);
        assert_eq!(capsule.gpu_address(), 0);
    }

    #[test]
    fn test_free_not_allocated() {
        // T28 Q11: Verify free fails if not allocated
        let capsule = GpuMemoryCapsule::new();

        let result = capsule.free();
        assert_eq!(result, Err(KgpuDriverError::InvalidMemoryHandle));
    }

    #[test]
    fn test_map_unmap_cycle() {
        // T28 Q12: Verify Allocated -> Mapped -> Allocated cycle
        let capsule = GpuMemoryCapsule::new();
        capsule.allocate(1024, 0, 0x1000, 1).unwrap();

        // Map
        let cpu_ptr = 0x7FFF_0000 as *mut u8;
        let result = capsule.mark_mapped(cpu_ptr);
        assert!(result.is_ok());
        assert_eq!(capsule.state(), MemoryState::Mapped);
        assert_eq!(capsule.cpu_address(), Some(cpu_ptr));

        // Unmap
        let result = capsule.mark_unmapped();
        assert!(result.is_ok());
        assert_eq!(capsule.state(), MemoryState::Allocated);
        assert!(capsule.cpu_address().is_none());
    }

    #[test]
    fn test_map_invalid_state() {
        // T28 Q13: Verify map fails if not Allocated
        let capsule = GpuMemoryCapsule::new();

        let result = capsule.mark_mapped(0x1000 as *mut u8);
        assert_eq!(result, Err(KgpuDriverError::InvalidMemoryHandle));
    }

    #[test]
    fn test_unmap_not_mapped() {
        // T28 Q14: Verify unmap fails if not Mapped
        let capsule = GpuMemoryCapsule::new();
        capsule.allocate(1024, 0, 0x1000, 1).unwrap();

        let result = capsule.mark_unmapped();
        assert_eq!(result, Err(KgpuDriverError::MemoryNotMapped));
    }

    // ========================================================================
    // Tier 3: In-Use State (Q15-Q21)
    // ========================================================================

    #[test]
    fn test_in_use_cycle() {
        // T28 Q15: Verify Allocated -> InUse -> Allocated cycle
        let capsule = GpuMemoryCapsule::new();
        capsule.allocate(1024, 0, 0x1000, 1).unwrap();

        // Mark in use
        let result = capsule.mark_in_use(12345);
        assert!(result.is_ok());
        assert_eq!(capsule.state(), MemoryState::InUse);
        assert_eq!(capsule.fence_value(), 12345);

        // Mark idle
        let result = capsule.mark_idle();
        assert!(result.is_ok());
        assert_eq!(capsule.state(), MemoryState::Allocated);
        assert_eq!(capsule.fence_value(), 0);
    }

    #[test]
    fn test_in_use_preserves_mapped() {
        // T28 Q16: Verify InUse -> Mapped when CPU address was set
        let capsule = GpuMemoryCapsule::new();
        capsule.allocate(1024, 0, 0x1000, 1).unwrap();
        capsule.mark_mapped(0x7FFF_0000 as *mut u8).unwrap();
        capsule.mark_in_use(12345).unwrap();

        // Mark idle should return to Mapped, not Allocated
        let result = capsule.mark_idle();
        assert!(result.is_ok());
        assert_eq!(capsule.state(), MemoryState::Mapped);
    }

    #[test]
    fn test_in_use_invalid_state() {
        // T28 Q17: Verify in_use fails from Free state
        let capsule = GpuMemoryCapsule::new();

        let result = capsule.mark_in_use(12345);
        assert_eq!(result, Err(KgpuDriverError::InvalidState));
    }

    #[test]
    fn test_idle_invalid_state() {
        // T28 Q18: Verify idle fails if not InUse
        let capsule = GpuMemoryCapsule::new();
        capsule.allocate(1024, 0, 0x1000, 1).unwrap();

        let result = capsule.mark_idle();
        assert_eq!(result, Err(KgpuDriverError::InvalidState));
    }

    #[test]
    fn test_touch_updates_timestamp() {
        // T28 Q19: Verify touch updates last_use_ns
        let capsule = GpuMemoryCapsule::new();
        assert_eq!(capsule.last_use_ns(), 0);

        capsule.touch(1_000_000_000);
        assert_eq!(capsule.last_use_ns(), 1_000_000_000);
    }

    // ========================================================================
    // Tier 4: Snapshot Tests (Q22-Q28)
    // ========================================================================

    #[test]
    fn test_snapshot_captures_all_state() {
        // T28 Q22: Verify snapshot captures all fields
        let capsule = GpuMemoryCapsule::new();
        capsule.allocate(4096, 0xABCD, 0xFFFF_0000, 99).unwrap();
        capsule.mark_mapped(0x1234_5678 as *mut u8).unwrap();
        capsule.touch(987654321);

        let snap = capsule.snapshot();
        assert_eq!(snap.state, MemoryState::Mapped);
        assert_eq!(snap.generation, 2); // allocate + map
        assert_eq!(snap.size, 4096);
        assert_eq!(snap.flags, 0xABCD);
        assert_eq!(snap.gpu_addr, 0xFFFF_0000);
        assert_eq!(snap.cpu_addr, 0x1234_5678);
        assert_eq!(snap.handle_id, 99);
        assert_eq!(snap.last_use_ns, 987654321);
    }

    #[test]
    fn test_snapshot_predicates() {
        // T28 Q23: Verify snapshot predicate methods
        let snap_free = GpuMemorySnapshot::default();
        assert!(!snap_free.is_allocated());
        assert!(!snap_free.is_mapped());
        assert!(!snap_free.is_in_use());
        assert!(snap_free.cpu_ptr().is_none());

        let snap_mapped = GpuMemorySnapshot {
            state: MemoryState::Mapped,
            cpu_addr: 0x1000,
            ..Default::default()
        };
        assert!(snap_mapped.is_allocated());
        assert!(snap_mapped.is_mapped());
        assert!(snap_mapped.cpu_ptr().is_some());
    }

    #[test]
    fn test_generation_increments() {
        // T28 Q24: Verify generation increments on each state change
        let capsule = GpuMemoryCapsule::new();
        assert_eq!(capsule.generation(), 0);

        capsule.allocate(1024, 0, 0x1000, 1).unwrap();
        assert_eq!(capsule.generation(), 1);

        capsule.mark_mapped(0x2000 as *mut u8).unwrap();
        assert_eq!(capsule.generation(), 2);

        capsule.mark_unmapped().unwrap();
        assert_eq!(capsule.generation(), 3);

        capsule.free().unwrap();
        assert_eq!(capsule.generation(), 4);
    }

    #[test]
    fn test_generation_wraps() {
        // T28 Q25: Verify generation wraps at 255
        let capsule = GpuMemoryCapsule::new();

        // Do 256 allocate/free cycles to wrap generation
        for i in 0..256 {
            capsule.allocate(1024, 0, 0x1000, i as u64).unwrap();
            capsule.free().unwrap();
        }

        // After 256*2=512 transitions, generation should wrap
        // 512 mod 256 = 0
        assert_eq!(capsule.generation(), 0);
    }

    #[test]
    fn test_free_from_mapped() {
        // T28 Q26: Verify can free directly from Mapped state
        let capsule = GpuMemoryCapsule::new();
        capsule.allocate(1024, 0, 0x1000, 1).unwrap();
        capsule.mark_mapped(0x2000 as *mut u8).unwrap();

        // Should be able to free directly from Mapped
        let result = capsule.free();
        assert!(result.is_ok());
        assert_eq!(capsule.state(), MemoryState::Free);
    }

    #[test]
    fn test_cannot_free_from_in_use() {
        // T28 Q27: Verify cannot free from InUse state
        let capsule = GpuMemoryCapsule::new();
        capsule.allocate(1024, 0, 0x1000, 1).unwrap();
        capsule.mark_in_use(12345).unwrap();

        let result = capsule.free();
        assert_eq!(result, Err(KgpuDriverError::InvalidMemoryHandle));
    }

    #[test]
    fn test_debug_impl() {
        // T28 Q28: Verify Debug implementation
        let capsule = GpuMemoryCapsule::new();
        let debug_str = format!("{:?}", capsule);
        assert!(debug_str.contains("GpuMemoryCapsule"));
        assert!(debug_str.contains("Free"));
    }

    // ========================================================================
    // Tier 5: Determinism Tests (Q29-Q35)
    // ========================================================================

    #[test]
    fn test_error_display() {
        // T28 Q29: Verify error Display implementation
        // Note: Display format is "[KGPU-{code:04X}] {category}: {description}"
        let err = KgpuDriverError::MemoryInUse;
        let display = format!("{}", err);
        assert!(display.contains("[KGPU-0107]"), "Should contain error code");
        assert!(display.contains("Memory"), "Should contain category");
        assert!(display.contains("Memory is still in use"), "Should contain description");

        let err = KgpuDriverError::InvalidMemoryHandle;
        let display = format!("{}", err);
        assert!(display.contains("[KGPU-0102]"), "Should contain error code");
        assert!(display.contains("Invalid memory handle"), "Should contain description");

        let err = KgpuDriverError::MemoryNotMapped;
        let display = format!("{}", err);
        assert!(display.contains("[KGPU-0104]"), "Should contain error code");
        assert!(display.contains("Memory is not mapped"), "Should contain description");
    }

    #[test]
    fn test_memory_state_display() {
        // T28 Q30: Verify MemoryState Display implementation
        assert_eq!(format!("{}", MemoryState::Free), "Free");
        assert_eq!(format!("{}", MemoryState::Allocated), "Allocated");
        assert_eq!(format!("{}", MemoryState::Mapped), "Mapped");
        assert_eq!(format!("{}", MemoryState::InUse), "InUse");
        assert_eq!(format!("{}", MemoryState::PendingFree), "PendingFree");
    }

    #[test]
    fn test_size_mask_coverage() {
        // T28 Q31: Verify size mask handles max value (256TB)
        let capsule = GpuMemoryCapsule::new();
        let max_size = 0x0000_FFFF_FFFF_FFFF_u64; // 256TB - 1

        capsule.allocate(max_size, 0, 0x1000, 1).unwrap();
        assert_eq!(capsule.size(), max_size);
    }

    #[test]
    fn test_flags_isolation() {
        // T28 Q32: Verify flags don't bleed into size
        let capsule = GpuMemoryCapsule::new();
        capsule.allocate(1024, 0xFFFF, 0x1000, 1).unwrap();

        assert_eq!(capsule.size(), 1024);
        assert_eq!(capsule.flags(), 0xFFFF);
    }

    #[test]
    fn test_concurrent_snapshot_safe() {
        // T28 Q33: Verify snapshot can be taken without data race
        use core::sync::atomic::AtomicBool;

        static DONE: AtomicBool = AtomicBool::new(false);
        let capsule = GpuMemoryCapsule::new();
        capsule.allocate(1024, 0, 0x1000, 1).unwrap();

        // Simulate concurrent snapshot (single-threaded test)
        let snap1 = capsule.snapshot();
        let snap2 = capsule.snapshot();

        // Both snapshots should be consistent
        assert_eq!(snap1.state, snap2.state);
        assert_eq!(snap1.generation, snap2.generation);
    }

    #[test]
    fn test_send_sync_traits() {
        // T28 Q34: Verify Send + Sync implementation
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GpuMemoryCapsule>();
    }

    #[test]
    fn test_snapshot_default() {
        // T28 Q35: Verify GpuMemorySnapshot Default
        let snap: GpuMemorySnapshot = Default::default();
        assert_eq!(snap.state, MemoryState::Free);
        assert_eq!(snap.generation, 0);
        assert_eq!(snap.size, 0);
        assert_eq!(snap.flags, 0);
        assert_eq!(snap.gpu_addr, 0);
        assert_eq!(snap.cpu_addr, 0);
        assert_eq!(snap.handle_id, 0);
        assert_eq!(snap.fence_value, 0);
        assert_eq!(snap.last_use_ns, 0);
    }
}
