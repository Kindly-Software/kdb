/// System Responsiveness Library
/// Exposes computational capsules for benchmarking and testing

pub mod capsules;

// Re-export for convenience
pub use capsules::{ProcessStateCapsule, ResourceGovernorCapsule, CircuitState, StreamingMonitorCapsule};
