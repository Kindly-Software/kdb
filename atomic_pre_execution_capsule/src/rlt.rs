//! Helpers for applying RLT-1024 actions to PEX drafts.

use atomic_breaker::{ActionBases, AppliedActionSet, LevelDecision, RoutePolicy, StrategyActions};

use crate::{
    PexDraft, Play, RouteTemplate, TailDefaults, BRACKET_TEMPLATE_COUNT, PLAY_COUNT,
    PLAY_LAT_BUDGET_US, PLAY_QTY, PLAY_SLIP_CAP_BP, ROUTE_SLIP_CAP_BP, ROUTE_TEMPLATE_COUNT,
    W7_LAT_BUDGET_DEFAULT_US, W7_SLIP_CAP_DEFAULT_BP,
};

fn scale_to_bits(value: f32, bits: u32) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    let max_int = (1u32 << bits) - 1;
    let max = max_int as f32;
    let clamped = if value > max { max } else { value };
    let adjusted = clamped + 0.5;
    let truncated = adjusted as u32;
    if truncated > max_int {
        max_int
    } else {
        truncated
    }
}

fn bases_from_play(play: &Play) -> ActionBases {
    ActionBases {
        size: play.qty as f32,
        slip_cap: play.slip_cap_bp as f32,
        latency_budget: play.lat_budget_us as f32,
    }
}

fn applied_from_route(route: &RouteTemplate, actions: &StrategyActions) -> AppliedActionSet {
    actions.apply(ActionBases {
        size: 1.0,
        slip_cap: route.slip_cap_bp as f32,
        latency_budget: 1.0,
    })
}

fn applied_from_defaults(defaults: &TailDefaults, actions: &StrategyActions) -> AppliedActionSet {
    actions.apply(ActionBases {
        size: 1.0,
        slip_cap: defaults.slip_cap_default_bp as f32,
        latency_budget: defaults.lat_budget_default_us as f32,
    })
}

fn scale_play(base: &Play, actions: &StrategyActions, policy: RoutePolicy) -> Play {
    let applied = actions.apply(bases_from_play(base));
    let mut play = *base;
    play.qty = scale_to_bits(applied.size, PLAY_QTY.bits) as u32;
    play.slip_cap_bp = scale_to_bits(applied.slip_cap, PLAY_SLIP_CAP_BP.bits) as u16;
    play.lat_budget_us = scale_to_bits(applied.latency_budget, PLAY_LAT_BUDGET_US.bits) as u16;
    if matches!(policy, RoutePolicy::ForbidNew) {
        play.qty = 0;
        play.enable = false;
    } else if play.qty == 0 {
        play.enable = false;
    }
    play
}

fn scale_route_template(
    base: &RouteTemplate,
    actions: &StrategyActions,
    policy: RoutePolicy,
) -> RouteTemplate {
    let applied = applied_from_route(base, actions);
    let mut tpl = *base;
    tpl.slip_cap_bp = scale_to_bits(applied.slip_cap, ROUTE_SLIP_CAP_BP.bits) as u16;
    tpl.maker_taker = match policy {
        RoutePolicy::Normal => base.maker_taker,
        RoutePolicy::MakerPreferred => false,
        RoutePolicy::TakerOnly | RoutePolicy::ForbidNew => true,
    };
    if matches!(policy, RoutePolicy::ForbidNew) {
        tpl.allow_partial = false;
    } else {
        tpl.allow_partial = base.allow_partial;
    }
    tpl
}

fn scale_defaults(base: &TailDefaults, actions: &StrategyActions) -> TailDefaults {
    let applied = applied_from_defaults(base, actions);
    let mut defaults = *base;
    defaults.slip_cap_default_bp =
        scale_to_bits(applied.slip_cap, W7_SLIP_CAP_DEFAULT_BP.bits) as u16;
    defaults.lat_budget_default_us =
        scale_to_bits(applied.latency_budget, W7_LAT_BUDGET_DEFAULT_US.bits) as u16;
    defaults
}

/// Apply the breaker level decision to a mutable draft using the provided base template.
///
/// The draft is updated in-place with scaled quantities, slip caps, latency budgets, and
/// portfolio breaker level. Plays are disabled when the policy requests `ForbidNew` or the
/// scaled quantity resolves to zero.
pub fn apply_level_to_draft(target: &mut PexDraft, base: &PexDraft, decision: &LevelDecision) {
    let policy = decision.actions.route;
    target.header.portfolio_breaker_level = decision.level;

    for idx in 0..PLAY_COUNT {
        let base_play = base.plays[idx];
        target.plays[idx] = scale_play(&base_play, &decision.actions, policy);
    }

    for idx in 0..BRACKET_TEMPLATE_COUNT {
        target.bracket_templates[idx] = base.bracket_templates[idx];
    }

    for idx in 0..ROUTE_TEMPLATE_COUNT {
        let base_route = base.route_templates[idx];
        target.route_templates[idx] = scale_route_template(&base_route, &decision.actions, policy);
    }

    target.defaults = scale_defaults(&base.defaults, &decision.actions);
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomic_breaker::{LevelTransition, StrategyId, StressInputs};
    use atomic_risk_ladder_table::layout::{
        actions::ActionsWordDraft,
        header::{HeaderWord, StrategyMask},
        trips::TripWord,
        RecoverScale,
    };
    use atomic_risk_ladder_table::{layout::trips::TripThresholds, Rlt1024};

    fn sample_decision() -> (LevelDecision, PexDraft) {
        let mut base = PexDraft::default();
        base.header.portfolio_breaker_level = 0;
        for (idx, play) in base.plays.iter_mut().enumerate() {
            play.enable = true;
            play.qty = 100 + (idx as u32) * 10;
            play.slip_cap_bp = 6 + idx as u16;
            play.lat_budget_us = 900 - idx as u16 * 50;
        }
        for (idx, route) in base.route_templates.iter_mut().enumerate() {
            route.slip_cap_bp = 8 + idx as u16;
            route.maker_taker = idx >= 2;
            route.allow_partial = true;
        }
        base.defaults.slip_cap_default_bp = 6;
        base.defaults.lat_budget_default_us = 2_500;

        let mut table = Rlt1024::new();
        let mut header = HeaderWord::ZERO;
        header.set_strategy_mask(StrategyMask::new(StrategyMask::STRATEGY_A));
        header.set_recover_scale(RecoverScale::new(90));
        table.header = header;
        let mut trips = TripWord::ZERO;
        trips.set_thresholds(TripThresholds::DEFAULT);
        table.strat_a_trips = trips;
        table.strat_a_actions.apply_draft(ActionsWordDraft::DEFAULT);

        let mut state = atomic_breaker::rlt::LevelState::new();
        let decision = atomic_breaker::evaluate_strategy(
            &table,
            StrategyId::StrategyA,
            StressInputs {
                alt_idx: 800,
                reject_bps: 0,
                loss_bps: 0,
                vol_q4_8: 0,
            },
            1_000,
            &mut state,
        )
        .expect("evaluation");

        (decision, base)
    }

    #[test]
    fn scaling_updates_plays_routes_and_defaults() {
        let (decision, base) = sample_decision();
        let mut draft = base.clone();
        apply_level_to_draft(&mut draft, &base, &decision);

        assert_eq!(draft.header.portfolio_breaker_level, decision.level);
        for (idx, play) in draft.plays.iter().enumerate() {
            let expected_qty = ((base.plays[idx].qty as f32 * 0.5).round()) as u32;
            assert_eq!(play.qty, expected_qty);
            assert!(play.slip_cap_bp <= base.plays[idx].slip_cap_bp);
            assert!(play.lat_budget_us <= base.plays[idx].lat_budget_us);
            assert!(play.enable);
        }

        for (idx, route) in draft.route_templates.iter().enumerate() {
            assert!(route.slip_cap_bp <= base.route_templates[idx].slip_cap_bp);
        }

        assert!(draft.defaults.slip_cap_default_bp <= base.defaults.slip_cap_default_bp);
        assert!(draft.defaults.lat_budget_default_us <= base.defaults.lat_budget_default_us);
    }

    #[test]
    fn forbid_new_disables_plays_and_routes() {
        let (mut decision, base) = sample_decision();
        decision.actions.route = RoutePolicy::ForbidNew;
        decision.transition = Some(LevelTransition::Escalated {
            from: 2,
            to: 3,
            axis: atomic_breaker::rlt::StressAxis::Alt,
        });
        let mut draft = base.clone();
        apply_level_to_draft(&mut draft, &base, &decision);

        for play in draft.plays.iter() {
            assert!(!play.enable);
            assert_eq!(play.qty, 0);
        }
        for route in draft.route_templates.iter() {
            assert!(!route.allow_partial);
        }
    }
}
