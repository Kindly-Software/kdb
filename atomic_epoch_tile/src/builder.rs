//! Helpers that translate high-level session data into ET-1kB tiles.

use crate::layout::{
    CountersSection, EtTile, HeaderSection, LogEntry, LogSection, SymbolSection, SymbolSlice,
    MINI_LOG_CAPACITY, SYMBOL_SLICE_COUNT,
};
use atomic_event_lockout_map::EcoSnapshot;

/// Grouped inputs that describe the session snapshot captured in an ET tile.
#[derive(Debug, Clone)]
pub struct TileInputs {
    pub header: HeaderInputs,
    pub counters: CountersInputs,
    pub symbols: Vec<SymbolInputs>,
    pub log: LogInputs,
}

impl TileInputs {
    pub fn new(header: HeaderInputs) -> Self {
        Self {
            header,
            counters: CountersInputs::default(),
            symbols: Vec::new(),
            log: LogInputs::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HeaderInputs {
    pub epoch_id: u64,
    pub created_ms: u64,
    pub run_id: u128,
    pub policy_id: u16,
    pub account_id: u16,
    pub tz_id: u8,
    pub symbol_mask: u8,
    pub forbid_after_min_ct: u16,
    pub eod_flat_min_ct: u16,
    pub applied_level: u8,
    pub global_flags: u8,
    pub prev_tile_hash: [u8; 16],
    pub ale_tail_hash: u64,
    pub capsule_digests: [u64; 8],
    pub tile_index: u16,
    pub created_seq_head: u8,
}

#[derive(Debug, Clone)]
pub struct CountersInputs {
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
    pub realized_cents: i64,
    pub unreal_cents: i64,
    pub fees_cents: i64,
    pub slip_mbp_sum: i64,
    pub slip_mbp_abs_sum: u64,
    pub peak_equity_cents: i64,
    pub max_draw_cents: i64,
    pub lat_d2a_quantiles: [u16; 3],
    pub lat_a2f_quantiles: [u16; 3],
    pub rej_rate_bp: u16,
    pub cxl_rate_bp: u16,
    pub loss_bp: u16,
    pub jitter_us: u16,
    pub lat_hist8: [u32; 8],
    pub slip_hist8: [u32; 8],
}

impl Default for CountersInputs {
    fn default() -> Self {
        Self {
            orders_sent: 0,
            acks: 0,
            fills: 0,
            cancels: 0,
            rejects: 0,
            maker_sends: 0,
            taker_sends: 0,
            reduce_only: 0,
            qty_traded: 0,
            trades_won: 0,
            trades_lost: 0,
            realized_cents: 0,
            unreal_cents: 0,
            fees_cents: 0,
            slip_mbp_sum: 0,
            slip_mbp_abs_sum: 0,
            peak_equity_cents: 0,
            max_draw_cents: 0,
            lat_d2a_quantiles: [0; 3],
            lat_a2f_quantiles: [0; 3],
            rej_rate_bp: 0,
            cxl_rate_bp: 0,
            loss_bp: 0,
            jitter_us: 0,
            lat_hist8: [0; 8],
            slip_hist8: [0; 8],
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SymbolInputs {
    pub sym_id: u16,
    pub breaker_level: u8,
    pub flags: u8,
    pub pos_qty: i32,
    pub avg_px_ticks: i32,
    pub realized_cents: i64,
    pub unreal_cents: i64,
    pub rem_daily_loss_cents: u32,
    pub trailing_draw_cents: u32,
    pub spread_ticks: u8,
    pub vol_bp_q8_8: u16,
    pub obi_q1_10: i16,
    pub last_exec_id: u32,
    pub sum_bid_l1_3: u16,
    pub sum_ask_l1_3: u16,
}

#[derive(Debug, Clone)]
pub struct LogInputs {
    pub entries: Vec<LogInputsEntry>,
    pub head: u8,
    pub count: u8,
    pub now_min_ct: u16,
    pub next_lockout_min_ct: u16,
    pub next_resume_min_ct: u16,
    pub eco_action_now: u8,
    pub apm_summary: u32,
}

impl Default for LogInputs {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            head: 0,
            count: 0,
            now_min_ct: 0,
            next_lockout_min_ct: 0,
            next_resume_min_ct: 0,
            eco_action_now: 0,
            apm_summary: 0,
        }
    }
}

impl LogInputs {
    /// Populate the ECO fields using a snapshot from `atomic_event_lockout_map`.
    pub fn apply_eco_snapshot(&mut self, snapshot: &EcoSnapshot) {
        let tail = snapshot.tail();
        self.now_min_ct = tail.now_min_ct;
        self.next_lockout_min_ct = snapshot.next_lockout_minute().unwrap_or_default();
        self.next_resume_min_ct = snapshot.next_resume_minute().unwrap_or_default();
        self.eco_action_now = tail.active_action as u8;
    }
}

#[derive(Debug, Clone, Default)]
pub struct LogInputsEntry {
    pub ts_ms: u32,
    pub event: u8,
    pub actor: u8,
    pub sym_id: u16,
    pub code: i32,
    pub aux: i32,
    pub flags: u8,
}

/// Applies the provided inputs to an ET tile structure.
pub fn populate_tile(tile: &mut EtTile, inputs: &TileInputs) {
    fill_header(&mut tile.header, inputs);
    fill_counters(&mut tile.counters, &inputs.counters);
    fill_symbols(&mut tile.symbols, &inputs.symbols);
    fill_log(
        &mut tile.log,
        &inputs.log,
        tile.header.seq_head,
        tile.header.ver_even,
        inputs.header.tile_index,
    );
}

fn fill_header(header: &mut HeaderSection, inputs: &TileInputs) {
    let hdr = &inputs.header;
    header.epoch_id = hdr.epoch_id;
    header.created_ms = hdr.created_ms;
    header.run_id = hdr.run_id;
    header.policy_id = hdr.policy_id;
    header.account_id = hdr.account_id;
    header.tz_id = hdr.tz_id;
    header.symbol_mask = hdr.symbol_mask;
    header.forbid_after_min_ct = hdr.forbid_after_min_ct;
    header.eod_flat_min_ct = hdr.eod_flat_min_ct;
    header.applied_level = hdr.applied_level;
    header.global_flags = hdr.global_flags;
    header.prev_tile_hash = hdr.prev_tile_hash;
    header.ale_tail_hash = hdr.ale_tail_hash;
    header.capsule_digests = hdr.capsule_digests;
    header.seq_head = inputs.header.created_seq_head;
    header.commit = 0;
    header.ver_even = 0;
}

fn fill_counters(dest: &mut CountersSection, src: &CountersInputs) {
    dest.orders_sent = src.orders_sent;
    dest.acks = src.acks;
    dest.fills = src.fills;
    dest.cancels = src.cancels;
    dest.rejects = src.rejects;
    dest.maker_sends = src.maker_sends;
    dest.taker_sends = src.taker_sends;
    dest.reduce_only = src.reduce_only;
    dest.qty_traded = src.qty_traded;
    dest.trades_won = src.trades_won;
    dest.trades_lost = src.trades_lost;
    dest.realized_cents = src.realized_cents;
    dest.unreal_cents = src.unreal_cents;
    dest.fees_cents = src.fees_cents;
    dest.slip_mbp_sum = src.slip_mbp_sum;
    dest.slip_mbp_abs_sum = src.slip_mbp_abs_sum;
    dest.peak_equity_cents = src.peak_equity_cents;
    dest.max_draw_cents = src.max_draw_cents;
    dest.lat_d2a_us_p50 = src.lat_d2a_quantiles[0];
    dest.lat_d2a_us_p90 = src.lat_d2a_quantiles[1];
    dest.lat_d2a_us_p99 = src.lat_d2a_quantiles[2];
    dest.lat_a2f_us_p50 = src.lat_a2f_quantiles[0];
    dest.lat_a2f_us_p90 = src.lat_a2f_quantiles[1];
    dest.lat_a2f_us_p99 = src.lat_a2f_quantiles[2];
    dest.rej_rate_bp = src.rej_rate_bp;
    dest.cxl_rate_bp = src.cxl_rate_bp;
    dest.loss_bp = src.loss_bp;
    dest.jitter_us = src.jitter_us;
    dest.lat_hist8 = src.lat_hist8;
    dest.slip_hist8 = src.slip_hist8;
}

fn fill_symbols(dest: &mut SymbolSection, symbols: &[SymbolInputs]) {
    for (slot, src) in dest
        .slots
        .iter_mut()
        .zip(symbols.iter().take(SYMBOL_SLICE_COUNT))
    {
        apply_symbol(slot, src);
    }

    // Clear remaining slots if fewer inputs were provided.
    for slot in dest
        .slots
        .iter_mut()
        .skip(symbols.len().min(SYMBOL_SLICE_COUNT))
    {
        *slot = SymbolSlice::default();
    }
}

fn apply_symbol(dest: &mut SymbolSlice, src: &SymbolInputs) {
    dest.sym_id = src.sym_id;
    dest.breaker_level = src.breaker_level;
    dest.flags = src.flags;
    dest.pos_qty = src.pos_qty;
    dest.avg_px_ticks = src.avg_px_ticks;
    dest.realized_cents = src.realized_cents;
    dest.unreal_cents = src.unreal_cents;
    dest.rem_daily_loss_cents = src.rem_daily_loss_cents;
    dest.trailing_draw_cents = src.trailing_draw_cents;
    dest.spread_ticks = src.spread_ticks;
    dest.vol_bp_q8_8 = src.vol_bp_q8_8;
    dest.obi_q1_10 = src.obi_q1_10;
    dest.last_exec_id = src.last_exec_id;
    dest.sum_bid_l1_3 = src.sum_bid_l1_3;
    dest.sum_ask_l1_3 = src.sum_ask_l1_3;
}

fn fill_log(dest: &mut LogSection, src: &LogInputs, seq_head: u8, ver_even: u8, tile_index: u16) {
    for (slot, entry) in dest
        .entries
        .iter_mut()
        .zip(src.entries.iter().take(MINI_LOG_CAPACITY))
    {
        *slot = LogEntry {
            ts_ms: entry.ts_ms,
            event: entry.event,
            actor: entry.actor,
            sym_id: entry.sym_id,
            code: entry.code,
            aux: entry.aux,
            flags: entry.flags,
            pad: [0; 7],
        };
    }

    for slot in dest
        .entries
        .iter_mut()
        .skip(src.entries.len().min(MINI_LOG_CAPACITY))
    {
        *slot = LogEntry::default();
    }

    dest.tail.mini_head = src.head;
    dest.tail.mini_count = src.count;
    dest.tail.now_min_ct = src.now_min_ct;
    dest.tail.next_lockout_min_ct = src.next_lockout_min_ct;
    dest.tail.next_resume_min_ct = src.next_resume_min_ct;
    dest.tail.eco_action_now = src.eco_action_now;
    dest.tail.apm_summary = src.apm_summary;
    dest.tail.ver_tail = ver_even;
    dest.tail.seq_tail = seq_head;
    dest.tail.tile_index = tile_index;
}
