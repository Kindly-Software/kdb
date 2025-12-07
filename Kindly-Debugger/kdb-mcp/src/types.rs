//! Common types for kdb_mcp
//!
//! Re-exports of types from their canonical source modules.
//! This module avoids circular dependencies by re-exporting types
//! from their actual implementations.

// ============================================================================
// Type Re-exports (avoiding circular dependencies)
// ============================================================================

// Re-export Command from access_control module
#[cfg(feature = "access-control")]
pub use crate::access_control::Command;

// Re-export Operation from audit_enhancement module
#[cfg(feature = "audit")]
pub use crate::audit_enhancement::Operation;

// Re-export PolicyAction from zero_trust_policy module
#[cfg(feature = "zero-trust")]
pub use crate::zero_trust_policy::PolicyAction;

// ============================================================================
// Session Types (for auth_token, zero_trust_policy)
// ============================================================================

/// Session identifier (stub)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SessionId(pub u64);

impl SessionId {
    /// Create new session ID
    pub fn new(id: u64) -> Self {
        SessionId(id)
    }

    /// Get ID value
    pub fn value(&self) -> u64 {
        self.0
    }

    /// Check if session ID is empty (zero)
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Get length (always 8 for u64)
    pub fn len(&self) -> usize {
        8
    }
}

// ============================================================================
// Capsule Re-exports (to avoid circular dependencies)
// ============================================================================

// NOTE: These are re-exports to allow modules to reference capsules
// without circular dependencies. All capsules below are fully implemented
// in their respective modules.

/// Audit enhancement capsule (implemented in audit_enhancement.rs)
#[cfg(feature = "audit")]
pub use crate::audit_enhancement::AuditEnhancementCapsule;

/// Audit enhancement capsule placeholder (when feature disabled)
#[cfg(not(feature = "audit"))]
pub struct AuditEnhancementCapsule {
    _phantom: core::marker::PhantomData<()>,
}

/// Dynamic PID whitelist capsule (implemented in dynamic_pid_whitelist.rs)
#[cfg(feature = "dynamic-pid")]
pub use crate::dynamic_pid_whitelist::DynamicPidWhitelistCapsule;

/// Dynamic PID whitelist capsule placeholder (when feature disabled)
#[cfg(not(feature = "dynamic-pid"))]
pub struct DynamicPidWhitelistCapsule {
    _phantom: core::marker::PhantomData<()>,
}

/// Key rotation capsule (implemented in key_rotation.rs)
#[cfg(feature = "key-rotation")]
pub use crate::key_rotation::KeyRotationCapsule;

/// Key rotation capsule placeholder (when feature disabled)
#[cfg(not(feature = "key-rotation"))]
pub struct KeyRotationCapsule {
    _phantom: core::marker::PhantomData<()>,
}

/// Memory encryption capsule (implemented in memory_encryption.rs)
#[cfg(feature = "memory-encryption")]
pub use crate::memory_encryption::MemoryEncryptionCapsule;

/// Memory encryption capsule placeholder (when feature disabled)
#[cfg(not(feature = "memory-encryption"))]
pub struct MemoryEncryptionCapsule {
    _phantom: core::marker::PhantomData<()>,
}

/// ACME cert manager capsule (implemented in acme_cert_manager.rs)
#[cfg(feature = "acme")]
pub use crate::acme_cert_manager::AcmeCertManagerCapsule;

/// ACME cert manager capsule placeholder (when feature disabled)
#[cfg(not(feature = "acme"))]
pub struct AcmeCertManagerCapsule {
    _phantom: core::marker::PhantomData<()>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "access-control")]
    fn test_command_enum() {
        use crate::access_control::Command;
        // Test that Command enum variants exist
        let _ = Command::Read;
        let _ = Command::Write;
        let _ = Command::Step;
        let _ = Command::Continue;
    }

    #[test]
    fn test_session_id() {
        let id = SessionId::new(42);
        assert_eq!(id.value(), 42);
    }
}
