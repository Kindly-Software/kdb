//! Hysteresis policy evaluation for the atomic breaker.

use crate::breaker::{AtomicBreakerGuard, BreakerLike, LayoutKind, State};
use crate::cause;
#[cfg(feature = "compact48")]
use crate::layout::pack_q6_10;
use crate::layout::pack_q8_8;
#[cfg(feature = "std")]
use crate::telemetry::TelemetrySample;
#[cfg(feature = "auto_tune")]
use crate::telemetry::{
    tune_policy as calibrate_policy, CalibrationMode, CalibrationTargets, HistoryBuffer,
    HistoryEntry, LevelFeedbackResult, MetricsTap, PolicyDraft,
};
#[cfg(feature = "auto_tune")]
use crate::MetricsSnapshot;

/// Policy thresholds expressed in normalized Q8.8 fixed-point units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    /// Trip threshold for normalized mean metric (Q8.8).
    pub mu_trip: u16,
    /// Trip threshold for normalized jitter metric (Q8.8).
    pub sg_trip: u16,
    /// Close threshold for normalized mean metric (Q8.8).
    pub mu_close: u16,
    /// Close threshold for normalized jitter metric (Q8.8).
    pub sg_close: u16,
    /// Minimum time to stay open before probing (milliseconds).
    pub cool_down_ms: u32,
    /// Time window that metrics must remain healthy before closing (milliseconds).
    pub ok_window_ms: u32,
    /// Error count threshold for trips.
    pub err_trip: u16,
}

impl Policy {
    const Q8_SCALE: f32 = 256.0;

    /// Policy tuned for holographic user interfaces.
    #[must_use]
    pub const fn ui_holographic() -> Self {
        Self {
            mu_trip: 4608,
            sg_trip: 4096,
            mu_close: 2048,
            sg_close: 1536,
            cool_down_ms: 75,
            ok_window_ms: 16,
            err_trip: 20,
        }
    }

    /// Policy tuned for low-latency audio rendering pipelines.
    #[must_use]
    pub const fn audio_lowlatency() -> Self {
        Self {
            mu_trip: 5120,
            sg_trip: 3328,
            mu_close: 1792,
            sg_close: 1280,
            cool_down_ms: 20,
            ok_window_ms: 10,
            err_trip: 8,
        }
    }

    /// Policy tuned for IO-bound workloads.
    #[must_use]
    pub const fn io_disk() -> Self {
        Self {
            mu_trip: 4096,
            sg_trip: 5120,
            mu_close: 1792,
            sg_close: 2048,
            cool_down_ms: 100,
            ok_window_ms: 50,
            err_trip: 24,
        }
    }

    /// Policy tuned for arbitrage/trading venues.
    #[must_use]
    pub const fn arb_venue() -> Self {
        Self {
            mu_trip: 5888,
            sg_trip: 4864,
            mu_close: 2304,
            sg_close: 2048,
            cool_down_ms: 40,
            ok_window_ms: 12,
            err_trip: 6,
        }
    }

    #[must_use]
    fn mu_trip_f32(&self) -> f32 {
        f32::from(self.mu_trip) / Self::Q8_SCALE
    }

    #[must_use]
    fn sg_trip_f32(&self) -> f32 {
        f32::from(self.sg_trip) / Self::Q8_SCALE
    }

    #[must_use]
    fn mu_close_f32(&self) -> f32 {
        f32::from(self.mu_close) / Self::Q8_SCALE
    }

    #[must_use]
    fn sg_close_f32(&self) -> f32 {
        f32::from(self.sg_close) / Self::Q8_SCALE
    }
}

#[cfg(feature = "auto_tune")]
/// Optional observers for recording evaluations when adaptive tuning is enabled.
pub struct EvaluationObservers<'a> {
    /// Sliding history buffer for transition analysis.
    pub history: Option<&'a mut HistoryBuffer>,
    /// User-supplied metrics tap for workload feedback.
    pub metrics_tap: Option<&'a mut dyn MetricsTap>,
}

#[cfg(feature = "auto_tune")]
impl EvaluationObservers<'_> {
    /// Construct observers with no hooks.
    #[must_use]
    pub fn none() -> Self {
        Self {
            history: None,
            metrics_tap: None,
        }
    }
}

/// Evaluate the breaker state machine according to the provided policy.
#[allow(clippy::too_many_arguments)]
pub fn evaluate<B: BreakerLike>(
    breaker: &B,
    mu_norm: f32,
    sg_norm: f32,
    err_inc: u16,
    now_ms: u32,
    last_change_ms: &mut u32,
    policy: &Policy,
) {
    let layout = breaker.layout_kind();
    let packed = breaker.load_relaxed();
    let guard = AtomicBreakerGuard::from_layout(packed, layout);

    let mut err_total = guard.err();
    let err_cap = match layout {
        LayoutKind::Standard64 => 0x3fff,
        #[cfg(feature = "compact48")]
        LayoutKind::Compact48 => 0x0fff,
    };

    let mu_q = match layout {
        LayoutKind::Standard64 => pack_q8_8(mu_norm),
        #[cfg(feature = "compact48")]
        LayoutKind::Compact48 => pack_q6_10(mu_norm),
    };
    let sg_q = match layout {
        LayoutKind::Standard64 => pack_q8_8(sg_norm),
        #[cfg(feature = "compact48")]
        LayoutKind::Compact48 => pack_q6_10(sg_norm),
    };

    let mu_high = mu_norm > policy.mu_trip_f32();
    let sg_high = sg_norm > policy.sg_trip_f32();
    let mu_ok = mu_norm < policy.mu_close_f32();
    let sg_ok = sg_norm < policy.sg_close_f32();

    let mut cause_flags = 0u8;
    if mu_high {
        cause_flags |= cause::LAT;
    }
    if sg_high {
        cause_flags |= cause::JIT;
    }

    err_total = err_total.saturating_add(err_inc).min(err_cap);
    let mut trip_due_to_error = false;
    if policy.err_trip > 0 && err_total >= policy.err_trip {
        trip_due_to_error = true;
        cause_flags |= cause::IO;
    }

    if cause_flags == 0 {
        cause_flags = guard.cause();
    }

    let state = guard.state();
    let mut new_state = state;
    let mut backoff = guard.backoff();
    let elapsed = now_ms.wrapping_sub(*last_change_ms);
    let should_trip = mu_high || sg_high || trip_due_to_error;
    let mut reset_error = false;

    match state {
        State::Closed => {
            if should_trip {
                new_state = State::Open;
            }
        }
        State::Open => {
            if !should_trip && elapsed >= policy.cool_down_ms {
                new_state = State::HalfOpen;
            }
        }
        State::HalfOpen => {
            if should_trip {
                new_state = State::Open;
            } else if mu_ok && sg_ok && elapsed >= policy.ok_window_ms {
                new_state = State::Closed;
                reset_error = true;
                cause_flags = 0;
            }
        }
        State::ForcedOpen => {
            // Sticky state; operator intervention required.
            new_state = State::ForcedOpen;
        }
    }

    if new_state == State::Open && state != State::Open {
        backoff = backoff.saturating_add(1).min(63);
    } else if matches!(new_state, State::Closed) {
        backoff = 0;
    }

    if reset_error {
        breaker.clear_error();
        err_total = err_inc.min(err_cap);
    }

    let err_ratio = if policy.err_trip > 0 {
        f32::from(err_total) / f32::from(policy.err_trip)
    } else {
        0.0
    };

    let mut desired_level = derive_level(mu_norm, sg_norm, err_ratio);
    if matches!(new_state, State::Open) {
        desired_level = desired_level.max(2);
    }
    if matches!(new_state, State::HalfOpen) {
        desired_level = desired_level.max(1);
    }
    if matches!(new_state, State::ForcedOpen) {
        desired_level = 3;
    }

    let current_level = guard.level();
    if desired_level < current_level && !(mu_ok && sg_ok) {
        desired_level = current_level;
    }
    desired_level = desired_level.min(3);

    breaker.update_metrics(err_inc, mu_q, sg_q, cause_flags, backoff);

    if new_state != state || desired_level != current_level {
        breaker.set_state_level(new_state, desired_level);
        *last_change_ms = now_ms;
    }
}

fn derive_level(mu_norm: f32, sg_norm: f32, err_ratio: f32) -> u8 {
    let mut level = 0u8;
    if mu_norm >= 3.0 || sg_norm >= 3.0 {
        level = 3;
    } else if mu_norm >= 2.0 || sg_norm >= 2.0 {
        level = 2;
    } else if mu_norm >= 1.15 || sg_norm >= 1.15 {
        level = 1;
    }

    if err_ratio >= 1.0 {
        level = 3;
    } else if err_ratio >= 0.75 {
        level = level.max(2);
    } else if err_ratio >= 0.5 {
        level = level.max(1);
    }

    level
}

#[cfg(feature = "auto_tune")]
fn record_observers(
    now_ms: u32,
    last_change_prev: u32,
    mu_norm: f32,
    sg_norm: f32,
    err_inc: u16,
    policy: &Policy,
    observers: &mut EvaluationObservers<'_>,
    before_guard: AtomicBreakerGuard,
    after_guard: AtomicBreakerGuard,
    before_snapshot: MetricsSnapshot,
    after_snapshot: MetricsSnapshot,
) {
    use crate::breaker::State;

    let changed =
        after_guard.state() != before_guard.state() || after_guard.level() != before_guard.level();

    let recorded_sample = TelemetrySample {
        mu_norm,
        sg_norm,
        err_inc,
        cause: after_snapshot.cause,
        backoff_hint: if after_snapshot.backoff > 0 {
            Some(after_snapshot.backoff)
        } else {
            None
        },
    };

    let action_outcome = if let Some(tap) = observers.metrics_tap.as_mut() {
        tap.record_transition(now_ms, &before_snapshot, &after_snapshot, &recorded_sample)
    } else {
        None
    };

    if changed {
        if let Some(history) = observers.history.as_mut() {
            let success = after_guard.state() == State::Closed
                && mu_norm < policy.mu_close_f32()
                && sg_norm < policy.sg_close_f32()
                && err_inc == 0;
            let entry = HistoryEntry {
                timestamp_ms: now_ms,
                prev_state: before_guard.state(),
                next_state: after_guard.state(),
                prev_level: before_guard.level(),
                next_level: after_guard.level(),
                dwell_ms: now_ms.wrapping_sub(last_change_prev),
                success,
                before: before_snapshot,
                after: after_snapshot,
                sample: recorded_sample,
                action_outcome,
            };
            history.record(entry);
        }
    }
}

/// Apply telemetry data and evaluate the breaker in a single step.
#[cfg(feature = "std")]
pub fn evaluate_with_telemetry<B: BreakerLike>(
    breaker: &B,
    sample: &TelemetrySample,
    now_ms: u32,
    last_change_ms: &mut u32,
    policy: &Policy,
) {
    breaker.apply_sample(sample);
    evaluate(
        breaker,
        sample.mu_norm,
        sample.sg_norm,
        sample.err_inc,
        now_ms,
        last_change_ms,
        policy,
    );
}

#[cfg(feature = "auto_tune")]
#[allow(clippy::too_many_arguments)]
/// Evaluate breaker logic while recording observer hooks (raw metrics version).
pub fn evaluate_with_observers<B: BreakerLike>(
    breaker: &B,
    mu_norm: f32,
    sg_norm: f32,
    err_inc: u16,
    now_ms: u32,
    last_change_ms: &mut u32,
    policy: &Policy,
    observers: &mut EvaluationObservers<'_>,
) {
    let layout = breaker.layout_kind();
    let before_guard = AtomicBreakerGuard::from_layout(breaker.load_relaxed(), layout);
    let before_snapshot = before_guard.metrics_snapshot();
    let last_change_prev = *last_change_ms;
    evaluate(
        breaker,
        mu_norm,
        sg_norm,
        err_inc,
        now_ms,
        last_change_ms,
        policy,
    );
    let after_guard = AtomicBreakerGuard::from_layout(breaker.load_relaxed(), layout);
    let after_snapshot = after_guard.metrics_snapshot();
    record_observers(
        now_ms,
        last_change_prev,
        mu_norm,
        sg_norm,
        err_inc,
        policy,
        observers,
        before_guard,
        after_guard,
        before_snapshot,
        after_snapshot,
    );
}

#[cfg(feature = "auto_tune")]
/// Evaluate with telemetry while recording observer hooks.
pub fn evaluate_with_telemetry_and_observers<B: BreakerLike>(
    breaker: &B,
    sample: &TelemetrySample,
    now_ms: u32,
    last_change_ms: &mut u32,
    policy: &Policy,
    observers: &mut EvaluationObservers<'_>,
) {
    let layout = breaker.layout_kind();
    let before_guard = AtomicBreakerGuard::from_layout(breaker.load_relaxed(), layout);
    let before_snapshot = before_guard.metrics_snapshot();
    let last_change_prev = *last_change_ms;
    evaluate_with_telemetry(breaker, sample, now_ms, last_change_ms, policy);
    let after_guard = AtomicBreakerGuard::from_layout(breaker.load_relaxed(), layout);
    let after_snapshot = after_guard.metrics_snapshot();
    record_observers(
        now_ms,
        last_change_prev,
        sample.mu_norm,
        sample.sg_norm,
        sample.err_inc,
        policy,
        observers,
        before_guard,
        after_guard,
        before_snapshot,
        after_snapshot,
    );
}

#[cfg(feature = "auto_tune")]
/// Run the auto-calibration loop in offline mode using the supplied history buffer.
#[must_use]
pub fn tune(
    history: &HistoryBuffer,
    baseline: &Policy,
    targets: &CalibrationTargets,
) -> Option<PolicyDraft> {
    tune_with_mode(history, baseline, targets, CalibrationMode::Offline)
}

#[cfg(feature = "auto_tune")]
/// Run the auto-calibration loop with an explicit mode (warm-up or offline).
#[must_use]
pub fn tune_with_mode(
    history: &HistoryBuffer,
    baseline: &Policy,
    targets: &CalibrationTargets,
    mode: CalibrationMode,
) -> Option<PolicyDraft> {
    calibrate_policy(history, baseline, targets, mode)
}

#[cfg(feature = "auto_tune")]
fn apply_delta(value: &mut u32, delta: i32) {
    if delta > 0 {
        *value = value.saturating_add(delta as u32);
    } else if delta < 0 {
        let reduce = (-delta) as u32;
        *value = value.saturating_sub(reduce);
    }
}

#[cfg(feature = "auto_tune")]
/// Apply level feedback recommendations to the policy dwell configuration.
#[must_use]
pub fn adjust_dwell(policy: &mut Policy, feedback: &LevelFeedbackResult) -> Option<u8> {
    apply_delta(&mut policy.cool_down_ms, feedback.cool_down_delta_ms);
    apply_delta(&mut policy.ok_window_ms, feedback.ok_window_delta_ms);
    if policy.ok_window_ms == 0 {
        policy.ok_window_ms = 1;
    }
    feedback.backoff_hint
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::breaker::{AtomicBreakerGuard, AtomicBreakerSWeMR, State};
    #[cfg(feature = "auto_tune")]
    use crate::telemetry::TelemetrySample;
    #[cfg(feature = "auto_tune")]
    use crate::telemetry::{ActionOutcome, HistoryBuffer};
    #[cfg(feature = "auto_tune")]
    use crate::MetricsSnapshot;
    use proptest::prelude::*;

    #[derive(Clone, Debug)]
    struct Stimulus {
        mu: f32,
        sg: f32,
        err_inc: u16,
        dt: u32,
    }

    prop_compose! {
        fn arb_stimulus()(mu in 0.0_f32..4.0, sg in 0.0_f32..4.0, err in 0u16..16, dt in 0u32..200) -> Stimulus {
            Stimulus { mu, sg, err_inc: err, dt }
        }
    }

    #[test]
    fn policy_trips_and_recovers() {
        let policy = Policy::ui_holographic();
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
        let mut last_change = 0u32;

        evaluate(&breaker, 20.0, 0.4, 4, 10, &mut last_change, &policy);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.state(), State::Open);
        assert!(last_change >= 10);

        let half_time = last_change + policy.cool_down_ms + 1;
        evaluate(&breaker, 0.9, 0.8, 0, half_time, &mut last_change, &policy);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.state(), State::HalfOpen);
        assert!(guard.level() >= 1);

        let close_time = last_change + policy.ok_window_ms + 2;
        evaluate(&breaker, 0.6, 0.5, 0, close_time, &mut last_change, &policy);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.state(), State::Closed);
        assert_eq!(guard.err(), 0);
        assert!(last_change >= close_time);
    }

    #[test]
    fn forced_open_remains_sticky() {
        let policy = Policy::audio_lowlatency();
        let breaker = AtomicBreakerSWeMR::new_standard64(State::ForcedOpen);
        let mut last_change = 0u32;

        evaluate(&breaker, 0.5, 0.4, 0, 5_000, &mut last_change, &policy);
        let guard = AtomicBreakerGuard::new(breaker.load_relaxed());
        assert_eq!(guard.state(), State::ForcedOpen);
        assert_eq!(guard.level(), 3);
        assert_eq!(last_change, 5_000);
    }

    #[test]
    fn policy_profiles_are_distinct() {
        let ui = Policy::ui_holographic();
        let audio = Policy::audio_lowlatency();
        let io = Policy::io_disk();
        let arb = Policy::arb_venue();

        assert_ne!(ui.mu_trip, audio.mu_trip);
        assert_ne!(io.sg_trip, ui.sg_trip);
        assert_ne!(arb.err_trip, io.err_trip);
    }

    proptest! {
        #[test]
        fn state_machine_remains_legal(stimuli in proptest::collection::vec(arb_stimulus(), 1..50)) {
            let policy = Policy::io_disk();
            let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
            let mut last_change = 0u32;
            let mut now = 0u32;

            for stimulus in stimuli {
                now = now.wrapping_add(stimulus.dt.max(1));
                evaluate(
                    &breaker,
                    stimulus.mu,
                    stimulus.sg,
                    stimulus.err_inc,
                    now,
                    &mut last_change,
                    &policy,
                );

                let packed = breaker.load_relaxed();
                let guard = AtomicBreakerGuard::new(packed);

                // Levels are always within encoded bounds.
                prop_assert!(guard.level() <= 3);
                // Forced open must pin the level to 3.
                if guard.state() == State::ForcedOpen {
                    prop_assert_eq!(guard.level(), 3);
                }
                // Ensure last_change only moves forward when state/level update.
                prop_assert!(now >= last_change);
            }
        }
    }

    #[cfg(feature = "auto_tune")]
    struct CountingTap {
        calls: usize,
        last_next_state: Option<State>,
    }

    #[cfg(feature = "auto_tune")]
    impl CountingTap {
        fn new() -> Self {
            Self {
                calls: 0,
                last_next_state: None,
            }
        }
    }

    #[cfg(feature = "auto_tune")]
    impl MetricsTap for CountingTap {
        fn record_transition(
            &mut self,
            _now_ms: u32,
            _before: &MetricsSnapshot,
            after: &MetricsSnapshot,
            _sample: &TelemetrySample,
        ) -> Option<ActionOutcome> {
            self.calls += 1;
            self.last_next_state = Some(after.state);
            Some(ActionOutcome {
                recovered_within_target: after.state == State::Closed,
                observed_recovery_ms: Some(90),
            })
        }
    }

    #[cfg(feature = "auto_tune")]
    #[test]
    fn observers_capture_history_on_transition() {
        let policy = Policy::ui_holographic();
        let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
        let mut last_change = 0u32;
        let mut history = HistoryBuffer::new(4);
        let mut tap = CountingTap::new();
        let mut observers = EvaluationObservers {
            history: Some(&mut history),
            metrics_tap: Some(&mut tap),
        };

        evaluate_with_observers(
            &breaker,
            20.0,
            0.5,
            3,
            1,
            &mut last_change,
            &policy,
            &mut observers,
        );

        drop(observers);

        assert_eq!(tap.calls, 1);
        assert_eq!(tap.last_next_state, Some(State::Open));
        assert_eq!(history.len(), 1);
        let entry = history.iter().next().unwrap();
        assert_eq!(entry.prev_state, State::Closed);
        assert_eq!(entry.next_state, State::Open);
        assert_eq!(entry.dwell_ms, 1);
    }

    #[cfg(feature = "auto_tune")]
    fn synthetic_history(mu: f32, sg: f32, success: bool) -> HistoryEntry {
        let snapshot = MetricsSnapshot {
            state: if success { State::Closed } else { State::Open },
            level: if success { 0 } else { 2 },
            err: if success { 1 } else { 24 },
            mu_norm: mu,
            sg_norm: sg,
            cause: 0,
            backoff: 0,
        };
        HistoryEntry {
            timestamp_ms: 0,
            prev_state: State::Closed,
            next_state: snapshot.state,
            prev_level: 0,
            next_level: snapshot.level,
            dwell_ms: 10,
            success,
            before: snapshot,
            after: snapshot,
            sample: TelemetrySample {
                mu_norm: mu,
                sg_norm: sg,
                err_inc: if success { 0 } else { 5 },
                cause: 0,
                backoff_hint: None,
            },
            action_outcome: Some(ActionOutcome {
                recovered_within_target: success,
                observed_recovery_ms: Some(if success { 45 } else { 160 }),
            }),
        }
    }

    #[cfg(feature = "auto_tune")]
    #[test]
    fn tune_returns_adjusted_policy() {
        let mut history = HistoryBuffer::new(4);
        history.record(synthetic_history(3.2, 2.4, false));
        history.record(synthetic_history(2.8, 2.0, false));
        let baseline = Policy::arb_venue();
        let targets = CalibrationTargets {
            success_rate: 0.7,
            max_transitions_per_min: 4.0,
            mu_p95_target: 1.5,
            sg_p95_target: 1.2,
            err_trip_target: 6,
        };
        let result = tune(&history, &baseline, &targets).expect("expected tune result");
        assert!(result.policy.mu_trip < baseline.mu_trip);
    }

    #[cfg(feature = "auto_tune")]
    #[test]
    fn adjust_dwell_applies_deltas() {
        let mut policy = Policy::audio_lowlatency();
        let original = policy;
        let feedback = LevelFeedbackResult {
            cool_down_delta_ms: 15,
            ok_window_delta_ms: -3,
            backoff_hint: Some(5),
            notes: Vec::new(),
        };
        let hint = adjust_dwell(&mut policy, &feedback);
        assert_eq!(policy.cool_down_ms, original.cool_down_ms + 15);
        assert_eq!(policy.ok_window_ms, original.ok_window_ms.saturating_sub(3));
        assert_eq!(hint, Some(5));
    }
}
