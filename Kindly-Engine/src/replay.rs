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
    /// Command/courier overlay snapshot.
    Command {
        command_stress_q16: u16,
        courier_eta_ticks: u16,
        courier_losses: u8,
        courier_spoofed: u8,
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
        0xC300 => ReplayRecord::Command {
            command_stress_q16: ((payload >> 32) & 0xFFFF) as u16,
            courier_eta_ticks: ((payload >> 16) & 0xFFFF) as u16,
            courier_losses: ((payload >> 8) & 0xFF) as u8,
            courier_spoofed: (payload & 0xFF) as u8,
        },
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

/// Extract doctrine/rank-fire samples from a decoded stream.
pub fn doctrine_series(
    events: &[(u64, ReplayRecord)],
) -> Vec<(u64, u8, u8, u16, u8, u8)> {
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
verify_capsule_properties!(ReplayIoUringWriterCapsule, 128, 128);

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
        &self,
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
        let mmap_capsule = ReplayMmapCapsule::new(&tmp_path, 1_048_576, 1).expect("create mmap");
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
        let mmap_capsule = ReplayMmapCapsule::new(&tmp_path, 1_048_576, 1).expect("create mmap");
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
        let mmap_capsule = ReplayMmapCapsule::new(&tmp_path, 1_048_576, 1).expect("create mmap");
        let flush = ReplayFlushCapsule::new();
        let snap = flush
            .flush_to_mmap(&log, &mmap_capsule, None)
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
    fn decode_stream_yields_supply_series() {
        let payload = encode_supply_replay_payload(10_000, 2_000);
        let events = vec![ReplayEvent::new(7, payload)];
        let decoded = decode_events(&events);
        let series = supply_series(&decoded);
        assert_eq!(series, vec![(7, 10_000, 2_000)]);
    }
}
