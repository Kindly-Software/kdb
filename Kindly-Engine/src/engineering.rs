use crate::terrain::TerrainGridCapsule;
use crate::siege::SiegeCapsule;
use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU64, Ordering};

/// Engineering capsule for deterministic terrain edits (trenches/fortifications).
#[repr(C, align(64))]
pub struct EngineeringCapsule {
    ops: AtomicU64,
    _padding: [u8; 56],
}

impl EngineeringCapsule {
    pub const fn new() -> Self {
        Self {
            ops: AtomicU64::new(0),
            _padding: [0; 56],
        }
    }

    /// Dig a trench: reduce cover and increase mud within radius.
    pub fn dig_trench(
        &self,
        grid: &mut TerrainGridCapsule,
        center_x: u32,
        center_y: u32,
        radius_tiles: u32,
    ) -> usize {
        let updated = grid.apply_crater_q16(center_x, center_y, radius_tiles, -3_000, 4_000);
        self.ops.fetch_add(1, Ordering::AcqRel);
        updated
    }

    /// Fortify: increase cover and slightly dry mud within radius.
    pub fn fortify(
        &self,
        grid: &mut TerrainGridCapsule,
        center_x: u32,
        center_y: u32,
        radius_tiles: u32,
    ) -> usize {
        let updated = grid.apply_crater_q16(center_x, center_y, radius_tiles, 3_000, -1_000);
        self.ops.fetch_add(1, Ordering::AcqRel);
        updated
    }

    pub fn ops(&self) -> u64 {
        self.ops.load(Ordering::Acquire)
    }

    /// Apply sapper pressure to a fortification face (attrition toward breach).
    pub fn sap(
        &self,
        siege: &SiegeCapsule,
        structure_id: u32,
        face_idx: usize,
        intensity_q16: u32,
    ) -> bool {
        let ok = siege.apply_sapper_attrition(structure_id, face_idx, intensity_q16);
        if ok {
            self.ops.fetch_add(1, Ordering::AcqRel);
        }
        ok
    }

    /// Repair a breached wall face using engineering resources.
    pub fn repair(
        &self,
        siege: &SiegeCapsule,
        structure: &crate::structure::StructureCapsule,
        face_idx: usize,
        repair_q16: u32,
    ) -> bool {
        let repaired = siege
            .apply_repair(structure, face_idx, repair_q16)
            .is_some();
        if repaired {
            self.ops.fetch_add(1, Ordering::AcqRel);
        }
        repaired
    }
}

verify_capsule_properties!(EngineeringCapsule, 64, 64);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};

    #[test]
    fn trench_reduces_cover_and_increases_mud() {
        let mut grid = TerrainGridCapsule::new(
            4,
            4,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 10_000,
                mud_q16: 2_000,
                material: 0,
            },
        );
        let eng = EngineeringCapsule::new();
        let updated = eng.dig_trench(&mut grid, 1, 1, 1);
        assert!(updated > 0);
        let snap = grid.get_tile(1, 1).unwrap().snapshot();
        assert!(snap.cover_q16 < 10_000);
        assert!(snap.mud_q16 > 2_000);
    }

    #[test]
    fn fortify_increases_cover() {
        let mut grid = TerrainGridCapsule::new(
            4,
            4,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 5_000,
                mud_q16: 3_000,
                material: 0,
            },
        );
        let eng = EngineeringCapsule::new();
        let _ = eng.fortify(&mut grid, 2, 2, 1);
        let snap = grid.get_tile(2, 2).unwrap().snapshot();
        assert!(snap.cover_q16 > 5_000);
    }
}
