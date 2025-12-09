//! Metal Buffer Capsule - T1 Atomic, 128B cache-aligned
//!
//! Represents a Metal GPU buffer (MTLBuffer) with storage mode management.
//! This is a MOCK implementation for design validation, not real Metal FFI.
//!
//! # Design
//!
//! **Tier**: T1 Atomic (<100ns operations)
//! **Size**: 128B cache-aligned (two 64-byte cache lines)
//! **Performance Targets**:
//! - State query: <10ns (single atomic load)
//! - Map/unmap: <50ns (CAS + pointer operations)
//! - GPU address query: <10ns (atomic load)
//!
//! # Memory Layout
//!
//! ```text
//! MtlBufferCapsule (128B, two cache lines)
//! +-- Cache Line 0 (64B): Handle + primary coordination
//! |   +-- handle: AtomicU64 (8B) - Mock MTLBuffer handle
//! |   +-- primary: AtomicU64 (8B) - state(8)|storage_mode(8)|generation(48)
//! |   +-- length: AtomicU64 (8B) - Buffer size in bytes
//! |   +-- contents_ptr: AtomicPtr<u8> (8B) - CPU-accessible pointer
//! |   +-- _padding0: [u8; 32]
//! +-- Cache Line 1 (64B): GPU address + metadata
//!     +-- gpu_address: AtomicU64 (8B) - GPU virtual address
//!     +-- label_hash: AtomicU64 (8B) - Hash of buffer label
//!     +-- _padding1: [u8; 48]
//! ```
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_MOCK_HANDLE`: handle is a mock value, not a real MTLBuffer pointer
//! - `#ASSUME_CONTENTS_PTR_MOCK`: contents_ptr is mock, not a real mapped pointer
//! - `#ASSUME_STATE_MACHINE_VALID`: State transitions validated via CAS
//! - `#ASSUME_GENERATION_MONOTONIC`: Generation counter only increases
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination)
//! - **Q33**: 128B alignment verified at compile time
//! - **Q34**: Generation counter enables audit trail integration

use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

use super::types::MTLStorageMode;

// ============================================================================
// State Constants
// ============================================================================

/// Buffer state: Uninitialized
pub const BUFFER_STATE_UNINITIALIZED: u8 = 0;
/// Buffer state: Created (not mapped)
pub const BUFFER_STATE_CREATED: u8 = 1;
/// Buffer state: Mapped for CPU access
pub const BUFFER_STATE_MAPPED: u8 = 2;
/// Buffer state: In GPU use (submitted to command buffer)
pub const BUFFER_STATE_IN_GPU_USE: u8 = 3;
/// Buffer state: Destroyed
pub const BUFFER_STATE_DESTROYED: u8 = 4;

// ============================================================================
// Bit Field Layouts
// ============================================================================

// Primary atomic: state(8) | storage_mode(8) | generation(48)
const STATE_SHIFT: u32 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;
const STORAGE_MODE_SHIFT: u32 = 48;
const STORAGE_MODE_MASK: u64 = 0xFF << STORAGE_MODE_SHIFT;
const GENERATION_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during Metal buffer operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtlBufferError {
    /// Buffer is in invalid state for the requested operation
    InvalidState {
        /// Current state
        current: u8,
        /// Expected state
        expected: u8,
    },
    /// State transition failed
    TransitionFailed {
        /// Expected state
        expected: u8,
        /// Observed state
        observed: u8,
    },
    /// Buffer has been destroyed
    BufferDestroyed,
    /// Cannot map this storage mode (Private/Memoryless)
    CannotMap {
        /// Storage mode that cannot be mapped
        storage_mode: MTLStorageMode,
    },
    /// Buffer is currently in GPU use
    InGpuUse,
    /// Invalid buffer length
    InvalidLength {
        /// The invalid length
        length: u64,
    },
}

impl core::fmt::Display for MtlBufferError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidState { current, expected } => {
                write!(f, "Invalid buffer state: current={}, expected={}", current, expected)
            }
            Self::TransitionFailed { expected, observed } => {
                write!(f, "Buffer transition failed: expected={}, observed={}", expected, observed)
            }
            Self::BufferDestroyed => write!(f, "Buffer has been destroyed"),
            Self::CannotMap { storage_mode } => {
                write!(f, "Cannot map storage mode {:?}", storage_mode)
            }
            Self::InGpuUse => write!(f, "Buffer is in GPU use"),
            Self::InvalidLength { length } => {
                write!(f, "Invalid buffer length: {}", length)
            }
        }
    }
}

/// Result type for Metal buffer operations
pub type MtlBufferResult<T> = Result<T, MtlBufferError>;

// ============================================================================
// Buffer Snapshot
// ============================================================================

/// Atomic snapshot of buffer state for debugging/monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MtlBufferSnapshot {
    /// Mock buffer handle
    pub handle: u64,
    /// Current state (0-4)
    pub state: u8,
    /// Storage mode
    pub storage_mode: MTLStorageMode,
    /// Generation counter
    pub generation: u64,
    /// Buffer length in bytes
    pub length: u64,
    /// Whether contents_ptr is non-null
    pub has_contents: bool,
    /// GPU virtual address
    pub gpu_address: u64,
}

// ============================================================================
// MtlBufferCapsule
// ============================================================================

/// Metal Buffer Capsule - Represents a Metal GPU buffer
///
/// Manages buffer state, mapping, and GPU address tracking.
/// All operations are lockfree using atomic primitives.
///
/// # Tier: T1 Atomic
/// # Size: 128B (two cache lines, prevents false sharing)
///
/// # State Machine
///
/// - `Uninitialized` (0): Buffer not yet created
/// - `Created` (1): Buffer created, not mapped
/// - `Mapped` (2): Buffer mapped for CPU access
/// - `InGpuUse` (3): Buffer submitted to GPU
/// - `Destroyed` (4): Buffer destroyed
///
/// # ASSUM Safety
///
/// - `#ASSUME_MOCK_HANDLE`: handle is mock, not real MTLBuffer
/// - `#ASSUME_CONTENTS_PTR_MOCK`: contents_ptr is mock
/// - `#ASSUME_STATE_MACHINE_VALID`: State transitions validated via CAS
#[repr(C, align(128))]
pub struct MtlBufferCapsule {
    // ========================================================================
    // Cache Line 0: Handle + primary coordination
    // ========================================================================
    /// Mock MTLBuffer handle
    ///
    /// #ASSUME_MOCK_HANDLE: This is a mock value for testing.
    handle: AtomicU64,

    /// Primary coordination channel
    ///
    /// Layout: state(8) | storage_mode(8) | generation(48)
    primary: AtomicU64,

    /// Buffer length in bytes
    length: AtomicU64,

    /// CPU-accessible pointer (for Shared/Managed storage)
    ///
    /// #ASSUME_CONTENTS_PTR_MOCK: This is mock for testing.
    contents_ptr: AtomicPtr<u8>,

    /// Padding to complete first cache line
    _padding0: [u8; 32],

    // ========================================================================
    // Cache Line 1: GPU address + metadata
    // ========================================================================
    /// GPU virtual address
    gpu_address: AtomicU64,

    /// Hash of buffer label (for debugging)
    label_hash: AtomicU64,

    /// Padding to complete second cache line
    _padding1: [u8; 48],
}

// Compile-time size and alignment verification
const _: () = {
    assert!(core::mem::size_of::<MtlBufferCapsule>() == 128);
    assert!(core::mem::align_of::<MtlBufferCapsule>() == 128);
};

impl MtlBufferCapsule {
    /// Creates a new buffer in `Uninitialized` state.
    ///
    /// # Performance
    ///
    /// O(1), ~10ns (stack allocation + atomic init)
    #[inline]
    pub const fn new() -> Self {
        Self {
            handle: AtomicU64::new(0),
            primary: AtomicU64::new(0),
            length: AtomicU64::new(0),
            contents_ptr: AtomicPtr::new(ptr::null_mut()),
            _padding0: [0u8; 32],

            gpu_address: AtomicU64::new(0),
            label_hash: AtomicU64::new(0),
            _padding1: [0u8; 48],
        }
    }

    /// Returns the mock buffer handle.
    #[inline]
    pub fn handle(&self) -> u64 {
        self.handle.load(Ordering::Acquire)
    }

    /// Returns the current state.
    #[inline]
    pub fn state(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & STATE_MASK) >> STATE_SHIFT) as u8
    }

    /// Returns the storage mode.
    #[inline]
    pub fn storage_mode(&self) -> MTLStorageMode {
        let primary = self.primary.load(Ordering::Acquire);
        let mode = ((primary & STORAGE_MODE_MASK) >> STORAGE_MODE_SHIFT) as u32;
        match mode {
            0 => MTLStorageMode::Shared,
            1 => MTLStorageMode::Managed,
            2 => MTLStorageMode::Private,
            3 => MTLStorageMode::Memoryless,
            _ => MTLStorageMode::Shared,
        }
    }

    /// Returns the generation counter.
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Returns the buffer length in bytes.
    #[inline]
    pub fn length(&self) -> u64 {
        self.length.load(Ordering::Acquire)
    }

    /// Returns the GPU virtual address.
    #[inline]
    pub fn gpu_address(&self) -> u64 {
        self.gpu_address.load(Ordering::Acquire)
    }

    /// Returns the contents pointer (may be null for Private storage).
    #[inline]
    pub fn contents_ptr(&self) -> *mut u8 {
        self.contents_ptr.load(Ordering::Acquire)
    }

    /// Takes an atomic snapshot of the buffer state.
    ///
    /// # Performance
    ///
    /// ~30ns (multiple atomic loads)
    pub fn snapshot(&self) -> MtlBufferSnapshot {
        let primary = self.primary.load(Ordering::Acquire);

        MtlBufferSnapshot {
            handle: self.handle.load(Ordering::Acquire),
            state: ((primary & STATE_MASK) >> STATE_SHIFT) as u8,
            storage_mode: self.storage_mode(),
            generation: primary & GENERATION_MASK,
            length: self.length.load(Ordering::Acquire),
            has_contents: !self.contents_ptr.load(Ordering::Acquire).is_null(),
            gpu_address: self.gpu_address.load(Ordering::Acquire),
        }
    }

    /// Creates the buffer with the specified parameters.
    ///
    /// # Performance
    ///
    /// <100ns (CAS + atomic stores)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_STATE_MACHINE_VALID`: Validates transition is legal
    /// - `#ASSUME_MOCK_HANDLE`: Sets mock handle value
    pub fn create(
        &self,
        mock_handle: u64,
        length: u64,
        storage_mode: MTLStorageMode,
    ) -> MtlBufferResult<()> {
        if length == 0 {
            return Err(MtlBufferError::InvalidLength { length });
        }

        // #ASSUME_STATE_MACHINE_VALID: Transition from Uninitialized to Created
        let current = self.primary.load(Ordering::Acquire);
        let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

        if current_state != BUFFER_STATE_UNINITIALIZED {
            return Err(MtlBufferError::InvalidState {
                current: current_state,
                expected: BUFFER_STATE_UNINITIALIZED,
            });
        }

        // Build new primary value
        let current_gen = current & GENERATION_MASK;
        let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
        let new_primary = ((BUFFER_STATE_CREATED as u64) << STATE_SHIFT)
            | ((storage_mode as u64) << STORAGE_MODE_SHIFT)
            | new_gen;

        match self.primary.compare_exchange(
            current,
            new_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {}
            Err(observed) => {
                let observed_state = ((observed & STATE_MASK) >> STATE_SHIFT) as u8;
                return Err(MtlBufferError::TransitionFailed {
                    expected: BUFFER_STATE_UNINITIALIZED,
                    observed: observed_state,
                });
            }
        }

        // Set handle, length, and GPU address
        self.handle.store(mock_handle, Ordering::Release);
        self.length.store(length, Ordering::Release);

        // Mock GPU address (based on handle)
        let mock_gpu_addr = 0x0001_0000_0000_0000u64 | (mock_handle & 0xFFFF_FFFF);
        self.gpu_address.store(mock_gpu_addr, Ordering::Release);

        // Set contents pointer for CPU-accessible storage modes
        // #ASSUME_CONTENTS_PTR_MOCK: This would be the real mapped pointer in production
        if storage_mode.is_cpu_accessible() {
            // Mock pointer value (non-null sentinel)
            let mock_ptr = 0x7FFF_0000_0000_0000usize as *mut u8;
            self.contents_ptr.store(mock_ptr, Ordering::Release);
        }

        Ok(())
    }

    /// Maps the buffer for CPU access.
    ///
    /// # Performance
    ///
    /// <50ns (CAS)
    ///
    /// # Returns
    ///
    /// Mock pointer to buffer contents on success.
    pub fn map(&self) -> MtlBufferResult<*mut u8> {
        let storage_mode = self.storage_mode();

        // Cannot map Private or Memoryless storage
        if !storage_mode.is_cpu_accessible() {
            return Err(MtlBufferError::CannotMap { storage_mode });
        }

        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;
            let mode = (current & STORAGE_MODE_MASK) >> STORAGE_MODE_SHIFT;
            let current_gen = current & GENERATION_MASK;

            match current_state {
                BUFFER_STATE_DESTROYED => return Err(MtlBufferError::BufferDestroyed),
                BUFFER_STATE_IN_GPU_USE => return Err(MtlBufferError::InGpuUse),
                BUFFER_STATE_MAPPED => {
                    // Already mapped, return pointer
                    return Ok(self.contents_ptr.load(Ordering::Acquire));
                }
                BUFFER_STATE_CREATED => {
                    // Transition to Mapped
                    let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
                    let new_primary =
                        ((BUFFER_STATE_MAPPED as u64) << STATE_SHIFT) | (mode << STORAGE_MODE_SHIFT) | new_gen;

                    if self
                        .primary
                        .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        return Ok(self.contents_ptr.load(Ordering::Acquire));
                    }
                    // Retry on CAS failure
                }
                _ => {
                    return Err(MtlBufferError::InvalidState {
                        current: current_state,
                        expected: BUFFER_STATE_CREATED,
                    });
                }
            }
        }
    }

    /// Unmaps the buffer.
    ///
    /// # Performance
    ///
    /// <50ns (CAS)
    pub fn unmap(&self) -> MtlBufferResult<()> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;
            let mode = (current & STORAGE_MODE_MASK) >> STORAGE_MODE_SHIFT;
            let current_gen = current & GENERATION_MASK;

            if current_state == BUFFER_STATE_DESTROYED {
                return Err(MtlBufferError::BufferDestroyed);
            }

            if current_state != BUFFER_STATE_MAPPED {
                // Already unmapped or in different state
                return Ok(());
            }

            // Transition back to Created
            let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
            let new_primary =
                ((BUFFER_STATE_CREATED as u64) << STATE_SHIFT) | (mode << STORAGE_MODE_SHIFT) | new_gen;

            if self
                .primary
                .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
            // Retry on CAS failure
        }
    }

    /// Marks the buffer as in GPU use (submitted to command buffer).
    ///
    /// # Performance
    ///
    /// <50ns (CAS)
    pub fn mark_gpu_use(&self) -> MtlBufferResult<()> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;
            let mode = (current & STORAGE_MODE_MASK) >> STORAGE_MODE_SHIFT;
            let current_gen = current & GENERATION_MASK;

            match current_state {
                BUFFER_STATE_DESTROYED => return Err(MtlBufferError::BufferDestroyed),
                BUFFER_STATE_IN_GPU_USE => return Ok(()), // Already in GPU use
                BUFFER_STATE_CREATED => {
                    let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
                    let new_primary =
                        ((BUFFER_STATE_IN_GPU_USE as u64) << STATE_SHIFT) | (mode << STORAGE_MODE_SHIFT) | new_gen;

                    if self
                        .primary
                        .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        return Ok(());
                    }
                }
                _ => {
                    return Err(MtlBufferError::InvalidState {
                        current: current_state,
                        expected: BUFFER_STATE_CREATED,
                    });
                }
            }
        }
    }

    /// Marks the buffer as no longer in GPU use.
    ///
    /// # Performance
    ///
    /// <50ns (CAS)
    pub fn clear_gpu_use(&self) -> MtlBufferResult<()> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;
            let mode = (current & STORAGE_MODE_MASK) >> STORAGE_MODE_SHIFT;
            let current_gen = current & GENERATION_MASK;

            if current_state == BUFFER_STATE_DESTROYED {
                return Err(MtlBufferError::BufferDestroyed);
            }

            if current_state != BUFFER_STATE_IN_GPU_USE {
                return Ok(()); // Not in GPU use
            }

            // Transition back to Created
            let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
            let new_primary =
                ((BUFFER_STATE_CREATED as u64) << STATE_SHIFT) | (mode << STORAGE_MODE_SHIFT) | new_gen;

            if self
                .primary
                .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
        }
    }

    /// Destroys the buffer.
    ///
    /// # Performance
    ///
    /// <50ns (CAS)
    pub fn destroy(&self) -> MtlBufferResult<()> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;
            let current_gen = current & GENERATION_MASK;

            if current_state == BUFFER_STATE_DESTROYED {
                return Err(MtlBufferError::BufferDestroyed);
            }

            let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
            let destroyed = ((BUFFER_STATE_DESTROYED as u64) << STATE_SHIFT) | new_gen;

            if self
                .primary
                .compare_exchange(current, destroyed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // Clear the contents pointer
                self.contents_ptr.store(ptr::null_mut(), Ordering::Release);
                return Ok(());
            }
        }
    }

    /// Checks if the buffer is valid (not Destroyed or Uninitialized).
    #[inline]
    pub fn is_valid(&self) -> bool {
        let state = self.state();
        state != BUFFER_STATE_DESTROYED && state != BUFFER_STATE_UNINITIALIZED
    }

    /// Checks if the buffer is mapped.
    #[inline]
    pub fn is_mapped(&self) -> bool {
        self.state() == BUFFER_STATE_MAPPED
    }

    /// Checks if the buffer is in GPU use.
    #[inline]
    pub fn is_in_gpu_use(&self) -> bool {
        self.state() == BUFFER_STATE_IN_GPU_USE
    }

    /// Sets the label hash for debugging.
    #[inline]
    pub fn set_label_hash(&self, hash: u64) {
        self.label_hash.store(hash, Ordering::Release);
    }

    /// Gets the label hash.
    #[inline]
    pub fn label_hash(&self) -> u64 {
        self.label_hash.load(Ordering::Acquire)
    }
}

impl Default for MtlBufferCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for MtlBufferCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let snapshot = self.snapshot();
        f.debug_struct("MtlBufferCapsule")
            .field("handle", &format_args!("{:#018x}", snapshot.handle))
            .field("state", &snapshot.state)
            .field("storage_mode", &snapshot.storage_mode)
            .field("length", &snapshot.length)
            .field("gpu_address", &format_args!("{:#018x}", snapshot.gpu_address))
            .finish()
    }
}

// SAFETY: All operations are atomic; no mutable aliasing possible
unsafe impl Send for MtlBufferCapsule {}
unsafe impl Sync for MtlBufferCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<MtlBufferCapsule>(), 128);
        assert_eq!(core::mem::align_of::<MtlBufferCapsule>(), 128);
    }

    #[test]
    fn test_initial_state() {
        let buffer = MtlBufferCapsule::new();
        assert_eq!(buffer.state(), BUFFER_STATE_UNINITIALIZED);
        assert_eq!(buffer.handle(), 0);
        assert_eq!(buffer.length(), 0);
        assert_eq!(buffer.gpu_address(), 0);
        assert!(buffer.contents_ptr().is_null());
    }

    #[test]
    fn test_create_shared() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x1234, 4096, MTLStorageMode::Shared)
            .expect("Create failed");

        assert_eq!(buffer.state(), BUFFER_STATE_CREATED);
        assert_eq!(buffer.handle(), 0x1234);
        assert_eq!(buffer.length(), 4096);
        assert_eq!(buffer.storage_mode(), MTLStorageMode::Shared);
        assert!(!buffer.contents_ptr().is_null());
        assert_ne!(buffer.gpu_address(), 0);
    }

    #[test]
    fn test_create_private() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x5678, 8192, MTLStorageMode::Private)
            .expect("Create failed");

        assert_eq!(buffer.state(), BUFFER_STATE_CREATED);
        assert_eq!(buffer.storage_mode(), MTLStorageMode::Private);
        // Private storage should have null contents_ptr
        assert!(buffer.contents_ptr().is_null());
    }

    #[test]
    fn test_create_zero_length_fails() {
        let buffer = MtlBufferCapsule::new();
        let result = buffer.create(0x1234, 0, MTLStorageMode::Shared);
        assert!(matches!(result, Err(MtlBufferError::InvalidLength { length: 0 })));
    }

    #[test]
    fn test_double_create_fails() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x1234, 4096, MTLStorageMode::Shared)
            .unwrap();

        let result = buffer.create(0x5678, 8192, MTLStorageMode::Private);
        assert!(result.is_err());
    }

    #[test]
    fn test_map_shared() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x1234, 4096, MTLStorageMode::Shared)
            .unwrap();

        let ptr = buffer.map().expect("Map failed");
        assert!(!ptr.is_null());
        assert_eq!(buffer.state(), BUFFER_STATE_MAPPED);
        assert!(buffer.is_mapped());
    }

    #[test]
    fn test_map_private_fails() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x1234, 4096, MTLStorageMode::Private)
            .unwrap();

        let result = buffer.map();
        assert!(matches!(result, Err(MtlBufferError::CannotMap { .. })));
    }

    #[test]
    fn test_unmap() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x1234, 4096, MTLStorageMode::Shared)
            .unwrap();

        buffer.map().unwrap();
        assert!(buffer.is_mapped());

        buffer.unmap().expect("Unmap failed");
        assert!(!buffer.is_mapped());
        assert_eq!(buffer.state(), BUFFER_STATE_CREATED);
    }

    #[test]
    fn test_gpu_use() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x1234, 4096, MTLStorageMode::Private)
            .unwrap();

        buffer.mark_gpu_use().expect("Mark GPU use failed");
        assert!(buffer.is_in_gpu_use());
        assert_eq!(buffer.state(), BUFFER_STATE_IN_GPU_USE);

        // Cannot map while in GPU use
        let result = buffer.map();
        assert!(matches!(result, Err(MtlBufferError::CannotMap { .. })));
    }

    #[test]
    fn test_clear_gpu_use() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x1234, 4096, MTLStorageMode::Private)
            .unwrap();

        buffer.mark_gpu_use().unwrap();
        buffer.clear_gpu_use().expect("Clear GPU use failed");

        assert!(!buffer.is_in_gpu_use());
        assert_eq!(buffer.state(), BUFFER_STATE_CREATED);
    }

    #[test]
    fn test_destroy() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x1234, 4096, MTLStorageMode::Shared)
            .unwrap();

        buffer.destroy().expect("Destroy failed");
        assert_eq!(buffer.state(), BUFFER_STATE_DESTROYED);
        assert!(!buffer.is_valid());
        assert!(buffer.contents_ptr().is_null());
    }

    #[test]
    fn test_double_destroy_fails() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x1234, 4096, MTLStorageMode::Shared)
            .unwrap();

        buffer.destroy().unwrap();
        let result = buffer.destroy();
        assert!(matches!(result, Err(MtlBufferError::BufferDestroyed)));
    }

    #[test]
    fn test_is_valid() {
        let buffer = MtlBufferCapsule::new();
        assert!(!buffer.is_valid()); // Uninitialized

        buffer
            .create(0x1234, 4096, MTLStorageMode::Shared)
            .unwrap();
        assert!(buffer.is_valid());

        buffer.destroy().unwrap();
        assert!(!buffer.is_valid());
    }

    #[test]
    fn test_snapshot() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x1234, 4096, MTLStorageMode::Shared)
            .unwrap();

        let snapshot = buffer.snapshot();
        assert_eq!(snapshot.handle, 0x1234);
        assert_eq!(snapshot.state, BUFFER_STATE_CREATED);
        assert_eq!(snapshot.storage_mode, MTLStorageMode::Shared);
        assert_eq!(snapshot.length, 4096);
        assert!(snapshot.has_contents);
    }

    #[test]
    fn test_label_hash() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x1234, 4096, MTLStorageMode::Shared)
            .unwrap();

        buffer.set_label_hash(0xDEAD_BEEF);
        assert_eq!(buffer.label_hash(), 0xDEAD_BEEF);
    }

    #[test]
    fn test_generation_increments() {
        let buffer = MtlBufferCapsule::new();
        let gen0 = buffer.generation();

        buffer
            .create(0x1234, 4096, MTLStorageMode::Shared)
            .unwrap();
        let gen1 = buffer.generation();
        assert!(gen1 > gen0);

        buffer.map().unwrap();
        let gen2 = buffer.generation();
        assert!(gen2 > gen1);

        buffer.unmap().unwrap();
        let gen3 = buffer.generation();
        assert!(gen3 > gen2);
    }

    #[test]
    fn test_debug_format() {
        let buffer = MtlBufferCapsule::new();
        buffer
            .create(0x1234, 4096, MTLStorageMode::Shared)
            .unwrap();

        let debug_str = format!("{:?}", buffer);
        assert!(debug_str.contains("MtlBufferCapsule"));
        assert!(debug_str.contains("Shared"));
    }

    #[test]
    fn test_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let buffer = Arc::new(MtlBufferCapsule::new());
        buffer
            .create(0x1234, 4096, MTLStorageMode::Shared)
            .unwrap();

        let mut handles = vec![];

        // Spawn readers
        for _ in 0..4 {
            let buf = Arc::clone(&buffer);
            handles.push(thread::spawn(move || {
                for _ in 0..500 {
                    let _ = buf.snapshot();
                    let _ = buf.state();
                    let _ = buf.length();
                    let _ = buf.gpu_address();
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        assert!(buffer.is_valid());
    }
}
