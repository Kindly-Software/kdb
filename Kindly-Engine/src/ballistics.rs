use crate::formation::FormationSnapshot;
use crate::order::{unpack_fire_meta_extended, unpack_fire_payload, OrderData, OrderKind};
use crate::replay::ReplayLogCapsule;
use crate::structure::{find_structure_hit, StructureCapsule};
use crate::terrain::TerrainGridCapsule;
use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU64, Ordering};

const Q16_SCALE: f64 = 65536.0;
const GRAVITY: f64 = 9.80665;
const PHYSICS_PERMEABLE: u16 = 1 << 0;

#[inline]
fn q16_distance_mm(a: (u32, u32), b: (u32, u32)) -> u32 {
    let dx = a.0 as i64 - b.0 as i64;
    let dz = a.1 as i64 - b.1 as i64;
    let dist_tiles = ((dx * dx + dz * dz) as f64).sqrt();
    (dist_tiles * 1000.0).round() as u32
}

/// Ballistics profile capsule for muskets/artillery.
///
/// - Alignment: 64B, size 64B.
/// - Stores Q16.16 for deterministic math.
#[repr(C, align(64))]
pub struct BallisticsCapsule {
    muzzle_vel_q16: AtomicU64,   // m/s Q16.16
    angle_deg_q16: AtomicU64,    // degrees Q16.16
    wind_speed_q16: AtomicU64,   // m/s Q16.16
    wind_dir_deg_q16: AtomicU64, // degrees Q16.16
    seed: AtomicU64,             // RNG seed per battery
    last_update_tick: AtomicU64, // world tick of last update
    _padding: [u8; 16],
}

impl BallisticsCapsule {
    pub fn new(
        muzzle_vel_q16: u32,
        angle_deg_q16: u32,
        wind_speed_q16: u32,
        wind_dir_deg_q16: u32,
        seed: u64,
        last_update_tick: u64,
    ) -> Self {
        Self {
            muzzle_vel_q16: AtomicU64::new(muzzle_vel_q16 as u64),
            angle_deg_q16: AtomicU64::new(angle_deg_q16 as u64),
            wind_speed_q16: AtomicU64::new(wind_speed_q16 as u64),
            wind_dir_deg_q16: AtomicU64::new(wind_dir_deg_q16 as u64),
            seed: AtomicU64::new(seed),
            last_update_tick: AtomicU64::new(last_update_tick),
            _padding: [0; 16],
        }
    }

    pub fn snapshot(&self) -> BallisticsSnapshot {
        BallisticsSnapshot {
            muzzle_vel_q16: self.muzzle_vel_q16.load(Ordering::Relaxed) as u32,
            angle_deg_q16: self.angle_deg_q16.load(Ordering::Relaxed) as u32,
            wind_speed_q16: self.wind_speed_q16.load(Ordering::Relaxed) as u32,
            wind_dir_deg_q16: self.wind_dir_deg_q16.load(Ordering::Relaxed) as u32,
            seed: self.seed.load(Ordering::Relaxed),
            last_update_tick: self.last_update_tick.load(Ordering::Relaxed),
        }
    }

    pub fn update_profile(
        &self,
        muzzle_vel_q16: u32,
        angle_deg_q16: u32,
        wind_speed_q16: u32,
        wind_dir_deg_q16: u32,
        tick: u64,
    ) {
        self.muzzle_vel_q16
            .store(muzzle_vel_q16 as u64, Ordering::Release);
        self.angle_deg_q16
            .store(angle_deg_q16 as u64, Ordering::Release);
        self.wind_speed_q16
            .store(wind_speed_q16 as u64, Ordering::Release);
        self.wind_dir_deg_q16
            .store(wind_dir_deg_q16 as u64, Ordering::Release);
        self.last_update_tick.store(tick, Ordering::Release);
    }

    /// Estimate flat-ground range in meters (simple projectile model).
    pub fn estimate_range_m(&self) -> f64 {
        let snap = self.snapshot();
        let v = q16_to_f64(snap.muzzle_vel_q16);
        let theta = deg_to_rad(q16_to_f64(snap.angle_deg_q16));
        let wind = q16_to_f64(snap.wind_speed_q16);

        // Base ballistic range without drag.
        let base = (v * v * (2.0 * theta).sin()) / GRAVITY;
        // Crude wind adjustment: +/-5% per 5 m/s tail/head wind.
        let wind_adjust = 1.0 + (wind / 5.0) * 0.05;
        base * wind_adjust
    }

    /// Terrain-aware effective range estimate with simple LOS/cover scaling.
    pub fn estimate_effective_range_with_terrain(
        &self,
        grid: &TerrainGridCapsule,
        start: (u32, u32),
        end: (u32, u32),
        structures: Option<&[StructureCapsule]>,
    ) -> BallisticsOutcome {
        let base = self.estimate_range_m();
        let (los_clear, avg_cover_q16) = if let Some(structs) = structures {
            let snaps: Vec<_> = structs.iter().map(|s| s.snapshot()).collect();
            grid.los_clear_with_structures(start, end, &snaps)
        } else {
            grid.los_clear(start, end)
        };

        let cover_scale = 1.0 - (avg_cover_q16 as f64 / Q16_SCALE) * 0.3;
        let mut effective = base * cover_scale.clamp(0.5, 1.0);
        if !los_clear {
            effective *= 0.5;
        }

        BallisticsOutcome {
            base_range_m: base,
            effective_range_m: effective,
            los_clear,
            avg_cover_q16,
            expected_casualties: 0,
            ricochet: RicochetOutcome::default(),
            crater: None,
        }
    }
}

verify_capsule_properties!(BallisticsCapsule, 64, 64);

#[derive(Debug, Clone, Copy)]
pub struct BallisticsSnapshot {
    pub muzzle_vel_q16: u32,
    pub angle_deg_q16: u32,
    pub wind_speed_q16: u32,
    pub wind_dir_deg_q16: u32,
    pub seed: u64,
    pub last_update_tick: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct BallisticsOutcome {
    pub base_range_m: f64,
    pub effective_range_m: f64,
    pub los_clear: bool,
    pub avg_cover_q16: u32,
    pub expected_casualties: u32,
    pub ricochet: RicochetOutcome,
    pub crater: Option<TerrainCrater>,
}

/// Optional physics context for fire control (target/shooter physics snapshots).
#[derive(Debug, Clone, Copy, Default)]
pub struct FireControlPhysicsContext<'a> {
    pub target: Option<&'a FormationSnapshot>,
    pub shooter: Option<&'a FormationSnapshot>,
    pub target_aperture: Option<ApertureMask>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ApertureMask {
    pub aperture_deg_q16: u32,
    pub aperture_width_q16: u32,
    pub target_x_q16: u32,
    pub target_z_q16: u32,
}

/// Ricochet outcome for artillery skipping shots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RicochetOutcome {
    pub bounces: u32,
    pub retained_energy_q16: u32,
    pub expected_casualties: u32,
}

impl Default for RicochetOutcome {
    fn default() -> Self {
        Self {
            bounces: 0,
            retained_energy_q16: 65_536,
            expected_casualties: 0,
        }
    }
}

/// Deterministic crater request for terrain deformation at the impact site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainCrater {
    pub center: (u32, u32),
    pub radius_tiles: u32,
    pub cover_delta_q16: i32,
    pub mud_delta_q16: i32,
}

/// Standardized artillery calibers/presets.
pub enum CaliberPreset {
    SixPounder,
    TwelvePounder,
    Howitzer,
}

impl CaliberPreset {
    pub fn dispersion_table(self) -> [u16; 4] {
        match self {
            CaliberPreset::SixPounder => [22, 50, 95, 150],
            CaliberPreset::TwelvePounder => [18, 40, 80, 140],
            CaliberPreset::Howitzer => [28, 65, 120, 190],
        }
    }

    pub fn profile(self) -> FireControlProfileCapsule {
        match self {
            CaliberPreset::SixPounder => FireControlProfileCapsule::new(
                self.dispersion_table(),
                [450, 900, 1500, 2000],
                3 << 16,
                70_000,
                55_000,
                55,
                50_000, // ~0.76× casualty scale vs baseline
            ),
            CaliberPreset::TwelvePounder => FireControlProfileCapsule::new(
                self.dispersion_table(),
                [400, 800, 1300, 1800],
                6 << 16,
                76_000,
                60_000,
                40,      // tighter baseline dispersion
                130_000, // heavier shell → higher casualty scale
            ),
            CaliberPreset::Howitzer => FireControlProfileCapsule::new(
                self.dispersion_table(),
                [520, 1100, 1700, 2300],
                5 << 16,
                78_000,
                52_000,
                70,
                78_000, // airburst-friendly mid-mass scale
            ),
        }
    }
}

/// Fire-control profile capsule with deterministic dispersion/fuse/damage tables.
///
/// - Alignment: 64B, size 64B.
#[repr(C, align(64))]
pub struct FireControlProfileCapsule {
    dispersion_table_mils: [u16; 4], // indexed by quality tier
    fuse_reliability_ms: [u16; 4],   // expected fuse error budget per tier
    frag_per_volley_q16: u32,        // base casualties per volley in Q16.16
    airburst_bonus_q16: u32,         // multiplier bonus for airburst in Q16.16
    impact_penalty_q16: u32,         // penalty for impact in Q16.16
    baseline_dispersion_mils: u16,   // fallback dispersion when not specified
    caliber_weight_q16: u32,         // casualty scaling based on shell mass in Q16.16
    _padding: [u8; 16],
}

impl FireControlProfileCapsule {
    pub const fn new(
        dispersion_table_mils: [u16; 4],
        fuse_reliability_ms: [u16; 4],
        frag_per_volley_q16: u32,
        airburst_bonus_q16: u32,
        impact_penalty_q16: u32,
        baseline_dispersion_mils: u16,
        caliber_weight_q16: u32,
    ) -> Self {
        Self {
            dispersion_table_mils,
            fuse_reliability_ms,
            frag_per_volley_q16,
            airburst_bonus_q16,
            impact_penalty_q16,
            baseline_dispersion_mils,
            caliber_weight_q16,
            _padding: [0; 16],
        }
    }

    /// Derive an index from dispersion mils into the table (clamped).
    fn tier_for_dispersion(&self, dispersion_mils: u16) -> usize {
        match dispersion_mils {
            0..=30 => 0,
            31..=80 => 1,
            81..=140 => 2,
            _ => 3,
        }
    }

    /// Estimate casualties deterministically using fixed-point math.
    pub fn estimate_casualties(
        &self,
        volley: u16,
        dispersion_mils: u16,
        fuse_ms: u16,
        avg_cover_q16: u32,
        los_clear: bool,
        airburst: bool,
    ) -> u32 {
        self.estimate_casualties_physics(
            volley,
            dispersion_mils,
            fuse_ms,
            avg_cover_q16,
            los_clear,
            airburst,
            None,
            None,
        )
    }

    /// Estimate casualties with optional physics scaling (target density, shooter variance).
    pub fn estimate_casualties_physics(
        &self,
        volley: u16,
        dispersion_mils: u16,
        fuse_ms: u16,
        avg_cover_q16: u32,
        los_clear: bool,
        airburst: bool,
        target_density_q16: Option<u32>,
        shooter_variance_q16: Option<u32>,
    ) -> u32 {
        let applied_dispersion = if dispersion_mils == 0 {
            self.baseline_dispersion_mils
        } else {
            dispersion_mils
        };
        let tier = self.tier_for_dispersion(applied_dispersion);
        let dispersion_penalty_q16 =
            ((self.dispersion_table_mils[tier] as u32 * 256) / 1000).min(65_536);
        let fuse_budget = self.fuse_reliability_ms[tier] as u32;
        let fuse_penalty_q16 = if fuse_ms > fuse_budget as u16 {
            // Over budget fuse time reduces effectiveness.
            32_768u32.saturating_sub(((fuse_ms as u32 - fuse_budget) * 256).min(20_000))
        } else {
            65_536
        };
        let cover_penalty_q16 = 65_536u32.saturating_sub((avg_cover_q16 / 2).min(40_000));
        let los_penalty_q16 = if los_clear { 65_536 } else { 32_768 };
        let base_casualties_q16 = (self.frag_per_volley_q16.saturating_mul(volley as u32)) as u64;

        let mut casualty_q16 = base_casualties_q16;
        casualty_q16 = casualty_q16
            .saturating_mul((65_536u32.saturating_sub(dispersion_penalty_q16)) as u64)
            / 65_536;
        casualty_q16 = casualty_q16.saturating_mul(fuse_penalty_q16 as u64) / 65_536;
        casualty_q16 = casualty_q16.saturating_mul(cover_penalty_q16 as u64) / 65_536;
        casualty_q16 = casualty_q16.saturating_mul(los_penalty_q16 as u64) / 65_536;

        let burst_scale = if airburst {
            self.airburst_bonus_q16
        } else {
            self.impact_penalty_q16
        };
        casualty_q16 = casualty_q16.saturating_mul(burst_scale as u64) / 65_536;
        casualty_q16 = casualty_q16.saturating_mul(self.caliber_weight_q16 as u64) / 65_536;
        let mut casualties = ((casualty_q16 / 65_536).min(u32::MAX as u64)) as u32;
        if let Some(density_q16) = target_density_q16 {
            let scale = density_casualty_scale(density_q16);
            casualties = ((casualties as u64 * scale as u64) / 65_536) as u32;
        }
        if let Some(var_q16) = shooter_variance_q16 {
            let scale = variance_accuracy_scale(var_q16);
            casualties = ((casualties as u64 * scale as u64) / 65_536) as u32;
        }
        casualties
    }
}

impl Default for FireControlProfileCapsule {
    fn default() -> Self {
        FireControlProfileCapsule::new(
            [25, 60, 110, 180],
            [500, 1000, 1600, 2200],
            4 << 16,
            73_728,
            57_344,
            60,
            65_536,
        )
    }
}

verify_capsule_properties!(FireControlProfileCapsule, 64, 64);

/// Density scaling: high density (squares/Old Guard) → more hits; gas (skirmishers) → fewer.
fn density_casualty_scale(density_q16: u32) -> u32 {
    let d = density_q16.min(65_536) as i64;
    // Map 0..65k to 0.2..1.3
    let scaled = 13_000 + (d * 11_000 / 65_536);
    scaled as u32
}

/// Variance scaling: lower variance (sharpshooters) → higher accuracy multiplier.
fn variance_accuracy_scale(variance_q16: u32) -> u32 {
    let v = variance_q16.min(65_536) as i64;
    // Map 0..65k to 1.2..0.8 (lower variance = tighter grouping)
    let scaled = 80_000 + ((65_536 - v) * 40_000 / 65_536);
    scaled as u32
}

/// Shooter variance widens effective dispersion slightly; lower variance keeps dispersion tight.
fn variance_dispersion_penalty(variance_q16: u32) -> f64 {
    let v = variance_q16.min(65_536) as f64 / 65_536.0;
    // Map 0..1 to 0.95..1.1 (small boost for very accurate shooters, penalty for noisy ones).
    0.95 + (v * 0.15)
}

/// Encode a fire-control replay payload: upper 32 bits casualties, lower 16 volley, lower 16 reserved.
pub fn encode_fire_replay_payload(volley: u16, expected_casualties: u32) -> u64 {
    ((expected_casualties as u64) << 32) | ((volley as u64) << 16)
}

/// Extended replay payload that also captures ricochet bounces in the lowest 8 bits (reserved slice).
pub fn encode_fire_replay_payload_with_ricochet(
    volley: u16,
    expected_casualties: u32,
    ricochet_bounces: u8,
) -> u64 {
    encode_fire_replay_payload(volley, expected_casualties) | ricochet_bounces as u64
}

/// Model per-volley fidelity: misfires, powder variance, and smoke occlusion that widens dispersion.
fn apply_volley_fidelity(volley: u16, seed: u64) -> (u16, u32, u16, u16) {
    if volley == 0 {
        return (0, 65_536, 0, 0);
    }
    let base_seed = mix64(seed ^ 0xB16B_00B5);
    let misfire_seed = base_seed;
    let powder_seed = mix64(base_seed ^ 0xA5A5_5A5A);
    let smoke_seed = mix64(base_seed ^ 0x5A5A_A5A5);

    // Misfire rate: ~2–5% depending on seed.
    let misfire_rate_q16 = 1_300u32 + (misfire_seed as u32 & 0x3FF); // 0.02..0.036
    let misfires = (((volley as u32 * misfire_rate_q16) / 65_536) as u16).min(volley);
    let effective_volley = volley.saturating_sub(misfires);

    // Powder variance: ±8% effect on muzzle energy/casualties.
    let powder_jitter = (powder_seed & 0x7FFF) as i32; // 0..32767
    let powder_offset = ((powder_jitter - 16_384) * 5_200 / 16_384).clamp(-7_000, 7_000);
    let powder_scale_q16 = (65_536i32.saturating_add(powder_offset)).clamp(55_000, 75_000) as u32;

    // Smoke occlusion: widen dispersion by ~2 mils per 8 barrels plus small jitter.
    let smoke_steps = ((effective_volley as u32 + 7) / 8) as u16;
    let smoke_jitter = (smoke_seed as u16 & 0x0003) as u16; // 0..3 mils
    let smoke_penalty_mils = smoke_steps.saturating_mul(2).saturating_add(smoke_jitter);

    (
        effective_volley,
        powder_scale_q16,
        smoke_penalty_mils,
        misfires,
    )
}

fn crater_from_volley(target_tile: (u32, u32), volley: u16) -> Option<TerrainCrater> {
    let radius_tiles = (volley as u32 / 12).clamp(1, 3);
    let cover_delta_q16 = -2_000 * radius_tiles as i32;
    let mud_delta_q16 = 3_000 * radius_tiles as i32;
    Some(TerrainCrater {
        center: target_tile,
        radius_tiles,
        cover_delta_q16,
        mud_delta_q16,
    })
}

#[inline(always)]
fn aperture_allows(
    aperture_deg_q16: u32,
    aperture_width_q16: u32,
    shooter_x_q16: u32,
    shooter_z_q16: u32,
    target_x_q16: u32,
    target_z_q16: u32,
) -> bool {
    if aperture_width_q16 == 0 {
        return true;
    }
    let dx = shooter_x_q16 as i64 - target_x_q16 as i64;
    let dz = shooter_z_q16 as i64 - target_z_q16 as i64;
    if dx == 0 && dz == 0 {
        return true;
    }
    let angle = (dz as f64).atan2(dx as f64).to_degrees();
    let aperture_deg = aperture_deg_q16 as f64 / Q16_SCALE;
    let mut delta = angle - aperture_deg;
    while delta > 180.0 {
        delta -= 360.0;
    }
    while delta < -180.0 {
        delta += 360.0;
    }
    let width = aperture_width_q16 as f64 / Q16_SCALE;
    delta.abs() <= width
}

fn apply_fire_control_inner(
    order: &OrderData,
    shooter_tile: (u32, u32),
    grid: &TerrainGridCapsule,
    ballistics: &BallisticsCapsule,
    structures: Option<&[StructureCapsule]>,
    siege: Option<&crate::siege::SiegeCapsule>,
    telemetry: &crate::telemetry::TelemetryCapsule,
    profile: &FireControlProfileCapsule,
    physics: FireControlPhysicsContext<'_>,
) -> Option<(BallisticsOutcome, u16)> {
    if order.kind != OrderKind::ArtilleryFire && order.kind != OrderKind::FireControl {
        return None;
    }
    let (target_x_q16, target_z_q16) = unpack_fire_payload(order.payload_a);
    let (volley, fuse_ms, dispersion_mils, airburst) = unpack_fire_meta_extended(order.payload_b);
    telemetry.log_fire_control(volley, dispersion_mils);
    telemetry.log_event();

    let target_tile = (target_x_q16 >> 16, target_z_q16 >> 16);
    let shooter_x_q16 = (shooter_tile.0 << 16).saturating_add(32_768);
    let shooter_z_q16 = (shooter_tile.1 << 16).saturating_add(32_768);
    let rng_seed = seed_fire(
        order,
        shooter_tile,
        target_tile,
        dispersion_mils,
        fuse_ms,
        airburst,
    );
    let (effective_volley, powder_scale_q16, smoke_penalty_mils, _misfires) =
        apply_volley_fidelity(volley, rng_seed);
    let applied_dispersion = dispersion_mils.saturating_add(smoke_penalty_mils);
    let outcome = ballistics.estimate_effective_range_with_terrain(
        grid,
        shooter_tile,
        target_tile,
        structures,
    );
    let mut los_clear = outcome.los_clear;
    let mut effective_cover_q16 = outcome.avg_cover_q16;
    let mut structure_hit: Option<(&StructureCapsule, usize)> = None;
    if let (Some(structs), Some(target_snap)) = (structures, physics.target) {
        if let Some((hit, face_cover, face_idx)) = find_structure_hit(
            structs,
            target_snap.position_x_q16,
            target_snap.position_z_q16,
            shooter_x_q16,
            shooter_z_q16,
        ) {
            effective_cover_q16 = effective_cover_q16.saturating_add(face_cover).min(65_536);
            structure_hit = Some((hit, face_idx));
        }
    }
    if let Some(ap) = physics.target_aperture {
        if !aperture_allows(
            ap.aperture_deg_q16,
            ap.aperture_width_q16,
            shooter_x_q16,
            shooter_z_q16,
            ap.target_x_q16,
            ap.target_z_q16,
        ) {
            los_clear = false;
            // Treat the wall as heavy cover if the shot is outside the aperture cone.
            effective_cover_q16 = effective_cover_q16.saturating_add(28_000).min(65_536);
        }
    }
    let shooter_dispersion = physics
        .shooter
        .map(|s| variance_dispersion_penalty(s.variance_q16))
        .unwrap_or(1.0);
    let dispersion_scale =
        (1.0 - (applied_dispersion as f64 / 800.0)).clamp(0.7, 1.0) * shooter_dispersion;
    let fuse_scale = if fuse_ms > 0 {
        // Longer fuses slightly reduce effective range; short fuses keep range high.
        (1.0 - (fuse_ms as f64 / 4000.0)).clamp(0.8, 1.0)
    } else {
        1.0
    };
    let airburst_bonus = if airburst { 1.05 } else { 1.0 };
    // TODO: apply crater when mutable terrain access is available in the tick path.
    // Ricochet/skip estimation: shallow slopes + long travel increase bounces.
    let range_mm = q16_distance_mm(shooter_tile, target_tile);
    let slope_q16 = grid
        .get_tile(target_tile.0, target_tile.1)
        .map(|t| t.snapshot().slope_q16)
        .unwrap_or(0);
    let density_q16 = physics.target.map(|t| t.density_q16).unwrap_or(32_768);
    let target_permeable = physics
        .target
        .map(|t| t.physics_flags & PHYSICS_PERMEABLE != 0)
        .unwrap_or(false);
    let ricochet =
        compute_ricochet_outcome(range_mm, slope_q16, density_q16, effective_volley as u32);
    let crater = crater_from_volley(target_tile, effective_volley);

    let target_density = physics.target.map(|t| t.density_q16);
    let shooter_variance = physics.shooter.map(|s| s.variance_q16);
    let mut expected_casualties_pre = profile.estimate_casualties_physics(
        effective_volley,
        applied_dispersion,
        fuse_ms,
        effective_cover_q16,
        los_clear,
        airburst,
        target_density,
        shooter_variance,
    );
    if target_permeable {
        expected_casualties_pre = ((expected_casualties_pre as u64 * 35_000) / 65_536) as u32;
    }
    let expected_casualties =
        ((expected_casualties_pre as u64 * powder_scale_q16 as u64) / 65_536) as u32;
    let mut ricochet_scaled =
        ((ricochet.expected_casualties as u64 * powder_scale_q16 as u64) / 65_536) as u32;
    if target_permeable {
        ricochet_scaled = ((ricochet_scaled as u64 * 35_000) / 65_536) as u32;
    }
    let casualties_base = resolve_discrete_hits(effective_volley, expected_casualties, rng_seed);
    let casualties = casualties_base.saturating_add(ricochet_scaled);
    let crater_to_apply = crater;
    if let Some((hit, face_idx)) = structure_hit {
        if let Some(siege_capsule) = siege {
            let _ = siege_capsule.apply_artillery_hit(
                hit,
                face_idx,
                effective_volley,
                expected_casualties,
            );
        }
        if crater_to_apply.is_some() || casualties > 0 {
            let mask = 1u32 << face_idx;
            hit.apply_breach(mask, 48_000);
        }
    }
    if casualties > 0 {
        // Feed casualties into telemetry so morale decay/shock logic can read it.
        telemetry.add_casualties(casualties);
        telemetry.log_artillery_shock(casualties, effective_volley);
    }
    let powder_scale_f = powder_scale_q16 as f64 / 65_536.0;

    Some((
        BallisticsOutcome {
            base_range_m: outcome.base_range_m * powder_scale_f,
            effective_range_m: outcome.effective_range_m
                * dispersion_scale
                * fuse_scale
                * airburst_bonus
                * powder_scale_f,
            los_clear,
            avg_cover_q16: effective_cover_q16,
            expected_casualties: casualties,
            ricochet,
            crater: crater_to_apply,
        },
        volley,
    ))
}

/// Apply an artillery fire control order: returns outcome and logs telemetry.
pub fn apply_fire_control(
    order: &OrderData,
    shooter_tile: (u32, u32),
    grid: &TerrainGridCapsule,
    ballistics: &BallisticsCapsule,
    structures: Option<&[StructureCapsule]>,
    siege: Option<&crate::siege::SiegeCapsule>,
    telemetry: &crate::telemetry::TelemetryCapsule,
    profile: &FireControlProfileCapsule,
) -> Option<BallisticsOutcome> {
    apply_fire_control_inner(
        order,
        shooter_tile,
        grid,
        ballistics,
        structures,
        siege,
        telemetry,
        profile,
        FireControlPhysicsContext::default(),
    )
    .map(|(outcome, _)| outcome)
}

/// Apply fire control with explicit target/shooter physics context (density/variance scaling).
pub fn apply_fire_control_with_context(
    order: &OrderData,
    shooter_tile: (u32, u32),
    grid: &TerrainGridCapsule,
    ballistics: &BallisticsCapsule,
    structures: Option<&[StructureCapsule]>,
    siege: Option<&crate::siege::SiegeCapsule>,
    telemetry: &crate::telemetry::TelemetryCapsule,
    profile: &FireControlProfileCapsule,
    physics: FireControlPhysicsContext<'_>,
) -> Option<BallisticsOutcome> {
    apply_fire_control_inner(
        order,
        shooter_tile,
        grid,
        ballistics,
        structures,
        siege,
        telemetry,
        profile,
        physics,
    )
    .map(|(outcome, _)| outcome)
}

/// Fire control using full formation snapshots (shooter + target) to supply physics context.
pub fn apply_fire_control_for_formations(
    order: &OrderData,
    shooter_tile: (u32, u32),
    grid: &TerrainGridCapsule,
    ballistics: &BallisticsCapsule,
    structures: Option<&[StructureCapsule]>,
    siege: Option<&crate::siege::SiegeCapsule>,
    telemetry: &crate::telemetry::TelemetryCapsule,
    profile: &FireControlProfileCapsule,
    target: Option<&FormationSnapshot>,
    shooter: Option<&FormationSnapshot>,
    target_aperture: Option<ApertureMask>,
) -> Option<BallisticsOutcome> {
    apply_fire_control_inner(
        order,
        shooter_tile,
        grid,
        ballistics,
        structures,
        siege,
        telemetry,
        profile,
        FireControlPhysicsContext {
            target,
            shooter,
            target_aperture,
        },
    )
    .map(|(outcome, _)| outcome)
}

/// Compute deterministic ricochet/skip outcome for artillery at shallow impact angles.
///
/// - `range_mm`: impact travel distance (mm).
/// - `slope_q16`: ground slope at impact (Q16.16).
/// - `density_q16`: target density used to scale casualties (Q16.16).
/// - `volley`: volley size (rounds).
pub fn compute_ricochet_outcome(
    range_mm: u32,
    slope_q16: u32,
    density_q16: u32,
    volley: u32,
) -> RicochetOutcome {
    let angle_factor_q16 = (slope_q16 / 8).min(65_536); // gentler slopes → more skips
    let travel_factor_q16 = ((range_mm as u64 / 50).min(65_536) as u32).max(8_192); // longer travel increases skip chance
    let bounces = ((angle_factor_q16 as u64 * travel_factor_q16 as u64) / 65_536)
        .min(3)
        .max(1) as u32;
    // Energy decays per bounce; keep deterministic Q16.16.
    let decay_q16 = 40_000; // ~0.61 per bounce
    let mut energy_q16 = 65_536;
    for _ in 0..bounces {
        energy_q16 = ((energy_q16 as u64 * decay_q16 as u64) / 65_536).min(u32::MAX as u64) as u32;
    }
    let density_scale_q16 = (density_q16 as u64 / 2 + 32_768).min(98_304) as u32; // denser targets take more hits
    let casualties = ((volley as u64 * energy_q16 as u64 * density_scale_q16 as u64)
        / (65_536 * 65_536))
        .min(u32::MAX as u64) as u32;
    RicochetOutcome {
        bounces,
        retained_energy_q16: energy_q16,
        expected_casualties: casualties,
    }
}

/// Convenience helper when the caller has formation indices (shooter/target) available.
pub fn apply_fire_control_for_ids(
    order: &OrderData,
    grid: &TerrainGridCapsule,
    ballistics: &BallisticsCapsule,
    structures: Option<&[StructureCapsule]>,
    siege: Option<&crate::siege::SiegeCapsule>,
    telemetry: &crate::telemetry::TelemetryCapsule,
    profile: &FireControlProfileCapsule,
    formations: &[crate::formation::FormationCapsule],
    shooter_id: usize,
    target_id: Option<usize>,
) -> Option<BallisticsOutcome> {
    let shooter_snap = formations.get(shooter_id)?.snapshot();
    let target_snap = target_id
        .and_then(|id| formations.get(id))
        .map(|f| f.snapshot());
    let shooter_tile = (
        shooter_snap.position_x_q16 >> 16,
        shooter_snap.position_z_q16 >> 16,
    );
    apply_fire_control_for_formations(
        order,
        shooter_tile,
        grid,
        ballistics,
        structures,
        siege,
        telemetry,
        profile,
        target_snap.as_ref(),
        Some(&shooter_snap),
        None,
    )
}

/// Fire-control helper that also records the volley/casualty payload into a replay log.
pub fn apply_fire_control_with_replay<const N: usize>(
    tick: u64,
    order: &OrderData,
    shooter_tile: (u32, u32),
    grid: &TerrainGridCapsule,
    ballistics: &BallisticsCapsule,
    structures: Option<&[StructureCapsule]>,
    siege: Option<&crate::siege::SiegeCapsule>,
    telemetry: &crate::telemetry::TelemetryCapsule,
    profile: &FireControlProfileCapsule,
    replay_log: &ReplayLogCapsule<N>,
) -> Option<BallisticsOutcome> {
    let (outcome, volley) = apply_fire_control_inner(
        order,
        shooter_tile,
        grid,
        ballistics,
        structures,
        siege,
        telemetry,
        profile,
        FireControlPhysicsContext::default(),
    )?;
    let payload = encode_fire_replay_payload_with_ricochet(
        volley,
        outcome.expected_casualties,
        outcome.ricochet.bounces as u8,
    );
    let _ = replay_log.record(tick, payload);
    Some(outcome)
}

/// Fire control + replay logging with physics context supplied from formation snapshots.
pub fn apply_fire_control_with_replay_for_formations<const N: usize>(
    tick: u64,
    order: &OrderData,
    shooter_tile: (u32, u32),
    grid: &TerrainGridCapsule,
    ballistics: &BallisticsCapsule,
    structures: Option<&[StructureCapsule]>,
    siege: Option<&crate::siege::SiegeCapsule>,
    telemetry: &crate::telemetry::TelemetryCapsule,
    profile: &FireControlProfileCapsule,
    replay_log: &ReplayLogCapsule<N>,
    target: Option<&FormationSnapshot>,
    shooter: Option<&FormationSnapshot>,
) -> Option<BallisticsOutcome> {
    let (outcome, volley) = apply_fire_control_inner(
        order,
        shooter_tile,
        grid,
        ballistics,
        structures,
        siege,
        telemetry,
        profile,
        FireControlPhysicsContext {
            target,
            shooter,
            target_aperture: None,
        },
    )?;
    let payload = encode_fire_replay_payload_with_ricochet(
        volley,
        outcome.expected_casualties,
        outcome.ricochet.bounces as u8,
    );
    let _ = replay_log.record(tick, payload);
    Some(outcome)
}

/// Fire-control + replay logging with explicit physics context for casualty scaling.
pub fn apply_fire_control_with_replay_and_context<const N: usize>(
    tick: u64,
    order: &OrderData,
    shooter_tile: (u32, u32),
    grid: &TerrainGridCapsule,
    ballistics: &BallisticsCapsule,
    structures: Option<&[StructureCapsule]>,
    siege: Option<&crate::siege::SiegeCapsule>,
    telemetry: &crate::telemetry::TelemetryCapsule,
    profile: &FireControlProfileCapsule,
    replay_log: &ReplayLogCapsule<N>,
    physics: FireControlPhysicsContext<'_>,
) -> Option<BallisticsOutcome> {
    let (outcome, volley) = apply_fire_control_inner(
        order,
        shooter_tile,
        grid,
        ballistics,
        structures,
        siege,
        telemetry,
        profile,
        physics,
    )?;
    let payload = encode_fire_replay_payload_with_ricochet(
        volley,
        outcome.expected_casualties,
        outcome.ricochet.bounces as u8,
    );
    let _ = replay_log.record(tick, payload);
    Some(outcome)
}

#[inline(always)]
fn seed_fire(
    order: &OrderData,
    shooter_tile: (u32, u32),
    target_tile: (u32, u32),
    dispersion_mils: u16,
    fuse_ms: u16,
    airburst: bool,
) -> u64 {
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    seed ^= order.formation_id as u64;
    seed = mix64(seed ^ order.payload_a);
    seed = mix64(seed ^ order.payload_b);
    seed ^= (shooter_tile.0 as u64) << 16 ^ shooter_tile.1 as u64;
    seed = mix64(seed ^ ((target_tile.0 as u64) << 16) ^ target_tile.1 as u64);
    seed ^= (dispersion_mils as u64) << 32;
    seed ^= (fuse_ms as u64) << 16;
    seed ^= if airburst { 0xA5A5_A5A5 } else { 0x5A5A_5A5A };
    mix64(seed)
}

#[inline(always)]
fn resolve_discrete_hits(volley: u16, expected_casualties: u32, seed: u64) -> u32 {
    if volley == 0 {
        return 0;
    }
    // Probability per projectile in Q16.16 using a saturating logistic: expected/(expected+volley).
    let exp = expected_casualties as u128;
    let vol = volley as u128;
    let prob_q16 = if exp == 0 {
        0
    } else {
        (((exp * 65_536u128) / (exp + vol)).min(65_536u128)) as u64
    };
    // Deterministic fractional binomial: base hits from expectation + one fractional bonus.
    let expected_times_volley = prob_q16 * volley as u64;
    let base_hits = (expected_times_volley >> 16).min(volley as u64);
    let frac = (expected_times_volley & 0xFFFF) as u32;
    let extra = if frac > 0 {
        (mix64(seed) & 0xFFFF) as u32
    } else {
        0
    };
    let bonus = if extra < frac { 1 } else { 0 };
    (base_hits as u32).saturating_add(bonus).min(volley as u32)
}

#[inline(always)]
fn mix64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[inline(always)]
fn q16_to_f64(val: u32) -> f64 {
    val as f64 / Q16_SCALE
}

#[inline(always)]
fn deg_to_rad(deg: f64) -> f64 {
    deg.to_radians()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::{pack_fire_meta_extended, pack_fire_payload};

    #[test]
    fn ballistics_range_estimate() {
        let capsule = BallisticsCapsule::new(
            (400.0 * Q16_SCALE) as u32,
            (5.0 * Q16_SCALE) as u32,
            0,
            0,
            1,
            0,
        );
        let range = capsule.estimate_range_m();
        assert!(range > 1000.0);
    }

    #[test]
    fn artillery_fire_control_uses_cover() {
        use crate::order::{pack_fire_meta, pack_fire_payload, OrderData, OrderKind};
        use crate::telemetry::TelemetryCapsule;
        use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};

        let grid = TerrainGridCapsule::new(
            2,
            2,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 0,
                material: 0,
            },
        );
        let ballistics = BallisticsCapsule::new(
            (220.0 * Q16_SCALE) as u32,
            (3.0 * Q16_SCALE) as u32,
            0,
            0,
            1,
            0,
        );
        let telemetry = TelemetryCapsule::new();
        // Low-frag profile to keep probabilities below saturation so density differences surface.
        let profile = FireControlProfileCapsule::new(
            [25, 60, 110, 180],
            [500, 1000, 1600, 2200],
            1 << 12, // frag_per_volley_q16
            65_536,  // airburst_bonus_q16
            65_536,  // impact_penalty_q16
            60,
            65_536, // caliber_weight_q16
        );

        let order = OrderData {
            kind: OrderKind::ArtilleryFire,
            formation_id: 0,
            generation: 0,
            payload_a: pack_fire_payload(1 << 16, 1 << 16),
            payload_b: pack_fire_meta(6, 0),
        };

        let outcome = apply_fire_control(
            &order,
            (0, 0),
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
        )
        .unwrap();
        assert!(outcome.effective_range_m > 500.0);
    }

    #[test]
    fn extended_fire_meta_scales_effective_range() {
        use crate::order::{pack_fire_meta_extended, pack_fire_payload, OrderData, OrderKind};
        use crate::replay::ReplayLogCapsule;
        use crate::telemetry::TelemetryCapsule;
        use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};

        let grid = TerrainGridCapsule::new(
            2,
            2,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 0,
                material: 0,
            },
        );
        let ballistics = BallisticsCapsule::new(
            (400.0 * Q16_SCALE) as u32,
            (5.0 * Q16_SCALE) as u32,
            0,
            0,
            1,
            0,
        );
        let telemetry = TelemetryCapsule::new();
        let profile = FireControlProfileCapsule::default();
        let replay: ReplayLogCapsule<8> = ReplayLogCapsule::new();

        let tight = OrderData {
            kind: OrderKind::ArtilleryFire,
            formation_id: 0,
            generation: 0,
            payload_a: pack_fire_payload(1 << 16, 1 << 16),
            payload_b: pack_fire_meta_extended(6, 0, 0, false),
        };
        let loose = OrderData {
            payload_b: pack_fire_meta_extended(6, 2500, 120, false),
            ..tight
        };

        let tight_outcome = apply_fire_control(
            &tight,
            (0, 0),
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
        )
        .unwrap();
        let loose_outcome = apply_fire_control(
            &loose,
            (0, 0),
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
        )
        .unwrap();
        assert!(tight_outcome.effective_range_m > loose_outcome.effective_range_m);
        assert!(tight_outcome.expected_casualties >= loose_outcome.expected_casualties);

        let payload = encode_fire_replay_payload(6, tight_outcome.expected_casualties);
        assert!(replay.record(1, payload));
        let drained = replay.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(
            drained[0].payload >> 32,
            tight_outcome.expected_casualties as u64
        );
    }

    #[test]
    fn physics_context_scales_casualties() {
        use crate::formation::FormationCapsule;
        use crate::order::{pack_fire_meta_extended, pack_fire_payload, OrderData, OrderKind};
        use crate::physics::PhysicsPreset;
        use crate::telemetry::TelemetryCapsule;
        use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};

        let grid = TerrainGridCapsule::new(
            2,
            2,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 0,
                material: 0,
            },
        );
        let ballistics = BallisticsCapsule::new(
            (400.0 * Q16_SCALE) as u32,
            (5.0 * Q16_SCALE) as u32,
            0,
            0,
            1,
            0,
        );
        let telemetry = TelemetryCapsule::new();
        let profile = FireControlProfileCapsule::default();

        let dense = FormationCapsule::new_with_preset(
            10,
            0,
            0,
            40_000,
            10_000,
            50_000,
            100,
            0,
            0,
            0,
            PhysicsPreset::OldGuard,
        );
        let skirm = FormationCapsule::new_with_preset(
            11,
            0,
            0,
            30_000,
            8_000,
            50_000,
            100,
            0,
            0,
            0,
            PhysicsPreset::Skirmisher,
        );
        let dense_snap = dense.snapshot();
        let skirm_snap = skirm.snapshot();

        let order = OrderData {
            kind: OrderKind::ArtilleryFire,
            formation_id: 0,
            generation: 0,
            payload_a: pack_fire_payload(1 << 16, 1 << 16),
            payload_b: pack_fire_meta_extended(24, 0, 0, false),
        };

        let dense_outcome = apply_fire_control_for_formations(
            &order,
            (0, 0),
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
            Some(&dense_snap),
            Some(&dense_snap),
            None,
        )
        .unwrap();
        let skirm_outcome = apply_fire_control_for_formations(
            &order,
            (0, 0),
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
            Some(&skirm_snap),
            Some(&skirm_snap),
            None,
        )
        .unwrap();
        assert!(dense_outcome.expected_casualties > skirm_outcome.expected_casualties);
    }

    #[test]
    fn shooter_variance_reduces_casualties() {
        use crate::formation::FormationCapsule;
        use crate::order::{pack_fire_meta_extended, pack_fire_payload, OrderData, OrderKind};
        use crate::physics::PhysicsPreset;
        use crate::telemetry::TelemetryCapsule;
        use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};

        let grid = TerrainGridCapsule::new(
            2,
            2,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 0,
                material: 0,
            },
        );
        let ballistics = BallisticsCapsule::new(
            (400.0 * Q16_SCALE) as u32,
            (5.0 * Q16_SCALE) as u32,
            0,
            0,
            1,
            0,
        );
        let telemetry = TelemetryCapsule::new();
        let profile = FireControlProfileCapsule::default();

        let accurate = FormationCapsule::new_with_preset(
            21,
            0,
            0,
            38_000,
            8_000,
            48_000,
            100,
            0,
            0,
            0,
            PhysicsPreset::OldGuard,
        );
        let inaccurate = FormationCapsule::new_with_preset(
            22,
            0,
            0,
            38_000,
            8_000,
            48_000,
            100,
            0,
            0,
            0,
            PhysicsPreset::LineInfantry,
        );
        let target = FormationCapsule::new_with_preset(
            23,
            0,
            0,
            34_000,
            7_500,
            46_000,
            100,
            0,
            0,
            0,
            PhysicsPreset::LineInfantry,
        )
        .snapshot();

        let order = OrderData {
            kind: OrderKind::ArtilleryFire,
            formation_id: 0,
            generation: 0,
            payload_a: pack_fire_payload(1 << 16, 1 << 16),
            payload_b: pack_fire_meta_extended(16, 0, 0, false),
        };

        let mut accurate_snap = accurate.snapshot();
        accurate_snap.variance_q16 = 8_000;
        let mut inaccurate_snap = inaccurate.snapshot();
        inaccurate_snap.variance_q16 = 40_000;
        let high_var = FormationSnapshot {
            variance_q16: 60_000,
            ..inaccurate_snap
        };

        let accurate_outcome = apply_fire_control_for_formations(
            &order,
            (0, 0),
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
            Some(&target),
            Some(&accurate_snap),
            None,
        )
        .unwrap();
        let inaccurate_outcome = apply_fire_control_for_formations(
            &order,
            (0, 0),
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
            Some(&target),
            Some(&high_var),
            None,
        )
        .unwrap();

        assert!(accurate_outcome.expected_casualties > inaccurate_outcome.expected_casualties);
    }

    #[test]
    fn caliber_weight_scales_casualties() {
        let six_profile = CaliberPreset::SixPounder.profile();
        let twelve_profile = CaliberPreset::TwelvePounder.profile();
        let volley = 20;
        let casualties_six = six_profile.estimate_casualties(volley, 0, 0, 0, true, false);
        let casualties_twelve = twelve_profile.estimate_casualties(volley, 0, 0, 0, true, false);
        assert!(casualties_twelve > casualties_six);
    }

    #[test]
    fn fire_control_replay_payload_is_logged() {
        use crate::order::{pack_fire_meta_extended, pack_fire_payload, OrderData, OrderKind};
        use crate::replay::ReplayLogCapsule;
        use crate::telemetry::TelemetryCapsule;
        use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};

        let grid = TerrainGridCapsule::new(
            2,
            2,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 0,
                material: 0,
            },
        );
        let ballistics = BallisticsCapsule::new(
            (400.0 * super::Q16_SCALE) as u32,
            (5.0 * super::Q16_SCALE) as u32,
            0,
            0,
            1,
            0,
        );
        let telemetry = TelemetryCapsule::new();
        let profile = FireControlProfileCapsule::default();
        let replay: ReplayLogCapsule<8> = ReplayLogCapsule::new();

        let order = OrderData {
            kind: OrderKind::ArtilleryFire,
            formation_id: 0,
            generation: 0,
            payload_a: pack_fire_payload(1 << 16, 1 << 16),
            payload_b: pack_fire_meta_extended(4, 0, 0, false),
        };

        let outcome = apply_fire_control_with_replay(
            99,
            &order,
            (0, 0),
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
            &replay,
        )
        .unwrap();
        let drained = replay.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].tick, 99);
        assert_eq!(drained[0].payload >> 32, outcome.expected_casualties as u64);
    }

    #[test]
    fn fire_control_with_context_records_replay_hits() {
        use crate::formation::FormationCapsule;
        use crate::order::{OrderData, OrderKind};
        use crate::physics::PhysicsPreset;
        use crate::telemetry::TelemetryCapsule;
        use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};

        let grid = TerrainGridCapsule::new(
            2,
            2,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 0,
                material: 0,
            },
        );
        let ballistics = BallisticsCapsule::new(
            (220.0 * super::Q16_SCALE) as u32,
            (3.0 * super::Q16_SCALE) as u32,
            0,
            0,
            1,
            0,
        );
        let telemetry = TelemetryCapsule::new();
        let profile = FireControlProfileCapsule::default();
        let replay: ReplayLogCapsule<8> = ReplayLogCapsule::new();

        let dense = FormationCapsule::new_with_preset(
            10,
            0,
            0,
            40_000,
            10_000,
            50_000,
            100,
            0,
            0,
            0,
            PhysicsPreset::OldGuard,
        )
        .snapshot();

        let order = OrderData {
            kind: OrderKind::ArtilleryFire,
            formation_id: 0,
            generation: 0,
            payload_a: pack_fire_payload(1 << 16, 1 << 16),
            payload_b: pack_fire_meta_extended(16, 0, 0, false),
        };

        let outcome = apply_fire_control_with_replay_for_formations(
            7,
            &order,
            (0, 0),
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
            &replay,
            Some(&dense),
            Some(&dense),
        )
        .unwrap();
        assert!(outcome.expected_casualties > 0);
        let drained = replay.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].payload >> 32, outcome.expected_casualties as u64);
    }

    #[test]
    fn fire_control_for_ids_uses_known_target() {
        use crate::formation::FormationCapsule;
        use crate::order::{OrderData, OrderKind};
        use crate::physics::PhysicsPreset;
        use crate::telemetry::TelemetryCapsule;
        use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};

        let grid = TerrainGridCapsule::new(
            2,
            2,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 0,
                mud_q16: 0,
                material: 0,
            },
        );
        let ballistics = BallisticsCapsule::new(
            (220.0 * super::Q16_SCALE) as u32,
            (3.0 * super::Q16_SCALE) as u32,
            0,
            0,
            1,
            0,
        );
        let telemetry = TelemetryCapsule::new();
        let profile = FireControlProfileCapsule::default();

        let dense = FormationCapsule::new_with_preset(
            0,
            0,
            0,
            40_000,
            10_000,
            50_000,
            100,
            0,
            0,
            0,
            PhysicsPreset::OldGuard,
        );
        let skirm = FormationCapsule::new_with_preset(
            1,
            0,
            0,
            30_000,
            8_000,
            50_000,
            100,
            0,
            0,
            0,
            PhysicsPreset::Skirmisher,
        );
        let formations = vec![dense, skirm];

        let order = OrderData {
            kind: OrderKind::ArtilleryFire,
            formation_id: 0,
            generation: 0,
            payload_a: pack_fire_payload(1 << 16, 1 << 16),
            payload_b: pack_fire_meta_extended(12, 0, 0, false),
        };

        let dense_target = apply_fire_control_for_ids(
            &order,
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
            &formations,
            0,
            Some(0),
        )
        .unwrap();
        let skirm_target = apply_fire_control_for_ids(
            &order,
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
            &formations,
            0,
            Some(1),
        )
        .unwrap();

        assert!(dense_target.expected_casualties > skirm_target.expected_casualties);
    }

    #[test]
    fn ricochet_energy_decays_and_scales_with_density() {
        let out_low = compute_ricochet_outcome(500, 2_000, 10_000, 4);
        let out_high = compute_ricochet_outcome(1_000, 2_000, 30_000, 4);
        assert!(out_low.retained_energy_q16 <= 65_536);
        assert!(out_high.expected_casualties >= out_low.expected_casualties);
        assert!(out_high.bounces >= 1);
    }

    #[test]
    fn artillery_replay_payload_stable_across_runs() {
        use crate::order::{pack_fire_meta_extended, pack_fire_payload, OrderData, OrderKind};
        use crate::replay::ReplayLogCapsule;
        use crate::telemetry::TelemetryCapsule;
        use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};

        let grid = TerrainGridCapsule::new(
            4,
            4,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 4_000,
                mud_q16: 2_000,
                material: 1,
            },
        );
        let ballistics = BallisticsCapsule::new(
            (600.0 * super::Q16_SCALE) as u32,
            (12.0 * super::Q16_SCALE) as u32,
            0,
            0,
            1,
            0,
        );
        let telemetry = TelemetryCapsule::new();
        let profile = FireControlProfileCapsule::default();

        let order = OrderData {
            kind: OrderKind::ArtilleryFire,
            formation_id: 42,
            generation: 7,
            payload_a: pack_fire_payload(2 << 16, 2 << 16),
            payload_b: pack_fire_meta_extended(8, 0, 0, false),
        };

        let replay_a: ReplayLogCapsule<8> = ReplayLogCapsule::new();
        let replay_b: ReplayLogCapsule<8> = ReplayLogCapsule::new();

        let outcome_a = apply_fire_control_with_replay(
            101,
            &order,
            (1, 1),
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
            &replay_a,
        )
        .unwrap();
        let outcome_b = apply_fire_control_with_replay(
            101,
            &order,
            (1, 1),
            &grid,
            &ballistics,
            None,
            None,
            &telemetry,
            &profile,
            &replay_b,
        )
        .unwrap();

        let payloads_a: Vec<_> = replay_a.drain().into_iter().map(|e| e.payload).collect();
        let payloads_b: Vec<_> = replay_b.drain().into_iter().map(|e| e.payload).collect();

        assert_eq!(outcome_a.expected_casualties, outcome_b.expected_casualties);
        assert_eq!(payloads_a, payloads_b);
    }
}
