//! Audit Module - Q34 Compliant Configuration Audit Trail
//!
//! Hash-chained audit logging for configuration operations with tamper detection.
//!
//! ## Modules
//!
//! - `capsule` - AuditLoggerCapsule (64B, T0 Auditable + T1 Atomic)
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use kdb_mcp::configure::audit::{AuditLoggerCapsule, AuditOperation};
//!
//! let logger = AuditLoggerCapsule::new();
//!
//! // Log configuration events
//! logger.log_config_detected("claude-code", "/home/user/.config/claude/mcp.json")?;
//! logger.log_config_backup("claude-code", "2025-12-11T14-30-00")?;
//! logger.log_config_modified("claude-code", "updated", "/home/user/.config/claude/mcp.json")?;
//!
//! // Verify hash chain integrity (Q34 compliance)
//! let entry_count = logger.verify_integrity()?;
//! println!("Verified {} entries", entry_count);
//!
//! // Get current hash chain head
//! let hash = logger.current_hash();
//! println!("Chain head: {:016x}", hash);
//! ```
//!
//! ## Log Format
//!
//! ```text
//! 2025-12-11T14:30:00Z | CONFIG_DETECTED | claude-code | path=/home/user/... | hash:abc123...
//! 2025-12-11T14:30:01Z | CONFIG_BACKUP | claude-code | backup_id=2025-12-11T14-30-00 | hash:def456...
//! ```
//!
//! ## Q34 Compliance
//!
//! - Each entry's hash includes the previous entry's hash (chain)
//! - Tamper detection via hash chain verification
//! - SOX/SOC2/GDPR/HIPAA audit trail support

mod capsule;

pub use capsule::{
    // Core capsule
    AuditLoggerCapsule,
    // Types
    AuditEntry,
    AuditOperation,
    AuditStats,
    AuditError,
    // Constants
    AUDIT_DIR_NAME,
    MAX_ENTRIES_PER_FILE,
    MAX_LOG_FILES,
    GENESIS_HASH,
    // Utility
    fnv1a_hash,
    hash_with_prev,
};
