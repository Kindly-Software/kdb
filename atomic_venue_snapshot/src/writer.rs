//! Statefull AVS-128 producer that computes snapshot fields from order-book events.

use std::cmp::Ordering as CmpOrdering;
use std::collections::VecDeque;

use crate::{layout, Avs128, Avs128Snapshot};

/// Configuration knobs for the [`AvsWriter`].
#[derive(Debug, Clone, Copy)]
pub struct WriterConfig {
    /// Initial schema version stamped into the snapshot.
    pub version: u8,
    /// EWMA blending factor for short-horizon volatility (0 < α ≤ 1).
    pub vol_alpha: f64,
    /// Basis points contributed by a single tick move at the current price bucket.
    pub bp_per_tick: f64,
    /// Lookback window for the trend metric in milliseconds.
    pub trend_window_ms: u64,
    /// Minimum mid-price move (ticks) that, together with a depth collapse, raises the sweep flag.
    pub sweep_mid_jump_ticks: i64,
    /// Window for sweep detection heuristics in milliseconds.
    pub sweep_window_ms: u64,
    /// Hold time for the sweep flag after detection in milliseconds.
    pub sweep_hold_ms: u64,
    /// Remaining depth ratio that qualifies as a collapse (e.g. 0.5 = >=50% drop).
    pub sweep_collapse_ratio: f64,
    /// Multiple of L1 depth to compare against recent marketable flow.
    pub sweep_volume_factor: f64,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            version: 1,
            vol_alpha: 0.1,
            bp_per_tick: 0.0,
            trend_window_ms: 200,
            sweep_mid_jump_ticks: 2,
            sweep_window_ms: 150,
            sweep_hold_ms: 200,
            sweep_collapse_ratio: 0.5,
            sweep_volume_factor: 1.5,
        }
    }
}

impl WriterConfig {
    /// Clamp parameters into sensible bounds and return the adjusted copy.
    #[must_use]
    pub fn normalised(mut self) -> Self {
        if !(0.0..=1.0).contains(&self.vol_alpha) {
            self.vol_alpha = self.vol_alpha.clamp(0.0, 1.0);
        }
        if self.vol_alpha == 0.0 {
            self.vol_alpha = 0.01;
        }
        if self.bp_per_tick.is_sign_negative() || !self.bp_per_tick.is_finite() {
            self.bp_per_tick = 0.0;
        }
        if self.trend_window_ms == 0 {
            self.trend_window_ms = 200;
        }
        if self.sweep_mid_jump_ticks < 1 {
            self.sweep_mid_jump_ticks = 1;
        }
        if self.sweep_window_ms == 0 {
            self.sweep_window_ms = 100;
        }
        if self.sweep_hold_ms == 0 {
            self.sweep_hold_ms = self.sweep_window_ms.max(50);
        }
        if !self.sweep_collapse_ratio.is_finite() {
            self.sweep_collapse_ratio = 0.5;
        }
        self.sweep_collapse_ratio = self.sweep_collapse_ratio.clamp(0.0, 1.0);
        if !self.sweep_volume_factor.is_finite() || self.sweep_volume_factor <= 0.0 {
            self.sweep_volume_factor = 1.5;
        }
        self
    }
}

/// Input payload describing the current top-of-book event.
#[derive(Debug, Clone, Copy)]
pub struct WriterInput {
    /// Timestamp in milliseconds since the venue session open.
    pub timestamp_ms: u64,
    /// Best bid price expressed in ticks.
    pub bid_px_ticks: i64,
    /// Best ask price expressed in ticks.
    pub ask_px_ticks: i64,
    /// Aggregated bid size for book levels 1, 2, and 3.
    pub bid_sizes: [u32; 3],
    /// Aggregated ask size for book levels 1, 2, and 3.
    pub ask_sizes: [u32; 3],
    /// Marketable flow executed within this update (contracts).
    pub marketable_volume: u32,
}

impl WriterInput {
    /// Construct a `WriterInput` from per-level depth arrays.
    #[must_use]
    pub fn new(
        timestamp_ms: u64,
        bid_px_ticks: i64,
        ask_px_ticks: i64,
        bid_sizes: [u32; 3],
        ask_sizes: [u32; 3],
        marketable_volume: u32,
    ) -> Self {
        Self {
            timestamp_ms,
            bid_px_ticks,
            ask_px_ticks,
            bid_sizes,
            ask_sizes,
            marketable_volume,
        }
    }

    /// Build a `WriterInput` from arbitrary depth slices (L1..L3 extracted, missing levels set to 0).
    #[must_use]
    pub fn from_depth_slices(
        timestamp_ms: u64,
        bid_px_ticks: i64,
        ask_px_ticks: i64,
        bid_levels: &[u32],
        ask_levels: &[u32],
        marketable_volume: u32,
    ) -> Self {
        let mut bid_sizes = [0u32; 3];
        for (dst, src) in bid_sizes.iter_mut().zip(bid_levels.iter().copied()) {
            *dst = src;
        }

        let mut ask_sizes = [0u32; 3];
        for (dst, src) in ask_sizes.iter_mut().zip(ask_levels.iter().copied()) {
            *dst = src;
        }

        Self::new(
            timestamp_ms,
            bid_px_ticks,
            ask_px_ticks,
            bid_sizes,
            ask_sizes,
            marketable_volume,
        )
    }

    /// Update the marketable volume field while reusing an existing `WriterInput` shell.
    #[must_use]
    pub fn with_marketable_volume(mut self, marketable_volume: u32) -> Self {
        self.marketable_volume = marketable_volume;
        self
    }
}

#[derive(Debug, Clone, Copy)]
struct TrendPoint {
    timestamp_ms: u64,
    mid_ticks: f64,
}

#[derive(Debug, Clone, Copy)]
struct MarketableSample {
    timestamp_ms: u64,
    volume: u64,
}

/// AVS-128 writer that computes derived quantities and publishes atomically.
#[derive(Debug)]
pub struct AvsWriter {
    capsule: Avs128,
    cfg: WriterConfig,
    sequence: u8,
    last_mid_ticks: Option<f64>,
    last_mid_timestamp_ms: u64,
    vol_ewma_bp: f64,
    trend_points: VecDeque<TrendPoint>,
    sweep_until_ms: u64,
    last_bid_depth: u64,
    last_ask_depth: u64,
    marketable_window: VecDeque<MarketableSample>,
    rolling_marketable: u64,
}

impl AvsWriter {
    /// Create a new writer with the provided configuration.
    #[must_use]
    pub fn new(config: WriterConfig) -> Self {
        Self {
            capsule: Avs128::new(),
            cfg: config.normalised(),
            sequence: 0,
            last_mid_ticks: None,
            last_mid_timestamp_ms: 0,
            vol_ewma_bp: 0.0,
            trend_points: VecDeque::with_capacity(32),
            sweep_until_ms: 0,
            last_bid_depth: 0,
            last_ask_depth: 0,
            marketable_window: VecDeque::with_capacity(16),
            rolling_marketable: 0,
        }
    }

    /// Borrow the underlying atomically updated capsule for reader registration.
    #[must_use]
    pub fn capsule(&self) -> &Avs128 {
        &self.capsule
    }

    /// Consume the writer and return the owned capsule.
    #[must_use]
    pub fn into_capsule(self) -> Avs128 {
        self.capsule
    }

    /// Update the basis-point-per-tick coefficient (e.g. when price bucket changes).
    pub fn set_bp_per_tick(&mut self, bp_per_tick: f64) {
        if bp_per_tick.is_finite() && bp_per_tick >= 0.0 {
            self.cfg.bp_per_tick = bp_per_tick;
        }
    }

    /// Compute the latest snapshot and publish it with release semantics.
    pub fn publish(&mut self, input: WriterInput) -> Avs128Snapshot {
        let snapshot = self.compose_snapshot(&input);
        self.capsule.publish(snapshot);
        snapshot
    }

    fn compose_snapshot(&mut self, input: &WriterInput) -> Avs128Snapshot {
        let spread_ticks = compute_spread(input);
        let sum_bid = sum_levels(&input.bid_sizes);
        let sum_ask = sum_levels(&input.ask_sizes);
        let obi = layout::obi_from_depths(sum_bid as u64, sum_ask as u64);
        let sum_bid_field = sum_bid.min(u16::MAX as u64) as u16;
        let sum_ask_field = sum_ask.min(u16::MAX as u64) as u16;
        let (mid_ticks, micro_off_ticks) = compute_microprice(input);
        let prev_mid = self.last_mid_ticks;
        let prev_mid_ts = self.last_mid_timestamp_ms;

        let vol_bp = self.update_volatility(mid_ticks, prev_mid);
        let trend_ticks = self.update_trend(mid_ticks, input.timestamp_ms);
        let sweep_flag = self.evaluate_sweep(
            mid_ticks,
            prev_mid,
            prev_mid_ts,
            input.timestamp_ms,
            sum_bid,
            sum_ask,
            input,
        );

        let ts_coarse = layout::quantise_timestamp_ms(input.timestamp_ms);

        self.last_bid_depth = sum_bid;
        self.last_ask_depth = sum_ask;
        self.last_mid_ticks = Some(mid_ticks);
        self.last_mid_timestamp_ms = input.timestamp_ms;

        self.sequence = (self.sequence + 1) & layout::SEQUENCE_MAX;

        Avs128Snapshot {
            spread_ticks,
            obi_q1_10: obi,
            micro_off_ticks,
            sum_bid_l1_3: sum_bid_field,
            sum_ask_l1_3: sum_ask_field,
            vol_bp_q8_8: vol_bp,
            sweep_flag,
            trend_200ms_ticks: trend_ticks,
            ts_coarse_ms: ts_coarse,
            version: self.cfg.version,
            sequence: self.sequence,
        }
    }

    fn update_volatility(&mut self, mid_ticks: f64, prev_mid: Option<f64>) -> u16 {
        let delta_ticks = match prev_mid {
            Some(prev) => mid_ticks - prev,
            None => 0.0,
        };

        let delta_bp = delta_ticks.abs() * self.cfg.bp_per_tick;
        if prev_mid.is_none() {
            self.vol_ewma_bp = delta_bp;
        } else {
            let alpha = self.cfg.vol_alpha;
            self.vol_ewma_bp = (1.0 - alpha) * self.vol_ewma_bp + alpha * delta_bp;
        }

        layout::encode_vol_bp_q8_8(self.vol_ewma_bp as f32)
    }

    fn update_trend(&mut self, mid_ticks: f64, timestamp_ms: u64) -> i16 {
        self.trend_points.push_back(TrendPoint {
            timestamp_ms,
            mid_ticks,
        });

        while self.trend_points.front().map_or(false, |front| {
            timestamp_ms.saturating_sub(front.timestamp_ms) > self.cfg.trend_window_ms
        }) {
            self.trend_points.pop_front();
        }

        let baseline = self
            .trend_points
            .front()
            .map_or(mid_ticks, |point| point.mid_ticks);
        let raw_trend = (mid_ticks - baseline).round();
        clamp_to_i16(
            raw_trend,
            layout::TREND_200MS_TICKS_MIN,
            layout::TREND_200MS_TICKS_MAX,
        )
    }

    fn evaluate_sweep(
        &mut self,
        mid_ticks: f64,
        prev_mid: Option<f64>,
        prev_mid_ts: u64,
        timestamp_ms: u64,
        sum_bid: u64,
        sum_ask: u64,
        input: &WriterInput,
    ) -> bool {
        let mut triggered = false;

        if let Some(prev_mid) = prev_mid {
            let dt = timestamp_ms.saturating_sub(prev_mid_ts);
            let mid_jump = (mid_ticks - prev_mid).abs() >= self.cfg.sweep_mid_jump_ticks as f64;
            let collapse =
                depth_collapse(sum_bid, self.last_bid_depth, self.cfg.sweep_collapse_ratio)
                    || depth_collapse(sum_ask, self.last_ask_depth, self.cfg.sweep_collapse_ratio);

            if mid_jump && collapse && dt <= self.cfg.sweep_window_ms {
                triggered = true;
            }
        }

        self.last_mid_timestamp_ms = timestamp_ms;

        if input.marketable_volume > 0 {
            let volume = u64::from(input.marketable_volume);
            self.rolling_marketable = self.rolling_marketable.saturating_add(volume);
            self.marketable_window.push_back(MarketableSample {
                timestamp_ms,
                volume,
            });
        }

        while self.marketable_window.front().map_or(false, |front| {
            timestamp_ms.saturating_sub(front.timestamp_ms) > self.cfg.sweep_window_ms
        }) {
            if let Some(sample) = self.marketable_window.pop_front() {
                self.rolling_marketable = self.rolling_marketable.saturating_sub(sample.volume);
            }
        }

        let l1_depth = u64::from(input.bid_sizes[0].max(input.ask_sizes[0]));
        if l1_depth > 0 {
            let threshold = self.cfg.sweep_volume_factor * l1_depth as f64;
            if (self.rolling_marketable as f64) > threshold {
                triggered = true;
            }
        }

        if triggered {
            self.sweep_until_ms = timestamp_ms.saturating_add(self.cfg.sweep_hold_ms);
        }

        if self.sweep_until_ms == 0 {
            false
        } else if timestamp_ms <= self.sweep_until_ms {
            true
        } else {
            self.sweep_until_ms = 0;
            false
        }
    }
}

fn compute_spread(input: &WriterInput) -> u8 {
    let raw = input.ask_px_ticks - input.bid_px_ticks;
    let spread = match raw.cmp(&0) {
        CmpOrdering::Less => 0,
        _ => raw,
    };
    spread.clamp(0, i64::from(layout::SPREAD_TICKS_MAX)) as u8
}

fn sum_levels(levels: &[u32; 3]) -> u64 {
    levels.iter().map(|&v| u64::from(v)).sum()
}

fn compute_microprice(input: &WriterInput) -> (f64, i16) {
    let bid = input.bid_px_ticks as f64;
    let ask = input.ask_px_ticks as f64;
    let mid = (bid + ask) * 0.5;

    let bid_vol = input.bid_sizes[0] as f64;
    let ask_vol = input.ask_sizes[0] as f64;
    let total = bid_vol + ask_vol;

    let micro = if total > 0.0 {
        ((ask * bid_vol) + (bid * ask_vol)) / total
    } else {
        mid
    };

    let offset = (micro - mid).round();
    let clamped = clamp_to_i16(
        offset,
        layout::MICRO_OFF_TICKS_MIN,
        layout::MICRO_OFF_TICKS_MAX,
    );

    (mid, clamped)
}

fn depth_collapse(current: u64, previous: u64, ratio: f64) -> bool {
    if previous == 0 {
        return false;
    }
    let ratio = ratio.clamp(0.0, 1.0);
    (current as f64) <= (previous as f64) * ratio
}

fn clamp_to_i16(value: f64, min: i16, max: i16) -> i16 {
    value
        .clamp(f64::from(min), f64::from(max))
        .round()
        .clamp(f64::from(min), f64::from(max)) as i16
}
