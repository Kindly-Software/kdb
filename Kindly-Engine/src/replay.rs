use crate::order::OrderKind;
use crate::strategic_map::StrategicEventKind;
use atomic_capsule::{verify_alignment_only, verify_capsule_properties};
use core::array::from_fn;
use core::sync::atomic::{AtomicU64, Ordering};
use std::io::Write;
use std::path::Path;

use atomic_capsule::mmap::MmapError;
use atomic_capsule::mmap::{MmapLayout, MmapManager};
#[cfg(feature = "io-uring")]
use atomic_capsule::runtime::{IoUringBatchCapsule, IoUringCapsule, IoUringError};

#[derive(Debug, Clone, Copy)]
pub struct ReplayEvent {
    pub tick: u64,
    pub payload: u64,
}

impl ReplayEvent {
    pub fn new(tick: u64, payload: u64) -> Self {
        Self { tick, payload }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandHistogramKind {
    Delay,
    Eta,
}
/// Replay entry capsule (tick + payload).
#[repr(C, align(32))]
pub struct ReplayEventCapsule {
    tick: AtomicU64,
    payload: AtomicU64,
}

impl ReplayEventCapsule {
    pub const fn new() -> Self {
        Self {
            tick: AtomicU64::new(0),
            payload: AtomicU64::new(0),
        }
    }

    pub fn write(&self, tick: u64, payload: u64) {
        self.tick.store(tick, Ordering::Relaxed);
        self.payload.store(payload, Ordering::Release);
    }

    pub fn read(&self) -> ReplayEvent {
        ReplayEvent {
            tick: self.tick.load(Ordering::Acquire),
            payload: self.payload.load(Ordering::Acquire),
        }
    }
}

verify_capsule_properties!(ReplayEventCapsule, 32, 32);

/// Replay log capsule (T5 streaming, T9 persistence-ready).
///
/// - SPSC ring buffer of fixed capacity.
/// - Use tick to enforce deterministic ordering.
#[repr(C, align(128))]
pub struct ReplayLogCapsule<const N: usize> {
    head: AtomicU64,
    tail: AtomicU64,
    dropped: AtomicU64,
    entries: [ReplayEventCapsule; N],
    _padding: [u8; 64],
}

impl<const N: usize> ReplayLogCapsule<N> {
    pub fn new() -> Self {
        Self {
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            entries: from_fn(|_| ReplayEventCapsule::new()),
            _padding: [0; 64],
        }
    }

    /// Record an event; returns false if buffer is full.
    pub fn record(&self, tick: u64, payload: u64) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);
        if tail.wrapping_sub(head) as usize >= N {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        let slot = (tail as usize) & (N - 1);
        self.entries[slot].write(tick, payload);
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        true
    }

    /// Drain all events into a Vec in FIFO order.
    pub fn drain(&self) -> Vec<ReplayEvent> {
        let mut out = Vec::new();
        loop {
            let head = self.head.load(Ordering::Relaxed);
            let tail = self.tail.load(Ordering::Acquire);
            if head == tail {
                break;
            }
            let slot = (head as usize) & (N - 1);
            out.push(self.entries[slot].read());
            self.head.store(head.wrapping_add(1), Ordering::Release);
        }
        out
    }

    pub fn stats(&self) -> ReplayStats {
        ReplayStats {
            head: self.head.load(Ordering::Relaxed),
            tail: self.tail.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
            capacity: N as u64,
        }
    }
}

verify_alignment_only!(ReplayLogCapsule<8>, 128);

#[derive(Debug, Clone, Copy)]
pub struct ReplayStats {
    pub head: u64,
    pub tail: u64,
    pub dropped: u64,
    pub capacity: u64,
}

/// Persistence capsule to serialize replay events to an external sink (e.g., mmap/file).
#[repr(C, align(64))]
pub struct ReplayPersistCapsule {
    flushed_events: AtomicU64,
    flushed_bytes: AtomicU64,
    _padding: [u8; 48],
}

impl ReplayPersistCapsule {
    pub const fn new() -> Self {
        Self {
            flushed_events: AtomicU64::new(0),
            flushed_bytes: AtomicU64::new(0),
            _padding: [0; 48],
        }
    }

    /// Serialize all drained events to a writer (little-endian tick/payload).
    pub fn flush_to_writer<const N: usize, W: Write>(
        &self,
        log: &ReplayLogCapsule<N>,
        writer: &mut W,
    ) -> std::io::Result<()> {
        let events = log.drain();
        for ev in &events {
            writer.write_all(&ev.tick.to_le_bytes())?;
            writer.write_all(&ev.payload.to_le_bytes())?;
        }
        self.flushed_events
            .fetch_add(events.len() as u64, Ordering::AcqRel);
        self.flushed_bytes
            .fetch_add((events.len() * 16) as u64, Ordering::AcqRel);
        Ok(())
    }

    pub fn snapshot(&self) -> ReplayPersistSnapshot {
        ReplayPersistSnapshot {
            flushed_events: self.flushed_events.load(Ordering::Relaxed),
            flushed_bytes: self.flushed_bytes.load(Ordering::Relaxed),
        }
    }
}

verify_capsule_properties!(ReplayPersistCapsule, 64, 64);

#[derive(Debug, Clone, Copy)]
pub struct ReplayPersistSnapshot {
    pub flushed_events: u64,
    pub flushed_bytes: u64,
}

/// Compact hash-chained index capsule for deterministic replay validation.
#[repr(C, align(64))]
pub struct ReplayIndexCapsule {
    last_tick: AtomicU64,
    frame_count: AtomicU64,
    hash_chain: AtomicU64,
}

impl ReplayIndexCapsule {
    pub const fn new() -> Self {
        Self {
            last_tick: AtomicU64::new(0),
            frame_count: AtomicU64::new(0),
            hash_chain: AtomicU64::new(0x9E37_79B9),
        }
    }

    /// Update the index with a frame hash (e.g., hash of rendered buffer or log payload).
    pub fn record_frame(&self, tick: u64, payload_hash: u64) -> ReplayIndexSnapshot {
        let next_hash =
            self.hash_chain.load(Ordering::Relaxed).rotate_left(13) ^ payload_hash ^ tick;
        self.hash_chain.store(next_hash, Ordering::Release);
        self.last_tick.store(tick, Ordering::Release);
        self.frame_count.fetch_add(1, Ordering::AcqRel);
        self.snapshot()
    }

    pub fn snapshot(&self) -> ReplayIndexSnapshot {
        ReplayIndexSnapshot {
            last_tick: self.last_tick.load(Ordering::Relaxed),
            frame_count: self.frame_count.load(Ordering::Relaxed),
            hash_chain: self.hash_chain.load(Ordering::Relaxed),
        }
    }
}

verify_capsule_properties!(ReplayIndexCapsule, 64, 64);

#[derive(Debug, Clone, Copy)]
pub struct ReplayIndexSnapshot {
    pub last_tick: u64,
    pub frame_count: u64,
    pub hash_chain: u64,
}

/// Mmap-backed persistence capsule for replay logs.
#[repr(C, align(128))]
pub struct ReplayMmapCapsule {
    manager: MmapManager,
    region_idx: usize,
    persist_counters: ReplayPersistCapsule,
    hash_chain: AtomicU64,
}

impl ReplayMmapCapsule {
    /// Create a new mmap-backed log.
    ///
    /// `file_size` must be 4KB aligned; `region_count` partitions the file.
    pub fn new(path: &Path, file_size: u64, region_count: usize) -> Result<Self, MmapError> {
        let layout = MmapLayout::new(file_size, region_count)?;
        let manager = MmapManager::new(path, &layout)?;
        Ok(Self {
            manager,
            region_idx: 0,
            persist_counters: ReplayPersistCapsule::new(),
            hash_chain: AtomicU64::new(0xD1B5_54C3_1234_5678),
        })
    }

    /// Append drained events from `log` into the mmap region and fsync.
    pub fn append_from_log<const N: usize>(
        &mut self,
        log: &ReplayLogCapsule<N>,
    ) -> Result<(), MmapError> {
        let events = log.drain();
        if events.is_empty() {
            return Ok(());
        }

        let region = self.manager.region(self.region_idx).ok_or_else(|| {
            MmapError::invalid_region_index(self.region_idx, self.manager.region_count())
        })?;

        let bytes_needed = (events.len() * 16) as u32;
        let offset = region.allocate(bytes_needed)?;

        // SAFETY: offset validated by region.allocate; ptr_at_offset bounds-checks.
        let ptr = unsafe { self.manager.ptr_at_offset(offset)? };
        let buf = unsafe { std::slice::from_raw_parts_mut(ptr, bytes_needed as usize) };

        for (i, ev) in events.iter().enumerate() {
            let base = i * 16;
            buf[base..base + 8].copy_from_slice(&ev.tick.to_le_bytes());
            buf[base + 8..base + 16].copy_from_slice(&ev.payload.to_le_bytes());
        }

        self.manager.fsync()?;
        self.persist_counters
            .flushed_events
            .fetch_add(events.len() as u64, Ordering::AcqRel);
        self.persist_counters
            .flushed_bytes
            .fetch_add(bytes_needed as u64, Ordering::AcqRel);
        // Update hash chain with this batch for integrity auditing.
        let mut hash_acc = self.hash_chain.load(Ordering::Relaxed);
        hash_acc ^= hash_events(&events);
        self.hash_chain.store(hash_acc, Ordering::Release);
        Ok(())
    }

    /// Append drained events and update an index capsule with a hash chain.
    pub fn append_from_log_with_index<const N: usize>(
        &mut self,
        log: &ReplayLogCapsule<N>,
        index: &ReplayIndexCapsule,
    ) -> Result<(), MmapError> {
        let events = log.drain();
        if events.is_empty() {
            return Ok(());
        }

        let region = self.manager.region(self.region_idx).ok_or_else(|| {
            MmapError::invalid_region_index(self.region_idx, self.manager.region_count())
        })?;

        let bytes_needed = (events.len() * 16) as u32;
        let offset = region.allocate(bytes_needed)?;

        // SAFETY: offset validated by region.allocate; ptr_at_offset bounds-checks.
        let ptr = unsafe { self.manager.ptr_at_offset(offset)? };
        let buf = unsafe { std::slice::from_raw_parts_mut(ptr, bytes_needed as usize) };

        let mut hash_acc: u64 = 0;
        for (i, ev) in events.iter().enumerate() {
            let base = i * 16;
            buf[base..base + 8].copy_from_slice(&ev.tick.to_le_bytes());
            buf[base + 8..base + 16].copy_from_slice(&ev.payload.to_le_bytes());
            hash_acc ^= ev.tick.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ ev.payload.rotate_left(7);
        }

        self.manager.fsync()?;
        self.persist_counters
            .flushed_events
            .fetch_add(events.len() as u64, Ordering::AcqRel);
        self.persist_counters
            .flushed_bytes
            .fetch_add(bytes_needed as u64, Ordering::AcqRel);
        let chain = index.record_frame(events.last().map(|e| e.tick).unwrap_or(0), hash_acc);
        self.hash_chain.store(chain.hash_chain, Ordering::Release);
        Ok(())
    }

    pub fn snapshot(&self) -> ReplayPersistSnapshot {
        self.persist_counters.snapshot()
    }

    pub fn hash_chain(&self) -> u64 {
        self.hash_chain.load(Ordering::Relaxed)
    }
}

fn hash_events(events: &[ReplayEvent]) -> u64 {
    let mut h = 0xD1B5_54C3_1234_5678u64;
    for ev in events {
        h ^= ev.tick.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        h ^= ev.payload.rotate_left(7);
        h = h.rotate_left(9);
    }
    h
}

/// Encode a shock/morale replay payload: upper 32 bits casualties, next 16 shock penalty Q16, low 16 shock weight delta (clamped).
pub fn encode_shock_replay_payload(
    casualty_delta: u32,
    shock_penalty_q16: u32,
    shock_weight_delta_q16: u32,
) -> u64 {
    let shock_pen = shock_penalty_q16.min(0xFFFF);
    let shock_wt = shock_weight_delta_q16.min(0xFFFF);
    ((casualty_delta as u64) << 32) | ((shock_pen as u64) << 16) | (shock_wt as u64)
}

/// Encode charge/brace replay payload with a tag for disambiguation.
///
/// Layout:
/// - bits 63..48: 0xC100 tag
/// - bits 47..32: charge orders (clamped)
/// - bits 31..16: charge commits (clamped)
/// - bits 15..0 : brace orders (clamped)
pub fn encode_charge_replay_payload(
    charge_orders: u32,
    charge_commits: u32,
    brace_orders: u32,
) -> u64 {
    let tag = 0xC100u64 << 48;
    let charges = (charge_orders.min(0xFFFF) as u64) << 32;
    let commits = (charge_commits.min(0xFFFF) as u64) << 16;
    let braces = brace_orders.min(0xFFFF) as u64;
    tag | charges | commits | braces
}

/// Encode supply averages for replay (pressure/fatigue Q16.16).
///
/// Layout:
/// - bits 63..48: 0xC200 tag
/// - bits 47..32: avg pressure (clamped)
/// - bits 31..16: avg fatigue penalty (clamped)
/// - bits 15..0 : reserved (zero)
pub fn encode_supply_replay_payload(pressure_avg_q16: u32, fatigue_avg_q16: u32) -> u64 {
    let tag = 0xC200u64 << 48;
    let p = (pressure_avg_q16.min(0xFFFF) as u64) << 32;
    let f = (fatigue_avg_q16.min(0xFFFF) as u64) << 16;
    tag | p | f
}

/// Encode logistics overlays (throughput + command-delay penalty + disruptions/attrition).
///
/// Layout:
/// - bits 63..48: 0xC230 tag
/// - bits 47..32: avg throughput (Q16.16, clamped)
/// - bits 31..16: avg command-delay penalty (ticks, clamped)
/// - bits 15..8 : disruptions (clamped to u8)
/// - bits 7..0  : attrition events (clamped to u8)
pub fn encode_logistics_replay_payload(
    throughput_avg_q16: u32,
    command_delay_penalty_ticks: u32,
    disruptions: u32,
    attrition_events: u32,
) -> u64 {
    let tag = 0xC230u64 << 48;
    let throughput = (throughput_avg_q16.min(0xFFFF) as u64) << 32;
    let penalty = (command_delay_penalty_ticks.min(0xFFFF) as u64) << 16;
    let disruptions = (disruptions.min(0xFF) as u64) << 8;
    let attrition = attrition_events.min(0xFF) as u64;
    tag | throughput | penalty | disruptions | attrition
}

/// Encode supply route cut event (disruption cluster).
///
/// Layout:
/// - bits 63..48: 0xC231 tag
/// - bits 31..16: cuts (u16)
/// - bits 15..0 : reserved
pub fn encode_supply_route_cut(cuts: u32) -> u64 {
    let tag = 0xC231u64 << 48;
    let c = (cuts.min(0xFFFF) as u64) << 16;
    tag | c
}

/// Encode Ops/C2 guardrails (backpressure + courier reliability).
///
/// Layout:
/// - bits 63..48: 0xC340 tag
/// - bits 47..32: ops backpressure (Q16.16, clamped)
/// - bits 31..16: courier reliability (Q16.16, clamped)
/// - bits 15..0 : reserved
pub fn encode_ops_guard_payload(backpressure_q16: u32, courier_reliability_q16: u32) -> u64 {
    let tag = 0xC340u64 << 48;
    let backpressure = (backpressure_q16.min(0xFFFF) as u64) << 32;
    let reliability = (courier_reliability_q16.min(0xFFFF) as u64) << 16;
    tag | backpressure | reliability
}

/// Encode ops backpressure drop count (orders dropped due to caps).
///
/// Layout:
/// - bits 63..48: 0xC342 tag
/// - bits 31..16: drops (u16)
/// - bits 15..0 : reserved
pub fn encode_ops_backpressure_drops(drops: u32) -> u64 {
    let tag = 0xC342u64 << 48;
    let d = (drops.min(0xFFFF) as u64) << 16;
    tag | d
}

/// Encode ops congestion bucket (queue depth severity).
///
/// Layout:
/// - bits 63..48: 0xC343 tag
/// - bits 31..24: congestion bucket (0..7)
/// - bits 23..0 : reserved
pub fn encode_ops_congestion(bucket: u8) -> u64 {
    let tag = 0xC343u64 << 48;
    let b = (bucket.min(0xFF) as u64) << 24;
    tag | b
}

/// Encode command/courier overlays (command stress + courier latency/loss).
///
/// Layout:
/// - bits 63..48: 0xC300 tag
/// - bits 47..32: command stress (Q16.16, clamped)
/// - bits 31..16: courier ETA (ticks, clamped)
/// - bits 15..8 : courier losses (clamped to 8 bits)
/// - bits 7..0  : courier spoofed (clamped to 8 bits)
pub fn encode_command_replay_payload(
    command_stress_q16: u32,
    courier_eta_ticks: u32,
    courier_losses: u32,
    courier_spoofed: u32,
) -> u64 {
    let tag = 0xC300u64 << 48;
    let stress = (command_stress_q16.min(0xFFFF) as u64) << 32;
    let eta = (courier_eta_ticks.min(0xFFFF) as u64) << 16;
    let losses = (courier_losses.min(0xFF) as u64) << 8;
    let spoofed = courier_spoofed.min(0xFF) as u64;
    tag | stress | eta | losses | spoofed
}

/// Encode command delay p95 bucket (histogram-derived).
///
/// Layout:
/// - bits 63..48: 0xC324 tag
/// - bits 31..24: p95 bucket (0..7)
/// - bits 23..0 : reserved
pub fn encode_command_delay_p95(bucket: u8) -> u64 {
    let tag = 0xC324u64 << 48;
    let b = (bucket.min(0xFF) as u64) << 24;
    tag | b
}

/// Encode courier ETA p95 bucket (histogram-derived).
///
/// Layout:
/// - bits 63..48: 0xC325 tag
/// - bits 31..24: p95 bucket (0..7)
/// - bits 23..0 : reserved
pub fn encode_courier_eta_p95(bucket: u8) -> u64 {
    let tag = 0xC325u64 << 48;
    let b = (bucket.min(0xFF) as u64) << 24;
    tag | b
}

/// Encode applied command delay summary (count + avg ticks).
///
/// Layout:
/// - bits 63..48: 0xC320 tag
/// - bits 47..32: delayed order count (u16)
/// - bits 31..16: avg delay ticks (u16)
pub fn encode_command_delay_applied(count: u32, avg_delay_ticks: u32) -> u64 {
    let tag = 0xC320u64 << 48;
    let c = (count.min(0xFFFF) as u64) << 32;
    let avg = (avg_delay_ticks.min(0xFFFF) as u64) << 16;
    tag | c | avg
}

/// Encode a strategic ownership/repair event for replay overlays.
///
/// Layout:
/// - bits 63..48: 0xCA00 tag
/// - bits 47..40: kind (0 = ownership change, 1 = infrastructure repair)
/// - bits 39..24: province_id (clamped to 16 bits)
/// - bits 23..12: primary (owner_from or infra_from_q16 >> 4, clamped to 12 bits)
/// - bits 11..0 : secondary (owner_to or infra_to_q16 >> 4, clamped to 12 bits)
pub fn encode_strategic_event_payload(ev: &crate::strategic_map::StrategicEventSnapshot) -> u64 {
    let tag = 0xCA00u64 << 48;
    let kind = (ev.kind as u64 & 0xFF) << 40;
    let province = (ev.province_id.min(0xFFFF) as u64) << 24;
    let (primary, secondary) = match ev.kind {
        StrategicEventKind::OwnershipChange => (
            (ev.from_owner_id.min(0xFFF) as u64) << 12,
            ev.to_owner_id.min(0xFFF) as u64,
        ),
        StrategicEventKind::InfrastructureRepair => (
            ((ev.from_infra_q16 >> 4).min(0xFFF) as u64) << 12,
            (ev.to_infra_q16 >> 4).min(0xFFF) as u64,
        ),
    };
    tag | kind | province | primary | secondary
}

/// Encode auxiliary strategic metadata (generation + infra delta) for auditability.
///
/// Layout:
/// - bits 63..48: 0xCA01 tag
/// - bits 47..32: province_id (u16)
/// - bits 31..16: generation (lsb 16; snapshot carries full 64)
/// - bits 15..0 : infra_delta_q16 >> 4 (u16, clamped); 0 for ownership changes
pub fn encode_strategic_event_meta(ev: &crate::strategic_map::StrategicEventSnapshot) -> u64 {
    let tag = 0xCA01u64 << 48;
    let province = (ev.province_id.min(0xFFFF) as u64) << 32;
    let generation = (ev.generation as u64 & 0xFFFF) << 16;
    let delta_q16 = if ev.kind == StrategicEventKind::InfrastructureRepair {
        ev.to_infra_q16
            .saturating_sub(ev.from_infra_q16)
            .saturating_div(16)
            .min(0xFFFF)
    } else {
        0
    };
    tag | province | generation | delta_q16 as u64
}

/// Encode 4 histogram buckets (command delay) into one payload chunk.
///
/// Tag scheme:
/// - 0xC310 chunk0 (buckets 0..3) delay
/// - 0xC311 chunk1 (buckets 4..7) delay
pub fn encode_command_delay_hist_payload(chunk: u8, buckets: &[u32]) -> u64 {
    debug_assert!(buckets.len() == 4);
    let tag = ((0xC310u64 + chunk.min(1) as u64) << 48) as u64;
    encode_histogram_body(tag, buckets)
}

/// Encode 4 histogram buckets (courier ETA) into one payload chunk.
///
/// Tag scheme:
/// - 0xC312 chunk0 (buckets 0..3) eta
/// - 0xC313 chunk1 (buckets 4..7) eta
pub fn encode_courier_eta_hist_payload(chunk: u8, buckets: &[u32]) -> u64 {
    debug_assert!(buckets.len() == 4);
    let tag = ((0xC312u64 + chunk.min(1) as u64) << 48) as u64;
    encode_histogram_body(tag, buckets)
}

fn encode_histogram_body(tag: u64, buckets: &[u32]) -> u64 {
    let mut out = tag;
    for (idx, bucket) in buckets.iter().enumerate().take(4) {
        let shift = 36 - (idx * 12);
        let val = (*bucket).min(0xFFF) as u64;
        out |= val << shift;
    }
    out
}

/// Encode artillery debug overlays (ricochet/crater/fuse/splash).
///
/// Layout:
/// - bits 63..48: 0xC400 tag
/// - bits 47..40: ricochet bounces (clamped to 8 bits)
/// - bits 39..32: crater radius in tiles (clamped to 8 bits)
/// - bits 31..16: fuse (ms, clamped)
/// - bits 15..0 : splash proxy (Q16.16 casualties scale, clamped)
pub fn encode_artillery_replay_payload(
    ricochet_bounces: u32,
    crater_radius_tiles: u32,
    fuse_ms: u32,
    splash_q16: u32,
) -> u64 {
    let tag = 0xC400u64 << 48;
    let ricochet = (ricochet_bounces.min(0xFF) as u64) << 40;
    let crater = (crater_radius_tiles.min(0xFF) as u64) << 32;
    let fuse = (fuse_ms.min(0xFFFF) as u64) << 16;
    let splash = splash_q16.min(0xFFFF) as u64;
    tag | ricochet | crater | fuse | splash
}

/// Encode charge corridor/path overlays (tiles) with impact mode.
///
/// Layout (11-bit tiles, 4-bit impact):
/// - bits 63..48: 0xC500 tag
/// - bits 47..37: start_x_tile (11 bits)
/// - bits 36..26: start_z_tile (11 bits)
/// - bits 25..15: target_x_tile (11 bits)
/// - bits 14..4 : target_z_tile (11 bits)
/// - bits 3..0  : impact mode (0..15)
pub fn encode_charge_path_replay_payload(
    start_x_tile: u32,
    start_z_tile: u32,
    target_x_tile: u32,
    target_z_tile: u32,
    impact_mode: u8,
) -> u64 {
    let tag = 0xC500u64 << 48;
    let sx = (start_x_tile.min(0x7FF) as u64) << 37;
    let sz = (start_z_tile.min(0x7FF) as u64) << 26;
    let tx = (target_x_tile.min(0x7FF) as u64) << 15;
    let tz = (target_z_tile.min(0x7FF) as u64) << 4;
    let impact = (impact_mode.min(0xF) as u64) & 0xF;
    tag | sx | sz | tx | tz | impact
}

/// Encode doctrine/rank-fire telemetry for replay/debug overlays.
///
/// Layout:
/// - bits 63..48: 0xC600 tag
/// - bits 47..40: rank_fire_mask_or (bit0 = front rank)
/// - bits 39..32: last_doctrine_mode (enum discriminant)
/// - bits 31..16: cadence_ticks (clamped)
/// - bits 15..8 : rank_fire_events (clamped)
/// - bits 7..0  : advance_fire_events (clamped)
pub fn encode_doctrine_replay_payload(
    rank_fire_mask: u8,
    mode: u8,
    cadence_ticks: u16,
    rank_fire_events: u16,
    advance_fire_events: u16,
) -> u64 {
    let tag = 0xC600u64 << 48;
    let mask = (rank_fire_mask as u64) << 40;
    let mode_bits = (mode as u64) << 32;
    let cadence = (cadence_ticks as u64) << 16;
    let rank_events = (rank_fire_events.min(0xFF) as u64) << 8;
    let advance = advance_fire_events.min(0xFF) as u64;
    tag | mask | mode_bits | cadence | rank_events | advance
}

/// Encode battle AI decision (source/target/order/score) for telemetry/replay.
///
/// Layout:
/// - bits 63..48: 0xC900 tag
/// - bits 47..32: source formation id (u16, clamped)
/// - bits 31..16: target formation id (u16, clamped)
/// - bits 15..8 : order kind (u8)
/// - bits 7..0  : score_q8 (u8, normalized)
pub fn encode_battle_ai_replay_payload(
    source_formation_id: u32,
    target_formation_id: u32,
    order: OrderKind,
    score_q8: u8,
) -> u64 {
    let tag = 0xC900u64 << 48;
    let src = (source_formation_id.min(0xFFFF) as u64) << 32;
    let tgt = (target_formation_id.min(0xFFFF) as u64) << 16;
    let ord = (order as u64) << 8;
    tag | src | tgt | ord | (score_q8 as u64)
}

/// Encode aggregated battle AI intent (stance/doctrine/threat centroid).
///
/// Layout:
/// - bits 63..48: 0xC901 tag
/// - bits 47..36: threat centroid x tile (u12)
/// - bits 35..24: threat centroid z tile (u12)
/// - bits 23..16: doctrine mode (u8)
/// - bits 15..8 : dominant stance (u8)
/// - bits 7..0  : generation lsb (u8)
pub fn encode_battle_ai_intent_payload(
    threat_x_tile: u16,
    threat_z_tile: u16,
    doctrine_mode: u8,
    dominant_stance: u8,
    generation_lsb: u8,
) -> u64 {
    let tag = 0xC901u64 << 48;
    let x = (threat_x_tile.min(0x0FFF) as u64) << 36;
    let z = (threat_z_tile.min(0x0FFF) as u64) << 24;
    let doctrine = (doctrine_mode as u64) << 16;
    let stance = (dominant_stance as u64) << 8;
    tag | x | z | doctrine | stance | (generation_lsb as u64)
}

/// Encode grenade telemetry: casualties, cover, detonation.
///
/// Layout:
/// - bits 63..48: 0xC700 tag
/// - bits 47..32: expected casualties (u16, clamped)
/// - bits 31..16: avg cover q16 (u16, clamped)
/// - bits 15..0 : detonation ms (u16, clamped)
pub fn encode_grenade_replay_payload(
    casualties: u32,
    avg_cover_q16: u32,
    detonation_ms: u32,
) -> u64 {
    let tag = 0xC700u64 << 48;
    let cas = (casualties.min(0xFFFF) as u64) << 32;
    let cover = (avg_cover_q16.min(0xFFFF) as u64) << 16;
    let det = detonation_ms.min(0xFFFF) as u64;
    tag | cas | cover | det
}

/// Encode garrison overlay snapshot.
///
/// Layout:
/// - bits 63..48: 0xC800 tag
/// - bits 47..32: garrisoned count (u16, clamped)
/// - bits 31..16: breached structures (u16, clamped)
/// - bits 15..0 : avg aperture width (deg, Q16.16 truncated to u16)
pub fn encode_garrison_replay_payload(
    garrisoned: u32,
    breached: u32,
    avg_aperture_q16: u32,
) -> u64 {
    let tag = 0xC800u64 << 48;
    let g = (garrisoned.min(0xFFFF) as u64) << 32;
    let b = (breached.min(0xFFFF) as u64) << 16;
    let a = avg_aperture_q16.min(0xFFFF) as u64;
    tag | g | b | a
}

/// Encode per-slot aperture detail (min/max) for garrisons (Q16.16 truncated to u16).
///
/// Layout:
/// - bits 63..48: 0xC801 tag
/// - bits 31..16: min aperture width (u16, Q16.16 truncated)
/// - bits 15..0 : max aperture width (u16, Q16.16 truncated)
pub fn encode_garrison_aperture_detail_payload(min_q16: u32, max_q16: u32) -> u64 {
    let tag = 0xC801u64 << 48;
    let min = (min_q16.min(0xFFFF) as u64) << 16;
    let max = max_q16.min(0xFFFF) as u64;
    tag | min | max
}

/// Encode siege overlay snapshot (integrity/progress + breach/repair counts).
///
/// Layout:
/// - bits 63..48: 0xC820 tag
/// - bits 47..32: avg integrity q16 (u16, clamped)
/// - bits 31..24: breach events (u8)
/// - bits 23..16: repair events (u8)
/// - bits 15..0 : avg breach progress q16 (u16, clamped)
pub fn encode_siege_replay_payload(
    integrity_avg_q16: u32,
    breach_events: u32,
    repair_events: u32,
    progress_q16: u32,
) -> u64 {
    let tag = 0xC820u64 << 48;
    let integrity = (integrity_avg_q16.min(0xFFFF) as u64) << 32;
    let breach = (breach_events.min(0xFF) as u64) << 24;
    let repair = (repair_events.min(0xFF) as u64) << 16;
    let progress = progress_q16.min(0xFFFF) as u64;
    tag | integrity | breach | repair | progress
}

/// Encode a specific siege face event (breach/repair) for replay/telemetry.
///
/// Layout:
/// - bits 63..48: 0xC821 tag
/// - bits 47..32: structure_id (u16)
/// - bits 31..24: face_idx (u8)
/// - bit 23     : breached flag (1 = breach, 0 = repair/seal)
/// - bits 22..8 : integrity_q16 >> 4 (u15, clamped)
/// - bits 7..0  : breach_progress_q16 >> 8 (u8, clamped)
pub fn encode_siege_event_detail(
    structure_id: u32,
    face_idx: u8,
    breached: bool,
    integrity_q16: u32,
    breach_progress_q16: u32,
) -> u64 {
    let tag = 0xC821u64 << 48;
    let sid = (structure_id.min(0xFFFF) as u64) << 32;
    let face = (face_idx.min(0xFF) as u64) << 24;
    let breach_bit = if breached { 1u64 } else { 0 } << 23;
    let integrity = ((integrity_q16 >> 4).min(0x7FFF) as u64) << 8;
    let progress = (breach_progress_q16 >> 8).min(0xFF) as u64;
    tag | sid | face | breach_bit | integrity | progress
}

/// Decoded replay record kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayRecord {
    /// Shock event: casualties + shock penalty/weight.
    Shock {
        casualties: u32,
        shock_penalty_q16: u16,
        shock_weight_delta_q16: u16,
    },
    /// Charge/brace counts.
    Charge {
        orders: u16,
        commits: u16,
        braces: u16,
    },
    /// Supply overlay averages (pressure/fatigue).
    Supply {
        pressure_avg_q16: u16,
        fatigue_avg_q16: u16,
    },
    /// Logistics overlay (throughput + command-delay penalty + disruptions/attrition).
    Logistics {
        throughput_avg_q16: u16,
        command_delay_penalty_ticks: u16,
        disruptions: u8,
        attrition_events: u8,
    },
    /// Supply route cut event (clustered disruptions).
    SupplyRouteCut {
        cuts: u16,
    },
    /// Ops/C2 guardrails snapshot (backpressure + courier reliability).
    OpsGuard {
        backpressure_q16: u16,
        courier_reliability_q16: u16,
    },
    /// Ops backpressure drop count (orders dropped due to caps).
    OpsBackpressureDrops {
        drops: u16,
    },
    /// Ops/C2 congestion severity bucket.
    OpsCongestion {
        bucket: u8,
    },
    /// Command/courier overlay snapshot.
    Command {
        command_stress_q16: u16,
        courier_eta_ticks: u16,
        courier_losses: u8,
        courier_spoofed: u8,
    },
    /// Applied command delay summary.
    CommandDelayApplied {
        count: u16,
        avg_delay_ticks: u16,
    },
    /// Command delay p95 bucket (histogram-derived).
    CommandDelayP95 {
        bucket: u8,
    },
    /// Courier ETA p95 bucket (histogram-derived).
    CourierEtaP95 {
        bucket: u8,
    },
    /// Strategic overlay: ownership/repair events.
    Strategic {
        kind: StrategicEventKind,
        province_id: u16,
        primary: u16,
        secondary: u16,
    },
    /// Strategic metadata (generation + infra delta).
    StrategicMeta {
        province_id: u16,
        generation_lsb: u16,
        infra_delta_q16_shift: u16,
    },
    /// Command delay/courier ETA histogram chunk (4 buckets).
    CommandHistogram {
        kind: CommandHistogramKind,
        chunk: u8,
        buckets: [u16; 4],
    },
    /// Artillery overlay snapshot.
    Artillery {
        ricochet_bounces: u8,
        crater_radius_tiles: u8,
        fuse_ms: u16,
        splash_q16: u16,
    },
    /// Charge corridor/path snapshot (tile coords).
    ChargePath {
        start_x_tile: u16,
        start_z_tile: u16,
        target_x_tile: u16,
        target_z_tile: u16,
        impact_mode: u8,
    },
    /// Doctrine/rank-fire snapshot.
    Doctrine {
        rank_fire_mask: u8,
        doctrine_mode: u8,
        cadence_ticks: u16,
        rank_fire_events: u8,
        advance_fire_events: u8,
    },
    /// Battle AI aggregated intent snapshot.
    BattleAiIntent {
        threat_x_tile: u16,
        threat_z_tile: u16,
        doctrine_mode: u8,
        dominant_stance: u8,
        generation_lsb: u8,
    },
    /// Battle AI decision snapshot.
    BattleAi {
        source_formation_id: u16,
        target_formation_id: u16,
        order_kind: OrderKind,
        score_q8: u8,
    },
    /// Grenade snapshot: casualties/cover/detonation.
    Grenade {
        casualties: u16,
        avg_cover_q16: u16,
        detonation_ms: u16,
    },
    /// Siege overlay snapshot (integrity/progress + breach/repair counts).
    Siege {
        integrity_q16: u16,
        breach_events: u8,
        repair_events: u8,
        progress_q16: u16,
    },
    /// Siege face event detail (breach/repair + integrity/progress).
    SiegeEventDetail {
        structure_id: u16,
        face_idx: u8,
        breached: bool,
        integrity_q16: u16,
        breach_progress_q16: u16,
    },
    /// Garrison overlay snapshot (count, breaches, avg aperture width).
    Garrison {
        garrisoned: u16,
        breached: u16,
        avg_aperture_q16: u16,
    },
    /// Garrison aperture detail snapshot (min/max widths).
    GarrisonDetail {
        min_aperture_q16: u16,
        max_aperture_q16: u16,
    },
    /// Unknown/untagged payload (for forward compatibility).
    Unknown(u64),
}

/// Decode a replay payload into a record enum.
pub fn decode_replay_payload(payload: u64) -> ReplayRecord {
    let tag = payload >> 48;
    match tag {
        0xC100 => ReplayRecord::Charge {
            orders: ((payload >> 32) & 0xFFFF) as u16,
            commits: ((payload >> 16) & 0xFFFF) as u16,
            braces: (payload & 0xFFFF) as u16,
        },
        0xC200 => ReplayRecord::Supply {
            pressure_avg_q16: ((payload >> 32) & 0xFFFF) as u16,
            fatigue_avg_q16: ((payload >> 16) & 0xFFFF) as u16,
        },
        0xC230 => ReplayRecord::Logistics {
            throughput_avg_q16: ((payload >> 32) & 0xFFFF) as u16,
            command_delay_penalty_ticks: ((payload >> 16) & 0xFFFF) as u16,
            disruptions: ((payload >> 8) & 0xFF) as u8,
            attrition_events: (payload & 0xFF) as u8,
        },
        0xC231 => ReplayRecord::SupplyRouteCut {
            cuts: ((payload >> 16) & 0xFFFF) as u16,
        },
        0xC340 => ReplayRecord::OpsGuard {
            backpressure_q16: ((payload >> 32) & 0xFFFF) as u16,
            courier_reliability_q16: ((payload >> 16) & 0xFFFF) as u16,
        },
        0xC342 => ReplayRecord::OpsBackpressureDrops {
            drops: ((payload >> 16) & 0xFFFF) as u16,
        },
        0xC343 => ReplayRecord::OpsCongestion {
            bucket: ((payload >> 24) & 0xFF) as u8,
        },
        0xC300 => ReplayRecord::Command {
            command_stress_q16: ((payload >> 32) & 0xFFFF) as u16,
            courier_eta_ticks: ((payload >> 16) & 0xFFFF) as u16,
            courier_losses: ((payload >> 8) & 0xFF) as u8,
            courier_spoofed: (payload & 0xFF) as u8,
        },
        0xC320 => ReplayRecord::CommandDelayApplied {
            count: ((payload >> 32) & 0xFFFF) as u16,
            avg_delay_ticks: ((payload >> 16) & 0xFFFF) as u16,
        },
        0xC324 => ReplayRecord::CommandDelayP95 {
            bucket: ((payload >> 24) & 0xFF) as u8,
        },
        0xC325 => ReplayRecord::CourierEtaP95 {
            bucket: ((payload >> 24) & 0xFF) as u8,
        },
        0xC310 | 0xC311 => {
            let tag = payload >> 48;
            let buckets = [
                ((payload >> 36) & 0xFFF) as u16,
                ((payload >> 24) & 0xFFF) as u16,
                ((payload >> 12) & 0xFFF) as u16,
                (payload & 0xFFF) as u16,
            ];
            ReplayRecord::CommandHistogram {
                kind: CommandHistogramKind::Delay,
                chunk: if tag == 0xC311 { 1 } else { 0 },
                buckets,
            }
        }
        0xC312 | 0xC313 => {
            let tag = payload >> 48;
            let buckets = [
                ((payload >> 36) & 0xFFF) as u16,
                ((payload >> 24) & 0xFFF) as u16,
                ((payload >> 12) & 0xFFF) as u16,
                (payload & 0xFFF) as u16,
            ];
            ReplayRecord::CommandHistogram {
                kind: CommandHistogramKind::Eta,
                chunk: if tag == 0xC313 { 1 } else { 0 },
                buckets,
            }
        }
        0xCA00 => {
            let kind_raw = ((payload >> 40) & 0xFF) as u8;
            let province_id = ((payload >> 24) & 0xFFFF) as u16;
            let primary = ((payload >> 12) & 0xFFF) as u16;
            let secondary = (payload & 0xFFF) as u16;
            let kind = StrategicEventKind::from_u8(kind_raw)
                .unwrap_or(StrategicEventKind::OwnershipChange);
            ReplayRecord::Strategic {
                kind,
                province_id,
                primary,
                secondary,
            }
        }
        0xCA01 => {
            let province_id = ((payload >> 32) & 0xFFFF) as u16;
            let generation_lsb = ((payload >> 16) & 0xFFFF) as u16;
            let infra_delta_q16_shift = (payload & 0xFFFF) as u16;
            ReplayRecord::StrategicMeta {
                province_id,
                generation_lsb,
                infra_delta_q16_shift,
            }
        }
        0xC400 => ReplayRecord::Artillery {
            ricochet_bounces: ((payload >> 40) & 0xFF) as u8,
            crater_radius_tiles: ((payload >> 32) & 0xFF) as u8,
            fuse_ms: ((payload >> 16) & 0xFFFF) as u16,
            splash_q16: (payload & 0xFFFF) as u16,
        },
        0xC500 => ReplayRecord::ChargePath {
            start_x_tile: ((payload >> 37) & 0x7FF) as u16,
            start_z_tile: ((payload >> 26) & 0x7FF) as u16,
            target_x_tile: ((payload >> 15) & 0x7FF) as u16,
            target_z_tile: ((payload >> 4) & 0x7FF) as u16,
            impact_mode: (payload & 0xF) as u8,
        },
        0xC600 => ReplayRecord::Doctrine {
            rank_fire_mask: ((payload >> 40) & 0xFF) as u8,
            doctrine_mode: ((payload >> 32) & 0xFF) as u8,
            cadence_ticks: ((payload >> 16) & 0xFFFF) as u16,
            rank_fire_events: ((payload >> 8) & 0xFF) as u8,
            advance_fire_events: (payload & 0xFF) as u8,
        },
        0xC901 => ReplayRecord::BattleAiIntent {
            threat_x_tile: ((payload >> 36) & 0xFFF) as u16,
            threat_z_tile: ((payload >> 24) & 0xFFF) as u16,
            doctrine_mode: ((payload >> 16) & 0xFF) as u8,
            dominant_stance: ((payload >> 8) & 0xFF) as u8,
            generation_lsb: (payload & 0xFF) as u8,
        },
        0xC900 => match OrderKind::from_u8(((payload >> 8) & 0xFF) as u8) {
            Some(order_kind) => ReplayRecord::BattleAi {
                source_formation_id: ((payload >> 32) & 0xFFFF) as u16,
                target_formation_id: ((payload >> 16) & 0xFFFF) as u16,
                order_kind,
                score_q8: (payload & 0xFF) as u8,
            },
            None => ReplayRecord::Unknown(payload),
        },
        0xC700 => ReplayRecord::Grenade {
            casualties: ((payload >> 32) & 0xFFFF) as u16,
            avg_cover_q16: ((payload >> 16) & 0xFFFF) as u16,
            detonation_ms: (payload & 0xFFFF) as u16,
        },
        0xC820 => ReplayRecord::Siege {
            integrity_q16: ((payload >> 32) & 0xFFFF) as u16,
            breach_events: ((payload >> 24) & 0xFF) as u8,
            repair_events: ((payload >> 16) & 0xFF) as u8,
            progress_q16: (payload & 0xFFFF) as u16,
        },
        0xC821 => {
            let structure_id = ((payload >> 32) & 0xFFFF) as u16;
            let face_idx = ((payload >> 24) & 0xFF) as u8;
            let breached = ((payload >> 23) & 0x1) != 0;
            let integrity_q16 = (((payload >> 8) & 0x7FFF) as u16) << 4;
            let breach_progress_q16 = ((payload & 0xFF) as u16) << 8;
            ReplayRecord::SiegeEventDetail {
                structure_id,
                face_idx,
                breached,
                integrity_q16,
                breach_progress_q16,
            }
        }
        0xC800 => ReplayRecord::Garrison {
            garrisoned: ((payload >> 32) & 0xFFFF) as u16,
            breached: ((payload >> 16) & 0xFFFF) as u16,
            avg_aperture_q16: (payload & 0xFFFF) as u16,
        },
        0xC801 => ReplayRecord::GarrisonDetail {
            min_aperture_q16: ((payload >> 16) & 0xFFFF) as u16,
            max_aperture_q16: (payload & 0xFFFF) as u16,
        },
        _ => ReplayRecord::Shock {
            casualties: (payload >> 32) as u32,
            shock_penalty_q16: ((payload >> 16) & 0xFFFF) as u16,
            shock_weight_delta_q16: (payload & 0xFFFF) as u16,
        },
    }
}

/// Decode a list of replay events into typed records.
pub fn decode_events(events: &[ReplayEvent]) -> Vec<(u64, ReplayRecord)> {
    events
        .iter()
        .map(|ev| (ev.tick, decode_replay_payload(ev.payload)))
        .collect()
}

/// Extract supply samples (tick, pressure, fatigue) from a decoded stream.
pub fn supply_series(events: &[(u64, ReplayRecord)]) -> Vec<(u64, u16, u16)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::Supply {
                pressure_avg_q16,
                fatigue_avg_q16,
            } => Some((*tick, *pressure_avg_q16, *fatigue_avg_q16)),
            _ => None,
        })
        .collect()
}

/// Extract grenade timeline: (tick, casualties, cover_q16, detonation_ms).
pub fn grenade_series(events: &[(u64, ReplayRecord)]) -> Vec<(u64, u16, u16, u16)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::Grenade {
                casualties,
                avg_cover_q16,
                detonation_ms,
            } => Some((*tick, *casualties, *avg_cover_q16, *detonation_ms)),
            _ => None,
        })
        .collect()
}

/// Extract doctrine/rank-fire samples from a decoded stream.
pub fn doctrine_series(events: &[(u64, ReplayRecord)]) -> Vec<(u64, u8, u8, u16, u8, u8)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::Doctrine {
                rank_fire_mask,
                doctrine_mode,
                cadence_ticks,
                rank_fire_events,
                advance_fire_events,
            } => Some((
                *tick,
                *rank_fire_mask,
                *doctrine_mode,
                *cadence_ticks,
                *rank_fire_events,
                *advance_fire_events,
            )),
            _ => None,
        })
        .collect()
}

/// Extract battle AI intent timeline: (tick, x_tile, z_tile, doctrine, stance, gen_lsb).
pub fn battle_ai_intent_series(
    events: &[(u64, ReplayRecord)],
) -> Vec<(u64, u16, u16, u8, u8, u8)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::BattleAiIntent {
                threat_x_tile,
                threat_z_tile,
                doctrine_mode,
                dominant_stance,
                generation_lsb,
            } => Some((
                *tick,
                *threat_x_tile,
                *threat_z_tile,
                *doctrine_mode,
                *dominant_stance,
                *generation_lsb,
            )),
            _ => None,
        })
        .collect()
}

/// Extract battle AI decision timeline: (tick, src_id, tgt_id, order, score_q8).
pub fn battle_ai_series(events: &[(u64, ReplayRecord)]) -> Vec<(u64, u16, u16, OrderKind, u8)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::BattleAi {
                source_formation_id,
                target_formation_id,
                order_kind,
                score_q8,
            } => Some((
                *tick,
                *source_formation_id,
                *target_formation_id,
                *order_kind,
                *score_q8,
            )),
            _ => None,
        })
        .collect()
}

/// Extract siege face events: (tick, structure_id, face_idx, breached, integrity_q16, progress_q16).
pub fn siege_event_series(
    events: &[(u64, ReplayRecord)],
) -> Vec<(u64, u16, u8, bool, u16, u16)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::SiegeEventDetail {
                structure_id,
                face_idx,
                breached,
                integrity_q16,
                breach_progress_q16,
            } => Some((
                *tick,
                *structure_id,
                *face_idx,
                *breached,
                *integrity_q16,
                *breach_progress_q16,
            )),
            _ => None,
        })
        .collect()
}

/// Extract strategic ownership/repair events: (tick, kind, province_id, primary, secondary).
pub fn strategic_series(
    events: &[(u64, ReplayRecord)],
) -> Vec<(u64, StrategicEventKind, u16, u16, u16)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::Strategic {
                kind,
                province_id,
                primary,
                secondary,
            } => Some((*tick, *kind, *province_id, *primary, *secondary)),
            _ => None,
        })
        .collect()
}

/// Extract strategic metadata: (tick, province_id, generation_lsb, infra_delta_q16>>4).
pub fn strategic_meta_series(events: &[(u64, ReplayRecord)]) -> Vec<(u64, u16, u16, u16)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::StrategicMeta {
                province_id,
                generation_lsb,
                infra_delta_q16_shift,
            } => Some((*tick, *province_id, *generation_lsb, *infra_delta_q16_shift)),
            _ => None,
        })
        .collect()
}

/// Extract applied command delays: (tick, count, avg_delay_ticks).
pub fn command_delay_applied_series(events: &[(u64, ReplayRecord)]) -> Vec<(u64, u16, u16)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::CommandDelayApplied {
                count,
                avg_delay_ticks,
            } => Some((*tick, *count, *avg_delay_ticks)),
            _ => None,
        })
        .collect()
}

/// Extract command delay p95 buckets: (tick, bucket).
pub fn command_delay_p95_series(events: &[(u64, ReplayRecord)]) -> Vec<(u64, u8)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::CommandDelayP95 { bucket } => Some((*tick, *bucket)),
            _ => None,
        })
        .collect()
}

/// Extract courier ETA p95 buckets: (tick, bucket).
pub fn courier_eta_p95_series(events: &[(u64, ReplayRecord)]) -> Vec<(u64, u8)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::CourierEtaP95 { bucket } => Some((*tick, *bucket)),
            _ => None,
        })
        .collect()
}

/// Extract ops backpressure drops: (tick, drops).
pub fn ops_backpressure_drops_series(events: &[(u64, ReplayRecord)]) -> Vec<(u64, u16)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::OpsBackpressureDrops { drops } => Some((*tick, *drops)),
            _ => None,
        })
        .collect()
}

/// Extract ops congestion buckets: (tick, bucket).
pub fn ops_congestion_series(events: &[(u64, ReplayRecord)]) -> Vec<(u64, u8)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::OpsCongestion { bucket } => Some((*tick, *bucket)),
            _ => None,
        })
        .collect()
}

pub fn supply_route_cut_series(events: &[(u64, ReplayRecord)]) -> Vec<(u64, u16)> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::SupplyRouteCut { cuts } => Some((*tick, *cuts)),
            _ => None,
        })
        .collect()
}

/// Extract command delay histogram chunks: (tick, chunk_idx, [b0,b1,b2,b3]).
pub fn command_delay_hist_series(
    events: &[(u64, ReplayRecord)],
) -> Vec<(u64, u8, [u16; 4])> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::CommandHistogram {
                kind: CommandHistogramKind::Delay,
                chunk,
                buckets,
            } => Some((*tick, *chunk, *buckets)),
            _ => None,
        })
        .collect()
}

/// Extract courier ETA histogram chunks: (tick, chunk_idx, [b0,b1,b2,b3]).
pub fn courier_eta_hist_series(
    events: &[(u64, ReplayRecord)],
) -> Vec<(u64, u8, [u16; 4])> {
    events
        .iter()
        .filter_map(|(tick, rec)| match rec {
            ReplayRecord::CommandHistogram {
                kind: CommandHistogramKind::Eta,
                chunk,
                buckets,
            } => Some((*tick, *chunk, *buckets)),
            _ => None,
        })
        .collect()
}

/// StratOps-focused replay record for campaign analytics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StratOpsRecord {
    Strategic {
        tick: u64,
        kind: StrategicEventKind,
        province_id: u16,
        primary: u16,
        secondary: u16,
    },
    StrategicMeta {
        tick: u64,
        province_id: u16,
        generation_lsb: u16,
        infra_delta_q16_shift: u16,
    },
    CommandDelayApplied {
        tick: u64,
        count: u16,
        avg_delay_ticks: u16,
    },
    CommandDelayHist {
        tick: u64,
        chunk: u8,
        buckets: [u16; 4],
    },
    CourierEtaHist {
        tick: u64,
        chunk: u8,
        buckets: [u16; 4],
    },
}

/// Build a campaign-focused replay lane from decoded events.
pub fn build_stratops_lane(events: &[(u64, ReplayRecord)]) -> Vec<StratOpsRecord> {
    let mut out = Vec::new();
    for (tick, rec) in events {
        match rec {
            ReplayRecord::Strategic {
                kind,
                province_id,
                primary,
                secondary,
            } => out.push(StratOpsRecord::Strategic {
                tick: *tick,
                kind: *kind,
                province_id: *province_id,
                primary: *primary,
                secondary: *secondary,
            }),
            ReplayRecord::StrategicMeta {
                province_id,
                generation_lsb,
                infra_delta_q16_shift,
            } => out.push(StratOpsRecord::StrategicMeta {
                tick: *tick,
                province_id: *province_id,
                generation_lsb: *generation_lsb,
                infra_delta_q16_shift: *infra_delta_q16_shift,
            }),
            ReplayRecord::CommandDelayApplied {
                count,
                avg_delay_ticks,
            } => out.push(StratOpsRecord::CommandDelayApplied {
                tick: *tick,
                count: *count,
                avg_delay_ticks: *avg_delay_ticks,
            }),
            ReplayRecord::CommandHistogram {
                kind: CommandHistogramKind::Delay,
                chunk,
                buckets,
            } => out.push(StratOpsRecord::CommandDelayHist {
                tick: *tick,
                chunk: *chunk,
                buckets: *buckets,
            }),
            ReplayRecord::CommandHistogram {
                kind: CommandHistogramKind::Eta,
                chunk,
                buckets,
            } => out.push(StratOpsRecord::CourierEtaHist {
                tick: *tick,
                chunk: *chunk,
                buckets: *buckets,
            }),
            _ => {}
        }
    }
    out
}

verify_capsule_properties!(ReplayMmapCapsule, 128, 256);

/// Helper capsule to flush replay logs into mmap with optional index chaining.
#[repr(C, align(128))]
pub struct ReplayFlushCapsule {
    _padding: [u8; 128],
}

impl ReplayFlushCapsule {
    pub const fn new() -> Self {
        Self { _padding: [0; 128] }
    }

    /// Flush drained events into mmap; if `index` is provided, update its hash chain as well.
    pub fn flush_to_mmap<const N: usize>(
        &self,
        log: &ReplayLogCapsule<N>,
        mmap: &mut ReplayMmapCapsule,
        index: Option<&ReplayIndexCapsule>,
    ) -> Result<ReplayPersistSnapshot, MmapError> {
        if let Some(idx) = index {
            mmap.append_from_log_with_index(log, idx)?;
        } else {
            mmap.append_from_log(log)?;
        }
        Ok(mmap.snapshot())
    }
}

verify_capsule_properties!(ReplayFlushCapsule, 128, 128);

/// io_uring-backed writer that batches replay events into a caller-owned buffer and submits writes.
#[cfg(feature = "io-uring")]
#[repr(C, align(128))]
pub struct ReplayIoUringWriterCapsule {
    ring: IoUringCapsule,
    batch: IoUringBatchCapsule,
    fd: i32,
    offset: AtomicU64,
    buffer: Box<[u8]>,
    persist_counters: ReplayPersistCapsule,
}

#[cfg(feature = "io-uring")]
verify_capsule_properties!(
    ReplayIoUringWriterCapsule,
    core::mem::align_of::<ReplayIoUringWriterCapsule>(),
    core::mem::size_of::<ReplayIoUringWriterCapsule>()
);

#[cfg(feature = "io-uring")]
impl ReplayIoUringWriterCapsule {
    /// `entries`/`flags` mirror `IoUringCapsule::new`. `buffer_len` should cover worst-case log flush.
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
            persist_counters: ReplayPersistCapsule::new(),
        })
    }

    /// Mutable access to the buffer for registration/pinning.
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    /// Append drained events via a single batched write; returns user_data id.
    pub fn append_from_log<const N: usize>(
        &mut self,
        log: &ReplayLogCapsule<N>,
    ) -> Result<u64, IoUringError> {
        let events = log.drain();
        if events.is_empty() {
            return Ok(0);
        }
        let bytes_needed = (events.len() * 16) as usize;
        if bytes_needed > self.buffer.len() {
            return Err(IoUringError::InvalidParameters);
        }

        for (i, ev) in events.iter().enumerate() {
            let base = i * 16;
            self.buffer[base..base + 8].copy_from_slice(&ev.tick.to_le_bytes());
            self.buffer[base + 8..base + 16].copy_from_slice(&ev.payload.to_le_bytes());
        }

        let off = self.offset.load(Ordering::Relaxed);
        let ids = self
            .batch
            .batch_write(&[self.fd], &[&self.buffer[..bytes_needed]], &[off])?;
        self.offset.fetch_add(bytes_needed as u64, Ordering::AcqRel);
        self.persist_counters
            .flushed_events
            .fetch_add(events.len() as u64, Ordering::AcqRel);
        self.persist_counters
            .flushed_bytes
            .fetch_add(bytes_needed as u64, Ordering::AcqRel);
        Ok(ids[0])
    }

    pub fn set_offset(&self, offset: u64) {
        self.offset.store(offset, Ordering::Release);
    }

    pub fn snapshot(&self) -> ReplayPersistSnapshot {
        self.persist_counters.snapshot()
    }

    pub fn batch_stats(&self) -> atomic_capsule::runtime::IoUringBatchStats {
        self.batch.stats()
    }
}

/// Example: Create mmap-backed replay and append drained events.
///
/// ```ignore
/// use kindly_engine::{
///     ReplayLogCapsule, ReplayMmapCapsule, ReplayEvent, standard_retry
/// };
/// use std::path::Path;
///
/// // Create a small in-memory log
/// let log: ReplayLogCapsule<8> = ReplayLogCapsule::new();
/// log.record(1, 42);
/// log.record(2, 77);
///
/// // Persist to mmap file (1MB, 1 region)
/// let mmap_capsule = ReplayMmapCapsule::new(Path::new("replay.bin"), 1_048_576, 1)?;
/// mmap_capsule.append_from_log(&log)?;
///
/// // Inspect counters
/// let snap = mmap_capsule.snapshot();
/// assert_eq!(snap.flushed_events, 2);
/// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_drain() {
        let log: ReplayLogCapsule<8> = ReplayLogCapsule::new();
        assert!(log.record(1, 10));
        assert!(log.record(2, 20));
        let events = log.drain();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].tick, 1);
        assert_eq!(events[1].payload, 20);
    }

    #[test]
    fn persist_flushes_events() {
        let log: ReplayLogCapsule<4> = ReplayLogCapsule::new();
        let persist = ReplayPersistCapsule::new();
        log.record(1, 10);
        log.record(2, 20);

        let mut buf: Vec<u8> = Vec::new();
        persist.flush_to_writer(&log, &mut buf).unwrap();
        assert_eq!(buf.len(), 32); // 2 events × 16 bytes
        let snap = persist.snapshot();
        assert_eq!(snap.flushed_events, 2);
    }

    #[test]
    fn mmap_replay_round_trip() {
        let log: ReplayLogCapsule<8> = ReplayLogCapsule::new();
        log.record(10, 111);
        log.record(11, 222);

        let tmp_path = std::env::temp_dir().join("kindly_engine_replay_roundtrip.bin");
        let mut mmap_capsule =
            ReplayMmapCapsule::new(&tmp_path, 1_048_576, 1).expect("create mmap");
        mmap_capsule.append_from_log(&log).expect("append");

        let snap = mmap_capsule.snapshot();
        assert_eq!(snap.flushed_events, 2);
        // Best-effort cleanup.
        let _ = std::fs::remove_file(tmp_path);
    }

    #[test]
    fn replay_index_hashes_frames() {
        let index = ReplayIndexCapsule::new();
        let snap0 = index.snapshot();
        assert_eq!(snap0.frame_count, 0);
        let snap1 = index.record_frame(10, 0xABCD);
        assert_eq!(snap1.frame_count, 1);
        assert_eq!(snap1.last_tick, 10);
        let snap2 = index.record_frame(11, 0xABCD ^ 1);
        assert_eq!(snap2.frame_count, 2);
        assert_eq!(snap2.last_tick, 11);
        assert_ne!(snap1.hash_chain, snap2.hash_chain);
    }

    #[test]
    fn append_with_index_updates_hash_chain() {
        let log: ReplayLogCapsule<8> = ReplayLogCapsule::new();
        log.record(10, 111);
        log.record(11, 222);

        let tmp_path = std::env::temp_dir().join("kindly_engine_replay_index.bin");
        let mut mmap_capsule =
            ReplayMmapCapsule::new(&tmp_path, 1_048_576, 1).expect("create mmap");
        let index = ReplayIndexCapsule::new();
        mmap_capsule
            .append_from_log_with_index(&log, &index)
            .expect("append");
        let snap = index.snapshot();
        assert_eq!(snap.frame_count, 1);
        assert_eq!(snap.last_tick, 11);
        let chain = mmap_capsule.hash_chain();
        assert_ne!(chain, 0);
        let _ = std::fs::remove_file(tmp_path);
    }

    #[test]
    fn replay_flush_capsule_persists_events() {
        use std::path::Path;

        let log: ReplayLogCapsule<8> = ReplayLogCapsule::new();
        log.record(3, 123);
        let tmp_path = std::env::temp_dir().join("kindly_engine_replay_flush.bin");
        let mut mmap_capsule =
            ReplayMmapCapsule::new(&tmp_path, 1_048_576, 1).expect("create mmap");
        let flush = ReplayFlushCapsule::new();
        let snap = flush
            .flush_to_mmap(&log, &mut mmap_capsule, None)
            .expect("flush to mmap");
        assert_eq!(snap.flushed_events, 1);
        let _ = std::fs::remove_file(Path::new(&tmp_path));
    }

    #[test]
    fn supply_payload_round_trips() {
        let payload = encode_supply_replay_payload(50_000, 12_000);
        match decode_replay_payload(payload) {
            ReplayRecord::Supply {
                pressure_avg_q16,
                fatigue_avg_q16,
            } => {
                assert_eq!(pressure_avg_q16, 50_000);
                assert_eq!(fatigue_avg_q16, 12_000);
            }
            other => panic!("expected supply record, got {other:?}"),
        }
    }

    #[test]
    fn battle_ai_intent_round_trips() {
        let payload = encode_battle_ai_intent_payload(0x123, 0x456, 3, 2, 7);
        match decode_replay_payload(payload) {
            ReplayRecord::BattleAiIntent {
                threat_x_tile,
                threat_z_tile,
                doctrine_mode,
                dominant_stance,
                generation_lsb,
            } => {
                assert_eq!(threat_x_tile, 0x123);
                assert_eq!(threat_z_tile, 0x456 & 0x0FFF);
                assert_eq!(doctrine_mode, 3);
                assert_eq!(dominant_stance, 2);
                assert_eq!(generation_lsb, 7);
            }
            other => panic!("expected battle ai intent, got {other:?}"),
        }
    }

    #[test]
    fn logistics_payload_round_trips() {
        let payload = encode_logistics_replay_payload(40_000, 12, 3, 2);
        match decode_replay_payload(payload) {
            ReplayRecord::Logistics {
                throughput_avg_q16,
                command_delay_penalty_ticks,
                disruptions,
                attrition_events,
            } => {
                assert_eq!(throughput_avg_q16, 40_000);
                assert_eq!(command_delay_penalty_ticks, 12);
                assert_eq!(disruptions, 3);
                assert_eq!(attrition_events, 2);
            }
            other => panic!("expected logistics record, got {other:?}"),
        }
    }

    #[test]
    fn ops_guard_payload_round_trips() {
        let payload = encode_ops_guard_payload(20_000, 50_000);
        match decode_replay_payload(payload) {
            ReplayRecord::OpsGuard {
                backpressure_q16,
                courier_reliability_q16,
            } => {
                assert_eq!(backpressure_q16, 20_000);
                assert_eq!(courier_reliability_q16, 50_000);
            }
            other => panic!("expected ops guard record, got {other:?}"),
        }
    }

    #[test]
    fn decode_stream_yields_supply_series() {
        let payload = encode_supply_replay_payload(10_000, 2_000);
        let events = vec![ReplayEvent::new(7, payload)];
        let decoded = decode_events(&events);
        let series = supply_series(&decoded);
        assert_eq!(series, vec![(7, 10_000, 2_000)]);
    }

    #[test]
    fn strategic_and_command_delay_series_roundtrip() {
        use crate::strategic_map::{StrategicEventKind, StrategicEventSnapshot};

        let strat_payload = encode_strategic_event_payload(&StrategicEventSnapshot {
            kind: StrategicEventKind::InfrastructureRepair,
            province_id: 9,
            from_owner_id: 0,
            to_owner_id: 0,
            from_infra_q16: 12_000,
            to_infra_q16: 14_000,
            resistance_q16: 0,
            generation: 0,
        });
        let strat_meta_payload = encode_strategic_event_meta(&StrategicEventSnapshot {
            kind: StrategicEventKind::InfrastructureRepair,
            province_id: 9,
            from_owner_id: 0,
            to_owner_id: 0,
            from_infra_q16: 12_000,
            to_infra_q16: 14_000,
            resistance_q16: 0,
            generation: 0x1234,
        });
        let cmd_delay_payload = encode_command_delay_applied(5, 12);
        let cmd_hist_payload = encode_command_delay_hist_payload(0, &[1, 2, 3, 4]);
        let courier_hist_payload = encode_courier_eta_hist_payload(1, &[5, 6, 7, 8]);

        let events = vec![
            ReplayEvent::new(1, strat_payload),
            ReplayEvent::new(1, strat_meta_payload),
            ReplayEvent::new(2, cmd_delay_payload),
            ReplayEvent::new(3, cmd_hist_payload),
            ReplayEvent::new(4, courier_hist_payload),
        ];

        let decoded = decode_events(&events);
        let strat_series = strategic_series(&decoded);
        assert_eq!(strat_series.len(), 1);
        assert_eq!(strat_series[0].0, 1);
        assert_eq!(strat_series[0].1, StrategicEventKind::InfrastructureRepair);
        assert_eq!(strat_series[0].2, 9);
        assert_eq!(strat_series[0].3, 750); // 12_000 >> 4
        assert_eq!(strat_series[0].4, 875); // 14_000 >> 4

        let applied = command_delay_applied_series(&decoded);
        assert_eq!(applied, vec![(2, 5, 12)]);

        let cmd_hist = command_delay_hist_series(&decoded);
        assert_eq!(cmd_hist, vec![(3, 0, [1, 2, 3, 4])]);

        let courier_hist = courier_eta_hist_series(&decoded);
        assert_eq!(courier_hist, vec![(4, 1, [5, 6, 7, 8])]);

        let stratops = build_stratops_lane(&decoded);
        assert!(matches!(
            stratops[0],
            StratOpsRecord::Strategic {
                tick: 1,
                kind: StrategicEventKind::InfrastructureRepair,
                province_id: 9,
                ..
            }
        ));
        assert!(matches!(
            stratops[1],
            StratOpsRecord::StrategicMeta {
                tick: 1,
                province_id: 9,
                generation_lsb,
                infra_delta_q16_shift,
            } if generation_lsb == 0x1234 && infra_delta_q16_shift == ((14_000 - 12_000) >> 4) as u16
        ));
        assert!(matches!(
            stratops[2],
            StratOpsRecord::CommandDelayApplied {
                tick: 2,
                count: 5,
                avg_delay_ticks: 12
            }
        ));
        assert!(matches!(
            stratops[3],
            StratOpsRecord::CommandDelayHist {
                tick: 3,
                chunk: 0,
                buckets
            } if buckets == [1,2,3,4]
        ));
        assert!(matches!(
            stratops[4],
            StratOpsRecord::CourierEtaHist {
                tick: 4,
                chunk: 1,
                buckets
            } if buckets == [5,6,7,8]
        ));
    }

    #[test]
    fn siege_payload_round_trip() {
        let payload = encode_siege_replay_payload(40_000, 2, 1, 22_000);
        match decode_replay_payload(payload) {
            ReplayRecord::Siege {
                integrity_q16,
                breach_events,
                repair_events,
                progress_q16,
            } => {
                assert_eq!(integrity_q16, 40_000);
                assert_eq!(breach_events, 2);
                assert_eq!(repair_events, 1);
                assert_eq!(progress_q16, 22_000);
            }
            other => panic!("unexpected decode: {:?}", other),
        }
    }

    #[test]
    fn siege_event_detail_round_trip() {
        let payload = encode_siege_event_detail(77, 2, true, 50_000, 12_000);
        match decode_replay_payload(payload) {
            ReplayRecord::SiegeEventDetail {
                structure_id,
                face_idx,
                breached,
                integrity_q16,
                breach_progress_q16,
            } => {
                assert_eq!(structure_id, 77);
                assert_eq!(face_idx, 2);
                assert!(breached);
                assert_eq!(integrity_q16, (50_000 >> 4) << 4);
                assert_eq!(breach_progress_q16, (12_000 >> 8) << 8);
            }
            other => panic!("unexpected decode: {:?}", other),
        }
    }

    #[test]
    fn ops_backpressure_drops_round_trip() {
        let payload = encode_ops_backpressure_drops(777);
        match decode_replay_payload(payload) {
            ReplayRecord::OpsBackpressureDrops { drops } => {
                assert_eq!(drops, 777u16);
            }
            other => panic!("unexpected decode: {:?}", other),
        }
    }

    #[test]
    fn ops_congestion_round_trip() {
        let payload = encode_ops_congestion(5);
        match decode_replay_payload(payload) {
            ReplayRecord::OpsCongestion { bucket } => {
                assert_eq!(bucket, 5);
            }
            other => panic!("unexpected decode: {:?}", other),
        }
    }

    #[test]
    fn command_delay_p95_round_trip() {
        let payload = encode_command_delay_p95(3);
        match decode_replay_payload(payload) {
            ReplayRecord::CommandDelayP95 { bucket } => assert_eq!(bucket, 3),
            other => panic!("unexpected decode: {:?}", other),
        }
    }

    #[test]
    fn courier_eta_p95_round_trip() {
        let payload = encode_courier_eta_p95(4);
        match decode_replay_payload(payload) {
            ReplayRecord::CourierEtaP95 { bucket } => assert_eq!(bucket, 4),
            other => panic!("unexpected decode: {:?}", other),
        }
    }

    #[test]
    fn supply_route_cut_round_trip() {
        let payload = encode_supply_route_cut(9);
        match decode_replay_payload(payload) {
            ReplayRecord::SupplyRouteCut { cuts } => assert_eq!(cuts, 9),
            other => panic!("unexpected decode: {:?}", other),
        }
    }
}
