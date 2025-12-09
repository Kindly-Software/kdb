//! KGPU Adapter Capsule - T1 Atomic, 256B cache-aligned
//!
//! Represents a physical GPU adapter with capabilities, limits, and device management.
//! Provides lockfree coordination via dual atomic channels for state management.
//!
//! # Design
//!
//! **Tier**: T1 Atomic (<100ns operations)
//! **Size**: 256B cache-aligned (four 64-byte cache lines)
//! **Performance Targets**:
//! - Capability query: <10ns (single atomic load)
//! - Device request: <100ns (CAS + validation)
//! - Limit queries: <10ns (atomic loads)
//!
//! # Memory Layout
//!
//! ```text
//! KgpuAdapterCapsule (256B, four cache lines)
//! +-- Cache Line 0 (64B): Primary coordination
//! |   +-- primary: AtomicU64 (8B) - state(8)|device_count(8)|generation(48)
//! |   +-- _padding0: [u8; 56]
//! +-- Cache Line 1 (64B): Secondary coordination
//! |   +-- secondary: AtomicU64 (8B) - vendor_id(16)|device_id(16)|capabilities(32)
//! |   +-- _padding1: [u8; 56]
//! +-- Cache Line 2 (64B): Device limits (low)
//! |   +-- adapter_type: AtomicU8 (1B)
//! |   +-- max_bind_groups: AtomicU8 (1B)
//! |   +-- max_samplers: AtomicU16 (2B)
//! |   +-- max_texture_size: AtomicU32 (4B)
//! |   +-- max_buffer_size: AtomicU64 (8B)
//! |   +-- max_compute_workgroup_size: AtomicU32 (4B)
//! |   +-- max_compute_invocations: AtomicU32 (4B)
//! |   +-- _padding2: [u8; 40]
//! +-- Cache Line 3 (64B): Backend handle + reserved
//!     +-- backend_handle: AtomicU64 (8B)
//!     +-- driver_version: AtomicU32 (4B)
//!     +-- api_version: AtomicU32 (4B)
//!     +-- _padding3: [u8; 48]
//! ```
//!
//! # Adapter Types
//!
//! - `DiscreteGpu` (0): Dedicated GPU with own memory
//! - `IntegratedGpu` (1): CPU-integrated GPU, shared memory
//! - `VirtualGpu` (2): Virtualized GPU (cloud/VM)
//! - `Cpu` (3): Software rasterizer fallback
//! - `Unknown` (255): Unidentified adapter type
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_STATE_MACHINE_VALID`: State transitions validated via CAS
//! - `#ASSUME_GENERATION_MONOTONIC`: Generation counter only increases
//! - `#ASSUME_LOCKFREE_COORDINATION`: All operations use atomic primitives
//! - `#ASSUME_CACHE_LINE_SEPARATION`: 256B alignment prevents false sharing
//! - `#ASSUME_LIMITS_IMMUTABLE_AFTER_INIT`: Limits set once during initialization
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination)
//! - **Q33**: 256B alignment verified at compile time
//! - **Q34**: Generation counter enables audit trail integration

use core::sync::atomic::{AtomicU8, AtomicU16, AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Adapter Type Constants
// ============================================================================

/// Adapter type: Discrete GPU with dedicated memory
pub const ADAPTER_TYPE_DISCRETE_GPU: u8 = 0;
/// Adapter type: Integrated GPU sharing system memory
pub const ADAPTER_TYPE_INTEGRATED_GPU: u8 = 1;
/// Adapter type: Virtual GPU (cloud/VM environment)
pub const ADAPTER_TYPE_VIRTUAL_GPU: u8 = 2;
/// Adapter type: CPU software rasterizer
pub const ADAPTER_TYPE_CPU: u8 = 3;
/// Adapter type: Unknown/unidentified
pub const ADAPTER_TYPE_UNKNOWN: u8 = 255;

// ============================================================================
// State Constants
// ============================================================================

/// Adapter state: Invalid/uninitialized
pub const ADAPTER_STATE_INVALID: u8 = 0;
/// Adapter state: Initializing
pub const ADAPTER_STATE_INITIALIZING: u8 = 1;
/// Adapter state: Ready for device creation
pub const ADAPTER_STATE_READY: u8 = 2;
/// Adapter state: Device created, in use
pub const ADAPTER_STATE_IN_USE: u8 = 3;
/// Adapter state: Lost (GPU reset, driver crash)
pub const ADAPTER_STATE_LOST: u8 = 4;

// ============================================================================
// Capability Flags (same as instance for consistency)
// ============================================================================

/// Capability: Compute shaders supported
pub const ADAPTER_CAP_COMPUTE: u32 = 1 << 0;
/// Capability: Graphics pipelines supported
pub const ADAPTER_CAP_GRAPHICS: u32 = 1 << 1;
/// Capability: Ray tracing supported
pub const ADAPTER_CAP_RAYTRACING: u32 = 1 << 2;
/// Capability: Mesh shaders supported
pub const ADAPTER_CAP_MESH_SHADERS: u32 = 1 << 3;
/// Capability: Variable rate shading supported
pub const ADAPTER_CAP_VRS: u32 = 1 << 4;
/// Capability: Sparse resources supported
pub const ADAPTER_CAP_SPARSE: u32 = 1 << 5;
/// Capability: 16-bit storage supported
pub const ADAPTER_CAP_16BIT_STORAGE: u32 = 1 << 6;
/// Capability: 64-bit atomics supported
pub const ADAPTER_CAP_64BIT_ATOMICS: u32 = 1 << 7;
/// Capability: Shader float16 supported
pub const ADAPTER_CAP_FLOAT16: u32 = 1 << 8;
/// Capability: Bindless resources supported
pub const ADAPTER_CAP_BINDLESS: u32 = 1 << 9;
/// Capability: Multi-view rendering supported
pub const ADAPTER_CAP_MULTI_VIEW: u32 = 1 << 10;
/// Capability: Subgroups supported
pub const ADAPTER_CAP_SUBGROUPS: u32 = 1 << 11;

// ============================================================================
// Well-Known Vendor IDs
// ============================================================================

/// Vendor ID: NVIDIA
pub const VENDOR_NVIDIA: u16 = 0x10DE;
/// Vendor ID: AMD
pub const VENDOR_AMD: u16 = 0x1002;
/// Vendor ID: Intel
pub const VENDOR_INTEL: u16 = 0x8086;
/// Vendor ID: Apple (Metal)
pub const VENDOR_APPLE: u16 = 0x106B;
/// Vendor ID: Qualcomm (Adreno)
pub const VENDOR_QUALCOMM: u16 = 0x5143;
/// Vendor ID: ARM (Mali)
pub const VENDOR_ARM: u16 = 0x13B5;
/// Vendor ID: Unknown
pub const VENDOR_UNKNOWN: u16 = 0x0000;

// ============================================================================
// Bit Field Layouts
// ============================================================================

// Primary atomic: state(8) | device_count(8) | generation(48)
const STATE_SHIFT: u32 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;
const DEVICE_COUNT_SHIFT: u32 = 48;
const DEVICE_COUNT_MASK: u64 = 0xFF << DEVICE_COUNT_SHIFT;
const GENERATION_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// Secondary atomic: vendor_id(16) | device_id(16) | capabilities(32)
const VENDOR_ID_SHIFT: u32 = 48;
const VENDOR_ID_MASK: u64 = 0xFFFF << VENDOR_ID_SHIFT;
const DEVICE_ID_SHIFT: u32 = 32;
const DEVICE_ID_MASK: u64 = 0xFFFF << DEVICE_ID_SHIFT;
const CAPABILITIES_MASK: u64 = 0x0000_0000_FFFF_FFFF;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during KGPU adapter operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KgpuAdapterError {
    /// Adapter is in invalid state for the requested operation
    InvalidState {
        /// Current state of the adapter
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
    /// Device creation failed
    DeviceCreationFailed,
    /// Maximum devices already created
    MaxDevicesReached {
        /// Current device count
        count: u8,
    },
    /// Adapter has been lost (GPU reset)
    AdapterLost,
    /// Requested feature not supported
    FeatureNotSupported {
        /// The unsupported capability flags
        capability: u32,
    },
    /// Resource limit exceeded
    LimitExceeded {
        /// Name of the limit
        limit_name: &'static str,
        /// Requested value
        requested: u64,
        /// Maximum allowed value
        maximum: u64,
    },
}

impl core::fmt::Display for KgpuAdapterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidState { current, expected } => {
                write!(f, "Invalid state: current={}, expected={}", current, expected)
            }
            Self::TransitionFailed { expected, observed } => {
                write!(f, "State transition failed: expected={}, observed={}", expected, observed)
            }
            Self::DeviceCreationFailed => write!(f, "Device creation failed"),
            Self::MaxDevicesReached { count } => {
                write!(f, "Maximum devices reached (count={})", count)
            }
            Self::AdapterLost => write!(f, "Adapter has been lost"),
            Self::FeatureNotSupported { capability } => {
                write!(f, "Feature {:08x} not supported", capability)
            }
            Self::LimitExceeded { limit_name, requested, maximum } => {
                write!(f, "Limit '{}' exceeded: {} > {}", limit_name, requested, maximum)
            }
        }
    }
}

/// Result type for KGPU adapter operations
pub type KgpuAdapterResult<T> = Result<T, KgpuAdapterError>;

// ============================================================================
// Limits Structure
// ============================================================================

/// GPU device limits for validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterLimits {
    /// Maximum 2D texture dimension (width/height)
    pub max_texture_size: u32,
    /// Maximum buffer size in bytes
    pub max_buffer_size: u64,
    /// Maximum bind groups per pipeline
    pub max_bind_groups: u8,
    /// Maximum samplers per shader stage
    pub max_samplers: u16,
    /// Maximum compute workgroup size (per dimension)
    pub max_compute_workgroup_size: u32,
    /// Maximum compute shader invocations (total)
    pub max_compute_invocations: u32,
}

impl Default for AdapterLimits {
    fn default() -> Self {
        Self {
            max_texture_size: 8192,
            max_buffer_size: 256 * 1024 * 1024, // 256 MB
            max_bind_groups: 4,
            max_samplers: 16,
            max_compute_workgroup_size: 256,
            max_compute_invocations: 256 * 256 * 64,
        }
    }
}

// ============================================================================
// Snapshot Type
// ============================================================================

/// Atomic snapshot of adapter state for debugging/monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KgpuAdapterSnapshot {
    /// Current state (0-4)
    pub state: u8,
    /// Number of created devices
    pub device_count: u8,
    /// Generation counter (monotonic)
    pub generation: u64,
    /// Vendor ID (PCI vendor)
    pub vendor_id: u16,
    /// Device ID (PCI device)
    pub device_id: u16,
    /// Capability flags
    pub capabilities: u32,
    /// Adapter type
    pub adapter_type: u8,
    /// Device limits
    pub limits: AdapterLimits,
    /// Driver version
    pub driver_version: u32,
    /// API version
    pub api_version: u32,
}

// ============================================================================
// KgpuAdapterCapsule
// ============================================================================

/// KGPU Adapter - Represents a physical GPU
///
/// Manages device creation, capability queries, and limit validation.
/// All operations are lockfree using atomic primitives.
///
/// # Tier: T1 Atomic
/// # Size: 256B (four cache lines, prevents false sharing)
///
/// # State Machine
///
/// - `Invalid` (0): Uninitialized adapter
/// - `Initializing` (1): Querying capabilities
/// - `Ready` (2): Ready for device creation
/// - `InUse` (3): Device(s) created
/// - `Lost` (4): Adapter lost (GPU reset)
///
/// # ASSUM Safety
///
/// - `#ASSUME_STATE_MACHINE_VALID`: State transitions are validated via CAS
/// - `#ASSUME_GENERATION_MONOTONIC`: Generation counter only increases
/// - `#ASSUME_LOCKFREE_COORDINATION`: All operations use atomic primitives
/// - `#ASSUME_CACHE_LINE_SEPARATION`: 256B alignment prevents false sharing
/// - `#ASSUME_LIMITS_IMMUTABLE_AFTER_INIT`: Limits set once during initialization
///
/// # Example
///
/// ```rust
/// use atomic_capsule::gpu::kgpu::adapter::{
///     KgpuAdapterCapsule, ADAPTER_TYPE_DISCRETE_GPU, VENDOR_NVIDIA,
///     ADAPTER_CAP_COMPUTE, ADAPTER_CAP_RAYTRACING,
/// };
///
/// let adapter = KgpuAdapterCapsule::new();
///
/// // Initialize as discrete NVIDIA GPU
/// adapter.initialize(
///     VENDOR_NVIDIA,
///     0x2684, // RTX 4090
///     ADAPTER_TYPE_DISCRETE_GPU,
///     ADAPTER_CAP_COMPUTE | ADAPTER_CAP_RAYTRACING,
/// ).expect("Init failed");
///
/// // Query capabilities
/// assert!(adapter.has_capability(ADAPTER_CAP_RAYTRACING));
///
/// // Request a device
/// let device_handle = adapter.request_device().expect("Device creation failed");
/// ```
#[repr(C, align(256))]
pub struct KgpuAdapterCapsule {
    // ========================================================================
    // Cache Line 0: Primary coordination
    // ========================================================================
    /// Primary coordination channel
    ///
    /// Layout: state(8) | device_count(8) | generation(48)
    /// - Bits 56-63: State (0-4)
    /// - Bits 48-55: Device count (0-255)
    /// - Bits 0-47: Generation counter (TOCTOU prevention)
    primary: AtomicU64,

    /// Padding to complete first cache line
    _padding0: [u8; 56],

    // ========================================================================
    // Cache Line 1: Secondary coordination
    // ========================================================================
    /// Secondary coordination channel
    ///
    /// Layout: vendor_id(16) | device_id(16) | capabilities(32)
    /// - Bits 48-63: Vendor ID (PCI)
    /// - Bits 32-47: Device ID (PCI)
    /// - Bits 0-31: Capability flags
    secondary: AtomicU64,

    /// Padding to complete second cache line
    _padding1: [u8; 56],

    // ========================================================================
    // Cache Line 2: Device limits
    // ========================================================================
    /// Adapter type (discrete, integrated, etc.)
    adapter_type: AtomicU8,

    /// Maximum bind groups per pipeline
    max_bind_groups: AtomicU8,

    /// Maximum samplers per shader stage
    max_samplers: AtomicU16,

    /// Maximum 2D texture dimension
    max_texture_size: AtomicU32,

    /// Maximum buffer size in bytes
    max_buffer_size: AtomicU64,

    /// Maximum compute workgroup size (per dimension)
    max_compute_workgroup_size: AtomicU32,

    /// Maximum compute shader invocations
    max_compute_invocations: AtomicU32,

    /// Padding to complete third cache line
    _padding2: [u8; 36],

    // ========================================================================
    // Cache Line 3: Backend handle + version info
    // ========================================================================
    /// Opaque backend handle (driver-specific)
    backend_handle: AtomicU64,

    /// Driver version (encoded)
    driver_version: AtomicU32,

    /// API version (encoded)
    api_version: AtomicU32,

    /// Padding to complete fourth cache line
    _padding3: [u8; 48],
}

// Compile-time size and alignment verification
const _: () = {
    assert!(core::mem::size_of::<KgpuAdapterCapsule>() == 256);
    assert!(core::mem::align_of::<KgpuAdapterCapsule>() == 256);
};

impl KgpuAdapterCapsule {
    /// Creates a new KGPU adapter in `Invalid` state.
    ///
    /// # Performance
    ///
    /// O(1), ~10ns (stack allocation + atomic init)
    #[inline]
    pub const fn new() -> Self {
        Self {
            // Primary: Invalid(0), 0 devices, generation 0
            primary: AtomicU64::new(0),
            _padding0: [0u8; 56],

            // Secondary: Unknown vendor/device, no capabilities
            secondary: AtomicU64::new(0),
            _padding1: [0u8; 56],

            // Default limits (will be set during initialization)
            adapter_type: AtomicU8::new(ADAPTER_TYPE_UNKNOWN),
            max_bind_groups: AtomicU8::new(4),
            max_samplers: AtomicU16::new(16),
            max_texture_size: AtomicU32::new(8192),
            max_buffer_size: AtomicU64::new(256 * 1024 * 1024),
            max_compute_workgroup_size: AtomicU32::new(256),
            max_compute_invocations: AtomicU32::new(256 * 256 * 64),
            _padding2: [0u8; 36],

            // Backend handle (0 = invalid)
            backend_handle: AtomicU64::new(0),
            driver_version: AtomicU32::new(0),
            api_version: AtomicU32::new(0),
            _padding3: [0u8; 48],
        }
    }

    /// Returns the current state of the adapter.
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn state(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & STATE_MASK) >> STATE_SHIFT) as u8
    }

    /// Returns the number of created devices.
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn device_count(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & DEVICE_COUNT_MASK) >> DEVICE_COUNT_SHIFT) as u8
    }

    /// Returns the current generation counter.
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Returns the vendor ID (PCI vendor).
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn vendor_id(&self) -> u16 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & VENDOR_ID_MASK) >> VENDOR_ID_SHIFT) as u16
    }

    /// Returns the device ID (PCI device).
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn device_id(&self) -> u16 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & DEVICE_ID_MASK) >> DEVICE_ID_SHIFT) as u16
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

    /// Returns the adapter type.
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn adapter_type(&self) -> u8 {
        self.adapter_type.load(Ordering::Acquire)
    }

    /// Returns the device limits.
    ///
    /// # Performance
    ///
    /// ~40ns (multiple atomic loads)
    pub fn limits(&self) -> AdapterLimits {
        AdapterLimits {
            max_texture_size: self.max_texture_size.load(Ordering::Acquire),
            max_buffer_size: self.max_buffer_size.load(Ordering::Acquire),
            max_bind_groups: self.max_bind_groups.load(Ordering::Acquire),
            max_samplers: self.max_samplers.load(Ordering::Acquire),
            max_compute_workgroup_size: self.max_compute_workgroup_size.load(Ordering::Acquire),
            max_compute_invocations: self.max_compute_invocations.load(Ordering::Acquire),
        }
    }

    /// Takes an atomic snapshot of the adapter state.
    ///
    /// # Performance
    ///
    /// ~60ns (multiple atomic loads)
    pub fn snapshot(&self) -> KgpuAdapterSnapshot {
        let primary = self.primary.load(Ordering::Acquire);
        let secondary = self.secondary.load(Ordering::Acquire);

        KgpuAdapterSnapshot {
            state: ((primary & STATE_MASK) >> STATE_SHIFT) as u8,
            device_count: ((primary & DEVICE_COUNT_MASK) >> DEVICE_COUNT_SHIFT) as u8,
            generation: primary & GENERATION_MASK,
            vendor_id: ((secondary & VENDOR_ID_MASK) >> VENDOR_ID_SHIFT) as u16,
            device_id: ((secondary & DEVICE_ID_MASK) >> DEVICE_ID_SHIFT) as u16,
            capabilities: (secondary & CAPABILITIES_MASK) as u32,
            adapter_type: self.adapter_type.load(Ordering::Acquire),
            limits: self.limits(),
            driver_version: self.driver_version.load(Ordering::Acquire),
            api_version: self.api_version.load(Ordering::Acquire),
        }
    }

    /// Initializes the adapter with the specified properties.
    ///
    /// Transitions: `Invalid` -> `Initializing` -> `Ready`
    ///
    /// # Arguments
    ///
    /// * `vendor_id` - PCI vendor ID
    /// * `device_id` - PCI device ID
    /// * `adapter_type` - Type of adapter (discrete, integrated, etc.)
    /// * `capabilities` - Supported capability flags
    ///
    /// # Performance
    ///
    /// <100ns (CAS + atomic stores)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_STATE_MACHINE_VALID`: Validates transition is legal
    /// - `#ASSUME_GENERATION_MONOTONIC`: Bumps generation on each transition
    /// - `#ASSUME_LIMITS_IMMUTABLE_AFTER_INIT`: Sets default limits
    pub fn initialize(
        &self,
        vendor_id: u16,
        device_id: u16,
        adapter_type: u8,
        capabilities: u32,
    ) -> KgpuAdapterResult<()> {
        // #ASSUME_STATE_MACHINE_VALID: Transition from Invalid to Initializing
        let current = self.primary.load(Ordering::Acquire);
        let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

        if current_state != ADAPTER_STATE_INVALID {
            return Err(KgpuAdapterError::InvalidState {
                current: current_state,
                expected: ADAPTER_STATE_INVALID,
            });
        }

        // Transition to Initializing
        let current_gen = current & GENERATION_MASK;
        let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
        let new_primary = ((ADAPTER_STATE_INITIALIZING as u64) << STATE_SHIFT) | new_gen;

        match self.primary.compare_exchange(
            current,
            new_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {}
            Err(observed) => {
                let observed_state = ((observed & STATE_MASK) >> STATE_SHIFT) as u8;
                return Err(KgpuAdapterError::TransitionFailed {
                    expected: ADAPTER_STATE_INVALID,
                    observed: observed_state,
                });
            }
        }

        // Set secondary data (vendor, device, capabilities)
        let secondary_value = ((vendor_id as u64) << VENDOR_ID_SHIFT)
            | ((device_id as u64) << DEVICE_ID_SHIFT)
            | (capabilities as u64);
        self.secondary.store(secondary_value, Ordering::Release);

        // Set adapter type
        self.adapter_type.store(adapter_type, Ordering::Release);

        // Set limits based on adapter type
        // #ASSUME_LIMITS_IMMUTABLE_AFTER_INIT: Limits set once here
        self.set_default_limits(adapter_type, vendor_id);

        // Set driver/API versions (simulated)
        self.driver_version.store(self.detect_driver_version(vendor_id), Ordering::Release);
        self.api_version.store(0x0001_0003, Ordering::Release); // 1.3

        // Transition to Ready
        let ready_gen = new_gen.wrapping_add(1) & GENERATION_MASK;
        let ready_primary = ((ADAPTER_STATE_READY as u64) << STATE_SHIFT) | ready_gen;

        match self.primary.compare_exchange(
            new_primary,
            ready_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(observed) => {
                let observed_state = ((observed & STATE_MASK) >> STATE_SHIFT) as u8;
                Err(KgpuAdapterError::TransitionFailed {
                    expected: ADAPTER_STATE_INITIALIZING,
                    observed: observed_state,
                })
            }
        }
    }

    /// Initializes the adapter with custom limits.
    ///
    /// Same as `initialize()` but allows specifying custom device limits.
    pub fn initialize_with_limits(
        &self,
        vendor_id: u16,
        device_id: u16,
        adapter_type: u8,
        capabilities: u32,
        limits: AdapterLimits,
    ) -> KgpuAdapterResult<()> {
        // Initialize normally first
        self.initialize(vendor_id, device_id, adapter_type, capabilities)?;

        // Override with custom limits
        self.max_texture_size.store(limits.max_texture_size, Ordering::Release);
        self.max_buffer_size.store(limits.max_buffer_size, Ordering::Release);
        self.max_bind_groups.store(limits.max_bind_groups, Ordering::Release);
        self.max_samplers.store(limits.max_samplers, Ordering::Release);
        self.max_compute_workgroup_size.store(limits.max_compute_workgroup_size, Ordering::Release);
        self.max_compute_invocations.store(limits.max_compute_invocations, Ordering::Release);

        Ok(())
    }

    /// Requests a new device from this adapter.
    ///
    /// Returns a device handle (index). In a full implementation,
    /// this would return a `DeviceHandle` type.
    ///
    /// # Performance
    ///
    /// <100ns (CAS + validation)
    ///
    /// # Errors
    ///
    /// - `InvalidState`: Adapter not in `Ready` or `InUse` state
    /// - `MaxDevicesReached`: Already at maximum device count
    /// - `AdapterLost`: Adapter has been lost
    pub fn request_device(&self) -> KgpuAdapterResult<u8> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;
            let device_count = ((current & DEVICE_COUNT_MASK) >> DEVICE_COUNT_SHIFT) as u8;
            let current_gen = current & GENERATION_MASK;

            // Check state
            if current_state == ADAPTER_STATE_LOST {
                return Err(KgpuAdapterError::AdapterLost);
            }

            if current_state != ADAPTER_STATE_READY && current_state != ADAPTER_STATE_IN_USE {
                return Err(KgpuAdapterError::InvalidState {
                    current: current_state,
                    expected: ADAPTER_STATE_READY,
                });
            }

            // Check device limit (max 255 devices)
            if device_count >= 255 {
                return Err(KgpuAdapterError::MaxDevicesReached { count: device_count });
            }

            // Increment device count and transition to InUse
            let new_device_count = device_count + 1;
            let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
            let new_primary = ((ADAPTER_STATE_IN_USE as u64) << STATE_SHIFT)
                | ((new_device_count as u64) << DEVICE_COUNT_SHIFT)
                | new_gen;

            if self.primary.compare_exchange(
                current,
                new_primary,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                return Ok(device_count); // Return index of new device
            }
            // CAS failed, retry
        }
    }

    /// Releases a device, decrementing the device count.
    ///
    /// # Performance
    ///
    /// <100ns (CAS)
    pub fn release_device(&self) -> KgpuAdapterResult<()> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;
            let device_count = ((current & DEVICE_COUNT_MASK) >> DEVICE_COUNT_SHIFT) as u8;
            let current_gen = current & GENERATION_MASK;

            if current_state == ADAPTER_STATE_LOST {
                return Err(KgpuAdapterError::AdapterLost);
            }

            if device_count == 0 {
                return Ok(()); // No devices to release
            }

            // Decrement device count
            let new_device_count = device_count - 1;
            let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;

            // Transition back to Ready if no more devices
            let new_state = if new_device_count == 0 {
                ADAPTER_STATE_READY
            } else {
                ADAPTER_STATE_IN_USE
            };

            let new_primary = ((new_state as u64) << STATE_SHIFT)
                | ((new_device_count as u64) << DEVICE_COUNT_SHIFT)
                | new_gen;

            if self.primary.compare_exchange(
                current,
                new_primary,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                return Ok(());
            }
            // Retry on CAS failure
        }
    }

    /// Checks if the adapter is valid (not Invalid or Lost).
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn is_valid(&self) -> bool {
        let state = self.state();
        state != ADAPTER_STATE_INVALID && state != ADAPTER_STATE_LOST
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

    /// Validates that a resource size is within limits.
    ///
    /// # Performance
    ///
    /// ~20ns (atomic loads + comparison)
    pub fn validate_buffer_size(&self, size: u64) -> KgpuAdapterResult<()> {
        let max = self.max_buffer_size.load(Ordering::Acquire);
        if size > max {
            return Err(KgpuAdapterError::LimitExceeded {
                limit_name: "max_buffer_size",
                requested: size,
                maximum: max,
            });
        }
        Ok(())
    }

    /// Validates that a texture size is within limits.
    ///
    /// # Performance
    ///
    /// ~10ns (atomic load + comparison)
    pub fn validate_texture_size(&self, size: u32) -> KgpuAdapterResult<()> {
        let max = self.max_texture_size.load(Ordering::Acquire);
        if size > max {
            return Err(KgpuAdapterError::LimitExceeded {
                limit_name: "max_texture_size",
                requested: size as u64,
                maximum: max as u64,
            });
        }
        Ok(())
    }

    /// Marks the adapter as lost (e.g., after GPU reset).
    ///
    /// # Performance
    ///
    /// <50ns (CAS loop)
    pub fn mark_lost(&self) {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_gen = current & GENERATION_MASK;
            let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
            let lost = ((ADAPTER_STATE_LOST as u64) << STATE_SHIFT) | new_gen;

            if self.primary.compare_exchange(
                current,
                lost,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                break;
            }
        }
    }

    /// Sets the backend handle (opaque driver handle).
    ///
    /// # Performance
    ///
    /// <10ns (atomic store)
    #[inline]
    pub fn set_backend_handle(&self, handle: u64) {
        self.backend_handle.store(handle, Ordering::Release);
    }

    /// Gets the backend handle.
    ///
    /// # Performance
    ///
    /// <10ns (atomic load)
    #[inline]
    pub fn backend_handle(&self) -> u64 {
        self.backend_handle.load(Ordering::Acquire)
    }

    /// Returns a human-readable vendor name.
    pub fn vendor_name(&self) -> &'static str {
        match self.vendor_id() {
            VENDOR_NVIDIA => "NVIDIA",
            VENDOR_AMD => "AMD",
            VENDOR_INTEL => "Intel",
            VENDOR_APPLE => "Apple",
            VENDOR_QUALCOMM => "Qualcomm",
            VENDOR_ARM => "ARM",
            _ => "Unknown",
        }
    }

    /// Returns a human-readable adapter type name.
    pub fn adapter_type_name(&self) -> &'static str {
        match self.adapter_type() {
            ADAPTER_TYPE_DISCRETE_GPU => "Discrete GPU",
            ADAPTER_TYPE_INTEGRATED_GPU => "Integrated GPU",
            ADAPTER_TYPE_VIRTUAL_GPU => "Virtual GPU",
            ADAPTER_TYPE_CPU => "CPU (Software)",
            _ => "Unknown",
        }
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    /// Sets default limits based on adapter type and vendor
    fn set_default_limits(&self, adapter_type: u8, vendor_id: u16) {
        match adapter_type {
            ADAPTER_TYPE_DISCRETE_GPU => {
                // High-end discrete GPU limits
                self.max_texture_size.store(16384, Ordering::Release);
                self.max_buffer_size.store(2 * 1024 * 1024 * 1024, Ordering::Release); // 2GB
                self.max_bind_groups.store(8, Ordering::Release);
                self.max_samplers.store(2048, Ordering::Release);
                self.max_compute_workgroup_size.store(1024, Ordering::Release);
                self.max_compute_invocations.store(1024 * 1024 * 1024, Ordering::Release);

                // Vendor-specific adjustments
                if vendor_id == VENDOR_NVIDIA {
                    // NVIDIA tends to have higher limits
                    self.max_samplers.store(4096, Ordering::Release);
                }
            }
            ADAPTER_TYPE_INTEGRATED_GPU => {
                // Integrated GPU (shared memory)
                self.max_texture_size.store(8192, Ordering::Release);
                self.max_buffer_size.store(512 * 1024 * 1024, Ordering::Release); // 512MB
                self.max_bind_groups.store(4, Ordering::Release);
                self.max_samplers.store(256, Ordering::Release);
                self.max_compute_workgroup_size.store(512, Ordering::Release);
                self.max_compute_invocations.store(512 * 512 * 512, Ordering::Release);
            }
            ADAPTER_TYPE_VIRTUAL_GPU => {
                // Virtual GPU (cloud)
                self.max_texture_size.store(8192, Ordering::Release);
                self.max_buffer_size.store(1024 * 1024 * 1024, Ordering::Release); // 1GB
                self.max_bind_groups.store(4, Ordering::Release);
                self.max_samplers.store(512, Ordering::Release);
                self.max_compute_workgroup_size.store(256, Ordering::Release);
                self.max_compute_invocations.store(256 * 256 * 256, Ordering::Release);
            }
            _ => {
                // CPU fallback / unknown
                self.max_texture_size.store(4096, Ordering::Release);
                self.max_buffer_size.store(256 * 1024 * 1024, Ordering::Release); // 256MB
                self.max_bind_groups.store(4, Ordering::Release);
                self.max_samplers.store(16, Ordering::Release);
                self.max_compute_workgroup_size.store(64, Ordering::Release);
                self.max_compute_invocations.store(64 * 64 * 64, Ordering::Release);
            }
        }
    }

    /// Detect driver version (simulated)
    fn detect_driver_version(&self, vendor_id: u16) -> u32 {
        // In a real implementation, this would query the driver
        match vendor_id {
            VENDOR_NVIDIA => 0x01F8_0000, // 550.0.0
            VENDOR_AMD => 0x0018_0000,    // 24.0.0
            VENDOR_INTEL => 0x001F_0000,  // 31.0.0
            _ => 0x0001_0000,             // 1.0.0
        }
    }
}

impl Default for KgpuAdapterCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for KgpuAdapterCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let snapshot = self.snapshot();
        f.debug_struct("KgpuAdapterCapsule")
            .field("state", &snapshot.state)
            .field("device_count", &snapshot.device_count)
            .field("vendor", &self.vendor_name())
            .field("vendor_id", &format_args!("{:04x}", snapshot.vendor_id))
            .field("device_id", &format_args!("{:04x}", snapshot.device_id))
            .field("adapter_type", &self.adapter_type_name())
            .field("capabilities", &format_args!("{:08x}", snapshot.capabilities))
            .field("generation", &snapshot.generation)
            .finish()
    }
}

// SAFETY: All operations are atomic; no mutable aliasing possible
unsafe impl Send for KgpuAdapterCapsule {}
unsafe impl Sync for KgpuAdapterCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<KgpuAdapterCapsule>(), 256);
        assert_eq!(core::mem::align_of::<KgpuAdapterCapsule>(), 256);
    }

    #[test]
    fn test_initial_state() {
        let adapter = KgpuAdapterCapsule::new();
        assert_eq!(adapter.state(), ADAPTER_STATE_INVALID);
        assert_eq!(adapter.device_count(), 0);
        assert_eq!(adapter.generation(), 0);
        assert_eq!(adapter.vendor_id(), 0);
        assert_eq!(adapter.device_id(), 0);
        assert_eq!(adapter.capabilities(), 0);
        assert_eq!(adapter.adapter_type(), ADAPTER_TYPE_UNKNOWN);
    }

    #[test]
    fn test_snapshot() {
        let adapter = KgpuAdapterCapsule::new();
        let snapshot = adapter.snapshot();
        assert_eq!(snapshot.state, ADAPTER_STATE_INVALID);
        assert_eq!(snapshot.device_count, 0);
        assert_eq!(snapshot.vendor_id, 0);
        assert_eq!(snapshot.device_id, 0);
    }

    #[test]
    fn test_initialize_nvidia() {
        let adapter = KgpuAdapterCapsule::new();
        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684, // RTX 4090
            ADAPTER_TYPE_DISCRETE_GPU,
            ADAPTER_CAP_COMPUTE | ADAPTER_CAP_GRAPHICS | ADAPTER_CAP_RAYTRACING,
        ).expect("Init failed");

        assert_eq!(adapter.state(), ADAPTER_STATE_READY);
        assert_eq!(adapter.vendor_id(), VENDOR_NVIDIA);
        assert_eq!(adapter.device_id(), 0x2684);
        assert_eq!(adapter.adapter_type(), ADAPTER_TYPE_DISCRETE_GPU);
        assert!(adapter.has_capability(ADAPTER_CAP_COMPUTE));
        assert!(adapter.has_capability(ADAPTER_CAP_RAYTRACING));
        assert_eq!(adapter.vendor_name(), "NVIDIA");

        // Check limits for discrete GPU
        let limits = adapter.limits();
        assert_eq!(limits.max_texture_size, 16384);
        assert_eq!(limits.max_samplers, 4096); // NVIDIA boost
    }

    #[test]
    fn test_initialize_amd() {
        let adapter = KgpuAdapterCapsule::new();
        adapter.initialize(
            VENDOR_AMD,
            0x744C, // RX 7900 XTX
            ADAPTER_TYPE_DISCRETE_GPU,
            ADAPTER_CAP_COMPUTE | ADAPTER_CAP_GRAPHICS,
        ).expect("Init failed");

        assert_eq!(adapter.vendor_name(), "AMD");
        assert_eq!(adapter.adapter_type_name(), "Discrete GPU");
    }

    #[test]
    fn test_initialize_integrated() {
        let adapter = KgpuAdapterCapsule::new();
        adapter.initialize(
            VENDOR_INTEL,
            0x9A49, // Intel Iris Xe
            ADAPTER_TYPE_INTEGRATED_GPU,
            ADAPTER_CAP_COMPUTE | ADAPTER_CAP_GRAPHICS,
        ).expect("Init failed");

        assert_eq!(adapter.adapter_type(), ADAPTER_TYPE_INTEGRATED_GPU);
        assert_eq!(adapter.adapter_type_name(), "Integrated GPU");

        // Integrated GPU has lower limits
        let limits = adapter.limits();
        assert!(limits.max_buffer_size < 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_double_initialize_fails() {
        let adapter = KgpuAdapterCapsule::new();
        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            ADAPTER_CAP_COMPUTE,
        ).unwrap();

        let result = adapter.initialize(VENDOR_AMD, 0x744C, ADAPTER_TYPE_DISCRETE_GPU, 0);
        assert!(result.is_err());

        match result {
            Err(KgpuAdapterError::InvalidState { current, expected }) => {
                assert_eq!(current, ADAPTER_STATE_READY);
                assert_eq!(expected, ADAPTER_STATE_INVALID);
            }
            _ => panic!("Expected InvalidState error"),
        }
    }

    #[test]
    fn test_request_device() {
        let adapter = KgpuAdapterCapsule::new();
        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            ADAPTER_CAP_COMPUTE,
        ).unwrap();

        let device0 = adapter.request_device().expect("Device 0 failed");
        assert_eq!(device0, 0);
        assert_eq!(adapter.state(), ADAPTER_STATE_IN_USE);
        assert_eq!(adapter.device_count(), 1);

        let device1 = adapter.request_device().expect("Device 1 failed");
        assert_eq!(device1, 1);
        assert_eq!(adapter.device_count(), 2);
    }

    #[test]
    fn test_release_device() {
        let adapter = KgpuAdapterCapsule::new();
        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            ADAPTER_CAP_COMPUTE,
        ).unwrap();

        adapter.request_device().unwrap();
        adapter.request_device().unwrap();
        assert_eq!(adapter.device_count(), 2);

        adapter.release_device().unwrap();
        assert_eq!(adapter.device_count(), 1);
        assert_eq!(adapter.state(), ADAPTER_STATE_IN_USE);

        adapter.release_device().unwrap();
        assert_eq!(adapter.device_count(), 0);
        assert_eq!(adapter.state(), ADAPTER_STATE_READY); // Back to Ready
    }

    #[test]
    fn test_is_valid() {
        let adapter = KgpuAdapterCapsule::new();
        assert!(!adapter.is_valid()); // Invalid state

        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            0,
        ).unwrap();
        assert!(adapter.is_valid());

        adapter.mark_lost();
        assert!(!adapter.is_valid());
    }

    #[test]
    fn test_mark_lost() {
        let adapter = KgpuAdapterCapsule::new();
        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            0,
        ).unwrap();

        adapter.mark_lost();
        assert_eq!(adapter.state(), ADAPTER_STATE_LOST);

        // Operations should fail on lost adapter
        let result = adapter.request_device();
        assert!(matches!(result, Err(KgpuAdapterError::AdapterLost)));
    }

    #[test]
    fn test_validate_buffer_size() {
        let adapter = KgpuAdapterCapsule::new();
        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            0,
        ).unwrap();

        // Valid size
        assert!(adapter.validate_buffer_size(1024 * 1024 * 1024).is_ok());

        // Invalid size (too large)
        let result = adapter.validate_buffer_size(10 * 1024 * 1024 * 1024);
        assert!(result.is_err());

        match result {
            Err(KgpuAdapterError::LimitExceeded { limit_name, .. }) => {
                assert_eq!(limit_name, "max_buffer_size");
            }
            _ => panic!("Expected LimitExceeded error"),
        }
    }

    #[test]
    fn test_validate_texture_size() {
        let adapter = KgpuAdapterCapsule::new();
        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            0,
        ).unwrap();

        assert!(adapter.validate_texture_size(8192).is_ok());
        assert!(adapter.validate_texture_size(16384).is_ok()); // At limit

        let result = adapter.validate_texture_size(32768);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_limits() {
        let adapter = KgpuAdapterCapsule::new();

        let custom_limits = AdapterLimits {
            max_texture_size: 32768,
            max_buffer_size: 4 * 1024 * 1024 * 1024,
            max_bind_groups: 16,
            max_samplers: 8192,
            max_compute_workgroup_size: 2048,
            max_compute_invocations: 1024 * 1024 * 64, // 67M invocations (fits in u32)
        };

        adapter.initialize_with_limits(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            ADAPTER_CAP_COMPUTE,
            custom_limits,
        ).expect("Init failed");

        let limits = adapter.limits();
        assert_eq!(limits.max_texture_size, 32768);
        assert_eq!(limits.max_bind_groups, 16);
    }

    #[test]
    fn test_backend_handle() {
        let adapter = KgpuAdapterCapsule::new();
        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            0,
        ).unwrap();

        assert_eq!(adapter.backend_handle(), 0);

        adapter.set_backend_handle(0xDEAD_BEEF_CAFE_BABE);
        assert_eq!(adapter.backend_handle(), 0xDEAD_BEEF_CAFE_BABE);
    }

    #[test]
    fn test_generation_increments() {
        let adapter = KgpuAdapterCapsule::new();
        let gen0 = adapter.generation();

        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            0,
        ).unwrap();
        let gen1 = adapter.generation();
        assert!(gen1 > gen0, "Generation should increase after init");

        adapter.request_device().unwrap();
        let gen2 = adapter.generation();
        assert!(gen2 > gen1, "Generation should increase after device request");
    }

    #[test]
    fn test_debug_format() {
        let adapter = KgpuAdapterCapsule::new();
        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            ADAPTER_CAP_COMPUTE,
        ).unwrap();

        let debug_str = format!("{:?}", adapter);
        assert!(debug_str.contains("KgpuAdapterCapsule"));
        assert!(debug_str.contains("NVIDIA"));
        assert!(debug_str.contains("Discrete GPU"));
    }

    #[test]
    fn test_default() {
        let adapter = KgpuAdapterCapsule::default();
        assert_eq!(adapter.state(), ADAPTER_STATE_INVALID);
    }

    #[test]
    fn test_capability_flags() {
        let adapter = KgpuAdapterCapsule::new();
        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            ADAPTER_CAP_COMPUTE | ADAPTER_CAP_GRAPHICS | ADAPTER_CAP_RAYTRACING | ADAPTER_CAP_MESH_SHADERS,
        ).unwrap();

        assert!(adapter.has_capability(ADAPTER_CAP_COMPUTE));
        assert!(adapter.has_capability(ADAPTER_CAP_GRAPHICS));
        assert!(adapter.has_capability(ADAPTER_CAP_RAYTRACING));
        assert!(adapter.has_capability(ADAPTER_CAP_MESH_SHADERS));
        assert!(!adapter.has_capability(ADAPTER_CAP_VRS));

        // Combined capability check
        assert!(adapter.has_capability(ADAPTER_CAP_COMPUTE | ADAPTER_CAP_GRAPHICS));
    }

    #[test]
    fn test_thread_safety_smoke() {
        use std::sync::Arc;
        use std::thread;

        let adapter = Arc::new(KgpuAdapterCapsule::new());
        adapter.initialize(
            VENDOR_NVIDIA,
            0x2684,
            ADAPTER_TYPE_DISCRETE_GPU,
            ADAPTER_CAP_COMPUTE,
        ).unwrap();

        let mut handles = vec![];

        // Spawn readers
        for _ in 0..4 {
            let adp = Arc::clone(&adapter);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = adp.state();
                    let _ = adp.capabilities();
                    let _ = adp.limits();
                    let _ = adp.snapshot();
                }
            }));
        }

        // Spawn device requesters
        for _ in 0..2 {
            let adp = Arc::clone(&adapter);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let _ = adp.request_device();
                    let _ = adp.release_device();
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Adapter should still be valid
        assert!(adapter.is_valid());
    }
}
