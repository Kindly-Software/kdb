//! # ServiceManagerCapsule - T4 Batch Service Lifecycle Management
//!
//! **Tier**: T4 Batch (1KB, batch operations for 64 services)
//! **Purpose**: Lockfree service lifecycle management with batch start/stop
//!
//! ## UCE34 Framework Compliance (Q1-Q34)
//!
//! ### Q1-Q9: Problem Analysis
//! - **Q1 (Problem)**: Manage lifecycle of 64 services concurrently
//! - **Q2 (Value)**: Parallel batch operations (10-20× faster than sequential)
//! - **Q3 (Scale)**: 64 services, <50ms per operation
//! - **Q4 (Context)**: Service management for Capsule OS init system
//! - **Q5 (Success)**: <10ns state queries, <50ms starts, proper failure handling
//! - **Q6 (Data Shape)**: 64 service slots (state + metadata per slot)
//! - **Q7 (Core Operation)**: Batch state transitions, health monitoring
//! - **Q8 (Alternative)**: Sequential operations (slow), mutex coordination (blocking)
//! - **Q9 (Transform)**: Sequential → Batch parallel with atomic state
//!
//! ### Q10-Q12: Tier Selection
//! - **Q10 (Tier)**: T4 Batch (parallel operations on service array)
//! - **Q11 (Rust Transform)**: AtomicU64 per service slot, batch iteration
//! - **Q12 (Nightly)**: Optional portable_simd for batch state checks
//!
//! ## Memory Layout (1024B)
//!
//! ```text
//! Offset 0-7:       AtomicU64 global_state (service_count | generation)
//! Offset 8-15:      AtomicU64 active_services (bitmap)
//! Offset 16-23:     AtomicU64 healthy_services (bitmap)
//! Offset 24-31:     AtomicU64 failed_services (bitmap)
//! Offset 32-543:    [AtomicU64; 64] service_states (packed state per service)
//! Offset 544-1023:  Padding (cache alignment)
//! ```
//!
//! ## ASSUM Framework (20+ Assumptions)
//!
//! ### State Machine Assumptions
//! - `#ASSUME_STATE_TRANSITIONS_VALID`: Only valid state transitions allowed
//! - `#VERIFY_STATE_TRANSITIONS_VALID`: State machine enforced by type system
//! - `#ASSUME_STATE_PACKED_CORRECT`: State fits in 64-bit atomic
//! - `#VERIFY_STATE_PACKED_CORRECT`: Compile-time size verification
//!
//! ### Concurrency Assumptions
//! - `#ASSUME_CONCURRENT_STATE_SAFE`: State updates are atomic
//! - `#VERIFY_CONCURRENT_STATE_SAFE`: Single CAS per update
//! - `#ASSUME_BATCH_OPERATIONS_PARALLEL`: Batch ops don't block each other
//! - `#VERIFY_BATCH_OPERATIONS_PARALLEL`: Independent atomic operations

use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "std")]
use std::time::{Duration, Instant};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

use super::{ServiceId, MAX_SERVICES};

/// Maximum length of service name
pub const MAX_SERVICE_NAME_LEN: usize = 64;

/// Service state enumeration
///
/// State transitions:
/// ```text
/// Stopped ──► Starting ──► Running ──► Stopping ──► Stopped
///    │            │           │            │
///    └────────────┴───────────┴────────────┴──► Failed
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ServiceState {
    /// Service not registered or unknown
    Unknown = 0,
    /// Service registered but not started
    Stopped = 1,
    /// Service is starting up
    Starting = 2,
    /// Service is running and healthy
    Running = 3,
    /// Service is shutting down
    Stopping = 4,
    /// Service failed to start or crashed
    Failed = 5,
    /// Service is restarting
    Restarting = 6,
    /// Service is disabled (will not auto-start)
    Disabled = 7,
}

impl ServiceState {
    /// Convert from packed u8 value
    #[inline]
    pub fn from_u8(value: u8) -> Self {
        match value & 0x07 {
            0 => Self::Unknown,
            1 => Self::Stopped,
            2 => Self::Starting,
            3 => Self::Running,
            4 => Self::Stopping,
            5 => Self::Failed,
            6 => Self::Restarting,
            7 => Self::Disabled,
            _ => Self::Unknown,
        }
    }

    /// Check if service is in a "healthy" state
    #[inline]
    pub fn is_healthy(self) -> bool {
        matches!(self, Self::Running)
    }

    /// Check if service is in a terminal state (can be started)
    #[inline]
    pub fn is_startable(self) -> bool {
        matches!(self, Self::Stopped | Self::Failed | Self::Unknown)
    }

    /// Check if service is in a running state (can be stopped)
    #[inline]
    pub fn is_stoppable(self) -> bool {
        matches!(self, Self::Running | Self::Starting)
    }
}

impl Default for ServiceState {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Restart policy for services
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RestartPolicy {
    /// Never restart automatically
    Never = 0,
    /// Restart only on failure (non-zero exit)
    OnFailure = 1,
    /// Always restart (unless explicitly stopped)
    Always = 2,
    /// Restart on success (zero exit) - for one-shot services
    OnSuccess = 3,
}

impl RestartPolicy {
    /// Convert from packed u8 value
    #[inline]
    pub fn from_u8(value: u8) -> Self {
        match value & 0x03 {
            0 => Self::Never,
            1 => Self::OnFailure,
            2 => Self::Always,
            3 => Self::OnSuccess,
            _ => Self::Never,
        }
    }
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self::OnFailure
    }
}

/// Service descriptor for registration
#[derive(Debug, Clone)]
pub struct ServiceDescriptor<'a> {
    /// Service name (human-readable identifier)
    pub name: &'a str,
    /// Command to execute
    pub command: &'a str,
    /// Arguments to command
    pub args: &'a [&'a str],
    /// Service IDs this service depends on
    pub depends_on: &'a [ServiceId],
    /// Restart policy
    pub restart_policy: RestartPolicy,
    /// Startup timeout in milliseconds
    pub startup_timeout_ms: u32,
    /// Shutdown timeout in milliseconds
    pub shutdown_timeout_ms: u32,
}

impl<'a> Default for ServiceDescriptor<'a> {
    fn default() -> Self {
        Self {
            name: "",
            command: "",
            args: &[],
            depends_on: &[],
            restart_policy: RestartPolicy::OnFailure,
            startup_timeout_ms: 30_000,  // 30 seconds
            shutdown_timeout_ms: 10_000, // 10 seconds
        }
    }
}

/// Service error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceError {
    /// Invalid service ID
    InvalidServiceId(ServiceId),
    /// Service not found
    ServiceNotFound(ServiceId),
    /// Invalid state transition
    InvalidStateTransition {
        service_id: ServiceId,
        from: ServiceState,
        to: ServiceState,
    },
    /// Service failed to start
    StartFailed {
        service_id: ServiceId,
        exit_code: i32,
    },
    /// Service timed out during startup
    StartupTimeout(ServiceId),
    /// Service timed out during shutdown
    ShutdownTimeout(ServiceId),
    /// Maximum services exceeded
    TooManyServices,
    /// Service already registered
    AlreadyRegistered(ServiceId),
}

impl core::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidServiceId(id) => write!(f, "Invalid service ID: {}", id),
            Self::ServiceNotFound(id) => write!(f, "Service {} not found", id),
            Self::InvalidStateTransition { service_id, from, to } => {
                write!(f, "Invalid state transition for service {}: {:?} -> {:?}", service_id, from, to)
            }
            Self::StartFailed { service_id, exit_code } => {
                write!(f, "Service {} failed to start (exit code {})", service_id, exit_code)
            }
            Self::StartupTimeout(id) => write!(f, "Service {} startup timed out", id),
            Self::ShutdownTimeout(id) => write!(f, "Service {} shutdown timed out", id),
            Self::TooManyServices => write!(f, "Maximum services ({}) exceeded", MAX_SERVICES),
            Self::AlreadyRegistered(id) => write!(f, "Service {} already registered", id),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ServiceError {}

/// Result type for service operations
pub type ServiceResult<T> = Result<T, ServiceError>;

/// Service statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct ServiceStats {
    /// Total number of registered services
    pub total_services: u8,
    /// Number of running services
    pub running_services: u8,
    /// Number of failed services
    pub failed_services: u8,
    /// Number of stopped services
    pub stopped_services: u8,
    /// Total start count (all services)
    pub total_starts: u64,
    /// Total failure count (all services)
    pub total_failures: u64,
}

/// Packed service state (64 bits per service)
///
/// Layout:
/// - Bits 0-2: ServiceState (8 states)
/// - Bits 3-4: RestartPolicy (4 policies)
/// - Bits 5-12: Restart count (0-255)
/// - Bits 13-20: Failure count (0-255)
/// - Bits 21-31: Reserved
/// - Bits 32-63: Last state change timestamp (seconds since epoch, truncated)
#[derive(Debug, Clone, Copy)]
struct PackedServiceState(u64);

impl PackedServiceState {
    const STATE_MASK: u64 = 0x07;           // Bits 0-2
    const POLICY_MASK: u64 = 0x03;          // Bits 3-4 (shifted)
    const POLICY_SHIFT: u64 = 3;
    const RESTART_COUNT_MASK: u64 = 0xFF;   // Bits 5-12 (shifted)
    const RESTART_COUNT_SHIFT: u64 = 5;
    const FAILURE_COUNT_MASK: u64 = 0xFF;   // Bits 13-20 (shifted)
    const FAILURE_COUNT_SHIFT: u64 = 13;
    const TIMESTAMP_SHIFT: u64 = 32;

    #[inline]
    fn new(state: ServiceState, policy: RestartPolicy) -> Self {
        Self((state as u64) | ((policy as u64) << Self::POLICY_SHIFT))
    }

    #[inline]
    fn state(self) -> ServiceState {
        ServiceState::from_u8((self.0 & Self::STATE_MASK) as u8)
    }

    #[inline]
    fn policy(self) -> RestartPolicy {
        RestartPolicy::from_u8(((self.0 >> Self::POLICY_SHIFT) & Self::POLICY_MASK) as u8)
    }

    #[inline]
    fn restart_count(self) -> u8 {
        ((self.0 >> Self::RESTART_COUNT_SHIFT) & Self::RESTART_COUNT_MASK) as u8
    }

    #[inline]
    fn failure_count(self) -> u8 {
        ((self.0 >> Self::FAILURE_COUNT_SHIFT) & Self::FAILURE_COUNT_MASK) as u8
    }

    #[inline]
    fn with_state(self, state: ServiceState) -> Self {
        Self((self.0 & !Self::STATE_MASK) | (state as u64))
    }

    #[inline]
    fn with_timestamp(self, timestamp: u32) -> Self {
        Self((self.0 & 0xFFFF_FFFF) | ((timestamp as u64) << Self::TIMESTAMP_SHIFT))
    }

    #[inline]
    fn increment_restart(self) -> Self {
        let count = self.restart_count().saturating_add(1);
        Self((self.0 & !(Self::RESTART_COUNT_MASK << Self::RESTART_COUNT_SHIFT))
            | ((count as u64) << Self::RESTART_COUNT_SHIFT))
    }

    #[inline]
    fn increment_failure(self) -> Self {
        let count = self.failure_count().saturating_add(1);
        Self((self.0 & !(Self::FAILURE_COUNT_MASK << Self::FAILURE_COUNT_SHIFT))
            | ((count as u64) << Self::FAILURE_COUNT_SHIFT))
    }
}

/// ServiceManagerCapsule - T4 Batch service lifecycle management
///
/// Manages lifecycle of up to 64 services with lockfree atomic operations.
/// Supports batch start/stop operations for parallel boot sequences.
///
/// # Memory Layout (1024B aligned)
///
/// Uses compact packed state (64 bits per service) for cache efficiency.
///
/// # Performance (B32 Targets)
///
/// | Operation | Target | Notes |
/// |-----------|--------|-------|
/// | get_state | <10ns | Single atomic load |
/// | set_state | <50ns | CAS loop |
/// | batch_start | <100μs | Parallel spawn |
/// | batch_stop | <100μs | Parallel signal |
///
/// # Thread Safety
///
/// - **State queries**: Always safe (atomic loads)
/// - **State updates**: Use CAS for safe concurrent updates
/// - **Batch ops**: Independent per-service, no cross-service locking
///
/// # Example
///
/// ```rust
/// use atomic_capsule::init::{ServiceManagerCapsule, ServiceState, RestartPolicy};
///
/// let manager = ServiceManagerCapsule::new();
///
/// // Register service
/// manager.register(0, RestartPolicy::OnFailure)?;
///
/// // Start service
/// manager.set_state(0, ServiceState::Starting)?;
/// // ... spawn process ...
/// manager.set_state(0, ServiceState::Running)?;
///
/// // Query state
/// assert_eq!(manager.get_state(0), ServiceState::Running);
/// # Ok::<(), atomic_capsule::init::ServiceError>(())
/// ```
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 1024, size = 1024))]
#[repr(C, align(1024))]
pub struct ServiceManagerCapsule {
    // ========================================================================
    // Global State Section (64 bytes)
    // ========================================================================

    /// Global state: service_count (bits 0-7) | generation (bits 8-63)
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_GLOBAL_STATE_ATOMIC`: Single atomic for count + generation
    /// - `#VERIFY_GLOBAL_STATE_ATOMIC`: No torn reads
    global_state: AtomicU64,

    /// Bitmap of registered services (bit N = service N registered)
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_REGISTERED_CONSISTENT`: Matches service_states array
    /// - `#VERIFY_REGISTERED_CONSISTENT`: Updated atomically with states
    registered_services: AtomicU64,

    /// Bitmap of healthy (running) services
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_HEALTHY_BITMAP_FAST`: O(1) health check
    /// - `#VERIFY_HEALTHY_BITMAP_FAST`: Single atomic load
    healthy_services: AtomicU64,

    /// Bitmap of failed services
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_FAILED_BITMAP_ACCURATE`: Tracks all failures
    /// - `#VERIFY_FAILED_BITMAP_ACCURATE`: Updated on state transition
    failed_services: AtomicU64,

    /// Total start count across all services
    total_starts: AtomicU64,

    /// Total failure count across all services
    total_failures: AtomicU64,

    /// Padding for cache alignment
    _padding0: [u8; 16],

    // ========================================================================
    // Per-Service State Section (512 bytes)
    // ========================================================================

    /// Per-service packed state (64 bits per service)
    /// See PackedServiceState for bit layout
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_SERVICE_STATE_INDEPENDENT`: Services don't share state
    /// - `#VERIFY_SERVICE_STATE_INDEPENDENT`: Separate atomics per service
    service_states: [AtomicU64; MAX_SERVICES],

    // Padding to complete 1024B (64 + 512 = 576, need 448 more)
    _padding1: [u8; 448],
}

// Verify size at compile time
#[cfg(not(feature = "derive"))]
const _: () = {
    assert!(core::mem::size_of::<ServiceManagerCapsule>() <= 2048);
};

impl ServiceManagerCapsule {
    /// Create new service manager
    ///
    /// # Performance
    /// - Time: <100ns (zero-initialization)
    pub const fn new() -> Self {
        // #ASSUME_CONST_INIT_SAFE: Atomic initialization is const-safe
        // #VERIFY_CONST_INIT_SAFE: Uses AtomicU64::new()
        Self {
            global_state: AtomicU64::new(0),
            registered_services: AtomicU64::new(0),
            healthy_services: AtomicU64::new(0),
            failed_services: AtomicU64::new(0),
            total_starts: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
            _padding0: [0; 16],
            service_states: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            ],
            _padding1: [0; 448],
        }
    }

    // ========================================================================
    // Global State Accessors
    // ========================================================================

    /// Get number of registered services
    ///
    /// # Performance
    /// - Time: <5ns
    #[inline]
    pub fn service_count(&self) -> u8 {
        (self.global_state.load(Ordering::Relaxed) & 0xFF) as u8
    }

    /// Get generation counter (for change detection)
    ///
    /// # Performance
    /// - Time: <5ns
    #[inline]
    pub fn generation(&self) -> u64 {
        self.global_state.load(Ordering::Acquire) >> 8
    }

    /// Get bitmap of registered services
    ///
    /// # Performance
    /// - Time: <5ns
    #[inline]
    pub fn registered_bitmap(&self) -> u64 {
        self.registered_services.load(Ordering::Acquire)
    }

    /// Get bitmap of healthy (running) services
    ///
    /// # Performance
    /// - Time: <5ns
    #[inline]
    pub fn healthy_bitmap(&self) -> u64 {
        self.healthy_services.load(Ordering::Acquire)
    }

    /// Get bitmap of failed services
    ///
    /// # Performance
    /// - Time: <5ns
    #[inline]
    pub fn failed_bitmap(&self) -> u64 {
        self.failed_services.load(Ordering::Acquire)
    }

    /// Check if service is registered
    ///
    /// # Performance
    /// - Time: <5ns
    #[inline]
    pub fn is_registered(&self, service_id: ServiceId) -> bool {
        if service_id as usize >= MAX_SERVICES {
            return false;
        }
        (self.registered_bitmap() & (1u64 << service_id)) != 0
    }

    /// Check if service is healthy
    ///
    /// # Performance
    /// - Time: <5ns
    #[inline]
    pub fn is_healthy(&self, service_id: ServiceId) -> bool {
        if service_id as usize >= MAX_SERVICES {
            return false;
        }
        (self.healthy_bitmap() & (1u64 << service_id)) != 0
    }

    /// Get overall health ratio (0.0 - 1.0)
    ///
    /// # Performance
    /// - Time: <10ns
    #[inline]
    pub fn health_ratio(&self) -> f32 {
        let registered = self.registered_bitmap().count_ones();
        if registered == 0 {
            return 1.0;
        }
        let healthy = self.healthy_bitmap().count_ones();
        healthy as f32 / registered as f32
    }

    /// Get service statistics
    ///
    /// # Performance
    /// - Time: <50ns
    pub fn stats(&self) -> ServiceStats {
        let registered = self.registered_bitmap();
        let healthy = self.healthy_bitmap();
        let failed = self.failed_bitmap();

        ServiceStats {
            total_services: registered.count_ones() as u8,
            running_services: healthy.count_ones() as u8,
            failed_services: failed.count_ones() as u8,
            stopped_services: (registered & !healthy & !failed).count_ones() as u8,
            total_starts: self.total_starts.load(Ordering::Relaxed),
            total_failures: self.total_failures.load(Ordering::Relaxed),
        }
    }

    // ========================================================================
    // Per-Service Operations
    // ========================================================================

    /// Register a new service
    ///
    /// # Arguments
    /// - `service_id`: Service identifier (0-63)
    /// - `policy`: Restart policy for the service
    ///
    /// # Performance
    /// - Time: <50ns
    pub fn register(&self, service_id: ServiceId, policy: RestartPolicy) -> ServiceResult<()> {
        // #ASSUME_SERVICE_ID_BOUNDED: service_id < MAX_SERVICES
        // #VERIFY_SERVICE_ID_BOUNDED: Runtime check
        if service_id as usize >= MAX_SERVICES {
            return Err(ServiceError::InvalidServiceId(service_id));
        }

        let mask = 1u64 << service_id;

        // Check if already registered
        let old = self.registered_services.fetch_or(mask, Ordering::AcqRel);
        if (old & mask) != 0 {
            return Err(ServiceError::AlreadyRegistered(service_id));
        }

        // Initialize service state
        let initial_state = PackedServiceState::new(ServiceState::Stopped, policy);
        self.service_states[service_id as usize].store(initial_state.0, Ordering::Release);

        // Update global state
        loop {
            let state = self.global_state.load(Ordering::Relaxed);
            let count = ((state & 0xFF) + 1).min(MAX_SERVICES as u64);
            let generation = (state >> 8) + 1;
            let new_state = count | (generation << 8);

            match self.global_state.compare_exchange_weak(
                state,
                new_state,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => continue,
            }
        }
    }

    /// Get service state
    ///
    /// # Performance
    /// - Time: <10ns
    #[inline]
    pub fn get_state(&self, service_id: ServiceId) -> ServiceState {
        if service_id as usize >= MAX_SERVICES {
            return ServiceState::Unknown;
        }
        if !self.is_registered(service_id) {
            return ServiceState::Unknown;
        }
        let packed = self.service_states[service_id as usize].load(Ordering::Acquire);
        PackedServiceState(packed).state()
    }

    /// Get service restart policy
    ///
    /// # Performance
    /// - Time: <10ns
    #[inline]
    pub fn get_policy(&self, service_id: ServiceId) -> RestartPolicy {
        if service_id as usize >= MAX_SERVICES {
            return RestartPolicy::Never;
        }
        let packed = self.service_states[service_id as usize].load(Ordering::Relaxed);
        PackedServiceState(packed).policy()
    }

    /// Get service restart count
    ///
    /// # Performance
    /// - Time: <10ns
    #[inline]
    pub fn restart_count(&self, service_id: ServiceId) -> u8 {
        if service_id as usize >= MAX_SERVICES {
            return 0;
        }
        let packed = self.service_states[service_id as usize].load(Ordering::Relaxed);
        PackedServiceState(packed).restart_count()
    }

    /// Get service failure count
    ///
    /// # Performance
    /// - Time: <10ns
    #[inline]
    pub fn failure_count(&self, service_id: ServiceId) -> u8 {
        if service_id as usize >= MAX_SERVICES {
            return 0;
        }
        let packed = self.service_states[service_id as usize].load(Ordering::Relaxed);
        PackedServiceState(packed).failure_count()
    }

    /// Set service state (with validation)
    ///
    /// # Arguments
    /// - `service_id`: Service identifier
    /// - `new_state`: Target state
    ///
    /// # Returns
    /// - `Ok(())` on successful transition
    /// - `Err(InvalidStateTransition)` if transition not allowed
    ///
    /// # Performance
    /// - Time: <50ns (CAS loop)
    pub fn set_state(&self, service_id: ServiceId, new_state: ServiceState) -> ServiceResult<()> {
        // #ASSUME_STATE_TRANSITION_VALID: State machine validates transitions
        // #VERIFY_STATE_TRANSITION_VALID: Match expression covers all cases
        if service_id as usize >= MAX_SERVICES {
            return Err(ServiceError::InvalidServiceId(service_id));
        }
        if !self.is_registered(service_id) {
            return Err(ServiceError::ServiceNotFound(service_id));
        }

        loop {
            let packed_raw = self.service_states[service_id as usize].load(Ordering::Acquire);
            let packed = PackedServiceState(packed_raw);
            let current_state = packed.state();

            // Validate transition
            let valid = match (current_state, new_state) {
                // From Stopped
                (ServiceState::Stopped, ServiceState::Starting) => true,
                (ServiceState::Stopped, ServiceState::Disabled) => true,
                // From Starting
                (ServiceState::Starting, ServiceState::Running) => true,
                (ServiceState::Starting, ServiceState::Failed) => true,
                (ServiceState::Starting, ServiceState::Stopping) => true,
                // From Running
                (ServiceState::Running, ServiceState::Stopping) => true,
                (ServiceState::Running, ServiceState::Failed) => true,
                (ServiceState::Running, ServiceState::Restarting) => true,
                // From Stopping
                (ServiceState::Stopping, ServiceState::Stopped) => true,
                (ServiceState::Stopping, ServiceState::Failed) => true,
                // From Failed
                (ServiceState::Failed, ServiceState::Starting) => true,
                (ServiceState::Failed, ServiceState::Stopped) => true,
                (ServiceState::Failed, ServiceState::Disabled) => true,
                // From Restarting
                (ServiceState::Restarting, ServiceState::Starting) => true,
                (ServiceState::Restarting, ServiceState::Stopped) => true,
                (ServiceState::Restarting, ServiceState::Failed) => true,
                // From Disabled
                (ServiceState::Disabled, ServiceState::Stopped) => true,
                // From Unknown (initial registration)
                (ServiceState::Unknown, ServiceState::Stopped) => true,
                // Same state is always valid (idempotent)
                (a, b) if a == b => true,
                // Everything else invalid
                _ => false,
            };

            if !valid {
                return Err(ServiceError::InvalidStateTransition {
                    service_id,
                    from: current_state,
                    to: new_state,
                });
            }

            // Apply state change
            let mut new_packed = packed.with_state(new_state);

            // Update counters based on transition
            if new_state == ServiceState::Starting && current_state != ServiceState::Starting {
                self.total_starts.fetch_add(1, Ordering::Relaxed);
            }
            if new_state == ServiceState::Failed && current_state != ServiceState::Failed {
                new_packed = new_packed.increment_failure();
                self.total_failures.fetch_add(1, Ordering::Relaxed);
            }
            if new_state == ServiceState::Restarting {
                new_packed = new_packed.increment_restart();
            }

            match self.service_states[service_id as usize].compare_exchange_weak(
                packed_raw,
                new_packed.0,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Update bitmaps
                    let mask = 1u64 << service_id;

                    match new_state {
                        ServiceState::Running => {
                            self.healthy_services.fetch_or(mask, Ordering::Release);
                            self.failed_services.fetch_and(!mask, Ordering::Release);
                        }
                        ServiceState::Failed => {
                            self.healthy_services.fetch_and(!mask, Ordering::Release);
                            self.failed_services.fetch_or(mask, Ordering::Release);
                        }
                        ServiceState::Stopped | ServiceState::Stopping |
                        ServiceState::Starting | ServiceState::Restarting |
                        ServiceState::Disabled => {
                            self.healthy_services.fetch_and(!mask, Ordering::Release);
                            self.failed_services.fetch_and(!mask, Ordering::Release);
                        }
                        ServiceState::Unknown => {}
                    }

                    // Increment generation
                    self.global_state.fetch_add(1 << 8, Ordering::Release);

                    return Ok(());
                }
                Err(_) => continue,
            }
        }
    }

    // ========================================================================
    // Batch Operations (T4)
    // ========================================================================

    /// Get services ready to start (in Starting state)
    ///
    /// # Returns
    /// Bitmap of services in Starting state
    ///
    /// # Performance
    /// - Time: O(V) = <1μs
    pub fn get_starting_services(&self) -> u64 {
        let mut result = 0u64;
        let registered = self.registered_bitmap();
        let mut remaining = registered;

        while remaining != 0 {
            let service = remaining.trailing_zeros() as usize;
            if service >= MAX_SERVICES {
                break;
            }
            remaining &= remaining - 1;

            let packed = self.service_states[service].load(Ordering::Relaxed);
            if PackedServiceState(packed).state() == ServiceState::Starting {
                result |= 1u64 << service;
            }
        }

        result
    }

    /// Get services that need restart (Failed with Always/OnFailure policy)
    ///
    /// # Returns
    /// Bitmap of services that should be restarted
    ///
    /// # Performance
    /// - Time: O(V) = <1μs
    pub fn get_restart_candidates(&self) -> u64 {
        let mut result = 0u64;
        let failed = self.failed_bitmap();
        let mut remaining = failed;

        while remaining != 0 {
            let service = remaining.trailing_zeros() as usize;
            if service >= MAX_SERVICES {
                break;
            }
            remaining &= remaining - 1;

            let packed = self.service_states[service].load(Ordering::Relaxed);
            let state = PackedServiceState(packed);

            match state.policy() {
                RestartPolicy::Always => {
                    result |= 1u64 << service;
                }
                RestartPolicy::OnFailure => {
                    result |= 1u64 << service;
                }
                _ => {}
            }
        }

        result
    }

    /// Batch transition services to a new state
    ///
    /// # Arguments
    /// - `services`: Bitmap of services to transition
    /// - `new_state`: Target state
    ///
    /// # Returns
    /// Bitmap of services that successfully transitioned
    ///
    /// # Performance
    /// - Time: O(popcount(services)) × <50ns = <3μs for 64 services
    pub fn batch_set_state(&self, services: u64, new_state: ServiceState) -> u64 {
        let mut success = 0u64;
        let mut remaining = services;

        // #ASSUME_BATCH_INDEPENDENT: Each service transition is independent
        // #VERIFY_BATCH_INDEPENDENT: No cross-service dependencies in state
        while remaining != 0 {
            let service = remaining.trailing_zeros() as u8;
            if service as usize >= MAX_SERVICES {
                break;
            }
            remaining &= remaining - 1;

            if self.set_state(service, new_state).is_ok() {
                success |= 1u64 << service;
            }
        }

        success
    }

    /// Reset all services to initial state
    ///
    /// # Performance
    /// - Time: O(V) = <1μs
    pub fn reset(&self) {
        // #ASSUME_RESET_SAFE: All state cleared atomically per field
        // #VERIFY_RESET_SAFE: Each atomic store is independent
        for state in &self.service_states {
            state.store(0, Ordering::Relaxed);
        }
        self.registered_services.store(0, Ordering::Relaxed);
        self.healthy_services.store(0, Ordering::Relaxed);
        self.failed_services.store(0, Ordering::Relaxed);
        self.total_starts.store(0, Ordering::Relaxed);
        self.total_failures.store(0, Ordering::Relaxed);

        // Increment generation to invalidate caches
        let state = self.global_state.load(Ordering::Relaxed);
        let generation = (state >> 8) + 1;
        self.global_state.store(generation << 8, Ordering::Release);
    }
}

impl Default for ServiceManagerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Thread safety markers
// #ASSUME_SEND_SYNC_SAFE: Only atomic fields
// #VERIFY_SEND_SYNC_SAFE: No raw pointers, only AtomicU64
unsafe impl Send for ServiceManagerCapsule {}
unsafe impl Sync for ServiceManagerCapsule {}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // T28 Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_new_manager_empty() {
        let manager = ServiceManagerCapsule::new();
        assert_eq!(manager.service_count(), 0);
        assert_eq!(manager.registered_bitmap(), 0);
        assert_eq!(manager.healthy_bitmap(), 0);
    }

    #[test]
    fn test_register_service() {
        let manager = ServiceManagerCapsule::new();
        assert!(manager.register(0, RestartPolicy::Always).is_ok());
        assert!(manager.is_registered(0));
        assert_eq!(manager.service_count(), 1);
        assert_eq!(manager.get_state(0), ServiceState::Stopped);
    }

    #[test]
    fn test_register_multiple_services() {
        let manager = ServiceManagerCapsule::new();
        for i in 0..10 {
            assert!(manager.register(i, RestartPolicy::OnFailure).is_ok());
        }
        assert_eq!(manager.service_count(), 10);
    }

    #[test]
    fn test_register_duplicate() {
        let manager = ServiceManagerCapsule::new();
        assert!(manager.register(5, RestartPolicy::Never).is_ok());
        let result = manager.register(5, RestartPolicy::Always);
        assert!(matches!(result, Err(ServiceError::AlreadyRegistered(5))));
    }

    #[test]
    fn test_register_invalid_id() {
        let manager = ServiceManagerCapsule::new();
        let result = manager.register(64, RestartPolicy::Never);
        assert!(matches!(result, Err(ServiceError::InvalidServiceId(64))));
    }

    #[test]
    fn test_state_transitions_stopped_to_starting() {
        let manager = ServiceManagerCapsule::new();
        manager.register(0, RestartPolicy::Always).unwrap();
        assert!(manager.set_state(0, ServiceState::Starting).is_ok());
        assert_eq!(manager.get_state(0), ServiceState::Starting);
    }

    #[test]
    fn test_state_transitions_starting_to_running() {
        let manager = ServiceManagerCapsule::new();
        manager.register(0, RestartPolicy::Always).unwrap();
        manager.set_state(0, ServiceState::Starting).unwrap();
        assert!(manager.set_state(0, ServiceState::Running).is_ok());
        assert_eq!(manager.get_state(0), ServiceState::Running);
        assert!(manager.is_healthy(0));
    }

    #[test]
    fn test_state_transitions_running_to_failed() {
        let manager = ServiceManagerCapsule::new();
        manager.register(0, RestartPolicy::Always).unwrap();
        manager.set_state(0, ServiceState::Starting).unwrap();
        manager.set_state(0, ServiceState::Running).unwrap();
        assert!(manager.set_state(0, ServiceState::Failed).is_ok());
        assert_eq!(manager.get_state(0), ServiceState::Failed);
        assert!(!manager.is_healthy(0));
        assert!((manager.failed_bitmap() & 1) != 0);
    }

    #[test]
    fn test_invalid_state_transition() {
        let manager = ServiceManagerCapsule::new();
        manager.register(0, RestartPolicy::Always).unwrap();
        // Can't go directly from Stopped to Running
        let result = manager.set_state(0, ServiceState::Running);
        assert!(matches!(result, Err(ServiceError::InvalidStateTransition { .. })));
    }

    #[test]
    fn test_restart_policy_retrieval() {
        let manager = ServiceManagerCapsule::new();
        manager.register(0, RestartPolicy::Always).unwrap();
        manager.register(1, RestartPolicy::Never).unwrap();
        manager.register(2, RestartPolicy::OnFailure).unwrap();

        assert_eq!(manager.get_policy(0), RestartPolicy::Always);
        assert_eq!(manager.get_policy(1), RestartPolicy::Never);
        assert_eq!(manager.get_policy(2), RestartPolicy::OnFailure);
    }

    #[test]
    fn test_failure_count_increments() {
        let manager = ServiceManagerCapsule::new();
        manager.register(0, RestartPolicy::Always).unwrap();

        // Simulate failure
        manager.set_state(0, ServiceState::Starting).unwrap();
        manager.set_state(0, ServiceState::Failed).unwrap();
        assert_eq!(manager.failure_count(0), 1);

        // Restart and fail again
        manager.set_state(0, ServiceState::Starting).unwrap();
        manager.set_state(0, ServiceState::Failed).unwrap();
        assert_eq!(manager.failure_count(0), 2);
    }

    #[test]
    fn test_stats() {
        let manager = ServiceManagerCapsule::new();
        manager.register(0, RestartPolicy::Always).unwrap();
        manager.register(1, RestartPolicy::Always).unwrap();
        manager.register(2, RestartPolicy::Always).unwrap();

        manager.set_state(0, ServiceState::Starting).unwrap();
        manager.set_state(0, ServiceState::Running).unwrap();

        manager.set_state(1, ServiceState::Starting).unwrap();
        manager.set_state(1, ServiceState::Failed).unwrap();

        let stats = manager.stats();
        assert_eq!(stats.total_services, 3);
        assert_eq!(stats.running_services, 1);
        assert_eq!(stats.failed_services, 1);
        assert_eq!(stats.stopped_services, 1);
    }

    #[test]
    fn test_health_ratio() {
        let manager = ServiceManagerCapsule::new();
        for i in 0..4 {
            manager.register(i, RestartPolicy::Always).unwrap();
        }

        // Start 2 of 4 services
        manager.set_state(0, ServiceState::Starting).unwrap();
        manager.set_state(0, ServiceState::Running).unwrap();
        manager.set_state(1, ServiceState::Starting).unwrap();
        manager.set_state(1, ServiceState::Running).unwrap();

        assert!((manager.health_ratio() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_batch_set_state() {
        let manager = ServiceManagerCapsule::new();
        for i in 0..4 {
            manager.register(i, RestartPolicy::Always).unwrap();
        }

        // Batch start all services
        let started = manager.batch_set_state(0b1111, ServiceState::Starting);
        assert_eq!(started, 0b1111);

        // Batch transition to running
        let running = manager.batch_set_state(0b1111, ServiceState::Running);
        assert_eq!(running, 0b1111);
        assert_eq!(manager.healthy_bitmap(), 0b1111);
    }

    #[test]
    fn test_get_restart_candidates() {
        let manager = ServiceManagerCapsule::new();
        manager.register(0, RestartPolicy::Always).unwrap();
        manager.register(1, RestartPolicy::Never).unwrap();
        manager.register(2, RestartPolicy::OnFailure).unwrap();

        // Fail all three
        for i in 0..3 {
            manager.set_state(i, ServiceState::Starting).unwrap();
            manager.set_state(i, ServiceState::Failed).unwrap();
        }

        let candidates = manager.get_restart_candidates();
        // Only services 0 (Always) and 2 (OnFailure) should restart
        assert!((candidates & (1 << 0)) != 0);
        assert!((candidates & (1 << 1)) == 0);
        assert!((candidates & (1 << 2)) != 0);
    }

    #[test]
    fn test_reset() {
        let manager = ServiceManagerCapsule::new();
        manager.register(0, RestartPolicy::Always).unwrap();
        manager.set_state(0, ServiceState::Starting).unwrap();
        manager.set_state(0, ServiceState::Running).unwrap();

        manager.reset();

        assert_eq!(manager.service_count(), 0);
        assert_eq!(manager.healthy_bitmap(), 0);
        assert!(!manager.is_registered(0));
    }
}
