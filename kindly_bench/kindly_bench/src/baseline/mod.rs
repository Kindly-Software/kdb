//! Baseline generation strategies for fair benchmark comparison
//!
//! Provides tier-specific baseline generators:
//! - T1 Atomic → RwLock/Mutex
//! - T2 SIMD → Scalar loops
//! - T3 Fixed-Point → f64

pub mod t1_atomic;
pub mod t2_simd;
pub mod t3_fixed;

/// Baseline kind for different tiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineKind {
    /// No audit trail (T0 Auditable)
    NoAuditTrail,
    /// RwLock (T1 Atomic)
    RwLock,
    /// Mutex (T1 Atomic, more conservative)
    Mutex,
    /// Scalar loops (T2 SIMD)
    Scalar,
    /// f64 floating-point (T3 Fixed-Point)
    F64,
    /// Sequential processing (T4 Batch)
    Sequential,
    /// Batch recomputation (T5 Streaming)
    Batch,
    /// Non-optimized composition (T6 Mixed)
    NonOptimizedComposition,
    /// Custom baseline (user-defined)
    Custom,
}

impl BaselineKind {
    /// Get default baseline for tier
    pub fn default_for_tier(tier: &str) -> Self {
        match tier {
            "T0" | "T0-Auditable" => Self::NoAuditTrail,
            "T1" | "T1-Atomic" => Self::RwLock,
            "T2" | "T2-SIMD" => Self::Scalar,
            "T3" | "T3-Fixed-Point" => Self::F64,
            "T4" | "T4-Batch" => Self::Sequential,
            "T5" | "T5-Streaming" => Self::Batch,
            "T6" | "T6-Mixed" => Self::NonOptimizedComposition,
            _ => Self::Custom,
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::NoAuditTrail => "No Audit Trail",
            Self::RwLock => "RwLock",
            Self::Mutex => "Mutex",
            Self::Scalar => "Scalar",
            Self::F64 => "F64",
            Self::Sequential => "Sequential",
            Self::Batch => "Batch",
            Self::NonOptimizedComposition => "Non-Optimized Composition",
            Self::Custom => "Custom",
        }
    }
}
