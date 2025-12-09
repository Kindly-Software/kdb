//! Computational capsules for dashboard state and data
//!
//! Four core capsules:
//! - DashboardStateCapsule (128B, Tier 1 Atomic): UI state (<20ns access)
//! - ChartDataCapsule (256B, Tier 2 SIMD): Chart preprocessing (<50ns)
//! - MessageBatchCapsule (1KB, Tier 4 Batch): WebSocket batching (100ms)
//! - WebSocketHealthCapsule (64B, Tier 1 Atomic): Health monitoring (<20ns)
//!
//! # Status
//!
//! Phase 6 complete: WebSocket health monitoring with circuit breaker pattern.

pub mod dashboard_state;
pub mod chart_data;
pub mod message_batch;
pub mod websocket_health;

// Export implemented capsules
pub use chart_data::ChartDataCapsule;
pub use message_batch::MessageBatchCapsule;
pub use websocket_health::{WebSocketHealthCapsule, HealthState, HealthMetrics};

// TODO: Export when implemented
// pub use dashboard_state::DashboardStateCapsule;
