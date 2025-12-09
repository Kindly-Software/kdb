use crate::formation::FormationSnapshot;
use crate::terrain::TerrainGridCapsule;
use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU64, Ordering};

/// Fog-of-war capsule controlling visibility reach for formations.
///
/// - Alignment: 128B, size 128B.
/// - Deterministic: radius derived from cached bases + formation stance/morale (no RNG).
#[repr(C, align(128))]
#[derive(Debug)]
pub struct FogOfWarCapsule {
    base_radius_q16: AtomicU64,
    bonus_radius_q16: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 96],
}

impl FogOfWarCapsule {
    /// `base_radius_q16`: baseline visibility radius (meters Q16.16).
    /// `bonus_radius_q16`: bonus applied from stance/morale (meters Q16.16).
    pub const fn new(base_radius_q16: u32, bonus_radius_q16: u32) -> Self {
        Self {
            base_radius_q16: AtomicU64::new(base_radius_q16 as u64),
            bonus_radius_q16: AtomicU64::new(bonus_radius_q16 as u64),
            generation: AtomicU64::new(0),
            _padding: [0; 96],
        }
    }

    pub fn configure(&self, base_radius_q16: u32, bonus_radius_q16: u32) {
        self.base_radius_q16
            .store(base_radius_q16 as u64, Ordering::Release);
        self.bonus_radius_q16
            .store(bonus_radius_q16 as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Deterministic visibility radius squared (Q16.16 distance squared).
    pub fn visibility_radius_sq_q16(&self, formation: &FormationSnapshot) -> u64 {
        let base = self.base_radius_q16.load(Ordering::Relaxed);
        let bonus = self.bonus_radius_q16.load(Ordering::Relaxed);
        let stance_scale = match formation.stance {
            0 => 16, // column: narrow, lower LOS
            1 => 20, // line: wider frontage
            2 => 24, // square/defense: heightened alert
            _ => 18,
        };
        // Morale amplifies spotting a bit; fatigue dampens it.
        let morale_boost = (formation.morale_q16 as u64 / 4096).min(bonus / 2);
        let fatigue_penalty = (formation.fatigue_q16 as u64 / 8192).min(bonus / 3);
        let stance_bonus = (bonus * stance_scale / 32).saturating_sub(fatigue_penalty);
        let radius = base
            .saturating_add(morale_boost)
            .saturating_add(stance_bonus);
        radius.saturating_mul(radius)
    }
}

verify_capsule_properties!(FogOfWarCapsule, 128, 128);

/// Read-only view passed into AI/tick; keeps the capsule borrow short-lived.
#[derive(Clone, Copy, Debug)]
pub struct FogOfWarView<'a> {
    capsule: &'a FogOfWarCapsule,
    terrain: Option<&'a TerrainGridCapsule>,
}

impl<'a> FogOfWarView<'a> {
    pub fn new(capsule: &'a FogOfWarCapsule) -> Self {
        Self {
            capsule,
            terrain: None,
        }
    }

    pub fn with_terrain(mut self, terrain: &'a TerrainGridCapsule) -> Self {
        self.terrain = Some(terrain);
        self
    }

    /// Returns true if target is within deterministic visibility reach of source.
    pub fn can_see(&self, source: &FormationSnapshot, target: &FormationSnapshot) -> bool {
        let radius_sq = self.capsule.visibility_radius_sq_q16(source);
        let dx = target.position_x_q16 as i64 - source.position_x_q16 as i64;
        let dz = target.position_z_q16 as i64 - source.position_z_q16 as i64;
        let dist_sq = dx.unsigned_abs().saturating_mul(dx.unsigned_abs())
            + dz.unsigned_abs().saturating_mul(dz.unsigned_abs());
        if dist_sq > radius_sq {
            return false;
        }
        if let Some(terrain) = self.terrain {
            // Coarse LOS check using terrain grid; ignores structures for now.
            let start = (source.position_x_q16 >> 16, source.position_z_q16 >> 16);
            let end = (target.position_x_q16 >> 16, target.position_z_q16 >> 16);
            let (clear, _) = terrain.los_clear(start, end);
            if !clear {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: u32, stance: u8, morale: u32, fatigue: u32, x: u32, z: u32) -> FormationSnapshot {
        FormationSnapshot {
            formation_id: id,
            posture: 0,
            stance,
            generation: 0,
            cohesion_q16: 0,
            fatigue_q16: fatigue,
            ammo: 0,
            morale_q16: morale,
            facing_deg_q16: 0,
            position_x_q16: x,
            position_z_q16: z,
            command_delay_ms: 0,
            retreat_mode_flags: 0,
            charge_posture: 0,
            braced: false,
            density_q16: 0,
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
    fn higher_morale_extends_visibility() {
        let fog = FogOfWarCapsule::new(5_000, 1_000);
        let low = snap(1, 1, 20_000, 10_000, 0, 0);
        let high = snap(2, 1, 60_000, 10_000, 0, 0);
        let r_low = fog.visibility_radius_sq_q16(&low);
        let r_high = fog.visibility_radius_sq_q16(&high);
        assert!(r_high > r_low);
    }

    #[test]
    fn can_see_uses_radius() {
        let fog = FogOfWarCapsule::new(10_000, 0);
        let view = FogOfWarView::new(&fog);
        let src = snap(1, 1, 40_000, 10_000, 0, 0);
        let tgt_near = snap(2, 1, 40_000, 10_000, 5_000, 0);
        let tgt_far = snap(3, 1, 40_000, 10_000, 80_000, 0);
        assert!(view.can_see(&src, &tgt_near));
        assert!(!view.can_see(&src, &tgt_far));
    }
}
