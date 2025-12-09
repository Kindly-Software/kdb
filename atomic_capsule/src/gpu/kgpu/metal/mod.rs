//! Metal Backend Module - KGPU Phase 5
//!
//! Provides Metal-specific GPU backend implementation for macOS/iOS platforms.
//! This is a MOCK/STUB implementation for design validation, not real Metal FFI.
//!
//! # Architecture
//!
//! The Metal backend follows the KGPU capsule architecture:
//!
//! - [`MtlBackendCapsule`]: Root backend capsule (512B) - T1 Atomic tier
//! - [`MtlDeviceCapsule`]: Logical device (256B) - T1 Atomic tier
//! - [`MtlBufferCapsule`]: GPU buffer (128B) - T1 Atomic tier
//! - [`MtlTextureCapsule`]: GPU texture (256B) - T1 Atomic tier
//!
//! # Design Goals
//!
//! 1. **100% Lockfree**: No mutex/RwLock, all atomic operations
//! 2. **Type Safety**: Strong typing for Metal concepts (pixel formats, storage modes)
//! 3. **Cache Aligned**: All capsules aligned to cache line boundaries
//! 4. **Generation Counters**: TOCTOU prevention via embedded counters
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_MOCK_BACKEND`: All handles are mock values for testing
//! - `#ASSUME_PLATFORM_DETECTION`: Platform checks are compile-time
//! - `#ASSUME_NO_REAL_FFI`: No actual Metal FFI calls
//!
//! # Example
//!
//! ```rust,ignore
//! use atomic_capsule::gpu::kgpu::metal::{MtlBackendCapsule, MtlDeviceCapsule};
//!
//! let backend = MtlBackendCapsule::new();
//! backend.initialize().expect("Backend init failed");
//!
//! if backend.device_count() > 0 {
//!     let device = MtlDeviceCapsule::new();
//!     // ... configure device
//! }
//! ```

// Sub-modules
pub mod buffer;
pub mod device;
pub mod texture;
pub mod types;

// Re-exports
pub use buffer::{
    MtlBufferCapsule, MtlBufferError, MtlBufferResult, MtlBufferSnapshot,
    BUFFER_STATE_CREATED, BUFFER_STATE_DESTROYED, BUFFER_STATE_IN_GPU_USE,
    BUFFER_STATE_MAPPED, BUFFER_STATE_UNINITIALIZED,
};
pub use device::{
    MtlDeviceCapsule, MtlDeviceError, MtlDeviceProperties, MtlDeviceResult, MtlDeviceSnapshot,
    DEVICE_STATE_ACTIVE, DEVICE_STATE_DESTROYED, DEVICE_STATE_INITIALIZING,
    DEVICE_STATE_LOST, DEVICE_STATE_READY, DEVICE_STATE_UNINITIALIZED,
};
pub use texture::{
    MtlTextureCapsule, MtlTextureDescriptor, MtlTextureError, MtlTextureResult, MtlTextureSnapshot,
    TEXTURE_STATE_CREATED, TEXTURE_STATE_DESTROYED, TEXTURE_STATE_IN_COMPUTE_PASS,
    TEXTURE_STATE_IN_RENDER_PASS, TEXTURE_STATE_UNINITIALIZED,
};
pub use types::{
    MTLGPUFamily, MTLLanguageVersion, MTLPixelFormat, MTLResourceOptions, MTLStorageMode,
    MTLTextureType, MTLTextureUsage, state::*,
};

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Backend Constants
// ============================================================================

/// Maximum devices supported by the backend
pub const MAX_METAL_DEVICES: usize = 4;

/// Backend feature: Unified Memory (Apple Silicon)
pub const FEATURE_UNIFIED_MEMORY: u32 = 1 << 0;
/// Backend feature: Apple Silicon GPU
pub const FEATURE_APPLE_SILICON: u32 = 1 << 1;
/// Backend feature: Ray Tracing support
pub const FEATURE_RAYTRACING: u32 = 1 << 2;
/// Backend feature: Mesh Shaders support
pub const FEATURE_MESH_SHADERS: u32 = 1 << 3;
/// Backend feature: Metal 3
pub const FEATURE_METAL_3: u32 = 1 << 4;
/// Backend feature: Tile Shading
pub const FEATURE_TILE_SHADING: u32 = 1 << 5;

// ============================================================================
// Bit Field Layouts
// ============================================================================

// Primary atomic: state(8) | device_count(8) | generation(48)
const STATE_SHIFT: u32 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;
const DEVICE_COUNT_SHIFT: u32 = 48;
const DEVICE_COUNT_MASK: u64 = 0xFF << DEVICE_COUNT_SHIFT;
const GENERATION_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// Secondary atomic: feature_set(32) | gpu_family(32)
const FEATURE_SET_SHIFT: u32 = 32;
const FEATURE_SET_MASK: u64 = 0xFFFF_FFFF << FEATURE_SET_SHIFT;
const GPU_FAMILY_MASK: u64 = 0x0000_0000_FFFF_FFFF;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during Metal backend operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtlBackendError {
    /// Backend is in invalid state
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
    /// Backend has been destroyed
    BackendDestroyed,
    /// No Metal devices found
    NoDevicesFound,
    /// Maximum devices reached
    MaxDevicesReached,
    /// Metal not available on this platform
    NotAvailable,
    /// Feature not supported
    FeatureNotSupported {
        /// Feature flag
        feature: u32,
    },
}

impl core::fmt::Display for MtlBackendError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidState { current, expected } => {
                write!(f, "Invalid backend state: current={}, expected={}", current, expected)
            }
            Self::TransitionFailed { expected, observed } => {
                write!(f, "Backend transition failed: expected={}, observed={}", expected, observed)
            }
            Self::BackendDestroyed => write!(f, "Metal backend has been destroyed"),
            Self::NoDevicesFound => write!(f, "No Metal devices found"),
            Self::MaxDevicesReached => write!(f, "Maximum Metal devices reached"),
            Self::NotAvailable => write!(f, "Metal is not available on this platform"),
            Self::FeatureNotSupported { feature } => {
                write!(f, "Metal feature {:08x} not supported", feature)
            }
        }
    }
}

/// Result type for Metal backend operations
pub type MtlBackendResult<T> = Result<T, MtlBackendError>;

// ============================================================================
// Backend Snapshot
// ============================================================================

/// Atomic snapshot of backend state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MtlBackendSnapshot {
    /// Current state
    pub state: u8,
    /// Number of devices
    pub device_count: u8,
    /// Generation counter
    pub generation: u64,
    /// Feature flags
    pub features: u32,
    /// GPU family
    pub gpu_family: MTLGPUFamily,
    /// Metal language version
    pub metal_version: MTLLanguageVersion,
    /// Is unified memory available
    pub unified_memory: bool,
    /// Is Apple Silicon
    pub apple_silicon: bool,
    /// Supports ray tracing
    pub supports_raytracing: bool,
    /// Command buffers submitted
    pub command_buffers_submitted: u64,
    /// Encoders created
    pub encoders_created: u64,
}

// ============================================================================
// MtlBackendCapsule
// ============================================================================

/// Metal Backend Capsule - Root backend management
///
/// Manages Metal device enumeration, feature detection, and statistics.
/// All operations are lockfree using atomic primitives.
///
/// # Tier: T1 Atomic
/// # Size: 512B (eight cache lines, prevents false sharing)
///
/// # State Machine
///
/// - `Uninitialized` (0): Backend not yet initialized
/// - `Initializing` (1): Backend initialization in progress
/// - `Ready` (2): Backend ready for use
/// - `Active` (3): Devices created
/// - `ShuttingDown` (4): Cleanup in progress
/// - `Destroyed` (5): Backend destroyed
///
/// # ASSUM Safety
///
/// - `#ASSUME_MOCK_BACKEND`: All handles are mock values
/// - `#ASSUME_STATE_MACHINE_VALID`: State transitions validated via CAS
/// - `#ASSUME_GENERATION_MONOTONIC`: Generation counter only increases
/// - `#ASSUME_PLATFORM_DETECTION`: Platform checks are compile-time
#[repr(C, align(512))]
pub struct MtlBackendCapsule {
    // ========================================================================
    // Cache Line 0 (64B): Primary coordination
    // ========================================================================
    /// Primary coordination channel
    ///
    /// Layout: state(8) | device_count(8) | generation(48)
    primary: AtomicU64,

    /// Padding to complete first cache line
    _padding0: [u8; 56],

    // ========================================================================
    // Cache Line 1 (64B): Secondary coordination
    // ========================================================================
    /// Secondary coordination channel
    ///
    /// Layout: feature_set(32) | gpu_family(32)
    secondary: AtomicU64,

    /// Padding to complete second cache line
    _padding1: [u8; 56],

    // ========================================================================
    // Cache Line 2 (64B): Version info
    // ========================================================================
    /// Metal language version (encoded)
    metal_version: AtomicU32,

    /// OS version (encoded as major.minor.patch)
    os_version: AtomicU32,

    /// Padding to complete third cache line
    _padding2: [u8; 56],

    // ========================================================================
    // Cache Line 3 (64B): Device handles
    // ========================================================================
    /// Mock device handles (4 devices max)
    ///
    /// #ASSUME_MOCK_BACKEND: These are mock values, not real MTLDevice pointers
    device_handles: [AtomicU64; MAX_METAL_DEVICES],

    /// Padding to complete fourth cache line
    _padding3: [u8; 32],

    // ========================================================================
    // Cache Line 4 (64B): Feature flags
    // ========================================================================
    /// Has unified memory (Apple Silicon)
    unified_memory: AtomicBool,

    /// Is Apple Silicon
    apple_silicon: AtomicBool,

    /// Supports ray tracing
    supports_raytracing: AtomicBool,

    /// Supports mesh shaders
    supports_mesh_shaders: AtomicBool,

    /// Padding to complete fifth cache line
    _padding4: [u8; 60],

    // ========================================================================
    // Cache Line 5 (64B): Statistics
    // ========================================================================
    /// Command buffers submitted
    command_buffers_submitted: AtomicU64,

    /// Encoders created
    encoders_created: AtomicU64,

    /// Padding to complete sixth cache line
    _padding5: [u8; 48],

    // ========================================================================
    // Cache Lines 6-7 (128B): Reserved
    // ========================================================================
    _reserved: [u8; 128],
}

// Compile-time size and alignment verification
const _: () = {
    assert!(core::mem::size_of::<MtlBackendCapsule>() == 512);
    assert!(core::mem::align_of::<MtlBackendCapsule>() == 512);
};

impl MtlBackendCapsule {
    /// Creates a new Metal backend in `Uninitialized` state.
    ///
    /// # Performance
    ///
    /// O(1), ~20ns (stack allocation + atomic init)
    #[inline]
    pub const fn new() -> Self {
        Self {
            primary: AtomicU64::new(0),
            _padding0: [0u8; 56],

            secondary: AtomicU64::new(0),
            _padding1: [0u8; 56],

            metal_version: AtomicU32::new(MTLLanguageVersion::Version2_4 as u32),
            os_version: AtomicU32::new(0),
            _padding2: [0u8; 56],

            device_handles: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            _padding3: [0u8; 32],

            unified_memory: AtomicBool::new(false),
            apple_silicon: AtomicBool::new(false),
            supports_raytracing: AtomicBool::new(false),
            supports_mesh_shaders: AtomicBool::new(false),
            _padding4: [0u8; 60],

            command_buffers_submitted: AtomicU64::new(0),
            encoders_created: AtomicU64::new(0),
            _padding5: [0u8; 48],

            _reserved: [0u8; 128],
        }
    }

    /// Returns the current state.
    #[inline]
    pub fn state(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & STATE_MASK) >> STATE_SHIFT) as u8
    }

    /// Returns the device count.
    #[inline]
    pub fn device_count(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & DEVICE_COUNT_MASK) >> DEVICE_COUNT_SHIFT) as u8
    }

    /// Returns the generation counter.
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Returns the feature flags.
    #[inline]
    pub fn features(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & FEATURE_SET_MASK) >> FEATURE_SET_SHIFT) as u32
    }

    /// Returns the GPU family.
    #[inline]
    pub fn gpu_family(&self) -> MTLGPUFamily {
        let secondary = self.secondary.load(Ordering::Acquire);
        let family = (secondary & GPU_FAMILY_MASK) as u32;
        match family {
            1001 => MTLGPUFamily::Apple1,
            1002 => MTLGPUFamily::Apple2,
            1003 => MTLGPUFamily::Apple3,
            1004 => MTLGPUFamily::Apple4,
            1005 => MTLGPUFamily::Apple5,
            1006 => MTLGPUFamily::Apple6,
            1007 => MTLGPUFamily::Apple7,
            1008 => MTLGPUFamily::Apple8,
            1009 => MTLGPUFamily::Apple9,
            2001 => MTLGPUFamily::Mac1,
            2002 => MTLGPUFamily::Mac2,
            _ => MTLGPUFamily::Unknown,
        }
    }

    /// Returns the Metal language version.
    #[inline]
    pub fn metal_version(&self) -> MTLLanguageVersion {
        let version = self.metal_version.load(Ordering::Acquire);
        match version {
            0x10000 => MTLLanguageVersion::Version1_0,
            0x10001 => MTLLanguageVersion::Version1_1,
            0x10002 => MTLLanguageVersion::Version1_2,
            0x20000 => MTLLanguageVersion::Version2_0,
            0x20001 => MTLLanguageVersion::Version2_1,
            0x20002 => MTLLanguageVersion::Version2_2,
            0x20003 => MTLLanguageVersion::Version2_3,
            0x20004 => MTLLanguageVersion::Version2_4,
            0x30000 => MTLLanguageVersion::Version3_0,
            0x30001 => MTLLanguageVersion::Version3_1,
            _ => MTLLanguageVersion::Version2_4,
        }
    }

    /// Returns whether unified memory is available.
    #[inline]
    pub fn has_unified_memory(&self) -> bool {
        self.unified_memory.load(Ordering::Acquire)
    }

    /// Returns whether this is Apple Silicon.
    #[inline]
    pub fn is_apple_silicon(&self) -> bool {
        self.apple_silicon.load(Ordering::Acquire)
    }

    /// Returns whether ray tracing is supported.
    #[inline]
    pub fn supports_raytracing(&self) -> bool {
        self.supports_raytracing.load(Ordering::Acquire)
    }

    /// Returns whether mesh shaders are supported.
    #[inline]
    pub fn supports_mesh_shaders(&self) -> bool {
        self.supports_mesh_shaders.load(Ordering::Acquire)
    }

    /// Returns command buffers submitted count.
    #[inline]
    pub fn command_buffers_submitted(&self) -> u64 {
        self.command_buffers_submitted.load(Ordering::Acquire)
    }

    /// Returns encoders created count.
    #[inline]
    pub fn encoders_created(&self) -> u64 {
        self.encoders_created.load(Ordering::Acquire)
    }

    /// Takes an atomic snapshot of the backend state.
    ///
    /// # Performance
    ///
    /// ~60ns (multiple atomic loads)
    pub fn snapshot(&self) -> MtlBackendSnapshot {
        let primary = self.primary.load(Ordering::Acquire);
        let secondary = self.secondary.load(Ordering::Acquire);

        MtlBackendSnapshot {
            state: ((primary & STATE_MASK) >> STATE_SHIFT) as u8,
            device_count: ((primary & DEVICE_COUNT_MASK) >> DEVICE_COUNT_SHIFT) as u8,
            generation: primary & GENERATION_MASK,
            features: ((secondary & FEATURE_SET_MASK) >> FEATURE_SET_SHIFT) as u32,
            gpu_family: self.gpu_family(),
            metal_version: self.metal_version(),
            unified_memory: self.unified_memory.load(Ordering::Acquire),
            apple_silicon: self.apple_silicon.load(Ordering::Acquire),
            supports_raytracing: self.supports_raytracing.load(Ordering::Acquire),
            command_buffers_submitted: self.command_buffers_submitted.load(Ordering::Acquire),
            encoders_created: self.encoders_created.load(Ordering::Acquire),
        }
    }

    /// Initializes the Metal backend.
    ///
    /// # Performance
    ///
    /// <100ns (CAS + atomic stores)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_STATE_MACHINE_VALID`: Validates transition is legal
    /// - `#ASSUME_MOCK_BACKEND`: Simulates Metal device detection
    pub fn initialize(&self) -> MtlBackendResult<()> {
        // #ASSUME_STATE_MACHINE_VALID: Transition from Uninitialized to Initializing
        let current = self.primary.load(Ordering::Acquire);
        let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

        if current_state != BACKEND_STATE_UNINITIALIZED {
            return Err(MtlBackendError::InvalidState {
                current: current_state,
                expected: BACKEND_STATE_UNINITIALIZED,
            });
        }

        // Transition to Initializing
        let current_gen = current & GENERATION_MASK;
        let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
        let new_primary = ((BACKEND_STATE_INITIALIZING as u64) << STATE_SHIFT) | new_gen;

        match self.primary.compare_exchange(
            current,
            new_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {}
            Err(observed) => {
                let observed_state = ((observed & STATE_MASK) >> STATE_SHIFT) as u8;
                return Err(MtlBackendError::TransitionFailed {
                    expected: BACKEND_STATE_UNINITIALIZED,
                    observed: observed_state,
                });
            }
        }

        // #ASSUME_MOCK_BACKEND: Simulate Metal detection
        // In a real implementation, this would call Metal APIs
        self.detect_platform_features();

        // Simulate finding 1 Metal device
        let device_count = 1u8;
        let mock_device_handle = 0x4D544C44_0000_0001u64; // "MTLD" in hex
        self.device_handles[0].store(mock_device_handle, Ordering::Release);

        // Set secondary (features + GPU family)
        let features = if self.apple_silicon.load(Ordering::Acquire) {
            FEATURE_UNIFIED_MEMORY | FEATURE_APPLE_SILICON | FEATURE_RAYTRACING | FEATURE_TILE_SHADING
        } else {
            FEATURE_METAL_3
        };
        let gpu_family = MTLGPUFamily::Apple8 as u64;
        let secondary_value = ((features as u64) << FEATURE_SET_SHIFT) | gpu_family;
        self.secondary.store(secondary_value, Ordering::Release);

        // Transition to Ready
        let ready_gen = new_gen.wrapping_add(1) & GENERATION_MASK;
        let ready_primary = ((BACKEND_STATE_READY as u64) << STATE_SHIFT)
            | ((device_count as u64) << DEVICE_COUNT_SHIFT)
            | ready_gen;

        match self.primary.compare_exchange(
            new_primary,
            ready_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(observed) => {
                let observed_state = ((observed & STATE_MASK) >> STATE_SHIFT) as u8;
                Err(MtlBackendError::TransitionFailed {
                    expected: BACKEND_STATE_INITIALIZING,
                    observed: observed_state,
                })
            }
        }
    }

    /// Gets a device handle by index.
    ///
    /// # Performance
    ///
    /// <20ns (atomic loads + bounds check)
    pub fn get_device_handle(&self, index: usize) -> MtlBackendResult<u64> {
        let state = self.state();
        if state == BACKEND_STATE_DESTROYED {
            return Err(MtlBackendError::BackendDestroyed);
        }
        if state != BACKEND_STATE_READY && state != BACKEND_STATE_ACTIVE {
            return Err(MtlBackendError::InvalidState {
                current: state,
                expected: BACKEND_STATE_READY,
            });
        }

        let count = self.device_count() as usize;
        if index >= count {
            return Err(MtlBackendError::NoDevicesFound);
        }

        Ok(self.device_handles[index].load(Ordering::Acquire))
    }

    /// Checks if a feature is supported.
    #[inline]
    pub fn has_feature(&self, feature: u32) -> bool {
        (self.features() & feature) == feature
    }

    /// Checks if Metal is available on this platform.
    ///
    /// #ASSUME_PLATFORM_DETECTION: Uses compile-time platform detection
    #[inline]
    pub fn is_available() -> bool {
        // Metal is available on macOS and iOS
        cfg!(any(target_os = "macos", target_os = "ios", target_os = "tvos"))
    }

    /// Tracks a command buffer submission.
    #[inline]
    pub fn track_command_buffer_submitted(&self) {
        self.command_buffers_submitted.fetch_add(1, Ordering::AcqRel);
    }

    /// Tracks an encoder creation.
    #[inline]
    pub fn track_encoder_created(&self) {
        self.encoders_created.fetch_add(1, Ordering::AcqRel);
    }

    /// Destroys the backend.
    ///
    /// # Performance
    ///
    /// <50ns (CAS)
    pub fn destroy(&self) -> MtlBackendResult<()> {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

            if current_state == BACKEND_STATE_DESTROYED {
                return Err(MtlBackendError::BackendDestroyed);
            }

            let current_gen = current & GENERATION_MASK;
            let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
            let destroyed = ((BACKEND_STATE_DESTROYED as u64) << STATE_SHIFT) | new_gen;

            if self
                .primary
                .compare_exchange(current, destroyed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
        }
    }

    /// Checks if the backend is valid.
    #[inline]
    pub fn is_valid(&self) -> bool {
        let state = self.state();
        state != BACKEND_STATE_DESTROYED && state != BACKEND_STATE_UNINITIALIZED
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    /// Detects platform features (mock implementation)
    fn detect_platform_features(&self) {
        // #ASSUME_MOCK_BACKEND: Simulate Apple Silicon detection
        // In a real implementation, this would query actual hardware

        // Assume Apple Silicon (M1/M2) for mock
        self.apple_silicon.store(true, Ordering::Release);
        self.unified_memory.store(true, Ordering::Release);
        self.supports_raytracing.store(true, Ordering::Release);
        self.supports_mesh_shaders.store(true, Ordering::Release);

        // Set Metal 3.1 (latest)
        self.metal_version.store(MTLLanguageVersion::Version3_1 as u32, Ordering::Release);

        // macOS 14.0 = 0x0E0000
        self.os_version.store(0x0E_00_00, Ordering::Release);
    }
}

impl Default for MtlBackendCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for MtlBackendCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let snapshot = self.snapshot();
        f.debug_struct("MtlBackendCapsule")
            .field("state", &snapshot.state)
            .field("device_count", &snapshot.device_count)
            .field("gpu_family", &snapshot.gpu_family)
            .field("metal_version", &snapshot.metal_version)
            .field("apple_silicon", &snapshot.apple_silicon)
            .field("unified_memory", &snapshot.unified_memory)
            .field("supports_raytracing", &snapshot.supports_raytracing)
            .finish()
    }
}

// SAFETY: All operations are atomic; no mutable aliasing possible
unsafe impl Send for MtlBackendCapsule {}
unsafe impl Sync for MtlBackendCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<MtlBackendCapsule>(), 512);
        assert_eq!(core::mem::align_of::<MtlBackendCapsule>(), 512);
    }

    #[test]
    fn test_backend_initial_state() {
        let backend = MtlBackendCapsule::new();
        assert_eq!(backend.state(), BACKEND_STATE_UNINITIALIZED);
        assert_eq!(backend.device_count(), 0);
        assert_eq!(backend.generation(), 0);
        assert!(!backend.has_unified_memory());
        assert!(!backend.is_apple_silicon());
    }

    #[test]
    fn test_backend_initialize() {
        let backend = MtlBackendCapsule::new();
        backend.initialize().expect("Init failed");

        assert_eq!(backend.state(), BACKEND_STATE_READY);
        assert_eq!(backend.device_count(), 1);
        assert!(backend.has_unified_memory());
        assert!(backend.is_apple_silicon());
        assert!(backend.supports_raytracing());
        assert!(backend.is_valid());
    }

    #[test]
    fn test_backend_double_initialize_fails() {
        let backend = MtlBackendCapsule::new();
        backend.initialize().unwrap();

        let result = backend.initialize();
        assert!(result.is_err());
    }

    #[test]
    fn test_backend_get_device_handle() {
        let backend = MtlBackendCapsule::new();
        backend.initialize().unwrap();

        let handle = backend.get_device_handle(0).expect("Get device failed");
        assert_ne!(handle, 0);

        // Out of bounds
        let result = backend.get_device_handle(1);
        assert!(matches!(result, Err(MtlBackendError::NoDevicesFound)));
    }

    #[test]
    fn test_backend_has_feature() {
        let backend = MtlBackendCapsule::new();
        backend.initialize().unwrap();

        assert!(backend.has_feature(FEATURE_UNIFIED_MEMORY));
        assert!(backend.has_feature(FEATURE_APPLE_SILICON));
        assert!(backend.has_feature(FEATURE_RAYTRACING));
    }

    #[test]
    fn test_backend_statistics() {
        let backend = MtlBackendCapsule::new();
        backend.initialize().unwrap();

        backend.track_command_buffer_submitted();
        backend.track_command_buffer_submitted();
        backend.track_encoder_created();

        assert_eq!(backend.command_buffers_submitted(), 2);
        assert_eq!(backend.encoders_created(), 1);
    }

    #[test]
    fn test_backend_destroy() {
        let backend = MtlBackendCapsule::new();
        backend.initialize().unwrap();

        backend.destroy().expect("Destroy failed");
        assert_eq!(backend.state(), BACKEND_STATE_DESTROYED);
        assert!(!backend.is_valid());
    }

    #[test]
    fn test_backend_snapshot() {
        let backend = MtlBackendCapsule::new();
        backend.initialize().unwrap();
        backend.track_command_buffer_submitted();

        let snapshot = backend.snapshot();
        assert_eq!(snapshot.state, BACKEND_STATE_READY);
        assert_eq!(snapshot.device_count, 1);
        assert!(snapshot.apple_silicon);
        assert!(snapshot.unified_memory);
        assert_eq!(snapshot.command_buffers_submitted, 1);
    }

    #[test]
    fn test_backend_generation_increments() {
        let backend = MtlBackendCapsule::new();
        let gen0 = backend.generation();

        backend.initialize().unwrap();
        let gen1 = backend.generation();
        assert!(gen1 > gen0);

        backend.destroy().unwrap();
        let gen2 = backend.generation();
        assert!(gen2 > gen1);
    }

    #[test]
    fn test_backend_debug_format() {
        let backend = MtlBackendCapsule::new();
        backend.initialize().unwrap();

        let debug_str = format!("{:?}", backend);
        assert!(debug_str.contains("MtlBackendCapsule"));
        assert!(debug_str.contains("apple_silicon"));
    }

    #[test]
    fn test_backend_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let backend = Arc::new(MtlBackendCapsule::new());
        backend.initialize().unwrap();

        let mut handles = vec![];

        // Spawn readers
        for _ in 0..4 {
            let be = Arc::clone(&backend);
            handles.push(thread::spawn(move || {
                for _ in 0..500 {
                    let _ = be.snapshot();
                    let _ = be.state();
                    let _ = be.device_count();
                    let _ = be.features();
                }
            }));
        }

        // Spawn stat trackers
        for _ in 0..2 {
            let be = Arc::clone(&backend);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    be.track_command_buffer_submitted();
                    be.track_encoder_created();
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        assert!(backend.is_valid());
        assert!(backend.command_buffers_submitted() > 0);
        assert!(backend.encoders_created() > 0);
    }

    // Device capsule tests
    #[test]
    fn test_device_capsule_size() {
        assert_eq!(core::mem::size_of::<MtlDeviceCapsule>(), 256);
        assert_eq!(core::mem::align_of::<MtlDeviceCapsule>(), 256);
    }

    // Buffer capsule tests
    #[test]
    fn test_buffer_capsule_size() {
        assert_eq!(core::mem::size_of::<MtlBufferCapsule>(), 128);
        assert_eq!(core::mem::align_of::<MtlBufferCapsule>(), 128);
    }

    // Texture capsule tests
    #[test]
    fn test_texture_capsule_size() {
        assert_eq!(core::mem::size_of::<MtlTextureCapsule>(), 256);
        assert_eq!(core::mem::align_of::<MtlTextureCapsule>(), 256);
    }
}
