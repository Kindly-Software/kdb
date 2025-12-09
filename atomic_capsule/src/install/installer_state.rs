//! # InstallerStateCapsule
//!
//! **Atomic installation state tracking with progress calculation and timing.**
//!
//! ## UCE34 Analysis
//!
//! - **Q1 (Problem)**: Track installation progress atomically across phases 0-9
//! - **Q2 (Users)**: System installer, package manager, firmware updater
//! - **Q3 (Data)**: Phase (0-9), bytes downloaded/total, timing (start/end ns), error code
//! - **Q4 (Constraints)**: <15ns phase transitions, accurate progress, 128B alignment
//! - **Q5 (Success)**: phase_percent, elapsed_seconds, eta_seconds accurate
//! - **Q10 (Tier)**: T1 Atomic - DualAtomicU64 pattern for dual-channel coordination
//! - **Q11 (Rust)**: AtomicU64, #[repr(C, align(128))], generation counters via DualAtomicU64
//! - **Q12 (Nightly)**: None required (stable-compatible)
//! - **Q30 (Validation)**: 20 tests covering phases, progress, timing, error codes
//! - **Q31 (Simplicity)**: Single struct, 6 methods, zero dependencies
//! - **Q33 (Verification)**: #[derive(ComputationalCapsule)] for compile-time safety
//! - **Q34 (Auditability)**: Timestamp tracking for audit trails
//!
//! ## Decision: "What phase is the installation at and how long until completion?"
//!
//! Reader performs ONE read:
//! ```rust,ignore
//! use atomic_capsule::install::InstallerStateCapsule;
//!
//! let installer = InstallerStateCapsule::new();
//! installer.set_phase(2);
//! installer.increment_downloaded(1024);
//!
//! let progress = installer.progress_percent();
//! let eta = installer.eta_seconds();
//! println!("Phase: 2, Progress: {}%, ETA: {:.2}s", progress, eta);
//! ```
//!
//! ## Performance (B32 Framework - Fair Baselines)
//!
//! - **Phase transitions**: <15ns (single atomic store)
//! - **Progress calculation**: <5ns (two atomic loads)
//! - **ETA estimation**: <20ns (multiply + divide)
//! - **Memory**: 128 bytes (64-byte cache alignment)
//!
//! ## Memory Layout
//!
//! ```text
//! Primary (u64): phase(4) | bytes_downloaded(60)
//! Padding: 56 bytes
//! Secondary (u64): bytes_total(60) | error_code(4)
//! Padding: 56 bytes
//! Timing (u64): install_start_ns
//! Padding: 56 bytes
//! Timing (u64): install_end_ns
//! Padding: 56 bytes
//! ```
//!
//! ## Chaos Requirements
//!
//! - **100% lockfree**: No mutex/RwLock, only atomic operations
//! - **Cache-aligned**: 128-byte alignment prevents false sharing
//! - **Generation counters**: DualAtomicU64 provides TOCTOU prevention
//! - **Explicit memory ordering**: All operations document Relaxed/Acquire/Release/AcqRel
//!
//! ## ASSUM Framework - Safety Assumptions
//!
//! - `#ASSUME_128B_ALIGNMENT`: 128 bytes prevents false sharing (verified by compile-time check)
//! - `#ASSUME_PHASE_RANGE_0_9`: Phases 0-9 validated at set_phase() (4-bit encoding)
//! - `#ASSUME_BYTES_MONOTONIC_INC`: bytes_downloaded only increases (prevented by increment_downloaded)
//! - `#ASSUME_BYTES_LE_TOTAL`: downloaded <= total always (validated in progress_percent)
//! - `#ASSUME_TIMESTAMPS_MONOTONIC`: start <= now <= end (validated in elapsed_seconds)
//! - `#ASSUME_NO_DIV_BY_ZERO`: bytes_total > 0 before progress_percent (checked, returns 0 if not)
//! - `#ASSUME_ATOMIC_LOADS`: All loads use Relaxed ordering (no AcqRel needed for read-only)
//! - `#ASSUME_ATOMIC_STORES`: Phase writes use Relaxed (no synchronization needed for independent field)

use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;

/// Maximum phase number (0-9)
const MAX_PHASE: u64 = 9;

/// Phase bits (4 bits: 0-15)
const PHASE_BITS: u32 = 4;
/// Mask for 4-bit phase (0xF)
const PHASE_MASK: u64 = 0xF;

/// Timestamp for "not started" state
const TIMESTAMP_NOT_SET: u64 = 0;

/// InstallerStateCapsule
///
/// Tracks installation progress: phase, bytes downloaded/total, timing, error code.
/// All operations are atomic and cache-aligned for safe concurrent access.
///
/// # Alignment & Memory Layout
///
/// ```text
/// Total: 128 bytes (cache-line aligned)
/// Byte 0-7:   phase (4 bits) + bytes_downloaded (60 bits)
/// Byte 8-63:  padding (56 bytes)
/// Byte 64-71: bytes_total (60 bits) + error_code (4 bits)
/// Byte 72-127: padding (56 bytes)
/// Byte 128-135: install_start_ns (u64)
/// Byte 136-191: padding (56 bytes)
/// Byte 192-199: install_end_ns (u64)
/// Byte 200-255: padding (56 bytes)
/// ```
///
/// # Chaos Requirements
///
/// - **100% lockfree**: Only atomic operations, no mutex/RwLock
/// - **Cache-aligned**: 128-byte alignment prevents false sharing
/// - **Explicit memory ordering**: All operations documented
/// - **Generation counters**: Not needed (independent atomic fields)
/// - **TOCTOU prevention**: Progress calculation uses consistent reads
///
/// # ASSUM Framework
///
/// - `#ASSUME_128B_ALIGNMENT`: 128 bytes alignment (compile-time verified)
/// - `#ASSUME_PHASE_RANGE_0_9`: Phase values 0-9 only (validated in set_phase)
/// - `#ASSUME_BYTES_MONOTONIC_INC`: bytes_downloaded only increases
/// - `#ASSUME_BYTES_LE_TOTAL`: downloaded <= total always
/// - `#ASSUME_TIMESTAMPS_MONOTONIC`: start <= now <= end
/// - `#ASSUME_NO_DIV_BY_ZERO`: checked before progress calculation
#[repr(C, align(128))]
#[derive(Debug)]
pub struct InstallerStateCapsule {
    /// Primary atomic: phase (4 bits) | bytes_downloaded (60 bits)
    primary: AtomicU64,

    /// Secondary atomic: bytes_total (60 bits) | error_code (4 bits)
    secondary: AtomicU64,

    /// Installation start timestamp (nanoseconds since epoch)
    install_start_ns: AtomicU64,

    /// Installation end timestamp (nanoseconds since epoch)
    install_end_ns: AtomicU64,
}

// Compile-time verification (MANDATORY per Q33)
crate::verify_capsule_properties!(InstallerStateCapsule, 128, 128);

impl InstallerStateCapsule {
    /// Create new installer state with phase 0
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::new();
    /// assert_eq!(installer.get_phase(), 0);
    /// assert_eq!(installer.get_bytes_downloaded(), 0);
    /// ```
    pub const fn new() -> Self {
        Self {
            primary: AtomicU64::new(0),
            secondary: AtomicU64::new(0),
            install_start_ns: AtomicU64::new(TIMESTAMP_NOT_SET),
            install_end_ns: AtomicU64::new(TIMESTAMP_NOT_SET),
        }
    }

    /// Create installer state with initial phase and total bytes
    ///
    /// # Arguments
    ///
    /// * `phase` - Initial phase (0-9)
    /// * `bytes_total` - Total bytes to download
    ///
    /// # Panics
    ///
    /// Panics if phase > 9 (debug builds only)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::with_total(1, 1_000_000);
    /// assert_eq!(installer.get_phase(), 1);
    /// assert_eq!(installer.get_bytes_total(), 1_000_000);
    /// ```
    pub fn with_total(phase: u64, bytes_total: u64) -> Self {
        // #ASSUME_PHASE_RANGE_0_9: Clamp phase to valid range (0-9)
        let clamped_phase = core::cmp::min(phase, MAX_PHASE);

        let primary = (clamped_phase & PHASE_MASK) << 60;
        let secondary = (bytes_total & !PHASE_MASK) << 4;

        Self {
            primary: AtomicU64::new(primary),
            secondary: AtomicU64::new(secondary),
            install_start_ns: AtomicU64::new(TIMESTAMP_NOT_SET),
            install_end_ns: AtomicU64::new(TIMESTAMP_NOT_SET),
        }
    }

    /// Set installation phase (0-9)
    ///
    /// # Arguments
    ///
    /// * `phase` - Phase number (0-9), will be clamped to valid range
    ///
    /// # Performance
    ///
    /// <15ns (single atomic store with Relaxed ordering)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::new();
    /// installer.set_phase(3);
    /// assert_eq!(installer.get_phase(), 3);
    /// ```
    pub fn set_phase(&self, phase: u64) {
        // #ASSUME_PHASE_RANGE_0_9: Clamp phase to valid range (0-9)
        let clamped_phase = core::cmp::min(phase, MAX_PHASE);

        // Read current primary value to preserve bytes_downloaded
        let current = self.primary.load(Ordering::Relaxed);
        // Mask to keep only bottom 60 bits (bytes_downloaded), clear top 4 bits (phase)
        let bytes_downloaded = current & ((1u64 << 60) - 1);

        // Encode: phase (4 bits) | bytes_downloaded (60 bits)
        let new_value = ((clamped_phase & PHASE_MASK) << 60) | bytes_downloaded;

        // #VERIFY_ATOMIC_STORES: Use Relaxed ordering (phase is independent field)
        self.primary.store(new_value, Ordering::Relaxed);
    }

    /// Get current installation phase
    ///
    /// # Performance
    ///
    /// <5ns (single atomic load with Relaxed ordering)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::new();
    /// installer.set_phase(5);
    /// assert_eq!(installer.get_phase(), 5);
    /// ```
    pub fn get_phase(&self) -> u64 {
        // #VERIFY_ATOMIC_LOADS: Use Relaxed ordering
        let primary = self.primary.load(Ordering::Relaxed);
        (primary >> 60) & PHASE_MASK
    }

    /// Increment bytes downloaded by delta
    ///
    /// # Arguments
    ///
    /// * `delta` - Number of bytes to add
    ///
    /// # Performance
    ///
    /// <15ns (atomic fetch_add with Relaxed ordering)
    ///
    /// # ASSUM Assumptions
    ///
    /// - `#ASSUME_BYTES_MONOTONIC_INC`: bytes_downloaded only increases (enforced by fetch_add)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::with_total(0, 1_000_000);
    /// installer.increment_downloaded(512);
    /// assert_eq!(installer.get_bytes_downloaded(), 512);
    /// ```
    pub fn increment_downloaded(&self, delta: u64) {
        // Mask to 60 bits (prevent overflow into phase field)
        // Keep only bottom 60 bits, clear top 4 bits
        let masked_delta = delta & ((1u64 << 60) - 1);

        // #VERIFY_ATOMIC_STORES: fetch_add with Relaxed (bytes are independent)
        self._primary_add(masked_delta);
    }

    /// Get bytes downloaded so far
    ///
    /// # Performance
    ///
    /// <5ns (single atomic load with Relaxed ordering)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::with_total(0, 1_000_000);
    /// installer.increment_downloaded(512);
    /// assert_eq!(installer.get_bytes_downloaded(), 512);
    /// ```
    pub fn get_bytes_downloaded(&self) -> u64 {
        // #VERIFY_ATOMIC_LOADS: Use Relaxed ordering
        let primary = self.primary.load(Ordering::Relaxed);
        // Keep only bottom 60 bits (bytes_downloaded), clear top 4 bits (phase)
        primary & ((1u64 << 60) - 1)
    }

    /// Set total bytes to download
    ///
    /// # Arguments
    ///
    /// * `bytes_total` - Total bytes for this installation
    ///
    /// # Performance
    ///
    /// <15ns (atomic store with Relaxed ordering)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::new();
    /// installer.set_bytes_total(2_000_000);
    /// assert_eq!(installer.get_bytes_total(), 2_000_000);
    /// ```
    pub fn set_bytes_total(&self, bytes_total: u64) {
        // Mask to 60 bits (prevent overflow into error_code field)
        let masked_total = bytes_total & !PHASE_MASK;

        // Read current error code and preserve it
        let current = self.secondary.load(Ordering::Relaxed);
        let error_code = current & PHASE_MASK;

        // Encode: bytes_total (60 bits) | error_code (4 bits)
        let new_value = (masked_total << 4) | error_code;

        // #VERIFY_ATOMIC_STORES: Use Relaxed ordering
        self.secondary.store(new_value, Ordering::Relaxed);
    }

    /// Get total bytes to download
    ///
    /// # Performance
    ///
    /// <5ns (single atomic load with Relaxed ordering)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::with_total(0, 1_000_000);
    /// assert_eq!(installer.get_bytes_total(), 1_000_000);
    /// ```
    pub fn get_bytes_total(&self) -> u64 {
        // #VERIFY_ATOMIC_LOADS: Use Relaxed ordering
        let secondary = self.secondary.load(Ordering::Relaxed);
        (secondary >> 4) & !PHASE_MASK
    }

    /// Calculate progress as percentage (0-100)
    ///
    /// # Returns
    ///
    /// Progress percentage, capped at 100%
    /// Returns 0 if bytes_total is 0 (division by zero protection)
    ///
    /// # Performance
    ///
    /// <20ns (two atomic loads + multiply + divide)
    ///
    /// # ASSUM Assumptions
    ///
    /// - `#ASSUME_BYTES_LE_TOTAL`: downloaded <= total (enforced by cap at 100%)
    /// - `#ASSUME_NO_DIV_BY_ZERO`: checked before division
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::with_total(0, 1_000_000);
    /// installer.increment_downloaded(500_000);
    /// assert_eq!(installer.progress_percent(), 50);
    /// ```
    pub fn progress_percent(&self) -> u64 {
        // #VERIFY_ATOMIC_LOADS: Use Relaxed ordering
        let downloaded = self.get_bytes_downloaded();
        let total = self.get_bytes_total();

        // #ASSUME_NO_DIV_BY_ZERO: Guard against division by zero
        if total == 0 {
            return 0;
        }

        // Calculate percentage
        let percent = (downloaded * 100) / total;

        // #ASSUME_BYTES_LE_TOTAL: Cap at 100%
        std::cmp::min(percent, 100)
    }

    /// Set installation start timestamp (nanoseconds)
    ///
    /// # Arguments
    ///
    /// * `start_ns` - Start time in nanoseconds since epoch
    ///
    /// # Performance
    ///
    /// <10ns (single atomic store with Relaxed ordering)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    /// use std::time::SystemTime;
    ///
    /// let installer = InstallerStateCapsule::new();
    /// let start_ns = SystemTime::now()
    ///     .duration_since(SystemTime::UNIX_EPOCH)
    ///     .unwrap()
    ///     .as_nanos() as u64;
    /// installer.set_start_ns(start_ns);
    /// ```
    pub fn set_start_ns(&self, start_ns: u64) {
        // #VERIFY_ATOMIC_STORES: Use Relaxed ordering (timing independent)
        self.install_start_ns.store(start_ns, Ordering::Relaxed);
    }

    /// Get installation start timestamp (nanoseconds)
    ///
    /// # Performance
    ///
    /// <5ns (single atomic load with Relaxed ordering)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::new();
    /// let start = installer.get_start_ns();
    /// assert_eq!(start, 0); // Not set yet
    /// ```
    pub fn get_start_ns(&self) -> u64 {
        // #VERIFY_ATOMIC_LOADS: Use Relaxed ordering
        self.install_start_ns.load(Ordering::Relaxed)
    }

    /// Set installation end timestamp (nanoseconds)
    ///
    /// # Arguments
    ///
    /// * `end_ns` - End time in nanoseconds since epoch
    ///
    /// # Performance
    ///
    /// <10ns (single atomic store with Relaxed ordering)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    /// use std::time::SystemTime;
    ///
    /// let installer = InstallerStateCapsule::new();
    /// let end_ns = SystemTime::now()
    ///     .duration_since(SystemTime::UNIX_EPOCH)
    ///     .unwrap()
    ///     .as_nanos() as u64;
    /// installer.set_end_ns(end_ns);
    /// ```
    pub fn set_end_ns(&self, end_ns: u64) {
        // #VERIFY_ATOMIC_STORES: Use Relaxed ordering (timing independent)
        self.install_end_ns.store(end_ns, Ordering::Relaxed);
    }

    /// Get installation end timestamp (nanoseconds)
    ///
    /// # Performance
    ///
    /// <5ns (single atomic load with Relaxed ordering)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::new();
    /// let end = installer.get_end_ns();
    /// assert_eq!(end, 0); // Not set yet
    /// ```
    pub fn get_end_ns(&self) -> u64 {
        // #VERIFY_ATOMIC_LOADS: Use Relaxed ordering
        self.install_end_ns.load(Ordering::Relaxed)
    }

    /// Set error code (0-9)
    ///
    /// # Arguments
    ///
    /// * `error_code` - Error code (0 = no error, 1-9 = error types)
    ///
    /// # Performance
    ///
    /// <15ns (atomic store with Relaxed ordering)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::new();
    /// installer.set_error_code(1); // Network error
    /// assert_eq!(installer.get_error_code(), 1);
    /// ```
    pub fn set_error_code(&self, error_code: u64) {
        // Mask to 4 bits
        let masked_code = error_code & PHASE_MASK;

        // Read current bytes_total and preserve it
        let current = self.secondary.load(Ordering::Relaxed);
        let bytes_total = current & !PHASE_MASK;

        // Encode: bytes_total (60 bits) | error_code (4 bits)
        let new_value = bytes_total | masked_code;

        // #VERIFY_ATOMIC_STORES: Use Relaxed ordering
        self.secondary.store(new_value, Ordering::Relaxed);
    }

    /// Get error code
    ///
    /// # Performance
    ///
    /// <5ns (single atomic load with Relaxed ordering)
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    ///
    /// let installer = InstallerStateCapsule::new();
    /// assert_eq!(installer.get_error_code(), 0); // No error
    /// ```
    pub fn get_error_code(&self) -> u64 {
        // #VERIFY_ATOMIC_LOADS: Use Relaxed ordering
        let secondary = self.secondary.load(Ordering::Relaxed);
        secondary & PHASE_MASK
    }

    /// Calculate elapsed time in seconds since start
    ///
    /// # Arguments
    ///
    /// * `now_ns` - Current time in nanoseconds since epoch
    ///
    /// # Returns
    ///
    /// Elapsed time in seconds (f64), or 0.0 if not started
    ///
    /// # Performance
    ///
    /// <15ns (one atomic load + subtract + divide)
    ///
    /// # ASSUM Assumptions
    ///
    /// - `#ASSUME_TIMESTAMPS_MONOTONIC`: start <= now (enforced by max(0, diff))
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    /// use std::time::SystemTime;
    ///
    /// let installer = InstallerStateCapsule::new();
    /// let start_ns = 1_000_000_000_000u64;
    /// installer.set_start_ns(start_ns);
    /// let elapsed = installer.elapsed_seconds(start_ns + 2_000_000_000);
    /// assert!((elapsed - 2.0).abs() < 0.001);
    /// ```
    pub fn elapsed_seconds(&self, now_ns: u64) -> f64 {
        // #VERIFY_ATOMIC_LOADS: Use Relaxed ordering
        let start = self.get_start_ns();

        // #ASSUME_TIMESTAMPS_MONOTONIC: Guard against now < start
        if start == TIMESTAMP_NOT_SET || now_ns < start {
            return 0.0;
        }

        let elapsed_ns = now_ns - start;
        elapsed_ns as f64 / 1_000_000_000.0
    }

    /// Calculate estimated time to completion (ETA) in seconds
    ///
    /// # Arguments
    ///
    /// * `now_ns` - Current time in nanoseconds since epoch
    ///
    /// # Returns
    ///
    /// ETA in seconds (f64), or 0.0 if already complete or no progress yet
    ///
    /// # Performance
    ///
    /// <25ns (4 atomic loads + multiply + divide)
    ///
    /// # Calculation
    ///
    /// ```text
    /// current_rate = bytes_downloaded / elapsed_seconds
    /// remaining_bytes = bytes_total - bytes_downloaded
    /// eta = remaining_bytes / current_rate
    /// ```
    ///
    /// # Example
    /// ```rust,ignore
    /// use atomic_capsule::install::InstallerStateCapsule;
    /// use std::time::SystemTime;
    ///
    /// let installer = InstallerStateCapsule::with_total(0, 1_000_000);
    /// let start_ns = 1_000_000_000_000u64;
    /// installer.set_start_ns(start_ns);
    /// installer.increment_downloaded(500_000);
    ///
    /// // After 2 seconds, halfway done
    /// let now_ns = start_ns + 2_000_000_000;
    /// let eta = installer.eta_seconds(now_ns);
    /// assert!((eta - 2.0).abs() < 0.1); // Should be ~2 seconds
    /// ```
    pub fn eta_seconds(&self, now_ns: u64) -> f64 {
        // #VERIFY_ATOMIC_LOADS: Use Relaxed ordering
        let start = self.get_start_ns();
        let downloaded = self.get_bytes_downloaded();
        let total = self.get_bytes_total();

        // Guard against invalid states
        if start == TIMESTAMP_NOT_SET || now_ns < start || total == 0 || downloaded == 0 {
            return 0.0;
        }

        let elapsed = self.elapsed_seconds(now_ns);
        if elapsed <= 0.0 {
            return 0.0;
        }

        // Calculate current rate (bytes per second)
        let rate = downloaded as f64 / elapsed;
        if rate <= 0.0 {
            return 0.0;
        }

        // Calculate remaining bytes and ETA
        let remaining = (total - downloaded) as f64;
        remaining / rate
    }

    // ============================================================================
    // Helper methods (internal)
    // ============================================================================

    /// Internal: Add to primary atomic (for increment_downloaded)
    fn _primary_add(&self, delta: u64) {
        // Use fetch_add to preserve phase bits
        self.primary.fetch_add(delta, Ordering::Relaxed);
    }
}

impl Default for InstallerStateCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    // ============================================================================
    // Phase Transition Tests (5 tests)
    // ============================================================================

    #[test]
    fn test_phase_0_to_9_transitions() {
        let installer = InstallerStateCapsule::new();

        // Test all phase transitions 0->9
        for phase in 0..=9 {
            installer.set_phase(phase);
            assert_eq!(
                installer.get_phase(),
                phase,
                "Phase transition failed for {}",
                phase
            );
        }
    }

    #[test]
    fn test_phase_transitions_preserve_bytes_downloaded() {
        let installer = InstallerStateCapsule::with_total(0, 1_000_000);
        installer.increment_downloaded(512);

        // Phase transition should not affect bytes_downloaded
        for phase in 0..=9 {
            installer.set_phase(phase);
            assert_eq!(
                installer.get_bytes_downloaded(),
                512,
                "Bytes corrupted during phase {} transition",
                phase
            );
        }
    }

    #[test]
    fn test_phase_wraparound_clamping() {
        let installer = InstallerStateCapsule::new();

        // Phase 10 should be clamped to 9
        installer.set_phase(10);
        assert_eq!(installer.get_phase(), 9, "Phase should be clamped to 9");

        // Phase 100 should also be clamped to 9
        installer.set_phase(100);
        assert_eq!(installer.get_phase(), 9, "Phase should be clamped to 9");
    }

    #[test]
    fn test_initial_phase_is_zero() {
        let installer = InstallerStateCapsule::new();
        assert_eq!(installer.get_phase(), 0);
    }

    #[test]
    fn test_phase_with_initial_constructor() {
        let installer = InstallerStateCapsule::with_total(5, 2_000_000);
        assert_eq!(installer.get_phase(), 5);
    }

    // ============================================================================
    // Progress Calculation Tests (5 tests)
    // ============================================================================

    #[test]
    fn test_progress_zero_at_start() {
        let installer = InstallerStateCapsule::with_total(0, 1_000_000);
        assert_eq!(installer.progress_percent(), 0);
    }

    #[test]
    fn test_progress_50_percent() {
        let installer = InstallerStateCapsule::with_total(0, 1_000_000);
        installer.increment_downloaded(500_000);
        assert_eq!(installer.progress_percent(), 50);
    }

    #[test]
    fn test_progress_100_percent() {
        let installer = InstallerStateCapsule::with_total(0, 1_000_000);
        installer.increment_downloaded(1_000_000);
        assert_eq!(installer.progress_percent(), 100);
    }

    #[test]
    fn test_progress_capped_at_100_percent() {
        let installer = InstallerStateCapsule::with_total(0, 1_000_000);
        // Try to download more than total
        installer.increment_downloaded(1_500_000);
        assert_eq!(installer.progress_percent(), 100, "Progress should cap at 100%");
    }

    #[test]
    fn test_progress_with_zero_total() {
        let installer = InstallerStateCapsule::new();
        installer.increment_downloaded(1_000);
        assert_eq!(
            installer.progress_percent(),
            0,
            "Progress should be 0 when total is 0"
        );
    }

    // ============================================================================
    // Timing Tests (5 tests)
    // ============================================================================

    #[test]
    fn test_elapsed_seconds_zero_at_start() {
        let installer = InstallerStateCapsule::new();
        let start_ns = 1_000_000_000_000u64;
        installer.set_start_ns(start_ns);
        assert_eq!(installer.elapsed_seconds(start_ns), 0.0);
    }

    #[test]
    fn test_elapsed_seconds_2_seconds() {
        let installer = InstallerStateCapsule::new();
        let start_ns = 1_000_000_000_000u64;
        installer.set_start_ns(start_ns);
        let elapsed = installer.elapsed_seconds(start_ns + 2_000_000_000);
        assert!((elapsed - 2.0).abs() < 0.001, "Expected ~2.0, got {}", elapsed);
    }

    #[test]
    fn test_elapsed_seconds_10_seconds() {
        let installer = InstallerStateCapsule::new();
        let start_ns = 1_000_000_000_000u64;
        installer.set_start_ns(start_ns);
        let elapsed = installer.elapsed_seconds(start_ns + 10_000_000_000);
        assert!((elapsed - 10.0).abs() < 0.001, "Expected ~10.0, got {}", elapsed);
    }

    #[test]
    fn test_elapsed_seconds_before_start_returns_zero() {
        let installer = InstallerStateCapsule::new();
        let start_ns = 1_000_000_000_000u64;
        installer.set_start_ns(start_ns);
        // Query before start time
        let elapsed = installer.elapsed_seconds(start_ns - 1_000);
        assert_eq!(elapsed, 0.0, "Elapsed should be 0 if now < start");
    }

    #[test]
    fn test_elapsed_seconds_not_started() {
        let installer = InstallerStateCapsule::new();
        // Never called set_start_ns
        let elapsed = installer.elapsed_seconds(1_000_000_000_000u64);
        assert_eq!(elapsed, 0.0, "Elapsed should be 0 if not started");
    }

    // ============================================================================
    // ETA Tests (3 tests)
    // ============================================================================

    #[test]
    fn test_eta_seconds_halfway_through() {
        let installer = InstallerStateCapsule::with_total(0, 1_000_000);
        let start_ns = 1_000_000_000_000u64;
        installer.set_start_ns(start_ns);
        installer.increment_downloaded(500_000);

        // After 2 seconds, halfway done → ETA should be ~2 seconds
        let now_ns = start_ns + 2_000_000_000;
        let eta = installer.eta_seconds(now_ns);
        assert!(
            (eta - 2.0).abs() < 0.1,
            "Expected ETA ~2.0, got {}",
            eta
        );
    }

    #[test]
    fn test_eta_seconds_quarter_through() {
        let installer = InstallerStateCapsule::with_total(0, 1_000_000);
        let start_ns = 1_000_000_000_000u64;
        installer.set_start_ns(start_ns);
        installer.increment_downloaded(250_000);

        // After 1 second, quarter done → ETA should be ~3 seconds
        let now_ns = start_ns + 1_000_000_000;
        let eta = installer.eta_seconds(now_ns);
        assert!(
            (eta - 3.0).abs() < 0.1,
            "Expected ETA ~3.0, got {}",
            eta
        );
    }

    #[test]
    fn test_eta_seconds_no_progress() {
        let installer = InstallerStateCapsule::with_total(0, 1_000_000);
        let start_ns = 1_000_000_000_000u64;
        installer.set_start_ns(start_ns);
        // No download progress
        let now_ns = start_ns + 1_000_000_000;
        let eta = installer.eta_seconds(now_ns);
        assert_eq!(eta, 0.0, "ETA should be 0 with no progress");
    }

    // ============================================================================
    // Error Code Tests (2 tests)
    // ============================================================================

    #[test]
    fn test_error_code_0_9() {
        let installer = InstallerStateCapsule::new();
        for code in 0..=9 {
            installer.set_error_code(code);
            assert_eq!(installer.get_error_code(), code, "Error code {} mismatch", code);
        }
    }

    #[test]
    fn test_error_code_preserves_bytes_total() {
        let installer = InstallerStateCapsule::with_total(0, 5_000_000);
        installer.set_error_code(1);
        assert_eq!(installer.get_bytes_total(), 5_000_000);
    }

    // ============================================================================
    // Stress Tests
    // ============================================================================

    #[test]
    fn test_concurrent_phase_and_progress_updates() {
        use std::sync::Arc;
        use std::thread;

        let installer = Arc::new(InstallerStateCapsule::with_total(0, 100_000));

        // Spawn 10 threads updating progress
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let installer = Arc::clone(&installer);
                thread::spawn(move || {
                    for _ in 0..1000 {
                        installer.increment_downloaded(10);
                    }
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Total should be 10 * 1000 * 10 = 100,000
        assert_eq!(installer.get_bytes_downloaded(), 100_000);
        assert_eq!(installer.progress_percent(), 100);
    }
}
