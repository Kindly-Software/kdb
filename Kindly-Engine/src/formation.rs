use crate::order::{
    unpack_brace_payload, unpack_charge_meta, unpack_move_payload, unpack_posture_payload,
    unpack_retreat_meta, unpack_retreat_payload, OrderData, OrderKind,
};
use crate::physics::{PhysicsPreset, PhysicsProfile};
use crate::telemetry::TelemetryCapsule;
use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RetreatMode {
    None = 0,
    FallBack = 1,
    Withdraw = 2,
}

/// Formation state capsule (line/column/square) with morale/fatigue/ammo.
///
/// - Alignment: 128B to isolate from neighbors.
/// - Size: 256B padded to full two-cache-line footprint for future fields.
#[repr(C, align(128))]
pub struct FormationCapsule {
    /// Packed bits: formation_id(24) | posture(8) | stance(8) | gen(24)
    header: AtomicU64,
    /// Cohesion (Q16.16, 0.0-1.0 range)
    cohesion_q16: AtomicU64,
    /// Fatigue (Q16.16, 0.0-1.0 range)
    fatigue_q16: AtomicU64,
    /// Ammo remaining (units)
    ammo: AtomicU64,
    /// Morale (Q16.16, 0.0-1.0 range)
    morale_q16: AtomicU64,
    /// Facing degrees (Q16.16)
    facing_deg_q16: AtomicU64,
    /// Position meters (Q16.16 x/z)
    position_x_q16: AtomicU64,
    position_z_q16: AtomicU64,
    /// Command latency applied to next order (ms)
    command_delay_ms: AtomicU64,
    /// Retreat/fallback state
    retreat_mode: AtomicU64,
    /// Charge state: bits charge_posture(8) | braced(1) | reserved
    charge_state: AtomicU64,
    /// Physics: density/mass/variance/damping/velocity
    physics_density_q16: AtomicU64,
    physics_mass_q16: AtomicU64,
    physics_variance_q16: AtomicU64,
    physics_damping_q16: AtomicU64,
    physics_velocity_q16: AtomicU64,
    physics_flags: AtomicU64,
    gap_close_scale_q16: AtomicU64,
    rank_variance_scale_q16: AtomicU64,
    /// Padding to 256B
    _padding: [u8; 104],
}

impl FormationCapsule {
    pub fn new(
        formation_id: u32,
        posture: u8,
        stance: u8,
        cohesion_q16: u32,
        fatigue_q16: u32,
        morale_q16: u32,
        ammo: u32,
        facing_deg_q16: u32,
        position_x_q16: u32,
        position_z_q16: u32,
    ) -> Self {
        let header = pack_header(formation_id, posture, stance, 0);
        let phys = PhysicsPreset::LineInfantry.profile();
        Self {
            header: AtomicU64::new(header),
            cohesion_q16: AtomicU64::new(cohesion_q16 as u64),
            fatigue_q16: AtomicU64::new(fatigue_q16 as u64),
            ammo: AtomicU64::new(ammo as u64),
            morale_q16: AtomicU64::new(morale_q16 as u64),
            facing_deg_q16: AtomicU64::new(facing_deg_q16 as u64),
            position_x_q16: AtomicU64::new(position_x_q16 as u64),
            position_z_q16: AtomicU64::new(position_z_q16 as u64),
            command_delay_ms: AtomicU64::new(0),
            retreat_mode: AtomicU64::new(RetreatMode::None as u64),
            charge_state: AtomicU64::new(0),
            physics_density_q16: AtomicU64::new(phys.density_q16 as u64),
            physics_mass_q16: AtomicU64::new(phys.mass_q16 as u64),
            physics_variance_q16: AtomicU64::new(phys.variance_q16 as u64),
            physics_damping_q16: AtomicU64::new(phys.damping_q16 as u64),
            physics_velocity_q16: AtomicU64::new(phys.velocity_q16 as u64),
            physics_flags: AtomicU64::new(flags_from_profile(phys)),
            gap_close_scale_q16: AtomicU64::new(65_536),
            rank_variance_scale_q16: AtomicU64::new(65_536),
            _padding: [0; 104],
        }
    }

    /// Spawn a formation and immediately apply a physics preset (unit archetype).
    /// Use this for new units so density/mass/variance/damping/velocity match the archetype at birth.
    pub fn new_with_preset(
        formation_id: u32,
        posture: u8,
        stance: u8,
        cohesion_q16: u32,
        fatigue_q16: u32,
        morale_q16: u32,
        ammo: u32,
        facing_deg_q16: u32,
        position_x_q16: u32,
        position_z_q16: u32,
        preset: PhysicsPreset,
    ) -> Self {
        let capsule = Self::new(
            formation_id,
            posture,
            stance,
            cohesion_q16,
            fatigue_q16,
            morale_q16,
            ammo,
            facing_deg_q16,
            position_x_q16,
            position_z_q16,
        );
        capsule.apply_physics_preset(preset);
        capsule
    }

    /// Convenience spawn helpers for common archetypes (preferred for new formations).
    pub fn spawn_line(
        formation_id: u32,
        posture: u8,
        stance: u8,
        cohesion_q16: u32,
        fatigue_q16: u32,
        morale_q16: u32,
        ammo: u32,
        facing_deg_q16: u32,
        position_x_q16: u32,
        position_z_q16: u32,
    ) -> Self {
        Self::new_with_preset(
            formation_id,
            posture,
            stance,
            cohesion_q16,
            fatigue_q16,
            morale_q16,
            ammo,
            facing_deg_q16,
            position_x_q16,
            position_z_q16,
            PhysicsPreset::LineInfantry,
        )
    }

    pub fn spawn_guard(
        formation_id: u32,
        posture: u8,
        stance: u8,
        cohesion_q16: u32,
        fatigue_q16: u32,
        morale_q16: u32,
        ammo: u32,
        facing_deg_q16: u32,
        position_x_q16: u32,
        position_z_q16: u32,
    ) -> Self {
        Self::new_with_preset(
            formation_id,
            posture,
            stance,
            cohesion_q16,
            fatigue_q16,
            morale_q16,
            ammo,
            facing_deg_q16,
            position_x_q16,
            position_z_q16,
            PhysicsPreset::OldGuard,
        )
    }

    pub fn spawn_skirmisher(
        formation_id: u32,
        posture: u8,
        stance: u8,
        cohesion_q16: u32,
        fatigue_q16: u32,
        morale_q16: u32,
        ammo: u32,
        facing_deg_q16: u32,
        position_x_q16: u32,
        position_z_q16: u32,
    ) -> Self {
        Self::new_with_preset(
            formation_id,
            posture,
            stance,
            cohesion_q16,
            fatigue_q16,
            morale_q16,
            ammo,
            facing_deg_q16,
            position_x_q16,
            position_z_q16,
            PhysicsPreset::Skirmisher,
        )
    }

    pub fn spawn_grenadier(
        formation_id: u32,
        posture: u8,
        stance: u8,
        cohesion_q16: u32,
        fatigue_q16: u32,
        morale_q16: u32,
        ammo: u32,
        facing_deg_q16: u32,
        position_x_q16: u32,
        position_z_q16: u32,
    ) -> Self {
        Self::new_with_preset(
            formation_id,
            posture,
            stance,
            cohesion_q16,
            fatigue_q16,
            morale_q16,
            ammo,
            facing_deg_q16,
            position_x_q16,
            position_z_q16,
            PhysicsPreset::Grenadier,
        )
    }

    pub fn retreat_state(&self) -> (RetreatMode, bool) {
        let flags = self.retreat_mode.load(Ordering::Relaxed);
        let mode = match (flags & 0xFF) as u8 {
            1 => RetreatMode::FallBack,
            2 => RetreatMode::Withdraw,
            _ => RetreatMode::None,
        };
        let backstep = (flags & RETREAT_BACKSTEP_FLAG) != 0;
        (mode, backstep)
    }

    /// Atomically snapshot formation state.
    pub fn snapshot(&self) -> FormationSnapshot {
        let header = self.header.load(Ordering::Relaxed);
        FormationSnapshot {
            formation_id: (header & FORMATION_ID_MASK) as u32,
            posture: ((header >> 24) & 0xFF) as u8,
            stance: ((header >> 32) & 0xFF) as u8,
            generation: ((header >> 40) & 0xFFFFFF) as u32,
            cohesion_q16: self.cohesion_q16.load(Ordering::Relaxed) as u32,
            fatigue_q16: self.fatigue_q16.load(Ordering::Relaxed) as u32,
            ammo: self.ammo.load(Ordering::Relaxed) as u32,
            morale_q16: self.morale_q16.load(Ordering::Relaxed) as u32,
            facing_deg_q16: self.facing_deg_q16.load(Ordering::Relaxed) as u32,
            position_x_q16: self.position_x_q16.load(Ordering::Relaxed) as u32,
            position_z_q16: self.position_z_q16.load(Ordering::Relaxed) as u32,
            command_delay_ms: self.command_delay_ms.load(Ordering::Relaxed) as u32,
            retreat_mode_flags: self.retreat_mode.load(Ordering::Relaxed) as u16,
            charge_posture: (self.charge_state.load(Ordering::Relaxed) & 0xFF) as u8,
            braced: (self.charge_state.load(Ordering::Relaxed) & CHARGE_BRACED_FLAG) != 0,
            gap_close_q16: self.gap_close_scale_q16.load(Ordering::Relaxed) as u32,
            rank_variance_scale_q16: self.rank_variance_scale_q16.load(Ordering::Relaxed) as u32,
            gap_fatigue_penalty_q16: gap_fatigue_penalty(
                self.gap_close_scale_q16.load(Ordering::Relaxed) as u32,
            ),
            density_q16: ((self.physics_density_q16.load(Ordering::Relaxed) as u64
                * self.gap_close_scale_q16.load(Ordering::Relaxed))
                / 65_536) as u32,
            mass_q16: self.physics_mass_q16.load(Ordering::Relaxed) as u32,
            variance_q16: ((self.physics_variance_q16.load(Ordering::Relaxed) as u64
                * self.rank_variance_scale_q16.load(Ordering::Relaxed))
                / 65_536) as u32,
            damping_q16: self.physics_damping_q16.load(Ordering::Relaxed) as u32,
            velocity_q16: self.physics_velocity_q16.load(Ordering::Relaxed) as u32,
            physics_flags: self.physics_flags.load(Ordering::Relaxed) as u16,
        }
    }

    /// Apply a decoded order and emit telemetry side effects.
    pub fn apply_order(&self, order: &OrderData, telemetry: &TelemetryCapsule) {
        match order.kind {
            OrderKind::Move => {
                let (x, z) = unpack_move_payload(order.payload_a);
                self.set_position_q16(x, z);
                telemetry.log_event();
            }
            OrderKind::ChangePosture => {
                let (posture, stance) = unpack_posture_payload(order.payload_a);
                self.set_posture(posture, stance);
                telemetry.log_event();
            }
            OrderKind::Fire => {
                let mut ammo_spent = (order.payload_a & 0xFFFF) as u32;
                // Packet loss under stress: reduce effective volleys if morale is low.
                if self.packet_loss_under_stress() {
                    ammo_spent = ammo_spent.saturating_sub(ammo_spent / 10);
                }
                self.spend_ammo(ammo_spent);
                // Rotate ranks to keep a fresh firing line; will relax over time.
                self.rotate_ranks();
                telemetry.add_ammo_spent(ammo_spent);
                telemetry.log_event();
            }
            OrderKind::Hold | OrderKind::ArtilleryFire => {
                telemetry.log_event();
            }
            OrderKind::FireControl => {
                // Formation-level acknowledgement for fire-control; ammo handled in ballistics path.
                telemetry.log_event();
            }
            OrderKind::FallBack | OrderKind::Withdraw => {
                self.apply_retreat(order, telemetry);
            }
            OrderKind::Charge => {
                let (_x, _z) = unpack_move_payload(order.payload_a);
                let (charge_posture, _commit) = unpack_charge_meta(order.payload_b);
                self.set_charge_posture(charge_posture);
                self.set_braced(false);
                telemetry.log_event();
            }
            OrderKind::Brace => {
                let braced = unpack_brace_payload(order.payload_a);
                self.set_braced(braced);
                telemetry.log_event();
            }
            OrderKind::SetFireDoctrine => {
                // Doctrine updates are applied in tick; acknowledge for telemetry.
                telemetry.log_event();
            }
            OrderKind::Grenade => {
                telemetry.log_event();
            }
            OrderKind::GarrisonEnter | OrderKind::GarrisonExit => {
                telemetry.log_event();
            }
        }
    }

    pub fn set_position_q16(&self, x_q16: u32, z_q16: u32) {
        self.position_x_q16.store(x_q16 as u64, Ordering::Release);
        self.position_z_q16.store(z_q16 as u64, Ordering::Release);
    }

    /// Morale tick using fixed-point penalties (deterministic).
    pub fn morale_tick(&self, casualties: u32, fatigue_q16: u32) {
        // penalty = casualties factor + fatigue factor (scaled in Q16.16)
        let casualty_penalty = (casualties as i64 * 512).min(30_000); // up to ~0.45 drop
        let fatigue_penalty = (fatigue_q16 as i64 / 4).min(15_000);
        let total_penalty = casualty_penalty + fatigue_penalty;
        self.update_q16(&self.morale_q16, -(total_penalty as i32));
        // Gradually relax rank rotation toward baseline.
        let rv = self.rank_variance_scale_q16.load(Ordering::Relaxed) as u32;
        if rv < 65_536 {
            let next = (rv.saturating_add(2_000)).min(65_536);
            self.rank_variance_scale_q16
                .store(next as u64, Ordering::Relaxed);
        }
        // If not braced, relax gap-close back to baseline.
        let braced = (self.charge_state.load(Ordering::Relaxed) & CHARGE_BRACED_FLAG) != 0;
        if !braced {
            self.gap_close_scale_q16.store(65_536, Ordering::Relaxed);
        }
    }

    /// Update posture/stance and bump generation.
    pub fn set_posture(&self, posture: u8, stance: u8) {
        self.bump_generation_with(|id, gen| pack_header(id, posture, stance, gen));
    }

    fn bump_generation_with<F: Fn(u32, u32) -> u64>(&self, f: F) {
        let mut current = self.header.load(Ordering::Relaxed);
        loop {
            let id = (current & FORMATION_ID_MASK) as u32;
            let gen = ((current >> 40) & 0xFFFFFF) as u32;
            let next = f(id, gen + 1);
            match self
                .header
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
    }

    fn bump_generation(&self) {
        let header = self.header.load(Ordering::Relaxed);
        let id = (header & FORMATION_ID_MASK) as u32;
        let posture = ((header >> 24) & 0xFF) as u8;
        let stance = ((header >> 32) & 0xFF) as u8;
        self.bump_generation_with(|_, gen| pack_header(id, posture, stance, gen));
    }

    pub fn adjust_morale(&self, delta_q16: i32) {
        self.update_q16(&self.morale_q16, delta_q16);
    }

    /// Apply a deterministic morale penalty (Q16.16); no-op if zero.
    pub fn apply_shock_penalty_q16(&self, penalty_q16: u32) {
        if penalty_q16 == 0 {
            return;
        }
        self.update_q16(&self.morale_q16, -(penalty_q16 as i32));
    }

    /// Apply a combined shock package: morale drop, cohesion hit, and fatigue bump.
    pub fn apply_shock_package(
        &self,
        morale_penalty_q16: u32,
        cohesion_penalty_q16: u32,
        fatigue_delta_q16: u32,
    ) {
        if morale_penalty_q16 > 0 {
            self.apply_shock_penalty_q16(morale_penalty_q16);
        }
        if cohesion_penalty_q16 > 0 {
            self.update_q16(&self.cohesion_q16, -(cohesion_penalty_q16 as i32));
        }
        if fatigue_delta_q16 > 0 {
            self.adjust_fatigue(fatigue_delta_q16 as i32);
        }
    }

    pub fn adjust_fatigue(&self, delta_q16: i32) {
        self.update_q16(&self.fatigue_q16, delta_q16);
    }

    pub fn set_charge_posture(&self, charge_posture: u8) {
        self.charge_state
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |state| {
                let braced = state & CHARGE_BRACED_FLAG;
                Some(braced | charge_posture as u64)
            })
            .ok();
    }

    pub fn set_braced(&self, braced: bool) {
        self.charge_state
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |state| {
                let charge = state & 0xFF;
                let flag = if braced { CHARGE_BRACED_FLAG } else { 0 };
                Some(charge | flag)
            })
            .ok();
        // Bracing tightens ranks; unbracing relaxes.
        self.set_gap_close(braced);
    }

    /// Tighten ranks to close gaps (higher density, better square integrity).
    pub fn set_gap_close(&self, tight: bool) {
        let scale = if tight { 72_000 } else { 65_536 };
        self.gap_close_scale_q16.store(scale, Ordering::Release);
        self.bump_generation();
    }

    /// Rotate front ranks to fresh shooters (reduces variance until relaxed).
    pub fn rotate_ranks(&self) {
        self.rank_variance_scale_q16
            .store(55_000, Ordering::Release);
        self.bump_generation();
    }

    /// Restore rank variance to baseline.
    pub fn relax_ranks(&self) {
        self.rank_variance_scale_q16
            .store(65_536, Ordering::Release);
        self.bump_generation();
    }

    pub fn spend_ammo(&self, amount: u32) {
        self.ammo
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |v| {
                v.checked_sub(amount as u64)
            })
            .ok();
    }

    /// Resupply ammo deterministically (saturating at u32::MAX).
    pub fn resupply_ammo(&self, amount: u32) {
        if amount == 0 {
            return;
        }
        self.ammo
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |v| {
                let next = v.saturating_add(amount as u64).min(u32::MAX as u64);
                Some(next)
            })
            .ok();
    }

    /// Apply a physics profile (density/mass/variance/damping/velocity).
    pub fn apply_physics_profile(&self, profile: PhysicsProfile) {
        self.physics_density_q16
            .store(profile.density_q16 as u64, Ordering::Release);
        self.physics_mass_q16
            .store(profile.mass_q16 as u64, Ordering::Release);
        self.physics_variance_q16
            .store(profile.variance_q16 as u64, Ordering::Release);
        self.physics_damping_q16
            .store(profile.damping_q16 as u64, Ordering::Release);
        self.physics_velocity_q16
            .store(profile.velocity_q16 as u64, Ordering::Release);
        self.physics_flags
            .store(flags_from_profile(profile), Ordering::Release);
        self.rank_variance_scale_q16
            .store(65_536, Ordering::Release);
        self.gap_close_scale_q16.store(65_536, Ordering::Release);
    }

    /// Apply a physics preset (Line/OldGuard/Skirmisher/Cavalry/etc.).
    pub fn apply_physics_preset(&self, preset: PhysicsPreset) {
        self.apply_physics_profile(preset.profile());
    }

    fn packet_loss_under_stress(&self) -> bool {
        let flags = self.physics_flags.load(Ordering::Relaxed);
        let morale = self.morale_q16.load(Ordering::Relaxed) as u32;
        flags & PHYSICS_PACKET_LOSS != 0 && morale < 40_000
    }

    fn update_q16(&self, target: &AtomicU64, delta_q16: i32) {
        let mut current = target.load(Ordering::Relaxed) as i64;
        loop {
            let next = ((current + delta_q16 as i64).clamp(0, u32::MAX as i64)) as u64;
            match target.compare_exchange(current as u64, next, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(observed) => current = observed as i64,
            }
        }
    }

    fn apply_retreat(&self, order: &OrderData, telemetry: &TelemetryCapsule) {
        let (target_x, target_z) = unpack_retreat_payload(order.payload_a);
        let (backstep, command_delay_ms, suppression) = unpack_retreat_meta(order.payload_b);
        self.command_delay_ms
            .store(command_delay_ms as u64, Ordering::Release);
        let mode = if order.kind == OrderKind::Withdraw {
            RetreatMode::Withdraw
        } else {
            RetreatMode::FallBack
        };
        self.retreat_mode
            .store(mode as u64 | ((backstep as u64) << 8), Ordering::Release);
        self.set_position_q16(target_x, target_z);
        // Apply a small morale/cohesion penalty when retreating under pressure.
        if suppression > 0 {
            let morale_penalty = -(suppression as i32 * 8);
            let cohesion_penalty = -(suppression as i32 * 4);
            self.adjust_morale(morale_penalty);
            self.update_q16(&self.cohesion_q16, cohesion_penalty);
            telemetry.log_morale_shock();
        }
        telemetry.record_retreat();
        telemetry.log_event();
    }
}

verify_capsule_properties!(FormationCapsule, 128, 256);

#[derive(Debug, Clone, Copy)]
pub struct FormationSnapshot {
    pub formation_id: u32,
    pub posture: u8,
    pub stance: u8,
    pub generation: u32,
    pub cohesion_q16: u32,
    pub fatigue_q16: u32,
    pub ammo: u32,
    pub morale_q16: u32,
    pub facing_deg_q16: u32,
    pub position_x_q16: u32,
    pub position_z_q16: u32,
    pub command_delay_ms: u32,
    pub retreat_mode_flags: u16,
    pub charge_posture: u8,
    pub braced: bool,
    pub density_q16: u32,
    pub mass_q16: u32,
    pub variance_q16: u32,
    pub damping_q16: u32,
    pub velocity_q16: u32,
    pub physics_flags: u16,
    pub gap_close_q16: u32,
    pub rank_variance_scale_q16: u32,
    /// Derived: fatigue penalty hint when marching tight (optional overlay/debug)
    pub gap_fatigue_penalty_q16: u32,
}

const FORMATION_ID_MASK: u64 = 0xFFFFFF;
const RETREAT_BACKSTEP_FLAG: u64 = 1 << 8;
const CHARGE_BRACED_FLAG: u64 = 1 << 8;
const PHYSICS_PERMEABLE: u64 = 1 << 0;
const PHYSICS_PACKET_LOSS: u64 = 1 << 1;

#[inline(always)]
pub(crate) fn gap_fatigue_penalty(scale_q16: u32) -> u32 {
    if scale_q16 <= 65_536 {
        ((65_536 - scale_q16) / 8).min(4_000)
    } else {
        ((scale_q16 - 65_536) / 4).min(8_000)
    }
}

#[inline(always)]
fn pack_header(formation_id: u32, posture: u8, stance: u8, generation: u32) -> u64 {
    (formation_id as u64 & FORMATION_ID_MASK)
        | ((posture as u64) << 24)
        | ((stance as u64) << 32)
        | (((generation & 0xFFFFFF) as u64) << 40)
}

const fn flags_from_profile(profile: PhysicsProfile) -> u64 {
    let mut f = 0;
    if profile.permeable {
        f |= PHYSICS_PERMEABLE;
    }
    if profile.packet_loss_under_stress {
        f |= PHYSICS_PACKET_LOSS;
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formation_snapshot_updates() {
        let capsule = FormationCapsule::new(42, 1, 2, 50, 10, 80, 120, 90, 0, 0);
        let snap = capsule.snapshot();
        assert_eq!(snap.formation_id, 42);
        assert_eq!(snap.posture, 1);
        assert_eq!(snap.stance, 2);

        capsule.set_posture(3, 4);
        let snap2 = capsule.snapshot();
        assert_eq!(snap2.posture, 3);
        assert_eq!(snap2.stance, 4);
        assert!(snap2.generation > snap.generation);

        capsule.adjust_morale(-5);
        capsule.adjust_fatigue(5);
        capsule.spend_ammo(10);
        let snap3 = capsule.snapshot();
        assert!(snap3.morale_q16 <= snap2.morale_q16);
        assert!(snap3.fatigue_q16 >= snap2.fatigue_q16);
        assert!(snap3.ammo <= snap2.ammo);
    }

    #[test]
    fn resupply_ammo_adds_rounds() {
        let capsule = FormationCapsule::new(7, 0, 0, 50, 10, 80, 0, 0, 0, 0);
        capsule.resupply_ammo(25);
        let snap = capsule.snapshot();
        assert!(snap.ammo >= 25);
    }

    #[test]
    fn bracing_tightens_gap_close() {
        let capsule = FormationCapsule::new(1, 0, 0, 50, 10, 80, 120, 90, 0, 0);
        let snap = capsule.snapshot();
        assert_eq!(snap.gap_close_q16, 65_536);
        capsule.set_braced(true);
        let snap_braced = capsule.snapshot();
        assert!(snap_braced.braced);
        assert!(snap_braced.gap_close_q16 > snap.gap_close_q16);
        capsule.set_braced(false);
        let snap_relaxed = capsule.snapshot();
        assert_eq!(snap_relaxed.gap_close_q16, 65_536);
    }

    #[test]
    fn apply_orders_updates_state() {
        let capsule = FormationCapsule::new(10, 0, 0, 50, 10, 80, 120, 90, 0, 0);
        let telemetry = TelemetryCapsule::new();

        let move_order = OrderData {
            kind: OrderKind::Move,
            formation_id: 10,
            generation: 0,
            payload_a: crate::order::pack_move_payload(1000, 2000),
            payload_b: 0,
        };
        capsule.apply_order(&move_order, &telemetry);
        let snap = capsule.snapshot();
        assert_eq!(snap.position_x_q16, 1000);
        assert_eq!(snap.position_z_q16, 2000);

        let change_posture = OrderData {
            kind: OrderKind::ChangePosture,
            formation_id: 10,
            generation: 0,
            payload_a: crate::order::pack_posture_payload(2, 3),
            payload_b: 0,
        };
        capsule.apply_order(&change_posture, &telemetry);
        let snap2 = capsule.snapshot();
        assert_eq!(snap2.posture, 2);
        assert_eq!(snap2.stance, 3);

        let retreat_order = OrderData {
            kind: OrderKind::FallBack,
            formation_id: 10,
            generation: 0,
            payload_a: crate::order::pack_retreat_payload(900, 1800),
            payload_b: crate::order::pack_retreat_meta(true, 250, 5),
        };
        capsule.apply_order(&retreat_order, &telemetry);
        let snap3 = capsule.snapshot();
        assert_eq!(snap3.position_x_q16, 900);
        assert_eq!(snap3.command_delay_ms, 250);
        assert_ne!(snap3.retreat_mode_flags, 0);
    }

    #[test]
    fn charge_and_brace_orders_update_state() {
        let capsule = FormationCapsule::new(11, 0, 0, 50, 10, 80, 120, 90, 0, 0);
        let telemetry = TelemetryCapsule::new();

        let charge_order = OrderData {
            kind: OrderKind::Charge,
            formation_id: 11,
            generation: 0,
            payload_a: crate::order::pack_move_payload(200, 300),
            payload_b: crate::order::pack_charge_meta(5, true),
        };
        capsule.apply_order(&charge_order, &telemetry);
        let snap = capsule.snapshot();
        assert_eq!(snap.position_x_q16, 0);
        assert_eq!(snap.position_z_q16, 0);
        assert_eq!(snap.charge_posture, 5);
        assert!(!snap.braced);

        let brace_order = OrderData {
            kind: OrderKind::Brace,
            formation_id: 11,
            generation: 0,
            payload_a: crate::order::pack_brace_payload(true),
            payload_b: 0,
        };
        capsule.apply_order(&brace_order, &telemetry);
        let snap2 = capsule.snapshot();
        assert!(snap2.braced);
    }

    #[test]
    fn morale_tick_reduces_value() {
        let capsule = FormationCapsule::new(1, 0, 0, 50_000, 10_000, 60_000, 0, 0, 0, 0);
        capsule.morale_tick(10, 20_000);
        let snap = capsule.snapshot();
        assert!(snap.morale_q16 < 60_000);
    }

    #[test]
    fn shock_penalty_applies_drop() {
        let capsule = FormationCapsule::new(2, 0, 0, 50_000, 10_000, 55_000, 0, 0, 0, 0);
        capsule.apply_shock_penalty_q16(1_000);
        let snap = capsule.snapshot();
        assert!(snap.morale_q16 < 55_000);
    }

    #[test]
    fn shock_package_hits_morale_cohesion_and_fatigue() {
        let capsule = FormationCapsule::new(4, 0, 0, 50_000, 10_000, 60_000, 0, 0, 0, 0);
        let before = capsule.snapshot();
        capsule.apply_shock_package(5_000, 4_000, 3_000);
        let after = capsule.snapshot();
        assert!(after.morale_q16 < before.morale_q16);
        assert!(after.cohesion_q16 < before.cohesion_q16);
        assert!(after.fatigue_q16 > before.fatigue_q16);
    }
    #[test]
    fn apply_physics_preset_updates_snapshot() {
        let capsule = FormationCapsule::new(5, 0, 0, 50, 10, 80, 120, 90, 0, 0);
        capsule.apply_physics_preset(crate::physics::PhysicsPreset::OldGuard);
        let snap = capsule.snapshot();
        assert!(snap.density_q16 >= 60_000);
        assert_eq!(
            snap.velocity_q16,
            crate::physics::PhysicsPreset::OldGuard
                .profile()
                .velocity_q16
        );
    }

    #[test]
    fn new_with_preset_applies_archetype_physics() {
        let capsule = FormationCapsule::new_with_preset(
            6,
            0,
            0,
            40_000,
            8_000,
            60_000,
            120,
            90,
            0,
            0,
            crate::physics::PhysicsPreset::Cuirassier,
        );
        let snap = capsule.snapshot();
        let profile = crate::physics::PhysicsPreset::Cuirassier.profile();
        assert_eq!(snap.velocity_q16, profile.velocity_q16);
        assert_eq!(snap.mass_q16, profile.mass_q16);
    }
}
