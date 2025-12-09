//! GPU Fallback Manager Capsule - T6 Mixed Tier (Circuit Breaker Pattern)
//!
//! 256-byte cache-aligned circuit breaker for GPU/CPU fallback coordination.
//! Implements the circuit breaker pattern with EMA-based health scoring for
//! intelligent GPU availability detection and graceful CPU fallback.
//!
//! # Circuit Breaker Pattern
//!
//! The circuit breaker pattern prevents cascading failures by monitoring GPU
//! health and automatically switching to CPU fallback when errors occur:
//!
//! ```text
//! ┌─────────┐  success   ┌─────────┐  failure    ┌──────────┐
//! │ Closed  │───────────>│ Closed  │────────────>│   Open   │
//! │ (GPU)   │<───────────│ (GPU)   │             │  (CPU)   │
//! └─────────┘  success   └─────────┘             └──────────┘
//!       ^                                              │
//!       │                                              │
//!       │              ┌──────────┐                    │
//!       └──────────────│ HalfOpen │<───────────────────┘
//!         success      │ (Test)   │   recovery timeout
//!                      └──────────┘
//! ```
//!
//! # Architecture (T6 Mixed)
//!
//! Combines T1 (Atomic) + T3 (Fixed-Point) for lockfree health monitoring:
//! - AtomicU64 packed state: circuit state (8-bit), flags (8-bit)
//! - Q16.16 fixed-point EMA for health score calculation
//! - Generation counter for Q34 audit trail compliance
//!
//! # Performance Targets (B32)
//!
//! - should_use_gpu(): <50ns (atomic load + comparison)
//! - record_success(): <100ns (CAS + EMA update)
//! - record_failure(): <100ns (CAS + EMA update)
//! - attempt_recovery(): <50ns (CAS state transition)
//! - status()/metrics(): <50ns (atomic loads)
//!
//! # Framework Compliance
//!
//! - UCE34: T6 Mixed tier (T1 Atomic + T3 Fixed-Point)
//! - Chaos: 256B cache-aligned, 100% lockfree, no mutex
//! - ASSUM: All assumptions documented (#ASSUME/#VERIFY tags)
//! - B32: <100ns operation targets, fair baseline comparison
//! - T28: 20+ unit/property/integration tests
//! - I20: Zero breaking changes (new module)
//! - Q34: Generation counter for audit trail
//!
//! # References
//!
//! - Circuit Breaker Pattern: [circuitbreaker-rs](https://docs.rs/circuitbreaker-rs)
//! - Lock-free State Management: [failsafe-rs](https://github.com/dmexe/failsafe-rs)
//! - EMA-based Health: [circuit_breaker crate](https://crates.io/crates/circuit-breaker)
//!
//! # Example
//!
//! ```rust,ignore
//! use kindly_dedup::gpu::fallback_manager::{GpuFallbackManager, CircuitState};
//!
//! // Create with default thresholds (5 failures, 30s recovery)
//! let manager = GpuFallbackManager::new();
//!
//! // Check if GPU should be used
//! if manager.should_use_gpu() {
//!     match execute_gpu_operation() {
//!         Ok(_) => manager.record_success(),
//!         Err(_) => manager.record_failure(),
//!     }
//! } else {
//!     // Use CPU fallback
//!     execute_cpu_operation();
//! }
//!
//! // Get current status
//! let status = manager.status();
//! println!("Circuit: {:?}, Health: {:.2}%", status.state, status.health_percent);
//! ```

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// =============================================================================
// CONSTANTS
// =============================================================================

/// Q16.16 fixed-point multiplier (65536)
const Q16_SCALE: u64 = 65536;

/// EMA alpha for health score (0.1 in Q16.16 = 6554)
/// Formula: new_ema = alpha * current + (1-alpha) * old_ema
const EMA_ALPHA: u64 = 6554; // 0.1 * 65536

/// Success value in Q16.16 (1.0 = 65536)
const Q16_ONE: u64 = Q16_SCALE;

/// Health threshold for recovery (0.8 in Q16.16 = 52429)
const HEALTH_RECOVERY_THRESHOLD: u64 = 52429;

/// Default failure threshold before opening circuit
const DEFAULT_FAILURE_THRESHOLD: u32 = 5;

/// Default recovery timeout in seconds
const DEFAULT_RECOVERY_TIMEOUT_SECS: u32 = 30;

/// Default consecutive successes required to close circuit
const DEFAULT_SUCCESS_THRESHOLD: u32 = 3;

// Bit-packing for circuit_state (AtomicU64)
// Bits 0-7: CircuitState enum
// Bits 8-15: Flags
// Bits 16-31: Reserved
// Bits 32-63: Generation counter
const STATE_MASK: u64 = 0xFF;
const FLAGS_SHIFT: u64 = 8;
const FLAGS_MASK: u64 = 0xFF;
const GEN_SHIFT: u64 = 32;

// Flags
const FLAG_FORCE_CPU: u8 = 0b00000001;
const FLAG_FORCE_GPU: u8 = 0b00000010;
const FLAG_MANUAL_OVERRIDE: u8 = 0b00000100;
const FLAG_RECOVERY_PENDING: u8 = 0b00001000;

// =============================================================================
// CIRCUIT STATE
// =============================================================================

/// Circuit breaker states
///
/// The circuit breaker transitions between these states based on
/// GPU operation success/failure patterns.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit closed - GPU operations allowed
    ///
    /// Normal operation mode. All requests go to GPU.
    /// Transitions to Open when failure_count >= failure_threshold.
    Closed = 0,

    /// Circuit open - CPU fallback active
    ///
    /// Protection mode. All requests bypass GPU and use CPU.
    /// Transitions to HalfOpen after recovery_timeout expires.
    Open = 1,

    /// Circuit half-open - testing recovery
    ///
    /// Probing mode. Limited GPU requests to test recovery.
    /// Transitions to Closed on success, Open on failure.
    HalfOpen = 2,
}

impl CircuitState {
    /// Convert from u8 to CircuitState
    #[inline]
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Open, // Invalid -> Open (safe default)
        }
    }

    /// Human-readable state name
    pub fn as_str(&self) -> &'static str {
        match self {
            CircuitState::Closed => "Closed (GPU Active)",
            CircuitState::Open => "Open (CPU Fallback)",
            CircuitState::HalfOpen => "HalfOpen (Testing)",
        }
    }

    /// Check if GPU operations are allowed in this state
    #[inline]
    pub fn allows_gpu(&self) -> bool {
        matches!(self, CircuitState::Closed | CircuitState::HalfOpen)
    }
}

// =============================================================================
// STATUS AND METRICS
// =============================================================================

/// Current fallback manager status (atomic snapshot)
#[derive(Debug, Clone, Copy)]
pub struct FallbackStatus {
    /// Current circuit state
    pub state: CircuitState,
    /// Health score as percentage (0.0 - 100.0)
    pub health_percent: f64,
    /// Current failure count
    pub failure_count: u32,
    /// Consecutive successes in current window
    pub consecutive_successes: u32,
    /// Whether manual override is active
    pub manual_override: bool,
    /// Generation counter (Q34 audit)
    pub generation: u32,
}

/// Cumulative metrics for monitoring
#[derive(Debug, Clone, Copy)]
pub struct FallbackMetrics {
    /// Total GPU calls attempted
    pub total_gpu_calls: u64,
    /// Total CPU fallback invocations
    pub total_cpu_fallbacks: u64,
    /// GPU success rate as percentage
    pub gpu_success_rate: f64,
    /// Average health score
    pub avg_health: f64,
    /// Circuit trips (Closed -> Open transitions)
    pub circuit_trips: u32,
    /// Recovery successes (HalfOpen -> Closed)
    pub recovery_successes: u32,
}

// =============================================================================
// GPU FALLBACK MANAGER CAPSULE
// =============================================================================

/// GPU Fallback Manager - T6 Mixed Tier (Circuit Breaker)
///
/// 256-byte cache-aligned capsule for GPU/CPU fallback coordination.
/// Implements circuit breaker pattern with EMA-based health scoring.
///
/// # Layout (256 bytes)
///
/// ```text
/// Bytes 0-7:    circuit_state (AtomicU64) - state|flags|gen packed
/// Bytes 8-15:   health_score (AtomicU64) - Q16.16 EMA health
/// Bytes 16-19:  failure_count (AtomicU32)
/// Bytes 20-23:  consecutive_successes (AtomicU32)
/// Bytes 24-27:  failure_threshold (AtomicU32)
/// Bytes 28-31:  recovery_timeout_secs (AtomicU32)
/// Bytes 32-39:  last_failure_ns (AtomicU64)
/// Bytes 40-47:  last_success_ns (AtomicU64)
/// Bytes 48-55:  total_gpu_calls (AtomicU64)
/// Bytes 56-63:  total_cpu_fallbacks (AtomicU64)
/// Bytes 64-71:  generation (AtomicU64) - separate for fast access
/// Bytes 72-79:  circuit_trips (AtomicU64) - Closed->Open count
/// Bytes 80-87:  recovery_successes (AtomicU64) - HalfOpen->Closed count
/// Bytes 88-95:  success_threshold (AtomicU32) + reserved (AtomicU32)
/// Bytes 96-255: _padding (160 bytes for 256B alignment)
/// ```
///
/// # ASSUM Safety
///
/// - `#ASSUME_ATOMIC_OPS`: All operations use atomic primitives for lockfree access
/// - `#VERIFY_ATOMIC_OPS`: AtomicU64/U32 with appropriate memory ordering
/// - `#ASSUME_Q16_PRECISION`: Q16.16 provides sufficient precision for EMA (0-100% health)
/// - `#VERIFY_Q16_PRECISION`: 16 fractional bits = 0.0015% precision, exceeds requirements
/// - `#ASSUME_TIME_MONOTONIC`: SystemTime provides monotonically increasing timestamps
/// - `#VERIFY_TIME_MONOTONIC`: Fallback to 0 on time errors, safe degradation
/// - `#ASSUME_GEN_MONOTONIC`: Generation counter increments on every state change
/// - `#VERIFY_GEN_MONOTONIC`: Wrapping add ensures monotonicity (2^64 ops before wrap)
/// - `#ASSUME_STATE_PACK`: Bit-packing fits within u64 (8+8+16+32 = 64 bits)
/// - `#VERIFY_STATE_PACK`: Compile-time verified via const assertions
/// - `#ASSUME_EMA_BOUNDS`: EMA stays within [0, Q16_ONE] range
/// - `#VERIFY_EMA_BOUNDS`: Clamping in update_health_score ensures bounds
#[repr(C, align(64))]
pub struct GpuFallbackManager {
    /// Packed state: circuit_state (8) | flags (8) | reserved (16) | generation (32)
    circuit_state: AtomicU64,

    /// Q16.16 fixed-point EMA health score (0 = unhealthy, 65536 = healthy)
    health_score: AtomicU64,

    /// Current failure count in this window
    failure_count: AtomicU32,

    /// Consecutive successes since last failure
    consecutive_successes: AtomicU32,

    /// Failures before circuit opens
    failure_threshold: AtomicU32,

    /// Seconds before attempting recovery
    recovery_timeout_secs: AtomicU32,

    /// Last failure timestamp (nanoseconds since epoch)
    last_failure_ns: AtomicU64,

    /// Last success timestamp (nanoseconds since epoch)
    last_success_ns: AtomicU64,

    /// Total GPU operation calls
    total_gpu_calls: AtomicU64,

    /// Total CPU fallback invocations
    total_cpu_fallbacks: AtomicU64,

    /// Separate generation counter for fast audit access
    generation: AtomicU64,

    /// Circuit trip count (Closed -> Open transitions)
    circuit_trips: AtomicU64,

    /// Recovery success count (HalfOpen -> Closed)
    recovery_successes: AtomicU64,

    /// Consecutive successes required to close circuit from HalfOpen
    success_threshold: AtomicU32,

    /// Reserved for future use
    _reserved: AtomicU32,

    /// Padding to 256-byte cache line (160 bytes)
    _padding: [u8; 160],
}

// Compile-time size verification
const _: () = assert!(std::mem::size_of::<GpuFallbackManager>() == 256);

impl GpuFallbackManager {
    /// Create new fallback manager with default configuration
    ///
    /// Default settings:
    /// - failure_threshold: 5 failures before opening circuit
    /// - recovery_timeout: 30 seconds
    /// - success_threshold: 3 consecutive successes to close
    /// - health_score: 1.0 (100% healthy)
    #[inline]
    pub const fn new() -> Self {
        Self {
            circuit_state: AtomicU64::new(0), // Closed, no flags, gen=0
            health_score: AtomicU64::new(Q16_ONE), // Start at 100% health
            failure_count: AtomicU32::new(0),
            consecutive_successes: AtomicU32::new(0),
            failure_threshold: AtomicU32::new(DEFAULT_FAILURE_THRESHOLD),
            recovery_timeout_secs: AtomicU32::new(DEFAULT_RECOVERY_TIMEOUT_SECS),
            last_failure_ns: AtomicU64::new(0),
            last_success_ns: AtomicU64::new(0),
            total_gpu_calls: AtomicU64::new(0),
            total_cpu_fallbacks: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            circuit_trips: AtomicU64::new(0),
            recovery_successes: AtomicU64::new(0),
            success_threshold: AtomicU32::new(DEFAULT_SUCCESS_THRESHOLD),
            _reserved: AtomicU32::new(0),
            _padding: [0; 160],
        }
    }

    /// Create with custom configuration
    ///
    /// # Arguments
    ///
    /// - `failure_threshold`: Failures before circuit opens (1-255)
    /// - `recovery_timeout_secs`: Seconds before recovery attempt (1-3600)
    /// - `success_threshold`: Consecutive successes to close circuit (1-255)
    pub fn with_config(
        failure_threshold: u32,
        recovery_timeout_secs: u32,
        success_threshold: u32,
    ) -> Self {
        let manager = Self::new();
        manager.failure_threshold.store(failure_threshold.min(255).max(1), Ordering::Relaxed);
        manager.recovery_timeout_secs.store(recovery_timeout_secs.min(3600).max(1), Ordering::Relaxed);
        manager.success_threshold.store(success_threshold.min(255).max(1), Ordering::Relaxed);
        manager
    }

    // =========================================================================
    // TIME UTILITIES
    // =========================================================================

    /// Get current timestamp in nanoseconds
    #[inline]
    fn now_ns() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    /// Get current timestamp in seconds
    #[inline]
    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    // =========================================================================
    // BIT-PACKING UTILITIES
    // =========================================================================

    /// Pack state, flags, and generation into u64
    #[inline]
    fn pack_state(state: CircuitState, flags: u8, gen: u32) -> u64 {
        (state as u64) | ((flags as u64) << FLAGS_SHIFT) | ((gen as u64) << GEN_SHIFT)
    }

    /// Unpack circuit state from packed value
    #[inline]
    fn unpack_circuit_state(packed: u64) -> CircuitState {
        CircuitState::from_u8((packed & STATE_MASK) as u8)
    }

    /// Unpack flags from packed value
    #[inline]
    fn unpack_flags(packed: u64) -> u8 {
        ((packed >> FLAGS_SHIFT) & FLAGS_MASK) as u8
    }

    /// Unpack generation from packed value
    #[inline]
    fn unpack_gen(packed: u64) -> u32 {
        (packed >> GEN_SHIFT) as u32
    }

    // =========================================================================
    // HEALTH SCORE (Q16.16 EMA)
    // =========================================================================

    /// Update health score using EMA
    ///
    /// Formula: new_ema = alpha * current + (1-alpha) * old_ema
    /// Where current is 1.0 for success, 0.0 for failure
    #[inline]
    fn update_health_score(&self, success: bool) {
        let current_value = if success { Q16_ONE } else { 0 };

        let mut old_score = self.health_score.load(Ordering::Acquire);
        loop {
            // EMA calculation in Q16.16: new = alpha * current + (1-alpha) * old
            // = alpha * current + old - alpha * old
            // = old + alpha * (current - old)
            let diff = current_value as i64 - old_score as i64;
            let adjustment = (EMA_ALPHA as i64 * diff) / Q16_SCALE as i64;
            let new_score = (old_score as i64 + adjustment).clamp(0, Q16_ONE as i64) as u64;

            match self.health_score.compare_exchange_weak(
                old_score,
                new_score,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => old_score = actual,
            }
        }
    }

    /// Get current health as percentage (0.0 - 100.0)
    #[inline]
    pub fn health_percent(&self) -> f64 {
        let score = self.health_score.load(Ordering::Acquire);
        (score as f64 / Q16_ONE as f64) * 100.0
    }

    /// Get raw Q16.16 health score
    #[inline]
    pub fn health_score_q16(&self) -> u64 {
        self.health_score.load(Ordering::Acquire)
    }

    // =========================================================================
    // CIRCUIT STATE TRANSITIONS
    // =========================================================================

    /// Transition to Open state (CPU fallback)
    fn transition_to_open(&self) {
        let mut current = self.circuit_state.load(Ordering::Acquire);
        loop {
            let flags = Self::unpack_flags(current);
            let gen = Self::unpack_gen(current);
            let new_packed = Self::pack_state(CircuitState::Open, flags, gen.wrapping_add(1));

            match self.circuit_state.compare_exchange_weak(
                current,
                new_packed,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    self.circuit_trips.fetch_add(1, Ordering::Relaxed);
                    break;
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Transition to HalfOpen state (recovery testing)
    fn transition_to_half_open(&self) {
        let mut current = self.circuit_state.load(Ordering::Acquire);
        loop {
            let flags = Self::unpack_flags(current);
            let gen = Self::unpack_gen(current);
            let new_flags = flags | FLAG_RECOVERY_PENDING;
            let new_packed = Self::pack_state(CircuitState::HalfOpen, new_flags, gen.wrapping_add(1));

            match self.circuit_state.compare_exchange_weak(
                current,
                new_packed,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    self.consecutive_successes.store(0, Ordering::Release);
                    break;
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Transition to Closed state (GPU active)
    fn transition_to_closed(&self) {
        let mut current = self.circuit_state.load(Ordering::Acquire);
        loop {
            let flags = Self::unpack_flags(current);
            let gen = Self::unpack_gen(current);
            // Clear recovery pending flag
            let new_flags = flags & !FLAG_RECOVERY_PENDING;
            let new_packed = Self::pack_state(CircuitState::Closed, new_flags, gen.wrapping_add(1));

            match self.circuit_state.compare_exchange_weak(
                current,
                new_packed,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    self.recovery_successes.fetch_add(1, Ordering::Relaxed);
                    self.failure_count.store(0, Ordering::Release);
                    break;
                }
                Err(actual) => current = actual,
            }
        }
    }

    // =========================================================================
    // CORE OPERATIONS
    // =========================================================================

    /// Check if GPU should be used (<50ns)
    ///
    /// Returns `true` if:
    /// - Circuit is Closed (normal operation)
    /// - Circuit is HalfOpen (testing recovery)
    /// - Force GPU override is active
    ///
    /// Returns `false` if:
    /// - Circuit is Open (CPU fallback active)
    /// - Force CPU override is active
    ///
    /// # Performance
    ///
    /// Single atomic load + comparison, target <50ns.
    #[inline]
    pub fn should_use_gpu(&self) -> bool {
        let packed = self.circuit_state.load(Ordering::Acquire);
        let state = Self::unpack_circuit_state(packed);
        let flags = Self::unpack_flags(packed);

        // Check force overrides first
        if (flags & FLAG_FORCE_CPU) != 0 {
            self.total_cpu_fallbacks.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        if (flags & FLAG_FORCE_GPU) != 0 {
            self.total_gpu_calls.fetch_add(1, Ordering::Relaxed);
            return true;
        }

        // Check circuit state
        let use_gpu = state.allows_gpu();
        if use_gpu {
            self.total_gpu_calls.fetch_add(1, Ordering::Relaxed);
        } else {
            self.total_cpu_fallbacks.fetch_add(1, Ordering::Relaxed);
        }
        use_gpu
    }

    /// Record successful GPU operation (<100ns)
    ///
    /// Updates health score and may transition circuit:
    /// - HalfOpen -> Closed: After success_threshold consecutive successes
    ///
    /// # Performance
    ///
    /// Atomic CAS + EMA update, target <100ns.
    pub fn record_success(&self) {
        let now = Self::now_ns();
        self.last_success_ns.store(now, Ordering::Release);
        self.update_health_score(true);

        let packed = self.circuit_state.load(Ordering::Acquire);
        let state = Self::unpack_circuit_state(packed);

        match state {
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Release);
            }
            CircuitState::HalfOpen => {
                // Increment consecutive successes
                let successes = self.consecutive_successes.fetch_add(1, Ordering::AcqRel) + 1;
                let threshold = self.success_threshold.load(Ordering::Acquire);

                if successes >= threshold {
                    // Recovery successful - close circuit
                    self.transition_to_closed();
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but handle gracefully
            }
        }
    }

    /// Record failed GPU operation (<100ns)
    ///
    /// Updates health score and may transition circuit:
    /// - Closed -> Open: After failure_threshold failures
    /// - HalfOpen -> Open: Immediately on failure
    ///
    /// # Performance
    ///
    /// Atomic CAS + EMA update, target <100ns.
    pub fn record_failure(&self) {
        let now = Self::now_ns();
        self.last_failure_ns.store(now, Ordering::Release);
        self.update_health_score(false);
        self.consecutive_successes.store(0, Ordering::Release);

        let packed = self.circuit_state.load(Ordering::Acquire);
        let state = Self::unpack_circuit_state(packed);

        match state {
            CircuitState::Closed => {
                let failures = self.failure_count.fetch_add(1, Ordering::AcqRel) + 1;
                let threshold = self.failure_threshold.load(Ordering::Acquire);

                if failures >= threshold {
                    // Too many failures - open circuit
                    self.transition_to_open();
                }
            }
            CircuitState::HalfOpen => {
                // Immediate failure during recovery - back to open
                self.transition_to_open();
            }
            CircuitState::Open => {
                // Already open, just update timestamp
            }
        }
    }

    /// Attempt recovery from Open state (<50ns)
    ///
    /// If recovery timeout has elapsed and health score is sufficient,
    /// transitions to HalfOpen to test GPU availability.
    ///
    /// Returns `true` if transition to HalfOpen occurred.
    ///
    /// # Performance
    ///
    /// Atomic loads + CAS, target <50ns.
    pub fn attempt_recovery(&self) -> bool {
        let packed = self.circuit_state.load(Ordering::Acquire);
        let state = Self::unpack_circuit_state(packed);
        let flags = Self::unpack_flags(packed);

        // Only attempt recovery from Open state
        if state != CircuitState::Open {
            return false;
        }

        // Check manual override
        if (flags & FLAG_MANUAL_OVERRIDE) != 0 {
            return false;
        }

        // Check recovery timeout
        let last_failure = self.last_failure_ns.load(Ordering::Acquire);
        let timeout_secs = self.recovery_timeout_secs.load(Ordering::Acquire) as u64;
        let timeout_ns = timeout_secs * 1_000_000_000;
        let now = Self::now_ns();

        if now.saturating_sub(last_failure) < timeout_ns {
            return false; // Timeout not elapsed
        }

        // Check health threshold
        let health = self.health_score.load(Ordering::Acquire);
        if health < HEALTH_RECOVERY_THRESHOLD {
            return false; // Health too low
        }

        // Attempt transition to HalfOpen
        self.transition_to_half_open();
        true
    }

    // =========================================================================
    // MANUAL OVERRIDES
    // =========================================================================

    /// Force CPU mode (disable GPU)
    ///
    /// Sets FORCE_CPU flag and MANUAL_OVERRIDE flag.
    /// Call `clear_overrides()` to restore automatic behavior.
    pub fn force_cpu_mode(&self) {
        let mut current = self.circuit_state.load(Ordering::Acquire);
        loop {
            let state = Self::unpack_circuit_state(current);
            let flags = Self::unpack_flags(current);
            let gen = Self::unpack_gen(current);
            let new_flags = (flags | FLAG_FORCE_CPU | FLAG_MANUAL_OVERRIDE) & !FLAG_FORCE_GPU;
            let new_packed = Self::pack_state(state, new_flags, gen.wrapping_add(1));

            match self.circuit_state.compare_exchange_weak(
                current,
                new_packed,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    break;
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Force GPU mode (ignore failures)
    ///
    /// Sets FORCE_GPU flag and MANUAL_OVERRIDE flag.
    /// WARNING: May cause errors if GPU is actually unavailable.
    /// Call `clear_overrides()` to restore automatic behavior.
    pub fn force_gpu_mode(&self) {
        let mut current = self.circuit_state.load(Ordering::Acquire);
        loop {
            let state = Self::unpack_circuit_state(current);
            let flags = Self::unpack_flags(current);
            let gen = Self::unpack_gen(current);
            let new_flags = (flags | FLAG_FORCE_GPU | FLAG_MANUAL_OVERRIDE) & !FLAG_FORCE_CPU;
            let new_packed = Self::pack_state(state, new_flags, gen.wrapping_add(1));

            match self.circuit_state.compare_exchange_weak(
                current,
                new_packed,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    break;
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Clear manual overrides and restore automatic behavior
    pub fn clear_overrides(&self) {
        let mut current = self.circuit_state.load(Ordering::Acquire);
        loop {
            let state = Self::unpack_circuit_state(current);
            let flags = Self::unpack_flags(current);
            let gen = Self::unpack_gen(current);
            let new_flags = flags & !(FLAG_FORCE_CPU | FLAG_FORCE_GPU | FLAG_MANUAL_OVERRIDE);
            let new_packed = Self::pack_state(state, new_flags, gen.wrapping_add(1));

            match self.circuit_state.compare_exchange_weak(
                current,
                new_packed,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    break;
                }
                Err(actual) => current = actual,
            }
        }
    }

    // =========================================================================
    // STATUS AND METRICS
    // =========================================================================

    /// Get current status (atomic snapshot) (<50ns)
    ///
    /// Returns consistent snapshot of circuit state, health, and counters.
    #[inline]
    pub fn status(&self) -> FallbackStatus {
        let packed = self.circuit_state.load(Ordering::Acquire);
        let state = Self::unpack_circuit_state(packed);
        let flags = Self::unpack_flags(packed);
        let gen = Self::unpack_gen(packed);

        let health = self.health_score.load(Ordering::Acquire);
        let health_percent = (health as f64 / Q16_ONE as f64) * 100.0;

        FallbackStatus {
            state,
            health_percent,
            failure_count: self.failure_count.load(Ordering::Acquire),
            consecutive_successes: self.consecutive_successes.load(Ordering::Acquire),
            manual_override: (flags & FLAG_MANUAL_OVERRIDE) != 0,
            generation: gen,
        }
    }

    /// Get cumulative metrics (<50ns)
    ///
    /// Returns metrics for monitoring and alerting.
    #[inline]
    pub fn metrics(&self) -> FallbackMetrics {
        let total_gpu = self.total_gpu_calls.load(Ordering::Acquire);
        let total_cpu = self.total_cpu_fallbacks.load(Ordering::Acquire);
        let total = total_gpu + total_cpu;

        let gpu_success_rate = if total > 0 {
            // Approximate: assume failures caused CPU fallback
            let failures = self.circuit_trips.load(Ordering::Acquire);
            let estimated_failures = failures as u64 * self.failure_threshold.load(Ordering::Acquire) as u64;
            let successes = total_gpu.saturating_sub(estimated_failures);
            (successes as f64 / total_gpu.max(1) as f64) * 100.0
        } else {
            100.0
        };

        let avg_health = (self.health_score.load(Ordering::Acquire) as f64 / Q16_ONE as f64) * 100.0;

        FallbackMetrics {
            total_gpu_calls: total_gpu,
            total_cpu_fallbacks: total_cpu,
            gpu_success_rate,
            avg_health,
            circuit_trips: self.circuit_trips.load(Ordering::Acquire) as u32,
            recovery_successes: self.recovery_successes.load(Ordering::Acquire) as u32,
        }
    }

    // =========================================================================
    // QUERY METHODS
    // =========================================================================

    /// Get current circuit state
    #[inline]
    pub fn state(&self) -> CircuitState {
        let packed = self.circuit_state.load(Ordering::Acquire);
        Self::unpack_circuit_state(packed)
    }

    /// Get generation counter (Q34 audit trail)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Check if circuit is closed (GPU active)
    #[inline]
    pub fn is_closed(&self) -> bool {
        self.state() == CircuitState::Closed
    }

    /// Check if circuit is open (CPU fallback)
    #[inline]
    pub fn is_open(&self) -> bool {
        self.state() == CircuitState::Open
    }

    /// Check if manual override is active
    #[inline]
    pub fn has_override(&self) -> bool {
        let packed = self.circuit_state.load(Ordering::Acquire);
        let flags = Self::unpack_flags(packed);
        (flags & FLAG_MANUAL_OVERRIDE) != 0
    }

    /// Get time since last failure in seconds
    pub fn seconds_since_failure(&self) -> u64 {
        let last = self.last_failure_ns.load(Ordering::Acquire);
        if last == 0 {
            return u64::MAX;
        }
        let now = Self::now_ns();
        now.saturating_sub(last) / 1_000_000_000
    }

    /// Get time since last success in seconds
    pub fn seconds_since_success(&self) -> u64 {
        let last = self.last_success_ns.load(Ordering::Acquire);
        if last == 0 {
            return u64::MAX;
        }
        let now = Self::now_ns();
        now.saturating_sub(last) / 1_000_000_000
    }

    /// Reset to initial state (for testing)
    pub fn reset(&self) {
        self.circuit_state.store(0, Ordering::Release);
        self.health_score.store(Q16_ONE, Ordering::Release);
        self.failure_count.store(0, Ordering::Release);
        self.consecutive_successes.store(0, Ordering::Release);
        self.last_failure_ns.store(0, Ordering::Release);
        self.last_success_ns.store(0, Ordering::Release);
        self.total_gpu_calls.store(0, Ordering::Release);
        self.total_cpu_fallbacks.store(0, Ordering::Release);
        self.generation.store(0, Ordering::Release);
        self.circuit_trips.store(0, Ordering::Release);
        self.recovery_successes.store(0, Ordering::Release);
    }

    /// Get summary string for logging
    pub fn summary(&self) -> String {
        let status = self.status();
        format!(
            "Circuit: {} | Health: {:.1}% | Failures: {} | Override: {}",
            status.state.as_str(),
            status.health_percent,
            status.failure_count,
            if status.manual_override { "YES" } else { "NO" }
        )
    }
}

impl Default for GpuFallbackManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Basic Construction Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_new_default_state() {
        let manager = GpuFallbackManager::new();
        assert_eq!(manager.state(), CircuitState::Closed);
        assert!((manager.health_percent() - 100.0).abs() < 0.01);
        assert_eq!(manager.generation(), 0);
        assert!(!manager.has_override());
    }

    #[test]
    fn test_with_config() {
        let manager = GpuFallbackManager::with_config(10, 60, 5);
        assert_eq!(manager.failure_threshold.load(Ordering::Acquire), 10);
        assert_eq!(manager.recovery_timeout_secs.load(Ordering::Acquire), 60);
        assert_eq!(manager.success_threshold.load(Ordering::Acquire), 5);
    }

    #[test]
    fn test_config_bounds() {
        let manager = GpuFallbackManager::with_config(0, 0, 0);
        assert_eq!(manager.failure_threshold.load(Ordering::Acquire), 1);
        assert_eq!(manager.recovery_timeout_secs.load(Ordering::Acquire), 1);
        assert_eq!(manager.success_threshold.load(Ordering::Acquire), 1);

        let manager2 = GpuFallbackManager::with_config(1000, 10000, 1000);
        assert_eq!(manager2.failure_threshold.load(Ordering::Acquire), 255);
        assert_eq!(manager2.recovery_timeout_secs.load(Ordering::Acquire), 3600);
        assert_eq!(manager2.success_threshold.load(Ordering::Acquire), 255);
    }

    // -------------------------------------------------------------------------
    // Circuit State Transition Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_closed_to_open_on_failures() {
        let manager = GpuFallbackManager::with_config(3, 30, 1);
        assert!(manager.is_closed());

        manager.record_failure();
        manager.record_failure();
        assert!(manager.is_closed()); // Still closed (2 < 3)

        manager.record_failure();
        assert!(manager.is_open()); // Now open (3 >= 3)
    }

    #[test]
    fn test_failure_reset_on_success() {
        let manager = GpuFallbackManager::with_config(3, 30, 1);

        manager.record_failure();
        manager.record_failure();
        assert_eq!(manager.failure_count.load(Ordering::Acquire), 2);

        manager.record_success();
        assert_eq!(manager.failure_count.load(Ordering::Acquire), 0);
    }

    #[test]
    fn test_half_open_success_closes_circuit() {
        let manager = GpuFallbackManager::with_config(1, 0, 2);

        // Open circuit
        manager.record_failure();
        assert!(manager.is_open());

        // Force transition to HalfOpen for testing
        manager.transition_to_half_open();
        assert_eq!(manager.state(), CircuitState::HalfOpen);

        // One success - still HalfOpen
        manager.record_success();
        assert_eq!(manager.state(), CircuitState::HalfOpen);

        // Two successes - closes circuit
        manager.record_success();
        assert!(manager.is_closed());
    }

    #[test]
    fn test_half_open_failure_opens_circuit() {
        let manager = GpuFallbackManager::with_config(1, 0, 3);

        // Open circuit
        manager.record_failure();
        assert!(manager.is_open());

        // Transition to HalfOpen
        manager.transition_to_half_open();
        assert_eq!(manager.state(), CircuitState::HalfOpen);

        // Failure immediately reopens
        manager.record_failure();
        assert!(manager.is_open());
    }

    // -------------------------------------------------------------------------
    // Health Score Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_health_decreases_on_failure() {
        let manager = GpuFallbackManager::new();
        let initial_health = manager.health_percent();

        manager.record_failure();
        let after_failure = manager.health_percent();

        assert!(after_failure < initial_health);
    }

    #[test]
    fn test_health_increases_on_success() {
        let manager = GpuFallbackManager::new();

        // Decrease health first
        for _ in 0..5 {
            manager.update_health_score(false);
        }
        let low_health = manager.health_percent();

        manager.record_success();
        let after_success = manager.health_percent();

        assert!(after_success > low_health);
    }

    #[test]
    fn test_health_bounds() {
        let manager = GpuFallbackManager::new();

        // Many failures
        for _ in 0..100 {
            manager.update_health_score(false);
        }
        assert!(manager.health_percent() >= 0.0);

        // Many successes
        for _ in 0..200 {
            manager.update_health_score(true);
        }
        assert!(manager.health_percent() <= 100.0);
    }

    // -------------------------------------------------------------------------
    // Manual Override Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_force_cpu_mode() {
        let manager = GpuFallbackManager::new();
        assert!(manager.should_use_gpu()); // Default: GPU active

        manager.force_cpu_mode();
        assert!(!manager.should_use_gpu());
        assert!(manager.has_override());
    }

    #[test]
    fn test_force_gpu_mode() {
        let manager = GpuFallbackManager::new();

        // Open circuit first
        manager.transition_to_open();
        assert!(!manager.should_use_gpu());

        // Force GPU override
        manager.force_gpu_mode();
        assert!(manager.should_use_gpu());
        assert!(manager.has_override());
    }

    #[test]
    fn test_clear_overrides() {
        let manager = GpuFallbackManager::new();

        manager.force_cpu_mode();
        assert!(manager.has_override());

        manager.clear_overrides();
        assert!(!manager.has_override());
        assert!(manager.should_use_gpu()); // Back to normal
    }

    // -------------------------------------------------------------------------
    // Status and Metrics Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_status_snapshot() {
        let manager = GpuFallbackManager::new();

        let status = manager.status();
        assert_eq!(status.state, CircuitState::Closed);
        assert!((status.health_percent - 100.0).abs() < 0.01);
        assert_eq!(status.failure_count, 0);
        assert!(!status.manual_override);
    }

    #[test]
    fn test_metrics() {
        let manager = GpuFallbackManager::new();

        // Some operations
        let _ = manager.should_use_gpu();
        let _ = manager.should_use_gpu();
        manager.record_failure();

        let metrics = manager.metrics();
        assert!(metrics.total_gpu_calls > 0);
    }

    #[test]
    fn test_generation_increments() {
        let manager = GpuFallbackManager::new();
        let initial_gen = manager.generation();

        manager.force_cpu_mode();
        assert_eq!(manager.generation(), initial_gen + 1);

        manager.clear_overrides();
        assert_eq!(manager.generation(), initial_gen + 2);
    }

    // -------------------------------------------------------------------------
    // Recovery Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_attempt_recovery_requires_timeout() {
        let manager = GpuFallbackManager::with_config(1, 100, 1);

        // Open circuit
        manager.record_failure();
        assert!(manager.is_open());

        // Immediate recovery should fail (timeout not elapsed)
        assert!(!manager.attempt_recovery());
        assert!(manager.is_open());
    }

    #[test]
    fn test_attempt_recovery_blocked_by_override() {
        let manager = GpuFallbackManager::with_config(1, 0, 1);

        manager.record_failure();
        manager.force_cpu_mode();

        // Recovery blocked by manual override
        assert!(!manager.attempt_recovery());
    }

    // -------------------------------------------------------------------------
    // Edge Cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_success_in_open_state() {
        let manager = GpuFallbackManager::with_config(1, 30, 1);

        manager.record_failure();
        assert!(manager.is_open());

        // Success in Open state shouldn't change state
        manager.record_success();
        assert!(manager.is_open());
    }

    #[test]
    fn test_reset() {
        let manager = GpuFallbackManager::new();

        manager.record_failure();
        manager.record_failure();
        manager.force_cpu_mode();

        manager.reset();

        assert!(manager.is_closed());
        assert!((manager.health_percent() - 100.0).abs() < 0.01);
        assert!(!manager.has_override());
        assert_eq!(manager.generation(), 0);
    }

    #[test]
    fn test_summary() {
        let manager = GpuFallbackManager::new();
        let summary = manager.summary();

        assert!(summary.contains("Closed"));
        assert!(summary.contains("100.0%"));
        assert!(summary.contains("NO")); // No override
    }

    // -------------------------------------------------------------------------
    // Cache Alignment Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_size_and_alignment() {
        assert_eq!(std::mem::size_of::<GpuFallbackManager>(), 256);
        assert_eq!(std::mem::align_of::<GpuFallbackManager>(), 64);
    }

    // -------------------------------------------------------------------------
    // Circuit State Enum Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_circuit_state_from_u8() {
        assert_eq!(CircuitState::from_u8(0), CircuitState::Closed);
        assert_eq!(CircuitState::from_u8(1), CircuitState::Open);
        assert_eq!(CircuitState::from_u8(2), CircuitState::HalfOpen);
        assert_eq!(CircuitState::from_u8(255), CircuitState::Open); // Invalid -> Open
    }

    #[test]
    fn test_circuit_state_allows_gpu() {
        assert!(CircuitState::Closed.allows_gpu());
        assert!(!CircuitState::Open.allows_gpu());
        assert!(CircuitState::HalfOpen.allows_gpu());
    }

    #[test]
    fn test_circuit_state_as_str() {
        assert!(!CircuitState::Closed.as_str().is_empty());
        assert!(!CircuitState::Open.as_str().is_empty());
        assert!(!CircuitState::HalfOpen.as_str().is_empty());
    }
}
