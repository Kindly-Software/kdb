//! Utilities for analysing AVS-128 snapshots over historical replays.

use crate::{layout, Avs128Snapshot};

/// Aggregates statistics while replaying snapshots.
#[derive(Debug, Default, Clone)]
pub struct SnapshotStatsBuilder {
    events: u64,
    sweeps: u64,
    stale: u64,
    max_spread: u8,
    max_vol_bp: f32,
    sum_vol_bp: f64,
    sum_obi: f64,
    sum_micro_abs: f64,
}

impl SnapshotStatsBuilder {
    /// Record a snapshot and update aggregated statistics.
    pub fn observe(&mut self, snapshot: &Avs128Snapshot, now_ms: u64, stale_budget_ms: u64) {
        self.events += 1;
        if snapshot.sweep_flag {
            self.sweeps += 1;
        }
        if snapshot.is_stale(now_ms, stale_budget_ms) {
            self.stale += 1;
        }
        if snapshot.spread_ticks > self.max_spread {
            self.max_spread = snapshot.spread_ticks;
        }

        let vol_bp = layout::decode_vol_bp_q8_8(snapshot.vol_bp_q8_8);
        if vol_bp > self.max_vol_bp {
            self.max_vol_bp = vol_bp;
        }
        self.sum_vol_bp += f64::from(vol_bp);

        let obi_ratio = layout::obi_to_ratio(snapshot.obi_q1_10);
        self.sum_obi += f64::from(obi_ratio);

        self.sum_micro_abs += f64::from(snapshot.micro_off_ticks.abs());
    }

    /// Finalise and obtain the snapshot statistics summary.
    #[must_use]
    pub fn finish(self) -> SnapshotStats {
        let mean_vol_bp = if self.events > 0 {
            (self.sum_vol_bp / self.events as f64) as f32
        } else {
            0.0
        };
        let mean_obi = if self.events > 0 {
            (self.sum_obi / self.events as f64) as f32
        } else {
            0.0
        };
        let mean_abs_micro_ticks = if self.events > 0 {
            (self.sum_micro_abs / self.events as f64) as f32
        } else {
            0.0
        };

        SnapshotStats {
            events: self.events,
            sweeps: self.sweeps,
            stale: self.stale,
            max_spread: self.max_spread,
            max_vol_bp: self.max_vol_bp,
            mean_vol_bp,
            mean_obi,
            mean_abs_micro_ticks,
        }
    }
}

/// Summary statistics gathered after replay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapshotStats {
    /// Total snapshots processed.
    pub events: u64,
    /// Snapshots that carried the sweep flag.
    pub sweeps: u64,
    /// Snapshots considered stale for the supplied budget.
    pub stale: u64,
    /// Maximum spread observed in ticks.
    pub max_spread: u8,
    /// Maximum short-horizon volatility observed (basis points).
    pub max_vol_bp: f32,
    /// Arithmetic mean of short-horizon volatility readings (basis points).
    pub mean_vol_bp: f32,
    /// Arithmetic mean of the order-book imbalance ratio.
    pub mean_obi: f32,
    /// Mean absolute microprice offset in ticks.
    pub mean_abs_micro_ticks: f32,
}

impl SnapshotStats {
    /// Ratio of sweep-flagged snapshots.
    #[must_use]
    pub fn sweep_ratio(&self) -> f32 {
        if self.events == 0 {
            0.0
        } else {
            self.sweeps as f32 / self.events as f32
        }
    }

    /// Ratio of stale snapshots.
    #[must_use]
    pub fn stale_ratio(&self) -> f32 {
        if self.events == 0 {
            0.0
        } else {
            self.stale as f32 / self.events as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_expected_statistics() {
        let mut builder = SnapshotStatsBuilder::default();
        let mut snapshot = Avs128Snapshot {
            spread_ticks: 3,
            obi_q1_10: layout::clamp_obi(256),
            micro_off_ticks: 2,
            sum_bid_l1_3: 100,
            sum_ask_l1_3: 90,
            vol_bp_q8_8: layout::encode_vol_bp_q8_8(1.5),
            sweep_flag: true,
            trend_200ms_ticks: -2,
            ts_coarse_ms: layout::quantise_timestamp_ms(1_000),
            version: 1,
            sequence: 1,
        };

        builder.observe(&snapshot, 1_010, 50);

        snapshot.sweep_flag = false;
        snapshot.spread_ticks = 6;
        snapshot.vol_bp_q8_8 = layout::encode_vol_bp_q8_8(2.2);
        snapshot.micro_off_ticks = -4;
        snapshot.ts_coarse_ms = layout::quantise_timestamp_ms(2_000);

        builder.observe(&snapshot, 2_500, 200);

        let stats = builder.finish();
        assert_eq!(stats.events, 2);
        assert_eq!(stats.sweeps, 1);
        assert_eq!(stats.stale, 1);
        assert_eq!(stats.max_spread, 6);
        assert!(stats.max_vol_bp >= 2.2 - 1e-3);
        assert!(stats.mean_vol_bp > 0.0);
        assert!(stats.mean_abs_micro_ticks >= 0.0);
        assert!(stats.sweep_ratio() > 0.0);
        assert!(stats.stale_ratio() > 0.0);
    }

    #[test]
    fn ratios_are_zero_when_empty() {
        let builder = SnapshotStatsBuilder::default();
        let stats = builder.finish();
        assert_eq!(stats.events, 0);
        assert_eq!(stats.sweep_ratio(), 0.0);
        assert_eq!(stats.stale_ratio(), 0.0);
        assert_eq!(stats.max_vol_bp, 0.0);
    }
}
