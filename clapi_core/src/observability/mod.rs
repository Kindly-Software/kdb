//! Observability Module - Alerts, Metrics, Monitoring
//!
//! **Purpose**: External integrations for operational visibility
//! **Integration**: PagerDuty (critical alerts) + Slack (team notifications)
//! **Architecture**: Lockfree queue + async dispatch (I20 validated)
//!
//! # I20 Integration Analysis
//! - **Q1-Q5 (Scope)**: Alert system + metrics endpoint, integrating with existing server
//! - **Q6-Q10 (Compatibility)**: Lockfree queue + Axum async handlers, compatible
//! - **Q11-Q15 (Safety)**: No races (lockfree queue), no corruption (immutable alerts)
//! - **Q16-Q20 (Validation)**: Integration tests, rate limiting, auth optional

pub mod alert_system;

pub use alert_system::{Alert, AlertLevel, AlertSystem};
