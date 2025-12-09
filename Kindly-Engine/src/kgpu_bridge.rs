use atomic_capsule::verify_alignment_only;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::terrain::TerrainOverlayView;
use crate::tick::RenderSoaView;

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
use atomic_capsule::gpu::kgpu_driver::{GpuPlatform, KgpuDriverError, LinuxGpuPlatformCapsule};

/// Zero-copy view handed to kgpu ingestion.
pub struct KgpuRenderSlice<'a> {
    pub view: &'a RenderSoaView<'a>,
}

pub struct RenderSnapshot<'a> {
    pub frame_id: u64,
    pub tick: u64,
    pub view: RenderSoaView<'a>,
    pub terrain: Option<TerrainOverlaySnapshot<'a>>,
}

pub struct TerrainOverlaySnapshot<'a> {
    pub width: u32,
    pub height: u32,
    pub cover_strip: &'a [AtomicU32],
    pub cost_strip: &'a [AtomicU32],
    pub lod2: Option<TerrainLodSnapshot<'a>>,
    pub lod4: Option<TerrainLodSnapshot<'a>>,
}

pub struct TerrainLodSnapshot<'a> {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub cover: &'a [u32],
    pub mud: &'a [u32],
}

/// Shard overlay view for kgpu (LOD stride + morale min/max).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KgpuShardOverlay {
    pub shard_id: u32,
    pub start: u32,
    pub len: u32,
    pub lod_stride: u32,
    pub morale_min_q16: u32,
    pub morale_max_q16: u32,
    pub charging: u32,
    pub braced: u32,
    pub garrisoned: u32,
    pub structures_breached: u32,
    /// Q16.16 density of charging units (charging / len)
    pub charge_density_q16: u32,
    /// Q16.16 density of braced units (braced / len)
    pub brace_density_q16: u32,
    /// Average garrison aperture width for occupied slots (Q16.16 degrees)
    pub avg_garrison_aperture_width_q16: u32,
    /// Minimum garrison aperture width observed this tick (Q16.16 degrees)
    pub min_garrison_aperture_width_q16: u32,
    /// Maximum garrison aperture width observed this tick (Q16.16 degrees)
    pub max_garrison_aperture_width_q16: u32,
    /// Average formation density for the shard (Q16.16)
    pub avg_density_q16: u32,
    /// Average formation variance/aim noise for the shard (Q16.16)
    pub avg_variance_q16: u32,
    /// Average gap-close scale (Q16.16), higher = tighter ranks
    pub avg_gap_close_q16: u32,
    /// Average rank-variance scale (Q16.16), lower = tighter volleys
    pub avg_rank_variance_q16: u32,
    /// Average fatigue penalty derived from gap-close posture (Q16.16)
    pub avg_gap_fatigue_penalty_q16: u32,
    /// Average supply pressure for the shard (Q16.16)
    pub supply_pressure_avg_q16: u32,
    /// Average supply-induced fatigue penalty for the shard (Q16.16)
    pub supply_fatigue_penalty_avg_q16: u32,
    /// Average delivered throughput for the shard (Q16.16)
    pub supply_throughput_avg_q16: u32,
    /// Disruption events observed on supply lines (clamped)
    pub supply_disruptions: u32,
    /// Attrition events observed on supply lines (clamped)
    pub supply_attrition_events: u32,
    /// Detected supply route cuts (disruption clusters)
    pub supply_route_cuts: u32,
    /// Average command-delay penalty induced by logistics (ticks)
    pub supply_command_delay_penalty_avg_ticks: u32,
    /// Command graph stress (order queue depth normalized, Q16.16)
    pub command_stress_q16: u32,
    /// Courier latency baseline (ticks)
    pub courier_eta_ticks: u32,
    /// Courier losses recorded (clamped)
    pub courier_losses: u32,
    /// Courier spoofed count (clamped)
    pub courier_spoofed: u32,
    /// Grenade expected casualties (clamped to u32)
    pub grenade_casualties: u32,
    /// Grenade average cover (Q16.16)
    pub grenade_cover_q16: u32,
    /// Grenade detonation time (ms)
    pub grenade_detonation_ms: u32,
    /// Ricochet bounce count from last artillery volley
    pub artillery_ricochet_bounces: u32,
    /// Crater radius (tiles) from last artillery volley
    pub artillery_crater_radius_tiles: u32,
    /// Fuse (ms) used for last artillery volley
    pub artillery_fuse_ms: u32,
    /// Splash intensity proxy (Q16.16 casualties scale)
    pub artillery_splash_q16: u32,
    /// Bitmask of ranks that fired recently (bit0 = front rank)
    pub rank_fire_mask_or: u32,
    /// Count of rank-fire events in last tick
    pub rank_fire_events: u32,
    /// Count of advance-and-fire events in last tick
    pub advance_fire_events: u32,
    /// Last doctrine mode observed (enum discriminant)
    pub last_doctrine_mode: u32,
    /// Cadence (ticks) of last doctrine update
    pub last_doctrine_cadence_ticks: u32,
    /// Doctrine change events applied in last tick
    pub doctrine_sets: u32,
    /// Courier reliability derived from deliveries/losses (Q16.16)
    pub courier_reliability_q16: u32,
    /// Ops backpressure from order rate (Q16.16 above unity = pressure)
    pub ops_backpressure_q16: u32,
    /// Count of orders dropped due to backpressure caps (clamped)
    pub ops_backpressure_drops: u32,
    /// Congestion bucket (0=idle, 7=severe queue congestion)
    pub ops_congestion_bucket: u32,
    /// P95 bucket index for command delay histogram (0..7)
    pub command_delay_p95_bucket: u32,
    /// P95 bucket index for courier ETA histogram (0..7)
    pub courier_eta_p95_bucket: u32,
    /// Threat pressure derived from fog-of-war contacts (Q16.16)
    pub threat_pressure_q16: u32,
    /// Count of visible contacts contributing to threat pressure
    pub fog_visible_contacts: u32,
    /// Sample ratio of visible contacts (Q16.16)
    pub fog_visible_ratio_q16: u32,
    /// Battle AI threat centroid (tiles, truncated)
    pub ai_threat_x_tile: u32,
    pub ai_threat_z_tile: u32,
    /// Dominant stance and doctrine mode selected by AI
    pub ai_dominant_stance: u32,
    pub ai_doctrine_mode: u32,
    /// Low byte of AI generation for intent snapshot
    pub ai_generation_lsb: u32,
}

/// Shader-side note (pseudo kgpu fragment):
///
/// ```glsl
/// // Inputs: overlay.charge_density_q16 / brace_density_q16 (0..65535 = 0..1)
/// float charge = overlay.charge_density_q16 / 65535.0;
/// float brace  = overlay.brace_density_q16  / 65535.0;
/// // Map to channels: R = charge, G = brace, B = morale heat
/// vec3 color = vec3(charge, brace, morale_heat);
/// out_color = vec4(color, 1.0);
/// ```
/// This keeps charge corridors distinct (red), braced lines (green), and your existing morale LOD in blue.
/// Debug overlays can also pull `avg_density_q16` / `avg_variance_q16` to render physics heatmaps.
///
/// Doctrine overlay hint:
/// ```glsl
/// float rank_mask = overlay.rank_fire_mask_or / 255.0;
/// float rank_events = overlay.rank_fire_events / 255.0;
/// float advance = overlay.advance_fire_events / 255.0;
/// float cadence = overlay.last_doctrine_cadence_ticks / 65535.0;
/// // R=rank mask density, G=rank fire cadence, B=advance-and-fire density, A=cadence
/// vec3 color = vec3(rank_mask, rank_events, advance);
/// out_color = vec4(color, cadence);
/// ```
///
/// Supply overlay hint:
/// ```glsl
/// float supply = overlay.supply_pressure_avg_q16 / 65535.0;
/// float fatigue_pen = overlay.supply_fatigue_penalty_avg_q16 / 65535.0;
/// // e.g., encode supply as cyan (pressure) and fatigue penalty as alpha
/// vec3 color = vec3(supply, supply, fatigue_pen);
/// out_color = vec4(color, 1.0);
/// ```
/// Heatmap suggestion: map supply pressure to green, fatigue penalty to red, morale to blue for a
/// composite logistic/discipline view.
///
/// Artillery debug overlay hint:
/// ```glsl
/// float ricochet = overlay.artillery_ricochet_bounces / 8.0;
/// float crater = overlay.artillery_crater_radius_tiles / 8.0;
/// float splash = overlay.artillery_splash_q16 / 65535.0;
/// float fuse = overlay.artillery_fuse_ms / 4000.0;
/// // Map fuse/splash to heatmap channels if desired:
/// vec3 color = vec3(splash, ricochet, fuse);
/// ```
///
/// Grenade overlay hint:
/// ```glsl
/// float g_cas = overlay.grenade_casualties / 255.0;
/// float g_cover = overlay.grenade_cover_q16 / 65535.0;
/// float g_time = overlay.grenade_detonation_ms / 4000.0;
/// vec3 g_color = vec3(g_cas, g_cover, g_time);
/// ```
///
/// Garrison aperture overlay hint:
/// ```glsl
/// float avg_ap = overlay.avg_garrison_aperture_width_q16 / 65535.0;
/// float min_ap = overlay.min_garrison_aperture_width_q16 / 65535.0;
/// float max_ap = overlay.max_garrison_aperture_width_q16 / 65535.0;
/// // Example: R = min aperture, G = max aperture, B = avg
/// vec3 ap_color = vec3(min_ap, max_ap, avg_ap);
/// ```

pub struct KgpuOverlayView<'a> {
    overlays: &'a [crate::tick::ShardOverlay],
}

impl<'a> KgpuOverlayView<'a> {
    pub fn iter(&self) -> core::slice::Iter<'_, crate::tick::ShardOverlay> {
        self.overlays.iter()
    }

    /// Supply heatmap channels normalized to 0.0-1.0 for renderer consumption.
    pub fn supply_heatmap_channels(&self) -> impl Iterator<Item = (u32, f32, f32)> + '_ {
        self.overlays.iter().map(|o| {
            (
                o.shard_id as u32,
                o.supply_pressure_avg_q16 as f32 / 65_535.0,
                o.supply_fatigue_penalty_avg_q16 as f32 / 65_535.0,
            )
        })
    }

    /// Doctrine/rank-fire overlay channels normalized for kgpu.
    pub fn doctrine_overlay_channels(&self) -> impl Iterator<Item = DoctrineOverlayChannel> + '_ {
        self.overlays.iter().map(|o| {
            let rank_mask_norm = (o.rank_fire_mask_or as f32).min(255.0) / 255.0;
            let rank_fire_events_norm = (o.rank_fire_events as f32).min(255.0) / 255.0;
            let advance_fire_events_norm = (o.advance_fire_events as f32).min(255.0) / 255.0;
            let cadence_norm = (o.last_doctrine_cadence_ticks as f32).min(65_535.0) / 65_535.0;
            DoctrineOverlayChannel {
                shard_id: o.shard_id as u32,
                rank_mask_norm,
                rank_fire_events_norm,
                advance_fire_events_norm,
                cadence_norm,
                doctrine_mode: o.last_doctrine_mode as u8,
            }
        })
    }

    /// Garrison aperture overlay channels (avg/min/max) normalized to 0..1 for shader consumption.
    pub fn aperture_overlay_channels(&self) -> impl Iterator<Item = (u32, f32, f32, f32)> + '_ {
        self.overlays.iter().map(|o| {
            let avg = o.avg_garrison_aperture_width_q16 as f32 / 65_535.0;
            let min = o.min_garrison_aperture_width_q16 as f32 / 65_535.0;
            let max = o.max_garrison_aperture_width_q16 as f32 / 65_535.0;
            (o.shard_id as u32, avg, min, max)
        })
    }
}

/// Supply heatmap frame for kgpu ingestion (tick-stamped).
pub struct SupplyHeatmapFrame {
    pub tick: u64,
    pub source_version: u64,
    pub channels: Vec<(u32, f32, f32)>,
}

/// Doctrine/rank-fire overlay channel (normalized for renderer shaders).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DoctrineOverlayChannel {
    pub shard_id: u32,
    /// Bitmask density of ranks that fired (0..1)
    pub rank_mask_norm: f32,
    /// Rank-fire events density (0..1)
    pub rank_fire_events_norm: f32,
    /// Advance-and-fire events density (0..1)
    pub advance_fire_events_norm: f32,
    /// Cadence normalized to u16 range (0..1)
    pub cadence_norm: f32,
    /// Doctrine mode discriminant (for palette lookup)
    pub doctrine_mode: u8,
}

/// Doctrine overlay frame for kgpu ingestion.
pub struct DoctrineOverlayFrame {
    pub tick: u64,
    pub source_version: u64,
    pub channels: Vec<DoctrineOverlayChannel>,
}

/// Sink interface for doctrine overlays (renderer or terminal/debug).
pub trait DoctrineOverlaySink {
    fn submit(&self, frame: &DoctrineOverlayFrame);
}

/// Optional legend for renderer/overlay mapping.
#[derive(Clone, Copy)]
pub struct SupplyHeatmapLegend {
    pub supply_label: &'static str,
    pub fatigue_label: &'static str,
}

impl Default for SupplyHeatmapLegend {
    fn default() -> Self {
        Self {
            supply_label: "supply_pressure",
            fatigue_label: "supply_fatigue_penalty",
        }
    }
}

/// Build a supply heatmap frame from a render overlay snapshot.
pub fn make_supply_heatmap(snapshot: &RenderOverlaySnapshot) -> SupplyHeatmapFrame {
    SupplyHeatmapFrame {
        tick: snapshot.tick,
        source_version: snapshot.version,
        channels: snapshot.view().supply_heatmap_channels().collect(),
    }
}

/// Convenience: build a supply heatmap frame directly from a render snapshot (drops terrain refs).
pub fn make_supply_heatmap_from_render(snapshot: &RenderSnapshot<'_>) -> SupplyHeatmapFrame {
    SupplyHeatmapFrame {
        tick: snapshot.tick,
        source_version: snapshot.frame_id,
        channels: KgpuOverlayView {
            overlays: &snapshot.view.overlays,
        }
        .supply_heatmap_channels()
        .collect(),
    }
}

/// Build doctrine/rank-fire overlay frame from overlay snapshot.
pub fn make_doctrine_overlay(snapshot: &RenderOverlaySnapshot) -> DoctrineOverlayFrame {
    DoctrineOverlayFrame {
        tick: snapshot.tick,
        source_version: snapshot.version,
        channels: snapshot.view().doctrine_overlay_channels().collect(),
    }
}

/// Convenience: build doctrine overlay frame directly from a render snapshot.
pub fn make_doctrine_overlay_from_render(snapshot: &RenderSnapshot<'_>) -> DoctrineOverlayFrame {
    DoctrineOverlayFrame {
        tick: snapshot.tick,
        source_version: snapshot.frame_id,
        channels: KgpuOverlayView {
            overlays: &snapshot.view.overlays,
        }
        .doctrine_overlay_channels()
        .collect(),
    }
}

/// Sink interface for supply/fatigue heatmaps (renderer or terminal/debug).
pub trait SupplyHeatmapSink {
    fn submit(&self, frame: &SupplyHeatmapFrame);
}

/// Terminal/debug sink: counts frames and optionally logs supply/fatigue ranges.
#[repr(C, align(64))]
pub struct TerminalHeatmapSink {
    count: core::sync::atomic::AtomicU64,
}

impl TerminalHeatmapSink {
    pub const fn new() -> Self {
        Self {
            count: core::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn count(&self) -> u64 {
        self.count.load(core::sync::atomic::Ordering::Acquire)
    }
}

impl SupplyHeatmapSink for TerminalHeatmapSink {
    fn submit(&self, frame: &SupplyHeatmapFrame) {
        let _ = self
            .count
            .fetch_add(1, core::sync::atomic::Ordering::AcqRel);
        if let Some((supply_max, fatigue_max)) =
            frame
                .channels
                .iter()
                .fold(None, |acc: Option<(f32, f32)>, (_, s, f)| {
                    let (mut s_max, mut f_max) = acc.unwrap_or((0.0f32, 0.0f32));
                    if *s > s_max {
                        s_max = *s;
                    }
                    if *f > f_max {
                        f_max = *f;
                    }
                    Some((s_max, f_max))
                })
        {
            println!(
                "supply heatmap tick {} frames {}: max_supply {:.3}, max_fatigue {:.3}",
                frame.tick,
                self.count(),
                supply_max,
                fatigue_max
            );
        }
    }
}

/// Terminal/debug sink for doctrine overlays.
#[repr(C, align(64))]
pub struct TerminalDoctrineSink {
    count: core::sync::atomic::AtomicU64,
}

impl TerminalDoctrineSink {
    pub const fn new() -> Self {
        Self {
            count: core::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn count(&self) -> u64 {
        self.count.load(core::sync::atomic::Ordering::Acquire)
    }
}

impl DoctrineOverlaySink for TerminalDoctrineSink {
    fn submit(&self, frame: &DoctrineOverlayFrame) {
        let _ = self
            .count
            .fetch_add(1, core::sync::atomic::Ordering::AcqRel);
        if let Some(max_mask) = frame
            .channels
            .iter()
            .map(|c| c.rank_mask_norm)
            .max_by(|a, b| a.total_cmp(b))
        {
            println!(
                "doctrine overlay tick {} frames {}: max_rank_mask {:.3}",
                frame.tick,
                self.count(),
                max_mask
            );
        }
    }
}

/// kgpu-backed sink: submit supply/fatigue overlays to the renderer when available.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub struct KgpuDriverHeatmapSink<'a> {
    pub session: &'a KgpuDriverSession,
    pub legend: SupplyHeatmapLegend,
    pub enabled: bool,
}

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
impl<'a> KgpuDriverHeatmapSink<'a> {
    pub fn from_session(
        session: &'a KgpuDriverSession,
        legend: SupplyHeatmapLegend,
        enabled: bool,
    ) -> Self {
        Self {
            session,
            legend,
            enabled,
        }
    }
}

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
impl<'a> SupplyHeatmapSink for KgpuDriverHeatmapSink<'a> {
    fn submit(&self, frame: &SupplyHeatmapFrame) {
        if !self.enabled {
            return;
        }
        let _ = self.session.submit_supply_heatmap(frame, &self.legend);
    }
}

/// Stub sink for non-kgpu builds.
#[cfg(not(all(feature = "kgpu-driver-linux", target_os = "linux")))]
pub struct KgpuDriverHeatmapSink<'a> {
    pub _phantom: core::marker::PhantomData<&'a ()>,
}

#[cfg(not(all(feature = "kgpu-driver-linux", target_os = "linux")))]
impl<'a> KgpuDriverHeatmapSink<'a> {
    pub fn new_disabled() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn from_session(
        _session: &'a KgpuDriverSession,
        _legend: SupplyHeatmapLegend,
        _enabled: bool,
    ) -> Self {
        Self::new_disabled()
    }
}

#[cfg(not(all(feature = "kgpu-driver-linux", target_os = "linux")))]
impl<'a> SupplyHeatmapSink for KgpuDriverHeatmapSink<'a> {
    fn submit(&self, _frame: &SupplyHeatmapFrame) {
        // No-op without kgpu driver support.
    }
}

/// Overlay snapshot handed to kgpu (versioned, tick-stamped).
pub struct RenderOverlaySnapshot {
    pub version: u64,
    pub tick: u64,
    pub overlays: Vec<crate::tick::ShardOverlay>,
}

/// Aperture overlay frame for renderer/debug consumers.
pub struct ApertureOverlaySample {
    pub shard_id: u32,
    pub avg: f32,
    pub min: f32,
    pub max: f32,
}

pub struct ApertureOverlayFrame {
    pub tick: u64,
    pub version: u64,
    pub samples: Vec<ApertureOverlaySample>,
}

/// Threat overlay sample per shard for dashboards/heatmaps.
pub struct ThreatOverlaySample {
    pub shard_id: u32,
    pub threat_pressure_q16: u32,
    pub fog_visible_contacts: u32,
    pub fog_visible_ratio_q16: u32,
}

pub struct ThreatOverlayFrame {
    pub tick: u64,
    pub version: u64,
    pub samples: Vec<ThreatOverlaySample>,
}

impl RenderOverlaySnapshot {
    pub fn view(&self) -> KgpuOverlayView<'_> {
        KgpuOverlayView {
            overlays: &self.overlays,
        }
    }
}

/// Overlay publication capsule: versioned, lock-free handoff of LOD/morale overlays.
#[repr(C, align(64))]
pub struct RenderOverlayCapsule {
    version: AtomicU64,
}

verify_alignment_only!(RenderOverlayCapsule, 64);

impl RenderOverlayCapsule {
    pub const fn new() -> Self {
        Self {
            version: AtomicU64::new(0),
        }
    }

    pub fn publish(&self, render: &RenderSoaView<'_>) -> (u64, Vec<crate::tick::ShardOverlay>) {
        let v = self.version.fetch_add(1, Ordering::AcqRel) + 1;
        (v, render.overlays.clone())
    }
}

/// Build an aperture overlay frame (avg/min/max per shard) from a render snapshot.
pub fn make_aperture_overlay_from_render(snapshot: &RenderSnapshot<'_>) -> ApertureOverlayFrame {
    let view = snapshot.view.overlays.iter();
    let samples: Vec<ApertureOverlaySample> = view
        .map(|o| ApertureOverlaySample {
            shard_id: o.shard_id as u32,
            avg: o.avg_garrison_aperture_width_q16 as f32 / 65_535.0,
            min: o.min_garrison_aperture_width_q16 as f32 / 65_535.0,
            max: o.max_garrison_aperture_width_q16 as f32 / 65_535.0,
        })
        .collect();
    ApertureOverlayFrame {
        tick: snapshot.tick,
        version: snapshot.frame_id,
        samples,
    }
}

/// Build a threat overlay frame (fog-derived pressure) from a render snapshot.
pub fn make_threat_overlay_from_render(snapshot: &RenderSnapshot<'_>) -> ThreatOverlayFrame {
    let samples: Vec<ThreatOverlaySample> = snapshot
        .view
        .overlays
        .iter()
        .map(|o| ThreatOverlaySample {
            shard_id: o.shard_id as u32,
            threat_pressure_q16: o.threat_pressure_q16,
            fog_visible_contacts: o.fog_visible_contacts,
            fog_visible_ratio_q16: o.fog_visible_ratio_q16,
        })
        .collect();
    ThreatOverlayFrame {
        tick: snapshot.tick,
        version: snapshot.frame_id,
        samples,
    }
}

/// Minimal terminal ingest capsule for kgpu: tracks frame ids and hands out immutable slices.
#[repr(C, align(64))]
pub struct KgpuTerminalCapsule {
    frame_id: core::sync::atomic::AtomicU64,
}

verify_alignment_only!(KgpuTerminalCapsule, 64);

impl KgpuTerminalCapsule {
    pub const fn new() -> Self {
        Self {
            frame_id: core::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Ingest a render view without copying; returns frame id and slice wrapper.
    pub fn ingest<'a>(&self, render: &'a RenderSoaView<'a>) -> (u64, KgpuRenderSlice<'a>) {
        let id = self
            .frame_id
            .fetch_add(1, core::sync::atomic::Ordering::AcqRel);
        (id + 1, KgpuRenderSlice { view: render })
    }

    /// Ingest with world clock tick, returning a snapshot wrapper.
    pub fn ingest_with_clock<'a>(
        &self,
        render: RenderSoaView<'a>,
        tick: u64,
    ) -> RenderSnapshot<'a> {
        let (frame_id, _) = self.ingest(&render);
        RenderSnapshot {
            frame_id,
            tick,
            view: render,
            terrain: None,
        }
    }

    /// Expose shard offsets for LOD consumers (start, len) per shard.
    pub fn shard_offsets<'a>(&self, render: &'a RenderSoaView<'a>) -> &'a [(usize, usize)] {
        render.shard_offsets
    }

    /// Expose shard overlays (LOD stride + morale extrema) without copying.
    pub fn overlay_view<'a>(&self, render: &'a RenderSoaView<'a>) -> KgpuOverlayView<'a> {
        let _ = self; // symmetry with other ingest helpers
        KgpuOverlayView {
            overlays: &render.overlays,
        }
    }

    /// Ingest with terrain overlays for renderer (cover/cost/LOD).
    pub fn ingest_with_overlays<'a>(
        &self,
        render: RenderSoaView<'a>,
        tick: u64,
        terrain: TerrainOverlayView<'a>,
    ) -> RenderSnapshot<'a> {
        let mut snap = self.ingest_with_clock(render, tick);
        snap.terrain = Some(TerrainOverlaySnapshot::from_view(terrain));
        snap
    }

    /// Publish overlays to kgpu with versioning (no copies).
    pub fn ingest_overlays(
        &self,
        overlay_capsule: &RenderOverlayCapsule,
        render: &RenderSoaView<'_>,
        tick: u64,
    ) -> RenderOverlaySnapshot {
        let (version, overlays) = overlay_capsule.publish(render);
        RenderOverlaySnapshot {
            version,
            tick,
            overlays,
        }
    }
}

impl<'a> TerrainOverlaySnapshot<'a> {
    fn from_view(view: TerrainOverlayView<'a>) -> Self {
        Self {
            width: view.width,
            height: view.height,
            cover_strip: view.cover_strip,
            cost_strip: view.cost_strip,
            lod2: view.lod2.map(TerrainLodSnapshot::from_view),
            lod4: view.lod4.map(TerrainLodSnapshot::from_view),
        }
    }
}

impl<'a> TerrainLodSnapshot<'a> {
    fn from_view(view: crate::terrain::TerrainLodView<'a>) -> Self {
        Self {
            width: view.width,
            height: view.height,
            stride: view.stride,
            cover: view.cover,
            mud: view.mud,
        }
    }
}

/// Best-effort hook into the real kgpu driver stack (feature-gated).
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn try_submit_with_kgpu_driver(
    snapshot: &RenderSnapshot<'_>,
) -> Result<(), atomic_capsule::gpu::kgpu_driver::KgpuDriverError> {
    // Enumerate and open the first device; actual render submission is platform-specific and
    // should replace this handshake in the real renderer integration.
    let devices = LinuxGpuPlatformCapsule::enumerate_devices()?;
    let handle = LinuxGpuPlatformCapsule::open_device(0)?;
    // Placeholder: ingest the render snapshot here once the renderer pipeline is wired.
    let _ = snapshot.frame_id;
    let _ = snapshot.view.total_len;
    LinuxGpuPlatformCapsule::close_device(handle)?;
    // Basic sanity: ensure at least one device was visible.
    if devices.is_empty() {
        return Err(atomic_capsule::gpu::kgpu_driver::KgpuDriverError::NoDevice);
    }
    Ok(())
}

/// No-op stub for non-kgpu builds.
#[cfg(not(all(feature = "kgpu-driver-linux", target_os = "linux")))]
pub fn try_submit_with_kgpu_driver(_snapshot: &RenderSnapshot<'_>) -> Result<(), ()> {
    Ok(())
}

/// Persistent kgpu-driver session so we don't reopen per frame.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub struct KgpuDriverSession {
    handle: LinuxGpuPlatformCapsule::DeviceHandle,
}

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
impl KgpuDriverSession {
    pub fn new(
        device_index: usize,
    ) -> Result<(Self, Vec<atomic_capsule::gpu::kgpu_driver::GpuDeviceInfo>), KgpuDriverError> {
        let devices = LinuxGpuPlatformCapsule::enumerate_devices()?;
        let handle = LinuxGpuPlatformCapsule::open_device(device_index)?;
        Ok((Self { handle }, devices))
    }

    pub fn submit(&self, snapshot: &RenderSnapshot<'_>) -> Result<(), KgpuDriverError> {
        // Replace with real renderer ingest; keep deterministic no-op for now.
        let _ = snapshot.view.total_len;
        let _ = snapshot.frame_id;
        Ok(())
    }

    /// Submit a supply/fatigue heatmap frame to the kgpu renderer (placeholder hook).
    pub fn submit_supply_heatmap(
        &self,
        frame: &SupplyHeatmapFrame,
        legend: &SupplyHeatmapLegend,
    ) -> Result<(), KgpuDriverError> {
        let _ = frame.tick;
        let _ = frame.channels.len();
        let _ = legend.supply_label;
        // Real implementation should upload channels into a kgpu overlay buffer.
        Ok(())
    }

    /// Submit doctrine/rank-fire overlays to kgpu renderer (placeholder hook).
    pub fn submit_doctrine_overlay(
        &self,
        frame: &DoctrineOverlayFrame,
    ) -> Result<(), KgpuDriverError> {
        let _ = frame.tick;
        let _ = frame.channels.len();
        Ok(())
    }
}

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
impl Drop for KgpuDriverSession {
    fn drop(&mut self) {
        let _ = LinuxGpuPlatformCapsule::close_device(self.handle);
    }
}

/// Stub for non-kgpu builds.
#[cfg(not(all(feature = "kgpu-driver-linux", target_os = "linux")))]
pub struct KgpuDriverSession;

#[cfg(not(all(feature = "kgpu-driver-linux", target_os = "linux")))]
impl KgpuDriverSession {
    pub fn new(_device_index: usize) -> Result<(Self, Vec<()>), ()> {
        Ok((Self, Vec::new()))
    }
    pub fn submit(&self, _snapshot: &RenderSnapshot<'_>) -> Result<(), ()> {
        Ok(())
    }

    pub fn submit_supply_heatmap(
        &self,
        _frame: &SupplyHeatmapFrame,
        _legend: &SupplyHeatmapLegend,
    ) -> Result<(), ()> {
        Ok(())
    }

    pub fn submit_doctrine_overlay(&self, _frame: &DoctrineOverlayFrame) -> Result<(), ()> {
        Ok(())
    }
}

impl<'a> KgpuOverlayView<'a> {
    pub fn overlays(&self) -> impl Iterator<Item = KgpuShardOverlay> + '_ {
        self.overlays.iter().map(|o| KgpuShardOverlay {
            shard_id: o.shard_id as u32,
            start: o.start as u32,
            len: o.len as u32,
            lod_stride: o.lod_stride as u32,
            morale_min_q16: o.morale_min_q16,
            morale_max_q16: o.morale_max_q16,
            charging: o.charging,
            braced: o.braced,
            garrisoned: o.garrisoned,
            structures_breached: o.structures_breached,
            charge_density_q16: if o.len > 0 {
                ((o.charging as u64 * 65_536) / o.len as u64).min(u32::MAX as u64) as u32
            } else {
                0
            },
            brace_density_q16: if o.len > 0 {
                ((o.braced as u64 * 65_536) / o.len as u64).min(u32::MAX as u64) as u32
            } else {
                0
            },
            avg_density_q16: o.avg_density_q16,
            avg_variance_q16: o.avg_variance_q16,
            avg_gap_close_q16: o.avg_gap_close_q16,
            avg_rank_variance_q16: o.avg_rank_variance_q16,
            avg_gap_fatigue_penalty_q16: o.avg_gap_fatigue_penalty_q16,
            supply_pressure_avg_q16: o.supply_pressure_avg_q16,
            supply_fatigue_penalty_avg_q16: o.supply_fatigue_penalty_avg_q16,
            supply_throughput_avg_q16: o.supply_throughput_avg_q16,
            supply_disruptions: o.supply_disruptions,
            supply_attrition_events: o.supply_attrition_events,
            supply_route_cuts: o.supply_route_cuts,
            supply_command_delay_penalty_avg_ticks: o.supply_command_delay_penalty_avg_ticks,
            command_stress_q16: o.command_stress_q16,
            courier_eta_ticks: o.courier_eta_ticks,
            courier_losses: o.courier_losses,
            courier_spoofed: o.courier_spoofed,
            avg_garrison_aperture_width_q16: o.avg_garrison_aperture_width_q16,
            min_garrison_aperture_width_q16: o.min_garrison_aperture_width_q16,
            max_garrison_aperture_width_q16: o.max_garrison_aperture_width_q16,
            grenade_casualties: o.grenade_casualties,
            grenade_cover_q16: o.grenade_cover_q16,
            grenade_detonation_ms: o.grenade_detonation_ms,
            artillery_ricochet_bounces: o.artillery_ricochet_bounces,
            artillery_crater_radius_tiles: o.artillery_crater_radius_tiles,
            artillery_fuse_ms: o.artillery_fuse_ms,
            artillery_splash_q16: o.artillery_splash_q16,
            rank_fire_mask_or: o.rank_fire_mask_or as u32,
            rank_fire_events: o.rank_fire_events,
            advance_fire_events: o.advance_fire_events,
            last_doctrine_mode: o.last_doctrine_mode as u32,
            last_doctrine_cadence_ticks: o.last_doctrine_cadence_ticks as u32,
            doctrine_sets: o.doctrine_sets,
            courier_reliability_q16: o.courier_reliability_q16,
            ops_backpressure_q16: o.ops_backpressure_q16,
            ops_backpressure_drops: o.ops_backpressure_drops,
            ops_congestion_bucket: o.ops_congestion_bucket as u32,
            command_delay_p95_bucket: o.command_delay_p95_bucket as u32,
            courier_eta_p95_bucket: o.courier_eta_p95_bucket as u32,
            threat_pressure_q16: o.threat_pressure_q16,
            fog_visible_contacts: o.fog_visible_contacts,
            fog_visible_ratio_q16: o.fog_visible_ratio_q16,
            ai_threat_x_tile: o.ai_threat_x_tile as u32,
            ai_threat_z_tile: o.ai_threat_z_tile as u32,
            ai_dominant_stance: o.ai_dominant_stance as u32,
            ai_doctrine_mode: o.ai_doctrine_mode as u32,
            ai_generation_lsb: o.ai_generation_lsb as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formation::FormationCapsule;
    use crate::terrain::{TerrainGridCapsule, TerrainSnapshot};
    use crate::tick::{collect_world_render_slab, RenderSoaSlabCapsule};

    #[test]
    fn ingest_increments_frames() {
        let mut slab = RenderSoaSlabCapsule::new(1, 1);
        let f = FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let shard = [f];
        let shards = [&shard[..]];
        let view = collect_world_render_slab(&shards, &mut slab).unwrap();
        let kgpu = KgpuTerminalCapsule::new();
        let (frame_a, slice_a) = kgpu.ingest(&view);
        let (frame_b, _slice_b) = kgpu.ingest(&view);
        assert_eq!(frame_a, 1);
        assert_eq!(frame_b, 2);
        assert_eq!(slice_a.view.total_len, 1);
    }

    #[test]
    fn ingest_with_clock_records_tick() {
        let mut slab = RenderSoaSlabCapsule::new(1, 1);
        let f = FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let shard = [f];
        let shards = [&shard[..]];
        let view = collect_world_render_slab(&shards, &mut slab).unwrap();
        let kgpu = KgpuTerminalCapsule::new();
        let snap = kgpu.ingest_with_clock(view, 42);
        assert_eq!(snap.tick, 42);
        assert_eq!(snap.frame_id, 1);
        assert_eq!(snap.view.shard_offsets.len(), 1);
        assert_eq!(snap.view.overlays.len(), 1);
        assert!(snap.terrain.is_none());
    }

    #[test]
    fn ingest_with_overlays_exposes_terrain() {
        let grid = TerrainGridCapsule::new(
            2,
            2,
            TerrainSnapshot {
                height_mm: 0,
                slope_q16: 0,
                cover_q16: 10,
                mud_q16: 20,
                material: 0,
            },
        );
        let mut slab = RenderSoaSlabCapsule::new(1, 1);
        let f = FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        let shard = [f];
        let shards = [&shard[..]];
        let view = collect_world_render_slab(&shards, &mut slab).unwrap();
        let mut grid_mut = grid;
        grid_mut.rebuild_lod_masks();
        let overlay = grid_mut.overlay_view();
        let kgpu = KgpuTerminalCapsule::new();
        let snap = kgpu.ingest_with_overlays(view, 7, overlay);
        let terrain = snap.terrain.unwrap();
        assert_eq!(terrain.width, 2);
        assert!(terrain.lod2.is_some());
    }

    #[test]
    fn overlay_view_exposes_morale_and_stride() {
        let mut slab = RenderSoaSlabCapsule::new(1, 2);
        let f1 = FormationCapsule::new(1, 0, 0, 0, 0, 10_000, 0, 0, 0, 0);
        let f2 = FormationCapsule::new(2, 0, 0, 0, 0, 20_000, 0, 0, 0, 0);
        let shard_a = [f1];
        let shard_b = [f2];
        let shards = [&shard_a[..], &shard_b[..]];
        let view = collect_world_render_slab(&shards, &mut slab).unwrap();
        let kgpu = KgpuTerminalCapsule::new();
        let overlay_view = kgpu.overlay_view(&view);
        let overlays: Vec<_> = overlay_view.overlays().collect();
        assert_eq!(overlays.len(), 2);
        assert_eq!(overlays[0].shard_id, 0);
        assert_eq!(overlays[1].shard_id, 1);
        assert!(overlays[0].morale_min_q16 <= overlays[0].morale_max_q16);
        assert!(overlays[1].lod_stride >= 1);
        assert_eq!(overlays[0].charge_density_q16, 0);
        assert_eq!(overlays[1].brace_density_q16, 0);
    }

    #[test]
    fn render_overlay_capsule_versions_and_exposes_overlays() {
        let mut slab = RenderSoaSlabCapsule::new(1, 1);
        let f = FormationCapsule::new(1, 0, 0, 0, 0, 10_000, 0, 0, 0, 0);
        let shard = [f];
        let shards = [&shard[..]];
        let view = collect_world_render_slab(&shards, &mut slab).unwrap();
        let overlay_capsule = RenderOverlayCapsule::new();
        let (ver_a, overlay_vec) = overlay_capsule.publish(&view);
        let (ver_b, _) = overlay_capsule.publish(&view);
        let overlays_view = KgpuOverlayView {
            overlays: &overlay_vec,
        };
        let overlays: Vec<_> = overlays_view.overlays().collect();
        assert_eq!(ver_a, 1);
        assert_eq!(ver_b, 2);
        assert_eq!(overlays.len(), 1);
        assert!(overlays[0].morale_min_q16 <= overlays[0].morale_max_q16);
        assert_eq!(overlays[0].charge_density_q16, 0);
    }

    #[test]
    fn ingest_overlays_returns_versioned_snapshot() {
        let mut slab = RenderSoaSlabCapsule::new(1, 1);
        let f = FormationCapsule::new(1, 0, 0, 0, 0, 10_000, 0, 0, 0, 0);
        let shard = [f];
        let shards = [&shard[..]];
        let view = collect_world_render_slab(&shards, &mut slab).unwrap();
        let overlay_capsule = RenderOverlayCapsule::new();
        let kgpu = KgpuTerminalCapsule::new();
        let snap = kgpu.ingest_overlays(&overlay_capsule, &view, 77);
        let overlays: Vec<_> = snap.view().overlays().collect();
        assert_eq!(snap.version, 1);
        assert_eq!(snap.tick, 77);
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].brace_density_q16, 0);
    }
}
