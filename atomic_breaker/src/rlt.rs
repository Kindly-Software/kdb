//! RLT-1024 integration helpers for breaker evaluation.

use core::fmt;

use atomic_risk_ladder_table::{
    layout::{
        actions::{ActionBases, ActionWord, ActionsWordDraft, AppliedActionSet, RoutePolicy},
        header::{HeaderWord, StrategyMask},
        trips::{TripThresholds, TripWord},
        RecoverScale,
    },
    Rlt1024,
};

#[inline]
fn decode_q2_6(raw: u8) -> f32 {
    f32::from(raw) / 64.0
}

#[inline]
fn decode_q1_7(raw: u8) -> f32 {
    f32::from(raw) / 128.0
}

/// Strategies supported by the RLT-1024 capsule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrategyId {
    /// Strategy A — reversion playbook.
    StrategyA,
    /// Strategy B — momentum / sweep-follow playbook.
    StrategyB,
    /// Strategy C — maker-nibble playbook.
    StrategyC,
}

impl StrategyId {
    const fn mask_bit(self) -> u8 {
        match self {
            Self::StrategyA => StrategyMask::STRATEGY_A,
            Self::StrategyB => StrategyMask::STRATEGY_B,
            Self::StrategyC => StrategyMask::STRATEGY_C,
        }
    }

    fn trips(self, table: &Rlt1024) -> &TripWord {
        match self {
            Self::StrategyA => &table.strat_a_trips,
            Self::StrategyB => &table.strat_b_trips,
            Self::StrategyC => &table.strat_c_trips,
        }
    }

    fn actions(self, table: &Rlt1024) -> &ActionWord {
        match self {
            Self::StrategyA => &table.strat_a_actions,
            Self::StrategyB => &table.strat_b_actions,
            Self::StrategyC => &table.strat_c_actions,
        }
    }
}

/// Stress axes tracked by the breaker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StressAxis {
    /// ALT index, dimensionless 10-bit metric.
    Alt,
    /// Reject rate, expressed in basis points.
    Reject,
    /// Packet loss rate, expressed in basis points.
    Loss,
    /// Volatility (Q4.8 basis points).
    Vol,
}

/// Live stress measurements derived from ALT/AVS/ACT feeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StressInputs {
    /// Order-path stress index (ALT), 0..=1023.
    pub alt_idx: u16,
    /// Reject rate in basis points, 0..=1023.
    pub reject_bps: u16,
    /// Packet loss in basis points, 0..=1023.
    pub loss_bps: u16,
    /// Volatility in Q4.8 basis points, 0..=4095.
    pub vol_q4_8: u16,
}

impl StressInputs {
    /// Returns the maximum stress axis currently observed.
    #[must_use]
    pub fn dominant_axis(self, thresholds: &TripThresholds, level: u8) -> Option<StressAxis> {
        if level >= 3 {
            return None;
        }
        let idx = level as usize;
        if self.alt_idx >= thresholds.alt[idx] {
            return Some(StressAxis::Alt);
        }
        if self.reject_bps >= thresholds.rej[idx] {
            return Some(StressAxis::Reject);
        }
        if self.loss_bps >= thresholds.loss[idx] {
            return Some(StressAxis::Loss);
        }
        if self.vol_q4_8 >= thresholds.vol[idx] {
            return Some(StressAxis::Vol);
        }
        None
    }
}

/// Track the breaker level and the last time it changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelState {
    /// Active breaker level (0..=3).
    pub level: u8,
    /// Timestamp (milliseconds) when the level was last updated.
    pub entered_at_ms: u64,
}

impl Default for LevelState {
    fn default() -> Self {
        Self::new()
    }
}

impl LevelState {
    /// Construct a new state pinned to level `L0`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            level: 0,
            entered_at_ms: 0,
        }
    }
}

/// Transition descriptions emitted by [`LevelDecision`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelTransition {
    /// Breaker escalated to a higher level due to the supplied axis.
    Escalated {
        /// Source level before the update.
        from: u8,
        /// Destination level (from + 1).
        to: u8,
        /// Axis responsible for the escalation.
        axis: StressAxis,
    },
    /// Breaker recovered to a lower level.
    Recovered {
        /// Source level before the update.
        from: u8,
        /// Destination level (from - 1).
        to: u8,
    },
}

/// Action multipliers and routing directives for the current level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrategyActions {
    /// Size multiplier encoded in Q2.6.
    pub size_q2_6: u8,
    /// Slip cap multiplier encoded in Q1.7.
    pub slip_q1_7: u8,
    /// Latency budget multiplier encoded in Q1.7.
    pub latency_q1_7: u8,
    /// Route enforcement for the level.
    pub route: RoutePolicy,
}

impl StrategyActions {
    fn from_draft(draft: &ActionsWordDraft, level: u8) -> Self {
        let idx = level.min(3) as usize;
        Self {
            size_q2_6: draft.size_q2_6[idx],
            slip_q1_7: draft.slip_q1_7[idx],
            latency_q1_7: draft.latency_q1_7[idx],
            route: draft.route[idx],
        }
    }

    /// Applies the stored multipliers to the provided bases.
    #[must_use]
    pub fn apply(&self, bases: ActionBases) -> AppliedActionSet {
        AppliedActionSet {
            size: bases.size * decode_q2_6(self.size_q2_6),
            slip_cap: bases.slip_cap * decode_q1_7(self.slip_q1_7),
            latency_budget: bases.latency_budget * decode_q1_7(self.latency_q1_7),
            route: self.route,
        }
    }
}

/// Result of evaluating breaker stress against the RLT snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelDecision {
    /// Level after applying the hysteresis rules.
    pub level: u8,
    /// Optional transition metadata when a level change occurred.
    pub transition: Option<LevelTransition>,
    /// Action multipliers associated with the resolved level.
    pub actions: StrategyActions,
    /// Dwell-up enforcement in milliseconds.
    pub dwell_up_ms: u16,
    /// Dwell-down enforcement in milliseconds.
    pub dwell_down_ms: u16,
}

impl LevelDecision {
    /// Returns `true` when the breaker level changed as part of this decision.
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.transition.is_some()
    }
}

/// Errors raised when the RLT snapshot cannot drive the requested strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluationError {
    /// The strategy bit was not enabled in the capsule header.
    StrategyDisabled,
    /// The strategy requested a level outside the supported range.
    InvalidInitialLevel(u8),
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StrategyDisabled => write!(f, "strategy not enabled in RLT header"),
            Self::InvalidInitialLevel(level) => write!(f, "invalid initial level {level}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EvaluationError {}

/// Evaluate the breaker level for the given strategy and stress snapshot.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_strategy(
    table: &Rlt1024,
    strategy: StrategyId,
    inputs: StressInputs,
    now_ms: u64,
    state: &mut LevelState,
) -> Result<LevelDecision, EvaluationError> {
    let header: HeaderWord = table.header;
    let mask = header.strategy_mask();
    if !mask.contains(strategy.mask_bit()) {
        return Err(EvaluationError::StrategyDisabled);
    }
    if state.level > 3 {
        return Err(EvaluationError::InvalidInitialLevel(state.level));
    }

    let recover_scale: RecoverScale = header.recover_scale();
    let thresholds: TripThresholds = strategy.trips(table).thresholds();
    let actions_word: &ActionWord = strategy.actions(table);
    let actions_draft: ActionsWordDraft = actions_word.draft();

    let dwell_up_ms: u16 = if actions_draft.dwell_up_ms != 0 {
        actions_draft.dwell_up_ms
    } else {
        header.dwell_up_ms()
    };
    let dwell_down_ms: u16 = if actions_draft.dwell_down_ms != 0 {
        actions_draft.dwell_down_ms
    } else {
        header.dwell_down_ms()
    };

    let mut current_level = state.level.min(3);
    let mut transition: Option<LevelTransition> = None;

    let elapsed = if state.entered_at_ms == 0 {
        u64::MAX
    } else {
        now_ms.wrapping_sub(state.entered_at_ms)
    };

    let dwell_up_ready =
        dwell_up_ms == 0 || state.entered_at_ms == 0 || elapsed >= u64::from(dwell_up_ms);
    let dwell_down_ready =
        dwell_down_ms == 0 || state.entered_at_ms == 0 || elapsed >= u64::from(dwell_down_ms);

    if current_level < 3 && dwell_up_ready {
        if let Some(axis) = inputs.dominant_axis(&thresholds, current_level) {
            transition = Some(LevelTransition::Escalated {
                from: current_level,
                to: current_level + 1,
                axis,
            });
            current_level += 1;
        }
    }

    if transition.is_none() && current_level > 0 && dwell_down_ready {
        let idx = usize::from(current_level - 1);
        let alt_rec = recover_scale.apply(thresholds.alt[idx]);
        let rej_rec = recover_scale.apply(thresholds.rej[idx]);
        let loss_rec = recover_scale.apply(thresholds.loss[idx]);
        let vol_rec = recover_scale.apply(thresholds.vol[idx]);

        if inputs.alt_idx <= alt_rec
            && inputs.reject_bps <= rej_rec
            && inputs.loss_bps <= loss_rec
            && inputs.vol_q4_8 <= vol_rec
        {
            transition = Some(LevelTransition::Recovered {
                from: current_level,
                to: current_level - 1,
            });
            current_level -= 1;
        }
    }

    if transition.is_some() || state.entered_at_ms == 0 {
        state.entered_at_ms = now_ms;
    }
    state.level = current_level;

    let actions = StrategyActions::from_draft(&actions_draft, current_level);

    Ok(LevelDecision {
        level: current_level,
        transition,
        actions,
        dwell_up_ms,
        dwell_down_ms,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    fn base_table() -> Rlt1024 {
        let mut table = Rlt1024::new();
        let mut header = table.header;
        header.set_strategy_mask(StrategyMask::new(
            StrategyMask::STRATEGY_A | StrategyMask::STRATEGY_B | StrategyMask::STRATEGY_C,
        ));
        header.set_recover_scale(RecoverScale::new(90));
        header.set_dwell_up_ms(0);
        header.set_dwell_down_ms(0);
        table.header = header;

        let mut trips = TripWord::ZERO;
        trips.set_thresholds(TripThresholds {
            alt: [512, 768, 1023],
            rej: [100, 200, 400],
            loss: [50, 150, 300],
            vol: [256, 512, 768],
        });
        table.strat_a_trips = trips;
        table.strat_a_actions.apply_draft(ActionsWordDraft::DEFAULT);
        table
    }

    #[test]
    fn rejects_disabled_strategy() {
        let mut table = Rlt1024::new();
        let mut header = table.header;
        header.set_strategy_mask(StrategyMask::new(StrategyMask::STRATEGY_A));
        table.header = header;

        let result = evaluate_strategy(
            &table,
            StrategyId::StrategyB,
            StressInputs {
                alt_idx: 0,
                reject_bps: 0,
                loss_bps: 0,
                vol_q4_8: 0,
            },
            0,
            &mut LevelState::new(),
        );

        assert!(matches!(result, Err(EvaluationError::StrategyDisabled)));
    }

    #[test]
    fn escalates_when_threshold_crossed() {
        let table = base_table();
        let mut state = LevelState::new();

        let decision = evaluate_strategy(
            &table,
            StrategyId::StrategyA,
            StressInputs {
                alt_idx: 600,
                reject_bps: 0,
                loss_bps: 0,
                vol_q4_8: 0,
            },
            1_000,
            &mut state,
        )
        .expect("evaluation");

        assert_eq!(decision.level, 1);
        assert!(decision.changed());
        assert!(matches!(
            decision.transition,
            Some(LevelTransition::Escalated {
                from: 0,
                to: 1,
                axis: StressAxis::Alt,
            })
        ));
        assert_eq!(
            decision.actions.size_q2_6,
            ActionsWordDraft::DEFAULT.size_q2_6[1]
        );
        assert_eq!(state.level, 1);
        assert_eq!(state.entered_at_ms, 1_000);

        let applied = decision.actions.apply(ActionBases {
            size: 10.0,
            slip_cap: 1.0,
            latency_budget: 1.0,
        });
        assert!((applied.size - 5.0).abs() < 1e-6);
        assert!((applied.slip_cap - 0.8515625).abs() < 1e-6);
        assert!(matches!(applied.route, RoutePolicy::MakerPreferred));
    }

    #[test]
    fn dwell_up_blocks_escalation() {
        let mut table = base_table();
        let mut header = table.header;
        header.set_dwell_up_ms(2_000);
        table.header = header;

        let mut state = LevelState {
            level: 0,
            entered_at_ms: 500,
        };

        let decision = evaluate_strategy(
            &table,
            StrategyId::StrategyA,
            StressInputs {
                alt_idx: 600,
                reject_bps: 0,
                loss_bps: 0,
                vol_q4_8: 0,
            },
            1_000,
            &mut state,
        )
        .expect("evaluation");

        assert_eq!(decision.level, 0);
        assert!(!decision.changed());
        assert_eq!(state.level, 0);
    }

    #[test]
    fn recovers_when_all_axes_clear() {
        let table = base_table();
        let mut state = LevelState {
            level: 2,
            entered_at_ms: 10,
        };

        let decision = evaluate_strategy(
            &table,
            StrategyId::StrategyA,
            StressInputs {
                alt_idx: 0,
                reject_bps: 0,
                loss_bps: 0,
                vol_q4_8: 0,
            },
            5_000,
            &mut state,
        )
        .expect("evaluation");

        assert_eq!(decision.level, 1);
        assert!(matches!(
            decision.transition,
            Some(LevelTransition::Recovered { from: 2, to: 1 })
        ));
        assert_eq!(state.level, 1);
        assert_eq!(state.entered_at_ms, 5_000);
    }

    #[test]
    fn dwell_down_blocks_recovery() {
        let mut table = base_table();
        let mut header = table.header;
        header.set_dwell_down_ms(4_000);
        table.header = header;

        let mut state = LevelState {
            level: 1,
            entered_at_ms: 8_000,
        };

        let decision = evaluate_strategy(
            &table,
            StrategyId::StrategyA,
            StressInputs {
                alt_idx: 0,
                reject_bps: 0,
                loss_bps: 0,
                vol_q4_8: 0,
            },
            10_000,
            &mut state,
        )
        .expect("evaluation");

        assert_eq!(decision.level, 1);
        assert!(!decision.changed());
        assert_eq!(state.level, 1);
    }
}
