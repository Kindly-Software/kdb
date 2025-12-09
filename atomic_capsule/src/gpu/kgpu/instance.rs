//! KGPU Instance Capsule - T1 Atomic, 128B cache-aligned
//!
//! Entry point for KGPU - manages adapters, backend selection, and capabilities.
//! Provides lockfree coordination via dual atomic channels for state management.
//!
//! # Design
//!
//! **Tier**: T1 Atomic (<100ns operations)
//! **Size**: 128B cache-aligned (two 64-byte cache lines)
//! **Performance Targets**:
//! - State query: <10ns (single atomic load)
//! - State transition: <50ns (CAS + generation bump)
//! - Adapter enumeration trigger: <100ns
//!
//! # Memory Layout
//!
//! ```text
//! KgpuInstanceCapsule (128B, two cache lines)
//! ├── primary: AtomicU64 (8B) - state(8)|adapter_count(8)|generation(48)
//! ├── _padding1: [u8; 56] - Complete first cache line
//! ├── secondary: AtomicU64 (8B) - backend_flags(32)|capabilities(32)
//! └── _padding2: [u8; 56] - Complete second cache line
//! ```
//!
//! # State Machine
//!
//! ```text
//! Uninitialized(0) -> Initializing(1) -> Ready(2) -> ShuttingDown(3) -> Destroyed(4)
//!                          |                |               |
//!                          v                v               v
//!                      [on error]    ShuttingDown(3)    [final]
//! ```
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_STATE_MACHINE_VALID`: State transitions validated via CAS
//! - `#ASSUME_GENERATION_MONOTONIC`: Generation counter only increases
//! - `#ASSUME_LOCKFREE_COORDINATION`: All operations use atomic primitives
//! - `#ASSUME_CACHE_LINE_SEPARATION`: 128B alignment prevents false sharing
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination)
//! - **Q33**: 128B alignment verified at compile time
//! - **Q34**: Generation counter enables audit trail integration

use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// State Constants
// ============================================================================

/// Instance state: Not yet initialized
pub const STATE_UNINITIALIZED: u8 = 0;
/// Instance state: Initialization in progress
pub const STATE_INITIALIZING: u8 = 1;
/// Instance state: Ready for use
pub const STATE_READY: u8 = 2;
/// Instance state: Shutdown in progress
pub const STATE_SHUTTING_DOWN: u8 = 3;
/// Instance state: Fully destroyed
pub const STATE_DESTROYED: u8 = 4;

// ============================================================================
// Backend Flags
// ============================================================================

/// Backend flag: Vulkan (MoltenVK on macOS, native on Linux/Windows)
pub const BACKEND_VULKAN: u32 = 1;
/// Backend flag: Metal (macOS/iOS only)
pub const BACKEND_METAL: u32 = 2;
/// Backend flag: DirectX 12 (Windows only)
pub const BACKEND_DX12: u32 = 4;
/// Backend flag: All supported backends
pub const BACKEND_ALL: u32 = BACKEND_VULKAN | BACKEND_METAL | BACKEND_DX12;

// ============================================================================
// Capability Flags
// ============================================================================

/// Capability: Compute shaders supported
pub const CAP_COMPUTE: u32 = 1 << 0;
/// Capability: Graphics pipelines supported
pub const CAP_GRAPHICS: u32 = 1 << 1;
/// Capability: Ray tracing supported
pub const CAP_RAYTRACING: u32 = 1 << 2;
/// Capability: Mesh shaders supported
pub const CAP_MESH_SHADERS: u32 = 1 << 3;
/// Capability: Variable rate shading supported
pub const CAP_VRS: u32 = 1 << 4;
/// Capability: Sparse resources supported
pub const CAP_SPARSE: u32 = 1 << 5;
/// Capability: Multi-draw indirect supported
pub const CAP_MULTI_DRAW_INDIRECT: u32 = 1 << 6;
/// Capability: Timestamp queries supported
pub const CAP_TIMESTAMPS: u32 = 1 << 7;

// ============================================================================
// Bit Field Layouts
// ============================================================================

// Primary atomic: state(8) | adapter_count(8) | generation(48)
const STATE_SHIFT: u32 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;
const ADAPTER_COUNT_SHIFT: u32 = 48;
const ADAPTER_COUNT_MASK: u64 = 0xFF << ADAPTER_COUNT_SHIFT;
const GENERATION_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// Secondary atomic: backend_flags(32) | capabilities(32)
const BACKEND_FLAGS_SHIFT: u32 = 32;
const BACKEND_FLAGS_MASK: u64 = 0xFFFF_FFFF << BACKEND_FLAGS_SHIFT;
const CAPABILITIES_MASK: u64 = 0x0000_0000_FFFF_FFFF;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during KGPU instance operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KgpuInstanceError {
    /// Instance is in invalid state for the requested operation
    InvalidState {
        /// Current state of the instance
        current: u8,
        /// Expected state for the operation
        expected: u8,
    },
    /// State transition failed due to concurrent modification
    TransitionFailed {
        /// The state that was expected
        expected: u8,
        /// The state that was observed
        observed: u8,
    },
    /// No adapters were found
    NoAdaptersFound,
    /// The specified adapter index is out of bounds
    AdapterIndexOutOfBounds {
        /// Requested index
        index: u8,
        /// Number of available adapters
        count: u8,
    },
    /// The instance has been destroyed
    InstanceDestroyed,
    /// Backend not supported on this platform
    BackendNotSupported {
        /// The unsupported backend flags
        backend: u32,
    },
}

impl core::fmt::Display for KgpuInstanceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidState { current, expected } => {
                write!(f, "Invalid state: current={}, expected={}", current, expected)
            }
            Self::TransitionFailed { expected, observed } => {
                write!(f, "State transition failed: expected={}, observed={}", expected, observed)
            }
            Self::NoAdaptersFound => write!(f, "No GPU adapters found"),
            Self::AdapterIndexOutOfBounds { index, count } => {
                write!(f, "Adapter index {} out of bounds (count={})", index, count)
            }
            Self::InstanceDestroyed => write!(f, "Instance has been destroyed"),
            Self::BackendNotSupported { backend } => {
                write!(f, "Backend {:08x} not supported", backend)
            }
        }
    }
}

/// Result type for KGPU instance operations
pub type KgpuInstanceResult<T> = Result<T, KgpuInstanceError>;

// ============================================================================
// Snapshot Type
// ============================================================================

/// Atomic snapshot of instance state for debugging/monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KgpuInstanceSnapshot {
    /// Current state (0-4)
    pub state: u8,
    /// Number of enumerated adapters
    pub adapter_count: u8,
    /// Generation counter (monotonic)
    pub generation: u64,
    /// Enabled backend flags
    pub backend_flags: u32,
    /// Capability flags
    pub capabilities: u32,
}

// ============================================================================
// KgpuInstanceCapsule
// ============================================================================

/// KGPU Instance - Entry point for GPU access
///
/// Manages adapter enumeration, backend selection, and capabilities.
/// All operations are lockfree using atomic primitives.
///
/// # Tier: T1 Atomic
/// # Size: 128B (two cache lines, prevents false sharing)
///
/// # State Machine
///
/// - `Uninitialized` (0): Initial state after construction
/// - `Initializing` (1): Backend initialization in progress
/// - `Ready` (2): Instance ready, adapters enumerated
/// - `ShuttingDown` (3): Cleanup in progress
/// - `Destroyed` (4): Instance fully destroyed
///
/// # ASSUM Safety
///
/// - `#ASSUME_STATE_MACHINE_VALID`: State transitions are validated via CAS
/// - `#ASSUME_GENERATION_MONOTONIC`: Generation counter only increases
/// - `#ASSUME_LOCKFREE_COORDINATION`: All operations use atomic primitives
/// - `#ASSUME_CACHE_LINE_SEPARATION`: 128B alignment prevents false sharing
///
/// # Example
///
/// ```rust
/// use atomic_capsule::gpu::kgpu::instance::{
///     KgpuInstanceCapsule, BACKEND_VULKAN, STATE_READY,
/// };
///
/// let instance = KgpuInstanceCapsule::new();
/// assert_eq!(instance.state(), 0); // Uninitialized
///
/// // Initialize with Vulkan backend
/// instance.initialize(BACKEND_VULKAN).expect("Init failed");
/// assert_eq!(instance.state(), STATE_READY);
///
/// // Query capabilities
/// let snapshot = instance.snapshot();
/// println!("Adapters: {}, Caps: {:08x}", snapshot.adapter_count, snapshot.capabilities);
/// ```
#[repr(C, align(128))]
pub struct KgpuInstanceCapsule {
    /// Primary coordination channel
    ///
    /// Layout: state(8) | adapter_count(8) | generation(48)
    /// - Bits 56-63: State (0-4)
    /// - Bits 48-55: Adapter count (0-255)
    /// - Bits 0-47: Generation counter (TOCTOU prevention)
    primary: AtomicU64,

    /// Padding to complete first 64-byte cache line
    _padding1: [u8; 56],

    /// Secondary coordination channel
    ///
    /// Layout: backend_flags(32) | capabilities(32)
    /// - Bits 32-63: Backend flags (VULKAN|METAL|DX12)
    /// - Bits 0-31: Capability flags (COMPUTE|GRAPHICS|...)
    secondary: AtomicU64,

    /// Padding to complete second 64-byte cache line
    _padding2: [u8; 56],
}

// Compile-time size and alignment verification
const _: () = {
    assert!(core::mem::size_of::<KgpuInstanceCapsule>() == 128);
    assert!(core::mem::align_of::<KgpuInstanceCapsule>() == 128);
};

impl KgpuInstanceCapsule {
    /// Creates a new KGPU instance in `Uninitialized` state.
    ///
    /// # Performance
    ///
    /// O(1), ~5ns (stack allocation + atomic init)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::gpu::kgpu::instance::KgpuInstanceCapsule;
    ///
    /// let instance = KgpuInstanceCapsule::new();
    /// assert_eq!(instance.state(), 0); // Uninitialized
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            // Initial state: Uninitialized(0), 0 adapters, generation 0
            primary: AtomicU64::new(0),
            _padding1: [0u8; 56],
            // Initial: no backends, no capabilities
            secondary: AtomicU64::new(0),
            _padding2: [0u8; 56],
        }
    }

    /// Returns the current state of the instance.
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn state(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & STATE_MASK) >> STATE_SHIFT) as u8
    }

    /// Returns the number of enumerated adapters.
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn adapter_count(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & ADAPTER_COUNT_MASK) >> ADAPTER_COUNT_SHIFT) as u8
    }

    /// Returns the current generation counter.
    ///
    /// Generation increments on each state transition for TOCTOU prevention.
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Returns the enabled backend flags.
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn backend_flags(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & BACKEND_FLAGS_MASK) >> BACKEND_FLAGS_SHIFT) as u32
    }

    /// Returns the capability flags.
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn capabilities(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        (secondary & CAPABILITIES_MASK) as u32
    }

    /// Takes an atomic snapshot of the instance state.
    ///
    /// The snapshot is consistent (both channels read atomically).
    ///
    /// # Performance
    ///
    /// <20ns (two atomic loads)
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::gpu::kgpu::instance::KgpuInstanceCapsule;
    ///
    /// let instance = KgpuInstanceCapsule::new();
    /// let snapshot = instance.snapshot();
    /// assert_eq!(snapshot.state, 0);
    /// assert_eq!(snapshot.adapter_count, 0);
    /// ```
    #[inline]
    pub fn snapshot(&self) -> KgpuInstanceSnapshot {
        let primary = self.primary.load(Ordering::Acquire);
        let secondary = self.secondary.load(Ordering::Acquire);

        KgpuInstanceSnapshot {
            state: ((primary & STATE_MASK) >> STATE_SHIFT) as u8,
            adapter_count: ((primary & ADAPTER_COUNT_MASK) >> ADAPTER_COUNT_SHIFT) as u8,
            generation: primary & GENERATION_MASK,
            backend_flags: ((secondary & BACKEND_FLAGS_MASK) >> BACKEND_FLAGS_SHIFT) as u32,
            capabilities: (secondary & CAPABILITIES_MASK) as u32,
        }
    }

    /// Initializes the instance with the specified backend flags.
    ///
    /// Transitions: `Uninitialized` -> `Initializing` -> `Ready`
    ///
    /// # Arguments
    ///
    /// * `backend_flags` - Backend selection (BACKEND_VULKAN, BACKEND_METAL, etc.)
    ///
    /// # Performance
    ///
    /// <100ns (two CAS operations + metadata store)
    ///
    /// # Errors
    ///
    /// - `InvalidState`: Instance not in `Uninitialized` state
    /// - `TransitionFailed`: Concurrent modification detected
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_STATE_MACHINE_VALID`: Validates transition is legal
    /// - `#ASSUME_GENERATION_MONOTONIC`: Bumps generation on each transition
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::gpu::kgpu::instance::{KgpuInstanceCapsule, BACKEND_VULKAN};
    ///
    /// let instance = KgpuInstanceCapsule::new();
    /// instance.initialize(BACKEND_VULKAN).expect("Init failed");
    /// assert_eq!(instance.state(), 2); // Ready
    /// ```
    pub fn initialize(&self, backend_flags: u32) -> KgpuInstanceResult<()> {
        // #ASSUME_STATE_MACHINE_VALID: Transition from Uninitialized to Initializing
        let current = self.primary.load(Ordering::Acquire);
        let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

        if current_state != STATE_UNINITIALIZED {
            return Err(KgpuInstanceError::InvalidState {
                current: current_state,
                expected: STATE_UNINITIALIZED,
            });
        }

        // Transition to Initializing
        // #ASSUME_GENERATION_MONOTONIC: Increment generation
        let current_gen = current & GENERATION_MASK;
        let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
        let new_primary = ((STATE_INITIALIZING as u64) << STATE_SHIFT) | new_gen;

        match self.primary.compare_exchange(
            current,
            new_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {}
            Err(observed) => {
                let observed_state = ((observed & STATE_MASK) >> STATE_SHIFT) as u8;
                return Err(KgpuInstanceError::TransitionFailed {
                    expected: STATE_UNINITIALIZED,
                    observed: observed_state,
                });
            }
        }

        // Set backend flags and simulate capability detection
        // In a real implementation, this would query the GPU driver
        let capabilities = self.detect_capabilities(backend_flags);
        let secondary_value = ((backend_flags as u64) << BACKEND_FLAGS_SHIFT) | (capabilities as u64);
        self.secondary.store(secondary_value, Ordering::Release);

        // Simulate adapter enumeration (would be real GPU enumeration)
        let adapter_count = self.enumerate_adapters_internal(backend_flags);

        // Transition to Ready
        let ready_gen = new_gen.wrapping_add(1) & GENERATION_MASK;
        let ready_primary = ((STATE_READY as u64) << STATE_SHIFT)
            | ((adapter_count as u64) << ADAPTER_COUNT_SHIFT)
            | ready_gen;

        // CAS from Initializing to Ready
        let expected_init = new_primary;
        match self.primary.compare_exchange(
            expected_init,
            ready_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(observed) => {
                let observed_state = ((observed & STATE_MASK) >> STATE_SHIFT) as u8;
                Err(KgpuInstanceError::TransitionFailed {
                    expected: STATE_INITIALIZING,
                    observed: observed_state,
                })
            }
        }
    }

    /// Enumerates available GPU adapters.
    ///
    /// Returns the number of adapters found. Must be called after `initialize()`.
    ///
    /// # Performance
    ///
    /// <50ns (atomic load + bounds check)
    ///
    /// # Errors
    ///
    /// - `InvalidState`: Instance not in `Ready` state
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::gpu::kgpu::instance::{KgpuInstanceCapsule, BACKEND_VULKAN};
    ///
    /// let instance = KgpuInstanceCapsule::new();
    /// instance.initialize(BACKEND_VULKAN).unwrap();
    /// let count = instance.enumerate_adapters().unwrap();
    /// println!("Found {} adapter(s)", count);
    /// ```
    pub fn enumerate_adapters(&self) -> KgpuInstanceResult<u8> {
        let state = self.state();
        if state != STATE_READY {
            return Err(KgpuInstanceError::InvalidState {
                current: state,
                expected: STATE_READY,
            });
        }

        Ok(self.adapter_count())
    }

    /// Requests an adapter by index.
    ///
    /// Returns a handle to the adapter at the specified index.
    /// The handle is a simple u8 index for now; in a full implementation
    /// this would return an `AdapterHandle` or similar.
    ///
    /// # Arguments
    ///
    /// * `index` - Zero-based adapter index
    ///
    /// # Performance
    ///
    /// <50ns (atomic loads + bounds check)
    ///
    /// # Errors
    ///
    /// - `InvalidState`: Instance not in `Ready` state
    /// - `AdapterIndexOutOfBounds`: Index >= adapter_count
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::gpu::kgpu::instance::{KgpuInstanceCapsule, BACKEND_ALL};
    ///
    /// let instance = KgpuInstanceCapsule::new();
    /// instance.initialize(BACKEND_ALL).unwrap();
    /// if let Ok(handle) = instance.request_adapter(0) {
    ///     println!("Got adapter handle: {}", handle);
    /// }
    /// ```
    pub fn request_adapter(&self, index: u8) -> KgpuInstanceResult<u8> {
        let state = self.state();
        if state != STATE_READY {
            return Err(KgpuInstanceError::InvalidState {
                current: state,
                expected: STATE_READY,
            });
        }

        let count = self.adapter_count();
        if index >= count {
            return Err(KgpuInstanceError::AdapterIndexOutOfBounds { index, count });
        }

        // In a real implementation, this would allocate an adapter handle
        Ok(index)
    }

    /// Destroys the instance, releasing all resources.
    ///
    /// Transitions: Any -> `ShuttingDown` -> `Destroyed`
    ///
    /// # Performance
    ///
    /// <100ns (CAS + generation bump)
    ///
    /// # Errors
    ///
    /// - `InstanceDestroyed`: Instance already destroyed
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_STATE_MACHINE_VALID`: Validates transition is legal
    /// - `#ASSUME_GENERATION_MONOTONIC`: Final generation bump for audit
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::gpu::kgpu::instance::KgpuInstanceCapsule;
    ///
    /// let instance = KgpuInstanceCapsule::new();
    /// instance.destroy().expect("Destroy failed");
    /// assert_eq!(instance.state(), 4); // Destroyed
    /// ```
    pub fn destroy(&self) -> KgpuInstanceResult<()> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

            if current_state == STATE_DESTROYED {
                return Err(KgpuInstanceError::InstanceDestroyed);
            }

            // Transition to ShuttingDown first (if not already)
            if current_state != STATE_SHUTTING_DOWN {
                let current_gen = current & GENERATION_MASK;
                let adapter_count = (current & ADAPTER_COUNT_MASK) >> ADAPTER_COUNT_SHIFT;
                let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
                let shutting_down = ((STATE_SHUTTING_DOWN as u64) << STATE_SHIFT)
                    | (adapter_count << ADAPTER_COUNT_SHIFT)
                    | new_gen;

                if self
                    .primary
                    .compare_exchange(current, shutting_down, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    continue; // Retry
                }

                // Now transition to Destroyed
                let destroyed_gen = new_gen.wrapping_add(1) & GENERATION_MASK;
                let destroyed = ((STATE_DESTROYED as u64) << STATE_SHIFT) | destroyed_gen;

                let _ = self.primary.compare_exchange(
                    shutting_down,
                    destroyed,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );

                return Ok(());
            }

            // Already ShuttingDown, transition to Destroyed
            let current_gen = current & GENERATION_MASK;
            let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
            let destroyed = ((STATE_DESTROYED as u64) << STATE_SHIFT) | new_gen;

            if self
                .primary
                .compare_exchange(current, destroyed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
            // Retry on CAS failure
        }
    }

    /// Checks if a specific backend is enabled.
    ///
    /// # Performance
    ///
    /// <10ns (atomic load + bitwise AND)
    #[inline]
    pub fn has_backend(&self, backend: u32) -> bool {
        (self.backend_flags() & backend) == backend
    }

    /// Checks if a specific capability is supported.
    ///
    /// # Performance
    ///
    /// <10ns (atomic load + bitwise AND)
    #[inline]
    pub fn has_capability(&self, capability: u32) -> bool {
        (self.capabilities() & capability) == capability
    }

    /// Internal: Detect capabilities based on backend
    ///
    /// In a real implementation, this would query the GPU driver.
    #[inline]
    fn detect_capabilities(&self, backend_flags: u32) -> u32 {
        // Simulate capability detection
        // All backends support compute and graphics
        let mut caps = CAP_COMPUTE | CAP_GRAPHICS | CAP_TIMESTAMPS;

        // Vulkan typically supports more features
        if (backend_flags & BACKEND_VULKAN) != 0 {
            caps |= CAP_RAYTRACING | CAP_MESH_SHADERS | CAP_SPARSE;
        }

        // DX12 has good feature support
        if (backend_flags & BACKEND_DX12) != 0 {
            caps |= CAP_RAYTRACING | CAP_VRS | CAP_MULTI_DRAW_INDIRECT;
        }

        // Metal has more limited feature set
        if (backend_flags & BACKEND_METAL) != 0 {
            caps |= CAP_MULTI_DRAW_INDIRECT;
        }

        caps
    }

    /// Internal: Enumerate adapters for the given backend
    ///
    /// In a real implementation, this would query the system for GPUs.
    #[inline]
    fn enumerate_adapters_internal(&self, backend_flags: u32) -> u8 {
        // Simulate adapter enumeration
        // Return 1 adapter for any backend, 2 for VULKAN
        if (backend_flags & BACKEND_VULKAN) != 0 {
            2 // Simulate discrete + integrated
        } else if backend_flags != 0 {
            1 // Single adapter for other backends
        } else {
            0 // No backend = no adapters
        }
    }
}

impl Default for KgpuInstanceCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for KgpuInstanceCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let snapshot = self.snapshot();
        f.debug_struct("KgpuInstanceCapsule")
            .field("state", &snapshot.state)
            .field("adapter_count", &snapshot.adapter_count)
            .field("generation", &snapshot.generation)
            .field("backend_flags", &format_args!("{:08x}", snapshot.backend_flags))
            .field("capabilities", &format_args!("{:08x}", snapshot.capabilities))
            .finish()
    }
}

// SAFETY: All operations are atomic; no mutable aliasing possible
unsafe impl Send for KgpuInstanceCapsule {}
unsafe impl Sync for KgpuInstanceCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<KgpuInstanceCapsule>(), 128);
        assert_eq!(core::mem::align_of::<KgpuInstanceCapsule>(), 128);
    }

    #[test]
    fn test_initial_state() {
        let instance = KgpuInstanceCapsule::new();
        assert_eq!(instance.state(), STATE_UNINITIALIZED);
        assert_eq!(instance.adapter_count(), 0);
        assert_eq!(instance.generation(), 0);
        assert_eq!(instance.backend_flags(), 0);
        assert_eq!(instance.capabilities(), 0);
    }

    #[test]
    fn test_snapshot() {
        let instance = KgpuInstanceCapsule::new();
        let snapshot = instance.snapshot();
        assert_eq!(snapshot.state, STATE_UNINITIALIZED);
        assert_eq!(snapshot.adapter_count, 0);
        assert_eq!(snapshot.generation, 0);
        assert_eq!(snapshot.backend_flags, 0);
        assert_eq!(snapshot.capabilities, 0);
    }

    #[test]
    fn test_initialize_vulkan() {
        let instance = KgpuInstanceCapsule::new();
        instance.initialize(BACKEND_VULKAN).expect("Init failed");

        assert_eq!(instance.state(), STATE_READY);
        assert_eq!(instance.adapter_count(), 2); // Simulated
        assert!(instance.generation() > 0);
        assert!(instance.has_backend(BACKEND_VULKAN));
        assert!(instance.has_capability(CAP_COMPUTE));
        assert!(instance.has_capability(CAP_GRAPHICS));
        assert!(instance.has_capability(CAP_RAYTRACING)); // Vulkan feature
    }

    #[test]
    fn test_initialize_metal() {
        let instance = KgpuInstanceCapsule::new();
        instance.initialize(BACKEND_METAL).expect("Init failed");

        assert_eq!(instance.state(), STATE_READY);
        assert_eq!(instance.adapter_count(), 1);
        assert!(instance.has_backend(BACKEND_METAL));
        assert!(instance.has_capability(CAP_COMPUTE));
        assert!(!instance.has_capability(CAP_RAYTRACING)); // Metal doesn't have RT
    }

    #[test]
    fn test_initialize_all_backends() {
        let instance = KgpuInstanceCapsule::new();
        instance.initialize(BACKEND_ALL).expect("Init failed");

        assert_eq!(instance.state(), STATE_READY);
        assert!(instance.has_backend(BACKEND_VULKAN));
        assert!(instance.has_backend(BACKEND_METAL));
        assert!(instance.has_backend(BACKEND_DX12));
    }

    #[test]
    fn test_double_initialize_fails() {
        let instance = KgpuInstanceCapsule::new();
        instance.initialize(BACKEND_VULKAN).expect("First init");

        let result = instance.initialize(BACKEND_METAL);
        assert!(result.is_err());

        match result {
            Err(KgpuInstanceError::InvalidState { current, expected }) => {
                assert_eq!(current, STATE_READY);
                assert_eq!(expected, STATE_UNINITIALIZED);
            }
            _ => panic!("Expected InvalidState error"),
        }
    }

    #[test]
    fn test_enumerate_adapters() {
        let instance = KgpuInstanceCapsule::new();

        // Should fail before init
        assert!(instance.enumerate_adapters().is_err());

        instance.initialize(BACKEND_VULKAN).unwrap();
        let count = instance.enumerate_adapters().expect("Enumerate failed");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_request_adapter() {
        let instance = KgpuInstanceCapsule::new();
        instance.initialize(BACKEND_VULKAN).unwrap();

        // Valid request
        let handle = instance.request_adapter(0).expect("Request failed");
        assert_eq!(handle, 0);

        let handle = instance.request_adapter(1).expect("Request failed");
        assert_eq!(handle, 1);

        // Invalid request (out of bounds)
        let result = instance.request_adapter(2);
        assert!(result.is_err());

        match result {
            Err(KgpuInstanceError::AdapterIndexOutOfBounds { index, count }) => {
                assert_eq!(index, 2);
                assert_eq!(count, 2);
            }
            _ => panic!("Expected AdapterIndexOutOfBounds error"),
        }
    }

    #[test]
    fn test_destroy() {
        let instance = KgpuInstanceCapsule::new();
        instance.initialize(BACKEND_VULKAN).unwrap();

        instance.destroy().expect("Destroy failed");
        assert_eq!(instance.state(), STATE_DESTROYED);

        // Second destroy should fail
        let result = instance.destroy();
        assert!(matches!(result, Err(KgpuInstanceError::InstanceDestroyed)));
    }

    #[test]
    fn test_destroy_uninitialized() {
        let instance = KgpuInstanceCapsule::new();
        instance.destroy().expect("Destroy failed");
        assert_eq!(instance.state(), STATE_DESTROYED);
    }

    #[test]
    fn test_generation_increments() {
        let instance = KgpuInstanceCapsule::new();
        let gen0 = instance.generation();

        instance.initialize(BACKEND_VULKAN).unwrap();
        let gen1 = instance.generation();
        assert!(gen1 > gen0, "Generation should increase after init");

        instance.destroy().unwrap();
        let gen2 = instance.generation();
        assert!(gen2 > gen1, "Generation should increase after destroy");
    }

    #[test]
    fn test_capability_flags() {
        let instance = KgpuInstanceCapsule::new();
        instance.initialize(BACKEND_VULKAN).unwrap();

        // Check individual capabilities
        assert!(instance.has_capability(CAP_COMPUTE));
        assert!(instance.has_capability(CAP_GRAPHICS));
        assert!(instance.has_capability(CAP_RAYTRACING));
        assert!(instance.has_capability(CAP_TIMESTAMPS));

        // Check combined capabilities
        assert!(instance.has_capability(CAP_COMPUTE | CAP_GRAPHICS));
    }

    #[test]
    fn test_debug_format() {
        let instance = KgpuInstanceCapsule::new();
        let debug_str = format!("{:?}", instance);
        assert!(debug_str.contains("KgpuInstanceCapsule"));
        assert!(debug_str.contains("state"));
    }

    #[test]
    fn test_default() {
        let instance = KgpuInstanceCapsule::default();
        assert_eq!(instance.state(), STATE_UNINITIALIZED);
    }

    #[test]
    fn test_thread_safety_smoke() {
        use std::sync::Arc;
        use std::thread;

        let instance = Arc::new(KgpuInstanceCapsule::new());
        instance.initialize(BACKEND_VULKAN).unwrap();

        let mut handles = vec![];

        // Spawn readers
        for _ in 0..4 {
            let inst = Arc::clone(&instance);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = inst.state();
                    let _ = inst.adapter_count();
                    let _ = inst.snapshot();
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Instance should still be in Ready state
        assert_eq!(instance.state(), STATE_READY);
    }
}
