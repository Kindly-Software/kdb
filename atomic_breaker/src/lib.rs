#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::identity_op)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::new_without_default)]
#![allow(clippy::elidable_lifetime_names)]
#![doc = "Universal atomic, bit-packed circuit breaker primitive."]
#![doc = ""]
#![doc = "The crate exposes a lock-free breaker whose entire state fits in a single 64-bit "]
#![doc = "atomic word, suitable for ultra-low latency control loops."]

#[cfg(all(feature = "standard64", feature = "compact48"))]
compile_error!("`standard64` and `compact48` features are mutually exclusive");
#[cfg(not(any(feature = "standard64", feature = "compact48")))]
compile_error!("enable either `standard64` (default) or `compact48`");

pub mod aggregate;
pub mod breaker;
pub mod cause;
pub mod diag;
pub mod layout;
pub mod policy;
/// RLT-1024 strategy evaluation helpers.
pub mod rlt;
#[cfg(feature = "std")]
pub mod telemetry;

pub use atomic_risk_ladder_table::layout::actions::{ActionBases, AppliedActionSet, RoutePolicy};
#[cfg(feature = "mpmc")]
pub use breaker::AtomicBreakerMPMC;
#[cfg(feature = "auto_tune")]
pub use breaker::MetricsSnapshot;
pub use breaker::{AtomicBreakerGuard, AtomicBreakerSWeMR};
pub use rlt::{
    evaluate_strategy, EvaluationError, LevelDecision, LevelState, LevelTransition,
    StrategyActions, StrategyId, StressInputs,
};

/// Layout marker for the standard 64-bit packing.
pub use layout::{DefaultLayout, STANDARD64_V1};
#[cfg(feature = "auto_tune")]
pub use telemetry::{
    generate_all, tune_policy, ActionOutcome, AutoCalibrator, CalibrationMode, CalibrationTargets,
    HistoryBuffer, HistoryEntry, LevelFeedback, LevelFeedbackConfig, LevelFeedbackResult,
    LevelTarget, MetricsTap, PolicyDraft, ScenarioData, ScenarioKind, WindowMetrics,
};
#[cfg(feature = "std")]
pub use telemetry::{MockSource, TelemetrySample, TelemetrySource};
#[cfg(all(feature = "pmu", target_os = "linux"))]
pub use telemetry::{PmuCollector, PmuConfig};
