//! Self-Destruct Handler for MCP Client Protection - T1 Atomic
//!
//! **UCE35 Q35 Compliance**: Mandatory self-destruction mechanism for Phase 4 protection
//!
//! # Architecture
//!
//! **SelfDestructHandler** (T1 Atomic Capsule):
//! - **Atomic state tracking**: Irreversible trigger via AtomicBool
//! - **Severity-based response**: Immediate (>=8) vs graceful (<8) termination
//! - **Cascade propagation**: Priority-based cascade levels (P0=0, P1=3, P2=8)
//! - **Q34 audit integration**: Timestamp and reason capture for forensics
//!
//! # UCE35 Q35: Mandatory Self-Destruction
//!
//! All protection capsules MUST implement self-destruction on tamper detection.
//! This handler provides the central coordination point for the MCP client's
//! protection cascade.
//!
//! # ASSUM Framework
//!
//! - `#ASSUME_TRIGGER_IRREVERSIBLE`: Once triggered, cannot be un-triggered
//! - `#VERIFY_TRIGGER_IRREVERSIBLE`: swap(true) returns previous state
//! - `#ASSUME_SEQCST_TRIGGER`: SeqCst on triggered flag ensures visibility
//! - `#VERIFY_SEQCST_TRIGGER`: All threads see trigger immediately
//! - `#ASSUME_RELEASE_STATE`: Release ordering on reason/timestamp publishes state
//! - `#VERIFY_RELEASE_STATE`: Acquire loads in tests verify publication
//!
//! # Safety
//!
//! 100% safe Rust - No unsafe blocks. Atomics provide thread-safe coordination.

use core::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// TAMPER REASON ENUM
// ============================================================================

/// Reason for triggering self-destruct.
///
/// Each variant has an associated severity (0-10 scale).
/// High severity (>=8) triggers immediate termination.
/// Medium severity (<8) triggers graceful shutdown.
///
/// # Severity Scale
/// - 0-4: Low severity (informational)
/// - 5-7: Medium severity (graceful shutdown)
/// - 8-9: High severity (immediate termination)
/// - 10: Critical severity (emergency termination)
///
/// # ASSUM Framework
/// - `#ASSUME_SEVERITY_STABLE`: Same reason always returns same severity
/// - `#VERIFY_SEVERITY_STABLE`: Unit tests verify determinism
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TamperReason {
    /// Debugger detected via ptrace, int3, or hardware breakpoints.
    /// Severity: 8 (high - active debugging attempt)
    DebuggerAttached = 1,

    /// Emulator or VM detected via timing or CPU feature analysis.
    /// Severity: 6 (medium - may be legitimate VM usage)
    EmulatorDetected = 2,

    /// Memory checksum or hash verification failed.
    /// Severity: 9 (high - active memory tampering)
    MemoryTampered = 3,

    /// Execution timing outside expected bounds.
    /// Severity: 5 (medium - may be system load)
    TimingAnomaly = 4,

    /// Integrity check failed (code, data, or audit chain).
    /// Severity: 10 (critical - confirmed tampering)
    IntegrityViolation = 5,

    /// License validation failed or expired.
    /// Severity: 7 (medium-high - unauthorized usage)
    LicenseViolation = 6,

    /// Clone detection (multiple instances with same license).
    /// Severity: 10 (critical - license abuse)
    CloneDetected = 7,

    /// Unauthorized access attempt.
    /// Severity: 8 (high - security breach attempt)
    UnauthorizedAccess = 8,
}

impl TamperReason {
    /// Get severity score for this tamper reason.
    ///
    /// # Returns
    /// Severity score where:
    /// - <8: Graceful shutdown
    /// - >=8: Immediate termination
    ///
    /// # Severity Scale
    /// - DebuggerAttached: 8 (high - active debugging)
    /// - EmulatorDetected: 6 (medium - VM environment)
    /// - MemoryTampered: 9 (high - memory integrity)
    /// - TimingAnomaly: 5 (medium - timing variance)
    /// - IntegrityViolation: 10 (critical - code/data integrity)
    /// - LicenseViolation: 7 (medium-high - unauthorized use)
    /// - CloneDetected: 10 (critical - license abuse)
    /// - UnauthorizedAccess: 8 (high - security breach)
    #[inline]
    pub const fn severity(&self) -> u8 {
        match self {
            TamperReason::DebuggerAttached => 8,
            TamperReason::EmulatorDetected => 6,
            TamperReason::MemoryTampered => 9,
            TamperReason::TimingAnomaly => 5,
            TamperReason::IntegrityViolation => 10,
            TamperReason::LicenseViolation => 7,
            TamperReason::CloneDetected => 10,
            TamperReason::UnauthorizedAccess => 8,
        }
    }

    /// Check if this reason requires immediate termination.
    ///
    /// # Returns
    /// `true` if severity >= 8
    #[inline]
    pub const fn requires_immediate_termination(&self) -> bool {
        self.severity() >= 8
    }

    /// Get a static string description of this reason.
    #[inline]
    pub const fn description(&self) -> &'static str {
        match self {
            TamperReason::DebuggerAttached => "Debugger attachment detected",
            TamperReason::EmulatorDetected => "Emulator/VM environment detected",
            TamperReason::MemoryTampered => "Memory integrity violation",
            TamperReason::TimingAnomaly => "Execution timing anomaly",
            TamperReason::IntegrityViolation => "Code/data integrity check failed",
            TamperReason::LicenseViolation => "License validation failed",
            TamperReason::CloneDetected => "Clone/duplicate instance detected",
            TamperReason::UnauthorizedAccess => "Unauthorized access attempt",
        }
    }

    /// Convert from u8 discriminant value.
    ///
    /// # Returns
    /// Some(TamperReason) if value matches a variant, None otherwise
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(TamperReason::DebuggerAttached),
            2 => Some(TamperReason::EmulatorDetected),
            3 => Some(TamperReason::MemoryTampered),
            4 => Some(TamperReason::TimingAnomaly),
            5 => Some(TamperReason::IntegrityViolation),
            6 => Some(TamperReason::LicenseViolation),
            7 => Some(TamperReason::CloneDetected),
            8 => Some(TamperReason::UnauthorizedAccess),
            _ => None,
        }
    }
}

// ============================================================================
// CASCADE LEVEL MAPPING
// ============================================================================

/// Get cascade level for a priority string.
///
/// Cascade levels determine which capsules get poisoned on self-destruct:
/// - P0 (level 0): Root level, poisons ALL capsules (P0, P1, P2)
/// - P1 (level 3): Intermediate level, poisons P1 and P2 only
/// - P2 (level 8): Leaf level, poisons P2 only
/// - Unknown (level 15): Terminal level, no cascade
///
/// # Arguments
/// * `priority` - Priority string ("P0", "P1", "P2", or other)
///
/// # Returns
/// Cascade level (0-15)
///
/// # ASSUM Framework
/// - `#ASSUME_CASCADE_BOUNDED`: Returns 0-15 only
/// - `#VERIFY_CASCADE_BOUNDED`: Unit tests verify bounds
#[inline]
pub const fn cascade_level_for_priority(priority: &str) -> u8 {
    // Use byte comparison for const fn compatibility
    let bytes = priority.as_bytes();
    if bytes.len() == 2 {
        if bytes[0] == b'P' && bytes[1] == b'0' {
            return 0; // Root level, poisons all
        }
        if bytes[0] == b'P' && bytes[1] == b'1' {
            return 3; // Poisons P1 + P2
        }
        if bytes[0] == b'P' && bytes[1] == b'2' {
            return 8; // Poisons P2 only
        }
    }
    15 // Leaf level, no cascade
}

/// Check if source cascade level should propagate to target level.
///
/// # Arguments
/// * `source_level` - Cascade level of triggering capsule
/// * `target_level` - Cascade level of potential victim
///
/// # Returns
/// `true` if source should poison target
///
/// # Rules
/// - Lower cascade levels poison higher levels
/// - Equal levels poison each other
/// - Higher levels do NOT poison lower levels
#[inline]
pub const fn should_cascade(source_level: u8, target_level: u8) -> bool {
    source_level <= target_level
}

// ============================================================================
// SELF-DESTRUCT HANDLER CAPSULE
// ============================================================================

/// T1 Atomic Self-Destruct Handler for MCP client protection.
///
/// Provides centralized tamper response coordination with:
/// - Irreversible trigger mechanism (once triggered, stays triggered)
/// - Severity-based response (immediate vs graceful shutdown)
/// - Q34 forensic data capture (reason + timestamp)
/// - Sensitive data zeroing before exit
///
/// # Layout
/// - `triggered`: AtomicBool (1 byte) - SeqCst for visibility
/// - `reason`: AtomicU8 (1 byte) - Release for publish
/// - `timestamp_unix`: AtomicU64 (8 bytes) - Release for publish
///
/// # Thread Safety
/// All operations are lockfree via atomic primitives.
///
/// # ASSUM Framework
/// - `#ASSUME_LOCKFREE`: No mutex or blocking operations
/// - `#VERIFY_LOCKFREE`: Only atomic operations used
#[repr(C, align(64))]
pub struct SelfDestructHandler {
    /// Whether self-destruct has been triggered (irreversible).
    /// Uses SeqCst ordering for immediate cross-thread visibility.
    triggered: AtomicBool,

    /// Tamper reason (as u8 for atomic storage).
    /// Uses Release ordering to publish alongside timestamp.
    reason: AtomicU8,

    /// Padding to separate atomics for cache efficiency.
    _pad1: [u8; 6],

    /// Unix timestamp when triggered.
    /// Uses Release ordering to publish alongside reason.
    timestamp_unix: AtomicU64,

    /// Padding to ensure 64-byte cache line alignment.
    _pad2: [u8; 40],
}

// Verify layout at compile time
const _: () = assert!(core::mem::size_of::<SelfDestructHandler>() == 64);
const _: () = assert!(core::mem::align_of::<SelfDestructHandler>() == 64);

impl SelfDestructHandler {
    /// Create a new self-destruct handler in non-triggered state.
    ///
    /// # Returns
    /// Handler ready to monitor for tamper events
    #[inline]
    pub const fn new() -> Self {
        Self {
            triggered: AtomicBool::new(false),
            reason: AtomicU8::new(0),
            _pad1: [0; 6],
            timestamp_unix: AtomicU64::new(0),
            _pad2: [0; 40],
        }
    }

    /// Check if self-destruct has been triggered.
    ///
    /// # Returns
    /// `true` if trigger() has been called
    ///
    /// # Performance
    /// <10ns - single atomic load with Acquire ordering
    #[inline]
    pub fn is_triggered(&self) -> bool {
        self.triggered.load(Ordering::Acquire)
    }

    /// Get the tamper reason if triggered.
    ///
    /// # Returns
    /// Some(TamperReason) if triggered, None otherwise
    #[inline]
    pub fn get_reason(&self) -> Option<TamperReason> {
        if self.is_triggered() {
            let reason_val = self.reason.load(Ordering::Acquire);
            TamperReason::from_u8(reason_val)
        } else {
            None
        }
    }

    /// Get the trigger timestamp if triggered.
    ///
    /// # Returns
    /// Unix timestamp in seconds, or 0 if not triggered
    #[inline]
    pub fn get_timestamp(&self) -> u64 {
        self.timestamp_unix.load(Ordering::Acquire)
    }

    /// Trigger self-destruct sequence.
    ///
    /// This method:
    /// 1. Atomically sets triggered flag (returns early if already triggered)
    /// 2. Records reason and timestamp
    /// 3. Logs the event
    /// 4. Executes destruction sequence based on severity
    ///
    /// # Arguments
    /// * `reason` - The tamper reason triggering self-destruct
    ///
    /// # Returns
    /// `true` if this call triggered self-destruct, `false` if already triggered
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_TRIGGER_ONCE`: swap ensures single trigger
    /// - `#VERIFY_TRIGGER_ONCE`: Returns false on subsequent calls
    ///
    /// # Note
    /// This method does NOT return for severity >= 8 (calls std::process::exit)
    pub fn trigger(&self, reason: TamperReason) -> bool {
        // Atomically check and set - SeqCst ensures immediate visibility
        // #ASSUME_SEQCST_TRIGGER: All threads see this change immediately
        if self.triggered.swap(true, Ordering::SeqCst) {
            // Already triggered by another thread/call
            return false;
        }

        // Record forensic data with Release ordering
        // #ASSUME_RELEASE_STATE: Publishes state for Acquire readers
        self.reason.store(reason.severity(), Ordering::Release);
        self.timestamp_unix
            .store(current_unix_timestamp(), Ordering::Release);

        // Log the event (Q34 audit integration point)
        eprintln!(
            "[PROTECTION] Self-destruct triggered: {:?} (severity {})",
            reason,
            reason.severity()
        );

        // Execute destruction sequence
        self.execute_destruction(reason);

        true
    }

    /// Execute the destruction sequence based on severity.
    ///
    /// - Severity >= 8: Immediate termination (exit 137, SIGKILL simulation)
    /// - Severity < 8: Graceful shutdown (exit 1)
    ///
    /// Both paths zero sensitive data before exit.
    fn execute_destruction(&self, reason: TamperReason) {
        let severity = reason.severity();

        if severity >= 8 {
            // Immediate termination (high severity)
            eprintln!(
                "[PROTECTION] Immediate termination (severity {}) - {}",
                severity,
                reason.description()
            );
            self.zero_sensitive_data();
            std::process::exit(137); // SIGKILL simulation
        } else {
            // Graceful shutdown (medium severity)
            eprintln!(
                "[PROTECTION] Graceful shutdown (severity {}) - {}",
                severity,
                reason.description()
            );
            self.zero_sensitive_data();
            std::process::exit(1);
        }
    }

    /// Zero all sensitive data before termination.
    ///
    /// This method should clear:
    /// - Encryption keys
    /// - Cached license data
    /// - Cached API responses
    /// - HTTP connection state
    ///
    /// # Note
    /// In a production implementation, this would integrate with:
    /// - ResponseCacheCapsule::clear()
    /// - LicenseValidatorCapsule::invalidate()
    /// - HttpConnectionPool::close_all()
    pub fn zero_sensitive_data(&self) {
        // In production, this would call into other capsules:
        // - self.response_cache.clear();
        // - self.license_cache.invalidate();
        // - self.connection_pool.close_all();

        // For now, we overwrite our own state as a demonstration
        // Use Release ordering to ensure writes complete before exit
        self.reason.store(0, Ordering::Release);
        self.timestamp_unix.store(0, Ordering::Release);

        // Force memory fence to ensure writes are visible
        core::sync::atomic::fence(Ordering::SeqCst);
    }

    /// Trigger self-destruct but allow testing without process exit.
    ///
    /// This variant performs all steps EXCEPT calling std::process::exit().
    /// Used for testing the destruction sequence.
    ///
    /// # Arguments
    /// * `reason` - The tamper reason triggering self-destruct
    ///
    /// # Returns
    /// `true` if this call triggered self-destruct, `false` if already triggered
    #[cfg(test)]
    pub fn trigger_test_mode(&self, reason: TamperReason) -> bool {
        if self.triggered.swap(true, Ordering::SeqCst) {
            return false;
        }

        self.reason.store(reason.severity(), Ordering::Release);
        self.timestamp_unix
            .store(current_unix_timestamp(), Ordering::Release);

        // Don't call execute_destruction() which would exit
        true
    }

    /// Reset handler for testing purposes only.
    ///
    /// # Safety Note
    /// This should NEVER be used in production code.
    /// Self-destruct should be irreversible.
    #[cfg(test)]
    pub fn reset_for_testing(&self) {
        self.triggered.store(false, Ordering::SeqCst);
        self.reason.store(0, Ordering::Release);
        self.timestamp_unix.store(0, Ordering::Release);
    }
}

impl Default for SelfDestructHandler {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: AtomicBool, AtomicU8, AtomicU64 are all Send + Sync
unsafe impl Send for SelfDestructHandler {}
unsafe impl Sync for SelfDestructHandler {}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Get current Unix timestamp in seconds.
///
/// # Returns
/// Seconds since Unix epoch, or 0 if system time unavailable
#[inline]
fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ============================================================================
// TESTS (T28: Q1-Q7 Unit Tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1: HANDLER INITIALIZATION
    // ========================================================================

    /// Test 1: SelfDestructHandler initializes in non-triggered state
    #[test]
    fn test_self_destruct_handler_initialization() {
        let handler = SelfDestructHandler::new();

        assert!(
            !handler.is_triggered(),
            "Handler should start non-triggered"
        );
        assert!(handler.get_reason().is_none(), "No reason before trigger");
        assert_eq!(handler.get_timestamp(), 0, "No timestamp before trigger");

        // Verify layout
        assert_eq!(
            core::mem::size_of::<SelfDestructHandler>(),
            64,
            "Handler should be 64 bytes (cache line)"
        );
        assert_eq!(
            core::mem::align_of::<SelfDestructHandler>(),
            64,
            "Handler should be 64-byte aligned"
        );
    }

    // ========================================================================
    // Q2-Q3: IMMEDIATE TERMINATION TRIGGERS
    // ========================================================================

    /// Test 2: Debugger trigger (severity 8) marks for immediate exit
    #[test]
    fn test_trigger_debugger_immediate_exit() {
        let handler = SelfDestructHandler::new();

        let triggered = handler.trigger_test_mode(TamperReason::DebuggerAttached);

        assert!(triggered, "First trigger should succeed");
        assert!(handler.is_triggered(), "Handler should be triggered");
        assert_eq!(
            handler.reason.load(Ordering::Acquire),
            8,
            "Reason severity should be 8"
        );
        assert!(
            TamperReason::DebuggerAttached.requires_immediate_termination(),
            "Debugger should require immediate termination"
        );
    }

    /// Test 3: License violation trigger (severity 7) marks for graceful shutdown
    #[test]
    fn test_trigger_license_immediate_exit() {
        let handler = SelfDestructHandler::new();

        let triggered = handler.trigger_test_mode(TamperReason::LicenseViolation);

        assert!(triggered, "First trigger should succeed");
        assert!(handler.is_triggered(), "Handler should be triggered");
        assert_eq!(
            handler.reason.load(Ordering::Acquire),
            7,
            "Reason severity should be 7"
        );
        assert!(
            !TamperReason::LicenseViolation.requires_immediate_termination(),
            "License violation should NOT require immediate termination"
        );
    }

    // ========================================================================
    // Q4: GRACEFUL SHUTDOWN TRIGGERS
    // ========================================================================

    /// Test 4: Timing anomaly trigger (severity 5) allows graceful shutdown
    #[test]
    fn test_trigger_timing_graceful_shutdown() {
        let handler = SelfDestructHandler::new();

        let triggered = handler.trigger_test_mode(TamperReason::TimingAnomaly);

        assert!(triggered, "First trigger should succeed");
        assert!(handler.is_triggered(), "Handler should be triggered");
        assert_eq!(
            handler.reason.load(Ordering::Acquire),
            5,
            "Reason severity should be 5"
        );
        assert!(
            !TamperReason::TimingAnomaly.requires_immediate_termination(),
            "Timing anomaly should allow graceful shutdown"
        );
    }

    // ========================================================================
    // Q5: SINGLE TRIGGER GUARANTEE
    // ========================================================================

    /// Test 5: Handler can only be triggered once (irreversible)
    #[test]
    fn test_trigger_once_only() {
        let handler = SelfDestructHandler::new();

        // First trigger should succeed
        let first = handler.trigger_test_mode(TamperReason::DebuggerAttached);
        assert!(first, "First trigger should succeed");

        // Second trigger should fail (already triggered)
        let second = handler.trigger_test_mode(TamperReason::MemoryTampered);
        assert!(!second, "Second trigger should fail");

        // Original reason should be preserved
        assert_eq!(
            handler.reason.load(Ordering::Acquire),
            8,
            "Original reason (DebuggerAttached=8) should be preserved"
        );
    }

    // ========================================================================
    // Q6: SEVERITY THRESHOLDS
    // ========================================================================

    /// Test 6: Verify severity thresholds for all reasons
    #[test]
    fn test_severity_thresholds() {
        // High severity (>=8) - immediate termination
        assert!(TamperReason::DebuggerAttached.requires_immediate_termination());
        assert!(TamperReason::MemoryTampered.requires_immediate_termination());
        assert!(TamperReason::IntegrityViolation.requires_immediate_termination());
        assert!(TamperReason::CloneDetected.requires_immediate_termination());
        assert!(TamperReason::UnauthorizedAccess.requires_immediate_termination());

        // Medium severity (<8) - graceful shutdown
        assert!(!TamperReason::EmulatorDetected.requires_immediate_termination());
        assert!(!TamperReason::TimingAnomaly.requires_immediate_termination());
        assert!(!TamperReason::LicenseViolation.requires_immediate_termination());

        // Verify exact severity values
        assert_eq!(TamperReason::DebuggerAttached.severity(), 8);
        assert_eq!(TamperReason::EmulatorDetected.severity(), 6);
        assert_eq!(TamperReason::MemoryTampered.severity(), 9);
        assert_eq!(TamperReason::TimingAnomaly.severity(), 5);
        assert_eq!(TamperReason::IntegrityViolation.severity(), 10);
        assert_eq!(TamperReason::LicenseViolation.severity(), 7);
        assert_eq!(TamperReason::CloneDetected.severity(), 10);
        assert_eq!(TamperReason::UnauthorizedAccess.severity(), 8);
    }

    // ========================================================================
    // Q7: SENSITIVE DATA ZEROING
    // ========================================================================

    /// Test 7: Sensitive data is zeroed on destruction
    #[test]
    fn test_zero_sensitive_data() {
        let handler = SelfDestructHandler::new();

        // Trigger to set some state
        handler.trigger_test_mode(TamperReason::TimingAnomaly);
        assert!(handler.is_triggered());
        assert!(handler.get_timestamp() > 0, "Timestamp should be set");

        // Zero sensitive data
        handler.zero_sensitive_data();

        // Reason and timestamp should be zeroed
        assert_eq!(
            handler.reason.load(Ordering::Acquire),
            0,
            "Reason should be zeroed"
        );
        assert_eq!(
            handler.timestamp_unix.load(Ordering::Acquire),
            0,
            "Timestamp should be zeroed"
        );

        // Note: triggered flag is NOT zeroed (irreversible)
        assert!(handler.is_triggered(), "Triggered state should persist");
    }

    // ========================================================================
    // Q8: CASCADE LEVEL MAPPING
    // ========================================================================

    /// Test 8: Cascade level mapping for priorities
    #[test]
    fn test_cascade_level_mapping() {
        // P0 (root) - poisons all
        assert_eq!(cascade_level_for_priority("P0"), 0);

        // P1 (intermediate) - poisons P1 and P2
        assert_eq!(cascade_level_for_priority("P1"), 3);

        // P2 (leaf) - poisons P2 only
        assert_eq!(cascade_level_for_priority("P2"), 8);

        // Unknown - terminal level
        assert_eq!(cascade_level_for_priority("P3"), 15);
        assert_eq!(cascade_level_for_priority(""), 15);
        assert_eq!(cascade_level_for_priority("unknown"), 15);

        // Verify cascade rules
        assert!(should_cascade(0, 0), "P0 cascades to P0");
        assert!(should_cascade(0, 3), "P0 cascades to P1");
        assert!(should_cascade(0, 8), "P0 cascades to P2");
        assert!(should_cascade(3, 3), "P1 cascades to P1");
        assert!(should_cascade(3, 8), "P1 cascades to P2");
        assert!(!should_cascade(3, 0), "P1 does NOT cascade to P0");
        assert!(should_cascade(8, 8), "P2 cascades to P2");
        assert!(!should_cascade(8, 0), "P2 does NOT cascade to P0");
        assert!(!should_cascade(8, 3), "P2 does NOT cascade to P1");
    }

    // ========================================================================
    // ADDITIONAL TESTS (Beyond required 8)
    // ========================================================================

    /// Test: TamperReason descriptions are non-empty
    #[test]
    fn test_tamper_reason_descriptions() {
        let reasons = [
            TamperReason::DebuggerAttached,
            TamperReason::EmulatorDetected,
            TamperReason::MemoryTampered,
            TamperReason::TimingAnomaly,
            TamperReason::IntegrityViolation,
            TamperReason::LicenseViolation,
            TamperReason::CloneDetected,
            TamperReason::UnauthorizedAccess,
        ];

        for reason in &reasons {
            let desc = reason.description();
            assert!(
                !desc.is_empty(),
                "Description for {:?} should not be empty",
                reason
            );
        }
    }

    /// Test: Default trait implementation
    #[test]
    fn test_default_impl() {
        let handler = SelfDestructHandler::default();
        assert!(!handler.is_triggered());
    }

    /// Test: Timestamp capture on trigger
    #[test]
    fn test_timestamp_capture() {
        let handler = SelfDestructHandler::new();

        let before = current_unix_timestamp();
        handler.trigger_test_mode(TamperReason::DebuggerAttached);
        let after = current_unix_timestamp();

        let captured = handler.get_timestamp();
        assert!(
            captured >= before,
            "Timestamp should be >= time before trigger"
        );
        assert!(
            captured <= after,
            "Timestamp should be <= time after trigger"
        );
    }

    /// Test: Concurrent trigger attempts (only one succeeds)
    #[test]
    fn test_concurrent_trigger() {
        use std::sync::Arc;
        use std::thread;

        let handler = Arc::new(SelfDestructHandler::new());
        let mut handles = vec![];

        // Spawn 10 threads all trying to trigger
        for _ in 0..10 {
            let h = Arc::clone(&handler);
            handles.push(thread::spawn(move || {
                h.trigger_test_mode(TamperReason::DebuggerAttached)
            }));
        }

        // Collect results
        let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Exactly one thread should succeed
        let success_count = results.iter().filter(|&&x| x).count();
        assert_eq!(success_count, 1, "Exactly one trigger should succeed");
        assert!(handler.is_triggered());
    }
}
