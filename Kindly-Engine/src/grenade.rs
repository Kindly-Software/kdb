use crate::math::Q16_16;
use crate::structure::{find_structure_hit, StructureCapsule};
use crate::terrain::TerrainGridCapsule;
use atomic_capsule::{verify_capsule_properties, BackoffStrategy, RetryPolicy};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Capsule describing grenade physics (arc + fragment energy) with deterministic RNG.
#[repr(C, align(64))]
pub struct GrenadeCapsule {
    throw_speed_q16: AtomicU32,     // m/s Q16.16
    fuse_ms: AtomicU32,             // base fuse
    fragment_count: AtomicU32,      // number of fragments
    fragment_energy_q16: AtomicU32, // fragment lethality scale (Q16.16)
    arc_height_q16: AtomicU32,      // arc height hint (Q16.16 meters)
    seed: AtomicU64,                // deterministic jitter
    _padding: [u8; 28],
}

verify_capsule_properties!(GrenadeCapsule, 64, 64);

impl GrenadeCapsule {
    pub fn new(
        throw_speed_q16: u32,
        fuse_ms: u32,
        fragment_count: u32,
        fragment_energy_q16: u32,
        arc_height_q16: u32,
        seed: u64,
    ) -> Self {
        Self {
            throw_speed_q16: AtomicU32::new(throw_speed_q16),
            fuse_ms: AtomicU32::new(fuse_ms),
            fragment_count: AtomicU32::new(fragment_count),
            fragment_energy_q16: AtomicU32::new(fragment_energy_q16),
            arc_height_q16: AtomicU32::new(arc_height_q16),
            seed: AtomicU64::new(seed),
            _padding: [0; 28],
        }
    }

    pub fn snapshot(&self) -> GrenadeSnapshot {
        GrenadeSnapshot {
            throw_speed_q16: self.throw_speed_q16.load(Ordering::Relaxed),
            fuse_ms: self.fuse_ms.load(Ordering::Relaxed),
            fragment_count: self.fragment_count.load(Ordering::Relaxed),
            fragment_energy_q16: self.fragment_energy_q16.load(Ordering::Relaxed),
            arc_height_q16: self.arc_height_q16.load(Ordering::Relaxed),
            seed: self.seed.load(Ordering::Relaxed),
        }
    }

    pub fn configure(
        &self,
        throw_speed_q16: u32,
        fuse_ms: u32,
        fragment_count: u32,
        fragment_energy_q16: u32,
        arc_height_q16: u32,
        seed: u64,
    ) {
        self.throw_speed_q16
            .store(throw_speed_q16, Ordering::Release);
        self.fuse_ms.store(fuse_ms, Ordering::Release);
        self.fragment_count
            .store(fragment_count.max(1), Ordering::Release);
        self.fragment_energy_q16
            .store(fragment_energy_q16.max(1), Ordering::Release);
        self.arc_height_q16.store(arc_height_q16, Ordering::Release);
        self.seed.store(seed, Ordering::Release);
    }

    /// Deterministic grenade throw with terrain/structure-aware cover.
    pub fn throw(
        &self,
        start_x_q16: u32,
        start_z_q16: u32,
        target_x_q16: u32,
        target_z_q16: u32,
        terrain: &TerrainGridCapsule,
        structures: Option<&[StructureCapsule]>,
        fuse_override_ms: Option<u32>,
        fragment_override: Option<u32>,
        tick: u64,
    ) -> GrenadeOutcome {
        let snap = self.snapshot();
        let fuse_ms = fuse_override_ms.unwrap_or(snap.fuse_ms);
        let fragments = fragment_override.unwrap_or(snap.fragment_count);
        let fragment_energy_q16 = snap.fragment_energy_q16;
        let dist_mm = distance_mm_q16((start_x_q16, start_z_q16), (target_x_q16, target_z_q16));
        let speed_mm_per_s = (snap.throw_speed_q16 as u64 * 1000) / Q16_16::ONE.to_raw() as u64;
        let flight_ms = if speed_mm_per_s > 0 {
            (dist_mm as u64 * 1000 / speed_mm_per_s) as u32
        } else {
            0
        };
        let arc_penalty_ms = (snap.arc_height_q16 / 1024).min(2_000);
        let detonation_ms = flight_ms
            .saturating_add(fuse_ms)
            .saturating_add(arc_penalty_ms);

        let (los_clear, terrain_cover_q16) = if let Some(structs) = structures {
            let snaps: Vec<_> = structs.iter().map(|s| s.snapshot()).collect();
            terrain.los_clear_with_structures(
                (start_x_q16 >> 16, start_z_q16 >> 16),
                (target_x_q16 >> 16, target_z_q16 >> 16),
                &snaps,
            )
        } else {
            terrain.los_clear(
                (start_x_q16 >> 16, start_z_q16 >> 16),
                (target_x_q16 >> 16, target_z_q16 >> 16),
            )
        };

        let mut structure_cover_q16 = 0u32;
        let mut hit_struct: Option<(&StructureCapsule, usize)> = None;
        if let Some(structs) = structures {
            if let Some((s, cover, face)) = find_structure_hit(
                structs,
                target_x_q16,
                target_z_q16,
                start_x_q16,
                start_z_q16,
            ) {
                structure_cover_q16 = cover;
                hit_struct = Some((s, face));
            }
        }
        let avg_cover_q16 = terrain_cover_q16
            .saturating_add(structure_cover_q16)
            .min(65_536);

        let cover_scale = (65_536u64).saturating_sub(avg_cover_q16 as u64);
        let base_hits = (fragments as u64)
            .saturating_mul(fragment_energy_q16 as u64)
            .saturating_mul(cover_scale)
            / (65_536 * 65_536);
        let jitter = xorshift64star(snap.seed.wrapping_add(tick)) & 0x7;
        let expected_casualties =
            base_hits.saturating_add(jitter as u64).min(u32::MAX as u64) as u32;

        if let Some((s, face)) = hit_struct {
            // Breach the impacted face; grenade blasts are lighter than cannonballs.
            s.apply_breach(1u32 << face, 52_000);
        }

        GrenadeOutcome {
            impact_tile: (target_x_q16 >> 16, target_z_q16 >> 16),
            detonation_ms,
            los_clear,
            avg_cover_q16,
            expected_casualties,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GrenadeSnapshot {
    pub throw_speed_q16: u32,
    pub fuse_ms: u32,
    pub fragment_count: u32,
    pub fragment_energy_q16: u32,
    pub arc_height_q16: u32,
    pub seed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrenadeOutcome {
    pub impact_tile: (u32, u32),
    pub detonation_ms: u32,
    pub los_clear: bool,
    pub avg_cover_q16: u32,
    pub expected_casualties: u32,
}

fn distance_mm_q16(a: (u32, u32), b: (u32, u32)) -> u32 {
    let dx = a.0 as i64 - b.0 as i64;
    let dz = a.1 as i64 - b.1 as i64;
    let dx_mm = ((dx.unsigned_abs() as u64) * 1000) / Q16_16::ONE.to_raw() as u64;
    let dz_mm = ((dz.unsigned_abs() as u64) * 1000) / Q16_16::ONE.to_raw() as u64;
    dx_mm.max(dz_mm).min(u32::MAX as u64) as u32
}

#[inline(always)]
fn xorshift64star(mut x: u64) -> u64 {
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

/// Simple backoff helper for callers who want deterministic, retry-able throws.
#[inline(always)]
pub fn grenade_retry_policy() -> RetryPolicy {
    RetryPolicy::new(BackoffStrategy::STANDARD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::StructureCapsule;
    use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};

    #[test]
    fn grenade_cover_reduces_casualties() {
        let grenade = GrenadeCapsule::new(30 << 16, 500, 40, 50_000, 2 << 16, 42);
        let grid = TerrainGridCapsule::new(
            4,
            4,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 0,
                material: 0,
            },
        );
        let open = grenade.throw(
            1 << 16,
            1 << 16,
            2 << 16,
            2 << 16,
            &grid,
            None,
            None,
            None,
            1,
        );
        // Higher cover should drop the expected casualties.
        let structure = StructureCapsule::new(
            1,
            2 << 16,
            2 << 16,
            1 << 16,
            1 << 16,
            60_000,
            60_000,
            60_000,
            60_000,
            4,
            2,
        );
        let cover = grenade.throw(
            1 << 16,
            1 << 16,
            2 << 16,
            2 << 16,
            &grid,
            Some(&[structure]),
            None,
            None,
            2,
        );
        assert!(cover.expected_casualties <= open.expected_casualties);
    }
}
