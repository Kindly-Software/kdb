#![cfg(feature = "io-uring")]

use atomic_capsule::runtime::{IoUringBatchCapsule, IoUringCapsule, IoUringError};
use atomic_capsule::{verify_alignment_only, verify_capsule_properties};
use core::sync::atomic::{AtomicU64, Ordering};

use crate::formation::FormationCapsule;
use crate::frame_stream::FrameStreamCapsule;
use crate::diplomacy::DiplomaticSnapshot;
use crate::kgpu_bridge::{
    KgpuTerminalCapsule, RenderOverlayCapsule, RenderOverlaySnapshot, RenderSnapshot,
};
use crate::kgpu_ingest::KgpuRenderSinkCapsule;
use crate::order::OrderQueueCapsule;
use crate::replay::{ReplayFlushCapsule, ReplayIndexCapsule, ReplayLogCapsule, ReplayMmapCapsule};
use crate::snapshot::{CampaignSnapshotCapsule, SnapshotMmapCapsule};
use crate::strategic_map::StrategicSnapshot;
use crate::structure::StructureCapsule;
use crate::telemetry::TelemetryCapsule;
use crate::tick::RenderSoaView;
use crate::tick::{ShardContext, WorldFrame, WorldPersistError, WorldRuntimeCapsule};

/// Encapsulated io_uring sink for render frames with a preallocated buffer.
///
/// Caller is responsible for registering the buffer with the kernel (if desired) before submission.
#[repr(C, align(64))]
pub struct RenderUringSinkCapsule {
    ring: IoUringCapsule,
    batch: IoUringBatchCapsule,
    fd: i32,
    offset: AtomicU64,
    buffer: Box<[u8]>,
    stream: FrameStreamCapsule,
}

verify_alignment_only!(
    RenderUringSinkCapsule,
    core::mem::align_of::<RenderUringSinkCapsule>()
);
verify_capsule_properties!(FrameStreamCapsule, 64, 64);

impl RenderUringSinkCapsule {
    /// Create a new sink with an owned io_uring ring, batch helper, and buffer.
    ///
    /// `entries`/`flags` are passed to `IoUringCapsule::new`. `buffer_len` should be large
    /// enough for the largest render frame and ideally registered/pinned by the caller.
    pub fn new(entries: u32, flags: u32, fd: i32, buffer_len: usize) -> Result<Self, IoUringError> {
        let ring = IoUringCapsule::new(entries, flags)?;
        let batch = IoUringBatchCapsule::new(&ring)?;
        let buffer = vec![0u8; buffer_len].into_boxed_slice();
        Ok(Self {
            ring,
            batch,
            fd,
            offset: AtomicU64::new(0),
            buffer,
            stream: FrameStreamCapsule::new(),
        })
    }

    /// Mutable access to the underlying buffer (e.g., for registration/pinning).
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    /// Submit a render frame to the configured fd at the current offset.
    /// Returns the io_uring user_data identifier.
    pub fn submit_render(&mut self, view: &RenderSoaView<'_>) -> Result<u64, IoUringError> {
        let used = self
            .stream
            .write_render_frame_into(view, &mut self.buffer)
            .map_err(|_| IoUringError::InvalidParameters)?;
        let off = self.offset.load(Ordering::Relaxed);
        let ids = self
            .batch
            .batch_write(&[self.fd], &[&self.buffer[..used]], &[off])?;
        self.offset.fetch_add(used as u64, Ordering::AcqRel);
        Ok(ids[0])
    }

    /// Override the current offset (e.g., for random-access snapshots).
    pub fn set_offset(&self, offset: u64) {
        self.offset.store(offset, Ordering::Release);
    }

    /// Expose batch stats for monitoring/telemetry.
    pub fn stats(&self) -> atomic_capsule::runtime::IoUringBatchStats {
        self.batch.stats()
    }

    /// Access the underlying ring if advanced configuration is needed.
    pub fn ring(&self) -> &IoUringCapsule {
        &self.ring
    }
}

/// Error type for combined persist + io_uring streaming.
#[derive(Debug)]
pub enum RuntimeStreamError {
    Persist(WorldPersistError),
    Io(IoUringError),
}

impl From<WorldPersistError> for RuntimeStreamError {
    fn from(err: WorldPersistError) -> Self {
        RuntimeStreamError::Persist(err)
    }
}

impl From<IoUringError> for RuntimeStreamError {
    fn from(err: IoUringError) -> Self {
        RuntimeStreamError::Io(err)
    }
}

/// Runtime capsule that wires tick + persist + io_uring render streaming.
#[repr(C, align(128))]
pub struct RuntimeStreamCapsule {
    runtime: WorldRuntimeCapsule,
    render_sink: RenderUringSinkCapsule,
}

verify_alignment_only!(
    RuntimeStreamCapsule,
    core::mem::align_of::<RuntimeStreamCapsule>()
);

impl RuntimeStreamCapsule {
    pub fn new(runtime: WorldRuntimeCapsule, render_sink: RenderUringSinkCapsule) -> Self {
        Self {
            runtime,
            render_sink,
        }
    }

    pub fn runtime(&self) -> &WorldRuntimeCapsule {
        &self.runtime
    }

    pub fn render_sink(&mut self) -> &mut RenderUringSinkCapsule {
        &mut self.render_sink
    }

    /// Run tick → replay flush → snapshot append, then stream the render view via io_uring.
    #[allow(clippy::too_many_arguments)]
    pub fn tick_persist_and_stream<'a, const FB: usize, const N: usize>(
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
        diplomatic: Option<&DiplomaticSnapshot>,
        economy: Option<&crate::province_economy::EconomySnapshot>,
        command_delays: Option<&crate::order::CommandDelayBufferCapsule>,
    ) -> Result<(WorldFrame<'a>, crate::replay::ReplayPersistSnapshot, u64), RuntimeStreamError>
    {
        let (frame, replay_snap, chain) = self.runtime.tick_and_persist(
            shards,
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
        )?;
        self.render_sink.submit_render(&frame.render)?;
        Ok((frame, replay_snap, chain))
    }

    /// Run tick→persist→stream and publish overlays to kgpu for morale/LOD heatmaps.
    #[allow(clippy::too_many_arguments)]
    pub fn tick_persist_stream_and_publish<'a, const FB: usize, const N: usize>(
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
    diplomatic: Option<&DiplomaticSnapshot>,
    economy: Option<&crate::province_economy::EconomySnapshot>,
    command_delays: Option<&crate::order::CommandDelayBufferCapsule>,
    overlay_capsule: &RenderOverlayCapsule,
    kgpu: &KgpuTerminalCapsule,
    kgpu_sink: Option<&mut KgpuRenderSinkCapsule>,
) -> Result<
    (
            WorldFrame<'a>,
            crate::replay::ReplayPersistSnapshot,
            u64,
            RenderOverlaySnapshot,
            RenderSnapshot<'a>,
        ),
        RuntimeStreamError,
    > {
        let (frame, replay_snap, chain) = self.tick_persist_and_stream(
            shards,
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
        )?;
        let render_view = frame.render.clone();
        let overlay = kgpu.ingest_overlays(overlay_capsule, &render_view, frame.tick);
        let render_snapshot = kgpu.ingest_with_clock(render_view, frame.tick);
        if let Some(sink) = kgpu_sink {
            #[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
            {
                let _ = sink.submit(&render_snapshot);
            }
            #[cfg(not(all(feature = "kgpu-driver-linux", target_os = "linux")))]
            {
                let _ = sink.submit(&render_snapshot);
            }
        }
        Ok((frame, replay_snap, chain, overlay, render_snapshot))
    }
}

/// Result of a streaming frame (tick + persistence + overlay publish).
pub struct StreamingFrame<'a> {
    pub tick: u64,
    pub frame: WorldFrame<'a>,
    pub replay: crate::replay::ReplayPersistSnapshot,
    pub snapshot_chain: u64,
    pub overlay: RenderOverlaySnapshot,
    pub render: RenderSnapshot<'a>,
}

/// Convenience helper for a main loop: run tick→persist→stream and publish overlays to kgpu.
#[allow(clippy::too_many_arguments)]
pub fn streaming_frame_step<'a, const FB: usize, const N: usize>(
    stream: &'a mut RuntimeStreamCapsule,
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
    diplomatic: Option<&DiplomaticSnapshot>,
    economy: Option<&crate::province_economy::EconomySnapshot>,
    command_delays: Option<&crate::order::CommandDelayBufferCapsule>,
    overlay_capsule: &RenderOverlayCapsule,
    kgpu: &KgpuTerminalCapsule,
    kgpu_sink: Option<&mut KgpuRenderSinkCapsule>,
) -> Result<StreamingFrame<'a>, RuntimeStreamError> {
    let (frame, replay_snap, chain, overlay, render) = stream.tick_persist_stream_and_publish(
        shards,
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
        overlay_capsule,
        kgpu,
        kgpu_sink,
    )?;
    let _ = replay_log.record(frame.tick, overlay.version);
    Ok(StreamingFrame {
        tick: frame.tick,
        frame,
        replay: replay_snap,
        snapshot_chain: chain,
        overlay,
        render,
    })
}
