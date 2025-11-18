//! # GitCoordinator - High-Level Coordination API

use crate::capsules::{LockCapsule, QueueCapsule, InstanceCapsule, AuditLogCapsule, AuditEntry};
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Git operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitOperation {
    /// No-op (for testing)
    Noop,
    /// Commit operation
    Commit,
    /// Branch operation
    Branch,
    /// Checkout operation
    Checkout,
}

/// Instance ID wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceId(u32);

impl InstanceId {
    /// Create new instance ID
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get as u64
    pub fn as_u64(&self) -> u64 {
        self.0 as u64
    }
}

/// Instance registry for tracking active instances
#[derive(Debug, Default)]
pub struct InstanceRegistry {
    next_id: AtomicU32,
}

impl InstanceRegistry {
    /// Create new registry
    pub fn new() -> Self {
        Self {
            next_id: AtomicU32::new(1),
        }
    }

    /// Generate new instance ID
    pub fn generate_id(&self) -> InstanceId {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        InstanceId(id)
    }
}

/// High-level git coordination
pub struct GitCoordinator {
    lock: Arc<LockCapsule>,
    queue: Arc<QueueCapsule>,
    registry: InstanceRegistry,
    audit: Arc<std::sync::Mutex<Vec<AuditEntry>>>,  // Simple audit log
    instance_id: InstanceId,
}

impl GitCoordinator {
    /// Create new coordinator
    pub fn new() -> Self {
        let registry = InstanceRegistry::new();
        let instance_id = registry.generate_id();

        Self {
            lock: Arc::new(LockCapsule::new()),
            queue: Arc::new(QueueCapsule::new(1024)),
            registry,
            audit: Arc::new(std::sync::Mutex::new(Vec::new())),
            instance_id,
        }
    }

    /// Get instance ID
    pub fn instance_id(&self) -> InstanceId {
        self.instance_id
    }

    /// Execute git operation with coordination
    pub fn execute<F, T>(&self, f: F) -> crate::error::Result<T>
    where
        F: FnOnce() -> crate::error::Result<T>,
    {
        // For now, just execute the operation directly
        // In production, this would acquire locks and coordinate access
        f()
    }
}

impl Default for GitCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_create() {
        let coord = GitCoordinator::new();
        assert_ne!(coord.instance_id().0, 0);
    }

    #[test]
    fn test_multiple_instances() {
        let coord1 = GitCoordinator::new();
        let coord2 = GitCoordinator::new();

        assert_ne!(coord1.instance_id(), coord2.instance_id());
    }
}
