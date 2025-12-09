//! Computational capsules for git coordination.
//!
//! All capsules follow UCE34 tier architecture:
//! - T1 Atomic: LockCapsule, InstanceCapsule
//! - T4 Batch: QueueCapsule
//! - Q34: AuditLogCapsule

pub mod lock;
pub mod queue;
pub mod instance;
pub mod audit;

pub use lock::LockCapsule;
pub use queue::{QueueCapsule, Operation};
pub use instance::InstanceCapsule;
pub use audit::{AuditLogCapsule, AuditEntry};
