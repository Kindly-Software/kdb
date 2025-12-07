//! Access Control Module
//!
//! Cryptographic primitives for license validation and secure access control.
//!
//! ## Capsules
//! - `AccessModeCapsule`: Lockfree state machine for Observer/Operator modes (T1 Atomic)
//! - `OperatorChallengeCapsule`: Ed25519 challenge-response authentication (T1 Atomic)
//! - `OperatorSessionCapsule`: SOTA session management with Q34 audit trail (T1 Atomic)
//! - `ed25519_verifier`: Signature verification utilities
//!
//! ## Security Properties
//! - 256-bit cryptographic nonces (hybrid timestamp + OsRng)
//! - Constant-time operations to prevent timing attacks
//! - Ed25519 signature verification with strict mode (rejects weak keys)
//! - SHA-256 hashing for public key binding
//! - Single-use challenge enforcement via atomic CAS
//! - Generation counters for replay/ABA prevention
//! - Zero secrets on drop (automatic via ed25519-dalek)
//! - Session cryptographic binding (challenge hash + pubkey hash)
//! - Configurable session timeouts (5min/30min/1hr/never)
//!
//! ## Framework Compliance
//! - T1 Atomic: Lockfree, no mutex/RwLock
//! - ASSUM: All unsafe documented (none in this module - pure safe Rust)
//! - T28: Comprehensive test coverage (30+ tests)
//! - Q34: Cryptographic audit trail support (rolling hash-chain)

pub mod access_mode_capsule;
mod config_loader;
pub mod ed25519_verifier;
pub mod operator_challenge_capsule;
pub mod operator_session_capsule;
pub mod security_config;

pub use access_mode_capsule::{AccessMode, AccessModeCapsule, AccessModeError};

pub use ed25519_verifier::{
    hash_public_key, parse_public_key, parse_signature, verify_challenge_signature,
    VerificationError,
};

pub use operator_challenge_capsule::{
    ChallengeCapsuleError, ChallengeState, OperatorChallengeCapsule,
};

pub use operator_session_capsule::{
    OperatorSessionCapsule, OperatorSessionError, SessionStats,
    TIMEOUT_5_MIN, TIMEOUT_30_MIN, TIMEOUT_1_HOUR, TIMEOUT_NEVER,
};

pub use security_config::{
    AuditLevel, KeyStorageMethod, SecurityConfig, SecurityConfigError, SecurityPreset,
};

pub use config_loader::{load_default, load_from_file, load_from_str, ConfigLoadError};

/// Tool permission classification for Observer/Operator access control.
///
/// Each MCP tool is classified as either:
/// - [`Observer`](ToolPermission::Observer): Read-only operations, always permitted
/// - [`Operator`](ToolPermission::Operator): Write/execute operations, require authentication
///
/// # Design Rationale
///
/// The classification follows the principle of least privilege:
/// - **Observer tools** cannot modify process state or execution flow
/// - **Operator tools** can modify state and require Ed25519 authentication
///
/// High-risk Operator tools (write_memory, write_registers) may require
/// re-authentication in Paranoid security preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ToolPermission {
    /// Read-only tools permitted in Observer mode.
    ///
    /// These tools cannot modify process state:
    /// - Reading memory, registers, stack traces
    /// - Listing breakpoints and snapshots
    /// - Verifying audit trails
    /// - Capturing snapshots (non-destructive)
    Observer = 0,

    /// Write/execute tools requiring Operator mode.
    ///
    /// These tools can modify process state:
    /// - Attaching/detaching from processes
    /// - Setting/removing breakpoints
    /// - Continuing execution or stepping
    /// - Writing memory or registers
    /// - Time-travel navigation (modifies view)
    Operator = 1,
}

impl ToolPermission {
    /// Check if this permission level allows process modification.
    #[inline]
    pub const fn can_modify_process(&self) -> bool {
        matches!(self, ToolPermission::Operator)
    }

    /// Check if this permission level is read-only.
    #[inline]
    pub const fn is_read_only(&self) -> bool {
        matches!(self, ToolPermission::Observer)
    }
}

impl std::fmt::Display for ToolPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolPermission::Observer => write!(f, "Observer"),
            ToolPermission::Operator => write!(f, "Operator"),
        }
    }
}

/// Check if a tool ID requires Operator mode.
///
/// This function implements the permission matrix defined in the module documentation.
/// Tool IDs are assigned by the MCP server integration.
///
/// # Tool ID Mapping
///
/// | Range   | Permission | Examples                                    |
/// |---------|------------|---------------------------------------------|
/// | 1-5     | Operator   | attach, detach, set_breakpoint, continue    |
/// | 6-12    | Observer   | info_breakpoints, stack_trace, read_memory  |
/// | 13-15   | Operator   | step, step_backward, step_forward           |
/// | 16-17   | Observer   | get_comprehensive_audit, export_audit_json  |
/// | 18-21   | Operator   | write_memory, write_registers (HIGH RISK)   |
/// | 22-23   | Observer   | get_process_status, get_session_stats       |
/// | 24+     | Observer   | Future read-only tools                      |
///
/// # Example
///
/// ```rust
/// use kdb::access_control::requires_operator;
///
/// assert!(requires_operator(1));  // attach requires Operator
/// assert!(requires_operator(3));  // set_breakpoint requires Operator
/// assert!(!requires_operator(7)); // get_stack_trace is Observer
/// assert!(!requires_operator(8)); // read_memory is Observer
/// assert!(requires_operator(18)); // write_memory requires Operator
/// ```
#[inline]
pub const fn requires_operator(tool_id: u16) -> bool {
    // Tool ID permission matrix:
    // 1-5: Operator (attach, detach, set_breakpoint, remove_breakpoint, continue)
    // 6-12: Observer (info, stack, read_memory, read_registers, snapshot, list_snapshots, verify_audit)
    // 13-15: Operator (step, step_backward, step_forward)
    // 16-17: Observer (get_comprehensive_audit, export_audit_json)
    // 18-21: Operator (write_memory, write_registers, inject_breakpoint, restore_breakpoint)
    // 22-23: Observer (get_process_status, get_session_stats)
    matches!(tool_id, 1..=5 | 13..=15 | 18..=21)
}

/// Get the permission level required for a tool ID.
///
/// This is the inverse of [`requires_operator`] - it returns the full
/// [`ToolPermission`] enum rather than a boolean.
///
/// # Example
///
/// ```rust
/// use kdb::access_control::{get_tool_permission, ToolPermission};
///
/// assert_eq!(get_tool_permission(1), ToolPermission::Operator);
/// assert_eq!(get_tool_permission(7), ToolPermission::Observer);
/// ```
#[inline]
pub const fn get_tool_permission(tool_id: u16) -> ToolPermission {
    if requires_operator(tool_id) {
        ToolPermission::Operator
    } else {
        ToolPermission::Observer
    }
}

/// Check if a tool ID is classified as high-risk.
///
/// High-risk tools can cause significant damage to the target process:
/// - **write_memory (18)**: Can corrupt process memory
/// - **write_registers (19)**: Can crash process or cause undefined behavior
/// - **inject_breakpoint (20)**: Can corrupt instruction stream
/// - **restore_breakpoint (21)**: Can restore corrupted instructions
///
/// In Paranoid security preset, high-risk tools require re-authentication
/// even during an active Operator session.
///
/// # Example
///
/// ```rust
/// use kdb::access_control::is_high_risk_tool;
///
/// assert!(is_high_risk_tool(18));  // write_memory is high-risk
/// assert!(is_high_risk_tool(19));  // write_registers is high-risk
/// assert!(!is_high_risk_tool(3));  // set_breakpoint is NOT high-risk
/// assert!(!is_high_risk_tool(13)); // step is NOT high-risk
/// ```
#[inline]
pub const fn is_high_risk_tool(tool_id: u16) -> bool {
    matches!(tool_id, 18..=21)
}

/// Get the human-readable name for a tool ID.
///
/// Returns `None` for unknown tool IDs.
///
/// # Example
///
/// ```rust
/// use kdb::access_control::get_tool_name;
///
/// assert_eq!(get_tool_name(1), Some("attach"));
/// assert_eq!(get_tool_name(7), Some("get_stack_trace"));
/// assert_eq!(get_tool_name(100), None);
/// ```
#[inline]
pub const fn get_tool_name(tool_id: u16) -> Option<&'static str> {
    match tool_id {
        1 => Some("attach"),
        2 => Some("detach"),
        3 => Some("set_breakpoint"),
        4 => Some("remove_breakpoint"),
        5 => Some("continue_execution"),
        6 => Some("info_breakpoints"),
        7 => Some("get_stack_trace"),
        8 => Some("read_memory"),
        9 => Some("read_registers"),
        10 => Some("capture_snapshot"),
        11 => Some("list_snapshots"),
        12 => Some("verify_audit_trail"),
        13 => Some("step"),
        14 => Some("step_backward"),
        15 => Some("step_forward"),
        16 => Some("get_comprehensive_audit"),
        17 => Some("export_audit_json"),
        18 => Some("write_memory"),
        19 => Some("write_registers"),
        20 => Some("inject_breakpoint"),
        21 => Some("restore_breakpoint"),
        22 => Some("get_process_status"),
        23 => Some("get_session_stats"),
        _ => None,
    }
}

/// Total number of defined tools in the MCP interface.
pub const TOOL_COUNT: u16 = 23;

/// Tool IDs that are classified as Observer (read-only).
pub const OBSERVER_TOOLS: &[u16] = &[6, 7, 8, 9, 10, 11, 12, 16, 17, 22, 23];

/// Tool IDs that are classified as Operator (write/execute).
pub const OPERATOR_TOOLS: &[u16] = &[1, 2, 3, 4, 5, 13, 14, 15, 18, 19, 20, 21];

/// Tool IDs that are classified as high-risk (require re-auth in Paranoid mode).
pub const HIGH_RISK_TOOLS: &[u16] = &[18, 19, 20, 21];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_requires_operator() {
        // Operator tools (1-5)
        assert!(requires_operator(1)); // attach
        assert!(requires_operator(2)); // detach
        assert!(requires_operator(3)); // set_breakpoint
        assert!(requires_operator(4)); // remove_breakpoint
        assert!(requires_operator(5)); // continue_execution

        // Observer tools (6-12)
        assert!(!requires_operator(6));  // info_breakpoints
        assert!(!requires_operator(7));  // get_stack_trace
        assert!(!requires_operator(8));  // read_memory
        assert!(!requires_operator(9));  // read_registers
        assert!(!requires_operator(10)); // capture_snapshot
        assert!(!requires_operator(11)); // list_snapshots
        assert!(!requires_operator(12)); // verify_audit_trail

        // Operator tools (13-15)
        assert!(requires_operator(13)); // step
        assert!(requires_operator(14)); // step_backward
        assert!(requires_operator(15)); // step_forward

        // Observer tools (16-17)
        assert!(!requires_operator(16)); // get_comprehensive_audit
        assert!(!requires_operator(17)); // export_audit_json

        // Operator tools (18-21) - HIGH RISK
        assert!(requires_operator(18)); // write_memory
        assert!(requires_operator(19)); // write_registers
        assert!(requires_operator(20)); // inject_breakpoint
        assert!(requires_operator(21)); // restore_breakpoint

        // Observer tools (22-23)
        assert!(!requires_operator(22)); // get_process_status
        assert!(!requires_operator(23)); // get_session_stats

        // Unknown tools default to Observer
        assert!(!requires_operator(24));
        assert!(!requires_operator(100));
    }

    #[test]
    fn test_get_tool_permission() {
        assert_eq!(get_tool_permission(1), ToolPermission::Operator);
        assert_eq!(get_tool_permission(7), ToolPermission::Observer);
        assert_eq!(get_tool_permission(18), ToolPermission::Operator);
        assert_eq!(get_tool_permission(22), ToolPermission::Observer);
    }

    #[test]
    fn test_is_high_risk_tool() {
        // High-risk tools
        assert!(is_high_risk_tool(18)); // write_memory
        assert!(is_high_risk_tool(19)); // write_registers
        assert!(is_high_risk_tool(20)); // inject_breakpoint
        assert!(is_high_risk_tool(21)); // restore_breakpoint

        // Non-high-risk Operator tools
        assert!(!is_high_risk_tool(1));  // attach
        assert!(!is_high_risk_tool(3));  // set_breakpoint
        assert!(!is_high_risk_tool(13)); // step

        // Observer tools
        assert!(!is_high_risk_tool(7)); // get_stack_trace
        assert!(!is_high_risk_tool(8)); // read_memory
    }

    #[test]
    fn test_get_tool_name() {
        assert_eq!(get_tool_name(1), Some("attach"));
        assert_eq!(get_tool_name(7), Some("get_stack_trace"));
        assert_eq!(get_tool_name(18), Some("write_memory"));
        assert_eq!(get_tool_name(23), Some("get_session_stats"));
        assert_eq!(get_tool_name(24), None);
        assert_eq!(get_tool_name(100), None);
    }

    #[test]
    fn test_tool_permission_methods() {
        assert!(ToolPermission::Operator.can_modify_process());
        assert!(!ToolPermission::Observer.can_modify_process());

        assert!(!ToolPermission::Operator.is_read_only());
        assert!(ToolPermission::Observer.is_read_only());
    }

    #[test]
    fn test_tool_permission_display() {
        assert_eq!(format!("{}", ToolPermission::Observer), "Observer");
        assert_eq!(format!("{}", ToolPermission::Operator), "Operator");
    }

    #[test]
    fn test_tool_constants() {
        assert_eq!(TOOL_COUNT, 23);
        assert_eq!(OBSERVER_TOOLS.len(), 11);
        assert_eq!(OPERATOR_TOOLS.len(), 12);
        assert_eq!(HIGH_RISK_TOOLS.len(), 4);

        // Verify all tools are accounted for
        assert_eq!(
            OBSERVER_TOOLS.len() + OPERATOR_TOOLS.len(),
            TOOL_COUNT as usize
        );

        // Verify high-risk tools are subset of operator tools
        for tool_id in HIGH_RISK_TOOLS {
            assert!(OPERATOR_TOOLS.contains(tool_id));
        }
    }

    #[test]
    fn test_tool_id_consistency() {
        // Verify all defined tools have names
        for tool_id in 1..=TOOL_COUNT {
            assert!(
                get_tool_name(tool_id).is_some(),
                "Tool {} should have a name",
                tool_id
            );
        }

        // Verify Observer tools don't require operator
        for &tool_id in OBSERVER_TOOLS {
            assert!(
                !requires_operator(tool_id),
                "Observer tool {} should not require operator",
                tool_id
            );
        }

        // Verify Operator tools do require operator
        for &tool_id in OPERATOR_TOOLS {
            assert!(
                requires_operator(tool_id),
                "Operator tool {} should require operator",
                tool_id
            );
        }
    }
}
