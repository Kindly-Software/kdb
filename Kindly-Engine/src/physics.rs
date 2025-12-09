use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU64, Ordering};

/// Formation-level physics capsule (fixed-point, deterministic).
///
/// Fields use Q16.16 scaling for density/mass/variance/damping/velocity.
#[repr(C, align(128))]
pub struct FormationPhysicsCapsule {
    density_q16: AtomicU64,
    mass_q16: AtomicU64,
    variance_q16: AtomicU64,
    damping_q16: AtomicU64,
    velocity_q16: AtomicU64,
    flags: AtomicU64,
    _padding: [u8; 80],
}

const FLAG_PERMEABLE: u64 = 1 << 0;
const FLAG_PACKET_LOSS: u64 = 1 << 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsPreset {
    LineInfantry,
    OldGuard,
    Skirmisher,
    Cuirassier,
    Hussar,
    Artillery,
    Grenadier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicsProfile {
    pub density_q16: u32,
    pub mass_q16: u32,
    pub variance_q16: u32,
    pub damping_q16: u32,
    pub velocity_q16: u32,
    pub permeable: bool,
    pub packet_loss_under_stress: bool,
}

impl FormationPhysicsCapsule {
    pub const fn new(profile: PhysicsProfile) -> Self {
        Self {
            density_q16: AtomicU64::new(profile.density_q16 as u64),
            mass_q16: AtomicU64::new(profile.mass_q16 as u64),
            variance_q16: AtomicU64::new(profile.variance_q16 as u64),
            damping_q16: AtomicU64::new(profile.damping_q16 as u64),
            velocity_q16: AtomicU64::new(profile.velocity_q16 as u64),
            flags: AtomicU64::new(flags_from_profile(profile)),
            _padding: [0; 80],
        }
    }

    /// Apply a preset (Line, Old Guard, etc.).
    pub fn set_preset(&self, preset: PhysicsPreset) {
        let p = preset.profile();
        self.density_q16
            .store(p.density_q16 as u64, Ordering::Release);
        self.mass_q16.store(p.mass_q16 as u64, Ordering::Release);
        self.variance_q16
            .store(p.variance_q16 as u64, Ordering::Release);
        self.damping_q16
            .store(p.damping_q16 as u64, Ordering::Release);
        self.velocity_q16
            .store(p.velocity_q16 as u64, Ordering::Release);
        self.flags.store(flags_from_profile(p), Ordering::Release);
    }

    pub fn snapshot(&self) -> FormationPhysicsSnapshot {
        FormationPhysicsSnapshot {
            density_q16: self.density_q16.load(Ordering::Relaxed) as u32,
            mass_q16: self.mass_q16.load(Ordering::Relaxed) as u32,
            variance_q16: self.variance_q16.load(Ordering::Relaxed) as u32,
            damping_q16: self.damping_q16.load(Ordering::Relaxed) as u32,
            velocity_q16: self.velocity_q16.load(Ordering::Relaxed) as u32,
            permeable: self.flags.load(Ordering::Relaxed) & FLAG_PERMEABLE != 0,
            packet_loss_under_stress: self.flags.load(Ordering::Relaxed) & FLAG_PACKET_LOSS != 0,
        }
    }
}

verify_capsule_properties!(FormationPhysicsCapsule, 128, 128);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormationPhysicsSnapshot {
    pub density_q16: u32,
    pub mass_q16: u32,
    pub variance_q16: u32,
    pub damping_q16: u32,
    pub velocity_q16: u32,
    pub permeable: bool,
    pub packet_loss_under_stress: bool,
}

impl PhysicsPreset {
    pub const fn profile(self) -> PhysicsProfile {
        match self {
            PhysicsPreset::LineInfantry => PhysicsProfile {
                density_q16: 40_000,
                mass_q16: 44_000,
                variance_q16: 28_000, // higher aim noise under stress
                damping_q16: 12_000,
                velocity_q16: 24_000,
                permeable: false,
                packet_loss_under_stress: true,
            },
            PhysicsPreset::OldGuard => PhysicsProfile {
                density_q16: 65_000,
                mass_q16: 52_000,
                variance_q16: 8_000,
                damping_q16: 65_000, // near-infinite damping
                velocity_q16: 22_000,
                permeable: false,
                packet_loss_under_stress: false,
            },
            PhysicsPreset::Skirmisher => PhysicsProfile {
                density_q16: 4_000,
                mass_q16: 22_000,
                variance_q16: 6_000,
                damping_q16: 20_000,
                velocity_q16: 26_000,
                permeable: true, // projectiles often pass through gaps
                packet_loss_under_stress: false,
            },
            PhysicsPreset::Cuirassier => PhysicsProfile {
                density_q16: 45_000,
                mass_q16: 65_000,
                variance_q16: 18_000,
                damping_q16: 28_000,
                velocity_q16: 40_000,
                permeable: false,
                packet_loss_under_stress: false,
            },
            PhysicsPreset::Hussar => PhysicsProfile {
                density_q16: 28_000,
                mass_q16: 32_000,
                variance_q16: 16_000,
                damping_q16: 24_000,
                velocity_q16: 50_000,
                permeable: false,
                packet_loss_under_stress: false,
            },
            PhysicsPreset::Artillery => PhysicsProfile {
                density_q16: 55_000,
                mass_q16: 70_000,
                variance_q16: 20_000,
                damping_q16: 36_000,
                velocity_q16: 0, // terrain modifier, not a mover
                permeable: false,
                packet_loss_under_stress: false,
            },
            PhysicsPreset::Grenadier => PhysicsProfile {
                density_q16: 44_000,
                mass_q16: 46_000,
                variance_q16: 14_000,
                damping_q16: 28_000,
                velocity_q16: 26_000,
                permeable: false,
                packet_loss_under_stress: false,
            },
        }
    }
}

const fn flags_from_profile(p: PhysicsProfile) -> u64 {
    let mut f = 0;
    if p.permeable {
        f |= FLAG_PERMEABLE;
    }
    if p.packet_loss_under_stress {
        f |= FLAG_PACKET_LOSS;
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_load_expected_values() {
        let line = PhysicsPreset::LineInfantry.profile();
        assert!(line.density_q16 > 30_000);
        assert!(line.variance_q16 > 20_000);
        assert!(line.packet_loss_under_stress);

        let guard = PhysicsPreset::OldGuard.profile();
        assert_eq!(guard.density_q16, 65_000);
        assert_eq!(guard.damping_q16, 65_000);
        assert!(!guard.packet_loss_under_stress);

        let skirm = PhysicsPreset::Skirmisher.profile();
        assert!(skirm.permeable);
        assert!(skirm.density_q16 < 10_000);
    }

    #[test]
    fn capsule_applies_presets() {
        let caps = FormationPhysicsCapsule::new(PhysicsPreset::LineInfantry.profile());
        let snap = caps.snapshot();
        assert_eq!(snap.density_q16, 40_000);
        assert!(snap.packet_loss_under_stress);

        caps.set_preset(PhysicsPreset::Hussar);
        let snap2 = caps.snapshot();
        assert_eq!(snap2.velocity_q16, 50_000);
        assert!(!snap2.permeable);
    }
}
