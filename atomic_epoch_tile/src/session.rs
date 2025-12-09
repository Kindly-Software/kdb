use core::sync::atomic::Ordering;

use atomic_breaker::AtomicBreakerSWeMR;
use atomic_cost_tracker::{layout::ActWord, writer::ActSlot};
use atomic_execution_bundle::AtomicExecutionBundle;
use atomic_latency_ticket::{AltAtomic, AltQuantized};
use atomic_portfolio_map::{
    layout::{ApmSnapshot, ApmWords},
    slot::ApmSlot,
};
use atomic_position_capsule::{
    AtomicPositionCapsule, EquityWord, PositionHeadWord, SessionWord, Snapshot as ApcSnapshot,
    TailWord, BREAKER_REDUCE_ONLY_LEVEL, FLAG_HALT, FLAG_LOCKED,
};
use atomic_pre_execution_capsule::PexCapsule;
use atomic_risk_envelope::AtomicRiskEnvelope;
use atomic_risk_ladder_table::Rlt1024;
use atomic_venue_snapshot::{Avs128, Avs128Snapshot};

use crate::builder::{
    populate_tile, CountersInputs, HeaderInputs, LogInputs, SymbolInputs, TileInputs,
};
use crate::integrity::TileHash;
use crate::ring::{FlushStrategy, TileRing};
use crate::writer::{TilePublisher, TileShadow};

/// Flags recorded in `SymbolInputs::flags` for downstream classifiers.
pub const SYMBOL_FLAG_CAN_SCALE: u8 = 0b0000_0001;
/// Symbol is in reduce-only posture.
pub const SYMBOL_FLAG_REDUCE_ONLY: u8 = 0b0000_0010;
/// Symbol is locked out (breaches, halts, or violations).
pub const SYMBOL_FLAG_LOCKOUT: u8 = 0b0000_0100;

pub(crate) const APM_SUMMARY_BREAKER_BITS: u32 = 2;
pub(crate) const APM_SUMMARY_SYMBOL_BITS: u32 = 4;
pub(crate) const APM_SUMMARY_FLAGS_BITS: u32 = 10;
pub(crate) const APM_SUMMARY_HEADROOM_SCALE_CENTS: u32 = 10_000;
pub(crate) const APM_SUMMARY_BREAKER_SHIFT: u32 = 0;
pub(crate) const APM_SUMMARY_SYMBOL_SHIFT: u32 =
    APM_SUMMARY_BREAKER_SHIFT + APM_SUMMARY_BREAKER_BITS;
pub(crate) const APM_SUMMARY_FLAGS_SHIFT: u32 = APM_SUMMARY_SYMBOL_SHIFT + APM_SUMMARY_SYMBOL_BITS;
pub(crate) const APM_SUMMARY_HEADROOM_SHIFT: u32 = APM_SUMMARY_FLAGS_SHIFT + APM_SUMMARY_FLAGS_BITS;
pub(crate) const APM_SUMMARY_BREAKER_MASK: u32 = (1 << APM_SUMMARY_BREAKER_BITS) - 1;
pub(crate) const APM_SUMMARY_SYMBOL_MASK: u32 = (1 << APM_SUMMARY_SYMBOL_BITS) - 1;
pub(crate) const APM_SUMMARY_FLAGS_MASK: u32 = (1 << APM_SUMMARY_FLAGS_BITS) - 1;

/// Handles loading atomic capsules and mapping them into ET tile inputs.
pub struct LiveFeeds<'a> {
    pub position: &'a AtomicPositionCapsule,
    pub venue: Option<&'a Avs128>,
    pub latency: Option<&'a AltAtomic>,
    pub breaker: Option<&'a AtomicBreakerSWeMR>,
    pub cost_tracker: Option<&'a ActSlot>,
    pub risk_envelope: Option<&'a AtomicRiskEnvelope>,
    pub portfolio_map: Option<&'a ApmSlot>,
    pub pre_execution: Option<&'a PexCapsule>,
    pub execution_bundle: Option<&'a AtomicExecutionBundle>,
    pub risk_ladder: Option<&'a Rlt1024>,
    pub router_metrics: Option<RouterMetrics>,
}

/// Router/order-flow statistics used to hydrate the counters section.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RouterMetrics {
    pub orders_sent: u32,
    pub acks: u32,
    pub fills: u32,
    pub cancels: u32,
    pub rejects: u32,
    pub maker_sends: u32,
    pub taker_sends: u32,
    pub reduce_only: u32,
    pub qty_traded: i32,
    pub trades_won: u32,
    pub trades_lost: u32,
    pub fees_cents: i64,
    pub slip_mbp_sum: i64,
    pub slip_mbp_abs_sum: u64,
}

/// Metadata provided by the session daemon when assembling a tile.
#[derive(Clone, Copy)]
pub struct SessionMetadata {
    pub epoch_id: u64,
    pub created_ms: u64,
    pub run_id: u128,
    pub policy_id: u16,
    pub tz_id: u8,
    pub tile_index: u16,
    pub ale_tail_hash: u64,
    pub prev_tile_hash: [u8; 16],
    pub capsule_digests: [u64; 8],
}

/// Builds `TileInputs` from live APC/AVS/ALT feeds.
pub fn build_tile_inputs(
    feeds: &LiveFeeds<'_>,
    meta: SessionMetadata,
    mut counters: CountersInputs,
    mut log: LogInputs,
) -> Option<TileInputs> {
    let position_snapshot = feeds.position.load()?;
    let head = position_snapshot.head();
    let equity = position_snapshot.equity();
    let session = position_snapshot.session();
    let tail = position_snapshot.tail();

    let avs_snapshot = feeds.venue.map(|avs| avs.load_relaxed().unpack());
    let alt_quantized = feeds.latency.map(|alt| alt.load_relaxed().quantized());
    let apm_words = feeds.portfolio_map.and_then(|slot| slot.load_relaxed());

    hydrate_counters_from_equity(&mut counters, &equity);
    if let Some(alt) = alt_quantized {
        hydrate_counters_from_latency(&mut counters, alt);
    }
    if let Some(metrics) = feeds.router_metrics {
        hydrate_counters_from_router(&mut counters, &metrics);
    }

    hydrate_log_from_session(&mut log, &session);
    if let Some(ref words) = apm_words {
        log.apm_summary = summarize_apm(words);
    }

    let mut symbol = SymbolInputs::default();
    hydrate_symbol_from_position(&mut symbol, &head, &equity, &tail);
    if let Some(avs) = avs_snapshot {
        hydrate_symbol_from_venue(&mut symbol, avs);
    }

    let mut capsule_digests = meta.capsule_digests;
    hydrate_capsule_digests(
        &mut capsule_digests,
        feeds,
        &position_snapshot,
        apm_words.as_ref(),
    );

    let header = HeaderInputs {
        epoch_id: meta.epoch_id,
        created_ms: meta.created_ms,
        run_id: meta.run_id,
        policy_id: meta.policy_id,
        account_id: tail.account_id,
        tz_id: meta.tz_id,
        symbol_mask: 0b0001,
        forbid_after_min_ct: session.forbid_after_min_ct,
        eod_flat_min_ct: session.eod_flat_min_ct,
        applied_level: tail.breaker_level,
        global_flags: head.flags,
        prev_tile_hash: meta.prev_tile_hash,
        ale_tail_hash: meta.ale_tail_hash,
        capsule_digests,
        tile_index: meta.tile_index,
        created_seq_head: position_snapshot.sequence() as u8,
    };

    let mut inputs = TileInputs::new(header);
    inputs.counters = counters;
    inputs.symbols.push(symbol);
    inputs.log = log;

    Some(inputs)
}

/// Loads live sources, assembles the tile, publishes into the ring, and optionally flushes.
pub fn publish_from_feeds(
    ring: &mut TileRing,
    publisher: &mut TilePublisher,
    shadow: &mut TileShadow,
    feeds: &LiveFeeds<'_>,
    mut meta: SessionMetadata,
    counters: CountersInputs,
    log: LogInputs,
    flush: Option<FlushStrategy>,
) -> anyhow::Result<Option<crate::writer::CommitOutcome>> {
    meta.prev_tile_hash = publisher.prev_tile_hash();
    let inputs = match build_tile_inputs(feeds, meta, counters, log) {
        Some(inputs) => inputs,
        None => return Ok(None),
    };

    shadow.reset();
    populate_tile(shadow.tile_mut(), &inputs);

    let slot = ring.tile_slot(meta.tile_index as usize);
    let outcome = publisher.publish_into(slot, shadow);

    if let Some(strategy) = flush {
        ring.flush(strategy)?;
    }

    Ok(Some(outcome))
}

fn hydrate_symbol_from_position(
    symbol: &mut SymbolInputs,
    head: &PositionHeadWord,
    equity: &EquityWord,
    tail: &TailWord,
) {
    symbol.sym_id = tail.symbol_id;
    symbol.breaker_level = tail.breaker_level;
    symbol.flags = derive_symbol_flags(head.flags, tail);
    symbol.pos_qty = head.position_qty;
    symbol.avg_px_ticks = head.avg_px_ticks;
    symbol.realized_cents = equity.realized_cents.into();
    symbol.unreal_cents = equity.unrealized_cents.into();
    symbol.rem_daily_loss_cents = head.remaining_daily_loss_cents;
    symbol.trailing_draw_cents = equity.trailing_draw_cents;
    symbol.last_exec_id = tail.last_exec_id;
}

fn hydrate_symbol_from_venue(symbol: &mut SymbolInputs, avs: Avs128Snapshot) {
    symbol.spread_ticks = avs.spread_ticks;
    symbol.vol_bp_q8_8 = avs.vol_bp_q8_8;
    symbol.obi_q1_10 = avs.obi_q1_10;
    symbol.sum_bid_l1_3 = avs.sum_bid_l1_3;
    symbol.sum_ask_l1_3 = avs.sum_ask_l1_3;
}

fn hydrate_counters_from_equity(counters: &mut CountersInputs, equity: &EquityWord) {
    counters.realized_cents = equity.realized_cents.into();
    counters.unreal_cents = equity.unrealized_cents.into();
    counters.peak_equity_cents = equity.peak_equity_cents.into();
    counters.max_draw_cents = -(equity.trailing_draw_cents as i64);
}

fn hydrate_counters_from_latency(counters: &mut CountersInputs, alt: AltQuantized) {
    let d2a_us = dequantize_latency(alt.decision_to_ack_us2);
    let a2f_us = dequantize_latency(alt.ack_to_fill_us2);

    counters.lat_d2a_quantiles = [d2a_us; 3];
    counters.lat_a2f_quantiles = [a2f_us; 3];

    counters.rej_rate_bp = alt.reject_rate_bps;
    counters.cxl_rate_bp = alt.cancel_rate_bps;
    counters.loss_bp = alt.loss_rate_bps;
    counters.jitter_us = dequantize_latency(alt.jitter_us2);
}

fn hydrate_counters_from_router(counters: &mut CountersInputs, metrics: &RouterMetrics) {
    counters.orders_sent = metrics.orders_sent;
    counters.acks = metrics.acks;
    counters.fills = metrics.fills;
    counters.cancels = metrics.cancels;
    counters.rejects = metrics.rejects;
    counters.maker_sends = metrics.maker_sends;
    counters.taker_sends = metrics.taker_sends;
    counters.reduce_only = metrics.reduce_only;
    counters.qty_traded = metrics.qty_traded;
    counters.trades_won = metrics.trades_won;
    counters.trades_lost = metrics.trades_lost;
    counters.fees_cents = metrics.fees_cents;
    counters.slip_mbp_sum = metrics.slip_mbp_sum;
    counters.slip_mbp_abs_sum = metrics.slip_mbp_abs_sum;
}

fn hydrate_log_from_session(log: &mut LogInputs, session: &SessionWord) {
    log.now_min_ct = session.now_min_ct;
    log.next_lockout_min_ct = session.forbid_after_min_ct;
    log.next_resume_min_ct = session.eod_flat_min_ct;
}

fn dequantize_latency(value_us2: u16) -> u16 {
    (u32::from(value_us2) * 2).min(u16::MAX as u32) as u16
}

fn derive_symbol_flags(head_flags: u8, tail: &TailWord) -> u8 {
    let mut flags = 0u8;
    if tail.breaker_level >= BREAKER_REDUCE_ONLY_LEVEL {
        flags |= SYMBOL_FLAG_REDUCE_ONLY;
    }
    if (head_flags & (FLAG_LOCKED | FLAG_HALT)) != 0 || tail.violation_bits != 0 {
        flags |= SYMBOL_FLAG_LOCKOUT;
    }
    if flags == 0 {
        flags |= SYMBOL_FLAG_CAN_SCALE;
    }
    flags
}

fn compute_apc_digest(snapshot: &ApcSnapshot) -> u64 {
    let mut bytes = [0u8; 64];
    for (idx, word) in snapshot.words().iter().enumerate() {
        bytes[idx * 16..(idx + 1) * 16].copy_from_slice(&word.to_le_bytes());
    }
    hash_bytes(&bytes)
}

fn hydrate_capsule_digests(
    digests: &mut [u64; 8],
    feeds: &LiveFeeds<'_>,
    apc: &ApcSnapshot,
    apm_words: Option<&ApmWords>,
) {
    digests[3] = compute_apc_digest(apc);

    if let Some(breaker) = feeds.breaker {
        let packed = breaker.load_relaxed();
        digests[0] = hash_u64(packed);
    }

    if let Some(slot) = feeds.cost_tracker {
        let act_word: ActWord = slot.load_relaxed();
        digests[1] = hash_u128(act_word.raw());
    }

    if let Some(envelope) = feeds.risk_envelope {
        let bits = envelope.load(Ordering::Relaxed).bits();
        digests[2] = hash_u128(bits);
    }

    if let Some(words) = apm_words {
        digests[4] = hash_words(words.as_words());
    }

    if let Some(pex) = feeds.pre_execution {
        if let Some(snapshot) = pex.load_snapshot() {
            digests[5] = hash_words(snapshot.words());
        }
    }

    if let Some(bundle) = feeds.execution_bundle {
        if let Some(snapshot) = bundle.load() {
            digests[6] = hash_words(&snapshot.words());
        }
    }

    if let Some(risk_ladder) = feeds.risk_ladder {
        digests[7] = hash_words(&risk_ladder.into_words());
    }
}

fn hash_u64(value: u64) -> u64 {
    hash_bytes(&value.to_le_bytes())
}

fn hash_u128(value: u128) -> u64 {
    hash_bytes(&value.to_le_bytes())
}

fn hash_words(words: &[u128]) -> u64 {
    let mut buffer = [0u8; 16 * 8];
    let len = words.len();
    for (idx, word) in words.iter().enumerate() {
        buffer[idx * 16..(idx + 1) * 16].copy_from_slice(&word.to_le_bytes());
    }
    hash_bytes(&buffer[..len * 16])
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let digest = TileHash::blake3_64(bytes);
    u64::from_le_bytes(digest)
}

/// Compress key APM header fields into a 32-bit log summary.
///
/// Layout (LSB→MSB):
/// - bits 0..2   : portfolio breaker level (2 bits)
/// - bits 2..6   : active symbol count (4 bits)
/// - bits 6..16  : portfolio flags mask (10 bits)
/// - bits 16..32 : remaining daily loss headroom in 10k-cent buckets (saturating)
fn summarize_apm(words: &ApmWords) -> u32 {
    let snapshot = ApmSnapshot::unpack(words);
    let breaker = (snapshot.header.portfolio_breaker.as_u8() as u32) & APM_SUMMARY_BREAKER_MASK;
    let symbol_count = (snapshot.header.symbol_count as u32) & APM_SUMMARY_SYMBOL_MASK;
    let flags = (snapshot.header.portfolio_flags.bits() as u32) & APM_SUMMARY_FLAGS_MASK;
    let headroom = (snapshot.header.rem_daily_loss_total_cents / APM_SUMMARY_HEADROOM_SCALE_CENTS)
        .min(u16::MAX as u32);

    (headroom << APM_SUMMARY_HEADROOM_SHIFT)
        | (flags << APM_SUMMARY_FLAGS_SHIFT)
        | (symbol_count << APM_SUMMARY_SYMBOL_SHIFT)
        | (breaker << APM_SUMMARY_BREAKER_SHIFT)
}
