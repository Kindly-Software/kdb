use std::collections::VecDeque;

use crate::{
    capsule::pack_header, layout, Dos1024, Dos1024Snapshot, DosHeader, DosInstrument,
    DosInstrumentDerived, DosInstrumentHeader, DosLevel, DosSummary,
};

/// Input for a single depth level.
#[derive(Debug, Clone, Copy)]
pub struct LevelInput {
    /// Price in ticks relative to tick zero.
    pub px_ticks: i32,
    /// Displayed quantity.
    pub qty: u32,
}

impl LevelInput {
    /// Convert into a packed level applying clamping.
    fn to_level(self) -> DosLevel {
        DosLevel {
            px_ticks: self
                .px_ticks
                .clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16,
            qty: layout::clamp_qty(self.qty),
        }
    }
}

/// Input payload for one instrument.
#[derive(Debug, Clone)]
pub struct InstrumentInput {
    /// Metadata header.
    pub header: DosInstrumentHeader,
    /// Bid levels ordered from L1 to L5.
    pub bids: [LevelInput; 5],
    /// Ask levels ordered from L1 to L5.
    pub asks: [LevelInput; 5],
    /// Cumulative marketable volume over the recent sweep window (optional).
    pub recent_marketable_volume: Option<u32>,
}

impl InstrumentInput {
    fn to_instrument(&self) -> DosInstrument {
        let bids = self.bids.map(LevelInput::to_level);
        let asks = self.asks.map(LevelInput::to_level);
        let sum_bid = bids
            .iter()
            .take(3)
            .fold(0u32, |acc, level| acc + u32::from(level.qty));
        let sum_ask = asks
            .iter()
            .take(3)
            .fold(0u32, |acc, level| acc + u32::from(level.qty));
        DosInstrument {
            header: self.header,
            bids,
            asks,
            sum_bid_l1_3: layout::clamp_qty(sum_bid),
            sum_ask_l1_3: layout::clamp_qty(sum_ask),
        }
    }
}

/// Global writer input for a capsule update.
#[derive(Debug, Clone)]
pub struct WriterInput {
    /// Wall-clock timestamp in milliseconds.
    pub now_ms: u64,
    /// Logical creation timestamp (usually identical to `now_ms`).
    pub created_ms: u64,
    /// Instrument identifiers for slots A and B.
    pub sym_a_id: u16,
    /// Instrument identifier for symbol B.
    pub sym_b_id: u16,
    /// Minutes-after-open guard.
    pub forbid_after_min_ct: u16,
    /// Minutes until end-of-day flatten complete.
    pub eod_flat_min_ct: u16,
    /// Session flags.
    pub flags: u16,
    /// Reserved spare bits.
    pub spare: u16,
    /// Optional external stale flag.
    pub force_stale: bool,
    /// Instrument A payload.
    pub instrument_a: InstrumentInput,
    /// Instrument B payload.
    pub instrument_b: InstrumentInput,
}

/// Writer configuration parameters.
#[derive(Debug, Clone)]
pub struct WriterConfig {
    /// Age budget in milliseconds before `stale` flips.
    pub staleness_budget_ms: u64,
    /// Minimum absolute tick change to trigger sweep detection.
    pub sweep_mid_jump_ticks: i32,
    /// Time window in milliseconds for the mid jump condition.
    pub sweep_mid_window_ms: u64,
    /// Required shrink ratio numerator (default 1) for the top-of-book depth.
    pub sweep_shrink_ratio_num: u32,
    /// Required shrink ratio denominator (default 2) for the depth comparison.
    pub sweep_shrink_ratio_den: u32,
    /// Sweep decay horizon in milliseconds.
    pub sweep_decay_ms: u64,
    /// Marketable volume ratio numerator (default 3 => 1.5x when combined with denominator 2).
    pub marketable_ratio_num: u32,
    /// Marketable volume ratio denominator (default 2).
    pub marketable_ratio_den: u32,
    /// Trend look-back horizon in milliseconds.
    pub trend_lookback_ms: u64,
    /// Maximum number of mid-price samples retained.
    pub trend_capacity: usize,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            staleness_budget_ms: 250,
            sweep_mid_jump_ticks: 2,
            sweep_mid_window_ms: 150,
            sweep_shrink_ratio_num: 1,
            sweep_shrink_ratio_den: 2,
            sweep_decay_ms: 200,
            marketable_ratio_num: 3,
            marketable_ratio_den: 2,
            trend_lookback_ms: 200,
            trend_capacity: 64,
        }
    }
}

/// High-level writer that stages DOS capsule updates and performs derived calculations.
#[derive(Debug)]
pub struct DosWriter<'a> {
    capsule: &'a Dos1024,
    config: WriterConfig,
    even_version: u8,
    sequence: u16,
    instrument_state: [InstrumentState; 2],
}

impl<'a> DosWriter<'a> {
    /// Create a new writer bound to a capsule.
    #[must_use]
    pub fn new(capsule: &'a Dos1024, config: WriterConfig) -> Self {
        Self {
            capsule,
            config,
            even_version: 0,
            sequence: 0,
            instrument_state: [InstrumentState::default(), InstrumentState::default()],
        }
    }

    /// Publish a fresh snapshot using the provided input.
    pub fn publish(&mut self, input: WriterInput) {
        let seq = self.sequence.wrapping_add(1);
        self.sequence = seq;

        let odd_version = self.even_version.wrapping_add(1) | 1;
        let even_version = odd_version.wrapping_add(1);
        self.even_version = even_version & !1;

        let created_ms_coarse = layout::quantise_timestamp_ms(input.created_ms);
        let stale = input.force_stale
            || input
                .now_ms
                .saturating_sub(layout::dequantise_timestamp_ms(created_ms_coarse))
                > self.config.staleness_budget_ms;

        let instrument_a = input.instrument_a.to_instrument();
        let instrument_b = input.instrument_b.to_instrument();

        let derived_a = self.derive_instrument(0, &instrument_a, &input.instrument_a, input.now_ms);
        let derived_b = self.derive_instrument(1, &instrument_b, &input.instrument_b, input.now_ms);

        let snapshot = Dos1024Snapshot {
            header: DosHeader {
                commit: true,
                stale,
                version_even: even_version & !1,
                sequence_head: seq,
                sym_a_id: input.sym_a_id,
                sym_b_id: input.sym_b_id,
                created_ms_coarse,
                forbid_after_min_ct: input.forbid_after_min_ct,
                eod_flat_min_ct: input.eod_flat_min_ct,
                flags: input.flags,
                spare: input.spare,
            },
            instrument_a,
            instrument_b,
            summary: DosSummary {
                instrument_a: derived_a,
                instrument_b: derived_b,
                checksum16: 0,
                ver_tail: odd_version,
                seq_tail: seq,
            },
        };

        let packed = snapshot.pack();
        let words = packed.words();

        // Stage odd header (commit cleared, version odd) before publishing the body words.
        let staging_header = DosHeader {
            commit: false,
            stale,
            version_even: odd_version,
            sequence_head: seq,
            sym_a_id: input.sym_a_id,
            sym_b_id: input.sym_b_id,
            created_ms_coarse,
            forbid_after_min_ct: input.forbid_after_min_ct,
            eod_flat_min_ct: input.eod_flat_min_ct,
            flags: input.flags,
            spare: input.spare,
        };
        let staging_header_word = pack_header(&staging_header);
        self.capsule.store_header_relaxed(staging_header_word);

        for (idx, word) in words.iter().enumerate().skip(1) {
            self.capsule.store_body_relaxed(idx, *word);
        }
        self.capsule.store_header_release(words[0]);
    }

    fn derive_instrument(
        &mut self,
        slot: usize,
        instrument: &DosInstrument,
        input: &InstrumentInput,
        now_ms: u64,
    ) -> DosInstrumentDerived {
        let bids = &instrument.bids;
        let asks = &instrument.asks;
        let bid_l1 = i32::from(bids[0].px_ticks);
        let ask_l1 = i32::from(asks[0].px_ticks);
        let spread = (ask_l1 - bid_l1).max(0).min(i32::from(u8::MAX)) as u8;

        let sum_bid = u64::from(instrument.sum_bid_l1_3);
        let sum_ask = u64::from(instrument.sum_ask_l1_3);
        let obi = layout::obi_from_depths(sum_bid, sum_ask);

        let micro_off = microprice_offset(bids[0], asks[0]);

        let mid = (bid_l1 + ask_l1) / 2;
        let top_qty = u32::from(bids[0].qty) + u32::from(asks[0].qty);
        let sweep = self.instrument_state[slot].update_sweep(
            now_ms,
            mid,
            top_qty,
            input.recent_marketable_volume,
            &self.config,
        );
        let trend = self.instrument_state[slot].update_trend(mid, now_ms, &self.config);

        DosInstrumentDerived {
            spread_ticks: spread,
            obi_q1_10: obi,
            micro_off_ticks: micro_off,
            sweep_flag: sweep,
            trend_200ms_ticks: trend,
        }
    }
}

#[derive(Debug, Default, Clone)]
struct InstrumentState {
    last_mid_ticks: Option<i32>,
    last_mid_ms: Option<u64>,
    last_top_qty: Option<u32>,
    sweep_until_ms: u64,
    mid_history: VecDeque<(u64, i32)>,
}

impl InstrumentState {
    fn update_sweep(
        &mut self,
        now_ms: u64,
        mid: i32,
        top_qty: u32,
        recent_volume: Option<u32>,
        cfg: &WriterConfig,
    ) -> bool {
        let mut sweep_active = now_ms < self.sweep_until_ms;

        if let Some(prev_mid) = self.last_mid_ticks {
            let dt = now_ms.saturating_sub(self.last_mid_ms.unwrap_or(now_ms));
            let delta = (mid - prev_mid).unsigned_abs() as i32;
            if delta >= cfg.sweep_mid_jump_ticks && dt <= cfg.sweep_mid_window_ms {
                if let Some(prev_qty) = self.last_top_qty {
                    if prev_qty > 0
                        && u64::from(top_qty) * u64::from(cfg.sweep_shrink_ratio_den)
                            <= u64::from(prev_qty) * u64::from(cfg.sweep_shrink_ratio_num)
                    {
                        sweep_active = true;
                    }
                }
            }
        }

        if !sweep_active {
            if let (Some(volume), true) = (recent_volume, top_qty > 0) {
                if u64::from(volume) * u64::from(cfg.marketable_ratio_den)
                    >= u64::from(top_qty) * u64::from(cfg.marketable_ratio_num)
                {
                    sweep_active = true;
                }
            }
        }

        if sweep_active {
            self.sweep_until_ms = now_ms.saturating_add(cfg.sweep_decay_ms);
        } else if now_ms >= self.sweep_until_ms {
            self.sweep_until_ms = now_ms;
        }

        self.last_mid_ticks = Some(mid);
        self.last_mid_ms = Some(now_ms);
        self.last_top_qty = Some(top_qty);

        sweep_active || now_ms < self.sweep_until_ms
    }

    fn update_trend(&mut self, mid: i32, now_ms: u64, cfg: &WriterConfig) -> i16 {
        let cutoff = now_ms.saturating_sub(cfg.trend_lookback_ms);
        while let Some(&(ts, _)) = self.mid_history.front() {
            if ts + cfg.trend_lookback_ms + 100 < now_ms
                || self.mid_history.len() > cfg.trend_capacity
            {
                self.mid_history.pop_front();
            } else {
                break;
            }
        }

        let reference_mid = self
            .mid_history
            .iter()
            .rev()
            .find(|(ts, _)| *ts <= cutoff)
            .or_else(|| self.mid_history.front())
            .map_or(mid, |(_, value)| *value);

        self.mid_history.push_back((now_ms, mid));
        if self.mid_history.len() > cfg.trend_capacity {
            self.mid_history.pop_front();
        }

        layout::clamp_s11(mid - reference_mid)
    }
}

fn microprice_offset(bid: DosLevel, ask: DosLevel) -> i16 {
    let bid_px = i64::from(bid.px_ticks);
    let ask_px = i64::from(ask.px_ticks);
    let bid_qty = i64::from(bid.qty);
    let ask_qty = i64::from(ask.qty);
    let total = bid_qty + ask_qty;
    if total == 0 {
        return 0;
    }
    let micro = (ask_px * bid_qty + bid_px * ask_qty) / total;
    let mid = (bid_px + ask_px) / 2;
    layout::clamp_s12((micro - mid) as i32)
}
