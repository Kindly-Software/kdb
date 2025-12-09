use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

const GENERAL_FLAG_CHARIOT: u32 = 1;

/// Command aura capsule for generals (morale/fatigue effects in proximity).
///
/// - Alignment: 128B, size 128B to avoid false sharing.
/// - Deterministic: aura radius, morale bonus, and fatigue recovery are fixed per snapshot.
#[repr(C, align(128))]
pub struct GeneralCapsule {
    generation: AtomicU64,
    position_x_q16: AtomicU32,
    position_z_q16: AtomicU32,
    aura_radius_sq_q16: AtomicU64,
    morale_boost_q16: AtomicU32,
    fatigue_recovery_q16: AtomicU32,
    flags: AtomicU32,
    _padding: [u8; 76],
}

impl GeneralCapsule {
    pub const fn new(
        position_x_q16: u32,
        position_z_q16: u32,
        aura_radius_q16: u32,
        morale_boost_q16: u32,
        fatigue_recovery_q16: u32,
        chariot: bool,
    ) -> Self {
        let radius_sq = (aura_radius_q16 as u64).saturating_mul(aura_radius_q16 as u64);
        Self {
            generation: AtomicU64::new(0),
            position_x_q16: AtomicU32::new(position_x_q16),
            position_z_q16: AtomicU32::new(position_z_q16),
            aura_radius_sq_q16: AtomicU64::new(radius_sq),
            morale_boost_q16: AtomicU32::new(morale_boost_q16),
            fatigue_recovery_q16: AtomicU32::new(fatigue_recovery_q16),
            flags: AtomicU32::new(if chariot { GENERAL_FLAG_CHARIOT } else { 0 }),
            _padding: [0; 76],
        }
    }

    pub fn snapshot(&self) -> GeneralSnapshot {
        GeneralSnapshot {
            generation: self.generation.load(Ordering::Relaxed) as u32,
            position_x_q16: self.position_x_q16.load(Ordering::Relaxed),
            position_z_q16: self.position_z_q16.load(Ordering::Relaxed),
            aura_radius_sq_q16: self.aura_radius_sq_q16.load(Ordering::Relaxed),
            morale_boost_q16: self.morale_boost_q16.load(Ordering::Relaxed),
            fatigue_recovery_q16: self.fatigue_recovery_q16.load(Ordering::Relaxed),
            chariot: self.flags.load(Ordering::Relaxed) & GENERAL_FLAG_CHARIOT != 0,
        }
    }

    pub fn set_position(&self, x_q16: u32, z_q16: u32) {
        self.position_x_q16.store(x_q16, Ordering::Release);
        self.position_z_q16.store(z_q16, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

verify_capsule_properties!(GeneralCapsule, 128, 128);

#[derive(Debug, Clone, Copy)]
pub struct GeneralSnapshot {
    pub generation: u32,
    pub position_x_q16: u32,
    pub position_z_q16: u32,
    pub aura_radius_sq_q16: u64,
    pub morale_boost_q16: u32,
    pub fatigue_recovery_q16: u32,
    pub chariot: bool,
}

impl GeneralSnapshot {
    /// Returns true if the target position is within the aura radius.
    pub fn in_aura(&self, target_x_q16: u32, target_z_q16: u32) -> bool {
        let dx = target_x_q16 as i64 - self.position_x_q16 as i64;
        let dz = target_z_q16 as i64 - self.position_z_q16 as i64;
        let dist_sq = dx.unsigned_abs().saturating_mul(dx.unsigned_abs())
            + dz.unsigned_abs().saturating_mul(dz.unsigned_abs());
        dist_sq <= self.aura_radius_sq_q16
    }
}

/// Snapshot a slice of general capsules for tick-time aura application.
pub fn snapshot_generals(generals: &[GeneralCapsule]) -> Vec<GeneralSnapshot> {
    generals.iter().map(GeneralCapsule::snapshot).collect()
}
