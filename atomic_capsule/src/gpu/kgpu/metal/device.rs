//! Metal Device Capsule - T1 Atomic, 256B cache-aligned
//!
//! Represents a Metal logical device (MTLDevice) with command queue management.
//! This is a MOCK implementation for design validation, not real Metal FFI.
//!
//! # Design
//!
//! **Tier**: T1 Atomic (<100ns operations)
//! **Size**: 256B cache-aligned (four 64-byte cache lines)
//! **Performance Targets**:
//! - State query: <10ns (single atomic load)
//! - Queue allocation: <100ns (CAS + generation bump)
//! - Resource tracking: <50ns (atomic increments)
//!
//! # Memory Layout
//!
//! ```text
//! MtlDeviceCapsule (256B, four cache lines)
//! +-- Cache Line 0 (64B): Handles + primary coordination
//! |   +-- handle: AtomicU64 (8B) - Mock MTLDevice handle
//! |   +-- primary: AtomicU64 (8B) - state(8)|queue_count(8)|generation(48)
//! |   +-- command_queue: AtomicU64 (8B) - Mock MTLCommandQueue handle
//! |   +-- _padding0: [u8; 40]
//! +-- Cache Line 1 (64B): Secondary coordination + limits
//! |   +-- secondary: AtomicU64 (8B) - max_buffer_length(32)|max_threads_per_group(32)
//! |   +-- _padding1: [u8; 56]
//! +-- Cache Line 2 (64B): Resource tracking
//! |   +-- buffer_count: AtomicU32 (4B)
//! |   +-- texture_count: AtomicU32 (4B)
//! |   +-- sampler_count: AtomicU32 (4B)
//! |   +-- pipeline_count: AtomicU32 (4B)
//! |   +-- _padding2: [u8; 48]
//! +-- Cache Line 3 (64B): Memory tracking
//!     +-- current_allocated_size: AtomicU64 (8B)
//!     +-- recommended_max_working_set_size: AtomicU64 (8B)
//!     +-- _padding3: [u8; 48]
//! ```
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_MOCK_HANDLE`: handle field is a mock value, not a real MTLDevice pointer
//! - `#ASSUME_STATE_MACHINE_VALID`: State transitions validated via CAS
//! - `#ASSUME_GENERATION_MONOTONIC`: Generation counter only increases
//! - `#ASSUME_LOCKFREE_COORDINATION`: All operations use atomic primitives
//! - `#ASSUME_RESOURCE_TRACKING_APPROXIMATE`: Resource counts may be slightly stale
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination)
//! - **Q33**: 256B alignment verified at compile time
//! - **Q34**: Generation counter enables audit trail integration

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::types::{MTLGPUFamily, MTLLanguageVersion, MTLStorageMode};

// ============================================================================
// State Constants
// ============================================================================

/// Device state: Uninitialized
pub const DEVICE_STATE_UNINITIALIZED: u8 = 0;
/// Device state: Initializing
pub const DEVICE_STATE_INITIALIZING: u8 = 1;
/// Device state: Ready for use
pub const DEVICE_STATE_READY: u8 = 2;
/// Device state: Active (has command queues/resources)
pub const DEVICE_STATE_ACTIVE: u8 = 3;
/// Device state: Lost (GPU reset)
pub const DEVICE_STATE_LOST: u8 = 4;
/// Device state: Destroyed
pub const DEVICE_STATE_DESTROYED: u8 = 5;

// ============================================================================
// Bit Field Layouts
// ============================================================================

// Primary atomic: state(8) | queue_count(8) | generation(48)
const STATE_SHIFT: u32 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;
const QUEUE_COUNT_SHIFT: u32 = 48;
const QUEUE_COUNT_MASK: u64 = 0xFF << QUEUE_COUNT_SHIFT;
const GENERATION_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// Secondary atomic: max_buffer_length(32) | max_threads_per_group(32)
const MAX_BUFFER_LENGTH_SHIFT: u32 = 32;
const MAX_BUFFER_LENGTH_MASK: u64 = 0xFFFF_FFFF << MAX_BUFFER_LENGTH_SHIFT;
const MAX_THREADS_MASK: u64 = 0x0000_0000_FFFF_FFFF;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during Metal device operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtlDeviceError {
    /// Device is in invalid state for the requested operation
    InvalidState {
        /// Current state
        current: u8,
        /// Expected state
        expected: u8,
    },
    /// State transition failed due to concurrent modification
    TransitionFailed {
        /// Expected state
        expected: u8,
        /// Observed state
        observed: u8,
    },
    /// Device has been lost (GPU reset)
    DeviceLost,
    /// Maximum resource count reached
    ResourceLimitReached {
        /// Resource type name
        resource_type: &'static str,
        /// Current count
        count: u32,
    },
    /// Requested allocation exceeds available memory
    OutOfMemory {
        /// Requested size
        requested: u64,
        /// Available size
        available: u64,
    },
}

impl core::fmt::Display for MtlDeviceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidState { current, expected } => {
                write!(f, "Invalid device state: current={}, expected={}", current, expected)
            }
            Self::TransitionFailed { expected, observed } => {
                write!(f, "Device transition failed: expected={}, observed={}", expected, observed)
            }
            Self::DeviceLost => write!(f, "Metal device has been lost"),
            Self::ResourceLimitReached { resource_type, count } => {
                write!(f, "Resource limit reached for {}: count={}", resource_type, count)
            }
            Self::OutOfMemory { requested, available } => {
                write!(f, "Out of memory: requested={}, available={}", requested, available)
            }
        }
    }
}

/// Result type for Metal device operations
pub type MtlDeviceResult<T> = Result<T, MtlDeviceError>;

// ============================================================================
// Device Snapshot
// ============================================================================

/// Atomic snapshot of device state for debugging/monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MtlDeviceSnapshot {
    /// Mock device handle
    pub handle: u64,
    /// Current state (0-5)
    pub state: u8,
    /// Number of command queues
    pub queue_count: u8,
    /// Generation counter
    pub generation: u64,
    /// Maximum buffer length
    pub max_buffer_length: u32,
    /// Maximum threads per threadgroup
    pub max_threads_per_group: u32,
    /// Buffer count
    pub buffer_count: u32,
    /// Texture count
    pub texture_count: u32,
    /// Sampler count
    pub sampler_count: u32,
    /// Pipeline count
    pub pipeline_count: u32,
    /// Current allocated memory
    pub current_allocated_size: u64,
    /// Recommended max working set size
    pub recommended_max_working_set_size: u64,
}

// ============================================================================
// Device Properties
// ============================================================================

/// Metal device properties (set at initialization)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MtlDeviceProperties {
    /// Device name (mock)
    pub name_hash: u64,
    /// GPU family
    pub gpu_family: MTLGPUFamily,
    /// Metal language version
    pub language_version: MTLLanguageVersion,
    /// Maximum buffer length
    pub max_buffer_length: u32,
    /// Maximum threads per threadgroup
    pub max_threads_per_group: u32,
    /// Recommended max working set size
    pub recommended_max_working_set_size: u64,
    /// Has unified memory (Apple Silicon)
    pub has_unified_memory: bool,
    /// Supports ray tracing
    pub supports_raytracing: bool,
}

impl Default for MtlDeviceProperties {
    fn default() -> Self {
        Self {
            name_hash: 0,
            gpu_family: MTLGPUFamily::Apple8,
            language_version: MTLLanguageVersion::Version2_4,
            max_buffer_length: 256 * 1024 * 1024, // 256 MB default
            max_threads_per_group: 1024,
            recommended_max_working_set_size: 4 * 1024 * 1024 * 1024, // 4 GB
            has_unified_memory: true,
            supports_raytracing: true,
        }
    }
}

// ============================================================================
// MtlDeviceCapsule
// ============================================================================

/// Metal Device Capsule - Represents a Metal logical device
///
/// Manages command queue creation, resource tracking, and memory management.
/// All operations are lockfree using atomic primitives.
///
/// # Tier: T1 Atomic
/// # Size: 256B (four cache lines, prevents false sharing)
///
/// # ASSUM Safety
///
/// - `#ASSUME_MOCK_HANDLE`: handle is mock, not real MTLDevice
/// - `#ASSUME_STATE_MACHINE_VALID`: State transitions validated via CAS
/// - `#ASSUME_GENERATION_MONOTONIC`: Generation counter only increases
/// - `#ASSUME_LOCKFREE_COORDINATION`: All operations use atomic primitives
#[repr(C, align(256))]
pub struct MtlDeviceCapsule {
    // ========================================================================
    // Cache Line 0: Handles + primary coordination
    // ========================================================================
    /// Mock MTLDevice handle
    ///
    /// #ASSUME_MOCK_HANDLE: This is a mock value for testing, not a real pointer.
    /// In a real implementation, this would be a raw pointer to MTLDevice.
    handle: AtomicU64,

    /// Primary coordination channel
    ///
    /// Layout: state(8) | queue_count(8) | generation(48)
    /// - Bits 56-63: State (0-5)
    /// - Bits 48-55: Command queue count (0-255)
    /// - Bits 0-47: Generation counter (TOCTOU prevention)
    primary: AtomicU64,

    /// Mock MTLCommandQueue handle
    command_queue: AtomicU64,

    /// Padding to complete first cache line
    _padding0: [u8; 40],

    // ========================================================================
    // Cache Line 1: Secondary coordination + limits
    // ========================================================================
    /// Secondary coordination channel
    ///
    /// Layout: max_buffer_length(32) | max_threads_per_group(32)
    secondary: AtomicU64,

    /// Padding to complete second cache line
    _padding1: [u8; 56],

    // ========================================================================
    // Cache Line 2: Resource tracking
    // ========================================================================
    /// Number of created buffers
    buffer_count: AtomicU32,

    /// Number of created textures
    texture_count: AtomicU32,

    /// Number of created samplers
    sampler_count: AtomicU32,

    /// Number of created pipelines
    pipeline_count: AtomicU32,

    /// Padding to complete third cache line
    _padding2: [u8; 48],

    // ========================================================================
    // Cache Line 3: Memory tracking
    // ========================================================================
    /// Current allocated GPU memory
    current_allocated_size: AtomicU64,

    /// Recommended maximum working set size
    recommended_max_working_set_size: AtomicU64,

    /// Padding to complete fourth cache line
    _padding3: [u8; 48],
}

// Compile-time size and alignment verification
const _: () = {
    assert!(core::mem::size_of::<MtlDeviceCapsule>() == 256);
    assert!(core::mem::align_of::<MtlDeviceCapsule>() == 256);
};

impl MtlDeviceCapsule {
    /// Creates a new Metal device in `Uninitialized` state.
    ///
    /// # Performance
    ///
    /// O(1), ~10ns (stack allocation + atomic init)
    #[inline]
    pub const fn new() -> Self {
        Self {
            handle: AtomicU64::new(0),
            primary: AtomicU64::new(0),
            command_queue: AtomicU64::new(0),
            _padding0: [0u8; 40],

            secondary: AtomicU64::new(0),
            _padding1: [0u8; 56],

            buffer_count: AtomicU32::new(0),
            texture_count: AtomicU32::new(0),
            sampler_count: AtomicU32::new(0),
            pipeline_count: AtomicU32::new(0),
            _padding2: [0u8; 48],

            current_allocated_size: AtomicU64::new(0),
            recommended_max_working_set_size: AtomicU64::new(0),
            _padding3: [0u8; 48],
        }
    }

    /// Returns the mock device handle.
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

    /// Returns the command queue count.
    #[inline]
    pub fn queue_count(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & QUEUE_COUNT_MASK) >> QUEUE_COUNT_SHIFT) as u8
    }

    /// Returns the generation counter.
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Returns the maximum buffer length.
    #[inline]
    pub fn max_buffer_length(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & MAX_BUFFER_LENGTH_MASK) >> MAX_BUFFER_LENGTH_SHIFT) as u32
    }

    /// Returns the maximum threads per threadgroup.
    #[inline]
    pub fn max_threads_per_group(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        (secondary & MAX_THREADS_MASK) as u32
    }

    /// Returns the buffer count.
    #[inline]
    pub fn buffer_count(&self) -> u32 {
        self.buffer_count.load(Ordering::Acquire)
    }

    /// Returns the texture count.
    #[inline]
    pub fn texture_count(&self) -> u32 {
        self.texture_count.load(Ordering::Acquire)
    }

    /// Returns the sampler count.
    #[inline]
    pub fn sampler_count(&self) -> u32 {
        self.sampler_count.load(Ordering::Acquire)
    }

    /// Returns the pipeline count.
    #[inline]
    pub fn pipeline_count(&self) -> u32 {
        self.pipeline_count.load(Ordering::Acquire)
    }

    /// Returns the current allocated memory size.
    #[inline]
    pub fn current_allocated_size(&self) -> u64 {
        self.current_allocated_size.load(Ordering::Acquire)
    }

    /// Returns the recommended max working set size.
    #[inline]
    pub fn recommended_max_working_set_size(&self) -> u64 {
        self.recommended_max_working_set_size.load(Ordering::Acquire)
    }

    /// Takes an atomic snapshot of the device state.
    ///
    /// # Performance
    ///
    /// ~60ns (multiple atomic loads)
    pub fn snapshot(&self) -> MtlDeviceSnapshot {
        let primary = self.primary.load(Ordering::Acquire);
        let secondary = self.secondary.load(Ordering::Acquire);

        MtlDeviceSnapshot {
            handle: self.handle.load(Ordering::Acquire),
            state: ((primary & STATE_MASK) >> STATE_SHIFT) as u8,
            queue_count: ((primary & QUEUE_COUNT_MASK) >> QUEUE_COUNT_SHIFT) as u8,
            generation: primary & GENERATION_MASK,
            max_buffer_length: ((secondary & MAX_BUFFER_LENGTH_MASK) >> MAX_BUFFER_LENGTH_SHIFT) as u32,
            max_threads_per_group: (secondary & MAX_THREADS_MASK) as u32,
            buffer_count: self.buffer_count.load(Ordering::Acquire),
            texture_count: self.texture_count.load(Ordering::Acquire),
            sampler_count: self.sampler_count.load(Ordering::Acquire),
            pipeline_count: self.pipeline_count.load(Ordering::Acquire),
            current_allocated_size: self.current_allocated_size.load(Ordering::Acquire),
            recommended_max_working_set_size: self.recommended_max_working_set_size.load(Ordering::Acquire),
        }
    }

    /// Initializes the device with the specified properties.
    ///
    /// # Performance
    ///
    /// <100ns (CAS + atomic stores)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_STATE_MACHINE_VALID`: Validates transition is legal
    /// - `#ASSUME_GENERATION_MONOTONIC`: Bumps generation on each transition
    pub fn initialize(&self, mock_handle: u64, props: MtlDeviceProperties) -> MtlDeviceResult<()> {
        // #ASSUME_STATE_MACHINE_VALID: Transition from Uninitialized to Initializing
        let current = self.primary.load(Ordering::Acquire);
        let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

        if current_state != DEVICE_STATE_UNINITIALIZED {
            return Err(MtlDeviceError::InvalidState {
                current: current_state,
                expected: DEVICE_STATE_UNINITIALIZED,
            });
        }

        // Transition to Initializing
        let current_gen = current & GENERATION_MASK;
        let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
        let new_primary = ((DEVICE_STATE_INITIALIZING as u64) << STATE_SHIFT) | new_gen;

        match self.primary.compare_exchange(
            current,
            new_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {}
            Err(observed) => {
                let observed_state = ((observed & STATE_MASK) >> STATE_SHIFT) as u8;
                return Err(MtlDeviceError::TransitionFailed {
                    expected: DEVICE_STATE_UNINITIALIZED,
                    observed: observed_state,
                });
            }
        }

        // Set handle and properties
        self.handle.store(mock_handle, Ordering::Release);

        // Set secondary (limits)
        let secondary_value = ((props.max_buffer_length as u64) << MAX_BUFFER_LENGTH_SHIFT)
            | (props.max_threads_per_group as u64);
        self.secondary.store(secondary_value, Ordering::Release);

        // Set memory limits
        self.recommended_max_working_set_size
            .store(props.recommended_max_working_set_size, Ordering::Release);

        // Transition to Ready
        let ready_gen = new_gen.wrapping_add(1) & GENERATION_MASK;
        let ready_primary = ((DEVICE_STATE_READY as u64) << STATE_SHIFT) | ready_gen;

        match self.primary.compare_exchange(
            new_primary,
            ready_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(observed) => {
                let observed_state = ((observed & STATE_MASK) >> STATE_SHIFT) as u8;
                Err(MtlDeviceError::TransitionFailed {
                    expected: DEVICE_STATE_INITIALIZING,
                    observed: observed_state,
                })
            }
        }
    }

    /// Creates a new command queue.
    ///
    /// # Returns
    ///
    /// Mock queue handle on success.
    ///
    /// # Performance
    ///
    /// <100ns (CAS + generation bump)
    pub fn new_command_queue(&self) -> MtlDeviceResult<u64> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;
            let queue_count = ((current & QUEUE_COUNT_MASK) >> QUEUE_COUNT_SHIFT) as u8;
            let current_gen = current & GENERATION_MASK;

            if current_state == DEVICE_STATE_LOST {
                return Err(MtlDeviceError::DeviceLost);
            }

            if current_state != DEVICE_STATE_READY && current_state != DEVICE_STATE_ACTIVE {
                return Err(MtlDeviceError::InvalidState {
                    current: current_state,
                    expected: DEVICE_STATE_READY,
                });
            }

            // Create mock queue handle ("CMDQ" in hex)
            let mock_queue_handle = 0x434D4451_0000_0000u64 | (queue_count as u64);

            // Increment queue count and transition to Active
            let new_queue_count = queue_count.saturating_add(1);
            let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
            let new_primary = ((DEVICE_STATE_ACTIVE as u64) << STATE_SHIFT)
                | ((new_queue_count as u64) << QUEUE_COUNT_SHIFT)
                | new_gen;

            if self
                .primary
                .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // Store queue handle on first queue
                if queue_count == 0 {
                    self.command_queue.store(mock_queue_handle, Ordering::Release);
                }
                return Ok(mock_queue_handle);
            }
            // Retry on CAS failure
        }
    }

    /// Tracks buffer creation.
    ///
    /// # Performance
    ///
    /// <20ns (atomic increment)
    #[inline]
    pub fn track_buffer_created(&self, size: u64) {
        self.buffer_count.fetch_add(1, Ordering::AcqRel);
        self.current_allocated_size.fetch_add(size, Ordering::AcqRel);
    }

    /// Tracks buffer destruction.
    ///
    /// # Performance
    ///
    /// <20ns (atomic decrement)
    #[inline]
    pub fn track_buffer_destroyed(&self, size: u64) {
        self.buffer_count.fetch_sub(1, Ordering::AcqRel);
        self.current_allocated_size.fetch_sub(size, Ordering::AcqRel);
    }

    /// Tracks texture creation.
    #[inline]
    pub fn track_texture_created(&self, size: u64) {
        self.texture_count.fetch_add(1, Ordering::AcqRel);
        self.current_allocated_size.fetch_add(size, Ordering::AcqRel);
    }

    /// Tracks texture destruction.
    #[inline]
    pub fn track_texture_destroyed(&self, size: u64) {
        self.texture_count.fetch_sub(1, Ordering::AcqRel);
        self.current_allocated_size.fetch_sub(size, Ordering::AcqRel);
    }

    /// Tracks sampler creation.
    #[inline]
    pub fn track_sampler_created(&self) {
        self.sampler_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Tracks sampler destruction.
    #[inline]
    pub fn track_sampler_destroyed(&self) {
        self.sampler_count.fetch_sub(1, Ordering::AcqRel);
    }

    /// Tracks pipeline creation.
    #[inline]
    pub fn track_pipeline_created(&self) {
        self.pipeline_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Tracks pipeline destruction.
    #[inline]
    pub fn track_pipeline_destroyed(&self) {
        self.pipeline_count.fetch_sub(1, Ordering::AcqRel);
    }

    /// Checks if the device is valid (Ready or Active state only).
    ///
    /// Returns `false` for Uninitialized, Initializing, Lost, or Destroyed states.
    #[inline]
    pub fn is_valid(&self) -> bool {
        let state = self.state();
        state == DEVICE_STATE_READY || state == DEVICE_STATE_ACTIVE
    }

    /// Marks the device as lost.
    pub fn mark_lost(&self) {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_gen = current & GENERATION_MASK;
            let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
            let lost = ((DEVICE_STATE_LOST as u64) << STATE_SHIFT) | new_gen;

            if self
                .primary
                .compare_exchange(current, lost, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Validates that a buffer allocation is within limits.
    pub fn validate_buffer_allocation(&self, size: u64) -> MtlDeviceResult<()> {
        let max = self.max_buffer_length() as u64;
        if size > max {
            return Err(MtlDeviceError::OutOfMemory {
                requested: size,
                available: max,
            });
        }

        let current = self.current_allocated_size();
        let max_working = self.recommended_max_working_set_size();
        if current + size > max_working {
            return Err(MtlDeviceError::OutOfMemory {
                requested: size,
                available: max_working.saturating_sub(current),
            });
        }

        Ok(())
    }

    /// Returns the preferred storage mode for this device.
    pub fn preferred_storage_mode(&self) -> MTLStorageMode {
        // Apple Silicon prefers Shared (unified memory)
        // Intel Macs prefer Managed
        // For mock, assume Apple Silicon
        MTLStorageMode::Shared
    }
}

impl Default for MtlDeviceCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for MtlDeviceCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let snapshot = self.snapshot();
        f.debug_struct("MtlDeviceCapsule")
            .field("handle", &format_args!("{:#018x}", snapshot.handle))
            .field("state", &snapshot.state)
            .field("queue_count", &snapshot.queue_count)
            .field("generation", &snapshot.generation)
            .field("buffers", &snapshot.buffer_count)
            .field("textures", &snapshot.texture_count)
            .field("allocated_mb", &(snapshot.current_allocated_size / (1024 * 1024)))
            .finish()
    }
}

// SAFETY: All operations are atomic; no mutable aliasing possible
unsafe impl Send for MtlDeviceCapsule {}
unsafe impl Sync for MtlDeviceCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<MtlDeviceCapsule>(), 256);
        assert_eq!(core::mem::align_of::<MtlDeviceCapsule>(), 256);
    }

    #[test]
    fn test_initial_state() {
        let device = MtlDeviceCapsule::new();
        assert_eq!(device.state(), DEVICE_STATE_UNINITIALIZED);
        assert_eq!(device.handle(), 0);
        assert_eq!(device.queue_count(), 0);
        assert_eq!(device.generation(), 0);
        assert_eq!(device.buffer_count(), 0);
        assert_eq!(device.texture_count(), 0);
    }

    #[test]
    fn test_initialize() {
        let device = MtlDeviceCapsule::new();
        let props = MtlDeviceProperties::default();

        device
            .initialize(0xDEAD_BEEF_CAFE_BABEu64, props)
            .expect("Init failed");

        assert_eq!(device.state(), DEVICE_STATE_READY);
        assert_eq!(device.handle(), 0xDEAD_BEEF_CAFE_BABEu64);
        assert_eq!(device.max_buffer_length(), props.max_buffer_length);
        assert_eq!(device.max_threads_per_group(), props.max_threads_per_group);
    }

    #[test]
    fn test_double_initialize_fails() {
        let device = MtlDeviceCapsule::new();
        device
            .initialize(0x1234, MtlDeviceProperties::default())
            .unwrap();

        let result = device.initialize(0x5678, MtlDeviceProperties::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_new_command_queue() {
        let device = MtlDeviceCapsule::new();
        device
            .initialize(0x1234, MtlDeviceProperties::default())
            .unwrap();

        let queue1 = device.new_command_queue().expect("Queue 1 failed");
        assert_eq!(device.state(), DEVICE_STATE_ACTIVE);
        assert_eq!(device.queue_count(), 1);
        assert_ne!(queue1, 0);

        let queue2 = device.new_command_queue().expect("Queue 2 failed");
        assert_eq!(device.queue_count(), 2);
        assert_ne!(queue2, queue1);
    }

    #[test]
    fn test_resource_tracking() {
        let device = MtlDeviceCapsule::new();
        device
            .initialize(0x1234, MtlDeviceProperties::default())
            .unwrap();

        // Track buffer creation
        device.track_buffer_created(1024);
        assert_eq!(device.buffer_count(), 1);
        assert_eq!(device.current_allocated_size(), 1024);

        device.track_buffer_created(2048);
        assert_eq!(device.buffer_count(), 2);
        assert_eq!(device.current_allocated_size(), 3072);

        // Track texture creation
        device.track_texture_created(4096);
        assert_eq!(device.texture_count(), 1);
        assert_eq!(device.current_allocated_size(), 7168);

        // Track destruction
        device.track_buffer_destroyed(1024);
        assert_eq!(device.buffer_count(), 1);
        assert_eq!(device.current_allocated_size(), 6144);
    }

    #[test]
    fn test_sampler_and_pipeline_tracking() {
        let device = MtlDeviceCapsule::new();
        device
            .initialize(0x1234, MtlDeviceProperties::default())
            .unwrap();

        device.track_sampler_created();
        device.track_sampler_created();
        assert_eq!(device.sampler_count(), 2);

        device.track_pipeline_created();
        assert_eq!(device.pipeline_count(), 1);

        device.track_sampler_destroyed();
        assert_eq!(device.sampler_count(), 1);
    }

    #[test]
    fn test_is_valid() {
        let device = MtlDeviceCapsule::new();
        assert!(!device.is_valid()); // Uninitialized is not valid for operations

        device
            .initialize(0x1234, MtlDeviceProperties::default())
            .unwrap();
        assert!(device.is_valid());

        device.mark_lost();
        assert!(!device.is_valid());
    }

    #[test]
    fn test_mark_lost() {
        let device = MtlDeviceCapsule::new();
        device
            .initialize(0x1234, MtlDeviceProperties::default())
            .unwrap();

        device.mark_lost();
        assert_eq!(device.state(), DEVICE_STATE_LOST);

        // Operations should fail on lost device
        let result = device.new_command_queue();
        assert!(matches!(result, Err(MtlDeviceError::DeviceLost)));
    }

    #[test]
    fn test_validate_buffer_allocation() {
        let device = MtlDeviceCapsule::new();
        let props = MtlDeviceProperties {
            max_buffer_length: 1024 * 1024, // 1 MB
            recommended_max_working_set_size: 10 * 1024 * 1024, // 10 MB
            ..Default::default()
        };
        device.initialize(0x1234, props).unwrap();

        // Valid allocation
        assert!(device.validate_buffer_allocation(512 * 1024).is_ok());

        // Too large for single buffer
        let result = device.validate_buffer_allocation(2 * 1024 * 1024);
        assert!(matches!(result, Err(MtlDeviceError::OutOfMemory { .. })));
    }

    #[test]
    fn test_snapshot() {
        let device = MtlDeviceCapsule::new();
        device
            .initialize(0x1234, MtlDeviceProperties::default())
            .unwrap();
        device.track_buffer_created(4096);
        device.track_texture_created(8192);

        let snapshot = device.snapshot();
        assert_eq!(snapshot.handle, 0x1234);
        assert_eq!(snapshot.state, DEVICE_STATE_READY);
        assert_eq!(snapshot.buffer_count, 1);
        assert_eq!(snapshot.texture_count, 1);
        assert_eq!(snapshot.current_allocated_size, 4096 + 8192);
    }

    #[test]
    fn test_generation_increments() {
        let device = MtlDeviceCapsule::new();
        let gen0 = device.generation();

        device
            .initialize(0x1234, MtlDeviceProperties::default())
            .unwrap();
        let gen1 = device.generation();
        assert!(gen1 > gen0);

        device.new_command_queue().unwrap();
        let gen2 = device.generation();
        assert!(gen2 > gen1);
    }

    #[test]
    fn test_preferred_storage_mode() {
        let device = MtlDeviceCapsule::new();
        device
            .initialize(0x1234, MtlDeviceProperties::default())
            .unwrap();

        // Mock assumes Apple Silicon
        assert_eq!(device.preferred_storage_mode(), MTLStorageMode::Shared);
    }

    #[test]
    fn test_debug_format() {
        let device = MtlDeviceCapsule::new();
        device
            .initialize(0x1234, MtlDeviceProperties::default())
            .unwrap();

        let debug_str = format!("{:?}", device);
        assert!(debug_str.contains("MtlDeviceCapsule"));
        assert!(debug_str.contains("state"));
    }

    #[test]
    fn test_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let device = Arc::new(MtlDeviceCapsule::new());
        device
            .initialize(0x1234, MtlDeviceProperties::default())
            .unwrap();

        let mut handles = vec![];

        // Spawn resource trackers
        for _ in 0..4 {
            let dev = Arc::clone(&device);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    dev.track_buffer_created(1024);
                    dev.track_buffer_destroyed(1024);
                }
            }));
        }

        // Spawn readers
        for _ in 0..2 {
            let dev = Arc::clone(&device);
            handles.push(thread::spawn(move || {
                for _ in 0..500 {
                    let _ = dev.snapshot();
                    let _ = dev.buffer_count();
                    let _ = dev.current_allocated_size();
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Device should still be valid
        assert!(device.is_valid());
    }
}
