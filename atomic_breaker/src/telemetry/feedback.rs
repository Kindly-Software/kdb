//! Level-to-action feedback analysis for tuning dwell and backoff behaviour.
//!
//! Consumes `HistoryBuffer` entries annotated with [`ActionOutcome`] to understand whether the
//! workload recovered within its target window for each breaker level.

use crate::telemetry::HistoryBuffer;

/// Target recovery goals for a specific breaker level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelTarget {
    /// Desired maximum recovery window in milliseconds.
    pub recovery_target_ms: u32,
    /// Minimum fraction of observations that should meet the recovery target.
    pub min_success_rate: f32,
}

impl LevelTarget {
    /// Construct a level target.
    #[must_use]
    pub const fn new(recovery_target_ms: u32, min_success_rate: f32) -> Self {
        Self {
            recovery_target_ms,
            min_success_rate,
        }
    }
}

/// Configuration for the level feedback analyser.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelFeedbackConfig {
    /// Per-level targets (L0-L3). `None` disables feedback for that level.
    pub targets: [Option<LevelTarget>; 4],
    /// Step applied to `Policy::cool_down_ms` when tightening/loosening dwell.
    pub cool_down_step_ms: u32,
    /// Step applied to `Policy::ok_window_ms` when tightening/loosening dwell.
    pub ok_window_step_ms: u32,
    /// Increment applied to the breaker backoff hint when dwell proves insufficient.
    pub backoff_step: u8,
}

impl LevelFeedbackConfig {
    /// Balanced defaults suitable for user-facing workloads.
    #[must_use]
    pub fn balanced() -> Self {
        Self {
            targets: [
                Some(LevelTarget::new(40, 0.85)),
                Some(LevelTarget::new(80, 0.75)),
                Some(LevelTarget::new(140, 0.65)),
                Some(LevelTarget::new(200, 0.6)),
            ],
            cool_down_step_ms: 10,
            ok_window_step_ms: 5,
            backoff_step: 1,
        }
    }
}

impl Default for LevelFeedbackConfig {
    fn default() -> Self {
        Self::balanced()
    }
}

/// Summary of the feedback analysis.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LevelFeedbackResult {
    /// Delta to apply to `Policy::cool_down_ms` (positive increases dwell).
    pub cool_down_delta_ms: i32,
    /// Delta to apply to `Policy::ok_window_ms` (positive increases the stability window).
    pub ok_window_delta_ms: i32,
    /// Optional backoff hint (0-63) recommended for subsequent breaker openings.
    pub backoff_hint: Option<u8>,
    /// Notes describing which levels drove the adjustments.
    pub notes: Vec<String>,
}

impl LevelFeedbackResult {
    fn record_failure(
        &mut self,
        config: &LevelFeedbackConfig,
        level: u8,
        success_rate: f32,
        avg_ms: Option<f32>,
        target: &LevelTarget,
    ) {
        self.cool_down_delta_ms += config.cool_down_step_ms as i32;
        self.ok_window_delta_ms += config.ok_window_step_ms as i32;
        let hint = self
            .backoff_hint
            .unwrap_or(0)
            .saturating_add(config.backoff_step)
            .min(63);
        self.backoff_hint = Some(hint);
        self.notes.push(format!(
            "level L{} underperformed (success {:.2}/{:.2}, avg {:?} ms > target {} ms)",
            level,
            success_rate,
            target.min_success_rate,
            avg_ms.map(f32::round),
            target.recovery_target_ms
        ));
    }

    fn record_success(
        &mut self,
        config: &LevelFeedbackConfig,
        level: u8,
        success_rate: f32,
        avg_ms: Option<f32>,
        target: &LevelTarget,
    ) {
        self.cool_down_delta_ms -= config.cool_down_step_ms as i32;
        self.ok_window_delta_ms -= config.ok_window_step_ms as i32;
        self.notes.push(format!(
            "level L{} exceeded targets (success {:.2} > {:.2}, avg {:?} ms)",
            level,
            success_rate,
            target.min_success_rate,
            avg_ms.map(f32::round)
        ));
    }
}

/// Level feedback analyser that converts annotated history into dwell/backoff suggestions.
#[derive(Clone, Debug)]
pub struct LevelFeedback {
    config: LevelFeedbackConfig,
}

impl LevelFeedback {
    /// Create a new analyser.
    #[must_use]
    pub fn new(config: LevelFeedbackConfig) -> Self {
        Self { config }
    }

    /// Analyse the provided history buffer. Returns `None` when no adjustments are warranted.
    #[must_use]
    pub fn analyze(&self, history: &HistoryBuffer) -> Option<LevelFeedbackResult> {
        let mut stats = [LevelStats::default(); 4];

        for entry in history.iter() {
            let Some(outcome) = entry.action_outcome else {
                continue;
            };
            let level_idx = usize::from(entry.next_level.min(3));
            let stat = &mut stats[level_idx];
            stat.total += 1;
            if outcome.recovered_within_target {
                stat.success_within_target += 1;
            }
            if let Some(ms) = outcome.observed_recovery_ms {
                stat.latency_samples += 1;
                stat.total_latency_ms += u64::from(ms);
            }
        }

        let mut result = LevelFeedbackResult::default();
        for level in (0..4).rev() {
            let Some(target) = self.config.targets[level] else {
                continue;
            };
            let stats = &stats[level];
            if stats.total == 0 {
                continue;
            }
            let success_rate = stats.success_rate();
            let avg_ms = stats.average_latency_ms();
            let violates_success = success_rate < target.min_success_rate;
            let violates_latency = avg_ms.is_some_and(|avg| avg > target.recovery_target_ms as f32);

            if violates_success || violates_latency {
                result.record_failure(&self.config, level as u8, success_rate, avg_ms, &target);
                continue;
            }

            let generous_success = (target.min_success_rate + 0.2).min(0.99);
            let latency_threshold = target.recovery_target_ms as f32 * 0.7;
            if success_rate > generous_success
                && avg_ms.map_or(true, |avg| avg <= latency_threshold)
            {
                result.record_success(&self.config, level as u8, success_rate, avg_ms, &target);
            }
        }

        if result.notes.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct LevelStats {
    total: usize,
    success_within_target: usize,
    latency_samples: usize,
    total_latency_ms: u64,
}

impl LevelStats {
    fn success_rate(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.success_within_target as f32 / self.total as f32
        }
    }

    fn average_latency_ms(&self) -> Option<f32> {
        if self.latency_samples == 0 {
            None
        } else {
            Some(self.total_latency_ms as f32 / self.latency_samples as f32)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::{generate_all, ScenarioKind};

    #[test]
    fn failures_raise_dwell() {
        let scenarios = generate_all(16);
        let overload = scenarios
            .into_iter()
            .find(|data| data.kind == ScenarioKind::ChronicOverload)
            .unwrap();
        let feedback = LevelFeedback::new(LevelFeedbackConfig::default())
            .analyze(&overload.history)
            .expect("expected adjustments");
        assert!(feedback.cool_down_delta_ms > 0);
        assert!(feedback.ok_window_delta_ms > 0);
        assert!(feedback.backoff_hint.is_some());
    }

    #[test]
    fn successes_relax_dwell() {
        let scenarios = generate_all(16);
        let under = scenarios
            .into_iter()
            .find(|data| data.kind == ScenarioKind::UnderUtilised)
            .unwrap();
        let feedback = LevelFeedback::new(LevelFeedbackConfig::default())
            .analyze(&under.history)
            .expect("expected relaxation");
        assert!(feedback.cool_down_delta_ms <= 0);
        assert!(feedback.ok_window_delta_ms <= 0);
    }
}
