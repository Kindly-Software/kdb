use crate::formation::gap_fatigue_penalty;
use crate::formation::FormationCapsule;
use crate::telemetry::TelemetryCapsule;
use crate::terrain::TerrainGridCapsule;
use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "simd-los")]
use core::simd::{num::SimdUint, Simd};

const CHARGE_ACTIVE: u64 = 1;
const CHARGE_COMMIT: u64 = 1 << 1;
const CHARGE_DIR_X_SHIFT: u64 = 8;
const CHARGE_DIR_Z_SHIFT: u64 = 10;

/// Pathing capsule: target waypoint + fixed-point speed.
#[repr(C, align(128))]
pub struct PathingCapsule {
    target_x_q16: AtomicU64,
    target_z_q16: AtomicU64,
    speed_q16: AtomicU64,
    slope_penalty_q16: AtomicU64,
    terrain_penalty_q16: AtomicU64,
    charge_state: AtomicU64,
    _padding: [u8; 80],
}

impl PathingCapsule {
    pub const fn new(target_x_q16: u32, target_z_q16: u32, speed_q16: u32) -> Self {
        Self {
            target_x_q16: AtomicU64::new(target_x_q16 as u64),
            target_z_q16: AtomicU64::new(target_z_q16 as u64),
            speed_q16: AtomicU64::new(speed_q16 as u64),
            slope_penalty_q16: AtomicU64::new(0),
            terrain_penalty_q16: AtomicU64::new(0),
            charge_state: AtomicU64::new(0),
            _padding: [0; 80],
        }
    }

    pub fn set_target(&self, x_q16: u32, z_q16: u32) {
        self.target_x_q16.store(x_q16 as u64, Ordering::Release);
        self.target_z_q16.store(z_q16 as u64, Ordering::Release);
    }

    /// Set a slope/terrain penalty in Q16.16 (0.0 = none, 0.5 = halve speed).
    pub fn set_slope_penalty_q16(&self, penalty_q16: u32) {
        self.slope_penalty_q16
            .store(penalty_q16 as u64, Ordering::Release);
    }

    /// Set terrain penalty derived from a cost map (Q16.16 scale).
    pub fn set_terrain_penalty_q16(&self, penalty_q16: u32) {
        self.terrain_penalty_q16
            .store(penalty_q16 as u64, Ordering::Release);
    }

    /// Move the formation toward the target using fixed-point step.
    ///
    /// Returns true if movement occurred.
    pub fn step(&self, formation: &FormationCapsule, telemetry: &TelemetryCapsule) -> bool {
        self.step_with_backstep(formation, telemetry, false)
    }

    /// Variant that supports controlled fallback (backstep = keep facing enemy, slower pace).
    pub fn step_with_backstep(
        &self,
        formation: &FormationCapsule,
        telemetry: &TelemetryCapsule,
        backstep: bool,
    ) -> bool {
        self.step_internal(formation, telemetry, backstep, None)
    }

    /// Step using terrain-driven penalties (avg mud/cost) computed from the grid.
    pub fn step_with_terrain(
        &self,
        formation: &FormationCapsule,
        telemetry: &TelemetryCapsule,
        grid: &TerrainGridCapsule,
        stride: usize,
        backstep: bool,
    ) -> bool {
        let (slope_pen_q16, terrain_pen_q16) = compute_terrain_penalties(
            grid,
            formation.snapshot().position_x_q16,
            formation.snapshot().position_z_q16,
            stride,
        );
        self.set_slope_penalty_q16(slope_pen_q16);
        self.set_terrain_penalty_q16(terrain_pen_q16);
        // Fatigue rises when trudging through mud/costly ground.
        if slope_pen_q16 > 0 || terrain_pen_q16 > 0 {
            let fatigue_bump =
                ((slope_pen_q16 as u64 + terrain_pen_q16 as u64) / 8).min(10_000) as i32;
            formation.adjust_fatigue(fatigue_bump);
        }
        self.step_internal(
            formation,
            telemetry,
            backstep,
            Some((slope_pen_q16, terrain_pen_q16)),
        )
    }

    /// Apply a charge target with commit pacing and a simple corridor lock.
    pub fn set_charge_target(
        &self,
        start_x_q16: u32,
        start_z_q16: u32,
        target_x_q16: u32,
        target_z_q16: u32,
        commit: bool,
    ) {
        self.set_target(target_x_q16, target_z_q16);
        let dx = target_x_q16 as i64 - start_x_q16 as i64;
        let dz = target_z_q16 as i64 - start_z_q16 as i64;
        let dir_x = encode_dir(dx);
        let dir_z = encode_dir(dz);
        let mut state =
            CHARGE_ACTIVE | (dir_x << CHARGE_DIR_X_SHIFT) | (dir_z << CHARGE_DIR_Z_SHIFT);
        if commit {
            state |= CHARGE_COMMIT;
        }
        self.charge_state.store(state, Ordering::Release);
    }

    pub fn clear_charge(&self) {
        self.charge_state.store(0, Ordering::Release);
    }

    fn step_internal(
        &self,
        formation: &FormationCapsule,
        telemetry: &TelemetryCapsule,
        backstep: bool,
        penalties: Option<(u32, u32)>,
    ) -> bool {
        let snap = formation.snapshot();
        let tx = self.target_x_q16.load(Ordering::Acquire) as i64;
        let tz = self.target_z_q16.load(Ordering::Acquire) as i64;
        let fx = snap.position_x_q16 as i64;
        let fz = snap.position_z_q16 as i64;

        let dx = tx - fx;
        let dz = tz - fz;
        if dx == 0 && dz == 0 {
            self.clear_charge();
            return false;
        }

        let base_speed = self.speed_q16.load(Ordering::Acquire) as i64;
        // If the formation has a physics velocity, fall back to it when pathing speed is unset.
        let phys_speed = snap.velocity_q16 as i64;
        let mut speed = if base_speed > 0 {
            base_speed
        } else if phys_speed > 0 {
            phys_speed
        } else {
            base_speed
        };
        let posture_scale_q16 = posture_speed_scale_q16(snap.posture);
        speed = ((speed * posture_scale_q16 as i64) / 65_536).max(0);
        let (slope_pen, terrain_pen) = if let Some((s, t)) = penalties {
            (s as i64, t as i64)
        } else {
            (
                self.slope_penalty_q16.load(Ordering::Acquire) as i64,
                self.terrain_penalty_q16.load(Ordering::Acquire) as i64,
            )
        };
        let charge_state = self.charge_state.load(Ordering::Relaxed);
        let charge_active = (charge_state & CHARGE_ACTIVE) != 0;
        let charge_commit = (charge_state & CHARGE_COMMIT) != 0;
        let combined_pen = (slope_pen + terrain_pen).min(60_000);
        if combined_pen > 0 {
            // Apply terrain/slope penalty: speed *= (1 - penalty)
            speed = (speed * (65_536 - combined_pen).max(0)) / 65_536;
        }
        // Marching tightly costs fatigue (tighter gaps → more fatigue).
        let gap_penalty = gap_fatigue_penalty(snap.gap_close_q16);
        if gap_penalty > 0 {
            formation.adjust_fatigue(gap_penalty as i32);
        }
        if backstep {
            // Controlled fallback: slower pace to avoid turning backs.
            speed = (speed * 3) / 4;
            if charge_active {
                self.clear_charge();
            }
        } else if charge_active {
            // Commit pacing: slight speed-up during an active charge.
            if charge_commit {
                speed = (speed * 5) / 4;
            }
        }
        let step_x = clamp_step(dx, speed);
        let step_z = clamp_step(dz, speed);

        // Corridor lock: avoid oscillating once we have crossed the charge vector.
        let mut step_x = step_x;
        let mut step_z = step_z;
        if charge_active {
            let dir_x = decode_dir((charge_state >> CHARGE_DIR_X_SHIFT) & 0x3);
            let dir_z = decode_dir((charge_state >> CHARGE_DIR_Z_SHIFT) & 0x3);
            if dir_x > 0 && (dx.signum() as i32) != dir_x {
                step_x = 0;
            }
            if dir_z > 0 && (dz.signum() as i32) != dir_z {
                step_z = 0;
            }
        }

        if snap.braced && !backstep {
            return false;
        }

        formation.set_position_q16((fx + step_x) as u32, (fz + step_z) as u32);

        if charge_active && (step_x == 0 && step_z == 0 || (fx + step_x == tx && fz + step_z == tz))
        {
            self.clear_charge();
        }

        telemetry.log_event();
        true
    }
}

verify_capsule_properties!(PathingCapsule, 128, 128);

#[inline(always)]
fn clamp_step(delta: i64, speed: i64) -> i64 {
    if delta > speed {
        speed
    } else if delta < -speed {
        -speed
    } else {
        delta
    }
}

#[inline(always)]
fn encode_dir(delta: i64) -> u64 {
    if delta > 0 {
        1
    } else if delta < 0 {
        2
    } else {
        0
    }
}

#[inline(always)]
fn decode_dir(bits: u64) -> i32 {
    match bits as i32 {
        1 => 1,
        2 => -1,
        _ => 0,
    }
}

#[inline(always)]
fn posture_speed_scale_q16(posture: u8) -> u32 {
    match posture {
        // Column march: faster road pace than line/square.
        1 => 81_920, // ~1.25x
        _ => 65_536, // 1.0x default
    }
}

/// Compute average mud/cost penalties around a tile using an 8-sample neighborhood.
fn compute_terrain_penalties(
    grid: &TerrainGridCapsule,
    pos_x_q16: u32,
    pos_z_q16: u32,
    stride: usize,
) -> (u32, u32) {
    let cx = pos_x_q16 >> 16;
    let cz = pos_z_q16 >> 16;
    let offsets: &[(i32, i32)] = &[
        (0, 0),
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (-1, 1),
        (1, -1),
    ];
    let mut mud_buf = [0u32; 8];
    let mut cost_buf = [0u32; 8];
    let mut count = 0usize;

    for (i, (ox, oz)) in offsets.iter().enumerate() {
        let tx = (cx as i32 + ox).max(0) as u32;
        let tz = (cz as i32 + oz).max(0) as u32;
        if tx >= grid.width() || tz >= grid.height() {
            continue;
        }
        let (cover, mud) = grid.sample_cover_mud(tx, tz, stride);
        let cost = grid.cost_at(tx, tz).unwrap_or(0);
        mud_buf[i] = mud + cover / 4; // cover contributes mildly to slowdown
        cost_buf[i] = cost;
        count += 1;
    }

    if count == 0 {
        return (0, 0);
    }

    #[cfg(feature = "simd-los")]
    {
        let mud_simd = Simd::from_array(mud_buf);
        let cost_simd = Simd::from_array(cost_buf);
        let mud_sum: u32 = mud_simd.reduce_sum();
        let cost_sum: u32 = cost_simd.reduce_sum();
        (
            (mud_sum / count as u32).min(50_000),
            (cost_sum / count as u32).min(50_000),
        )
    }
    #[cfg(not(feature = "simd-los"))]
    {
        let mut mud_sum: u64 = 0;
        let mut cost_sum: u64 = 0;
        for i in 0..offsets.len() {
            mud_sum += mud_buf[i] as u64;
            cost_sum += cost_buf[i] as u64;
        }
        (
            ((mud_sum / count as u64) as u32).min(50_000),
            ((cost_sum / count as u64) as u32).min(50_000),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formation::FormationCapsule;
    use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};

    #[test]
    fn pathing_moves_toward_target() {
        let formation = FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let telemetry = TelemetryCapsule::new();
        let pathing = PathingCapsule::new(100, 0, 20);
        let moved = pathing.step(&formation, &telemetry);
        assert!(moved);
        let snap = formation.snapshot();
        assert_eq!(snap.position_x_q16, 20);
    }

    #[test]
    fn slope_penalty_slows_speed() {
        let formation = FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let telemetry = TelemetryCapsule::new();
        let pathing = PathingCapsule::new(100, 0, 32);
        pathing.set_slope_penalty_q16(32_768); // 0.5 penalty
        let _ = pathing.step(&formation, &telemetry);
        let snap = formation.snapshot();
        // 32 * 0.5 = 16
        assert_eq!(snap.position_x_q16, 16);
    }

    #[test]
    fn backstep_slows_progress() {
        let formation = FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let telemetry = TelemetryCapsule::new();
        let pathing = PathingCapsule::new(80, 0, 32);
        let _ = pathing.step_with_backstep(&formation, &telemetry, true);
        let snap = formation.snapshot();
        // 3/4 speed => expected step 24 (clamp to delta 80)
        assert_eq!(snap.position_x_q16, 24);
    }

    #[test]
    fn terrain_cost_penalty_slows_speed() {
        let formation = FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let telemetry = TelemetryCapsule::new();
        let pathing = PathingCapsule::new(100, 0, 64);
        let mut grid = TerrainGridCapsule::new(
            1,
            1,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 0,
                material: 0,
            },
        );
        grid.set_tile(
            0,
            0,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 8_000,
                cover_q16: 30_000,
                mud_q16: 20_000,
                material: 0,
            },
        );
        let cost_penalty = grid.cost_at(0, 0).unwrap_or(0).min(40_000);
        pathing.set_terrain_penalty_q16(cost_penalty);
        let _ = pathing.step(&formation, &telemetry);
        let snap = formation.snapshot();
        assert!(snap.position_x_q16 < 64);
    }

    #[test]
    fn simd_penalties_feed_pathing() {
        let formation = FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let telemetry = TelemetryCapsule::new();
        let pathing = PathingCapsule::new(100, 0, 64);
        let mut grid = TerrainGridCapsule::new(
            3,
            3,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 0,
                material: 0,
            },
        );
        grid.set_tile(
            1,
            1,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 40_000,
                mud_q16: 45_000,
                material: 0,
            },
        );
        let _ = pathing.step_with_terrain(&formation, &telemetry, &grid, 1, false);
        let snap = formation.snapshot();
        // Penalties should slow us below the raw speed of 64.
        assert!(snap.position_x_q16 < 64);
        // Fatigue should increase due to trudging through mud.
        assert!(snap.fatigue_q16 > 0);
    }

    #[test]
    fn charge_targets_enable_commit_pacing() {
        let formation = FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let telemetry = TelemetryCapsule::new();
        let pathing = PathingCapsule::new(0, 0, 32);
        pathing.set_charge_target(0, 0, 64, 0, true);
        let moved = pathing.step(&formation, &telemetry);
        let snap = formation.snapshot();
        assert!(moved);
        assert!(snap.position_x_q16 >= 40); // commit pacing gives 1.25x speed (floor to 40)
        assert_eq!(snap.position_z_q16, 0);
    }

    #[test]
    fn column_posture_moves_faster() {
        let telemetry = TelemetryCapsule::new();
        let pathing = PathingCapsule::new(64, 0, 32);

        let line = FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let column = FormationCapsule::new(2, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        column.set_posture(1, 0); // column posture

        let _ = pathing.step(&line, &telemetry);
        let line_pos = line.snapshot().position_x_q16;

        let _ = pathing.step(&column, &telemetry);
        let column_pos = column.snapshot().position_x_q16;

        assert!(column_pos > line_pos);
    }
}
