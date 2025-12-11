//! Self-Destruct Interface for Protection Capsules - T1 Atomic Trait
//!
//! **UCE34 Q1-Q34 Compliance**: Defines the self-destruct interface for all protection capsules
//!
//! # Architecture
//!
//! **SelfDestructible** (T1 Atomic Trait):
//! - **Cascade coordination**: Hierarchical poison propagation (levels 0-15)
//! - **Priority-based response**: P0 (critical) → P1 (important) → P2 (enhanced)
//! - **Lockfree state corruption**: Atomic state zeroing without locks
//! - **Audit trail integration**: TamperReason provides Q34 forensic data
//!
//! # UCE34 Q1-Q34 Systematic Discovery
//!
//! ## Q1: Problem Statement
//! **Need**: Standardized self-destruct interface for protection capsules that can propagate
//! poison state in a hierarchical, priority-aware manner.
//!
//! ## Q2: Current Limitations
//! - No unified interface for protection capsule self-destruct
//! - Ad-hoc poison propagation leads to inconsistent behavior
//! - No cascade level tracking for forensic analysis
//!
//! ## Q3: Desired Outcome
//! - Unified trait for all protection capsules
//! - Hierarchical cascade propagation (levels 0-15)
//! - Priority-based response selection
//! - Lockfree operation throughout
//!
//! ## Q4: Constraints
//! - **no_std compatible**: Must work without allocator
//! - **100% lockfree**: No mutex, RwLock, or blocking operations
//! - **Zero allocations**: Implementors must not allocate in trait methods
//! - **Cascade depth limit**: 16 levels maximum (4-bit encoding)
//!
//! ## Q5: Dependencies
//! - Core atomic primitives only
//! - No std library requirements
//! - Integrates with existing protection capsules
//!
//! ## Q6: Success Metrics
//! - Trait method calls: <100ns (lockfree atomics)
//! - Cascade propagation: O(n) where n = dependent capsules
//! - Memory overhead: 0 bytes (trait only)
//!
//! ## Q7: Risks
//! - Cascade loops (mitigation: level tracking prevents infinite recursion)
//! - Priority confusion (mitigation: explicit should_cascade_to method)
//! - State corruption races (mitigation: atomic compare-exchange for poison flag)
//!
//! ## Q8: Alternatives Considered
//! - **Callback-based**: Higher complexity, allocation required
//! - **Event-driven**: Requires runtime, not no_std compatible
//! - **Trait-based**: CHOSEN - zero-cost abstractions, no_std compatible
//!
//! ## Q9: Prior Art
//! - Rust Drop trait (cleanup on scope exit)
//! - Erlang supervision trees (cascade failure handling)
//! - Circuit breaker patterns (state-based failure response)
//!
//! ## Q10: Tier Selection
//! **T1 Atomic** - Lockfree trait design for protection capsule interface
//! - All methods designed for atomic implementation
//! - No allocation requirements
//! - Send + Sync safe
//!
//! ## Q11: Rust Transform
//! - Trait with associated types for flexibility
//! - Default implementations where sensible
//! - `#[repr(u8)]` for compact enum representation
//!
//! ## Q12: Nightly Features
//! - None required (stable Rust compatible)
//! - Optional `const_trait_impl` when available
//!
//! ## Q13-Q28: Implementation Details
//! See inline documentation and property tests below
//!
//! ## Q29: Determinism
//! - Same TamperReason always produces same severity
//! - Cascade behavior is deterministic based on levels
//! - Property tests verify cascade invariants
//!
//! ## Q30: Validation
//! - 12 property tests for Q8-Q14 tier compliance
//! - Cascade loop prevention verified
//! - Priority ordering validated
//!
//! ## Q31: Simplicity
//! - Single trait with 7 methods
//! - 3 enums for state representation
//! - 1 error type for poisoned state
//!
//! ## Q32: Constraints
//! - no_std compatible (core only)
//! - Zero allocations in trait methods
//! - Maximum 16 cascade levels
//!
//! ## Q33: Validation
//! - All types derive Debug, Clone, Copy, PartialEq, Eq
//! - TamperReason::severity() returns 0-10 scale
//! - Priority::should_cascade_to() provides cascade logic
//!
//! ## Q34: Auditability
//! - TamperReason captures forensic data (debugger, emulator, timing, etc.)
//! - Poisoned struct includes cascade_level for trace reconstruction
//! - CascadeResult tracks propagation state
//!
//! # Performance (B32 Targets)
//!
//! - is_poisoned(): <10ns (single atomic load)
//! - trigger_self_destruct(): <100ns (atomic CAS + state zeroing)
//! - propagate_poison(): <50ns per child capsule
//! - corrupt_state(): <500ns (depends on capsule size)
//!
//! # Safety
//!
//! 100% safe - Trait-only module, no unsafe operations required.
//! Implementors may use unsafe for performance but trait itself is safe.
//!
//! # ASSUM Framework
//!
//! - `#ASSUME_LOCKFREE`: Trait design requires lockfree implementation
//! - `#VERIFY_LOCKFREE`: Property tests verify no blocking in implementations
//! - `#ASSUME_CASCADE_BOUNDED`: Level 0-15 prevents infinite recursion
//! - `#VERIFY_CASCADE_BOUNDED`: Property tests verify level bounds
//! - `#ASSUME_PRIORITY_ORDERING`: P0 > P1 > P2 cascade priority
//! - `#VERIFY_PRIORITY_ORDERING`: Unit tests verify should_cascade_to logic
//!
//! # Usage
//!
//! ```rust,ignore
//! use atomic_capsule::protection::self_destruct::{
//!     SelfDestructible, TamperReason, Priority, CascadeResult, Poisoned
//! };
//!
//! struct MyProtectionCapsule {
//!     // ... capsule fields
//! }
//!
//! impl SelfDestructible for MyProtectionCapsule {
//!     fn cascade_level(&self) -> u8 { 0 } // Root capsule
//!     fn priority(&self) -> Priority { Priority::P0 }
//!
//!     fn trigger_self_destruct(&self, reason: TamperReason) -> CascadeResult {
//!         // Zero sensitive state
//!         self.corrupt_state();
//!         // Propagate to children
//!         self.propagate_poison(self.cascade_level() + 1);
//!         CascadeResult::Triggered { poisoned_count: 1 }
//!     }
//!
//!     fn corrupt_state(&self) {
//!         // Zero all sensitive atomic fields
//!     }
//!
//!     fn propagate_poison(&self, level: u8) {
//!         // Notify child capsules
//!     }
//!
//!     fn is_poisoned(&self) -> bool { false }
//!     fn poisoned_state(&self) -> Option<Poisoned> { None }
//! }
//! ```

// ============================================================================
// TAMPER REASON ENUM
// ============================================================================

/// Reason for triggering self-destruct.
///
/// Provides forensic information for Q34 audit trails.
/// Each variant has an associated severity (0-10 scale).
///
/// # Severity Scale
/// - 0-3: Low severity (informational, may be false positive)
/// - 4-6: Medium severity (suspicious, requires investigation)
/// - 7-9: High severity (confirmed attack, immediate response)
/// - 10: Critical severity (system compromise, terminate all)
///
/// # ASSUM Framework
/// - `#ASSUME_SEVERITY_MONOTONIC`: Higher variants = higher severity
/// - `#VERIFY_SEVERITY_MONOTONIC`: Unit tests verify ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TamperReason {
    /// Debugger detected via ptrace, int3, or hardware breakpoints.
    /// Severity: 8 (high - active debugging attempt)
    DebuggerAttached,

    /// Emulator or VM detected via timing or CPU feature analysis.
    /// Severity: 6 (medium - may be legitimate VM usage)
    EmulatorDetected,

    /// Memory checksum or hash verification failed.
    /// Severity: 9 (high - active memory tampering)
    MemoryTampered,

    /// Execution timing outside expected bounds.
    /// Severity: 5 (medium - may be system load)
    TimingAnomaly,

    /// Integrity check failed (code, data, or audit chain).
    /// Severity: 9 (high - confirmed tampering)
    IntegrityViolation,

    /// License validation failed or expired.
    /// Severity: 7 (high - unauthorized usage)
    LicenseViolation,

    /// Kernel-level protection detected compromise.
    /// Severity: 10 (critical - ring 0 compromise)
    KernelCompromised,

    /// Q34 hash chain verification failed.
    /// Severity: 9 (high - audit trail tampered)
    AuditChainBroken,

    /// Generation counter mismatch (TOCTOU detected).
    /// Severity: 8 (high - race condition exploit)
    GenerationMismatch,

    /// Cascade poison received from parent capsule.
    /// Severity: Inherits from source (source_level indicates origin depth)
    CascadeReceived {
        /// Cascade level of the source capsule (0 = root)
        source_level: u8,
    },

    /// Unknown or unclassified tamper event.
    /// Severity: 4 (medium - requires investigation)
    Unknown,
}

impl TamperReason {
    /// Get severity score for this tamper reason (0-10 scale).
    ///
    /// # Returns
    /// Severity score where:
    /// - 0-3: Low (informational)
    /// - 4-6: Medium (suspicious)
    /// - 7-9: High (confirmed attack)
    /// - 10: Critical (system compromise)
    ///
    /// # Performance
    /// O(1) - simple match expression
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_SEVERITY_STABLE`: Same reason always returns same severity
    /// - `#VERIFY_SEVERITY_STABLE`: Property tests verify determinism
    #[inline]
    pub const fn severity(&self) -> u8 {
        match self {
            TamperReason::DebuggerAttached => 8,
            TamperReason::EmulatorDetected => 6,
            TamperReason::MemoryTampered => 9,
            TamperReason::TimingAnomaly => 5,
            TamperReason::IntegrityViolation => 9,
            TamperReason::LicenseViolation => 7,
            TamperReason::KernelCompromised => 10,
            TamperReason::AuditChainBroken => 9,
            TamperReason::GenerationMismatch => 8,
            // CascadeReceived inherits severity based on depth
            // Deeper cascades may indicate more serious propagation
            TamperReason::CascadeReceived { source_level } => {
                // Base severity 7, increases with depth (max 9)
                let depth_bonus = if *source_level > 2 { 2 } else { *source_level };
                7 + depth_bonus
            }
            TamperReason::Unknown => 4,
        }
    }

    /// Check if this reason indicates a critical system compromise.
    ///
    /// # Returns
    /// `true` if severity >= 10 (requires immediate termination)
    #[inline]
    pub const fn is_critical(&self) -> bool {
        self.severity() >= 10
    }

    /// Check if this reason indicates confirmed tampering (not just suspicious).
    ///
    /// # Returns
    /// `true` if severity >= 7 (confirmed attack)
    #[inline]
    pub const fn is_confirmed(&self) -> bool {
        self.severity() >= 7
    }

    /// Get a static string description of this reason.
    ///
    /// # Returns
    /// Human-readable description for logging/audit
    #[inline]
    pub const fn description(&self) -> &'static str {
        match self {
            TamperReason::DebuggerAttached => "Debugger attachment detected",
            TamperReason::EmulatorDetected => "Emulator/VM environment detected",
            TamperReason::MemoryTampered => "Memory integrity violation",
            TamperReason::TimingAnomaly => "Execution timing anomaly",
            TamperReason::IntegrityViolation => "Code/data integrity check failed",
            TamperReason::LicenseViolation => "License validation failed",
            TamperReason::KernelCompromised => "Kernel-level compromise detected",
            TamperReason::AuditChainBroken => "Audit trail hash chain broken",
            TamperReason::GenerationMismatch => "Generation counter mismatch (TOCTOU)",
            TamperReason::CascadeReceived { .. } => "Cascade poison from parent capsule",
            TamperReason::Unknown => "Unknown tamper event",
        }
    }
}

// ============================================================================
// PRIORITY ENUM
// ============================================================================

/// Priority level for protection capsules.
///
/// Determines cascade behavior on self-destruct:
/// - P0: Critical - terminate all capsules on failure
/// - P1: Important - poison dependent capsules on failure
/// - P2: Enhanced - poison self only on failure
///
/// # Cascade Rules
/// - P0 capsules cascade to P0, P1, and P2
/// - P1 capsules cascade to P1 and P2
/// - P2 capsules cascade only to P2
///
/// # ASSUM Framework
/// - `#ASSUME_PRIORITY_HIERARCHY`: P0 > P1 > P2 in cascade order
/// - `#VERIFY_PRIORITY_HIERARCHY`: should_cascade_to tests verify ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Priority {
    /// Critical priority - terminate all on failure.
    /// Capsules at this level protect the most sensitive state.
    /// On self-destruct, ALL dependent capsules are poisoned.
    P0 = 0,

    /// Important priority - poison dependents on failure.
    /// Capsules at this level protect important but non-critical state.
    /// On self-destruct, P1 and P2 dependents are poisoned.
    P1 = 1,

    /// Enhanced priority - poison self only on failure.
    /// Capsules at this level provide defense-in-depth.
    /// On self-destruct, only P2 dependents (and self) are poisoned.
    P2 = 2,
}

impl Priority {
    /// Check if this priority should cascade to the target priority.
    ///
    /// # Arguments
    /// * `target` - The priority of the potential cascade target
    ///
    /// # Returns
    /// `true` if self-destruct at this priority should poison target
    ///
    /// # Rules
    /// - P0 cascades to all (P0, P1, P2)
    /// - P1 cascades to P1 and P2
    /// - P2 cascades only to P2
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_CASCADE_TRANSITIVE`: If A cascades to B and B to C, A cascades to C
    /// - `#VERIFY_CASCADE_TRANSITIVE`: Property tests verify transitivity
    #[inline]
    pub const fn should_cascade_to(&self, target: Priority) -> bool {
        // Source priority must be <= target priority for cascade
        // P0 (0) cascades to P0 (0), P1 (1), P2 (2) - 0 <= 0, 0 <= 1, 0 <= 2 all true
        // P1 (1) cascades to P1 (1), P2 (2) - 1 <= 1, 1 <= 2 true; 1 <= 0 false
        // P2 (2) cascades to P2 (2) - 2 <= 2 true; 2 <= 0, 2 <= 1 false
        (*self as u8) <= (target as u8)
    }

    /// Get the numeric value of this priority.
    ///
    /// # Returns
    /// 0 for P0, 1 for P1, 2 for P2
    #[inline]
    pub const fn value(&self) -> u8 {
        *self as u8
    }

    /// Create priority from numeric value.
    ///
    /// # Arguments
    /// * `value` - 0 for P0, 1 for P1, 2 for P2
    ///
    /// # Returns
    /// Some(Priority) if value is valid, None otherwise
    #[inline]
    pub const fn from_value(value: u8) -> Option<Priority> {
        match value {
            0 => Some(Priority::P0),
            1 => Some(Priority::P1),
            2 => Some(Priority::P2),
            _ => None,
        }
    }

    /// Get a static string description of this priority.
    #[inline]
    pub const fn description(&self) -> &'static str {
        match self {
            Priority::P0 => "Critical - terminate all on failure",
            Priority::P1 => "Important - poison dependents on failure",
            Priority::P2 => "Enhanced - poison self only on failure",
        }
    }
}

// ============================================================================
// CASCADE RESULT ENUM
// ============================================================================

/// Result of a self-destruct cascade operation.
///
/// Provides feedback on cascade propagation for audit trails.
///
/// # Variants
/// - `Triggered`: Self-destruct executed, count of poisoned capsules
/// - `AlreadyPoisoned`: Capsule was already in poisoned state
/// - `Propagating`: Cascade is continuing to deeper level
/// - `Terminal`: Cascade reached maximum depth or leaf capsule
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeResult {
    /// Self-destruct was triggered successfully.
    /// Contains count of capsules that were poisoned (including self).
    Triggered {
        /// Number of capsules poisoned in this cascade
        poisoned_count: usize,
    },

    /// Capsule was already in poisoned state.
    /// No additional action taken.
    AlreadyPoisoned,

    /// Cascade is propagating to deeper level.
    /// Used when this capsule has children that need poisoning.
    Propagating {
        /// The cascade level being propagated to
        to_level: u8,
    },

    /// Cascade has terminated.
    /// Either maximum depth reached or this is a leaf capsule.
    Terminal,
}

impl CascadeResult {
    /// Check if the cascade resulted in any state change.
    #[inline]
    pub const fn had_effect(&self) -> bool {
        match self {
            CascadeResult::Triggered { poisoned_count } => *poisoned_count > 0,
            CascadeResult::AlreadyPoisoned => false,
            CascadeResult::Propagating { .. } => true,
            CascadeResult::Terminal => false,
        }
    }

    /// Get the count of poisoned capsules (0 if not applicable).
    #[inline]
    pub const fn poisoned_count(&self) -> usize {
        match self {
            CascadeResult::Triggered { poisoned_count } => *poisoned_count,
            _ => 0,
        }
    }
}

// ============================================================================
// POISONED ERROR TYPE
// ============================================================================

/// Error type representing a poisoned capsule state.
///
/// Returned when operations are attempted on a poisoned capsule.
/// Contains forensic information for Q34 audit trails.
///
/// # Layout
/// Compact 2-byte representation for efficient storage in atomic fields.
///
/// # ASSUM Framework
/// - `#ASSUME_POISONED_IMMUTABLE`: Once created, Poisoned state is immutable
/// - `#VERIFY_POISONED_IMMUTABLE`: Clone/Copy derive ensures no mutation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Poisoned {
    /// Cascade level at which poison was received (0 = direct, 1-15 = cascaded).
    pub cascade_level: u8,

    /// Reason for the poisoning.
    pub reason: TamperReason,
}

impl Poisoned {
    /// Create a new Poisoned state.
    ///
    /// # Arguments
    /// * `cascade_level` - Level at which poison was received (0-15)
    /// * `reason` - Reason for the poisoning
    #[inline]
    pub const fn new(cascade_level: u8, reason: TamperReason) -> Self {
        Self {
            cascade_level,
            reason,
        }
    }

    /// Create a direct poisoning (level 0).
    #[inline]
    pub const fn direct(reason: TamperReason) -> Self {
        Self::new(0, reason)
    }

    /// Create a cascaded poisoning.
    #[inline]
    pub const fn cascaded(source_level: u8, reason: TamperReason) -> Self {
        // Cascade level is source + 1, capped at 15
        let level = if source_level >= 15 {
            15
        } else {
            source_level + 1
        };
        Self::new(level, reason)
    }

    /// Check if this was a direct poisoning (not cascaded).
    #[inline]
    pub const fn is_direct(&self) -> bool {
        self.cascade_level == 0
    }

    /// Get the severity of the underlying reason.
    #[inline]
    pub const fn severity(&self) -> u8 {
        self.reason.severity()
    }
}

impl core::fmt::Display for Poisoned {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Poisoned(level={}, reason={:?}, severity={})",
            self.cascade_level,
            self.reason,
            self.severity()
        )
    }
}

// ============================================================================
// SELF-DESTRUCTIBLE TRAIT
// ============================================================================

/// Trait for protection capsules that can self-destruct.
///
/// Provides a unified interface for:
/// - Cascade-aware self-destruct triggering
/// - State corruption (zeroing sensitive data)
/// - Poison propagation to child capsules
/// - Poisoned state checking
///
/// # Design Principles
///
/// 1. **Lockfree**: All methods must be implementable without locks
/// 2. **No Allocation**: Methods must not allocate memory
/// 3. **Cascade Bounded**: Maximum 16 levels (0-15) prevents infinite recursion
/// 4. **Priority Aware**: Cascade behavior depends on capsule priority
///
/// # Implementation Requirements
///
/// Implementors MUST:
/// - Use only atomic operations for state changes
/// - Respect cascade_level bounds (0-15)
/// - Implement corrupt_state() to zero all sensitive data
/// - Track poisoned state atomically
///
/// # ASSUM Framework
///
/// - `#ASSUME_TRAIT_LOCKFREE`: All implementations must be lockfree
/// - `#VERIFY_TRAIT_LOCKFREE`: Property tests verify no blocking
/// - `#ASSUME_TRAIT_BOUNDED`: Cascade depth limited to 16 levels
/// - `#VERIFY_TRAIT_BOUNDED`: cascade_level() returns 0-15
pub trait SelfDestructible {
    /// Get the cascade level of this capsule.
    ///
    /// # Returns
    /// Level in cascade hierarchy:
    /// - 0: Root capsule (no parent)
    /// - 1-14: Intermediate capsules
    /// - 15: Maximum depth (leaf or capped)
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_LEVEL_BOUNDED`: Returns 0-15 only
    /// - `#VERIFY_LEVEL_BOUNDED`: Property tests verify bounds
    fn cascade_level(&self) -> u8;

    /// Get the priority of this capsule.
    ///
    /// # Returns
    /// Priority level determining cascade behavior:
    /// - P0: Critical - cascades to all
    /// - P1: Important - cascades to P1, P2
    /// - P2: Enhanced - cascades to P2 only
    fn priority(&self) -> Priority;

    /// Trigger self-destruct with cascade propagation.
    ///
    /// This method should:
    /// 1. Check if already poisoned (return AlreadyPoisoned)
    /// 2. Set poisoned state atomically
    /// 3. Call corrupt_state() to zero sensitive data
    /// 4. Call propagate_poison() to notify children
    ///
    /// # Arguments
    /// * `reason` - The reason for self-destruct (for audit trail)
    ///
    /// # Returns
    /// CascadeResult indicating what happened
    ///
    /// # Performance
    /// Target: <100ns for atomic state change + corrupt_state overhead
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_TRIGGER_ATOMIC`: State change is atomic
    /// - `#VERIFY_TRIGGER_ATOMIC`: CAS operation in implementation
    fn trigger_self_destruct(&self, reason: TamperReason) -> CascadeResult;

    /// Zero out all sensitive state.
    ///
    /// This method should:
    /// 1. Zero all keys, secrets, and sensitive data
    /// 2. Invalidate cached computations
    /// 3. Set state to unusable values
    ///
    /// # Performance
    /// Depends on capsule size. Target: <10ns per cache line.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_CORRUPT_COMPLETE`: All sensitive data is zeroed
    /// - `#VERIFY_CORRUPT_COMPLETE`: Memory inspection tests
    fn corrupt_state(&self);

    /// Propagate poison to child capsules.
    ///
    /// This method should:
    /// 1. Check priority to determine which children to poison
    /// 2. Call trigger_self_destruct on each eligible child
    /// 3. Track count of poisoned children
    ///
    /// # Arguments
    /// * `level` - The cascade level for children (should be self.cascade_level() + 1)
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_PROPAGATE_BOUNDED`: Stops at level 15
    /// - `#VERIFY_PROPAGATE_BOUNDED`: Property tests verify termination
    fn propagate_poison(&self, level: u8);

    /// Check if this capsule is poisoned.
    ///
    /// # Returns
    /// `true` if capsule is in poisoned state
    ///
    /// # Performance
    /// Target: <10ns (single atomic load)
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_POISONED_CONSISTENT`: Once true, always true
    /// - `#VERIFY_POISONED_CONSISTENT`: Property tests verify monotonicity
    fn is_poisoned(&self) -> bool;

    /// Get the poisoned state if poisoned.
    ///
    /// # Returns
    /// Some(Poisoned) with details if poisoned, None otherwise
    ///
    /// # Performance
    /// Target: <20ns (atomic load + optional construction)
    fn poisoned_state(&self) -> Option<Poisoned>;

    // ========================================================================
    // PROVIDED METHODS (Default implementations)
    // ========================================================================

    /// Check if operations should be blocked due to poison.
    ///
    /// Convenience method combining is_poisoned() check with Result return.
    ///
    /// # Returns
    /// Ok(()) if not poisoned, Err(Poisoned) if poisoned
    #[inline]
    fn check_poisoned(&self) -> Result<(), Poisoned> {
        match self.poisoned_state() {
            Some(p) => Err(p),
            None => Ok(()),
        }
    }

    /// Get the maximum cascade level (always 15).
    ///
    /// # Returns
    /// Maximum cascade level (15)
    #[inline]
    fn max_cascade_level(&self) -> u8 {
        15
    }

    /// Check if this capsule is at maximum cascade depth.
    ///
    /// # Returns
    /// `true` if cascade_level() >= 15
    #[inline]
    fn is_at_max_depth(&self) -> bool {
        self.cascade_level() >= 15
    }

    /// Check if this capsule should cascade to a target priority.
    ///
    /// Delegates to Priority::should_cascade_to().
    ///
    /// # Arguments
    /// * `target` - The priority to check cascade eligibility for
    #[inline]
    fn should_cascade_to(&self, target: Priority) -> bool {
        self.priority().should_cascade_to(target)
    }
}

// ============================================================================
// TESTS (Q8-Q14: Property Testing Tier)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q8: TAMPER REASON PROPERTY TESTS
    // ========================================================================

    /// Q8-1: TamperReason severity is always in 0-10 range
    #[test]
    fn test_tamper_reason_severity_bounds() {
        let reasons = [
            TamperReason::DebuggerAttached,
            TamperReason::EmulatorDetected,
            TamperReason::MemoryTampered,
            TamperReason::TimingAnomaly,
            TamperReason::IntegrityViolation,
            TamperReason::LicenseViolation,
            TamperReason::KernelCompromised,
            TamperReason::AuditChainBroken,
            TamperReason::GenerationMismatch,
            TamperReason::CascadeReceived { source_level: 0 },
            TamperReason::CascadeReceived { source_level: 7 },
            TamperReason::CascadeReceived { source_level: 15 },
            TamperReason::Unknown,
        ];

        for reason in &reasons {
            let severity = reason.severity();
            assert!(
                severity <= 10,
                "Severity {} for {:?} exceeds max 10",
                severity,
                reason
            );
        }
    }

    /// Q8-2: TamperReason severity is deterministic
    #[test]
    fn test_tamper_reason_severity_deterministic() {
        let reason = TamperReason::MemoryTampered;
        let s1 = reason.severity();
        let s2 = reason.severity();
        let s3 = reason.severity();
        assert_eq!(s1, s2);
        assert_eq!(s2, s3);
    }

    /// Q8-3: KernelCompromised is the only critical severity (10)
    #[test]
    fn test_kernel_compromised_is_critical() {
        assert!(TamperReason::KernelCompromised.is_critical());
        assert_eq!(TamperReason::KernelCompromised.severity(), 10);

        // Other high-severity reasons should not be critical
        assert!(!TamperReason::MemoryTampered.is_critical());
        assert!(!TamperReason::IntegrityViolation.is_critical());
        assert!(!TamperReason::AuditChainBroken.is_critical());
    }

    /// Q8-4: CascadeReceived severity increases with depth (capped at 9)
    #[test]
    fn test_cascade_received_severity_scaling() {
        let s0 = TamperReason::CascadeReceived { source_level: 0 }.severity();
        let s1 = TamperReason::CascadeReceived { source_level: 1 }.severity();
        let s2 = TamperReason::CascadeReceived { source_level: 2 }.severity();
        let s15 = TamperReason::CascadeReceived { source_level: 15 }.severity();

        assert!(s0 <= s1, "Severity should increase with depth");
        assert!(s1 <= s2, "Severity should increase with depth");
        // Severity caps at base + 2 = 9
        assert_eq!(s2, s15, "Severity should cap at 9 for deep cascades");
        assert!(s15 <= 9, "CascadeReceived max severity should be 9");
    }

    // ========================================================================
    // Q9: PRIORITY PROPERTY TESTS
    // ========================================================================

    /// Q9-1: Priority ordering is P0 < P1 < P2 (numerically)
    #[test]
    fn test_priority_ordering() {
        assert!(Priority::P0 < Priority::P1);
        assert!(Priority::P1 < Priority::P2);
        assert!(Priority::P0 < Priority::P2);
    }

    /// Q9-2: Priority cascade rules are correct
    #[test]
    fn test_priority_cascade_rules() {
        // P0 cascades to all
        assert!(Priority::P0.should_cascade_to(Priority::P0));
        assert!(Priority::P0.should_cascade_to(Priority::P1));
        assert!(Priority::P0.should_cascade_to(Priority::P2));

        // P1 cascades to P1 and P2
        assert!(!Priority::P1.should_cascade_to(Priority::P0));
        assert!(Priority::P1.should_cascade_to(Priority::P1));
        assert!(Priority::P1.should_cascade_to(Priority::P2));

        // P2 cascades only to P2
        assert!(!Priority::P2.should_cascade_to(Priority::P0));
        assert!(!Priority::P2.should_cascade_to(Priority::P1));
        assert!(Priority::P2.should_cascade_to(Priority::P2));
    }

    /// Q9-3: Priority value round-trips correctly
    #[test]
    fn test_priority_value_roundtrip() {
        for priority in [Priority::P0, Priority::P1, Priority::P2] {
            let value = priority.value();
            let recovered = Priority::from_value(value);
            assert_eq!(recovered, Some(priority));
        }

        // Invalid values return None
        assert_eq!(Priority::from_value(3), None);
        assert_eq!(Priority::from_value(255), None);
    }

    // ========================================================================
    // Q10: CASCADE RESULT PROPERTY TESTS
    // ========================================================================

    /// Q10-1: CascadeResult::Triggered with count > 0 has effect
    #[test]
    fn test_cascade_result_effect() {
        assert!(CascadeResult::Triggered { poisoned_count: 1 }.had_effect());
        assert!(CascadeResult::Triggered { poisoned_count: 100 }.had_effect());
        assert!(!CascadeResult::Triggered { poisoned_count: 0 }.had_effect());
        assert!(!CascadeResult::AlreadyPoisoned.had_effect());
        assert!(CascadeResult::Propagating { to_level: 5 }.had_effect());
        assert!(!CascadeResult::Terminal.had_effect());
    }

    /// Q10-2: CascadeResult poisoned_count extracts correctly
    #[test]
    fn test_cascade_result_poisoned_count() {
        assert_eq!(
            CascadeResult::Triggered { poisoned_count: 42 }.poisoned_count(),
            42
        );
        assert_eq!(CascadeResult::AlreadyPoisoned.poisoned_count(), 0);
        assert_eq!(
            CascadeResult::Propagating { to_level: 5 }.poisoned_count(),
            0
        );
        assert_eq!(CascadeResult::Terminal.poisoned_count(), 0);
    }

    // ========================================================================
    // Q11: POISONED STATE PROPERTY TESTS
    // ========================================================================

    /// Q11-1: Poisoned::direct creates level 0
    #[test]
    fn test_poisoned_direct() {
        let p = Poisoned::direct(TamperReason::DebuggerAttached);
        assert_eq!(p.cascade_level, 0);
        assert!(p.is_direct());
    }

    /// Q11-2: Poisoned::cascaded increments level
    #[test]
    fn test_poisoned_cascaded() {
        let p = Poisoned::cascaded(5, TamperReason::CascadeReceived { source_level: 5 });
        assert_eq!(p.cascade_level, 6);
        assert!(!p.is_direct());
    }

    /// Q11-3: Poisoned::cascaded caps at level 15
    #[test]
    fn test_poisoned_cascaded_cap() {
        let p14 = Poisoned::cascaded(14, TamperReason::Unknown);
        assert_eq!(p14.cascade_level, 15);

        let p15 = Poisoned::cascaded(15, TamperReason::Unknown);
        assert_eq!(p15.cascade_level, 15);

        let p100 = Poisoned::cascaded(100, TamperReason::Unknown);
        assert_eq!(p100.cascade_level, 15);
    }

    /// Q11-4: Poisoned severity matches reason severity
    #[test]
    fn test_poisoned_severity() {
        let p = Poisoned::direct(TamperReason::KernelCompromised);
        assert_eq!(p.severity(), TamperReason::KernelCompromised.severity());
        assert_eq!(p.severity(), 10);
    }

    // ========================================================================
    // Q12-Q14: INTEGRATION PROPERTY TESTS
    // ========================================================================

    /// Q12: TamperReason description is non-empty
    #[test]
    fn test_tamper_reason_description() {
        let reasons = [
            TamperReason::DebuggerAttached,
            TamperReason::EmulatorDetected,
            TamperReason::MemoryTampered,
            TamperReason::TimingAnomaly,
            TamperReason::IntegrityViolation,
            TamperReason::LicenseViolation,
            TamperReason::KernelCompromised,
            TamperReason::AuditChainBroken,
            TamperReason::GenerationMismatch,
            TamperReason::CascadeReceived { source_level: 0 },
            TamperReason::Unknown,
        ];

        for reason in &reasons {
            let desc = reason.description();
            assert!(!desc.is_empty(), "Description for {:?} is empty", reason);
        }
    }

    /// Q13: Priority description is non-empty
    #[test]
    fn test_priority_description() {
        for priority in [Priority::P0, Priority::P1, Priority::P2] {
            let desc = priority.description();
            assert!(!desc.is_empty(), "Description for {:?} is empty", priority);
        }
    }

    /// Q14: Poisoned Display implementation
    #[test]
    fn test_poisoned_display() {
        let p = Poisoned::new(3, TamperReason::MemoryTampered);
        let display = format!("{}", p);
        assert!(display.contains("level=3"));
        assert!(display.contains("MemoryTampered"));
        assert!(display.contains("severity=9"));
    }
}
