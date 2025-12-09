//! Async integration for AtomicHedgeCapsule
//!
//! Provides async-compatible interfaces for hedge operations while maintaining
//! the same lockfree guarantees and performance characteristics.

use crate::capsule_standalone::AtomicHedgeCapsule;
use crate::types::{HedgeError, HedgeResult};

/// Async wrapper for hedge operations
pub struct AsyncHedgeCapsule {
    capsule: AtomicHedgeCapsule,
}

impl AsyncHedgeCapsule {
    /// Create a new async hedge capsule
    pub fn new(capsule: AtomicHedgeCapsule) -> Self {
        Self { capsule }
    }

    /// Execute hedge asynchronously
    pub async fn execute_hedge(&self, size: f64) -> Result<HedgeResult, HedgeError> {
        // For now, just delegate to sync implementation
        // In a full implementation, this would integrate with async runtime
        self.capsule.execute_hedge(size)
    }

    /// Check status asynchronously
    pub async fn status(&self) -> crate::types::HedgeStatus {
        self.capsule.status()
    }
}
