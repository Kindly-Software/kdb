//! Auto-calibration primitives for adaptive breaker policies.
//!
//! The analyser operates entirely off the optional `HistoryBuffer` ring without touching the
//! hot word. All computations live under the `auto_tune` feature.

use crate::policy::Policy;
use crate::telemetry::HistoryBuffer;
use std::cmp::Ordering;

/// Desired behavioural targets for the auto-calibration routine.
#[derive(Clone, Copy, Debug)]
pub struct CalibrationTargets {
    /// Minimum ratio of successful transitions (0.0 - 1.0).
    pub success_rate: f32,
    /// Maximum acceptable transitions per minute before considering the breaker too twitchy.
    pub max_transitions_per_min: f32,
    /// Desired 95th percentile bound for the normalized mu metric.
    pub mu_p95_target: f32,
    /// Desired 95th percentile bound for the normalized sigma metric.
    pub sg_p95_target: f32,
    /// Desired long-run error trip threshold (acts as centre of gravity for `err_trip`).
    pub err_trip_target: u16,
}

impl Default for CalibrationTargets {
    fn default() -> Self {
        Self {
            success_rate: 0.6,
            max_transitions_per_min: 4.0,
            mu_p95_target: 2.0,
            sg_p95_target: 1.5,
            err_trip_target: 12,
        }
    }
}

/// Mode in which the calibrator operates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalibrationMode {
    /// Only emit adjustments after a warm-up period with sufficient samples.
    WarmUp {
        /// Minimum number of observations required before issuing adjustments.
        min_observations: usize,
    },
    /// Always evaluate the supplied history (offline analysis).
    Offline,
}

impl CalibrationMode {
    fn satisfied(&self, observations: usize) -> bool {
        match *self {
            Self::WarmUp { min_observations } => observations >= min_observations,
            Self::Offline => true,
        }
    }
}

/// Captured results of an auto-calibration pass.
#[derive(Clone, Debug)]
pub struct PolicyDraft {
    /// Tuned policy to be reviewed by operators.
    pub policy: Policy,
    /// Human-readable notes detailing which adjustments fired.
    pub notes: Vec<String>,
    /// Metrics aggregated over the analysed window.
    pub metrics: WindowMetrics,
}

/// Aggregated statistics derived from the history window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowMetrics {
    /// Count of entries analysed.
    pub observations: usize,
    /// Ratio of successful transitions.
    pub success_rate: f32,
    /// Transitions per minute.
    pub transitions_per_min: f32,
    /// 95th percentile of the mu ratio.
    pub mu_p95: f32,
    /// 95th percentile of the sigma ratio.
    pub sg_p95: f32,
    /// Average terminal error counter from the samples.
    pub avg_err: f32,
}

/// Sliding-window auto-calibrator that nudges policy thresholds towards target behaviour.
#[derive(Clone, Debug)]
pub struct AutoCalibrator {
    mu_step: u16,
    err_step: u16,
    mode: CalibrationMode,
}

impl AutoCalibrator {
    /// Construct a calibrator with default step sizes (≈0.25 in normalized units).
    #[must_use]
    pub fn new(mode: CalibrationMode) -> Self {
        Self {
            mu_step: 64, // 0.25 in Q8.8
            err_step: 2,
            mode,
        }
    }

    /// Override the fixed-point step size used when nudging thresholds.
    #[must_use]
    pub fn with_steps(mut self, mu_step: u16, err_step: u16) -> Self {
        self.mu_step = mu_step.max(1);
        self.err_step = err_step.max(1);
        self
    }

    /// Analyse the provided history buffer and emit a tuned policy draft when targets warrant it.
    #[must_use]
    pub fn tune(
        &self,
        history: &HistoryBuffer,
        baseline: &Policy,
        targets: &CalibrationTargets,
    ) -> Option<PolicyDraft> {
        if history.is_empty() {
            return None;
        }
        let metrics = compute_window_metrics(history);
        if !self.mode.satisfied(metrics.observations) {
            return None;
        }

        let mut policy = *baseline;
        let mut notes = Vec::new();

        adjust_for_transition_rate(
            &mut policy,
            metrics.transitions_per_min,
            targets.max_transitions_per_min,
            self.mu_step,
            self.err_step,
            &mut notes,
        );

        adjust_for_success_ratio(
            &mut policy,
            metrics.success_rate,
            targets.success_rate,
            self.mu_step,
            self.err_step,
            &mut notes,
        );

        adjust_for_percentiles(
            &mut policy,
            metrics.mu_p95,
            metrics.sg_p95,
            targets,
            self.mu_step,
            &mut notes,
        );

        adjust_for_error_budget(
            &mut policy,
            metrics.avg_err,
            targets.err_trip_target,
            self.err_step,
            &mut notes,
        );

        if notes.is_empty() {
            return None;
        }

        Some(PolicyDraft {
            policy,
            notes,
            metrics,
        })
    }
}

fn compute_window_metrics(history: &HistoryBuffer) -> WindowMetrics {
    let mut mu_values = Vec::new();
    let mut sg_values = Vec::new();
    let mut err_sum = 0f32;
    let mut successes = 0usize;

    let mut first_ts = None;
    let mut last_ts = None;

    for entry in history.iter() {
        mu_values.push(entry.after.mu_norm);
        sg_values.push(entry.after.sg_norm);
        err_sum += f32::from(entry.after.err);
        if entry.success {
            successes += 1;
        }
        first_ts = Some(first_ts.unwrap_or(entry.timestamp_ms));
        last_ts = Some(entry.timestamp_ms);
    }

    let observations = history.len();
    let success_rate = if observations > 0 {
        successes as f32 / observations as f32
    } else {
        0.0
    };
    let mu_p95 = percentile(&mut mu_values, 0.95);
    let sg_p95 = percentile(&mut sg_values, 0.95);
    let avg_err = if observations > 0 {
        err_sum / observations as f32
    } else {
        0.0
    };

    let transitions_per_min = match (first_ts, last_ts) {
        (Some(first), Some(last)) if last > first => {
            let duration_ms = (last - first).max(1) as f32;
            (observations as f32) * 60_000.0 / duration_ms
        }
        _ => observations as f32,
    };

    WindowMetrics {
        observations,
        success_rate,
        transitions_per_min,
        mu_p95,
        sg_p95,
        avg_err,
    }
}

fn percentile(values: &mut [f32], percentile: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| ordered_float(*a, *b));
    let clamped = percentile.clamp(0.0, 1.0);
    let idx = ((values.len() - 1) as f32 * clamped).round() as usize;
    values[idx]
}

fn ordered_float(a: f32, b: f32) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

fn adjust_for_transition_rate(
    policy: &mut Policy,
    observed: f32,
    max_allowed: f32,
    mu_step: u16,
    err_step: u16,
    notes: &mut Vec<String>,
) {
    if observed > max_allowed {
        policy.mu_trip = policy.mu_trip.saturating_add(mu_step);
        policy.sg_trip = policy.sg_trip.saturating_add(mu_step);
        policy.err_trip = policy.err_trip.saturating_add(err_step).min(0x3fff);
        notes.push(format!(
            "reduced flicker: transitions {observed:.2}/min > {max_allowed:.2} → raised trip thresholds"
        ));
    }
}

fn adjust_for_success_ratio(
    policy: &mut Policy,
    observed: f32,
    target: f32,
    mu_step: u16,
    err_step: u16,
    notes: &mut Vec<String>,
) {
    if observed < target {
        policy.mu_trip = policy.mu_trip.saturating_sub(mu_step);
        policy.sg_trip = policy.sg_trip.saturating_sub(mu_step);
        policy.err_trip = policy.err_trip.saturating_sub(err_step);
        notes.push(format!(
            "boosted sensitivity: success rate {observed:.2} < {target:.2}"
        ));
    }
}

fn adjust_for_percentiles(
    policy: &mut Policy,
    mu_p95: f32,
    sg_p95: f32,
    targets: &CalibrationTargets,
    mu_step: u16,
    notes: &mut Vec<String>,
) {
    if mu_p95 > targets.mu_p95_target {
        policy.mu_trip = policy.mu_trip.saturating_sub(mu_step);
        notes.push(format!(
            "tightened mu_trip: p95 {:.2} > target {:.2}",
            mu_p95, targets.mu_p95_target
        ));
    } else if mu_p95 < targets.mu_p95_target * 0.6 {
        policy.mu_trip = policy.mu_trip.saturating_add(mu_step);
        notes.push(format!(
            "relaxed mu_trip: p95 {:.2} well below target {:.2}",
            mu_p95, targets.mu_p95_target
        ));
    }

    if sg_p95 > targets.sg_p95_target {
        policy.sg_trip = policy.sg_trip.saturating_sub(mu_step);
        notes.push(format!(
            "tightened sg_trip: p95 {:.2} > target {:.2}",
            sg_p95, targets.sg_p95_target
        ));
    } else if sg_p95 < targets.sg_p95_target * 0.6 {
        policy.sg_trip = policy.sg_trip.saturating_add(mu_step);
        notes.push(format!(
            "relaxed sg_trip: p95 {:.2} well below target {:.2}",
            sg_p95, targets.sg_p95_target
        ));
    }
}

fn adjust_for_error_budget(
    policy: &mut Policy,
    avg_err: f32,
    err_target: u16,
    err_step: u16,
    notes: &mut Vec<String>,
) {
    let err_target_f = f32::from(err_target.max(1));
    if avg_err > err_target_f {
        policy.err_trip = policy.err_trip.saturating_sub(err_step);
        notes.push(format!(
            "tightened err_trip: avg {avg_err:.1} > target {err_target}"
        ));
    } else if avg_err < err_target_f * 0.5 {
        policy.err_trip = (policy.err_trip + err_step).min(0x3fff);
        notes.push(format!(
            "relaxed err_trip: avg {avg_err:.1} < 0.5 * target {err_target}"
        ));
    }
}

/// Run the auto-calibrator and return both the draft policy and metrics (helper for policy module).
#[must_use]
pub fn tune_policy(
    history: &HistoryBuffer,
    baseline: &Policy,
    targets: &CalibrationTargets,
    mode: CalibrationMode,
) -> Option<PolicyDraft> {
    AutoCalibrator::new(mode).tune(history, baseline, targets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::breaker::State;
    use crate::layout::pack_q8_8;
    use crate::telemetry::history::HistoryEntry;
    use crate::telemetry::ActionOutcome;
    use crate::telemetry::TelemetrySample;

    fn make_entry(ts: u32, mu: f32, sg: f32, success: bool) -> HistoryEntry {
        let snapshot = crate::breaker::MetricsSnapshot {
            state: if success { State::Closed } else { State::Open },
            level: if success { 0 } else { 2 },
            err: if success { 1 } else { 20 },
            mu_norm: mu,
            sg_norm: sg,
            cause: 0,
            backoff: 0,
        };
        HistoryEntry {
            timestamp_ms: ts,
            prev_state: State::Closed,
            next_state: if success { State::Closed } else { State::Open },
            prev_level: 0,
            next_level: if success { 0 } else { 2 },
            dwell_ms: 10,
            success,
            before: snapshot,
            after: snapshot,
            sample: TelemetrySample {
                mu_norm: mu,
                sg_norm: sg,
                err_inc: if success { 0 } else { 4 },
                cause: 0,
                backoff_hint: None,
            },
            action_outcome: Some(ActionOutcome {
                recovered_within_target: success,
                observed_recovery_ms: Some(if success { 60 } else { 220 }),
            }),
        }
    }

    fn baseline_policy() -> Policy {
        Policy {
            mu_trip: pack_q8_8(2.5),
            sg_trip: pack_q8_8(2.0),
            mu_close: pack_q8_8(1.0),
            sg_close: pack_q8_8(0.8),
            cool_down_ms: 50,
            ok_window_ms: 10,
            err_trip: 16,
        }
    }

    #[test]
    fn warmup_mode_respects_minimum_observations() {
        let mut history = HistoryBuffer::new(4);
        history.record(make_entry(1, 1.0, 1.0, true));
        let calibrator = AutoCalibrator::new(CalibrationMode::WarmUp {
            min_observations: 2,
        });
        assert!(calibrator
            .tune(&history, &baseline_policy(), &CalibrationTargets::default())
            .is_none());
    }

    #[test]
    fn offline_mode_emits_draft_when_targets_violated() {
        let mut history = HistoryBuffer::new(8);
        for idx in 0..5 {
            history.record(make_entry(idx * 10, 3.0, 2.5, false));
        }
        let calibrator = AutoCalibrator::new(CalibrationMode::Offline);
        let draft = calibrator
            .tune(&history, &baseline_policy(), &CalibrationTargets::default())
            .expect("expected adjustments");
        assert!(draft.policy.mu_trip < baseline_policy().mu_trip);
        assert!(!draft.notes.is_empty());
        assert!(draft.metrics.mu_p95 >= 3.0);
    }

    #[test]
    fn adjustments_handle_low_transition_rate() {
        let mut history = HistoryBuffer::new(8);
        for idx in 0..5 {
            history.record(make_entry(idx * 60_000, 0.4, 0.3, true));
        }
        let targets = CalibrationTargets {
            success_rate: 0.9,
            max_transitions_per_min: 1.0,
            mu_p95_target: 1.0,
            sg_p95_target: 0.8,
            err_trip_target: 4,
        };
        let calibrator = AutoCalibrator::new(CalibrationMode::Offline).with_steps(32, 1);
        let draft = calibrator.tune(&history, &baseline_policy(), &targets);
        assert!(draft.is_some());
    }
}
