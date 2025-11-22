//! # Circuit Breaker Patterns (Production-Grade)
//!
//! Universal, atomic, bit-packed circuit breaker primitives migrated from `atomic_breaker`.
//!
//! **Tier**: T1 Atomic (Lockfree Coordination)
//! **Origin**: Migrated from standalone `atomic_breaker` crate (v0.1.0)
//! **Status**: Production-ready (battle-tested in trading, UI, audio systems)
//!
//! ## Features
//!
//! - **Dual layouts**: Standard64 (full metrics) + Compact48 (embedded)
//! - **MPMC support**: Multi-writer variant with bounded CAS retries
//! - **Fixed-point metrics**: Q8.8 or Q6.10 for deterministic arithmetic
//! - **8 cause flags**: THERM, NET, IO, CPU, LAT, MEM, GPU, DISK
//! - **Exponential backoff**: 6-bit index (0-63 levels)
//! - **Fractal degradation**: L0-L3 quality tiers
//! - **Hardware telemetry**: Linux perf-event integration (feature-gated)
//! - **Adaptive policies**: Auto-calibration from history (feature-gated)
//!
//! ## Layout Bit Packing
//!
//! ### Standard64 (default)
//! ```text
//! 63-58     57-50   49-34        33-18        17-4      3-2  1-0
//! backoff   cause   sg_norm      mu_norm      err       L    S
//! (6)       (8)     Q8.8(16)     Q8.8(16)     (14)      (2)  (2)
//! ```
//!
//! ### Compact48
//! ```text
//! 47-32        31-16        15-4   3-2  1-0
//! sg_norm      mu_norm      err    L    S
//! Q6.10(16)    Q6.10(16)    (12)   (2)  (2)
//! ```
//!
//! ## Performance (B32 Validated)
//!
//! - Load: <5ns (relaxed), <8ns (acquire)
//! - Update: <15ns (single store, SWeMR)
//! - MPMC: <50ns (bounded CAS loop, 8 retries)
//!
//! ## Usage
//!
//! ```rust
//! use atomic_capsule::patterns::circuit_breaker::{CircuitBreaker, State, Policy, evaluate};
//!
//! // Create breaker
//! let breaker = CircuitBreaker::new(State::Closed);
//!
//! // Policy-driven evaluation
//! let pol = Policy::ui_holographic();
//! let mut last_change = 0;
//! evaluate(&breaker, mu, sigma, err_inc, timestamp, &mut last_change, &pol);
//!
//! // State inspection
//! let guard = breaker.guard();
//! if guard.state() == State::Open {
//!     // Circuit is open, reject operations
//! }
//! ```
//!
//! ## Feature Flags
//!
//! - `circuit-breaker-standard64` *(default)* – Full 64-bit layout with causes/backoff
//! - `circuit-breaker-compact48` – 48-bit layout for embedded systems
//! - `circuit-breaker-mpmc` – Multi-writer variant (bounded CAS)
//! - `circuit-breaker-pmu` – Linux perf-event telemetry
//! - `circuit-breaker-auto-tune` – Adaptive policy calibration
//!
//! ## Trade Secret Notice
//!
//! This module contains production-grade circuit breaker implementations tested in
//! high-frequency trading (MES/MNQ scalping), real-time UI rendering, and audio pipelines.
//! The adaptive policy system and hardware telemetry integration are proprietary innovations.

#![cfg_attr(not(feature = "std"), no_std)]
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

// Feature flag validation (mutually exclusive layouts)
#[cfg(all(
    feature = "circuit-breaker-standard64",
    feature = "circuit-breaker-compact48"
))]
compile_error!(
    "`circuit-breaker-standard64` and `circuit-breaker-compact48` are mutually exclusive"
);

#[cfg(not(any(
    feature = "circuit-breaker-standard64",
    feature = "circuit-breaker-compact48"
)))]
compile_error!(
    "enable either `circuit-breaker-standard64` (default) or `circuit-breaker-compact48`"
);

// Core modules (always available)
pub mod aggregate;
pub mod breaker;
pub mod cause;
pub mod diag;
pub mod layout;
pub mod policy;

// Serialization module (feature-gated)
#[cfg(feature = "capsule-serialize")]
pub mod serialize;

// Telemetry modules (feature-gated)
#[cfg(feature = "std")]
pub mod telemetry;

// Re-exports for convenience
pub use breaker::{AtomicBreakerGuard, AtomicBreakerSWeMR, State};

// Type aliases for ergonomic naming
pub type CircuitBreaker = AtomicBreakerSWeMR;
pub type CircuitBreakerGuard = AtomicBreakerGuard;

#[cfg(feature = "circuit-breaker-mpmc")]
pub use breaker::AtomicBreakerMPMC;

#[cfg(feature = "circuit-breaker-auto-tune")]
pub use breaker::MetricsSnapshot;

pub use layout::{DefaultLayout, STANDARD64_V1};

#[cfg(feature = "circuit-breaker-auto-tune")]
pub use telemetry::{
    generate_all, tune_policy, ActionOutcome, AutoCalibrator, CalibrationMode, CalibrationTargets,
    HistoryBuffer, HistoryEntry, LevelFeedback, LevelFeedbackConfig, LevelFeedbackResult,
    LevelTarget, MetricsTap, PolicyDraft, ScenarioData, ScenarioKind, WindowMetrics,
};

#[cfg(feature = "std")]
pub use telemetry::{MockSource, TelemetrySample, TelemetrySource};

#[cfg(all(feature = "circuit-breaker-pmu", target_os = "linux"))]
pub use telemetry::{PmuCollector, PmuConfig};

// Re-export policy and evaluation
pub use policy::{evaluate, Policy};

#[cfg(feature = "circuit-breaker-adaptive")]
pub use policy::{evaluate_adaptive, AdaptiveState};

#[cfg(feature = "circuit-breaker-auto-tune")]
pub use policy::{evaluate_with_observers, EvaluationObservers};

// Re-export serialization (feature-gated)
#[cfg(feature = "capsule-serialize")]
pub use serialize::BreakerStateSnapshot;
