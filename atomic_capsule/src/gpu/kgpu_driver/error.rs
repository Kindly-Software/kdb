//! KGPU-Driver v2.0 Error Types
//!
//! Comprehensive error handling for the pure Rust GPU driver stack.
//!
//! # Design Principles
//!
//! - **Fixed-size**: All error types use `#[repr(u32)]` for stable ABI
//! - **no_std compatible**: Core error types work without std
//! - **const fn**: Most operations are const for compile-time evaluation
//! - **Categorized**: Errors organized by subsystem (0x0100 increments)
//!
//! # Error Code Ranges
//!
//! | Range | Category |
//! |-------|----------|
//! | 0x0001-0x00FF | Device errors |
//! | 0x0100-0x01FF | Memory errors |
//! | 0x0200-0x02FF | Command errors |
//! | 0x0300-0x03FF | Fence errors |
//! | 0x0400-0x04FF | Firmware errors |
//! | 0x0500-0x05FF | Platform errors |
//! | 0x0600-0x06FF | Ring buffer errors |
//! | 0x0700-0x07FF | NVIDIA Trojan errors |
//! | 0xFF00-0xFFFF | Generic errors |
//!
//! # UCE34 Compliance
//!
//! - Q10: T0 Auditable tier (error tracking for compliance)
//! - Q11: Rust transform (type-safe error handling)
//! - Q34: Audit trail (error context for SOX/SOC2/GDPR/HIPAA)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_ERROR_CODES_STABLE`: Error codes are ABI-stable across versions
//! - `#VERIFY_ERROR_CODES_STABLE`: Verified by repr(u32) and explicit discriminants

use core::fmt;

// ============================================================================
// Error Category
// ============================================================================

/// Error category for grouping related errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ErrorCategory {
    /// Device-related errors (open, close, enumerate)
    Device = 0,
    /// Memory allocation and mapping errors
    Memory = 1,
    /// Command submission and execution errors
    Command = 2,
    /// Fence and synchronization errors
    Fence = 3,
    /// Firmware loading and authentication errors
    Firmware = 4,
    /// Platform-specific errors (DRM, PCI, MMIO)
    Platform = 5,
    /// Ring buffer errors
    RingBuffer = 6,
    /// NVIDIA Trojan kernel errors
    Trojan = 7,
    /// Generic/uncategorized errors
    Generic = 255,
}

impl ErrorCategory {
    /// Returns the human-readable name of this category
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Device => "Device",
            Self::Memory => "Memory",
            Self::Command => "Command",
            Self::Fence => "Fence",
            Self::Firmware => "Firmware",
            Self::Platform => "Platform",
            Self::RingBuffer => "RingBuffer",
            Self::Trojan => "Trojan",
            Self::Generic => "Generic",
        }
    }

    /// Returns the error code range start for this category
    #[inline]
    pub const fn range_start(self) -> u32 {
        match self {
            Self::Device => 0x0001,
            Self::Memory => 0x0100,
            Self::Command => 0x0200,
            Self::Fence => 0x0300,
            Self::Firmware => 0x0400,
            Self::Platform => 0x0500,
            Self::RingBuffer => 0x0600,
            Self::Trojan => 0x0700,
            Self::Generic => 0xFF00,
        }
    }

    /// Returns the error code range end for this category
    #[inline]
    pub const fn range_end(self) -> u32 {
        match self {
            Self::Device => 0x00FF,
            Self::Memory => 0x01FF,
            Self::Command => 0x02FF,
            Self::Fence => 0x03FF,
            Self::Firmware => 0x04FF,
            Self::Platform => 0x05FF,
            Self::RingBuffer => 0x06FF,
            Self::Trojan => 0x07FF,
            Self::Generic => 0xFFFF,
        }
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Main Error Enum
// ============================================================================

/// KGPU-Driver error types
///
/// All errors are fixed-size (u32) for stable ABI and efficient transmission.
/// Error codes are organized by category with explicit discriminants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum KgpuDriverError {
    // ========================================================================
    // Device Errors (0x0001 - 0x00FF)
    // ========================================================================

    /// GPU device not found in the system
    DeviceNotFound = 0x0001,

    /// GPU device architecture/generation not supported
    DeviceNotSupported = 0x0002,

    /// GPU device is currently busy (try again later)
    DeviceBusy = 0x0003,

    /// GPU device lost (may need reset or reboot)
    DeviceLost = 0x0004,

    /// GPU device is already open by this process
    DeviceAlreadyOpen = 0x0005,

    /// Invalid device index (out of range)
    InvalidDeviceIndex = 0x0006,

    // ========================================================================
    // Memory Errors (0x0100 - 0x01FF)
    // ========================================================================

    /// Out of GPU/device memory
    OutOfDeviceMemory = 0x0100,

    /// Out of host/system memory
    OutOfHostMemory = 0x0101,

    /// Invalid memory handle (stale or corrupted)
    InvalidMemoryHandle = 0x0102,

    /// Failed to map GPU memory into CPU address space
    MemoryMapFailed = 0x0103,

    /// Memory region is not currently mapped
    MemoryNotMapped = 0x0104,

    /// Memory alignment requirement not met
    InvalidAlignment = 0x0105,

    /// Memory size is invalid (zero or too large)
    InvalidSize = 0x0106,

    /// Memory is still in use by GPU
    MemoryInUse = 0x0107,

    /// Invalid state for the requested operation
    InvalidState = 0x0108,

    /// Memory is already mapped
    MemoryAlreadyMapped = 0x0109,

    /// State transition failed (concurrent modification)
    StateTransitionFailed = 0x010A,

    // ========================================================================
    // Command Errors (0x0200 - 0x02FF)
    // ========================================================================

    /// Command buffer is full (no space for new commands)
    CommandBufferFull = 0x0200,

    /// Invalid command format or opcode
    InvalidCommand = 0x0201,

    /// Command execution timed out
    CommandTimeout = 0x0202,

    /// Command execution failed on GPU
    CommandFailed = 0x0203,

    /// Queue type not supported by this GPU
    QueueNotSupported = 0x0204,

    /// Invalid submission ID (stale or corrupted)
    InvalidSubmissionId = 0x0205,

    // ========================================================================
    // Fence Errors (0x0300 - 0x03FF)
    // ========================================================================

    /// Fence wait timed out
    FenceTimeout = 0x0300,

    /// Invalid fence handle (stale or corrupted)
    InvalidFenceHandle = 0x0301,

    /// Fence has already been signaled
    FenceSignaled = 0x0302,

    /// Fence has not yet been signaled
    FenceNotSignaled = 0x0303,

    // ========================================================================
    // Firmware Errors (0x0400 - 0x04FF)
    // ========================================================================

    /// Required firmware file not found
    FirmwareNotFound = 0x0400,

    /// Firmware file is invalid or corrupted
    FirmwareInvalid = 0x0401,

    /// Failed to load firmware onto GPU
    FirmwareLoadFailed = 0x0402,

    /// Firmware version does not match hardware
    FirmwareMismatch = 0x0403,

    /// Firmware initialization timed out
    FirmwareTimeout = 0x0404,

    /// Firmware bypassed (NVIDIA Trojan mode - not an error)
    FirmwareBypassed = 0x0405,

    // ========================================================================
    // Platform Errors (0x0500 - 0x05FF)
    // ========================================================================

    /// Failed to open DRM device node
    DrmOpenFailed = 0x0500,

    /// DRM ioctl call failed
    DrmIoctlFailed = 0x0501,

    /// PCI configuration space access denied
    PciAccessDenied = 0x0502,

    /// Failed to map MMIO region
    MmioMapFailed = 0x0503,

    /// Failed to setup interrupt handler
    InterruptSetupFailed = 0x0504,

    /// Platform (OS/hardware) not supported
    PlatformNotSupported = 0x0505,

    // ========================================================================
    // Ring Buffer Errors (0x0600 - 0x06FF)
    // ========================================================================

    /// Ring buffer is full (no space for new entries)
    RingBufferFull = 0x0600,

    /// Ring buffer is empty (no entries to consume)
    RingBufferEmpty = 0x0601,

    /// Ring buffer is corrupted (head/tail mismatch)
    RingBufferCorrupted = 0x0602,

    /// Ring buffer submit operation failed
    RingSubmitFailed = 0x0603,

    // ========================================================================
    // NVIDIA Trojan Errors (0x0700 - 0x07FF)
    // ========================================================================

    /// Persistent Trojan kernel is not running
    TrojanKernelNotRunning = 0x0700,

    /// Failed to allocate pinned memory for Trojan ring
    TrojanPinnedMemoryFailed = 0x0701,

    /// CUDA runtime initialization failed
    TrojanCudaInitFailed = 0x0702,

    /// Trojan kernel rejected command as invalid
    TrojanCommandRejected = 0x0703,

    // ========================================================================
    // Generic Errors (0xFF00 - 0xFFFF)
    // ========================================================================

    /// Unknown error (should not occur)
    Unknown = 0xFF00,

    /// Feature or operation not yet implemented
    NotImplemented = 0xFF01,

    /// Invalid parameter passed to function
    InvalidParameter = 0xFF02,

    /// Permission denied (insufficient privileges)
    PermissionDenied = 0xFF03,
}

impl KgpuDriverError {
    /// Returns the error code as a u32
    ///
    /// Error codes are stable and can be used for serialization or FFI.
    #[inline]
    pub const fn code(self) -> u32 {
        self as u32
    }

    /// Returns the error category
    ///
    /// Category is derived from the error code's high byte.
    #[inline]
    pub const fn category(self) -> ErrorCategory {
        match self.code() >> 8 {
            0x00 => ErrorCategory::Device,
            0x01 => ErrorCategory::Memory,
            0x02 => ErrorCategory::Command,
            0x03 => ErrorCategory::Fence,
            0x04 => ErrorCategory::Firmware,
            0x05 => ErrorCategory::Platform,
            0x06 => ErrorCategory::RingBuffer,
            0x07 => ErrorCategory::Trojan,
            _ => ErrorCategory::Generic,
        }
    }

    /// Returns true if the error is recoverable
    ///
    /// Recoverable errors can be retried after a short delay.
    #[inline]
    pub const fn is_recoverable(self) -> bool {
        matches!(
            self,
            Self::DeviceBusy
                | Self::CommandBufferFull
                | Self::RingBufferFull
                | Self::FenceTimeout
                | Self::CommandTimeout
                | Self::FirmwareTimeout
        )
    }

    /// Returns true if the error is fatal
    ///
    /// Fatal errors typically require device reset or system reboot.
    #[inline]
    pub const fn is_fatal(self) -> bool {
        matches!(
            self,
            Self::DeviceLost | Self::RingBufferCorrupted | Self::FirmwareLoadFailed
        )
    }

    /// Returns true if the error is transient
    ///
    /// Transient errors are temporary and may resolve on retry.
    #[inline]
    pub const fn is_transient(self) -> bool {
        matches!(
            self,
            Self::DeviceBusy
                | Self::CommandBufferFull
                | Self::RingBufferFull
                | Self::RingBufferEmpty
                | Self::FenceNotSignaled
                | Self::MemoryInUse
        )
    }

    /// Returns the suggested retry delay in milliseconds
    ///
    /// Returns 0 if retry is not recommended.
    #[inline]
    pub const fn retry_delay_ms(self) -> u32 {
        match self {
            Self::DeviceBusy => 100,
            Self::CommandBufferFull => 1,
            Self::RingBufferFull => 1,
            Self::FenceTimeout => 10,
            Self::CommandTimeout => 10,
            Self::FirmwareTimeout => 1000,
            Self::MemoryInUse => 10,
            Self::FenceNotSignaled => 1,
            Self::RingBufferEmpty => 1,
            _ => 0,
        }
    }

    /// Returns a human-readable description of the error
    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            // Device Errors
            Self::DeviceNotFound => "GPU device not found",
            Self::DeviceNotSupported => "GPU device not supported",
            Self::DeviceBusy => "GPU device is busy",
            Self::DeviceLost => "GPU device lost (may need reset)",
            Self::DeviceAlreadyOpen => "GPU device already open",
            Self::InvalidDeviceIndex => "Invalid device index",

            // Memory Errors
            Self::OutOfDeviceMemory => "Out of GPU memory",
            Self::OutOfHostMemory => "Out of host memory",
            Self::InvalidMemoryHandle => "Invalid memory handle",
            Self::MemoryMapFailed => "Failed to map GPU memory",
            Self::MemoryNotMapped => "Memory is not mapped",
            Self::InvalidAlignment => "Invalid memory alignment",
            Self::InvalidSize => "Invalid memory size",
            Self::MemoryInUse => "Memory is still in use",
            Self::InvalidState => "Invalid state for operation",
            Self::MemoryAlreadyMapped => "Memory is already mapped",
            Self::StateTransitionFailed => "State transition failed",

            // Command Errors
            Self::CommandBufferFull => "Command buffer is full",
            Self::InvalidCommand => "Invalid GPU command",
            Self::CommandTimeout => "Command execution timed out",
            Self::CommandFailed => "Command execution failed",
            Self::QueueNotSupported => "Queue type not supported",
            Self::InvalidSubmissionId => "Invalid submission ID",

            // Fence Errors
            Self::FenceTimeout => "Fence wait timed out",
            Self::InvalidFenceHandle => "Invalid fence handle",
            Self::FenceSignaled => "Fence already signaled",
            Self::FenceNotSignaled => "Fence not yet signaled",

            // Firmware Errors
            Self::FirmwareNotFound => "Firmware file not found",
            Self::FirmwareInvalid => "Firmware file is invalid",
            Self::FirmwareLoadFailed => "Failed to load firmware",
            Self::FirmwareMismatch => "Firmware version mismatch",
            Self::FirmwareTimeout => "Firmware initialization timed out",
            Self::FirmwareBypassed => "Firmware bypassed (Trojan mode)",

            // Platform Errors
            Self::DrmOpenFailed => "Failed to open DRM device",
            Self::DrmIoctlFailed => "DRM ioctl failed",
            Self::PciAccessDenied => "PCI access denied",
            Self::MmioMapFailed => "Failed to map MMIO region",
            Self::InterruptSetupFailed => "Failed to setup interrupts",
            Self::PlatformNotSupported => "Platform not supported",

            // Ring Buffer Errors
            Self::RingBufferFull => "Ring buffer is full",
            Self::RingBufferEmpty => "Ring buffer is empty",
            Self::RingBufferCorrupted => "Ring buffer corrupted",
            Self::RingSubmitFailed => "Ring buffer submit failed",

            // NVIDIA Trojan Errors
            Self::TrojanKernelNotRunning => "NVIDIA Trojan kernel not running",
            Self::TrojanPinnedMemoryFailed => "Failed to allocate pinned memory for Trojan",
            Self::TrojanCudaInitFailed => "CUDA initialization failed for Trojan",
            Self::TrojanCommandRejected => "Trojan kernel rejected command",

            // Generic Errors
            Self::Unknown => "Unknown error",
            Self::NotImplemented => "Feature not implemented",
            Self::InvalidParameter => "Invalid parameter",
            Self::PermissionDenied => "Permission denied",
        }
    }

    /// Returns a short error name suitable for logging
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            // Device Errors
            Self::DeviceNotFound => "DEVICE_NOT_FOUND",
            Self::DeviceNotSupported => "DEVICE_NOT_SUPPORTED",
            Self::DeviceBusy => "DEVICE_BUSY",
            Self::DeviceLost => "DEVICE_LOST",
            Self::DeviceAlreadyOpen => "DEVICE_ALREADY_OPEN",
            Self::InvalidDeviceIndex => "INVALID_DEVICE_INDEX",

            // Memory Errors
            Self::OutOfDeviceMemory => "OUT_OF_DEVICE_MEMORY",
            Self::OutOfHostMemory => "OUT_OF_HOST_MEMORY",
            Self::InvalidMemoryHandle => "INVALID_MEMORY_HANDLE",
            Self::MemoryMapFailed => "MEMORY_MAP_FAILED",
            Self::MemoryNotMapped => "MEMORY_NOT_MAPPED",
            Self::InvalidAlignment => "INVALID_ALIGNMENT",
            Self::InvalidSize => "INVALID_SIZE",
            Self::MemoryInUse => "MEMORY_IN_USE",
            Self::InvalidState => "INVALID_STATE",
            Self::MemoryAlreadyMapped => "MEMORY_ALREADY_MAPPED",
            Self::StateTransitionFailed => "STATE_TRANSITION_FAILED",

            // Command Errors
            Self::CommandBufferFull => "COMMAND_BUFFER_FULL",
            Self::InvalidCommand => "INVALID_COMMAND",
            Self::CommandTimeout => "COMMAND_TIMEOUT",
            Self::CommandFailed => "COMMAND_FAILED",
            Self::QueueNotSupported => "QUEUE_NOT_SUPPORTED",
            Self::InvalidSubmissionId => "INVALID_SUBMISSION_ID",

            // Fence Errors
            Self::FenceTimeout => "FENCE_TIMEOUT",
            Self::InvalidFenceHandle => "INVALID_FENCE_HANDLE",
            Self::FenceSignaled => "FENCE_SIGNALED",
            Self::FenceNotSignaled => "FENCE_NOT_SIGNALED",

            // Firmware Errors
            Self::FirmwareNotFound => "FIRMWARE_NOT_FOUND",
            Self::FirmwareInvalid => "FIRMWARE_INVALID",
            Self::FirmwareLoadFailed => "FIRMWARE_LOAD_FAILED",
            Self::FirmwareMismatch => "FIRMWARE_MISMATCH",
            Self::FirmwareTimeout => "FIRMWARE_TIMEOUT",
            Self::FirmwareBypassed => "FIRMWARE_BYPASSED",

            // Platform Errors
            Self::DrmOpenFailed => "DRM_OPEN_FAILED",
            Self::DrmIoctlFailed => "DRM_IOCTL_FAILED",
            Self::PciAccessDenied => "PCI_ACCESS_DENIED",
            Self::MmioMapFailed => "MMIO_MAP_FAILED",
            Self::InterruptSetupFailed => "INTERRUPT_SETUP_FAILED",
            Self::PlatformNotSupported => "PLATFORM_NOT_SUPPORTED",

            // Ring Buffer Errors
            Self::RingBufferFull => "RING_BUFFER_FULL",
            Self::RingBufferEmpty => "RING_BUFFER_EMPTY",
            Self::RingBufferCorrupted => "RING_BUFFER_CORRUPTED",
            Self::RingSubmitFailed => "RING_SUBMIT_FAILED",

            // NVIDIA Trojan Errors
            Self::TrojanKernelNotRunning => "TROJAN_KERNEL_NOT_RUNNING",
            Self::TrojanPinnedMemoryFailed => "TROJAN_PINNED_MEMORY_FAILED",
            Self::TrojanCudaInitFailed => "TROJAN_CUDA_INIT_FAILED",
            Self::TrojanCommandRejected => "TROJAN_COMMAND_REJECTED",

            // Generic Errors
            Self::Unknown => "UNKNOWN",
            Self::NotImplemented => "NOT_IMPLEMENTED",
            Self::InvalidParameter => "INVALID_PARAMETER",
            Self::PermissionDenied => "PERMISSION_DENIED",
        }
    }

    /// Construct error from raw code (for FFI)
    ///
    /// Returns `Unknown` for unrecognized error codes.
    #[inline]
    pub const fn from_code(code: u32) -> Self {
        match code {
            // Device Errors
            0x0001 => Self::DeviceNotFound,
            0x0002 => Self::DeviceNotSupported,
            0x0003 => Self::DeviceBusy,
            0x0004 => Self::DeviceLost,
            0x0005 => Self::DeviceAlreadyOpen,
            0x0006 => Self::InvalidDeviceIndex,

            // Memory Errors
            0x0100 => Self::OutOfDeviceMemory,
            0x0101 => Self::OutOfHostMemory,
            0x0102 => Self::InvalidMemoryHandle,
            0x0103 => Self::MemoryMapFailed,
            0x0104 => Self::MemoryNotMapped,
            0x0105 => Self::InvalidAlignment,
            0x0106 => Self::InvalidSize,
            0x0107 => Self::MemoryInUse,
            0x0108 => Self::InvalidState,
            0x0109 => Self::MemoryAlreadyMapped,
            0x010A => Self::StateTransitionFailed,

            // Command Errors
            0x0200 => Self::CommandBufferFull,
            0x0201 => Self::InvalidCommand,
            0x0202 => Self::CommandTimeout,
            0x0203 => Self::CommandFailed,
            0x0204 => Self::QueueNotSupported,
            0x0205 => Self::InvalidSubmissionId,

            // Fence Errors
            0x0300 => Self::FenceTimeout,
            0x0301 => Self::InvalidFenceHandle,
            0x0302 => Self::FenceSignaled,
            0x0303 => Self::FenceNotSignaled,

            // Firmware Errors
            0x0400 => Self::FirmwareNotFound,
            0x0401 => Self::FirmwareInvalid,
            0x0402 => Self::FirmwareLoadFailed,
            0x0403 => Self::FirmwareMismatch,
            0x0404 => Self::FirmwareTimeout,
            0x0405 => Self::FirmwareBypassed,

            // Platform Errors
            0x0500 => Self::DrmOpenFailed,
            0x0501 => Self::DrmIoctlFailed,
            0x0502 => Self::PciAccessDenied,
            0x0503 => Self::MmioMapFailed,
            0x0504 => Self::InterruptSetupFailed,
            0x0505 => Self::PlatformNotSupported,

            // Ring Buffer Errors
            0x0600 => Self::RingBufferFull,
            0x0601 => Self::RingBufferEmpty,
            0x0602 => Self::RingBufferCorrupted,
            0x0603 => Self::RingSubmitFailed,

            // NVIDIA Trojan Errors
            0x0700 => Self::TrojanKernelNotRunning,
            0x0701 => Self::TrojanPinnedMemoryFailed,
            0x0702 => Self::TrojanCudaInitFailed,
            0x0703 => Self::TrojanCommandRejected,

            // Generic Errors
            0xFF01 => Self::NotImplemented,
            0xFF02 => Self::InvalidParameter,
            0xFF03 => Self::PermissionDenied,

            // Unknown
            _ => Self::Unknown,
        }
    }
}

// ============================================================================
// Display and Error Traits
// ============================================================================

impl fmt::Display for KgpuDriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[KGPU-{:04X}] {}: {}",
            self.code(),
            self.category().name(),
            self.description()
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for KgpuDriverError {
    // description() is deprecated, Display impl is used instead
}

// ============================================================================
// Result Type Alias
// ============================================================================

/// Result type for KGPU-Driver operations
pub type KgpuDriverResult<T> = Result<T, KgpuDriverError>;

// ============================================================================
// Conversions From OS Errors
// ============================================================================

#[cfg(feature = "std")]
impl From<std::io::Error> for KgpuDriverError {
    fn from(e: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match e.kind() {
            ErrorKind::NotFound => Self::DeviceNotFound,
            ErrorKind::PermissionDenied => Self::PermissionDenied,
            ErrorKind::TimedOut => Self::CommandTimeout,
            ErrorKind::OutOfMemory => Self::OutOfHostMemory,
            ErrorKind::WouldBlock => Self::DeviceBusy,
            ErrorKind::InvalidInput => Self::InvalidParameter,
            ErrorKind::InvalidData => Self::InvalidCommand,
            ErrorKind::Interrupted => Self::DeviceBusy,
            ErrorKind::AlreadyExists => Self::DeviceAlreadyOpen,
            _ => Self::Unknown,
        }
    }
}

// ============================================================================
// Error Context (for rich error reporting)
// ============================================================================

/// Extended error context for debugging and audit trails
///
/// This type wraps a [`KgpuDriverError`] with additional context such as
/// source file, line number, and optional message.
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// The underlying error
    pub error: KgpuDriverError,
    /// Source file where error occurred
    pub file: &'static str,
    /// Line number where error occurred
    pub line: u32,
    /// Optional additional context message
    pub message: Option<&'static str>,
    /// Timestamp (nanoseconds since epoch, if available)
    pub timestamp_ns: u64,
}

impl ErrorContext {
    /// Create a new error context
    #[inline]
    pub const fn new(
        error: KgpuDriverError,
        file: &'static str,
        line: u32,
    ) -> Self {
        Self {
            error,
            file,
            line,
            message: None,
            timestamp_ns: 0,
        }
    }

    /// Create a new error context with a message
    #[inline]
    pub const fn with_message(
        error: KgpuDriverError,
        file: &'static str,
        line: u32,
        message: &'static str,
    ) -> Self {
        Self {
            error,
            file,
            line,
            message: Some(message),
            timestamp_ns: 0,
        }
    }

    /// Set timestamp
    #[inline]
    pub const fn with_timestamp(mut self, timestamp_ns: u64) -> Self {
        self.timestamp_ns = timestamp_ns;
        self
    }

    /// Get the underlying error
    #[inline]
    pub const fn error(&self) -> KgpuDriverError {
        self.error
    }

    /// Check if error is recoverable
    #[inline]
    pub const fn is_recoverable(&self) -> bool {
        self.error.is_recoverable()
    }

    /// Check if error is fatal
    #[inline]
    pub const fn is_fatal(&self) -> bool {
        self.error.is_fatal()
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}:{}", self.error, self.file, self.line)?;
        if let Some(msg) = self.message {
            write!(f, " - {}", msg)?;
        }
        if self.timestamp_ns > 0 {
            write!(f, " [ts={}ns]", self.timestamp_ns)?;
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ErrorContext {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl From<ErrorContext> for KgpuDriverError {
    fn from(ctx: ErrorContext) -> Self {
        ctx.error
    }
}

// ============================================================================
// Macros for Error Creation
// ============================================================================

/// Create an error with file/line context
#[macro_export]
macro_rules! kgpu_error {
    ($err:expr) => {
        $crate::gpu::kgpu_driver::error::ErrorContext::new(
            $err,
            file!(),
            line!(),
        )
    };
    ($err:expr, $msg:expr) => {
        $crate::gpu::kgpu_driver::error::ErrorContext::with_message(
            $err,
            file!(),
            line!(),
            $msg,
        )
    };
}

/// Early return with error context
#[macro_export]
macro_rules! kgpu_bail {
    ($err:expr) => {
        return Err($crate::kgpu_error!($err).into())
    };
    ($err:expr, $msg:expr) => {
        return Err($crate::kgpu_error!($err, $msg).into())
    };
}

/// Ensure condition or return error
#[macro_export]
macro_rules! kgpu_ensure {
    ($cond:expr, $err:expr) => {
        if !($cond) {
            $crate::kgpu_bail!($err);
        }
    };
    ($cond:expr, $err:expr, $msg:expr) => {
        if !($cond) {
            $crate::kgpu_bail!($err, $msg);
        }
    };
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        // Device errors
        assert_eq!(KgpuDriverError::DeviceNotFound.code(), 0x0001);
        assert_eq!(KgpuDriverError::DeviceBusy.code(), 0x0003);
        assert_eq!(KgpuDriverError::DeviceLost.code(), 0x0004);

        // Memory errors
        assert_eq!(KgpuDriverError::OutOfDeviceMemory.code(), 0x0100);
        assert_eq!(KgpuDriverError::InvalidMemoryHandle.code(), 0x0102);
        assert_eq!(KgpuDriverError::InvalidState.code(), 0x0108);
        assert_eq!(KgpuDriverError::MemoryAlreadyMapped.code(), 0x0109);
        assert_eq!(KgpuDriverError::StateTransitionFailed.code(), 0x010A);

        // Command errors
        assert_eq!(KgpuDriverError::CommandBufferFull.code(), 0x0200);
        assert_eq!(KgpuDriverError::CommandTimeout.code(), 0x0202);

        // Fence errors
        assert_eq!(KgpuDriverError::FenceTimeout.code(), 0x0300);
        assert_eq!(KgpuDriverError::FenceNotSignaled.code(), 0x0303);

        // Firmware errors
        assert_eq!(KgpuDriverError::FirmwareNotFound.code(), 0x0400);
        assert_eq!(KgpuDriverError::FirmwareBypassed.code(), 0x0405);

        // Platform errors
        assert_eq!(KgpuDriverError::DrmOpenFailed.code(), 0x0500);
        assert_eq!(KgpuDriverError::PlatformNotSupported.code(), 0x0505);

        // Ring buffer errors
        assert_eq!(KgpuDriverError::RingBufferFull.code(), 0x0600);
        assert_eq!(KgpuDriverError::RingBufferCorrupted.code(), 0x0602);

        // Trojan errors
        assert_eq!(KgpuDriverError::TrojanKernelNotRunning.code(), 0x0700);
        assert_eq!(KgpuDriverError::TrojanCommandRejected.code(), 0x0703);

        // Generic errors
        assert_eq!(KgpuDriverError::Unknown.code(), 0xFF00);
        assert_eq!(KgpuDriverError::PermissionDenied.code(), 0xFF03);
    }

    #[test]
    fn test_error_categories() {
        // Device category
        assert_eq!(KgpuDriverError::DeviceNotFound.category(), ErrorCategory::Device);
        assert_eq!(KgpuDriverError::DeviceBusy.category(), ErrorCategory::Device);

        // Memory category
        assert_eq!(KgpuDriverError::OutOfDeviceMemory.category(), ErrorCategory::Memory);
        assert_eq!(KgpuDriverError::MemoryInUse.category(), ErrorCategory::Memory);
        assert_eq!(KgpuDriverError::InvalidState.category(), ErrorCategory::Memory);
        assert_eq!(KgpuDriverError::MemoryAlreadyMapped.category(), ErrorCategory::Memory);
        assert_eq!(KgpuDriverError::StateTransitionFailed.category(), ErrorCategory::Memory);

        // Command category
        assert_eq!(KgpuDriverError::CommandBufferFull.category(), ErrorCategory::Command);
        assert_eq!(KgpuDriverError::CommandFailed.category(), ErrorCategory::Command);

        // Fence category
        assert_eq!(KgpuDriverError::FenceTimeout.category(), ErrorCategory::Fence);
        assert_eq!(KgpuDriverError::FenceSignaled.category(), ErrorCategory::Fence);

        // Firmware category
        assert_eq!(KgpuDriverError::FirmwareNotFound.category(), ErrorCategory::Firmware);
        assert_eq!(KgpuDriverError::FirmwareBypassed.category(), ErrorCategory::Firmware);

        // Platform category
        assert_eq!(KgpuDriverError::DrmOpenFailed.category(), ErrorCategory::Platform);
        assert_eq!(KgpuDriverError::MmioMapFailed.category(), ErrorCategory::Platform);

        // Ring buffer category
        assert_eq!(KgpuDriverError::RingBufferFull.category(), ErrorCategory::RingBuffer);
        assert_eq!(KgpuDriverError::RingBufferCorrupted.category(), ErrorCategory::RingBuffer);

        // Trojan category
        assert_eq!(KgpuDriverError::TrojanKernelNotRunning.category(), ErrorCategory::Trojan);
        assert_eq!(KgpuDriverError::TrojanCudaInitFailed.category(), ErrorCategory::Trojan);

        // Generic category
        assert_eq!(KgpuDriverError::Unknown.category(), ErrorCategory::Generic);
        assert_eq!(KgpuDriverError::NotImplemented.category(), ErrorCategory::Generic);
    }

    #[test]
    fn test_is_recoverable() {
        // Recoverable errors
        assert!(KgpuDriverError::DeviceBusy.is_recoverable());
        assert!(KgpuDriverError::CommandBufferFull.is_recoverable());
        assert!(KgpuDriverError::RingBufferFull.is_recoverable());
        assert!(KgpuDriverError::FenceTimeout.is_recoverable());
        assert!(KgpuDriverError::CommandTimeout.is_recoverable());
        assert!(KgpuDriverError::FirmwareTimeout.is_recoverable());

        // Non-recoverable errors
        assert!(!KgpuDriverError::DeviceNotFound.is_recoverable());
        assert!(!KgpuDriverError::DeviceLost.is_recoverable());
        assert!(!KgpuDriverError::OutOfDeviceMemory.is_recoverable());
        assert!(!KgpuDriverError::RingBufferCorrupted.is_recoverable());
    }

    #[test]
    fn test_is_fatal() {
        // Fatal errors
        assert!(KgpuDriverError::DeviceLost.is_fatal());
        assert!(KgpuDriverError::RingBufferCorrupted.is_fatal());
        assert!(KgpuDriverError::FirmwareLoadFailed.is_fatal());

        // Non-fatal errors
        assert!(!KgpuDriverError::DeviceBusy.is_fatal());
        assert!(!KgpuDriverError::FenceTimeout.is_fatal());
        assert!(!KgpuDriverError::OutOfDeviceMemory.is_fatal());
    }

    #[test]
    fn test_is_transient() {
        // Transient errors
        assert!(KgpuDriverError::DeviceBusy.is_transient());
        assert!(KgpuDriverError::CommandBufferFull.is_transient());
        assert!(KgpuDriverError::RingBufferFull.is_transient());
        assert!(KgpuDriverError::RingBufferEmpty.is_transient());
        assert!(KgpuDriverError::FenceNotSignaled.is_transient());
        assert!(KgpuDriverError::MemoryInUse.is_transient());

        // Non-transient errors
        assert!(!KgpuDriverError::DeviceLost.is_transient());
        assert!(!KgpuDriverError::OutOfDeviceMemory.is_transient());
    }

    #[test]
    fn test_retry_delay() {
        assert_eq!(KgpuDriverError::DeviceBusy.retry_delay_ms(), 100);
        assert_eq!(KgpuDriverError::CommandBufferFull.retry_delay_ms(), 1);
        assert_eq!(KgpuDriverError::FirmwareTimeout.retry_delay_ms(), 1000);
        assert_eq!(KgpuDriverError::DeviceLost.retry_delay_ms(), 0);
    }

    #[test]
    fn test_from_code_roundtrip() {
        let errors = [
            KgpuDriverError::DeviceNotFound,
            KgpuDriverError::OutOfDeviceMemory,
            KgpuDriverError::CommandBufferFull,
            KgpuDriverError::FenceTimeout,
            KgpuDriverError::FirmwareBypassed,
            KgpuDriverError::DrmIoctlFailed,
            KgpuDriverError::RingBufferCorrupted,
            KgpuDriverError::TrojanKernelNotRunning,
            KgpuDriverError::PermissionDenied,
        ];

        for err in errors {
            let code = err.code();
            let reconstructed = KgpuDriverError::from_code(code);
            assert_eq!(err, reconstructed, "Roundtrip failed for {:?}", err);
        }
    }

    #[test]
    fn test_from_code_unknown() {
        assert_eq!(KgpuDriverError::from_code(0xFFFF), KgpuDriverError::Unknown);
        assert_eq!(KgpuDriverError::from_code(0x9999), KgpuDriverError::Unknown);
        assert_eq!(KgpuDriverError::from_code(0), KgpuDriverError::Unknown);
    }

    #[test]
    fn test_display_format() {
        let err = KgpuDriverError::DeviceNotFound;
        let display = format!("{}", err);
        assert!(display.contains("[KGPU-0001]"));
        assert!(display.contains("Device"));
        assert!(display.contains("GPU device not found"));

        let err = KgpuDriverError::OutOfDeviceMemory;
        let display = format!("{}", err);
        assert!(display.contains("[KGPU-0100]"));
        assert!(display.contains("Memory"));
    }

    #[test]
    fn test_error_description() {
        assert_eq!(
            KgpuDriverError::DeviceNotFound.description(),
            "GPU device not found"
        );
        assert_eq!(
            KgpuDriverError::FirmwareBypassed.description(),
            "Firmware bypassed (Trojan mode)"
        );
    }

    #[test]
    fn test_error_name() {
        assert_eq!(KgpuDriverError::DeviceNotFound.name(), "DEVICE_NOT_FOUND");
        assert_eq!(KgpuDriverError::OutOfDeviceMemory.name(), "OUT_OF_DEVICE_MEMORY");
        assert_eq!(KgpuDriverError::TrojanKernelNotRunning.name(), "TROJAN_KERNEL_NOT_RUNNING");
    }

    #[test]
    fn test_category_names() {
        assert_eq!(ErrorCategory::Device.name(), "Device");
        assert_eq!(ErrorCategory::Memory.name(), "Memory");
        assert_eq!(ErrorCategory::Trojan.name(), "Trojan");
        assert_eq!(ErrorCategory::Generic.name(), "Generic");
    }

    #[test]
    fn test_category_ranges() {
        assert_eq!(ErrorCategory::Device.range_start(), 0x0001);
        assert_eq!(ErrorCategory::Device.range_end(), 0x00FF);
        assert_eq!(ErrorCategory::Memory.range_start(), 0x0100);
        assert_eq!(ErrorCategory::Memory.range_end(), 0x01FF);
        assert_eq!(ErrorCategory::Generic.range_start(), 0xFF00);
        assert_eq!(ErrorCategory::Generic.range_end(), 0xFFFF);
    }

    #[test]
    fn test_error_context() {
        let ctx = ErrorContext::new(
            KgpuDriverError::DeviceNotFound,
            "test.rs",
            42,
        );
        assert_eq!(ctx.error(), KgpuDriverError::DeviceNotFound);
        assert_eq!(ctx.file, "test.rs");
        assert_eq!(ctx.line, 42);
        assert!(ctx.message.is_none());
        assert!(!ctx.is_fatal());
        assert!(!ctx.is_recoverable());
    }

    #[test]
    fn test_error_context_with_message() {
        let ctx = ErrorContext::with_message(
            KgpuDriverError::MemoryMapFailed,
            "memory.rs",
            100,
            "Failed to map 4MB region",
        );
        assert_eq!(ctx.message, Some("Failed to map 4MB region"));
    }

    #[test]
    fn test_error_context_display() {
        let ctx = ErrorContext::with_message(
            KgpuDriverError::DeviceLost,
            "device.rs",
            50,
            "GPU hung",
        ).with_timestamp(1234567890);

        let display = format!("{}", ctx);
        assert!(display.contains("KGPU-0004"));
        assert!(display.contains("device.rs:50"));
        assert!(display.contains("GPU hung"));
        assert!(display.contains("[ts=1234567890ns]"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_from_io_error() {
        use std::io::{Error, ErrorKind};

        assert_eq!(
            KgpuDriverError::from(Error::new(ErrorKind::NotFound, "test")),
            KgpuDriverError::DeviceNotFound
        );
        assert_eq!(
            KgpuDriverError::from(Error::new(ErrorKind::PermissionDenied, "test")),
            KgpuDriverError::PermissionDenied
        );
        assert_eq!(
            KgpuDriverError::from(Error::new(ErrorKind::TimedOut, "test")),
            KgpuDriverError::CommandTimeout
        );
        assert_eq!(
            KgpuDriverError::from(Error::new(ErrorKind::WouldBlock, "test")),
            KgpuDriverError::DeviceBusy
        );
        assert_eq!(
            KgpuDriverError::from(Error::new(ErrorKind::Other, "test")),
            KgpuDriverError::Unknown
        );
    }

    #[test]
    fn test_error_size() {
        // Ensure error enum is exactly 4 bytes (u32)
        assert_eq!(core::mem::size_of::<KgpuDriverError>(), 4);
        // Ensure category enum is exactly 1 byte (u8)
        assert_eq!(core::mem::size_of::<ErrorCategory>(), 1);
    }
}
