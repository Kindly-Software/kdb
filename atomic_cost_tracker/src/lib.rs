//! Atomic Cost Tracker (ACT-128)
//!
//! This crate provides a narrow interface for producing and consuming a packed
//! 128-bit snapshot that encapsulates the edge, cost, and gating decision for a
//! proposed trade. Writers publish a fully populated word with a single
//! release-store, while readers consume it with relaxed loads on the hot path.

pub mod coordinator;
pub mod engine;
pub mod estimator;
pub mod events;
pub mod gate;
pub mod layout;
pub mod manager;
pub mod router;
pub mod service;
pub mod strategy;
pub mod telemetry;
pub mod writer;

pub use coordinator::{ActCoordinator, OrderRequest, StrategyRouter};
pub use engine::ActEngine;
pub use estimator::{
    ActEstimator, EstimationInputs, EstimatorConfig, FeeSchedule, FillFeedback, LatencyTicket,
    OrderIntent, Route, Side, SlipCoefficients, SlipFeeSurface, VenueSnapshot,
};
pub use gate::{evaluate_gate, GateConfig, GateDecision, GateOutcome};
pub use layout::{ActFlags, ActSnapshot, ActWord, FixedQ8_8};
pub use manager::{ActEngineManager, ManagerError};
pub use router::CountingRouter;
pub use service::{ActService, NoopTelemetrySink, ServiceError, TelemetrySink};
pub use strategy::StrategyGate;
pub use telemetry::{
    ActTelemetry, FillStats, SnapshotStats, TelemetryFillEntry, TelemetryKey, TelemetryReport,
    TelemetrySnapshotEntry,
};
pub use writer::ActSlot;
