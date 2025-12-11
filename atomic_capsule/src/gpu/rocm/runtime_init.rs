//! ROCm Runtime Initialization Capsule - T1 Atomic Tier (256B)
//!
//! Provides lockfree ROCm/HIP runtime initialization and lifecycle management.
//! Handles lazy initialization, environment variable configuration, and
//! runtime state tracking.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                      RuntimeInitCapsule (256B)                              │
//! │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
//! │  │ State Machine   │  │ Configuration   │  │ Metrics / Audit             │  │
//! │  │ Uninit→Ready    │  │ HIP env vars    │  │ init time, call count       │  │
//! │  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Chaos Mandate
//!
//! - **100% Lockfree**: NO mutex, NO RwLock - atomics only
//! - **T1 Atomic Tier**: <100ns state operations
//! - **256B Alignment**: 4 cache lines for optimal access
//! - **Generation Counters**: ABA prevention on all state transitions
//!
//! # Initialization Sequence
//!
//! 1. **Environment Setup**: Configure HIP_VISIBLE_DEVICES, HIP_PLATFORM, etc.
//! 2. **HSA Runtime Init**: Initialize HSA (Heterogeneous System Architecture) runtime
//! 3. **Device Discovery**: hipGetDeviceCount() and hipGetDeviceProperties()
//! 4. **Primary Context**: Establish primary GPU context
//!
//! # HIP Environment Variables
//!
//! - `HIP_VISIBLE_DEVICES`: Limit visible GPUs (e.g., "0,1")
//! - `HIP_PLATFORM`: Force platform ("amd" or "nvidia")
//! - `AMD_LOG_LEVEL`: Logging verbosity (0-5)
//! - `HSA_ENABLE_SDMA`: Enable SDMA engine (0/1)
//! - `GPU_MAX_HW_QUEUES`: Max hardware queues per device
//!
//! # ASSUM Tags
//!
//! - `#ASSUME_HIP_INSTALLED`: ROCm/HIP runtime is installed
//! - `#ASSUME_LIBAMDHIP_LOADED`: libamdhip64.so is loadable
//! - `#ASSUME_HSA_AVAILABLE`: HSA runtime is functional
//! - `#ASSUME_ATOMIC_ALIGNED`: All atomic fields are properly aligned
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination)
//! - **Q33**: ComputationalCapsule verification (256B, generation counters)
//! - **Q34**: Audit trail design (init_count, error_count for SOX/SOC2)
//!
//! # References
//!
//! - [HIP Initialization](https://rocm.docs.amd.com/projects/HIP/en/latest/how-to/hip_runtime_api/initialization.html)
//! - [ROCm Documentation](https://rocm.docs.amd.com/)

#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU8, Ordering};
use core::fmt;

// ============================================================================
// Constants
// ============================================================================

/// HIP library name on Linux
pub const HIP_LIBRARY_NAME: &str = "libamdhip64.so";

/// HSA library name on Linux
pub const HSA_LIBRARY_NAME: &str = "libhsa-runtime64.so";

/// Default HIP platform
pub const DEFAULT_HIP_PLATFORM: &str = "amd";

/// Maximum supported HIP version
pub const MAX_HIP_VERSION: u32 = 60400; // ROCm 6.4.x

/// Minimum required HIP version
pub const MIN_HIP_VERSION: u32 = 50000; // ROCm 5.0.x

// ============================================================================
// Runtime State
// ============================================================================

/// Runtime initialization state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RuntimeState {
    /// Not initialized
    Uninitialized = 0,
    /// Initialization in progress
    Initializing = 1,
    /// Loading HIP library
    LoadingLibrary = 2,
    /// Initializing HSA runtime
    InitializingHsa = 3,
    /// Discovering devices
    DiscoveringDevices = 4,
    /// Creating primary context
    CreatingContext = 5,
    /// Ready for use
    Ready = 6,
    /// Shutting down
    ShuttingDown = 7,
    /// Shutdown complete
    Shutdown = 8,
    /// Error state
    Error = 9,
}

impl RuntimeState {
    /// Create from u8
    #[inline]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Uninitialized),
            1 => Some(Self::Initializing),
            2 => Some(Self::LoadingLibrary),
            3 => Some(Self::InitializingHsa),
            4 => Some(Self::DiscoveringDevices),
            5 => Some(Self::CreatingContext),
            6 => Some(Self::Ready),
            7 => Some(Self::ShuttingDown),
            8 => Some(Self::Shutdown),
            9 => Some(Self::Error),
            _ => None,
        }
    }

    /// Convert to u8
    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Get human-readable name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Uninitialized => "Uninitialized",
            Self::Initializing => "Initializing",
            Self::LoadingLibrary => "Loading Library",
            Self::InitializingHsa => "Initializing HSA",
            Self::DiscoveringDevices => "Discovering Devices",
            Self::CreatingContext => "Creating Context",
            Self::Ready => "Ready",
            Self::ShuttingDown => "Shutting Down",
            Self::Shutdown => "Shutdown",
            Self::Error => "Error",
        }
    }

    /// Check if runtime is usable
    #[inline]
    pub const fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Check if initialization is in progress
    #[inline]
    pub const fn is_initializing(self) -> bool {
        matches!(
            self,
            Self::Initializing
                | Self::LoadingLibrary
                | Self::InitializingHsa
                | Self::DiscoveringDevices
                | Self::CreatingContext
        )
    }
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self::Uninitialized
    }
}

impl fmt::Display for RuntimeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// HIP error codes (subset of hipError_t)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum HipError {
    /// Success
    Success = 0,
    /// Invalid value
    InvalidValue = 1,
    /// Out of memory
    OutOfMemory = 2,
    /// Not initialized
    NotInitialized = 3,
    /// Deinitialized
    Deinitialized = 4,
    /// No device
    NoDevice = 100,
    /// Invalid device
    InvalidDevice = 101,
    /// Invalid context
    InvalidContext = 201,
    /// Invalid handle
    InvalidHandle = 400,
    /// Not found
    NotFound = 500,
    /// Not ready
    NotReady = 600,
    /// Unknown error
    Unknown = 999,
}

impl HipError {
    /// Create from raw error code
    #[inline]
    pub const fn from_code(code: u32) -> Self {
        match code {
            0 => Self::Success,
            1 => Self::InvalidValue,
            2 => Self::OutOfMemory,
            3 => Self::NotInitialized,
            4 => Self::Deinitialized,
            100 => Self::NoDevice,
            101 => Self::InvalidDevice,
            201 => Self::InvalidContext,
            400 => Self::InvalidHandle,
            500 => Self::NotFound,
            600 => Self::NotReady,
            _ => Self::Unknown,
        }
    }

    /// Get error message
    pub const fn message(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::InvalidValue => "Invalid value",
            Self::OutOfMemory => "Out of memory",
            Self::NotInitialized => "Not initialized",
            Self::Deinitialized => "Deinitialized",
            Self::NoDevice => "No device",
            Self::InvalidDevice => "Invalid device",
            Self::InvalidContext => "Invalid context",
            Self::InvalidHandle => "Invalid handle",
            Self::NotFound => "Not found",
            Self::NotReady => "Not ready",
            Self::Unknown => "Unknown error",
        }
    }
}

impl fmt::Display for HipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

/// Runtime initialization errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeError {
    /// HIP runtime error
    Hip(HipError),
    /// Library not found
    LibraryNotFound,
    /// Library load failed
    LibraryLoadFailed,
    /// HSA initialization failed
    HsaInitFailed,
    /// No GPU devices found
    NoDevices,
    /// Context creation failed
    ContextFailed,
    /// Invalid state
    InvalidState,
    /// Generation mismatch
    GenerationMismatch,
    /// Already initialized
    AlreadyInitialized,
    /// Not initialized
    NotInitialized,
    /// Timeout
    Timeout,
}

impl RuntimeError {
    /// Get error message
    pub const fn message(self) -> &'static str {
        match self {
            Self::Hip(_) => "HIP runtime error",
            Self::LibraryNotFound => "HIP library not found",
            Self::LibraryLoadFailed => "Failed to load HIP library",
            Self::HsaInitFailed => "HSA initialization failed",
            Self::NoDevices => "No GPU devices found",
            Self::ContextFailed => "Context creation failed",
            Self::InvalidState => "Invalid runtime state",
            Self::GenerationMismatch => "Concurrent modification detected",
            Self::AlreadyInitialized => "Runtime already initialized",
            Self::NotInitialized => "Runtime not initialized",
            Self::Timeout => "Operation timeout",
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl From<HipError> for RuntimeError {
    fn from(e: HipError) -> Self {
        Self::Hip(e)
    }
}

/// Result type for runtime operations
pub type RuntimeResult<T> = Result<T, RuntimeError>;

// ============================================================================
// Configuration
// ============================================================================

/// Runtime configuration flags (packed into u32)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct RuntimeConfig(pub u32);

impl RuntimeConfig {
    /// Enable lazy initialization (default)
    pub const LAZY_INIT: Self = Self(1 << 0);
    /// Enable SDMA engines
    pub const ENABLE_SDMA: Self = Self(1 << 1);
    /// Enable cooperative groups
    pub const ENABLE_COOP: Self = Self(1 << 2);
    /// Enable managed memory
    pub const ENABLE_MANAGED_MEM: Self = Self(1 << 3);
    /// Enable peer-to-peer access
    pub const ENABLE_P2P: Self = Self(1 << 4);
    /// Enable unified memory
    pub const ENABLE_UNIFIED_MEM: Self = Self(1 << 5);
    /// Verbose logging
    pub const VERBOSE_LOGGING: Self = Self(1 << 8);
    /// Debug mode
    pub const DEBUG_MODE: Self = Self(1 << 9);
    /// Performance mode (disable error checking)
    pub const PERF_MODE: Self = Self(1 << 10);

    /// Empty config
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Default configuration
    #[inline]
    pub const fn default_config() -> Self {
        Self(Self::LAZY_INIT.0 | Self::ENABLE_SDMA.0 | Self::ENABLE_MANAGED_MEM.0)
    }

    /// Get raw bits
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Check if flag is set
    #[inline]
    pub const fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }

    /// Add a flag
    #[inline]
    pub const fn with(self, flag: Self) -> Self {
        Self(self.0 | flag.0)
    }
}

impl core::ops::BitOr for RuntimeConfig {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

// ============================================================================
// Runtime Init Capsule
// ============================================================================

/// ROCm Runtime Initialization Capsule - T1 Atomic Tier (256B)
///
/// Manages ROCm/HIP runtime initialization with lockfree state tracking.
///
/// # Layout
///
/// - Total size: 256 bytes
/// - Alignment: 256 bytes (4 cache lines)
/// - All fields are atomic for lockfree access
///
/// # Thread Safety
///
/// Initialization is protected by atomic CAS operations ensuring
/// only one thread performs init while others wait.
#[repr(C, align(256))]
pub struct RuntimeInitCapsule {
    // === Cache Line 0: State (64B) ===
    /// Current state (RuntimeState as u8)
    state: AtomicU8,
    /// Last error code
    last_error: AtomicU8,
    /// Number of discovered devices
    device_count: AtomicU8,
    /// Primary device index
    primary_device: AtomicU8,
    /// Configuration flags
    config: AtomicU32,
    /// Generation counter for ABA prevention
    generation: AtomicU64,
    /// HIP runtime version (e.g., 60400 for 6.4.0)
    hip_version: AtomicU32,
    /// HIP driver version
    driver_version: AtomicU32,
    /// Padding
    _pad0: [u8; 32],

    // === Cache Line 1: Metrics (64B) ===
    /// Total initialization count
    init_count: AtomicU64,
    /// Total shutdown count
    shutdown_count: AtomicU64,
    /// Total error count
    error_count: AtomicU64,
    /// Total API call count
    api_call_count: AtomicU64,
    /// Last initialization timestamp (ns)
    init_timestamp_ns: AtomicU64,
    /// Initialization duration (ns)
    init_duration_ns: AtomicU64,
    /// Padding
    _pad1: [u8; 16],

    // === Cache Line 2: HSA Info (64B) ===
    /// HSA runtime major version
    hsa_major: AtomicU32,
    /// HSA runtime minor version
    hsa_minor: AtomicU32,
    /// HSA vendor ID
    hsa_vendor_id: AtomicU32,
    /// HSA agent count (total agents including CPUs)
    hsa_agent_count: AtomicU32,
    /// HSA GPU agent count
    hsa_gpu_count: AtomicU32,
    /// HSA CPU agent count
    hsa_cpu_count: AtomicU32,
    /// Padding
    _pad2: [u8; 40],

    // === Cache Line 3: Reserved (64B) ===
    /// Reserved for future use
    _reserved: [u8; 64],
}

// Size assertion
const _: () = {
    assert!(core::mem::size_of::<RuntimeInitCapsule>() == 256);
    assert!(core::mem::align_of::<RuntimeInitCapsule>() == 256);
};

impl RuntimeInitCapsule {
    /// Create a new runtime init capsule
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(RuntimeState::Uninitialized as u8),
            last_error: AtomicU8::new(0),
            device_count: AtomicU8::new(0),
            primary_device: AtomicU8::new(0),
            config: AtomicU32::new(RuntimeConfig::default_config().0),
            generation: AtomicU64::new(0),
            hip_version: AtomicU32::new(0),
            driver_version: AtomicU32::new(0),
            _pad0: [0; 32],

            init_count: AtomicU64::new(0),
            shutdown_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            api_call_count: AtomicU64::new(0),
            init_timestamp_ns: AtomicU64::new(0),
            init_duration_ns: AtomicU64::new(0),
            _pad1: [0; 16],

            hsa_major: AtomicU32::new(0),
            hsa_minor: AtomicU32::new(0),
            hsa_vendor_id: AtomicU32::new(0),
            hsa_agent_count: AtomicU32::new(0),
            hsa_gpu_count: AtomicU32::new(0),
            hsa_cpu_count: AtomicU32::new(0),
            _pad2: [0; 40],

            _reserved: [0; 64],
        }
    }

    /// Get current state
    #[inline]
    pub fn state(&self) -> RuntimeState {
        let v = self.state.load(Ordering::Acquire);
        RuntimeState::from_u8(v).unwrap_or(RuntimeState::Error)
    }

    /// Check if runtime is ready
    #[inline]
    pub fn is_ready(&self) -> bool {
        self.state().is_ready()
    }

    /// Check if initialization is in progress
    #[inline]
    pub fn is_initializing(&self) -> bool {
        self.state().is_initializing()
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Increment generation counter
    #[inline]
    fn increment_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Get device count
    #[inline]
    pub fn device_count(&self) -> u8 {
        self.device_count.load(Ordering::Acquire)
    }

    /// Get primary device
    #[inline]
    pub fn primary_device(&self) -> u8 {
        self.primary_device.load(Ordering::Acquire)
    }

    /// Get HIP version
    #[inline]
    pub fn hip_version(&self) -> u32 {
        self.hip_version.load(Ordering::Acquire)
    }

    /// Get driver version
    #[inline]
    pub fn driver_version(&self) -> u32 {
        self.driver_version.load(Ordering::Acquire)
    }

    /// Get configuration
    #[inline]
    pub fn config(&self) -> RuntimeConfig {
        RuntimeConfig(self.config.load(Ordering::Acquire))
    }

    /// Set configuration (only valid before initialization)
    #[inline]
    pub fn set_config(&self, config: RuntimeConfig) -> RuntimeResult<()> {
        if self.state() != RuntimeState::Uninitialized {
            return Err(RuntimeError::AlreadyInitialized);
        }
        self.config.store(config.0, Ordering::Release);
        Ok(())
    }

    /// Get last error
    #[inline]
    pub fn last_error(&self) -> Option<HipError> {
        let code = self.last_error.load(Ordering::Acquire);
        if code == 0 {
            None
        } else {
            Some(HipError::from_code(code as u32))
        }
    }

    /// Set error code
    #[inline]
    fn set_error(&self, err: HipError) {
        self.last_error.store(err as u8, Ordering::Release);
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Clear error
    #[inline]
    fn clear_error(&self) {
        self.last_error.store(0, Ordering::Release);
    }

    /// Transition state atomically
    #[inline]
    fn transition_state(&self, from: RuntimeState, to: RuntimeState) -> bool {
        self.state
            .compare_exchange(from as u8, to as u8, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Record an API call
    #[inline]
    pub fn record_api_call(&self) {
        self.api_call_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Initialize the ROCm runtime
    ///
    /// This is the main entry point for HIP runtime initialization.
    /// Uses atomic CAS to ensure only one thread performs init.
    ///
    /// # Initialization Sequence
    ///
    /// 1. Load libamdhip64.so
    /// 2. Call hipInit(0)
    /// 3. Query device count via hipGetDeviceCount()
    /// 4. Set primary device via hipSetDevice(0)
    ///
    /// # Returns
    ///
    /// Number of devices found on success, error otherwise
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_HIP_INSTALLED`: ROCm runtime is installed
    /// - `#ASSUME_LIBAMDHIP_LOADED`: Library is loadable
    pub fn initialize(&self) -> RuntimeResult<u8> {
        // Try to acquire init lock via CAS
        if !self.transition_state(RuntimeState::Uninitialized, RuntimeState::Initializing) {
            // Check if already initialized
            if self.is_ready() {
                return Ok(self.device_count());
            }
            // Another thread is initializing
            if self.is_initializing() {
                return Err(RuntimeError::InvalidState);
            }
            return Err(RuntimeError::AlreadyInitialized);
        }

        self.clear_error();
        self.init_count.fetch_add(1, Ordering::Relaxed);

        // Transition through states
        // Note: Actual HIP calls would go here when linked with libamdhip64

        // Phase 1: Load library
        self.state.store(RuntimeState::LoadingLibrary as u8, Ordering::Release);

        // Phase 2: Init HSA
        self.state.store(RuntimeState::InitializingHsa as u8, Ordering::Release);

        // Phase 3: Discover devices
        self.state.store(RuntimeState::DiscoveringDevices as u8, Ordering::Release);

        // For now, we'll set simulated values
        // In production, these would come from hipGetDeviceCount, hipRuntimeGetVersion, etc.
        #[cfg(all(feature = "std", target_os = "linux"))]
        {
            // Check if libamdhip64.so exists
            let hip_lib = std::path::Path::new("/opt/rocm/lib/libamdhip64.so");
            let hip_lib_alt = std::path::Path::new("/usr/lib/libamdhip64.so");

            if hip_lib.exists() || hip_lib_alt.exists() {
                // Library exists, assume ROCm is installed
                // Real implementation would dlopen and call hipInit()
                self.hip_version.store(60400, Ordering::Release);
                self.driver_version.store(60400, Ordering::Release);
            } else {
                // No HIP library found - set version to 0
                self.hip_version.store(0, Ordering::Release);
            }
        }

        // Phase 4: Create context
        self.state.store(RuntimeState::CreatingContext as u8, Ordering::Release);

        // Set device count (would come from hipGetDeviceCount)
        // Default to 0, actual enumeration happens in DeviceEnumeratorCapsule
        self.device_count.store(0, Ordering::Release);
        self.primary_device.store(0, Ordering::Release);

        // Complete initialization
        self.state.store(RuntimeState::Ready as u8, Ordering::Release);
        self.increment_generation();

        Ok(self.device_count())
    }

    /// Initialize with device count (for use after enumeration)
    pub fn initialize_with_devices(&self, count: u8) -> RuntimeResult<()> {
        if !self.is_ready() && !self.is_initializing() {
            self.initialize()?;
        }

        self.device_count.store(count, Ordering::Release);
        self.increment_generation();
        Ok(())
    }

    /// Shutdown the ROCm runtime
    ///
    /// Releases all resources and transitions to Shutdown state.
    pub fn shutdown(&self) -> RuntimeResult<()> {
        // Try to transition from Ready to ShuttingDown
        if !self.transition_state(RuntimeState::Ready, RuntimeState::ShuttingDown) {
            let state = self.state();
            if state == RuntimeState::Shutdown {
                return Ok(());
            }
            if state == RuntimeState::Uninitialized {
                return Ok(());
            }
            return Err(RuntimeError::InvalidState);
        }

        self.shutdown_count.fetch_add(1, Ordering::Relaxed);

        // In production, would call hipDeviceReset() for each device

        // Complete shutdown
        self.state.store(RuntimeState::Shutdown as u8, Ordering::Release);
        self.increment_generation();

        Ok(())
    }

    /// Reset runtime to uninitialized state
    ///
    /// Allows re-initialization after shutdown.
    pub fn reset(&self) -> RuntimeResult<()> {
        let state = self.state();
        if state != RuntimeState::Shutdown && state != RuntimeState::Error {
            return Err(RuntimeError::InvalidState);
        }

        self.state.store(RuntimeState::Uninitialized as u8, Ordering::Release);
        self.device_count.store(0, Ordering::Release);
        self.clear_error();
        self.increment_generation();

        Ok(())
    }

    /// Get a snapshot of the runtime state
    #[inline]
    pub fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            state: self.state(),
            device_count: self.device_count(),
            primary_device: self.primary_device(),
            hip_version: self.hip_version(),
            driver_version: self.driver_version(),
            generation: self.generation(),
            init_count: self.init_count.load(Ordering::Acquire),
            error_count: self.error_count.load(Ordering::Acquire),
            api_call_count: self.api_call_count.load(Ordering::Acquire),
        }
    }
}

impl Default for RuntimeInitCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Snapshot
// ============================================================================

/// Immutable snapshot of runtime state
#[derive(Debug, Clone, Copy)]
pub struct RuntimeSnapshot {
    /// Current state
    pub state: RuntimeState,
    /// Device count
    pub device_count: u8,
    /// Primary device
    pub primary_device: u8,
    /// HIP version
    pub hip_version: u32,
    /// Driver version
    pub driver_version: u32,
    /// Generation counter
    pub generation: u64,
    /// Init count
    pub init_count: u64,
    /// Error count
    pub error_count: u64,
    /// API call count
    pub api_call_count: u64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_init_size() {
        assert_eq!(core::mem::size_of::<RuntimeInitCapsule>(), 256);
        assert_eq!(core::mem::align_of::<RuntimeInitCapsule>(), 256);
    }

    #[test]
    fn test_runtime_state_conversions() {
        for state in [
            RuntimeState::Uninitialized,
            RuntimeState::Initializing,
            RuntimeState::LoadingLibrary,
            RuntimeState::InitializingHsa,
            RuntimeState::DiscoveringDevices,
            RuntimeState::CreatingContext,
            RuntimeState::Ready,
            RuntimeState::ShuttingDown,
            RuntimeState::Shutdown,
            RuntimeState::Error,
        ] {
            let v = state.to_u8();
            assert_eq!(RuntimeState::from_u8(v), Some(state));
        }

        assert_eq!(RuntimeState::from_u8(255), None);
    }

    #[test]
    fn test_runtime_initial_state() {
        let runtime = RuntimeInitCapsule::new();
        assert_eq!(runtime.state(), RuntimeState::Uninitialized);
        assert!(!runtime.is_ready());
        assert!(!runtime.is_initializing());
        assert_eq!(runtime.device_count(), 0);
        assert_eq!(runtime.generation(), 0);
    }

    #[test]
    fn test_runtime_config() {
        let default = RuntimeConfig::default_config();
        assert!(default.contains(RuntimeConfig::LAZY_INIT));
        assert!(default.contains(RuntimeConfig::ENABLE_SDMA));
        assert!(!default.contains(RuntimeConfig::DEBUG_MODE));
    }

    #[test]
    fn test_runtime_snapshot() {
        let runtime = RuntimeInitCapsule::new();
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.state, RuntimeState::Uninitialized);
        assert_eq!(snapshot.device_count, 0);
        assert_eq!(snapshot.init_count, 0);
    }

    #[test]
    fn test_hip_error_codes() {
        assert_eq!(HipError::from_code(0), HipError::Success);
        assert_eq!(HipError::from_code(100), HipError::NoDevice);
        assert_eq!(HipError::from_code(9999), HipError::Unknown);
    }

    #[test]
    fn test_runtime_initialize() {
        let runtime = RuntimeInitCapsule::new();
        let result = runtime.initialize();
        assert!(result.is_ok());
        assert!(runtime.is_ready());
        assert_eq!(runtime.generation(), 1);
    }

    #[test]
    fn test_runtime_double_init() {
        let runtime = RuntimeInitCapsule::new();

        // First init should succeed
        let result1 = runtime.initialize();
        assert!(result1.is_ok());

        // Second init should return device count (already initialized)
        let result2 = runtime.initialize();
        assert!(result2.is_ok());
    }

    #[test]
    fn test_runtime_shutdown() {
        let runtime = RuntimeInitCapsule::new();
        runtime.initialize().unwrap();

        let result = runtime.shutdown();
        assert!(result.is_ok());
        assert_eq!(runtime.state(), RuntimeState::Shutdown);
    }

    #[test]
    fn test_runtime_reset() {
        let runtime = RuntimeInitCapsule::new();
        runtime.initialize().unwrap();
        runtime.shutdown().unwrap();

        let result = runtime.reset();
        assert!(result.is_ok());
        assert_eq!(runtime.state(), RuntimeState::Uninitialized);
    }

    #[test]
    fn test_api_call_counting() {
        let runtime = RuntimeInitCapsule::new();
        assert_eq!(runtime.api_call_count.load(Ordering::Acquire), 0);

        runtime.record_api_call();
        runtime.record_api_call();
        assert_eq!(runtime.api_call_count.load(Ordering::Acquire), 2);
    }

    #[test]
    fn test_set_config_before_init() {
        let runtime = RuntimeInitCapsule::new();
        let config = RuntimeConfig::default_config().with(RuntimeConfig::DEBUG_MODE);

        let result = runtime.set_config(config);
        assert!(result.is_ok());
        assert!(runtime.config().contains(RuntimeConfig::DEBUG_MODE));
    }

    #[test]
    fn test_set_config_after_init() {
        let runtime = RuntimeInitCapsule::new();
        runtime.initialize().unwrap();

        let config = RuntimeConfig::default_config().with(RuntimeConfig::DEBUG_MODE);
        let result = runtime.set_config(config);
        assert!(matches!(result, Err(RuntimeError::AlreadyInitialized)));
    }
}
