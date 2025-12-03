#![cfg(feature = "io-uring")]

//! Driver loop wiring for io_uring + mmap persistence + kgpu overlays.
//!
//! Capsules involved:
//! - `RuntimeStreamCapsule`: tick → persist (replay + snapshot) → io_uring render stream → overlay publish.
//! - `RenderOverlayCapsule` + `KgpuTerminalCapsule`: LOD/morale overlays for heatmaps.
//! - Replay/snapshot capsules: mmap-backed persistence chain.

use crate::ballistics::{BallisticsCapsule, FireControlProfileCapsule};
use crate::command::{CommandHierarchyCapsule, CommanderSnapshot};
use crate::courier::CourierCapsule;
use crate::diplomacy::DiplomaticSnapshot;
use crate::fire_doctrine::FireDoctrineCapsule;
use crate::formation::FormationCapsule;
use crate::garrison::GarrisonSlabCapsule;
use crate::grenade::GrenadeCapsule;
use crate::io_bridge::{
    streaming_frame_step, RenderUringSinkCapsule, RuntimeStreamCapsule, RuntimeStreamError,
    StreamingFrame,
};
use crate::kgpu_bridge::{KgpuTerminalCapsule, RenderOverlayCapsule};
use crate::kgpu_ingest::KgpuRenderSinkCapsule;
use crate::order::OrderQueueCapsule;
use crate::pathing::PathingCapsule;
use crate::replay::{ReplayFlushCapsule, ReplayIndexCapsule, ReplayLogCapsule, ReplayMmapCapsule};
use crate::snapshot::{CampaignSnapshotCapsule, SnapshotMmapCapsule};
use crate::strategic_map::StrategicSnapshot;
use crate::structure::StructureCapsule;
use crate::supply::SupplySnapshot;
use crate::telemetry::{FormationBreakTelemetryCapsule, TelemetryCapsule};
use crate::terrain::TerrainGridCapsule;
use crate::tick::WorldRuntimeCapsule;
use crate::tick::{make_shard_context, ShardContext};
use atomic_capsule::verify_alignment_only;

/// Convenience helper to drive one tick from a scheduler loop.
///
/// This is intended for the live driver: pass shard contexts plus persistence/overlay handles and
/// the driver will tick → persist (replay+snapshot) → stream via io_uring → publish overlays to kgpu.
/// Keep the caller in control of pacing/clock/RNG seeding.
pub fn run_driver_tick<'a, const FB: usize, const N: usize>(
    driver: &'a mut DriverCapsule<'a, FB, N>,
    shards: &'a [ShardContext<'a, FB>],
    strategic: Option<&'a StrategicSnapshot>,
    kgpu_sink: Option<&'a mut crate::kgpu_ingest::KgpuRenderSinkCapsule>,
) -> Result<StreamingFrame<'a>, RuntimeStreamError> {
    driver.step(shards, strategic, kgpu_sink)
}

/// Stateful driver capsule that owns the runtime stream and ties together persistence + overlays.
#[repr(C, align(128))]
pub struct DriverCapsule<'a, const FB: usize, const N: usize> {
    stream: RuntimeStreamCapsule,
    overlay_capsule: &'a RenderOverlayCapsule,
    kgpu: &'a KgpuTerminalCapsule,
    replay_log: &'a ReplayLogCapsule<N>,
    replay_flush: &'a ReplayFlushCapsule,
    replay_mmap: &'a mut ReplayMmapCapsule,
    replay_index: Option<&'a ReplayIndexCapsule>,
    snapshot_capsule: &'a CampaignSnapshotCapsule,
    snapshot_mmap: &'a mut SnapshotMmapCapsule,
    formations: &'a [FormationCapsule],
    structures: &'a [StructureCapsule],
    garrisons: Option<&'a GarrisonSlabCapsule>,
    orders: &'a OrderQueueCapsule,
    telemetry: &'a TelemetryCapsule,
    ballistics: Option<&'a BallisticsCapsule>,
    fire_profile: Option<&'a FireControlProfileCapsule>,
    terrain: Option<&'a TerrainGridCapsule>,
    grenades: Option<&'a GrenadeCapsule>,
    courier: Option<&'a CourierCapsule>,
    fire_doctrine: Option<&'a FireDoctrineCapsule>,
}

verify_alignment_only!(
    DriverCapsule<'_, 1, 1>,
    core::mem::align_of::<DriverCapsule<'_, 1, 1>>()
);

impl<'a, const FB: usize, const N: usize> DriverCapsule<'a, FB, N> {
    pub fn new(
        runtime: WorldRuntimeCapsule,
        render_sink: RenderUringSinkCapsule,
        overlay_capsule: &'a RenderOverlayCapsule,
        kgpu: &'a KgpuTerminalCapsule,
        replay_log: &'a ReplayLogCapsule<N>,
        replay_flush: &'a ReplayFlushCapsule,
        replay_mmap: &'a mut ReplayMmapCapsule,
        replay_index: Option<&'a ReplayIndexCapsule>,
        snapshot_capsule: &'a CampaignSnapshotCapsule,
        snapshot_mmap: &'a mut SnapshotMmapCapsule,
        formations: &'a [FormationCapsule],
        structures: &'a [StructureCapsule],
        garrisons: Option<&'a GarrisonSlabCapsule>,
        orders: &'a OrderQueueCapsule,
        telemetry: &'a TelemetryCapsule,
        ballistics: Option<&'a BallisticsCapsule>,
        fire_profile: Option<&'a FireControlProfileCapsule>,
        terrain: Option<&'a TerrainGridCapsule>,
        grenades: Option<&'a GrenadeCapsule>,
        courier: Option<&'a CourierCapsule>,
        fire_doctrine: Option<&'a FireDoctrineCapsule>,
    ) -> Self {
        Self {
            stream: RuntimeStreamCapsule::new(runtime, render_sink),
            overlay_capsule,
            kgpu,
            replay_log,
            replay_flush,
            replay_mmap,
            replay_index,
            snapshot_capsule,
            snapshot_mmap,
            formations,
            structures,
            garrisons,
            orders,
            telemetry,
            ballistics,
            fire_profile,
            terrain,
            grenades,
            courier,
            fire_doctrine,
        }
    }

    /// Run one frame through the full pipeline; returns the streaming bundle with overlay snapshot.
    pub fn step<'s>(
        &'s mut self,
        shards: &'s [ShardContext<'s, FB>],
        strategic: Option<&'s StrategicSnapshot>,
        diplomatic: Option<&'s DiplomaticSnapshot>,
        economy: Option<&'s crate::province_economy::EconomySnapshot>,
        command_delays: Option<&'s crate::order::CommandDelayBufferCapsule>,
        kgpu_sink: Option<&mut KgpuRenderSinkCapsule>,
    ) -> Result<StreamingFrame<'s>, RuntimeStreamError> {
        streaming_frame_step(
            &mut self.stream,
            shards,
            self.replay_log,
            self.replay_flush,
            &mut *self.replay_mmap,
            self.replay_index,
            self.snapshot_capsule,
            &mut *self.snapshot_mmap,
            self.formations,
            self.structures,
            self.garrisons,
            self.orders,
            self.telemetry,
            strategic,
            diplomatic,
            economy,
            command_delays,
            self.overlay_capsule,
            self.kgpu,
            kgpu_sink,
        )
    }

    /// Build a shard context using the driver's cached handles (orders/telemetry/ballistics/profile/terrain/supply).
    pub fn make_shard_context(
        &'a self,
        shard_id: usize,
        formations: &'a [FormationCapsule],
        pathings: &'a [PathingCapsule],
        formation_breaks: Option<&'a FormationBreakTelemetryCapsule<FB>>,
        supply: Option<&'a SupplySnapshot>,
        strategic: Option<&'a StrategicSnapshot>,
        command_hierarchy: Option<&'a CommandHierarchyCapsule>,
        commanders: Option<&'a [CommanderSnapshot]>,
        generals: Option<&'a [crate::general::GeneralSnapshot]>,
        command_delays: Option<&'a crate::order::CommandDelayBufferCapsule>,
    ) -> ShardContext<'a, FB> {
        make_shard_context(
            shard_id,
            self.orders,
            formations,
            pathings,
            self.telemetry,
            formation_breaks,
            self.ballistics,
            self.fire_profile,
            self.terrain,
            self.grenades,
            Some(self.structures),
            self.garrisons,
            supply,
            self.courier,
            self.fire_doctrine,
            None,
            None,
            generals,
            command_hierarchy,
            commanders,
            strategic,
            command_delays,
        )
    }
}
