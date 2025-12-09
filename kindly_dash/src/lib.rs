//! kindly_dash: Real-Time Monitoring Dashboard for the Kindly Ecosystem
//!
//! A production-grade dashboard built in 100% Rust with sub-100ms latency,
//! built-in forecasting, and anomaly detection.
//!
//! # Quick Start
//!
//! ```no_run
//! use kindly_dash::{DashboardServer, MetricsSource, DashboardSnapshot};
//! use std::sync::Arc;
//! use std::sync::atomic::AtomicU64;
//! use std::sync::atomic::Ordering;
//!
//! struct MyMetrics {
//!     cost: Arc<AtomicU64>,
//!     requests: Arc<AtomicU64>,
//! }
//!
//! impl MetricsSource for MyMetrics {
//!     fn snapshot(&self) -> DashboardSnapshot {
//!         DashboardSnapshot {
//!             timestamp_ns: std::time::SystemTime::now()
//!                 .duration_since(std::time::UNIX_EPOCH)
//!                 .unwrap()
//!                 .as_nanos() as u64,
//!             total_cost_cents: self.cost.load(Ordering::Relaxed),
//!             total_requests: self.requests.load(Ordering::Relaxed),
//!             // ... other fields
//!             ..Default::default()
//!         }
//!     }
//!
//!     fn budget_metrics(&self, _id: u64) -> Option<crate::BudgetMetrics> { None }
//!     fn provider_metrics(&self) -> Vec<crate::ProviderMetrics> { Vec::new() }
//!     fn alert_history(&self) -> Vec<crate::Alert> { Vec::new() }
//!     fn forecast(&self, _budget_id: u64, _days: u32) -> Option<crate::Forecast> { None }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let metrics = MyMetrics {
//!         cost: Arc::new(AtomicU64::new(0)),
//!         requests: Arc::new(AtomicU64::new(0)),
//!     };
//!
//!     let dashboard = DashboardServer::builder()
//!         .metrics_source(Arc::new(metrics))
//!         .build()?;
//!
//!     println!("Dashboard running on http://localhost:8080");
//!     // Start HTTP server with dashboard routes...
//!
//!     Ok(())
//! }
//! ```

// Enable nightly features when simd feature is enabled
#![cfg_attr(feature = "simd", feature(portable_simd))]

pub mod traits;
pub mod websocket;
pub mod capsules;
pub mod server;
pub mod types;
pub mod hash;
pub mod forensics;

// Re-export public API
pub use traits::MetricsSource;
pub use server::DashboardServer;
pub use types::{
    DashboardSnapshot,
    BudgetMetrics,
    ProviderMetrics,
    Alert,
    Forecast,
    AlertSeverity,
    CircuitState,
};
pub use hash::CapsuleHash64;
pub use forensics::{
    HashedCapsule,
    CapsuleAuditTrail,
    CapsuleSnapshot,
    TamperEvent,
    TamperType,
    SOXAudit,
    SOC2Audit,
    GDPRAudit,
    HIPAAAudit,
    export_audit_json,
    export_audit_csv,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// All 18 computational capsules from clapi_core
pub mod capsules_info {
    /// Phase 1: Foundation Capsules (7)
    pub const PHASE1_CAPSULES: &[&str] = &[
        "RequestCapsule128",
        "ResponseCapsule256",
        "RoutingCapsule128",
        "AuditLogEntry128",
        "EpochTile1024",
        "CircuitBreakerCapsule",
        "BudgetSlotCapsule",
    ];

    /// Phase 2: Monitoring Capsules (3)
    pub const PHASE2_CAPSULES: &[&str] = &[
        "CircuitBreakerMetrics",
        "ProviderCircuitStatus",
        "ProviderCircuitArray",
    ];

    /// Phase 3: Hash Integrity Capsules (2)
    pub const PHASE3_CAPSULES: &[&str] = &[
        "CapsuleHash64",
        "RequestCapsule128Enhanced",
    ];

    /// Phase 4: Compliance Capsules (2)
    pub const PHASE4_CAPSULES: &[&str] = &[
        "BudgetMetaCapsule",
        "AuditEntry",
    ];

    /// Phase 4.5: Metrics & Forecasting Capsules (4)
    pub const PHASE45_CAPSULES: &[&str] = &[
        "MetricsSnapshot",
        "ProviderMetrics",
        "BudgetMetrics",
        "ChartDataCapsule",
    ];

    /// All capsules
    pub fn all_capsules() -> Vec<&'static str> {
        let mut capsules = Vec::new();
        capsules.extend_from_slice(PHASE1_CAPSULES);
        capsules.extend_from_slice(PHASE2_CAPSULES);
        capsules.extend_from_slice(PHASE3_CAPSULES);
        capsules.extend_from_slice(PHASE4_CAPSULES);
        capsules.extend_from_slice(PHASE45_CAPSULES);
        capsules
    }

    /// Total count
    pub fn total_capsules() -> usize {
        all_capsules().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_capsules_count() {
        assert_eq!(capsules_info::total_capsules(), 18);
    }

    #[test]
    fn test_phase_breakdown() {
        let all = capsules_info::all_capsules();
        assert_eq!(all.len(), 18);

        // Verify phase counts
        assert_eq!(capsules_info::PHASE1_CAPSULES.len(), 7);
        assert_eq!(capsules_info::PHASE2_CAPSULES.len(), 3);
        assert_eq!(capsules_info::PHASE3_CAPSULES.len(), 2);
        assert_eq!(capsules_info::PHASE4_CAPSULES.len(), 2);
        assert_eq!(capsules_info::PHASE45_CAPSULES.len(), 4);
    }
}
