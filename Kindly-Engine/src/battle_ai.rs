use crate::fire_doctrine::FireDoctrineMode;
use crate::fog::FogOfWarView;
use crate::formation::FormationSnapshot;
use crate::order::OrderKind;
use crate::replay::{
    encode_battle_ai_intent_payload, encode_battle_ai_replay_payload,
};
use atomic_capsule::verify_capsule_properties;
use core::cmp::Ordering as CmpOrdering;
use core::sync::atomic::{AtomicU64, Ordering};
use std::string::String;

/// Maximum AI-issued orders per shard tick (bounded to avoid order spam).
pub const MAX_AI_DECISIONS_PER_TICK: usize = 32;

/// Deterministic AI decision emitted for a shard tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleAiDecision {
    pub source_formation_id: u32,
    pub target_formation_id: u32,
    pub order: OrderKind,
    /// Normalized fixed-point score (Q0.8), 0–255.
    pub score_q8: u8,
    /// Generation counter to keep delivery idempotent.
    pub generation: u32,
}

impl BattleAiDecision {
    pub const fn empty() -> Self {
        Self {
            source_formation_id: 0,
            target_formation_id: 0,
            order: OrderKind::Hold,
            score_q8: 0,
            generation: 0,
        }
    }

    /// Encode this decision into a replay payload for telemetry/audit.
    pub fn replay_payload(&self) -> u64 {
        encode_battle_ai_replay_payload(
            self.source_formation_id,
            self.target_formation_id,
            self.order,
            self.score_q8,
        )
    }
}

/// Aggregated AI intent snapshot for telemetry/replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleAiIntent {
    pub dominant_stance: u8,
    pub doctrine_mode: u8,
    pub threat_centroid_x_tile: u16,
    pub threat_centroid_z_tile: u16,
    pub generation_lsb: u8,
}

impl BattleAiIntent {
    pub fn replay_payload(&self) -> u64 {
        encode_battle_ai_intent_payload(
            self.threat_centroid_x_tile,
            self.threat_centroid_z_tile,
            self.doctrine_mode,
            self.dominant_stance,
            self.generation_lsb,
        )
    }
}

/// Fixed-capacity buffer of AI decisions for a shard tick.
#[derive(Debug, Clone)]
pub struct BattleAiPlan {
    decisions: [BattleAiDecision; MAX_AI_DECISIONS_PER_TICK],
    len: usize,
    intent: Option<BattleAiIntent>,
}

#[inline]
fn select_doctrine(
    dominant_stance: u8,
    threat_count: u32,
    current: Option<FireDoctrineMode>,
    courier_latency_ticks: u16,
) -> FireDoctrineMode {
    if threat_count == 0 {
        return current.unwrap_or(FireDoctrineMode::Disabled);
    }
    if courier_latency_ticks > 24 {
        return FireDoctrineMode::Volley;
    }
    if threat_count >= 6 {
        return FireDoctrineMode::Volley;
    }
    if threat_count >= 3 {
        return FireDoctrineMode::ByRank;
    }
    if dominant_stance >= 4 {
        return FireDoctrineMode::AdvanceAndFire;
    }
    FireDoctrineMode::Rolling
}

impl BattleAiPlan {
    pub const fn empty() -> Self {
        Self {
            decisions: [BattleAiDecision::empty(); MAX_AI_DECISIONS_PER_TICK],
            len: 0,
            intent: None,
        }
    }

    /// Push a decision; returns false if at capacity.
    pub fn push(&mut self, decision: BattleAiDecision) -> bool {
        if self.len >= MAX_AI_DECISIONS_PER_TICK {
            return false;
        }
        self.decisions[self.len] = decision;
        self.len += 1;
        true
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn iter(&self) -> impl Iterator<Item = &BattleAiDecision> {
        self.decisions[..self.len].iter()
    }

    /// Iterator over replay payloads for telemetry/logging.
    pub fn replay_payloads(&self) -> impl Iterator<Item = u64> + '_ {
        self.iter().map(BattleAiDecision::replay_payload)
    }

    /// Optional aggregated intent payload (stance/doctrine/threat centroid).
    pub fn intent_payload(&self) -> Option<u64> {
        self.intent.map(|i| i.replay_payload())
    }

    pub fn intent(&self) -> Option<BattleAiIntent> {
        self.intent
    }
}

/// Build a lightweight NDJSON line for dashboards/telemetry.
pub fn intent_ndjson_line(tick: u64, intent: BattleAiIntent) -> String {
    format!(
        "{{\"tick\":{},\"dominant_stance\":{},\"doctrine_mode\":{},\"threat_centroid\":[{},{}],\"generation_lsb\":{}}}",
        tick,
        intent.dominant_stance,
        intent.doctrine_mode,
        intent.threat_centroid_x_tile,
        intent.threat_centroid_z_tile,
        intent.generation_lsb
    )
}

/// Inputs for a shard-level AI pass.
#[derive(Debug, Clone)]
pub struct BattleAiInputs<'a> {
    pub tick: u64,
    pub formations: &'a [FormationSnapshot],
    pub doctrine: Option<FireDoctrineMode>,
    /// Courier latency hint (ticks) to bias cadence.
    pub courier_latency_ticks: u16,
    /// Optional fog-of-war view to filter visible targets.
    pub fog: Option<FogOfWarView<'a>>,
}

/// Capsule orchestrating deterministic, lock-free battle AI per shard.
///
/// - Alignment: 128B, size 128B (padding prevents false sharing).
/// - Determinism: generation counter for idempotent order delivery.
/// - Performance: no per-tick allocations; bounded output.
#[repr(C, align(128))]
pub struct BattleAiCapsule {
    generation: AtomicU64,
    decisions_emitted: AtomicU64,
    _padding: [u8; 112],
}

impl BattleAiCapsule {
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            decisions_emitted: AtomicU64::new(0),
            _padding: [0; 112],
        }
    }

    /// Deterministic planning pass for one shard (initial heuristic).
    ///
    /// - Picks the nearest viable target per formation.
    /// - Issues at most MAX_AI_DECISIONS_PER_TICK orders.
    /// - Uses fixed-point scoring and generation counters for idempotent delivery.
    pub fn plan_for_shard(&self, inputs: BattleAiInputs<'_>) -> BattleAiPlan {
        let mut plan = BattleAiPlan::empty();
        let mut stance_hist = [0u16; 8];
        let mut threat_x_sum = 0u64;
        let mut threat_z_sum = 0u64;
        let mut threat_count = 0u32;
        let mut last_generation = 0u32;

        for src in inputs.formations.iter() {
            if plan.len() >= MAX_AI_DECISIONS_PER_TICK {
                break;
            }
            if !should_issue(src, inputs.tick, inputs.courier_latency_ticks) {
                continue;
            }

            let mut visible_targets = 0u16;
            for tgt in inputs.formations.iter() {
                if tgt.formation_id == src.formation_id {
                    continue;
                }
                if let Some(fog) = inputs.fog {
                    if !fog.can_see(src, tgt) {
                        continue;
                    }
                }
                visible_targets = visible_targets.saturating_add(1);
            }
            let visibility_bonus = (visible_targets.min(15) as u8) << 1;
            let mut best_target: Option<(&FormationSnapshot, u64, u8)> = None;
            for tgt in inputs.formations.iter() {
                if tgt.formation_id == src.formation_id {
                    continue;
                }
                if let Some(fog) = inputs.fog {
                    if !fog.can_see(src, tgt) {
                        continue;
                    }
                }
                let dist_sq = distance_sq_q16(src, tgt);
                let mut score_q8 =
                    effective_score_q8(src, tgt, dist_sq, inputs.doctrine);
                score_q8 = score_q8.saturating_add(visibility_bonus);
                match best_target {
                    None => best_target = Some((tgt, dist_sq, score_q8)),
                    Some((bt, bdist, bscore)) => match score_q8.cmp(&bscore) {
                        CmpOrdering::Greater => best_target = Some((tgt, dist_sq, score_q8)),
                        CmpOrdering::Equal => {
                            // Tie-breaker: nearer target, then lower id.
                            if dist_sq < bdist
                                || (dist_sq == bdist && tgt.formation_id < bt.formation_id)
                            {
                                best_target = Some((tgt, dist_sq, score_q8));
                            }
                        }
                        CmpOrdering::Less => {}
                    },
                }
            }

            let Some((target, _dist_sq, score_q8)) = best_target else {
                continue;
            };

            let order = if src.ammo > 0 {
                OrderKind::Fire
            } else {
                OrderKind::Charge
            };

            let decision = BattleAiDecision {
                source_formation_id: src.formation_id,
                target_formation_id: target.formation_id,
                order,
                score_q8,
                generation: self.next_generation(),
            };

            if !plan.push(decision) {
                break;
            }
            let stance_idx = (src.stance as usize).min(stance_hist.len() - 1);
            stance_hist[stance_idx] = stance_hist[stance_idx].saturating_add(1);
            threat_x_sum = threat_x_sum.saturating_add(target.position_x_q16 as u64);
            threat_z_sum = threat_z_sum.saturating_add(target.position_z_q16 as u64);
            threat_count = threat_count.saturating_add(1);
            last_generation = decision.generation;
        }

        self.record_decisions(plan.len());
        if plan.len() > 0 && threat_count > 0 {
            let threat_x_avg_q16 = (threat_x_sum / threat_count as u64) as u32;
            let threat_z_avg_q16 = (threat_z_sum / threat_count as u64) as u32;
            let threat_x_tile = ((threat_x_avg_q16 >> 16).min(0x0FFF)) as u16;
            let threat_z_tile = ((threat_z_avg_q16 >> 16).min(0x0FFF)) as u16;
            let dominant_stance = stance_hist
                .iter()
                .enumerate()
                .max_by_key(|(_, &c)| c)
                .map(|(idx, _)| idx as u8)
                .unwrap_or(0);
            let recommended_mode = select_doctrine(
                dominant_stance,
                threat_count,
                inputs.doctrine,
                inputs.courier_latency_ticks,
            );
            plan.intent = Some(BattleAiIntent {
                dominant_stance,
                doctrine_mode: recommended_mode as u8,
                threat_centroid_x_tile: threat_x_tile,
                threat_centroid_z_tile: threat_z_tile,
                generation_lsb: (last_generation & 0xFF) as u8,
            });
        }
        plan
    }

    /// Reserve a new generation id for outbound orders.
    pub fn next_generation(&self) -> u32 {
        let next = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        (next & 0xFF_FFFF) as u32
    }

    /// Snapshot counts for telemetry.
    pub fn decisions_emitted(&self) -> u64 {
        self.decisions_emitted.load(Ordering::Relaxed)
    }

    fn record_decisions(&self, count: usize) {
        if count == 0 {
            return;
        }
        self.decisions_emitted
            .fetch_add(count as u64, Ordering::AcqRel);
    }
}

verify_capsule_properties!(BattleAiCapsule, 128, 128);

fn distance_sq_q16(a: &FormationSnapshot, b: &FormationSnapshot) -> u64 {
    let dx = b.position_x_q16 as i64 - a.position_x_q16 as i64;
    let dz = b.position_z_q16 as i64 - a.position_z_q16 as i64;
    let dx_abs = dx.unsigned_abs();
    let dz_abs = dz.unsigned_abs();
    dx_abs
        .saturating_mul(dx_abs)
        .saturating_add(dz_abs.saturating_mul(dz_abs))
}

fn effective_score_q8(
    src: &FormationSnapshot,
    target: &FormationSnapshot,
    dist_sq: u64,
    doctrine: Option<FireDoctrineMode>,
) -> u8 {
    let morale = (src.morale_q16 >> 8).min(255) as i32;
    let fatigue_penalty = (src.fatigue_q16 >> 10).min(200) as i32;
    let ammo_bonus = if src.ammo > 0 { 20 } else { -40 };
    let density_bonus = (target.density_q16 >> 12).min(120) as i32;
    let vulnerability_bonus =
        ((255 - (target.morale_q16 >> 8).min(255)) as i32).saturating_sub(40) / 2;
    let braced_penalty = if target.braced { 40 } else { 0 };
    let doctrine_bonus = match doctrine {
        Some(FireDoctrineMode::AdvanceAndFire | FireDoctrineMode::Rolling) => 18,
        Some(FireDoctrineMode::Volley | FireDoctrineMode::ByRank) => 10,
        _ => 0,
    };
    // Coarse distance penalty scaled from Q16 positions; keeps local targets favored.
    let distance_penalty = (dist_sq >> 18).min(200) as i32;
    let base = 80;
    let score = base + morale + density_bonus + vulnerability_bonus + doctrine_bonus + ammo_bonus
        - fatigue_penalty
        - distance_penalty
        - braced_penalty;
    score.clamp(0, 255) as u8
}

fn should_issue(src: &FormationSnapshot, tick: u64, courier_latency_ticks: u16) -> bool {
    if src.morale_q16 < 16_000 || src.fatigue_q16 > 196_608 {
        return false;
    }
    // Deterministic cadence gate tied to courier latency hint.
    let cadence = courier_latency_ticks.max(1) as u64;
    tick % cadence == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(
        id: u32,
        x: u32,
        z: u32,
        morale: u32,
        fatigue: u32,
        ammo: u32,
        density: u32,
    ) -> FormationSnapshot {
        FormationSnapshot {
            formation_id: id,
            posture: 0,
            stance: 0,
            generation: 0,
            cohesion_q16: 0,
            fatigue_q16: fatigue,
            ammo,
            morale_q16: morale,
            facing_deg_q16: 0,
            position_x_q16: x,
            position_z_q16: z,
            command_delay_ms: 0,
            retreat_mode_flags: 0,
            charge_posture: 0,
            braced: false,
            density_q16: density,
            mass_q16: 0,
            variance_q16: 0,
            damping_q16: 0,
            velocity_q16: 0,
            physics_flags: 0,
            gap_close_q16: 0,
            rank_variance_scale_q16: 0,
            gap_fatigue_penalty_q16: 0,
        }
    }

    #[test]
    fn deterministic_for_same_inputs() {
        let ai_a = BattleAiCapsule::new();
        let ai_b = BattleAiCapsule::new();
        let formations = vec![
            snap(1, 10, 0, 80_000, 10_000, 10, 65_536),
            snap(2, 20, 0, 75_000, 12_000, 10, 65_536),
        ];
        let inputs = BattleAiInputs {
            tick: 0,
            formations: &formations,
            doctrine: None,
            courier_latency_ticks: 1,
            fog: None,
        };
        let p1 = ai_a.plan_for_shard(inputs.clone());
        let p2 = ai_b.plan_for_shard(inputs);
        let to_key = |d: &BattleAiDecision| {
            (
                d.source_formation_id,
                d.target_formation_id,
                d.order as u8,
                d.score_q8,
            )
        };
        let s1: Vec<_> = p1.iter().map(to_key).collect();
        let s2: Vec<_> = p2.iter().map(to_key).collect();
        assert_eq!(s1, s2);
    }

    #[test]
    fn capacity_is_bounded() {
        let ai = BattleAiCapsule::new();
        let mut formations = Vec::new();
        for i in 0..(MAX_AI_DECISIONS_PER_TICK as u32 + 10) {
            formations.push(snap(i + 1, i * 10, 0, 70_000, 8_000, 5, 70_000));
        }
        let inputs = BattleAiInputs {
            tick: 0,
            formations: &formations,
            doctrine: None,
            courier_latency_ticks: 1,
            fog: None,
        };
        let plan = ai.plan_for_shard(inputs);
        assert_eq!(plan.len(), MAX_AI_DECISIONS_PER_TICK);
    }

    #[test]
    fn tie_breaker_prefers_nearer_then_lower_id() {
        let ai = BattleAiCapsule::new();
        let src = snap(1, 0, 0, 80_000, 10_000, 10, 65_536);
        let near = snap(2, 10, 0, 70_000, 10_000, 10, 65_536);
        let far_same_score = snap(3, 20, 0, 70_000, 10_000, 10, 65_536);
        let formations = vec![src, near, far_same_score];
        let inputs = BattleAiInputs {
            tick: 0,
            formations: &formations,
            doctrine: None,
            courier_latency_ticks: 1,
            fog: None,
        };
        let plan = ai.plan_for_shard(inputs);
        let decision = plan.iter().next().expect("decision");
        assert_eq!(decision.target_formation_id, 2);
    }

    #[test]
    fn cadence_gate_respects_latency() {
        let ai = BattleAiCapsule::new();
        let formations = vec![
            snap(1, 0, 0, 80_000, 10_000, 10, 65_536),
            snap(2, 10, 0, 70_000, 10_000, 10, 65_536),
        ];
        let inputs = BattleAiInputs {
            tick: 1,
            formations: &formations,
            doctrine: None,
            courier_latency_ticks: 2,
            fog: None,
        };
        let plan = ai.plan_for_shard(inputs);
        assert!(plan.is_empty());
    }
}
