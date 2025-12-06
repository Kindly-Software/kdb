//! Observability Module - Health Checks, Metrics, and Logging
//!
//! **Purpose**: Expose observability endpoints for Kubernetes/Docker health checks
//! and Prometheus metrics collection.
//!
//! **Endpoints**:
//! - GET /health        → HealthStatus (liveness/readiness probe)
//! - GET /metrics       → Prometheus metrics (observability)
//!
//! **Framework Compliance**:
//! - UCE34: Not applicable (infrastructure, not capsules)
//! - ASSUM: Atomic counters verified, zero unsafe code in fast paths
//! - B32: Fair baselines (health check <100ms, metrics <50ms)
//! - T28: Production-grade monitoring
//! - COCA: T1 Atomic (lockfree counters)

use crate::health::HealthStatus;
use crate::metrics::MetricsCapsule;

pub use crate::health::{decrement_active_sessions, get_active_sessions, get_health_status, init_health, increment_active_sessions};
pub use crate::metrics::{get_metrics_instance};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_serialization() {
        let status = HealthStatus {
            status: "healthy".to_string(),
            version: "0.1.0".to_string(),
            uptime_secs: 3600,
            active_sessions: 42,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"status\":\"healthy\""));
        assert!(json.contains("\"version\":\"0.1.0\""));
    }

    #[test]
    fn test_metrics_export() {
        let metrics = MetricsCapsule::new();
        metrics.increment_requests();
        metrics.increment_deletion_proofs();

        let output = metrics.export_prometheus();
        assert!(output.contains("kdb_requests_total"));
        assert!(output.contains("kdb_deletion_proofs_issued_total"));
    }
}
