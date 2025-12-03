use crate::ballistics::ApertureMask;
use crate::battle_ai::MAX_AI_DECISIONS_PER_TICK;
use crate::courier::CourierCapsule;
use crate::diplomacy::DiplomaticSnapshot;
use crate::fire_doctrine::FireDoctrineCapsule;
use crate::formation::{FormationCapsule, FormationSnapshot, RetreatMode};
use crate::general::GeneralSnapshot;
use crate::command::{CommandHierarchyCapsule, CommanderSnapshot};
use crate::order::{
    pack_ai_order_payload, unpack_brace_payload, unpack_charge_meta, unpack_fire_doctrine_payload,
    unpack_fire_meta_extended, unpack_fire_payload, unpack_garrison_payload, unpack_grenade_meta,
    unpack_grenade_payload, unpack_move_payload, unpack_retreat_payload, OrderData, OrderKind,
    OrderQueueCapsule,
};
use crate::pathing::PathingCapsule;
use crate::replay::{ReplayFlushCapsule, ReplayIndexCapsule, ReplayLogCapsule, ReplayMmapCapsule};
use crate::snapshot::{CampaignSnapshotCapsule, SnapshotMmapCapsule};
use crate::strategic_map::StrategicSnapshot;
use crate::structure::StructureCapsule;
use crate::supply::SupplySnapshot;
use crate::telemetry::{FormationBreakTelemetryCapsule, TelemetryCapsule};
use crate::{DeterministicRngCapsule, WorldClockCapsule};
use atomic_capsule::mmap::MmapError;
use atomic_capsule::verify_alignment_only;
use core::sync::atomic::{AtomicU64, Ordering};

const COMMAND_HIST_BUCKETS: usize = 8;
const COMMAND_HIST_THRESHOLDS: [u32; COMMAND_HIST_BUCKETS] = [4, 8, 16, 24, 32, 48, 64, u32::MAX];

#[inline(always)]
fn command_hist_bucket(val: u32) -> usize {
    for (idx, &threshold) in COMMAND_HIST_THRESHOLDS.iter().enumerate() {
        if val <= threshold {
            return idx;
        }
    }
    COMMAND_HIST_BUCKETS - 1
}

// ---------------- Render Paged Slab ----------------

const RENDER_PAGE_SIZE: usize = 10_000;

#[derive(Clone)]
struct RenderPage {
    formation_ids: [u32; RENDER_PAGE_SIZE],
    posture: [u8; RENDER_PAGE_SIZE],
    stance: [u8; RENDER_PAGE_SIZE],
    morale_q16: [u32; RENDER_PAGE_SIZE],
    fatigue_q16: [u32; RENDER_PAGE_SIZE],
    cohesion_q16: [u32; RENDER_PAGE_SIZE],
    ammo: [u32; RENDER_PAGE_SIZE],
    facing_deg_q16: [u32; RENDER_PAGE_SIZE],
    position_x_q16: [u32; RENDER_PAGE_SIZE],
    position_z_q16: [u32; RENDER_PAGE_SIZE],
    retreat_flags: [u16; RENDER_PAGE_SIZE],
    charge_posture: [u8; RENDER_PAGE_SIZE],
    braced: [u8; RENDER_PAGE_SIZE],
    density_q16: [u32; RENDER_PAGE_SIZE],
    variance_q16: [u32; RENDER_PAGE_SIZE],
    gap_close_q16: [u32; RENDER_PAGE_SIZE],
    rank_variance_q16: [u32; RENDER_PAGE_SIZE],
    gap_fatigue_penalty_q16: [u32; RENDER_PAGE_SIZE],
}

impl RenderPage {
    fn new() -> Self {
        Self {
            formation_ids: [0; RENDER_PAGE_SIZE],
            posture: [0; RENDER_PAGE_SIZE],
            stance: [0; RENDER_PAGE_SIZE],
            morale_q16: [0; RENDER_PAGE_SIZE],
            fatigue_q16: [0; RENDER_PAGE_SIZE],
            cohesion_q16: [0; RENDER_PAGE_SIZE],
            ammo: [0; RENDER_PAGE_SIZE],
            facing_deg_q16: [0; RENDER_PAGE_SIZE],
            position_x_q16: [0; RENDER_PAGE_SIZE],
            position_z_q16: [0; RENDER_PAGE_SIZE],
            retreat_flags: [0; RENDER_PAGE_SIZE],
            charge_posture: [0; RENDER_PAGE_SIZE],
            braced: [0; RENDER_PAGE_SIZE],
            density_q16: [0; RENDER_PAGE_SIZE],
            variance_q16: [0; RENDER_PAGE_SIZE],
            gap_close_q16: [0; RENDER_PAGE_SIZE],
            rank_variance_q16: [0; RENDER_PAGE_SIZE],
            gap_fatigue_penalty_q16: [0; RENDER_PAGE_SIZE],
        }
    }
}

fn within_aperture(
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
    let dx = target_x_q16 as i64 - shooter_x_q16 as i64;
    let dz = target_z_q16 as i64 - shooter_z_q16 as i64;
    if dx == 0 && dz == 0 {
        return true;
    }
    let angle = (dz as f64).atan2(dx as f64).to_degrees();
    let aperture_deg = aperture_deg_q16 as f64 / 65_536.0;
    let mut delta = angle - aperture_deg;
    while delta > 180.0 {
        delta -= 360.0;
    }
    while delta < -180.0 {
        delta += 360.0;
    }
    let width = aperture_width_q16 as f64 / 65_536.0;
    delta.abs() <= width
}

/// Paged render capsule: pages never move; add pages on overflow, no realloc copies.
#[repr(C, align(64))]
pub struct RenderSoaSlabCapsule {
    pages: Vec<RenderPage>,
    shard_offsets: Vec<(usize, usize)>,
    len: usize,
}

verify_alignment_only!(RenderSoaSlabCapsule, 64);

impl RenderSoaSlabCapsule {
    pub fn new(initial_pages: usize, shard_capacity: usize) -> Self {
        let mut pages = Vec::with_capacity(initial_pages.max(1));
        for _ in 0..initial_pages.max(1) {
            pages.push(RenderPage::new());
        }
        Self {
            pages,
            shard_offsets: Vec::with_capacity(shard_capacity),
            len: 0,
        }
    }

    pub fn reset(&mut self) {
        self.len = 0;
        self.shard_offsets.clear();
    }

    fn ensure_page(&mut self, idx: usize) {
        while self.pages.len() <= idx {
            self.pages.push(RenderPage::new());
        }
    }

    pub fn add_shard(&mut self, formations: &[FormationCapsule]) -> Result<(), RenderOverflow> {
        let start = self.len;
        for f in formations {
            let snap = f.snapshot();
            let page_idx = self.len / RENDER_PAGE_SIZE;
            let offset = self.len % RENDER_PAGE_SIZE;
            self.ensure_page(page_idx);
            let page = &mut self.pages[page_idx];
            page.formation_ids[offset] = snap.formation_id;
            page.posture[offset] = snap.posture;
            page.stance[offset] = snap.stance;
            page.morale_q16[offset] = snap.morale_q16;
            page.fatigue_q16[offset] = snap.fatigue_q16;
            page.cohesion_q16[offset] = snap.cohesion_q16;
            page.ammo[offset] = snap.ammo;
            page.facing_deg_q16[offset] = snap.facing_deg_q16;
            page.position_x_q16[offset] = snap.position_x_q16;
            page.position_z_q16[offset] = snap.position_z_q16;
            page.retreat_flags[offset] = snap.retreat_mode_flags;
            page.charge_posture[offset] = snap.charge_posture;
            page.braced[offset] = snap.braced as u8;
            page.density_q16[offset] = snap.density_q16;
            page.variance_q16[offset] = snap.variance_q16;
            page.gap_close_q16[offset] = snap.gap_close_q16;
            page.rank_variance_q16[offset] = snap.rank_variance_scale_q16;
            page.gap_fatigue_penalty_q16[offset] = snap.gap_fatigue_penalty_q16;
            self.len += 1;
        }
        if formations.len() > 0 {
            self.shard_offsets.push((start, formations.len()));
        }
        Ok(())
    }

    pub fn view(&self) -> RenderSoaView<'_> {
        let mut page_views = Vec::with_capacity(self.pages.len());
        let mut remaining = self.len;
        for page in &self.pages {
            let take = remaining.min(RENDER_PAGE_SIZE);
            if take == 0 {
                break;
            }
            page_views.push(RenderPageView {
                formation_ids: &page.formation_ids[..take],
                posture: &page.posture[..take],
                stance: &page.stance[..take],
                morale_q16: &page.morale_q16[..take],
                fatigue_q16: &page.fatigue_q16[..take],
                cohesion_q16: &page.cohesion_q16[..take],
                ammo: &page.ammo[..take],
                facing_deg_q16: &page.facing_deg_q16[..take],
                position_x_q16: &page.position_x_q16[..take],
                position_z_q16: &page.position_z_q16[..take],
                retreat_flags: &page.retreat_flags[..take],
                charge_posture: &page.charge_posture[..take],
                braced: &page.braced[..take],
                density_q16: &page.density_q16[..take],
                variance_q16: &page.variance_q16[..take],
                gap_close_q16: &page.gap_close_q16[..take],
                rank_variance_q16: &page.rank_variance_q16[..take],
                gap_fatigue_penalty_q16: &page.gap_fatigue_penalty_q16[..take],
            });
            remaining -= take;
        }
        let overlays = self.compute_overlays(&page_views);
        RenderSoaView {
            total_len: self.len,
            shard_offsets: &self.shard_offsets,
            pages: page_views,
            overlays,
        }
    }

    fn compute_overlays<'a>(&self, pages: &[RenderPageView<'a>]) -> Vec<ShardOverlay> {
        let mut overlays = Vec::with_capacity(self.shard_offsets.len());
        for (shard_id, &(start, len)) in self.shard_offsets.iter().enumerate() {
            let stride = if len > 5_000 {
                8
            } else if len > 2_000 {
                4
            } else if len > 800 {
                2
            } else {
                1
            };
            let mut morale_min = u32::MAX;
            let mut morale_max = 0;
            let mut charging = 0u32;
            let mut braced = 0u32;
            let mut density_sum = 0u64;
            let mut variance_sum = 0u64;
            let mut gap_close_sum = 0u64;
            let mut rank_variance_sum = 0u64;
            let mut gap_fatigue_sum = 0u64;
            let fog_visible_contacts = 0u32;
            let fog_visible_ratio_q16 = 0u32;
            let mut remaining = len;
            let mut idx = start;
            while remaining > 0 {
                let page_idx = idx / RENDER_PAGE_SIZE;
                let offset = idx % RENDER_PAGE_SIZE;
                let take = remaining.min(RENDER_PAGE_SIZE - offset);
                if let Some(page) = pages.get(page_idx) {
                    for &m in &page.morale_q16[offset..offset + take] {
                        morale_min = morale_min.min(m);
                        morale_max = morale_max.max(m);
                    }
                    for &charge in &page.charge_posture[offset..offset + take] {
                        if charge > 0 {
                            charging = charging.saturating_add(1);
                        }
                    }
                    for &brace_flag in &page.braced[offset..offset + take] {
                        if brace_flag != 0 {
                            braced = braced.saturating_add(1);
                        }
                    }
                    for &density in &page.density_q16[offset..offset + take] {
                        density_sum = density_sum.saturating_add(density as u64);
                    }
                    for &variance in &page.variance_q16[offset..offset + take] {
                        variance_sum = variance_sum.saturating_add(variance as u64);
                    }
                    for &gap_close in &page.gap_close_q16[offset..offset + take] {
                        gap_close_sum = gap_close_sum.saturating_add(gap_close as u64);
                    }
                    for &rank_var in &page.rank_variance_q16[offset..offset + take] {
                        rank_variance_sum = rank_variance_sum.saturating_add(rank_var as u64);
                    }
                    for &gap_fatigue in &page.gap_fatigue_penalty_q16[offset..offset + take] {
                        gap_fatigue_sum = gap_fatigue_sum.saturating_add(gap_fatigue as u64);
                    }
                    // Optional: stash supply info later via stats; keep zero here.
                }
                idx += take;
                remaining -= take;
            }
            if morale_min == u32::MAX {
                morale_min = 0;
            }
            let avg_density_q16 = if len > 0 {
                (density_sum / len as u64).min(u32::MAX as u64) as u32
            } else {
                0
            };
            let avg_variance_q16 = if len > 0 {
                (variance_sum / len as u64).min(u32::MAX as u64) as u32
            } else {
                0
            };
            let avg_gap_close_q16 = if len > 0 {
                (gap_close_sum / len as u64).min(u32::MAX as u64) as u32
            } else {
                0
            };
            let avg_rank_variance_q16 = if len > 0 {
                (rank_variance_sum / len as u64).min(u32::MAX as u64) as u32
            } else {
                0
            };
            let avg_gap_fatigue_penalty_q16 = if len > 0 {
                (gap_fatigue_sum / len as u64).min(u32::MAX as u64) as u32
            } else {
                0
            };
            overlays.push(ShardOverlay {
                shard_id,
                start,
                len,
                lod_stride: stride,
                morale_min_q16: morale_min,
                morale_max_q16: morale_max,
                charging,
                braced,
                garrisoned: 0,
                structures_breached: 0,
                avg_garrison_aperture_width_q16: 0,
                min_garrison_aperture_width_q16: 0,
                max_garrison_aperture_width_q16: 0,
                grenade_casualties: 0,
                grenade_cover_q16: 0,
                grenade_detonation_ms: 0,
                avg_density_q16,
                avg_variance_q16,
                avg_gap_close_q16,
                avg_rank_variance_q16,
                avg_gap_fatigue_penalty_q16,
                supply_pressure_avg_q16: 0,
                supply_fatigue_penalty_avg_q16: 0,
                province_infra_avg_q16: 0,
                province_resistance_avg_q16: 0,
                command_stress_q16: 0,
                courier_eta_ticks: 0,
                courier_losses: 0,
                courier_spoofed: 0,
                command_delay_applied: 0,
                command_delay_total_ticks: 0,
                strategic_hash_chain: 0,
                strategic_prev_hash_chain: 0,
                fog_visible_contacts,
                fog_visible_ratio_q16,
                artillery_ricochet_bounces: 0,
                artillery_crater_radius_tiles: 0,
                artillery_fuse_ms: 0,
                artillery_splash_q16: 0,
                rank_fire_mask_or: 0,
                rank_fire_events: 0,
                advance_fire_events: 0,
                last_doctrine_mode: 0,
                last_doctrine_cadence_ticks: 0,
                doctrine_sets: 0,
            });
        }
        overlays
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOverflow;

#[derive(Debug)]
pub enum WorldPersistError {
    Render(RenderOverflow),
    Mmap(MmapError),
}

impl From<RenderOverflow> for WorldPersistError {
    fn from(err: RenderOverflow) -> Self {
        WorldPersistError::Render(err)
    }
}

impl From<MmapError> for WorldPersistError {
    fn from(err: MmapError) -> Self {
        WorldPersistError::Mmap(err)
    }
}

/// High-level runtime capsule that wires scheduler + persistence into a single call.
///
/// Owns a render slab to avoid reallocations between frames and keeps the snapshot hash chain.
#[repr(C, align(128))]
pub struct WorldRuntimeCapsule {
    loop_capsule: WorldLoopCapsule,
    persistence: WorldPersistenceCapsule,
    render_slab: RenderSoaSlabCapsule,
}

verify_alignment_only!(WorldRuntimeCapsule, 128);

impl WorldRuntimeCapsule {
    pub fn new(loop_capsule: WorldLoopCapsule, render_pages: usize, shard_capacity: usize) -> Self {
        Self {
            loop_capsule,
            persistence: WorldPersistenceCapsule::new(),
            render_slab: RenderSoaSlabCapsule::new(render_pages, shard_capacity),
        }
    }

    pub fn loop_capsule(&self) -> &WorldLoopCapsule {
        &self.loop_capsule
    }

    pub fn reset_snapshot_chain(&self, prev_hash: u64) {
        self.persistence.reset_chain(prev_hash);
    }

    pub fn render_slab(&mut self) -> &mut RenderSoaSlabCapsule {
        &mut self.render_slab
    }

    #[allow(clippy::too_many_arguments)]
    pub fn tick_and_persist<'a, const FB: usize, const N: usize>(
        &'a mut self,
        shards: &'a [ShardContext<'a, FB>],
        replay_log: &ReplayLogCapsule<N>,
        replay_flush: &ReplayFlushCapsule,
        replay_mmap: &mut ReplayMmapCapsule,
        replay_index: Option<&ReplayIndexCapsule>,
        snapshot_capsule: &CampaignSnapshotCapsule,
        snapshot_mmap: &mut SnapshotMmapCapsule,
        formations: &[FormationCapsule],
        structures: &[StructureCapsule],
        orders: &OrderQueueCapsule,
        telemetry: &TelemetryCapsule,
        strategic: Option<&StrategicSnapshot>,
        diplomatic: Option<&crate::diplomacy::DiplomaticSnapshot>,
        economy: Option<&crate::province_economy::EconomySnapshot>,
        command_delays: Option<&crate::order::CommandDelayBufferCapsule>,
    ) -> Result<(WorldFrame<'a>, crate::replay::ReplayPersistSnapshot, u64), WorldPersistError>
    {
        self.persistence.run_and_persist_frame(
            &self.loop_capsule,
            shards,
            &mut self.render_slab,
            replay_log,
            replay_flush,
            replay_mmap,
            replay_index,
            snapshot_capsule,
            snapshot_mmap,
            formations,
            structures,
            orders,
            telemetry,
            strategic,
            diplomatic,
            economy,
            command_delays,
        )
    }
}

/// Snapshot + replay persistence orchestrator (capsule).
#[repr(C, align(128))]
pub struct WorldPersistenceCapsule {
    snapshot_prev_hash: AtomicU64,
    _padding: [u8; 120],
}

verify_alignment_only!(WorldPersistenceCapsule, 128);

impl WorldPersistenceCapsule {
    pub const fn new() -> Self {
        Self {
            snapshot_prev_hash: AtomicU64::new(0),
            _padding: [0; 120],
        }
    }

    pub fn reset_chain(&self, prev: u64) {
        self.snapshot_prev_hash.store(prev, Ordering::Release);
    }

    /// Run a frame (with replay flush) and append a campaign snapshot to mmap; returns snapshot hash chain.
    #[allow(clippy::too_many_arguments)]
    pub fn run_and_persist_frame<'a, const FB: usize, const N: usize>(
        &self,
        world_loop: &WorldLoopCapsule,
        shards: &'a [ShardContext<'a, FB>],
        render_slab: &'a mut RenderSoaSlabCapsule,
        replay_log: &ReplayLogCapsule<N>,
        replay_flush: &ReplayFlushCapsule,
        replay_mmap: &mut ReplayMmapCapsule,
        replay_index: Option<&ReplayIndexCapsule>,
        snapshot_capsule: &CampaignSnapshotCapsule,
        snapshot_mmap: &mut SnapshotMmapCapsule,
        formations: &[FormationCapsule],
        structures: &[StructureCapsule],
        orders: &OrderQueueCapsule,
        telemetry: &TelemetryCapsule,
        strategic: Option<&StrategicSnapshot>,
        diplomatic: Option<&DiplomaticSnapshot>,
        economy: Option<&crate::province_economy::EconomySnapshot>,
        command_delays: Option<&crate::order::CommandDelayBufferCapsule>,
    ) -> Result<(WorldFrame<'a>, crate::replay::ReplayPersistSnapshot, u64), WorldPersistError>
    {
        let (frame, replay_snap) = world_loop
            .run_frame_with_replay_flush(
                shards,
                render_slab,
                replay_log,
                replay_flush,
                replay_mmap,
                replay_index,
                strategic,
            )
            .map_err(WorldPersistError::from)?;

        let prev_hash = self.snapshot_prev_hash.load(Ordering::Relaxed);
        let snapshot_bytes = snapshot_capsule.serialize(
            formations,
            orders,
            telemetry,
            structures,
            strategic,
            diplomatic,
            economy,
            command_delays,
            prev_hash,
        );
        let (chain, _offset, _len) = snapshot_mmap
            .append_and_verify(&snapshot_bytes, prev_hash)
            .map_err(WorldPersistError::from)?;
        self.snapshot_prev_hash.store(chain, Ordering::Release);
        Ok((frame, replay_snap, chain))
    }
}

#[derive(Debug, Clone)]
pub struct RenderPageView<'a> {
    pub formation_ids: &'a [u32],
    pub posture: &'a [u8],
    pub stance: &'a [u8],
    pub morale_q16: &'a [u32],
    pub fatigue_q16: &'a [u32],
    pub cohesion_q16: &'a [u32],
    pub ammo: &'a [u32],
    pub facing_deg_q16: &'a [u32],
    pub position_x_q16: &'a [u32],
    pub position_z_q16: &'a [u32],
    pub retreat_flags: &'a [u16],
    pub charge_posture: &'a [u8],
    pub braced: &'a [u8],
    pub density_q16: &'a [u32],
    pub variance_q16: &'a [u32],
    pub gap_close_q16: &'a [u32],
    pub rank_variance_q16: &'a [u32],
    pub gap_fatigue_penalty_q16: &'a [u32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShardOverlay {
    pub shard_id: usize,
    pub start: usize,
    pub len: usize,
    pub lod_stride: usize,
    pub morale_min_q16: u32,
    pub morale_max_q16: u32,
    pub charging: u32,
    pub braced: u32,
    pub garrisoned: u32,
    pub structures_breached: u32,
    pub avg_garrison_aperture_width_q16: u32,
    pub min_garrison_aperture_width_q16: u32,
    pub max_garrison_aperture_width_q16: u32,
    pub grenade_casualties: u32,
    pub grenade_cover_q16: u32,
    pub grenade_detonation_ms: u32,
    pub avg_density_q16: u32,
    pub avg_variance_q16: u32,
    pub avg_gap_close_q16: u32,
    pub avg_rank_variance_q16: u32,
    pub avg_gap_fatigue_penalty_q16: u32,
    pub supply_pressure_avg_q16: u32,
    pub supply_fatigue_penalty_avg_q16: u32,
    pub province_infra_avg_q16: u32,
    pub province_resistance_avg_q16: u32,
    pub command_stress_q16: u32,
    pub courier_eta_ticks: u32,
    pub courier_losses: u32,
    pub courier_spoofed: u32,
    pub command_delay_applied: u32,
    pub command_delay_total_ticks: u32,
    pub strategic_hash_chain: u64,
    pub strategic_prev_hash_chain: u64,
    pub fog_visible_contacts: u32,
    pub fog_visible_ratio_q16: u32,
    pub artillery_ricochet_bounces: u32,
    pub artillery_crater_radius_tiles: u32,
    pub artillery_fuse_ms: u32,
    pub artillery_splash_q16: u32,
    pub rank_fire_mask_or: u8,
    pub rank_fire_events: u32,
    pub advance_fire_events: u32,
    pub last_doctrine_mode: u8,
    pub last_doctrine_cadence_ticks: u16,
    pub doctrine_sets: u32,
}

#[derive(Debug, Clone)]
pub struct RenderSoaView<'a> {
    pub total_len: usize,
    pub shard_offsets: &'a [(usize, usize)],
    pub pages: Vec<RenderPageView<'a>>,
    pub overlays: Vec<ShardOverlay>,
}

#[inline(always)]
fn compute_shock_penalty_q16(
    shock_delta: u64,
    shock_weight_delta_q16: u64,
    casualty_delta: u64,
    formation_count: usize,
    avg_fatigue_q16: u32,
    avg_ammo: u32,
) -> u32 {
    if formation_count == 0
        || (shock_delta == 0 && casualty_delta == 0 && shock_weight_delta_q16 == 0)
    {
        return 0;
    }
    let denom = formation_count as u64;
    let shock_term = shock_delta
        .saturating_mul(1_500)
        .saturating_div(denom.max(1));
    let casualty_term = casualty_delta.saturating_mul(96).saturating_div(denom);
    // Convert weighted artillery shock (Q16) into an integer penalty; emphasize heavy volleys.
    let artillery_term = shock_weight_delta_q16
        .saturating_div(denom.max(1))
        .saturating_mul(3)
        .saturating_div(256)
        .min(80_000);
    // Fatigue amplifies impact; abundant ammo dampens fear.
    let fatigue_scale_q16 = (avg_fatigue_q16.saturating_add(24_000)).min(65_536);
    let ammo_dampen_q16 = if avg_ammo > 0 {
        65_536u64
            .saturating_sub((avg_ammo as u64).min(12_000) * 2)
            .max(24_576)
    } else {
        65_536
    };
    let base = shock_term
        .saturating_add(casualty_term)
        .saturating_add(artillery_term);
    let scaled = base
        .saturating_mul(fatigue_scale_q16 as u64)
        .saturating_div(65_536);
    let dampened = scaled
        .saturating_mul(ammo_dampen_q16)
        .saturating_div(65_536);
    dampened.min(60_000) as u32
}

#[derive(Debug, Clone, Copy)]
pub struct RenderEntry {
    pub formation_id: u32,
    pub posture: u8,
    pub stance: u8,
    pub morale_q16: u32,
    pub fatigue_q16: u32,
    pub cohesion_q16: u32,
    pub ammo: u32,
    pub facing_deg_q16: u32,
    pub position_x_q16: u32,
    pub position_z_q16: u32,
    pub retreat_flags: u16,
}

pub struct RenderIter<'a> {
    view: &'a RenderSoaView<'a>,
    stride: usize,
    idx: usize,
    shard_bounds: Option<(usize, usize)>,
}

impl<'a> RenderSoaView<'a> {
    /// Get (start, len) for a shard in the flattened render buffer.
    pub fn shard_span(&self, shard_id: usize) -> Option<(usize, usize)> {
        self.shard_offsets.get(shard_id).copied()
    }

    /// Iterate the entire render buffer with optional LOD stride (>=1).
    pub fn iter_strided(&'a self, stride: usize) -> RenderIter<'a> {
        RenderIter {
            view: self,
            stride: stride.max(1),
            idx: 0,
            shard_bounds: None,
        }
    }

    /// Iterate a single shard slice with optional LOD stride.
    pub fn iter_shard(&'a self, shard_id: usize, stride: usize) -> Option<RenderIter<'a>> {
        let (start, len) = self.shard_span(shard_id)?;
        Some(RenderIter {
            view: self,
            stride: stride.max(1),
            idx: start,
            shard_bounds: Some((start, len)),
        })
    }
}

impl<'a> Iterator for RenderIter<'a> {
    type Item = RenderEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((start, len)) = self.shard_bounds {
            if self.idx >= start + len {
                return None;
            }
        } else if self.idx >= self.view.total_len {
            return None;
        }

        let idx = self.idx;
        self.idx += self.stride;

        let page_idx = idx / RENDER_PAGE_SIZE;
        let offset = idx % RENDER_PAGE_SIZE;
        let page = self.view.pages.get(page_idx)?;
        Some(RenderEntry {
            formation_id: *page.formation_ids.get(offset)?,
            posture: *page.posture.get(offset)?,
            stance: *page.stance.get(offset)?,
            morale_q16: *page.morale_q16.get(offset)?,
            fatigue_q16: *page.fatigue_q16.get(offset)?,
            cohesion_q16: *page.cohesion_q16.get(offset)?,
            ammo: *page.ammo.get(offset)?,
            facing_deg_q16: *page.facing_deg_q16.get(offset)?,
            position_x_q16: *page.position_x_q16.get(offset)?,
            position_z_q16: *page.position_z_q16.get(offset)?,
            retreat_flags: *page.retreat_flags.get(offset)?,
        })
    }
}

pub fn collect_world_render_slab<'a>(
    shard_formations: &'a [&'a [FormationCapsule]],
    slab: &'a mut RenderSoaSlabCapsule,
) -> Result<RenderSoaView<'a>, RenderOverflow> {
    slab.reset();
    for shard in shard_formations {
        slab.add_shard(shard)?;
    }
    Ok(slab.view())
}

// ---------------- Tick + Scheduler ----------------

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShardTickStats {
    pub processed_orders: u64,
    pub ai_decisions: u32,
    pub ai_replay_len: u8,
    pub ai_replay_payloads: [u64; MAX_AI_DECISIONS_PER_TICK],
    pub visible_contacts: u32,
    pub visible_samples: u32,
    pub visible_ratio_q16: u32,
    pub moved: u64,
    pub retreats: u64,
    pub garrisoned: u32,
    pub structures_breached: u32,
    pub avg_garrison_aperture_width_q16: u32,
    pub min_garrison_aperture_width_q16: u32,
    pub max_garrison_aperture_width_q16: u32,
    pub grenade_casualties: u64,
    pub grenade_cover_q16: u32,
    pub grenade_detonation_ms: u32,
    pub supply_pressure_avg_q16: u32,
    pub supply_fatigue_penalty_avg_q16: u32,
    pub province_infra_avg_q16: u32,
    pub province_resistance_avg_q16: u32,
    pub command_stress_q16: u32,
    pub courier_eta_ticks: u32,
    pub command_delay_hist: [u32; COMMAND_HIST_BUCKETS],
    pub courier_eta_hist: [u32; COMMAND_HIST_BUCKETS],
    pub command_delay_applied: u32,
    pub command_delay_total_ticks: u32,
    pub courier_losses: u32,
    pub courier_spoofed: u32,
    pub strategic_hash_chain: u64,
    pub strategic_prev_hash_chain: u64,
    pub artillery_ricochet_bounces: u32,
    pub artillery_crater_radius_tiles: u32,
    pub artillery_fuse_ms: u32,
    pub artillery_splash_q16: u32,
    pub last_charge_start_x_q16: u32,
    pub last_charge_start_z_q16: u32,
    pub last_charge_target_x_q16: u32,
    pub last_charge_target_z_q16: u32,
    pub last_charge_impact_mode: u8,
    pub rank_fire_mask_or: u8,
    pub rank_fire_events: u32,
    pub advance_fire_events: u32,
    pub last_doctrine_mode: u8,
    pub last_doctrine_cadence_ticks: u16,
    pub doctrine_sets: u32,
}

/// Tick a shard: drain orders, apply to formations, step pathing (retreat-aware).
pub fn tick_shard<const FB: usize>(
    tick: u64,
    shard_id: usize,
    orders: &OrderQueueCapsule,
    formations: &[FormationCapsule],
    pathings: &[PathingCapsule],
    telemetry: &TelemetryCapsule,
    formation_breaks: Option<&FormationBreakTelemetryCapsule<FB>>,
    ballistics: Option<&crate::ballistics::BallisticsCapsule>,
    fire_profile: Option<&crate::ballistics::FireControlProfileCapsule>,
    terrain: Option<&crate::terrain::TerrainGridCapsule>,
    grenades: Option<&crate::grenade::GrenadeCapsule>,
    structures_opt: Option<&[StructureCapsule]>,
    garrisons: Option<&crate::garrison::GarrisonSlabCapsule>,
    supply: Option<&SupplySnapshot>,
    courier: Option<&CourierCapsule>,
    fire_doctrine: Option<&FireDoctrineCapsule>,
    battle_ai: Option<&crate::battle_ai::BattleAiCapsule>,
    fog: Option<&crate::fog::FogOfWarCapsule>,
    generals: Option<&[GeneralSnapshot]>,
    command_hierarchy: Option<&CommandHierarchyCapsule>,
    commanders: Option<&[CommanderSnapshot]>,
    strategic: Option<&StrategicSnapshot>,
    command_delays: Option<&crate::order::CommandDelayBufferCapsule>,
) -> ShardTickStats {
    let mut stats = ShardTickStats::default();

    // Command graph stress: queue depth normalized to capacity (Q16.16).
    let qstats = orders.stats();
    let depth = qstats.tail.wrapping_sub(qstats.head);
    if qstats.capacity > 0 {
        let capped = depth.min(qstats.capacity);
        stats.command_stress_q16 =
            ((capped.saturating_mul(65_536) / qstats.capacity).min(u32::MAX as u64)) as u32;
    }
    // Courier debug signal: expected latency baseline + loss/spoof counters.
    if let Some(c) = courier {
        let snap = c.debug_snapshot();
        let eta = snap
            .base_eta_ticks
            .saturating_add(snap.cadence_ticks.saturating_div(2));
        stats.courier_eta_ticks = eta;
        stats.courier_losses = snap.losses.min(u32::MAX as u64) as u32;
        stats.courier_spoofed = snap.spoofed.min(u32::MAX as u64) as u32;
    }

    let base_eta_ticks = stats.courier_eta_ticks;

    // Command chain penalty: missing/out-of-range commanders add delay and stress.
    if let (Some(hierarchy), Some(cmds)) = (command_hierarchy, commanders) {
        let mut penalty_sum = 0u64;
        let mut samples = 0u64;
        for (idx, formation) in formations.iter().enumerate() {
            if let Some(cid) = hierarchy.commander_for(idx) {
                if let Some(cmd) = cmds.get(cid as usize) {
                    let snap = formation.snapshot();
                    let in_range = cmd.in_command_range(snap.position_x_q16, snap.position_z_q16);
                    let penalty_ticks = if in_range {
                        0
                    } else {
                        cmd.command_delay_ticks
                    };
                    if penalty_ticks > 0 {
                        penalty_sum = penalty_sum.saturating_add(penalty_ticks as u64);
                    }
                    if base_eta_ticks > 0 || penalty_ticks > 0 {
                        let delay_bucket = command_hist_bucket(penalty_ticks);
                        stats.command_delay_hist[delay_bucket] = stats.command_delay_hist[delay_bucket]
                            .saturating_add(1);
                        let eta_bucket = command_hist_bucket(
                            base_eta_ticks.saturating_add(penalty_ticks),
                        );
                        stats.courier_eta_hist[eta_bucket] =
                            stats.courier_eta_hist[eta_bucket].saturating_add(1);
                    }
                    samples = samples.saturating_add(1);
                }
            }
        }
        if samples > 0 {
            let avg_penalty = (penalty_sum / samples).min(65_536);
            let penalty_q16 = (avg_penalty as u64 * 65_536 / 64).min(u32::MAX as u64) as u32;
            stats.command_stress_q16 = stats
                .command_stress_q16
                .saturating_add(penalty_q16)
                .min(u32::MAX);
            if stats.courier_eta_ticks > 0 {
                let scale = 65_536u64.saturating_add(penalty_q16.min(20_000) as u64);
                stats.courier_eta_ticks = ((stats.courier_eta_ticks as u64 * scale) / 65_536)
                    .min(u32::MAX as u64) as u32;
            }
        }
    }
    // If we have courier latency but no command hierarchy, still bucket per-formation ETA baseline.
    else if base_eta_ticks > 0 {
        for _ in formations.iter() {
            let eta_bucket = command_hist_bucket(base_eta_ticks);
            stats.courier_eta_hist[eta_bucket] =
                stats.courier_eta_hist[eta_bucket].saturating_add(1);
            let delay_bucket = command_hist_bucket(0);
            stats.command_delay_hist[delay_bucket] =
                stats.command_delay_hist[delay_bucket].saturating_add(1);
        }
    }

    if let Some(gs) = garrisons {
        let mut width_sum = 0u64;
        let mut count = 0u64;
        let mut min_width = u32::MAX;
        let mut max_width = 0u32;
        for g in gs.iter() {
            if g.formation_id != u32::MAX {
                stats.garrisoned = stats.garrisoned.saturating_add(1);
                width_sum = width_sum.saturating_add(g.aperture_width_deg_q16 as u64);
                count = count.saturating_add(1);
                min_width = min_width.min(g.aperture_width_deg_q16);
                max_width = max_width.max(g.aperture_width_deg_q16);
            }
        }
        if count > 0 {
            stats.avg_garrison_aperture_width_q16 = (width_sum / count).min(u32::MAX as u64) as u32;
            stats.min_garrison_aperture_width_q16 = min_width;
            stats.max_garrison_aperture_width_q16 = max_width;
        }
    }
    let snapshots_needed = battle_ai.is_some() || fog.is_some() || generals.is_some();
    let snaps_storage: Vec<_>;
    let snaps_opt = if snapshots_needed {
        snaps_storage = formations.iter().map(|f| f.snapshot()).collect();
        Some(&snaps_storage)
    } else {
        None
    };

    if let Some(ai) = battle_ai {
        // Run deterministic AI planning and emit orders.
        let snaps = snaps_opt.expect("snapshots present");
        let courier_latency_ticks = courier
            .map(|c| c.debug_snapshot().base_eta_ticks)
            .unwrap_or(1);
        let fog_view = fog.map(|f| {
            let view = crate::fog::FogOfWarView::new(f);
            if let Some(terrain) = terrain {
                view.with_terrain(terrain)
            } else {
                view
            }
        });
        let plan = ai.plan_for_shard(crate::battle_ai::BattleAiInputs {
            tick,
            formations: snaps,
            doctrine: None,
            courier_latency_ticks: courier_latency_ticks.min(u16::MAX as u32) as u16,
            fog: fog_view,
        });
        stats.ai_decisions = plan.len() as u32;
        if stats.ai_decisions > 0 {
            telemetry.log_ai_orders(stats.ai_decisions);
            let mut replay_len: u8 = 0;
            for (idx, payload) in plan.replay_payloads().enumerate() {
                if idx >= MAX_AI_DECISIONS_PER_TICK {
                    break;
                }
                stats.ai_replay_payloads[idx] = payload;
                replay_len = replay_len.saturating_add(1);
            }
            stats.ai_replay_len = replay_len;
        }
        for decision in plan.iter() {
            let payload_a = pack_ai_order_payload(
                decision.target_formation_id,
                decision.order,
                decision.score_q8,
            );
            let _ = orders.push_order(
                decision.order,
                decision.source_formation_id,
                payload_a,
                decision.generation as u64,
            );
        }
    }
    if let (Some(snaps), Some(fog_capsule)) = (snaps_opt, fog) {
        let stride = (snaps.len() / 256).max(1);
        let mut visible = 0u64;
        let mut samples = 0u64;
        let fog_view = if let Some(terrain) = terrain {
            crate::fog::FogOfWarView::new(fog_capsule).with_terrain(terrain)
        } else {
            crate::fog::FogOfWarView::new(fog_capsule)
        };
        for i in (0..snaps.len()).step_by(stride) {
            for j in (i + 1)..snaps.len() {
                if j % stride != 0 {
                    continue;
                }
                samples = samples.saturating_add(1);
                let a = &snaps[i];
                let b = &snaps[j];
                if fog_view.can_see(a, b) || fog_view.can_see(b, a) {
                    visible = visible.saturating_add(1);
                }
            }
        }
        stats.visible_contacts = visible.min(u32::MAX as u64) as u32;
        stats.visible_samples = samples.min(u32::MAX as u64) as u32;
        if samples > 0 {
            stats.visible_ratio_q16 =
                ((visible.saturating_mul(65_536) / samples).min(u32::MAX as u64)) as u32;
        }
    }
    if let Some(structs) = structures_opt {
        stats.structures_breached =
            crate::structure::breached_structure_count(structs).min(u32::MAX as usize) as u32;
    }

    if let Some(strat) = strategic {
        stats.strategic_hash_chain = strat.hash_chain;
        stats.strategic_prev_hash_chain = strat.prev_hash_chain;
        if !strat.provinces.is_empty() {
            let mut infra_sum = 0u64;
            let mut resistance_sum = 0u64;
            for province in &strat.provinces {
                infra_sum = infra_sum.saturating_add(province.infrastructure_q16 as u64);
                resistance_sum = resistance_sum.saturating_add(province.resistance_q16 as u64);
            }
            let count = strat.provinces.len() as u64;
            stats.province_infra_avg_q16 = (infra_sum / count).min(u32::MAX as u64) as u32;
            stats.province_resistance_avg_q16 =
                (resistance_sum / count).min(u32::MAX as u64) as u32;
        }
    }

    let mut supply_pressure_sum = 0u64;
    let mut supply_fatigue_sum = 0u64;
    let mut supply_count = 0u32;
    if let Some(snap) = supply {
        for (idx, formation) in formations.iter().enumerate() {
            if let Some(&fatigue_q16) = snap.fatigue_penalty_q16.get(idx) {
                if fatigue_q16 > 0 {
                    formation.adjust_fatigue(fatigue_q16 as i32);
                }
            }
            if let Some(&ammo_gain) = snap.ammo_gain.get(idx) {
                formation.resupply_ammo(ammo_gain);
            }
            if let Some(&p) = snap.pressure.get(idx) {
                if p < 28_000 {
                    // Morale drag when undersupplied; gentle slope to avoid sharp drops.
                    let morale_penalty = ((28_000u32.saturating_sub(p)) / 32).min(12_000);
                    if morale_penalty > 0 {
                        formation.adjust_morale(-(morale_penalty as i32));
                    }
                }
                supply_pressure_sum = supply_pressure_sum.saturating_add(p as u64);
            }
            if let Some(&f) = snap.fatigue_penalty_q16.get(idx) {
                supply_fatigue_sum = supply_fatigue_sum.saturating_add(f as u64);
            }
            supply_count = supply_count.saturating_add(1);
        }
    }

    if let (Some(snaps), Some(generals)) = (snaps_opt.as_ref(), generals) {
        for (idx, snap) in snaps.iter().enumerate() {
            let mut morale_boost = 0u32;
            let mut fatigue_recovery = 0u32;
            for general in generals.iter() {
                if general.in_aura(snap.position_x_q16, snap.position_z_q16) {
                    morale_boost = morale_boost.saturating_add(general.morale_boost_q16);
                    fatigue_recovery =
                        fatigue_recovery.saturating_add(general.fatigue_recovery_q16);
                }
            }
            if morale_boost > 0 {
                if let Some(f) = formations.get(idx) {
                    let delta = morale_boost.min(i32::MAX as u32) as i32;
                    f.adjust_morale(delta);
                }
            }
            if fatigue_recovery > 0 {
                if let Some(f) = formations.get(idx) {
                    let delta = fatigue_recovery.min(i32::MAX as u32) as i32;
                    f.adjust_fatigue(-(delta));
                }
            }
        }
    }

    let mut ready_orders: Vec<OrderData> = Vec::new();
    if let Some(buffer) = command_delays {
        buffer.drain_ready(tick, &mut ready_orders);
    }
    let courier_eta_base = courier
        .map(|c| {
            let snap = c.debug_snapshot();
            snap.base_eta_ticks
                .saturating_add(snap.cadence_ticks.saturating_div(2))
        })
        .unwrap_or(0);

    while let Some(order) = orders.pop_order() {
        // Compute per-order delay from commander range + courier baseline.
        let mut delay_ticks = courier_eta_base;
        if let (Some(hierarchy), Some(cmds)) = (command_hierarchy, commanders) {
            if let Some(cid) = hierarchy.commander_for(order.formation_id as usize) {
                if let Some(cmd) = cmds.get(cid as usize) {
                    let snap = formations
                        .get(order.formation_id as usize)
                        .map(|f| f.snapshot());
                    if let Some(snap) = snap {
                        if !cmd.in_command_range(snap.position_x_q16, snap.position_z_q16) {
                            delay_ticks = delay_ticks.saturating_add(cmd.command_delay_ticks);
                        }
                    }
                }
            }
        }
        if delay_ticks > 0 {
            if let Some(buffer) = command_delays {
                let ready_tick = tick.saturating_add(delay_ticks as u64);
                if buffer.enqueue(&order, ready_tick) {
                    stats.command_delay_applied =
                        stats.command_delay_applied.saturating_add(1);
                    stats.command_delay_total_ticks = stats
                        .command_delay_total_ticks
                        .saturating_add(delay_ticks);
                    continue;
                }
            }
        }
        ready_orders.push(order);
    }

    for order in ready_orders {
        let fid = order.formation_id as usize;
        if let Some(formation) = formations.get(fid) {
            stats.processed_orders += 1;
            let pathing = pathings.get(fid);
            if matches!(order.kind, OrderKind::Move | OrderKind::Charge) {
                stats.moved += 1;
            }
            apply_order_with_breaks(
                order,
                formations,
                formation,
                pathing,
                telemetry,
                formation_breaks,
                ballistics,
                fire_profile,
                terrain,
                grenades,
                structures_opt,
                garrisons,
                fire_doctrine,
                stats.processed_orders as u32,
                &mut stats,
            );
        }
    }

    for (idx, formation) in formations.iter().enumerate() {
        if let Some(pathing) = pathings.get(idx) {
            let (mode, backstep) = formation.retreat_state();
            let should_backstep = backstep || mode != RetreatMode::None;
            let moved = pathing.step_with_backstep(formation, telemetry, should_backstep);
            if moved {
                stats.moved += 1;
            }
            if mode != RetreatMode::None {
                stats.retreats += 1;
            }
        }
    }

    if supply_count > 0 {
        stats.supply_pressure_avg_q16 =
            (supply_pressure_sum / supply_count as u64).min(u32::MAX as u64) as u32;
        stats.supply_fatigue_penalty_avg_q16 =
            (supply_fatigue_sum / supply_count as u64).min(u32::MAX as u64) as u32;
        telemetry.log_supply_stats(
            stats.supply_pressure_avg_q16,
            stats.supply_fatigue_penalty_avg_q16,
            supply_count,
        );
    }

    if let Some(snap) = supply {
        if snap.baggage_captured {
            stats.courier_losses = stats.courier_losses.saturating_add(1);
            telemetry.log_event();
        }
        if stats.courier_eta_ticks > 0 {
            if snap.avg_pressure_q16 > 0 && snap.avg_pressure_q16 < 20_000 {
                let slow_scale = 65_536u64
                    .saturating_add(((20_000u32.saturating_sub(snap.avg_pressure_q16)) as u64) * 2);
                stats.courier_eta_ticks = ((stats.courier_eta_ticks as u64 * slow_scale) / 65_536)
                    .min(u32::MAX as u64) as u32;
            }
        } else if snap.avg_pressure_q16 < 25_000 {
            stats.courier_eta_ticks =
                (25_000u32.saturating_sub(snap.avg_pressure_q16) / 128).saturating_add(4);
        }
    }

    let _ = shard_id;
    stats
}

fn nearest_target_snapshot(
    target_x_q16: u32,
    target_z_q16: u32,
    shooter_id: usize,
    formations: &[FormationCapsule],
) -> Option<FormationSnapshot> {
    let tx = target_x_q16 as i64;
    let tz = target_z_q16 as i64;
    let mut best = None;
    let mut best_dist = i128::MAX;
    for (idx, formation) in formations.iter().enumerate() {
        if idx == shooter_id {
            continue;
        }
        let snap = formation.snapshot();
        let dx = snap.position_x_q16 as i64 - tx;
        let dz = snap.position_z_q16 as i64 - tz;
        let dist2 = (dx as i128 * dx as i128) + (dz as i128 * dz as i128);
        if dist2 < best_dist {
            best_dist = dist2;
            best = Some(snap);
        }
    }
    best
}

fn apply_order_with_breaks<const FB: usize>(
    order: OrderData,
    formations: &[FormationCapsule],
    formation: &FormationCapsule,
    pathing: Option<&PathingCapsule>,
    telemetry: &TelemetryCapsule,
    formation_breaks: Option<&FormationBreakTelemetryCapsule<FB>>,
    ballistics: Option<&crate::ballistics::BallisticsCapsule>,
    fire_profile: Option<&crate::ballistics::FireControlProfileCapsule>,
    terrain: Option<&crate::terrain::TerrainGridCapsule>,
    grenades: Option<&crate::grenade::GrenadeCapsule>,
    structures: Option<&[StructureCapsule]>,
    garrisons: Option<&crate::garrison::GarrisonSlabCapsule>,
    fire_doctrine: Option<&FireDoctrineCapsule>,
    sim_tick: u32,
    stats: &mut ShardTickStats,
) {
    let shooter_id = order.formation_id as usize;
    if order.kind == OrderKind::Withdraw {
        telemetry.log_formation_break();
        if let Some(per) = formation_breaks {
            per.record(order.formation_id as usize);
        }
    }
    match order.kind {
        OrderKind::Move => {
            if let Some(pathing) = pathing {
                let (x, z) = unpack_move_payload(order.payload_a);
                pathing.set_target(x, z);
            }
            formation.apply_order(&order, telemetry);
        }
        OrderKind::FallBack | OrderKind::Withdraw => {
            if let Some(pathing) = pathing {
                let (x, z) = unpack_retreat_payload(order.payload_a);
                pathing.set_target(x, z);
                pathing.clear_charge();
            }
            formation.apply_order(&order, telemetry);
        }
        OrderKind::Charge => {
            let (x, z) = unpack_move_payload(order.payload_a);
            let (charge_posture, commit) = unpack_charge_meta(order.payload_b);
            if let Some(pathing) = pathing {
                let snap = formation.snapshot();
                pathing.set_charge_target(snap.position_x_q16, snap.position_z_q16, x, z, commit);
                stats.last_charge_start_x_q16 = snap.position_x_q16;
                stats.last_charge_start_z_q16 = snap.position_z_q16;
                stats.last_charge_target_x_q16 = x;
                stats.last_charge_target_z_q16 = z;
                stats.last_charge_impact_mode = if commit { 1 } else { 0 };
            }
            formation.set_charge_posture(charge_posture);
            formation.set_braced(false);
            telemetry.log_charge_order(commit);
            telemetry.log_event();
            formation.apply_order(&order, telemetry);
        }
        OrderKind::Grenade => {
            if let (Some(grenade), Some(terrain)) = (grenades, terrain) {
                let (target_x_q16, target_z_q16) = unpack_grenade_payload(order.payload_a);
                let (fuse_ms, fragments) = unpack_grenade_meta(order.payload_b);
                let snap = formation.snapshot();
                if let Some(gsnap) = garrisons.and_then(|g| g.find_by_formation(order.formation_id))
                {
                    if !within_aperture(
                        gsnap.aperture_deg_q16,
                        gsnap.aperture_width_deg_q16,
                        snap.position_x_q16,
                        snap.position_z_q16,
                        target_x_q16,
                        target_z_q16,
                    ) {
                        return;
                    }
                }
                let outcome = grenade.throw(
                    snap.position_x_q16,
                    snap.position_z_q16,
                    target_x_q16,
                    target_z_q16,
                    terrain,
                    structures,
                    Some(fuse_ms as u32),
                    Some(fragments as u32),
                    stats.processed_orders,
                );
                telemetry.log_event();
                if outcome.expected_casualties > 0 {
                    telemetry.add_casualties(outcome.expected_casualties);
                }
                stats.grenade_casualties = stats
                    .grenade_casualties
                    .saturating_add(outcome.expected_casualties as u64);
                stats.grenade_cover_q16 = outcome.avg_cover_q16;
                stats.grenade_detonation_ms = outcome.detonation_ms;
            }
        }
        OrderKind::GarrisonEnter => {
            let (structure_id, slot) = unpack_garrison_payload(order.payload_a);
            let aperture_deg_q16 = order.payload_b as u32;
            if let Some(gs) = garrisons {
                let base_width_q16 = 60u32 << 16;
                let slot_penalty_q16 = (slot as u32).saturating_mul(3 << 16);
                let aperture_width_q16 = base_width_q16
                    .saturating_sub(slot_penalty_q16)
                    .max(30 << 16);
                let _ = gs.occupy(
                    structure_id,
                    order.formation_id,
                    slot,
                    aperture_deg_q16,
                    aperture_width_q16,
                );
            }
            formation.set_braced(true);
            formation.adjust_morale(1_500);
            telemetry.log_event();
            formation.apply_order(&order, telemetry);
        }
        OrderKind::GarrisonExit => {
            if let Some(gs) = garrisons {
                gs.vacate_formation(order.formation_id);
            }
            formation.set_braced(false);
            formation.adjust_morale(-1_000);
            telemetry.log_event();
            formation.apply_order(&order, telemetry);
        }
        OrderKind::Brace => {
            if let Some(pathing) = pathing {
                pathing.clear_charge();
            }
            let braced = unpack_brace_payload(order.payload_a);
            formation.set_braced(braced);
            telemetry.log_brace_order();
            telemetry.log_event();
        }
        OrderKind::Fire => {
            let snap = formation.snapshot();
            let garrison_aperture = garrisons.and_then(|g| g.find_by_formation(order.formation_id));
            if let Some(gsnap) = garrison_aperture {
                let aperture_deg_q16 = gsnap.aperture_deg_q16;
                let aperture_width_q16 = if gsnap.aperture_width_deg_q16 == 0 {
                    45u32 << 16
                } else {
                    gsnap.aperture_width_deg_q16
                };
                let facing = snap.facing_deg_q16;
                let period = 360i64 << 16;
                let mut delta = (aperture_deg_q16 as i64 - facing as i64) % period;
                if delta < -(period / 2) {
                    delta += period;
                } else if delta > period / 2 {
                    delta -= period;
                }
                if delta.unsigned_abs() > aperture_width_q16 as u64 {
                    return;
                }
            }
            let rank_count = if garrison_aperture.is_some() {
                1u8
            } else {
                3u8
            };
            let decision =
                fire_doctrine.map(|d| d.plan_fire(order.formation_id, sim_tick, rank_count));
            if let Some(dec) = decision {
                if !dec.fire_now {
                    return;
                }
                stats.rank_fire_mask_or |= dec.rank_mask;
                stats.rank_fire_events = stats.rank_fire_events.saturating_add(1);
                if dec.advance_step {
                    stats.advance_fire_events = stats.advance_fire_events.saturating_add(1);
                }
                stats.last_doctrine_mode = dec.mode as u8;
                stats.last_doctrine_cadence_ticks = dec.cadence_ticks;
            }
            let ranks = decision
                .map(|d| d.rank_mask.count_ones().max(1) as u32)
                .unwrap_or(rank_count as u32);
            let mut adjusted = order;
            let base_ammo = (order.payload_a & 0xFFFF) as u32;
            let scaled = (base_ammo.saturating_mul(ranks)).max(1) / (rank_count.max(1) as u32);
            adjusted.payload_a = (order.payload_a & !0xFFFF) | (scaled as u64 & 0xFFFF);
            formation.apply_order(&adjusted, telemetry);
        }
        OrderKind::ArtilleryFire | OrderKind::FireControl => {
            if let (Some(b), Some(p), Some(t)) = (ballistics, fire_profile, terrain) {
                let grid_ptr = t as *const crate::terrain::TerrainGridCapsule
                    as *mut crate::terrain::TerrainGridCapsule;
                let (target_x_q16, target_z_q16) = unpack_fire_payload(order.payload_a);
                let (volley, fuse_ms, dispersion_mils, airburst) =
                    unpack_fire_meta_extended(order.payload_b);
                let shooter_snap = formation.snapshot();
                let shooter_tile = (
                    shooter_snap.position_x_q16 >> 16,
                    shooter_snap.position_z_q16 >> 16,
                );
                if let Some(gsnap) = garrisons.and_then(|g| g.find_by_formation(order.formation_id))
                {
                    if !within_aperture(
                        gsnap.aperture_deg_q16,
                        gsnap.aperture_width_deg_q16,
                        shooter_snap.position_x_q16,
                        shooter_snap.position_z_q16,
                        target_x_q16,
                        target_z_q16,
                    ) {
                        return;
                    }
                }
                let target_snap =
                    nearest_target_snapshot(target_x_q16, target_z_q16, shooter_id, formations);
                let target_aperture = target_snap.as_ref().and_then(|snap| {
                    garrisons
                        .and_then(|g| g.find_by_formation(snap.formation_id))
                        .map(|gsnap| {
                            let aperture_width_q16 = if gsnap.aperture_width_deg_q16 == 0 {
                                45u32 << 16
                            } else {
                                gsnap.aperture_width_deg_q16
                            };
                            ApertureMask {
                                aperture_deg_q16: gsnap.aperture_deg_q16,
                                aperture_width_q16,
                                target_x_q16: snap.position_x_q16,
                                target_z_q16: snap.position_z_q16,
                            }
                        })
                });
                if let Some(outcome) = crate::ballistics::apply_fire_control_for_formations(
                    &order,
                    shooter_tile,
                    t,
                    b,
                    structures,
                    telemetry,
                    p,
                    target_snap.as_ref(),
                    Some(&shooter_snap),
                    target_aperture,
                ) {
                    stats.artillery_ricochet_bounces = outcome.ricochet.bounces;
                    stats.artillery_crater_radius_tiles =
                        outcome.crater.as_ref().map(|c| c.radius_tiles).unwrap_or(0);
                    stats.artillery_fuse_ms = fuse_ms as u32;
                    let mut splash = outcome.expected_casualties as u64
                        + outcome.ricochet.expected_casualties as u64;
                    splash = splash
                        .saturating_add((volley as u64).max(1))
                        .saturating_add((dispersion_mils as u64) / 8);
                    if airburst {
                        splash = splash.saturating_add(256);
                    }
                    stats.artillery_splash_q16 =
                        (splash.saturating_mul(65_536) / 4_096).min(u32::MAX as u64) as u32;
                    if let Some(crater) = outcome.crater {
                        // Safety: tick runs shard-local; we intentionally mutate terrain in-place for artillery craters.
                        unsafe {
                            (*grid_ptr).apply_crater_q16(
                                crater.center.0,
                                crater.center.1,
                                crater.radius_tiles,
                                crater.cover_delta_q16,
                                crater.mud_delta_q16,
                            );
                        }
                    }
                }
            } else {
                formation.apply_order(&order, telemetry);
            }
        }
        OrderKind::SetFireDoctrine => {
            if let Some(doc) = fire_doctrine {
                let (mode, cadence) = unpack_fire_doctrine_payload(order.payload_a);
                doc.set_doctrine(order.formation_id, mode, cadence);
                stats.doctrine_sets = stats.doctrine_sets.saturating_add(1);
                stats.last_doctrine_mode = mode as u8;
                stats.last_doctrine_cadence_ticks = cadence;
            }
            formation.apply_order(&order, telemetry);
        }
        _ => formation.apply_order(&order, telemetry),
    }
}

/// Input bundle for world tick (multi-shard).
pub struct ShardContext<'a, const FB: usize> {
    pub shard_id: usize,
    pub orders: &'a OrderQueueCapsule,
    pub formations: &'a [FormationCapsule],
    pub pathings: &'a [PathingCapsule],
    pub telemetry: &'a TelemetryCapsule,
    pub formation_breaks: Option<&'a FormationBreakTelemetryCapsule<FB>>,
    pub ballistics: Option<&'a crate::ballistics::BallisticsCapsule>,
    pub fire_profile: Option<&'a crate::ballistics::FireControlProfileCapsule>,
    pub terrain: Option<&'a crate::terrain::TerrainGridCapsule>,
    pub grenades: Option<&'a crate::grenade::GrenadeCapsule>,
    pub structures: Option<&'a [StructureCapsule]>,
    pub garrisons: Option<&'a crate::garrison::GarrisonSlabCapsule>,
    pub supply: Option<&'a SupplySnapshot>,
    pub courier: Option<&'a CourierCapsule>,
    pub fire_doctrine: Option<&'a FireDoctrineCapsule>,
    pub battle_ai: Option<&'a crate::battle_ai::BattleAiCapsule>,
    pub fog: Option<&'a crate::fog::FogOfWarCapsule>,
    pub generals: Option<&'a [GeneralSnapshot]>,
    pub command_hierarchy: Option<&'a CommandHierarchyCapsule>,
    pub commanders: Option<&'a [CommanderSnapshot]>,
    pub strategic: Option<&'a StrategicSnapshot>,
    pub command_delays: Option<&'a crate::order::CommandDelayBufferCapsule>,
}

/// Convenience constructor for shard contexts so callers wire terrain/ballistics/profile consistently.
pub fn make_shard_context<'a, const FB: usize>(
    shard_id: usize,
    orders: &'a OrderQueueCapsule,
    formations: &'a [FormationCapsule],
    pathings: &'a [PathingCapsule],
    telemetry: &'a TelemetryCapsule,
    formation_breaks: Option<&'a FormationBreakTelemetryCapsule<FB>>,
    ballistics: Option<&'a crate::ballistics::BallisticsCapsule>,
    fire_profile: Option<&'a crate::ballistics::FireControlProfileCapsule>,
    terrain: Option<&'a crate::terrain::TerrainGridCapsule>,
    grenades: Option<&'a crate::grenade::GrenadeCapsule>,
    structures: Option<&'a [StructureCapsule]>,
    garrisons: Option<&'a crate::garrison::GarrisonSlabCapsule>,
    supply: Option<&'a SupplySnapshot>,
    courier: Option<&'a CourierCapsule>,
    fire_doctrine: Option<&'a FireDoctrineCapsule>,
    battle_ai: Option<&'a crate::battle_ai::BattleAiCapsule>,
    fog: Option<&'a crate::fog::FogOfWarCapsule>,
    generals: Option<&'a [GeneralSnapshot]>,
    command_hierarchy: Option<&'a CommandHierarchyCapsule>,
    commanders: Option<&'a [CommanderSnapshot]>,
    strategic: Option<&'a StrategicSnapshot>,
    command_delays: Option<&'a crate::order::CommandDelayBufferCapsule>,
) -> ShardContext<'a, FB> {
    ShardContext {
        shard_id,
        orders,
        formations,
        pathings,
        telemetry,
        formation_breaks,
        ballistics,
        fire_profile,
        terrain,
        grenades,
        structures,
        garrisons,
        supply,
        courier,
        fire_doctrine,
        battle_ai,
        fog,
        generals,
        command_hierarchy,
        commanders,
        strategic,
        command_delays,
    }
}

/// Run tick across all shards sequentially; returns per-shard stats.
pub fn tick_world<const FB: usize>(
    tick: u64,
    shards: &[ShardContext<'_, FB>],
) -> Vec<ShardTickStats> {
    let mut out = Vec::with_capacity(shards.len());
    for shard in shards {
        let stats = tick_shard(
            tick,
            shard.shard_id,
            shard.orders,
            shard.formations,
            shard.pathings,
            shard.telemetry,
            shard.formation_breaks,
            shard.ballistics,
            shard.fire_profile,
            shard.terrain,
            shard.grenades,
            shard.structures,
            shard.garrisons,
            shard.supply,
            shard.courier,
            shard.fire_doctrine,
            shard.battle_ai,
            shard.fog,
            shard.generals,
            shard.command_hierarchy,
            shard.commanders,
            shard.strategic,
            shard.command_delays,
        );
        out.push(stats);
    }
    out
}

/// Scheduler capsule to orchestrate ticks with world clock + deterministic RNG.
#[repr(C, align(128))]
pub struct SchedulerCapsule {
    clock: WorldClockCapsule,
    rng: DeterministicRngCapsule,
    last_tick: AtomicU64,
    _padding: [u8; 56],
}

verify_alignment_only!(SchedulerCapsule, 128);

impl SchedulerCapsule {
    pub const fn new(start_tick: u64, tick_ns: u64, rng_seed: u64, rng_stream: u64) -> Self {
        Self {
            clock: WorldClockCapsule::new(start_tick, tick_ns),
            rng: DeterministicRngCapsule::new(rng_seed, rng_stream),
            last_tick: AtomicU64::new(start_tick),
            _padding: [0; 56],
        }
    }

    pub fn run_tick<'a, const FB: usize>(
        &self,
        shards: &'a [ShardContext<'a, FB>],
        render_slab: &'a mut RenderSoaSlabCapsule,
    ) -> Result<(u64, Vec<ShardTickStats>, RenderSoaView<'a>, u64), RenderOverflow> {
        let tick = self.clock.advance();
        let (rng_head, _) = self.rng.next_u64();
        let stats = tick_world(tick, shards);
        render_slab.reset();
        for shard in shards {
            render_slab.add_shard(shard.formations)?;
        }
        let mut render = render_slab.view();
        // Inject supply averages into overlays if stats are available.
        for (ov, st) in render.overlays.iter_mut().zip(stats.iter()) {
            ov.supply_pressure_avg_q16 = st.supply_pressure_avg_q16;
            ov.supply_fatigue_penalty_avg_q16 = st.supply_fatigue_penalty_avg_q16;
            ov.province_infra_avg_q16 = st.province_infra_avg_q16;
            ov.province_resistance_avg_q16 = st.province_resistance_avg_q16;
            ov.command_stress_q16 = st.command_stress_q16;
            ov.courier_eta_ticks = st.courier_eta_ticks;
            ov.courier_losses = st.courier_losses;
            ov.courier_spoofed = st.courier_spoofed;
            ov.command_delay_applied = st.command_delay_applied;
            ov.command_delay_total_ticks = st.command_delay_total_ticks;
            ov.avg_garrison_aperture_width_q16 = st.avg_garrison_aperture_width_q16;
            ov.min_garrison_aperture_width_q16 = st.min_garrison_aperture_width_q16;
            ov.max_garrison_aperture_width_q16 = st.max_garrison_aperture_width_q16;
            ov.garrisoned = st.garrisoned;
            ov.structures_breached = st.structures_breached;
            ov.fog_visible_contacts = st.visible_contacts;
            ov.fog_visible_ratio_q16 = st.visible_ratio_q16;
            ov.grenade_casualties = st.grenade_casualties.min(u32::MAX as u64) as u32;
            ov.grenade_cover_q16 = st.grenade_cover_q16;
            ov.grenade_detonation_ms = st.grenade_detonation_ms;
            ov.artillery_ricochet_bounces = st.artillery_ricochet_bounces;
            ov.artillery_crater_radius_tiles = st.artillery_crater_radius_tiles;
            ov.artillery_fuse_ms = st.artillery_fuse_ms;
            ov.artillery_splash_q16 = st.artillery_splash_q16;
            ov.rank_fire_mask_or = st.rank_fire_mask_or;
            ov.rank_fire_events = st.rank_fire_events;
            ov.advance_fire_events = st.advance_fire_events;
            ov.last_doctrine_mode = st.last_doctrine_mode;
            ov.last_doctrine_cadence_ticks = st.last_doctrine_cadence_ticks;
            ov.doctrine_sets = st.doctrine_sets;
            ov.strategic_hash_chain = st.strategic_hash_chain;
            ov.strategic_prev_hash_chain = st.strategic_prev_hash_chain;
        }
        self.last_tick.store(tick, Ordering::Release);
        Ok((tick, stats, render, rng_head))
    }

    pub fn last_tick(&self) -> u64 {
        self.last_tick.load(Ordering::Acquire)
    }

    /// Reseed the internal RNG stream for the next tick (used by higher-level loops).
    pub fn reseed_rng(&self, seed: u64, stream: u64) {
        self.rng.reseed(seed, stream);
    }
}

/// Higher-level world loop capsule that orchestrates the scheduler with deterministic seeding.
#[repr(C, align(128))]
pub struct WorldLoopCapsule {
    scheduler: SchedulerCapsule,
    seed_base: AtomicU64,
    stream_base: AtomicU64,
    last_morale_shocks: AtomicU64,
    last_shock_weight_q16: AtomicU64,
    last_casualties: AtomicU64,
    last_charge_orders: AtomicU64,
    last_charge_commits: AtomicU64,
    last_brace_orders: AtomicU64,
    last_avg_damping_q16: AtomicU64,
    _padding: [u8; 16],
}

verify_alignment_only!(WorldLoopCapsule, 128);

impl WorldLoopCapsule {
    pub const fn new(start_tick: u64, tick_ns: u64, seed_base: u64, stream_base: u64) -> Self {
        Self {
            scheduler: SchedulerCapsule::new(start_tick, tick_ns, seed_base, stream_base),
            seed_base: AtomicU64::new(seed_base),
            stream_base: AtomicU64::new(stream_base),
            last_morale_shocks: AtomicU64::new(0),
            last_shock_weight_q16: AtomicU64::new(0),
            last_casualties: AtomicU64::new(0),
            last_charge_orders: AtomicU64::new(0),
            last_charge_commits: AtomicU64::new(0),
            last_brace_orders: AtomicU64::new(0),
            last_avg_damping_q16: AtomicU64::new(0),
            _padding: [0; 16],
        }
    }

    pub fn set_seed_base(&self, seed_base: u64) {
        self.seed_base.store(seed_base, Ordering::Release);
    }

    pub fn set_stream_base(&self, stream_base: u64) {
        self.stream_base.store(stream_base, Ordering::Release);
    }

    /// Run one frame with deterministic per-tick reseeding and return a frame bundle.
    pub fn run_frame<'a, const FB: usize>(
        &self,
        shards: &'a [ShardContext<'a, FB>],
        render_slab: &'a mut RenderSoaSlabCapsule,
    ) -> Result<WorldFrame<'a>, RenderOverflow> {
        let next_tick = self.scheduler.last_tick() + 1;
        let seed = self.seed_base.load(Ordering::Relaxed) ^ next_tick;
        let stream = self.stream_base.load(Ordering::Relaxed);
        self.scheduler.reseed_rng(seed, stream);
        let (tick, stats, render, rng_head) = self.scheduler.run_tick(shards, render_slab)?;
        let mut total_shocks = 0u64;
        let mut total_shock_weight_q16 = 0u64;
        let mut total_casualties = 0u64;
        let mut formation_count = 0usize;
        let mut total_fatigue_q16 = 0u64;
        let mut total_ammo = 0u64;
        let mut total_charge_orders = 0u64;
        let mut total_charge_commits = 0u64;
        let mut total_brace_orders = 0u64;
        let mut total_damping_q16 = 0u64;
        let mut supply_pressure_sum = 0u64;
        let mut supply_fatigue_sum = 0u64;
        let mut supply_samples = 0u64;
        let mut command_stress_sum_q16 = 0u64;
        let mut command_samples = 0u64;
        let mut courier_eta_sum = 0u64;
        let mut courier_samples = 0u64;
        let mut command_delay_hist = [0u64; COMMAND_HIST_BUCKETS];
        let mut courier_eta_hist = [0u64; COMMAND_HIST_BUCKETS];
        let mut command_delay_applied = 0u64;
        let mut command_delay_total_ticks = 0u64;
        let mut fog_visible_contacts = 0u64;
        let mut fog_visible_samples = 0u64;
        let mut courier_losses = 0u64;
        let mut courier_spoofed = 0u64;
        let mut artillery_ricochet_bounces = 0u64;
        let mut artillery_crater_radius_tiles = 0u64;
        let mut artillery_fuse_ms = 0u64;
        let mut artillery_splash_q16 = 0u64;
        let mut garrisoned = 0u64;
        let mut breached = 0u64;
        let mut aperture_sum_q16 = 0u64;
        let mut aperture_samples = 0u64;
        let mut aperture_min_q16 = u32::MAX;
        let mut aperture_max_q16 = 0u32;
        let mut grenade_casualties = 0u64;
        let mut grenade_cover_q16 = 0u32;
        let mut grenade_detonation_ms = 0u32;
        let mut last_charge_start_x_q16 = 0u32;
        let mut last_charge_start_z_q16 = 0u32;
        let mut last_charge_target_x_q16 = 0u32;
        let mut last_charge_target_z_q16 = 0u32;
        let mut last_charge_impact_mode = 0u8;
        let mut rank_fire_mask_or = 0u8;
        let mut rank_fire_events = 0u64;
        let mut advance_fire_events = 0u64;
        let mut doctrine_sets = 0u64;
        let mut last_doctrine_mode = 0u8;
        let mut last_doctrine_cadence_ticks = 0u16;
        let mut province_infra_sum_q16 = 0u64;
        let mut province_resistance_sum_q16 = 0u64;
        let mut province_samples = 0u64;
        let mut strategic_hash_chain = 0u64;
        let mut strategic_prev_hash_chain = 0u64;
        for (shard, shard_stats) in shards.iter().zip(stats.iter()) {
            let snap = shard.telemetry.snapshot();
            total_shocks = total_shocks.saturating_add(snap.morale_shocks);
            total_shock_weight_q16 = total_shock_weight_q16.saturating_add(snap.shock_weight_q16);
            total_casualties = total_casualties.saturating_add(snap.casualties);
            total_charge_orders = total_charge_orders.saturating_add(snap.charge_orders);
            total_charge_commits = total_charge_commits.saturating_add(snap.charge_commits);
            total_brace_orders = total_brace_orders.saturating_add(snap.brace_orders);
            supply_pressure_sum =
                supply_pressure_sum.saturating_add(snap.supply_pressure_accum_q16);
            supply_fatigue_sum = supply_fatigue_sum.saturating_add(snap.supply_fatigue_accum_q16);
            supply_samples = supply_samples.saturating_add(snap.supply_samples);
            if shard_stats.command_stress_q16 > 0 {
                command_stress_sum_q16 =
                    command_stress_sum_q16.saturating_add(shard_stats.command_stress_q16 as u64);
                command_samples = command_samples.saturating_add(1);
            }
            if shard_stats.courier_eta_ticks > 0 {
                courier_eta_sum =
                    courier_eta_sum.saturating_add(shard_stats.courier_eta_ticks as u64);
                courier_samples = courier_samples.saturating_add(1);
            }
            for (i, bucket) in shard_stats.command_delay_hist.iter().enumerate() {
                command_delay_hist[i] =
                    command_delay_hist[i].saturating_add(*bucket as u64);
            }
            for (i, bucket) in shard_stats.courier_eta_hist.iter().enumerate() {
                courier_eta_hist[i] = courier_eta_hist[i].saturating_add(*bucket as u64);
            }
            command_delay_applied =
                command_delay_applied.saturating_add(shard_stats.command_delay_applied as u64);
            command_delay_total_ticks = command_delay_total_ticks
                .saturating_add(shard_stats.command_delay_total_ticks as u64);
            fog_visible_contacts =
                fog_visible_contacts.saturating_add(shard_stats.visible_contacts as u64);
            fog_visible_samples =
                fog_visible_samples.saturating_add(shard_stats.visible_samples as u64);
            courier_losses = courier_losses.saturating_add(shard_stats.courier_losses as u64);
            courier_spoofed = courier_spoofed.saturating_add(shard_stats.courier_spoofed as u64);
            artillery_ricochet_bounces = artillery_ricochet_bounces
                .saturating_add(shard_stats.artillery_ricochet_bounces as u64);
            artillery_crater_radius_tiles =
                artillery_crater_radius_tiles.max(shard_stats.artillery_crater_radius_tiles as u64);
            artillery_fuse_ms = artillery_fuse_ms.max(shard_stats.artillery_fuse_ms as u64);
            artillery_splash_q16 =
                artillery_splash_q16.max(shard_stats.artillery_splash_q16 as u64);
            garrisoned = garrisoned.saturating_add(shard_stats.garrisoned as u64);
            breached = breached.saturating_add(shard_stats.structures_breached as u64);
            if shard_stats.avg_garrison_aperture_width_q16 > 0 {
                aperture_sum_q16 = aperture_sum_q16
                    .saturating_add(shard_stats.avg_garrison_aperture_width_q16 as u64);
                aperture_samples = aperture_samples.saturating_add(1);
            }
            if shard_stats.min_garrison_aperture_width_q16 > 0 {
                aperture_min_q16 =
                    aperture_min_q16.min(shard_stats.min_garrison_aperture_width_q16);
            }
            aperture_max_q16 = aperture_max_q16.max(shard_stats.max_garrison_aperture_width_q16);
            grenade_casualties =
                grenade_casualties.saturating_add(shard_stats.grenade_casualties as u64);
            if shard_stats.grenade_casualties > 0 {
                grenade_cover_q16 = shard_stats.grenade_cover_q16;
                grenade_detonation_ms = shard_stats.grenade_detonation_ms;
            }
            if shard_stats.last_charge_target_x_q16 != 0
                || shard_stats.last_charge_target_z_q16 != 0
                || shard_stats.last_charge_start_x_q16 != 0
                || shard_stats.last_charge_start_z_q16 != 0
                || shard_stats.last_charge_impact_mode != 0
            {
                last_charge_start_x_q16 = shard_stats.last_charge_start_x_q16;
                last_charge_start_z_q16 = shard_stats.last_charge_start_z_q16;
                last_charge_target_x_q16 = shard_stats.last_charge_target_x_q16;
                last_charge_target_z_q16 = shard_stats.last_charge_target_z_q16;
                last_charge_impact_mode = shard_stats.last_charge_impact_mode;
            }
            rank_fire_mask_or |= shard_stats.rank_fire_mask_or;
            rank_fire_events = rank_fire_events.saturating_add(shard_stats.rank_fire_events as u64);
            advance_fire_events =
                advance_fire_events.saturating_add(shard_stats.advance_fire_events as u64);
            doctrine_sets = doctrine_sets.saturating_add(shard_stats.doctrine_sets as u64);
            if shard_stats.last_doctrine_mode != 0 {
                last_doctrine_mode = shard_stats.last_doctrine_mode;
            }
            if shard_stats.last_doctrine_cadence_ticks != 0 {
                last_doctrine_cadence_ticks = shard_stats.last_doctrine_cadence_ticks;
            }
            if shard_stats.province_infra_avg_q16 > 0 {
                province_infra_sum_q16 = province_infra_sum_q16
                    .saturating_add(shard_stats.province_infra_avg_q16 as u64);
                province_resistance_sum_q16 = province_resistance_sum_q16
                    .saturating_add(shard_stats.province_resistance_avg_q16 as u64);
                province_samples = province_samples.saturating_add(1);
            }
            if shard_stats.strategic_hash_chain != 0 {
                strategic_hash_chain = shard_stats.strategic_hash_chain;
                strategic_prev_hash_chain = shard_stats.strategic_prev_hash_chain;
            }
            formation_count = formation_count.saturating_add(shard.formations.len());
            for formation in shard.formations.iter() {
                let snap = formation.snapshot();
                total_fatigue_q16 = total_fatigue_q16.saturating_add(snap.fatigue_q16 as u64);
                total_ammo = total_ammo.saturating_add(snap.ammo as u64);
                total_damping_q16 = total_damping_q16.saturating_add(snap.damping_q16 as u64);
            }
        }
        let shock_delta = total_shocks
            .saturating_sub(self.last_morale_shocks.swap(total_shocks, Ordering::AcqRel));
        let shock_weight_delta_q16 = total_shock_weight_q16.saturating_sub(
            self.last_shock_weight_q16
                .swap(total_shock_weight_q16, Ordering::AcqRel),
        );
        let casualty_delta = total_casualties.saturating_sub(
            self.last_casualties
                .swap(total_casualties, Ordering::AcqRel),
        );
        let charge_delta = total_charge_orders.saturating_sub(
            self.last_charge_orders
                .swap(total_charge_orders, Ordering::AcqRel),
        );
        let charge_commit_delta = total_charge_commits.saturating_sub(
            self.last_charge_commits
                .swap(total_charge_commits, Ordering::AcqRel),
        );
        let brace_delta = total_brace_orders.saturating_sub(
            self.last_brace_orders
                .swap(total_brace_orders, Ordering::AcqRel),
        );
        let avg_fatigue_q16 = if formation_count > 0 {
            (total_fatigue_q16 / formation_count as u64) as u32
        } else {
            0
        };
        let avg_ammo = if formation_count > 0 {
            (total_ammo / formation_count as u64) as u32
        } else {
            0
        };
        let avg_damping_q16 = if formation_count > 0 {
            (total_damping_q16 / formation_count as u64) as u32
        } else {
            0
        };
        let avg_supply_pressure_q16 = if supply_samples > 0 {
            (supply_pressure_sum / supply_samples) as u32
        } else {
            0
        };
        let avg_supply_fatigue_q16 = if supply_samples > 0 {
            (supply_fatigue_sum / supply_samples) as u32
        } else {
            0
        };
        let avg_province_infra_q16 = if province_samples > 0 {
            (province_infra_sum_q16 / province_samples)
                .min(u32::MAX as u64) as u32
        } else {
            0
        };
        let avg_province_resistance_q16 = if province_samples > 0 {
            (province_resistance_sum_q16 / province_samples)
                .min(u32::MAX as u64) as u32
        } else {
            0
        };
        let avg_command_stress_q16 = if command_samples > 0 {
            (command_stress_sum_q16 / command_samples).min(u32::MAX as u64) as u32
        } else {
            0
        };
        let avg_courier_eta_ticks = if courier_samples > 0 {
            (courier_eta_sum / courier_samples).min(u32::MAX as u64) as u32
        } else {
            0
        };
        let avg_command_delay_ticks = if command_delay_applied > 0 {
            (command_delay_total_ticks / command_delay_applied)
                .min(u32::MAX as u64) as u32
        } else {
            0
        };
        let fog_visible_ratio_q16 = if fog_visible_samples > 0 {
            (fog_visible_contacts.saturating_mul(65_536) / fog_visible_samples).min(u32::MAX as u64)
                as u32
        } else {
            0
        };
        let shock_penalty_q16 = compute_shock_penalty_q16(
            shock_delta,
            shock_weight_delta_q16,
            casualty_delta,
            formation_count,
            avg_fatigue_q16,
            avg_ammo,
        );
        self.last_avg_damping_q16
            .store(avg_damping_q16 as u64, Ordering::Release);
        let damping_scale_q16 = if avg_damping_q16 > 0 {
            let damp = avg_damping_q16.min(65_536) as u64;
            (65_536u64.saturating_sub(damp / 2)) as u32
        } else {
            65_536
        };
        let shock_penalty_q16 =
            ((shock_penalty_q16 as u64 * damping_scale_q16 as u64) / 65_536) as u32;
        if shock_penalty_q16 > 0 {
            let shock_weight_delta_q16_clamped = shock_weight_delta_q16.min(u32::MAX as u64) as u32;
            let casualty_term_q16 = (casualty_delta.min(2_000) as u32).saturating_mul(8);
            let cohesion_penalty_q16 = ((shock_penalty_q16 as u64 / 2)
                + shock_weight_delta_q16_clamped as u64 / 4
                + casualty_term_q16 as u64)
                .min(65_000) as u32;
            let fatigue_delta_q16 = ((shock_penalty_q16 as u64 / 4)
                + (shock_weight_delta_q16_clamped as u64 >> 6)
                + (casualty_delta.min(2_000) << 2))
                .min(40_000) as u32;
            for shard in shards {
                for formation in shard.formations.iter() {
                    formation.apply_shock_package(
                        shock_penalty_q16.min(60_000),
                        cohesion_penalty_q16,
                        fatigue_delta_q16,
                    );
                }
            }
        }
        let command_delay_hist_u32 =
            command_delay_hist.map(|v| v.min(u32::MAX as u64) as u32);
        let courier_eta_hist_u32 =
            courier_eta_hist.map(|v| v.min(u32::MAX as u64) as u32);
        Ok(WorldFrame {
            tick,
            stats,
            render,
            rng_head,
            seed_used: seed,
            shock_penalty_q16,
            casualty_delta,
            shock_weight_delta_q16,
            supply_pressure_avg_q16: avg_supply_pressure_q16,
            supply_fatigue_penalty_avg_q16: avg_supply_fatigue_q16,
            province_infra_avg_q16: avg_province_infra_q16,
            province_resistance_avg_q16: avg_province_resistance_q16,
            command_stress_avg_q16: avg_command_stress_q16,
            courier_eta_avg_ticks: avg_courier_eta_ticks,
            command_delay_hist: command_delay_hist_u32,
            courier_eta_hist: courier_eta_hist_u32,
            command_delay_applied: command_delay_applied.min(u32::MAX as u64) as u32,
            command_delay_avg_ticks: avg_command_delay_ticks,
            courier_losses,
            courier_spoofed,
            artillery_ricochet_bounces,
            artillery_crater_radius_tiles,
            artillery_fuse_ms,
            artillery_splash_q16,
            garrisoned,
            structures_breached: breached,
            avg_garrison_aperture_width_q16: if aperture_samples > 0 {
                (aperture_sum_q16 / aperture_samples).min(u32::MAX as u64) as u32
            } else {
                0
            },
            min_garrison_aperture_width_q16: if aperture_min_q16 == u32::MAX {
                0
            } else {
                aperture_min_q16
            },
            max_garrison_aperture_width_q16: aperture_max_q16,
            grenade_casualties,
            grenade_cover_q16,
            grenade_detonation_ms,
            charge_delta,
            charge_commit_delta,
            brace_delta,
            charge_path_start_x_tile: last_charge_start_x_q16 >> 16,
            charge_path_start_z_tile: last_charge_start_z_q16 >> 16,
            charge_path_target_x_tile: last_charge_target_x_q16 >> 16,
            charge_path_target_z_tile: last_charge_target_z_q16 >> 16,
            charge_impact_mode: last_charge_impact_mode,
            rank_fire_mask_or,
            rank_fire_events,
            advance_fire_events,
            doctrine_sets,
            last_doctrine_mode,
            last_doctrine_cadence_ticks,
            fog_visible_contacts: fog_visible_contacts.min(u32::MAX as u64) as u32,
            fog_visible_ratio_q16,
            strategic_hash_chain,
            strategic_prev_hash_chain,
        })
    }

    /// Run a frame and record shock/casualty metrics into a replay log capsule.
    pub fn run_frame_with_replay<'a, const FB: usize, const N: usize>(
        &self,
        shards: &'a [ShardContext<'a, FB>],
        render_slab: &'a mut RenderSoaSlabCapsule,
        replay_log: &crate::replay::ReplayLogCapsule<N>,
        strategic: Option<&StrategicSnapshot>,
    ) -> Result<WorldFrame<'a>, RenderOverflow> {
        let frame = self.run_frame(shards, render_slab)?;
        let payload = crate::replay::encode_shock_replay_payload(
            frame.casualty_delta as u32,
            frame.shock_penalty_q16,
            frame.shock_weight_delta_q16 as u32,
        );
        let _ = replay_log.record(frame.tick, payload);
        if frame.charge_delta > 0 || frame.brace_delta > 0 || frame.charge_commit_delta > 0 {
            let charge_payload = crate::replay::encode_charge_replay_payload(
                frame.charge_delta.min(u32::MAX as u64) as u32,
                frame.charge_commit_delta.min(u32::MAX as u64) as u32,
                frame.brace_delta.min(u32::MAX as u64) as u32,
            );
            let _ = replay_log.record(frame.tick, charge_payload);
        }
        // Record supply overlays for deterministic replays/analytics.
        if frame.supply_pressure_avg_q16 > 0 || frame.supply_fatigue_penalty_avg_q16 > 0 {
            let supply_payload = crate::replay::encode_supply_replay_payload(
                frame.supply_pressure_avg_q16,
                frame.supply_fatigue_penalty_avg_q16,
            );
            let _ = replay_log.record(frame.tick, supply_payload);
        }
        if frame.command_stress_avg_q16 > 0
            || frame.courier_eta_avg_ticks > 0
            || frame.courier_losses > 0
            || frame.courier_spoofed > 0
        {
            let payload = crate::replay::encode_command_replay_payload(
                frame.command_stress_avg_q16,
                frame.courier_eta_avg_ticks,
                frame.courier_losses.min(u32::MAX as u64) as u32,
                frame.courier_spoofed.min(u32::MAX as u64) as u32,
            );
            let _ = replay_log.record(frame.tick, payload);
        }
        if frame.command_delay_applied > 0 {
            let payload = crate::replay::encode_command_delay_applied(
                frame.command_delay_applied as u32,
                frame.command_delay_avg_ticks,
            );
            let _ = replay_log.record(frame.tick, payload);
        }
        if frame.command_delay_hist.iter().any(|&v| v > 0) {
            let chunk0 =
                crate::replay::encode_command_delay_hist_payload(0, &frame.command_delay_hist[0..4]);
            let chunk1 =
                crate::replay::encode_command_delay_hist_payload(1, &frame.command_delay_hist[4..8]);
            let _ = replay_log.record(frame.tick, chunk0);
            let _ = replay_log.record(frame.tick, chunk1);
        }
        if frame.courier_eta_hist.iter().any(|&v| v > 0) {
            let chunk0 =
                crate::replay::encode_courier_eta_hist_payload(0, &frame.courier_eta_hist[0..4]);
            let chunk1 =
                crate::replay::encode_courier_eta_hist_payload(1, &frame.courier_eta_hist[4..8]);
            let _ = replay_log.record(frame.tick, chunk0);
            let _ = replay_log.record(frame.tick, chunk1);
        }
        if frame.artillery_ricochet_bounces > 0
            || frame.artillery_crater_radius_tiles > 0
            || frame.artillery_fuse_ms > 0
            || frame.artillery_splash_q16 > 0
        {
            let payload = crate::replay::encode_artillery_replay_payload(
                frame.artillery_ricochet_bounces.min(u32::MAX as u64) as u32,
                frame.artillery_crater_radius_tiles.min(u32::MAX as u64) as u32,
                frame.artillery_fuse_ms.min(u32::MAX as u64) as u32,
                frame.artillery_splash_q16.min(u32::MAX as u64) as u32,
            );
            let _ = replay_log.record(frame.tick, payload);
        }
        if frame.garrisoned > 0 || frame.structures_breached > 0 {
            let payload = crate::replay::encode_garrison_replay_payload(
                frame.garrisoned.min(u32::MAX as u64) as u32,
                frame.structures_breached.min(u32::MAX as u64) as u32,
                frame.avg_garrison_aperture_width_q16,
            );
            let _ = replay_log.record(frame.tick, payload);
        }
        if frame.min_garrison_aperture_width_q16 > 0 || frame.max_garrison_aperture_width_q16 > 0 {
            let payload = crate::replay::encode_garrison_aperture_detail_payload(
                frame.min_garrison_aperture_width_q16,
                frame.max_garrison_aperture_width_q16,
            );
            let _ = replay_log.record(frame.tick, payload);
        }
        if frame.grenade_casualties > 0 {
            let payload = crate::replay::encode_grenade_replay_payload(
                frame.grenade_casualties.min(u32::MAX as u64) as u32,
                frame.grenade_cover_q16,
                frame.grenade_detonation_ms,
            );
            let _ = replay_log.record(frame.tick, payload);
        }
        if frame.rank_fire_mask_or > 0
            || frame.rank_fire_events > 0
            || frame.advance_fire_events > 0
            || frame.last_doctrine_mode > 0
            || frame.last_doctrine_cadence_ticks > 0
        {
            let payload = crate::replay::encode_doctrine_replay_payload(
                frame.rank_fire_mask_or,
                frame.last_doctrine_mode,
                frame.last_doctrine_cadence_ticks,
                frame.rank_fire_events.min(u16::MAX as u64) as u16,
                frame.advance_fire_events.min(u16::MAX as u64) as u16,
            );
            let _ = replay_log.record(frame.tick, payload);
        }
        if frame.charge_path_start_x_tile > 0
            || frame.charge_path_start_z_tile > 0
            || frame.charge_path_target_x_tile > 0
            || frame.charge_path_target_z_tile > 0
            || frame.charge_impact_mode > 0
        {
            let payload = crate::replay::encode_charge_path_replay_payload(
                frame.charge_path_start_x_tile,
                frame.charge_path_start_z_tile,
                frame.charge_path_target_x_tile,
                frame.charge_path_target_z_tile,
                frame.charge_impact_mode,
            );
            let _ = replay_log.record(frame.tick, payload);
        }
        for shard_stats in &frame.stats {
            if shard_stats.ai_replay_len == 0 {
                continue;
            }
            for payload in shard_stats
                .ai_replay_payloads
                .iter()
                .take(shard_stats.ai_replay_len as usize)
            {
                let _ = replay_log.record(frame.tick, *payload);
            }
        }
        if let Some(strat) = strategic {
            for ev in &strat.events {
                let payload = crate::replay::encode_strategic_event_payload(ev);
                let _ = replay_log.record(frame.tick, payload);
            }
        }
        Ok(frame)
    }

    /// Run a frame, log replay payloads, and flush to mmap with optional index chaining.
    pub fn run_frame_with_replay_flush<'a, const FB: usize, const N: usize>(
        &self,
        shards: &'a [ShardContext<'a, FB>],
        render_slab: &'a mut RenderSoaSlabCapsule,
        replay_log: &ReplayLogCapsule<N>,
        replay_flush: &ReplayFlushCapsule,
        replay_mmap: &mut ReplayMmapCapsule,
        replay_index: Option<&ReplayIndexCapsule>,
        strategic: Option<&StrategicSnapshot>,
    ) -> Result<(WorldFrame<'a>, crate::replay::ReplayPersistSnapshot), WorldPersistError> {
        let frame = self
            .run_frame_with_replay(shards, render_slab, replay_log, strategic)
            .map_err(WorldPersistError::from)?;
        if frame.charge_delta > 0 || frame.brace_delta > 0 || frame.charge_commit_delta > 0 {
            let payload = crate::replay::encode_charge_replay_payload(
                frame.charge_delta.min(u32::MAX as u64) as u32,
                frame.charge_commit_delta.min(u32::MAX as u64) as u32,
                frame.brace_delta.min(u32::MAX as u64) as u32,
            );
            let _ = replay_log.record(frame.tick, payload);
        }
        let snap = replay_flush
            .flush_to_mmap(replay_log, replay_mmap, replay_index)
            .map_err(WorldPersistError::from)?;
        Ok((frame, snap))
    }

    pub fn scheduler(&self) -> &SchedulerCapsule {
        &self.scheduler
    }
}

#[derive(Debug)]
pub struct WorldFrame<'a> {
    pub tick: u64,
    pub stats: Vec<ShardTickStats>,
    pub render: RenderSoaView<'a>,
    pub rng_head: u64,
    pub seed_used: u64,
    pub shock_penalty_q16: u32,
    pub casualty_delta: u64,
    pub shock_weight_delta_q16: u64,
    pub supply_pressure_avg_q16: u32,
    pub supply_fatigue_penalty_avg_q16: u32,
    pub province_infra_avg_q16: u32,
    pub province_resistance_avg_q16: u32,
    pub command_stress_avg_q16: u32,
    pub courier_eta_avg_ticks: u32,
    pub command_delay_hist: [u32; COMMAND_HIST_BUCKETS],
    pub courier_eta_hist: [u32; COMMAND_HIST_BUCKETS],
    pub command_delay_applied: u32,
    pub command_delay_avg_ticks: u32,
    pub courier_losses: u64,
    pub courier_spoofed: u64,
    pub artillery_ricochet_bounces: u64,
    pub artillery_crater_radius_tiles: u64,
    pub artillery_fuse_ms: u64,
    pub artillery_splash_q16: u64,
    pub garrisoned: u64,
    pub structures_breached: u64,
    pub avg_garrison_aperture_width_q16: u32,
    pub min_garrison_aperture_width_q16: u32,
    pub max_garrison_aperture_width_q16: u32,
    pub grenade_casualties: u64,
    pub grenade_cover_q16: u32,
    pub grenade_detonation_ms: u32,
    pub charge_delta: u64,
    pub charge_commit_delta: u64,
    pub brace_delta: u64,
    pub charge_path_start_x_tile: u32,
    pub charge_path_start_z_tile: u32,
    pub charge_path_target_x_tile: u32,
    pub charge_path_target_z_tile: u32,
    pub charge_impact_mode: u8,
    pub rank_fire_mask_or: u8,
    pub rank_fire_events: u64,
    pub advance_fire_events: u64,
    pub doctrine_sets: u64,
    pub last_doctrine_mode: u8,
    pub last_doctrine_cadence_ticks: u16,
    pub fog_visible_contacts: u32,
    pub fog_visible_ratio_q16: u32,
    pub strategic_hash_chain: u64,
    pub strategic_prev_hash_chain: u64,
}

// ---------------- Tests ----------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::{pack_move_payload, pack_retreat_meta, pack_retreat_payload};
    use crate::replay::ReplayLogCapsule;
    use crate::strategic_map::{ProvinceCapsule, StrategicMapCapsule};
    use crate::supply::SupplyCapsule;
    use crate::telemetry::TelemetryCapsule;
    use crate::courier::{CourierCapsule, Doctrine};
    use crate::command::{CommandHierarchyCapsule, CommanderCapsule};

    #[test]
    fn tick_shard_applies_orders_and_backsteps() {
        let telemetry = TelemetryCapsule::new();
        let orders = OrderQueueCapsule::new();
        let formations = vec![
            FormationCapsule::new(0, 0, 0, 10, 10, 50, 100, 90, 0, 0),
            FormationCapsule::new(1, 0, 0, 10, 10, 50, 100, 90, 0, 0),
        ];
        let pathings = vec![
            PathingCapsule::new(50, 0, 10),
            PathingCapsule::new(80, 0, 16),
        ];

        orders
            .push_order(OrderKind::Move, 0, pack_move_payload(10, 0), 0)
            .unwrap();
        orders
            .push_order(
                OrderKind::FallBack,
                1,
                pack_retreat_payload(40, 0),
                pack_retreat_meta(true, 0, 0),
            )
            .unwrap();

        let stats = tick_shard::<16>(
            0,
            0,
            &orders,
            &formations,
            &pathings,
            &telemetry,
            None, // formation_breaks
            None, // ballistics
            None, // fire_profile
            None, // terrain
            None, // grenades
            None, // structures
            None, // garrisons
            None, // supply
            None, // courier
            None, // fire_doctrine
            None, // battle_ai
            None, // fog
            None, // generals
            None, // command_hierarchy
            None, // commanders
            None, // strategic
            None, // command_delays
        );

        assert!(stats.processed_orders >= 1);
        assert_eq!(stats.retreats, 1);
        assert!(stats.moved >= 1);
    }

    #[test]
    fn supply_snapshot_applies_fatigue_and_ammo() {
        let telemetry = TelemetryCapsule::new();
        let orders = OrderQueueCapsule::new();
        let formations = vec![FormationCapsule::new(0, 0, 0, 10, 0, 50, 0, 0, 0, 0)];
        let pathings = vec![PathingCapsule::new(0, 0, 0)];
        let supply = SupplyCapsule::new(1);
        supply.inject_pressure(0, 40_000);
        let supply_snap = supply.step();

        let before = formations[0].snapshot();
        let _stats = tick_shard::<16>(
            0,
            0,
            &orders,
            &formations,
            &pathings,
            &telemetry,
            None, // formation_breaks
            None, // ballistics
            None, // fire_profile
            None, // terrain
            None, // grenades
            None, // structures
            None, // garrisons
            Some(&supply_snap), // supply
            None, // courier
            None, // fire_doctrine
            None, // battle_ai
            None, // fog
            None, // generals
            None, // command_hierarchy
            None, // commanders
            None, // strategic
            None, // command_delays
        );

        let after = formations[0].snapshot();
        assert!(after.ammo > before.ammo);
        assert!(after.fatigue_q16 > before.fatigue_q16);
    }

    #[test]
    fn command_out_of_range_increases_stress_and_eta() {
        let telemetry = TelemetryCapsule::new();
        let orders = OrderQueueCapsule::new();
        let courier = CourierCapsule::new(Doctrine::aggressive(), 6);
        let cmd = CommanderCapsule::new(0, 0, 20_000, 0, 0, 0, 8, true);
        let commander_snaps = [cmd.snapshot()];
        let hierarchy = CommandHierarchyCapsule::new(1);
        hierarchy.assign_commander(0, 0);

        let mk_stats = |pos_q16: u32| {
            let formations = vec![FormationCapsule::new(
                0, 0, 0, 10, 0, 50, 0, 0, pos_q16, pos_q16,
            )];
            let pathings = vec![PathingCapsule::new(0, 0, 0)];
            tick_shard::<16>(
                0,
                0,
                &orders,
                &formations,
                &pathings,
                &telemetry,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(&courier),
                None, // fire_doctrine
                None, // battle_ai
                None, // fog
                None, // generals
                Some(&hierarchy), // command_hierarchy
                Some(&commander_snaps), // commanders
                None, // strategic
                None, // command_delays
            )
        };

        let in_range = mk_stats(1_000);
        let out_range = mk_stats(90_000);
        let courier_base = courier.debug_snapshot();
        let expected_eta =
            courier_base.base_eta_ticks + courier_base.cadence_ticks.saturating_div(2);
        assert_eq!(in_range.courier_eta_ticks, expected_eta);
        assert!(out_range.courier_eta_ticks > expected_eta);
        assert!(out_range.command_stress_q16 > in_range.command_stress_q16);
        assert_eq!(in_range.command_delay_hist.iter().sum::<u32>(), 1);
        assert_eq!(out_range.command_delay_hist.iter().sum::<u32>(), 1);
        let in_eta_bucket = in_range
            .courier_eta_hist
            .iter()
            .position(|v| *v > 0)
            .unwrap();
        let out_eta_bucket = out_range
            .courier_eta_hist
            .iter()
            .position(|v| *v > 0)
            .unwrap();
        assert!(out_eta_bucket >= in_eta_bucket);
    }

    #[test]
    fn strategic_snapshot_propagates_stats() {
        let telemetry = TelemetryCapsule::new();
        let orders = OrderQueueCapsule::new();
        let formations = vec![FormationCapsule::new(0, 0, 0, 10, 0, 50, 0, 0, 0, 0)];
        let pathings = vec![PathingCapsule::new(0, 0, 0)];
        let mut map = StrategicMapCapsule::new(vec![ProvinceCapsule::new(0, 100_000, 40_000)], None, None);
        map.provinces()[0].set_depot_pressure(32_000);
        let strat_snap = map.step(0);

        let stats = tick_shard::<16>(
            0,
            0,
            &orders,
            &formations,
            &pathings,
            &telemetry,
            None, // formation_breaks
            None, // ballistics
            None, // fire_profile
            None, // terrain
            None, // grenades
            None, // structures
            None, // garrisons
            Some(&strat_snap.supply), // supply
            None, // courier
            None, // fire_doctrine
            None, // battle_ai
            None, // fog
            None, // generals
            None, // command_hierarchy
            None, // commanders
            Some(&strat_snap), // strategic
            None, // command_delays
        );

        assert_eq!(stats.strategic_hash_chain, strat_snap.hash_chain);
        assert!(stats.province_infra_avg_q16 > 0);
        assert!(stats.province_resistance_avg_q16 <= 65_536);
    }

    #[test]
    fn general_aura_boosts_morale_and_reduces_fatigue() {
        use crate::general::{snapshot_generals, GeneralCapsule};

        let telemetry = TelemetryCapsule::new();
        let orders = OrderQueueCapsule::new();
        let formations = vec![FormationCapsule::new(
            0, 0, 0, 40_000, 5_000, 50, 0, 10_000, 0, 0,
        )];
        let pathings = vec![PathingCapsule::new(0, 0, 0)];
        let generals = vec![GeneralCapsule::new(
            formations[0].snapshot().position_x_q16,
            formations[0].snapshot().position_z_q16,
            10_000,
            2_000,
            1_000,
            true,
        )];
        let general_snaps = snapshot_generals(&generals);
        let before = formations[0].snapshot();

        let _stats = tick_shard::<16>(
            0,
            0,
            &orders,
            &formations,
            &pathings,
            &telemetry,
            None, // formation_breaks
            None, // ballistics
            None, // fire_profile
            None, // terrain
            None, // grenades
            None, // structures
            None, // garrisons
            None, // supply
            None, // courier
            None, // fire_doctrine
            None, // battle_ai
            None, // fog
            Some(&general_snaps), // generals
            None, // command_hierarchy
            None, // commanders
            None, // strategic
            None, // command_delays
        );


        let after = formations[0].snapshot();
        assert!(after.morale_q16 > before.morale_q16);
        assert!(after.fatigue_q16 < before.fatigue_q16);
    }

    #[test]
    fn world_render_snapshot_combines_shards() {
        let f1 = FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let f2 = FormationCapsule::new(2, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let mut slab = RenderSoaSlabCapsule::new(1, 2);
        let shard_a = [f1];
        let shard_b = [f2];
        let shard_slices = [&shard_a[..], &shard_b[..]];
        let view = collect_world_render_slab(&shard_slices, &mut slab).unwrap();
        assert_eq!(view.total_len, 2);
        assert_eq!(view.shard_offsets, &[(0, 1), (1, 1)]);
        assert_eq!(view.pages.first().unwrap().formation_ids, &[1, 2]);
        assert_eq!(view.overlays.len(), 2);
        assert_eq!(view.overlays[0].lod_stride, 1);
    }

    #[test]
    fn scheduler_runs_and_records_render() {
        let telemetry = TelemetryCapsule::new();
        let orders = OrderQueueCapsule::new();
        let formations = vec![FormationCapsule::new(0, 0, 0, 10, 10, 50, 100, 90, 0, 0)];
        let pathings = vec![PathingCapsule::new(8, 0, 8)];
        let shard_ctx: [ShardContext<'_, 16>; 1] = [ShardContext {
            shard_id: 0,
            orders: &orders,
            formations: &formations,
            pathings: &pathings,
            telemetry: &telemetry,
            formation_breaks: None,
            ballistics: None,
            fire_profile: None,
            terrain: None,
            grenades: None,
            structures: None,
            garrisons: None,
            supply: None,
            courier: None,
            fire_doctrine: None,
            battle_ai: None,
            fog: None,
            generals: None,
            command_hierarchy: None,
            commanders: None,
            strategic: None,
            command_delays: None,
        }];
        let scheduler = SchedulerCapsule::new(0, 16_666_667, 1234, 1);
        let mut slab = RenderSoaSlabCapsule::new(1, 1);
        let (_tick, stats, render, _rng_head) = scheduler.run_tick(&shard_ctx, &mut slab).unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(render.total_len, 1);
        assert_eq!(scheduler.last_tick(), 1);
    }

    #[test]
    fn nearest_target_prefers_closest_non_shooter() {
        let formations = vec![
            FormationCapsule::new(0, 0, 0, 10, 10, 50, 100, 90, 0, 0),
            FormationCapsule::new(1, 0, 0, 10, 10, 50, 100, 90, 40 << 16, 0),
            FormationCapsule::new(2, 0, 0, 10, 10, 50, 100, 90, 120 << 16, 0),
        ];
        let target = nearest_target_snapshot(50 << 16, 0, 0, &formations).unwrap();
        assert_eq!(target.formation_id, 1);
    }

    #[test]
    fn multi_shard_replay_determinism() {
        let telemetry_a = TelemetryCapsule::new();
        let telemetry_b = TelemetryCapsule::new();
        let orders_a = OrderQueueCapsule::new();
        let orders_b = OrderQueueCapsule::new();
        let formations_a = vec![FormationCapsule::new(0, 0, 0, 10, 10, 50, 100, 90, 0, 0)];
        let formations_b = vec![FormationCapsule::new(0, 0, 0, 10, 10, 50, 100, 90, 0, 0)];
        let pathings_a = vec![PathingCapsule::new(16, 0, 8)];
        let pathings_b = vec![PathingCapsule::new(16, 0, 8)];

        let rng_seed = 0xDEADBEEF;
        let make_orders = |orders: &OrderQueueCapsule| {
            let rng = crate::DeterministicRngCapsule::new(rng_seed, 1);
            for idx in 0..3 {
                let (val, _) = rng.next_u64();
                let target = (val as u32) % 64;
                orders
                    .push_order(
                        OrderKind::Move,
                        0,
                        crate::order::pack_move_payload(target + idx, 0),
                        0,
                    )
                    .unwrap();
            }
        };
        make_orders(&orders_a);
        make_orders(&orders_b);

        let shard_ctx_a: [ShardContext<'_, 16>; 1] = [ShardContext {
            shard_id: 0,
            orders: &orders_a,
            formations: &formations_a,
            pathings: &pathings_a,
            telemetry: &telemetry_a,
            formation_breaks: None,
            ballistics: None,
            fire_profile: None,
            terrain: None,
            grenades: None,
            structures: None,
            garrisons: None,
            supply: None,
            courier: None,
            fire_doctrine: None,
            battle_ai: None,
            fog: None,
            generals: None,
            command_hierarchy: None,
            commanders: None,
            strategic: None,
            command_delays: None,
        }];
        let shard_ctx_b: [ShardContext<'_, 16>; 1] = [ShardContext {
            shard_id: 0,
            orders: &orders_b,
            formations: &formations_b,
            pathings: &pathings_b,
            telemetry: &telemetry_b,
            formation_breaks: None,
            ballistics: None,
            fire_profile: None,
            terrain: None,
            grenades: None,
            structures: None,
            garrisons: None,
            supply: None,
            courier: None,
            fire_doctrine: None,
            battle_ai: None,
            fog: None,
            generals: None,
            command_hierarchy: None,
            commanders: None,
            strategic: None,
            command_delays: None,
        }];

        let scheduler_a = SchedulerCapsule::new(0, 16_666_667, rng_seed, 1);
        let scheduler_b = SchedulerCapsule::new(0, 16_666_667, rng_seed, 1);
        let mut slab_a = RenderSoaSlabCapsule::new(1, 1);
        let mut slab_b = RenderSoaSlabCapsule::new(1, 1);

        let (tick_a, stats_a, render_a, rng_a) =
            scheduler_a.run_tick(&shard_ctx_a, &mut slab_a).unwrap();
        let (tick_b, stats_b, render_b, rng_b) =
            scheduler_b.run_tick(&shard_ctx_b, &mut slab_b).unwrap();

        assert_eq!(tick_a, tick_b);
        assert_eq!(rng_a, rng_b);
        assert_eq!(stats_a, stats_b);
        assert_eq!(render_a.total_len, render_b.total_len);

        let log_a: ReplayLogCapsule<8> = ReplayLogCapsule::new();
        let log_b: ReplayLogCapsule<8> = ReplayLogCapsule::new();
        for (page_idx, page) in render_a.pages.iter().enumerate() {
            for (i, pos) in page.position_x_q16.iter().enumerate() {
                log_a.record(
                    tick_a,
                    (page_idx * RENDER_PAGE_SIZE + i) as u64 ^ (*pos as u64),
                );
            }
        }
        for (page_idx, page) in render_b.pages.iter().enumerate() {
            for (i, pos) in page.position_x_q16.iter().enumerate() {
                log_b.record(
                    tick_b,
                    (page_idx * RENDER_PAGE_SIZE + i) as u64 ^ (*pos as u64),
                );
            }
        }
        let events_a = log_a.drain();
        let events_b = log_b.drain();
        assert_eq!(events_a.len(), events_b.len());
        for (a, b) in events_a.iter().zip(events_b.iter()) {
            assert_eq!(a.payload, b.payload);
        }
    }

    #[test]
    fn render_iter_strides_and_shard_spans() {
        let f1 = FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let f2 = FormationCapsule::new(2, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let mut slab = RenderSoaSlabCapsule::new(1, 2);
        let shard_a = [f1];
        let shard_b = [f2];
        let shard_slices = [&shard_a[..], &shard_b[..]];
        let view = collect_world_render_slab(&shard_slices, &mut slab).unwrap();
        assert_eq!(view.shard_span(0), Some((0, 1)));
        assert_eq!(view.shard_span(1), Some((1, 1)));
        let ids: Vec<u32> = view.iter_strided(1).map(|e| e.formation_id).collect();
        assert_eq!(ids, vec![1, 2]);
        let shard_ids: Vec<u32> = view
            .iter_shard(1, 1)
            .unwrap()
            .map(|e| e.formation_id)
            .collect();
        assert_eq!(shard_ids, vec![2]);
        assert_eq!(view.overlays.len(), 2);
    }

    #[test]
    fn world_loop_reseeds_and_advances() {
        let telemetry = TelemetryCapsule::new();
        let orders = OrderQueueCapsule::new();
        let formations = vec![FormationCapsule::new(0, 0, 0, 10, 10, 50, 100, 90, 0, 0)];
        let pathings = vec![PathingCapsule::new(8, 0, 8)];
        let shard_ctx: [ShardContext<'_, 16>; 1] = [ShardContext {
            shard_id: 0,
            orders: &orders,
            formations: &formations,
            pathings: &pathings,
            telemetry: &telemetry,
            formation_breaks: None,
            ballistics: None,
            fire_profile: None,
            terrain: None,
            grenades: None,
            structures: None,
            garrisons: None,
            supply: None,
            courier: None,
            fire_doctrine: None,
            battle_ai: None,
            fog: None,
            generals: None,
            command_hierarchy: None,
            commanders: None,
            strategic: None,
            command_delays: None,
        }];
        let loop_capsule = WorldLoopCapsule::new(0, 16_666_667, 0xDEAD, 7);
        let mut slab = RenderSoaSlabCapsule::new(1, 1);
        let frame = loop_capsule.run_frame(&shard_ctx, &mut slab).unwrap();
        let seed0 = frame.seed_used;
        let tick0 = frame.tick;
        let stats_len0 = frame.stats.len();
        let render_len0 = frame.render.total_len;
        drop(frame);
        assert_eq!(tick0, 1);
        assert_eq!(stats_len0, 1);
        assert_eq!(render_len0, 1);
        // Derive a new seed when tick advances.
        let frame2 = loop_capsule.run_frame(&shard_ctx, &mut slab).unwrap();
        assert!(frame2.seed_used != seed0);
        assert_eq!(loop_capsule.scheduler().last_tick(), 2);
    }

    #[test]
    fn world_loop_applies_shock_decay_to_morale() {
        let telemetry = TelemetryCapsule::new();
        // Seed a casualty shock before the first frame runs.
        telemetry.log_casualty_shock(40);
        let orders = OrderQueueCapsule::new();
        let formations = vec![FormationCapsule::new(
            0, 0, 0, 10, 10, 50_000, 100, 90, 0, 0,
        )];
        let pathings = vec![PathingCapsule::new(8, 0, 8)];
        let shard_ctx: [ShardContext<'_, 16>; 1] = [ShardContext {
            shard_id: 0,
            orders: &orders,
            formations: &formations,
            pathings: &pathings,
            telemetry: &telemetry,
            formation_breaks: None,
            ballistics: None,
            fire_profile: None,
            terrain: None,
            grenades: None,
            structures: None,
            garrisons: None,
            supply: None,
            courier: None,
            fire_doctrine: None,
            battle_ai: None,
            fog: None,
            generals: None,
            command_hierarchy: None,
            commanders: None,
            strategic: None,
            command_delays: None,
        }];
        let loop_capsule = WorldLoopCapsule::new(0, 16_666_667, 0xABCD, 3);
        let mut slab = RenderSoaSlabCapsule::new(1, 1);
        let morale_before = formations[0].snapshot().morale_q16;
        loop_capsule.run_frame(&shard_ctx, &mut slab).unwrap();
        let morale_after = formations[0].snapshot().morale_q16;
        assert!(morale_after < morale_before);
    }

    #[test]
    fn shock_penalty_scales_with_casualties_and_weight() {
        let base = compute_shock_penalty_q16(10, 0, 10, 10, 20_000, 1_000);
        let more_cas = compute_shock_penalty_q16(10, 0, 100, 10, 20_000, 1_000);
        let more_weight = compute_shock_penalty_q16(10, 50_000, 10, 10, 20_000, 1_000);
        assert!(more_cas > base);
        assert!(more_weight > base);
        // High ammo dampens fear.
        let high_ammo = compute_shock_penalty_q16(10, 50_000, 100, 10, 20_000, 20_000);
        assert!(high_ammo < more_weight);
    }

    #[test]
    fn world_loop_flushes_replay_to_mmap() {
        use crate::replay::{
            ReplayFlushCapsule, ReplayIndexCapsule, ReplayLogCapsule, ReplayMmapCapsule,
        };
        use std::path::Path;

        let telemetry = TelemetryCapsule::new();
        let orders = OrderQueueCapsule::new();
        let formations = vec![FormationCapsule::new(0, 0, 0, 10, 10, 50, 100, 90, 0, 0)];
        let pathings = vec![PathingCapsule::new(8, 0, 8)];
        let shard_ctx: [ShardContext<'_, 16>; 1] = [ShardContext {
            shard_id: 0,
            orders: &orders,
            formations: &formations,
            pathings: &pathings,
            telemetry: &telemetry,
            formation_breaks: None,
            ballistics: None,
            fire_profile: None,
            terrain: None,
            grenades: None,
            structures: None,
            garrisons: None,
            supply: None,
            courier: None,
            fire_doctrine: None,
            battle_ai: None,
            fog: None,
            generals: None,
            command_hierarchy: None,
            commanders: None,
            strategic: None,
            command_delays: None,
        }];
        let loop_capsule = WorldLoopCapsule::new(0, 16_666_667, 0xBEEF, 9);
        let mut slab = RenderSoaSlabCapsule::new(1, 1);

        let replay_log: ReplayLogCapsule<8> = ReplayLogCapsule::new();
        let replay_flush = ReplayFlushCapsule::new();
        let tmp_path = std::env::temp_dir().join("kindly_engine_world_flush.bin");
        let mut replay_mmap = ReplayMmapCapsule::new(&tmp_path, 1_048_576, 1).expect("create mmap");
        let replay_index = ReplayIndexCapsule::new();

        let (_frame, snap) = loop_capsule
            .run_frame_with_replay_flush(
                &shard_ctx,
                &mut slab,
                &replay_log,
                &replay_flush,
                &mut replay_mmap,
                Some(&replay_index),
                None,
            )
            .expect("frame + flush");
        assert!(snap.flushed_events >= 1);
        let _ = std::fs::remove_file(Path::new(&tmp_path));
    }

    #[test]
    fn world_persistence_capsule_runs_and_persists_snapshot() {
        use crate::replay::{
            ReplayFlushCapsule, ReplayIndexCapsule, ReplayLogCapsule, ReplayMmapCapsule,
        };
        use crate::snapshot::{CampaignSnapshotCapsule, SnapshotMmapCapsule};
        use std::path::Path;

        let telemetry = TelemetryCapsule::new();
        let orders = OrderQueueCapsule::new();
        let formations = vec![FormationCapsule::new(0, 0, 0, 10, 10, 50, 100, 90, 0, 0)];
        let pathings = vec![PathingCapsule::new(8, 0, 8)];
        let shard_ctx: [ShardContext<'_, 16>; 1] = [ShardContext {
            shard_id: 0,
            orders: &orders,
            formations: &formations,
            pathings: &pathings,
            telemetry: &telemetry,
            formation_breaks: None,
            ballistics: None,
            fire_profile: None,
            terrain: None,
            grenades: None,
            structures: None,
            garrisons: None,
            supply: None,
            courier: None,
            fire_doctrine: None,
            battle_ai: None,
            fog: None,
            generals: None,
            command_hierarchy: None,
            commanders: None,
            strategic: None,
            command_delays: None,
        }];
        let loop_capsule = WorldLoopCapsule::new(0, 16_666_667, 0xCAFE, 11);
        let mut slab = RenderSoaSlabCapsule::new(1, 1);

        let replay_log: ReplayLogCapsule<8> = ReplayLogCapsule::new();
        let replay_flush = ReplayFlushCapsule::new();
        let tmp_replay = std::env::temp_dir().join("kindly_engine_world_persist_replay.bin");
        let mut replay_mmap =
            ReplayMmapCapsule::new(&tmp_replay, 1_048_576, 1).expect("create replay mmap");
        let replay_index = ReplayIndexCapsule::new();

        let snapshot_capsule = CampaignSnapshotCapsule::new();
        let tmp_snap = std::env::temp_dir().join("kindly_engine_world_persist_snapshot.bin");
        let mut snapshot_mmap =
            SnapshotMmapCapsule::new(&tmp_snap, 1_048_576, 1).expect("create snapshot mmap");

        let persister = WorldPersistenceCapsule::new();
        let (_frame, replay_snap, chain) = persister
            .run_and_persist_frame(
                &loop_capsule,
                &shard_ctx,
                &mut slab,
                &replay_log,
                &replay_flush,
                &mut replay_mmap,
                Some(&replay_index),
                &snapshot_capsule,
                &mut snapshot_mmap,
                &formations,
                &[],
                &orders,
                &telemetry,
                None,
                None,
                None,
                None,
            )
            .expect("persist frame");
        assert!(replay_snap.flushed_events >= 1);
        assert_ne!(chain, 0);
        let _ = std::fs::remove_file(Path::new(&tmp_replay));
        let _ = std::fs::remove_file(Path::new(&tmp_snap));
    }

    #[test]
    fn volley_weight_increases_shock_penalty() {
        // Higher artillery shock weight (e.g., from a larger volley) should drive a higher penalty.
        let low = compute_shock_penalty_q16(
            1,      // shock_delta
            12_000, // shock_weight_delta_q16 (small volley)
            10,     // casualty_delta
            8,      // formation_count
            12_000, // avg_fatigue_q16
            5_000,  // avg_ammo
        );
        let high = compute_shock_penalty_q16(
            1,      // shock_delta
            64_000, // shock_weight_delta_q16 (larger volley bonus)
            10,     // casualty_delta
            8,      // formation_count
            12_000, // avg_fatigue_q16
            5_000,  // avg_ammo
        );
        assert!(high > low);
    }
}
