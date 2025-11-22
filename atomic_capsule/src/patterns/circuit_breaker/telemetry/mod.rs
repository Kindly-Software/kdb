//! Telemetry utilities for driving breaker metrics from external sources.

pub mod sample;

#[cfg(all(feature = "circuit-breaker-pmu", target_os = "linux"))]
pub mod pmu;

#[cfg(feature = "circuit-breaker-auto-tune")]
pub mod history;

#[cfg(feature = "circuit-breaker-auto-tune")]
pub mod calibration;

#[cfg(feature = "circuit-breaker-auto-tune")]
pub mod scenario;

#[cfg(feature = "circuit-breaker-auto-tune")]
pub mod feedback;

#[cfg(feature = "circuit-breaker-auto-tune")]
pub mod adaptive;

pub use sample::{MockSource, TelemetrySample, TelemetrySource};

#[cfg(all(feature = "circuit-breaker-pmu", target_os = "linux"))]
pub use pmu::{PmuCollector, PmuConfig};

#[cfg(all(feature = "circuit-breaker-pmu", not(target_os = "linux")))]
compile_error!("the `pmu` feature currently requires a Linux target");

#[cfg(feature = "circuit-breaker-auto-tune")]
use crate::patterns::circuit_breaker::breaker::MetricsSnapshot;

#[cfg(feature = "circuit-breaker-auto-tune")]
pub use adaptive::update_adaptive_thresholds;
#[cfg(feature = "circuit-breaker-auto-tune")]
pub use calibration::{
    tune_policy, AutoCalibrator, CalibrationMode, CalibrationTargets, PolicyDraft, WindowMetrics,
};
#[cfg(feature = "circuit-breaker-auto-tune")]
pub use feedback::{LevelFeedback, LevelFeedbackConfig, LevelFeedbackResult, LevelTarget};
#[cfg(feature = "circuit-breaker-auto-tune")]
pub use history::{HistoryBuffer, HistoryEntry};
#[cfg(feature = "circuit-breaker-auto-tune")]
pub use scenario::{generate_all, ScenarioData, ScenarioKind};

#[cfg(feature = "circuit-breaker-auto-tune")]
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

#[cfg(feature = "circuit-breaker-auto-tune")]
/// Outcome supplied by workloads indicating whether level actions were effective.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionOutcome {
    /// True if the workload recovered within the target window after this transition.
    pub recovered_within_target: bool,
    /// Observed recovery latency in milliseconds, when available.
    pub observed_recovery_ms: Option<u32>,
}
