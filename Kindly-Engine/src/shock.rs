use crate::formation::{gap_fatigue_penalty, FormationCapsule, FormationSnapshot};
use crate::telemetry::TelemetryCapsule;
use crate::terrain::TerrainGridCapsule;
use atomic_capsule::los::Q16_16;
use atomic_capsule::verify_capsule_properties;

const FLAG_PERMEABLE: u16 = 1 << 0;

/// Shock physics capsule for bayonet and cavalry impacts (deterministic, fixed-point).
#[repr(C, align(64))]
pub struct ShockPhysicsCapsule {
    /// Base impulse for a bayonet charge (Q16.16)
    base_bayonet_impulse_q16: u32,
    /// Base impulse for a cavalry charge (Q16.16)
    base_cavalry_impulse_q16: u32,
}

impl ShockPhysicsCapsule {
    pub const fn new(base_bayonet_impulse_q16: u32, base_cavalry_impulse_q16: u32) -> Self {
        Self {
            base_bayonet_impulse_q16,
            base_cavalry_impulse_q16,
        }
    }

    pub const fn default() -> Self {
        Self::new(45_000, 60_000)
    }

    /// Compute a bayonet charge outcome (attacker vs defender) with traction scaling.
    pub fn bayonet_charge(
        &self,
        attacker: &FormationSnapshot,
        defender: &FormationSnapshot,
        traction_q16: Q16_16,
    ) -> ShockOutcome {
        let charge_bonus = (attacker.charge_posture as u32)
            .saturating_mul(512)
            .min(20_000);
        let brace_penalty = if defender.braced { 12_000 } else { 0 };
        let base = self
            .base_bayonet_impulse_q16
            .saturating_add(charge_bonus)
            .saturating_sub(brace_penalty);
        let impulse_q16 = physics_impulse(attacker, defender, traction_q16, base, None);
        ShockOutcome::from_impulse(
            impulse_q16,
            defender.braced,
            defender.physics_flags & FLAG_PERMEABLE != 0,
        )
        .with_square_bonus(defender)
    }

    /// Compute a cavalry charge outcome using horse mass (kg) and speed (m/s in Q16.16).
    pub fn cavalry_charge(
        &self,
        attacker_speed_q16: Q16_16,
        horse_mass_kg: u32,
        defender: &FormationSnapshot,
        traction_q16: Q16_16,
    ) -> ShockOutcome {
        // Treat horse_mass as an additive override to attacker mass.
        let attacker_stub = stub_attacker_snapshot(horse_mass_kg, attacker_speed_q16.raw() as u32);
        let impulse_q16 = physics_impulse(
            &attacker_stub,
            defender,
            traction_q16,
            self.base_cavalry_impulse_q16,
            Some(attacker_speed_q16),
        );
        ShockOutcome::from_impulse(
            impulse_q16,
            defender.braced,
            defender.physics_flags & FLAG_PERMEABLE != 0,
        )
        .with_square_bonus(defender)
    }

    /// Charge outcome using terrain traction at defender tile.
    pub fn bayonet_with_terrain(
        &self,
        attacker: &FormationSnapshot,
        defender: &FormationSnapshot,
        grid: &TerrainGridCapsule,
    ) -> ShockOutcome {
        let traction =
            grid.traction_at(defender.position_x_q16 >> 16, defender.position_z_q16 >> 16);
        self.bayonet_charge(attacker, defender, traction)
    }

    pub fn cavalry_with_terrain(
        &self,
        attacker_speed_q16: Q16_16,
        horse_mass_kg: u32,
        defender: &FormationSnapshot,
        grid: &TerrainGridCapsule,
    ) -> ShockOutcome {
        let traction =
            grid.traction_at(defender.position_x_q16 >> 16, defender.position_z_q16 >> 16);
        self.cavalry_charge(attacker_speed_q16, horse_mass_kg, defender, traction)
    }
}

verify_capsule_properties!(ShockPhysicsCapsule, 64, 64);

/// Deterministic shock outcome applied to a formation.
#[derive(Debug, Clone, Copy)]
pub struct ShockOutcome {
    pub morale_penalty_q16: u32,
    pub cohesion_penalty_q16: u32,
    pub fatigue_delta_q16: u32,
    pub casualties: u32,
    pub shock_weight_q16: u32,
}

impl ShockOutcome {
    fn from_impulse(impulse_q16: u32, braced: bool, permeable: bool) -> Self {
        let brace_damp = if braced { 24_576u32 } else { 0 }; // ~0.375 in Q16.16
        let damped = impulse_q16.saturating_sub(brace_damp);
        let morale_penalty_q16 = damped.min(90_000);
        let cohesion_penalty_q16 = (damped.saturating_mul(3) / 4).min(60_000);
        let fatigue_delta_q16 = (damped / 2).min(30_000);
        let mut casualties = (damped / 5_000).min(500);
        let mut shock_weight_q16 = morale_penalty_q16.min(70_000);
        if permeable {
            // Skirmish-like gas: most hits pass through.
            casualties = (casualties / 4).max(1);
            shock_weight_q16 = (shock_weight_q16 / 2).max(8_000);
        }
        Self {
            morale_penalty_q16,
            cohesion_penalty_q16,
            fatigue_delta_q16,
            casualties,
            shock_weight_q16,
        }
    }

    /// Apply outcome to a formation and log into telemetry (deterministic).
    pub fn apply_to_defender(&self, defender: &FormationCapsule, telemetry: &TelemetryCapsule) {
        if self.casualties > 0 {
            telemetry.add_casualties(self.casualties);
        }
        telemetry.log_charge_shock(self.casualties, self.shock_weight_q16);
        defender.apply_shock_package(
            self.morale_penalty_q16,
            self.cohesion_penalty_q16,
            self.fatigue_delta_q16,
        );
    }

    fn with_square_bonus(mut self, defender: &FormationSnapshot) -> Self {
        if defender.braced && defender.density_q16 > 50_000 {
            self.morale_penalty_q16 = (self.morale_penalty_q16 as u64 * 48_000 / 65_536) as u32;
            self.cohesion_penalty_q16 = (self.cohesion_penalty_q16 as u64 * 48_000 / 65_536) as u32;
            self.fatigue_delta_q16 = (self.fatigue_delta_q16 as u64 * 52_000 / 65_536) as u32;
            self.casualties = (self.casualties as u64 * 45_000 / 65_536) as u32;
            self.shock_weight_q16 = (self.shock_weight_q16 as u64 * 48_000 / 65_536) as u32;
        }
        self
    }
}

fn physics_impulse(
    attacker: &FormationSnapshot,
    defender: &FormationSnapshot,
    traction_q16: Q16_16,
    base_impulse_q16: u32,
    override_speed_q16: Option<Q16_16>,
) -> u32 {
    let mass = attacker.mass_q16 as u64;
    let speed_q16 = override_speed_q16
        .map(|s| s.raw() as u64)
        .unwrap_or(attacker.velocity_q16 as u64);
    let momentum_q32 = (mass.saturating_mul(speed_q16)) >> 16;
    let def_density = defender.density_q16.max(1) as u64;
    let ratio_q16 = ((momentum_q32 << 16) / def_density).min(u32::MAX as u64) as u32;
    // Scale base impulse by density ratio (0.5x..~2x) and add momentum contribution.
    let scale_q16 = (32_768u64 + (ratio_q16 as u64 / 2)).min(90_000);
    let mut impulse = (base_impulse_q16 as u64)
        .saturating_mul(scale_q16)
        .saturating_div(65_536)
        .saturating_add(momentum_q32.min(120_000) as u64);
    // Traction
    impulse = (impulse.saturating_mul(traction_q16.raw() as u64)) >> 16;
    impulse.min(u32::MAX as u64) as u32
}

fn stub_attacker_snapshot(mass_q16: u32, velocity_q16: u32) -> FormationSnapshot {
    FormationSnapshot {
        formation_id: 0,
        posture: 0,
        stance: 0,
        generation: 0,
        cohesion_q16: 0,
        fatigue_q16: 0,
        ammo: 0,
        morale_q16: 50_000,
        facing_deg_q16: 0,
        position_x_q16: 0,
        position_z_q16: 0,
        command_delay_ms: 0,
        retreat_mode_flags: 0,
        charge_posture: 0,
        braced: false,
        density_q16: 40_000,
        mass_q16,
        variance_q16: 20_000,
        damping_q16: 12_000,
        velocity_q16,
        physics_flags: 0,
        gap_close_q16: 65_536,
        rank_variance_scale_q16: 65_536,
        gap_fatigue_penalty_q16: gap_fatigue_penalty(65_536),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};

    #[test]
    fn bayonet_charge_scales_with_posture_and_brace() {
        let physics = ShockPhysicsCapsule::default();
        let attacker = FormationCapsule::new(1, 1, 0, 50, 10, 80, 120, 90, 0, 0);
        let defender = FormationCapsule::new(2, 1, 0, 50, 10, 80, 120, 90, 0, 0);
        attacker.set_charge_posture(4);
        defender.set_braced(true);
        let att_snap = attacker.snapshot();
        let def_snap = defender.snapshot();
        let traction = Q16_16::from_f32(0.85);
        let outcome = physics.bayonet_charge(&att_snap, &def_snap, traction);
        assert!(outcome.morale_penalty_q16 > 0);
        assert!(outcome.cohesion_penalty_q16 > 0);
        assert!(outcome.casualties > 0);
    }

    #[test]
    fn cavalry_charge_penalizes_unbraced_targets_more() {
        let physics = ShockPhysicsCapsule::default();
        let defender = FormationCapsule::new(2, 1, 0, 50, 10, 80, 120, 90, 0, 0);
        let mut def_snap = defender.snapshot();
        def_snap.braced = false;
        let traction = Q16_16::from_f32(1.0);
        let unbraced = physics.cavalry_charge(Q16_16::from_f32(6.0), 550, &def_snap, traction);
        let mut def_snap_braced = def_snap;
        def_snap_braced.braced = true;
        let braced = physics.cavalry_charge(Q16_16::from_f32(6.0), 550, &def_snap_braced, traction);
        assert!(unbraced.morale_penalty_q16 > braced.morale_penalty_q16);
        assert!(unbraced.casualties >= braced.casualties);
    }

    #[test]
    fn terrain_traction_reduces_charge_effect() {
        let physics = ShockPhysicsCapsule::default();
        let attacker = FormationCapsule::new(3, 1, 0, 50, 10, 80, 120, 90, 0, 0);
        let defender = FormationCapsule::new(4, 1, 0, 50, 10, 80, 120, 90, 0, 0);
        let mut grid = TerrainGridCapsule::new(
            2,
            2,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 50_000,
                material: 0,
            },
        );
        let att = attacker.snapshot();
        let def = defender.snapshot();
        let traction_slow = physics.bayonet_with_terrain(&att, &def, &grid);
        // Now clear mud to boost traction
        grid.set_tile(
            0,
            0,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 0,
                material: 0,
            },
        );
        let traction_fast = physics.bayonet_with_terrain(&att, &def, &grid);
        assert!(traction_fast.morale_penalty_q16 >= traction_slow.morale_penalty_q16);
    }
}
