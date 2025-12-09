//! Minimal kgpu ingestion helper (placeholder until the real kgpu render path is wired).
//!
//! This capsule encodes a `RenderSnapshot` into a staging buffer so it can be handed to a future
//! kgpu queue submission. It keeps the interface deterministic and lock-free; once the kgpu
//! renderer API is available, swap the `submit` method to issue real GPU commands.

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
use crate::kgpu_bridge::KgpuDriverSession;
use crate::kgpu_bridge::RenderSnapshot;
use crate::kgpu_bridge::{
    make_aperture_overlay_from_render, make_doctrine_overlay_from_render,
    make_supply_heatmap_from_render, SupplyHeatmapLegend,
};
use atomic_capsule::{verify_alignment_only, verify_capsule_properties};
use core::sync::atomic::{AtomicU64, Ordering};

/// Staging capsule for render ingestion. Lock-free, preallocates a byte buffer.
#[repr(C, align(64))]
pub struct KgpuIngestCapsule {
    version: AtomicU64,
    buffer: Vec<u8>,
}

verify_capsule_properties!(KgpuIngestCapsule, 64, 64);

impl KgpuIngestCapsule {
    /// Create a new ingest capsule with a preallocated buffer capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            version: AtomicU64::new(0),
            buffer: Vec::with_capacity(capacity),
        }
    }

    /// Encode a render snapshot into the staging buffer and bump the version.
    ///
    /// Current layout (little endian):
    /// - u64: frame_id
    /// - u64: tick
    /// - u64: total_len (formations)
    /// - u64: overlays_len
    /// - overlays_len × fields:
    ///   shard_id, start, len, lod_stride,
    ///   morale_min, morale_max,
    ///   charge_density_q16, brace_density_q16,
    ///   avg_density_q16, avg_variance_q16,
    ///   avg_gap_close_q16, avg_rank_variance_q16, avg_gap_fatigue_penalty_q16,
    ///   supply_pressure_avg_q16, supply_fatigue_penalty_avg_q16
    pub fn encode(&mut self, snapshot: &RenderSnapshot<'_>) -> &[u8] {
        let overlays = &snapshot.view.overlays;
        // 4 u64 headers + 15 u32 fields per overlay (60B each).
        let needed = 8 * 4 + overlays.len() * 15 * 4;
        if self.buffer.capacity() < needed {
            self.buffer.reserve(needed - self.buffer.capacity());
        }
        self.buffer.clear();
        self.buffer
            .extend_from_slice(&snapshot.frame_id.to_le_bytes());
        self.buffer.extend_from_slice(&snapshot.tick.to_le_bytes());
        self.buffer
            .extend_from_slice(&(snapshot.view.total_len as u64).to_le_bytes());
        self.buffer
            .extend_from_slice(&(overlays.len() as u64).to_le_bytes());
        for o in overlays {
            let charge_density_q16 = if o.len > 0 {
                ((o.charging as u64 * 65_536) / o.len as u64).min(u32::MAX as u64) as u32
            } else {
                0
            };
            let brace_density_q16 = if o.len > 0 {
                ((o.braced as u64 * 65_536) / o.len as u64).min(u32::MAX as u64) as u32
            } else {
                0
            };
            self.buffer
                .extend_from_slice(&(o.shard_id as u32).to_le_bytes());
            self.buffer
                .extend_from_slice(&(o.start as u32).to_le_bytes());
            self.buffer.extend_from_slice(&(o.len as u32).to_le_bytes());
            self.buffer.extend_from_slice(&o.lod_stride.to_le_bytes());
            self.buffer
                .extend_from_slice(&o.morale_min_q16.to_le_bytes());
            self.buffer
                .extend_from_slice(&o.morale_max_q16.to_le_bytes());
            self.buffer
                .extend_from_slice(&charge_density_q16.to_le_bytes());
            self.buffer
                .extend_from_slice(&brace_density_q16.to_le_bytes());
            self.buffer
                .extend_from_slice(&o.avg_density_q16.to_le_bytes());
            self.buffer
                .extend_from_slice(&o.avg_variance_q16.to_le_bytes());
            self.buffer
                .extend_from_slice(&o.avg_gap_close_q16.to_le_bytes());
            self.buffer
                .extend_from_slice(&o.avg_rank_variance_q16.to_le_bytes());
            self.buffer
                .extend_from_slice(&o.avg_gap_fatigue_penalty_q16.to_le_bytes());
            self.buffer
                .extend_from_slice(&o.supply_pressure_avg_q16.to_le_bytes());
            self.buffer
                .extend_from_slice(&o.supply_fatigue_penalty_avg_q16.to_le_bytes());
        }
        let _ = self.version.fetch_add(1, Ordering::AcqRel);
        &self.buffer
    }

    /// Version counter for ingestion epochs.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    /// Convenience: project overlays into normalized supply channels (0.0-1.0) for heatmaps.
    pub fn supply_heatmap_channels<'a>(
        &self,
        snapshot: &'a RenderSnapshot<'a>,
    ) -> impl Iterator<Item = (u32, f32, f32)> + 'a {
        snapshot.view.overlays.iter().map(|o| {
            (
                o.shard_id as u32,
                o.supply_pressure_avg_q16 as f32 / 65_535.0,
                o.supply_fatigue_penalty_avg_q16 as f32 / 65_535.0,
            )
        })
    }
}

/// Render sink capsule: encodes a render snapshot and optionally submits to kgpu.
///
/// - Chaos/UCE34: lock-free buffer, deterministic encoding, no mutexes.
/// - When kgpu driver is unavailable, submission is a no-op but encoding still advances version.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
#[repr(C, align(64))]
pub struct KgpuRenderSinkCapsule {
    ingest: KgpuIngestCapsule,
    session: KgpuDriverSession,
    legend: SupplyHeatmapLegend,
    heatmap_enabled: bool,
}

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
verify_alignment_only!(KgpuRenderSinkCapsule, 64);

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
impl KgpuRenderSinkCapsule {
    /// Create a sink with a persistent kgpu session.
    pub fn new(
        device_index: usize,
        legend: SupplyHeatmapLegend,
        heatmap_enabled: bool,
        staging_capacity: usize,
    ) -> Result<Self, crate::kgpu_bridge::KgpuDriverError> {
        let (session, _devices) = KgpuDriverSession::new(device_index)?;
        Ok(Self {
            ingest: KgpuIngestCapsule::new(staging_capacity),
            session,
            legend,
            heatmap_enabled,
        })
    }

    /// Encode + submit render and optional heatmap to kgpu driver.
    pub fn submit(
        &mut self,
        snapshot: &RenderSnapshot<'_>,
    ) -> Result<(), crate::kgpu_bridge::KgpuDriverError> {
        let _ = self.ingest.encode(snapshot);
        self.session.submit(snapshot)?;
        if self.heatmap_enabled {
            let frame = make_supply_heatmap_from_render(snapshot);
            self.session.submit_supply_heatmap(&frame, &self.legend)?;
        }
        let doctrine = make_doctrine_overlay_from_render(snapshot);
        self.session.submit_doctrine_overlay(&doctrine)?;
        let _aperture = make_aperture_overlay_from_render(snapshot);
        Ok(())
    }

    pub fn enable_heatmap(&mut self, enabled: bool) {
        self.heatmap_enabled = enabled;
    }
}

/// Stub sink when kgpu driver is not compiled in.
#[cfg(not(all(feature = "kgpu-driver-linux", target_os = "linux")))]
#[repr(C, align(64))]
pub struct KgpuRenderSinkCapsule {
    ingest: KgpuIngestCapsule,
    legend: SupplyHeatmapLegend,
    heatmap_enabled: bool,
}

#[cfg(not(all(feature = "kgpu-driver-linux", target_os = "linux")))]
verify_alignment_only!(KgpuRenderSinkCapsule, 64);

#[cfg(not(all(feature = "kgpu-driver-linux", target_os = "linux")))]
impl KgpuRenderSinkCapsule {
    pub fn new(
        _device_index: usize,
        legend: SupplyHeatmapLegend,
        heatmap_enabled: bool,
        staging_capacity: usize,
    ) -> Result<Self, ()> {
        Ok(Self {
            ingest: KgpuIngestCapsule::new(staging_capacity),
            legend,
            heatmap_enabled,
        })
    }

    pub fn submit(&mut self, snapshot: &RenderSnapshot<'_>) -> Result<(), ()> {
        let _ = self.ingest.encode(snapshot);
        if self.heatmap_enabled {
            let _ = make_supply_heatmap_from_render(snapshot);
        }
        let _ = make_doctrine_overlay_from_render(snapshot);
        let _ = make_aperture_overlay_from_render(snapshot);
        Ok(())
    }

    pub fn enable_heatmap(&mut self, enabled: bool) {
        self.heatmap_enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formation::FormationCapsule;
    use crate::kgpu_bridge::KgpuTerminalCapsule;
    use crate::tick::{collect_world_render_slab, RenderSoaSlabCapsule};

    #[test]
    fn encode_increments_version() {
        let mut slab = RenderSoaSlabCapsule::new(1, 1);
        let f = FormationCapsule::new(1, 0, 0, 0, 0, 10_000, 0, 0, 0, 0);
        let shard = [f];
        let shards = [&shard[..]];
        let view = collect_world_render_slab(&shards, &mut slab).unwrap();
        let kgpu = KgpuTerminalCapsule::new();
        let snap = kgpu.ingest_with_clock(view, 7);
        let mut ingest = KgpuIngestCapsule::new(256);
        let buf = ingest.encode(&snap);
        assert!(buf.len() >= 32);
        assert_eq!(ingest.version(), 1);
    }
}
