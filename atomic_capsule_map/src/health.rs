//! Circuit breaker and health monitoring.

use core::sync::atomic::{AtomicU64, Ordering};

/// Breaker levels for circuit breaker pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum BreakerLevel {
    /// L0: Normal operation
    L0 = 0,
    /// L1: Elevated caution (minor degradation)
    L1 = 1,
    /// L2: High risk (major degradation)
    L2 = 2,
    /// L3: Critical (circuit open, reject new operations)
    L3 = 3,
}

impl BreakerLevel {
    fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::L0,
            1 => Self::L1,
            2 => Self::L2,
            _ => Self::L3,
        }
    }
}

/// Health status of the map.
#[derive(Debug, Clone)]
pub struct HealthStatus {
    /// Current breaker level
    pub breaker_level: BreakerLevel,
    /// Total operations performed
    pub total_ops: u64,
    /// Failed operations
    pub failed_ops: u64,
    /// Current error rate (basis points)
    pub error_rate_bp: u16,
}

/// Health monitor with circuit breaker.
#[repr(C, align(64))]
pub struct HealthMonitor {
    // Layout: [level:8 | total_ops:28 | failed_ops:28]
    state: AtomicU64,
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
        }
    }

    pub fn status(&self) -> HealthStatus {
        let state = self.state.load(Ordering::Relaxed);
        let level = ((state >> 56) & 0xFF) as u8;
        let total_ops = (state >> 28) & 0x0FFF_FFFF;
        let failed_ops = state & 0x0FFF_FFFF;

        let error_rate_bp = if total_ops > 0 {
            ((failed_ops * 10_000) / total_ops) as u16
        } else {
            0
        };

        HealthStatus {
            breaker_level: BreakerLevel::from_u8(level),
            total_ops,
            failed_ops,
            error_rate_bp,
        }
    }

    pub fn set_level(&self, level: BreakerLevel) {
        let current = self.state.load(Ordering::Relaxed);
        let new = (current & 0x00FF_FFFF_FFFF_FFFF) | ((level as u64) << 56);
        self.state.store(new, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn record_op(&self, success: bool) {
        // Increment total_ops and maybe failed_ops
        let increment = if success {
            1u64 << 28 // Just total_ops
        } else {
            (1u64 << 28) | 1 // Both total_ops and failed_ops
        };

        self.state.fetch_add(increment, Ordering::Relaxed);
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}
