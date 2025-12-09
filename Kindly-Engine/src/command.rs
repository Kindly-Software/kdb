use crate::general::GeneralSnapshot;
use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

const CMD_FLAG_LOGISTICS: u32 = 1 << 0;
const CMD_FLAG_AGGRESSIVE: u32 = 1 << 1;

/// Commander capsule: command aura + traits for strategic chain of command.
///
/// - Alignment: 128B to avoid false sharing.
/// - Fields: command range, morale/fatigue/logistics boosts, command delay.
#[repr(C, align(128))]
pub struct CommanderCapsule {
    generation: AtomicU64,
    position_x_q16: AtomicU32,
    position_z_q16: AtomicU32,
    command_range_sq_q16: AtomicU64,
    morale_boost_q16: AtomicU32,
    fatigue_recovery_q16: AtomicU32,
    logistics_boost_q16: AtomicU32,
    command_delay_ticks: AtomicU32,
    flags: AtomicU32,
    _padding: [u8; 44],
}

impl CommanderCapsule {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        position_x_q16: u32,
        position_z_q16: u32,
        command_range_q16: u32,
        morale_boost_q16: u32,
        fatigue_recovery_q16: u32,
        logistics_boost_q16: u32,
        command_delay_ticks: u32,
        aggressive: bool,
    ) -> Self {
        let range_sq = (command_range_q16 as u64).saturating_mul(command_range_q16 as u64);
        let mut flags = CMD_FLAG_LOGISTICS;
        if aggressive {
            flags |= CMD_FLAG_AGGRESSIVE;
        }
        Self {
            generation: AtomicU64::new(0),
            position_x_q16: AtomicU32::new(position_x_q16),
            position_z_q16: AtomicU32::new(position_z_q16),
            command_range_sq_q16: AtomicU64::new(range_sq),
            morale_boost_q16: AtomicU32::new(morale_boost_q16),
            fatigue_recovery_q16: AtomicU32::new(fatigue_recovery_q16),
            logistics_boost_q16: AtomicU32::new(logistics_boost_q16),
            command_delay_ticks: AtomicU32::new(command_delay_ticks),
            flags: AtomicU32::new(flags),
            _padding: [0; 44],
        }
    }

    pub fn snapshot(&self) -> CommanderSnapshot {
        CommanderSnapshot {
            generation: self.generation.load(Ordering::Relaxed) as u32,
            position_x_q16: self.position_x_q16.load(Ordering::Relaxed),
            position_z_q16: self.position_z_q16.load(Ordering::Relaxed),
            command_range_sq_q16: self.command_range_sq_q16.load(Ordering::Relaxed),
            morale_boost_q16: self.morale_boost_q16.load(Ordering::Relaxed),
            fatigue_recovery_q16: self.fatigue_recovery_q16.load(Ordering::Relaxed),
            logistics_boost_q16: self.logistics_boost_q16.load(Ordering::Relaxed),
            command_delay_ticks: self.command_delay_ticks.load(Ordering::Relaxed),
            aggressive: self.flags.load(Ordering::Relaxed) & CMD_FLAG_AGGRESSIVE != 0,
        }
    }

    pub fn set_position(&self, x_q16: u32, z_q16: u32) {
        self.position_x_q16.store(x_q16, Ordering::Release);
        self.position_z_q16.store(z_q16, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

verify_capsule_properties!(CommanderCapsule, 128, 128);

#[derive(Debug, Clone, Copy)]
pub struct CommanderSnapshot {
    pub generation: u32,
    pub position_x_q16: u32,
    pub position_z_q16: u32,
    pub command_range_sq_q16: u64,
    pub morale_boost_q16: u32,
    pub fatigue_recovery_q16: u32,
    pub logistics_boost_q16: u32,
    pub command_delay_ticks: u32,
    pub aggressive: bool,
}

impl CommanderSnapshot {
    pub fn in_command_range(&self, target_x_q16: u32, target_z_q16: u32) -> bool {
        let dx = target_x_q16 as i64 - self.position_x_q16 as i64;
        let dz = target_z_q16 as i64 - self.position_z_q16 as i64;
        let dist_sq = dx.unsigned_abs().saturating_mul(dx.unsigned_abs())
            + dz.unsigned_abs().saturating_mul(dz.unsigned_abs());
        dist_sq <= self.command_range_sq_q16
    }
}

/// Deterministic mapping formation_id -> commander_id (optional; u32::MAX = unassigned).
#[repr(C, align(128))]
pub struct CommandHierarchyCapsule {
    assignments: Vec<AtomicU32>,
    _padding: [u8; 64],
}

impl CommandHierarchyCapsule {
    pub fn new(formation_count: usize) -> Self {
        let mut assignments = Vec::with_capacity(formation_count);
        for _ in 0..formation_count {
            assignments.push(AtomicU32::new(u32::MAX));
        }
        Self {
            assignments,
            _padding: [0; 64],
        }
    }

    pub fn assign_commander(&self, formation_id: usize, commander_id: u32) {
        if let Some(slot) = self.assignments.get(formation_id) {
            slot.store(commander_id, Ordering::Release);
        }
    }

    pub fn commander_for(&self, formation_id: usize) -> Option<u32> {
        self.assignments
            .get(formation_id)
            .map(|a| a.load(Ordering::Acquire))
            .and_then(|id| if id == u32::MAX { None } else { Some(id) })
    }
}

verify_capsule_properties!(CommandHierarchyCapsule, 128, 128);

/// Convert commander snapshots into General snapshots for tick-time aura reuse.
pub fn commanders_to_generals(snapshots: &[CommanderSnapshot]) -> Vec<GeneralSnapshot> {
    snapshots
        .iter()
        .map(|c| GeneralSnapshot {
            generation: c.generation,
            position_x_q16: c.position_x_q16,
            position_z_q16: c.position_z_q16,
            aura_radius_sq_q16: c.command_range_sq_q16,
            morale_boost_q16: c.morale_boost_q16,
            fatigue_recovery_q16: c.fatigue_recovery_q16,
            chariot: false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commanders_map_into_general_snapshots() {
        let cmd = CommanderCapsule::new(0, 0, 10_000, 2_000, 1_000, 500, 4, true);
        let snaps = [cmd.snapshot()];
        let generals = commanders_to_generals(&snaps);
        assert_eq!(generals.len(), 1);
        assert!(generals[0].aura_radius_sq_q16 > 0);
    }

    #[test]
    fn hierarchy_assignments_roundtrip() {
        let hierarchy = CommandHierarchyCapsule::new(3);
        hierarchy.assign_commander(1, 7);
        assert_eq!(hierarchy.commander_for(0), None);
        assert_eq!(hierarchy.commander_for(1), Some(7));
    }
}
