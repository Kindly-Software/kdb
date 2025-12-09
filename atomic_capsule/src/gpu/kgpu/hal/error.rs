//! KGPU HAL Error Types
//!
//! Error types for the Hardware Abstraction Layer (HAL).
//!
//! # Design Philosophy
//!
//! Errors are designed to be:
//! - **Lockfree**: No allocations in error paths (using static strings where possible)
//! - **Type-safe**: Distinct error types for different subsystems
//! - **Informative**: Rich context for debugging without runtime overhead
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_ERROR_NO_ALLOCATION`: Error construction does not allocate
//! - `#ASSUME_ERROR_SEND_SYNC`: All error types are Send + Sync for cross-thread propagation

use core::fmt;

// ============================================================================
// HalError - Main HAL Error Type
// ============================================================================

/// Hardware Abstraction Layer error type.
///
/// Covers all possible errors from GPU backend operations.
///
/// # Design
///
/// Uses an enum with static string messages where possible to avoid
/// allocation in error paths. For dynamic messages, uses a fixed-size
/// buffer (no heap allocation).
///
/// # ASSUM Safety
///
/// - `#ASSUME_ERROR_NO_ALLOCATION`: Most variants use static strings
/// - `#ASSUME_ERROR_THREAD_SAFE`: All variants are Send + Sync
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HalError {
    // ========================================================================
    // Memory Errors
    // ========================================================================

    /// Out of device memory (VRAM exhausted).
    OutOfDeviceMemory,

    /// Out of host memory (system RAM for GPU operations).
    OutOfHostMemory,

    /// Memory mapping failed.
    MapFailed,

    /// Invalid memory access (e.g., out of bounds).
    InvalidMemoryAccess,

    /// Buffer too small for operation.
    BufferTooSmall {
        /// Required size in bytes.
        required: u64,
        /// Available size in bytes.
        available: u64,
    },

    // ========================================================================
    // Device Errors
    // ========================================================================

    /// Device lost (GPU reset, driver crash, etc.).
    DeviceLost,

    /// Device not found (no suitable GPU available).
    DeviceNotFound,

    /// Device initialization failed.
    InitializationFailed,

    /// Device busy (operation in progress).
    DeviceBusy,

    /// Feature not supported by device.
    UnsupportedFeature(&'static str),

    /// Device limit exceeded.
    LimitExceeded {
        /// Name of the limit.
        limit: &'static str,
        /// Requested value.
        requested: u64,
        /// Maximum allowed value.
        maximum: u64,
    },

    // ========================================================================
    // Resource Errors
    // ========================================================================

    /// Invalid handle (stale generation, wrong type, etc.).
    InvalidHandle,

    /// Resource already in use.
    ResourceInUse,

    /// Resource not ready (not yet created, destroyed, etc.).
    ResourceNotReady,

    /// Resource creation failed.
    CreationFailed(&'static str),

    /// Incompatible resource format.
    IncompatibleFormat,

    /// Invalid resource state for operation.
    InvalidState(&'static str),

    // ========================================================================
    // Validation Errors
    // ========================================================================

    /// Validation error with description.
    ValidationError(&'static str),

    /// Invalid parameter value.
    InvalidParameter(&'static str),

    /// Missing required parameter.
    MissingParameter(&'static str),

    /// Shader compilation/validation failed.
    ShaderError(&'static str),

    /// Pipeline layout mismatch.
    LayoutMismatch,

    // ========================================================================
    // Backend Errors
    // ========================================================================

    /// Backend-specific error with error code.
    BackendError {
        /// Backend identifier (vulkan, metal, dx12).
        backend: &'static str,
        /// Backend-specific error code.
        code: i32,
    },

    /// Backend not available on this platform.
    BackendNotAvailable(&'static str),

    /// Surface not supported.
    SurfaceNotSupported,

    // ========================================================================
    // Command Errors
    // ========================================================================

    /// Command buffer overflow.
    CommandBufferOverflow,

    /// Invalid command sequence.
    InvalidCommandSequence(&'static str),

    /// Render pass not active.
    RenderPassNotActive,

    /// Compute pass not active.
    ComputePassNotActive,

    // ========================================================================
    // Synchronization Errors
    // ========================================================================

    /// Timeout waiting for fence/semaphore.
    Timeout,

    /// Deadlock detected.
    Deadlock,

    /// Invalid fence state.
    InvalidFenceState,

    // ========================================================================
    // Other Errors
    // ========================================================================

    /// Unknown/unexpected error.
    Unknown,

    /// Internal error (should not happen in correct code).
    Internal(&'static str),
}

impl HalError {
    /// Returns true if this is a recoverable error.
    ///
    /// Recoverable errors can potentially be retried or worked around.
    /// Non-recoverable errors typically require device recreation or
    /// application restart.
    #[inline]
    pub const fn is_recoverable(&self) -> bool {
        matches!(
            self,
            HalError::OutOfDeviceMemory
                | HalError::OutOfHostMemory
                | HalError::DeviceBusy
                | HalError::Timeout
                | HalError::ResourceInUse
        )
    }

    /// Returns true if this error indicates the device is lost.
    ///
    /// Device lost errors require full device recreation.
    #[inline]
    pub const fn is_device_lost(&self) -> bool {
        matches!(self, HalError::DeviceLost)
    }

    /// Returns an error code for logging/metrics.
    ///
    /// Codes are stable across versions for telemetry.
    #[inline]
    pub const fn error_code(&self) -> u32 {
        match self {
            // Memory: 1xxx
            HalError::OutOfDeviceMemory => 1001,
            HalError::OutOfHostMemory => 1002,
            HalError::MapFailed => 1003,
            HalError::InvalidMemoryAccess => 1004,
            HalError::BufferTooSmall { .. } => 1005,

            // Device: 2xxx
            HalError::DeviceLost => 2001,
            HalError::DeviceNotFound => 2002,
            HalError::InitializationFailed => 2003,
            HalError::DeviceBusy => 2004,
            HalError::UnsupportedFeature(_) => 2005,
            HalError::LimitExceeded { .. } => 2006,

            // Resource: 3xxx
            HalError::InvalidHandle => 3001,
            HalError::ResourceInUse => 3002,
            HalError::ResourceNotReady => 3003,
            HalError::CreationFailed(_) => 3004,
            HalError::IncompatibleFormat => 3005,
            HalError::InvalidState(_) => 3006,

            // Validation: 4xxx
            HalError::ValidationError(_) => 4001,
            HalError::InvalidParameter(_) => 4002,
            HalError::MissingParameter(_) => 4003,
            HalError::ShaderError(_) => 4004,
            HalError::LayoutMismatch => 4005,

            // Backend: 5xxx
            HalError::BackendError { .. } => 5001,
            HalError::BackendNotAvailable(_) => 5002,
            HalError::SurfaceNotSupported => 5003,

            // Command: 6xxx
            HalError::CommandBufferOverflow => 6001,
            HalError::InvalidCommandSequence(_) => 6002,
            HalError::RenderPassNotActive => 6003,
            HalError::ComputePassNotActive => 6004,

            // Synchronization: 7xxx
            HalError::Timeout => 7001,
            HalError::Deadlock => 7002,
            HalError::InvalidFenceState => 7003,

            // Other: 9xxx
            HalError::Unknown => 9001,
            HalError::Internal(_) => 9002,
        }
    }
}

impl fmt::Display for HalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Memory errors
            HalError::OutOfDeviceMemory => write!(f, "out of device memory (VRAM)"),
            HalError::OutOfHostMemory => write!(f, "out of host memory"),
            HalError::MapFailed => write!(f, "memory mapping failed"),
            HalError::InvalidMemoryAccess => write!(f, "invalid memory access"),
            HalError::BufferTooSmall { required, available } => {
                write!(f, "buffer too small: required {} bytes, available {}", required, available)
            }

            // Device errors
            HalError::DeviceLost => write!(f, "GPU device lost"),
            HalError::DeviceNotFound => write!(f, "no suitable GPU device found"),
            HalError::InitializationFailed => write!(f, "device initialization failed"),
            HalError::DeviceBusy => write!(f, "device busy"),
            HalError::UnsupportedFeature(feat) => write!(f, "unsupported feature: {}", feat),
            HalError::LimitExceeded { limit, requested, maximum } => {
                write!(f, "limit exceeded: {} (requested {}, max {})", limit, requested, maximum)
            }

            // Resource errors
            HalError::InvalidHandle => write!(f, "invalid resource handle"),
            HalError::ResourceInUse => write!(f, "resource is in use"),
            HalError::ResourceNotReady => write!(f, "resource not ready"),
            HalError::CreationFailed(what) => write!(f, "failed to create {}", what),
            HalError::IncompatibleFormat => write!(f, "incompatible format"),
            HalError::InvalidState(state) => write!(f, "invalid state: {}", state),

            // Validation errors
            HalError::ValidationError(msg) => write!(f, "validation error: {}", msg),
            HalError::InvalidParameter(param) => write!(f, "invalid parameter: {}", param),
            HalError::MissingParameter(param) => write!(f, "missing parameter: {}", param),
            HalError::ShaderError(msg) => write!(f, "shader error: {}", msg),
            HalError::LayoutMismatch => write!(f, "pipeline layout mismatch"),

            // Backend errors
            HalError::BackendError { backend, code } => {
                write!(f, "{} backend error: code {}", backend, code)
            }
            HalError::BackendNotAvailable(backend) => write!(f, "{} backend not available", backend),
            HalError::SurfaceNotSupported => write!(f, "surface not supported"),

            // Command errors
            HalError::CommandBufferOverflow => write!(f, "command buffer overflow"),
            HalError::InvalidCommandSequence(seq) => write!(f, "invalid command sequence: {}", seq),
            HalError::RenderPassNotActive => write!(f, "render pass not active"),
            HalError::ComputePassNotActive => write!(f, "compute pass not active"),

            // Synchronization errors
            HalError::Timeout => write!(f, "operation timed out"),
            HalError::Deadlock => write!(f, "deadlock detected"),
            HalError::InvalidFenceState => write!(f, "invalid fence state"),

            // Other errors
            HalError::Unknown => write!(f, "unknown error"),
            HalError::Internal(msg) => write!(f, "internal error: {}", msg),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for HalError {}

// ============================================================================
// MapError - Memory Mapping Specific Errors
// ============================================================================

/// Error type for buffer mapping operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    /// Buffer already mapped.
    AlreadyMapped,

    /// Mapping not supported for this buffer.
    NotMappable,

    /// Invalid map range.
    InvalidRange,

    /// Map mode not compatible with buffer usage.
    IncompatibleMode,

    /// Device lost during mapping.
    DeviceLost,

    /// Unknown mapping error.
    Unknown,
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapError::AlreadyMapped => write!(f, "buffer already mapped"),
            MapError::NotMappable => write!(f, "buffer is not mappable"),
            MapError::InvalidRange => write!(f, "invalid map range"),
            MapError::IncompatibleMode => write!(f, "incompatible map mode"),
            MapError::DeviceLost => write!(f, "device lost"),
            MapError::Unknown => write!(f, "unknown mapping error"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MapError {}

impl From<MapError> for HalError {
    fn from(err: MapError) -> Self {
        match err {
            MapError::AlreadyMapped => HalError::InvalidState("buffer already mapped"),
            MapError::NotMappable => HalError::UnsupportedFeature("buffer mapping"),
            MapError::InvalidRange => HalError::InvalidParameter("map range"),
            MapError::IncompatibleMode => HalError::IncompatibleFormat,
            MapError::DeviceLost => HalError::DeviceLost,
            MapError::Unknown => HalError::Unknown,
        }
    }
}

// ============================================================================
// SurfaceError - Surface/Swapchain Errors
// ============================================================================

/// Error type for surface and swapchain operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceError {
    /// Surface lost (window destroyed, etc.).
    Lost,

    /// Swapchain out of date (resize needed).
    OutOfDate,

    /// No suitable surface format found.
    NoFormat,

    /// Surface not configured.
    NotConfigured,

    /// Timeout acquiring next frame.
    Timeout,
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SurfaceError::Lost => write!(f, "surface lost"),
            SurfaceError::OutOfDate => write!(f, "swapchain out of date"),
            SurfaceError::NoFormat => write!(f, "no suitable surface format"),
            SurfaceError::NotConfigured => write!(f, "surface not configured"),
            SurfaceError::Timeout => write!(f, "timeout acquiring frame"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SurfaceError {}

impl From<SurfaceError> for HalError {
    fn from(err: SurfaceError) -> Self {
        match err {
            SurfaceError::Lost => HalError::DeviceLost,
            SurfaceError::OutOfDate => HalError::InvalidState("swapchain out of date"),
            SurfaceError::NoFormat => HalError::IncompatibleFormat,
            SurfaceError::NotConfigured => HalError::InvalidState("surface not configured"),
            SurfaceError::Timeout => HalError::Timeout,
        }
    }
}

// ============================================================================
// Result Type Aliases
// ============================================================================

/// Result type for HAL operations.
pub type HalResult<T> = Result<T, HalError>;

/// Result type for mapping operations.
pub type MapResult<T> = Result<T, MapError>;

/// Result type for surface operations.
pub type SurfaceResult<T> = Result<T, SurfaceError>;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hal_error_display() {
        let err = HalError::OutOfDeviceMemory;
        let s = format!("{}", err);
        assert!(s.contains("device memory"));
    }

    #[test]
    fn test_hal_error_debug() {
        let err = HalError::DeviceLost;
        let s = format!("{:?}", err);
        assert!(s.contains("DeviceLost"));
    }

    #[test]
    fn test_hal_error_equality() {
        assert_eq!(HalError::DeviceLost, HalError::DeviceLost);
        assert_ne!(HalError::DeviceLost, HalError::DeviceNotFound);
    }

    #[test]
    fn test_hal_error_clone() {
        let err1 = HalError::UnsupportedFeature("raytracing");
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn test_is_recoverable() {
        assert!(HalError::OutOfDeviceMemory.is_recoverable());
        assert!(HalError::Timeout.is_recoverable());
        assert!(!HalError::DeviceLost.is_recoverable());
        assert!(!HalError::InvalidHandle.is_recoverable());
    }

    #[test]
    fn test_is_device_lost() {
        assert!(HalError::DeviceLost.is_device_lost());
        assert!(!HalError::OutOfDeviceMemory.is_device_lost());
    }

    #[test]
    fn test_error_codes_unique() {
        // Collect error codes from representative errors
        let codes = [
            HalError::OutOfDeviceMemory.error_code(),
            HalError::DeviceLost.error_code(),
            HalError::InvalidHandle.error_code(),
            HalError::ValidationError("").error_code(),
            HalError::BackendError { backend: "", code: 0 }.error_code(),
            HalError::CommandBufferOverflow.error_code(),
            HalError::Timeout.error_code(),
            HalError::Unknown.error_code(),
        ];

        // All codes should start with their category digit
        assert!(codes[0] >= 1000 && codes[0] < 2000); // Memory: 1xxx
        assert!(codes[1] >= 2000 && codes[1] < 3000); // Device: 2xxx
        assert!(codes[2] >= 3000 && codes[2] < 4000); // Resource: 3xxx
        assert!(codes[3] >= 4000 && codes[3] < 5000); // Validation: 4xxx
        assert!(codes[4] >= 5000 && codes[4] < 6000); // Backend: 5xxx
        assert!(codes[5] >= 6000 && codes[5] < 7000); // Command: 6xxx
        assert!(codes[6] >= 7000 && codes[6] < 8000); // Sync: 7xxx
        assert!(codes[7] >= 9000 && codes[7] < 10000); // Other: 9xxx
    }

    #[test]
    fn test_buffer_too_small_display() {
        let err = HalError::BufferTooSmall {
            required: 1024,
            available: 512,
        };
        let s = format!("{}", err);
        assert!(s.contains("1024"));
        assert!(s.contains("512"));
    }

    #[test]
    fn test_limit_exceeded_display() {
        let err = HalError::LimitExceeded {
            limit: "max_texture_size",
            requested: 16384,
            maximum: 8192,
        };
        let s = format!("{}", err);
        assert!(s.contains("max_texture_size"));
        assert!(s.contains("16384"));
        assert!(s.contains("8192"));
    }

    #[test]
    fn test_backend_error_display() {
        let err = HalError::BackendError {
            backend: "vulkan",
            code: -4,
        };
        let s = format!("{}", err);
        assert!(s.contains("vulkan"));
        assert!(s.contains("-4"));
    }

    #[test]
    fn test_map_error_display() {
        let err = MapError::AlreadyMapped;
        let s = format!("{}", err);
        assert!(s.contains("already mapped"));
    }

    #[test]
    fn test_map_error_to_hal_error() {
        let hal_err: HalError = MapError::DeviceLost.into();
        assert_eq!(hal_err, HalError::DeviceLost);
    }

    #[test]
    fn test_surface_error_display() {
        let err = SurfaceError::OutOfDate;
        let s = format!("{}", err);
        assert!(s.contains("out of date"));
    }

    #[test]
    fn test_surface_error_to_hal_error() {
        let hal_err: HalError = SurfaceError::Timeout.into();
        assert_eq!(hal_err, HalError::Timeout);
    }

    #[test]
    fn test_send_sync_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<HalError>();
        assert_send_sync::<MapError>();
        assert_send_sync::<SurfaceError>();
    }

    #[test]
    fn test_result_types() {
        let _: HalResult<()> = Ok(());
        let _: MapResult<()> = Ok(());
        let _: SurfaceResult<()> = Ok(());

        let _: HalResult<()> = Err(HalError::Unknown);
        let _: MapResult<()> = Err(MapError::Unknown);
        let _: SurfaceResult<()> = Err(SurfaceError::Lost);
    }
}
