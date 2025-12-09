//! Telemetry utilities for driving breaker metrics from external sources.

pub mod sample;

#[cfg(all(feature = "pmu", target_os = "linux"))]
pub mod pmu;

#[cfg(feature = "auto_tune")]
pub mod history;

#[cfg(feature = "auto_tune")]
pub mod calibration;

#[cfg(feature = "auto_tune")]
pub mod scenario;

#[cfg(feature = "auto_tune")]
pub mod feedback;

pub use sample::{MockSource, TelemetrySample, TelemetrySource};

#[cfg(all(feature = "pmu", target_os = "linux"))]
pub use pmu::{PmuCollector, PmuConfig};

#[cfg(all(feature = "pmu", not(target_os = "linux")))]
compile_error!("the `pmu` feature currently requires a Linux target");

#[cfg(feature = "auto_tune")]
use crate::breaker::MetricsSnapshot;

#[cfg(feature = "auto_tune")]
pub use calibration::{
    tune_policy, AutoCalibrator, CalibrationMode, CalibrationTargets, PolicyDraft, WindowMetrics,
};
#[cfg(feature = "auto_tune")]
pub use feedback::{LevelFeedback, LevelFeedbackConfig, LevelFeedbackResult, LevelTarget};
#[cfg(feature = "auto_tune")]
pub use history::{HistoryBuffer, HistoryEntry};
#[cfg(feature = "auto_tune")]
pub use scenario::{generate_all, ScenarioData, ScenarioKind};

#[cfg(feature = "auto_tune")]
/// Observer trait for feeding breaker outcomes to adaptive controllers.
pub trait MetricsTap {
    /// Record the metrics before/after an evaluation along with the driving sample.
    fn record_transition(
        &mut self,
        now_ms: u32,
        before: &MetricsSnapshot,
        after: &MetricsSnapshot,
        sample: &TelemetrySample,
    ) -> Option<ActionOutcome> {
        let _ = (now_ms, before, after, sample);
        None
    }

    /// Flush any buffered observations (no-op by default).
    fn flush(&mut self) {}
}

#[cfg(feature = "auto_tune")]
/// Outcome supplied by workloads indicating whether level actions were effective.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionOutcome {
    /// True if the workload recovered within the target window after this transition.
    pub recovered_within_target: bool,
    /// Observed recovery latency in milliseconds, when available.
    pub observed_recovery_ms: Option<u32>,
}
