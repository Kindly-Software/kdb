//! KGPU Backend Dispatcher - T1 Atomic, 256B cache-aligned
//!
//! Provides runtime backend selection for KGPU with platform-specific preferences.
//! Handles Vulkan/Metal/DX12/WebGPU backend detection and selection.
//!
//! # Design
//!
//! **Tier**: T1 Atomic (<100ns operations)
//! **Size**: 256B cache-aligned (four 64-byte cache lines)
//! **Performance Targets**:
//! - Backend detection: <50ns (compile-time + atomic loads)
//! - Backend selection: <30ns (atomic CAS)
//! - Active query: <10ns (single atomic load)
//!
//! # Memory Layout
//!
//! ```text
//! KgpuBackendDispatcher (256B, four cache lines)
//! +-- Cache Line 0 (64B): Primary coordination
//! |   +-- primary: AtomicU64 (8B) - state(8)|active_backend(8)|backend_count(8)|generation(40)
//! |   +-- secondary: AtomicU64 (8B) - available_backends(32)|flags(32)
//! |   +-- _padding0: [u8; 48]
//! +-- Cache Line 1 (64B): Backend handles
//! |   +-- vulkan_handle: AtomicU64 (8B)
//! |   +-- metal_handle: AtomicU64 (8B)
//! |   +-- dx12_handle: AtomicU64 (8B)
//! |   +-- webgpu_handle: AtomicU64 (8B)
//! |   +-- _padding1: [u8; 32]
//! +-- Cache Line 2 (64B): Backend statistics
//! |   +-- vulkan_device_count: AtomicU32 (4B)
//! |   +-- metal_device_count: AtomicU32 (4B)
//! |   +-- dx12_device_count: AtomicU32 (4B)
//! |   +-- webgpu_device_count: AtomicU32 (4B)
//! |   +-- _padding2: [u8; 48]
//! +-- Cache Line 3 (64B): Reserved
//!     +-- _reserved: [u8; 64]
//! ```
//!
//! # Platform Detection
//!
//! The dispatcher automatically detects available backends based on compile-time
//! platform flags:
//!
//! | Platform | Preferred | Available |
//! |----------|-----------|-----------|
//! | macOS | Metal | Metal, (Vulkan via MoltenVK) |
//! | iOS | Metal | Metal |
//! | Windows | DX12 | DX12, Vulkan |
//! | Linux | Vulkan | Vulkan |
//! | Android | Vulkan | Vulkan |
//! | Web | WebGPU | WebGPU |
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_PLATFORM_DETECTION_CONST`: Platform detection is compile-time
//! - `#ASSUME_STATE_MACHINE_VALID`: State transitions validated via CAS
//! - `#ASSUME_GENERATION_MONOTONIC`: Generation counter only increases
//! - `#ASSUME_BACKEND_HANDLES_MOCK`: Backend handles are mock values for testing
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination)
//! - **Q33**: 256B alignment verified at compile time
//! - **Q34**: Generation counter enables audit trail integration

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::hal::BackendType;

// ============================================================================
// State Constants
// ============================================================================

/// Dispatcher state: Uninitialized
pub const DISPATCHER_STATE_UNINITIALIZED: u8 = 0;
/// Dispatcher state: Detecting backends
pub const DISPATCHER_STATE_DETECTING: u8 = 1;
/// Dispatcher state: Ready (backends detected)
pub const DISPATCHER_STATE_READY: u8 = 2;
/// Dispatcher state: Active (backend selected)
pub const DISPATCHER_STATE_ACTIVE: u8 = 3;
/// Dispatcher state: Error (no backends available)
pub const DISPATCHER_STATE_ERROR: u8 = 4;

// ============================================================================
// Bit Field Layouts
// ============================================================================

// Primary atomic: state(8) | active_backend(8) | backend_count(8) | generation(40)
const STATE_SHIFT: u32 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;
const ACTIVE_BACKEND_SHIFT: u32 = 48;
const ACTIVE_BACKEND_MASK: u64 = 0xFF << ACTIVE_BACKEND_SHIFT;
const BACKEND_COUNT_SHIFT: u32 = 40;
const BACKEND_COUNT_MASK: u64 = 0xFF << BACKEND_COUNT_SHIFT;
const GENERATION_MASK: u64 = 0x0000_00FF_FFFF_FFFF;

// Secondary atomic: available_backends(32) | flags(32)
const AVAILABLE_BACKENDS_SHIFT: u32 = 32;
const AVAILABLE_BACKENDS_MASK: u64 = 0xFFFF_FFFF << AVAILABLE_BACKENDS_SHIFT;
const FLAGS_MASK: u64 = 0x0000_0000_FFFF_FFFF;

// ============================================================================
// Backend Flags (bitfield)
// ============================================================================

/// Backend flag: Vulkan available
pub const BACKEND_FLAG_VULKAN: u32 = 1 << 0;
/// Backend flag: Metal available
pub const BACKEND_FLAG_METAL: u32 = 1 << 1;
/// Backend flag: DX12 available
pub const BACKEND_FLAG_DX12: u32 = 1 << 2;
/// Backend flag: WebGPU available
pub const BACKEND_FLAG_WEBGPU: u32 = 1 << 3;
/// Backend flag: Null/test backend available
pub const BACKEND_FLAG_NULL: u32 = 1 << 7;

// ============================================================================
// Dispatcher Flags
// ============================================================================

/// Dispatcher flag: Auto-select best backend
pub const FLAG_AUTO_SELECT: u32 = 1 << 0;
/// Dispatcher flag: Prefer discrete GPU
pub const FLAG_PREFER_DISCRETE: u32 = 1 << 1;
/// Dispatcher flag: Prefer low power
pub const FLAG_PREFER_LOW_POWER: u32 = 1 << 2;
/// Dispatcher flag: Allow software rendering
pub const FLAG_ALLOW_SOFTWARE: u32 = 1 << 3;
/// Dispatcher flag: Enable validation layers
pub const FLAG_ENABLE_VALIDATION: u32 = 1 << 4;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during backend dispatch operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatcherError {
    /// Dispatcher is in invalid state for the requested operation
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
    /// No backends available
    NoBackendsAvailable,
    /// Requested backend not available
    BackendNotAvailable {
        /// The unavailable backend
        backend: BackendType,
    },
    /// Backend already selected
    BackendAlreadySelected {
        /// Currently selected backend
        current: BackendType,
    },
    /// Invalid backend type
    InvalidBackend,
}

impl core::fmt::Display for DispatcherError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidState { current, expected } => {
                write!(f, "Invalid dispatcher state: current={}, expected={}", current, expected)
            }
            Self::TransitionFailed { expected, observed } => {
                write!(f, "Dispatcher transition failed: expected={}, observed={}", expected, observed)
            }
            Self::NoBackendsAvailable => write!(f, "No GPU backends available"),
            Self::BackendNotAvailable { backend } => {
                write!(f, "Backend {:?} not available on this platform", backend)
            }
            Self::BackendAlreadySelected { current } => {
                write!(f, "Backend already selected: {:?}", current)
            }
            Self::InvalidBackend => write!(f, "Invalid backend type"),
        }
    }
}

/// Result type for dispatcher operations
pub type DispatcherResult<T> = Result<T, DispatcherError>;

// ============================================================================
// Dispatcher Snapshot
// ============================================================================

/// Atomic snapshot of dispatcher state for debugging/monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatcherSnapshot {
    /// Current state (0-4)
    pub state: u8,
    /// Active backend (if selected)
    pub active_backend: Option<BackendType>,
    /// Number of available backends
    pub backend_count: u8,
    /// Generation counter
    pub generation: u64,
    /// Available backend flags
    pub available_backends: u32,
    /// Dispatcher flags
    pub flags: u32,
    /// Vulkan device count
    pub vulkan_device_count: u32,
    /// Metal device count
    pub metal_device_count: u32,
    /// DX12 device count
    pub dx12_device_count: u32,
    /// WebGPU device count
    pub webgpu_device_count: u32,
}

// ============================================================================
// KgpuBackendDispatcher
// ============================================================================

/// KGPU Backend Dispatcher - Runtime backend selection
///
/// Manages backend detection and selection for KGPU across platforms.
/// All operations are lockfree using atomic primitives.
///
/// # Tier: T1 Atomic
/// # Size: 256B (four cache lines, prevents false sharing)
///
/// # State Machine
///
/// - `Uninitialized` (0): Dispatcher not yet initialized
/// - `Detecting` (1): Backend detection in progress
/// - `Ready` (2): Backends detected, ready for selection
/// - `Active` (3): Backend selected and active
/// - `Error` (4): No backends available
///
/// # ASSUM Safety
///
/// - `#ASSUME_PLATFORM_DETECTION_CONST`: Platform detection is compile-time
/// - `#ASSUME_STATE_MACHINE_VALID`: State transitions validated via CAS
/// - `#ASSUME_GENERATION_MONOTONIC`: Generation counter only increases
#[repr(C, align(256))]
pub struct KgpuBackendDispatcher {
    // ========================================================================
    // Cache Line 0: Primary coordination
    // ========================================================================
    /// Primary coordination channel
    ///
    /// Layout: state(8) | active_backend(8) | backend_count(8) | generation(40)
    primary: AtomicU64,

    /// Secondary coordination channel
    ///
    /// Layout: available_backends(32) | flags(32)
    secondary: AtomicU64,

    /// Padding to complete first cache line
    _padding0: [u8; 48],

    // ========================================================================
    // Cache Line 1: Backend handles
    // ========================================================================
    /// Mock Vulkan backend handle (if available)
    vulkan_handle: AtomicU64,

    /// Mock Metal backend handle (if available)
    metal_handle: AtomicU64,

    /// Mock DX12 backend handle (if available)
    dx12_handle: AtomicU64,

    /// Mock WebGPU backend handle (if available)
    webgpu_handle: AtomicU64,

    /// Padding to complete second cache line
    _padding1: [u8; 32],

    // ========================================================================
    // Cache Line 2: Backend statistics
    // ========================================================================
    /// Number of Vulkan devices
    vulkan_device_count: AtomicU32,

    /// Number of Metal devices
    metal_device_count: AtomicU32,

    /// Number of DX12 devices
    dx12_device_count: AtomicU32,

    /// Number of WebGPU devices
    webgpu_device_count: AtomicU32,

    /// Padding to complete third cache line
    _padding2: [u8; 48],

    // ========================================================================
    // Cache Line 3: Reserved
    // ========================================================================
    /// Reserved for future use
    _reserved: [u8; 64],
}

// Compile-time size and alignment verification
const _: () = {
    assert!(core::mem::size_of::<KgpuBackendDispatcher>() == 256);
    assert!(core::mem::align_of::<KgpuBackendDispatcher>() == 256);
};

impl KgpuBackendDispatcher {
    /// Creates a new dispatcher in `Uninitialized` state.
    ///
    /// # Performance
    ///
    /// O(1), ~10ns (stack allocation + atomic init)
    #[inline]
    pub const fn new() -> Self {
        Self {
            primary: AtomicU64::new(0),
            secondary: AtomicU64::new(0),
            _padding0: [0u8; 48],

            vulkan_handle: AtomicU64::new(0),
            metal_handle: AtomicU64::new(0),
            dx12_handle: AtomicU64::new(0),
            webgpu_handle: AtomicU64::new(0),
            _padding1: [0u8; 32],

            vulkan_device_count: AtomicU32::new(0),
            metal_device_count: AtomicU32::new(0),
            dx12_device_count: AtomicU32::new(0),
            webgpu_device_count: AtomicU32::new(0),
            _padding2: [0u8; 48],

            _reserved: [0u8; 64],
        }
    }

    /// Returns the current state.
    #[inline]
    pub fn state(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & STATE_MASK) >> STATE_SHIFT) as u8
    }

    /// Returns the active backend (if selected).
    #[inline]
    pub fn active_backend(&self) -> Option<BackendType> {
        let state = self.state();
        if state != DISPATCHER_STATE_ACTIVE {
            return None;
        }

        let primary = self.primary.load(Ordering::Acquire);
        let backend_id = ((primary & ACTIVE_BACKEND_MASK) >> ACTIVE_BACKEND_SHIFT) as u8;
        Self::backend_from_id(backend_id)
    }

    /// Returns the number of available backends.
    #[inline]
    pub fn backend_count(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & BACKEND_COUNT_MASK) >> BACKEND_COUNT_SHIFT) as u8
    }

    /// Returns the generation counter.
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Returns the available backend flags.
    #[inline]
    pub fn available_backends(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & AVAILABLE_BACKENDS_MASK) >> AVAILABLE_BACKENDS_SHIFT) as u32
    }

    /// Returns the dispatcher flags.
    #[inline]
    pub fn flags(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        (secondary & FLAGS_MASK) as u32
    }

    /// Takes an atomic snapshot of the dispatcher state.
    ///
    /// # Performance
    ///
    /// ~40ns (multiple atomic loads)
    pub fn snapshot(&self) -> DispatcherSnapshot {
        let primary = self.primary.load(Ordering::Acquire);
        let secondary = self.secondary.load(Ordering::Acquire);

        let state = ((primary & STATE_MASK) >> STATE_SHIFT) as u8;
        let backend_id = ((primary & ACTIVE_BACKEND_MASK) >> ACTIVE_BACKEND_SHIFT) as u8;

        DispatcherSnapshot {
            state,
            active_backend: if state == DISPATCHER_STATE_ACTIVE {
                Self::backend_from_id(backend_id)
            } else {
                None
            },
            backend_count: ((primary & BACKEND_COUNT_MASK) >> BACKEND_COUNT_SHIFT) as u8,
            generation: primary & GENERATION_MASK,
            available_backends: ((secondary & AVAILABLE_BACKENDS_MASK) >> AVAILABLE_BACKENDS_SHIFT) as u32,
            flags: (secondary & FLAGS_MASK) as u32,
            vulkan_device_count: self.vulkan_device_count.load(Ordering::Acquire),
            metal_device_count: self.metal_device_count.load(Ordering::Acquire),
            dx12_device_count: self.dx12_device_count.load(Ordering::Acquire),
            webgpu_device_count: self.webgpu_device_count.load(Ordering::Acquire),
        }
    }

    /// Detects available backends on the current platform.
    ///
    /// # Performance
    ///
    /// <100ns (CAS + compile-time platform detection)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_PLATFORM_DETECTION_CONST`: Platform detection is compile-time
    /// - `#ASSUME_STATE_MACHINE_VALID`: Validates transition is legal
    pub fn detect_backends(&self) -> DispatcherResult<u8> {
        // Transition from Uninitialized to Detecting
        let current = self.primary.load(Ordering::Acquire);
        let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

        if current_state != DISPATCHER_STATE_UNINITIALIZED {
            return Err(DispatcherError::InvalidState {
                current: current_state,
                expected: DISPATCHER_STATE_UNINITIALIZED,
            });
        }

        let current_gen = current & GENERATION_MASK;
        let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;
        let detecting_primary = ((DISPATCHER_STATE_DETECTING as u64) << STATE_SHIFT) | new_gen;

        match self.primary.compare_exchange(
            current,
            detecting_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {}
            Err(observed) => {
                let observed_state = ((observed & STATE_MASK) >> STATE_SHIFT) as u8;
                return Err(DispatcherError::TransitionFailed {
                    expected: DISPATCHER_STATE_UNINITIALIZED,
                    observed: observed_state,
                });
            }
        }

        // #ASSUME_PLATFORM_DETECTION_CONST: Detect available backends
        let mut available = 0u32;
        let mut count = 0u8;

        // Vulkan: Linux, Windows, Android
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "android"))]
        {
            available |= BACKEND_FLAG_VULKAN;
            count += 1;
            // Mock: 1 Vulkan device ("VKBK" in hex)
            self.vulkan_device_count.store(1, Ordering::Release);
            self.vulkan_handle.store(0x564B424B_0000_0001, Ordering::Release);
        }

        // Metal: macOS, iOS
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            available |= BACKEND_FLAG_METAL;
            count += 1;
            // Mock: 1 Metal device ("MTLB" in hex)
            self.metal_device_count.store(1, Ordering::Release);
            self.metal_handle.store(0x4D544C42_0000_0001, Ordering::Release);
        }

        // DX12: Windows
        #[cfg(target_os = "windows")]
        {
            available |= BACKEND_FLAG_DX12;
            count += 1;
            // Mock: 1 DX12 device ("DX12" in hex approximation)
            self.dx12_device_count.store(1, Ordering::Release);
            self.dx12_handle.store(0x44583132_0000_0001, Ordering::Release);
        }

        // WebGPU: WASM
        #[cfg(target_arch = "wasm32")]
        {
            available |= BACKEND_FLAG_WEBGPU;
            count += 1;
            // Mock: 1 WebGPU device ("WGPU" in hex)
            self.webgpu_device_count.store(1, Ordering::Release);
            self.webgpu_handle.store(0x57475055_0000_0001, Ordering::Release);
        }

        // Null backend always available (for testing)
        available |= BACKEND_FLAG_NULL;

        // If no real backends, count the null backend
        if count == 0 {
            count = 1;
        }

        // Set secondary (available backends + default flags)
        let default_flags = FLAG_AUTO_SELECT | FLAG_PREFER_DISCRETE;
        let secondary_value = ((available as u64) << AVAILABLE_BACKENDS_SHIFT) | (default_flags as u64);
        self.secondary.store(secondary_value, Ordering::Release);

        // Determine final state
        let final_state = if count > 0 || (available & !BACKEND_FLAG_NULL) != 0 {
            DISPATCHER_STATE_READY
        } else {
            DISPATCHER_STATE_ERROR
        };

        // Transition to Ready or Error
        let ready_gen = new_gen.wrapping_add(1) & GENERATION_MASK;
        let ready_primary = ((final_state as u64) << STATE_SHIFT)
            | ((count as u64) << BACKEND_COUNT_SHIFT)
            | ready_gen;

        match self.primary.compare_exchange(
            detecting_primary,
            ready_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(count),
            Err(observed) => {
                let observed_state = ((observed & STATE_MASK) >> STATE_SHIFT) as u8;
                Err(DispatcherError::TransitionFailed {
                    expected: DISPATCHER_STATE_DETECTING,
                    observed: observed_state,
                })
            }
        }
    }

    /// Selects the best available backend for the current platform.
    ///
    /// # Performance
    ///
    /// <50ns (atomic loads + CAS)
    ///
    /// # Platform Preferences
    ///
    /// - macOS/iOS: Metal
    /// - Windows: DX12 > Vulkan
    /// - Linux/Android: Vulkan
    /// - Web: WebGPU
    pub fn select_best(&self) -> DispatcherResult<BackendType> {
        let state = self.state();
        if state != DISPATCHER_STATE_READY {
            return Err(DispatcherError::InvalidState {
                current: state,
                expected: DISPATCHER_STATE_READY,
            });
        }

        let available = self.available_backends();
        let best = Self::preferred_for_platform(available);

        match best {
            Some(backend) => self.select(backend),
            None => Err(DispatcherError::NoBackendsAvailable),
        }
    }

    /// Selects a specific backend.
    ///
    /// # Performance
    ///
    /// <30ns (CAS)
    pub fn select(&self, backend: BackendType) -> DispatcherResult<BackendType> {
        // Check if already active
        if let Some(current) = self.active_backend() {
            return Err(DispatcherError::BackendAlreadySelected { current });
        }

        let state = self.state();
        if state != DISPATCHER_STATE_READY {
            return Err(DispatcherError::InvalidState {
                current: state,
                expected: DISPATCHER_STATE_READY,
            });
        }

        // Check if backend is available
        if !self.is_available(backend) {
            return Err(DispatcherError::BackendNotAvailable { backend });
        }

        // Transition to Active
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let current_state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

            if current_state != DISPATCHER_STATE_READY {
                // Another thread selected a backend
                if current_state == DISPATCHER_STATE_ACTIVE {
                    let active_id = ((current & ACTIVE_BACKEND_MASK) >> ACTIVE_BACKEND_SHIFT) as u8;
                    if let Some(active) = Self::backend_from_id(active_id) {
                        return Err(DispatcherError::BackendAlreadySelected { current: active });
                    }
                }
                return Err(DispatcherError::InvalidState {
                    current: current_state,
                    expected: DISPATCHER_STATE_READY,
                });
            }

            let backend_count = ((current & BACKEND_COUNT_MASK) >> BACKEND_COUNT_SHIFT) as u8;
            let current_gen = current & GENERATION_MASK;
            let new_gen = current_gen.wrapping_add(1) & GENERATION_MASK;

            let backend_id = Self::backend_to_id(backend);
            let active_primary = ((DISPATCHER_STATE_ACTIVE as u64) << STATE_SHIFT)
                | ((backend_id as u64) << ACTIVE_BACKEND_SHIFT)
                | ((backend_count as u64) << BACKEND_COUNT_SHIFT)
                | new_gen;

            if self
                .primary
                .compare_exchange(current, active_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(backend);
            }
            // Retry on CAS failure
        }
    }

    /// Checks if a specific backend is available.
    #[inline]
    pub fn is_available(&self, backend: BackendType) -> bool {
        let available = self.available_backends();
        let flag = Self::backend_to_flag(backend);
        (available & flag) != 0
    }

    /// Returns the preferred backend for the current platform.
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_PLATFORM_DETECTION_CONST`: Platform detection is compile-time
    pub fn preferred_for_platform(available: u32) -> Option<BackendType> {
        // macOS/iOS prefer Metal
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            if (available & BACKEND_FLAG_METAL) != 0 {
                return Some(BackendType::Metal);
            }
        }

        // Windows prefers DX12, then Vulkan
        #[cfg(target_os = "windows")]
        {
            if (available & BACKEND_FLAG_DX12) != 0 {
                return Some(BackendType::Dx12);
            }
            if (available & BACKEND_FLAG_VULKAN) != 0 {
                return Some(BackendType::Vulkan);
            }
        }

        // Linux/Android prefer Vulkan
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            if (available & BACKEND_FLAG_VULKAN) != 0 {
                return Some(BackendType::Vulkan);
            }
        }

        // Web prefers WebGPU
        #[cfg(target_arch = "wasm32")]
        {
            if (available & BACKEND_FLAG_WEBGPU) != 0 {
                return Some(BackendType::WebGpu);
            }
        }

        // Fallback: try any available backend
        if (available & BACKEND_FLAG_VULKAN) != 0 {
            return Some(BackendType::Vulkan);
        }
        if (available & BACKEND_FLAG_METAL) != 0 {
            return Some(BackendType::Metal);
        }
        if (available & BACKEND_FLAG_DX12) != 0 {
            return Some(BackendType::Dx12);
        }
        if (available & BACKEND_FLAG_WEBGPU) != 0 {
            return Some(BackendType::WebGpu);
        }
        if (available & BACKEND_FLAG_NULL) != 0 {
            return Some(BackendType::Null);
        }

        None
    }

    /// Gets the backend handle (mock value).
    pub fn get_backend_handle(&self, backend: BackendType) -> Option<u64> {
        if !self.is_available(backend) {
            return None;
        }

        let handle = match backend {
            BackendType::Vulkan => self.vulkan_handle.load(Ordering::Acquire),
            BackendType::Metal => self.metal_handle.load(Ordering::Acquire),
            BackendType::Dx12 => self.dx12_handle.load(Ordering::Acquire),
            BackendType::WebGpu => self.webgpu_handle.load(Ordering::Acquire),
            BackendType::Null => 0x4E554C4C_0000_0001, // "NULL" in hex
        };

        if handle != 0 { Some(handle) } else { None }
    }

    /// Gets the device count for a backend.
    #[inline]
    pub fn device_count(&self, backend: BackendType) -> u32 {
        match backend {
            BackendType::Vulkan => self.vulkan_device_count.load(Ordering::Acquire),
            BackendType::Metal => self.metal_device_count.load(Ordering::Acquire),
            BackendType::Dx12 => self.dx12_device_count.load(Ordering::Acquire),
            BackendType::WebGpu => self.webgpu_device_count.load(Ordering::Acquire),
            BackendType::Null => 1, // Always 1 null device
        }
    }

    /// Sets dispatcher flags.
    pub fn set_flags(&self, flags: u32) {
        loop {
            let current = self.secondary.load(Ordering::Acquire);
            let available = (current & AVAILABLE_BACKENDS_MASK) >> AVAILABLE_BACKENDS_SHIFT;
            let new_secondary = (available << AVAILABLE_BACKENDS_SHIFT) | (flags as u64);

            if self
                .secondary
                .compare_exchange(current, new_secondary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Checks if the dispatcher is ready.
    #[inline]
    pub fn is_ready(&self) -> bool {
        let state = self.state();
        state == DISPATCHER_STATE_READY || state == DISPATCHER_STATE_ACTIVE
    }

    /// Checks if a backend is active.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state() == DISPATCHER_STATE_ACTIVE
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    #[inline]
    fn backend_to_id(backend: BackendType) -> u8 {
        match backend {
            BackendType::Vulkan => 0,
            BackendType::Metal => 1,
            BackendType::Dx12 => 2,
            BackendType::WebGpu => 3,
            BackendType::Null => 255,
        }
    }

    #[inline]
    fn backend_from_id(id: u8) -> Option<BackendType> {
        match id {
            0 => Some(BackendType::Vulkan),
            1 => Some(BackendType::Metal),
            2 => Some(BackendType::Dx12),
            3 => Some(BackendType::WebGpu),
            255 => Some(BackendType::Null),
            _ => None,
        }
    }

    #[inline]
    fn backend_to_flag(backend: BackendType) -> u32 {
        match backend {
            BackendType::Vulkan => BACKEND_FLAG_VULKAN,
            BackendType::Metal => BACKEND_FLAG_METAL,
            BackendType::Dx12 => BACKEND_FLAG_DX12,
            BackendType::WebGpu => BACKEND_FLAG_WEBGPU,
            BackendType::Null => BACKEND_FLAG_NULL,
        }
    }
}

impl Default for KgpuBackendDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for KgpuBackendDispatcher {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let snapshot = self.snapshot();
        f.debug_struct("KgpuBackendDispatcher")
            .field("state", &snapshot.state)
            .field("active_backend", &snapshot.active_backend)
            .field("backend_count", &snapshot.backend_count)
            .field("available_backends", &format_args!("{:#010x}", snapshot.available_backends))
            .field("generation", &snapshot.generation)
            .finish()
    }
}

// SAFETY: All operations are atomic; no mutable aliasing possible
unsafe impl Send for KgpuBackendDispatcher {}
unsafe impl Sync for KgpuBackendDispatcher {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<KgpuBackendDispatcher>(), 256);
        assert_eq!(core::mem::align_of::<KgpuBackendDispatcher>(), 256);
    }

    #[test]
    fn test_initial_state() {
        let dispatcher = KgpuBackendDispatcher::new();
        assert_eq!(dispatcher.state(), DISPATCHER_STATE_UNINITIALIZED);
        assert_eq!(dispatcher.backend_count(), 0);
        assert_eq!(dispatcher.generation(), 0);
        assert!(dispatcher.active_backend().is_none());
        assert!(!dispatcher.is_ready());
        assert!(!dispatcher.is_active());
    }

    #[test]
    fn test_detect_backends() {
        let dispatcher = KgpuBackendDispatcher::new();
        let count = dispatcher.detect_backends().expect("Detection failed");

        // Should have detected at least the null backend
        assert!(count >= 1);
        assert!(dispatcher.is_ready());
        assert_eq!(dispatcher.state(), DISPATCHER_STATE_READY);
        assert!(dispatcher.generation() > 0);

        // Null backend should always be available
        assert!(dispatcher.is_available(BackendType::Null));
    }

    #[test]
    fn test_double_detect_fails() {
        let dispatcher = KgpuBackendDispatcher::new();
        dispatcher.detect_backends().unwrap();

        let result = dispatcher.detect_backends();
        assert!(result.is_err());
    }

    #[test]
    fn test_select_null_backend() {
        let dispatcher = KgpuBackendDispatcher::new();
        dispatcher.detect_backends().unwrap();

        let result = dispatcher.select(BackendType::Null);
        assert!(result.is_ok());
        assert_eq!(dispatcher.state(), DISPATCHER_STATE_ACTIVE);
        assert_eq!(dispatcher.active_backend(), Some(BackendType::Null));
        assert!(dispatcher.is_active());
    }

    #[test]
    fn test_select_best() {
        let dispatcher = KgpuBackendDispatcher::new();
        dispatcher.detect_backends().unwrap();

        let best = dispatcher.select_best().expect("Selection failed");
        assert!(dispatcher.is_active());
        assert_eq!(dispatcher.active_backend(), Some(best));
    }

    #[test]
    fn test_double_select_fails() {
        let dispatcher = KgpuBackendDispatcher::new();
        dispatcher.detect_backends().unwrap();
        dispatcher.select(BackendType::Null).unwrap();

        let result = dispatcher.select(BackendType::Null);
        assert!(matches!(result, Err(DispatcherError::BackendAlreadySelected { .. })));
    }

    #[test]
    fn test_select_unavailable_backend() {
        let dispatcher = KgpuBackendDispatcher::new();
        dispatcher.detect_backends().unwrap();

        // WebGPU is only available on wasm32
        #[cfg(not(target_arch = "wasm32"))]
        {
            let result = dispatcher.select(BackendType::WebGpu);
            assert!(matches!(result, Err(DispatcherError::BackendNotAvailable { .. })));
        }
    }

    #[test]
    fn test_backend_handle() {
        let dispatcher = KgpuBackendDispatcher::new();
        dispatcher.detect_backends().unwrap();

        // Null backend handle
        let handle = dispatcher.get_backend_handle(BackendType::Null);
        assert!(handle.is_some());
    }

    #[test]
    fn test_device_count() {
        let dispatcher = KgpuBackendDispatcher::new();
        dispatcher.detect_backends().unwrap();

        // Null backend always has 1 device
        assert_eq!(dispatcher.device_count(BackendType::Null), 1);
    }

    #[test]
    fn test_set_flags() {
        let dispatcher = KgpuBackendDispatcher::new();
        dispatcher.detect_backends().unwrap();

        let new_flags = FLAG_AUTO_SELECT | FLAG_PREFER_LOW_POWER | FLAG_ENABLE_VALIDATION;
        dispatcher.set_flags(new_flags);

        assert_eq!(dispatcher.flags(), new_flags);
    }

    #[test]
    fn test_snapshot() {
        let dispatcher = KgpuBackendDispatcher::new();
        dispatcher.detect_backends().unwrap();
        dispatcher.select(BackendType::Null).unwrap();

        let snapshot = dispatcher.snapshot();
        assert_eq!(snapshot.state, DISPATCHER_STATE_ACTIVE);
        assert_eq!(snapshot.active_backend, Some(BackendType::Null));
        assert!(snapshot.backend_count >= 1);
        assert!(snapshot.generation > 0);
        assert!((snapshot.available_backends & BACKEND_FLAG_NULL) != 0);
    }

    #[test]
    fn test_generation_increments() {
        let dispatcher = KgpuBackendDispatcher::new();
        let gen0 = dispatcher.generation();

        dispatcher.detect_backends().unwrap();
        let gen1 = dispatcher.generation();
        assert!(gen1 > gen0);

        dispatcher.select(BackendType::Null).unwrap();
        let gen2 = dispatcher.generation();
        assert!(gen2 > gen1);
    }

    #[test]
    fn test_debug_format() {
        let dispatcher = KgpuBackendDispatcher::new();
        dispatcher.detect_backends().unwrap();

        let debug_str = format!("{:?}", dispatcher);
        assert!(debug_str.contains("KgpuBackendDispatcher"));
        assert!(debug_str.contains("state"));
    }

    #[test]
    fn test_backend_to_from_id() {
        assert_eq!(KgpuBackendDispatcher::backend_from_id(0), Some(BackendType::Vulkan));
        assert_eq!(KgpuBackendDispatcher::backend_from_id(1), Some(BackendType::Metal));
        assert_eq!(KgpuBackendDispatcher::backend_from_id(2), Some(BackendType::Dx12));
        assert_eq!(KgpuBackendDispatcher::backend_from_id(3), Some(BackendType::WebGpu));
        assert_eq!(KgpuBackendDispatcher::backend_from_id(255), Some(BackendType::Null));
        assert_eq!(KgpuBackendDispatcher::backend_from_id(100), None);

        assert_eq!(KgpuBackendDispatcher::backend_to_id(BackendType::Vulkan), 0);
        assert_eq!(KgpuBackendDispatcher::backend_to_id(BackendType::Metal), 1);
        assert_eq!(KgpuBackendDispatcher::backend_to_id(BackendType::Dx12), 2);
        assert_eq!(KgpuBackendDispatcher::backend_to_id(BackendType::WebGpu), 3);
        assert_eq!(KgpuBackendDispatcher::backend_to_id(BackendType::Null), 255);
    }

    #[test]
    fn test_preferred_for_platform() {
        // With all backends available
        let all = BACKEND_FLAG_VULKAN | BACKEND_FLAG_METAL | BACKEND_FLAG_DX12 | BACKEND_FLAG_WEBGPU | BACKEND_FLAG_NULL;

        let preferred = KgpuBackendDispatcher::preferred_for_platform(all);
        assert!(preferred.is_some());

        // Platform-specific checks
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        assert_eq!(preferred, Some(BackendType::Metal));

        #[cfg(target_os = "windows")]
        assert_eq!(preferred, Some(BackendType::Dx12));

        #[cfg(any(target_os = "linux", target_os = "android"))]
        assert_eq!(preferred, Some(BackendType::Vulkan));

        #[cfg(target_arch = "wasm32")]
        assert_eq!(preferred, Some(BackendType::WebGpu));
    }

    #[test]
    fn test_preferred_null_only() {
        let null_only = BACKEND_FLAG_NULL;
        let preferred = KgpuBackendDispatcher::preferred_for_platform(null_only);
        assert_eq!(preferred, Some(BackendType::Null));
    }

    #[test]
    fn test_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let dispatcher = Arc::new(KgpuBackendDispatcher::new());
        dispatcher.detect_backends().unwrap();

        let mut handles = vec![];

        // Spawn readers
        for _ in 0..4 {
            let disp = Arc::clone(&dispatcher);
            handles.push(thread::spawn(move || {
                for _ in 0..500 {
                    let _ = disp.snapshot();
                    let _ = disp.state();
                    let _ = disp.available_backends();
                    let _ = disp.is_ready();
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        assert!(dispatcher.is_ready());
    }
}
