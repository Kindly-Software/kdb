//! Synthetic telemetry scenarios for exercising the auto-calibration stack.
//!
//! These generators are meant for tests, examples, and offline tooling so agents can inspect
//! how the calibrator responds to specific operating regimes without capturing real traffic.

use crate::breaker::State;
use crate::telemetry::{ActionOutcome, HistoryBuffer, HistoryEntry, TelemetrySample};
use crate::MetricsSnapshot;

/// Fixed set of synthetic operating regimes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScenarioKind {
    /// Chronic overload with sustained high latency/jitter and failures.
    ChronicOverload,
    /// Breaker rarely trips; metrics stay far below budgets.
    UnderUtilised,
    /// Alternating successes/failures leading to high transition frequency.
    Flicker,
    /// Mostly successful recoveries but brushing close to thresholds.
    MixedRecovery,
    /// Latency within budget but persistent error increments.
    ErrorHeavy,
}

/// Convenience handle combining a scenario label with a populated history buffer.
#[derive(Clone, Debug)]
pub struct ScenarioData {
    /// Scenario classification.
    pub kind: ScenarioKind,
    /// Captured history entries suitable for calibration.
    pub history: HistoryBuffer,
}

impl ScenarioData {
    /// Create a scenario with a default of 32 observations.
    #[must_use]
    pub fn new(kind: ScenarioKind) -> Self {
        Self::with_len(kind, 32)
    }

    /// Create a scenario with a specific number of observations (minimum 4).
    #[must_use]
    pub fn with_len(kind: ScenarioKind, observations: usize) -> Self {
        let len = observations.max(4);
        let mut history = HistoryBuffer::new(len);
        let mut generator = ScenarioGenerator::new(kind);
        for idx in 0..len {
            if let Some(mut entry) = generator.next() {
                entry.timestamp_ms = generator.timestamp(idx as u32);
                history.record(entry);
            }
        }
        Self { kind, history }
    }
}

/// Build synthetic data for all scenarios with a shared observation count.
#[must_use]
pub fn generate_all(observations: usize) -> Vec<ScenarioData> {
    use ScenarioKind::{ChronicOverload, ErrorHeavy, Flicker, MixedRecovery, UnderUtilised};
    vec![
        ScenarioData::with_len(ChronicOverload, observations),
        ScenarioData::with_len(UnderUtilised, observations),
        ScenarioData::with_len(Flicker, observations),
        ScenarioData::with_len(MixedRecovery, observations),
        ScenarioData::with_len(ErrorHeavy, observations),
    ]
}

struct ScenarioGenerator {
    kind: ScenarioKind,
    tick_ms: u32,
}

impl ScenarioGenerator {
    fn new(kind: ScenarioKind) -> Self {
        let tick_ms = match kind {
            ScenarioKind::ChronicOverload => 5,
            ScenarioKind::UnderUtilised => 1_000,
            ScenarioKind::Flicker => 250,
            ScenarioKind::MixedRecovery => 120,
            ScenarioKind::ErrorHeavy => 180,
        };
        Self { kind, tick_ms }
    }

    fn timestamp(&self, idx: u32) -> u32 {
        idx.saturating_mul(self.tick_ms)
    }
}

impl Iterator for ScenarioGenerator {
    type Item = HistoryEntry;

    fn next(&mut self) -> Option<Self::Item> {
        // We rely on the user to bound iteration via `take`, so this iterator is infinite.
        let snapshot = match self.kind {
            ScenarioKind::ChronicOverload => MetricsSnapshot {
                state: State::Open,
                level: 3,
                err: 24,
                mu_norm: 3.0,
                sg_norm: 2.5,
                cause: 0,
                backoff: 12,
            },
            ScenarioKind::UnderUtilised => MetricsSnapshot {
                state: State::Closed,
                level: 0,
                err: 0,
                mu_norm: 0.4,
                sg_norm: 0.3,
                cause: 0,
                backoff: 0,
            },
            ScenarioKind::Flicker => MetricsSnapshot {
                state: State::HalfOpen,
                level: 2,
                err: 8,
                mu_norm: 1.6,
                sg_norm: 1.4,
                cause: 0,
                backoff: 6,
            },
            ScenarioKind::MixedRecovery => MetricsSnapshot {
                state: State::HalfOpen,
                level: 1,
                err: 10,
                mu_norm: 1.4,
                sg_norm: 1.2,
                cause: 0,
                backoff: 4,
            },
            ScenarioKind::ErrorHeavy => MetricsSnapshot {
                state: State::Open,
                level: 2,
                err: 28,
                mu_norm: 0.9,
                sg_norm: 0.8,
                cause: 0,
                backoff: 9,
            },
        };

        Some(match self.kind {
            ScenarioKind::ChronicOverload => make_entry(
                snapshot,
                false,
                6,
                Some(ActionOutcome {
                    recovered_within_target: false,
                    observed_recovery_ms: Some(360),
                }),
            ),
            ScenarioKind::UnderUtilised => make_entry(
                snapshot,
                true,
                0,
                Some(ActionOutcome {
                    recovered_within_target: true,
                    observed_recovery_ms: Some(18),
                }),
            ),
            ScenarioKind::Flicker => make_entry(
                snapshot,
                false,
                2,
                Some(ActionOutcome {
                    recovered_within_target: false,
                    observed_recovery_ms: Some(180),
                }),
            ),
            ScenarioKind::MixedRecovery => make_entry(
                snapshot,
                true,
                1,
                Some(ActionOutcome {
                    recovered_within_target: true,
                    observed_recovery_ms: Some(85),
                }),
            ),
            ScenarioKind::ErrorHeavy => make_entry(
                snapshot,
                false,
                8,
                Some(ActionOutcome {
                    recovered_within_target: false,
                    observed_recovery_ms: Some(240),
                }),
            ),
        })
    }
}

fn make_entry(
    snapshot: MetricsSnapshot,
    success: bool,
    err_inc: u16,
    outcome: Option<ActionOutcome>,
) -> HistoryEntry {
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
            mu_norm: snapshot.mu_norm,
            sg_norm: snapshot.sg_norm,
            err_inc,
            cause: 0,
            backoff_hint: Some(snapshot.backoff),
        },
        action_outcome: outcome,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_produces_expected_counts() {
        let scenarios = generate_all(16);
        assert_eq!(scenarios.len(), 5);
        for data in scenarios {
            assert_eq!(data.history.len(), 16);
        }
    }

    #[test]
    fn chronic_overload_contains_failures() {
        let data = ScenarioData::with_len(ScenarioKind::ChronicOverload, 8);
        assert!(data.history.iter().all(|entry| !entry.success));
        let metrics: Vec<_> = data.history.iter().map(|e| e.after.mu_norm).collect();
        assert!(metrics.iter().all(|&mu| mu >= 3.0));
        assert!(data.history.iter().all(|entry| entry
            .action_outcome
            .is_some_and(|outcome| !outcome.recovered_within_target)));
    }

    #[test]
    fn under_utilised_records_successes() {
        let data = ScenarioData::with_len(ScenarioKind::UnderUtilised, 8);
        assert!(data.history.iter().all(|entry| entry.success));
        assert!(data
            .history
            .iter()
            .all(|entry| entry.after.mu_norm < 0.6 && entry.after.sg_norm < 0.5));
        assert!(data.history.iter().all(|entry| entry
            .action_outcome
            .is_some_and(|outcome| outcome.recovered_within_target)));
    }
}
